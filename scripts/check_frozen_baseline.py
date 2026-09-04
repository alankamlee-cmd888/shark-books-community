#!/usr/bin/env python3
"""Frozen foundation + approved SBC-1B/1C boundary guard for Shark Books Community."""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FACADE = ROOT / "workspace" / "shark-foundation" / "src" / "lib.rs"
TAURI_ADAPTER = ROOT / "workspace" / "shark-tauri-spike" / "src" / "lib.rs"
LOCK = ROOT / "workspace" / "Cargo.lock"
FOUNDATION_TOML = ROOT / "workspace" / "shark-foundation" / "Cargo.toml"
TAURI_TOML = ROOT / "workspace" / "shark-tauri-spike" / "Cargo.toml"
MANIFEST = ROOT / "docs" / "frozen" / "41_SBC0E_FROZEN_FOUNDATION_MANIFEST_2026-09-04.json"
SBC1B_CONTRACT = ROOT / "docs" / "SBC1B_PRODUCTION_FACADE_CONTRACT_2026-09-04.json"
SBC1C_CONTRACT = ROOT / "docs" / "SBC1C_ENCRYPTED_LIFECYCLE_CONTRACT_2026-09-04.json"
BOOTSTRAP = ROOT / "scripts" / "bootstrap_beankeeper.py"

