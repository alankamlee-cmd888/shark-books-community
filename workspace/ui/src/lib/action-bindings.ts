import { UIA_ACTIONS } from "../generated/uia-actions";
import { UIB_ACTIONS } from "../generated/uib-actions";

export const UIA_BINDINGS = {
  booksCreate: { action: UIA_ACTIONS.booksCreate, command: "books_create" },
  booksOpen: { action: UIA_ACTIONS.booksOpen, command: "books_open" },
  booksVerify: { action: UIA_ACTIONS.booksVerify, command: "books_verify" },
  homeStatus: { action: UIA_ACTIONS.homeStatus, command: "owner_home_status" },

  moneyInPreview: {
    action: UIA_ACTIONS.moneyInPreview,
    command: "owner_money_in_preview",
  },
  moneyInSave: { action: UIA_ACTIONS.moneyInSave, command: "owner_money_in_save" },
  moneyOutPreview: {
    action: UIA_ACTIONS.moneyOutPreview,
    command: "owner_money_out_preview",
  },
  moneyOutSave: {
    action: UIA_ACTIONS.moneyOutSave,
    command: "owner_money_out_save",
  },
  moneyRecordsList: {
    action: UIA_ACTIONS.moneyRecordsList,
    command: "owner_money_records_list",
  },
  moneyRecordDetail: {
    action: UIA_ACTIONS.moneyRecordDetail,
    command: "owner_money_record_detail",
  },
  correctionPreview: {
    action: UIA_ACTIONS.correctionPreview,
    command: "owner_correction_preview",
  },
  correctionConfirm: {
    action: UIA_ACTIONS.correctionConfirm,
    command: "owner_correction_confirm",
  },
  correctionHistory: {
    action: UIA_ACTIONS.correctionHistory,
    command: "owner_correction_history",
  },

  bankImportReviewCsv: {
    action: UIA_ACTIONS.bankImportReviewCsv,
    command: "owner_bank_import_review_csv",
  },
  bankImportReviewOfxQfx: {
    action: UIA_ACTIONS.bankImportReviewOfxQfx,
    command: "owner_bank_import_review_ofx_qfx",
  },
  bankImportConfirmCsv: {
    action: UIA_ACTIONS.bankImportConfirmCsv,
    command: "owner_bank_import_confirm_csv",
  },
  bankImportConfirmOfxQfx: {
    action: UIA_ACTIONS.bankImportConfirmOfxQfx,
    command: "owner_bank_import_confirm_ofx_qfx",
  },
  bankActivityList: {
    action: UIA_ACTIONS.bankActivityList,
    command: "owner_bank_activity_list",
  },
  bankActivityDetail: {
    action: UIA_ACTIONS.bankActivityDetail,
    command: "owner_bank_activity_detail",
  },
  bankMatchReview: {
    action: UIA_ACTIONS.bankMatchReview,
    command: "owner_bank_activity_match_review",
  },
  bankMatchConfirm: {
    action: UIA_ACTIONS.bankMatchConfirm,
    command: "owner_bank_match_confirm",
  },
  bankReconcilePreview: {
    action: UIA_ACTIONS.bankReconcilePreview,
    command: "owner_bank_reconcile_preview",
  },
  bankReconcileFinalise: {
    action: UIA_ACTIONS.bankReconcileFinalise,
    command: "owner_bank_reconcile_finalise",
  },
} as const;

export const UIB_BINDINGS = {
  documentSelectRegister: { action: UIB_ACTIONS.documentSelectRegister, command: "owner_document_select_register" },
  documentVerify: { action: UIB_ACTIONS.documentVerify, command: "owner_document_verify" },
  documentList: { action: UIB_ACTIONS.documentList, command: "owner_document_list" },
  documentOpenView: { action: UIB_ACTIONS.documentOpenView, command: "owner_document_open_view" },
  documentAttach: { action: UIB_ACTIONS.documentAttach, command: "owner_document_attach" },
  ocrExtract: { action: UIB_ACTIONS.ocrExtract, command: "owner_ocr_extract_receipt" },
  receiptSuggestBank: { action: UIB_ACTIONS.receiptSuggestBank, command: "owner_receipt_suggest_bank" },
  receiptConfirmBank: { action: UIB_ACTIONS.receiptConfirmBank, command: "owner_receipt_confirm_bank" },
  receiptRejectBank: { action: UIB_ACTIONS.receiptRejectBank, command: "owner_receipt_reject_bank" },
  contactsList: { action: UIB_ACTIONS.contactsList, command: "owner_contacts_list" },
  contactsSave: { action: UIB_ACTIONS.contactsSave, command: "owner_contacts_save" },
  reportSummary: { action: UIB_ACTIONS.reportSummary, command: "owner_report_summary" },
  settingsBooksInfo: { action: UIB_ACTIONS.settingsBooksInfo, command: "owner_settings_books_info" },
  settingsStorageRoot: { action: UIB_ACTIONS.settingsStorageRoot, command: "owner_settings_storage_root_select" },
} as const;
