#!/usr/bin/env python3
"""Shark frozen foundation + bounded current native-shell guard.

This guard keeps the accounting/dependency/frontend foundation byte-frozen while
allowing explicitly reviewed native-shell evolution such as SBC-6B B3C. Feature
gates remain responsible for exact changed-path allowlists and feature-specific
behavioural proofs.
"""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FACADE = ROOT / "workspace" / "shark-foundation" / "src" / "lib.rs"
TAURI = ROOT / "workspace" / "shark-tauri-spike" / "src" / "lib.rs"
OCR_NATIVE = ROOT / "workspace" / "shark-tauri-spike" / "src" / "ocr_native.rs"
BUILD_RS = ROOT / "workspace" / "shark-tauri-spike" / "build.rs"
GITIGNORE = ROOT / ".gitignore"
LOCK = ROOT / "workspace" / "Cargo.lock"
ROOT_TOML = ROOT / "workspace" / "Cargo.toml"
FOUNDATION_TOML = ROOT / "workspace" / "shark-foundation" / "Cargo.toml"
TAURI_TOML = ROOT / "workspace" / "shark-tauri-spike" / "Cargo.toml"
TAURI_CONF = ROOT / "workspace" / "shark-tauri-spike" / "tauri.conf.json"
CAPABILITY = ROOT / "workspace" / "shark-tauri-spike" / "capabilities" / "default.json"
PERMISSION = ROOT / "workspace" / "shark-tauri-spike" / "permissions" / "shark-shell.toml"
INDEX = ROOT / "workspace" / "dist" / "index.html"
APP_JS = ROOT / "workspace" / "dist" / "app.js"
STYLE = ROOT / "workspace" / "dist" / "style.css"
WINDOWS_ICON = ROOT / "workspace" / "shark-tauri-spike" / "icons" / "icon.ico"
MANIFEST = ROOT / "docs" / "frozen" / "41_SBC0E_FROZEN_FOUNDATION_MANIFEST_2026-09-04.json"
SBC1B_CONTRACT = ROOT / "docs" / "SBC1B_PRODUCTION_FACADE_CONTRACT_2026-09-04.json"
SBC1C_CONTRACT = ROOT / "docs" / "SBC1C_ENCRYPTED_LIFECYCLE_CONTRACT_2026-09-04.json"
SBC1D_CONTRACT = ROOT / "docs" / "SBC1D_WINDOWS_SHELL_CONTRACT_2026-09-04.json"
BOOTSTRAP = ROOT / "scripts" / "bootstrap_beankeeper.py"

FROZEN_FACADE = "2c679b0fd5e146e2f82050a32e047c48fa9aa02d386964f8b307c2cae68fb87b"
SBC1B_FACADE = "422a39c1735a347d9472ff61f45cea992765b31b216ac4531a78fd424bde1503"
SBC1C_FACADE = "6274cffc89ccb2fcea4d489e76375c9c5be1e3c8cc2fbd4c4e56b37bdef0ef46"
EXPECTED_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_BEANKEEPER = "d573db5e61089b0922f95c991732394d08e3cf92"
EXPECTED_SBC1C_HEAD = "1cd097dd78e8022a9a0aab410147cafd12f16245"
EXPECTED_ROOT_TOML = "6df618efdf2592e238cf09ad541b20a81aebbe0ab32fed1bde56273423bf2688"
EXPECTED_FOUNDATION_TOML = "7e56f04930490c104b5b558f4201e3ed86b7dd048d9a8afba237eab64de19749"
EXPECTED_TAURI_TOML = "6785bf54bb14235cd260ef7334bd56745001ae707baea49bd2296cff26cff918"
EXPECTED_GITIGNORE = "e7d27710a916f2c9866798ddbd9d28c39b00eabed0f2ef5fc671139b8280da5d"
EXPECTED_TAURI_CONF = "b8e7928d088fc33c36a64f5b311667f5015bc5ddc4414f773387bfc88b2ee606"
EXPECTED_CAPABILITY = "ae24947450d99a8a80f48e8e944ab981875f85f66ac61da426e920d2a534afd8"
EXPECTED_INDEX = "c795ed045afd50a8d027ef591d5463e10124d11bf9eb7907bde1cd08c323dd79"
EXPECTED_APP_JS = "3e734c345363ebe2b6f4bad5353419fc9a1706e4fb61625046d28f8973ec53e2"
EXPECTED_STYLE = "d24fb9a4de78706e12549772cff32fd87ceb8306d8c814ebcffe0c6dbc8a322f"
EXPECTED_ICON = "eaa03cca47eb57dc4c159667844fd1f9ccfa1e71ccbe6fd11defbff08b80961d"
EXPECTED_CONTRACT = "b7d28adbb555fa803f1e33b0c7a171e03bb89d689ea56854bce23ad993f968a1"

