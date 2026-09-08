$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$BaseHead = '4e3a47b70933cfafe27be262ac2da1ab2a63d1d9'
$BeankeeperPin = 'd573db5e61089b0922f95c991732394d08e3cf92'
$LockShaExpected = '3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9'
$FacadeShaExpected = '6274cffc89ccb2fcea4d489e76375c9c5be1e3c8cc2fbd4c4e56b37bdef0ef46'
$RustToolchain = if ($env:RUST_TOOLCHAIN) { $env:RUST_TOOLCHAIN } else { '1.98.1' }
$RepoRoot = if ($env:CM_BUILD_DIR) { $env:CM_BUILD_DIR } else { (Resolve-Path (Join-Path $PSScriptRoot '..')).Path }
$Workspace = Join-Path $RepoRoot 'workspace'
$Artifacts = Join-Path $RepoRoot 'artifacts'
$Timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$ResultDir = Join-Path $Artifacts "sbc1g_windows_$Timestamp"
$LogDir = Join-Path $ResultDir 'logs'
$ResultZip = Join-Path $Artifacts "SBC1G_WINDOWS_CODEMAGIC_RESULT_$Timestamp.zip"
$Summary = Join-Path $Artifacts 'SBC1G_WINDOWS_SUMMARY.txt'
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

$Assertions = 0
$Failures = 0
$Phase = 'PREP'
$Overall = 'FAIL'
$Classification = 'UNRESOLVED'

function Pass([string]$Message) {
    $script:Assertions++
    Add-Content -Path (Join-Path $ResultDir 'assertions.tsv') -Value "PASS`t$Message"
    Write-Host "[PASS] $Message"
}

function Fail([string]$Message) {
    $script:Assertions++
    $script:Failures++
    Add-Content -Path (Join-Path $ResultDir 'assertions.tsv') -Value "FAIL`t$Message"
    Write-Host "[FAIL] $Message"
}

function Stop-Gate([string]$Message, [string]$Code) {
    Fail $Message
    $script:Classification = $Code
    throw $Message
}

function Invoke-Logged([string]$Label, [scriptblock]$Command) {
    $Log = Join-Path $LogDir "$Label.log"
    $PreviousErrorActionPreference = $ErrorActionPreference
    $NativeExitCode = 0
    try {
        # PowerShell 5.1 surfaces normal native stderr (for example rustup/cargo
        # progress messages) as non-terminating ErrorRecord objects. Temporarily
        # allow those records through the logging pipeline, then adjudicate the
        # native process solely by its real exit code.
        $ErrorActionPreference = 'Continue'
        & $Command 2>&1 | Tee-Object -FilePath $Log
        $NativeExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
    }
    if ($NativeExitCode -ne 0) {
        throw "Command $Label failed with exit code $NativeExitCode"
    }
}

function Finalize([int]$ExitCode) {
    try {
        $LockPath = Join-Path $Workspace 'Cargo.lock'
        $LockSha = if (Test-Path $LockPath) { (Get-FileHash -Algorithm SHA256 $LockPath).Hash.ToLowerInvariant() } else { '' }
        $Head = (& git -C $RepoRoot rev-parse HEAD 2>$null)
        & git -C $RepoRoot status --short | Out-File -Encoding utf8 (Join-Path $ResultDir 'git_status_at_finish.txt')
        $Head | Out-File -Encoding ascii (Join-Path $ResultDir 'repository_commit.txt')
        $LockSha | Out-File -Encoding ascii (Join-Path $ResultDir 'cargo_lock_sha256.txt')
        @"
SBC-1G Codemagic Windows Foundation Guard
Overall: $Overall
Classification: $Classification
Phase: $Phase
Assertions: $Assertions
Failures: $Failures
Codemagic build id: $($env:CM_BUILD_ID)
Repository commit: $Head
Base SBC-1F head: $BaseHead
Beankeeper pin: $BeankeeperPin
Approved facade SHA-256: $FacadeShaExpected
Reviewed Cargo.lock SHA-256: $LockShaExpected
Observed Cargo.lock SHA-256: $LockSha
Rust toolchain: $RustToolchain
"@ | Out-File -Encoding utf8 $Summary
        Copy-Item $Summary (Join-Path $ResultDir 'SUMMARY.txt') -Force
        if (Test-Path $ResultZip) { Remove-Item $ResultZip -Force }
        Compress-Archive -Path $ResultDir -DestinationPath $ResultZip -CompressionLevel Optimal
    } catch {
        Write-Warning "Evidence finalization failed: $($_.Exception.Message)"
    }
    exit $ExitCode
}

