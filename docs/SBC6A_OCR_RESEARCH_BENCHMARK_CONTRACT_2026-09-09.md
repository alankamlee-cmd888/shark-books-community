# Shark Books Community - SBC-6A OCR research / benchmark / interface contract

Date: 2026-09-09
Status: PREREGISTERED RESEARCH GATE - NO OCR PRODUCT DEPENDENCY OR WIRING AUTHORISED
Entry production main: `d20a356e374e6b7b56d4f025503896506a6027ce`

## 1. Question this gate must answer

Can SharkBooks provide useful local-first receipt OCR on modest consumer hardware without making OCR an accounting/tax authority, without document egress, and without creating a platform/dependency burden disproportionate to V1?

SBC-6A is an architecture and evidence gate. It does not add OCR to the production dependency graph.

## 2. Current external evidence checked before candidate selection

### PaddleOCR

Official PaddleOCR release `v3.7.0` (2026-06-11) introduces PP-OCRv6 in tiny/small/medium tiers. The official project reports PP-OCRv6 tiny as the fastest tier and publishes ONNX Runtime results including Apple M4 and CPU measurements. Official cross-platform material includes PP-OCRv6 ONNX deployment and an official browser SDK path. Repository licence: Apache-2.0.

Primary sources:
- https://github.com/PaddlePaddle/PaddleOCR/releases/tag/v3.7.0
- https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv6/PP-OCRv6.en.md
- https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/inference_deployment/cross_platform/browser.en.md
- https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/inference_deployment/cross_platform/android_deployment.en.md
- https://github.com/PaddlePaddle/PaddleOCR/blob/v3.7.0/LICENSE

Published model-storage figures relevant to this gate are approximately:
- `PP-OCRv6_tiny_det`: 1.9 MB
- `PP-OCRv6_tiny_rec`: 4.4 MB
- `PP-OCRv6_small_det`: 9.6 MB
- `PP-OCRv6_small_rec`: 20.4 MB

The benchmark does not rely on PaddleOCR's generic benchmark accuracy as proof for UK receipts. Shark-specific field accuracy is measured below.

### Tesseract baseline

Tesseract `5.5.3` is retained as the independent mature OCR baseline. It is Apache-2.0 and has well-established Windows/macOS build/install routes. Its current documentation also makes clear that current Windows installers are not an official upstream binary distribution, so it is a useful comparator rather than an automatic SharkBooks production choice.

Primary sources:
- https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.3
- https://github.com/tesseract-ocr/tessdoc/blob/main/Installation.md
- https://github.com/tesseract-ocr/tesseract/blob/5.5.3/LICENSE

## 3. Frozen benchmark candidates

No post-result model shopping is allowed in SBC-6A v1.

- `T0`: Tesseract 5.5.3 English - comparison/control.
- `P1`: PaddleOCR 3.7.0 + `PP-OCRv6_tiny_det` + `PP-OCRv6_tiny_rec`, CPU/local inference.
- `P2`: PaddleOCR 3.7.0 + `PP-OCRv6_small_det` + `PP-OCRv6_small_rec`, CPU/local inference.

PP-OCRv6 medium, PaddleOCR-VL, cloud OCR APIs, VLMs and proprietary OCR services are outside this comparator.

## 4. Synthetic UK receipt benchmark

The committed generator creates a deterministic synthetic benchmark so no merchant/customer personal data or copyrighted receipt scans need to enter the repository.

Minimum population: 48 images = 6 semantic receipt templates x 8 image conditions.

Semantic templates deliberately vary:
- merchant name length/style;
- DD/MM/YYYY dates;
- GBP totals from pence-scale to four figures;
- references/receipt numbers;
- item-line count and total placement;
- VAT-looking text may appear as ordinary printed text but is never interpreted as VAT accounting authority.

Eight conditions:
1. clean scan;
2. small rotation;
3. perspective/skew;
4. blur;
5. shadow/uneven lighting;
6. low contrast;
7. faded thermal-paper simulation;
8. long/crumpled/noisy simulation.

The ground truth contains factual fields only:
- merchant text;
- document date;
- total amount in exact pence;
- currency (`GBP` when printed/inferred only from an explicit pound sign or GBP token by the benchmark parser);
- reference/receipt number when present.

No category, business/private split, deductibility, VAT treatment, CIS/payroll/company conclusion or posting instruction exists in the benchmark ground truth.

## 5. Metrics

Report per engine and per image condition:

- document OCR success/failure;
- merchant normalised exact match;
- date exact match;
- amount exact-pence match;
- currency match;
- reference exact match where present;
- all-critical-fields match (`date + amount`);
- all-factual-fields match;
- mean OCR confidence where supplied by engine;
- latency p50/p95/max;
- process peak working set where measurable;
- installed benchmark-environment size where measurable;
- engine/model identity and versions;
- any network attempt after models/runtime are already present.

