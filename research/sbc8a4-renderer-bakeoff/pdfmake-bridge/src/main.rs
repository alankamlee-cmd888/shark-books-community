use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PDF_BYTES: u64 = 25 * 1024 * 1024;

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn validate_filename(value: &str) -> String {
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
        fail("INVALID_FILENAME");
    }
    trimmed.to_owned()
}

fn ensure_contained_parent(root: &Path, parent: &Path) -> PathBuf {
    fs::create_dir_all(parent).unwrap_or_else(|_| fail("CREATE_PARENT_FAILED"));
    let canonical = fs::canonicalize(parent).unwrap_or_else(|_| fail("CANONICALIZE_PARENT_FAILED"));
    if !canonical.starts_with(root) {
        fail("PARENT_ESCAPES_ROOT");
    }
    canonical
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        fail("USAGE: bridge <storage-root> <filename>");
    }
    let root_arg = PathBuf::from(&args[1]);
    if !root_arg.is_dir() {
        fail("ROOT_NOT_DIRECTORY");
    }
    let root = fs::canonicalize(&root_arg).unwrap_or_else(|_| fail("ROOT_CANONICALIZE_FAILED"));
    let filename = validate_filename(&args[2]);

    let mut bytes = Vec::new();
    io::stdin()
        .take(MAX_PDF_BYTES + 1)
        .read_to_end(&mut bytes)
        .unwrap_or_else(|_| fail("STDIN_READ_FAILED"));
    if bytes.is_empty() || bytes.len() as u64 > MAX_PDF_BYTES {
        fail("PDF_SIZE_INVALID");
    }
    if !bytes.starts_with(b"%PDF-") {
        fail("NOT_PDF");
    }

    let sha256 = shark_books_core::bank_import::sha256_hex(&bytes);
    let parent = ensure_contained_parent(&root, &root.join("generated").join(&sha256));
    let destination = parent.join(&filename);
    let relative_path = format!("generated/{sha256}/{filename}");

    let mut created = false;
    if destination.exists() {
        let existing = fs::read(&destination).unwrap_or_else(|_| fail("EXISTING_READ_FAILED"));
        if existing.len() != bytes.len()
            || shark_books_core::bank_import::sha256_hex(&existing) != sha256
        {
            fail("EXISTING_DESTINATION_CONFLICT");
        }
    } else {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| fail("CLOCK_INVALID"))
            .as_nanos();
        let temp = parent.join(format!(".{filename}.sbc8a4-{}-{nonce}.tmp", std::process::id()));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .unwrap_or_else(|_| fail("TEMP_CREATE_FAILED"));
        output.write_all(&bytes).unwrap_or_else(|_| fail("TEMP_WRITE_FAILED"));
        output.sync_all().unwrap_or_else(|_| fail("TEMP_SYNC_FAILED"));
        drop(output);

        let verify = fs::read(&temp).unwrap_or_else(|_| fail("TEMP_VERIFY_READ_FAILED"));
        if verify.len() != bytes.len()
            || shark_books_core::bank_import::sha256_hex(&verify) != sha256
        {
            let _ = fs::remove_file(&temp);
            fail("TEMP_INTEGRITY_FAILED");
        }
        fs::rename(&temp, &destination).unwrap_or_else(|_| fail("FINAL_RENAME_FAILED"));
        created = true;
    }

    let final_path = fs::canonicalize(&destination).unwrap_or_else(|_| fail("FINAL_CANONICALIZE_FAILED"));
    if !final_path.starts_with(&root) {
        fail("FINAL_PATH_ESCAPES_ROOT");
    }
    let final_bytes = fs::read(&final_path).unwrap_or_else(|_| fail("FINAL_READ_FAILED"));
    if final_bytes.len() != bytes.len()
        || shark_books_core::bank_import::sha256_hex(&final_bytes) != sha256
    {
        fail("FINAL_INTEGRITY_FAILED");
    }

    println!(
        "{{\"status\":\"PASS\",\"bytes\":{},\"sha256\":\"{}\",\"created\":{},\"relative_path\":\"{}\",\"max_bytes\":{}}}",
        bytes.len(),
        sha256,
        if created { "true" } else { "false" },
        relative_path,
        MAX_PDF_BYTES
    );
}
