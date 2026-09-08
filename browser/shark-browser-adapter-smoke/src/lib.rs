//! SBC-1F browser application-adapter contract smoke.
//!
//! This crate is deliberately dependency-free and storage-neutral. It defines
//! only the application-operation seam a future browser implementation must
//! satisfy. It does not implement accounting rules, SQLite, OPFS, encryption,
//! tax logic, UI, OCR, AI, or MTD filing.
//!
//! The production native implementation remains `shark-foundation` behind the
//! Shark facade. Browser parity is explicitly NOT claimed at SBC-1F.

pub const SHARK_FACADE_API_VERSION: u32 = 1;
pub const BROWSER_ADAPTER_CONTRACT_VERSION: u32 = 1;
pub const BROWSER_PARITY_CLAIMED: bool = false;

pub type AdapterResult<T> = Result<T, AdapterError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterError {
    pub code: AdapterErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdapterErrorCode {
    InvalidInput,
    Validation,
    NotFound,
    Storage,
    Io,
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBooksRequest {
    pub books_id: String,
    pub company_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenBooksRequest {
    pub books_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooksMetadata {
    pub books_id: String,
    pub company_slug: String,
    pub company_name: String,
    pub database_schema_version: i64,
    pub books_format_version: u32,
    pub application_schema_version: u32,
    pub facade_api_version: u32,
    pub crate_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationMetadata {
    pub database_schema_version: i64,
    pub expected_database_schema_version: i64,
    pub books_format_version: u32,
    pub application_schema_version: u32,
    pub migration_required: bool,
    pub encrypted_native_required: bool,
    pub backup_before_existing_open_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTransactionRequest {
    pub description: String,
    pub date: String,
    pub currency_code: String,
    pub reference: Option<String>,
    pub metadata: Option<String>,
    pub lines: Vec<PostingLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingLine {
    pub account_code: String,
    pub direction: Direction,
    pub amount_minor: i64,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostOutcome {
    Created(i64),
    Skipped(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryView {
    pub id: i64,
    pub account_code: String,
    pub direction: String,
    pub amount_minor: i64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionView {
    pub id: i64,
    pub description: String,
    pub reference: Option<String>,
    pub currency: String,
    pub date: String,
    pub entries: Vec<EntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceLine {
    pub code: String,
    pub account_type: String,
    pub debit_total: i64,
    pub credit_total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialBalance {
    pub accounts: Vec<BalanceLine>,
    pub total_debits: i64,
    pub total_credits: i64,
    pub balanced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub before_count: i64,
    pub after_count: i64,
    pub imported_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusChangeResult {
    pub prior_status: String,
    pub new_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentView {
    pub id: i64,
    pub uri: String,
    pub hash: Option<String>,
    pub original_filename: Option<String>,
    pub document_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditView {
    pub actor: String,
    pub entity: String,
    pub entity_id: String,
    pub action: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Browser-side source object. Bytes are supplied by the browser host rather
/// than by a native filesystem path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// High-level application-operation seam for a future browser implementation.
///
/// Important: implementations must preserve Shark accounting semantics by
/// reusing the shared accounting/domain layer. This interface does not grant
/// permission to reimplement balancing, validation, tax, or posting rules in
/// browser UI/persistence code.
pub trait BrowserApplicationAdapter {
    fn create_books(&mut self, request: CreateBooksRequest) -> AdapterResult<BooksMetadata>;
    fn open_books(&mut self, request: OpenBooksRequest) -> AdapterResult<BooksMetadata>;
    fn verify_books(&self) -> AdapterResult<i64>;
    fn metadata(&self) -> AdapterResult<BooksMetadata>;
    fn migration_metadata(&self) -> AdapterResult<MigrationMetadata>;

    fn create_account(
        &mut self,
        code: String,
        name: String,
        account_type: String,
    ) -> AdapterResult<()>;

    fn post_transaction(&mut self, request: PostTransactionRequest) -> AdapterResult<PostOutcome>;
    fn count_transactions(&self) -> AdapterResult<i64>;
    fn find_by_reference(&self, reference: String) -> AdapterResult<Vec<TransactionView>>;
    fn transaction(&self, transaction_id: i64) -> AdapterResult<TransactionView>;
    fn trial_balance(&self) -> AdapterResult<TrialBalance>;

    fn import_ofx(
        &mut self,
        source: BrowserFile,
        bank_account: String,
        suspense_account: String,
    ) -> AdapterResult<ImportSummary>;

    fn reconcile_entry(
        &mut self,
        transaction_id: i64,
        entry_id: i64,
    ) -> AdapterResult<StatusChangeResult>;

    fn audit_status_changes(&self) -> AdapterResult<Vec<AuditView>>;

    fn attach_document(
        &mut self,
        transaction_id: i64,
        source: BrowserFile,
        document_type: String,
    ) -> AdapterResult<AttachmentView>;

    fn attachments(&self, transaction_id: i64) -> AdapterResult<Vec<AttachmentView>>;
}

/// Marker used by the SBC-1F gate to make the deferred storage decision explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPersistenceCandidate {
    SqliteWasmOpfs,
}

/// SBC-1F deliberately selects only the candidate family, not production parity.
pub const BROWSER_PERSISTENCE_CANDIDATE: BrowserPersistenceCandidate =
    BrowserPersistenceCandidate::SqliteWasmOpfs;
