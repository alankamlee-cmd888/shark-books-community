# SBC-7B1 Bank Mutation / Persistence Contract — 2026-09-11

## Status

Formal executable implementation contract for **SBC-7B1 Slice 2B**. Entry protected `main`: `4b3eee65bcef24c3ee7d6c06f404aa4180a7c4d8`.

This contract follows the merged read-only Bank review Slice 2A and authorises the smallest safe persisted Bank-operation seam. It does **not** close all of SBC-7B1 and does not authorise SBC-7B2 frontend binding.

## Required architecture

`owner UI -> exact typed Tauri operation -> Shark application bridge -> deterministic core revalidation -> Shark foundation -> encrypted audited persistence`

Never:

`owner UI -> raw SQLite / Beankeeper / arbitrary filesystem path / generic shell / automatic accounting authority`

## Authorised owner operations

1. `owner_bank_import_confirm_csv`
2. `owner_bank_import_confirm_ofx_qfx`
3. `owner_bank_activity_list`
4. `owner_bank_match_confirm`
5. `owner_bank_reconcile_finalise`

No generic mutation command is permitted.

## Shark application schema change

This slice is an explicit foundation/application-schema change-control event. The Shark application schema version advances from 1 to 2 while the pinned Beankeeper database schema remains 8.

Private Shark-owned application tables inside the encrypted books database are authorised for:
- immutable canonical bank activity/provenance;
- immutable owner-confirmed activity-to-ledger match records;
- immutable reconciliation headers and membership.

Imported bank activity is **not an accounting transaction** and must not be posted to the ledger merely as a storage mechanism.

The pinned Beankeeper source and database schema are not modified.

## Bank import confirmation

Confirmation must re-run the deterministic SBC-3 parser from the bounded statement content and must not trust a stale preview object as persistence authority.

Rules:
- maximum statement content 10 MiB;
- maximum 10,000 confirmed lines;
- current preview must be non-empty and error-free;
- owner confirmation echoes are equality guards only; persisted content comes from the fresh canonical `BankLine` values;
- no OS path is accepted from the webview;
- integer GBP pence only;
- source-file/raw-record SHA-256 and provenance are preserved.

Duplicate semantics remain frozen SBC-3:
- `STRONG`: explicit duplicate when accounting content agrees;
- reused strong identity with conflicting source account/date/amount/currency: fail the whole confirmation;
- `FILE_EXACT`: explicit duplicate;
- `HEURISTIC`: never a database uniqueness rule and never automatic suppression;
- `DISTINCT`: insert as a new immutable Bank activity record.

The entire confirmed batch must be atomic: a late conflict may not leave earlier rows from the same confirmation persisted.

## Bank match confirmation

The command must re-read the persisted Bank activity and authoritative encrypted-books candidate transactions, then re-run frozen SBC-4 ranking at execution time.

Rules:
- maximum 100 unique positive candidate transaction IDs;
- selected transaction must be present in that candidate set;
- ambiguous top result fails closed;
- selected `UNMATCHED` candidate fails closed;
- explicit owner confirmation is mandatory;
- `matching::confirm_match` remains the deterministic transition authority;
- persistence may only perform `UNCLEARED -> CLEARED` on the exact Business Bank entry;
- the clearance change and immutable match record must commit atomically;
- the existing Beankeeper audited status-change primitive remains underneath the Shark foundation.

Repeated confirmation of the identical persisted match may return an explicit idempotent already-confirmed result. A conflicting second match must fail closed.

## Reconciliation finalisation

Finalisation must re-read every selected authoritative transaction and Business Bank entry, rebuild the reconciliation inputs and call frozen `matching::finalize_reconciliation` at execution time.

Rules:
- maximum 1,000 unique positive transaction IDs;
- statement ID/date and opening/ending balances are explicit owner inputs;
- every selected entry must still be `CLEARED` and already have a confirmed Bank match;
- difference must be exactly zero pence;
- no stale preview grants authority;
- persistence may only perform `CLEARED -> RECONCILED`;
- reconciliation header, membership, all status changes and their existing Beankeeper audit rows must be all-or-nothing.

Repeated finalisation with the exact same statement identity and membership may return an explicit already-finalised result. Reuse of a statement ID with different content fails closed.

## Foundation boundary

`workspace/shark-foundation/src/lib.rs` may change only as needed to initialise application schema v2 and expose exact typed Bank persistence methods/DTOs. A private implementation module is preferred.

No raw connection, generic SQL execution, arbitrary status setter or upstream Beankeeper type may cross the public Shark facade.

No new dependency or Cargo.lock change is authorised by this contract. Any need to change the Beankeeper pin, a Cargo manifest dependency graph, SQLCipher/Tauri baseline, or product-core semantics is a STOP/re-contract event.

## Frozen paths / boundaries

Unless separately re-contracted, this slice must not change:
- `product/shark-books-core/**`;
- pinned `upstream/beankeeper` commit;
- OCR-native/runtime implementation;
- `workspace/shark-tauri-spike/Cargo.toml`;
- `workspace/Cargo.lock`;
- Tauri config/capability files;
- `workspace/dist/**` frontend;
- VAT/CIS/payroll/company/partnership/tax/HMRC/Open Banking/cloud OCR/AI boundaries.

## Evidence gate

Before merge the candidate must prove:
- exact declared changed-path allow-list;
- application schema v2 initialises for new books and existing encrypted books only after the pre-open backup boundary;
- Beankeeper database schema remains 8;
- bank import batch atomicity and explicit strong/file-exact duplicate outcomes;
- no Bank activity storage posts accounting transactions;
- match confirmation is revalidated, audited and only `UNCLEARED -> CLEARED`;
- reconciliation is revalidated, exact-zero/all-cleared and only `CLEARED -> RECONCILED`;
- failed/stale/conflicting operations leave no partial mutation;
- prior Home/Money and Bank-review bridge regressions pass;
- full product-core and foundation regressions pass;
- locked Tauri compile/check passes;
- reviewed Cargo.lock remains exact;
- repository is clean after proof;
- Windows and Apple physical/Simulator evidence bind to the exact same immutable final candidate SHA.

Do not merge on partial evidence. Do not declare full SBC-7B1 complete from Slice 2B alone.