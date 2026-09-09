#!/usr/bin/env python3
"""Fail-closed SBC-6B B3C native/Tauri OCR integration validator."""
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
BASE = "376301ef8b23f7c393a332845737f51b27624603"
RUST_TOOLCHAIN = "1.98.1"
LOCK = ROOT / "workspace" / "Cargo.lock"
LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
TAURI_SOURCE = ROOT / "workspace" / "shark-tauri-spike" / "src" / "lib.rs"
OCR_SOURCE = ROOT / "workspace" / "shark-tauri-spike" / "src" / "ocr_native.rs"
BUILD_RS = ROOT / "workspace" / "shark-tauri-spike" / "build.rs"
PERMISSION = ROOT / "workspace" / "shark-tauri-spike" / "permissions" / "shark-shell.toml"
CAPABILITY = ROOT / "workspace" / "shark-tauri-spike" / "capabilities" / "default.json"

ALLOWED_PATHS = {
    ".github/workflows/sbc6b-b3c-gate.yml",
    "ci/run_sbc6b_b3c_codemagic.sh",
    "ci/run_sbc6b_b3c_windows.ps1",
    "codemagic.yaml",
    "docs/SBC6B_B3C_NATIVE_TAURI_INTEGRATION_PLAN_2026-09-09.md",
    "research/sbc6_ocr_runtime/run_b3c_native_windows.py",
    "scripts/check_frozen_baseline.py",
    "scripts/check_sbc6b_b3c_native_gate.py",
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/permissions/shark-shell.toml",
    "workspace/shark-tauri-spike/src/lib.rs",
    "workspace/shark-tauri-spike/src/ocr_native.rs",
}
UNCHANGED_PATHS = (
    "workspace/Cargo.toml",
    "workspace/Cargo.lock",
    "workspace/shark-tauri-spike/Cargo.toml",
    "workspace/shark-tauri-spike/tauri.conf.json",
    "workspace/shark-tauri-spike/capabilities/default.json",
    "workspace/shark-foundation/Cargo.toml",
    "workspace/shark-foundation/src/lib.rs",
    "product/shark-books-core/Cargo.toml",
    "product/shark-books-core/src/ocr.rs",
    "product/shark-books-core/src/documents.rs",
    "product/ocr-runtime/windows/shark_ocr_single.py",
)
REQUIRED_OCR_ANCHORS = (
    'const SIDECAR_SCHEMA: &str = "sbc6b.single_document.v1"',
    "pub(crate) struct NativeOcrRegistry",
    "pub(crate) struct OcrReceiptCommandRequest",
    "request_id: String",
    "document_id: String",
    "Command::new(sidecar)",
    '.arg("--input")',
    '.arg("--expected-sha256")',
    '.arg("--expected-byte-len")',
    "MAX_STDOUT_BYTES",
    "MAX_STDERR_BYTES",
    "SIDECAR_TIMEOUT",
    "child.kill()",
    "parse_exact_json",
    "de.end()",
    "ShellOcrFailureKind::IntegrityMismatch",
    "ShellOcrFailureKind::Timeout",
    "ShellOcrFailureKind::MalformedOutput",
    "ShellOcrFailureKind::EngineFailure",
    "ShellOcrFailureKind::ResourceLimit",
    "ShellOcrUnavailableReason::NotInstalled",
    "ShellOcrUnavailableReason::ModelsUnavailable",
    "ShellOcrUnavailableReason::UnsupportedPlatform",
    "python_tcp_connections_denied_during_runtime",
    "paddleocr_imported",
    "paddlex_imported",
    "webview_request_accepts_only_opaque_ids",
    "windows_wrong_hash_is_rejected_by_verified_sidecar_before_ocr",
    "windows_timeout_is_owned_by_rust_and_child_is_terminated",
)
FORBIDDEN_NATIVE_MARKERS = (
    "std::net", "reqwest", "ureq", "tokio::net",
    'Command::new("cmd', 'Command::new("powershell', 'Command::new("sh',
    '.arg("/C")', '.arg("-c")',
    "http://", "https://",
    "confirm_match", "finalize_reconciliation",
    "PostingPlan", "ExpenseCategory", "IncomeCategory",
)


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


