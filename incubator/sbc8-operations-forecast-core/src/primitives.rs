use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Empty,
    TooLong,
    ControlCharacter,
    InvalidValue(&'static str),
    InvalidState(&'static str),
    Overflow,
    Duplicate(&'static str),
    LimitExceeded(&'static str),
    InconsistentEvidence(&'static str),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "value must not be empty"),
            Self::TooLong => write!(f, "value is too long"),
            Self::ControlCharacter => write!(f, "control characters are not allowed"),
            Self::InvalidValue(message) => write!(f, "invalid value: {message}"),
            Self::InvalidState(message) => write!(f, "invalid state: {message}"),
            Self::Overflow => write!(f, "checked arithmetic overflow"),
            Self::Duplicate(message) => write!(f, "duplicate: {message}"),
            Self::LimitExceeded(message) => write!(f, "limit exceeded: {message}"),
            Self::InconsistentEvidence(message) => write!(f, "inconsistent evidence: {message}"),
        }
    }
}

impl std::error::Error for DomainError {}

pub type DomainResult<T> = Result<T, DomainError>;

fn validate_text(value: &str, max_bytes: usize) -> DomainResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Empty);
    }
    if trimmed.len() > max_bytes {
        return Err(DomainError::TooLong);
    }
    if trimmed.chars().any(char::is_control) {
        return Err(DomainError::ControlCharacter);
    }
    Ok(trimmed.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(String);

impl EntityId {
    pub fn new(value: impl AsRef<str>) -> DomainResult<Self> {
        Ok(Self(validate_text(value.as_ref(), 128)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedText(String);

impl BoundedText {
    pub fn new(value: impl AsRef<str>, max_bytes: usize) -> DomainResult<Self> {
        Ok(Self(validate_text(value.as_ref(), max_bytes)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Self = Self(0);

    pub fn from_minor(minor: i64) -> Self {
        Self(minor)
    }

    pub fn nonnegative(minor: i64) -> DomainResult<Self> {
        if minor < 0 {
            return Err(DomainError::InvalidValue("money must be nonnegative"));
        }
        Ok(Self(minor))
    }

    pub fn positive(minor: i64) -> DomainResult<Self> {
        if minor <= 0 {
            return Err(DomainError::InvalidValue("money must be positive"));
        }
        Ok(Self(minor))
    }

    pub fn minor(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> DomainResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(DomainError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> DomainResult<Self> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(DomainError::Overflow)
    }

    pub fn midpoint(low: Self, high: Self) -> DomainResult<Self> {
        if low > high {
            return Err(DomainError::InvalidValue("range low must not exceed high"));
        }
        let sum = i128::from(low.0) + i128::from(high.0);
        let midpoint = sum / 2;
        let midpoint = i64::try_from(midpoint).map_err(|_| DomainError::Overflow)?;
        Ok(Self(midpoint))
    }
}

pub fn checked_bill_for_minutes(rate_per_hour: Money, minutes: u64) -> DomainResult<Money> {
    if rate_per_hour.minor() < 0 {
        return Err(DomainError::InvalidValue("billing rate must be nonnegative"));
    }
    let numerator = i128::from(rate_per_hour.minor())
        .checked_mul(i128::from(minutes))
        .ok_or(DomainError::Overflow)?;
    let whole = numerator / 60;
    let remainder = numerator % 60;
    let rounded = if remainder * 2 >= 60 {
        whole.checked_add(1).ok_or(DomainError::Overflow)?
    } else {
        whole
    };
    let rounded = i64::try_from(rounded).map_err(|_| DomainError::Overflow)?;
    Ok(Money::from_minor(rounded))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalSourceKind {
    TimeEntry,
    Mileage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftCommercialLineProposal {
    id: EntityId,
    source_kind: ProposalSourceKind,
    source_id: EntityId,
    project_id: Option<EntityId>,
    description: BoundedText,
    amount: Money,
}

impl DraftCommercialLineProposal {
    pub(crate) fn new(
        id: EntityId,
        source_kind: ProposalSourceKind,
        source_id: EntityId,
        project_id: Option<EntityId>,
        description: BoundedText,
        amount: Money,
    ) -> DomainResult<Self> {
        if amount.minor() < 0 {
            return Err(DomainError::InvalidValue("draft commercial line amount must be nonnegative"));
        }
        Ok(Self {
            id,
            source_kind,
            source_id,
            project_id,
            description,
            amount,
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn source_kind(&self) -> ProposalSourceKind {
        self.source_kind
    }

    pub fn source_id(&self) -> &EntityId {
        &self.source_id
    }

    pub fn project_id(&self) -> Option<&EntityId> {
        self.project_id.as_ref()
    }

    pub fn description(&self) -> &BoundedText {
        &self.description
    }

    pub fn amount(&self) -> Money {
        self.amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    #[test]
    fn bounded_ids_and_text_fail_closed() {
        assert!(EntityId::new(" ").is_err());
        assert!(EntityId::new("a".repeat(129)).is_err());
        assert!(BoundedText::new("bad\ntext", 50).is_err());
    }

    #[test]
    fn checked_money_overflow_fails() {
        assert_eq!(Money::from_minor(7).checked_add(Money::from_minor(8)).unwrap().minor(), 15);
        assert!(Money::from_minor(i64::MAX).checked_add(Money::from_minor(1)).is_err());
    }

    #[test]
    fn time_billing_rounding_is_deterministic() {
        let rate = Money::nonnegative(1000).unwrap();
        assert_eq!(checked_bill_for_minutes(rate, 30).unwrap().minor(), 500);
        assert_eq!(checked_bill_for_minutes(rate, 1).unwrap().minor(), 17);
    }

    #[test]
    fn draft_commercial_line_proposal_rejects_negative_amount() {
        let result = DraftCommercialLineProposal::new(
            id("proposal"),
            ProposalSourceKind::TimeEntry,
            id("time"),
            None,
            BoundedText::new("work", 120).unwrap(),
            Money::from_minor(-1),
        );
        assert!(result.is_err());
    }
}
