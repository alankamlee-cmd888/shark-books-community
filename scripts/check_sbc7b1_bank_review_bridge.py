#!/usr/bin/env python3
"""Fail-closed SBC-7B1 read-only Bank review bridge gate."""

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

BASE = "bbe31ba295b28e1dcfcddeabe30ae49515353e35"
RUST_TOOLCHAIN = "1.98.1"
NATIVE_LOCK_SHA256 = "8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61"

ALLOWED_PATHS = {
    "ci/run_sbc7b1_bank_review_codemagic.sh",
    "ci/run_sbc7b1_bank_review_windows.ps1",
    "codemagic.yaml",
    "docs/SBC7B1_BANK_REVIEW_BRIDGE_CONTRACT_2026-09-10.md",
    "scripts/check_sbc7b1_bank_review_bridge.py",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/owner_bank_review.rs",
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
]

NEW_COMMANDS = [
    "owner_bank_import_preview_csv",
    "owner_bank_import_preview_ofx_qfx",
    "owner_bank_match_review",
    "owner_bank_reconcile_preview",
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
    require(True, "SBC-7B1 Slice 1 protected merge is an ancestor of candidate HEAD")

    changed = {
        line.strip()
        for line in git(repo, "diff", "--name-only", f"{BASE}..HEAD").splitlines()
        if line.strip()
    }
    require(
        changed == ALLOWED_PATHS,
        f"candidate changed paths equal exact Bank review allow-list ({len(ALLOWED_PATHS)} paths)",
    )

    for protected in [
        "product/shark-books-core",
        "workspace/shark-foundation",
        "workspace/shark-tauri-spike/src/ocr_native.rs",
        "workspace/shark-tauri-spike/Cargo.toml",
        "workspace/Cargo.lock",
        "workspace/shark-tauri-spike/tauri.conf.json",
        "workspace/shark-tauri-spike/capabilities/default.json",
        "workspace/dist",
    ]:
        diff = git(repo, "diff", "--name-only", f"{BASE}..HEAD", "--", protected).strip()
        require(not diff, f"protected path unchanged: {protected}")

    lock_path = repo / "workspace" / "Cargo.lock"
    lock_bytes = lock_path.read_bytes()
    require(
        sha256_bytes(lock_bytes) == NATIVE_LOCK_SHA256,
        "native Cargo.lock remains exact from merged Slice 1",
    )

    owner = (
        repo / "workspace" / "shark-tauri-spike" / "src" / "owner_bank_review.rs"
    ).read_text(encoding="utf-8")
    required_owner_anchors = [
        "deny_unknown_fields",
        "MAX_STATEMENT_BYTES",
        "MAX_MATCH_CANDIDATES",
        "MAX_RECONCILIATION_ROWS",
        "core::bank_import::preview_csv",
        "core::bank_import::preview_ofx_or_qfx",
        "core::matching::rank_matches",
        "core::matching::preview_reconciliation",
        "books.transaction(*id)",
        'const BANK_ACCOUNT_CODE: &str = "1000"',
        "requires_explicit_confirmation",
        "owner_bank_import_preview_csv",
        "owner_bank_import_preview_ofx_qfx",
        "owner_bank_match_review",
        "owner_bank_reconcile_preview",
        "unsafe_or_unknown_request_fields_are_rejected",
        "candidate_lists_are_bounded_unique_positive_ids",
        "reconciliation_preview_preserves_exact_zero_and_cleared_rule",
    ]
    for anchor in required_owner_anchors:
        require(anchor in owner, f"Bank review bridge anchor present: {anchor}")

    forbidden_owner_markers = [
        "use beankeeper",
        "use rusqlite",
        "reqwest::",
        "ureq::",
        "std::net",
        "tokio::net",
        "std::process",
        "Command::new",
        "tauri_plugin_shell",
        "http://",
        "https://",
        "databasePath:",
        "db_path:",
        "passphrase:",
        ".post(",
        "confirm_match(",
        "finalize_reconciliation(",
        "reconcile_entry(",
        "set_reconciled(",
        "attach_document(",
        "confirm_receipt_bank_suggestion(",
        "reject_receipt_bank_suggestions(",
    ]
    runtime_owner = owner.split("#[cfg(test)]", 1)[0].lower()
    for marker in forbidden_owner_markers:
        require(
            marker.lower() not in runtime_owner,
            f"Bank review bridge excludes forbidden mutating/raw surface: {marker}",
        )

    require(
        "statement_text: String" in owner and "path: String" not in runtime_owner,
        "Bank preview accepts bounded content and no owner-supplied OS path",
    )
    require(
        "candidate_transaction_ids: Vec<i64>" in owner,
        "match review accepts only bounded transaction identifiers, not ledger entries",
    )
    require(
        "transaction_ids: Vec<i64>" in owner,
        "reconciliation preview accepts only bounded transaction identifiers",
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

    require("mod owner_bank_review;" in lib_rs, "Rust shell declares Bank review module")
    for command in EXISTING_COMMANDS + NEW_COMMANDS:
        require(command in build_rs, f"Tauri manifest contains command: {command}")
        require(command in permission, f"Tauri exact permission contains command: {command}")
    for command in NEW_COMMANDS:
        require(
            f"owner_bank_review::{command}" in lib_rs,
            f"Rust invoke handler wires Bank review command: {command}",
        )
        require(
            command not in frontend,
            f"frontend remains unwired from Bank review command: {command}",
        )

    require(
        "owner_bank_import_confirm" not in build_rs
        and "owner_bank_match_confirm" not in build_rs
        and "owner_bank_reconcile_finalise" not in build_rs,
        "mutating Bank commands remain absent from this read-only slice",
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
    env.setdefault("SHARK_SBC1D_PROOF_KEY", "SBC7B1-Bank-Review-Proof-Key-Only-Do-Not-Ship")
    proof_root = Path(tempfile.gettempdir()) / "sharkbooks-sbc7b1-bank-review-proof-books"
    proof_root.mkdir(parents=True, exist_ok=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(proof_root)
    env["RUST_TEST_THREADS"] = "1"

    cargo_target = Path(tempfile.gettempdir()) / "SharkBooks-SBC7B1-BankReview-CargoTarget"
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
            ],
            env=env,
        )
        run(
            repo,
            [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                "owner_app::tests", "--", "--test-threads=1",
            ],
            env=env,
        )
        run(
            repo,
            [
                "rustup", "run", RUST_TOOLCHAIN, "cargo", "test",
                "--manifest-path", str(repo / "workspace" / "Cargo.toml"),
                "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
                "owner_bank_review::tests", "--", "--test-threads=1",
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
            "native Cargo.lock unchanged through Bank review runtime gate",
        )
        status = git(repo, "status", "--porcelain").strip()
        require(not status, "repository clean after Bank review runtime gate")
    finally:
        shutil.rmtree(cargo_target, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    result: dict[str, object] = {
        "schema": "sbc7b1-bank-review-bridge-v1",
        "gate": "SBC-7B1-BANK-REVIEW",
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
        print(f"[FAIL] SBC-7B1 Bank review gate: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
