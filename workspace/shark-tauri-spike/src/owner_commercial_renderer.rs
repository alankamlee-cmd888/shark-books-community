//! SBC-8A4 bounded native save/store boundary for generated commercial PDFs.
//!
//! The webview supplies only generated PDF bytes, a portable basename, the
//! precomputed SHA-256, and an opaque session storage-root ID. Native filesystem
//! paths never cross this boundary.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use shark_books_core as core;

use super::owner_documents_ocr::NativeDocumentRootRegistry;

pub(crate) const MAX_GENERATED_PDF_BYTES: u64 = 25 * 1024 * 1024;
const MAX_GENERATED_PDF_READ_BYTES: u64 = MAX_GENERATED_PDF_BYTES + 1;
const OWNER_COMMERCIAL_RENDERER_BRIDGE_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum OwnerCommercialPdfStoreRequest {
    StoreCommercialPdf {
        storage_root_id: String,
        filename: String,
        pdf_bytes: Vec<u8>,
        expected_sha256: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommercialPdfStoreReceipt {
    bridge_version: u32,
    byte_len: u64,
    sha256: String,
    created: bool,
    relative_path: String,
    max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnerCommercialRendererError {
    code: &'static str,
    message: String,
}

impl OwnerCommercialRendererError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalidInput",
            message: message.into(),
        }
    }

    fn storage(message: impl Into<String>) -> Self {
        Self {
            code: "storageFailed",
            message: message.into(),
        }
    }

    fn integrity(message: impl Into<String>) -> Self {
        Self {
            code: "integrityFailed",
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

type OwnerCommercialRendererResult<T> = Result<T, OwnerCommercialRendererError>;

fn validate_filename(value: &str) -> OwnerCommercialRendererResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 255
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_control)
        || !trimmed.to_ascii_lowercase().ends_with(".pdf")
    {
        return Err(OwnerCommercialRendererError::invalid(
            "generated PDF filename must be a portable .pdf basename",
        ));
    }
    Ok(trimmed.to_owned())
}

fn validate_expected_sha256(value: &str) -> OwnerCommercialRendererResult<String> {
    let trimmed = value.trim();
    if trimmed.len() != 64 || !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OwnerCommercialRendererError::invalid(
            "generated PDF expected SHA-256 must contain exactly 64 hexadecimal characters",
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn read_bounded_pdf(path: &Path) -> OwnerCommercialRendererResult<Vec<u8>> {
    let file = File::open(path).map_err(|error| {
        OwnerCommercialRendererError::storage(format!(
            "generated PDF could not be opened for verification: {error}"
        ))
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_GENERATED_PDF_READ_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            OwnerCommercialRendererError::storage(format!(
                "generated PDF could not be read for verification: {error}"
            ))
        })?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_GENERATED_PDF_BYTES {
        return Err(OwnerCommercialRendererError::integrity(
            "stored generated PDF has an invalid bounded size",
        ));
    }
    Ok(bytes)
}

fn ensure_contained_parent(
    root: &Path,
    parent: &Path,
) -> OwnerCommercialRendererResult<PathBuf> {
    fs::create_dir_all(parent).map_err(|error| {
        OwnerCommercialRendererError::storage(format!(
            "generated PDF destination directory could not be created: {error}"
        ))
    })?;
    let canonical = fs::canonicalize(parent).map_err(|error| {
        OwnerCommercialRendererError::storage(format!(
            "generated PDF destination directory could not be resolved: {error}"
        ))
    })?;
    if !canonical.starts_with(root) {
        return Err(OwnerCommercialRendererError::storage(
            "generated PDF destination escapes the configured storage root",
        ));
    }
    Ok(canonical)
}

fn verify_exact_file(
    root: &Path,
    path: &Path,
    expected_sha256: &str,
    expected_len: u64,
) -> OwnerCommercialRendererResult<()> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        OwnerCommercialRendererError::storage(format!(
            "generated PDF destination could not be resolved: {error}"
        ))
    })?;
    if !canonical.starts_with(root) {
        return Err(OwnerCommercialRendererError::storage(
            "generated PDF destination escapes the configured storage root",
        ));
    }
    let bytes = read_bounded_pdf(&canonical)?;
    if bytes.len() as u64 != expected_len
        || core::bank_import::sha256_hex(&bytes) != expected_sha256
    {
        return Err(OwnerCommercialRendererError::integrity(
            "generated PDF destination does not match the authorised bytes",
        ));
    }
    Ok(())
}

