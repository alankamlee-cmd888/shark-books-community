use crate::date::CivilDate;
use crate::primitives::{
    BoundedText, DomainError, DomainResult, DraftCommercialLineProposal, EntityId, Money,
    ProposalSourceKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceUnit {
    Metre,
    MilliMile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Distance {
    pub units: u64,
    pub unit: DistanceUnit,
}

impl Distance {
    pub fn positive(units: u64, unit: DistanceUnit) -> DomainResult<Self> {
        if units == 0 {
            return Err(DomainError::InvalidValue("distance must be positive"));
        }
        Ok(Self { units, unit })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MileageSourceMethod {
    Manual,
    Odometer,
    RouteDerived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MileageEntry {
    pub id: EntityId,
    pub date: CivilDate,
    pub from_text: BoundedText,
    pub to_text: BoundedText,
    pub business_purpose: BoundedText,
    pub distance: Distance,
    pub source_method: MileageSourceMethod,
    pub project_id: Option<EntityId>,
    pub customer_id: Option<EntityId>,
    pub odometer_start: Option<u64>,
    pub odometer_end: Option<u64>,
}

impl MileageEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EntityId,
        date: CivilDate,
        from_text: BoundedText,
        to_text: BoundedText,
        business_purpose: BoundedText,
        distance: Distance,
        source_method: MileageSourceMethod,
        project_id: Option<EntityId>,
        customer_id: Option<EntityId>,
        odometer_start: Option<u64>,
        odometer_end: Option<u64>,
    ) -> DomainResult<Self> {
        match source_method {
            MileageSourceMethod::Odometer => {
                let start = odometer_start.ok_or(DomainError::InconsistentEvidence("odometer start required"))?;
                let end = odometer_end.ok_or(DomainError::InconsistentEvidence("odometer end required"))?;
                if end <= start {
                    return Err(DomainError::InconsistentEvidence("odometer end must exceed start"));
                }
                let delta = end.checked_sub(start).ok_or(DomainError::Overflow)?;
                if delta != distance.units {
                    return Err(DomainError::InconsistentEvidence("odometer delta must equal recorded distance units"));
                }
            }
            MileageSourceMethod::Manual | MileageSourceMethod::RouteDerived => {
                if odometer_start.is_some() || odometer_end.is_some() {
                    return Err(DomainError::InconsistentEvidence("odometer evidence supplied for non-odometer method"));
                }
            }
        }

        Ok(Self {
            id,
            date,
            from_text,
            to_text,
            business_purpose,
            distance,
            source_method,
            project_id,
            customer_id,
            odometer_start,
            odometer_end,
        })
    }

    pub fn to_draft_commercial_line_proposal(
        &self,
        proposal_id: EntityId,
        rate: &MileageRate,
    ) -> DomainResult<DraftCommercialLineProposal> {
        if !rate.applies_to(self.date, self.distance.unit) {
            return Err(DomainError::InvalidValue("mileage rate does not apply to entry"));
        }
        let numerator = i128::from(rate.minor_units_per_1000_units.minor())
            .checked_mul(i128::from(self.distance.units))
            .ok_or(DomainError::Overflow)?;
        let whole = numerator / 1000;
        let remainder = numerator % 1000;
        let rounded = if remainder * 2 >= 1000 {
            whole.checked_add(1).ok_or(DomainError::Overflow)?
        } else {
            whole
        };
        let amount_minor = i64::try_from(rounded).map_err(|_| DomainError::Overflow)?;
        let description = BoundedText::new(
            format!(
                "Mileage: {} to {} — {}",
                self.from_text.as_str(),
                self.to_text.as_str(),
                self.business_purpose.as_str()
            ),
            240,
        )?;
        Ok(DraftCommercialLineProposal {
            id: proposal_id,
            source_kind: ProposalSourceKind::Mileage,
            source_id: self.id.clone(),
            project_id: self.project_id.clone(),
            description,
            amount: Money::from_minor(amount_minor),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MileageRate {
    pub id: EntityId,
    pub effective_from: CivilDate,
    pub effective_to: Option<CivilDate>,
    pub unit: DistanceUnit,
    pub minor_units_per_1000_units: Money,
    pub source_reference: BoundedText,
}

impl MileageRate {
    pub fn new(
        id: EntityId,
        effective_from: CivilDate,
        effective_to: Option<CivilDate>,
        unit: DistanceUnit,
        minor_units_per_1000_units: Money,
        source_reference: BoundedText,
    ) -> DomainResult<Self> {
        if minor_units_per_1000_units.minor() < 0 {
            return Err(DomainError::InvalidValue("mileage rate must be nonnegative"));
        }
        if let Some(end) = effective_to {
            if end < effective_from {
                return Err(DomainError::InvalidValue("rate end precedes start"));
            }
        }
        Ok(Self {
            id,
            effective_from,
            effective_to,
            unit,
            minor_units_per_1000_units,
            source_reference,
        })
    }

    pub fn applies_to(&self, date: CivilDate, unit: DistanceUnit) -> bool {
        self.unit == unit
            && date >= self.effective_from
            && self.effective_to.map(|end| date <= end).unwrap_or(true)
    }
}

pub fn select_rate<'a>(
    rates: &'a [MileageRate],
    date: CivilDate,
    unit: DistanceUnit,
) -> DomainResult<Option<&'a MileageRate>> {
    let mut matches = rates.iter().filter(|rate| rate.applies_to(date, unit));
    let first = matches.next();
    if matches.next().is_some() {
        return Err(DomainError::Duplicate("overlapping mileage rates"));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn text(value: &str) -> BoundedText {
        BoundedText::new(value, 200).unwrap()
    }

    #[test]
    fn manual_mileage_requires_no_route_or_location_dependency() {
        let entry = MileageEntry::new(
            id("m1"),
            CivilDate::new(2026, 9, 14).unwrap(),
            text("Office"),
            text("Client"),
            text("Business meeting"),
            Distance::positive(12_500, DistanceUnit::MilliMile).unwrap(),
            MileageSourceMethod::Manual,
            Some(id("project")),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(entry.source_method, MileageSourceMethod::Manual);
    }

    #[test]
    fn odometer_inconsistency_is_rejected() {
        let result = MileageEntry::new(
            id("m1"),
            CivilDate::new(2026, 9, 14).unwrap(),
            text("A"),
            text("B"),
            text("Visit"),
            Distance::positive(50, DistanceUnit::Metre).unwrap(),
            MileageSourceMethod::Odometer,
            None,
            None,
            Some(1000),
            Some(1040),
        );
        assert!(result.is_err());
    }

    #[test]
    fn effective_dated_source_linked_rate_selection_is_explicit() {
        let old = MileageRate::new(
            id("r1"),
            CivilDate::new(2026, 1, 1).unwrap(),
            Some(CivilDate::new(2026, 6, 30).unwrap()),
            DistanceUnit::MilliMile,
            Money::nonnegative(45).unwrap(),
            text("policy-source-v1"),
        )
        .unwrap();
        let new = MileageRate::new(
            id("r2"),
            CivilDate::new(2026, 7, 1).unwrap(),
            None,
            DistanceUnit::MilliMile,
            Money::nonnegative(50).unwrap(),
            text("policy-source-v2"),
        )
        .unwrap();
        let rates = vec![old, new];
        let selected = select_rate(&rates, CivilDate::new(2026, 9, 14).unwrap(), DistanceUnit::MilliMile)
            .unwrap()
            .unwrap();
        assert_eq!(selected.id, id("r2"));
    }

    #[test]
    fn explicit_mileage_rate_can_create_draft_line_proposal() {
        let entry = MileageEntry::new(
            id("m1"),
            CivilDate::new(2026, 9, 14).unwrap(),
            text("Office"),
            text("Client"),
            text("Business meeting"),
            Distance::positive(12_500, DistanceUnit::MilliMile).unwrap(),
            MileageSourceMethod::Manual,
            Some(id("project")),
            None,
            None,
            None,
        )
        .unwrap();
        let rate = MileageRate::new(
            id("rate"),
            CivilDate::new(2026, 1, 1).unwrap(),
            None,
            DistanceUnit::MilliMile,
            Money::nonnegative(45).unwrap(),
            text("explicit-billing-policy"),
        )
        .unwrap();
        let proposal = entry.to_draft_commercial_line_proposal(id("proposal"), &rate).unwrap();
        assert_eq!(proposal.amount.minor(), 563);
        assert_eq!(proposal.source_kind, ProposalSourceKind::Mileage);
    }
}