def git(*args: str) -> str:
    p = subprocess.run(
        ["git", *args], cwd=ROOT, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if p.returncode:
        fail(f"git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout.strip()


def run(args: list[str], *, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    p = subprocess.run(
        args, cwd=ROOT, env=env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if p.stdout:
        print(p.stdout, end="" if p.stdout.endswith("\n") else "\n")
    if p.stderr:
        print(p.stderr, end="" if p.stderr.endswith("\n") else "\n", file=sys.stderr)
    if p.returncode:
        fail(f"command exited {p.returncode}: {' '.join(args)}")
    return p


def static_checks() -> None:
    if git("status", "--short"):
        fail("repository must be clean at B3C proof entry")
    passed("Repository clean at B3C proof entry")

    head = git("rev-parse", "HEAD")
    if subprocess.run(
        ["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=ROOT, check=False
    ).returncode:
        fail("B3B protected merge is not B3C candidate ancestor")
    passed(f"B3B protected merge is candidate ancestor; HEAD {head}")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(
            "candidate diff is not exact B3C allowlist\n"
            f"expected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}"
        )
    passed("Candidate diff is exactly the authorised twelve-path B3C set")

    for path in UNCHANGED_PATHS:
        p = subprocess.run(
            ["git", "diff", "--quiet", f"{BASE}...HEAD", "--", path],
            cwd=ROOT, check=False,
        )
        if p.returncode != 0:
            fail(f"frozen path changed during B3C: {path}")
    passed("Dependency/accounting/B2/SBC5/B3B foundation paths remain unchanged")

    if sha256(LOCK) != LOCK_SHA256:
        fail("reviewed native Cargo.lock hash drift")
    passed("Native Cargo.lock retains reviewed exact hash")

    source = OCR_SOURCE.read_text(encoding="utf-8")
    for anchor in REQUIRED_OCR_ANCHORS:
        if anchor not in source:
            fail(f"native OCR source missing required anchor: {anchor}")
    for marker in FORBIDDEN_NATIVE_MARKERS:
        if marker.lower() in source.lower():
            fail(f"native OCR source contains forbidden marker: {marker}")
    passed("Native OCR host has fixed local process boundary and typed failure controls")

    request_start = source.find("pub(crate) struct OcrReceiptCommandRequest")
    request_end = source.find("\n}\n", request_start)
    if request_start < 0 or request_end < 0:
        fail("webview OCR request structure not found")
    request_block = source[request_start: request_end + 3]
    for forbidden in (
        "path", "sha256", "byte_len", "model", "executable", "url",
        "database", "output", "argument", "shell",
    ):
        if forbidden.lower() in request_block.lower():
            fail(f"webview OCR request leaks forbidden field class: {forbidden}")
    if request_block.count(": String") != 2:
        fail("webview OCR request must contain exactly request_id and document_id strings")
    passed("Webview OCR request exposes only two opaque bounded IDs")

    lib = TAURI_SOURCE.read_text(encoding="utf-8")
    for anchor in (
        "mod ocr_native;",
        ".manage(ocr_native::NativeOcrRegistry::default())",
        "ocr_native::ocr_extract_receipt",
    ):
        if anchor not in lib:
            fail(f"Tauri shell missing B3C wiring: {anchor}")
    if 'APP.contains("ocr_extract_receipt")' in lib:
        fail("SBC-7 frontend wiring has crept into B3C")
    passed("Tauri owns OCR state/command while existing frontend remains unwired")

    for path, label in ((BUILD_RS, "build manifest"), (PERMISSION, "permission")):
        text = path.read_text(encoding="utf-8")
        if "ocr_extract_receipt" not in text:
            fail(f"{label} does not explicitly declare ocr_extract_receipt")
    capability = CAPABILITY.read_text(encoding="utf-8")
    if "core:default" in capability or '"shark-shell"' not in capability:
        fail("Tauri capability widened or lost bounded shark-shell permission")
    passed("AppManifest/permission are explicit and capability remains bounded")

    run([sys.executable, str(ROOT / "scripts" / "check_frozen_baseline.py")])
    passed("Frozen accounting/dependency + bounded current native-shell guard passes")

    change = run([
        sys.executable,
        str(ROOT / "scripts" / "check_sbc1g_change_control.py"),
        "--base-ref", BASE,
    ]).stdout
    if "dependency_changed=false" not in change or "apple_native_changed=true" not in change:
        fail("permanent change-control classification is not dependency=false/apple-native=true")
    passed("Permanent change control classifies B3C correctly and passes")

    if git("status", "--short"):
        fail("repository became dirty during B3C static checks")
    passed("Repository remains clean after B3C static checks")


def ensure_toolchain() -> str:
    rustup = shutil.which("rustup")
    if not rustup:
        fail("rustup is required for B3C runtime proof")
    probe = subprocess.run(
        [rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"],
        cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if probe.returncode:
        run([rustup, "toolchain", "install", RUST_TOOLCHAIN, "--profile", "minimal"])
    version = run([rustup, "run", RUST_TOOLCHAIN, "rustc", "--version"]).stdout.strip()
    if not version.startswith(f"rustc {RUST_TOOLCHAIN}"):
        fail(f"unexpected Rust toolchain: {version}")
    passed(f"Rust {RUST_TOOLCHAIN} selected")
    return rustup


def runtime_checks() -> None:
    required_env = (
        "SHARK_SBC6B_B3C_REAL_SIDECAR",
        "SHARK_SBC6B_B3C_REAL_RECEIPT",
        "SHARK_SBC6B_B3C_REAL_SHA256",
        "SHARK_SBC6B_B3C_REAL_LEN",
        "SHARK_SBC6B_B3C_FAKE_SIDECAR",
        "SHARK_SBC6B_B3C_FAKE_ROOT",
    )
    missing = [name for name in required_env if not os.environ.get(name)]
    if missing:
        fail("B3C runtime proof environment missing: " + ", ".join(missing))

    rustup = ensure_toolchain()
    before = sha256(LOCK)
    target = Path(tempfile.gettempdir()) / "sharkbooks-sbc6b-b3c-target"
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    env["SHARK_SBC1D_PROOF_KEY"] = "sbc6b-b3c-proof-key"
    books_root = Path(tempfile.gettempdir()) / "sharkbooks-sbc6b-b3c-books"
    if books_root.exists():
        shutil.rmtree(books_root, ignore_errors=True)
    books_root.mkdir(parents=True)
    env["SHARK_SBC1D_BOOKS_DIR"] = str(books_root)

    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(ROOT / "workspace" / "Cargo.toml"),
        "-p", "shark-foundation", "--locked", "--jobs", "1",
    ], env=env)
    passed("Inherited permanent foundation regressions pass")

    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "test",
        "--manifest-path", str(ROOT / "workspace" / "Cargo.toml"),
        "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
    ], env=env)
    passed("Tauri shell plus B3C positive/negative process tests pass")

    run([
        rustup, "run", RUST_TOOLCHAIN, "cargo", "build",
        "--manifest-path", str(ROOT / "workspace" / "Cargo.toml"),
        "-p", "shark-tauri-spike", "--locked", "--jobs", "1",
    ], env=env)
    passed("Windows production Tauri shell compiles under frozen lock")

    if sha256(LOCK) != before or before != LOCK_SHA256:
        fail("native Cargo.lock changed during B3C runtime proof")
    if git("status", "--short"):
        fail("repository became dirty during B3C runtime proof")
    passed("Lockfile and repository remain unchanged through B3C runtime proof")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    parser.add_argument("--runtime-only", action="store_true")
    args = parser.parse_args()
    if args.static_only and args.runtime_only:
        fail("choose at most one proof mode")
    if not args.runtime_only:
        static_checks()
    if not args.static_only:
        runtime_checks()
    print("[PASS] SBC-6B B3C native/Tauri validator complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
