#!/usr/bin/env python3
"""Fail-closed SBC-6B B3B single-document runtime preflight."""
from __future__ import annotations

import ast
import hashlib
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "d304ea3093c3ed6fbc8041ac2a071f8585bc67ea"
PRODUCT_LOCK = ROOT / "product" / "shark-books-core" / "Cargo.lock"
NATIVE_LOCK = ROOT / "workspace" / "Cargo.lock"
EXPECTED_PRODUCT_LOCK = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
EXPECTED_NATIVE_LOCK = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
ALLOWED = {
    "ci/run_sbc6b_b3b_windows.ps1",
    "docs/SBC6B_B3B_SINGLE_DOCUMENT_RUNTIME_CONTRACT_2026-09-09.md",
    "product/ocr-runtime/windows/shark_ocr_single.py",
    "research/sbc6_ocr_runtime/run_b3b_single_windows.py",
    "scripts/check_sbc6b_b3b_single_runtime.py",
}


def fail(message: str) -> None:
    print(f"[FAIL] {message}")
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}")


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


def require(source: str, anchors: list[str], label: str) -> None:
    for anchor in anchors:
        if anchor not in source:
            fail(f"{label} missing required anchor: {anchor}")


def main() -> int:
    if git("status", "--short"):
        fail("repository must be clean at B3B preflight entry")
    passed("Repository clean at B3B preflight entry")

    head = git("rev-parse", "HEAD")
    passed(f"B3B candidate HEAD resolved: {head}")
    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT).returncode:
        fail("B3A protected merge is not B3B candidate ancestor")
    passed("B3A protected merge is B3B candidate ancestor")

    changed = {x for x in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if x}
    if changed != ALLOWED:
        fail(f"B3B diff mismatch; expected={sorted(ALLOWED)} actual={sorted(changed)}")
    passed("Candidate diff is exactly the authorised five-path B3B set")

    if any(path.startswith("workspace/") for path in changed):
        fail("B3B must not change the native workspace")
    if any(path.startswith("product/shark-books-core/") for path in changed):
        fail("B3B must not change the dependency-free product core")
    passed("B3B contains no native-workspace or product-core changes")

    if sha(PRODUCT_LOCK) != EXPECTED_PRODUCT_LOCK:
        fail("product Cargo.lock drift")
    if sha(NATIVE_LOCK) != EXPECTED_NATIVE_LOCK:
        fail("native Cargo.lock drift")
    passed("Product and native Cargo.lock hashes remain exact")

    sidecar_path = ROOT / "product" / "ocr-runtime" / "windows" / "shark_ocr_single.py"
    runner_path = ROOT / "research" / "sbc6_ocr_runtime" / "run_b3b_single_windows.py"
    sidecar = sidecar_path.read_text(encoding="utf-8")
    runner = runner_path.read_text(encoding="utf-8")
    ast.parse(sidecar, filename=str(sidecar_path))
    ast.parse(runner, filename=str(runner_path))
    passed("B3B Python sources parse without bytecode side effects")

    require(sidecar, [
        'SCHEMA = "sbc6b.single_document.v1"', 'MAX_INPUT_BYTES = 25 * 1024 * 1024', '"--input"',
        '"--expected-sha256"', '"--expected-byte-len"', 'hashlib.sha256(data).hexdigest()', 'integrity_mismatch',
        'unsupported_document', 'engine_failure', 'NamedTemporaryFile', 'import direct_onnx_parity as core',
        'core.deny_network()', 'disable_telemetry_events', 'models" / "det', 'models" / "rec',
        '"paddleocr_imported"', '"paddlex_imported"', '"total_pence"', '"document_date"', '"warnings"', '"regions": []',
    ], "B3B sidecar")

    for forbidden in ["--det-model-dir", "--rec-model-dir", "--output", "--repo", "tauri::", "rusqlite", "beankeeper", "http://", "https://", "access_token", "refresh_token", "api_key", "client_secret"]:
        if forbidden in sidecar:
            fail(f"B3B sidecar exposes forbidden surface: {forbidden}")
    passed("B3B sidecar exposes only bounded single-document IPC and no network/accounting/native-shell surface")

    require(runner, [
        '"--onedir"', '"shark-ocr-single"', '"--paths"', '"direct_onnx_parity"', 'System32', 'shutil.which',
        'where.exe', 'one-document', 'integrity_mismatch', 'unsupported_document', 'engine_failure',
        'SBC6B_B3B_SINGLE_DOCUMENT_RUNTIME.zip', 'LATENCY_P95_MAX_MS = 8000.0', 'MEMORY_MAX_BYTES = 2 * 1024**3',
        'len(manifest.get("fixtures", [])) != 48',
    ], "B3B proof runner")
    passed("B3B runner contains packaging, no-system-Python, 48-document, negative-path and resource-gate anchors")

    if "research/sbc6_ocr_runtime/direct_onnx_parity.py" in changed:
        fail("B3B must not modify the already-proven B1 direct-ONNX implementation")
    passed("Frozen B1 direct-ONNX implementation remains unchanged")

    run([sys.executable, "-B", str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen foundation guard passes")
    run([sys.executable, "-B", str(ROOT / "scripts" / "check_sbc1g_change_control.py"), "--base-ref", BASE])
    passed("Permanent SBC-1G change-control guard passes")

    if git("status", "--short"):
        fail("repository became dirty during B3B preflight")
    passed("Repository remains clean after B3B preflight")
    print("[PASS] SBC-6B B3B single-document local OCR runtime preflight complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
