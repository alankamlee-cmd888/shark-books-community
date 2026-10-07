//! SBC-7B1 Batch B typed owner bridge for Contacts, Settings and factual Reports.
//!
//! This module exposes only bounded owner-safe DTOs. It has no raw database,
//! key/passphrase, filesystem-path, accounting-rule, tax or network authority.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{
    Books, ContactPersistOutcome, ContactView, ContactWrite, CreditNoteDraftWrite,
    CreditNoteLineWrite, CreditNoteMutationOutcome, CreditNoteState, CreditNoteView,
    FoundationError, FoundationErrorCode, InvoiceDraftWrite, InvoiceLineWrite,
    InvoiceMutationOutcome, InvoiceState, InvoiceView, IssuedQuoteSnapshotView, QuoteDraftWrite,
    QuoteKind, QuoteLineView, QuoteLineWrite, QuoteMutationOutcome, QuoteMutationView, QuoteState,
    QuoteView, TrialBalance,
};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_dialog::DialogExt;

use super::owner_commercial_renderer::{
    OwnerCommercialPdfStoreReceipt, OwnerCommercialPdfStoreRequest, store_commercial_pdf,
};
use super::owner_documents_ocr::NativeDocumentRootRegistry;
use super::{open_books_impl, OpenBooksRequest};

const OWNER_SUPPORTING_DATA_BRIDGE_VERSION: u32 = 1;
const OWNER_CONTACT_LIST_MAX: i64 = 200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerSupportingDataError {
    code: &'static str,
    message: String,
}

impl OwnerSupportingDataError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalidInput",
            message: message.into(),
        }
    }

    fn foundation(error: FoundationError) -> Self {
        let code = match error.code {
            FoundationErrorCode::NotFound => "notFound",
            FoundationErrorCode::InvalidInput | FoundationErrorCode::Validation => "invalidInput",
            _ => "booksOperationFailed",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }

    fn storage(message: impl Into<String>) -> Self {
        Self {
            code: "storageRootFailed",
            message: message.into(),
        }
    }

    fn unsupported_platform(message: impl Into<String>) -> Self {
        Self {
            code: "unsupportedPlatform",
            message: message.into(),
        }
    }
}

type OwnerSupportingDataResult<T> = Result<T, OwnerSupportingDataError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerBooksRef {
    fn open(&self) -> OwnerSupportingDataResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerSupportingDataError::foundation)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerContactKind {
    Customer,
    Supplier,
}

