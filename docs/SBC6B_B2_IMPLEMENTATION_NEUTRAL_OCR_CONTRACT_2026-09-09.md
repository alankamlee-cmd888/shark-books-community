# Shark Books Community — SBC-6B B2 implementation-neutral OCR contract

Date: 2026-09-09  
Status: CANDIDATE / WINDOWS PRODUCT-CORE PROOF REQUIRED  
Entry protected main: `c8417665516476e77468c5815aa33b8c69787aa9`  
B1 decision: **PASS — direct PP-OCRv6 tiny ONNX parity earned; broad PaddleOCR/PaddleX research environment rejected as production packaging**

## 1. Purpose

B2 introduces only the Shark-owned application contract that OCR implementations must satisfy. It does **not** add an OCR engine, Python runtime, ONNX Runtime dependency, model asset, subprocess, filesystem operation, Tauri command, persistence path or UI wiring.

The contract is deliberately small so Windows can use the B1-proven local direct-ONNX adapter while Apple or another platform may use a different local implementation later without changing bookkeeping or accounting logic.

## 2. Frozen factual boundary

OCR may receive only an opaque application request bound to the integrity identity of an existing `DocumentReference`:

- request id;
- document id;
- expected SHA-256;
- expected byte length;
- fixed task `ReceiptFacts`.

The product-core OCR request intentionally contains **no storage-root id, relative path, arbitrary filesystem path, database handle, books key, attached accounting-record id, network endpoint or model path**.

The platform adapter remains responsible for obtaining verified bytes through the already-proven document/storage boundary.

## 3. Output contract

A completed extraction may contain factual OCR evidence only:

- contract schema version;
- request/document integrity identity;
- engine/runtime/model provenance;
- raw recognised text;
- bounded text regions and confidence values;
- optional merchant text;
- optional document date;
- optional total in integer pence;
- optional three-letter currency code;
- optional receipt/reference text;
- bounded warnings.

Confidence uses integer basis points `0..=10000`; floating-point confidence does not enter the product contract.

Missing facts remain `None`. The contract does not guess missing values.

## 4. Integrity binding

`OcrExtraction::from_adapter_output` requires the adapter to return the SHA-256 of the bytes it actually processed. That hash must equal the hash in the originating `OcrRequest`. A malformed or different hash fails closed.

This prevents a stale, switched or incorrectly selected document from being silently attached to the OCR result.

## 5. Explicit non-completed outcomes

OCR remains optional. The common contract has explicit outcomes for:

- completed extraction;
- unavailable because disabled/not installed/unsupported/models unavailable;
- failed because of integrity mismatch/timeout/malformed output/engine failure/resource limit.

An unavailable or failed OCR path does not create substitute facts. The existing manual document/bookkeeping path remains valid.

## 6. Prohibited semantics

B2 contains no mechanism for OCR to:

- choose bookkeeping categories;
- infer business/private use;
- infer tax deductibility;
- infer VAT/CIS/payroll/company treatment;
- create or alter postings;
- persist accounting records;
- confirm bank matches;
- clear or reconcile entries;
- submit to HMRC;
- call an LLM or cloud OCR service.

Those behaviours are outside the OCR factual boundary.

## 7. B1 packaging decision carried forward

The B1 48-fixture direct-ONNX proof reproduced the Stage A factual quality result without importing PaddleOCR or PaddleX:

- critical date+amount: `47/48 = 97.9167%`;
- exact amount: `47/48 = 97.9167%`;
- false-confident wrong amount: `0`;
- every degradation condition met the frozen `>=80%` critical floor;
- p95: about `794 ms`;
- peak RSS: `468,717,568` bytes;
- ONNX Runtime telemetry-disable API called;
- Python TCP connection surfaces blocked during model load and all 48 predictions.

The exact selected model assets remain:

- detector `inference.onnx` SHA-256 `193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8`;
- detector `inference.yml` SHA-256 `3ac018be6f97499a08faa3bbdeb33640968d9307f6736d152902747a9f259593`;
- recogniser `inference.onnx` SHA-256 `9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6`;
- recogniser `inference.yml` SHA-256 `66170210bad538e83fff3c4a3867e547d6bf20b50d64b20347c4b913f3034ea1`.

B3 may therefore implement the smallest Windows local adapter around the direct-ONNX path. It must **not** reintroduce the 74-distribution research environment merely because it exists.

## 8. B2 allowed repository change

Exactly five paths are authorised from B1 protected main:

1. `product/shark-books-core/src/ocr.rs`
2. `product/shark-books-core/src/lib_sbc3.rs`
3. `docs/SBC6B_B2_IMPLEMENTATION_NEUTRAL_OCR_CONTRACT_2026-09-09.md`
4. `scripts/check_sbc6b_b2_ocr_contract.py`
5. `ci/run_sbc6b_b2_windows.ps1`

No `Cargo.toml`, Cargo.lock, native workspace, Beankeeper, facade, Tauri capability or installer change is authorised.

## 9. B2 executable gate

Windows proof must establish:

- exact five-path diff;
- product core remains dependency-free;
- both reviewed Cargo.lock hashes remain exact;
- `ocr.rs` contains no platform/runtime/network/persistence/accounting-action implementation markers;
- frozen-foundation guard PASS;
- permanent SBC-1G change-control guard PASS from B1 main;
- Rust `1.98.1` selected;
- complete product-core tests PASS under `--locked`;
- both lockfiles unchanged;
- repository clean before and after.

The existing 74 product-core tests plus the 18 B2 OCR-contract tests give an expected total of **92 tests**.

## 10. Hard stop after B2

A B2 PASS authorises B3 Windows adapter work only. It does not by itself authorise SBC-7 UI implementation, Apple OCR parity, cloud OCR, model changes or autonomous receipt posting.
