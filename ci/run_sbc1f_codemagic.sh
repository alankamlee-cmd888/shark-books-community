#!/bin/bash
set -u
set -o pipefail

BASE_HEAD="f7193c530aa3df0abc7c950e09c3a3a8fc21544d"
BEANKEEPER_PIN="d573db5e61089b0922f95c991732394d08e3cf92"
FACADE_SHA="6274cffc89ccb2fcea4d489e76375c9c5be1e3c8cc2fbd4c4e56b37bdef0ef46"
ROOT_LOCK_SHA_EXPECTED="3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.98.1}"

REPO_ROOT="${CM_BUILD_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
WORKSPACE="$REPO_ROOT/workspace"
BROWSER_CRATE="$REPO_ROOT/browser/shark-browser-adapter-smoke"
ARTIFACTS="$REPO_ROOT/artifacts"
TS="$(date +%Y%m%d_%H%M%S)"
RESULT_DIR="$ARTIFACTS/sbc1f_$TS"
LOG_DIR="$RESULT_DIR/logs"
RESULT_ZIP="$ARTIFACTS/SBC1F_CODEMAGIC_RESULT_$TS.zip"
SUMMARY_TXT="$ARTIFACTS/SBC1F_SUMMARY.txt"
SUMMARY_JSON="$ARTIFACTS/SBC1F_SUMMARY.json"
mkdir -p "$LOG_DIR"

ASSERT_N=0
FAIL_N=0
CMD_N=0
PHASE="PREP"
OVERALL="FAIL"
CLASSIFICATION="UNRESOLVED"
ROOT_LOCK_SHA=""

json_escape() { python3 -c 'import json,sys; print(json.dumps(sys.stdin.read())[1:-1])'; }
pass() { ASSERT_N=$((ASSERT_N+1)); printf 'PASS\t%s\n' "$1" >> "$RESULT_DIR/assertions.tsv"; printf '[PASS] %s\n' "$1"; }
fail() { ASSERT_N=$((ASSERT_N+1)); FAIL_N=$((FAIL_N+1)); printf 'FAIL\t%s\n' "$1" >> "$RESULT_DIR/assertions.tsv"; printf '[FAIL] %s\n' "$1"; }
run_cmd() {
  label="$1"; shift; CMD_N=$((CMD_N+1)); num="$(printf '%03d' "$CMD_N")";
  printf '%q ' "$@" > "$LOG_DIR/${num}_${label}.command.txt"; printf '\n' >> "$LOG_DIR/${num}_${label}.command.txt";
  "$@" >"$LOG_DIR/${num}_${label}.stdout.txt" 2>"$LOG_DIR/${num}_${label}.stderr.txt"; rc=$?;
  printf 'exit_code=%s\n' "$rc" > "$LOG_DIR/${num}_${label}.result.txt"; return "$rc";
}
stop() { fail "$1"; CLASSIFICATION="$2"; exit "${3:-1}"; }

