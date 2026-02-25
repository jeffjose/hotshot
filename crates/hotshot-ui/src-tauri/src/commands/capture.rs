use crate::state::AppState;
use hotshot_core::capture;
use hotshot_core::clipboard;
use hotshot_core::metadata::Metadata;
use tauri::Manager;

#[tauri::command]
pub async fn capture_fullscreen(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    display: Option<String>,
    copy_to_clipboard: Option<bool>,
) -> Result<Metadata, String> {
    // Hide window before capture
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Brief delay for window to hide
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let result = (|| -> Result<Metadata, String> {
        let display_bounds = if let Some(ref d) = display {
            let monitor = capture::resolve_display(d).map_err(|e| e.to_string())?;
            Some(monitor.to_region())
        } else {
            None
        };

        let mode = capture::CaptureMode::Fullscreen;
        let image = capture::capture(&mode, display_bounds).map_err(|e| e.to_string())?;

        let should_copy = copy_to_clipboard.unwrap_or(true);
        if should_copy {
            let _ = clipboard::copy_image(&image);
        }

        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let metadata = storage.save(&image, &mode, capture::detect_display_server().map_err(|e| e.to_string())?, None)
            .map_err(|e| e.to_string())?;

        Ok(metadata)
    })();

    // Show window after capture
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    result
}

#[tauri::command]
pub async fn capture_region(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    display: Option<String>,
    copy_to_clipboard: Option<bool>,
) -> Result<Metadata, String> {
    // Hide window before capture
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let result = (|| -> Result<Metadata, String> {
        let display_bounds = if let Some(ref d) = display {
            let monitor = capture::resolve_display(d).map_err(|e| e.to_string())?;
            Some(monitor.to_region())
        } else {
            None
        };

        let mode = capture::CaptureMode::RegionInteractive;
        let image = capture::capture(&mode, display_bounds).map_err(|e| e.to_string())?;

        let should_copy = copy_to_clipboard.unwrap_or(true);
        if should_copy {
            let _ = clipboard::copy_image(&image);
        }

        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let metadata = storage.save(&image, &mode, capture::detect_display_server().map_err(|e| e.to_string())?, None)
            .map_err(|e| e.to_string())?;

        Ok(metadata)
    })();

    // Show window after capture
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    result
}

#[tauri::command]
pub async fn capture_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    copy_to_clipboard: Option<bool>,
) -> Result<Metadata, String> {
    // Hide window before capture
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let result = (|| -> Result<Metadata, String> {
        let mode = capture::CaptureMode::ActiveWindow;
        let image = capture::capture(&mode, None).map_err(|e| e.to_string())?;

        let should_copy = copy_to_clipboard.unwrap_or(true);
        if should_copy {
            let _ = clipboard::copy_image(&image);
        }

        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let metadata = storage.save(&image, &mode, capture::detect_display_server().map_err(|e| e.to_string())?, None)
            .map_err(|e| e.to_string())?;

        Ok(metadata)
    })();

    // Show window after capture
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    result
}
