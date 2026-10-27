use std::ffi::OsString;
use std::path::PathBuf;

use crate::canvas::Rect;
use crate::editor::{Finish, Settings, Windows, PALETTE};
use crate::frame::{self, Backdrop, Shadow, BACKDROPS, PADS};
use crate::output::{Format, Video};

pub const USAGE: &str = "\
Usage: hotshot [OPTIONS]

Freezes the screen. Drag out an area, draw on it if you like, and copy
or save it. With --full, --monitor, --window or --region there is
nothing to drag out: the picture is taken at once.

What to take:
  -f, --full                 the whole screen, every monitor of it
  -m, --monitor N            one monitor, as --monitors numbers them, or
                             'pointer' for the one the pointer is on
  -w, --window               the window that has the keyboard
  -i, --choose               one of a list: the whole screen, each display
                             where there are several, and the windows.
                             They are listed with numbers, and the one
                             whose number is typed is opened selected, to
                             be drawn on
  -t, --title TEXT           the window with TEXT in its title, whatever
                             the capitals; the topmost, if several have.
                             Taken at once, as the window's own picture,
                             whatever is in front of it
  -g, --region X,Y,W,H       this rectangle of the screen (also WxH+X+Y)
  -a, --again [WHICH]        the last picture again: the same area, and
                             set out the same way, with the same corners,
                             frame, backdrop, room, shadow and pointer.
                             WHICH is another taken before: @2 for the
                             one before the last, and so on, or the name
                             --history gives it, which is its own for
                             good. --last and --repeat say the same
      --history              list the pictures taken before, to take one
                             again; where there is a terminal, its number
                             or name typed takes it
  -r, --reopen               the last picture again, where it was, with what
                             was drawn on it still to be moved and changed
  -e, --edit                 with any of those: open with that area selected,
                             to be drawn on, rather than taking it at once;
                             by itself, your projects; with files, the
                             same as --canvas
  -D, --delay SEC            wait before freezing the screen

What to do with it:
  -c, --copy                 copy it to the clipboard
  -s, --save                 save it in the screenshots folder, and print
                             where
  -p, --pin                  leave it on the screen, over everything, in a
                             window of its own: drag it about, turn the
                             wheel to make it larger or smaller, Esc or a
                             double click to be done with it (Ctrl+C copies
                             it again, Ctrl+S saves it)
      --show FILE            leave a PNG from a file on the screen the same
                             way; --at X,Y says where
  -o, --output FILE          save it as FILE instead; - is standard output
  -d, --dir DIR              the screenshots folder (~/Pictures/Screenshots)
      --filename NAME        what a saved picture is called, less the .png,
                             with the time as strftime sets it out
                             (screenshot-%Y-%m-%d-%H%M%S)
      --format KIND          png (the default), jpg or webp; a name given
                             with -o that ends in one of them says so itself
      --quality N            how good a JPEG is, 1 to 100 (90)
      --pointer              with the mouse pointer in the picture (in the
                             editor, the button for it or M puts it in and
                             takes it out)
      --aspect W:H           keep the area dragged out to a shape, as 16:9
      --fixed WxH            or to a size in pixels, as 1280x720: a press
                             puts it down and a drag moves it
  When dragging an area out, these say what Enter does; the usual is to
  copy. Taken at once, a picture is saved unless told otherwise.

Recording, which needs ffmpeg (and under Wayland GStreamer with its
PipeWire plugin, where the desktop asks what to share the first time
and there is no line, button or keys):
  -R, --record               record the area, not take a picture of it:
                             drag it out and press Enter, or name it with
                             -g, -f, -w or -m. A line shows round it, and
                             a button beside it stops it; so does --record
                             again, so that one key starts and stops
      --record-format KIND   mp4 (the default), gif, apng or webp; a name
                             given with -o says so itself by how it ends
      --fps N                frames a second (60; a GIF has 50 at most)
      --duration SEC         stop by itself after so long
  -k, --keys                 say the keys pressed along the bottom of it,
                             as subtitles are
      --no-record-pointer    leave the mouse pointer out of it
  -c                         with --record: the file on the clipboard

Making a picture of pictures:
      --canvas [FILE...]     a window to make one in, as in Figma: files of
                             pages and frames, kept as you work; pictures
                             from files, the clipboard (Ctrl+V) or the
                             screen (Capture), moved, sized, turned,
                             cropped (double click), rounded and
                             shadowed, with shapes over them, on a color
                             or a backdrop. Ctrl+E exports it as a PNG,
                             Ctrl+C copies it, and Ctrl+S keeps all of it
                             in a .hotshot file, which --canvas opens

Staying about:
      --tray                 an icon in the panel's tray: a click is
                             hotshot, and its menu the other ways of
                             starting it. In the tray every desktop has
                             now, kept over D-Bus; or X11's own, failing
      --menu                 that list by itself, where the pointer is

