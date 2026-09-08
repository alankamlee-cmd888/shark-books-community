#!/bin/bash
set -u
set -o pipefail

BASE_HEAD="4e3a47b70933cfafe27be262ac2da1ab2a63d1d9"
BEANKEEPER_PIN="d573db5e61089b0922f95c991732394d08e3cf92"
LOCK_SHA_EXPECTED="3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
FACADE_SHA_EXPECTED="6274cffc89ccb2fcea4d489e76375c9c5be1e3c8cc2fbd4c4e56b37bdef0ef46"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.98.1}"
REPO_ROOT="${CM_BUILD_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
WORKSPACE="$REPO_ROOT/workspace"
ARTIFACTS="$REPO_ROOT/artifacts"
TS="$(date +%Y%m%d_%H%M%S)"
RESULT_DIR="$ARTIFACTS/sbc1g_mac_$TS"
LOG_DIR="$RESULT_DIR/logs"
RESULT_ZIP="$ARTIFACTS/SBC1G_MAC_CODEMAGIC_RESULT_$TS.zip"
SUMMARY_TXT="$ARTIFACTS/SBC1G_MAC_SUMMARY.txt"
SUMMARY_JSON="$ARTIFACTS/SBC1G_MAC_SUMMARY.json"
mkdir -p "$LOG_DIR"

ASSERT_N=0
FAIL_N=0
PHASE="PREP"
OVERALL="FAIL"
CLASSIFICATION="UNRESOLVED"
ICON_BACKUP=""

pass() {
  ASSERT_N=$((ASSERT_N+1))
  printf 'PASS\t%s\n' "$1" >> "$RESULT_DIR/assertions.tsv"
  printf '[PASS] %s\n' "$1"
}

fail() {
  ASSERT_N=$((ASSERT_N+1))
  FAIL_N=$((FAIL_N+1))
  printf 'FAIL\t%s\n' "$1" >> "$RESULT_DIR/assertions.tsv"
  printf '[FAIL] %s\n' "$1"
}

stop() {
  fail "$1"
  CLASSIFICATION="$2"
  exit "${3:-1}"
}

run_log() {
  label="$1"
  shift
  "$@" >"$LOG_DIR/${label}.stdout.txt" 2>"$LOG_DIR/${label}.stderr.txt"
}

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.read())[1:-1])'
}

finish() {
  rc=$?
  set +e
  if [ -n "$ICON_BACKUP" ] && [ -f "$ICON_BACKUP" ]; then
    cp "$ICON_BACKUP" "$WORKSPACE/shark-tauri-spike/icons/icon.png"
  fi
  LOCK_SHA="$(shasum -a 256 "$WORKSPACE/Cargo.lock" 2>/dev/null | awk '{print $1}')"
  HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || true)"
  git -C "$REPO_ROOT" status --short > "$RESULT_DIR/git_status_at_finish.txt" 2>/dev/null || true
  printf '%s\n' "$HEAD" > "$RESULT_DIR/repository_commit.txt"
  printf '%s\n' "$LOCK_SHA" > "$RESULT_DIR/cargo_lock_sha256.txt"
  cat > "$SUMMARY_TXT" <<EOF
SBC-1G Codemagic Mac Foundation Guard
Overall: $OVERALL
Classification: $CLASSIFICATION
Phase: $PHASE
Assertions: $ASSERT_N
Failures: $FAIL_N
Codemagic build id: ${CM_BUILD_ID:-unknown}
Repository commit: ${CM_COMMIT:-$HEAD}
Base SBC-1F head: $BASE_HEAD
Beankeeper pin: $BEANKEEPER_PIN
Approved facade SHA-256: $FACADE_SHA_EXPECTED
Reviewed Cargo.lock SHA-256: $LOCK_SHA_EXPECTED
Observed Cargo.lock SHA-256: $LOCK_SHA
Rust toolchain: $RUST_TOOLCHAIN
EOF
  cat > "$SUMMARY_JSON" <<EOF
{"overall":"$OVERALL","classification":"$CLASSIFICATION","phase":"$PHASE","assertions":$ASSERT_N,"failures":$FAIL_N,"codemagic_build_id":"$(printf '%s' "${CM_BUILD_ID:-unknown}" | json_escape)","repository_commit":"$(printf '%s' "${CM_COMMIT:-$HEAD}" | json_escape)","base_head":"$BASE_HEAD","beankeeper_pin":"$BEANKEEPER_PIN","facade_sha256":"$FACADE_SHA_EXPECTED","reviewed_cargo_lock_sha256":"$LOCK_SHA_EXPECTED","observed_cargo_lock_sha256":"$LOCK_SHA","rust_toolchain":"$RUST_TOOLCHAIN"}
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
  exit "$rc"
}
trap finish EXIT
: > "$RESULT_DIR/assertions.tsv"

