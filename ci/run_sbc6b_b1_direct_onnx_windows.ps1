param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Base = 'a2d0c776199934d79fe1e1dce1889d3999f44b91'
$VenvPython = Join-Path $env:TEMP 'sharkbooks-sbc6a-venv\Scripts\python.exe'
$Validator = Join-Path $Repo 'scripts\check_sbc6b_b1_packaging_gate.py'
$ChangeGuard = Join-Path $Repo 'scripts\check_sbc1g_change_control.py'
$FrozenGuard = Join-Path $Repo 'scripts\check_frozen_baseline.py'
$Generator = Join-Path $Repo 'research\sbc6_ocr_benchmark\generate_receipts.py'
$Direct = Join-Path $Repo 'research\sbc6_ocr_runtime\direct_onnx_parity.py'
$FixtureRoot = Join-Path $env:TEMP 'sharkbooks-sbc6b-b1-fixtures'
$DetModel = Join-Path $env:USERPROFILE '.paddlex\official_models\PP-OCRv6_tiny_det_onnx'
$RecModel = Join-Path $env:USERPROFILE '.paddlex\official_models\PP-OCRv6_tiny_rec_onnx'
$OutputRoot = 'C:\SharkBooks-SBC6B-B1'
$Zip = Join-Path $OutputRoot 'SBC6B_B1_DIRECT_ONNX_PARITY.zip'

function Assert-ExitCode([string]$Label) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

Push-Location $Repo
try {
    if (-not (Test-Path $VenvPython)) {
        throw 'SBC-6B B1 requires the cached SBC-6A proof venv.'
    }
    if (-not (Test-Path (Join-Path $DetModel 'inference.onnx'))) {
        throw "Selected tiny detector ONNX cache is missing: $DetModel"
    }
    if (-not (Test-Path (Join-Path $RecModel 'inference.onnx'))) {
        throw "Selected tiny recogniser ONNX cache is missing: $RecModel"
    }

    $env:PYTHONDONTWRITEBYTECODE = '1'
    $env:ORT_DISABLE_TELEMETRY = '1'

    & $VenvPython -B $Validator
    Assert-ExitCode 'SBC-6B B1 static packaging/parity validator'

    & $VenvPython -B $FrozenGuard
    Assert-ExitCode 'Frozen foundation guard'

    & $VenvPython -B $ChangeGuard --base-ref $Base
    Assert-ExitCode 'SBC-1G permanent change-control guard'

    if (Test-Path $FixtureRoot) {
        Remove-Item -Recurse -Force $FixtureRoot
    }
    New-Item -ItemType Directory -Force -Path $FixtureRoot | Out-Null
    & $VenvPython -B $Generator --output $FixtureRoot
    Assert-ExitCode 'SBC-6B B1 deterministic fixture generation'

    if (Test-Path $OutputRoot) {
        Remove-Item -Recurse -Force $OutputRoot
    }
    New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

    & $VenvPython -B $Direct `
        --repo $Repo `
        --fixtures $FixtureRoot `
        --det-model-dir $DetModel `
        --rec-model-dir $RecModel `
        --output $OutputRoot
    $DirectExit = $LASTEXITCODE

    git rev-parse HEAD | Out-File -Encoding ascii (Join-Path $OutputRoot 'GIT_HEAD.txt')
    git status --short | Out-File -Encoding ascii (Join-Path $OutputRoot 'GIT_STATUS.txt')

    if (git status --short) {
        throw 'Repository became dirty during SBC-6B B1 direct-ONNX proof.'
    }

    $Files = Get-ChildItem -LiteralPath $OutputRoot -File | Where-Object { $_.FullName -ne $Zip }
    if (-not $Files) {
        throw 'SBC-6B B1 produced no evidence files to archive.'
    }
    if (Test-Path $Zip) {
        Remove-Item -Force $Zip
    }
    Compress-Archive -Path $Files.FullName -DestinationPath $Zip -Force

    Write-Host '[PASS] Repository remains clean after SBC-6B B1 proof.'
    Write-Host "SBC6B_B1_RESULT_ZIP=$Zip"

    if ($DirectExit -ne 0) {
        throw "SBC-6B B1 direct-ONNX parity gate did not pass; evidence was preserved at $Zip (exit code $DirectExit)"
    }

    Write-Host '[PASS] SBC-6B B1 direct-ONNX packaging parity gate complete.'
}
finally {
    Pop-Location
}