Look:
      --accent COLOR         color of the selection and of what is picked out:
                             RRGGBB, or one of the Radix colors such as
                             indigo (the default), blue, crimson or teal
      --color COLOR          color to draw in to begin with: red (the
                             default) or any other of the Radix colors
                             the color box has, white, black or RRGGBB
      --size N               width of line to begin with, 1 to 4
      --corner-radius N      how round a drawn rectangle's corners are; 0 is
                             square
      --border-radius N      round the corners of the picture itself by N
                             pixels (of a screen 1080 high, so that it looks
                             the same on any), leaving what is outside them
                             see-through: taken at once, and to begin with
                             in the editor, where the dot inside a corner of
                             the selection changes it and K goes through 8,
                             16 and 28. --rounded says the same
      --dim PERCENT          how much darker what is not selected is made
      --magnifier            a magnifier beside the pointer while an edge is
                             being placed (config: magnifier = true)
      --windows HOW          before anything is selected, how the window
                             under the pointer, which a click would take,
                             shows: outline (the default): a line round it,
                             the screen all dark; bright: undarkened, which
                             is most of the screen where a window fills it;
                             off: not at all, and a click takes no window
      --font NAME            the face text is typed in to begin with: geist
                             (sans, the default), instrument-serif (serif),
                             architects-daughter (hand), fira-code (mono)
                             or anton (impact), which is for captions
      --window-frame KIND    show the picture in a window: browser, mac,
                             terminal or desktop; none, the usual
      --backdrop NAME        set the picture on a backdrop: aurora, orchid,
                             sorbet, glacier, dusk, graphite or any other
                             of the forty the box of them names (W, then
                             the dots; a wrong name here lists them all);
                             photo, the one --photo names; random, a
                             photograph nobody chose; none, nothing, which
                             with --shadow is a shadow on nothing at all
      --padding N            with this much room round it: 0 to 3, or past
                             that so many pixels; or two, beside it and
                             above and below it, as 80x40
      --frame-radius N       how round the corners of the frame are
      --shadow [WHICH]       and a shadow under it: Tailwind's 2xs, xs, sm,
                             md, lg, xl or 2xl (the usual), or as CSS's
                             box-shadow says it, up to four of them, as
                             --shadow '0 10px 15px -3px rgb(0 0 0 / 0.1)';
                             CSS that begins with a minus goes after =
      --photo FILE           a PNG or JPEG of your own to set it on, which
                             the box of backdrops then offers
      --filled               rectangles and ovals filled to begin with
      --fill-color COLOR     what they are filled with (the usual: --color)
      --no-border            and with no border, only a fill
      --opacity PERCENT      how opaque what is drawn is to begin with
      --dashed               lines and arrows in dashes to begin with
      --arrow-heads WHICH    end (the usual), start or both
      --zoom round|square    the shape a zoom is drawn in
      --tool NAME            the tool in hand to begin with: move, arrow,
                             line, rect, oval, pen, marker, text, counter,
                             blur, spotlight or zoom
  Any of these, and of all the long options, can go in the config file
  instead, as name = value: hotshot --default-config shows how.
      --no-remember          begin with the color, width, roundness, fill
                             and font the config file says, not the ones
                             in hand when hotshot was last used

Other:
      --check                try each thing hotshot does on this machine,
                             and say which worked: the screen read, the
                             window and its keys, saving, pinning, a
                             second of recording, the tray. With -c the
                             clipboard too, which takes the place of
                             what is on it. Nothing it makes is kept
      --monitors             list the monitors and quit
      --benchmark            freeze the screen, drag, draw and encode by itself,
                             timing each, print the figures and quit
      --debug                print how long each step of starting up took
                             (also HOTSHOT_DEBUG=1)
      --default-config       print a config file with every setting in it
  -h, --help                 show this help
  -v, --version              show the version

