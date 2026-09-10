use shark_books_core::bank_import::{BankLine, BankSourceFormat};
use shark_books_core::documents::{DocumentReference, StorageProvider, StorageRootDescriptor};
use shark_books_core::matching::MatchLevel;
use shark_books_core::ocr::{
    OcrAmountCandidate, OcrCurrencyCandidate, OcrDateCandidate, OcrEngineProvenance,
    OcrExtraction, OcrReceiptCandidates, OcrRequest, OcrTextCandidate,
};
use shark_books_core::receipt_bank_suggestion::{
    confirm_receipt_bank_suggestion, rank_receipt_bank_suggestions,
    reject_receipt_bank_suggestions, ReceiptBankDecisionKind,
};
use shark_books_core::{Date, RecordId, SourceKind, SourceProvenance};

fn id(value: &str) -> RecordId {
    RecordId::new(value).unwrap()
}

fn hash(ch: char) -> String {
    std::iter::repeat_n(ch, 64).collect()
}

fn sample_document() -> DocumentReference {
    let root = StorageRootDescriptor::new(
        id("root-receipts"),
        StorageProvider::LocalFilesystem,
        "User receipts",
    )
    .unwrap();
    DocumentReference::from_bytes(
        id("receipt-doc-1"),
        id("expense-placeholder-1"),
        &root,
        "2026/04/north-pier.png",
        "north-pier.png",
        Some("image/png".into()),
        b"bounded receipt fixture",
    )
    .unwrap()
}

fn extraction(
    total: Option<i64>,
    date: Option<Date>,
    currency: Option<&str>,
    merchant: Option<&str>,
    reference: Option<&str>,
) -> OcrExtraction {
    let document = sample_document();
    let request = OcrRequest::for_receipt_document(id("ocr-request-1"), &document);
    let provenance = OcrEngineProvenance::new(
        "shark-direct-onnx",
        "1",
        "onnxruntime-1.23.2-cpu",
        vec!["PP-OCRv6_tiny_det".into(), "PP-OCRv6_tiny_rec".into()],
    )
    .unwrap();
    let candidates = OcrReceiptCandidates::new(
        merchant.map(|v| OcrTextCandidate::new(v, None).unwrap()),
        date.map(|v| OcrDateCandidate::new(v, None)),
        total.map(|v| OcrAmountCandidate::new(v, None).unwrap()),
        currency.map(|v| OcrCurrencyCandidate::new(v, None).unwrap()),
        reference.map(|v| OcrTextCandidate::new(v, None).unwrap()),
    );
    OcrExtraction::from_adapter_output(
        &request,
        request.document().sha256(),
        provenance,
        "NORTH PIER\nDATE 04/04/2026\nTOTAL GBP 28.49\nREF NP-260404-1842",
        Vec::new(),
        candidates,
        Vec::new(),
    )
    .unwrap()
}

