#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def require(ok: bool, message: str) -> None:
    if not ok:
        raise SystemExit("FAIL: " + message)
    print("PASS: " + message)

def squash(text: str) -> str:
    return "".join(text.split())

def region(text: str, start: str, end: str | None = None) -> str:
    if start not in text:
        raise SystemExit("FAIL: region start missing: " + start)
    tail = text.split(start, 1)[1]
    if end is None:
        return tail
    if end not in tail:
        raise SystemExit("FAIL: region end missing after " + start + ": " + end)
    return tail.split(end, 1)[0]

mod = (ROOT / "workspace/shark-foundation/src/bank_application/mod.rs").read_text(encoding="utf-8")
activity = (ROOT / "workspace/shark-foundation/src/bank_application/activity.rs").read_text(encoding="utf-8")
foundation_tests = (ROOT / "workspace/shark-foundation/src/bank_application/tests.rs").read_text(encoding="utf-8")
foundation_lib = (ROOT / "workspace/shark-foundation/src/lib.rs").read_text(encoding="utf-8")
review = (ROOT / "workspace/shark-tauri-spike/src/owner_bank_review.rs").read_text(encoding="utf-8")
mutation = (ROOT / "workspace/shark-tauri-spike/src/owner_bank_mutation.rs").read_text(encoding="utf-8")
tauri_lib = (ROOT / "workspace/shark-tauri-spike/src/lib.rs").read_text(encoding="utf-8")
build = (ROOT / "workspace/shark-tauri-spike/build.rs").read_text(encoding="utf-8")
permission = (ROOT / "workspace/shark-tauri-spike/permissions/shark-shell.toml").read_text(encoding="utf-8")

review_c = squash(review)
mutation_c = squash(mutation)

for marker in ["BankActivityReviewKind", "BankActivityReviewOutcome"]:
    require(marker in mod, f"Foundation defines {marker}")
    require(marker in foundation_lib, f"Foundation re-exports {marker}")

review_fn = region(
    activity,
    "pub fn review_bank_activity_batch",
    "pub fn persist_bank_activity_batch",
)
for marker in ["StrongDuplicate", "FileExactDuplicate", "batch_strong", "batch_file_exact"]:
    require(marker in review_fn, f"read-only duplicate classifier contains {marker}")
require("insert_bank_activity" not in review_fn, "duplicate classifier performs no Bank insert")
require("confirm_bank_match" not in review_fn, "duplicate classifier performs no match mutation")
require(
    "bank_activity_review_is_read_only_and_matches_duplicate_semantics" in foundation_tests,
    "Foundation real-database duplicate review regression exists",
)

for command in [
    "owner_bank_import_review_csv",
    "owner_bank_import_review_ofx_qfx",
    "owner_bank_activity_match_review",
]:
    require(command in review, f"review source defines {command}")
    require(command in tauri_lib, f"runtime registers {command}")
    require(command in build, f"build manifest registers {command}")
    require(command in permission, f"permission manifest allows {command}")

for existing in [
    "owner_bank_import_preview_csv",
    "owner_bank_import_preview_ofx_qfx",
    "owner_bank_match_review",
    "owner_bank_reconcile_preview",
]:
    require(existing in review, f"existing Bank review command retained: {existing}")

for marker in [
    "statement_sha256",
    "duplicate_reviews",
    "duplicate_count",
    "review_bank_activity_batch",
]:
    require(marker in review, f"Books-aware import review includes {marker}")

review_struct = region(
    review,
    "pub(crate) struct OwnerBankImportReview",
    "fn source_kind_name",
)
for forbidden in [
    "strong_identity_key",
    "source_file_sha256",
    "raw_record_sha256",
    "provenance_fingerprint",
    "database_path",
    "storage_root_path",
    "relative_path",
]:
    require(forbidden not in review_struct, f"owner import review omits {forbidden}")

persisted_match_region = region(
    review,
    "pub(crate) fn owner_bank_activity_match_review",
    "#[derive(Debug, Clone, Deserialize)]",
)
persisted_match_c = squash(persisted_match_region)
require(
    "books.bank_activity(request.bank_activity_id)" in persisted_match_c,
    "persisted activity match review reloads authoritative Bank activity",
)
require(
    "match_review_from_persisted_activity(&persisted,&transactions)" in persisted_match_c,
    "persisted activity match review routes through bounded persisted-review helper",
)

persisted_helper = region(
    review,
    "fn match_review_from_persisted_activity",
    "#[tauri::command]",
)
persisted_helper_c = squash(persisted_helper)
require(
    "matched_transaction_id.is_some()" in persisted_helper_c,
    "persisted activity match review rejects already-confirmed rows",
)
require(
    "match_review_from_transactions(&bank_line,transactions)" in persisted_helper_c,
    "persisted activity match review reuses frozen deterministic matcher",
)
require(
    "persisted_activity_match_review_reuses_frozen_matcher_and_rejects_matched_rows"
    in review,
    "persisted activity match-review regression exists",
)

require(
    "confirmed_duplicate_reviews" in mutation,
    "import confirmation carries reviewed duplicate snapshot",
)

dup_guard = region(
    mutation,
    "fn confirm_current_duplicate_review",
    "fn receipt_from_outcomes",
)
dup_guard_c = squash(dup_guard)
require(
    "review_bank_activity_batch(writes)" in dup_guard_c,
    "import confirmation re-runs current duplicate classification",
)
require(
    "require_duplicate_review_match(current,confirmed)" in dup_guard_c,
    "import confirmation compares current duplicate state with reviewed snapshot",
)
require(
    "bank duplicate review changed after review" in mutation,
    "changed duplicate outcome fails closed",
)
require(
    mutation_c.count("persist_bank_activity_batch(&writes)") == 2,
    "both existing atomic import persistence routes remain authoritative",
)
require(
    "duplicate_review_mismatch_fails_closed" in mutation,
    "Tauri stale duplicate-snapshot regression exists",
)

confirm_region = region(
    mutation,
    "pub(crate) struct OwnerCsvImportConfirmRequest",
    "pub(crate) struct OwnerImportLineReceipt",
)
for forbidden in [
    "database_path",
    "db_path",
    "passphrase",
    "account_code",
    "debit",
    "credit",
]:
    require(
        forbidden not in confirm_region,
        f"import confirm DTO omits authority field {forbidden}",
    )

for command in [
    "owner_bank_import_review_csv",
    "owner_bank_import_review_ofx_qfx",
    "owner_bank_activity_match_review",
]:
    token = f'"{command}"'
    require(token in build, f"build manifest carries exact command {command}")
    require(token in permission, f"permission manifest carries exact command {command}")

print("PASS: SBC-7B2 Bank Composition Closure static contract")
