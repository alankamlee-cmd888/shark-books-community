#!/usr/bin/env python3
"""Fail-closed SBC-6B B3A packaging-proof preflight."""
from __future__ import annotations

import hashlib
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "7e43cb8a64097679028b1f4154172f07eadfcfa5"
PRODUCT_LOCK = ROOT / "product" / "shark-books-core" / "Cargo.lock"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_PRODUCT_LOCK = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
EXPECTED_NATIVE_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
ALLOWED = {
    "ci/run_sbc6b_b3a_windows.ps1",
    "docs/SBC6B_B3A_WINDOWS_SELF_CONTAINED_PACKAGING_PROOF_2026-09-09.md",
    "research/sbc6_ocr_runtime/requirements_b3a_packaging.txt",
    "research/sbc6_ocr_runtime/run_b3a_packaging_windows.py",
    "scripts/check_sbc6b_b3a_packaging_gate.py",
}


def fail(msg: str) -> None:
    print(f"[FAIL] {msg}")
    raise SystemExit(1)


def passed(msg: str) -> None:
    print(f"[PASS] {msg}")


def git(*args: str) -> str:
    p = subprocess.run(["git", *args], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if p.returncode:
        fail(f"git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout.strip()


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args: list[str]) -> None:
    p = subprocess.run(args, cwd=ROOT)
    if p.returncode:
        fail(f"command exited {p.returncode}: {' '.join(args)}")


def main() -> int:
    if git("status", "--short"):
        fail("repository must be clean at B3A preflight entry")
    passed("Repository clean at B3A preflight entry")

    head = git("rev-parse", "HEAD")
    passed(f"B3A candidate HEAD resolved: {head}")

    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT).returncode:
        fail("B2 protected merge is not candidate ancestor")
    passed("B2 protected merge is candidate ancestor")

    changed = {x for x in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if x}
    if changed != ALLOWED:
        fail(f"B3A diff mismatch; expected={sorted(ALLOWED)} actual={sorted(changed)}")
    passed("Candidate diff is exactly the authorised five-path B3A evidence set")

    if any(p.startswith("product/") or p.startswith("workspace/") for p in changed):
        fail("B3A must not change product or native workspace paths")
    passed("B3A contains zero product/workspace changes")

    if sha(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK:
        fail("product Cargo.lock drift")
    if sha(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK:
        fail("native Cargo.lock drift")
    passed("Product and frozen native Cargo.lock hashes remain exact")

    req = (ROOT / "research" / "sbc6_ocr_runtime" / "requirements_b3a_packaging.txt").read_text(encoding="utf-8")
    for line in [
        "pyinstaller==6.22.2",
        "onnxruntime==1.23.2",
        "numpy==2.3.5",
        "opencv-contrib-python==4.10.0.84",
        "pyclipper==1.4.0",
        "PyYAML==6.0.2",
        "psutil==7.2.2",
        "pillow==12.3.0",
    ]:
        if line not in req:
            fail(f"B3A requirements missing exact pin: {line}")
    passed("B3A packaging/proof dependency pins are exact")

    runner = (ROOT / "research" / "sbc6_ocr_runtime" / "run_b3a_packaging_windows.py").read_text(encoding="utf-8")
    for anchor in [
        "--onedir",
        "--copy-metadata",
        "restricted_env",
        'env["PATH"] = str(system32)',
        "shutil.which(name, path=env[\"PATH\"])",
        'system_root / "System32" / "where.exe"',
        "PYTHONHOME",
        "PYTHONPATH",
        "SBC6B_B3A_WINDOWS_PACKAGING.zip",
        "direct_onnx_parity.py",
        "PP-OCRv6_tiny_det_onnx",
        "PP-OCRv6_tiny_rec_onnx",
    ]:
        if anchor not in runner:
            fail(f"B3A runner missing required anchor: {anchor}")
    if 'str(system_root / "System32"), str(system_root)' in runner:
        fail("B3A restricted runtime PATH must not include the Windows root containing py.exe")
    passed("B3A runner contains self-contained/System32-only restricted-runtime proof anchors")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([sys.executable, str(ROOT / "scripts" / "check_sbc1g_change_control.py"), "--base-ref", BASE])
    passed("Permanent SBC-1G change-control guard passes")

    if git("status", "--short"):
        fail("repository became dirty during B3A preflight")
    passed("Repository remains clean after B3A preflight")
    print("[PASS] SBC-6B B3A self-contained Windows packaging preflight complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
