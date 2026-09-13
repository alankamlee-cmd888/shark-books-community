# Shark Books Community — SBC-7B1 Batch A Mutation + Audit Contract

**Date:** 14 September 2026  
**Planning entry main:** `40fcd4d8a65e9b02337a198d8294d37cc0daee67`  
**Stage:** SBC-7B1 remaining typed owner bridge — Batch A  
**Disposition:** CONTRACT FROZEN / IMPLEMENTATION PENDING

## 1. Objective

Add the smallest typed owner-facing mutation/audit bridge needed to finish the frozen receipt-decision and correction journeys without creating new accounting, tax or filesystem authority.

The batch composes existing proven behaviour rather than replacing it:
- SBC-6 factual OCR and deterministic `rank_receipt_bank_suggestions`;
- `confirm_receipt_bank_suggestion` / `reject_receipt_bank_suggestions`;
- SBC-4 `CorrectionPlan` history-preserving contract;
- current encrypted-books transaction lookup/posting through the Shark foundation;
- current Shark application persistence and bank-activity metadata.

## 2. Frozen owner command family

Exactly these capability families must be satisfied by a finite typed native bridge:

1. `owner_receipt_suggest_bank`
2. `owner_receipt_confirm_bank`
3. `owner_receipt_reject_bank`
4. `owner_correction_preview`
5. `owner_correction_confirm`
6. `owner_correction_history`

The correction commands cover both frozen `MONEY_IN.CORRECT` and `MONEY_OUT.CORRECT` capabilities through an explicit `recordKind` enum limited to `moneyIn` / `moneyOut`.

No generic `execute`, raw posting-lines request, raw transaction ID authority, SQL, shell or unrestricted path command is permitted.

## 3. Receipt suggestion/decision contract

### Suggest

`owner_receipt_suggest_bank` must:
- accept books identity plus opaque document/request identifiers only;
- reopen/verify the registered user-controlled document under the already-proven document-integrity boundary;
- use the frozen factual OCR seam when OCR is available;
- load authoritative persisted Bank activity rather than accepting bank rows from the webview;
- convert those rows to the frozen canonical SBC-3 `BankLine` shape;
- call `rank_receipt_bank_suggestions` unchanged;
- return owner-safe candidate IDs/levels/reasons and an opaque native suggestion ID;
- retain the exact core suggestion set in a Rust-only bounded in-memory registry for the confirmation window;
- never persist or expose a raw OS path, model path, database handle, hash secret or shell command.

An OCR-unavailable outcome remains a recoverable manual-document path, not an accounting failure.

### Confirm / reject

`owner_receipt_confirm_bank` and `owner_receipt_reject_bank` must:
- resolve the opaque suggestion ID from the native-only registry;
- use the exact stored `ReceiptBankSuggestionSet`, not a webview-reconstructed set;
- call the existing core confirm/reject function unchanged;
- on confirmation, map the chosen stable bank-line identity back to the authoritative persisted Bank activity ID captured during suggestion generation;
- persist an append-only Shark receipt decision record;
- be idempotent for an exact repeat of the same suggestion/action and fail closed on conflicting replay;
- never clear, reconcile, post, categorise, decide business/private use or decide tax treatment.

A rejected suggestion is non-destructive. A later newly generated suggestion may create a later immutable decision; history is retained.

## 4. Correction contract

### Preview

`owner_correction_preview` must:
- identify the original owner record by the frozen `sbc7b1:<recordKind>:<recordId>` reference; the webview does not supply an authoritative transaction ID;
- require exactly one authoritative original transaction;
- reject an original already superseded by a correction record unless the new correction targets a later replacement record explicitly;
- construct reversal lines by swapping debit/credit direction from the authoritative original entries while preserving exact whole-pence amounts/account codes;
- validate distinct original/reversal/replacement RecordIds using the existing `CorrectionPlan` contract;
- optionally build a replacement through the existing Money In / Money Out deterministic planning rules;
- return owner-safe original/reversal/replacement facts and `requiresConfirmation=true` without mutating persistence.

### Confirm

`owner_correction_confirm` must rerun the preview from authoritative current state and then persist, atomically within the encrypted books connection:
- a new reversal transaction;
- optional new replacement transaction;
- an immutable Shark correction-history relationship binding original, reversal and optional replacement transaction/record identities, actor and reason.

The original transaction is never rewritten or deleted. Reversal/replacement references must be new and collision-safe. Exact idempotent replay may return already-applied; conflicting replay must fail closed.

### History

