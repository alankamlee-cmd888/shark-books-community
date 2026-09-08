$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-4 Windows proof must run on Windows.'
}

python .\scripts\check_sbc4_matching_reconciliation.py
if ($LASTEXITCODE -ne 0) {
    throw "SBC-4 validator failed with exit code $LASTEXITCODE"
}
