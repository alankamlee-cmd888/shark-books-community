//! SBC-7B1 bounded owner-facing application bridge.
//!
//! This module exposes a finite typed owner operation surface over the already-
//! proven Shark product core and frozen accounting facade. It does not expose raw
//! database handles, Beankeeper objects, arbitrary filesystem paths, generic shell
//! execution, tax decisions or automatic matching/reconciliation authority.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{
    Books, Direction, FoundationError, PostOutcome, PostTransactionRequest, PostingLine,
};

use super::{open_books_impl, OpenBooksRequest};

const OWNER_BRIDGE_VERSION: u32 = 1;
const GBP: &str = "GBP";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandError {
    code: &'static str,
    message: String,
}

impl OwnerCommandError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalidInput",
            message: message.into(),
        }
    }

    fn domain(error: core::DomainError) -> Self {
        Self {
            code: "invalidInput",
            message: error.to_string(),
        }
    }

    fn foundation(error: FoundationError) -> Self {
        Self {
            code: "booksOperationFailed",
            message: error.to_string(),
        }
    }
}

type OwnerResult<T> = Result<T, OwnerCommandError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerBooksRef {
    fn open(&self) -> OwnerResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerCommandError::foundation)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerSettlementAccount {
    BusinessBank,
    Cash,
}

