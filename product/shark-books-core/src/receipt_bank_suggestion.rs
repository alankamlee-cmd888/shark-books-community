//! SBC-6C deterministic receipt-to-bank suggestion contracts.
//!
//! This module composes factual OCR output with canonical SBC-3 bank lines. It
//! produces inspectable suggestions only. It does not post accounting entries,
//! clear or reconcile bank records, decide tax treatment, or persist anything.
//! A user must explicitly confirm any accepted receipt-to-bank association.

use crate::bank_import::BankLine;
use crate::matching::MatchLevel;
use crate::ocr::OcrExtraction;
use crate::{Date, RecordId};
use std::cmp::Ordering;
use std::fmt;

pub type ReceiptSuggestionResult<T> = Result<T, ReceiptSuggestionError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptSuggestionError {
    InvalidActor(String),
    InvalidDecision(String),
}
impl fmt::Display for ReceiptSuggestionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActor(s) | Self::InvalidDecision(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for ReceiptSuggestionError {}

/// Stable identity for one imported bank-line candidate. This is deliberately
/// separate from SBC-3 duplicate classification: similar-looking lines remain
/// distinct candidates unless SBC-3 has independently established a duplicate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BankLineIdentity {
    source_account_id: RecordId,
    source_file_sha256: String,
    source_locator: String,
    raw_record_sha256: String,
}
impl BankLineIdentity {
    #[must_use]
    pub fn for_line(line: &BankLine) -> Self {
        Self {
            source_account_id: line.source_account_id().clone(),
            source_file_sha256: line.source_file_sha256().to_owned(),
            source_locator: line.source_locator().to_owned(),
            raw_record_sha256: line.raw_record_sha256().to_owned(),
        }
    }
    #[must_use] pub fn source_account_id(&self) -> &RecordId { &self.source_account_id }
    #[must_use] pub fn source_file_sha256(&self) -> &str { &self.source_file_sha256 }
    #[must_use] pub fn source_locator(&self) -> &str { &self.source_locator }
    #[must_use] pub fn raw_record_sha256(&self) -> &str { &self.raw_record_sha256 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReceiptMatchReason {
    AmountExact,
    DateExact,
    DateWithinThreeDays,
    DateWithinSevenDays,
    CurrencyExact,
    ReferenceExact,
    MerchantPayeeStrong,
    MerchantDescriptionStrong,
    MultipleTopCandidates,
    MissingTotal,
    MissingDate,
    MissingCurrency,
    BankLineNotOutflow,
    BankAmountUnrepresentable,
    AmountMismatch,
    CurrencyMismatch,
    DateTooFar,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptBankCandidate {
    bank_line: BankLineIdentity,
    level: MatchLevel,
    score: u16,
    reasons: Vec<ReceiptMatchReason>,
    requires_user_confirmation: bool,
}
impl ReceiptBankCandidate {
    #[must_use] pub fn bank_line(&self) -> &BankLineIdentity { &self.bank_line }
    #[must_use] pub const fn level(&self) -> MatchLevel { self.level }
    #[must_use] pub const fn score(&self) -> u16 { self.score }
    #[must_use] pub fn reasons(&self) -> &[ReceiptMatchReason] { &self.reasons }
    #[must_use] pub const fn requires_user_confirmation(&self) -> bool { self.requires_user_confirmation }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptBankSuggestionSet {
    document_id: RecordId,
    candidates: Vec<ReceiptBankCandidate>,
    recommended_bank_line: Option<BankLineIdentity>,
    ambiguous_top: bool,
    requires_user_confirmation: bool,
}
impl ReceiptBankSuggestionSet {
    #[must_use] pub fn document_id(&self) -> &RecordId { &self.document_id }
    #[must_use] pub fn candidates(&self) -> &[ReceiptBankCandidate] { &self.candidates }
    #[must_use] pub fn recommended_bank_line(&self) -> Option<&BankLineIdentity> { self.recommended_bank_line.as_ref() }
    #[must_use] pub const fn ambiguous_top(&self) -> bool { self.ambiguous_top }
    #[must_use] pub const fn requires_user_confirmation(&self) -> bool { self.requires_user_confirmation }
}

#[must_use]
pub fn evaluate_receipt_bank_candidate(
    extraction: &OcrExtraction,
    bank_line: &BankLine,
) -> ReceiptBankCandidate {
    let identity = BankLineIdentity::for_line(bank_line);
    let facts = extraction.candidates();
    let mut reasons = Vec::new();
    let mut score = 0u16;

    let Some(total) = facts.total() else {
        reasons.push(ReceiptMatchReason::MissingTotal);
        return unmatched(identity, reasons);
    };

    if bank_line.signed_amount_minor() >= 0 {
        reasons.push(ReceiptMatchReason::BankLineNotOutflow);
        return unmatched(identity, reasons);
    }
    let Some(bank_magnitude) = bank_line.signed_amount_minor().checked_abs() else {
        reasons.push(ReceiptMatchReason::BankAmountUnrepresentable);
        return unmatched(identity, reasons);
    };
    if bank_magnitude != total.total_pence() {
        reasons.push(ReceiptMatchReason::AmountMismatch);
        return unmatched(identity, reasons);
    }
    reasons.push(ReceiptMatchReason::AmountExact);
    score += 40;

    let currency_exact = match facts.currency() {
        Some(currency) if currency.code() == bank_line.currency_code() => {
            reasons.push(ReceiptMatchReason::CurrencyExact);
            score += 10;
            true
        }
        Some(_) => {
            reasons.push(ReceiptMatchReason::CurrencyMismatch);
            return unmatched(identity, reasons);
        }
        None => {
            reasons.push(ReceiptMatchReason::MissingCurrency);
            false
        }
    };

    let date_gap = facts.document_date().map(|receipt_date| {
        date_distance_days(receipt_date.value(), bank_line.posted_date())
    });
    match date_gap {
        Some(0) => {
            reasons.push(ReceiptMatchReason::DateExact);
            score += 20;
        }
        Some(1..=3) => {
            reasons.push(ReceiptMatchReason::DateWithinThreeDays);
            score += 15;
        }
        Some(4..=7) => {
            reasons.push(ReceiptMatchReason::DateWithinSevenDays);
            score += 8;
        }
        Some(_) => {
            reasons.push(ReceiptMatchReason::DateTooFar);
            return unmatched(identity, reasons);
        }
        None => reasons.push(ReceiptMatchReason::MissingDate),
    }

    let reference_exact = facts.reference().is_some_and(|receipt_reference| {
        text_equal(receipt_reference.value(), bank_line.reference())
    });
    if reference_exact {
        reasons.push(ReceiptMatchReason::ReferenceExact);
        score += 25;
    }

    let merchant_payee_strong = facts.merchant_text().is_some_and(|merchant| {
        bank_line.payee().is_some_and(|payee| strong_text_support(merchant.value(), payee))
    });
    if merchant_payee_strong {
        reasons.push(ReceiptMatchReason::MerchantPayeeStrong);
        score += 15;
    }

    let merchant_description_strong = facts.merchant_text().is_some_and(|merchant| {
        strong_text_support(merchant.value(), bank_line.description())
    });
    if merchant_description_strong {
        reasons.push(ReceiptMatchReason::MerchantDescriptionStrong);
        score += 10;
    }

    let has_support = reference_exact || merchant_payee_strong || merchant_description_strong;
    let level = if currency_exact
        && date_gap == Some(0)
        && (reference_exact || (merchant_payee_strong && merchant_description_strong))
    {
        MatchLevel::Exact
    } else if date_gap.is_some_and(|gap| gap <= 3) && has_support {
        MatchLevel::Likely
    } else if date_gap.is_some_and(|gap| gap <= 7) || (date_gap.is_none() && has_support) {
        MatchLevel::Possible
    } else {
        reasons.push(ReceiptMatchReason::InsufficientEvidence);
        MatchLevel::Unmatched
    };

    ReceiptBankCandidate {
        bank_line: identity,
        level,
        score,
        reasons,
        requires_user_confirmation: level != MatchLevel::Unmatched,
    }
}

#[must_use]
pub fn rank_receipt_bank_suggestions(
    extraction: &OcrExtraction,
    bank_lines: &[BankLine],
) -> ReceiptBankSuggestionSet {
    let mut evaluated: Vec<_> = bank_lines
        .iter()
        .map(|line| evaluate_receipt_bank_candidate(extraction, line))
        .collect();
    evaluated.sort_by(compare_candidates);

    let exact_count = evaluated.iter().filter(|candidate| candidate.level == MatchLevel::Exact).count();
    let mut forced_ambiguous = false;
    if exact_count > 1 {
        for candidate in evaluated.iter_mut().filter(|candidate| candidate.level == MatchLevel::Exact) {
            candidate.level = MatchLevel::Likely;
            candidate.reasons.push(ReceiptMatchReason::MultipleTopCandidates);
        }
        evaluated.sort_by(compare_candidates);
        forced_ambiguous = true;
    }

    let top_level = evaluated.first().map_or(MatchLevel::Unmatched, |candidate| candidate.level);
    let top_score = evaluated.first().map_or(0, |candidate| candidate.score);
    let top_count = evaluated
        .iter()
        .take_while(|candidate| candidate.level == top_level && candidate.score == top_score)
        .count();
    let tied_ambiguous = top_level != MatchLevel::Unmatched && top_count > 1;
    if tied_ambiguous && !forced_ambiguous {
        for candidate in evaluated.iter_mut().take(top_count) {
            candidate.reasons.push(ReceiptMatchReason::MultipleTopCandidates);
        }
    }

    let ambiguous_top = forced_ambiguous || tied_ambiguous;
    let recommended_bank_line = if ambiguous_top {
        None
    } else {
        evaluated
            .first()
            .filter(|candidate| candidate.level != MatchLevel::Unmatched)
            .map(|candidate| candidate.bank_line.clone())
    };
    let requires_user_confirmation = evaluated.iter().any(|candidate| candidate.level != MatchLevel::Unmatched);

    ReceiptBankSuggestionSet {
        document_id: extraction.document().document_id().clone(),
        candidates: evaluated,
        recommended_bank_line,
        ambiguous_top,
        requires_user_confirmation,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptBankDecisionKind {
    Confirmed { bank_line: BankLineIdentity },
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptBankDecision {
    document_id: RecordId,
    actor: String,
    kind: ReceiptBankDecisionKind,
}
impl ReceiptBankDecision {
    #[must_use] pub fn document_id(&self) -> &RecordId { &self.document_id }
    #[must_use] pub fn actor(&self) -> &str { &self.actor }
    #[must_use] pub fn kind(&self) -> &ReceiptBankDecisionKind { &self.kind }
}

pub fn confirm_receipt_bank_suggestion(
    suggestions: &ReceiptBankSuggestionSet,
    chosen_bank_line: &BankLineIdentity,
    actor: impl Into<String>,
) -> ReceiptSuggestionResult<ReceiptBankDecision> {
    let candidate = suggestions
        .candidates()
        .iter()
        .find(|candidate| candidate.bank_line() == chosen_bank_line)
        .ok_or_else(|| ReceiptSuggestionError::InvalidDecision(
            "chosen bank line is not present in this receipt suggestion set".into(),
        ))?;
    if candidate.level() == MatchLevel::Unmatched {
        return Err(ReceiptSuggestionError::InvalidDecision(
            "an unmatched receipt-to-bank candidate cannot be confirmed".into(),
        ));
    }
    Ok(ReceiptBankDecision {
        document_id: suggestions.document_id.clone(),
        actor: valid_actor(actor.into())?,
        kind: ReceiptBankDecisionKind::Confirmed { bank_line: chosen_bank_line.clone() },
    })
}

pub fn reject_receipt_bank_suggestions(
    suggestions: &ReceiptBankSuggestionSet,
    actor: impl Into<String>,
) -> ReceiptSuggestionResult<ReceiptBankDecision> {
    Ok(ReceiptBankDecision {
        document_id: suggestions.document_id.clone(),
        actor: valid_actor(actor.into())?,
        kind: ReceiptBankDecisionKind::Rejected,
    })
}

fn valid_actor(value: String) -> ReceiptSuggestionResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(ReceiptSuggestionError::InvalidActor(
            "actor must be 1-128 non-whitespace characters".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

fn unmatched(bank_line: BankLineIdentity, reasons: Vec<ReceiptMatchReason>) -> ReceiptBankCandidate {
    ReceiptBankCandidate {
        bank_line,
        level: MatchLevel::Unmatched,
        score: 0,
        reasons,
        requires_user_confirmation: false,
    }
}

fn compare_candidates(left: &ReceiptBankCandidate, right: &ReceiptBankCandidate) -> Ordering {
    right
        .level
        .cmp(&left.level)
        .then_with(|| right.score.cmp(&left.score))
        .then_with(|| compare_identity(&left.bank_line, &right.bank_line))
}

fn compare_identity(left: &BankLineIdentity, right: &BankLineIdentity) -> Ordering {
    left.source_account_id
        .as_str()
        .cmp(right.source_account_id.as_str())
        .then_with(|| left.source_file_sha256.cmp(&right.source_file_sha256))
        .then_with(|| left.source_locator.cmp(&right.source_locator))
        .then_with(|| left.raw_record_sha256.cmp(&right.raw_record_sha256))
}

fn text_equal(receipt: &str, bank: Option<&str>) -> bool {
    bank.is_some_and(|bank| {
        let left = normalize_text(receipt);
        !left.is_empty() && left == normalize_text(bank)
    })
}

fn normalize_text(value: &str) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_space = false;
        } else if !last_space && !out.is_empty() {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_owned()
}

fn strong_text_support(left: &str, right: &str) -> bool {
    let left = normalize_text(left);
    let right = normalize_text(right);
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if left == right {
        return true;
    }
    let left_tokens: Vec<&str> = left.split_whitespace().filter(|token| token.len() >= 3).collect();
    let right_tokens: Vec<&str> = right.split_whitespace().filter(|token| token.len() >= 3).collect();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }
    let common = left_tokens.iter().filter(|token| right_tokens.contains(token)).count();
    let shorter = left_tokens.len().min(right_tokens.len());
    common >= 2 && common * 2 >= shorter
}

fn date_distance_days(left: Date, right: Date) -> u64 {
    days_from_civil(left).abs_diff(days_from_civil(right))
}

fn days_from_civil(date: Date) -> i64 {
    let mut year = i64::from(date.year());
    let month = i64::from(date.month());
    let day = i64::from(date.day());
    if month <= 2 {
        year -= 1;
    }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank_import::BankSourceFormat;
    use crate::documents::{DocumentReference, StorageProvider, StorageRootDescriptor};
    use crate::ocr::{
        OcrAmountCandidate, OcrCurrencyCandidate, OcrDateCandidate, OcrEngineProvenance,
        OcrReceiptCandidates, OcrRequest, OcrTextCandidate,
    };
    use crate::{SourceKind, SourceProvenance};

    fn id(value: &str) -> RecordId { RecordId::new(value).unwrap() }
    fn date(day: u8) -> Date { Date::new(2026, 9, day).unwrap() }
    fn hash(ch: char) -> String { std::iter::repeat_n(ch, 64).collect() }

    fn extraction(
        total: Option<i64>,
        day: Option<u8>,
        currency: Option<&str>,
        merchant: Option<&str>,
        reference: Option<&str>,
    ) -> OcrExtraction {
        let root = StorageRootDescriptor::new(id("root-1"), StorageProvider::LocalFilesystem, "Receipts").unwrap();
        let document = DocumentReference::from_bytes(
            id("receipt-1"), id("expense-1"), &root, "2026/receipt.png", "receipt.png",
            Some("image/png".into()), b"receipt bytes",
        ).unwrap();
        let request = OcrRequest::for_receipt_document(id("ocr-request-1"), &document);
        let facts = OcrReceiptCandidates::new(
            merchant.map(|value| OcrTextCandidate::new(value, None).unwrap()),
            day.map(|value| OcrDateCandidate::new(date(value), None)),
            total.map(|value| OcrAmountCandidate::new(value, None).unwrap()),
            currency.map(|value| OcrCurrencyCandidate::new(value, None).unwrap()),
            reference.map(|value| OcrTextCandidate::new(value, None).unwrap()),
        );
        OcrExtraction::from_adapter_output(
            &request,
            request.document().sha256(),
            OcrEngineProvenance::new("test-ocr", "1", "test-runtime", vec!["test-model".into()]).unwrap(),
            "receipt text",
            Vec::new(),
            facts,
            Vec::new(),
        ).unwrap()
    }

    fn bank_line(
        locator: &str,
        day: u8,
        amount: i64,
        payee: Option<&str>,
        reference: Option<&str>,
        description: &str,
    ) -> BankLine {
        let raw_marker = locator.chars().last().unwrap_or('f');
        BankLine::new(
            id("bank-main"), None, BankSourceFormat::Csv, hash('a'), locator, date(day), None, amount,
            description, payee.map(str::to_owned), reference.map(str::to_owned), None, hash(raw_marker),
            SourceProvenance::new(SourceKind::Csv, Some(locator.into()), Some(hash(raw_marker)), None).unwrap(),
        ).unwrap()
    }

    #[test]
    fn unique_exact_reference_date_currency_match_is_suggested() {
        let receipt = extraction(Some(2_849), Some(8), Some("GBP"), Some("North Pier"), Some("NP-1842"));
        let line = bank_line("row:2", 8, -2_849, Some("North Pier"), Some("np 1842"), "North Pier card purchase");
        let set = rank_receipt_bank_suggestions(&receipt, &[line]);
        assert_eq!(set.candidates()[0].level(), MatchLevel::Exact);
        assert!(set.recommended_bank_line().is_some());
        assert!(set.requires_user_confirmation());
        assert!(!set.ambiguous_top());
    }

    #[test]
    fn near_date_with_strong_merchant_is_likely() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), Some("Adobe Creative Cloud"), None);
        let line = bank_line("row:3", 10, -1_500, Some("Adobe Creative Cloud"), None, "Adobe Creative Cloud monthly");
        assert_eq!(evaluate_receipt_bank_candidate(&receipt, &line).level(), MatchLevel::Likely);
    }

    #[test]
    fn exact_amount_and_date_without_text_support_is_possible() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let line = bank_line("row:4", 8, -1_500, None, None, "Card payment");
        assert_eq!(evaluate_receipt_bank_candidate(&receipt, &line).level(), MatchLevel::Possible);
    }

