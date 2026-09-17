export interface BooksRef {
  fileName: string;
  booksId: string;
  actor: string;
}

export interface CreateBooksRequest extends BooksRef {
  companyName: string;
}

export interface OwnerHomeStatus {
  bridgeVersion: number;
  companyName: string;
  transactionCount: number;
  booksBalanced: boolean;
  productionEncryptionRequired: boolean;
}

export type SettlementAccount = "businessBank" | "cash";
export type IncomeCategory = "salesTrading" | "otherBusinessIncome";
export type ExpenseCategory =
  | "goodsStockMaterials"
  | "officePhoneSoftware"
  | "travel"
  | "vehicleCosts"
  | "premisesUtilities"
  | "advertisingMarketing"
  | "bankFinanceInsurance"
  | "professionalFees"
  | "repairsMaintenance"
  | "training"
  | "staffSubcontractorCosts"
  | "otherBusinessExpense";

export type BusinessUse =
  | { kind: "business" }
  | { kind: "private" }
  | { kind: "mixed"; businessBasisPoints: number };

export interface OwnerMoneyInRequest {
  books: BooksRef;
  recordId: string;
  description: string;
  date: string;
  amountPence: number;
  category: IncomeCategory;
  settlement: SettlementAccount;
}

export interface OwnerMoneyOutRequest {
  books: BooksRef;
  recordId: string;
  description: string;
  date: string;
  amountPence: number;
  category: ExpenseCategory;
  businessUse: BusinessUse;
  settlement: SettlementAccount;
}

export interface OwnerPostingPreview {
  bridgeVersion: number;
  recordId: string;
  recordKind: "moneyIn" | "moneyOut";
  date: string;
  amountPence: number;
  businessAmountPence: number;
  privateAmountPence: number;
  currency: string;
  balanced: boolean;
  requiresConfirmation: boolean;
}

export interface OwnerSaveReceipt {
  bridgeVersion: number;
  recordId: string;
  recordKind: "moneyIn" | "moneyOut";
  transactionId: number;
  created: boolean;
  alreadyRecorded: boolean;
  requiresFurtherAutomaticAction: boolean;
}

export interface OwnerMoneyRecordRead {
  recordId: string;
  recordKind: "moneyIn" | "moneyOut";
  transactionId: number;
  description: string;
  date: string;
  amountPence: number;
  currency: string;
}

export interface OwnerDocumentRead {
  documentId: string;
  originalFilename: string;
  mediaType: string | null;
  byteLen: number;
  registeredAt: string;
}

export interface OwnerCorrectionRead {
  correctionId: string;
  reason: string;
  correctedAt: string;
  replacementRecordId: string | null;
}

export interface OwnerMoneyRecordDetailRead {
  record: OwnerMoneyRecordRead;
  documents: OwnerDocumentRead[];
  corrections: OwnerCorrectionRead[];
}

export interface OwnerMoneyRecordsPage {
  bridgeVersion: number;
  rows: OwnerMoneyRecordRead[];
}

export interface OwnerMoneyRecordDetailResponse {
  bridgeVersion: number;
  detail: OwnerMoneyRecordDetailRead;
}

export type CorrectionReplacement =
  | {
      kind: "moneyIn";
      recordId: string;
      description: string;
      date: string;
      amountPence: number;
      category: IncomeCategory;
      settlement: SettlementAccount;
    }
  | {
      kind: "moneyOut";
      recordId: string;
      description: string;
      date: string;
      amountPence: number;
      category: ExpenseCategory;
      businessUse: BusinessUse;
      settlement: SettlementAccount;
    };

export interface OwnerCorrectionRequest {
  books: BooksRef;
  recordKind: "moneyIn" | "moneyOut";
  originalRecordId: string;
  reversalRecordId: string;
  replacement: CorrectionReplacement | null;
  reason: string;
}

export interface OwnerCorrectionTransactionSummary {
  recordId: string;
  date: string;
  description: string;
  amountPence: number;
}

export interface OwnerCorrectionPreview {
  bridgeVersion: number;
  recordKind: "moneyIn" | "moneyOut";
  original: OwnerCorrectionTransactionSummary;
  reversal: OwnerCorrectionTransactionSummary;
  replacement: OwnerCorrectionTransactionSummary | null;
  previewFingerprint: string;
  correctionId: string;
  requiresConfirmation: boolean;
}

