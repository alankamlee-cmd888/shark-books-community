use crate::inventory::{InventoryItem, StockMovement, StockMovementKind};
use crate::primitives::{BoundedText, DomainError, DomainResult, EntityId, PositiveQuantity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockTransferProposal {
    transfer_id: EntityId,
    outbound: StockMovement,
    inbound: StockMovement,
}

impl StockTransferProposal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        transfer_id: EntityId,
        outbound_movement_id: EntityId,
        inbound_movement_id: EntityId,
        item: &InventoryItem,
        source_location_id: EntityId,
        destination_location_id: EntityId,
        quantity: PositiveQuantity,
        unit_id: EntityId,
        occurred_minute: i64,
        source_reference: BoundedText,
        source_id: Option<EntityId>,
    ) -> DomainResult<Self> {
        if source_location_id == destination_location_id {
            return Err(DomainError::InvalidValue(
                "transfer source and destination must differ",
            ));
        }
        if outbound_movement_id == inbound_movement_id {
            return Err(DomainError::InvalidValue(
                "transfer movements require distinct identities",
            ));
        }

        let outbound = StockMovement::new_internal(
            outbound_movement_id,
            item,
            source_location_id,
            quantity,
            unit_id.clone(),
            StockMovementKind::TransferOut,
            occurred_minute,
            source_reference.clone(),
            source_id.clone(),
            Some(transfer_id.clone()),
            None,
        )?;
        let inbound = StockMovement::new_internal(
            inbound_movement_id,
            item,
            destination_location_id,
            quantity,
            unit_id,
            StockMovementKind::TransferIn,
            occurred_minute,
            source_reference,
            source_id,
            Some(transfer_id.clone()),
            None,
        )?;

        validate_transfer_pair(&outbound, &inbound)?;
        Ok(Self {
            transfer_id,
            outbound,
            inbound,
        })
    }

    pub fn transfer_id(&self) -> &EntityId {
        &self.transfer_id
    }

    pub fn outbound(&self) -> &StockMovement {
        &self.outbound
    }

    pub fn inbound(&self) -> &StockMovement {
        &self.inbound
    }

    pub fn into_movements(self) -> (StockMovement, StockMovement) {
        (self.outbound, self.inbound)
    }
}

pub fn validate_transfer_pair(outbound: &StockMovement, inbound: &StockMovement) -> DomainResult<()> {
    if outbound.kind() != StockMovementKind::TransferOut
        || inbound.kind() != StockMovementKind::TransferIn
    {
        return Err(DomainError::InconsistentEvidence(
            "transfer pair must be outbound then inbound",
        ));
    }
    if outbound.id() == inbound.id() {
        return Err(DomainError::InconsistentEvidence(
            "transfer movement identities must differ",
        ));
    }
    if outbound.item_id() != inbound.item_id()
        || outbound.unit_id() != inbound.unit_id()
        || outbound.quantity() != inbound.quantity()
        || outbound.occurred_minute() != inbound.occurred_minute()
        || outbound.source_reference() != inbound.source_reference()
        || outbound.source_id() != inbound.source_id()
        || outbound.transfer_id() != inbound.transfer_id()
    {
        return Err(DomainError::InconsistentEvidence(
            "transfer pair facts do not match",
        ));
    }
    if outbound.transfer_id().is_none() {
        return Err(DomainError::InconsistentEvidence(
            "transfer pair missing transfer identity",
        ));
    }
    if outbound.location_id() == inbound.location_id() {
        return Err(DomainError::InconsistentEvidence(
            "transfer pair locations must differ",
        ));
    }
    if outbound.signed_delta() + inbound.signed_delta() != 0 {
        return Err(DomainError::InconsistentEvidence(
            "transfer pair must conserve quantity",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::summarize_item;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn text(value: &str) -> BoundedText {
        BoundedText::new(value, 200).unwrap()
    }

    fn item() -> InventoryItem {
        InventoryItem::new(id("item"), text("SKU-1"), text("Widget"), id("each"))
    }

    #[test]
    fn transfer_rejects_same_location() {
        let result = StockTransferProposal::new(
            id("transfer"),
            id("out"),
            id("in"),
            &item(),
            id("store"),
            id("store"),
            PositiveQuantity::new(2).unwrap(),
            id("each"),
            10,
            text("move stock"),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn valid_transfer_conserves_total_and_shifts_locations() {
        let item = item();
        let opening = StockMovement::new(
            id("opening"),
            &item,
            id("a"),
            PositiveQuantity::new(10).unwrap(),
            id("each"),
            StockMovementKind::Opening,
            1,
            text("opening"),
            None,
            None,
            None,
        )
        .unwrap();
        let transfer = StockTransferProposal::new(
            id("transfer"),
            id("out"),
            id("in"),
            &item,
            id("a"),
            id("b"),
            PositiveQuantity::new(4).unwrap(),
            id("each"),
            2,
            text("move stock"),
            Some(id("request")),
        )
        .unwrap();
        let (outbound, inbound) = transfer.into_movements();
        let summary = summarize_item(&item, &[opening, outbound, inbound]).unwrap();
        assert_eq!(summary.total_balance.get(), 10);
        assert_eq!(summary.locations.len(), 2);
        assert_eq!(summary.locations[0].location_id, id("a"));
        assert_eq!(summary.locations[0].balance.get(), 6);
        assert_eq!(summary.locations[1].location_id, id("b"));
        assert_eq!(summary.locations[1].balance.get(), 4);
    }

    #[test]
    fn malformed_transfer_pair_is_rejected() {
        let item = item();
        let outbound = StockMovement::new_internal(
            id("out"),
            &item,
            id("a"),
            PositiveQuantity::new(3).unwrap(),
            id("each"),
            StockMovementKind::TransferOut,
            2,
            text("move stock"),
            None,
            Some(id("transfer")),
            None,
        )
        .unwrap();
        let inbound = StockMovement::new_internal(
            id("in"),
            &item,
            id("b"),
            PositiveQuantity::new(2).unwrap(),
            id("each"),
            StockMovementKind::TransferIn,
            2,
            text("move stock"),
            None,
            Some(id("transfer")),
            None,
        )
        .unwrap();
        assert!(validate_transfer_pair(&outbound, &inbound).is_err());
    }
}
