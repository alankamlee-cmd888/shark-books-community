$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc7b1_owner_bridge.py'
$OutRoot = Join-Path $env:TEMP 'SharkBooks-SBC7B1'
$ZipName = 'SBC7B1_OWNER_APPLICATION_BRIDGE.zip'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads $ZipName
$TmpZip = Join-Path $env:TEMP ('tmp_' + $ZipName)

if ($env:OS -ne 'Windows_NT') { throw 'SBC-7B1 proof must run on Windows.' }
if (Test-Path $OutRoot) { Remove-Item $OutRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null
New-Item -ItemType Directory -Force -Path $Downloads | Out-Null
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }

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
if ($null -eq $Python) { throw 'Python 3 is required for the SBC-7B1 evidence validator.' }
if ($null -eq (Get-Command rustup -ErrorAction SilentlyContinue)) { throw 'rustup is required for SBC-7B1.' }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-7B1 proof repository: $Repo"
Write-Host '[INFO] Running bounded owner application bridge gate'

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
if ($ValidatorExit -ne 0) { throw "SBC-7B1 validator failed with exit code $ValidatorExit" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after SBC-7B1 proof.' }
$StatusLines = @(& git -C $Repo status --short)
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after SBC-7B1 proof.' }
$Status = ($StatusLines -join "`n").Trim()
if ($Status) { throw "Repository dirty after SBC-7B1 proof: $Status" }

$NativeLockHash = (Get-FileHash -Algorithm SHA256 (Join-Path $Repo 'workspace\Cargo.lock')).Hash.ToLowerInvariant()
$ProductTree = (& git -C $Repo rev-parse 'HEAD:product/shark-books-core').Trim()
$OcrBlob = (& git -C $Repo rev-parse 'HEAD:workspace/shark-tauri-spike/src/ocr_native.rs').Trim()
$FrontendTree = (& git -C $Repo rev-parse 'HEAD:workspace/dist').Trim()

$Result = [ordered]@{
    schema = 'sbc7b1-owner-application-bridge-windows-v1'
    gate = 'SBC-7B1'
    gate_pass = $true
    entry_protected_main = '70777e04ccfc1e427c60e18269b8a0738cc8610c'
    git_head = $Head
    git_status = $Status
    native_cargo_lock_sha256 = $NativeLockHash
    product_core_tree = $ProductTree
    ocr_native_blob = $OcrBlob
    frontend_tree = $FrontendTree
    proofs = [ordered]@{
        exact_changed_path_allowlist = $true
        local_product_core_dependency_only = $true
        external_crate_graph_unchanged = $true
        product_core_unchanged = $true
        ocr_native_unchanged = $true
        frontend_unwired = $true
        typed_owner_commands_registered = $true
        unknown_fields_rejected = $true
        explicit_business_use_input = $true
        integer_pence_fail_closed = $true
        explicit_preview_then_save_boundary = $true
        foundation_regressions = $true
        product_core_regressions = $true
        tauri_owner_bridge_tests = $true
        locked_windows_compile = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC7B1_OWNER_APPLICATION_BRIDGE_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")
(& git -C $Repo diff --name-status '70777e04ccfc1e427c60e18269b8a0738cc8610c..HEAD') | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'CHANGED_PATHS.txt')

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutRoot, $TmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force

Write-Host "[PASS] SBC-7B1 evidence package written: $ZipPath"
Write-Host '[PASS] SBC-7B1 bounded owner application bridge Windows proof complete'
