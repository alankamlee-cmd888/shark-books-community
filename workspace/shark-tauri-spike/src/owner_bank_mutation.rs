//! SBC-7B1 Slice 2B bounded owner-facing Bank mutation bridge.
//!
//! Every mutation re-resolves authoritative state at execution time. Statement
//! confirmation reparses the statement, matching re-runs the frozen matcher,
//! and reconciliation re-runs the exact-zero/all-cleared contract. The webview
//! never receives raw SQLite/Beankeeper handles or arbitrary filesystem paths.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{
    BankActivityPersistKind, BankActivityView, BankActivityWrite, BankMatchPersistOutcome,
    BankMatchWrite, BankReconciliationEntryWrite, BankReconciliationPersistOutcome,
    BankReconciliationWrite, Books, FoundationError, TransactionView,
};

use super::{open_books_impl, OpenBooksRequest};

const OWNER_BANK_MUTATION_VERSION: u32 = 1;
const BANK_ACCOUNT_CODE: &str = "1000";
const MAX_STATEMENT_BYTES: usize = 10 * 1024 * 1024;
const MAX_IMPORT_LINES: usize = 10_000;
const MAX_MATCH_CANDIDATES: usize = 100;
const MAX_RECONCILIATION_ROWS: usize = 1_000;
const MAX_ACTIVITY_PAGE: i64 = 200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankMutationError {
    code: &'static str,
    message: String,
}

