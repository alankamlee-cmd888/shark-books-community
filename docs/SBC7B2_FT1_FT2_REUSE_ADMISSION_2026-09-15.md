# SBC-7B2 FT1/FT2 — Reuse Admission Record

**Date:** 15 September 2026  
**Stage:** Phase B — bounded schema/type generation  
**Status:** PRE-CANDIDATE / LOCKFILE + GENERATED OUTPUT PROOF PENDING

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

The same target generates the Action Registry JSON Schema and TypeScript declarations from the actual Rust registry types. It does not own accounting semantics or execution authority. Controller/execution DTO generation is deliberately not expanded in this admission slice; the bounded generator decision is proven first and broader generated DTO coverage can follow under the same Action System authority.

## Exact pinning

Phase B pins exact versions rather than semver ranges:

- `schemars = "=1.2.2"`
- `ts-rs = "=12.0.1"`

The final admission remains pending until Cargo resolves these pins into `workspace/Cargo.lock`, the ten-type smoke executes under Rust 1.98.1, generated files are frozen, and `--locked` re-check succeeds.

## Boundary

No runtime Action System call path enables `action-contract-gen`. No Tauri command, persistence surface, accounting mutation, SBC8 production activation, model call or speech runtime is added here.

Formal interim state:

`SCHEMARS_1_2_2_ADOPTED_PENDING_LOCK_PROOF`  
`TYPESHARE_1_0_5_GENERATOR_ROLE_REJECTED`  
`TS_RS_12_0_1_FALLBACK_ACTIVATED_PENDING_LOCK_PROOF`  
`SWIFT_GENERATION_DEFERRED_TO_APPINTENT_STAGE`  
`CANDIDATE_NOT_FROZEN`
