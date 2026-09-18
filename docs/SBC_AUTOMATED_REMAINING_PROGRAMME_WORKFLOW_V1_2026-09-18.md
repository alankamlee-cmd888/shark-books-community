# Shark Books Community — Automated Remaining Programme Workflow V1

**Date:** 18 September 2026  
**Status:** FROZEN / READY FOR IMPLEMENTATION  
**Purpose:** Replace repetitive owner command transport and routine technical approvals with bounded, evidence-driven automation while preserving the frozen SharkBooks/MTD Shark scope, privacy-first architecture, cost controls, architecture controls, GitHub development publication, and the hard public-launch lock.

## 1. Current entry state

- Repository: `alankamlee-cmd888/shark-books-community`
- Active PR: `#29 — SBC-7B2 FT3/FT4: owner UI + finite command surface`
- Protected base: `main` at `ebf1ae9d0e10f0f427625d4c3ab5c1703299d154`
- Active branch: `sbc7b2-ft3-ft4-integrated-20260916`
- Current exact head: `1ad86cc7715aa38a8162952f02c8debe3d743736`
- PR state at freeze: OPEN / DRAFT / UNMERGED / MERGEABLE.

The current R2/UI-B gate remains governed by the already-authorised audited V3 runner. This workflow does **not** replace R2, does not redesign it, and must not change PR #29 head before the R2 execution/adjudication boundary is cleared.

After formal R2 PASS, this document becomes the default automation authority for the remaining authorised development sequence.

## 2. Normal automated flow

`authority -> bounded task -> SLE isolated execution -> executor validation -> evidence publication -> event-driven completion relay -> independent adjudication -> exact reviewed GitHub publication -> CI/proof -> next authorised bounded task`

The owner is not used as a clipboard or routine technical gate.

Routine phrases such as `done`, `check the agent result`, `proceed with the next test`, `run Windows`, `run Apple`, `freeze candidate`, or `merge the proved candidate` are removed from the normal flow.

## 3. Standing owner authority

### O1 — Product scope hard lock

The workflow must remain strictly inside the current frozen SharkBooks/MTD Shark roadmap and product scope.

Deliberately excluded capabilities remain excluded from this build, including VAT accounting/filing, CIS, payroll, company/partnership accounting, direct HMRC submission, Open Banking/live feeds, autonomous AI accounting/tax judgement, and any other capability explicitly excluded by current authority.

If completion would require scope expansion: **STOP — OWNER_EXCEPTION_SCOPE**.

### O2 — Free-only automatic cost authority

Free, local, and open-source components may proceed automatically where they pass existing licence, privacy, security, and technical gates.

Any new monetary cost, subscription, paid API, usage charge, commercial licence, or unavoidable new paid service requires explicit owner authority.

Trigger: **OWNER_EXCEPTION_COST**.

### O3 — Privacy/security equivalence only

Low-risk implementation changes may proceed automatically only when they preserve the existing local-first/privacy-first architecture and do not expand user-data exposure or technical authority.

Any material expansion of off-device transfer, third-party processing, network authority, credential handling, filesystem authority, shell/process authority, central Shark custody, or remote/cloud processing requires owner authority.

Trigger: **OWNER_EXCEPTION_PRIVACY_SECURITY**.

A full privacy/data transparency audit is mandatory at every relevant stage.

### O4 — Equivalent implementation substitution only

Equivalent technical substitutions may proceed automatically only where they preserve product behaviour, privacy model, security boundaries, licensing position, cost profile, frozen dependency policy, product scope, and material user experience.

Any material architecture, data-flow, authority-boundary, dependency-policy, schema-contract, or user-behaviour change requires owner authority.

Trigger: **OWNER_EXCEPTION_ARCHITECTURE**.

### O5 — GitHub development authorised; public launch withheld

The workflow is authorised to:
- build the real product;
- create/update governed development branches;
- create exact reviewed commits;
- push reviewed commits;
- create/update PRs;
- run CI/proof workflows;
- automatically merge exact proved candidates where all frozen merge conditions pass;
- build and update non-public/protected preview or staging environments;
- prepare the real homepage/application so it is fully launch-ready.

However:

**`mtdshark.co.uk` must continue to display the holding page.**

No automation may remove, bypass, replace, redirect, or otherwise defeat that holding-page boundary. Technical readiness, merge success, release-candidate status, or whole-roadmap completion is not public-launch authority.

