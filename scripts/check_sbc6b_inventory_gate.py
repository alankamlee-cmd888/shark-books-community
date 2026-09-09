#!/usr/bin/env python3
"""Fail-closed B0 validator for the SBC-6B P1 runtime inventory branch."""
from __future__ import annotations

import hashlib
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "ce69110dd1683ab8561b91f1dfff35df0354900d"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
PRODUCT_LOCK = ROOT / "product" / "shark-books-core" / "Cargo.lock"
EXPECTED_NATIVE = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_PRODUCT = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
ALLOWED = {
    "ci/run_sbc6b_inventory_windows.ps1",
    "docs/SBC6B_OCR_RUNTIME_ENTRY_CONTRACT_2026-09-09.md",
    "scripts/check_sbc6b_inventory_gate.py",
    "scripts/sbc6b_inventory_p1_runtime.py",
}


def fail(msg: str) -> None:
    print(f"[FAIL] {msg}")
    raise SystemExit(1)


def passed(msg: str) -> None:
    print(f"[PASS] {msg}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str) -> str:
    p = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if p.returncode != 0:
        fail(f"git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout.strip()


def main() -> int:
    if git("status", "--short"):
        fail("repository must be clean at SBC-6B B0 entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")

    anc = subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False)
    if anc.returncode != 0:
        fail("Stage A protected merge is not candidate ancestor")
    passed("Stage A protected merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED:
        fail(f"candidate diff is not exact B0 allowlist\nexpected={sorted(ALLOWED)}\nactual={sorted(changed)}")
    passed("Candidate diff is exactly the authorised four-path B0 inventory set")

    if any(p.startswith("product/") or p.startswith("workspace/") for p in changed):
        fail("B0 inventory gate may not change product/native source")

    if sha256(NATIVE_LOCK) != EXPECTED_NATIVE:
        fail("frozen native Cargo.lock drift")
    if sha256(PRODUCT_LOCK) != EXPECTED_PRODUCT:
        fail("product Cargo.lock drift")
    passed("Frozen native and product Cargo.lock hashes remain exact")

    for path in (
        ROOT / "scripts" / "sbc6b_inventory_p1_runtime.py",
        ROOT / "ci" / "run_sbc6b_inventory_windows.ps1",
        ROOT / "docs" / "SBC6B_OCR_RUNTIME_ENTRY_CONTRACT_2026-09-09.md",
    ):
        if not path.is_file():
            fail(f"missing required B0 file: {path.relative_to(ROOT)}")

    ps = (ROOT / "ci" / "run_sbc6b_inventory_windows.ps1").read_bytes()
    try:
        ps.decode("ascii")
    except UnicodeDecodeError:
        fail("Windows B0 harness must remain ASCII-safe")

    inventory = (ROOT / "scripts" / "sbc6b_inventory_p1_runtime.py").read_text(encoding="utf-8")
    for anchor in (
        '"paddleocr": "3.7.0"',
        '"onnxruntime": "1.23.2"',
        "PP-OCRv6_tiny_det",
        "PP-OCRv6_tiny_rec",
        "PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK",
        "deny_network",
        "SBC6B_P1_PYTHON_SBOM.csv",
        "SBC6B_P1_MODEL_FILES.csv",
        "SBC6B_P1_LICENSE_FILES.csv",
    ):
        if anchor not in inventory:
            fail(f"inventory script missing required anchor: {anchor}")
    passed("B0 inventory script contains selected-runtime, asset, SBOM and no-network anchors")

    print("[PASS] SBC-6B B0 runtime inventory gate preflight complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
