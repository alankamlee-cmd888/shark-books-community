# SBC-7B1 Bank Review Bridge Contract — 2026-09-10

## Status

Bounded implementation candidate stage inside **SBC-7B1 typed application/native bridge**. This is **Slice 2A: read-only Bank review**. It follows the merged Home + Money In + Money Out Slice 1 and does not close all of SBC-7B1.

## Entry point

Protected `main` at entry: `bbe31ba295b28e1dcfcddeabe30ae49515353e35`.

Frozen owner authority:
- `docs/SBC7A_OWNER_FACING_UI_CONTRACT_2026-09-10.md`
- `docs/SBC7A_APPLICATION_OPERATION_MATRIX_2026-09-10.md`
- Library handover `132_SBC7B1_REMAINING_TYPED_OWNER_BRIDGE_HANDOVER_2026-09-10.md`

## Purpose

Expose the already-proven deterministic SBC-3/SBC-4 Bank **review** capabilities through finite owner-safe Tauri commands without changing product-core, the frozen Shark foundation, OCR-native code, frontend code, dependencies or persistence semantics.

This slice proves a safe read-only seam before a later separately reviewed mutation/persistence slice implements import confirmation/activity persistence, match confirmation and reconciliation finalisation.

## Exact new commands

1. `owner_bank_import_preview_csv`
2. `owner_bank_import_preview_ofx_qfx`
3. `owner_bank_match_review`
4. `owner_bank_reconcile_preview`

All four commands are read-only with respect to the owner books. None may persist an import, clear an entry, finalise a reconciliation or post accounting entries.

## Architecture

Required path:

`owner UI -> exact typed Tauri command -> owner_bank_review -> frozen Shark core/foundation read APIs`

Never:

`owner UI -> raw SQLite / Beankeeper / arbitrary filesystem path / generic shell`

The bridge may open encrypted books through the existing Shark-owned `open_books_impl` and obtain authoritative `TransactionView` values by bounded positive transaction IDs. It must not accept ledger/accounting entries supplied by the webview as authority.

## Bank import preview

CSV preview maps a bounded owner DTO to the frozen `bank_import::CsvMappingProfile` and then calls `bank_import::preview_csv`.

OFX/QFX preview calls `bank_import::preview_ofx_or_qfx`.

Rules:
- statement content is bounded to 10 MiB and must be non-empty;
- no OS path, database path, executable path, model path or credential is accepted;
- amounts remain signed whole pence from the frozen parser;
- duplicate/source identity semantics remain the frozen SBC-3 semantics;
- preview is never acceptance authority;
- returned data is an owner-safe read model, not a raw `BankLine` or persistence object;
- moving beyond preview always requires a separate explicit owner confirmation operation that is outside this slice.

A later document/statement selection stage must still implement the frozen native-owned/opaque selection boundary for real file picking. This read-only parser command does not authorise arbitrary webview filesystem paths.

## Match review

`owner_bank_match_review` must:
- deterministically re-resolve the selected statement line from statement content + profile/format + line reference;
- load each candidate transaction from encrypted books by bounded positive unique transaction ID;
- derive the Business Bank entry from authoritative `TransactionView` data;
- call frozen `matching::rank_matches`;
- expose levels and reason codes without changing their semantics;
- preserve ambiguity and explicit user-confirmation requirements.

It must not call `matching::confirm_match` or change persisted clearance state.

## Reconciliation preview

`owner_bank_reconcile_preview` must:
- load selected authoritative transactions from encrypted books;
- derive exactly one Business Bank entry per selected transaction;
- preserve stored uncleared/cleared/reconciled status;
- call frozen `matching::preview_reconciliation`;
- expose computed ending balance, expected ending balance, difference, all-cleared and can-finalise facts;
- never finalise or clear anything.

The frozen exact-zero **and** all-cleared rule remains authoritative. A future finalise command must be separately explicit and tested.

## Bounds and fail-closed rules

- statement content: maximum 10 MiB;
- match candidate transaction IDs: maximum 100;
- reconciliation transaction IDs: maximum 1,000;
- IDs must be positive, unique and non-empty;
- request DTOs use `deny_unknown_fields` where applicable;
- invalid dates, unsupported statuses, ambiguous/missing Business Bank entry or malformed statement data fail closed;
- no hidden network call, generic shell, raw DB access or autonomous decision is permitted.

## Frozen boundaries

This slice must leave unchanged:
- `product/shark-books-core/**`;
- `workspace/shark-foundation/**`;
- `workspace/shark-tauri-spike/src/ocr_native.rs`;
- `workspace/shark-tauri-spike/Cargo.toml`;
- `workspace/Cargo.lock` at SHA-256 `8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61`;
- Tauri configuration/capability files;
- `workspace/dist/**` frontend;
- VAT/CIS/payroll/company/partnership/tax/HMRC/Open Banking/cloud OCR/AI boundaries.

## Explicitly deferred from this slice

Still required before full SBC-7B1 closure:
- `BANK.IMPORT_CONFIRM` and persisted `BANK.ACTIVITY_LIST`;
- `BANK.MATCH_CONFIRM`;
- `BANK.RECONCILE_FINALISE`;
- document registration/integrity/attachment;
- factual OCR owner orchestration/read model;
- receipt-to-bank confirm/reject;
- correction/reversal history support;
- required Contacts/Settings operations;
- factual report/read views beyond existing trial balance.

No production SBC-7B2 frontend binding is authorised by this slice.

## Evidence gate

Before merge:
- candidate diff must equal the declared bounded allow-list exactly;
- frozen product/foundation/OCR/frontend/native dependency paths must remain unchanged;
- all previous owner commands must remain registered;
- the four new commands must be present in Tauri AppManifest, invoke handler and exact permission allow-list;
- static boundary checks must pass;
- inherited product-core and foundation regressions must pass;
- prior `owner_app::tests` must pass;
- new `owner_bank_review::tests` must pass;
- locked Tauri compile/check must pass;
- reviewed native Cargo.lock must remain exact;
- repository must be clean after proof;
- Windows and Apple physical/Simulator evidence must bind to the same immutable candidate SHA.

Do not merge on partial evidence. Do not declare full SBC-7B1 complete from this read-only Bank review slice.
