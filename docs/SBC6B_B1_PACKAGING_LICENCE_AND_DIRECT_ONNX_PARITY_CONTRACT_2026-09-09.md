# Shark Books Community — SBC-6B B1 packaging, licence and direct-ONNX parity contract

Date: 2026-09-09  
Status: B1 CANDIDATE / DIRECT-ONNX PARITY PROOF REQUIRED  
Entry protected main: `a2d0c776199934d79fe1e1dce1889d3999f44b91`  
B0 validated candidate: `646ccbdc5921b9161f15d83d0772fc167602fa74`  
B0 evidence ZIP SHA-256: `bf2efda91ce2e363e0ebdc1f3d8823f9a1d203cdc2bcd508b888dc3071cd5d22`

## 1. B0 adjudication

B0 is PASS.

The returned evidence package proves:

- clean proof checkout at exact validated HEAD;
- Python 3.13.15 proof host;
- PaddleOCR `3.7.0`;
- PaddleX `3.7.2`;
- ONNX Runtime `1.23.2`;
- explicit local `PP-OCRv6_tiny_det_onnx` and `PP-OCRv6_tiny_rec_onnx` cache assets;
- selected tiny logical names bound to selected tiny local directories;
- PaddleX model-source check disabled before model load;
- Python TCP connection surfaces denied during cache resolution/model load/prediction;
- recognised text returned from the deterministic clean receipt;
- repository clean after proof;
- exact model file inventory, Python SBOM and licence-file inventory.

Frozen selected model evidence:

| Asset | Files | Bytes | Tree SHA-256 | ONNX payload SHA-256 |
| --- | ---: | ---: | --- | --- |
| `PP-OCRv6_tiny_det_onnx` | 13 | 2,029,740 | `2ea11de12b6081ec5a5a3c4100aaca1ea53b2c0b16df23f846788dd6226c11c3` | `193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8` |
| `PP-OCRv6_tiny_rec_onnx` | 13 | 4,646,791 | `8f8070cd228da125e7f0324ebd4fa586f6c846e44644316fcf64f816de27334c` | `9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6` |

Frozen config hashes used by the B1 parity spike:

- detector `inference.yml`: `3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593`
- recogniser `inference.yml`: `66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1`

## 2. Why the Stage A/B0 Python environment is rejected as the production package

The B0 environment contains **74 Python distributions** and **150 discovered licence/NOTICE files**. It is valid research evidence, but it is not accepted as the SharkBooks production OCR package.

The full PaddleOCR/PaddleX route carries model-host and general-network dependencies that are unnecessary for bounded local receipt OCR. PaddleX 3.7's required dependency set includes model-source clients such as Hugging Face Hub, ModelScope and AIStudio plus `requests`; PaddleOCR itself also declares `requests` and `aiohttp`. The observed B0 environment additionally contains benchmark-only packages such as `pytesseract` and broader packages not required by the selected local image-only receipt path.

The inventory also includes LGPL-labelled packages (`python-bidi`, `crc32c`). This is not adjudicated here as a prohibition on lawful distribution; rather, avoiding dependencies that are not needed by the selected UK receipt path reduces licence obligations, package size and attack/network surface.

**Decision:** do not ship the Stage A/B0 virtual environment and do not make PaddleOCR/PaddleX a production runtime dependency merely because they were used to select and validate the model.

## 3. B1 production architecture candidate

B1 selects a **Shark-owned direct ONNX adapter** for a parity spike before product wiring.

This is a production-architecture candidate, not yet a production dependency decision.

The direct adapter must:

- use the exact frozen detector and recogniser ONNX payloads above;
- implement only the preprocessing/postprocessing required by the selected PP-OCRv6 tiny OCR path;
- preserve the Stage A OCR-pipeline parameters used by the winning comparator;
- use local files only;
- set `ORT_DISABLE_TELEMETRY=1` before importing ONNX Runtime and call ONNX Runtime's telemetry-disable API where available;
- deny Python TCP connect surfaces for the whole OCR run;
- import neither `paddleocr` nor `paddlex`;
- return factual OCR text/confidence only;
- remain outside `shark-books-core` and outside the frozen accounting facade;
- retain manual receipt/document entry as the fallback.

