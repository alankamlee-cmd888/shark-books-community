//! SBC-6B B3C bounded native/Tauri host for local receipt OCR.
//!
//! Security boundary: the webview supplies only bounded opaque identifiers.
//! Filesystem paths, hashes, executable/model paths and process arguments remain
//! native-owned. The adapter maps the B3B sidecar into the implementation-neutral
//! B2 factual OCR outcome shape and has no accounting, matching or tax authority.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const SIDECAR_SCHEMA: &str = "sbc6b.single_document.v1";
const B2_CONTRACT_VERSION: u32 = 1;
const MAX_DOCUMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_STDERR_BYTES: usize = 256 * 1024;
const MAX_RAW_TEXT_BYTES: usize = 256 * 1024;
const MAX_MODEL_IDS: usize = 8;
const MAX_WARNINGS: usize = 64;
const SIDECAR_TIMEOUT: Duration = Duration::from_secs(15);
const SIDECAR_RELATIVE_PATH: &str = "ocr/windows/shark-ocr-single/shark-ocr-single.exe";

#[derive(Debug, Clone)]
pub(crate) struct NativeApprovedOcrDocument {
    path: PathBuf,
    sha256: String,
    byte_len: u64,
}

impl NativeApprovedOcrDocument {
    pub(crate) fn new(
        path: PathBuf,
        sha256: impl Into<String>,
        byte_len: u64,
    ) -> Result<Self, &'static str> {
        let sha256 = canonical_sha256(&sha256.into()).ok_or("invalid document SHA-256")?;
        if byte_len == 0 || byte_len > MAX_DOCUMENT_BYTES {
            return Err("invalid document byte length");
        }
        Ok(Self {
            path,
            sha256,
            byte_len,
        })
    }
}

#[derive(Default)]
pub(crate) struct NativeOcrRegistry {
    documents: Mutex<HashMap<String, NativeApprovedOcrDocument>>,
}

