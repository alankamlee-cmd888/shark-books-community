# Shark Books Community — SBC-6B OCR runtime entry contract

Date: 2026-09-09  
Status: STAGE B ENTRY / P1 RUNTIME INVENTORY REQUIRED BEFORE PRODUCT WIRING  
Entry protected main: `ce69110dd1683ab8561b91f1dfff35df0354900d`  
Stage A decision: PASS_SELECT_PP_OCRV6_TINY

## 1. Selected runtime

Only the Stage A winner is authorised for SBC-6B:

- PaddleOCR `3.7.0`
- ONNX Runtime `1.23.2`
- `PP-OCRv6_tiny_det`
- `PP-OCRv6_tiny_rec`
- local CPU inference

No Tesseract production fallback, PP-OCRv6 small/medium, PaddleOCR-VL, VLM, cloud OCR or model shopping is authorised.

## 2. Why Stage B begins with an inventory gate

The Stage A benchmark environment measured about 540 MiB and includes many transitive Python/network-client packages. That benchmark environment is evidence, not an accepted SharkBooks installer design.

Before product wiring, SBC-6B must freeze:

1. exact selected model file hashes and sizes;
2. exact Python/runtime package population;
3. licence/NOTICE evidence for the runtime population;
4. explicit local model-directory loading with model-host lookup disabled;
5. a smoke proof with socket connection attempts blocked during model load and OCR;
6. a packaging decision that keeps OCR removable and does not place OCR/Python/ONNX types inside `shark-books-core`.

This prevents the research environment from silently becoming the production dependency architecture.

## 3. Runtime boundary

The preferred production boundary remains an optional, Shark-owned local OCR adapter.

Input:
- a user-selected local document/image reference supplied by the platform adapter;
- no arbitrary database path;
- no books encryption key;
- no accounting/posting instruction.

Output schema remains factual only:

```text
OcrExtraction
  schema_version
  engine_id
  engine_version
  model_ids[]
  document_sha256
  raw_text
  regions[]
  candidates
    merchant_text?
    document_date?
    total_pence?
    currency?
    reference?
  warnings[]
```

OCR may not persist, post, clear, reconcile, classify, infer deductibility/VAT/CIS/payroll/company treatment, or confirm a bank match.

## 4. Offline/runtime rules

Production must use explicit local model directories. It must not depend on PaddleX/PaddleOCR model-host discovery at document-processing time.

Runtime/model download, if any, is an installation/update concern and must be distinguishable from document processing. Document bytes must never be sent to model hosts or Shark.

OCR unavailable/failed must degrade to the already-proven manual document path.

## 5. Platform position

Windows is the first executable production-runtime proof surface.

Apple/iOS accounting and document flows must continue to compile/work even if OCR is not yet available on that platform. SBC-6 may not force a Python subprocess requirement into iOS. The common Shark OCR interface must therefore remain implementation-neutral and optional.

A later Stage D adjudication must decide whether P1 is packaged on Apple through a compatible local ONNX path or remains unavailable there in V1 with manual-receipt fallback. That decision must not rewrite accounting logic.

## 6. B0 inventory evidence

The committed B0 inventory harness uses the already-proven SBC-6A isolated environment; it installs nothing and changes no product/native source.

It must:
- require exact `paddleocr==3.7.0` and `onnxruntime==1.23.2`;
- resolve only the selected P1 ONNX cache assets `PP-OCRv6_tiny_det_onnx` and `PP-OCRv6_tiny_rec_onnx` for the frozen ONNX Runtime lane;
- derive the PaddleX cache root from the installed runtime and use only bounded local cache discovery, with no download or model shopping;
- record the physical cache directory actually used;
- hash every selected model file;
- enumerate installed Python distributions and licence metadata/files;
- validate each cached directory against its embedded PaddleX logical model name before pipeline construction;
- instantiate P1 with both the explicit selected logical names and explicit local directories;
- disable Paddle model-source checks;
- deny socket connection calls during import/model load/prediction;
- run one deterministic clean synthetic receipt smoke;
- leave the repository clean.

## 7. B0 Windows failure history and bounded repairs

### V1 — physical cache-directory assumption

The first Windows B0 run passed the repository/change-control preflight and exact runtime package-version checks, then stopped before model inventory because the harness assumed that the selected logical model names were also the physical cache directory names under `~/.paddlex/official_models`.