Keys, once an area is selected:
  Enter           copy (or what -c, -s and -o say)     Esc     cancel
  Ctrl+C  copy    Ctrl+S  save    Ctrl+Z  undo    Ctrl+Shift+Z  redo
  Ctrl+P  pin to the screen       Ctrl+R  record this area
  Ctrl+E  open it on the canvas, its shapes still to be changed
  V move   A arrow   L line   R rectangle   O oval   P pen   H marker
  T text   N counter   B blur   S spotlight   Z zoom   G the next font
  J badges   E emoji (type to search)   W backdrop, room and shadow
  D dashes   M the mouse pointer, in the picture or out of it
  C colors   I pick a color off the screen
  In text, Enter is a new line; Ctrl+Enter, Esc or a click elsewhere ends
  it; the arrows, Home, End and a click move the caret; Ctrl+V pastes;
  Ctrl+A picks all of it out, to be typed over
  F fill     1-4 or the wheel: width of line   Shift: square, circle, 45\u{b0}
  The dot inside each corner of the selection rounds the corners of the
  picture as it is dragged in; K does it by steps, and then not at all
  What is drawn is picked out, with the move tool in hand: drag it, or a
  handle of its box to resize it, or the dot in a rectangle's corner to
  round it; a line or an arrow, by either of its ends, and a click on an
  end gives it an arrow's head or takes it away. Shift or Ctrl with
  a click picks out several, and with a drag from empty space all that a
  box touches. Ctrl+D is a copy of what is picked out.
  A zoom is round, and F makes one square; the dot in its middle moves
  its larger picture, and the one on its rim says how many times larger.
  It may go past the selection, and the picture is then as large as takes
  it in. A badge is one of twelve looks, in the color in hand: type what
  it says and press Enter; the box of colors changes it afterwards, and
  a double click types over it.
  The frame shows round the selection: drag beside the picture to make
  it wider, above or below it to make it taller, and the dot inside its
  corner to round its corners; click it for the backdrops, a photograph
  nobody chose, and the shadow.
  A tool with a mark in the corner of its button has more ways than one
  of drawing: press the mark, or the tool again, for the list.
  Text being typed or picked out has a bar over it: heavy (Ctrl+B),
  slanted (Ctrl+I), gone over with a marker, and each line after a dot.
  The box of colors also says how opaque. A shape being moved goes to
  the edges and middles of the others; Ctrl held leaves it be.
  An edge of the selection goes to the edge of a window, or of what is
  on the screen, near it or along its line; Ctrl held leaves it where it
  is put. As in Figma: Shift held keeps an area square as it is dragged
  out, the selection, a shape or the frame the shape it has as it is
  made larger or smaller, and what is moved to across or down; Alt held
  draws, or makes larger or smaller, about the middle. X puts up the
  loupe, and puts it away again.
  Just outside a corner of the shape picked out, a drag turns it about
  its middle; Shift turns it by fifteen degrees at a time.
  The box of colors has every color, in its square and strip of hues;
  # or a click on its hex lets one be typed, Enter taking it.
  Ctrl+] puts what is picked out over the shape above it, Ctrl+[ under
  the one below; with Shift, over or under them all.
  A line or an arrow picked out has a dot half way along it: dragged, it
  bends the line through where it is taken; clicked, it straightens it.
  The bar over text also sets it on a patch of its own color.
  Arrows nudge the selection, or the shape picked out; Shift by ten;
  Ctrl+arrows resize. Delete removes the shape picked out. Ctrl+A is the
  whole screen. A right click is the colors.

Config file:
  ~/.config/hotshot/config.toml sets defaults, using the long option names:
      dir = \"~/Pictures/shots\"
      enter = \"both\"
      magnifier = true
";

/// What the picture is of.
#[derive(Clone, PartialEq, Debug)]
pub enum Take {
    /// An area dragged out by hand.
    Pick,
    Full,
    /// A monitor by its number, or the one the pointer is on.
    Monitor(Option<usize>),
    Window,
    /// A window chosen from a list of them, by its number.
    Choose,
    /// The window whose title has this in it.
    Titled(String),
    Region(Rect),
    /// A picture taken before, taken again as it was set out: the last,
    /// or one by how long ago (`@2`) or by its name.
    Again(Option<String>),
    /// The last picture itself, with what was drawn on it, to go on with.
    Reopen,
}

pub struct Options {
    pub take: Take,
    pub edit: bool,
    pub delay: f64,
    pub copy: bool,
    pub save: bool,
    pub output: Option<PathBuf>,
    pub dir: PathBuf,
    /// The folders Save can save to: `dir`, and any others the config
    /// file names, in the order it names them.
    pub places: Vec<PathBuf>,
    /// Which of them Save saves to, as it was last left.
    pub save_to: Option<PathBuf>,
    /// How round the corners are of a picture taken at once: what the
    /// config file and the command line say, and not what was last in
    /// hand in the editor.
    pub plain_corners: f32,
    pub filename: String,
    /// What kind of file a picture is saved as, where the name given
    /// with `-o` does not say; and how good a JPEG is, of 100.
    pub format: Format,
    pub quality: u8,
    /// Whether the mouse pointer is in the picture.
    pub pointer: bool,
    /// Whether the picture is left on the screen once taken.
    pub pin: bool,
    /// A picture in a file to leave on the screen, and where; and
    /// whether the file is to be done away with afterwards.
    pub show: Option<PathBuf>,
    pub at: Option<(i32, i32)>,
    pub forget: bool,
    /// A PNG in a file to put on the clipboard.
    pub clip: Option<PathBuf>,
    /// A recording made of the area, not a picture taken of it: so
    /// many frames a second, as a file of some kind, for so long (or
    /// until it is stopped, at 0), with the mouse pointer in it or not,
    /// and the keys pressed said along its bottom or not.
    pub record: bool,
    pub fps: u32,
    pub video: Video,
    pub duration: f64,
    pub record_pointer: bool,
    pub keys: bool,
    /// An icon in the panel's tray, to take pictures from; and the
    /// list that icon opens, by itself, where the pointer is.
    pub tray: bool,
    pub menu: bool,
    /// Each thing hotshot does tried on this machine, and said to have
    /// worked or not.
    pub check: bool,
    /// The canvas, in a window of its own, rather than a picture taken;
    /// and the files to open on it.
    pub canvas: bool,
    pub files: Vec<PathBuf>,
    /// The pictures taken before listed, to take one again.
    pub history: bool,
    pub settings: Settings,
    pub list_monitors: bool,
    pub benchmark: bool,
    pub debug: bool,
    /// Whether what is in hand at the end is what is in hand to begin
    /// with next time.
    pub remember: bool,
}

pub enum Parsed {
    Run(Box<Options>),
    Help,
    Version,
    Config,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// A path as typed, with a leading `~` taken for the home directory.
fn path(text: &str) -> PathBuf {
    match text.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None if text == "~" => home(),
        None => PathBuf::from(text),
    }
}

fn defaults() -> Options {
    Options {
        take: Take::Pick,
        edit: false,
        delay: 0.0,
        copy: false,
        save: false,
        output: None,
        dir: home().join("Pictures/Screenshots"),
        places: Vec::new(),
        save_to: None,
        plain_corners: 0.0,
        filename: "screenshot-%Y-%m-%d-%H%M%S".into(),
        format: Format::Png,
        quality: 90,
        pointer: false,
        pin: false,
        show: None,
        at: None,
        forget: false,
        clip: None,
        record: false,
        fps: 60,
        video: Video::Mp4,
        duration: 0.0,
        record_pointer: true,
        keys: false,
        tray: false,
        menu: false,
        check: false,
        canvas: false,
        files: Vec::new(),
        history: false,
        settings: Settings::default(),
        list_monitors: false,
        benchmark: false,
        debug: std::env::var_os("HOTSHOT_DEBUG").is_some_and(|value| !value.is_empty() && value != "0"),
        remember: true,
    }
}

/// The long name of each short flag.
fn short_flag(flag: char) -> Option<&'static str> {
    Some(match flag {
        'f' => "full",
        'm' => "monitor",
        'w' => "window",
        'i' => "choose",
        'r' => "reopen",
        't' => "title",
        'g' => "region",
        'a' => "again",
        'e' => "edit",
        'D' => "delay",
        'p' => "pin",
        'R' => "record",
        'k' => "keys",
        'c' => "copy",
        's' => "save",
        'o' => "output",
        'd' => "dir",
        'h' => "help",
        'v' => "version",
        _ => return None,
    })
}

/// Two whole numbers, neither of them nothing, with a mark between.
fn pair(text: &str, mark: char) -> Option<(i32, i32)> {
    let (first, second) = text.split_once(mark)?;
    let number = |text: &str| text.trim().parse::<i32>().ok().filter(|number| (1..=100_000).contains(number));
    Some((number(first)?, number(second)?))
}

fn takes_value(name: &str) -> bool {
    ["show", "at", "clip", "fps", "record-format", "duration", "format", "quality", "aspect", "fixed", "monitor", "region", "delay", "output", "dir", "filename", "accent", "color", "fill-color", "window-frame", "size", "corner-radius", "rounded", "border-radius", "frame-radius", "opacity", "tool", "arrow-heads", "zoom", "dim", "enter", "windows", "font", "title", "place", "save-to", "backdrop", "padding", "photo"].contains(&name)
}

