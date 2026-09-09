#!/usr/bin/env python3
"""Fail-closed validator for the SBC-6A / SBC-7A research-only batch entry."""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "d20a356e374e6b7b56d4f025503896506a6027ce"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
PRODUCT_LOCK = ROOT / "product" / "shark-books-core" / "Cargo.lock"
PRODUCT_MANIFEST = ROOT / "product" / "shark-books-core" / "Cargo.toml"
BATCH = ROOT / "docs" / "SBC6_7_BOUNDED_BATCH_CONTRACT_2026-09-09.json"
OCR_CONTRACT = ROOT / "docs" / "SBC6A_OCR_RESEARCH_BENCHMARK_CONTRACT_2026-09-09.md"
UI_DRAFT = ROOT / "docs" / "SBC7_UI_INFORMATION_ARCHITECTURE_DRAFT_2026-09-09.md"
RUNNER = ROOT / "research" / "sbc6_ocr_benchmark" / "run_benchmark.py"
GENERATOR = ROOT / "research" / "sbc6_ocr_benchmark" / "generate_receipts.py"
POWERSHELL = ROOT / "research" / "sbc6_ocr_benchmark" / "run_windows.ps1"
REQUIREMENTS = ROOT / "research" / "sbc6_ocr_benchmark" / "requirements.txt"
RUST_TOOLCHAIN = "1.98.1"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"

ALLOWED_PATHS = {
    "docs/SBC6_7_BOUNDED_BATCH_CONTRACT_2026-09-09.json",
    "docs/SBC6A_OCR_RESEARCH_BENCHMARK_CONTRACT_2026-09-09.md",
    "docs/SBC7_UI_INFORMATION_ARCHITECTURE_DRAFT_2026-09-09.md",
    "research/sbc6_ocr_benchmark/generate_receipts.py",
    "research/sbc6_ocr_benchmark/requirements.txt",
    "research/sbc6_ocr_benchmark/run_benchmark.py",
    "research/sbc6_ocr_benchmark/run_windows.ps1",
    "scripts/check_sbc6a_research.py",
}

REQUIRED_OCR_ANCHORS = (
    "T0`: Tesseract 5.5.3",
    "P1`: PaddleOCR 3.7.0",
    "P2`: PaddleOCR 3.7.0",
    "48 images = 6 semantic receipt templates x 8 image conditions",
    "critical (`date + amount`) exact-match >= 95% overall",
    "amount exact-pence match >= 97% overall",
    "DEFER_OCR_V1",
    "OcrExtraction",
    "user must confirm",
)

REQUIRED_UI_ANCHORS = (
    "Home",
    "Money in",
    "Money out",
    "Bank",
    "Receipts",
    "Contacts",
    "Reports",
    "plain owner language",
    "no accounting rules in the UI",
    "SBC-8",
)


def fail(message: str) -> None:
    print(f"[FAIL] {message}")
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def run(args: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None) -> None:
    result = subprocess.run(args, cwd=cwd, env=env, check=False, text=True)
    if result.returncode != 0:
        fail(f"command exited {result.returncode}: {' '.join(args)}")


def require_anchors(path: Path, anchors: tuple[str, ...]) -> None:
    if not path.is_file():
        fail(f"required file missing: {path.relative_to(ROOT)}")
    text = path.read_text(encoding="utf-8")
    for anchor in anchors:
        if anchor not in text:
            fail(f"{path.relative_to(ROOT)} missing required anchor: {anchor}")


