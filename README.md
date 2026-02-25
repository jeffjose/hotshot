# hotshot

A fast, zero-dependency screenshot tool for Linux (X11 and Wayland).

## Install

```sh
# CLI only
cargo install --path crates/hotshot-cli

# CLI + GUI
cargo install --path crates/hotshot-cli --features gui
```

## Usage

```sh
hotshot capture fullscreen        # capture entire screen
hotshot capture region            # interactive region selection
hotshot capture region --geometry 100,200,800,600
hotshot capture window            # capture focused window
hotshot gui                       # launch the GUI (requires --features gui)
```

Options:

- `--format png|jpeg|webp` -- override image format
- `--clipboard` -- copy to clipboard after capture
- `--display <name|index>` -- target a specific monitor (see below)

## GUI

Build with the `gui` feature to enable `hotshot gui`:

```sh
cargo build -p hotshot-cli --features gui
```

For development with hot-reload:

```sh
cd crates/hotshot-ui && pnpm install && pnpm tauri dev
```

The GUI opens showing the latest screenshot. Click Capture to take a new one — the window hides, captures, copies to clipboard, and shows the result.

## Multi-monitor support

```sh
hotshot display list              # show connected monitors
hotshot capture fullscreen -d 0   # capture only the first monitor
hotshot capture fullscreen -d HDMI-1  # capture by name
hotshot capture region -d 0       # interactive selection on one monitor
```

When `--display` is used with `capture region`, the overlay and crosshair only appear on the target monitor. Other monitors remain fully interactive.

## Managing screenshots

```sh
hotshot list                      # list recent screenshots
hotshot open <id>                 # open screenshot in default viewer
hotshot delete <id>               # move screenshot to trash
hotshot tag <id> <tag1> <tag2>    # add tags
hotshot search <query>            # search by tag, note, or id
```

## Configuration

Config lives at `~/.config/hotshot/config.toml`. Created on first run.

```toml
# Base directory for screenshots
storage_dir = "~/Screenshots"

[image]
format = "png"              # png, jpeg, webp
quality = 90                # 1-100 (jpeg/webp only)
filename_template = "{timestamp}-{random}"

[storage]
organize_by = "month"       # "month" (YYYY-MM subdirs) or "none"

[behavior]
copy_to_clipboard = false   # auto-copy to clipboard after capture
notification = false        # desktop notification after capture
```

Use `hotshot config show` to view current config and `hotshot config set key=value` to change values.

## Platform support

- X11: native (uses x11rb + XRender, no external tools)
- Wayland: via XDG Desktop Portal
