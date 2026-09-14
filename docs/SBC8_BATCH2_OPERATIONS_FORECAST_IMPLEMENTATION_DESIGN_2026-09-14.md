# SBC-8 Batch 2 — Operations + Forecast Implementation Design

Date: 2026-09-14
Status: REFROZEN FOR ISOLATED INCUBATION AFTER SOURCE-REVIEW HARDENING

## Package

Standalone Rust crate:

`incubator/sbc8-operations-forecast-core/`

No dependencies. `#![forbid(unsafe_code)]`.

Modules:

- `primitives.rs` — bounded IDs/text, checked `Money`, sealed neutral draft-commercial proposals;
- `date.rs` — validated Gregorian `CivilDate`, comparison and bounded calendar stepping;
- `projects.rs` — project lifecycle, duplicate-safe project facts and deterministic summaries;
- `timesheets.rs` — sealed time-entry lifecycle, duplicate-safe overlap detection, approved billable draft-line proposal;
- `mileage.rs` — sealed manual/odometer/route-derived evidence, explicit approval/billable lifecycle, effective-dated source-linked policy rates;
- `recurrence.rs` — sealed restricted, bounded Shark recurrence generator, no timezone-offset resolution claim;
- `forecast.rs` — sealed forecast events/templates, duplicate-safe deterministic low/expected/high traceable bands and scenario filtering;
- `lib.rs` — exports plus representative domain-only journeys/tests.

## Independence from Batch 1

Batch 2 deliberately does not depend on the unmerged Batch 1 crate. Neutral proposal DTOs model the later typed seam. SBC-8P will reconcile/shared-type these interfaces against the final admitted commercial engine after SBC-7 closure.

This avoids stacking Batch 2 on Draft PR #24 and prevents an incubation PR from acquiring production integration authority accidentally.

## Date/time design

The crate does not invent a timezone database. Calendar recurrence operates on validated local civil dates plus bounded timezone-identifier metadata. Minute timestamps on time entries are caller-provided integer timeline values used only for duration/overlap facts.

Calendar stepping avoids unbounded day-by-day work for large recurrence intervals while remaining inside the supported year range. Every recurrence preview/generation call remains capped at 512 occurrences.

RRule/timezone adapters, DST offset resolution and any platform clock conversion are deferred to SBC-8P admission/proof.

## Money/rounding design

Money is integer minor units. Time billing uses a snapshotted integer minor-unit hourly rate and deterministic nearest-minor-unit rounding with ties away from zero, implemented with checked i128 intermediates. Mileage proposal arithmetic also uses checked i128 intermediates. No float arithmetic is permitted.

Forecast ranges preserve low/expected/high bands rather than silently selecting one point estimate. Approximate values are represented as one explicit approximate amount but remain labelled as approximate in the contribution trace.

## Audit/history design

- terminal project states do not reopen in incubation;
- duplicate project fact identities fail closed rather than double-counting;
- approved time evidence is not edited; correction creates a new identity with `supersedes`;
- duplicate time-entry identities fail closed during overlap analysis;
- mileage evidence/rates expose validated fields only through read accessors;
- only approved + billable mileage may emit one draft commercial-line proposal; repeat emission fails closed;
- policy/rate facts are effective-dated and source-linked; duplicate identities and overlapping applicable rates fail closed;
- recurrence constructor invariants cannot be bypassed by public fields;
- forecast events/templates expose validated fields only through controlled APIs;
- duplicate forecast-event identities fail closed rather than double-counting;
- forecast events have explicit skip/actual-link states;
- generated proposals have no persistence/posting/issue authority and the neutral commercial proposal itself is not caller-forgeable through public fields.

## Static proof design

`scripts/check_sbc8_batch2_operations_forecast_domain.py` will enforce:

- exact changed-path allow-list relative to entry protected main;
- active product/workspace/native/SBC-7 surfaces unchanged;
- zero dependencies and standalone lock;
- forbidden integration/network/tax markers absent from runtime source;
- no float domain arithmetic;
- required module/type/function anchors;
- validated stateful structs retain private fields;
- required focused regression names;
- clean repository before/after runtime tests;
- exact HEAD binding.

`ci/run_sbc8_batch2_operations_forecast_windows.ps1` requires `-ExpectedHead`, runs the validator/tests using Rust 1.98.1, captures hashes/status/path evidence, builds a bounded ZIP, and preserves a second byte-identical evidence copy on the Desktop so a later failed rerun cannot silently erase the last complete archive.

## Candidate rule

The development head is not a PASS candidate until deep source review and executable proof are clean. Repairs stay on the Batch 2 branch only. Once one exact head is frozen for proof, any further source/harness movement creates a new candidate.
