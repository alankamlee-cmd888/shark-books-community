#!/usr/bin/env python3
"""Fail-closed SBC-7B1 Batch B Supporting Data + Read Models gate."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys

BASE = "6f85734d0e9e11089a1a47b7850a306a4ed7ba87"
RUST_TOOLCHAIN = "1.98.1"
NATIVE_LOCK_SHA256 = "10fe1431f26cef2d427e76658690544027e955845faabfbbdaebf2a2ad17e115"

ALLOWED_PATHS = {
    "ci/run_sbc7b1_data_readmodels_codemagic.sh",
    "ci/run_sbc7b1_data_readmodels_windows.ps1",
    "codemagic.yaml",
    "docs/SBC7B1_DATA_READMODELS_IMPLEMENTATION_DESIGN_2026-09-14.md",
    "scripts/check_sbc7b1_data_readmodels.py",
    "workspace/shark-foundation/src/bank_application/mod.rs",
    "workspace/shark-foundation/src/bank_application/tests.rs",
    "workspace/shark-foundation/src/contact_application.rs",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/owner_documents_ocr.rs",
    "workspace/shark-tauri-spike/src/owner_supporting_data.rs",
}

NEW_COMMANDS = [
    "owner_contacts_list",
    "owner_contacts_save",
    "owner_settings_books_info",
    "owner_settings_storage_root_select",
    "owner_report_summary",
]

PORTABLE_OCR_TESTS = [
    "ocr_native::tests::webview_request_accepts_only_opaque_ids",
    "ocr_native::tests::registry_is_native_only_and_resolves_by_opaque_document_id",
    "ocr_native::tests::completed_sidecar_maps_to_b2_factual_shape_without_inferred_confidence",
    "ocr_native::tests::exact_one_json_object_and_schema_are_enforced",
    "ocr_native::tests::completed_hash_or_length_drift_fails_integrity_closed",
    "ocr_native::tests::sidecar_failure_is_typed_and_nonzero_is_required",
    "ocr_native::tests::invalid_factual_values_fail_closed",
]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(repo: Path, args: list[str], *, env: dict[str, str] | None = None) -> None:
    proc = subprocess.run(
        args,
        cwd=repo,
        env=env,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        raise RuntimeError(f"command failed ({proc.returncode}): {' '.join(args)}")


def git(repo: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"git {' '.join(args)} failed")
    return proc.stdout


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)
    print(f"[PASS] {message}")


def text(repo: Path, relative: str) -> str:
    return (repo / relative).read_text(encoding="utf-8")


def static_gate(repo: Path) -> dict[str, object]:
    head = git(repo, "rev-parse", "HEAD").strip()
    run(repo, ["git", "merge-base", "--is-ancestor", BASE, head])
    require(True, "post-Batch-A protected main is an ancestor of candidate HEAD")

    changed = {
        line.strip()
        for line in git(repo, "diff", "--name-only", f"{BASE}..HEAD").splitlines()
        if line.strip()
    }
    require(
        changed == ALLOWED_PATHS,
        f"candidate changed paths equal exact Data + Read Models allow-list ({len(ALLOWED_PATHS)} paths)",
    )

    for protected in [
        "product/shark-books-core",
        "workspace/Cargo.lock",
        "workspace/shark-foundation/Cargo.toml",
        "workspace/shark-tauri-spike/Cargo.toml",
        "workspace/shark-tauri-spike/src/ocr_native.rs",
        "workspace/shark-tauri-spike/src/owner_app.rs",
        "workspace/shark-tauri-spike/src/owner_bank_review.rs",
        "workspace/shark-tauri-spike/src/owner_bank_mutation.rs",
        "workspace/shark-tauri-spike/src/owner_mutation_audit.rs",
        "workspace/shark-tauri-spike/tauri.conf.json",
        "workspace/shark-tauri-spike/tauri.ios.conf.json",
        "workspace/shark-tauri-spike/capabilities",
        "workspace/dist",
        "scripts/bootstrap_beankeeper.py",
        "upstream",
    ]:
        diff = git(repo, "diff", "--name-only", f"{BASE}..HEAD", "--", protected).strip()
        require(not diff, f"protected path unchanged: {protected}")

    lock_bytes = (repo / "workspace" / "Cargo.lock").read_bytes()
    require(
        sha256_bytes(lock_bytes) == NATIVE_LOCK_SHA256,
        "native Cargo.lock remains byte-identical to Batch A final lock",
    )

    contract = text(repo, "docs/SBC7B1_DATA_READMODELS_BATCH_CONTRACT_2026-09-14.md")
    design = text(repo, "docs/SBC7B1_DATA_READMODELS_IMPLEMENTATION_DESIGN_2026-09-14.md")
    for command in NEW_COMMANDS:
        require(command in contract, f"frozen Batch B contract contains command: {command}")
        require(command in design, f"implementation design contains command: {command}")
    require("no new dependency is authorised" in contract.lower(), "frozen contract authorises no new dependency")
    require(
        "unsupportedPlatform" in design and "iOS/Android" in design,
        "DR-BB-10 design records fail-closed mobile folder-selection boundary",
    )
    require(
        "file picking must not be substituted for folder picking" in design,
        "DR-BB-10 design forbids file-picker substitution on mobile",
    )

    foundation = text(repo, "workspace/shark-foundation/src/lib.rs")
    require(
        "pub const SHARK_APPLICATION_SCHEMA_VERSION: u32 = 5;" in foundation,
        "Shark application schema version is exactly 5",
    )
    require(
        "pub const BEANKEEPER_DATABASE_SCHEMA_VERSION: i64 = 8;" in foundation,
        "pinned Beankeeper database schema remains exactly 8",
    )
    require("mod contact_application;" in foundation, "foundation owns minimal contact persistence module")
    for anchor in ["ContactPersistOutcome", "ContactView", "ContactWrite"]:
        require(anchor in foundation, f"typed foundation contact export present: {anchor}")
    require("checked_add" in foundation and "trial balance debit total overflow" in foundation,
            "foundation trial-balance totals fail closed on overflow")
    open_pos = foundation.index("pub fn open_encrypted")
    require(
        foundation.index("backup_hook.backup_before_open", open_pos)
        < foundation.index("bank_application::ensure_application_schema(&db)?;", open_pos),
        "pre-open encrypted backup remains before Shark application-schema migration",
    )

    app_mod = text(repo, "workspace/shark-foundation/src/bank_application/mod.rs")
    for anchor in [
        "BANK_APPLICATION_SCHEMA_VERSION: u32 = 5",
        "SAVEPOINT shark_application_schema_v5",
        "CREATE TABLE IF NOT EXISTS shark_contact",
        "CHECK(kind IN ('customer','supplier'))",
        "PRIMARY KEY(company_slug, contact_id)",
        "VALUES(1, 5)",
    ]:
        require(anchor in app_mod, f"schema-v5 anchor present: {anchor}")
    for inherited in [
        "shark_bank_activity",
        "shark_document",
        "shark_receipt_bank_decision",
        "shark_owner_correction",
    ]:
        require(f"CREATE TABLE IF NOT EXISTS {inherited}" in app_mod, f"schema v5 retains {inherited}")

    bank_tests = text(repo, "workspace/shark-foundation/src/bank_application/tests.rs")
    require(
        "application_schema_v5_is_separate_from_beankeeper_schema_8" in bank_tests
        and "assert_eq!(SHARK_APPLICATION_SCHEMA_VERSION, 5);" in bank_tests,
        "bank schema regression is explicitly advanced to v5",
    )
    require("application_schema_v4_migrates_to_v5_without_dropping_batch_a_data" in bank_tests,
            "v4 to v5 migration preservation regression is present")

    contacts = text(repo, "workspace/shark-foundation/src/contact_application.rs")
    for anchor in [
        "pub struct ContactWrite",
        "pub struct ContactView",
        "pub enum ContactPersistOutcome",
        "pub fn save_contact",
        "pub fn contacts",
        "AlreadyCurrent",
        "contact id already exists with a different immutable kind",
        "contacts_create_update_list_are_idempotent_and_non_posting",
        "contact_kind_conflict_and_invalid_values_fail_closed",
    ]:
        require(anchor in contacts, f"contact persistence anchor present: {anchor}")
    runtime_contacts = contacts.split("#[cfg(test)]", 1)[0].lower()
    for forbidden in ["std::process", "command::new", "reqwest::", "ureq::", "http://", "https://"]:
        require(forbidden not in runtime_contacts, f"contact persistence excludes forbidden primitive: {forbidden}")

    documents = text(repo, "workspace/shark-tauri-spike/src/owner_documents_ocr.rs")
    for anchor in [
        "register_session_root",
        "storage-root-session-",
        "registered_root_count",
    ]:
        require(anchor in documents, f"native document-root registry Batch B anchor present: {anchor}")

    owner = text(repo, "workspace/shark-tauri-spike/src/owner_supporting_data.rs")
    for anchor in [
        "core::Customer::new",
        "core::Supplier::new",
        "owner_contacts_list",
        "owner_contacts_save",
        "owner_settings_books_info",
        "owner_settings_storage_root_select",
        "owner_report_summary",
        "blocking_pick_folder",
        "unsupportedPlatform",
        "storage_root_mobile_unsupported_error",
        "DeviceSession",
        "report_from_trial_balance",
        "contact_request_rejects_raw_authority_and_uses_core_validation",
        "books_info_serialization_contains_no_path_key_or_database_authority",
        "storage_root_registration_is_opaque_session_scoped_and_cancellation_safe",
        "mobile_storage_root_selection_fails_closed_without_raw_authority",
        "report_summary_uses_factual_account_signs_and_optional_cash_balances",
        "report_summary_fails_closed_on_i64_result_overflow_and_ambiguous_bank",
    ]:
        require(anchor in owner, f"owner supporting-data bridge anchor present: {anchor}")
    desktop_guard = '#[cfg(not(any(target_os = "ios", target_os = "android")))]\nfn select_storage_root('
    mobile_guard = '#[cfg(any(target_os = "ios", target_os = "android"))]\nfn select_storage_root('
    require(desktop_guard in owner, "storage-root picker implementation is explicitly non-mobile")
    require(mobile_guard in owner, "mobile storage-root command has an explicit fail-closed implementation")
    desktop_pos = owner.index(desktop_guard)
    picker_pos = owner.index("blocking_pick_folder")
    mobile_pos = owner.index(mobile_guard)
    require(
        desktop_pos < picker_pos < mobile_pos and owner.count("blocking_pick_folder") == 1,
        "blocking folder picker is confined to the non-mobile target branch",
    )
    require(
        'code: "unsupportedPlatform"' in owner
        and '"folder selection is not supported on this mobile platform"' in owner,
        "mobile storage-root selection fails closed with an owner-safe unsupportedPlatform error",
    )
    runtime_owner = owner.split("#[cfg(test)]", 1)[0].lower()
    for marker in [
        "use beankeeper", "use rusqlite", "tauri_plugin_shell", "reqwest::", "ureq::",
        "std::net", "tokio::net", "http://", "https://", "database_path:", "db_path:",
        "passphrase:", "encryption_key:", "shellcommand:", "sourcepath:", "destinationpath:",
    ]:
        require(marker not in runtime_owner, f"owner supporting-data bridge excludes forbidden/raw surface: {marker}")
    require("currency: \"GBP\"" in owner, "report currency is factual GBP")
    require("credit_total" in owner and "debit_total" in owner, "report derives from authoritative debit/credit totals")

    codemagic = text(repo, "codemagic.yaml")
    require(
        'chmod +x "$CM_BUILD_DIR/ci/run_sbc7b1_data_readmodels_codemagic.sh"' not in codemagic,
        "Data + Read Models Codemagic workflow does not self-dirty the tracked runner",
    )

    shell = text(repo, "workspace/shark-tauri-spike/src/lib.rs")
    build_rs = text(repo, "workspace/shark-tauri-spike/build.rs")
    permission = text(repo, "workspace/shark-tauri-spike/permissions/shark-shell.toml")
    frontend = "\n".join([
        text(repo, "workspace/dist/index.html"),
        text(repo, "workspace/dist/app.js"),
    ])
    require("mod owner_supporting_data;" in shell, "owner supporting-data module is wired")
    for command in NEW_COMMANDS:
        require(f"owner_supporting_data::{command}" in shell, f"Rust invoke handler wires command: {command}")
        require(command in build_rs, f"Tauri manifest contains command: {command}")
        require(command in permission, f"exact Shark permission contains command: {command}")
        require(command not in frontend, f"frontend remains intentionally unwired from: {command}")

    status = git(repo, "status", "--porcelain").strip()
    require(not status, "repository clean before runtime gate")

    return {
        "head": head,
        "changed_paths": sorted(changed),
        "native_lock_sha256": sha256_bytes(lock_bytes),
    }


def runtime_gate(repo: Path) -> None:
    env = os.environ.copy()
    env.setdefault(
        "SHARK_SBC1D_PROOF_KEY",
        "SBC7B1-Data-ReadModels-Proof-Key-Only-Do-Not-Ship",
    )
    proof_root = Path.home() / "sbc7b1-data-readmodels-proof-books"
    proof_root.mkdir(parents=True, exist_ok=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(proof_root)
    env["RUST_TEST_THREADS"] = "1"

    target = Path(env.get("CARGO_TARGET_DIR", "")) if env.get("CARGO_TARGET_DIR") else Path.home() / "sbc7b1-data-readmodels-proof-target"
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    target.mkdir(parents=True, exist_ok=True)
    env["CARGO_TARGET_DIR"] = str(target)

    try:
        run(repo, [sys.executable, "-B", str(repo / "scripts" / "bootstrap_beankeeper.py")], env=env)
        run(repo, ["rustup", "run", RUST_TOOLCHAIN, "rustc", "--version"], env=env)
        run(repo, [
            "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
            "--manifest-path", str(repo / "product" / "shark-books-core" / "Cargo.toml"),
            "--locked", "--jobs", "1",
        ], env=env)
        run(repo, [
            "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
            "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
            "-p", "shark-foundation", "--locked", "--jobs", "1", "--", "--test-threads=1",
        ], env=env)

        for test_name in PORTABLE_OCR_TESTS:
            run(repo, [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                test_name, "--", "--exact", "--test-threads=1",
            ], env=env)

        for test_filter in [
            "owner_app::tests",
            "owner_bank_review::tests",
            "owner_bank_mutation::tests",
            "owner_documents_ocr::tests",
            "owner_mutation_audit::tests",
            "owner_supporting_data::tests",
        ]:
            run(repo, [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                test_filter, "--", "--test-threads=1",
            ], env=env)

        for exact_test in [
            "tests::encrypted_command_cycle_uses_only_shark_facade",
            "tests::frontend_contract_is_keyless_and_uses_only_bounded_commands",
            "tests::tauri_acl_and_csp_are_bounded",
        ]:
            run(repo, [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                exact_test, "--", "--exact", "--test-threads=1",
            ], env=env)

        run(repo, [
            "rustup", "run", RUST_TOOLCHAIN, "cargo", "check",
            "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
            "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
        ], env=env)

        lock_bytes = (repo / "workspace" / "Cargo.lock").read_bytes()
        require(sha256_bytes(lock_bytes) == NATIVE_LOCK_SHA256, "native Cargo.lock unchanged after runtime gate")
        require(not git(repo, "status", "--porcelain").strip(), "repository clean after runtime gate")
    finally:
        shutil.rmtree(proof_root, ignore_errors=True)
        shutil.rmtree(target, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    try:
        result = static_gate(repo)
        if not args.static_only:
            runtime_gate(repo)
        print(f"[PASS] SBC-7B1 Data + Read Models gate complete at {result['head']}")
        return 0
    except Exception as error:
        print(f"[FAIL] {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
