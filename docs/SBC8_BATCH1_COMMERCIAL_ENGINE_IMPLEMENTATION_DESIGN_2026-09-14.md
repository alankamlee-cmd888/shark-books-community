# SBC-8 Batch 1 — Commercial Engine Implementation Design

**Date:** 14 September 2026  
**Status:** DESIGN FROZEN — PREACTIVATION INCUBATION

## 1. Package design

Standalone crate:

`incubator/sbc8-commercial-core`

No dependency on Shark Books core, Shark foundation, Beankeeper, Tauri, Vue, SQLite, network/provider SDKs or platform code.

The crate is intentionally adapter-neutral. Later SBC-8 integration will map these domain objects into the then-current typed Shark foundation rather than teaching the domain engine about persistence.

## 2. Modules

- `primitives.rs` — bounded IDs, integer money, integer quantity, checked arithmetic and canonical reference keys.
- `identity.rs` — commercial party identity and immutable commercial party snapshot.
- `documents.rs` — line items, quotes, invoices, credit notes and immutable issue snapshots.
- `payments.rs` — provider-neutral intents/events, event idempotency, explicit invoice allocations and settlement arithmetic.
- `purchasing.rs` — supplier bills, supplier credit/payment handling, purchase orders, receipts and deterministic PO/bill variance.
- `lib.rs` — public typed surface only.

## 3. Primitive rules

### Bounded IDs

`EntityId` validates:

- 1–128 bytes after trimming;
- no control characters;
- printable semantic identifier only;
- the canonical stored form is trimmed.

### Money

`Money` stores signed `i64` minor units. Commercial line prices and payment/credit amounts use positive constructors. All arithmetic uses checked `i64`/`i128` conversions. Currency is intentionally not polymorphic in this incubation batch: values are GBP minor units by contract.

### Quantity

`Quantity` stores positive integer subunits. The engine does not choose whether one subunit means an item, gram, millilitre or thousandth-unit; the later adapter/catalog contract owns that meaning. The only invariant here is deterministic integral arithmetic.

## 4. Commercial lines

`CommercialLine` contains:

- immutable line identity;
- bounded description;
- positive quantity subunits;
- non-negative unit price in minor units.

Line total = checked `quantity * unit_price`. No tax/VAT field exists.

## 5. Party snapshot

Draft documents may refer to a current `CommercialParty`. Issuing captures a `CommercialPartySnapshot` by value. Issued document views contain the snapshot and never retain a live mutable reference to the party.

## 6. Quote aggregate

`Quote` owns:

- identity;
- customer snapshot candidate;
- mutable draft lines;
- state;
- optional immutable `IssuedQuoteSnapshot`.

Only Draft permits line/customer edits. `issue(number)` freezes the snapshot. `accept`, `reject`, `expire`, `cancel` enforce exact state transitions.

`to_draft_invoice(new_invoice_id)` is permitted only from Accepted. It creates a separate `Invoice` with copied commercial facts and `source_quote_id`; it never mutates the quote.

## 7. Invoice aggregate

`Invoice` owns:

- invoice identity;
- optional source quote identity;
- customer facts;
- draft lines;
- state;
- optional immutable issued snapshot;
- allocated paid amount;
- credited amount.

Issue freezes the invoice number, customer snapshot and lines. Payment allocation is explicit and checked against `issued_total - credited_amount - paid_amount`.

`refresh_due_state(is_overdue)` may derive Overdue only for an issued unpaid balance and may restore Issued/PartPaid when no longer overdue. Paid remains Paid.

Credit notes are separate `CreditNote` values linked to one invoice. Creating one reserves credit against the invoice and cannot exceed the uncredited invoice total or create a negative outstanding balance.

Void is allowed only on an issued invoice with zero paid and zero credited amount. Credited is reached when credits consume the full invoice total.

## 8. Payment domain

`PaymentIntent` contains no secret/card data. It holds:

- Shark operation ID;
- invoice ID;
- amount;
- state;
- optional provider object ID.

`PaymentEventRegistry` is an in-memory domain structure used to prove semantics, not production storage. It records `PaymentEvent` by provider event ID:

- exact repeated event => `AlreadySeen`;
- same event ID with different payload => error;
- verified `Succeeded` event may change the `PaymentIntent` state but does not touch an invoice.

Invoice allocation is a separate `apply_payment` operation.

`Settlement` requires positive gross, non-negative fee and exact `gross - fee = net`; fees may not exceed gross.

## 9. Supplier bill domain

`SupplierBill` mirrors the commercial control model while remaining separate from invoices:

- Draft is mutable;
- `approve` creates ApprovedOpen;
- payment application is explicit;
- partial/full payment states derive deterministically;
- credit is distinct and bounded;
- void requires no allocated payment/credit.

A canonical duplicate key is derived from supplier ID + normalised supplier reference. It is an inspectable fact only; persistence later decides uniqueness policy.

## 10. Purchase orders

`PurchaseOrder` contains ordered lines and cumulative received subunits. Only Draft permits line editing. Issue freezes the ordered line facts. `receive(line_id, quantity)` performs checked cumulative receipt and rejects over-receipt.

State becomes PartReceived after any positive receipt and Received when every ordered line is fully received.

`compare_bill` returns `PurchaseVariance` facts by matching line identities supplied by the caller. It reports quantity and amount differences only. It does not approve/post anything.

## 11. Deliberate omissions

The following are absent by design:

- VAT/tax fields;
- multi-currency;
- PDF/document rendering implementation;
- email/share delivery;
- Stripe/provider SDK;
- webhooks/network;
- ledger account codes/posting plans;
- persistence/database;
- Tauri/UI;
- OCR/filesystem;
- timestamps generated by the domain engine;
- AI classification.

## 12. Evidence strategy

The validator will require an exact incubation allow-list and scan runtime source for forbidden platform/persistence/network/tax markers. The Windows proof runner will execute the standalone crate with `cargo test --locked`, preserve an external target directory, emit a JSON result and ZIP evidence, and verify repository cleanliness.

Apple proof is deferred because this crate is not admitted into the native product graph. At later SBC-8P integration it must be compiled/proven with the final product target set as part of admission/integration evidence.
