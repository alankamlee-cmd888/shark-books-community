use shark_browser_adapter_smoke::{
    AdapterError, AdapterErrorCode, AdapterResult, AttachmentView, AuditView,
    BROWSER_ADAPTER_CONTRACT_VERSION, BROWSER_PARITY_CLAIMED, BROWSER_PERSISTENCE_CANDIDATE,
    BalanceLine, BooksMetadata, BrowserApplicationAdapter, BrowserFile, BrowserPersistenceCandidate,
    CreateBooksRequest, Direction, EntryView, ImportSummary, MigrationMetadata, OpenBooksRequest,
    PostOutcome, PostTransactionRequest, PostingLine, SHARK_FACADE_API_VERSION, StatusChangeResult,
    TransactionView, TrialBalance,
};

struct TypeOnlyBrowserAdapter;

fn unsupported<T>() -> AdapterResult<T> {
    Err(AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "SBC-1F type-only adapter; implementation deferred".to_string(),
    })
}

impl BrowserApplicationAdapter for TypeOnlyBrowserAdapter {
    fn create_books(&mut self, _request: CreateBooksRequest) -> AdapterResult<BooksMetadata> { unsupported() }
    fn open_books(&mut self, _request: OpenBooksRequest) -> AdapterResult<BooksMetadata> { unsupported() }
    fn verify_books(&self) -> AdapterResult<i64> { unsupported() }
    fn metadata(&self) -> AdapterResult<BooksMetadata> { unsupported() }
    fn migration_metadata(&self) -> AdapterResult<MigrationMetadata> { unsupported() }
    fn create_account(&mut self, _code: String, _name: String, _account_type: String) -> AdapterResult<()> { unsupported() }
    fn post_transaction(&mut self, _request: PostTransactionRequest) -> AdapterResult<PostOutcome> { unsupported() }
    fn count_transactions(&self) -> AdapterResult<i64> { unsupported() }
    fn find_by_reference(&self, _reference: String) -> AdapterResult<Vec<TransactionView>> { unsupported() }
    fn transaction(&self, _transaction_id: i64) -> AdapterResult<TransactionView> { unsupported() }
    fn trial_balance(&self) -> AdapterResult<TrialBalance> { unsupported() }
    fn import_ofx(&mut self, _source: BrowserFile, _bank_account: String, _suspense_account: String) -> AdapterResult<ImportSummary> { unsupported() }
    fn reconcile_entry(&mut self, _transaction_id: i64, _entry_id: i64) -> AdapterResult<StatusChangeResult> { unsupported() }
    fn audit_status_changes(&self) -> AdapterResult<Vec<AuditView>> { unsupported() }
    fn attach_document(&mut self, _transaction_id: i64, _source: BrowserFile, _document_type: String) -> AdapterResult<AttachmentView> { unsupported() }
    fn attachments(&self, _transaction_id: i64) -> AdapterResult<Vec<AttachmentView>> { unsupported() }
}

#[test]
fn contract_is_versioned_and_parity_is_not_claimed() {
    assert_eq!(SHARK_FACADE_API_VERSION, 1);
    assert_eq!(BROWSER_ADAPTER_CONTRACT_VERSION, 1);
    assert!(!BROWSER_PARITY_CLAIMED);
    assert_eq!(BROWSER_PERSISTENCE_CANDIDATE, BrowserPersistenceCandidate::SqliteWasmOpfs);
}

#[test]
fn adapter_trait_is_implementable_without_native_persistence_types() {
    fn accepts_adapter(_adapter: &dyn BrowserApplicationAdapter) {}
    let adapter = TypeOnlyBrowserAdapter;
    accepts_adapter(&adapter);
}

#[test]
fn transport_shapes_cover_current_facade_semantics_without_rules() {
    let request = PostTransactionRequest {
        description: "Smoke".to_string(),
        date: "2026-09-08".to_string(),
        currency_code: "GBP".to_string(),
        reference: Some("SBC1F".to_string()),
        metadata: None,
        lines: vec![
            PostingLine { account_code: "1000".to_string(), direction: Direction::Debit, amount_minor: 100, memo: None },
            PostingLine { account_code: "2000".to_string(), direction: Direction::Credit, amount_minor: 100, memo: None },
        ],
    };
    assert_eq!(request.lines.len(), 2);

    let transaction = TransactionView {
        id: 1,
        description: "Smoke".to_string(),
        reference: Some("SBC1F".to_string()),
        currency: "GBP".to_string(),
        date: "2026-09-08".to_string(),
        entries: vec![EntryView { id: 1, account_code: "1000".to_string(), direction: "debit".to_string(), amount_minor: 100, status: "uncleared".to_string() }],
    };
    assert_eq!(transaction.currency, "GBP");

    let trial = TrialBalance {
        accounts: vec![BalanceLine { code: "1000".to_string(), account_type: "asset".to_string(), debit_total: 100, credit_total: 0 }],
        total_debits: 100,
        total_credits: 100,
        balanced: true,
    };
    assert!(trial.balanced);

    let file = BrowserFile { name: "statement.ofx".to_string(), bytes: b"OFX".to_vec() };
    assert_eq!(file.name, "statement.ofx");
}
