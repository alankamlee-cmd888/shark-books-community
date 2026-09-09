param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Validator = Join-Path $Repo 'scripts\check_sbc6b_b2_ocr_contract.py'

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-6B B2 proof must run on Windows.'
}

$Py = Get-Command py -ErrorAction SilentlyContinue
if (-not $Py) {
    throw 'Python launcher py.exe is required for the SBC-6B B2 proof.'
}

$env:PYTHONDONTWRITEBYTECODE = '1'

Push-Location $Repo
try {
    & py -3.13 -B $Validator
    if ($LASTEXITCODE -ne 0) {
        throw "SBC-6B B2 validator failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
