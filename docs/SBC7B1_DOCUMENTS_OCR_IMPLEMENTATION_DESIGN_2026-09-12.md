# SBC-7B1 Slice 3A — Documents + factual OCR implementation design

**Date:** 12 September 2026  
**Entry protected main:** `50c898c7b8042e11810933e3e596fc124085e831`  
**Branch:** `sbc7b1-documents-ocr-owner-bridge-20260912`  
**Status:** IMPLEMENTATION DESIGN FROZEN — production implementation not yet proven

## Decision summary

The design review concludes that Slice 3A requires two narrowly-scoped changes that were deliberately not pre-authorised by the entry contract:

1. **Shark application schema v2 -> v3 is required.** Existing Beankeeper attachment storage is not suitable because `Books::attach_document` calls Beankeeper `hash_and_store_file`, which copies the source into Beankeeper's own attachment area. That conflicts with the frozen SBC-5 model of user-controlled document custody with Shark persisting only canonical references/integrity metadata.
2. **One native picker dependency is required:** official `tauri-plugin-dialog`, pinned exactly at `=2.7.3`. It is MIT OR Apache-2.0 and supports file picking on Windows and iOS. No JavaScript plugin package is required because Slice 3A invokes the picker only inside the Rust owner command. No folder-picker or root-management UI is added.

No other new dependency is authorised.

## Why application schema v3 is necessary

Slice 2B application schema v2 owns only bank activity, confirmed bank matches and reconciliations. A receipt/document must be durable before it is attached and must survive reopen so that integrity verification and OCR can re-read the canonical identity. The SBC-5 core `DocumentReference` is platform-neutral and persistence-free; it is not itself a durable store.

The existing Beankeeper attachment table cannot be used as the canonical document store because its current Shark facade copies the source file into a database-adjacent attachment directory. Slice 3A must instead persist only:

- opaque document identity;
- opaque storage-root identity;
- portable relative reference;
- original filename/media type when known;
- SHA-256 and byte length derived from actual selected bytes;
- registration actor/time;
- immutable document-to-record relationship metadata.

The actual document bytes remain in a user-controlled local or synced storage root.

## Schema v3

`SHARK_APPLICATION_SCHEMA_VERSION` becomes `3`; pinned Beankeeper database schema remains exactly `8`.

The existing backed-up open ordering remains mandatory: backup occurs before Beankeeper open and before Shark application migration.

The v3 migration is one savepoint and adds exactly two Shark-owned tables.

### `shark_document`

Required fields:

- `company_slug TEXT NOT NULL`
- `document_id TEXT NOT NULL`
- `storage_root_id TEXT NOT NULL`
- `relative_path TEXT NOT NULL`
- `original_filename TEXT NOT NULL`
- `media_type TEXT`
- `sha256 TEXT NOT NULL`
- `byte_len INTEGER NOT NULL`
- `registered_by TEXT NOT NULL`
- `registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP`

Constraints:

- primary key `(company_slug, document_id)`;
- unique `(company_slug, storage_root_id, relative_path)` so an immutable reference cannot silently alias overwritten content;
- SHA-256 exactly 64 hex characters at the Shark validation boundary;
- byte length `> 0` and `<= 25 MiB` for this slice;
- all IDs/text bounded before persistence;
- relative paths must remain portable and traversal-free.

### `shark_document_attachment`

Required fields:

- `company_slug TEXT NOT NULL`
- `document_id TEXT NOT NULL`
- `record_kind TEXT NOT NULL`
- `record_id TEXT NOT NULL`
- `transaction_id INTEGER NOT NULL`
- `attached_by TEXT NOT NULL`
- `attached_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP`

Slice 3A permits only `record_kind IN ('moneyIn','moneyOut')` because those are the owner-created accounting records already proven by SBC-7B1 Slice 1. General cross-family attachment is not required to close the current receipt journey.

Constraints:

- primary key `(company_slug, document_id, record_kind, record_id)`;
- foreign key to the registered document;
- foreign key to the authoritative transaction;
- repeated identical attachment returns explicit `AlreadyAttached` rather than creating another row;
- conflicting record/transaction identity fails closed.

No document bytes are stored in either table.

## Foundation API

A new `workspace/shark-foundation/src/document_application.rs` module owns typed persistence. Tauri/UI code must never access raw SQLite or Beankeeper attachment primitives.

Planned public Shark foundation types:

- `DocumentWrite`
- `DocumentView`
- `DocumentPersistOutcome::{Registered, AlreadyRegistered}`
- `DocumentAttachmentWrite`
- `DocumentAttachmentView`
- `DocumentAttachmentPersistOutcome::{Attached, AlreadyAttached}`

Planned `Books` operations:

- `register_document`
- `document`
- `attach_registered_document`
- `document_attachments` only if required by tests/readback; no generic attachment CRUD.

