#!/usr/bin/env python3
"""Run the preregistered SBC-6A OCR comparator on synthetic receipts."""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import os
import platform
import re
import shutil
import socket
import statistics
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any, Callable

import psutil
from PIL import Image

LANES = {
    "T0": {"engine": "tesseract", "label": "Tesseract 5.5.3 English"},
    "P1": {
        "engine": "paddleocr",
        "label": "PaddleOCR 3.7.0 PP-OCRv6 tiny ONNX Runtime CPU",
        "det": "PP-OCRv6_tiny_det",
        "rec": "PP-OCRv6_tiny_rec",
    },
    "P2": {
        "engine": "paddleocr",
        "label": "PaddleOCR 3.7.0 PP-OCRv6 small ONNX Runtime CPU",
        "det": "PP-OCRv6_small_det",
        "rec": "PP-OCRv6_small_rec",
    },
}

CRITICAL_TARGET = 0.95
AMOUNT_TARGET = 0.97
CONDITION_CRITICAL_FLOOR = 0.80
FALSE_CONFIDENT_AMOUNT_MAX = 1
LATENCY_P95_MAX_MS = 8000.0
MEMORY_PREFERRED_MAX_BYTES = int(1.5 * 1024**3)
MEMORY_OWNER_DECISION_BYTES = int(2.0 * 1024**3)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def norm_text(value: str | None) -> str | None:
    if value is None:
        return None
    return " ".join(re.sub(r"[^A-Z0-9 ]+", " ", value.upper()).split())


def norm_ref(value: str | None) -> str | None:
    if value is None:
        return None
    cleaned = re.sub(r"[^A-Z0-9]+", "", value.upper())
    return cleaned or None


def parse_date(raw: str) -> str | None:
    m = re.search(r"\b([0-3]?\d)[/\-.]([01]?\d)[/\-.]((?:20)?\d{2})\b", raw)
    if not m:
        return None
    day = int(m.group(1))
    month = int(m.group(2))
    year_text = m.group(3)
    year = int(year_text) if len(year_text) == 4 else 2000 + int(year_text)
    if not (1 <= day <= 31 and 1 <= month <= 12):
        return None
    return f"{day:02d}/{month:02d}/{year:04d}"


def parse_total_pence(lines: list[str]) -> int | None:
    # Fail closed: only a line containing TOTAL can supply the amount.
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


def extract_factual_candidates(lines: list[str]) -> dict[str, Any]:
    nonblank = [" ".join(line.split()) for line in lines if line and line.strip()]
    raw = "\n".join(nonblank)
    merchant = nonblank[0] if nonblank else None
    date = None
    reference = None
    for line in nonblank:
        upper = line.upper()
        if date is None and ("DATE" in upper or re.search(r"\d[/\-.]\d", line)):
            date = parse_date(line)
        if reference is None and ("RECEIPT" in upper or re.search(r"\bREF(?:ERENCE)?\b", upper)):
            m = re.search(r"(?:RECEIPT|REF(?:ERENCE)?)\s*[:#-]?\s*([A-Z0-9][A-Z0-9\- ]+)", upper)
            if m:
                reference = m.group(1).strip()
    total_pence = parse_total_pence(nonblank)
    currency = "GBP" if ("GBP" in raw.upper() or "\u00a3" in raw) else None
    return {
        "merchant": merchant,
        "date": date,
        "total_pence": total_pence,
        "currency": currency,
        "reference": reference,
        "raw_text": raw,
    }


