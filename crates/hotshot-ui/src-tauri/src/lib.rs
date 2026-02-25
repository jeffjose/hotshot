mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new().expect("Failed to initialize app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::capture::capture_fullscreen,
            commands::capture::capture_region,
            commands::capture::capture_window,
            commands::screenshots::list_screenshots,
            commands::screenshots::get_screenshot,
            commands::screenshots::search_screenshots,
            commands::screenshots::delete_screenshot,
            commands::screenshots::tag_screenshot,
            commands::screenshots::read_screenshot_image,
            commands::monitors::list_monitors,
            commands::config::get_config,
            commands::config::update_config,
        ])
        .register_asynchronous_uri_scheme_protocol("hotshot", |_ctx, request, responder| {
            std::thread::spawn(move || {
                let uri = request.uri().to_string();
                // URI format: hotshot://localhost/<path-to-image>
                let path = uri
                    .strip_prefix("hotshot://localhost/")
                    .or_else(|| uri.strip_prefix("hotshot://localhost"))
                    .unwrap_or("");
                let path = percent_decode(path);

                let file_path = std::path::Path::new(&path);
                if file_path.exists() {
                    match std::fs::read(file_path) {
                        Ok(data) => {
                            let mime = match file_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("png")
                            {
                                "png" => "image/png",
                                "jpg" | "jpeg" => "image/jpeg",
                                "webp" => "image/webp",
                                _ => "application/octet-stream",
                            };
                            responder.respond(
                                tauri::http::Response::builder()
                                    .status(200)
                                    .header("Content-Type", mime)
                                    .header("Access-Control-Allow-Origin", "*")
                                    .body(data)
                                    .unwrap(),
                            );
                        }
                        Err(e) => {
                            responder.respond(
                                tauri::http::Response::builder()
                                    .status(500)
                                    .body(format!("Failed to read file: {e}").into_bytes())
                                    .unwrap(),
                            );
                        }
                    }
                } else {
                    responder.respond(
                        tauri::http::Response::builder()
                            .status(404)
                            .body(format!("File not found: {path}").into_bytes())
                            .unwrap(),
                    );
                }
            });
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let hex = [hi, lo];
            if let Ok(s) = std::str::from_utf8(&hex) {
                if let Ok(val) = u8::from_str_radix(s, 16) {
                    result.push(val as char);
                    continue;
                }
            }
            result.push('%');
            result.push(hi as char);
            result.push(lo as char);
        } else {
            result.push(b as char);
        }
    }
    result
}
