$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-3 Windows proof must run on Windows.'
}

python .\scripts\check_sbc3_bank_import.py
if ($LASTEXITCODE -ne 0) {
    throw "SBC-3 validator failed with exit code $LASTEXITCODE"
}