BASE_COMMANDS = (
    "foundation_health",
    "production_encryption_required",
    "books_create",
    "books_open",
    "books_verify",
    "books_trial_balance",
)
CURRENT_COMMANDS = BASE_COMMANDS + ("ocr_extract_receipt",)
UPSTREAM_PUBLIC_NAMES = re.compile(
    r"\b(?:beankeeper|beankeeper_cli|rusqlite|CliError|Db|Actor|PostTransactionParams|PostEntryParams|StoreAttachmentParams|SecretString)\b"
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def require_hash(path: Path, expected: str, label: str) -> None:
    if sha256(path) != expected:
        fail(f"{label} hash drift")


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


def require_current_native_shell() -> None:
    for path in (TAURI, OCR_NATIVE, BUILD_RS, PERMISSION):
        if not path.is_file():
            fail(f"current native-shell file missing: {path.relative_to(ROOT)}")

    build_rs = BUILD_RS.read_text(encoding="utf-8")
    for anchor in (
        'const AUTOGENERATED_PERMISSIONS_DIR: &str = "permissions/autogenerated"',
        "fs::remove_dir_all(autogenerated)",
        "AppManifest::new().commands",
    ):
        if anchor not in build_rs:
            fail(f"Tauri build manifest missing required control: {anchor}")
    for command in CURRENT_COMMANDS:
        if f'"{command}"' not in build_rs:
            fail(f"Tauri AppManifest command list missing: {command}")

    tauri_source = TAURI.read_text(encoding="utf-8")
    for anchor in (
        "struct ProofEnvironmentKeyProvider",
        'const PROOF_KEY_ENV: &str = "SHARK_SBC1D_PROOF_KEY"',
        'const PROOF_BOOKS_DIR_ENV: &str = "SHARK_SBC1D_BOOKS_DIR"',
        'file_name.strip_suffix(".sqlite")',
        "Books::create_encrypted(",
        "Books::open_encrypted(",
        "books.verify()?",
        "trial_balance()",
        "mod ocr_native;",
        ".manage(ocr_native::NativeOcrRegistry::default())",
        "ocr_native::ocr_extract_receipt",
        "tauri::generate_handler![",
    ):
        if anchor not in tauri_source:
            fail(f"required bounded native-shell anchor missing: {anchor}")
    for command in BASE_COMMANDS:
        if f"fn {command}(" not in tauri_source:
            fail(f"required historical bounded command missing: {command}")

    runtime_tauri = tauri_source.split("#[cfg(test)]", 1)[0]
    for forbidden in (
        "rusqlite",
        "beankeeper_cli",
        "beankeeper::",
        "SecretString",
        "PostTransactionParams",
        "db_path:",
        "dbPath",
        "passphrase",
    ):
        if forbidden in runtime_tauri:
            fail(f"forbidden raw/secret runtime shell surface found: {forbidden}")

    permission = PERMISSION.read_text(encoding="utf-8")
    if 'identifier = "shark-shell"' not in permission:
        fail("bounded shark-shell permission identifier missing")
    for command in CURRENT_COMMANDS:
        if f'"{command}"' not in permission:
            fail(f"permission does not explicitly allow {command}")
    for forbidden in (
        "post_transaction",
        "create_account",
        "import_ofx",
        "set_reconciled",
        "confirm_match",
        "finalize_reconciliation",
    ):
        if forbidden in permission:
            fail(f"mutating/raw accounting command unexpectedly exposed: {forbidden}")

    ocr = OCR_NATIVE.read_text(encoding="utf-8")
    for anchor in (
        'const SIDECAR_SCHEMA: &str = "sbc6b.single_document.v1"',
        "pub(crate) struct NativeOcrRegistry",
        "pub(crate) struct OcrReceiptCommandRequest",
        "request_id: String",
        "document_id: String",
        "Command::new(sidecar)",
        '.arg("--expected-sha256")',
        '.arg("--expected-byte-len")',
        "SIDECAR_TIMEOUT",
        "MAX_STDOUT_BYTES",
        "MAX_STDERR_BYTES",
        "child.kill()",
        "parse_exact_json",
        "ShellOcrFailureKind::Timeout",
        "ShellOcrFailureKind::MalformedOutput",
        "ShellOcrFailureKind::ResourceLimit",
        "ShellOcrUnavailableReason::UnsupportedPlatform",
    ):
        if anchor not in ocr:
            fail(f"current OCR native boundary missing: {anchor}")
    for forbidden in (
        "std::net",
        "reqwest",
        "ureq",
        "tokio::net",
        'Command::new("cmd',
        'Command::new("powershell',
        'Command::new("sh',
        "http://",
        "https://",
        "confirm_match",
        "finalize_reconciliation",
        "PostingPlan",
        "ExpenseCategory",
        "IncomeCategory",
    ):
        if forbidden.lower() in ocr.lower():
            fail(f"current OCR native boundary contains forbidden marker: {forbidden}")

    app_js = APP_JS.read_text(encoding="utf-8")
    index = INDEX.read_text(encoding="utf-8")
    style = STYLE.read_text(encoding="utf-8")
    for command in BASE_COMMANDS:
        if command not in app_js:
            fail(f"frontend lost historical bounded command: {command}")
    if "ocr_extract_receipt" in app_js:
        fail("SBC-7 frontend OCR wiring must not appear during B3C")
    for forbidden in (
        "SHARK_SBC1D_PROOF_KEY",
        "passphrase",
        "dbPath",
        "databasePath",
        "inputPath",
        "modelPath",
        "executablePath",
        "shellCommand",
        "http://",
        "https://",
        "fetch(",
        "eval(",
    ):
        if forbidden in index or forbidden in app_js or forbidden in style:
            fail(f"frontend contains forbidden external/secret/raw-path surface: {forbidden}")


def main() -> int:
    required = (
        FACADE, TAURI, OCR_NATIVE, BUILD_RS, GITIGNORE, LOCK, ROOT_TOML,
        FOUNDATION_TOML, TAURI_TOML, TAURI_CONF, CAPABILITY, PERMISSION,
        INDEX, APP_JS, STYLE, WINDOWS_ICON, MANIFEST, SBC1B_CONTRACT,
        SBC1C_CONTRACT, SBC1D_CONTRACT, BOOTSTRAP,
    )
    for path in required:
        if not path.is_file():
            fail(f"missing required baseline/contract file: {path.relative_to(ROOT)}")

    for path, expected, label in (
        (LOCK, EXPECTED_LOCK, "Cargo.lock"),
        (ROOT_TOML, EXPECTED_ROOT_TOML, "workspace Cargo.toml"),
        (FOUNDATION_TOML, EXPECTED_FOUNDATION_TOML, "foundation Cargo.toml"),
        (TAURI_TOML, EXPECTED_TAURI_TOML, "Tauri Cargo.toml"),
        (GITIGNORE, EXPECTED_GITIGNORE, ".gitignore"),
        (FACADE, SBC1C_FACADE, "approved SBC-1C production facade"),
        (TAURI_CONF, EXPECTED_TAURI_CONF, "Tauri config"),
        (CAPABILITY, EXPECTED_CAPABILITY, "Tauri capability"),
        (INDEX, EXPECTED_INDEX, "frontend index"),
        (APP_JS, EXPECTED_APP_JS, "frontend app.js"),
        (STYLE, EXPECTED_STYLE, "frontend style.css"),
        (WINDOWS_ICON, EXPECTED_ICON, "Windows icon"),
        (SBC1D_CONTRACT, EXPECTED_CONTRACT, "SBC-1D historical contract"),
    ):
        require_hash(path, expected, label)

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
        fail("SBC-1B facade provenance drift")
    sbc1c = json.loads(SBC1C_CONTRACT.read_text(encoding="utf-8"))
    if sbc1c.get("foundation", {}).get("sbc1c_production_facade_sha256") != SBC1C_FACADE:
        fail("SBC-1C facade provenance drift")
    if sbc1c.get("dependency_change") is not False:
        fail("SBC-1C dependency-change provenance drift")
    sbc1d = json.loads(SBC1D_CONTRACT.read_text(encoding="utf-8"))
    if sbc1d.get("gate") != "SBC-1D" or sbc1d.get("sbc1c_base_head") != EXPECTED_SBC1C_HEAD:
        fail("SBC-1D historical contract identity drift")
    if sbc1d.get("dependency_change") is not False or sbc1d.get("cargo_update_permitted") is not False:
        fail("SBC-1D historical contract improperly permits dependency drift")

    foundation_toml = FOUNDATION_TOML.read_text(encoding="utf-8")
    if 'beankeeper = { path = "../../upstream/beankeeper/beankeeper" }' not in foundation_toml:
        fail("Beankeeper path dependency drift")
    if 'beankeeper-cli = { path = "../../upstream/beankeeper/beankeeper-cli" }' not in foundation_toml:
        fail("Beankeeper CLI path dependency drift")
    tauri_toml = TAURI_TOML.read_text(encoding="utf-8")
    for expected in ('tauri = { version = "=2.11.5"', 'tauri-build = { version = "=2.6.3"'):
        if expected not in tauri_toml:
            fail(f"Tauri manifest pin drift: {expected}")
    if EXPECTED_BEANKEEPER not in BOOTSTRAP.read_text(encoding="utf-8"):
        fail("bootstrap script does not pin frozen Beankeeper commit")

    facade = FACADE.read_text(encoding="utf-8")
    validation = facade.find("journal.post().map_err(validation_error)?")
    persistence = facade.find("db::post_transaction(self.db.conn(), &params)")
    if validation < 0 or persistence < 0 or validation >= persistence:
        fail("typed Beankeeper validation is not demonstrably before persistence")
    if "pub trait SecureKeyProvider: Send + Sync" not in facade:
        fail("SBC-1C secure-key provider boundary missing")
    if "pub fn create_encrypted(" not in facade or "pub fn open_encrypted(" not in facade:
        fail("SBC-1C encrypted lifecycle boundary missing")
    for declaration in public_declarations(facade):
        if UPSTREAM_PUBLIC_NAMES.search(declaration):
            fail(f"raw upstream/secret type leaked through public facade declaration: {declaration}")

    config = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    if config.get("productName") != "Shark Books Community":
        fail("Tauri product name drift")
    if config.get("identifier") != "uk.co.mtdshark.sharkbookscommunity":
        fail("Tauri identifier drift")
    app = config.get("app", {})
    if app.get("withGlobalTauri") is not True:
        fail("withGlobalTauri boundary drift")
    if not isinstance(app.get("security", {}).get("csp"), dict):
        fail("Tauri CSP must remain enabled and structured")
    if app.get("security", {}).get("capabilities") != ["windows-main"]:
        fail("Tauri capability activation drift")
    if config.get("bundle", {}).get("active") is not False:
        fail("engineering shell unexpectedly became a release bundle")

    capability = json.loads(CAPABILITY.read_text(encoding="utf-8"))
    if capability.get("local") is not True:
        fail("capability local-origin boundary must be explicit")
    if capability.get("windows") != ["main"] or capability.get("platforms") != ["windows"]:
        fail("capability widened beyond Windows main window")
    if capability.get("permissions") != ["shark-shell"]:
        fail("capability permissions widened")
    if "core:default" in CAPABILITY.read_text(encoding="utf-8"):
        fail("broad core:default capability must not be present")

    require_current_native_shell()

    print("PASS: Shark frozen accounting/dependency foundation + bounded current native shell guard")
    print(f"  sbc1c_facade_sha256={SBC1C_FACADE}")
    print(f"  cargo_lock_sha256={EXPECTED_LOCK}")
    print(f"  beankeeper_commit={EXPECTED_BEANKEEPER}")
    print("  current_commands=7 bounded application commands")
    print("  frontend_ocr_surface=absent")
    print("  arbitrary_db_path_surface=absent")
    print("  generic_shell_surface=absent")
    print("  invariant=typed Beankeeper validation before persistence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