/// Sets one option, named the way the config file spells it. A flag
/// given no value is switched on.
fn apply(options: &mut Options, name: &str, value: &mut dyn FnMut() -> Result<String, String>) -> Result<(), String> {
    let number = |text: String, what: &str| text.parse::<f64>().ok().filter(|value| value.is_finite() && *value >= 0.0).ok_or(format!("invalid {what} '{text}'"));
    match name {
        "full" => options.take = Take::Full,
        "window" => options.take = Take::Window,
        "choose" => options.take = Take::Choose,
        "title" => options.take = Take::Titled(value()?),
        "monitor" => {
            let text = value()?;
            options.take = Take::Monitor(match text.as_str() {
                "pointer" => None,
                _ => Some(text.parse().map_err(|_| format!("invalid monitor '{text}', expected a number or 'pointer'"))?),
            });
        }
        "region" => options.take = Take::Region(parse_region(&value()?)?),
        "again" | "last" | "repeat" => options.take = Take::Again(None),
        "history" => options.history = true,
        "canvas" => options.canvas = true,
        "reopen" => (options.take, options.edit) = (Take::Reopen, true),
        "edit" => options.edit = true,
        "delay" => options.delay = number(value()?, "delay")?,
        "copy" => options.copy = true,
        "save" => options.save = true,
        "output" => options.output = Some(path(&value()?)),
        "dir" => options.dir = path(&value()?),
        "format" => {
            let text = value()?;
            options.format = Format::named(&text).ok_or(format!("invalid format '{text}', expected png, jpg or webp"))?;
        }
        "quality" => {
            let text = value()?;
            options.quality = text.parse::<u8>().ok().filter(|quality| (1..=100).contains(quality)).ok_or(format!("invalid quality '{text}', expected 1 to 100"))?;
        }
        "pointer" => options.pointer = true,
        "no-pointer" => options.pointer = false,
        "pin" => options.pin = true,
        // A picture from a file, left on the screen as one just taken
        // is: where `at` says, or in the middle of the screen. `forget`
        // is for one hotshot wrote itself, to be done away with after.
        "show" => options.show = Some(path(&value()?)),
        "at" => {
            let text = value()?;
            let place = text.split_once(',').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
            options.at = Some(place.ok_or(format!("invalid place '{text}', expected X,Y"))?);
        }
        "forget" => options.forget = true,
        // What is in a file, put on the clipboard: for a picture left
        // on the screen to be copied from.
        "clip" => options.clip = Some(path(&value()?)),
        "record" => options.record = true,
        "fps" => {
            let text = value()?;
            options.fps = text.parse::<u32>().ok().filter(|fps| (1..=120).contains(fps)).ok_or(format!("invalid fps '{text}', expected 1 to 120"))?;
        }
        "record-format" => {
            let text = value()?;
            options.video = Video::named(&text).ok_or(format!("invalid recording format '{text}', expected mp4, gif, apng or webp"))?;
        }
        "duration" => options.duration = number(value()?, "duration")?,
        "record-pointer" => options.record_pointer = true,
        "no-record-pointer" => options.record_pointer = false,
        "keys" => options.keys = true,
        "no-keys" => options.keys = false,
        "tray" => options.tray = true,
        "menu" => options.menu = true,
        "check" => options.check = true,
        // The shape an area is kept to as it is dragged out: so wide to
        // so tall, as 16:9; or so many pixels each way, as 1280x720.
        "aspect" => {
            let text = value()?;
            options.settings.aspect = match text.as_str() {
                "free" | "none" => None,
                _ => Some(pair(&text, ':').ok_or(format!("invalid aspect '{text}', expected something like 16:9, or free"))?),
            };
        }
        "fixed" => {
            let text = value()?;
            options.settings.fixed = match text.as_str() {
                "free" | "none" => None,
                _ => Some(pair(&text, 'x').ok_or(format!("invalid size '{text}', expected something like 1280x720, or free"))?),
            };
        }
        // Another folder Save can save to: as many as are named.
        "place" => options.places.push(path(&value()?)),
        // The one of them it saves to, by its path: looked for once
        // they have all been named.
        "save-to" => options.save_to = Some(path(&value()?)),
        "backdrop" => {
            let text = value()?;
            // A photograph: the user's own, which --photo names, or
            // one nobody chose, a new one each time.
            options.settings.fetch = text.eq_ignore_ascii_case("random");
            options.settings.frame.backdrop = match frame::named(&text) {
                Some(which) => Backdrop::Gradient(which),
                None if text.eq_ignore_ascii_case("none") => Backdrop::None,
                None if text.eq_ignore_ascii_case("photo") || text.eq_ignore_ascii_case("random") => Backdrop::Photo,
                None => {
                    let names: Vec<String> = BACKDROPS.iter().map(|(name, _)| name.to_lowercase()).collect();
                    return Err(format!("invalid backdrop '{text}', expected none, photo, random or one of: {}", names.join(", ")));
                }
            };
        }
        "padding" => {
            let text = value()?;
            // A step of it, 0 to 3, or past that so many pixels: one
            // for every side, or beside and then above and below, as
            // 64x40.
            let one = |text: &str| {
                let asked = text.trim().parse::<i32>().ok().filter(|pad| (0..=800).contains(pad))?;
                Some(PADS.get(asked as usize).copied().unwrap_or(asked))
            };
            let wrong = || format!("invalid padding '{text}', expected 0 to 3, a number of pixels, or two of them as 64x40");
            options.settings.frame.pad = match text.split_once(['x', ',']) {
                Some((across, down)) => (one(across).ok_or_else(wrong)?, one(down).ok_or_else(wrong)?),
                None => {
                    let room = one(&text).ok_or_else(wrong)?;
                    (room, room)
                }
            };
        }
        "frame-radius" => options.settings.frame.radius = number(value()?, "frame radius")?.round() as i32,
        "opacity" => {
            let text = value()?;
            let percent = text.trim_end_matches('%').parse::<u32>().ok().filter(|percent| (1..=100).contains(percent)).ok_or(format!("invalid opacity '{text}', expected 1 to 100"))?;
            options.settings.opacity = (percent * 256).div_ceil(100);
        }
        "dashed" => options.settings.dashed = true,
        "no-dashed" => options.settings.dashed = false,
        "tool" => {
            let text = value()?;
            options.settings.tool = crate::shapes::tool_named(&text).ok_or(format!("invalid tool '{text}', expected move, arrow, line, rect, oval, pen, marker, text, counter, blur, spotlight or zoom"))?;
        }
        "arrow-heads" => {
            let text = value()?;
            options.settings.heads = match text.to_ascii_lowercase().as_str() {
                "end" | "one" => [false, true],
                "start" => [true, false],
                "both" => [true, true],
                _ => return Err(format!("invalid arrow heads '{text}', expected end, start or both")),
            };
        }
        "window-frame" => {
            let text = value()?;
            let wanted = text.to_ascii_lowercase();
            options.settings.frame.window = frame::WINDOWS
                .iter()
                .find(|(window, name)| name.to_ascii_lowercase().starts_with(&wanted) || (wanted == "none" && *window == frame::Window::None))
                .map(|(window, _)| *window)
                .ok_or(format!("invalid window '{text}', expected browser, mac, terminal, desktop or none"))?;
        }
        "zoom" => {
            let text = value()?;
            options.settings.square = match text.to_ascii_lowercase().as_str() {
                "round" => false,
                "square" => true,
                _ => return Err(format!("invalid zoom '{text}', expected round or square")),
            };
        }
        "shadow" => options.settings.frame.shadow = Shadow::named(frame::SHADOW).expect("the usual shadow is one of them"),
        "no-shadow" => options.settings.frame.shadow = Shadow::NONE,
        "shadow-is" => options.settings.frame.shadow = Shadow::parse(&value()?)?,
        "photo" => options.settings.photo = Some(path(&value()?)),
        "filename" => options.filename = value()?,
        "accent" => options.settings.accent = parse_accent(&value()?)?,
        "color" => options.settings.color = parse_ink(&value()?)?,
        "size" => {
            let text = value()?;
            let size = text.parse::<usize>().ok().filter(|size| (1..=4).contains(size)).ok_or(format!("invalid size '{text}', expected 1 to 4"))?;
            options.settings.size = size - 1;
        }
        "corner-radius" => options.settings.radius = number(value()?, "corner radius")? as f32,
        "rounded" | "border-radius" => options.settings.corners = number(value()?, "rounding")? as f32,
        "dim" => options.settings.dim = (number(value()?, "dim")? as u32).min(100),
        "magnifier" => options.settings.magnifier = true,
        "no-magnifier" => options.settings.magnifier = false,
        "remember" => options.remember = true,
        "no-remember" => options.remember = false,
        "filled" => options.settings.filled = true,
        "no-filled" => options.settings.filled = false,
        "fill-color" => options.settings.fill = Some(parse_ink(&value()?)?),
        "border" => options.settings.border = Some(true),
        "no-border" => options.settings.border = Some(false),
        "font" => {
            let text = value()?;
            let wanted = text.to_lowercase().replace(' ', "-");
            options.settings.face = FACE_NAMES.iter().position(|names| names.contains(&wanted.as_str())).ok_or(format!("invalid font '{text}', expected geist, instrument-serif, architects-daughter or fira-code (or sans, serif, hand, mono)"))?;
        }
        "windows" => {
            let text = value()?;
            options.settings.windows = match text.as_str() {
                "outline" => Windows::Outline,
                "bright" => Windows::Bright,
                "off" => Windows::Off,
                _ => return Err(format!("invalid windows '{text}', expected outline, bright or off")),
            };
        }
        "enter" => {
            let text = value()?;
            options.settings.enter = match text.as_str() {
                "copy" => Finish { copy: true, ..Finish::NONE },
                "save" => Finish { save: true, ..Finish::NONE },
                "both" => Finish { copy: true, save: true, ..Finish::NONE },
                "pin" => Finish { pin: true, ..Finish::NONE },
                _ => return Err(format!("invalid enter '{text}', expected copy, save, both or pin")),
            };
        }
        "monitors" => options.list_monitors = true,
        "benchmark" => options.benchmark = true,
        "debug" => options.debug = true,
        _ => return Err(format!("unknown option {name}")),
    }
    Ok(())
}

