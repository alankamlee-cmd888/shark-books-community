# Shark Books Community — SBC-4 Matching and Reconciliation Implementation Contract

**Date:** 9 September 2026  
**Batch:** SBC-2–5 bounded batch  
**Stage:** D / SBC-4 — Matching and reconciliation  
**Entry:** SBC-3 protected PASS `main` `8d39f0c509e9c1c6f2f714ac4d9f121432d67743`

## Scope

SBC-4 adds a deterministic, platform-neutral matching and reconciliation module to the standalone Shark-owned product core.

It does **not** persist ledger state, access SQLite/Beankeeper directly, open files or networks, call Open Banking, infer tax treatment, or automate ambiguous accounting decisions.

## Matching contract

Duplicate identity and transaction matching remain separate concepts.

Candidate classes are:

- `EXACT` — unique, high-specificity candidate with exact account/currency/amount/date and exact source identity or normalized reference;
- `LIKELY` — exact account/currency/amount with a date gap of at most three days plus strong source/reference/text evidence;
- `POSSIBLE` — exact account/currency/amount with a date gap of at most seven days and plausible supporting evidence;
- `UNMATCHED` — no safe candidate.

Every non-unmatched candidate requires user confirmation. No class authorises an automatic ledger mutation.

If more than one candidate independently qualifies as `EXACT`, `EXACT` is withdrawn: the candidates are downgraded and the result is explicitly ambiguous. This preserves the contract that `EXACT` means one unique candidate.

The matcher emits reason codes as well as a deterministic score. Scores are explanatory/ranking support only; reason codes and hard prerequisites remain inspectable.

SBC-3 `HEURISTIC` duplicate similarity remains review-only and never authorises automatic duplicate suppression or automatic matching.

## Clearance contract

The only permitted forward clearance sequence is:

`UNCLEARED -> CLEARED -> RECONCILED`

A user-confirmed non-unmatched match may produce an audited `UNCLEARED -> CLEARED` transition.

Matching may not jump directly to `RECONCILED`, and a cleared/reconciled item may not be silently reconfirmed as if new.

## Reconciliation contract

A statement reconciliation input contains:

- statement identity/date;
- opening statement balance in integer pence;
- expected ending balance in integer pence;
- selected entries with signed integer-pence amounts and current clearance state;
- actor identity supplied by the caller.

The deterministic check is:

`computed ending = opening balance + sum(selected signed entries)`

`difference = expected ending - computed ending`

Finalization is allowed only when:

1. at least one entry is selected;
2. every selected entry is currently `CLEARED` and not already reconciled;
3. the difference is exactly zero pence.

A successful finalization produces explicit audited `CLEARED -> RECONCILED` transition plans. Persistence remains a later controlled adapter/facade action.

## Correction / history contract

Reconciled history is not rewritten destructively.

A correction must identify the original record and create a distinct reversal record, with an optional distinct replacement record, actor and reason. Reuse of the original ID as the reversal/replacement is invalid.

## Validation contract

The committed SBC-4 Windows validator must prove:

- exact bounded five-path diff from the SBC-3 protected merge;
- product crate remains zero-third-party-dependency;
- product Cargo.lock remains byte-identical;
- frozen native Cargo.lock remains byte-identical;
- no native workspace/facade/Beankeeper/Tauri change;
- frozen foundation and SBC-1G change-control guards pass;
- Rust 1.98.1;
- all inherited SBC-2 and SBC-3 tests plus new SBC-4 tests pass under `--locked`;
- exact/likely/possible/unmatched behaviour;
- multiple-exact ambiguity handling;
- explicit user-confirmation requirement;
- uncleared-to-cleared transition only;
- zero-difference reconciliation requirement;
- uncleared/already-reconciled entries fail closed;
- correction-by-new-record IDs rather than destructive rewrite;
- repository and lockfiles remain clean after proof.

## Hard stop

SBC-5 document/storage implementation may not begin until this SBC-4 candidate passes the committed runtime proof, is formally adjudicated, and is merged through protected `main`.
