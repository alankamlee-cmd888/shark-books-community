use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Empty,
    TooLong,
    ControlCharacter,
    InvalidValue(&'static str),
    Duplicate(&'static str),
    Overflow,
    InconsistentEvidence(&'static str),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "value must not be empty"),
            Self::TooLong => write!(f, "value is too long"),
            Self::ControlCharacter => write!(f, "control characters are not allowed"),
            Self::InvalidValue(message) => write!(f, "invalid value: {message}"),
            Self::Duplicate(message) => write!(f, "duplicate: {message}"),
            Self::Overflow => write!(f, "checked arithmetic overflow"),
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
pub struct PositiveQuantity(u64);

impl PositiveQuantity {
    pub fn new(value: u64) -> DomainResult<Self> {
        if value == 0 {
            return Err(DomainError::InvalidValue("quantity must be positive"));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StockBalance(i128);

impl StockBalance {
    pub const ZERO: Self = Self(0);

    pub fn from_signed(value: i128) -> Self {
        Self(value)
    }

    pub fn get(self) -> i128 {
        self.0
    }

    pub fn checked_add(self, delta: i128) -> DomainResult<Self> {
        self.0
            .checked_add(delta)
            .map(Self)
            .ok_or(DomainError::Overflow)
    }
}

pub fn quantity_to_i128(quantity: PositiveQuantity) -> i128 {
    i128::from(quantity.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_ids_and_text_fail_closed() {
        assert!(EntityId::new(" ").is_err());
        assert!(EntityId::new("a".repeat(129)).is_err());
        assert!(BoundedText::new("bad\ntext", 20).is_err());
    }

    #[test]
    fn positive_quantity_rejects_zero() {
        assert!(PositiveQuantity::new(0).is_err());
        assert_eq!(PositiveQuantity::new(7).unwrap().get(), 7);
    }

    #[test]
    fn signed_balance_preserves_negative_values() {
        let balance = StockBalance::ZERO.checked_add(-5).unwrap();
        assert_eq!(balance.get(), -5);
    }
}
