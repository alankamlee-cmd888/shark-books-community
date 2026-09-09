$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Runner = Join-Path $Repo 'research\sbc6_ocr_runtime\run_b3c_native_windows.py'

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-6B B3C proof must run on Windows.'
}

$Python = (Get-Command python -ErrorAction Stop).Source
& $Python -B $Runner --repo $Repo
if ($LASTEXITCODE -ne 0) {
    throw "SBC-6B B3C proof failed with exit code $LASTEXITCODE"
}

Write-Host '[PASS] SBC-6B B3C Windows harness complete'
