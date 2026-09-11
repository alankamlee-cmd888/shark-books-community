use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_db_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "shark-sbc7b1-bank-{label}-{}-{nonce}.db",
        std::process::id()
    ))
}

fn test_books(label: &str) -> (Books, PathBuf) {
    let path = temp_db_path(label);
    let books = Books::create_plain_for_test(&path, "bank-test", "Bank Test", "local-owner")
        .expect("create test books");
    books
        .create_account("1000", "Business Bank", "asset")
        .expect("create bank account");
    books
        .create_account("9000", "Bank Import Suspense", "asset")
        .expect("create suspense account");
    ensure_application_schema(&books.db).expect("application schema");
    (books, path)
}

fn hash(marker: char) -> String {
    marker.to_string().repeat(64)
}

fn activity(
    locator: &str,
    raw_marker: char,
    strong_identity_key: Option<&str>,
    amount: i64,
) -> BankActivityWrite {
    BankActivityWrite {
        source_account_id: "bank-main".to_string(),
        institution_account_id: None,
        source_format: "csv".to_string(),
        source_file_sha256: hash('a'),
        source_locator: locator.to_string(),
        posted_date: "2026-09-11".to_string(),
        value_date: None,
        signed_amount_minor: amount,
        currency_code: "GBP".to_string(),
        description: format!("Bank row {locator}"),
        payee: Some("Example supplier".to_string()),
        reference: None,
        external_transaction_id: strong_identity_key.map(|_| format!("T-{locator}")),
        raw_record_sha256: hash(raw_marker),
        strong_identity_key: strong_identity_key.map(str::to_string),
        provenance_kind: "csv".to_string(),
        provenance_source_reference: Some(locator.to_string()),
        provenance_fingerprint: Some(hash(raw_marker)),
        provenance_label: Some("Test bank".to_string()),
    }
}

fn post_bank_transaction(books: &Books, reference: &str, amount: i64) -> (i64, i64) {
    let (bank_direction, suspense_direction) = if amount > 0 {
        (Direction::Debit, Direction::Credit)
    } else {
        (Direction::Credit, Direction::Debit)
    };
    let absolute = amount.unsigned_abs();
    let absolute = i64::try_from(absolute).expect("test amount in range");
    let outcome = books
        .post(&PostTransactionRequest {
            description: "Matched bank transaction".to_string(),
            date: "2026-09-11".to_string(),
            currency_code: "GBP".to_string(),
            reference: Some(reference.to_string()),
            metadata: None,
            lines: vec![
                PostingLine {
                    account_code: "1000".to_string(),
                    direction: bank_direction,
                    amount_minor: absolute,
                    memo: None,
                },
                PostingLine {
                    account_code: "9000".to_string(),
                    direction: suspense_direction,
                    amount_minor: absolute,
                    memo: None,
                },
            ],
        })
        .expect("post balanced bank transaction");
    let transaction_id = match outcome {
        PostOutcome::Created(id) => id,
        PostOutcome::Skipped(_) => panic!("unexpected duplicate test transaction"),
    };
    let transaction = books.transaction(transaction_id).expect("transaction");
    let bank_entry = transaction
        .entries
        .iter()
        .find(|entry| entry.account_code == "1000")
        .expect("bank entry");
    (transaction_id, bank_entry.id)
}

fn confirm_match_for(
    books: &Books,
    bank_activity_id: i64,
    transaction_id: i64,
    entry_id: i64,
) -> BankMatchPersistOutcome {
    books
        .confirm_bank_match(&BankMatchWrite {
            bank_activity_id,
            transaction_id,
            entry_id,
            match_level: "likely".to_string(),
            score: 75,
            reasons: vec![
                "accountExact".to_string(),
                "amountExact".to_string(),
                "dateExact".to_string(),
            ],
        })
        .expect("confirm bank match")
}

