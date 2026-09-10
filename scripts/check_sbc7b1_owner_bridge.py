#!/usr/bin/env python3
"""Fail-closed SBC-7B1 bounded owner application bridge gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

BASE = "70777e04ccfc1e427c60e18269b8a0738cc8610c"
RUST_TOOLCHAIN = "1.98.1"
NATIVE_LOCK_SHA256 = "8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61"
PRE_7B_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"

ALLOWED_PATHS = {
    "ci/run_sbc7b1_codemagic.sh",
    "ci/run_sbc7b1_windows.ps1",
    "codemagic.yaml",
    "docs/SBC7B1_OWNER_APPLICATION_BRIDGE_CONTRACT_2026-09-10.md",
    "scripts/check_sbc7b1_owner_bridge.py",
    "workspace/Cargo.lock",
    "workspace/shark-tauri-spike/Cargo.toml",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/owner_app.rs",
}

OWNER_COMMANDS = [
    "owner_home_status",
    "owner_money_in_preview",
    "owner_money_in_save",
    "owner_money_out_preview",
    "owner_money_out_save",
]

OLD_COMMANDS = [
    "foundation_health",
    "production_encryption_required",
    "books_create",
    "books_open",
    "books_verify",
    "books_trial_balance",
    "ocr_extract_receipt",
]

LOCAL_PACKAGE_BLOCK = '\n[[package]]\nname = "shark-books-core"\nversion = "0.0.1"\n'
LOCAL_DEP_EDGE = ' "shark-books-core",\n'


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(repo: Path, args: list[str], *, env: dict[str, str] | None = None, capture: bool = True) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        args,
        cwd=repo,
        env=env,
        text=True,
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
    require(True, "SBC-7A protected merge is an ancestor of candidate HEAD")

    changed = {line.strip() for line in git(repo, "diff", "--name-only", f"{BASE}..HEAD").splitlines() if line.strip()}
    require(changed == ALLOWED_PATHS, f"candidate changed paths equal exact SBC-7B1 allow-list ({len(ALLOWED_PATHS)} paths)")

    for protected in [
        "product/shark-books-core",
        "workspace/shark-foundation",
        "workspace/shark-tauri-spike/src/ocr_native.rs",
        "workspace/shark-tauri-spike/tauri.conf.json",
        "workspace/shark-tauri-spike/capabilities/default.json",
        "workspace/dist",
    ]:
        diff = git(repo, "diff", "--name-only", f"{BASE}..HEAD", "--", protected).strip()
        require(not diff, f"protected path unchanged: {protected}")

    lock_path = repo / "workspace" / "Cargo.lock"
    current_lock_bytes = lock_path.read_bytes()
    require(sha256_bytes(current_lock_bytes) == NATIVE_LOCK_SHA256, "native Cargo.lock equals reviewed SBC-7B1 hash")

    current_lock = current_lock_bytes.decode("utf-8")
    require(current_lock.count(LOCAL_PACKAGE_BLOCK) == 1, "lockfile contains exactly one local shark-books-core package block")
    require(current_lock.count(LOCAL_DEP_EDGE) == 1, "lockfile contains exactly one shark-books-core dependency edge")
    normalized = current_lock.replace(LOCAL_PACKAGE_BLOCK, "", 1).replace(LOCAL_DEP_EDGE, "", 1).encode("utf-8")
    require(sha256_bytes(normalized) == PRE_7B_NATIVE_LOCK_SHA256, "removing only local 7B1 additions reproduces frozen pre-7B lock byte-for-byte")

    cargo_toml = (repo / "workspace" / "shark-tauri-spike" / "Cargo.toml").read_text(encoding="utf-8")
    require('shark-books-core = { path = "../../product/shark-books-core" }' in cargo_toml, "Tauri shell uses reviewed local product-core path dependency")

    owner = (repo / "workspace" / "shark-tauri-spike" / "src" / "owner_app.rs").read_text(encoding="utf-8")
    required_owner_anchors = [
        "deny_unknown_fields",
        "OwnerMoneyInRequest",
        "OwnerMoneyOutRequest",
        "OwnerBusinessUse",
        "requires_confirmation: true",
        "core::plan_income",
        "core::plan_expense",
        ".post(&foundation_request)",
        "owner_home_status",
        "owner_money_in_preview",
        "owner_money_in_save",
        "owner_money_out_preview",
        "owner_money_out_save",
        "unsafe_or_ledger_shaped_request_fields_are_rejected",
        "invalid_dates_and_nonpositive_amounts_fail_closed",
    ]
    for anchor in required_owner_anchors:
        require(anchor in owner, f"owner bridge anchor present: {anchor}")

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
        "ocr_extract_receipt(",
        "suggest_receipt_bank_matches(",
        "confirm_receipt_bank_suggestion(",
        "confirm_match(",
        "finalize_reconciliation(",
    ]
    lowered = owner.lower()
    for marker in forbidden_owner_markers:
        require(marker.lower() not in lowered, f"owner bridge excludes forbidden direct surface: {marker}")

    require("business_use" in owner and "business_basis_points" in owner, "business/private/mixed treatment remains explicit owner input")
    require("amount_pence: i64" in owner, "owner money amounts remain whole-pence integers")

    lib_rs = (repo / "workspace" / "shark-tauri-spike" / "src" / "lib.rs").read_text(encoding="utf-8")
    build_rs = (repo / "workspace" / "shark-tauri-spike" / "build.rs").read_text(encoding="utf-8")
    permission = (repo / "workspace" / "shark-tauri-spike" / "permissions" / "shark-shell.toml").read_text(encoding="utf-8")
    frontend = "\n".join(
        path.read_text(encoding="utf-8")
        for path in [repo / "workspace" / "dist" / "index.html", repo / "workspace" / "dist" / "app.js"]
    )

    for command in OLD_COMMANDS + OWNER_COMMANDS:
        require(command in build_rs, f"Tauri manifest contains command: {command}")
        require(command in permission, f"Tauri exact permission contains command: {command}")
    for command in OWNER_COMMANDS:
        require(f"owner_app::{command}" in lib_rs, f"Rust invoke handler wires owner command: {command}")
        require(command not in frontend, f"frontend remains unwired from 7B1 command: {command}")

    status = git(repo, "status", "--porcelain").strip()
    require(not status, "repository clean before runtime gate")

    return {
        "head": head,
        "changed_paths": sorted(changed),
        "native_lock_sha256": sha256_bytes(current_lock_bytes),
    }


def runtime_gate(repo: Path) -> None:
    env = os.environ.copy()
    env.setdefault("SHARK_SBC1D_PROOF_KEY", "SBC7B1-Proof-Key-Only-Do-Not-Ship")
    proof_root = repo.parent / "sbc7b1-proof-books"
    proof_root.mkdir(parents=True, exist_ok=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(proof_root)
    env["RUST_TEST_THREADS"] = "1"

    run(repo, [sys.executable, "-B", str(repo / "scripts" / "bootstrap_beankeeper.py")], env=env)
    run(repo, ["rustup", "run", RUST_TOOLCHAIN, "rustc", "--version"], env=env)
    run(repo, ["rustup", "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(repo / "product" / "shark-books-core" / "Cargo.toml"), "--locked", "--jobs", "1"], env=env)
    run(repo, ["rustup", "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(repo / "workspace" / "Cargo.toml"), "-p", "shark-foundation", "--locked", "--jobs", "1"], env=env)
    run(repo, ["rustup", "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(repo / "workspace" / "Cargo.toml"), "-p", "shark-tauri-spike", "--locked", "--jobs", "1", "--", "--test-threads=1"], env=env)
    run(repo, ["rustup", "run", RUST_TOOLCHAIN, "cargo", "check", "--manifest-path", str(repo / "workspace" / "Cargo.toml"), "-p", "shark-tauri-spike", "--locked", "--jobs", "1"], env=env)

    current = (repo / "workspace" / "Cargo.lock").read_bytes()
    require(sha256_bytes(current) == NATIVE_LOCK_SHA256, "native Cargo.lock unchanged through runtime gate")
    status = git(repo, "status", "--porcelain").strip()
    require(not status, "repository clean after runtime gate")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    result: dict[str, object] = {
        "schema": "sbc7b1-owner-application-bridge-v1",
        "gate": "SBC-7B1",
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
        print(f"[FAIL] SBC-7B1 gate: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