PHASE="G0_PREFLIGHT"
[ "$(uname -s)" = "Darwin" ] || stop "Host is macOS" "HOST_NOT_MACOS" 20
pass "Host is macOS"
[ "$(uname -m)" = "arm64" ] || stop "Host is Apple Silicon arm64" "HOST_NOT_ARM64" 21
pass "Host is Apple Silicon arm64"
for tool in git python3 shasum zip rustup cargo; do
  command -v "$tool" >/dev/null 2>&1 || stop "$tool available" "HOST_TOOL_MISSING_$tool" 22
  pass "$tool available"
done

if [ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]; then
  git -C "$REPO_ROOT" status --short > "$RESULT_DIR/dirty_at_entry.txt"
  stop "Repository clean at entry" "REPOSITORY_DIRTY_AT_ENTRY" 23
fi
pass "Repository clean at entry"

HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD)"
if [ -n "${CM_COMMIT:-}" ] && [ "$HEAD" != "$CM_COMMIT" ]; then
  stop "Checked-out HEAD equals Codemagic commit" "CODEMAGIC_COMMIT_MISMATCH" 24
fi
pass "Checked-out HEAD equals Codemagic commit"
git -C "$REPO_ROOT" merge-base --is-ancestor "$BASE_HEAD" "$HEAD" || stop "SBC-1F PASS head is ancestor" "BASE_HEAD_NOT_ANCESTOR" 25
pass "SBC-1F PASS head is ancestor"

EXPECTED_CHANGED="$(cat <<'EOF'
.github/workflows/sbc-foundation-guard.yml
ci/prepare_rgba_png.py
ci/run_sbc1g_codemagic.sh
ci/run_sbc1g_windows.ps1
codemagic.yaml
docs/SBC1GH_BOUNDED_BATCH_CONTRACT_2026-09-08.json
docs/SBC1G_CI_REGRESSION_AND_CHANGE_CONTROL_POLICY_2026-09-08.md
scripts/check_sbc1g_change_control.py
workspace/shark-foundation/tests/fixtures/gbp_statement.ofx
workspace/shark-foundation/tests/fixtures/sample_receipt.txt
workspace/shark-foundation/tests/foundation_regression.rs
EOF
)"
OBSERVED_CHANGED="$(git -C "$REPO_ROOT" diff --name-only "$BASE_HEAD..$HEAD" | LC_ALL=C sort)"
printf '%s\n' "$OBSERVED_CHANGED" > "$RESULT_DIR/observed_changed_files.txt"
[ "$OBSERVED_CHANGED" = "$EXPECTED_CHANGED" ] || stop "Candidate diff is exactly the authorised SBC-1G/H closure set" "UNAUTHORISED_SOURCE_CHANGE" 26
pass "Candidate diff is exactly the authorised SBC-1G/H closure set"

PHASE="G1_STATIC_GUARDS"
run_log frozen_baseline python3 "$REPO_ROOT/scripts/check_frozen_baseline.py" || stop "Frozen baseline guard passes" "FROZEN_BASELINE_GUARD_FAIL" 30
pass "Frozen baseline guard passes"
run_log change_control python3 "$REPO_ROOT/scripts/check_sbc1g_change_control.py" --base-ref "$BASE_HEAD" || stop "Permanent change-control guard passes" "CHANGE_CONTROL_GUARD_FAIL" 31
pass "Permanent change-control guard passes"

LOCK_SHA="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
[ "$LOCK_SHA" = "$LOCK_SHA_EXPECTED" ] || stop "Native Cargo.lock remains reviewed hash" "LOCK_DRIFT" 32
pass "Native Cargo.lock remains reviewed hash"

