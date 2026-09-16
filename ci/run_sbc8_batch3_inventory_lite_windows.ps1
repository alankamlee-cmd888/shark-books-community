param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$ExpectedHead
)

$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc8_batch3_inventory_lite_domain.py'
$OutRoot = Join-Path $env:TEMP 'SharkBooks-SBC8-Batch3-Inventory-Lite'
$CargoTarget = Join-Path $env:USERPROFILE 'sbc8-batch3-inventory-lite-proof-target'
$ZipName = 'SBC8_BATCH3_INVENTORY_LITE_WINDOWS.zip'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads $ZipName
$TmpZip = Join-Path $env:TEMP ('tmp_' + $ZipName)
$Base = '6f85734d0e9e11089a1a47b7850a306a4ed7ba87'
$ShortHead = $ExpectedHead.Substring(0, 8).ToLowerInvariant()
$Desktop = [Environment]::GetFolderPath('Desktop')
$PreservedZip = Join-Path $Desktop ("SBC8_BATCH3_INVENTORY_LITE_${ShortHead}_EVIDENCE.zip")

if ($env:OS -ne 'Windows_NT') { throw 'SBC-8 Batch 3 proof must run on Windows.' }
if (Test-Path $OutRoot) { Remove-Item $OutRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null
New-Item -ItemType Directory -Force -Path $Downloads | Out-Null
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
if (Test-Path $CargoTarget) { Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $CargoTarget | Out-Null
$env:CARGO_TARGET_DIR = $CargoTarget

$Python = Get-Command python -ErrorAction SilentlyContinue
if ($null -eq $Python) { throw 'Python 3 is required for SBC-8 Batch 3 proof.' }
if ($null -eq (Get-Command rustup -ErrorAction SilentlyContinue)) { throw 'rustup is required for SBC-8 Batch 3 proof.' }

$EntryHead = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD before proof.' }
if ($EntryHead.ToLowerInvariant() -ne $ExpectedHead.ToLowerInvariant()) {
    throw "Wrong candidate HEAD before proof. Expected $ExpectedHead but found $EntryHead"
}
$EntryStatus = ((& git -C $Repo status --porcelain) -join "`n").Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status before proof.' }
if ($EntryStatus) { throw "Repository dirty before proof: $EntryStatus" }

$RustVersion = (& rustup run 1.98.1 rustc --version).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Rust 1.98.1 toolchain is required for SBC-8 Batch 3 proof.' }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-8 Batch 3 proof repository: $Repo"
Write-Host "[INFO] Expected candidate HEAD: $ExpectedHead"
Write-Host "[INFO] Rust toolchain: $RustVersion"
Write-Host "[INFO] Cargo target: $CargoTarget"

$Process = Start-Process `
    -FilePath $Python.Source `
    -ArgumentList @('-B', $Validator, '--expected-head', $ExpectedHead) `
    -WorkingDirectory $Repo `
    -RedirectStandardOutput $Stdout `
    -RedirectStandardError $Stderr `
    -NoNewWindow `
    -Wait `
    -PassThru

if (Test-Path $Stdout) { Get-Content $Stdout | ForEach-Object { Write-Host $_ } }
if ((Test-Path $Stderr) -and (Get-Item $Stderr).Length -gt 0) {
    Get-Content $Stderr | ForEach-Object { Write-Warning $_ }
}
if ($Process.ExitCode -ne 0) { throw "SBC-8 Batch 3 validator failed with exit code $($Process.ExitCode)" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD after proof.' }
if ($Head.ToLowerInvariant() -ne $ExpectedHead.ToLowerInvariant()) {
    throw "Candidate HEAD changed during proof. Expected $ExpectedHead but found $Head"
}
$Status = ((& git -C $Repo status --porcelain) -join "`n").Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status after proof.' }
if ($Status) { throw "Repository dirty after proof: $Status" }

$Lock = Join-Path $Repo 'incubator\sbc8-inventory-lite-core\Cargo.lock'
$LockHash = (Get-FileHash -Algorithm SHA256 $Lock).Hash.ToLowerInvariant()
$ChangedLines = @(& git -C $Repo diff --name-status "$Base..HEAD")
if ($LASTEXITCODE -ne 0) { throw 'Unable to capture changed paths.' }
if ($ChangedLines.Count -ne 12) { throw "Expected exactly 12 Batch 3 changed paths but found $($ChangedLines.Count)." }
$Changed = $ChangedLines -join "`n"

$Result = [ordered]@{
    schema = 'sbc8-batch3-inventory-lite-windows-v1'
    gate = 'SBC8-BATCH3-INVENTORY-LITE-INCUBATION'
    gate_pass = $true
    production_integration_authorised = $false
    entry_protected_main = $Base
    expected_candidate_head = $ExpectedHead.ToLowerInvariant()
    git_head = $Head
    git_status = $Status
    changed_path_count = $ChangedLines.Count
    cargo_lock_sha256 = $LockHash
    rust_toolchain = $RustVersion
    proofs = [ordered]@{
        exact_candidate_head = $true
        exact_changed_path_allowlist = $true
        active_sbc7_surfaces_unchanged = $true
        zero_runtime_dependencies = $true
        append_only_stock_movements = $true
        deterministic_derived_balances = $true
        negative_stock_not_clamped = $true
        paired_transfer_conservation = $true
        stocktake_proposes_append_only_adjustment = $true
        valuation_cogs_excluded = $true
        no_network_platform_persistence_accounting_authority = $true
        standalone_tests = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC8_BATCH3_INVENTORY_LITE_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'EXPECTED_HEAD.txt') ($ExpectedHead.ToLowerInvariant() + "`n")
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding ASCII (Join-Path $OutRoot 'CARGO_LOCK_SHA256.txt') ($LockHash + "`n")
Set-Content -Encoding ASCII (Join-Path $OutRoot 'CHANGED_PATH_COUNT.txt') ($ChangedLines.Count.ToString() + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'RUSTC_VERSION.txt') ($RustVersion + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'CHANGED_PATHS.txt') ($Changed + "`n")

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutRoot, $TmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force

$ZipHash = (Get-FileHash -Algorithm SHA256 $ZipPath).Hash.ToLowerInvariant()
Copy-Item $ZipPath $PreservedZip -Force
$PreservedHash = (Get-FileHash -Algorithm SHA256 $PreservedZip).Hash.ToLowerInvariant()
if ($ZipHash -ne $PreservedHash) { throw 'Preserved evidence copy hash mismatch.' }

Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[PASS] SBC-8 Batch 3 evidence package written: $ZipPath"
Write-Host "[PASS] Preserved evidence copy: $PreservedZip"
Write-Host "[INFO] Evidence SHA-256: $ZipHash"
Write-Host "[PASS] SBC-8 Batch 3 standalone Inventory Lite Windows proof complete at $Head"
