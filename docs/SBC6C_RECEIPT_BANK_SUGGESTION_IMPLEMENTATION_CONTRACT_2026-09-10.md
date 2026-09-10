# SBC-6C — Deterministic receipt-to-bank suggestion implementation contract

**Date:** 10 September 2026  
**Protected entry main:** `5748b0236d3dc0f89975b9d7cb021af05f742f43`  
**Scope:** Shark Books Community, UK sole-trader product core  
**Status:** implementation candidate; evidence required before PASS/merge

## Objective

Compose already-proven factual receipt OCR output with already-proven canonical bank-line data to produce deterministic, inspectable receipt-to-bank **suggestions**.

The bounded flow is:

`factual OCR extraction -> deterministic bank-line comparison -> reason-coded ranked suggestion -> explicit user confirm or reject decision`

This stage does not post accounting entries, clear or reconcile bank records, decide business/private use, categorise expenditure, decide deductibility/tax/VAT treatment, or perform HMRC actions.

## Reused contracts

SBC-6C reuses without rewriting:

- SBC-3 `BankLine` canonical bank facts and separate duplicate-certainty semantics;
- SBC-4 `MatchLevel` vocabulary and the established deterministic/tie-safe matching discipline;
- SBC-5 receipt/document identity boundaries;
- SBC-6B B2 `OcrExtraction` factual candidates only.

No new crate dependency is authorised. No native/Tauri source or native `Cargo.lock` change is authorised.

## Candidate identity

A bank-line suggestion is identified by the bounded immutable tuple:

- source account id;
- source file SHA-256;
- source locator;
- raw record SHA-256.

This identity is not a duplicate decision. Two similar-looking imported rows remain separate suggestion candidates unless the independent SBC-3 duplicate contract has already established otherwise.

## Factual signals and deterministic scoring

The comparison may use only these receipt facts when present:

- total pence;
- document date;
- factual currency code;
- reference text;
- merchant text.

The bank side may use the corresponding canonical `BankLine` fields.

### Hard fail-closed conditions

A candidate is `Unmatched` when any of these applies:

- receipt total is missing;
- bank line is not an outflow for this purchase-receipt flow;
- the negative bank amount cannot be represented as a positive magnitude;
- bank amount magnitude differs from receipt total by even one penny;
- OCR supplied a factual currency and it differs from the bank-line currency;
- OCR supplied a factual date and it is more than seven calendar days from the bank posting date.

Missing currency/date are not silently invented. Missing currency therefore cannot contribute to an `Exact` result.

### Positive evidence weights

- exact amount magnitude: +40;
- factual currency exact: +10;
- exact date: +20;
- date within 1–3 days: +15;
- date within 4–7 days: +8;
- normalized reference exact: +25;
- strong merchant/payee support: +15;
- strong merchant/description support: +10.

Text normalization is deterministic and mirrors the already-proven SBC-4 style: lowercase alphanumeric tokens with punctuation treated as spacing. Strong text support is exact normalized equality, or at least two common tokens of length >=3 covering at least half of the shorter token sequence.

### Match levels

- `Exact`: exact amount + factual currency exact + exact date + either exact reference or strong merchant support in both payee and description.
- `Likely`: exact amount + date within three days + reference or strong merchant support.
- `Possible`: exact amount + date within seven days even without text support, or date missing with strong reference/merchant support.
- `Unmatched`: otherwise.

These labels are suggestion confidence only. Every non-unmatched result still requires user confirmation.

## Ranking and ambiguity

Ordering is deterministic:

1. match level descending;
2. score descending;
3. stable bank-line identity ascending.

More than one `Exact` candidate is downgraded from `Exact`, marked `MultipleTopCandidates`, and produces no recommendation. Any equal top level+score tie is also ambiguous and produces no recommendation.

A unique top non-unmatched candidate may be presented as the recommendation, but it is never automatically accepted.

## Explicit decisions

`confirm_receipt_bank_suggestion` may return a bounded immutable decision only when the chosen bank-line identity belongs to the suggestion set and is not `Unmatched`.

`reject_receipt_bank_suggestions` returns a rejection decision only.

Both require a nonblank actor. Neither function mutates OCR facts, bank lines, clearance state, reconciliation state, ledger postings or persistence.

Persistence/application wiring of an accepted document-bank association is deferred to the later combined flow and remains subject to the same explicit-user boundary.

## Hard prohibitions

SBC-6C must not introduce:

- LLM/AI judgement;
- tax, VAT, CIS, payroll or company logic;
- expense-category or business/private inference;
- automatic ledger posting;
- automatic match confirmation;
- automatic bank clearance or reconciliation;
- network/cloud OCR;
- filesystem/process/native/Tauri APIs;
- arbitrary path/model/executable/shell fields;
- final SBC-7 UI;
- dependency or frozen-foundation drift.

## Required evidence before PASS

The committed gate must prove:

1. exact authorised diff from protected entry main;
2. zero product dependencies and unchanged product/native lockfiles;
3. inherited frozen-foundation/change-control guards pass;
4. exact-pence amount matching and date windows behave deterministically;
5. merchant/reference signals remain bounded supporting evidence;
6. missing facts remain missing rather than inferred;
7. ties/multiple exact candidates are ambiguous and not auto-selected;
8. duplicate-looking bank lines remain distinct in this layer;
9. user confirmation is required and unmatched candidates cannot be confirmed;
10. rejection is non-destructive;
11. no accounting/tax/posting/reconciliation authority is added;
12. all inherited SBC-2/3/4/5/6B product-core tests and new SBC-6C tests pass under `--locked`;
13. repository and lockfiles remain clean after proof.

Only after formal PASS and protected merge may SBC-6D begin.