export interface OwnerCorrectionReceipt {
  bridgeVersion: number;
  correctionId: string;
  recordKind: "moneyIn" | "moneyOut";
  originalRecordId: string;
  reversalRecordId: string;
  replacementRecordId: string | null;
  applied: boolean;
  alreadyApplied: boolean;
  requiresFurtherAutomaticAction: boolean;
}

export interface OwnerCorrectionHistoryItem {
  correctionId: string;
  recordKind: string;
  originalRecordId: string;
  reversalRecordId: string;
  replacementRecordId: string | null;
  reason: string;
  correctedBy: string;
  correctedAt: string;
}

export interface OwnerCorrectionHistory {
  bridgeVersion: number;
  items: OwnerCorrectionHistoryItem[];
}

export type CsvDateFormat = "isoYmd" | "dmySlash";
export type CsvAmountMapping =
  | { kind: "signed"; amountHeader: string }
  | { kind: "debitCredit"; debitHeader: string; creditHeader: string };

export interface CsvProfile {
  id: string;
  name: string;
  delimiter: string;
  dateHeader: string;
  valueDateHeader: string | null;
  descriptionHeader: string;
  payeeHeader: string | null;
  referenceHeader: string | null;
  transactionIdHeader: string | null;
  currencyHeader: string | null;
  amountMapping: CsvAmountMapping;
  dateFormat: CsvDateFormat;
}

export type OfxFormat = "ofx" | "qfx";

export interface OwnerBankLineView {
  lineRef: string;
  postedDate: string;
  valueDate: string | null;
  signedAmountPence: number;
  currency: string;
  description: string;
  payee: string | null;
  reference: string | null;
  sourceFormat: string;
  hasStrongSourceIdentity: boolean;
}

export interface OwnerBankDuplicateReview {
  lineRef: string;
  outcome: "new" | "strongDuplicate" | "fileExactDuplicate";
}

export interface OwnerBankPreviewError {
  lineRef: string;
  message: string;
}

export interface OwnerBankImportReview {
  bridgeVersion: number;
  statementSha256: string;
  canConfirm: boolean;
  requiresExplicitConfirmation: boolean;
  lines: OwnerBankLineView[];
  errors: OwnerBankPreviewError[];
  duplicateReviews: OwnerBankDuplicateReview[];
  duplicateCount: number;
}

export interface OwnerBankImportReceipt {
  bridgeVersion: number;
  createdCount: number;
  duplicateCount: number;
  requiresFurtherAutomaticAction: boolean;
  lines: Array<{
    lineRef: string;
    bankActivityId: number;
    outcome: string;
  }>;
}

export interface OwnerBankActivityRow {
  bankActivityId: number;
  postedDate: string;
  signedAmountPence: number;
  currency: string;
  description: string;
  payee: string | null;
  reference: string | null;
  importedAt: string;
  matchedTransactionId: number | null;
  matchLevel: string | null;
  clearanceState: string | null;
}

export interface OwnerBankActivityPage {
  bridgeVersion: number;
  rows: OwnerBankActivityRow[];
}

export interface OwnerBankActivityDetail {
  bridgeVersion: number;
  bankActivityId: number;
  sourceAccountId: string;
  postedDate: string;
  valueDate: string | null;
  signedAmountPence: number;
  currency: string;
  description: string;
  payee: string | null;
  reference: string | null;
  sourceFormat: string;
  importedAt: string;
  importedBy: string;
  matchedTransactionId: number | null;
  matchLevel: string | null;
  matchScore: number | null;
  matchReasons: string[];
  clearanceState: string | null;
  hasStrongSourceIdentity: boolean;
}

export interface OwnerBankMatchCandidate {
  transactionId: number;
  level: "unmatched" | "possible" | "likely" | "exact";
  score: number;
  reasons: string[];
  requiresExplicitConfirmation: boolean;
}

export interface OwnerBankMatchReview {
  bridgeVersion: number;
  bankLine: OwnerBankLineView;
  candidates: OwnerBankMatchCandidate[];
  recommendedTransactionId: number | null;
  ambiguousTop: boolean;
  requiresExplicitConfirmation: boolean;
}

