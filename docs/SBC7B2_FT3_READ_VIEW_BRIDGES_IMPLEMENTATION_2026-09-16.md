# SBC-7B2 FT3 — Owner-safe read/view bridges implementation

Date: 2026-09-16  
Status: DEVELOPMENT IMPLEMENTATION — NOT A FROZEN CANDIDATE

This slice implements the five read/view gaps admitted by the frozen FT3A+FT3B+FT4 contract:

- `MONEY.RECORDS.LIST`
- `MONEY.RECORD.DETAIL`
- `BANK.ACTIVITY_DETAIL`
- `DOCUMENT.LIST`
- `DOCUMENT.OPEN_VIEW`

## Boundaries

Money reads are collapsed in Foundation to owner semantics before crossing Tauri. No debit/credit rows, account codes, tax categories, posting lines or generic transaction-query authority are serialized to Vue.

The Money list is bounded by exact canonical `sbc7b1:moneyIn:` / `sbc7b1:moneyOut:` references. Correction replacements use the same canonical owner-record form and therefore remain visible, while correction reversals use the distinct `sbc7b1:correctionReversal:` form and are excluded. Records already superseded by a correction are omitted from the conventional current-record list; their history remains available through Money detail.

Money detail returns the owner record plus bounded attached-document metadata and bounded correction history. It does not expose native paths, document hashes or accounting entries.

Bank detail rereads one existing `BankActivityView` by positive opaque activity ID and returns factual owner-safe fields only. Source-file/raw-record hashes, source locators, institution IDs, matched entry IDs and provenance fingerprints remain native/Foundation-only.

Document list returns only opaque document ID, filename, media type, byte length and registration time. It does not return storage-root identity, relative/native path or hash.

Document open accepts books context plus opaque document ID only. It rereads canonical metadata, resolves the already-configured trusted root natively, enforces canonical containment, verifies current length/hash, performs a second bounded read and length/hash check to close the read-time integrity window, then returns raw bytes through Tauri native binary IPC. No native path crosses IPC.

## Change classification

- no Shark application schema change;
- no Beankeeper change;
- no Cargo dependency change;
- no npm dependency change;
- no accounting mutation authority;
- no Action Registry activation yet;
- no FT3/FT4 candidate freeze or merge authority.

The Action Registry entries stay locked until this backend slice passes focused development proof. UI bindings and Action activation follow in a later atomic step.
