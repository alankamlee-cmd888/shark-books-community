use crate::primitives::{
    checked_bill_for_minutes, BoundedText, DomainError, DomainResult,
    DraftCommercialLineProposal, EntityId, Money, ProposalSourceKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeEntryState {
    Draft,
    Approved,
    BilledProposal,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeEntry {
    pub id: EntityId,
    pub worker_id: EntityId,
    pub project_id: EntityId,
    pub activity: BoundedText,
    pub start_minute: i64,
    pub end_minute: i64,
    pub billable: bool,
    pub billing_rate_per_hour: Option<Money>,
    pub notes: BoundedText,
    pub state: TimeEntryState,
    pub supersedes: Option<EntityId>,
}

impl TimeEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EntityId,
        worker_id: EntityId,
        project_id: EntityId,
        activity: BoundedText,
        start_minute: i64,
        end_minute: i64,
        billable: bool,
        billing_rate_per_hour: Option<Money>,
        notes: BoundedText,
    ) -> DomainResult<Self> {
        if end_minute <= start_minute {
            return Err(DomainError::InvalidValue("time entry end must be after start"));
        }
        if billable && billing_rate_per_hour.is_none() {
            return Err(DomainError::InvalidValue("billable time requires a rate snapshot"));
        }
        if let Some(rate) = billing_rate_per_hour {
            if rate.minor() < 0 {
                return Err(DomainError::InvalidValue("billing rate must be nonnegative"));
            }
        }
        Ok(Self {
            id,
            worker_id,
            project_id,
            activity,
            start_minute,
            end_minute,
            billable,
            billing_rate_per_hour,
            notes,
            state: TimeEntryState::Draft,
            supersedes: None,
        })
    }

    pub fn duration_minutes(&self) -> DomainResult<u64> {
        let duration = self
            .end_minute
            .checked_sub(self.start_minute)
            .ok_or(DomainError::Overflow)?;
        u64::try_from(duration).map_err(|_| DomainError::InvalidValue("negative duration"))
    }

    pub fn approve(&mut self) -> DomainResult<()> {
        if self.state != TimeEntryState::Draft {
            return Err(DomainError::InvalidState("only draft time may be approved"));
        }
        self.state = TimeEntryState::Approved;
        Ok(())
    }

    pub fn cancel(&mut self) -> DomainResult<()> {
        if self.state != TimeEntryState::Draft {
            return Err(DomainError::InvalidState("only draft time may be cancelled"));
        }
        self.state = TimeEntryState::Cancelled;
        Ok(())
    }

    pub fn correction(
        &self,
        new_id: EntityId,
        start_minute: i64,
        end_minute: i64,
        activity: BoundedText,
        notes: BoundedText,
    ) -> DomainResult<Self> {
        if !matches!(self.state, TimeEntryState::Approved | TimeEntryState::BilledProposal) {
            return Err(DomainError::InvalidState("only approved evidence may be corrected by new identity"));
        }
        if new_id == self.id {
            return Err(DomainError::InvalidValue("correction must have a new identity"));
        }
        let mut corrected = Self::new(
            new_id,
            self.worker_id.clone(),
            self.project_id.clone(),
            activity,
            start_minute,
            end_minute,
            self.billable,
            self.billing_rate_per_hour,
            notes,
        )?;
        corrected.supersedes = Some(self.id.clone());
        Ok(corrected)
    }

    pub fn to_draft_commercial_line_proposal(
        &mut self,
        proposal_id: EntityId,
    ) -> DomainResult<DraftCommercialLineProposal> {
        if self.state != TimeEntryState::Approved {
            return Err(DomainError::InvalidState("only approved time may create a draft-line proposal"));
        }
        if !self.billable {
            return Err(DomainError::InvalidValue("non-billable time cannot create a billing proposal"));
        }
        let rate = self
            .billing_rate_per_hour
            .ok_or(DomainError::InvalidValue("billable time missing rate snapshot"))?;
        let amount = checked_bill_for_minutes(rate, self.duration_minutes()?)?;
        let description = BoundedText::new(
            format!("{} ({} minutes)", self.activity.as_str(), self.duration_minutes()?),
            240,
        )?;
        self.state = TimeEntryState::BilledProposal;
        Ok(DraftCommercialLineProposal {
            id: proposal_id,
            source_kind: ProposalSourceKind::TimeEntry,
            source_id: self.id.clone(),
            project_id: Some(self.project_id.clone()),
            description,
            amount,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeOverlap {
    pub first_id: EntityId,
    pub second_id: EntityId,
    pub worker_id: EntityId,
}

pub fn detect_overlaps(entries: &[TimeEntry]) -> Vec<TimeOverlap> {
    let mut overlaps = Vec::new();
    for (index, first) in entries.iter().enumerate() {
        if first.state == TimeEntryState::Cancelled {
            continue;
        }
        for second in entries.iter().skip(index + 1) {
            if second.state == TimeEntryState::Cancelled || first.worker_id != second.worker_id {
                continue;
            }
            if first.start_minute < second.end_minute && second.start_minute < first.end_minute {
                overlaps.push(TimeOverlap {
                    first_id: first.id.clone(),
                    second_id: second.id.clone(),
                    worker_id: first.worker_id.clone(),
                });
            }
        }
    }
    overlaps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn entry(id_value: &str, worker: &str, start: i64, end: i64) -> TimeEntry {
        TimeEntry::new(
            id(id_value),
            id(worker),
            id("project"),
            BoundedText::new("Consulting", 120).unwrap(),
            start,
            end,
            true,
            Some(Money::nonnegative(6000).unwrap()),
            BoundedText::new("client work", 500).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn invalid_time_interval_is_rejected() {
        let result = TimeEntry::new(
            id("t1"),
            id("w1"),
            id("p1"),
            BoundedText::new("Work", 120).unwrap(),
            100,
            100,
            false,
            None,
            BoundedText::new("notes", 500).unwrap(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn overlap_detection_is_worker_scoped() {
        let entries = vec![entry("a", "w1", 100, 160), entry("b", "w1", 150, 200), entry("c", "w2", 150, 200)];
        let overlaps = detect_overlaps(&entries);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].first_id, id("a"));
        assert_eq!(overlaps[0].second_id, id("b"));
    }

    #[test]
    fn approved_time_correction_uses_new_identity() {
        let mut original = entry("old", "w1", 100, 160);
        original.approve().unwrap();
        let corrected = original
            .correction(
                id("new"),
                100,
                170,
                BoundedText::new("Corrected consulting", 120).unwrap(),
                BoundedText::new("corrected", 500).unwrap(),
            )
            .unwrap();
        assert_eq!(original.end_minute, 160);
        assert_eq!(corrected.supersedes, Some(id("old")));
        assert_eq!(corrected.state, TimeEntryState::Draft);
    }

    #[test]
    fn approved_billable_time_creates_draft_line_proposal_only() {
        let mut time = entry("t1", "w1", 100, 190);
        time.approve().unwrap();
        let proposal = time.to_draft_commercial_line_proposal(id("proposal")).unwrap();
        assert_eq!(proposal.amount.minor(), 9000);
        assert_eq!(proposal.source_kind, ProposalSourceKind::TimeEntry);
        assert_eq!(time.state, TimeEntryState::BilledProposal);
    }
}