Public go-live requires explicit contemporaneous owner authorisation clearly stating that MTD Shark may go live publicly.

Trigger for any attempted public exposure without that authority: **OWNER_EXCEPTION_PUBLIC_RELEASE**.

## 4. Development plane vs public plane

### Development plane — authorised

`GitHub branch/PR -> CI -> protected/non-public preview or staging -> real UI/application inspection`

### Public plane — hard locked

`mtdshark.co.uk -> holding page`

Any path/configuration capable of changing DNS, public domain routing, production host binding, holding-page routing, public feature exposure, or equivalent launch controls is classified **PUBLIC_RELEASE_CRITICAL**.

## 5. Shark Local Executor role

Verified SLE baseline: `v1.9.0`.

SLE is a bounded executor, not programme authority.

Permitted lanes:

1. **READ/PROOF** — no mutation.
2. **EXACT** — deterministic `repo_patch_exact`, zero model calls.
3. **SEMANTIC** — direct structured local `qwen3:4b-instruct`, tightly bounded write paths.
4. **PROMOTION/PUBLICATION PREP** — reproduce an independently reviewed patch in a fresh isolated candidate and prove exact diff/after-hash identity before GitHub publication.

The historical SBC proof profile must not be repointed to active PR #29.

A dedicated live SBC profile must be created after R2 PASS and pinned to the exact post-R2 reviewed branch/head/scope.

## 6. Initial live-SBC bounded limits

Until a later evidence-backed profile proves a safe expansion:

- one active write-capable SLE task per governed branch;
- semantic task: maximum **2 authorised write files**;
- exact deterministic task: maximum **2 authorised write files**;
- no model-selected paths;
- checker files cannot be writable by the task they validate;
- no new dependency/manifest/lock/schema authority unless already frozen by the active stage;
- exact entry HEAD and clean source required;
- all write work occurs in isolated clone/worktree/candidate;
- complete before/after hashes and diff required;
- successful executor exit is evidence only, never programme PASS.

## 7. Event-driven completion relay

The manual owner step between local execution and review is removed.

Before SLE becomes the normal R3+ executor, implement and prove an event-driven completion relay.

Minimum event metadata:

- schema/version;
- task ID;
- executor version;
- project profile key/version/hash;
- completion state;
- result/evidence SHA-256;
- evidence location/reference;
- expected source HEAD;
- timestamp;
- unique event ID/nonce.

The relay:
- contains no source code or secrets;
- has no product-write authority;
- has no commit/push/merge authority;
- has no public-release authority;
- exists only to wake the review/orchestration layer.

Preferred transport:
1. event-driven connected-service trigger capable of immediate wake;
2. independently reviewed authenticated completion endpoint/relay if supported;
3. hourly Dropbox condition-watch only as emergency fallback, not the target normal workflow.

## 8. Automatic evidence adjudication

Every write-capable task must automatically verify:

1. exact task/profile/version/hash identity;
2. exact source/entry HEAD;
3. source invariance;
4. exact changed-path envelope;
5. no untracked/out-of-envelope output;
6. correct model-call policy;
7. all required tests/checkers;
8. expected/actual post-edit hashes where applicable;
9. complete diff against frozen intent;
10. privacy/security impact;
11. dependency/schema/config impact.

Outcome classification:
- `PASS`
- `REPAIRABLE_FAIL`
- `GOVERNED_STOP`
- `OWNER_EXCEPTION`

## 9. Automatic repair policy

A technical failure does not automatically become an owner gate.

### Harness/environment defect

Automatically repair and rerun when the repair is demonstrably harness/environment-only and cannot change product semantics or weaken proof.

### Bounded implementation defect

Up to **two automatic bounded repair cycles** under the same frozen product/architecture/security contract.

After two failed repair cycles:
- perform deeper independent failure review;
- issue a new successor batch automatically if still an equivalent implementation repair;
- involve the owner only if O1–O5 fires.

Forbidden:
- waiving failed gates;
- weakening tests merely to pass;
- scope broadening;
- privacy/security authority broadening;
- introducing a cost;
- making a material architecture decision;
- public exposure.

## 10. Automatic GitHub publication

After independent PASS:

- publish the exact reviewed tree/diff to the governed development/integration branch;
- require exact reviewed diff/hash identity;
- use expected-head protection;
- never commit direct unreviewed model output;
- update PR metadata/state automatically;
- monitor CI automatically;
- pin the next task to the new exact reviewed head.

