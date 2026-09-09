#!/usr/bin/env python3
"""SBC-6B B3B Windows single-document packaged-runtime proof."""
from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from datetime import date
from pathlib import Path
from typing import Any

os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.dont_write_bytecode = True

BASE = "d304ea3093c3ed6fbc8041ac2a071f8585bc67ea"
EXPECTED_MODELS = {
    "det_onnx": "193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8",
    "det_yml": "3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593",
    "rec_onnx": "9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6",
    "rec_yml": "66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1",
}
SCHEMA = "sbc6b.single_document.v1"
MAX_SINGLE_SECONDS = 15.0
CRITICAL_TARGET = 0.95
AMOUNT_TARGET = 0.97
CONDITION_FLOOR = 0.80
FALSE_CONFIDENT_MAX = 1
LATENCY_P95_MAX_MS = 8000.0
MEMORY_MAX_BYTES = 2 * 1024**3


def fail(message: str) -> None:
    print(f"[FAIL] {message}", flush=True)
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}", flush=True)


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


def run_checked(args: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> None:
    p = subprocess.run(args, cwd=cwd, env=env)
    if p.returncode:
        fail(f"command exited {p.returncode}: {' '.join(args)}")


def build_root() -> Path:
    return Path(tempfile.gettempdir()) / "sharkbooks-sbc6b-b3b-build"


def ensure_outer_environment(repo: Path) -> None:
    root = build_root()
    if root.exists():
        shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True)
    venv = root / "venv"
    run_checked([sys.executable, "-m", "venv", str(venv)], cwd=repo)
    py = venv / "Scripts" / "python.exe"
    pip = [str(py), "-m", "pip"]
    run_checked(pip + ["install", "--disable-pip-version-check", "--upgrade", "pip"], cwd=repo)
    req = repo / "research" / "sbc6_ocr_runtime" / "requirements_b3a_packaging.txt"
    run_checked(pip + ["install", "--disable-pip-version-check", "-r", str(req)], cwd=repo)
    passed("Isolated B3B build/proof environment installed exact inherited pins")
    run_checked([str(py), "-B", str(Path(__file__).resolve()), "--repo", str(repo), "--inner"], cwd=repo)


def resolve_cached_models() -> tuple[Path, Path]:
    root = Path.home() / ".paddlex" / "official_models"
    det = root / "PP-OCRv6_tiny_det_onnx"
    rec = root / "PP-OCRv6_tiny_rec_onnx"
    if not det.is_dir() or not rec.is_dir():
        fail(f"selected B1 ONNX cache directories are unavailable under {root}")
    return det, rec


def verify_model_dir(directory: Path, prefix: str) -> None:
    onnx = directory / "inference.onnx"
    yml = directory / "inference.yml"
    if not onnx.is_file() or not yml.is_file():
        fail(f"selected model files missing under {directory}")
    if sha256(onnx) != EXPECTED_MODELS[f"{prefix}_onnx"]:
        fail(f"{prefix} ONNX hash drift")
    if sha256(yml) != EXPECTED_MODELS[f"{prefix}_yml"]:
        fail(f"{prefix} config hash drift")


def copy_models(package: Path) -> tuple[Path, Path]:
    det_cache, rec_cache = resolve_cached_models()
    verify_model_dir(det_cache, "det")
    verify_model_dir(rec_cache, "rec")
    det = package / "models" / "det"
    rec = package / "models" / "rec"
    det.mkdir(parents=True, exist_ok=True)
    rec.mkdir(parents=True, exist_ok=True)
    for src, dst in [
        (det_cache / "inference.onnx", det / "inference.onnx"),
        (det_cache / "inference.yml", det / "inference.yml"),
        (rec_cache / "inference.onnx", rec / "inference.onnx"),
        (rec_cache / "inference.yml", rec / "inference.yml"),
    ]:
        shutil.copy2(src, dst)
    verify_model_dir(det, "det")
    verify_model_dir(rec, "rec")
    passed("Package-local B3B model copies retain all four frozen hashes")
    return det, rec


def restricted_env() -> dict[str, str]:
    env = os.environ.copy()
    system_root = Path(env.get("SystemRoot", r"C:\Windows"))
    system32 = system_root / "System32"
    if not system32.is_dir():
        fail(f"System32 unavailable: {system32}")
    env["PATH"] = str(system32)
    for name in ("PYTHONHOME", "PYTHONPATH", "SHARK_OCR_MODEL_ROOT"):
        env.pop(name, None)
    env["ORT_DISABLE_TELEMETRY"] = "1"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    return env


