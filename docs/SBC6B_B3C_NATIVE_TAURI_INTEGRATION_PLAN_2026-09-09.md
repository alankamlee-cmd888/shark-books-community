# SBC-6B B3C Native/Tauri OCR Integration

**Date:** 9 September 2026  
**Status:** **CANDIDATE READY / WINDOWS + APPLE EXECUTION EVIDENCE PENDING**  
**Entry protected main:** `376301ef8b23f7c393a332845737f51b27624603`  
**Branch:** `sbc6b-b3c-native-tauri-integration-20260909`  
**Pull request:** `#14 — SBC-6B B3C: native/Tauri OCR integration`  
**Acceptance authority:** Library handover `113_SBC6B_B3C_NATIVE_TAURI_INTEGRATION_ENTRY_HANDOVER_2026-09-09.md`

## Objective

B3C proves the smallest native host boundary:

`native-approved receipt/document -> Rust-owned verified sidecar invocation -> bounded factual B2-compatible OCR result`

The webview supplies only `requestId` and `documentId`. It cannot supply an absolute filesystem path, expected hash/length, model path, executable path, URL, database path, output path, arbitrary arguments or a shell command.

## Implemented native boundary

The Tauri shell now manages a native-only OCR registry that resolves an opaque document ID to the already-approved native path plus expected SHA-256 and byte length. Rust owns:

- package-internal Windows sidecar discovery at the fixed resource-relative path `ocr/windows/shark-ocr-single/shark-ocr-single.exe`;
- fixed sidecar arguments `--input`, `--expected-sha256`, and `--expected-byte-len`;
- a restricted Windows runtime environment;
- child creation/lifecycle;
- a 15-second hard timeout and child termination;
- bounded stdout (1 MiB) and stderr (256 KiB);
- exactly-one-JSON-object parsing;
- `sbc6b.single_document.v1` schema/status validation;
- document hash/length identity validation;
- bounded factual candidates/provenance/warnings/privacy validation;
- typed unavailable/failure mapping;
- non-Windows `UnsupportedPlatform` response without claiming that the Windows ONNX sidecar runs on iOS.

OCR remains factual only. It cannot classify, post, reconcile, confirm matches, select tax treatment or perform any accounting decision.

## Frozen foundation preserved

No Cargo dependency or lockfile change is authorised or present. The following remain unchanged from the protected B3B merge:

- `workspace/Cargo.toml`;
- `workspace/Cargo.lock` — reviewed SHA-256 `3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`;
- `workspace/shark-tauri-spike/Cargo.toml`;
- Tauri configuration/capability;
- Shark foundation Cargo manifest/source and Beankeeper facade;
- SBC-5 document contract;
- B2 OCR contract;
- B3B Windows OCR sidecar.

The permanent frozen-baseline guard was evolved because its previous form byte-pinned the historical SBC-1D Tauri implementation itself. The revised guard continues to byte-pin the accounting/dependency/frontend foundation while semantically validating the current bounded native shell. Feature-specific exact-diff and runtime evidence remain mandatory.

## Exact B3C candidate paths

The final candidate is required to differ from entry main in exactly these eleven paths:

1. `ci/run_sbc6b_b3c_codemagic.sh`
2. `ci/run_sbc6b_b3c_windows.ps1`
3. `codemagic.yaml`
4. `docs/SBC6B_B3C_NATIVE_TAURI_INTEGRATION_PLAN_2026-09-09.md`
5. `research/sbc6_ocr_runtime/run_b3c_native_windows.py`
6. `scripts/check_frozen_baseline.py`
7. `scripts/check_sbc6b_b3c_native_gate.py`
8. `workspace/shark-tauri-spike/build.rs`
9. `workspace/shark-tauri-spike/permissions/shark-shell.toml`
10. `workspace/shark-tauri-spike/src/lib.rs`
11. `workspace/shark-tauri-spike/src/ocr_native.rs`

## Windows proof

Run from a fresh canonical-LF checkout of the exact candidate:

`powershell -NoProfile -ExecutionPolicy Bypass -File .\ci\run_sbc6b_b3c_windows.ps1`

