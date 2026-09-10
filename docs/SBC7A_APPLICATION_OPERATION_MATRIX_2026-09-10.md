# Shark Books Community — SBC-7A Application Operation Matrix

**Date:** 10 September 2026  
**Protected entry main:** `3fcbbc69a8e30a6d6281ce410f1e5e80c8f43ace`  
**Purpose:** define what the owner UI may ask the Shark application boundary to do, and identify which capabilities still need a bounded 7B bridge.

## 1. Contract rule

The identifiers below are **application capability IDs**, not permission for Vue/JavaScript to call Rust domain functions directly. SBC-7B may map each authorised capability to an exact typed Tauri command/API, but no generic `execute`, raw SQL, arbitrary path, arbitrary command or unrestricted filesystem interface is permitted.

Every UI call must terminate at a Shark-owned typed operation. Domain/accounting/persistence rules remain in Rust/application/facade code.

## 2. Current native command inventory — frozen entry fact

At SBC-7A entry the Tauri shell exposes exactly these bounded commands:

- `foundation_health`
- `production_encryption_required`
- `books_create`
- `books_open`
- `books_verify`
- `books_trial_balance`
- `ocr_extract_receipt`

The legacy engineering frontend invokes the first six only. `ocr_extract_receipt` is intentionally present in Rust but not yet wired from frontend JavaScript.

This inventory is not sufficient for the owner journeys; 7B must add an explicit bridge rather than bypassing the application boundary.

## 3. Status vocabulary

- **EXISTING_NATIVE** — already exposed as a bounded Tauri operation at 7A entry.
- **CORE_AVAILABLE_BRIDGE_REQUIRED** — deterministic/product/foundation capability exists, but no owner-facing Tauri operation exposes it yet.
- **COMPOSITION_REQUIRED** — proven lower-level contracts exist, but 7B needs a typed application service that composes them safely.
- **APPLICATION_GAP_7B** — owner journey needs a bounded application operation/read model not currently proven as a complete callable service. 7B may implement the smallest such operation needed by the frozen journey.
- **DEFERRED** — not authorised in SBC-7.

## 4. Allowed owner-facing application capabilities

| Capability ID | Screen/journey | Owner intent | Entry status | Existing basis | 7B boundary requirement |
|---|---|---|---|---|---|
| `BOOKS.CREATE` | Start/open | create encrypted books | EXISTING_NATIVE | `books_create` | retain bounded request; no key/path from webview |
| `BOOKS.OPEN` | Start/open | open existing books | EXISTING_NATIVE | `books_open` | owner-safe errors; no raw DB path |
| `BOOKS.VERIFY` | Start/open/Settings | verify books | EXISTING_NATIVE | `books_verify` | read-only |
| `HOME.STATUS` | Home | see attention/recent status | APPLICATION_GAP_7B | proven states across core | create typed read model only; no tax/compliance inference |
| `MONEY_IN.PREVIEW` | Money in | preview income/invoice effect | COMPOSITION_REQUIRED | SBC-2 domain posting plans | Rust creates deterministic preview; UI never constructs debit/credit lines |
| `MONEY_IN.SAVE` | Money in | save confirmed income/invoice | COMPOSITION_REQUIRED | SBC-2 plan + Shark facade persistence | validate before persistence; explicit owner action |
| `MONEY_IN.CORRECT` | Money in/detail | correct prior record | COMPOSITION_REQUIRED | audit/correction foundations | preserve history; no destructive rewrite |
| `MONEY_OUT.PREVIEW` | Money out | preview expense effect | COMPOSITION_REQUIRED | SBC-2 domain posting plans | business/private/mixed remains explicit owner input |
| `MONEY_OUT.SAVE` | Money out | save confirmed expense | COMPOSITION_REQUIRED | SBC-2 plan + Shark facade persistence | no tax-deductibility inference |
| `MONEY_OUT.CORRECT` | Money out/detail | correct prior record | COMPOSITION_REQUIRED | audit/correction foundations | preserve history |
| `BANK.IMPORT_PREVIEW_CSV` | Bank import | preview CSV | CORE_AVAILABLE_BRIDGE_REQUIRED | `bank_import::preview_csv` | typed text/file content from native-approved selection; no path from webview |
| `BANK.IMPORT_PREVIEW_OFX_QFX` | Bank import | preview OFX/QFX | CORE_AVAILABLE_BRIDGE_REQUIRED | `bank_import::preview_ofx_or_qfx` | same bounded selection rule |
| `BANK.IMPORT_CONFIRM` | Bank import | accept reviewed import | APPLICATION_GAP_7B | SBC-3 canonical BankLine/provenance + facade capabilities | typed commit service; duplicates never silently discarded |
| `BANK.ACTIVITY_LIST` | Bank | view imported rows/status | APPLICATION_GAP_7B | SBC-3/4 models | bounded read model; no raw database objects |
| `BANK.MATCH_REVIEW` | Bank match | see deterministic candidates/reasons | CORE_AVAILABLE_BRIDGE_REQUIRED | SBC-4 matching contracts | translate levels without changing semantics |
| `BANK.MATCH_CONFIRM` | Bank match | confirm chosen match | CORE_AVAILABLE_BRIDGE_REQUIRED | `matching::confirm_match` | explicit owner confirmation only |
| `BANK.RECONCILE_PREVIEW` | Reconcile | calculate statement difference | CORE_AVAILABLE_BRIDGE_REQUIRED | `matching::preview_reconciliation` | read/preview only |
| `BANK.RECONCILE_FINALISE` | Reconcile | finalise exact reconciliation | CORE_AVAILABLE_BRIDGE_REQUIRED | `matching::finalize_reconciliation` | must preserve exact-zero/all-cleared rule and explicit owner action |
| `DOCUMENT.SELECT_REGISTER` | Receipts | choose/register user-controlled file | APPLICATION_GAP_7B | SBC-5 document/native-selection contract | native picker/approval owns path; webview gets opaque ID and bounded metadata |
| `DOCUMENT.VERIFY` | Receipts/detail | verify integrity | CORE_AVAILABLE_BRIDGE_REQUIRED | SBC-5 integrity contract | hash/length result only; fail closed |
| `DOCUMENT.ATTACH` | Receipts/detail | attach document to record | COMPOSITION_REQUIRED | SBC-5 reference + Shark facade attachment capability | typed record/document IDs only |
| `OCR.EXTRACT` | Receipt review | run local factual OCR | EXISTING_NATIVE | `ocr_extract_receipt` | webview sends opaque request/document IDs only |
| `RECEIPT.SUGGEST_BANK` | Receipt review | see bank-line suggestion | CORE_AVAILABLE_BRIDGE_REQUIRED | `rank_receipt_bank_suggestions` | factual OCR + canonical bank lines only; reasons visible |
| `RECEIPT.CONFIRM_BANK` | Receipt review | confirm selected suggestion | CORE_AVAILABLE_BRIDGE_REQUIRED | `confirm_receipt_bank_suggestion` | explicit owner action; does not itself post/reconcile |
| `RECEIPT.REJECT_BANK` | Receipt review | reject suggestions | CORE_AVAILABLE_BRIDGE_REQUIRED | `reject_receipt_bank_suggestions` | non-destructive |
| `CONTACTS.LIST` | Contacts/editors | select customer/supplier | APPLICATION_GAP_7B | SBC-2 customer/supplier domain DTOs | smallest typed owner-facing read operation |
| `CONTACTS.SAVE` | Contacts | add/edit customer/supplier | APPLICATION_GAP_7B | SBC-2 domain DTOs | validated bounded persistence through application/facade only |
| `REPORT.TRIAL_BALANCE` | Reports/support | view factual balance report | EXISTING_NATIVE | `books_trial_balance` | owner presentation may simplify wording; source remains factual |
| `SETTINGS.BOOKS_INFO` | Settings | view business/books metadata | COMPOSITION_REQUIRED | existing books metadata/verify | read-only owner-safe DTO |
| `SETTINGS.STORAGE_ROOT` | Settings/Receipts | choose/manage user-controlled document root | APPLICATION_GAP_7B | SBC-5 storage-root contract | native-owned selection; no credential or arbitrary path surface |

