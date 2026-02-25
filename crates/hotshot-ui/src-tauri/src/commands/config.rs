use crate::state::AppState;
use hotshot_core::config::Config;

#[tauri::command]
pub fn get_config(
    state: tauri::State<'_, AppState>,
) -> Result<Config, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
pub fn update_config(
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
) -> Result<Config, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.set_value(&key, &value)?;
    config.save().map_err(|e| e.to_string())?;

    // Update storage with new config
    let mut storage = state.storage.lock().map_err(|e| e.to_string())?;
    *storage = hotshot_core::storage::Storage::new(config.clone());

    Ok(config.clone())
}
