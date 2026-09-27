# SBC-7B2 R1 / FT3A / UI-A Implementation

Authorities: 197 + 198  
Entry SHA: `8f943302c6aeea863983bbd634a691b635ccc2b9`

This development batch implements the frozen UI-A owner surface as one substantial unit:

- permanent shell/session context with owner-language Books create/open;
- Assistant Home / deterministic Attention foundation;
- factual Home status;
- Money In / Money Out list, detail, create, preview, explicit save and correction;
- Bank local statement review, duplicate review, explicit import confirmation, activity list/detail, deterministic persisted-activity match review/confirm and reconciliation;
- generated Action metadata sourced from the canonical Action Registry;
- Reka UI confirmation dialogs;
- TanStack Vue Table v9 row modelling for Money and Bank lists;
- responsive/accessibility foundation;
- coherent repair of the inherited frontend command-security test.

Authority 198 is applied inside this same batch: persisted Bank match review no longer trusts candidate transaction IDs from Vue. Rust discovers a bounded current candidate set from current owner Money records, skips Cash, already-matched and reconciled transactions, and calls the existing frozen matcher.

No Action Registry JSONL lifecycle/exposure row is changed. No schema, Cargo/npm dependency manifest or lockfile, CSP, OCR, Foundation accounting logic, product-core matching logic or Bank mutation code is changed.

This is a development implementation only. It does not freeze the integrated FT3/FT4 candidate and does not authorise merge.