`owner_correction_history` is read-only and returns bounded owner-safe history for a record chain. It exposes record identities, correction reason, actor/time and replacement relationship, but not raw Beankeeper/SQLite objects.

## 5. Shark application schema v4

Batch A is expected to advance `SHARK_APPLICATION_SCHEMA_VERSION` from 3 to 4 while Beankeeper remains exactly schema 8.

Only the following new Shark-owned metadata is authorised:

### `shark_receipt_bank_decision`
Append-only decision history containing, at minimum:
- company slug;
- opaque suggestion ID;
- document ID;
- decision kind `confirmed|rejected`;
- confirmed Bank activity ID when applicable;
- confirmed stable bank identity fields sufficient for audit/integrity;
- actor/time.

No OCR image bytes or arbitrary paths are stored.

### `shark_owner_correction`
Immutable correction relationship containing, at minimum:
- company slug + correction ID;
- record kind;
- original record and authoritative transaction ID;
- reversal record and transaction ID;
- optional replacement record and transaction ID;
- bounded reason;
- actor/time.

Foreign keys must restrict destructive deletion. Existing backup-before-open/migration ordering remains mandatory.

## 6. Atomicity and validation

Mutation must use the existing encrypted connection and a bounded Shark savepoint/transaction boundary. No partial correction may leave a reversal without its required correction relationship or optional replacement relationship.

Required fail-closed cases include:
- stale/unknown suggestion ID;
- candidate not in the exact stored suggestion set;
- unmatched receipt candidate confirmation;
- missing/changed document integrity;
- missing or conflicting Bank activity identity;
- unknown/multiple original owner transactions;
- original/reversal/replacement ID collision;
- blank/oversized actor or correction reason;
- zero/invalid replacement amount/date;
- conflicting idempotent replay;
- arithmetic overflow;
- repository/application schema inconsistency.

## 7. No product-core semantic rewrite

The following frozen product-core files are expected to remain unchanged:
- `product/shark-books-core/src/receipt_bank_suggestion.rs`;
- `product/shark-books-core/src/matching_reconciliation.rs`;
- SBC-2 posting-plan/domain modules.

If implementation discovers a real core defect that prevents this contract, stop and open explicit change control rather than silently modifying the frozen core.

## 8. Pre-authorised implementation path envelope

The implementation candidate may touch only the following categories unless an explicit contract amendment is committed before evidence:

- this contract document;
- `workspace/shark-foundation/src/lib.rs`;
- `workspace/shark-foundation/src/bank_application/mod.rs`;
- `workspace/shark-foundation/src/bank_application/tests.rs` only for the v3->v4 schema regression update if required;
- one new Shark foundation receipt/correction persistence module and its focused tests;
- `workspace/shark-tauri-spike/src/owner_documents_ocr.rs` only for bounded reusable verified-OCR helper exposure;
- `workspace/shark-tauri-spike/src/owner_app.rs` only for reuse of existing deterministic owner Money In/Out planning in correction preview/confirm;
- one new typed owner receipt/correction bridge module;
- `workspace/shark-tauri-spike/src/lib.rs`;
- `workspace/shark-tauri-spike/build.rs`;
- `workspace/shark-tauri-spike/permissions/shark-shell.toml`;
- one focused static/runtime validator;
- one Windows runner;
- one Codemagic Apple runner;
- `codemagic.yaml` only to register the Batch A Apple workflow.

`workspace/Cargo.lock` is expected to remain byte-identical because no dependency addition is authorised.

The final candidate must freeze an exact file list; this envelope is not permission for unused paths to drift.

## 9. Proof obligations

Before merge, prove at minimum:
- exact candidate diff/allow-list;
- application schema v4 separate from Beankeeper schema 8;
- inherited product-core 113-test suite and later combined receipt-flow regression remain PASS;
- foundation Bank/document/application regressions remain PASS;
- prior owner Home/Money, Bank review/mutation and Documents/OCR bridges remain PASS;
- new receipt suggest/confirm/reject tests cover exact, ambiguous, unmatched, reject, stale token, conflicting replay and no-clear/no-post side effects;
- new correction tests cover preview-only, reversal-only, reversal+replacement, original preserved, atomic rollback, idempotent replay, conflicting replay and history read;
- frontend remains unwired;
- exact Tauri ACL/CSP command set;
- locked Windows compile/runtime proof;
- same immutable SHA Apple physical + Simulator compile/regression proof;
- Cargo.lock and repository clean before/after.

## 10. Exit

Batch A closes only after same-SHA Windows + Apple PASS and protected merge. Batch B implementation must start from that new protected main. Full SBC-7B1 remains incomplete after Batch A.