# Hotshot — Roadmap

## What
Screenshot tool with CLI-first capture and a lightweight UI for annotation/organization.
Think flameshot but with proper organization, editing, and cross-platform support.

## Architecture

```
hotshot/
├── crates/
│   ├── hotshot-core/    # Shared library: capture, storage, metadata, config
│   ├── hotshot-cli/     # Pure Rust CLI binary (~2ms startup, no webview)
│   └── hotshot-ui/      # Tauri v2 + Svelte app (spawned on demand)
```

- **hotshot-cli**: lightweight, no GUI dependencies. Takes screenshots, manages files.
- **hotshot-ui**: Tauri app with Svelte frontend. Launched by CLI only when annotation/gallery is needed.
- **hotshot-core**: shared logic so both binaries stay in sync.

## Tech Stack
- **Language**: Rust (Cargo workspace)
- **UI framework**: Tauri v2 (system webview, not Electron)
- **Frontend**: Svelte + TypeScript + Canvas API
- **Config**: TOML (`~/.config/hotshot/config.toml`)
- **Metadata**: Sidecar JSON per screenshot
- **Formats**: PNG default, configurable (WebP, JPEG with quality)
- **Storage**: `~/Screenshots/` default, configurable

## Platform Support
- **Linux (primary)**: X11 + Wayland (XDG Desktop Portal for Wayland, xcb/maim for X11)
- **macOS**: CoreGraphics screen capture API
- **Windows**: win32 / DXGI screen capture API
- Runtime detection via `$XDG_SESSION_TYPE` / `$WAYLAND_DISPLAY` on Linux

## Phases

### Phase 1 — CLI Capture
- [ ] Cargo workspace setup (core, cli, ui crates)
- [ ] Config system (TOML, XDG paths, defaults)
- [ ] Fullscreen capture (X11 + Wayland)
- [ ] Region selection capture
- [ ] Window capture
- [ ] Auto-save to organized directory (`YYYY-MM/` or configurable)
- [ ] Copy to clipboard
- [ ] Sidecar JSON metadata (timestamp, dimensions, source monitor)

### Phase 2 — CLI Organization
- [ ] `hotshot list` — list recent screenshots
- [ ] `hotshot open <id>` — open in default viewer
- [ ] `hotshot tag <id> <tags>` — add tags to metadata
- [ ] `hotshot search <query>` — search by tag, date, name
- [ ] `hotshot delete <id>` — move to trash

### Phase 3 — System Tray + Hotkeys
- [ ] Tray icon (Tauri system tray)
- [ ] Global hotkey registration (configurable)
- [ ] Tray menu: capture fullscreen / region / window
- [ ] Notification on capture

### Phase 4 — Annotation UI
- [ ] Canvas-based editor (Svelte)
- [ ] Draw: rectangle, ellipse, arrow, freehand
- [ ] Text overlay
- [ ] Crop / resize
- [ ] Opacity / blur regions (redaction)
- [ ] Undo/redo
- [ ] Save annotated copy (preserve original)

### Phase 5 — Gallery UI
- [ ] Grid view of screenshots
- [ ] Filter by date, tags
- [ ] Quick preview
- [ ] Bulk actions (delete, tag, export)

### Phase 6 — Extras
- [ ] OCR text extraction (optional)
- [ ] Upload/share (configurable endpoints)
- [ ] Multi-monitor support
- [ ] Delay capture (timer)
- [ ] Video/GIF capture (stretch goal)

## Open Decisions
- Hotkey defaults (Print Screen? Super+Shift+S?)
- Gallery thumbnail generation strategy (on capture vs lazy)
- Annotation file format (SVG layer overlay vs baked PNG)
