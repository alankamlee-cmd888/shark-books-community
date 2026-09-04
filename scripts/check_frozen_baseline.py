#!/usr/bin/env python3
"""Frozen-foundation + approved production-facade guard for Shark Books Community."""
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
CONTRACT = ROOT / "docs" / "SBC1B_PRODUCTION_FACADE_CONTRACT_2026-09-04.json"
BOOTSTRAP = ROOT / "scripts" / "bootstrap_beankeeper.py"

FROZEN_FACADE = "2c679b0fd5e146e2f82050a32e047c48fa9aa02d386964f8b307c2cae68fb87b"
EXPECTED_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_BEANKEEPER = "d573db5e61089b0922f95c991732394d08e3cf92"
EXPECTED_SBC1A_HEAD = "6233a44c802c1205eb5958c5037b3df294c97d99"
UPSTREAM_PUBLIC_NAMES = re.compile(
    r"\b(?:beankeeper|beankeeper_cli|rusqlite|CliError|Db|Actor|PostTransactionParams|PostEntryParams|StoreAttachmentParams)\b"
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def public_declarations(source: str) -> list[str]:
    """Capture public type/function declaration headers, including multiline fn signatures."""
    declarations: list[str] = []
    lines = source.splitlines()
    i = 0
    while i < len(lines):
        stripped = lines[i].lstrip()
        if not stripped.startswith("pub "):
            i += 1
            continue
        parts = [stripped]
        if stripped.startswith("pub fn "):
            while "{" not in parts[-1] and ";" not in parts[-1] and i + 1 < len(lines):
                i += 1
                parts.append(lines[i].strip())
        declarations.append(" ".join(parts))
        i += 1
    return declarations


def main() -> int:
    required = (
        FACADE,
        TAURI_ADAPTER,
        LOCK,
        FOUNDATION_TOML,
        TAURI_TOML,
        MANIFEST,
        CONTRACT,
        BOOTSTRAP,
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
        fail("frozen SBC-0 facade reference drift")
    if manifest["dependency_baseline"]["cargo_lock_sha256"] != EXPECTED_LOCK:
        fail("frozen manifest lock hash drift")
    if manifest["dependency_baseline"]["tauri_core"] != "2.11.5":
        fail("frozen manifest Tauri core drift")
    if manifest["dependency_baseline"]["tauri_cli"] != "2.11.4":
        fail("frozen manifest Tauri CLI drift")
    if manifest["dependency_baseline"]["tauri_build"] != "2.6.3":
        fail("frozen manifest Tauri build drift")

    contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
    foundation = contract.get("foundation", {})
    if contract.get("gate") != "SBC-1B":
        fail("production facade contract gate is not SBC-1B")
    if contract.get("sbc1a_base_head") != EXPECTED_SBC1A_HEAD:
        fail("production facade contract has wrong SBC-1A base HEAD")
    if foundation.get("beankeeper_commit") != EXPECTED_BEANKEEPER:
        fail("production facade contract Beankeeper commit drift")
    if foundation.get("frozen_baseline_facade_sha256") != FROZEN_FACADE:
        fail("production facade contract lost frozen facade provenance")
    if foundation.get("cargo_lock_sha256") != EXPECTED_LOCK:
        fail("production facade contract lock hash drift")
    if contract.get("dependency_change") is not False or contract.get("cargo_update_permitted") is not False:
        fail("SBC-1B contract improperly permits dependency drift")

    production_facade = foundation.get("production_facade_sha256")
    if not isinstance(production_facade, str) or sha256(FACADE) != production_facade:
        fail("production Shark facade hash is not the approved SBC-1B contract hash")
    expected_tauri = foundation.get("production_tauri_adapter_sha256")
    if not isinstance(expected_tauri, str) or sha256(TAURI_ADAPTER) != expected_tauri:
        fail("Tauri adapter hash is not the approved SBC-1B contract hash")

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
    validation = facade.find("journal.post().map_err(validation_error)?")
    persistence = facade.find("db::post_transaction(self.db.conn(), &params)")
    if validation < 0 or persistence < 0 or validation >= persistence:
        fail("typed Beankeeper validation is not demonstrably before persistence")

    required_symbols = (
        "pub enum FoundationErrorCode",
        "pub struct FoundationError",
        "pub struct BooksId",
        "pub struct BooksMetadata",
        "pub struct MigrationMetadata",
        "pub struct PostTransactionRequest",
        "pub struct ImportSummary",
        "pub struct StatusChangeResult",
        "pub fn metadata(",
        "pub fn migration_metadata(",
        "pub fn post(",
        "fn map_cli_error(error: CliError)",
    )
    for symbol in required_symbols:
        if symbol not in facade:
            fail(f"required SBC-1B contract symbol missing: {symbol}")

    for declaration in public_declarations(facade):
        if UPSTREAM_PUBLIC_NAMES.search(declaration):
            fail(f"raw upstream type leaked through public facade declaration: {declaration}")

    # Static regression anchors for the behavioural tests that Windows will execute.
    for test_name in (
        "cli_error_mapping_is_stable_and_shark_owned",
        "metadata_is_shark_owned_and_versioned",
        "unbalanced_post_is_rejected_before_database_mutation",
    ):
        if f"fn {test_name}()" not in facade:
            fail(f"required SBC-1B unit test missing: {test_name}")

    print("PASS: Shark frozen-foundation + SBC-1B production-facade guard")
    print(f"  frozen_facade_sha256={FROZEN_FACADE}")
    print(f"  production_facade_sha256={production_facade}")
    print(f"  cargo_lock_sha256={EXPECTED_LOCK}")
    print(f"  beankeeper_commit={EXPECTED_BEANKEEPER}")
    print("  public_contract=Shark-owned DTOs/errors/version metadata")
    print("  invariant=typed Beankeeper validation before persistence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
