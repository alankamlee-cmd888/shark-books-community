$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-2 Windows proof must run on Windows.'
}

python .\scripts\check_sbc2_domain.py
if ($LASTEXITCODE -ne 0) {
    throw "SBC-2 validator failed with exit code $LASTEXITCODE"
}