That assumption was too narrow for the ONNX Runtime lane. The actual selected assets were present as:

- `PP-OCRv6_tiny_det_onnx`
- `PP-OCRv6_tiny_rec_onnx`

V2 therefore resolved runtime-format cache assets rather than assuming exact logical-name directories.

### V2 — PaddleOCR default model-name retention

The second Windows B0 run successfully resolved the selected ONNX assets, then PaddleOCR attempted to construct:

`('PP-OCRv6_medium_det', '<PP-OCRv6_tiny_det_onnx path>', 'onnxruntime')`

and failed closed because the local directory's embedded config correctly identified `PP-OCRv6_tiny_det`.

This exposed a separate harness bug: PaddleOCR 3.7.0 defaults to `PP-OCRv6_medium_det` / `PP-OCRv6_medium_rec` when model names are omitted. Supplying only `text_detection_model_dir` and `text_recognition_model_dir` does not rewrite those default logical model names.

The V3 repair therefore:

1. permits only `_onnx` selected-cache variants for this frozen ONNX Runtime lane;
2. validates each local model directory using PaddleX `resolve_model_name` before OCR pipeline construction;
3. passes `text_detection_model_name='PP-OCRv6_tiny_det'` together with its explicit directory;
4. passes `text_recognition_model_name='PP-OCRv6_tiny_rec'` together with its explicit directory;
5. retains `engine='onnxruntime'`, local CPU, offline flags and process-level socket denial;
6. leaves the original V2 inventory script as evidence lineage, while the Windows runner invokes the strict V3 wrapper.

The fix is directly aligned with the Stage A benchmark, which selected P1 by explicitly supplying the tiny model names. It changes no model, dependency, benchmark threshold, product code, workspace/native code, Cargo.lock, Beankeeper, facade or accounting behaviour.

## 8. Forward plan after B0 PASS

B0 is only an evidence/packaging preflight. A PASS must not trigger broad Python-environment bundling automatically.

The next decision sequence is:

### B1 — production packaging adjudication
- inspect the returned SBOM and licence evidence;
- identify the minimum runtime population actually required for selected P1 inference;
- freeze exact model-tree SHA-256 values and model byte sizes;
- decide whether the Windows V1 package is a bounded local sidecar/runtime bundle or whether a narrower native ONNX adapter is materially simpler and safer;
- reject network/model-host dependencies from the document-processing path;
- produce a dependency/licence/SBOM change record before any product dependency is introduced.

### B2 — implementation-neutral OCR contract
- add only Shark-owned factual OCR DTOs/requests at the application/platform boundary;
- keep `shark-books-core` independent of PaddleOCR, Python and ONNX types;
- no OCR output may directly create, classify, post, clear or reconcile accounting records;
- preserve manual receipt/document entry when OCR is unavailable.

### B3 — Windows selected-runtime adapter
- implement the smallest local adapter behind the Shark boundary;
- consume only bounded local document selections/references;
- use only the frozen selected local model assets;
- return factual extraction/provenance/warnings;
- add timeout, malformed-output, unavailable-runtime and integrity failure behaviour;
- prove no document-processing network dependency.

### C — deterministic receipt matching
- combine OCR factual candidates with existing SBC-3 bank-line provenance and SBC-4 matching logic;
- amount/date/reference/merchant evidence can rank suggestions but cannot confirm a match;
- explicit user confirmation remains mandatory;
- ambiguity and missing critical fields fail closed.

### D — platform/regression proof
- rerun all product-core/frozen-foundation guards;
- prove Windows packaging/runtime behaviour;
- keep Apple/iOS build and manual document flows working even if P1 OCR is Windows-only in V1;
- separately adjudicate any Apple ONNX path rather than forcing Python into iOS.

### E/F/G — SBC-7 UI contract, implementation and integrated acceptance
- build the owner-facing UI only against stable Shark operations;
- expose OCR as an optional convenience, never as a prerequisite for bookkeeping;
- prove a representative journey such as receipt -> factual OCR -> suggested bank match -> user confirmation -> document/bookkeeping state visible in UI.

## 9. Hard stop

Do not add a production OCR package, model asset, Tauri command, installer resource, native dependency, or UI wiring until the B0 inventory output is adjudicated and a bounded packaging/interface decision is frozen.

SBC-7 information-architecture/application-operation work may continue, but product UI implementation remains blocked until SBC-6 closes PASS.
