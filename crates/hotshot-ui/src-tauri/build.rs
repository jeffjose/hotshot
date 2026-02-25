use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ui_dir = manifest_dir.join("..");
    let frontend_dir = ui_dir.join("build");

    if !frontend_dir.join("index.html").exists() {
        eprintln!("Building frontend...");

        // Use local vite from node_modules
        let vite = ui_dir.join("node_modules/.bin/vite");
        assert!(
            vite.exists(),
            "node_modules not found — run `pnpm install` in crates/hotshot-ui/ first"
        );

        let status = Command::new(vite)
            .args(["build"])
            .current_dir(&ui_dir)
            .status()
            .expect("failed to run vite build");
        assert!(status.success(), "vite build failed");
    }

    tauri_build::build()
}
