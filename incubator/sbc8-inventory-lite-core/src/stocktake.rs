use crate::inventory::{project_location_balance, InventoryItem, StockMovement, StockMovementKind};
use crate::primitives::{
    BoundedText, DomainError, DomainResult, EntityId, PositiveQuantity, StockBalance,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StocktakeObservation {
    id: EntityId,
    item_id: EntityId,
    location_id: EntityId,
    observed_quantity: u64,
    unit_id: EntityId,
    occurred_minute: i64,
    source_reference: BoundedText,
}

impl StocktakeObservation {
    pub fn new(
        id: EntityId,
        item: &InventoryItem,
        location_id: EntityId,
        observed_quantity: u64,
        unit_id: EntityId,
        occurred_minute: i64,
        source_reference: BoundedText,
    ) -> DomainResult<Self> {
        if &unit_id != item.base_unit_id() {
            return Err(DomainError::InvalidValue(
                "stocktake unit must match item tracked unit",
            ));
        }
        Ok(Self {
            id,
            item_id: item.id().clone(),
            location_id,
            observed_quantity,
            unit_id,
            occurred_minute,
            source_reference,
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn item_id(&self) -> &EntityId {
        &self.item_id
    }

    pub fn location_id(&self) -> &EntityId {
        &self.location_id
    }

    pub fn observed_quantity(&self) -> u64 {
        self.observed_quantity
    }

    pub fn unit_id(&self) -> &EntityId {
        &self.unit_id
    }

    pub fn occurred_minute(&self) -> i64 {
        self.occurred_minute
    }

    pub fn source_reference(&self) -> &BoundedText {
        &self.source_reference
    }

    pub fn propose_adjustment(
        &self,
        item: &InventoryItem,
        movements: &[StockMovement],
        movement_id: EntityId,
    ) -> DomainResult<Option<StocktakeAdjustmentProposal>> {
        if self.item_id != *item.id() || self.unit_id != *item.base_unit_id() {
            return Err(DomainError::InconsistentEvidence(
                "stocktake observation does not match item",
            ));
        }
        let projection = project_location_balance(item, &self.location_id, movements)?;
        let observed = i128::from(self.observed_quantity);
        let delta = observed
            .checked_sub(projection.balance.get())
            .ok_or(DomainError::Overflow)?;
        if delta == 0 {
            return Ok(None);
        }
        let increase = delta > 0;
        let magnitude = if increase {
            delta
        } else {
            delta.checked_neg().ok_or(DomainError::Overflow)?
        };
        let magnitude = u64::try_from(magnitude).map_err(|_| DomainError::Overflow)?;
        let movement = StockMovement::new(
            movement_id,
            item,
            self.location_id.clone(),
            PositiveQuantity::new(magnitude)?,
            self.unit_id.clone(),
            if increase {
                StockMovementKind::StocktakeIncrease
            } else {
                StockMovementKind::StocktakeDecrease
            },
            self.occurred_minute,
            self.source_reference.clone(),
            Some(self.id.clone()),
            None,
            None,
        )?;
        Ok(Some(StocktakeAdjustmentProposal {
            observation_id: self.id.clone(),
            previous_balance: projection.balance,
            observed_quantity: self.observed_quantity,
            movement,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StocktakeAdjustmentProposal {
    observation_id: EntityId,
    previous_balance: StockBalance,
    observed_quantity: u64,
    movement: StockMovement,
}

impl StocktakeAdjustmentProposal {
    pub fn observation_id(&self) -> &EntityId {
        &self.observation_id
    }

    pub fn previous_balance(&self) -> StockBalance {
        self.previous_balance
    }

    pub fn observed_quantity(&self) -> u64 {
        self.observed_quantity
    }

    pub fn movement(&self) -> &StockMovement {
        &self.movement
    }

    pub fn into_movement(self) -> StockMovement {
        self.movement
    }
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

    fn item() -> InventoryItem {
        InventoryItem::new(id("item"), text("SKU-1"), text("Widget"), id("each"))
    }

    fn opening(quantity: u64) -> StockMovement {
        let item = item();
        StockMovement::new(
            id("opening"),
            &item,
            id("store"),
            PositiveQuantity::new(quantity).unwrap(),
            id("each"),
            StockMovementKind::Opening,
            1,
            text("opening"),
            None,
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn stocktake_equal_count_creates_no_adjustment() {
        let item = item();
        let observation = StocktakeObservation::new(
            id("count"),
            &item,
            id("store"),
            10,
            id("each"),
            2,
            text("physical count"),
        )
        .unwrap();
        assert!(observation
            .propose_adjustment(&item, &[opening(10)], id("adjust"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn stocktake_excess_creates_append_only_increase() {
        let item = item();
        let existing = opening(8);
        let observation = StocktakeObservation::new(
            id("count"),
            &item,
            id("store"),
            11,
            id("each"),
            2,
            text("physical count"),
        )
        .unwrap();
        let proposal = observation
            .propose_adjustment(&item, std::slice::from_ref(&existing), id("adjust"))
            .unwrap()
            .unwrap();
        assert_eq!(existing.quantity().get(), 8);
        assert_eq!(proposal.previous_balance().get(), 8);
        assert_eq!(proposal.movement().kind(), StockMovementKind::StocktakeIncrease);
        assert_eq!(proposal.movement().quantity().get(), 3);
        assert_eq!(proposal.movement().source_id(), Some(&id("count")));
    }

    #[test]
    fn stocktake_shortage_creates_append_only_decrease_and_reaches_observed() {
        let item = item();
        let existing = opening(12);
        let observation = StocktakeObservation::new(
            id("count"),
            &item,
            id("store"),
            9,
            id("each"),
            2,
            text("physical count"),
        )
        .unwrap();
        let adjustment = observation
            .propose_adjustment(&item, std::slice::from_ref(&existing), id("adjust"))
            .unwrap()
            .unwrap()
            .into_movement();
        assert_eq!(adjustment.kind(), StockMovementKind::StocktakeDecrease);
        let projection = project_location_balance(&item, &id("store"), &[existing, adjustment]).unwrap();
        assert_eq!(projection.balance.get(), 9);
    }
}
