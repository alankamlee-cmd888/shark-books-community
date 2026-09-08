//! SBC-4 deterministic matching and reconciliation contracts.
//!
//! Duplicate identity remains an SBC-3 concern. This module evaluates possible
//! ledger matches, always requires user confirmation before clearance, and only
//! permits statement reconciliation when the statement balance difference is
//! exactly zero. It is platform-neutral and persistence-free.

use crate::bank_import::BankLine;
use crate::{Date, RecordId, DEFAULT_CURRENCY_CODE};
use std::cmp::Ordering;
use std::fmt;

pub type MatchingResult<T> = Result<T, MatchingError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchingError {
    InvalidCandidate(String),
    InvalidActor(String),
    InvalidTransition(String),
    InvalidReconciliation(String),
    ArithmeticOverflow,
}
impl fmt::Display for MatchingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCandidate(s)
            | Self::InvalidActor(s)
            | Self::InvalidTransition(s)
            | Self::InvalidReconciliation(s) => f.write_str(s),
            Self::ArithmeticOverflow => f.write_str("integer arithmetic overflow"),
        }
    }
}
impl std::error::Error for MatchingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchLevel {
    Unmatched,
    Possible,
    Likely,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchReason {
    AccountExact,
    CurrencyExact,
    AmountExact,
    DateExact,
    DateWithinThreeDays,
    DateWithinSevenDays,
    SourceIdentityExact,
    ReferenceExact,
    DescriptionStrong,
    PayeeStrong,
    MultipleTopCandidates,
    AmountMismatch,
    AccountMismatch,
    CurrencyMismatch,
    DateTooFar,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceState {
    Uncleared,
    Cleared,
    Reconciled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerCandidate {
    id: RecordId,
    source_account_id: RecordId,
    posted_date: Date,
    signed_amount_minor: i64,
    currency_code: &'static str,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    source_identity: Option<String>,
    clearance_state: ClearanceState,
}
impl LedgerCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: RecordId,
        source_account_id: RecordId,
        posted_date: Date,
        signed_amount_minor: i64,
        description: impl Into<String>,
        payee: Option<String>,
        reference: Option<String>,
        source_identity: Option<String>,
        clearance_state: ClearanceState,
    ) -> MatchingResult<Self> {
        if signed_amount_minor == 0 {
            return Err(MatchingError::InvalidCandidate(
                "ledger candidate amount must be non-zero whole pence".into(),
            ));
        }
        let description = nonblank(description.into(), "description")?;
        let payee = optional_nonblank(payee, "payee")?;
        let reference = optional_nonblank(reference, "reference")?;
        let source_identity = optional_nonblank(source_identity, "source identity")?;
        Ok(Self {
            id,
            source_account_id,
            posted_date,
            signed_amount_minor,
            currency_code: DEFAULT_CURRENCY_CODE,
            description,
            payee,
            reference,
            source_identity,
            clearance_state,
        })
    }

    #[must_use] pub fn id(&self) -> &RecordId { &self.id }
    #[must_use] pub fn source_account_id(&self) -> &RecordId { &self.source_account_id }
    #[must_use] pub const fn posted_date(&self) -> Date { self.posted_date }
    #[must_use] pub const fn signed_amount_minor(&self) -> i64 { self.signed_amount_minor }
    #[must_use] pub const fn currency_code(&self) -> &'static str { self.currency_code }
    #[must_use] pub fn description(&self) -> &str { &self.description }
    #[must_use] pub fn payee(&self) -> Option<&str> { self.payee.as_deref() }
    #[must_use] pub fn reference(&self) -> Option<&str> { self.reference.as_deref() }
    #[must_use] pub fn source_identity(&self) -> Option<&str> { self.source_identity.as_deref() }
    #[must_use] pub const fn clearance_state(&self) -> ClearanceState { self.clearance_state }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchCandidate {
    candidate_id: RecordId,
    level: MatchLevel,
    score: u16,
    reasons: Vec<MatchReason>,
    requires_user_confirmation: bool,
}
impl MatchCandidate {
    #[must_use] pub fn candidate_id(&self) -> &RecordId { &self.candidate_id }
    #[must_use] pub const fn level(&self) -> MatchLevel { self.level }
    #[must_use] pub const fn score(&self) -> u16 { self.score }
    #[must_use] pub fn reasons(&self) -> &[MatchReason] { &self.reasons }
    #[must_use] pub const fn requires_user_confirmation(&self) -> bool { self.requires_user_confirmation }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSet {
    candidates: Vec<MatchCandidate>,
    recommended_candidate_id: Option<RecordId>,
    ambiguous_top: bool,
    requires_user_confirmation: bool,
}
impl MatchSet {
    #[must_use] pub fn candidates(&self) -> &[MatchCandidate] { &self.candidates }
    #[must_use] pub fn recommended_candidate_id(&self) -> Option<&RecordId> { self.recommended_candidate_id.as_ref() }
    #[must_use] pub const fn ambiguous_top(&self) -> bool { self.ambiguous_top }
    #[must_use] pub const fn requires_user_confirmation(&self) -> bool { self.requires_user_confirmation }
}

#[must_use]
pub fn evaluate_match(bank_line: &BankLine, candidate: &LedgerCandidate) -> MatchCandidate {
    let mut reasons = Vec::new();
    let mut score = 0u16;

    if bank_line.source_account_id() != candidate.source_account_id() {
        reasons.push(MatchReason::AccountMismatch);
        return unmatched(candidate, reasons);
    }
    reasons.push(MatchReason::AccountExact);
    score += 20;

    if bank_line.currency_code() != candidate.currency_code() {
        reasons.push(MatchReason::CurrencyMismatch);
        return unmatched(candidate, reasons);
    }
    reasons.push(MatchReason::CurrencyExact);
    score += 15;

    if bank_line.signed_amount_minor() != candidate.signed_amount_minor() {
        reasons.push(MatchReason::AmountMismatch);
        return unmatched(candidate, reasons);
    }
    reasons.push(MatchReason::AmountExact);
    score += 30;

    let date_gap = date_distance_days(bank_line.posted_date(), candidate.posted_date());
    match date_gap {
        0 => { reasons.push(MatchReason::DateExact); score += 15; }
        1..=3 => { reasons.push(MatchReason::DateWithinThreeDays); score += 10; }
        4..=7 => { reasons.push(MatchReason::DateWithinSevenDays); score += 5; }
        _ => {
            reasons.push(MatchReason::DateTooFar);
            return unmatched(candidate, reasons);
        }
    }

    let source_identity_exact = bank_line
        .strong_identity_key()
        .zip(candidate.source_identity())
        .is_some_and(|(left, right)| normalize_text(&left) == normalize_text(right));
    if source_identity_exact {
        reasons.push(MatchReason::SourceIdentityExact);
        score += 30;
    }

    let reference_exact = option_text_equal(bank_line.reference(), candidate.reference());
    if reference_exact {
        reasons.push(MatchReason::ReferenceExact);
        score += 20;
    }

    let description_strong = strong_text_support(bank_line.description(), candidate.description());
    if description_strong {
        reasons.push(MatchReason::DescriptionStrong);
        score += 10;
    }

    let payee_strong = option_strong_text(bank_line.payee(), candidate.payee());
    if payee_strong {
        reasons.push(MatchReason::PayeeStrong);
        score += 5;
    }

    let level = if date_gap == 0 && (source_identity_exact || reference_exact) {
        MatchLevel::Exact
    } else if date_gap <= 3 && (source_identity_exact || reference_exact || description_strong || payee_strong) {
        MatchLevel::Likely
    } else if date_gap <= 7 && (description_strong || payee_strong || reference_exact || source_identity_exact || date_gap == 0) {
        MatchLevel::Possible
    } else {
        reasons.push(MatchReason::InsufficientEvidence);
        MatchLevel::Unmatched
    };

    MatchCandidate {
        candidate_id: candidate.id.clone(),
        level,
        score,
        reasons,
        requires_user_confirmation: level != MatchLevel::Unmatched,
    }
}

#[must_use]
pub fn rank_matches(bank_line: &BankLine, candidates: &[LedgerCandidate]) -> MatchSet {
    let mut evaluated: Vec<_> = candidates.iter().map(|c| evaluate_match(bank_line, c)).collect();
    evaluated.sort_by(compare_match_candidates);

    let top_level = evaluated.first().map_or(MatchLevel::Unmatched, MatchCandidate::level);
    let top_score = evaluated.first().map_or(0, MatchCandidate::score);
    let top_count = evaluated
        .iter()
        .take_while(|m| m.level == top_level && m.score == top_score)
        .count();
    let mut ambiguous_top = top_level != MatchLevel::Unmatched && top_count > 1;

    // EXACT means one unique, high-specificity candidate. Multiple equal EXACT
    // candidates are deliberately downgraded to LIKELY and flagged ambiguous.
    if top_level == MatchLevel::Exact && top_count > 1 {
        for candidate in evaluated.iter_mut().take(top_count) {
            candidate.level = MatchLevel::Likely;
            candidate.reasons.push(MatchReason::MultipleTopCandidates);
        }
        ambiguous_top = true;
    } else if ambiguous_top {
        for candidate in evaluated.iter_mut().take(top_count) {
            candidate.reasons.push(MatchReason::MultipleTopCandidates);
        }
    }

    let recommended_candidate_id = if !ambiguous_top {
        evaluated
            .first()
            .filter(|m| m.level != MatchLevel::Unmatched)
            .map(|m| m.candidate_id.clone())
    } else {
        None
    };
    let requires_user_confirmation = evaluated.iter().any(|m| m.level != MatchLevel::Unmatched);

    MatchSet { candidates: evaluated, recommended_candidate_id, ambiguous_top, requires_user_confirmation }
}

fn compare_match_candidates(a: &MatchCandidate, b: &MatchCandidate) -> Ordering {
    b.level
        .cmp(&a.level)
        .then_with(|| b.score.cmp(&a.score))
        .then_with(|| a.candidate_id.as_str().cmp(b.candidate_id.as_str()))
}

fn unmatched(candidate: &LedgerCandidate, reasons: Vec<MatchReason>) -> MatchCandidate {
    MatchCandidate {
        candidate_id: candidate.id.clone(),
        level: MatchLevel::Unmatched,
        score: 0,
        reasons,
        requires_user_confirmation: false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditReason {
    MatchConfirmed,
    StatementReconciled { statement_id: RecordId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearanceTransition {
    record_id: RecordId,
    from: ClearanceState,
    to: ClearanceState,
    actor: String,
    reason: AuditReason,
}
impl ClearanceTransition {
    #[must_use] pub fn record_id(&self) -> &RecordId { &self.record_id }
    #[must_use] pub const fn from(&self) -> ClearanceState { self.from }
    #[must_use] pub const fn to(&self) -> ClearanceState { self.to }
    #[must_use] pub fn actor(&self) -> &str { &self.actor }
    #[must_use] pub fn reason(&self) -> &AuditReason { &self.reason }
}

pub fn confirm_match(
    candidate: &LedgerCandidate,
    assessment: &MatchCandidate,
    actor: impl Into<String>,
) -> MatchingResult<ClearanceTransition> {
    if candidate.id() != assessment.candidate_id() {
        return Err(MatchingError::InvalidTransition(
            "match assessment does not refer to candidate".into(),
        ));
    }
    if assessment.level() == MatchLevel::Unmatched {
        return Err(MatchingError::InvalidTransition(
            "an unmatched candidate cannot be confirmed".into(),
        ));
    }
    if candidate.clearance_state() != ClearanceState::Uncleared {
        return Err(MatchingError::InvalidTransition(
            "match confirmation requires an uncleared candidate".into(),
        ));
    }
    let actor = valid_actor(actor.into())?;
    Ok(ClearanceTransition {
        record_id: candidate.id.clone(),
        from: ClearanceState::Uncleared,
        to: ClearanceState::Cleared,
        actor,
        reason: AuditReason::MatchConfirmed,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationEntry {
    record_id: RecordId,
    signed_amount_minor: i64,
    state: ClearanceState,
}
impl ReconciliationEntry {
    pub fn new(record_id: RecordId, signed_amount_minor: i64, state: ClearanceState) -> MatchingResult<Self> {
        if signed_amount_minor == 0 {
            return Err(MatchingError::InvalidReconciliation(
                "reconciliation entry amount must be non-zero whole pence".into(),
            ));
        }
        Ok(Self { record_id, signed_amount_minor, state })
    }
    #[must_use] pub fn record_id(&self) -> &RecordId { &self.record_id }
    #[must_use] pub const fn signed_amount_minor(&self) -> i64 { self.signed_amount_minor }
    #[must_use] pub const fn state(&self) -> ClearanceState { self.state }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationCheck {
    computed_ending_balance_minor: i64,
    expected_ending_balance_minor: i64,
    difference_minor: i64,
    all_entries_cleared: bool,
}
impl ReconciliationCheck {
    #[must_use] pub const fn computed_ending_balance_minor(&self) -> i64 { self.computed_ending_balance_minor }
    #[must_use] pub const fn expected_ending_balance_minor(&self) -> i64 { self.expected_ending_balance_minor }
    #[must_use] pub const fn difference_minor(&self) -> i64 { self.difference_minor }
    #[must_use] pub const fn all_entries_cleared(&self) -> bool { self.all_entries_cleared }
    #[must_use] pub const fn can_finalize(&self) -> bool { self.difference_minor == 0 && self.all_entries_cleared }
}

pub fn preview_reconciliation(
    opening_balance_minor: i64,
    ending_balance_minor: i64,
    entries: &[ReconciliationEntry],
) -> MatchingResult<ReconciliationCheck> {
    if entries.is_empty() {
        return Err(MatchingError::InvalidReconciliation(
            "reconciliation requires at least one selected entry".into(),
        ));
    }
    let mut computed = i128::from(opening_balance_minor);
    for entry in entries {
        computed = computed
            .checked_add(i128::from(entry.signed_amount_minor()))
            .ok_or(MatchingError::ArithmeticOverflow)?;
    }
    let computed = i64::try_from(computed).map_err(|_| MatchingError::ArithmeticOverflow)?;
    let difference = i128::from(ending_balance_minor)
        .checked_sub(i128::from(computed))
        .ok_or(MatchingError::ArithmeticOverflow)?;
    let difference = i64::try_from(difference).map_err(|_| MatchingError::ArithmeticOverflow)?;
    Ok(ReconciliationCheck {
        computed_ending_balance_minor: computed,
        expected_ending_balance_minor: ending_balance_minor,
        difference_minor: difference,
        all_entries_cleared: entries.iter().all(|e| e.state() == ClearanceState::Cleared),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationResult {
    statement_id: RecordId,
    statement_date: Date,
    check: ReconciliationCheck,
    transitions: Vec<ClearanceTransition>,
}
impl ReconciliationResult {
    #[must_use] pub fn statement_id(&self) -> &RecordId { &self.statement_id }
    #[must_use] pub const fn statement_date(&self) -> Date { self.statement_date }
    #[must_use] pub fn check(&self) -> &ReconciliationCheck { &self.check }
    #[must_use] pub fn transitions(&self) -> &[ClearanceTransition] { &self.transitions }
}

pub fn finalize_reconciliation(
    statement_id: RecordId,
    statement_date: Date,
    opening_balance_minor: i64,
    ending_balance_minor: i64,
    entries: &[ReconciliationEntry],
    actor: impl Into<String>,
) -> MatchingResult<ReconciliationResult> {
    let actor = valid_actor(actor.into())?;
    let check = preview_reconciliation(opening_balance_minor, ending_balance_minor, entries)?;
    if !check.all_entries_cleared() {
        return Err(MatchingError::InvalidReconciliation(
            "only cleared, unreconciled entries may be finalized".into(),
        ));
    }
    if check.difference_minor() != 0 {
        return Err(MatchingError::InvalidReconciliation(format!(
            "statement difference must be zero; difference={} pence",
            check.difference_minor()
        )));
    }
    let transitions = entries
        .iter()
        .map(|entry| ClearanceTransition {
            record_id: entry.record_id.clone(),
            from: ClearanceState::Cleared,
            to: ClearanceState::Reconciled,
            actor: actor.clone(),
            reason: AuditReason::StatementReconciled { statement_id: statement_id.clone() },
        })
        .collect();
    Ok(ReconciliationResult { statement_id, statement_date, check, transitions })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionPlan {
    original_record_id: RecordId,
    reversal_record_id: RecordId,
    replacement_record_id: Option<RecordId>,
    actor: String,
    reason: String,
}
impl CorrectionPlan {
    pub fn new(
        original_record_id: RecordId,
        reversal_record_id: RecordId,
        replacement_record_id: Option<RecordId>,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> MatchingResult<Self> {
        if original_record_id == reversal_record_id
            || replacement_record_id.as_ref().is_some_and(|id| id == &original_record_id || id == &reversal_record_id)
        {
            return Err(MatchingError::InvalidTransition(
                "correction must use new record IDs; history is not rewritten in place".into(),
            ));
        }
        let actor = valid_actor(actor.into())?;
        let reason = nonblank(reason.into(), "correction reason")?;
        Ok(Self { original_record_id, reversal_record_id, replacement_record_id, actor, reason })
    }
    #[must_use] pub fn original_record_id(&self) -> &RecordId { &self.original_record_id }
    #[must_use] pub fn reversal_record_id(&self) -> &RecordId { &self.reversal_record_id }
    #[must_use] pub fn replacement_record_id(&self) -> Option<&RecordId> { self.replacement_record_id.as_ref() }
    #[must_use] pub fn actor(&self) -> &str { &self.actor }
    #[must_use] pub fn reason(&self) -> &str { &self.reason }
}

fn valid_actor(value: String) -> MatchingResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(MatchingError::InvalidActor(
            "actor must be 1-128 non-whitespace characters".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn nonblank(value: String, field: &str) -> MatchingResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(MatchingError::InvalidCandidate(format!(
            "{field} must be 1-512 non-whitespace characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn optional_nonblank(value: Option<String>, field: &str) -> MatchingResult<Option<String>> {
    value.map(|v| nonblank(v, field)).transpose()
}

fn option_text_equal(left: Option<&str>, right: Option<&str>) -> bool {
    left.zip(right)
        .is_some_and(|(a, b)| normalize_text(a) == normalize_text(b) && !normalize_text(a).is_empty())
}

fn option_strong_text(left: Option<&str>, right: Option<&str>) -> bool {
    left.zip(right).is_some_and(|(a, b)| strong_text_support(a, b))
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
    out.trim().to_string()
}

fn strong_text_support(left: &str, right: &str) -> bool {
    let a = normalize_text(left);
    let b = normalize_text(right);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let a_tokens: Vec<&str> = a.split_whitespace().filter(|t| t.len() >= 3).collect();
    let b_tokens: Vec<&str> = b.split_whitespace().filter(|t| t.len() >= 3).collect();
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return false;
    }
    let common = a_tokens.iter().filter(|t| b_tokens.contains(t)).count();
    let shorter = a_tokens.len().min(b_tokens.len());
    common >= 2 && common * 2 >= shorter
}

fn date_distance_days(left: Date, right: Date) -> u64 {
    days_from_civil(left).abs_diff(days_from_civil(right))
}

// Howard Hinnant-style civil-date serialisation, implemented locally to keep
// the product core dependency-free. Only differences are used.
fn days_from_civil(date: Date) -> i64 {
    let mut year = i64::from(date.year());
    let month = i64::from(date.month());
    let day = i64::from(date.day());
    if month <= 2 { year -= 1; }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank_import::{BankSourceFormat, DuplicateCertainty};
    use crate::{SourceKind, SourceProvenance};

    fn id(value: &str) -> RecordId { RecordId::new(value).unwrap() }
    fn date(day: u8) -> Date { Date::new(2026, 9, day).unwrap() }
    fn hash(ch: char) -> String { std::iter::repeat_n(ch, 64).collect() }
    fn line(day: u8, amount: i64, reference: Option<&str>, txid: Option<&str>, description: &str) -> BankLine {
        BankLine::new(
            id("bank-main"),
            None,
            BankSourceFormat::Csv,
            hash('a'),
            "row:2",
            date(day),
            None,
            amount,
            description,
            Some("Acme Ltd".into()),
            reference.map(str::to_string),
            txid.map(str::to_string),
            hash('b'),
            SourceProvenance::new(SourceKind::Csv, Some("statement.csv:2".into()), Some(hash('b')), None).unwrap(),
        ).unwrap()
    }
    fn candidate(id_value: &str, day: u8, amount: i64, reference: Option<&str>, source_identity: Option<String>, description: &str, state: ClearanceState) -> LedgerCandidate {
        LedgerCandidate::new(
            id(id_value), id("bank-main"), date(day), amount, description,
            Some("Acme Ltd".into()), reference.map(str::to_string), source_identity, state,
        ).unwrap()
    }

    #[test]
    fn exact_unique_reference_match() {
        let b = line(8, -12_345, Some("INV-100"), None, "Acme subscription");
        let c = candidate("c1", 8, -12_345, Some("inv 100"), None, "Acme subscription", ClearanceState::Uncleared);
        let set = rank_matches(&b, &[c]);
        assert_eq!(set.candidates()[0].level(), MatchLevel::Exact);
        assert_eq!(set.recommended_candidate_id().unwrap().as_str(), "c1");
        assert!(set.requires_user_confirmation());
        assert!(!set.ambiguous_top());
    }

    #[test]
    fn exact_source_identity_match() {
        let b = line(8, 5_000, None, Some("TX-9"), "Customer receipt");
        let key = b.strong_identity_key().unwrap();
        let c = candidate("c1", 8, 5_000, None, Some(key), "Customer receipt", ClearanceState::Uncleared);
        let assessment = evaluate_match(&b, &c);
        assert_eq!(assessment.level(), MatchLevel::Exact);
        assert!(assessment.reasons().contains(&MatchReason::SourceIdentityExact));
    }

    #[test]
    fn multiple_exact_candidates_are_not_called_exact() {
        let b = line(8, -1_000, Some("REF-X"), None, "Office rent");
        let a = candidate("a", 8, -1_000, Some("REF-X"), None, "Office rent", ClearanceState::Uncleared);
        let c = candidate("c", 8, -1_000, Some("REF-X"), None, "Office rent", ClearanceState::Uncleared);
        let set = rank_matches(&b, &[a, c]);
        assert!(set.ambiguous_top());
        assert!(set.recommended_candidate_id().is_none());
        assert_eq!(set.candidates()[0].level(), MatchLevel::Likely);
        assert!(set.candidates()[0].reasons().contains(&MatchReason::MultipleTopCandidates));
    }

    #[test]
    fn likely_near_date_with_strong_text() {
        let b = line(8, -2_500, None, None, "Adobe Creative Cloud monthly");
        let c = candidate("c1", 10, -2_500, None, None, "Adobe Creative Cloud", ClearanceState::Uncleared);
        assert_eq!(evaluate_match(&b, &c).level(), MatchLevel::Likely);
    }

    #[test]
    fn possible_exact_date_without_reference_or_text_support() {
        let b = line(8, -2_500, None, None, "Card payment");
        let c = candidate("c1", 8, -2_500, None, None, "Unlabelled purchase", ClearanceState::Uncleared);
        assert_eq!(evaluate_match(&b, &c).level(), MatchLevel::Possible);
    }

    #[test]
    fn amount_mismatch_is_unmatched() {
        let b = line(8, -2_500, None, None, "Adobe");
        let c = candidate("c1", 8, -2_501, None, None, "Adobe", ClearanceState::Uncleared);
        let a = evaluate_match(&b, &c);
        assert_eq!(a.level(), MatchLevel::Unmatched);
        assert!(a.reasons().contains(&MatchReason::AmountMismatch));
        assert!(!a.requires_user_confirmation());
    }

    #[test]
    fn date_beyond_seven_days_is_unmatched() {
        let b = line(1, -2_500, None, None, "Adobe Creative Cloud");
        let c = candidate("c1", 9, -2_500, None, None, "Adobe Creative Cloud", ClearanceState::Uncleared);
        assert_eq!(evaluate_match(&b, &c).level(), MatchLevel::Unmatched);
    }

    #[test]
    fn leap_day_date_distance_is_calendar_correct() {
        let left = Date::new(2028, 2, 28).unwrap();
        let right = Date::new(2028, 3, 1).unwrap();
        assert_eq!(date_distance_days(left, right), 2);
    }

    #[test]
    fn heuristic_duplicate_identity_does_not_become_automatic_match() {
        let a = line(8, -1_000, None, None, "Same payment");
        let b = BankLine::new(
            id("bank-main"), None, BankSourceFormat::Csv, hash('c'), "row:9", date(8), None,
            -1_000, "Same payment", Some("Acme Ltd".into()), None, None, hash('d'),
            SourceProvenance::new(SourceKind::Csv, Some("other.csv:9".into()), Some(hash('d')), None).unwrap(),
        ).unwrap();
        assert_eq!(a.duplicate_certainty(&b).unwrap(), DuplicateCertainty::Heuristic);
        assert!(!DuplicateCertainty::Heuristic.may_auto_suppress());
    }

    #[test]
    fn confirm_match_moves_only_uncleared_to_cleared() {
        let b = line(8, -1_000, Some("R1"), None, "Rent");
        let c = candidate("c1", 8, -1_000, Some("R1"), None, "Rent", ClearanceState::Uncleared);
        let assessment = evaluate_match(&b, &c);
        let transition = confirm_match(&c, &assessment, "user").unwrap();
        assert_eq!(transition.from(), ClearanceState::Uncleared);
        assert_eq!(transition.to(), ClearanceState::Cleared);
        assert_eq!(transition.reason(), &AuditReason::MatchConfirmed);
    }

    #[test]
    fn unmatched_candidate_cannot_be_confirmed() {
        let b = line(8, -1_000, None, None, "Rent");
        let c = candidate("c1", 8, -2_000, None, None, "Rent", ClearanceState::Uncleared);
        let assessment = evaluate_match(&b, &c);
        assert!(confirm_match(&c, &assessment, "user").is_err());
    }

    #[test]
    fn already_cleared_candidate_cannot_be_reconfirmed() {
        let b = line(8, -1_000, Some("R1"), None, "Rent");
        let c = candidate("c1", 8, -1_000, Some("R1"), None, "Rent", ClearanceState::Cleared);
        let assessment = evaluate_match(&b, &c);
        assert!(confirm_match(&c, &assessment, "user").is_err());
    }

    #[test]
    fn reconciliation_zero_difference_finalizes_cleared_entries() {
        let entries = vec![
            ReconciliationEntry::new(id("e1"), 10_000, ClearanceState::Cleared).unwrap(),
            ReconciliationEntry::new(id("e2"), -2_500, ClearanceState::Cleared).unwrap(),
        ];
        let result = finalize_reconciliation(id("stmt-1"), date(30), 50_000, 57_500, &entries, "user").unwrap();
        assert_eq!(result.check().difference_minor(), 0);
        assert_eq!(result.transitions().len(), 2);
        assert!(result.transitions().iter().all(|t| t.from() == ClearanceState::Cleared && t.to() == ClearanceState::Reconciled));
    }

    #[test]
    fn nonzero_reconciliation_difference_fails_closed() {
        let entries = vec![ReconciliationEntry::new(id("e1"), 10_000, ClearanceState::Cleared).unwrap()];
        let check = preview_reconciliation(50_000, 59_999, &entries).unwrap();
        assert_eq!(check.difference_minor(), -1);
        assert!(!check.can_finalize());
        assert!(finalize_reconciliation(id("stmt"), date(30), 50_000, 59_999, &entries, "user").is_err());
    }

    #[test]
    fn uncleared_entry_cannot_be_reconciled() {
        let entries = vec![ReconciliationEntry::new(id("e1"), 10_000, ClearanceState::Uncleared).unwrap()];
        assert!(finalize_reconciliation(id("stmt"), date(30), 50_000, 60_000, &entries, "user").is_err());
    }

    #[test]
    fn already_reconciled_entry_cannot_be_reconciled_again() {
        let entries = vec![ReconciliationEntry::new(id("e1"), 10_000, ClearanceState::Reconciled).unwrap()];
        assert!(finalize_reconciliation(id("stmt"), date(30), 50_000, 60_000, &entries, "user").is_err());
    }

    #[test]
    fn correction_requires_new_record_ids_and_preserves_original_reference() {
        let plan = CorrectionPlan::new(id("orig"), id("reversal"), Some(id("replacement")), "user", "Fix wrong category").unwrap();
        assert_eq!(plan.original_record_id().as_str(), "orig");
        assert_eq!(plan.reversal_record_id().as_str(), "reversal");
        assert_eq!(plan.replacement_record_id().unwrap().as_str(), "replacement");
        assert!(CorrectionPlan::new(id("orig"), id("orig"), None, "user", "bad").is_err());
    }

    #[test]
    fn blank_actor_is_rejected() {
        let b = line(8, -1_000, Some("R1"), None, "Rent");
        let c = candidate("c1", 8, -1_000, Some("R1"), None, "Rent", ClearanceState::Uncleared);
        let assessment = evaluate_match(&b, &c);
        assert!(confirm_match(&c, &assessment, "   ").is_err());
    }
}
