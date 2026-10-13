# hotshot

A fast screenshot tool for Linux (X11). Run it and the screen freezes;
drag out an area, draw on it if you like, and copy, save, pin or record
it. It is one small native program: no web view, no toolkit, and
nothing left running in between unless its tray icon is asked for.

![The editor: an area selected and drawn on, with the bar of tools under it](docs/editor.png)

## Install

```sh
cargo build --release
cp target/release/hotshot ~/.local/bin/
```

Then bind `hotshot` to a key (Print, say) in your desktop's keyboard
settings. For it to be in the list of programs, with its icon:

```sh
cp assets/hotshot.desktop ~/.local/share/applications/
cp assets/hotshot.svg ~/.local/share/icons/hicolor/scalable/apps/
```

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
hotshot --pin          # the picture left on the screen, over everything
hotshot --record       # record an area; again to stop (needs ffmpeg)
hotshot --tray         # an icon in the panel's tray; --menu is its list
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
| `J` `E` | badges in twelve looks and any color, emoji |
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

## Pinning

`Ctrl+P`, the pin button, or `--pin` leaves the picture on the screen in
a window of its own with nothing round it, over every other. Drag it
about, turn the wheel to make it larger or smaller, `Ctrl+C` to copy it
again, `Esc` or a double click to be done with it. `hotshot --show
pic.png` pins a PNG from a file.

## Recording

`hotshot --record` (or `Ctrl+R`, or the record button) records the area
instead of taking a picture of it: an MP4 at 60 frames a second, or a
GIF, APNG or WebP with `--record-format` or by what the `-o` name ends
in. A red line shows round the area and a button beside it stops it; so
does running `hotshot --record` again, so one key both starts and stops.
`--keys` says the keys pressed along the bottom, as subtitles are.
Recording is done by `ffmpeg`, which has to be installed; a GIF is made
by `gifski` where that is too.

## Tray

`hotshot --tray` keeps an icon in the panel: a click is hotshot, and its
menu has the other ways of starting it. It speaks the tray every
desktop has now (StatusNotifierItem, over D-Bus): XFCE's panel, KDE's,
waybar and the like, and GNOME with the AppIndicator extension; where
there is none it falls back to the tray X11 has always had. Add it to
your session's autostart to have it from login. `hotshot --menu` is the
same menu where the pointer is, with no icon, for a key.

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

Taking pictures under Wayland: hotshot reads the screen through X11,
so in a Wayland session its tray icon shows and what it starts cannot
take a picture yet. And zooming in on part of a recording.

## License

MIT; see `LICENSE`. The icons are
[Solar](https://www.figma.com/community/file/1166831539721848736)'s
(CC BY 4.0). The fonts are [Geist](https://vercel.com/font),
[Instrument Serif](https://github.com/Instrument/instrument-serif),
[Architects Daughter](https://fonts.google.com/specimen/Architects+Daughter),
[Fira Code](https://github.com/tonsky/FiraCode) and
[Anton](https://fonts.google.com/specimen/Anton), all under the SIL Open
Font License; a copy of each license is in `assets`.