## 5. Explicitly deferred capabilities

The following are **not** allowed application operations in SBC-7:

- tax liability/reserve or tax advice;
- VAT, CIS, payroll, company or partnership workflows;
- direct HMRC filing/submission;
- Open Banking/live feeds;
- cloud OCR as required/default path;
- AI/LLM accounting judgement or autonomous actions;
- browser/PWA persistence/encryption/OCR parity;
- generic shell execution;
- raw SQL/database access;
- broad filesystem operations;
- arbitrary executable/model path execution.

## 6. File-selection boundary

For statement/document selection the webview must never provide an arbitrary OS path for Rust to trust.

The 7B implementation must use a bounded native selection/approval flow that returns an opaque selection/document identifier plus only the metadata required by the typed application operation. Any new native picker/plugin dependency is a formal dependency/change-control event and must be exact-pinned/reviewed before merge.

## 7. Composition rules

### Money in/out
The UI supplies owner facts. Rust/domain code produces the accounting plan. The approved facade validates and persists. Vue does not calculate balancing entries.

### Bank import
Preview and duplicate classification occur before acceptance. A separate explicit confirm operation persists only the reviewed result. Heuristic similarity is not deletion authority.

### Receipt flow
`DOCUMENT.SELECT_REGISTER -> OCR.EXTRACT (optional) -> RECEIPT.SUGGEST_BANK -> RECEIPT.CONFIRM_BANK or RECEIPT.REJECT_BANK`

OCR unavailable/failure may skip `OCR.EXTRACT` and continue through manual document handling.

### Reconciliation
`BANK.RECONCILE_PREVIEW -> resolve outstanding difference -> BANK.RECONCILE_FINALISE`

Finalise remains unavailable unless the proven exact-zero/all-cleared invariant is satisfied.

## 8. 7B native/change-control classification

Because required owner capabilities exceed the seven current native commands, SBC-7B is expected to modify the native/Tauri application surface.

Before 7B merge:
- exact command names and DTOs must be finite and allowlisted;
- command permissions/build manifest/capability changes must be reviewed together;
- any dependency or native configuration change must trigger SBC-1G change control;
- Windows regression is mandatory;
- Apple physical-device and Simulator compile regression is mandatory if native code/config/dependency graph changes;
- current accounting/foundation/OCR regressions remain mandatory where the changed surface can affect them.

This matrix does not itself approve a dependency or implementation technique.

## 9. 7A freeze condition

This matrix is frozen with the owner UI contract only if repository validation confirms:
- the seven-command entry inventory is still accurate;
- the current frontend remains the prior engineering shell;
- `product/` and `workspace/` are unchanged by 7A;
- no dependency/lockfile change occurred;
- the operation list contains no deferred/forbidden authority.