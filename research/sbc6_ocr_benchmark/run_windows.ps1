param(
    [switch]$OfflineProof,
    [ValidateSet('ALL','T0','P1','P2')]
    [string]$Lane = 'ALL'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Requirements = Join-Path $PSScriptRoot 'requirements.txt'
$Generator = Join-Path $PSScriptRoot 'generate_receipts.py'
$Runner = Join-Path $PSScriptRoot 'run_benchmark.py'
$Validator = Join-Path $Repo 'scripts\check_sbc6a_research.py'
$Venv = Join-Path $env:TEMP 'sharkbooks-sbc6a-venv'
$VenvPython = Join-Path $Venv 'Scripts\python.exe'
$FixtureRoot = Join-Path $env:TEMP 'sharkbooks-sbc6a-fixtures'
$ResultRoot = 'C:\SharkBooks-SBC6A-Results'
$Mode = if ($OfflineProof) { 'offline' } else { 'online' }
$Output = Join-Path $ResultRoot $Mode
$Zip = Join-Path $ResultRoot ("SBC6A_{0}_RESULT.zip" -f $Mode.ToUpperInvariant())

function Assert-ExitCode([string]$Label) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

Push-Location $Repo
try {
    if (git status --short) {
        throw 'Repository must be clean before SBC-6A benchmark proof.'
    }

    python $Validator --static-only
    Assert-ExitCode 'SBC-6A static validator'

    $BootstrapExe = $null
    $BootstrapArgs = @()
    if (Get-Command py -ErrorAction SilentlyContinue) {
        $BootstrapExe = 'py'
        $BootstrapArgs = @('-3')
    }
    elseif (Get-Command python -ErrorAction SilentlyContinue) {
        $BootstrapExe = 'python'
    }
    else {
        throw 'Python 3 is required. Install a supported Python 3.10-3.13 interpreter and rerun.'
    }

    if (-not $OfflineProof) {
        if (Test-Path $Venv) {
            Remove-Item -Recurse -Force $Venv
        }
        & $BootstrapExe @BootstrapArgs -m venv $Venv
        Assert-ExitCode 'Python venv creation'
        & $VenvPython -m pip install --disable-pip-version-check -r $Requirements
        Assert-ExitCode 'Research dependency installation'
    }
    elseif (-not (Test-Path $VenvPython)) {
        throw 'Offline proof requires the cached SBC-6A venv from a completed online run.'
    }

    & $VenvPython -c "import sys; assert (3,10) <= sys.version_info[:2] <= (3,13), sys.version"
    Assert-ExitCode 'Python version check'

    $TesseractExe = $null
    $TesseractCommand = Get-Command tesseract -ErrorAction SilentlyContinue
    if ($TesseractCommand) {
        $TesseractExe = $TesseractCommand.Source
    }
    elseif (Test-Path 'C:\Program Files\Tesseract-OCR\tesseract.exe') {
        $TesseractExe = 'C:\Program Files\Tesseract-OCR\tesseract.exe'
        $env:PATH = "C:\Program Files\Tesseract-OCR;$env:PATH"
    }

    if (($Lane -eq 'ALL' -or $Lane -eq 'T0') -and -not $TesseractExe) {
        Write-Host '[BLOCKED] Tesseract 5.5.3 baseline is not installed.'
        Write-Host 'Install it, then rerun this same command:'
        Write-Host '  winget install --id tesseract-ocr.tesseract --exact --version 5.5.3'
        exit 3
    }
    if ($TesseractExe) {
        $VersionLine = (& $TesseractExe --version | Select-Object -First 1)
        if ($VersionLine -notmatch '5\.5\.3') {
            throw "Tesseract 5.5.3 required for frozen T0 lane; found: $VersionLine"
        }
        Write-Host "[PASS] Tesseract baseline: $VersionLine"
    }

    if (Test-Path $FixtureRoot) {
        Remove-Item -Recurse -Force $FixtureRoot
    }
    New-Item -ItemType Directory -Force -Path $FixtureRoot | Out-Null
    & $VenvPython $Generator --output $FixtureRoot
    Assert-ExitCode 'Synthetic receipt generation'

    if (Test-Path $Output) {
        Remove-Item -Recurse -Force $Output
    }
    New-Item -ItemType Directory -Force -Path $Output | Out-Null

    $VenvBytes = (Get-ChildItem -LiteralPath $Venv -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    if (-not $VenvBytes) { $VenvBytes = 0 }
    $env:SBC6A_VENV_BYTES = [string]$VenvBytes

    & $VenvPython -m pip freeze | Out-File -Encoding ascii (Join-Path $Output 'PIP_FREEZE.txt')
    & $VenvPython --version | Out-File -Encoding ascii (Join-Path $Output 'PYTHON_VERSION.txt')
    if ($TesseractExe) {
        (& $TesseractExe --version) | Out-File -Encoding ascii (Join-Path $Output 'TESSERACT_VERSION.txt')
    }

    $Lanes = if ($Lane -eq 'ALL') { @('T0','P1','P2') } else { @($Lane) }
    $RunArgs = @($Runner, '--fixtures', $FixtureRoot, '--output-dir', $Output, '--lanes') + $Lanes
    if ($OfflineProof) {
        $RunArgs += '--offline-proof'
    }

    & $VenvPython @RunArgs
    $BenchmarkExit = $LASTEXITCODE

    Copy-Item (Join-Path $FixtureRoot 'manifest.json') (Join-Path $Output 'SYNTHETIC_RECEIPT_MANIFEST.json') -Force
    git rev-parse HEAD | Out-File -Encoding ascii (Join-Path $Output 'GIT_HEAD.txt')
    git status --short | Out-File -Encoding ascii (Join-Path $Output 'GIT_STATUS.txt')

    if (Test-Path $Zip) { Remove-Item -Force $Zip }
    Compress-Archive -Path (Join-Path $Output '*') -DestinationPath $Zip -Force
    Write-Host "SBC6A_RESULT_ZIP=$Zip"

    if ($BenchmarkExit -ne 0) {
        throw "SBC-6A comparator did not complete all requested lanes; exit code $BenchmarkExit"
    }

    if (git status --short) {
        throw 'Repository became dirty during SBC-6A benchmark proof.'
    }
    Write-Host '[PASS] Repository remains clean after SBC-6A research benchmark.'
    if ($OfflineProof) {
        Write-Host '[PASS] SBC-6A offline benchmark run completed with connectivity absent before document bytes were read.'
    }
    else {
        Write-Host '[PASS] SBC-6A online comparator completed; offline no-egress rerun remains required before Stage A can close.'
    }
}
finally {
    Pop-Location
}
