# SBC-8 Batch 2 — Operations + Forecast Domain Contract

Date: 2026-09-14
Status: FROZEN FOR PREACTIVATION INCUBATION
Entry protected main: `6f85734d0e9e11089a1a47b7850a306a4ed7ba87`

## 1. Purpose

Freeze the pure-domain contract for SBC-8D1/D2/D3 and SBC-8E1/E2 without coupling to the unfinished SBC-7 application or admitting any new dependency.

## 2. Primitive rules

- IDs and free-text fields are bounded and reject control characters.
- Money is signed integer minor units; checked arithmetic only; no floating point.
- Durations/distances are integer quantities with checked arithmetic.
- Civil dates are validated Gregorian dates.
- Domain APIs fail closed on invalid state transitions or arithmetic overflow.

## 3. Projects

A project has a stable identity, bounded name, optional customer identity and explicit lifecycle state.

Frozen lifecycle:

`Planned -> Active -> Paused -> Active -> Completed`

`Planned/Active/Paused -> Cancelled` is explicit. Completed/Cancelled are terminal in this incubation.

Project profitability/read facts are deterministic views over explicit inputs. Income and costs are money facts; time and mileage remain separately traceable operational quantities. Time/mileage never silently become costs or postings.

## 4. Timesheets

A time entry contains:

- stable entry identity;
- worker identity;
- project identity;
- activity text;
- start/end minute timestamps supplied by the caller;
- billable flag;
- immutable billing-rate snapshot where billable;
- notes;
- explicit state.

Frozen state family:

`Draft -> Approved -> BilledProposal`

Draft may be cancelled. Approved entries are immutable commercial evidence; corrections create a new entry identity linked by `supersedes` rather than rewriting the approved record.

Overlapping intervals for the same worker are detected and reported deterministically. Overlap detection never silently deletes or edits a time entry.

Only an Approved + billable entry may create a draft commercial-line proposal. The proposal is not an invoice and carries no posting authority.

## 5. Mileage

A mileage entry contains:

- stable identity/date;
- from/to text;
- business purpose;
- positive integer distance;
- explicit distance unit;
- source method: Manual, Odometer or RouteDerived;
- optional project/customer association;
- optional odometer start/end evidence for Odometer method.

Odometer evidence must be internally consistent with the recorded distance. Manual mileage remains fully functional with no map, route or location service.

An official/policy rate is a separate effective-dated, source-linked domain object. The engine does not hard-code a tax rate and does not infer tax treatment.

## 6. Recurrence

The public model is a Shark-owned restricted `RecurrenceSpec`, never raw RFC5545.

Supported incubation frequencies:

- OneOff;
- Daily;
- Weekly;
- Monthly;
- Yearly.

The spec includes bounded interval, start date, optional end date, optional occurrence count, bounded timezone identifier metadata, and a monthly policy of SameDay or MonthEnd.

Generation rules:

- every preview/generation call has an explicit hard cap;
- no unbounded enumeration;
- end date and count are both enforced;
- invalid dates fail closed;
- monthly SameDay clamps to the last valid day only when explicitly requested by the contract; MonthEnd always selects the calendar month end;
- leap years are Gregorian;
- timezone offset/DST resolution is **not claimed** by this zero-dependency incubation engine and remains a later admitted-adapter concern.

## 7. Deterministic cash-flow forecast

Forecasting is a projection, never accounting authority.

A forecast event contains:

- stable event identity;
- civil date;
- source reference and source kind;
- certainty: KnownContractual, Expected or ScenarioOnly;
- amount estimate: Exact, Approximate, or Range(low/high);
- optional scenario identity;
- explicit state: Planned, Skipped or LinkedActual.

`LinkedActual` explicitly suppresses the forecast event from future projection so schedule-to-actual linkage cannot double count.

Forecast output is a chronological trace. Each point records low/expected/high balances and the exact contribution identities used. Range amounts propagate a deterministic balance band rather than hiding a midpoint assumption.

ScenarioOnly events are included only when their scenario is explicitly enabled. KnownContractual and Expected events are baseline inputs unless skipped/linked actual.

Recurring events may create forecast-event proposals through the restricted recurrence engine. Recurrence owns dates only; it does not own cash-flow/accounting semantics.

## 8. Cross-domain proposals

Batch 2 may emit neutral proposal DTOs only:

- approved billable time -> `DraftCommercialLineProposal`;
- mileage where an explicit billing policy is supplied -> `DraftCommercialLineProposal`;
- recurrence -> planned forecast-event proposals.

No proposal may issue a commercial document, mutate a ledger, approve a bill, reconcile a bank transaction, or call persistence/network/native APIs.

## 9. Required regression families

At minimum prove:

- bounded IDs/text and checked money arithmetic;
- project lifecycle fail-closed transitions;
- deterministic profitability facts;
- valid duration calculation and invalid interval rejection;
- overlap detection for same worker but not unrelated workers;
- approved entry immutability/correction-by-new-identity;
- approved billable time -> draft line proposal only;
- mileage manual baseline without route dependency;
- odometer consistency rejection;
- effective-dated/source-linked rate selection;
- recurrence hard cap, end/count bounds, leap day and month-end behaviour;
- no unbounded recurrence;
- deterministic forecast ordering and traceability;
- range propagation into low/expected/high balances;
- scenario inclusion/exclusion;
- skipped and linked-actual events excluded from projection;
- recurrence-to-forecast creates proposals only;
- representative project -> time/mileage -> draft line and recurrence -> forecast journey remains domain-only.

## 10. Explicit non-goals

No persistence, accounting posting, Tauri/Vue/native binding, map/routing, network, RRule.rs, provider SDK, AI judgement, tax calculation, VAT, CIS, payroll, HMRC filing, Open Banking or third-party runtime dependency is admitted by this contract.
