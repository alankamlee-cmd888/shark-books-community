#![forbid(unsafe_code)]

pub mod inventory;
pub mod primitives;
pub mod stocktake;
pub mod transfer;

#[cfg(test)]
mod integration_tests {
    use crate::inventory::{
        project_location_balance, summarize_item, InventoryItem, StockMovement, StockMovementKind,
    };
    use crate::primitives::{BoundedText, EntityId, PositiveQuantity};
    use crate::stocktake::StocktakeObservation;
    use crate::transfer::StockTransferProposal;

    fn id(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn text(value: &str) -> BoundedText {
        BoundedText::new(value, 200).unwrap()
    }

    #[test]
    fn representative_inventory_lite_flow_is_append_only_and_domain_only() {
        let item = InventoryItem::new(id("item"), text("SKU-1"), text("Widget"), id("each"));
        let opening = StockMovement::new(
            id("opening"),
            &item,
            id("a"),
            PositiveQuantity::new(20).unwrap(),
            id("each"),
            StockMovementKind::Opening,
            1,
            text("opening balance"),
            None,
            None,
            None,
        )
        .unwrap();
        let receipt = StockMovement::new(
            id("receipt"),
            &item,
            id("a"),
            PositiveQuantity::new(5).unwrap(),
            id("each"),
            StockMovementKind::PurchaseReceipt,
            2,
            text("supplier receipt"),
            Some(id("receipt-source")),
            None,
            None,
        )
        .unwrap();
        let consume = StockMovement::new(
            id("consume"),
            &item,
            id("a"),
            PositiveQuantity::new(3).unwrap(),
            id("each"),
            StockMovementKind::SaleConsume,
            3,
            text("customer use"),
            Some(id("sale-source")),
            None,
            None,
        )
        .unwrap();
        let transfer = StockTransferProposal::new(
            id("transfer"),
            id("transfer-out"),
            id("transfer-in"),
            &item,
            id("a"),
            id("b"),
            PositiveQuantity::new(4).unwrap(),
            id("each"),
            4,
            text("move to van stock"),
            Some(id("move-request")),
        )
        .unwrap();
        let (transfer_out, transfer_in) = transfer.into_movements();
        let mut movements = vec![opening, receipt, consume, transfer_out, transfer_in];

        let before_count = project_location_balance(&item, &id("b"), &movements).unwrap();
        assert_eq!(before_count.balance.get(), 4);

        let stocktake = StocktakeObservation::new(
            id("stocktake"),
            &item,
            id("b"),
            5,
            id("each"),
            5,
            text("physical van count"),
        )
        .unwrap();
        let adjustment = stocktake
            .propose_adjustment(&item, &movements, id("stocktake-adjustment"))
            .unwrap()
            .unwrap()
            .into_movement();
        movements.push(adjustment);

        let a = project_location_balance(&item, &id("a"), &movements).unwrap();
        let b = project_location_balance(&item, &id("b"), &movements).unwrap();
        let summary = summarize_item(&item, &movements).unwrap();
        assert_eq!(a.balance.get(), 18);
        assert_eq!(b.balance.get(), 5);
        assert_eq!(summary.total_balance.get(), 23);
        assert_eq!(summary.contributing_movement_ids.len(), 6);
    }
}
