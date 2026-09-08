#!/usr/bin/env python3
"""SBC-5 fail-closed documents/storage validator."""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "7f0ac276c7d2a0c2da5dd2597535e946004c404f"
PRODUCT = ROOT / "product" / "shark-books-core"
MANIFEST = PRODUCT / "Cargo.toml"
PRODUCT_LOCK = PRODUCT / "Cargo.lock"
ENTRY = PRODUCT / "src" / "lib_sbc3.rs"
SOURCE = PRODUCT / "src" / "documents.rs"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
RUST_TOOLCHAIN = "1.98.1"

ALLOWED_PATHS = {
    "ci/run_sbc5_windows.ps1",
    "docs/SBC5_DOCUMENTS_STORAGE_IMPLEMENTATION_CONTRACT_2026-09-09.md",
    "product/shark-books-core/src/documents.rs",
    "product/shark-books-core/src/lib_sbc3.rs",
    "scripts/check_sbc5_documents_storage.py",
}

FORBIDDEN_SOURCE_MARKERS = (
    "beankeeper",
    "rusqlite",
    "tauri::",
    "std::fs",
    "std::path",
    "std::net",
    "tokio::net",
    "reqwest",
    "ureq",
    "hyper::",
    "access_token",
    "refresh_token",
    "api_key",
    "client_secret",
)

REQUIRED_SOURCE_ANCHORS = (
    "pub enum StorageProvider",
    "pub enum StorageCustody",
    "UserControlled",
    "pub struct StorageRootDescriptor",
    "pub struct DocumentReference",
    "pub fn from_bytes",
    "pub fn from_persisted",
    "pub fn verify_bytes",
    "pub enum IntegrityStatus",
    "pub struct NativeSelectionId",
    "pub struct CopySelectedDocumentRequest",
    "pub struct ReadVerifiedDocumentRequest",
    "validate_relative_path",
    "sha256_hex",
    "persisted_reopen_roundtrip_preserves_integrity_metadata",
    "tampered_same_length_document_fails_hash_check",
    "absolute_and_traversal_paths_fail_closed",
    "copy_request_uses_native_selection_id_and_relative_destination_only",
)


def fail(message: str) -> None:
    print(f"[FAIL] {message}")
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode != 0:
        fail(f"command exited {result.returncode}: {' '.join(args)}")
    return result


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def static_checks() -> None:
    if git("status", "--short"):
        fail("repository must be clean at SBC-5 validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    if not head:
        fail("could not resolve HEAD")
    passed(f"Candidate HEAD resolved: {head}")

    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", BASE, "HEAD"],
        cwd=ROOT, check=False,
    )
    if ancestor.returncode != 0:
        fail("SBC-4 protected merge is not an ancestor of candidate")
    passed("SBC-4 protected merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(
            "candidate diff is not exact SBC-5 allowlist\n"
            f"expected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}"
        )
    passed("Candidate diff is exactly the authorised five-path SBC-5 set")

    for path in (MANIFEST, PRODUCT_LOCK, ENTRY, SOURCE, NATIVE_LOCK):
        if not path.is_file():
            fail(f"required file missing: {path.relative_to(ROOT)}")

    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    package = manifest.get("package", {})
    if package.get("name") != "shark-books-core" or package.get("version") != "0.0.1":
        fail("unexpected shark-books-core package identity")
    if manifest.get("dependencies", {}) != {}:
        fail("SBC-5 product core must remain zero-third-party-dependency")
    lib = manifest.get("lib", {})
    if lib.get("path") != "src/lib_sbc3.rs":
        fail("unexpected product-core library entry point")
    passed("Product core remains dependency-free with bounded SBC-5 module entry")

    lock = tomllib.loads(PRODUCT_LOCK.read_text(encoding="utf-8"))
    packages = lock.get("package", [])
    if len(packages) != 1 or packages[0].get("name") != "shark-books-core" or packages[0].get("version") != "0.0.1":
        fail("product Cargo.lock must contain only shark-books-core 0.0.1")
    if "dependencies" in packages[0]:
        fail("product Cargo.lock unexpectedly contains dependencies")
    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock byte hash drift")
    passed("Product Cargo.lock remains exact and dependency-free")

    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("frozen native Cargo.lock hash drift")
    passed("Frozen native Cargo.lock remains exact")

    entry = ENTRY.read_text(encoding="utf-8")
    if "pub mod documents;" not in entry:
        fail("product entry point does not expose bounded documents module")

    source = SOURCE.read_text(encoding="utf-8")
    lowered = source.lower()
    for marker in FORBIDDEN_SOURCE_MARKERS:
        if marker.lower() in lowered:
            fail(f"document core contains forbidden platform/network/persistence/credential marker: {marker}")
    for anchor in REQUIRED_SOURCE_ANCHORS:
        if anchor not in source:
            fail(f"document core missing required contract anchor: {anchor}")
    if "SharkCentral" in source or "SharkHosted" in source:
        fail("document core contains a Shark central-custody provider surface")
    passed("Document/storage source contains required bounded contracts and no forbidden platform/network/persistence/credential implementation")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([
        sys.executable,
        str(ROOT / "scripts" / "check_sbc1g_change_control.py"),
        "--base-ref",
        BASE,
    ])
    passed("Permanent SBC-1G change-control guard passes")


def ensure_rustup_toolchain() -> None:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for SBC-5 runtime proof")
    passed("rustup available")

    check = subprocess.run(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"],
        cwd=ROOT, check=False, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if check.returncode != 0:
        run([rustup, "toolchain", "install", RUST_TOOLCHAIN, "--profile", "minimal"])
    version = run([rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"]).stdout.strip()
    if not version.startswith(f"rustc {RUST_TOOLCHAIN}"):
        fail(f"unexpected Rust toolchain: {version}")
    passed(f"Rust {RUST_TOOLCHAIN} selected")


def runtime_checks() -> None:
    ensure_rustup_toolchain()
    native_before = sha256(NATIVE_LOCK)
    product_before = sha256(PRODUCT_LOCK)

    target_dir = Path(tempfile.gettempdir()) / "sharkbooks-sbc5-target"
    if target_dir.exists():
        shutil.rmtree(target_dir, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)

    rustup = shutil.which("rustup")
    assert rustup is not None
    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(MANIFEST), "--locked",
    ], env=env)
    passed("All inherited SBC-2/SBC-3/SBC-4 plus new SBC-5 document/storage tests pass under --locked")

    if sha256(PRODUCT_LOCK) != product_before or product_before != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during SBC-5 runtime proof")
    if sha256(NATIVE_LOCK) != native_before or native_before != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during SBC-5 runtime proof")
    passed("Product and frozen native lockfiles remain unchanged through runtime proof")

    if git("status", "--short"):
        fail("repository is dirty after SBC-5 runtime proof")
    passed("Repository remains clean after runtime proof")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()

    static_checks()
    if not args.static_only:
        runtime_checks()
    print("[PASS] SBC-5 documents and storage bounded gate complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