class MemorySampler:
    def __init__(self) -> None:
        self._stop = threading.Event()
        self.peak = 0
        self._thread = threading.Thread(target=self._loop, daemon=True)

    @staticmethod
    def _rss_tree() -> int:
        process = psutil.Process(os.getpid())
        total = 0
        procs = [process]
        try:
            procs.extend(process.children(recursive=True))
        except psutil.Error:
            pass
        for proc in procs:
            try:
                total += int(proc.memory_info().rss)
            except psutil.Error:
                pass
        return total

    def _loop(self) -> None:
        while not self._stop.is_set():
            self.peak = max(self.peak, self._rss_tree())
            self._stop.wait(0.025)
        self.peak = max(self.peak, self._rss_tree())

    def __enter__(self) -> "MemorySampler":
        self.peak = self._rss_tree()
        self._thread.start()
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        self._stop.set()
        self._thread.join(timeout=2.0)


@dataclass
class OcrOutput:
    lines: list[str]
    confidences: list[float]


def build_tesseract() -> tuple[Callable[[Path], OcrOutput], dict[str, Any]]:
    import pytesseract

    exe = shutil.which("tesseract")
    if not exe:
        raise RuntimeError("Tesseract executable is not available on PATH")
    version = str(pytesseract.get_tesseract_version())
    if not version.startswith("5.5.3"):
        raise RuntimeError(f"Tesseract 5.5.3 required, found {version}")

    def predict(path: Path) -> OcrOutput:
        from pytesseract import Output

        with Image.open(path) as image:
            data = pytesseract.image_to_data(image, config="--psm 6", output_type=Output.DICT, lang="eng")
        words: list[str] = []
        confs: list[float] = []
        line_map: dict[tuple[int, int, int], list[str]] = {}
        count = len(data.get("text", []))
        for i in range(count):
            text = str(data["text"][i]).strip()
            if not text:
                continue
            try:
                conf = float(data["conf"][i])
            except (ValueError, TypeError):
                conf = -1.0
            if conf >= 0:
                confs.append(max(0.0, min(1.0, conf / 100.0)))
            key = (int(data["block_num"][i]), int(data["par_num"][i]), int(data["line_num"][i]))
            line_map.setdefault(key, []).append(text)
            words.append(text)
        lines = [" ".join(parts) for _, parts in sorted(line_map.items())]
        if not lines and words:
            lines = [" ".join(words)]
        return OcrOutput(lines=lines, confidences=confs)

    return predict, {"binary": exe, "version": version}


def _result_get(result: Any, key: str, default: Any) -> Any:
    try:
        return result[key]
    except Exception:
        pass
    data = getattr(result, "json", None)
    if callable(data):
        try:
            data = data()
        except Exception:
            data = None
    if isinstance(data, str):
        try:
            data = json.loads(data)
        except json.JSONDecodeError:
            data = None
    if isinstance(data, dict):
        if key in data:
            return data[key]
        if isinstance(data.get("res"), dict) and key in data["res"]:
            return data["res"][key]
    return default


def build_paddle(lane: str) -> tuple[Callable[[Path], OcrOutput], dict[str, Any]]:
    from paddleocr import PaddleOCR

    spec = LANES[lane]
    started = time.perf_counter()
    with MemorySampler() as mem:
        ocr = PaddleOCR(
            text_detection_model_name=spec["det"],
            text_recognition_model_name=spec["rec"],
            use_doc_orientation_classify=False,
            use_doc_unwarping=False,
            use_textline_orientation=False,
            engine="onnxruntime",
            device="cpu",
            cpu_threads=max(1, min(4, (os.cpu_count() or 2))),
        )
    load_ms = (time.perf_counter() - started) * 1000.0

    def predict(path: Path) -> OcrOutput:
        results = ocr.predict(str(path))
        lines: list[str] = []
        confs: list[float] = []
        for result in results:
            rec_texts = _result_get(result, "rec_texts", []) or []
            rec_scores = _result_get(result, "rec_scores", []) or []
            lines.extend(str(text).strip() for text in rec_texts if str(text).strip())
            for score in rec_scores:
                try:
                    confs.append(max(0.0, min(1.0, float(score))))
                except (TypeError, ValueError):
                    pass
        return OcrOutput(lines=lines, confidences=confs)

    metadata = {
        "paddleocr": importlib.metadata.version("paddleocr"),
        "onnxruntime": importlib.metadata.version("onnxruntime"),
        "det_model": spec["det"],
        "rec_model": spec["rec"],
        "engine": "onnxruntime",
        "device": "cpu",
        "model_load_ms": round(load_ms, 3),
        "model_load_peak_rss_bytes": mem.peak,
    }
    return predict, metadata


