# SBC-8 Batch 2 — Operations + Forecast Implementation Design

Date: 2026-09-14
Status: FROZEN FOR ISOLATED INCUBATION

## Package

Standalone Rust crate:

`incubator/sbc8-operations-forecast-core/`

No dependencies. `#![forbid(unsafe_code)]`.

Modules:

- `primitives.rs` — bounded IDs/text, checked `Money`, duration/distance helpers;
- `date.rs` — validated Gregorian `CivilDate`, comparison and bounded calendar stepping;
- `projects.rs` — project lifecycle and deterministic project facts;
- `timesheets.rs` — time-entry lifecycle, overlap detection, explicit draft-line proposal;
- `mileage.rs` — manual/odometer/route-derived evidence and effective-dated policy rates;
- `recurrence.rs` — restricted, bounded Shark recurrence generator, no timezone-offset resolution claim;
- `forecast.rs` — deterministic low/expected/high traceable forecast bands and scenario filtering;
- `lib.rs` — exports plus representative domain-only journeys/tests.

## Independence from Batch 1

Batch 2 deliberately does not depend on the unmerged Batch 1 crate. Neutral proposal DTOs model the later typed seam. SBC-8P will reconcile/shared-type these interfaces against the final admitted commercial engine after SBC-7 closure.

This avoids stacking Batch 2 on Draft PR #24 and prevents an incubation PR from acquiring production integration authority accidentally.

## Date/time design

The crate does not invent a timezone database. Calendar recurrence operates on validated local civil dates plus bounded timezone-identifier metadata. Minute timestamps on time entries are caller-provided integer timeline values used only for duration/overlap facts.

RRule/timezone adapters, DST offset resolution and any platform clock conversion are deferred to SBC-8P admission/proof.

## Money/rounding design

Money is integer minor units. Time billing uses a snapshotted integer minor-unit hourly rate and deterministic nearest-minor-unit rounding with ties away from zero, implemented with checked i128 intermediates. No float arithmetic is permitted.

Forecast ranges preserve low/expected/high bands rather than silently selecting one point estimate. Approximate values are represented as one explicit approximate amount but remain labelled as approximate in the contribution trace.

## Audit/history design

- terminal project states do not reopen in incubation;
- approved time evidence is not edited; correction creates a new identity with `supersedes`;
- mileage source method is explicit;
- policy/rate facts are effective-dated and source-linked;
- forecast events have explicit skip/actual-link states;
- generated proposals have no persistence/posting/issue authority.

## Static proof design

`scripts/check_sbc8_batch2_operations_forecast_domain.py` will enforce:

- exact changed-path allow-list relative to entry protected main;
- active product/workspace/native/SBC-7 surfaces unchanged;
- zero dependencies and standalone lock;
- forbidden integration/network/tax markers absent from runtime source;
- required module/type/function anchors;
- required focused regression names;
- clean repository before/after runtime tests;
- exact HEAD binding.

`ci/run_sbc8_batch2_operations_forecast_windows.ps1` will require `-ExpectedHead`, run the validator/tests using Rust 1.98.1, capture hashes/status/path evidence and build a bounded ZIP for independent adjudication.

## Candidate rule

The development head is not a PASS candidate until deep source review and executable proof are clean. Repairs stay on the Batch 2 branch only. Once one exact head is frozen for proof, any further source/harness movement creates a new candidate.
