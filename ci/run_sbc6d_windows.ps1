$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc6d_combined_closure.py'
$OutRoot = 'C:\SharkBooks-SBC6D'
$ZipName = 'SBC6D_COMBINED_OCR_RECEIPT_FLOW.zip'
$ZipPath = Join-Path $OutRoot $ZipName
$TmpZip = Join-Path $env:TEMP $ZipName

if ($env:OS -ne 'Windows_NT') { throw 'SBC-6D proof must run on Windows.' }
if (Test-Path $OutRoot) { Remove-Item $OutRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null

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
if ($null -eq $Python) { throw 'Python 3 is required for the SBC-6D evidence validator.' }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-6D proof repository: $Repo"
Write-Host '[INFO] Running combined OCR/receipt-flow closure gate'

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
if ($ValidatorExit -ne 0) { throw "SBC-6D validator failed with exit code $ValidatorExit" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after SBC-6D proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after SBC-6D proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-6D proof: $Status" }

$ProductLockHash = (Get-FileHash -Algorithm SHA256 (Join-Path $Repo 'product\shark-books-core\Cargo.lock')).Hash.ToLowerInvariant()
$NativeLockHash = (Get-FileHash -Algorithm SHA256 (Join-Path $Repo 'workspace\Cargo.lock')).Hash.ToLowerInvariant()
$NativeTree = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike').Trim()

$Result = [ordered]@{
    schema = 'sbc6d-combined-ocr-receipt-flow-windows-v1'
    gate = 'SBC-6D'
    gate_pass = $true
    entry_protected_main = 'ef799f5a5c3e4c1788ae7858b4865b6b3c5be496'
    inherited_b3c_proven_main = '5748b0236d3dc0f89975b9d7cb021af05f742f43'
    git_head = $Head
    git_status = $Status
    native_tauri_tree = $NativeTree
    product_cargo_lock_sha256 = $ProductLockHash
    native_cargo_lock_sha256 = $NativeLockHash
    proofs = [ordered]@{
        exact_four_path_closure_diff = $true
        b3c_native_tree_byte_identical = $true
        factual_ocr_to_bank_suggestion_composed = $true
        explicit_owner_confirmation_required = $true
        ambiguity_blocks_implicit_recommendation = $true
        penny_mismatch_fails_closed = $true
        missing_total_fails_closed = $true
        document_integrity_mismatch_fails_closed = $true
        rejection_non_destructive = $true
        no_accounting_tax_reconciliation_authority_added = $true
        inherited_product_regressions = $true
        frozen_foundation_tests = $true
        tauri_native_compile_locked = $true
        lockfiles_unchanged = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC6D_COMBINED_OCR_RECEIPT_FLOW_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutRoot, $TmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force

Write-Host "[PASS] SBC-6D evidence package written: $ZipPath"
Write-Host '[PASS] SBC-6D combined OCR/receipt-flow Windows bounded proof complete'