impl OwnerBankMutationError {
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

type OwnerBankMutationResult<T> = Result<T, OwnerBankMutationError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerMutationBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerMutationBooksRef {
    fn open(&self) -> OwnerBankMutationResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerBankMutationError::foundation)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerMutationCsvDateFormat {
    IsoYmd,
    DmySlash,
}

impl From<OwnerMutationCsvDateFormat> for core::bank_import::CsvDateFormat {
    fn from(value: OwnerMutationCsvDateFormat) -> Self {
        match value {
            OwnerMutationCsvDateFormat::IsoYmd => Self::IsoYmd,
            OwnerMutationCsvDateFormat::DmySlash => Self::DmySlash,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum OwnerMutationCsvAmountMapping {
    Signed { amount_header: String },
    DebitCredit {
        debit_header: String,
        credit_header: String,
    },
}

impl OwnerMutationCsvAmountMapping {
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
pub(crate) struct OwnerMutationCsvProfile {
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
    amount_mapping: OwnerMutationCsvAmountMapping,
    date_format: OwnerMutationCsvDateFormat,
}

impl OwnerMutationCsvProfile {
    fn to_core(&self) -> OwnerBankMutationResult<core::bank_import::CsvMappingProfile> {
        let mut chars = self.delimiter.chars();
        let delimiter = chars.next().ok_or_else(|| {
            OwnerBankMutationError::invalid("CSV delimiter must contain one character")
        })?;
        if chars.next().is_some() {
            return Err(OwnerBankMutationError::invalid(
                "CSV delimiter must contain exactly one character",
            ));
        }
        core::bank_import::CsvMappingProfile::new(
            core::RecordId::new(self.id.clone())
                .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))?,
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
        .map_err(OwnerBankMutationError::bank)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerMutationOfxFormat {
    Ofx,
    Qfx,
}

impl From<OwnerMutationOfxFormat> for core::bank_import::BankSourceFormat {
    fn from(value: OwnerMutationOfxFormat) -> Self {
        match value {
            OwnerMutationOfxFormat::Ofx => Self::Ofx,
            OwnerMutationOfxFormat::Qfx => Self::Qfx,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerConfirmedBankLine {
    line_ref: String,
    posted_date: String,
    value_date: Option<String>,
    signed_amount_pence: i64,
    currency: String,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    source_format: String,
    has_strong_source_identity: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCsvImportConfirmRequest {
    books: OwnerMutationBooksRef,
    source_account_id: String,
    statement_text: String,
    profile: OwnerMutationCsvProfile,
    confirmed_statement_sha256: String,
    confirmed_lines: Vec<OwnerConfirmedBankLine>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerOfxImportConfirmRequest {
    books: OwnerMutationBooksRef,
    source_account_id: String,
    statement_text: String,
    format: OwnerMutationOfxFormat,
    confirmed_statement_sha256: String,
    confirmed_lines: Vec<OwnerConfirmedBankLine>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerImportLineReceipt {
    line_ref: String,
    bank_activity_id: i64,
    outcome: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankImportReceipt {
    bridge_version: u32,
    created_count: usize,
    duplicate_count: usize,
    requires_further_automatic_action: bool,
    lines: Vec<OwnerImportLineReceipt>,
}

fn require_statement_text(text: &str) -> OwnerBankMutationResult<()> {
    if text.trim().is_empty() {
        return Err(OwnerBankMutationError::invalid(
            "statement text must not be empty",
        ));
    }
    if text.len() > MAX_STATEMENT_BYTES {
        return Err(OwnerBankMutationError::invalid(
            "statement text exceeds the bounded 10 MiB limit",
        ));
    }
    Ok(())
}

fn source_account(value: &str) -> OwnerBankMutationResult<core::RecordId> {
    core::RecordId::new(value.to_string())
        .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))
}

fn source_format_name(value: core::bank_import::BankSourceFormat) -> &'static str {
    match value {
        core::bank_import::BankSourceFormat::Csv => "csv",
        core::bank_import::BankSourceFormat::Ofx => "ofx",
        core::bank_import::BankSourceFormat::Qfx => "qfx",
    }
}

fn source_kind_name(value: core::SourceKind) -> &'static str {
    match value {
        core::SourceKind::Manual => "manual",
        core::SourceKind::Csv => "csv",
        core::SourceKind::Ofx => "ofx",
        core::SourceKind::Qfx => "qfx",
        core::SourceKind::Document => "document",
        core::SourceKind::Adapter => "adapter",
    }
}

fn confirmed_line(line: &core::bank_import::BankLine) -> OwnerConfirmedBankLine {
    OwnerConfirmedBankLine {
        line_ref: line.source_locator().to_string(),
        posted_date: line.posted_date().iso(),
        value_date: line.value_date().map(core::Date::iso),
        signed_amount_pence: line.signed_amount_minor(),
        currency: line.currency_code().to_string(),
        description: line.description().to_string(),
        payee: line.payee().map(str::to_string),
        reference: line.reference().map(str::to_string),
        source_format: source_format_name(line.source_format()).to_string(),
        has_strong_source_identity: line.strong_identity_key().is_some(),
    }
}

fn bank_activity_write(line: &core::bank_import::BankLine) -> BankActivityWrite {
    BankActivityWrite {
        source_account_id: line.source_account_id().as_str().to_string(),
        institution_account_id: line.institution_account_id().map(str::to_string),
        source_format: source_format_name(line.source_format()).to_string(),
        source_file_sha256: line.source_file_sha256().to_string(),
        source_locator: line.source_locator().to_string(),
        posted_date: line.posted_date().iso(),
        value_date: line.value_date().map(core::Date::iso),
        signed_amount_minor: line.signed_amount_minor(),
        currency_code: line.currency_code().to_string(),
        description: line.description().to_string(),
        payee: line.payee().map(str::to_string),
        reference: line.reference().map(str::to_string),
        external_transaction_id: line.external_transaction_id().map(str::to_string),
        raw_record_sha256: line.raw_record_sha256().to_string(),
        strong_identity_key: line.strong_identity_key(),
        provenance_kind: source_kind_name(line.provenance().kind()).to_string(),
        provenance_source_reference: line.provenance().source_reference().map(str::to_string),
        provenance_fingerprint: line.provenance().fingerprint().map(str::to_string),
        provenance_label: line.provenance().label().map(str::to_string),
    }
}

fn confirm_current_preview(
    statement_text: &str,
    confirmed_statement_sha256: &str,
    confirmed_lines: &[OwnerConfirmedBankLine],
    preview: &core::bank_import::BankImportPreview,
) -> OwnerBankMutationResult<Vec<BankActivityWrite>> {
    let current_statement_sha256 = core::bank_import::sha256_hex(statement_text.as_bytes());
    if confirmed_statement_sha256 != current_statement_sha256 {
        return Err(OwnerBankMutationError::invalid(
            "statement content changed after preview; preview again before confirming",
        ));
    }
    if !preview.can_commit() || preview.lines().is_empty() || !preview.errors().is_empty() {
        return Err(OwnerBankMutationError::invalid(
            "current bank preview is not eligible for confirmation",
        ));
    }
    if preview.lines().len() > MAX_IMPORT_LINES {
        return Err(OwnerBankMutationError::invalid(
            "bank import confirmation exceeds the 10000-line bound",
        ));
    }
    let current_lines: Vec<_> = preview.lines().iter().map(confirmed_line).collect();
    if current_lines != confirmed_lines {
        return Err(OwnerBankMutationError::invalid(
            "bank preview changed after review; preview again before confirming",
        ));
    }
    Ok(preview.lines().iter().map(bank_activity_write).collect())
}

fn receipt_from_outcomes(
    outcomes: Vec<shark_foundation::BankActivityPersistOutcome>,
) -> OwnerBankImportReceipt {
    let mut created_count = 0usize;
    let lines = outcomes
        .into_iter()
        .map(|outcome| {
            let label = match outcome.kind {
                BankActivityPersistKind::Created => {
                    created_count += 1;
                    "created"
                }
                BankActivityPersistKind::StrongDuplicate => "strongDuplicate",
                BankActivityPersistKind::FileExactDuplicate => "fileExactDuplicate",
            };
            OwnerImportLineReceipt {
                line_ref: outcome.source_locator,
                bank_activity_id: outcome.activity_id,
                outcome: label,
            }
        })
        .collect::<Vec<_>>();
    OwnerBankImportReceipt {
        bridge_version: OWNER_BANK_MUTATION_VERSION,
        created_count,
        duplicate_count: lines.len() - created_count,
        requires_further_automatic_action: false,
        lines,
    }
}

#[tauri::command]
pub(crate) fn owner_bank_import_confirm_csv(
    request: OwnerCsvImportConfirmRequest,
) -> OwnerBankMutationResult<OwnerBankImportReceipt> {
    require_statement_text(&request.statement_text)?;
    let profile = request.profile.to_core()?;
    let preview = core::bank_import::preview_csv(
        &request.statement_text,
        source_account(&request.source_account_id)?,
        &profile,
    )
    .map_err(OwnerBankMutationError::bank)?;
    let writes = confirm_current_preview(
        &request.statement_text,
        &request.confirmed_statement_sha256,
        &request.confirmed_lines,
        &preview,
    )?;
    let books = request.books.open()?;
    let outcomes = books
        .persist_bank_activity_batch(&writes)
        .map_err(OwnerBankMutationError::foundation)?;
    Ok(receipt_from_outcomes(outcomes))
}

#[tauri::command]
pub(crate) fn owner_bank_import_confirm_ofx_qfx(
    request: OwnerOfxImportConfirmRequest,
) -> OwnerBankMutationResult<OwnerBankImportReceipt> {
    require_statement_text(&request.statement_text)?;
    let preview = core::bank_import::preview_ofx_or_qfx(
        &request.statement_text,
        source_account(&request.source_account_id)?,
        request.format.into(),
    )
    .map_err(OwnerBankMutationError::bank)?;
    let writes = confirm_current_preview(
        &request.statement_text,
        &request.confirmed_statement_sha256,
        &request.confirmed_lines,
        &preview,
    )?;
    let books = request.books.open()?;
    let outcomes = books
        .persist_bank_activity_batch(&writes)
        .map_err(OwnerBankMutationError::foundation)?;
    Ok(receipt_from_outcomes(outcomes))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBankActivityListRequest {
    books: OwnerMutationBooksRef,
    limit: i64,
    offset: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankActivityRow {
    bank_activity_id: i64,
    posted_date: String,
    signed_amount_pence: i64,
    currency: String,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    imported_at: String,
    matched_transaction_id: Option<i64>,
    match_level: Option<String>,
    clearance_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankActivityPage {
    bridge_version: u32,
    rows: Vec<OwnerBankActivityRow>,
}

fn activity_row(value: BankActivityView) -> OwnerBankActivityRow {
    OwnerBankActivityRow {
        bank_activity_id: value.id,
        posted_date: value.activity.posted_date,
        signed_amount_pence: value.activity.signed_amount_minor,
        currency: value.activity.currency_code,
        description: value.activity.description,
        payee: value.activity.payee,
        reference: value.activity.reference,
        imported_at: value.imported_at,
        matched_transaction_id: value.matched_transaction_id,
        match_level: value.match_level,
        clearance_state: value.clearance_state,
    }
}

#[tauri::command]
pub(crate) fn owner_bank_activity_list(
    request: OwnerBankActivityListRequest,
) -> OwnerBankMutationResult<OwnerBankActivityPage> {
    if !(1..=MAX_ACTIVITY_PAGE).contains(&request.limit) {
        return Err(OwnerBankMutationError::invalid(
            "bank activity page limit must be between 1 and 200",
        ));
    }
    if request.offset < 0 || request.offset > 1_000_000 {
        return Err(OwnerBankMutationError::invalid(
            "bank activity page offset is outside the supported range",
        ));
    }
    let books = request.books.open()?;
    let rows = books
        .list_bank_activity(request.limit, request.offset)
        .map_err(OwnerBankMutationError::foundation)?
        .into_iter()
        .map(activity_row)
        .collect();
    Ok(OwnerBankActivityPage {
        bridge_version: OWNER_BANK_MUTATION_VERSION,
        rows,
    })
}

fn parse_date(value: &str) -> OwnerBankMutationResult<core::Date> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .ok_or_else(|| OwnerBankMutationError::invalid("date must be YYYY-MM-DD"))?;
    let month = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerBankMutationError::invalid("date must be YYYY-MM-DD"))?;
    let day = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerBankMutationError::invalid("date must be YYYY-MM-DD"))?;
    if parts.next().is_some() || value.len() != 10 {
        return Err(OwnerBankMutationError::invalid(
            "date must be YYYY-MM-DD",
        ));
    }
    core::Date::new(year, month, day)
        .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))
}

fn source_kind(value: &str) -> OwnerBankMutationResult<core::SourceKind> {
    match value {
        "csv" => Ok(core::SourceKind::Csv),
        "ofx" => Ok(core::SourceKind::Ofx),
        "qfx" => Ok(core::SourceKind::Qfx),
        _ => Err(OwnerBankMutationError::invalid(
            "persisted bank activity has an unsupported source kind",
        )),
    }
}

fn source_format(value: &str) -> OwnerBankMutationResult<core::bank_import::BankSourceFormat> {
    match value {
        "csv" => Ok(core::bank_import::BankSourceFormat::Csv),
        "ofx" => Ok(core::bank_import::BankSourceFormat::Ofx),
        "qfx" => Ok(core::bank_import::BankSourceFormat::Qfx),
        _ => Err(OwnerBankMutationError::invalid(
            "persisted bank activity has an unsupported source format",
        )),
    }
}

fn bank_line_from_persisted(
    persisted: &BankActivityView,
) -> OwnerBankMutationResult<core::bank_import::BankLine> {
    let activity = &persisted.activity;
    let provenance = core::SourceProvenance::new(
        source_kind(&activity.provenance_kind)?,
        activity.provenance_source_reference.clone(),
        activity.provenance_fingerprint.clone(),
        activity.provenance_label.clone(),
    )
    .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))?;
    let line = core::bank_import::BankLine::new(
        source_account(&activity.source_account_id)?,
        activity.institution_account_id.clone(),
        source_format(&activity.source_format)?,
        activity.source_file_sha256.clone(),
        activity.source_locator.clone(),
        parse_date(&activity.posted_date)?,
        activity
            .value_date
            .as_deref()
            .map(parse_date)
            .transpose()?,
        activity.signed_amount_minor,
        activity.description.clone(),
        activity.payee.clone(),
        activity.reference.clone(),
        activity.external_transaction_id.clone(),
        activity.raw_record_sha256.clone(),
        provenance,
    )
    .map_err(OwnerBankMutationError::bank)?;
    if line.strong_identity_key() != activity.strong_identity_key {
        return Err(OwnerBankMutationError::invalid(
            "persisted bank activity source identity failed integrity reconstruction",
        ));
    }
    Ok(line)
}

#[derive(Debug, Clone, Copy)]
struct ResolvedBankEntry {
    entry_id: i64,
    signed_amount_pence: i64,
    state: core::matching::ClearanceState,
}

fn clearance_state(status: &str) -> OwnerBankMutationResult<core::matching::ClearanceState> {
    match status {
        "uncleared" => Ok(core::matching::ClearanceState::Uncleared),
        "cleared" => Ok(core::matching::ClearanceState::Cleared),
        "reconciled" => Ok(core::matching::ClearanceState::Reconciled),
        other => Err(OwnerBankMutationError::invalid(format!(
            "unsupported Bank-entry clearance state '{other}'"
        ))),
    }
}

fn resolved_bank_entry(
    transaction: &TransactionView,
) -> OwnerBankMutationResult<ResolvedBankEntry> {
    let mut matching = transaction
        .entries
        .iter()
        .filter(|entry| entry.account_code == BANK_ACCOUNT_CODE);
    let entry = matching.next().ok_or_else(|| {
        OwnerBankMutationError::invalid("candidate transaction has no Business Bank entry")
    })?;
    if matching.next().is_some() {
        return Err(OwnerBankMutationError::invalid(
            "candidate transaction has more than one Business Bank entry",
        ));
    }
    let signed_amount_pence = match entry.direction.as_str() {
        "debit" => entry.amount_minor,
        "credit" => entry
            .amount_minor
            .checked_neg()
            .ok_or_else(|| OwnerBankMutationError::invalid("Bank-entry amount overflow"))?,
        other => {
            return Err(OwnerBankMutationError::invalid(format!(
                "unsupported Bank-entry direction '{other}'"
            )))
        }
    };
    Ok(ResolvedBankEntry {
        entry_id: entry.id,
        signed_amount_pence,
        state: clearance_state(&entry.status)?,
    })
}

fn ledger_candidate(
    transaction: &TransactionView,
    source_account_id: core::RecordId,
) -> OwnerBankMutationResult<core::matching::LedgerCandidate> {
    let bank = resolved_bank_entry(transaction)?;
    core::matching::LedgerCandidate::new(
        core::RecordId::new(format!("transaction-{}", transaction.id))
            .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))?,
        source_account_id,
        parse_date(&transaction.date)?,
        bank.signed_amount_pence,
        transaction.description.clone(),
        None,
        transaction.reference.clone(),
        None,
        bank.state,
    )
    .map_err(OwnerBankMutationError::matching)
}

fn require_transaction_ids(
    ids: &[i64],
    maximum: usize,
    label: &str,
) -> OwnerBankMutationResult<()> {
    if ids.is_empty() {
        return Err(OwnerBankMutationError::invalid(format!(
            "{label} must not be empty"
        )));
    }
    if ids.len() > maximum {
        return Err(OwnerBankMutationError::invalid(format!(
            "{label} exceeds the bounded item limit"
        )));
    }
    if ids.iter().any(|id| *id <= 0) {
        return Err(OwnerBankMutationError::invalid(format!(
            "{label} contains an invalid transaction id"
        )));
    }
    let unique: HashSet<i64> = ids.iter().copied().collect();
    if unique.len() != ids.len() {
        return Err(OwnerBankMutationError::invalid(format!(
            "{label} contains duplicate transaction ids"
        )));
    }
    Ok(())
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBankMatchConfirmRequest {
    books: OwnerMutationBooksRef,
    bank_activity_id: i64,
    candidate_transaction_ids: Vec<i64>,
    selected_transaction_id: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankMatchReceipt {
    bridge_version: u32,
    bank_activity_id: i64,
    transaction_id: i64,
    bank_entry_id: i64,
    match_level: String,
    score: u16,
    reasons: Vec<String>,
    already_confirmed: bool,
    clearance_state: &'static str,
}

#[tauri::command]
pub(crate) fn owner_bank_match_confirm(
    request: OwnerBankMatchConfirmRequest,
) -> OwnerBankMutationResult<OwnerBankMatchReceipt> {
    require_transaction_ids(
        &request.candidate_transaction_ids,
        MAX_MATCH_CANDIDATES,
        "candidate transaction ids",
    )?;
    if !request
        .candidate_transaction_ids
        .contains(&request.selected_transaction_id)
    {
        return Err(OwnerBankMutationError::invalid(
            "selected transaction must be present in the reviewed candidate set",
        ));
    }

    let books = request.books.open()?;
    let persisted = books
        .bank_activity(request.bank_activity_id)
        .map_err(OwnerBankMutationError::foundation)?;
    if let Some(existing) = books
        .bank_match_for_activity(request.bank_activity_id)
        .map_err(OwnerBankMutationError::foundation)?
    {
        if existing.transaction_id != request.selected_transaction_id {
            return Err(OwnerBankMutationError::invalid(
                "bank activity already has a different confirmed match",
            ));
        }
        return Ok(OwnerBankMatchReceipt {
            bridge_version: OWNER_BANK_MUTATION_VERSION,
            bank_activity_id: existing.bank_activity_id,
            transaction_id: existing.transaction_id,
            bank_entry_id: existing.entry_id,
            match_level: existing.match_level,
            score: existing.score,
            reasons: existing.reasons,
            already_confirmed: true,
            clearance_state: persisted
                .clearance_state
                .as_deref()
                .unwrap_or("cleared"),
        });
    }

    let bank_line = bank_line_from_persisted(&persisted)?;
    let transactions = request
        .candidate_transaction_ids
        .iter()
        .map(|id| books.transaction(*id).map_err(OwnerBankMutationError::foundation))
        .collect::<OwnerBankMutationResult<Vec<_>>>()?;

    let mut candidate_by_transaction = HashMap::<i64, core::matching::LedgerCandidate>::new();
    let mut transaction_by_record = HashMap::<String, i64>::new();
    for transaction in &transactions {
        let candidate = ledger_candidate(transaction, bank_line.source_account_id().clone())?;
        transaction_by_record.insert(candidate.id().as_str().to_string(), transaction.id);
        candidate_by_transaction.insert(transaction.id, candidate);
    }
    let candidate_values: Vec<_> = request
        .candidate_transaction_ids
        .iter()
        .map(|id| {
            candidate_by_transaction
                .get(id)
                .expect("validated candidate map")
                .clone()
        })
        .collect();
    let ranked = core::matching::rank_matches(&bank_line, &candidate_values);
    if ranked.ambiguous_top() {
        return Err(OwnerBankMutationError::invalid(
            "match review is ambiguous; review the candidate set again",
        ));
    }

    let selected_candidate = candidate_by_transaction
        .get(&request.selected_transaction_id)
        .ok_or_else(|| OwnerBankMutationError::invalid("selected match candidate disappeared"))?;
    let assessment = ranked
        .candidates()
        .iter()
        .find(|assessment| {
            transaction_by_record
                .get(assessment.candidate_id().as_str())
                .is_some_and(|id| *id == request.selected_transaction_id)
        })
        .ok_or_else(|| OwnerBankMutationError::invalid("selected match assessment disappeared"))?;
    if assessment.level() == core::matching::MatchLevel::Unmatched {
        return Err(OwnerBankMutationError::invalid(
            "an unmatched candidate cannot be confirmed",
        ));
    }
    core::matching::confirm_match(
        selected_candidate,
        assessment,
        request.books.actor.clone(),
    )
    .map_err(OwnerBankMutationError::matching)?;

    let selected_transaction = transactions
        .iter()
        .find(|transaction| transaction.id == request.selected_transaction_id)
        .ok_or_else(|| OwnerBankMutationError::invalid("selected transaction disappeared"))?;
    let bank_entry = resolved_bank_entry(selected_transaction)?;
    let write = BankMatchWrite {
        bank_activity_id: request.bank_activity_id,
        transaction_id: request.selected_transaction_id,
        entry_id: bank_entry.entry_id,
        match_level: match_level_name(assessment.level()).to_string(),
        score: assessment.score(),
        reasons: assessment
            .reasons()
            .iter()
            .copied()
            .map(match_reason_name)
            .map(str::to_string)
            .collect(),
    };
    let outcome = books
        .confirm_bank_match(&write)
        .map_err(OwnerBankMutationError::foundation)?;
    let (view, already_confirmed) = match outcome {
        BankMatchPersistOutcome::Confirmed(view) => (view, false),
        BankMatchPersistOutcome::AlreadyConfirmed(view) => (view, true),
    };
    Ok(OwnerBankMatchReceipt {
        bridge_version: OWNER_BANK_MUTATION_VERSION,
        bank_activity_id: view.bank_activity_id,
        transaction_id: view.transaction_id,
        bank_entry_id: view.entry_id,
        match_level: view.match_level,
        score: view.score,
        reasons: view.reasons,
        already_confirmed,
        clearance_state: "cleared",
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBankReconcileFinaliseRequest {
    books: OwnerMutationBooksRef,
    statement_id: String,
    statement_date: String,
    opening_balance_pence: i64,
    ending_balance_pence: i64,
    transaction_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerBankReconciliationReceipt {
    bridge_version: u32,
    statement_id: String,
    statement_date: String,
    opening_balance_pence: i64,
    ending_balance_pence: i64,
    entry_count: usize,
    already_finalised: bool,
    clearance_state: &'static str,
}

fn reconciliation_write_from_transactions(
    statement_id: &str,
    statement_date: &str,
    opening_balance_pence: i64,
    ending_balance_pence: i64,
    transactions: &[TransactionView],
) -> OwnerBankMutationResult<BankReconciliationWrite> {
    let mut entries = Vec::with_capacity(transactions.len());
    for transaction in transactions {
        let bank = resolved_bank_entry(transaction)?;
        entries.push(BankReconciliationEntryWrite {
            transaction_id: transaction.id,
            entry_id: bank.entry_id,
            signed_amount_minor: bank.signed_amount_pence,
        });
    }
    Ok(BankReconciliationWrite {
        statement_id: statement_id.to_string(),
        statement_date: statement_date.to_string(),
        opening_balance_minor: opening_balance_pence,
        ending_balance_minor: ending_balance_pence,
        entries,
    })
}

fn reconciliation_membership_equal(
    left: &[BankReconciliationEntryWrite],
    right: &[BankReconciliationEntryWrite],
) -> bool {
    let mut left_rows: Vec<_> = left
        .iter()
        .map(|entry| (entry.transaction_id, entry.entry_id, entry.signed_amount_minor))
        .collect();
    let mut right_rows: Vec<_> = right
        .iter()
        .map(|entry| (entry.transaction_id, entry.entry_id, entry.signed_amount_minor))
        .collect();
    left_rows.sort_unstable();
    right_rows.sort_unstable();
    left_rows == right_rows
}

#[tauri::command]
pub(crate) fn owner_bank_reconcile_finalise(
    request: OwnerBankReconcileFinaliseRequest,
) -> OwnerBankMutationResult<OwnerBankReconciliationReceipt> {
    require_transaction_ids(
        &request.transaction_ids,
        MAX_RECONCILIATION_ROWS,
        "reconciliation transaction ids",
    )?;
    let statement_record_id = core::RecordId::new(request.statement_id.clone())
        .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))?;
    let statement_date = parse_date(&request.statement_date)?;
    let books = request.books.open()?;
    let transactions = request
        .transaction_ids
        .iter()
        .map(|id| books.transaction(*id).map_err(OwnerBankMutationError::foundation))
        .collect::<OwnerBankMutationResult<Vec<_>>>()?;
    let write = reconciliation_write_from_transactions(
        &request.statement_id,
        &request.statement_date,
        request.opening_balance_pence,
        request.ending_balance_pence,
        &transactions,
    )?;

    if let Some(existing) = books
        .bank_reconciliation(&request.statement_id)
        .map_err(OwnerBankMutationError::foundation)?
    {
        if existing.reconciliation.statement_date != request.statement_date
            || existing.reconciliation.opening_balance_minor != request.opening_balance_pence
            || existing.reconciliation.ending_balance_minor != request.ending_balance_pence
            || !reconciliation_membership_equal(&existing.entries, &write.entries)
        {
            return Err(OwnerBankMutationError::invalid(
                "statement id is already finalised with different reconciliation content",
            ));
        }
        return Ok(OwnerBankReconciliationReceipt {
            bridge_version: OWNER_BANK_MUTATION_VERSION,
            statement_id: existing.reconciliation.statement_id,
            statement_date: existing.reconciliation.statement_date,
            opening_balance_pence: existing.reconciliation.opening_balance_minor,
            ending_balance_pence: existing.reconciliation.ending_balance_minor,
            entry_count: existing.reconciliation.entry_count,
            already_finalised: true,
            clearance_state: "reconciled",
        });
    }

    let mut core_entries = Vec::with_capacity(transactions.len());
    for (transaction, write_entry) in transactions.iter().zip(write.entries.iter()) {
        let bank = resolved_bank_entry(transaction)?;
        if bank.state != core::matching::ClearanceState::Cleared {
            return Err(OwnerBankMutationError::invalid(
                "reconciliation finalisation requires every Business Bank entry to still be cleared",
            ));
        }
        if books
            .bank_match_for_entry(transaction.id, bank.entry_id)
            .map_err(OwnerBankMutationError::foundation)?
            .is_none()
        {
            return Err(OwnerBankMutationError::invalid(
                "reconciliation entry has no owner-confirmed bank match",
            ));
        }
        let record_id = core::RecordId::new(format!(
            "transaction-{}-entry-{}",
            transaction.id, write_entry.entry_id
        ))
        .map_err(|error| OwnerBankMutationError::invalid(error.to_string()))?;
        core_entries.push(
            core::matching::ReconciliationEntry::new(
                record_id,
                bank.signed_amount_pence,
                bank.state,
            )
            .map_err(OwnerBankMutationError::matching)?,
        );
    }
    core::matching::finalize_reconciliation(
        statement_record_id,
        statement_date,
        request.opening_balance_pence,
        request.ending_balance_pence,
        &core_entries,
        request.books.actor.clone(),
    )
    .map_err(OwnerBankMutationError::matching)?;

    let outcome = books
        .finalize_bank_reconciliation(&write)
        .map_err(OwnerBankMutationError::foundation)?;
    let (view, already_finalised) = match outcome {
        BankReconciliationPersistOutcome::Finalized(view) => (view, false),
        BankReconciliationPersistOutcome::AlreadyFinalized(view) => (view, true),
    };
    Ok(OwnerBankReconciliationReceipt {
        bridge_version: OWNER_BANK_MUTATION_VERSION,
        statement_id: view.statement_id,
        statement_date: view.statement_date,
        opening_balance_pence: view.opening_balance_minor,
        ending_balance_pence: view.ending_balance_minor,
        entry_count: view.entry_count,
        already_finalised,
        clearance_state: "reconciled",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> OwnerMutationCsvProfile {
        OwnerMutationCsvProfile {
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
            amount_mapping: OwnerMutationCsvAmountMapping::Signed {
                amount_header: "Amount".into(),
            },
            date_format: OwnerMutationCsvDateFormat::IsoYmd,
        }
    }

    fn csv_text() -> String {
        "Date,Description,Reference,TransactionId,Amount\n2026-09-11,Adobe,INV-1,T-1,-25.00\n"
            .to_string()
    }

    fn csv_preview(text: &str) -> core::bank_import::BankImportPreview {
        core::bank_import::preview_csv(
            text,
            core::RecordId::new("bank-main").unwrap(),
            &profile().to_core().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn import_confirmation_is_bound_to_current_statement_and_preview_echo() {
        let text = csv_text();
        let preview = csv_preview(&text);
        let echoes: Vec<_> = preview.lines().iter().map(confirmed_line).collect();
        let hash = core::bank_import::sha256_hex(text.as_bytes());
        let writes = confirm_current_preview(&text, &hash, &echoes, &preview)
            .expect("current preview confirms");
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].signed_amount_minor, -2_500);
        assert_eq!(writes[0].source_format, "csv");
        assert!(writes[0].strong_identity_key.is_some());

        assert!(confirm_current_preview(&text, &"0".repeat(64), &echoes, &preview).is_err());
        let mut stale_echoes = echoes.clone();
        stale_echoes[0].signed_amount_pence = -2_499;
        assert!(confirm_current_preview(&text, &hash, &stale_echoes, &preview).is_err());
    }

    #[test]
    fn persisted_bank_line_reconstruction_preserves_source_identity() {
        let text = csv_text();
        let preview = csv_preview(&text);
        let line = &preview.lines()[0];
        let write = bank_activity_write(line);
        let persisted = BankActivityView {
            id: 1,
            activity: write,
            imported_at: "2026-09-11 00:00:00".to_string(),
            imported_by: "local-owner".to_string(),
            matched_transaction_id: None,
            matched_entry_id: None,
            match_level: None,
            match_score: None,
            match_reasons: Vec::new(),
            clearance_state: None,
        };
        let reconstructed = bank_line_from_persisted(&persisted).expect("reconstruct");
        assert_eq!(reconstructed.source_locator(), line.source_locator());
        assert_eq!(reconstructed.signed_amount_minor(), line.signed_amount_minor());
        assert_eq!(reconstructed.strong_identity_key(), line.strong_identity_key());
    }

    #[test]
    fn candidate_lists_are_bounded_unique_positive_ids() {
        assert!(require_transaction_ids(&[1, 2, 3], 3, "ids").is_ok());
        assert!(require_transaction_ids(&[], 3, "ids").is_err());
        assert!(require_transaction_ids(&[1, 1], 3, "ids").is_err());
        assert!(require_transaction_ids(&[0], 3, "ids").is_err());
        assert!(require_transaction_ids(&[1, 2, 3, 4], 3, "ids").is_err());
    }

    #[test]
    fn unsafe_path_secret_and_ledger_shaped_fields_are_rejected() {
        let unsafe_request = serde_json::json!({
            "books": {
                "fileName": "safe.sqlite",
                "booksId": "safe-books",
                "actor": "local-owner"
            },
            "sourceAccountId": "bank-main",
            "statementText": csv_text(),
            "profile": {
                "id": "bank-profile",
                "name": "Test profile",
                "delimiter": ",",
                "dateHeader": "Date",
                "valueDateHeader": null,
                "descriptionHeader": "Description",
                "payeeHeader": null,
                "referenceHeader": "Reference",
                "transactionIdHeader": "TransactionId",
                "currencyHeader": null,
                "amountMapping": {"kind":"signed","amountHeader":"Amount"},
                "dateFormat": "isoYmd"
            },
            "confirmedStatementSha256": "0".repeat(64),
            "confirmedLines": [],
            "databasePath": "C:/outside.sqlite",
            "accountCode": "1000",
            "debit": 2500,
            "passphrase": "secret"
        });
        assert!(serde_json::from_value::<OwnerCsvImportConfirmRequest>(unsafe_request).is_err());
    }

    #[test]
    fn mutation_requests_expose_no_arbitrary_os_path_field() {
        let books = serde_json::json!({
            "fileName": "safe.sqlite",
            "booksId": "safe-books",
            "actor": "local-owner",
            "path": "/tmp/unsafe"
        });
        assert!(serde_json::from_value::<OwnerMutationBooksRef>(books).is_err());
    }
}