fn bank_line(locator: &str, raw_hash_char: char, amount: i64, day: u8) -> BankLine {
    BankLine::new(
        id("bank-main"),
        None,
        BankSourceFormat::Csv,
        hash('a'),
        locator,
        Date::new(2026, 4, day).unwrap(),
        None,
        amount,
        "NORTH PIER CAFE CARD PURCHASE",
        Some("North Pier".into()),
        Some("NP-260404-1842".into()),
        None,
        hash(raw_hash_char),
        SourceProvenance::new(
            SourceKind::Csv,
            Some(format!("statement.csv:{locator}")),
            Some(hash(raw_hash_char)),
            None,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn factual_ocr_to_unique_suggestion_still_requires_explicit_confirmation() {
    let extraction = extraction(
        Some(2_849),
        Some(Date::new(2026, 4, 4).unwrap()),
        Some("GBP"),
        Some("North Pier"),
        Some("NP-260404-1842"),
    );
    let line = bank_line("row-4", 'b', -2_849, 4);
    let suggestions = rank_receipt_bank_suggestions(&extraction, &[line]);

    assert!(!suggestions.ambiguous_top());
    assert!(suggestions.requires_user_confirmation());
    let chosen = suggestions.recommended_bank_line().unwrap().clone();
    assert_eq!(suggestions.candidates()[0].level(), MatchLevel::Exact);

    let decision = confirm_receipt_bank_suggestion(&suggestions, &chosen, "owner").unwrap();
    assert_eq!(decision.document_id(), extraction.document().document_id());
    assert_eq!(decision.actor(), "owner");
    assert!(matches!(
        decision.kind(),
        ReceiptBankDecisionKind::Confirmed { bank_line } if bank_line == &chosen
    ));
}

#[test]
fn ambiguous_equal_candidates_never_become_an_implicit_recommendation() {
    let extraction = extraction(
        Some(2_849),
        Some(Date::new(2026, 4, 4).unwrap()),
        Some("GBP"),
        Some("North Pier"),
        Some("NP-260404-1842"),
    );
    let first = bank_line("row-4", 'b', -2_849, 4);
    let second = bank_line("row-5", 'c', -2_849, 4);
    let suggestions = rank_receipt_bank_suggestions(&extraction, &[first, second]);

    assert!(suggestions.ambiguous_top());
    assert!(suggestions.recommended_bank_line().is_none());
    assert!(suggestions.requires_user_confirmation());
    assert!(suggestions
        .candidates()
        .iter()
        .take(2)
        .all(|candidate| candidate.level() != MatchLevel::Exact));
}

#[test]
fn penny_mismatch_fails_closed_and_cannot_be_confirmed() {
    let extraction = extraction(
        Some(2_849),
        Some(Date::new(2026, 4, 4).unwrap()),
        Some("GBP"),
        Some("North Pier"),
        Some("NP-260404-1842"),
    );
    let line = bank_line("row-4", 'b', -2_850, 4);
    let identity = shark_books_core::receipt_bank_suggestion::BankLineIdentity::for_line(&line);
    let suggestions = rank_receipt_bank_suggestions(&extraction, &[line]);

    assert!(suggestions.recommended_bank_line().is_none());
    assert_eq!(suggestions.candidates()[0].level(), MatchLevel::Unmatched);
    assert!(confirm_receipt_bank_suggestion(&suggestions, &identity, "owner").is_err());
}

#[test]
fn missing_total_remains_missing_and_produces_no_match() {
    let extraction = extraction(
        None,
        Some(Date::new(2026, 4, 4).unwrap()),
        Some("GBP"),
        Some("North Pier"),
        Some("NP-260404-1842"),
    );
    let line = bank_line("row-4", 'b', -2_849, 4);
    let suggestions = rank_receipt_bank_suggestions(&extraction, &[line]);

    assert!(suggestions.recommended_bank_line().is_none());
    assert!(!suggestions.requires_user_confirmation());
    assert_eq!(suggestions.candidates()[0].level(), MatchLevel::Unmatched);
}

#[test]
fn wrong_document_integrity_cannot_create_factual_extraction() {
    let document = sample_document();
    let request = OcrRequest::for_receipt_document(id("ocr-request-integrity"), &document);
    let provenance = OcrEngineProvenance::new(
        "shark-direct-onnx",
        "1",
        "onnxruntime-1.23.2-cpu",
        vec!["PP-OCRv6_tiny_det".into(), "PP-OCRv6_tiny_rec".into()],
    )
    .unwrap();
    let candidates = OcrReceiptCandidates::new(
        Some(OcrTextCandidate::new("North Pier", None).unwrap()),
        Some(OcrDateCandidate::new(Date::new(2026, 4, 4).unwrap(), None)),
        Some(OcrAmountCandidate::new(2_849, None).unwrap()),
        Some(OcrCurrencyCandidate::new("GBP", None).unwrap()),
        None,
    );

    assert!(OcrExtraction::from_adapter_output(
        &request,
        "00".repeat(32),
        provenance,
        "TOTAL GBP 28.49",
        Vec::new(),
        candidates,
        Vec::new(),
    )
    .is_err());
}

#[test]
fn rejection_is_explicit_and_non_destructive() {
    let extraction = extraction(
        Some(2_849),
        Some(Date::new(2026, 4, 4).unwrap()),
        Some("GBP"),
        Some("North Pier"),
        Some("NP-260404-1842"),
    );
    let line = bank_line("row-4", 'b', -2_849, 4);
    let suggestions = rank_receipt_bank_suggestions(&extraction, &[line]);
    let before = format!("{suggestions:?}");

    let decision = reject_receipt_bank_suggestions(&suggestions, "owner").unwrap();
    assert!(matches!(decision.kind(), ReceiptBankDecisionKind::Rejected));
    assert_eq!(before, format!("{suggestions:?}"));
}
