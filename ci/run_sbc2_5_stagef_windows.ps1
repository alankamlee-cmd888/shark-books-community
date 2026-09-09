$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-2-5 Stage F Windows proof must run on Windows.'
}

python .\scripts\check_sbc2_5_stagef.py
if ($LASTEXITCODE -ne 0) {
    throw "SBC-2-5 Stage F validator failed with exit code $LASTEXITCODE"
}
