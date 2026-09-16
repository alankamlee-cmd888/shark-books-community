param(
    [Parameter(Mandatory = $true)]
    [string]$ExpectedHead
)

$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Workspace = Join-Path $Repo 'workspace'
$Base = '99c3b0d16fe4934f1b397a58956df745203f744f'
$Toolchain = '1.98.1'
$Timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$ResultDir = Join-Path $env:TEMP "SBC7B2_SUPERGATE_WINDOWS_$Timestamp"
$GateDir = Join-Path $ResultDir 'gates'
$LogDir = Join-Path $ResultDir 'logs'
$CargoTarget = Join-Path $env:USERPROFILE 'sbc7b2-supergate-cargo-target'
$ProofBooks = Join-Path $env:TEMP 'sharkbooks-sbc7b2-ft1ft2-proof-books'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads "SBC7B2_FT1_FT2_SUPERGATE_WINDOWS_$Timestamp.zip"
$HadProofKey = Test-Path Env:SHARK_SBC1D_PROOF_KEY
$PreviousProofKey = $env:SHARK_SBC1D_PROOF_KEY
$HadProofBooks = Test-Path Env:SHARK_SBC1D_BOOKS_DIR
$PreviousProofBooks = $env:SHARK_SBC1D_BOOKS_DIR

function Write-GateResult {
    param([string]$Gate, [string]$Status, [string]$Summary, [bool]$Mandatory = $true)
    $obj = [ordered]@{
        gate = $Gate
        status = $Status
        mandatory = $Mandatory
        summary = $Summary
        candidate_sha = $script:Head
    }
    $obj | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $GateDir "$Gate.json")
}

function Run-Logged {
    param([string]$Label, [scriptblock]$Command)
    $log = Join-Path $LogDir "$Label.log"
    # Windows PowerShell surfaces native stderr as non-terminating ErrorRecords when
    # redirected. Cargo writes ordinary compile progress to stderr, so judge native
    # command success by its exit code while still capturing both streams.
    $PreviousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $Command 2>&1 | Tee-Object -FilePath $log
        $ExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
    }
    if ($ExitCode -ne 0) {
        throw "$Label failed with exit code $ExitCode"
    }
}

