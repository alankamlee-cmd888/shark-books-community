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

The primary permanent enforcement is stored in root `codemagic.yaml` as two workflows:

- `sbc-foundation-guard-macos` — Mac/Apple regression and target proof;
- `sbc-foundation-guard-windows` — Windows production-shell regression/build proof.

Both are configured for `pull_request` and `push` events targeting `main`, with outdated webhook builds cancelled. Codemagic automatic execution requires the repository webhook to be active in the Codemagic app connection. Manual execution remains possible as an evidence path if the webhook must first be refreshed.

Every normal Rust build/test/metadata command in the permanent guard harnesses uses `--locked`.

The permanent guard performs:

1. exact SBC-1F ancestry and bounded-change checking;
2. the existing frozen-baseline guard plus explicit critical-package checks;
3. the permanent Shark foundation regression scenario and SBC-1F browser contract smoke;
4. a locked Windows production Tauri shell build;
5. physical `aarch64-apple-ios` and `aarch64-apple-ios-sim` foundation/Tauri compile proof on Mac;
6. unchanged native Cargo.lock verification before and after proof.

The Apple job uses a temporary CI-only RGB→RGBA icon conversion and restores/abandons it with the disposable checkout. It does not change committed product assets.

## GitHub Actions fallback and repository setting limitation

`.github/workflows/sbc-foundation-guard.yml` is retained as a **manual fallback**, not the primary automatic enforcement path.

During SBC-1G preparation, two GitHub Actions pull-request runs were created but the classifier job terminated before any recorded job steps/runner execution; downstream jobs were skipped. Removing the marketplace checkout action did not change that runner-startup behaviour. This was therefore treated as CI-platform execution evidence rather than product/foundation evidence.

The connected GitHub integration also returned `403 Resource not accessible by integration` for the `main` branch-protection endpoint. Therefore this programme does not claim that GitHub branch protection or required GitHub status checks were configured through the connector. Repository-level protection may be enabled later if account/API permissions permit; the source-controlled Codemagic guards remain the primary enforcement mechanism.

## Permanent regression scenario

`workspace/shark-foundation/tests/foundation_regression.rs` is a Shark-owned regression fixture covering the behaviours that must not silently regress:

- GBP/pence exactness and golden trial-balance totals;
- validation-before-persistence by rejection of an unbalanced posting;
- encrypted create, wrong-key fail-closed and hard reopen;
- duplicate-safe GBP OFX/FITID import;
- correction by separate inverse posting while retaining the original;
- reconciliation audit actor/before/after evidence;
- document attachment hash/reference persistence after reopen.

The deterministic fixtures live under `workspace/shark-foundation/tests/fixtures/`.

## Dependency-change rule

A change to the native lock/manifests, Beankeeper bootstrap pin or frozen foundation manifest is never an ordinary feature change.

Such a change must include, in the same reviewed change set, a `docs/dependency-reviews/` evidence bundle containing at minimum:

- a regenerated `*SBOM*.csv`;
- a regenerated `*LICENSE*.csv` register;
- an `*ADJUDICATION*.md` record.

The existing frozen-baseline guard will also fail until the explicit foundation review updates the approved hashes/versions. This is deliberate: changing review documents alone is not permission to drift the foundation.

Changes to Beankeeper, rusqlite/SQLCipher/OpenSSL, Tauri, Rust toolchain/MSRV or licence class must follow the frozen SBC-0E update policy and rerun the targeted Windows/Apple/facade regressions before acceptance.

## Browser boundary

The separate SBC-1F browser smoke crate remains outside the native Cargo workspace and makes no browser parity claim. Browser SQLite WASM/OPFS durability, encryption and parity remain SBC-17 work.
