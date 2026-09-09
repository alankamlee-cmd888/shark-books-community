$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Runner = Join-Path $Repo 'research\sbc6_ocr_runtime\run_b3c_native_windows.py'

if ($env:OS -ne 'Windows_NT') {
    throw 'SBC-6B B3C proof must run on Windows.'
}

function Invoke-B3CCompatiblePython {
    $Launcher = Get-Command py -ErrorAction SilentlyContinue
    if ($null -ne $Launcher) {
        foreach ($Spec in @('-3.13', '-3.12', '-3.11', '-3.10')) {
            $Version = & $Launcher.Source $Spec -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')" 2>$null
            if ($LASTEXITCODE -eq 0) {
                Write-Host "[INFO] SBC-6B B3C selected compatible CPython $Version via py $Spec"
                & $Launcher.Source $Spec -B $Runner --repo $Repo
                if ($LASTEXITCODE -ne 0) {
                    throw "SBC-6B B3C proof failed with exit code $LASTEXITCODE"
                }
                return
            }
        }
    }

    $Python = Get-Command python -ErrorAction SilentlyContinue
    if ($null -ne $Python) {
        $Minor = & $Python.Source -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')" 2>$null
        $Version = & $Python.Source -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')" 2>$null
        if ($LASTEXITCODE -eq 0 -and $Minor -match '^3\.(10|11|12|13)$') {
            Write-Host "[INFO] SBC-6B B3C selected compatible CPython $Version via python"
            & $Python.Source -B $Runner --repo $Repo
            if ($LASTEXITCODE -ne 0) {
                throw "SBC-6B B3C proof failed with exit code $LASTEXITCODE"
            }
            return
        }
    }

    $Detected = '<none>'
    if ($null -ne $Python) {
        $Detected = & $Python.Source -c "import sys; print(sys.version.split()[0])" 2>$null
    }
    throw "No compatible CPython 3.10-3.13 interpreter is installed. Frozen onnxruntime 1.23.2 has no CPython 3.14 Windows wheel. Detected default python: $Detected. Install Python 3.13 (user scope is sufficient), then rerun this same script."
}

Invoke-B3CCompatiblePython
Write-Host '[PASS] SBC-6B B3C Windows harness complete'
