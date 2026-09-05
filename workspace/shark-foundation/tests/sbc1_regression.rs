use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use shark_foundation::{
    Books, BooksId, BooksKey, Direction, FoundationResult, PostOutcome, PostTransactionRequest,
    PostingLine, SecureKeyProvider, SiblingEncryptedBackup,
};

struct RegressionKeyProvider(&'static str);

impl SecureKeyProvider for RegressionKeyProvider {
    fn load_key(&self, _books_id: &BooksId) -> FoundationResult<BooksKey> {
        BooksKey::new(self.0.to_string())
    }
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "shark-sbc1g-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create regression root");
    root
}

fn create_books(root: &Path, id: &str) -> (Books, BooksId, RegressionKeyProvider) {
    let books_id = BooksId::new(id).expect("valid books id");
    let provider = RegressionKeyProvider("sbc1g-regression-key-material");
    let books = Books::create_encrypted(
        &root.join("books.sqlite"),
        &books_id,
        "SBC-1G Regression Books",
        "sbc1g-ci",
        &provider,
    )
    .expect("create encrypted regression books");
    (books, books_id, provider)
}

fn create_core_accounts(books: &Books) {
    books
        .create_account("1000", "Bank", "asset")
        .expect("create bank account");
    books
        .create_account("5000", "Expenses", "expense")
        .expect("create expense account");
    books
        .create_account("9999", "Import Suspense", "expense")
        .expect("create suspense account");
}

fn gbp_post(reference: &str, debit: &str, credit: &str, amount_minor: i64) -> PostTransactionRequest {
    PostTransactionRequest {
        description: format!("SBC-1G {reference}"),
        date: "2026-09-05".to_string(),
        currency_code: "GBP".to_string(),
        reference: Some(reference.to_string()),
        metadata: Some(r#"{"fixture":"sbc1g"}"#.to_string()),
        lines: vec![
            PostingLine {
                account_code: debit.to_string(),
                direction: Direction::Debit,
                amount_minor,
                memo: None,
            },
            PostingLine {
                account_code: credit.to_string(),
                direction: Direction::Credit,
                amount_minor,
                memo: None,
            },
        ],
    }
}

#[test]
fn gbp_encryption_and_hard_reopen_regression() {
    let root = temp_root("gbp-reopen");
    let db_path = root.join("books.sqlite");
    let (books, books_id, provider) = create_books(&root, "sbc1g-gbp-reopen");
    create_core_accounts(&books);

    let outcome = books
        .post(&gbp_post("SBC1G-GBP-001", "5000", "1000", 12_345))
        .expect("post GBP fixture");
    assert!(matches!(outcome, PostOutcome::Created(_)));
    drop(books);

    let bytes = fs::read(&db_path).expect("read encrypted books file");
    assert!(
        !bytes.starts_with(b"SQLite format 3\0"),
        "production books must remain encrypted at rest"
    );

    let reopened = Books::open_encrypted(
        &db_path,
        &books_id,
        "sbc1g-ci",
        &provider,
        &SiblingEncryptedBackup,
    )
    .expect("hard reopen encrypted books");
    let rows = reopened
        .find_by_reference("SBC1G-GBP-001")
        .expect("find GBP fixture after reopen");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].currency, "GBP");
    assert_eq!(rows[0].entries.len(), 2);
    assert_eq!(rows[0].entries[0].amount_minor, 12_345);
    assert_eq!(rows[0].entries[1].amount_minor, 12_345);
    assert!(reopened.trial_balance().expect("trial balance").balanced);

    drop(reopened);
    fs::remove_dir_all(root).expect("clean regression root");
}

#[test]
fn duplicate_ofx_is_idempotently_skipped() {
    let root = temp_root("duplicate-ofx");
    let (books, _books_id, _provider) = create_books(&root, "sbc1g-ofx");
    create_core_accounts(&books);

    let fixture = root.join("duplicate.ofx");
    fs::write(
        &fixture,
        include_str!("fixtures/sbc1g_duplicate_gbp.ofx"),
    )
    .expect("write OFX fixture");

    let first = books
        .import_ofx_summary(&fixture, "1000", "9999")
        .expect("first OFX import");
    assert_eq!(first.before_count, 0);
    assert_eq!(first.after_count, 1);
    assert_eq!(first.imported_count, 1);

    let second = books
        .import_ofx_summary(&fixture, "1000", "9999")
        .expect("duplicate OFX import");
    assert_eq!(second.before_count, 1);
    assert_eq!(second.after_count, 1);
    assert_eq!(second.imported_count, 0, "duplicate FITID must not repost");

    let imported = books
        .find_by_reference("ofx:GBP:00012345:SBC1G-DUP-001")
        .expect("find imported OFX transaction");
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].currency, "GBP");

    drop(books);
    fs::remove_dir_all(root).expect("clean regression root");
}

