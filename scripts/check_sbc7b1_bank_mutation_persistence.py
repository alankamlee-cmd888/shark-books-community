#!/usr/bin/env python3
"""Fail-closed SBC-7B1 Slice 2B Bank mutation/persistence gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

BASE = "4b3eee65bcef24c3ee7d6c06f404aa4180a7c4d8"
RUST_TOOLCHAIN = "1.98.1"
NATIVE_LOCK_SHA256 = "8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61"

ALLOWED_PATHS = {
    "ci/run_sbc7b1_bank_mutation_codemagic.sh",
    "ci/run_sbc7b1_bank_mutation_windows.ps1",
    "codemagic.yaml",
    "docs/SBC7B1_BANK_MUTATION_PERSISTENCE_CONTRACT_2026-09-11.md",
    "scripts/check_sbc7b1_bank_mutation_persistence.py",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-foundation/src/bank_application/mod.rs",
    "workspace/shark-foundation/src/bank_application/activity.rs",
    "workspace/shark-foundation/src/bank_application/match_reconcile.rs",
    "workspace/shark-foundation/src/bank_application/tests.rs",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/owner_bank_mutation.rs",
}

EXISTING_COMMANDS = [
    "foundation_health",
    "production_encryption_required",
    "books_create",
    "books_open",
    "books_verify",
    "books_trial_balance",
    "ocr_extract_receipt",
    "owner_home_status",
    "owner_money_in_preview",
    "owner_money_in_save",
    "owner_money_out_preview",
    "owner_money_out_save",
    "owner_bank_import_preview_csv",
    "owner_bank_import_preview_ofx_qfx",
    "owner_bank_match_review",
    "owner_bank_reconcile_preview",
]

NEW_COMMANDS = [
    "owner_bank_import_confirm_csv",
    "owner_bank_import_confirm_ofx_qfx",
    "owner_bank_activity_list",
    "owner_bank_match_confirm",
    "owner_bank_reconcile_finalise",
]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(
    repo: Path,
    args: list[str],
    *,
    env: dict[str, str] | None = None,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        args,
        cwd=repo,
        env=env,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
    )
    if capture:
        if proc.stdout:
            sys.stdout.write(proc.stdout)
        if proc.stderr:
            sys.stderr.write(proc.stderr)
    if proc.returncode != 0:
        raise RuntimeError(f"command failed ({proc.returncode}): {' '.join(args)}")
    return proc


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


def static_gate(repo: Path) -> dict[str, object]:
    head = git(repo, "rev-parse", "HEAD").strip()
    run(repo, ["git", "merge-base", "--is-ancestor", BASE, head])
    require(True, "Slice 2A protected merge is an ancestor of candidate HEAD")

    changed = {
        line.strip()
        for line in git(repo, "diff", "--name-only", f"{BASE}..HEAD").splitlines()
        if line.strip()
    }
    require(
        changed == ALLOWED_PATHS,
        f"candidate changed paths equal exact Bank mutation allow-list ({len(ALLOWED_PATHS)} paths)",
    )

    for protected in [
        "product/shark-books-core",
        "workspace/shark-foundation/Cargo.toml",
        "workspace/shark-tauri-spike/Cargo.toml",
        "workspace/Cargo.lock",
        "workspace/shark-tauri-spike/src/ocr_native.rs",
        "workspace/shark-tauri-spike/src/owner_app.rs",
        "workspace/shark-tauri-spike/src/owner_bank_review.rs",
        "workspace/shark-tauri-spike/tauri.conf.json",
        "workspace/shark-tauri-spike/tauri.ios.conf.json",
        "workspace/shark-tauri-spike/capabilities",
        "workspace/dist",
        "scripts/bootstrap_beankeeper.py",
    ]:
        diff = git(repo, "diff", "--name-only", f"{BASE}..HEAD", "--", protected).strip()
        require(not diff, f"protected path unchanged: {protected}")

    lock_path = repo / "workspace" / "Cargo.lock"
    lock_bytes = lock_path.read_bytes()
    require(
        sha256_bytes(lock_bytes) == NATIVE_LOCK_SHA256,
        "native Cargo.lock remains exact from merged Slice 2A",
    )

    foundation = (repo / "workspace" / "shark-foundation" / "src" / "lib.rs").read_text(
        encoding="utf-8"
    )
    require(
        "pub const SHARK_APPLICATION_SCHEMA_VERSION: u32 = 2;" in foundation,
        "Shark application schema version is exactly 2",
    )
    require(
        "pub const BEANKEEPER_DATABASE_SCHEMA_VERSION: i64 = 8;" in foundation,
        "pinned Beankeeper database schema remains 8",
    )
    require(
        foundation.index("backup_hook.backup_before_open")
        < foundation.index("bank_application::ensure_application_schema(&db)?;", foundation.index("pub fn open_encrypted")),
        "existing encrypted books are backed up before Shark application-schema migration",
    )

    app_mod = (
        repo / "workspace" / "shark-foundation" / "src" / "bank_application" / "mod.rs"
    ).read_text(encoding="utf-8")
    activity = (
        repo / "workspace" / "shark-foundation" / "src" / "bank_application" / "activity.rs"
    ).read_text(encoding="utf-8")
    match_reconcile = (
        repo
        / "workspace"
        / "shark-foundation"
        / "src"
        / "bank_application"
        / "match_reconcile.rs"
    ).read_text(encoding="utf-8")
    foundation_tests = (
        repo / "workspace" / "shark-foundation" / "src" / "bank_application" / "tests.rs"
    ).read_text(encoding="utf-8")

    for anchor in [
        "shark_application_meta",
        "shark_bank_activity",
        "shark_bank_match",
        "shark_bank_reconciliation",
        "shark_bank_reconciliation_entry",
        "SAVEPOINT shark_application_schema_v2",
        "BANK_APPLICATION_SCHEMA_VERSION: u32 = 2",
        "UNIQUE(company_slug, source_file_sha256, source_locator, raw_record_sha256)",
        "idx_shark_bank_activity_strong",
    ]:
        require(anchor in app_mod, f"application-schema anchor present: {anchor}")

    for anchor in [
        "persist_bank_activity_batch",
        "shark_bank_import_confirm",
        "StrongDuplicate",
        "FileExactDuplicate",
        "strong bank identity",
        "bank_activity(",
        "list_bank_activity(",
    ]:
        require(anchor in activity, f"Bank activity persistence anchor present: {anchor}")
    runtime_activity = activity.split("#[cfg(test)]", 1)[0]
    require(
        ".post(" not in runtime_activity and "post_transaction(" not in runtime_activity,
        "Bank activity persistence never posts accounting transactions",
    )

    for anchor in [
        "confirm_bank_match",
        "EntryStatus::Cleared",
        "EntryStatus::Uncleared",
        "shark_bank_match_confirm",
        "finalize_bank_reconciliation",
        "EntryStatus::Reconciled",
        "shark_bank_reconcile_finalise",
        "owner-confirmed bank match",
        "statement difference must be zero",
        "AlreadyConfirmed",
        "AlreadyFinalized",
    ]:
        require(anchor in match_reconcile, f"Bank mutation invariant present: {anchor}")

    for anchor in [
        "bank_activity_batch_is_atomic_and_does_not_post_accounting_transactions",
        "match_confirmation_is_atomic_audited_forward_only_and_idempotent",
        "reconciliation_finalisation_is_exact_zero_audited_and_idempotent",
        "failed_reconciliation_leaves_cleared_state_and_no_header",
        "application_schema_v2_is_separate_from_beankeeper_schema_8",
    ]:
        require(anchor in foundation_tests, f"foundation mutation regression present: {anchor}")

    owner = (
        repo / "workspace" / "shark-tauri-spike" / "src" / "owner_bank_mutation.rs"
    ).read_text(encoding="utf-8")
    for anchor in [
        "deny_unknown_fields",
        "MAX_STATEMENT_BYTES",
        "MAX_IMPORT_LINES",
        "MAX_MATCH_CANDIDATES",
        "MAX_RECONCILIATION_ROWS",
        "confirmed_statement_sha256",
        "confirm_current_preview",
        "core::bank_import::preview_csv",
        "core::bank_import::preview_ofx_or_qfx",
        "core::matching::rank_matches",
        "core::matching::confirm_match",
        "core::matching::finalize_reconciliation",
        "bank_line_from_persisted",
        "books.transaction(*id)",
        "owner_bank_import_confirm_csv",
        "owner_bank_import_confirm_ofx_qfx",
        "owner_bank_activity_list",
        "owner_bank_match_confirm",
        "owner_bank_reconcile_finalise",
        "import_confirmation_is_bound_to_current_statement_and_preview_echo",
        "unsafe_path_secret_and_ledger_shaped_fields_are_rejected",
    ]:
        require(anchor in owner, f"owner Bank mutation bridge anchor present: {anchor}")

    runtime_owner = owner.split("#[cfg(test)]", 1)[0].lower()
    for marker in [
        "use beankeeper",
        "use rusqlite",
        "std::process",
        "command::new",
        "tauri_plugin_shell",
        "reqwest::",
        "ureq::",
        "std::net",
        "tokio::net",
        "http://",
        "https://",
        "database_path:",
        "db_path:",
        "passphrase:",
        "postingline",
        "posttransactionrequest",
        ".post(",
        "create_account(",
    ]:
        require(
            marker not in runtime_owner,
            f"owner Bank mutation bridge excludes forbidden/raw surface: {marker}",
        )
    require(
        "statement_text: String" in owner and "path: String" not in runtime_owner,
        "Bank import confirmation accepts bounded content and no owner-supplied OS path",
    )

    lib_rs = (repo / "workspace" / "shark-tauri-spike" / "src" / "lib.rs").read_text(
        encoding="utf-8"
    )
    build_rs = (repo / "workspace" / "shark-tauri-spike" / "build.rs").read_text(
        encoding="utf-8"
    )
    permission = (
        repo / "workspace" / "shark-tauri-spike" / "permissions" / "shark-shell.toml"
    ).read_text(encoding="utf-8")
    frontend = "\n".join(
        path.read_text(encoding="utf-8")
        for path in [
            repo / "workspace" / "dist" / "index.html",
            repo / "workspace" / "dist" / "app.js",
        ]
    )

    require("mod owner_bank_mutation;" in lib_rs, "Rust shell declares Bank mutation module")
    for command in EXISTING_COMMANDS + NEW_COMMANDS:
        require(command in build_rs, f"Tauri manifest contains command: {command}")
        require(command in permission, f"Tauri exact permission contains command: {command}")
    for command in NEW_COMMANDS:
        require(
            f"owner_bank_mutation::{command}" in lib_rs,
            f"Rust invoke handler wires Bank mutation command: {command}",
        )
        require(
            command not in frontend,
            f"frontend remains unwired from Bank mutation command: {command}",
        )

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
        "SBC7B1-Bank-Mutation-Proof-Key-Only-Do-Not-Ship",
    )
    proof_root = Path(tempfile.gettempdir()) / "sharkbooks-sbc7b1-bank-mutation-proof-books"
    proof_root.mkdir(parents=True, exist_ok=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(proof_root)
    env["RUST_TEST_THREADS"] = "1"

    cargo_target = Path(tempfile.gettempdir()) / "SharkBooks-SBC7B1-BankMutation-CargoTarget"
    if cargo_target.exists():
        shutil.rmtree(cargo_target, ignore_errors=True)
    cargo_target.mkdir(parents=True, exist_ok=True)
    env["CARGO_TARGET_DIR"] = str(cargo_target)

    try:
        run(
            repo,
            [sys.executable, "-B", str(repo / "scripts" / "bootstrap_beankeeper.py")],
            env=env,
        )
        run(repo, ["rustup", "run", RUST_TOOLCHAIN, "rustc", "--version"], env=env)
        run(
            repo,
            [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "product" / "shark-books-core" / "Cargo.toml"),
                "--locked", "--jobs", "1",
            ],
            env=env,
        )
        run(
            repo,
            [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-foundation", "--locked", "--jobs", "1",
                "--", "--test-threads=1",
            ],
            env=env,
        )
        for test_filter in [
            "owner_app::tests",
            "owner_bank_review::tests",
            "owner_bank_mutation::tests",
            "tests::encrypted_command_cycle_uses_only_shark_facade",
            "tests::frontend_contract_is_keyless_and_uses_only_bounded_commands",
            "tests::tauri_acl_and_csp_are_bounded",
        ]:
            run(
                repo,
                [
                    "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                    "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                    "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                    test_filter, "--", "--test-threads=1",
                ],
                env=env,
            )
        run(
            repo,
            [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "check",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
            ],
            env=env,
        )

        current = (repo / "workspace" / "Cargo.lock").read_bytes()
        require(
            sha256_bytes(current) == NATIVE_LOCK_SHA256,
            "native Cargo.lock unchanged through Bank mutation runtime gate",
        )
        status = git(repo, "status", "--porcelain").strip()
        require(not status, "repository clean after Bank mutation runtime gate")
    finally:
        shutil.rmtree(cargo_target, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    result: dict[str, object] = {
        "schema": "sbc7b1-bank-mutation-persistence-v1",
        "gate": "SBC-7B1-BANK-MUTATION",
        "gate_pass": False,
        "entry_protected_main": BASE,
        "rust_toolchain": RUST_TOOLCHAIN,
    }

    try:
        result.update(static_gate(repo))
        if not args.static_only:
            runtime_gate(repo)
        result["static_only"] = bool(args.static_only)
        result["gate_pass"] = True
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except Exception as exc:
        result["error"] = str(exc)
        print(json.dumps(result, indent=2, sort_keys=True))
        print(f"[FAIL] SBC-7B1 Bank mutation gate: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
