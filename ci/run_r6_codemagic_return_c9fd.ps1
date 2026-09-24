$ErrorActionPreference = 'Stop'

$Candidate = 'c9fd45ae67e977ba986706308ac6fae2d3bdf731'
$Parent = '06f3223ed666518505381335c771ea92fa68241b'
$ExpectedIconBlob = 'ae77e04a393cc7abf87e72bb14ec2647e3f4d81e'
$IconPath = 'workspace/shark-tauri-spike/icons/icon.png'
$EvidenceBranch = 'sbc7b2-r6-evidence-c9fd-20260924'
$Repo = $env:CM_BUILD_DIR
if (-not $Repo) { throw 'CM_BUILD_DIR is not set.' }

$ReturnDir = Join-Path $Repo 'artifacts\r6-c9fd-return'
$ConsoleLog = Join-Path $ReturnDir 'r6_windows_console.log'
New-Item -ItemType Directory -Force -Path $ReturnDir | Out-Null

$Started = (Get-Date).ToUniversalTime().ToString('o')
$ProofExit = 99
$ProofStatus = 'NOT_RUN'
$EvidenceZip = $null
$EvidenceZipSha = $null
$ReturnPush = 'NOT_ATTEMPTED'
$Failure = $null

try {
    Write-Host '=== SBC-7B2 R6 CODEMAGIC EXACT-SHA WINDOWS PROOF ==='
    Write-Host "Candidate: $Candidate"

    & git -C $Repo fetch origin $Candidate
    if ($LASTEXITCODE -ne 0) { throw 'Failed to fetch exact candidate SHA.' }
    & git -C $Repo checkout --detach $Candidate
    if ($LASTEXITCODE -ne 0) { throw 'Failed to checkout exact candidate SHA.' }

    $Head = (& git -C $Repo rev-parse HEAD).Trim()
    if ($Head -ne $Candidate) { throw "Candidate mismatch: $Head" }

    $Status = ((& git -C $Repo status --porcelain) -join "`n").Trim()
    if ($Status) { throw "Candidate checkout is not clean: $Status" }

    $ActualParent = (& git -C $Repo rev-parse 'HEAD^').Trim()
    if ($ActualParent -ne $Parent) { throw "Unexpected candidate parent: $ActualParent" }

    $Changed = @(& git -C $Repo diff --name-only "$Parent..HEAD")
    if ($Changed.Count -ne 1 -or $Changed[0] -ne $IconPath) {
        throw "Candidate is not exact one-file icon successor: $($Changed -join ', ')"
    }

    $IconBlob = (& git -C $Repo rev-parse "HEAD:$IconPath").Trim()
    if ($IconBlob -ne $ExpectedIconBlob) { throw "Unexpected icon blob: $IconBlob" }

    Write-Host '[PASS] Exact one-file RGBA successor identity verified.'

    $PreviousEAP = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $Repo 'ci\run_sbc7b2_supergate_windows.ps1') -ExpectedHead $Candidate 2>&1 |
            Tee-Object -FilePath $ConsoleLog
        $ProofExit = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $PreviousEAP
    }

    if ($ProofExit -eq 0) { $ProofStatus = 'PASS' } else { $ProofStatus = 'FAIL' }

    $Downloads = Join-Path $env:USERPROFILE 'Downloads'
    $EvidenceZip = Get-ChildItem -Path $Downloads -Filter 'SBC7B2_FT3_FT4_SUPERGATE_WINDOWS_*.zip' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1

    if ($EvidenceZip) {
        $EvidenceZipSha = (Get-FileHash -Algorithm SHA256 $EvidenceZip.FullName).Hash.ToLowerInvariant()
        Set-Content -Encoding ASCII (Join-Path $ReturnDir 'evidence_zip_sha256.txt') ($EvidenceZipSha + [Environment]::NewLine)
        Set-Content -Encoding UTF8 (Join-Path $ReturnDir 'evidence_zip_name.txt') ($EvidenceZip.Name + [Environment]::NewLine)

        $Extract = Join-Path $env:TEMP 'SBC7B2_R6_C9FD_EXTRACT'
        Remove-Item $Extract -Recurse -Force -ErrorAction SilentlyContinue
        Expand-Archive -Path $EvidenceZip.FullName -DestinationPath $Extract -Force
        foreach ($name in @('SUMMARY.json','SUMMARY.txt','CANDIDATE_SHA.txt','EXPECTED_SHA.txt','BASE_SHA.txt','LOCK_HASHES.json','TOOLCHAIN.json','CHANGED_PATHS.txt')) {
            $p = Join-Path $Extract $name
            if (Test-Path $p) { Copy-Item $p -Destination (Join-Path $ReturnDir $name) -Force }
        }
        $gateSrc = Join-Path $Extract 'gates'
        if (Test-Path $gateSrc) {
            Copy-Item $gateSrc -Destination (Join-Path $ReturnDir 'gates') -Recurse -Force
        }
    }
}
catch {
    $ProofStatus = 'HARNESS_FAIL'
    $Failure = $_.Exception.Message
    if ($ProofExit -eq 99) { $ProofExit = 98 }
    Write-Host "HARNESS FAILURE: $Failure"
}

