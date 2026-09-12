$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc7b1_bank_mutation_persistence.py'
$OutRoot = Join-Path $env:TEMP 'SharkBooks-SBC7B1-BankMutation'
$CargoTarget = Join-Path $env:TEMP 'SharkBooks-SBC7B1-BankMutation-CargoTarget'
$ZipName = 'SBC7B1_BANK_MUTATION_PERSISTENCE.zip'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads $ZipName
$TmpZip = Join-Path $env:TEMP ('tmp_' + $ZipName)
$Base = '4b3eee65bcef24c3ee7d6c06f404aa4180a7c4d8'
$ExpectedLock = '8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61'

if ($env:OS -ne 'Windows_NT') { throw 'SBC-7B1 Bank mutation proof must run on Windows.' }
if (Test-Path $OutRoot) { Remove-Item $OutRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null
New-Item -ItemType Directory -Force -Path $Downloads | Out-Null
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
if (Test-Path $CargoTarget) { Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue }
$env:CARGO_TARGET_DIR = $CargoTarget

$Python = $null
$PythonPrefix = @()
$PythonCommand = Get-Command python -ErrorAction SilentlyContinue
if ($null -ne $PythonCommand) {
    $Python = $PythonCommand.Source
} else {
    $PyLauncher = Get-Command py -ErrorAction SilentlyContinue
    if ($null -ne $PyLauncher) {
        $Python = $PyLauncher.Source
        $PythonPrefix = @('-3')
    }
}
if ($null -eq $Python) { throw 'Python 3 is required for the SBC-7B1 Bank mutation validator.' }
if ($null -eq (Get-Command rustup -ErrorAction SilentlyContinue)) { throw 'rustup is required for SBC-7B1 Bank mutation proof.' }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-7B1 Bank mutation proof repository: $Repo"
Write-Host "[INFO] Cargo target isolated outside repository: $CargoTarget"
Write-Host '[INFO] Running bounded Bank mutation/persistence gate'

$ValidatorArgs = @($PythonPrefix + @('-B', $Validator))
$ValidatorProcess = Start-Process `
    -FilePath $Python `
    -ArgumentList $ValidatorArgs `
    -WorkingDirectory $Repo `
    -RedirectStandardOutput $Stdout `
    -RedirectStandardError $Stderr `
    -NoNewWindow `
    -Wait `
    -PassThru
$ValidatorExit = $ValidatorProcess.ExitCode

if (Test-Path $Stdout) { Get-Content $Stdout | ForEach-Object { Write-Host $_ } }
if ((Test-Path $Stderr) -and (Get-Item $Stderr).Length -gt 0) {
    Get-Content $Stderr | ForEach-Object { Write-Warning $_ }
}
if ($ValidatorExit -ne 0) { throw "SBC-7B1 Bank mutation validator failed with exit code $ValidatorExit" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after Bank mutation proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after Bank mutation proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-7B1 Bank mutation proof: $Status" }

$NativeLockHash = (Get-FileHash -Algorithm SHA256 (Join-Path $Repo 'workspace\Cargo.lock')).Hash.ToLowerInvariant()
if ($NativeLockHash -ne $ExpectedLock) { throw "Native Cargo.lock drift: $NativeLockHash" }
$ProductTree = (& git -C $Repo rev-parse 'HEAD:product/shark-books-core').Trim()
$FoundationTree = (& git -C $Repo rev-parse 'HEAD:workspace/shark-foundation').Trim()
$OcrBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/ocr_native.rs').Trim()
$OwnerAppBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_app.rs').Trim()
$BankReviewBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_bank_review.rs').Trim()
$FrontendTree = (& git -C $Repo rev-parse 'HEAD:workspace/dist').Trim()

$Result = [ordered]@{
    schema = 'sbc7b1-bank-mutation-persistence-windows-v1'
    gate = 'SBC-7B1-BANK-MUTATION'
    gate_pass = $true
    entry_protected_main = $Base
    git_head = $Head
    git_status = $Status
    native_cargo_lock_sha256 = $NativeLockHash
    product_core_tree = $ProductTree
    foundation_tree = $FoundationTree
    ocr_native_blob = $OcrBlob
    prior_owner_app_blob = $OwnerAppBlob
    prior_bank_review_blob = $BankReviewBlob
    frontend_tree = $FrontendTree
    proofs = [ordered]@{
        exact_changed_path_allowlist = $true
        product_core_unchanged = $true
        cargo_manifests_unchanged = $true
        native_lock_unchanged = $true
        ocr_native_unchanged = $true
        prior_owner_app_unchanged = $true
        prior_bank_review_unchanged = $true
        frontend_unwired = $true
        shark_application_schema_v2 = $true
        beankeeper_schema8_unchanged = $true
        preopen_backup_before_application_migration = $true
        immutable_bank_activity_persistence = $true
        strong_and_file_exact_duplicates_explicit = $true
        heuristic_not_suppression_authority = $true
        bank_activity_does_not_post_ledger = $true
        typed_bank_mutation_commands_registered = $true
        confirmation_reparses_current_statement = $true
        confirmation_bound_to_statement_and_preview = $true
        no_webview_os_path = $true
        authoritative_match_rerun = $true
        audited_uncleared_to_cleared_only = $true
        authoritative_reconciliation_rerun = $true
        exact_zero_all_cleared_required = $true
        audited_cleared_to_reconciled_only = $true
        batch_atomicity = $true
        prior_owner_bridge_regressions = $true
        prior_bank_review_regressions = $true
        foundation_regressions = $true
        product_core_regressions = $true
        bank_mutation_bridge_tests = $true
        locked_windows_compile = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC7B1_BANK_MUTATION_PERSISTENCE_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")
(& git -C $Repo diff --name-status "$Base..HEAD") | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'CHANGED_PATHS.txt')

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutRoot, $TmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force
Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[PASS] SBC-7B1 Bank mutation evidence package written: $ZipPath"
Write-Host '[PASS] SBC-7B1 bounded Bank mutation/persistence Windows proof complete'