finish() {
  rc=$?; set +e
  [ -f "$WORKSPACE/Cargo.lock" ] && shasum -a 256 "$WORKSPACE/Cargo.lock" > "$RESULT_DIR/root_Cargo.lock.sha256"
  [ -f "$BROWSER_CRATE/Cargo.lock" ] && shasum -a 256 "$BROWSER_CRATE/Cargo.lock" > "$RESULT_DIR/browser_Cargo.lock.sha256"
  git -C "$REPO_ROOT" status --short > "$RESULT_DIR/git_status_at_finish.txt" 2>/dev/null || true
  git -C "$REPO_ROOT" rev-parse HEAD > "$RESULT_DIR/repository_commit.txt" 2>/dev/null || true
  git -C "$REPO_ROOT" diff --name-only "$BASE_HEAD..HEAD" > "$RESULT_DIR/changed_files.txt" 2>/dev/null || true
  cat > "$SUMMARY_TXT" <<EOF
SBC-1F Codemagic Browser Adapter Interface Smoke
Overall: $OVERALL
Classification: $CLASSIFICATION
Phase: $PHASE
Assertions: $ASSERT_N
Failures: $FAIL_N
Commands: $CMD_N
Codemagic build id: ${CM_BUILD_ID:-unknown}
Codemagic workflow id: ${CM_WORKFLOW_ID:-unknown}
Repository commit: ${CM_COMMIT:-unknown}
Base SBC-1E head: $BASE_HEAD
Beankeeper pin: $BEANKEEPER_PIN
Approved SBC-1C facade SHA-256: $FACADE_SHA
Reviewed root Cargo.lock SHA-256: $ROOT_LOCK_SHA_EXPECTED
Observed root Cargo.lock SHA-256: $ROOT_LOCK_SHA
Rust toolchain: $RUST_TOOLCHAIN
Browser contract: dependency-free / no parity claim
Browser persistence candidate: SQLite WASM + OPFS (implementation deferred to SBC-17)
EOF
  cat > "$SUMMARY_JSON" <<EOF
{
  "overall": "$(printf '%s' "$OVERALL" | json_escape)",
  "classification": "$(printf '%s' "$CLASSIFICATION" | json_escape)",
  "phase": "$(printf '%s' "$PHASE" | json_escape)",
  "assertions": $ASSERT_N,
  "failures": $FAIL_N,
  "commands": $CMD_N,
  "codemagic_build_id": "$(printf '%s' "${CM_BUILD_ID:-unknown}" | json_escape)",
  "codemagic_workflow_id": "$(printf '%s' "${CM_WORKFLOW_ID:-unknown}" | json_escape)",
  "repository_commit": "$(printf '%s' "${CM_COMMIT:-unknown}" | json_escape)",
  "base_head": "$BASE_HEAD",
  "beankeeper_pin": "$BEANKEEPER_PIN",
  "sbc1c_facade_sha256": "$FACADE_SHA",
  "reviewed_root_cargo_lock_sha256": "$ROOT_LOCK_SHA_EXPECTED",
  "observed_root_cargo_lock_sha256": "$(printf '%s' "$ROOT_LOCK_SHA" | json_escape)",
  "rust_toolchain": "$RUST_TOOLCHAIN",
  "browser_parity_claimed": false,
  "browser_persistence_candidate": "SQLite WASM + OPFS"
}
EOF
  cp "$SUMMARY_TXT" "$RESULT_DIR/SUMMARY.txt" 2>/dev/null || true
  cp "$SUMMARY_JSON" "$RESULT_DIR/SUMMARY.json" 2>/dev/null || true
  rm -f "$RESULT_ZIP"
  if command -v ditto >/dev/null 2>&1; then ditto -c -k --sequesterRsrc --keepParent "$RESULT_DIR" "$RESULT_ZIP" 2>/dev/null || true; fi
  if [ ! -f "$RESULT_ZIP" ]; then (cd "$ARTIFACTS" && zip -qry "$RESULT_ZIP" "$(basename "$RESULT_DIR")") || true; fi
  [ -f "$RESULT_ZIP" ] && shasum -a 256 "$RESULT_ZIP" > "$RESULT_ZIP.sha256" 2>/dev/null || true
  echo "OVERALL: $OVERALL"; echo "Classification: $CLASSIFICATION"; exit "$rc"
}
trap finish EXIT
: > "$RESULT_DIR/assertions.tsv"

PHASE="F0_PREFLIGHT"
for tool in git python3 shasum zip; do command -v "$tool" >/dev/null 2>&1 || stop "$tool available" "HOST_PREREQUISITE_MISSING_$tool" 20; pass "$tool available"; done
[ -z "$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null)" ] || stop "Repository is clean at SBC-1F entry" "REPOSITORY_DIRTY_AT_ENTRY" 21
pass "Repository is clean at SBC-1F entry"
HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || true)"; [ -n "$HEAD" ] || stop "Repository HEAD resolves" "REPOSITORY_HEAD_MISSING" 22; pass "Repository HEAD resolves"
if [ -n "${CM_COMMIT:-}" ] && [ "$HEAD" != "$CM_COMMIT" ]; then stop "Checked-out HEAD matches Codemagic commit" "CODEMAGIC_COMMIT_MISMATCH" 23; fi
pass "Checked-out HEAD matches Codemagic commit"
git -C "$REPO_ROOT" merge-base --is-ancestor "$BASE_HEAD" "$HEAD" >/dev/null 2>&1 || stop "SBC-1E PASS head is ancestor of candidate" "BASE_HEAD_NOT_ANCESTOR" 24
pass "SBC-1E PASS head is ancestor of candidate"
EXPECTED_CHANGED="$(cat <<'EOF'
browser/shark-browser-adapter-smoke/Cargo.lock
browser/shark-browser-adapter-smoke/Cargo.toml
browser/shark-browser-adapter-smoke/src/lib.rs
browser/shark-browser-adapter-smoke/tests/interface.rs
ci/run_sbc1f_codemagic.sh
codemagic.yaml
docs/SBC17_BROWSER_OPFS_DURABILITY_BACKLOG_2026-09-08.md
docs/SBC1F_BROWSER_ADAPTER_CANDIDATE_CONTRACT_2026-09-08.json
EOF
)"
OBSERVED_CHANGED="$(git -C "$REPO_ROOT" diff --name-only "$BASE_HEAD..$HEAD" | LC_ALL=C sort)"
if [ "$OBSERVED_CHANGED" != "$EXPECTED_CHANGED" ]; then printf '%s\n' "$OBSERVED_CHANGED" > "$RESULT_DIR/observed_changed_files.txt"; stop "Candidate changes are exactly the eight authorised SBC-1F files" "UNAUTHORISED_SOURCE_CHANGE" 25; fi
pass "Candidate changes are exactly the eight authorised SBC-1F files"

