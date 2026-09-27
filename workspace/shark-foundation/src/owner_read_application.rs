//! SBC-7B2 owner-safe read models for conventional Money and document screens.
//!
//! This module deliberately collapses ledger-shaped Foundation rows into bounded
//! owner semantics before they can cross an application bridge. It owns no
//! posting, matching, reconciliation, tax or filesystem authority.

use std::collections::HashSet;

use super::*;
use crate::bank_application::{nonblank, sqlite_error, validation};

const MAX_OWNER_READ_PAGE: i64 = 200;
const MAX_OWNER_READ_OFFSET: i64 = 1_000_000;
const MAX_CORRECTION_HISTORY: i64 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OwnerMoneyRecordRead {
    pub record_id: String,
    pub record_kind: String,
    pub transaction_id: i64,
    pub description: String,
    pub date: String,
    pub amount_pence: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OwnerDocumentRead {
    pub document_id: String,
    pub original_filename: String,
    pub media_type: Option<String>,
    pub byte_len: u64,
    pub registered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OwnerCorrectionRead {
    pub correction_id: String,
    pub reason: String,
    pub corrected_at: String,
    pub replacement_record_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OwnerMoneyRecordDetailRead {
    pub record: OwnerMoneyRecordRead,
    pub documents: Vec<OwnerDocumentRead>,
    pub corrections: Vec<OwnerCorrectionRead>,
}

fn owner_kind(value: &str) -> FoundationResult<&str> {
    match value {
        "moneyIn" | "moneyOut" => Ok(value),
        _ => Err(validation(
            "owner money record kind must be moneyIn or moneyOut",
        )),
    }
}

fn owner_reference_prefix(kind: &str) -> FoundationResult<&'static str> {
    match owner_kind(kind)? {
        "moneyIn" => Ok("sbc7b1:moneyIn:"),
        "moneyOut" => Ok("sbc7b1:moneyOut:"),
        _ => unreachable!("owner_kind validated the value"),
    }
}

fn owner_amount(entries: &[EntryView]) -> FoundationResult<i64> {
    if entries.is_empty() {
        return Err(FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted owner record has no accounting entries",
        ));
    }
    let mut debits = 0_i64;
    let mut credits = 0_i64;
    for entry in entries {
        if entry.amount_minor <= 0 {
            return Err(FoundationError::new(
                FoundationErrorCode::Storage,
                "persisted owner record has a non-positive amount",
            ));
        }
        match entry.direction.as_str() {
            "debit" => {
                debits = debits.checked_add(entry.amount_minor).ok_or_else(|| {
                    FoundationError::new(
                        FoundationErrorCode::Storage,
                        "persisted owner record debit total overflow",
                    )
                })?;
            }
            "credit" => {
                credits = credits.checked_add(entry.amount_minor).ok_or_else(|| {
                    FoundationError::new(
                        FoundationErrorCode::Storage,
                        "persisted owner record credit total overflow",
                    )
                })?;
            }
            _ => {
                return Err(FoundationError::new(
                    FoundationErrorCode::Storage,
                    "persisted owner record has an invalid entry direction",
                ));
            }
        }
    }
    if debits <= 0 || debits != credits {
        return Err(FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted owner record is not a positive balanced transaction",
        ));
    }
    Ok(debits)
}

fn owner_record_from_transaction(
    kind: &str,
    expected_record_id: Option<&str>,
    transaction: TransactionView,
) -> FoundationResult<OwnerMoneyRecordRead> {
    let prefix = owner_reference_prefix(kind)?;
    let reference = transaction.reference.as_deref().ok_or_else(|| {
        FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted owner record is missing its canonical reference",
        )
    })?;
    let record_id = reference.strip_prefix(prefix).ok_or_else(|| {
        FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted owner record reference does not match its owner kind",
        )
    })?;
    nonblank(record_id, "owner record id", 128)?;
    if let Some(expected) = expected_record_id {
        if record_id != expected {
            return Err(FoundationError::new(
                FoundationErrorCode::Storage,
                "persisted owner record identity changed during readback",
            ));
        }
    }
    if transaction.currency != "GBP" {
        return Err(FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted owner record currency is not GBP",
        ));
    }
    let amount_pence = owner_amount(&transaction.entries)?;
    Ok(OwnerMoneyRecordRead {
        record_id: record_id.to_string(),
        record_kind: kind.to_string(),
        transaction_id: transaction.id,
        description: transaction.description,
        date: transaction.date,
        amount_pence,
        currency: transaction.currency,
    })
}

