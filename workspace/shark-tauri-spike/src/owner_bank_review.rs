//! SBC-7B1 bounded owner-facing bank review bridge.
//!
//! This slice is deliberately read-only. It exposes deterministic bank-file
//! preview, ledger-match review and reconciliation preview over already-proven
//! Shark product/foundation contracts. It does not persist imports, confirm a
//! match, finalise reconciliation, expose raw database objects or trust an OS
//! path supplied by the webview.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{Books, EntryView, FoundationError, TransactionView};

use super::{open_books_impl, OpenBooksRequest};

const OWNER_BANK_BRIDGE_VERSION: u32 = 1;
const BANK_ACCOUNT_CODE: &str = "1000";
const MAX_STATEMENT_BYTES: usize = 10 * 1024 * 1024;
const MAX_MATCH_CANDIDATES: usize = 100;
const MAX_RECONCILIATION_ROWS: usize = 1_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankError {
    code: &'static str,
    message: String,
}

impl OwnerBankError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalidInput",
            message: message.into(),
        }
    }

    fn bank(error: core::bank_import::BankImportError) -> Self {
        Self {
            code: "bankImportInvalid",
            message: error.to_string(),
        }
    }

    fn matching(error: core::matching::MatchingError) -> Self {
        Self {
            code: "bankReviewInvalid",
            message: error.to_string(),
        }
    }

    fn foundation(error: FoundationError) -> Self {
        Self {
            code: "booksOperationFailed",
            message: error.to_string(),
        }
    }
}

type OwnerBankResult<T> = Result<T, OwnerBankError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBankBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerBankBooksRef {
    fn open(&self) -> OwnerBankResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerBankError::foundation)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerCsvDateFormat {
    IsoYmd,
    DmySlash,
}