Critical errors are amount/date errors. A plausible-looking wrong amount is more serious than failure to extract a merchant name.

## 6. Preregistered adjudication thresholds

A production candidate may proceed to SBC-6B only if all are true on this v1 benchmark:

1. critical (`date + amount`) exact-match >= 95% overall;
2. amount exact-pence match >= 97% overall;
3. no image-condition class has critical-field exact-match below 80%;
4. false confident amount substitutions <= 1/48 and must not be silently accepted by the downstream contract;
5. CPU p95 on the Windows proof machine <= 8 seconds/image, with <= 4 seconds/image preferred;
6. peak working set <= 1.5 GB preferred; any result > 2.0 GB requires owner decision before product integration;
7. no document bytes are sent to Shark or any third-party OCR service during the offline rerun;
8. licence is commercially/open-source compatible with SharkBooks distribution and obligations are recordable;
9. the candidate has a credible Windows path and an Apple/iOS path that does not require rewriting accounting logic;
10. the engine returns factual text/regions/confidence only; Shark remains the owner of parsing, matching and accounting action.

Selection rule:
- prefer P1 if it passes the hard gates and P2's critical-field gain is < 3 percentage points;
- select P2 only if it materially improves critical-field accuracy without breaching the resource/packaging gates;
- select T0 only if it is at least as safe/accurate on the same benchmark and has the simpler credible Windows + Apple packaging story;
- if none pass, `DEFER_OCR_V1` is a valid result. SBC-7 may still proceed without OCR.

## 7. Extraction machine contract to freeze if Stage A passes

The OCR adapter may return only factual evidence:

```text
OcrExtraction
  schema_version
  engine_id
  engine_version
  model_ids[]
  document_sha256
  raw_text
  regions[]
    text
    confidence?
    bbox?
  candidates
    merchant_text?
    document_date?
    total_pence?
    currency?
    reference?
  warnings[]
```

Every candidate must be traceable to raw OCR text/regions. `total_pence` uses an exact integer, never float.

The adapter must not return:
- expense/category account;
- business/private status;
- tax deductibility;
- VAT treatment/rate;
- posting plan;
- match confirmation;
- ledger/database path.

## 8. Deterministic receipt-to-record matching design

Receipt matching reuses SBC-3/SBC-4 factual signals only:
- exact amount is a hard prerequisite for an exact candidate;
- bounded date distance;
- explicit reference similarity;
- merchant/payee text similarity;
- document identity/hash;
- existing duplicate/matching states.

OCR confidence may down-rank or hold a suggestion but never authorise persistence. Multiple plausible matches cannot be labelled exact. The user must confirm before the normal Shark action/facade path is invoked.

## 9. Runtime architecture hypotheses

Stage A compares interfaces rather than committing immediately:

### Preferred hypothesis - bounded native OCR adapter using portable model/runtime assets

A Shark-owned adapter starts a local OCR runtime, sends only user-selected local bytes, receives the factual extraction contract and terminates/returns control. The accounting core remains dependency-free and does not import OCR/Python/ONNX/Paddle types.

Benefits:
- contains the substantial OCR dependency away from the ledger/domain core;
- permits Windows and Apple implementations behind one Shark contract;
- supports complete removal/disablement of OCR;
- makes no-egress easier to test.

### Rejected for SBC-6 initial implementation

- cloud OCR default;
- OCR embedded in accounting/persistence crates;
- model writing directly to SQLite/Beankeeper;
- browser-parity claim now;
- a VLM/LLM interpreting receipt tax/accounting meaning.

Official PaddleOCR.js/ONNX evidence is retained as a useful SBC-17 browser candidate, not a reason to pull browser parity into SBC-6.

## 10. SBC-7 preparation allowed in parallel

While the benchmark runs, SBC-7 may freeze information architecture, owner journeys, view contracts and component-reuse decisions because these do not depend on the OCR winner. Product UI implementation remains hard-blocked until SBC-6 is closed PASS or OCR is explicitly deferred.

## 11. Stage A exit outcomes

Exactly one:
- `PASS_SELECT_PP_OCRV6_TINY`
- `PASS_SELECT_PP_OCRV6_SMALL`
- `PASS_SELECT_TESSERACT`
- `DEFER_OCR_V1_NO_CANDIDATE_EARNS_GATE`
- `BLOCKED_BENCHMARK_ENVIRONMENT_NOT_A_MODEL_RESULT`

No Stage A outcome by itself authorises tax logic, AI action, speech, Open Banking, direct HMRC filing or browser parity.
