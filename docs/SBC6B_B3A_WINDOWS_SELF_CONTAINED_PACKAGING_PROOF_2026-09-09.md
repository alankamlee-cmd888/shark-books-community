# SBC-6B B3A — Windows self-contained direct-ONNX packaging proof

**Date:** 9 September 2026
**Programme:** Shark Books Community
**Batch:** SBC-6 + SBC-7 bounded batch
**Stage:** B3A — Windows OCR packaging feasibility

## Purpose

B1 proved that the selected PP-OCRv6 Tiny detector/recogniser can reproduce the frozen 48-receipt quality gate through a Shark-owned direct ONNX Runtime path, without PaddleOCR or PaddleX. B2 then froze a dependency-free Shark-owned factual OCR application contract.

B3A answers one narrow release-feasibility question before product/runtime wiring:

> Can the already-proven direct-ONNX proof be frozen into a self-contained Windows onedir executable package that runs with no system Python on PATH, uses only the exact selected local model files, retains the frozen 48-receipt quality gate, and preserves the no-network runtime guard?

This is a packaging proof only. It does not add a Tauri command, product adapter, installer, accounting action, receipt matching action, UI or Apple runtime.

## Frozen input

Entry protected main: B2 merge.

OCR engine/model behaviour is inherited unchanged from `research/sbc6_ocr_runtime/direct_onnx_parity.py`. The B3A branch is not permitted to modify that file.

Exact OCR/runtime packages inside the isolated packaging-build environment:

- `onnxruntime==1.23.2`
- `numpy==2.3.5`
- `opencv-contrib-python==4.10.0.84`
- `pyclipper==1.4.0`
- `PyYAML==6.0.2`

Proof-only packages required by the frozen scoring/fixture harness:

- `psutil==7.2.2`
- `pillow==12.3.0`

Packaging tool:

- `pyinstaller==6.22.2`

PyInstaller is a build-time tool only. Its generated bundle is evidence, not yet the accepted SharkBooks production distribution.

## Packaging form

B3A uses PyInstaller **onedir**, not onefile. One-dir is intentionally preferred for the proof because it avoids onefile extraction/startup behaviour and leaves the native dependency population inspectable.

The proof package contains:

- frozen `shark-ocr-direct-proof.exe` and its PyInstaller support tree;
- exact copies of the selected detector/recogniser `inference.onnx` and `inference.yml` files under a package-local `models/` tree;
- a proof-only copy of the frozen Stage A scoring module under `support/`.

The selected model hashes must remain the B1 values:

- detector ONNX: `193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8`
- detector config: `3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593`
- recogniser ONNX: `9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6`
- recogniser config: `66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1`

## Required runtime proof

The frozen executable must run with an intentionally restricted environment:

- absolute executable path;
- `PATH` reduced to Windows system directories only;
- `PYTHONHOME` and `PYTHONPATH` removed;
- `where python` and `where py` must not resolve through the restricted PATH;
- package-local selected model copies only;
- all 48 frozen synthetic receipt fixtures;
- same B1 quality/resource gates;
- B1 Python TCP deny guard active throughout ONNX Runtime model load and prediction;
- ONNX Runtime telemetry disabled before session construction;
- PaddleOCR and PaddleX absent.

The proof does not claim that process-level Python socket interception is a universal operating-system egress sandbox. It proves that the selected frozen Python OCR path itself has no permitted Python TCP connection surface while document OCR executes. Release-level installer/firewall/sandbox decisions remain a later hardening concern.

## Quality gates

No threshold changes are permitted:

- 48/48 fixture execution required;
- critical date+amount >= 95%;
- exact-pence amount >= 97%;
- every degradation condition critical >= 80%;
- false-confident wrong amount <= 1;
- CPU p95 <= 8 seconds/image;
- peak working set <= 2 GiB.

Bundle size and file count are recorded but are **not** post-hoc pass/fail thresholds. They are adjudication evidence for B3B/installer design.

## Hard stop

B3A PASS does not itself authorise Tauri/native OCR wiring. After B3A evidence is returned, the bundle size/dependency manifest and runtime behaviour must be adjudicated. Only then may B3B promote the narrow direct-ONNX implementation behind the B2 Shark OCR contract.
