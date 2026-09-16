# SBC-8 Batch 2 — Preactivation Incubation Authority

Date: 2026-09-14
Status: ISOLATED PURE-DOMAIN INCUBATION ONLY / PRODUCTION INTEGRATION BLOCKED

## Authority

The owner authorised continuation from the independently adjudicated SBC-8 Batch 1 `INCUBATION DOMAIN PASS` into Batch 2 where suitable. This is a bounded exception to the otherwise-frozen SBC-8 production activation rule. It does not activate SBC-8 production integration and it does not authorise any SBC-7 change.

Entry protected `main` for this independent Batch 2 branch:

`6f85734d0e9e11089a1a47b7850a306a4ed7ba87`

Branch:

`sbc8-incubation-batch2-operations-forecast-20260914`

## Frozen incubation scope

Batch 2 may implement only Shark-owned pure-domain logic for:

- SBC-8D1 projects;
- SBC-8D2 timesheets;
- SBC-8D3 mileage;
- SBC-8E1 restricted recurrence semantics;
- SBC-8E2 deterministic, traceable cash-flow forecasting.

The implementation must remain a standalone incubator crate under `incubator/` and must not enter the active product/native graph.

## Explicit exclusions

This batch must not change or integrate:

- active SBC-7 branch/work;
- `product/` or `workspace/` runtime surfaces;
- Shark foundation, Beankeeper, SQLite/SQLCipher or ledger persistence;
- Tauri commands, capabilities or permissions;
- Vue/frontend bindings;
- network access or provider SDKs;
- map/routing providers or location services;
- RRule.rs or any other new third-party runtime dependency;
- PDF/email/payment-provider adapters;
- VAT, e-invoicing, CIS, payroll, direct HMRC submission or Open Banking;
- autonomous AI accounting/tax judgement.

## Recurrence rule during incubation

RRule.rs remains a later SBC-8P dependency candidate. This incubation may prove the restricted Shark recurrence contract and bounded deterministic calendar generation only. Upstream/library types must not leak into the public Shark model and no dependency is admitted by this branch.

Timezone identity may be captured as bounded domain metadata; timezone offset/DST resolution requiring an admitted timezone/recurrence engine remains a later SBC-8P integration proof. The incubation generator must not claim timezone-offset resolution it cannot prove.

## Operations/accounting boundary

- Projects, time entries and mileage are operational evidence, not ledger postings.
- Approved billable operational evidence may create typed **draft commercial-line proposals only**; it may never issue an invoice automatically.
- Official mileage rates/rules are effective-dated and source-linked; no timeless tax/rate constant is embedded.
- Forecasts are projections only. Forecast events never become ledger entries automatically.
- Every projected amount must remain traceable to source facts and explicit assumptions.

## Evidence rule

The batch can earn only `INCUBATION DOMAIN PASS` at an exact immutable candidate SHA after source review, static allow-list validation, standalone Rust tests, bounded Windows evidence and independent evidence adjudication. Apple/native proof is deferred because this crate deliberately remains outside the native product graph.

No Batch 2 PASS authorises merge while SBC-7 remains open. Later SBC-8P must re-adjudicate all interfaces/dependencies/persistence/platform integration against the then-current protected `main`.
