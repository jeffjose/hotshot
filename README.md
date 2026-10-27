# hotshot

A fast screenshot tool for Linux. It is built for X11, and also works on
Wayland through the desktop's portal. Run it and the screen freezes: drag
out an area, draw on it if you like, then copy, save, pin or record it.
It is one small native program with no web view and no toolkit, and
nothing stays running in the background unless you ask for the tray icon.

![The editor: an area selected and drawn on, with the bar of tools under it](docs/editor.png)

## Install

```sh
cargo build --release
cp target/release/hotshot ~/.local/bin/
```

Then bind `hotshot` to a key (Print, for example) in your desktop's
keyboard settings. To add it to your application menu with its icon:

```sh
cp assets/hotshot.desktop ~/.local/share/applications/
cp assets/hotshot.svg ~/.local/share/icons/hicolor/scalable/apps/
```

## Use

```sh
hotshot                # freeze the screen and pick an area
hotshot -f             # capture the whole screen and save it immediately
hotshot -w -c          # copy the active window to the clipboard
hotshot -g 0,0,800,600 -o shot.png
hotshot --again        # capture the last screenshot again, with the same corners, frame and backdrop
hotshot --history      # list earlier screenshots and pick one to capture again
hotshot -a @2          # capture the one before last again; -a 4djc picks one by its name
hotshot --reopen       # reopen the last screenshot with its shapes still editable
hotshot -t calculator  # capture the window with "calculator" in its title
hotshot -D 3 --pointer # wait three seconds, and include the mouse pointer
hotshot --aspect 16:9  # lock the area to an aspect ratio; --fixed 1280x720 locks the size
hotshot -f -o shot.jpg # save as JPEG (or .webp); --format sets the format for Save
hotshot --pin          # pin the screenshot on screen, above other windows
hotshot --record       # record an area; run again to stop (needs ffmpeg)
hotshot --tray         # show an icon in the panel's tray; --menu opens its menu
```

Drag to select an area, click a window to select it, or press Enter for
the whole screen. Then:

| Key | |
|---|---|
| `Enter` | copy to the clipboard |
| `Ctrl+S` | save to `~/Pictures/Screenshots` |
| `Ctrl+E` | open it on the canvas, with its shapes still editable |
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
| `T` | text: five fonts, bold, italic, highlighted, bulleted, or on a colored background |
| `N` | numbered counters |
| `B` | blur |
| `S` | spotlight |
| `Z` | zoom lens |
| `J` `E` | the library: labels, status dots, key caps, tooltips, sticky notes, pins and the rest; and emoji, all searchable |
| `W` | frame: one of forty backdrops, with padding, a shadow, and a browser, Mac, terminal or desktop window round the picture |
| `M` | show or hide the mouse pointer in the screenshot |
| `C` `I` | colors, color picker |
| `F` | fill a rectangle or oval, or not |
| `1`-`4` | line width and text size |

Shapes stay editable: select one and a small bar appears above it with
what you can change about it (color, width, font and text style) and
buttons to bring it forward or send it backward (Shift-click for all the
way), duplicate it or delete it. Rectangles and ovals have a border and
a fill, each in its own color or none. An arrow is made of parts: a head
at either end (filled, open, hollow, dot, ring, diamond, bar or none), a
straight or elbow path, and a solid, dashed or dotted line, with a white
edge for busy pictures. Boxes, ovals and arrows can be sketched, drawn
by hand. With
a tool in hand and nothing selected, the same bar sits above the main
one, for how the next shape is drawn. The keys work too: `Ctrl+D`
duplicates, and `Ctrl+]` and `Ctrl+[` move a shape above or below
another. Drag the dot in the middle of an arrow or line to bend it, and
just outside a corner of a shape to rotate it. Shapes snap to each other,
and the selection snaps to window edges. As in Figma, hold Shift to keep
proportions (or rotate in 15° steps) and Alt to draw or resize from the
center.

![A window set in a frame, with a backdrop and a shadow](docs/frame.png)

![A spotlight, and a zoom lens that reaches past the picture](docs/zoom.png)

## Canvas

