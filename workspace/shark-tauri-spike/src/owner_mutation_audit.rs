//! SBC-7B1 Batch A — bounded owner receipt-decision and correction bridge.
//!
//! This module composes frozen product-core semantics with Shark-owned encrypted
//! persistence. The webview receives only bounded semantic values and opaque
//! suggestion/preview identifiers: no raw paths, database handles, account-code
//! posting authority, shell authority, tax judgement or automatic reconciliation.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{
    BankActivityView, Books, Direction, FoundationError, OwnerCorrectionPersistOutcome,
    OwnerCorrectionWrite, PostTransactionRequest, PostingLine, ReceiptBankDecisionPersistOutcome,
    ReceiptBankDecisionWrite, TransactionView,
};

use super::ocr_native::{NativeOcrRegistry, ShellOcrOutcome};
use super::owner_documents_ocr::NativeDocumentRootRegistry;
use super::{open_books_impl, OpenBooksRequest};

const OWNER_MUTATION_AUDIT_VERSION: u32 = 1;
const MAX_RECEIPT_SUGGESTIONS: usize = 32;
const MAX_BANK_ACTIVITY_CANDIDATES: i64 = 500;
const MAX_CORRECTION_HISTORY: i64 = 100;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerMutationAuditError {
    code: &'static str,
    message: String,
}

impl OwnerMutationAuditError {
    fn invalid(message: impl Into<String>) -> Self {
        Self { code: "invalidInput", message: message.into() }
    }

    fn stale(message: impl Into<String>) -> Self {
        Self { code: "staleReview", message: message.into() }
    }

    fn document(message: impl Into<String>) -> Self {
        Self { code: "documentReviewFailed", message: message.into() }
    }

    fn foundation(error: FoundationError) -> Self {
        Self { code: "booksOperationFailed", message: error.to_string() }
    }

    fn domain(error: impl std::fmt::Display) -> Self {
        Self { code: "invalidInput", message: error.to_string() }
    }
}

type OwnerMutationAuditResult<T> = Result<T, OwnerMutationAuditError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerMutationBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerMutationBooksRef {
    fn open(&self) -> OwnerMutationAuditResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerMutationAuditError::foundation)
    }
}

