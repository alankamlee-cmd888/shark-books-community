use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use shark_foundation::{
    Books, BooksId, BooksKey, Direction, PostOutcome, PostTransactionRequest, PostingLine,
    SecureKeyProvider, SiblingEncryptedBackup,
};

const RECEIPT_SHA256: &str = "b4eb560f73c6191e64f7d4a716da2a517b285ed1348a0ee4ea38f6ae9f5de167";

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
    fn load_key(&self, _books_id: &BooksId) -> shark_foundation::FoundationResult<BooksKey> {
        BooksKey::new(self.key.clone())
    }
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "shark-sbc1g-regression-{}-{nonce}",
        std::process::id()
    ))
}

fn created_id(outcome: PostOutcome) -> i64 {
    match outcome {
        PostOutcome::Created(id) => id,
        PostOutcome::Skipped(id) => panic!("regression posting unexpectedly skipped as {id}"),
    }
}

fn post(
    books: &Books,
    description: &str,
    reference: &str,
    metadata: Option<&str>,
    lines: Vec<PostingLine>,
) -> i64 {
    created_id(
        books
            .post(&PostTransactionRequest {
                description: description.to_string(),
                date: "2026-09-08".to_string(),
                currency_code: "GBP".to_string(),
                reference: Some(reference.to_string()),
                metadata: metadata.map(str::to_string),
                lines,
            })
            .expect("post regression transaction"),
    )
}

fn line(account_code: &str, direction: Direction, amount_minor: i64) -> PostingLine {
    PostingLine {
        account_code: account_code.to_string(),
        direction,
        amount_minor,
        memo: None,
    }
}

