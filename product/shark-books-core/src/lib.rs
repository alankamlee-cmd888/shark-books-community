//! Shark Books Community UK sole-trader domain core.
//! Stage SBC-2 is deliberately platform-neutral and persistence-free: it creates
//! deterministic accounting plans; persistence remains behind the frozen Shark facade.

#![forbid(unsafe_code)]

use std::fmt;

pub const SHARK_BOOKS_CORE_CONTRACT_VERSION: u32 = 1;
pub const DEFAULT_CURRENCY_CODE: &str = "GBP";
pub const BASIS_POINTS_DENOMINATOR: u16 = 10_000;
pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidDate(String), InvalidAmount(String), InvalidId(String), InvalidName(String),
    InvalidBusinessUse(String), InvalidInvoiceDates(String), InvalidProvenance(String),
    ArithmeticOverflow,
}
impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDate(s) | Self::InvalidAmount(s) | Self::InvalidId(s) |
            Self::InvalidName(s) | Self::InvalidBusinessUse(s) |
            Self::InvalidInvoiceDates(s) | Self::InvalidProvenance(s) => f.write_str(s),
            Self::ArithmeticOverflow => f.write_str("integer arithmetic overflow"),
        }
    }
}
impl std::error::Error for DomainError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date { year: u16, month: u8, day: u8 }
impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> DomainResult<Self> {
        if year == 0 || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return Err(DomainError::InvalidDate(format!("invalid date {year:04}-{month:02}-{day:02}")));
        }
        Ok(Self { year, month, day })
    }
    #[must_use] pub const fn year(self) -> u16 { self.year }
    #[must_use] pub const fn month(self) -> u8 { self.month }
    #[must_use] pub const fn day(self) -> u8 { self.day }
    #[must_use] pub fn iso(self) -> String { format!("{:04}-{:02}-{:02}", self.year, self.month, self.day) }
}
const fn leap(year: u16) -> bool { (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 }
const fn days_in_month(year: u16, month: u8) -> u8 {
    match month { 1|3|5|7|8|10|12 => 31, 4|6|9|11 => 30, 2 if leap(year) => 29, 2 => 28, _ => 0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaxYear { start_year: u16 }
impl TaxYear {
    pub fn new(start_year: u16) -> DomainResult<Self> {
        if start_year == 0 || start_year == u16::MAX {
            return Err(DomainError::InvalidDate("tax year must permit a following year".into()));
        }
        Ok(Self { start_year })
    }
    #[must_use] pub const fn start_year(self) -> u16 { self.start_year }
    pub fn start_date(self) -> DomainResult<Date> { Date::new(self.start_year, 4, 6) }
    pub fn end_date(self) -> DomainResult<Date> { Date::new(self.start_year + 1, 4, 5) }
    #[must_use] pub fn label(self) -> String { format!("{}-{:02}", self.start_year, (self.start_year + 1) % 100) }
    pub fn contains(self, d: Date) -> DomainResult<bool> { Ok(d >= self.start_date()? && d <= self.end_date()?) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountingBasis { #[default] Cash, Traditional }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GbpAmount { minor: i64 }
impl GbpAmount {
    pub fn positive_minor(minor: i64) -> DomainResult<Self> {
        if minor <= 0 { return Err(DomainError::InvalidAmount("amount must be positive whole pence".into())); }
        Ok(Self { minor })
    }
    #[must_use] pub const fn minor(self) -> i64 { self.minor }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusinessUse { Business, Private, Mixed { business_basis_points: u16 } }
impl BusinessUse {
    pub fn mixed(points: u16) -> DomainResult<Self> {
        if points == 0 || points >= BASIS_POINTS_DENOMINATOR {
            return Err(DomainError::InvalidBusinessUse("mixed use must be 1-9999 basis points".into()));
        }
        Ok(Self::Mixed { business_basis_points: points })
    }
    pub fn split(self, amount: GbpAmount) -> DomainResult<(i64, i64)> {
        let total = i128::from(amount.minor());
        let business = match self {
            Self::Business => total,
            Self::Private => 0,
            Self::Mixed { business_basis_points } => total.checked_mul(i128::from(business_basis_points))
                .ok_or(DomainError::ArithmeticOverflow)? / i128::from(BASIS_POINTS_DENOMINATOR),
        };
        let private = total.checked_sub(business).ok_or(DomainError::ArithmeticOverflow)?;
        Ok((i64::try_from(business).map_err(|_| DomainError::ArithmeticOverflow)?,
            i64::try_from(private).map_err(|_| DomainError::ArithmeticOverflow)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordId(String);
impl RecordId {
    pub fn new(value: impl Into<String>) -> DomainResult<Self> {
        let v = value.into(); let t = v.trim();
        if t.is_empty() || t.len() > 128 { return Err(DomainError::InvalidId("record id must be 1-128 non-whitespace characters".into())); }
        Ok(Self(t.into()))
    }
    #[must_use] pub fn as_str(&self) -> &str { &self.0 }
}
fn valid_name(value: impl Into<String>) -> DomainResult<String> {
    let v = value.into(); let t = v.trim();
    if t.is_empty() || t.len() > 256 { return Err(DomainError::InvalidName("name/description must be 1-256 non-whitespace characters".into())); }
    Ok(t.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind { Manual, Csv, Ofx, Qfx, Document, Adapter }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProvenance { kind: SourceKind, source_reference: Option<String>, fingerprint: Option<String>, label: Option<String> }
impl SourceProvenance {
    #[must_use] pub const fn manual() -> Self { Self { kind: SourceKind::Manual, source_reference: None, fingerprint: None, label: None } }
    pub fn new(kind: SourceKind, source_reference: Option<String>, fingerprint: Option<String>, label: Option<String>) -> DomainResult<Self> {
        for (name, value) in [("source reference", source_reference.as_deref()), ("fingerprint", fingerprint.as_deref()), ("label", label.as_deref())] {
            if value.is_some_and(|v| v.trim().is_empty()) { return Err(DomainError::InvalidProvenance(format!("{name} must not be blank"))); }
        }
        Ok(Self { kind, source_reference, fingerprint, label })
    }
    #[must_use] pub const fn kind(&self) -> SourceKind { self.kind }
    #[must_use] pub fn source_reference(&self) -> Option<&str> { self.source_reference.as_deref() }
    #[must_use] pub fn fingerprint(&self) -> Option<&str> { self.fingerprint.as_deref() }
    #[must_use] pub fn label(&self) -> Option<&str> { self.label.as_deref() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType { Asset, Liability, Equity, Revenue, Expense }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultAccount { pub code: &'static str, pub name: &'static str, pub account_type: AccountType }
pub const DEFAULT_CHART: &[DefaultAccount] = &[
    DefaultAccount{code:"1000",name:"Business Bank",account_type:AccountType::Asset},
    DefaultAccount{code:"1010",name:"Cash",account_type:AccountType::Asset},
    DefaultAccount{code:"1100",name:"Accounts Receivable",account_type:AccountType::Asset},
    DefaultAccount{code:"2000",name:"Accounts Payable",account_type:AccountType::Liability},
    DefaultAccount{code:"2100",name:"Business Loans",account_type:AccountType::Liability},
    DefaultAccount{code:"3000",name:"Owner Capital",account_type:AccountType::Equity},
    DefaultAccount{code:"3100",name:"Owner Drawings",account_type:AccountType::Equity},
    DefaultAccount{code:"4000",name:"Sales / Trading Income",account_type:AccountType::Revenue},
    DefaultAccount{code:"4100",name:"Other Business Income",account_type:AccountType::Revenue},
    DefaultAccount{code:"5000",name:"Goods / Stock / Materials",account_type:AccountType::Expense},
    DefaultAccount{code:"5100",name:"Office / Phone / Software",account_type:AccountType::Expense},
    DefaultAccount{code:"5200",name:"Travel",account_type:AccountType::Expense},
    DefaultAccount{code:"5300",name:"Vehicle Costs",account_type:AccountType::Expense},
    DefaultAccount{code:"5400",name:"Premises / Utilities",account_type:AccountType::Expense},
    DefaultAccount{code:"5500",name:"Advertising / Marketing",account_type:AccountType::Expense},
    DefaultAccount{code:"5600",name:"Bank / Finance / Insurance",account_type:AccountType::Expense},
    DefaultAccount{code:"5700",name:"Professional Fees",account_type:AccountType::Expense},
    DefaultAccount{code:"5800",name:"Repairs / Maintenance",account_type:AccountType::Expense},
    DefaultAccount{code:"5900",name:"Training",account_type:AccountType::Expense},
    DefaultAccount{code:"5950",name:"Staff / Subcontractor Costs",account_type:AccountType::Expense},
    DefaultAccount{code:"5990",name:"Other Business Expense",account_type:AccountType::Expense},
    DefaultAccount{code:"9000",name:"Bank Import Suspense / Needs Review",account_type:AccountType::Asset},
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomeCategory { SalesTrading, OtherBusinessIncome }
impl IncomeCategory { #[must_use] pub const fn account_code(self) -> &'static str { match self { Self::SalesTrading=>"4000", Self::OtherBusinessIncome=>"4100" } } }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpenseCategory { GoodsStockMaterials, OfficePhoneSoftware, Travel, VehicleCosts, PremisesUtilities, AdvertisingMarketing, BankFinanceInsurance, ProfessionalFees, RepairsMaintenance, Training, StaffSubcontractorCosts, OtherBusinessExpense }
impl ExpenseCategory { #[must_use] pub const fn account_code(self) -> &'static str { match self {
    Self::GoodsStockMaterials=>"5000", Self::OfficePhoneSoftware=>"5100", Self::Travel=>"5200", Self::VehicleCosts=>"5300",
    Self::PremisesUtilities=>"5400", Self::AdvertisingMarketing=>"5500", Self::BankFinanceInsurance=>"5600", Self::ProfessionalFees=>"5700",
    Self::RepairsMaintenance=>"5800", Self::Training=>"5900", Self::StaffSubcontractorCosts=>"5950", Self::OtherBusinessExpense=>"5990" } } }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementAccount { BusinessBank, Cash }
impl SettlementAccount { #[must_use] pub const fn account_code(self) -> &'static str { match self { Self::BusinessBank=>"1000", Self::Cash=>"1010" } } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostingDirection { Debit, Credit }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingLinePlan { account_code: &'static str, direction: PostingDirection, amount_minor: i64 }
impl PostingLinePlan {
    #[must_use] pub const fn account_code(&self)->&'static str{self.account_code}
    #[must_use] pub const fn direction(&self)->PostingDirection{self.direction}
    #[must_use] pub const fn amount_minor(&self)->i64{self.amount_minor}
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingPlan { description: String, date: Date, lines: Vec<PostingLinePlan> }
impl PostingPlan {
    #[must_use] pub fn description(&self)->&str{&self.description}
    #[must_use] pub const fn date(&self)->Date{self.date}
    #[must_use] pub fn lines(&self)->&[PostingLinePlan]{&self.lines}
    #[must_use] pub fn total_debits(&self)->i128{self.lines.iter().filter(|x|x.direction==PostingDirection::Debit).map(|x|i128::from(x.amount_minor)).sum()}
    #[must_use] pub fn total_credits(&self)->i128{self.lines.iter().filter(|x|x.direction==PostingDirection::Credit).map(|x|i128::from(x.amount_minor)).sum()}
    #[must_use] pub fn is_balanced(&self)->bool{!self.lines.is_empty()&&self.lines.iter().all(|x|x.amount_minor>0)&&self.total_debits()==self.total_credits()}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookDomainSettings { currency_code: &'static str, accounting_basis: AccountingBasis }
impl Default for BookDomainSettings { fn default()->Self{Self{currency_code:DEFAULT_CURRENCY_CODE,accounting_basis:AccountingBasis::Cash}} }
impl BookDomainSettings { #[must_use] pub const fn currency_code(&self)->&'static str{self.currency_code} #[must_use] pub const fn accounting_basis(&self)->AccountingBasis{self.accounting_basis} }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer { id: RecordId, display_name: String }
impl Customer { pub fn new(id:RecordId,name:impl Into<String>)->DomainResult<Self>{Ok(Self{id,display_name:valid_name(name)?})} #[must_use] pub fn id(&self)->&RecordId{&self.id} #[must_use] pub fn display_name(&self)->&str{&self.display_name} }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supplier { id: RecordId, display_name: String }
impl Supplier { pub fn new(id:RecordId,name:impl Into<String>)->DomainResult<Self>{Ok(Self{id,display_name:valid_name(name)?})} #[must_use] pub fn id(&self)->&RecordId{&self.id} #[must_use] pub fn display_name(&self)->&str{&self.display_name} }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceStatus { Draft, Issued, Paid, Cancelled }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice { id:RecordId, customer_id:RecordId, issue_date:Date, due_date:Option<Date>, total:GbpAmount, status:InvoiceStatus }
impl Invoice {
    pub fn new(id:RecordId,customer_id:RecordId,issue_date:Date,due_date:Option<Date>,total:GbpAmount,status:InvoiceStatus)->DomainResult<Self>{
        if due_date.is_some_and(|d|d<issue_date){return Err(DomainError::InvalidInvoiceDates("invoice due date cannot be before issue date".into()));}
        Ok(Self{id,customer_id,issue_date,due_date,total,status})
    }
    #[must_use] pub fn id(&self)->&RecordId{&self.id} #[must_use] pub fn customer_id(&self)->&RecordId{&self.customer_id}
    #[must_use] pub const fn issue_date(&self)->Date{self.issue_date} #[must_use] pub const fn due_date(&self)->Option<Date>{self.due_date}
    #[must_use] pub const fn total(&self)->GbpAmount{self.total} #[must_use] pub const fn status(&self)->InvoiceStatus{self.status}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentDirection { Inbound, Outbound }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment { id:RecordId,date:Date,amount:GbpAmount,direction:PaymentDirection,settlement:SettlementAccount,counterparty_id:Option<RecordId>,provenance:SourceProvenance }
impl Payment {
    #[must_use] pub fn new(id:RecordId,date:Date,amount:GbpAmount,direction:PaymentDirection,settlement:SettlementAccount,counterparty_id:Option<RecordId>,provenance:SourceProvenance)->Self{Self{id,date,amount,direction,settlement,counterparty_id,provenance}}
    #[must_use] pub fn id(&self)->&RecordId{&self.id} #[must_use] pub const fn amount(&self)->GbpAmount{self.amount} #[must_use] pub fn provenance(&self)->&SourceProvenance{&self.provenance}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomeRecord { id:RecordId,description:String,date:Date,amount:GbpAmount,category:IncomeCategory,settlement:SettlementAccount,provenance:SourceProvenance }
impl IncomeRecord { pub fn new(id:RecordId,description:impl Into<String>,date:Date,amount:GbpAmount,category:IncomeCategory,settlement:SettlementAccount,provenance:SourceProvenance)->DomainResult<Self>{Ok(Self{id,description:valid_name(description)?,date,amount,category,settlement,provenance})} }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseRecord { id:RecordId,description:String,date:Date,amount:GbpAmount,category:ExpenseCategory,business_use:BusinessUse,settlement:SettlementAccount,supplier_id:Option<RecordId>,provenance:SourceProvenance }
impl ExpenseRecord { #[allow(clippy::too_many_arguments)] pub fn new(id:RecordId,description:impl Into<String>,date:Date,amount:GbpAmount,category:ExpenseCategory,business_use:BusinessUse,settlement:SettlementAccount,supplier_id:Option<RecordId>,provenance:SourceProvenance)->DomainResult<Self>{Ok(Self{id,description:valid_name(description)?,date,amount,category,business_use,settlement,supplier_id,provenance})} }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerContribution { id:RecordId,description:String,date:Date,amount:GbpAmount,settlement:SettlementAccount,provenance:SourceProvenance }
impl OwnerContribution { pub fn new(id:RecordId,description:impl Into<String>,date:Date,amount:GbpAmount,settlement:SettlementAccount,provenance:SourceProvenance)->DomainResult<Self>{Ok(Self{id,description:valid_name(description)?,date,amount,settlement,provenance})} }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerDrawing { id:RecordId,description:String,date:Date,amount:GbpAmount,settlement:SettlementAccount,provenance:SourceProvenance }
impl OwnerDrawing { pub fn new(id:RecordId,description:impl Into<String>,date:Date,amount:GbpAmount,settlement:SettlementAccount,provenance:SourceProvenance)->DomainResult<Self>{Ok(Self{id,description:valid_name(description)?,date,amount,settlement,provenance})} }

#[must_use]
pub fn plan_income(r:&IncomeRecord)->PostingPlan{let a=r.amount.minor();PostingPlan{description:r.description.clone(),date:r.date,lines:vec![PostingLinePlan{account_code:r.settlement.account_code(),direction:PostingDirection::Debit,amount_minor:a},PostingLinePlan{account_code:r.category.account_code(),direction:PostingDirection::Credit,amount_minor:a}]}}
pub fn plan_expense(r:&ExpenseRecord)->DomainResult<PostingPlan>{let(b,p)=r.business_use.split(r.amount)?;let mut lines=Vec::with_capacity(3);if b>0{lines.push(PostingLinePlan{account_code:r.category.account_code(),direction:PostingDirection::Debit,amount_minor:b});}if p>0{lines.push(PostingLinePlan{account_code:"3100",direction:PostingDirection::Debit,amount_minor:p});}lines.push(PostingLinePlan{account_code:r.settlement.account_code(),direction:PostingDirection::Credit,amount_minor:r.amount.minor()});Ok(PostingPlan{description:r.description.clone(),date:r.date,lines})}
#[must_use]
pub fn plan_owner_contribution(r:&OwnerContribution)->PostingPlan{let a=r.amount.minor();PostingPlan{description:r.description.clone(),date:r.date,lines:vec![PostingLinePlan{account_code:r.settlement.account_code(),direction:PostingDirection::Debit,amount_minor:a},PostingLinePlan{account_code:"3000",direction:PostingDirection::Credit,amount_minor:a}]}}
#[must_use]
pub fn plan_owner_drawing(r:&OwnerDrawing)->PostingPlan{let a=r.amount.minor();PostingPlan{description:r.description.clone(),date:r.date,lines:vec![PostingLinePlan{account_code:"3100",direction:PostingDirection::Debit,amount_minor:a},PostingLinePlan{account_code:r.settlement.account_code(),direction:PostingDirection::Credit,amount_minor:a}]}}

#[cfg(test)]
mod tests {
    use super::*; use std::collections::HashSet;
    fn d()->Date{Date::new(2026,9,8).unwrap()} fn id(s:&str)->RecordId{RecordId::new(s).unwrap()} fn a(v:i64)->GbpAmount{GbpAmount::positive_minor(v).unwrap()}
    #[test] fn exact_gbp_pence_are_preserved(){assert_eq!(a(1234).minor(),1234);}
    #[test] fn zero_and_negative_amounts_rejected(){assert!(GbpAmount::positive_minor(0).is_err());assert!(GbpAmount::positive_minor(-1).is_err());}
    #[test] fn tax_year_boundaries(){let y=TaxYear::new(2026).unwrap();assert_eq!(y.start_date().unwrap(),Date::new(2026,4,6).unwrap());assert_eq!(y.end_date().unwrap(),Date::new(2027,4,5).unwrap());assert!(y.contains(Date::new(2027,4,5).unwrap()).unwrap());assert!(!y.contains(Date::new(2026,4,5).unwrap()).unwrap());assert_eq!(y.label(),"2026-27");}
    #[test] fn leap_year_validation(){assert!(Date::new(2024,2,29).is_ok());assert!(Date::new(2025,2,29).is_err());}
    #[test] fn defaults_are_gbp_cash(){let s=BookDomainSettings::default();assert_eq!(s.currency_code(),"GBP");assert_eq!(s.accounting_basis(),AccountingBasis::Cash);}
    #[test] fn mixed_use_endpoints_rejected(){assert!(BusinessUse::mixed(0).is_err());assert!(BusinessUse::mixed(10_000).is_err());}
    #[test] fn mixed_use_integer_remainder_private(){let(b,p)=BusinessUse::mixed(3500).unwrap().split(a(1001)).unwrap();assert_eq!((b,p),(350,651));}
    #[test] fn private_purchase_is_drawings_not_expense(){let e=ExpenseRecord::new(id("e1"),"Private",d(),a(2500),ExpenseCategory::OfficePhoneSoftware,BusinessUse::Private,SettlementAccount::BusinessBank,None,SourceProvenance::manual()).unwrap();let p=plan_expense(&e).unwrap();assert!(p.is_balanced());assert!(p.lines.iter().any(|x|x.account_code=="3100"));assert!(!p.lines.iter().any(|x|x.account_code=="5100"));}
    #[test] fn mixed_use_purchase_splits_expense_and_drawings(){let e=ExpenseRecord::new(id("e2"),"Mixed",d(),a(1001),ExpenseCategory::OfficePhoneSoftware,BusinessUse::mixed(3500).unwrap(),SettlementAccount::BusinessBank,None,SourceProvenance::manual()).unwrap();let p=plan_expense(&e).unwrap();assert!(p.is_balanced());assert!(p.lines.iter().any(|x|x.account_code=="5100"&&x.amount_minor==350));assert!(p.lines.iter().any(|x|x.account_code=="3100"&&x.amount_minor==651));}
    #[test] fn business_income_balances(){let r=IncomeRecord::new(id("i1"),"Sale",d(),a(12345),IncomeCategory::SalesTrading,SettlementAccount::BusinessBank,SourceProvenance::manual()).unwrap();assert!(plan_income(&r).is_balanced());}
    #[test] fn business_expense_balances(){let e=ExpenseRecord::new(id("e3"),"Stationery",d(),a(4723),ExpenseCategory::OfficePhoneSoftware,BusinessUse::Business,SettlementAccount::BusinessBank,None,SourceProvenance::manual()).unwrap();assert!(plan_expense(&e).unwrap().is_balanced());}
    #[test] fn owner_contribution_is_equity_not_revenue(){let r=OwnerContribution::new(id("c1"),"Introduced",d(),a(50000),SettlementAccount::BusinessBank,SourceProvenance::manual()).unwrap();let p=plan_owner_contribution(&r);assert!(p.lines.iter().any(|x|x.account_code=="3000"&&x.direction==PostingDirection::Credit));assert!(!p.lines.iter().any(|x|x.account_code.starts_with('4')));}
    #[test] fn owner_drawing_is_equity_not_expense(){let r=OwnerDrawing::new(id("w1"),"Taken",d(),a(10000),SettlementAccount::BusinessBank,SourceProvenance::manual()).unwrap();let p=plan_owner_drawing(&r);assert!(p.lines.iter().any(|x|x.account_code=="3100"&&x.direction==PostingDirection::Debit));assert!(!p.lines.iter().any(|x|x.account_code.starts_with('5')));}
    #[test] fn chart_codes_unique(){let s:HashSet<_>=DEFAULT_CHART.iter().map(|x|x.code).collect();assert_eq!(s.len(),DEFAULT_CHART.len());}
    #[test] fn expense_categories_are_expense_accounts(){for c in [ExpenseCategory::GoodsStockMaterials,ExpenseCategory::OfficePhoneSoftware,ExpenseCategory::Travel,ExpenseCategory::VehicleCosts,ExpenseCategory::PremisesUtilities,ExpenseCategory::AdvertisingMarketing,ExpenseCategory::BankFinanceInsurance,ExpenseCategory::ProfessionalFees,ExpenseCategory::RepairsMaintenance,ExpenseCategory::Training,ExpenseCategory::StaffSubcontractorCosts,ExpenseCategory::OtherBusinessExpense]{assert_eq!(DEFAULT_CHART.iter().find(|x|x.code==c.account_code()).unwrap().account_type,AccountType::Expense);}}
    #[test] fn owner_accounts_are_equity(){for code in ["3000","3100"]{assert_eq!(DEFAULT_CHART.iter().find(|x|x.code==code).unwrap().account_type,AccountType::Equity);}}
    #[test] fn suspense_is_asset(){assert_eq!(DEFAULT_CHART.iter().find(|x|x.code=="9000").unwrap().account_type,AccountType::Asset);}
    #[test] fn blank_ids_and_names_rejected(){assert!(RecordId::new("  ").is_err());assert!(Customer::new(id("c")," ").is_err());assert!(Supplier::new(id("s"),"").is_err());}
    #[test] fn invoice_date_order_enforced(){let issue=d();assert!(Invoice::new(id("inv"),id("c"),issue,Some(Date::new(2026,9,7).unwrap()),a(1000),InvoiceStatus::Issued).is_err());assert!(Invoice::new(id("inv2"),id("c"),issue,Some(issue),a(1000),InvoiceStatus::Issued).is_ok());}
    #[test] fn provenance_rejects_blanks(){assert!(SourceProvenance::new(SourceKind::Csv,Some(" ".into()),None,None).is_err());assert!(SourceProvenance::new(SourceKind::Ofx,Some("ofx:abc".into()),None,None).is_ok());}
    #[test] fn payment_preserves_provenance(){let p=Payment::new(id("p1"),d(),a(1500),PaymentDirection::Inbound,SettlementAccount::BusinessBank,Some(id("c")),SourceProvenance::new(SourceKind::Csv,Some("file:12".into()),Some("sha256:abc".into()),None).unwrap());assert_eq!(p.amount().minor(),1500);assert_eq!(p.provenance().kind(),SourceKind::Csv);}
    #[test] fn posting_plans_never_contain_zero_or_negative_lines(){let e=ExpenseRecord::new(id("e4"),"Private",d(),a(1),ExpenseCategory::Travel,BusinessUse::Private,SettlementAccount::Cash,None,SourceProvenance::manual()).unwrap();let p=plan_expense(&e).unwrap();assert!(p.lines.iter().all(|x|x.amount_minor>0));assert!(p.is_balanced());}
}
