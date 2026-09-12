$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc7b1_documents_ocr.py'
$OutRoot = Join-Path $env:TEMP 'SharkBooks-SBC7B1-DocumentsOCR'
$CargoTarget = Join-Path $env:USERPROFILE 'sbc7b1-docsocr-proof-target'
$ZipName = 'SBC7B1_DOCUMENTS_OCR_WINDOWS.zip'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads $ZipName
$TmpZip = Join-Path $env:TEMP ('tmp_' + $ZipName)
$Base = '50c898c7b8042e11810933e3e596fc124085e831'
$ExpectedLock = '10fe1431f26cef2d427e76658690544027e955845faabfbbdaebf2a2ad17e115'

if ($env:OS -ne 'Windows_NT') { throw 'SBC-7B1 Documents/OCR proof must run on Windows.' }
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
if ($null -eq $Python) { throw 'Python 3 is required for the SBC-7B1 Documents/OCR validator.' }
if ($null -eq (Get-Command rustup -ErrorAction SilentlyContinue)) { throw 'rustup is required for SBC-7B1 Documents/OCR proof.' }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-7B1 Documents/OCR proof repository: $Repo"
Write-Host "[INFO] Stable Cargo target outside TEMP: $CargoTarget"
Write-Host '[INFO] Running bounded Documents + factual OCR gate'

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
if ($ValidatorExit -ne 0) { throw "SBC-7B1 Documents/OCR validator failed with exit code $ValidatorExit" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after Documents/OCR proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after Documents/OCR proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-7B1 Documents/OCR proof: $Status" }

$NativeLockHash = (Get-FileHash -Algorithm SHA256 (Join-Path $Repo 'workspace\Cargo.lock')).Hash.ToLowerInvariant()
if ($NativeLockHash -ne $ExpectedLock) { throw "Native Cargo.lock drift: $NativeLockHash" }
$ProductTree = (& git -C $Repo rev-parse 'HEAD:product/shark-books-core').Trim()
$OcrBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/ocr_native.rs').Trim()
$OwnerAppBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_app.rs').Trim()
$BankReviewBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_bank_review.rs').Trim()
$BankMutationBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/owner_bank_mutation.rs').Trim()
$FrontendTree = (& git -C $Repo rev-parse 'HEAD:workspace/dist').Trim()

$Result = [ordered]@{
    schema = 'sbc7b1-documents-ocr-windows-v1'
    gate = 'SBC-7B1-DOCUMENTS-OCR'
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
    frontend_tree = $FrontendTree
    proofs = [ordered]@{
        exact_changed_path_allowlist = $true
        product_core_unchanged = $true
        beankeeper_pin_unchanged = $true
        reviewed_native_dialog_dependency = $true
        native_lock_frozen = $true
        shark_application_schema_v3 = $true
        beankeeper_schema8_unchanged = $true
        preopen_backup_before_application_migration = $true
        user_controlled_document_reference_persistence = $true
        no_beankeeper_attachment_copy_for_new_bridge = $true
        native_root_paths_not_serialized = $true
        native_picker_rust_only = $true
        document_hash_and_length_derived_from_bytes = $true
        path_containment_and_traversal_rejection = $true
        document_tamper_detection = $true
        typed_money_in_money_out_attachment = $true
        attachment_does_not_post = $true
        factual_ocr_only_after_integrity_verification = $true
        inherited_ocr_runtime_unchanged = $true
        owner_dto_forbidden_fields_rejected = $true
        prior_owner_bridge_regressions = $true
        prior_bank_review_regressions = $true
        prior_bank_mutation_regressions = $true
        foundation_regressions = $true
        product_core_regressions = $true
        frontend_unwired = $true
        exact_tauri_acl_csp = $true
        locked_windows_compile = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC7B1_DOCUMENTS_OCR_RESULT.json')
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

Write-Host "[PASS] SBC-7B1 Documents/OCR evidence package written: $ZipPath"
Write-Host '[PASS] SBC-7B1 bounded Documents + factual OCR Windows proof complete'