### Automatic protected merge

Merge may proceed automatically only if all are true:

1. programme stage allows merge;
2. one immutable candidate SHA is frozen;
3. required Windows proof passes on that SHA;
4. required Apple/same-SHA proof passes where applicable;
5. independent evidence reconciliation passes;
6. privacy audit passes;
7. dependency/licence/SBOM checks pass where triggered;
8. PR head still equals exact proved candidate;
9. protected base is expected/reconciled;
10. no O1–O5 exception exists;
11. merge cannot expose the public product or defeat the holding-page lock.

Otherwise fail closed.

## 11. Mandatory privacy transparency audit

Every privacy-relevant stage produces:

- privacy delta audit;
- cumulative privacy/data-handling audit;
- machine-readable data-flow inventory;
- user-facing wording impact register;
- unresolved privacy questions/risks.

The audit must explicitly cover, where relevant:

- data inventory and origin;
- device/local/user-cloud/staging storage;
- network/off-device transfer;
- third-party recipients/processors;
- credentials/secrets;
- permissions and least privilege;
- encryption/integrity;
- logs, telemetry, crash diagnostics;
- temporary files/caches/Blob previews;
- backups/recovery;
- retention/deletion;
- local AI/OCR input/output/network/training/telemetry;
- preview/staging/public separation.

User-facing documentation must plainly answer:
- what data is used;
- why;
- where it is stored;
- whether it leaves the device;
- where it goes;
- who processes it;
- retention/deletion;
- permissions;
- whether AI/OCR is involved and whether it is local/remote.

Claims such as `private`, `local`, `encrypted`, or `never leaves your device` may not be used unless current evidence supports the exact claim.

## 12. Automation-enablement batches

### AUT-0 — workflow authority freeze

**Status: COMPLETE by this document.**

### AUT-1 — event-driven SLE completion relay

**Status: READY / NOT YET IMPLEMENTED.**

Sub-batches:
- AUT-1A transport discovery + threat model + event schema;
- AUT-1B minimal deterministic notifier/relay;
- AUT-1C adversarial proof: duplicate, stale, forged, tampered, wrong-task, reordered, secret/code-leak rejection;
- AUT-1D live safe non-product completion event;
- AUT-1E automatic evidence-review wake proof;
- AUT-1F failure/retry/fallback proof;
- AUT-1G closure/activation.

Must not modify active PR #29 product source.

### AUT-2 — dedicated live SBC SLE profile

**Status: CONTRACT READY / exact SHA pending R2 PASS.**

Sub-batches:
- AUT-2A generate versioned live profile from exact post-R2 head and R3 scope;
- AUT-2B pin repo/branch/head/path/checker/model/network/release policy;
- AUT-2C offline tamper/scope-escape tests;
- AUT-2D safe live READ/PROOF;
- AUT-2E safe isolated EXACT proof;
- AUT-2F closure.

## 13. Current SBC-7 remaining bounded map

### R2 — UI-B exact frozen execution

- Entry head: `1ad86cc7715aa38a8162952f02c8debe3d743736`
- Existing audited V3 runner only.
- SLE does not replace this execution.
- Independent adjudication follows.
- PASS automatically advances to AUT-2/R3.

### R3 — finite command-text / Attention integration

Automatic objectives:
- same canonical Action Registry/controller;
- exposed/admitted actions only;
- locked actions fail closed;
- deterministic missing-slot clarification;
- canonical confirmation class authoritative;
- no speech runtime;
- no LLM accounting interpretation;
- no new capability from text route.

Bounded split:
- R3-A exact/generated preparation;
- R3-B semantic implementation tasks, max 2 files each;
- R3-C controller/Attention wiring;
- R3-D static/security/action-identity proof;
- R3-E integrated adjudication + exact reviewed GitHub publication.

### R4 — integrated reconciliation

Automatically:
- reconcile lifecycle/backend/exposure metadata;
- regenerate/check contracts;
- upgrade SG coverage as required;
- static/build/affected regressions;
- exact dependency/lock/config/path identities;
- privacy delta/cumulative audit;
- bounded repair before candidate freeze.

### R5 — immutable candidate freeze

No owner decision.

If R4 PASSes, freeze the exact integrated SHA automatically.

### R6 — Windows SG0–SG8

