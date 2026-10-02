$ErrorActionPreference='Stop'
$root=$env:CM_BUILD_DIR
if(-not $root){$root=(Get-Location).Path}
$crate=Join-Path $root 'research\sbc8a4-renderer-bakeoff\typst'
$bid=if($env:CM_BUILD_ID){$env:CM_BUILD_ID}else{'manual'}
$out=Join-Path $root ('artifacts\SBC8A4_TYPST_WINDOWS_RESULT_'+$bid)
$expected='8fe8c18f3074557d0251541e7de12c6d7ba62881a9bb9bbb22d4a52da45566cd'
New-Item -ItemType Directory -Force -Path $out | Out-Null
rustc --version | Set-Content -Encoding UTF8 (Join-Path $out 'rustc.txt')
cargo --version | Set-Content -Encoding UTF8 (Join-Path $out 'cargo.txt')
Set-Location $crate
cargo generate-lockfile
if($LASTEXITCODE -ne 0){throw "LOCK_GENERATION_FAILED:$LASTEXITCODE"}
$lock=(Get-FileHash Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant()
if($lock -ne $expected){throw "LOCK_MISMATCH:$lock"}
$lock | Set-Content -Encoding ASCII (Join-Path $out 'cargo_lock_sha256.txt')
cargo run --locked --release *>&1 | Tee-Object -FilePath (Join-Path $out 'host_run.txt')
if($LASTEXITCODE -ne 0){throw "CARGO_RUN_FAILED:$LASTEXITCODE"}
$ha=(Get-FileHash fixture-a.pdf -Algorithm SHA256).Hash.ToLowerInvariant()
$hb=(Get-FileHash fixture-b.pdf -Algorithm SHA256).Hash.ToLowerInvariant()
if($ha -ne $hb){throw 'NONDETERMINISTIC_PDF'}
Copy-Item fixture-a.pdf (Join-Path $out 'fixture-a.pdf')
cargo check --locked --release --lib *>&1 | Tee-Object -FilePath (Join-Path $out 'windows_check.txt')
if($LASTEXITCODE -ne 0){throw "WINDOWS_CHECK_FAILED:$LASTEXITCODE"}
cargo tree --locked --edges normal --prefix none | Set-Content -Encoding UTF8 (Join-Path $out 'cargo_tree.txt')
cargo metadata --locked --format-version 1 | Set-Content -Encoding UTF8 (Join-Path $out 'cargo_metadata.json')
Copy-Item Cargo.lock (Join-Path $out 'Cargo.lock')
$targetBytes=(Get-ChildItem target -File -Recurse -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
$bytes=(Get-Item fixture-a.pdf).Length
$summary=[ordered]@{schema_version=1;candidate='Typst embedded';typst='0.15.1';typst_as_lib='0.16.0';platform='CodeMagic windows_x2';host_render='PASS';windows_check='PASS';deterministic_pdf=$true;pdf_bytes=$bytes;pdf_sha256=$ha;cargo_lock_sha256=$expected;target_bytes=$targetBytes;commit=$env:CM_COMMIT;build_id=$bid;remote_resources_used=$false;completed_at_utc=(Get-Date).ToUniversalTime().ToString('o')}
$summary | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $out 'SUMMARY.json')
$zip=Join-Path (Split-Path $out -Parent) ('SBC8A4_TYPST_WINDOWS_RESULT_'+$bid+'.zip')
Compress-Archive -Path $out -DestinationPath $zip -CompressionLevel Optimal
Get-Content (Join-Path $out 'SUMMARY.json') -Raw