impl NativeOcrRegistry {
    pub(crate) fn approve(
        &self,
        document_id: impl Into<String>,
        document: NativeApprovedOcrDocument,
    ) -> Result<(), &'static str> {
        let document_id = bounded_id(document_id.into()).ok_or("invalid document id")?;
        self.documents
            .lock()
            .map_err(|_| "OCR document registry unavailable")?
            .insert(document_id, document);
        Ok(())
    }

    fn resolve(&self, document_id: &str) -> Result<NativeApprovedOcrDocument, &'static str> {
        let document_id = bounded_id(document_id.to_owned()).ok_or("invalid document id")?;
        self.documents
            .lock()
            .map_err(|_| "OCR document registry unavailable")?
            .get(&document_id)
            .cloned()
            .ok_or("OCR document is not native-approved")
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OcrReceiptCommandRequest {
    request_id: String,
    document_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrDocumentIdentity {
    document_id: String,
    sha256: String,
    byte_len: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrProvenance {
    engine_id: String,
    engine_version: String,
    runtime_id: String,
    model_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrTextCandidate {
    value: String,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrDateCandidate {
    value: String,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrAmountCandidate {
    total_pence: i64,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrCurrencyCandidate {
    code: String,
    confidence_bps: Option<u16>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrReceiptCandidates {
    merchant_text: Option<ShellOcrTextCandidate>,
    document_date: Option<ShellOcrDateCandidate>,
    total: Option<ShellOcrAmountCandidate>,
    currency: Option<ShellOcrCurrencyCandidate>,
    reference: Option<ShellOcrTextCandidate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrWarning {
    code: String,
    detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellOcrExtraction {
    schema_version: u32,
    request_id: String,
    document: ShellOcrDocumentIdentity,
    provenance: ShellOcrProvenance,
    raw_text: String,
    regions: Vec<Value>,
    candidates: ShellOcrReceiptCandidates,
    warnings: Vec<ShellOcrWarning>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ShellOcrUnavailableReason {
    DisabledByUser,
    NotInstalled,
    UnsupportedPlatform,
    ModelsUnavailable,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ShellOcrFailureKind {
    IntegrityMismatch,
    Timeout,
    MalformedOutput,
    EngineFailure,
    ResourceLimit,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ShellOcrOutcome {
    Completed { extraction: ShellOcrExtraction },
    Unavailable { reason: ShellOcrUnavailableReason },
    Failed { kind: ShellOcrFailureKind },
}

#[derive(Debug)]
enum StreamEvent {
    Complete { stdout: bool, bytes: Vec<u8> },
    TooLarge { stdout: bool },
    Io { stdout: bool },
}

fn spawn_bounded_reader<R: Read + Send + 'static>(
    mut reader: R,
    stdout: bool,
    limit: usize,
    tx: mpsc::Sender<StreamEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut out = Vec::with_capacity(limit.min(64 * 1024));
        let mut chunk = [0_u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => {
                    let _ = tx.send(StreamEvent::Complete { stdout, bytes: out });
                    return;
                }
                Ok(count) => {
                    if out.len().saturating_add(count) > limit {
                        let _ = tx.send(StreamEvent::TooLarge { stdout });
                        return;
                    }
                    out.extend_from_slice(&chunk[..count]);
                }
                Err(_) => {
                    let _ = tx.send(StreamEvent::Io { stdout });
                    return;
                }
            }
        }
    })
}

fn canonical_sha256(value: &str) -> Option<String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

fn bounded_id(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || trimmed.chars().any(|c| c.is_control() || c == '\0')
    {
        return None;
    }
    Some(trimmed.to_owned())
}

fn bounded_text(value: &str, max: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max || trimmed.contains('\0') {
        return None;
    }
    Some(trimmed.to_owned())
}

fn valid_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<u32>().ok();
    let month = value[5..7].parse::<u32>().ok();
    let day = value[8..10].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return false;
    };
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    day <= max_day
}

fn parse_exact_json(bytes: &[u8]) -> Result<Value, ShellOcrFailureKind> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = Value::deserialize(&mut de).map_err(|_| ShellOcrFailureKind::MalformedOutput)?;
    de.end().map_err(|_| ShellOcrFailureKind::MalformedOutput)?;
    if !value.is_object() {
        return Err(ShellOcrFailureKind::MalformedOutput);
    }
    Ok(value)
}

fn required_str<'a>(object: &'a Value, key: &str) -> Result<&'a str, ShellOcrFailureKind> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(ShellOcrFailureKind::MalformedOutput)
}

fn optional_str(object: &Value, key: &str) -> Result<Option<String>, ShellOcrFailureKind> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(ShellOcrFailureKind::MalformedOutput),
    }
}

fn parse_warning(value: &Value) -> Result<ShellOcrWarning, ShellOcrFailureKind> {
    let object = value.as_object().ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let code = object
        .get("code")
        .and_then(Value::as_str)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let allowed = [
        "no_text_detected",
        "missing_merchant",
        "missing_date",
        "missing_total",
        "missing_currency",
        "missing_reference",
        "low_confidence",
        "unsupported_document",
        "truncated_output",
        "engine_notice",
    ];
    if !allowed.contains(&code) {
        return Err(ShellOcrFailureKind::MalformedOutput);
    }
    let detail = optional_str(value, "detail")?;
    if let Some(detail_value) = detail.as_deref() {
        if bounded_text(detail_value, 1024).is_none() {
            return Err(ShellOcrFailureKind::MalformedOutput);
        }
    }
    Ok(ShellOcrWarning {
        code: code.to_owned(),
        detail,
    })
}

fn parse_completed(
    value: &Value,
    request_id: &str,
    document_id: &str,
    document: &NativeApprovedOcrDocument,
) -> Result<ShellOcrOutcome, ShellOcrFailureKind> {
    let observed_hash = canonical_sha256(required_str(value, "observed_document_sha256")?)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let observed_len = value
        .get("observed_byte_len")
        .and_then(Value::as_u64)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    if observed_hash != document.sha256 || observed_len != document.byte_len {
        return Err(ShellOcrFailureKind::IntegrityMismatch);
    }

    let provenance_value = value
        .get("provenance")
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let engine_id = bounded_text(required_str(provenance_value, "engine_id")?, 128)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let engine_version = bounded_text(required_str(provenance_value, "engine_version")?, 128)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let runtime_id = bounded_text(required_str(provenance_value, "runtime_id")?, 128)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let models = provenance_value
        .get("model_ids")
        .and_then(Value::as_array)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    if models.is_empty() || models.len() > MAX_MODEL_IDS {
        return Err(ShellOcrFailureKind::MalformedOutput);
    }
    let mut model_ids = Vec::with_capacity(models.len());
    for model in models {
        let model = bounded_text(
            model.as_str().ok_or(ShellOcrFailureKind::MalformedOutput)?,
            256,
        )
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
        model_ids.push(model);
    }

    let raw_text = value
        .get("raw_text")
        .and_then(Value::as_str)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    if raw_text.len() > MAX_RAW_TEXT_BYTES || raw_text.contains('\0') {
        return Err(ShellOcrFailureKind::ResourceLimit);
    }

    let regions = value
        .get("regions")
        .and_then(Value::as_array)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    // B3B v1 deliberately emits no geometry. A later non-empty region protocol
    // requires its own schema/version review rather than silently inventing one.
    if !regions.is_empty() {
        return Err(ShellOcrFailureKind::MalformedOutput);
    }

    let candidate_value = value
        .get("candidates")
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let merchant = optional_str(candidate_value, "merchant_text")?
        .map(|text| {
            bounded_text(&text, 512)
                .map(|value| ShellOcrTextCandidate {
                    value,
                    confidence_bps: None,
                })
                .ok_or(ShellOcrFailureKind::MalformedOutput)
        })
        .transpose()?;
    let reference = optional_str(candidate_value, "reference")?
        .map(|text| {
            bounded_text(&text, 512)
                .map(|value| ShellOcrTextCandidate {
                    value,
                    confidence_bps: None,
                })
                .ok_or(ShellOcrFailureKind::MalformedOutput)
        })
        .transpose()?;
    let document_date = optional_str(candidate_value, "document_date")?
        .map(|date| {
            if !valid_iso_date(&date) {
                return Err(ShellOcrFailureKind::MalformedOutput);
            }
            Ok(ShellOcrDateCandidate {
                value: date,
                confidence_bps: None,
            })
        })
        .transpose()?;
    let total = match candidate_value.get("total_pence") {
        None | Some(Value::Null) => None,
        Some(number) => {
            let total_pence = number.as_i64().ok_or(ShellOcrFailureKind::MalformedOutput)?;
            if total_pence < 0 {
                return Err(ShellOcrFailureKind::MalformedOutput);
            }
            Some(ShellOcrAmountCandidate {
                total_pence,
                confidence_bps: None,
            })
        }
    };
    let currency = optional_str(candidate_value, "currency")?
        .map(|code| {
            if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err(ShellOcrFailureKind::MalformedOutput);
            }
            Ok(ShellOcrCurrencyCandidate {
                code,
                confidence_bps: None,
            })
        })
        .transpose()?;

    let warnings_value = value
        .get("warnings")
        .and_then(Value::as_array)
        .ok_or(ShellOcrFailureKind::MalformedOutput)?;
    if warnings_value.len() > MAX_WARNINGS {
        return Err(ShellOcrFailureKind::ResourceLimit);
    }
    let warnings = warnings_value
        .iter()
        .map(parse_warning)
        .collect::<Result<Vec<_>, _>>()?;

    let privacy = value.get("privacy").ok_or(ShellOcrFailureKind::MalformedOutput)?;
    let privacy_ok = privacy
        .get("ort_disable_telemetry_env")
        .and_then(Value::as_bool)
        == Some(true)
        && privacy
            .get("ort_telemetry_disable_api_called")
            .and_then(Value::as_bool)
            == Some(true)
        && privacy
            .get("python_tcp_connections_denied_during_runtime")
            .and_then(Value::as_bool)
            == Some(true)
        && privacy.get("paddleocr_imported").and_then(Value::as_bool) == Some(false)
        && privacy.get("paddlex_imported").and_then(Value::as_bool) == Some(false);
    if !privacy_ok {
        return Err(ShellOcrFailureKind::EngineFailure);
    }

    Ok(ShellOcrOutcome::Completed {
        extraction: ShellOcrExtraction {
            schema_version: B2_CONTRACT_VERSION,
            request_id: request_id.to_owned(),
            document: ShellOcrDocumentIdentity {
                document_id: document_id.to_owned(),
                sha256: document.sha256.clone(),
                byte_len: document.byte_len,
            },
            provenance: ShellOcrProvenance {
                engine_id,
                engine_version,
                runtime_id,
                model_ids,
            },
            raw_text: raw_text.to_owned(),
            regions: Vec::new(),
            candidates: ShellOcrReceiptCandidates {
                merchant_text: merchant,
                document_date,
                total,
                currency,
                reference,
            },
            warnings,
        },
    })
}

