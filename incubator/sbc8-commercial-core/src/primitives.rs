use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Invalid(String),
    InvalidTransition(String),
    Conflict(String),
    Overflow(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(value)
            | Self::InvalidTransition(value)
            | Self::Conflict(value)
            | Self::Overflow(value) => f.write_str(value),
        }
    }
}

impl std::error::Error for DomainError {}

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(String);

impl EntityId {
    pub fn new(value: impl Into<String>) -> DomainResult<Self> {
        let raw = value.into();
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.len() > 128
            || trimmed.chars().any(char::is_control)
        {
            return Err(DomainError::Invalid(
                "id must be 1-128 non-control characters".into(),
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommercialNumber(String);

impl CommercialNumber {
    pub fn new(value: impl Into<String>) -> DomainResult<Self> {
        let raw = value.into();
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.len() > 64
            || trimmed.chars().any(char::is_control)
        {
            return Err(DomainError::Invalid(
                "commercial number must be 1-64 non-control characters".into(),
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money(i64);

impl Money {
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn from_minor(minor: i64) -> Self {
        Self(minor)
    }

    pub fn positive(minor: i64) -> DomainResult<Self> {
        if minor <= 0 {
            return Err(DomainError::Invalid(
                "money amount must be positive minor units".into(),
            ));
        }
        Ok(Self(minor))
    }

    pub fn non_negative(minor: i64) -> DomainResult<Self> {
        if minor < 0 {
            return Err(DomainError::Invalid(
                "money amount must not be negative".into(),
            ));
        }
        Ok(Self(minor))
    }

    #[must_use]
    pub const fn minor(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> DomainResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| DomainError::Overflow("money addition overflow".into()))
    }

    pub fn checked_sub(self, other: Self) -> DomainResult<Self> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or_else(|| DomainError::Overflow("money subtraction overflow".into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(i64);

impl Quantity {
    pub fn positive(subunits: i64) -> DomainResult<Self> {
        if subunits <= 0 {
            return Err(DomainError::Invalid(
                "quantity must be positive integer subunits".into(),
            ));
        }
        Ok(Self(subunits))
    }

    #[must_use]
    pub const fn subunits(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> DomainResult<Self> {
        self.0
            .checked_add(other.0)
            .ok_or_else(|| DomainError::Overflow("quantity addition overflow".into()))
            .and_then(Self::positive)
    }
}

pub fn checked_line_total(quantity: Quantity, unit_price: Money) -> DomainResult<Money> {
    if unit_price.minor() < 0 {
        return Err(DomainError::Invalid(
            "commercial unit price must not be negative".into(),
        ));
    }
    let total = i128::from(quantity.subunits())
        .checked_mul(i128::from(unit_price.minor()))
        .ok_or_else(|| DomainError::Overflow("commercial line total overflow".into()))?;
    let total = i64::try_from(total)
        .map_err(|_| DomainError::Overflow("commercial line total overflow".into()))?;
    Ok(Money::from_minor(total))
}

pub fn bounded_text(value: impl Into<String>, label: &str, max: usize) -> DomainResult<String> {
    let raw = value.into();
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > max || trimmed.chars().any(char::is_control) {
        return Err(DomainError::Invalid(format!(
            "{label} must be 1-{max} non-control characters"
        )));
    }
    Ok(trimmed.to_owned())
}

pub fn optional_bounded_text(
    value: Option<impl Into<String>>,
    label: &str,
    max: usize,
) -> DomainResult<Option<String>> {
    value
        .map(|item| bounded_text(item, label, max))
        .transpose()
}

#[must_use]
pub fn canonical_reference(value: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            out.push(ch);
            pending_space = false;
        } else if !out.is_empty() {
            pending_space = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_ids_fail_closed() {
        assert!(EntityId::new("").is_err());
        assert!(EntityId::new("bad\nvalue").is_err());
        assert!(EntityId::new("x".repeat(129)).is_err());
        assert_eq!(EntityId::new("  abc-1  ").unwrap().as_str(), "abc-1");
    }

    #[test]
    fn checked_money_overflow_fails() {
        assert!(Money::from_minor(i64::MAX)
            .checked_add(Money::from_minor(1))
            .is_err());
        assert!(checked_line_total(
            Quantity::positive(i64::MAX).unwrap(),
            Money::from_minor(2)
        )
        .is_err());
    }

    #[test]
    fn canonical_reference_is_stable() {
        assert_eq!(canonical_reference(" INV-- 001 / A "), "inv 001 a");
    }
}
