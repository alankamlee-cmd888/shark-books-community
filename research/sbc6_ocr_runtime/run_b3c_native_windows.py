#!/usr/bin/env python3
"""SBC-6B B3C Windows native/Tauri integration proof."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.dont_write_bytecode = True

BASE = "376301ef8b23f7c393a332845737f51b27624603"
RUST_TOOLCHAIN = "1.98.1"
EXPECTED_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_MODELS = {
    "det_onnx": "193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8",
    "det_yml": "3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593",
    "rec_onnx": "9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6",
    "rec_yml": "66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1",
}


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
    p = subprocess.run(
        ["git", *args], cwd=repo, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if p.returncode:
        fail(f"git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout.strip()


def run_checked(
    args: list[str], *, cwd: Path, env: dict[str, str] | None = None,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    p = subprocess.run(
        args, cwd=cwd, env=env,
        text=True if capture else None,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        check=False,
    )
    if p.returncode:
        if capture:
            print(p.stdout or "", end="", flush=True)
            print(p.stderr or "", end="", file=sys.stderr, flush=True)
        fail(f"command exited {p.returncode}: {' '.join(args)}")
    return p


def build_root() -> Path:
    return Path(tempfile.gettempdir()) / "sharkbooks-sbc6b-b3c-build"


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
    passed("Isolated B3C build/proof environment installed exact inherited OCR pins")
    run_checked(
        [str(py), "-B", str(Path(__file__).resolve()), "--repo", str(repo), "--inner"],
        cwd=repo,
    )


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


def copy_models(package: Path) -> None:
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
    passed("B3C package-local OCR models retain all four frozen hashes")


def build_real_sidecar(repo: Path, root: Path) -> Path:
    dist = root / "dist"
    work = root / "pyinstaller-work"
    spec = root / "pyinstaller-spec"
    entry = repo / "product" / "ocr-runtime" / "windows" / "shark_ocr_single.py"
    cmd = [
        sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean", "--onedir", "--console",
        "--name", "shark-ocr-single", "--distpath", str(dist), "--workpath", str(work),
        "--specpath", str(spec), "--paths", str(repo / "research" / "sbc6_ocr_runtime"),
        "--collect-all", "onnxruntime", "--collect-all", "cv2", "--collect-all", "pyclipper",
        "--hidden-import", "yaml", "--hidden-import", "direct_onnx_parity",
    ]
    for package_name in ["onnxruntime", "numpy", "opencv-contrib-python", "pyclipper", "PyYAML"]:
        cmd += ["--copy-metadata", package_name]
    cmd.append(str(entry))
    run_checked(cmd, cwd=repo)
    built = dist / "shark-ocr-single"
    exe = built / "shark-ocr-single.exe"
    if not exe.is_file():
        fail(f"B3C real sidecar executable missing: {exe}")
    package = root / "real-package"
    shutil.copytree(built, package)
    copy_models(package)
    exe = package / "shark-ocr-single.exe"
    passed(f"Real B3B sidecar rebuilt for B3C host proof: {sha256(exe)}")
    return exe


FAKE_RUST = r"""
use std::{env, io::{self, Write}, path::Path, process, thread, time::Duration};

const SCHEMA: &str = "sbc6b.single_document.v1";

fn input_name() -> String {
    let args: Vec<String> = env::args().collect();
    let index = args.iter().position(|v| v == "--input").expect("--input") + 1;
    Path::new(&args[index]).file_name().unwrap().to_string_lossy().to_string()
}

