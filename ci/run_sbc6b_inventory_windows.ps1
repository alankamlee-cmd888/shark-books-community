param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$VenvPython = Join-Path $env:TEMP 'sharkbooks-sbc6a-venv\Scripts\python.exe'
$Generator = Join-Path $Repo 'research\sbc6_ocr_benchmark\generate_receipts.py'
$Inventory = Join-Path $Repo 'scripts\sbc6b_inventory_p1_runtime.py'
$Validator = Join-Path $Repo 'scripts\check_sbc6b_inventory_gate.py'
$FixtureRoot = Join-Path $env:TEMP 'sharkbooks-sbc6b-inventory-fixtures'
$OutputRoot = 'C:\SharkBooks-SBC6B-Inventory'
$Zip = 'C:\SharkBooks-SBC6B-Inventory\SBC6B_P1_RUNTIME_INVENTORY.zip'

function Assert-ExitCode([string]$Label) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

Push-Location $Repo
try {
    if (-not (Test-Path $VenvPython)) {
        throw 'SBC-6B inventory requires the cached SBC-6A venv from the completed Stage A benchmark.'
    }

    & $VenvPython $Validator
    Assert-ExitCode 'SBC-6B inventory gate validator'

    if (Test-Path $FixtureRoot) {
        Remove-Item -Recurse -Force $FixtureRoot
    }
    New-Item -ItemType Directory -Force -Path $FixtureRoot | Out-Null
    & $VenvPython $Generator --output $FixtureRoot
    Assert-ExitCode 'SBC-6B deterministic smoke fixture generation'

    $SmokeImage = Join-Path $FixtureRoot 'images\r01__clean.png'
    if (-not (Test-Path $SmokeImage)) {
        throw "Expected deterministic smoke image missing: $SmokeImage"
    }

    if (Test-Path $OutputRoot) {
        Remove-Item -Recurse -Force $OutputRoot
    }
    New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

    & $VenvPython $Inventory --repo $Repo --smoke-image $SmokeImage --output $OutputRoot
    Assert-ExitCode 'SBC-6B P1 runtime inventory'

    git rev-parse HEAD | Out-File -Encoding ascii (Join-Path $OutputRoot 'GIT_HEAD.txt')
    git status --short | Out-File -Encoding ascii (Join-Path $OutputRoot 'GIT_STATUS.txt')

    if (git status --short) {
        throw 'Repository became dirty during SBC-6B inventory proof.'
    }

    if (Test-Path $Zip) {
        Remove-Item -Force $Zip
    }
    $Files = Get-ChildItem -LiteralPath $OutputRoot -File | Where-Object { $_.FullName -ne $Zip }
    Compress-Archive -Path $Files.FullName -DestinationPath $Zip -Force

    Write-Host '[PASS] Repository remains clean after SBC-6B inventory proof.'
    Write-Host '[PASS] SBC-6B P1 runtime inventory and explicit-local-model smoke complete.'
    Write-Host "SBC6B_RESULT_ZIP=$Zip"
}
finally {
    Pop-Location
}