try {
    New-Item -ItemType File -Force -Path (Join-Path $ResultDir 'assertions.tsv') | Out-Null
    $Phase = 'G0_PREFLIGHT'
    if ($env:OS -ne 'Windows_NT') { Stop-Gate 'Host is Windows' 'HOST_NOT_WINDOWS' }
    Pass 'Host is Windows'
    foreach ($tool in @('git', 'python')) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Stop-Gate "$tool available" "HOST_TOOL_MISSING_$tool" }
        Pass "$tool available"
    }

    $Dirty = (& git -C $RepoRoot status --porcelain)
    if ($Dirty) {
        $Dirty | Out-File -Encoding utf8 (Join-Path $ResultDir 'dirty_at_entry.txt')
        Stop-Gate 'Repository clean at entry' 'REPOSITORY_DIRTY_AT_ENTRY'
    }
    Pass 'Repository clean at entry'

    $Head = (& git -C $RepoRoot rev-parse HEAD).Trim()
    if ($env:CM_COMMIT -and $Head -ne $env:CM_COMMIT) { Stop-Gate 'Checked-out HEAD equals Codemagic commit' 'CODEMAGIC_COMMIT_MISMATCH' }
    Pass 'Checked-out HEAD equals Codemagic commit'
    & git -C $RepoRoot merge-base --is-ancestor $BaseHead $Head
    if ($LASTEXITCODE -ne 0) { Stop-Gate 'SBC-1F PASS head is ancestor' 'BASE_HEAD_NOT_ANCESTOR' }
    Pass 'SBC-1F PASS head is ancestor'

    $ExpectedChanged = @(
        '.github/workflows/sbc-foundation-guard.yml',
        'ci/prepare_rgba_png.py',
        'ci/run_sbc1g_codemagic.sh',
        'ci/run_sbc1g_windows.ps1',
        'codemagic.yaml',
        'docs/SBC1GH_BOUNDED_BATCH_CONTRACT_2026-09-08.json',
        'docs/SBC1G_CI_REGRESSION_AND_CHANGE_CONTROL_POLICY_2026-09-08.md',
        'scripts/check_sbc1g_change_control.py',
        'workspace/shark-foundation/tests/fixtures/gbp_statement.ofx',
        'workspace/shark-foundation/tests/fixtures/sample_receipt.txt',
        'workspace/shark-foundation/tests/foundation_regression.rs'
    ) | Sort-Object
    $ObservedChanged = @(& git -C $RepoRoot diff --name-only "$BaseHead..$Head") | Sort-Object
    $ObservedChanged | Out-File -Encoding utf8 (Join-Path $ResultDir 'observed_changed_files.txt')
    if ((Compare-Object $ExpectedChanged $ObservedChanged)) { Stop-Gate 'Candidate diff is exactly the authorised SBC-1G/H closure set' 'UNAUTHORISED_SOURCE_CHANGE' }
    Pass 'Candidate diff is exactly the authorised SBC-1G/H closure set'

    $Phase = 'G1_STATIC_GUARDS'
    Invoke-Logged 'frozen_baseline' { python (Join-Path $RepoRoot 'scripts/check_frozen_baseline.py') }
    Pass 'Frozen baseline guard passes'
    Invoke-Logged 'change_control' { python (Join-Path $RepoRoot 'scripts/check_sbc1g_change_control.py') --base-ref $BaseHead }
    Pass 'Permanent change-control guard passes'
    $LockSha = (Get-FileHash -Algorithm SHA256 (Join-Path $Workspace 'Cargo.lock')).Hash.ToLowerInvariant()
    if ($LockSha -ne $LockShaExpected) { Stop-Gate 'Native Cargo.lock remains reviewed hash' 'LOCK_DRIFT' }
    Pass 'Native Cargo.lock remains reviewed hash'

    $Phase = 'G2_RUST_AND_UPSTREAM'
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        $RustupInit = Join-Path $env:TEMP 'rustup-init.exe'
        Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $RustupInit
        & $RustupInit -y --profile minimal --default-toolchain $RustToolchain
        if ($LASTEXITCODE -ne 0) { Stop-Gate 'rustup installs on Windows host' 'RUSTUP_INSTALL_FAIL' }
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    }
    Pass 'rustup available'
    Invoke-Logged 'rust_toolchain' { rustup toolchain install $RustToolchain --profile minimal }
    Invoke-Logged 'rust_default' { rustup default $RustToolchain }
    Pass "Rust $RustToolchain selected"

    $ShortRoot = 'C:\sbc1g'
    New-Item -ItemType Directory -Force -Path $ShortRoot | Out-Null
    $env:CARGO_TARGET_DIR = Join-Path $ShortRoot 'target'
    $env:TEMP = Join-Path $ShortRoot 'tmp'
    $env:TMP = $env:TEMP
    New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
    Pass 'Short Windows Cargo/TEMP paths configured'

    Invoke-Logged 'bootstrap' { python (Join-Path $RepoRoot 'scripts/bootstrap_beankeeper.py') }
    Pass 'Exact Beankeeper source materialises'
    $UpstreamHead = (& git -C (Join-Path $RepoRoot 'upstream/beankeeper') rev-parse HEAD).Trim()
    if ($UpstreamHead -ne $BeankeeperPin) { Stop-Gate 'Beankeeper checkout equals frozen pin' 'BEANKEEPER_PIN_DRIFT' }
    Pass 'Beankeeper checkout equals frozen pin'

    $Phase = 'G3_WINDOWS_REGRESSION'
    Invoke-Logged 'metadata' { cargo metadata --manifest-path (Join-Path $Workspace 'Cargo.toml') --locked --no-deps }
    Pass 'Native workspace metadata resolves under frozen lock'
    Invoke-Logged 'foundation_tests' { cargo test --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-foundation --locked --jobs 1 }
    Pass 'Permanent Shark foundation regressions pass on Windows'
    Invoke-Logged 'windows_tauri_build' { cargo build --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-tauri-spike --locked --jobs 1 }
    Pass 'Production Tauri shell compiles on Windows under frozen lock'

    $LockShaFinal = (Get-FileHash -Algorithm SHA256 (Join-Path $Workspace 'Cargo.lock')).Hash.ToLowerInvariant()
    if ($LockShaFinal -ne $LockShaExpected) { Stop-Gate 'Native Cargo.lock unchanged through Windows proof' 'LOCK_DRIFT_AFTER_PROOF' }
    Pass 'Native Cargo.lock unchanged through Windows proof'

    $Phase = 'COMPLETE'
    $Overall = 'PASS'
    $Classification = 'PASS_SBC1G_WINDOWS_FOUNDATION_GUARD'
    Pass 'SBC-1G Windows bounded gate complete'
    Finalize 0
} catch {
    if ($Failures -eq 0) { Fail "Unhandled gate failure: $($_.Exception.Message)" }
    if ($Classification -eq 'UNRESOLVED') { $Classification = 'WINDOWS_GATE_UNRESOLVED_FAILURE' }
    Write-Error $_
    Finalize 1
}