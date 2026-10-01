#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import os
import tempfile
import unittest
import sys
from unittest.mock import patch
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("sle_completion_relay", HERE / "sle_completion_relay.py")
relay = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = relay
SPEC.loader.exec_module(relay)


class RelayTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / "SHARK_LOCAL_AGENT"
        self.completed = self.root / "completed"
        self.failed = self.root / "failed"
        self.outbox = self.root / "relay_outbox"
        self.state = Path(self.tmp.name) / "local_state" / "state.json"
        self.completed.mkdir(parents=True)
        self.failed.mkdir(parents=True)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write_result(self, task_id: str, obj: dict, root: str = "completed", name: str = "result.json") -> Path:
        path = self.root / root / task_id / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf-8")
        return path

    def events(self) -> list[dict]:
        return [json.loads(p.read_text(encoding="utf-8")) for p in sorted(self.outbox.glob("*.json"))]

    def test_completed_event_is_metadata_only(self) -> None:
        self.write_result(
            "SLE-TEST-001",
            {
                "task_id": "SLE-TEST-001",
                "executor_version": "1.9.0",
                "passed": True,
                "source_code": "SECRET SOURCE SHOULD NOT COPY",
                "prompt": "DO NOT COPY",
                "token": "ghp_not_real",
                "stdout": "sensitive output",
            },
        )
        emitted, duplicates = relay.run_once(self.root, self.outbox, self.state)
        self.assertEqual((emitted, duplicates), (1, 0))
        event = self.events()[0]
        self.assertEqual(event["task_id"], "SLE-TEST-001")
        self.assertEqual(event["terminal_state"], "COMPLETED")
        self.assertEqual(event["executor_version"], "1.9.0")
        self.assertIs(event["passed"], True)
        self.assertNotIn("source_code", event)
        self.assertNotIn("prompt", event)
        self.assertNotIn("token", event)
        self.assertNotIn("stdout", event)
        payload = json.dumps(event)
        self.assertNotIn("SECRET SOURCE", payload)
        self.assertNotIn("ghp_", payload)

    def test_duplicate_is_not_reemitted(self) -> None:
        self.write_result("SLE-TEST-002", {"task_id": "SLE-TEST-002", "passed": True})
        self.assertEqual(relay.run_once(self.root, self.outbox, self.state), (1, 0))
        self.assertEqual(relay.run_once(self.root, self.outbox, self.state), (0, 1))
        self.assertEqual(len(list(self.outbox.glob("*.json"))), 1)

    def test_same_task_terminal_mutation_fails_closed(self) -> None:
        result = self.write_result("SLE-TEST-003", {"task_id": "SLE-TEST-003", "passed": True})
        relay.run_once(self.root, self.outbox, self.state)
        result.write_text(json.dumps({"task_id": "SLE-TEST-003", "passed": False}) + "\n", encoding="utf-8")
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)

    def test_task_id_mismatch_rejected(self) -> None:
        self.write_result("SLE-TEST-004", {"task_id": "OTHER"})
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)
        self.assertFalse(self.outbox.exists())

    def test_malformed_json_rejected(self) -> None:
        p = self.root / "completed" / "SLE-TEST-005" / "result.json"
        p.parent.mkdir(parents=True)
        p.write_text("{not json}\n", encoding="utf-8")
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)

    def test_unsafe_task_directory_ignored(self) -> None:
        p = self.completed / "bad id" / "result.json"
        p.parent.mkdir(parents=True)
        p.write_text('{"task_id":"bad id"}\n', encoding="utf-8")
        self.assertEqual(relay.run_once(self.root, self.outbox, self.state), (0, 0))

    def test_failed_event_supported(self) -> None:
        self.write_result("SLE-TEST-006", {"task_id": "SLE-TEST-006", "passed": False}, root="failed", name="error.json")
        self.assertEqual(relay.run_once(self.root, self.outbox, self.state), (1, 0))
        event = self.events()[0]
        self.assertEqual(event["terminal_state"], "FAILED")
        self.assertEqual(event["terminal_artifact_name"], "error.json")

    def test_reordered_json_hashes_actual_bytes(self) -> None:
        p = self.write_result("SLE-TEST-007", {"task_id": "SLE-TEST-007", "passed": True})
        data1 = p.read_bytes()
        c = relay.discover_candidates(self.root)[0]
        e1 = relay.make_event(c, data1, emitted_at=datetime(2026, 9, 18, tzinfo=timezone.utc))
        p.write_text('{"passed": true, "task_id": "SLE-TEST-007"}\n', encoding="utf-8")
        data2 = p.read_bytes()
        e2 = relay.make_event(c, data2, emitted_at=datetime(2026, 9, 18, tzinfo=timezone.utc))
        self.assertNotEqual(e1["terminal_artifact_sha256"], e2["terminal_artifact_sha256"])
        self.assertNotEqual(e1["event_id"], e2["event_id"])
        self.assertEqual(e1["task_id"], e2["task_id"])

    def test_tampered_existing_outbox_rejected(self) -> None:
        self.write_result("SLE-TEST-008", {"task_id": "SLE-TEST-008", "passed": True})
        candidate = relay.discover_candidates(self.root)[0]
        data = relay.read_regular_file(candidate.artifact_path, max_bytes=relay.MAX_TERMINAL_BYTES)
        event = relay.make_event(candidate, data)
        target = self.outbox / f"{event['task_id']}.{event['event_id']}.json"
        target.parent.mkdir(parents=True)
        target.write_text('{"tampered":true}\n', encoding="utf-8")
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)

    @unittest.skipIf(os.name == "nt", "Windows symlink creation often requires elevated/developer mode")
    def test_symlink_terminal_artifact_rejected(self) -> None:
        outside = Path(self.tmp.name) / "outside.json"
        outside.write_text('{"task_id":"SLE-TEST-009"}\n', encoding="utf-8")
        task = self.completed / "SLE-TEST-009"
        task.mkdir(parents=True)
        (task / "result.json").symlink_to(outside)
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)

    def test_oversized_terminal_artifact_rejected(self) -> None:
        p = self.completed / "SLE-TEST-010" / "result.json"
        p.parent.mkdir(parents=True)
        p.write_bytes(b"{" + b"x" * relay.MAX_TERMINAL_BYTES + b"}")
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)

    def test_outbox_must_remain_inside_queue_root(self) -> None:
        self.write_result("SLE-TEST-011", {"task_id": "SLE-TEST-011", "passed": True})
        outside = Path(self.tmp.name) / "outside_outbox"
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, outside, self.state)

    def test_invalid_state_fails_closed(self) -> None:
        self.write_result("SLE-TEST-012", {"task_id": "SLE-TEST-012", "passed": True})
        self.state.parent.mkdir(parents=True)
        self.state.write_text('{"../escape":"not-a-hash"}\n', encoding="utf-8")
        with self.assertRaises(relay.RelayError):
            relay.run_once(self.root, self.outbox, self.state)

    def test_baseline_existing_suppresses_history(self) -> None:
        self.write_result("SLE-HIST-001", {"task_id": "SLE-HIST-001", "passed": True})
        count = relay.baseline_existing(self.root, self.state)
        self.assertEqual(count, 1)
        self.assertFalse(self.outbox.exists())
        self.assertEqual(relay.run_once(self.root, self.outbox, self.state), (0, 1))

    def test_baseline_cannot_overwrite_existing_state(self) -> None:
        self.write_result("SLE-HIST-002", {"task_id": "SLE-HIST-002", "passed": True})
        relay.baseline_existing(self.root, self.state)
        with self.assertRaises(relay.RelayError):
            relay.baseline_existing(self.root, self.state)

    def test_heartbeat_is_metadata_only(self) -> None:
        relay.write_heartbeat(self.root, state="WATCHING", note="ok", baseline_count=3)
        hb = json.loads((self.root / relay.HEARTBEAT_NAME).read_text(encoding="utf-8"))
        self.assertEqual(hb["relay_version"], relay.RELAY_VERSION)
        self.assertIs(hb["network_authority"], False)
        self.assertEqual(hb["outbox_relpath"], "relay_outbox")
        self.assertNotIn("queue_root", hb)

    def test_atomic_write_retries_transient_permission_error(self) -> None:
        target = self.root / "retry.json"
        real_replace = os.replace
        calls = {"n": 0}

        def flaky_replace(src, dst):
            calls["n"] += 1
            if calls["n"] <= 3:
                raise PermissionError(13, "simulated Dropbox contention")
            return real_replace(src, dst)

        with patch.object(relay.os, "replace", side_effect=flaky_replace):
            relay.atomic_write(
                target,
                b'{"ok":true}\n',
                replace_timeout_seconds=1.0,
                initial_retry_seconds=0.001,
                max_retry_seconds=0.002,
            )

        self.assertEqual(calls["n"], 4)
        self.assertEqual(target.read_bytes(), b'{"ok":true}\n')
        self.assertEqual(list(target.parent.glob(target.name + ".*.tmp")), [])

    def test_atomic_write_fails_closed_after_persistent_permission_error(self) -> None:
        target = self.root / "persistent.json"
        with patch.object(relay.os, "replace", side_effect=PermissionError(13, "persistent contention")):
            with self.assertRaises(relay.RelayError):
                relay.atomic_write(
                    target,
                    b'{"ok":false}\n',
                    replace_timeout_seconds=0.01,
                    initial_retry_seconds=0.001,
                    max_retry_seconds=0.002,
                )
        self.assertFalse(target.exists())
        self.assertEqual(list(target.parent.glob(target.name + ".*.tmp")), [])

    def test_single_instance_lock_rejects_second_holder(self) -> None:
        lock = Path(self.tmp.name) / "lock" / "relay.lock"
        with relay.SingleInstance(lock):
            with self.assertRaises(relay.RelayError):
                with relay.SingleInstance(lock):
                    pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