if ($env:OS -ne 'Windows_NT') { throw 'SBC-7B2 Windows Super-Gate must run on Windows.' }
New-Item -ItemType Directory -Force -Path $ResultDir, $GateDir, $LogDir, $Downloads | Out-Null
if (Test-Path $CargoTarget) { Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue }
if (Test-Path $ProofBooks) { Remove-Item $ProofBooks -Recurse -Force -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $CargoTarget, $ProofBooks | Out-Null
$env:CARGO_TARGET_DIR = $CargoTarget
$env:SHARK_SBC1D_PROOF_KEY = 'SBC7B2-FT1FT2-Proof-Key-Only-Do-Not-Ship'
$env:SHARK_SBC1D_BOOKS_DIR = $ProofBooks
$script:Head = (& git -C $Repo rev-parse HEAD).Trim()

try {
    # SG0 — immutable candidate preflight
    if ($script:Head -ne $ExpectedHead) { throw "Expected candidate $ExpectedHead but checkout is $script:Head" }
    if (@(& git -C $Repo status --porcelain).Count -ne 0) { throw 'Repository must be clean at Super-Gate entry.' }
    & git -C $Repo merge-base --is-ancestor $Base $script:Head
    if ($LASTEXITCODE -ne 0) { throw 'Protected post-7B1 main is not an ancestor of candidate.' }
    foreach ($tool in @('git','python','rustup')) {
        if ($null -eq (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "$tool is required." }
    }
    Run-Logged 'sg0_bootstrap_beankeeper' { python -B (Join-Path $Repo 'scripts\bootstrap_beankeeper.py') }
    Run-Logged 'sg0_rust_version' { rustup run $Toolchain rustc --version }
    $LockPath = Join-Path $Workspace 'Cargo.lock'
    $LockShaStart = (Get-FileHash -Algorithm SHA256 $LockPath).Hash.ToLowerInvariant()
    $LockBlobExpected = (& git -C $Repo rev-parse 'HEAD:workspace/Cargo.lock').Trim()
    $LockBlobObserved = (& git -C $Repo hash-object $LockPath).Trim()
    if ($LockBlobObserved -ne $LockBlobExpected) { throw 'Working Cargo.lock bytes do not equal candidate Cargo.lock blob.' }
    (& git -C $Repo diff --name-only "$Base..HEAD") | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'CHANGED_PATHS.txt')
    Set-Content -Encoding ASCII (Join-Path $ResultDir 'CANDIDATE_SHA.txt') ($script:Head + "`n")
    Set-Content -Encoding ASCII (Join-Path $ResultDir 'EXPECTED_SHA.txt') ($ExpectedHead + "`n")
    Set-Content -Encoding ASCII (Join-Path $ResultDir 'BASE_SHA.txt') ($Base + "`n")
    Write-GateResult 'SG0_PREFLIGHT' 'PASS' 'Exact candidate, clean tree, toolchain/bootstrap and candidate lock identity verified.'

    # SG1 — Action Stack / generator contract
    Run-Logged 'sg1_static_gate' { python -B (Join-Path $Repo 'scripts\check_sbc7b2_ft1_ft2.py') --repo $Repo }
    Run-Logged 'sg1_generated_contract_check' {
        rustup run $Toolchain cargo run --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-foundation --example action-contract --features action-contract-gen --locked -- --check
    }
    Run-Logged 'sg1_dependency_tree' {
        rustup run $Toolchain cargo tree --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-foundation --features action-contract-gen --locked
    }
    $TreeText = Get-Content (Join-Path $LogDir 'sg1_dependency_tree.log') -Raw
    if ($TreeText -notmatch 'schemars v1\.2\.2' -or $TreeText -notmatch 'ts-rs v12\.0\.1') {
        throw 'Exact admitted Schemars/ts-rs versions absent from locked dependency tree.'
    }
    Write-GateResult 'SG1_ACTION_STACK' 'PASS' '206/175/700 registry invariants, generated contracts and exact reuse pins verified.'

    # SG2 — deterministic controller / Attention Queue
    Run-Logged 'sg2_action_system_contract' {
        rustup run $Toolchain cargo test --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-foundation --test action_system_contract --locked --jobs 1 -- --test-threads=1
    }
    Write-GateResult 'SG2_CONTROLLER' 'PASS' 'Deterministic resolution, clarification, confirmation, stale-state, replay and Attention tests pass.'

    # SG3/SG4 are later FT3 UI groups and are not claimed by this FT1/FT2 candidate.
    Write-GateResult 'SG3_UI_A' 'NOT_APPLICABLE_TO_THIS_CANDIDATE' 'FT3A UI is outside the FT1/FT2 candidate.' $false
    Write-GateResult 'SG4_UI_B' 'NOT_APPLICABLE_TO_THIS_CANDIDATE' 'FT3B UI is outside the FT1/FT2 candidate.' $false

    # SG5 — finite command/voice parity metadata (no speech runtime)
    Run-Logged 'sg5_voice_registry_recheck' { python -B (Join-Path $Repo 'scripts\check_sbc7b2_ft1_ft2.py') --repo $Repo }
    Write-GateResult 'SG5_COMMAND_VOICE' 'PASS' '175 canonical voice actions, 700 fixtures, aliases, collisions and locked-action fail-closed semantics verified.'

    # SG6 — inherited regressions affected by public Foundation export and lock update
    Run-Logged 'sg6_product_core' {
        rustup run $Toolchain cargo test --manifest-path (Join-Path $Repo 'product\shark-books-core\Cargo.toml') --locked --jobs 1
    }
    Run-Logged 'sg6_foundation' {
        rustup run $Toolchain cargo test --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-foundation --locked --jobs 1 -- --test-threads=1
    }

    # The eight Windows B3C packaged-runtime tests require the standalone B3C harness
    # to rebuild real/fake OCR sidecars and receipt fixtures. FT1/FT2 does not change
    # those runtime paths, so prove that boundary is unchanged before excluding only
    # those eight already-proven packaged-runtime cases from this inherited SG6 sweep.
    $B3cInvariantPaths = @(
        'workspace/shark-tauri-spike/Cargo.toml',
        'workspace/shark-tauri-spike/build.rs',
        'workspace/shark-tauri-spike/src/ocr_native.rs',
        'workspace/shark-tauri-spike/permissions/shark-shell.toml',
        'product/ocr-runtime/windows/shark_ocr_single.py',
        'research/sbc6_ocr_runtime/run_b3c_native_windows.py',
        'scripts/check_sbc6b_b3c_native_gate.py'
    )
    $B3cChanged = @(& git -C $Repo diff --name-only "$Base..HEAD" -- $B3cInvariantPaths)
    if ($B3cChanged.Count -ne 0) {
        throw "B3C packaged-runtime exclusion is invalid because inherited OCR paths changed: $($B3cChanged -join ', ')"
    }

    Run-Logged 'sg6_tauri' {
        rustup run $Toolchain cargo test --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-tauri-spike --locked --jobs 1 -- `
            --test-threads=1 `
            --skip ocr_native::tests::windows_malformed_and_wrong_schema_fail_closed `
            --skip ocr_native::tests::windows_missing_sidecar_is_unavailable `
            --skip ocr_native::tests::windows_nonzero_sidecar_maps_to_typed_engine_failure `
            --skip ocr_native::tests::windows_real_packaged_sidecar_returns_b2_compatible_facts `
            --skip ocr_native::tests::windows_stdout_and_stderr_limits_fail_closed `
            --skip ocr_native::tests::windows_timeout_is_owned_by_rust_and_child_is_terminated `
            --skip ocr_native::tests::windows_wrong_hash_is_rejected_by_verified_sidecar_before_ocr `
            --skip ocr_native::tests::windows_wrong_length_fails_before_sidecar_execution
    }
    Write-GateResult 'SG6_REGRESSIONS' 'PASS' 'Product core, Foundation and inherited Tauri owner-bridge regressions pass; eight unchanged B3C packaged-runtime-only Windows tests remain covered by their dedicated prior harness.'

    # SG7 — Windows platform compile/check
    Run-Logged 'sg7_workspace_check' {
        rustup run $Toolchain cargo check --manifest-path (Join-Path $Workspace 'Cargo.toml') --workspace --locked --jobs 1
    }
    Write-GateResult 'SG7_PLATFORM_COMPILE' 'PASS' 'Locked Windows workspace compile/check passes.'

    # SG8 — final integrity
    $HeadEnd = (& git -C $Repo rev-parse HEAD).Trim()
    if ($HeadEnd -ne $script:Head -or $HeadEnd -ne $ExpectedHead) { throw 'Candidate SHA moved during Windows Super-Gate.' }
    $LockShaEnd = (Get-FileHash -Algorithm SHA256 $LockPath).Hash.ToLowerInvariant()
    if ($LockShaEnd -ne $LockShaStart) { throw 'Cargo.lock changed during Windows Super-Gate.' }
    $LockBlobEnd = (& git -C $Repo hash-object $LockPath).Trim()
    if ($LockBlobEnd -ne $LockBlobExpected) { throw 'Cargo.lock no longer equals candidate blob at exit.' }
    $Status = ((& git -C $Repo status --porcelain) -join "`n").Trim()
    if ($Status) { throw "Repository dirty after Windows Super-Gate: $Status" }
    Write-GateResult 'SG8_FINAL_INTEGRITY' 'PASS' 'Candidate SHA, Cargo.lock and repository cleanliness preserved through proof.'

    $LockRecord = [ordered]@{
        workspace_cargo_lock_sha256 = $LockShaEnd
        workspace_cargo_lock_git_blob = $LockBlobExpected
    }
    $LockRecord | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'LOCK_HASHES.json')
    [ordered]@{ rust_toolchain = $Toolchain; platform = 'windows'; cargo_target_dir = $CargoTarget } |
        ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'TOOLCHAIN.json')

    $Summary = [ordered]@{
        schema = 'sbc7b2-ft1-ft2-supergate-windows-v1'
        overall = 'PASS'
        candidate_sha = $script:Head
        expected_sha = $ExpectedHead
        base_sha = $Base
        mandatory_gates = @('SG0_PREFLIGHT','SG1_ACTION_STACK','SG2_CONTROLLER','SG5_COMMAND_VOICE','SG6_REGRESSIONS','SG7_PLATFORM_COMPILE','SG8_FINAL_INTEGRITY')
        non_applicable_gates = @('SG3_UI_A','SG4_UI_B')
        cargo_lock_sha256 = $LockShaEnd
        rust_toolchain = $Toolchain
    }
    $Summary | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'SUMMARY.json')
    @(
        'SBC-7B2 FT1/FT2 Windows Super-Gate',
        'Overall: PASS',
        "Candidate: $script:Head",
        "Cargo.lock SHA-256: $LockShaEnd",
        'SG3/SG4: NOT_APPLICABLE_TO_THIS_CANDIDATE'
    ) | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'SUMMARY.txt')

    if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
    Compress-Archive -Path (Join-Path $ResultDir '*') -DestinationPath $ZipPath -Force
    Write-Host "[PASS] SBC-7B2 FT1/FT2 Windows Super-Gate evidence: $ZipPath"
}
finally {
    Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $ProofBooks -Recurse -Force -ErrorAction SilentlyContinue
    if ($HadProofKey) {
        $env:SHARK_SBC1D_PROOF_KEY = $PreviousProofKey
    } else {
        Remove-Item Env:SHARK_SBC1D_PROOF_KEY -ErrorAction SilentlyContinue
    }
    if ($HadProofBooks) {
        $env:SHARK_SBC1D_BOOKS_DIR = $PreviousProofBooks
    } else {
        Remove-Item Env:SHARK_SBC1D_BOOKS_DIR -ErrorAction SilentlyContinue
    }
}
