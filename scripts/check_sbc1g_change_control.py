#!/usr/bin/env python3
"""Permanent SBC-1G frozen-foundation CI/change-control guard."""
from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "workspace" / "Cargo.lock"
BASELINE_GUARD = ROOT / "scripts" / "check_frozen_baseline.py"
WORKFLOW = ROOT / ".github" / "workflows" / "sbc-foundation-guard.yml"
POLICY = ROOT / "docs" / "SBC1G_CI_REGRESSION_AND_CHANGE_CONTROL_POLICY_2026-09-08.md"

EXPECTED_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
EXPECTED_CRITICAL_PACKAGES = {
    "beankeeper": {"0.4.0"},
    "beankeeper-cli": {"0.8.0"},
    "rusqlite": {"0.39.0"},
    "openssl-sys": {"0.9.112"},
    "openssl-src": {"300.5.4+3.5.4"},
    "tauri": {"2.11.5"},
    "tauri-build": {"2.6.3"},
}

DEPENDENCY_CONTROL_PATHS = {
    "workspace/Cargo.lock",
    "workspace/Cargo.toml",
    "workspace/shark-foundation/Cargo.toml",
    "workspace/shark-tauri-spike/Cargo.toml",
    "scripts/bootstrap_beankeeper.py",
    "docs/frozen/41_SBC0E_FROZEN_FOUNDATION_MANIFEST_2026-09-04.json",
}

APPLE_NATIVE_PREFIXES = (
    "workspace/shark-foundation/src/",
    "workspace/shark-tauri-spike/src/",
    "workspace/shark-tauri-spike/capabilities/",
    "workspace/shark-tauri-spike/permissions/",
)
APPLE_NATIVE_EXACT = DEPENDENCY_CONTROL_PATHS | {
    "workspace/shark-tauri-spike/build.rs",
    "workspace/shark-tauri-spike/tauri.conf.json",
    "workspace/shark-tauri-spike/tauri.ios.conf.json",
    "scripts/check_frozen_baseline.py",
}


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args: str, cwd: Path | None = None) -> str:
    result = subprocess.run(
        list(args),
        cwd=cwd or ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(
            f"command failed ({result.returncode}): {' '.join(args)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result.stdout.strip()


def resolve_base(explicit: str | None) -> str | None:
    candidate = explicit or os.environ.get("SBC_CHANGE_BASE")
    if candidate and set(candidate) != {"0"}:
        return candidate
    probe = subprocess.run(
        ["git", "rev-parse", "HEAD^"],
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return probe.stdout.strip() if probe.returncode == 0 else None


def changed_paths(base: str | None) -> list[str]:
    if not base:
        return []
    out = run("git", "diff", "--name-only", f"{base}...HEAD")
    return sorted(path for path in out.splitlines() if path)


def native_apple_change(paths: list[str]) -> bool:
    for path in paths:
        if path in APPLE_NATIVE_EXACT or path.startswith(APPLE_NATIVE_PREFIXES):
            return True
    return False


def dependency_change(paths: list[str]) -> bool:
    return any(path in DEPENDENCY_CONTROL_PATHS for path in paths)


def require_dependency_review_bundle(paths: list[str]) -> None:
    review_paths = [path for path in paths if path.startswith("docs/dependency-reviews/")]
    upper = [path.upper() for path in review_paths]
    if not any("SBOM" in path and path.endswith(".CSV") for path in upper):
        fail("dependency change requires a regenerated SBOM under docs/dependency-reviews/")
    if not any("LICENSE" in path and path.endswith(".CSV") for path in upper):
        fail("dependency change requires a regenerated licence register under docs/dependency-reviews/")
    if not any("ADJUDICATION" in path and path.endswith(".MD") for path in upper):
        fail("dependency change requires an adjudication record under docs/dependency-reviews/")


def check_lock_packages() -> None:
    if not LOCK.is_file():
        fail("workspace/Cargo.lock is missing")
    if sha256(LOCK) != EXPECTED_LOCK_SHA256:
        fail("reviewed native Cargo.lock hash drift")
    parsed = tomllib.loads(LOCK.read_text(encoding="utf-8"))
    actual: dict[str, set[str]] = {}
    for package in parsed.get("package", []):
        actual.setdefault(str(package.get("name")), set()).add(str(package.get("version")))
    for name, expected_versions in EXPECTED_CRITICAL_PACKAGES.items():
        versions = actual.get(name, set())
        if versions != expected_versions:
            fail(f"critical package drift for {name}: expected {sorted(expected_versions)}, got {sorted(versions)}")


def check_workflow_policy() -> None:
    if not WORKFLOW.is_file():
        fail("permanent foundation guard workflow is missing")
    text = WORKFLOW.read_text(encoding="utf-8")
    required = (
        "pull_request:",
        "push:",
        "windows-latest",
        "apple-native-compile",
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "check_sbc1g_change_control.py",
        "check_frozen_baseline.py",
        "cargo test -p shark-foundation --locked",
        "cargo build -p shark-tauri-spike --locked",
        "cargo metadata --locked",
    )
    for anchor in required:
        if anchor not in text:
            fail(f"CI workflow missing required guard anchor: {anchor}")

    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if "cargo " in stripped and any(f"cargo {verb}" in stripped for verb in ("test", "build", "check", "metadata", "fetch")):
            if "--locked" not in stripped:
                fail(f"normal Rust CI command is not locked: {stripped}")

    if not POLICY.is_file():
        fail("SBC-1G permanent CI/change-control policy document is missing")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-ref")
    parser.add_argument("--classify-only", action="store_true")
    args = parser.parse_args()

    base = resolve_base(args.base_ref)
    paths = changed_paths(base)
    apple_changed = native_apple_change(paths)
    deps_changed = dependency_change(paths)

    if args.classify_only:
        print(f"apple_native_changed={'true' if apple_changed else 'false'}")
        print(f"dependency_changed={'true' if deps_changed else 'false'}")
        return 0

    if not BASELINE_GUARD.is_file():
        fail("existing frozen-baseline guard is missing")
    run(sys.executable, str(BASELINE_GUARD))
    check_lock_packages()
    check_workflow_policy()

    if deps_changed:
        require_dependency_review_bundle(paths)

    print("PASS: SBC-1G permanent frozen-foundation change-control guard")
    print(f"base_ref={base or 'none'}")
    print(f"changed_paths={len(paths)}")
    print(f"dependency_changed={'true' if deps_changed else 'false'}")
    print(f"apple_native_changed={'true' if apple_changed else 'false'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