fn store_pdf_bytes(
    roots: &NativeDocumentRootRegistry,
    storage_root_id: &str,
    filename: &str,
    bytes: &[u8],
    expected_sha256: &str,
) -> OwnerCommercialRendererResult<OwnerCommercialPdfStoreReceipt> {
    let filename = validate_filename(filename)?;
    let expected_sha256 = validate_expected_sha256(expected_sha256)?;

    if bytes.is_empty() || bytes.len() as u64 > MAX_GENERATED_PDF_BYTES {
        return Err(OwnerCommercialRendererError::invalid(
            "generated PDF must be between 1 byte and 25 MiB",
        ));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(OwnerCommercialRendererError::invalid(
            "generated bytes do not have a PDF signature",
        ));
    }

    let sha256 = core::bank_import::sha256_hex(bytes);
    if sha256 != expected_sha256 {
        return Err(OwnerCommercialRendererError::integrity(
            "generated PDF SHA-256 does not match the webview pre-store hash",
        ));
    }

    let root = roots
        .resolve(storage_root_id)
        .map_err(|error| OwnerCommercialRendererError::storage(error.message()))?;
    let parent = ensure_contained_parent(&root, &root.join("generated").join(&sha256))?;
    let destination = parent.join(&filename);
    let relative_path = format!("generated/{sha256}/{filename}");
    let expected_len = u64::try_from(bytes.len()).map_err(|_| {
        OwnerCommercialRendererError::invalid("generated PDF size cannot be represented")
    })?;

    let mut created = false;
    if destination.exists() {
        verify_exact_file(&root, &destination, &sha256, expected_len)?;
    } else {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| OwnerCommercialRendererError::storage("system clock is invalid"))?
            .as_nanos();
        let temp = parent.join(format!(
            ".{filename}.sbc8a4-{}-{nonce}.tmp",
            std::process::id()
        ));

        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| {
                OwnerCommercialRendererError::storage(format!(
                    "temporary generated PDF could not be created: {error}"
                ))
            })?;
        if let Err(error) = output.write_all(bytes).and_then(|_| output.sync_all()) {
            let _ = fs::remove_file(&temp);
            return Err(OwnerCommercialRendererError::storage(format!(
                "generated PDF could not be written: {error}"
            )));
        }
        drop(output);

        if let Err(error) = verify_exact_file(&root, &temp, &sha256, expected_len) {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }

        match fs::rename(&temp, &destination) {
            Ok(()) => created = true,
            Err(_) if destination.exists() => {
                let _ = fs::remove_file(&temp);
                verify_exact_file(&root, &destination, &sha256, expected_len)?;
            }
            Err(error) => {
                let _ = fs::remove_file(&temp);
                return Err(OwnerCommercialRendererError::storage(format!(
                    "generated PDF could not be finalized: {error}"
                )));
            }
        }
    }

    verify_exact_file(&root, &destination, &sha256, expected_len)?;

    Ok(OwnerCommercialPdfStoreReceipt {
        bridge_version: OWNER_COMMERCIAL_RENDERER_BRIDGE_VERSION,
        byte_len: expected_len,
        sha256,
        created,
        relative_path,
        max_bytes: MAX_GENERATED_PDF_BYTES,
    })
}

