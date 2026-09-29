use std::collections::{BTreeMap, BTreeSet};

use crate::primitives::{
    quantity_to_i128, BoundedText, DomainError, DomainResult, EntityId, PositiveQuantity,
    StockBalance,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItem {
    id: EntityId,
    sku: BoundedText,
    name: BoundedText,
    base_unit_id: EntityId,
    active: bool,
}

impl InventoryItem {
    pub fn new(id: EntityId, sku: BoundedText, name: BoundedText, base_unit_id: EntityId) -> Self {
        Self {
            id,
            sku,
            name,
            base_unit_id,
            active: true,
        }
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn sku(&self) -> &BoundedText {
        &self.sku
    }

    pub fn name(&self) -> &BoundedText {
        &self.name
    }

    pub fn base_unit_id(&self) -> &EntityId {
        &self.base_unit_id
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryLocation {
    id: EntityId,
    name: BoundedText,
    active: bool,
}

impl InventoryLocation {
    pub fn new(id: EntityId, name: BoundedText) -> Self {
        Self {
            id,
            name,
            active: true,
        }
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn name(&self) -> &BoundedText {
        &self.name
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockMovementKind {
    Opening,
    PurchaseReceipt,
    SaleConsume,
    ReturnIn,
    ReturnOut,
    AdjustmentIncrease,
    AdjustmentDecrease,
    StocktakeIncrease,
    StocktakeDecrease,
    DamageWriteOff,
    TransferIn,
    TransferOut,
}

impl StockMovementKind {
    pub fn sign(self) -> i8 {
        match self {
            Self::Opening
            | Self::PurchaseReceipt
            | Self::ReturnIn
            | Self::AdjustmentIncrease
            | Self::StocktakeIncrease
            | Self::TransferIn => 1,
            Self::SaleConsume
            | Self::ReturnOut
            | Self::AdjustmentDecrease
            | Self::StocktakeDecrease
            | Self::DamageWriteOff
            | Self::TransferOut => -1,
        }
    }

    pub fn is_transfer(self) -> bool {
        matches!(self, Self::TransferIn | Self::TransferOut)
    }

    pub fn is_stocktake(self) -> bool {
        matches!(self, Self::StocktakeIncrease | Self::StocktakeDecrease)
    }

    pub fn is_correction_adjustment(self) -> bool {
        matches!(self, Self::AdjustmentIncrease | Self::AdjustmentDecrease)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockMovement {
    id: EntityId,
    item_id: EntityId,
    location_id: EntityId,
    quantity: PositiveQuantity,
    unit_id: EntityId,
    kind: StockMovementKind,
    occurred_minute: i64,
    source_reference: BoundedText,
    source_id: Option<EntityId>,
    transfer_id: Option<EntityId>,
    corrects_movement_id: Option<EntityId>,
}

impl StockMovement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EntityId,
        item: &InventoryItem,
        location_id: EntityId,
        quantity: PositiveQuantity,
        unit_id: EntityId,
        kind: StockMovementKind,
        occurred_minute: i64,
        source_reference: BoundedText,
        source_id: Option<EntityId>,
        transfer_id: Option<EntityId>,
        corrects_movement_id: Option<EntityId>,
    ) -> DomainResult<Self> {
        if kind.is_transfer() || kind.is_stocktake() {
            return Err(DomainError::InvalidValue(
                "transfer and stocktake movements require typed proposal workflows",
            ));
        }
        Self::new_internal(
            id,
            item,
            location_id,
            quantity,
            unit_id,
            kind,
            occurred_minute,
            source_reference,
            source_id,
            transfer_id,
            corrects_movement_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_internal(
        id: EntityId,
        item: &InventoryItem,
        location_id: EntityId,
        quantity: PositiveQuantity,
        unit_id: EntityId,
        kind: StockMovementKind,
        occurred_minute: i64,
        source_reference: BoundedText,
        source_id: Option<EntityId>,
        transfer_id: Option<EntityId>,
        corrects_movement_id: Option<EntityId>,
    ) -> DomainResult<Self> {
        if &unit_id != item.base_unit_id() {
            return Err(DomainError::InvalidValue(
                "movement unit must match item tracked unit",
            ));
        }
        match (kind.is_transfer(), transfer_id.is_some()) {
            (true, false) => {
                return Err(DomainError::InconsistentEvidence(
                    "transfer movement requires transfer identity",
                ));
            }
            (false, true) => {
                return Err(DomainError::InconsistentEvidence(
                    "non-transfer movement must not carry transfer identity",
                ));
            }
            _ => {}
        }
        if let Some(corrected_id) = corrects_movement_id.as_ref() {
            if !kind.is_correction_adjustment() {
                return Err(DomainError::InvalidValue(
                    "correction link requires adjustment movement",
                ));
            }
            if corrected_id == &id {
                return Err(DomainError::InvalidValue(
                    "correction movement cannot correct itself",
                ));
            }
        }
        Ok(Self {
            id,
            item_id: item.id().clone(),
            location_id,
            quantity,
            unit_id,
            kind,
            occurred_minute,
            source_reference,
            source_id,
            transfer_id,
            corrects_movement_id,
        })
    }

    pub fn correction_adjustment(
        original: &Self,
        new_id: EntityId,
        quantity: PositiveQuantity,
        increase: bool,
        occurred_minute: i64,
        source_reference: BoundedText,
    ) -> DomainResult<Self> {
        if original.kind.is_transfer() {
            return Err(DomainError::InvalidValue(
                "paired transfer correction is outside Inventory Lite correction helper",
            ));
        }
        if new_id == original.id {
            return Err(DomainError::InvalidValue(
                "correction movement requires new identity",
            ));
        }
        Ok(Self {
            id: new_id,
            item_id: original.item_id.clone(),
            location_id: original.location_id.clone(),
            quantity,
            unit_id: original.unit_id.clone(),
            kind: if increase {
                StockMovementKind::AdjustmentIncrease
            } else {
                StockMovementKind::AdjustmentDecrease
            },
            occurred_minute,
            source_reference,
            source_id: None,
            transfer_id: None,
            corrects_movement_id: Some(original.id.clone()),
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

    pub fn quantity(&self) -> PositiveQuantity {
        self.quantity
    }

    pub fn unit_id(&self) -> &EntityId {
        &self.unit_id
    }

    pub fn kind(&self) -> StockMovementKind {
        self.kind
    }

    pub fn occurred_minute(&self) -> i64 {
        self.occurred_minute
    }

    pub fn source_reference(&self) -> &BoundedText {
        &self.source_reference
    }

    pub fn source_id(&self) -> Option<&EntityId> {
        self.source_id.as_ref()
    }

    pub fn transfer_id(&self) -> Option<&EntityId> {
        self.transfer_id.as_ref()
    }

    pub fn corrects_movement_id(&self) -> Option<&EntityId> {
        self.corrects_movement_id.as_ref()
    }

    pub fn signed_delta(&self) -> i128 {
        quantity_to_i128(self.quantity) * i128::from(self.kind.sign())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementContribution {
    pub movement_id: EntityId,
    pub occurred_minute: i64,
    pub signed_delta: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceProjection {
    pub item_id: EntityId,
    pub location_id: EntityId,
    pub balance: StockBalance,
    pub contributions: Vec<MovementContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationBalance {
    pub location_id: EntityId,
    pub balance: StockBalance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItemSummary {
    pub item_id: EntityId,
    pub total_balance: StockBalance,
    pub locations: Vec<LocationBalance>,
    pub contributing_movement_ids: Vec<EntityId>,
}

fn validate_unique_movement_ids(movements: &[StockMovement]) -> DomainResult<()> {
    let mut seen = BTreeSet::new();
    for movement in movements {
        if !seen.insert(movement.id.clone()) {
            return Err(DomainError::Duplicate("stock movement identity reused"));
        }
    }
    Ok(())
}

fn validate_item_unit(item: &InventoryItem, movement: &StockMovement) -> DomainResult<()> {
    if movement.item_id == item.id && movement.unit_id != item.base_unit_id {
        return Err(DomainError::InconsistentEvidence(
            "stored movement unit differs from item tracked unit",
        ));
    }
    Ok(())
}

pub fn project_location_balance(
    item: &InventoryItem,
    location_id: &EntityId,
    movements: &[StockMovement],
) -> DomainResult<BalanceProjection> {
    validate_unique_movement_ids(movements)?;
    let mut balance = StockBalance::ZERO;
    let mut contributions = Vec::new();

    for movement in movements
        .iter()
        .filter(|movement| movement.item_id() == item.id() && movement.location_id() == location_id)
    {
        validate_item_unit(item, movement)?;
        balance = balance.checked_add(movement.signed_delta())?;
        contributions.push(MovementContribution {
            movement_id: movement.id.clone(),
            occurred_minute: movement.occurred_minute,
            signed_delta: movement.signed_delta(),
        });
    }

    contributions.sort_by(|left, right| {
        left.occurred_minute
            .cmp(&right.occurred_minute)
            .then_with(|| left.movement_id.as_str().cmp(right.movement_id.as_str()))
    });

    Ok(BalanceProjection {
        item_id: item.id.clone(),
        location_id: location_id.clone(),
        balance,
        contributions,
    })
}

pub fn summarize_item(
    item: &InventoryItem,
    movements: &[StockMovement],
) -> DomainResult<InventoryItemSummary> {
    validate_unique_movement_ids(movements)?;
    let mut by_location: BTreeMap<EntityId, StockBalance> = BTreeMap::new();
    let mut contributions: Vec<(i64, EntityId)> = Vec::new();

    for movement in movements.iter().filter(|movement| movement.item_id() == item.id()) {
        validate_item_unit(item, movement)?;
        let current = by_location
            .get(movement.location_id())
            .copied()
            .unwrap_or(StockBalance::ZERO);
        by_location.insert(
            movement.location_id().clone(),
            current.checked_add(movement.signed_delta())?,
        );
        contributions.push((movement.occurred_minute(), movement.id().clone()));
    }

    let mut total = StockBalance::ZERO;
    let mut locations = Vec::new();
    for (location_id, balance) in by_location {
        total = total.checked_add(balance.get())?;
        locations.push(LocationBalance {
            location_id,
            balance,
        });
    }

    contributions.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.as_str().cmp(right.1.as_str()))
    });

    Ok(InventoryItemSummary {
        item_id: item.id.clone(),
        total_balance: total,
        locations,
        contributing_movement_ids: contributions.into_iter().map(|(_, id)| id).collect(),
    })
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

    fn movement(
        movement_id: &str,
        location: &str,
        quantity: u64,
        kind: StockMovementKind,
        minute: i64,
    ) -> StockMovement {
        let item = item();
        StockMovement::new(
            id(movement_id),
            &item,
            id(location),
            PositiveQuantity::new(quantity).unwrap(),
            id("each"),
            kind,
            minute,
            text(movement_id),
            None,
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn item_and_location_fields_are_validated_and_private() {
        let mut item = item();
        let mut location = InventoryLocation::new(id("store"), text("Main store"));
        assert_eq!(item.base_unit_id(), &id("each"));
        assert!(item.active());
        item.deactivate();
        location.deactivate();
        assert!(!item.active());
        assert!(!location.active());
    }

    #[test]
    fn movement_rejects_wrong_tracked_unit() {
        let item = item();
        let result = StockMovement::new(
            id("m1"),
            &item,
            id("store"),
            PositiveQuantity::new(2).unwrap(),
            id("box"),
            StockMovementKind::Opening,
            1,
            text("opening"),
            None,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn direct_transfer_and_stocktake_movement_construction_is_rejected() {
        let item = item();
        let transfer = StockMovement::new(
            id("transfer-out"),
            &item,
            id("a"),
            PositiveQuantity::new(2).unwrap(),
            id("each"),
            StockMovementKind::TransferOut,
            1,
            text("direct transfer"),
            None,
            Some(id("transfer")),
            None,
        );
        let stocktake = StockMovement::new(
            id("stocktake"),
            &item,
            id("a"),
            PositiveQuantity::new(2).unwrap(),
            id("each"),
            StockMovementKind::StocktakeIncrease,
            1,
            text("direct stocktake"),
            Some(id("count")),
            None,
            None,
        );
        assert!(transfer.is_err());
        assert!(stocktake.is_err());
    }

    #[test]
    fn all_movement_kinds_have_deterministic_signed_effects() {
        let positive = [
            StockMovementKind::Opening,
            StockMovementKind::PurchaseReceipt,
            StockMovementKind::ReturnIn,
            StockMovementKind::AdjustmentIncrease,
            StockMovementKind::StocktakeIncrease,
            StockMovementKind::TransferIn,
        ];
        let negative = [
            StockMovementKind::SaleConsume,
            StockMovementKind::ReturnOut,
            StockMovementKind::AdjustmentDecrease,
            StockMovementKind::StocktakeDecrease,
            StockMovementKind::DamageWriteOff,
            StockMovementKind::TransferOut,
        ];
        assert!(positive.into_iter().all(|kind| kind.sign() == 1));
        assert!(negative.into_iter().all(|kind| kind.sign() == -1));
    }

    #[test]
    fn duplicate_stock_movement_identity_is_rejected() {
        let item = item();
        let movements = vec![
            movement("same", "store", 5, StockMovementKind::Opening, 1),
            movement("same", "store", 1, StockMovementKind::SaleConsume, 2),
        ];
        assert!(project_location_balance(&item, &id("store"), &movements).is_err());
    }

    #[test]
    fn location_balance_is_deterministic_and_traceable() {
        let item = item();
        let movements = vec![
            movement("consume", "store", 3, StockMovementKind::SaleConsume, 30),
            movement("opening", "store", 10, StockMovementKind::Opening, 10),
            movement("receipt", "store", 5, StockMovementKind::PurchaseReceipt, 20),
        ];
        let projection = project_location_balance(&item, &id("store"), &movements).unwrap();
        assert_eq!(projection.balance.get(), 12);
        assert_eq!(projection.contributions[0].movement_id, id("opening"));
        assert_eq!(projection.contributions[2].movement_id, id("consume"));
    }

    #[test]
    fn negative_stock_is_explicit_not_clamped() {
        let item = item();
        let movements = vec![movement(
            "consume",
            "store",
            4,
            StockMovementKind::SaleConsume,
            1,
        )];
        let projection = project_location_balance(&item, &id("store"), &movements).unwrap();
        assert_eq!(projection.balance.get(), -4);
    }

    #[test]
    fn item_summary_derives_cross_location_total() {
        let item = item();
        let movements = vec![
            movement("a", "store-a", 10, StockMovementKind::Opening, 1),
            movement("b", "store-b", 4, StockMovementKind::Opening, 2),
            movement("c", "store-a", 3, StockMovementKind::SaleConsume, 3),
        ];
        let summary = summarize_item(&item, &movements).unwrap();
        assert_eq!(summary.total_balance.get(), 11);
        assert_eq!(summary.locations.len(), 2);
        assert_eq!(summary.contributing_movement_ids, vec![id("a"), id("b"), id("c")]);
    }

    #[test]
    fn correction_is_append_only_and_links_original() {
        let original = movement("original", "store", 10, StockMovementKind::Opening, 1);
        let correction = StockMovement::correction_adjustment(
            &original,
            id("correction"),
            PositiveQuantity::new(2).unwrap(),
            false,
            2,
            text("correct opening quantity"),
        )
        .unwrap();
        assert_eq!(original.quantity().get(), 10);
        assert_eq!(original.kind(), StockMovementKind::Opening);
        assert_eq!(correction.corrects_movement_id(), Some(&id("original")));
        assert_eq!(correction.signed_delta(), -2);
    }
}
