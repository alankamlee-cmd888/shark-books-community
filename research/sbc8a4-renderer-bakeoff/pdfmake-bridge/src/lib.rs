use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_PDF_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug)]
pub struct StoreReceipt {
    pub bytes: usize,
    pub sha256: String,
    pub created: bool,
    pub relative_path: String,
}

fn validate_filename(value: &str) -> Result<String, &'static str> {
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
        return Err("INVALID_FILENAME");
    }
    Ok(trimmed.to_owned())
}

fn ensure_contained_parent(root: &Path, parent: &Path) -> Result<PathBuf, &'static str> {
    fs::create_dir_all(parent).map_err(|_| "CREATE_PARENT_FAILED")?;
    let canonical = fs::canonicalize(parent).map_err(|_| "CANONICALIZE_PARENT_FAILED")?;
    if !canonical.starts_with(root) {
        return Err("PARENT_ESCAPES_ROOT");
    }
    Ok(canonical)
}

pub fn store_pdf_bytes(
    root_arg: &Path,
    filename: &str,
    bytes: &[u8],
) -> Result<StoreReceipt, &'static str> {
    if !root_arg.is_dir() {
        return Err("ROOT_NOT_DIRECTORY");
    }
    let root = fs::canonicalize(root_arg).map_err(|_| "ROOT_CANONICALIZE_FAILED")?;
    let filename = validate_filename(filename)?;

    if bytes.is_empty() || bytes.len() as u64 > MAX_PDF_BYTES {
        return Err("PDF_SIZE_INVALID");
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err("NOT_PDF");
    }

    let sha256 = shark_books_core::bank_import::sha256_hex(bytes);
    let parent = ensure_contained_parent(&root, &root.join("generated").join(&sha256))?;
    let destination = parent.join(&filename);
    let relative_path = format!("generated/{sha256}/{filename}");

    let mut created = false;
    if destination.exists() {
        let existing = fs::read(&destination).map_err(|_| "EXISTING_READ_FAILED")?;
        if existing.len() != bytes.len()
            || shark_books_core::bank_import::sha256_hex(&existing) != sha256
        {
            return Err("EXISTING_DESTINATION_CONFLICT");
        }
    } else {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "CLOCK_INVALID")?
            .as_nanos();
        let temp = parent.join(format!(
            ".{filename}.sbc8a4-{}-{nonce}.tmp",
            std::process::id()
        ));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "TEMP_CREATE_FAILED")?;
        output.write_all(bytes).map_err(|_| "TEMP_WRITE_FAILED")?;
        output.sync_all().map_err(|_| "TEMP_SYNC_FAILED")?;
        drop(output);

        let verify = fs::read(&temp).map_err(|_| "TEMP_VERIFY_READ_FAILED")?;
        if verify.len() != bytes.len()
            || shark_books_core::bank_import::sha256_hex(&verify) != sha256
        {
            let _ = fs::remove_file(&temp);
            return Err("TEMP_INTEGRITY_FAILED");
        }
        fs::rename(&temp, &destination).map_err(|_| "FINAL_RENAME_FAILED")?;
        created = true;
    }

    let final_path = fs::canonicalize(&destination).map_err(|_| "FINAL_CANONICALIZE_FAILED")?;
    if !final_path.starts_with(&root) {
        return Err("FINAL_PATH_ESCAPES_ROOT");
    }
    let final_bytes = fs::read(&final_path).map_err(|_| "FINAL_READ_FAILED")?;
    if final_bytes.len() != bytes.len()
        || shark_books_core::bank_import::sha256_hex(&final_bytes) != sha256
    {
        return Err("FINAL_INTEGRITY_FAILED");
    }

    Ok(StoreReceipt {
        bytes: bytes.len(),
        sha256,
        created,
        relative_path,
    })
}

pub fn receipt_json(receipt: &StoreReceipt) -> String {
    format!(
        "{{\"status\":\"PASS\",\"bytes\":{},\"sha256\":\"{}\",\"created\":{},\"relative_path\":\"{}\",\"max_bytes\":{}}}",
        receipt.bytes,
        receipt.sha256,
        if receipt.created { "true" } else { "false" },
        receipt.relative_path,
        MAX_PDF_BYTES
    )
}
