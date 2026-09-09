#!/usr/bin/env python3
"""Strict V3 wrapper for the SBC-6B selected-P1 runtime inventory.

This wrapper intentionally reuses the already-audited B0 inventory implementation
but tightens two runtime assumptions discovered by Windows proof:

1. the frozen Stage A engine is ONNX Runtime, so only `_onnx` cache assets are
   eligible for the B0 production-packaging inventory;
2. PaddleOCR 3.7.0 defaults to PP-OCRv6 *medium* model names when only model
   directories are supplied, so the selected tiny logical model names must be
   passed explicitly together with those directories.
"""
from __future__ import annotations

import json
import os
import time
from pathlib import Path
from typing import Any

import sbc6b_inventory_p1_runtime as base

DET_MODEL = "PP-OCRv6_tiny_det"
REC_MODEL = "PP-OCRv6_tiny_rec"


def strict_local_variants(name: str) -> tuple[str, ...]:
    """Return only the runtime format frozen by the Stage A winner."""
    return (f"{name}_onnx",)


def _result_texts(result: Any) -> list[str]:
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
    return [str(x).strip() for x in (rec_texts or []) if str(x).strip()]


def strict_local_model_smoke(models: dict[str, Path], smoke_image: Path) -> dict[str, Any]:
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK"] = "True"

    started = time.perf_counter()
    with base.deny_network():
        # Validate each cached directory against its embedded PaddleX model
        # configuration before constructing the OCR pipeline. This catches any
        # logical-name / physical-cache mismatch before inference begins.
        from paddlex.inference.models.utils.model_resolver import resolve_model_name

        resolve_model_name(model_name=DET_MODEL, model_dir=models[DET_MODEL])
        resolve_model_name(model_name=REC_MODEL, model_dir=models[REC_MODEL])
        base.passed("Selected P1 cached model configs match the frozen tiny logical names")

        from paddleocr import PaddleOCR

        # PaddleOCR 3.7.0 defaults to PP-OCRv6_medium_det/rec when model names
        # are omitted. Always provide BOTH selected names and selected local
        # directories so the pipeline cannot silently retain the medium defaults.
        ocr = PaddleOCR(
            text_detection_model_name=DET_MODEL,
            text_detection_model_dir=str(models[DET_MODEL]),
            text_recognition_model_name=REC_MODEL,
            text_recognition_model_dir=str(models[REC_MODEL]),
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
            text_parts.extend(_result_texts(result))

    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if not text_parts:
        base.fail("explicit-local-model P1 smoke produced no recognised text")
    base.passed("P1 explicit-name + explicit-local-model smoke completed with network guard active")
    return {
        "elapsed_ms": round(elapsed_ms, 3),
        "recognised_line_count": len(text_parts),
        "sample_text": text_parts[:8],
        "configured_detection_model": DET_MODEL,
        "configured_recognition_model": REC_MODEL,
        "cache_format": "onnx",
        "network_guard": "PASS_NO_SOCKET_CONNECT_ALLOWED_DURING_IMPORT_INIT_AND_PREDICT",
    }


def main() -> int:
    base.OUTPUT_SCHEMA = "sbc6b.p1_runtime_inventory.v3"
    base.local_variants = strict_local_variants
    base.local_model_smoke = strict_local_model_smoke
    return base.main()


if __name__ == "__main__":
    raise SystemExit(main())