PHASE="F1_FROZEN_FOUNDATION"
if run_cmd baseline_guard python3 "$REPO_ROOT/scripts/check_frozen_baseline.py"; then pass "Frozen foundation/SBC-1D baseline guard passes unchanged"; else stop "Frozen foundation/SBC-1D baseline guard passes unchanged" "FROZEN_BASELINE_GUARD_FAIL" 30; fi
ROOT_LOCK_SHA="$(shasum -a 256 "$WORKSPACE/Cargo.lock" | awk '{print $1}')"
[ "$ROOT_LOCK_SHA" = "$ROOT_LOCK_SHA_EXPECTED" ] || stop "Root Cargo.lock remains the reviewed frozen graph" "ROOT_LOCK_HASH_MISMATCH" 31
pass "Root Cargo.lock remains the reviewed frozen graph"

PHASE="F2_BROWSER_CONTRACT"
if python3 - "$REPO_ROOT" >"$LOG_DIR/browser_contract_check.stdout.txt" 2>"$LOG_DIR/browser_contract_check.stderr.txt" <<'PY'
import json, pathlib, re, sys, tomllib
root = pathlib.Path(sys.argv[1]); crate = root / "browser/shark-browser-adapter-smoke"
manifest = tomllib.loads((crate / "Cargo.toml").read_text()); deps = manifest.get("dependencies", {})
assert deps == {}, f"browser smoke crate must remain dependency-free, found {deps}"
source = (crate / "src/lib.rs").read_text(); foundation = (root / "workspace/shark-foundation/src/lib.rs").read_text()
contract = json.loads((root / "docs/SBC1F_BROWSER_ADAPTER_CANDIDATE_CONTRACT_2026-09-08.json").read_text())
backlog = (root / "docs/SBC17_BROWSER_OPFS_DURABILITY_BACKLOG_2026-09-08.md").read_text()
for forbidden in ("use beankeeper", "use beankeeper_cli", "use rusqlite", "use tauri", "use std::fs", "use std::path"):
    assert forbidden not in source, forbidden
for forbidden in ("JournalEntry", "Money::", "Currency::", "Db::", "journal.post("):
    assert forbidden not in source, forbidden
assert "pub const BROWSER_PARITY_CLAIMED: bool = false;" in source
assert "BrowserPersistenceCandidate::SqliteWasmOpfs" in source
assert "SHARK_FACADE_API_VERSION: u32 = 1" in source and "SHARK_FACADE_API_VERSION: u32 = 1" in foundation
for op in contract["required_operations"]:
    assert re.search(rf"\bfn\s+{re.escape(op)}\s*\(", source), f"missing browser operation {op}"
for marker in ("pub fn create_encrypted(", "pub fn open_encrypted(", "pub fn verify(", "pub fn metadata(", "pub fn migration_metadata(", "pub fn create_account(", "pub fn post_transaction(", "pub fn count_transactions(", "pub fn find_by_reference(", "pub fn transaction(", "pub fn trial_balance(", "pub fn import_ofx_summary(", "pub fn reconcile_entry(", "pub fn audit_status_changes(", "pub fn attach_document(", "pub fn attachments("):
    assert marker in foundation, f"foundation marker missing: {marker}"
assert "NOT A PARITY CLAIM" in backlog and "SBC-17" in backlog
print("PASS: dependency-free browser application seam mirrors current facade operation families; no native persistence or accounting-rule implementation is present.")
PY
then pass "Browser adapter contract is dependency-free, native-storage-free and rule-free"; else stop "Browser adapter contract is dependency-free, native-storage-free and rule-free" "BROWSER_CONTRACT_STATIC_FAIL" 32; fi

