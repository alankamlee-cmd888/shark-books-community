# SBC-7B1 Slice 3A — Documents + factual OCR owner bridge contract

**Date:** 12 September 2026  
**Entry protected main:** `50c898c7b8042e11810933e3e596fc124085e831`  
**Status:** FROZEN ENTRY CONTRACT — implementation not yet proven

## Purpose

Close the next owner-facing SBC-7B1 application gap for the frozen Receipts/document journey without widening Shark Books into cloud custody, arbitrary filesystem access, accounting judgement, or tax advice.

This slice composes already-proven SBC-5 document/storage contracts and SBC-6B factual local OCR behind exact typed owner commands. The webview must never receive or supply arbitrary operating-system paths, storage credentials, OCR executable/model paths, database paths, passphrases, or ledger-shaped mutation data.

## Frozen source authorities

- SBC-5 `documents` contract: user-controlled storage, relative references, SHA-256 + byte-length integrity, opaque native selection IDs, no cloud-provider API requirement.
- SBC-6B OCR contract/runtime: optional local factual OCR only; bounded document identity; provenance/warnings; no accounting, matching, tax, category or deductibility authority.
- SBC-7A operation matrix and owner-facing UI contract: `DOCUMENT.SELECT_REGISTER`, `DOCUMENT.VERIFY`, `DOCUMENT.ATTACH`, `OCR.EXTRACT`.
- Protected main through SBC-7B1 Slice 2B: `50c898c7b8042e11810933e3e596fc124085e831`.

## Exact owner operation family

The implementation may expose exactly these new owner operations for this slice:

1. `owner_document_select_register`
2. `owner_document_verify`
3. `owner_document_attach`
4. `owner_ocr_extract_receipt`

The historical bounded `ocr_extract_receipt` native command may remain as an internal/native implementation primitive, but the frozen owner-facing surface is `owner_ocr_extract_receipt` and must accept only typed/opaque IDs.

No frontend wiring is authorised in Slice 3A. SBC-7B2 remains blocked.

## 1. `owner_document_select_register`

### Intent
Allow an owner to choose/register one document using a native-owned picker/selection boundary.

### Owner input
May contain only bounded semantic values such as:
- opaque books/company context;
- opaque configured storage-root ID where required;
- optional owner-visible filename/label hints that are revalidated natively.

### Forbidden input
Must not accept:
- absolute or relative OS source path supplied by the webview;
- `file://` URL supplied by the webview;
- cloud provider credential/token;
- arbitrary destination path;
- database path;
- passphrase/key;
- hash or byte length asserted as authority by the UI.

### Native authority
The native/platform layer owns file selection and any source path. It must derive the document bytes, SHA-256 and byte length itself. If copying into a configured user-controlled root, the destination is resolved from a native-known root plus a validated portable relative reference.

### Result
Return only bounded metadata, including an opaque document ID, original filename/media type where known, byte length, SHA-256, storage-root ID/portable relative reference only where appropriate for owner display, and explicit cancelled/failed outcome. Never return an absolute OS path or provider credential.

## 2. `owner_document_verify`

### Intent
Re-read the currently registered document through native-owned storage resolution and verify its integrity.

### Input
Opaque document ID only, plus bounded books/company context where required.

### Required behaviour
- reread the persisted/current canonical document reference;
- resolve storage location natively;
- compare current byte length and SHA-256 with the canonical registered identity;
- return only `verified`, `size_mismatch`, `hash_mismatch`, `missing` or another explicit bounded failure state;
- fail closed on missing root/document, unsafe reference, or ambiguous identity.

Verification must not mutate accounting records.

## 3. `owner_document_attach`

### Intent
Attach an already registered document to an existing typed Shark record.

### Input
- opaque document ID;
- opaque target record ID;
- bounded target record kind if required to prevent cross-family ambiguity.

### Required behaviour
- reread canonical document and target record;
- require document integrity to be valid at attachment time;
- persist only the typed document-to-record relationship / canonical reference metadata;
- attachment is not evidence that an expense is allowable, business, private, deductible, matched or reconciled;
- repeated identical attachment must be idempotent or return an explicit already-attached result;
- conflicting ownership/context must fail closed.

The webview must not be able to inject ledger entries, posting lines, account IDs, tax treatment or path data through this command.

## 4. `owner_ocr_extract_receipt`

### Intent
Run optional local factual receipt OCR on an already registered, integrity-verified document.

### Input
- opaque OCR request ID;
- opaque registered document ID;
- bounded books/company context where required.

