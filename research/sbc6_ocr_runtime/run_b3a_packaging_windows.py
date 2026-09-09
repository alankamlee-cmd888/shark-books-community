#!/usr/bin/env python3
"""Build and exercise a self-contained Windows PyInstaller onedir B1 proof executable."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path

BASE = "7e43cb8a64097679028b1f4154172f07eadfcfa5"
EXPECTED_MODELS = {
    "det_onnx": "193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8",
    "det_yml": "3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593",
    "rec_onnx": "9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6",
    "rec_yml": "66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1",
}
CRITICAL_TARGET = 0.95
AMOUNT_TARGET = 0.97
CONDITION_FLOOR = 0.80
FALSE_CONFIDENT_MAX = 1
LATENCY_P95_MAX_MS = 8000.0
MEMORY_MAX_BYTES = 2 * 1024**3


def fail(msg: str) -> None:
    print(f"[FAIL] {msg}", flush=True)
    raise SystemExit(1)


def passed(msg: str) -> None:
    print(f"[PASS] {msg}", flush=True)


def run(args: list[str], *, cwd: Path, env: dict[str, str] | None = None, capture: bool = False) -> subprocess.CompletedProcess[str]:
    p = subprocess.run(args, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE if capture else None, stderr=subprocess.PIPE if capture else None)
    if capture:
        if p.stdout:
            print(p.stdout, end="" if p.stdout.endswith("\n") else "\n")
        if p.stderr:
            print(p.stderr, end="" if p.stderr.endswith("\n") else "\n", file=sys.stderr)
    if p.returncode:
        fail(f"command exited {p.returncode}: {' '.join(args)}")
    return p


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def git(repo: Path, *args: str) -> str:
    p = subprocess.run(["git", *args], cwd=repo, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if p.returncode:
        fail(f"git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout.strip()


def resolve_cached_models() -> tuple[Path, Path]:
    root = Path.home() / ".paddlex" / "official_models"
    det = root / "PP-OCRv6_tiny_det_onnx"
    rec = root / "PP-OCRv6_tiny_rec_onnx"
    if not det.is_dir() or not rec.is_dir():
        fail(f"selected B1 ONNX cache directories are unavailable under {root}")
    return det, rec


def verify_model_dir(directory: Path, prefix: str) -> dict[str, str]:
    onnx = directory / "inference.onnx"
    yml = directory / "inference.yml"
    for path in (onnx, yml):
        if not path.is_file():
            fail(f"selected model file missing: {path}")
    actual_onnx = sha256(onnx)
    actual_yml = sha256(yml)
    if actual_onnx != EXPECTED_MODELS[f"{prefix}_onnx"]:
        fail(f"{prefix} ONNX hash drift")
    if actual_yml != EXPECTED_MODELS[f"{prefix}_yml"]:
        fail(f"{prefix} config hash drift")
    return {"onnx": actual_onnx, "yml": actual_yml}


def copy_models(det: Path, rec: Path, package: Path) -> tuple[Path, Path]:
    det_out = package / "models" / "det"
    rec_out = package / "models" / "rec"
    det_out.mkdir(parents=True, exist_ok=True)
    rec_out.mkdir(parents=True, exist_ok=True)
    for src, dst in [
        (det / "inference.onnx", det_out / "inference.onnx"),
        (det / "inference.yml", det_out / "inference.yml"),
        (rec / "inference.onnx", rec_out / "inference.onnx"),
        (rec / "inference.yml", rec_out / "inference.yml"),
    ]:
        shutil.copy2(src, dst)
    verify_model_dir(det_out, "det")
    verify_model_dir(rec_out, "rec")
    passed("Package-local selected model copies retain exact B1 hashes")
    return det_out, rec_out


def restricted_env() -> dict[str, str]:
    env = os.environ.copy()
    system_root = Path(env.get("SystemRoot", r"C:\Windows"))
    env["PATH"] = os.pathsep.join([str(system_root / "System32"), str(system_root)])
    env.pop("PYTHONHOME", None)
    env.pop("PYTHONPATH", None)
    env["ORT_DISABLE_TELEMETRY"] = "1"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    return env


def assert_no_python_on_restricted_path(env: dict[str, str], repo: Path) -> None:
    for name in ("python", "py"):
        p = subprocess.run(["where.exe", name], cwd=repo, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if p.returncode == 0:
            fail(f"restricted runtime PATH unexpectedly resolves {name}: {p.stdout.strip()}")
    passed("Restricted runtime PATH resolves neither python nor py")


def tree_manifest(root: Path, output: Path) -> tuple[int, int]:
    rows: list[tuple[str, int, str]] = []
    total = 0
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        rel = path.relative_to(root).as_posix()
        size = path.stat().st_size
        total += size
        rows.append((rel, size, sha256(path)))
    with output.open("w", encoding="utf-8", newline="") as f:
        w = csv.writer(f)
        w.writerow(["path", "size_bytes", "sha256"])
        w.writerows(rows)
    return len(rows), total


def gate(summary: dict[str, object]) -> list[str]:
    reasons: list[str] = []
    if int(summary.get("count", 0)) != 48:
        reasons.append("frozen package did not produce 48 fixture results")
        return reasons
    if float(summary["critical_match_rate"]) < CRITICAL_TARGET:
        reasons.append("critical date+amount rate below 95%")
    field = summary["field_match_rate"]
    if float(field["total_pence"]) < AMOUNT_TARGET:  # type: ignore[index]
        reasons.append("exact-pence amount rate below 97%")
    bad = [name for name, data in summary["by_condition"].items() if float(data["critical_match_rate"]) < CONDITION_FLOOR]  # type: ignore[union-attr,index]
    if bad:
        reasons.append("condition critical floor failed: " + ", ".join(bad))
    if int(summary["false_confident_amount_count"]) > FALSE_CONFIDENT_MAX:
        reasons.append("too many false-confident wrong amounts")
    if float(summary["latency_ms"]["p95"]) > LATENCY_P95_MAX_MS:  # type: ignore[index]
        reasons.append("p95 latency exceeds 8 seconds")
    if int(summary["peak_rss_bytes"]) > MEMORY_MAX_BYTES:
        reasons.append("peak working set exceeds 2 GiB")
    return reasons


def main() -> int:
    if os.name != "nt":
        fail("B3A packaging proof is Windows-only")
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    args = ap.parse_args()
    repo = Path(args.repo).resolve()
    if git(repo, "status", "--short"):
        fail("repository must be clean at B3A packaging entry")
    head = git(repo, "rev-parse", "HEAD")
    passed(f"Repository clean; B3A candidate HEAD {head}")
    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=repo).returncode:
        fail("B2 protected merge is not B3A candidate ancestor")

    out_root = Path(r"C:\SharkBooks-SBC6B-B3A")
    if out_root.exists():
        shutil.rmtree(out_root, ignore_errors=True)
    out_root.mkdir(parents=True)
    build_root = Path(tempfile.gettempdir()) / "sharkbooks-sbc6b-b3a-build"
    if build_root.exists():
        shutil.rmtree(build_root, ignore_errors=True)
    build_root.mkdir(parents=True)
    venv = build_root / "venv"
    run([sys.executable, "-m", "venv", str(venv)], cwd=repo)
    py = venv / "Scripts" / "python.exe"
    pip = [str(py), "-m", "pip"]
    run(pip + ["install", "--disable-pip-version-check", "--upgrade", "pip"], cwd=repo)
    run(pip + ["install", "--disable-pip-version-check", "-r", str(repo / "research" / "sbc6_ocr_runtime" / "requirements_b3a_packaging.txt")], cwd=repo)
    passed("Isolated B3A packaging environment installed exact pins")

    fixtures = build_root / "fixtures"
    run([str(py), "-B", str(repo / "research" / "sbc6_ocr_benchmark" / "generate_receipts.py"), "--output", str(fixtures)], cwd=repo)

    dist = build_root / "dist"
    work = build_root / "work"
    spec = build_root / "spec"
    entry = repo / "research" / "sbc6_ocr_runtime" / "direct_onnx_parity.py"
    pyinstaller = [str(py), "-m", "PyInstaller"]
    cmd = pyinstaller + [
        "--noconfirm", "--clean", "--onedir", "--console",
        "--name", "shark-ocr-direct-proof",
        "--distpath", str(dist), "--workpath", str(work), "--specpath", str(spec),
        "--collect-all", "onnxruntime",
        "--collect-all", "cv2",
        "--collect-all", "pyclipper",
        "--hidden-import", "psutil",
        "--hidden-import", "PIL",
        "--collect-submodules", "PIL",
    ]
    for package in ["onnxruntime", "numpy", "opencv-contrib-python", "pyclipper", "PyYAML"]:
        cmd += ["--copy-metadata", package]
    cmd.append(str(entry))
    run(cmd, cwd=repo)
    passed("PyInstaller onedir build completed")

    built = dist / "shark-ocr-direct-proof"
    exe = built / "shark-ocr-direct-proof.exe"
    if not exe.is_file():
        fail(f"frozen executable missing: {exe}")

    package = out_root / "package"
    shutil.copytree(built, package)
    support = package / "support" / "research" / "sbc6_ocr_benchmark"
    support.mkdir(parents=True, exist_ok=True)
    shutil.copy2(repo / "research" / "sbc6_ocr_benchmark" / "run_benchmark.py", support / "run_benchmark.py")
    det_cache, rec_cache = resolve_cached_models()
    verify_model_dir(det_cache, "det")
    verify_model_dir(rec_cache, "rec")
    det_pkg, rec_pkg = copy_models(det_cache, rec_cache, package)

    manifest_path = out_root / "SBC6B_B3A_PACKAGE_MANIFEST.csv"
    file_count, package_bytes = tree_manifest(package, manifest_path)
    passed(f"Package manifest captured {file_count} files, {package_bytes} bytes")

    env = restricted_env()
    assert_no_python_on_restricted_path(env, repo)
    frozen_output = out_root / "frozen_result"
    frozen_output.mkdir()
    package_exe = package / "shark-ocr-direct-proof.exe"
    started = time.perf_counter()
    run([
        str(package_exe),
        "--repo", str(package / "support"),
        "--fixtures", str(fixtures),
        "--det-model-dir", str(det_pkg),
        "--rec-model-dir", str(rec_pkg),
        "--output", str(frozen_output),
    ], cwd=package, env=env)
    elapsed_ms = round((time.perf_counter() - started) * 1000.0, 3)

    result_path = frozen_output / "SBC6B_B1_DIRECT_ONNX_PARITY.json"
    if not result_path.is_file():
        fail("frozen executable did not produce the B1-compatible result JSON")
    result = json.loads(result_path.read_text(encoding="utf-8"))
    summary = result.get("summary") or {}
    reasons = gate(summary)
    if reasons:
        fail("frozen package parity gate failed: " + "; ".join(reasons))
    passed("Frozen executable retains the complete B1 48-receipt quality/resource gate")

    evidence = {
        "schema": "sbc6b-b3a-windows-packaging-v1",
        "git_head": head,
        "git_status": git(repo, "status", "--short"),
        "python_build": sys.version,
        "pyinstaller": subprocess.check_output([str(py), "-c", "import PyInstaller; print(PyInstaller.__version__)"], text=True).strip(),
        "restricted_path": env["PATH"],
        "system_python_resolvable_in_runtime": False,
        "package_file_count": file_count,
        "package_bytes": package_bytes,
        "executable_sha256": sha256(package_exe),
        "package_runtime_elapsed_ms": elapsed_ms,
        "model_hashes": {
            "det_onnx": sha256(det_pkg / "inference.onnx"),
            "det_yml": sha256(det_pkg / "inference.yml"),
            "rec_onnx": sha256(rec_pkg / "inference.onnx"),
            "rec_yml": sha256(rec_pkg / "inference.yml"),
        },
        "summary": summary,
        "packages": result.get("packages"),
        "pipeline_contract": result.get("pipeline_contract"),
        "model_load_ms": result.get("model_load_ms"),
        "model_load_peak_rss_bytes": result.get("model_load_peak_rss_bytes"),
        "privacy": result.get("privacy"),
        "gate_pass": True,
    }
    evidence_path = out_root / "SBC6B_B3A_WINDOWS_PACKAGING_RESULT.json"
    evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True), encoding="utf-8")
    (out_root / "GIT_HEAD.txt").write_text(head + "\n", encoding="ascii")
    (out_root / "GIT_STATUS.txt").write_text(git(repo, "status", "--short") + "\n", encoding="utf-8")

    if git(repo, "status", "--short"):
        fail("repository became dirty during B3A packaging proof")
    passed("Repository remains clean after B3A packaging proof")

    zip_path = out_root / "SBC6B_B3A_WINDOWS_PACKAGING.zip"
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for path in [evidence_path, manifest_path, out_root / "GIT_HEAD.txt", out_root / "GIT_STATUS.txt", result_path]:
            z.write(path, path.relative_to(out_root).as_posix())
    passed(f"Evidence package written: {zip_path}")
    print("[PASS] SBC-6B B3A self-contained Windows packaging proof complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
