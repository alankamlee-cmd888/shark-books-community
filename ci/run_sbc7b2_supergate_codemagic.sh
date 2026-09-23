#!/bin/bash
set -u
set -o pipefail

BASE="ebf1ae9d0e10f0f427625d4c3ab5c1703299d154"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.98.1}"
REPO_ROOT="${CM_BUILD_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
WORKSPACE="$REPO_ROOT/workspace"
ARTIFACTS="$REPO_ROOT/artifacts"
TS="$(date +%Y%m%d_%H%M%S)"
RESULT_DIR="$ARTIFACTS/SBC7B2_FT3_FT4_SUPERGATE_MAC_$TS"
GATE_DIR="$RESULT_DIR/gates"
LOG_DIR="$RESULT_DIR/logs"
RESULT_ZIP="$ARTIFACTS/SBC7B2_FT3_FT4_SUPERGATE_MAC_CODEMAGIC_RESULT_$TS.zip"
SUMMARY_TXT="$ARTIFACTS/SBC7B2_FT3_FT4_SUPERGATE_MAC_SUMMARY.txt"
SUMMARY_JSON="$ARTIFACTS/SBC7B2_FT3_FT4_SUPERGATE_MAC_SUMMARY.json"
CARGO_TARGET_DIR="${TMPDIR:-/tmp}/SharkBooks-SBC7B2-FT3FT4-CargoTarget"
PROOF_BOOKS_DIR="${TMPDIR:-/tmp}/SharkBooks-SBC7B2-FT3FT4-ProofBooks"
export CARGO_TARGET_DIR
export SHARK_SBC1D_PROOF_KEY="SBC7B2-FT3FT4-Proof-Key-Only-Do-Not-Ship"
export SHARK_SBC1D_BOOKS_DIR="$PROOF_BOOKS_DIR"

OVERALL="FAIL"
CLASSIFICATION="UNRESOLVED"
PHASE="SG0_PREFLIGHT"
HEAD=""
LOCK_SHA_START=""
LOCK_BLOB_EXPECTED=""
ICON_BACKUP=""
mkdir -p "$GATE_DIR" "$LOG_DIR" "$ARTIFACTS"
rm -rf "$CARGO_TARGET_DIR" "$PROOF_BOOKS_DIR"
mkdir -p "$CARGO_TARGET_DIR" "$PROOF_BOOKS_DIR"

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.read())[1:-1])'
}

write_gate() {
  gate="$1"; status="$2"; mandatory="$3"; summary="$4"
  cat > "$GATE_DIR/$gate.json" <<EOF
{"gate":"$gate","status":"$status","mandatory":$mandatory,"summary":"$(printf '%s' "$summary" | json_escape)","candidate_sha":"$HEAD"}
EOF
}

run_log() {
  label="$1"; shift
  "$@" >"$LOG_DIR/$label.stdout.txt" 2>"$LOG_DIR/$label.stderr.txt"
}

stop() {
  gate="$1"; classification="$2"; summary="$3"; code="${4:-1}"
  write_gate "$gate" "FAIL" true "$summary"
  CLASSIFICATION="$classification"
  exit "$code"
}

finish() {
  rc=$?
  set +e
  if [ -n "$ICON_BACKUP" ] && [ -f "$ICON_BACKUP" ]; then
    cp "$ICON_BACKUP" "$WORKSPACE/shark-tauri-spike/icons/icon.png"
  fi
  FINAL_HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || true)"
  FINAL_LOCK_SHA="$(shasum -a 256 "$WORKSPACE/Cargo.lock" 2>/dev/null | awk '{print $1}')"
  git -C "$REPO_ROOT" status --short > "$RESULT_DIR/git_status_at_finish.txt" 2>/dev/null || true
  printf '%s\n' "$FINAL_HEAD" > "$RESULT_DIR/CANDIDATE_SHA.txt"
  printf '%s\n' "${CM_COMMIT:-$FINAL_HEAD}" > "$RESULT_DIR/EXPECTED_SHA.txt"
  printf '%s\n' "$BASE" > "$RESULT_DIR/BASE_SHA.txt"
  git -C "$REPO_ROOT" diff --name-only "$BASE..HEAD" > "$RESULT_DIR/CHANGED_PATHS.txt" 2>/dev/null || true
  cat > "$RESULT_DIR/LOCK_HASHES.json" <<EOF
{"workspace_cargo_lock_sha256":"$FINAL_LOCK_SHA","workspace_cargo_lock_git_blob":"$LOCK_BLOB_EXPECTED"}
EOF
  cat > "$RESULT_DIR/TOOLCHAIN.json" <<EOF
{"rust_toolchain":"$RUST_TOOLCHAIN","platform":"macos-apple-silicon","physical_target":"aarch64-apple-ios","simulator_target":"aarch64-apple-ios-sim"}
EOF
  cat > "$SUMMARY_TXT" <<EOF
