# Shark Books Community — SBC-7B1 Batch B Supporting Data + Read Models Implementation Design

**Date:** 14 September 2026  
**Entry protected main:** `6f85734d0e9e11089a1a47b7850a306a4ed7ba87`  
**Stage:** SBC-7B1 remaining typed owner bridge — Batch B  
**Authority:** `docs/SBC7B1_DATA_READMODELS_BATCH_CONTRACT_2026-09-14.md`  
**Repair amendment:** DR-BB-10, 15 September 2026 — mobile folder selection must fail closed because the admitted Tauri dialog plugin does not implement folder picking on iOS/Android.

## 1. Objective

Implement exactly the five frozen Batch B owner commands from the post-Batch-A protected main, without adding accounting/tax authority, dependencies, frontend binding or raw native/database/path authority to the webview.

Frozen commands:

1. `owner_contacts_list`
2. `owner_contacts_save`
3. `owner_settings_books_info`
4. `owner_settings_storage_root_select`
5. `owner_report_summary`

## 2. Contact persistence

Shark application schema advances from v4 to v5 solely by adding `shark_contact`. Beankeeper remains schema 8.

Foundation DTOs:
- `ContactWrite { contact_id, kind, display_name }`
- `ContactView { contact_id, kind, display_name, created_by, created_at, updated_by, updated_at }`
- `ContactPersistOutcome::{Created, Updated, AlreadyCurrent}`

Rules:
- ID is bounded to the existing SBC-2 `RecordId` envelope (1–128 non-blank characters) and is revalidated defensively in foundation.
- kind is exactly `customer|supplier`.
- display name is bounded to the existing SBC-2 `Customer`/`Supplier` name envelope (1–256 non-blank characters).
- exact repeat returns `AlreadyCurrent` and does not rewrite timestamps.
- same ID + same kind + changed name updates only the display name/update audit fields.
- same ID + different kind fails closed.
- persistence never posts an accounting transaction.
- list is deterministic and bounded; owner bridge maximum is 200 rows.

The owner bridge must construct and validate the existing product-core `Customer` or `Supplier` before any persistence request is made.

## 3. Owner-safe books/settings read model

`owner_settings_books_info` opens the bounded books reference through the existing Shark native books boundary and combines only:
- `Books::metadata()`;
- `Books::migration_metadata()`;
- `shark_foundation::production_encryption_required()`;
- the native shell package version.

It returns company/books identity and version/support facts only. It never returns `file_name`, absolute/native database paths, key/passphrase data, environment variables, SQLCipher connection information or Beankeeper objects.

Successful open is represented factually as an encrypted-native books session; no key material is serialized.

## 4. Session-scoped document storage root

Reuse the already admitted `tauri-plugin-dialog = 2.7.3` and the existing Rust-only `NativeDocumentRootRegistry`.

The registry gains a native-only session registration helper that:
- accepts a native `PathBuf` supplied by a supported native folder picker;
- requires an existing directory;
- canonicalises the directory before storage;
- assigns/reuses an opaque `storage-root-session-N` identifier in memory;
- never serializes the canonical path.

`owner_settings_storage_root_select`:
1. validates/opens the owner books reference on every platform;
2. on supported desktop targets (Windows/macOS and other non-mobile targets), invokes the admitted native folder picker with `blocking_pick_folder()`;
3. desktop cancellation returns a typed `cancelled` outcome without registry mutation;
4. a selected desktop native path is registered in the Rust registry;
5. the desktop response contains only bridge version, opaque root ID, generic safe label and explicit `deviceSession` scope;
6. on iOS/Android, where `tauri-plugin-dialog` does not implement folder picking, the command compiles but fails closed with the owner-safe error code `unsupportedPlatform`;
7. the mobile unsupported path performs no registry mutation and returns no native path, URL, provider token or filesystem authority;
8. file picking must not be substituted for folder picking merely to make the mobile build pass.

A real iPhone/iPad storage-root adapter is deferred to SBC-7B2/native mobile work and must preserve the same opaque-root/no-raw-path boundary.

No root path is persisted in the encrypted books and no cloud credentials/provider mapping is introduced.

## 5. Factual reports

`owner_report_summary` reads authoritative `Books::trial_balance()` and returns a factual GBP snapshot.

