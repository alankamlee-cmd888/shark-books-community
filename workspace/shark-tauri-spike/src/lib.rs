//! SBC-1D minimal Windows Tauri shell for Shark Books Community.
//!
//! The webview receives no encryption secret and no arbitrary database path.
//! A proof-only Rust environment provider is used at this engineering gate; it is
//! explicitly not the release secure-storage implementation.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shark_foundation::{
    Books, BooksId, BooksKey, BooksMetadata, FoundationError, FoundationErrorCode,
    SecureKeyProvider, SiblingEncryptedBackup, TrialBalance,
};

const PROOF_KEY_ENV: &str = "SHARK_SBC1D_PROOF_KEY";
const PROOF_BOOKS_DIR_ENV: &str = "SHARK_SBC1D_BOOKS_DIR";

#[derive(Debug, Clone, Copy, Default)]
struct ProofEnvironmentKeyProvider;

impl SecureKeyProvider for ProofEnvironmentKeyProvider {
    fn load_key(&self, _books_id: &BooksId) -> Result<BooksKey, FoundationError> {
        let secret = std::env::var(PROOF_KEY_ENV).map_err(|_| {
            FoundationError::new(
                FoundationErrorCode::Storage,
                "SBC-1D proof key is unavailable in the Rust process environment",
            )
        })?;
        BooksKey::new(secret)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateBooksRequest {
    file_name: String,
    books_id: String,
    company_name: String,
    actor: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenBooksRequest {
    file_name: String,
    books_id: String,
    actor: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellBooksStatus {
    file_name: String,
    metadata: BooksMetadata,
    schema_version: i64,
    production_encryption_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellVerifyStatus {
    file_name: String,
    books_id: String,
    schema_version: i64,
    production_encryption_required: bool,
}

fn invalid_input(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::InvalidInput, message)
}

fn io_error(error: std::io::Error) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Io, error.to_string())
}

fn require_bounded_text(
    label: &str,
    value: &str,
    max_chars: usize,
) -> Result<(), FoundationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid_input(format!("{label} must not be empty")));
    }
    if trimmed.chars().count() > max_chars {
        return Err(invalid_input(format!("{label} is too long")));
    }
    Ok(())
}

fn proof_books_root() -> Result<PathBuf, FoundationError> {
    let root = std::env::var_os(PROOF_BOOKS_DIR_ENV).ok_or_else(|| {
        FoundationError::new(
            FoundationErrorCode::Storage,
            "SBC-1D proof books directory is unavailable in the Rust process environment",
        )
    })?;
    let root = PathBuf::from(root);
    if root.as_os_str().is_empty() {
        return Err(invalid_input("SBC-1D proof books directory must not be empty"));
    }
    Ok(root)
}

fn validate_file_name(file_name: &str) -> Result<(), FoundationError> {
    let Some(stem) = file_name.strip_suffix(".sqlite") else {
        return Err(invalid_input("books filename must end with .sqlite"));
    };
    if stem.is_empty() || stem.len() > 96 {
        return Err(invalid_input("books filename stem is invalid"));
    }
    if !stem
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(invalid_input(
            "books filename may contain only letters, numbers, '-' and '_' before .sqlite",
        ));
    }
    Ok(())
}

fn proof_books_path(file_name: &str) -> Result<PathBuf, FoundationError> {
    validate_file_name(file_name)?;
    Ok(proof_books_root()?.join(file_name))
}

fn create_books_impl(request: &CreateBooksRequest) -> Result<ShellBooksStatus, FoundationError> {
    require_bounded_text("company name", &request.company_name, 200)?;
    require_bounded_text("actor", &request.actor, 100)?;

    let path = proof_books_path(&request.file_name)?;
    let root = path
        .parent()
        .ok_or_else(|| invalid_input("proof books path has no parent directory"))?;
    fs::create_dir_all(root).map_err(io_error)?;

    let books_id = BooksId::new(request.books_id.clone())?;
    let provider = ProofEnvironmentKeyProvider;
    let books = Books::create_encrypted(
        &path,
        &books_id,
        &request.company_name,
        &request.actor,
        &provider,
    )?;
    let schema_version = books.verify()?;
    let metadata = books.metadata()?;

    Ok(ShellBooksStatus {
        file_name: request.file_name.clone(),
        metadata,
        schema_version,
        production_encryption_required: shark_foundation::production_encryption_required(),
    })
}

