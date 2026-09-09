#!/usr/bin/env python3
"""SBC-6B B1 direct-ONNX PP-OCRv6 tiny parity proof.

Research/proof only. This script intentionally does NOT import PaddleOCR or
PaddleX. It adapts the narrowly required PP-OCRv6 preprocessing/postprocessing
algorithms from PaddleX 3.7 under Apache-2.0 for parity testing.

Upstream algorithm references:
- PaddleX paddlex/inference/models/text_detection/processors.py
- PaddleX paddlex/inference/pipelines/components/common/sort_boxes.py
- PaddleX paddlex/inference/pipelines/components/common/crop_image_regions.py
- PaddleX paddlex/inference/models/text_recognition/processors.py
- PaddleX paddlex/configs/pipelines/OCR.yaml

Copyright (c) PaddlePaddle Authors, Apache License 2.0.
Shark-owned orchestration, evidence, hashing, guards and scoring glue are
Copyright (c) MTD Shark contributors, Apache License 2.0.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata as md
import importlib.util
import json
import math
import os
import socket
import sys
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Any

# Privacy controls must be established before ONNX Runtime is imported.
os.environ["ORT_DISABLE_TELEMETRY"] = "1"
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.dont_write_bytecode = True

EXPECTED_PACKAGES = {
    "onnxruntime": "1.23.2",
    "numpy": "2.3.5",
    "opencv-contrib-python": "4.10.0.84",
    "pyclipper": "1.4.0",
    "PyYAML": "6.0.2",
}

DET_MODEL = "PP-OCRv6_tiny_det"
REC_MODEL = "PP-OCRv6_tiny_rec"
EXPECTED = {
    "det_onnx": "193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8",
    "det_yml": "3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593",
    "rec_onnx": "9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6",
    "rec_yml": "66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1",
}

DET_LIMIT_SIDE_LEN = 64
DET_LIMIT_TYPE = "min"
DET_MAX_SIDE_LIMIT = 4000
DET_THRESH = 0.3
DET_BOX_THRESH = 0.6
DET_UNCLIP_RATIO = 1.5
REC_IMAGE_SHAPE = (3, 48, 320)
REC_MAX_IMAGE_WIDTH = 3200
REC_BATCH_SIZE = 6
REC_SCORE_THRESH = 0.0

CRITICAL_TARGET = 0.95
AMOUNT_TARGET = 0.97
CONDITION_CRITICAL_FLOOR = 0.80
FALSE_CONFIDENT_AMOUNT_MAX = 1
LATENCY_P95_MAX_MS = 8000.0
MEMORY_OWNER_DECISION_BYTES = 2 * 1024**3


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


@contextmanager
def deny_network() -> Any:
    """Deny Python TCP connection surfaces for the entire direct OCR run."""
    original_connect = socket.socket.connect
    original_connect_ex = socket.socket.connect_ex
    original_create_connection = socket.create_connection

    def blocked_connect(self: socket.socket, address: Any) -> None:
        raise RuntimeError(f"network access blocked by SBC-6B B1 guard: {address!r}")

    def blocked_connect_ex(self: socket.socket, address: Any) -> int:
        raise RuntimeError(f"network access blocked by SBC-6B B1 guard: {address!r}")

    def blocked_create_connection(*args: Any, **kwargs: Any) -> None:
        raise RuntimeError("network access blocked by SBC-6B B1 guard")

    socket.socket.connect = blocked_connect  # type: ignore[assignment]
    socket.socket.connect_ex = blocked_connect_ex  # type: ignore[assignment]
    socket.create_connection = blocked_create_connection  # type: ignore[assignment]
    try:
        yield
    finally:
        socket.socket.connect = original_connect  # type: ignore[assignment]
        socket.socket.connect_ex = original_connect_ex  # type: ignore[assignment]
        socket.create_connection = original_create_connection  # type: ignore[assignment]


def require_packages() -> dict[str, str]:
    versions: dict[str, str] = {}
    for package, expected in EXPECTED_PACKAGES.items():
        try:
            actual = md.version(package)
        except md.PackageNotFoundError:
            fail(f"required direct-parity package missing: {package}")
        if actual != expected:
            fail(f"direct-parity package drift: {package} expected {expected}, got {actual}")
        versions[package] = actual
    passed("Direct-ONNX parity package versions are exact")
    return versions


def require_model_files(det_dir: Path, rec_dir: Path) -> dict[str, Any]:
    paths = {
        "det_onnx": det_dir / "inference.onnx",
        "det_yml": det_dir / "inference.yml",
        "rec_onnx": rec_dir / "inference.onnx",
        "rec_yml": rec_dir / "inference.yml",
    }
    evidence: dict[str, Any] = {}
    for key, path in paths.items():
        if not path.is_file():
            fail(f"required selected model file missing: {path}")
        actual = sha256(path)
        if actual != EXPECTED[key]:
            fail(f"selected model hash drift: {key} expected {EXPECTED[key]}, got {actual}")
        evidence[key] = {"path": str(path), "sha256": actual, "size_bytes": path.stat().st_size}
    passed("Selected tiny detector/recogniser ONNX and config hashes are exact")
    return evidence


def load_scoring_module(repo: Path) -> Any:
    path = repo / "research" / "sbc6_ocr_benchmark" / "run_benchmark.py"
    spec = importlib.util.spec_from_file_location("sbc6a_scoring", path)
    if spec is None or spec.loader is None:
        fail("could not load frozen SBC-6A scoring module")
    module = importlib.util.module_from_spec(spec)
    sys.modules["sbc6a_scoring"] = module
    spec.loader.exec_module(module)
    if "paddleocr" in sys.modules or "paddlex" in sys.modules:
        fail("frozen scoring helper unexpectedly imported PaddleOCR/PaddleX")
    return module


def read_recognition_config(yml_path: Path) -> tuple[list[str], tuple[int, int, int]]:
    import yaml

    cfg = yaml.safe_load(yml_path.read_text(encoding="utf-8"))
    if not isinstance(cfg, dict):
        fail("recogniser inference.yml is not a mapping")
    global_cfg = cfg.get("Global") or {}
    if global_cfg.get("model_name") != REC_MODEL:
        fail(f"recogniser model config identity mismatch: {global_cfg.get('model_name')!r}")

    post = cfg.get("PostProcess") or {}
    chars = post.get("character_dict")
    if not isinstance(chars, list) or not chars or not all(isinstance(x, str) for x in chars):
        fail("recogniser character_dict missing or malformed")
    character = ["blank", *chars, " "]

    shape: tuple[int, int, int] | None = None
    pre = cfg.get("PreProcess") or {}
    for op in pre.get("transform_ops") or []:
        if isinstance(op, dict) and isinstance(op.get("RecResizeImg"), dict):
            candidate = op["RecResizeImg"].get("image_shape")
            if isinstance(candidate, list) and len(candidate) >= 3 and all(isinstance(x, int) for x in candidate[:3]):
                shape = tuple(candidate[:3])  # type: ignore[assignment]
                break
    if shape != REC_IMAGE_SHAPE:
        fail(f"recogniser image-shape drift: expected {REC_IMAGE_SHAPE}, got {shape}")
    passed("Recognizer character dictionary and [3,48,320] preprocessing contract loaded")
    return character, shape


def read_detection_config(yml_path: Path) -> None:
    import yaml

    cfg = yaml.safe_load(yml_path.read_text(encoding="utf-8"))
    if not isinstance(cfg, dict):
        fail("detector inference.yml is not a mapping")
    global_cfg = cfg.get("Global") or {}
    if global_cfg.get("model_name") != DET_MODEL:
        fail(f"detector model config identity mismatch: {global_cfg.get('model_name')!r}")
    passed("Detector embedded model identity matches frozen tiny detector")


def resize_det(img: Any, cv2: Any, np: Any) -> tuple[Any, tuple[float, float]]:
    h, w, _ = img.shape
    ratio = 1.0
    if DET_LIMIT_TYPE != "min":
        fail("internal detector limit policy drift")
    if min(h, w) < DET_LIMIT_SIDE_LEN:
        ratio = float(DET_LIMIT_SIDE_LEN) / float(min(h, w))
    resize_h = int(h * ratio)
    resize_w = int(w * ratio)
    if max(resize_h, resize_w) > DET_MAX_SIDE_LIMIT:
        ratio2 = float(DET_MAX_SIDE_LIMIT) / float(max(resize_h, resize_w))
        resize_h = int(resize_h * ratio2)
        resize_w = int(resize_w * ratio2)
    resize_h = max(int(round(resize_h / 32) * 32), 32)
    resize_w = max(int(round(resize_w / 32) * 32), 32)
    if resize_h != h or resize_w != w:
        img = cv2.resize(img, (resize_w, resize_h))
    return img, (resize_h / float(h), resize_w / float(w))


def normalize_det(img: Any, np: Any) -> Any:
    img = img.astype(np.float32)
    img *= 1.0 / 255.0
    mean = np.asarray([0.485, 0.456, 0.406], dtype=np.float32)
    std = np.asarray([0.229, 0.224, 0.225], dtype=np.float32)
    img = (img - mean) / std
    return np.transpose(img, (2, 0, 1))[None, ...].astype(np.float32, copy=False)


def mini_boxes(contour: Any, cv2: Any) -> tuple[list[Any], float]:
    bounding = cv2.minAreaRect(contour)
    points = sorted(list(cv2.boxPoints(bounding)), key=lambda x: x[0])
    if points[1][1] > points[0][1]:
        i1, i4 = 0, 1
    else:
        i1, i4 = 1, 0
    if points[3][1] > points[2][1]:
        i2, i3 = 2, 3
    else:
        i2, i3 = 3, 2
    return [points[i1], points[i2], points[i3], points[i4]], min(bounding[1])


def box_score_fast(bitmap: Any, box: Any, cv2: Any, np: Any) -> float:
    h, w = bitmap.shape[:2]
    b = box.copy()
    xmin = max(0, min(math.floor(float(b[:, 0].min())), w - 1))
    xmax = max(0, min(math.ceil(float(b[:, 0].max())), w - 1))
    ymin = max(0, min(math.floor(float(b[:, 1].min())), h - 1))
    ymax = max(0, min(math.ceil(float(b[:, 1].max())), h - 1))
    mask = np.zeros((ymax - ymin + 1, xmax - xmin + 1), dtype=np.uint8)
    b[:, 0] -= xmin
    b[:, 1] -= ymin
    cv2.fillPoly(mask, b.reshape(1, -1, 2).astype(np.int32), 1)
    return float(cv2.mean(bitmap[ymin : ymax + 1, xmin : xmax + 1], mask)[0])


def unclip(box: Any, ratio: float, cv2: Any, np: Any, pyclipper: Any) -> Any:
    area = float(cv2.contourArea(box))
    length = float(cv2.arcLength(box, True))
    if length <= 0:
        return np.asarray([])
    distance = area * ratio / length
    offset = pyclipper.PyclipperOffset()
    offset.AddPath(box, pyclipper.JT_ROUND, pyclipper.ET_CLOSEDPOLYGON)
    expanded = offset.Execute(distance)
    if not expanded:
        return np.asarray([])
    return np.asarray(expanded)


def postprocess_det(pred: Any, src_h: int, src_w: int, cv2: Any, np: Any, pyclipper: Any) -> list[Any]:
    if pred.ndim != 2:
        fail(f"detector probability map must be 2D, got shape {pred.shape}")
    bitmap = (pred > DET_THRESH).astype(np.uint8)
    height, width = bitmap.shape
    width_scale = src_w / float(width)
    height_scale = src_h / float(height)
    contours, _ = cv2.findContours(bitmap * 255, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE)
    boxes: list[Any] = []
    for contour in contours[:1000]:
        points, sside = mini_boxes(contour, cv2)
        if sside < 3:
            continue
        points_arr = np.asarray(points, dtype=np.float32)
        score = box_score_fast(pred, points_arr.reshape(-1, 2), cv2, np)
        if DET_BOX_THRESH > score:
            continue
        expanded = unclip(points_arr, DET_UNCLIP_RATIO, cv2, np, pyclipper)
        if expanded.size == 0:
            continue
        try:
            expanded = expanded.reshape(-1, 1, 2)
        except ValueError:
            continue
        box, sside = mini_boxes(expanded, cv2)
        if sside < 5:
            continue
        box_arr = np.asarray(box, dtype=np.float32)
        for i in range(box_arr.shape[0]):
            box_arr[i, 0] = max(0, min(round(float(box_arr[i, 0]) * width_scale), src_w))
            box_arr[i, 1] = max(0, min(round(float(box_arr[i, 1]) * height_scale), src_h))
        boxes.append(box_arr.astype(np.int16))
    return boxes


def sort_quad_boxes(boxes: list[Any]) -> list[Any]:
    out = list(sorted(boxes, key=lambda x: (x[0][1], x[0][0])))
    for i in range(len(out) - 1):
        for j in range(i, -1, -1):
            if abs(int(out[j + 1][0][1]) - int(out[j][0][1])) < 10 and int(out[j + 1][0][0]) < int(out[j][0][0]):
                out[j], out[j + 1] = out[j + 1], out[j]
            else:
                break
    return out


def crop_quad(img: Any, points: Any, cv2: Any, np: Any) -> Any:
    bounding = cv2.minAreaRect(np.asarray(points).astype(np.int32))
    pts = sorted(list(cv2.boxPoints(bounding)), key=lambda x: x[0])
    if pts[1][1] > pts[0][1]:
        ia, id_ = 0, 1
    else:
        ia, id_ = 1, 0
    if pts[3][1] > pts[2][1]:
        ib, ic = 2, 3
    else:
        ib, ic = 3, 2
    ordered = np.asarray([pts[ia], pts[ib], pts[ic], pts[id_]], dtype=np.float32)
    crop_w = int(max(np.linalg.norm(ordered[0] - ordered[1]), np.linalg.norm(ordered[2] - ordered[3])))
    crop_h = int(max(np.linalg.norm(ordered[0] - ordered[3]), np.linalg.norm(ordered[1] - ordered[2])))
    if crop_w <= 0 or crop_h <= 0:
        return np.asarray([], dtype=np.uint8)
    target = np.float32([[0, 0], [crop_w, 0], [crop_w, crop_h], [0, crop_h]])
    matrix = cv2.getPerspectiveTransform(ordered, target)
    cropped = cv2.warpPerspective(img, matrix, (crop_w, crop_h), borderMode=cv2.BORDER_REPLICATE, flags=cv2.INTER_CUBIC)
    if cropped.shape[0] * 1.0 / cropped.shape[1] >= 1.5:
        cropped = np.rot90(cropped)
    return cropped


def preprocess_rec_crop(img_bgr: Any, cv2: Any, np: Any) -> Any:
    if img_bgr.ndim != 3 or img_bgr.shape[0] <= 0 or img_bgr.shape[1] <= 0:
        fail(f"invalid recognition crop shape: {getattr(img_bgr, 'shape', None)}")
    img = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2RGB)
    _, img_h, base_w = REC_IMAGE_SHAPE
    ratio = img.shape[1] / float(img.shape[0])
    max_wh_ratio = max(base_w / float(img_h), ratio)
    img_w = min(int(img_h * max_wh_ratio), REC_MAX_IMAGE_WIDTH)
    resized_w = img_w if math.ceil(img_h * ratio) > img_w else int(math.ceil(img_h * ratio))
    resized_w = max(1, resized_w)
    resized = cv2.resize(img, (resized_w, img_h)).astype(np.float32)
    resized = resized.transpose((2, 0, 1)) / 255.0
    resized -= 0.5
    resized /= 0.5
    padded = np.zeros((3, img_h, img_w), dtype=np.float32)
    padded[:, :, :resized_w] = resized
    return padded


def batch_pad(images: list[Any], np: Any) -> Any:
    max_width = max(int(img.shape[2]) for img in images)
    padded = []
    for img in images:
        pad_width = max_width - int(img.shape[2])
        if pad_width:
            img = np.pad(img, ((0, 0), (0, 0), (0, pad_width)), mode="constant", constant_values=0)
        padded.append(img)
    return np.stack(padded, axis=0).astype(np.float32, copy=False)


def ctc_decode(batch_pred: Any, characters: list[str], np: Any) -> list[tuple[str, float]]:
    if batch_pred.ndim != 3:
        fail(f"recogniser output must be [B,T,C], got shape {batch_pred.shape}")
    idx = batch_pred.argmax(axis=-1)
    prob = batch_pred.max(axis=-1)
    out: list[tuple[str, float]] = []
    for row_idx, row in enumerate(idx):
        selection = np.ones(len(row), dtype=bool)
        if len(row) > 1:
            selection[1:] = row[1:] != row[:-1]
        selection &= row != 0
        selected_ids = row[selection]
        if any(int(i) >= len(characters) or int(i) < 0 for i in selected_ids):
            fail("recogniser class index exceeds frozen character dictionary")
        text = "".join(characters[int(i)] for i in selected_ids)
        selected_probs = prob[row_idx][selection]
        score = float(selected_probs.mean()) if len(selected_probs) else 0.0
        out.append((text, score))
    return out


class DirectTinyOcr:
    def __init__(self, det_dir: Path, rec_dir: Path, characters: list[str], ort: Any, cv2: Any, np: Any, pyclipper: Any):
        self.det_dir = det_dir
        self.rec_dir = rec_dir
        self.characters = characters
        self.ort = ort
        self.cv2 = cv2
        self.np = np
        self.pyclipper = pyclipper
        threads = max(1, min(4, os.cpu_count() or 2))
        options = ort.SessionOptions()
        options.intra_op_num_threads = threads
        options.inter_op_num_threads = 1
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        self.det_session = ort.InferenceSession(str(det_dir / "inference.onnx"), sess_options=options, providers=["CPUExecutionProvider"])
        self.rec_session = ort.InferenceSession(str(rec_dir / "inference.onnx"), sess_options=options, providers=["CPUExecutionProvider"])
        self.det_input = self.det_session.get_inputs()[0].name
        self.rec_input = self.rec_session.get_inputs()[0].name
        if self.det_session.get_providers() != ["CPUExecutionProvider"]:
            fail(f"detector provider drift: {self.det_session.get_providers()}")
        if self.rec_session.get_providers() != ["CPUExecutionProvider"]:
            fail(f"recogniser provider drift: {self.rec_session.get_providers()}")

    def predict(self, path: Path) -> tuple[list[str], list[float]]:
        cv2, np = self.cv2, self.np
        img = cv2.imread(str(path), cv2.IMREAD_COLOR)
        if img is None or img.ndim != 3:
            fail(f"failed to read receipt image as BGR: {path}")
        src_h, src_w = int(img.shape[0]), int(img.shape[1])
        det_img, _ = resize_det(img, cv2, np)
        det_input = normalize_det(det_img, np)
        outputs = self.det_session.run(None, {self.det_input: det_input})
        if not outputs:
            fail("detector ONNX session returned no outputs")
        det_out = np.asarray(outputs[0])
        if det_out.ndim != 4 or det_out.shape[0] != 1 or det_out.shape[1] != 1:
            fail(f"detector output shape drift: {det_out.shape}")
        boxes = sort_quad_boxes(postprocess_det(det_out[0, 0], src_h, src_w, cv2, np, self.pyclipper))
        crops: list[Any] = []
        for box in boxes:
            crop = crop_quad(img, box, cv2, np)
            if getattr(crop, "size", 0) and crop.shape[0] > 0 and crop.shape[1] > 0:
                crops.append(crop)
        if not crops:
            return [], []
        infos = [{"id": i, "ratio": crop.shape[1] / float(crop.shape[0])} for i, crop in enumerate(crops)]
        sorted_infos = sorted(infos, key=lambda x: x["ratio"])
        texts_by_id: dict[int, tuple[str, float]] = {}
        for start in range(0, len(sorted_infos), REC_BATCH_SIZE):
            chunk = sorted_infos[start : start + REC_BATCH_SIZE]
            pre = [preprocess_rec_crop(crops[item["id"]], cv2, np) for item in chunk]
            batch = batch_pad(pre, np)
            outputs = self.rec_session.run(None, {self.rec_input: batch})
            if not outputs:
                fail("recogniser ONNX session returned no outputs")
            decoded = ctc_decode(np.asarray(outputs[0]), self.characters, np)
            if len(decoded) != len(chunk):
                fail("recogniser batch result count mismatch")
            for item, result in zip(chunk, decoded):
                texts_by_id[item["id"]] = result
        lines: list[str] = []
        scores: list[float] = []
        for idx in range(len(crops)):
            text, score = texts_by_id[idx]
            if score >= REC_SCORE_THRESH:
                lines.append(text)
                scores.append(score)
        return lines, scores


def gate(summary: dict[str, Any]) -> dict[str, Any]:
    reasons: list[str] = []
    if summary.get("count") != 48:
        return {"pass": False, "reasons": ["direct lane did not produce all 48 fixture results"]}
    if float(summary["critical_match_rate"]) < CRITICAL_TARGET:
        reasons.append("critical date+amount rate below 95%")
    if float(summary["field_match_rate"]["total_pence"]) < AMOUNT_TARGET:
        reasons.append("amount exact-pence rate below 97%")
    bad_conditions = [name for name, data in summary["by_condition"].items() if float(data["critical_match_rate"]) < CONDITION_CRITICAL_FLOOR]
    if bad_conditions:
        reasons.append("critical-field condition floor failed: " + ", ".join(bad_conditions))
    if int(summary["false_confident_amount_count"]) > FALSE_CONFIDENT_AMOUNT_MAX:
        reasons.append("too many false confident amount substitutions")
    if float(summary["latency_ms"]["p95"]) > LATENCY_P95_MAX_MS:
        reasons.append("CPU p95 exceeds 8 seconds/image")
    if int(summary["peak_rss_bytes"]) > MEMORY_OWNER_DECISION_BYTES:
        reasons.append("peak working set exceeds 2 GiB")
    return {"pass": not reasons, "reasons": reasons}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--fixtures", required=True)
    parser.add_argument("--det-model-dir", required=True)
    parser.add_argument("--rec-model-dir", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    repo = Path(args.repo).resolve()
    fixtures_dir = Path(args.fixtures).resolve()
    det_dir = Path(args.det_model_dir).resolve()
    rec_dir = Path(args.rec_model_dir).resolve()
    output_dir = Path(args.output).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    packages = require_packages()
    model_files = require_model_files(det_dir, rec_dir)
    scorer = load_scoring_module(repo)
    if "paddleocr" in sys.modules or "paddlex" in sys.modules:
        fail("PaddleOCR/PaddleX must not be imported by the direct parity path")

    manifest_path = fixtures_dir / "manifest.json"
    if not manifest_path.is_file():
        fail(f"fixture manifest missing: {manifest_path}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    fixtures = manifest.get("fixtures", [])
    if manifest.get("schema") != "sbc6a.synthetic_receipts.v1" or len(fixtures) != 48:
        fail("synthetic fixture manifest drift")

    with deny_network():
        import numpy as np
        import cv2
        import pyclipper
        import onnxruntime as ort

        if ort.__version__ != EXPECTED_PACKAGES["onnxruntime"]:
            fail(f"onnxruntime module version drift: {ort.__version__}")
        telemetry_disable = getattr(ort, "disable_telemetry_events", None)
        if not callable(telemetry_disable):
            fail("onnxruntime.disable_telemetry_events is unavailable")
        telemetry_disable()
        passed("ONNX Runtime telemetry-disable API called before session construction")
        read_detection_config(det_dir / "inference.yml")
        characters, _ = read_recognition_config(rec_dir / "inference.yml")
        started = time.perf_counter()
        with scorer.MemorySampler() as model_mem:
            engine = DirectTinyOcr(det_dir, rec_dir, characters, ort, cv2, np, pyclipper)
        model_load_ms = (time.perf_counter() - started) * 1000.0
        passed("Direct tiny ONNX detector and recogniser sessions loaded with CPUExecutionProvider")

        rows: list[dict[str, Any]] = []
        for index, fixture in enumerate(fixtures, 1):
            image_path = fixtures_dir / fixture["image"]
            started = time.perf_counter()
            with scorer.MemorySampler() as mem:
                lines, confidences = engine.predict(image_path)
            latency_ms = (time.perf_counter() - started) * 1000.0
            predicted = scorer.extract_factual_candidates(lines)
            mean_conf = scorer.mean_or_none(confidences)
            scored = scorer.score_fixture(fixture["truth"], predicted, mean_conf)
            row = {"fixture_id": fixture["fixture_id"], "condition": fixture["condition"], "latency_ms": round(latency_ms, 3), "peak_rss_bytes": mem.peak, "mean_confidence": round(mean_conf, 6) if mean_conf is not None else None, "predicted": {k: v for k, v in predicted.items() if k != "raw_text"}, "raw_text": predicted["raw_text"], **scored}
            rows.append(row)
            print(f"  D1 {index:02d}/48 {fixture['fixture_id']} critical={'PASS' if row['critical_match'] else 'FAIL'} {latency_ms:.0f}ms", flush=True)
        if "paddleocr" in sys.modules or "paddlex" in sys.modules:
            fail("direct parity execution imported PaddleOCR/PaddleX")

    summary = scorer.summarize(rows)
    direct_gate = gate(summary)
    result = {
        "schema": "sbc6b.direct_onnx_parity.v1",
        "generated_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "git_head_expected_base": "a2d0c776199934d79fe1e1dce1889d3999f44b91",
        "fixture_manifest_sha256": sha256(manifest_path),
        "fixture_count": len(fixtures),
        "packages": packages,
        "model_files": model_files,
        "pipeline_contract": {"det_limit_side_len": DET_LIMIT_SIDE_LEN, "det_limit_type": DET_LIMIT_TYPE, "det_max_side_limit": DET_MAX_SIDE_LIMIT, "det_thresh": DET_THRESH, "det_box_thresh": DET_BOX_THRESH, "det_unclip_ratio": DET_UNCLIP_RATIO, "rec_image_shape": list(REC_IMAGE_SHAPE), "rec_batch_size": REC_BATCH_SIZE, "rec_score_thresh": REC_SCORE_THRESH},
        "privacy": {"ort_disable_telemetry_env": os.environ.get("ORT_DISABLE_TELEMETRY"), "ort_disable_telemetry_api_called": True, "network_guard": "PASS_NO_PYTHON_TCP_CONNECT_ALLOWED_DURING_DIRECT_MODEL_LOAD_AND_ALL_48_PREDICTIONS", "paddleocr_imported": "paddleocr" in sys.modules, "paddlex_imported": "paddlex" in sys.modules},
        "model_load_ms": round(model_load_ms, 3),
        "model_load_peak_rss_bytes": model_mem.peak,
        "rows": rows,
        "summary": summary,
        "gate": direct_gate,
    }
    result_path = output_dir / "SBC6B_B1_DIRECT_ONNX_PARITY.json"
    result_path.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    summary_lines = [
        "Shark Books Community - SBC-6B B1 direct ONNX parity",
        f"gate: {'PASS' if direct_gate['pass'] else 'FAIL'}",
        f"fixtures: {summary.get('count', 0)}",
        f"critical_match_rate: {summary.get('critical_match_rate')}",
        f"amount_match_rate: {summary.get('field_match_rate', {}).get('total_pence')}",
        f"false_confident_amount_count: {summary.get('false_confident_amount_count')}",
        f"p95_ms: {summary.get('latency_ms', {}).get('p95')}",
        f"peak_rss_bytes: {summary.get('peak_rss_bytes')}",
        "paddleocr_imported: false",
        "paddlex_imported: false",
        "telemetry_disable_api_called: true",
        "network_guard: PASS",
    ]
    if direct_gate["reasons"]:
        summary_lines.append("reasons: " + "; ".join(direct_gate["reasons"]))
    (output_dir / "SBC6B_B1_DIRECT_ONNX_PARITY_SUMMARY.txt").write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    print("\n" + "\n".join(summary_lines), flush=True)
    print(f"SBC6B_B1_RESULT_DIR={output_dir}", flush=True)
    return 0 if direct_gate["pass"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