fn main() {
    match input_name().as_str() {
        "engine-failure.png" => {
            println!("{{\"schema\":\"{}\",\"status\":\"failed\",\"failure_kind\":\"engine_failure\",\"detail\":\"bounded fake failure\",\"runtime_loaded\":false}}", SCHEMA);
            process::exit(30);
        }
        "malformed.png" => println!("this is not json"),
        "wrong-schema.png" => println!("{{\"schema\":\"wrong\",\"status\":\"completed\"}}"),
        "stdout-overflow.png" => {
            let mut out = io::stdout().lock();
            let block = vec![b'X'; 1_100_000];
            out.write_all(&block).unwrap();
            out.flush().unwrap();
        }
        "stderr-overflow.png" => {
            let mut err = io::stderr().lock();
            let block = vec![b'E'; 300_000];
            err.write_all(&block).unwrap();
            err.flush().unwrap();
            println!("{{\"schema\":\"{}\",\"status\":\"failed\",\"failure_kind\":\"engine_failure\",\"detail\":\"after overflow\",\"runtime_loaded\":false}}", SCHEMA);
            process::exit(30);
        }
        "timeout.png" => {
            thread::sleep(Duration::from_secs(5));
            println!("{{\"schema\":\"{}\",\"status\":\"failed\",\"failure_kind\":\"engine_failure\",\"detail\":\"late\",\"runtime_loaded\":false}}", SCHEMA);
            process::exit(30);
        }
        _ => {
            println!("{{\"schema\":\"{}\",\"status\":\"failed\",\"failure_kind\":\"engine_failure\",\"detail\":\"unexpected fake case\",\"runtime_loaded\":false}}", SCHEMA);
            process::exit(30);
        }
    }
}
"""


def build_fake_sidecar(repo: Path, root: Path) -> tuple[Path, Path]:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required to build B3C fake sidecar")
    fake_root = root / "fake-package"
    fake_root.mkdir(parents=True)
    source = fake_root / "fake_sidecar.rs"
    source.write_text(FAKE_RUST, encoding="utf-8")
    exe = fake_root / "fake-sidecar.exe"
    run_checked(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--edition=2024", "-O", str(source), "-o", str(exe)],
        cwd=repo,
    )
    if not exe.is_file():
        fail("fake B3C sidecar did not compile")
    for relative in [
        "models/det/inference.onnx", "models/det/inference.yml",
        "models/rec/inference.onnx", "models/rec/inference.yml",
    ]:
        target = fake_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"B3C fake model sentinel\n")
    input_root = root / "fake-inputs"
    input_root.mkdir(parents=True)
    passed("Bounded fake sidecar compiled for timeout/malformed/output-limit tests")
    return exe, input_root


def select_fixture(repo: Path, root: Path) -> Path:
    fixtures = root / "fixtures"
    run_checked(
        [sys.executable, "-B", str(repo / "research" / "sbc6_ocr_benchmark" / "generate_receipts.py"), "--output", str(fixtures)],
        cwd=repo,
    )
    manifest = json.loads((fixtures / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("schema") != "sbc6a.synthetic_receipts.v1" or len(manifest.get("fixtures", [])) != 48:
        fail("frozen receipt fixture manifest drift")
    first = fixtures / manifest["fixtures"][0]["image"]
    if not first.is_file():
        fail("first frozen receipt fixture missing")
    passed(f"Selected frozen receipt fixture {manifest['fixtures'][0]['fixture_id']} for native host proof")
    return first


def run_validator(repo: Path, env: dict[str, str], mode: str) -> tuple[str, str]:
    validator = repo / "scripts" / "check_sbc6b_b3c_native_gate.py"
    p = subprocess.run(
        [sys.executable, str(validator), mode], cwd=repo, env=env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    print(p.stdout, end="" if p.stdout.endswith("\n") else "\n", flush=True)
    if p.stderr:
        print(p.stderr, end="" if p.stderr.endswith("\n") else "\n", file=sys.stderr, flush=True)
    if p.returncode:
        fail(f"B3C validator {mode} failed with exit {p.returncode}")
    return p.stdout, p.stderr


def main_inner(repo: Path) -> int:
    if os.name != "nt":
        fail("B3C native/Tauri packaged-runtime proof is Windows-only")
    if git(repo, "status", "--short"):
        fail("repository must be clean at B3C proof entry")
    head = git(repo, "rev-parse", "HEAD")
    if subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=repo, check=False).returncode:
        fail("B3B protected merge is not B3C candidate ancestor")
    passed(f"Repository clean; B3C candidate HEAD {head}")

    root = build_root()
    static_out, static_err = run_validator(repo, os.environ.copy(), "--static-only")
    receipt = select_fixture(repo, root)
    real_sidecar = build_real_sidecar(repo, root)
    fake_sidecar, fake_inputs = build_fake_sidecar(repo, root)

    env = os.environ.copy()
    env["SHARK_SBC6B_B3C_REAL_SIDECAR"] = str(real_sidecar)
    env["SHARK_SBC6B_B3C_REAL_RECEIPT"] = str(receipt)
    env["SHARK_SBC6B_B3C_REAL_SHA256"] = sha256(receipt)
    env["SHARK_SBC6B_B3C_REAL_LEN"] = str(receipt.stat().st_size)
    env["SHARK_SBC6B_B3C_FAKE_SIDECAR"] = str(fake_sidecar)
    env["SHARK_SBC6B_B3C_FAKE_ROOT"] = str(fake_inputs)
    runtime_out, runtime_err = run_validator(repo, env, "--runtime-only")

    lock = repo / "workspace" / "Cargo.lock"
    if sha256(lock) != EXPECTED_LOCK_SHA256:
        fail("native Cargo.lock drift after B3C runtime proof")
    if git(repo, "status", "--short"):
        fail("repository became dirty during B3C proof")
    passed("Repository and reviewed native lock remain unchanged after B3C proof")

    out_root = Path(r"C:\SharkBooks-SBC6B-B3C")
    if out_root.exists():
        shutil.rmtree(out_root, ignore_errors=True)
    out_root.mkdir(parents=True)
    result = {
        "schema": "sbc6b-b3c-native-tauri-windows-v1",
        "gate": "SBC-6B B3C",
        "gate_pass": True,
        "entry_protected_main": BASE,
        "git_head": head,
        "git_status": git(repo, "status", "--short"),
        "native_cargo_lock_sha256": sha256(lock),
        "real_sidecar_sha256": sha256(real_sidecar),
        "receipt_sha256": sha256(receipt),
        "receipt_byte_len": receipt.stat().st_size,
        "model_hashes": EXPECTED_MODELS,
        "proofs": {
            "webview_opaque_ids_only": True,
            "fixed_native_process": True,
            "real_packaged_sidecar_positive": True,
            "wrong_length_integrity_fail": True,
            "wrong_hash_integrity_fail": True,
            "missing_sidecar_unavailable": True,
            "nonzero_exit_typed": True,
            "malformed_json_fail": True,
            "wrong_schema_fail": True,
            "stdout_limit_fail": True,
            "stderr_limit_fail": True,
            "rust_owned_timeout_and_kill": True,
            "no_network_or_cloud_ocr_path": True,
            "windows_tauri_tests_locked": True,
            "windows_tauri_build_locked": True,
        },
        "apple_proof": "separate GitHub Actions B3C cross-platform workflow required before merge",
    }
    result_path = out_root / "SBC6B_B3C_NATIVE_TAURI_INTEGRATION_RESULT.json"
    result_path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (out_root / "STATIC_VALIDATOR.stdout.txt").write_text(static_out, encoding="utf-8")
    (out_root / "STATIC_VALIDATOR.stderr.txt").write_text(static_err, encoding="utf-8")
    (out_root / "RUNTIME_VALIDATOR.stdout.txt").write_text(runtime_out, encoding="utf-8")
    (out_root / "RUNTIME_VALIDATOR.stderr.txt").write_text(runtime_err, encoding="utf-8")
    (out_root / "GIT_HEAD.txt").write_text(head + "\n", encoding="ascii")
    (out_root / "GIT_STATUS.txt").write_text(git(repo, "status", "--short") + "\n", encoding="utf-8")

    zip_path = out_root / "SBC6B_B3C_NATIVE_TAURI_INTEGRATION.zip"
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for path in sorted(p for p in out_root.iterdir() if p.is_file() and p != zip_path):
            z.write(path, path.name)
    passed(f"B3C Windows evidence package written: {zip_path}")
    print("[PASS] SBC-6B B3C native/Tauri integration Windows bounded proof complete")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--inner", action="store_true")
    args = parser.parse_args()
    repo = Path(args.repo).resolve()
    if not args.inner:
        if git(repo, "status", "--short"):
            fail("repository must be clean before B3C environment bootstrap")
        ensure_outer_environment(repo)
        return 0
    return main_inner(repo)


if __name__ == "__main__":
    raise SystemExit(main())
