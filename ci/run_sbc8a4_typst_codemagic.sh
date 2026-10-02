#!/usr/bin/env bash
set -euo pipefail
ROOT="${CM_BUILD_DIR:-$(pwd)}"
CRATE="$ROOT/research/sbc8a4-renderer-bakeoff/typst"
OUT="$ROOT/artifacts/SBC8A4_TYPST_MAC_RESULT_${CM_BUILD_ID:-manual}"
EXPECTED_LOCK="8fe8c18f3074557d0251541e7de12c6d7ba62881a9bb9bbb22d4a52da45566cd"
mkdir -p "$OUT"
export PATH="$HOME/.cargo/bin:$PATH"
rustc --version | tee "$OUT/rustc.txt"
cargo --version | tee "$OUT/cargo.txt"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cd "$CRATE"
cargo generate-lockfile
ACTUAL_LOCK="$(shasum -a 256 Cargo.lock | awk '{print $1}')"
test "$ACTUAL_LOCK" = "$EXPECTED_LOCK"
echo "$ACTUAL_LOCK" > "$OUT/cargo_lock_sha256.txt"
cargo run --locked --release | tee "$OUT/host_run.txt"
shasum -a 256 fixture-a.pdf fixture-b.pdf | tee "$OUT/pdf_sha256.txt"
cmp fixture-a.pdf fixture-b.pdf
cp fixture-a.pdf "$OUT/"
cargo check --locked --release --lib --target aarch64-apple-ios 2>&1 | tee "$OUT/ios_check.txt"
cargo check --locked --release --lib --target aarch64-apple-ios-sim 2>&1 | tee "$OUT/ios_sim_check.txt"
cargo tree --locked --edges normal --prefix none > "$OUT/cargo_tree.txt"
cargo metadata --locked --format-version 1 > "$OUT/cargo_metadata.json"
du -sk target | tee "$OUT/target_kib.txt"
cp Cargo.lock "$OUT/Cargo.lock"
python3 - <<'PY' "$OUT" "${CM_COMMIT:-unknown}" "${CM_BUILD_ID:-unknown}"
import json,sys,hashlib,os,glob,datetime
out,commit,build=sys.argv[1:4]
pdf=glob.glob(os.path.join(out,"fixture-a.pdf"))[0]
b=open(pdf,"rb").read()
result={"schema_version":1,"candidate":"Typst embedded","typst":"0.15.1","typst_as_lib":"0.16.0",
"platform":"CodeMagic mac_mini_m2","host_render":"PASS","ios_check":"PASS","ios_sim_check":"PASS",
"deterministic_pdf":True,"pdf_bytes":len(b),"pdf_sha256":hashlib.sha256(b).hexdigest(),
"cargo_lock_sha256":"8fe8c18f3074557d0251541e7de12c6d7ba62881a9bb9bbb22d4a52da45566cd",
"commit":commit,"build_id":build,"remote_resources_used":False,
"completed_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00","Z")}
open(os.path.join(out,"SUMMARY.json"),"w").write(json.dumps(result,indent=2)+"\n")
PY
(cd "$ROOT/artifacts" && zip -qr "SBC8A4_TYPST_MAC_RESULT_${CM_BUILD_ID:-manual}.zip" "SBC8A4_TYPST_MAC_RESULT_${CM_BUILD_ID:-manual}")
