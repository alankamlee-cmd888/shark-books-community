# SBC-2–5 Stage F — Combined Reconciliation Contract

**Date:** 9 September 2026  
**Batch:** SBC-2–5 bounded batch  
**Stage:** F — combined reconciliation  
**Entry:** protected SBC-5 PASS `main` `1f27e3a5497f6dd6e6046d1bc2efcc7b2e0a7743`

## Purpose

Stage F closes the coordinated SBC-2–5 bounded batch only after re-running the complete product-core test suite and proving that the separately validated domain, import, matching/reconciliation and document contracts compose coherently without changing the frozen accounting foundation.

This stage adds executable evidence only. It does not add a new user-facing feature or persistence implementation.

## Required end-to-end contract

The combined executable fixture must prove this chain:

`domain object -> source/import provenance -> bank line -> deterministic posting plan / frozen facade boundary -> match + user confirmation -> statement reconciliation -> document reference`

The fixture must demonstrate, in one coherent scenario:

1. a CSV source becomes a canonical preview `BankLine` with exact integer pence and immutable source provenance;
2. the bank-line provenance can feed a Shark-owned sole-trader domain object;
3. the domain object creates a balanced deterministic posting plan, while actual persistence remains behind the already-frozen Shark facade;
4. the bank line can be evaluated against a ledger candidate using deterministic matching;
5. the match remains user-confirmed and moves only `UNCLEARED -> CLEARED`;
6. statement reconciliation is separate and can finalize only at exact zero-pence difference, producing `CLEARED -> RECONCILED`;
7. a `DocumentReference` can attach integrity-protected evidence to the same bookkeeping record under a user-controlled storage root;
8. document verification produces a bounded read request using root identity + relative path + expected hash/length rather than arbitrary filesystem access.

## Frozen boundaries

Stage F may not change:

- `product/shark-books-core/Cargo.toml`;
- `product/shark-books-core/Cargo.lock`;
- any production SBC-2/SBC-3/SBC-4/SBC-5 source module;
- any native `workspace/` file;
- Beankeeper source/pin;
- the production Shark facade;
- Tauri configuration/capabilities;
- any dependency version.

The product core remains zero-third-party-dependency.

## Exact bounded source set

Relative to SBC-5 PASS `1f27e3a5497f6dd6e6046d1bc2efcc7b2e0a7743`, the Stage F candidate may change exactly:

1. `ci/run_sbc2_5_stagef_windows.ps1`
2. `docs/SBC2_5_STAGE_F_COMBINED_RECONCILIATION_CONTRACT_2026-09-09.md`
3. `product/shark-books-core/tests/sbc2_5_end_to_end.rs`
4. `scripts/check_sbc2_5_stagef.py`

## Required runtime proof

The committed Windows validator must prove:

- clean repository at entry;
- exact Stage F base ancestry and exact four-path diff;
- product package identity and zero external dependencies;
- exact product Cargo.lock SHA-256 `f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6`;
- exact frozen native Cargo.lock SHA-256 `3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`;
- combined entry point exposes domain, bank import, matching/reconciliation and documents modules;
- the end-to-end fixture contains every required stage and no direct raw database/platform/network implementation;
- frozen foundation guard PASS;
- permanent SBC-1G change-control guard PASS with no dependency/native-Apple change;
- Rust `1.98.1`;
- the complete current product-core suite, including the new Stage F integration test, passes under `--locked`;
- both lockfiles remain unchanged;
- repository remains clean after proof.

## Expected test count

The existing SBC-5 merge contains **74 unit tests**. Stage F adds **1 integration test**, so a successful full run should execute **75 product-core tests in total** (74 unit + 1 integration), plus zero doc-test failures.

## Hard stop

The SBC-2–5 bounded batch is **not closed** until the Stage F Windows proof passes and is independently adjudicated.

If Stage F passes, merge the evidence-only PR through protected `main`, update the authoritative Library state, mark SBC-2 through SBC-5 and the bounded batch closed, and hand over to the next roadmap gate. No later roadmap feature is authorised inside this Stage F closure.
