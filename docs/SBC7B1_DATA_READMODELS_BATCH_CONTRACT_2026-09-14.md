# Shark Books Community — SBC-7B1 Batch B Supporting Data + Read Models Contract

**Date:** 14 September 2026  
**Planning entry main:** `40fcd4d8a65e9b02337a198d8294d37cc0daee67`  
**Stage:** SBC-7B1 remaining typed owner bridge — Batch B  
**Disposition:** CONTRACT FROZEN / IMPLEMENTATION BLOCKED UNTIL BATCH A MERGES

## 1. Objective

After Batch A has passed and merged, add the smallest remaining typed owner-facing supporting data/read surfaces required by the frozen SBC-7A journeys before SBC-7B2 Vue binding begins.

Scope is deliberately limited to:
- minimal customer/supplier Contacts persistence and read models;
- owner-safe books/settings information;
- bounded native document storage-root selection/registration for the current device session;
- simple factual bookkeeping report/read model derived from authoritative existing books data.

No accounting rule, tax conclusion, cloud service, contact CRM, invoice engine or broad settings framework is authorised.

## 2. Frozen owner command family

1. `owner_contacts_list`
2. `owner_contacts_save`
3. `owner_settings_books_info`
4. `owner_settings_storage_root_select`
5. `owner_report_summary`

Existing `books_trial_balance` remains available as a factual engineering/application primitive but is not sufficient by itself as the owner-friendly Reports surface.

## 3. Contacts contract

Contacts exist only to support customer/supplier selection in the frozen owner journeys.

### Contact shape

Minimum persisted fields:
- bounded contact ID;
- kind: `customer|supplier`;
- display name;
- created/updated actor and timestamps.

Do not add address-book sync, phone/email marketing fields, credit limits, tax/VAT IDs, payment terms, CRM notes or external network lookups in this batch.

`owner_contacts_save` must:
- validate the existing SBC-2 `Customer` / `Supplier` domain DTO before persistence;
- create a new contact or update the display name of the same ID/kind;
- fail closed if an existing ID is reused with a different kind;
- be idempotent for an exact repeat;
- never create ledger/accounting postings.

`owner_contacts_list` is read-only, bounded and ordered deterministically.

## 4. Settings books-info contract

`owner_settings_books_info` must provide an owner-safe factual DTO derived from existing `Books::metadata`, `migration_metadata`, encryption requirement and current app version information.

Allowed information includes:
- books/company name;
- opaque books ID;
- books/application format/schema version numbers where useful for support/about;
- encryption required/active contract status;
- application/facade version/support metadata.

It must not expose:
- raw database paths;
- encryption keys/passphrases;
- SQLite/SQLCipher connection details;
- Beankeeper objects;
- arbitrary environment variables.

## 5. Device-session storage-root selection

The existing Slice 3A `NativeDocumentRootRegistry` is Rust-only and deliberately stores canonical OS paths only in native memory.

`owner_settings_storage_root_select` may expose a bounded native folder picker using the already-reviewed `tauri-plugin-dialog = 2.7.3` dependency. The command must:
- let the native layer choose the folder;
- canonicalise and register it in `NativeDocumentRootRegistry`;
- return only an opaque storage-root ID and an owner-safe status/label;
- never serialize the full OS path to the webview;
- mark the registration explicitly as device/session scoped in V1;
- permit cancellation without mutation.

Persistent cross-device folder mapping, cloud-provider credentials and broad filesystem browsing are outside this batch. A future settings/storage feature may supersede this session-only registration under separate change control.

## 6. Factual report/read model

`owner_report_summary` is a factual bookkeeping snapshot only. It must derive from authoritative current books data (principally the existing trial balance) and must not infer tax liability, tax reserve, deductibility or compliance conclusions.

Minimum owner-safe fields:
- recorded Money In total in whole pence from revenue accounts;
- recorded Money Out total in whole pence from expense accounts;
- Business Bank balance in whole pence when account `1000` exists;
- Cash balance in whole pence when account `1010` exists;
- books-balanced factual flag;
- currency `GBP`.

Arithmetic must be checked and deterministic. The UI later owns friendly wording; this command does not expose debit/credit entry lines as required user concepts.

No date-filtered P&L, tax estimate, MTD calculation, cash-flow forecast, balance sheet claim or export format is authorised here.

## 7. Shark application schema v5

Batch B is expected to begin from Batch A's application schema v4 and advance Shark application schema to v5 solely for minimal contact persistence. Beankeeper remains exactly schema 8.

Only one new persistent table family is authorised:

### `shark_contact`
At minimum:
- company slug;
- contact ID;
- kind `customer|supplier`;
- display name;
- created/updated actor and timestamps;
- primary key `(company_slug, contact_id)`.

No storage-root path is persisted in the encrypted books by this batch; storage-root registration remains native device-session state.

Existing backup-before-open/migration ordering remains mandatory.

## 8. Pre-authorised implementation path envelope

The Batch B implementation candidate may touch only the following categories unless an explicit amendment is committed before evidence:

- this contract document;
- `workspace/shark-foundation/src/lib.rs`;
- `workspace/shark-foundation/src/bank_application/mod.rs`;
- `workspace/shark-foundation/src/bank_application/tests.rs` only for the v4->v5 schema regression update if required;
- one new Shark foundation contact-persistence module and focused tests;
- `workspace/shark-tauri-spike/src/owner_documents_ocr.rs` only to use/register the already-existing native storage-root registry;
- one new typed owner supporting-data/read-model bridge module;
- `workspace/shark-tauri-spike/src/lib.rs`;
- `workspace/shark-tauri-spike/build.rs`;
- `workspace/shark-tauri-spike/permissions/shark-shell.toml`;
- one focused static/runtime validator;
- one Windows runner;
- one Codemagic Apple runner;
- `codemagic.yaml` only to register the Batch B Apple workflow.

No product-core source change is expected. `workspace/Cargo.lock` is expected to remain byte-identical because no new dependency is authorised.

The final candidate must freeze an exact changed-file list; this envelope is not permission for unused paths to drift.

## 9. Proof obligations

Before merge, prove at minimum:
- exact candidate diff/allow-list;
- application schema v5 separate from Beankeeper schema 8;
- all inherited product/foundation/owner regressions including Batch A remain PASS;
- Contacts create/update/list, ID/kind conflict, validation and idempotent cases PASS;
- settings books-info contains no raw path/key/database authority;
- storage-root selection/registration is native-owned, path-free to the webview and cancellation-safe;
- report whole-pence arithmetic and account-type/sign handling PASS including overflow fail-closed cases;
- no tax/compliance wording or computation is introduced;
- frontend remains unwired;
- exact Tauri ACL/CSP command set;
- locked Windows compile/runtime proof;
- same immutable SHA Apple physical + Simulator compile/regression proof;
- Cargo.lock and repository clean before/after.

## 10. Exit

After same-SHA Windows + Apple PASS and protected merge, reconcile the complete SBC-7B1 command inventory against the frozen SBC-7A operation matrix. If every required typed bridge is present and proven, close full SBC-7B1 and release SBC-7B2 owner-facing Vue binding. SBC-7C remains blocked until SBC-7B2 passes.