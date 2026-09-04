#!/usr/bin/env python3
"""Materialise the exact frozen Beankeeper source without dependency drift."""
from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

REPO = "https://github.com/Govcraft/beankeeper.git"
COMMIT = "d573db5e61089b0922f95c991732394d08e3cf92"
ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "upstream" / "beankeeper"


def run(*args: str, cwd: Path | None = None) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True, stderr=subprocess.STDOUT).strip()


def main() -> int:
    if shutil.which("git") is None:
        print("FAIL: git is required", file=sys.stderr)
        return 2

    if TARGET.exists():
        try:
            head = run("git", "rev-parse", "HEAD", cwd=TARGET)
            dirty = run("git", "status", "--porcelain", cwd=TARGET)
        except subprocess.CalledProcessError as exc:
            print(f"FAIL: existing {TARGET} is not a valid git checkout: {exc.output}", file=sys.stderr)
            return 2
        if dirty:
            print(f"FAIL: existing Beankeeper checkout is dirty: {TARGET}", file=sys.stderr)
            return 2
        if head != COMMIT:
            print(f"FAIL: Beankeeper HEAD {head} != frozen {COMMIT}", file=sys.stderr)
            return 2
        print(f"PASS: Beankeeper already pinned at {COMMIT}")
        return 0

    TARGET.parent.mkdir(parents=True, exist_ok=True)
    subprocess.check_call(["git", "init", str(TARGET)])
    subprocess.check_call(["git", "remote", "add", "origin", REPO], cwd=TARGET)
    subprocess.check_call(["git", "fetch", "--depth", "1", "origin", COMMIT], cwd=TARGET)
    subprocess.check_call(["git", "checkout", "--detach", "FETCH_HEAD"], cwd=TARGET)
    head = run("git", "rev-parse", "HEAD", cwd=TARGET)
    if head != COMMIT:
        print(f"FAIL: checked out {head}, expected {COMMIT}", file=sys.stderr)
        return 2
    print(f"PASS: materialised Beankeeper {COMMIT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