fn open_books_impl(request: &OpenBooksRequest) -> Result<Books, FoundationError> {
    require_bounded_text("actor", &request.actor, 100)?;
    let path = proof_books_path(&request.file_name)?;
    let books_id = BooksId::new(request.books_id.clone())?;
    let provider = ProofEnvironmentKeyProvider;
    let backup = SiblingEncryptedBackup;
    Books::open_encrypted(&path, &books_id, &request.actor, &provider, &backup)
}

fn open_status_impl(request: &OpenBooksRequest) -> Result<ShellBooksStatus, FoundationError> {
    let books = open_books_impl(request)?;
    let schema_version = books.verify()?;
    let metadata = books.metadata()?;
    Ok(ShellBooksStatus {
        file_name: request.file_name.clone(),
        metadata,
        schema_version,
        production_encryption_required: shark_foundation::production_encryption_required(),
    })
}

fn verify_impl(request: &OpenBooksRequest) -> Result<ShellVerifyStatus, FoundationError> {
    let books = open_books_impl(request)?;
    Ok(ShellVerifyStatus {
        file_name: request.file_name.clone(),
        books_id: books.books_id().as_str().to_string(),
        schema_version: books.verify()?,
        production_encryption_required: shark_foundation::production_encryption_required(),
    })
}

fn trial_balance_impl(request: &OpenBooksRequest) -> Result<TrialBalance, FoundationError> {
    open_books_impl(request)?.trial_balance()
}

fn ui_error(error: FoundationError) -> String {
    error.to_string()
}

#[tauri::command]
fn foundation_health() -> String {
    shark_foundation::boundary_id().to_string()
}

#[tauri::command]
fn production_encryption_required() -> bool {
    shark_foundation::production_encryption_required()
}

#[tauri::command]
fn books_create(request: CreateBooksRequest) -> Result<ShellBooksStatus, String> {
    create_books_impl(&request).map_err(ui_error)
}

#[tauri::command]
fn books_open(request: OpenBooksRequest) -> Result<ShellBooksStatus, String> {
    open_status_impl(&request).map_err(ui_error)
}

#[tauri::command]
fn books_verify(request: OpenBooksRequest) -> Result<ShellVerifyStatus, String> {
    verify_impl(&request).map_err(ui_error)
}

