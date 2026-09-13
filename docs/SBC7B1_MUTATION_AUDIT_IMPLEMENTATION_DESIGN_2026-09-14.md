# Shark Books Community — SBC-7B1 Batch A Mutation + Audit — Implementation Design

**Date:** 14 September 2026  
**Protected entry `main`:** `8f55eaf67b311a7c2d69ebea86ae702e4934a56c`  
**Branch:** `sbc7b1-mutation-audit-owner-bridge-20260914`  
**Parent contract:** `docs/SBC7B1_MUTATION_AUDIT_BATCH_CONTRACT_2026-09-14.md`  
**Status:** **IMPLEMENTATION DESIGN FROZEN / CODE CANDIDATE NOT YET FROZEN**

## 1. Existing authority reused unchanged

Batch A must compose, not rewrite:

- `product/shark-books-core/src/receipt_bank_suggestion.rs`
  - `rank_receipt_bank_suggestions`
  - `confirm_receipt_bank_suggestion`
  - `reject_receipt_bank_suggestions`
- `product/shark-books-core/src/matching_reconciliation.rs`
  - `CorrectionPlan`
- `workspace/shark-tauri-spike/src/owner_app.rs`
  - authoritative deterministic Money In / Money Out posting-plan construction
  - existing owner record reference convention `sbc7b1:<recordKind>:<recordId>`
- `workspace/shark-tauri-spike/src/owner_documents_ocr.rs`
  - registered-document lookup, user-controlled root containment and integrity verification
- `workspace/shark-tauri-spike/src/ocr_native.rs`
  - frozen factual OCR native host
- `workspace/shark-foundation`
  - encrypted books lifecycle, transaction lookup/posting and Shark application persistence.

No product-core semantic source change is planned.

## 2. New owner bridge module

Add one module:

`workspace/shark-tauri-spike/src/owner_mutation_audit.rs`

It owns exactly these Tauri commands:

1. `owner_receipt_suggest_bank`
2. `owner_receipt_confirm_bank`
3. `owner_receipt_reject_bank`
4. `owner_correction_preview`
5. `owner_correction_confirm`
6. `owner_correction_history`

All request DTOs use `deny_unknown_fields`. They accept only bounded semantic/opaque values and never accept raw database paths, OS file paths, account codes, debit/credit lines, shell commands, executable/model paths, tax treatments or authoritative transaction IDs.

## 3. Receipt suggestion composition

### 3.1 Verified factual extraction helper

`owner_documents_ocr.rs` will expose a `pub(crate)` helper used by both the existing `owner_ocr_extract_receipt` command and Batch A. The helper:

1. opens the authoritative books;
2. resolves the registered document;
3. rechecks native root/path containment, byte length and SHA-256;
4. registers the resulting `NativeApprovedOcrDocument` with `NativeOcrRegistry`;
5. calls the unchanged `ocr_native::ocr_extract_receipt` seam;
6. returns the exact bounded `ShellOcrOutcome`.

No path is serialized to the webview.

### 3.2 Shell factual output -> core factual contract

The Batch A module converts only a `ShellOcrOutcome::Completed` result into the already-frozen `core::ocr::OcrExtraction` by using the public B2 constructors and the factual values already validated by `ocr_native`.

Unavailable/failure states are returned as typed owner-safe non-suggestion outcomes. They do not create an accounting decision.

### 3.3 Authoritative Bank activity

Suggestion generation opens current books and calls the existing bounded `list_bank_activity` surface. Only unmatched/unconsumed outflow rows that can be faithfully reconstructed as canonical SBC-3 `BankLine` values are candidates.

The webview never supplies canonical BankLine fields. A native `ReceiptSuggestionRegistry` retains:
- a random/opaque bounded suggestion ID;
- document ID;
- exact `ReceiptBankSuggestionSet`;
- mapping from stable `BankLineIdentity` to authoritative Shark bank activity ID.

Registry state is Rust-only, process-local and bounded. Oldest entries are evicted over a small fixed maximum; stale/unknown IDs fail closed.

## 4. Receipt decision persistence

Add a Shark foundation module, expected path:

`workspace/shark-foundation/src/mutation_audit_application.rs`