SBC-7B2 FT3/FT4 Mac/Apple Super-Gate
Overall: $OVERALL
Classification: $CLASSIFICATION
Phase: $PHASE
Repository commit: ${CM_COMMIT:-$FINAL_HEAD}
Entry protected main: $BASE
Cargo.lock SHA-256: $FINAL_LOCK_SHA
Rust toolchain: $RUST_TOOLCHAIN
EOF
  cat > "$SUMMARY_JSON" <<EOF
{"schema":"sbc7b2-ft3-ft4-supergate-mac-v1","overall":"$OVERALL","classification":"$CLASSIFICATION","phase":"$PHASE","candidate_sha":"$(printf '%s' "${CM_COMMIT:-$FINAL_HEAD}" | json_escape)","base_sha":"$BASE","cargo_lock_sha256":"$FINAL_LOCK_SHA","rust_toolchain":"$RUST_TOOLCHAIN","mandatory_gates":["SG0_PREFLIGHT","SG1_ACTION_STACK","SG2_CONTROLLER","SG3_UI_A","SG4_UI_B","SG5_COMMAND_TEXT","SG6_REGRESSIONS","SG7_PLATFORM_COMPILE","SG8_FINAL_INTEGRITY"]}
EOF
  cp "$SUMMARY_TXT" "$RESULT_DIR/SUMMARY.txt" 2>/dev/null || true
  cp "$SUMMARY_JSON" "$RESULT_DIR/SUMMARY.json" 2>/dev/null || true
  rm -f "$RESULT_ZIP"
  if command -v ditto >/dev/null 2>&1; then
    ditto -c -k --sequesterRsrc --keepParent "$RESULT_DIR" "$RESULT_ZIP" 2>/dev/null || true
  fi
  if [ ! -f "$RESULT_ZIP" ]; then
    (cd "$ARTIFACTS" && zip -qry "$RESULT_ZIP" "$(basename "$RESULT_DIR")") || true
  fi
  rm -rf "$CARGO_TARGET_DIR" "$PROOF_BOOKS_DIR"
  exit "$rc"
}
trap finish EXIT

# SG0 — immutable candidate preflight
[ "$(uname -s)" = "Darwin" ] || stop "SG0_PREFLIGHT" "HOST_NOT_MACOS" "Host is not macOS" 20
[ "$(uname -m)" = "arm64" ] || stop "SG0_PREFLIGHT" "HOST_NOT_ARM64" "Host is not Apple Silicon arm64" 21
for tool in git python3 shasum zip rustup cargo; do
  command -v "$tool" >/dev/null 2>&1 || stop "SG0_PREFLIGHT" "HOST_TOOL_MISSING" "$tool is required" 22
done
[ -z "$(git -C "$REPO_ROOT" status --porcelain)" ] || stop "SG0_PREFLIGHT" "REPOSITORY_DIRTY_AT_ENTRY" "Repository must be clean at entry" 23
HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD)"
if [ -n "${CM_COMMIT:-}" ] && [ "$HEAD" != "$CM_COMMIT" ]; then
  stop "SG0_PREFLIGHT" "CODEMAGIC_COMMIT_MISMATCH" "Checked-out HEAD does not equal CM_COMMIT" 24
