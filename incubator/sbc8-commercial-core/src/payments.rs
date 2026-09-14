use std::collections::HashMap;

use crate::documents::Invoice;
use crate::primitives::{DomainError, DomainResult, EntityId, Money};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentIntentState {
    Created,
    Pending,
    Succeeded,
    Failed,
    Cancelled,
    Refunded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentEventKind {
    Pending,
    Succeeded,
    Failed,
    Cancelled,
    Refunded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentEvent {
    provider_event_id: EntityId,
    operation_id: EntityId,
    amount: Money,
    kind: PaymentEventKind,
    provider_object_id: Option<EntityId>,
}

impl PaymentEvent {
    pub fn new(
        provider_event_id: EntityId,
        operation_id: EntityId,
        amount: Money,
        kind: PaymentEventKind,
        provider_object_id: Option<EntityId>,
    ) -> DomainResult<Self> {
        let amount = Money::positive(amount.minor())?;
        Ok(Self {
            provider_event_id,
            operation_id,
            amount,
            kind,
            provider_object_id,
        })
    }

    #[must_use]
    pub fn provider_event_id(&self) -> &EntityId {
        &self.provider_event_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentIntent {
    operation_id: EntityId,
    invoice_id: EntityId,
    amount: Money,
    state: PaymentIntentState,
    provider_object_id: Option<EntityId>,
}

impl PaymentIntent {
    pub fn new(operation_id: EntityId, invoice_id: EntityId, amount: Money) -> DomainResult<Self> {
        Ok(Self {
            operation_id,
            invoice_id,
            amount: Money::positive(amount.minor())?,
            state: PaymentIntentState::Created,
            provider_object_id: None,
        })
    }

    #[must_use]
    pub fn operation_id(&self) -> &EntityId {
        &self.operation_id
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
    pub const fn state(&self) -> PaymentIntentState {
        self.state
    }
    #[must_use]
    pub fn provider_object_id(&self) -> Option<&EntityId> {
        self.provider_object_id.as_ref()
    }

    pub fn apply_verified_event(&mut self, event: &PaymentEvent) -> DomainResult<()> {
        if event.operation_id != self.operation_id {
            return Err(DomainError::Conflict(
                "payment event operation identity does not match intent".into(),
            ));
        }
        if event.amount != self.amount {
            return Err(DomainError::Conflict(
                "payment event amount does not match intent".into(),
            ));
        }
        if let (Some(existing), Some(incoming)) = (&self.provider_object_id, &event.provider_object_id)
        {
            if existing != incoming {
                return Err(DomainError::Conflict(
                    "payment provider object identity changed".into(),
                ));
            }
        }
        if self.provider_object_id.is_none() {
            self.provider_object_id = event.provider_object_id.clone();
        }

        self.state = match (self.state, event.kind) {
            (PaymentIntentState::Created, PaymentEventKind::Pending)
            | (PaymentIntentState::Pending, PaymentEventKind::Pending) => PaymentIntentState::Pending,
            (PaymentIntentState::Created, PaymentEventKind::Succeeded)
            | (PaymentIntentState::Pending, PaymentEventKind::Succeeded)
            | (PaymentIntentState::Succeeded, PaymentEventKind::Succeeded) => {
                PaymentIntentState::Succeeded
            }
            (PaymentIntentState::Created, PaymentEventKind::Failed)
            | (PaymentIntentState::Pending, PaymentEventKind::Failed)
            | (PaymentIntentState::Failed, PaymentEventKind::Failed) => PaymentIntentState::Failed,
            (PaymentIntentState::Created, PaymentEventKind::Cancelled)
            | (PaymentIntentState::Pending, PaymentEventKind::Cancelled)
            | (PaymentIntentState::Cancelled, PaymentEventKind::Cancelled) => {
                PaymentIntentState::Cancelled
            }
            (PaymentIntentState::Succeeded, PaymentEventKind::Refunded)
            | (PaymentIntentState::Refunded, PaymentEventKind::Refunded) => {
                PaymentIntentState::Refunded
            }
            _ => {
                return Err(DomainError::InvalidTransition(
                    "payment event is not valid for current intent state".into(),
                ));
            }
        };
        Ok(())
    }

    pub fn allocate_to_invoice(&self, invoice: &mut Invoice, amount: Money) -> DomainResult<()> {
        if self.state != PaymentIntentState::Succeeded {
            return Err(DomainError::InvalidTransition(
                "only a verified succeeded payment can be allocated".into(),
            ));
        }
        if invoice.id() != &self.invoice_id {
            return Err(DomainError::Conflict(
                "payment intent invoice identity does not match allocation target".into(),
            ));
        }
        if amount > self.amount {
            return Err(DomainError::Conflict(
                "invoice allocation exceeds payment intent amount".into(),
            ));
        }
        invoice.apply_payment(amount)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRecordOutcome {
    Recorded,
    AlreadySeen,
}

#[derive(Debug, Default)]
pub struct PaymentEventRegistry {
    events: HashMap<EntityId, PaymentEvent>,
}

impl PaymentEventRegistry {
    pub fn record(&mut self, event: PaymentEvent) -> DomainResult<EventRecordOutcome> {
        if let Some(existing) = self.events.get(event.provider_event_id()) {
            if existing == &event {
                return Ok(EventRecordOutcome::AlreadySeen);
            }
            return Err(DomainError::Conflict(
                "provider event id was reused with conflicting immutable content".into(),
            ));
        }
        self.events.insert(event.provider_event_id.clone(), event);
        Ok(EventRecordOutcome::Recorded)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    id: EntityId,
    gross: Money,
    fee: Money,
    net: Money,
}

impl Settlement {
    pub fn new(id: EntityId, gross: Money, fee: Money, net: Money) -> DomainResult<Self> {
        let gross = Money::positive(gross.minor())?;
        let fee = Money::non_negative(fee.minor())?;
        let net = Money::non_negative(net.minor())?;
        if fee > gross {
            return Err(DomainError::Invalid(
                "settlement fee cannot exceed gross".into(),
            ));
        }
        if gross.checked_sub(fee)? != net {
            return Err(DomainError::Conflict(
                "settlement must satisfy gross minus fee equals net".into(),
            ));
        }
        Ok(Self { id, gross, fee, net })
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    #[must_use]
    pub const fn gross(&self) -> Money {
        self.gross
    }
    #[must_use]
    pub const fn fee(&self) -> Money {
        self.fee
    }
    #[must_use]
    pub const fn net(&self) -> Money {
        self.net
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::{CommercialLine, Invoice, InvoiceState};
    use crate::identity::CommercialParty;
    use crate::primitives::{CommercialNumber, Quantity};

    fn invoice() -> Invoice {
        let customer = CommercialParty::new(
            EntityId::new("customer-1").unwrap(),
            "Customer",
            None::<String>,
            None::<String>,
            None::<String>,
        )
        .unwrap()
        .snapshot();
        let mut invoice = Invoice::new(EntityId::new("invoice-1").unwrap(), customer);
        invoice
            .add_line(
                CommercialLine::new(
                    EntityId::new("line-1").unwrap(),
                    "Service",
                    Quantity::positive(1).unwrap(),
                    Money::positive(10_000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        invoice
            .issue(CommercialNumber::new("INV-1").unwrap())
            .unwrap();
        invoice
    }

    fn success_event() -> PaymentEvent {
        PaymentEvent::new(
            EntityId::new("event-1").unwrap(),
            EntityId::new("operation-1").unwrap(),
            Money::positive(10_000).unwrap(),
            PaymentEventKind::Succeeded,
            Some(EntityId::new("provider-object-1").unwrap()),
        )
        .unwrap()
    }

    #[test]
    fn provider_event_registry_is_idempotent_and_conflict_closed() {
        let event = success_event();
        let mut registry = PaymentEventRegistry::default();
        assert_eq!(registry.record(event.clone()).unwrap(), EventRecordOutcome::Recorded);
        assert_eq!(
            registry.record(event.clone()).unwrap(),
            EventRecordOutcome::AlreadySeen
        );
        let conflict = PaymentEvent::new(
            EntityId::new("event-1").unwrap(),
            EntityId::new("operation-1").unwrap(),
            Money::positive(10_000).unwrap(),
            PaymentEventKind::Failed,
            Some(EntityId::new("provider-object-1").unwrap()),
        )
        .unwrap();
        assert!(registry.record(conflict).is_err());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn verified_success_does_not_allocate_invoice_without_explicit_action() {
        let mut invoice = invoice();
        let mut intent = PaymentIntent::new(
            EntityId::new("operation-1").unwrap(),
            invoice.id().clone(),
            Money::positive(10_000).unwrap(),
        )
        .unwrap();
        intent.apply_verified_event(&success_event()).unwrap();
        assert_eq!(intent.state(), PaymentIntentState::Succeeded);
        assert_eq!(invoice.state(), InvoiceState::Issued);
        assert_eq!(invoice.outstanding().unwrap().minor(), 10_000);
        intent
            .allocate_to_invoice(&mut invoice, Money::positive(10_000).unwrap())
            .unwrap();
        assert_eq!(invoice.state(), InvoiceState::Paid);
    }

    #[test]
    fn settlement_arithmetic_is_exact() {
        let settlement = Settlement::new(
            EntityId::new("settlement-1").unwrap(),
            Money::positive(10_000).unwrap(),
            Money::non_negative(300).unwrap(),
            Money::non_negative(9_700).unwrap(),
        )
        .unwrap();
        assert_eq!(settlement.net().minor(), 9_700);
        assert!(Settlement::new(
            EntityId::new("settlement-2").unwrap(),
            Money::positive(10_000).unwrap(),
            Money::non_negative(300).unwrap(),
            Money::non_negative(9_699).unwrap(),
        )
        .is_err());
    }
}
