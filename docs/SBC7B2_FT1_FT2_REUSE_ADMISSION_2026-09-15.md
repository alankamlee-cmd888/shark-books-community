# SBC-7B2 FT1/FT2 — Reuse Admission Record

**Date:** 15 September 2026  
**Stage:** Phase B — bounded schema/type generation  
**Status:** **LOCK RESOLUTION PASS / GENERATOR REPAIR APPLIED / GENERATED OUTPUT PROOF PENDING**

## Decision summary

### Schemars 1.2.2 — ADOPT

- Exact crate: `schemars = 1.2.2`
- Published: 27 July 2026.
- Licence: MIT.
- Declared MSRV: Rust 1.74.
- Project proof toolchain: Rust 1.98.1.
- Use: JSON Schema generation for the Shark-owned Action Registry contract.
- Runtime boundary: optional dependency behind `action-contract-gen`; disabled by default.

### Typeshare 1.0.5 — REJECT FOR GENERATOR ROLE

The frozen plan named `Typeshare 1.0.5` for the ten-type TypeScript/Swift fidelity smoke. Exact package review found that `typeshare 1.0.5` is the annotation/library crate. The language generator is a separate `typeshare-cli` package, whose published version line does not contain `1.0.5`.

Using a newer `typeshare-cli` would silently change the frozen dependency identity, so it is not admitted in this batch.

This counts as the bounded Typeshare qualification failure contemplated by the frozen fallback rule. No prolonged generator bake-off is authorised.

### ts-rs 12.0.1 — ADOPT AS PRE-AUTHORISED FALLBACK

- Exact crate: `ts-rs = 12.0.1`.
- Published: 31 January 2026.
- Licence: MIT.
- Declared MSRV: Rust 1.88.
- Project proof toolchain: Rust 1.98.1.
- Use: Rust -> TypeScript contract generation.
- Runtime boundary: optional dependency behind `action-contract-gen`; disabled by default.
- Swift generation: deferred to the later Apple AppIntent/native-voice work as explicitly permitted by the frozen fallback.

## Locked dependency resolution evidence

Windows development proof resolved the exact feature-enabled dependency graph under Rust 1.98.1 and `--locked`:

- `schemars v1.2.2`;
- `schemars_derive v1.2.2`;
- `ts-rs v12.0.1`;
- `ts-rs-macros v12.0.1`.

The existing Action System contract regression simultaneously remained **16/16 PASS, 0 failed**.

The locally materialised `workspace/Cargo.lock` is not yet authoritative repository evidence. Its exact bytes must be captured and committed before candidate freeze.

## First generator execution — bounded API mismatch

The first `action-contract --write` and `--check` executions failed before writing generated artifacts because the tooling source used the older zero-argument `TS::decl()` API.

The exact admitted `ts-rs 12.0.1` API requires:

`TS::decl(&Config)`

All observed compile errors were the same E0061 call-shape mismatch. No generated JSON Schema or TypeScript artifact was produced by that failed run.

This was classified as a tooling API-integration defect only. It did not alter or invalidate:

- the Action Registry;
- the 206/175/700 invariants;
- controller semantics;
- confirmation/replay/Attention behaviour;
- accounting logic;
- the admitted dependency identities.

## Minimal repair

The generator was repaired only in `tools/action-contract/action_contract.rs`.

The repair:

- creates one `ts_rs::Config::default()`;
- passes `&Config` to all ten fidelity-smoke `TS::decl` calls;
- passes the same fixed config to all production Action Registry TypeScript declarations;
- deliberately does **not** use `Config::from_env()`, so `TS_RS_*` environment variables cannot silently change generated contract bytes.

No dependency version, Action ID, registry row, controller rule, Tauri command, persistence path or accounting behaviour changed.

## Admission smoke

The generator target contains exactly ten tooling-only fidelity types covering:

1. scalar strings;
2. `Option<T>`;
3. `Vec<T>`;
4. `BTreeMap<String, String>`;
5. nested structs;
6. serde-renamed enums;
7. enum-containing structs;
8. bool + integer fields;
9. vectors of nested types;
10. combined optional/nested/enum structures.

The same target generates the Action Registry JSON Schema and TypeScript declarations from the actual Rust registry types. It does not own accounting semantics or execution authority. Controller/execution DTO generation is deliberately not expanded in this admission slice; the bounded generator decision is proved first and broader generated DTO coverage can follow under the same Action System authority.

## Exact pinning

Phase B pins exact versions rather than semver ranges:

- `schemars = "=1.2.2"`
- `ts-rs = "=12.0.1"`

The final admission remains pending until:

1. the repaired ten-type smoke executes successfully;
2. JSON Schema and TypeScript files are generated;
3. an immediate `--locked --check` reproduces exactly the same bytes;
4. the locally generated `workspace/Cargo.lock` exact bytes are committed;
5. the generated files and lock are independently inspected;
6. the integrated static gate passes at the eventual candidate SHA.

## Boundary

No runtime Action System call path enables `action-contract-gen`. No Tauri command, persistence surface, accounting mutation, SBC8 production activation, model call or speech runtime is added here.

Formal interim state:

`SCHEMARS_1_2_2_LOCK_RESOLUTION_PASS`  
`TYPESHARE_1_0_5_GENERATOR_ROLE_REJECTED`  
`TS_RS_12_0_1_FALLBACK_LOCK_RESOLUTION_PASS`  
`TS_RS_12_API_MISMATCH_REPAIRED`  
`GENERATED_OUTPUT_PROOF_PENDING`  
`SWIFT_GENERATION_DEFERRED_TO_APPINTENT_STAGE`  
`CANDIDATE_NOT_FROZEN`
