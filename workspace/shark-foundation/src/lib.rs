//! SBC-1B production Shark accounting/application facade.
//! All application callers use Shark-owned DTOs and errors rather than raw SQLite/Beankeeper APIs.

use std::fmt;
use std::path::{Path, PathBuf};

use beankeeper::core::JournalEntry;
use beankeeper::types::{Currency, Entry, Money};
use beankeeper_cli::cli::{
    Cli, Command, FormatOptions, OnConflictArg, OutputFormat, PassphraseOptions, VerbosityOptions,
};
use beankeeper_cli::commands::import_ofx::run_import_ofx;
use beankeeper_cli::db::{
    self, Actor, ConflictStrategy, Db, EntryStatus, ListAuditParams, ListTransactionParams,
    PostEntryParams, PostResult, PostTransactionParams, StoreAttachmentParams,
};
use beankeeper_cli::error::CliError;
use chrono::NaiveDate;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

pub const SHARK_FACADE_API_VERSION: u32 = 1;
pub const SHARK_APPLICATION_SCHEMA_VERSION: u32 = 1;
pub const SHARK_BOOKS_FORMAT_VERSION: u32 = 1;
pub const BEANKEEPER_DATABASE_SCHEMA_VERSION: i64 = 8;

pub type FoundationResult<T> = Result<T, FoundationError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum FoundationErrorCode {
    InvalidInput,
    Validation,
    NotFound,
    Storage,
    Io,
    Unsupported,
    Internal,
}

impl FoundationErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::Validation => "VALIDATION",
            Self::NotFound => "NOT_FOUND",
            Self::Storage => "STORAGE",
            Self::Io => "IO",
            Self::Unsupported => "UNSUPPORTED",
            Self::Internal => "INTERNAL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FoundationError {
    pub code: FoundationErrorCode,
    pub message: String,
}

