//! SBC-7B1 Batch A — Shark-owned receipt-decision and correction persistence.
//!
//! This module persists only explicit owner decisions and immutable correction
//! relationships inside the encrypted Shark application schema. It exposes no
//! raw SQLite/Beankeeper objects, paths, tax decisions or automatic accounting
//! authority.

use std::collections::HashSet;

use super::*;
use crate::bank_application::{nonblank, sqlite_error, validation};

const MAX_HISTORY_ROWS: i64 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptBankDecisionWrite {
    pub suggestion_id: String,
    pub document_id: String,
    pub decision_kind: String,
    pub bank_activity_id: Option<i64>,
    pub source_account_id: Option<String>,
    pub source_file_sha256: Option<String>,
    pub source_locator: Option<String>,
    pub raw_record_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptBankDecisionView {
    pub suggestion_id: String,
    pub document_id: String,
    pub decision_kind: String,
    pub bank_activity_id: Option<i64>,
    pub source_account_id: Option<String>,
    pub source_file_sha256: Option<String>,
    pub source_locator: Option<String>,
    pub raw_record_sha256: Option<String>,
    pub decided_by: String,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReceiptBankDecisionPersistOutcome {
    Recorded(ReceiptBankDecisionView),
    AlreadyRecorded(ReceiptBankDecisionView),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerCorrectionWrite {
    pub correction_id: String,
    pub record_kind: String,
    pub original_record_id: String,
    pub original_transaction_id: i64,
    pub reversal_record_id: String,
    pub reversal_request: PostTransactionRequest,
    pub replacement_record_id: Option<String>,
    pub replacement_request: Option<PostTransactionRequest>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerCorrectionView {
    pub correction_id: String,
    pub record_kind: String,
    pub original_record_id: String,
    pub original_transaction_id: i64,
    pub reversal_record_id: String,
    pub reversal_transaction_id: i64,
    pub replacement_record_id: Option<String>,
    pub replacement_transaction_id: Option<i64>,
    pub reason: String,
    pub corrected_by: String,
    pub corrected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OwnerCorrectionPersistOutcome {
    Applied(OwnerCorrectionView),
    AlreadyApplied(OwnerCorrectionView),
}

fn validate_receipt_decision(write: &ReceiptBankDecisionWrite) -> FoundationResult<()> {
    nonblank(&write.suggestion_id, "receipt suggestion id", 128)?;
    nonblank(&write.document_id, "receipt document id", 128)?;
    match write.decision_kind.as_str() {
        "rejected" => {
            if write.bank_activity_id.is_some()
                || write.source_account_id.is_some()
                || write.source_file_sha256.is_some()
                || write.source_locator.is_some()
                || write.raw_record_sha256.is_some()
            {
                return Err(validation(
                    "rejected receipt decision must not carry a bank identity",
                ));
            }
        }
        "confirmed" => {
            let activity_id = write
                .bank_activity_id
                .ok_or_else(|| validation("confirmed receipt decision requires bank activity id"))?;
            if activity_id <= 0 {
                return Err(validation("bank activity id must be positive"));
            }
            nonblank(
                write.source_account_id.as_deref().unwrap_or(""),
                "receipt bank source account id",
                128,
            )?;
            nonblank(
                write.source_file_sha256.as_deref().unwrap_or(""),
                "receipt bank source file sha256",
                64,
            )?;
            nonblank(
                write.source_locator.as_deref().unwrap_or(""),
                "receipt bank source locator",
                1024,
            )?;
            nonblank(
                write.raw_record_sha256.as_deref().unwrap_or(""),
                "receipt bank raw record sha256",
                64,
            )?;
        }
        _ => {
            return Err(validation(
                "receipt decision kind must be confirmed or rejected",
            ))
        }
    }
    Ok(())
}

fn receipt_decision_from_row(row: &beankeeper_cli::db::rusqlite::Row<'_>) -> FoundationResult<ReceiptBankDecisionView> {
    Ok(ReceiptBankDecisionView {
        suggestion_id: row.get(0).map_err(sqlite_error)?,
        document_id: row.get(1).map_err(sqlite_error)?,
        decision_kind: row.get(2).map_err(sqlite_error)?,
        bank_activity_id: row.get(3).map_err(sqlite_error)?,
        source_account_id: row.get(4).map_err(sqlite_error)?,
        source_file_sha256: row.get(5).map_err(sqlite_error)?,
        source_locator: row.get(6).map_err(sqlite_error)?,
        raw_record_sha256: row.get(7).map_err(sqlite_error)?,
        decided_by: row.get(8).map_err(sqlite_error)?,
        decided_at: row.get(9).map_err(sqlite_error)?,
    })
}

fn correction_from_row(row: &beankeeper_cli::db::rusqlite::Row<'_>) -> FoundationResult<OwnerCorrectionView> {
    Ok(OwnerCorrectionView {
        correction_id: row.get(0).map_err(sqlite_error)?,
        record_kind: row.get(1).map_err(sqlite_error)?,
        original_record_id: row.get(2).map_err(sqlite_error)?,
        original_transaction_id: row.get(3).map_err(sqlite_error)?,
        reversal_record_id: row.get(4).map_err(sqlite_error)?,
        reversal_transaction_id: row.get(5).map_err(sqlite_error)?,
        replacement_record_id: row.get(6).map_err(sqlite_error)?,
        replacement_transaction_id: row.get(7).map_err(sqlite_error)?,
        reason: row.get(8).map_err(sqlite_error)?,
        corrected_by: row.get(9).map_err(sqlite_error)?,
        corrected_at: row.get(10).map_err(sqlite_error)?,
    })
}

fn exact_receipt_repeat(
    existing: &ReceiptBankDecisionView,
    write: &ReceiptBankDecisionWrite,
) -> bool {
    existing.suggestion_id == write.suggestion_id
        && existing.document_id == write.document_id
        && existing.decision_kind == write.decision_kind
        && existing.bank_activity_id == write.bank_activity_id
        && existing.source_account_id == write.source_account_id
        && existing.source_file_sha256 == write.source_file_sha256
        && existing.source_locator == write.source_locator
        && existing.raw_record_sha256 == write.raw_record_sha256
}

fn exact_correction_repeat(existing: &OwnerCorrectionView, write: &OwnerCorrectionWrite) -> bool {
    existing.correction_id == write.correction_id
        && existing.record_kind == write.record_kind
        && existing.original_record_id == write.original_record_id
        && existing.original_transaction_id == write.original_transaction_id
        && existing.reversal_record_id == write.reversal_record_id
        && existing.replacement_record_id == write.replacement_record_id
        && existing.reason == write.reason
}

fn validate_correction_request(write: &OwnerCorrectionWrite) -> FoundationResult<()> {
    nonblank(&write.correction_id, "correction id", 128)?;
    nonblank(&write.original_record_id, "original record id", 128)?;
    nonblank(&write.reversal_record_id, "reversal record id", 128)?;
    nonblank(&write.reason, "correction reason", 512)?;
    if !matches!(write.record_kind.as_str(), "moneyIn" | "moneyOut") {
        return Err(validation("correction record kind must be moneyIn or moneyOut"));
    }
    if write.original_transaction_id <= 0 {
        return Err(validation("original transaction id must be positive"));
    }
    if write.original_record_id == write.reversal_record_id {
        return Err(validation("correction reversal record id must be new"));
    }
    if let Some(replacement) = write.replacement_record_id.as_deref() {
        nonblank(replacement, "replacement record id", 128)?;
        if replacement == write.original_record_id || replacement == write.reversal_record_id {
            return Err(validation("correction replacement record id must be distinct"));
        }
    }
    if write.replacement_record_id.is_some() != write.replacement_request.is_some() {
        return Err(validation(
            "replacement record id and posting request must either both exist or both be absent",
        ));
    }

    let expected_original = format!(
        "sbc7b1:{}:{}",
        write.record_kind, write.original_record_id
    );
    let expected_reversal = format!(
        "sbc7b1:correctionReversal:{}:{}",
        write.record_kind, write.reversal_record_id
    );
    if write.reversal_request.reference.as_deref() != Some(expected_reversal.as_str()) {
        return Err(validation("correction reversal reference is not canonical"));
    }
    if write.reversal_request.currency_code != "GBP" || write.reversal_request.lines.is_empty() {
        return Err(validation("correction reversal must be a non-empty GBP posting"));
    }
    if write
        .reversal_request
        .lines
        .iter()
        .any(|line| line.amount_minor <= 0)
    {
        return Err(validation("correction reversal contains invalid posting amount"));
    }

    if let (Some(replacement_id), Some(replacement_request)) = (
        write.replacement_record_id.as_deref(),
        write.replacement_request.as_ref(),
    ) {
        let expected_replacement = format!("sbc7b1:{}:{replacement_id}", write.record_kind);
        if replacement_request.reference.as_deref() != Some(expected_replacement.as_str()) {
            return Err(validation("correction replacement reference is not canonical"));
        }
        if replacement_request.currency_code != "GBP" || replacement_request.lines.is_empty() {
            return Err(validation("correction replacement must be a non-empty GBP posting"));
        }
    }

    let original_reference = expected_original;
    if original_reference.len() > 512 {
        return Err(validation("canonical correction reference is too long"));
    }
    Ok(())
}

impl Books {
    fn receipt_decision_by_suggestion(
        &self,
        suggestion_id: &str,
    ) -> FoundationResult<Option<ReceiptBankDecisionView>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT suggestion_id, document_id, decision_kind, bank_activity_id, \
                        source_account_id, source_file_sha256, source_locator, raw_record_sha256, \
                        decided_by, decided_at \
                 FROM shark_receipt_bank_decision \
                 WHERE company_slug = ?1 AND suggestion_id = ?2 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, suggestion_id))
            .map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => Ok(Some(receipt_decision_from_row(row)?)),
            None => Ok(None),
        }
    }

    pub fn persist_receipt_bank_decision(
        &self,
        write: &ReceiptBankDecisionWrite,
    ) -> FoundationResult<ReceiptBankDecisionPersistOutcome> {
        bank_application::ensure_application_schema(&self.db)?;
        validate_receipt_decision(write)?;
        let _ = self.document(&write.document_id)?;

        if let Some(existing) = self.receipt_decision_by_suggestion(&write.suggestion_id)? {
            if exact_receipt_repeat(&existing, write) {
                return Ok(ReceiptBankDecisionPersistOutcome::AlreadyRecorded(existing));
            }
            return Err(validation(
                "receipt suggestion id already has a conflicting immutable decision",
            ));
        }

        if write.decision_kind == "confirmed" {
            let activity = self.bank_activity(write.bank_activity_id.expect("validated activity id"))?;
            if activity.activity.source_account_id != write.source_account_id.as_deref().unwrap_or("")
                || activity.activity.source_file_sha256
                    != write.source_file_sha256.as_deref().unwrap_or("")
                || activity.activity.source_locator != write.source_locator.as_deref().unwrap_or("")
                || activity.activity.raw_record_sha256
                    != write.raw_record_sha256.as_deref().unwrap_or("")
            {
                return Err(validation(
                    "confirmed receipt bank identity no longer matches authoritative bank activity",
                ));
            }
        }

        self.shark_savepoint("shark_receipt_bank_decision", || {
            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_receipt_bank_decision( \
                        company_slug, suggestion_id, document_id, decision_kind, bank_activity_id, \
                        source_account_id, source_file_sha256, source_locator, raw_record_sha256, decided_by \
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (
                        &self.company_slug,
                        &write.suggestion_id,
                        &write.document_id,
                        &write.decision_kind,
                        write.bank_activity_id,
                        write.source_account_id.as_deref(),
                        write.source_file_sha256.as_deref(),
                        write.source_locator.as_deref(),
                        write.raw_record_sha256.as_deref(),
                        self.actor.name(),
                    ),
                )
                .map_err(sqlite_error)?;
            Ok(())
        })?;

        let stored = self
            .receipt_decision_by_suggestion(&write.suggestion_id)?
            .ok_or_else(|| {
                FoundationError::new(
                    FoundationErrorCode::Storage,
                    "persisted receipt decision could not be read back",
                )
            })?;
        Ok(ReceiptBankDecisionPersistOutcome::Recorded(stored))
    }

    fn correction_by_id(&self, correction_id: &str) -> FoundationResult<Option<OwnerCorrectionView>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT correction_id, record_kind, original_record_id, original_transaction_id, \
                        reversal_record_id, reversal_transaction_id, replacement_record_id, \
                        replacement_transaction_id, reason, corrected_by, corrected_at \
                 FROM shark_owner_correction \
                 WHERE company_slug = ?1 AND correction_id = ?2 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, correction_id))
            .map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => Ok(Some(correction_from_row(row)?)),
            None => Ok(None),
        }
    }

    pub fn correction_for_original(
        &self,
        record_kind: &str,
        original_record_id: &str,
    ) -> FoundationResult<Option<OwnerCorrectionView>> {
        bank_application::ensure_application_schema(&self.db)?;
        if !matches!(record_kind, "moneyIn" | "moneyOut") {
            return Err(validation("correction record kind must be moneyIn or moneyOut"));
        }
        nonblank(original_record_id, "original record id", 128)?;
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT correction_id, record_kind, original_record_id, original_transaction_id, \
                        reversal_record_id, reversal_transaction_id, replacement_record_id, \
                        replacement_transaction_id, reason, corrected_by, corrected_at \
                 FROM shark_owner_correction \
                 WHERE company_slug = ?1 AND record_kind = ?2 AND original_record_id = ?3 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, record_kind, original_record_id))
            .map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => Ok(Some(correction_from_row(row)?)),
            None => Ok(None),
        }
    }

    pub fn apply_owner_correction(
        &self,
        write: &OwnerCorrectionWrite,
    ) -> FoundationResult<OwnerCorrectionPersistOutcome> {
        bank_application::ensure_application_schema(&self.db)?;
        validate_correction_request(write)?;

        if let Some(existing) = self.correction_by_id(&write.correction_id)? {
            if exact_correction_repeat(&existing, write) {
                return Ok(OwnerCorrectionPersistOutcome::AlreadyApplied(existing));
            }
            return Err(validation("correction id already exists with conflicting immutable content"));
        }
        if self
            .correction_for_original(&write.record_kind, &write.original_record_id)?
            .is_some()
        {
            return Err(validation(
                "original owner record is already superseded by a correction",
            ));
        }

        let original = self.transaction(write.original_transaction_id)?;
        let expected_original = format!(
            "sbc7b1:{}:{}",
            write.record_kind, write.original_record_id
        );
        if original.reference.as_deref() != Some(expected_original.as_str()) {
            return Err(validation(
                "authoritative original transaction does not match owner record identity",
            ));
        }

        let reversal_reference = write
            .reversal_request
            .reference
            .as_deref()
            .expect("validated reversal reference");
        if !self.find_by_reference(reversal_reference)?.is_empty() {
            return Err(validation("correction reversal reference already exists"));
        }
        if let Some(replacement_request) = write.replacement_request.as_ref() {
            let reference = replacement_request
                .reference
                .as_deref()
                .expect("validated replacement reference");
            if !self.find_by_reference(reference)?.is_empty() {
                return Err(validation("correction replacement reference already exists"));
            }
        }

        self.shark_savepoint("shark_owner_correction_apply", || {
            let reversal_transaction_id = match self.post(&write.reversal_request)? {
                PostOutcome::Created(id) => id,
                PostOutcome::Skipped(_) => {
                    return Err(validation(
                        "correction reversal unexpectedly resolved as an existing posting",
                    ))
                }
            };
            let replacement_transaction_id = match write.replacement_request.as_ref() {
                Some(request) => match self.post(request)? {
                    PostOutcome::Created(id) => Some(id),
                    PostOutcome::Skipped(_) => {
                        return Err(validation(
                            "correction replacement unexpectedly resolved as an existing posting",
                        ))
                    }
                },
                None => None,
            };

            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_owner_correction( \
                        company_slug, correction_id, record_kind, original_record_id, original_transaction_id, \
                        reversal_record_id, reversal_transaction_id, replacement_record_id, \
                        replacement_transaction_id, reason, corrected_by \
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    (
                        &self.company_slug,
                        &write.correction_id,
                        &write.record_kind,
                        &write.original_record_id,
                        write.original_transaction_id,
                        &write.reversal_record_id,
                        reversal_transaction_id,
                        write.replacement_record_id.as_deref(),
                        replacement_transaction_id,
                        &write.reason,
                        self.actor.name(),
                    ),
                )
                .map_err(sqlite_error)?;
            Ok(())
        })?;

        let stored = self.correction_by_id(&write.correction_id)?.ok_or_else(|| {
            FoundationError::new(
                FoundationErrorCode::Storage,
                "persisted owner correction could not be read back",
            )
        })?;
        Ok(OwnerCorrectionPersistOutcome::Applied(stored))
    }

    pub fn correction_history(
        &self,
        record_kind: &str,
        record_id: &str,
        limit: i64,
    ) -> FoundationResult<Vec<OwnerCorrectionView>> {
        bank_application::ensure_application_schema(&self.db)?;
        if !matches!(record_kind, "moneyIn" | "moneyOut") {
            return Err(validation("correction record kind must be moneyIn or moneyOut"));
        }
        nonblank(record_id, "correction history record id", 128)?;
        if !(1..=MAX_HISTORY_ROWS).contains(&limit) {
            return Err(validation("correction history limit must be between 1 and 100"));
        }

        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT correction_id, record_kind, original_record_id, original_transaction_id, \
                        reversal_record_id, reversal_transaction_id, replacement_record_id, \
                        replacement_transaction_id, reason, corrected_by, corrected_at \
                 FROM shark_owner_correction \
                 WHERE company_slug = ?1 AND record_kind = ?2 \
                 ORDER BY id ASC LIMIT 100",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, record_kind))
            .map_err(sqlite_error)?;
        let mut all = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            all.push(correction_from_row(row)?);
        }

        let mut connected = HashSet::new();
        connected.insert(record_id.to_string());
        loop {
            let mut changed = false;
            for row in &all {
                let touches = connected.contains(&row.original_record_id)
                    || connected.contains(&row.reversal_record_id)
                    || row
                        .replacement_record_id
                        .as_ref()
                        .is_some_and(|value| connected.contains(value));
                if touches {
                    changed |= connected.insert(row.original_record_id.clone());
                    changed |= connected.insert(row.reversal_record_id.clone());
                    if let Some(value) = row.replacement_record_id.as_ref() {
                        changed |= connected.insert(value.clone());
                    }
                }
            }
            if !changed {
                break;
            }
        }

        Ok(all
            .into_iter()
            .filter(|row| connected.contains(&row.original_record_id))
            .take(limit as usize)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "shark-sbc7b1-mutation-audit-{label}-{}-{nonce}.db",
            std::process::id()
        ))
    }

    fn two_line_request(reference: &str, description: &str, debit: &str, credit: &str) -> PostTransactionRequest {
        PostTransactionRequest {
            description: description.to_string(),
            date: "2026-09-14".to_string(),
            currency_code: "GBP".to_string(),
            reference: Some(reference.to_string()),
            metadata: None,
            lines: vec![
                PostingLine { account_code: debit.to_string(), direction: Direction::Debit, amount_minor: 1_000, memo: None },
                PostingLine { account_code: credit.to_string(), direction: Direction::Credit, amount_minor: 1_000, memo: None },
            ],
        }
    }

    #[test]
    fn rejected_receipt_decision_is_idempotent_and_non_posting() {
        let path = temp_db_path("receipt-reject");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        let document = DocumentWrite {
            document_id: format!("doc-{}", "11".repeat(32)),
            storage_root_id: "root-1".into(),
            relative_path: format!("documents/{}/receipt.png", "11".repeat(32)),
            original_filename: "receipt.png".into(),
            media_type: Some("image/png".into()),
            sha256: "11".repeat(32),
            byte_len: 12,
        };
        books.register_document(&document).unwrap();
        let before = books.count_transactions().unwrap();
        let write = ReceiptBankDecisionWrite {
            suggestion_id: "suggestion-1".into(),
            document_id: document.document_id.clone(),
            decision_kind: "rejected".into(),
            bank_activity_id: None,
            source_account_id: None,
            source_file_sha256: None,
            source_locator: None,
            raw_record_sha256: None,
        };
        assert!(matches!(
            books.persist_receipt_bank_decision(&write).unwrap(),
            ReceiptBankDecisionPersistOutcome::Recorded(_)
        ));
        assert!(matches!(
            books.persist_receipt_bank_decision(&write).unwrap(),
            ReceiptBankDecisionPersistOutcome::AlreadyRecorded(_)
        ));
        assert_eq!(before, books.count_transactions().unwrap());
        let mut conflict = write.clone();
        conflict.decision_kind = "confirmed".into();
        assert!(books.persist_receipt_bank_decision(&conflict).is_err());
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn correction_applies_reversal_and_replacement_atomically_and_is_idempotent() {
        let path = temp_db_path("correction");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        books.create_account("1000", "Bank", "asset").unwrap();
        books.create_account("4000", "Sales", "revenue").unwrap();
        let original = two_line_request("sbc7b1:moneyIn:original-1", "Original", "1000", "4000");
        let original_id = match books.post(&original).unwrap() {
            PostOutcome::Created(id) => id,
            PostOutcome::Skipped(_) => unreachable!(),
        };
        let reversal = two_line_request(
            "sbc7b1:correctionReversal:moneyIn:reversal-1",
            "Reversal",
            "4000",
            "1000",
        );
        let replacement = two_line_request(
            "sbc7b1:moneyIn:replacement-1",
            "Replacement",
            "1000",
            "4000",
        );
        let write = OwnerCorrectionWrite {
            correction_id: "corr-1".into(),
            record_kind: "moneyIn".into(),
            original_record_id: "original-1".into(),
            original_transaction_id: original_id,
            reversal_record_id: "reversal-1".into(),
            reversal_request: reversal,
            replacement_record_id: Some("replacement-1".into()),
            replacement_request: Some(replacement),
            reason: "Correct amount".into(),
        };
        let before = books.count_transactions().unwrap();
        let first = books.apply_owner_correction(&write).unwrap();
        assert!(matches!(first, OwnerCorrectionPersistOutcome::Applied(_)));
        assert_eq!(books.count_transactions().unwrap(), before + 2);
        let repeat = books.apply_owner_correction(&write).unwrap();
        assert!(matches!(repeat, OwnerCorrectionPersistOutcome::AlreadyApplied(_)));
        assert_eq!(books.count_transactions().unwrap(), before + 2);
        assert_eq!(books.correction_history("moneyIn", "original-1", 10).unwrap().len(), 1);
        assert!(books.correction_for_original("moneyIn", "original-1").unwrap().is_some());
        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn failed_replacement_rolls_back_reversal_and_history() {
        let path = temp_db_path("rollback");
        let books = Books::create_plain_for_test(&path, "test-books", "Test Books", "owner").unwrap();
        books.create_account("1000", "Bank", "asset").unwrap();
        books.create_account("4000", "Sales", "revenue").unwrap();
        let original = two_line_request("sbc7b1:moneyIn:original-2", "Original", "1000", "4000");
        let original_id = match books.post(&original).unwrap() {
            PostOutcome::Created(id) => id,
            PostOutcome::Skipped(_) => unreachable!(),
        };
        let reversal = two_line_request(
            "sbc7b1:correctionReversal:moneyIn:reversal-2",
            "Reversal",
            "4000",
            "1000",
        );
        let mut replacement = two_line_request(
            "sbc7b1:moneyIn:replacement-2",
            "Replacement",
            "1000",
            "missing-account",
        );
        replacement.lines[1].account_code = "9999".into();
        let write = OwnerCorrectionWrite {
            correction_id: "corr-rollback".into(),
            record_kind: "moneyIn".into(),
            original_record_id: "original-2".into(),
            original_transaction_id: original_id,
            reversal_record_id: "reversal-2".into(),
            reversal_request: reversal,
            replacement_record_id: Some("replacement-2".into()),
            replacement_request: Some(replacement),
            reason: "Force rollback".into(),
        };
        let before = books.count_transactions().unwrap();
        assert!(books.apply_owner_correction(&write).is_err());
        assert_eq!(books.count_transactions().unwrap(), before);
        assert!(books.find_by_reference("sbc7b1:correctionReversal:moneyIn:reversal-2").unwrap().is_empty());
        assert!(books.correction_for_original("moneyIn", "original-2").unwrap().is_none());
        drop(books);
        let _ = fs::remove_file(path);
    }
}
