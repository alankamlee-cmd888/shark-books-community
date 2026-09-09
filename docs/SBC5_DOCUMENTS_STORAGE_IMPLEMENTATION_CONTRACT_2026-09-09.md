# SBC-5 Documents / Storage — Implementation Contract

**Date:** 9 September 2026  
**Batch:** SBC-2–5 bounded batch  
**Stage:** E / SBC-5  
**Entry:** protected SBC-4 PASS `main` `7f0ac276c7d2a0c2da5dd2597535e946004c404f`

## Scope

SBC-5 implements the document-reference and storage boundary already adjudicated in the Stage A integrated architecture. It does not implement final Tauri/UI wiring or any cloud-provider API.

The product core remains platform-neutral, persistence-free and dependency-free.

## Required behaviour

1. `DocumentReference` identifies the attached bookkeeping record, user-controlled storage root, portable relative path, original filename, optional media type, SHA-256 and byte length.
2. Document hashes are derived from bytes using the already-proven SBC-3 SHA-256 implementation.
3. Rehydrating persisted reference fields must preserve the attachment identity and allow later integrity verification.
4. Integrity verification distinguishes exact verification, size mismatch and same-size hash mismatch.
5. Storage roots are local filesystem folders or user-selected/synced filesystem folders. Custody is always `UserControlled`; there is no Shark central document-store provider.
6. The book stores only root identity/provider metadata plus relative document references. Provider credentials are not modelled.
7. Relative paths fail closed on absolute paths, Windows drive syntax, traversal segments, backslashes, control characters, blank segments or other non-portable forms.
8. Native source-file selection is represented by an opaque `NativeSelectionId`, not a webview-supplied arbitrary source path.
9. Native file adapters may implement bounded copy/read-verification requests rooted in a configured storage-root ID plus validated relative path. No broad arbitrary filesystem API is authorised.
10. Empty documents, malformed hashes, unsafe filenames and blank media-type values fail closed.

## Explicit non-scope

SBC-5 does not add:

- Shark-hosted document custody;
- Dropbox/OneDrive/iCloud/Google Drive APIs;
- OAuth, API keys, access tokens or cloud credentials;
- network access;
- arbitrary webview filesystem access;
- OCR;
- AI/LLM document interpretation;
- tax inference;
- final Tauri/native-dialog wiring;
- deletion/retention policy UX;
- browser OPFS parity;
- release installers/signing.

A user-selected synced folder may be identified as a provider type, but sync itself remains the user's/provider's filesystem responsibility.

## Exact bounded source set

Relative to SBC-4 PASS `7f0ac276c7d2a0c2da5dd2597535e946004c404f`, the candidate may change exactly:

1. `ci/run_sbc5_windows.ps1`
2. `docs/SBC5_DOCUMENTS_STORAGE_IMPLEMENTATION_CONTRACT_2026-09-09.md`
3. `product/shark-books-core/src/documents.rs`
4. `product/shark-books-core/src/lib_sbc3.rs`
5. `scripts/check_sbc5_documents_storage.py`

No product Cargo manifest/lock and no frozen native `workspace/` file may change.

## Required executable evidence

The Windows gate must prove:

- exact five-path source diff;
- product crate remains zero-third-party-dependency;
- product Cargo.lock remains exact and dependency-free;
- frozen native Cargo.lock remains exact;
- document source contains the bounded contract and no platform/network/persistence/credential implementation;
- frozen foundation guard PASS;
- SBC-1G permanent change-control guard PASS with no dependency/native-Apple change;
- Rust 1.98.1;
- all inherited SBC-2/SBC-3/SBC-4 tests and all new SBC-5 tests pass under `--locked`;
- both lockfiles remain unchanged;
- repository remains clean after proof.

## Hard stop

SBC-5 is not PASS until the committed validator completes successfully on Windows and the result is independently adjudicated.

Do not merge SBC-5 or declare the SBC-2–5 batch closed before that proof. After SBC-5 PASS/merge, execute Stage F combined reconciliation before closing the bounded batch.
