# SBC-8 Batch 3 — Inventory Lite Preactivation Incubation Authority

Date: 2026-09-14
Status: PREACTIVATION INCUBATION AUTHORISED / PRODUCTION INTEGRATION BLOCKED
Entry protected main: `6f85734d0e9e11089a1a47b7850a306a4ed7ba87`

## Authority

The owner-authorised SBC-8 preactivation amendment permits isolated Shark-owned pure-domain work while SBC-7 remains open. This Batch 3 authority applies only to SBC-8F1 Inventory Lite.

Batch 1 Commercial Engine and Batch 2 Operations + Forecast Engine have separate exact-SHA `INCUBATION DOMAIN PASS` records. Batch 3 does not depend on either unmerged incubator crate and does not inherit production integration authority from them.

## Scope

Authorised pure-domain work:

- stable inventory item and location identities;
- one explicit tracked/base unit per item in this incubation;
- immutable append-only/audit-oriented stock movements;
- derived on-hand balances by item/location and across locations;
- movement reasons covering opening, purchase receipt, sale/consume, returns, adjustments, stocktake, damage/write-off and transfer in/out;
- explicit paired stock transfers that conserve quantity across locations;
- stocktake observation -> proposed append-only adjustment, never silent balance overwrite;
- duplicate movement identity rejection;
- explicit negative derived balances rather than clamping or inventing stock;
- deterministic ordering and contribution traceability.

## Explicit exclusions

Not authorised in Batch 3:

- SBC-8F2 valuation/COGS;
- FIFO, moving average or any inventory valuation method;
- inventory-account posting;
- money/cost/price accounting semantics;
- manufacturing/BOM/work orders;
- serial/batch/lot tracking;
- reservations/allocations;
- implicit unit conversion;
- barcode/QR runtime admission;
- persistence/SQLite/Beankeeper;
- product/workspace/Tauri/Vue/native changes;
- network/provider APIs;
- changes to active SBC-7 surfaces;
- any third-party runtime dependency.

Inventory Lite follows the frozen V2 direction: InvenTree is the primary behavioural reference and Grocy the secondary reference; they are test/workflow oracles, not runtime or close-port dependencies.

## Integration boundary

Batch 3 may produce neutral stock movement and stocktake/transfer domain facts only. It cannot receive a purchase order directly from Batch 1, post accounting, mutate production persistence, or expose Tauri/UI APIs. Those typed seams are re-adjudicated later in SBC-8P after SBC-7 closes.

## Candidate rule

A development head is not an incubation PASS candidate until deep source review, exact changed-path static validation and executable standalone Rust proof are clean. Any source or harness movement after candidate freeze creates a new candidate.

## Production rule

PRODUCTION INTEGRATION BLOCKED.

This branch must remain Draft/unmerged while SBC-7 remains open. Apple/native proof is deferred because this zero-dependency crate remains outside the product/native graph. Formal SBC-8P later re-adjudicates all shared types, persistence, accounting and platform integration against then-current protected `main`.