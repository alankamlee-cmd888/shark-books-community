//! SBC-7B1 Slice 2B Shark-owned bank activity persistence.
//!
//! This module owns only application metadata inside the encrypted books
//! database. It does not expose the raw connection, SQL types or Beankeeper
//! objects outside the foundation crate. Imported bank activity is deliberately
//! separate from accounting transactions until an owner-confirmed accounting
//! action exists.

use super::*;
use std::collections::HashSet;

pub(super) const BANK_APPLICATION_SCHEMA_VERSION: u32 = SHARK_APPLICATION_SCHEMA_VERSION;
const BUSINESS_BANK_ACCOUNT_CODE: &str = "1000";
const MAX_ACTIVITY_BATCH: usize = 10_000;
const MAX_ACTIVITY_PAGE: i64 = 500;
const MAX_RECONCILIATION_ENTRIES: usize = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankActivityWrite {
    pub source_account_id: String,
    pub institution_account_id: Option<String>,
    pub source_format: String,
    pub source_file_sha256: String,
    pub source_locator: String,
    pub posted_date: String,
    pub value_date: Option<String>,
    pub signed_amount_minor: i64,
    pub currency_code: String,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub external_transaction_id: Option<String>,
    pub raw_record_sha256: String,
    pub strong_identity_key: Option<String>,
    pub provenance_kind: String,
    pub provenance_source_reference: Option<String>,
    pub provenance_fingerprint: Option<String>,
    pub provenance_label: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BankActivityPersistKind {
    Created,
    StrongDuplicate,
    FileExactDuplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankActivityPersistOutcome {
    pub source_locator: String,
    pub activity_id: i64,
    pub kind: BankActivityPersistKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankActivityView {
    pub id: i64,
    pub activity: BankActivityWrite,
    pub imported_at: String,
    pub imported_by: String,
    pub matched_transaction_id: Option<i64>,
    pub matched_entry_id: Option<i64>,
    pub match_level: Option<String>,
    pub match_score: Option<u16>,
    pub match_reasons: Vec<String>,
    pub clearance_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankMatchView {
    pub bank_activity_id: i64,
    pub transaction_id: i64,
    pub entry_id: i64,
    pub match_level: String,
    pub score: u16,
    pub reasons: Vec<String>,
    pub confirmed_by: String,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BankMatchPersistOutcome {
    Confirmed(BankMatchView),
    AlreadyConfirmed(BankMatchView),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankReconciliationEntryWrite {
    pub transaction_id: i64,
    pub entry_id: i64,
    pub signed_amount_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankReconciliationWrite {
    pub statement_id: String,
    pub statement_date: String,
    pub opening_balance_minor: i64,
    pub ending_balance_minor: i64,
    pub entries: Vec<BankReconciliationEntryWrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankReconciliationView {
    pub statement_id: String,
    pub statement_date: String,
    pub opening_balance_minor: i64,
    pub ending_balance_minor: i64,
    pub entry_count: usize,
    pub finalized_by: String,
    pub finalized_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BankReconciliationPersistOutcome {
    Finalized(BankReconciliationView),
    AlreadyFinalized(BankReconciliationView),
}

pub(super) fn ensure_application_schema(db: &Db) -> FoundationResult<()> {
    db.conn()
        .execute_batch("SAVEPOINT shark_application_schema_v2")
        .map_err(sqlite_error)?;
    let migration = db.conn().execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS shark_application_meta (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            schema_version INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS shark_bank_activity (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            company_slug TEXT NOT NULL,
            strong_identity_key TEXT,
            source_file_sha256 TEXT NOT NULL,
            source_locator TEXT NOT NULL,
            raw_record_sha256 TEXT NOT NULL,
            source_account_id TEXT NOT NULL,
            posted_date TEXT NOT NULL,
            signed_amount_minor INTEGER NOT NULL CHECK(signed_amount_minor <> 0),
            currency TEXT NOT NULL CHECK(currency = 'GBP'),
            payload_json TEXT NOT NULL,
            imported_by TEXT NOT NULL,
            imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(company_slug, source_file_sha256, source_locator, raw_record_sha256)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_shark_bank_activity_strong
            ON shark_bank_activity(company_slug, strong_identity_key)
            WHERE strong_identity_key IS NOT NULL;
        CREATE INDEX IF NOT EXISTS idx_shark_bank_activity_date
            ON shark_bank_activity(company_slug, posted_date, id);
        CREATE TABLE IF NOT EXISTS shark_bank_match (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            company_slug TEXT NOT NULL,
            bank_activity_id INTEGER NOT NULL,
            transaction_id INTEGER NOT NULL,
            entry_id INTEGER NOT NULL,
            match_level TEXT NOT NULL,
            score INTEGER NOT NULL,
            reasons_json TEXT NOT NULL,
            confirmed_by TEXT NOT NULL,
            confirmed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(company_slug, bank_activity_id),
            UNIQUE(company_slug, transaction_id, entry_id),
            FOREIGN KEY(bank_activity_id) REFERENCES shark_bank_activity(id) ON DELETE RESTRICT
        );
        CREATE TABLE IF NOT EXISTS shark_bank_reconciliation (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            company_slug TEXT NOT NULL,
            statement_id TEXT NOT NULL,
            statement_date TEXT NOT NULL,
            opening_balance_minor INTEGER NOT NULL,
            ending_balance_minor INTEGER NOT NULL,
            finalized_by TEXT NOT NULL,
            finalized_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(company_slug, statement_id)
        );
        CREATE TABLE IF NOT EXISTS shark_bank_reconciliation_entry (
            reconciliation_id INTEGER NOT NULL,
            bank_activity_id INTEGER NOT NULL,
            transaction_id INTEGER NOT NULL,
            entry_id INTEGER NOT NULL,
            signed_amount_minor INTEGER NOT NULL CHECK(signed_amount_minor <> 0),
            PRIMARY KEY(reconciliation_id, entry_id),
            FOREIGN KEY(reconciliation_id) REFERENCES shark_bank_reconciliation(id) ON DELETE RESTRICT,
            FOREIGN KEY(bank_activity_id) REFERENCES shark_bank_activity(id) ON DELETE RESTRICT
        );
        INSERT INTO shark_application_meta(id, schema_version)
            VALUES(1, 2)
            ON CONFLICT(id) DO UPDATE SET schema_version = excluded.schema_version
            WHERE shark_application_meta.schema_version < excluded.schema_version;
        "#,
    );
    if let Err(error) = migration {
        let _ = db
            .conn()
            .execute_batch("ROLLBACK TO shark_application_schema_v2; RELEASE shark_application_schema_v2");
        return Err(sqlite_error(error));
    }
    db.conn()
        .execute_batch("RELEASE shark_application_schema_v2")
        .map_err(sqlite_error)?;

    let observed: i64 = db
        .conn()
        .query_row(
            "SELECT schema_version FROM shark_application_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    if observed != i64::from(BANK_APPLICATION_SCHEMA_VERSION) {
        return Err(FoundationError::new(
            FoundationErrorCode::Unsupported,
            format!(
                "Shark application schema version {observed} is not supported; expected {}",
                BANK_APPLICATION_SCHEMA_VERSION
            ),
        ));
    }
    Ok(())
}

fn sqlite_error<E: fmt::Display>(error: E) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Storage, error.to_string())
}

fn validation(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Validation, message)
}

fn nonblank(value: &str, field: &str, max: usize) -> FoundationResult<()> {
    if value.trim().is_empty() {
        return Err(validation(format!("{field} must not be blank")));
    }
    if value.len() > max {
        return Err(validation(format!("{field} exceeds the bounded length")));
    }
    Ok(())
}

fn optional_nonblank(value: Option<&str>, field: &str, max: usize) -> FoundationResult<()> {
    if let Some(value) = value {
        nonblank(value, field, max)?;
    }
    Ok(())
}

fn sha256(value: &str, field: &str) -> FoundationResult<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(validation(format!("{field} must be a 64-character SHA-256 hex value")));
    }
    Ok(())
}

fn iso_date(value: &str, field: &str) -> FoundationResult<()> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| validation(format!("{field} must be YYYY-MM-DD")))
}

fn validate_activity(activity: &BankActivityWrite) -> FoundationResult<()> {
    nonblank(&activity.source_account_id, "source account id", 128)?;
    nonblank(&activity.source_locator, "source locator", 1024)?;
    nonblank(&activity.description, "description", 4096)?;
    optional_nonblank(activity.institution_account_id.as_deref(), "institution account id", 512)?;
    optional_nonblank(activity.payee.as_deref(), "payee", 2048)?;
    optional_nonblank(activity.reference.as_deref(), "reference", 2048)?;
    optional_nonblank(activity.external_transaction_id.as_deref(), "external transaction id", 1024)?;
    optional_nonblank(activity.strong_identity_key.as_deref(), "strong identity key", 4096)?;
    optional_nonblank(activity.provenance_source_reference.as_deref(), "provenance source reference", 4096)?;
    optional_nonblank(activity.provenance_fingerprint.as_deref(), "provenance fingerprint", 4096)?;
    optional_nonblank(activity.provenance_label.as_deref(), "provenance label", 2048)?;
    sha256(&activity.source_file_sha256, "source file SHA-256")?;
    sha256(&activity.raw_record_sha256, "raw record SHA-256")?;
    iso_date(&activity.posted_date, "posted date")?;
    if let Some(value) = activity.value_date.as_deref() {
        iso_date(value, "value date")?;
    }
    if activity.signed_amount_minor == 0 {
        return Err(validation("bank activity amount must be non-zero whole pence"));
    }
    if activity.currency_code != "GBP" {
        return Err(validation("bank activity currency must be GBP"));
    }
    if !matches!(activity.source_format.as_str(), "csv" | "ofx" | "qfx") {
        return Err(validation("bank activity source format must be csv, ofx or qfx"));
    }
    if activity.provenance_kind != activity.source_format {
        return Err(validation("bank activity provenance kind must match the canonical source format"));
    }
    Ok(())
}

fn signed_bank_amount(direction: &str, amount_minor: i64) -> FoundationResult<i64> {
    if amount_minor <= 0 {
        return Err(validation("bank entry amount must be positive in persisted books"));
    }
    match direction {
        "debit" => Ok(amount_minor),
        "credit" => amount_minor
            .checked_neg()
            .ok_or_else(|| validation("bank entry amount overflow")),
        _ => Err(validation("bank entry direction is invalid")),
    }
}

mod activity;
mod match_reconcile;
#[cfg(test)]
mod tests;
