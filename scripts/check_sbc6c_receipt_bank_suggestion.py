#!/usr/bin/env python3
"""SBC-6C fail-closed deterministic receipt-to-bank suggestion validator."""
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
BASE = "5748b0236d3dc0f89975b9d7cb021af05f742f43"
PRODUCT = ROOT / "product" / "shark-books-core"
MANIFEST = PRODUCT / "Cargo.toml"
PRODUCT_LOCK = PRODUCT / "Cargo.lock"
ENTRY = PRODUCT / "src" / "lib_sbc3.rs"
SOURCE = PRODUCT / "src" / "receipt_bank_suggestion.rs"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
RUST_TOOLCHAIN = "1.98.1"

ALLOWED_PATHS = {
    "ci/run_sbc6c_windows.ps1",
    "docs/SBC6C_RECEIPT_BANK_SUGGESTION_IMPLEMENTATION_CONTRACT_2026-09-10.md",
    "product/shark-books-core/src/lib_sbc3.rs",
    "product/shark-books-core/src/receipt_bank_suggestion.rs",
    "scripts/check_sbc6c_receipt_bank_suggestion.py",
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
    "std::process",
    "command::new",
    "confirm_match(",
    "finalize_reconciliation(",
    "postingplan",
    "expensecategory",
    "businessuse",
)

REQUIRED_ANCHORS = (
    "pub struct BankLineIdentity",
    "pub enum ReceiptMatchReason",
    "pub struct ReceiptBankCandidate",
    "pub struct ReceiptBankSuggestionSet",
    "pub fn evaluate_receipt_bank_candidate",
    "pub fn rank_receipt_bank_suggestions",
    "pub enum ReceiptBankDecisionKind",
    "pub fn confirm_receipt_bank_suggestion",
    "pub fn reject_receipt_bank_suggestions",
    "MultipleTopCandidates",
    "requires_user_confirmation",
    "MissingTotal",
    "MissingDate",
    "MissingCurrency",
    "BankLineNotOutflow",
    "AmountMismatch",
    "CurrencyMismatch",
    "DateTooFar",
    "unique_exact_reference_date_currency_match_is_suggested",
    "multiple_exact_candidates_are_ambiguous_and_not_called_exact",
    "duplicate_looking_lines_are_not_auto_suppressed_by_receipt_matching",
    "unmatched_candidate_cannot_be_confirmed",
    "rejecting_is_explicit_and_selects_no_bank_line",
    "missing_currency_is_not_inferred_as_exact",
    "repeated_inputs_produce_identical_ranked_output",
    "minimum_i64_bank_amount_fails_closed_without_overflow",
)


def fail(message: str) -> None:
    print(f"[FAIL] {message}", flush=True)
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}", flush=True)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def run(
    args: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
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
        ["git", *args],
        cwd=ROOT,
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def static_checks() -> None:
    if git("status", "--short"):
        fail("repository must be clean at SBC-6C validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")
    if subprocess.run(
        ["git", "merge-base", "--is-ancestor", BASE, "HEAD"],
        cwd=ROOT,
        check=False,
    ).returncode != 0:
        fail("B3C protected merge is not SBC-6C candidate ancestor")
    passed("B3C protected merge is candidate ancestor")

    changed = {path for path in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if path}
    if changed != ALLOWED_PATHS:
        fail(
            "candidate diff is not exact SBC-6C allowlist\n"
            f"expected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}"
        )
    passed("Candidate diff is exactly the authorised five-path SBC-6C set")

    for path in (MANIFEST, PRODUCT_LOCK, ENTRY, SOURCE, NATIVE_LOCK):
        if not path.is_file():
            fail(f"required file missing: {path.relative_to(ROOT)}")

    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest.get("dependencies", {}) != {}:
        fail("SBC-6C product core must remain zero-third-party-dependency")
    if manifest.get("lib", {}).get("path") != "src/lib_sbc3.rs":
        fail("unexpected product-core library entry path")
    passed("Product core remains dependency-free with established library entry")

    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock byte hash drift")
    passed("Product Cargo.lock remains exact and dependency-free")
    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("frozen native Cargo.lock hash drift")
    passed("Frozen native Cargo.lock remains exact")

    entry = ENTRY.read_text(encoding="utf-8")
    if "pub mod receipt_bank_suggestion;" not in entry:
        fail("product entry does not expose bounded receipt-bank suggestion module")
    passed("Product entry exposes bounded SBC-6C module")

    source = SOURCE.read_text(encoding="utf-8")
    lowered = source.lower()
    for marker in FORBIDDEN_SOURCE_MARKERS:
        if marker.lower() in lowered:
            fail(f"SBC-6C source contains forbidden platform/accounting authority marker: {marker}")
    for anchor in REQUIRED_ANCHORS:
        if anchor not in source:
            fail(f"SBC-6C source missing required anchor: {anchor}")
    passed("Receipt-bank suggestion source contains required deterministic/user-confirmed contracts and no forbidden authority markers")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([sys.executable, str(ROOT / "scripts" / "check_sbc1g_change_control.py"), "--base-ref", BASE])
    passed("Permanent SBC-1G change-control guard passes")


def ensure_rust() -> str:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for SBC-6C runtime proof")
    passed("rustup available")
    probe = subprocess.run(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"],
        cwd=ROOT,
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
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
    target = Path(tempfile.gettempdir()) / "sharkbooks-sbc6c-target"
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(MANIFEST), "--locked",
    ], env=env)
    passed("All inherited product-core plus new SBC-6C receipt-bank suggestion tests pass under --locked")

    if sha256(PRODUCT_LOCK) != product_before or product_before != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during SBC-6C runtime proof")
    if sha256(NATIVE_LOCK) != native_before or native_before != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during SBC-6C runtime proof")
    passed("Product and frozen native lockfiles remain unchanged through runtime proof")
    if git("status", "--short"):
        fail("repository is dirty after SBC-6C runtime proof")
    passed("Repository remains clean after runtime proof")


def main() -> int:
    static_checks()
    runtime_checks()
    print("[PASS] SBC-6C deterministic receipt-to-bank suggestion bounded gate complete", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
