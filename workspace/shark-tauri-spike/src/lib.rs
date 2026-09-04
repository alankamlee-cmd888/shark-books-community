use std::path::PathBuf;

#[tauri::command]
fn foundation_health() -> String {
    shark_foundation::boundary_id().to_string()
}

#[tauri::command]
fn verify_books(db_path: String) -> Result<i64, String> {
    shark_foundation::verify_books(&PathBuf::from(db_path)).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![foundation_health, verify_books])
        .run(tauri::generate_context!())
        .expect("error while running SBC-0D Tauri iOS boundary spike");
}
