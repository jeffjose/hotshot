# hotshot

A fast screenshot tool for Linux (X11). Run it and the screen freezes;
drag out an area, draw on it if you like, and copy or save it. It is one
small native program: no web view, no toolkit, nothing left running in
between.

![The editor: an area selected and drawn on, with the bar of tools under it](docs/editor.png)

## Install

```sh
cargo build --release
cp target/release/hotshot ~/.local/bin/
```

Then bind `hotshot` to a key (Print, say) in your desktop's keyboard
settings.

## Use

```sh
hotshot                # freeze the screen and pick an area
hotshot -f             # the whole screen, saved at once
hotshot -w -c          # the active window, to the clipboard
hotshot -g 0,0,800,600 -o shot.png
hotshot --again        # the same area as last time
hotshot --reopen       # the last picture, its shapes still movable
hotshot -t calculator  # the window with that in its title
hotshot -D 3 --pointer # after three seconds, with the mouse pointer in it
hotshot --aspect 16:9  # an area kept to that shape; --fixed 1280x720 to a size
hotshot -f -o shot.jpg # a JPEG, or .webp; --format says it for Save
```

Drag to select an area, click a window to take it, or press Enter for
the whole screen. Then:

| Key | |
|---|---|
| `Enter` | copy to the clipboard |
| `Ctrl+S` | save to `~/Pictures/Screenshots` |
| `Esc` | cancel |
| `Ctrl+Z` / `Ctrl+Shift+Z` | undo / redo |

`hotshot --help` lists every option and key.

## Drawing

| Key | Tool |
|---|---|
| `V` | move and select |
| `A` `L` | arrow, line |
| `R` `O` | rectangle, oval |
| `P` `H` | pen, marker |
| `T` | text, in five fonts, bold, italic, highlighted, bulleted, or on a patch of color |
| `N` | numbered counters |
| `B` | blur |
| `S` | spotlight |
| `Z` | zoom lens |
| `J` `E` | badges, emoji |
| `W` | frame: one of forty backdrops, padding and a shadow |
| `M` | the mouse pointer, in the picture or out of it |
| `C` `I` | colors, color picker |
| `1`-`4` | line width and text size |

Shapes stay editable: move them, resize them, recolor them, duplicate
them with `Ctrl+D`, and put one over or under another with `Ctrl+]` and
`Ctrl+[`. An arrow or a line bends by the dot in its middle. They snap
to each other, and the selection snaps to window edges. Shift held
keeps the shape of whatever is being resized: the selection, a shape,
or the frame.

![A window set in a frame, with a backdrop and a shadow](docs/frame.png)

![A spotlight, and a zoom lens that reaches past the picture](docs/zoom.png)

## Speed

On a 4K display the frozen screen is showing 3 ms after launch, and
hotshot answers the pointer and keys within 10 to 45 ms. The X server
does the freeze itself, without a pixel leaving the graphics card; the
pixels are then read in strips, those nearest the pointer first.
`hotshot --benchmark` measures it on your machine.

## Settings

`~/.config/hotshot/config.toml` sets the defaults, using the long option
names. For a commented file with every setting in it:

```sh
hotshot --default-config > ~/.config/hotshot/config.toml
```

## Not yet

Wayland, a tray icon, and pinning a picture to the screen.

## License

MIT; see `LICENSE`. The icons are
[Solar](https://www.figma.com/community/file/1166831539721848736)'s
(CC BY 4.0). The fonts are [Geist](https://vercel.com/font),
[Instrument Serif](https://github.com/Instrument/instrument-serif),
[Architects Daughter](https://fonts.google.com/specimen/Architects+Daughter),
[Fira Code](https://github.com/tonsky/FiraCode) and
[Anton](https://fonts.google.com/specimen/Anton), all under the SIL Open
Font License; a copy of each license is in `assets`.
