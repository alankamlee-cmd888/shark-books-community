//! SBC-0C Shark foundation facade spike.
//! All application callers use this facade rather than raw SQLite/Beankeeper DB APIs.

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
use chrono::NaiveDate;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

pub type FoundationResult<T> = Result<T, String>;

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
        let db = Db::open(path, None).map_err(err)?;
        db::create_company(db.conn(), company_slug, company_name, None).map_err(err)?;
        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: company_slug.to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn open_plain(path: &Path, company_slug: &str, actor: &str) -> FoundationResult<Self> {
        let db = Db::open(path, None).map_err(err)?;
        db::get_company(db.conn(), company_slug).map_err(err)?;
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
        let db = Db::open(path, Some(&secret)).map_err(err)?;
        db::create_company(db.conn(), company_slug, company_name, None).map_err(err)?;
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
        let db = Db::open(path, Some(&secret)).map_err(err)?;
        db::get_company(db.conn(), company_slug).map_err(err)?;
        Ok(Self {
            db,
            db_path: path.to_path_buf(),
            company_slug: company_slug.to_string(),
            actor: Actor::new(actor),
        })
    }

    pub fn verify(&self) -> FoundationResult<i64> {
        db::get_schema_version(self.db.conn()).map_err(err)
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
        .map_err(err)
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
        let currency = Currency::from_code(currency_code).map_err(err)?;
        let txn_date = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(err)?;

        let mut journal = JournalEntry::new(txn_date, description);
        let mut db_entries = Vec::with_capacity(lines.len());

        for line in lines {
            if line.amount_minor <= 0 {
                return Err("posting amount must be positive".to_string());
            }
            let row = db::get_account(self.db.conn(), &self.company_slug, &line.account_code)
                .map_err(err)?;
            let account = db::row_to_account(&row).map_err(err)?;
            let money = Money::from_minor(i128::from(line.amount_minor), currency);
            let entry = match line.direction {
                Direction::Debit => Entry::debit(account, money),
                Direction::Credit => Entry::credit(account, money),
            }
            .map_err(err)?;
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
        let _validated = journal.post().map_err(err)?;

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
        match db::post_transaction(self.db.conn(), &params).map_err(err)? {
            PostResult::Created(id) => Ok(PostOutcome::Created(id)),
            PostResult::Skipped(id) => Ok(PostOutcome::Skipped(id)),
        }
    }

    pub fn count_transactions(&self) -> FoundationResult<i64> {
        let params = ListTransactionParams::for_company(&self.company_slug);
        db::count_transactions(self.db.conn(), &params).map_err(err)
    }

    pub fn find_by_reference(&self, reference: &str) -> FoundationResult<Vec<TransactionView>> {
        let mut params = ListTransactionParams::for_company(&self.company_slug);
        params.reference_filter = Some(reference);
        let rows = db::list_transactions(self.db.conn(), &params).map_err(err)?;
        rows.into_iter()
            .map(|txn| {
                let entries = db::get_entries_for_transaction(self.db.conn(), txn.id).map_err(err)?;
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
            db::get_transaction(self.db.conn(), &self.company_slug, transaction_id).map_err(err)?;
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
                .map_err(err)?;
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
            .ok_or_else(|| "OFX fixture path is not valid UTF-8".to_string())?;
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
        .map_err(err)?;
        let after = self.count_transactions()?;
        Ok((before, after))
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
        .map_err(err)?;
        Ok(prior.as_str().to_string())
    }

    pub fn audit_status_changes(&self) -> FoundationResult<Vec<AuditView>> {
        let params = ListAuditParams {
            company_slug: Some(&self.company_slug),
            entity: Some("entry"),
            action: Some("status_change"),
            limit: 50,
        };
        let rows = db::list_audit(self.db.conn(), &params).map_err(err)?;
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
            db::hash_and_store_file(source, &self.db_path).map_err(err)?;
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
        let id = db::store_attachment(self.db.conn(), &params).map_err(err)?;
        let row = db::get_attachment(self.db.conn(), &self.company_slug, id).map_err(err)?;
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
            db::list_attachments(self.db.conn(), &self.company_slug, transaction_id).map_err(err)?;
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
    let db = Db::open(path, None).map_err(err)?;
    db::get_schema_version(db.conn()).map_err(err)
}

pub fn boundary_id() -> &'static str {
    "SBC0C-B1-direct-rust-facade"
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
