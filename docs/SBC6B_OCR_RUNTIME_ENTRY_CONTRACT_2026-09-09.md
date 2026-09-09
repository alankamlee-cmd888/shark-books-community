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
- resolve only the selected P1 logical models and their runtime-format cache variants (`_onnx` preferred for the frozen ONNX Runtime lane, plus exact-name and `_safetensors` fallback); 
- derive the PaddleX cache root from the installed runtime and use only bounded local cache discovery, with no download or model shopping;
- record the physical cache directory actually used;
- hash every selected model file;
- enumerate installed Python distributions and licence metadata/files;
- instantiate P1 from explicit local model directories;
- disable Paddle model-source checks;
- deny socket connection calls during import/model load/prediction;
- run one deterministic clean synthetic receipt smoke;
- leave the repository clean.

## 7. Initial B0 harness failure and bounded repair

The first Windows B0 run passed the repository/change-control preflight and exact runtime package-version checks, then stopped before model inventory because the harness assumed that the selected logical model names were also the physical cache directory names under `~/.paddlex/official_models`.

That assumption was too narrow for an ONNX Runtime lane. PaddleX model resolution supports runtime-format suffixes including `_onnx`; the selected logical model remains `PP-OCRv6_tiny_det` / `PP-OCRv6_tiny_rec`, but the physical cached asset may be format-qualified.

The repair is harness-only. It does not change the selected model, PaddleOCR version, ONNX Runtime version, quality thresholds, product code, workspace/native code, Cargo.lock, Beankeeper, facade or accounting behaviour. The V2 resolver checks only the authorised logical names and their bounded runtime-format variants, records the resolved physical path, and still requires explicit-local-model loading with socket connections denied.

## 8. Hard stop

Do not add a production OCR package, model asset, Tauri command, installer resource, native dependency, or UI wiring until the B0 inventory output is adjudicated and a bounded packaging/interface decision is frozen.

SBC-7 information-architecture/application-operation work may continue, but product UI implementation remains blocked until SBC-6 closes PASS.
