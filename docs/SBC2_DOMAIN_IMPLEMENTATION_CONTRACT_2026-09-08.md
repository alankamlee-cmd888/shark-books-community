# SBC-2 Domain Implementation Contract

Entry main: `9fd34510bc91b2877e4e640e82af40432f57e2b7`.

This branch implements only Stage B / SBC-2 of the hard-gated SBC-2–5 batch.

The implementation is intentionally isolated in `product/shark-books-core` and has no third-party dependencies. It does not edit the frozen native workspace, approved production facade, Beankeeper pin, Tauri capabilities or native Cargo.lock.

Required proof before merge:

- all `shark-books-core` tests PASS under `--locked`;
- no third-party dependency in the SBC-2 crate;
- no `beankeeper`, `rusqlite`, `tauri`, network or persistence import in the product core;
- frozen foundation guard PASS;
- SBC-1G change-control guard PASS;
- native Cargo.lock remains SHA-256 `3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`;
- no SBC-3 bank parser/matcher implementation enters this branch.

SBC-3 remains hard-blocked until this gate is formally adjudicated PASS and merged through protected `main`.

Implementation hardening:

- externally constructible invariant-bearing values use validated constructors with private fields/getters;
- money, dates, record IDs, provenance, invoice dates and posting-plan internals cannot be bypassed by public struct literals;
- posting-plan functions remain deterministic and persistence-free;
- private/personal purchases from business funds map to Owner Drawings, not an expense;
- mixed-use expense allocation uses integer basis points and assigns the integer remainder to the private/drawings side so total pence remain exact;
- owner contributions/drawings map to equity accounts, never revenue/expense;
- user-facing expense categories are bookkeeping labels only and do not claim tax deductibility.

Validation entry point: `scripts/check_sbc2_domain.py` (Windows wrapper: `ci/run_sbc2_windows.ps1`).
