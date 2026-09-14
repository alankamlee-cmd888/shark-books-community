use crate::identity::CommercialPartySnapshot;
use crate::primitives::{
    bounded_text, checked_line_total, CommercialNumber, DomainError, DomainResult, EntityId, Money,
    Quantity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommercialLine {
    id: EntityId,
    description: String,
    quantity: Quantity,
    unit_price: Money,
}

impl CommercialLine {
    pub fn new(
        id: EntityId,
        description: impl Into<String>,
        quantity: Quantity,
        unit_price: Money,
    ) -> DomainResult<Self> {
        if unit_price.minor() < 0 {
            return Err(DomainError::Invalid(
                "commercial unit price must not be negative".into(),
            ));
        }
        Ok(Self {
            id,
            description: bounded_text(description, "line description", 500)?,
            quantity,
            unit_price,
        })
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub const fn quantity(&self) -> Quantity {
        self.quantity
    }

    #[must_use]
    pub const fn unit_price(&self) -> Money {
        self.unit_price
    }

    pub fn total(&self) -> DomainResult<Money> {
        checked_line_total(self.quantity, self.unit_price)
    }
}

fn lines_total(lines: &[CommercialLine]) -> DomainResult<Money> {
    if lines.is_empty() {
        return Err(DomainError::Invalid(
            "commercial document requires at least one line".into(),
        ));
    }
    lines
        .iter()
        .try_fold(Money::zero(), |total, line| total.checked_add(line.total()?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteState {
    Draft,
    Issued,
    Accepted,
    Rejected,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedQuoteSnapshot {
    quote_id: EntityId,
    quote_number: CommercialNumber,
    customer: CommercialPartySnapshot,
    lines: Vec<CommercialLine>,
    total: Money,
}

impl IssuedQuoteSnapshot {
    #[must_use]
    pub fn quote_id(&self) -> &EntityId {
        &self.quote_id
    }
    #[must_use]
    pub fn quote_number(&self) -> &CommercialNumber {
        &self.quote_number
    }
    #[must_use]
    pub fn customer(&self) -> &CommercialPartySnapshot {
        &self.customer
    }
    #[must_use]
    pub fn lines(&self) -> &[CommercialLine] {
        &self.lines
    }
    #[must_use]
    pub const fn total(&self) -> Money {
        self.total
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    id: EntityId,
    customer: CommercialPartySnapshot,
    lines: Vec<CommercialLine>,
    state: QuoteState,
    issued: Option<IssuedQuoteSnapshot>,
}

impl Quote {
    #[must_use]
    pub fn new(id: EntityId, customer: CommercialPartySnapshot) -> Self {
        Self {
            id,
            customer,
            lines: Vec::new(),
            state: QuoteState::Draft,
            issued: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    #[must_use]
    pub const fn state(&self) -> QuoteState {
        self.state
    }

    #[must_use]
    pub fn issued_snapshot(&self) -> Option<&IssuedQuoteSnapshot> {
        self.issued.as_ref()
    }

    pub fn set_customer(&mut self, customer: CommercialPartySnapshot) -> DomainResult<()> {
        self.require_draft()?;
        self.customer = customer;
        Ok(())
    }

    pub fn add_line(&mut self, line: CommercialLine) -> DomainResult<()> {
        self.require_draft()?;
        if self.lines.iter().any(|existing| existing.id == line.id) {
            return Err(DomainError::Conflict("duplicate quote line id".into()));
        }
        self.lines.push(line);
        Ok(())
    }

    pub fn replace_line(&mut self, line: CommercialLine) -> DomainResult<()> {
        self.require_draft()?;
        let Some(existing) = self.lines.iter_mut().find(|existing| existing.id == line.id) else {
            return Err(DomainError::Invalid("quote line does not exist".into()));
        };
        *existing = line;
        Ok(())
    }

    pub fn issue(&mut self, number: CommercialNumber) -> DomainResult<&IssuedQuoteSnapshot> {
        self.require_draft()?;
        let total = lines_total(&self.lines)?;
        self.issued = Some(IssuedQuoteSnapshot {
            quote_id: self.id.clone(),
            quote_number: number,
            customer: self.customer.clone(),
            lines: self.lines.clone(),
            total,
        });
        self.state = QuoteState::Issued;
        Ok(self.issued.as_ref().expect("issued snapshot set"))
    }

    pub fn accept(&mut self) -> DomainResult<()> {
        self.transition_from_issued(QuoteState::Accepted)
    }
    pub fn reject(&mut self) -> DomainResult<()> {
        self.transition_from_issued(QuoteState::Rejected)
    }
    pub fn expire(&mut self) -> DomainResult<()> {
        self.transition_from_issued(QuoteState::Expired)
    }
    pub fn cancel(&mut self) -> DomainResult<()> {
        self.transition_from_issued(QuoteState::Cancelled)
    }

    pub fn to_draft_invoice(&self, new_invoice_id: EntityId) -> DomainResult<Invoice> {
        if self.state != QuoteState::Accepted {
            return Err(DomainError::InvalidTransition(
                "only an accepted quote can create a draft invoice".into(),
            ));
        }
        if new_invoice_id == self.id {
            return Err(DomainError::Conflict(
                "quote and invoice must have distinct identities".into(),
            ));
        }
        let snapshot = self
            .issued
            .as_ref()
            .ok_or_else(|| DomainError::Conflict("accepted quote has no issued snapshot".into()))?;
        Ok(Invoice::from_quote(
            new_invoice_id,
            self.id.clone(),
            snapshot.customer.clone(),
            snapshot.lines.clone(),
        ))
    }

    fn require_draft(&self) -> DomainResult<()> {
        if self.state != QuoteState::Draft {
            return Err(DomainError::InvalidTransition(
                "quote is immutable after issue".into(),
            ));
        }
        Ok(())
    }

    fn transition_from_issued(&mut self, target: QuoteState) -> DomainResult<()> {
        if self.state != QuoteState::Issued {
            return Err(DomainError::InvalidTransition(
                "quote transition requires issued state".into(),
            ));
        }
        self.state = target;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceState {
    Draft,
    Issued,
    PartPaid,
    Paid,
    Overdue,
    Void,
    Credited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedInvoiceSnapshot {
    invoice_id: EntityId,
    invoice_number: CommercialNumber,
    customer: CommercialPartySnapshot,
    lines: Vec<CommercialLine>,
    total: Money,
    source_quote_id: Option<EntityId>,
}

impl IssuedInvoiceSnapshot {
    #[must_use]
    pub fn invoice_id(&self) -> &EntityId {
        &self.invoice_id
    }
    #[must_use]
    pub fn invoice_number(&self) -> &CommercialNumber {
        &self.invoice_number
    }
    #[must_use]
    pub fn customer(&self) -> &CommercialPartySnapshot {
        &self.customer
    }
    #[must_use]
    pub fn lines(&self) -> &[CommercialLine] {
        &self.lines
    }
    #[must_use]
    pub const fn total(&self) -> Money {
        self.total
    }
    #[must_use]
    pub fn source_quote_id(&self) -> Option<&EntityId> {
        self.source_quote_id.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditNote {
    id: EntityId,
    invoice_id: EntityId,
    amount: Money,
    reason: String,
}

impl CreditNote {
    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    #[must_use]
    pub fn invoice_id(&self) -> &EntityId {
        &self.invoice_id
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
pub struct Invoice {
    id: EntityId,
    source_quote_id: Option<EntityId>,
    customer: CommercialPartySnapshot,
    lines: Vec<CommercialLine>,
    state: InvoiceState,
    issued: Option<IssuedInvoiceSnapshot>,
    paid: Money,
    credited: Money,
}

impl Invoice {
    #[must_use]
    pub fn new(id: EntityId, customer: CommercialPartySnapshot) -> Self {
        Self {
            id,
            source_quote_id: None,
            customer,
            lines: Vec::new(),
            state: InvoiceState::Draft,
            issued: None,
            paid: Money::zero(),
            credited: Money::zero(),
        }
    }

    fn from_quote(
        id: EntityId,
        quote_id: EntityId,
        customer: CommercialPartySnapshot,
        lines: Vec<CommercialLine>,
    ) -> Self {
        Self {
            id,
            source_quote_id: Some(quote_id),
            customer,
            lines,
            state: InvoiceState::Draft,
            issued: None,
            paid: Money::zero(),
            credited: Money::zero(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    #[must_use]
    pub const fn state(&self) -> InvoiceState {
        self.state
    }
    #[must_use]
    pub fn source_quote_id(&self) -> Option<&EntityId> {
        self.source_quote_id.as_ref()
    }
    #[must_use]
    pub fn issued_snapshot(&self) -> Option<&IssuedInvoiceSnapshot> {
        self.issued.as_ref()
    }
    #[must_use]
    pub const fn paid_amount(&self) -> Money {
        self.paid
    }
    #[must_use]
    pub const fn credited_amount(&self) -> Money {
        self.credited
    }

    pub fn add_line(&mut self, line: CommercialLine) -> DomainResult<()> {
        self.require_draft()?;
        if self.lines.iter().any(|existing| existing.id == line.id) {
            return Err(DomainError::Conflict("duplicate invoice line id".into()));
        }
        self.lines.push(line);
        Ok(())
    }

    pub fn replace_line(&mut self, line: CommercialLine) -> DomainResult<()> {
        self.require_draft()?;
        let Some(existing) = self.lines.iter_mut().find(|existing| existing.id == line.id) else {
            return Err(DomainError::Invalid("invoice line does not exist".into()));
        };
        *existing = line;
        Ok(())
    }

    pub fn issue(&mut self, number: CommercialNumber) -> DomainResult<&IssuedInvoiceSnapshot> {
        self.require_draft()?;
        let total = lines_total(&self.lines)?;
        self.issued = Some(IssuedInvoiceSnapshot {
            invoice_id: self.id.clone(),
            invoice_number: number,
            customer: self.customer.clone(),
            lines: self.lines.clone(),
            total,
            source_quote_id: self.source_quote_id.clone(),
        });
        self.state = InvoiceState::Issued;
        Ok(self.issued.as_ref().expect("issued snapshot set"))
    }

    pub fn apply_payment(&mut self, amount: Money) -> DomainResult<()> {
        let amount = Money::positive(amount.minor())?;
        self.require_open_for_value_change("payment")?;
        let outstanding = self.outstanding()?;
        if amount > outstanding {
            return Err(DomainError::Conflict(
                "invoice payment exceeds outstanding balance".into(),
            ));
        }
        self.paid = self.paid.checked_add(amount)?;
        self.state = if self.outstanding()?.minor() == 0 {
            InvoiceState::Paid
        } else {
            InvoiceState::PartPaid
        };
        Ok(())
    }

    pub fn create_credit_note(
        &mut self,
        credit_id: EntityId,
        amount: Money,
        reason: impl Into<String>,
    ) -> DomainResult<CreditNote> {
        let amount = Money::positive(amount.minor())?;
        self.require_open_for_value_change("credit")?;
        if credit_id == self.id {
            return Err(DomainError::Conflict(
                "credit note and invoice must have distinct identities".into(),
            ));
        }
        let outstanding = self.outstanding()?;
        if amount > outstanding {
            return Err(DomainError::Conflict(
                "credit note exceeds remaining creditable balance".into(),
            ));
        }
        let reason = bounded_text(reason, "credit note reason", 500)?;
        self.credited = self.credited.checked_add(amount)?;
        self.state = if self.outstanding()?.minor() == 0 {
            InvoiceState::Credited
        } else if self.paid.minor() > 0 {
            InvoiceState::PartPaid
        } else {
            InvoiceState::Issued
        };
        Ok(CreditNote {
            id: credit_id,
            invoice_id: self.id.clone(),
            amount,
            reason,
        })
    }

    pub fn refresh_due_state(&mut self, is_overdue: bool) -> DomainResult<()> {
        if matches!(self.state, InvoiceState::Draft | InvoiceState::Void | InvoiceState::Credited) {
            return Err(DomainError::InvalidTransition(
                "due state is unavailable for this invoice state".into(),
            ));
        }
        if self.outstanding()?.minor() == 0 {
            return Ok(());
        }
        self.state = if is_overdue {
            InvoiceState::Overdue
        } else if self.paid.minor() > 0 {
            InvoiceState::PartPaid
        } else {
            InvoiceState::Issued
        };
        Ok(())
    }

    pub fn void(&mut self) -> DomainResult<()> {
        if !matches!(self.state, InvoiceState::Issued | InvoiceState::Overdue)
            || self.paid.minor() != 0
            || self.credited.minor() != 0
        {
            return Err(DomainError::InvalidTransition(
                "only an unpaid, uncredited issued invoice can be voided".into(),
            ));
        }
        self.state = InvoiceState::Void;
        Ok(())
    }

    pub fn outstanding(&self) -> DomainResult<Money> {
        let total = self
            .issued
            .as_ref()
            .ok_or_else(|| DomainError::InvalidTransition("invoice is not issued".into()))?
            .total;
        total.checked_sub(self.paid)?.checked_sub(self.credited)
    }

    fn require_draft(&self) -> DomainResult<()> {
        if self.state != InvoiceState::Draft {
            return Err(DomainError::InvalidTransition(
                "invoice is immutable after issue".into(),
            ));
        }
        Ok(())
    }

    fn require_open_for_value_change(&self, operation: &str) -> DomainResult<()> {
        if !matches!(
            self.state,
            InvoiceState::Issued | InvoiceState::PartPaid | InvoiceState::Overdue
        ) {
            return Err(DomainError::InvalidTransition(format!(
                "invoice {operation} requires an open issued invoice"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::CommercialParty;

    fn customer(name: &str) -> CommercialPartySnapshot {
        CommercialParty::new(
            EntityId::new("customer-1").unwrap(),
            name,
            None::<String>,
            None::<String>,
            None::<String>,
        )
        .unwrap()
        .snapshot()
    }

    fn line(id: &str, amount: i64) -> CommercialLine {
        CommercialLine::new(
            EntityId::new(id).unwrap(),
            "Service",
            Quantity::positive(1).unwrap(),
            Money::non_negative(amount).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn quote_draft_mutates_then_issue_freezes_snapshot() {
        let mut quote = Quote::new(EntityId::new("quote-1").unwrap(), customer("Alpha"));
        quote.add_line(line("q-line", 10_000)).unwrap();
        let snapshot = quote
            .issue(CommercialNumber::new("Q-0001").unwrap())
            .unwrap()
            .clone();
        assert_eq!(snapshot.total().minor(), 10_000);
        assert!(quote.add_line(line("late", 1)).is_err());
        assert_eq!(snapshot.customer().display_name(), "Alpha");
    }

    #[test]
    fn invalid_quote_transition_fails_closed() {
        let mut quote = Quote::new(EntityId::new("quote-1").unwrap(), customer("Alpha"));
        assert!(quote.accept().is_err());
    }

    #[test]
    fn accepted_quote_creates_new_draft_invoice_identity() {
        let mut quote = Quote::new(EntityId::new("quote-1").unwrap(), customer("Alpha"));
        quote.add_line(line("q-line", 10_000)).unwrap();
        quote.issue(CommercialNumber::new("Q-1").unwrap()).unwrap();
        quote.accept().unwrap();
        let invoice = quote
            .to_draft_invoice(EntityId::new("invoice-1").unwrap())
            .unwrap();
        assert_eq!(quote.state(), QuoteState::Accepted);
        assert_eq!(invoice.state(), InvoiceState::Draft);
        assert_eq!(invoice.source_quote_id().unwrap().as_str(), "quote-1");
        assert_ne!(invoice.id(), quote.id());
    }

    #[test]
    fn invoice_partial_then_final_payment_is_exact() {
        let mut invoice = Invoice::new(EntityId::new("invoice-1").unwrap(), customer("Alpha"));
        invoice.add_line(line("i-line", 10_000)).unwrap();
        invoice
            .issue(CommercialNumber::new("INV-1").unwrap())
            .unwrap();
        invoice.apply_payment(Money::positive(4_000).unwrap()).unwrap();
        assert_eq!(invoice.state(), InvoiceState::PartPaid);
        assert_eq!(invoice.outstanding().unwrap().minor(), 6_000);
        invoice.apply_payment(Money::positive(6_000).unwrap()).unwrap();
        assert_eq!(invoice.state(), InvoiceState::Paid);
        assert_eq!(invoice.outstanding().unwrap().minor(), 0);
    }

    #[test]
    fn invoice_overpayment_fails() {
        let mut invoice = Invoice::new(EntityId::new("invoice-1").unwrap(), customer("Alpha"));
        invoice.add_line(line("i-line", 5_000)).unwrap();
        invoice
            .issue(CommercialNumber::new("INV-1").unwrap())
            .unwrap();
        assert!(invoice.apply_payment(Money::positive(5_001).unwrap()).is_err());
    }

    #[test]
    fn credit_note_is_distinct_and_bounded_by_outstanding() {
        let mut invoice = Invoice::new(EntityId::new("invoice-1").unwrap(), customer("Alpha"));
        invoice.add_line(line("i-line", 10_000)).unwrap();
        invoice
            .issue(CommercialNumber::new("INV-1").unwrap())
            .unwrap();
        invoice.apply_payment(Money::positive(2_000).unwrap()).unwrap();
        assert!(invoice
            .create_credit_note(
                EntityId::new("credit-1").unwrap(),
                Money::positive(8_001).unwrap(),
                "Too much"
            )
            .is_err());
        let credit = invoice
            .create_credit_note(
                EntityId::new("credit-1").unwrap(),
                Money::positive(8_000).unwrap(),
                "Correct balance"
            )
            .unwrap();
        assert_eq!(credit.invoice_id().as_str(), "invoice-1");
        assert_eq!(invoice.state(), InvoiceState::Credited);
        assert_eq!(invoice.outstanding().unwrap().minor(), 0);
    }

    #[test]
    fn issued_invoice_snapshot_is_immutable() {
        let mut invoice = Invoice::new(EntityId::new("invoice-1").unwrap(), customer("Alpha"));
        invoice.add_line(line("i-line", 10_000)).unwrap();
        let snapshot = invoice
            .issue(CommercialNumber::new("INV-1").unwrap())
            .unwrap()
            .clone();
        assert!(invoice.replace_line(line("i-line", 20_000)).is_err());
        assert_eq!(snapshot.total().minor(), 10_000);
    }
}
