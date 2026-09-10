# SBC-6C Windows V1 PowerShell stderr harness repair

**Date:** 10 September 2026

## Classification
HARNESS / WINDOWS POWERSHELL 5.1 STDERR HANDLING. Not a Shark Books product-core or Rust logic failure.

## Observed stop
The first SBC-6C Windows evidence run stopped at the outer PowerShell wrapper while invoking the Python validator. Cargo emitted normal compilation progress to stderr through the validator. Because the wrapper used `$ErrorActionPreference = 'Stop'` with native stderr redirection, Windows PowerShell surfaced the stderr stream as `NativeCommandError` and terminated the wrapper before the wrapper could inspect the validator process exit code.

Observed line included `Compiling shark-books-core v0.0.1` and did not establish a Rust compilation failure.

## Bounded repair
Only `ci/run_sbc6c_windows.ps1` changed. The validator invocation now uses `Start-Process -Wait -PassThru` with explicit `-RedirectStandardOutput` and `-RedirectStandardError`. The wrapper checks the real child `ExitCode` after process completion. Normal Cargo stderr is therefore preserved as evidence without being treated as a PowerShell terminating error; any non-zero validator exit still fails the gate.

No product logic, Cargo manifest, Cargo lockfile, native/Tauri code, OCR contract, matching logic, dependency graph or authorised five-path stage boundary changed.

## Repaired candidate
`2c98dc2e58a21bfd907fd6473b7d300d579096ad`

PR #15 remains draft/unmerged. Windows proof must be rerun at this exact candidate before SBC-6C PASS or merge.
