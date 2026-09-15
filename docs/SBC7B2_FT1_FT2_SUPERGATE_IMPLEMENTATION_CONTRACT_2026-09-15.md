# SBC-7B2 FT1 + FT2 — First Super-Gate Implementation Contract

**Date:** 15 September 2026  
**Programme:** Shark Books Community  
**Stage:** SBC-7B2 fast-track  
**Substages:** FT1 Action Stack + FT2 Deterministic Controller / Attention Queue  
**Protected base:** `99c3b0d16fe4934f1b397a58956df745203f744f`  
**Branch:** `sbc7b2-supergate-fasttrack-20260915`  
**Status:** **IMPLEMENTATION CONTRACT FROZEN / CANDIDATE NOT YET FROZEN**

## 1. Objective

Implement the first post-7B1 Shark-owned Action System without changing accounting semantics.

The batch must establish:

1. one canonical machine Action Registry reconciled to the 206 canonical actions in authority 168;
2. exactly 175 canonical voice-eligible actions from authority 171A;
3. the five non-canonical convenience aliases as recipes that canonicalise to real Action IDs;
4. at least four representative utterance fixtures for each voice-eligible action (minimum 700 fixtures);
5. one deterministic resolution/controller model with `KNOWN`, `UNKNOWN`, `AMBIGUOUS`, `CONFLICTING`;
6. explicit backend/exposure locking so future/SBC8 actions remain non-executable;
7. required-slot clarification metadata;
8. confirmation-class enforcement that invocation surfaces cannot lower;
9. stale-confirmation and replay protection;
10. deterministic Attention Items for unresolved/locked states.

No assistant model, speech runtime, accounting mutation dispatcher, UI binding or SBC8 production activation is part of this batch.

## 2. Authority

This contract implements the planning decisions in Library authorities 166–180, especially:

- 168/168A/168B/168C — master capability/action registry;
- 171/171A — fast-track reuse and 175-action voice parity;
- 173/173A — same-SHA Super-Gate batching policy;
- 179 — SBC-7B1 closure;
- 180 — SBC-7B2 Super-Gate activation.

Authority 168 remains semantic planning provenance. This batch may update implementation/evidence state where later merged evidence supersedes the 15 September planning snapshot, but it may not silently redefine an Action ID.

## 3. Canonical-count invariants

The implementation must prove:

- canonical actions: **206**;
- convenience aliases: **5**;
- voice-eligible canonical actions: **175**;
- `REQUIRED_WHEN_EXPOSED`: **32**;
- `REQUIRED_WHEN_ACTIVATED`: **143**;
- canonical actions outside voice target: **31**;
- minimum utterance fixtures: **700**.

The five convenience aliases are not separate voice authority:

- `MONEY_IN.DAILY_TAKINGS` -> `MONEY_IN.SAVE`;
- `MONEY_OUT.MIXED_USE` -> `MONEY_OUT.SAVE`;
- `MONEY_OUT.PRIVATE_FROM_BUSINESS_FUNDS` -> `MONEY_OUT.SAVE`;
- `CONTACT.CREATE_CUSTOMER` -> `CONTACTS.SAVE`;
- `CONTACT.CREATE_SUPPLIER` -> `CONTACTS.SAVE`.

## 4. Current-state overlay after SBC-7B1

The following 168/171A rows were development-only when those planning artifacts were produced, but are now protected-main backend-ready following the independently proven and merged SBC-7B1 Batch B:

- `CONTACTS.LIST`;
- `CONTACTS.SAVE`;
- `SETTINGS.BOOKS_INFO`;
- `SETTINGS.STORAGE_ROOT.SELECT`;
- `REPORT.SUMMARY`.

The implementation registry must preserve the original planning provenance while recording their current state as `PRODUCTION_MAIN / BACKEND_READY_UI_LOCKED / SBC7B1_BATCH_B_DUAL_PLATFORM_PASS`.

## 5. Controller boundary

The deterministic controller may:

- canonicalise reviewed aliases;
- identify one or more candidate Action IDs;
- detect unknown, ambiguous and conflicting requests;
- identify missing required slots;
- distinguish owner facts from system/context facts;
- expose an execution-availability result;
- require the ActionSpec confirmation class;
- create/validate stale-state confirmation receipts;
- reject duplicate operation IDs;
- create deterministic Attention Items.

It may **not**:

- execute accounting mutations itself;
- invent an Action ID;
- route internal/prohibited actions into owner execution;
- activate `PRODUCTION_LOCKED`, `FUTURE_NOT_AVAILABLE`, `NOT_AUTHORISED` or internal actions;
- treat a backend-ready action as user-exposed unless the caller explicitly supplies the reviewed exposure set;
- lower confirmation because the source was voice/text/AI;
- perform tax, VAT, CIS, payroll, HMRC or Open Banking judgement.

## 6. Dependency/reuse decision

### Schemars
Target: **Schemars 1.2.2**, MIT, for Rust -> JSON Schema generation.  
Status at contract freeze: **ADOPTION DECIDED; exact project admission/evidence still required before candidate freeze**.

### Typeshare
Target: **Typeshare 1.0.5**, MIT OR Apache-2.0, for the already-authorised 10-type fidelity smoke and then TypeScript + Swift generation if it passes.  
Status at contract freeze: **PREFERRED / ONE SHORT FIDELITY SMOKE ONLY**.

If that smoke fails, the only authorised fallback is **ts-rs 12.0.1 (MIT)** for TypeScript; Swift generation may then be deferred to AppIntent work. No prolonged generator bake-off is authorised.

Tooling dependencies should be isolated from the bookkeeping runtime where practical. Exact versions, licences, transitives and lock effects must be recorded in the reuse-admission record before the final candidate freezes.

## 7. Exact changed-path envelope

No final Super-Gate candidate may contain a changed path outside this list without a formal contract amendment:

1. `docs/SBC7B2_FT1_FT2_SUPERGATE_IMPLEMENTATION_CONTRACT_2026-09-15.md`
2. `docs/SBC7B2_ACTION_SYSTEM_DESIGN_2026-09-15.md`
3. `docs/SBC7B2_FT1_FT2_REUSE_ADMISSION_2026-09-15.md`
4. `workspace/shark-foundation/src/action_system.rs`
5. `workspace/shark-foundation/src/lib.rs`
6. `workspace/shark-foundation/data/action_registry_v1.json`
7. `workspace/shark-foundation/data/voice_utterance_fixtures_v1.json`
8. `workspace/shark-foundation/Cargo.toml`
9. `workspace/Cargo.lock`
10. `tools/action-contract/action_contract.rs`
11. `contracts/action-system/v1/action_contract.schema.json`
12. `contracts/action-system/v1/action_contract.ts`
13. `contracts/action-system/v1/action_contract.swift`
14. `scripts/check_sbc7b2_ft1_ft2.py`
15. `ci/run_sbc7b2_supergate_windows.ps1`
16. `ci/run_sbc7b2_supergate_codemagic.sh`
17. `codemagic.yaml`

A path in this allow-list does not have to change. It is permission, not a requirement.

## 8. Staged implementation inside this one contract

### Phase A — dependency-free Action System core
Use already-admitted `serde`, `serde_json` and `sha2` to land:

- canonical registry data;
- 175-action voice metadata;
- 700+ fixtures;
- Rust ActionSpec/controller/Attention structures;
- static validator and Rust unit tests.

This phase must not change Cargo dependencies.

### Phase B — bounded schema/type-generator admission
Run the Schemars exact-version admission and the one Typeshare fidelity smoke. Only after the smoke outcome is known may the generated schema/TS/Swift outputs be frozen.

### Phase C — Super-Gate harness
Create the Windows and Codemagic runners under 173A. They may share one setup/cache session but must emit separately attributable SG0–SG8 evidence.

## 9. Proof policy

During implementation, run cheap static/unit checks freely.

Do **not** spend a separate Codemagic session after Phase A or Phase B merely because an internal slice completed. Freeze one substantial immutable candidate first.

Before merge, mandatory evidence is:

1. changed-path reconciliation;
2. registry/count/voice fixture static PASS;
3. Action System Rust tests PASS;
4. reuse/dependency admission PASS;
5. inherited affected regressions PASS;
6. Windows Super-Gate PASS;
7. same-SHA Apple Super-Gate PASS if native/dependency scope remains affected;
8. final independent reconciliation;
9. expected-head protected merge.

## 10. Formal state

`SBC7B2_FT1_FT2_CONTRACT_FROZEN`  
`PHASE_A_IMPLEMENTATION_AUTHORISED`  
`NO_CODEMAGIC_BUILD_REQUIRED_YET`  
`CANDIDATE_NOT_FROZEN`