fi
git -C "$REPO_ROOT" merge-base --is-ancestor "$BASE" "$HEAD" || stop "SG0_PREFLIGHT" "BASE_NOT_ANCESTOR" "Protected post-7B1 main is not an ancestor" 25
run_log sg0_toolchain_install rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal || stop "SG0_PREFLIGHT" "RUST_INSTALL_FAIL" "Rust toolchain install failed" 26
rustup default "$RUST_TOOLCHAIN" >"$LOG_DIR/sg0_rust_default.stdout.txt" 2>"$LOG_DIR/sg0_rust_default.stderr.txt" || stop "SG0_PREFLIGHT" "RUST_SELECT_FAIL" "Rust toolchain selection failed" 27
run_log sg0_apple_targets rustup target add aarch64-apple-ios aarch64-apple-ios-sim || stop "SG0_PREFLIGHT" "APPLE_TARGET_INSTALL_FAIL" "Apple Rust target installation failed" 28
run_log sg0_bootstrap python3 "$REPO_ROOT/scripts/bootstrap_beankeeper.py" || stop "SG0_PREFLIGHT" "BEANKEEPER_BOOTSTRAP_FAIL" "Exact Beankeeper bootstrap failed" 29
LOCK_SHA_START="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
LOCK_BLOB_EXPECTED="$(git -C "$REPO_ROOT" rev-parse 'HEAD:workspace/Cargo.lock')"
LOCK_BLOB_OBSERVED="$(git -C "$REPO_ROOT" hash-object "$WORKSPACE/Cargo.lock")"
[ "$LOCK_BLOB_EXPECTED" = "$LOCK_BLOB_OBSERVED" ] || stop "SG0_PREFLIGHT" "LOCK_NOT_CANDIDATE_BYTES" "Working Cargo.lock differs from candidate blob" 30
write_gate "SG0_PREFLIGHT" "PASS" true "Exact candidate, clean tree, Rust/Apple targets, bootstrap and candidate lock identity verified."

# SG1 — Action Stack / generated contracts
PHASE="SG1_ACTION_STACK"
run_log sg1_static python3 "$REPO_ROOT/scripts/check_sbc7b2_ft1_ft2.py" --repo "$REPO_ROOT" --skip-git || stop "SG1_ACTION_STACK" "STATIC_GATE_FAIL" "Integrated FT1/FT2 static gate failed" 40
run_log sg1_generator_check cargo run --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --example action-contract --features action-contract-gen --locked -- --check || stop "SG1_ACTION_STACK" "GENERATED_CONTRACT_DRIFT" "Generated schema/TypeScript check failed" 41
run_log sg1_dependency_tree cargo tree --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --features action-contract-gen --locked || stop "SG1_ACTION_STACK" "DEPENDENCY_TREE_FAIL" "Locked dependency tree failed" 42
grep -q 'schemars v1.2.2' "$LOG_DIR/sg1_dependency_tree.stdout.txt" || stop "SG1_ACTION_STACK" "SCHEMARS_PIN_FAIL" "Schemars 1.2.2 missing from tree" 43
grep -q 'ts-rs v12.0.1' "$LOG_DIR/sg1_dependency_tree.stdout.txt" || stop "SG1_ACTION_STACK" "TSRS_PIN_FAIL" "ts-rs 12.0.1 missing from tree" 44
write_gate "SG1_ACTION_STACK" "PASS" true "206/175/700 invariants, generated contracts and exact reuse pins verified."

# SG2 — controller / Attention Queue
PHASE="SG2_CONTROLLER"
run_log sg2_action_system cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --test action_system_contract --locked --jobs 1 -- --test-threads=1 || stop "SG2_CONTROLLER" "ACTION_CONTROLLER_FAIL" "Action System controller contract tests failed" 50
write_gate "SG2_CONTROLLER" "PASS" true "Deterministic controller, clarification, confirmation, stale-state, replay and Attention tests pass."

