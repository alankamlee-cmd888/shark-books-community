#!/usr/bin/env python3
"""Fail-closed SBC-6B B1 packaging/direct-ONNX parity gate."""
from __future__ import annotations

import hashlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "a2d0c776199934d79fe1e1dce1889d3999f44b91"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
PRODUCT_LOCK = ROOT / "product" / "shark-books-core" / "Cargo.lock"
EXPECTED_NATIVE = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_PRODUCT = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
ALLOWED = {
    "ci/run_sbc6b_b1_direct_onnx_windows.ps1",
    "docs/SBC6B_B1_PACKAGING_LICENCE_AND_DIRECT_ONNX_PARITY_CONTRACT_2026-09-09.md",
    "research/sbc6_ocr_runtime/direct_onnx_parity.py",
    "scripts/check_sbc6b_b1_packaging_gate.py",
}


def fail(message: str) -> None:
    print(f"[FAIL] {message}")
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str) -> str:
    p = subprocess.run(["git", *args], cwd=ROOT, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if p.returncode != 0:
        fail(f"git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout.strip()


def main() -> int:
    if git("status", "--short"):
        fail("repository must be clean at SBC-6B B1 entry")
    passed("Repository clean at B1 entry")
    head = git("rev-parse", "HEAD")
    passed(f"B1 candidate HEAD resolved: {head}")
    anc = subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False)
    if anc.returncode != 0:
        fail("B0 protected merge is not B1 candidate ancestor")
    passed("B0 protected merge is B1 candidate ancestor")
    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED:
        fail(f"B1 candidate diff is not exact allowlist\nexpected={sorted(ALLOWED)}\nactual={sorted(changed)}")
    passed("B1 candidate diff is exactly the authorised four-path research/evidence set")
    if any(p.startswith("product/") or p.startswith("workspace/") for p in changed):
        fail("B1 parity gate may not change product/native source")
    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE:
        fail("frozen native Cargo.lock drift")
    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT:
        fail("product Cargo.lock drift")
    passed("Frozen native and product Cargo.lock hashes remain exact")

    direct_path = ROOT / "research" / "sbc6_ocr_runtime" / "direct_onnx_parity.py"
    runner_path = ROOT / "ci" / "run_sbc6b_b1_direct_onnx_windows.ps1"
    contract_path = ROOT / "docs" / "SBC6B_B1_PACKAGING_LICENCE_AND_DIRECT_ONNX_PARITY_CONTRACT_2026-09-09.md"
    for p in (direct_path, runner_path, contract_path):
        if not p.is_file():
            fail(f"required B1 file missing: {p.relative_to(ROOT)}")

    runner_bytes = runner_path.read_bytes()
    try:
        runner_text = runner_bytes.decode("ascii")
    except UnicodeDecodeError:
        fail("Windows B1 runner must remain ASCII-safe")
    for anchor in (
        "PYTHONDONTWRITEBYTECODE",
        "-B $Validator",
        "-B $Generator",
        "-B $Direct",
        "PP-OCRv6_tiny_det_onnx",
        "PP-OCRv6_tiny_rec_onnx",
        "SBC6B_B1_DIRECT_ONNX_PARITY.zip",
        "check_sbc1g_change_control.py",
        "--base-ref $Base",
    ):
        if anchor not in runner_text:
            fail(f"Windows B1 runner missing required anchor: {anchor}")
    passed("Windows B1 runner is ASCII-safe and contains required proof controls")

    source = direct_path.read_text(encoding="utf-8")
    try:
        compile(source, str(direct_path), "exec")
    except SyntaxError as exc:
        fail(f"direct parity script syntax error: {exc}")
    if re.search(r"(?m)^\s*(?:from|import)\s+(?:paddleocr|paddlex)\b", source):
        fail("direct parity script may not import PaddleOCR/PaddleX")
    for anchor in (
        'os.environ["ORT_DISABLE_TELEMETRY"] = "1"',
        '"onnxruntime": "1.23.2"',
        '"numpy": "2.3.5"',
        '"opencv-contrib-python": "4.10.0.84"',
        '"pyclipper": "1.4.0"',
        '"PyYAML": "6.0.2"',
        '"193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8"',
        '"9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6"',
        'DET_LIMIT_SIDE_LEN = 64',
        'DET_THRESH = 0.3',
        'DET_BOX_THRESH = 0.6',
        'DET_UNCLIP_RATIO = 1.5',
        'REC_IMAGE_SHAPE = (3, 48, 320)',
        'REC_BATCH_SIZE = 6',
        'socket.socket.connect_ex = blocked_connect_ex',
        'disable_telemetry_events',
        'providers=["CPUExecutionProvider"]',
        '"paddleocr" in sys.modules',
        '"paddlex" in sys.modules',
        'critical date+amount rate below 95%',
        'amount exact-pence rate below 97%',
    ):
        if anchor not in source:
            fail(f"direct parity script missing required anchor: {anchor}")
    passed("Direct parity code freezes exact selected models/runtime, privacy guards and quality gates")

    contract = contract_path.read_text(encoding="utf-8")
    for anchor in (
        "bf2efda91ce2e363e0ebdc1f3d8823f9a1d203cdc2bcd508b888dc3071cd5d22",
        "2ea11de12b6081ec5a5a3c4100aaca1ea53b2c0b16df23f846788dd6226c11c3",
        "8f8070cd228da125e7f0324ebd4fa586f6c846e44644316fcf64f816de27334c",
        "do not ship the Stage A/B0 virtual environment",
        "direct ONNX",
    ):
        if anchor.lower() not in contract.lower():
            fail(f"B1 contract missing required adjudication anchor: {anchor}")
    passed("B1 packaging contract records B0 evidence and direct-ONNX decision")
    print("[PASS] SBC-6B B1 packaging/direct-ONNX parity preflight complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
