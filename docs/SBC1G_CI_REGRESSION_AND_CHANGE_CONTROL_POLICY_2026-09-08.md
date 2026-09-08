# Shark Books Community — SBC-1G CI, Regression and Change-Control Policy

**Date:** 8 September 2026  
**Scope:** permanent frozen-foundation protection after SBC-1F  
**Baseline HEAD:** `4e3a47b70933cfafe27be262ac2da1ab2a63d1d9`

## Frozen foundation

The following remain controlled foundation identifiers unless an explicit foundation-change review is opened:

- Beankeeper `d573db5e61089b0922f95c991732394d08e3cf92`;
- production Shark facade SHA-256 `6274cffc89ccb2fcea4d489e76375c9c5be1e3c8cc2fbd4c4e56b37bdef0ef46`;
- native `workspace/Cargo.lock` SHA-256 `3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`;
- Rust proof baseline `1.98.1`;
- Tauri core / CLI / build `2.11.5 / 2.11.4 / 2.6.3`;
- SQLCipher mandatory for production native books;
- Shark facade validation before persistence and no raw Beankeeper/rusqlite exposure to UI/import/AI.

## Permanent CI rule

`.github/workflows/sbc-foundation-guard.yml` runs on pull requests and pushes to `main`.

Every ordinary Rust build/test/metadata command in this workflow uses `--locked`.

The workflow has four functions:

1. **change classification** — determines whether the candidate changes the native Apple graph or a controlled dependency surface;
2. **foundation/regression guard** — runs the frozen-baseline guard, explicit critical-package checks, permanent Shark foundation regressions and browser contract smoke;
3. **Windows compile** — builds the production Tauri shell on `windows-latest` under the frozen lock;
4. **Apple native compile** — runs only when native/foundation paths change, and compiles the shared foundation and Tauri shell for both `aarch64-apple-ios` and `aarch64-apple-ios-sim` after a temporary CI-only RGB→RGBA icon conversion.

The Apple job does not alter committed product assets. It operates only in the disposable CI checkout.

## Permanent regression scenario

`workspace/shark-foundation/tests/foundation_regression.rs` is a Shark-owned regression fixture covering the behaviours that must not silently regress:

- GBP/pence exactness and golden trial-balance totals;
- validation-before-persistence by rejection of an unbalanced posting;
- encrypted create, wrong-key fail-closed and hard reopen;
- duplicate-safe GBP OFX/FITID import;
- correction by separate inverse posting while retaining the original;
- reconciliation audit actor/before/after evidence;
- document attachment hash/reference persistence after reopen.

The fixtures are deterministic and live under `workspace/shark-foundation/tests/fixtures/`.

## Dependency-change rule

A change to the native lock/manifests, Beankeeper bootstrap pin or frozen foundation manifest is never an ordinary feature change.

Such a change must include, in the same reviewed change set, a `docs/dependency-reviews/` evidence bundle containing at minimum:

- a regenerated `*SBOM*.csv`;
- a regenerated `*LICENSE*.csv` register;
- an `*ADJUDICATION*.md` record.

The existing frozen-baseline guard will also fail until the explicit foundation review updates the approved hashes/versions. This is deliberate: changing the review documents alone is not permission to drift the foundation.

Changes to Beankeeper, rusqlite/SQLCipher/OpenSSL, Tauri, Rust toolchain/MSRV or licence class must follow the frozen SBC-0E update policy and rerun the targeted Windows/Apple/facade regressions before acceptance.

## Browser boundary

The separate SBC-1F browser smoke crate remains outside the native Cargo workspace and makes no browser parity claim. Browser SQLite WASM/OPFS durability, encryption and parity remain SBC-17 work.

## Repository-enforcement note

CI source controls are stored in the repository. Required-status/branch-protection settings should be enabled at repository level where account/plan/API permissions permit. If repository settings cannot be mutated by the connected automation, that limitation must be recorded during SBC-1H rather than silently claimed as configured.