fn map_sidecar_failure(value: &Value) -> ShellOcrFailureKind {
    match value.get("failure_kind").and_then(Value::as_str) {
        Some("integrity_mismatch") => ShellOcrFailureKind::IntegrityMismatch,
        Some("engine_failure") | Some("unsupported_document") => ShellOcrFailureKind::EngineFailure,
        _ => ShellOcrFailureKind::MalformedOutput,
    }
}

fn interpret_sidecar(
    exit_success: bool,
    stdout: &[u8],
    request_id: &str,
    document_id: &str,
    document: &NativeApprovedOcrDocument,
) -> ShellOcrOutcome {
    let value = match parse_exact_json(stdout) {
        Ok(value) => value,
        Err(kind) => return ShellOcrOutcome::Failed { kind },
    };
    if value.get("schema").and_then(Value::as_str) != Some(SIDECAR_SCHEMA) {
        return ShellOcrOutcome::Failed {
            kind: ShellOcrFailureKind::MalformedOutput,
        };
    }
    match value.get("status").and_then(Value::as_str) {
        Some("completed") if exit_success => {
            match parse_completed(&value, request_id, document_id, document) {
                Ok(outcome) => outcome,
                Err(kind) => ShellOcrOutcome::Failed { kind },
            }
        }
        Some("failed") if !exit_success => ShellOcrOutcome::Failed {
            kind: map_sidecar_failure(&value),
        },
        _ => ShellOcrOutcome::Failed {
            kind: ShellOcrFailureKind::MalformedOutput,
        },
    }
}

