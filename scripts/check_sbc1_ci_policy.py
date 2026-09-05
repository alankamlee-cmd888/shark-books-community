#!/usr/bin/env python3
"""SBC-1E/1F/1G fail-closed policy guard layered on the frozen baseline guard."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "workspace" / "Cargo.lock"
WIN_CAP = ROOT / "workspace" / "shark-tauri-spike" / "capabilities" / "default.json"
IOS_CAP = ROOT / "workspace" / "shark-tauri-spike" / "capabilities" / "ios.json"
IOS_CONF = ROOT / "workspace" / "shark-tauri-spike" / "tauri.ios.conf.json"
IOS_CONTRACT = ROOT / "docs" / "SBC1E_IOS_SHELL_CONTRACT_2026-09-05.json"
BROWSER_CONTRACT = ROOT / "docs" / "SBC1F_BROWSER_ADAPTER_CONTRACT_2026-09-05.json"
CI_CONTRACT = ROOT / "docs" / "SBC1G_CI_REGRESSION_POLICY_2026-09-05.json"
ADAPTER = ROOT / "workspace" / "browser-adapter-contract" / "adapter.ts"
SMOKE = ROOT / "workspace" / "browser-adapter-contract" / "contract-smoke.ts"
TSCONFIG = ROOT / "workspace" / "browser-adapter-contract" / "tsconfig.json"
REGRESSION = ROOT / "workspace" / "shark-foundation" / "tests" / "sbc1_regression.rs"
OFX_FIXTURE = ROOT / "workspace" / "shark-foundation" / "tests" / "fixtures" / "sbc1g_duplicate_gbp.ofx"
WORKFLOW = ROOT / ".github" / "workflows" / "sbc-core.yml"

EXPECTED_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
COMMANDS = (
    "foundation_health",
    "production_encryption_required",
    "books_create",
    "books_open",
    "books_verify",
    "books_trial_balance",
)


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    required = (
        LOCK, WIN_CAP, IOS_CAP, IOS_CONF, IOS_CONTRACT, BROWSER_CONTRACT, CI_CONTRACT,
        ADAPTER, SMOKE, TSCONFIG, REGRESSION, OFX_FIXTURE, WORKFLOW,
    )
    for path in required:
        require(path.is_file(), f"missing SBC-1 policy artifact: {path.relative_to(ROOT)}")

    require(sha256(LOCK) == EXPECTED_LOCK, "Cargo.lock drifted from reviewed graph")

    windows = json.loads(WIN_CAP.read_text(encoding="utf-8"))
    ios = json.loads(IOS_CAP.read_text(encoding="utf-8"))
    ios_conf = json.loads(IOS_CONF.read_text(encoding="utf-8"))
    require(windows.get("identifier") == "windows-main", "Windows capability identifier drift")
    require(windows.get("platforms") == ["windows"], "Windows capability broadened")
    require(windows.get("permissions") == ["shark-shell"], "Windows permission set drift")
    require(ios.get("identifier") == "ios-main", "iOS capability identifier drift")
    require(ios.get("platforms") == ["iOS"], "iOS capability platform drift")
    require(ios.get("windows") == ["main"], "iOS capability window scope drift")
    require(ios.get("permissions") == ["shark-shell"], "iOS permission set drift")
    require(ios.get("local") is True, "iOS capability must remain local")
    require(
        ios_conf.get("app", {}).get("security", {}).get("capabilities") == ["ios-main"],
        "iOS platform override must select only ios-main",
    )

    ios_contract = json.loads(IOS_CONTRACT.read_text(encoding="utf-8"))
    require(ios_contract.get("gate") == "SBC-1E", "iOS contract gate drift")
    frozen = ios_contract.get("frozen_foundation", {})
    require(frozen.get("cargo_lock_sha256") == EXPECTED_LOCK, "iOS contract lock provenance drift")
    require(frozen.get("dependency_change_allowed") is False, "iOS contract permits dependency drift")
    security = ios_contract.get("security", {})
    require(security.get("production_encryption_required") is True, "iOS contract weakened encryption")
    require(security.get("webview_receives_key_or_passphrase") is False, "iOS contract exposes key surface")
    release = ios_contract.get("apple_release_scope", {})
    for forbidden in ("signing", "provisioning_profiles", "testflight", "app_store_distribution", "physical_device_installation"):
        require(release.get(forbidden) is False, f"SBC-1E unexpectedly enables {forbidden}")

    adapter = ADAPTER.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    tsconfig = json.loads(TSCONFIG.read_text(encoding="utf-8"))
    browser_contract = json.loads(BROWSER_CONTRACT.read_text(encoding="utf-8"))
    require(browser_contract.get("gate") == "SBC-1F", "browser contract gate drift")
    boundaries = browser_contract.get("boundaries", {})
    for must_be_false in (
        "browser_persistence_implemented",
        "opfs_implemented",
        "sqlite_wasm_implemented",
        "browser_encryption_parity_claimed",
        "accounting_rules_in_adapter",
        "raw_native_database_api_exposed",
    ):
        require(boundaries.get(must_be_false) is False, f"browser seam broadened: {must_be_false}")
    require(tsconfig.get("compilerOptions", {}).get("strict") is True, "browser seam TypeScript strict mode disabled")
    require(tsconfig.get("compilerOptions", {}).get("noEmit") is True, "browser seam must remain compile-only")
    for anchor in ("interface BooksApplicationAdapter", "class TauriBooksAdapter", "class BrowserBooksAdapter", "BrowserBooksBackend"):
        require(anchor in adapter, f"browser adapter anchor missing: {anchor}")
    for forbidden in ("rusqlite", "beankeeper_cli", "JournalEntry", "db::", "PostTransactionParams"):
        require(forbidden not in adapter and forbidden not in smoke, f"browser adapter leaks accounting/persistence implementation: {forbidden}")
    for command in COMMANDS:
        require(command in adapter, f"native adapter missing stable shell command: {command}")

    regression = REGRESSION.read_text(encoding="utf-8")
    for test_name in (
        "gbp_encryption_and_hard_reopen_regression",
        "duplicate_ofx_is_idempotently_skipped",
        "append_only_reversal_and_audit_regression",
        "document_attachment_round_trip_regression",
    ):
        require(test_name in regression, f"mandatory SBC-1G regression missing: {test_name}")
    fixture = OFX_FIXTURE.read_text(encoding="utf-8")
    for anchor in ("<CURDEF>GBP</CURDEF>", "<FITID>SBC1G-DUP-001</FITID>", "<TRNAMT>-12.34</TRNAMT>"):
        require(anchor in fixture, f"OFX regression fixture drift: {anchor}")

    ci_contract = json.loads(CI_CONTRACT.read_text(encoding="utf-8"))
    require(ci_contract.get("gate") == "SBC-1G", "CI contract gate drift")
    require(ci_contract.get("dependency_policy", {}).get("silent_dependency_drift_allowed") is False, "CI policy permits silent dependency drift")
    require(ci_contract.get("dependency_policy", {}).get("sbom_and_licence_regeneration_required_on_approved_change") is True, "CI policy lost SBOM/licence regeneration rule")
    apple = ci_contract.get("apple_ci", {})
    require(apple.get("cadence") == "weekly", "Apple cadence drift")
    require(apple.get("secrets") == "none", "Apple scheduled proof must be zero-secret")
    require(apple.get("monthly_free_m2_minutes") == 500, "Apple free-minute budget drift")
    require(apple.get("normal_target_max_minutes") == 400, "Apple normal budget target drift")
    require(apple.get("reserve_min_minutes") == 100, "Apple reserve policy drift")
    require(apple.get("public_forks_can_trigger_owner_codemagic") is False, "public forks must not consume owner Codemagic")

    workflow = WORKFLOW.read_text(encoding="utf-8")
    for required_anchor in (
        "contents: read",
        "python scripts/check_frozen_baseline.py",
        "python scripts/check_sbc1_ci_policy.py",
        "typescript@5.9.3",
        "cargo test -p shark-foundation --locked",
        "cargo test -p shark-tauri-spike --locked",
        "cargo check --workspace --locked",
        "cargo build -p shark-tauri-spike --locked",
        "rustup toolchain install 1.98.1",
        "python scripts/bootstrap_beankeeper.py",
    ):
        require(required_anchor in workflow, f"CI workflow missing required control: {required_anchor}")
    for forbidden in ("pull_request_target:", "secrets.", "cargo update"):
        require(forbidden not in workflow, f"CI workflow contains forbidden surface: {forbidden}")

    print("PASS: SBC-1E/1F/1G policy guard")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
