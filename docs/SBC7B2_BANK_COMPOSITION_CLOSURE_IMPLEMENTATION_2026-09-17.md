# SBC-7B2 Bank Composition Closure — Implementation

Authority: Library 195
Entry SHA: `5b32598b4f8135a705aba8973ff8ffadab1a27bf`

This bounded development slice closes exactly two frozen Bank UI composition gaps:

1. pre-import duplicate review using the same Foundation duplicate semantics as persistence;
2. deterministic match review for an already-persisted Bank activity row.

The parser-level preview commands remain intact. New review commands add Books-aware duplicate classification without persistence. Existing import-confirm commands revalidate the owner-reviewed duplicate snapshot before calling the existing atomic persistence path.

A new persisted-activity match-review command is read-only and reuses the existing deterministic match-review result shape. Match confirmation remains the only match mutation authority.

No database schema, dependency, Action Registry, UI, CSP, OCR, accounting posting rule, matching rule or reconciliation rule changes are made.

## Inherited frontend-test baseline

The inherited `frontend_contract_is_keyless_and_uses_only_bounded_commands` test is a known
pre-FT3 stale test and is not used as a Bank-composition regression gate in this slice.

At exact entry SHA `5b32598b4f8135a705aba8973ff8ffadab1a27bf`, that test already requires
`books_trial_balance` to appear in the frontend JavaScript bundle, while the committed Vite
bundle at the same SHA does not contain that command. The Bank Composition Closure changes
neither `workspace/ui/` nor `workspace/dist/`, and the test body and Vite JavaScript bytes
are mechanically proven unchanged from entry before publication.

Authority 194 assigns reconciliation of this stale frontend-security test to R1/UI-A, where
the permanent frontend command inventory will be updated as one coherent policy rather than
being altered inside this backend-only Bank closure.
