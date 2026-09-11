#!/bin/bash
set -u
set -o pipefail

BASE="bbe31ba295b28e1dcfcddeabe30ae49515353e35"
LOCK_EXPECTED="8d44b338d9469d5d9f28c2552af64e655f58e5bb283f6eef9b088f05ce772c61"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.98.1}"
REPO_ROOT="${CM_BUILD_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
WORKSPACE="$REPO_ROOT/workspace"
ARTIFACTS="$REPO_ROOT/artifacts"
TS="$(date +%Y%m%d_%H%M%S)"
RESULT_DIR="$ARTIFACTS/sbc7b1_bank_review_mac_$TS"
LOG_DIR="$RESULT_DIR/logs"
RESULT_ZIP="$ARTIFACTS/SBC7B1_BANK_REVIEW_MAC_CODEMAGIC_RESULT_$TS.zip"
SUMMARY_TXT="$ARTIFACTS/SBC7B1_BANK_REVIEW_MAC_SUMMARY.txt"
SUMMARY_JSON="$ARTIFACTS/SBC7B1_BANK_REVIEW_MAC_SUMMARY.json"
CARGO_TARGET_DIR="${TMPDIR:-/tmp}/SharkBooks-SBC7B1-BankReview-CargoTarget"
export CARGO_TARGET_DIR
rm -rf "$CARGO_TARGET_DIR"
mkdir -p "$CARGO_TARGET_DIR" "$LOG_DIR"

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
SBC-7B1 Bank Review Codemagic Mac/Apple Gate
Overall: $OVERALL
Classification: $CLASSIFICATION
Phase: $PHASE
Assertions: $ASSERT_N
Failures: $FAIL_N
Codemagic build id: ${CM_BUILD_ID:-unknown}
Repository commit: ${CM_COMMIT:-$HEAD}
Entry protected main: $BASE
Reviewed Cargo.lock SHA-256: $LOCK_EXPECTED
Observed Cargo.lock SHA-256: $LOCK_SHA
Rust toolchain: $RUST_TOOLCHAIN
EOF
  cat > "$SUMMARY_JSON" <<EOF
{"overall":"$OVERALL","classification":"$CLASSIFICATION","phase":"$PHASE","assertions":$ASSERT_N,"failures":$FAIL_N,"codemagic_build_id":"$(printf '%s' "${CM_BUILD_ID:-unknown}" | json_escape)","repository_commit":"$(printf '%s' "${CM_COMMIT:-$HEAD}" | json_escape)","entry_main":"$BASE","reviewed_cargo_lock_sha256":"$LOCK_EXPECTED","observed_cargo_lock_sha256":"$LOCK_SHA","rust_toolchain":"$RUST_TOOLCHAIN"}
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
  rm -rf "$CARGO_TARGET_DIR"
  exit "$rc"
}
trap finish EXIT
: > "$RESULT_DIR/assertions.tsv"

PHASE="BANK_0_PREFLIGHT"
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
git -C "$REPO_ROOT" merge-base --is-ancestor "$BASE" "$HEAD" || stop "Slice 1 protected merge is candidate ancestor" "BASE_HEAD_NOT_ANCESTOR" 25
pass "Slice 1 protected merge is candidate ancestor"

PHASE="BANK_1_STATIC"
run_log static_gate python3 "$REPO_ROOT/scripts/check_sbc7b1_bank_review_bridge.py" --static-only || stop "Bank review static governance gate passes" "STATIC_GATE_FAIL" 30
pass "Bank review static governance gate passes"

LOCK_SHA="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
[ "$LOCK_SHA" = "$LOCK_EXPECTED" ] || stop "Native Cargo.lock equals reviewed Slice 1 hash" "LOCK_DRIFT" 31
pass "Native Cargo.lock equals reviewed Slice 1 hash"

