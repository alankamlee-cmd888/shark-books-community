use std::collections::BTreeSet;

use crate::primitives::{BoundedText, DomainError, DomainResult, EntityId, Money};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectState {
    Planned,
    Active,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    id: EntityId,
    name: BoundedText,
    customer_id: Option<EntityId>,
    state: ProjectState,
}

impl Project {
    pub fn new(
        id: EntityId,
        name: BoundedText,
        customer_id: Option<EntityId>,
    ) -> Self {
        Self {
            id,
            name,
            customer_id,
            state: ProjectState::Planned,
        }
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn name(&self) -> &BoundedText {
        &self.name
    }

    pub fn customer_id(&self) -> Option<&EntityId> {
        self.customer_id.as_ref()
    }

    pub fn state(&self) -> ProjectState {
        self.state
    }

    pub fn activate(&mut self) -> DomainResult<()> {
        match self.state {
            ProjectState::Planned | ProjectState::Paused => {
                self.state = ProjectState::Active;
                Ok(())
            }
            _ => Err(DomainError::InvalidState("project cannot activate from current state")),
        }
    }

    pub fn pause(&mut self) -> DomainResult<()> {
        if self.state != ProjectState::Active {
            return Err(DomainError::InvalidState("only active project can pause"));
        }
        self.state = ProjectState::Paused;
        Ok(())
    }

    pub fn complete(&mut self) -> DomainResult<()> {
        match self.state {
            ProjectState::Active | ProjectState::Paused => {
                self.state = ProjectState::Completed;
                Ok(())
            }
            _ => Err(DomainError::InvalidState("project cannot complete from current state")),
        }
    }

    pub fn cancel(&mut self) -> DomainResult<()> {
        match self.state {
            ProjectState::Planned | ProjectState::Active | ProjectState::Paused => {
                self.state = ProjectState::Cancelled;
                Ok(())
            }
            _ => Err(DomainError::InvalidState("terminal project cannot be cancelled")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFactKind {
    Income,
    Cost,
    TimeMinutes,
    MileageUnits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFact {
    id: EntityId,
    project_id: EntityId,
    kind: ProjectFactKind,
    money: Option<Money>,
    quantity: Option<u64>,
}

impl ProjectFact {
    pub fn money(
        id: EntityId,
        project_id: EntityId,
        kind: ProjectFactKind,
        amount: Money,
    ) -> DomainResult<Self> {
        if !matches!(kind, ProjectFactKind::Income | ProjectFactKind::Cost) {
            return Err(DomainError::InvalidValue("money fact requires income or cost kind"));
        }
        Ok(Self {
            id,
            project_id,
            kind,
            money: Some(amount),
            quantity: None,
        })
    }

    pub fn quantity(
        id: EntityId,
        project_id: EntityId,
        kind: ProjectFactKind,
        quantity: u64,
    ) -> DomainResult<Self> {
        if !matches!(kind, ProjectFactKind::TimeMinutes | ProjectFactKind::MileageUnits) {
            return Err(DomainError::InvalidValue("quantity fact requires time or mileage kind"));
        }
        Ok(Self {
            id,
            project_id,
            kind,
            money: None,
            quantity: Some(quantity),
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn project_id(&self) -> &EntityId {
        &self.project_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub project_id: EntityId,
    pub income: Money,
    pub cost: Money,
    pub profit: Money,
    pub time_minutes: u64,
    pub mileage_units: u64,
    pub contributing_fact_ids: Vec<EntityId>,
}

pub fn summarize_project(project_id: &EntityId, facts: &[ProjectFact]) -> DomainResult<ProjectSummary> {
    let mut income = Money::ZERO;
    let mut cost = Money::ZERO;
    let mut time_minutes = 0_u64;
    let mut mileage_units = 0_u64;
    let mut contributing_fact_ids = Vec::new();
    let mut seen_fact_ids = BTreeSet::new();

    for fact in facts.iter().filter(|fact| &fact.project_id == project_id) {
        if !seen_fact_ids.insert(fact.id.clone()) {
            return Err(DomainError::Duplicate("project fact identity reused"));
        }
        match fact.kind {
            ProjectFactKind::Income => {
                income = income.checked_add(fact.money.ok_or(DomainError::InvalidValue("income fact missing money"))?)?;
            }
            ProjectFactKind::Cost => {
                cost = cost.checked_add(fact.money.ok_or(DomainError::InvalidValue("cost fact missing money"))?)?;
            }
            ProjectFactKind::TimeMinutes => {
                time_minutes = time_minutes
                    .checked_add(fact.quantity.ok_or(DomainError::InvalidValue("time fact missing quantity"))?)
                    .ok_or(DomainError::Overflow)?;
            }
            ProjectFactKind::MileageUnits => {
                mileage_units = mileage_units
                    .checked_add(fact.quantity.ok_or(DomainError::InvalidValue("mileage fact missing quantity"))?)
                    .ok_or(DomainError::Overflow)?;
            }
        }
        contributing_fact_ids.push(fact.id.clone());
    }

    let profit = income.checked_sub(cost)?;
    Ok(ProjectSummary {
        project_id: project_id.clone(),
        income,
        cost,
        profit,
        time_minutes,
        mileage_units,
        contributing_fact_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    #[test]
    fn project_lifecycle_transitions_fail_closed() {
        let mut project = Project::new(id("p1"), BoundedText::new("Salon refit", 120).unwrap(), None);
        assert!(project.pause().is_err());
        project.activate().unwrap();
        project.pause().unwrap();
        project.activate().unwrap();
        project.complete().unwrap();
        assert_eq!(project.state(), ProjectState::Completed);
        assert!(project.activate().is_err());
        assert!(project.cancel().is_err());
    }

    #[test]
    fn project_profitability_is_deterministic_and_traceable() {
        let project_id = id("p1");
        let facts = vec![
            ProjectFact::money(id("i1"), project_id.clone(), ProjectFactKind::Income, Money::from_minor(10_000)).unwrap(),
            ProjectFact::money(id("c1"), project_id.clone(), ProjectFactKind::Cost, Money::from_minor(3_500)).unwrap(),
            ProjectFact::quantity(id("t1"), project_id.clone(), ProjectFactKind::TimeMinutes, 90).unwrap(),
            ProjectFact::quantity(id("m1"), project_id.clone(), ProjectFactKind::MileageUnits, 12_000).unwrap(),
        ];
        let summary = summarize_project(&project_id, &facts).unwrap();
        assert_eq!(summary.profit.minor(), 6_500);
        assert_eq!(summary.time_minutes, 90);
        assert_eq!(summary.mileage_units, 12_000);
        assert_eq!(summary.contributing_fact_ids.len(), 4);
    }

    #[test]
    fn duplicate_project_fact_identity_is_rejected() {
        let project_id = id("p1");
        let facts = vec![
            ProjectFact::money(id("same"), project_id.clone(), ProjectFactKind::Income, Money::from_minor(100)).unwrap(),
            ProjectFact::money(id("same"), project_id.clone(), ProjectFactKind::Cost, Money::from_minor(20)).unwrap(),
        ];
        assert!(summarize_project(&project_id, &facts).is_err());
    }
}