impl FoundationError {
    #[must_use]
    pub fn new(code: FoundationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for FoundationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for FoundationError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct BooksId(String);

impl BooksId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BooksMetadata {
    pub books_id: BooksId,
    pub company_slug: String,
    pub company_name: String,
    pub database_schema_version: i64,
    pub books_format_version: u32,
    pub application_schema_version: u32,
    pub facade_api_version: u32,
    pub crate_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationMetadata {
    pub database_schema_version: i64,
    pub expected_database_schema_version: i64,
    pub books_format_version: u32,
    pub application_schema_version: u32,
    pub migration_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PostTransactionRequest {
    pub description: String,
    pub date: String,
    pub currency_code: String,
    pub reference: Option<String>,
    pub metadata: Option<String>,
    pub lines: Vec<PostingLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportSummary {
    pub before_count: i64,
    pub after_count: i64,
    pub imported_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusChangeResult {
    pub prior_status: String,
    pub new_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PostOutcome {
    Created(i64),
    Skipped(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PostingLine {
    pub account_code: String,
    pub direction: Direction,
    pub amount_minor: i64,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryView {
    pub id: i64,
    pub account_code: String,
    pub direction: String,
    pub amount_minor: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionView {
    pub id: i64,
    pub description: String,
    pub reference: Option<String>,
    pub currency: String,
    pub date: String,
    pub entries: Vec<EntryView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BalanceLine {
    pub code: String,
    pub account_type: String,
    pub debit_total: i64,
    pub credit_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrialBalance {
    pub accounts: Vec<BalanceLine>,
    pub total_debits: i64,
    pub total_credits: i64,
    pub balanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentView {
    pub id: i64,
    pub uri: String,
    pub hash: Option<String>,
    pub original_filename: Option<String>,
    pub document_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditView {
    pub actor: String,
    pub entity: String,
    pub entity_id: String,
    pub action: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

pub struct Books {
    db: Db,
    db_path: PathBuf,
    company_slug: String,
    actor: Actor,
}

impl Books {
    pub fn create_plain(
        path: &Path,
        company_slug: &str,
        company_name: &str,
        actor: &str,
    ) -> FoundationResult<Self> {
        let db = Db::open(path, None).map_err(map_cli_error)?;
        db::create_company(db.conn(), company_slug, company_name, None).map_err(map_cli_error)?;
        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: company_slug.to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn open_plain(path: &Path, company_slug: &str, actor: &str) -> FoundationResult<Self> {
        let db = Db::open(path, None).map_err(map_cli_error)?;
        db::get_company(db.conn(), company_slug).map_err(map_cli_error)?;
        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: company_slug.to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn create_encrypted(
        path: &Path,
        passphrase: &str,
        company_slug: &str,
        company_name: &str,
        actor: &str,
    ) -> FoundationResult<Self> {
        let secret = SecretString::from(passphrase.to_owned());
        let db = Db::open(path, Some(&secret)).map_err(map_cli_error)?;
        db::create_company(db.conn(), company_slug, company_name, None).map_err(map_cli_error)?;
        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: company_slug.to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn open_encrypted(
        path: &Path,
        passphrase: &str,
        company_slug: &str,
        actor: &str,
    ) -> FoundationResult<Self> {
        let secret = SecretString::from(passphrase.to_owned());
        let db = Db::open(path, Some(&secret)).map_err(map_cli_error)?;
        db::get_company(db.conn(), company_slug).map_err(map_cli_error)?;
        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: company_slug.to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn verify(&self) -> FoundationResult<i64> {
        db::get_schema_version(self.db.conn()).map_err(map_cli_error)
    }

    pub fn books_id(&self) -> BooksId {
        BooksId(self.company_slug.clone())
    }

    pub fn metadata(&self) -> FoundationResult<BooksMetadata> {
        let company = db::get_company(self.db.conn(), &self.company_slug).map_err(map_cli_error)?;
        let database_schema_version =
            db::get_schema_version(self.db.conn()).map_err(map_cli_error)?;
        Ok(BooksMetadata {
            books_id: self.books_id(),
            company_slug: company.slug,
            company_name: company.name,
            database_schema_version,
            books_format_version: SHARK_BOOKS_FORMAT_VERSION,
            application_schema_version: SHARK_APPLICATION_SCHEMA_VERSION,
            facade_api_version: SHARK_FACADE_API_VERSION,
            crate_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    pub fn migration_metadata(&self) -> FoundationResult<MigrationMetadata> {
        let database_schema_version = self.verify()?;
        Ok(MigrationMetadata {
            database_schema_version,
            expected_database_schema_version: BEANKEEPER_DATABASE_SCHEMA_VERSION,
            books_format_version: SHARK_BOOKS_FORMAT_VERSION,
            application_schema_version: SHARK_APPLICATION_SCHEMA_VERSION,
            migration_required: database_schema_version != BEANKEEPER_DATABASE_SCHEMA_VERSION,
        })
    }

    pub fn post(&self, request: &PostTransactionRequest) -> FoundationResult<PostOutcome> {
        self.post_transaction(
            &request.description,
            &request.date,
            &request.currency_code,
            request.reference.as_deref(),
            request.metadata.as_deref(),
            &request.lines,
        )
    }

    pub fn create_account(
        &self,
        code: &str,
        name: &str,
        account_type: &str,
    ) -> FoundationResult<()> {
        db::create_account(
            self.db.conn(),
            &self.company_slug,
            code,
            name,
            account_type,
            None,
        )
        .map(|_| ())
        .map_err(map_cli_error)
    }

    /// Validate through the Beankeeper accounting core first, then persist.
    pub fn post_transaction(
        &self,
        description: &str,
        date: &str,
        currency_code: &str,
        reference: Option<&str>,
        metadata: Option<&str>,
        lines: &[PostingLine],
    ) -> FoundationResult<PostOutcome> {
        let currency = Currency::from_code(currency_code).map_err(invalid_input_error)?;
        let txn_date = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(invalid_input_error)?;

        let mut journal = JournalEntry::new(txn_date, description);
        let mut db_entries = Vec::with_capacity(lines.len());

        for line in lines {
            if line.amount_minor <= 0 {
                return Err(FoundationError::new(
                    FoundationErrorCode::Validation,
                    "posting amount must be positive",
                ));
            }
            let row = db::get_account(self.db.conn(), &self.company_slug, &line.account_code)
                .map_err(map_cli_error)?;
            let account = db::row_to_account(&row).map_err(map_cli_error)?;
            let money = Money::from_minor(i128::from(line.amount_minor), currency);
            let entry = match line.direction {
                Direction::Debit => Entry::debit(account, money),
                Direction::Credit => Entry::credit(account, money),
            }
            .map_err(validation_error)?;
            journal = journal.entry(entry);

            db_entries.push(PostEntryParams {
                account_code: line.account_code.clone(),
                direction: match line.direction {
                    Direction::Debit => "debit".to_string(),
                    Direction::Credit => "credit".to_string(),
                },
                amount: line.amount_minor,
                memo: line.memo.clone(),
                tax_category: None,
            });
        }

        // Critical safety boundary: this enforces balanced double entry.
        let _validated = journal.post().map_err(validation_error)?;

        let params = PostTransactionParams {
            company_slug: &self.company_slug,
            description,
            metadata,
            currency: currency_code,
            date,
            entries: &db_entries,
            correlate: None,
            reference,
            on_conflict: ConflictStrategy::Error,
            actor: &self.actor,
        };
        match db::post_transaction(self.db.conn(), &params).map_err(map_cli_error)? {
            PostResult::Created(id) => Ok(PostOutcome::Created(id)),
            PostResult::Skipped(id) => Ok(PostOutcome::Skipped(id)),
        }
    }

    pub fn count_transactions(&self) -> FoundationResult<i64> {
        let params = ListTransactionParams::for_company(&self.company_slug);
        db::count_transactions(self.db.conn(), &params).map_err(map_cli_error)
    }

    pub fn find_by_reference(&self, reference: &str) -> FoundationResult<Vec<TransactionView>> {
        let mut params = ListTransactionParams::for_company(&self.company_slug);
        params.reference_filter = Some(reference);
        let rows = db::list_transactions(self.db.conn(), &params).map_err(map_cli_error)?;
        rows.into_iter()
            .map(|txn| {
                let entries = db::get_entries_for_transaction(self.db.conn(), txn.id).map_err(map_cli_error)?;
                Ok(TransactionView {
                    id: txn.id,
                    description: txn.description,
                    reference: txn.reference,
                    currency: txn.currency,
                    date: txn.date,
                    entries: entries
                        .into_iter()
                        .map(|e| EntryView {
                            id: e.id,
                            account_code: e.account_code,
                            direction: e.direction,
                            amount_minor: e.amount,
                            status: e.status,
                        })
                        .collect(),
                })
            })
            .collect()
    }

    pub fn transaction(&self, transaction_id: i64) -> FoundationResult<TransactionView> {
        let (txn, entries) =
            db::get_transaction(self.db.conn(), &self.company_slug, transaction_id).map_err(map_cli_error)?;
        Ok(TransactionView {
            id: txn.id,
            description: txn.description,
            reference: txn.reference,
            currency: txn.currency,
            date: txn.date,
            entries: entries
                .into_iter()
                .map(|e| EntryView {
                    id: e.id,
                    account_code: e.account_code,
                    direction: e.direction,
                    amount_minor: e.amount,
                    status: e.status,
                })
                .collect(),
        })
    }

    pub fn trial_balance(&self) -> FoundationResult<TrialBalance> {
        let rows =
            db::compute_trial_balance(self.db.conn(), &self.company_slug, None, None, None)
                .map_err(map_cli_error)?;
        let total_debits = rows.iter().map(|r| r.debit_total).sum();
        let total_credits = rows.iter().map(|r| r.credit_total).sum();
        Ok(TrialBalance {
            accounts: rows
                .into_iter()
                .map(|r| BalanceLine {
                    code: r.code,
                    account_type: r.account_type,
                    debit_total: r.debit_total,
                    credit_total: r.credit_total,
                })
                .collect(),
            total_debits,
            total_credits,
            balanced: total_debits == total_credits,
        })
    }

    /// Reuse pinned Beankeeper's public OFX orchestrator through the facade.
    /// Returns (count_before, count_after).
    pub fn import_ofx(
        &self,
        file: &Path,
        bank_account: &str,
        suspense_account: &str,
    ) -> FoundationResult<(i64, i64)> {
        let before = self.count_transactions()?;
        let cli = Cli {
            db: self.db_path.clone(),
            company: Some(self.company_slug.clone()),
            actor: Some(self.actor.name().to_string()),
            output: FormatOptions {
                format: Some(OutputFormat::Json),
                json: false,
            },
            verbosity: VerbosityOptions {
                verbose: false,
                quiet: true,
                no_color: true,
            },
            passphrase: PassphraseOptions {
                passphrase_fd: None,
                passphrase_file: None,
            },
            command: Command::Verify,
        };
        let file_str = file
            .to_str()
            .ok_or_else(|| {
                FoundationError::new(
                    FoundationErrorCode::InvalidInput,
                    "OFX fixture path is not valid UTF-8",
                )
            })?;
        run_import_ofx(
            &cli,
            &self.db,
            &self.company_slug,
            Some(file_str),
            bank_account,
            suspense_account,
            false,
            OnConflictArg::Skip,
        )
        .map_err(map_cli_error)?;
        let after = self.count_transactions()?;
        Ok((before, after))
    }

    pub fn import_ofx_summary(
        &self,
        file: &Path,
        bank_account: &str,
        suspense_account: &str,
    ) -> FoundationResult<ImportSummary> {
        let (before_count, after_count) =
            self.import_ofx(file, bank_account, suspense_account)?;
        Ok(ImportSummary {
            before_count,
            after_count,
            imported_count: after_count - before_count,
        })
    }

    pub fn set_reconciled(
        &self,
        transaction_id: i64,
        entry_id: i64,
    ) -> FoundationResult<String> {
        let prior = db::set_entry_status(
            self.db.conn(),
            &self.actor,
            &self.company_slug,
            transaction_id,
            entry_id,
            EntryStatus::Reconciled,
        )
        .map_err(map_cli_error)?;
        Ok(prior.as_str().to_string())
    }

    pub fn reconcile_entry(
        &self,
        transaction_id: i64,
        entry_id: i64,
    ) -> FoundationResult<StatusChangeResult> {
        let prior_status = self.set_reconciled(transaction_id, entry_id)?;
        Ok(StatusChangeResult {
            prior_status,
            new_status: "reconciled".to_string(),
        })
    }

    pub fn audit_status_changes(&self) -> FoundationResult<Vec<AuditView>> {
        let params = ListAuditParams {
            company_slug: Some(&self.company_slug),
            entity: Some("entry"),
            action: Some("status_change"),
            limit: 50,
        };
        let rows = db::list_audit(self.db.conn(), &params).map_err(map_cli_error)?;
        Ok(rows
            .into_iter()
            .map(|r| AuditView {
                actor: r.actor,
                entity: r.entity,
                entity_id: r.entity_id,
                action: r.action,
                before: r.before,
                after: r.after,
            })
            .collect())
    }

    pub fn attach_document(
        &self,
        transaction_id: i64,
        source: &Path,
        document_type: &str,
    ) -> FoundationResult<AttachmentView> {
        let (hash, stored_path) =
            db::hash_and_store_file(source, &self.db_path).map_err(map_cli_error)?;
        let uri = stored_path
            .file_name()
            .map(|n| format!("attachments/{}", n.to_string_lossy()))
            .unwrap_or_else(|| stored_path.to_string_lossy().to_string());
        let original_filename = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string());

        let params = StoreAttachmentParams {
            transaction_id,
            entry_id: None,
            company_slug: &self.company_slug,
            uri: &uri,
            document_type,
            hash: Some(&hash),
            original_filename: original_filename.as_deref(),
        };
        let id = db::store_attachment(self.db.conn(), &params).map_err(map_cli_error)?;
        let row = db::get_attachment(self.db.conn(), &self.company_slug, id).map_err(map_cli_error)?;
        Ok(AttachmentView {
            id: row.id,
            uri: row.uri,
            hash: row.hash,
            original_filename: row.original_filename,
            document_type: row.document_type,
        })
    }

    pub fn attachments(&self, transaction_id: i64) -> FoundationResult<Vec<AttachmentView>> {
        let rows =
            db::list_attachments(self.db.conn(), &self.company_slug, transaction_id).map_err(map_cli_error)?;
        Ok(rows
            .into_iter()
            .map(|row| AttachmentView {
                id: row.id,
                uri: row.uri,
                hash: row.hash,
                original_filename: row.original_filename,
                document_type: row.document_type,
            })
            .collect())
    }
}

pub fn verify_books(path: &Path) -> FoundationResult<i64> {
    let db = Db::open(path, None).map_err(map_cli_error)?;
    db::get_schema_version(db.conn()).map_err(map_cli_error)
}

pub fn boundary_id() -> &'static str {
    "SBC1B-production-shark-facade-v1"
}

fn map_cli_error(error: CliError) -> FoundationError {
    match error {
        CliError::Usage(message) => {
            FoundationError::new(FoundationErrorCode::InvalidInput, message)
        }
        CliError::Validation(message) => {
            FoundationError::new(FoundationErrorCode::Validation, message)
        }
        CliError::Database(message) => {
            FoundationError::new(FoundationErrorCode::Storage, message)
        }
        CliError::NotFound(message) => {
            FoundationError::new(FoundationErrorCode::NotFound, message)
        }
        CliError::General(message) => {
            FoundationError::new(FoundationErrorCode::Internal, message)
        }
        CliError::Unimplemented(message) => {
            FoundationError::new(FoundationErrorCode::Unsupported, message)
        }
        CliError::Bean(error) => {
            FoundationError::new(FoundationErrorCode::Validation, error.to_string())
        }
        CliError::Sqlite(error) => {
            FoundationError::new(FoundationErrorCode::Storage, error.to_string())
        }
        CliError::Io(error) => FoundationError::new(FoundationErrorCode::Io, error.to_string()),
    }
}

fn invalid_input_error<E: fmt::Display>(error: E) -> FoundationError {
    FoundationError::new(FoundationErrorCode::InvalidInput, error.to_string())
}

fn validation_error<E: fmt::Display>(error: E) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Validation, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "shark-sbc1b-{label}-{}-{nonce}.db",
            std::process::id()
        ))
    }

    #[test]
    fn cli_error_mapping_is_stable_and_shark_owned() {
        let cases = [
            (
                CliError::Usage("bad input".to_string()),
                FoundationErrorCode::InvalidInput,
            ),
            (
                CliError::Validation("unbalanced".to_string()),
                FoundationErrorCode::Validation,
            ),
            (
                CliError::NotFound("missing".to_string()),
                FoundationErrorCode::NotFound,
            ),
            (
                CliError::Database("db".to_string()),
                FoundationErrorCode::Storage,
            ),
            (
                CliError::Unimplemented("later".to_string()),
                FoundationErrorCode::Unsupported,
            ),
            (
                CliError::General("general".to_string()),
                FoundationErrorCode::Internal,
            ),
        ];

        for (source, expected) in cases {
            let mapped = map_cli_error(source);
            assert_eq!(mapped.code, expected);
            assert!(!mapped.message.is_empty());
        }
    }

    #[test]
    fn metadata_is_shark_owned_and_versioned() {
        let path = temp_db_path("metadata");
        let books =
            Books::create_plain(&path, "test-books", "Test Books", "test").expect("create books");
        let metadata = books.metadata().expect("metadata");
        assert_eq!(metadata.books_id.as_str(), "test-books");
        assert_eq!(metadata.company_slug, "test-books");
        assert_eq!(metadata.company_name, "Test Books");
        assert!(metadata.database_schema_version > 0);
        assert_eq!(metadata.books_format_version, SHARK_BOOKS_FORMAT_VERSION);
        assert_eq!(
            metadata.application_schema_version,
            SHARK_APPLICATION_SCHEMA_VERSION
        );
        assert_eq!(metadata.facade_api_version, SHARK_FACADE_API_VERSION);
        let migration = books.migration_metadata().expect("migration metadata");
        assert_eq!(
            migration.database_schema_version,
            BEANKEEPER_DATABASE_SCHEMA_VERSION
        );
        assert_eq!(
            migration.expected_database_schema_version,
            BEANKEEPER_DATABASE_SCHEMA_VERSION
        );
        assert!(!migration.migration_required);
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn unbalanced_post_is_rejected_before_database_mutation() {
        let path = temp_db_path("unbalanced");
        let books =
            Books::create_plain(&path, "test-books", "Test Books", "test").expect("create books");
        books
            .create_account("1000", "Cash", "asset")
            .expect("create cash account");

        let before = books.count_transactions().expect("count before");
        let request = PostTransactionRequest {
            description: "Unbalanced test".to_string(),
            date: "2026-09-04".to_string(),
            currency_code: "GBP".to_string(),
            reference: Some("SBC1B-UNBALANCED".to_string()),
            metadata: None,
            lines: vec![PostingLine {
                account_code: "1000".to_string(),
                direction: Direction::Debit,
                amount_minor: 100,
                memo: None,
            }],
        };

        let error = books.post(&request).expect_err("unbalanced post must fail");
        assert_eq!(error.code, FoundationErrorCode::Validation);

        let after = books.count_transactions().expect("count after");
        assert_eq!(before, after, "rejected post must not mutate transaction count");
        drop(books);
        let _ = fs::remove_file(path);
    }
}
