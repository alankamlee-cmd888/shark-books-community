$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc7a_ui_contract.py'
$OutRoot = 'C:\SharkBooks-SBC7A'
$ZipName = 'SBC7A_OWNER_UI_CONTRACT.zip'
$ZipPath = Join-Path $OutRoot $ZipName
$TmpZip = Join-Path $env:TEMP $ZipName

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-7A proof must run on Windows.'
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
    throw 'Python 3 is required for the SBC-7A evidence validator.'
}

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-7A proof repository: $Repo"
Write-Host '[INFO] Validating frozen owner-facing UI contract and operation boundary'

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
    throw "SBC-7A validator failed with exit code $ValidatorExit"
}

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after SBC-7A proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after SBC-7A proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-7A proof: $Status" }

$ProductLock = Join-Path $Repo 'product\shark-books-core\Cargo.lock'
$NativeLock = Join-Path $Repo 'workspace\Cargo.lock'
$ProductLockHash = (Get-FileHash -Algorithm SHA256 $ProductLock).Hash.ToLowerInvariant()
$NativeLockHash = (Get-FileHash -Algorithm SHA256 $NativeLock).Hash.ToLowerInvariant()

$Result = [ordered]@{
    schema = 'sbc7a-owner-ui-contract-windows-v1'
    gate = 'SBC-7A'
    gate_pass = $true
    entry_protected_main = '3fcbbc69a8e30a6d6281ce410f1e5e80c8f43ace'
    git_head = $Head
    git_status = $Status
    product_cargo_lock_sha256 = $ProductLockHash
    native_cargo_lock_sha256 = $NativeLockHash
    proofs = [ordered]@{
        exact_four_path_design_diff = $true
        product_tree_unchanged = $true
        native_workspace_and_frontend_unchanged = $true
        navigation_frozen = $true
        eight_owner_journeys_frozen = $true
        error_and_degrade_states_frozen = $true
        explicit_confirmation_points_frozen = $true
        owner_language_frozen = $true
        accessibility_and_responsive_intent_frozen = $true
        application_operation_matrix_frozen = $true
        current_native_command_inventory_reconciled = $true
        sbc7b_native_application_bridge_change_control_required = $true
        no_accounting_tax_ai_or_raw_database_authority_added = $true
        frozen_foundation_change_control = $true
        lockfiles_unchanged = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC7A_OWNER_UI_CONTRACT_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory(
    $OutRoot,
    $TmpZip,
    [System.IO.Compression.CompressionLevel]::Optimal,
    $false
)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force

Write-Host "[PASS] SBC-7A evidence package written: $ZipPath"
Write-Host '[PASS] SBC-7A owner-facing UI contract Windows bounded proof complete'