fn bounded_id(value: &str, label: &str) -> OwnerMutationAuditResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || trimmed.chars().any(|c| c.is_control() || c == '\0')
    {
        return Err(OwnerMutationAuditError::invalid(format!(
            "{label} must be 1-128 bounded characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn parse_date(value: &str) -> OwnerMutationAuditResult<core::Date> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .ok_or_else(|| OwnerMutationAuditError::invalid("date must be YYYY-MM-DD"))?;
    let month = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerMutationAuditError::invalid("date must be YYYY-MM-DD"))?;
    let day = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerMutationAuditError::invalid("date must be YYYY-MM-DD"))?;
    if parts.next().is_some() || value.len() != 10 {
        return Err(OwnerMutationAuditError::invalid("date must be YYYY-MM-DD"));
    }
    core::Date::new(year, month, day).map_err(OwnerMutationAuditError::domain)
}

// -----------------------------------------------------------------------------
// Receipt suggestion registry and factual OCR conversion
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RegisteredReceiptCandidate {
    token: String,
    identity: core::receipt_bank_suggestion::BankLineIdentity,
    bank_activity_id: i64,
    posted_date: String,
    amount_pence: i64,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
}

#[derive(Debug, Clone)]
struct RegisteredReceiptSuggestion {
    file_name: String,
    books_id: String,
    document_id: String,
    suggestions: core::receipt_bank_suggestion::ReceiptBankSuggestionSet,
    candidates: Vec<RegisteredReceiptCandidate>,
}

#[derive(Default)]
struct ReceiptRegistryInner {
    order: VecDeque<String>,
    entries: HashMap<String, RegisteredReceiptSuggestion>,
}

pub(crate) struct ReceiptSuggestionRegistry {
    counter: AtomicU64,
    inner: Mutex<ReceiptRegistryInner>,
}

impl Default for ReceiptSuggestionRegistry {
    fn default() -> Self {
        Self {
            counter: AtomicU64::new(1),
            inner: Mutex::new(ReceiptRegistryInner::default()),
        }
    }
}

impl ReceiptSuggestionRegistry {
    fn new_suggestion_id(
        &self,
        books: &OwnerMutationBooksRef,
        request_id: &str,
        document_id: &str,
    ) -> OwnerMutationAuditResult<String> {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| OwnerMutationAuditError::stale("system clock is invalid"))?
            .as_nanos();
        let seed = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            books.file_name,
            books.books_id,
            request_id,
            document_id,
            std::process::id(),
            now,
            counter
        );
        Ok(format!(
            "receipt-suggestion-{}",
            core::bank_import::sha256_hex(seed.as_bytes())
        ))
    }

    fn candidate_token(
        suggestion_id: &str,
        identity: &core::receipt_bank_suggestion::BankLineIdentity,
    ) -> String {
        let seed = format!(
            "{}|{}|{}|{}|{}",
            suggestion_id,
            identity.source_account_id().as_str(),
            identity.source_file_sha256(),
            identity.source_locator(),
            identity.raw_record_sha256()
        );
        format!("receipt-candidate-{}", core::bank_import::sha256_hex(seed.as_bytes()))
    }

    fn insert(
        &self,
        suggestion_id: String,
        suggestion: RegisteredReceiptSuggestion,
    ) -> OwnerMutationAuditResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| OwnerMutationAuditError::stale("receipt suggestion registry is unavailable"))?;
        if inner.entries.contains_key(&suggestion_id) {
            return Err(OwnerMutationAuditError::stale(
                "receipt suggestion identity collision",
            ));
        }
        inner.order.push_back(suggestion_id.clone());
        inner.entries.insert(suggestion_id, suggestion);
        while inner.order.len() > MAX_RECEIPT_SUGGESTIONS {
            if let Some(oldest) = inner.order.pop_front() {
                inner.entries.remove(&oldest);
            }
        }
        Ok(())
    }

    fn resolve(&self, suggestion_id: &str) -> OwnerMutationAuditResult<RegisteredReceiptSuggestion> {
        let suggestion_id = bounded_id(suggestion_id, "receipt suggestion id")?;
        self.inner
            .lock()
            .map_err(|_| OwnerMutationAuditError::stale("receipt suggestion registry is unavailable"))?
            .entries
            .get(&suggestion_id)
            .cloned()
            .ok_or_else(|| {
                OwnerMutationAuditError::stale(
                    "receipt suggestion is unknown or expired; generate a new suggestion",
                )
            })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum NativeOcrMirror {
    Completed { extraction: NativeOcrExtractionMirror },
    Unavailable { reason: String },
    Failed { kind: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeOcrExtractionMirror {
    provenance: NativeOcrProvenanceMirror,
    raw_text: String,
    candidates: NativeOcrCandidatesMirror,
    warnings: Vec<NativeOcrWarningMirror>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeOcrProvenanceMirror {
    engine_id: String,
    engine_version: String,
    runtime_id: String,
    model_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct NativeOcrCandidatesMirror {
    merchant_text: Option<NativeTextCandidateMirror>,
    document_date: Option<NativeDateCandidateMirror>,
    total: Option<NativeAmountCandidateMirror>,
    currency: Option<NativeCurrencyCandidateMirror>,
    reference: Option<NativeTextCandidateMirror>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeTextCandidateMirror {
    value: String,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeDateCandidateMirror {
    value: String,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeAmountCandidateMirror {
    total_pence: i64,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeCurrencyCandidateMirror {
    code: String,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeOcrWarningMirror {
    code: String,
    detail: Option<String>,
}

fn confidence(value: Option<u16>) -> OwnerMutationAuditResult<Option<core::ocr::OcrConfidenceBps>> {
    value
        .map(core::ocr::OcrConfidenceBps::new)
        .transpose()
        .map_err(OwnerMutationAuditError::domain)
}

fn warning_code(value: &str) -> OwnerMutationAuditResult<core::ocr::OcrWarningCode> {
    use core::ocr::OcrWarningCode as W;
    match value {
        "no_text_detected" => Ok(W::NoTextDetected),
        "missing_merchant" => Ok(W::MissingMerchant),
        "missing_date" => Ok(W::MissingDate),
        "missing_total" => Ok(W::MissingTotal),
        "missing_currency" => Ok(W::MissingCurrency),
        "missing_reference" => Ok(W::MissingReference),
        "low_confidence" => Ok(W::LowConfidence),
        "unsupported_document" => Ok(W::UnsupportedDocument),
        "truncated_output" => Ok(W::TruncatedOutput),
        "engine_notice" => Ok(W::EngineNotice),
        _ => Err(OwnerMutationAuditError::document(
            "OCR warning code is outside the frozen factual contract",
        )),
    }
}

fn core_extraction_from_native(
    native: NativeOcrExtractionMirror,
    request_id: &str,
    document: &shark_foundation::DocumentView,
) -> OwnerMutationAuditResult<core::ocr::OcrExtraction> {
    let document_id = core::RecordId::new(document.document_id.clone())
        .map_err(OwnerMutationAuditError::domain)?;
    let reference = core::documents::DocumentReference::from_persisted(
        document_id.clone(),
        document_id,
        core::RecordId::new(document.storage_root_id.clone()).map_err(OwnerMutationAuditError::domain)?,
        document.relative_path.clone(),
        document.original_filename.clone(),
        document.media_type.clone(),
        document.sha256.clone(),
        document.byte_len,
    )
    .map_err(OwnerMutationAuditError::domain)?;
    let request = core::ocr::OcrRequest::for_receipt_document(
        core::RecordId::new(request_id.to_string()).map_err(OwnerMutationAuditError::domain)?,
        &reference,
    );
    let provenance = core::ocr::OcrEngineProvenance::new(
        native.provenance.engine_id,
        native.provenance.engine_version,
        native.provenance.runtime_id,
        native.provenance.model_ids,
    )
    .map_err(OwnerMutationAuditError::domain)?;

    let merchant_text = native
        .candidates
        .merchant_text
        .map(|candidate| {
            core::ocr::OcrTextCandidate::new(candidate.value, confidence(candidate.confidence_bps)?)
                .map_err(OwnerMutationAuditError::domain)
        })
        .transpose()?;
    let document_date = native
        .candidates
        .document_date
        .map(|candidate| {
            Ok::<_, OwnerMutationAuditError>(core::ocr::OcrDateCandidate::new(
                parse_date(&candidate.value)?,
                confidence(candidate.confidence_bps)?,
            ))
        })
        .transpose()?;
    let total = native
        .candidates
        .total
        .map(|candidate| {
            core::ocr::OcrAmountCandidate::new(
                candidate.total_pence,
                confidence(candidate.confidence_bps)?,
            )
            .map_err(OwnerMutationAuditError::domain)
        })
        .transpose()?;
    let currency = native
        .candidates
        .currency
        .map(|candidate| {
            core::ocr::OcrCurrencyCandidate::new(
                candidate.code,
                confidence(candidate.confidence_bps)?,
            )
            .map_err(OwnerMutationAuditError::domain)
        })
        .transpose()?;
    let receipt_reference = native
        .candidates
        .reference
        .map(|candidate| {
            core::ocr::OcrTextCandidate::new(candidate.value, confidence(candidate.confidence_bps)?)
                .map_err(OwnerMutationAuditError::domain)
        })
        .transpose()?;
    let candidates = core::ocr::OcrReceiptCandidates::new(
        merchant_text,
        document_date,
        total,
        currency,
        receipt_reference,
    );
    let warnings = native
        .warnings
        .into_iter()
        .map(|warning| {
            core::ocr::OcrWarning::new(warning_code(&warning.code)?, warning.detail)
                .map_err(OwnerMutationAuditError::domain)
        })
        .collect::<Result<Vec<_>, _>>()?;

    core::ocr::OcrExtraction::from_adapter_output(
        &request,
        document.sha256.clone(),
        provenance,
        native.raw_text,
        Vec::new(),
        candidates,
        warnings,
    )
    .map_err(OwnerMutationAuditError::domain)
}

fn source_format(value: &str) -> OwnerMutationAuditResult<core::bank_import::BankSourceFormat> {
    match value {
        "csv" => Ok(core::bank_import::BankSourceFormat::Csv),
        "ofx" => Ok(core::bank_import::BankSourceFormat::Ofx),
        "qfx" => Ok(core::bank_import::BankSourceFormat::Qfx),
        _ => Err(OwnerMutationAuditError::stale(
            "persisted bank activity has an unsupported source format",
        )),
    }
}

fn source_kind(value: &str) -> OwnerMutationAuditResult<core::SourceKind> {
    match value {
        "csv" => Ok(core::SourceKind::Csv),
        "ofx" => Ok(core::SourceKind::Ofx),
        "qfx" => Ok(core::SourceKind::Qfx),
        _ => Err(OwnerMutationAuditError::stale(
            "persisted bank activity has an unsupported provenance kind",
        )),
    }
}

fn bank_line_from_activity(view: &BankActivityView) -> OwnerMutationAuditResult<core::bank_import::BankLine> {
    let activity = &view.activity;
    let provenance = core::SourceProvenance::new(
        source_kind(&activity.provenance_kind)?,
        activity.provenance_source_reference.clone(),
        activity.provenance_fingerprint.clone(),
        activity.provenance_label.clone(),
    )
    .map_err(OwnerMutationAuditError::domain)?;
    let line = core::bank_import::BankLine::new(
        core::RecordId::new(activity.source_account_id.clone()).map_err(OwnerMutationAuditError::domain)?,
        activity.institution_account_id.clone(),
        source_format(&activity.source_format)?,
        activity.source_file_sha256.clone(),
        activity.source_locator.clone(),
        parse_date(&activity.posted_date)?,
        activity.value_date.as_deref().map(parse_date).transpose()?,
        activity.signed_amount_minor,
        activity.description.clone(),
        activity.payee.clone(),
        activity.reference.clone(),
        activity.external_transaction_id.clone(),
        activity.raw_record_sha256.clone(),
        provenance,
    )
    .map_err(OwnerMutationAuditError::domain)?;
    if line.strong_identity_key() != activity.strong_identity_key {
        return Err(OwnerMutationAuditError::stale(
            "persisted bank activity identity no longer reconstructs canonically",
        ));
    }
    Ok(line)
}

fn match_level(value: core::matching::MatchLevel) -> &'static str {
    match value {
        core::matching::MatchLevel::Unmatched => "unmatched",
        core::matching::MatchLevel::Possible => "possible",
        core::matching::MatchLevel::Likely => "likely",
        core::matching::MatchLevel::Exact => "exact",
    }
}

fn receipt_reason(value: core::receipt_bank_suggestion::ReceiptMatchReason) -> &'static str {
    use core::receipt_bank_suggestion::ReceiptMatchReason as R;
    match value {
        R::AmountExact => "amountExact",
        R::DateExact => "dateExact",
        R::DateWithinThreeDays => "dateWithinThreeDays",
        R::DateWithinSevenDays => "dateWithinSevenDays",
        R::CurrencyExact => "currencyExact",
        R::ReferenceExact => "referenceExact",
        R::MerchantPayeeStrong => "merchantPayeeStrong",
        R::MerchantDescriptionStrong => "merchantDescriptionStrong",
        R::MultipleTopCandidates => "multipleTopCandidates",
        R::MissingTotal => "missingTotal",
        R::MissingDate => "missingDate",
        R::MissingCurrency => "missingCurrency",
        R::BankLineNotOutflow => "bankLineNotOutflow",
        R::BankAmountUnrepresentable => "bankAmountUnrepresentable",
        R::AmountMismatch => "amountMismatch",
        R::CurrencyMismatch => "currencyMismatch",
        R::DateTooFar => "dateTooFar",
        R::InsufficientEvidence => "insufficientEvidence",
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerReceiptSuggestRequest {
    books: OwnerMutationBooksRef,
    request_id: String,
    document_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReceiptCandidateView {
    candidate_id: String,
    posted_date: String,
    amount_pence: i64,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    level: &'static str,
    score: u16,
    reasons: Vec<&'static str>,
    requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum OwnerReceiptSuggestionOutcome {
    Ready {
        bridge_version: u32,
        suggestion_id: String,
        document_id: String,
        candidates: Vec<OwnerReceiptCandidateView>,
        recommended_candidate_id: Option<String>,
        ambiguous_top: bool,
        requires_confirmation: bool,
    },
    OcrUnavailable { reason: String },
    OcrFailed { kind: String },
}

fn decode_native_ocr(outcome: &ShellOcrOutcome) -> OwnerMutationAuditResult<NativeOcrMirror> {
    let value = serde_json::to_value(outcome)
        .map_err(|_| OwnerMutationAuditError::document("OCR result could not be serialized"))?;
    serde_json::from_value(value)
        .map_err(|_| OwnerMutationAuditError::document("OCR result does not match the factual contract"))
}

#[tauri::command]
pub(crate) fn owner_receipt_suggest_bank(
    app: tauri::AppHandle,
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    ocr_registry: tauri::State<'_, NativeOcrRegistry>,
    suggestions: tauri::State<'_, ReceiptSuggestionRegistry>,
    request: OwnerReceiptSuggestRequest,
) -> OwnerMutationAuditResult<OwnerReceiptSuggestionOutcome> {
    let request_id = bounded_id(&request.request_id, "OCR request id")?;
    let document_id = bounded_id(&request.document_id, "document id")?;
    let ocr_request: super::owner_documents_ocr::OwnerOcrExtractRequest =
        serde_json::from_value(serde_json::json!({
            "books": {
                "fileName": request.books.file_name,
                "booksId": request.books.books_id,
                "actor": request.books.actor
            },
            "requestId": request_id,
            "documentId": document_id
        }))
        .map_err(|_| OwnerMutationAuditError::invalid("receipt OCR request is invalid"))?;
    let native = super::owner_documents_ocr::owner_ocr_extract_receipt(
        app,
        roots,
        ocr_registry,
        ocr_request,
    )
    .map_err(|error| OwnerMutationAuditError::document(format!("{error:?}")))?;
    let mirrored = decode_native_ocr(&native)?;
    let extraction_mirror = match mirrored {
        NativeOcrMirror::Completed { extraction } => extraction,
        NativeOcrMirror::Unavailable { reason } => {
            return Ok(OwnerReceiptSuggestionOutcome::OcrUnavailable { reason })
        }
        NativeOcrMirror::Failed { kind } => {
            return Ok(OwnerReceiptSuggestionOutcome::OcrFailed { kind })
        }
    };

    let books = request.books.open()?;
    let document = books
        .document(&document_id)
        .map_err(OwnerMutationAuditError::foundation)?;
    let extraction = core_extraction_from_native(extraction_mirror, &request_id, &document)?;
    let activity = books
        .list_bank_activity(MAX_BANK_ACTIVITY_CANDIDATES, 0)
        .map_err(OwnerMutationAuditError::foundation)?;
    let mut lines = Vec::new();
    let mut authoritative = Vec::new();
    for row in activity {
        if row.matched_transaction_id.is_some() || row.activity.signed_amount_minor >= 0 {
            continue;
        }
        let line = bank_line_from_activity(&row)?;
        let identity = core::receipt_bank_suggestion::BankLineIdentity::for_line(&line);
        authoritative.push((row, identity));
        lines.push(line);
    }
    let ranked = core::receipt_bank_suggestion::rank_receipt_bank_suggestions(&extraction, &lines);
    let suggestion_id = suggestions.new_suggestion_id(&request.books, &request_id, &document_id)?;

    let mut registered = Vec::new();
    let mut response_candidates = Vec::new();
    for candidate in ranked.candidates() {
        let Some((row, identity)) = authoritative
            .iter()
            .find(|(_, identity)| identity == candidate.bank_line())
        else {
            return Err(OwnerMutationAuditError::stale(
                "receipt suggestion candidate lost its authoritative bank identity",
            ));
        };
        let token = ReceiptSuggestionRegistry::candidate_token(&suggestion_id, identity);
        registered.push(RegisteredReceiptCandidate {
            token: token.clone(),
            identity: identity.clone(),
            bank_activity_id: row.id,
            posted_date: row.activity.posted_date.clone(),
            amount_pence: row.activity.signed_amount_minor,
            description: row.activity.description.clone(),
            payee: row.activity.payee.clone(),
            reference: row.activity.reference.clone(),
        });
        response_candidates.push(OwnerReceiptCandidateView {
            candidate_id: token,
            posted_date: row.activity.posted_date.clone(),
            amount_pence: row.activity.signed_amount_minor,
            description: row.activity.description.clone(),
            payee: row.activity.payee.clone(),
            reference: row.activity.reference.clone(),
            level: match_level(candidate.level()),
            score: candidate.score(),
            reasons: candidate
                .reasons()
                .iter()
                .copied()
                .map(receipt_reason)
                .collect(),
            requires_confirmation: candidate.requires_user_confirmation(),
        });
    }
    let recommended_candidate_id = ranked.recommended_bank_line().and_then(|identity| {
        registered
            .iter()
            .find(|candidate| &candidate.identity == identity)
            .map(|candidate| candidate.token.clone())
    });
    let outcome = OwnerReceiptSuggestionOutcome::Ready {
        bridge_version: OWNER_MUTATION_AUDIT_VERSION,
        suggestion_id: suggestion_id.clone(),
        document_id: document_id.clone(),
        candidates: response_candidates,
        recommended_candidate_id,
        ambiguous_top: ranked.ambiguous_top(),
        requires_confirmation: ranked.requires_user_confirmation(),
    };
    suggestions.insert(
        suggestion_id,
        RegisteredReceiptSuggestion {
            file_name: request.books.file_name,
            books_id: request.books.books_id,
            document_id,
            suggestions: ranked,
            candidates: registered,
        },
    )?;
    Ok(outcome)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerReceiptConfirmRequest {
    books: OwnerMutationBooksRef,
    suggestion_id: String,
    candidate_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerReceiptRejectRequest {
    books: OwnerMutationBooksRef,
    suggestion_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReceiptDecisionReceipt {
    bridge_version: u32,
    suggestion_id: String,
    document_id: String,
    decision: &'static str,
    recorded: bool,
    already_recorded: bool,
    requires_further_automatic_action: bool,
}

fn require_same_books(
    books: &OwnerMutationBooksRef,
    registered: &RegisteredReceiptSuggestion,
) -> OwnerMutationAuditResult<()> {
    if books.file_name != registered.file_name || books.books_id != registered.books_id {
        return Err(OwnerMutationAuditError::stale(
            "receipt suggestion belongs to different books",
        ));
    }
    Ok(())
}

fn verify_registered_document(
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    books: &OwnerMutationBooksRef,
    document_id: &str,
) -> OwnerMutationAuditResult<()> {
    let verify_request: super::owner_documents_ocr::OwnerDocumentVerifyRequest =
        serde_json::from_value(serde_json::json!({
            "books": {
                "fileName": books.file_name,
                "booksId": books.books_id,
                "actor": books.actor
            },
            "documentId": document_id
        }))
        .map_err(|_| OwnerMutationAuditError::invalid("document verification request is invalid"))?;
    let outcome = super::owner_documents_ocr::owner_document_verify(roots, verify_request)
        .map_err(|error| OwnerMutationAuditError::document(format!("{error:?}")))?;
    let value = serde_json::to_value(outcome)
        .map_err(|_| OwnerMutationAuditError::document("document verification could not be serialized"))?;
    if value.get("integrity").and_then(serde_json::Value::as_str) != Some("verified") {
        return Err(OwnerMutationAuditError::stale(
            "registered document changed after receipt review; review it again",
        ));
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn owner_receipt_confirm_bank(
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    suggestions: tauri::State<'_, ReceiptSuggestionRegistry>,
    request: OwnerReceiptConfirmRequest,
) -> OwnerMutationAuditResult<OwnerReceiptDecisionReceipt> {
    let registered = suggestions.resolve(&request.suggestion_id)?;
    require_same_books(&request.books, &registered)?;
    verify_registered_document(roots, &request.books, &registered.document_id)?;
    let candidate_id = bounded_id(&request.candidate_id, "receipt candidate id")?;
    let candidate = registered
        .candidates
        .iter()
        .find(|candidate| candidate.token == candidate_id)
        .ok_or_else(|| OwnerMutationAuditError::stale("receipt candidate is not part of this suggestion"))?;
    core::receipt_bank_suggestion::confirm_receipt_bank_suggestion(
        &registered.suggestions,
        &candidate.identity,
        request.books.actor.clone(),
    )
    .map_err(OwnerMutationAuditError::domain)?;

    let books = request.books.open()?;
    let current = books
        .bank_activity(candidate.bank_activity_id)
        .map_err(OwnerMutationAuditError::foundation)?;
    if current.matched_transaction_id.is_some() {
        return Err(OwnerMutationAuditError::stale(
            "bank activity changed after receipt review; generate a new suggestion",
        ));
    }
    let current_line = bank_line_from_activity(&current)?;
    if core::receipt_bank_suggestion::BankLineIdentity::for_line(&current_line) != candidate.identity {
        return Err(OwnerMutationAuditError::stale(
            "bank activity identity changed after receipt review",
        ));
    }
    let write = ReceiptBankDecisionWrite {
        suggestion_id: request.suggestion_id.clone(),
        document_id: registered.document_id.clone(),
        decision_kind: "confirmed".to_string(),
        bank_activity_id: Some(candidate.bank_activity_id),
        source_account_id: Some(candidate.identity.source_account_id().as_str().to_string()),
        source_file_sha256: Some(candidate.identity.source_file_sha256().to_string()),
        source_locator: Some(candidate.identity.source_locator().to_string()),
        raw_record_sha256: Some(candidate.identity.raw_record_sha256().to_string()),
    };
    let persisted = books
        .persist_receipt_bank_decision(&write)
        .map_err(OwnerMutationAuditError::foundation)?;
    let (recorded, already_recorded) = match persisted {
        ReceiptBankDecisionPersistOutcome::Recorded(_) => (true, false),
        ReceiptBankDecisionPersistOutcome::AlreadyRecorded(_) => (false, true),
    };
    Ok(OwnerReceiptDecisionReceipt {
        bridge_version: OWNER_MUTATION_AUDIT_VERSION,
        suggestion_id: request.suggestion_id,
        document_id: registered.document_id,
        decision: "confirmed",
        recorded,
        already_recorded,
        requires_further_automatic_action: false,
    })
}

#[tauri::command]
pub(crate) fn owner_receipt_reject_bank(
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    suggestions: tauri::State<'_, ReceiptSuggestionRegistry>,
    request: OwnerReceiptRejectRequest,
) -> OwnerMutationAuditResult<OwnerReceiptDecisionReceipt> {
    let registered = suggestions.resolve(&request.suggestion_id)?;
    require_same_books(&request.books, &registered)?;
    verify_registered_document(roots, &request.books, &registered.document_id)?;
    core::receipt_bank_suggestion::reject_receipt_bank_suggestions(
        &registered.suggestions,
        request.books.actor.clone(),
    )
    .map_err(OwnerMutationAuditError::domain)?;
    let books = request.books.open()?;
    let persisted = books
        .persist_receipt_bank_decision(&ReceiptBankDecisionWrite {
            suggestion_id: request.suggestion_id.clone(),
            document_id: registered.document_id.clone(),
            decision_kind: "rejected".to_string(),
            bank_activity_id: None,
            source_account_id: None,
            source_file_sha256: None,
            source_locator: None,
            raw_record_sha256: None,
        })
        .map_err(OwnerMutationAuditError::foundation)?;
    let (recorded, already_recorded) = match persisted {
        ReceiptBankDecisionPersistOutcome::Recorded(_) => (true, false),
        ReceiptBankDecisionPersistOutcome::AlreadyRecorded(_) => (false, true),
    };
    Ok(OwnerReceiptDecisionReceipt {
        bridge_version: OWNER_MUTATION_AUDIT_VERSION,
        suggestion_id: request.suggestion_id,
        document_id: registered.document_id,
        decision: "rejected",
        recorded,
        already_recorded,
        requires_further_automatic_action: false,
    })
}

// -----------------------------------------------------------------------------
// Correction preview / confirm / history
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerCorrectionRecordKind {
    MoneyIn,
    MoneyOut,
}

impl OwnerCorrectionRecordKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MoneyIn => "moneyIn",
            Self::MoneyOut => "moneyOut",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum CorrectionSettlement {
    BusinessBank,
    Cash,
}

impl From<CorrectionSettlement> for core::SettlementAccount {
    fn from(value: CorrectionSettlement) -> Self {
        match value {
            CorrectionSettlement::BusinessBank => Self::BusinessBank,
            CorrectionSettlement::Cash => Self::Cash,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum CorrectionIncomeCategory {
    SalesTrading,
    OtherBusinessIncome,
}

impl From<CorrectionIncomeCategory> for core::IncomeCategory {
    fn from(value: CorrectionIncomeCategory) -> Self {
        match value {
            CorrectionIncomeCategory::SalesTrading => Self::SalesTrading,
            CorrectionIncomeCategory::OtherBusinessIncome => Self::OtherBusinessIncome,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum CorrectionExpenseCategory {
    GoodsStockMaterials,
    OfficePhoneSoftware,
    Travel,
    VehicleCosts,
    PremisesUtilities,
    AdvertisingMarketing,
    BankFinanceInsurance,
    ProfessionalFees,
    RepairsMaintenance,
    Training,
    StaffSubcontractorCosts,
    OtherBusinessExpense,
}

impl From<CorrectionExpenseCategory> for core::ExpenseCategory {
    fn from(value: CorrectionExpenseCategory) -> Self {
        match value {
            CorrectionExpenseCategory::GoodsStockMaterials => Self::GoodsStockMaterials,
            CorrectionExpenseCategory::OfficePhoneSoftware => Self::OfficePhoneSoftware,
            CorrectionExpenseCategory::Travel => Self::Travel,
            CorrectionExpenseCategory::VehicleCosts => Self::VehicleCosts,
            CorrectionExpenseCategory::PremisesUtilities => Self::PremisesUtilities,
            CorrectionExpenseCategory::AdvertisingMarketing => Self::AdvertisingMarketing,
            CorrectionExpenseCategory::BankFinanceInsurance => Self::BankFinanceInsurance,
            CorrectionExpenseCategory::ProfessionalFees => Self::ProfessionalFees,
            CorrectionExpenseCategory::RepairsMaintenance => Self::RepairsMaintenance,
            CorrectionExpenseCategory::Training => Self::Training,
            CorrectionExpenseCategory::StaffSubcontractorCosts => Self::StaffSubcontractorCosts,
            CorrectionExpenseCategory::OtherBusinessExpense => Self::OtherBusinessExpense,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum CorrectionBusinessUse {
    Business,
    Private,
    Mixed {
        #[serde(rename = "businessBasisPoints")]
        business_basis_points: u16,
    },
}

impl CorrectionBusinessUse {
    fn to_core(&self) -> OwnerMutationAuditResult<core::BusinessUse> {
        match self {
            Self::Business => Ok(core::BusinessUse::Business),
            Self::Private => Ok(core::BusinessUse::Private),
            Self::Mixed { business_basis_points } => core::BusinessUse::mixed(*business_basis_points)
                .map_err(OwnerMutationAuditError::domain),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum OwnerCorrectionReplacement {
    MoneyIn {
        record_id: String,
        description: String,
        date: String,
        amount_pence: i64,
        category: CorrectionIncomeCategory,
        settlement: CorrectionSettlement,
    },
    MoneyOut {
        record_id: String,
        description: String,
        date: String,
        amount_pence: i64,
        category: CorrectionExpenseCategory,
        business_use: CorrectionBusinessUse,
        settlement: CorrectionSettlement,
    },
}

impl OwnerCorrectionReplacement {
    fn record_id(&self) -> &str {
        match self {
            Self::MoneyIn { record_id, .. } | Self::MoneyOut { record_id, .. } => record_id,
        }
    }

    fn kind(&self) -> OwnerCorrectionRecordKind {
        match self {
            Self::MoneyIn { .. } => OwnerCorrectionRecordKind::MoneyIn,
            Self::MoneyOut { .. } => OwnerCorrectionRecordKind::MoneyOut,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCorrectionRequest {
    books: OwnerMutationBooksRef,
    record_kind: OwnerCorrectionRecordKind,
    original_record_id: String,
    reversal_record_id: String,
    replacement: Option<OwnerCorrectionReplacement>,
    reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCorrectionConfirmRequest {
    correction: OwnerCorrectionRequest,
    preview_fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCorrectionHistoryRequest {
    books: OwnerMutationBooksRef,
    record_kind: OwnerCorrectionRecordKind,
    record_id: String,
    limit: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCorrectionTransactionSummary {
    record_id: String,
    date: String,
    description: String,
    amount_pence: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCorrectionPreview {
    bridge_version: u32,
    record_kind: OwnerCorrectionRecordKind,
    original: OwnerCorrectionTransactionSummary,
    reversal: OwnerCorrectionTransactionSummary,
    replacement: Option<OwnerCorrectionTransactionSummary>,
    preview_fingerprint: String,
    correction_id: String,
    requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCorrectionReceipt {
    bridge_version: u32,
    correction_id: String,
    record_kind: OwnerCorrectionRecordKind,
    original_record_id: String,
    reversal_record_id: String,
    replacement_record_id: Option<String>,
    applied: bool,
    already_applied: bool,
    requires_further_automatic_action: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCorrectionHistoryItem {
    correction_id: String,
    record_kind: String,
    original_record_id: String,
    reversal_record_id: String,
    replacement_record_id: Option<String>,
    reason: String,
    corrected_by: String,
    corrected_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCorrectionHistory {
    bridge_version: u32,
    items: Vec<OwnerCorrectionHistoryItem>,
}

struct CorrectionExecutionPlan {
    preview: OwnerCorrectionPreview,
    write: OwnerCorrectionWrite,
}

fn foundation_request_from_plan(
    plan: &core::PostingPlan,
    record_kind: &str,
    record_id: &str,
) -> PostTransactionRequest {
    PostTransactionRequest {
        description: plan.description().to_string(),
        date: plan.date().iso(),
        currency_code: "GBP".to_string(),
        reference: Some(format!("sbc7b1:{record_kind}:{record_id}")),
        metadata: Some(
            serde_json::json!({
                "source": "owner-ui-correction",
                "recordKind": record_kind,
                "recordId": record_id,
                "bridgeVersion": OWNER_MUTATION_AUDIT_VERSION
            })
            .to_string(),
        ),
        lines: plan
            .lines()
            .iter()
            .map(|line| PostingLine {
                account_code: line.account_code().to_string(),
                direction: match line.direction() {
                    core::PostingDirection::Debit => Direction::Debit,
                    core::PostingDirection::Credit => Direction::Credit,
                },
                amount_minor: line.amount_minor(),
                memo: None,
            })
            .collect(),
    }
}

fn replacement_request(
    replacement: &OwnerCorrectionReplacement,
    record_kind: OwnerCorrectionRecordKind,
) -> OwnerMutationAuditResult<PostTransactionRequest> {
    if replacement.kind() != record_kind {
        return Err(OwnerMutationAuditError::invalid(
            "correction replacement must have the same record kind as the original",
        ));
    }
    match replacement {
        OwnerCorrectionReplacement::MoneyIn {
            record_id,
            description,
            date,
            amount_pence,
            category,
            settlement,
        } => {
            let id = core::RecordId::new(record_id.clone()).map_err(OwnerMutationAuditError::domain)?;
            let record = core::IncomeRecord::new(
                id.clone(),
                description.clone(),
                parse_date(date)?,
                core::GbpAmount::positive_minor(*amount_pence).map_err(OwnerMutationAuditError::domain)?,
                (*category).into(),
                (*settlement).into(),
                core::SourceProvenance::manual(),
            )
            .map_err(OwnerMutationAuditError::domain)?;
            Ok(foundation_request_from_plan(&core::plan_income(&record), "moneyIn", id.as_str()))
        }
        OwnerCorrectionReplacement::MoneyOut {
            record_id,
            description,
            date,
            amount_pence,
            category,
            business_use,
            settlement,
        } => {
            let id = core::RecordId::new(record_id.clone()).map_err(OwnerMutationAuditError::domain)?;
            let record = core::ExpenseRecord::new(
                id.clone(),
                description.clone(),
                parse_date(date)?,
                core::GbpAmount::positive_minor(*amount_pence).map_err(OwnerMutationAuditError::domain)?,
                (*category).into(),
                business_use.to_core()?,
                (*settlement).into(),
                None,
                core::SourceProvenance::manual(),
            )
            .map_err(OwnerMutationAuditError::domain)?;
            let plan = core::plan_expense(&record).map_err(OwnerMutationAuditError::domain)?;
            Ok(foundation_request_from_plan(&plan, "moneyOut", id.as_str()))
        }
    }
}

fn transaction_total(transaction: &TransactionView) -> OwnerMutationAuditResult<i64> {
    let mut total: i128 = 0;
    for entry in &transaction.entries {
        if entry.direction == "debit" {
            total = total
                .checked_add(i128::from(entry.amount_minor))
                .ok_or_else(|| OwnerMutationAuditError::invalid("transaction total overflow"))?;
        }
    }
    i64::try_from(total).map_err(|_| OwnerMutationAuditError::invalid("transaction total overflow"))
}

fn reversal_request(
    original: &TransactionView,
    record_kind: OwnerCorrectionRecordKind,
    reversal_record_id: &str,
) -> OwnerMutationAuditResult<PostTransactionRequest> {
    if original.currency != "GBP" || original.entries.is_empty() {
        return Err(OwnerMutationAuditError::invalid(
            "authoritative owner transaction is not a supported GBP posting",
        ));
    }
    let lines = original
        .entries
        .iter()
        .map(|entry| {
            if entry.amount_minor <= 0 {
                return Err(OwnerMutationAuditError::invalid(
                    "authoritative transaction contains an invalid posting amount",
                ));
            }
            let direction = match entry.direction.as_str() {
                "debit" => Direction::Credit,
                "credit" => Direction::Debit,
                _ => {
                    return Err(OwnerMutationAuditError::invalid(
                        "authoritative transaction contains an invalid posting direction",
                    ))
                }
            };
            Ok(PostingLine {
                account_code: entry.account_code.clone(),
                direction,
                amount_minor: entry.amount_minor,
                memo: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PostTransactionRequest {
        description: format!("Correction reversal: {}", original.description),
        date: original.date.clone(),
        currency_code: "GBP".to_string(),
        reference: Some(format!(
            "sbc7b1:correctionReversal:{}:{}",
            record_kind.as_str(),
            reversal_record_id
        )),
        metadata: Some(
            serde_json::json!({
                "source": "owner-ui-correction",
                "kind": "reversal",
                "recordKind": record_kind.as_str(),
                "reversalRecordId": reversal_record_id,
                "bridgeVersion": OWNER_MUTATION_AUDIT_VERSION
            })
            .to_string(),
        ),
        lines,
    })
}

fn summary_from_request(record_id: &str, request: &PostTransactionRequest) -> OwnerMutationAuditResult<OwnerCorrectionTransactionSummary> {
    let transaction = TransactionView {
        id: 1,
        description: request.description.clone(),
        reference: request.reference.clone(),
        currency: request.currency_code.clone(),
        date: request.date.clone(),
        entries: request
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| shark_foundation::EntryView {
                id: i64::try_from(index + 1).unwrap_or(i64::MAX),
                account_code: line.account_code.clone(),
                direction: match line.direction {
                    Direction::Debit => "debit".to_string(),
                    Direction::Credit => "credit".to_string(),
                },
                amount_minor: line.amount_minor,
                status: "unposted".to_string(),
            })
            .collect(),
    };
    Ok(OwnerCorrectionTransactionSummary {
        record_id: record_id.to_string(),
        date: request.date.clone(),
        description: request.description.clone(),
        amount_pence: transaction_total(&transaction)?,
    })
}

fn build_correction_execution(
    request: &OwnerCorrectionRequest,
) -> OwnerMutationAuditResult<CorrectionExecutionPlan> {
    let original_record_id = bounded_id(&request.original_record_id, "original record id")?;
    let reversal_record_id = bounded_id(&request.reversal_record_id, "reversal record id")?;
    if request.reason.trim().is_empty() || request.reason.len() > 512 {
        return Err(OwnerMutationAuditError::invalid(
            "correction reason must be 1-512 non-whitespace characters",
        ));
    }
    let books = request.books.open()?;
    let reference = format!("sbc7b1:{}:{}", request.record_kind.as_str(), original_record_id);
    let originals = books
        .find_by_reference(&reference)
        .map_err(OwnerMutationAuditError::foundation)?;
    if originals.len() != 1 {
        return Err(OwnerMutationAuditError::stale(
            "correction requires exactly one authoritative original owner transaction",
        ));
    }
    if books
        .correction_for_original(request.record_kind.as_str(), &original_record_id)
        .map_err(OwnerMutationAuditError::foundation)?
        .is_some()
    {
        return Err(OwnerMutationAuditError::stale(
            "this owner record has already been superseded; target its replacement explicitly",
        ));
    }
    let original = &originals[0];
    let replacement_id = request
        .replacement
        .as_ref()
        .map(|replacement| core::RecordId::new(replacement.record_id().to_string()))
        .transpose()
        .map_err(OwnerMutationAuditError::domain)?;
    let original_core_id = core::RecordId::new(original_record_id.clone()).map_err(OwnerMutationAuditError::domain)?;
    let reversal_core_id = core::RecordId::new(reversal_record_id.clone()).map_err(OwnerMutationAuditError::domain)?;
    let _plan = core::matching::CorrectionPlan::new(
        original_core_id,
        reversal_core_id,
        replacement_id.clone(),
        request.books.actor.clone(),
        request.reason.clone(),
    )
    .map_err(OwnerMutationAuditError::domain)?;

    let reversal = reversal_request(original, request.record_kind, &reversal_record_id)?;
    let replacement = request
        .replacement
        .as_ref()
        .map(|replacement| replacement_request(replacement, request.record_kind))
        .transpose()?;
    if !books
        .find_by_reference(reversal.reference.as_deref().expect("reversal reference"))
        .map_err(OwnerMutationAuditError::foundation)?
        .is_empty()
    {
        return Err(OwnerMutationAuditError::stale(
            "correction reversal identity already exists",
        ));
    }
    if let Some(replacement_request) = replacement.as_ref() {
        if !books
            .find_by_reference(replacement_request.reference.as_deref().expect("replacement reference"))
            .map_err(OwnerMutationAuditError::foundation)?
            .is_empty()
        {
            return Err(OwnerMutationAuditError::stale(
                "correction replacement identity already exists",
            ));
        }
    }

    let fingerprint_payload = serde_json::json!({
        "booksId": request.books.books_id,
        "actor": request.books.actor,
        "recordKind": request.record_kind.as_str(),
        "originalRecordId": original_record_id,
        "reversalRecordId": reversal_record_id,
        "reason": request.reason,
        "original": original,
        "reversal": reversal,
        "replacement": replacement,
    });
    let fingerprint_bytes = serde_json::to_vec(&fingerprint_payload)
        .map_err(|_| OwnerMutationAuditError::invalid("correction preview could not be fingerprinted"))?;
    let digest = core::bank_import::sha256_hex(&fingerprint_bytes);
    let preview_fingerprint = format!("correction-preview-{digest}");
    let correction_id = format!("correction-{digest}");

    let original_summary = OwnerCorrectionTransactionSummary {
        record_id: original_record_id.clone(),
        date: original.date.clone(),
        description: original.description.clone(),
        amount_pence: transaction_total(original)?,
    };
    let reversal_summary = summary_from_request(&reversal_record_id, &reversal)?;
    let replacement_summary = match (&request.replacement, &replacement) {
        (Some(spec), Some(posting)) => Some(summary_from_request(spec.record_id(), posting)?),
        (None, None) => None,
        _ => return Err(OwnerMutationAuditError::invalid("replacement preview is inconsistent")),
    };
    let write = OwnerCorrectionWrite {
        correction_id: correction_id.clone(),
        record_kind: request.record_kind.as_str().to_string(),
        original_record_id: original_record_id.clone(),
        original_transaction_id: original.id,
        reversal_record_id: reversal_record_id.clone(),
        reversal_request: reversal,
        replacement_record_id: replacement_id.map(|id| id.as_str().to_string()),
        replacement_request: replacement,
        reason: request.reason.trim().to_string(),
    };
    Ok(CorrectionExecutionPlan {
        preview: OwnerCorrectionPreview {
            bridge_version: OWNER_MUTATION_AUDIT_VERSION,
            record_kind: request.record_kind,
            original: original_summary,
            reversal: reversal_summary,
            replacement: replacement_summary,
            preview_fingerprint,
            correction_id,
            requires_confirmation: true,
        },
        write,
    })
}

#[tauri::command]
pub(crate) fn owner_correction_preview(
    request: OwnerCorrectionRequest,
) -> OwnerMutationAuditResult<OwnerCorrectionPreview> {
    Ok(build_correction_execution(&request)?.preview)
}

#[tauri::command]
pub(crate) fn owner_correction_confirm(
    request: OwnerCorrectionConfirmRequest,
) -> OwnerMutationAuditResult<OwnerCorrectionReceipt> {
    let supplied = bounded_id(&request.preview_fingerprint, "correction preview fingerprint")?;
    let execution = build_correction_execution(&request.correction)?;
    if supplied != execution.preview.preview_fingerprint {
        return Err(OwnerMutationAuditError::stale(
            "correction preview changed; preview again before confirming",
        ));
    }
    let books = request.correction.books.open()?;
    let persisted = books
        .apply_owner_correction(&execution.write)
        .map_err(OwnerMutationAuditError::foundation)?;
    let (view, applied, already_applied) = match persisted {
        OwnerCorrectionPersistOutcome::Applied(view) => (view, true, false),
        OwnerCorrectionPersistOutcome::AlreadyApplied(view) => (view, false, true),
    };
    Ok(OwnerCorrectionReceipt {
        bridge_version: OWNER_MUTATION_AUDIT_VERSION,
        correction_id: view.correction_id,
        record_kind: request.correction.record_kind,
        original_record_id: view.original_record_id,
        reversal_record_id: view.reversal_record_id,
        replacement_record_id: view.replacement_record_id,
        applied,
        already_applied,
        requires_further_automatic_action: false,
    })
}

#[tauri::command]
pub(crate) fn owner_correction_history(
    request: OwnerCorrectionHistoryRequest,
) -> OwnerMutationAuditResult<OwnerCorrectionHistory> {
    let record_id = bounded_id(&request.record_id, "correction history record id")?;
    if !(1..=MAX_CORRECTION_HISTORY).contains(&request.limit) {
        return Err(OwnerMutationAuditError::invalid(
            "correction history limit must be between 1 and 100",
        ));
    }
    let books = request.books.open()?;
    let items = books
        .correction_history(request.record_kind.as_str(), &record_id, request.limit)
        .map_err(OwnerMutationAuditError::foundation)?
        .into_iter()
        .map(|view| OwnerCorrectionHistoryItem {
            correction_id: view.correction_id,
            record_kind: view.record_kind,
            original_record_id: view.original_record_id,
            reversal_record_id: view.reversal_record_id,
            replacement_record_id: view.replacement_record_id,
            reason: view.reason,
            corrected_by: view.corrected_by,
            corrected_at: view.corrected_at,
        })
        .collect();
    Ok(OwnerCorrectionHistory {
        bridge_version: OWNER_MUTATION_AUDIT_VERSION,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_requests_reject_raw_bank_path_and_accounting_authority() {
        let base = serde_json::json!({
            "books": { "fileName": "safe.sqlite", "booksId": "safe-books", "actor": "owner" },
            "requestId": "ocr-1",
            "documentId": "doc-1"
        });
        assert!(serde_json::from_value::<OwnerReceiptSuggestRequest>(base.clone()).is_ok());
        for forbidden in [
            "bankActivityId", "transactionId", "accountCode", "databasePath", "sourcePath",
            "modelPath", "shellCommand", "taxTreatment", "businessUse",
        ] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().insert(
                forbidden.to_string(),
                serde_json::Value::String("forbidden".to_string()),
            );
            assert!(serde_json::from_value::<OwnerReceiptSuggestRequest>(bad).is_err());
        }
    }

    #[test]
    fn correction_request_rejects_raw_transaction_and_posting_authority() {
        let base = serde_json::json!({
            "books": { "fileName": "safe.sqlite", "booksId": "safe-books", "actor": "owner" },
            "recordKind": "moneyIn",
            "originalRecordId": "sale-1",
            "reversalRecordId": "reversal-1",
            "replacement": null,
            "reason": "Correct mistake"
        });
        assert!(serde_json::from_value::<OwnerCorrectionRequest>(base.clone()).is_ok());
        for forbidden in ["transactionId", "accountCode", "debit", "credit", "databasePath"] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().insert(
                forbidden.to_string(),
                serde_json::Value::String("forbidden".to_string()),
            );
            assert!(serde_json::from_value::<OwnerCorrectionRequest>(bad).is_err());
        }
    }

    #[test]
    fn receipt_registry_is_bounded_and_oldest_entry_expires() {
        let registry = ReceiptSuggestionRegistry::default();
        let books = OwnerMutationBooksRef {
            file_name: "safe.sqlite".into(),
            books_id: "safe-books".into(),
            actor: "owner".into(),
        };
        for index in 0..=MAX_RECEIPT_SUGGESTIONS {
            let id = format!("manual-{index}");
            let suggestions = core::receipt_bank_suggestion::rank_receipt_bank_suggestions(
                &sample_extraction(&format!("doc-{index}")),
                &[],
            );
            registry
                .insert(
                    id,
                    RegisteredReceiptSuggestion {
                        file_name: books.file_name.clone(),
                        books_id: books.books_id.clone(),
                        document_id: format!("doc-{index}"),
                        suggestions,
                        candidates: vec![],
                    },
                )
                .unwrap();
        }
        assert!(registry.resolve("manual-0").is_err());
        assert!(registry
            .resolve(&format!("manual-{MAX_RECEIPT_SUGGESTIONS}"))
            .is_ok());
    }

    fn sample_extraction(document_id: &str) -> core::ocr::OcrExtraction {
        let id = core::RecordId::new(document_id).unwrap();
        let reference = core::documents::DocumentReference::from_persisted(
            id.clone(),
            id,
            core::RecordId::new("root-1").unwrap(),
            "documents/receipt.txt",
            "receipt.txt",
            Some("text/plain".into()),
            "11".repeat(32),
            1,
        )
        .unwrap();
        let request = core::ocr::OcrRequest::for_receipt_document(
            core::RecordId::new("request-1").unwrap(),
            &reference,
        );
        core::ocr::OcrExtraction::from_adapter_output(
            &request,
            "11".repeat(32),
            core::ocr::OcrEngineProvenance::new(
                "test",
                "1",
                "test-runtime",
                vec!["test-model".into()],
            )
            .unwrap(),
            "",
            vec![],
            core::ocr::OcrReceiptCandidates::default(),
            vec![],
        )
        .unwrap()
    }
}