PHASE="G2_RUST_AND_UPSTREAM"
run_log rust_toolchain rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal || stop "Rust toolchain installs" "RUST_TOOLCHAIN_INSTALL_FAIL" 40
rustup default "$RUST_TOOLCHAIN" >"$LOG_DIR/rust_default.stdout.txt" 2>"$LOG_DIR/rust_default.stderr.txt" || stop "Rust toolchain selected" "RUST_TOOLCHAIN_SELECT_FAIL" 41
pass "Rust $RUST_TOOLCHAIN selected"
run_log apple_targets rustup target add aarch64-apple-ios aarch64-apple-ios-sim || stop "Apple Rust targets installed" "APPLE_TARGET_INSTALL_FAIL" 42
pass "Apple Rust targets installed"
run_log bootstrap python3 "$REPO_ROOT/scripts/bootstrap_beankeeper.py" || stop "Exact Beankeeper source materialises" "BEANKEEPER_BOOTSTRAP_FAIL" 43
pass "Exact Beankeeper source materialises"
UPSTREAM_HEAD="$(git -C "$REPO_ROOT/upstream/beankeeper" rev-parse HEAD)"
[ "$UPSTREAM_HEAD" = "$BEANKEEPER_PIN" ] || stop "Beankeeper checkout equals frozen pin" "BEANKEEPER_PIN_DRIFT" 44
pass "Beankeeper checkout equals frozen pin"

PHASE="G3_REGRESSION"
run_log metadata cargo metadata --manifest-path "$WORKSPACE/Cargo.toml" --locked --no-deps || stop "Native workspace metadata resolves under frozen lock" "LOCKED_METADATA_FAIL" 50
pass "Native workspace metadata resolves under frozen lock"
run_log foundation_tests cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --locked --jobs 1 || stop "Permanent Shark foundation regressions pass" "FOUNDATION_REGRESSION_FAIL" 51
pass "Permanent Shark foundation regressions pass"
run_log browser_tests cargo test --manifest-path "$REPO_ROOT/browser/shark-browser-adapter-smoke/Cargo.toml" --locked --jobs 1 || stop "Browser adapter contract smoke remains passing" "BROWSER_REGRESSION_FAIL" 52
pass "Browser adapter contract smoke remains passing"

PHASE="G4_APPLE_POLICY_PROOF"
ICON="$WORKSPACE/shark-tauri-spike/icons/icon.png"
ICON_BACKUP="$RESULT_DIR/icon.original.png"
cp "$ICON" "$ICON_BACKUP" || stop "Source icon backed up for temporary Apple proof" "ICON_BACKUP_FAIL" 60
pass "Source icon backed up for temporary Apple proof"
run_log rgba_icon python3 "$REPO_ROOT/ci/prepare_rgba_png.py" "$ICON" || stop "Temporary Apple RGBA conversion succeeds" "ICON_RGBA_FAIL" 61
pass "Temporary Apple RGBA conversion succeeds"
run_log ios_foundation cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --target aarch64-apple-ios --locked --jobs 1 || stop "Foundation compiles for physical Apple target" "APPLE_FOUNDATION_COMPILE_FAIL" 62
pass "Foundation compiles for physical Apple target"
run_log ios_tauri cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios --locked --jobs 1 || stop "Tauri shell compiles for physical Apple target" "APPLE_TAURI_COMPILE_FAIL" 63
pass "Tauri shell compiles for physical Apple target"
run_log ios_sim_foundation cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --target aarch64-apple-ios-sim --locked --jobs 1 || stop "Foundation compiles for Apple Simulator target" "APPLE_SIM_FOUNDATION_COMPILE_FAIL" 64
pass "Foundation compiles for Apple Simulator target"
run_log ios_sim_tauri cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios-sim --locked --jobs 1 || stop "Tauri shell compiles for Apple Simulator target" "APPLE_SIM_TAURI_COMPILE_FAIL" 65
pass "Tauri shell compiles for Apple Simulator target"
cp "$ICON_BACKUP" "$ICON" || stop "Source icon restored after Apple proof" "ICON_RESTORE_FAIL" 66
ICON_BACKUP=""
pass "Source icon restored after Apple proof"

LOCK_SHA_FINAL="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
[ "$LOCK_SHA_FINAL" = "$LOCK_SHA_EXPECTED" ] || stop "Native Cargo.lock unchanged through all regressions" "LOCK_DRIFT_AFTER_PROOF" 70
pass "Native Cargo.lock unchanged through all regressions"

PHASE="COMPLETE"
OVERALL="PASS"
CLASSIFICATION="PASS_SBC1G_MAC_FOUNDATION_GUARD"
pass "SBC-1G Mac/Apple bounded gate complete"