It defines Shark-owned DTOs and Books methods for append-only receipt decisions and correction history. No raw rusqlite type crosses the module boundary.

Application schema advances **3 -> 4**. Beankeeper remains exactly schema 8.

New table `shark_receipt_bank_decision` stores:
- company slug;
- suggestion ID;
- document ID;
- decision kind `confirmed|rejected`;
- optional confirmed Bank activity ID;
- stable Bank identity snapshot for a confirmed decision;
- actor/time.

Exact repeat is idempotent. Same suggestion ID with a conflicting action/Bank identity fails closed.

A receipt decision does **not** mutate `shark_bank_match`, entry status, reconciliation status or ledger postings.

## 5. Correction design

### Preview

`owner_correction_preview` accepts:
- books reference;
- `recordKind = moneyIn|moneyOut`;
- original record ID;
- new reversal record ID;
- optional replacement request of the same record kind;
- actor is taken from the books reference;
- bounded reason.

It resolves the original transaction through the exact owner reference `sbc7b1:<recordKind>:<recordId>`. Exactly one current authoritative transaction is required.

It creates a `core::matching_reconciliation::CorrectionPlan` to enforce distinct IDs. The reversal plan is derived only from authoritative original transaction entries by swapping debit/credit direction while retaining account code and exact positive pence amount.

An optional replacement is constructed through the existing Money In / Money Out deterministic plan builder rather than through webview-supplied ledger lines.

Preview returns factual original/reversal/replacement summary and `requiresConfirmation=true`; it does not mutate books.

### Confirm

`owner_correction_confirm` receives the same semantic request plus an opaque preview fingerprint. It reruns authoritative preview and requires the fingerprint to match current state.

Within one Shark foundation savepoint it persists:
1. the reversal transaction;
2. optional replacement transaction;
3. `shark_owner_correction` relationship/history.

The original transaction is never edited or deleted.

Exact replay returns the prior correction result; conflicting replay fails closed. A record already superseded may not be corrected again through its obsolete identity; a later correction must explicitly target the replacement record.

## 6. Correction history persistence

`shark_owner_correction` contains:
- correction ID;
- company slug;
- record kind;
- original record + authoritative transaction ID;
- reversal record + transaction ID;
- optional replacement record + transaction ID;
- reason;
- actor/time.

`owner_correction_history` returns a bounded chain ordered deterministically, with owner-safe IDs/reason/actor/time only.

## 7. Atomic foundation API

The new foundation module owns one atomic correction persistence method. It receives already-validated Shark posting requests, executes both/new postings and the history insert under the existing encrypted connection savepoint, and rolls the entire operation back on any failure.

The Tauri bridge must not attempt multi-step pseudo-atomicity itself.

## 8. Tauri state and ACL

`lib.rs` will manage a new native-only `ReceiptSuggestionRegistry` and register the six exact commands.

`build.rs` and `permissions/shark-shell.toml` add exactly those six command names to the existing finite allowlist.

The frontend remains deliberately unwired during SBC-7B1.

## 9. Dependency / lock position

No new crate is required or authorised. Existing `tauri`, `serde`, `serde_json`, `shark-books-core`, `shark-foundation` and the existing OCR/dialog graph are sufficient.

`workspace/Cargo.lock` must remain byte-identical to the Slice 3A reviewed lock:

`10fe1431f26cef2d427e76658690544027e955845faabfbbdaebf2a2ad17e115`

Any lock drift is a stop condition.

## 10. Candidate proof design

A focused static/runtime validator and Windows/Codemagic runners will prove:
- exact final changed-path allow-list;
- schema v4 / Beankeeper schema 8 separation and backup-before-migration ordering;
- no product-core semantic drift;
- lock hash exact;
- all inherited product/foundation/owner regressions;
- receipt suggestion and decision edge cases;
- correction preview/confirm/history, atomic rollback and idempotency;
- frontend remains unwired;
- exact Tauri command/permission inventory;
- repository clean before/after;
- Windows locked build;
- same immutable SHA physical iOS + Simulator compile/regression.

The implementation candidate is **not frozen** until the code diff, exact file allow-list and local/static validation have been reconciled.