#!/usr/bin/env python3
"""SBC-1A frozen-baseline integrity and accounting-boundary checks."""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FACADE = ROOT / "workspace" / "shark-foundation" / "src" / "lib.rs"
LOCK = ROOT / "workspace" / "Cargo.lock"
FOUNDATION_TOML = ROOT / "workspace" / "shark-foundation" / "Cargo.toml"
TAURI_TOML = ROOT / "workspace" / "shark-tauri-spike" / "Cargo.toml"
MANIFEST = ROOT / "docs" / "frozen" / "41_SBC0E_FROZEN_FOUNDATION_MANIFEST_2026-09-04.json"
BOOTSTRAP = ROOT / "scripts" / "bootstrap_beankeeper.py"

EXPECTED_FACADE = "2c679b0fd5e146e2f82050a32e047c48fa9aa02d386964f8b307c2cae68fb87b"
EXPECTED_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_BEANKEEPER = "d573db5e61089b0922f95c991732394d08e3cf92"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def main() -> int:
    for path in (FACADE, LOCK, FOUNDATION_TOML, TAURI_TOML, MANIFEST, BOOTSTRAP):
        if not path.is_file():
            fail(f"missing required baseline file: {path.relative_to(ROOT)}")

    if sha256(FACADE) != EXPECTED_FACADE:
        fail("Shark facade hash drift")
    if sha256(LOCK) != EXPECTED_LOCK:
        fail("Cargo.lock hash drift")

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest["accounting"]["commit"] != EXPECTED_BEANKEEPER:
        fail("frozen manifest Beankeeper commit drift")
    if manifest["accounting"]["facade_sha256"] != EXPECTED_FACADE:
        fail("frozen manifest facade hash drift")
    if manifest["dependency_baseline"]["cargo_lock_sha256"] != EXPECTED_LOCK:
        fail("frozen manifest lock hash drift")
    if manifest["dependency_baseline"]["tauri_core"] != "2.11.5":
        fail("frozen manifest Tauri core drift")
    if manifest["dependency_baseline"]["tauri_cli"] != "2.11.4":
        fail("frozen manifest Tauri CLI drift")
    if manifest["dependency_baseline"]["tauri_build"] != "2.6.3":
        fail("frozen manifest Tauri build drift")

    foundation_toml = FOUNDATION_TOML.read_text(encoding="utf-8")
    if 'beankeeper = { path = "../../upstream/beankeeper/beankeeper" }' not in foundation_toml:
        fail("Beankeeper path dependency drift")
    if 'beankeeper-cli = { path = "../../upstream/beankeeper/beankeeper-cli" }' not in foundation_toml:
        fail("Beankeeper CLI path dependency drift")

    tauri_toml = TAURI_TOML.read_text(encoding="utf-8")
    for needle in ('tauri = { version = "=2.11.5"', 'tauri-build = { version = "=2.6.3"'):
        if needle not in tauri_toml:
            fail(f"Tauri exact pin missing: {needle}")

    bootstrap = BOOTSTRAP.read_text(encoding="utf-8")
    if EXPECTED_BEANKEEPER not in bootstrap:
        fail("bootstrap script does not pin frozen Beankeeper commit")

    facade = FACADE.read_text(encoding="utf-8")
    validation = facade.find("journal.post().map_err(err)?")
    persistence = facade.find("db::post_transaction(self.db.conn(), &params)")
    if validation < 0 or persistence < 0 or validation >= persistence:
        fail("typed Beankeeper validation is not demonstrably before persistence")

    # The frozen public surface must not name raw upstream persistence types.
    for line in facade.splitlines():
        stripped = line.strip()
        if stripped.startswith("pub ") and re.search(r"\b(beankeeper|beankeeper_cli|Db|PostTransactionParams|PostEntryParams)\b", stripped):
            fail(f"raw upstream type leaked through public facade: {stripped}")

    print("PASS: SBC-1A frozen baseline integrity")
    print(f"  facade_sha256={EXPECTED_FACADE}")
    print(f"  cargo_lock_sha256={EXPECTED_LOCK}")
    print(f"  beankeeper_commit={EXPECTED_BEANKEEPER}")
    print("  invariant=typed Beankeeper validation before persistence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
