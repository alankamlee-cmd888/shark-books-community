# SBC-7B1 Owner Application Bridge Contract — 2026-09-10

## Status
Implementation candidate stage. This is the first implementation slice beneath the frozen SBC-7A owner UI contract.

## Entry point
Protected `main` at entry: `70777e04ccfc1e427c60e18269b8a0738cc8610c` (SBC-7A merged/closed).

## Purpose
Expose a small, typed owner-facing application operation surface from the Tauri/native shell to the already-proven Shark product core and foundation facade before any owner-facing Vue binding is added.

This slice provides:
- factual Home/books status;
- deterministic Money in preview;
- explicit Money in save;
- deterministic Money out preview;
- explicit Money out save.

It does not attempt to complete all SBC-7 owner journeys. Bank, receipt, contact, report and settings binding remain later SBC-7B slices under the frozen SBC-7A contract.

## Required boundaries
1. The webview may provide only typed bounded DTO fields. It may not provide an arbitrary database path, encryption key/passphrase, account-code posting plan, debit/credit lines, model path, executable path, shell command or URL.
2. The bridge may depend on `shark-books-core` only through the local reviewed path dependency. No external package movement is permitted.
3. `shark-books-core` remains platform-neutral and unchanged in this slice.
4. `ocr_native.rs` and the B3C OCR runtime/model boundary remain unchanged.
5. The bridge must derive posting plans from the proven product-core domain functions; the frontend must never construct double-entry posting lines.
6. All persistence still passes through `shark-foundation::Books`, which validates through the pinned Beankeeper accounting core before persistence.
7. Money in/out preview does not persist anything and must report `requiresConfirmation=true`.
8. Money in/out save occurs only through its explicit save command. No preview may automatically invoke save.
9. Business/private/mixed treatment for Money out is explicit owner input. The bridge must not infer business use.
10. GBP whole-pence arithmetic remains integer-based. Zero/negative amounts and invalid dates fail closed.
11. No VAT, CIS, payroll, company, partnership, tax-calculation or personalised-advice logic is introduced.
12. No automatic matching, reconciliation, categorisation or posting from OCR/bank suggestions is introduced.
13. No network/cloud call, generic shell/process surface or broad filesystem capability is introduced.
14. The existing proof-only environment key provider remains an engineering gate mechanism, not a production secure-storage claim.
15. The current frontend remains unchanged and must not contain/call the new `owner_*` commands during 7B1. Vue binding starts only after 7B1 passes and merges.

## Owner command allow-list
Exactly these new owner commands are added in this slice:
- `owner_home_status`
- `owner_money_in_preview`
- `owner_money_in_save`
- `owner_money_out_preview`
- `owner_money_out_save`

They must be present in the Rust invoke handler, Tauri application manifest and `shark-shell` permission allow-list.

## Lockfile rule
The reviewed post-change native lockfile SHA-256 is:
`8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61`

Removing only:
- the local `shark-books-core 0.0.1` package block; and
- the `"shark-books-core",` dependency edge under `shark-tauri-spike`

must reproduce the pre-7B native lockfile byte-for-byte, SHA-256:
`3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`.

No registry package version/source/checksum movement is permitted.

## Gate evidence
Windows proof must establish:
- exact entry-main ancestry;
- exact changed-path allow-list;
- reviewed lockfile and reversible local-only lock delta;
- product core unchanged;
- OCR-native implementation unchanged;
- frontend unchanged and owner commands not yet bound;
- foundation regressions pass;
- product-core regressions pass;
- Tauri shell/owner bridge tests pass under Rust 1.98.1 and `--locked`;
- locked Windows compilation succeeds;
- repository remains clean.

Apple/Codemagic regression must establish on the same final candidate SHA:
- same reviewed lockfile;
- static 7B1 boundary validator pass;
- foundation regressions pass on macOS;
- Tauri shell including the local product-core dependency compiles for physical `aarch64-apple-ios` and `aarch64-apple-ios-sim` targets;
- repository and lockfile remain unchanged.

## Merge rule
Do not merge SBC-7B1 until both Windows and Apple evidence from the exact same candidate SHA have been inspected and formally adjudicated PASS.

SBC-7B2 owner-facing Vue binding remains blocked until SBC-7B1 is merged.
