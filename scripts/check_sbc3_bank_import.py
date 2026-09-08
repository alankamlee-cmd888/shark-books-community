#!/usr/bin/env python3
"""SBC-3 fail-closed bank-import validator."""
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
BASE = "99c41ce60d0fd8807be653a1c3e6d9de08064d5a"
PRODUCT = ROOT / "product" / "shark-books-core"
MANIFEST = PRODUCT / "Cargo.toml"
PRODUCT_LOCK = PRODUCT / "Cargo.lock"
ENTRY_SOURCE = PRODUCT / "src" / "lib_sbc3.rs"
IMPORT_SOURCE = PRODUCT / "src" / "bank_import.rs"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
RUST_TOOLCHAIN = "1.98.1"

ALLOWED_PATHS = {
    "ci/run_sbc3_windows.ps1",
    "docs/SBC3_BANK_IMPORT_IMPLEMENTATION_CONTRACT_2026-09-08.md",
    "product/shark-books-core/Cargo.toml",
    "product/shark-books-core/src/bank_import.rs",
    "product/shark-books-core/src/lib_sbc3.rs",
    "scripts/check_sbc3_bank_import.py",
}

REQUIRED_IMPORT_ANCHORS = (
    "pub struct BankLine",
    "pub struct CsvMappingProfile",
    "pub fn preview_csv",
    "pub fn preview_ofx_or_qfx",
    "pub enum DuplicateCertainty",
    "Strong",
    "FileExact",
    "Heuristic",
    "pub fn strong_identity_key",
    "pub fn duplicate_certainty",
    "pub fn sha256_hex",
    "sha256_known_vectors",
    "ambiguous_debit_credit_row_fails_closed",
    "strong_identity_conflict_is_an_error",
    "heuristic_similarity_never_authorises_auto_suppression",
    "ofx_preview_creates_canonical_lines_and_fitid_identity",
    "qfx_uses_same_ofx_strong_identity_namespace",
)

FORBIDDEN_IMPORT_MARKERS = (
    "beankeeper::",
    "rusqlite",
    "tauri::",
    "reqwest",
    "ureq",
    "hyper::",
    "std::fs",
    "std::net",
    "tokio::net",
    "TcpStream",
    "OpenBanking",
    "open_banking",
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
        args, cwd=cwd, env=env, check=False, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
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
        fail("repository must be clean at SBC-3 validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")

    ancestor = subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False)
    if ancestor.returncode != 0:
        fail("SBC-2 protected merge is not an ancestor of candidate")
    passed("SBC-2 protected merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(
            "candidate diff is not exact SBC-3 allowlist\n"
            f"expected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}"
        )
    passed("Candidate diff is exactly the authorised six-path SBC-3 set")

    for path in (MANIFEST, PRODUCT_LOCK, ENTRY_SOURCE, IMPORT_SOURCE, NATIVE_LOCK):
        if not path.is_file():
            fail(f"required file missing: {path.relative_to(ROOT)}")

    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    package = manifest.get("package", {})
    if package.get("name") != "shark-books-core" or package.get("version") != "0.0.1":
        fail("unexpected shark-books-core package identity")
    if manifest.get("dependencies", {}) != {}:
        fail("SBC-3 product core must still have zero third-party dependencies")
    if manifest.get("lib", {}).get("path") != "src/lib_sbc3.rs":
        fail("SBC-3 product core must enter through src/lib_sbc3.rs")
    passed("SBC-3 product core remains dependency-free with bounded import entry point")

    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock drifted from SBC-2 dependency-free lock")
    lock = tomllib.loads(PRODUCT_LOCK.read_text(encoding="utf-8"))
    packages = lock.get("package", [])
    if len(packages) != 1 or packages[0].get("name") != "shark-books-core":
        fail("product Cargo.lock unexpectedly gained packages")
    passed("Product Cargo.lock remains exact and dependency-free")

    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("frozen native Cargo.lock hash drift")
    passed("Frozen native Cargo.lock remains exact")

    entry = ENTRY_SOURCE.read_text(encoding="utf-8")
    if "pub mod bank_import;" not in entry or "pub use domain::*;" not in entry:
        fail("SBC-3 entry point does not preserve/re-export SBC-2 domain and bank import module")

    source = IMPORT_SOURCE.read_text(encoding="utf-8")
    for marker in FORBIDDEN_IMPORT_MARKERS:
        if marker in source:
            fail(f"bank-import core contains forbidden platform/persistence/network marker: {marker}")
    for anchor in REQUIRED_IMPORT_ANCHORS:
        if anchor not in source:
            fail(f"bank-import core missing required anchor: {anchor}")
    passed("Bank import source contains required contracts and no forbidden platform/network persistence markers")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([
        sys.executable, str(ROOT / "scripts" / "check_sbc1g_change_control.py"),
        "--base-ref", BASE,
    ])
    passed("Permanent SBC-1G change-control guard passes")


def ensure_rustup_toolchain() -> str:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for SBC-3 runtime proof")
    passed("rustup available")
    check = subprocess.run(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"], cwd=ROOT, check=False,
        text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if check.returncode != 0:
        run([rustup, "toolchain", "install", RUST_TOOLCHAIN, "--profile", "minimal"])
    version = run([rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"]).stdout.strip()
    if not version.startswith(f"rustc {RUST_TOOLCHAIN}"):
        fail(f"unexpected Rust toolchain: {version}")
    passed(f"Rust {RUST_TOOLCHAIN} selected")
    return rustup


def runtime_checks() -> None:
    rustup = ensure_rustup_toolchain()
    native_before = sha256(NATIVE_LOCK)
    product_before = sha256(PRODUCT_LOCK)
    target_dir = Path(tempfile.gettempdir()) / "sharkbooks-sbc3-target"
    if target_dir.exists():
        shutil.rmtree(target_dir, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)

    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(MANIFEST), "--locked",
    ], env=env)
    passed("All inherited SBC-2 and new SBC-3 bank-import tests pass under --locked")

    if sha256(PRODUCT_LOCK) != product_before or product_before != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during SBC-3 proof")
    if sha256(NATIVE_LOCK) != native_before or native_before != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during SBC-3 proof")
    passed("Product and frozen native lockfiles remain unchanged through runtime proof")

    if git("status", "--short"):
        fail("repository is dirty after SBC-3 runtime proof")
    passed("Repository remains clean after runtime proof")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()
    static_checks()
    if not args.static_only:
        runtime_checks()
    print("[PASS] SBC-3 bank import bounded gate complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