fn transaction_view(
    self_books: &Books,
    txn: beankeeper_cli::db::TransactionRow,
) -> FoundationResult<TransactionView> {
    let entries =
        db::get_entries_for_transaction(self_books.db.conn(), txn.id).map_err(map_cli_error)?;
    Ok(TransactionView {
        id: txn.id,
        description: txn.description,
        reference: txn.reference,
        currency: txn.currency,
        date: txn.date,
        entries: entries
            .into_iter()
            .map(|entry| EntryView {
                id: entry.id,
                account_code: entry.account_code,
                direction: entry.direction,
                amount_minor: entry.amount,
                status: entry.status,
            })
            .collect(),
    })
}

fn document_read_from_row(
    document_id: String,
    original_filename: String,
    media_type: Option<String>,
    byte_len_raw: i64,
    registered_at: String,
) -> FoundationResult<OwnerDocumentRead> {
    let byte_len = u64::try_from(byte_len_raw).map_err(|_| {
        FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted document byte length is invalid",
        )
    })?;
    if byte_len == 0 {
        return Err(FoundationError::new(
            FoundationErrorCode::Storage,
            "persisted document byte length must be positive",
        ));
    }
    Ok(OwnerDocumentRead {
        document_id,
        original_filename,
        media_type,
        byte_len,
        registered_at,
    })
}

fn superseded_owner_record_ids(
    books: &Books,
    record_kind: &str,
) -> FoundationResult<HashSet<String>> {
    owner_kind(record_kind)?;
    crate::bank_application::ensure_application_schema(&books.db)?;
    let mut statement = books
        .db
        .conn()
        .prepare(
            "SELECT original_record_id FROM shark_owner_correction \
             WHERE company_slug = ?1 AND record_kind = ?2",
        )
        .map_err(sqlite_error)?;
    let mut rows = statement
        .query((&books.company_slug, record_kind))
        .map_err(sqlite_error)?;
    let mut result = HashSet::new();
    while let Some(row) = rows.next().map_err(sqlite_error)? {
        let record_id: String = row.get(0).map_err(sqlite_error)?;
        nonblank(&record_id, "persisted correction original record id", 128)?;
        result.insert(record_id);
    }
    Ok(result)
}

impl Books {
    pub fn owner_money_records(
        &self,
        record_kind: &str,
        limit: i64,
        offset: i64,
    ) -> FoundationResult<Vec<OwnerMoneyRecordRead>> {
        let prefix = owner_reference_prefix(record_kind)?;
        if !(1..=MAX_OWNER_READ_PAGE).contains(&limit) {
            return Err(validation(
                "owner money page limit must be between 1 and 200",
            ));
        }
        if !(0..=MAX_OWNER_READ_OFFSET).contains(&offset) {
            return Err(validation(
                "owner money page offset is outside the supported range",
            ));
        }

        // Canonical sbc7b1:<kind>:<recordId> references identify ordinary owner
        // records and correction replacements. Correction reversals use a distinct
        // sbc7b1:correctionReversal: prefix and therefore never enter this view.
        // Any record already superseded by a correction is omitted so the list
        // presents the current owner record set rather than double-counting history.
        let superseded = superseded_owner_record_ids(self, record_kind)?;
        let count_params = ListTransactionParams::for_company(&self.company_slug);
        let total = db::count_transactions(self.db.conn(), &count_params).map_err(map_cli_error)?;
        let requested = usize::try_from(limit).map_err(|_| {
            validation("owner money page limit cannot be represented on this platform")
        })?;
        let mut scan_end = total;
        let mut matching_seen = 0_i64;
        let mut result = Vec::new();

        while scan_end > 0 && result.len() < requested {
            let take = MAX_OWNER_READ_PAGE.min(scan_end);
            let start = scan_end - take;
            let mut params = ListTransactionParams::for_company(&self.company_slug);
            params.limit = take;
            params.offset = start;
            let mut rows = db::list_transactions(self.db.conn(), &params).map_err(map_cli_error)?;
            rows.reverse();

            for txn in rows {
                let Some(reference) = txn.reference.as_deref() else {
                    continue;
                };
                let Some(record_id) = reference.strip_prefix(prefix) else {
                    continue;
                };
                nonblank(record_id, "owner record id", 128)?;
                if superseded.contains(record_id) {
                    continue;
                }
                if matching_seen < offset {
                    matching_seen += 1;
                    continue;
                }
                let transaction = transaction_view(self, txn)?;
                result.push(owner_record_from_transaction(
                    record_kind,
                    None,
                    transaction,
                )?);
                if result.len() == requested {
                    break;
                }
            }
            scan_end = start;
        }
        Ok(result)
    }

