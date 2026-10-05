#!/usr/bin/env bash
set -euo pipefail

ROOT="${CM_BUILD_DIR:-$(pwd)}"
BID="${CM_BUILD_ID:-manual}"
BASE_OUT="$ROOT/artifacts/SBC8A4_PDFMAKE_APPLE_RESULT_$BID"
OUT="$ROOT/artifacts/SBC8A4_PDFMAKE_SIM_RESULT_$BID"
BRIDGE="$ROOT/research/sbc8a4-renderer-bakeoff/pdfmake-bridge"
EXPECTED_PDF_SHA="521d8d37297e1fed6c1367a4edcec413cb60c3b62d6ad058edfb09aa53fbc990"
EXPECTED_PDF_BYTES="35287"
BUNDLE_ID="com.mtdshark.sbc8a4.pdfmake.runtimeprobe"
APP="$OUT/PdfmakeRuntimeProbe.app"

mkdir -p "$OUT"

chmod +x "$ROOT/ci/run_sbc8a4_pdfmake_codemagic.sh"
"$ROOT/ci/run_sbc8a4_pdfmake_codemagic.sh"

FIXTURE="$BASE_OUT/store/generated/$EXPECTED_PDF_SHA/invoice-proof.pdf"
test -f "$FIXTURE"
test "$(stat -f%z "$FIXTURE")" = "$EXPECTED_PDF_BYTES"
test "$(shasum -a 256 "$FIXTURE" | awk '{print $1}')" = "$EXPECTED_PDF_SHA"
cp "$BASE_OUT/SUMMARY.json" "$OUT/base_apple_summary.json"

export PATH="$HOME/.cargo/bin:$PATH"
rustup target add aarch64-apple-ios-sim
cd "$BRIDGE"
cargo fmt --check
cargo build --release --locked --target aarch64-apple-ios-sim --bin ios_runtime_probe \
  2>&1 | tee "$OUT/simulator_probe_build.txt"

mkdir -p "$APP"
cp "$BRIDGE/target/aarch64-apple-ios-sim/release/ios_runtime_probe" "$APP/PdfmakeRuntimeProbe"
cp "$FIXTURE" "$APP/fixture.pdf"
chmod +x "$APP/PdfmakeRuntimeProbe"

cat > "$APP/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>SBC8A4 pdfmake runtime probe</string>
  <key>CFBundleExecutable</key><string>PdfmakeRuntimeProbe</string>
  <key>CFBundleIdentifier</key><string>com.mtdshark.sbc8a4.pdfmake.runtimeprobe</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>PdfmakeRuntimeProbe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSRequiresIPhoneOS</key><true/>
  <key>MinimumOSVersion</key><string>17.0</string>
  <key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
</dict>
</plist>
PLIST

plutil -lint "$APP/Info.plist" | tee "$OUT/plist_lint.txt"
file "$APP/PdfmakeRuntimeProbe" | tee "$OUT/simulator_binary_file.txt"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP" 2>&1 | tee "$OUT/codesign_verify.txt"

SIM_UDID="$(xcrun simctl list devices available -j | python3 -c '
import json,sys
d=json.load(sys.stdin)
for runtime,devices in d.get("devices",{}).items():
    if ".iOS-" not in runtime:
        continue
    for x in devices:
        if x.get("isAvailable") and str(x.get("name","")).startswith("iPhone"):
            print(x["udid"]); raise SystemExit
raise SystemExit("NO_AVAILABLE_IPHONE_SIMULATOR")
')"
echo "$SIM_UDID" | tee "$OUT/simulator_udid.txt"

xcrun simctl boot "$SIM_UDID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$SIM_UDID" -b 2>&1 | tee "$OUT/simulator_boot.txt"
xcrun simctl install "$SIM_UDID" "$APP" 2>&1 | tee "$OUT/simulator_install.txt"
xcrun simctl launch "$SIM_UDID" "$BUNDLE_ID" 2>&1 | tee "$OUT/simulator_launch.txt"

DATA_DIR="$(xcrun simctl get_app_container "$SIM_UDID" "$BUNDLE_ID" data)"
MARKER="$DATA_DIR/Documents/SBC8A4_PDFMAKE_SIM_RUNTIME.json"
for _ in $(seq 1 30); do
  [ -f "$MARKER" ] && break
  sleep 1
done
test -f "$MARKER"
cp "$MARKER" "$OUT/simulator_runtime.json"

python3 - "$OUT/simulator_runtime.json" "$OUT/base_apple_summary.json" "${CM_COMMIT:-unknown}" "$BID" <<'PY'
import json,sys,os,datetime
runtime_path,base_path,commit,bid=sys.argv[1:5]
runtime=json.load(open(runtime_path,encoding="utf-8"))
base=json.load(open(base_path,encoding="utf-8"))
expected="521d8d37297e1fed6c1367a4edcec413cb60c3b62d6ad058edfb09aa53fbc990"
if runtime.get("simulator_runtime")!="PASS": raise SystemExit("SIM_RUNTIME_NOT_PASS")
if runtime.get("fixture_sha256")!=expected: raise SystemExit("SIM_FIXTURE_SHA_MISMATCH")
if int(runtime.get("fixture_bytes",0))!=35287: raise SystemExit("SIM_FIXTURE_BYTES_MISMATCH")
receipt=runtime.get("rust_save_store") or {}
if receipt.get("status")!="PASS" or receipt.get("sha256")!=expected: raise SystemExit("SIM_STORE_MISMATCH")
if base.get("ios_device_compile_check")!="PASS": raise SystemExit("PHYSICAL_IOS_TARGET_COMPILE_NOT_PASS")
if base.get("ios_simulator_compile_check")!="PASS": raise SystemExit("SIM_TARGET_COMPILE_NOT_PASS")
summary={
 "schema_version":1,
 "candidate":"pdfmake",
 "pdfmake":"0.3.11",
 "platform":"CodeMagic mac_mini_m2 + iOS Simulator",
 "physical_ios_target_compile":"PASS",
 "ios_simulator_target_compile":"PASS",
 "ios_simulator_application_build":"PASS",
 "ios_simulator_runtime":"PASS",
 "deterministic_pdf":True,
 "pdf_sha256":expected,
 "pdf_bytes":35287,
 "rust_save_store_runtime":"PASS",
 "production_source_mutated":False,
 "production_dependency_activated":False,
 "commit":commit,
 "build_id":bid,
 "completed_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00","Z")
}
json.dump(summary,open(os.path.join(os.path.dirname(runtime_path),"SUMMARY.json"),"w",encoding="utf-8"),indent=2)
print(json.dumps(summary,indent=2))
PY

xcrun simctl shutdown "$SIM_UDID" >/dev/null 2>&1 || true

(
  cd "$ROOT/artifacts"
  zip -qr "SBC8A4_PDFMAKE_SIM_RESULT_$BID.zip" "SBC8A4_PDFMAKE_SIM_RESULT_$BID"
)
