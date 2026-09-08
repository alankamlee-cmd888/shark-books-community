//! SBC-3 deterministic bank-file ingestion contracts.
//!
//! This module is platform-neutral and persistence-free. It turns CSV and
//! OFX/QFX text into immutable canonical `BankLine` values for preview. It does
//! not write the ledger, open network connections, or infer accounting
//! categories.

use crate::{Date, RecordId, SourceKind, SourceProvenance, DEFAULT_CURRENCY_CODE};
use std::fmt;
use std::fmt::Write as _;

pub type BankImportResult<T> = Result<T, BankImportError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BankImportError {
    InvalidProfile(String),
    InvalidCsv(String),
    MissingColumn(String),
    InvalidDate(String),
    InvalidAmount(String),
    UnsupportedCurrency(String),
    InvalidOfx(String),
    InvalidHash(String),
    InvalidField(String),
    ConflictingStrongIdentity(String),
}
impl fmt::Display for BankImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(s)
            | Self::InvalidCsv(s)
            | Self::MissingColumn(s)
            | Self::InvalidDate(s)
            | Self::InvalidAmount(s)
            | Self::UnsupportedCurrency(s)
            | Self::InvalidOfx(s)
            | Self::InvalidHash(s)
            | Self::InvalidField(s)
            | Self::ConflictingStrongIdentity(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for BankImportError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BankSourceFormat { Csv, Ofx, Qfx }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportState { Preview }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateCertainty { Strong, FileExact, Heuristic, Distinct }
impl DuplicateCertainty {
    #[must_use]
    pub const fn may_auto_suppress(self) -> bool {
        matches!(self, Self::Strong | Self::FileExact)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankLine {
    source_account_id: RecordId,
    institution_account_id: Option<String>,
    source_format: BankSourceFormat,
    source_file_sha256: String,
    source_locator: String,
    posted_date: Date,
    value_date: Option<Date>,
    signed_amount_minor: i64,
    currency_code: &'static str,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    external_transaction_id: Option<String>,
    raw_record_sha256: String,
    import_state: ImportState,
    provenance: SourceProvenance,
}
impl BankLine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_account_id: RecordId,
        institution_account_id: Option<String>,
        source_format: BankSourceFormat,
        source_file_sha256: impl Into<String>,
        source_locator: impl Into<String>,
        posted_date: Date,
        value_date: Option<Date>,
        signed_amount_minor: i64,
        description: impl Into<String>,
        payee: Option<String>,
        reference: Option<String>,
        external_transaction_id: Option<String>,
        raw_record_sha256: impl Into<String>,
        provenance: SourceProvenance,
    ) -> BankImportResult<Self> {
        if signed_amount_minor == 0 {
            return Err(BankImportError::InvalidAmount(
                "bank line amount must be non-zero whole pence".into(),
            ));
        }
        let source_file_sha256 = source_file_sha256.into();
        require_sha256(&source_file_sha256, "source file SHA-256")?;
        let raw_record_sha256 = raw_record_sha256.into();
        require_sha256(&raw_record_sha256, "raw record SHA-256")?;
        let source_locator = nonblank(source_locator.into(), "source locator")?;
        let description = nonblank(description.into(), "description")?;
        let institution_account_id = optional_nonblank(institution_account_id, "institution account id")?;
        let payee = optional_nonblank(payee, "payee")?;
        let reference = optional_nonblank(reference, "reference")?;
        let external_transaction_id = optional_nonblank(external_transaction_id, "external transaction id")?;
        Ok(Self {
            source_account_id,
            institution_account_id,
            source_format,
            source_file_sha256,
            source_locator,
            posted_date,
            value_date,
            signed_amount_minor,
            currency_code: DEFAULT_CURRENCY_CODE,
            description,
            payee,
            reference,
            external_transaction_id,
            raw_record_sha256,
            import_state: ImportState::Preview,
            provenance,
        })
    }

    #[must_use] pub fn source_account_id(&self) -> &RecordId { &self.source_account_id }
    #[must_use] pub fn institution_account_id(&self) -> Option<&str> { self.institution_account_id.as_deref() }
    #[must_use] pub const fn source_format(&self) -> BankSourceFormat { self.source_format }
    #[must_use] pub fn source_file_sha256(&self) -> &str { &self.source_file_sha256 }
    #[must_use] pub fn source_locator(&self) -> &str { &self.source_locator }
    #[must_use] pub const fn posted_date(&self) -> Date { self.posted_date }
    #[must_use] pub const fn value_date(&self) -> Option<Date> { self.value_date }
    #[must_use] pub const fn signed_amount_minor(&self) -> i64 { self.signed_amount_minor }
    #[must_use] pub const fn currency_code(&self) -> &'static str { self.currency_code }
    #[must_use] pub fn description(&self) -> &str { &self.description }
    #[must_use] pub fn payee(&self) -> Option<&str> { self.payee.as_deref() }
    #[must_use] pub fn reference(&self) -> Option<&str> { self.reference.as_deref() }
    #[must_use] pub fn external_transaction_id(&self) -> Option<&str> { self.external_transaction_id.as_deref() }
    #[must_use] pub fn raw_record_sha256(&self) -> &str { &self.raw_record_sha256 }
    #[must_use] pub const fn import_state(&self) -> ImportState { self.import_state }
    #[must_use] pub fn provenance(&self) -> &SourceProvenance { &self.provenance }

    #[must_use]
    pub fn strong_identity_key(&self) -> Option<String> {
        let external = self.external_transaction_id.as_deref()?;
        match self.source_format {
            BankSourceFormat::Csv => Some(format!(
                "csv:{}:{}:{}",
                self.currency_code,
                self.source_account_id.as_str(),
                external
            )),
            BankSourceFormat::Ofx | BankSourceFormat::Qfx => {
                let account = self.institution_account_id.as_deref()?;
                Some(format!("ofx:{}:{}:{}", self.currency_code, account, external))
            }
        }
    }

    pub fn duplicate_certainty(&self, other: &Self) -> BankImportResult<DuplicateCertainty> {
        if let (Some(a), Some(b)) = (self.strong_identity_key(), other.strong_identity_key()) {
            if a == b {
                if self.source_account_id != other.source_account_id
                    || self.signed_amount_minor != other.signed_amount_minor
                    || self.posted_date != other.posted_date
                    || self.currency_code != other.currency_code
                {
                    return Err(BankImportError::ConflictingStrongIdentity(format!(
                        "strong source identity {a} has conflicting accounting content"
                    )));
                }
                return Ok(DuplicateCertainty::Strong);
            }
        }
        if self.source_file_sha256 == other.source_file_sha256
            && self.source_locator == other.source_locator
            && self.raw_record_sha256 == other.raw_record_sha256
        {
            return Ok(DuplicateCertainty::FileExact);
        }
        if self.source_account_id == other.source_account_id
            && self.signed_amount_minor == other.signed_amount_minor
            && self.posted_date == other.posted_date
            && normalize_match_text(&self.description) == normalize_match_text(&other.description)
        {
            return Ok(DuplicateCertainty::Heuristic);
        }
        Ok(DuplicateCertainty::Distinct)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewError { locator: String, message: String }
impl PreviewError {
    #[must_use] pub fn locator(&self) -> &str { &self.locator }
    #[must_use] pub fn message(&self) -> &str { &self.message }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankImportPreview { lines: Vec<BankLine>, errors: Vec<PreviewError> }
impl BankImportPreview {
    #[must_use] pub fn lines(&self) -> &[BankLine] { &self.lines }
    #[must_use] pub fn errors(&self) -> &[PreviewError] { &self.errors }
    #[must_use] pub fn can_commit(&self) -> bool { !self.lines.is_empty() && self.errors.is_empty() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvDateFormat { IsoYmd, DmySlash }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsvAmountMapping {
    Signed { amount_header: String },
    DebitCredit { debit_header: String, credit_header: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvMappingProfile {
    id: RecordId,
    name: String,
    delimiter: char,
    date_header: String,
    value_date_header: Option<String>,
    description_header: String,
    payee_header: Option<String>,
    reference_header: Option<String>,
    transaction_id_header: Option<String>,
    currency_header: Option<String>,
    amount_mapping: CsvAmountMapping,
    date_format: CsvDateFormat,
}
impl CsvMappingProfile {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: RecordId,
        name: impl Into<String>,
        delimiter: char,
        date_header: impl Into<String>,
        value_date_header: Option<String>,
        description_header: impl Into<String>,
        payee_header: Option<String>,
        reference_header: Option<String>,
        transaction_id_header: Option<String>,
        currency_header: Option<String>,
        amount_mapping: CsvAmountMapping,
        date_format: CsvDateFormat,
    ) -> BankImportResult<Self> {
        if !delimiter.is_ascii() || matches!(delimiter, '\r' | '\n' | '"') {
            return Err(BankImportError::InvalidProfile(
                "CSV delimiter must be one safe ASCII character".into(),
            ));
        }
        let name = nonblank(name.into(), "CSV profile name")?;
        let date_header = nonblank(date_header.into(), "date header")?;
        let description_header = nonblank(description_header.into(), "description header")?;
        let value_date_header = optional_nonblank(value_date_header, "value-date header")?;
        let payee_header = optional_nonblank(payee_header, "payee header")?;
        let reference_header = optional_nonblank(reference_header, "reference header")?;
        let transaction_id_header = optional_nonblank(transaction_id_header, "transaction-id header")?;
        let currency_header = optional_nonblank(currency_header, "currency header")?;
        let amount_mapping = match amount_mapping {
            CsvAmountMapping::Signed { amount_header } => CsvAmountMapping::Signed {
                amount_header: nonblank(amount_header, "amount header")?,
            },
            CsvAmountMapping::DebitCredit { debit_header, credit_header } => {
                let debit_header = nonblank(debit_header, "debit header")?;
                let credit_header = nonblank(credit_header, "credit header")?;
                if debit_header.eq_ignore_ascii_case(&credit_header) {
                    return Err(BankImportError::InvalidProfile(
                        "debit and credit headers must be different".into(),
                    ));
                }
                CsvAmountMapping::DebitCredit { debit_header, credit_header }
            }
        };
        Ok(Self {
            id, name, delimiter, date_header, value_date_header, description_header,
            payee_header, reference_header, transaction_id_header, currency_header,
            amount_mapping, date_format,
        })
    }
    #[must_use] pub fn id(&self) -> &RecordId { &self.id }
    #[must_use] pub fn name(&self) -> &str { &self.name }
    #[must_use] pub const fn delimiter(&self) -> char { self.delimiter }
}

pub fn preview_csv(
    text: &str,
    source_account_id: RecordId,
    profile: &CsvMappingProfile,
) -> BankImportResult<BankImportPreview> {
    let records = parse_csv_records(text, profile.delimiter)?;
    if records.is_empty() {
        return Err(BankImportError::InvalidCsv("CSV contains no records".into()));
    }
    let headers = &records[0].fields;
    if headers.is_empty() {
        return Err(BankImportError::InvalidCsv("CSV header is empty".into()));
    }
    let map = ResolvedCsvProfile::resolve(headers, profile)?;
    let file_sha = sha256_hex(text.as_bytes());
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (index, record) in records.iter().enumerate().skip(1) {
        if record.fields.iter().all(|v| v.trim().is_empty()) { continue; }
        let locator = format!("csv:row:{}", index + 1);
        match bank_line_from_csv_record(record, &locator, &file_sha, source_account_id.clone(), profile, &map) {
            Ok(line) => lines.push(line),
            Err(error) => errors.push(PreviewError { locator, message: error.to_string() }),
        }
    }
    Ok(BankImportPreview { lines, errors })
}

#[derive(Debug)]
struct CsvRecord { fields: Vec<String>, raw: String }

fn parse_csv_records(text: &str, delimiter: char) -> BankImportResult<Vec<CsvRecord>> {
    let mut records = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut raw = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    let mut at_field_start = true;

    while let Some(ch) = chars.next() {
        if in_quotes {
            raw.push(ch);
            if ch == '"' {
                if chars.peek().copied() == Some('"') {
                    raw.push('"');
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }

        if ch == '"' {
            if !at_field_start {
                return Err(BankImportError::InvalidCsv(
                    "quote appeared in the middle of an unquoted field".into(),
                ));
            }
            raw.push(ch);
            in_quotes = true;
            at_field_start = false;
        } else if ch == delimiter {
            raw.push(ch);
            fields.push(std::mem::take(&mut field));
            at_field_start = true;
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek().copied() == Some('\n') { chars.next(); }
            fields.push(std::mem::take(&mut field));
            if !raw.is_empty() || fields.iter().any(|v| !v.is_empty()) {
                records.push(CsvRecord { fields: std::mem::take(&mut fields), raw: std::mem::take(&mut raw) });
            } else {
                fields.clear(); raw.clear();
            }
            at_field_start = true;
        } else {
            raw.push(ch); field.push(ch); at_field_start = false;
        }
    }
    if in_quotes {
        return Err(BankImportError::InvalidCsv("CSV ended inside a quoted field".into()));
    }
    if !field.is_empty() || !fields.is_empty() || !raw.is_empty() {
        fields.push(field);
        records.push(CsvRecord { fields, raw });
    }
    if let Some(first) = records.first_mut().and_then(|r| r.fields.first_mut()) {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    Ok(records)
}

struct ResolvedCsvProfile {
    date: usize,
    value_date: Option<usize>,
    description: usize,
    payee: Option<usize>,
    reference: Option<usize>,
    transaction_id: Option<usize>,
    currency: Option<usize>,
    amount: ResolvedAmount,
}
enum ResolvedAmount { Signed(usize), DebitCredit { debit: usize, credit: usize } }
impl ResolvedCsvProfile {
    fn resolve(headers: &[String], profile: &CsvMappingProfile) -> BankImportResult<Self> {
        let date = header_index(headers, &profile.date_header)?;
        let value_date = optional_header_index(headers, profile.value_date_header.as_deref())?;
        let description = header_index(headers, &profile.description_header)?;
        let payee = optional_header_index(headers, profile.payee_header.as_deref())?;
        let reference = optional_header_index(headers, profile.reference_header.as_deref())?;
        let transaction_id = optional_header_index(headers, profile.transaction_id_header.as_deref())?;
        let currency = optional_header_index(headers, profile.currency_header.as_deref())?;
        let amount = match &profile.amount_mapping {
            CsvAmountMapping::Signed { amount_header } => ResolvedAmount::Signed(header_index(headers, amount_header)?),
            CsvAmountMapping::DebitCredit { debit_header, credit_header } => ResolvedAmount::DebitCredit {
                debit: header_index(headers, debit_header)?, credit: header_index(headers, credit_header)?,
            },
        };
        Ok(Self { date, value_date, description, payee, reference, transaction_id, currency, amount })
    }
}

fn header_index(headers: &[String], target: &str) -> BankImportResult<usize> {
    let matches: Vec<usize> = headers.iter().enumerate()
        .filter_map(|(i, h)| h.trim().eq_ignore_ascii_case(target.trim()).then_some(i)).collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(BankImportError::MissingColumn(format!("CSV column '{target}' was not found"))),
        _ => Err(BankImportError::InvalidCsv(format!("CSV column '{target}' occurs more than once"))),
    }
}
fn optional_header_index(headers: &[String], target: Option<&str>) -> BankImportResult<Option<usize>> {
    target.map(|t| header_index(headers, t)).transpose()
}
fn cell<'a>(record: &'a CsvRecord, index: usize, label: &str) -> BankImportResult<&'a str> {
    record.fields.get(index).map(String::as_str)
        .ok_or_else(|| BankImportError::InvalidCsv(format!("row is missing mapped {label} column")))
}
fn optional_cell(record: &CsvRecord, index: Option<usize>) -> BankImportResult<Option<String>> {
    match index {
        Some(i) => { let value = cell(record, i, "optional")?.trim(); Ok((!value.is_empty()).then(|| value.to_string())) }
        None => Ok(None),
    }
}

fn bank_line_from_csv_record(
    record: &CsvRecord,
    locator: &str,
    file_sha: &str,
    source_account_id: RecordId,
    profile: &CsvMappingProfile,
    map: &ResolvedCsvProfile,
) -> BankImportResult<BankLine> {
    let posted_date = parse_csv_date(cell(record, map.date, "date")?, profile.date_format)?;
    let value_date = match map.value_date {
        Some(i) => { let value = cell(record, i, "value date")?.trim(); if value.is_empty() { None } else { Some(parse_csv_date(value, profile.date_format)?) } }
        None => None,
    };
    let signed_amount_minor = match map.amount {
        ResolvedAmount::Signed(index) => parse_signed_minor(cell(record, index, "amount")?)?,
        ResolvedAmount::DebitCredit { debit, credit } => {
            let debit = parse_optional_positive_minor(cell(record, debit, "debit")?)?;
            let credit = parse_optional_positive_minor(cell(record, credit, "credit")?)?;
            match (debit, credit) {
                (Some(d), None) => d.checked_neg().ok_or_else(|| BankImportError::InvalidAmount("debit amount overflow".into()))?,
                (None, Some(c)) => c,
                _ => return Err(BankImportError::InvalidAmount("exactly one of debit or credit must contain a positive amount".into())),
            }
        }
    };
    if let Some(currency_index) = map.currency {
        let currency = cell(record, currency_index, "currency")?.trim();
        if !currency.eq_ignore_ascii_case(DEFAULT_CURRENCY_CODE) {
            return Err(BankImportError::UnsupportedCurrency(format!("V1 bank import supports GBP only, not '{currency}'")));
        }
    }
    let description = nonblank(cell(record, map.description, "description")?.to_string(), "description")?;
    let payee = optional_cell(record, map.payee)?;
    let reference = optional_cell(record, map.reference)?;
    let external_transaction_id = optional_cell(record, map.transaction_id)?;
    let raw_hash = sha256_hex(record.raw.as_bytes());
    let fingerprint = external_transaction_id.as_ref()
        .map(|id| format!("csv:{}:{}:{}", DEFAULT_CURRENCY_CODE, source_account_id.as_str(), id))
        .unwrap_or_else(|| format!("sha256:{raw_hash}"));
    let provenance = SourceProvenance::new(
        SourceKind::Csv, Some(locator.to_string()), Some(fingerprint), Some(profile.name.clone()),
    ).map_err(|e| BankImportError::InvalidField(e.to_string()))?;
    BankLine::new(
        source_account_id, None, BankSourceFormat::Csv, file_sha, locator, posted_date, value_date,
        signed_amount_minor, description, payee, reference, external_transaction_id, raw_hash, provenance,
    )
}

pub fn preview_ofx_or_qfx(
    text: &str,
    source_account_id: RecordId,
    format: BankSourceFormat,
) -> BankImportResult<BankImportPreview> {
    if !matches!(format, BankSourceFormat::Ofx | BankSourceFormat::Qfx) {
        return Err(BankImportError::InvalidOfx("OFX/QFX preview requires OFX or QFX source format".into()));
    }
    let currency = tag_value(text, "CURDEF").ok_or_else(|| BankImportError::InvalidOfx("OFX/QFX is missing CURDEF".into()))?;
    if !currency.eq_ignore_ascii_case(DEFAULT_CURRENCY_CODE) {
        return Err(BankImportError::UnsupportedCurrency(format!("V1 bank import supports GBP only, not '{currency}'")));
    }
    let institution_account_id = tag_value(text, "ACCTID").ok_or_else(|| BankImportError::InvalidOfx("OFX/QFX is missing ACCTID".into()))?;
    let blocks = find_blocks(text, "STMTTRN");
    if blocks.is_empty() {
        return Err(BankImportError::InvalidOfx("OFX/QFX contains no STMTTRN records".into()));
    }
    let file_sha = sha256_hex(text.as_bytes());
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let locator = format!("ofx:stmttrn:{}", index + 1);
        match bank_line_from_ofx_block(block, &locator, &file_sha, source_account_id.clone(), &institution_account_id, format) {
            Ok(line) => lines.push(line),
            Err(error) => errors.push(PreviewError { locator, message: error.to_string() }),
        }
    }
    Ok(BankImportPreview { lines, errors })
}

fn bank_line_from_ofx_block(
    block: &str,
    locator: &str,
    file_sha: &str,
    source_account_id: RecordId,
    institution_account_id: &str,
    format: BankSourceFormat,
) -> BankImportResult<BankLine> {
    let date_text = tag_value(block, "DTPOSTED").ok_or_else(|| BankImportError::InvalidOfx("STMTTRN is missing DTPOSTED".into()))?;
    let posted_date = parse_ofx_date(&date_text)?;
    let value_date = tag_value(block, "DTUSER").or_else(|| tag_value(block, "DTAVAIL"))
        .map(|v| parse_ofx_date(&v)).transpose()?;
    let amount_text = tag_value(block, "TRNAMT").ok_or_else(|| BankImportError::InvalidOfx("STMTTRN is missing TRNAMT".into()))?;
    let signed_amount_minor = parse_signed_minor(&amount_text)?;
    let fitid = nonblank(tag_value(block, "FITID").ok_or_else(|| BankImportError::InvalidOfx("STMTTRN is missing FITID".into()))?, "FITID")?;
    let name = tag_value(block, "NAME").and_then(blank_to_none);
    let memo = tag_value(block, "MEMO").and_then(blank_to_none);
    let description = match (&name, &memo) {
        (Some(n), Some(m)) if !n.eq_ignore_ascii_case(m) => format!("{n} - {m}"),
        (Some(n), _) => n.clone(),
        (_, Some(m)) => m.clone(),
        _ => format!("OFX transaction {fitid}"),
    };
    let reference = tag_value(block, "REFNUM").or_else(|| tag_value(block, "CHECKNUM")).and_then(blank_to_none);
    let raw_hash = sha256_hex(block.as_bytes());
    let strong = format!("ofx:{}:{}:{}", DEFAULT_CURRENCY_CODE, institution_account_id, fitid);
    let kind = match format {
        BankSourceFormat::Ofx => SourceKind::Ofx,
        BankSourceFormat::Qfx => SourceKind::Qfx,
        BankSourceFormat::Csv => unreachable!(),
    };
    let provenance = SourceProvenance::new(kind, Some(locator.to_string()), Some(strong), None)
        .map_err(|e| BankImportError::InvalidField(e.to_string()))?;
    BankLine::new(
        source_account_id, Some(institution_account_id.to_string()), format, file_sha, locator,
        posted_date, value_date, signed_amount_minor, description, name, reference, Some(fitid), raw_hash, provenance,
    )
}

fn find_blocks<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut blocks = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        let after_open = &rest[start + open.len()..];
        let Some(end) = after_open.find(&close) else { break };
        blocks.push(&after_open[..end]);
        rest = &after_open[end + close.len()..];
    }
    blocks
}
fn tag_value(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = text.find(&open)? + open.len();
    let rest = &text[start..];
    let close = format!("</{tag}>");
    let end = rest.find(&close).or_else(|| rest.find('<')).unwrap_or(rest.len());
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_csv_date(value: &str, format: CsvDateFormat) -> BankImportResult<Date> {
    let value = value.trim();
    let parts: Vec<&str> = match format { CsvDateFormat::IsoYmd => value.split('-').collect(), CsvDateFormat::DmySlash => value.split('/').collect() };
    if parts.len() != 3 {
        return Err(BankImportError::InvalidDate(format!("date '{value}' does not match the saved CSV profile")));
    }
    let (year, month, day) = match format {
        CsvDateFormat::IsoYmd => (parse_u16(parts[0], "year")?, parse_u8(parts[1], "month")?, parse_u8(parts[2], "day")?),
        CsvDateFormat::DmySlash => (parse_u16(parts[2], "year")?, parse_u8(parts[1], "month")?, parse_u8(parts[0], "day")?),
    };
    Date::new(year, month, day).map_err(|e| BankImportError::InvalidDate(e.to_string()))
}
fn parse_ofx_date(value: &str) -> BankImportResult<Date> {
    let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).take(8).collect();
    if digits.len() != 8 {
        return Err(BankImportError::InvalidDate(format!("OFX date '{value}' does not begin with YYYYMMDD")));
    }
    Date::new(parse_u16(&digits[0..4], "year")?, parse_u8(&digits[4..6], "month")?, parse_u8(&digits[6..8], "day")?)
        .map_err(|e| BankImportError::InvalidDate(e.to_string()))
}
fn parse_u16(value: &str, label: &str) -> BankImportResult<u16> {
    value.parse().map_err(|_| BankImportError::InvalidDate(format!("invalid {label} in bank date")))
}
fn parse_u8(value: &str, label: &str) -> BankImportResult<u8> {
    value.parse().map_err(|_| BankImportError::InvalidDate(format!("invalid {label} in bank date")))
}

fn parse_optional_positive_minor(value: &str) -> BankImportResult<Option<i64>> {
    let value = value.trim();
    if value.is_empty() { return Ok(None); }
    let amount = parse_signed_minor(value)?;
    if amount <= 0 {
        return Err(BankImportError::InvalidAmount("debit/credit columns must contain positive amounts only".into()));
    }
    Ok(Some(amount))
}
fn parse_signed_minor(value: &str) -> BankImportResult<i64> {
    let compact = value.trim().replace(',', "");
    if compact.is_empty() { return Err(BankImportError::InvalidAmount("amount is blank".into())); }
    let (negative, digits) = match compact.as_bytes()[0] {
        b'-' => (true, &compact[1..]), b'+' => (false, &compact[1..]), _ => (false, compact.as_str()),
    };
    if digits.is_empty() { return Err(BankImportError::InvalidAmount(format!("invalid amount '{value}'"))); }
    let mut parts = digits.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some() || whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return Err(BankImportError::InvalidAmount(format!("invalid decimal amount '{value}'")));
    }
    let fraction_minor = match fraction {
        None | Some("") => 0_i128,
        Some(f) if f.len() <= 2 && f.chars().all(|c| c.is_ascii_digit()) => {
            if f.len() == 1 { i128::from(f.as_bytes()[0] - b'0') * 10 }
            else { i128::from(f.parse::<u8>().map_err(|_| BankImportError::InvalidAmount(format!("invalid amount '{value}'")))?) }
        }
        _ => return Err(BankImportError::InvalidAmount(format!("amount '{value}' must resolve to whole pence"))),
    };
    let whole_minor = whole.parse::<i128>().map_err(|_| BankImportError::InvalidAmount(format!("amount '{value}' is too large")))?
        .checked_mul(100).ok_or_else(|| BankImportError::InvalidAmount("amount overflow".into()))?;
    let magnitude = whole_minor.checked_add(fraction_minor).ok_or_else(|| BankImportError::InvalidAmount("amount overflow".into()))?;
    if magnitude == 0 { return Err(BankImportError::InvalidAmount("bank line amount must be non-zero".into())); }
    let signed = if negative { -magnitude } else { magnitude };
    i64::try_from(signed).map_err(|_| BankImportError::InvalidAmount("amount exceeds i64 range".into()))
}

fn nonblank(value: String, label: &str) -> BankImportResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { return Err(BankImportError::InvalidField(format!("{label} must not be blank"))); }
    Ok(trimmed.to_string())
}
fn optional_nonblank(value: Option<String>, label: &str) -> BankImportResult<Option<String>> {
    value.map(|v| nonblank(v, label)).transpose()
}
fn blank_to_none(value: String) -> Option<String> {
    let trimmed = value.trim(); (!trimmed.is_empty()).then(|| trimmed.to_string())
}
fn require_sha256(value: &str, label: &str) -> BankImportResult<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(BankImportError::InvalidHash(format!("{label} must be 64 hexadecimal characters")));
    }
    Ok(())
}
fn normalize_match_text(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[must_use]
pub fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let mut h = [0x6a09e667_u32,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut data = input.to_vec(); data.push(0x80);
    while data.len() % 64 != 56 { data.push(0); }
    data.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in data.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (i, slot) in w.iter_mut().take(16).enumerate() {
            let j = i * 4; *slot = u32::from_be_bytes([chunk[j], chunk[j+1], chunk[j+2], chunk[j+3]]);
        }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
            let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let mut a=h[0]; let mut b=h[1]; let mut c=h[2]; let mut d=h[3];
        let mut e=h[4]; let mut f=h[5]; let mut g=h[6]; let mut hh=h[7];
        for i in 0..64 {
            let s1=e.rotate_right(6)^e.rotate_right(11)^e.rotate_right(25);
            let ch=(e&f)^((!e)&g);
            let t1=hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0=a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22);
            let maj=(a&b)^(a&c)^(b&c);
            let t2=s0.wrapping_add(maj);
            hh=g; g=f; f=e; e=d.wrapping_add(t1); d=c; c=b; b=a; a=t1.wrapping_add(t2);
        }
        h[0]=h[0].wrapping_add(a); h[1]=h[1].wrapping_add(b); h[2]=h[2].wrapping_add(c); h[3]=h[3].wrapping_add(d);
        h[4]=h[4].wrapping_add(e); h[5]=h[5].wrapping_add(f); h[6]=h[6].wrapping_add(g); h[7]=h[7].wrapping_add(hh);
    }
    let mut out = String::with_capacity(64);
    for value in h { write!(&mut out, "{value:08x}").expect("writing to String cannot fail"); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(value: &str) -> RecordId { RecordId::new(value).unwrap() }
    fn signed_profile() -> CsvMappingProfile {
        CsvMappingProfile::new(
            id("profile-signed"), "Bank CSV", ',', "Date", None, "Description",
            Some("Payee".into()), Some("Reference".into()), Some("Transaction ID".into()), Some("Currency".into()),
            CsvAmountMapping::Signed { amount_header: "Amount".into() }, CsvDateFormat::DmySlash,
        ).unwrap()
    }

    #[test] fn sha256_known_vectors() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
    #[test] fn csv_preview_handles_quotes_and_exact_pence() {
        let csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\r\n08/09/2026,\"Stationery, pens\",Shop,R1,T1,GBP,-12.34\r\n";
        let preview = preview_csv(csv, id("bank-main"), &signed_profile()).unwrap();
        assert!(preview.can_commit()); assert_eq!(preview.lines().len(), 1);
        let line = &preview.lines()[0];
        assert_eq!(line.signed_amount_minor(), -1234); assert_eq!(line.description(), "Stationery, pens");
        assert_eq!(line.strong_identity_key().as_deref(), Some("csv:GBP:bank-main:T1"));
        assert_eq!(line.provenance().kind(), SourceKind::Csv);
    }
    #[test] fn csv_saved_profile_supports_debit_credit_columns() {
        let profile = CsvMappingProfile::new(
            id("profile-dc"), "DC", ',', "Date", None, "Description", None, None, None, None,
            CsvAmountMapping::DebitCredit { debit_header:"Debit".into(), credit_header:"Credit".into() }, CsvDateFormat::IsoYmd,
        ).unwrap();
        let csv = "Date,Description,Debit,Credit\n2026-09-08,Rent,500.01,\n2026-09-09,Sale,,700.02\n";
        let preview = preview_csv(csv, id("bank"), &profile).unwrap();
        assert!(preview.can_commit()); assert_eq!(preview.lines()[0].signed_amount_minor(), -50001); assert_eq!(preview.lines()[1].signed_amount_minor(), 70002);
    }
    #[test] fn ambiguous_debit_credit_row_fails_closed() {
        let profile = CsvMappingProfile::new(
            id("profile-dc2"), "DC", ',', "Date", None, "Description", None, None, None, None,
            CsvAmountMapping::DebitCredit { debit_header:"Debit".into(), credit_header:"Credit".into() }, CsvDateFormat::IsoYmd,
        ).unwrap();
        let preview = preview_csv("Date,Description,Debit,Credit\n2026-09-08,Ambiguous,10.00,10.00\n", id("bank"), &profile).unwrap();
        assert!(!preview.can_commit()); assert_eq!(preview.lines().len(), 0); assert_eq!(preview.errors().len(), 1);
    }
    #[test] fn malformed_csv_quote_is_rejected() {
        let csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,\"bad,Shop,R1,T1,GBP,-12.34\n";
        assert!(preview_csv(csv, id("bank"), &signed_profile()).is_err());
    }
    #[test] fn unsupported_currency_is_not_silently_converted() {
        let csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Sale,Client,R1,T1,USD,12.34\n";
        let preview = preview_csv(csv, id("bank"), &signed_profile()).unwrap();
        assert!(!preview.can_commit()); assert!(preview.errors()[0].message().contains("GBP only"));
    }
    #[test] fn excessive_decimal_places_fail_closed() {
        let csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Sale,Client,R1,T1,GBP,12.345\n";
        assert!(!preview_csv(csv, id("bank"), &signed_profile()).unwrap().can_commit());
    }
    #[test] fn file_exact_and_heuristic_are_distinct_from_strong() {
        let csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Coffee,Cafe,R1,,GBP,-3.50\n";
        let a = preview_csv(csv, id("bank"), &signed_profile()).unwrap().lines()[0].clone(); let b = a.clone();
        assert_eq!(a.duplicate_certainty(&b).unwrap(), DuplicateCertainty::FileExact);
        assert!(DuplicateCertainty::FileExact.may_auto_suppress()); assert!(!DuplicateCertainty::Heuristic.may_auto_suppress());
    }
    #[test] fn strong_csv_identity_detects_duplicate() {
        let csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Sale,Client,R1,T1,GBP,10.00\n";
        let a = preview_csv(csv, id("bank"), &signed_profile()).unwrap().lines()[0].clone();
        assert_eq!(a.duplicate_certainty(&a).unwrap(), DuplicateCertainty::Strong);
    }
    #[test] fn strong_identity_conflict_is_an_error() {
        let a_csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Sale,Client,R1,T1,GBP,10.00\n";
        let b_csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Sale,Client,R1,T1,GBP,11.00\n";
        let a = preview_csv(a_csv, id("bank"), &signed_profile()).unwrap().lines()[0].clone();
        let b = preview_csv(b_csv, id("bank"), &signed_profile()).unwrap().lines()[0].clone();
        assert!(matches!(a.duplicate_certainty(&b), Err(BankImportError::ConflictingStrongIdentity(_))));
    }
    #[test] fn heuristic_similarity_never_authorises_auto_suppression() {
        let a_csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Coffee Shop,Cafe,R1,,GBP,-3.50\n";
        let b_csv = "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n08/09/2026,Coffee-Shop,Cafe,R2,,GBP,-3.50\n";
        let a = preview_csv(a_csv, id("bank"), &signed_profile()).unwrap().lines()[0].clone();
        let b = preview_csv(b_csv, id("bank"), &signed_profile()).unwrap().lines()[0].clone();
        let certainty = a.duplicate_certainty(&b).unwrap(); assert_eq!(certainty, DuplicateCertainty::Heuristic); assert!(!certainty.may_auto_suppress());
    }

    const OFX: &str = r#"<OFX><CURDEF>GBP<BANKACCTFROM><BANKID>123456<ACCTID>99887766<ACCTTYPE>CHECKING</BANKACCTFROM><BANKTRANLIST>
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260908120000[0:GMT]<TRNAMT>-47.23<FITID>FIT-001<NAME>OFFICE SHOP<MEMO>SUPPLIES<REFNUM>R-001</STMTTRN>
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260909120000[0:GMT]<TRNAMT>125.00<FITID>FIT-002<NAME>CLIENT PAYMENT</STMTTRN>
</BANKTRANLIST></OFX>"#;
    #[test] fn ofx_preview_creates_canonical_lines_and_fitid_identity() {
        let preview = preview_ofx_or_qfx(OFX, id("bank-main"), BankSourceFormat::Ofx).unwrap();
        assert!(preview.can_commit()); assert_eq!(preview.lines().len(), 2);
        let first = &preview.lines()[0]; assert_eq!(first.signed_amount_minor(), -4723); assert_eq!(first.posted_date(), Date::new(2026,9,8).unwrap());
        assert_eq!(first.institution_account_id(), Some("99887766")); assert_eq!(first.strong_identity_key().as_deref(), Some("ofx:GBP:99887766:FIT-001"));
        assert_eq!(first.provenance().kind(), SourceKind::Ofx);
    }
    #[test] fn qfx_uses_same_ofx_strong_identity_namespace() {
        let preview = preview_ofx_or_qfx(OFX, id("bank-main"), BankSourceFormat::Qfx).unwrap();
        assert!(preview.can_commit()); assert_eq!(preview.lines()[0].strong_identity_key().as_deref(), Some("ofx:GBP:99887766:FIT-001"));
        assert_eq!(preview.lines()[0].provenance().kind(), SourceKind::Qfx);
    }
    #[test] fn ofx_missing_fitid_fails_record_closed() {
        let bad = "<OFX><CURDEF>GBP<ACCTID>1<STMTTRN><DTPOSTED>20260908<TRNAMT>10.00</STMTTRN></OFX>";
        let preview = preview_ofx_or_qfx(bad, id("bank"), BankSourceFormat::Ofx).unwrap();
        assert!(!preview.can_commit()); assert_eq!(preview.errors().len(), 1);
    }
    #[test] fn ofx_non_gbp_is_rejected() {
        let bad = OFX.replace("<CURDEF>GBP", "<CURDEF>USD");
        assert!(matches!(preview_ofx_or_qfx(&bad, id("bank"), BankSourceFormat::Ofx), Err(BankImportError::UnsupportedCurrency(_))));
    }
    #[test] fn zero_amount_bank_line_is_rejected() {
        assert!(parse_signed_minor("0.00").is_err()); assert!(parse_signed_minor("-0.00").is_err());
    }
}