#[test]
fn application_schema_v2_is_separate_from_beankeeper_schema_8() {
    let (books, path) = test_books("schema");
    assert_eq!(SHARK_APPLICATION_SCHEMA_VERSION, 2);
    assert_eq!(books.verify().expect("Beankeeper schema"), 8);
    let observed: i64 = books
        .db
        .conn()
        .query_row(
            "SELECT schema_version FROM shark_application_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .expect("Shark application schema row");
    assert_eq!(observed, 2);
    drop(books);
    remove_sqlite_artifacts(&path);
}

#[test]
fn bank_activity_batch_is_atomic_and_does_not_post_accounting_transactions() {
    let (books, path) = test_books("activity-atomic");
    let before_transactions = books.count_transactions().expect("before count");
    let strong = "csv:GBP:bank-main:T-row-1";
    let first = activity("row-1", 'b', Some(strong), -2_500);
    let outcomes = books
        .persist_bank_activity_batch(std::slice::from_ref(&first))
        .expect("initial import");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].kind, BankActivityPersistKind::Created);
    assert_eq!(
        books.count_transactions().expect("after import count"),
        before_transactions,
        "bank activity persistence must not post an accounting transaction"
    );

    let duplicate = books
        .persist_bank_activity_batch(std::slice::from_ref(&first))
        .expect("strong duplicate");
    assert_eq!(duplicate[0].kind, BankActivityPersistKind::StrongDuplicate);

    let file_exact = activity("row-file", 'c', None, -1_000);
    let created = books
        .persist_bank_activity_batch(std::slice::from_ref(&file_exact))
        .expect("file exact seed");
    assert_eq!(created[0].kind, BankActivityPersistKind::Created);
    let repeated = books
        .persist_bank_activity_batch(std::slice::from_ref(&file_exact))
        .expect("file exact duplicate");
    assert_eq!(
        repeated[0].kind,
        BankActivityPersistKind::FileExactDuplicate
    );

    let count_before_failed_batch = books
        .list_bank_activity(500, 0)
        .expect("activity list")
        .len();
    let new_row = activity("row-atomic-new", 'd', None, 3_000);
    let conflict = activity("row-conflict", 'e', Some(strong), -2_600);
    assert!(
        books
            .persist_bank_activity_batch(&[new_row, conflict])
            .is_err(),
        "conflicting strong identity must fail the whole batch"
    );
    let after_failed_batch = books
        .list_bank_activity(500, 0)
        .expect("activity list after failure");
    assert_eq!(after_failed_batch.len(), count_before_failed_batch);
    assert!(
        after_failed_batch
            .iter()
            .all(|row| row.activity.source_locator != "row-atomic-new"),
        "earlier rows from a failed batch must roll back"
    );

    drop(books);
    remove_sqlite_artifacts(&path);
}

#[test]
fn match_confirmation_is_atomic_audited_forward_only_and_idempotent() {
    let (books, path) = test_books("match");
    let source = activity(
        "match-row",
        'f',
        Some("csv:GBP:bank-main:T-match-row"),
        -2_500,
    );
    let persisted = books
        .persist_bank_activity_batch(std::slice::from_ref(&source))
        .expect("persist activity");
    let activity_id = persisted[0].activity_id;
    let (transaction_id, entry_id) = post_bank_transaction(&books, "match-ledger", -2_500);

    let confirmed = confirm_match_for(&books, activity_id, transaction_id, entry_id);
    let view = match confirmed {
        BankMatchPersistOutcome::Confirmed(view) => view,
        BankMatchPersistOutcome::AlreadyConfirmed(_) => panic!("first match must be new"),
    };
    assert_eq!(view.bank_activity_id, activity_id);
    assert_eq!(view.transaction_id, transaction_id);
    assert_eq!(view.entry_id, entry_id);

    let transaction = books.transaction(transaction_id).expect("matched transaction");
    let bank_entry = transaction
        .entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .expect("matched entry");
    assert_eq!(bank_entry.status, "cleared");
    let audit = books.audit_status_changes().expect("status audit");
    assert!(audit.iter().any(|row| {
        row.entity_id == entry_id.to_string()
            && row.before.as_deref().is_some_and(|value| value.contains("uncleared"))
            && row.after.as_deref().is_some_and(|value| value.contains("cleared"))
    }));

    let repeated = confirm_match_for(&books, activity_id, transaction_id, entry_id);
    assert!(matches!(
        repeated,
        BankMatchPersistOutcome::AlreadyConfirmed(_)
    ));

    let other_activity = activity(
        "other-match-row",
        '1',
        Some("csv:GBP:bank-main:T-other-match-row"),
        -2_500,
    );
    let other_id = books
        .persist_bank_activity_batch(std::slice::from_ref(&other_activity))
        .expect("persist other activity")[0]
        .activity_id;
    assert!(
        books
            .confirm_bank_match(&BankMatchWrite {
                bank_activity_id: other_id,
                transaction_id,
                entry_id,
                match_level: "likely".to_string(),
                score: 75,
                reasons: vec!["amountExact".to_string()],
            })
            .is_err(),
        "one Business Bank entry cannot be matched to a second activity"
    );

    drop(books);
    remove_sqlite_artifacts(&path);
}