def percentile(values: list[float], q: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, int((len(ordered) - 1) * q + 0.999999)))
    return ordered[index]


def mean_or_none(values: list[float]) -> float | None:
    return statistics.mean(values) if values else None


def score_fixture(truth: dict[str, Any], predicted: dict[str, Any], mean_conf: float | None) -> dict[str, Any]:
    field = {
        "merchant": norm_text(predicted.get("merchant")) == norm_text(str(truth["merchant"])),
        "date": predicted.get("date") == truth["date"],
        "total_pence": predicted.get("total_pence") == truth["total_pence"],
        "currency": predicted.get("currency") == truth["currency"],
        "reference": norm_ref(predicted.get("reference")) == norm_ref(str(truth["reference"])),
    }
    wrong_amount = predicted.get("total_pence") is not None and not field["total_pence"]
    false_confident_amount = bool(wrong_amount and mean_conf is not None and mean_conf >= 0.80)
    return {
        "field_match": field,
        "critical_match": bool(field["date"] and field["total_pence"]),
        "all_factual_match": all(field.values()),
        "false_confident_amount": false_confident_amount,
    }


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    n = len(rows)
    if n == 0:
        return {"count": 0}
    latencies = [float(row["latency_ms"]) for row in rows]
    rss = [int(row["peak_rss_bytes"]) for row in rows]
    confs = [float(row["mean_confidence"]) for row in rows if row["mean_confidence"] is not None]

    def rate(key: str) -> float:
        return sum(bool(row[key]) for row in rows) / n

    field_rates: dict[str, float] = {}
    for key in ("merchant", "date", "total_pence", "currency", "reference"):
        field_rates[key] = sum(bool(row["field_match"][key]) for row in rows) / n

    conditions: dict[str, Any] = {}
    for condition in sorted({str(row["condition"]) for row in rows}):
        subset = [row for row in rows if row["condition"] == condition]
        conditions[condition] = {
            "count": len(subset),
            "critical_match_rate": sum(bool(row["critical_match"]) for row in subset) / len(subset),
            "amount_match_rate": sum(bool(row["field_match"]["total_pence"]) for row in subset) / len(subset),
        }
    return {
        "count": n,
        "field_match_rate": field_rates,
        "critical_match_rate": rate("critical_match"),
        "all_factual_match_rate": rate("all_factual_match"),
        "false_confident_amount_count": sum(bool(row["false_confident_amount"]) for row in rows),
        "latency_ms": {
            "p50": round(percentile(latencies, 0.50) or 0.0, 3),
            "p95": round(percentile(latencies, 0.95) or 0.0, 3),
            "max": round(max(latencies), 3),
        },
        "peak_rss_bytes": max(rss),
        "mean_ocr_confidence": round(mean_or_none(confs), 6) if confs else None,
        "by_condition": conditions,
    }


