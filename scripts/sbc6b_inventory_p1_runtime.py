#!/usr/bin/env python3
"""SBC-6B P1 production-runtime inventory and explicit-local-model smoke proof."""
from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.metadata as md
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Any

SELECTED = {"paddleocr": "3.7.0", "onnxruntime": "1.23.2"}
MODEL_NAMES = ("PP-OCRv6_tiny_det", "PP-OCRv6_tiny_rec")
OUTPUT_SCHEMA = "sbc6b.p1_runtime_inventory.v1"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def fail(message: str) -> None:
    print(f"[FAIL] {message}")
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}")


def package_version(name: str) -> str:
    try:
        return md.version(name)
    except md.PackageNotFoundError:
        fail(f"required package is not installed: {name}")
        raise AssertionError


def csv_write(path: Path, fieldnames: list[str], rows: list[dict[str, Any]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def resolve_models() -> dict[str, Path]:
    # PaddleX release/3.7 official_models.py uses CACHE_DIR/official_models;
    # standard Windows installs resolve CACHE_DIR beneath ~/.paddlex.
    root = Path.home() / ".paddlex" / "official_models"
    if not root.is_dir():
        fail(f"PaddleX official-model cache not found: {root}")
    resolved: dict[str, Path] = {}
    for name in MODEL_NAMES:
        path = root / name
        if not path.is_dir():
            fail(f"selected cached model directory missing: {path}")
        resolved[name] = path
    passed(f"Selected P1 model directories found under {root}")
    return resolved


def inventory_tree(label: str, root: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        rows.append({
            "component": label,
            "relative_path": path.relative_to(root).as_posix(),
            "size_bytes": path.stat().st_size,
            "sha256": sha256(path),
        })
    if not rows:
        fail(f"no model/runtime files found beneath {root}")
    return rows


def distribution_rows() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    packages: list[dict[str, Any]] = []
    licenses: list[dict[str, Any]] = []
    for dist in sorted(md.distributions(), key=lambda d: (d.metadata.get("Name") or "").lower()):
        name = dist.metadata.get("Name") or ""
        version = dist.version or ""
        license_expr = dist.metadata.get("License-Expression") or ""
        license_field = dist.metadata.get("License") or ""
        classifiers = [
            c for c in (dist.metadata.get_all("Classifier") or [])
            if c.startswith("License ::")
        ]
        packages.append({
            "name": name,
            "version": version,
            "license_expression": license_expr,
            "license_field": license_field.replace("\r", " ").replace("\n", " ")[:500],
            "license_classifiers": " | ".join(classifiers),
        })
        for file in dist.files or []:
            text = str(file)
            base = Path(text).name.upper()
            if not (
                base.startswith("LICENSE")
                or base.startswith("COPYING")
                or base.startswith("NOTICE")
                or base.startswith("COPYRIGHT")
            ):
                continue
            try:
                full = Path(dist.locate_file(file))
                if full.is_file():
                    licenses.append({
                        "package": name,
                        "version": version,
                        "path": text.replace("\\", "/"),
                        "size_bytes": full.stat().st_size,
                        "sha256": sha256(full),
                    })
            except OSError:
                continue
    return packages, licenses


@contextmanager
def deny_network() -> Any:
    original_socket_connect = socket.socket.connect
    original_create_connection = socket.create_connection

    def blocked_connect(self: socket.socket, address: Any) -> None:
        raise RuntimeError(f"network access blocked by SBC-6B smoke guard: {address!r}")

    def blocked_create_connection(*args: Any, **kwargs: Any) -> None:
        raise RuntimeError("network access blocked by SBC-6B smoke guard")

    socket.socket.connect = blocked_connect  # type: ignore[assignment]
    socket.create_connection = blocked_create_connection  # type: ignore[assignment]
    try:
        yield
    finally:
        socket.socket.connect = original_socket_connect  # type: ignore[assignment]
        socket.create_connection = original_create_connection  # type: ignore[assignment]


def local_model_smoke(models: dict[str, Path], smoke_image: Path) -> dict[str, Any]:
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK"] = "True"

    started = time.perf_counter()
    with deny_network():
        from paddleocr import PaddleOCR

        ocr = PaddleOCR(
            text_detection_model_dir=str(models["PP-OCRv6_tiny_det"]),
            text_recognition_model_dir=str(models["PP-OCRv6_tiny_rec"]),
            use_doc_orientation_classify=False,
            use_doc_unwarping=False,
            use_textline_orientation=False,
            engine="onnxruntime",
            device="cpu",
            cpu_threads=max(1, min(4, (os.cpu_count() or 2))),
        )
        results = ocr.predict(str(smoke_image))
        text_parts: list[str] = []
        for result in results:
            try:
                rec_texts = result["rec_texts"]
            except Exception:
                data = getattr(result, "json", None)
                if callable(data):
                    data = data()
                if isinstance(data, str):
                    try:
                        data = json.loads(data)
                    except json.JSONDecodeError:
                        data = None
                if isinstance(data, dict) and isinstance(data.get("res"), dict):
                    rec_texts = data["res"].get("rec_texts", [])
                elif isinstance(data, dict):
                    rec_texts = data.get("rec_texts", [])
                else:
                    rec_texts = []
            text_parts.extend(str(x).strip() for x in (rec_texts or []) if str(x).strip())
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if not text_parts:
        fail("explicit-local-model P1 smoke produced no recognised text")
    passed("P1 explicit-local-model smoke completed with process-level network guard active")
    return {
        "elapsed_ms": round(elapsed_ms, 3),
        "recognised_line_count": len(text_parts),
        "sample_text": text_parts[:8],
        "network_guard": "PASS_NO_SOCKET_CONNECT_ALLOWED_DURING_IMPORT_INIT_AND_PREDICT",
    }


def git(repo: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *args],
        cwd=repo,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        fail(f"git {' '.join(args)} failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--smoke-image", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    repo = Path(args.repo).resolve()
    smoke_image = Path(args.smoke_image).resolve()
    output = Path(args.output).resolve()

    if not smoke_image.is_file():
        fail(f"smoke image missing: {smoke_image}")
    if git(repo, "status", "--short"):
        fail("repository must be clean at SBC-6B inventory entry")
    head = git(repo, "rev-parse", "HEAD")
    passed(f"Repository clean; candidate HEAD {head}")

    if not ((3, 10) <= sys.version_info[:2] <= (3, 13)):
        fail(f"unsupported runtime Python: {sys.version}")

    for name, expected in SELECTED.items():
        actual = package_version(name)
        if actual != expected:
            fail(f"{name} version drift: expected {expected}, got {actual}")
    passed("Selected P1 direct runtime package versions are exact")

    models = resolve_models()

    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True, exist_ok=True)

    model_rows: list[dict[str, Any]] = []
    model_summary: dict[str, Any] = {}
    for name, root in models.items():
        rows = inventory_tree(name, root)
        model_rows.extend(rows)
        model_summary[name] = {
            "path": str(root),
            "file_count": len(rows),
            "total_bytes": sum(int(row["size_bytes"]) for row in rows),
            "tree_sha256": hashlib.sha256(
                "\n".join(
                    f"{row['relative_path']}\0{row['size_bytes']}\0{row['sha256']}"
                    for row in rows
                ).encode("utf-8")
            ).hexdigest(),
        }

    package_rows, license_rows = distribution_rows()
    smoke = local_model_smoke(models, smoke_image)

    csv_write(
        output / "SBC6B_P1_MODEL_FILES.csv",
        ["component", "relative_path", "size_bytes", "sha256"],
        model_rows,
    )
    csv_write(
        output / "SBC6B_P1_PYTHON_SBOM.csv",
        ["name", "version", "license_expression", "license_field", "license_classifiers"],
        package_rows,
    )
    csv_write(
        output / "SBC6B_P1_LICENSE_FILES.csv",
        ["package", "version", "path", "size_bytes", "sha256"],
        license_rows,
    )

    result = {
        "schema": OUTPUT_SCHEMA,
        "git_head": head,
        "python": sys.version,
        "selected_runtime": {
            "paddleocr": package_version("paddleocr"),
            "onnxruntime": package_version("onnxruntime"),
            "det_model": "PP-OCRv6_tiny_det",
            "rec_model": "PP-OCRv6_tiny_rec",
            "device": "cpu",
            "engine": "onnxruntime",
        },
        "models": model_summary,
        "python_distribution_count": len(package_rows),
        "license_file_count": len(license_rows),
        "explicit_local_model_smoke": smoke,
    }
    result_path = output / "SBC6B_P1_RUNTIME_INVENTORY.json"
    result_path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    summary = [
        "Shark Books Community - SBC-6B P1 runtime inventory",
        f"git_head: {head}",
        f"python: {sys.version.splitlines()[0]}",
        "selected: PaddleOCR 3.7.0 / ONNX Runtime 1.23.2 / PP-OCRv6 tiny det+rec / CPU",
        f"python_distributions: {len(package_rows)}",
        f"license_files: {len(license_rows)}",
        f"det_model_bytes: {model_summary['PP-OCRv6_tiny_det']['total_bytes']}",
        f"rec_model_bytes: {model_summary['PP-OCRv6_tiny_rec']['total_bytes']}",
        f"smoke_network_guard: {smoke['network_guard']}",
        f"smoke_recognised_lines: {smoke['recognised_line_count']}",
    ]
    (output / "SBC6B_P1_RUNTIME_INVENTORY_SUMMARY.txt").write_text(
        "\n".join(summary) + "\n", encoding="utf-8"
    )
    print("\n".join(summary))
    print(f"SBC6B_INVENTORY_DIR={output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
