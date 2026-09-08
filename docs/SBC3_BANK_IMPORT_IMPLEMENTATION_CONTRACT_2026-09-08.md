# SBC-3 Bank Import Implementation Contract

**Date:** 8 September 2026  
**Batch:** SBC-2–5 bounded batch  
**Stage:** C / SBC-3 — Bank import  
**Entry protected main:** `99c41ce60d0fd8807be653a1c3e6d9de08064d5a`  
**Entry state:** SBC-2 PASS / MERGED

## Authorised implementation

SBC-3 extends the standalone Shark-owned `product/shark-books-core` only. It remains outside the frozen native workspace and owns no database handle, filesystem handle, Tauri capability, network client or Open Banking integration.

The bank-import layer must create immutable preview `BankLine` values before any persistence decision.

Required canonical fields include local source account identity, source format, source-file SHA-256, source record locator, posted/value dates, signed GBP minor units, description/payee/reference, optional bank-provided transaction ID/FITID, raw-record SHA-256, preview state and source provenance.

## CSV

CSV import is profile-driven and preview-first. A saved mapping profile defines delimiter, date format, mapped headers and either a signed amount column or separate debit/credit columns. Quoted fields and escaped quotes must be handled deterministically. Missing/duplicate mapped headers, malformed quoted data, invalid dates, ambiguous debit/credit rows, zero amounts, non-GBP currency and amounts that do not resolve to whole pence fail closed.

No malformed row may be silently repaired by guessing. A preview containing any row error is not commit-eligible.

## OFX/QFX

The bounded V1 parser accepts statement transaction aggregates and maps `CURDEF`, `ACCTID`, `DTPOSTED`, optional value-date fields, `TRNAMT`, mandatory `FITID`, `NAME`/`MEMO` and reference fields into canonical bank lines. V1 posting currency is GBP only.

OFX/QFX strong identity remains:

`ofx:<currency>:<institution_account_id>:<FITID>`

This preserves the source-identity rule already proven in pinned Beankeeper without directly posting through the upstream importer.

## Duplicate certainty

Duplicate identity and transaction matching remain different operations.

- `STRONG`: same explicit CSV transaction ID or OFX/QFX FITID namespace.
- `FILE_EXACT`: same file SHA-256 + source locator + raw-record SHA-256.
- `HEURISTIC`: amount/date/normalised text similarity only.
- `DISTINCT`: no duplicate indication.

Only `STRONG` and `FILE_EXACT` are eligible for automatic duplicate suppression. `HEURISTIC` is review-only and cannot silently suppress a transaction. A reused strong identity with conflicting accounting content fails closed.

## Cryptographic fingerprints

SBC-3 includes a dependency-free SHA-256 implementation in the product bank-import module so source-file and raw-record fingerprints are deterministic without adding a new third-party dependency. Known SHA-256 vectors are executable tests.

## Non-claims / exclusions

SBC-3 does not implement ledger posting, matching, reconciliation, category inference, Open Banking/live feeds, network access, OCR, AI, tax logic or UI. Matching/reconciliation remains SBC-4.

## Gate

PASS requires the committed validator to prove:

- exact six-path SBC-3 diff from the SBC-2 protected merge;
- product crate still has zero third-party dependencies and unchanged product lockfile;
- frozen native Cargo.lock exact and unchanged;
- frozen foundation and permanent SBC-1G guards PASS;
- Rust 1.98.1;
- all SBC-2 + SBC-3 tests PASS under `--locked`;
- source contains no persistence/network/Open-Banking implementation markers;
- repository clean after proof.

**HARD STOP:** do not merge SBC-3 and do not implement SBC-4 until this gate passes and is formally adjudicated.
