param(
    [Parameter(Mandatory = $true)]
    [string]$ExpectedHead
)

$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Ui = Join-Path $Repo 'workspace\ui'
$Workspace = Join-Path $Repo 'workspace'
$Base = 'b4ec9674eec233facc4e75da7bdceddaab92f3c8'
$Toolchain = '1.98.1'
$Timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$ResultDir = Join-Path $env:TEMP "SBC8A4_PDFMAKE_WINDOWS_$Timestamp"
$LogDir = Join-Path $ResultDir 'logs'
$GateDir = Join-Path $ResultDir 'gates'
New-Item -ItemType Directory -Force -Path $ResultDir,$LogDir,$GateDir | Out-Null

function Run-Logged {
    param([string]$Label,[scriptblock]$Command)
    $log = Join-Path $LogDir "$Label.log"
    $previous = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $Command 2>&1 | Tee-Object -FilePath $log
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previous
    }
    if ($exitCode -ne 0) { throw "$Label failed with exit code $exitCode" }
}

function Write-Gate {
    param([string]$Gate,[string]$Status,[string]$Summary)
    [ordered]@{ gate=$Gate; status=$Status; summary=$Summary; candidate_sha=$script:Head } |
        ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $GateDir "$Gate.json")
}

function Assert-AllowedPaths {
    $exact = @(
        'workspace/ui/package.json',
        'workspace/ui/package-lock.json',
        'workspace/ui/src/lib/commercialDocumentRenderer.ts',
        'workspace/shark-tauri-spike/src/owner_commercial_renderer.rs',
        'workspace/ui/src/lib/__tests__/commercialDocumentRenderer.spec.ts',
        'workspace/ui/src/lib/tauri.ts',
        'workspace/ui/src/screens/QuotesScreen.vue',
        'workspace/ui/src/screens/InvoicesScreen.vue',
        'workspace/ui/src/screens/ReportsScreen.vue',
        'workspace/shark-tauri-spike/src/lib.rs',
        'workspace/shark-tauri-spike/src/owner_documents_ocr.rs',
        'workspace/shark-tauri-spike/src/owner_supporting_data.rs',
        'ci/run_sbc8a4_pdfmake_integration_windows.ps1',
        'ci/run_sbc8a4_pdfmake_integration_codemagic.sh',
        'codemagic.yaml'
    )
    $prefix = @('workspace/ui/src/fixtures/sbc8a4/','workspace/shark-tauri-spike/tests/sbc8a4/')
    $changed = @(& git -C $Repo diff --name-only "$Base..HEAD")
    $forbidden = @()
    foreach ($p in $changed) {
        if (($exact -notcontains $p) -and -not @($prefix | Where-Object { $p.StartsWith($_) }).Count) {
            $forbidden += $p
        }
    }
    if ($forbidden.Count) { throw "Changed path outside frozen allow-list: $($forbidden -join ', ')" }
    $changed | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'CHANGED_PATHS.txt')
}

$script:Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($script:Head -ne $ExpectedHead) { throw "Expected $ExpectedHead, observed $script:Head" }
if (@(& git -C $Repo status --porcelain).Count -ne 0) { throw 'Repository must be clean at proof entry.' }
& git -C $Repo merge-base --is-ancestor $Base $script:Head
if ($LASTEXITCODE -ne 0) { throw 'Frozen protected-main base is not an ancestor of candidate.' }
Assert-AllowedPaths
Run-Logged 'bootstrap_beankeeper' { python -B (Join-Path $Repo 'scripts\bootstrap_beankeeper.py') }
Run-Logged 'rust_version' { rustup run $Toolchain rustc --version }
Write-Gate 'G0_PREFLIGHT' 'PASS' 'Exact candidate, frozen base ancestry, clean tree and changed-path allow-list verified.'

$package = Get-Content (Join-Path $Ui 'package.json') -Raw | ConvertFrom-Json
if ($package.dependencies.pdfmake -ne '0.3.11') { throw 'pdfmake is not pinned exactly to 0.3.11.' }
Run-Logged 'npm_ci' { npm.cmd --prefix $Ui ci }
$auditPath = Join-Path $ResultDir 'npm-audit.json'
& npm.cmd --prefix $Ui audit --json | Set-Content -Encoding UTF8 $auditPath
if ($LASTEXITCODE -ne 0) { throw 'npm audit returned non-zero.' }
$audit = Get-Content $auditPath -Raw | ConvertFrom-Json
if ([int]$audit.metadata.vulnerabilities.total -ne 0) { throw 'npm audit total vulnerabilities is not zero.' }
Write-Gate 'G1_DEPENDENCY_SECURITY' 'PASS' 'pdfmake 0.3.11 exact pin and fresh zero-vulnerability npm audit verified.'

