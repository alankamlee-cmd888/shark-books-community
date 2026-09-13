//! SBC-7B1 Slice 3A typed owner bridge for user-controlled documents and factual OCR.
//!
//! Webview DTOs carry only opaque IDs and bounded semantic values. Native file
//! paths remain inside Rust, document bytes remain in a user-controlled root,
//! and OCR has no accounting, category, matching, reconciliation or tax authority.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use shark_books_core as core;
use shark_foundation::{
    Books, DocumentAttachmentPersistOutcome, DocumentAttachmentWrite, DocumentPersistOutcome,
    DocumentView, DocumentWrite, FoundationError, FoundationErrorCode,
};
use tauri_plugin_dialog::DialogExt;

use super::ocr_native::{self, NativeApprovedOcrDocument, NativeOcrRegistry, ShellOcrOutcome};
use super::{open_books_impl, OpenBooksRequest};

const OWNER_DOCUMENT_BRIDGE_VERSION: u32 = 1;
const MAX_DOCUMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_DOCUMENT_READ_BYTES: u64 = MAX_DOCUMENT_BYTES + 1;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerDocumentError {
    code: &'static str,
    message: String,
}

impl OwnerDocumentError {
    fn invalid(message: impl Into<String>) -> Self {
        Self { code: "invalidInput", message: message.into() }
    }

    fn foundation(error: FoundationError) -> Self {
        let code = match error.code {
            FoundationErrorCode::NotFound => "notFound",
            FoundationErrorCode::InvalidInput | FoundationErrorCode::Validation => "invalidInput",
            _ => "booksOperationFailed",
        };
        Self { code, message: error.to_string() }
    }

    fn storage_root_not_configured() -> Self {
        Self {
            code: "storageRootNotConfigured",
            message: "the selected user-controlled storage root is not configured on this device".to_string(),
        }
    }

    fn document(message: impl Into<String>) -> Self {
        Self { code: "documentOperationFailed", message: message.into() }
    }

    fn integrity(message: impl Into<String>) -> Self {
        Self { code: "documentIntegrityFailed", message: message.into() }
    }
}

type OwnerDocumentResult<T> = Result<T, OwnerDocumentError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnerBooksRef {
    file_name: String,
    books_id: String,
    actor: String,
}

impl OwnerBooksRef {
    fn open(&self) -> OwnerDocumentResult<Books> {
        open_books_impl(&OpenBooksRequest {
            file_name: self.file_name.clone(),
            books_id: self.books_id.clone(),
            actor: self.actor.clone(),
        })
        .map_err(OwnerDocumentError::foundation)
    }
}

#[derive(Default)]
pub(crate) struct NativeDocumentRootRegistry {
    roots: Mutex<HashMap<String, PathBuf>>,
}

impl NativeDocumentRootRegistry {
    /// Trusted native/settings layers may register a user-controlled root.
    /// This is deliberately not a Tauri command and never serializes the path.
    pub(crate) fn register_native_root(
        &self,
        root_id: impl Into<String>,
        path: PathBuf,
    ) -> OwnerDocumentResult<()> {
        let root_id = bounded_id(root_id.into(), "storage root id")?;
        if !path.is_dir() {
            return Err(OwnerDocumentError::document(
                "configured document storage root is not an existing directory",
            ));
        }
        let canonical = fs::canonicalize(path)
            .map_err(|error| OwnerDocumentError::document(format!("could not resolve document storage root: {error}")))?;
        self.roots
            .lock()
            .map_err(|_| OwnerDocumentError::document("document storage-root registry is unavailable"))?
            .insert(root_id, canonical);
        Ok(())
    }