fn validate_package(sidecar: &Path) -> Result<(), ShellOcrUnavailableReason> {
    if !sidecar.is_file() {
        return Err(ShellOcrUnavailableReason::NotInstalled);
    }
    let root = sidecar.parent().ok_or(ShellOcrUnavailableReason::NotInstalled)?;
    for relative in [
        "models/det/inference.onnx",
        "models/det/inference.yml",
        "models/rec/inference.onnx",
        "models/rec/inference.yml",
    ] {
        if !root.join(relative).is_file() {
            return Err(ShellOcrUnavailableReason::ModelsUnavailable);
        }
    }
    Ok(())
}

fn preflight_document(document: &NativeApprovedOcrDocument) -> Result<(), ShellOcrFailureKind> {
    let metadata = fs::metadata(&document.path).map_err(|_| ShellOcrFailureKind::EngineFailure)?;
    if !metadata.is_file() {
        return Err(ShellOcrFailureKind::EngineFailure);
    }
    if metadata.len() != document.byte_len {
        return Err(ShellOcrFailureKind::IntegrityMismatch);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn restricted_runtime_path() -> Option<std::ffi::OsString> {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut path = PathBuf::from(root);
    path.push("System32");
    Some(path.into_os_string())
}

fn run_sidecar_for_document(
    sidecar: &Path,
    document_id: &str,
    request_id: &str,
    document: &NativeApprovedOcrDocument,
    timeout: Duration,
) -> ShellOcrOutcome {
    if let Err(reason) = validate_package(sidecar) {
        return ShellOcrOutcome::Unavailable { reason };
    }
    if let Err(kind) = preflight_document(document) {
        return ShellOcrOutcome::Failed { kind };
    }

    let mut command = Command::new(sidecar);
    command
        .arg("--input")
        .arg(&document.path)
        .arg("--expected-sha256")
        .arg(&document.sha256)
        .arg("--expected-byte-len")
        .arg(document.byte_len.to_string())
        .current_dir(sidecar.parent().unwrap_or_else(|| Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = restricted_runtime_path() {
            command.env("PATH", path);
        }
        command.env_remove("PYTHONHOME");
        command.env_remove("PYTHONPATH");
        command.env_remove("SHARK_OCR_MODEL_ROOT");
        command.env("ORT_DISABLE_TELEMETRY", "1");
        command.env("PYTHONDONTWRITEBYTECODE", "1");
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            return ShellOcrOutcome::Unavailable {
                reason: ShellOcrUnavailableReason::NotInstalled,
            }
        }
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return ShellOcrOutcome::Failed {
            kind: ShellOcrFailureKind::EngineFailure,
        };
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return ShellOcrOutcome::Failed {
            kind: ShellOcrFailureKind::EngineFailure,
        };
    };

    let (tx, rx) = mpsc::channel();
    let stdout_handle = spawn_bounded_reader(stdout, true, MAX_STDOUT_BYTES, tx.clone());
    let stderr_handle = spawn_bounded_reader(stderr, false, MAX_STDERR_BYTES, tx);
    let started = Instant::now();
    let mut stdout_bytes: Option<Vec<u8>> = None;
    let mut stderr_complete = false;

    let outcome = loop {
        while let Ok(event) = rx.try_recv() {
            match event {
                StreamEvent::Complete { stdout: true, bytes } => stdout_bytes = Some(bytes),
                StreamEvent::Complete { stdout: false, .. } => stderr_complete = true,
                StreamEvent::TooLarge { .. } => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break ShellOcrOutcome::Failed {
                        kind: ShellOcrFailureKind::ResourceLimit,
                    };
                }
                StreamEvent::Io { .. } => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break ShellOcrOutcome::Failed {
                        kind: ShellOcrFailureKind::EngineFailure,
                    };
                }
            }
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break ShellOcrOutcome::Failed {
                kind: ShellOcrFailureKind::Timeout,
            };
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                let mut terminal_kind: Option<ShellOcrFailureKind> = None;
                while stdout_bytes.is_none() || !stderr_complete {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(StreamEvent::Complete { stdout: true, bytes }) => stdout_bytes = Some(bytes),
                        Ok(StreamEvent::Complete { stdout: false, .. }) => stderr_complete = true,
                        Ok(StreamEvent::TooLarge { .. }) => {
                            terminal_kind = Some(ShellOcrFailureKind::ResourceLimit);
                            break;
                        }
                        Ok(StreamEvent::Io { .. }) => {
                            terminal_kind = Some(ShellOcrFailureKind::EngineFailure);
                            break;
                        }
                        Err(_) => {
                            terminal_kind = Some(ShellOcrFailureKind::EngineFailure);
                            break;
                        }
                    }
                    if started.elapsed() >= timeout {
                        terminal_kind = Some(ShellOcrFailureKind::Timeout);
                        break;
                    }
                }
                if let Some(kind) = terminal_kind {
                    break ShellOcrOutcome::Failed { kind };
                }
                let Some(bytes) = stdout_bytes.as_deref() else {
                    break ShellOcrOutcome::Failed {
                        kind: ShellOcrFailureKind::EngineFailure,
                    };
                };
                break interpret_sidecar(status.success(), bytes, request_id, document_id, document);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break ShellOcrOutcome::Failed {
                    kind: ShellOcrFailureKind::EngineFailure,
                };
            }
        }
    };

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();
    outcome
}

