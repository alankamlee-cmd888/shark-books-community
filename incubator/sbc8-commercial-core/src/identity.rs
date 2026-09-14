use crate::primitives::{bounded_text, optional_bounded_text, DomainResult, EntityId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommercialParty {
    id: EntityId,
    display_name: String,
    postal_address: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

impl CommercialParty {
    pub fn new(
        id: EntityId,
        display_name: impl Into<String>,
        postal_address: Option<impl Into<String>>,
        email: Option<impl Into<String>>,
        phone: Option<impl Into<String>>,
    ) -> DomainResult<Self> {
        Ok(Self {
            id,
            display_name: bounded_text(display_name, "display name", 200)?,
            postal_address: optional_bounded_text(postal_address, "postal address", 500)?,
            email: optional_bounded_text(email, "email", 254)?,
            phone: optional_bounded_text(phone, "phone", 64)?,
        })
    }

    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn snapshot(&self) -> CommercialPartySnapshot {
        CommercialPartySnapshot {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            postal_address: self.postal_address.clone(),
            email: self.email.clone(),
            phone: self.phone.clone(),
        }
    }

    pub fn rename(&mut self, value: impl Into<String>) -> DomainResult<()> {
        self.display_name = bounded_text(value, "display name", 200)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommercialPartySnapshot {
    pub(crate) id: EntityId,
    pub(crate) display_name: String,
    pub(crate) postal_address: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
}

impl CommercialPartySnapshot {
    #[must_use]
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn postal_address(&self) -> Option<&str> {
        self.postal_address.as_deref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    #[must_use]
    pub fn phone(&self) -> Option<&str> {
        self.phone.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_do_not_follow_later_party_edits() {
        let mut party = CommercialParty::new(
            EntityId::new("customer-1").unwrap(),
            "Original Name",
            None::<String>,
            Some("billing@example.test"),
            None::<String>,
        )
        .unwrap();
        let snapshot = party.snapshot();
        party.rename("Later Name").unwrap();
        assert_eq!(snapshot.display_name(), "Original Name");
        assert_eq!(party.display_name(), "Later Name");
    }
}