`hotshot --canvas [FILE...]` opens a window for making a picture out of
pictures, laid out as Figma is: tools and actions along the top, layers
down the left, and the properties of whatever is selected down the
right. With nothing selected, the right panel belongs to the artboard:
its size (X post, square, 16:9, story, A4, or typed), its background (a
color, one of the frame box's backdrops, or nothing), and the export
scale.

Pictures come from files given on the command line, from the clipboard
(`Ctrl+V`), or from the screen (Capture, or `Ctrl+G`, which hides the
window while you take one). Drag to move, drag a handle to resize
(corners keep proportions unless Shift is held), and drag just outside a
corner to rotate. Every number in the panel can be typed into with a
double click, or scrubbed by dragging it sideways. Pictures can be
rounded, shadowed, flipped, faded and cropped: double-click one, or
press Enter, to crop it. The markup tools draw over everything, with the
same arrow heads, fills and sketched lines as the editor. Space or the
middle button pans, Ctrl+scroll zooms, `Ctrl+0` fits the artboard and
`Ctrl+1` shows it at 100%. Shift-click or drag a box to select several,
and line them up from the panel.

![The canvas: a screenshot on a backdrop, rounded and shadowed, with an arrow over it and its properties on the right](docs/canvas.png)

`Ctrl+E` exports a PNG to the screenshots folder at 1x, 2x or 3x, `Ctrl+C`
copies it, and `Ctrl+S` saves everything in a `.hotshot` file, with the
pictures inside it, which `hotshot --canvas` opens again.

## Pinning

`Ctrl+P`, the pin button, or `--pin` keeps the screenshot on screen in a
borderless window above all others. Drag to move it, scroll to resize
it, press `Ctrl+C` to copy it again, and press `Esc` or double-click to
close it. `hotshot --show pic.png` pins a PNG from a file.

## Recording

`hotshot --record` (or `Ctrl+R`, or the record button) records the area
instead of taking a screenshot. The output is an MP4 at 60 frames per
second by default, or a GIF, APNG or WebP if you pass `--record-format`
or give `-o` a file name with that extension. A red outline marks the
area, with a stop button beside it. Running `hotshot --record` again
also stops the recording, so a single key can both start and stop it.
`--keys` shows the keys you press along the bottom, like subtitles.
Recording requires `ffmpeg`; GIFs are made with `gifski` when it is
installed.

## Tray

`hotshot --tray` keeps an icon in the panel. Clicking it starts hotshot,
and its menu offers the other ways of starting it. It uses the standard
tray protocol (StatusNotifierItem over D-Bus), which works with the XFCE
and KDE panels, waybar and similar, and GNOME with the AppIndicator
extension. Where that isn't available, it falls back to the older X11
system tray. Add it to your session's autostart to have it from login.
`hotshot --menu` opens the same menu at the pointer without an icon,
which is useful for binding to a key.

## Speed

On a 4K display, the frozen screen appears 3 ms after launch, and
hotshot responds to the pointer and keys within 10 to 45 ms. The X
server performs the freeze itself, so no pixels leave the graphics card
at that point. The pixels are then read in strips, starting with those
nearest the pointer. `hotshot --benchmark` measures this on your machine.

## Settings

`~/.config/hotshot/config.toml` sets the defaults, using the long option
names. To generate a commented file with every setting:

```sh
hotshot --default-config > ~/.config/hotshot/config.toml
```

## Wayland

Wayland doesn't let programs read the screen directly, so hotshot asks
the desktop for a screenshot through its portal (GNOME, KDE, and wlroots
compositors with a portal installed), then opens it for editing as it
does on X11. hotshot's own window is an X11 window, so the session needs
XWayland, which nearly all sessions run. The differences on Wayland are:

- Startup takes as long as the desktop needs to provide the screenshot,
  rather than three milliseconds, and the desktop may ask for permission
  first.
- It opens one window per monitor, whereas on X11 a single window covers
  them all.
- On a scaled display, the screenshot has the desktop's logical
  resolution rather than the screen's physical resolution, and monitors
  with different scale factors may not line up.
- The clipboard uses `wl-copy` when it is installed (`wl-clipboard`),
  which is the most reliable option.
- A recording captures a whole monitor, which the desktop asks you to
  choose the first time, and the area is cropped out afterwards. This
  needs GStreamer with its PipeWire plugin (`gst-launch-1.0`,
  `gstreamer1.0-pipewire`) as well as ffmpeg. Stop it by running
  `hotshot --record` again or with the desktop's own screen-sharing
  indicator. There is no outline, no stop button and no key display.
- The mouse pointer can't be included in a screenshot, there is no
  snapping to windows, and `-w` can't find the active window.

## Does it work here?

```sh
hotshot --check        # add -c to try the clipboard as well
```

This tries each thing hotshot does on the current machine: reading the
screen, opening the window and receiving keys, saving, pinning, a second
of recording, and the tray. It reports which worked and what went wrong
with the rest. It briefly opens a window and closes it again, and keeps
none of the files it creates. `cargo test` covers the rest: the logic
that can be tested without a display.

## Not yet

Zooming in on part of a recording.

## License

MIT; see `LICENSE`. The icons are from
[Solar](https://www.figma.com/community/file/1166831539721848736)
(CC BY 4.0). The fonts are [Geist](https://vercel.com/font),
[Instrument Serif](https://github.com/Instrument/instrument-serif),
[Architects Daughter](https://fonts.google.com/specimen/Architects+Daughter),
[Fira Code](https://github.com/tonsky/FiraCode) and
[Anton](https://fonts.google.com/specimen/Anton), all under the SIL Open
Font License; a copy of each license is in `assets`.