#[tauri::command]
fn books_trial_balance(request: OpenBooksRequest) -> Result<TrialBalance, String> {
    trial_balance_impl(&request).map_err(ui_error)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            foundation_health,
            production_encryption_required,
            books_create,
            books_open,
            books_verify,
            books_trial_balance
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shark Books Community SBC-1D shell");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn proof_key_provider() -> ProofEnvironmentKeyProvider {
        ProofEnvironmentKeyProvider
    }

    fn clean_proof_root() {
        if let Ok(root) = proof_books_root() {
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create isolated SBC-1D proof root");
        }
    }

    #[test]
    fn proof_environment_key_is_redacted() {
        let books_id = BooksId::new("sbc1d-key-proof").expect("books id");
        let key = proof_key_provider()
            .load_key(&books_id)
            .expect("validator supplies proof key");
        assert_eq!(format!("{key:?}"), "BooksKey([REDACTED])");
    }

    #[test]
    fn filename_boundary_rejects_paths_and_traversal() {
        for invalid in [
            "../outside.sqlite",
            "folder/book.sqlite",
            r"folder\book.sqlite",
            r"C:\outside.sqlite",
            ".sqlite",
            "book.db",
            "book name.sqlite",
        ] {
            assert!(
                validate_file_name(invalid).is_err(),
                "unsafe filename unexpectedly accepted: {invalid}"
            );
        }
        assert!(validate_file_name("sbc1d-proof_01.sqlite").is_ok());
    }

    #[test]
    fn encrypted_command_cycle_uses_only_shark_facade() {
        clean_proof_root();

        let create = CreateBooksRequest {
            file_name: "sbc1d-command-cycle.sqlite".to_string(),
            books_id: "sbc1d-command-cycle".to_string(),
            company_name: "Shark SBC-1D Windows Proof".to_string(),
            actor: "sbc1d-validator".to_string(),
        };

        let created = books_create(create).expect("create encrypted books command");
        assert_eq!(created.schema_version, 8);
        assert!(created.production_encryption_required);

        let path = proof_books_path(&created.file_name).expect("resolved proof path");
        let mut file = fs::File::open(&path).expect("encrypted books file exists");
        let mut header = [0_u8; 16];
        file.read_exact(&mut header).expect("read encrypted header");
        assert_ne!(&header, b"SQLite format 3\0", "books file is plaintext SQLite");

        let open = OpenBooksRequest {
            file_name: created.file_name.clone(),
            books_id: "sbc1d-command-cycle".to_string(),
            actor: "sbc1d-validator".to_string(),
        };

        let opened = books_open(OpenBooksRequest {
            file_name: open.file_name.clone(),
            books_id: open.books_id.clone(),
            actor: open.actor.clone(),
        })
        .expect("open encrypted books command");
        assert_eq!(opened.metadata.company_name, "Shark SBC-1D Windows Proof");

        let verified = books_verify(OpenBooksRequest {
            file_name: open.file_name.clone(),
            books_id: open.books_id.clone(),
            actor: open.actor.clone(),
        })
        .expect("verify encrypted books command");
        assert_eq!(verified.schema_version, 8);
        assert_eq!(verified.books_id, "sbc1d-command-cycle");

        let balance = books_trial_balance(open).expect("read-only trial balance command");
        assert!(balance.balanced);
        assert_eq!(balance.total_debits, balance.total_credits);

        clean_proof_root();
    }

    #[test]
    fn frontend_contract_is_keyless_and_uses_only_bounded_commands() {
        const INDEX: &str = include_str!("../../dist/index.html");
        const APP: &str = include_str!("../../dist/app.js");

        let unknown_secret = serde_json::json!({
            "fileName": "safe.sqlite",
            "booksId": "safe-books",
            "actor": "local-owner",
            "passphrase": "must-not-be-accepted"
        });
        assert!(
            serde_json::from_value::<OpenBooksRequest>(unknown_secret).is_err(),
            "unknown secret-like frontend fields must be rejected"
        );

        for command in [
            "foundation_health",
            "production_encryption_required",
            "books_create",
            "books_open",
            "books_verify",
            "books_trial_balance",
        ] {
            assert!(APP.contains(command), "frontend missing command {command}");
        }

        for forbidden in [
            "SHARK_SBC1D_PROOF_KEY",
            "passphrase",
            "dbPath",
            "databasePath",
            "http://",
            "https://",
        ] {
            assert!(
                !APP.contains(forbidden) && !INDEX.contains(forbidden),
                "frontend contains forbidden surface: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_acl_and_csp_are_bounded() {
        const BUILD_RS: &str = include_str!("../build.rs");
        const CONFIG: &str = include_str!("../tauri.conf.json");
        const CAPABILITY: &str = include_str!("../capabilities/default.json");
        const PERMISSION: &str = include_str!("../permissions/shark-shell.toml");

        let config: serde_json::Value = serde_json::from_str(CONFIG).expect("valid Tauri config");
        assert_eq!(
            config["app"]["withGlobalTauri"],
            serde_json::Value::Bool(true)
        );
        assert!(config["app"]["security"]["csp"].is_object());

        let capability: serde_json::Value =
            serde_json::from_str(CAPABILITY).expect("valid capability");
        assert_eq!(capability["windows"][0], "main");
        assert_eq!(capability["platforms"][0], "windows");
        assert_eq!(capability["permissions"][0], "shark-shell");
        assert!(!CAPABILITY.contains("core:default"));
        assert_eq!(config["app"]["security"]["capabilities"][0], "windows-main");
        assert_eq!(config["app"]["security"]["capabilities"].as_array().unwrap().len(), 1);

        assert!(BUILD_RS.contains("AppManifest::new().commands"));
        for command in [
            "foundation_health",
            "production_encryption_required",
            "books_create",
            "books_open",
            "books_verify",
            "books_trial_balance",
        ] {
            assert!(BUILD_RS.contains(command), "build manifest missing command {command}");
        }

        for command in [
            "foundation_health",
            "production_encryption_required",
            "books_create",
            "books_open",
            "books_verify",
            "books_trial_balance",
        ] {
            assert!(PERMISSION.contains(command));
        }
    }
}
