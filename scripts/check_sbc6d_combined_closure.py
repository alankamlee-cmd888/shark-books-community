#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "ef799f5a5c3e4c1788ae7858b4865b6b3c5be496"
B3C_MERGE = "5748b0236d3dc0f89975b9d7cb021af05f742f43"
PRODUCT = ROOT / "product" / "shark-books-core"
PRODUCT_MANIFEST = PRODUCT / "Cargo.toml"
PRODUCT_LOCK = PRODUCT / "Cargo.lock"
NATIVE_MANIFEST = ROOT / "workspace" / "Cargo.toml"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
RUST_TOOLCHAIN = "1.98.1"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"

ALLOWED_PATHS = {
    "ci/run_sbc6d_windows.ps1",
    "docs/SBC6D_COMBINED_OCR_RECEIPT_FLOW_CLOSURE_CONTRACT_2026-09-10.md",
    "product/shark-books-core/tests/sbc6d_receipt_flow.rs",
    "scripts/check_sbc6d_combined_closure.py",
}

FROZEN_PRODUCT_SOURCES = [
    "product/shark-books-core/src/bank_import.rs",
    "product/shark-books-core/src/matching_reconciliation.rs",
    "product/shark-books-core/src/documents.rs",
    "product/shark-books-core/src/ocr.rs",
    "product/shark-books-core/src/receipt_bank_suggestion.rs",
    "product/shark-books-core/src/lib_sbc3.rs",
]

REQUIRED_TEST_ANCHORS = (
    "factual_ocr_to_unique_suggestion_still_requires_explicit_confirmation",
    "ambiguous_equal_candidates_never_become_an_implicit_recommendation",
    "penny_mismatch_fails_closed_and_cannot_be_confirmed",
    "missing_total_remains_missing_and_produces_no_match",
    "wrong_document_integrity_cannot_create_factual_extraction",
    "rejection_is_explicit_and_non_destructive",
)


def fail(message: str) -> None:
    print(f"[FAIL] {message}", flush=True)
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}", flush=True)


def run(args: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n", flush=True)
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr, flush=True)
    if result.returncode != 0:
        fail(f"command exited {result.returncode}: {' '.join(args)}")
    return result


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, text=True,
        encoding="utf-8", errors="replace", stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def static_checks() -> None:
    if git("status", "--short"):
        fail("repository must be clean at SBC-6D validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")
    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False).returncode != 0:
        fail("protected SBC-6C merge is not candidate ancestor")
    passed("Protected SBC-6C merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(f"candidate diff is not exact SBC-6D allowlist\nexpected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}")
    passed("Candidate diff is exactly the authorised four-path SBC-6D set")

    for path in [PRODUCT_MANIFEST, PRODUCT_LOCK, NATIVE_MANIFEST, NATIVE_LOCK]:
        if not path.is_file():
            fail(f"required file missing: {path.relative_to(ROOT)}")

    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock hash drift")
    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock hash drift")
    passed("Product and frozen native Cargo.lock hashes remain exact")

    for path in FROZEN_PRODUCT_SOURCES:
        if git("rev-parse", f"{BASE}:{path}") != git("rev-parse", f"HEAD:{path}"):
            fail(f"proven production source drifted during closure gate: {path}")
    passed("SBC-3/4/5/6B/6C production source modules are unchanged from protected SBC-6C main")

    b3c_native_tree = git("rev-parse", f"{B3C_MERGE}:workspace/shark-tauri-spike")
    current_native_tree = git("rev-parse", "HEAD:workspace/shark-tauri-spike")
    if b3c_native_tree != current_native_tree:
        fail("Tauri/native OCR tree differs from the exact Windows+Apple-proven B3C tree")
    passed(f"Tauri/native OCR tree is byte-identical to B3C proven tree {b3c_native_tree}")

    test_source = (PRODUCT / "tests" / "sbc6d_receipt_flow.rs").read_text(encoding="utf-8")
    for anchor in REQUIRED_TEST_ANCHORS:
        if anchor not in test_source:
            fail(f"combined closure regression missing anchor: {anchor}")
    passed("Combined receipt-flow regression contains every required fail-closed/confirmation scenario")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation/native-shell guard passes")
    run([sys.executable, str(ROOT / "scripts" / "check_sbc1g_change_control.py"), "--base-ref", BASE])
    passed("Permanent SBC-1G change-control guard passes")


def ensure_rust() -> str:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for SBC-6D runtime proof")
    probe = subprocess.run(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"], cwd=ROOT, check=False,
        text=True, encoding="utf-8", errors="replace", stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if probe.returncode != 0:
        run([rustup, "toolchain", "install", RUST_TOOLCHAIN, "--profile", "minimal"])
    version = run([rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"]).stdout.strip()
    if not version.startswith(f"rustc {RUST_TOOLCHAIN}"):
        fail(f"unexpected Rust toolchain: {version}")
    passed(f"Rust {RUST_TOOLCHAIN} selected")
    return rustup


def runtime_checks() -> None:
    rustup = ensure_rust()
    product_before = sha256(PRODUCT_LOCK)
    native_before = sha256(NATIVE_LOCK)

    product_target = Path(tempfile.gettempdir()) / "sharkbooks-sbc6d-product-target"
    native_target = Path(tempfile.gettempdir()) / "sharkbooks-sbc6d-native-target"
    for target in [product_target, native_target]:
        if target.exists():
            shutil.rmtree(target, ignore_errors=True)

    env_product = os.environ.copy()
    env_product["CARGO_TARGET_DIR"] = str(product_target)
    run(
        [rustup, "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(PRODUCT_MANIFEST), "--locked"],
        env=env_product,
    )
    passed("All inherited product-core tests plus SBC-6D combined receipt-flow regressions pass under --locked")

    run([sys.executable, str(ROOT / "scripts" / "bootstrap_beankeeper.py")])
    passed("Frozen Beankeeper checkout materialised at exact pinned commit")

    env_native = os.environ.copy()
    env_native["CARGO_TARGET_DIR"] = str(native_target)
    run(
        [rustup, "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(NATIVE_MANIFEST), "-p", "shark-foundation", "--locked", "--jobs", "1"],
        env=env_native,
    )
    passed("Frozen foundation tests pass")
    run(
        [rustup, "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(NATIVE_MANIFEST), "-p", "shark-tauri-spike", "--locked", "--jobs", "1", "--no-run"],
        env=env_native,
    )
    passed("Current Tauri/native OCR crate compiles under frozen lock without executing environment-bound sidecar tests")

    if sha256(PRODUCT_LOCK) != product_before or product_before != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during SBC-6D proof")
    if sha256(NATIVE_LOCK) != native_before or native_before != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during SBC-6D proof")
    passed("Product and native lockfiles remain unchanged through runtime proof")

    if git("status", "--short"):
        fail("repository is dirty after SBC-6D runtime proof")
    passed("Repository remains clean after runtime proof")


def main() -> int:
    static_checks()
    runtime_checks()
    print("[PASS] SBC-6D combined OCR/receipt-flow closure gate complete", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
