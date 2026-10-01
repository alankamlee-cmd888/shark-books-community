#!/usr/bin/env python3
"""Shark Local Executor completion relay v1.0.1.

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

RELAY_VERSION = "1.0.1"
SCHEMA_VERSION = 1
EVENT_TYPE = "SLE_TASK_TERMINAL"
TASK_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
TERMINAL_NAMES = ("result.json", "error.json", "failure.json")
MAX_TERMINAL_BYTES = 16 * 1024 * 1024
MAX_EVENT_BYTES = 8 * 1024
DEFAULT_POLL_SECONDS = 2.0
HEARTBEAT_NAME = "relay_heartbeat.json"
DROPBOX_REPLACE_TIMEOUT_SECONDS = 15.0
DROPBOX_REPLACE_INITIAL_DELAY_SECONDS = 0.05
DROPBOX_REPLACE_MAX_DELAY_SECONDS = 0.5


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


def utc_now_text() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def safe_task_id(value: str) -> bool:
    return bool(TASK_ID_RE.fullmatch(value))


def default_queue_root() -> Path:
    override = os.environ.get("SHARK_LOCAL_AGENT_ROOT")
    if override:
        return Path(override).expanduser().resolve()
    return (Path.home() / "Dropbox" / "SHARK_LOCAL_AGENT").resolve()


def default_state_dir() -> Path:
    local_app = os.environ.get("LOCALAPPDATA")
    base = Path(local_app) if local_app else Path.home() / ".local" / "share"
    return (base / "SharkLocalExecutorRelay").resolve()


def default_state_path() -> Path:
    return default_state_dir() / "state.json"


def default_lock_path() -> Path:
    return default_state_dir() / "relay.lock"


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


def event_id_for(candidate: Candidate, artifact_bytes: bytes) -> str:
    digest = sha256_bytes(artifact_bytes)
    identity = "|".join(
        [
            str(SCHEMA_VERSION),
            EVENT_TYPE,
            candidate.task_id,
            candidate.terminal_state,
            candidate.artifact_name,
            digest,
            str(len(artifact_bytes)),
        ]
    ).encode("utf-8")
    return sha256_bytes(identity)


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
    timestamp = emitted_at or datetime.now(timezone.utc)
    if timestamp.tzinfo is None:
        raise RelayError("emitted_at must be timezone-aware")

    event: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "event_type": EVENT_TYPE,
        "event_id": event_id_for(candidate, artifact_bytes),
        "task_id": candidate.task_id,
        "terminal_state": candidate.terminal_state,
        "terminal_artifact_name": candidate.artifact_name,
        "terminal_artifact_sha256": digest,
        "terminal_artifact_size_bytes": len(artifact_bytes),
        "evidence_relpath": f"{candidate.terminal_root_name}/{candidate.task_id}/{candidate.artifact_name}",
        "relay_version": RELAY_VERSION,
        "emitted_at_utc": timestamp.astimezone(timezone.utc).isoformat().replace("+00:00", "Z"),
    }

    executor_version = bounded_scalar(parsed.get("executor_version"), 64)
    if executor_version is not None:
        event["executor_version"] = executor_version
    if isinstance(parsed.get("passed"), bool):
        event["passed"] = parsed["passed"]

    if len(canonical_json_bytes(event)) > MAX_EVENT_BYTES:
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


def _retryable_replace_error(exc: OSError) -> bool:
    # Dropbox Desktop and Windows AV/indexing can briefly deny replacement of a
    # synced destination (commonly WinError 5 / 32). PermissionError is also
    # retryable on non-Windows test hosts so the contention contract is portable.
    return isinstance(exc, PermissionError) or getattr(exc, "winerror", None) in {5, 32}


def replace_with_retry(
    temp_path: Path,
    path: Path,
    *,
    timeout_seconds: float = DROPBOX_REPLACE_TIMEOUT_SECONDS,
    initial_delay_seconds: float = DROPBOX_REPLACE_INITIAL_DELAY_SECONDS,
    max_delay_seconds: float = DROPBOX_REPLACE_MAX_DELAY_SECONDS,
) -> int:
    if timeout_seconds <= 0:
        raise RelayError("atomic replace retry timeout must be positive")
    deadline = time.monotonic() + timeout_seconds
    delay = max(0.001, initial_delay_seconds)
    attempts = 0
    while True:
        try:
            os.replace(temp_path, path)
            return attempts
        except OSError as exc:
            attempts += 1
            if not _retryable_replace_error(exc) or time.monotonic() >= deadline:
                raise RelayError(
                    f"atomic replace failed for {path.name} after {attempts} attempt(s): {exc}"
                ) from exc
            time.sleep(delay)
            delay = min(max_delay_seconds, max(delay, 0.001) * 2)


def atomic_write(
    path: Path,
    data: bytes,
    *,
    replace_timeout_seconds: float = DROPBOX_REPLACE_TIMEOUT_SECONDS,
    initial_retry_seconds: float = DROPBOX_REPLACE_INITIAL_DELAY_SECONDS,
    max_retry_seconds: float = DROPBOX_REPLACE_MAX_DELAY_SECONDS,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(prefix=path.name + ".", suffix=".tmp", dir=path.parent, delete=False) as handle:
        temp_path = Path(handle.name)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    try:
        replace_with_retry(
            temp_path,
            path,
            timeout_seconds=replace_timeout_seconds,
            initial_delay_seconds=initial_retry_seconds,
            max_delay_seconds=max_retry_seconds,
        )
    finally:
        if temp_path.exists():
            temp_path.unlink(missing_ok=True)


def save_state(path: Path, state: dict[str, str]) -> None:
    atomic_write(path, canonical_json_bytes(state))


def baseline_existing(queue_root: Path, state_path: Path) -> int:
    if state_path.exists():
        raise RelayError("baseline requested but relay state already exists")
    state: dict[str, str] = {}
    for candidate in discover_candidates(queue_root):
        data = read_regular_file(candidate.artifact_path, max_bytes=MAX_TERMINAL_BYTES)
        state[candidate.task_id] = event_id_for(candidate, data)
    save_state(state_path, state)
    return len(state)


def write_heartbeat(queue_root: Path, *, state: str, note: str = "", baseline_count: int | None = None) -> None:
    heartbeat: dict[str, Any] = {
        "relay_version": RELAY_VERSION,
        "pid": os.getpid(),
        "updated_at": utc_now_text(),
        "state": state,
        "outbox_relpath": "relay_outbox",
        "network_authority": False,
    }
    if note:
        heartbeat["note"] = note[:256]
    if baseline_count is not None:
        heartbeat["baseline_count"] = baseline_count
    atomic_write(queue_root / HEARTBEAT_NAME, canonical_json_bytes(heartbeat))


class SingleInstance:
    def __init__(self, path: Path):
        self.path = path
        self.handle = None

    def __enter__(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.handle = self.path.open("a+b")
        self.handle.seek(0)
        if self.handle.tell() == 0:
            self.handle.write(b"0")
            self.handle.flush()
        self.handle.seek(0)
        try:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (OSError, BlockingIOError) as exc:
            self.handle.close()
            self.handle = None
            raise RelayError("another relay instance already holds the single-instance lock") from exc
        return self

    def __exit__(self, exc_type, exc, tb):
        if self.handle is None:
            return
        try:
            self.handle.seek(0)
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.handle.fileno(), fcntl.LOCK_UN)
        finally:
            self.handle.close()
            self.handle = None


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

    target = outbox / f"{candidate.task_id}.{event_id}.json"
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
    parser.add_argument("--lock", type=Path, default=None)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--once", action="store_true", help="scan once and exit (default)")
    mode.add_argument("--watch", action="store_true", help="poll locally for terminal evidence")
    mode.add_argument("--baseline-existing", action="store_true", help="record existing terminal tasks without emitting events")
    parser.add_argument("--poll-seconds", type=float, default=DEFAULT_POLL_SECONDS)
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    queue_root = (args.queue_root or default_queue_root()).expanduser().resolve()
    outbox = (args.outbox or (queue_root / "relay_outbox")).expanduser().resolve()
    state_path = (args.state or default_state_path()).expanduser().resolve()
    lock_path = (args.lock or default_lock_path()).expanduser().resolve()
    if args.poll_seconds < 0.5 or args.poll_seconds > 60:
        raise RelayError("poll interval must be between 0.5 and 60 seconds")

    if args.baseline_existing:
        count = baseline_existing(queue_root, state_path)
        write_heartbeat(queue_root, state="BASELINED", baseline_count=count)
        print(json.dumps({"relay_version": RELAY_VERSION, "baselined": count}, sort_keys=True))
        return 0

    if not args.watch:
        emitted, duplicates = run_once(queue_root, outbox, state_path)
        write_heartbeat(queue_root, state="ONCE_COMPLETE", note=f"emitted={emitted};duplicates={duplicates}")
        print(json.dumps({"relay_version": RELAY_VERSION, "emitted": emitted, "duplicates": duplicates}, sort_keys=True))
        return 0

    with SingleInstance(lock_path):
        write_heartbeat(queue_root, state="WATCHING")
        while True:
            try:
                emitted, duplicates = run_once(queue_root, outbox, state_path)
                write_heartbeat(queue_root, state="WATCHING", note=f"emitted={emitted};duplicates={duplicates}")
            except RelayError as exc:
                write_heartbeat(queue_root, state="FAIL_CLOSED", note=str(exc))
                raise
            time.sleep(args.poll_seconds)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RelayError as exc:
        print(f"RELAY_POLICY_ERROR: {exc}", file=sys.stderr)
        raise SystemExit(2)
