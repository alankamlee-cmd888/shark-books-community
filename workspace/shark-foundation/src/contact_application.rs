//! SBC-7B1 Batch B minimal Shark-owned customer/supplier contact persistence.
//!
//! Contacts are supporting application data only. They do not create accounts,
//! postings, tax decisions or external/network side effects.

use super::*;

const MAX_CONTACT_ID_BYTES: usize = 128;
const MAX_CONTACT_NAME_BYTES: usize = 256;
pub(crate) const MAX_CONTACT_PAGE: i64 = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactWrite {
    pub contact_id: String,
    pub kind: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactView {
    pub contact_id: String,
    pub kind: String,
    pub display_name: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_by: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContactPersistOutcome {
    Created(ContactView),
    Updated(ContactView),
    AlreadyCurrent(ContactView),
}

fn validation(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Validation, message)
}

fn validate_contact_id(value: &str) -> FoundationResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_CONTACT_ID_BYTES
        || trimmed.chars().any(|character| character.is_control() || character == '\0')
    {
        return Err(validation(
            "contact id must be 1-128 bounded non-control characters",
        ));
    }
    Ok(())
}

fn validate_contact_kind(value: &str) -> FoundationResult<()> {
    if !matches!(value, "customer" | "supplier") {
        return Err(validation("contact kind must be customer or supplier"));
    }
    Ok(())
}

fn validate_contact_name(value: &str) -> FoundationResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_CONTACT_NAME_BYTES
        || trimmed.chars().any(|character| character == '\0')
    {
        return Err(validation(
            "contact display name must be 1-256 bounded non-blank characters",
        ));
    }
    Ok(())
}

fn normalized_write(write: &ContactWrite) -> FoundationResult<ContactWrite> {
    validate_contact_id(&write.contact_id)?;
    validate_contact_kind(&write.kind)?;
    validate_contact_name(&write.display_name)?;
    Ok(ContactWrite {
        contact_id: write.contact_id.trim().to_string(),
        kind: write.kind.clone(),
        display_name: write.display_name.trim().to_string(),
    })
}

