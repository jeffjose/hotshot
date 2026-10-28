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
hotshot -f -o shot.jxl # save as JPEG XL (or .avif, .webp, .jpg); --format sets the format for Save
hotshot --convert jxl ~/Pictures/*.png   # convert pictures, as jog does, each beside its original
hotshot --export design.hotshot --frame 2 --scale 2 --format avif   # a canvas frame, with no window
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
| `Ctrl+E` | open it on the canvas, with its shapes still editable (or the # button at the end of the bar) |
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
| `W` | frame: one of forty backdrops, with padding, a shadow, and a browser, Mac, terminal or desktop window round the picture, with the title of the window you took in its bar |
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
by hand. An arrow or line drawn from or onto a box, oval or text sticks
to it, as Figma's connectors do: put down well inside, it points at
the box and stops at its edge; put down at the edge, it stays at that
spot. It follows when the box moves, and lets go when its end is dragged
away or it is moved on its own. With
a tool in hand and nothing selected, the same bar sits above the main
one, for how the next shape is drawn. The keys work too: `Ctrl+D`
duplicates, and `Ctrl+]` and `Ctrl+[` move a shape above or below
another. A duplicate goes beside the shape, to its right or, with no
room there in the picture, below it; move it and duplicate again, and
the next goes as far from it again, as on the canvas. Drag the dot in
the middle of an arrow or line to bend it, and just outside a corner of
a shape to rotate it. A shape being moved snaps to the edges and middles
of the others and of the picture, with a guide where it does, and to
even gaps between shapes, marked in magenta; hold Ctrl not to snap. The
selection snaps to window edges. As in Figma, hold Shift to keep
proportions (or rotate in 15° steps) and Alt to draw or resize from the
center.

![A window set in a frame, with a backdrop and a shadow](docs/frame.png)

![A spotlight, and a zoom lens that reaches past the picture](docs/zoom.png)

## Canvas

`hotshot --edit` opens your projects: folders of files in
`~/Pictures/hotshot`, with Recent, Drafts, and every capture taken in
capture mode, any of which starts a new file. `hotshot --edit FILE`
(or `--canvas`) opens a `.hotshot` file, or starts a file in Drafts with
the pictures given. Files are saved as you work, as Figma saves them.

A file has pages, listed down the left, and each page has frames: the
pictures you export, laid out side by side. `F`, or the frame tool,
drags out a new one; drag a frame by its name to move it, double-click
the name to rename it. A frame being moved snaps to the edges and
middles of the other frames, with a red line across where it does, and
to even gaps between them, marked in magenta with how wide; hold Ctrl
not to snap, and Shift to keep it to across or down.

`Ctrl+D` duplicates as Figma does. A frame's first copy goes 100 to its
right, past any frame already there; a layer's goes 20 to its right, or
below it if that would leave the frame, past anything in the way. Move
the copy and duplicate it again, and the next copy goes as far from it
as it went from the first, so a row or a grid lays itself out; anything
else done in between starts over. Alt-drag a layer, or a frame by its
name, to drag off a copy and leave it where it was; Alt on empty space
still pans. Each duplicate is one step to undo. The layers of the frame you are in are listed
under it; double-click one, or right click it and Rename, to rename it,
a box, an arrow or a blur as well as a picture (text is typed into
instead, and renamed from its menu). Zoomed far out, frames' names that
would run into each other are left out, the one you are in kept. The tools float along the bottom, grouped as Figma's are, and
the right panel holds the properties of whatever is selected, a section
at a time. With nothing selected it belongs to the frame:
its size (X post, square, 16:9, story, A4, or typed), its background (a
solid color, a linear gradient at any angle, a radial one out from the
middle, one of the frame box's backdrops, or nothing), and the export
scale. A box or an oval is filled the same way, as in Figma: Solid,
Linear or Radial, from the panel or along the top of the colors, with
a strip showing the gradient whose stops are clicked to color them; a
gradient turns with its shape, and is drawn smooth at any zoom and in
exports at any scale. On the canvas a new box or oval is filled, in a
light gray, with no border, as Figma draws one (capture mode still
outlines it); give one a border, take its fill away or change its
colors, and the next is drawn the same, even the next time the canvas
opens.

Each panel has a button at its top corner that hides it, as Claude's
sidebar button does: the button stays where it is, alone, and the page
takes the room; it brings the panel back. `Ctrl+Shift+\` hides or shows
the left panel and `Ctrl+Alt+\` the right, and the next time the canvas
opens they are as they were left. `Ctrl+\`, as in Figma, or Zen mode in
the menu, the View menu, the zoom's list or `Ctrl+K`, hides everything
but the page: both panels, the bar of tools and the rulers. The tools'
keys still work, and say the tool taken up for a moment; Esc leaves it
be, and `Ctrl+\` again, or the one button left at the top left, brings
everything back.

Pictures come from files given on the command line, from the clipboard
(`Ctrl+V`), dropped from a file manager (on the projects, a new file
begins with them; a `.hotshot` dropped opens), or from the screen (Capture, or `Ctrl+G`, which hides the
window while you take one). Drag to move, drag a handle to resize
(corners keep proportions unless Shift is held), and drag just outside a
corner to rotate, in 15° steps with Shift, with how far shown by the
pointer. Several picked out rotate together about their middle. The
pointer says what a drag would do before it starts: a curved arrow just
outside a corner, bowed the way the corner points, and a resize arrow on
a corner or a side, turned as the layer is. Every number in the panel can be typed into with a
double click, or scrubbed by dragging it sideways. A box or a picture
picked out has a dot inside each corner, as in Figma, once it is large
enough on screen: drag one in to round all four corners, or Alt-drag it
to round that corner alone. The panel has the radius for all of them,
and a button beside it that shows one for each corner. Pictures can be
rounded, shadowed, flipped, faded and cropped: double-click one, or
press Enter, to crop it. A side or corner of the crop being pulled snaps
to the lines in the picture itself, the borders, dividers and card edges
of a screenshot, with a guide drawn across it where it does and faint
ticks by the pointer for the other lines near; hold Ctrl not to snap.
Tight, on the bar over the crop, in the panel or from `Ctrl+K`, crops
to what is in the picture: a margin of one color trimmed off, then each
time it is pressed the largest box of lines inside, the window, its
pane, the cards in it, saying "Tight · 2 of 5" as it goes; Shift+Tight
goes back out. Double-click inside the crop to fit it round the box of
lines clicked in, and again for the box round that.
The markup tools draw over everything, with the
same arrow heads, fills and sketched lines as the editor. Space or the
middle button pans, Ctrl+scroll zooms, `Ctrl+0` fits the artboard and
`Ctrl+1` shows it at 100%. It zooms from 2% to 25600%, as far as
Figma's; once a picture's pixel is eight of the screen's it is drawn as
a hard square rather than smoothed, to look at a screenshot pixel by
pixel, and from 1600% a faint grid shows between the pixels. Shift-click or drag a box to select several,
and line them up from the panel. Things being moved snap to the edges
and middles of the artboard and of each other, with a guide drawn where
they do; hold Ctrl not to snap. Boxes, ovals and text keep their handles
when rotated, and text grows by its corners; arrows and lines have a
handle at each end, and stick to pictures as well as shapes.

The rarer tools are kept behind one More tools button at the end of the
bar, which shows the one in hand while it is; their keys work without
opening it. `B` blurs what is under a box, everything below it in the
frame, pictures and layers alike, as a mosaic or smoothly, as strongly
as the panel says; it is worked out at the size the frame is exported
at, so an export at 2x is blurred from pixels drawn at 2x. `D` (S being
Style Mode) puts a spotlight down: everything under it in the frame but
its box or oval is darkened, as much as the panel says, its box with
corners as round as you like. Several spotlights darken the rest once
between them, and one brought from capture mode goes just over the
picture, so what was drawn on it stays bright. `Z` drags out what a
zoom lens is of, and puts its lens beside it, round or square, twice
the size; drag the lens from inside it to move it on its own, its rim
to make it larger, or a corner of what it is of, and the lens keeps
how many times larger it shows it. `J` opens capture mode's library
over the bar, and `E` its emoji: type to search it, or pick a shelf;
a label or note clicked goes on the frame as a layer, its words picked
out for the first key to type over, and is typed into again as text is.

Hover a button, a swatch, a shadow or a style and the canvas shows what
it would do; only a click keeps it. While moving, a layer also snaps to
even gaps between its neighbors, marked in magenta; while resizing, to
widths and heights other layers have. `Shift+1` fits the frame, then the
selection; `Shift+2` the selection. With several selected the panel
arranges them as a row or a column (equal gaps, packed), matches width,
height, size, corners, shadow or the whole look to the first one picked,
puts them to the frame, turns them, and lists their colors once each:
hover one to see who uses it. `S` with a shape selected opens Style
Mode, ten styles from its own color picked with Q W E R T / A S D G H.
`Ctrl+K` searches tools, actions, layers, pages, frames and files: type
"rect" or "box", "oval", "pen", "zoom lens", "emoji", "frame" or "crop"
and the tool is listed with its key, taken up with Enter. Every
menu is gone through with the arrow keys and Enter as it is. A color
right clicked in the panel copies its hex, and a file or project right
clicked on the projects its path; an export size right clicked is
duplicated, given another format or removed, and a menu's flips show
while hovered, as the panel's buttons do. A name being typed is kept if
the window is closed before Enter. Lines of
text starting `# `, `## ` or `### ` are headings. A frame's size comes
from a list of social, device and document sizes; "Save W×H as a size"
adds yours to the config file as `frame-size = "Pixel 11 = 1080x2424"`,
shown under Yours. `Ctrl+V` on the projects starts a file from a copied
picture.

![The canvas: a screenshot on a backdrop, rounded and shadowed, with an arrow over it and its properties on the right](docs/canvas.png)

`Ctrl+E` exports the frame you are in to the screenshots folder at each
size the panel's Export lists, a new frame at 1x; a size other than 1x
with no suffix of its own is named for it, as Figma does, so twice the
size is written as `Frame 1@2x.png`. `Ctrl+C`
copies it. Ctrl+E in capture mode opens the picture there in a new file
in Drafts, its shapes still editable.

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

## Formats

A picture is written as PNG, JPEG XL or WebP with every pixel kept, or
smaller and nearly the same as JPEG XL, AVIF, WebP or JPEG, each by an
encoder built into hotshot (jixel, ravif, libwebp and mozjpeg), and all
of them, PNG and JPEG too, are read back: opened, pasted or dropped on
the canvas. The settings are jog's, aimed for every lossy format at a
SSIMULACRA2 score of about 85, very good to the eye: JPEG XL at quality
90, AVIF at 88 in full color (4:4:4), WebP and JPEG at 90; and a JPEG
turned into JPEG XL is repacked, nothing lost, rather than encoded
again. The same `[convert]` table as jog's, at the end of the config
file, changes them:

```toml
[convert]
avif = { quality = 88, speed = 6 }
jxl  = { quality = 90, lossless = true }
```

`format = "jxl"` in the config file, or `--format`, says what Save
writes; `-o shot.avif` says it by the name. What goes to the clipboard
is a PNG whichever, since other programs cannot paste AVIF or JPEG XL
yet.

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