Registration is idempotent only when the persisted canonical identity exactly matches. Same ID or same root/path with conflicting hash/length/metadata fails closed.

The old `Books::attach_document` Beankeeper-copying method remains frozen/historical and is **not called** by Slice 3A.

## Native root and file-selection boundary

Slice 3A introduces a Rust-only `NativeDocumentRootRegistry` managed as Tauri state. It maps bounded opaque storage-root IDs to native `PathBuf` roots. The path never crosses a serializable owner DTO.

Root creation/editing remains outside Slice 3A. The registry therefore accepts only roots already configured by a trusted native/settings layer. Until the later Settings bridge populates a root, owner document registration must fail explicitly with `storageRootNotConfigured`; it must never invent or silently fall back to a Shark-controlled cloud location.

### Native picker

`owner_document_select_register` invokes the official Rust dialog plugin itself. The owner request contains no source path and no `file://` URI.

Dependency:

```toml
tauri-plugin-dialog = { version = "=2.7.3", default-features = false }
```

If platform requirements force the crate's own mandatory transitive `tauri-plugin-fs` dependency, that is accepted only as transitive dependency of the selected official plugin; Slice 3A does not expose its filesystem commands to the webview.

The plugin is initialized in Rust with `tauri_plugin_dialog::init()`. No `@tauri-apps/plugin-dialog` JavaScript dependency or dialog ACL is added because the webview never calls the plugin directly.

The current upstream plugin reports full Windows support and partial iOS support, where folder picking is not supported. That is compatible with this slice because storage-root management/folder selection is explicitly outside scope.

There are known upstream iOS concerns around persistent security-scoped external picker resources. Therefore Slice 3A must **not persist the external picker path/URL**. The selected file is read during the native selection operation and copied immediately into an already configured user-controlled root. Only the root ID and portable relative reference are persisted.

## Copy/register algorithm

`owner_document_select_register` performs the following fail-closed sequence:

1. validate books reference and requested opaque storage-root ID;
2. confirm the root exists in `NativeDocumentRootRegistry`;
3. invoke the native file picker;
4. cancellation returns explicit `cancelled` without mutation;
5. obtain native file access from the picker result; no path is serialized;
6. read at most 25 MiB + 1 byte and reject empty/oversized input;
7. derive SHA-256 using the already-proven dependency-free `shark_books_core::bank_import::sha256_hex`;
8. derive/validate a portable original filename;
9. derive an opaque deterministic document ID from the content hash: `doc-<64 lowercase sha256>`;
10. derive a native-owned relative destination under `documents/<sha256>/<filename>`;
11. write to a temporary file inside the configured root, re-read/verify hash + length, then atomically rename; never overwrite conflicting existing content;
12. persist canonical metadata through `Books::register_document`;
13. on persistence failure, remove only a newly-created destination owned by this operation; never delete a pre-existing verified file;
14. return bounded metadata only.

If the same content has already been registered with exactly the same canonical metadata, return `AlreadyRegistered`. Conflicting immutable identity fails closed.

## Integrity verification

`owner_document_verify` accepts only books context + opaque `document_id`.

It:

1. reads `DocumentView` from the encrypted books database;
2. resolves its `storage_root_id` through the native root registry;
3. joins the validated stored relative path under that trusted root;
4. rejects any path that escapes the root;
5. reads the current bytes with the 25 MiB bound;
6. compares current length then SHA-256;
7. returns exactly one bounded state: `verified`, `sizeMismatch`, `hashMismatch`, `missing`, or explicit operation failure.

It never mutates an accounting record.

## Attachment operation

`owner_document_attach` accepts:

- books context;
- `document_id`;
- `record_kind` (`moneyIn` or `moneyOut` only);
- `record_id`.

It does not accept raw transaction ID as authority.

The owner bridge reconstructs the frozen owner transaction reference `sbc7b1:<recordKind>:<recordId>` and rereads the authoritative Shark transaction. Exactly one existing transaction must resolve. The current document must verify successfully before attachment.

The relationship is then persisted through `Books::attach_registered_document`. No Beankeeper file-copy attachment API is invoked, no posting occurs and no account/tax fields are accepted.

## OCR orchestration

The existing B3C `ocr_extract_receipt` command remains intact for regression compatibility. `ocr_native.rs` gains a crate-private reusable implementation function so the new owner bridge can invoke the exact same bounded OCR runtime without duplicating process/security logic.

`owner_ocr_extract_receipt` accepts only:

- books context;
- bounded OCR request ID;
- opaque document ID.

Sequence:

1. reread canonical document metadata from encrypted books;
2. resolve trusted root + relative path natively;
3. verify size/hash before OCR;
4. create/refresh the existing `NativeApprovedOcrDocument` entry internally;
5. invoke the existing B3C implementation;
6. return the existing factual `Completed / Unavailable / Failed` shape.