/// The names the faces go by in the config file and on the command
/// line, each with a shorter one for the kind of face it is.
const FACE_NAMES: [[&str; 2]; 5] = [["geist", "sans"], ["instrument-serif", "serif"], ["architects-daughter", "hand"], ["fira-code", "mono"], ["anton", "impact"]];

/// Keeps what is in hand, to begin with next time: written the way the
/// config file is, so that it is read the same way.
pub fn remember_tools(settings: &Settings, save_to: Option<&std::path::Path>) {
    let Some(file) = crate::output::state_file("tools") else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Only what is drawn with is kept. What changes the picture itself,
    // its rounded corners and the frame it is set in, is not: a plain
    // picture is what is taken unless it is asked otherwise, this time,
    // and one that came out framed because the last one was is a
    // surprise nobody wants.
    let mut text = format!(
        "color = \"{:06X}\"\nsize = {}\ncorner-radius = {}\nrounded = {}\nfilled = {}\nfont = \"{}\"\n",
        settings.color,
        settings.size + 1,
        settings.radius,
        settings.corners,
        settings.filled,
        FACE_NAMES[settings.face.min(FACE_NAMES.len() - 1)][0]
    );
    if let Some(fill) = settings.fill {
        text += &format!("fill-color = \"{fill:06X}\"\n");
    }
    if let Some(border) = settings.border {
        text += &format!("border = {border}\n");
    }
    if let Some(folder) = save_to {
        text += &format!("save-to = \"{}\"\n", folder.display());
    }
    let _ = std::fs::write(file, text);
}

/// Where the config file is: under `$XDG_CONFIG_HOME`, or `~/.config`.
pub fn config_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    base.join("hotshot/config.toml")
}

