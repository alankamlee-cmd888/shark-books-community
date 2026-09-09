#!/usr/bin/env python3
"""Fail-closed SBC-6B B2 implementation-neutral OCR contract validator."""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "c8417665516476e77468c5815aa33b8c69787aa9"
PRODUCT = ROOT / "product" / "shark-books-core"
MANIFEST = PRODUCT / "Cargo.toml"
PRODUCT_LOCK = PRODUCT / "Cargo.lock"
ENTRY = PRODUCT / "src" / "lib_sbc3.rs"
SOURCE = PRODUCT / "src" / "ocr.rs"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
RUST_TOOLCHAIN = "1.98.1"
EXPECTED_NEW_TESTS = 18

ALLOWED_PATHS = {
    "product/shark-books-core/src/ocr.rs",
    "product/shark-books-core/src/lib_sbc3.rs",
    "docs/SBC6B_B2_IMPLEMENTATION_NEUTRAL_OCR_CONTRACT_2026-09-09.md",
    "scripts/check_sbc6b_b2_ocr_contract.py",
    "ci/run_sbc6b_b2_windows.ps1",
}

FORBIDDEN_IMPLEMENTATION_MARKERS = (
    "std::fs",
    "std::path",
    "std::net",
    "std::process",
    "command::new",
    "tauri::",
    "rusqlite",
    "beankeeper",
    "reqwest",
    "ureq",
    "tokio::",
    "serde_json",
    "expensecategory",
    "incomecategory",
    "postingplan",
    "confirm_match",
    "finalize_reconciliation",
)

REQUIRED_SOURCE_ANCHORS = (
    "pub const OCR_CONTRACT_VERSION",
    "pub struct OcrDocumentIdentity",
    "pub struct OcrRequest",
    "pub fn for_receipt_document",
    "pub struct OcrEngineProvenance",
    "pub struct OcrConfidenceBps",
    "pub struct OcrTextRegion",
    "pub struct OcrReceiptCandidates",
    "pub struct OcrExtraction",
    "pub fn from_adapter_output",
    "pub enum OcrUnavailableReason",
    "pub enum OcrFailureKind",
    "pub enum OcrOutcome",
    "DocumentMismatch",
    "OCR_MAX_RAW_TEXT_BYTES",
    "OCR_MAX_REGIONS",
    "OCR_MAX_WARNINGS",
    "missing_facts_remain_missing_instead_of_being_inferred",
    "request_preserves_only_document_integrity_identity",
    "mismatched_adapter_document_hash_fails_closed",
    "unavailable_and_failed_are_explicit_noncompleted_outcomes",
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
        fail("repository must be clean at SBC-6B B2 entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")

    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False
    )
    if ancestor.returncode != 0:
        fail("B1 protected merge is not candidate ancestor")
    passed("B1 protected merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(
            "candidate diff is not exact SBC-6B B2 allowlist\n"
            f"expected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}"
        )
    passed("Candidate diff is exactly the authorised five-path B2 set")

    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest.get("dependencies", {}) != {}:
        fail("B2 product core must remain zero-third-party-dependency")
    if manifest.get("lib", {}).get("path") != "src/lib_sbc3.rs":
        fail("unexpected product-core library entry point")
    passed("Product core remains dependency-free")

    lock = tomllib.loads(PRODUCT_LOCK.read_text(encoding="utf-8"))
    packages = lock.get("package", [])
    if len(packages) != 1 or packages[0].get("name") != "shark-books-core" or "dependencies" in packages[0]:
        fail("product Cargo.lock must remain single-package and dependency-free")
    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock byte hash drift")
    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("frozen native Cargo.lock byte hash drift")
    passed("Product and frozen native Cargo.lock hashes remain exact")

    entry = ENTRY.read_text(encoding="utf-8")
    if "pub mod ocr;" not in entry:
        fail("product entry point does not expose bounded OCR contract module")

    source = SOURCE.read_text(encoding="utf-8")
    lowered = source.lower()
    for marker in FORBIDDEN_IMPLEMENTATION_MARKERS:
        if marker.lower() in lowered:
            fail(f"OCR core contains forbidden implementation/accounting-action marker: {marker}")
    for anchor in REQUIRED_SOURCE_ANCHORS:
        if anchor not in source:
            fail(f"OCR core missing required contract anchor: {anchor}")
    if source.count("#[test]") != EXPECTED_NEW_TESTS:
        fail(f"expected exactly {EXPECTED_NEW_TESTS} B2 OCR tests, found {source.count('#[test]')}")
    passed("OCR source is bounded to factual DTO/integrity/outcome contracts with exactly 18 new tests")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([
        sys.executable,
        str(ROOT / "scripts" / "check_sbc1g_change_control.py"),
        "--base-ref", BASE,
    ])
    passed("Permanent SBC-1G change-control guard passes")


def ensure_rustup_toolchain() -> str:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for SBC-6B B2 runtime proof")
    passed("rustup available")
    probe = subprocess.run(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"], cwd=ROOT,
        check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if probe.returncode != 0:
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

    target_dir = Path(tempfile.gettempdir()) / "sharkbooks-sbc6b-b2-target"
    if target_dir.exists():
        shutil.rmtree(target_dir, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)

    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(MANIFEST), "--locked",
    ], env=env)
    passed("All inherited 74 product-core tests plus 18 B2 OCR-contract tests pass under --locked")

    if sha256(PRODUCT_LOCK) != product_before or product_before != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during B2 runtime proof")
    if sha256(NATIVE_LOCK) != native_before or native_before != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during B2 runtime proof")
    passed("Product and frozen native lockfiles remain unchanged through B2 proof")

    if git("status", "--short"):
        fail("repository became dirty during SBC-6B B2 proof")
    passed("Repository remains clean after B2 runtime proof")


def main() -> int:
    static_checks()
    runtime_checks()
    print("[PASS] SBC-6B B2 implementation-neutral OCR contract bounded gate complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
