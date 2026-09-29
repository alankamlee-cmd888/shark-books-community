# SBC-8 Batch 1 — Commercial Engine Contract

**Date:** 14 September 2026  
**Status:** FROZEN CONTRACT — PREACTIVATION INCUBATION  
**Entry protected main:** `8f55eaf67b311a7c2d69ebea86ae702e4934a56c`

## 1. Purpose

Build the first SBC-8 background batch as an isolated, Shark-owned, zero-dependency Rust domain engine covering the commercial lifecycle that can later be adapted into the final SBC-7 foundation.

This batch deliberately combines the closely related domain areas from frozen SBC-8A, SBC-8B and SBC-8C while keeping integration blocked.

## 2. Exact incubation scope

### 2.1 Commercial identity and snapshots

- bounded customer/supplier/contact identity;
- bounded display/postal/email/phone fields sufficient for commercial documents;
- issued-document facts use immutable snapshots;
- later contact edits must not rewrite an issued commercial document.

### 2.2 Quotes / estimates

Required state family:

`Draft -> Issued -> Accepted | Rejected | Expired | Cancelled`

Rules:

- draft is mutable;
- issue freezes a snapshot;
- issue is non-posting;
- accept/reject/expire/cancel are explicit transitions;
- accepted quote may create a **new** draft invoice identity;
- quote-to-invoice conversion must not mutate the quote into an invoice;
- no silent issue/send.

### 2.3 Invoices / credit notes

Required invoice state family:

`Draft -> Issued -> PartPaid -> Paid`

with explicit `Overdue`, `Void` and `Credited` outcomes where valid.

Rules:

- deterministic externally supplied invoice number is validated as a distinct immutable commercial number;
- draft commercial facts may be edited before issue;
- issued invoice facts are immutable;
- integer minor units only; no float money;
- payment allocation is explicit and cannot exceed outstanding balance;
- credit note is a distinct identity linked to an issued invoice;
- credit amount cannot exceed remaining creditable invoice value;
- VAT is absent from the model;
- ledger/accounting intent is represented only as a later adapter concern.

### 2.4 Provider-neutral customer payments

Model typed semantics only; no network/provider implementation.

Required concepts:

- `PaymentIntent` with Shark operation identity and optional provider object identity;
- `PaymentEvent` with provider event identity;
- states sufficient for created/pending/succeeded/failed/cancelled/refunded handling;
- exact idempotent repeated provider events accepted as already seen;
- conflicting reuse of provider event identity fails closed;
- verified success does **not** allocate money to an invoice automatically;
- invoice allocation is a separate explicit Shark action;
- no raw card data or secret-bearing fields;
- settlement model separates gross, fee and net and enforces `gross - fee = net`.

### 2.5 Supplier bills / AP domain

Required bill state family:

`Draft -> ApprovedOpen -> PartPaid -> Paid`

plus explicit `Credited` / `Void` paths.

Rules:

- supplier reference and supplier identity form an inspectable duplicate key;
- OCR/document facts are evidence only and never authority in this pure domain engine;
- approval is explicit;
- owner payment allocation cannot exceed outstanding balance;
- supplier credit is distinct from deleting/editing the original bill;
- no ledger posting is performed here.

### 2.6 Purchase orders

Required state family:

`Draft -> Issued -> PartReceived -> Received | Cancelled`

Rules:

- issue is non-posting;
- ordered quantities and received quantities use exact integer subunits, never float;
- partial receiving is explicit and cumulative;
- over-receipt fails closed;
- PO and supplier bill remain separate identities;
- deterministic variance comparison reports facts only and never auto-approves a bill.

## 3. Shared invariants

- all money is signed/unsigned integer minor units as appropriate; no floats;
- totals use checked arithmetic and fail on overflow;
- quantities use checked integer subunits;
- IDs are bounded, printable and non-empty;
- state transitions are explicit and invalid transitions fail closed;
- audit-relevant events have distinct identities rather than destructive rewriting;
- no timestamps are generated inside the domain engine: callers supply event dates/times later through adapters where needed;
- no filesystem, database, network, shell, platform, Tauri or UI primitive may appear in runtime domain code;
- no third-party dependency is authorised in this batch.

## 4. Standalone package boundary

Authorised implementation location:

`incubator/sbc8-commercial-core/`

This package must remain outside:

- `product/shark-books-core`;
- `workspace/`;
- Tauri manifests/permissions;
- Vue/dist assets;
- existing native Cargo graphs.

The package must have its own `Cargo.toml` and lockfile and zero runtime dependencies.

## 5. Minimum proof matrix

At minimum the standalone test suite must prove:

1. bounded IDs reject empty/control/oversized values;
2. checked money addition/multiplication detects overflow;
3. quote draft mutation succeeds before issue;
4. issued quote snapshot is immutable;
5. invalid quote transitions fail;
6. accepted quote converts to a **new** draft invoice identity;
7. invoice issue freezes commercial facts;
8. invoice partial payment then final payment derives correct state;
9. invoice over-allocation fails;
10. credit note cannot exceed remaining creditable value;
11. payment success does not itself allocate an invoice;
12. exact repeated provider event is idempotent;
13. conflicting provider-event replay fails closed;
14. settlement gross/fee/net arithmetic is exact;
15. supplier bill approval is explicit;
16. supplier bill partial/final payment is exact;
17. supplier payment over-allocation fails;
18. supplier duplicate key is stable/canonical;
19. PO issue is non-posting domain state only;
20. partial receiving then full receiving works;
21. over-receipt fails;
22. PO-to-bill variance reports amount/quantity facts and has no approval side effect;
23. VAT/multi-currency/provider secrets/network/platform/persistence markers are absent from runtime source;
24. repository remains clean after proof.

## 6. Exit from Batch 1 incubation

Batch 1 may be labelled **INCUBATION DOMAIN PASS** only when:

- the exact changed-path allow-list passes;
- standalone crate has zero dependencies;
- static source boundary gate passes;
- all standalone tests pass under the pinned Rust toolchain available to the SBC programme;
- the evidence binds to one immutable incubation candidate SHA;
- evidence is recorded in Library.

This does **not** authorise production integration or protected-main merge while SBC-7 remains open.