impl From<OwnerSettlementAccount> for core::SettlementAccount {
    fn from(value: OwnerSettlementAccount) -> Self {
        match value {
            OwnerSettlementAccount::BusinessBank => Self::BusinessBank,
            OwnerSettlementAccount::Cash => Self::Cash,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerIncomeCategory {
    SalesTrading,
    OtherBusinessIncome,
}

impl From<OwnerIncomeCategory> for core::IncomeCategory {
    fn from(value: OwnerIncomeCategory) -> Self {
        match value {
            OwnerIncomeCategory::SalesTrading => Self::SalesTrading,
            OwnerIncomeCategory::OtherBusinessIncome => Self::OtherBusinessIncome,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerExpenseCategory {
    GoodsStockMaterials,
    OfficePhoneSoftware,
    Travel,
    VehicleCosts,
    PremisesUtilities,
    AdvertisingMarketing,
    BankFinanceInsurance,
    ProfessionalFees,
    RepairsMaintenance,
    Training,
    StaffSubcontractorCosts,
    OtherBusinessExpense,
}

impl From<OwnerExpenseCategory> for core::ExpenseCategory {
    fn from(value: OwnerExpenseCategory) -> Self {
        match value {
            OwnerExpenseCategory::GoodsStockMaterials => Self::GoodsStockMaterials,
            OwnerExpenseCategory::OfficePhoneSoftware => Self::OfficePhoneSoftware,
            OwnerExpenseCategory::Travel => Self::Travel,
            OwnerExpenseCategory::VehicleCosts => Self::VehicleCosts,
            OwnerExpenseCategory::PremisesUtilities => Self::PremisesUtilities,
            OwnerExpenseCategory::AdvertisingMarketing => Self::AdvertisingMarketing,
            OwnerExpenseCategory::BankFinanceInsurance => Self::BankFinanceInsurance,
            OwnerExpenseCategory::ProfessionalFees => Self::ProfessionalFees,
            OwnerExpenseCategory::RepairsMaintenance => Self::RepairsMaintenance,
            OwnerExpenseCategory::Training => Self::Training,
            OwnerExpenseCategory::StaffSubcontractorCosts => Self::StaffSubcontractorCosts,
            OwnerExpenseCategory::OtherBusinessExpense => Self::OtherBusinessExpense,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum OwnerBusinessUse {
    Business,
    Private,
    Mixed {
        #[serde(rename = "businessBasisPoints")]
        business_basis_points: u16,
    },
}

impl OwnerBusinessUse {
    fn to_core(&self) -> OwnerResult<core::BusinessUse> {
        match self {
            Self::Business => Ok(core::BusinessUse::Business),
            Self::Private => Ok(core::BusinessUse::Private),
            Self::Mixed {
                business_basis_points,
            } => core::BusinessUse::mixed(*business_basis_points)
                .map_err(OwnerCommandError::domain),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerMoneyInRequest {
    books: OwnerBooksRef,
    record_id: String,
    description: String,
    date: String,
    amount_pence: i64,
    category: OwnerIncomeCategory,
    settlement: OwnerSettlementAccount,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerMoneyOutRequest {
    books: OwnerBooksRef,
    record_id: String,
    description: String,
    date: String,
    amount_pence: i64,
    category: OwnerExpenseCategory,
    business_use: OwnerBusinessUse,
    settlement: OwnerSettlementAccount,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerPostingPreview {
    bridge_version: u32,
    record_id: String,
    record_kind: &'static str,
    date: String,
    amount_pence: i64,
    business_amount_pence: i64,
    private_amount_pence: i64,
    currency: &'static str,
    balanced: bool,
    requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerSaveReceipt {
    bridge_version: u32,
    record_id: String,
    record_kind: &'static str,
    transaction_id: i64,
    created: bool,
    already_recorded: bool,
    requires_further_automatic_action: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerHomeStatus {
    bridge_version: u32,
    company_name: String,
    transaction_count: i64,
    books_balanced: bool,
    production_encryption_required: bool,
}

fn parse_date(value: &str) -> OwnerResult<core::Date> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .ok_or_else(|| OwnerCommandError::invalid("date must be YYYY-MM-DD"))?;
    let month = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerCommandError::invalid("date must be YYYY-MM-DD"))?;
    let day = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| OwnerCommandError::invalid("date must be YYYY-MM-DD"))?;
    if parts.next().is_some() || value.len() != 10 {
        return Err(OwnerCommandError::invalid("date must be YYYY-MM-DD"));
    }
    core::Date::new(year, month, day).map_err(OwnerCommandError::domain)
}

fn account_type_name(value: core::AccountType) -> &'static str {
    match value {
        core::AccountType::Asset => "asset",
        core::AccountType::Liability => "liability",
        core::AccountType::Equity => "equity",
        core::AccountType::Revenue => "revenue",
        core::AccountType::Expense => "expense",
    }
}

fn ensure_default_chart(books: &Books) -> OwnerResult<()> {
    let existing: HashSet<String> = books
        .trial_balance()
        .map_err(OwnerCommandError::foundation)?
        .accounts
        .into_iter()
        .map(|line| line.code)
        .collect();

    for account in core::DEFAULT_CHART {
        if !existing.contains(account.code) {
            books
                .create_account(account.code, account.name, account_type_name(account.account_type))
                .map_err(OwnerCommandError::foundation)?;
        }
    }
    Ok(())
}

fn foundation_request_from_plan(
    plan: &core::PostingPlan,
    record_kind: &str,
    record_id: &str,
) -> PostTransactionRequest {
    let lines = plan
        .lines()
        .iter()
        .map(|line| PostingLine {
            account_code: line.account_code().to_string(),
            direction: match line.direction() {
                core::PostingDirection::Debit => Direction::Debit,
                core::PostingDirection::Credit => Direction::Credit,
            },
            amount_minor: line.amount_minor(),
            memo: None,
        })
        .collect();

    PostTransactionRequest {
        description: plan.description().to_string(),
        date: plan.date().iso(),
        currency_code: GBP.to_string(),
        reference: Some(format!("sbc7b1:{record_kind}:{record_id}")),
        metadata: Some(
            serde_json::json!({
                "source": "owner-ui",
                "recordKind": record_kind,
                "recordId": record_id,
                "bridgeVersion": OWNER_BRIDGE_VERSION
            })
            .to_string(),
        ),
        lines,
    }
}

fn money_in_plan(request: &OwnerMoneyInRequest) -> OwnerResult<(core::PostingPlan, core::RecordId)> {
    let record_id = core::RecordId::new(request.record_id.clone()).map_err(OwnerCommandError::domain)?;
    let amount = core::GbpAmount::positive_minor(request.amount_pence).map_err(OwnerCommandError::domain)?;
    let date = parse_date(&request.date)?;
    let record = core::IncomeRecord::new(
        record_id.clone(),
        request.description.clone(),
        date,
        amount,
        request.category.into(),
        request.settlement.into(),
        core::SourceProvenance::manual(),
    )
    .map_err(OwnerCommandError::domain)?;
    Ok((core::plan_income(&record), record_id))
}

fn money_out_plan(
    request: &OwnerMoneyOutRequest,
) -> OwnerResult<(core::PostingPlan, core::RecordId, i64, i64)> {
    let record_id = core::RecordId::new(request.record_id.clone()).map_err(OwnerCommandError::domain)?;
    let amount = core::GbpAmount::positive_minor(request.amount_pence).map_err(OwnerCommandError::domain)?;
    let business_use = request.business_use.to_core()?;
    let (business_amount_pence, private_amount_pence) = business_use
        .split(amount)
        .map_err(OwnerCommandError::domain)?;
    let record = core::ExpenseRecord::new(
        record_id.clone(),
        request.description.clone(),
        parse_date(&request.date)?,
        amount,
        request.category.into(),
        business_use,
        request.settlement.into(),
        None,
        core::SourceProvenance::manual(),
    )
    .map_err(OwnerCommandError::domain)?;
    let plan = core::plan_expense(&record).map_err(OwnerCommandError::domain)?;
    Ok((plan, record_id, business_amount_pence, private_amount_pence))
}

fn preview_from_plan(
    plan: &core::PostingPlan,
    record_id: &core::RecordId,
    record_kind: &'static str,
    amount_pence: i64,
    business_amount_pence: i64,
    private_amount_pence: i64,
) -> OwnerPostingPreview {
    OwnerPostingPreview {
        bridge_version: OWNER_BRIDGE_VERSION,
        record_id: record_id.as_str().to_string(),
        record_kind,
        date: plan.date().iso(),
        amount_pence,
        business_amount_pence,
        private_amount_pence,
        currency: GBP,
        balanced: plan.is_balanced(),
        requires_confirmation: true,
    }
}

fn save_plan(
    books: &Books,
    plan: &core::PostingPlan,
    record_id: &core::RecordId,
    record_kind: &'static str,
) -> OwnerResult<OwnerSaveReceipt> {
    ensure_default_chart(books)?;
    let foundation_request = foundation_request_from_plan(plan, record_kind, record_id.as_str());
    let outcome = books
        .post(&foundation_request)
        .map_err(OwnerCommandError::foundation)?;
    let (transaction_id, created) = match outcome {
        PostOutcome::Created(id) => (id, true),
        PostOutcome::Skipped(id) => (id, false),
    };
    Ok(OwnerSaveReceipt {
        bridge_version: OWNER_BRIDGE_VERSION,
        record_id: record_id.as_str().to_string(),
        record_kind,
        transaction_id,
        created,
        already_recorded: !created,
        requires_further_automatic_action: false,
    })
}

#[tauri::command]
pub(crate) fn owner_home_status(books: OwnerBooksRef) -> OwnerResult<OwnerHomeStatus> {
    let opened = books.open()?;
    let metadata = opened.metadata().map_err(OwnerCommandError::foundation)?;
    let transaction_count = opened
        .count_transactions()
        .map_err(OwnerCommandError::foundation)?;
    let balance = opened
        .trial_balance()
        .map_err(OwnerCommandError::foundation)?;
    Ok(OwnerHomeStatus {
        bridge_version: OWNER_BRIDGE_VERSION,
        company_name: metadata.company_name,
        transaction_count,
        books_balanced: balance.balanced,
        production_encryption_required: super::production_encryption_required(),
    })
}

#[tauri::command]
pub(crate) fn owner_money_in_preview(
    request: OwnerMoneyInRequest,
) -> OwnerResult<OwnerPostingPreview> {
    let (plan, record_id) = money_in_plan(&request)?;
    Ok(preview_from_plan(
        &plan,
        &record_id,
        "moneyIn",
        request.amount_pence,
        request.amount_pence,
        0,
    ))
}

#[tauri::command]
pub(crate) fn owner_money_in_save(
    request: OwnerMoneyInRequest,
) -> OwnerResult<OwnerSaveReceipt> {
    let books = request.books.open()?;
    let (plan, record_id) = money_in_plan(&request)?;
    save_plan(&books, &plan, &record_id, "moneyIn")
}

#[tauri::command]
pub(crate) fn owner_money_out_preview(
    request: OwnerMoneyOutRequest,
) -> OwnerResult<OwnerPostingPreview> {
    let (plan, record_id, business_amount_pence, private_amount_pence) = money_out_plan(&request)?;
    Ok(preview_from_plan(
        &plan,
        &record_id,
        "moneyOut",
        request.amount_pence,
        business_amount_pence,
        private_amount_pence,
    ))
}

#[tauri::command]
pub(crate) fn owner_money_out_save(
    request: OwnerMoneyOutRequest,
) -> OwnerResult<OwnerSaveReceipt> {
    let books = request.books.open()?;
    let (plan, record_id, _, _) = money_out_plan(&request)?;
    save_plan(&books, &plan, &record_id, "moneyOut")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn books() -> OwnerBooksRef {
        OwnerBooksRef {
            file_name: "owner-test.sqlite".to_string(),
            books_id: "owner-test".to_string(),
            actor: "local-owner".to_string(),
        }
    }

    #[test]
    fn money_in_preview_is_owner_safe_and_confirmation_bound() {
        let preview = owner_money_in_preview(OwnerMoneyInRequest {
            books: books(),
            record_id: "sale-001".into(),
            description: "Customer payment".into(),
            date: "2026-09-10".into(),
            amount_pence: 12_345,
            category: OwnerIncomeCategory::SalesTrading,
            settlement: OwnerSettlementAccount::BusinessBank,
        })
        .expect("valid preview");
        assert_eq!(preview.amount_pence, 12_345);
        assert_eq!(preview.business_amount_pence, 12_345);
        assert_eq!(preview.private_amount_pence, 0);
        assert!(preview.balanced);
        assert!(preview.requires_confirmation);
    }

    #[test]
    fn mixed_money_out_preview_preserves_explicit_owner_split() {
        let preview = owner_money_out_preview(OwnerMoneyOutRequest {
            books: books(),
            record_id: "expense-001".into(),
            description: "Mixed phone bill".into(),
            date: "2026-09-10".into(),
            amount_pence: 10_001,
            category: OwnerExpenseCategory::OfficePhoneSoftware,
            business_use: OwnerBusinessUse::Mixed {
                business_basis_points: 3_500,
            },
            settlement: OwnerSettlementAccount::BusinessBank,
        })
        .expect("valid preview");
        assert_eq!(preview.business_amount_pence, 3_500);
        assert_eq!(preview.private_amount_pence, 6_501);
        assert!(preview.balanced);
        assert!(preview.requires_confirmation);
    }

    #[test]
    fn plan_translation_stays_behind_native_bridge() {
        let request = OwnerMoneyInRequest {
            books: books(),
            record_id: "sale-002".into(),
            description: "Sale".into(),
            date: "2026-09-10".into(),
            amount_pence: 2_500,
            category: OwnerIncomeCategory::SalesTrading,
            settlement: OwnerSettlementAccount::BusinessBank,
        };
        let (plan, id) = money_in_plan(&request).expect("plan");
        let foundation = foundation_request_from_plan(&plan, "moneyIn", id.as_str());
        assert_eq!(foundation.currency_code, "GBP");
        assert_eq!(foundation.reference.as_deref(), Some("sbc7b1:moneyIn:sale-002"));
        assert_eq!(foundation.lines.len(), 2);
        assert_eq!(
            foundation.lines.iter().map(|line| line.amount_minor).sum::<i64>(),
            5_000
        );
    }

    #[test]
    fn unsafe_or_ledger_shaped_request_fields_are_rejected() {
        let injected = serde_json::json!({
            "books": {
                "fileName": "safe.sqlite",
                "booksId": "safe-books",
                "actor": "local-owner"
            },
            "recordId": "sale-003",
            "description": "Sale",
            "date": "2026-09-10",
            "amountPence": 1000,
            "category": "salesTrading",
            "settlement": "businessBank",
            "accountCode": "4000",
            "debit": 1000,
            "databasePath": "C:/outside.sqlite"
        });
        assert!(serde_json::from_value::<OwnerMoneyInRequest>(injected).is_err());
    }

    #[test]
    fn invalid_dates_and_nonpositive_amounts_fail_closed() {
        let invalid_date = owner_money_in_preview(OwnerMoneyInRequest {
            books: books(),
            record_id: "sale-004".into(),
            description: "Sale".into(),
            date: "2026-02-30".into(),
            amount_pence: 100,
            category: OwnerIncomeCategory::SalesTrading,
            settlement: OwnerSettlementAccount::BusinessBank,
        });
        assert!(invalid_date.is_err());

        let zero = owner_money_out_preview(OwnerMoneyOutRequest {
            books: books(),
            record_id: "expense-004".into(),
            description: "Expense".into(),
            date: "2026-09-10".into(),
            amount_pence: 0,
            category: OwnerExpenseCategory::Travel,
            business_use: OwnerBusinessUse::Business,
            settlement: OwnerSettlementAccount::BusinessBank,
        });
        assert!(zero.is_err());
    }
}