$Completed = (Get-Date).ToUniversalTime().ToString('o')
$ConsoleSha = if (Test-Path $ConsoleLog) { (Get-FileHash -Algorithm SHA256 $ConsoleLog).Hash.ToLowerInvariant() } else { $null }
$Return = [ordered]@{
    schema = 'sbc7b2-r6-codemagic-return-v1'
    candidate_sha = $Candidate
    expected_parent_sha = $Parent
    expected_icon_blob = $ExpectedIconBlob
    proof_status = $ProofStatus
    proof_exit_code = $ProofExit
    started_at_utc = $Started
    completed_at_utc = $Completed
    evidence_zip_name = if ($EvidenceZip) { $EvidenceZip.Name } else { $null }
    evidence_zip_sha256 = $EvidenceZipSha
    console_sha256 = $ConsoleSha
    failure = $Failure
    candidate_mutated = $false
    merge = $false
    public_release = $false
}
$ReturnPath = Join-Path $ReturnDir 'R6_RETURN_STATUS.json'
$Return | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $ReturnPath

try {
    $EvidenceWork = Join-Path $env:TEMP 'SBC7B2_R6_C9FD_EVIDENCE_BRANCH'
    Remove-Item $EvidenceWork -Recurse -Force -ErrorAction SilentlyContinue

    & git -C $Repo fetch origin $EvidenceBranch
    if ($LASTEXITCODE -ne 0) { throw 'Failed to fetch evidence branch.' }
    & git -C $Repo worktree add --detach $EvidenceWork "origin/$EvidenceBranch"
    if ($LASTEXITCODE -ne 0) { throw 'Failed to create evidence worktree.' }

    $Target = Join-Path $EvidenceWork 'proof-return\r6-windows-c9fd'
    Remove-Item $Target -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    Copy-Item (Join-Path $ReturnDir '*') -Destination $Target -Recurse -Force

    & git -C $EvidenceWork config user.name 'MTD Shark SBC Proof Bot'
    & git -C $EvidenceWork config user.email 'sbc-proof@mtdshark.invalid'
    & git -C $EvidenceWork add -- 'proof-return/r6-windows-c9fd'
    & git -C $EvidenceWork commit -m "R6 Windows evidence: c9fd45ae $ProofStatus"
    if ($LASTEXITCODE -ne 0) { throw 'Failed to commit evidence return.' }
    & git -C $EvidenceWork push origin "HEAD:refs/heads/$EvidenceBranch"
    if ($LASTEXITCODE -ne 0) { throw 'Failed to push evidence return.' }
    $ReturnPush = 'PASS'
}
catch {
    $ReturnPush = 'FAIL'
    Write-Host "EVIDENCE RETURN PUSH FAILED: $($_.Exception.Message)"
}

Write-Host "R6 proof status: $ProofStatus"
Write-Host "R6 proof exit: $ProofExit"
Write-Host "Evidence return push: $ReturnPush"

if ($ProofStatus -ne 'PASS') { exit 1 }
if ($ReturnPush -ne 'PASS') { exit 2 }
exit 0