impl OwnerContactKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Customer => "customer",
            Self::Supplier => "supplier",
        }
    }

    fn from_persisted(value: &str) -> OwnerSupportingDataResult<Self> {
        match value {
            "customer" => Ok(Self::Customer),
            "supplier" => Ok(Self::Supplier),
            _ => Err(OwnerSupportingDataError::invalid(
                "persisted contact has an unsupported kind",
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerContactsSaveRequest {
    books: OwnerBooksRef,
    contact_id: String,
    kind: OwnerContactKind,
    display_name: String,
    postal_address: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerContactsListRequest {
    books: OwnerBooksRef,
    kind: Option<OwnerContactKind>,
    limit: Option<u16>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum OwnerQuoteReadOperation {
    QuotesList,
    QuoteDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerQuoteReadRequest {
    books: OwnerBooksRef,
    operation: OwnerQuoteReadOperation,
    quote_id: Option<String>,
    kind: Option<QuoteKind>,
    state: Option<QuoteState>,
    limit: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum OwnerSupportingListRequest {
    Contacts(OwnerContactsListRequest),
    Quotes(OwnerQuoteReadRequest),
    Invoices(OwnerInvoiceReadRequest),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum OwnerQuoteMutationRequest {
    CreateDraft {
        books: OwnerBooksRef,
        quote_id: String,
        kind: QuoteKind,
        customer_id: String,
    },
    SetCustomer {
        books: OwnerBooksRef,
        quote_id: String,
        customer_id: String,
    },
    SaveLine {
        books: OwnerBooksRef,
        quote_id: String,
        line_id: String,
        description: String,
        quantity_subunits: i64,
        unit_price_pence: i64,
    },
    RemoveLine {
        books: OwnerBooksRef,
        quote_id: String,
        line_id: String,
    },
    Issue {
        books: OwnerBooksRef,
        quote_id: String,
        commercial_number: String,
    },
    Transition {
        books: OwnerBooksRef,
        quote_id: String,
        target: QuoteState,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum OwnerSupportingSaveRequest {
    Contact(OwnerContactsSaveRequest),
    Quote(OwnerQuoteMutationRequest),
    Invoice(OwnerInvoiceMutationRequest),
    CommercialPdf(OwnerCommercialPdfStoreRequest),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerSettingsBooksInfoRequest {
    books: OwnerBooksRef,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerSettingsStorageRootSelectRequest {
    books: OwnerBooksRef,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerReportSummaryRequest {
    books: OwnerBooksRef,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerContactView {
    bridge_version: u32,
    contact_id: String,
    kind: OwnerContactKind,
    display_name: String,
    postal_address: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    created_by: String,
    created_at: String,
    updated_by: String,
    updated_at: String,
}

impl TryFrom<ContactView> for OwnerContactView {
    type Error = OwnerSupportingDataError;

    fn try_from(value: ContactView) -> Result<Self, Self::Error> {
        Ok(Self {
            bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
            contact_id: value.contact_id,
            kind: OwnerContactKind::from_persisted(&value.kind)?,
            display_name: value.display_name,
            postal_address: value.postal_address,
            email: value.email,
            phone: value.phone,
            created_by: value.created_by,
            created_at: value.created_at,
            updated_by: value.updated_by,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum OwnerContactSaveOutcome {
    Created { contact: OwnerContactView },
    Updated { contact: OwnerContactView },
    AlreadyCurrent { contact: OwnerContactView },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerContactsListOutcome {
    bridge_version: u32,
    contacts: Vec<OwnerContactView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OwnerQuoteLineView {
    line_id: String,
    description: String,
    quantity_subunits: i64,
    unit_price_pence: i64,
    total_pence: i64,
    position: i64,
}

impl From<QuoteLineView> for OwnerQuoteLineView {
    fn from(value: QuoteLineView) -> Self {
        Self {
            line_id: value.line_id,
            description: value.description,
            quantity_subunits: value.quantity_subunits,
            unit_price_pence: value.unit_price_minor,
            total_pence: value.total_minor,
            position: value.position,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OwnerQuoteCustomerSnapshotView {
    customer_id: String,
    display_name: String,
    postal_address: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

impl From<shark_foundation::QuoteCustomerSnapshotView> for OwnerQuoteCustomerSnapshotView {
    fn from(value: shark_foundation::QuoteCustomerSnapshotView) -> Self {
        Self {
            customer_id: value.customer_id,
            display_name: value.display_name,
            postal_address: value.postal_address,
            email: value.email,
            phone: value.phone,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OwnerIssuedQuoteSnapshotView {
    commercial_number: String,
    kind: QuoteKind,
    customer: OwnerQuoteCustomerSnapshotView,
    lines: Vec<OwnerQuoteLineView>,
    total_pence: i64,
    issued_by: String,
    issued_at: String,
}

impl From<IssuedQuoteSnapshotView> for OwnerIssuedQuoteSnapshotView {
    fn from(value: IssuedQuoteSnapshotView) -> Self {
        Self {
            commercial_number: value.commercial_number,
            kind: value.kind,
            customer: value.customer.into(),
            lines: value.lines.into_iter().map(Into::into).collect(),
            total_pence: value.total_minor,
            issued_by: value.issued_by,
            issued_at: value.issued_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OwnerQuoteView {
    bridge_version: u32,
    quote_id: String,
    kind: QuoteKind,
    state: QuoteState,
    customer: OwnerQuoteCustomerSnapshotView,
    lines: Vec<OwnerQuoteLineView>,
    total_pence: i64,
    issued_snapshot: Option<OwnerIssuedQuoteSnapshotView>,
    conversion_eligible: bool,
    created_by: String,
    created_at: String,
    updated_by: String,
    updated_at: String,
}

impl From<QuoteView> for OwnerQuoteView {
    fn from(value: QuoteView) -> Self {
        Self {
            bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
            quote_id: value.quote_id,
            kind: value.kind,
            state: value.state,
            customer: value.customer.into(),
            lines: value.lines.into_iter().map(Into::into).collect(),
            total_pence: value.total_minor,
            issued_snapshot: value.issued_snapshot.map(Into::into),
            conversion_eligible: value.conversion_eligible,
            created_by: value.created_by,
            created_at: value.created_at,
            updated_by: value.updated_by,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OwnerQuoteMutationView {
    mutation_id: i64,
    quote_id: String,
    action: String,
    before: Option<OwnerQuoteView>,
    after: Option<OwnerQuoteView>,
    actor: String,
    occurred_at: String,
}

impl From<QuoteMutationView> for OwnerQuoteMutationView {
    fn from(value: QuoteMutationView) -> Self {
        Self {
            mutation_id: value.mutation_id,
            quote_id: value.quote_id,
            action: value.action,
            before: value.before.map(Into::into),
            after: value.after.map(Into::into),
            actor: value.actor,
            occurred_at: value.occurred_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerQuotesListOutcome {
    bridge_version: u32,
    quotes: Vec<OwnerQuoteView>,
    non_posting: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerQuoteDetailOutcome {
    bridge_version: u32,
    quote: OwnerQuoteView,
    history: Vec<OwnerQuoteMutationView>,
    non_posting: bool,
    invoice_created: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerQuoteMutationOutcome {
    bridge_version: u32,
    quote: OwnerQuoteView,
    mutation: OwnerQuoteMutationView,
    non_posting: bool,
    invoice_created: bool,
    requires_further_automatic_action: bool,
}

impl From<QuoteMutationOutcome> for OwnerQuoteMutationOutcome {
    fn from(value: QuoteMutationOutcome) -> Self {
        Self {
            bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
            quote: value.quote.into(),
            mutation: value.mutation.into(),
            non_posting: true,
            invoice_created: false,
            requires_further_automatic_action: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum OwnerSupportingListOutcome {
    Contacts(OwnerContactsListOutcome),
    Quotes(OwnerQuotesListOutcome),
    QuoteDetail(OwnerQuoteDetailOutcome),
    Invoices(OwnerInvoiceReadOutcome),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum OwnerSupportingSaveOutcome {
    Contact(OwnerContactSaveOutcome),
    Quote(OwnerQuoteMutationOutcome),
    Invoice(OwnerInvoiceMutationOutcome),
    CommercialPdf(OwnerCommercialPdfStoreReceipt),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerSettingsBooksInfo {
    bridge_version: u32,
    books_id: String,
    company_name: String,
    database_schema_version: i64,
    expected_database_schema_version: i64,
    books_format_version: u32,
    application_schema_version: u32,
    facade_api_version: u32,
    foundation_version: String,
    shell_version: String,
    migration_required: bool,
    production_encryption_required: bool,
    encrypted_native_required: bool,
    encrypted_native_session_active: bool,
    backup_before_existing_open_required: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerStorageRootScope {
    DeviceSession,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum OwnerStorageRootSelectOutcome {
    Registered {
        bridge_version: u32,
        storage_root_id: String,
        label: String,
        scope: OwnerStorageRootScope,
    },
    Cancelled {
        bridge_version: u32,
        scope: OwnerStorageRootScope,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReportSummary {
    bridge_version: u32,
    money_in_minor: i64,
    money_out_minor: i64,
    business_bank_balance_minor: Option<i64>,
    cash_balance_minor: Option<i64>,
    books_balanced: bool,
    currency: &'static str,
}

fn core_contact(write: &OwnerContactsSaveRequest) -> OwnerSupportingDataResult<ContactWrite> {
    let record_id = core::RecordId::new(write.contact_id.clone())
        .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?;
    let snapshot = match write.kind {
        OwnerContactKind::Customer => core::Customer::with_contact_details(
            record_id,
            write.display_name.clone(),
            write.postal_address.clone(),
            write.email.clone(),
            write.phone.clone(),
        )
        .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?
        .snapshot(),
        OwnerContactKind::Supplier => core::Supplier::with_contact_details(
            record_id,
            write.display_name.clone(),
            write.postal_address.clone(),
            write.email.clone(),
            write.phone.clone(),
        )
        .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?
        .snapshot(),
    };
    Ok(ContactWrite {
        contact_id: snapshot.id().as_str().to_string(),
        kind: write.kind.as_str().to_string(),
        display_name: snapshot.display_name().to_string(),
        postal_address: snapshot.postal_address().map(str::to_string),
        email: snapshot.email().map(str::to_string),
        phone: snapshot.phone().map(str::to_string),
    })
}

fn core_record_id(value: String) -> OwnerSupportingDataResult<String> {
    core::RecordId::new(value)
        .map(|record_id| record_id.as_str().to_string())
        .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))
}

fn core_quote_line(
    line_id: String,
    description: String,
    quantity_subunits: i64,
    unit_price_pence: i64,
) -> OwnerSupportingDataResult<QuoteLineWrite> {
    let line = core::CommercialLine::new(
        core::RecordId::new(line_id)
            .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?,
        description,
        quantity_subunits,
        unit_price_pence,
    )
    .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?;
    Ok(QuoteLineWrite {
        line_id: line.id().as_str().to_string(),
        description: line.description().to_string(),
        quantity_subunits: line.quantity_subunits(),
        unit_price_minor: line.unit_price_minor(),
    })
}

fn quote_mutation(
    request: OwnerQuoteMutationRequest,
) -> OwnerSupportingDataResult<OwnerQuoteMutationOutcome> {
    let outcome = match request {
        OwnerQuoteMutationRequest::CreateDraft {
            books,
            quote_id,
            kind,
            customer_id,
        } => {
            let quote_id = core_record_id(quote_id)?;
            let customer_id = core_record_id(customer_id)?;
            books
                .open()?
                .create_quote_draft(&QuoteDraftWrite {
                    quote_id,
                    kind,
                    customer_id,
                })
        }
        OwnerQuoteMutationRequest::SetCustomer {
            books,
            quote_id,
            customer_id,
        } => {
            let quote_id = core_record_id(quote_id)?;
            let customer_id = core_record_id(customer_id)?;
            books.open()?.set_quote_customer(&quote_id, &customer_id)
        }
        OwnerQuoteMutationRequest::SaveLine {
            books,
            quote_id,
            line_id,
            description,
            quantity_subunits,
            unit_price_pence,
        } => {
            let quote_id = core_record_id(quote_id)?;
            let line = core_quote_line(
                line_id,
                description,
                quantity_subunits,
                unit_price_pence,
            )?;
            books.open()?.save_quote_line(&quote_id, &line)
        }
        OwnerQuoteMutationRequest::RemoveLine {
            books,
            quote_id,
            line_id,
        } => {
            let quote_id = core_record_id(quote_id)?;
            let line_id = core_record_id(line_id)?;
            books.open()?.remove_quote_line(&quote_id, &line_id)
        }
        OwnerQuoteMutationRequest::Issue {
            books,
            quote_id,
            commercial_number,
        } => {
            let quote_id = core_record_id(quote_id)?;
            let commercial_number = core::CommercialNumber::new(commercial_number)
                .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?;
            books
                .open()?
                .issue_quote(&quote_id, commercial_number.as_str())
        }
        OwnerQuoteMutationRequest::Transition {
            books,
            quote_id,
            target,
        } => {
            let quote_id = core_record_id(quote_id)?;
            books.open()?.transition_quote(&quote_id, target)
        }
    }
    .map_err(OwnerSupportingDataError::foundation)?;
    Ok(outcome.into())
}

fn quote_read(
    request: OwnerQuoteReadRequest,
) -> OwnerSupportingDataResult<OwnerSupportingListOutcome> {
    let books = request.books.open()?;
    match request.operation {
        OwnerQuoteReadOperation::QuotesList => {
            if request.quote_id.is_some() {
                return Err(OwnerSupportingDataError::invalid(
                    "quote list does not accept a quote id",
                ));
            }
            let limit = i64::from(request.limit.unwrap_or(200));
            let quotes = books
                .quotes(request.kind, request.state, limit)
                .map_err(OwnerSupportingDataError::foundation)?
                .into_iter()
                .map(Into::into)
                .collect();
            Ok(OwnerSupportingListOutcome::Quotes(OwnerQuotesListOutcome {
                bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
                quotes,
                non_posting: true,
            }))
        }
        OwnerQuoteReadOperation::QuoteDetail => {
            if request.kind.is_some() || request.state.is_some() {
                return Err(OwnerSupportingDataError::invalid(
                    "quote detail does not accept list filters",
                ));
            }
            let quote_id = request.quote_id.ok_or_else(|| {
                OwnerSupportingDataError::invalid("quote detail requires a quote id")
            })?;
            let quote_id = core_record_id(quote_id)?;
            let quote = books
                .quote(&quote_id)
                .map_err(OwnerSupportingDataError::foundation)?;
            let history = books
                .quote_history(&quote_id, i64::from(request.limit.unwrap_or(200)))
                .map_err(OwnerSupportingDataError::foundation)?
                .into_iter()
                .map(Into::into)
                .collect();
            Ok(OwnerSupportingListOutcome::QuoteDetail(
                OwnerQuoteDetailOutcome {
                    bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
                    quote: quote.into(),
                    history,
                    non_posting: true,
                    invoice_created: false,
                },
            ))
        }
    }
}

fn save_contact_request(
    request: OwnerContactsSaveRequest,
) -> OwnerSupportingDataResult<OwnerContactSaveOutcome> {
    let write = core_contact(&request)?;
    let books = request.books.open()?;
    match books
        .save_contact(&write)
        .map_err(OwnerSupportingDataError::foundation)?
    {
        ContactPersistOutcome::Created(contact) => Ok(OwnerContactSaveOutcome::Created {
            contact: contact.try_into()?,
        }),
        ContactPersistOutcome::Updated(contact) => Ok(OwnerContactSaveOutcome::Updated {
            contact: contact.try_into()?,
        }),
        ContactPersistOutcome::AlreadyCurrent(contact) => {
            Ok(OwnerContactSaveOutcome::AlreadyCurrent {
                contact: contact.try_into()?,
            })
        }
    }
}

fn list_contacts_request(
    request: OwnerContactsListRequest,
) -> OwnerSupportingDataResult<OwnerContactsListOutcome> {
    let limit = i64::from(request.limit.unwrap_or(OWNER_CONTACT_LIST_MAX as u16));
    if !(1..=OWNER_CONTACT_LIST_MAX).contains(&limit) {
        return Err(OwnerSupportingDataError::invalid(
            "owner contact list limit must be between 1 and 200",
        ));
    }
    let books = request.books.open()?;
    let contacts = books
        .contacts(request.kind.map(OwnerContactKind::as_str), limit)
        .map_err(OwnerSupportingDataError::foundation)?
        .into_iter()
        .map(OwnerContactView::try_from)
        .collect::<OwnerSupportingDataResult<Vec<_>>>()?;
    Ok(OwnerContactsListOutcome {
        bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
        contacts,
    })
}

fn checked_to_i64(value: i128, label: &str) -> OwnerSupportingDataResult<i64> {
    i64::try_from(value).map_err(|_| {
        OwnerSupportingDataError::invalid(format!(
            "{label} cannot be represented as whole pence"
        ))
    })
}

fn checked_add(total: i128, contribution: i128, label: &str) -> OwnerSupportingDataResult<i128> {
    total.checked_add(contribution).ok_or_else(|| {
        OwnerSupportingDataError::invalid(format!("{label} arithmetic overflow"))
    })
}

fn account_net(debit: i64, credit: i64, debit_positive: bool) -> OwnerSupportingDataResult<i128> {
    let debit = i128::from(debit);
    let credit = i128::from(credit);
    if debit_positive {
        debit
            .checked_sub(credit)
            .ok_or_else(|| OwnerSupportingDataError::invalid("report arithmetic overflow"))
    } else {
        credit
            .checked_sub(debit)
            .ok_or_else(|| OwnerSupportingDataError::invalid("report arithmetic overflow"))
    }
}

fn report_from_trial_balance(trial: TrialBalance) -> OwnerSupportingDataResult<OwnerReportSummary> {
    let mut money_in = 0_i128;
    let mut money_out = 0_i128;
    let mut bank: Option<i128> = None;
    let mut cash: Option<i128> = None;

    for account in &trial.accounts {
        match account.account_type.as_str() {
            "revenue" => {
                money_in = checked_add(
                    money_in,
                    account_net(account.debit_total, account.credit_total, false)?,
                    "Money In",
                )?;
            }
            "expense" => {
                money_out = checked_add(
                    money_out,
                    account_net(account.debit_total, account.credit_total, true)?,
                    "Money Out",
                )?;
            }
            _ => {}
        }

        if account.code == "1000" {
            if account.account_type != "asset" || bank.is_some() {
                return Err(OwnerSupportingDataError::invalid(
                    "Business Bank trial-balance account is ambiguous or not an asset",
                ));
            }
            bank = Some(account_net(account.debit_total, account.credit_total, true)?);
        }
        if account.code == "1010" {
            if account.account_type != "asset" || cash.is_some() {
                return Err(OwnerSupportingDataError::invalid(
                    "Cash trial-balance account is ambiguous or not an asset",
                ));
            }
            cash = Some(account_net(account.debit_total, account.credit_total, true)?);
        }
    }

    Ok(OwnerReportSummary {
        bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
        money_in_minor: checked_to_i64(money_in, "Money In")?,
        money_out_minor: checked_to_i64(money_out, "Money Out")?,
        business_bank_balance_minor: bank
            .map(|value| checked_to_i64(value, "Business Bank balance"))
            .transpose()?,
        cash_balance_minor: cash
            .map(|value| checked_to_i64(value, "Cash balance"))
            .transpose()?,
        books_balanced: trial.balanced && trial.total_debits == trial.total_credits,
        currency: "GBP",
    })
}

fn register_storage_root_selection(
    roots: &NativeDocumentRootRegistry,
    selected: Option<PathBuf>,
) -> OwnerSupportingDataResult<OwnerStorageRootSelectOutcome> {
    let Some(path) = selected else {
        return Ok(OwnerStorageRootSelectOutcome::Cancelled {
            bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
            scope: OwnerStorageRootScope::DeviceSession,
        });
    };
    let storage_root_id = roots
        .register_session_root(path)
        .map_err(|error| OwnerSupportingDataError::storage(error.message()))?;
    Ok(OwnerStorageRootSelectOutcome::Registered {
        bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
        storage_root_id,
        label: "Document storage folder selected".to_string(),
        scope: OwnerStorageRootScope::DeviceSession,
    })
}

fn storage_root_mobile_unsupported_error() -> OwnerSupportingDataError {
    OwnerSupportingDataError::unsupported_platform(
        "folder selection is not supported on this mobile platform",
    )
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn select_storage_root(
    app: &tauri::AppHandle,
    roots: &NativeDocumentRootRegistry,
) -> OwnerSupportingDataResult<OwnerStorageRootSelectOutcome> {
    let selected = app.dialog().file().blocking_pick_folder();
    let selected = selected
        .map(|value| {
            value.into_path().map_err(|_| {
                OwnerSupportingDataError::storage(
                    "selected folder could not be resolved to a native directory",
                )
            })
        })
        .transpose()?;
    register_storage_root_selection(roots, selected)
}

#[cfg(any(target_os = "ios", target_os = "android"))]
fn select_storage_root(
    _app: &tauri::AppHandle,
    _roots: &NativeDocumentRootRegistry,
) -> OwnerSupportingDataResult<OwnerStorageRootSelectOutcome> {
    Err(storage_root_mobile_unsupported_error())
}

#[tauri::command]
pub(crate) fn owner_contacts_save(
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    request: OwnerSupportingSaveRequest,
) -> OwnerSupportingDataResult<OwnerSupportingSaveOutcome> {
    match request {
        OwnerSupportingSaveRequest::Invoice(request) => invoice_mutation(request).map(OwnerSupportingSaveOutcome::Invoice),
        OwnerSupportingSaveRequest::Contact(request) => save_contact_request(request)
            .map(OwnerSupportingSaveOutcome::Contact),
        OwnerSupportingSaveRequest::Quote(request) => {
            quote_mutation(request).map(OwnerSupportingSaveOutcome::Quote)
        }
        OwnerSupportingSaveRequest::CommercialPdf(request) => store_commercial_pdf(&roots, request)
            .map(OwnerSupportingSaveOutcome::CommercialPdf)
            .map_err(|error| OwnerSupportingDataError::storage(error.message())),
    }
}

#[tauri::command]
pub(crate) fn owner_contacts_list(
    request: OwnerSupportingListRequest,
) -> OwnerSupportingDataResult<OwnerSupportingListOutcome> {
    match request {
        OwnerSupportingListRequest::Contacts(request) => list_contacts_request(request)
            .map(OwnerSupportingListOutcome::Contacts),
        OwnerSupportingListRequest::Quotes(request) => quote_read(request),
        OwnerSupportingListRequest::Invoices(request) => invoice_read(request).map(OwnerSupportingListOutcome::Invoices),
    }
}

#[tauri::command]
pub(crate) fn owner_settings_books_info(
    request: OwnerSettingsBooksInfoRequest,
) -> OwnerSupportingDataResult<OwnerSettingsBooksInfo> {
    let books = request.books.open()?;
    let metadata = books.metadata().map_err(OwnerSupportingDataError::foundation)?;
    let migration = books
        .migration_metadata()
        .map_err(OwnerSupportingDataError::foundation)?;
    Ok(OwnerSettingsBooksInfo {
        bridge_version: OWNER_SUPPORTING_DATA_BRIDGE_VERSION,
        books_id: metadata.books_id.as_str().to_string(),
        company_name: metadata.company_name,
        database_schema_version: metadata.database_schema_version,
        expected_database_schema_version: migration.expected_database_schema_version,
        books_format_version: metadata.books_format_version,
        application_schema_version: metadata.application_schema_version,
        facade_api_version: metadata.facade_api_version,
        foundation_version: metadata.crate_version,
        shell_version: env!("CARGO_PKG_VERSION").to_string(),
        migration_required: migration.migration_required,
        production_encryption_required: shark_foundation::production_encryption_required(),
        encrypted_native_required: migration.encrypted_native_required,
        encrypted_native_session_active: true,
        backup_before_existing_open_required: migration.backup_before_existing_open_required,
    })
}

#[tauri::command]
pub(crate) async fn owner_settings_storage_root_select(
    app: tauri::AppHandle,
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    request: OwnerSettingsStorageRootSelectRequest,
) -> OwnerSupportingDataResult<OwnerStorageRootSelectOutcome> {
    let _books = request.books.open()?;
    select_storage_root(&app, &roots)
}

#[tauri::command]
pub(crate) fn owner_report_summary(
    request: OwnerReportSummaryRequest,
) -> OwnerSupportingDataResult<OwnerReportSummary> {
    let books = request.books.open()?;
    let trial = books
        .trial_balance()
        .map_err(OwnerSupportingDataError::foundation)?;
    report_from_trial_balance(trial)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shark_foundation::BalanceLine;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn books_ref() -> OwnerBooksRef {
        OwnerBooksRef {
            file_name: "owner-support.sqlite".into(),
            books_id: "owner-support".into(),
            actor: "local-owner".into(),
        }
    }

    #[test]
    fn contact_request_rejects_raw_authority_and_uses_core_validation() {
        let raw = serde_json::json!({
            "books": {
                "fileName": "owner-support.sqlite",
                "booksId": "owner-support",
                "actor": "local-owner"
            },
            "contactId": "customer-1",
            "kind": "customer",
            "displayName": "Customer",
            "databasePath": "C:/raw.sqlite"
        });
        assert!(serde_json::from_value::<OwnerContactsSaveRequest>(raw).is_err());

        let invalid = OwnerContactsSaveRequest {
            books: books_ref(),
            contact_id: " ".into(),
            kind: OwnerContactKind::Customer,
            display_name: "Customer".into(),
            postal_address: None,
            email: None,
            phone: None,
        };
        assert!(core_contact(&invalid).is_err());

        let valid = OwnerContactsSaveRequest {
            books: books_ref(),
            contact_id: "customer-1".into(),
            kind: OwnerContactKind::Customer,
            display_name: " Customer Name ".into(),
            postal_address: Some("  1 High Street  ".into()),
            email: Some(" billing@example.test ".into()),
            phone: Some(" 01234 567890 ".into()),
        };
        let write = core_contact(&valid).expect("valid core customer");
        assert_eq!(write.contact_id, "customer-1");
        assert_eq!(write.kind, "customer");
        assert_eq!(write.display_name, "Customer Name");
        assert_eq!(write.postal_address.as_deref(), Some("1 High Street"));
        assert_eq!(write.email.as_deref(), Some("billing@example.test"));
        assert_eq!(write.phone.as_deref(), Some("01234 567890"));

        let too_long = OwnerContactsSaveRequest {
            books: books_ref(),
            contact_id: "customer-2".into(),
            kind: OwnerContactKind::Customer,
            display_name: "x".repeat(201),
            postal_address: None,
            email: None,
            phone: None,
        };
        assert!(core_contact(&too_long).is_err());
    }

    #[test]
    fn registered_supporting_commands_route_bounded_quote_operations() {
        let list = serde_json::json!({
            "books": {
                "fileName": "owner-support.sqlite",
                "booksId": "owner-support",
                "actor": "local-owner"
            },
            "operation": "quotesList",
            "kind": "estimate",
            "state": "draft",
            "limit": 50
        });
        assert!(matches!(
            serde_json::from_value::<OwnerSupportingListRequest>(list).unwrap(),
            OwnerSupportingListRequest::Quotes(_)
        ));

        let mutation = serde_json::json!({
            "books": {
                "fileName": "owner-support.sqlite",
                "booksId": "owner-support",
                "actor": "local-owner"
            },
            "operation": "saveLine",
            "quoteId": "quote-1",
            "lineId": "line-1",
            "description": "Service",
            "quantitySubunits": 2,
            "unitPricePence": 12500
        });
        assert!(matches!(
            serde_json::from_value::<OwnerSupportingSaveRequest>(mutation).unwrap(),
            OwnerSupportingSaveRequest::Quote(OwnerQuoteMutationRequest::SaveLine { .. })
        ));

        for forbidden in ["invoiceId", "vatRate", "paymentProvider", "pdfTemplate"] {
            let mut invalid = serde_json::json!({
                "books": {
                    "fileName": "owner-support.sqlite",
                    "booksId": "owner-support",
                    "actor": "local-owner"
                },
                "operation": "transition",
                "quoteId": "quote-1",
                "target": "accepted"
            });
            invalid
                .as_object_mut()
                .expect("object fixture")
                .insert(forbidden.to_string(), serde_json::json!("not-authorised"));
            assert!(
                serde_json::from_value::<OwnerSupportingSaveRequest>(invalid).is_err(),
                "quote mutation accepted prohibited field {forbidden}"
            );
        }
    }

    #[test]
    fn books_info_serialization_contains_no_path_key_or_database_authority() {
        let info = OwnerSettingsBooksInfo {
            bridge_version: 1,
            books_id: "books-1".into(),
            company_name: "Example".into(),
            database_schema_version: 8,
            expected_database_schema_version: 8,
            books_format_version: 1,
            application_schema_version: 6,
            facade_api_version: 1,
            foundation_version: "0.0.1".into(),
            shell_version: "0.0.1".into(),
            migration_required: false,
            production_encryption_required: true,
            encrypted_native_required: true,
            encrypted_native_session_active: true,
            backup_before_existing_open_required: true,
        };
        let json = serde_json::to_string(&info).expect("serialize books info");
        for forbidden in [
            "fileName",
            "databasePath",
            "dbPath",
            "passphrase",
            "encryptionKey",
            "SHARK_SBC1D_PROOF_KEY",
            "sqlite",
            "sqlcipher",
        ] {
            assert!(!json.contains(forbidden), "owner books info leaked {forbidden}");
        }
    }

    #[test]
    fn storage_root_registration_is_opaque_session_scoped_and_cancellation_safe() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sbc7b1-root-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create root");
        let roots = NativeDocumentRootRegistry::default();
        assert_eq!(roots.registered_root_count(), 0);

        let cancelled = register_storage_root_selection(&roots, None).expect("cancelled");
        assert!(matches!(cancelled, OwnerStorageRootSelectOutcome::Cancelled { .. }));
        assert_eq!(roots.registered_root_count(), 0);

        let registered = register_storage_root_selection(&roots, Some(path.clone())).expect("registered");
        let json = serde_json::to_string(&registered).expect("serialize root outcome");
        assert!(json.contains("storage-root-session-"));
        assert!(json.contains("deviceSession"));
        assert!(!json.contains(path.to_string_lossy().as_ref()));
        assert_eq!(roots.registered_root_count(), 1);

        let repeat = register_storage_root_selection(&roots, Some(path.clone())).expect("repeat root");
        let repeat_json = serde_json::to_string(&repeat).expect("serialize repeat root");
        assert_eq!(json, repeat_json, "same canonical root reuses opaque session id");
        assert_eq!(roots.registered_root_count(), 1);

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn mobile_storage_root_selection_fails_closed_without_raw_authority() {
        let error = storage_root_mobile_unsupported_error();
        assert_eq!(error.code, "unsupportedPlatform");
        assert_eq!(
            error.message,
            "folder selection is not supported on this mobile platform"
        );
        let json = serde_json::to_string(&error).expect("serialize unsupported-platform error");
        for forbidden in ["fileName", "databasePath", "dbPath", "passphrase", "file://", "content://"] {
            assert!(!json.contains(forbidden), "mobile unsupported result leaked {forbidden}");
        }
    }

    #[test]
    fn report_summary_uses_factual_account_signs_and_optional_cash_balances() {
        let trial = TrialBalance {
            accounts: vec![
                BalanceLine {
                    code: "4000".into(),
                    account_type: "revenue".into(),
                    debit_total: 500,
                    credit_total: 10_000,
                },
                BalanceLine {
                    code: "5000".into(),
                    account_type: "expense".into(),
                    debit_total: 4_000,
                    credit_total: 250,
                },
                BalanceLine {
                    code: "1000".into(),
                    account_type: "asset".into(),
                    debit_total: 8_000,
                    credit_total: 1_000,
                },
            ],
            total_debits: 12_500,
            total_credits: 12_500,
            balanced: true,
        };
        let report = report_from_trial_balance(trial).expect("report");
        assert_eq!(report.money_in_minor, 9_500);
        assert_eq!(report.money_out_minor, 3_750);
        assert_eq!(report.business_bank_balance_minor, Some(7_000));
        assert_eq!(report.cash_balance_minor, None);
        assert!(report.books_balanced);
        assert_eq!(report.currency, "GBP");
    }

    #[test]
    fn report_summary_fails_closed_on_i64_result_overflow_and_ambiguous_bank() {
        let overflow = TrialBalance {
            accounts: vec![
                BalanceLine {
                    code: "4000".into(),
                    account_type: "revenue".into(),
                    debit_total: 0,
                    credit_total: i64::MAX,
                },
                BalanceLine {
                    code: "4100".into(),
                    account_type: "revenue".into(),
                    debit_total: 0,
                    credit_total: 1,
                },
            ],
            total_debits: 0,
            total_credits: i64::MAX,
            balanced: false,
        };
        assert!(report_from_trial_balance(overflow).is_err());

        let ambiguous = TrialBalance {
            accounts: vec![
                BalanceLine {
                    code: "1000".into(),
                    account_type: "asset".into(),
                    debit_total: 1,
                    credit_total: 0,
                },
                BalanceLine {
                    code: "1000".into(),
                    account_type: "asset".into(),
                    debit_total: 2,
                    credit_total: 0,
                },
            ],
            total_debits: 3,
            total_credits: 3,
            balanced: true,
        };
        assert!(report_from_trial_balance(ambiguous).is_err());
    }
}


// SBC8A3 reuses the registered supporting-data commands with explicit bounded
// operations. No request contains raw storage, tax, PDF, provider or network authority.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub(crate) enum OwnerInvoiceMutationRequest {
    CreateInvoiceDraft { books:OwnerBooksRef,invoice_id:String,customer_id:String,issue_date:String,due_date:Option<String> },
    SetInvoiceCustomer { books:OwnerBooksRef,invoice_id:String,customer_id:String },
    SetInvoiceDates { books:OwnerBooksRef,invoice_id:String,issue_date:String,due_date:Option<String> },
    SaveInvoiceLine { books:OwnerBooksRef,invoice_id:String,line_id:String,description:String,quantity_subunits:i64,unit_price_pence:i64 },
    RemoveInvoiceLine { books:OwnerBooksRef,invoice_id:String,line_id:String },
    IssueInvoice { books:OwnerBooksRef,invoice_id:String,invoice_number:String },
    CancelInvoice { books:OwnerBooksRef,invoice_id:String },
    RecordManualPayment { books:OwnerBooksRef,invoice_id:String,amount_pence:i64 },
    ConvertAcceptedQuote { books:OwnerBooksRef,quote_id:String,invoice_id:String,issue_date:String,due_date:Option<String> },
    CreateCreditNoteDraft { books:OwnerBooksRef,credit_note_id:String,invoice_id:String },
    SaveCreditNoteLine { books:OwnerBooksRef,credit_note_id:String,line_id:String,invoice_line_id:String,quantity_subunits:i64 },
    RemoveCreditNoteLine { books:OwnerBooksRef,credit_note_id:String,line_id:String },
    IssueCreditNote { books:OwnerBooksRef,credit_note_id:String,credit_note_number:String },
    CancelCreditNote { books:OwnerBooksRef,credit_note_id:String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub(crate) enum OwnerInvoiceReadRequest {
    InvoicesList { books:OwnerBooksRef,state:Option<InvoiceState>,limit:u16 },
    InvoiceDetail { books:OwnerBooksRef,invoice_id:String,limit:u16 },
    CreditNotesList { books:OwnerBooksRef,invoice_id:Option<String>,state:Option<CreditNoteState>,limit:u16 },
    CreditNoteDetail { books:OwnerBooksRef,credit_note_id:String,limit:u16 },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(crate) enum OwnerInvoiceReadOutcome {
    Invoices { bridge_version:u32,invoices:Vec<InvoiceView>,bank_payment_created:bool,provider_network_used:bool },
    InvoiceDetail { bridge_version:u32,invoice:InvoiceView,history:Vec<shark_foundation::InvoiceMutationView>,credit_notes:Vec<CreditNoteView>,bank_payment_created:bool,provider_network_used:bool },
    CreditNotes { bridge_version:u32,credit_notes:Vec<CreditNoteView>,bank_payment_created:bool,provider_network_used:bool },
    CreditNoteDetail { bridge_version:u32,credit_note:CreditNoteView,history:Vec<shark_foundation::CreditNoteMutationView>,invoice:InvoiceView,bank_payment_created:bool,provider_network_used:bool },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(crate) enum OwnerInvoiceMutationOutcome {
    Invoice { bridge_version:u32,invoice:InvoiceView,mutation:shark_foundation::InvoiceMutationView,bank_payment_created:bool,provider_network_used:bool,requires_further_automatic_action:bool },
    CreditNote { bridge_version:u32,credit_note:CreditNoteView,mutation:shark_foundation::CreditNoteMutationView,invoice:InvoiceView,invoice_mutation:Option<shark_foundation::InvoiceMutationView>,bank_payment_created:bool,provider_network_used:bool,requires_further_automatic_action:bool },
}

fn invoice_read(request:OwnerInvoiceReadRequest)->OwnerSupportingDataResult<OwnerInvoiceReadOutcome>{
    match request {
        OwnerInvoiceReadRequest::InvoicesList{books,state,limit}=>Ok(OwnerInvoiceReadOutcome::Invoices{bridge_version:OWNER_SUPPORTING_DATA_BRIDGE_VERSION,invoices:books.open()?.invoices(state,i64::from(limit)).map_err(OwnerSupportingDataError::foundation)?,bank_payment_created:false,provider_network_used:false}),
        OwnerInvoiceReadRequest::InvoiceDetail{books,invoice_id,limit}=>{let id=core_record_id(invoice_id)?;let books=books.open()?;Ok(OwnerInvoiceReadOutcome::InvoiceDetail{bridge_version:OWNER_SUPPORTING_DATA_BRIDGE_VERSION,invoice:books.invoice(&id).map_err(OwnerSupportingDataError::foundation)?,history:books.invoice_history(&id,i64::from(limit)).map_err(OwnerSupportingDataError::foundation)?,credit_notes:books.credit_notes(Some(&id),None,i64::from(limit)).map_err(OwnerSupportingDataError::foundation)?,bank_payment_created:false,provider_network_used:false})}
        OwnerInvoiceReadRequest::CreditNotesList{books,invoice_id,state,limit}=>{let id=invoice_id.map(core_record_id).transpose()?;Ok(OwnerInvoiceReadOutcome::CreditNotes{bridge_version:OWNER_SUPPORTING_DATA_BRIDGE_VERSION,credit_notes:books.open()?.credit_notes(id.as_deref(),state,i64::from(limit)).map_err(OwnerSupportingDataError::foundation)?,bank_payment_created:false,provider_network_used:false})}
        OwnerInvoiceReadRequest::CreditNoteDetail{books,credit_note_id,limit}=>{let id=core_record_id(credit_note_id)?;let books=books.open()?;let credit=books.credit_note(&id).map_err(OwnerSupportingDataError::foundation)?;Ok(OwnerInvoiceReadOutcome::CreditNoteDetail{bridge_version:OWNER_SUPPORTING_DATA_BRIDGE_VERSION,invoice:books.invoice(&credit.invoice_id).map_err(OwnerSupportingDataError::foundation)?,history:books.credit_note_history(&id,i64::from(limit)).map_err(OwnerSupportingDataError::foundation)?,credit_note:credit,bank_payment_created:false,provider_network_used:false})}
    }
}

fn owner_invoice_outcome(outcome:InvoiceMutationOutcome)->OwnerInvoiceMutationOutcome{OwnerInvoiceMutationOutcome::Invoice{bridge_version:OWNER_SUPPORTING_DATA_BRIDGE_VERSION,invoice:outcome.invoice,mutation:outcome.mutation,bank_payment_created:false,provider_network_used:false,requires_further_automatic_action:false}}
fn owner_credit_outcome(outcome:CreditNoteMutationOutcome)->OwnerInvoiceMutationOutcome{OwnerInvoiceMutationOutcome::CreditNote{bridge_version:OWNER_SUPPORTING_DATA_BRIDGE_VERSION,credit_note:outcome.credit_note,mutation:outcome.mutation,invoice:outcome.invoice,invoice_mutation:outcome.invoice_mutation,bank_payment_created:false,provider_network_used:false,requires_further_automatic_action:false}}

fn invoice_mutation(request:OwnerInvoiceMutationRequest)->OwnerSupportingDataResult<OwnerInvoiceMutationOutcome>{
    use OwnerInvoiceMutationRequest::*;
    match request {
        CreateInvoiceDraft{books,invoice_id,customer_id,issue_date,due_date}=>books.open()?.create_invoice_draft(&InvoiceDraftWrite{invoice_id:core_record_id(invoice_id)?,customer_id:core_record_id(customer_id)?,issue_date,due_date}).map(owner_invoice_outcome),
        SetInvoiceCustomer{books,invoice_id,customer_id}=>books.open()?.set_invoice_customer(&core_record_id(invoice_id)?,&core_record_id(customer_id)?).map(owner_invoice_outcome),
        SetInvoiceDates{books,invoice_id,issue_date,due_date}=>books.open()?.set_invoice_dates(&core_record_id(invoice_id)?,&issue_date,due_date.as_deref()).map(owner_invoice_outcome),
        SaveInvoiceLine{books,invoice_id,line_id,description,quantity_subunits,unit_price_pence}=>{let line=core_quote_line(line_id,description,quantity_subunits,unit_price_pence)?;books.open()?.save_invoice_line(&core_record_id(invoice_id)?,&InvoiceLineWrite{line_id:line.line_id,description:line.description,quantity_subunits:line.quantity_subunits,unit_price_minor:line.unit_price_minor}).map(owner_invoice_outcome)},
        RemoveInvoiceLine{books,invoice_id,line_id}=>books.open()?.remove_invoice_line(&core_record_id(invoice_id)?,&core_record_id(line_id)?).map(owner_invoice_outcome),
        IssueInvoice{books,invoice_id,invoice_number}=>{let number=core::CommercialNumber::new(invoice_number).map_err(|error|OwnerSupportingDataError::invalid(error.to_string()))?;books.open()?.issue_invoice(&core_record_id(invoice_id)?,number.as_str()).map(owner_invoice_outcome)},
        CancelInvoice{books,invoice_id}=>books.open()?.cancel_invoice(&core_record_id(invoice_id)?).map(owner_invoice_outcome),
        RecordManualPayment{books,invoice_id,amount_pence}=>books.open()?.record_manual_invoice_payment(&core_record_id(invoice_id)?,amount_pence).map(owner_invoice_outcome),
        ConvertAcceptedQuote{books,quote_id,invoice_id,issue_date,due_date}=>books.open()?.convert_accepted_quote(&core_record_id(quote_id)?,&core_record_id(invoice_id)?,&issue_date,due_date.as_deref()).map(owner_invoice_outcome),
        CreateCreditNoteDraft{books,credit_note_id,invoice_id}=>books.open()?.create_credit_note_draft(&CreditNoteDraftWrite{credit_note_id:core_record_id(credit_note_id)?,invoice_id:core_record_id(invoice_id)?}).map(owner_credit_outcome),
        SaveCreditNoteLine{books,credit_note_id,line_id,invoice_line_id,quantity_subunits}=>books.open()?.save_credit_note_line(&core_record_id(credit_note_id)?,&CreditNoteLineWrite{line_id:core_record_id(line_id)?,invoice_line_id:core_record_id(invoice_line_id)?,quantity_subunits}).map(owner_credit_outcome),
        RemoveCreditNoteLine{books,credit_note_id,line_id}=>books.open()?.remove_credit_note_line(&core_record_id(credit_note_id)?,&core_record_id(line_id)?).map(owner_credit_outcome),
        IssueCreditNote{books,credit_note_id,credit_note_number}=>{let number=core::CommercialNumber::new(credit_note_number).map_err(|error|OwnerSupportingDataError::invalid(error.to_string()))?;books.open()?.issue_credit_note(&core_record_id(credit_note_id)?,number.as_str()).map(owner_credit_outcome)},
        CancelCreditNote{books,credit_note_id}=>books.open()?.cancel_credit_note(&core_record_id(credit_note_id)?).map(owner_credit_outcome),
    }.map_err(OwnerSupportingDataError::foundation)
}

#[cfg(test)]
mod invoice_bridge_tests {
    use super::*;
    #[test] fn sbc8a3_registered_bridge_is_explicit_and_rejects_unrelated_authority(){
        let books=serde_json::json!({"fileName":"books.db","booksId":"company","actor":"owner"});
        let request=serde_json::json!({"operation":"convertAcceptedQuote","books":books,"invoiceId":"inv-1","quoteId":"q-1","issueDate":"2026-10-02","dueDate":null});
        assert!(matches!(serde_json::from_value::<OwnerSupportingSaveRequest>(request.clone()).unwrap(),OwnerSupportingSaveRequest::Invoice(_)));
        for field in ["vatRate","provider","sql","paymentToken","pdfTemplate"]{let mut invalid=request.clone();invalid[field]=serde_json::json!("forbidden");assert!(serde_json::from_value::<OwnerSupportingSaveRequest>(invalid).is_err());}
        let fractional=serde_json::json!({"operation":"recordManualPayment","books":books,"invoiceId":"i","amountPence":1.5});assert!(serde_json::from_value::<OwnerSupportingSaveRequest>(fractional).is_err());
        let read=serde_json::json!({"operation":"invoiceDetail","books":books,"invoiceId":"inv-1","limit":200});assert!(matches!(serde_json::from_value::<OwnerSupportingListRequest>(read).unwrap(),OwnerSupportingListRequest::Invoices(_)));
    }
}
