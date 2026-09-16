use crate::date::CivilDate;
use crate::primitives::{BoundedText, DomainError, DomainResult};

pub const MAX_GENERATED_OCCURRENCES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceFrequency {
    OneOff,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthlyPolicy {
    SameDayClamped,
    MonthEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceSpec {
    start: CivilDate,
    frequency: RecurrenceFrequency,
    interval: u16,
    end_date: Option<CivilDate>,
    occurrence_count: Option<u32>,
    timezone_id: BoundedText,
    monthly_policy: MonthlyPolicy,
}

impl RecurrenceSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start: CivilDate,
        frequency: RecurrenceFrequency,
        interval: u16,
        end_date: Option<CivilDate>,
        occurrence_count: Option<u32>,
        timezone_id: BoundedText,
        monthly_policy: MonthlyPolicy,
    ) -> DomainResult<Self> {
        if interval == 0 || interval > 366 {
            return Err(DomainError::InvalidValue("recurrence interval out of range"));
        }
        if let Some(end) = end_date {
            if end < start {
                return Err(DomainError::InvalidValue("recurrence end precedes start"));
            }
        }
        if let Some(count) = occurrence_count {
            if count == 0 || count > 10_000 {
                return Err(DomainError::InvalidValue("occurrence count out of range"));
            }
        }
        Ok(Self {
            start,
            frequency,
            interval,
            end_date,
            occurrence_count,
            timezone_id,
            monthly_policy,
        })
    }

    pub fn start(&self) -> CivilDate {
        self.start
    }

    pub fn frequency(&self) -> RecurrenceFrequency {
        self.frequency
    }

    pub fn interval(&self) -> u16 {
        self.interval
    }

    pub fn end_date(&self) -> Option<CivilDate> {
        self.end_date
    }

    pub fn occurrence_count(&self) -> Option<u32> {
        self.occurrence_count
    }

    pub fn timezone_id(&self) -> &BoundedText {
        &self.timezone_id
    }

    pub fn monthly_policy(&self) -> MonthlyPolicy {
        self.monthly_policy
    }

    pub fn generate(&self, max: usize) -> DomainResult<Vec<CivilDate>> {
        if max == 0 || max > MAX_GENERATED_OCCURRENCES {
            return Err(DomainError::LimitExceeded("recurrence preview cap must be 1..=512"));
        }

        let count_limit = self
            .occurrence_count
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX));
        let mut output = Vec::new();

        for index in 0..max {
            if count_limit.map(|limit| index >= limit).unwrap_or(false) {
                break;
            }
            let date = self.occurrence_at(index)?;
            if self.end_date.map(|end| date > end).unwrap_or(false) {
                break;
            }
            output.push(date);
            if self.frequency == RecurrenceFrequency::OneOff {
                break;
            }
        }
        Ok(output)
    }

    fn occurrence_at(&self, index: usize) -> DomainResult<CivilDate> {
        if self.frequency == RecurrenceFrequency::OneOff {
            return Ok(self.start);
        }
        if index == 0 {
            return Ok(match (self.frequency, self.monthly_policy) {
                (RecurrenceFrequency::Monthly, MonthlyPolicy::MonthEnd) => self.start.month_end(),
                _ => self.start,
            });
        }

        let index = u32::try_from(index).map_err(|_| DomainError::Overflow)?;
        let interval = u32::from(self.interval);
        match self.frequency {
            RecurrenceFrequency::OneOff => Ok(self.start),
            RecurrenceFrequency::Daily => {
                let days = index.checked_mul(interval).ok_or(DomainError::Overflow)?;
                self.start.add_days(days)
            }
            RecurrenceFrequency::Weekly => {
                let weeks = index.checked_mul(interval).ok_or(DomainError::Overflow)?;
                let days = weeks.checked_mul(7).ok_or(DomainError::Overflow)?;
                self.start.add_days(days)
            }
            RecurrenceFrequency::Monthly => {
                let months = index.checked_mul(interval).ok_or(DomainError::Overflow)?;
                let target = self.start.add_months_clamped(months)?;
                Ok(match self.monthly_policy {
                    MonthlyPolicy::SameDayClamped => target,
                    MonthlyPolicy::MonthEnd => target.month_end(),
                })
            }
            RecurrenceFrequency::Yearly => {
                let years = index.checked_mul(interval).ok_or(DomainError::Overflow)?;
                self.start.add_years_clamped(years)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> BoundedText {
        BoundedText::new("Europe/London", 64).unwrap()
    }

    #[test]
    fn recurrence_hard_cap_prevents_unbounded_generation() {
        let spec = RecurrenceSpec::new(
            CivilDate::new(2026, 1, 1).unwrap(),
            RecurrenceFrequency::Daily,
            1,
            None,
            None,
            zone(),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        assert_eq!(spec.generate(5).unwrap().len(), 5);
        assert!(spec.generate(MAX_GENERATED_OCCURRENCES + 1).is_err());
    }

    #[test]
    fn recurrence_count_and_end_date_are_both_enforced() {
        let spec = RecurrenceSpec::new(
            CivilDate::new(2026, 1, 1).unwrap(),
            RecurrenceFrequency::Daily,
            1,
            Some(CivilDate::new(2026, 1, 3).unwrap()),
            Some(10),
            zone(),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        assert_eq!(spec.generate(20).unwrap().len(), 3);
    }

    #[test]
    fn monthly_anchor_does_not_drift_after_short_month() {
        let spec = RecurrenceSpec::new(
            CivilDate::new(2026, 1, 31).unwrap(),
            RecurrenceFrequency::Monthly,
            1,
            None,
            Some(3),
            zone(),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        assert_eq!(
            spec.generate(10).unwrap(),
            vec![
                CivilDate::new(2026, 1, 31).unwrap(),
                CivilDate::new(2026, 2, 28).unwrap(),
                CivilDate::new(2026, 3, 31).unwrap(),
            ]
        );
    }

    #[test]
    fn explicit_month_end_policy_stays_at_month_end_including_first_occurrence() {
        let spec = RecurrenceSpec::new(
            CivilDate::new(2026, 1, 15).unwrap(),
            RecurrenceFrequency::Monthly,
            1,
            None,
            Some(3),
            zone(),
            MonthlyPolicy::MonthEnd,
        )
        .unwrap();
        assert_eq!(
            spec.generate(10).unwrap(),
            vec![
                CivilDate::new(2026, 1, 31).unwrap(),
                CivilDate::new(2026, 2, 28).unwrap(),
                CivilDate::new(2026, 3, 31).unwrap(),
            ]
        );
    }

    #[test]
    fn yearly_leap_day_is_clamped_without_float_or_timezone_guessing() {
        let spec = RecurrenceSpec::new(
            CivilDate::new(2028, 2, 29).unwrap(),
            RecurrenceFrequency::Yearly,
            1,
            None,
            Some(2),
            zone(),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        assert_eq!(spec.generate(10).unwrap()[1], CivilDate::new(2029, 2, 28).unwrap());
        assert_eq!(spec.timezone_id().as_str(), "Europe/London");
    }

    #[test]
    fn large_weekly_interval_remains_bounded_by_supported_date_range() {
        let spec = RecurrenceSpec::new(
            CivilDate::new(2026, 1, 1).unwrap(),
            RecurrenceFrequency::Weekly,
            366,
            None,
            None,
            zone(),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        let output = spec.generate(8).unwrap();
        assert_eq!(output.len(), 8);
        assert_eq!(output[0], CivilDate::new(2026, 1, 1).unwrap());
    }
}