Deterministic sign handling:
- revenue contribution = `credit_total - debit_total`;
- expense contribution = `debit_total - credit_total`;
- Business Bank (`1000`) balance = `debit_total - credit_total` when present;
- Cash (`1010`) balance = `debit_total - credit_total` when present.

The two balance fields are optional when their accounts are absent.

All intermediate arithmetic uses checked wider arithmetic and conversion back to signed whole-pence `i64`; representational overflow fails closed. `Books::trial_balance()` is also hardened to checked totals so this read path cannot overflow before the owner report layer receives the rows.

`books_balanced` is factual only and requires the authoritative trial-balance balanced flag with equal total debits/credits. Currency is exactly `GBP`.

No tax estimate, reserve, MTD computation, deductibility, compliance conclusion, forecast, date-filtered P&L or balance-sheet claim is produced.

## 6. Native command boundary

One new module `workspace/shark-tauri-spike/src/owner_supporting_data.rs` owns all five commands and owner-safe DTOs/errors.

The module may use:
- `shark_books_core::{RecordId, Customer, Supplier}` only for contact validation;
- `shark_foundation` typed facade/read DTOs;
- `NativeDocumentRootRegistry` for opaque session-root registration;
- existing `open_books_impl`/`OpenBooksRequest` for bounded encrypted books access;
- `tauri_plugin_dialog::DialogExt` only behind a non-mobile target guard for the supported native desktop folder picker.

The mobile iOS/Android build must not reference the unsupported folder-picker method. It must retain the command identity and fail closed through the typed owner-safe error surface instead.

It must not use raw SQLite, Beankeeper DB objects, arbitrary path input, generic shell/network APIs or frontend-supplied filesystem paths.

## 7. Exact planned changed-path envelope

The implementation is expected to use exactly these paths unless a defect requires an explicit amendment before candidate freeze:

1. `ci/run_sbc7b1_data_readmodels_codemagic.sh`
2. `ci/run_sbc7b1_data_readmodels_windows.ps1`
3. `codemagic.yaml`
4. `docs/SBC7B1_DATA_READMODELS_IMPLEMENTATION_DESIGN_2026-09-14.md`
5. `scripts/check_sbc7b1_data_readmodels.py`
6. `workspace/shark-foundation/src/bank_application/mod.rs`
7. `workspace/shark-foundation/src/bank_application/tests.rs`
8. `workspace/shark-foundation/src/contact_application.rs`
9. `workspace/shark-foundation/src/lib.rs`
10. `workspace/shark-tauri-spike/build.rs`
11. `workspace/shark-tauri-spike/permissions/shark-shell.toml`
12. `workspace/shark-tauri-spike/src/lib.rs`
13. `workspace/shark-tauri-spike/src/owner_documents_ocr.rs`
14. `workspace/shark-tauri-spike/src/owner_supporting_data.rs`

No Cargo manifest or Cargo.lock change is authorised. Product core and frozen frontend remain unchanged.

## 8. Proof matrix

Static/runtime proof must demonstrate:
- exact entry ancestry and exact final allow-list;
- native Cargo.lock byte-identical to Batch A final lock;
- no dependency/product/frontend drift;
- Shark application schema v5 and Beankeeper schema 8;
- v4 books migrate to v5 while preserving existing Batch A tables/data;
- contact create/update/list/idempotency/kind-conflict/domain-validation/non-posting;
- books-info has no raw path/key/database authority;
- supported desktop storage-root selection returns only opaque ID/safe status, is session-scoped, canonicalized and cancellation-safe;
- iOS/Android storage-root selection compiles and fails closed with `unsupportedPlatform`, with no raw path/URL/provider authority and no registry mutation;
- `blocking_pick_folder()` remains confined to the non-mobile target branch;
- factual report sign/account handling and overflow fail-closed;
- inherited product/foundation/owner/Bank/Documents/OCR/Mutation+Audit tests remain PASS;
- frontend remains intentionally unwired from all five new commands;
- exact Tauri manifest/permission wiring;
- Windows locked compile/runtime PASS;
- exact same immutable SHA Apple physical + Simulator compile/regression PASS;
- repository clean before and after each authoritative gate.

## 9. Exit

No merge occurs before exact same-SHA Windows + Apple evidence is adjudicated PASS. After protected merge, reconcile the complete SBC-7B1 command inventory against SBC-7A; if complete, close SBC-7B1 and release SBC-7B2 Vue binding.
