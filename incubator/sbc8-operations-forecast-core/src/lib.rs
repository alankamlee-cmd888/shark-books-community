#![forbid(unsafe_code)]

pub mod date;
pub mod forecast;
pub mod mileage;
pub mod primitives;
pub mod projects;
pub mod recurrence;
pub mod timesheets;

#[cfg(test)]
mod integration_tests {
    use crate::date::CivilDate;
    use crate::forecast::{
        project_forecast, AmountEstimate, ForecastCertainty, ForecastSourceKind,
        ScheduledForecastTemplate,
    };
    use crate::mileage::{
        Distance, DistanceUnit, MileageEntry, MileageRate, MileageSourceMethod,
    };
    use crate::primitives::{BoundedText, EntityId, Money};
    use crate::projects::{summarize_project, Project, ProjectFact, ProjectFactKind};
    use crate::recurrence::{MonthlyPolicy, RecurrenceFrequency, RecurrenceSpec};
    use crate::timesheets::TimeEntry;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn text(value: &str, max: usize) -> BoundedText {
        BoundedText::new(value, max).unwrap()
    }

    #[test]
    fn representative_project_time_mileage_and_forecast_flow_is_domain_only() {
        let mut project = Project::new(id("project-1"), text("Customer rollout", 120), Some(id("customer-1")));
        project.activate().unwrap();

        let mut time = TimeEntry::new(
            id("time-1"),
            id("worker-1"),
            project.id().clone(),
            text("Implementation", 120),
            1_000,
            1_120,
            true,
            Some(Money::nonnegative(7_500).unwrap()),
            text("On-site setup", 500),
        )
        .unwrap();
        time.approve().unwrap();
        let time_line = time.to_draft_commercial_line_proposal(id("time-proposal")).unwrap();
        assert_eq!(time_line.amount.minor(), 15_000);

        let mileage = MileageEntry::new(
            id("mileage-1"),
            CivilDate::new(2026, 9, 14).unwrap(),
            text("Office", 200),
            text("Customer", 200),
            text("Project visit", 200),
            Distance::positive(10_000, DistanceUnit::MilliMile).unwrap(),
            MileageSourceMethod::Manual,
            Some(project.id().clone()),
            Some(id("customer-1")),
            None,
            None,
        )
        .unwrap();
        let mileage_rate = MileageRate::new(
            id("mileage-policy"),
            CivilDate::new(2026, 1, 1).unwrap(),
            None,
            DistanceUnit::MilliMile,
            Money::nonnegative(45).unwrap(),
            text("explicit owner billing policy", 200),
        )
        .unwrap();
        let mileage_line = mileage
            .to_draft_commercial_line_proposal(id("mileage-proposal"), &mileage_rate)
            .unwrap();
        assert_eq!(mileage_line.amount.minor(), 450);

        let facts = vec![
            ProjectFact::money(id("income"), project.id().clone(), ProjectFactKind::Income, Money::from_minor(20_000)).unwrap(),
            ProjectFact::money(id("cost"), project.id().clone(), ProjectFactKind::Cost, Money::from_minor(5_000)).unwrap(),
            ProjectFact::quantity(id("time-fact"), project.id().clone(), ProjectFactKind::TimeMinutes, 120).unwrap(),
            ProjectFact::quantity(id("mileage-fact"), project.id().clone(), ProjectFactKind::MileageUnits, 10_000).unwrap(),
        ];
        let summary = summarize_project(project.id(), &facts).unwrap();
        assert_eq!(summary.profit.minor(), 15_000);

        let recurrence = RecurrenceSpec::new(
            CivilDate::new(2026, 10, 1).unwrap(),
            RecurrenceFrequency::Monthly,
            1,
            None,
            Some(2),
            text("Europe/London", 64),
            MonthlyPolicy::SameDayClamped,
        )
        .unwrap();
        let template = ScheduledForecastTemplate::new(
            id("commitment"),
            text("confirmed recurring commitment", 120),
            ForecastSourceKind::RecurringCommitment,
            ForecastCertainty::KnownContractual,
            AmountEstimate::Exact(Money::from_minor(-2_500)),
            None,
        )
        .unwrap();
        let proposals = template.propose(&recurrence, 10).unwrap();
        assert_eq!(proposals.len(), 2);

        let events = proposals
            .into_iter()
            .map(|proposal| {
                crate::forecast::ForecastEvent::new(
                    proposal.id,
                    proposal.date,
                    proposal.source_reference,
                    proposal.source_kind,
                    proposal.certainty,
                    proposal.amount,
                    proposal.scenario_id,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let forecast = project_forecast(Money::from_minor(10_000), &events, &[]).unwrap();
        assert_eq!(forecast.len(), 2);
        assert_eq!(forecast[1].expected_balance.minor(), 5_000);
    }
}
