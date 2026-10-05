#!/usr/bin/env bash
set -euo pipefail

ROOT="${CM_BUILD_DIR:-$(pwd)}"
PDF="$ROOT/research/sbc8a4-renderer-bakeoff/pdfmake"
BRIDGE="$ROOT/research/sbc8a4-renderer-bakeoff/pdfmake-bridge"
BID="${CM_BUILD_ID:-manual}"
OUT="$ROOT/artifacts/SBC8A4_PDFMAKE_APPLE_RESULT_${BID}"
STORE="$OUT/store"
EXPECTED_PDF_SHA="521d8d37297e1fed6c1367a4edcec413cb60c3b62d6ad058edfb09aa53fbc990"
EXPECTED_PACKAGE_LOCK="164514d382b98268aaec7177772cd67c3e2113e09486f89d74baf17229093363"

mkdir -p "$OUT" "$STORE"
export PATH="$HOME/.cargo/bin:$PATH"

rustc --version | tee "$OUT/rustc.txt"
cargo --version | tee "$OUT/cargo.txt"
node --version | tee "$OUT/node.txt"
npm --version | tee "$OUT/npm.txt"

cd "$PDF"
ACTUAL_PACKAGE_LOCK="$(shasum -a 256 package-lock.json | awk '{print $1}')"
test "$ACTUAL_PACKAGE_LOCK" = "$EXPECTED_PACKAGE_LOCK"
echo "$ACTUAL_PACKAGE_LOCK" > "$OUT/package_lock_sha256.txt"

npm ci --ignore-scripts --no-audit --no-fund | tee "$OUT/npm_ci.txt"
set +e
npm audit --json > "$OUT/npm_audit.json"
AUDIT_RC=$?
set -e
if [ "$AUDIT_RC" -ne 0 ]; then
  echo "NPM_AUDIT_FAILED:$AUDIT_RC" >&2
  cat "$OUT/npm_audit.json" >&2
  exit "$AUDIT_RC"
fi

cd "$BRIDGE"
cargo build --release --locked | tee "$OUT/cargo_build_host.txt"
BRIDGE_EXE="$BRIDGE/target/release/sbc8a4-pdfmake-rust-bridge-proof"

node "$PDF/emit_pdfmake_to_rust.js" "$BRIDGE_EXE" "$STORE" | tee "$OUT/bridge_positive.json"

rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo check --locked --target aarch64-apple-ios 2>&1 | tee "$OUT/ios_device_check.txt"
cargo check --locked --target aarch64-apple-ios-sim 2>&1 | tee "$OUT/ios_sim_check.txt"

python3 - "$OUT" "${CM_COMMIT:-unknown}" "$BID" "$EXPECTED_PDF_SHA" <<'PY'
import hashlib,json,os,sys,datetime
out,commit,bid,expected_pdf=sys.argv[1:5]
bridge=json.load(open(os.path.join(out,"bridge_positive.json"),encoding="utf-8"))
audit=json.load(open(os.path.join(out,"npm_audit.json"),encoding="utf-8"))
vulns=(audit.get("metadata") or {}).get("vulnerabilities") or {}
if bridge.get("pdf_sha256") != expected_pdf:
    raise SystemExit("PDF_SHA_MISMATCH:"+str(bridge.get("pdf_sha256")))
if int(vulns.get("total",0)) != 0:
    raise SystemExit("NPM_AUDIT_NONZERO:"+json.dumps(vulns,sort_keys=True))
summary={
 "schema_version":1,
 "candidate":"pdfmake",
 "pdfmake":"0.3.11",
 "platform":"CodeMagic mac_mini_m2",
 "host_bridge_proof":"PASS",
 "ios_device_compile_check":"PASS",
 "ios_simulator_compile_check":"PASS",
 "physical_ios_runtime_proof":False,
 "ios_simulator_runtime_proof":False,
 "deterministic_pdf":bool(bridge.get("deterministic_pdf")),
 "pdf_bytes":bridge.get("pdf_bytes"),
 "pdf_sha256":bridge.get("pdf_sha256"),
 "rust_save_store":bridge.get("rust_save_store"),
 "npm_audit_vulnerabilities":vulns,
 "package_lock_sha256":open(os.path.join(out,"package_lock_sha256.txt"),encoding="utf-8").read().strip(),
 "commit":commit,
 "build_id":bid,
 "remote_resources_used_for_render":False,
 "completed_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00","Z")
}
open(os.path.join(out,"SUMMARY.json"),"w",encoding="utf-8").write(json.dumps(summary,indent=2)+"\n")
print(json.dumps(summary,indent=2))
PY

(
  cd "$ROOT/artifacts"
  zip -qr "SBC8A4_PDFMAKE_APPLE_RESULT_${BID}.zip" "SBC8A4_PDFMAKE_APPLE_RESULT_${BID}"
)
