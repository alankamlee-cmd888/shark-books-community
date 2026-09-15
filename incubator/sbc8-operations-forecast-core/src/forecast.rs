use std::collections::BTreeSet;

use crate::date::CivilDate;
use crate::primitives::{BoundedText, DomainError, DomainResult, EntityId, Money};
use crate::recurrence::RecurrenceSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastCertainty {
    KnownContractual,
    Expected,
    ScenarioOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastSourceKind {
    Receivable,
    Payable,
    RecurringCommitment,
    OwnerFutureEvent,
    PurchaseOrderCommitment,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmountEstimate {
    Exact(Money),
    Approximate(Money),
    Range { low: Money, high: Money },
}

impl AmountEstimate {
    pub fn validate(&self) -> DomainResult<()> {
        if let Self::Range { low, high } = self {
            if low > high {
                return Err(DomainError::InvalidValue("forecast range low exceeds high"));
            }
        }
        Ok(())
    }

    pub fn band(&self) -> DomainResult<(Money, Money, Money)> {
        self.validate()?;
        match self {
            Self::Exact(amount) | Self::Approximate(amount) => Ok((*amount, *amount, *amount)),
            Self::Range { low, high } => Ok((*low, Money::midpoint(*low, *high)?, *high)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastEventState {
    Planned,
    Skipped,
    LinkedActual(EntityId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastEvent {
    id: EntityId,
    date: CivilDate,
    source_reference: BoundedText,
    source_kind: ForecastSourceKind,
    certainty: ForecastCertainty,
    amount: AmountEstimate,
    scenario_id: Option<EntityId>,
    state: ForecastEventState,
}

impl ForecastEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EntityId,
        date: CivilDate,
        source_reference: BoundedText,
        source_kind: ForecastSourceKind,
        certainty: ForecastCertainty,
        amount: AmountEstimate,
        scenario_id: Option<EntityId>,
    ) -> DomainResult<Self> {
        amount.validate()?;
        validate_scenario_binding(certainty, scenario_id.as_ref())?;
        Ok(Self {
            id,
            date,
            source_reference,
            source_kind,
            certainty,
            amount,
            scenario_id,
            state: ForecastEventState::Planned,
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn date(&self) -> CivilDate {
        self.date
    }

    pub fn source_reference(&self) -> &BoundedText {
        &self.source_reference
    }

    pub fn source_kind(&self) -> ForecastSourceKind {
        self.source_kind
    }

    pub fn certainty(&self) -> ForecastCertainty {
        self.certainty
    }

    pub fn amount(&self) -> &AmountEstimate {
        &self.amount
    }

    pub fn scenario_id(&self) -> Option<&EntityId> {
        self.scenario_id.as_ref()
    }

    pub fn state(&self) -> &ForecastEventState {
        &self.state
    }

    pub fn skip(&mut self) -> DomainResult<()> {
        if self.state != ForecastEventState::Planned {
            return Err(DomainError::InvalidState("only planned forecast event may be skipped"));
        }
        self.state = ForecastEventState::Skipped;
        Ok(())
    }

    pub fn link_actual(&mut self, actual_id: EntityId) -> DomainResult<()> {
        if self.state != ForecastEventState::Planned {
            return Err(DomainError::InvalidState("only planned forecast event may link to actual"));
        }
        self.state = ForecastEventState::LinkedActual(actual_id);
        Ok(())
    }
}

fn validate_scenario_binding(
    certainty: ForecastCertainty,
    scenario_id: Option<&EntityId>,
) -> DomainResult<()> {
    match certainty {
        ForecastCertainty::ScenarioOnly if scenario_id.is_none() => {
            Err(DomainError::InvalidValue("scenario-only item requires scenario identity"))
        }
        ForecastCertainty::KnownContractual | ForecastCertainty::Expected if scenario_id.is_some() => {
            Err(DomainError::InvalidValue("baseline item must not carry scenario identity"))
        }
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastContribution {
    pub event_id: EntityId,
    pub source_reference: BoundedText,
    pub certainty: ForecastCertainty,
    pub amount: AmountEstimate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastPoint {
    pub date: CivilDate,
    pub low_balance: Money,
    pub expected_balance: Money,
    pub high_balance: Money,
    pub contributions: Vec<ForecastContribution>,
}

pub fn project_forecast(
    starting_balance: Money,
    events: &[ForecastEvent],
    enabled_scenarios: &[EntityId],
) -> DomainResult<Vec<ForecastPoint>> {
    let mut seen_event_ids = BTreeSet::new();
    for event in events {
        if !seen_event_ids.insert(event.id.clone()) {
            return Err(DomainError::Duplicate("forecast event identity reused"));
        }
    }

    let mut eligible: Vec<&ForecastEvent> = events
        .iter()
        .filter(|event| event.state == ForecastEventState::Planned)
        .filter(|event| match event.certainty {
            ForecastCertainty::ScenarioOnly => event
                .scenario_id
                .as_ref()
                .map(|scenario| enabled_scenarios.iter().any(|enabled| enabled == scenario))
                .unwrap_or(false),
            ForecastCertainty::KnownContractual | ForecastCertainty::Expected => true,
        })
        .collect();

    eligible.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });

    let mut low_balance = starting_balance;
    let mut expected_balance = starting_balance;
    let mut high_balance = starting_balance;
    let mut output = Vec::new();
    let mut index = 0;

    while index < eligible.len() {
        let date = eligible[index].date;
        let mut low_delta = Money::ZERO;
        let mut expected_delta = Money::ZERO;
        let mut high_delta = Money::ZERO;
        let mut contributions = Vec::new();

        while index < eligible.len() && eligible[index].date == date {
            let event = eligible[index];
            let (low, expected, high) = event.amount.band()?;
            low_delta = low_delta.checked_add(low)?;
            expected_delta = expected_delta.checked_add(expected)?;
            high_delta = high_delta.checked_add(high)?;
            contributions.push(ForecastContribution {
                event_id: event.id.clone(),
                source_reference: event.source_reference.clone(),
                certainty: event.certainty,
                amount: event.amount.clone(),
            });
            index += 1;
        }

        low_balance = low_balance.checked_add(low_delta)?;
        expected_balance = expected_balance.checked_add(expected_delta)?;
        high_balance = high_balance.checked_add(high_delta)?;
        output.push(ForecastPoint {
            date,
            low_balance,
            expected_balance,
            high_balance,
            contributions,
        });
    }

    Ok(output)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledForecastTemplate {
    id: EntityId,
    source_reference: BoundedText,
    source_kind: ForecastSourceKind,
    certainty: ForecastCertainty,
    amount: AmountEstimate,
    scenario_id: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastEventProposal {
    pub id: EntityId,
    pub template_id: EntityId,
    pub date: CivilDate,
    pub source_reference: BoundedText,
    pub source_kind: ForecastSourceKind,
    pub certainty: ForecastCertainty,
    pub amount: AmountEstimate,
    pub scenario_id: Option<EntityId>,
}

impl ScheduledForecastTemplate {
    pub fn new(
        id: EntityId,
        source_reference: BoundedText,
        source_kind: ForecastSourceKind,
        certainty: ForecastCertainty,
        amount: AmountEstimate,
        scenario_id: Option<EntityId>,
    ) -> DomainResult<Self> {
        amount.validate()?;
        validate_scenario_binding(certainty, scenario_id.as_ref())?;
        Ok(Self {
            id,
            source_reference,
            source_kind,
            certainty,
            amount,
            scenario_id,
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn propose(
        &self,
        recurrence: &RecurrenceSpec,
        max: usize,
    ) -> DomainResult<Vec<ForecastEventProposal>> {
        recurrence
            .generate(max)?
            .into_iter()
            .enumerate()
            .map(|(index, date)| {
                let proposal_id = EntityId::new(format!("{}-{}", self.id.as_str(), index + 1))?;
                Ok(ForecastEventProposal {
                    id: proposal_id,
                    template_id: self.id.clone(),
                    date,
                    source_reference: self.source_reference.clone(),
                    source_kind: self.source_kind,
                    certainty: self.certainty,
                    amount: self.amount.clone(),
                    scenario_id: self.scenario_id.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recurrence::{MonthlyPolicy, RecurrenceFrequency};

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn text(value: &str) -> BoundedText {
        BoundedText::new(value, 120).unwrap()
    }

    fn event(
        id_value: &str,
        day: u8,
        certainty: ForecastCertainty,
        amount: AmountEstimate,
        scenario: Option<&str>,
    ) -> ForecastEvent {
        ForecastEvent::new(
            id(id_value),
            CivilDate::new(2026, 9, day).unwrap(),
            text(id_value),
            ForecastSourceKind::OwnerFutureEvent,
            certainty,
            amount,
            scenario.map(id),
        )
        .unwrap()
    }

    #[test]
    fn forecast_is_chronological_and_traceable() {
        let events = vec![
            event("later", 20, ForecastCertainty::Expected, AmountEstimate::Exact(Money::from_minor(-200)), None),
            event("earlier", 10, ForecastCertainty::KnownContractual, AmountEstimate::Exact(Money::from_minor(500)), None),
        ];
        let points = project_forecast(Money::from_minor(1000), &events, &[]).unwrap();
        assert_eq!(points[0].date, CivilDate::new(2026, 9, 10).unwrap());
        assert_eq!(points[0].expected_balance.minor(), 1500);
        assert_eq!(points[0].contributions[0].event_id, id("earlier"));
        assert_eq!(points[1].expected_balance.minor(), 1300);
    }

    #[test]
    fn forecast_range_propagates_low_expected_high_without_hiding_band() {
        let events = vec![event(
            "range",
            10,
            ForecastCertainty::Expected,
            AmountEstimate::Range { low: Money::from_minor(100), high: Money::from_minor(300) },
            None,
        )];
        let points = project_forecast(Money::from_minor(1000), &events, &[]).unwrap();
        assert_eq!(points[0].low_balance.minor(), 1100);
        assert_eq!(points[0].expected_balance.minor(), 1200);
        assert_eq!(points[0].high_balance.minor(), 1300);
    }

    #[test]
    fn scenario_events_require_explicit_enablement() {
        let events = vec![event(
            "scenario-event",
            10,
            ForecastCertainty::ScenarioOnly,
            AmountEstimate::Exact(Money::from_minor(-400)),
            Some("scenario-a"),
        )];
        assert!(project_forecast(Money::from_minor(1000), &events, &[]).unwrap().is_empty());
        let enabled = project_forecast(Money::from_minor(1000), &events, &[id("scenario-a")]).unwrap();
        assert_eq!(enabled[0].expected_balance.minor(), 600);
    }

    #[test]
    fn skipped_and_linked_actual_events_do_not_double_count() {
        let mut skipped = event("skip", 10, ForecastCertainty::Expected, AmountEstimate::Exact(Money::from_minor(100)), None);
        skipped.skip().unwrap();
        let mut linked = event("linked", 11, ForecastCertainty::Expected, AmountEstimate::Exact(Money::from_minor(200)), None);
        linked.link_actual(id("actual-1")).unwrap();
        assert!(project_forecast(Money::from_minor(1000), &[skipped, linked], &[]).unwrap().is_empty());
    }

    #[test]
    fn duplicate_forecast_event_identity_is_rejected() {
        let events = vec![
            event("same", 10, ForecastCertainty::Expected, AmountEstimate::Exact(Money::from_minor(100)), None),
            event("same", 11, ForecastCertainty::Expected, AmountEstimate::Exact(Money::from_minor(200)), None),
        ];
        assert!(project_forecast(Money::from_minor(1000), &events, &[]).is_err());
    }

    #[test]
    fn recurrence_to_forecast_creates_proposals_only() {
        let recurrence = RecurrenceSpec::new(
            CivilDate::new(2026, 9, 1).unwrap(),
            RecurrenceFrequency::Monthly,
            1,
            None,
            Some(3),
            BoundedText::new("Europe/London", 64).unwrap(),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        let template = ScheduledForecastTemplate::new(
            id("rent"),
            text("office rent schedule"),
            ForecastSourceKind::RecurringCommitment,
            ForecastCertainty::KnownContractual,
            AmountEstimate::Exact(Money::from_minor(-50_000)),
            None,
        )
        .unwrap();
        let proposals = template.propose(&recurrence, 10).unwrap();
        assert_eq!(proposals.len(), 3);
        assert_eq!(proposals[0].id, id("rent-1"));
        assert_eq!(proposals[2].date, CivilDate::new(2026, 11, 1).unwrap());
    }

    #[test]
    fn invalid_scenario_template_binding_fails_at_construction() {
        let result = ScheduledForecastTemplate::new(
            id("bad"),
            text("bad binding"),
            ForecastSourceKind::Other,
            ForecastCertainty::ScenarioOnly,
            AmountEstimate::Exact(Money::from_minor(10)),
            None,
        );
        assert!(result.is_err());
    }
}