Run-Logged 'ui_typecheck' { npm.cmd --prefix $Ui run type-check }
Run-Logged 'renderer_determinism' { npm.cmd --prefix $Ui run test:sbc8a4 }
Run-Logged 'ui_build' { npm.cmd --prefix $Ui run build }
if (Test-Path (Join-Path $Repo 'workspace\dist')) {
    & git -C $Repo restore --source=HEAD --worktree --staged -- workspace/dist
    & git -C $Repo clean -fd -- workspace/dist
}
Write-Gate 'G2_FRONTEND_RENDERER' 'PASS' 'Type-check, deterministic renderer fixture and production build pass.'

Run-Logged 'rust_bounded_renderer' {
    rustup run $Toolchain cargo test --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-tauri-spike --locked owner_commercial_renderer -- --nocapture
}
$ProofBooks = Join-Path $ResultDir 'sbc1d-proof-books'
New-Item -ItemType Directory -Force -Path $ProofBooks | Out-Null
$HadProofKey = Test-Path Env:SHARK_SBC1D_PROOF_KEY
$PreviousProofKey = $env:SHARK_SBC1D_PROOF_KEY
$HadProofBooks = Test-Path Env:SHARK_SBC1D_BOOKS_DIR
$PreviousProofBooks = $env:SHARK_SBC1D_BOOKS_DIR
try {
    $env:SHARK_SBC1D_PROOF_KEY = 'SBC8A4-PDFMake-Windows-Proof-Key-Only-Do-Not-Ship'
    $env:SHARK_SBC1D_BOOKS_DIR = $ProofBooks
    Run-Logged 'rust_tauri_regressions' {
        rustup run $Toolchain cargo test --manifest-path (Join-Path $Workspace 'Cargo.toml') -p shark-tauri-spike --locked --jobs 1 -- --test-threads=1 --skip ocr_native::tests::windows_malformed_and_wrong_schema_fail_closed --skip ocr_native::tests::windows_missing_sidecar_is_unavailable --skip ocr_native::tests::windows_nonzero_sidecar_maps_to_typed_engine_failure --skip ocr_native::tests::windows_real_packaged_sidecar_returns_b2_compatible_facts --skip ocr_native::tests::windows_stdout_and_stderr_limits_fail_closed --skip ocr_native::tests::windows_timeout_is_owned_by_rust_and_child_is_terminated --skip ocr_native::tests::windows_wrong_hash_is_rejected_by_verified_sidecar_before_ocr --skip ocr_native::tests::windows_wrong_length_fails_before_sidecar_execution
    }
} finally {
    if ($HadProofKey) { $env:SHARK_SBC1D_PROOF_KEY = $PreviousProofKey } else { Remove-Item Env:SHARK_SBC1D_PROOF_KEY -ErrorAction SilentlyContinue }
    if ($HadProofBooks) { $env:SHARK_SBC1D_BOOKS_DIR = $PreviousProofBooks } else { Remove-Item Env:SHARK_SBC1D_BOOKS_DIR -ErrorAction SilentlyContinue }
}
Run-Logged 'rust_workspace_check' {
    rustup run $Toolchain cargo check --manifest-path (Join-Path $Workspace 'Cargo.toml') --workspace --locked --jobs 1
}
Write-Gate 'G3_NATIVE_BOUNDARY' 'PASS' 'Bounded PDF save/store suite, inherited Tauri regressions and locked workspace check pass.'

$renderer = Get-Content (Join-Path $Ui 'src\lib\commercialDocumentRenderer.ts') -Raw
if ($renderer -match 'https?://' -or $renderer -match 'fetch\s*\(' -or $renderer -match 'XMLHttpRequest|WebSocket|EventSource') {
    throw 'Production renderer contains a prohibited remote-resource/network reference.'
}
if ($renderer -notmatch 'pdfmake/build/pdfmake\.js' -or $renderer -notmatch 'pdfmake/build/vfs_fonts\.js') {
    throw 'Production renderer is not using admitted local pdfmake/VFS bundles.'
}
Write-Gate 'G4_OFFLINE_BOUNDARY' 'PASS' 'Production renderer has no remote resource/network reference and uses local admitted bundles.'

$HeadEnd = (& git -C $Repo rev-parse HEAD).Trim()
if ($HeadEnd -ne $script:Head) { throw 'Candidate SHA moved during proof.' }
$status = ((& git -C $Repo status --porcelain) -join [Environment]::NewLine).Trim()
if ($status) { throw "Repository dirty after proof: $status" }
Assert-AllowedPaths
Write-Gate 'G5_FINAL_INTEGRITY' 'PASS' 'Exact candidate SHA and clean repository preserved.'
[ordered]@{
    schema='sbc8a4-pdfmake-production-integration-windows-v1'
    overall='PASS'
    candidate_sha=$script:Head
    base_sha=$Base
    pdfmake='0.3.11'
    npm_audit_total=0
    platform='windows'
    completed_at_utc=[DateTime]::UtcNow.ToString('o')
} | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $ResultDir 'SUMMARY.json')
Write-Host "[PASS] SBC8A4 PDFMake exact-candidate Windows proof: $ResultDir"
