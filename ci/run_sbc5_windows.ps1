$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-5 Windows proof must run on Windows.'
}

python .\scripts\check_sbc5_documents_storage.py
if ($LASTEXITCODE -ne 0) {
    throw "SBC-5 validator failed with exit code $LASTEXITCODE"
}