### Required behaviour
- reread canonical document identity;
- verify current bytes before OCR;
- approve/resolve the native document internally; the UI cannot approve a path;
- invoke only the frozen bounded local OCR adapter/runtime;
- preserve the existing timeout/output/resource/privacy constraints;
- return factual extraction only: merchant text, document date, total, currency, reference, raw text/regions where contractually supported, provenance and warnings;
- preserve explicit unavailable/failed outcomes.

### No authority
OCR must never:
- create/post accounting transactions;
- choose business/private/mixed treatment;
- determine category/account;
- determine deductibility or tax treatment;
- select or confirm a bank match;
- finalise reconciliation;
- infer VAT/CIS/payroll treatment;
- call a cloud OCR service.

## Persistence / application-schema boundary

Document registration/attachment requires a Shark-owned application persistence layer if no existing durable canonical store is sufficient. Any such change must:

- remain separate from Beankeeper schema 8;
- advance the Shark application schema only if genuinely required and do so through a backed-up, atomic migration;
- store bounded metadata/references, not permanent Shark-hosted document bytes;
- keep user document custody in local/user-selected synced storage;
- use immutable/canonical document identity fields where practical;
- preserve auditability of attachment creation without rewriting historical accounting data.

No schema version is pre-authorised by this contract; implementation must justify and test the smallest necessary migration before candidate freeze.

## Storage boundary

Allowed storage providers remain the frozen SBC-5 user-controlled set (local filesystem or user-selected synced folders such as OneDrive, Dropbox, iCloud Drive, Google Drive or another user-selected synced folder).

This slice does **not** add provider APIs, OAuth, background upload/download, Shark-hosted document storage, or cloud credentials.

Configured storage-root creation/editing is not widened here. Where a root is required, only an already native-known/configured root ID may be used. Settings/root-management UI remains a later owner bridge unless separately authorised.

## Security and boundedness

Implementation must preserve at least:

- `deny_unknown_fields` on owner request DTOs where applicable;
- bounded identifier lengths;
- document max-size enforcement consistent with frozen document/OCR contracts (OCR native max remains 25 MiB unless separately reviewed);
- portable relative-path validation only inside trusted native/application layers;
- no arbitrary webview filesystem capability;
- no generic shell capability;
- no raw SQLite/Beankeeper surface;
- no network/cloud OCR requirement;
- no credentials/secrets in serializable DTOs;
- no silent mutation after OCR.

## Expected tests

At minimum, the candidate gate must prove:

1. source path never crosses the owner/webview command boundary;
2. native selection derives hash + length from actual bytes;
3. absolute/traversal paths fail closed inside trusted storage resolution;
4. changed/tampered document fails integrity verification;
5. missing document/root fails closed;
6. attach uses typed document + record IDs only;
7. duplicate attachment semantics are explicit/idempotent;
8. OCR can run only for a native-approved registered document;
9. OCR refuses integrity drift before execution;
10. OCR returns factual fields/provenance/warnings only;
11. no accounting posting/category/tax/matching/reconciliation authority is introduced;
12. prior product-core, foundation, owner bridge, Bank review and Bank mutation regressions remain green;
13. Cargo/dependency changes are absent unless separately justified before candidate freeze;
14. frontend remains unwired;
15. repository and lockfile are clean/exact after proof.

## Dual-platform proof

Final Slice 3A candidate must be immutable and pass the bounded Windows gate and Codemagic Apple gate on the **same exact SHA** before merge.

Apple may return the frozen explicit OCR `unsupported_platform` outcome where the current OCR runtime is Windows-only; that is acceptable only if the typed owner/document boundary compiles and tests on physical iOS + iOS Simulator targets and no false OCR-success claim is made.

## Explicit exclusions

Not authorised in this slice:

- receipt-to-bank suggestion confirm/reject;
- correction/reversal journey;
- Contacts or Settings general CRUD;
- storage-root management UI;
- owner-facing frontend binding;
- cloud provider API integration;
- cloud OCR;
- AI/LLM accounting judgement;
- VAT, CIS, payroll, direct HMRC filing;
- tax liability/reserve/deductibility advice;
- Open Banking/live feeds;
- automatic bookkeeping decision from OCR.

## Entry gate

Implementation starts only from protected main `50c898c7b8042e11810933e3e596fc124085e831` on branch:

`sbc7b1-documents-ocr-owner-bridge-20260912`

Before candidate freeze, produce a deterministic change allow-list and validator that proves this contract. No merge is permitted until same-SHA Windows + Apple evidence passes.