# SBC-8 Batch 3 — Inventory Lite Domain Contract

Date: 2026-09-14
Status: FROZEN FOR ISOLATED PREACTIVATION INCUBATION
Entry protected main: `6f85734d0e9e11089a1a47b7850a306a4ed7ba87`

## 1. Purpose

Freeze the pure-domain contract for SBC-8F1 Inventory Lite without coupling to unfinished SBC-7, unmerged SBC-8 Batch 1/2 crates, persistence, platform code or any third-party dependency.

## 2. Primitive rules

- IDs and text are bounded and reject controls.
- Quantities are positive integer units on individual stock movements.
- Derived stock balances use checked signed integer arithmetic.
- No floating point.
- No money, cost, price, COGS or valuation semantics.
- Stateful validated domain objects keep invariant-bearing fields private.
- Invalid evidence and duplicate identities fail closed.

## 3. Inventory item and location

An `InventoryItem` has stable identity, SKU, name, one explicit tracked/base unit identity and active/inactive state.

An `InventoryLocation` has stable identity, name and active/inactive state.

Batch 3 performs no implicit unit conversion. Every movement for an item must use that item's tracked unit. Future conversion requires an explicit later contract/admission.

## 4. StockMovement append/audit model

A `StockMovement` is an immutable inventory fact after construction. Its minimum facts are:

- stable movement identity;
- item identity;
- location identity;
- positive quantity;
- unit identity;
- movement kind;
- caller-supplied occurrence minute used only for deterministic ordering;
- bounded source reference;
- optional external/source identity;
- optional transfer identity;
- optional corrected-movement identity for explicit append-only correction adjustments.

Supported movement kinds:

- Opening;
- PurchaseReceipt;
- SaleConsume;
- ReturnIn;
- ReturnOut;
- AdjustmentIncrease;
- AdjustmentDecrease;
- StocktakeIncrease;
- StocktakeDecrease;
- DamageWriteOff;
- TransferIn;
- TransferOut.

Each kind has a fixed quantity direction. Quantities themselves are always positive. The signed stock effect is derived from kind + quantity.

TransferIn/TransferOut require a transfer identity. Non-transfer movements may not carry one. A correction link may only be carried by AdjustmentIncrease/AdjustmentDecrease and may not self-reference.

No API edits an existing stock movement. Corrections are represented as new movements.

## 5. Derived on-hand state

On-hand is derived from movements; it is never silently overwritten.

The engine must:

- reject duplicate movement identities before projection;
- filter by exact item/location identity;
- validate movement unit against the item's tracked unit;
- apply checked signed deltas;
- sort contribution identities deterministically by occurrence minute then movement identity;
- return a trace of contributing movement identities.

Negative derived stock is represented explicitly. Batch 3 does not clamp negative quantities to zero and does not freeze a valuation-layer negative-stock policy.

An item summary across locations derives each location balance plus the total item balance. Transfers therefore change location balances but conserve total stock when the pair is valid.

## 6. Transfers

A `StockTransferProposal` is a sealed domain proposal containing exactly one TransferOut movement and one TransferIn movement.

Required invariants:

- source and destination locations differ;
- both movements share item, unit, quantity, transfer identity, occurrence minute and source reference;
- movement identities differ;
- outbound belongs to source location;
- inbound belongs to destination location;
- pair validation fails closed on mismatch.

A transfer proposal has no persistence, PO, accounting or posting authority.

## 7. Stocktake

A `StocktakeObservation` records:

- stable observation identity;
- item and location;
- observed nonnegative quantity;
- exact tracked unit;
- occurrence minute;
- bounded source reference.

Stocktake never overwrites a balance. It compares observed quantity with derived on-hand and produces either:

- no adjustment when equal; or
- one sealed `StocktakeAdjustmentProposal` containing an append-only StocktakeIncrease or StocktakeDecrease movement.

The proposed movement links back to the observation identity as its source identity. Applying/persisting the proposal is outside Batch 3.

## 8. Explicit correction

When an already-recorded non-transfer movement needs quantity correction, the owner/application layer may create an explicit AdjustmentIncrease/AdjustmentDecrease movement linked by `corrects_movement_id` to the original. The original fact remains unchanged.

Batch 3 does not define transfer correction, serial/batch correction or accounting correction workflows.

## 9. Required regression families

At minimum prove:

- bounded IDs/text and positive quantity validation;
- item/location private invariant fields;
- item tracked-unit enforcement;
- all movement-kind signed effects;
- duplicate movement identity rejection;
- deterministic on-hand and contribution ordering;
- negative balance remains explicit, not clamped;
- cross-location item summary total;
- valid transfer conserves total quantity while shifting locations;
- malformed transfer pair rejection;
- no transfer to same location;
- stocktake equal count creates no adjustment;
- stocktake excess/shortage creates one correct append-only proposal;
- stocktake proposal links to observation and does not mutate prior movements;
- explicit correction adjustment links to original and original remains unchanged;
- representative opening -> purchase receipt -> consume -> transfer -> stocktake flow remains domain-only.

## 10. Explicit non-goals

No valuation/COGS, FIFO, moving average, money/cost/price, accounting postings, persistence, Beankeeper/SQLite, Tauri/Vue/native integration, network, manufacturing, BOM, serial/batch/lot tracking, reservations, automatic unit conversion, barcode/QR runtime, VAT/CIS/payroll/HMRC/Open Banking, AI judgement or third-party runtime dependency is admitted by this contract.