pub(crate) fn store_commercial_pdf(
    roots: &NativeDocumentRootRegistry,
    request: OwnerCommercialPdfStoreRequest,
) -> OwnerCommercialRendererResult<OwnerCommercialPdfStoreReceipt> {
    match request {
        OwnerCommercialPdfStoreRequest::StoreCommercialPdf {
            storage_root_id,
            filename,
            pdf_bytes,
            expected_sha256,
        } => store_pdf_bytes(
            roots,
            &storage_root_id,
            &filename,
            &pdf_bytes,
            &expected_sha256,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "shark-sbc8a4-commercial-renderer-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn registry(root: &Path) -> NativeDocumentRootRegistry {
        let roots = NativeDocumentRootRegistry::default();
        roots
            .register_native_root("render-root", root.to_path_buf())
            .unwrap();
        roots
    }

    fn pdf_bytes() -> Vec<u8> {
        b"%PDF-1.7\nSBC8A4 deterministic bounded fixture\n%%EOF\n".to_vec()
    }

    fn request(
        filename: &str,
        bytes: Vec<u8>,
        expected_sha256: String,
    ) -> OwnerCommercialPdfStoreRequest {
        OwnerCommercialPdfStoreRequest::StoreCommercialPdf {
            storage_root_id: "render-root".into(),
            filename: filename.into(),
            pdf_bytes: bytes,
            expected_sha256,
        }
    }

    #[test]
    fn exact_store_and_resave_are_idempotent_and_contained() {
        let root = temp_dir("idempotent");
        let roots = registry(&root);
        let bytes = pdf_bytes();
        let sha256 = core::bank_import::sha256_hex(&bytes);

        let first = store_commercial_pdf(
            &roots,
            request("invoice-0001.pdf", bytes.clone(), sha256.clone()),
        )
        .unwrap();
        assert!(first.created);
        assert_eq!(first.sha256, sha256);
        assert!(first.relative_path.starts_with("generated/"));
        assert!(!first.relative_path.contains(".."));

        let second =
            store_commercial_pdf(&roots, request("invoice-0001.pdf", bytes, sha256)).unwrap();
        assert!(!second.created);
        assert_eq!(first.relative_path, second.relative_path);
        assert_eq!(first.byte_len, second.byte_len);

        let canonical_root = fs::canonicalize(&root).unwrap();
        let stored = fs::canonicalize(root.join(&first.relative_path)).unwrap();
        assert!(stored.starts_with(canonical_root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_traversal_and_non_basename_filenames_fail_closed() {
        let root = temp_dir("traversal");
        let roots = registry(&root);
        let bytes = pdf_bytes();
        let sha256 = core::bank_import::sha256_hex(&bytes);
        for filename in [
            "../outside.pdf",
            "nested/outside.pdf",
            r"nested\outside.pdf",
            r"C:\outside.pdf",
            "..",
            "invoice.txt",
        ] {
            let error = store_commercial_pdf(
                &roots,
                request(filename, bytes.clone(), sha256.clone()),
            )
            .unwrap_err();
            assert_eq!(error.code, "invalidInput", "{filename}");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signature_size_and_pre_store_hash_are_verified() {
        let root = temp_dir("integrity");
        let roots = registry(&root);

        let not_pdf = b"not-a-pdf".to_vec();
        let not_pdf_hash = core::bank_import::sha256_hex(&not_pdf);
        assert_eq!(
            store_commercial_pdf(
                &roots,
                request("bad.pdf", not_pdf, not_pdf_hash),
            )
            .unwrap_err()
            .code,
            "invalidInput"
        );

        let bytes = pdf_bytes();
        assert_eq!(
            store_commercial_pdf(
                &roots,
                request("hash.pdf", bytes, "00".repeat(32)),
            )
            .unwrap_err()
            .code,
            "integrityFailed"
        );

        let oversized = vec![b'X'; (MAX_GENERATED_PDF_BYTES + 1) as usize];
        assert_eq!(
            store_commercial_pdf(
                &roots,
                request("large.pdf", oversized, "00".repeat(32)),
            )
            .unwrap_err()
            .code,
            "invalidInput"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tampered_existing_destination_is_rejected() {
        let root = temp_dir("tamper");
        let roots = registry(&root);
        let bytes = pdf_bytes();
        let sha256 = core::bank_import::sha256_hex(&bytes);
        let first = store_commercial_pdf(
            &roots,
            request("invoice.pdf", bytes.clone(), sha256.clone()),
        )
        .unwrap();
        fs::write(root.join(&first.relative_path), b"%PDF-tampered\n%%EOF\n").unwrap();

        let error = store_commercial_pdf(
            &roots,
            request("invoice.pdf", bytes, sha256),
        )
        .unwrap_err();
        assert_eq!(error.code, "integrityFailed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn request_schema_rejects_path_and_authority_expansion() {
        let bytes = pdf_bytes();
        let sha256 = core::bank_import::sha256_hex(&bytes);
        let base = serde_json::json!({
            "operation": "storeCommercialPdf",
            "storageRootId": "render-root",
            "filename": "invoice.pdf",
            "pdfBytes": bytes,
            "expectedSha256": sha256
        });
        assert!(serde_json::from_value::<OwnerCommercialPdfStoreRequest>(base.clone()).is_ok());
        for forbidden in [
            "path",
            "destinationPath",
            "sourcePath",
            "url",
            "accountId",
            "posting",
            "taxTreatment",
            "shareTarget",
            "email",
        ] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().insert(
                forbidden.to_string(),
                serde_json::Value::String("forbidden".into()),
            );
            assert!(
                serde_json::from_value::<OwnerCommercialPdfStoreRequest>(bad).is_err(),
                "forbidden field accepted: {forbidden}"
            );
        }
    }
}
