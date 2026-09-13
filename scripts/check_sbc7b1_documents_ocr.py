#!/usr/bin/env python3
"""Fail-closed SBC-7B1 Slice 3A Documents + factual OCR owner-bridge gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

BASE = "50c898c7b8042e11810933e3e596fc124085e831"
RUST_TOOLCHAIN = "1.98.1"
NATIVE_LOCK_SHA256 = "10fe1431f26cef2d427e76658690544027e955845faabfbbdaebf2a2ad17e115"

ALLOWED_PATHS = {
    "ci/run_sbc7b1_documents_ocr_codemagic.sh",
    "ci/run_sbc7b1_documents_ocr_windows.ps1",
    "codemagic.yaml",
    "docs/SBC7B1_DOCUMENTS_OCR_IMPLEMENTATION_DESIGN_2026-09-12.md",
    "docs/SBC7B1_DOCUMENTS_OCR_OWNER_BRIDGE_CONTRACT_2026-09-12.md",
    "scripts/check_sbc7b1_documents_ocr.py",
    "workspace/Cargo.lock",
    "workspace/shark-foundation/src/bank_application/mod.rs",
    "workspace/shark-foundation/src/bank_application/tests.rs",
    "workspace/shark-foundation/src/document_application.rs",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-tauri-spike/Cargo.toml",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/owner_documents_ocr.rs",
}

NEW_COMMANDS = [
    "owner_document_select_register",
    "owner_document_verify",
    "owner_document_attach",
    "owner_ocr_extract_receipt",
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
    require(True, "Slice 2B protected merge is an ancestor of candidate HEAD")

    changed = {
        line.strip()
        for line in git(repo, "diff", "--name-only", f"{BASE}..HEAD").splitlines()
        if line.strip()
    }
    require(
        changed == ALLOWED_PATHS,
        f"candidate changed paths equal exact Documents/OCR allow-list ({len(ALLOWED_PATHS)} paths)",
    )

    for protected in [
        "product/shark-books-core",
        "workspace/shark-foundation/Cargo.toml",
        "workspace/shark-tauri-spike/src/ocr_native.rs",
        "workspace/shark-tauri-spike/src/owner_app.rs",
        "workspace/shark-tauri-spike/src/owner_bank_review.rs",
        "workspace/shark-tauri-spike/src/owner_bank_mutation.rs",
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
        "native Cargo.lock equals reviewed Slice 3A dependency lock",
    )

    cargo_toml = text(repo, "workspace/shark-tauri-spike/Cargo.toml")
    require(
        'tauri-plugin-dialog = { version = "=2.7.3", default-features = false }' in cargo_toml,
        "only approved direct native dialog dependency is pinned exactly",
    )
    for forbidden in [
        "tauri-plugin-shell", "reqwest", "ureq", "hyper =", "@tauri-apps/plugin-dialog",
    ]:
        require(forbidden not in cargo_toml, f"Tauri manifest excludes forbidden dependency: {forbidden}")

    lock = lock_bytes.decode("utf-8")
    for anchor in [
        'name = "tauri-plugin-dialog"\nversion = "2.7.3"',
        'name = "rfd"\nversion = "0.16.0"',
        'name = "tauri-plugin-fs"\nversion = "2.5.2"',
    ]:
        require(anchor in lock, f"reviewed lock dependency present: {anchor.splitlines()[0]}")

    base_lock = git(repo, "show", f"{BASE}:workspace/Cargo.lock")
    for forbidden in ['name = "reqwest"', 'name = "ureq"', 'name = "tauri-plugin-shell"']:
        require(
            lock.count(forbidden) == base_lock.count(forbidden),
            f"dependency delta introduces no new forbidden package: {forbidden}",
        )

    foundation = text(repo, "workspace/shark-foundation/src/lib.rs")
    require(
        "pub const SHARK_APPLICATION_SCHEMA_VERSION: u32 = 3;" in foundation,
        "Shark application schema version is exactly 3",
    )
    require(
        "pub const BEANKEEPER_DATABASE_SCHEMA_VERSION: i64 = 8;" in foundation,
        "pinned Beankeeper database schema remains exactly 8",
    )
    open_pos = foundation.index("pub fn open_encrypted")
    require(
        foundation.index("backup_hook.backup_before_open", open_pos)
        < foundation.index("bank_application::ensure_application_schema(&db)?;", open_pos),
        "pre-open encrypted backup remains before Shark application-schema migration",
    )

    app_mod = text(repo, "workspace/shark-foundation/src/bank_application/mod.rs")
    for anchor in [
        "BANK_APPLICATION_SCHEMA_VERSION: u32 = 3",
        "SAVEPOINT shark_application_schema_v3",
        "CREATE TABLE IF NOT EXISTS shark_document",
        "CREATE TABLE IF NOT EXISTS shark_document_attachment",
        "VALUES(1, 3)",
    ]:
        require(anchor in app_mod, f"schema-v3 anchor present: {anchor}")

    bank_tests = text(repo, "workspace/shark-foundation/src/bank_application/tests.rs")
    require(
        "application_schema_v3_is_separate_from_beankeeper_schema_8" in bank_tests
        and "assert_eq!(SHARK_APPLICATION_SCHEMA_VERSION, 3);" in bank_tests,
        "inherited bank schema regression is explicitly advanced to v3",
    )

    docs = text(repo, "workspace/shark-foundation/src/document_application.rs")
    for anchor in [
        "pub struct DocumentWrite",
        "pub struct DocumentView",
        "pub enum DocumentPersistOutcome",
        "pub struct DocumentAttachmentWrite",
        "pub enum DocumentAttachmentPersistOutcome",
        "pub fn register_document",
        "pub fn document(",
        "pub fn attach_registered_document",
        "document_registration_is_idempotent_and_conflicts_fail_closed",
        "attachment_is_typed_idempotent_and_does_not_post",
    ]:
        require(anchor in docs, f"document persistence anchor present: {anchor}")
    runtime_docs = docs.split("#[cfg(test)]", 1)[0].lower()
    for forbidden in ["hash_and_store_file", "store_attachment(", "std::process", "reqwest", "ureq"]:
        require(forbidden not in runtime_docs, f"document persistence excludes forbidden primitive: {forbidden}")

    owner = text(repo, "workspace/shark-tauri-spike/src/owner_documents_ocr.rs")
    for anchor in [
        "NativeDocumentRootRegistry",
        "register_native_root",
        "blocking_pick_file",
        "read_bounded_file",
        "copy_selected_to_root",
        "current_integrity",
        "require_verified_document",
        "owner_document_select_register",
        "owner_document_verify",
        "owner_document_attach",
        "owner_ocr_extract_receipt",
        "owner_requests_reject_path_hash_key_and_account_authority",
        "verification_detects_hash_tamper_size_tamper_and_missing_file",
        "unsafe_persisted_relative_path_is_rejected_before_join",
    ]:
        require(anchor in owner, f"owner Documents/OCR bridge anchor present: {anchor}")
    runtime_owner = owner.split("#[cfg(test)]", 1)[0].lower()
    for marker in [
        "use beankeeper", "use rusqlite", "tauri_plugin_shell", "reqwest::", "ureq::",
        "std::net", "tokio::net", "http://", "https://", "database_path:", "db_path:",
        "passphrase:", "postingline", "posttransactionrequest", "create_account(",
    ]:
        require(marker not in runtime_owner, f"owner Documents/OCR bridge excludes forbidden/raw surface: {marker}")
    require(
        "source_path:" not in runtime_owner and "destination_path:" not in runtime_owner,
        "serializable owner DTOs expose no source/destination OS paths",
    )
    require(
        "core::bank_import::sha256_hex" in owner,
        "document integrity derives SHA-256 from native-selected/current bytes",
    )
    require(
        "ocr_native::ocr_extract_receipt" in owner,
        "owner OCR delegates to frozen native OCR runtime",
    )

    shell = text(repo, "workspace/shark-tauri-spike/src/lib.rs")
    build_rs = text(repo, "workspace/shark-tauri-spike/build.rs")
    permission = text(repo, "workspace/shark-tauri-spike/permissions/shark-shell.toml")
    frontend = "\n".join([
        text(repo, "workspace/dist/index.html"),
        text(repo, "workspace/dist/app.js"),
    ])
    require(".plugin(tauri_plugin_dialog::init())" in shell, "native dialog plugin is initialised in Rust")
    require("manage(owner_documents_ocr::NativeDocumentRootRegistry::default())" in shell, "native document root registry is Rust-managed state")
    for command in NEW_COMMANDS:
        require(f"owner_documents_ocr::{command}" in shell, f"Rust invoke handler wires command: {command}")
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
        "SBC7B1-Documents-OCR-Proof-Key-Only-Do-Not-Ship",
    )
    proof_root = Path.home() / "sbc7b1-docsocr-proof-books"
    proof_root.mkdir(parents=True, exist_ok=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(proof_root)
    env["RUST_TEST_THREADS"] = "1"

    target = Path(env.get("CARGO_TARGET_DIR", "")) if env.get("CARGO_TARGET_DIR") else Path.home() / "sbc7b1-docsocr-proof-target"
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
        require(
            True,
            "seven exact portable inherited OCR boundary tests pass; packaged Windows B3C runtime is inherited from unchanged frozen ocr_native.rs",
        )

        for test_filter in [
            "owner_app::tests",
            "owner_bank_review::tests",
            "owner_bank_mutation::tests",
            "owner_documents_ocr::tests",
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
    result: dict[str, object] = {
        "schema": "sbc7b1-documents-ocr-v1",
        "gate": "SBC-7B1-DOCUMENTS-OCR",
        "gate_pass": False,
        "entry_protected_main": BASE,
        "rust_toolchain": RUST_TOOLCHAIN,
        "reviewed_native_lock_sha256": NATIVE_LOCK_SHA256,
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
        print(f"[FAIL] SBC-7B1 Documents/OCR gate: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())