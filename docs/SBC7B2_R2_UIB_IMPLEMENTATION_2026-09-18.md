# SBC-7B2 R2 / FT3B / UI-B — Bounded Development Implementation

Date: 2026-09-18
Entry SHA: `1ad86cc7715aa38a8162952f02c8debe3d743736`
Scope authority: Library Authority 200 — R2/UI-B frozen implementation contract.

This commit implements only the frozen R2 development surface:

- Receipts/Documents list, native add, integrity verify, bounded view, manual attach;
- factual OCR with unavailable/failed/manual fallback;
- deterministic receipt-to-Bank suggestion with explicit confirm or reject;
- Contacts list/create/edit with customer/supplier filtering;
- factual Reports summary;
- Settings books information and session-scoped document storage selection;
- generated one-to-one UI-B Action Registry metadata;
- responsive/accessibility continuation from R1;
- minimum `blob:` CSP for verified in-memory image/PDF preview;
- frontend security-test reconciliation for the admitted finite UI-B commands.

Hard boundaries remain unchanged: no Action Registry row changes, no accounting-engine semantics, no persistence schema migration, no npm/Cargo dependency changes, no remote network/runtime dependency, no raw path/database/passphrase authority, no automatic receipt-to-Bank acceptance, no tax/VAT/deductibility inference, no merge, and no final FT3/FT4 candidate freeze.

The R2 Windows runner must prove generated Action metadata, UI-A regression, UI-B static architecture, exact package-lock stability, `npm ci`, Vue typecheck, Vite build, generated dist hashes, focused Rust/Tauri regressions, bounded CSP/security, native `cargo check`, exact changed-path envelope, remote-head stability, and clean publication before pushing the single development commit.
