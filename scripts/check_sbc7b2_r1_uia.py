#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "workspace/ui/src"

def require(ok: bool, message: str) -> None:
    if not ok:
        raise SystemExit("FAIL: " + message)
    print("PASS: " + message)

def read(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")

app = read("workspace/ui/src/App.vue")
styles = read("workspace/ui/src/styles.css")
tauri = read("workspace/ui/src/lib/tauri.ts")
session = read("workspace/ui/src/lib/session.ts")
bindings = read("workspace/ui/src/lib/action-bindings.ts")
generated = read("workspace/ui/src/generated/uia-actions.ts")
dialog = read("workspace/ui/src/components/ConfirmationDialog.vue")
money_table = read("workspace/ui/src/components/MoneyTable.vue")
bank_table = read("workspace/ui/src/components/BankActivityTable.vue")
home = read("workspace/ui/src/screens/HomeScreen.vue")
money = read("workspace/ui/src/screens/MoneyScreen.vue")
bank = read("workspace/ui/src/screens/BankScreen.vue")
bank_rust = read("workspace/shark-tauri-spike/src/owner_bank_review.rs")
tauri_lib = read("workspace/shark-tauri-spike/src/lib.rs")

for item in [
    '"Home"', '"Money in"', '"Money out"', '"Bank"',
    '"Receipts"', '"Contacts"', '"Reports"', '"Settings"',
]:
    require(item in app, f"permanent navigation contains {item}")

require(len(app.splitlines()) < 180, "App.vue remains a shell rather than a monolith")
for screen in ["HomeScreen", "MoneyScreen", "BankScreen"]:
    require(screen in app, f"App.vue delegates to {screen}")

for forbidden in ["Books file", "Books ID", ">Actor<", "databasePath", "dbPath", "passphrase"]:
    require(forbidden not in app + home + money + bank, f"normal UI omits technical authority {forbidden}")

require("slugBooksName" in session, "session derives bounded Books identity")
require('actor: "owner"' in session, "owner actor remains application context")
require(".sqlite" in session, "session derives bounded sqlite filename token")
require("raw path" not in session.lower(), "session does not expose raw path authority")

require("from \"reka-ui\"" in dialog, "confirmation dialog uses Reka UI")
for symbol in ["DialogRoot", "DialogPortal", "DialogOverlay", "DialogContent", "DialogTitle", "DialogDescription"]:
    require(symbol in dialog, f"Reka confirmation uses {symbol}")
require('role="status"' in dialog or 'aria-live' in dialog, "confirmation dialog has assistive status semantics")

for table_source, label in [(money_table, "Money"), (bank_table, "Bank")]:
    require("tableFeatures" in table_source, f"{label} table uses TanStack v9 tableFeatures")
    require("useTable" in table_source, f"{label} table uses TanStack v9 useTable")
    require("createColumnHelper" in table_source, f"{label} table uses TanStack v9 column helper")
    require("useVueTable" not in table_source, f"{label} table avoids obsolete TanStack v8 API")

for marker in [
    "MONEY.RECORDS.LIST", "MONEY.RECORD.DETAIL", "BANK.ACTIVITY_DETAIL",
    "BANK.IMPORT_PREVIEW_CSV", "BANK.IMPORT_PREVIEW_OFX_QFX",
    "BANK.IMPORT_CONFIRM_CSV", "BANK.IMPORT_CONFIRM_OFX_QFX",
]:
    require(marker in generated, f"generated Action metadata contains {marker}")

for semantic_key in [
    "booksCreate", "booksOpen", "booksVerify", "homeStatus",
    "moneyInPreview", "moneyInSave", "moneyOutPreview", "moneyOutSave",
    "moneyRecordsList", "moneyRecordDetail", "correctionPreview", "correctionConfirm",
    "correctionHistory", "bankImportReviewCsv", "bankImportReviewOfxQfx",
    "bankImportConfirmCsv", "bankImportConfirmOfxQfx",
    "bankActivityList", "bankActivityDetail", "bankMatchReview",
    "bankMatchConfirm", "bankReconcilePreview", "bankReconcileFinalise",
]:
    require(semantic_key in generated, f"generated metadata contains {semantic_key}")
    require(semantic_key in bindings, f"mechanical binding contains {semantic_key}")

uia_commands = [
    "foundation_health",
    "books_create", "books_open", "books_verify", "owner_home_status",
    "owner_money_in_preview", "owner_money_in_save",
    "owner_money_out_preview", "owner_money_out_save",
    "owner_money_records_list", "owner_money_record_detail",
    "owner_correction_preview", "owner_correction_confirm", "owner_correction_history",
    "owner_bank_import_review_csv", "owner_bank_import_review_ofx_qfx",
    "owner_bank_import_confirm_csv", "owner_bank_import_confirm_ofx_qfx",
    "owner_bank_activity_list", "owner_bank_activity_detail",
    "owner_bank_activity_match_review", "owner_bank_match_confirm",
    "owner_bank_reconcile_preview", "owner_bank_reconcile_finalise",
]
for command in uia_commands:
    require(f'"{command}"' in tauri, f"typed Tauri client admits {command}")

for forbidden_command in [
    "books_trial_balance",
    "owner_bank_import_preview_csv", "owner_bank_import_preview_ofx_qfx",
    "owner_bank_match_review",
    "owner_document_select_register", "owner_document_verify", "owner_document_open_view",
    "owner_document_attach", "owner_ocr_extract_receipt",
    "owner_receipt_suggest_bank", "owner_receipt_confirm_bank", "owner_receipt_reject_bank",
    "owner_contacts_list", "owner_contacts_save",
    "owner_settings_books_info", "owner_settings_storage_root_select", "owner_report_summary",
]:
    require(f'"{forbidden_command}"' not in tauri, f"UI-A client omits non-UI-A command {forbidden_command}")

review_fn = tauri.split("export function reviewPersistedBankActivityMatch", 1)[1].split(
    "export function confirmBankMatch", 1
)[0]
require("candidateTransactionIds" not in review_fn, "Vue supplies no candidate IDs to persisted match review")

require("pub(crate) struct OwnerBankActivityMatchReviewRequest" in bank_rust, "persisted match review request exists")
request_region = bank_rust.split("pub(crate) struct OwnerBankActivityMatchReviewRequest", 1)[1].split(
    "fn match_review_from_persisted_activity", 1
)[0]
require("candidate_transaction_ids" not in request_region, "Rust persisted match-review request no longer accepts candidate IDs")
for marker in [
    '["moneyIn", "moneyOut"]',
    "owner_money_records(record_kind",
    "MAX_DISCOVERY_RECORDS_PER_KIND",
    "OWNER_DISCOVERY_PAGE",
    "MAX_MATCH_CANDIDATES",
    "bank_match_for_entry",
    "Reconciled",
    "match_review_from_persisted_activity",
]:
    require(marker in bank_rust, f"Rust candidate discovery contains {marker}")
require("owner_bank_match_confirm" not in bank_rust, "read-only Bank review source contains no match mutation")

for screen_source, name in [(home, "Home"), (money, "Money"), (bank, "Bank")]:
    require("aria-live" in screen_source or "role=\"status\"" in screen_source, f"{name} exposes operation status accessibly")

for breakpoint in ["@media (max-width: 900px)", "@media (max-width: 600px)"]:
    require(breakpoint in styles, f"responsive stylesheet includes {breakpoint}")
require(":focus-visible" in styles, "visible focus styling remains present")
require("overflow-x: auto" in styles, "narrow table containment is present")

normal_ui = app + home + money + bank
for forbidden_label in [
    "Beankeeper", "SQLCipher", "SQLite", "journal",
    "Debit column", "Credit column", "debit and credit",
]:
    require(forbidden_label.lower() not in normal_ui.lower(), f"normal UI omits internal label {forbidden_label}")

security_region = tauri_lib.split(
    "fn frontend_contract_is_keyless_and_uses_only_bounded_commands()", 1
)[1].split(
    "fn tauri_acl_and_csp_are_bounded()", 1
)[0]
require("books_trial_balance" in security_region, "frontend security test explicitly forbids stale trial-balance exposure")
require("owner_home_status" in security_region, "frontend security test requires current Home surface")
require("owner_bank_activity_match_review" in security_region, "frontend security test requires persisted match review")
require("owner_document_select_register" in security_region, "frontend security test retains UI-B negative boundary")

print("PASS: SBC-7B2 R1/UI-A static architecture and binding contract")