    #[test]
    fn amount_mismatch_is_unmatched() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), Some("Adobe"), None);
        let line = bank_line("row:5", 8, -1_501, Some("Adobe"), None, "Adobe");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Unmatched);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::AmountMismatch));
    }

    #[test]
    fn incoming_bank_line_is_not_a_purchase_receipt_match() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), Some("Adobe"), None);
        let line = bank_line("row:6", 8, 1_500, Some("Adobe"), None, "Adobe");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Unmatched);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::BankLineNotOutflow));
    }

    #[test]
    fn date_beyond_seven_days_is_unmatched() {
        let receipt = extraction(Some(1_500), Some(1), Some("GBP"), Some("Adobe Creative Cloud"), None);
        let line = bank_line("row:7", 9, -1_500, Some("Adobe Creative Cloud"), None, "Adobe Creative Cloud");
        assert_eq!(evaluate_receipt_bank_candidate(&receipt, &line).level(), MatchLevel::Unmatched);
    }

    #[test]
    fn missing_total_cannot_produce_a_match() {
        let receipt = extraction(None, Some(8), Some("GBP"), Some("Adobe Creative Cloud"), None);
        let line = bank_line("row:8", 8, -1_500, Some("Adobe Creative Cloud"), None, "Adobe Creative Cloud");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Unmatched);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::MissingTotal));
    }

    #[test]
    fn missing_date_with_strong_merchant_is_only_possible() {
        let receipt = extraction(Some(1_500), None, Some("GBP"), Some("Adobe Creative Cloud"), None);
        let line = bank_line("row:9", 8, -1_500, Some("Adobe Creative Cloud"), None, "Adobe Creative Cloud monthly");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Possible);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::MissingDate));
    }

    #[test]
    fn reference_normalisation_is_deterministic() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, Some("INV-100 / A"));
        let line = bank_line("row:a", 8, -1_500, None, Some("inv 100 a"), "Card payment");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::ReferenceExact));
        assert_eq!(candidate.level(), MatchLevel::Exact);
    }

    #[test]
    fn multiple_exact_candidates_are_ambiguous_and_not_called_exact() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, Some("R-100"));
        let first = bank_line("row:b", 8, -1_500, None, Some("R-100"), "Card payment one");
        let second = bank_line("row:c", 8, -1_500, None, Some("R-100"), "Card payment two");
        let set = rank_receipt_bank_suggestions(&receipt, &[first, second]);
        assert!(set.ambiguous_top());
        assert!(set.recommended_bank_line().is_none());
        assert!(set.candidates().iter().all(|candidate| candidate.level() != MatchLevel::Exact));
    }

    #[test]
    fn equal_top_candidates_sort_stably_and_remain_ambiguous() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let second = bank_line("row:e", 8, -1_500, None, None, "Card payment");
        let first = bank_line("row:d", 8, -1_500, None, None, "Card payment");
        let set = rank_receipt_bank_suggestions(&receipt, &[second, first]);
        assert!(set.ambiguous_top());
        assert_eq!(set.candidates()[0].bank_line().source_locator(), "row:d");
        assert_eq!(set.candidates()[1].bank_line().source_locator(), "row:e");
    }

    #[test]
    fn duplicate_looking_lines_are_not_auto_suppressed_by_receipt_matching() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let first = bank_line("row:f", 8, -1_500, None, None, "Same card payment");
        let second = bank_line("row:0", 8, -1_500, None, None, "Same card payment");
        let set = rank_receipt_bank_suggestions(&receipt, &[first, second]);
        assert_eq!(set.candidates().len(), 2);
        assert!(set.ambiguous_top());
        assert_ne!(set.candidates()[0].bank_line(), set.candidates()[1].bank_line());
    }

    #[test]
    fn explicit_confirmation_returns_decision_only() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, Some("R-1"));
        let line = bank_line("row:1", 8, -1_500, None, Some("R-1"), "Card payment");
        let set = rank_receipt_bank_suggestions(&receipt, &[line]);
        let decision = confirm_receipt_bank_suggestion(&set, set.recommended_bank_line().unwrap(), " owner ").unwrap();
        assert_eq!(decision.actor(), "owner");
        assert!(matches!(decision.kind(), ReceiptBankDecisionKind::Confirmed { .. }));
    }

    #[test]
    fn unmatched_candidate_cannot_be_confirmed() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let line = bank_line("row:2", 8, -2_000, None, None, "Card payment");
        let set = rank_receipt_bank_suggestions(&receipt, &[line]);
        let identity = set.candidates()[0].bank_line().clone();
        assert!(confirm_receipt_bank_suggestion(&set, &identity, "owner").is_err());
    }

    #[test]
    fn rejecting_is_explicit_and_selects_no_bank_line() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let line = bank_line("row:3", 8, -1_500, None, None, "Card payment");
        let set = rank_receipt_bank_suggestions(&receipt, &[line]);
        let decision = reject_receipt_bank_suggestions(&set, "owner").unwrap();
        assert_eq!(decision.document_id(), receipt.document().document_id());
        assert_eq!(decision.kind(), &ReceiptBankDecisionKind::Rejected);
    }

    #[test]
    fn currency_mismatch_fails_closed() {
        let receipt = extraction(Some(1_500), Some(8), Some("USD"), Some("Adobe"), None);
        let line = bank_line("row:4", 8, -1_500, Some("Adobe"), None, "Adobe");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Unmatched);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::CurrencyMismatch));
    }

    #[test]
    fn missing_currency_is_not_inferred_as_exact() {
        let receipt = extraction(Some(1_500), Some(8), None, None, Some("R-5"));
        let line = bank_line("row:5", 8, -1_500, None, Some("R-5"), "Card payment");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Likely);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::MissingCurrency));
    }

    #[test]
    fn repeated_inputs_produce_identical_ranked_output() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), Some("North Pier"), None);
        let lines = vec![
            bank_line("row:6", 8, -1_500, Some("North Pier"), None, "North Pier"),
            bank_line("row:7", 10, -1_500, Some("North Pier"), None, "North Pier"),
        ];
        assert_eq!(rank_receipt_bank_suggestions(&receipt, &lines), rank_receipt_bank_suggestions(&receipt, &lines));
    }

    #[test]
    fn no_bank_lines_produces_no_suggestion() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), Some("North Pier"), None);
        let set = rank_receipt_bank_suggestions(&receipt, &[]);
        assert!(set.candidates().is_empty());
        assert!(set.recommended_bank_line().is_none());
        assert!(!set.requires_user_confirmation());
    }

    #[test]
    fn minimum_i64_bank_amount_fails_closed_without_overflow() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let line = bank_line("row:8", 8, i64::MIN, None, None, "Malformed magnitude");
        let candidate = evaluate_receipt_bank_candidate(&receipt, &line);
        assert_eq!(candidate.level(), MatchLevel::Unmatched);
        assert!(candidate.reasons().contains(&ReceiptMatchReason::BankAmountUnrepresentable));
    }

    #[test]
    fn invalid_actor_is_rejected() {
        let receipt = extraction(Some(1_500), Some(8), Some("GBP"), None, None);
        let set = rank_receipt_bank_suggestions(&receipt, &[]);
        assert!(reject_receipt_bank_suggestions(&set, "   ").is_err());
    }
}
