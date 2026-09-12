//! SBC-7B1 Slice 3A Shark-owned document metadata persistence.
//!
//! Document bytes remain in user-controlled storage. This module persists only
//! canonical integrity/reference metadata and explicit owner attachment links in
//! the encrypted Shark application schema. It never calls Beankeeper's
//! hash-and-copy attachment primitive.

use super::*;
use crate::bank_application::{nonblank, optional_nonblank, sha256, sqlite_error, validation};

const MAX_DOCUMENT_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentWrite {
    pub document_id: String,
    pub storage_root_id: String,
    pub relative_path: String,
    pub original_filename: String,
    pub media_type: Option<String>,
    pub sha256: String,
    pub byte_len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentView {
    pub document_id: String,
    pub storage_root_id: String,
    pub relative_path: String,
    pub original_filename: String,
    pub media_type: Option<String>,
    pub sha256: String,
    pub byte_len: u64,
    pub registered_by: String,
    pub registered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocumentPersistOutcome {
    Registered(DocumentView),
    AlreadyRegistered(DocumentView),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentAttachmentWrite {
    pub document_id: String,
    pub record_kind: String,
    pub record_id: String,
    pub transaction_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentAttachmentView {
    pub document_id: String,
    pub record_kind: String,
    pub record_id: String,
    pub transaction_id: i64,
    pub attached_by: String,
    pub attached_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocumentAttachmentPersistOutcome {
    Attached(DocumentAttachmentView),
    AlreadyAttached(DocumentAttachmentView),
}

fn validate_relative_path(value: &str) -> FoundationResult<()> {
    nonblank(value, "document relative path", 1024)?;
    if value.starts_with('/')
        || value.starts_with('~')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
    {
        return Err(validation("document path must be a portable relative path"));
    }
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.trim() != segment {
            return Err(validation("document path contains an unsafe segment"));
        }
    }
    Ok(())
}

fn validate_filename(value: &str) -> FoundationResult<()> {
    nonblank(value, "original filename", 255)?;
    if value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
    {
        return Err(validation("original filename must be a portable basename"));
    }
    Ok(())
}

fn validate_document(write: &DocumentWrite) -> FoundationResult<()> {
    nonblank(&write.document_id, "document id", 128)?;
    nonblank(&write.storage_root_id, "storage root id", 128)?;
    validate_relative_path(&write.relative_path)?;
    validate_filename(&write.original_filename)?;
    optional_nonblank(write.media_type.as_deref(), "media type", 255)?;
    sha256(&write.sha256, "document SHA-256")?;
    if write.sha256.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(validation("document SHA-256 must be lowercase hexadecimal"));
    }
    if write.byte_len == 0 || write.byte_len > MAX_DOCUMENT_BYTES {
        return Err(validation("document byte length must be between 1 byte and 25 MiB"));
    }
    let expected_id = format!("doc-{}", write.sha256);
    if write.document_id != expected_id {
        return Err(validation("document id must be derived from canonical SHA-256"));
    }
    let expected_path = format!("documents/{}/{}", write.sha256, write.original_filename);
    if write.relative_path != expected_path {
        return Err(validation("document relative path does not match canonical content location"));
    }
    Ok(())
}

fn validate_attachment(write: &DocumentAttachmentWrite) -> FoundationResult<()> {
    nonblank(&write.document_id, "document id", 128)?;
    nonblank(&write.record_id, "record id", 128)?;
    if !matches!(write.record_kind.as_str(), "moneyIn" | "moneyOut") {
        return Err(validation("document attachment record kind must be moneyIn or moneyOut"));
    }
    if write.transaction_id <= 0 {
        return Err(validation("document attachment transaction id must be positive"));
    }
    Ok(())
}

impl Books {
    fn document_savepoint<T>(
        &self,
        name: &'static str,
        operation: impl FnOnce() -> FoundationResult<T>,
    ) -> FoundationResult<T> {
        self.db.conn().execute_batch(&format!("SAVEPOINT {name}")).map_err(sqlite_error)?;
        match operation() {
            Ok(value) => {
                self.db.conn().execute_batch(&format!("RELEASE {name}")).map_err(sqlite_error)?;
                Ok(value)
            }
            Err(error) => {
                let _ = self.db.conn().execute_batch(&format!("ROLLBACK TO {name}; RELEASE {name}"));
                Err(error)
            }
        }
    }

    pub fn register_document(&self, write: &DocumentWrite) -> FoundationResult<DocumentPersistOutcome> {
        validate_document(write)?;

        match self.document(&write.document_id) {
            Ok(existing) => {
                let exact = existing.storage_root_id == write.storage_root_id
                    && existing.relative_path == write.relative_path
                    && existing.original_filename == write.original_filename
                    && existing.media_type == write.media_type
                    && existing.sha256 == write.sha256
                    && existing.byte_len == write.byte_len;
                if exact {
                    return Ok(DocumentPersistOutcome::AlreadyRegistered(existing));
                }
                return Err(validation("document id already exists with conflicting immutable metadata"));
            }
            Err(error) if error.code == FoundationErrorCode::NotFound => {}
            Err(error) => return Err(error),
        }

        let mut statement = self.db.conn().prepare(
            "SELECT document_id FROM shark_document WHERE company_slug = ?1 AND storage_root_id = ?2 AND relative_path = ?3 LIMIT 1",
        ).map_err(sqlite_error)?;
        let mut rows = statement.query((&self.company_slug, &write.storage_root_id, &write.relative_path)).map_err(sqlite_error)?;
        if rows.next().map_err(sqlite_error)?.is_some() {
            return Err(validation("document storage reference already belongs to another document"));
        }
        drop(rows);
        drop(statement);

        self.document_savepoint("shark_document_register", || {
            let byte_len = i64::try_from(write.byte_len).map_err(|_| validation("document byte length cannot be persisted"))?;
            self.db.conn().execute(
                "INSERT INTO shark_document(company_slug, document_id, storage_root_id, relative_path, original_filename, media_type, sha256, byte_len, registered_by) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    &self.company_slug,
                    &write.document_id,
                    &write.storage_root_id,
                    &write.relative_path,
                    &write.original_filename,
                    write.media_type.as_deref(),
                    &write.sha256,
                    byte_len,
                    self.actor.name(),
                ),
            ).map_err(sqlite_error)?;
            Ok(())
        })?;
        Ok(DocumentPersistOutcome::Registered(self.document(&write.document_id)?))
    }

    pub fn document(&self, document_id: &str) -> FoundationResult<DocumentView> {
        nonblank(document_id, "document id", 128)?;
        let mut statement = self.db.conn().prepare(
            "SELECT document_id, storage_root_id, relative_path, original_filename, media_type, sha256, byte_len, registered_by, registered_at FROM shark_document WHERE company_slug = ?1 AND document_id = ?2",
        ).map_err(sqlite_error)?;
        let mut rows = statement.query((&self.company_slug, document_id)).map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Err(FoundationError::new(FoundationErrorCode::NotFound, "registered document was not found"));
        };
        let byte_len_raw: i64 = row.get(6).map_err(sqlite_error)?;
        let byte_len = u64::try_from(byte_len_raw).map_err(|_| {
            FoundationError::new(FoundationErrorCode::Storage, "persisted document byte length is invalid")
        })?;
        Ok(DocumentView {
            document_id: row.get(0).map_err(sqlite_error)?,
            storage_root_id: row.get(1).map_err(sqlite_error)?,
            relative_path: row.get(2).map_err(sqlite_error)?,
            original_filename: row.get(3).map_err(sqlite_error)?,
            media_type: row.get(4).map_err(sqlite_error)?,
            sha256: row.get(5).map_err(sqlite_error)?,
            byte_len,
            registered_by: row.get(7).map_err(sqlite_error)?,
            registered_at: row.get(8).map_err(sqlite_error)?,
        })
    }

    pub fn attach_registered_document(
        &self,
        write: &DocumentAttachmentWrite,
    ) -> FoundationResult<DocumentAttachmentPersistOutcome> {
        validate_attachment(write)?;
        let _ = self.document(&write.document_id)?;
        let transaction = self.transaction(write.transaction_id)?;
        let expected_reference = format!("sbc7b1:{}:{}", write.record_kind, write.record_id);
        if transaction.reference.as_deref() != Some(expected_reference.as_str()) {
            return Err(validation("document attachment target does not match the authoritative owner record"));
        }

        let existing = self.document_attachments(&write.document_id)?;
        if let Some(row) = existing.into_iter().find(|row| {
            row.record_kind == write.record_kind && row.record_id == write.record_id
        }) {
            if row.transaction_id == write.transaction_id {
                return Ok(DocumentAttachmentPersistOutcome::AlreadyAttached(row));
            }
            return Err(validation("document attachment target conflicts with persisted transaction identity"));
        }

        self.document_savepoint("shark_document_attach", || {
            self.db.conn().execute(
                "INSERT INTO shark_document_attachment(company_slug, document_id, record_kind, record_id, transaction_id, attached_by) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    &self.company_slug,
                    &write.document_id,
                    &write.record_kind,
                    &write.record_id,
                    write.transaction_id,
                    self.actor.name(),
                ),
            ).map_err(sqlite_error)?;
            Ok(())
        })?;

        let row = self.document_attachments(&write.document_id)?
            .into_iter()
            .find(|row| row.record_kind == write.record_kind && row.record_id == write.record_id)
            .ok_or_else(|| FoundationError::new(FoundationErrorCode::Storage, "persisted document attachment could not be read back"))?;
        Ok(DocumentAttachmentPersistOutcome::Attached(row))
    }

    pub fn document_attachments(&self, document_id: &str) -> FoundationResult<Vec<DocumentAttachmentView>> {
        nonblank(document_id, "document id", 128)?;
        let mut statement = self.db.conn().prepare(
            "SELECT document_id, record_kind, record_id, transaction_id, attached_by, attached_at FROM shark_document_attachment WHERE company_slug = ?1 AND document_id = ?2 ORDER BY attached_at, record_kind, record_id",
        ).map_err(sqlite_error)?;
        let mut rows = statement.query((&self.company_slug, document_id)).map_err(sqlite_error)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            out.push(DocumentAttachmentView {
                document_id: row.get(0).map_err(sqlite_error)?,
                record_kind: row.get(1).map_err(sqlite_error)?,
                record_id: row.get(2).map_err(sqlite_error)?,
                transaction_id: row.get(3).map_err(sqlite_error)?,
                attached_by: row.get(4).map_err(sqlite_error)?,
                attached_at: row.get(5).map_err(sqlite_error)?,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("shark-sbc7b1-doc-{label}-{}-{nonce}.db", std::process::id()))
    }

    fn document_write(hash_byte: &str) -> DocumentWrite {
        let sha256 = hash_byte.repeat(64);
        DocumentWrite {
            document_id: format!("doc-{sha256}"),
            storage_root_id: "root-1".to_string(),
            relative_path: format!("documents/{sha256}/receipt.png"),
            original_filename: "receipt.png".to_string(),
            media_type: Some("image/png".to_string()),
            sha256,
            byte_len: 128,
        }
    }

    #[test]
    fn document_registration_is_idempotent_and_conflicts_fail_closed() {
        let path = temp_db_path("register");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        let write = document_write("a");
        assert!(matches!(books.register_document(&write).unwrap(), DocumentPersistOutcome::Registered(_)));
        assert!(matches!(books.register_document(&write).unwrap(), DocumentPersistOutcome::AlreadyRegistered(_)));

        let mut conflict = write.clone();
        conflict.media_type = Some("image/jpeg".to_string());
        assert_eq!(books.register_document(&conflict).unwrap_err().code, FoundationErrorCode::Validation);
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn document_validation_rejects_noncanonical_paths_and_sizes() {
        let path = temp_db_path("validation");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        let mut write = document_write("b");
        write.relative_path = "../outside.png".to_string();
        assert!(books.register_document(&write).is_err());
        let mut oversized = document_write("c");
        oversized.byte_len = MAX_DOCUMENT_BYTES + 1;
        assert!(books.register_document(&oversized).is_err());
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn attachment_is_typed_idempotent_and_does_not_post() {
        let path = temp_db_path("attach");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        books.create_account("1000", "Business Bank", "asset").unwrap();
        books.create_account("4000", "Sales", "revenue").unwrap();
        let posted = books.post(&PostTransactionRequest {
            description: "Income".to_string(),
            date: "2026-09-12".to_string(),
            currency_code: "GBP".to_string(),
            reference: Some("sbc7b1:moneyIn:income-1".to_string()),
            metadata: None,
            lines: vec![
                PostingLine { account_code: "1000".to_string(), direction: Direction::Debit, amount_minor: 1000, memo: None },
                PostingLine { account_code: "4000".to_string(), direction: Direction::Credit, amount_minor: 1000, memo: None },
            ],
        }).unwrap();
        let transaction_id = match posted { PostOutcome::Created(id) | PostOutcome::Skipped(id) => id };
        let before = books.count_transactions().unwrap();
        let document = document_write("d");
        books.register_document(&document).unwrap();
        let attachment = DocumentAttachmentWrite {
            document_id: document.document_id.clone(),
            record_kind: "moneyIn".to_string(),
            record_id: "income-1".to_string(),
            transaction_id,
        };
        assert!(matches!(books.attach_registered_document(&attachment).unwrap(), DocumentAttachmentPersistOutcome::Attached(_)));
        assert!(matches!(books.attach_registered_document(&attachment).unwrap(), DocumentAttachmentPersistOutcome::AlreadyAttached(_)));
        assert_eq!(books.count_transactions().unwrap(), before, "attachment must not post accounting transactions");
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn attachment_wrong_authoritative_record_fails_closed() {
        let path = temp_db_path("wrong-target");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        books.create_account("1000", "Business Bank", "asset").unwrap();
        books.create_account("4000", "Sales", "revenue").unwrap();
        let posted = books.post(&PostTransactionRequest {
            description: "Income".to_string(),
            date: "2026-09-12".to_string(),
            currency_code: "GBP".to_string(),
            reference: Some("sbc7b1:moneyIn:income-1".to_string()),
            metadata: None,
            lines: vec![
                PostingLine { account_code: "1000".to_string(), direction: Direction::Debit, amount_minor: 1000, memo: None },
                PostingLine { account_code: "4000".to_string(), direction: Direction::Credit, amount_minor: 1000, memo: None },
            ],
        }).unwrap();
        let transaction_id = match posted { PostOutcome::Created(id) | PostOutcome::Skipped(id) => id };
        let document = document_write("e");
        books.register_document(&document).unwrap();
        let error = books.attach_registered_document(&DocumentAttachmentWrite {
            document_id: document.document_id,
            record_kind: "moneyOut".to_string(),
            record_id: "expense-1".to_string(),
            transaction_id,
        }).unwrap_err();
        assert_eq!(error.code, FoundationErrorCode::Validation);
        drop(books);
        let _ = fs::remove_file(path);
    }
}