#[test]
fn reconciliation_finalisation_is_exact_zero_audited_and_idempotent() {
    let (books, path) = test_books("reconcile");
    let source = activity(
        "reconcile-row",
        '2',
        Some("csv:GBP:bank-main:T-reconcile-row"),
        -2_500,
    );
    let activity_id = books
        .persist_bank_activity_batch(std::slice::from_ref(&source))
        .expect("persist activity")[0]
        .activity_id;
    let (transaction_id, entry_id) =
        post_bank_transaction(&books, "reconcile-ledger", -2_500);
    let _ = confirm_match_for(&books, activity_id, transaction_id, entry_id);

    let write = BankReconciliationWrite {
        statement_id: "statement-2026-09".to_string(),
        statement_date: "2026-09-11".to_string(),
        opening_balance_minor: 50_000,
        ending_balance_minor: 47_500,
        entries: vec![BankReconciliationEntryWrite {
            transaction_id,
            entry_id,
            signed_amount_minor: -2_500,
        }],
    };
    let outcome = books
        .finalize_bank_reconciliation(&write)
        .expect("finalize reconciliation");
    assert!(matches!(
        outcome,
        BankReconciliationPersistOutcome::Finalized(_)
    ));
    let transaction = books.transaction(transaction_id).expect("transaction");
    assert_eq!(
        transaction
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
            .expect("bank entry")
            .status,
        "reconciled"
    );
    let audit = books.audit_status_changes().expect("audit");
    assert!(audit.iter().any(|row| {
        row.entity_id == entry_id.to_string()
            && row.before.as_deref().is_some_and(|value| value.contains("cleared"))
            && row.after.as_deref().is_some_and(|value| value.contains("reconciled"))
    }));

    let repeated = books
        .finalize_bank_reconciliation(&write)
        .expect("idempotent finalization");
    assert!(matches!(
        repeated,
        BankReconciliationPersistOutcome::AlreadyFinalized(_)
    ));

    let mut conflicting = write.clone();
    conflicting.ending_balance_minor = 47_499;
    assert!(
        books.finalize_bank_reconciliation(&conflicting).is_err(),
        "statement identity cannot be reused with different content"
    );

    drop(books);
    remove_sqlite_artifacts(&path);
}

#[test]
fn failed_reconciliation_leaves_cleared_state_and_no_header() {
    let (books, path) = test_books("reconcile-fail");
    let source = activity(
        "reconcile-fail-row",
        '3',
        Some("csv:GBP:bank-main:T-reconcile-fail-row"),
        1_000,
    );
    let activity_id = books
        .persist_bank_activity_batch(std::slice::from_ref(&source))
        .expect("persist activity")[0]
        .activity_id;
    let (transaction_id, entry_id) =
        post_bank_transaction(&books, "reconcile-fail-ledger", 1_000);
    let _ = confirm_match_for(&books, activity_id, transaction_id, entry_id);

    let invalid = BankReconciliationWrite {
        statement_id: "statement-fail".to_string(),
        statement_date: "2026-09-11".to_string(),
        opening_balance_minor: 10_000,
        ending_balance_minor: 10_999,
        entries: vec![BankReconciliationEntryWrite {
            transaction_id,
            entry_id,
            signed_amount_minor: 1_000,
        }],
    };
    assert!(books.finalize_bank_reconciliation(&invalid).is_err());
    assert!(
        books
            .bank_reconciliation("statement-fail")
            .expect("lookup failed reconciliation")
            .is_none()
    );
    let transaction = books.transaction(transaction_id).expect("transaction");
    assert_eq!(
        transaction
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
            .expect("bank entry")
            .status,
        "cleared",
        "failed finalisation must not advance clearance"
    );

    drop(books);
    remove_sqlite_artifacts(&path);
}