    fn resolve(&self, root_id: &str) -> OwnerDocumentResult<PathBuf> {
        let root_id = bounded_id(root_id.to_string(), "storage root id")?;
        let root = self
            .roots
            .lock()
            .map_err(|_| OwnerDocumentError::document("document storage-root registry is unavailable"))?
            .get(&root_id)
            .cloned()
            .ok_or_else(OwnerDocumentError::storage_root_not_configured)?;
        if !root.is_dir() {
            return Err(OwnerDocumentError::storage_root_not_configured());
        }
        fs::canonicalize(root)
            .map_err(|_| OwnerDocumentError::storage_root_not_configured())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerDocumentSelectRequest {
    books: OwnerBooksRef,
    storage_root_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerDocumentVerifyRequest {
    books: OwnerBooksRef,
    document_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerDocumentAttachRequest {
    books: OwnerBooksRef,
    document_id: String,
    record_kind: OwnerDocumentRecordKind,
    record_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerOcrExtractRequest {
    books: OwnerBooksRef,
    request_id: String,
    document_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerDocumentRecordKind {
    MoneyIn,
    MoneyOut,
}

impl OwnerDocumentRecordKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MoneyIn => "moneyIn",
            Self::MoneyOut => "moneyOut",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerDocumentMetadata {
    bridge_version: u32,
    document_id: String,
    storage_root_id: String,
    relative_path: String,
    original_filename: String,
    media_type: Option<String>,
    sha256: String,
    byte_len: u64,
}

impl From<DocumentView> for OwnerDocumentMetadata {
    fn from(value: DocumentView) -> Self {
        Self {
            bridge_version: OWNER_DOCUMENT_BRIDGE_VERSION,
            document_id: value.document_id,
            storage_root_id: value.storage_root_id,
            relative_path: value.relative_path,
            original_filename: value.original_filename,
            media_type: value.media_type,
            sha256: value.sha256,
            byte_len: value.byte_len,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum OwnerDocumentSelectOutcome {
    Registered { document: OwnerDocumentMetadata },
    AlreadyRegistered { document: OwnerDocumentMetadata },
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerDocumentIntegrityStatus {
    Verified,
    SizeMismatch,
    HashMismatch,
    Missing,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerDocumentVerifyOutcome {
    bridge_version: u32,
    document_id: String,
    integrity: OwnerDocumentIntegrityStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerDocumentAttachOutcome {
    bridge_version: u32,
    document_id: String,
    record_kind: OwnerDocumentRecordKind,
    record_id: String,
    attached: bool,
    already_attached: bool,
    requires_further_automatic_action: bool,
}

fn bounded_id(value: String, label: &str) -> OwnerDocumentResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || trimmed.chars().any(|c| c.is_control() || c == '\0')
    {
        return Err(OwnerDocumentError::invalid(format!(
            "{label} must be 1-128 bounded characters"
        )));
    }
    Ok(trimmed.to_owned())
}

fn validate_filename(value: &str) -> OwnerDocumentResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 255
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_control)
    {
        return Err(OwnerDocumentError::document(
            "selected document filename is not a portable basename",
        ));
    }
    Ok(trimmed.to_owned())
}

fn validate_portable_relative_path(value: &str) -> OwnerDocumentResult<()> {
    if value.is_empty()
        || value.len() > 1024
        || value.starts_with('/')
        || value.starts_with('~')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
    {
        return Err(OwnerDocumentError::document(
            "persisted document path is not a portable relative path",
        ));
    }
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.trim() != segment {
            return Err(OwnerDocumentError::document(
                "persisted document path contains an unsafe segment",
            ));
        }
    }
    Ok(())
}

fn media_type_for_filename(filename: &str) -> Option<String> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    let media_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "webp" => "image/webp",
        "heic" | "heif" => "image/heic",
        _ => return None,
    };
    Some(media_type.to_string())
}

fn read_bounded_file(path: &Path) -> OwnerDocumentResult<Vec<u8>> {
    let file = File::open(path).map_err(|error| {
        OwnerDocumentError::document(format!("selected document could not be opened: {error}"))
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_READ_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| OwnerDocumentError::document(format!("selected document could not be read: {error}")))?;
    if bytes.is_empty() {
        return Err(OwnerDocumentError::document("selected document is empty"));
    }
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(OwnerDocumentError::document("selected document exceeds the 25 MiB limit"));
    }
    Ok(bytes)
}

fn ensure_contained_parent(root: &Path, parent: &Path) -> OwnerDocumentResult<PathBuf> {
    fs::create_dir_all(parent).map_err(|error| {
        OwnerDocumentError::document(format!("document destination directory could not be created: {error}"))
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        OwnerDocumentError::document(format!("document destination directory could not be resolved: {error}"))
    })?;
    if !canonical_parent.starts_with(root) {
        return Err(OwnerDocumentError::document(
            "document destination escapes the configured storage root",
        ));
    }
    Ok(canonical_parent)
}

fn verify_existing_destination(
    root: &Path,
    destination: &Path,
    expected_sha256: &str,
    expected_len: u64,
) -> OwnerDocumentResult<()> {
    let canonical = fs::canonicalize(destination).map_err(|error| {
        OwnerDocumentError::document(format!("existing document destination could not be resolved: {error}"))
    })?;
    if !canonical.starts_with(root) {
        return Err(OwnerDocumentError::document(
            "existing document destination escapes the configured storage root",
        ));
    }
    let bytes = read_bounded_file(&canonical)?;
    if bytes.len() as u64 != expected_len || core::bank_import::sha256_hex(&bytes) != expected_sha256 {
        return Err(OwnerDocumentError::integrity(
            "existing document destination conflicts with the selected document",
        ));
    }
    Ok(())
}

fn copy_selected_to_root(
    source: &Path,
    root_id: &str,
    root: &Path,
) -> OwnerDocumentResult<(DocumentWrite, PathBuf, bool)> {
    let bytes = read_bounded_file(source)?;
    let original_filename = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| OwnerDocumentError::document("selected document filename is not valid UTF-8"))?;
    let original_filename = validate_filename(original_filename)?;
    let sha256 = core::bank_import::sha256_hex(&bytes);
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| OwnerDocumentError::document("selected document length cannot be represented"))?;
    let document_id = format!("doc-{sha256}");
    let relative_path = format!("documents/{sha256}/{original_filename}");

    let parent = root.join("documents").join(&sha256);
    let parent = ensure_contained_parent(root, &parent)?;
    let destination = parent.join(&original_filename);

    if destination.exists() {
        verify_existing_destination(root, &destination, &sha256, byte_len)?;
        return Ok((DocumentWrite {
            document_id,
            storage_root_id: root_id.to_string(),
            relative_path,
            original_filename: original_filename.clone(),
            media_type: media_type_for_filename(&original_filename),
            sha256,
            byte_len,
        }, destination, false));
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| OwnerDocumentError::document("system clock is invalid"))?
        .as_nanos();
    let temp = parent.join(format!(".{original_filename}.sbc7b1-{}-{nonce}.tmp", std::process::id()));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| OwnerDocumentError::document(format!("temporary document copy could not be created: {error}")))?;
    if let Err(error) = output.write_all(&bytes).and_then(|_| output.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(OwnerDocumentError::document(format!("document copy could not be written: {error}")));
    }
    drop(output);
    if let Err(error) = verify_existing_destination(root, &temp, &sha256, byte_len) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    let created_destination = match fs::rename(&temp, &destination) {
        Ok(()) => true,
        Err(_) if destination.exists() => {
            let _ = fs::remove_file(&temp);
            verify_existing_destination(root, &destination, &sha256, byte_len)?;
            false
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            return Err(OwnerDocumentError::document(format!("document copy could not be finalized: {error}")));
        }
    };

    Ok((DocumentWrite {
        document_id,
        storage_root_id: root_id.to_string(),
        relative_path,
        original_filename: original_filename.clone(),
        media_type: media_type_for_filename(&original_filename),
        sha256,
        byte_len,
    }, destination, created_destination))
}

fn register_selected_path(
    books: &Books,
    roots: &NativeDocumentRootRegistry,
    storage_root_id: &str,
    source: &Path,
) -> OwnerDocumentResult<OwnerDocumentSelectOutcome> {
    let storage_root_id = bounded_id(storage_root_id.to_string(), "storage root id")?;
    let root = roots.resolve(&storage_root_id)?;
    let (write, destination, created_destination) =
        copy_selected_to_root(source, &storage_root_id, &root)?;
    match books.register_document(&write) {
        Ok(DocumentPersistOutcome::Registered(document)) => Ok(OwnerDocumentSelectOutcome::Registered {
            document: document.into(),
        }),
        Ok(DocumentPersistOutcome::AlreadyRegistered(document)) => {
            Ok(OwnerDocumentSelectOutcome::AlreadyRegistered { document: document.into() })
        }
        Err(error) => {
            if created_destination {
                let _ = fs::remove_file(destination);
            }
            Err(OwnerDocumentError::foundation(error))
        }
    }
}

fn trusted_document_path(
    roots: &NativeDocumentRootRegistry,
    document: &DocumentView,
) -> OwnerDocumentResult<Option<PathBuf>> {
    validate_portable_relative_path(&document.relative_path)?;
    let root = roots.resolve(&document.storage_root_id)?;
    let candidate = root.join(&document.relative_path);
    if !candidate.exists() {
        return Ok(None);
    }
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        OwnerDocumentError::document(format!("registered document path could not be resolved: {error}"))
    })?;
    if !canonical.starts_with(&root) {
        return Err(OwnerDocumentError::document(
            "registered document path escapes the configured storage root",
        ));
    }
    Ok(Some(canonical))
}

fn current_integrity(
    roots: &NativeDocumentRootRegistry,
    document: &DocumentView,
) -> OwnerDocumentResult<(OwnerDocumentIntegrityStatus, Option<PathBuf>)> {
    let Some(path) = trusted_document_path(roots, document)? else {
        return Ok((OwnerDocumentIntegrityStatus::Missing, None));
    };
    let metadata = fs::metadata(&path)
        .map_err(|error| OwnerDocumentError::document(format!("registered document metadata could not be read: {error}")))?;
    if !metadata.is_file() {
        return Ok((OwnerDocumentIntegrityStatus::Missing, None));
    }
    if metadata.len() != document.byte_len {
        return Ok((OwnerDocumentIntegrityStatus::SizeMismatch, Some(path)));
    }
    let bytes = read_bounded_file(&path)?;
    if core::bank_import::sha256_hex(&bytes) != document.sha256 {
        return Ok((OwnerDocumentIntegrityStatus::HashMismatch, Some(path)));
    }
    Ok((OwnerDocumentIntegrityStatus::Verified, Some(path)))
}

fn require_verified_document(
    books: &Books,
    roots: &NativeDocumentRootRegistry,
    document_id: &str,
) -> OwnerDocumentResult<(DocumentView, PathBuf)> {
    let document_id = bounded_id(document_id.to_string(), "document id")?;
    let document = books.document(&document_id).map_err(OwnerDocumentError::foundation)?;
    let (integrity, path) = current_integrity(roots, &document)?;
    if integrity != OwnerDocumentIntegrityStatus::Verified {
        return Err(OwnerDocumentError::integrity(format!(
            "registered document is not verified: {integrity:?}"
        )));
    }
    let path = path.ok_or_else(|| OwnerDocumentError::integrity("verified document has no trusted native path"))?;
    Ok((document, path))
}

#[tauri::command]
pub(crate) async fn owner_document_select_register(
    app: tauri::AppHandle,
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    request: OwnerDocumentSelectRequest,
) -> OwnerDocumentResult<OwnerDocumentSelectOutcome> {
    let root_id = bounded_id(request.storage_root_id.clone(), "storage root id")?;
    let _ = roots.resolve(&root_id)?;
    let books = request.books.open()?;
    let selected = app.dialog().file().blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(OwnerDocumentSelectOutcome::Cancelled);
    };
    let path = selected
        .into_path()
        .map_err(|_| OwnerDocumentError::document("selected document could not be resolved to a native file path"))?;
    register_selected_path(&books, &roots, &root_id, &path)
}

