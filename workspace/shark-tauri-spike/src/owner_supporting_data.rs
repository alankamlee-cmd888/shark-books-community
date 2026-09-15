//! SBC-7B1 Batch B typed owner bridge for Contacts, Settings and factual Reports.
//!
//! This module exposes only bounded owner-safe DTOs. It has no raw database,
//! key/passphrase, filesystem-path, accounting-rule, tax or network authority.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{
    Books, ContactPersistOutcome, ContactView, ContactWrite, FoundationError,
    FoundationErrorCode, TrialBalance,
};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_dialog::DialogExt;

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
struct OwnerBooksRef {
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerContactsListRequest {
    books: OwnerBooksRef,
    kind: Option<OwnerContactKind>,
    limit: Option<u16>,
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
    match write.kind {
        OwnerContactKind::Customer => {
            let customer = core::Customer::new(record_id, write.display_name.clone())
                .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?;
            Ok(ContactWrite {
                contact_id: customer.id().as_str().to_string(),
                kind: write.kind.as_str().to_string(),
                display_name: customer.display_name().to_string(),
            })
        }
        OwnerContactKind::Supplier => {
            let supplier = core::Supplier::new(record_id, write.display_name.clone())
                .map_err(|error| OwnerSupportingDataError::invalid(error.to_string()))?;
            Ok(ContactWrite {
                contact_id: supplier.id().as_str().to_string(),
                kind: write.kind.as_str().to_string(),
                display_name: supplier.display_name().to_string(),
            })
        }
    }
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

#[tauri::command]
pub(crate) fn owner_contacts_list(
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
        };
        assert!(core_contact(&invalid).is_err());

        let valid = OwnerContactsSaveRequest {
            books: books_ref(),
            contact_id: "customer-1".into(),
            kind: OwnerContactKind::Customer,
            display_name: " Customer Name ".into(),
        };
        let write = core_contact(&valid).expect("valid core customer");
        assert_eq!(write.contact_id, "customer-1");
        assert_eq!(write.kind, "customer");
        assert_eq!(write.display_name, "Customer Name");
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
            application_schema_version: 5,
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