The launcher must select a CPython version compatible with the already-frozen B3A/B3B packaging pins. `onnxruntime==1.23.2` has Windows wheels for CPython 3.10 through 3.13 but not CPython 3.14. The launcher therefore tries Python 3.13, 3.12, 3.11 and 3.10 via the Windows `py` launcher before considering the default `python` command. It must not upgrade ONNX Runtime merely to accommodate a newer host interpreter because that would change the previously validated OCR runtime.

The proof rebuilds the already-frozen B3B onedir sidecar with the inherited pinned runtime, verifies the four selected PP-OCRv6 Tiny model hashes, selects a frozen receipt fixture, and runs the B3C static/runtime gate.

Required Windows proofs include:

- real packaged sidecar positive OCR;
- wrong length -> integrity failure;
- wrong SHA-256 -> verified sidecar integrity failure before OCR interpretation;
- missing sidecar -> unavailable;
- non-zero sidecar -> typed failure;
- malformed JSON -> fail closed;
- wrong schema -> fail closed;
- stdout overflow -> resource-limit failure;
- stderr overflow -> resource-limit failure;
- Rust-owned timeout/kill;
- permanent foundation regressions;
- Tauri tests under `--locked`;
- Windows production Tauri compile under `--locked`;
- exact Cargo.lock and clean repository after proof.

Expected evidence archive:

`C:\SharkBooks-SBC6B-B3C\SBC6B_B3C_NATIVE_TAURI_INTEGRATION.zip`

### Windows V1 harness stop and V2 repair — 9 September 2026

The first returned B3C Windows attempt stopped before product/static/runtime validation while creating the temporary packaging environment. The launcher had invoked the user's default `python`, which was too new for the frozen `onnxruntime==1.23.2` Windows wheel set. Pip therefore reported no matching 1.23.2 distribution while showing later ONNX Runtime versions.

Classification: **HARNESS / HOST INTERPRETER COMPATIBILITY ONLY**.

No B3C Rust/Tauri product test ran, no OCR model/runtime pin changed, no Cargo/dependency/foundation state changed, and no PASS claim was made.

Bounded V2 repair: `ci/run_sbc6b_b3c_windows.ps1` now selects the newest installed compatible CPython from 3.13/3.12/3.11/3.10 and only then invokes the unchanged proof. If none is installed it fails with a specific instruction instead of allowing pip to select newer OCR dependencies.

## Apple regression proof

The repository's already-proven Codemagic route is extended with workflow:

`SBC-6B B3C Native OCR — Apple Gate`

using `ci/run_sbc6b_b3c_codemagic.sh` on `mac_mini_m2`, Xcode 26.4 and Rust 1.98.1.

It must prove:

- B3C static governance gate;
- exact reviewed Cargo.lock;
- exact Beankeeper materialisation;
- permanent foundation regressions;
- physical `aarch64-apple-ios` foundation + Tauri compile;
- Simulator `aarch64-apple-ios-sim` foundation + Tauri compile;
- repository clean and lock unchanged at exit.

Expected evidence archive:

`SBC6B_B3C_MAC_CODEMAGIC_RESULT_*.zip`

This Apple lane is a no-regression/native-shell compile proof only; it does not claim Windows PP-OCRv6/ONNX OCR is implemented on iOS.

## GitHub hosted-runner diagnostic

A temporary GitHub Actions B3C workflow was tested during candidate preparation. Repeated Ubuntu, Windows and macOS jobs ended before runner assignment with `runner_id=0` and no executed steps. Repository Actions history also contained no successful hosted Actions runs. This was classified as hosted-runner/account infrastructure rather than product-test evidence. The temporary workflow was therefore removed from the candidate instead of leaving a permanent false-red gate.

## Hard stop

B3C is **not PASS and must not be merged** until both returned Windows evidence and Apple/Codemagic evidence are adjudicated against this contract.

Do not start SBC-6C receipt-to-bank matching or any SBC-7 UI implementation until B3C has its own PASS decision and protected merge.
