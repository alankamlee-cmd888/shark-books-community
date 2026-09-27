import type { CommandFact } from "./tauri";

export type CommandSection =
  | "Home"
  | "Money in"
  | "Money out"
  | "Bank"
  | "Receipts"
  | "Contacts"
  | "Reports"
  | "Settings";

const FIXED_ROUTES: Record<string, CommandSection> = {
  "BOOKS.CREATE": "Home",
  "BOOKS.OPEN": "Home",
  "BOOKS.VERIFY": "Home",
  "HOME.STATUS": "Home",
  "MONEY_IN.PREVIEW": "Money in",
  "MONEY_IN.SAVE": "Money in",
  "MONEY_OUT.PREVIEW": "Money out",
  "MONEY_OUT.SAVE": "Money out",
  "BANK.IMPORT_PREVIEW_CSV": "Bank",
  "BANK.IMPORT_PREVIEW_OFX_QFX": "Bank",
  "BANK.IMPORT_CONFIRM_CSV": "Bank",
  "BANK.IMPORT_CONFIRM_OFX_QFX": "Bank",
  "BANK.ACTIVITY_LIST": "Bank",
  "BANK.ACTIVITY_DETAIL": "Bank",
  "BANK.MATCH_REVIEW": "Bank",
  "BANK.MATCH_CONFIRM": "Bank",
  "BANK.RECONCILE_PREVIEW": "Bank",
  "BANK.RECONCILE_FINALISE": "Bank",
  "DOCUMENT.SELECT_REGISTER": "Receipts",
  "DOCUMENT.VERIFY": "Receipts",
  "DOCUMENT.LIST": "Receipts",
  "DOCUMENT.OPEN_VIEW": "Receipts",
  "DOCUMENT.ATTACH": "Receipts",
  "OCR.EXTRACT_RECEIPT": "Receipts",
  "RECEIPT.SUGGEST_BANK": "Receipts",
  "RECEIPT.CONFIRM_BANK": "Receipts",
  "RECEIPT.REJECT_BANK": "Receipts",
  "CONTACTS.LIST": "Contacts",
  "CONTACTS.SAVE": "Contacts",
  "REPORT.SUMMARY": "Reports",
  "SETTINGS.BOOKS_INFO": "Settings",
  "SETTINGS.STORAGE_ROOT.SELECT": "Settings",
};

export function commandSection(
  actionId: string | null,
  facts: CommandFact[],
): CommandSection | null {
  if (!actionId) return null;
  if (actionId === "MONEY.RECORDS.LIST") {
    const direction = facts.find((fact) => fact.slotId === "direction_filter")?.value.toLowerCase();
    if (direction?.includes("out")) return "Money out";
    if (direction?.includes("in")) return "Money in";
    return null;
  }
  return FIXED_ROUTES[actionId] ?? null;
}