# SG3 — integrated UI-A contract and type-safe production frontend
PHASE="SG3_UI_A"
run_log sg3_uia_generator python3 "$REPO_ROOT/scripts/generate_sbc7b2_uia_actions.py" --check || stop "SG3_UI_A" "UIA_GENERATOR_FAIL" "UI-A generated metadata drift" 55
run_log sg3_uia_static python3 "$REPO_ROOT/scripts/check_sbc7b2_r1_uia.py" || stop "SG3_UI_A" "UIA_STATIC_FAIL" "UI-A static contract failed" 56
run_log sg3_npm_ci npm --prefix "$REPO_ROOT/workspace/ui" ci || stop "SG3_UI_A" "NPM_CI_FAIL" "npm ci failed" 57
run_log sg3_vue_typecheck npm --prefix "$REPO_ROOT/workspace/ui" run type-check || stop "SG3_UI_A" "VUE_TYPECHECK_FAIL" "Vue typecheck failed" 58
write_gate "SG3_UI_A" "PASS" true "UI-A generated bindings, owner journeys, accessibility/responsive static contract and Vue typecheck pass."

# SG4 — integrated UI-B contract and reproducible Vite build
PHASE="SG4_UI_B"
run_log sg4_uib_generator python3 "$REPO_ROOT/scripts/generate_sbc7b2_uib_actions.py" --check || stop "SG4_UI_B" "UIB_GENERATOR_FAIL" "UI-B generated metadata drift" 59
run_log sg4_uib_static python3 "$REPO_ROOT/scripts/check_sbc7b2_r2_uib.py" || stop "SG4_UI_B" "UIB_STATIC_FAIL" "UI-B static contract failed" 60
run_log sg4_vite_build npm --prefix "$REPO_ROOT/workspace/ui" run build || stop "SG4_UI_B" "VITE_BUILD_FAIL" "Vite production build failed" 61
run_log sg4_uia_postbuild python3 "$REPO_ROOT/scripts/check_sbc7b2_r1_uia.py" || stop "SG4_UI_B" "UIA_POSTBUILD_FAIL" "UI-A post-build check failed" 62
run_log sg4_uib_postbuild python3 "$REPO_ROOT/scripts/check_sbc7b2_r2_uib.py" || stop "SG4_UI_B" "UIB_POSTBUILD_FAIL" "UI-B post-build check failed" 63
write_gate "SG4_UI_B" "PASS" true "UI-B receipts/documents/contacts/reports/settings contract and production Vite build pass."

# SG5 — actual finite command-text parity
PHASE="SG5_COMMAND_TEXT"
run_log sg5_r3_static python3 "$REPO_ROOT/scripts/check_sbc7b2_r3_ft4.py" --repo "$REPO_ROOT" || stop "SG5_COMMAND_TEXT" "R3_STATIC_FAIL" "FT4 finite command static check failed" 64
run_log sg5_r4_static python3 "$REPO_ROOT/scripts/check_sbc7b2_r4_precandidate.py" --repo "$REPO_ROOT" --skip-git || stop "SG5_COMMAND_TEXT" "R4_STATIC_FAIL" "R4 reconciliation check failed" 65
run_log sg5_foundation_command_text cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --locked command_text_tests -- --test-threads=1 || stop "SG5_COMMAND_TEXT" "FOUNDATION_COMMAND_TEXT_FAIL" "Foundation finite command tests failed" 66
run_log sg5_native_command_text cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --locked 'owner_command_text::tests' -- --test-threads=1 || stop "SG5_COMMAND_TEXT" "NATIVE_COMMAND_TEXT_FAIL" "Native command-text tests failed" 67
write_gate "SG5_COMMAND_TEXT" "PASS" true "Finite text matching, 206/175/700 parity, ambiguity/fail-closed behaviour and reconciled five-read execution tests pass; no speech/model runtime is used."

