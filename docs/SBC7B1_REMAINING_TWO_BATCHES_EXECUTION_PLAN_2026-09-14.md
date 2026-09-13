# Shark Books Community — SBC-7B1 Remaining Two-Batch Execution Plan

**Date:** 14 September 2026  
**Entry protected main:** `40fcd4d8a65e9b02337a198d8294d37cc0daee67`  
**Authority:** frozen SBC-7A owner UI contract and operation matrix; `132_SBC7B1_REMAINING_TYPED_OWNER_BRIDGE_HANDOVER_2026-09-10.md`; Slice 3A final PASS record 149.

## 1. Purpose

Complete the remaining SBC-7B1 typed owner application/native bridge in two bounded implementation/proof cycles rather than four separate feature slices, while retaining exact candidate identity, fail-closed mutation boundaries and same-SHA platform proof.

The two batches are planned together here, but implementation is strictly sequential. Batch B must start from protected main only after Batch A has passed, merged and been formally closed.

## 2. Batch split

### Batch A — Mutation + Audit

Scope:
- deterministic receipt-to-bank suggestion orchestration;
- explicit receipt suggestion confirm/reject;
- correction/reversal/replacement preview + confirm;
- correction history/read model.

Rationale: these operations share explicit-owner-confirmation, immutable/audited decision history and authoritative re-read-before-mutation requirements.

Frozen contract: `docs/SBC7B1_MUTATION_AUDIT_BATCH_CONTRACT_2026-09-14.md`.

### Batch B — Supporting Data + Read Models

Scope:
- Contacts list/save for minimal customer/supplier identity;
- Settings books-information read model;
- bounded native document-storage-root selection/registration for the device session;
- factual owner report summary/read model.

Rationale: these are lower-risk supporting owner data/read surfaces and do not belong in the high-risk mutation batch.

Frozen contract: `docs/SBC7B1_DATA_READMODELS_BATCH_CONTRACT_2026-09-14.md`.

## 3. Sequencing

`main 40fcd4d8... -> Batch A contract implementation -> Windows PASS -> same-SHA Apple PASS -> protected merge -> Batch B implementation -> Windows PASS -> same-SHA Apple PASS -> protected merge -> full SBC-7B1 closure -> SBC-7B2`

Batch B specification may be refined only by explicit amendment before its implementation branch is created. Batch B executable work must not be developed against the pre-Batch-A base.

## 4. Shared hard boundaries

Both batches retain all frozen exclusions:
- no VAT, CIS, payroll, companies or partnerships;
- no tax liability/reserve/advice;
- no direct HMRC filing;
- no Open Banking/live bank feeds;
- no cloud OCR requirement;
- no AI/LLM accounting judgement;
- no raw SQL/Beankeeper/webview database authority;
- no generic shell;
- no arbitrary webview filesystem paths;
- no silent match confirmation, clearing or reconciliation;
- no automatic business/private or deductibility decisions;
- no owner-facing Vue binding in SBC-7B1.

## 5. Shared proof discipline

For each batch:
1. branch from then-current protected `main`;
2. freeze exact operation contract before implementation;
3. declare exact final changed-path allow-list before evidence;
4. keep generated Cargo output outside the repository;
5. run inherited product/foundation/owner regressions plus new focused tests;
6. freeze one immutable candidate SHA after local/static PASS;
7. run Windows proof first;
8. run Apple physical `aarch64-apple-ios` + `aarch64-apple-ios-sim` proof on the exact same SHA whenever native/config/foundation surface changes (expected for both batches);
9. do not merge partial evidence;
10. merge only with expected-head guard after formal evidence adjudication.

No dependency addition is planned in either batch. The existing reviewed `tauri-plugin-dialog = 2.7.3` is sufficient for the planned bounded settings folder selection in Batch B. Any new dependency would invalidate this planning assumption and require explicit change control.

## 6. Schema direction

Current Shark application schema at entry is v3; Beankeeper remains schema 8.

Planned direction:
- Batch A may advance Shark application schema to v4 solely for Shark-owned receipt-decision and correction-history metadata;
- Batch B may advance Shark application schema to v5 solely for minimal Contact persistence;
- no Beankeeper schema change is authorised;
- existing backup-before-open/migration ordering remains mandatory.

Schema numbers are implementation contracts, not permission to add unrelated tables.

## 7. Exit

Full SBC-7B1 remains open until both Batch A and Batch B are proven and merged. Only then may SBC-7B2 bind the frozen Vue owner UI to the complete typed command surface. SBC-7C remains blocked until SBC-7B2 is complete.