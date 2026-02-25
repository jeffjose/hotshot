use hotshot_core::capture;

#[tauri::command]
pub fn list_monitors() -> Result<Vec<capture::Monitor>, String> {
    capture::list_monitors().map_err(|e| e.to_string())
}
