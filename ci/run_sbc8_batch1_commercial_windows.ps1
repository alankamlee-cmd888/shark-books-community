param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$ExpectedHead
)

$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc8_batch1_commercial_domain.py'
$OutRoot = Join-Path $env:TEMP 'SharkBooks-SBC8-Batch1-Commercial'
$CargoTarget = Join-Path $env:USERPROFILE 'sbc8-batch1-commercial-proof-target'
$ZipName = 'SBC8_BATCH1_COMMERCIAL_DOMAIN_WINDOWS.zip'
$Downloads = Join-Path $env:USERPROFILE 'Downloads'
$ZipPath = Join-Path $Downloads $ZipName
$TmpZip = Join-Path $env:TEMP ('tmp_' + $ZipName)
$Base = '8f55eaf67b311a7c2d69ebea86ae702e4934a56c'

if ($env:OS -ne 'Windows_NT') { throw 'SBC-8 Batch 1 proof must run on Windows.' }
if (Test-Path $OutRoot) { Remove-Item $OutRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null
New-Item -ItemType Directory -Force -Path $Downloads | Out-Null
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
if (Test-Path $CargoTarget) { Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $CargoTarget | Out-Null
$env:CARGO_TARGET_DIR = $CargoTarget

$Python = Get-Command python -ErrorAction SilentlyContinue
if ($null -eq $Python) { throw 'Python 3 is required for SBC-8 Batch 1 proof.' }
if ($null -eq (Get-Command rustup -ErrorAction SilentlyContinue)) { throw 'rustup is required for SBC-8 Batch 1 proof.' }

$EntryHead = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD before proof.' }
if ($EntryHead.ToLowerInvariant() -ne $ExpectedHead.ToLowerInvariant()) {
    throw "Wrong candidate HEAD before proof. Expected $ExpectedHead but found $EntryHead"
}
$EntryStatus = ((& git -C $Repo status --porcelain) -join "`n").Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status before proof.' }
if ($EntryStatus) { throw "Repository dirty before proof: $EntryStatus" }

$Stdout = Join-Path $OutRoot 'VALIDATOR.stdout.txt'
$Stderr = Join-Path $OutRoot 'VALIDATOR.stderr.txt'

Write-Host "[INFO] SBC-8 Batch 1 proof repository: $Repo"
Write-Host "[INFO] Expected candidate HEAD: $ExpectedHead"
Write-Host "[INFO] Cargo target: $CargoTarget"

$Process = Start-Process `
    -FilePath $Python.Source `
    -ArgumentList @('-B', $Validator) `
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
if ($Process.ExitCode -ne 0) { throw "SBC-8 Batch 1 validator failed with exit code $($Process.ExitCode)" }

$Head = (& git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to resolve repository HEAD.' }
if ($Head.ToLowerInvariant() -ne $ExpectedHead.ToLowerInvariant()) {
    throw "Candidate HEAD changed during proof. Expected $ExpectedHead but found $Head"
}
$Status = ((& git -C $Repo status --porcelain) -join "`n").Trim()
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository status.' }
if ($Status) { throw "Repository dirty after proof: $Status" }

$Lock = Join-Path $Repo 'incubator\sbc8-commercial-core\Cargo.lock'
$LockHash = (Get-FileHash -Algorithm SHA256 $Lock).Hash.ToLowerInvariant()
$Changed = (& git -C $Repo diff --name-status "$Base..HEAD") -join "`n"

$Result = [ordered]@{
    schema = 'sbc8-batch1-commercial-domain-windows-v1'
    gate = 'SBC8-BATCH1-COMMERCIAL-DOMAIN-INCUBATION'
    gate_pass = $true
    production_integration_authorised = $false
    entry_protected_main = $Base
    expected_candidate_head = $ExpectedHead.ToLowerInvariant()
    git_head = $Head
    git_status = $Status
    cargo_lock_sha256 = $LockHash
    rust_toolchain = '1.98.1'
    proofs = [ordered]@{
        exact_candidate_head = $true
        exact_changed_path_allowlist = $true
        active_sbc7_surfaces_unchanged = $true
        zero_runtime_dependencies = $true
        quote_invoice_credit_domain = $true
        provider_neutral_payment_domain = $true
        supplier_bill_domain = $true
        purchase_order_domain = $true
        integer_money_and_quantity = $true
        no_network_platform_persistence_tax_authority = $true
        standalone_tests = $true
        repository_clean = $true
    }
}
$Result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutRoot 'SBC8_BATCH1_COMMERCIAL_RESULT.json')
Set-Content -Encoding ASCII (Join-Path $OutRoot 'EXPECTED_HEAD.txt') ($ExpectedHead.ToLowerInvariant() + "`n")
Set-Content -Encoding ASCII (Join-Path $OutRoot 'GIT_HEAD.txt') ($Head + "`n")
Set-Content -Encoding ASCII (Join-Path $OutRoot 'CARGO_LOCK_SHA256.txt') ($LockHash + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'GIT_STATUS.txt') ($Status + "`n")
Set-Content -Encoding UTF8 (Join-Path $OutRoot 'CHANGED_PATHS.txt') ($Changed + "`n")

if (Test-Path $TmpZip) { Remove-Item $TmpZip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutRoot, $TmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Copy-Item $TmpZip $ZipPath -Force
Remove-Item $TmpZip -Force
Remove-Item $CargoTarget -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[PASS] SBC-8 Batch 1 evidence package written: $ZipPath"
Write-Host '[PASS] SBC-8 Batch 1 standalone commercial-domain Windows proof complete'
