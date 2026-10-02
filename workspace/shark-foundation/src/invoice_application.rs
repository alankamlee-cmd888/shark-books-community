//! SBC-8A3 sales invoice, credit-note and explicit quote-conversion persistence.
//!
//! Issued commercial facts live in immutable snapshot tables. Every successful
//! mutation and every resulting invoice-balance change is recorded in the same
//! fail-closed savepoint. Manual receipts are balance facts only: this module
//! never creates a bank activity, settlement, provider call or ledger payment.

use super::*;
use crate::bank_application::sqlite_error;

const MAX_ID_BYTES: usize = 128;
const MAX_NUMBER_BYTES: usize = 64;
const MAX_DESCRIPTION_BYTES: usize = 500;
pub(crate) const MAX_INVOICE_PAGE: i64 = 500;
pub(crate) const MAX_INVOICE_HISTORY_PAGE: i64 = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceState {
    Draft,
    Issued,
    PartPaid,
    Paid,
    Cancelled,
}
impl InvoiceState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Issued => "issued",
            Self::PartPaid => "part_paid",
            Self::Paid => "paid",
            Self::Cancelled => "cancelled",
        }
    }
    fn from_persisted(value: &str) -> FoundationResult<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "issued" => Ok(Self::Issued),
            "part_paid" => Ok(Self::PartPaid),
            "paid" => Ok(Self::Paid),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(storage("persisted invoice state is invalid")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CreditNoteState {
    Draft,
    Issued,
    Cancelled,
}
impl CreditNoteState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Issued => "issued",
            Self::Cancelled => "cancelled",
        }
    }
    fn from_persisted(value: &str) -> FoundationResult<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "issued" => Ok(Self::Issued),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(storage("persisted credit-note state is invalid")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceDraftWrite {
    pub invoice_id: String,
    pub customer_id: String,
    pub issue_date: String,
    pub due_date: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceLineWrite {
    pub line_id: String,
    pub description: String,
    pub quantity_subunits: i64,
    pub unit_price_minor: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceLineView {
    pub line_id: String,
    pub description: String,
    pub quantity_subunits: i64,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub position: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceCustomerSnapshotView {
    pub customer_id: String,
    pub display_name: String,
    pub postal_address: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedInvoiceSnapshotView {
    pub invoice_number: String,
    pub customer: InvoiceCustomerSnapshotView,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub lines: Vec<InvoiceLineView>,
    pub total_minor: i64,
    pub source_quote_id: Option<String>,
    pub issued_by: String,
    pub issued_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceView {
    pub invoice_id: String,
    pub state: InvoiceState,
    pub customer: InvoiceCustomerSnapshotView,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub lines: Vec<InvoiceLineView>,
    pub total_minor: i64,
    pub paid_minor: i64,
    pub credited_minor: i64,
    pub outstanding_minor: i64,
    pub source_quote_id: Option<String>,
    pub issued_snapshot: Option<IssuedInvoiceSnapshotView>,
    pub created_by: String,
    pub created_at: String,
    pub updated_by: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceMutationView {
    pub mutation_id: i64,
    pub invoice_id: String,
    pub action: String,
    pub before: Option<InvoiceView>,
    pub after: Option<InvoiceView>,
    pub actor: String,
    pub occurred_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvoiceMutationOutcome {
    pub invoice: InvoiceView,
    pub mutation: InvoiceMutationView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditNoteDraftWrite {
    pub credit_note_id: String,
    pub invoice_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditNoteLineWrite {
    pub line_id: String,
    pub invoice_line_id: String,
    pub quantity_subunits: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditNoteLineView {
    pub line_id: String,
    pub invoice_line_id: String,
    pub description: String,
    pub quantity_subunits: i64,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub position: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedCreditNoteSnapshotView {
    pub credit_note_number: String,
    pub invoice_id: String,
    pub invoice_number: String,
    pub customer: InvoiceCustomerSnapshotView,
    pub lines: Vec<CreditNoteLineView>,
    pub total_minor: i64,
    pub issued_by: String,
    pub issued_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditNoteView {
    pub credit_note_id: String,
    pub invoice_id: String,
    pub state: CreditNoteState,
    pub lines: Vec<CreditNoteLineView>,
    pub total_minor: i64,
    pub issued_snapshot: Option<IssuedCreditNoteSnapshotView>,
    pub created_by: String,
    pub created_at: String,
    pub updated_by: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditNoteMutationView {
    pub mutation_id: i64,
    pub credit_note_id: String,
    pub action: String,
    pub before: Option<CreditNoteView>,
    pub after: Option<CreditNoteView>,
    pub actor: String,
    pub occurred_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditNoteMutationOutcome {
    pub credit_note: CreditNoteView,
    pub mutation: CreditNoteMutationView,
    pub invoice: InvoiceView,
    pub invoice_mutation: Option<InvoiceMutationView>,
}

fn validation(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Validation, message)
}
fn storage(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Storage, message)
}
fn safe_text(value: &str, field: &str, max: usize) -> FoundationResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max || value.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(validation(format!(
            "{field} must be 1-{max} bounded non-control characters"
        )));
    }
    Ok(value.to_string())
}
fn normalized_id(value: &str, field: &str) -> FoundationResult<String> {
    safe_text(value, field, MAX_ID_BYTES)
}
fn checked_line_total(quantity: i64, unit_price: i64) -> FoundationResult<i64> {
    if quantity <= 0 {
        return Err(validation(
            "commercial line quantity must be positive integer subunits",
        ));
    }
    if unit_price < 0 {
        return Err(validation(
            "commercial line unit price must be non-negative whole pence",
        ));
    }
    i64::try_from(
        i128::from(quantity)
            .checked_mul(i128::from(unit_price))
            .ok_or_else(|| validation("commercial line total overflow"))?,
    )
    .map_err(|_| validation("commercial line total overflow"))
}
fn checked_invoice_total(lines: &[InvoiceLineView]) -> FoundationResult<i64> {
    lines.iter().try_fold(0_i64, |sum, line| {
        sum.checked_add(line.total_minor)
            .ok_or_else(|| validation("invoice total overflow"))
    })
}
fn checked_credit_total(lines: &[CreditNoteLineView]) -> FoundationResult<i64> {
    lines.iter().try_fold(0_i64, |sum, line| {
        sum.checked_add(line.total_minor)
            .ok_or_else(|| validation("credit-note total overflow"))
    })
}
fn normalized_invoice_line(write: &InvoiceLineWrite) -> FoundationResult<InvoiceLineWrite> {
    let result = InvoiceLineWrite {
        line_id: normalized_id(&write.line_id, "invoice line id")?,
        description: safe_text(
            &write.description,
            "invoice line description",
            MAX_DESCRIPTION_BYTES,
        )?,
        quantity_subunits: write.quantity_subunits,
        unit_price_minor: write.unit_price_minor,
    };
    checked_line_total(result.quantity_subunits, result.unit_price_minor)?;
    Ok(result)
}
fn normalized_date(value: &str, field: &str) -> FoundationResult<String> {
    let value = safe_text(value, field, 10)?;
    chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .map_err(|_| validation(format!("{field} must be a valid YYYY-MM-DD date")))?;
    Ok(value)
}
fn normalized_dates(issue: &str, due: Option<&str>) -> FoundationResult<(String, Option<String>)> {
    let issue = normalized_date(issue, "invoice issue date")?;
    let due = due
        .map(|value| normalized_date(value, "invoice due date"))
        .transpose()?;
    if due.as_ref().is_some_and(|value| value < &issue) {
        return Err(validation("invoice due date cannot be before issue date"));
    }
    Ok((issue, due))
}

pub(crate) fn ensure_invoice_schema(db: &Db) -> FoundationResult<()> {
    db.conn().execute_batch(r#"
        CREATE TABLE IF NOT EXISTS shark_invoice (
            company_slug TEXT NOT NULL, invoice_id TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('draft','issued','part_paid','paid','cancelled')),
            customer_id TEXT NOT NULL, customer_display_name TEXT NOT NULL,
            customer_postal_address TEXT, customer_email TEXT, customer_phone TEXT,
            issue_date TEXT NOT NULL, due_date TEXT, paid_minor INTEGER NOT NULL DEFAULT 0 CHECK(paid_minor >= 0),
            source_quote_id TEXT, created_by TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_by TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_slug, invoice_id),
            UNIQUE(company_slug, source_quote_id),
            FOREIGN KEY(company_slug, customer_id) REFERENCES shark_contact(company_slug, contact_id) ON DELETE RESTRICT,
            FOREIGN KEY(company_slug, source_quote_id) REFERENCES shark_quote_issue_snapshot(company_slug, quote_id) ON DELETE RESTRICT,
            CHECK(due_date IS NULL OR due_date >= issue_date)
        );
        CREATE INDEX IF NOT EXISTS idx_shark_invoice_owner_list ON shark_invoice(company_slug, updated_at, invoice_id);
        CREATE TABLE IF NOT EXISTS shark_invoice_line (
            company_slug TEXT NOT NULL, invoice_id TEXT NOT NULL, line_id TEXT NOT NULL, description TEXT NOT NULL,
            quantity_subunits INTEGER NOT NULL CHECK(quantity_subunits > 0), unit_price_minor INTEGER NOT NULL CHECK(unit_price_minor >= 0),
            position INTEGER NOT NULL CHECK(position > 0), PRIMARY KEY(company_slug, invoice_id, line_id), UNIQUE(company_slug, invoice_id, position),
            FOREIGN KEY(company_slug, invoice_id) REFERENCES shark_invoice(company_slug, invoice_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_invoice_issue_snapshot (
            company_slug TEXT NOT NULL, invoice_id TEXT NOT NULL, invoice_number TEXT NOT NULL,
            customer_id TEXT NOT NULL, customer_display_name TEXT NOT NULL, customer_postal_address TEXT, customer_email TEXT, customer_phone TEXT,
            issue_date TEXT NOT NULL, due_date TEXT, total_minor INTEGER NOT NULL CHECK(total_minor >= 0), line_count INTEGER NOT NULL CHECK(line_count > 0),
            source_quote_id TEXT, issued_by TEXT NOT NULL, issued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_slug, invoice_id), UNIQUE(company_slug, invoice_number),
            FOREIGN KEY(company_slug, invoice_id) REFERENCES shark_invoice(company_slug, invoice_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_invoice_issue_line (
            company_slug TEXT NOT NULL, invoice_id TEXT NOT NULL, line_id TEXT NOT NULL, description TEXT NOT NULL,
            quantity_subunits INTEGER NOT NULL CHECK(quantity_subunits > 0), unit_price_minor INTEGER NOT NULL CHECK(unit_price_minor >= 0),
            position INTEGER NOT NULL CHECK(position > 0), PRIMARY KEY(company_slug, invoice_id, line_id), UNIQUE(company_slug, invoice_id, position),
            FOREIGN KEY(company_slug, invoice_id) REFERENCES shark_invoice_issue_snapshot(company_slug, invoice_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_invoice_mutation (
            id INTEGER PRIMARY KEY AUTOINCREMENT, company_slug TEXT NOT NULL, invoice_id TEXT NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('create_draft','set_customer','set_dates','add_line','replace_line','remove_line','issue','cancel','manual_payment','quote_conversion','credit_issued','credit_cancelled')),
            before_json TEXT, after_json TEXT, actor TEXT NOT NULL, occurred_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(company_slug, invoice_id) REFERENCES shark_invoice(company_slug, invoice_id) ON DELETE RESTRICT,
            CHECK(before_json IS NOT NULL OR after_json IS NOT NULL)
        );
        CREATE INDEX IF NOT EXISTS idx_shark_invoice_mutation_history ON shark_invoice_mutation(company_slug, invoice_id, id);
        CREATE TABLE IF NOT EXISTS shark_quote_conversion (
            company_slug TEXT NOT NULL, quote_id TEXT NOT NULL, invoice_id TEXT NOT NULL, source_quote_number TEXT NOT NULL,
            converted_by TEXT NOT NULL, converted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_slug, quote_id), UNIQUE(company_slug, invoice_id),
            FOREIGN KEY(company_slug, quote_id) REFERENCES shark_quote_issue_snapshot(company_slug, quote_id) ON DELETE RESTRICT,
            FOREIGN KEY(company_slug, invoice_id) REFERENCES shark_invoice(company_slug, invoice_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_credit_note (
            company_slug TEXT NOT NULL, credit_note_id TEXT NOT NULL, invoice_id TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('draft','issued','cancelled')),
            created_by TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_by TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_slug, credit_note_id),
            FOREIGN KEY(company_slug, invoice_id) REFERENCES shark_invoice_issue_snapshot(company_slug, invoice_id) ON DELETE RESTRICT
        );
        CREATE INDEX IF NOT EXISTS idx_shark_credit_note_owner_list ON shark_credit_note(company_slug, updated_at, credit_note_id);
        CREATE TABLE IF NOT EXISTS shark_credit_note_line (
            company_slug TEXT NOT NULL, credit_note_id TEXT NOT NULL, line_id TEXT NOT NULL, invoice_line_id TEXT NOT NULL,
            description TEXT NOT NULL, quantity_subunits INTEGER NOT NULL CHECK(quantity_subunits > 0), unit_price_minor INTEGER NOT NULL CHECK(unit_price_minor >= 0),
            position INTEGER NOT NULL CHECK(position > 0), PRIMARY KEY(company_slug, credit_note_id, line_id), UNIQUE(company_slug, credit_note_id, invoice_line_id), UNIQUE(company_slug, credit_note_id, position),
            FOREIGN KEY(company_slug, credit_note_id) REFERENCES shark_credit_note(company_slug, credit_note_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_credit_note_issue_snapshot (
            company_slug TEXT NOT NULL, credit_note_id TEXT NOT NULL, credit_note_number TEXT NOT NULL, invoice_id TEXT NOT NULL, invoice_number TEXT NOT NULL,
            customer_id TEXT NOT NULL, customer_display_name TEXT NOT NULL, customer_postal_address TEXT, customer_email TEXT, customer_phone TEXT,
            total_minor INTEGER NOT NULL CHECK(total_minor >= 0), line_count INTEGER NOT NULL CHECK(line_count > 0), issued_by TEXT NOT NULL, issued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_slug, credit_note_id), UNIQUE(company_slug, credit_note_number),
            FOREIGN KEY(company_slug, credit_note_id) REFERENCES shark_credit_note(company_slug, credit_note_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_credit_note_issue_line (
            company_slug TEXT NOT NULL, credit_note_id TEXT NOT NULL, line_id TEXT NOT NULL, invoice_line_id TEXT NOT NULL,
            description TEXT NOT NULL, quantity_subunits INTEGER NOT NULL CHECK(quantity_subunits > 0), unit_price_minor INTEGER NOT NULL CHECK(unit_price_minor >= 0),
            position INTEGER NOT NULL CHECK(position > 0), PRIMARY KEY(company_slug, credit_note_id, line_id), UNIQUE(company_slug, credit_note_id, invoice_line_id), UNIQUE(company_slug, credit_note_id, position),
            FOREIGN KEY(company_slug, credit_note_id) REFERENCES shark_credit_note_issue_snapshot(company_slug, credit_note_id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_credit_note_mutation (
            id INTEGER PRIMARY KEY AUTOINCREMENT, company_slug TEXT NOT NULL, credit_note_id TEXT NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('create_draft','add_line','replace_line','remove_line','issue','cancel')),
            before_json TEXT, after_json TEXT, actor TEXT NOT NULL, occurred_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(company_slug, credit_note_id) REFERENCES shark_credit_note(company_slug, credit_note_id) ON DELETE RESTRICT,
            CHECK(before_json IS NOT NULL OR after_json IS NOT NULL)
        );
        CREATE INDEX IF NOT EXISTS idx_shark_credit_note_mutation_history ON shark_credit_note_mutation(company_slug, credit_note_id, id);

        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_no_delete BEFORE DELETE ON shark_invoice BEGIN SELECT RAISE(ABORT,'invoices are append-only'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_source_immutable BEFORE UPDATE OF source_quote_id ON shark_invoice BEGIN SELECT RAISE(ABORT,'invoice source quote identity is immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_issued_facts_immutable BEFORE UPDATE ON shark_invoice
        WHEN OLD.state <> 'draft' AND (NEW.customer_id IS NOT OLD.customer_id OR NEW.customer_display_name IS NOT OLD.customer_display_name OR NEW.customer_postal_address IS NOT OLD.customer_postal_address OR NEW.customer_email IS NOT OLD.customer_email OR NEW.customer_phone IS NOT OLD.customer_phone OR NEW.issue_date IS NOT OLD.issue_date OR NEW.due_date IS NOT OLD.due_date)
        BEGIN SELECT RAISE(ABORT,'issued invoice commercial facts are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_line_insert_draft_only BEFORE INSERT ON shark_invoice_line
        WHEN (SELECT state FROM shark_invoice WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued invoice lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_line_update_draft_only BEFORE UPDATE ON shark_invoice_line
        WHEN (SELECT state FROM shark_invoice WHERE company_slug=OLD.company_slug AND invoice_id=OLD.invoice_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued invoice lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_line_delete_draft_only BEFORE DELETE ON shark_invoice_line
        WHEN (SELECT state FROM shark_invoice WHERE company_slug=OLD.company_slug AND invoice_id=OLD.invoice_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued invoice lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_snapshot_draft_insert_only BEFORE INSERT ON shark_invoice_issue_snapshot
        WHEN (SELECT state FROM shark_invoice WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued invoice snapshot requires draft state'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_snapshot_no_update BEFORE UPDATE ON shark_invoice_issue_snapshot BEGIN SELECT RAISE(ABORT,'issued invoice snapshot and number are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_snapshot_no_delete BEFORE DELETE ON shark_invoice_issue_snapshot BEGIN SELECT RAISE(ABORT,'issued invoice snapshot and number are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_issue_line_bounded BEFORE INSERT ON shark_invoice_issue_line
        WHEN (SELECT COUNT(*) FROM shark_invoice_issue_line WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id) >= (SELECT line_count FROM shark_invoice_issue_snapshot WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id)
        BEGIN SELECT RAISE(ABORT,'issued invoice snapshot line count is immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_issue_line_no_update BEFORE UPDATE ON shark_invoice_issue_line BEGIN SELECT RAISE(ABORT,'issued invoice snapshot lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_issue_line_no_delete BEFORE DELETE ON shark_invoice_issue_line BEGIN SELECT RAISE(ABORT,'issued invoice snapshot lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_history_no_update BEFORE UPDATE ON shark_invoice_mutation BEGIN SELECT RAISE(ABORT,'invoice mutation history is append-only'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_invoice_history_no_delete BEFORE DELETE ON shark_invoice_mutation BEGIN SELECT RAISE(ABORT,'invoice mutation history is append-only'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_quote_conversion_no_update BEFORE UPDATE ON shark_quote_conversion BEGIN SELECT RAISE(ABORT,'quote conversion binding is immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_quote_conversion_no_delete BEFORE DELETE ON shark_quote_conversion BEGIN SELECT RAISE(ABORT,'quote conversion binding is immutable'); END;

        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_note_no_delete BEFORE DELETE ON shark_credit_note BEGIN SELECT RAISE(ABORT,'credit notes are append-only'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_note_invoice_immutable BEFORE UPDATE OF invoice_id ON shark_credit_note BEGIN SELECT RAISE(ABORT,'credit-note source invoice is immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_line_insert_draft_only BEFORE INSERT ON shark_credit_note_line
        WHEN (SELECT state FROM shark_credit_note WHERE company_slug=NEW.company_slug AND credit_note_id=NEW.credit_note_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued credit-note lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_line_update_draft_only BEFORE UPDATE ON shark_credit_note_line
        WHEN (SELECT state FROM shark_credit_note WHERE company_slug=OLD.company_slug AND credit_note_id=OLD.credit_note_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued credit-note lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_line_delete_draft_only BEFORE DELETE ON shark_credit_note_line
        WHEN (SELECT state FROM shark_credit_note WHERE company_slug=OLD.company_slug AND credit_note_id=OLD.credit_note_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued credit-note lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_snapshot_draft_insert_only BEFORE INSERT ON shark_credit_note_issue_snapshot
        WHEN (SELECT state FROM shark_credit_note WHERE company_slug=NEW.company_slug AND credit_note_id=NEW.credit_note_id) <> 'draft' BEGIN SELECT RAISE(ABORT,'issued credit-note snapshot requires draft state'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_snapshot_no_update BEFORE UPDATE ON shark_credit_note_issue_snapshot BEGIN SELECT RAISE(ABORT,'issued credit-note snapshot and number are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_snapshot_no_delete BEFORE DELETE ON shark_credit_note_issue_snapshot BEGIN SELECT RAISE(ABORT,'issued credit-note snapshot and number are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_issue_line_matches_invoice BEFORE INSERT ON shark_credit_note_issue_line
        WHEN NOT EXISTS (
            SELECT 1 FROM shark_invoice_issue_line il JOIN shark_credit_note cn ON cn.company_slug=NEW.company_slug AND cn.credit_note_id=NEW.credit_note_id
            WHERE il.company_slug=NEW.company_slug AND il.invoice_id=cn.invoice_id AND il.line_id=NEW.invoice_line_id
              AND il.description=NEW.description AND il.unit_price_minor=NEW.unit_price_minor AND NEW.quantity_subunits<=il.quantity_subunits
        ) BEGIN SELECT RAISE(ABORT,'credit-note line must match an issued invoice line'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_issue_line_no_update BEFORE UPDATE ON shark_credit_note_issue_line BEGIN SELECT RAISE(ABORT,'issued credit-note snapshot lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_issue_line_no_delete BEFORE DELETE ON shark_credit_note_issue_line BEGIN SELECT RAISE(ABORT,'issued credit-note snapshot lines are immutable'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_history_no_update BEFORE UPDATE ON shark_credit_note_mutation BEGIN SELECT RAISE(ABORT,'credit-note mutation history is append-only'); END;
        CREATE TRIGGER IF NOT EXISTS trg_shark_credit_history_no_delete BEFORE DELETE ON shark_credit_note_mutation BEGIN SELECT RAISE(ABORT,'credit-note mutation history is append-only'); END;
    "#).map_err(sqlite_error)?;
    ensure_invoice_guards(db)
}

impl Books {
    fn customer_for_invoice(
        &self,
        customer_id: &str,
    ) -> FoundationResult<InvoiceCustomerSnapshotView> {
        let customer_id = normalized_id(customer_id, "customer id")?;
        let contact = self.contact(&customer_id)?;
        if contact.kind != "customer" {
            return Err(validation("invoices require an existing customer contact"));
        }
        Ok(InvoiceCustomerSnapshotView {
            customer_id: contact.contact_id,
            display_name: contact.display_name,
            postal_address: contact.postal_address,
            email: contact.email,
            phone: contact.phone,
        })
    }
    fn invoice_lines(
        &self,
        invoice_id: &str,
        issued: bool,
    ) -> FoundationResult<Vec<InvoiceLineView>> {
        let sql = if issued {
            "SELECT line_id,description,quantity_subunits,unit_price_minor,position FROM shark_invoice_issue_line WHERE company_slug=?1 AND invoice_id=?2 ORDER BY position,line_id"
        } else {
            "SELECT line_id,description,quantity_subunits,unit_price_minor,position FROM shark_invoice_line WHERE company_slug=?1 AND invoice_id=?2 ORDER BY position,line_id"
        };
        let mut statement = self.db.conn().prepare(sql).map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, invoice_id), |row| {
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
            let (id, description, quantity, price, position) = row.map_err(sqlite_error)?;
            Ok(InvoiceLineView {
                line_id: id,
                description,
                quantity_subunits: quantity,
                unit_price_minor: price,
                total_minor: checked_line_total(quantity, price)?,
                position,
            })
        })
        .collect()
    }
    fn active_credit_total(&self, invoice_id: &str) -> FoundationResult<i64> {
        let mut statement=self.db.conn().prepare("SELECT s.total_minor FROM shark_credit_note_issue_snapshot s JOIN shark_credit_note c ON c.company_slug=s.company_slug AND c.credit_note_id=s.credit_note_id WHERE s.company_slug=?1 AND s.invoice_id=?2 AND c.state='issued' ORDER BY s.credit_note_id").map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, invoice_id), |row| row.get::<_, i64>(0))
            .map_err(sqlite_error)?;
        rows.map(|row| row.map_err(sqlite_error))
            .try_fold(0_i64, |sum, value| {
                sum.checked_add(value?)
                    .ok_or_else(|| storage("credited invoice total overflow"))
            })
    }
    fn invoice_optional(&self, invoice_id: &str) -> FoundationResult<Option<InvoiceView>> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let row=self.db.conn().query_row("SELECT state,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,issue_date,due_date,paid_minor,source_quote_id,created_by,created_at,updated_by,updated_at FROM shark_invoice WHERE company_slug=?1 AND invoice_id=?2",(&self.company_slug,&invoice_id),|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,String>(6)?,row.get::<_,Option<String>>(7)?,row.get::<_,i64>(8)?,row.get::<_,Option<String>>(9)?,row.get::<_,String>(10)?,row.get::<_,String>(11)?,row.get::<_,String>(12)?,row.get::<_,String>(13)?)));
        let row = match row {
            Ok(value) => value,
            Err(error) if error.to_string().contains("Query returned no rows") => return Ok(None),
            Err(error) => return Err(sqlite_error(error)),
        };
        let state = InvoiceState::from_persisted(&row.0)?;
        if row.8 < 0 {
            return Err(storage("persisted invoice payment is invalid"));
        }
        let customer = InvoiceCustomerSnapshotView {
            customer_id: row.1,
            display_name: row.2,
            postal_address: row.3,
            email: row.4,
            phone: row.5,
        };
        let lines = self.invoice_lines(&invoice_id, false)?;
        let draft_total = checked_invoice_total(&lines)?;
        let snapshot=self.db.conn().query_row("SELECT invoice_number,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,issue_date,due_date,total_minor,line_count,source_quote_id,issued_by,issued_at FROM shark_invoice_issue_snapshot WHERE company_slug=?1 AND invoice_id=?2",(&self.company_slug,&invoice_id),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,i64>(8)?,r.get::<_,i64>(9)?,r.get::<_,Option<String>>(10)?,r.get::<_,String>(11)?,r.get::<_,String>(12)?)));
        let issued_snapshot = match snapshot {
            Ok(s) => {
                let snapshot_lines = self.invoice_lines(&invoice_id, true)?;
                if usize::try_from(s.9).ok() != Some(snapshot_lines.len())
                    || checked_invoice_total(&snapshot_lines)? != s.8
                {
                    return Err(storage("issued invoice snapshot is inconsistent"));
                }
                Some(IssuedInvoiceSnapshotView {
                    invoice_number: s.0,
                    customer: InvoiceCustomerSnapshotView {
                        customer_id: s.1,
                        display_name: s.2,
                        postal_address: s.3,
                        email: s.4,
                        phone: s.5,
                    },
                    issue_date: s.6,
                    due_date: s.7,
                    lines: snapshot_lines,
                    total_minor: s.8,
                    source_quote_id: s.10,
                    issued_by: s.11,
                    issued_at: s.12,
                })
            }
            Err(error) if error.to_string().contains("Query returned no rows") => None,
            Err(error) => return Err(sqlite_error(error)),
        };
        if state == InvoiceState::Draft && issued_snapshot.is_some() {
            return Err(storage("draft invoice unexpectedly has an issued snapshot"));
        }
        if matches!(
            state,
            InvoiceState::Issued | InvoiceState::PartPaid | InvoiceState::Paid
        ) && issued_snapshot.is_none()
        {
            return Err(storage(
                "active invoice is missing its immutable issued snapshot",
            ));
        }
        let total = issued_snapshot
            .as_ref()
            .map_or(draft_total, |s| s.total_minor);
        let credited = self.active_credit_total(&invoice_id)?;
        if credited > total {
            return Err(storage("persisted credits exceed the invoice total"));
        }
        let settled = row
            .8
            .checked_add(credited)
            .ok_or_else(|| storage("invoice balance overflow"))?;
        let outstanding = total
            .checked_sub(settled)
            .ok_or_else(|| storage("invoice balance overflow"))?
            .max(0);
        if state == InvoiceState::PartPaid && (row.8 == 0 || outstanding == 0) {
            return Err(storage(
                "persisted part-paid invoice balance is inconsistent",
            ));
        }
        if state == InvoiceState::Paid && (row.8 == 0 || outstanding != 0) {
            return Err(storage("persisted paid invoice balance is inconsistent"));
        }
        Ok(Some(InvoiceView {
            invoice_id,
            state,
            customer,
            issue_date: row.6,
            due_date: row.7,
            lines,
            total_minor: total,
            paid_minor: row.8,
            credited_minor: credited,
            outstanding_minor: outstanding,
            source_quote_id: row.9,
            issued_snapshot,
            created_by: row.10,
            created_at: row.11,
            updated_by: row.12,
            updated_at: row.13,
        }))
    }
    pub fn invoice(&self, invoice_id: &str) -> FoundationResult<InvoiceView> {
        self.invoice_optional(invoice_id)?.ok_or_else(|| {
            FoundationError::new(FoundationErrorCode::NotFound, "invoice was not found")
        })
    }
    pub fn invoices(
        &self,
        state: Option<InvoiceState>,
        limit: i64,
    ) -> FoundationResult<Vec<InvoiceView>> {
        if !(1..=MAX_INVOICE_PAGE).contains(&limit) {
            return Err(validation("invoice list limit must be between 1 and 500"));
        }
        let mut statement=self.db.conn().prepare("SELECT invoice_id FROM shark_invoice WHERE company_slug=?1 AND (?2 IS NULL OR state=?2) ORDER BY updated_at DESC,invoice_id LIMIT ?3").map_err(sqlite_error)?;
        let ids = statement
            .query_map(
                (&self.company_slug, state.map(InvoiceState::as_str), limit),
                |row| row.get::<_, String>(0),
            )
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        ids.into_iter().map(|id| self.invoice(&id)).collect()
    }
    fn invoice_mutation(&self, id: i64) -> FoundationResult<InvoiceMutationView> {
        let row=self.db.conn().query_row("SELECT invoice_id,action,before_json,after_json,actor,occurred_at FROM shark_invoice_mutation WHERE company_slug=?1 AND id=?2",(&self.company_slug,id),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?))).map_err(sqlite_error)?;
        Ok(InvoiceMutationView {
            mutation_id: id,
            invoice_id: row.0,
            action: row.1,
            before: parse_json(row.2, "invoice")?,
            after: parse_json(row.3, "invoice")?,
            actor: row.4,
            occurred_at: row.5,
        })
    }
    fn record_invoice_mutation(
        &self,
        invoice_id: &str,
        action: &str,
        before: Option<&InvoiceView>,
        after: Option<&InvoiceView>,
    ) -> FoundationResult<InvoiceMutationView> {
        let before = serialize(before, "invoice")?;
        let after = serialize(after, "invoice")?;
        self.db.conn().execute("INSERT INTO shark_invoice_mutation(company_slug,invoice_id,action,before_json,after_json,actor) VALUES(?1,?2,?3,?4,?5,?6)",(&self.company_slug,invoice_id,action,before,after,self.actor.name())).map_err(sqlite_error)?;
        self.invoice_mutation(self.db.conn().last_insert_rowid())
    }
    pub fn invoice_history(
        &self,
        invoice_id: &str,
        limit: i64,
    ) -> FoundationResult<Vec<InvoiceMutationView>> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        if !(1..=MAX_INVOICE_HISTORY_PAGE).contains(&limit) {
            return Err(validation(
                "invoice history limit must be between 1 and 500",
            ));
        }
        self.invoice(&invoice_id)?;
        let mut statement=self.db.conn().prepare("SELECT id FROM shark_invoice_mutation WHERE company_slug=?1 AND invoice_id=?2 ORDER BY id DESC LIMIT ?3").map_err(sqlite_error)?;
        let ids = statement
            .query_map((&self.company_slug, &invoice_id, limit), |r| {
                r.get::<_, i64>(0)
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        ids.into_iter()
            .map(|id| self.invoice_mutation(id))
            .collect()
    }
    pub fn create_invoice_draft(
        &self,
        write: &InvoiceDraftWrite,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(&write.invoice_id, "invoice id")?;
        let customer = self.customer_for_invoice(&write.customer_id)?;
        let (issue, due) = normalized_dates(&write.issue_date, write.due_date.as_deref())?;
        self.shark_savepoint("shark_invoice_create",||{self.db.conn().execute("INSERT INTO shark_invoice(company_slug,invoice_id,state,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,issue_date,due_date,created_by,updated_by) VALUES(?1,?2,'draft',?3,?4,?5,?6,?7,?8,?9,?10,?10)",(&self.company_slug,&invoice_id,&customer.customer_id,&customer.display_name,&customer.postal_address,&customer.email,&customer.phone,&issue,&due,self.actor.name())).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"create_draft",None,Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn set_invoice_customer(
        &self,
        invoice_id: &str,
        customer_id: &str,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let customer = self.customer_for_invoice(customer_id)?;
        self.shark_savepoint("shark_invoice_customer",||{let before=self.invoice(&invoice_id)?;if before.state!=InvoiceState::Draft||before.source_quote_id.is_some(){return Err(validation("invoice customer is immutable after issue or conversion"));}self.db.conn().execute("UPDATE shark_invoice SET customer_id=?1,customer_display_name=?2,customer_postal_address=?3,customer_email=?4,customer_phone=?5,updated_by=?6,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?7 AND invoice_id=?8 AND state='draft'",(&customer.customer_id,&customer.display_name,&customer.postal_address,&customer.email,&customer.phone,self.actor.name(),&self.company_slug,&invoice_id)).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"set_customer",Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn set_invoice_dates(
        &self,
        invoice_id: &str,
        issue_date: &str,
        due_date: Option<&str>,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let (issue, due) = normalized_dates(issue_date, due_date)?;
        self.shark_savepoint("shark_invoice_dates",||{let before=self.invoice(&invoice_id)?;if before.state!=InvoiceState::Draft{return Err(validation("invoice dates are immutable after issue"));}self.db.conn().execute("UPDATE shark_invoice SET issue_date=?1,due_date=?2,updated_by=?3,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?4 AND invoice_id=?5 AND state='draft'",(&issue,&due,self.actor.name(),&self.company_slug,&invoice_id)).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"set_dates",Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn save_invoice_line(
        &self,
        invoice_id: &str,
        write: &InvoiceLineWrite,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let write = normalized_invoice_line(write)?;
        self.shark_savepoint("shark_invoice_line_save",||{let before=self.invoice(&invoice_id)?;if before.state!=InvoiceState::Draft{return Err(validation("invoice lines are immutable after issue"));}let position=before.lines.iter().find(|l|l.line_id==write.line_id).map(|l|l.position).unwrap_or_else(||before.lines.iter().map(|l|l.position).max().unwrap_or(0)+1);let action=if before.lines.iter().any(|l|l.line_id==write.line_id){"replace_line"}else{"add_line"};self.db.conn().execute("INSERT INTO shark_invoice_line(company_slug,invoice_id,line_id,description,quantity_subunits,unit_price_minor,position) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(company_slug,invoice_id,line_id) DO UPDATE SET description=excluded.description,quantity_subunits=excluded.quantity_subunits,unit_price_minor=excluded.unit_price_minor",(&self.company_slug,&invoice_id,&write.line_id,&write.description,write.quantity_subunits,write.unit_price_minor,position)).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,action,Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn remove_invoice_line(
        &self,
        invoice_id: &str,
        line_id: &str,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let line_id = normalized_id(line_id, "invoice line id")?;
        self.shark_savepoint("shark_invoice_line_remove",||{let before=self.invoice(&invoice_id)?;if before.state!=InvoiceState::Draft{return Err(validation("invoice lines are immutable after issue"));}let changed=self.db.conn().execute("DELETE FROM shark_invoice_line WHERE company_slug=?1 AND invoice_id=?2 AND line_id=?3",(&self.company_slug,&invoice_id,&line_id)).map_err(sqlite_error)?;if changed!=1{return Err(FoundationError::new(FoundationErrorCode::NotFound,"invoice line was not found"));}let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"remove_line",Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn issue_invoice(
        &self,
        invoice_id: &str,
        number: &str,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let number = safe_text(number, "invoice number", MAX_NUMBER_BYTES)?;
        self.shark_savepoint("shark_invoice_issue",||{let before=self.invoice(&invoice_id)?;if before.state!=InvoiceState::Draft{return Err(validation("only a draft invoice can be issued"));}if before.lines.is_empty(){return Err(validation("invoice requires at least one line before issue"));}let total=checked_invoice_total(&before.lines)?;self.db.conn().execute("INSERT INTO shark_invoice_issue_snapshot(company_slug,invoice_id,invoice_number,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,issue_date,due_date,total_minor,line_count,source_quote_id,issued_by) SELECT company_slug,invoice_id,?1,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,issue_date,due_date,?2,?3,source_quote_id,?4 FROM shark_invoice WHERE company_slug=?5 AND invoice_id=?6 AND state='draft'",(&number,total,i64::try_from(before.lines.len()).map_err(|_|validation("invoice line count overflow"))?,self.actor.name(),&self.company_slug,&invoice_id)).map_err(sqlite_error)?;let copied=self.db.conn().execute("INSERT INTO shark_invoice_issue_line(company_slug,invoice_id,line_id,description,quantity_subunits,unit_price_minor,position) SELECT company_slug,invoice_id,line_id,description,quantity_subunits,unit_price_minor,position FROM shark_invoice_line WHERE company_slug=?1 AND invoice_id=?2",(&self.company_slug,&invoice_id)).map_err(sqlite_error)?;if copied!=before.lines.len(){return Err(storage("issued invoice line copy was incomplete"));}self.db.conn().execute("UPDATE shark_invoice SET state='issued',updated_by=?1,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?2 AND invoice_id=?3 AND state='draft'",(self.actor.name(),&self.company_slug,&invoice_id)).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"issue",Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn cancel_invoice(&self, invoice_id: &str) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        self.shark_savepoint("shark_invoice_cancel",||{let before=self.invoice(&invoice_id)?;if !matches!(before.state,InvoiceState::Draft|InvoiceState::Issued){return Err(validation("only an unpaid draft or issued invoice can be cancelled"));}if before.paid_minor!=0||before.credited_minor!=0{return Err(validation("an invoice with payments or issued credits cannot be cancelled"));}self.db.conn().execute("UPDATE shark_invoice SET state='cancelled',updated_by=?1,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?2 AND invoice_id=?3 AND state IN ('draft','issued')",(self.actor.name(),&self.company_slug,&invoice_id)).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"cancel",Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn record_manual_invoice_payment(
        &self,
        invoice_id: &str,
        amount_minor: i64,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        if amount_minor <= 0 {
            return Err(validation("manual payment must be positive whole pence"));
        }
        self.shark_savepoint("shark_invoice_manual_payment",||{let before=self.invoice(&invoice_id)?;if !matches!(before.state,InvoiceState::Issued|InvoiceState::PartPaid){return Err(validation("manual payment requires an active issued invoice"));}if amount_minor>before.outstanding_minor{return Err(validation("manual payment cannot exceed the outstanding invoice balance"));}let paid=before.paid_minor.checked_add(amount_minor).ok_or_else(||validation("invoice payment overflow"))?;let state=if amount_minor==before.outstanding_minor{InvoiceState::Paid}else{InvoiceState::PartPaid};self.db.conn().execute("UPDATE shark_invoice SET paid_minor=?1,state=?2,updated_by=?3,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?4 AND invoice_id=?5",(paid,state.as_str(),self.actor.name(),&self.company_slug,&invoice_id)).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;let mutation=self.record_invoice_mutation(&invoice_id,"manual_payment",Some(&before),Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }
    pub fn convert_accepted_quote(
        &self,
        quote_id: &str,
        invoice_id: &str,
        issue_date: &str,
        due_date: Option<&str>,
    ) -> FoundationResult<InvoiceMutationOutcome> {
        let quote_id = normalized_id(quote_id, "quote or estimate id")?;
        let invoice_id = normalized_id(invoice_id, "invoice id")?;
        let (issue, due) = normalized_dates(issue_date, due_date)?;
        self.shark_savepoint("shark_quote_convert",||{let quote=self.quote(&quote_id)?;if quote.state!=QuoteState::Accepted||!quote.conversion_eligible{return Err(validation("only an accepted conversion-eligible quote or estimate can be explicitly converted"));}let source=quote.issued_snapshot.ok_or_else(||storage("accepted quote is missing its immutable issued snapshot"))?;self.db.conn().execute("INSERT INTO shark_invoice(company_slug,invoice_id,state,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,issue_date,due_date,source_quote_id,created_by,updated_by) VALUES(?1,?2,'draft',?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",(&self.company_slug,&invoice_id,&source.customer.customer_id,&source.customer.display_name,&source.customer.postal_address,&source.customer.email,&source.customer.phone,&issue,&due,&quote_id,self.actor.name())).map_err(sqlite_error)?;let copied=self.db.conn().execute("INSERT INTO shark_invoice_line(company_slug,invoice_id,line_id,description,quantity_subunits,unit_price_minor,position) SELECT company_slug,?1,line_id,description,quantity_subunits,unit_price_minor,position FROM shark_quote_issue_line WHERE company_slug=?2 AND quote_id=?3",(&invoice_id,&self.company_slug,&quote_id)).map_err(sqlite_error)?;if copied!=source.lines.len(){return Err(storage("quote conversion did not copy the complete immutable issued snapshot"));}self.db.conn().execute("INSERT INTO shark_quote_conversion(company_slug,quote_id,invoice_id,source_quote_number,converted_by) VALUES(?1,?2,?3,?4,?5)",(&self.company_slug,&quote_id,&invoice_id,&source.commercial_number,self.actor.name())).map_err(sqlite_error)?;let invoice=self.invoice(&invoice_id)?;if invoice.total_minor!=source.total_minor{return Err(storage("quote conversion total does not match its immutable source snapshot"));}let mutation=self.record_invoice_mutation(&invoice_id,"quote_conversion",None,Some(&invoice))?;Ok(InvoiceMutationOutcome{invoice,mutation})})
    }

    fn credit_lines(
        &self,
        credit_id: &str,
        issued: bool,
    ) -> FoundationResult<Vec<CreditNoteLineView>> {
        let table = if issued {
            "shark_credit_note_issue_line"
        } else {
            "shark_credit_note_line"
        };
        let sql = format!(
            "SELECT line_id,invoice_line_id,description,quantity_subunits,unit_price_minor,position FROM {table} WHERE company_slug=?1 AND credit_note_id=?2 ORDER BY position,line_id"
        );
        let mut statement = self.db.conn().prepare(&sql).map_err(sqlite_error)?;
        let rows = statement
            .query_map((&self.company_slug, credit_id), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(sqlite_error)?;
        rows.map(|row| {
            let (id, invoice_line, description, quantity, price, position) =
                row.map_err(sqlite_error)?;
            Ok(CreditNoteLineView {
                line_id: id,
                invoice_line_id: invoice_line,
                description,
                quantity_subunits: quantity,
                unit_price_minor: price,
                total_minor: checked_line_total(quantity, price)?,
                position,
            })
        })
        .collect()
    }
    fn credit_optional(&self, credit_id: &str) -> FoundationResult<Option<CreditNoteView>> {
        let credit_id = normalized_id(credit_id, "credit note id")?;
        let row=self.db.conn().query_row("SELECT invoice_id,state,created_by,created_at,updated_by,updated_at FROM shark_credit_note WHERE company_slug=?1 AND credit_note_id=?2",(&self.company_slug,&credit_id),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?)));
        let row = match row {
            Ok(v) => v,
            Err(e) if e.to_string().contains("Query returned no rows") => return Ok(None),
            Err(e) => return Err(sqlite_error(e)),
        };
        let state = CreditNoteState::from_persisted(&row.1)?;
        let lines = self.credit_lines(&credit_id, false)?;
        let draft_total = checked_credit_total(&lines)?;
        let snapshot=self.db.conn().query_row("SELECT credit_note_number,invoice_id,invoice_number,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,total_minor,line_count,issued_by,issued_at FROM shark_credit_note_issue_snapshot WHERE company_slug=?1 AND credit_note_id=?2",(&self.company_slug,&credit_id),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,i64>(8)?,r.get::<_,i64>(9)?,r.get::<_,String>(10)?,r.get::<_,String>(11)?)));
        let issued_snapshot = match snapshot {
            Ok(s) => {
                let snapshot_lines = self.credit_lines(&credit_id, true)?;
                if usize::try_from(s.9).ok() != Some(snapshot_lines.len())
                    || checked_credit_total(&snapshot_lines)? != s.8
                {
                    return Err(storage("issued credit-note snapshot is inconsistent"));
                }
                Some(IssuedCreditNoteSnapshotView {
                    credit_note_number: s.0,
                    invoice_id: s.1,
                    invoice_number: s.2,
                    customer: InvoiceCustomerSnapshotView {
                        customer_id: s.3,
                        display_name: s.4,
                        postal_address: s.5,
                        email: s.6,
                        phone: s.7,
                    },
                    lines: snapshot_lines,
                    total_minor: s.8,
                    issued_by: s.10,
                    issued_at: s.11,
                })
            }
            Err(e) if e.to_string().contains("Query returned no rows") => None,
            Err(e) => return Err(sqlite_error(e)),
        };
        if state == CreditNoteState::Draft && issued_snapshot.is_some() {
            return Err(storage(
                "draft credit note unexpectedly has an issued snapshot",
            ));
        }
        if state == CreditNoteState::Issued && issued_snapshot.is_none() {
            return Err(storage(
                "issued credit note is missing its immutable snapshot",
            ));
        }
        let total = issued_snapshot
            .as_ref()
            .map_or(draft_total, |s| s.total_minor);
        Ok(Some(CreditNoteView {
            credit_note_id: credit_id,
            invoice_id: row.0,
            state,
            lines,
            total_minor: total,
            issued_snapshot,
            created_by: row.2,
            created_at: row.3,
            updated_by: row.4,
            updated_at: row.5,
        }))
    }
    pub fn credit_note(&self, id: &str) -> FoundationResult<CreditNoteView> {
        self.credit_optional(id)?.ok_or_else(|| {
            FoundationError::new(FoundationErrorCode::NotFound, "credit note was not found")
        })
    }
    pub fn credit_notes(
        &self,
        invoice_id: Option<&str>,
        state: Option<CreditNoteState>,
        limit: i64,
    ) -> FoundationResult<Vec<CreditNoteView>> {
        if !(1..=MAX_INVOICE_PAGE).contains(&limit) {
            return Err(validation(
                "credit-note list limit must be between 1 and 500",
            ));
        }
        let invoice = invoice_id
            .map(|id| normalized_id(id, "invoice id"))
            .transpose()?;
        let mut statement=self.db.conn().prepare("SELECT credit_note_id FROM shark_credit_note WHERE company_slug=?1 AND (?2 IS NULL OR invoice_id=?2) AND (?3 IS NULL OR state=?3) ORDER BY updated_at DESC,credit_note_id LIMIT ?4").map_err(sqlite_error)?;
        let ids = statement
            .query_map(
                (
                    &self.company_slug,
                    invoice,
                    state.map(CreditNoteState::as_str),
                    limit,
                ),
                |r| r.get::<_, String>(0),
            )
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        ids.into_iter().map(|id| self.credit_note(&id)).collect()
    }
    fn credit_mutation(&self, id: i64) -> FoundationResult<CreditNoteMutationView> {
        let row=self.db.conn().query_row("SELECT credit_note_id,action,before_json,after_json,actor,occurred_at FROM shark_credit_note_mutation WHERE company_slug=?1 AND id=?2",(&self.company_slug,id),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?))).map_err(sqlite_error)?;
        Ok(CreditNoteMutationView {
            mutation_id: id,
            credit_note_id: row.0,
            action: row.1,
            before: parse_json(row.2, "credit note")?,
            after: parse_json(row.3, "credit note")?,
            actor: row.4,
            occurred_at: row.5,
        })
    }
    fn record_credit_mutation(
        &self,
        id: &str,
        action: &str,
        before: Option<&CreditNoteView>,
        after: Option<&CreditNoteView>,
    ) -> FoundationResult<CreditNoteMutationView> {
        let before = serialize(before, "credit note")?;
        let after = serialize(after, "credit note")?;
        self.db.conn().execute("INSERT INTO shark_credit_note_mutation(company_slug,credit_note_id,action,before_json,after_json,actor) VALUES(?1,?2,?3,?4,?5,?6)",(&self.company_slug,id,action,before,after,self.actor.name())).map_err(sqlite_error)?;
        self.credit_mutation(self.db.conn().last_insert_rowid())
    }
    pub fn credit_note_history(
        &self,
        id: &str,
        limit: i64,
    ) -> FoundationResult<Vec<CreditNoteMutationView>> {
        let id = normalized_id(id, "credit note id")?;
        if !(1..=MAX_INVOICE_HISTORY_PAGE).contains(&limit) {
            return Err(validation(
                "credit-note history limit must be between 1 and 500",
            ));
        }
        self.credit_note(&id)?;
        let mut statement=self.db.conn().prepare("SELECT id FROM shark_credit_note_mutation WHERE company_slug=?1 AND credit_note_id=?2 ORDER BY id DESC LIMIT ?3").map_err(sqlite_error)?;
        let ids = statement
            .query_map((&self.company_slug, &id, limit), |r| r.get::<_, i64>(0))
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        ids.into_iter()
            .map(|value| self.credit_mutation(value))
            .collect()
    }
    pub fn create_credit_note_draft(
        &self,
        write: &CreditNoteDraftWrite,
    ) -> FoundationResult<CreditNoteMutationOutcome> {
        let id = normalized_id(&write.credit_note_id, "credit note id")?;
        let invoice_id = normalized_id(&write.invoice_id, "invoice id")?;
        self.shark_savepoint("shark_credit_create",||{let invoice=self.invoice(&invoice_id)?;if !matches!(invoice.state,InvoiceState::Issued|InvoiceState::PartPaid|InvoiceState::Paid){return Err(validation("credit note must reference an existing issued invoice"));}self.db.conn().execute("INSERT INTO shark_credit_note(company_slug,credit_note_id,invoice_id,state,created_by,updated_by) VALUES(?1,?2,?3,'draft',?4,?4)",(&self.company_slug,&id,&invoice_id,self.actor.name())).map_err(sqlite_error)?;let credit=self.credit_note(&id)?;let mutation=self.record_credit_mutation(&id,"create_draft",None,Some(&credit))?;Ok(CreditNoteMutationOutcome{credit_note:credit,mutation,invoice,invoice_mutation:None})})
    }
    pub fn save_credit_note_line(
        &self,
        credit_id: &str,
        write: &CreditNoteLineWrite,
    ) -> FoundationResult<CreditNoteMutationOutcome> {
        let id = normalized_id(credit_id, "credit note id")?;
        let line_id = normalized_id(&write.line_id, "credit-note line id")?;
        let invoice_line_id = normalized_id(&write.invoice_line_id, "invoice line id")?;
        if write.quantity_subunits <= 0 {
            return Err(validation(
                "credit-note line quantity must be positive integer subunits",
            ));
        }
        self.shark_savepoint("shark_credit_line_save",||{let before=self.credit_note(&id)?;if before.state!=CreditNoteState::Draft{return Err(validation("credit-note lines are immutable after issue"));}let invoice=self.invoice(&before.invoice_id)?;let source=invoice.issued_snapshot.as_ref().and_then(|s|s.lines.iter().find(|line|line.line_id==invoice_line_id)).ok_or_else(||validation("credit-note line must reference an issued invoice line"))?;if write.quantity_subunits>source.quantity_subunits{return Err(validation("credit-note quantity cannot exceed the referenced invoice line"));}let position=before.lines.iter().find(|l|l.line_id==line_id).map(|l|l.position).unwrap_or_else(||before.lines.iter().map(|l|l.position).max().unwrap_or(0)+1);let action=if before.lines.iter().any(|l|l.line_id==line_id){"replace_line"}else{"add_line"};self.db.conn().execute("INSERT INTO shark_credit_note_line(company_slug,credit_note_id,line_id,invoice_line_id,description,quantity_subunits,unit_price_minor,position) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(company_slug,credit_note_id,line_id) DO UPDATE SET invoice_line_id=excluded.invoice_line_id,description=excluded.description,quantity_subunits=excluded.quantity_subunits,unit_price_minor=excluded.unit_price_minor",(&self.company_slug,&id,&line_id,&invoice_line_id,&source.description,write.quantity_subunits,source.unit_price_minor,position)).map_err(sqlite_error)?;let credit=self.credit_note(&id)?;let mutation=self.record_credit_mutation(&id,action,Some(&before),Some(&credit))?;Ok(CreditNoteMutationOutcome{credit_note:credit,mutation,invoice,invoice_mutation:None})})
    }
    pub fn remove_credit_note_line(
        &self,
        credit_id: &str,
        line_id: &str,
    ) -> FoundationResult<CreditNoteMutationOutcome> {
        let id = normalized_id(credit_id, "credit note id")?;
        let line_id = normalized_id(line_id, "credit-note line id")?;
        self.shark_savepoint("shark_credit_line_remove",||{let before=self.credit_note(&id)?;if before.state!=CreditNoteState::Draft{return Err(validation("credit-note lines are immutable after issue"));}let changed=self.db.conn().execute("DELETE FROM shark_credit_note_line WHERE company_slug=?1 AND credit_note_id=?2 AND line_id=?3",(&self.company_slug,&id,&line_id)).map_err(sqlite_error)?;if changed!=1{return Err(FoundationError::new(FoundationErrorCode::NotFound,"credit-note line was not found"));}let credit=self.credit_note(&id)?;let invoice=self.invoice(&credit.invoice_id)?;let mutation=self.record_credit_mutation(&id,"remove_line",Some(&before),Some(&credit))?;Ok(CreditNoteMutationOutcome{credit_note:credit,mutation,invoice,invoice_mutation:None})})
    }
    fn refresh_invoice_payment_state(&self, invoice: &InvoiceView) -> FoundationResult<()> {
        let credited = self.active_credit_total(&invoice.invoice_id)?;
        let outstanding = invoice
            .total_minor
            .checked_sub(invoice.paid_minor)
            .and_then(|v| v.checked_sub(credited))
            .ok_or_else(|| storage("invoice balance overflow"))?;
        let state = if invoice.paid_minor == 0 {
            InvoiceState::Issued
        } else if outstanding == 0 {
            InvoiceState::Paid
        } else {
            InvoiceState::PartPaid
        };
        if invoice.state != state {
            self.db.conn().execute("UPDATE shark_invoice SET state=?1,updated_by=?2,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?3 AND invoice_id=?4",(state.as_str(),self.actor.name(),&self.company_slug,&invoice.invoice_id)).map_err(sqlite_error)?;
        }
        Ok(())
    }
    pub fn issue_credit_note(
        &self,
        credit_id: &str,
        number: &str,
    ) -> FoundationResult<CreditNoteMutationOutcome> {
        let id = normalized_id(credit_id, "credit note id")?;
        let number = safe_text(number, "credit-note number", MAX_NUMBER_BYTES)?;
        self.shark_savepoint("shark_credit_issue",||{let before=self.credit_note(&id)?;if before.state!=CreditNoteState::Draft{return Err(validation("only a draft credit note can be issued"));}if before.lines.is_empty(){return Err(validation("credit note requires at least one line before issue"));}let invoice_before=self.invoice(&before.invoice_id)?;if !matches!(invoice_before.state,InvoiceState::Issued|InvoiceState::PartPaid|InvoiceState::Paid){return Err(validation("credit note must reference an active issued invoice"));}for line in &before.lines{let used:i64=self.db.conn().query_row("SELECT COALESCE(SUM(l.quantity_subunits),0) FROM shark_credit_note_issue_line l JOIN shark_credit_note c ON c.company_slug=l.company_slug AND c.credit_note_id=l.credit_note_id WHERE c.company_slug=?1 AND c.invoice_id=?2 AND c.state='issued' AND l.invoice_line_id=?3",(&self.company_slug,&before.invoice_id,&line.invoice_line_id),|r|r.get(0)).map_err(sqlite_error)?;let source=invoice_before.issued_snapshot.as_ref().and_then(|s|s.lines.iter().find(|v|v.line_id==line.invoice_line_id)).ok_or_else(||storage("referenced issued invoice line is missing"))?;let proposed=used.checked_add(line.quantity_subunits).ok_or_else(||validation("credit-note quantity overflow"))?;if proposed>source.quantity_subunits{return Err(validation("credit note cannot exceed the remaining creditable invoice-line quantity"));}}
        let total=checked_credit_total(&before.lines)?;let remaining=invoice_before.outstanding_minor;if total<=0||total>remaining{return Err(validation("credit note must be positive and cannot exceed the remaining creditable invoice amount"));}let invoice_snapshot=invoice_before.issued_snapshot.as_ref().ok_or_else(||storage("issued invoice snapshot is missing"))?;self.db.conn().execute("INSERT INTO shark_credit_note_issue_snapshot(company_slug,credit_note_id,credit_note_number,invoice_id,invoice_number,customer_id,customer_display_name,customer_postal_address,customer_email,customer_phone,total_minor,line_count,issued_by) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",(&self.company_slug,&id,&number,&before.invoice_id,&invoice_snapshot.invoice_number,&invoice_snapshot.customer.customer_id,&invoice_snapshot.customer.display_name,&invoice_snapshot.customer.postal_address,&invoice_snapshot.customer.email,&invoice_snapshot.customer.phone,total,i64::try_from(before.lines.len()).map_err(|_|validation("credit-note line count overflow"))?,self.actor.name())).map_err(sqlite_error)?;let copied=self.db.conn().execute("INSERT INTO shark_credit_note_issue_line(company_slug,credit_note_id,line_id,invoice_line_id,description,quantity_subunits,unit_price_minor,position) SELECT company_slug,credit_note_id,line_id,invoice_line_id,description,quantity_subunits,unit_price_minor,position FROM shark_credit_note_line WHERE company_slug=?1 AND credit_note_id=?2",(&self.company_slug,&id)).map_err(sqlite_error)?;if copied!=before.lines.len(){return Err(storage("issued credit-note line copy was incomplete"));}self.db.conn().execute("UPDATE shark_credit_note SET state='issued',updated_by=?1,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?2 AND credit_note_id=?3 AND state='draft'",(self.actor.name(),&self.company_slug,&id)).map_err(sqlite_error)?;let credit=self.credit_note(&id)?;self.refresh_invoice_payment_state(&invoice_before)?;let invoice=self.invoice(&before.invoice_id)?;let mutation=self.record_credit_mutation(&id,"issue",Some(&before),Some(&credit))?;let invoice_mutation=Some(self.record_invoice_mutation(&before.invoice_id,"credit_issued",Some(&invoice_before),Some(&invoice))?);Ok(CreditNoteMutationOutcome{credit_note:credit,mutation,invoice,invoice_mutation})})
    }
    pub fn cancel_credit_note(
        &self,
        credit_id: &str,
    ) -> FoundationResult<CreditNoteMutationOutcome> {
        let id = normalized_id(credit_id, "credit note id")?;
        self.shark_savepoint("shark_credit_cancel",||{let before=self.credit_note(&id)?;if before.state!=CreditNoteState::Draft{return Err(validation("only a draft credit note can be cancelled"));}let invoice_before=self.invoice(&before.invoice_id)?;self.db.conn().execute("UPDATE shark_credit_note SET state='cancelled',updated_by=?1,updated_at=CURRENT_TIMESTAMP WHERE company_slug=?2 AND credit_note_id=?3 AND state IN ('draft','issued')",(self.actor.name(),&self.company_slug,&id)).map_err(sqlite_error)?;let credit=self.credit_note(&id)?;let invoice_mutation=if before.state==CreditNoteState::Issued{self.refresh_invoice_payment_state(&invoice_before)?;let invoice=self.invoice(&before.invoice_id)?;Some(self.record_invoice_mutation(&before.invoice_id,"credit_cancelled",Some(&invoice_before),Some(&invoice))?)}else{None};let invoice=self.invoice(&before.invoice_id)?;let mutation=self.record_credit_mutation(&id,"cancel",Some(&before),Some(&credit))?;Ok(CreditNoteMutationOutcome{credit_note:credit,mutation,invoice,invoice_mutation})})
    }
}

fn serialize<T: Serialize>(value: Option<&T>, label: &str) -> FoundationResult<Option<String>> {
    value
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| storage(format!("could not serialize {label} mutation facts")))
}
fn parse_json<T: for<'de> Deserialize<'de>>(
    value: Option<String>,
    label: &str,
) -> FoundationResult<Option<T>> {
    value
        .map(|json| {
            serde_json::from_str(&json)
                .map_err(|_| storage(format!("persisted {label} mutation snapshot is invalid")))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "shark-sbc8a3-{label}-{}-{nonce}.db",
            std::process::id()
        ))
    }
    fn books(label: &str) -> (PathBuf, Books) {
        let p = path(label);
        let b =
            Books::create_plain_for_test(&p, "invoice-books", "Invoice Books", "owner").unwrap();
        b.save_contact(&ContactWrite {
            contact_id: "customer-1".into(),
            kind: "customer".into(),
            display_name: "Original Customer".into(),
            postal_address: Some("1 High Street".into()),
            email: None,
            phone: None,
        })
        .unwrap();
        (p, b)
    }
    fn draft(b: &Books, id: &str) {
        b.create_invoice_draft(&InvoiceDraftWrite {
            invoice_id: id.into(),
            customer_id: "customer-1".into(),
            issue_date: "2026-10-02".into(),
            due_date: Some("2026-10-31".into()),
        })
        .unwrap();
        b.save_invoice_line(
            id,
            &InvoiceLineWrite {
                line_id: "line-1".into(),
                description: "Service".into(),
                quantity_subunits: 2,
                unit_price_minor: 12_500,
            },
        )
        .unwrap();
    }
    #[test]
    fn invoice_credit_and_manual_payment_are_audited_without_bank_fabrication() {
        let (p, b) = books("lifecycle");
        let transactions = b.count_transactions().unwrap();
        draft(&b, "invoice-1");
        b.issue_invoice("invoice-1", "INV-001").unwrap();
        b.record_manual_invoice_payment("invoice-1", 5_000).unwrap();
        b.create_credit_note_draft(&CreditNoteDraftWrite {
            credit_note_id: "credit-1".into(),
            invoice_id: "invoice-1".into(),
        })
        .unwrap();
        b.save_credit_note_line(
            "credit-1",
            &CreditNoteLineWrite {
                line_id: "credit-line-1".into(),
                invoice_line_id: "line-1".into(),
                quantity_subunits: 1,
            },
        )
        .unwrap();
        let outcome = b.issue_credit_note("credit-1", "CN-001").unwrap();
        assert_eq!(outcome.invoice.paid_minor, 5_000);
        assert_eq!(outcome.invoice.credited_minor, 12_500);
        assert_eq!(outcome.invoice.outstanding_minor, 7_500);
        assert_eq!(b.count_transactions().unwrap(), transactions);
        assert!(
            b.invoice_history("invoice-1", 50)
                .unwrap()
                .iter()
                .any(|m| m.action == "credit_issued")
        );
        drop(b);
        remove_sqlite_artifacts(&p);
    }
    #[test]
    fn over_credit_and_failed_mutations_leave_state_and_history_unchanged() {
        let (p, b) = books("fail-closed");
        draft(&b, "invoice-1");
        b.issue_invoice("invoice-1", "INV-002").unwrap();
        b.create_credit_note_draft(&CreditNoteDraftWrite {
            credit_note_id: "credit-1".into(),
            invoice_id: "invoice-1".into(),
        })
        .unwrap();
        b.save_credit_note_line(
            "credit-1",
            &CreditNoteLineWrite {
                line_id: "credit-line-1".into(),
                invoice_line_id: "line-1".into(),
                quantity_subunits: 2,
            },
        )
        .unwrap();
        b.issue_credit_note("credit-1", "CN-002").unwrap();
        b.create_credit_note_draft(&CreditNoteDraftWrite {
            credit_note_id: "credit-2".into(),
            invoice_id: "invoice-1".into(),
        })
        .unwrap();
        b.save_credit_note_line(
            "credit-2",
            &CreditNoteLineWrite {
                line_id: "credit-line-2".into(),
                invoice_line_id: "line-1".into(),
                quantity_subunits: 1,
            },
        )
        .unwrap();
        let before = b.credit_note_history("credit-2", 50).unwrap();
        assert!(b.issue_credit_note("credit-2", "CN-003").is_err());
        assert_eq!(
            b.credit_note("credit-2").unwrap().state,
            CreditNoteState::Draft
        );
        assert_eq!(b.credit_note_history("credit-2", 50).unwrap(), before);
        assert!(b.db.conn().is_autocommit());
        drop(b);
        remove_sqlite_artifacts(&p);
    }
    #[test]
    fn accepted_quote_conversion_is_explicit_exactly_once_and_copies_snapshot() {
        let (p, b) = books("conversion");
        b.create_quote_draft(&QuoteDraftWrite {
            quote_id: "quote-1".into(),
            kind: QuoteKind::Quote,
            customer_id: "customer-1".into(),
        })
        .unwrap();
        b.save_quote_line(
            "quote-1",
            &QuoteLineWrite {
                line_id: "quote-line".into(),
                description: "Quoted service".into(),
                quantity_subunits: 3,
                unit_price_minor: 4_000,
            },
        )
        .unwrap();
        b.issue_quote("quote-1", "Q-001").unwrap();
        b.transition_quote("quote-1", QuoteState::Accepted).unwrap();
        let converted = b
            .convert_accepted_quote("quote-1", "invoice-1", "2026-10-02", None)
            .unwrap();
        assert_eq!(converted.invoice.total_minor, 12_000);
        assert_eq!(
            converted.invoice.source_quote_id.as_deref(),
            Some("quote-1")
        );
        assert!(!b.quote("quote-1").unwrap().conversion_eligible);
        let history = b.invoice_history("invoice-1", 20).unwrap();
        assert_eq!(history[0].action, "quote_conversion");
        assert!(
            b.convert_accepted_quote("quote-1", "invoice-2", "2026-10-02", None)
                .is_err()
        );
        assert!(b.invoice_optional("invoice-2").unwrap().is_none());
        drop(b);
        remove_sqlite_artifacts(&p);
    }
    #[test]
    fn database_guards_keep_issued_numbers_snapshots_and_history_immutable() {
        let (p, b) = books("guards");
        draft(&b, "invoice-1");
        b.issue_invoice("invoice-1", "INV-IMMUTABLE").unwrap();
        assert!(b.db.conn().execute("UPDATE shark_invoice_issue_snapshot SET invoice_number='CHANGED' WHERE company_slug=?1 AND invoice_id='invoice-1'",(&b.company_slug,)).is_err());
        assert!(
            b.db.conn()
                .execute(
                    "DELETE FROM shark_invoice_mutation WHERE company_slug=?1",
                    (&b.company_slug,)
                )
                .is_err()
        );
        assert_eq!(
            b.invoice("invoice-1")
                .unwrap()
                .issued_snapshot
                .unwrap()
                .invoice_number,
            "INV-IMMUTABLE"
        );
        drop(b);
        remove_sqlite_artifacts(&p);
    }
}

fn ensure_invoice_guards(db: &Db) -> FoundationResult<()> {
    db.conn().execute_batch(r#"
    CREATE TRIGGER IF NOT EXISTS trg_invoice_state_guard BEFORE UPDATE ON shark_invoice
    BEGIN
      SELECT CASE WHEN NEW.company_slug IS NOT OLD.company_slug OR NEW.invoice_id IS NOT OLD.invoice_id OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at THEN RAISE(ABORT,'invoice identity immutable') END;
      SELECT CASE WHEN OLD.state='cancelled' OR (OLD.state!='draft' AND NEW.state='draft') THEN RAISE(ABORT,'invoice transition invalid') END;
      SELECT CASE WHEN typeof(NEW.paid_minor)!='integer' OR NEW.paid_minor<OLD.paid_minor OR (NEW.paid_minor!=OLD.paid_minor AND OLD.state NOT IN ('issued','part_paid')) THEN RAISE(ABORT,'invalid manual payment') END;
      SELECT CASE WHEN NEW.state='cancelled' AND (NEW.paid_minor!=0 OR EXISTS(SELECT 1 FROM shark_credit_note WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id AND state='issued')) THEN RAISE(ABORT,'invoice has payments or credits') END;
      SELECT CASE WHEN NEW.state IN ('issued','part_paid','paid') AND NOT EXISTS(SELECT 1 FROM shark_invoice_issue_snapshot s WHERE s.company_slug=NEW.company_slug AND s.invoice_id=NEW.invoice_id AND s.line_count=(SELECT COUNT(*) FROM shark_invoice_issue_line l WHERE l.company_slug=s.company_slug AND l.invoice_id=s.invoice_id) AND s.total_minor=(SELECT SUM(quantity_subunits*unit_price_minor) FROM shark_invoice_issue_line l WHERE l.company_slug=s.company_slug AND l.invoice_id=s.invoice_id)) THEN RAISE(ABORT,'complete issued snapshot required') END;
      SELECT CASE WHEN NEW.paid_minor > COALESCE((SELECT total_minor FROM shark_invoice_issue_snapshot WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id),0)-COALESCE((SELECT SUM(s.total_minor) FROM shark_credit_note_issue_snapshot s JOIN shark_credit_note c USING(company_slug,credit_note_id) WHERE c.company_slug=NEW.company_slug AND c.invoice_id=NEW.invoice_id AND c.state='issued'),0) THEN RAISE(ABORT,'manual payment exceeds balance') END;
      SELECT CASE WHEN OLD.source_quote_id IS NOT NULL AND (NEW.customer_id IS NOT OLD.customer_id OR NEW.customer_display_name IS NOT OLD.customer_display_name OR NEW.customer_postal_address IS NOT OLD.customer_postal_address OR NEW.customer_email IS NOT OLD.customer_email OR NEW.customer_phone IS NOT OLD.customer_phone) THEN RAISE(ABORT,'converted customer immutable') END;
    END;
    CREATE TRIGGER IF NOT EXISTS trg_invoice_insert_guard BEFORE INSERT ON shark_invoice
    BEGIN
      SELECT CASE WHEN NEW.state!='draft' OR NEW.paid_minor!=0 THEN RAISE(ABORT,'invoice starts as draft') END;
      SELECT CASE WHEN NEW.source_quote_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM shark_quote q JOIN shark_quote_issue_snapshot s USING(company_slug,quote_id) WHERE q.company_slug=NEW.company_slug AND q.quote_id=NEW.source_quote_id AND q.state='accepted' AND q.conversion_eligible=1 AND s.customer_id IS NEW.customer_id AND s.customer_display_name IS NEW.customer_display_name AND s.customer_postal_address IS NEW.customer_postal_address AND s.customer_email IS NEW.customer_email AND s.customer_phone IS NEW.customer_phone) THEN RAISE(ABORT,'conversion requires eligible issued quote facts') END;
    END;
    CREATE TRIGGER IF NOT EXISTS trg_invoice_converted_line_insert BEFORE INSERT ON shark_invoice_line
    WHEN (SELECT source_quote_id FROM shark_invoice WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id) IS NOT NULL
    BEGIN
      SELECT CASE WHEN EXISTS(SELECT 1 FROM shark_quote_conversion WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id) THEN RAISE(ABORT,'converted lines immutable') END;
      SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM shark_invoice i JOIN shark_quote_issue_line q ON q.company_slug=i.company_slug AND q.quote_id=i.source_quote_id WHERE i.company_slug=NEW.company_slug AND i.invoice_id=NEW.invoice_id AND q.line_id=NEW.line_id AND q.description=NEW.description AND q.quantity_subunits=NEW.quantity_subunits AND q.unit_price_minor=NEW.unit_price_minor AND q.position=NEW.position) THEN RAISE(ABORT,'conversion must copy issued quote lines') END;
    END;
    CREATE TRIGGER IF NOT EXISTS trg_invoice_converted_line_update BEFORE UPDATE ON shark_invoice_line WHEN (SELECT source_quote_id FROM shark_invoice WHERE company_slug=OLD.company_slug AND invoice_id=OLD.invoice_id) IS NOT NULL BEGIN SELECT RAISE(ABORT,'converted lines immutable'); END;
    CREATE TRIGGER IF NOT EXISTS trg_invoice_converted_line_delete BEFORE DELETE ON shark_invoice_line WHEN (SELECT source_quote_id FROM shark_invoice WHERE company_slug=OLD.company_slug AND invoice_id=OLD.invoice_id) IS NOT NULL BEGIN SELECT RAISE(ABORT,'converted lines immutable'); END;
    CREATE TRIGGER IF NOT EXISTS trg_quote_conversion_binding_guard BEFORE INSERT ON shark_quote_conversion
    WHEN NOT EXISTS(SELECT 1 FROM shark_quote q JOIN shark_quote_issue_snapshot s USING(company_slug,quote_id) JOIN shark_invoice i ON i.company_slug=q.company_slug AND i.source_quote_id=q.quote_id WHERE q.company_slug=NEW.company_slug AND q.quote_id=NEW.quote_id AND q.state='accepted' AND q.conversion_eligible=1 AND i.invoice_id=NEW.invoice_id AND s.commercial_number=NEW.source_quote_number AND s.line_count=(SELECT COUNT(*) FROM shark_invoice_line l WHERE l.company_slug=i.company_slug AND l.invoice_id=i.invoice_id)) BEGIN SELECT RAISE(ABORT,'invalid conversion binding'); END;
    CREATE TRIGGER IF NOT EXISTS trg_credit_create_guard BEFORE INSERT ON shark_credit_note WHEN NEW.state!='draft' OR NOT EXISTS(SELECT 1 FROM shark_invoice WHERE company_slug=NEW.company_slug AND invoice_id=NEW.invoice_id AND state IN ('issued','part_paid','paid')) BEGIN SELECT RAISE(ABORT,'credit requires existing issued invoice'); END;
    CREATE TRIGGER IF NOT EXISTS trg_credit_state_guard BEFORE UPDATE ON shark_credit_note
    BEGIN
      SELECT CASE WHEN NEW.company_slug IS NOT OLD.company_slug OR NEW.credit_note_id IS NOT OLD.credit_note_id OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at THEN RAISE(ABORT,'credit identity immutable') END;
      SELECT CASE WHEN OLD.state!='draft' THEN RAISE(ABORT,'issued or cancelled credit immutable') END;
      SELECT CASE WHEN NEW.state='issued' AND NOT EXISTS(SELECT 1 FROM shark_credit_note_issue_snapshot s JOIN shark_invoice_issue_snapshot i ON i.company_slug=s.company_slug AND i.invoice_id=s.invoice_id JOIN shark_invoice inv ON inv.company_slug=i.company_slug AND inv.invoice_id=i.invoice_id WHERE s.company_slug=NEW.company_slug AND s.credit_note_id=NEW.credit_note_id AND s.invoice_id=NEW.invoice_id AND inv.state IN ('issued','part_paid','paid') AND s.line_count=(SELECT COUNT(*) FROM shark_credit_note_issue_line l WHERE l.company_slug=s.company_slug AND l.credit_note_id=s.credit_note_id) AND s.total_minor>0 AND s.total_minor=(SELECT SUM(quantity_subunits*unit_price_minor) FROM shark_credit_note_issue_line l WHERE l.company_slug=s.company_slug AND l.credit_note_id=s.credit_note_id) AND s.total_minor<=i.total_minor-inv.paid_minor-COALESCE((SELECT SUM(cs.total_minor) FROM shark_credit_note_issue_snapshot cs JOIN shark_credit_note c USING(company_slug,credit_note_id) WHERE c.company_slug=NEW.company_slug AND c.invoice_id=NEW.invoice_id AND c.state='issued'),0)) THEN RAISE(ABORT,'credit exceeds remaining invoice balance or incomplete snapshot') END;
    END;
    CREATE TRIGGER IF NOT EXISTS trg_credit_line_remaining_guard BEFORE INSERT ON shark_credit_note_issue_line
    BEGIN
      SELECT CASE WHEN (SELECT state FROM shark_credit_note WHERE company_slug=NEW.company_slug AND credit_note_id=NEW.credit_note_id)!='draft' THEN RAISE(ABORT,'issued credit lines immutable') END;
      SELECT CASE WHEN NEW.quantity_subunits > (SELECT l.quantity_subunits FROM shark_invoice_issue_line l JOIN shark_credit_note c ON c.company_slug=l.company_slug AND c.invoice_id=l.invoice_id WHERE c.company_slug=NEW.company_slug AND c.credit_note_id=NEW.credit_note_id AND l.line_id=NEW.invoice_line_id)-COALESCE((SELECT SUM(l.quantity_subunits) FROM shark_credit_note_issue_line l JOIN shark_credit_note c USING(company_slug,credit_note_id) WHERE c.company_slug=NEW.company_slug AND c.state='issued' AND c.invoice_id=(SELECT invoice_id FROM shark_credit_note WHERE company_slug=NEW.company_slug AND credit_note_id=NEW.credit_note_id) AND l.invoice_line_id=NEW.invoice_line_id),0) THEN RAISE(ABORT,'credit exceeds remaining line quantity') END;
    END;
    "#).map_err(sqlite_error)?;
    // REPLACE bypasses delete triggers if recursive triggers are disabled.
    for (table, keys) in [
        ("shark_invoice", "company_slug,invoice_id"), ("shark_invoice_issue_snapshot", "company_slug,invoice_id"), ("shark_invoice_issue_line", "company_slug,invoice_id,line_id"),
        ("shark_credit_note", "company_slug,credit_note_id"), ("shark_credit_note_issue_snapshot", "company_slug,credit_note_id"), ("shark_credit_note_issue_line", "company_slug,credit_note_id,line_id"),
        ("shark_quote_conversion", "company_slug,quote_id"), ("shark_invoice_mutation", "id"), ("shark_credit_note_mutation", "id"),
    ] {
        let predicate=keys.split(',').map(|k|format!("{k}=NEW.{k}")).collect::<Vec<_>>().join(" AND ");
        db.conn().execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS trg_{table}_no_replace BEFORE INSERT ON {table} WHEN EXISTS(SELECT 1 FROM {table} WHERE {predicate}) BEGIN SELECT RAISE(ABORT,'immutable identity cannot be replaced'); END;")).map_err(sqlite_error)?;
    }
    for table in ["shark_invoice_line","shark_invoice_issue_line","shark_credit_note_line","shark_credit_note_issue_line"] {
        for event in ["INSERT","UPDATE"] {
            db.conn().execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS trg_{table}_integer_{event} BEFORE {event} ON {table} WHEN typeof(NEW.quantity_subunits)!='integer' OR typeof(NEW.unit_price_minor)!='integer' OR typeof(NEW.quantity_subunits*NEW.unit_price_minor)!='integer' OR typeof(NEW.position)!='integer' BEGIN SELECT RAISE(ABORT,'whole-pence checked line arithmetic required'); END;")).map_err(sqlite_error)?;
        }
    }
    for table in ["shark_invoice_issue_snapshot","shark_credit_note_issue_snapshot"] {
        db.conn().execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS trg_{table}_integer BEFORE INSERT ON {table} WHEN typeof(NEW.total_minor)!='integer' OR typeof(NEW.line_count)!='integer' BEGIN SELECT RAISE(ABORT,'integer snapshot total required'); END;")).map_err(sqlite_error)?;
    }
    Ok(())
}
