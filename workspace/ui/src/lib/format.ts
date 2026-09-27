export const INCOME_CATEGORIES = [
  ["salesTrading", "Sales / trading"],
  ["otherBusinessIncome", "Other business income"],
] as const;

export const EXPENSE_CATEGORIES = [
  ["goodsStockMaterials", "Goods, stock & materials"],
  ["officePhoneSoftware", "Office, phone & software"],
  ["travel", "Travel"],
  ["vehicleCosts", "Vehicle costs"],
  ["premisesUtilities", "Premises & utilities"],
  ["advertisingMarketing", "Advertising & marketing"],
  ["bankFinanceInsurance", "Bank, finance & insurance"],
  ["professionalFees", "Professional fees"],
  ["repairsMaintenance", "Repairs & maintenance"],
  ["training", "Training"],
  ["staffSubcontractorCosts", "Staff & subcontractor costs"],
  ["otherBusinessExpense", "Other business expense"],
] as const;

export const SETTLEMENTS = [
  ["businessBank", "Business bank"],
  ["cash", "Cash"],
] as const;

export function parsePoundsToPence(value: string): number {
  const normalized = value.trim().replace(/,/g, "");
  if (!/^\d+(?:\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Enter a positive amount with no more than two decimal places.");
  }
  const [whole, fraction = ""] = normalized.split(".");
  const pence = Number(whole) * 100 + Number((fraction + "00").slice(0, 2));
  if (!Number.isSafeInteger(pence) || pence <= 0) {
    throw new Error("Amount must be a positive GBP value.");
  }
  return pence;
}

export function parseSignedPoundsToPence(value: string): number {
  const normalized = value.trim().replace(/,/g, "");
  if (!/^-?\d+(?:\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Enter a GBP balance with no more than two decimal places.");
  }
  const negative = normalized.startsWith("-");
  const unsigned = negative ? normalized.slice(1) : normalized;
  const [whole, fraction = ""] = unsigned.split(".");
  const absolutePence = Number(whole) * 100 + Number((fraction + "00").slice(0, 2));
  const pence = negative ? -absolutePence : absolutePence;
  if (!Number.isSafeInteger(pence)) {
    throw new Error("Balance is outside the supported whole-pence range.");
  }
  return pence;
}

export function formatMoney(amountPence: number, currency = "GBP"): string {
  try {
    return new Intl.NumberFormat("en-GB", {
      style: "currency",
      currency,
    }).format(amountPence / 100);
  } catch {
    return `${currency} ${(amountPence / 100).toFixed(2)}`;
  }
}

export function penceToInput(amountPence: number): string {
  return (amountPence / 100).toFixed(2);
}

export function businessPercentToBasisPoints(value: string): number {
  const normalized = value.trim();
  if (!/^\d+(?:\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Business use must be a percentage between 0.01 and 99.99.");
  }
  const percent = Number(normalized);
  if (!(percent > 0 && percent < 100)) {
    throw new Error("Mixed business use must be greater than 0% and less than 100%.");
  }
  const points = Math.round(percent * 100);
  if (points <= 0 || points >= 10_000) {
    throw new Error("Mixed business use is outside the supported range.");
  }
  return points;
}

export function slugBooksName(value: string): string {
  const ascii = value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
  const slug = ascii
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .replace(/-{2,}/g, "-")
    .slice(0, 80)
    .replace(/-+$/g, "");
  if (!slug) {
    throw new Error("Books name must contain at least one letter or number.");
  }
  return slug;
}

export function derivedRecordId(prefix: string, original: string): string {
  const safeOriginal = original
    .replace(/[^A-Za-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "record";
  const nonce = Date.now().toString(36);
  return `${prefix}-${safeOriginal}-${nonce}`.slice(0, 120);
}

export function readableToken(value: string): string {
  return value
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "The operation could not be completed.";
}