PHASE="F3_RUST_WASM_TYPE_SEAM"
if ! command -v rustup >/dev/null 2>&1; then run_cmd rustup_install /bin/bash -lc 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none' || stop "rustup installed" "RUSTUP_INSTALL_FAIL" 40; fi
export PATH="$HOME/.cargo/bin:$PATH"; pass "rustup available"
run_cmd rust_toolchain rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal || stop "Rust $RUST_TOOLCHAIN installed" "RUST_TOOLCHAIN_INSTALL_FAIL" 41; pass "Rust $RUST_TOOLCHAIN installed"
run_cmd rust_default rustup default "$RUST_TOOLCHAIN" || stop "Rust $RUST_TOOLCHAIN selected" "RUST_TOOLCHAIN_SELECT_FAIL" 42; pass "Rust $RUST_TOOLCHAIN selected"
run_cmd wasm_target rustup target add --toolchain "$RUST_TOOLCHAIN" wasm32-unknown-unknown || stop "wasm32-unknown-unknown target installed" "WASM_TARGET_INSTALL_FAIL" 43; pass "wasm32-unknown-unknown target installed"
run_cmd browser_wasm_check cargo check --manifest-path "$BROWSER_CRATE/Cargo.toml" --target wasm32-unknown-unknown --locked || stop "Browser adapter contract compiles for wasm32-unknown-unknown" "BROWSER_WASM_COMPILE_FAIL" 44; pass "Browser adapter contract compiles for wasm32-unknown-unknown"
run_cmd browser_host_tests cargo test --manifest-path "$BROWSER_CRATE/Cargo.toml" --locked || stop "Browser adapter type-smoke tests pass" "BROWSER_TYPE_TEST_FAIL" 45; pass "Browser adapter type-smoke tests pass"

PHASE="F4_NATIVE_REGRESSION"
if run_cmd bootstrap_beankeeper python3 "$REPO_ROOT/scripts/bootstrap_beankeeper.py"; then pass "Exact frozen Beankeeper source materialised"; else stop "Exact frozen Beankeeper source materialised" "BEANKEEPER_BOOTSTRAP_FAIL" 50; fi
UPSTREAM_HEAD="$(git -C "$REPO_ROOT/upstream/beankeeper" rev-parse HEAD 2>/dev/null || true)"; [ "$UPSTREAM_HEAD" = "$BEANKEEPER_PIN" ] || stop "Beankeeper checkout matches frozen commit" "BEANKEEPER_PIN_MISMATCH" 51; pass "Beankeeper checkout matches frozen commit"
cd "$WORKSPACE" || exit 52; export CARGO_BUILD_JOBS=1; export CARGO_INCREMENTAL=0
run_cmd foundation_tests cargo test -p shark-foundation --locked --jobs 1 || stop "Existing Shark foundation regression suite passes unchanged" "FOUNDATION_REGRESSION_FAIL" 52; pass "Existing Shark foundation regression suite passes unchanged"
run_cmd root_metadata cargo metadata --locked --no-deps --format-version 1 || stop "Native workspace graph resolves under frozen lock" "ROOT_METADATA_FAIL" 53; pass "Native workspace graph resolves under frozen lock"
ROOT_LOCK_SHA_FINAL="$(shasum -a 256 Cargo.lock | awk '{print $1}')"; [ "$ROOT_LOCK_SHA_FINAL" = "$ROOT_LOCK_SHA_EXPECTED" ] || stop "Root Cargo.lock unchanged through SBC-1F proof" "ROOT_LOCK_DRIFT" 54; ROOT_LOCK_SHA="$ROOT_LOCK_SHA_FINAL"; pass "Root Cargo.lock unchanged through SBC-1F proof"

PHASE="F5_NONCLAIM_AND_BACKLOG"
grep -q '"parity_claimed": false' "$REPO_ROOT/docs/SBC1F_BROWSER_ADAPTER_CANDIDATE_CONTRACT_2026-09-08.json" || stop "Browser parity remains explicitly unclaimed" "PARITY_NONCLAIM_MISSING" 60; pass "Browser parity remains explicitly unclaimed"
grep -q 'SBC-17' "$REPO_ROOT/docs/SBC17_BROWSER_OPFS_DURABILITY_BACKLOG_2026-09-08.md" || stop "SBC-17 OPFS durability backlog is recorded" "SBC17_BACKLOG_MISSING" 61; pass "SBC-17 OPFS durability backlog is recorded"

PHASE="COMPLETE"
OVERALL="PASS"
CLASSIFICATION="PASS_SBC1F_BROWSER_ADAPTER_INTERFACE_SMOKE"
exit 0