#[cfg(target_os = "windows")]
fn packaged_sidecar_path(app: &tauri::AppHandle) -> Result<PathBuf, ShellOcrUnavailableReason> {
    use tauri::Manager;
    let root = app
        .path()
        .resource_dir()
        .map_err(|_| ShellOcrUnavailableReason::NotInstalled)?;
    Ok(root.join(SIDECAR_RELATIVE_PATH))
}

#[tauri::command]
pub(crate) fn ocr_extract_receipt(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeOcrRegistry>,
    request: OcrReceiptCommandRequest,
) -> Result<ShellOcrOutcome, String> {
    let request_id = bounded_id(request.request_id).ok_or_else(|| "invalid OCR request id".to_string())?;
    let document_id = bounded_id(request.document_id).ok_or_else(|| "invalid OCR document id".to_string())?;
    let document = state.resolve(&document_id).map_err(str::to_string)?;

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        let _ = document;
        return Ok(ShellOcrOutcome::Unavailable {
            reason: ShellOcrUnavailableReason::UnsupportedPlatform,
        });
    }

    #[cfg(target_os = "windows")]
    {
        let sidecar = match packaged_sidecar_path(&app) {
            Ok(path) => path,
            Err(reason) => return Ok(ShellOcrOutcome::Unavailable { reason }),
        };
        Ok(run_sidecar_for_document(
            &sidecar,
            &document_id,
            &request_id,
            &document,
            SIDECAR_TIMEOUT,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_document(path: PathBuf) -> NativeApprovedOcrDocument {
        NativeApprovedOcrDocument::new(path, "11".repeat(32), 10).unwrap()
    }

    fn completed_json(document: &NativeApprovedOcrDocument) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema": SIDECAR_SCHEMA,
            "status": "completed",
            "observed_document_sha256": document.sha256,
            "observed_byte_len": document.byte_len,
            "provenance": {
                "engine_id": "shark-direct-onnx",
                "engine_version": "sbc6b-b1-v1",
                "runtime_id": "onnxruntime-1.23.2-cpu",
                "model_ids": ["PP-OCRv6_tiny_det", "PP-OCRv6_tiny_rec"]
            },
            "raw_text": "NORTH PIER\nDATE 04/04/2026\nTOTAL GBP 28.49",
            "regions": [],
            "candidates": {
                "merchant_text": "NORTH PIER",
                "document_date": "2026-04-04",
                "total_pence": 2849,
                "currency": "GBP",
                "reference": "NP-260404-1842"
            },
            "warnings": [],
            "privacy": {
                "ort_disable_telemetry_env": true,
                "ort_telemetry_disable_api_called": true,
                "python_tcp_connections_denied_during_runtime": true,
                "paddleocr_imported": false,
                "paddlex_imported": false
            }
        }))
        .unwrap()
    }

    #[test]
    fn webview_request_accepts_only_opaque_ids() {
        let good = serde_json::json!({
            "requestId": "request-1",
            "documentId": "document-1"
        });
        assert!(serde_json::from_value::<OcrReceiptCommandRequest>(good).is_ok());

        for forbidden in [
            "inputPath", "absolutePath", "expectedSha256", "expectedByteLen",
            "modelPath", "executablePath", "url", "databasePath", "outputPath",
            "arguments", "shellCommand",
        ] {
            let mut bad = serde_json::json!({
                "requestId": "request-1",
                "documentId": "document-1"
            });
            bad.as_object_mut()
                .unwrap()
                .insert(forbidden.to_string(), Value::String("forbidden".into()));
            assert!(
                serde_json::from_value::<OcrReceiptCommandRequest>(bad).is_err(),
                "forbidden webview field accepted: {forbidden}"
            );
        }
    }

    #[test]
    fn registry_is_native_only_and_resolves_by_opaque_document_id() {
        let registry = NativeOcrRegistry::default();
        registry
            .approve("doc-1", sample_document(PathBuf::from("native-only.png")))
            .unwrap();
        let resolved = registry.resolve("doc-1").unwrap();
        assert_eq!(resolved.sha256, "11".repeat(32));
        assert!(registry.resolve("../native-only.png").is_err());
    }

    #[test]
    fn completed_sidecar_maps_to_b2_factual_shape_without_inferred_confidence() {
        let document = sample_document(PathBuf::from("receipt.png"));
        let outcome = interpret_sidecar(
            true,
            &completed_json(&document),
            "request-1",
            "document-1",
            &document,
        );
        let ShellOcrOutcome::Completed { extraction } = outcome else {
            panic!("expected completed OCR outcome");
        };
        assert_eq!(extraction.schema_version, 1);
        assert_eq!(extraction.document.sha256, document.sha256);
        assert_eq!(extraction.candidates.total.unwrap().total_pence, 2849);
        assert_eq!(
            extraction.candidates.document_date.unwrap().value,
            "2026-04-04"
        );
        assert!(
            extraction
                .candidates
                .merchant_text
                .unwrap()
                .confidence_bps
                .is_none()
        );
    }

    #[test]
    fn exact_one_json_object_and_schema_are_enforced() {
        let document = sample_document(PathBuf::from("receipt.png"));
        let mut multiple = completed_json(&document);
        multiple.extend_from_slice(b"\n{}");
        assert!(matches!(
            interpret_sidecar(true, &multiple, "req", "doc", &document),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::MalformedOutput }
        ));

        let wrong = br#"{"schema":"wrong","status":"completed"}"#;
        assert!(matches!(
            interpret_sidecar(true, wrong, "req", "doc", &document),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::MalformedOutput }
        ));
    }

    #[test]
    fn completed_hash_or_length_drift_fails_integrity_closed() {
        let document = sample_document(PathBuf::from("receipt.png"));
        let mut value: Value = serde_json::from_slice(&completed_json(&document)).unwrap();
        value["observed_document_sha256"] = Value::String("22".repeat(32));
        let payload = serde_json::to_vec(&value).unwrap();
        assert!(matches!(
            interpret_sidecar(true, &payload, "req", "doc", &document),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::IntegrityMismatch }
        ));
    }

    #[test]
    fn sidecar_failure_is_typed_and_nonzero_is_required() {
        let document = sample_document(PathBuf::from("receipt.png"));
        let failed = serde_json::to_vec(&serde_json::json!({
            "schema": SIDECAR_SCHEMA,
            "status": "failed",
            "failure_kind": "engine_failure",
            "detail": "not exposed",
            "runtime_loaded": false
        }))
        .unwrap();
        assert!(matches!(
            interpret_sidecar(false, &failed, "req", "doc", &document),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::EngineFailure }
        ));
        assert!(matches!(
            interpret_sidecar(true, &failed, "req", "doc", &document),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::MalformedOutput }
        ));
    }

    #[test]
    fn invalid_factual_values_fail_closed() {
        let document = sample_document(PathBuf::from("receipt.png"));
        let mut value: Value = serde_json::from_slice(&completed_json(&document)).unwrap();
        value["candidates"]["document_date"] = Value::String("2026-02-31".into());
        value["candidates"]["currency"] = Value::String("gbp".into());
        let payload = serde_json::to_vec(&value).unwrap();
        assert!(matches!(
            interpret_sidecar(true, &payload, "req", "doc", &document),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::MalformedOutput }
        ));
    }

    #[cfg(target_os = "windows")]
    fn env_path(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).expect("B3C Windows harness supplies path"))
    }

    #[cfg(target_os = "windows")]
    fn real_document() -> NativeApprovedOcrDocument {
        let path = env_path("SHARK_SBC6B_B3C_REAL_RECEIPT");
        let sha = std::env::var("SHARK_SBC6B_B3C_REAL_SHA256").unwrap();
        let len = std::env::var("SHARK_SBC6B_B3C_REAL_LEN")
            .unwrap()
            .parse::<u64>()
            .unwrap();
        NativeApprovedOcrDocument::new(path, sha, len).unwrap()
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_real_packaged_sidecar_returns_b2_compatible_facts() {
        let sidecar = env_path("SHARK_SBC6B_B3C_REAL_SIDECAR");
        let outcome = run_sidecar_for_document(
            &sidecar,
            "real-document",
            "real-request",
            &real_document(),
            SIDECAR_TIMEOUT,
        );
        let ShellOcrOutcome::Completed { extraction } = outcome else {
            panic!("real packaged sidecar did not complete: {outcome:?}");
        };
        assert_eq!(extraction.schema_version, 1);
        assert_eq!(extraction.request_id, "real-request");
        assert!(!extraction.raw_text.is_empty());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_wrong_length_fails_before_sidecar_execution() {
        let sidecar = env_path("SHARK_SBC6B_B3C_REAL_SIDECAR");
        let mut document = real_document();
        document.byte_len += 1;
        assert!(matches!(
            run_sidecar_for_document(
                &sidecar,
                "real-document",
                "real-request",
                &document,
                SIDECAR_TIMEOUT,
            ),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::IntegrityMismatch }
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_wrong_hash_is_rejected_by_verified_sidecar_before_ocr() {
        let sidecar = env_path("SHARK_SBC6B_B3C_REAL_SIDECAR");
        let mut document = real_document();
        document.sha256 = "00".repeat(32);
        assert!(matches!(
            run_sidecar_for_document(
                &sidecar,
                "real-document",
                "real-request",
                &document,
                SIDECAR_TIMEOUT,
            ),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::IntegrityMismatch }
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_missing_sidecar_is_unavailable() {
        let missing = env_path("SHARK_SBC6B_B3C_REAL_SIDECAR").with_file_name("missing.exe");
        assert!(matches!(
            run_sidecar_for_document(
                &missing,
                "real-document",
                "real-request",
                &real_document(),
                Duration::from_secs(1),
            ),
            ShellOcrOutcome::Unavailable { reason: ShellOcrUnavailableReason::NotInstalled }
        ));
    }

    #[cfg(target_os = "windows")]
    fn fake_case(name: &str, timeout: Duration) -> ShellOcrOutcome {
        let sidecar = env_path("SHARK_SBC6B_B3C_FAKE_SIDECAR");
        let root = env_path("SHARK_SBC6B_B3C_FAKE_ROOT");
        let path = root.join(name);
        fs::write(&path, b"0123456789").unwrap();
        let document = NativeApprovedOcrDocument::new(path, "11".repeat(32), 10).unwrap();
        run_sidecar_for_document(&sidecar, "fake-doc", "fake-req", &document, timeout)
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_nonzero_sidecar_maps_to_typed_engine_failure() {
        assert!(matches!(
            fake_case("engine-failure.png", Duration::from_secs(3)),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::EngineFailure }
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_malformed_and_wrong_schema_fail_closed() {
        for name in ["malformed.png", "wrong-schema.png"] {
            assert!(matches!(
                fake_case(name, Duration::from_secs(3)),
                ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::MalformedOutput }
            ));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_stdout_and_stderr_limits_fail_closed() {
        for name in ["stdout-overflow.png", "stderr-overflow.png"] {
            assert!(matches!(
                fake_case(name, Duration::from_secs(3)),
                ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::ResourceLimit }
            ));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_timeout_is_owned_by_rust_and_child_is_terminated() {
        let started = Instant::now();
        assert!(matches!(
            fake_case("timeout.png", Duration::from_millis(250)),
            ShellOcrOutcome::Failed { kind: ShellOcrFailureKind::Timeout }
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