FROZEN_FACADE = "2c679b0fd5e146e2f82050a32e047c48fa9aa02d386964f8b307c2cae68fb87b"
SBC1B_FACADE = "422a39c1735a347d9472ff61f45cea992765b31b216ac4531a78fd424bde1503"
EXPECTED_SBC1C_FACADE = "6274cffc89ccb2fcea4d489e76375c9c5be1e3c8cc2fbd4c4e56b37bdef0ef46"
EXPECTED_TAURI = "4b098cdea4b840af8fef3dc09a37d2f26a4e1738d42d26a14c3044f0ddf88204"
EXPECTED_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_BEANKEEPER = "d573db5e61089b0922f95c991732394d08e3cf92"
EXPECTED_SBC1B_HEAD = "603f4d9cafc757b79cfd6aab01de85762889acba"
UPSTREAM_PUBLIC_NAMES = re.compile(
    r"\b(?:beankeeper|beankeeper_cli|rusqlite|CliError|Db|Actor|PostTransactionParams|PostEntryParams|StoreAttachmentParams|SecretString)\b"
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def public_declarations(source: str) -> list[str]:
    declarations: list[str] = []
    lines = source.splitlines()
    i = 0
    while i < len(lines):
        stripped = lines[i].lstrip()
        if not stripped.startswith("pub "):
            i += 1
            continue
        parts = [stripped]
        if stripped.startswith("pub fn ") or stripped.startswith("pub trait "):
            while "{" not in parts[-1] and ";" not in parts[-1] and i + 1 < len(lines):
                i += 1
                parts.append(lines[i].strip())
        declarations.append(" ".join(parts))
        i += 1
    return declarations


def main() -> int:
    required = (
        FACADE, TAURI_ADAPTER, LOCK, FOUNDATION_TOML, TAURI_TOML,
        MANIFEST, SBC1B_CONTRACT, SBC1C_CONTRACT, BOOTSTRAP,
    )
    for path in required:
        if not path.is_file():
            fail(f"missing required baseline/contract file: {path.relative_to(ROOT)}")

    if sha256(LOCK) != EXPECTED_LOCK:
        fail("Cargo.lock hash drift")

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest["accounting"]["commit"] != EXPECTED_BEANKEEPER:
        fail("frozen manifest Beankeeper commit drift")
    if manifest["accounting"]["facade_sha256"] != FROZEN_FACADE:
        fail("frozen SBC-0 facade provenance drift")
    if manifest["dependency_baseline"]["cargo_lock_sha256"] != EXPECTED_LOCK:
        fail("frozen manifest lock hash drift")
    if manifest["dependency_baseline"]["tauri_core"] != "2.11.5":
        fail("frozen Tauri core drift")
    if manifest["dependency_baseline"]["tauri_cli"] != "2.11.4":
        fail("frozen Tauri CLI drift")
    if manifest["dependency_baseline"]["tauri_build"] != "2.6.3":
        fail("frozen Tauri build drift")

    sbc1b = json.loads(SBC1B_CONTRACT.read_text(encoding="utf-8"))
    if sbc1b.get("foundation", {}).get("production_facade_sha256") != SBC1B_FACADE:
        fail("SBC-1B production facade provenance drift")

    contract = json.loads(SBC1C_CONTRACT.read_text(encoding="utf-8"))
    foundation = contract.get("foundation", {})
    if contract.get("gate") != "SBC-1C":
        fail("encrypted lifecycle contract gate is not SBC-1C")
    if contract.get("sbc1b_base_head") != EXPECTED_SBC1B_HEAD:
        fail("SBC-1C contract has wrong SBC-1B base HEAD")
    if foundation.get("beankeeper_commit") != EXPECTED_BEANKEEPER:
        fail("SBC-1C contract Beankeeper commit drift")
    if foundation.get("frozen_baseline_facade_sha256") != FROZEN_FACADE:
        fail("SBC-1C contract lost frozen facade provenance")
    if foundation.get("sbc1b_production_facade_sha256") != SBC1B_FACADE:
        fail("SBC-1C contract lost SBC-1B facade provenance")
    if foundation.get("cargo_lock_sha256") != EXPECTED_LOCK:
        fail("SBC-1C contract lock hash drift")
    if contract.get("dependency_change") is not False or contract.get("cargo_update_permitted") is not False:
        fail("SBC-1C contract improperly permits dependency drift")
    if sha256(FACADE) != EXPECTED_SBC1C_FACADE:
        fail("SBC-1C production facade hash drift")
    if foundation.get("sbc1c_production_facade_sha256") != EXPECTED_SBC1C_FACADE:
        fail("SBC-1C contract facade hash mismatch")
    if sha256(TAURI_ADAPTER) != EXPECTED_TAURI:
        fail("SBC-1C Tauri adapter hash drift")

    foundation_toml = FOUNDATION_TOML.read_text(encoding="utf-8")
    if 'beankeeper = { path = "../../upstream/beankeeper/beankeeper" }' not in foundation_toml:
        fail("Beankeeper path dependency drift")
    if 'beankeeper-cli = { path = "../../upstream/beankeeper/beankeeper-cli" }' not in foundation_toml:
        fail("Beankeeper CLI path dependency drift")
    tauri_toml = TAURI_TOML.read_text(encoding="utf-8")
    for expected in ('tauri = { version = "=2.11.5"', 'tauri-build = { version = "=2.6.3"'):
        if expected not in tauri_toml:
            fail(f"Tauri manifest pin drift: {expected}")

    bootstrap = BOOTSTRAP.read_text(encoding="utf-8")
    if EXPECTED_BEANKEEPER not in bootstrap:
        fail("bootstrap script does not pin frozen Beankeeper commit")

    facade = FACADE.read_text(encoding="utf-8")
    validation = facade.find("journal.post().map_err(validation_error)?")
    persistence = facade.find("db::post_transaction(self.db.conn(), &params)")
    if validation < 0 or persistence < 0 or validation >= persistence:
        fail("typed Beankeeper validation is not demonstrably before persistence")

    required_symbols = (
        "pub struct BooksKey",
        "pub trait SecureKeyProvider",
        "pub trait BackupBeforeMigrationHook",
        "pub struct SiblingEncryptedBackup",
        "pub struct BackupReceipt",
        "pub fn create_encrypted(",
        "pub fn open_encrypted(",
        "pub const fn production_encryption_required()",
    )
    for symbol in required_symbols:
        if symbol not in facade:
            fail(f"required SBC-1C contract symbol missing: {symbol}")

    if "pub fn create_plain" in facade or "pub fn open_plain" in facade:
        fail("plaintext production constructor exposed")
    if "passphrase: &str" in facade:
        fail("passphrase string leaked into production facade source")
    if "let _backup = backup_hook.backup_before_open(path, books_id)?;" not in facade:
        fail("pre-open backup hook missing")
    backup = facade.find("backup_hook.backup_before_open(path, books_id)?")
    db_open = facade.find("Db::open(path, Some(key.secret()))", backup)
    if backup < 0 or db_open < 0 or backup >= db_open:
        fail("backup hook is not demonstrably before encrypted Db::open")
    for required_backup_anchor in (
        "reusable_or_next_backup_path(path)?",
        'for suffix in ["-wal", "-journal"]',
        '"-shm"',
        "persistent_companion_backups",
        "persistent_sqlite_snapshot_matches",
        "reused_existing_snapshot",
    ):
        if required_backup_anchor not in facade:
            fail(f"required non-overwriting/WAL backup anchor missing: {required_backup_anchor}")
    if "pub trait SecureKeyProvider: Send + Sync" not in facade:
        fail("secure key provider is not platform-state safe (Send + Sync)")
    if "pub trait BackupBeforeMigrationHook: Send + Sync" not in facade:
        fail("backup hook is not platform-state safe (Send + Sync)")

    for declaration in public_declarations(facade):
        if UPSTREAM_PUBLIC_NAMES.search(declaration):
            fail(f"raw upstream/secret type leaked through public facade declaration: {declaration}")
        if "passphrase" in declaration.lower():
            fail(f"passphrase leaked through public facade declaration: {declaration}")

    for test_name in (
        "cli_error_mapping_is_stable_and_shark_owned",
        "metadata_is_shark_owned_and_versioned",
        "unbalanced_post_is_rejected_before_database_mutation",
        "books_key_debug_is_redacted",
        "encrypted_create_reopen_and_backup_contract",
        "backup_is_non_overwriting_and_preserves_persistent_companions",
        "wrong_key_fails_without_mutating_encrypted_database",
        "production_plaintext_constructors_are_not_exposed",
    ):
        if f"fn {test_name}()" not in facade:
            fail(f"required regression test missing: {test_name}")

    print("PASS: Shark frozen-foundation + SBC-1B/1C encrypted-lifecycle guard")
    print(f"  frozen_facade_sha256={FROZEN_FACADE}")
    print(f"  sbc1b_facade_sha256={SBC1B_FACADE}")
    print(f"  sbc1c_facade_sha256={EXPECTED_SBC1C_FACADE}")
    print(f"  cargo_lock_sha256={EXPECTED_LOCK}")
    print(f"  beankeeper_commit={EXPECTED_BEANKEEPER}")
    print("  production_encryption=mandatory")
    print("  key_boundary=Shark BooksKey + SecureKeyProvider")
    print("  backup_boundary=BackupBeforeMigrationHook before existing Db::open")
    print("  invariant=typed Beankeeper validation before persistence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
