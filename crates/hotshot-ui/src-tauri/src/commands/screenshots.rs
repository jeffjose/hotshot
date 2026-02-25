use crate::state::AppState;
use hotshot_core::metadata::Metadata;

#[tauri::command]
pub fn list_screenshots(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<Metadata>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.list(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_screenshot(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Metadata, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.find_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_screenshots(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<Metadata>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.search(&query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_screenshot(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Metadata, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tag_screenshot(
    state: tauri::State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<Metadata, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage.tag(&id, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_screenshot_image(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let metadata = storage.find_by_id(&id).map_err(|e| e.to_string())?;

    let data = std::fs::read(&metadata.path)
        .map_err(|e| format!("Failed to read image file: {e}"))?;

    let mime = match metadata.format.as_str() {
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:{mime};base64,{b64}"))
}
