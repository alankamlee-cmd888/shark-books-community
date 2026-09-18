#!/usr/bin/env python3
"""Shark Local Executor completion relay v1.0.0.

Deterministic local sidecar for AUT-1. It observes already-published SLE terminal
artifacts and emits a metadata-only canonical event into a Dropbox-synced outbox.

Security properties:
- standard library only;
- no network;
- no subprocess/shell/Git;
- no source-repository access;
- no credential/token reads;
- no copying of arbitrary result fields.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

RELAY_VERSION = "1.0.0"
SCHEMA_VERSION = 1
EVENT_TYPE = "SLE_TASK_TERMINAL"
TASK_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
TERMINAL_NAMES = ("result.json", "error.json", "failure.json")
MAX_TERMINAL_BYTES = 16 * 1024 * 1024
MAX_EVENT_BYTES = 8 * 1024
DEFAULT_POLL_SECONDS = 2.0


class RelayError(RuntimeError):
    """Fail-closed relay policy error."""


@dataclass(frozen=True)
class Candidate:
    task_id: str
    terminal_state: str
    terminal_root_name: str
    artifact_path: Path
    artifact_name: str


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("utf-8")


def safe_task_id(value: str) -> bool:
    return bool(TASK_ID_RE.fullmatch(value))


def default_queue_root() -> Path:
    override = os.environ.get("SHARK_LOCAL_AGENT_ROOT")
    if override:
        return Path(override).expanduser().resolve()
    home = Path.home()
    return (home / "Dropbox" / "SHARK_LOCAL_AGENT").resolve()


def default_state_path() -> Path:
    local_app = os.environ.get("LOCALAPPDATA")
    if local_app:
        base = Path(local_app)
    else:
        base = Path.home() / ".local" / "share"
    return (base / "SharkLocalExecutorRelay" / "state.json").resolve()


def within(child: Path, parent: Path) -> bool:
    try:
        child.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def read_regular_file(path: Path, *, max_bytes: int) -> bytes:
    if path.is_symlink():
        raise RelayError(f"symlink terminal artifact rejected: {path}")
    if not path.is_file():
        raise RelayError(f"terminal artifact is not a regular file: {path}")
    size = path.stat().st_size
    if size < 2:
        raise RelayError(f"terminal artifact too small: {path}")
    if size > max_bytes:
        raise RelayError(f"terminal artifact exceeds {max_bytes} bytes: {path}")
    data = path.read_bytes()
    if len(data) != size:
        raise RelayError(f"terminal artifact changed during read: {path}")
    return data


def discover_candidates(queue_root: Path) -> list[Candidate]:
    queue_root = queue_root.resolve()
    found: list[Candidate] = []
    for root_name, state in (("completed", "COMPLETED"), ("failed", "FAILED")):
        terminal_root = queue_root / root_name
        if not terminal_root.exists():
            continue
        if terminal_root.is_symlink() or not terminal_root.is_dir():
            raise RelayError(f"terminal root invalid: {terminal_root}")
        for task_dir in sorted(terminal_root.iterdir(), key=lambda p: p.name):
            if task_dir.is_symlink() or not task_dir.is_dir():
                continue
            task_id = task_dir.name
            if not safe_task_id(task_id):
                continue
            if not within(task_dir, terminal_root):
                raise RelayError(f"terminal task path escapes root: {task_dir}")
            for artifact_name in TERMINAL_NAMES:
                artifact = task_dir / artifact_name
                if artifact.exists():
                    if not within(artifact, task_dir):
                        raise RelayError(f"terminal artifact escapes task directory: {artifact}")
                    found.append(Candidate(task_id, state, root_name, artifact, artifact_name))
                    break
    return found


def bounded_scalar(value: Any, max_len: int) -> str | None:
    if not isinstance(value, str):
        return None
    if not (1 <= len(value) <= max_len):
        return None
    if any(ord(ch) < 32 for ch in value):
        return None
    return value


def make_event(candidate: Candidate, artifact_bytes: bytes, *, emitted_at: datetime | None = None) -> dict[str, Any]:
    try:
        parsed = json.loads(artifact_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RelayError(f"terminal artifact is not valid UTF-8 JSON: {candidate.artifact_path}") from exc
    if not isinstance(parsed, dict):
        raise RelayError("terminal artifact JSON must be an object")

    embedded_task = parsed.get("task_id")
    if embedded_task is not None and embedded_task != candidate.task_id:
        raise RelayError(f"task_id mismatch: directory={candidate.task_id!r} result={embedded_task!r}")

    digest = sha256_bytes(artifact_bytes)
    size = len(artifact_bytes)
    identity = "|".join(
        [
            str(SCHEMA_VERSION),
            EVENT_TYPE,
            candidate.task_id,
            candidate.terminal_state,
            candidate.artifact_name,
            digest,
            str(size),
        ]
    ).encode("utf-8")
    event_id = sha256_bytes(identity)
    timestamp = emitted_at or datetime.now(timezone.utc)
    if timestamp.tzinfo is None:
        raise RelayError("emitted_at must be timezone-aware")

    event: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "event_type": EVENT_TYPE,
        "event_id": event_id,
        "task_id": candidate.task_id,
        "terminal_state": candidate.terminal_state,
        "terminal_artifact_name": candidate.artifact_name,
        "terminal_artifact_sha256": digest,
        "terminal_artifact_size_bytes": size,
        "evidence_relpath": f"{candidate.terminal_root_name}/{candidate.task_id}/{candidate.artifact_name}",
        "relay_version": RELAY_VERSION,
        "emitted_at_utc": timestamp.astimezone(timezone.utc).isoformat().replace("+00:00", "Z"),
    }

    executor_version = bounded_scalar(parsed.get("executor_version"), 64)
    if executor_version is not None:
        event["executor_version"] = executor_version
    if isinstance(parsed.get("passed"), bool):
        event["passed"] = parsed["passed"]

    encoded = canonical_json_bytes(event)
    if len(encoded) > MAX_EVENT_BYTES:
        raise RelayError("event envelope exceeds maximum size")
    return event


def load_state(path: Path) -> dict[str, str]:
    if not path.exists():
        return {}
    raw = read_regular_file(path, max_bytes=1024 * 1024)
    try:
        value = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RelayError(f"invalid relay state file: {path}") from exc
    if not isinstance(value, dict):
        raise RelayError("relay state must be an object")
    result: dict[str, str] = {}
    for task_id, event_id in value.items():
        if not isinstance(task_id, str) or not safe_task_id(task_id):
            raise RelayError("relay state contains invalid task_id")
        if not isinstance(event_id, str) or not re.fullmatch(r"[0-9a-f]{64}", event_id):
            raise RelayError("relay state contains invalid event_id")
        result[task_id] = event_id
    return result


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(prefix=path.name + ".", suffix=".tmp", dir=path.parent, delete=False) as handle:
        temp_path = Path(handle.name)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    try:
        os.replace(temp_path, path)
    finally:
        if temp_path.exists():
            temp_path.unlink(missing_ok=True)


def save_state(path: Path, state: dict[str, str]) -> None:
    atomic_write(path, canonical_json_bytes(state))


def emit_candidate(candidate: Candidate, *, outbox: Path, state: dict[str, str], state_path: Path) -> str:
    data = read_regular_file(candidate.artifact_path, max_bytes=MAX_TERMINAL_BYTES)
    event = make_event(candidate, data)
    event_id = event["event_id"]
    previous = state.get(candidate.task_id)
    if previous is not None:
        if previous == event_id:
            return "duplicate"
        raise RelayError(
            f"terminal artifact mutation for already-emitted task {candidate.task_id}: old={previous} new={event_id}"
        )

    filename = f"{candidate.task_id}.{event_id}.json"
    target = outbox / filename
    payload = canonical_json_bytes(event)
    if target.exists():
        existing = read_regular_file(target, max_bytes=MAX_EVENT_BYTES)
        if existing != payload:
            raise RelayError(f"pre-existing outbox event bytes differ: {target}")
    else:
        atomic_write(target, payload)

    state[candidate.task_id] = event_id
    save_state(state_path, state)
    return "emitted"


def run_once(queue_root: Path, outbox: Path, state_path: Path) -> tuple[int, int]:
    queue_root = queue_root.resolve()
    outbox = outbox.resolve()
    state_path = state_path.resolve()
    if not within(outbox, queue_root):
        raise RelayError("relay outbox must remain inside SHARK_LOCAL_AGENT root")
    state = load_state(state_path)
    emitted = 0
    duplicates = 0
    for candidate in discover_candidates(queue_root):
        status = emit_candidate(candidate, outbox=outbox, state=state, state_path=state_path)
        if status == "emitted":
            emitted += 1
        else:
            duplicates += 1
    return emitted, duplicates


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Emit metadata-only SLE terminal events to a local Dropbox outbox.")
    parser.add_argument("--queue-root", type=Path, default=None)
    parser.add_argument("--outbox", type=Path, default=None)
    parser.add_argument("--state", type=Path, default=None)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--once", action="store_true", help="scan once and exit (default)")
    mode.add_argument("--watch", action="store_true", help="poll locally for terminal evidence")
    parser.add_argument("--poll-seconds", type=float, default=DEFAULT_POLL_SECONDS)
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    queue_root = (args.queue_root or default_queue_root()).expanduser().resolve()
    outbox = (args.outbox or (queue_root / "relay_outbox")).expanduser().resolve()
    state_path = (args.state or default_state_path()).expanduser().resolve()
    if args.poll_seconds < 0.5 or args.poll_seconds > 60:
        raise RelayError("poll interval must be between 0.5 and 60 seconds")

    while True:
        emitted, duplicates = run_once(queue_root, outbox, state_path)
        if not args.watch:
            print(json.dumps({"relay_version": RELAY_VERSION, "emitted": emitted, "duplicates": duplicates}, sort_keys=True))
            return 0
        time.sleep(args.poll_seconds)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RelayError as exc:
        print(f"RELAY_POLICY_ERROR: {exc}", file=sys.stderr)
        raise SystemExit(2)