    pub fn owner_money_record(
        &self,
        record_kind: &str,
        record_id: &str,
    ) -> FoundationResult<OwnerMoneyRecordRead> {
        owner_kind(record_kind)?;
        nonblank(record_id, "owner record id", 128)?;
        let reference = format!("sbc7b1:{record_kind}:{record_id}");
        let mut matches = self.find_by_reference(&reference)?;
        if matches.len() != 1 {
            return Err(FoundationError::new(
                if matches.is_empty() {
                    FoundationErrorCode::NotFound
                } else {
                    FoundationErrorCode::Storage
                },
                if matches.is_empty() {
                    "owner money record was not found"
                } else {
                    "owner money record reference is not unique"
                },
            ));
        }
        owner_record_from_transaction(record_kind, Some(record_id), matches.remove(0))
    }

    pub fn owner_documents(
        &self,
        limit: i64,
        offset: i64,
    ) -> FoundationResult<Vec<OwnerDocumentRead>> {
        crate::bank_application::ensure_application_schema(&self.db)?;
        if !(1..=MAX_OWNER_READ_PAGE).contains(&limit) {
            return Err(validation("document page limit must be between 1 and 200"));
        }
        if !(0..=MAX_OWNER_READ_OFFSET).contains(&offset) {
            return Err(validation(
                "document page offset is outside the supported range",
            ));
        }
        let mut statement = self
            .db
            .conn()
            .prepare(
                "SELECT document_id, original_filename, media_type, byte_len, registered_at \
             FROM shark_document WHERE company_slug = ?1 \
             ORDER BY registered_at DESC, document_id DESC LIMIT ?2 OFFSET ?3",
            )
            .map_err(sqlite_error)?;
        let mut rows = statement
            .query((&self.company_slug, limit, offset))
            .map_err(sqlite_error)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            result.push(document_read_from_row(
                row.get(0).map_err(sqlite_error)?,
                row.get(1).map_err(sqlite_error)?,
                row.get(2).map_err(sqlite_error)?,
                row.get(3).map_err(sqlite_error)?,
                row.get(4).map_err(sqlite_error)?,
            )?);
        }
        Ok(result)
    }

    fn owner_documents_for_record(
        &self,
        record_kind: &str,
        record_id: &str,
    ) -> FoundationResult<Vec<OwnerDocumentRead>> {
        crate::bank_application::ensure_application_schema(&self.db)?;
        owner_kind(record_kind)?;
        nonblank(record_id, "owner record id", 128)?;
        let mut statement = self.db.conn().prepare(
            "SELECT d.document_id, d.original_filename, d.media_type, d.byte_len, d.registered_at \
             FROM shark_document d JOIN shark_document_attachment a \
               ON a.company_slug = d.company_slug AND a.document_id = d.document_id \
             WHERE a.company_slug = ?1 AND a.record_kind = ?2 AND a.record_id = ?3 \
             ORDER BY a.attached_at ASC, d.document_id ASC",
        ).map_err(sqlite_error)?;
        let mut rows = statement
            .query((&self.company_slug, record_kind, record_id))
            .map_err(sqlite_error)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            result.push(document_read_from_row(
                row.get(0).map_err(sqlite_error)?,
                row.get(1).map_err(sqlite_error)?,
                row.get(2).map_err(sqlite_error)?,
                row.get(3).map_err(sqlite_error)?,
                row.get(4).map_err(sqlite_error)?,
            )?);
        }
        Ok(result)
    }

    pub fn owner_money_record_detail(
        &self,
        record_kind: &str,
        record_id: &str,
    ) -> FoundationResult<OwnerMoneyRecordDetailRead> {
        let record = self.owner_money_record(record_kind, record_id)?;
        let documents = self.owner_documents_for_record(record_kind, record_id)?;
        let corrections = self
            .correction_history(record_kind, record_id, MAX_CORRECTION_HISTORY)?
            .into_iter()
            .map(|value| OwnerCorrectionRead {
                correction_id: value.correction_id,
                reason: value.reason,
                corrected_at: value.corrected_at,
                replacement_record_id: value.replacement_record_id,
            })
            .collect();
        Ok(OwnerMoneyRecordDetailRead {
            record,
            documents,
            corrections,
        })
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
            "shark-sbc7b2-read-{label}-{}-{nonce}.db",
            std::process::id()
        ))
    }

    fn seed_owner_record(books: &Books, kind: &str, id: &str, date: &str, amount: i64) {
        books.create_account("1000", "Business Bank", "asset").ok();
        let counter = if kind == "moneyIn" { "4000" } else { "5000" };
        let counter_type = if kind == "moneyIn" {
            "revenue"
        } else {
            "expense"
        };
        books.create_account(counter, "Counter", counter_type).ok();
        let (first, second) = if kind == "moneyIn" {
            (Direction::Debit, Direction::Credit)
        } else {
            (Direction::Credit, Direction::Debit)
        };
        let request = PostTransactionRequest {
            description: format!("{kind} {id}"),
            date: date.to_string(),
            currency_code: "GBP".to_string(),
            reference: Some(format!("sbc7b1:{kind}:{id}")),
            metadata: Some(
                serde_json::json!({
                    "source": "owner-ui",
                    "recordKind": kind,
                    "recordId": id,
                    "bridgeVersion": 1
                })
                .to_string(),
            ),
            lines: vec![
                PostingLine {
                    account_code: "1000".to_string(),
                    direction: first,
                    amount_minor: amount,
                    memo: None,
                },
                PostingLine {
                    account_code: counter.to_string(),
                    direction: second,
                    amount_minor: amount,
                    memo: None,
                },
            ],
        };
        books.post(&request).unwrap();
    }

    #[test]
    fn owner_money_reads_are_paged_newest_first_without_ledger_rows() {
        let path = temp_db_path("money-list");
        let books = Books::create_plain_for_test(&path, "test-books", "Test", "owner").unwrap();
        seed_owner_record(&books, "moneyOut", "expense-1", "2026-09-01", 1200);
        seed_owner_record(&books, "moneyOut", "expense-2", "2026-09-03", 3400);
        seed_owner_record(&books, "moneyIn", "income-1", "2026-09-02", 5000);

        let first = books.owner_money_records("moneyOut", 1, 0).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].record_id, "expense-2");
        assert_eq!(first[0].amount_pence, 3400);
        let second = books.owner_money_records("moneyOut", 1, 1).unwrap();
        assert_eq!(second[0].record_id, "expense-1");
        assert!(books.owner_money_records("moneyOut", 0, 0).is_err());
        assert!(books.owner_money_records("other", 10, 0).is_err());

        drop(books);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn owner_document_list_and_money_detail_return_only_safe_evidence_metadata() {
        let path = temp_db_path("detail");
        let books = Books::create_plain_for_test(&path, "test-books", "Test", "owner").unwrap();
        seed_owner_record(&books, "moneyOut", "expense-doc", "2026-09-04", 2500);
        let hash = "a".repeat(64);
        let write = DocumentWrite {
            document_id: format!("doc-{hash}"),
            storage_root_id: "private-root".to_string(),
            relative_path: format!("documents/{hash}/receipt.png"),
            original_filename: "receipt.png".to_string(),
            media_type: Some("image/png".to_string()),
            sha256: hash,
            byte_len: 128,
        };
        books.register_document(&write).unwrap();
        let txn = books.owner_money_record("moneyOut", "expense-doc").unwrap();
        books
            .attach_registered_document(&DocumentAttachmentWrite {
                document_id: write.document_id.clone(),
                record_kind: "moneyOut".to_string(),
                record_id: "expense-doc".to_string(),
                transaction_id: txn.transaction_id,
            })
            .unwrap();

        let docs = books.owner_documents(10, 0).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].original_filename, "receipt.png");
        let detail = books
            .owner_money_record_detail("moneyOut", "expense-doc")
            .unwrap();
        assert_eq!(detail.documents.len(), 1);
        assert_eq!(detail.documents[0].document_id, write.document_id);
        assert!(detail.corrections.is_empty());

        drop(books);
        let _ = fs::remove_file(path);
    }
}
