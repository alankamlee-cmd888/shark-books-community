#!/usr/bin/env python3
"""SBC-6B B3B single-document local OCR sidecar.

Machine protocol:
- stdout: exactly one JSON object
- stderr: diagnostics only
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
from contextlib import redirect_stdout
from pathlib import Path
from typing import Any

os.environ["ORT_DISABLE_TELEMETRY"] = "1"
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.dont_write_bytecode = True

SCHEMA = "sbc6b.single_document.v1"
MAX_INPUT_BYTES = 25 * 1024 * 1024
MAX_RAW_TEXT_BYTES = 256 * 1024
DET_MODEL = "PP-OCRv6_tiny_det"
REC_MODEL = "PP-OCRv6_tiny_rec"


def emit(payload: dict[str, Any], code: int) -> int:
    sys.stdout.write(json.dumps(payload, separators=(",", ":"), sort_keys=True) + "\n")
    sys.stdout.flush()
    return code


def fail(kind: str, detail: str, code: int, *, runtime_loaded: bool = False) -> int:
    return emit(
        {
            "schema": SCHEMA,
            "status": "failed",
            "failure_kind": kind,
            "detail": detail[:1024],
            "runtime_loaded": runtime_loaded,
        },
        code,
    )


def canonical_sha256(value: str) -> str | None:
    if len(value) != 64:
        return None
    if not all(ch in "0123456789abcdefABCDEF" for ch in value):
        return None
    return value.lower()


def looks_like_supported_image(data: bytes) -> bool:
    return (
        data.startswith(b"\x89PNG\r\n\x1a\n")
        or data.startswith(b"\xff\xd8\xff")
        or data.startswith(b"BM")
    )


def runtime_root() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    override = os.environ.get("SHARK_OCR_MODEL_ROOT")
    if not override:
        raise RuntimeError("source-mode SHARK_OCR_MODEL_ROOT is unavailable")
    return Path(override).resolve()


def parse_date_iso(raw: str) -> str | None:
    import re

    m = re.search(r"\b([0-3]?\d)[/\-.]([01]?\d)[/\-.]((?:20)?\d{2})\b", raw)
    if not m:
        return None
    day = int(m.group(1))
    month = int(m.group(2))
    year_text = m.group(3)
    year = int(year_text) if len(year_text) == 4 else 2000 + int(year_text)
    from datetime import date

    try:
        value = date(year, month, day)
    except ValueError:
        return None
    return value.isoformat()


def parse_total_pence(lines: list[str]) -> int | None:
    import re
    from decimal import Decimal, InvalidOperation

    money = re.compile(r"(?:GBP|\u00a3)?\s*([0-9]{1,7}[.,][0-9]{2})", re.I)
    for line in reversed(lines):
        if "TOTAL" not in line.upper():
            continue
        matches = money.findall(line)
        if not matches:
            continue
        token = matches[-1].replace(",", ".")
        try:
            value = Decimal(token)
        except InvalidOperation:
            return None
        if value < 0 or value.as_tuple().exponent != -2:
            return None
        return int(value * 100)
    return None


def extract_candidates(lines: list[str]) -> dict[str, Any]:
    import re

    nonblank = [" ".join(line.split()) for line in lines if line and line.strip()]
    raw = "\n".join(nonblank)
    merchant = nonblank[0] if nonblank else None
    document_date = None
    reference = None
    for line in nonblank:
        upper = line.upper()
        if document_date is None and ("DATE" in upper or re.search(r"\d[/\-.]\d", line)):
            document_date = parse_date_iso(line)
        if reference is None and ("RECEIPT" in upper or re.search(r"\bREF(?:ERENCE)?\b", upper)):
            m = re.search(r"(?:RECEIPT|REF(?:ERENCE)?)\s*[:#-]?\s*([A-Z0-9][A-Z0-9\- ]+)", upper)
            if m:
                reference = m.group(1).strip()

    if len(raw.encode("utf-8")) > MAX_RAW_TEXT_BYTES:
        raise ValueError("OCR raw text exceeds 256 KiB")

    return {
        "raw_text": raw,
        "merchant": merchant,
        "document_date": document_date,
        "total_pence": parse_total_pence(nonblank),
        "currency": "GBP" if ("GBP" in raw.upper() or "\u00a3" in raw) else None,
        "reference": reference,
    }


def warnings_for(candidates: dict[str, Any]) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    mapping = [
        ("merchant", "missing_merchant"),
        ("document_date", "missing_date"),
        ("total_pence", "missing_total"),
        ("currency", "missing_currency"),
        ("reference", "missing_reference"),
    ]
    for field, code in mapping:
        if candidates[field] is None:
            out.append({"code": code})
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--expected-byte-len", required=True, type=int)
    args = parser.parse_args()

    expected_hash = canonical_sha256(args.expected_sha256)
    if expected_hash is None or args.expected_byte_len <= 0:
        return fail("integrity_mismatch", "invalid expected document integrity metadata", 20)

    path = Path(args.input).resolve()
    try:
        st = path.stat()
    except OSError as exc:
        return fail("engine_failure", f"document unavailable: {exc}", 30)
    if not path.is_file():
        return fail("engine_failure", "document input is not a regular file", 30)
    if st.st_size <= 0 or st.st_size > MAX_INPUT_BYTES:
        return fail("unsupported_document", "document size is outside the bounded OCR input range", 21)
    if st.st_size != args.expected_byte_len:
        return fail("integrity_mismatch", "document byte length does not match expected length", 20)

    try:
        data = path.read_bytes()
    except OSError as exc:
        return fail("engine_failure", f"document read failed: {exc}", 30)
    if len(data) != args.expected_byte_len:
        return fail("integrity_mismatch", "document length changed during verified read", 20)
    observed_hash = hashlib.sha256(data).hexdigest()
    if observed_hash != expected_hash:
        return fail("integrity_mismatch", "document SHA-256 does not match expected hash", 20)
    if not looks_like_supported_image(data):
        return fail("unsupported_document", "receipt OCR currently accepts PNG, JPEG or BMP image bytes", 21)

    suffix = ".png" if data.startswith(b"\x89PNG") else (".jpg" if data.startswith(b"\xff\xd8\xff") else ".bmp")
    temp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(prefix="shark-ocr-", suffix=suffix, delete=False) as temp:
            temp.write(data)
            temp.flush()
            temp_path = Path(temp.name)

        import direct_onnx_parity as core

        core.passed = lambda message: print(f"[OCR] {message}", file=sys.stderr, flush=True)
        def _core_fail(message: str) -> None:
            raise RuntimeError(message)
        core.fail = _core_fail

        model_root = runtime_root()
        det_dir = model_root / "models" / "det"
        rec_dir = model_root / "models" / "rec"

        with redirect_stdout(sys.stderr):
            packages = core.require_packages()
            core.require_model_files(det_dir, rec_dir)

            with core.deny_network():
                import numpy as np
                import cv2
                import pyclipper
                import onnxruntime as ort

                if ort.__version__ != "1.23.2":
                    raise RuntimeError(f"onnxruntime version drift: {ort.__version__}")
                telemetry_disable = getattr(ort, "disable_telemetry_events", None)
                if not callable(telemetry_disable):
                    raise RuntimeError("onnxruntime telemetry-disable API unavailable")
                telemetry_disable()
                core.read_detection_config(det_dir / "inference.yml")
                characters, _ = core.read_recognition_config(rec_dir / "inference.yml")
                engine = core.DirectTinyOcr(det_dir, rec_dir, characters, ort, cv2, np, pyclipper)
                lines, scores = engine.predict(temp_path)

        facts = extract_candidates(lines)
        mean_bps = None
        if scores:
            mean = max(0.0, min(1.0, sum(float(x) for x in scores) / len(scores)))
            mean_bps = int(round(mean * 10_000))

        extra_warnings = [{"code": "no_text_detected"}] if facts["raw_text"] == "" else []
        result = {
            "schema": SCHEMA,
            "status": "completed",
            "observed_document_sha256": observed_hash,
            "observed_byte_len": len(data),
            "provenance": {
                "engine_id": "shark-direct-onnx",
                "engine_version": "sbc6b-b1-v1",
                "runtime_id": "onnxruntime-1.23.2-cpu",
                "model_ids": [DET_MODEL, REC_MODEL],
            },
            "raw_text": facts["raw_text"],
            "regions": [],
            "candidates": {
                "merchant_text": facts["merchant"],
                "document_date": facts["document_date"],
                "total_pence": facts["total_pence"],
                "currency": facts["currency"],
                "reference": facts["reference"],
            },
            "mean_ocr_confidence_bps": mean_bps,
            "warnings": extra_warnings + warnings_for(facts),
            "privacy": {
                "ort_disable_telemetry_env": os.environ.get("ORT_DISABLE_TELEMETRY") == "1",
                "ort_telemetry_disable_api_called": True,
                "python_tcp_connections_denied_during_runtime": True,
                "paddleocr_imported": "paddleocr" in sys.modules,
                "paddlex_imported": "paddlex" in sys.modules,
            },
            "runtime_packages": packages,
        }
        return emit(result, 0)
    except Exception as exc:
        return fail("engine_failure", str(exc), 30, runtime_loaded="onnxruntime" in sys.modules)
    finally:
        if temp_path is not None:
            try:
                temp_path.unlink(missing_ok=True)
            except OSError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