def static_checks() -> None:
    if git("status", "--short"):
        fail("repository must be clean at SBC-6A validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")
    ancestor = subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False)
    if ancestor.returncode != 0:
        fail("SBC-2-5 final protected merge is not candidate ancestor")
    passed("SBC-2-5 final protected merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(f"research diff does not match exact allowlist\nexpected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}")
    passed("Candidate diff is exactly the authorised eight-path research set")

    for changed_path in changed:
        if changed_path.startswith("product/") or changed_path.startswith("workspace/"):
            fail(f"research gate may not change product/native source: {changed_path}")
    passed("No product or frozen native workspace path changed")

    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("frozen native Cargo.lock hash drift")
    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock hash drift")
    passed("Native and product Cargo.lock hashes remain exact")

    manifest = PRODUCT_MANIFEST.read_text(encoding="utf-8")
    if "[dependencies]\n" not in manifest or manifest.rstrip().split("[dependencies]", 1)[1].strip():
        fail("product core must remain zero-third-party-dependency during SBC-6A")
    passed("Product core remains zero-third-party-dependency")

    require_anchors(OCR_CONTRACT, REQUIRED_OCR_ANCHORS)
    require_anchors(UI_DRAFT, REQUIRED_UI_ANCHORS)
    require_anchors(BATCH, ("SBC-6 + SBC-7 bounded batch", "SBC-8 specification only"))
    passed("SBC-6A and SBC-7A research contracts contain required frozen anchors")

    expected_requirements = {
        "paddleocr==3.7.0",
        "onnxruntime==1.23.2",
        "Pillow==12.3.0",
        "pytesseract==0.3.13",
        "psutil==7.2.2",
    }
    actual_requirements = {
        line.strip() for line in REQUIREMENTS.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    }
    if actual_requirements != expected_requirements:
        fail(f"research requirements drift: {sorted(actual_requirements)}")
    passed("Research OCR comparator versions are exactly preregistered")

    generator_text = GENERATOR.read_text(encoding="utf-8")
    for anchor in ("SEED = 608072026", '"clean"', '"rotation"', '"skew"', '"blur"', '"shadow"', '"low_contrast"', '"thermal_fade"', '"long_noisy"', "len(fixtures) != 48"):
        if anchor not in generator_text:
            fail(f"synthetic receipt generator missing anchor: {anchor}")
    runner_text = RUNNER.read_text(encoding="utf-8")
    for anchor in ("PP-OCRv6_tiny_det", "PP-OCRv6_small_det", 'engine="onnxruntime"', "verify_offline_before_documents", "critical_match_rate", "false_confident_amount"):
        if anchor not in runner_text:
            fail(f"benchmark runner missing anchor: {anchor}")
    passed("Benchmark generator/runner implement the frozen comparator shape")

    try:
        POWERSHELL.read_bytes().decode("ascii")
    except UnicodeDecodeError:
        fail("Windows benchmark harness must remain ASCII-safe for Windows PowerShell 5.1")
    passed("Windows benchmark harness is ASCII-safe")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([sys.executable, str(ROOT / "scripts" / "check_sbc1g_change_control.py"), "--base-ref", BASE])
    passed("Permanent SBC-1G change-control guard passes")


def runtime_regression() -> None:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup required for inherited product regression")
    version = subprocess.run([rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"], cwd=ROOT, check=False, text=True, stdout=subprocess.PIPE).stdout.strip()
    if not version.startswith(f"rustc {RUST_TOOLCHAIN}"):
        fail(f"Rust {RUST_TOOLCHAIN} is not available: {version}")
    passed(f"Rust {RUST_TOOLCHAIN} selected")

    before_native = sha256(NATIVE_LOCK)
    before_product = sha256(PRODUCT_LOCK)
    target = Path(tempfile.gettempdir()) / "sharkbooks-sbc6a-research-target"
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(PRODUCT_MANIFEST), "--locked",
    ], env=env)
    passed("Inherited SBC-2-5 product-core regression suite passes under --locked")

    if sha256(NATIVE_LOCK) != before_native or before_native != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock changed during research regression")
    if sha256(PRODUCT_LOCK) != before_product or before_product != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock changed during research regression")
    if git("status", "--short"):
        fail("repository became dirty during research regression")
    passed("Lockfiles and repository remain unchanged after inherited regression")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()
    static_checks()
    if not args.static_only:
        runtime_regression()
    print("[PASS] SBC-6A / SBC-7A bounded research gate preflight complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