The parity implementation may adapt narrowly required algorithms from PaddleX/PaddleOCR under Apache-2.0. Any code promoted beyond research must retain copyright/licence attribution and be included in the project NOTICE/SBOM process.

## 4. Initial minimal runtime population candidate

For the direct-ONNX parity spike, the smallest obvious runtime surface is:

- CPython 3.13.x on Windows (proof host 3.13.15; final packaged patch version not frozen by B1);
- `onnxruntime==1.23.2`;
- `numpy==2.3.5`;
- `opencv-contrib-python==4.10.0.84` for parity with the selected benchmark environment;
- `pyclipper==1.4.0` for DB text-detection polygon expansion;
- `PyYAML==6.0.2` to consume the frozen recognition character dictionary/config.

Benchmark-only helpers (`psutil`, Pillow and the synthetic-fixture generator) may exist in the proof environment but are not production runtime requirements.

The direct production candidate explicitly excludes:

- PaddleOCR;
- PaddleX;
- Tesseract/pytesseract;
- Hugging Face Hub;
- ModelScope;
- AIStudio SDK;
- `requests` / `aiohttp` as OCR-runtime requirements;
- `pypdfium2` for the image-only receipt path;
- `python-bidi` for the selected general PP-OCRv6 receipt path;
- pandas;
- cloud OCR or remote model calls.

A later minimal-environment proof must resolve ONNX Runtime's own transitive package population exactly before installer packaging is frozen.

## 5. Stage A behaviour that the direct adapter must reproduce

The winning Stage A `PaddleOCR(...)` configuration supplied the tiny model names and disabled document orientation, unwarping and text-line orientation. The underlying OCR pipeline defaults that matter for parity are frozen here:

Detection:

- BGR input;
- `limit_side_len=64`;
- `limit_type=min`;
- `max_side_limit=4000`;
- `thresh=0.3`;
- `box_thresh=0.6`;
- `unclip_ratio=1.5`;
- DB quad-box postprocessing;
- top-to-bottom / left-to-right quad sorting.

Recognition:

- perspective-cropped detected quads;
- BGR-to-RGB conversion before recognition;
- image shape `[3,48,320]` with dynamic width and zero-padding;
- CTC decode using the frozen character dictionary from the selected recogniser `inference.yml`;
- batch size 6, sorted by crop aspect ratio;
- recognition score threshold 0.0.

## 6. B1 parity gate

Run the direct-ONNX adapter against the same deterministic 48-receipt corpus used by Stage A.

Required PASS conditions:

- all 48 fixtures processed;
- critical date+amount rate **>= 95%**;
- exact-pence amount rate **>= 97%**;
- every degradation condition critical rate **>= 80%**;
- false-confident wrong amount count **<= 1**;
- CPU p95 **<= 8 seconds/image**;
- peak process working set **<= 2 GiB**;
- detector/recogniser ONNX and config hashes exact;
- `paddleocr` and `paddlex` absent from `sys.modules` throughout direct inference;
- process-level Python TCP connect surfaces denied throughout model load and all 48 predictions;
- ONNX Runtime telemetry disable requested before session construction;
- repository remains clean.

For comparison, the Stage A P1 result was:

- critical date+amount: 47/48 = 97.92%;
- exact-pence amount: 47/48 = 97.92%;
- false-confident wrong amounts: 0;
- p95: about 544 ms/image in the online comparator.

B1 may tolerate at most one additional critical-field miss while still satisfying the original quality gate, but it may not fall below the original exact-pence amount gate.

## 7. Hard stops after B1

If direct ONNX parity fails, do **not** silently lower quality thresholds and do **not** immediately ship the full 74-package research environment. Diagnose the exact divergence and decide whether a small bounded compatibility layer is justified.

If direct ONNX parity passes, proceed to:

1. freeze the direct runtime dependency/licence manifest;
2. add the implementation-neutral Shark OCR request/result contract;
3. build the smallest Windows local adapter behind that contract;
4. rerun the 48-receipt corpus in a newly created minimal runtime environment;
5. only then introduce installer/product wiring under the permanent dependency-change controls.

SBC-7 owner-facing IA/workflow work may continue in parallel, but UI implementation remains blocked until SBC-6 closes PASS or OCR is explicitly deferred.