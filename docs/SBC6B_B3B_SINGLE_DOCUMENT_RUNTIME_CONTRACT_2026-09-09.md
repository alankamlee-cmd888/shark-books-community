# SBC-6B B3B — Single-document local OCR runtime/adapter boundary

**Date:** 9 September 2026
**Programme:** Shark Books Community
**Batch:** SBC-6 + SBC-7 bounded batch
**Entry main:** `d304ea3093c3ed6fbc8041ac2a071f8585bc67ea`

## Purpose

B3A proved that the selected direct PP-OCRv6 Tiny ONNX path can be distributed on Windows without a system Python installation. B3A is still a benchmark executable, not an application adapter.

B3B proves the next narrow boundary before any Tauri/native workspace change:

> one already-native-selected receipt document -> integrity-bound local sidecar -> bounded factual machine result compatible with the B2 OCR contract.

B3B deliberately does **not** add a Tauri command, webview filesystem path, installer, accounting action, receipt matching action or SBC-7 UI.

## Frozen OCR implementation

B3B imports the already-proven `research/sbc6_ocr_runtime/direct_onnx_parity.py` module and uses its `DirectTinyOcr` implementation without modifying that frozen B1/B3A source.

Frozen runtime/model identity remains:

- `onnxruntime==1.23.2`
- `numpy==2.3.5`
- `opencv-contrib-python==4.10.0.84`
- `pyclipper==1.4.0`
- `PyYAML==6.0.2`
- detector `PP-OCRv6_tiny_det`
- recogniser `PP-OCRv6_tiny_rec`
- detector ONNX SHA-256 `193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8`
- detector config SHA-256 `3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593`
- recogniser ONNX SHA-256 `9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6`
- recogniser config SHA-256 `66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1`

## Single-document IPC

The sidecar accepts only:

- one native-host input path;
- expected SHA-256;
- expected byte length.

The model directories are resolved internally from package-local `models/det` and `models/rec`. There is no CLI model path, URL, database path, storage-root path or output path.

The input path is an internal native-host transport detail for B3B. It is **not** a proposed webview/Tauri command field. B3C must provide it only after native document selection/read/integrity handling.

Before importing ONNX Runtime or loading models the sidecar reads the selected document bytes under a 25 MiB hard ceiling, verifies expected byte length and SHA-256, and fails with `integrity_mismatch` on any mismatch.

## Output protocol

Stdout contains exactly one JSON object using schema `sbc6b.single_document.v1`. Diagnostics are redirected to stderr.

Completed output contains status, observed document SHA-256/length, fixed factual OCR provenance, bounded raw text, factual merchant/date/total/currency/reference candidates, mean OCR confidence evidence and bounded warnings. Stable text regions are empty in this first adapter because the proven direct engine currently exposes text/scores rather than a frozen region contract.

Failure output contains `status=failed` plus one bounded failure kind. B3B proves `integrity_mismatch`, `unsupported_document` and `engine_failure`. Timeout is a native-host responsibility and is proven at B3C when Rust owns child-process lifecycle.

## Factual-only boundary

The sidecar may not emit or decide bookkeeping category, business/private status, deductibility, VAT/tax treatment, posting plan, bank-match confirmation, reconciliation state, tax return or HMRC action.

## Windows proof

The B3B Windows proof must preserve a clean repository, exact bounded branch diff and both Cargo locks; build a self-contained PyInstaller onedir single-document executable; copy/reverify the exact selected model hashes; run with System32-only PATH and no system `python`/`py` resolvable; process all 48 frozen receipts one document per sidecar process; retain >=95% critical date+amount accuracy, >=97% exact-pence amount accuracy, every condition >=80%, <=1 false-confident wrong amount, p95 end-to-end sidecar latency <=8 seconds/document and peak process-tree RSS <=2 GiB; prove wrong length/wrong SHA fail before OCR runtime load; prove a non-image input fails as `unsupported_document`; keep PaddleOCR/PaddleX absent and retain no-network/telemetry controls; and record package manifest/size/executable hash/result evidence.

## Hard stop

B3B PASS authorises design of B3C native/Tauri integration. It does not itself authorise exposing file paths to the webview or bypassing SBC-1G dependency/native-change controls.
