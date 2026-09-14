#!/usr/bin/env python3
"""Fail-closed SBC-7B1 Batch A Mutation + Audit owner-bridge gate."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys

BASE = "8f55eaf67b311a7c2d69ebea86ae702e4934a56c"
RUST_TOOLCHAIN = "1.98.1"
NATIVE_LOCK_SHA256 = "10fe1431f26cef2d427e76658690544027e955845faabfbbdaebf2a2ad17e115"

ALLOWED_PATHS = {
    "ci/run_sbc7b1_mutation_audit_codemagic.sh",
    "ci/run_sbc7b1_mutation_audit_windows.ps1",
    "codemagic.yaml",
    "docs/SBC7B1_MUTATION_AUDIT_IMPLEMENTATION_DESIGN_2026-09-14.md",
    "scripts/check_sbc7b1_mutation_audit.py",
    "workspace/shark-foundation/src/bank_application/mod.rs",
    "workspace/shark-foundation/src/bank_application/tests.rs",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-foundation/src/mutation_audit_application.rs",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/owner_mutation_audit.rs",
}

NEW_COMMANDS = [
    "owner_receipt_suggest_bank",
    "owner_receipt_confirm_bank",
    "owner_receipt_reject_bank",
    "owner_correction_preview",
    "owner_correction_confirm",
    "owner_correction_history",
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
    require(True, "planning-freeze protected main is an ancestor of candidate HEAD")

    changed = {
        line.strip()
        for line in git(repo, "diff", "--name-only", f"{BASE}..HEAD").splitlines()
        if line.strip()
    }
    require(
        changed == ALLOWED_PATHS,
        f"candidate changed paths equal exact Mutation + Audit allow-list ({len(ALLOWED_PATHS)} paths)",
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
        "workspace/shark-tauri-spike/src/owner_documents_ocr.rs",
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
        "native Cargo.lock remains byte-identical to reviewed Slice 3A lock",
    )

    contract = text(repo, "docs/SBC7B1_MUTATION_AUDIT_BATCH_CONTRACT_2026-09-14.md")
    design = text(repo, "docs/SBC7B1_MUTATION_AUDIT_IMPLEMENTATION_DESIGN_2026-09-14.md")
    for command in NEW_COMMANDS:
        require(command in contract, f"frozen Batch A contract contains command: {command}")
        require(command in design, f"implementation design contains command: {command}")
    require("no dependency addition is authorised" in contract.lower(), "frozen contract authorises no dependency addition")

    foundation = text(repo, "workspace/shark-foundation/src/lib.rs")
    require(
        "pub const SHARK_APPLICATION_SCHEMA_VERSION: u32 = 4;" in foundation,
        "Shark application schema version is exactly 4",
    )
    require(
        "pub const BEANKEEPER_DATABASE_SCHEMA_VERSION: i64 = 8;" in foundation,
        "pinned Beankeeper database schema remains exactly 8",
    )
    require("mod mutation_audit_application;" in foundation, "foundation owns Mutation + Audit persistence module")
    for anchor in [
        "OwnerCorrectionPersistOutcome",
        "OwnerCorrectionView",
        "OwnerCorrectionWrite",
        "ReceiptBankDecisionPersistOutcome",
        "ReceiptBankDecisionView",
        "ReceiptBankDecisionWrite",
    ]:
        require(anchor in foundation, f"typed foundation Mutation + Audit export present: {anchor}")
    open_pos = foundation.index("pub fn open_encrypted")
    require(
        foundation.index("backup_hook.backup_before_open", open_pos)
        < foundation.index("bank_application::ensure_application_schema(&db)?;", open_pos),
        "pre-open encrypted backup remains before Shark application-schema migration",
    )

    app_mod = text(repo, "workspace/shark-foundation/src/bank_application/mod.rs")
    for anchor in [
        "BANK_APPLICATION_SCHEMA_VERSION: u32 = 4",
        "SAVEPOINT shark_application_schema_v4",
        "CREATE TABLE IF NOT EXISTS shark_receipt_bank_decision",
        "CREATE TABLE IF NOT EXISTS shark_owner_correction",
        "idx_shark_owner_correction_replacement",
        "VALUES(1, 4)",
    ]:
        require(anchor in app_mod, f"schema-v4 anchor present: {anchor}")
    require("CREATE TABLE IF NOT EXISTS shark_document" in app_mod, "schema v4 retains document metadata")
    require("CREATE TABLE IF NOT EXISTS shark_bank_activity" in app_mod, "schema v4 retains bank activity")

    bank_tests = text(repo, "workspace/shark-foundation/src/bank_application/tests.rs")
    require(
        "application_schema_v4_is_separate_from_beankeeper_schema_8" in bank_tests
        and "assert_eq!(SHARK_APPLICATION_SCHEMA_VERSION, 4);" in bank_tests,
        "inherited bank schema regression is explicitly advanced to v4",
    )

    persistence = text(repo, "workspace/shark-foundation/src/mutation_audit_application.rs")
    for anchor in [
        "pub struct ReceiptBankDecisionWrite",
        "pub enum ReceiptBankDecisionPersistOutcome",
        "pub struct OwnerCorrectionWrite",
        "pub enum OwnerCorrectionPersistOutcome",
        "pub fn persist_receipt_bank_decision",
        "pub fn correction_for_original",
        "pub fn apply_owner_correction",
        "pub fn correction_history",
        "rejected_receipt_decision_is_idempotent_and_non_posting",
        "correction_applies_reversal_and_replacement_atomically_and_is_idempotent",
        "failed_replacement_rolls_back_reversal_and_history",
    ]:
        require(anchor in persistence, f"Mutation + Audit persistence anchor present: {anchor}")
    runtime_persistence = persistence.split("#[cfg(test)]", 1)[0].lower()
    for forbidden in [
        "std::process", "command::new", "reqwest::", "ureq::", "http://", "https://",
        "hash_and_store_file", "store_attachment(",
    ]:
        require(forbidden not in runtime_persistence, f"Mutation + Audit persistence excludes forbidden primitive: {forbidden}")
    require(
        "receipt suggestion id already has a conflicting immutable decision" in persistence,
        "receipt decision identity is append-only/idempotent",
    )
    require(
        "original owner record is already superseded by a correction" in persistence,
        "correction persistence prevents in-place/repeated supersession of one original",
    )
    require(
        "shark_owner_correction_apply" in persistence,
        "reversal + optional replacement + history use one Shark savepoint",
    )

    owner = text(repo, "workspace/shark-tauri-spike/src/owner_mutation_audit.rs")
    for anchor in [
        "ReceiptSuggestionRegistry",
        "MAX_RECEIPT_SUGGESTIONS",
        "core_extraction_from_native",
        "bank_line_from_activity",
        "rank_receipt_bank_suggestions",
        "confirm_receipt_bank_suggestion",
        "reject_receipt_bank_suggestions",
        "CorrectionPlan::new",
        "owner_receipt_suggest_bank",
        "owner_receipt_confirm_bank",
        "owner_receipt_reject_bank",
        "owner_correction_preview",
        "owner_correction_confirm",
        "owner_correction_history",
        "receipt_requests_reject_raw_bank_path_and_accounting_authority",
        "correction_request_rejects_raw_transaction_and_posting_authority",
        "receipt_registry_is_bounded_and_oldest_entry_expires",
    ]:
        require(anchor in owner, f"owner Mutation + Audit bridge anchor present: {anchor}")
    runtime_owner = owner.split("#[cfg(test)]", 1)[0].lower()
    for marker in [
        "use beankeeper", "use rusqlite", "tauri_plugin_shell", "reqwest::", "ureq::",
        "std::net", "tokio::net", "http://", "https://", "database_path:", "db_path:",
        "passphrase:", "shellcommand:", "sourcepath:", "destinationpath:",
    ]:
        require(marker not in runtime_owner, f"owner Mutation + Audit bridge excludes forbidden/raw surface: {marker}")
    require("requires_further_automatic_action: false" in owner, "receipt/correction confirmations stop after explicit owner action")
    require("matched_transaction_id.is_some()" in owner, "receipt confirmation rejects bank activity that changed after review")
    require("preview_fingerprint" in owner, "correction confirm is bound to a deterministic preview fingerprint")

    shell = text(repo, "workspace/shark-tauri-spike/src/lib.rs")
    build_rs = text(repo, "workspace/shark-tauri-spike/build.rs")
    permission = text(repo, "workspace/shark-tauri-spike/permissions/shark-shell.toml")
    frontend = "\n".join([
        text(repo, "workspace/dist/index.html"),
        text(repo, "workspace/dist/app.js"),
    ])
    require(
        "manage(owner_mutation_audit::ReceiptSuggestionRegistry::default())" in shell,
        "receipt suggestion registry is Rust-managed state",
    )
    for command in NEW_COMMANDS:
        require(f"owner_mutation_audit::{command}" in shell, f"Rust invoke handler wires command: {command}")
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
        "SBC7B1-Mutation-Audit-Proof-Key-Only-Do-Not-Ship",
    )
    proof_root = Path.home() / "sbc7b1-mutation-audit-proof-books"
    proof_root.mkdir(parents=True, exist_ok=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(proof_root)
    env["RUST_TEST_THREADS"] = "1"

    target = Path(env.get("CARGO_TARGET_DIR", "")) if env.get("CARGO_TARGET_DIR") else Path.home() / "sbc7b1-mutation-audit-proof-target"
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
        require(True, "seven exact portable inherited OCR boundary tests pass")

        for test_filter in [
            "owner_app::tests",
            "owner_bank_review::tests",
            "owner_bank_mutation::tests",
            "owner_documents_ocr::tests",
            "owner_mutation_audit::tests",
            "tests::encrypted_command_cycle_uses_only_shark_facade",
            "tests::frontend_contract_is_keyless_and_uses_only_bounded_commands",
            "tests::tauri_acl_and_csp_are_bounded",
        ]:
            run(repo, [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                test_filter, "--", "--test-threads=1",
            ], env=env)
        run(repo, [
            "rustup", "run", RUST_TOOLCHAIN, "cargo", "check",
            "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
            "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
        ], env=env)

        current = (repo / "workspace" / "Cargo.lock").read_bytes()
        require(sha256_bytes(current) == NATIVE_LOCK_SHA256, "native Cargo.lock unchanged through runtime gate")
        require(not git(repo, "status", "--porcelain").strip(), "repository clean after runtime gate")
    finally:
        shutil.rmtree(target, ignore_errors=True)
        shutil.rmtree(proof_root, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    try:
        result = static_gate(repo)
        if not args.static_only:
            runtime_gate(repo)
        print(f"[PASS] SBC-7B1 Mutation + Audit gate complete at {result['head']}")
        return 0
    except Exception as error:
        print(f"[FAIL] {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
