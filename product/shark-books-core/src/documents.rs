//! SBC-5 document/reference and user-controlled storage contracts.
//!
//! This module is deliberately platform-neutral and persistence-free. It stores
//! document integrity metadata and relative references to a user-controlled
//! storage root. Native adapters may implement only the bounded request shapes
//! below; this core exposes no arbitrary filesystem, network, credential, or
//! Shark-hosted document-custody surface.

use crate::bank_import::sha256_hex;
use crate::RecordId;
use std::fmt;

pub type DocumentResult<T> = Result<T, DocumentError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    InvalidStorageRoot(String),
    InvalidPath(String),
    InvalidHash(String),
    InvalidDocument(String),
    InvalidSelection(String),
}
impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStorageRoot(s)
            | Self::InvalidPath(s)
            | Self::InvalidHash(s)
            | Self::InvalidDocument(s)
            | Self::InvalidSelection(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for DocumentError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageProvider {
    LocalFilesystem,
    OneDriveFolder,
    DropboxFolder,
    ICloudDriveFolder,
    GoogleDriveFolder,
    OtherUserSelectedSyncedFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageCustody {
    UserControlled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRootDescriptor {
    id: RecordId,
    provider: StorageProvider,
    label: String,
}
impl StorageRootDescriptor {
    pub fn new(id: RecordId, provider: StorageProvider, label: impl Into<String>) -> DocumentResult<Self> {
        let label = nonblank(label.into(), "storage root label", 256)?;
        Ok(Self { id, provider, label })
    }

    #[must_use] pub fn id(&self) -> &RecordId { &self.id }
    #[must_use] pub const fn provider(&self) -> StorageProvider { self.provider }
    #[must_use] pub fn label(&self) -> &str { &self.label }
    #[must_use] pub const fn custody(&self) -> StorageCustody { StorageCustody::UserControlled }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentReference {
    id: RecordId,
    attached_record_id: RecordId,
    storage_root_id: RecordId,
    relative_path: String,
    original_filename: String,
    media_type: Option<String>,
    sha256: String,
    byte_len: u64,
}
impl DocumentReference {
    #[allow(clippy::too_many_arguments)]
    pub fn from_bytes(
        id: RecordId,
        attached_record_id: RecordId,
        storage_root: &StorageRootDescriptor,
        relative_path: impl Into<String>,
        original_filename: impl Into<String>,
        media_type: Option<String>,
        bytes: &[u8],
    ) -> DocumentResult<Self> {
        if bytes.is_empty() {
            return Err(DocumentError::InvalidDocument("document must not be empty".into()));
        }
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| DocumentError::InvalidDocument("document length cannot be represented".into()))?;
        Self::from_persisted(
            id,
            attached_record_id,
            storage_root.id.clone(),
            relative_path,
            original_filename,
            media_type,
            sha256_hex(bytes),
            byte_len,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_persisted(
        id: RecordId,
        attached_record_id: RecordId,
        storage_root_id: RecordId,
        relative_path: impl Into<String>,
        original_filename: impl Into<String>,
        media_type: Option<String>,
        sha256: impl Into<String>,
        byte_len: u64,
    ) -> DocumentResult<Self> {
        if byte_len == 0 {
            return Err(DocumentError::InvalidDocument("document byte length must be positive".into()));
        }
        let relative_path = validate_relative_path(relative_path.into())?;
        let original_filename = validate_filename(original_filename.into())?;
        let media_type = optional_nonblank(media_type, "media type", 255)?;
        let sha256 = canonical_sha256(sha256.into())?;
        Ok(Self {
            id,
            attached_record_id,
            storage_root_id,
            relative_path,
            original_filename,
            media_type,
            sha256,
            byte_len,
        })
    }

    #[must_use] pub fn id(&self) -> &RecordId { &self.id }
    #[must_use] pub fn attached_record_id(&self) -> &RecordId { &self.attached_record_id }
    #[must_use] pub fn storage_root_id(&self) -> &RecordId { &self.storage_root_id }
    #[must_use] pub fn relative_path(&self) -> &str { &self.relative_path }
    #[must_use] pub fn original_filename(&self) -> &str { &self.original_filename }
    #[must_use] pub fn media_type(&self) -> Option<&str> { self.media_type.as_deref() }
    #[must_use] pub fn sha256(&self) -> &str { &self.sha256 }
    #[must_use] pub const fn byte_len(&self) -> u64 { self.byte_len }

    #[must_use]
    pub fn verify_bytes(&self, bytes: &[u8]) -> IntegrityStatus {
        let observed_len = match u64::try_from(bytes.len()) {
            Ok(value) => value,
            Err(_) => return IntegrityStatus::SizeMismatch,
        };
        if observed_len != self.byte_len {
            return IntegrityStatus::SizeMismatch;
        }
        if sha256_hex(bytes) != self.sha256 {
            return IntegrityStatus::HashMismatch;
        }
        IntegrityStatus::Verified
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityStatus {
    Verified,
    SizeMismatch,
    HashMismatch,
}

/// Opaque identifier produced only by a native file-picker adapter. It is not a
/// filesystem path and therefore cannot be used by the webview to name an
/// arbitrary source file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NativeSelectionId(RecordId);
impl NativeSelectionId {
    #[must_use] pub const fn new(id: RecordId) -> Self { Self(id) }
    #[must_use] pub fn as_record_id(&self) -> &RecordId { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopySelectedDocumentRequest {
    selection_id: NativeSelectionId,
    storage_root_id: RecordId,
    relative_path: String,
    expected_sha256: String,
    expected_byte_len: u64,
}
impl CopySelectedDocumentRequest {
    pub fn new(
        selection_id: NativeSelectionId,
        storage_root_id: RecordId,
        relative_path: impl Into<String>,
        expected_sha256: impl Into<String>,
        expected_byte_len: u64,
    ) -> DocumentResult<Self> {
        if expected_byte_len == 0 {
            return Err(DocumentError::InvalidSelection("selected document length must be positive".into()));
        }
        Ok(Self {
            selection_id,
            storage_root_id,
            relative_path: validate_relative_path(relative_path.into())?,
            expected_sha256: canonical_sha256(expected_sha256.into())?,
            expected_byte_len,
        })
    }

    #[must_use] pub fn selection_id(&self) -> &NativeSelectionId { &self.selection_id }
    #[must_use] pub fn storage_root_id(&self) -> &RecordId { &self.storage_root_id }
    #[must_use] pub fn relative_path(&self) -> &str { &self.relative_path }
    #[must_use] pub fn expected_sha256(&self) -> &str { &self.expected_sha256 }
    #[must_use] pub const fn expected_byte_len(&self) -> u64 { self.expected_byte_len }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadVerifiedDocumentRequest {
    storage_root_id: RecordId,
    relative_path: String,
    expected_sha256: String,
    expected_byte_len: u64,
}
impl ReadVerifiedDocumentRequest {
    pub fn for_reference(reference: &DocumentReference) -> Self {
        Self {
            storage_root_id: reference.storage_root_id.clone(),
            relative_path: reference.relative_path.clone(),
            expected_sha256: reference.sha256.clone(),
            expected_byte_len: reference.byte_len,
        }
    }

    #[must_use] pub fn storage_root_id(&self) -> &RecordId { &self.storage_root_id }
    #[must_use] pub fn relative_path(&self) -> &str { &self.relative_path }
    #[must_use] pub fn expected_sha256(&self) -> &str { &self.expected_sha256 }
    #[must_use] pub const fn expected_byte_len(&self) -> u64 { self.expected_byte_len }
}

fn validate_relative_path(value: String) -> DocumentResult<String> {
    if value.is_empty() || value.len() > 1024 {
        return Err(DocumentError::InvalidPath("relative document path must be 1-1024 characters".into()));
    }
    if value.starts_with('/') || value.starts_with('~') || value.contains('\\') || value.contains(':') {
        return Err(DocumentError::InvalidPath("document path must be a portable relative path".into()));
    }
    if value.chars().any(char::is_control) {
        return Err(DocumentError::InvalidPath("document path must not contain control characters".into()));
    }
    let mut count = 0usize;
    for segment in value.split('/') {
        count += 1;
        if segment.is_empty() || segment == "." || segment == ".." || segment.trim() != segment {
            return Err(DocumentError::InvalidPath("document path contains an unsafe segment".into()));
        }
    }
    if count == 0 {
        return Err(DocumentError::InvalidPath("document path is empty".into()));
    }
    Ok(value)
}

fn validate_filename(value: String) -> DocumentResult<String> {
    let value = nonblank(value, "original filename", 255)?;
    if value == "." || value == ".." || value.contains('/') || value.contains('\\') || value.contains(':') || value.chars().any(char::is_control) {
        return Err(DocumentError::InvalidDocument("original filename must be a portable basename".into()));
    }
    Ok(value)
}

fn canonical_sha256(value: String) -> DocumentResult<String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DocumentError::InvalidHash("SHA-256 must contain exactly 64 hexadecimal characters".into()));
    }
    Ok(value.to_ascii_lowercase())
}

fn nonblank(value: String, name: &str, max: usize) -> DocumentResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max {
        return Err(DocumentError::InvalidDocument(format!("{name} must be 1-{max} non-whitespace characters")));
    }
    Ok(trimmed.to_owned())
}

fn optional_nonblank(value: Option<String>, name: &str, max: usize) -> DocumentResult<Option<String>> {
    match value {
        Some(value) => Ok(Some(nonblank(value, name, max)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> RecordId { RecordId::new(value).unwrap() }
    fn root(provider: StorageProvider) -> StorageRootDescriptor {
        StorageRootDescriptor::new(id("root-1"), provider, "My documents").unwrap()
    }
    fn sample() -> DocumentReference {
        DocumentReference::from_bytes(
            id("doc-1"), id("expense-1"), &root(StorageProvider::LocalFilesystem),
            "receipts/2026/receipt.txt", "receipt.txt", Some("text/plain".into()), b"receipt body\n",
        ).unwrap()
    }

    #[test]
    fn local_and_synced_roots_are_user_controlled() {
        for provider in [
            StorageProvider::LocalFilesystem,
            StorageProvider::OneDriveFolder,
            StorageProvider::DropboxFolder,
            StorageProvider::ICloudDriveFolder,
            StorageProvider::GoogleDriveFolder,
            StorageProvider::OtherUserSelectedSyncedFolder,
        ] {
            assert_eq!(root(provider).custody(), StorageCustody::UserControlled);
        }
    }

    #[test]
    fn blank_storage_root_label_is_rejected() {
        assert!(StorageRootDescriptor::new(id("r"), StorageProvider::LocalFilesystem, "  ").is_err());
    }

    #[test]
    fn relative_document_path_is_preserved() {
        assert_eq!(sample().relative_path(), "receipts/2026/receipt.txt");
    }

    #[test]
    fn absolute_and_traversal_paths_fail_closed() {
        for path in ["/tmp/a.pdf", "C:/Users/a.pdf", "../a.pdf", "receipts/../a.pdf", "receipts\\a.pdf", "~/a.pdf"] {
            assert!(DocumentReference::from_persisted(
                id("d"), id("e"), id("r"), path, "a.pdf", None,
                "00".repeat(32), 1,
            ).is_err(), "unexpectedly accepted {path}");
        }
    }

    #[test]
    fn unsafe_original_filename_is_rejected() {
        for name in ["../receipt.pdf", "folder/receipt.pdf", "folder\\receipt.pdf", "C:receipt.pdf"] {
            assert!(DocumentReference::from_persisted(
                id("d"), id("e"), id("r"), "receipt.pdf", name, None,
                "00".repeat(32), 1,
            ).is_err());
        }
    }

    #[test]
    fn blank_media_type_is_rejected() {
        assert!(DocumentReference::from_persisted(
            id("d"), id("e"), id("r"), "receipt.pdf", "receipt.pdf", Some(" ".into()),
            "00".repeat(32), 1,
        ).is_err());
    }

    #[test]
    fn empty_document_is_rejected() {
        assert!(DocumentReference::from_bytes(
            id("d"), id("e"), &root(StorageProvider::LocalFilesystem),
            "receipt.pdf", "receipt.pdf", None, b"",
        ).is_err());
    }

    #[test]
    fn document_hash_and_length_are_derived_from_bytes() {
        let d = sample();
        assert_eq!(d.sha256(), sha256_hex(b"receipt body\n"));
        assert_eq!(d.byte_len(), 13);
    }

    #[test]
    fn uppercase_persisted_hash_is_canonicalized() {
        let hash = sha256_hex(b"x").to_ascii_uppercase();
        let d = DocumentReference::from_persisted(
            id("d"), id("e"), id("r"), "x.txt", "x.txt", None, hash, 1,
        ).unwrap();
        assert_eq!(d.sha256(), sha256_hex(b"x"));
    }

    #[test]
    fn malformed_hash_is_rejected() {
        assert!(DocumentReference::from_persisted(
            id("d"), id("e"), id("r"), "x.txt", "x.txt", None, "xyz", 1,
        ).is_err());
    }

    #[test]
    fn attachment_relationship_is_preserved() {
        let d = sample();
        assert_eq!(d.attached_record_id().as_str(), "expense-1");
        assert_eq!(d.storage_root_id().as_str(), "root-1");
    }

    #[test]
    fn persisted_reopen_roundtrip_preserves_integrity_metadata() {
        let original = sample();
        let reopened = DocumentReference::from_persisted(
            original.id.clone(), original.attached_record_id.clone(), original.storage_root_id.clone(),
            original.relative_path.clone(), original.original_filename.clone(), original.media_type.clone(),
            original.sha256.clone(), original.byte_len,
        ).unwrap();
        assert_eq!(reopened, original);
        assert_eq!(reopened.verify_bytes(b"receipt body\n"), IntegrityStatus::Verified);
    }

    #[test]
    fn tampered_same_length_document_fails_hash_check() {
        let d = sample();
        assert_eq!(d.verify_bytes(b"receipt BODY\n"), IntegrityStatus::HashMismatch);
    }

    #[test]
    fn truncated_document_fails_size_check() {
        let d = sample();
        assert_eq!(d.verify_bytes(b"receipt"), IntegrityStatus::SizeMismatch);
    }

    #[test]
    fn copy_request_uses_native_selection_id_and_relative_destination_only() {
        let d = sample();
        let request = CopySelectedDocumentRequest::new(
            NativeSelectionId::new(id("selection-1")), d.storage_root_id.clone(), d.relative_path.clone(),
            d.sha256.clone(), d.byte_len,
        ).unwrap();
        assert_eq!(request.selection_id().as_record_id().as_str(), "selection-1");
        assert_eq!(request.storage_root_id().as_str(), "root-1");
        assert_eq!(request.relative_path(), "receipts/2026/receipt.txt");
    }

    #[test]
    fn copy_request_rejects_absolute_destination() {
        assert!(CopySelectedDocumentRequest::new(
            NativeSelectionId::new(id("selection-1")), id("root"), "C:/receipt.pdf",
            "00".repeat(32), 10,
        ).is_err());
    }

    #[test]
    fn verified_read_request_is_derived_from_document_reference() {
        let d = sample();
        let request = ReadVerifiedDocumentRequest::for_reference(&d);
        assert_eq!(request.storage_root_id(), d.storage_root_id());
        assert_eq!(request.relative_path(), d.relative_path());
        assert_eq!(request.expected_sha256(), d.sha256());
        assert_eq!(request.expected_byte_len(), d.byte_len());
    }

    #[test]
    fn document_reference_contains_no_storage_credentials() {
        let d = sample();
        assert_eq!(d.storage_root_id().as_str(), "root-1");
        assert_eq!(root(StorageProvider::DropboxFolder).provider(), StorageProvider::DropboxFolder);
    }
}