/// Reads settings from the text of a config file: a line each, as
/// `name = value`, the value in quotes or not. Something switched off
/// is `name = false`.
fn read_config(options: &mut Options, text: &str) -> Result<(), String> {
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let wrong = |error: String| format!("line {}: {error}", number + 1);
        let (name, value) = line.split_once('=').ok_or_else(|| wrong("expected name = value".into()))?;
        let (name, value) = (name.trim(), value.trim());
        // What follows a # is a remark, unless it is inside the quotes.
        let value = match value.strip_prefix('"') {
            Some(quoted) => quoted.split_once('"').map(|(inside, _)| inside).ok_or_else(|| wrong("a quote is not closed".into()))?,
            None => value.split('#').next().unwrap_or("").trim(),
        };
        let name = match (takes_value(name), value) {
            (true, _) => name.to_owned(),
            (false, "true") => name.to_owned(),
            (false, "false") if ["magnifier", "remember", "filled", "border", "shadow", "pointer", "record-pointer", "keys", "dashed"].contains(&name) => format!("no-{name}"),
            (false, "false") => continue,
            // A shadow is on or off, or which: Tailwind's name, or CSS.
            (false, _) if name == "shadow" => "shadow-is".to_owned(),
            (false, _) => return Err(wrong(format!("{name} is either true or false"))),
        };
        apply(options, &name, &mut || Ok(value.to_owned())).map_err(wrong)?;
    }
    Ok(())
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Parsed, String> {
    let mut options = defaults();
    let file = config_file();
    if let Ok(text) = std::fs::read_to_string(&file) {
        read_config(&mut options, &text).map_err(|error| format!("{}: {error}", file.display()))?;
    }
    // What was in hand last time comes after the config file, which
    // says how things are to begin with, and before the command line,
    // which says how they are to be now. A file that cannot be read,
    // or made sense of, is as good as none.
    options.plain_corners = options.settings.corners;
    if options.remember {
        if let Some(text) = crate::output::state_file("tools").and_then(|file| std::fs::read_to_string(file).ok()) {
            let mut kept = defaults();
            if read_config(&mut kept, &text).is_ok() {
                options.save_to = kept.save_to.take().or(options.save_to.take());
                let (now, kept) = (&mut options.settings, kept.settings);
                (now.color, now.size, now.radius, now.filled, now.face) = (kept.color, kept.size, kept.radius, kept.filled, kept.face);
                (now.fill, now.border) = (kept.fill.or(now.fill), kept.border.or(now.border));
                // How round the picture's corners were is kept too, for
                // the next one that is opened to be drawn on; a picture
                // taken at once has the corners the config file and the
                // command line say, which is usually square.
                now.corners = kept.corners;
            }
        }
    }
    let kept_corners = options.settings.corners;

    let mut args = args.into_iter().peekable();
    while let Some(arg) = args.next() {
        let text = arg.into_string().map_err(|_| "an argument is not text".to_string())?;
        // Each entry is a flag name and any value attached to it.
        let mut flags: Vec<(String, Option<String>)> = Vec::new();
        if let Some(long) = text.strip_prefix("--") {
            match long.split_once('=') {
                Some((name, value)) => flags.push((name.into(), Some(value.into()))),
                None => flags.push((long.into(), None)),
            }
        } else if let Some(cluster) = text.strip_prefix('-').filter(|cluster| !cluster.is_empty()) {
            for (at, flag) in cluster.char_indices() {
                let name = short_flag(flag).ok_or_else(|| format!("unknown option -{flag}"))?;
                let rest = &cluster[at + flag.len_utf8()..];
                if takes_value(name) && !rest.is_empty() {
                    flags.push((name.into(), Some(rest.into())));
                    break;
                }
                flags.push((name.into(), None));
            }
        } else {
            // Files, which the canvas opens.
            options.files.push(path(&text));
            continue;
        }

        for (name, attached) in flags {
            // Which picture to take again may be said, and need not be:
            // what comes after is that only if it is like one, `@2` or
            // a name of four letters, and is not another option.
            if matches!(name.as_str(), "again" | "last" | "repeat") {
                let like = |word: &str| word.starts_with('@') || (word.len() == 4 && word.chars().all(|letter| letter.is_ascii_alphanumeric()));
                let which = attached.clone().or_else(|| match args.peek().and_then(|next| next.to_str()) {
                    Some(next) if like(next) => args.next().and_then(|next| next.into_string().ok()),
                    _ => None,
                });
                options.take = Take::Again(which);
                continue;
            }
            // So may which shadow: what comes after is that if it is one
            // of Tailwind's names, or CSS, which begins with a number.
            // CSS that begins with a minus is said with `--shadow=`.
            if name == "shadow" {
                let like = |word: &str| Shadow::named(word).is_some() || word.eq_ignore_ascii_case("none") || word.starts_with(|letter: char| letter.is_ascii_digit());
                let which = attached.clone().or_else(|| match args.peek().and_then(|next| next.to_str()) {
                    Some(next) if like(next) => args.next().and_then(|next| next.into_string().ok()),
                    _ => None,
                });
                match which {
                    Some(which) => apply(&mut options, "shadow-is", &mut || Ok(which.clone()))?,
                    None => apply(&mut options, "shadow", &mut || Err(String::new()))?,
                }
                continue;
            }
            let mut value = || -> Result<String, String> {
                let next = attached.clone().or_else(|| args.next().and_then(|arg| arg.into_string().ok()));
                next.ok_or_else(|| format!("option --{name} needs a value"))
            };
            match name.as_str() {
                "help" => return Ok(Parsed::Help),
                "version" => return Ok(Parsed::Version),
                "default-config" => return Ok(Parsed::Config),
                name => apply(&mut options, name, &mut value).map_err(|error| error.replace("unknown option ", "unknown option --"))?,
            }
        }
    }
    if options.settings.corners != kept_corners {
        options.plain_corners = options.settings.corners;
    }
    // A backdrop with no room round the picture is not seen: it has
    // the room the box of backdrops gives it, unless some is said.
    if options.settings.frame.backdrop != Backdrop::None && options.settings.frame.pad == (0, 0) {
        options.settings.frame.pad = (PADS[2], PADS[2]);
    }
    // The folders Save can save to: the usual one first, then the
    // others; and of them, the one it was last pointed at.
    options.places.insert(0, options.dir.clone());
    options.places.dedup();
    options.settings.places = options.places.iter().map(|place| place.file_name().map_or_else(|| place.display().to_string(), |name| name.to_string_lossy().into_owned())).collect();
    options.settings.place = options.save_to.as_ref().and_then(|wanted| options.places.iter().position(|place| place == wanted)).unwrap_or(0);
    // --edit with nothing named to take, or with files, is the canvas:
    // a picture to be made, not one to be taken.
    if options.edit && !options.record && (options.take == Take::Pick || !options.files.is_empty()) {
        options.canvas = true;
    }
    if !options.canvas && !options.files.is_empty() {
        return Err(format!("unexpected argument '{}'", options.files[0].display()));
    }
    Ok(Parsed::Run(Box::new(options)))
}