impl From<OwnerCsvDateFormat> for core::bank_import::CsvDateFormat {
    fn from(value: OwnerCsvDateFormat) -> Self {
        match value {
            OwnerCsvDateFormat::IsoYmd => Self::IsoYmd,
            OwnerCsvDateFormat::DmySlash => Self::DmySlash,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum OwnerCsvAmountMapping {
    Signed { amount_header: String },
    DebitCredit {
        debit_header: String,
        credit_header: String,
    },
}

impl OwnerCsvAmountMapping {
    fn to_core(&self) -> core::bank_import::CsvAmountMapping {
        match self {
            Self::Signed { amount_header } => core::bank_import::CsvAmountMapping::Signed {
                amount_header: amount_header.clone(),
            },
            Self::DebitCredit {
                debit_header,
                credit_header,
            } => core::bank_import::CsvAmountMapping::DebitCredit {
                debit_header: debit_header.clone(),
                credit_header: credit_header.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCsvProfile {
    id: String,
    name: String,
    delimiter: String,
    date_header: String,
    value_date_header: Option<String>,
    description_header: String,
    payee_header: Option<String>,
    reference_header: Option<String>,
    transaction_id_header: Option<String>,
    currency_header: Option<String>,
    amount_mapping: OwnerCsvAmountMapping,
    date_format: OwnerCsvDateFormat,
}

impl OwnerCsvProfile {
    fn to_core(&self) -> OwnerBankResult<core::bank_import::CsvMappingProfile> {
        let mut chars = self.delimiter.chars();
        let delimiter = chars
            .next()
            .ok_or_else(|| OwnerBankError::invalid("CSV delimiter must contain one character"))?;
        if chars.next().is_some() {
            return Err(OwnerBankError::invalid(
                "CSV delimiter must contain exactly one character",
            ));
        }
        core::bank_import::CsvMappingProfile::new(
            core::RecordId::new(self.id.clone()).map_err(|e| OwnerBankError::invalid(e.to_string()))?,
            self.name.clone(),
            delimiter,
            self.date_header.clone(),
            self.value_date_header.clone(),
            self.description_header.clone(),
            self.payee_header.clone(),
            self.reference_header.clone(),
            self.transaction_id_header.clone(),
            self.currency_header.clone(),
            self.amount_mapping.to_core(),
            self.date_format.into(),
        )
        .map_err(OwnerBankError::bank)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerOfxFormat {
    Ofx,
    Qfx,
}

impl From<OwnerOfxFormat> for core::bank_import::BankSourceFormat {
    fn from(value: OwnerOfxFormat) -> Self {
        match value {
            OwnerOfxFormat::Ofx => Self::Ofx,
            OwnerOfxFormat::Qfx => Self::Qfx,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCsvPreviewRequest {
    source_account_id: String,
    statement_text: String,
    profile: OwnerCsvProfile,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerOfxPreviewRequest {
    source_account_id: String,
    statement_text: String,
    format: OwnerOfxFormat,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankLineView {
    line_ref: String,
    posted_date: String,
    value_date: Option<String>,
    signed_amount_pence: i64,
    currency: &'static str,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    source_format: &'static str,
    has_strong_source_identity: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankPreviewError {
    line_ref: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankImportPreview {
    bridge_version: u32,
    can_confirm: bool,
    requires_explicit_confirmation: bool,
    lines: Vec<OwnerBankLineView>,
    errors: Vec<OwnerBankPreviewError>,
}

fn require_statement_text(text: &str) -> OwnerBankResult<()> {
    if text.trim().is_empty() {
        return Err(OwnerBankError::invalid("statement text must not be empty"));
    }
    if text.len() > MAX_STATEMENT_BYTES {
        return Err(OwnerBankError::invalid("statement text exceeds the bounded 10 MiB limit"));
    }
    Ok(())
}

fn source_account(value: &str) -> OwnerBankResult<core::RecordId> {
    core::RecordId::new(value.to_string()).map_err(|e| OwnerBankError::invalid(e.to_string()))
}

fn source_format_name(value: core::bank_import::BankSourceFormat) -> &'static str {
    match value {
        core::bank_import::BankSourceFormat::Csv => "csv",
        core::bank_import::BankSourceFormat::Ofx => "ofx",
        core::bank_import::BankSourceFormat::Qfx => "qfx",
    }
}

fn line_view(line: &core::bank_import::BankLine) -> OwnerBankLineView {
    OwnerBankLineView {
        line_ref: line.source_locator().to_string(),
        posted_date: line.posted_date().iso(),
        value_date: line.value_date().map(core::Date::iso),
        signed_amount_pence: line.signed_amount_minor(),
        currency: line.currency_code(),
        description: line.description().to_string(),
        payee: line.payee().map(str::to_string),
        reference: line.reference().map(str::to_string),
        source_format: source_format_name(line.source_format()),
        has_strong_source_identity: line.strong_identity_key().is_some(),
    }
}

fn preview_view(preview: core::bank_import::BankImportPreview) -> OwnerBankImportPreview {
    OwnerBankImportPreview {
        bridge_version: OWNER_BANK_BRIDGE_VERSION,
        can_confirm: preview.can_commit(),
        requires_explicit_confirmation: true,
        lines: preview.lines().iter().map(line_view).collect(),
        errors: preview
            .errors()
            .iter()
            .map(|error| OwnerBankPreviewError {
                line_ref: error.locator().to_string(),
                message: error.message().to_string(),
            })
            .collect(),
    }
}

fn preview_csv(request: &OwnerCsvPreviewRequest) -> OwnerBankResult<core::bank_import::BankImportPreview> {
    require_statement_text(&request.statement_text)?;
    let profile = request.profile.to_core()?;
    core::bank_import::preview_csv(
        &request.statement_text,
        source_account(&request.source_account_id)?,
        &profile,
    )
    .map_err(OwnerBankError::bank)
}

fn preview_ofx(request: &OwnerOfxPreviewRequest) -> OwnerBankResult<core::bank_import::BankImportPreview> {
    require_statement_text(&request.statement_text)?;
    core::bank_import::preview_ofx_or_qfx(
        &request.statement_text,
        source_account(&request.source_account_id)?,
        request.format.into(),
    )
    .map_err(OwnerBankError::bank)
}

#[tauri::command]
pub(crate) fn owner_bank_import_preview_csv(
    request: OwnerCsvPreviewRequest,
) -> OwnerBankResult<OwnerBankImportPreview> {
    preview_csv(&request).map(preview_view)
}

#[tauri::command]
pub(crate) fn owner_bank_import_preview_ofx_qfx(
    request: OwnerOfxPreviewRequest,
) -> OwnerBankResult<OwnerBankImportPreview> {
    preview_ofx(&request).map(preview_view)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum OwnerStatementSelection {
    Csv {
        source_account_id: String,
        statement_text: String,
        profile: OwnerCsvProfile,
        line_ref: String,
    },
    OfxQfx {
        source_account_id: String,
        statement_text: String,
        format: OwnerOfxFormat,
        line_ref: String,
    },
}

impl OwnerStatementSelection {
    fn resolve(&self) -> OwnerBankResult<core::bank_import::BankLine> {
        let (preview, line_ref) = match self {
            Self::Csv {
                source_account_id,
                statement_text,
                profile,
                line_ref,
            } => {
                let request = OwnerCsvPreviewRequest {
                    source_account_id: source_account_id.clone(),
                    statement_text: statement_text.clone(),
                    profile: profile.clone(),
                };
                (preview_csv(&request)?, line_ref)
            }
            Self::OfxQfx {
                source_account_id,
                statement_text,
                format,
                line_ref,
            } => {
                let request = OwnerOfxPreviewRequest {
                    source_account_id: source_account_id.clone(),
                    statement_text: statement_text.clone(),
                    format: *format,
                };
                (preview_ofx(&request)?, line_ref)
            }
        };
        preview
            .lines()
            .iter()
            .find(|line| line.source_locator() == line_ref)
            .cloned()
            .ok_or_else(|| OwnerBankError::invalid("selected bank line is not present in the deterministic preview"))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBankMatchReviewRequest {
    books: OwnerBankBooksRef,
    statement: OwnerStatementSelection,
    candidate_transaction_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankMatchCandidateView {
    transaction_id: i64,
    level: &'static str,
    score: u16,
    reasons: Vec<&'static str>,
    requires_explicit_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankMatchReview {
    bridge_version: u32,
    bank_line: OwnerBankLineView,
    candidates: Vec<OwnerBankMatchCandidateView>,
    recommended_transaction_id: Option<i64>,
    ambiguous_top: bool,
    requires_explicit_confirmation: bool,
}

fn match_level_name(value: core::matching::MatchLevel) -> &'static str {
    match value {
        core::matching::MatchLevel::Unmatched => "unmatched",
        core::matching::MatchLevel::Possible => "possible",
        core::matching::MatchLevel::Likely => "likely",
        core::matching::MatchLevel::Exact => "exact",
    }
}

fn match_reason_name(value: core::matching::MatchReason) -> &'static str {
    use core::matching::MatchReason;
    match value {
        MatchReason::AccountExact => "accountExact",
        MatchReason::CurrencyExact => "currencyExact",
        MatchReason::AmountExact => "amountExact",
        MatchReason::DateExact => "dateExact",
        MatchReason::DateWithinThreeDays => "dateWithinThreeDays",
        MatchReason::DateWithinSevenDays => "dateWithinSevenDays",
        MatchReason::SourceIdentityExact => "sourceIdentityExact",
        MatchReason::ReferenceExact => "referenceExact",
        MatchReason::DescriptionStrong => "descriptionStrong",
        MatchReason::PayeeStrong => "payeeStrong",
        MatchReason::MultipleTopCandidates => "multipleTopCandidates",
        MatchReason::AmountMismatch => "amountMismatch",
        MatchReason::AccountMismatch => "accountMismatch",
        MatchReason::CurrencyMismatch => "currencyMismatch",
        MatchReason::DateTooFar => "dateTooFar",
        MatchReason::InsufficientEvidence => "insufficientEvidence",
    }
}

fn parse_date(value: &str) -> OwnerBankResult<core::Date> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .ok_or_else(|| OwnerBankError::invalid("transaction date must be YYYY-MM-DD"))?;
    let month = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerBankError::invalid("transaction date must be YYYY-MM-DD"))?;
    let day = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerBankError::invalid("transaction date must be YYYY-MM-DD"))?;
    if parts.next().is_some() || value.len() != 10 {
        return Err(OwnerBankError::invalid("transaction date must be YYYY-MM-DD"));
    }
    core::Date::new(year, month, day).map_err(|e| OwnerBankError::invalid(e.to_string()))
}

#[derive(Debug, Clone, Copy)]
struct ResolvedBankEntry {
    transaction_id: i64,
    entry_id: i64,
    signed_amount_pence: i64,
    state: core::matching::ClearanceState,
}

fn clearance_state(status: &str) -> OwnerBankResult<core::matching::ClearanceState> {
    match status {
        "uncleared" => Ok(core::matching::ClearanceState::Uncleared),
        "cleared" => Ok(core::matching::ClearanceState::Cleared),
        "reconciled" => Ok(core::matching::ClearanceState::Reconciled),
        other => Err(OwnerBankError::invalid(format!(
            "unsupported bank-entry clearance state '{other}'"
        ))),
    }
}

fn resolved_bank_entry(transaction: &TransactionView) -> OwnerBankResult<ResolvedBankEntry> {
    let mut matching = transaction
        .entries
        .iter()
        .filter(|entry| entry.account_code == BANK_ACCOUNT_CODE);
    let entry = matching
        .next()
        .ok_or_else(|| OwnerBankError::invalid("candidate transaction has no Business Bank entry"))?;
    if matching.next().is_some() {
        return Err(OwnerBankError::invalid(
            "candidate transaction has more than one Business Bank entry",
        ));
    }
    let signed_amount_pence = match entry.direction.as_str() {
        "debit" => entry.amount_minor,
        "credit" => entry
            .amount_minor
            .checked_neg()
            .ok_or_else(|| OwnerBankError::invalid("bank-entry amount overflow"))?,
        other => {
            return Err(OwnerBankError::invalid(format!(
                "unsupported bank-entry direction '{other}'"
            )))
        }
    };
    Ok(ResolvedBankEntry {
        transaction_id: transaction.id,
        entry_id: entry.id,
        signed_amount_pence,
        state: clearance_state(&entry.status)?,
    })
}

fn ledger_candidate(
    transaction: &TransactionView,
    source_account_id: core::RecordId,
) -> OwnerBankResult<core::matching::LedgerCandidate> {
    let bank = resolved_bank_entry(transaction)?;
    core::matching::LedgerCandidate::new(
        core::RecordId::new(format!("transaction-{}", transaction.id))
            .map_err(|e| OwnerBankError::invalid(e.to_string()))?,
        source_account_id,
        parse_date(&transaction.date)?,
        bank.signed_amount_pence,
        transaction.description.clone(),
        None,
        transaction.reference.clone(),
        None,
        bank.state,
    )
    .map_err(OwnerBankError::matching)
}

fn require_transaction_ids(ids: &[i64], maximum: usize, label: &str) -> OwnerBankResult<()> {
    if ids.is_empty() {
        return Err(OwnerBankError::invalid(format!("{label} must not be empty")));
    }
    if ids.len() > maximum {
        return Err(OwnerBankError::invalid(format!("{label} exceeds the bounded item limit")));
    }
    if ids.iter().any(|id| *id <= 0) {
        return Err(OwnerBankError::invalid(format!("{label} contains an invalid transaction id")));
    }
    let unique: HashSet<i64> = ids.iter().copied().collect();
    if unique.len() != ids.len() {
        return Err(OwnerBankError::invalid(format!("{label} contains duplicate transaction ids")));
    }
    Ok(())
}

fn match_review_from_transactions(
    bank_line: &core::bank_import::BankLine,
    transactions: &[TransactionView],
) -> OwnerBankResult<OwnerBankMatchReview> {
    let mut by_record_id = HashMap::<String, i64>::new();
    let mut candidates = Vec::with_capacity(transactions.len());
    for transaction in transactions {
        let candidate = ledger_candidate(transaction, bank_line.source_account_id().clone())?;
        by_record_id.insert(candidate.id().as_str().to_string(), transaction.id);
        candidates.push(candidate);
    }

    let ranked = core::matching::rank_matches(bank_line, &candidates);
    let recommended_transaction_id = ranked
        .recommended_candidate_id()
        .and_then(|id| by_record_id.get(id.as_str()).copied());
    let candidates = ranked
        .candidates()
        .iter()
        .map(|candidate| {
            let transaction_id = by_record_id
                .get(candidate.candidate_id().as_str())
                .copied()
                .ok_or_else(|| OwnerBankError::invalid("match result lost transaction identity"))?;
            Ok(OwnerBankMatchCandidateView {
                transaction_id,
                level: match_level_name(candidate.level()),
                score: candidate.score(),
                reasons: candidate.reasons().iter().copied().map(match_reason_name).collect(),
                requires_explicit_confirmation: candidate.requires_user_confirmation(),
            })
        })
        .collect::<OwnerBankResult<Vec<_>>>()?;

    Ok(OwnerBankMatchReview {
        bridge_version: OWNER_BANK_BRIDGE_VERSION,
        bank_line: line_view(bank_line),
        candidates,
        recommended_transaction_id,
        ambiguous_top: ranked.ambiguous_top(),
        requires_explicit_confirmation: ranked.requires_user_confirmation(),
    })
}

#[tauri::command]
pub(crate) fn owner_bank_match_review(
    request: OwnerBankMatchReviewRequest,
) -> OwnerBankResult<OwnerBankMatchReview> {
    require_transaction_ids(
        &request.candidate_transaction_ids,
        MAX_MATCH_CANDIDATES,
        "candidate transaction ids",
    )?;
    let bank_line = request.statement.resolve()?;
    let books = request.books.open()?;
    let transactions = request
        .candidate_transaction_ids
        .iter()
        .map(|id| books.transaction(*id).map_err(OwnerBankError::foundation))
        .collect::<OwnerBankResult<Vec<_>>>()?;
    match_review_from_transactions(&bank_line, &transactions)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerReconciliationPreviewRequest {
    books: OwnerBankBooksRef,
    opening_balance_pence: i64,
    ending_balance_pence: i64,
    transaction_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReconciliationRowView {
    transaction_id: i64,
    bank_entry_id: i64,
    signed_amount_pence: i64,
    clearance_state: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReconciliationPreview {
    bridge_version: u32,
    opening_balance_pence: i64,
    computed_ending_balance_pence: i64,
    expected_ending_balance_pence: i64,
    difference_pence: i64,
    all_entries_cleared: bool,
    can_finalise: bool,
    requires_explicit_confirmation: bool,
    rows: Vec<OwnerReconciliationRowView>,
}

fn clearance_state_name(value: core::matching::ClearanceState) -> &'static str {
    match value {
        core::matching::ClearanceState::Uncleared => "uncleared",
        core::matching::ClearanceState::Cleared => "cleared",
        core::matching::ClearanceState::Reconciled => "reconciled",
    }
}

fn reconciliation_preview_from_transactions(
    opening_balance_pence: i64,
    ending_balance_pence: i64,
    transactions: &[TransactionView],
) -> OwnerBankResult<OwnerReconciliationPreview> {
    let mut rows = Vec::with_capacity(transactions.len());
    let mut entries = Vec::with_capacity(transactions.len());
    for transaction in transactions {
        let bank = resolved_bank_entry(transaction)?;
        let record_id = core::RecordId::new(format!(
            "transaction-{}-entry-{}",
            bank.transaction_id, bank.entry_id
        ))
        .map_err(|e| OwnerBankError::invalid(e.to_string()))?;
        entries.push(
            core::matching::ReconciliationEntry::new(
                record_id,
                bank.signed_amount_pence,
                bank.state,
            )
            .map_err(OwnerBankError::matching)?,
        );
        rows.push(OwnerReconciliationRowView {
            transaction_id: bank.transaction_id,
            bank_entry_id: bank.entry_id,
            signed_amount_pence: bank.signed_amount_pence,
            clearance_state: clearance_state_name(bank.state),
        });
    }
    let check = core::matching::preview_reconciliation(
        opening_balance_pence,
        ending_balance_pence,
        &entries,
    )
    .map_err(OwnerBankError::matching)?;
    Ok(OwnerReconciliationPreview {
        bridge_version: OWNER_BANK_BRIDGE_VERSION,
        opening_balance_pence,
        computed_ending_balance_pence: check.computed_ending_balance_minor(),
        expected_ending_balance_pence: check.expected_ending_balance_minor(),
        difference_pence: check.difference_minor(),
        all_entries_cleared: check.all_entries_cleared(),
        can_finalise: check.can_finalize(),
        requires_explicit_confirmation: check.can_finalize(),
        rows,
    })
}

#[tauri::command]
pub(crate) fn owner_bank_reconcile_preview(
    request: OwnerReconciliationPreviewRequest,
) -> OwnerBankResult<OwnerReconciliationPreview> {
    require_transaction_ids(
        &request.transaction_ids,
        MAX_RECONCILIATION_ROWS,
        "reconciliation transaction ids",
    )?;
    let books = request.books.open()?;
    let transactions = request
        .transaction_ids
        .iter()
        .map(|id| books.transaction(*id).map_err(OwnerBankError::foundation))
        .collect::<OwnerBankResult<Vec<_>>>()?;
    reconciliation_preview_from_transactions(
        request.opening_balance_pence,
        request.ending_balance_pence,
        &transactions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> OwnerCsvProfile {
        OwnerCsvProfile {
            id: "bank-profile".into(),
            name: "Test profile".into(),
            delimiter: ",".into(),
            date_header: "Date".into(),
            value_date_header: None,
            description_header: "Description".into(),
            payee_header: None,
            reference_header: Some("Reference".into()),
            transaction_id_header: Some("TransactionId".into()),
            currency_header: None,
            amount_mapping: OwnerCsvAmountMapping::Signed {
                amount_header: "Amount".into(),
            },
            date_format: OwnerCsvDateFormat::IsoYmd,
        }
    }

    fn entry(id: i64, direction: &str, amount: i64, status: &str) -> EntryView {
        EntryView {
            id,
            account_code: BANK_ACCOUNT_CODE.to_string(),
            direction: direction.to_string(),
            amount_minor: amount,
            status: status.to_string(),
        }
    }

    fn transaction(id: i64, date: &str, description: &str, direction: &str, amount: i64, status: &str) -> TransactionView {
        TransactionView {
            id,
            description: description.to_string(),
            reference: None,
            currency: "GBP".to_string(),
            date: date.to_string(),
            entries: vec![entry(id * 10, direction, amount, status)],
        }
    }

    #[test]
    fn csv_preview_is_owner_safe_and_confirmation_bound() {
        let preview = owner_bank_import_preview_csv(OwnerCsvPreviewRequest {
            source_account_id: "bank-main".into(),
            statement_text: "Date,Description,Reference,TransactionId,Amount\n2026-09-10,Adobe,INV-1,T-1,-25.00\n".into(),
            profile: profile(),
        })
        .expect("valid bank preview");
        assert!(preview.can_confirm);
        assert!(preview.requires_explicit_confirmation);
        assert_eq!(preview.lines.len(), 1);
        assert_eq!(preview.lines[0].signed_amount_pence, -2_500);
        assert_eq!(preview.lines[0].source_format, "csv");
        assert!(preview.lines[0].has_strong_source_identity);
    }

    #[test]
    fn match_review_uses_core_ranking_and_keeps_confirmation_explicit() {
        let bank_line = preview_csv(&OwnerCsvPreviewRequest {
            source_account_id: "bank-main".into(),
            statement_text: "Date,Description,Reference,TransactionId,Amount\n2026-09-10,Adobe Creative Cloud,,T-1,-25.00\n".into(),
            profile: profile(),
        })
        .unwrap()
        .lines()[0]
            .clone();
        let review = match_review_from_transactions(
            &bank_line,
            &[transaction(7, "2026-09-10", "Adobe Creative Cloud", "credit", 2_500, "uncleared")],
        )
        .expect("match review");
        assert_eq!(review.candidates.len(), 1);
        assert_eq!(review.candidates[0].transaction_id, 7);
        assert_eq!(review.candidates[0].level, "likely");
        assert!(review.requires_explicit_confirmation);
        assert_eq!(review.recommended_transaction_id, Some(7));
    }

    #[test]
    fn reconciliation_preview_preserves_exact_zero_and_cleared_rule() {
        let transactions = vec![
            transaction(1, "2026-09-10", "Receipt", "debit", 10_000, "cleared"),
            transaction(2, "2026-09-10", "Payment", "credit", 2_500, "cleared"),
        ];
        let preview = reconciliation_preview_from_transactions(50_000, 57_500, &transactions)
            .expect("reconciliation preview");
        assert_eq!(preview.difference_pence, 0);
        assert!(preview.all_entries_cleared);
        assert!(preview.can_finalise);
        assert!(preview.requires_explicit_confirmation);
    }

    #[test]
    fn reconciliation_preview_never_treats_uncleared_as_finalisable() {
        let preview = reconciliation_preview_from_transactions(
            50_000,
            60_000,
            &[transaction(1, "2026-09-10", "Receipt", "debit", 10_000, "uncleared")],
        )
        .expect("preview still visible");
        assert_eq!(preview.difference_pence, 0);
        assert!(!preview.all_entries_cleared);
        assert!(!preview.can_finalise);
        assert!(!preview.requires_explicit_confirmation);
    }

    #[test]
    fn unsafe_or_unknown_request_fields_are_rejected() {
        let unsafe_request = serde_json::json!({
            "sourceAccountId": "bank-main",
            "statementText": "x",
            "profile": {
                "id": "p",
                "name": "n",
                "delimiter": ",",
                "dateHeader": "Date",
                "descriptionHeader": "Description",
                "amountMapping": {"kind":"signed","amount_header":"Amount"},
                "dateFormat": "isoYmd"
            },
            "databasePath": "C:\\outside.sqlite"
        });
        assert!(serde_json::from_value::<OwnerCsvPreviewRequest>(unsafe_request).is_err());
    }

    #[test]
    fn candidate_lists_are_bounded_unique_positive_ids() {
        assert!(require_transaction_ids(&[1, 2, 3], 3, "ids").is_ok());
        assert!(require_transaction_ids(&[], 3, "ids").is_err());
        assert!(require_transaction_ids(&[1, 1], 3, "ids").is_err());
        assert!(require_transaction_ids(&[0], 3, "ids").is_err());
        assert!(require_transaction_ids(&[1, 2, 3, 4], 3, "ids").is_err());
    }
}