export interface OwnerBankMatchReceipt {
  bridgeVersion: number;
  bankActivityId: number;
  transactionId: number;
  bankEntryId: number;
  matchLevel: string;
  score: number;
  reasons: string[];
  alreadyConfirmed: boolean;
  clearanceState: string;
}

export interface OwnerReconciliationRow {
  transactionId: number;
  bankEntryId: number;
  signedAmountPence: number;
  clearanceState: string;
}

export interface OwnerReconciliationPreview {
  bridgeVersion: number;
  openingBalancePence: number;
  computedEndingBalancePence: number;
  expectedEndingBalancePence: number;
  differencePence: number;
  allEntriesCleared: boolean;
  canFinalise: boolean;
  requiresExplicitConfirmation: boolean;
  rows: OwnerReconciliationRow[];
}

export interface OwnerBankReconciliationReceipt {
  bridgeVersion: number;
  statementId: string;
  statementDate: string;
  openingBalancePence: number;
  endingBalancePence: number;
  entryCount: number;
  alreadyFinalised: boolean;
  clearanceState: string;
}

type UiACommand =
  | "foundation_health"
  | "books_create"
  | "books_open"
  | "books_verify"
  | "owner_home_status"
  | "owner_money_in_preview"
  | "owner_money_in_save"
  | "owner_money_out_preview"
  | "owner_money_out_save"
  | "owner_money_records_list"
  | "owner_money_record_detail"
  | "owner_correction_preview"
  | "owner_correction_confirm"
  | "owner_correction_history"
  | "owner_bank_import_review_csv"
  | "owner_bank_import_review_ofx_qfx"
  | "owner_bank_import_confirm_csv"
  | "owner_bank_import_confirm_ofx_qfx"
  | "owner_bank_activity_list"
  | "owner_bank_activity_detail"
  | "owner_bank_activity_match_review"
  | "owner_bank_match_confirm"
  | "owner_bank_reconcile_preview"
  | "owner_bank_reconcile_finalise";

function nativeInvoke<T>(
  command: UiACommand,
  args?: Record<string, unknown>,
): Promise<T> {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) {
    return Promise.reject(new Error("Shark Books native bridge is unavailable."));
  }
  return invoke<T>(command, args);
}

export function foundationHealth(): Promise<string> {
  return nativeInvoke<string>("foundation_health");
}

export function createBooks(request: CreateBooksRequest): Promise<unknown> {
  return nativeInvoke("books_create", { request });
}

export function openBooks(request: BooksRef): Promise<unknown> {
  return nativeInvoke("books_open", { request });
}

export function verifyBooks(request: BooksRef): Promise<unknown> {
  return nativeInvoke("books_verify", { request });
}

export function homeStatus(books: BooksRef): Promise<OwnerHomeStatus> {
  return nativeInvoke("owner_home_status", { books });
}

export function listMoneyRecords(
  books: BooksRef,
  recordKind: "moneyIn" | "moneyOut",
  limit = 100,
  offset = 0,
): Promise<OwnerMoneyRecordsPage> {
  return nativeInvoke("owner_money_records_list", {
    request: { books, recordKind, limit, offset },
  });
}

export function moneyRecordDetail(
  books: BooksRef,
  recordKind: "moneyIn" | "moneyOut",
  recordId: string,
): Promise<OwnerMoneyRecordDetailResponse> {
  return nativeInvoke("owner_money_record_detail", {
    request: { books, recordKind, recordId },
  });
}

export function previewMoneyIn(
  request: OwnerMoneyInRequest,
): Promise<OwnerPostingPreview> {
  return nativeInvoke("owner_money_in_preview", { request });
}

export function saveMoneyIn(
  request: OwnerMoneyInRequest,
): Promise<OwnerSaveReceipt> {
  return nativeInvoke("owner_money_in_save", { request });
}

export function previewMoneyOut(
  request: OwnerMoneyOutRequest,
): Promise<OwnerPostingPreview> {
  return nativeInvoke("owner_money_out_preview", { request });
}

export function saveMoneyOut(
  request: OwnerMoneyOutRequest,
): Promise<OwnerSaveReceipt> {
  return nativeInvoke("owner_money_out_save", { request });
}

export function previewCorrection(
  request: OwnerCorrectionRequest,
): Promise<OwnerCorrectionPreview> {
  return nativeInvoke("owner_correction_preview", { request });
}