/// A rectangle as `X,Y,W,H` or as `WxH+X+Y`.
fn parse_region(text: &str) -> Result<Rect, String> {
    let wrong = || format!("invalid region '{text}', expected X,Y,W,H or WxH+X+Y");
    let numbers = |parts: Vec<&str>| -> Option<Vec<i32>> { parts.iter().map(|part| part.trim().parse().ok()).collect() };
    let rect = if text.contains('x') {
        let parts = numbers(text.split(['x', '+']).collect()).ok_or_else(wrong)?;
        match parts[..] {
            [w, h, x, y] => Rect::new(x, y, w, h),
            _ => return Err(wrong()),
        }
    } else {
        let parts = numbers(text.split(',').collect()).ok_or_else(wrong)?;
        match parts[..] {
            [x, y, w, h] => Rect::new(x, y, w, h),
            _ => return Err(wrong()),
        }
    };
    if rect.is_empty() { Err(wrong()) } else { Ok(rect) }
}

fn parse_color(text: &str) -> Option<u32> {
    let hex = text.trim_start_matches('#');
    u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() == 6)
}

/// A color to draw in: one of the palette's by name, or `RRGGBB`.
fn parse_ink(text: &str) -> Result<u32, String> {
    let named = PALETTE.iter().find(|(_, name)| *name == text.to_lowercase()).map(|(color, _)| *color);
    named.or_else(|| parse_color(text)).ok_or(format!("invalid color '{text}', expected a name such as red, or RRGGBB"))
}