def gate(summary: dict[str, Any]) -> dict[str, Any]:
    if not summary or summary.get("count") != 48:
        return {"pass": False, "reasons": ["lane did not produce all 48 fixture results"]}
    reasons: list[str] = []
    if float(summary["critical_match_rate"]) < CRITICAL_TARGET:
        reasons.append("critical date+amount rate below 95%")
    if float(summary["field_match_rate"]["total_pence"]) < AMOUNT_TARGET:
        reasons.append("amount exact-pence rate below 97%")
    bad_conditions = [
        name for name, data in summary["by_condition"].items()
        if float(data["critical_match_rate"]) < CONDITION_CRITICAL_FLOOR
    ]
    if bad_conditions:
        reasons.append("critical-field condition floor failed: " + ", ".join(bad_conditions))
    if int(summary["false_confident_amount_count"]) > FALSE_CONFIDENT_AMOUNT_MAX:
        reasons.append("too many false confident amount substitutions")
    if float(summary["latency_ms"]["p95"]) > LATENCY_P95_MAX_MS:
        reasons.append("CPU p95 exceeds 8 seconds/image")
    if int(summary["peak_rss_bytes"]) > MEMORY_OWNER_DECISION_BYTES:
        reasons.append("peak working set exceeds 2 GiB owner-decision ceiling")
    return {"pass": not reasons, "reasons": reasons}


def verify_offline_before_documents() -> None:
    # This check runs before fixture bytes are read. A successful TCP connection blocks
    # the no-egress proof rather than attempting OCR while connectivity is present.
    reachable: list[str] = []
    for host, port in (("1.1.1.1", 443), ("github.com", 443)):
        try:
            with socket.create_connection((host, port), timeout=1.2):
                reachable.append(f"{host}:{port}")
        except OSError:
            pass
    if reachable:
        raise RuntimeError("offline proof requested but outbound connectivity is still available: " + ", ".join(reachable))