export function confirmCorrection(
  correction: OwnerCorrectionRequest,
  previewFingerprint: string,
): Promise<OwnerCorrectionReceipt> {
  return nativeInvoke("owner_correction_confirm", {
    request: { correction, previewFingerprint },
  });
}

export function correctionHistory(
  books: BooksRef,
  recordKind: "moneyIn" | "moneyOut",
  recordId: string,
  limit = 100,
): Promise<OwnerCorrectionHistory> {
  return nativeInvoke("owner_correction_history", {
    request: { books, recordKind, recordId, limit },
  });
}

export function reviewCsvImport(
  books: BooksRef,
  statementText: string,
  profile: CsvProfile,
): Promise<OwnerBankImportReview> {
  return nativeInvoke("owner_bank_import_review_csv", {
    request: {
      books,
      sourceAccountId: "bank-main",
      statementText,
      profile,
    },
  });
}

export function reviewOfxImport(
  books: BooksRef,
  statementText: string,
  format: OfxFormat,
): Promise<OwnerBankImportReview> {
  return nativeInvoke("owner_bank_import_review_ofx_qfx", {
    request: {
      books,
      sourceAccountId: "bank-main",
      statementText,
      format,
    },
  });
}

export function confirmCsvImport(
  books: BooksRef,
  statementText: string,
  profile: CsvProfile,
  review: OwnerBankImportReview,
): Promise<OwnerBankImportReceipt> {
  return nativeInvoke("owner_bank_import_confirm_csv", {
    request: {
      books,
      sourceAccountId: "bank-main",
      statementText,
      profile,
      confirmedStatementSha256: review.statementSha256,
      confirmedLines: review.lines,
      confirmedDuplicateReviews: review.duplicateReviews,
    },
  });
}

export function confirmOfxImport(
  books: BooksRef,
  statementText: string,
  format: OfxFormat,
  review: OwnerBankImportReview,
): Promise<OwnerBankImportReceipt> {
  return nativeInvoke("owner_bank_import_confirm_ofx_qfx", {
    request: {
      books,
      sourceAccountId: "bank-main",
      statementText,
      format,
      confirmedStatementSha256: review.statementSha256,
      confirmedLines: review.lines,
      confirmedDuplicateReviews: review.duplicateReviews,
    },
  });
}

export function listBankActivity(
  books: BooksRef,
  limit = 200,
  offset = 0,
): Promise<OwnerBankActivityPage> {
  return nativeInvoke("owner_bank_activity_list", {
    request: { books, limit, offset },
  });
}

export function bankActivityDetail(
  books: BooksRef,
  bankActivityId: number,
): Promise<OwnerBankActivityDetail> {
  return nativeInvoke("owner_bank_activity_detail", {
    request: { books, bankActivityId },
  });
}

export function reviewPersistedBankActivityMatch(
  books: BooksRef,
  bankActivityId: number,
): Promise<OwnerBankMatchReview> {
  return nativeInvoke("owner_bank_activity_match_review", {
    request: { books, bankActivityId },
  });
}

export function confirmBankMatch(
  books: BooksRef,
  bankActivityId: number,
  candidateTransactionIds: number[],
  selectedTransactionId: number,
): Promise<OwnerBankMatchReceipt> {
  return nativeInvoke("owner_bank_match_confirm", {
    request: {
      books,
      bankActivityId,
      candidateTransactionIds,
      selectedTransactionId,
    },
  });
}

export function previewReconciliation(
  books: BooksRef,
  openingBalancePence: number,
  endingBalancePence: number,
  transactionIds: number[],
): Promise<OwnerReconciliationPreview> {
  return nativeInvoke("owner_bank_reconcile_preview", {
    request: {
      books,
      openingBalancePence,
      endingBalancePence,
      transactionIds,
    },
  });
}

export function finaliseReconciliation(
  books: BooksRef,
  statementId: string,
  statementDate: string,
  openingBalancePence: number,
  endingBalancePence: number,
  transactionIds: number[],
): Promise<OwnerBankReconciliationReceipt> {
  return nativeInvoke("owner_bank_reconcile_finalise", {
    request: {
      books,
      statementId,
      statementDate,
      openingBalancePence,
      endingBalancePence,
      transactionIds,
    },
  });
}
