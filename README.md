# hotshot

A fast screenshot tool for Linux (X11). Run it and the screen freezes;
drag out an area, draw on it if you like, and copy or save it. It is one
small native program: no web view, no toolkit, no process left running
in between.

```
hotshot                # freeze the screen and pick an area
hotshot -f             # the whole screen, saved at once
hotshot -w -c          # the active window, to the clipboard
hotshot -g 0,0,800,600 -o shot.png
hotshot --again        # the same area as last time, at once
                       # (--last and --repeat say the same)
```

It is made the way jog is, and looks like it: the same font
(Geist), the same icons (Solar), the same colours.

## Speed

On a 4K display (i7-12700K, RTX 3090, X11 with XFCE), from the process
being launched:

| | |
|---|---|
| the frozen screen is showing | 3 ms |
| answering the pointer and the keys | 10 to 45 ms |
| the whole screen read | 90 to 360 ms |
| a frame while dragging out, resizing, moving or drawing | 3 to 5 ms |

The screen is frozen by the X server itself, which copies and darkens
it without a pixel leaving the graphics card; that is why it is up in
three milliseconds. Reading those pixels out of the card is the slow
part, on this card and driver: some 250 ms for the 33 MB of a 4K
screen, give or take a good deal from one run to the next. So it is
read in 32 strips, those nearest the pointer first, with the pointer
and the keys seen to between one strip and the next: hotshot answers
from the first strip, and is never deaf for longer than 14 ms. Where a
strip has not yet come, what shows is the server's darkened copy, and
anything to be drawn there is drawn the moment it does. On a display
that hands its pixels over quickly (a 1080p software one, here) the
whole of it is read 24 ms after launch.

To measure on your own machine:

```
hotshot --benchmark
```

It freezes the screen, drags an area out, resizes it, moves it, draws
one of each shape on it and packs the picture, timing each, then prints
the figures and quits. Nothing is saved or copied. `--debug` prints how
long each step of starting up took.

## Using it

With nothing selected: **drag** to select an area, **click** a window to
take it, **Enter** for the screen the pointer is on, **Esc** to cancel.
The whole screen is dark until an area is selected, and the window a
click would take has a line round it; `windows = "bright"` shows that
window undarkened instead, and `"off"` leaves windows out of it.
With `--magnifier`, or `magnifier = true` in the config file, a
magnifier follows the pointer for placing an edge to the pixel.

With an area selected, a bar of tools appears beside it:

| Key | Tool |
|---|---|
| `V` | move: the selection, or a shape, which a click picks out |
| `A` `L` | arrow, line |
| `R` `O` | rectangle (rounded), oval; `F` fills them |
| `P` `H` | pen, marker |
| `T` | text: click and type; Enter is a new line, Ctrl+Enter or Esc ends it; arrows, Home, End and a click move the caret, Ctrl+V pastes |
| `G` | the next font: Geist, Instrument Serif, Architects Daughter, Fira Code |
| `N` | counter: numbered discs, dragged to point at something |
| `B` | blur: a mosaic over what should not be read |
| `S` | spotlight: everything but a rectangle of the picture darkened |
| `C` `I` | the colours, 24 of Radix's; pick a colour off the screen |
| `K` | round the corners of the picture itself: 8, 16, 28 pixels, none; or drag the dot inside a corner of the selection |
| `1`-`4`, wheel | width of line, and size of text |

What has just been drawn is picked out, with the move tool in hand, as
in Figma: drag it to move it, drag a handle of its box to resize it, and
on a rectangle drag the dot inside a corner to round its corners. The
pointer over any other shape shows that one's box, and a click picks it
out; Shift or Ctrl with a click picks out several, to be moved,
recoloured or deleted together, and with a drag from empty space picks
out all that a box touches. A line or an arrow picked out has a dot at
each end, and is reshaped by either. **Ctrl+D** is a copy of what is
picked out. Only the pen, the marker and the counter stay in hand: the
first two for the next stroke, the counter for putting down 1, 2, 3 one
after another.

The colour, width of line, roundness, fill and font in hand when hotshot
is closed are the ones in hand when it is next opened; `--no-remember`,
or `remember = false`, begins with what the config file says every time.

**Enter** or a double click copies the picture to the clipboard,
**Ctrl+C** copies, **Ctrl+S** saves to `~/Pictures/Screenshots` and
prints the path. **Ctrl+Z** and **Ctrl+Shift+Z** undo and redo. Shift
while drawing makes squares, circles and lines at 45°. The arrow keys
nudge the selection or the shape picked out, Shift by ten pixels, and
Ctrl+arrows resize. Delete removes the shape picked out. A right click
opens the colours where the pointer is.

`hotshot --help` lists the options. `-c`, `-s` and `-o FILE` say what
Enter does; `--edit` opens the editor on `--full`, `--window` or
`--region` rather than taking the picture at once.

To have it on a key, bind `hotshot` to one in your desktop's keyboard
settings (Print, say).

## Settings

`~/.config/hotshot/config.toml` sets defaults, using the long option
names. For a file with every setting in it, commented:

```
hotshot --default-config > ~/.config/hotshot/config.toml
```

## Building

```
cargo build --release
cp target/release/hotshot ~/.local/bin/
```

Rust and nothing else: it talks to the X server itself, and carries its
font and icons. `cargo test` runs the tests, which need no display: the
editor is told of the pointer and keys and paints into a buffer, and
knows nothing of X.

    HOTSHOT_SNAPSHOTS=/tmp/shots cargo test snapshots -- --ignored

paints the editor in several of its states and writes each as a PNG, to
be looked at after a change to how it draws.

## How it is put together

| | |
|---|---|
| `main.rs` | the order of things: freeze, read, edit, pack, save, copy |
| `x11.rs` | the display: the freeze, shared memory, the window, keys |
| `strips.rs` | reading the frozen screen a strip at a time |
| `editor.rs` | the selection, the tools, the bar; nothing of X in it |
| `shapes.rs` | what is drawn, each worked out once as a coverage mask |
| `canvas.rs` | pixels: rectangles, blending, what changed between frames |
| `text.rs` | the font and the icons |
| `clipboard.rs` | owning the clipboard, from a process left behind |
| `output.rs` | PNG and file names |
| `bench.rs` | `--benchmark` |

Each frame is painted whole into a buffer, compared with the last, and
only the rectangles that differ are sent to the display, through memory
shared with the X server.

On X11 a clipboard is not a place a picture is put but a promise to hand
it over when asked, so after a copy hotshot leaves a small process
behind to keep it. It goes when something else is copied.

## Not yet

- Wayland. It reads the screen through X11.
- The mouse pointer in the picture.
- A tray icon, and anything else that stays running.
- Formats other than PNG.

## Icons and fonts

The icons are [Solar](https://www.figma.com/community/file/1166831539721848736)'s
(CC BY 4.0), drawn at a heavier line and gathered into a font by
`tools/icon_font.py`, with a few plain shapes added in the same manner.
The interface is in [Geist](https://vercel.com/font). Text can also be
typed in [Instrument Serif](https://github.com/Instrument/instrument-serif),
[Architects Daughter](https://fonts.google.com/specimen/Architects+Daughter)
and [Fira Code](https://github.com/tonsky/FiraCode). All four are under
the SIL Open Font License, a copy of which is beside each in `assets`.
