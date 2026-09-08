#!/usr/bin/env python3
"""SBC-4 fail-closed matching/reconciliation validator."""
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
BASE = "8d39f0c509e9c1c6f2f714ac4d9f121432d67743"
PRODUCT = ROOT / "product" / "shark-books-core"
MANIFEST = PRODUCT / "Cargo.toml"
PRODUCT_LOCK = PRODUCT / "Cargo.lock"
ENTRY = PRODUCT / "src" / "lib_sbc3.rs"
SOURCE = PRODUCT / "src" / "matching_reconciliation.rs"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
RUST_TOOLCHAIN = "1.98.1"

ALLOWED_PATHS = {
    "ci/run_sbc4_windows.ps1",
    "docs/SBC4_MATCHING_RECONCILIATION_IMPLEMENTATION_CONTRACT_2026-09-09.md",
    "product/shark-books-core/src/lib_sbc3.rs",
    "product/shark-books-core/src/matching_reconciliation.rs",
    "scripts/check_sbc4_matching_reconciliation.py",
}

FORBIDDEN_SOURCE_MARKERS = (
    "beankeeper",
    "rusqlite",
    "tauri::",
    "reqwest",
    "ureq",
    "hyper::",
    "std::fs",
    "std::net",
    "tokio::net",
    "open banking",
    "open_banking",
)

REQUIRED_ANCHORS = (
    "pub enum MatchLevel",
    "Unmatched, Possible, Likely, Exact",
    "pub enum MatchReason",
    "MultipleTopCandidates",
    "pub fn evaluate_match",
    "pub fn rank_matches",
    "requires_user_confirmation",
    "pub enum ClearanceState",
    "Uncleared, Cleared, Reconciled",
    "pub fn confirm_match",
    "pub fn preview_reconciliation",
    "pub fn finalize_reconciliation",
    "difference_minor",
    "pub struct CorrectionPlan",
    "history is not rewritten in place",
    "multiple_exact_candidates_are_not_called_exact",
    "heuristic_duplicate_identity_does_not_authorise_suppression",
    "reconciliation_zero_difference_finalizes_cleared_entries",
    "nonzero_reconciliation_difference_fails_closed",
    "already_reconciled_entry_cannot_be_reconciled_again",
)


def fail(message: str) -> None:
    print(f"[FAIL] {message}")
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, cwd=cwd, env=env, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode != 0:
        fail(f"command exited {result.returncode}: {' '.join(args)}")
    return result


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def static_checks() -> None:
    if git("status", "--short"):
        fail("repository must be clean at SBC-4 validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")
    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False).returncode != 0:
        fail("SBC-3 protected merge is not candidate ancestor")
    passed("SBC-3 protected merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(f"candidate diff is not exact SBC-4 allowlist\nexpected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}")
    passed("Candidate diff is exactly the authorised five-path SBC-4 set")

    for path in (MANIFEST, PRODUCT_LOCK, ENTRY, SOURCE, NATIVE_LOCK):
        if not path.is_file():
            fail(f"required file missing: {path.relative_to(ROOT)}")

    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest.get("dependencies", {}) != {}:
        fail("SBC-4 product core must remain zero-third-party-dependency")
    if manifest.get("lib", {}).get("path") != "src/lib_sbc3.rs":
        fail("unexpected product-core library entry path")
    passed("Product core remains dependency-free with bounded SBC-4 module entry")

    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock byte hash drift")
    passed("Product Cargo.lock remains exact and dependency-free")
    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("frozen native Cargo.lock hash drift")
    passed("Frozen native Cargo.lock remains exact")

    entry = ENTRY.read_text(encoding="utf-8")
    if '#[path = "matching_reconciliation.rs"]' not in entry or "pub mod matching;" not in entry:
        fail("product entry does not expose bounded matching/reconciliation module")

    source = SOURCE.read_text(encoding="utf-8")
    lowered = source.lower()
    for marker in FORBIDDEN_SOURCE_MARKERS:
        if marker.lower() in lowered:
            fail(f"matching source contains forbidden platform/persistence/network marker: {marker}")
    for anchor in REQUIRED_ANCHORS:
        if anchor not in source:
            fail(f"matching source missing required anchor: {anchor}")
    passed("Matching/reconciliation source contains required contracts and no forbidden platform/network persistence markers")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([sys.executable, str(ROOT / "scripts" / "check_sbc1g_change_control.py"), "--base-ref", BASE])
    passed("Permanent SBC-1G change-control guard passes")


def ensure_rust() -> str:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for SBC-4 runtime proof")
    passed("rustup available")
    probe = subprocess.run([rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"], cwd=ROOT, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
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
    target = Path(tempfile.gettempdir()) / "sharkbooks-sbc4-target"
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    run([rustup, "run", RUST_TOOLCHAIN, "cargo", "test", "--manifest-path", str(MANIFEST), "--locked"], env=env)
    passed("All inherited SBC-2/SBC-3 plus new SBC-4 matching/reconciliation tests pass under --locked")

    if sha256(PRODUCT_LOCK) != product_before or product_before != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during SBC-4 runtime proof")
    if sha256(NATIVE_LOCK) != native_before or native_before != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during SBC-4 runtime proof")
    passed("Product and frozen native lockfiles remain unchanged through runtime proof")
    if git("status", "--short"):
        fail("repository is dirty after SBC-4 runtime proof")
    passed("Repository remains clean after runtime proof")


def main() -> int:
    static_checks()
    runtime_checks()
    print("[PASS] SBC-4 matching and reconciliation bounded gate complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
