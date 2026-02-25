# hotshot

A fast, zero-dependency screenshot tool for Linux (X11 and Wayland).

## Install

```sh
cargo install --path crates/hotshot-cli
```

## Usage

```sh
hotshot capture fullscreen        # capture entire screen
hotshot capture region            # interactive region selection
hotshot capture region --geometry 100,200,800,600
hotshot capture active-window     # capture focused window
```

Options:

- `--format png|jpeg|webp` -- override image format
- `--clipboard` -- copy to clipboard after capture

## Managing screenshots

```sh
hotshot list                      # list recent screenshots
hotshot show <id>                 # show metadata for a screenshot
hotshot delete <id>               # move screenshot to trash
hotshot tag <id> <tag1> <tag2>    # add tags
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