def package_versions() -> dict[str, str | None]:
    packages = ("paddleocr", "onnxruntime", "Pillow", "pytesseract", "psutil")
    out: dict[str, str | None] = {}
    for name in packages:
        try:
            out[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            out[name] = None
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixtures", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--lanes", nargs="+", default=["T0", "P1", "P2"])
    parser.add_argument("--offline-proof", action="store_true")
    args = parser.parse_args()

    selected_lanes = [lane.upper() for lane in args.lanes]
    unknown = sorted(set(selected_lanes) - set(LANES))
    if unknown:
        raise SystemExit(f"unknown lanes: {unknown}")

    if args.offline_proof:
        verify_offline_before_documents()
        os.environ.setdefault("HF_HUB_OFFLINE", "1")
        os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")

    fixtures_dir = Path(args.fixtures).resolve()
    manifest_path = fixtures_dir / "manifest.json"
    output_dir = Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    fixtures = manifest.get("fixtures", [])
    if manifest.get("schema") != "sbc6a.synthetic_receipts.v1" or len(fixtures) != 48:
        raise RuntimeError("synthetic fixture manifest drift")

    result: dict[str, Any] = {
        "schema": "sbc6a.ocr_benchmark_result.v1",
        "generated_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "fixture_manifest_sha256": sha256(manifest_path),
        "fixture_count": len(fixtures),
        "offline_proof_requested": bool(args.offline_proof),
        "offline_connectivity_check": "PASS_NO_CONNECTIVITY_BEFORE_DOCUMENT_READ" if args.offline_proof else "NOT_RUN",
        "environment": {
            "python": sys.version,
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "logical_cpu_count": os.cpu_count(),
            "packages": package_versions(),
            "research_venv_bytes": int(os.environ.get("SBC6A_VENV_BYTES", "0") or 0),
        },
        "lanes": {},
    }

    for lane in selected_lanes:
        spec = LANES[lane]
        print(f"SBC6A lane {lane}: {spec['label']}", flush=True)
        lane_result: dict[str, Any] = {"label": spec["label"], "status": "STARTED", "rows": []}
        try:
            if spec["engine"] == "tesseract":
                predictor, engine_meta = build_tesseract()
            else:
                predictor, engine_meta = build_paddle(lane)
            lane_result["engine"] = engine_meta
            rows: list[dict[str, Any]] = []
            for index, fixture in enumerate(fixtures, 1):
                image_path = fixtures_dir / fixture["image"]
                started = time.perf_counter()
                with MemorySampler() as mem:
                    output = predictor(image_path)
                latency_ms = (time.perf_counter() - started) * 1000.0
                predicted = extract_factual_candidates(output.lines)
                mean_conf = mean_or_none(output.confidences)
                scored = score_fixture(fixture["truth"], predicted, mean_conf)
                row = {
                    "fixture_id": fixture["fixture_id"],
                    "condition": fixture["condition"],
                    "latency_ms": round(latency_ms, 3),
                    "peak_rss_bytes": mem.peak,
                    "mean_confidence": round(mean_conf, 6) if mean_conf is not None else None,
                    "predicted": {key: value for key, value in predicted.items() if key != "raw_text"},
                    "raw_text": predicted["raw_text"],
                    **scored,
                }
                rows.append(row)
                print(
                    f"  {lane} {index:02d}/48 {fixture['fixture_id']} critical={'PASS' if row['critical_match'] else 'FAIL'} {latency_ms:.0f}ms",
                    flush=True,
                )
            lane_result["rows"] = rows
            lane_result["summary"] = summarize(rows)
            lane_result["gate"] = gate(lane_result["summary"])
            lane_result["status"] = "COMPLETE"
        except Exception as exc:
            lane_result["status"] = "BLOCKED"
            lane_result["error"] = f"{type(exc).__name__}: {exc}"
            print(f"  {lane} BLOCKED: {lane_result['error']}", flush=True)
        result["lanes"][lane] = lane_result

    complete = [lane for lane in selected_lanes if result["lanes"][lane]["status"] == "COMPLETE"]
    blocked = [lane for lane in selected_lanes if result["lanes"][lane]["status"] != "COMPLETE"]
    earned = [lane for lane in complete if result["lanes"][lane].get("gate", {}).get("pass")]
    result["comparator"] = {
        "requested_lanes": selected_lanes,
        "complete_lanes": complete,
        "blocked_lanes": blocked,
        "lanes_earning_quality_resource_gate": earned,
        "offline_proof_complete": bool(args.offline_proof and not blocked),
    }

    if blocked:
        classification = "BLOCKED_BENCHMARK_ENVIRONMENT_NOT_A_MODEL_RESULT"
    elif args.offline_proof:
        classification = "OFFLINE_RERUN_COMPLETE"
    else:
        classification = "ONLINE_COMPARATOR_COMPLETE_OFFLINE_PROOF_PENDING"
    result["classification"] = classification

    result_path = output_dir / "SBC6A_OCR_BENCHMARK_RESULT_2026-09-09.json"
    result_path.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    lines = [
        "Shark Books Community - SBC-6A OCR benchmark",
        f"classification: {classification}",
        f"fixtures: {len(fixtures)}",
        f"offline_proof_requested: {args.offline_proof}",
    ]
    for lane in selected_lanes:
        lane_result = result["lanes"][lane]
        if lane_result["status"] != "COMPLETE":
            lines.append(f"{lane}: BLOCKED - {lane_result.get('error')}")
            continue
        s = lane_result["summary"]
        lines.append(
            f"{lane}: gate={'PASS' if lane_result['gate']['pass'] else 'FAIL'} "
            f"critical={s['critical_match_rate']:.3f} amount={s['field_match_rate']['total_pence']:.3f} "
            f"p95_ms={s['latency_ms']['p95']:.1f} peak_rss_mib={s['peak_rss_bytes'] / 1024**2:.1f} "
            f"false_confident_amount={s['false_confident_amount_count']}"
        )
        if lane_result["gate"]["reasons"]:
            lines.append("  reasons: " + "; ".join(lane_result["gate"]["reasons"]))
    summary_path = output_dir / "SBC6A_OCR_BENCHMARK_SUMMARY_2026-09-09.txt"
    summary_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n" + "\n".join(lines), flush=True)
    print(f"SBC6A_RESULT_JSON={result_path}", flush=True)
    return 0 if not blocked else 2


if __name__ == "__main__":
    raise SystemExit(main())
