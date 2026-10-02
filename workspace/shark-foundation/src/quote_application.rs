//! SBC-8A2 non-posting quote and estimate persistence.
//!
//! Commercial documents live in Shark-owned application tables. Issuing copies
//! customer and line facts into append-only snapshot tables. Every successful
//! mutation writes owner-visible history in the same savepoint; no method in
//! this module creates an accounting transaction or an invoice.

use super::*;
use crate::bank_application::sqlite_error;

const MAX_QUOTE_ID_BYTES: usize = 128;
const MAX_COMMERCIAL_NUMBER_BYTES: usize = 64;
const MAX_LINE_DESCRIPTION_BYTES: usize = 500;
pub(crate) const MAX_QUOTE_PAGE: i64 = 500;
pub(crate) const MAX_QUOTE_HISTORY_PAGE: i64 = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QuoteKind {
    Quote,
    Estimate,
}

impl QuoteKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Estimate => "estimate",
        }
    }

    fn from_persisted(value: &str) -> FoundationResult<Self> {
        match value {
            "quote" => Ok(Self::Quote),
            "estimate" => Ok(Self::Estimate),
            _ => Err(storage("persisted commercial document kind is invalid")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QuoteState {
    Draft,
    Issued,
    Accepted,
    Rejected,
    Expired,
    Cancelled,
}

impl QuoteState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Issued => "issued",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_persisted(value: &str) -> FoundationResult<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "issued" => Ok(Self::Issued),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(storage("persisted commercial document state is invalid")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteDraftWrite {
    pub quote_id: String,
    pub kind: QuoteKind,
    pub customer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteLineWrite {
    pub line_id: String,
    pub description: String,
    pub quantity_subunits: i64,
    pub unit_price_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteLineView {
    pub line_id: String,
    pub description: String,
    pub quantity_subunits: i64,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteCustomerSnapshotView {
    pub customer_id: String,
    pub display_name: String,
    pub postal_address: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedQuoteSnapshotView {
    pub commercial_number: String,
    pub kind: QuoteKind,
    pub customer: QuoteCustomerSnapshotView,
    pub lines: Vec<QuoteLineView>,
    pub total_minor: i64,
    pub issued_by: String,
    pub issued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteView {
    pub quote_id: String,
    pub kind: QuoteKind,
    pub state: QuoteState,
    pub customer: QuoteCustomerSnapshotView,
    pub lines: Vec<QuoteLineView>,
    pub total_minor: i64,
    pub issued_snapshot: Option<IssuedQuoteSnapshotView>,
    pub conversion_eligible: bool,
    pub created_by: String,
    pub created_at: String,
    pub updated_by: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteMutationView {
    pub mutation_id: i64,
    pub quote_id: String,
    pub action: String,
    pub before: Option<QuoteView>,
    pub after: Option<QuoteView>,
    pub actor: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuoteMutationOutcome {
    pub quote: QuoteView,
    pub mutation: QuoteMutationView,
}

fn validation(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Validation, message)
}

fn storage(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Storage, message)
}

fn safe_text(value: &str, field: &str, max: usize) -> FoundationResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > max
        || trimmed
            .chars()
            .any(|character| character == '\0' || character.is_control())
    {
        return Err(validation(format!(
            "{field} must be 1-{max} bounded non-control characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalized_quote_id(value: &str) -> FoundationResult<String> {
    safe_text(value, "quote or estimate id", MAX_QUOTE_ID_BYTES)
}

fn normalized_line(write: &QuoteLineWrite) -> FoundationResult<QuoteLineWrite> {
    let line_id = safe_text(&write.line_id, "commercial line id", MAX_QUOTE_ID_BYTES)?;
    let description = safe_text(
        &write.description,
        "commercial line description",
        MAX_LINE_DESCRIPTION_BYTES,
    )?;
    if write.quantity_subunits <= 0 {
        return Err(validation(
            "commercial line quantity must be positive integer subunits",
        ));
    }
    if write.unit_price_minor < 0 {
        return Err(validation(
            "commercial line unit price must be non-negative whole pence",
        ));
    }
    checked_line_total(write.quantity_subunits, write.unit_price_minor)?;
    Ok(QuoteLineWrite {
        line_id,
        description,
        quantity_subunits: write.quantity_subunits,
        unit_price_minor: write.unit_price_minor,
    })
}

fn checked_line_total(quantity_subunits: i64, unit_price_minor: i64) -> FoundationResult<i64> {
    let total = i128::from(quantity_subunits)
        .checked_mul(i128::from(unit_price_minor))
        .ok_or_else(|| validation("commercial line total overflow"))?;
    i64::try_from(total).map_err(|_| validation("commercial line total overflow"))
}

fn checked_lines_total(lines: &[QuoteLineView]) -> FoundationResult<i64> {
    lines.iter().try_fold(0_i64, |total, line| {
        total
            .checked_add(line.total_minor)
            .ok_or_else(|| validation("commercial document total overflow"))
    })
}

fn parse_snapshot_json(value: Option<String>) -> FoundationResult<Option<QuoteView>> {
    value
        .map(|json| {
            serde_json::from_str(&json)
                .map_err(|_| storage("persisted quote mutation snapshot is invalid"))
        })
        .transpose()
}

impl Books {
    fn customer_snapshot_for_quote(
        &self,
        customer_id: &str,
    ) -> FoundationResult<QuoteCustomerSnapshotView> {
        let customer_id = safe_text(customer_id, "customer id", MAX_QUOTE_ID_BYTES)?;
        let contact = self.contact(&customer_id)?;
        if contact.kind != "customer" {
            return Err(validation(
                "quotes and estimates require an existing customer contact",
            ));
        }
        Ok(QuoteCustomerSnapshotView {
            customer_id: contact.contact_id,
            display_name: contact.display_name,
            postal_address: contact.postal_address,
            email: contact.email,
            phone: contact.phone,
        })
    }

    fn quote_lines_from_table(
        &self,
        quote_id: &str,
        table: &'static str,
    ) -> FoundationResult<Vec<QuoteLineView>> {
        let sql = match table {
            "shark_quote_line" => {
                "SELECT line_id, description, quantity_subunits, unit_price_minor, position
                 FROM shark_quote_line
                 WHERE company_slug = ?1 AND quote_id = ?2
                 ORDER BY position ASC, line_id ASC"
            }
            "shark_quote_issue_line" => {
                "SELECT line_id, description, quantity_subunits, unit_price_minor, position
                 FROM shark_quote_issue_line
                 WHERE company_slug = ?1 AND quote_id = ?2
                 ORDER BY position ASC, line_id ASC"
            }
            _ => return Err(storage("unsupported commercial line storage source")),
        };
        let mut statement = self.db.conn().prepare(sql).map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, quote_id), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(sqlite_error)?;
        rows.map(|row| {
            let (line_id, description, quantity_subunits, unit_price_minor, position) =
                row.map_err(sqlite_error)?;
            Ok(QuoteLineView {
                line_id,
                description,
                quantity_subunits,
                unit_price_minor,
                total_minor: checked_line_total(quantity_subunits, unit_price_minor)?,
                position,
            })
        })
        .collect()
    }

    fn quote_by_id_optional(&self, quote_id: &str) -> FoundationResult<Option<QuoteView>> {
        let quote_id = normalized_quote_id(quote_id)?;
        let count: i64 = self
            .db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM shark_quote WHERE company_slug = ?1 AND quote_id = ?2",
                (&self.company_slug, &quote_id),
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if count == 0 {
            return Ok(None);
        }
        if count != 1 {
            return Err(storage("quote or estimate identity is not unique"));
        }

        let row = self
            .db
            .conn()
            .query_row(
                "SELECT document_kind, state, customer_id, customer_display_name,
                        customer_postal_address, customer_email, customer_phone,
                        conversion_eligible, created_by, created_at, updated_by, updated_at
                 FROM shark_quote
                 WHERE company_slug = ?1 AND quote_id = ?2",
                (&self.company_slug, &quote_id),
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                    ))
                },
            )
            .map_err(sqlite_error)?;
        let kind = QuoteKind::from_persisted(&row.0)?;
        let state = QuoteState::from_persisted(&row.1)?;
        let persisted_conversion_eligible = match row.7 {
            0 => false,
            1 => true,
            _ => return Err(storage("persisted conversion eligibility is invalid")),
        };
        if persisted_conversion_eligible != (state == QuoteState::Accepted) {
            return Err(storage(
                "persisted conversion eligibility conflicts with commercial state",
            ));
        }
        let conversion_count: i64 = self
            .db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM shark_quote_conversion
                 WHERE company_slug = ?1 AND quote_id = ?2",
                (&self.company_slug, &quote_id),
                |conversion_row| conversion_row.get(0),
            )
            .map_err(sqlite_error)?;
        if conversion_count > 1 {
            return Err(storage("quote conversion identity is not unique"));
        }
        let conversion_eligible = persisted_conversion_eligible && conversion_count == 0;
        let customer = QuoteCustomerSnapshotView {
            customer_id: row.2,
            display_name: row.3,
            postal_address: row.4,
            email: row.5,
            phone: row.6,
        };
        let lines = self.quote_lines_from_table(&quote_id, "shark_quote_line")?;
        let draft_total = checked_lines_total(&lines)?;

        let snapshot_count: i64 = self
            .db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM shark_quote_issue_snapshot
                 WHERE company_slug = ?1 AND quote_id = ?2",
                (&self.company_slug, &quote_id),
                |snapshot_row| snapshot_row.get(0),
            )
            .map_err(sqlite_error)?;
        let issued_snapshot = if snapshot_count == 0 {
            None
        } else if snapshot_count == 1 {
            let snapshot = self
                .db
                .conn()
                .query_row(
                    "SELECT commercial_number, document_kind, customer_id,
                            customer_display_name, customer_postal_address, customer_email,
                            customer_phone, total_minor, line_count, issued_by, issued_at
                     FROM shark_quote_issue_snapshot
                     WHERE company_slug = ?1 AND quote_id = ?2",
                    (&self.company_slug, &quote_id),
                    |snapshot_row| {
                        Ok((
                            snapshot_row.get::<_, String>(0)?,
                            snapshot_row.get::<_, String>(1)?,
                            snapshot_row.get::<_, String>(2)?,
                            snapshot_row.get::<_, String>(3)?,
                            snapshot_row.get::<_, Option<String>>(4)?,
                            snapshot_row.get::<_, Option<String>>(5)?,
                            snapshot_row.get::<_, Option<String>>(6)?,
                            snapshot_row.get::<_, i64>(7)?,
                            snapshot_row.get::<_, i64>(8)?,
                            snapshot_row.get::<_, String>(9)?,
                            snapshot_row.get::<_, String>(10)?,
                        ))
                    },
                )
                .map_err(sqlite_error)?;
            let snapshot_kind = QuoteKind::from_persisted(&snapshot.1)?;
            let snapshot_lines =
                self.quote_lines_from_table(&quote_id, "shark_quote_issue_line")?;
            if usize::try_from(snapshot.8).ok() != Some(snapshot_lines.len()) {
                return Err(storage("issued commercial snapshot line count is invalid"));
            }
            if checked_lines_total(&snapshot_lines)? != snapshot.7 {
                return Err(storage("issued commercial snapshot total is invalid"));
            }
            Some(IssuedQuoteSnapshotView {
                commercial_number: snapshot.0,
                kind: snapshot_kind,
                customer: QuoteCustomerSnapshotView {
                    customer_id: snapshot.2,
                    display_name: snapshot.3,
                    postal_address: snapshot.4,
                    email: snapshot.5,
                    phone: snapshot.6,
                },
                lines: snapshot_lines,
                total_minor: snapshot.7,
                issued_by: snapshot.9,
                issued_at: snapshot.10,
            })
        } else {
            return Err(storage("issued commercial snapshot identity is not unique"));
        };
        if state == QuoteState::Draft && issued_snapshot.is_some() {
            return Err(storage(
                "draft commercial document unexpectedly has an issued snapshot",
            ));
        }
        if state != QuoteState::Draft && issued_snapshot.is_none() {
            return Err(storage(
                "issued commercial document is missing its immutable snapshot",
            ));
        }
        let total_minor = issued_snapshot
            .as_ref()
            .map_or(draft_total, |snapshot| snapshot.total_minor);
        Ok(Some(QuoteView {
            quote_id,
            kind,
            state,
            customer,
            lines,
            total_minor,
            issued_snapshot,
            conversion_eligible,
            created_by: row.8,
            created_at: row.9,
            updated_by: row.10,
            updated_at: row.11,
        }))
    }

    pub fn quote(&self, quote_id: &str) -> FoundationResult<QuoteView> {
        self.quote_by_id_optional(quote_id)?.ok_or_else(|| {
            FoundationError::new(
                FoundationErrorCode::NotFound,
                "quote or estimate was not found",
            )
        })
    }

    pub fn quotes(
        &self,
        kind: Option<QuoteKind>,
        state: Option<QuoteState>,
        limit: i64,
    ) -> FoundationResult<Vec<QuoteView>> {
        if !(1..=MAX_QUOTE_PAGE).contains(&limit) {
            return Err(validation("quote list limit must be between 1 and 500"));
        }
        let kind = kind.map(QuoteKind::as_str);
        let state = state.map(QuoteState::as_str);
        let mut statement = self
            .db
            .conn()
            .prepare(
                "SELECT quote_id FROM shark_quote
                 WHERE company_slug = ?1
                   AND (?2 IS NULL OR document_kind = ?2)
                   AND (?3 IS NULL OR state = ?3)
                 ORDER BY updated_at DESC, quote_id ASC
                 LIMIT ?4",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, kind, state, limit), |row| {
                row.get::<_, String>(0)
            })
            .map_err(sqlite_error)?;
        let ids = rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
        ids.into_iter()
            .map(|quote_id| self.quote(&quote_id))
            .collect()
    }

    fn quote_mutation_by_id(&self, mutation_id: i64) -> FoundationResult<QuoteMutationView> {
        let row = self
            .db
            .conn()
            .query_row(
                "SELECT quote_id, action, before_json, after_json, actor, occurred_at
                 FROM shark_quote_mutation
                 WHERE company_slug = ?1 AND id = ?2",
                (&self.company_slug, mutation_id),
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .map_err(sqlite_error)?;
        Ok(QuoteMutationView {
            mutation_id,
            quote_id: row.0,
            action: row.1,
            before: parse_snapshot_json(row.2)?,
            after: parse_snapshot_json(row.3)?,
            actor: row.4,
            occurred_at: row.5,
        })
    }

    fn record_quote_mutation(
        &self,
        quote_id: &str,
        action: &str,
        before: Option<&QuoteView>,
        after: Option<&QuoteView>,
    ) -> FoundationResult<QuoteMutationView> {
        let before_json = before
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| storage("could not serialize prior quote mutation facts"))?;
        let after_json = after
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| storage("could not serialize resulting quote mutation facts"))?;
        self.db
            .conn()
            .execute(
                "INSERT INTO shark_quote_mutation(
                    company_slug, quote_id, action, before_json, after_json, actor
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    &self.company_slug,
                    quote_id,
                    action,
                    before_json,
                    after_json,
                    self.actor.name(),
                ),
            )
            .map_err(sqlite_error)?;
        self.quote_mutation_by_id(self.db.conn().last_insert_rowid())
    }

    pub fn quote_history(
        &self,
        quote_id: &str,
        limit: i64,
    ) -> FoundationResult<Vec<QuoteMutationView>> {
        let quote_id = normalized_quote_id(quote_id)?;
        self.quote(&quote_id)?;
        if !(1..=MAX_QUOTE_HISTORY_PAGE).contains(&limit) {
            return Err(validation(
                "quote mutation history limit must be between 1 and 500",
            ));
        }
        let mut statement = self
            .db
            .conn()
            .prepare(
                "SELECT id FROM shark_quote_mutation
                 WHERE company_slug = ?1 AND quote_id = ?2
                 ORDER BY id DESC LIMIT ?3",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, &quote_id, limit), |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sqlite_error)?;
        let ids = rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
        ids.into_iter()
            .map(|mutation_id| self.quote_mutation_by_id(mutation_id))
            .collect()
    }

    pub fn create_quote_draft(
        &self,
        write: &QuoteDraftWrite,
    ) -> FoundationResult<QuoteMutationOutcome> {
        let quote_id = normalized_quote_id(&write.quote_id)?;
        let customer = self.customer_snapshot_for_quote(&write.customer_id)?;
        self.shark_savepoint("shark_quote_create", || {
            if self.quote_by_id_optional(&quote_id)?.is_some() {
                return Err(validation("quote or estimate id already exists"));
            }
            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_quote(
                        company_slug, quote_id, document_kind, state,
                        customer_id, customer_display_name, customer_postal_address,
                        customer_email, customer_phone, conversion_eligible,
                        created_by, updated_by
                     ) VALUES(?1, ?2, ?3, 'draft', ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9)",
                    (
                        &self.company_slug,
                        &quote_id,
                        write.kind.as_str(),
                        &customer.customer_id,
                        &customer.display_name,
                        &customer.postal_address,
                        &customer.email,
                        &customer.phone,
                        self.actor.name(),
                    ),
                )
                .map_err(sqlite_error)?;
            let quote = self.quote(&quote_id)?;
            let mutation =
                self.record_quote_mutation(&quote_id, "create_draft", None, Some(&quote))?;
            Ok(QuoteMutationOutcome { quote, mutation })
        })
    }

    pub fn set_quote_customer(
        &self,
        quote_id: &str,
        customer_id: &str,
    ) -> FoundationResult<QuoteMutationOutcome> {
        let quote_id = normalized_quote_id(quote_id)?;
        let customer = self.customer_snapshot_for_quote(customer_id)?;
        self.shark_savepoint("shark_quote_customer", || {
            let before = self.quote(&quote_id)?;
            if before.state != QuoteState::Draft {
                return Err(validation(
                    "quote or estimate customer is immutable after issue",
                ));
            }
            let changed = self
                .db
                .conn()
                .execute(
                    "UPDATE shark_quote
                     SET customer_id = ?1, customer_display_name = ?2,
                         customer_postal_address = ?3, customer_email = ?4,
                         customer_phone = ?5, updated_by = ?6, updated_at = CURRENT_TIMESTAMP
                     WHERE company_slug = ?7 AND quote_id = ?8 AND state = 'draft'",
                    (
                        &customer.customer_id,
                        &customer.display_name,
                        &customer.postal_address,
                        &customer.email,
                        &customer.phone,
                        self.actor.name(),
                        &self.company_slug,
                        &quote_id,
                    ),
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(validation("quote customer mutation lost draft authority"));
            }
            let quote = self.quote(&quote_id)?;
            let mutation =
                self.record_quote_mutation(&quote_id, "set_customer", Some(&before), Some(&quote))?;
            Ok(QuoteMutationOutcome { quote, mutation })
        })
    }

    pub fn save_quote_line(
        &self,
        quote_id: &str,
        write: &QuoteLineWrite,
    ) -> FoundationResult<QuoteMutationOutcome> {
        let quote_id = normalized_quote_id(quote_id)?;
        let write = normalized_line(write)?;
        self.shark_savepoint("shark_quote_line_save", || {
            let before = self.quote(&quote_id)?;
            if before.state != QuoteState::Draft {
                return Err(validation(
                    "quote or estimate lines are immutable after issue",
                ));
            }
            let existing_position = before
                .lines
                .iter()
                .find(|line| line.line_id == write.line_id)
                .map(|line| line.position);
            let position = existing_position.unwrap_or_else(|| {
                before
                    .lines
                    .iter()
                    .map(|line| line.position)
                    .max()
                    .unwrap_or(0)
                    + 1
            });
            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_quote_line(
                        company_slug, quote_id, line_id, description,
                        quantity_subunits, unit_price_minor, position
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(company_slug, quote_id, line_id) DO UPDATE SET
                        description = excluded.description,
                        quantity_subunits = excluded.quantity_subunits,
                        unit_price_minor = excluded.unit_price_minor",
                    (
                        &self.company_slug,
                        &quote_id,
                        &write.line_id,
                        &write.description,
                        write.quantity_subunits,
                        write.unit_price_minor,
                        position,
                    ),
                )
                .map_err(sqlite_error)?;
            let quote = self.quote(&quote_id)?;
            let action = if existing_position.is_some() {
                "replace_line"
            } else {
                "add_line"
            };
            let mutation =
                self.record_quote_mutation(&quote_id, action, Some(&before), Some(&quote))?;
            Ok(QuoteMutationOutcome { quote, mutation })
        })
    }

    pub fn remove_quote_line(
        &self,
        quote_id: &str,
        line_id: &str,
    ) -> FoundationResult<QuoteMutationOutcome> {
        let quote_id = normalized_quote_id(quote_id)?;
        let line_id = safe_text(line_id, "commercial line id", MAX_QUOTE_ID_BYTES)?;
        self.shark_savepoint("shark_quote_line_remove", || {
            let before = self.quote(&quote_id)?;
            if before.state != QuoteState::Draft {
                return Err(validation(
                    "quote or estimate lines are immutable after issue",
                ));
            }
            let changed = self
                .db
                .conn()
                .execute(
                    "DELETE FROM shark_quote_line
                     WHERE company_slug = ?1 AND quote_id = ?2 AND line_id = ?3",
                    (&self.company_slug, &quote_id, &line_id),
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(FoundationError::new(
                    FoundationErrorCode::NotFound,
                    "commercial line was not found",
                ));
            }
            let quote = self.quote(&quote_id)?;
            let mutation =
                self.record_quote_mutation(&quote_id, "remove_line", Some(&before), Some(&quote))?;
            Ok(QuoteMutationOutcome { quote, mutation })
        })
    }

    pub fn issue_quote(
        &self,
        quote_id: &str,
        commercial_number: &str,
    ) -> FoundationResult<QuoteMutationOutcome> {
        let quote_id = normalized_quote_id(quote_id)?;
        let commercial_number = safe_text(
            commercial_number,
            "commercial number",
            MAX_COMMERCIAL_NUMBER_BYTES,
        )?;
        self.shark_savepoint("shark_quote_issue", || {
            let before = self.quote(&quote_id)?;
            if before.state != QuoteState::Draft {
                return Err(validation("only a draft quote or estimate can be issued"));
            }
            if before.lines.is_empty() {
                return Err(validation(
                    "quote or estimate requires at least one line before issue",
                ));
            }
            let total_minor = checked_lines_total(&before.lines)?;
            let duplicate_number: i64 = self
                .db
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM shark_quote_issue_snapshot
                     WHERE company_slug = ?1 AND commercial_number = ?2",
                    (&self.company_slug, &commercial_number),
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if duplicate_number != 0 {
                return Err(validation("commercial number is already in use"));
            }
            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_quote_issue_snapshot(
                        company_slug, quote_id, commercial_number, document_kind,
                        customer_id, customer_display_name, customer_postal_address,
                        customer_email, customer_phone, total_minor, line_count, issued_by
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    (
                        &self.company_slug,
                        &quote_id,
                        &commercial_number,
                        before.kind.as_str(),
                        &before.customer.customer_id,
                        &before.customer.display_name,
                        &before.customer.postal_address,
                        &before.customer.email,
                        &before.customer.phone,
                        total_minor,
                        i64::try_from(before.lines.len())
                            .map_err(|_| validation("commercial line count overflow"))?,
                        self.actor.name(),
                    ),
                )
                .map_err(sqlite_error)?;
            let copied = self
                .db
                .conn()
                .execute(
                    "INSERT INTO shark_quote_issue_line(
                        company_slug, quote_id, line_id, description,
                        quantity_subunits, unit_price_minor, position
                     )
                     SELECT company_slug, quote_id, line_id, description,
                            quantity_subunits, unit_price_minor, position
                     FROM shark_quote_line
                     WHERE company_slug = ?1 AND quote_id = ?2",
                    (&self.company_slug, &quote_id),
                )
                .map_err(sqlite_error)?;
            if copied != before.lines.len() {
                return Err(storage(
                    "issued commercial snapshot line copy was incomplete",
                ));
            }
            let changed = self
                .db
                .conn()
                .execute(
                    "UPDATE shark_quote
                     SET state = 'issued', conversion_eligible = 0,
                         updated_by = ?1, updated_at = CURRENT_TIMESTAMP
                     WHERE company_slug = ?2 AND quote_id = ?3 AND state = 'draft'",
                    (self.actor.name(), &self.company_slug, &quote_id),
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(validation("quote issue lost draft authority"));
            }
            let quote = self.quote(&quote_id)?;
            let mutation =
                self.record_quote_mutation(&quote_id, "issue", Some(&before), Some(&quote))?;
            Ok(QuoteMutationOutcome { quote, mutation })
        })
    }

    pub fn transition_quote(
        &self,
        quote_id: &str,
        target: QuoteState,
    ) -> FoundationResult<QuoteMutationOutcome> {
        let quote_id = normalized_quote_id(quote_id)?;
        if !matches!(
            target,
            QuoteState::Accepted
                | QuoteState::Rejected
                | QuoteState::Expired
                | QuoteState::Cancelled
        ) {
            return Err(validation(
                "quote outcome must be accepted, rejected, expired or cancelled",
            ));
        }
        self.shark_savepoint("shark_quote_transition", || {
            let before = self.quote(&quote_id)?;
            if before.state != QuoteState::Issued {
                return Err(validation(
                    "quote or estimate outcome transition requires issued state",
                ));
            }
            let changed = self
                .db
                .conn()
                .execute(
                    "UPDATE shark_quote
                     SET state = ?1, conversion_eligible = ?2,
                         updated_by = ?3, updated_at = CURRENT_TIMESTAMP
                     WHERE company_slug = ?4 AND quote_id = ?5 AND state = 'issued'",
                    (
                        target.as_str(),
                        if target == QuoteState::Accepted {
                            1_i64
                        } else {
                            0_i64
                        },
                        self.actor.name(),
                        &self.company_slug,
                        &quote_id,
                    ),
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(validation("quote outcome lost issued-state authority"));
            }
            let quote = self.quote(&quote_id)?;
            let mutation = self.record_quote_mutation(
                &quote_id,
                target.as_str(),
                Some(&before),
                Some(&quote),
            )?;
            Ok(QuoteMutationOutcome { quote, mutation })
        })
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
            "shark-sbc8a2-quote-{label}-{}-{nonce}.db",
            std::process::id()
        ))
    }

    fn create_books(label: &str) -> (PathBuf, Books) {
        let path = temp_db_path(label);
        let books =
            Books::create_plain_for_test(&path, "quote-books", "Quote Books", "local-owner")
                .expect("create test books");
        books
            .save_contact(&ContactWrite {
                contact_id: "customer-1".into(),
                kind: "customer".into(),
                display_name: "Original Customer".into(),
                postal_address: Some("1 High Street".into()),
                email: Some("billing@example.test".into()),
                phone: None,
            })
            .expect("seed customer");
        (path, books)
    }

    fn create_draft(books: &Books, kind: QuoteKind) {
        books
            .create_quote_draft(&QuoteDraftWrite {
                quote_id: "commercial-1".into(),
                kind,
                customer_id: "customer-1".into(),
            })
            .expect("create draft");
        books
            .save_quote_line(
                "commercial-1",
                &QuoteLineWrite {
                    line_id: "line-1".into(),
                    description: "Design service".into(),
                    quantity_subunits: 2,
                    unit_price_minor: 12_500,
                },
            )
            .expect("add line");
    }

    #[test]
    fn quote_and_estimate_lifecycle_is_non_posting_and_history_is_owner_visible() {
        for kind in [QuoteKind::Quote, QuoteKind::Estimate] {
            let (path, books) = create_books(kind.as_str());
            let before_transactions = books.count_transactions().expect("before count");
            create_draft(&books, kind);
            let issued = books
                .issue_quote("commercial-1", &format!("{}-0001", kind.as_str()))
                .expect("issue");
            assert_eq!(issued.quote.state, QuoteState::Issued);
            assert_eq!(issued.quote.total_minor, 25_000);
            let snapshot = issued.quote.issued_snapshot.expect("issued snapshot");
            assert_eq!(snapshot.customer.display_name, "Original Customer");

            books
                .save_contact(&ContactWrite {
                    contact_id: "customer-1".into(),
                    kind: "customer".into(),
                    display_name: "Later Customer Name".into(),
                    postal_address: None,
                    email: None,
                    phone: None,
                })
                .expect("edit live contact");
            let accepted = books
                .transition_quote("commercial-1", QuoteState::Accepted)
                .expect("accept");
            assert!(accepted.quote.conversion_eligible);
            assert_eq!(
                accepted
                    .quote
                    .issued_snapshot
                    .as_ref()
                    .expect("snapshot")
                    .customer
                    .display_name,
                "Original Customer"
            );
            assert!(
                books
                    .save_quote_line(
                        "commercial-1",
                        &QuoteLineWrite {
                            line_id: "line-1".into(),
                            description: "Changed".into(),
                            quantity_subunits: 1,
                            unit_price_minor: 1,
                        }
                    )
                    .is_err()
            );
            let history = books.quote_history("commercial-1", 100).expect("history");
            assert_eq!(history.len(), 4);
            assert_eq!(history[0].action, "accepted");
            assert!(history.iter().all(|entry| entry.after.is_some()));
            assert_eq!(
                books.count_transactions().expect("after count"),
                before_transactions,
                "quote and estimate mutations must remain non-posting"
            );
            let invoice_count: i64 = books
                .db
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM shark_invoice WHERE company_slug = ?1",
                    (&books.company_slug,),
                    |row| row.get(0),
                )
                .expect("invoice row count");
            assert_eq!(
                invoice_count, 0,
                "acceptance must not create an invoice"
            );
            drop(books);
            remove_sqlite_artifacts(&path);
        }
    }

    #[test]
    fn invalid_transition_fails_without_state_or_history_mutation() {
        let (path, books) = create_books("invalid-transition");
        books
            .create_quote_draft(&QuoteDraftWrite {
                quote_id: "commercial-1".into(),
                kind: QuoteKind::Quote,
                customer_id: "customer-1".into(),
            })
            .expect("create draft");
        let before = books.quote_history("commercial-1", 100).unwrap();
        assert!(
            books
                .transition_quote("commercial-1", QuoteState::Accepted)
                .is_err()
        );
        assert_eq!(
            books.quote("commercial-1").unwrap().state,
            QuoteState::Draft
        );
        assert_eq!(books.quote_history("commercial-1", 100).unwrap(), before);
        assert!(books.db.conn().is_autocommit());
        drop(books);
        remove_sqlite_artifacts(&path);
    }

    #[test]
    fn database_guards_reject_issued_snapshot_and_history_rewrites() {
        let (path, books) = create_books("immutable-guards");
        create_draft(&books, QuoteKind::Quote);
        books.issue_quote("commercial-1", "Q-IMMUTABLE").unwrap();
        assert!(
            books
                .db
                .conn()
                .execute(
                    "UPDATE shark_quote_issue_snapshot SET total_minor = 1
                 WHERE company_slug = ?1 AND quote_id = ?2",
                    (&books.company_slug, "commercial-1"),
                )
                .is_err()
        );
        assert!(
            books
                .db
                .conn()
                .execute(
                    "DELETE FROM shark_quote_mutation
                 WHERE company_slug = ?1 AND quote_id = ?2",
                    (&books.company_slug, "commercial-1"),
                )
                .is_err()
        );
        assert!(
            books
                .db
                .conn()
                .execute(
                    "UPDATE shark_quote_line SET unit_price_minor = 1
                 WHERE company_slug = ?1 AND quote_id = ?2",
                    (&books.company_slug, "commercial-1"),
                )
                .is_err()
        );
        assert_eq!(
            books
                .quote("commercial-1")
                .unwrap()
                .issued_snapshot
                .unwrap()
                .total_minor,
            25_000
        );
        drop(books);
        remove_sqlite_artifacts(&path);
    }
}
