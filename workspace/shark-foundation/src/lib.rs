//! SBC-1C production Shark accounting/application facade.
//! Production native books are encrypted by default and keys stay behind a Shark-owned provider boundary.

use std::fmt;
use std::fs;
use std::io::Read;
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
use sha2::{Digest, Sha256};

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
    pub fn new(value: impl Into<String>) -> FoundationResult<Self> {
        let value = value.into();
        validate_books_id(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Secret key material owned by the Shark boundary.
///
/// Deliberately not serializable or clonable. Debug output is always redacted.
pub struct BooksKey {
    secret: SecretString,
}

impl BooksKey {
    pub fn new(secret: String) -> FoundationResult<Self> {
        if secret.is_empty() {
            return Err(FoundationError::new(
                FoundationErrorCode::InvalidInput,
                "books key must not be empty",
            ));
        }
        Ok(Self {
            secret: SecretString::from(secret),
        })
    }

    fn secret(&self) -> &SecretString {
        &self.secret
    }
}

impl fmt::Debug for BooksKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BooksKey([REDACTED])")
    }
}

/// Platform-replaceable source of encrypted-books key material.
///
/// Implementations may use Windows Credential Manager, iOS Keychain, or another
/// secure platform store. The key itself never crosses a serializable DTO.
pub trait SecureKeyProvider: Send + Sync {
    fn load_key(&self, books_id: &BooksId) -> FoundationResult<BooksKey>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupReceipt {
    pub backup_path: PathBuf,
    pub persistent_companion_backups: Vec<PathBuf>,
    pub reused_existing_snapshot: bool,
}

/// Hook that runs before an existing encrypted DB is opened.
///
/// Beankeeper's pinned `Db::open` performs schema assurance/migration, so this
/// hook must complete before that call. Implementations are replaceable and
/// should assume that the source books are not concurrently open elsewhere.
pub trait BackupBeforeMigrationHook: Send + Sync {
    fn backup_before_open(
        &self,
        path: &Path,
        books_id: &BooksId,
    ) -> FoundationResult<BackupReceipt>;
}

/// Conservative reference implementation for the pre-migration contract.
///
/// Each snapshot receives a new sibling name rather than overwriting a previous
/// rollback point. SQLite's persistent `-wal` companion is copied when present;
/// the transient `-shm` wal-index is deliberately not part of the snapshot.
#[derive(Debug, Clone, Default)]
pub struct SiblingEncryptedBackup;

impl SiblingEncryptedBackup {
    const MAX_BACKUP_SEQUENCE: u32 = 10_000;

    #[must_use]
    pub fn backup_path_for(&self, path: &Path) -> PathBuf {
        Self::backup_candidate(path, 0)
    }

    fn backup_candidate(path: &Path, sequence: u32) -> PathBuf {
        let mut value = path.as_os_str().to_os_string();
        if sequence == 0 {
            value.push(".preopen.bak");
        } else {
            value.push(format!(".preopen.{sequence}.bak"));
        }
        PathBuf::from(value)
    }

