use crate::primitives::{DomainError, DomainResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CivilDate {
    pub fn new(year: i32, month: u8, day: u8) -> DomainResult<Self> {
        if !(1..=9999).contains(&year) {
            return Err(DomainError::InvalidValue("year out of supported range"));
        }
        if !(1..=12).contains(&month) {
            return Err(DomainError::InvalidValue("month out of range"));
        }
        let max_day = days_in_month(year, month);
        if day == 0 || day > max_day {
            return Err(DomainError::InvalidValue("day out of range"));
        }
        Ok(Self { year, month, day })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    pub fn month_end(self) -> Self {
        Self {
            year: self.year,
            month: self.month,
            day: days_in_month(self.year, self.month),
        }
    }

    pub fn add_days(self, days: u32) -> DomainResult<Self> {
        let mut year = self.year;
        let mut month = self.month;
        let mut day = self.day;
        let mut remaining = days;

        while remaining > 0 {
            let max_day = days_in_month(year, month);
            let days_left_in_month = u32::from(max_day - day);
            if remaining <= days_left_in_month {
                day = day
                    .checked_add(u8::try_from(remaining).map_err(|_| DomainError::Overflow)?)
                    .ok_or(DomainError::Overflow)?;
                remaining = 0;
                continue;
            }

            remaining = remaining
                .checked_sub(days_left_in_month + 1)
                .ok_or(DomainError::Overflow)?;
            day = 1;
            if month < 12 {
                month += 1;
            } else {
                year = year.checked_add(1).ok_or(DomainError::Overflow)?;
                if year > 9999 {
                    return Err(DomainError::Overflow);
                }
                month = 1;
            }
        }

        Self::new(year, month, day)
    }

    pub fn add_months_clamped(self, months: u32) -> DomainResult<Self> {
        let start_index = i64::from(self.year)
            .checked_mul(12)
            .and_then(|value| value.checked_add(i64::from(self.month) - 1))
            .ok_or(DomainError::Overflow)?;
        let target = start_index
            .checked_add(i64::from(months))
            .ok_or(DomainError::Overflow)?;
        let year = i32::try_from(target / 12).map_err(|_| DomainError::Overflow)?;
        let month = u8::try_from((target % 12) + 1).map_err(|_| DomainError::Overflow)?;
        if !(1..=9999).contains(&year) {
            return Err(DomainError::Overflow);
        }
        let day = self.day.min(days_in_month(year, month));
        Self::new(year, month, day)
    }

    pub fn add_years_clamped(self, years: u32) -> DomainResult<Self> {
        let year = self
            .year
            .checked_add(i32::try_from(years).map_err(|_| DomainError::Overflow)?)
            .ok_or(DomainError::Overflow)?;
        if !(1..=9999).contains(&year) {
            return Err(DomainError::Overflow);
        }
        let day = self.day.min(days_in_month(year, self.month));
        Self::new(year, self.month, day)
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_civil_dates_fail_closed() {
        assert!(CivilDate::new(2026, 2, 29).is_err());
        assert!(CivilDate::new(2028, 2, 29).is_ok());
        assert!(CivilDate::new(2026, 13, 1).is_err());
    }

    #[test]
    fn leap_day_and_month_end_are_deterministic() {
        let leap = CivilDate::new(2028, 2, 29).unwrap();
        assert_eq!(leap.add_years_clamped(1).unwrap(), CivilDate::new(2029, 2, 28).unwrap());
        let jan = CivilDate::new(2026, 1, 31).unwrap();
        assert_eq!(jan.add_months_clamped(1).unwrap(), CivilDate::new(2026, 2, 28).unwrap());
    }

    #[test]
    fn add_days_crosses_month_and_year_boundaries() {
        let date = CivilDate::new(2026, 12, 31).unwrap();
        assert_eq!(date.add_days(1).unwrap(), CivilDate::new(2027, 1, 1).unwrap());
        let jan = CivilDate::new(2028, 1, 31).unwrap();
        assert_eq!(jan.add_days(30).unwrap(), CivilDate::new(2028, 3, 1).unwrap());
    }

    #[test]
    fn large_day_step_remains_calendar_correct() {
        let date = CivilDate::new(2026, 1, 1).unwrap();
        assert_eq!(date.add_days(365).unwrap(), CivilDate::new(2027, 1, 1).unwrap());
        assert_eq!(date.add_days(366 + 365).unwrap(), CivilDate::new(2028, 1, 2).unwrap());
    }
}