impl Books {
    fn contact_by_id_optional(&self, contact_id: &str) -> FoundationResult<Option<ContactView>> {
        validate_contact_id(contact_id)?;
        let count: i64 = self
            .db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM shark_contact WHERE company_slug = ?1 AND contact_id = ?2",
                (&self.company_slug, contact_id),
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if count == 0 {
            return Ok(None);
        }
        if count != 1 {
            return Err(FoundationError::new(
                FoundationErrorCode::Storage,
                "contact identity is not unique",
            ));
        }
        self.db
            .conn()
            .query_row(
                "SELECT contact_id, kind, display_name, created_by, created_at, updated_by, updated_at
                 FROM shark_contact
                 WHERE company_slug = ?1 AND contact_id = ?2",
                (&self.company_slug, contact_id),
                |row| {
                    Ok(ContactView {
                        contact_id: row.get(0)?,
                        kind: row.get(1)?,
                        display_name: row.get(2)?,
                        created_by: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_by: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .map(Some)
            .map_err(sqlite_error)
    }

    pub fn contact(&self, contact_id: &str) -> FoundationResult<ContactView> {
        self.contact_by_id_optional(contact_id)?.ok_or_else(|| {
            FoundationError::new(FoundationErrorCode::NotFound, "contact was not found")
        })
    }

    pub fn save_contact(&self, write: &ContactWrite) -> FoundationResult<ContactPersistOutcome> {
        let write = normalized_write(write)?;
        if let Some(existing) = self.contact_by_id_optional(&write.contact_id)? {
            if existing.kind != write.kind {
                return Err(validation(
                    "contact id already exists with a different immutable kind",
                ));
            }
            if existing.display_name == write.display_name {
                return Ok(ContactPersistOutcome::AlreadyCurrent(existing));
            }
            self.db
                .conn()
                .execute(
                    "UPDATE shark_contact
                     SET display_name = ?1, updated_by = ?2, updated_at = CURRENT_TIMESTAMP
                     WHERE company_slug = ?3 AND contact_id = ?4 AND kind = ?5",
                    (
                        &write.display_name,
                        self.actor.name(),
                        &self.company_slug,
                        &write.contact_id,
                        &write.kind,
                    ),
                )
                .map_err(sqlite_error)?;
            return self
                .contact(&write.contact_id)
                .map(ContactPersistOutcome::Updated);
        }

        self.db
            .conn()
            .execute(
                "INSERT INTO shark_contact(
                    company_slug, contact_id, kind, display_name,
                    created_by, updated_by
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?5)",
                (
                    &self.company_slug,
                    &write.contact_id,
                    &write.kind,
                    &write.display_name,
                    self.actor.name(),
                ),
            )
            .map_err(sqlite_error)?;
        self.contact(&write.contact_id)
            .map(ContactPersistOutcome::Created)
    }

    pub fn contacts(&self, kind: Option<&str>, limit: i64) -> FoundationResult<Vec<ContactView>> {
        if let Some(kind) = kind {
            validate_contact_kind(kind)?;
        }
        if !(1..=MAX_CONTACT_PAGE).contains(&limit) {
            return Err(validation("contact list limit must be between 1 and 500"));
        }
        let mut statement = self
            .db
            .conn()
            .prepare(
                "SELECT contact_id, kind, display_name, created_by, created_at, updated_by, updated_at
                 FROM shark_contact
                 WHERE company_slug = ?1 AND (?2 IS NULL OR kind = ?2)
                 ORDER BY CASE kind WHEN 'customer' THEN 0 ELSE 1 END,
                          lower(display_name) ASC,
                          contact_id ASC
                 LIMIT ?3",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, kind, limit), |row| {
                Ok(ContactView {
                    contact_id: row.get(0)?,
                    kind: row.get(1)?,
                    display_name: row.get(2)?,
                    created_by: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_by: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(sqlite_error)?;
        rows.map(|row| row.map_err(sqlite_error)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "shark-sbc7b1-contact-{label}-{}-{nonce}.db",
            std::process::id()
        ))
    }

    #[test]
    fn contacts_create_update_list_are_idempotent_and_non_posting() {
        let path = temp_db_path("lifecycle");
        let books = Books::create_plain_for_test(
            &path,
            "contact-books",
            "Contact Books",
            "local-owner",
        )
        .expect("create test books");
        let before = books.count_transactions().expect("before transaction count");

        let customer = ContactWrite {
            contact_id: "customer-1".into(),
            kind: "customer".into(),
            display_name: "Acme Customer".into(),
        };
        let created = books.save_contact(&customer).expect("create contact");
        assert!(matches!(created, ContactPersistOutcome::Created(_)));
        let repeated = books.save_contact(&customer).expect("repeat contact");
        assert!(matches!(repeated, ContactPersistOutcome::AlreadyCurrent(_)));

        let mut renamed = customer.clone();
        renamed.display_name = "Acme Customer Updated".into();
        let updated = books.save_contact(&renamed).expect("update contact");
        assert!(matches!(updated, ContactPersistOutcome::Updated(_)));

        let supplier = ContactWrite {
            contact_id: "supplier-1".into(),
            kind: "supplier".into(),
            display_name: "Beta Supplier".into(),
        };
        books.save_contact(&supplier).expect("create supplier");

        let all = books.contacts(None, 200).expect("list contacts");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].contact_id, "customer-1");
        assert_eq!(all[1].contact_id, "supplier-1");
        let suppliers = books.contacts(Some("supplier"), 200).expect("list suppliers");
        assert_eq!(suppliers.len(), 1);
        assert_eq!(suppliers[0].contact_id, "supplier-1");

        assert_eq!(
            books.count_transactions().expect("after transaction count"),
            before,
            "contact persistence must not post accounting transactions"
        );

        drop(books);
        remove_sqlite_artifacts(&path);
    }

    #[test]
    fn contact_kind_conflict_and_invalid_values_fail_closed() {
        let path = temp_db_path("validation");
        let books = Books::create_plain_for_test(
            &path,
            "contact-books",
            "Contact Books",
            "local-owner",
        )
        .expect("create test books");
        books
            .save_contact(&ContactWrite {
                contact_id: "shared-id".into(),
                kind: "customer".into(),
                display_name: "Customer".into(),
            })
            .expect("seed customer");
        assert!(books
            .save_contact(&ContactWrite {
                contact_id: "shared-id".into(),
                kind: "supplier".into(),
                display_name: "Supplier".into(),
            })
            .is_err());
        assert!(books
            .save_contact(&ContactWrite {
                contact_id: " ".into(),
                kind: "customer".into(),
                display_name: "Name".into(),
            })
            .is_err());
        assert!(books
            .save_contact(&ContactWrite {
                contact_id: "valid".into(),
                kind: "partner".into(),
                display_name: "Name".into(),
            })
            .is_err());
        assert!(books.contacts(None, 0).is_err());
        assert!(books.contacts(None, 501).is_err());

        let existing = books.contact("shared-id").expect("existing customer");
        assert_eq!(existing.kind, "customer");
        assert_eq!(existing.display_name, "Customer");

        drop(books);
        remove_sqlite_artifacts(&path);
    }
}
