#![forbid(unsafe_code)]

pub mod documents;
pub mod identity;
pub mod payments;
pub mod primitives;
pub mod purchasing;

pub use documents::{
    CommercialLine, CreditNote, Invoice, InvoiceState, IssuedInvoiceSnapshot, IssuedQuoteSnapshot,
    Quote, QuoteState,
};
pub use identity::{CommercialParty, CommercialPartySnapshot};
pub use payments::{
    EventRecordOutcome, PaymentEvent, PaymentEventKind, PaymentEventRegistry, PaymentIntent,
    PaymentIntentState, Settlement,
};
pub use primitives::{
    CommercialNumber, DomainError, DomainResult, EntityId, Money, Quantity,
};
pub use purchasing::{
    BillComparisonLine, PurchaseOrder, PurchaseOrderLine, PurchaseOrderState, PurchaseVariance,
    PurchaseVarianceLine, SupplierBill, SupplierBillState, SupplierCredit,
};

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn representative_quote_invoice_payment_bill_and_po_flow_is_domain_only() {
        let customer = CommercialParty::new(
            EntityId::new("customer-1").unwrap(),
            "Customer",
            None::<String>,
            Some("customer@example.test"),
            None::<String>,
        )
        .unwrap();
        let supplier = CommercialParty::new(
            EntityId::new("supplier-1").unwrap(),
            "Supplier",
            None::<String>,
            None::<String>,
            None::<String>,
        )
        .unwrap();

        let mut quote = Quote::new(EntityId::new("quote-1").unwrap(), customer.snapshot());
        quote
            .add_line(
                CommercialLine::new(
                    EntityId::new("quote-line-1").unwrap(),
                    "Service",
                    Quantity::positive(1).unwrap(),
                    Money::positive(12_000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        quote.issue(CommercialNumber::new("Q-1").unwrap()).unwrap();
        quote.accept().unwrap();

        let mut invoice = quote
            .to_draft_invoice(EntityId::new("invoice-1").unwrap())
            .unwrap();
        invoice.issue(CommercialNumber::new("INV-1").unwrap()).unwrap();

        let mut payment = PaymentIntent::new(
            EntityId::new("operation-1").unwrap(),
            invoice.id().clone(),
            Money::positive(12_000).unwrap(),
        )
        .unwrap();
        payment
            .apply_verified_event(
                &PaymentEvent::new(
                    EntityId::new("provider-event-1").unwrap(),
                    EntityId::new("operation-1").unwrap(),
                    Money::positive(12_000).unwrap(),
                    PaymentEventKind::Succeeded,
                    Some(EntityId::new("provider-object-1").unwrap()),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(invoice.state(), InvoiceState::Issued);
        payment
            .allocate_to_invoice(&mut invoice, Money::positive(12_000).unwrap())
            .unwrap();
        assert_eq!(invoice.state(), InvoiceState::Paid);

        let mut bill = SupplierBill::new(
            EntityId::new("bill-1").unwrap(),
            supplier.snapshot(),
            "SUP-001",
        )
        .unwrap();
        bill.add_line(
            CommercialLine::new(
                EntityId::new("bill-line-1").unwrap(),
                "Parts",
                Quantity::positive(2).unwrap(),
                Money::positive(2_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        bill.approve().unwrap();
        assert_eq!(bill.state(), SupplierBillState::ApprovedOpen);

        let mut po = PurchaseOrder::new(EntityId::new("po-1").unwrap(), supplier.snapshot());
        let po_line_id = EntityId::new("po-line-1").unwrap();
        po.add_line(
            PurchaseOrderLine::new(
                po_line_id.clone(),
                "Parts",
                Quantity::positive(2).unwrap(),
                Money::positive(2_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        po.issue().unwrap();
        po.receive(&po_line_id, Quantity::positive(2).unwrap())
            .unwrap();
        assert_eq!(po.state(), PurchaseOrderState::Received);
    }
}
