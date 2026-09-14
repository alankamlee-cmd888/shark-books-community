# SBC-8 Batch 3 — Inventory Lite Implementation Design

Date: 2026-09-14
Status: FROZEN FOR ISOLATED INCUBATION

## Package

Standalone zero-dependency Rust crate:

`incubator/sbc8-inventory-lite-core/`

`#![forbid(unsafe_code)]`.

Modules:

- `primitives.rs` — bounded IDs/text, positive quantity and checked signed balance arithmetic;
- `inventory.rs` — item/location identities, immutable stock movements, append-only correction adjustments and deterministic balance projections;
- `transfer.rs` — sealed paired stock transfer proposals and pair validation;
- `stocktake.rs` — stocktake observations and append-only adjustment proposals;
- `lib.rs` — exports plus representative Inventory Lite journey.

## Independence

Batch 3 does not depend on the unmerged Batch 1 or Batch 2 crates. Purchase-receipt and sale/consume movements carry neutral source facts only. Later SBC-8P reconciles those seams with final purchasing/commercial/accounting types.

## Quantity model

Individual movements carry positive integer quantity. Movement kind determines sign. Derived balances use checked `i128` arithmetic so the engine can represent negative stock explicitly without narrowing or silent clamp.

Each item has one tracked unit identity during this incubation. A movement whose unit does not match the item fails closed. No automatic conversion table or floating-point conversion is present.

## Append/audit design

Stock movements expose read-only accessors and no mutation API. On-hand is always recomputed from supplied movements. Duplicate movement IDs fail before projection.

Corrections are additional adjustment movements with an explicit `corrects_movement_id`. The original movement remains unchanged. Transfers and stocktakes have separately typed proposal APIs because those workflows require stronger pair/evidence invariants.

## Transfer design

A transfer proposal creates two immutable movement facts with a shared transfer ID: TransferOut from the source and TransferIn to the destination. Source and destination must differ. Pair validation proves item/unit/quantity/transfer/time/source equality and complementary directions.

## Stocktake design

A stocktake observation is evidence, not an overwrite command. The engine derives current stock, compares it with the observed count and, only when different, proposes one StocktakeIncrease or StocktakeDecrease movement linked to the observation identity.

## Inventory Lite boundaries

The crate deliberately contains no price, cost, money, valuation, FIFO/moving-average, COGS or accounting-posting model. Negative on-hand is observable operational state, not a valuation-policy decision. Manufacturing/BOM, serial/batch/lot tracking and reservations are also excluded.

## Static proof design

`scripts/check_sbc8_batch3_inventory_lite_domain.py` enforces:

- exact changed-path allow-list relative to entry protected main;
- active SBC-7/product/workspace/native surfaces unchanged;
- zero dependencies and standalone lock;
- forbidden persistence/network/accounting/valuation/runtime markers absent;
- no float arithmetic;
- sealed invariant-bearing structs;
- required domain anchors and focused regression names;
- clean repository before/after runtime gate;
- exact expected HEAD binding.

`ci/run_sbc8_batch3_inventory_lite_windows.ps1` requires an exact `-ExpectedHead`, runs the validator and Rust 1.98.1 tests, records exact HEAD/status/path/hash/toolchain evidence, packages the evidence ZIP and preserves a second byte-identical copy on the configured Windows Desktop.

## Candidate rule

No PASS is inferred from source review. One exact SHA is frozen only after final review; executable evidence at that same SHA is then independently adjudicated. Any later source/harness change creates a new candidate.