# SG6 — inherited regressions
PHASE="SG6_REGRESSIONS"
run_log sg6_product_core cargo test --manifest-path "$REPO_ROOT/product/shark-books-core/Cargo.toml" --locked --jobs 1 || stop "SG6_REGRESSIONS" "PRODUCT_CORE_FAIL" "Product core regressions failed" 70
run_log sg6_foundation cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --locked --jobs 1 -- --test-threads=1 || stop "SG6_REGRESSIONS" "FOUNDATION_FAIL" "Foundation regressions failed" 71
ICON="$WORKSPACE/shark-tauri-spike/icons/icon.png"
ICON_BACKUP="$RESULT_DIR/icon.original.png"
cp "$ICON" "$ICON_BACKUP" || stop "SG6_REGRESSIONS" "ICON_BACKUP_FAIL" "Could not back up source icon" 72
run_log sg6_rgba_icon python3 "$REPO_ROOT/ci/prepare_rgba_png.py" "$ICON" || stop "SG6_REGRESSIONS" "ICON_RGBA_FAIL" "Temporary Apple RGBA conversion failed" 73
run_log sg6_tauri cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --locked --jobs 1 -- --test-threads=1 || stop "SG6_REGRESSIONS" "TAURI_REGRESSION_FAIL" "Inherited Tauri owner-bridge regressions failed" 74
write_gate "SG6_REGRESSIONS" "PASS" true "Product core, Foundation and inherited Tauri regressions pass under the established proof key/books environment."

# SG7 — Apple physical + Simulator compile
PHASE="SG7_PLATFORM_COMPILE"
run_log sg7_ios cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios --locked --jobs 1 || stop "SG7_PLATFORM_COMPILE" "APPLE_PHYSICAL_COMPILE_FAIL" "Physical iOS target compile failed" 80
run_log sg7_ios_sim cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios-sim --locked --jobs 1 || stop "SG7_PLATFORM_COMPILE" "APPLE_SIM_COMPILE_FAIL" "iOS Simulator target compile failed" 81
cp "$ICON_BACKUP" "$ICON" || stop "SG7_PLATFORM_COMPILE" "ICON_RESTORE_FAIL" "Source icon restore failed" 82
ICON_BACKUP=""
write_gate "SG7_PLATFORM_COMPILE" "PASS" true "Exact candidate compiles for physical iOS and iOS Simulator targets."

# SG8 — final integrity
PHASE="SG8_FINAL_INTEGRITY"
HEAD_END="$(git -C "$REPO_ROOT" rev-parse HEAD)"
[ "$HEAD_END" = "$HEAD" ] || stop "SG8_FINAL_INTEGRITY" "HEAD_MOVED" "Candidate SHA moved during Apple Super-Gate" 90
if [ -n "${CM_COMMIT:-}" ] && [ "$HEAD_END" != "$CM_COMMIT" ]; then
  stop "SG8_FINAL_INTEGRITY" "CM_COMMIT_DRIFT" "Final HEAD differs from CM_COMMIT" 91
fi
LOCK_SHA_END="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
[ "$LOCK_SHA_END" = "$LOCK_SHA_START" ] || stop "SG8_FINAL_INTEGRITY" "LOCK_DRIFT" "Cargo.lock changed during Apple proof" 92
LOCK_BLOB_END="$(git -C "$REPO_ROOT" hash-object "$WORKSPACE/Cargo.lock")"
[ "$LOCK_BLOB_END" = "$LOCK_BLOB_EXPECTED" ] || stop "SG8_FINAL_INTEGRITY" "LOCK_BLOB_DRIFT" "Cargo.lock no longer equals candidate blob" 93
[ -z "$(git -C "$REPO_ROOT" status --porcelain)" ] || stop "SG8_FINAL_INTEGRITY" "REPOSITORY_DIRTY" "Repository dirty after Apple proof" 94
write_gate "SG8_FINAL_INTEGRITY" "PASS" true "Candidate SHA, Cargo.lock and repository cleanliness preserved through Apple proof."

PHASE="COMPLETE"
OVERALL="PASS"
CLASSIFICATION="PASS_SBC7B2_FT3_FT4_MAC_APPLE_SUPERGATE"
echo "[PASS] SBC-7B2 FT3/FT4 Mac/Apple Super-Gate complete at $HEAD"
