//! SBC-6B B2 implementation-neutral factual OCR application contract.
//!
//! This module deliberately contains no OCR engine, model runtime, filesystem,
//! network, subprocess, persistence or accounting-action implementation. It
//! defines only bounded Shark-owned request/result shapes so platform adapters
//! can provide optional local OCR without coupling `shark-books-core` to
//! PaddleOCR, PaddleX, ONNX Runtime, Python or a specific operating system.

use crate::documents::DocumentReference;
use crate::{Date, RecordId};
use std::fmt;

pub const OCR_CONTRACT_VERSION: u32 = 1;
pub const OCR_CONFIDENCE_BASIS_POINTS: u16 = 10_000;
pub const OCR_MAX_MODEL_IDS: usize = 8;
pub const OCR_MAX_REGIONS: usize = 4_096;
pub const OCR_MAX_WARNINGS: usize = 64;
pub const OCR_MAX_RAW_TEXT_BYTES: usize = 256 * 1024;

pub type OcrResult<T> = Result<T, OcrError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrError {
    InvalidConfidence(String),
    InvalidProvenance(String),
    InvalidCandidate(String),
    InvalidRegion(String),
    InvalidOutput(String),
    DocumentMismatch(String),
}
impl fmt::Display for OcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfidence(s)
            | Self::InvalidProvenance(s)
            | Self::InvalidCandidate(s)
            | Self::InvalidRegion(s)
            | Self::InvalidOutput(s)
            | Self::DocumentMismatch(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for OcrError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OcrConfidenceBps(u16);
impl OcrConfidenceBps {
    pub fn new(value: u16) -> OcrResult<Self> {
        if value > OCR_CONFIDENCE_BASIS_POINTS {
            return Err(OcrError::InvalidConfidence(
                "OCR confidence must be 0-10000 basis points".into(),
            ));
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn value(self) -> u16 { self.0 }
}

/// Integrity identity supplied to OCR. It intentionally excludes storage root,
/// relative filesystem path and attached accounting-record information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrDocumentIdentity {
    document_id: RecordId,
    sha256: String,
    byte_len: u64,
}
impl OcrDocumentIdentity {
    #[must_use]
    pub fn for_reference(reference: &DocumentReference) -> Self {
        Self {
            document_id: reference.id().clone(),
            sha256: reference.sha256().to_owned(),
            byte_len: reference.byte_len(),
        }
    }
    #[must_use] pub fn document_id(&self) -> &RecordId { &self.document_id }
    #[must_use] pub fn sha256(&self) -> &str { &self.sha256 }
    #[must_use] pub const fn byte_len(&self) -> u64 { self.byte_len }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrTask {
    ReceiptFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrRequest {
    request_id: RecordId,
    document: OcrDocumentIdentity,
    task: OcrTask,
}
impl OcrRequest {
    #[must_use]
    pub fn for_receipt_document(request_id: RecordId, reference: &DocumentReference) -> Self {
        Self {
            request_id,
            document: OcrDocumentIdentity::for_reference(reference),
            task: OcrTask::ReceiptFacts,
        }
    }
    #[must_use] pub fn request_id(&self) -> &RecordId { &self.request_id }
    #[must_use] pub fn document(&self) -> &OcrDocumentIdentity { &self.document }
    #[must_use] pub const fn task(&self) -> OcrTask { self.task }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrEngineProvenance {
    engine_id: String,
    engine_version: String,
    runtime_id: String,
    model_ids: Vec<String>,
}
impl OcrEngineProvenance {
    pub fn new(
        engine_id: impl Into<String>,
        engine_version: impl Into<String>,
        runtime_id: impl Into<String>,
        model_ids: Vec<String>,
    ) -> OcrResult<Self> {
        if model_ids.is_empty() || model_ids.len() > OCR_MAX_MODEL_IDS {
            return Err(OcrError::InvalidProvenance(format!(
                "OCR provenance must contain 1-{OCR_MAX_MODEL_IDS} model ids"
            )));
        }
        let engine_id = bounded_nonblank(engine_id.into(), "OCR engine id", 128, OcrError::InvalidProvenance)?;
        let engine_version = bounded_nonblank(engine_version.into(), "OCR engine version", 128, OcrError::InvalidProvenance)?;
        let runtime_id = bounded_nonblank(runtime_id.into(), "OCR runtime id", 128, OcrError::InvalidProvenance)?;
        let mut checked = Vec::with_capacity(model_ids.len());
        for model in model_ids {
            checked.push(bounded_nonblank(model, "OCR model id", 256, OcrError::InvalidProvenance)?);
        }
        Ok(Self { engine_id, engine_version, runtime_id, model_ids: checked })
    }
    #[must_use] pub fn engine_id(&self) -> &str { &self.engine_id }
    #[must_use] pub fn engine_version(&self) -> &str { &self.engine_version }
    #[must_use] pub fn runtime_id(&self) -> &str { &self.runtime_id }
    #[must_use] pub fn model_ids(&self) -> &[String] { &self.model_ids }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrTextCandidate {
    value: String,
    confidence: Option<OcrConfidenceBps>,
}
impl OcrTextCandidate {
    pub fn new(value: impl Into<String>, confidence: Option<OcrConfidenceBps>) -> OcrResult<Self> {
        Ok(Self {
            value: bounded_nonblank(value.into(), "OCR text candidate", 512, OcrError::InvalidCandidate)?,
            confidence,
        })
    }
    #[must_use] pub fn value(&self) -> &str { &self.value }
    #[must_use] pub const fn confidence(&self) -> Option<OcrConfidenceBps> { self.confidence }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcrDateCandidate {
    value: Date,
    confidence: Option<OcrConfidenceBps>,
}
impl OcrDateCandidate {
    #[must_use]
    pub const fn new(value: Date, confidence: Option<OcrConfidenceBps>) -> Self {
        Self { value, confidence }
    }
    #[must_use] pub const fn value(self) -> Date { self.value }
    #[must_use] pub const fn confidence(self) -> Option<OcrConfidenceBps> { self.confidence }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcrAmountCandidate {
    total_pence: i64,
    confidence: Option<OcrConfidenceBps>,
}
impl OcrAmountCandidate {
    pub fn new(total_pence: i64, confidence: Option<OcrConfidenceBps>) -> OcrResult<Self> {
        if total_pence < 0 {
            return Err(OcrError::InvalidCandidate(
                "OCR receipt total must not be negative".into(),
            ));
        }
        Ok(Self { total_pence, confidence })
    }
    #[must_use] pub const fn total_pence(self) -> i64 { self.total_pence }
    #[must_use] pub const fn confidence(self) -> Option<OcrConfidenceBps> { self.confidence }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrCurrencyCandidate {
    code: String,
    confidence: Option<OcrConfidenceBps>,
}
impl OcrCurrencyCandidate {
    pub fn new(code: impl Into<String>, confidence: Option<OcrConfidenceBps>) -> OcrResult<Self> {
        let code = code.into();
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(OcrError::InvalidCandidate(
                "OCR currency must be exactly three uppercase ASCII letters".into(),
            ));
        }
        Ok(Self { code, confidence })
    }
    #[must_use] pub fn code(&self) -> &str { &self.code }
    #[must_use] pub const fn confidence(&self) -> Option<OcrConfidenceBps> { self.confidence }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OcrReceiptCandidates {
    merchant_text: Option<OcrTextCandidate>,
    document_date: Option<OcrDateCandidate>,
    total: Option<OcrAmountCandidate>,
    currency: Option<OcrCurrencyCandidate>,
    reference: Option<OcrTextCandidate>,
}
impl OcrReceiptCandidates {
    #[must_use]
    pub const fn new(
        merchant_text: Option<OcrTextCandidate>,
        document_date: Option<OcrDateCandidate>,
        total: Option<OcrAmountCandidate>,
        currency: Option<OcrCurrencyCandidate>,
        reference: Option<OcrTextCandidate>,
    ) -> Self {
        Self { merchant_text, document_date, total, currency, reference }
    }
    #[must_use] pub fn merchant_text(&self) -> Option<&OcrTextCandidate> { self.merchant_text.as_ref() }
    #[must_use] pub const fn document_date(&self) -> Option<OcrDateCandidate> { self.document_date }
    #[must_use] pub const fn total(&self) -> Option<OcrAmountCandidate> { self.total }
    #[must_use] pub fn currency(&self) -> Option<&OcrCurrencyCandidate> { self.currency.as_ref() }
    #[must_use] pub fn reference(&self) -> Option<&OcrTextCandidate> { self.reference.as_ref() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrTextRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    text: String,
    confidence: Option<OcrConfidenceBps>,
}
impl OcrTextRegion {
    pub fn new(
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        text: impl Into<String>,
        confidence: Option<OcrConfidenceBps>,
    ) -> OcrResult<Self> {
        if width == 0 || height == 0 || x.checked_add(width).is_none() || y.checked_add(height).is_none() {
            return Err(OcrError::InvalidRegion(
                "OCR region dimensions must be positive and coordinate arithmetic must not overflow".into(),
            ));
        }
        Ok(Self {
            x, y, width, height,
            text: bounded_nonblank(text.into(), "OCR region text", 4096, OcrError::InvalidRegion)?,
            confidence,
        })
    }
    #[must_use] pub const fn x(&self) -> u32 { self.x }
    #[must_use] pub const fn y(&self) -> u32 { self.y }
    #[must_use] pub const fn width(&self) -> u32 { self.width }
    #[must_use] pub const fn height(&self) -> u32 { self.height }
    #[must_use] pub fn text(&self) -> &str { &self.text }
    #[must_use] pub const fn confidence(&self) -> Option<OcrConfidenceBps> { self.confidence }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrWarningCode {
    NoTextDetected,
    MissingMerchant,
    MissingDate,
    MissingTotal,
    MissingCurrency,
    MissingReference,
    LowConfidence,
    UnsupportedDocument,
    TruncatedOutput,
    EngineNotice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrWarning {
    code: OcrWarningCode,
    detail: Option<String>,
}
impl OcrWarning {
    pub fn new(code: OcrWarningCode, detail: Option<String>) -> OcrResult<Self> {
        let detail = match detail {
            Some(value) => Some(bounded_nonblank(value, "OCR warning detail", 1024, OcrError::InvalidOutput)?),
            None => None,
        };
        Ok(Self { code, detail })
    }
    #[must_use] pub const fn code(&self) -> OcrWarningCode { self.code }
    #[must_use] pub fn detail(&self) -> Option<&str> { self.detail.as_deref() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrExtraction {
    schema_version: u32,
    request_id: RecordId,
    document: OcrDocumentIdentity,
    provenance: OcrEngineProvenance,
    raw_text: String,
    regions: Vec<OcrTextRegion>,
    candidates: OcrReceiptCandidates,
    warnings: Vec<OcrWarning>,
}
impl OcrExtraction {
    #[allow(clippy::too_many_arguments)]
    pub fn from_adapter_output(
        request: &OcrRequest,
        observed_document_sha256: impl Into<String>,
        provenance: OcrEngineProvenance,
        raw_text: impl Into<String>,
        regions: Vec<OcrTextRegion>,
        candidates: OcrReceiptCandidates,
        warnings: Vec<OcrWarning>,
    ) -> OcrResult<Self> {
        let observed = canonical_sha256(observed_document_sha256.into())?;
        if observed != request.document.sha256 {
            return Err(OcrError::DocumentMismatch(
                "OCR output document hash does not match requested document".into(),
            ));
        }
        let raw_text = raw_text.into();
        if raw_text.len() > OCR_MAX_RAW_TEXT_BYTES || raw_text.contains('\0') {
            return Err(OcrError::InvalidOutput(
                "OCR raw text exceeds the bounded contract or contains NUL".into(),
            ));
        }
        if regions.len() > OCR_MAX_REGIONS {
            return Err(OcrError::InvalidOutput(format!(
                "OCR extraction exceeds {OCR_MAX_REGIONS} text regions"
            )));
        }
        if warnings.len() > OCR_MAX_WARNINGS {
            return Err(OcrError::InvalidOutput(format!(
                "OCR extraction exceeds {OCR_MAX_WARNINGS} warnings"
            )));
        }
        Ok(Self {
            schema_version: OCR_CONTRACT_VERSION,
            request_id: request.request_id.clone(),
            document: request.document.clone(),
            provenance,
            raw_text,
            regions,
            candidates,
            warnings,
        })
    }
    #[must_use] pub const fn schema_version(&self) -> u32 { self.schema_version }
    #[must_use] pub fn request_id(&self) -> &RecordId { &self.request_id }
    #[must_use] pub fn document(&self) -> &OcrDocumentIdentity { &self.document }
    #[must_use] pub fn provenance(&self) -> &OcrEngineProvenance { &self.provenance }
    #[must_use] pub fn raw_text(&self) -> &str { &self.raw_text }
    #[must_use] pub fn regions(&self) -> &[OcrTextRegion] { &self.regions }
    #[must_use] pub fn candidates(&self) -> &OcrReceiptCandidates { &self.candidates }
    #[must_use] pub fn warnings(&self) -> &[OcrWarning] { &self.warnings }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrUnavailableReason {
    DisabledByUser,
    NotInstalled,
    UnsupportedPlatform,
    ModelsUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrFailureKind {
    IntegrityMismatch,
    Timeout,
    MalformedOutput,
    EngineFailure,
    ResourceLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrOutcome {
    Completed(OcrExtraction),
    Unavailable(OcrUnavailableReason),
    Failed(OcrFailureKind),
}

fn canonical_sha256(value: String) -> OcrResult<String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(OcrError::InvalidOutput(
            "OCR document SHA-256 must be exactly 64 hexadecimal characters".into(),
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn bounded_nonblank(
    value: String,
    label: &str,
    max: usize,
    error: fn(String) -> OcrError,
) -> OcrResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max || trimmed.chars().any(|c| c == '\0') {
        return Err(error(format!("{label} must be 1-{max} nonblank characters without NUL")));
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::{DocumentReference, StorageProvider, StorageRootDescriptor};

    fn sample_document() -> DocumentReference {
        let root = StorageRootDescriptor::new(
            RecordId::new("root-1").unwrap(),
            StorageProvider::LocalFilesystem,
            "User receipts",
        ).unwrap();
        DocumentReference::from_bytes(
            RecordId::new("doc-1").unwrap(),
            RecordId::new("expense-1").unwrap(),
            &root,
            "2026/receipt.png",
            "receipt.png",
            Some("image/png".into()),
            b"receipt bytes",
        ).unwrap()
    }

    fn sample_request() -> OcrRequest {
        OcrRequest::for_receipt_document(RecordId::new("ocr-request-1").unwrap(), &sample_document())
    }

    fn provenance() -> OcrEngineProvenance {
        OcrEngineProvenance::new(
            "shark-direct-onnx",
            "1",
            "onnxruntime-1.23.2-cpu",
            vec!["PP-OCRv6_tiny_det".into(), "PP-OCRv6_tiny_rec".into()],
        ).unwrap()
    }

    #[test]
    fn request_preserves_only_document_integrity_identity() {
        let document = sample_document();
        let request = OcrRequest::for_receipt_document(RecordId::new("req").unwrap(), &document);
        assert_eq!(request.document().document_id(), document.id());
        assert_eq!(request.document().sha256(), document.sha256());
        assert_eq!(request.document().byte_len(), document.byte_len());
        let debug = format!("{request:?}");
        assert!(!debug.contains(document.relative_path()));
        assert!(!debug.contains(document.storage_root_id().as_str()));
    }

    #[test]
    fn confidence_accepts_closed_basis_point_range() {
        assert_eq!(OcrConfidenceBps::new(0).unwrap().value(), 0);
        assert_eq!(OcrConfidenceBps::new(10_000).unwrap().value(), 10_000);
    }

    #[test]
    fn confidence_over_one_hundred_percent_is_rejected() {
        assert!(OcrConfidenceBps::new(10_001).is_err());
    }

    #[test]
    fn provenance_requires_engine_runtime_and_models() {
        assert!(OcrEngineProvenance::new("", "1", "cpu", vec!["m".into()]).is_err());
        assert!(OcrEngineProvenance::new("e", "1", "cpu", Vec::new()).is_err());
    }

    #[test]
    fn provenance_model_count_is_bounded() {
        let models = (0..=OCR_MAX_MODEL_IDS).map(|i| format!("model-{i}")).collect();
        assert!(OcrEngineProvenance::new("e", "1", "cpu", models).is_err());
    }

    #[test]
    fn text_candidate_preserves_factual_value() {
        let c = OcrTextCandidate::new("  NORTH PIER  ", None).unwrap();
        assert_eq!(c.value(), "NORTH PIER");
    }

    #[test]
    fn blank_text_candidate_is_rejected() {
        assert!(OcrTextCandidate::new("   ", None).is_err());
    }

    #[test]
    fn negative_receipt_total_is_rejected() {
        assert!(OcrAmountCandidate::new(-1, None).is_err());
        assert_eq!(OcrAmountCandidate::new(0, None).unwrap().total_pence(), 0);
    }

    #[test]
    fn currency_is_factual_three_letter_uppercase_code() {
        assert_eq!(OcrCurrencyCandidate::new("GBP", None).unwrap().code(), "GBP");
        assert!(OcrCurrencyCandidate::new("gbp", None).is_err());
        assert!(OcrCurrencyCandidate::new("GB", None).is_err());
    }

    #[test]
    fn text_region_requires_positive_nonoverflowing_geometry() {
        assert!(OcrTextRegion::new(0, 0, 0, 1, "x", None).is_err());
        assert!(OcrTextRegion::new(u32::MAX, 0, 1, 1, "x", None).is_err());
        let region = OcrTextRegion::new(10, 20, 30, 40, "TOTAL GBP 28.49", None).unwrap();
        assert_eq!((region.x(), region.y(), region.width(), region.height()), (10, 20, 30, 40));
    }

    #[test]
    fn extraction_binds_to_requested_document_hash() {
        let request = sample_request();
        let extraction = OcrExtraction::from_adapter_output(
            &request,
            request.document().sha256().to_ascii_uppercase(),
            provenance(),
            "TOTAL GBP 28.49",
            Vec::new(),
            OcrReceiptCandidates::default(),
            Vec::new(),
        ).unwrap();
        assert_eq!(extraction.document().sha256(), request.document().sha256());
        assert_eq!(extraction.schema_version(), OCR_CONTRACT_VERSION);
    }

    #[test]
    fn mismatched_adapter_document_hash_fails_closed() {
        let request = sample_request();
        assert!(OcrExtraction::from_adapter_output(
            &request,
            "00".repeat(32),
            provenance(),
            "text",
            Vec::new(),
            OcrReceiptCandidates::default(),
            Vec::new(),
        ).is_err());
    }

    #[test]
    fn malformed_adapter_document_hash_is_rejected() {
        let request = sample_request();
        assert!(OcrExtraction::from_adapter_output(
            &request,
            "not-a-hash",
            provenance(),
            "text",
            Vec::new(),
            OcrReceiptCandidates::default(),
            Vec::new(),
        ).is_err());
    }

    #[test]
    fn raw_text_and_region_count_are_bounded() {
        let request = sample_request();
        let too_long = "x".repeat(OCR_MAX_RAW_TEXT_BYTES + 1);
        assert!(OcrExtraction::from_adapter_output(
            &request, request.document().sha256(), provenance(), too_long,
            Vec::new(), OcrReceiptCandidates::default(), Vec::new(),
        ).is_err());

        let region = OcrTextRegion::new(0, 0, 1, 1, "x", None).unwrap();
        let too_many = vec![region; OCR_MAX_REGIONS + 1];
        assert!(OcrExtraction::from_adapter_output(
            &request, request.document().sha256(), provenance(), "text",
            too_many, OcrReceiptCandidates::default(), Vec::new(),
        ).is_err());
    }

    #[test]
    fn warning_count_and_detail_are_bounded() {
        assert!(OcrWarning::new(OcrWarningCode::EngineNotice, Some("   ".into())).is_err());
        let request = sample_request();
        let warning = OcrWarning::new(OcrWarningCode::LowConfidence, None).unwrap();
        let warnings = vec![warning; OCR_MAX_WARNINGS + 1];
        assert!(OcrExtraction::from_adapter_output(
            &request, request.document().sha256(), provenance(), "text",
            Vec::new(), OcrReceiptCandidates::default(), warnings,
        ).is_err());
    }

    #[test]
    fn missing_facts_remain_missing_instead_of_being_inferred() {
        let request = sample_request();
        let extraction = OcrExtraction::from_adapter_output(
            &request, request.document().sha256(), provenance(), "unreadable",
            Vec::new(), OcrReceiptCandidates::default(),
            vec![OcrWarning::new(OcrWarningCode::MissingTotal, None).unwrap()],
        ).unwrap();
        assert!(extraction.candidates().merchant_text().is_none());
        assert!(extraction.candidates().document_date().is_none());
        assert!(extraction.candidates().total().is_none());
        assert!(extraction.candidates().currency().is_none());
        assert!(extraction.candidates().reference().is_none());
    }

    #[test]
    fn factual_candidates_can_be_populated_without_accounting_semantics() {
        let confidence = Some(OcrConfidenceBps::new(9_900).unwrap());
        let candidates = OcrReceiptCandidates::new(
            Some(OcrTextCandidate::new("North Pier", confidence).unwrap()),
            Some(OcrDateCandidate::new(Date::new(2026, 4, 4).unwrap(), confidence)),
            Some(OcrAmountCandidate::new(2_849, confidence).unwrap()),
            Some(OcrCurrencyCandidate::new("GBP", confidence).unwrap()),
            Some(OcrTextCandidate::new("NP-260404-1842", confidence).unwrap()),
        );
        assert_eq!(candidates.total().unwrap().total_pence(), 2_849);
        assert_eq!(candidates.document_date().unwrap().value().iso(), "2026-04-04");
    }

    #[test]
    fn unavailable_and_failed_are_explicit_noncompleted_outcomes() {
        let unavailable = OcrOutcome::Unavailable(OcrUnavailableReason::UnsupportedPlatform);
        let failed = OcrOutcome::Failed(OcrFailureKind::Timeout);
        assert!(matches!(unavailable, OcrOutcome::Unavailable(_)));
        assert!(matches!(failed, OcrOutcome::Failed(_)));
    }
}
