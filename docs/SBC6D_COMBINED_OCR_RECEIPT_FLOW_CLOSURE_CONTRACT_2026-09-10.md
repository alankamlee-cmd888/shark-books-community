# SBC-6D — Combined OCR / receipt-flow closure contract

**Date:** 10 September 2026  
**Protected entry main:** `ef799f5a5c3e4c1788ae7858b4865b6b3c5be496`  
**Scope:** Shark Books Community — final SBC-6 closure gate  
**Status:** candidate evidence required before PASS/merge

## Purpose

SBC-6D is a composition and regression closure gate. It does not add a new bookkeeping feature or widen authority.

The bounded flow under proof is:

`user-controlled receipt/document -> integrity-bound factual OCR -> deterministic receipt-to-bank suggestion -> explicit owner confirm/reject decision`

## Evidence continuity

The native/Tauri OCR implementation that passed SBC-6B B3C on Windows and Codemagic Apple must remain byte-identical at this gate. SBC-6C did not modify the native workspace. SBC-6D therefore proves continuity by comparing the complete `workspace/shark-tauri-spike` tree to the protected B3C merge `5748b0236d3dc0f89975b9d7cb021af05f742f43`, rather than rebuilding or changing the OCR runtime.

No Apple rerun is required unless native/Tauri or dependency state changes. Any such change is outside this bounded candidate and must stop the gate for explicit change control.

## New closure regression

A product-level end-to-end regression composes only already-proven public contracts:

- SBC-5 user-controlled `DocumentReference` integrity identity;
- SBC-6B B2 `OcrRequest` / `OcrExtraction` factual candidates;
- SBC-3 canonical `BankLine` facts;
- SBC-6C deterministic receipt-to-bank suggestion and explicit confirm/reject decision.

It proves:

- a factual date/amount/currency/merchant/reference extraction can yield a unique inspectable suggestion;
- the unique suggestion remains unaccepted until explicit owner confirmation;
- equal top candidates remain ambiguous with no implicit recommendation;
- a one-penny mismatch is unmatched and cannot be confirmed;
- missing critical OCR total remains missing and cannot produce a match;
- wrong document hash cannot create a factual extraction;
- rejection is explicit and non-destructive.

## Frozen authority boundaries

This gate must not introduce or exercise authority to:

- post accounting entries;
- select an expense or income category;
- decide business/private or mixed use;
- clear or reconcile a bank line;
- decide tax deductibility or VAT/CIS/payroll/company treatment;
- file or submit to HMRC;
- use LLM/AI judgement;
- use Open Banking or network/cloud OCR;
- expose arbitrary filesystem paths or a generic shell/process surface.

The receipt-to-bank confirmation result remains only the bounded association decision already defined in SBC-6C.

## Exact authorised diff

The SBC-6D candidate is limited to exactly four paths:

1. `ci/run_sbc6d_windows.ps1`
2. `docs/SBC6D_COMBINED_OCR_RECEIPT_FLOW_CLOSURE_CONTRACT_2026-09-10.md`
3. `product/shark-books-core/tests/sbc6d_receipt_flow.rs`
4. `scripts/check_sbc6d_combined_closure.py`

No production source, Cargo manifest, Cargo.lock, native/Tauri source/config, OCR model/runtime, frontend or dependency path may change.

## Required executable proof

The Windows gate must prove:

1. clean repository at entry and exit;
2. protected SBC-6C merge is the exact candidate ancestor;
3. candidate diff equals the four-path allowlist;
4. production source modules for bank import, matching/reconciliation, documents, OCR and receipt-bank suggestion are unchanged from protected SBC-6C main;
5. complete Tauri/native OCR tree is identical to the already Windows+Apple-proven B3C merge;
6. product and native lockfile SHA-256 hashes remain exact;
7. frozen foundation and permanent SBC-1G change-control guards PASS;
8. all inherited product-core tests plus the new combined receipt-flow regression PASS under `--locked`;
9. frozen foundation tests PASS;
10. the current Tauri/native crate still compiles under the frozen lock (`--no-run`) after exact Beankeeper materialisation;
11. no dependency/native drift occurs during the proof.

## Exit

Only after returned evidence is formally adjudicated PASS and the candidate merges through protected `main` may SBC-6 be declared COMPLETE and the programme advance to **SBC-7A — owner-facing UI contract**.
