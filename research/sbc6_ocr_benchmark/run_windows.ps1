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

function Select-SupportedPython {
    if (Get-Command py -ErrorAction SilentlyContinue) {
        foreach ($Minor in @('3.13','3.12','3.11','3.10')) {
            & py "-$Minor" -c "import sys; raise SystemExit(0 if sys.version_info[:2] == tuple(map(int, '$Minor'.split('.'))) else 2)" 2>$null
            if ($LASTEXITCODE -eq 0) {
                return @{ Exe = 'py'; Args = @("-$Minor"); Label = "Python $Minor via py launcher" }
            }
        }
    }

    if (Get-Command python -ErrorAction SilentlyContinue) {
        & python -c "import sys; raise SystemExit(0 if (3,10) <= sys.version_info[:2] <= (3,13) else 2)" 2>$null
        if ($LASTEXITCODE -eq 0) {
            $Version = (& python -c "import sys; print('.'.join(map(str, sys.version_info[:3])))").Trim()
            return @{ Exe = 'python'; Args = @(); Label = "Python $Version" }
        }
    }

    return $null
}

Push-Location $Repo
try {
    if (git status --short) {
        throw 'Repository must be clean before SBC-6A benchmark proof.'
    }

    $Bootstrap = Select-SupportedPython
    if (-not $Bootstrap) {
        Write-Host '[BLOCKED] SBC-6A requires CPython 3.10-3.13 because the frozen onnxruntime 1.23.2 comparator has no Windows CPython 3.14 wheel.'
        Write-Host 'Install Python 3.13, then rerun this same benchmark command:'
        Write-Host '  winget install --id Python.Python.3.13 --exact'
        throw 'No supported CPython 3.10-3.13 interpreter is installed; no OCR benchmark was run.'
    }
    $BootstrapExe = [string]$Bootstrap.Exe
    $BootstrapArgs = @($Bootstrap.Args)
    Write-Host ("[PASS] Supported benchmark interpreter selected: {0}" -f $Bootstrap.Label)

    & $BootstrapExe @BootstrapArgs $Validator --static-only
    Assert-ExitCode 'SBC-6A static validator'

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
        throw 'Tesseract baseline missing; no OCR benchmark was run.'
    }
    if ($TesseractExe) {
        $VersionLine = (& $TesseractExe --version | Select-Object -First 1)
        if ($VersionLine -notmatch '5\.5\.3') {
            throw "Tesseract 5.5.3 required for frozen T0 lane; found: $VersionLine"
        }
        Write-Host "[PASS] Tesseract baseline: $VersionLine"
    }

    if (-not $OfflineProof) {
        if (Test-Path $Venv) {
            Remove-Item -Recurse -Force $Venv
        }
        & $BootstrapExe @BootstrapArgs -m venv $Venv
        Assert-ExitCode 'Python venv creation'
        & $VenvPython -c "import sys; assert (3,10) <= sys.version_info[:2] <= (3,13), sys.version"
        Assert-ExitCode 'Python version check before dependency installation'
        & $VenvPython -m pip install --disable-pip-version-check -r $Requirements
        Assert-ExitCode 'Research dependency installation'
    }
    elseif (-not (Test-Path $VenvPython)) {
        throw 'Offline proof requires the cached SBC-6A venv from a completed online run.'
    }

    & $VenvPython -c "import sys; assert (3,10) <= sys.version_info[:2] <= (3,13), sys.version"
    Assert-ExitCode 'Python version check'

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
