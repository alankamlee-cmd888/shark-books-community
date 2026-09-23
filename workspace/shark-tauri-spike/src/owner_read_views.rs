//! SBC-7B2 bounded owner read/view bridge.
//!
//! This module exposes only owner-safe read shapes. It never serializes ledger
//! entries, raw database authority, document paths/hashes, or mutation inputs.

use serde::{Deserialize, Serialize};
use shark_foundation::{
    BankActivityView, Books, FoundationError, FoundationErrorCode, OwnerDocumentRead,
    OwnerMoneyRecordDetailRead, OwnerMoneyRecordRead,
};

use super::{OpenBooksRequest, open_books_impl};

const OWNER_READ_BRIDGE_VERSION: u32 = 1;
const MAX_READ_PAGE: i64 = 200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReadError {
    code: &'static str,
    message: String,
}

impl OwnerReadError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalidInput",
            message: message.into(),
        }
    }

    fn foundation(error: FoundationError) -> Self {
        let code = match error.code {
            FoundationErrorCode::InvalidInput | FoundationErrorCode::Validation => "invalidInput",
            FoundationErrorCode::NotFound => "notFound",
            _ => "booksOperationFailed",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

type OwnerReadResult<T> = Result<T, OwnerReadError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnerReadBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerReadBooksRef {
    fn open(&self) -> OwnerReadResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerReadError::foundation)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerMoneyReadKind {
    MoneyIn,
    MoneyOut,
}

impl OwnerMoneyReadKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MoneyIn => "moneyIn",
            Self::MoneyOut => "moneyOut",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerMoneyRecordsListRequest {
    books: OwnerReadBooksRef,
    record_kind: OwnerMoneyReadKind,
    limit: i64,
    offset: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerMoneyRecordDetailRequest {
    books: OwnerReadBooksRef,
    record_kind: OwnerMoneyReadKind,
    record_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerMoneyRecordsPage {
    bridge_version: u32,
    rows: Vec<OwnerMoneyRecordRead>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerMoneyRecordDetail {
    bridge_version: u32,
    detail: OwnerMoneyRecordDetailRead,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBankActivityDetailRequest {
    books: OwnerReadBooksRef,
    bank_activity_id: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankActivityDetail {
    bridge_version: u32,
    bank_activity_id: i64,
    source_account_id: String,
    posted_date: String,
    value_date: Option<String>,
    signed_amount_pence: i64,
    currency: String,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    source_format: String,
    imported_at: String,
    imported_by: String,
    matched_transaction_id: Option<i64>,
    match_level: Option<String>,
    match_score: Option<u16>,
    match_reasons: Vec<String>,
    clearance_state: Option<String>,
    has_strong_source_identity: bool,
}

fn bank_detail(value: BankActivityView) -> OwnerBankActivityDetail {
    OwnerBankActivityDetail {
        bridge_version: OWNER_READ_BRIDGE_VERSION,
        bank_activity_id: value.id,
        source_account_id: value.activity.source_account_id,
        posted_date: value.activity.posted_date,
        value_date: value.activity.value_date,
        signed_amount_pence: value.activity.signed_amount_minor,
        currency: value.activity.currency_code,
        description: value.activity.description,
        payee: value.activity.payee,
        reference: value.activity.reference,
        source_format: value.activity.source_format,
        imported_at: value.imported_at,
        imported_by: value.imported_by,
        matched_transaction_id: value.matched_transaction_id,
        match_level: value.match_level,
        match_score: value.match_score,
        match_reasons: value.match_reasons,
        clearance_state: value.clearance_state,
        has_strong_source_identity: value.activity.strong_identity_key.is_some(),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerDocumentListRequest {
    books: OwnerReadBooksRef,
    limit: i64,
    offset: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerDocumentPage {
    bridge_version: u32,
    rows: Vec<OwnerDocumentRead>,
}

fn validate_page(limit: i64, offset: i64) -> OwnerReadResult<()> {
    if !(1..=MAX_READ_PAGE).contains(&limit) {
        return Err(OwnerReadError::invalid(
            "read page limit must be between 1 and 200",
        ));
    }
    if !(0..=1_000_000).contains(&offset) {
        return Err(OwnerReadError::invalid(
            "read page offset is outside the supported range",
        ));
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn owner_money_records_list(
    request: OwnerMoneyRecordsListRequest,
) -> OwnerReadResult<OwnerMoneyRecordsPage> {
    validate_page(request.limit, request.offset)?;
    let books = request.books.open()?;
    let rows = books
        .owner_money_records(request.record_kind.as_str(), request.limit, request.offset)
        .map_err(OwnerReadError::foundation)?;
    Ok(OwnerMoneyRecordsPage {
        bridge_version: OWNER_READ_BRIDGE_VERSION,
        rows,
    })
}

#[tauri::command]
pub(crate) fn owner_money_record_detail(
    request: OwnerMoneyRecordDetailRequest,
) -> OwnerReadResult<OwnerMoneyRecordDetail> {
    if request.record_id.trim().is_empty() || request.record_id.len() > 128 {
        return Err(OwnerReadError::invalid(
            "owner record id must be non-empty and at most 128 bytes",
        ));
    }
    let books = request.books.open()?;
    let detail = books
        .owner_money_record_detail(request.record_kind.as_str(), &request.record_id)
        .map_err(OwnerReadError::foundation)?;
    Ok(OwnerMoneyRecordDetail {
        bridge_version: OWNER_READ_BRIDGE_VERSION,
        detail,
    })
}

#[tauri::command]
pub(crate) fn owner_bank_activity_detail(
    request: OwnerBankActivityDetailRequest,
) -> OwnerReadResult<OwnerBankActivityDetail> {
    if request.bank_activity_id <= 0 {
        return Err(OwnerReadError::invalid("bank activity id must be positive"));
    }
    let books = request.books.open()?;
    let value = books
        .bank_activity(request.bank_activity_id)
        .map_err(OwnerReadError::foundation)?;
    Ok(bank_detail(value))
}

#[tauri::command]
pub(crate) fn owner_document_list(
    request: OwnerDocumentListRequest,
) -> OwnerReadResult<OwnerDocumentPage> {
    validate_page(request.limit, request.offset)?;
    let books = request.books.open()?;
    let rows = books
        .owner_documents(request.limit, request.offset)
        .map_err(OwnerReadError::foundation)?;
    Ok(OwnerDocumentPage {
        bridge_version: OWNER_READ_BRIDGE_VERSION,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_read_requests_reject_raw_authority_fields() {
        let base = serde_json::json!({
            "books": {
                "fileName": "owner-read.sqlite",
                "booksId": "owner-read",
                "actor": "owner"
            },
            "recordKind": "moneyOut",
            "limit": 50,
            "offset": 0
        });
        assert!(serde_json::from_value::<OwnerMoneyRecordsListRequest>(base.clone()).is_ok());
        for forbidden in [
            "databasePath",
            "passphrase",
            "accountCode",
            "direction",
            "taxCategory",
            "sourcePath",
            "fileUrl",
        ] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().insert(
                forbidden.to_string(),
                serde_json::Value::String("forbidden".to_string()),
            );
            assert!(
                serde_json::from_value::<OwnerMoneyRecordsListRequest>(bad).is_err(),
                "forbidden field accepted: {forbidden}"
            );
        }
    }

    #[test]
    fn bank_detail_omits_hash_locator_and_entry_authority() {
        let value = BankActivityView {
            id: 7,
            activity: shark_foundation::BankActivityWrite {
                source_account_id: "bank-main".to_string(),
                institution_account_id: Some("secret-institution-id".to_string()),
                source_format: "csv".to_string(),
                source_file_sha256: "a".repeat(64),
                source_locator: "csv:row:2".to_string(),
                posted_date: "2026-09-01".to_string(),
                value_date: None,
                signed_amount_minor: -2500,
                currency_code: "GBP".to_string(),
                description: "Supplier".to_string(),
                payee: Some("Supplier Ltd".to_string()),
                reference: Some("REF-1".to_string()),
                external_transaction_id: None,
                raw_record_sha256: "b".repeat(64),
                strong_identity_key: None,
                provenance_kind: "csv".to_string(),
                provenance_source_reference: Some("csv:row:2".to_string()),
                provenance_fingerprint: Some(format!("sha256:{}", "b".repeat(64))),
                provenance_label: Some("profile".to_string()),
            },
            imported_at: "2026-09-01 12:00:00".to_string(),
            imported_by: "owner".to_string(),
            matched_transaction_id: None,
            matched_entry_id: None,
            match_level: None,
            match_score: None,
            match_reasons: Vec::new(),
            clearance_state: None,
        };
        let json = serde_json::to_string(&bank_detail(value)).unwrap();
        for forbidden in [
            "sourceFileSha256",
            "sourceLocator",
            "rawRecordSha256",
            "provenanceFingerprint",
            "matchedEntryId",
            "institutionAccountId",
        ] {
            assert!(
                !json.contains(forbidden),
                "unsafe field leaked: {forbidden}"
            );
        }
    }
}
