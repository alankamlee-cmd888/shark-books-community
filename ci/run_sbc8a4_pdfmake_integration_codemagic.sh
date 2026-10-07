#!/bin/bash
set -euo pipefail

BASE="b4ec9674eec233facc4e75da7bdceddaab92f3c8"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.98.1}"
REPO="${CM_BUILD_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
UI="$REPO/workspace/ui"
WORKSPACE="$REPO/workspace"
ARTIFACTS="$REPO/artifacts"
TS="$(date +%Y%m%d_%H%M%S)"
RESULT="$ARTIFACTS/SBC8A4_PDFMAKE_APPLE_$TS"
LOGS="$RESULT/logs"
mkdir -p "$LOGS" "$ARTIFACTS"

run_log() {
  label="$1"; shift
  "$@" >"$LOGS/$label.stdout.txt" 2>"$LOGS/$label.stderr.txt"
}

HEAD="$(git -C "$REPO" rev-parse HEAD)"
EXPECTED="${CM_COMMIT:-$HEAD}"
[ "$HEAD" = "$EXPECTED" ] || { echo "candidate mismatch"; exit 20; }
[ -z "$(git -C "$REPO" status --porcelain)" ] || { echo "repository dirty at entry"; exit 21; }
git -C "$REPO" merge-base --is-ancestor "$BASE" "$HEAD" || { echo "frozen base not ancestor"; exit 22; }

python3 - "$REPO" "$BASE" <<'PY'
import subprocess, sys
repo, base = sys.argv[1], sys.argv[2]
exact = {
  "workspace/ui/package.json",
  "workspace/ui/package-lock.json",
  "workspace/ui/src/lib/commercialDocumentRenderer.ts",
  "workspace/shark-tauri-spike/src/owner_commercial_renderer.rs",
  "workspace/ui/src/lib/__tests__/commercialDocumentRenderer.spec.ts",
  "workspace/ui/src/lib/tauri.ts",
  "workspace/ui/src/screens/QuotesScreen.vue",
  "workspace/ui/src/screens/InvoicesScreen.vue",
  "workspace/ui/src/screens/ReportsScreen.vue",
  "workspace/shark-tauri-spike/src/lib.rs",
  "workspace/shark-tauri-spike/src/owner_documents_ocr.rs",
  "workspace/shark-tauri-spike/src/owner_supporting_data.rs",
  "ci/run_sbc8a4_pdfmake_integration_windows.ps1",
  "ci/run_sbc8a4_pdfmake_integration_codemagic.sh",
  "codemagic.yaml",
}
prefix = ("workspace/ui/src/fixtures/sbc8a4/","workspace/shark-tauri-spike/tests/sbc8a4/")
changed = subprocess.check_output(["git","-C",repo,"diff","--name-only",f"{base}..HEAD"], text=True).splitlines()
bad = [p for p in changed if p not in exact and not p.startswith(prefix)]
if bad:
    raise SystemExit("changed path outside frozen allow-list: " + ", ".join(bad))
PY

run_log bootstrap_beankeeper python3 "$REPO/scripts/bootstrap_beankeeper.py"
rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal
rustup default "$RUST_TOOLCHAIN"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
run_log rust_version rustc --version

python3 - "$UI/package.json" <<'PY'
import json, sys
p=json.load(open(sys.argv[1], encoding="utf-8"))
assert p["dependencies"]["pdfmake"] == "0.3.11"
PY

run_log npm_ci npm --prefix "$UI" ci
if ! npm --prefix "$UI" audit --json >"$RESULT/npm-audit.json"; then
  cat "$RESULT/npm-audit.json"
  exit 30
fi
python3 - "$RESULT/npm-audit.json" <<'PY'
import json, sys
j=json.load(open(sys.argv[1], encoding="utf-8"))
assert j["metadata"]["vulnerabilities"]["total"] == 0
PY
run_log ui_typecheck npm --prefix "$UI" run type-check
run_log renderer_determinism npm --prefix "$UI" run test:sbc8a4
run_log ui_build npm --prefix "$UI" run build
git -C "$REPO" restore --source=HEAD --worktree --staged -- workspace/dist
git -C "$REPO" clean -fd -- workspace/dist

run_log rust_bounded_renderer cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --locked owner_commercial_renderer -- --nocapture
run_log rust_workspace_check cargo check --manifest-path "$WORKSPACE/Cargo.toml" --workspace --locked --jobs 1

ICON="$WORKSPACE/shark-tauri-spike/icons/icon.png"
ICON_BACKUP="$RESULT/icon.original.png"
cp "$ICON" "$ICON_BACKUP"
restore_icon() {
  if [ -f "$ICON_BACKUP" ]; then cp "$ICON_BACKUP" "$ICON"; fi
}
trap restore_icon EXIT
run_log rgba_icon python3 "$REPO/ci/prepare_rgba_png.py" "$ICON"
run_log ios_physical cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios --locked --jobs 1
run_log ios_simulator cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios-sim --locked --jobs 1
restore_icon
trap - EXIT

python3 - "$UI/src/lib/commercialDocumentRenderer.ts" <<'PY'
import pathlib, re, sys
s=pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
assert not re.search(r"https?://|fetch\s*\(|XMLHttpRequest|WebSocket|EventSource", s, re.I)
assert "pdfmake/build/pdfmake.js" in s
assert "pdfmake/build/vfs_fonts.js" in s
PY

[ "$(git -C "$REPO" rev-parse HEAD)" = "$HEAD" ] || { echo "candidate moved"; exit 40; }
[ -z "$(git -C "$REPO" status --porcelain)" ] || { git -C "$REPO" status --short; exit 41; }

cat >"$RESULT/SUMMARY.json" <<EOF
{
  "schema": "sbc8a4-pdfmake-production-integration-apple-v1",
  "overall": "PASS",
  "candidate_sha": "$HEAD",
  "base_sha": "$BASE",
  "pdfmake": "0.3.11",
  "npm_audit_total": 0,
  "physical_target": "aarch64-apple-ios",
  "simulator_target": "aarch64-apple-ios-sim"
}
EOF
cat >"$ARTIFACTS/SBC8A4_PDFMAKE_APPLE_SUMMARY.txt" <<EOF
SBC8A4 PDFMake production integration Apple proof
Overall: PASS
Candidate: $HEAD
Base: $BASE
pdfmake: 0.3.11
Targets: aarch64-apple-ios, aarch64-apple-ios-sim
EOF
(cd "$ARTIFACTS" && zip -qry "SBC8A4_PDFMAKE_APPLE_RESULT_$TS.zip" "$(basename "$RESULT")")
echo "[PASS] SBC8A4 PDFMake Apple proof candidate $HEAD"