The current Windows OCR runtime supports PNG/JPEG/BMP bytes. Other documents may still be registered/attached, but OCR must return the frozen unsupported/failed outcome rather than pretending success.

On Apple, the existing OCR runtime remains `UnsupportedPlatform`; Apple proof is compile/boundary proof, not a false OCR runtime claim.

## Frontend/security boundaries

The following remain forbidden in every new serializable request:

- source path, destination path or database path;
- file URL;
- storage-root native path;
- passphrase/key/token;
- hash/length asserted by the UI as authority;
- posting lines/account IDs;
- business/private/category/deductibility/tax fields;
- match/reconciliation mutation fields;
- executable/model/shell arguments;
- network endpoints.

`workspace/dist/**`, Tauri capability JSON and Tauri configuration remain unchanged. New commands are added only to the existing exact Shark application command manifest/permission list.

## Exact implementation allow-list

The final Slice 3A candidate may differ from entry protected main in exactly these **17 paths** and no others:

1. `ci/run_sbc7b1_documents_ocr_codemagic.sh`
2. `ci/run_sbc7b1_documents_ocr_windows.ps1`
3. `codemagic.yaml`
4. `docs/SBC7B1_DOCUMENTS_OCR_OWNER_BRIDGE_CONTRACT_2026-09-12.md`
5. `docs/SBC7B1_DOCUMENTS_OCR_IMPLEMENTATION_DESIGN_2026-09-12.md`
6. `scripts/check_sbc7b1_documents_ocr.py`
7. `workspace/Cargo.lock`
8. `workspace/shark-foundation/src/bank_application/mod.rs`
9. `workspace/shark-foundation/src/document_application.rs`
10. `workspace/shark-foundation/src/lib.rs`
11. `workspace/shark-tauri-spike/Cargo.toml`
12. `workspace/shark-tauri-spike/build.rs`
13. `workspace/shark-tauri-spike/permissions/shark-shell.toml`
14. `workspace/shark-tauri-spike/src/lib.rs`
15. `workspace/shark-tauri-spike/src/ocr_native.rs`
16. `workspace/shark-tauri-spike/src/owner_documents_ocr.rs`
17. `workspace/shark-tauri-spike/src/owner_app.rs`

`owner_app.rs` is included only to share/reuse the existing bounded books reference/error surface if that can be done without changing established command semantics. If implementation proves no edit is needed, the final candidate allow-list must be reduced rather than forcing a needless change.

No product-core source, product Cargo files, Beankeeper source, OCR Python/ONNX runtime/model, prior bank bridge source, Tauri configuration/capability or frontend file is authorised to change.

## Dependency/change-control gate

Because this is a native dependency event, the validator must prove:

- the only direct dependency added is exact `tauri-plugin-dialog = 2.7.3`;
- selected plugin licence is `MIT OR Apache-2.0`;
- no JavaScript package dependency is added;
- `workspace/Cargo.lock` is regenerated only through Cargo and its candidate SHA-256 is frozen before evidence;
- product-core remains zero-third-party-dependency and its lockfile remains exact;
- Beankeeper pin/source remains unchanged;
- no network/cloud library is introduced;
- no generic shell plugin is introduced;
- existing Tauri/Cargo exact pins remain unchanged.

## Required executable evidence

Windows candidate proof must cover at least:

- all inherited product-core tests;
- SBC2->5 combined regression;
- receipt-flow regression;
- all foundation/bank persistence regressions;
- prior owner Home/Money bridge;
- prior Bank Review bridge;
- prior Bank mutation bridge;
- new schema-v3 migration/open/reopen tests;
- document register/read/idempotency/conflict tests;
- root/path traversal escape rejection;
- byte/hash verification and tamper detection;
- missing root/document outcomes;
- typed moneyIn/moneyOut attachment and duplicate semantics;
- proof that attachment does not post accounting entries;
- owner DTO rejection of path/hash/key/account/tax-shaped fields;
- OCR only after canonical integrity verification;
- inherited B3C OCR tests;
- frontend remains unwired;
- exact Tauri ACL/CSP surface;
- locked Windows compile;
- repository clean after proof.

Apple/Codemagic proof on the same immutable candidate must cover static governance, inherited regressions, new pure-Rust/document tests, owner bridge tests, frozen explicit OCR `UnsupportedPlatform` behaviour where applicable, and successful compilation for both `aarch64-apple-ios` and `aarch64-apple-ios-sim`.

## Candidate freeze rule

This document freezes the design, **not** the final candidate SHA. Implementation may now begin only within the allow-list above. After implementation and local/static proof:

1. reduce the allow-list if any pre-authorised path was not actually needed;
2. record the exact changed paths and dependency diff;
3. freeze the new native Cargo.lock SHA-256;
4. freeze one immutable candidate commit;
5. run Windows evidence;
6. only after Windows PASS, run Codemagic Apple evidence on the same SHA;
7. merge PR #21 only after dual-platform PASS.