PHASE="BANK_2_TOOLCHAIN"
run_log rust_toolchain rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal || stop "Rust toolchain installs" "RUST_TOOLCHAIN_INSTALL_FAIL" 40
rustup default "$RUST_TOOLCHAIN" >"$LOG_DIR/rust_default.stdout.txt" 2>"$LOG_DIR/rust_default.stderr.txt" || stop "Rust toolchain selected" "RUST_TOOLCHAIN_SELECT_FAIL" 41
pass "Rust $RUST_TOOLCHAIN selected"
run_log apple_targets rustup target add aarch64-apple-ios aarch64-apple-ios-sim || stop "Apple Rust targets installed" "APPLE_TARGET_INSTALL_FAIL" 42
pass "Apple Rust targets installed"
run_log bootstrap python3 "$REPO_ROOT/scripts/bootstrap_beankeeper.py" || stop "Exact Beankeeper source materialises" "BEANKEEPER_BOOTSTRAP_FAIL" 43
pass "Exact Beankeeper source materialises"

PHASE="BANK_3_REGRESSION"
run_log product_core_tests cargo test --manifest-path "$REPO_ROOT/product/shark-books-core/Cargo.toml" --locked --jobs 1 || stop "Product-core regressions pass on Mac" "PRODUCT_CORE_REGRESSION_FAIL" 50
pass "Product-core regressions pass on Mac"
run_log foundation_tests cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-foundation --locked --jobs 1 || stop "Foundation regressions pass on Mac" "FOUNDATION_REGRESSION_FAIL" 51
pass "Foundation regressions pass on Mac"

ICON="$WORKSPACE/shark-tauri-spike/icons/icon.png"
ICON_BACKUP="$RESULT_DIR/icon.original.png"
cp "$ICON" "$ICON_BACKUP" || stop "Source icon backed up before Mac Tauri tests" "ICON_BACKUP_FAIL" 52
pass "Source icon backed up before Mac Tauri tests"
run_log rgba_icon python3 "$REPO_ROOT/ci/prepare_rgba_png.py" "$ICON" || stop "Temporary Apple RGBA conversion succeeds before Mac Tauri tests" "ICON_RGBA_FAIL" 53
pass "Temporary Apple RGBA conversion succeeds before Mac Tauri tests"
run_log prior_owner_tests cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --locked --jobs 1 owner_app::tests -- --test-threads=1 || stop "Prior owner bridge tests pass on Mac" "PRIOR_OWNER_REGRESSION_FAIL" 54
pass "Prior owner bridge tests pass on Mac"
run_log bank_review_tests cargo test --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --locked --jobs 1 owner_bank_review::tests -- --test-threads=1 || stop "Bank review bridge tests pass on Mac" "BANK_REVIEW_REGRESSION_FAIL" 55
pass "Bank review bridge tests pass on Mac"

PHASE="BANK_4_APPLE_COMPILE"
run_log ios_tauri cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios --locked --jobs 1 || stop "Bank review Tauri shell compiles for physical Apple target" "APPLE_TAURI_COMPILE_FAIL" 62
pass "Bank review Tauri shell compiles for physical Apple target"
run_log ios_sim_tauri cargo build --manifest-path "$WORKSPACE/Cargo.toml" -p shark-tauri-spike --target aarch64-apple-ios-sim --locked --jobs 1 || stop "Bank review Tauri shell compiles for Apple Simulator target" "APPLE_SIM_TAURI_COMPILE_FAIL" 63
pass "Bank review Tauri shell compiles for Apple Simulator target"
cp "$ICON_BACKUP" "$ICON" || stop "Source icon restored" "ICON_RESTORE_FAIL" 64
ICON_BACKUP=""
pass "Source icon restored"

LOCK_SHA_FINAL="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
[ "$LOCK_SHA_FINAL" = "$LOCK_EXPECTED" ] || stop "Native Cargo.lock unchanged through Apple proof" "LOCK_DRIFT_AFTER_PROOF" 70
pass "Native Cargo.lock unchanged through Apple proof"
[ -z "$(git -C "$REPO_ROOT" status --porcelain)" ] || stop "Repository clean after Apple proof" "REPOSITORY_DIRTY_AFTER_PROOF" 71
pass "Repository clean after Apple proof"

PHASE="COMPLETE"
OVERALL="PASS"
CLASSIFICATION="PASS_SBC7B1_BANK_REVIEW_MAC_APPLE"
pass "SBC-7B1 Bank review Mac/Apple bounded gate complete"