fn account_net(books: &Books, code: &str) -> i64 {
    let tb = books.trial_balance().expect("trial balance");
    let row = tb
        .accounts
        .iter()
        .find(|row| row.code == code)
        .unwrap_or_else(|| panic!("missing account {code}"));
    row.debit_total - row.credit_total
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn frozen_foundation_regression_scenario() {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("create regression temp directory");
    let db_path = root.join("books.sqlite");
    let books_id = BooksId::new("sbc1g-regression").expect("books id");
    let correct = TestKeyProvider::new("sbc1g-regression-correct-key");
    let wrong = TestKeyProvider::new("sbc1g-regression-wrong-key");
    let backup = SiblingEncryptedBackup;

    let books = Books::create_encrypted(
        &db_path,
        &books_id,
        "SBC-1G Regression Books",
        "SBC1G-REGRESSION",
        &correct,
    )
    .expect("create encrypted books");

    let header = fs::read(&db_path).expect("read encrypted database");
    assert!(
        !header.starts_with(b"SQLite format 3\0"),
        "production regression database must be encrypted"
    );

    for (code, name, kind) in [
        ("1000", "Business Bank", "asset"),
        ("3000", "Owner Capital", "equity"),
        ("3100", "Owner Drawings", "equity"),
        ("4000", "Sales Income", "revenue"),
        ("5000", "Rent Expense", "expense"),
        ("5100", "Supplies Expense", "expense"),
        ("9000", "Bank Import Suspense", "asset"),
    ] {
        books
            .create_account(code, name, kind)
            .unwrap_or_else(|error| panic!("create account {code}: {error}"));
    }

    post(
        &books,
        "Owner capital",
        "SBC1G-CAPITAL-001",
        None,
        vec![
            line("1000", Direction::Debit, 100_000),
            line("3000", Direction::Credit, 100_000),
        ],
    );
    post(
        &books,
        "Cash sale",
        "SBC1G-SALE-001",
        None,
        vec![
            line("1000", Direction::Debit, 12_550),
            line("4000", Direction::Credit, 12_550),
        ],
    );
    post(
        &books,
        "Rent",
        "SBC1G-RENT-001",
        None,
        vec![
            line("5000", Direction::Debit, 30_000),
            line("1000", Direction::Credit, 30_000),
        ],
    );
    let supplies_id = post(
        &books,
        "Supplies",
        "SBC1G-SUPPLIES-001",
        None,
        vec![
            line("5100", Direction::Debit, 4_723),
            line("1000", Direction::Credit, 4_723),
        ],
    );
    post(
        &books,
        "Owner drawing",
        "SBC1G-DRAWING-001",
        None,
        vec![
            line("3100", Direction::Debit, 8_000),
            line("1000", Direction::Credit, 8_000),
        ],
    );

    assert_eq!(books.count_transactions().expect("transaction count"), 5);
    let golden = books.trial_balance().expect("golden trial balance");
    assert!(golden.balanced);
    assert_eq!(golden.total_debits, 155_273);
    assert_eq!(golden.total_credits, 155_273);
    let bank = golden
        .accounts
        .iter()
        .find(|row| row.code == "1000")
        .expect("bank account in trial balance");
    assert_eq!(bank.debit_total, 112_550);
    assert_eq!(bank.credit_total, 42_723);

    let before_unbalanced = books.count_transactions().expect("count before invalid post");
    let invalid = books.post(&PostTransactionRequest {
        description: "Unbalanced regression".to_string(),
        date: "2026-09-08".to_string(),
        currency_code: "GBP".to_string(),
        reference: Some("SBC1G-UNBALANCED-001".to_string()),
        metadata: None,
        lines: vec![
            line("1000", Direction::Debit, 1_000),
            line("4000", Direction::Credit, 999),
        ],
    });
    assert!(invalid.is_err(), "unbalanced posting must be rejected");
    assert_eq!(
        books.count_transactions().expect("count after invalid post"),
        before_unbalanced,
        "rejected posting must not mutate the ledger"
    );

    drop(books);

    let wrong_open = Books::open_encrypted(
        &db_path,
        &books_id,
        "SBC1G-REGRESSION",
        &wrong,
        &backup,
    );
    assert!(wrong_open.is_err(), "wrong encryption key must fail closed");

    let books = Books::open_encrypted(
        &db_path,
        &books_id,
        "SBC1G-REGRESSION",
        &correct,
        &backup,
    )
    .expect("hard reopen encrypted books");
    assert_eq!(books.count_transactions().expect("reopen count"), 5);
    let reopened = books.trial_balance().expect("reopen trial balance");
    assert!(reopened.balanced);
    assert_eq!(reopened.total_debits, 155_273);
    assert_eq!(reopened.total_credits, 155_273);

    let ofx = fixture("gbp_statement.ofx");
    let first_import = books
        .import_ofx_summary(&ofx, "1000", "9000")
        .expect("first OFX import");
    assert_eq!(first_import.before_count, 5);
    assert_eq!(first_import.imported_count, 2);
    assert_eq!(first_import.after_count, 7);

    let duplicate_import = books
        .import_ofx_summary(&ofx, "1000", "9000")
        .expect("duplicate OFX import");
    assert_eq!(duplicate_import.before_count, 7);
    assert_eq!(duplicate_import.imported_count, 0);
    assert_eq!(duplicate_import.after_count, 7);
    assert_eq!(
        books
            .find_by_reference("ofx:GBP:SBC0BANK001:SBC0B-IN-001")
            .expect("find imported income")
            .len(),
        1
    );
    assert_eq!(
        books
            .find_by_reference("ofx:GBP:SBC0BANK001:SBC0B-OUT-001")
            .expect("find imported expense")
            .len(),
        1
    );
    let after_ofx = books.trial_balance().expect("post-OFX trial balance");
    assert!(after_ofx.balanced);
    assert_eq!(after_ofx.total_debits, 161_532);
    assert_eq!(after_ofx.total_credits, 161_532);

    let bank_before_reversal = account_net(&books, "1000");
    let supplies_before_reversal = account_net(&books, "5100");
    post(
        &books,
        "Erroneous supplies",
        "SBC1G-ERRONEOUS-001",
        None,
        vec![
            line("5100", Direction::Debit, 2_000),
            line("1000", Direction::Credit, 2_000),
        ],
    );
    post(
        &books,
        "Reverse erroneous supplies",
        "SBC1G-REVERSAL-001",
        Some("reverses:SBC1G-ERRONEOUS-001"),
        vec![
            line("1000", Direction::Debit, 2_000),
            line("5100", Direction::Credit, 2_000),
        ],
    );
    assert_eq!(account_net(&books, "1000"), bank_before_reversal);
    assert_eq!(account_net(&books, "5100"), supplies_before_reversal);
    assert_eq!(
        books
            .find_by_reference("SBC1G-ERRONEOUS-001")
            .expect("find original erroneous posting")
            .len(),
        1,
        "correction must retain the original transaction"
    );
    assert_eq!(
        books
            .find_by_reference("SBC1G-REVERSAL-001")
            .expect("find reversal")
            .len(),
        1
    );

    let supplies = books.transaction(supplies_id).expect("load supplies transaction");
    let entry_id = supplies.entries.first().expect("supplies entry").id;
    let status = books
        .reconcile_entry(supplies_id, entry_id)
        .expect("reconcile supplies entry");
    assert_eq!(status.new_status, "reconciled");
    let audit = books.audit_status_changes().expect("audit status changes");
    assert!(audit.iter().any(|row| {
        row.actor == "SBC1G-REGRESSION"
            && row.action == "status_change"
            && row.before.as_deref().unwrap_or_default().contains("uncleared")
            && row.after.as_deref().unwrap_or_default().contains("reconciled")
    }));

    let receipt = fixture("sample_receipt.txt");
    let attachment = books
        .attach_document(supplies_id, &receipt, "receipt")
        .expect("attach regression receipt");
    assert_eq!(attachment.hash.as_deref(), Some(RECEIPT_SHA256));
    assert_eq!(attachment.original_filename.as_deref(), Some("sample_receipt.txt"));
    assert!(attachment.uri.starts_with("attachments/"));

    drop(books);
    let books = Books::open_encrypted(
        &db_path,
        &books_id,
        "SBC1G-REGRESSION",
        &correct,
        &backup,
    )
    .expect("final reopen encrypted books");
    let attachments = books.attachments(supplies_id).expect("list persisted attachments");
    assert!(attachments.iter().any(|row| {
        row.hash.as_deref() == Some(RECEIPT_SHA256)
            && row.original_filename.as_deref() == Some("sample_receipt.txt")
    }));
    assert!(books.trial_balance().expect("final trial balance").balanced);

    drop(books);
    fs::remove_dir_all(root).expect("clean regression temp directory");
}