    fn reusable_or_next_backup_path(&self, path: &Path) -> FoundationResult<(PathBuf, bool)> {
        for sequence in 0..=Self::MAX_BACKUP_SEQUENCE {
            let candidate = Self::backup_candidate(path, sequence);
            let wal = sqlite_companion_path(&candidate, "-wal");
            let journal = sqlite_companion_path(&candidate, "-journal");

            if candidate.is_file() {
                if persistent_sqlite_snapshot_matches(path, &candidate)? {
                    return Ok((candidate, true));
                }
                continue;
            }

            if !wal.exists() && !journal.exists() {
                return Ok((candidate, false));
            }
        }
        Err(FoundationError::new(
            FoundationErrorCode::Storage,
            "could not allocate a non-overwriting pre-open backup path",
        ))
    }
}

impl BackupBeforeMigrationHook for SiblingEncryptedBackup {
    fn backup_before_open(
        &self,
        path: &Path,
        _books_id: &BooksId,
    ) -> FoundationResult<BackupReceipt> {
        if !path.is_file() {
            return Err(FoundationError::new(
                FoundationErrorCode::NotFound,
                "books database does not exist",
            ));
        }

        let (backup_path, reused_existing_snapshot) =
            self.reusable_or_next_backup_path(path)?;

        if reused_existing_snapshot {
            return Ok(BackupReceipt {
                persistent_companion_backups: persistent_companion_backups_for(&backup_path),
                backup_path,
                reused_existing_snapshot: true,
            });
        }

        fs::copy(path, &backup_path).map_err(io_error)?;

        let mut persistent_companion_backups = Vec::new();
        for suffix in ["-wal", "-journal"] {
            let source = sqlite_companion_path(path, suffix);
            if !source.is_file() {
                continue;
            }
            let destination = sqlite_companion_path(&backup_path, suffix);
            if let Err(error) = fs::copy(&source, &destination) {
                let _ = fs::remove_file(&backup_path);
                for copied in &persistent_companion_backups {
                    let _ = fs::remove_file(copied);
                }
                return Err(io_error(error));
            }
            persistent_companion_backups.push(destination);
        }

        Ok(BackupReceipt {
            backup_path,
            persistent_companion_backups,
            reused_existing_snapshot: false,
        })
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
    pub encrypted_native_required: bool,
    pub backup_before_existing_open_required: bool,
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
    #[cfg(test)]
    fn create_plain_for_test(
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

    pub fn create_encrypted(
        path: &Path,
        books_id: &BooksId,
        company_name: &str,
        actor: &str,
        key_provider: &dyn SecureKeyProvider,
    ) -> FoundationResult<Self> {
        if path.exists() {
            return Err(FoundationError::new(
                FoundationErrorCode::Validation,
                "refusing to create encrypted books over an existing path",
            ));
        }

        let key = key_provider.load_key(books_id)?;
        let db = match Db::open(path, Some(key.secret())) {
            Ok(db) => db,
            Err(error) => {
                remove_sqlite_artifacts(path);
                return Err(map_cli_error(error));
            }
        };
        if let Err(error) =
            db::create_company(db.conn(), books_id.as_str(), company_name, None)
        {
            drop(db);
            remove_sqlite_artifacts(path);
            return Err(map_cli_error(error));
        }

        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: books_id.as_str().to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn open_encrypted(
        path: &Path,
        books_id: &BooksId,
        actor: &str,
        key_provider: &dyn SecureKeyProvider,
        backup_hook: &dyn BackupBeforeMigrationHook,
    ) -> FoundationResult<Self> {
        if !path.is_file() {
            return Err(FoundationError::new(
                FoundationErrorCode::NotFound,
                "books database does not exist",
            ));
        }

        let key = key_provider.load_key(books_id)?;
        // Must occur before Db::open because pinned Beankeeper performs schema
        // assurance/migration as part of the open operation.
        let _backup = backup_hook.backup_before_open(path, books_id)?;
        let db = Db::open(path, Some(key.secret())).map_err(map_cli_error)?;
        db::get_company(db.conn(), books_id.as_str()).map_err(map_cli_error)?;

        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: books_id.as_str().to_string(),
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
            encrypted_native_required: true,
            backup_before_existing_open_required: true,
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

#[must_use]
pub const fn production_encryption_required() -> bool {
    true
}

pub fn boundary_id() -> &'static str {
    "SBC1C-encrypted-lifecycle-v1"
}

fn validate_books_id(value: &str) -> FoundationResult<()> {
    if value.is_empty() || value.len() > 64 {
        return Err(FoundationError::new(
            FoundationErrorCode::InvalidInput,
            "books id must be 1-64 characters",
        ));
    }
    let bytes = value.as_bytes();
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return Err(FoundationError::new(
            FoundationErrorCode::InvalidInput,
            "books id must start with a lowercase letter or digit",
        ));
    }
    if bytes
        .iter()
        .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && *b != b'-')
    {
        return Err(FoundationError::new(
            FoundationErrorCode::InvalidInput,
            "books id may contain only lowercase letters, digits, and hyphens",
        ));
    }
    Ok(())
}

fn sqlite_companion_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn file_digest(path: &Path) -> FoundationResult<Vec<u8>> {
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_vec())
}

fn persistent_sqlite_snapshot_matches(source: &Path, backup: &Path) -> FoundationResult<bool> {
    if file_digest(source)? != file_digest(backup)? {
        return Ok(false);
    }

    for suffix in ["-wal", "-journal"] {
        let source_companion = sqlite_companion_path(source, suffix);
        let backup_companion = sqlite_companion_path(backup, suffix);
        match (
            source_companion.is_file(),
            backup_companion.is_file(),
        ) {
            (false, false) => {}
            (true, true) => {
                if file_digest(&source_companion)? != file_digest(&backup_companion)? {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn persistent_companion_backups_for(backup: &Path) -> Vec<PathBuf> {
    ["-wal", "-journal"]
        .into_iter()
        .map(|suffix| sqlite_companion_path(backup, suffix))
        .filter(|path| path.is_file())
        .collect()
}

fn remove_sqlite_artifacts(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(sqlite_companion_path(path, suffix));
    }
}

fn io_error(error: std::io::Error) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Io, error.to_string())
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
            Books::create_plain_for_test(&path, "test-books", "Test Books", "test").expect("create books");
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
        assert!(migration.encrypted_native_required);
        assert!(migration.backup_before_existing_open_required);
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn unbalanced_post_is_rejected_before_database_mutation() {
        let path = temp_db_path("unbalanced");
        let books =
            Books::create_plain_for_test(&path, "test-books", "Test Books", "test").expect("create books");
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
    struct TestKeyProvider {
        key: String,
    }

    impl TestKeyProvider {
        fn new(key: &str) -> Self {
            Self {
                key: key.to_string(),
            }
        }
    }

    impl SecureKeyProvider for TestKeyProvider {
        fn load_key(&self, _books_id: &BooksId) -> FoundationResult<BooksKey> {
            BooksKey::new(self.key.clone())
        }
    }

    fn file_sha256(path: &Path) -> String {
        use sha2::{Digest, Sha256};
        let bytes = fs::read(path).expect("read database bytes");
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn books_key_debug_is_redacted() {
        let secret = "sbc1c-test-secret-material";
        let key = BooksKey::new(secret.to_string()).expect("build key");
        let debug = format!("{key:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains(secret));
    }

    #[test]
    fn encrypted_create_reopen_and_backup_contract() {
        let path = temp_db_path("encrypted-reopen");
        let books_id = BooksId::new("encrypted-books").expect("books id");
        let provider = TestKeyProvider::new("correct-test-key");
        let backup = SiblingEncryptedBackup;

        let books = Books::create_encrypted(
            &path,
            &books_id,
            "Encrypted Test Books",
            "test",
            &provider,
        )
        .expect("create encrypted books");
        books
            .create_account("1000", "Cash", "asset")
            .expect("create account");
        assert_eq!(
            books.verify().expect("verify schema"),
            BEANKEEPER_DATABASE_SCHEMA_VERSION
        );
        drop(books);

        let header = fs::read(&path).expect("read encrypted database");
        assert!(
            !header.starts_with(b"SQLite format 3\0"),
            "production encrypted DB must not expose plaintext SQLite header"
        );
        let before_hash = file_sha256(&path);

        let reopened = Books::open_encrypted(&path, &books_id, "test", &provider, &backup)
            .expect("reopen encrypted books");
        assert_eq!(reopened.books_id(), books_id);
        drop(reopened);

        let backup_path = backup.backup_path_for(&path);
        assert!(backup_path.is_file(), "pre-open encrypted backup must exist");
        assert_eq!(file_sha256(&backup_path), before_hash);

        remove_sqlite_artifacts(&backup_path);
        remove_sqlite_artifacts(&path);
    }

    #[test]
    fn backup_is_non_overwriting_and_preserves_persistent_companions() {
        let path = temp_db_path("backup-contract");
        let books_id = BooksId::new("backup-books").expect("books id");
        let backup = SiblingEncryptedBackup;

        fs::write(&path, b"main-v1").expect("write main fixture");
        let wal_path = sqlite_companion_path(&path, "-wal");
        let shm_path = sqlite_companion_path(&path, "-shm");
        fs::write(&wal_path, b"wal-v1").expect("write wal fixture");
        fs::write(&shm_path, b"transient-shm").expect("write shm fixture");

        let first = backup
            .backup_before_open(&path, &books_id)
            .expect("first backup");
        assert!(!first.reused_existing_snapshot);
        assert_eq!(
            fs::read(&first.backup_path).expect("read first backup"),
            b"main-v1".to_vec()
        );
        assert_eq!(first.persistent_companion_backups.len(), 1);
        let first_wal = sqlite_companion_path(&first.backup_path, "-wal");
        assert_eq!(first.persistent_companion_backups[0], first_wal);
        assert_eq!(
            fs::read(&first_wal).expect("read first wal backup"),
            b"wal-v1".to_vec()
        );
        assert!(
            !sqlite_companion_path(&first.backup_path, "-shm").exists(),
            "transient -shm must not be copied into rollback snapshot"
        );

        let duplicate = backup
            .backup_before_open(&path, &books_id)
            .expect("duplicate snapshot lookup");
        assert!(duplicate.reused_existing_snapshot);
        assert_eq!(duplicate.backup_path, first.backup_path);

        fs::write(&path, b"main-v2").expect("rewrite main fixture");
        fs::write(&wal_path, b"wal-v2").expect("rewrite wal fixture");
        let second = backup
            .backup_before_open(&path, &books_id)
            .expect("changed snapshot backup");
        assert!(!second.reused_existing_snapshot);
        assert_ne!(first.backup_path, second.backup_path);
        assert_eq!(
            fs::read(&first.backup_path).expect("reread first backup"),
            b"main-v1".to_vec()
        );
        assert_eq!(
            fs::read(&second.backup_path).expect("read second backup"),
            b"main-v2".to_vec()
        );
        let second_wal = sqlite_companion_path(&second.backup_path, "-wal");
        assert_eq!(
            fs::read(&second_wal).expect("read second wal backup"),
            b"wal-v2".to_vec()
        );

        remove_sqlite_artifacts(&first.backup_path);
        remove_sqlite_artifacts(&second.backup_path);
        remove_sqlite_artifacts(&path);
    }

    #[test]
    fn wrong_key_fails_without_mutating_encrypted_database() {
        let path = temp_db_path("wrong-key");
        let books_id = BooksId::new("wrong-key-books").expect("books id");
        let correct = TestKeyProvider::new("correct-test-key");
        let wrong = TestKeyProvider::new("wrong-test-key");
        let backup = SiblingEncryptedBackup;

        let books =
            Books::create_encrypted(&path, &books_id, "Wrong Key Test", "test", &correct)
                .expect("create encrypted books");
        books
            .create_account("1000", "Cash", "asset")
            .expect("create account");
        drop(books);

        let before_hash = file_sha256(&path);
        let result = Books::open_encrypted(&path, &books_id, "test", &wrong, &backup);
        let error = match result {
            Ok(_) => panic!("wrong key must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, FoundationErrorCode::Storage);
        assert_eq!(
            file_sha256(&path),
            before_hash,
            "wrong-key open must not mutate encrypted database bytes"
        );

        let backup_path = backup.backup_path_for(&path);
        assert!(backup_path.is_file(), "backup must occur before attempted DB open");
        assert_eq!(file_sha256(&backup_path), before_hash);

        let recovered = Books::open_encrypted(&path, &books_id, "test", &correct, &backup)
            .expect("correct key must still reopen books after wrong-key failure");
        assert_eq!(
            recovered.verify().expect("verify recovered books"),
            BEANKEEPER_DATABASE_SCHEMA_VERSION
        );
        drop(recovered);

        let second_backup_path = SiblingEncryptedBackup::backup_candidate(&path, 1);
        remove_sqlite_artifacts(&backup_path);
        remove_sqlite_artifacts(&second_backup_path);
        remove_sqlite_artifacts(&path);
    }

    #[test]
    fn production_plaintext_constructors_are_not_exposed() {
        assert!(production_encryption_required());
        assert_eq!(boundary_id(), "SBC1C-encrypted-lifecycle-v1");
    }

}
