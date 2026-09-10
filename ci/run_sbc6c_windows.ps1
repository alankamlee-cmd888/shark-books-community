$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc6c_receipt_bank_suggestion.py'
$OutRoot = 'C:\SharkBooks-SBC6C'
$ZipName = 'SBC6C_RECEIPT_BANK_SUGGESTION.zip'
$ZipPath = Join-Path $OutRoot $ZipName
$TmpZip = Join-Path $env:TEMP $ZipName

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-6C proof must run on Windows.'
}

if (Test-Path $OutRoot) {
    Remove-Item $OutRoot -Recurse -Force
}
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
if ($null -eq $Python) {
    throw 'Python 3 is required for the SBC-6C evidence validator.'
}

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-6C proof repository: $Repo"
Write-Host "[INFO] Running bounded receipt-to-bank suggestion gate"

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

if (Test-Path $Stdout) {
    Get-Content $Stdout | ForEach-Object { Write-Host $_ }
}
if ((Test-Path $Stderr) -and (Get-Item $Stderr).Length -gt 0) {
    Get-Content $Stderr | ForEach-Object { Write-Warning $_ }
}
if ($ValidatorExit -ne 0) {
    throw "SBC-6C validator failed with exit code $ValidatorExit"
}

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after SBC-6C proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after SBC-6C proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-6C proof: $Status" }

$ProductLock = Join-Path $Repo 'product\shark-books-core\Cargo.lock'
$NativeLock = Join-Path $Repo 'workspace\Cargo.lock'
$ProductLockHash = (Get-FileHash -Algorithm SHA256 $ProductLock).Hash.ToLowerInvariant()
$NativeLockHash = (Get-FileHash -Algorithm SHA256 $NativeLock).Hash.ToLowerInvariant()

$Result = [ordered]@{
    schema = 'sbc6c-receipt-bank-suggestion-windows-v1'
    gate = 'SBC-6C'
    gate_pass = $true
    entry_protected_main = '5748b0236d3dc0f89975b9d7cb021af05f742f43'
    git_head = $Head
    git_status = $Status
    product_cargo_lock_sha256 = $ProductLockHash
    native_cargo_lock_sha256 = $NativeLockHash
    proofs = [ordered]@{
        deterministic_receipt_bank_suggestion = $true
        exact_pence_fail_closed = $true
        bounded_date_and_text_support = $true
        ambiguity_prevents_auto_recommendation = $true
        duplicate_identity_separate = $true
        explicit_user_confirmation_required = $true
        rejection_non_destructive = $true
        no_accounting_tax_posting_reconciliation_authority = $true
        inherited_product_core_regressions = $true
        frozen_foundation_change_control = $true
        lockfiles_unchanged = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC6C_RECEIPT_BANK_SUGGESTION_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory(
    $OutRoot,
    $TmpZip,
    [System.IO.Compression.CompressionLevel]::Optimal,
    $false
)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force

Write-Host "[PASS] SBC-6C evidence package written: $ZipPath"
Write-Host '[PASS] SBC-6C deterministic receipt-to-bank suggestion Windows bounded proof complete'