No owner decision.

Run automatically on exact R5 SHA.

### R7 — Apple/Codemagic same-SHA SG0–SG8

No owner decision.

Run automatically on exact Windows-proved R5 SHA.

### R8 — independent reconciliation + protected merge

No owner decision unless O1–O5 fires.

Verify same-SHA proof, all SG evidence, privacy/security/licence/dependency evidence, exact current PR head/base, and public-release separation. Then expected-head protected merge automatically.

### SBC-7C — representative owner journey / SBC-6+7 closure

Technical acceptance is automatic against frozen journeys/degraded states.

A protected staging/preview may show the real owner-facing application for observation, but technical closure does not require manual clicking through every objective acceptance item.

On PASS: automatically advance to SBC-8P.

## 14. SBC-8 production integration

### SBC-8P

Automatically open after legitimate SBC-7 closure from then-current protected main.

Tasks:
- reconcile current interfaces;
- revalidate the three incubation evidence sets;
- dependency/licence/SBOM review;
- privacy/data-flow review;
- determine exact production integration envelopes;
- reject stale assumptions rather than silently port them.

### SBC-8A..8G

Follow the already-frozen SBC-8 production plan:

`entry -> bounded contract -> implementation -> exact candidate -> Windows proof -> same-SHA Apple proof when applicable -> independent reconciliation -> reviewed protected merge -> Library checkpoint`

SLE task limits and O1–O5 apply.

## 15. SBC-9 through SBC-18 automatic stage generator

The roadmap already defines:
- SBC-9 deterministic UK rules;
- SBC-10 local AI/action layer;
- SBC-11 local speech;
- SBC-12 MTD digital records/export;
- SBC-13 filing-route proofs;
- SBC-14 backup/recovery/security;
- SBC-15 Windows release;
- SBC-16 iPhone/iPad release;
- SBC-17 browser-local PWA;
- SBC-18 final hardening / 1.0.

These cannot honestly have exact future file-level batches frozen today because their entry SHAs, SDK versions, provider facts, and repository state are future-dependent.

Instead, each stage is automatically instantiated using:

- **P0 Entry reconciliation** — exact main SHA, prior closure, current external facts, exclusions, privacy/cost check.
- **P1 Contract freeze** — exact bounded implementation contract; no code.
- **P2 Bounded implementation** — automatic SLE decomposition.
- **P3 Integrated reconciliation** — security/privacy/dependency/static/build/test proof.
- **P4 Immutable candidate freeze**.
- **P5 Windows proof**.
- **P6 Apple/same-SHA proof when applicable**.
- **P7 Independent reconciliation + expected-head protected merge**.
- **P8 Closure + current-state advancement**.

No owner prompt between P0–P8 unless O1–O5 fires.

## 16. Preview/UI requirement

The development workflow should expose the real homepage/application through a protected or authenticated staging/preview environment.

The preview must:
- build from reviewed GitHub development state;
- show the real homepage/navigation/application;
- remain separate from the public holding page;
- not assume production user data;
- not make `mtdshark.co.uk` public.

Owner design feedback may create new bounded tasks, but preview availability is not public-launch authority.

## 17. Immediate execution order

1. Preserve active PR #29 exact head for R2.
2. Execute and independently adjudicate R2 under existing V3 authority.
3. AUT-1 completion-relay work may be prepared separately only if it does not touch PR #29 product source.
4. AUT-1 must PASS before first normal automated R3 SLE write.
5. Create/prove AUT-2 live SBC profile pinned to exact post-R2 head.
6. Run R3 through the automatic bounded workflow.
7. Continue automatically through R4–R8 and SBC-7C.
8. Automatically enter SBC-8P then 8A–8G.
9. Instantiate SBC-9–18 through the standard stage generator.
10. Maintain privacy audits and protected preview throughout.
11. Keep `mtdshark.co.uk` on the holding page until explicit owner public go-live authority.

## 18. Only owner exceptions

The workflow may interrupt the owner only for:

- `OWNER_EXCEPTION_SCOPE`
- `OWNER_EXCEPTION_COST`
- `OWNER_EXCEPTION_PRIVACY_SECURITY`
- `OWNER_EXCEPTION_ARCHITECTURE`
- `OWNER_EXCEPTION_PUBLIC_RELEASE`

Everything else should be resolved through frozen authority, objective evidence, independent review, bounded repair, and automatic progression.