def assert_no_system_python(env: dict[str, str]) -> None:
    system_root = Path(env.get("SystemRoot", r"C:\Windows"))
    where = system_root / "System32" / "where.exe"
    for name in ("python", "py"):
        if shutil.which(name, path=env["PATH"]) is not None:
            fail(f"restricted PATH unexpectedly resolves {name}")
        p = subprocess.run([str(where), name], env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if p.returncode == 0:
            fail(f"restricted PATH unexpectedly resolves {name}: {p.stdout.strip()}")
    passed("B3B restricted runtime PATH resolves neither python nor py")


def tree_manifest(root: Path, output: Path) -> tuple[int, int]:
    rows: list[tuple[str, int, str]] = []
    total = 0
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        rel = path.relative_to(root).as_posix()
        size = path.stat().st_size
        total += size
        rows.append((rel, size, sha256(path)))
    with output.open("w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["path", "size_bytes", "sha256"])
        writer.writerows(rows)
    return len(rows), total


def load_scoring(repo: Path) -> Any:
    path = repo / "research" / "sbc6_ocr_benchmark" / "run_benchmark.py"
    spec = importlib.util.spec_from_file_location("sbc6b_b3b_scoring", path)
    if spec is None or spec.loader is None:
        fail("could not load frozen SBC-6A scoring module")
    module = importlib.util.module_from_spec(spec)
    sys.modules["sbc6b_b3b_scoring"] = module
    spec.loader.exec_module(module)
    return module


def rss_tree(pid: int) -> int:
    import psutil

    try:
        root = psutil.Process(pid)
    except psutil.Error:
        return 0
    total = 0
    procs = [root]
    try:
        procs.extend(root.children(recursive=True))
    except psutil.Error:
        pass
    for proc in procs:
        try:
            total += int(proc.memory_info().rss)
        except psutil.Error:
            pass
    return total


def invoke_sidecar(exe: Path, input_path: Path, expected_hash: str, expected_len: int, env: dict[str, str], package: Path, *, timeout_s: float = MAX_SINGLE_SECONDS) -> tuple[int, dict[str, Any], str, float, int]:
    args = [str(exe), "--input", str(input_path), "--expected-sha256", expected_hash, "--expected-byte-len", str(expected_len)]
    started = time.perf_counter()
    proc = subprocess.Popen(args, cwd=package, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    peak = 0
    while proc.poll() is None:
        peak = max(peak, rss_tree(proc.pid))
        if time.perf_counter() - started > timeout_s:
            try:
                import psutil
                root = psutil.Process(proc.pid)
                for child in root.children(recursive=True):
                    try:
                        child.kill()
                    except psutil.Error:
                        pass
                root.kill()
            except Exception:
                proc.kill()
            _, err = proc.communicate()
            fail(f"single-document sidecar exceeded {timeout_s}s timeout; stderr={err[-2000:]}")
        time.sleep(0.02)
    peak = max(peak, rss_tree(proc.pid))
    out, err = proc.communicate()
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    text = out.strip()
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as exc:
        fail(f"sidecar stdout is not exactly one JSON object: {exc}; stdout={text[:1000]!r}; stderr={err[-2000:]!r}")
    if not isinstance(payload, dict) or payload.get("schema") != SCHEMA:
        fail(f"sidecar protocol/schema drift: {payload!r}")
    return proc.returncode, payload, err, elapsed_ms, peak


def to_scoring_prediction(payload: dict[str, Any]) -> dict[str, Any]:
    candidates = payload.get("candidates") or {}
    iso = candidates.get("document_date")
    score_date = None
    if iso is not None:
        try:
            score_date = date.fromisoformat(str(iso)).strftime("%d/%m/%Y")
        except ValueError:
            score_date = None
    return {
        "merchant": candidates.get("merchant_text"),
        "date": score_date,
        "total_pence": candidates.get("total_pence"),
        "currency": candidates.get("currency"),
        "reference": candidates.get("reference"),
        "raw_text": payload.get("raw_text", ""),
    }


def gate(summary: dict[str, Any]) -> list[str]:
    reasons: list[str] = []
    if int(summary.get("count", 0)) != 48:
        return ["sidecar did not complete all 48 frozen fixtures"]
    if float(summary["critical_match_rate"]) < CRITICAL_TARGET:
        reasons.append("critical date+amount rate below 95%")
    if float(summary["field_match_rate"]["total_pence"]) < AMOUNT_TARGET:
        reasons.append("exact-pence amount rate below 97%")
    bad = [name for name, data in summary["by_condition"].items() if float(data["critical_match_rate"]) < CONDITION_FLOOR]
    if bad:
        reasons.append("condition floor failed: " + ", ".join(bad))
    if int(summary["false_confident_amount_count"]) > FALSE_CONFIDENT_MAX:
        reasons.append("too many false-confident wrong amounts")
    if float(summary["latency_ms"]["p95"]) > LATENCY_P95_MAX_MS:
        reasons.append("sidecar p95 latency exceeds 8 seconds")
    if int(summary["peak_rss_bytes"]) > MEMORY_MAX_BYTES:
        reasons.append("sidecar process-tree peak RSS exceeds 2 GiB")
    return reasons


def main_inner(repo: Path) -> int:
    if os.name != "nt":
        fail("B3B packaged-runtime proof is Windows-only")
    if git(repo, "status", "--short"):
        fail("repository must be clean at B3B proof entry")
    head = git(repo, "rev-parse", "HEAD")
    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=repo).returncode:
        fail("B3A protected merge is not B3B candidate ancestor")
    passed(f"Repository clean; B3B candidate HEAD {head}")

    root = build_root()
    fixtures = root / "fixtures"
    run_checked([sys.executable, "-B", str(repo / "research" / "sbc6_ocr_benchmark" / "generate_receipts.py"), "--output", str(fixtures)], cwd=repo)
    manifest = json.loads((fixtures / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("schema") != "sbc6a.synthetic_receipts.v1" or len(manifest.get("fixtures", [])) != 48:
        fail("frozen receipt fixture manifest drift")

    dist = root / "dist"
    work = root / "work"
    spec = root / "spec"
    entry = repo / "product" / "ocr-runtime" / "windows" / "shark_ocr_single.py"
    cmd = [
        sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean", "--onedir", "--console",
        "--name", "shark-ocr-single", "--distpath", str(dist), "--workpath", str(work), "--specpath", str(spec),
        "--paths", str(repo / "research" / "sbc6_ocr_runtime"), "--collect-all", "onnxruntime", "--collect-all", "cv2",
        "--collect-all", "pyclipper", "--hidden-import", "yaml", "--hidden-import", "direct_onnx_parity",
    ]
    for package_name in ["onnxruntime", "numpy", "opencv-contrib-python", "pyclipper", "PyYAML"]:
        cmd += ["--copy-metadata", package_name]
    cmd.append(str(entry))
    run_checked(cmd, cwd=repo)
    passed("B3B PyInstaller onedir sidecar build completed")

    built = dist / "shark-ocr-single"
    exe = built / "shark-ocr-single.exe"
    if not exe.is_file():
        fail(f"B3B sidecar executable missing: {exe}")

    out_root = Path(r"C:\SharkBooks-SBC6B-B3B")
    if out_root.exists():
        shutil.rmtree(out_root, ignore_errors=True)
    out_root.mkdir(parents=True)
    package = out_root / "package"
    shutil.copytree(built, package)
    det, rec = copy_models(package)
    exe = package / "shark-ocr-single.exe"

    manifest_csv = out_root / "SBC6B_B3B_PACKAGE_MANIFEST.csv"
    file_count, package_bytes = tree_manifest(package, manifest_csv)
    passed(f"B3B package manifest captured {file_count} files, {package_bytes} bytes")

    env = restricted_env()
    assert_no_system_python(env)
    scoring = load_scoring(repo)

    rows: list[dict[str, Any]] = []
    per_fixture: list[dict[str, Any]] = []
    for item in manifest["fixtures"]:
        image = fixtures / item["image"]
        expected_len = image.stat().st_size
        expected_hash = sha256(image)
        code, payload, stderr, latency_ms, peak = invoke_sidecar(exe, image, expected_hash, expected_len, env, package)
        if code != 0 or payload.get("status") != "completed":
            fail(f"fixture {item['fixture_id']} did not complete: code={code} payload={payload} stderr={stderr[-1000:]}")
        if payload.get("observed_document_sha256") != expected_hash or int(payload.get("observed_byte_len", -1)) != expected_len:
            fail(f"fixture {item['fixture_id']} returned wrong document integrity identity")
        privacy = payload.get("privacy") or {}
        if not (privacy.get("ort_disable_telemetry_env") is True and privacy.get("ort_telemetry_disable_api_called") is True and privacy.get("python_tcp_connections_denied_during_runtime") is True and privacy.get("paddleocr_imported") is False and privacy.get("paddlex_imported") is False):
            fail(f"fixture {item['fixture_id']} privacy/runtime guard drift: {privacy}")

        prediction = to_scoring_prediction(payload)
        bps = payload.get("mean_ocr_confidence_bps")
        mean_conf = (float(bps) / 10_000.0) if isinstance(bps, int) else None
        scored = scoring.score_fixture(item["truth"], prediction, mean_conf)
        row = {"fixture_id": item["fixture_id"], "condition": item["condition"], "latency_ms": round(latency_ms, 3), "peak_rss_bytes": peak, "mean_confidence": mean_conf, **scored}
        rows.append(row)
        per_fixture.append({"fixture_id": item["fixture_id"], "condition": item["condition"], "return_code": code, "latency_ms": round(latency_ms, 3), "peak_rss_bytes": peak, "payload": payload, "score": scored})

    summary = scoring.summarize(rows)
    reasons = gate(summary)
    if reasons:
        fail("B3B one-document quality/resource gate failed: " + "; ".join(reasons))
    passed("All 48 one-document sidecar invocations retain the frozen quality/resource gates")

    first = fixtures / manifest["fixtures"][0]["image"]
    first_hash = sha256(first)
    first_len = first.stat().st_size

    code, payload, _, _, _ = invoke_sidecar(exe, first, first_hash, first_len + 1, env, package)
    if code != 20 or payload.get("failure_kind") != "integrity_mismatch" or payload.get("runtime_loaded") is not False:
        fail(f"wrong-length negative test did not fail before runtime load: code={code} payload={payload}")

    wrong_hash = ("0" * 64) if first_hash != ("0" * 64) else ("1" * 64)
    code, payload, _, _, _ = invoke_sidecar(exe, first, wrong_hash, first_len, env, package)
    if code != 20 or payload.get("failure_kind") != "integrity_mismatch" or payload.get("runtime_loaded") is not False:
        fail(f"wrong-hash negative test did not fail before runtime load: code={code} payload={payload}")

    non_image = root / "not-an-image.txt"
    non_image.write_text("not an image\n", encoding="utf-8")
    code, payload, _, _, _ = invoke_sidecar(exe, non_image, sha256(non_image), non_image.stat().st_size, env, package)
    if code != 21 or payload.get("failure_kind") != "unsupported_document" or payload.get("runtime_loaded") is not False:
        fail(f"unsupported-document negative test failed: code={code} payload={payload}")

    det_onnx = det / "inference.onnx"
    hidden = det / "inference.onnx.b3b-hidden"
    det_onnx.rename(hidden)
    try:
        code, payload, _, _, _ = invoke_sidecar(exe, first, first_hash, first_len, env, package)
        if code != 30 or payload.get("failure_kind") != "engine_failure":
            fail(f"engine-failure negative test failed: code={code} payload={payload}")
    finally:
        hidden.rename(det_onnx)
    verify_model_dir(det, "det")
    verify_model_dir(rec, "rec")
    passed("Integrity, unsupported-document and engine-failure negative paths fail closed")

    evidence = {
        "schema": "sbc6b-b3b-single-document-windows-v1", "git_head": head, "git_status": git(repo, "status", "--short"),
        "entry_main": BASE, "package_file_count": file_count, "package_bytes": package_bytes, "executable_sha256": sha256(exe),
        "model_hashes": EXPECTED_MODELS, "system_python_resolvable_in_runtime": False, "restricted_path": env["PATH"], "summary": summary,
        "negative_tests": {"wrong_length": "integrity_mismatch_before_runtime", "wrong_sha256": "integrity_mismatch_before_runtime", "unsupported_document": "unsupported_document_before_runtime", "engine_failure": "engine_failure"},
        "protocol": {"schema": SCHEMA, "one_document_per_process": True, "model_paths_internal": True, "stdout_single_json": True, "webview_path_surface": False},
        "gate_pass": True,
    }
    result_json = out_root / "SBC6B_B3B_SINGLE_DOCUMENT_RUNTIME_RESULT.json"
    result_json.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    fixture_json = out_root / "SBC6B_B3B_FIXTURE_RESULTS.json"
    fixture_json.write_text(json.dumps(per_fixture, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (out_root / "GIT_HEAD.txt").write_text(head + "\n", encoding="ascii")
    (out_root / "GIT_STATUS.txt").write_text(git(repo, "status", "--short") + "\n", encoding="utf-8")

    if git(repo, "status", "--short"):
        fail("repository became dirty during B3B proof")
    passed("Repository remains clean after B3B packaged-runtime proof")

    zip_path = out_root / "SBC6B_B3B_SINGLE_DOCUMENT_RUNTIME.zip"
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for path in [result_json, fixture_json, manifest_csv, out_root / "GIT_HEAD.txt", out_root / "GIT_STATUS.txt"]:
            z.write(path, path.relative_to(out_root).as_posix())
    passed(f"B3B evidence package written: {zip_path}")
    print("[PASS] SBC-6B B3B single-document local OCR runtime bounded gate complete")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--inner", action="store_true")
    args = parser.parse_args()
    repo = Path(args.repo).resolve()
    if not args.inner:
        if git(repo, "status", "--short"):
            fail("repository must be clean before B3B environment bootstrap")
        ensure_outer_environment(repo)
        return 0
    return main_inner(repo)


if __name__ == "__main__":
    raise SystemExit(main())