#[test]
fn append_only_reversal_and_audit_regression() {
    let root = temp_root("reversal-audit");
    let (books, _books_id, _provider) = create_books(&root, "sbc1g-reversal");
    create_core_accounts(&books);

    let original_id = match books
        .post(&gbp_post("SBC1G-ORIGINAL", "5000", "1000", 4_250))
        .expect("post original")
    {
        PostOutcome::Created(id) => id,
        PostOutcome::Skipped(_) => panic!("original must be newly created"),
    };
    let reversal_id = match books
        .post(&gbp_post("SBC1G-REVERSAL", "1000", "5000", 4_250))
        .expect("post reversing entry")
    {
        PostOutcome::Created(id) => id,
        PostOutcome::Skipped(_) => panic!("reversal must be newly created"),
    };
    assert_ne!(original_id, reversal_id, "reversal is append-only, not an edit");
    assert_eq!(books.count_transactions().expect("transaction count"), 2);

    let original = books.transaction(original_id).expect("original transaction");
    let reversal = books.transaction(reversal_id).expect("reversal transaction");
    assert_eq!(original.currency, "GBP");
    assert_eq!(reversal.currency, "GBP");
    assert_eq!(original.entries.len(), 2);
    assert_eq!(reversal.entries.len(), 2);

    for original_entry in &original.entries {
        let opposite = reversal
            .entries
            .iter()
            .find(|candidate| candidate.account_code == original_entry.account_code)
            .expect("matching reversal account");
        assert_eq!(opposite.amount_minor, original_entry.amount_minor);
        assert_ne!(opposite.direction, original_entry.direction);
    }

    let entry_id = original.entries[0].id;
    let change = books
        .reconcile_entry(original_id, entry_id)
        .expect("reconcile original entry");
    assert_eq!(change.prior_status, "uncleared");
    assert_eq!(change.new_status, "reconciled");

    let audit = books.audit_status_changes().expect("read audit trail");
    let event = audit
        .iter()
        .find(|row| row.entity_id == entry_id.to_string())
        .expect("status change audit event");
    assert_eq!(event.actor, "sbc1g-ci");
    assert_eq!(event.entity, "entry");
    assert_eq!(event.action, "status_change");
    assert!(event.before.as_deref().unwrap_or_default().contains("uncleared"));
    assert!(event.after.as_deref().unwrap_or_default().contains("reconciled"));

    drop(books);
    fs::remove_dir_all(root).expect("clean regression root");
}

#[test]
fn document_attachment_round_trip_regression() {
    let root = temp_root("documents");
    let (books, _books_id, _provider) = create_books(&root, "sbc1g-documents");
    create_core_accounts(&books);

    let transaction_id = match books
        .post(&gbp_post("SBC1G-DOC-001", "5000", "1000", 999))
        .expect("post document transaction")
    {
        PostOutcome::Created(id) => id,
        PostOutcome::Skipped(_) => panic!("document transaction must be newly created"),
    };

    let source = root.join("receipt.txt");
    fs::write(&source, b"SBC-1G deterministic receipt fixture\n").expect("write receipt fixture");
    let attached = books
        .attach_document(transaction_id, &source, "receipt")
        .expect("attach document");
    assert_eq!(attached.document_type, "receipt");
    assert_eq!(attached.original_filename.as_deref(), Some("receipt.txt"));
    assert!(attached.hash.as_deref().is_some_and(|value| value.len() == 64));

    let rows = books.attachments(transaction_id).expect("list attachments");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], attached);
    let stored = root.join(&attached.uri);
    assert_eq!(
        fs::read(stored).expect("read content-addressed attachment"),
        b"SBC-1G deterministic receipt fixture\n"
    );

    drop(books);
    fs::remove_dir_all(root).expect("clean regression root");
}
