#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def read(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")

def require(ok: bool, message: str) -> None:
    if not ok:
        raise SystemExit("FAIL: " + message)
    print("PASS: " + message)

app = read("workspace/ui/src/App.vue")
styles = read("workspace/ui/src/styles.css")
tauri = read("workspace/ui/src/lib/tauri.ts")
session = read("workspace/ui/src/lib/session.ts")
bindings = read("workspace/ui/src/lib/action-bindings.ts")
generated = read("workspace/ui/src/generated/uib-actions.ts")
receipts = read("workspace/ui/src/screens/ReceiptsScreen.vue")
contacts = read("workspace/ui/src/screens/ContactsScreen.vue")
reports = read("workspace/ui/src/screens/ReportsScreen.vue")
settings = read("workspace/ui/src/screens/SettingsScreen.vue")
doc_table = read("workspace/ui/src/components/DocumentTable.vue")
contact_table = read("workspace/ui/src/components/ContactsTable.vue")
lib_rs = read("workspace/shark-tauri-spike/src/lib.rs")
config = json.loads(read("workspace/shark-tauri-spike/tauri.conf.json"))

for screen in ["ReceiptsScreen", "ContactsScreen", "ReportsScreen", "SettingsScreen"]:
    require(screen in app, f"App.vue delegates to {screen}")
require(len(app.splitlines()) < 190, "App.vue remains a decomposed shell")

for action_id in [
    "DOCUMENT.SELECT_REGISTER", "DOCUMENT.VERIFY", "DOCUMENT.LIST", "DOCUMENT.OPEN_VIEW",
    "DOCUMENT.ATTACH", "OCR.EXTRACT_RECEIPT", "RECEIPT.SUGGEST_BANK", "RECEIPT.CONFIRM_BANK",
    "RECEIPT.REJECT_BANK", "CONTACTS.LIST", "CONTACTS.SAVE", "REPORT.SUMMARY",
    "SETTINGS.BOOKS_INFO", "SETTINGS.STORAGE_ROOT.SELECT",
]:
    require(action_id in generated, f"generated UI-B metadata contains {action_id}")

keys = [
    "documentSelectRegister", "documentVerify", "documentList", "documentOpenView",
    "documentAttach", "ocrExtract", "receiptSuggestBank", "receiptConfirmBank",
    "receiptRejectBank", "contactsList", "contactsSave", "reportSummary",
    "settingsBooksInfo", "settingsStorageRoot",
]
for key in keys:
    require(key in generated, f"generated UI-B metadata contains selector {key}")
    require(key in bindings, f"UI-B binding contains selector {key}")

commands = [
    "owner_document_select_register", "owner_document_verify", "owner_document_list",
    "owner_document_open_view", "owner_document_attach", "owner_ocr_extract_receipt",
    "owner_receipt_suggest_bank", "owner_receipt_confirm_bank", "owner_receipt_reject_bank",
    "owner_contacts_list", "owner_contacts_save", "owner_report_summary",
    "owner_settings_books_info", "owner_settings_storage_root_select",
]
for command in commands:
    require(f'"{command}"' in tauri, f"typed Tauri client admits {command}")

for raw_forbidden in [
    '"books_trial_balance"', '"ocr_extract_receipt"', '"owner_bank_import_preview_csv"',
    '"owner_bank_import_preview_ofx_qfx"', '"owner_bank_match_review"',
]:
    require(raw_forbidden not in tauri, f"typed permanent client excludes raw/stale command {raw_forbidden}")

require("storageRootId: string | null" in session, "session stores only opaque document-root identity")
require("storageRootLabel" in session, "session keeps owner-facing document-root status")
require("localStorage" not in session and "sessionStorage" not in session, "R2 adds no browser persistence")

for source, name in [(doc_table, "Documents"), (contact_table, "Contacts")]:
    require("tableFeatures" in source and "useTable" in source and "createColumnHelper" in source,
            f"{name} table uses TanStack Vue Table v9")
    require("useVueTable" not in source, f"{name} table avoids obsolete TanStack v8 API")

for marker in ["URL.createObjectURL", "URL.revokeObjectURL", "owner-safe", "manual"]:
    require(marker.lower() in receipts.lower(), f"Receipts implementation contains {marker}")
require("ConfirmationDialog" in receipts, "receipt confirm/reject reuse the Reka confirmation primitive")
require("recommendedCandidateId" in receipts and "selectedCandidateId" in receipts,
        "receipt recommendation is displayed separately from explicit owner selection")
require("attachDocument" in receipts and "listMoneyRecords" in receipts,
        "manual document attachment uses owner-safe current record lists")
require("tax" not in reports.lower() and "profit" not in reports.lower(),
        "factual Reports screen contains no tax/profit claim")
require("storageRootId" not in settings.split("<template>", 1)[1],
        "Settings template never presents opaque storage-root ID")

normal_templates = "\n".join(
    source.split("<template>", 1)[1] if "<template>" in source else source
    for source in [receipts, contacts, reports, settings]
)
for forbidden_label in [
    "SHA-256", "database path", "passphrase", "encryption key", "taxable profit",
    "accounting profit", "tax due", "tax reserve", "deductibility", "VAT treatment",
]:
    require(forbidden_label.lower() not in normal_templates.lower(),
            f"normal UI-B presentation omits {forbidden_label}")

for breakpoint in ["@media (max-width: 900px)", "@media (max-width: 600px)"]:
    require(breakpoint in styles, f"responsive stylesheet retains {breakpoint}")
require(":focus-visible" in styles, "visible keyboard focus remains present")
require("overflow-x: auto" in styles, "narrow table containment remains present")
require("aria-live" in receipts and "aria-live" in contacts and "aria-live" in reports and "aria-live" in settings,
        "all UI-B screens expose accessible operation status")

csp = config["app"]["security"]["csp"]
require(csp.get("img-src") == "'self' blob:", "CSP admits only self/blob for verified image preview")
require(csp.get("frame-src") == "'self' blob:", "CSP admits only self/blob for verified frame preview")
for directive in ["img-src", "frame-src"]:
    value = csp[directive]
    for forbidden in ["http:", "https:", "data:", "file:", "*"]:
        require(forbidden not in value, f"{directive} excludes {forbidden}")

security_region = lib_rs.split(
    "fn frontend_contract_is_keyless_and_uses_only_bounded_commands()", 1
)[1].split("fn tauri_acl_and_csp_are_bounded()", 1)[0]
for command in commands:
    require(command in security_region, f"frontend security test positively requires {command}")
for forbidden in ["books_trial_balance", "SHARK_SBC1D_PROOF_KEY", "databasePath", "shellCommand"]:
    require(forbidden in security_region, f"frontend security test retains negative boundary {forbidden}")

print("PASS: SBC-7B2 R2/UI-B static architecture, action, security and presentation contract")