/// An accent color: one of the Radix colors by name, at the step meant
/// for solid backgrounds, or any color as `RRGGBB`. The same as jog's.
fn parse_accent(text: &str) -> Result<u32, String> {
    let named = match text.to_lowercase().as_str() {
        "tomato" => 0xE54D2E,
        "red" => 0xE5484D,
        "ruby" => 0xE54666,
        "crimson" => 0xE93D82,
        "pink" => 0xD6409F,
        "plum" => 0xAB4ABA,
        "purple" => 0x8E4EC6,
        "violet" => 0x6E56CF,
        "iris" => 0x5B5BD6,
        "indigo" => 0x3E63DD,
        "blue" => 0x0090FF,
        "cyan" => 0x00A2C7,
        "teal" => 0x12A594,
        "jade" => 0x29A383,
        "green" => 0x30A46C,
        "grass" => 0x46A758,
        "orange" => 0xF76B15,
        "brown" => 0xAD7F58,
        "bronze" => 0xA18072,
        "gold" => 0x978365,
        _ => return parse_color(text).ok_or(format!("invalid accent '{text}', expected a Radix color name such as indigo, or RRGGBB")),
    };
    Ok(named)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Options, String> {
        match parse(args.iter().map(OsString::from))? {
            Parsed::Run(options) => Ok(*options),
            _ => Err("not a run".into()),
        }
    }

    #[test]
    fn flags_cluster_and_take_values_either_way() {
        // How round the picture's corners are, by either name: in the
        // editor to begin with, and for a picture taken at once.
        for name in ["--border-radius", "--rounded"] {
            let rounded = run(&[name, "5"]).unwrap();
            assert_eq!((rounded.settings.corners, rounded.plain_corners), (5.0, 5.0));
        }
        let options = run(&["-fc", "-o", "out.png", "--delay=2", "-D0.5"]).unwrap();
        assert_eq!(options.take, Take::Full);
        assert!(options.copy && !options.save);
        assert_eq!(options.output, Some(PathBuf::from("out.png")));
        assert_eq!(options.delay, 0.5);
        assert!(run(&["--nonsense"]).err().unwrap().contains("unknown option --nonsense"));
        assert!(run(&["-o"]).err().unwrap().contains("needs a value"));
        assert!(run(&["stray"]).is_err());
    }

    #[test]
    fn the_kind_of_file_the_pointer_and_the_shape_of_the_area_are_said() {
        let options = run(&["--format", "JPG", "--quality=70", "--pointer", "--aspect", "16:9", "--fixed", "1280x720"]).unwrap();
        assert_eq!((options.format, options.quality, options.pointer), (Format::Jpeg, 70, true));
        assert_eq!((options.settings.aspect, options.settings.fixed), (Some((16, 9)), Some((1280, 720))));
        let plain = run(&["--aspect", "free"]).unwrap();
        assert_eq!((plain.format, plain.quality, plain.pointer, plain.settings.aspect), (Format::Png, 90, false, None));
        for wrong in [&["--format", "gif"][..], &["--quality", "0"], &["--aspect", "16x9"], &["--fixed", "0x100"]] {
            assert!(run(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn pinning_recording_and_the_tray_are_asked_for() {
        let options = run(&["-pR", "--keys", "--fps=30", "--record-format", "GIF", "--duration", "2.5", "--no-record-pointer"]).unwrap();
        assert!(options.pin && options.record && options.keys && !options.record_pointer);
        assert_eq!((options.fps, options.video, options.duration), (30, Video::Gif, 2.5));
        let shown = run(&["--show", "pic.png", "--at", "40,-8", "--forget"]).unwrap();
        assert_eq!((shown.show, shown.at, shown.forget), (Some(PathBuf::from("pic.png")), Some((40, -8)), true));
        let plain = run(&[]).unwrap();
        assert_eq!((plain.fps, plain.video, plain.record_pointer, plain.keys, plain.tray, plain.menu), (60, Video::Mp4, true, false, false, false));
        assert!(run(&["--tray"]).unwrap().tray && run(&["--menu"]).unwrap().menu);
        for wrong in [&["--fps", "0"][..], &["--record-format", "avi"], &["--at", "here"]] {
            assert!(run(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn how_the_picture_is_set_out_and_drawn_on_is_said_on_the_command_line() {
        let options = run(&["--backdrop", "dusk", "--padding", "80x30", "--frame-radius", "24", "--shadow", "--opacity", "50", "--dashed", "--arrow-heads", "both", "--zoom", "square", "--tool", "arrow"]).unwrap();
        let (settings, frame) = (&options.settings, options.settings.frame);
        assert_eq!((frame.backdrop, frame.pad, frame.radius, frame.shadow), (Backdrop::Gradient(frame::named("dusk").unwrap()), (80, 30), 24, Shadow::named(frame::SHADOW).unwrap()));
        assert_eq!((settings.opacity, settings.dashed, settings.heads, settings.square, settings.tool), (128, true, [true, true], true, crate::shapes::Tool::Arrow));
        // Padding by its steps, or the same all round.
        assert_eq!((run(&["--padding", "2"]).unwrap().settings.frame.pad, run(&["--padding", "40"]).unwrap().settings.frame.pad), ((PADS[2], PADS[2]), (40, 40)));
        // A backdrop said with no room round it is given some, to be seen.
        assert_eq!(run(&["--backdrop", "ink"]).unwrap().settings.frame.pad, (PADS[2], PADS[2]));
        let random = run(&["--backdrop", "random"]).unwrap();
        assert!(random.settings.fetch && random.settings.frame.backdrop == Backdrop::Photo);
        let own = run(&["--backdrop", "photo", "--photo", "/tmp/x.jpg"]).unwrap();
        assert!(!own.settings.fetch && own.settings.frame.backdrop == Backdrop::Photo);
        // And a shadow on nothing at all.
        let shadow = run(&["--shadow"]).unwrap().settings.frame;
        assert!(shadow.backdrop == Backdrop::None && shadow.shadow.on() && shadow.is_clear());
        // Which shadow, by Tailwind's name or as CSS, said either way;
        // and `--shadow` before another option is the usual one.
        let lg = Shadow::named("lg").unwrap();
        assert_eq!(run(&["--shadow", "lg"]).unwrap().settings.frame.shadow, lg);
        assert_eq!(run(&["--shadow=shadow-lg"]).unwrap().settings.frame.shadow, lg);
        assert_eq!(run(&["--shadow", "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)"]).unwrap().settings.frame.shadow, lg);
        assert_eq!(run(&["--shadow=-4px 8px 0 #000"]).unwrap().settings.frame.shadow.layers()[0].x, -4);
        let before = run(&["--shadow", "-c"]).unwrap();
        assert!(before.copy && before.settings.frame.shadow == Shadow::named(frame::SHADOW).unwrap());
        assert!(!run(&["--shadow", "lg", "--no-shadow"]).unwrap().settings.frame.shadow.on());
        assert!(run(&["--shadow=0 0 1px blue"]).is_err());
        // And in the config file: on, off, a name, or CSS in quotes.
        for (said, wanted) in [("true", Shadow::named(frame::SHADOW).unwrap()), ("false", Shadow::NONE), ("\"md\"", Shadow::named("md").unwrap()), ("\"0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)\"", Shadow::named("md").unwrap())] {
            let mut options = defaults();
            read_config(&mut options, &format!("shadow = {said}")).unwrap();
            assert_eq!(options.settings.frame.shadow, wanted, "{said}");
        }
        for wrong in [&["--padding", "80x"][..], &["--opacity", "0"], &["--tool", "hammer"], &["--zoom", "oval"], &["--arrow-heads", "many"], &["--backdrop", "plaid"]] {
            assert!(run(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn regions_are_read_both_ways() {
        assert_eq!(run(&["-g", "10,20,300,400"]).unwrap().take, Take::Region(Rect::new(10, 20, 300, 400)));
        assert_eq!(run(&["--region", "300x400+10+20"]).unwrap().take, Take::Region(Rect::new(10, 20, 300, 400)));
        assert!(run(&["-g", "10,20,0,400"]).is_err());
        for again in ["-a", "--again", "--last", "--repeat"] {
            assert_eq!(run(&[again]).unwrap().take, Take::Again(None));
        }
        // Which, said or not: what comes after is taken for it only if
        // it is like one.
        assert_eq!(run(&["-a", "@2", "-c"]).unwrap().take, Take::Again(Some("@2".into())));
        assert_eq!(run(&["--again", "k3fq"]).unwrap().take, Take::Again(Some("k3fq".into())));
        assert_eq!(run(&["--again=@5"]).unwrap().take, Take::Again(Some("@5".into())));
        let plain = run(&["-ac"]).unwrap();
        assert!(plain.take == Take::Again(None) && plain.copy);
        assert!(run(&["-a", "toolong"]).is_err());
        assert!(run(&["--history"]).unwrap().history);
        assert_eq!(run(&["-m", "pointer"]).unwrap().take, Take::Monitor(None));
        assert_eq!(run(&["-m1"]).unwrap().take, Take::Monitor(Some(1)));
        // --edit alone, or with files, is the canvas; with an area, it is not.
        assert!(run(&["--edit"]).unwrap().canvas && run(&["-e", "a.png"]).unwrap().canvas);
        assert!(!run(&["-f", "-e"]).unwrap().canvas && run(&["a.png"]).is_err());
    }

    #[test]
    fn a_config_file_sets_what_the_flags_do() {
        let mut options = defaults();
        assert!(!options.settings.magnifier);
        let text = "# a remark\ndir = \"/tmp/shots\"   # and another\nenter = \"both\"\nmagnifier = true\nsize = 3\naccent = \"teal\"\ncolor = \"00ff00\"\nrounded = 16\n";
        read_config(&mut options, text).unwrap();
        assert!(options.settings.magnifier && options.settings.corners == 16.0);
        read_config(&mut options, "magnifier = false").unwrap();
        assert_eq!(options.dir, PathBuf::from("/tmp/shots"));
        assert_eq!(options.settings.enter, Finish { copy: true, save: true, ..Finish::NONE });
        assert!(!options.settings.magnifier);
        assert_eq!((options.settings.size, options.settings.accent, options.settings.color), (2, 0x12A594, 0x00FF00));
        assert_eq!(options.settings.windows, Windows::Outline);
        read_config(&mut options, "window-frame = \"terminal\"").unwrap();
        assert_eq!(options.settings.frame.window, frame::Window::Terminal);
        assert!(read_config(&mut options, "window-frame = \"tv\"").is_err());
        read_config(&mut options, "windows = \"bright\"").unwrap();
        assert_eq!(options.settings.windows, Windows::Bright);
        assert!(read_config(&mut options, "windows = \"dim\"").is_err());
        read_config(&mut options, "color = \"jade\"").unwrap();
        assert_eq!(options.settings.color, 0x29A383);
        assert_eq!((options.settings.fill, options.settings.border), (None, None));
        read_config(&mut options, "fill-color = \"yellow\"\nborder = false").unwrap();
        assert_eq!((options.settings.fill, options.settings.border), (Some(0xFFE629), Some(false)));
        assert!(read_config(&mut options, "size = 9").unwrap_err().starts_with("line 1"));
        assert!(read_config(&mut options, "nothing").is_err());
    }
}
