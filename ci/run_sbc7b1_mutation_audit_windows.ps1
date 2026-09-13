$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc7b1_mutation_audit.py'
$OutRoot = Join-Path $env:TEMP 'SharkBooks-SBC7B1-MutationAudit'
$CargoTarget = Join-Path $env:USERPROFILE 'sbc7b1-mutation-audit-proof-target'
$ZipName = 'SBC7B1_MUTATION_AUDIT_WINDOWS.zip'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads $ZipName
$TmpZip = Join-Path $env:TEMP ('tmp_' + $ZipName)
$Base = '8f55eaf67b311a7c2d69ebea86ae702e4934a56c'
$ExpectedLock = '10fe1431f26cef2d427e76658690544027e955845faabfbbdaebf2a2ad17e115'

if ($env:OS -ne 'Windows_NT') { throw 'SBC-7B1 Mutation + Audit proof must run on Windows.' }
if (Test-Path $OutRoot) { Remove-Item $OutRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null
New-Item -ItemType Directory -Force -Path $Downloads | Out-Null
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
if (Test-Path $CargoTarget) { Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $CargoTarget | Out-Null
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
if ($null -eq $Python) { throw 'Python 3 is required for the SBC-7B1 Mutation + Audit validator.' }
if ($null -eq (Get-Command rustup -ErrorAction SilentlyContinue)) { throw 'rustup is required for SBC-7B1 Mutation + Audit proof.' }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-7B1 Mutation + Audit proof repository: $Repo"
Write-Host "[INFO] Stable Cargo target outside TEMP: $CargoTarget"
Write-Host '[INFO] Running bounded Mutation + Audit gate'

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
if ($ValidatorExit -ne 0) { throw "SBC-7B1 Mutation + Audit validator failed with exit code $ValidatorExit" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after Mutation + Audit proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after Mutation + Audit proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-7B1 Mutation + Audit proof: $Status" }

$NativeLockHash = (Get-FileHash -Algorithm SHA256 (Join-Path $Repo 'workspace\Cargo.lock')).Hash.ToLowerInvariant()
if ($NativeLockHash -ne $ExpectedLock) { throw "Native Cargo.lock drift: $NativeLockHash" }
$ProductTree = (& git -C $Repo rev-parse 'HEAD:product/shark-books-core').Trim()
$OcrBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/ocr_native.rs').Trim()
$OwnerAppBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_app.rs').Trim()
$BankReviewBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_bank_review.rs').Trim()
$BankMutationBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_bank_mutation.rs').Trim()
$DocumentsOcrBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_documents_ocr.rs').Trim()
$FrontendTree = (& git -C $Repo rev-parse 'HEAD:workspace/dist').Trim()

$Result = [ordered]@{
    schema = 'sbc7b1-mutation-audit-windows-v1'
    gate = 'SBC-7B1-MUTATION-AUDIT'
    gate_pass = $true
    entry_protected_main = $Base
    git_head = $Head
    git_status = $Status
    native_cargo_lock_sha256 = $NativeLockHash
    product_core_tree = $ProductTree
    ocr_native_blob = $OcrBlob
    prior_owner_app_blob = $OwnerAppBlob
    prior_bank_review_blob = $BankReviewBlob
    prior_bank_mutation_blob = $BankMutationBlob
    prior_documents_ocr_blob = $DocumentsOcrBlob
    frontend_tree = $FrontendTree
    proofs = [ordered]@{
        exact_changed_path_allowlist = $true
        product_core_unchanged = $true
        native_lock_frozen = $true
        no_new_dependency = $true
        shark_application_schema_v4 = $true
        beankeeper_schema8_unchanged = $true
        preopen_backup_before_application_migration = $true
        receipt_decision_append_only = $true
        receipt_decision_exact_replay_idempotent = $true
        receipt_decision_conflict_fails_closed = $true
        receipt_confirm_revalidates_bank_identity = $true
        receipt_confirmation_does_not_post_or_reconcile = $true
        receipt_registry_native_only_and_bounded = $true
        factual_ocr_contract_reused = $true
        correction_plan_contract_reused = $true
        correction_original_never_rewritten = $true
        correction_reversal_and_replacement_atomic = $true
        correction_failure_rolls_back = $true
        correction_history_immutable_bounded = $true
        correction_preview_confirmation_bound = $true
        owner_dto_raw_authority_rejected = $true
        prior_owner_bridge_regressions = $true
        prior_bank_review_regressions = $true
        prior_bank_mutation_regressions = $true
        prior_documents_ocr_regressions = $true
        portable_ocr_regressions = $true
        foundation_regressions = $true
        product_core_regressions = $true
        frontend_unwired = $true
        exact_tauri_acl_csp = $true
        locked_windows_compile = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC7B1_MUTATION_AUDIT_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding ASCII (Join-Path $OutRoot 'CARGO_LOCK_SHA256.txt') ($NativeLockHash + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")
(& git -C $Repo diff --name-status "$Base..HEAD") | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'CHANGED_PATHS.txt')

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutRoot, $TmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force
Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[PASS] SBC-7B1 Mutation + Audit evidence package written: $ZipPath"
Write-Host '[PASS] SBC-7B1 bounded Mutation + Audit Windows proof complete'
