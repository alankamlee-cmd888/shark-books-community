use std::collections::{BTreeMap, BTreeSet};

use crate::documents::CommercialLine;
use crate::identity::CommercialPartySnapshot;
use crate::primitives::{
    bounded_text, canonical_reference, DomainError, DomainResult, EntityId, Money, Quantity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplierBillState {
    Draft,
    ApprovedOpen,
    PartPaid,
    Paid,
    Credited,
    Void,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplierCredit {
    id: EntityId,
    bill_id: EntityId,
    amount: Money,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplierBill {
    id: EntityId,
    supplier: CommercialPartySnapshot,
    supplier_reference: String,
    lines: Vec<CommercialLine>,
    state: SupplierBillState,
    approved_total: Option<Money>,
    paid: Money,
    credited: Money,
}

impl SupplierBill {
    pub fn new(
        id: EntityId,
        supplier: CommercialPartySnapshot,
        supplier_reference: impl Into<String>,
    ) -> DomainResult<Self> {
        Ok(Self {
            id,
            supplier,
            supplier_reference: bounded_text(
                supplier_reference,
                "supplier reference",
                128,
            )?,
            lines: Vec::new(),
            state: SupplierBillState::Draft,
            approved_total: None,
            paid: Money::zero(),
            credited: Money::zero(),
        })
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    #[must_use]
    pub const fn state(&self) -> SupplierBillState {
        self.state
    }

    #[must_use]
    pub fn duplicate_key(&self) -> String {
        format!(
            "{}|{}",
            self.supplier.id().as_str(),
            canonical_reference(&self.supplier_reference)
        )
    }

    pub fn add_line(&mut self, line: CommercialLine) -> DomainResult<()> {
        if self.state != SupplierBillState::Draft {
            return Err(DomainError::InvalidTransition(
                "supplier bill is immutable after approval".into(),
            ));
        }
        if self.lines.iter().any(|existing| existing.id() == line.id()) {
            return Err(DomainError::Conflict("duplicate supplier bill line id".into()));
        }
        self.lines.push(line);
        Ok(())
    }

    pub fn approve(&mut self) -> DomainResult<()> {
        if self.state != SupplierBillState::Draft {
            return Err(DomainError::InvalidTransition(
                "supplier bill approval requires draft state".into(),
            ));
        }
        if self.lines.is_empty() {
            return Err(DomainError::Invalid(
                "supplier bill requires at least one line".into(),
            ));
        }
        let total = self
            .lines
            .iter()
            .try_fold(Money::zero(), |sum, line| sum.checked_add(line.total()?))?;
        self.approved_total = Some(total);
        self.state = SupplierBillState::ApprovedOpen;
        Ok(())
    }

    pub fn apply_payment(&mut self, amount: Money) -> DomainResult<()> {
        let amount = Money::positive(amount.minor())?;
        self.require_open()?;
        if amount > self.outstanding()? {
            return Err(DomainError::Conflict(
                "supplier payment exceeds outstanding balance".into(),
            ));
        }
        self.paid = self.paid.checked_add(amount)?;
        self.state = if self.outstanding()?.minor() == 0 {
            SupplierBillState::Paid
        } else {
            SupplierBillState::PartPaid
        };
        Ok(())
    }

    pub fn create_credit(
        &mut self,
        credit_id: EntityId,
        amount: Money,
        reason: impl Into<String>,
    ) -> DomainResult<SupplierCredit> {
        let amount = Money::positive(amount.minor())?;
        self.require_open()?;
        if credit_id == self.id {
            return Err(DomainError::Conflict(
                "supplier credit and bill must have distinct identities".into(),
            ));
        }
        if amount > self.outstanding()? {
            return Err(DomainError::Conflict(
                "supplier credit exceeds outstanding balance".into(),
            ));
        }
        let reason = bounded_text(reason, "supplier credit reason", 500)?;
        self.credited = self.credited.checked_add(amount)?;
        self.state = if self.outstanding()?.minor() == 0 {
            SupplierBillState::Credited
        } else if self.paid.minor() > 0 {
            SupplierBillState::PartPaid
        } else {
            SupplierBillState::ApprovedOpen
        };
        Ok(SupplierCredit {
            id: credit_id,
            bill_id: self.id.clone(),
            amount,
            reason,
        })
    }

    pub fn void(&mut self) -> DomainResult<()> {
        if self.state != SupplierBillState::ApprovedOpen
            || self.paid.minor() != 0
            || self.credited.minor() != 0
        {
            return Err(DomainError::InvalidTransition(
                "only an unpaid, uncredited approved bill can be voided".into(),
            ));
        }
        self.state = SupplierBillState::Void;
        Ok(())
    }

    pub fn outstanding(&self) -> DomainResult<Money> {
        let total = self
            .approved_total
            .ok_or_else(|| DomainError::InvalidTransition("supplier bill is not approved".into()))?;
        total.checked_sub(self.paid)?.checked_sub(self.credited)
    }

    fn require_open(&self) -> DomainResult<()> {
        if !matches!(
            self.state,
            SupplierBillState::ApprovedOpen | SupplierBillState::PartPaid
        ) {
            return Err(DomainError::InvalidTransition(
                "supplier bill value change requires an open approved bill".into(),
            ));
        }
        Ok(())
    }
}

impl SupplierCredit {
    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    #[must_use]
    pub fn bill_id(&self) -> &EntityId {
        &self.bill_id
    }
    #[must_use]
    pub const fn amount(&self) -> Money {
        self.amount
    }
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseOrderLine {
    id: EntityId,
    description: String,
    quantity: Quantity,
    unit_cost: Money,
}

impl PurchaseOrderLine {
    pub fn new(
        id: EntityId,
        description: impl Into<String>,
        quantity: Quantity,
        unit_cost: Money,
    ) -> DomainResult<Self> {
        Ok(Self {
            id,
            description: bounded_text(description, "purchase order line description", 500)?,
            quantity,
            unit_cost: Money::non_negative(unit_cost.minor())?,
        })
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    #[must_use]
    pub const fn quantity(&self) -> Quantity {
        self.quantity
    }
    #[must_use]
    pub const fn unit_cost(&self) -> Money {
        self.unit_cost
    }
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseOrderState {
    Draft,
    Issued,
    PartReceived,
    Received,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillComparisonLine {
    pub po_line_id: EntityId,
    pub quantity: Quantity,
    pub unit_cost: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseVarianceLine {
    pub po_line_id: EntityId,
    pub ordered_quantity_subunits: i64,
    pub billed_quantity_subunits: i64,
    pub ordered_unit_cost_minor: i64,
    pub billed_unit_cost_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseVariance {
    pub lines: Vec<PurchaseVarianceLine>,
    pub has_variance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseOrder {
    id: EntityId,
    supplier: CommercialPartySnapshot,
    lines: Vec<PurchaseOrderLine>,
    state: PurchaseOrderState,
    received: BTreeMap<EntityId, i64>,
}

impl PurchaseOrder {
    #[must_use]
    pub fn new(id: EntityId, supplier: CommercialPartySnapshot) -> Self {
        Self {
            id,
            supplier,
            lines: Vec::new(),
            state: PurchaseOrderState::Draft,
            received: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    #[must_use]
    pub fn supplier(&self) -> &CommercialPartySnapshot {
        &self.supplier
    }
    #[must_use]
    pub const fn state(&self) -> PurchaseOrderState {
        self.state
    }

    pub fn add_line(&mut self, line: PurchaseOrderLine) -> DomainResult<()> {
        if self.state != PurchaseOrderState::Draft {
            return Err(DomainError::InvalidTransition(
                "purchase order is immutable after issue".into(),
            ));
        }
        if self.lines.iter().any(|existing| existing.id == line.id) {
            return Err(DomainError::Conflict("duplicate purchase order line id".into()));
        }
        self.lines.push(line);
        Ok(())
    }

    pub fn issue(&mut self) -> DomainResult<()> {
        if self.state != PurchaseOrderState::Draft || self.lines.is_empty() {
            return Err(DomainError::InvalidTransition(
                "purchase order issue requires a non-empty draft".into(),
            ));
        }
        self.state = PurchaseOrderState::Issued;
        Ok(())
    }

    pub fn receive(&mut self, line_id: &EntityId, quantity: Quantity) -> DomainResult<()> {
        if !matches!(
            self.state,
            PurchaseOrderState::Issued | PurchaseOrderState::PartReceived
        ) {
            return Err(DomainError::InvalidTransition(
                "receiving requires an issued purchase order".into(),
            ));
        }
        let ordered = self
            .lines
            .iter()
            .find(|line| &line.id == line_id)
            .ok_or_else(|| DomainError::Invalid("purchase order line does not exist".into()))?
            .quantity
            .subunits();
        let prior = self.received.get(line_id).copied().unwrap_or(0);
        let next = prior
            .checked_add(quantity.subunits())
            .ok_or_else(|| DomainError::Overflow("purchase receipt quantity overflow".into()))?;
        if next > ordered {
            return Err(DomainError::Conflict(
                "purchase receipt exceeds ordered quantity".into(),
            ));
        }
        self.received.insert(line_id.clone(), next);
        self.state = if self.lines.iter().all(|line| {
            self.received.get(&line.id).copied().unwrap_or(0) == line.quantity.subunits()
        }) {
            PurchaseOrderState::Received
        } else {
            PurchaseOrderState::PartReceived
        };
        Ok(())
    }

    pub fn cancel(&mut self) -> DomainResult<()> {
        if !matches!(self.state, PurchaseOrderState::Draft | PurchaseOrderState::Issued) {
            return Err(DomainError::InvalidTransition(
                "purchase order with receipts cannot be cancelled".into(),
            ));
        }
        self.state = PurchaseOrderState::Cancelled;
        Ok(())
    }

    #[must_use]
    pub fn received_subunits(&self, line_id: &EntityId) -> i64 {
        self.received.get(line_id).copied().unwrap_or(0)
    }

    pub fn compare_bill(&self, bill_lines: &[BillComparisonLine]) -> DomainResult<PurchaseVariance> {
        if self.state == PurchaseOrderState::Draft {
            return Err(DomainError::InvalidTransition(
                "purchase order must be issued before bill comparison".into(),
            ));
        }

        let mut seen_bill_mappings = BTreeSet::new();
        for billed in bill_lines {
            if billed.unit_cost.minor() < 0 {
                return Err(DomainError::Invalid(
                    "bill comparison unit cost must not be negative".into(),
                ));
            }
            if !seen_bill_mappings.insert(billed.po_line_id.clone()) {
                return Err(DomainError::Conflict(
                    "duplicate bill comparison mapping for purchase order line".into(),
                ));
            }
        }

        let mut lines = Vec::with_capacity(self.lines.len());
        let mut has_variance = false;
        for ordered in &self.lines {
            let billed = bill_lines.iter().find(|line| line.po_line_id == ordered.id);
            let billed_quantity = billed.map_or(0, |line| line.quantity.subunits());
            let billed_cost = billed.map_or(0, |line| line.unit_cost.minor());
            let line_variance = billed_quantity != ordered.quantity.subunits()
                || billed_cost != ordered.unit_cost.minor();
            has_variance |= line_variance;
            lines.push(PurchaseVarianceLine {
                po_line_id: ordered.id.clone(),
                ordered_quantity_subunits: ordered.quantity.subunits(),
                billed_quantity_subunits: billed_quantity,
                ordered_unit_cost_minor: ordered.unit_cost.minor(),
                billed_unit_cost_minor: billed_cost,
            });
        }
        if bill_lines
            .iter()
            .any(|billed| !self.lines.iter().any(|ordered| ordered.id == billed.po_line_id))
        {
            has_variance = true;
        }
        Ok(PurchaseVariance { lines, has_variance })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::CommercialParty;

    fn supplier() -> CommercialPartySnapshot {
        CommercialParty::new(
            EntityId::new("supplier-1").unwrap(),
            "Supplier",
            None::<String>,
            None::<String>,
            None::<String>,
        )
        .unwrap()
        .snapshot()
    }

    fn bill_line(amount: i64) -> CommercialLine {
        CommercialLine::new(
            EntityId::new("bill-line-1").unwrap(),
            "Supplies",
            Quantity::positive(1).unwrap(),
            Money::positive(amount).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn supplier_bill_requires_explicit_approval_then_partial_and_final_payment() {
        let mut bill = SupplierBill::new(
            EntityId::new("bill-1").unwrap(),
            supplier(),
            "SUP-001",
        )
        .unwrap();
        bill.add_line(bill_line(10_000)).unwrap();
        assert!(bill.apply_payment(Money::positive(1_000).unwrap()).is_err());
        bill.approve().unwrap();
        bill.apply_payment(Money::positive(4_000).unwrap()).unwrap();
        assert_eq!(bill.state(), SupplierBillState::PartPaid);
        bill.apply_payment(Money::positive(6_000).unwrap()).unwrap();
        assert_eq!(bill.state(), SupplierBillState::Paid);
    }

    #[test]
    fn supplier_payment_overallocation_fails() {
        let mut bill = SupplierBill::new(
            EntityId::new("bill-1").unwrap(),
            supplier(),
            "SUP-001",
        )
        .unwrap();
        bill.add_line(bill_line(5_000)).unwrap();
        bill.approve().unwrap();
        assert!(bill.apply_payment(Money::positive(5_001).unwrap()).is_err());
    }

    #[test]
    fn supplier_credit_identity_is_distinct_from_bill() {
        let mut bill = SupplierBill::new(
            EntityId::new("bill-1").unwrap(),
            supplier(),
            "SUP-001",
        )
        .unwrap();
        bill.add_line(bill_line(5_000)).unwrap();
        bill.approve().unwrap();
        assert!(bill
            .create_credit(
                EntityId::new("bill-1").unwrap(),
                Money::positive(1_000).unwrap(),
                "credit"
            )
            .is_err());
        assert_eq!(bill.outstanding().unwrap().minor(), 5_000);
    }

    #[test]
    fn supplier_duplicate_key_is_canonical() {
        let first = SupplierBill::new(
            EntityId::new("bill-1").unwrap(),
            supplier(),
            " INV--001 ",
        )
        .unwrap();
        let second = SupplierBill::new(
            EntityId::new("bill-2").unwrap(),
            supplier(),
            "inv 001",
        )
        .unwrap();
        assert_eq!(first.duplicate_key(), second.duplicate_key());
    }

    #[test]
    fn purchase_order_receives_partially_then_fully_and_rejects_overreceipt() {
        let mut po = PurchaseOrder::new(EntityId::new("po-1").unwrap(), supplier());
        let line_id = EntityId::new("po-line-1").unwrap();
        po.add_line(
            PurchaseOrderLine::new(
                line_id.clone(),
                "Parts",
                Quantity::positive(10).unwrap(),
                Money::positive(500).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        po.issue().unwrap();
        po.receive(&line_id, Quantity::positive(4).unwrap()).unwrap();
        assert_eq!(po.state(), PurchaseOrderState::PartReceived);
        assert!(po.receive(&line_id, Quantity::positive(7).unwrap()).is_err());
        po.receive(&line_id, Quantity::positive(6).unwrap()).unwrap();
        assert_eq!(po.state(), PurchaseOrderState::Received);
    }

    #[test]
    fn po_to_bill_comparison_reports_variance_only() {
        let mut po = PurchaseOrder::new(EntityId::new("po-1").unwrap(), supplier());
        let line_id = EntityId::new("po-line-1").unwrap();
        po.add_line(
            PurchaseOrderLine::new(
                line_id.clone(),
                "Parts",
                Quantity::positive(10).unwrap(),
                Money::positive(500).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        po.issue().unwrap();
        let variance = po
            .compare_bill(&[BillComparisonLine {
                po_line_id: line_id,
                quantity: Quantity::positive(9).unwrap(),
                unit_cost: Money::positive(550).unwrap(),
            }])
            .unwrap();
        assert!(variance.has_variance);
        assert_eq!(po.state(), PurchaseOrderState::Issued);
    }

    #[test]
    fn po_to_bill_comparison_rejects_duplicate_line_mappings() {
        let mut po = PurchaseOrder::new(EntityId::new("po-1").unwrap(), supplier());
        let line_id = EntityId::new("po-line-1").unwrap();
        po.add_line(
            PurchaseOrderLine::new(
                line_id.clone(),
                "Parts",
                Quantity::positive(10).unwrap(),
                Money::positive(500).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        po.issue().unwrap();
        let duplicated = [
            BillComparisonLine {
                po_line_id: line_id.clone(),
                quantity: Quantity::positive(4).unwrap(),
                unit_cost: Money::positive(500).unwrap(),
            },
            BillComparisonLine {
                po_line_id: line_id,
                quantity: Quantity::positive(6).unwrap(),
                unit_cost: Money::positive(500).unwrap(),
            },
        ];
        assert!(po.compare_bill(&duplicated).is_err());
        assert_eq!(po.state(), PurchaseOrderState::Issued);
    }

    #[test]
    fn po_to_bill_comparison_rejects_negative_unit_cost() {
        let mut po = PurchaseOrder::new(EntityId::new("po-1").unwrap(), supplier());
        let line_id = EntityId::new("po-line-1").unwrap();
        po.add_line(
            PurchaseOrderLine::new(
                line_id.clone(),
                "Parts",
                Quantity::positive(10).unwrap(),
                Money::positive(500).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        po.issue().unwrap();
        assert!(po
            .compare_bill(&[BillComparisonLine {
                po_line_id: line_id,
                quantity: Quantity::positive(10).unwrap(),
                unit_cost: Money::from_minor(-1),
            }])
            .is_err());
    }
}
