$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$env:PYTHONDONTWRITEBYTECODE = "1"

$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Repo

function Invoke-Checked([string]$Label, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

$Python = $null
foreach ($Version in @("3.13", "3.12", "3.11", "3.10")) {
    & py "-$Version" -c "import sys; print(sys.version)" *> $null
    if ($LASTEXITCODE -eq 0) {
        $Python = @("py", "-$Version")
        Write-Host "[PASS] Selected supported CPython $Version for B3B proof bootstrap"
        break
    }
}
if ($null -eq $Python) {
    throw "B3B requires CPython 3.10-3.13. Install Python 3.13 and rerun."
}

Invoke-Checked "B3B static preflight" { & $Python[0] $Python[1] -B .\scripts\check_sbc6b_b3b_single_runtime.py }
Invoke-Checked "B3B packaged single-document proof" { & $Python[0] $Python[1] -B .\research\sbc6_ocr_runtime\run_b3b_single_windows.py --repo $Repo }
