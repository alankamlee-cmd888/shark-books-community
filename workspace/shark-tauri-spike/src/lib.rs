#[tauri::command]
fn foundation_health() -> String {
    shark_foundation::boundary_id().to_string()
}

#[tauri::command]
fn production_encryption_required() -> bool {
    shark_foundation::production_encryption_required()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            foundation_health,
            production_encryption_required
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shark SBC-1C boundary shell");
}
