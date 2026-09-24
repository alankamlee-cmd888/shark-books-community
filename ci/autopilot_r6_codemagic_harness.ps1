param()

$ErrorActionPreference = 'Stop'
$Candidate = 'c9fd45ae67e977ba986706308ac6fae2d3bdf731'
$RepoUrl = 'https://github.com/alankamlee-cmd888/shark-books-community.git'
$CandidateDir = Join-Path $env:TEMP 'SBC7B2-R6-AUTOPILOT-CANDIDATE'
$ArtifactDir = Join-Path $env:CM_BUILD_DIR 'autopilot-artifacts'

if (Test-Path $CandidateDir) { Remove-Item $CandidateDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $ArtifactDir | Out-Null

git clone $RepoUrl $CandidateDir
if ($LASTEXITCODE -ne 0) { throw 'Candidate clone failed.' }
git -C $CandidateDir checkout --detach $Candidate
if ($LASTEXITCODE -ne 0) { throw 'Candidate checkout failed.' }

$Head = (& git -C $CandidateDir rev-parse HEAD).Trim()
if ($Head -ne $Candidate) { throw "Exact candidate mismatch: $Head" }
if (@(& git -C $CandidateDir status --porcelain).Count -ne 0) { throw 'Candidate checkout is not clean.' }

$Runner = Join-Path $CandidateDir 'ci\run_sbc7b2_supergate_windows.ps1'
if (-not (Test-Path $Runner)) { throw 'Candidate R6 runner missing.' }

& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Runner -ExpectedHead $Candidate
if ($LASTEXITCODE -ne 0) { throw "R6 runner failed with exit code $LASTEXITCODE" }

$Zip = Get-ChildItem (Join-Path $env:USERPROFILE 'Downloads') -Filter 'SBC7B2_FT3_FT4_SUPERGATE_WINDOWS_*.zip' |
    Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if ($null -eq $Zip) { throw 'R6 evidence ZIP not found.' }

$ZipSha = (Get-FileHash -Algorithm SHA256 $Zip.FullName).Hash.ToLowerInvariant()
Copy-Item $Zip.FullName (Join-Path $ArtifactDir $Zip.Name) -Force

$Summary = [ordered]@{
    schema_version = 1
    programme = 'SBC-7B2'
    stage = 'R6_WINDOWS_EXACT_SHA'
    disposition = 'PASS_PENDING_INDEPENDENT_ADJUDICATION'
    candidate_sha = $Candidate
    observed_head = $Head
    evidence_zip_name = $Zip.Name
    evidence_zip_sha256 = $ZipSha
    runner = 'ci/run_sbc7b2_supergate_windows.ps1'
    codemagic_workflow = 'sbc7b2-r6-autopilot-exact'
    source_candidate_modified = $false
    merge = $false
    public_release = $false
}
$Summary | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $ArtifactDir 'R6_AUTOPILOT_SUMMARY.json')

Write-Host "PASS_R6_AUTOPILOT_EXACT_SHA"
Write-Host "CANDIDATE_SHA=$Candidate"
Write-Host "EVIDENCE_ZIP=$($Zip.Name)"
Write-Host "EVIDENCE_SHA256=$ZipSha"