#[tauri::command]
pub(crate) fn owner_document_verify(
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    request: OwnerDocumentVerifyRequest,
) -> OwnerDocumentResult<OwnerDocumentVerifyOutcome> {
    let books = request.books.open()?;
    let document_id = bounded_id(request.document_id, "document id")?;
    let document = books.document(&document_id).map_err(OwnerDocumentError::foundation)?;
    let (integrity, _) = current_integrity(&roots, &document)?;
    Ok(OwnerDocumentVerifyOutcome {
        bridge_version: OWNER_DOCUMENT_BRIDGE_VERSION,
        document_id,
        integrity,
    })
}

#[tauri::command]
pub(crate) fn owner_document_attach(
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    request: OwnerDocumentAttachRequest,
) -> OwnerDocumentResult<OwnerDocumentAttachOutcome> {
    let books = request.books.open()?;
    let document_id = bounded_id(request.document_id, "document id")?;
    let record_id = bounded_id(request.record_id, "record id")?;
    let _ = require_verified_document(&books, &roots, &document_id)?;
    let reference = format!("sbc7b1:{}:{record_id}", request.record_kind.as_str());
    let transactions = books.find_by_reference(&reference).map_err(OwnerDocumentError::foundation)?;
    if transactions.len() != 1 {
        return Err(OwnerDocumentError::document(
            "document attachment requires exactly one authoritative owner transaction",
        ));
    }
    let write = DocumentAttachmentWrite {
        document_id: document_id.clone(),
        record_kind: request.record_kind.as_str().to_string(),
        record_id: record_id.clone(),
        transaction_id: transactions[0].id,
    };
    let outcome = books
        .attach_registered_document(&write)
        .map_err(OwnerDocumentError::foundation)?;
    let (attached, already_attached) = match outcome {
        DocumentAttachmentPersistOutcome::Attached(_) => (true, false),
        DocumentAttachmentPersistOutcome::AlreadyAttached(_) => (false, true),
    };
    Ok(OwnerDocumentAttachOutcome {
        bridge_version: OWNER_DOCUMENT_BRIDGE_VERSION,
        document_id,
        record_kind: request.record_kind,
        record_id,
        attached,
        already_attached,
        requires_further_automatic_action: false,
    })
}

#[tauri::command]
pub(crate) fn owner_ocr_extract_receipt(
    app: tauri::AppHandle,
    roots: tauri::State<'_, NativeDocumentRootRegistry>,
    ocr_registry: tauri::State<'_, NativeOcrRegistry>,
    request: OwnerOcrExtractRequest,
) -> OwnerDocumentResult<ShellOcrOutcome> {
    let books = request.books.open()?;
    let request_id = bounded_id(request.request_id, "OCR request id")?;
    let document_id = bounded_id(request.document_id, "document id")?;
    let (document, path) = require_verified_document(&books, &roots, &document_id)?;
    let approved = NativeApprovedOcrDocument::new(path, document.sha256, document.byte_len)
        .map_err(OwnerDocumentError::integrity)?;
    ocr_registry
        .approve(document_id.clone(), approved)
        .map_err(OwnerDocumentError::document)?;
    let ocr_request: ocr_native::OcrReceiptCommandRequest = serde_json::from_value(serde_json::json!({
        "requestId": request_id,
        "documentId": document_id
    }))
    .map_err(|_| OwnerDocumentError::invalid("OCR request identifiers are invalid"))?;
    ocr_native::ocr_extract_receipt(app, ocr_registry, ocr_request)
        .map_err(OwnerDocumentError::document)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "shark-sbc7b1-owner-doc-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn owner_requests_reject_path_hash_key_and_account_authority() {
        let base = serde_json::json!({
            "books": { "fileName": "safe.sqlite", "booksId": "safe-books", "actor": "owner" },
            "storageRootId": "root-1"
        });
        assert!(serde_json::from_value::<OwnerDocumentSelectRequest>(base.clone()).is_ok());
        for forbidden in [
            "sourcePath", "destinationPath", "fileUrl", "expectedSha256", "expectedByteLen",
            "databasePath", "passphrase", "token", "accountId", "category", "taxTreatment",
            "transactionId", "modelPath", "executablePath", "shellCommand", "url",
        ] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().insert(forbidden.to_string(), serde_json::Value::String("forbidden".into()));
            assert!(serde_json::from_value::<OwnerDocumentSelectRequest>(bad).is_err(), "forbidden field accepted: {forbidden}");
        }
    }

    #[test]
    fn missing_native_root_is_explicit() {
        let roots = NativeDocumentRootRegistry::default();
        let error = roots.resolve("root-1").unwrap_err();
        assert_eq!(error.code, "storageRootNotConfigured");
    }

    #[test]
    fn selected_bytes_drive_hash_length_and_user_root_copy() {
        let root = temp_dir("copy-root");
        let source_dir = temp_dir("copy-source");
        let source = source_dir.join("receipt.png");
        fs::write(&source, b"receipt-bytes-123").unwrap();
        let registry = NativeDocumentRootRegistry::default();
        registry.register_native_root("root-1", root.clone()).unwrap();
        let canonical_root = registry.resolve("root-1").unwrap();
        let (write, destination, created) = copy_selected_to_root(&source, "root-1", &canonical_root).unwrap();
        assert!(created);
        assert_eq!(write.sha256, core::bank_import::sha256_hex(b"receipt-bytes-123"));
        assert_eq!(write.byte_len, 17);
        assert_eq!(write.document_id, format!("doc-{}", write.sha256));
        assert!(destination.starts_with(canonical_root));
        assert_eq!(fs::read(destination).unwrap(), b"receipt-bytes-123");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(source_dir);
    }

    #[test]
    fn existing_conflicting_destination_fails_integrity_closed() {
        let root = temp_dir("conflict-root");
        let source_dir = temp_dir("conflict-source");
        let source = source_dir.join("receipt.png");
        fs::write(&source, b"receipt-original").unwrap();
        let registry = NativeDocumentRootRegistry::default();
        registry.register_native_root("root-1", root.clone()).unwrap();
        let canonical_root = registry.resolve("root-1").unwrap();
        let hash = core::bank_import::sha256_hex(b"receipt-original");
        let destination_dir = canonical_root.join("documents").join(&hash);
        fs::create_dir_all(&destination_dir).unwrap();
        fs::write(destination_dir.join("receipt.png"), b"tampered").unwrap();
        let error = copy_selected_to_root(&source, "root-1", &canonical_root).unwrap_err();
        assert_eq!(error.code, "documentIntegrityFailed");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(source_dir);
    }

    #[test]
    fn verification_detects_hash_tamper_size_tamper_and_missing_file() {
        let root = temp_dir("verify-root");
        let registry = NativeDocumentRootRegistry::default();
        registry.register_native_root("root-1", root.clone()).unwrap();
        let canonical_root = registry.resolve("root-1").unwrap();
        let bytes = b"receipt-body";
        let sha256 = core::bank_import::sha256_hex(bytes);
        let relative_path = format!("documents/{sha256}/receipt.png");
        let path = canonical_root.join(&relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        let document = DocumentView {
            document_id: format!("doc-{sha256}"),
            storage_root_id: "root-1".into(),
            relative_path,
            original_filename: "receipt.png".into(),
            media_type: Some("image/png".into()),
            sha256,
            byte_len: bytes.len() as u64,
            registered_by: "owner".into(),
            registered_at: "2026-09-12 00:00:00".into(),
        };
        assert_eq!(current_integrity(&registry, &document).unwrap().0, OwnerDocumentIntegrityStatus::Verified);
        fs::write(&path, b"Receipt-body").unwrap();
        assert_eq!(current_integrity(&registry, &document).unwrap().0, OwnerDocumentIntegrityStatus::HashMismatch);
        fs::write(&path, b"tampered-body").unwrap();
        assert_eq!(current_integrity(&registry, &document).unwrap().0, OwnerDocumentIntegrityStatus::SizeMismatch);
        fs::remove_file(&path).unwrap();
        assert_eq!(current_integrity(&registry, &document).unwrap().0, OwnerDocumentIntegrityStatus::Missing);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsafe_persisted_relative_path_is_rejected_before_join() {
        let root = temp_dir("unsafe-path-root");
        let registry = NativeDocumentRootRegistry::default();
        registry.register_native_root("root-1", root.clone()).unwrap();
        let document = DocumentView {
            document_id: format!("doc-{}", "11".repeat(32)),
            storage_root_id: "root-1".into(),
            relative_path: "../outside.png".into(),
            original_filename: "outside.png".into(),
            media_type: Some("image/png".into()),
            sha256: "11".repeat(32),
            byte_len: 1,
            registered_by: "owner".into(),
            registered_at: "2026-09-12 00:00:00".into(),
        };
        assert_eq!(trusted_document_path(&registry, &document).unwrap_err().code, "documentOperationFailed");
        let _ = fs::remove_dir_all(root);
    }
}
