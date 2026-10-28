//! Everything between the screen being frozen and the picture being
//! taken: the area dragged out, what is drawn on it, and the bar of
//! tools. It knows nothing of the display: it is told of the pointer and
//! the keys, and paints the screen into a buffer it is handed. That is
//! what lets it be tried, and timed, without a display at all.

use std::ops::DerefMut;

use crate::canvas::{self, Canvas, Rect};
use crate::shapes::{self, Shape, Tool};
use crate::spacing;
use crate::frame::{self, Backdrop, Frame, Photo, Shadow, BACKDROPS, FIRST, PADS};
use crate::text::{icons, Text, FACES};

// jog's colors, so that the two look like one family.
const TEXT_COLOR: u32 = 0xEEEEEE;
const DIM_TEXT_COLOR: u32 = 0x9A9A9A;
const MENU_COLOR: u32 = 0x1E1E1E;
const MENU_EDGE_COLOR: u32 = 0x555555;

/// The colors to draw in: Radix's, at the step meant for solid
/// backgrounds, as jog's accents are.
pub const PALETTE: [(u32, &str); 24] = [
    // The reds round to the violets,
    (0xE54D2E, "tomato"),
    (0xE5484D, "red"),
    (0xE54666, "ruby"),
    (0xE93D82, "crimson"),
    (0xD6409F, "pink"),
    (0xAB4ABA, "plum"),
    (0x8E4EC6, "purple"),
    (0x6E56CF, "violet"),
    // the blues to the greens,
    (0x5B5BD6, "iris"),
    (0x3E63DD, "indigo"),
    (0x0090FF, "blue"),
    (0x00A2C7, "cyan"),
    (0x12A594, "teal"),
    (0x29A383, "jade"),
    (0x30A46C, "green"),
    (0x46A758, "grass"),
    // the bright ones, and the ones that are no color.
    (0xBDEE63, "lime"),
    (0xFFE629, "yellow"),
    (0xFFC53D, "amber"),
    (0xF76B15, "orange"),
    (0xAD7F58, "brown"),
    (0x8D8D8D, "gray"),
    (0xFFFFFF, "white"),
    (0x161616, "black"),
];
/// The one drawn in unless another is asked for.
const RED: u32 = 0xE5484D;
/// How opaque what is drawn can be, out of 256: a quarter, a half,
/// three quarters and wholly.
const OPACITIES: [u32; 4] = [64, 128, 192, 256];

/// The ways text is set, in the order the bar over it has them, with
/// the letter each is shown by.
const STYLES: [(u8, &str); 5] = [(shapes::BOLD, "B"), (shapes::ITALIC, "I"), (shapes::HIGHLIGHT, "H"), (shapes::BOXED, "A"), (shapes::BULLETS, "\u{2022}")];

/// How many of them stand in a row of the box they are picked from.
const PALETTE_COLUMNS: i32 = 8;

/// How round the corners of the picture itself can be made, in pixels
/// of a screen 1080 high: the button for it goes from each to the next.
const CORNERS: [f32; 4] = [0.0, 8.0, 16.0, 28.0];

/// The widths of line there are to choose from, and the sizes of text
/// that go with them, in pixels of a screen 1080 high.
const STROKES: [f32; 4] = [2.0, 3.5, 6.0, 10.0];
const FONT_SIZES: [f32; 4] = [14.0, 18.0, 26.0, 40.0];

/// How far the pointer has to go with the button down to count as a drag.
const DRAG_AFTER: i32 = 3;
/// How soon a second click must follow the first to make a double click,
/// in milliseconds.
const DOUBLE_CLICK: u32 = 400;
/// How many pixels the magnifier shows each way.
const LOUPE_CELLS: i32 = 13;

/// The screen as it was when hotshot was started.
pub struct Shot {
    pub width: i32,
    pub height: i32,
    pub pixels: Box<dyn DerefMut<Target = [u32]>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    /// Held for the loupe: up while it is, wherever the pointer is.
    pub alt: bool,
}

/// What is done with the picture once it is taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Finish {
    pub copy: bool,
    pub save: bool,
    /// Left on the screen, over everything, in a window of its own.
    pub pin: bool,
    /// Not a picture at all: the area is what a recording is made of.
    pub record: bool,
    /// Opened on the canvas, to be made more of, its shapes still shapes.
    pub canvas: bool,
}

impl Finish {
    /// Nothing done with it: to be said otherwise a thing at a time.
    pub const NONE: Finish = Finish { copy: false, save: false, pin: false, record: false, canvas: false };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Continue,
    Cancel,
    Finish(Finish),
    /// The text on the clipboard is wanted, to be handed to `paste`.
    Paste,
}

/// The pointer's shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Cursor {
    Arrow,
    Crosshair,
    Move,
    Text,
    /// Resizing along the diagonal that falls to the right.
    Falling,
    /// Resizing along the diagonal that rises to the right.
    Rising,
    Across,
    Upright,
    /// Turning what is picked out.
    Turn,
}

/// How the window under the pointer shows, while nothing is selected
/// and a click would take it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Windows {
    /// Not at all, and a click takes nothing: the whole screen is dark
    /// until an area is dragged out.
    Off,
    /// By a line round it, dark like the rest.
    Outline,
    /// As it is, undarkened: on a screen one window fills, that is the
    /// whole screen bright until an area is dragged out.
    Bright,
}

#[derive(Clone)]
pub struct Settings {
    pub accent: u32,
    /// The color drawn in to begin with.
    pub color: u32,
    /// Which of the line widths, counted from 0.
    pub size: usize,
    pub magnifier: bool,
    /// What is made of the window under the pointer before anything is
    /// selected.
    pub windows: Windows,
    /// How much darker what is not selected is made, as a percentage.
    pub dim: u32,
    /// The radius of a rectangle's corners, in pixels of a screen 1080 high.
    pub radius: f32,
    /// The radius of the corners of the picture itself, in the same
    /// pixels; 0 for the square corners a screen has.
    pub corners: f32,
    /// Whether rectangles and ovals are drawn filled to begin with; in
    /// what, if not the color drawn in; and whether with a border, which
    /// unless it is said they have only when they are not filled.
    pub filled: bool,
    pub fill: Option<u32>,
    pub border: Option<bool>,
    /// Which of the faces text is typed in to begin with, counted from 0.
    pub face: usize,
    /// What the picture is set on, with how much room and whether with
    /// a shadow, to begin with.
    pub frame: Frame,
    /// A photograph of the user's own to set it on, if there is one.
    pub photo: Option<std::path::PathBuf>,
    /// What the folders Save can save to are called, and which of them
    /// it saves to to begin with.
    pub places: Vec<String>,
    pub place: usize,
    /// What Enter, and a double click, do.
    pub enter: Finish,
    /// The shape the selection is kept to, so wide to so tall; or the
    /// size it is, in pixels. Neither, and it is as it is dragged.
    pub aspect: Option<(i32, i32)>,
    pub fixed: Option<(i32, i32)>,
    /// An area is being chosen to be recorded, not drawn on: the bar
    /// has nothing in it but that, and the keys for the tools do nothing.
    pub recording: bool,
    /// With a photograph for a backdrop: one nobody chose, fetched,
    /// rather than the user's own.
    pub fetch: bool,
    /// How opaque what is drawn is to begin with, out of 256; whether
    /// lines and arrows are in dashes; the tool in hand; which ends of
    /// an arrow have heads; and whether a zoom is square.
    pub opacity: u32,
    pub dashed: bool,
    pub tool: Tool,
    pub heads: [bool; 2],
    pub square: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { accent: 0x3E63DD, color: RED, size: 1, magnifier: false, windows: Windows::Outline, dim: 50, radius: 6.0, corners: 0.0, filled: false, fill: None, border: None, face: 0, frame: Frame::default(), photo: None, places: Vec::new(), place: 0, enter: Finish { copy: true, ..Finish::NONE }, aspect: None, fixed: None, recording: false, fetch: false, opacity: 256, dashed: false, tool: Tool::Select, heads: [false, true], square: false }
    }
}

/// How large things are drawn on a screen of some height, against one
/// 1080 high.
pub fn scale_for(height: i32) -> f32 {
    ui_px(height) as f32 / 13.0
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Button {
    Tool(Tool),
    /// The color of a line, of text, or of anything but a box or an
    /// oval, which have a border and a fill to color apart.
    Color,
    Border,
    Fill,
    /// How wide a line is, or how large text is: a list of the four.
    Width,
    /// The face text is in: each press, the next of them.
    Face,
    /// One of the ways text is set: heavy, slanted and the rest.
    Style(u8),
    /// A line's way, straight or round a corner; the head at its first
    /// end and at its last, each a list of them; the kind of line, a list
    /// too; whether it is drawn by hand; and a zoom square, or round.
    Path,
    Start,
    End,
    Stroke,
    Sketch,
    Lens,
    /// What is picked out, over the shape above it, or under the one
    /// below; a copy of it; and away with it.
    Forward,
    Backward,
    Duplicate,
    Delete,
    Undo,
    Redo,
    Cancel,
    Save,
    Copy,
    /// The library: labels, notes and emoji to put on the picture.
    Badges,
    /// What the picture is set on, and how.
    Frame,
    /// Whether the mouse pointer is in the picture.
    Pointer,
    /// The picture left on the screen; and the area recorded.
    Pin,
    Record,
    /// Which folder Save saves to.
    Places,
    /// The picture opened in the design editor, its shapes still shapes:
    /// what Ctrl+E does.
    Design,
    /// The library opened on its emoji, which E opens it on. It has no
    /// button of its own.
    Emoji,
}

/// What a label says when it is put on the picture, to be typed over
/// there and then. A badge is a look, chosen from the library, in the
/// color in hand or its own: which the box of colors changes, as it does
/// any shape's.
pub(crate) const BADGE_SAYS: &str = "Label";

/// What the library has besides emoji, shelf by shelf: each a look of a
/// badge, the shelf it is on, what it says when it is put down, to be
/// typed over, and the color it comes in, or none for the color in hand.
/// The plain ones first, and the twelve with a finish after them.
pub(crate) const LIBRARY: [(usize, u8, &str, Option<u32>); 26] = [
    (12, 0, "Live", Some(0x30A46C)),
    (13, 0, "Do", Some(0x30A46C)),
    (14, 0, "Don't", Some(0xE5484D)),
    (15, 0, "#1284", Some(0x8D8D8D)),
    (16, 0, "Ctrl", None),
    (25, 0, "Ctrl", None),
    (17, 0, "P0", Some(0xE5484D)),
    (24, 0, "4", Some(0xE5484D)),
    (0, 0, BADGE_SAYS, None),
    (1, 0, BADGE_SAYS, None),
    (2, 0, BADGE_SAYS, None),
    (3, 0, BADGE_SAYS, None),
    (4, 0, BADGE_SAYS, None),
    (5, 0, BADGE_SAYS, None),
    (6, 0, BADGE_SAYS, None),
    (7, 0, BADGE_SAYS, None),
    (8, 0, BADGE_SAYS, None),
    (9, 0, BADGE_SAYS, None),
    (10, 0, BADGE_SAYS, None),
    (11, 0, BADGE_SAYS, None),
    (18, 1, "Opens in a new tab", Some(0x1C2024)),
    (19, 1, "Use 8 or more characters", Some(0xE5484D)),
    (20, 1, "cargo build", None),
    (21, 1, "Heads up", None),
    (22, 1, "Ask about this", Some(0xFFE629)),
    (23, 1, "Why does this say 0?", Some(0xFFFFFF)),
];

/// The library's shelves, down its side.
pub(crate) const SHELVES: [&str; 3] = ["Labels", "Notes", "Emoji"];
pub(crate) const EMOJI_SHELF: u8 = 2;

/// What of the library that is not emoji shows: whatever has in its
/// name, or in what it says, every word typed; with nothing typed, what
/// is on the shelf chosen. By their places in `LIBRARY`. The canvas's
/// library finds by it too.
pub(crate) fn library_found(query: &str, shelf: u8) -> Vec<usize> {
    let typed = query.to_lowercase();
    let words: Vec<&str> = typed.split_whitespace().collect();
    let fits = |&(look, on, says, _): &(usize, u8, &str, Option<u32>)| match words.is_empty() {
        true => on == shelf,
        false => {
            let named = format!("{} {says}", shapes::BADGE_NAMES[look]).to_lowercase();
            words.iter().all(|word| named.contains(word))
        }
    };
    LIBRARY.iter().enumerate().filter(|(_, item)| fits(item)).map(|(index, _)| index).collect()
}

/// The emoji whose names have in them every word typed, by their places
/// in the list of them all; all of them, with nothing typed.
pub(crate) fn emoji_found(query: &str) -> Vec<usize> {
    let typed = query.to_lowercase();
    let words: Vec<&str> = typed.split_whitespace().collect();
    emoji().iter().enumerate().filter(|(_, (_, name))| words.iter().all(|word| name.contains(word))).map(|(index, _)| index).collect()
}

/// The emoji there are to put on it, as stickers.
pub(crate) fn emoji() -> &'static [(&'static str, &'static str)] {
    static LIST: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| include_str!("../assets/emoji.txt").lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect())
}

/// How many emoji there are across the box they are chosen from, and
/// how many rows of them show at once.
pub(crate) const EMOJI_ACROSS: i32 = 12;
pub(crate) const EMOJI_DOWN: i32 = 7;

/// What the lines are drawn in that say where the selection lines up
/// with something: Figma's red for them.
const GUIDE_COLOR: u32 = 0xF24822;

/// How many backdrops the box of them has to a row; and the kinds of
/// them, in the order `BACKDROPS` has them, with how many of each.
const BACKDROP_ACROSS: i32 = 8;
const BACKDROP_KINDS: [(&str, usize); 3] = [("Mesh", 16), ("Gradient", 16), ("Plain", 8)];

/// A thing to be chosen in a box that hangs from the bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    /// One of the things in the library that are badges, by where it is
    /// in `LIBRARY`.
    Badge(usize),
    /// One of the library's shelves.
    Shelf(u8),
    /// An emoji: which of those the search has found.
    Emoji(usize),
    /// One of the widths of line, or sizes of text.
    Width(usize),
    /// The head at one end of a line: which end, and which of the
    /// `shapes::HEADS`, or past them, none.
    Head(usize, u8),
    /// A kind of line: solid, dashed, dotted; or, apart from those, an
    /// edge of white round it.
    Line(u8),
    /// What the picture is set on: nothing, or one of the backdrops
    /// counted from 1.
    Backdrop(usize),
    /// The rest of the backdrops, shown or put away again.
    More,
    /// A photograph of the user's own, behind the picture.
    Own,
    /// A photograph nobody chose, and a different one each time.
    Dice,
    Shadow,
    /// One of Tailwind's shadows, by where it is among them.
    Shade(usize),
    /// One of the folders Save can save to.
    Place(usize),
    /// One of the windows the picture can be in, by where it is in
    /// `frame::WINDOWS`.
    Window(usize),
}

/// The bar of tools, in the groups it is divided into. How what is drawn
/// looks is not on it: that is on the bar over what is picked out, or
/// over this one while a tool is in hand.
const GROUPS: [&[Button]; 5] = [
    &[
        Button::Tool(Tool::Select),
        Button::Tool(Tool::Arrow),
        Button::Tool(Tool::Line),
        Button::Tool(Tool::Rect),
        Button::Tool(Tool::Ellipse),
        Button::Tool(Tool::Pen),
        Button::Tool(Tool::Marker),
        Button::Tool(Tool::Text),
        Button::Tool(Tool::Counter),
        Button::Tool(Tool::Blur),
        Button::Tool(Tool::Spotlight),
        Button::Tool(Tool::Zoom),
        Button::Badges,
    ],
    &[Button::Undo, Button::Redo],
    &[Button::Frame, Button::Pointer],
    &[Button::Cancel, Button::Record, Button::Pin, Button::Save, Button::Places, Button::Copy],
    &[Button::Design],
];

/// The bar as it is when an area is being chosen to be recorded: there
/// is nothing to draw, and one thing to do.
const RECORDING: [&[Button]; 1] = [&[Button::Cancel, Button::Record]];

impl Button {
    /// What it is called, and the key that does the same.
    fn says(self) -> (&'static str, &'static str) {
        match self {
            Button::Tool(Tool::Select) => ("Move", "V"),
            Button::Tool(Tool::Arrow) => ("Arrow", "A"),
            Button::Tool(Tool::Line) => ("Line", "L"),
            Button::Tool(Tool::Rect) => ("Rectangle", "R"),
            Button::Tool(Tool::Ellipse) => ("Oval", "O"),
            Button::Tool(Tool::Pen) => ("Pen", "P"),
            Button::Tool(Tool::Marker) => ("Marker", "H"),
            Button::Tool(Tool::Text) => ("Text", "T"),
            Button::Tool(Tool::Counter) => ("Counter", "N"),
            Button::Tool(Tool::Blur) => ("Blur", "B"),
            Button::Tool(Tool::Spotlight) => ("Spotlight", "S"),
            Button::Tool(Tool::Zoom) => ("Zoom", "Z"),
            Button::Tool(Tool::Badge) | Button::Badges => ("Library: labels, notes, emoji", "J"),
            Button::Emoji => ("Emoji", "E"),
            Button::Color => ("Color", "C"),
            Button::Border => ("Border", "C"),
            Button::Fill => ("Fill", "F"),
            Button::Width => ("Width", "1\u{2013}4"),
            Button::Face => ("Font", "G"),
            Button::Style(shapes::BOLD) => ("Bold", "Ctrl+B"),
            Button::Style(shapes::ITALIC) => ("Italic", "Ctrl+I"),
            Button::Style(shapes::HIGHLIGHT) => ("Highlighted", ""),
            Button::Style(shapes::BOXED) => ("On a patch", ""),
            Button::Style(_) => ("Bulleted", ""),
            Button::Path => ("Straight or round a corner", ""),
            Button::Start => ("The start", ""),
            Button::End => ("The end", ""),
            Button::Stroke => ("Solid, dashed or dotted", "D"),
            Button::Sketch => ("Drawn by hand", ""),
            Button::Lens => ("Square or round", ""),
            Button::Forward => ("Bring forward, with Shift to the front", "Ctrl+]"),
            Button::Backward => ("Send backward, with Shift to the back", "Ctrl+["),
            Button::Duplicate => ("Duplicate", "Ctrl+D"),
            Button::Delete => ("Delete", "Del"),
            Button::Undo => ("Undo", "Ctrl+Z"),
            Button::Redo => ("Redo", "Ctrl+Shift+Z"),
            Button::Cancel => ("Cancel", "Esc"),
            Button::Save => ("Save", "Ctrl+S"),
            Button::Frame => ("Backdrop, room and shadow", "W"),
            Button::Pointer => ("Mouse pointer in the picture", "M"),
            Button::Pin => ("Pin to the screen", "Ctrl+P"),
            Button::Record => ("Record this area", "Ctrl+R"),
            Button::Places => ("Where Save saves to", ""),
            Button::Copy => ("Copy", "Ctrl+C"),
            Button::Design => ("Open in the design editor", "Ctrl+E"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Button::Tool(Tool::Select) => icons::SELECT,
            Button::Tool(Tool::Arrow) => icons::ARROW,
            Button::Tool(Tool::Line) => icons::LINE,
            Button::Tool(Tool::Rect) => icons::RECT,
            Button::Tool(Tool::Ellipse) => icons::ELLIPSE,
            Button::Tool(Tool::Pen) => icons::PEN,
            Button::Tool(Tool::Marker) => icons::MARKER,
            Button::Tool(Tool::Text) => icons::TEXT,
            Button::Tool(Tool::Counter) => icons::COUNTER,
            Button::Tool(Tool::Blur) => icons::BLUR,
            Button::Tool(Tool::Spotlight) => icons::SPOTLIGHT,
            Button::Tool(Tool::Zoom) => icons::ZOOM,
            Button::Tool(Tool::Badge) | Button::Badges => icons::BADGE,
            Button::Emoji => icons::STICKER,
            Button::Sketch => icons::PEN,
            Button::Forward => icons::FORWARD,
            Button::Backward => icons::BACKWARD,
            Button::Duplicate => icons::DUPLICATE,
            Button::Delete => icons::TRASH,
            Button::Undo => icons::UNDO,
            Button::Redo => icons::REDO,
            Button::Cancel => icons::CLOSE,
            Button::Save => icons::SAVE,
            Button::Frame => icons::FRAME,
            Button::Pointer => icons::POINTER,
            Button::Pin => icons::PIN,
            Button::Record => icons::RECORD,
            Button::Places => icons::DOWN,
            Button::Copy => icons::COPY,
            Button::Design => icons::HASH,
            // Drawn as what they stand for: a color, a width, a font,
            // a letter set the way it says, a box or a ring.
            Button::Color | Button::Border | Button::Fill | Button::Width | Button::Face | Button::Style(_) | Button::Lens | Button::Path | Button::Start | Button::End | Button::Stroke => "",
        }
    }
}

/// What the pointer is over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Button(Button),
    Swatch(u32),
    /// A step of how opaque what is drawn is, out of 256.
    Opacity(u32),
    /// Over the colors of a box or an oval: which of its border and its
    /// fill they are for; and among them, neither color at all.
    Paint(Paint),
    NoPaint,
    Pipette,
    /// The square of the box of colors, saturation across and value
    /// down; and the strip of hues under it.
    Square,
    Hues,
    /// A part of the bar, or of the box of colors, that is not a button.
    Panel,
    /// Something to be chosen in a box hung from the bar.
    Cell(Cell),
    /// An edge or a corner of the selection: which way it pulls across
    /// and down, each -1, 0 or 1.
    Handle(i32, i32),
    /// A corner of the box round the shape picked out, the same way.
    Grip(i32, i32),
    /// Just outside a corner of the shape picked out, by which it turns.
    Turn,
    /// The color in hand in hex, which a click lets be typed.
    Hex,
    /// The dot inside a corner of the rectangle picked out, by which its
    /// corners are made rounder.
    Rounder(i32, i32),
    /// The dot inside a corner of the selection, by which the corners
    /// of the picture itself are.
    Curve(i32, i32),
    /// The frame the picture is set in: what is round the selection.
    Backdrop,
    /// The dot inside a corner of the frame, which rounds its corners.
    FrameCorner,
    /// A handle on the edge of the frame, by which it is made larger:
    /// which way it pulls across and down.
    FrameGrip(i32, i32),
    /// An end of the line or arrow picked out, or the tip a counter
    /// points with: which of the shape's points it is.
    End(usize),
    Inside,
    Outside,
}

/// What is being done with the button down.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    None,
    /// A color being picked in the square of the box of colors, or its
    /// hue in the strip under it.
    Pick { hue: bool },
    /// Dragging out an area. It replaces the one there was only once the
    /// pointer has gone far enough to mean it.
    Select { from: (i32, i32), begun: bool, before: Option<Rect> },
    Move { from: (i32, i32), origin: Rect },
    Resize { pull: (i32, i32), from: (i32, i32), origin: Rect },
    /// Drawing the last of the shapes.
    Draw { from: (f32, f32) },
    /// Moving a shape: `moved` once it has gone anywhere.
    Shape { index: usize, last: (i32, i32), moved: bool },
    /// Making a shape larger or smaller by a corner of its box, which
    /// was `origin` when the corner was taken hold of.
    Scale { index: usize, pull: (i32, i32), from: (i32, i32), origin: Rect },
    /// Turning a shape about `about`, from the angle the pointer was at
    /// round it when it was taken hold of.
    Turn { index: usize, about: (f32, f32), began: f32 },
    /// Making a rectangle's corners rounder by the dot in one of them,
    /// which was taken hold of `slack` further in than the radius.
    Round { index: usize, corner: (i32, i32), slack: f32 },
    /// The same of the picture's own corners, by a dot in the selection's.
    Curve { corner: (i32, i32), slack: i32 },
    /// Taking one end of a line somewhere else.
    End { index: usize, point: usize, from: (i32, i32), moved: bool },
    /// Dragging the frame wider or narrower, by anywhere on it: taken
    /// hold of `slack` further out than the room there was.
    /// Wider when it was taken hold of beside the picture, taller when
    /// above or below it, and both at a corner.
    /// `opens` when it was the frame itself that was pressed and not a
    /// handle on its edge: a click there, with no drag, opens the box
    /// of what there is to choose about it.
    /// `shape` is how many times as wide as it was tall when taken hold
    /// of, which with Shift it stays.
    Room { from: (i32, i32), slack: (i32, i32), sides: (bool, bool), moved: bool, opens: bool, shape: f32 },
    /// Making the corners of the frame rounder, by the dot in one.
    FrameCorner { slack: i32 },
    /// Dragging out a box, with Shift or Ctrl held, to pick out the
    /// shapes it touches.
    Marquee { from: (i32, i32) },
    Button(Button),
}

/// Where the box of colors hangs from: the button that opened it, or
/// where the pointer was.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Anchor {
    Bar,
    At(i32, i32),
}

/// What the box of colors colors, of a box or an oval: its border, which
/// is what everything else has, or what is inside it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Paint {
    Line,
    Fill,
}

/// Where everything of the interface is, for one frame.
#[derive(Default)]
struct Layout {
    px: u32,
    side: i32,
    bar: Option<Rect>,
    buttons: Vec<(Rect, Button)>,
    dividers: Vec<i32>,
    label: Option<(Rect, String)>,
    popup: Option<Rect>,
    swatches: Vec<(Rect, u32)>,
    /// The steps of how opaque, in the same box.
    opacities: Vec<(Rect, u32)>,
    /// Over its colors, for a box or an oval, the border and the fill;
    /// and among the colors, none.
    paints: Vec<(Rect, Paint)>,
    none: Option<Rect>,
    pipette: Rect,
    /// The bar over what is picked out, or over the other while a tool
    /// is in hand: how what is drawn looks, and for what is picked out
    /// what can be done with it. Where it is, its buttons, the lines
    /// between their groups, and whether it is above what it is for.
    tray: Option<Rect>,
    tray_buttons: Vec<(Rect, Button)>,
    tray_dividers: Vec<i32>,
    tray_above: bool,
    /// In the box of colors: the square of saturation and value, the
    /// strip of hues, and where the color in hand is said in hex.
    square: Rect,
    hues: Rect,
    hex: Rect,
    /// The box hung from one of the bar's buttons, and what is in it.
    flyout: Option<Rect>,
    cells: Vec<(Rect, Cell)>,
    /// Where in that box the picture is shown as it will be, framed.
    look: Option<Rect>,
    /// The line to type a search in, and the line a name is said on.
    field: Option<Rect>,
    caption: Option<Rect>,
    /// What each kind of backdrop is called, over the rows of it.
    captions: Vec<((i32, i32), &'static str)>,
}

pub struct Editor {
    shot: Shot,
    /// The screen darkened, as it shows outside the selection.
    dim: Vec<u32>,
    monitors: Vec<Rect>,
    /// The windows on the screen, the topmost first.
    windows: Vec<Rect>,
    /// What the windows are called, and where they are, once known: for
    /// the bar of the window a picture is set in.
    titles: Vec<(Rect, String)>,
    settings: Settings,
    text: Text,

    pointer: (i32, i32),
    mods: Mods,
    selection: Option<Rect>,
    drag: Drag,
    tool: Tool,
    color: u32,
    size: usize,
    /// Whether the next box or oval has a fill, and in what; and whether
    /// it has a border, which is in `color`.
    filled: bool,
    fill: u32,
    bordered: bool,
    /// Which of those the box of colors is for.
    paint: Paint,
    /// The radius of the picture's corners, in pixels of a screen 1080 high.
    corners: f32,
    shapes: Vec<Shape>,
    undo: Vec<Vec<Shape>>,
    redo: Vec<Vec<Shape>>,
    /// The shape picked out with the move tool.
    selected: Vec<usize>,
    /// The shape the pointer is over, with the move tool.
    hovered: Option<usize>,
    /// The shape being made larger or smaller, as it was before.
    scaling: Option<Shape>,
    /// The text being typed, and whether it was begun just now rather
    /// than being one from before taken up again.
    editing: Option<(usize, bool)>,
    /// Where in that text the next letter goes, in bytes from its start.
    caret: usize,
    /// Which of the faces text is typed in.
    face: usize,
    /// Which ends of an arrow have heads: its first, and its last.
    heads: [bool; 2],
    popup: Option<Anchor>,
    /// The box open under one of the bar's other buttons, by the button.
    flyout: Option<Button>,
    /// Which of the library's shelves it shows.
    shelf: u8,
    /// Whether lines and arrows are drawn in dashes.
    dashed: bool,
    /// How the next line or arrow is drawn besides: the kind of head at
    /// each end; which ends of a plain line have one, where an arrow's
    /// are `heads`; in dots, with an edge of white, round a corner. And
    /// whether boxes, ovals, lines and arrows are drawn by hand.
    ends: [u8; 2],
    line_heads: [bool; 2],
    dotted: bool,
    halo: bool,
    elbow: bool,
    sketch: bool,
    /// What the picture is set on, and how.
    frame: Frame,
    /// The photograph it is set on, when it is set on one.
    photo: Option<Photo>,
    /// A photograph is wanted, of about this size: for whoever has the
    /// network to fetch.
    wish: Option<(i32, i32)>,
    /// How many photographs there have been: what tells one from the
    /// next, to whatever is made of them and kept.
    photos: u32,
    /// The picture as it will be, framed, made small for the box the
    /// frame is chosen in; and what it was made from, to tell when it
    /// is out of date.
    look: Option<(String, i32, i32, Vec<u32>)>,
    /// Whether the box of backdrops shows all of them, or the few.
    more: bool,
    /// The mouse pointer as it was when the screen was frozen, where
    /// the display would say; and whether it is in the picture.
    arrow: Option<canvas::Pointer>,
    arrow_shows: bool,
    /// Whether the loupe is up, X having been pressed for it.
    loupe: bool,
    /// How far what is being turned has gone, in radians.
    turned: f32,
    /// The digits of a color being typed in hex, while one is.
    hex: Option<String>,
    /// The color in hand as hue, saturation and value; and the colors
    /// taken lately that are not among the box's own, latest first.
    hsv: (f32, f32, f32),
    lately: Vec<u32>,
    /// Which of the folders Save saves to.
    place: usize,
    /// The frame as it shows round the selection: what it was made
    /// from, how wide it is, and its pixels, each with how opaque.
    surround: Option<(String, i32, Vec<u32>)>,
    /// How opaque what is drawn is, out of 256; and how text is set.
    opacity: u32,
    style: u8,
    /// Whether the next zoom is square, where it is usually round.
    square: bool,
    /// The shapes being moved, as they were when taken hold of; and the
    /// lines they have gone to, across and down, to be shown.
    moving: Vec<(usize, Shape)>,
    guides: Vec<(bool, i32)>,
    /// The gaps they are spaced evenly by, as Figma marks them.
    gaps: Vec<spacing::Gap>,
    /// The last copies made, and what they were made from, each by what
    /// it is called: for the next copy of them to go as far from them
    /// again, until anything but moving them is done.
    repeat: Option<(Vec<u32>, Vec<u32>)>,
    /// Where the selection being put in place lines up with something
    /// elsewhere on the screen: whether the line runs down it, where it
    /// is, and from where to where along it, to be shown as Figma shows
    /// such things.
    lines: Vec<(bool, i32, i32, i32)>,
    /// What has been typed to look for emoji by, and how many rows
    /// down them the box is.
    query: String,
    scroll: i32,
    /// Where the edges of the windows and the monitors are, across and
    /// down: what an edge of the selection goes to when it is near.
    edges: (Vec<i32>, Vec<i32>),
    /// How near that is.
    reach: i32,
    /// Whether all of the text being typed is picked out, so that what
    /// is typed next takes its place.
    all: bool,
    /// Picking a color off the screen.
    pipette: bool,
    hover: Target,
    /// The window the pointer is over, while there is no selection.
    window: Option<Rect>,
    last_press: (u32, (i32, i32)),
    dirty: bool,
}

fn ui_px(monitor_height: i32) -> u32 {
    (monitor_height.max(0) as u32 / 90).clamp(13, 30)
}

/// The color for an icon on the accent: white, or near black on an
/// accent light enough that white would be lost on it.
fn on(color: u32) -> u32 {
    shapes::ink_on(color)
}

/// The handles on the box of a shape picked out: its corners, which
/// resize it both ways, and the middles of its sides, which do one.
/// Each as which way it pulls across and down.
const GRIPS: [(i32, i32); 8] = [(-1, -1), (1, -1), (-1, 1), (1, 1), (0, -1), (0, 1), (-1, 0), (1, 0)];

/// Whether a tool draws what has a border and a fill, each in a color of
/// its own.
fn paintable(tool: Tool) -> bool {
    matches!(tool, Tool::Rect | Tool::Ellipse)
}

/// The kinds of line there are, by what each is called; the last is not
/// one of them but goes with any, an edge of white round it.
const LINE_KINDS: [&str; 4] = ["Solid", "Dashed", "Dotted", "White edge"];

/// Whether a tool draws a line, with heads to have and a way to go.
fn lines(tool: Tool) -> bool {
    matches!(tool, Tool::Line | Tool::Arrow)
}

/// What the bar over what is picked out has for how a kind of shape
/// looks; or, for shapes of more than one kind, what they all have.
/// `border` says whether a box or an oval has one, which only then has a
/// width; `styled`, whether there is the one text to set the ways of.
fn looks(kind: Option<Tool>, border: bool, styled: bool) -> Vec<Button> {
    let styles = STYLES.iter().map(|&(style, _)| Button::Style(style)).filter(|_| styled);
    match kind {
        Some(Tool::Rect | Tool::Ellipse) => [Button::Fill, Button::Border].into_iter().chain(border.then_some(Button::Width)).chain([Button::Sketch]).collect(),
        Some(Tool::Arrow | Tool::Line) => vec![Button::Color, Button::Width, Button::Path, Button::Start, Button::End, Button::Stroke, Button::Sketch],
        Some(Tool::Text) => [Button::Color, Button::Width, Button::Face].into_iter().chain(styles).collect(),
        Some(Tool::Badge) => vec![Button::Color, Button::Face],
        Some(Tool::Zoom) => vec![Button::Color, Button::Width, Button::Lens],
        Some(Tool::Blur) => vec![Button::Width],
        Some(Tool::Spotlight | Tool::Select) => Vec::new(),
        Some(Tool::Pen | Tool::Marker | Tool::Counter) | None => vec![Button::Color, Button::Width],
    }
}

/// Where near a place along the screen what is on it changes: the
/// edge of a panel, a border, where one color gives way to another,
/// for an edge of the selection to go to. `across` says the edge runs
/// down the screen, between two columns, and is looked for in the rows
/// `span` covers; otherwise it runs across, between two rows, looked
/// for in the columns. The edge is the one where most of those rows or
/// columns change, of those within reach, if enough of them do; the
/// nearest, of those that are nearly as good.
pub fn content_edge(pixels: &[u32], width: i32, height: i32, across: bool, near: i32, span: (i32, i32), reach: i32) -> Option<i32> {
    let (along, most) = if across { (height, width) } else { (width, height) };
    let (start, end) = (span.0.clamp(0, along), span.1.clamp(0, along));
    if end - start < 8 {
        return None;
    }
    // No more than a few hundred of them looked at, evenly: it is done
    // as the pointer moves.
    let step = ((end - start) / 400).max(1);
    let looked = ((end - start) / step) as u32;
    let at = |line: i32, along: i32| if across { pixels[(along * width + line) as usize] } else { pixels[(line * width + along) as usize] };
    let apart = |a: u32, b: u32| [16, 8, 0].iter().map(|shift| (a >> shift & 0xFF).abs_diff(b >> shift & 0xFF)).sum::<u32>();
    let mut found: Vec<(i32, u32)> = Vec::new();
    for edge in (near - reach).max(1)..=(near + reach).min(most - 1) {
        let changes = (start..end).step_by(step as usize).filter(|&along| apart(at(edge - 1, along), at(edge, along)) > 40).count() as u32;
        found.push((edge, changes));
    }
    let best = found.iter().map(|&(_, changes)| changes).max()?;
    if best < (looked * 3 / 10).max(6) {
        return None;
    }
    found.into_iter().filter(|&(_, changes)| changes * 4 >= best * 3).min_by_key(|&(edge, _)| (edge - near).abs()).map(|(edge, _)| edge)
}

/// An edge of something elsewhere along the line an edge of the
/// selection is on: above or below it, for one that runs down the
/// screen; beside it, for one that runs across. Found as a stretch
/// along which what is on the screen changes at the same place, a
/// little gap or two in it allowed, at least `least` long, and clear of
/// `span`, which is where the selection's own edge is. The nearest of
/// them within reach, and the longest of the nearest: where it is, and
/// where along the line the stretch begins and ends.
pub fn aligned_edge(pixels: &[u32], width: i32, height: i32, across: bool, near: i32, span: (i32, i32), reach: i32, least: i32) -> Option<(i32, i32, i32)> {
    let (along, most) = if across { (height, width) } else { (width, height) };
    let at = |line: i32, along: i32| if across { pixels[(along * width + line) as usize] } else { pixels[(line * width + along) as usize] };
    let apart = |a: u32, b: u32| [16, 8, 0].iter().map(|shift| (a >> shift & 0xFF).abs_diff(b >> shift & 0xFF)).sum::<u32>();
    let mut best: Option<(i32, i32, i32, i32)> = None;
    for edge in (near - reach).max(1)..=(near + reach).min(most - 1) {
        let (mut begun, mut last, mut gaps) = (None::<i32>, 0, 0);
        let mut runs = Vec::new();
        for spot in 0..along {
            if apart(at(edge - 1, spot), at(edge, spot)) > 40 {
                begun.get_or_insert(spot);
                (last, gaps) = (spot, 0);
            } else if let Some(first) = begun {
                gaps += 1;
                if gaps > 2 {
                    runs.push((first, last + 1));
                    begun = None;
                }
            }
        }
        if let Some(first) = begun {
            runs.push((first, last + 1));
        }
        for (start, end) in runs.into_iter().filter(|&(start, end)| end - start >= least && (end <= span.0 || start >= span.1)) {
            let candidate = ((edge - near).abs(), -(end - start), edge, start);
            if best.is_none_or(|kept| (candidate.0, candidate.1) < (kept.0, kept.1)) {
                best = Some((candidate.0, candidate.1, edge, start));
                let _ = end;
            }
        }
    }
    best.map(|(_, length, edge, start)| (edge, start, start - length))
}

/// A place along the screen, moved to the nearest of some edges if one
/// is within reach of it, and otherwise left where it is.
fn snapped(edges: &[i32], value: i32, reach: i32) -> i32 {
    edges.iter().copied().filter(|edge| (edge - value).abs() <= reach).min_by_key(|edge| (edge - value).abs()).unwrap_or(value)
}

/// How wide and tall a rectangle is that was dragged to `size` and is
/// kept to a shape, so wide to so tall. `lead` says which of the two it
/// was dragged by: its width, its height, or both, when the larger
/// says. It is no larger than there is `room` for.
fn shaped(ratio: (i32, i32), size: (i32, i32), lead: (bool, bool), room: (i32, i32)) -> (i32, i32) {
    let (across, down) = (ratio.0.max(1) as f32, ratio.1.max(1) as f32);
    let (wide, high) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let by_width = match lead {
        (true, false) => true,
        (false, true) => false,
        _ => wide / across >= high / down,
    };
    let (mut wide, mut high) = if by_width { (wide, wide * down / across) } else { (high * across / down, high) };
    let fits = (room.0.max(1) as f32 / wide).min(room.1.max(1) as f32 / high).min(1.0);
    (wide, high) = (wide * fits, high * fits);
    ((wide.round() as i32).max(1), (high.round() as i32).max(1))
}

/// Whether a shape picked out has a box with handles to resize it by.
/// Not a line or an arrow, which have their two ends; nor a counter,
/// which is the size it is.
fn boxed(tool: Tool) -> bool {
    !matches!(tool, Tool::Arrow | Tool::Line | Tool::Counter | Tool::Zoom | Tool::Badge)
}

/// Where on a box one of its handles is.
fn grip_at(frame: Rect, across: i32, down: i32) -> (i32, i32) {
    (frame.x + frame.w * (across + 1) / 2, frame.y + frame.h * (down + 1) / 2)
}

/// Where a box `size` large goes that hangs from a button on a bar: its
/// middle under the button's, under the bar; or over the bar, where it
/// would not fit under or `over` would rather have it there and it fits.
fn hung(button: Rect, bar: Rect, over: bool, size: (i32, i32), monitor: Rect, pad: i32) -> (i32, i32) {
    let (above, below) = (bar.y - pad / 2 - size.1, bar.bottom() + pad / 2);
    let (room_above, room_below) = (above >= monitor.y + pad, below + size.1 <= monitor.bottom() - pad);
    let top = if (over && room_above) || !room_below { above } else { below };
    (button.x + button.w / 2 - size.0 / 2, top)
}

/// A short line drawn as `like` is drawn, heads, dashes and all, in `ink`
/// and `stroke` wide: how a button or a list shows a kind of line.
fn sample(canvas: &mut Canvas, text: &mut Text, like: &Shape, from: (i32, i32), to: (i32, i32), ink: u32, stroke: f32) {
    let mut line = Shape::new(Tool::Line, ink, stroke, (from.0 as f32, from.1 as f32));
    line.points[1] = (to.0 as f32, to.1 as f32);
    (line.heads, line.ends, line.dashed, line.dotted, line.halo, line.elbow) = (like.heads, like.ends, like.dashed, like.dotted, like.halo, like.elbow);
    line.draw(canvas, text);
}

fn panel(canvas: &mut Canvas, rect: Rect, radius: i32) {
    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, radius, MENU_COLOR, 256);
    canvas.round_outline(rect.x, rect.y, rect.w, rect.h, radius, 1, MENU_EDGE_COLOR);
}

impl Editor {
    /// `dim` is the screen already darkened: it was made for the first
    /// frame, before there was an editor.
    pub fn new(shot: Shot, dim: Vec<u32>, monitors: Vec<Rect>, windows: Vec<Rect>, settings: Settings, text: Text) -> Editor {
        let screen = Rect::new(0, 0, shot.width, shot.height);
        let monitors: Vec<Rect> = monitors.into_iter().map(|monitor| monitor.intersect(screen)).filter(|monitor| !monitor.is_empty()).collect();
        let wanted = settings.windows != Windows::Off;
        // How things are drawn to begin with, as the settings say.
        let (opacity, square, tool, heads, dashed) = (settings.opacity.clamp(1, 256), settings.square, settings.tool, settings.heads, settings.dashed);
        // Set on a photograph to begin with: the user's own, read now,
        // or one nobody chose, asked for now and set behind the picture
        // when it comes.
        let on_photo = settings.frame.backdrop == Backdrop::Photo;
        let photo = settings.photo.as_ref().filter(|_| on_photo && !settings.fetch).and_then(|file| std::fs::read(file).ok()).and_then(|bytes| frame::open(&bytes));
        let wish = (on_photo && settings.fetch).then_some((shot.width, shot.height));
        // Every window's edges are something for the selection's to go
        // to, whether or not the windows themselves are shown; and
        // every monitor's, less the edges of the screen itself, which
        // the selection stops at anyway.
        let mut edges: (Vec<i32>, Vec<i32>) = (Vec::new(), Vec::new());
        for rect in windows.iter().map(|window| window.intersect(screen)).filter(|window| window.w > 8 && window.h > 8).chain(monitors.iter().copied()) {
            edges.0.extend([rect.x, rect.right()].into_iter().filter(|&x| x > 0 && x < screen.w));
            edges.1.extend([rect.y, rect.bottom()].into_iter().filter(|&y| y > 0 && y < screen.h));
        }
        let reach = (screen.h / 180).clamp(6, 14);
        let windows: Vec<Rect> = windows.into_iter().map(|window| window.intersect(screen)).filter(|window| wanted && window.w > 8 && window.h > 8).collect();
        Editor {
            edges,
            reach,
            dim,
            monitors: if monitors.is_empty() { vec![screen] } else { monitors },
            windows,
            titles: Vec::new(),
            color: settings.color,
            corners: settings.corners,
            // Filled to begin with, as the config file may say: unless it
            // says otherwise, in the color drawn in and with no border, as
            // was always so.
            filled: settings.filled,
            fill: settings.fill.unwrap_or(settings.color),
            bordered: settings.border.unwrap_or(!settings.filled),
            paint: Paint::Line,
            face: settings.face.min(FACES.len() - 1),
            frame: settings.frame,
            place: settings.place.min(settings.places.len().saturating_sub(1)),
            photos: u32::from(photo.is_some()),
            photo,
            wish,
            look: None,
            more: false,
            arrow: None,
            arrow_shows: false,
            loupe: false,
            turned: 0.0,
            hex: None,
            hsv: to_hsv(settings.color),
            // Tests keep to themselves, and do not read what was taken.
            lately: crate::output::state_file("colors")
                .filter(|_| !cfg!(test))
                .and_then(|file| std::fs::read_to_string(file).ok())
                .map(|text| text.split_whitespace().filter_map(|word| u32::from_str_radix(word, 16).ok()).take(5).collect())
                .unwrap_or_default(),
            surround: None,
            opacity,
            style: 0,
            square,
            moving: Vec::new(),
            guides: Vec::new(),
            gaps: Vec::new(),
            repeat: None,
            lines: Vec::new(),
            query: String::new(),
            scroll: 0,
            size: settings.size.min(STROKES.len() - 1),
            settings,
            text,
            pointer: (shot.width / 2, shot.height / 2),
            shot,
            mods: Mods::default(),
            selection: None,
            drag: Drag::None,
            tool,
            shapes: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            selected: Vec::new(),
            hovered: None,
            scaling: None,
            editing: None,
            caret: 0,
            heads,
            popup: None,
            flyout: None,
            shelf: 0,
            dashed,
            ends: [shapes::FILLED_HEAD; 2],
            line_heads: [false; 2],
            dotted: false,
            halo: false,
            elbow: false,
            sketch: false,
            all: false,
            pipette: false,
            hover: Target::Outside,
            window: None,
            last_press: (0, (-1, -1)),
            dirty: true,
        }
    }

    fn screen(&self) -> Rect {
        Rect::new(0, 0, self.shot.width, self.shot.height)
    }

    /// The monitor a point is on, or the nearest to it.
    fn monitor_at(&self, (x, y): (i32, i32)) -> Rect {
        let off = |monitor: &Rect| (monitor.x - x).max(x - monitor.right() + 1).max(0) + (monitor.y - y).max(y - monitor.bottom() + 1).max(0);
        self.monitors.iter().copied().min_by_key(off).unwrap_or(self.screen())
    }

    /// The monitor the interface is laid out on: the one most of the
    /// selection is on, or the pointer's.
    fn monitor(&self) -> Rect {
        match self.selection {
            Some(selection) => self.monitor_at(selection.center()),
            None => self.monitor_at(self.pointer),
        }
    }

    /// How large everything drawn is, against a screen 1080 high.
    fn scale(&self) -> f32 {
        ui_px(self.monitor().h) as f32 / 13.0
    }

    fn stroke(&self) -> f32 {
        STROKES[self.size] * self.scale()
    }

    /// The radius of the picture's corners on this screen: never more
    /// than half the selection's shorter side, which makes its ends round.
    fn corner_radius(&self) -> i32 {
        let most = self.selection.map_or(0, |area| area.w.min(area.h) / 2);
        ((self.corners * self.scale()).round() as i32).clamp(0, most)
    }

    fn font_px(&self) -> u32 {
        (FONT_SIZES[self.size] * self.scale()).round() as u32
    }

    pub fn selection(&self) -> Option<Rect> {
        self.selection
    }

    /// Sets the area out, as if it had been dragged.
    pub fn select(&mut self, area: Rect) {
        let area = area.intersect(self.screen());
        if !area.is_empty() {
            self.selection = Some(area);
            self.window = None;
            self.dirty = true;
        }
    }

    /// Some rows of the screen have been read, where it is read a strip
    /// at a time: they are darkened, as the rest were or will be.
    pub fn arrived(&mut self, rows: std::ops::Range<i32>) {
        let pixels = (rows.start * self.shot.width) as usize..(rows.end * self.shot.width) as usize;
        let keep = 256 - self.settings.dim.min(100) * 256 / 100;
        canvas::darken_into(&mut self.dim[pixels.clone()], &self.shot.pixels[pixels], keep);
    }

    /// The mouse pointer as it was when the screen was frozen, and
    /// whether it is in the picture to begin with: the button for it,
    /// or M, puts it in and takes it out.
    pub fn set_pointer(&mut self, pointer: canvas::Pointer, shows: bool) {
        (self.arrow, self.arrow_shows, self.dirty) = (Some(pointer), shows, true);
    }

    /// The screen darkened, as far as it has been read.
    pub fn dimmed(&self) -> &[u32] {
        &self.dim
    }

    /// Whether anything has changed since the last frame was drawn.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    // ---- Where things are ----

    fn layout(&mut self) -> Layout {
        let monitor = self.monitor();
        let px = ui_px(monitor.h);
        let (pad, side) = (px as i32 / 2, px as i32 * 9 / 4);
        let mut layout = Layout { px, side, ..Layout::default() };
        let Some(selection) = self.selection else { return layout };
        let reshaping = matches!(self.drag, Drag::Select { begun: true, .. } | Drag::Move { .. } | Drag::Resize { .. });

        // The bar: under the selection, or over it, or failing both
        // along its bottom edge on the inside.
        let margin = px as i32 / 3;
        let gap = px as i32 * 2 / 3;
        let groups: &[&[Button]] = if self.settings.recording { &RECORDING } else { &GROUPS };
        let count: i32 = groups.iter().map(|group| group.len() as i32).sum();
        let width = 2 * margin + count * side + (groups.len() as i32 - 1) * gap;
        let height = side + 2 * margin;
        // Clear of the frame the picture is set in, when it is in one.
        let room = self.frame_room().1;
        let left = selection.x + (selection.w - width) / 2;
        let below = selection.bottom() + room + pad;
        let above = selection.y - room - pad - height;
        let top = if below + height + pad <= monitor.bottom() {
            below
        } else if above >= monitor.y + pad {
            above
        } else {
            selection.bottom() - height - pad
        };
        let bar = Rect::new(left, top, width, height).kept_within(monitor.inflate(-pad));
        let hidden = reshaping || (matches!(self.drag, Drag::Draw { .. }) && bar.intersects(selection));
        if !hidden {
            layout.bar = Some(bar);
            let mut x = bar.x + margin;
            for (index, group) in groups.iter().enumerate() {
                if index > 0 {
                    layout.dividers.push(x + gap / 2);
                    x += gap;
                }
                for &button in group.iter() {
                    layout.buttons.push((Rect::new(x, bar.y + margin, side, side), button));
                    x += side;
                }
            }
        }

        // The bar over what is picked out, or the text being typed: over
        // it, clear of the handles round it, or under it where there is
        // no room over or the other bar is in the way. With nothing
        // picked out and a tool in hand, over the other bar; or under
        // it, where that is up over the selection. Not while anything is
        // being drawn, moved or reshaped, which it would be in the way of.
        let busy = matches!(self.drag, Drag::Draw { .. } | Drag::Shape { moved: true, .. } | Drag::Scale { .. } | Drag::Turn { .. } | Drag::End { moved: true, .. } | Drag::Round { .. } | Drag::Marquee { .. });
        let tray = self.tray_contents();
        if !tray.is_empty() && !hidden && !busy {
            let count = tray.iter().flatten().count() as i32;
            let parts = tray.len() as i32 - count;
            let (wide, high) = (2 * margin + count * side + parts * gap, side + 2 * margin);
            let (place, above) = match self.tray_around() {
                Some(around) => {
                    let out = px as i32;
                    let over = Rect::new(around.x + around.w / 2 - wide / 2, around.y - out - high, wide, high);
                    let under = Rect::new(over.x, around.bottom() + out, wide, high);
                    let fits = |rect: Rect| rect.y >= monitor.y + pad && rect.bottom() <= monitor.bottom() - pad && !layout.bar.is_some_and(|bar| bar.intersects(rect));
                    let above = fits(over) || !fits(under);
                    (if above { over } else { under }, above)
                }
                None => {
                    let above = bar.y >= selection.y + selection.h / 2;
                    let top = if above { bar.y - pad / 2 - high } else { bar.bottom() + pad / 2 };
                    (Rect::new(bar.x + bar.w / 2 - wide / 2, top, wide, high), above)
                }
            };
            let place = place.kept_within(monitor.inflate(-pad));
            let mut x = place.x + margin;
            for slot in tray {
                match slot {
                    Some(button) => {
                        layout.tray_buttons.push((Rect::new(x, place.y + margin, side, side), button));
                        x += side;
                    }
                    None => {
                        layout.tray_dividers.push(x + gap / 2);
                        x += gap;
                    }
                }
            }
            (layout.tray, layout.tray_above) = (Some(place), above);
        }

        // The size of the selection, over its top-left corner, or inside
        // it where there is no room or the bar is in the way.
        let words = format!("{} \u{d7} {}", selection.w, selection.h);
        let line = Text::line_height(px);
        let size = (self.text.width(&words, px) + 2 * pad, line + pad / 2);
        let outside = Rect::new(selection.x, selection.y - size.1 - pad, size.0, size.1);
        let clear = outside.y >= monitor.y && !layout.bar.is_some_and(|bar| bar.intersects(outside));
        let place = if clear { outside } else { Rect::new(selection.x + pad, selection.y + pad, size.0, size.1) };
        layout.label = Some((place.kept_within(monitor), words));

        // The box of colors: two rows of them, and under those the
        // pipette.
        if let (Some(anchor), false) = (self.popup, hidden) {
            let columns = PALETTE_COLUMNS;
            let rows = (PALETTE.len() as i32 + columns - 1) / columns;
            let words = self.text.width("Pick from the screen", px) + self.text.width("I", px) + side + 3 * pad;
            let width = (columns * side).max(words) + 2 * margin;
            // Over the colors, every other: a square of saturation and
            // value at a hue, the hues under it, and the color in hand
            // in hex beside the colors taken lately.
            let inner = width - 2 * margin;
            let (square_h, strip_h) = (inner * 3 / 5, side / 2);
            // For a box or an oval, over all of it, which of its border
            // and its fill the colors are for.
            let apart = self.paints_apart();
            let tabs = if apart { side + pad / 2 } else { 0 };
            let picker = tabs + square_h + pad / 2 + strip_h + pad / 2 + side;
            // A row more than the colors and the pipette: how opaque.
            let height = picker + rows * side + 2 * side + 2 * margin + pad / 2;
            let at = match anchor {
                Anchor::At(x, y) => (x + pad / 2, y + pad / 2),
                // From the button for what it colors, on whichever bar
                // that is on.
                Anchor::Bar => {
                    let wanted = |button: Button| match self.paint {
                        Paint::Fill => button == Button::Fill,
                        Paint::Line => matches!(button, Button::Color | Button::Border),
                    };
                    match layout.tray_buttons.iter().find(|(_, button)| wanted(*button)) {
                        Some(&(button, _)) => hung(button, layout.tray.unwrap_or(bar), layout.tray_above, (width, height), monitor, pad),
                        None => hung(Rect::new(bar.x + bar.w / 2, bar.y, 0, bar.h), bar, false, (width, height), monitor, pad),
                    }
                }
            };
            let popup = Rect::new(at.0, at.1, width, height).kept_within(monitor.inflate(-pad));
            let across = (width - 2 * margin) / columns;
            let (left, top) = (popup.x + margin, popup.y + margin);
            if apart {
                let half = inner / 2;
                layout.paints = vec![(Rect::new(left, top, half, side), Paint::Line), (Rect::new(left + half, top, inner - half, side), Paint::Fill)];
            }
            let top = top + tabs;
            layout.square = Rect::new(left, top, inner, square_h);
            layout.hues = Rect::new(left, top + square_h + pad / 2, inner, strip_h);
            let lately_top = layout.hues.bottom() + pad / 2;
            layout.hex = Rect::new(left, lately_top, across * 3, side);
            // None, beside the hex, before the colors taken lately.
            let first = if apart { 4 } else { 3 };
            if apart {
                layout.none = Some(Rect::new(left + 3 * across, lately_top, across, side));
            }
            for (index, &color) in self.lately.iter().take(columns as usize - first).enumerate() {
                layout.swatches.push((Rect::new(left + (first as i32 + index as i32) * across, lately_top, across, side), color));
            }
            let top = popup.y + margin + picker;
            for (index, &(color, _)) in PALETTE.iter().enumerate() {
                let (column, row) = (index as i32 % columns, index as i32 / columns);
                layout.swatches.push((Rect::new(left + column * across, top + row * side, across, side), color));
            }
            // How opaque: four steps of it, under the colors.
            let each = (width - 2 * margin) / OPACITIES.len() as i32;
            for (index, &opacity) in OPACITIES.iter().enumerate() {
                layout.opacities.push((Rect::new(left + index as i32 * each, top + rows * side, each, side), opacity));
            }
            layout.pipette = Rect::new(left, top + (rows + 1) * side + pad / 2, width - 2 * margin, side);
            layout.popup = Some(popup);
        }

        // The box of badges and stickers: the badges as they will look,
        // one after another for as far as a row goes, and under them
        // the stickers in rows of eight.
        if let (Some(from), false) = (self.flyout, hidden) {
            let mut cells: Vec<(Rect, Cell)> = Vec::new();
            let (across, down) = match from {
                // What the picture is set on: a look at it as it will
                // be; under that the backdrops, none first; and under
                // those a photograph nobody chose, one of the user's
                // own if there is one, how much room, and the shadow.
                Button::Frame => {
                    // The picture itself shows what is chosen, where it
                    // is, so there is nothing here but the choosing:
                    // none, a few of the backdrops and dots for the
                    // rest; or, the dots pressed, all of them, each
                    // kind under what it is called, eight to a row.
                    let across = BACKDROP_ACROSS * side;
                    let mut down = 0;
                    match self.more {
                        false => {
                            let row = [Cell::Backdrop(0)].into_iter().chain(FIRST.iter().filter_map(|name| frame::named(name)).map(|which| Cell::Backdrop(which + 1))).chain([Cell::More]);
                            for (index, cell) in row.enumerate() {
                                cells.push((Rect::new(index as i32 * side, 0, side, side), cell));
                            }
                            down += side;
                        }
                        true => {
                            let line = Text::line_height(px) + pad / 2;
                            let mut from = 0;
                            for (name, count) in BACKDROP_KINDS {
                                layout.captions.push(((pad / 2, down + pad / 4), name));
                                down += line;
                                for index in 0..count {
                                    cells.push((Rect::new(index as i32 % BACKDROP_ACROSS * side, down + index as i32 / BACKDROP_ACROSS * side, side, side), Cell::Backdrop(from + index + 1)));
                                }
                                from += count;
                                down += (count as i32 + BACKDROP_ACROSS - 1) / BACKDROP_ACROSS * side;
                            }
                            down += pad / 2;
                        }
                    }
                    let mut row = vec![Cell::Dice];
                    row.extend(self.settings.photo.is_some().then_some(Cell::Own));
                    row.push(Cell::Shadow);
                    if self.more {
                        row.extend([Cell::Backdrop(0), Cell::More]);
                    }
                    for (index, cell) in row.into_iter().enumerate() {
                        cells.push((Rect::new(index as i32 * side, down, side, side), cell));
                    }
                    // How large the shadow is, Tailwind's sizes of it,
                    // from xs to 2xl: one chosen is the shadow, on.
                    let down = down + side;
                    for index in 1..frame::SHADOWS.len() {
                        cells.push((Rect::new((index as i32 - 1) * side, down, side, side), Cell::Shade(index)));
                    }
                    // And the window it is in, if any: none, a browser,
                    // a Mac's, a terminal, a desktop's.
                    let down = down + side + pad / 2;
                    for index in 0..frame::WINDOWS.len() {
                        cells.push((Rect::new(index as i32 * side, down, side, side), Cell::Window(index)));
                    }
                    (across, down + side)
                }
                // Every emoji there is, or those whose names have in
                // them what has been typed: a line to type in, as many
                // rows of them as show at once, and under those the
                // name of the one the pointer is on.
                // The library: a line to search all of it in; its shelves
                // down its side; and what is on the shelf chosen, or what
                // has been found on all of them, the labels and notes as
                // they will look and then the emoji; and under them the
                // name of the one the pointer is on.
                Button::Badges | Button::Emoji => {
                    let (rail, each, tall) = (side * 4, (px as i32 * 19 / 2).max(side * 4), side * 3 / 2);
                    let content = (3 * each).max(EMOJI_ACROSS * side);
                    let across = rail + pad + content;
                    layout.field = Some(Rect::new(0, 0, across, side));
                    let top = side + pad / 2;
                    for (index, _) in SHELVES.iter().enumerate() {
                        cells.push((Rect::new(0, top + index as i32 * side, rail, side), Cell::Shelf(index as u8)));
                    }
                    let (left, searching) = (rail + pad, !self.query.is_empty());
                    let mut down = top;
                    if searching || self.shelf != EMOJI_SHELF {
                        let found = self.library_found();
                        for (at, &item) in found.iter().enumerate() {
                            cells.push((Rect::new(left + at as i32 % 3 * each, down + at as i32 / 3 * tall, each, tall), Cell::Badge(item)));
                        }
                        down += (found.len() as i32 + 2) / 3 * tall;
                    }
                    if searching || self.shelf == EMOJI_SHELF {
                        let found = self.emoji_found();
                        let rows = if searching { 3 } else { EMOJI_DOWN };
                        for at in 0..EMOJI_ACROSS * rows {
                            let index = (self.scroll * EMOJI_ACROSS + at) as usize;
                            if index < found.len() {
                                cells.push((Rect::new(left + at % EMOJI_ACROSS * side, down + at / EMOJI_ACROSS * side, side, side), Cell::Emoji(index)));
                            }
                        }
                        down += ((found.len() as i32 - self.scroll * EMOJI_ACROSS).clamp(0, EMOJI_ACROSS * rows) + EMOJI_ACROSS - 1) / EMOJI_ACROSS * side;
                    }
                    let down = down.max(top + SHELVES.len() as i32 * side);
                    let line = Text::line_height(px);
                    layout.caption = Some(Rect::new(0, down + pad / 4, across, line));
                    (across, down + pad / 4 + line)
                }
                // The heads there are for an end of a line, none first;
                // or the kinds of line: each drawn as it is, and named.
                Button::Start | Button::End | Button::Stroke => {
                    let rows: Vec<Cell> = match from {
                        Button::Stroke => (0..LINE_KINDS.len() as u8).map(Cell::Line).collect(),
                        _ => {
                            let (end, none) = (usize::from(from == Button::End), shapes::HEADS.len() as u8);
                            [none].into_iter().chain(0..none).map(|style| Cell::Head(end, style)).collect()
                        }
                    };
                    let widest = shapes::HEADS.iter().chain(LINE_KINDS.iter()).map(|name| self.text.width(name, px)).max().unwrap_or(0);
                    let across = 3 * side + widest + 2 * pad;
                    for (index, cell) in rows.iter().enumerate() {
                        cells.push((Rect::new(0, index as i32 * side, across, side), *cell));
                    }
                    (across, rows.len() as i32 * side)
                }
                // The widths of line, or sizes of text, one to a row:
                // each drawn as it is, what it is called, and its key.
                Button::Width => {
                    let names = self.width_names();
                    let widest = names.iter().map(|name| self.text.width(name, px)).max().unwrap_or(0);
                    let across = 2 * side + widest + self.text.width("4", px) + 3 * pad;
                    for index in 0..names.len() {
                        cells.push((Rect::new(0, index as i32 * side, across, side), Cell::Width(index)));
                    }
                    (across, names.len() as i32 * side)
                }
                // Where Save saves to: the folders the config file
                // names, one to a row.
                Button::Places => {
                    let widest = self.settings.places.iter().map(|place| self.text.width(place, px)).max().unwrap_or(0);
                    let across = widest + side + 2 * pad;
                    for index in 0..self.settings.places.len() {
                        cells.push((Rect::new(0, index as i32 * side, across, side), Cell::Place(index)));
                    }
                    (across, self.settings.places.len() as i32 * side)
                }
                // Nothing else has a box of its own.
                _ => (0, 0),
            };
            let (width, height) = (across + 2 * margin, down + 2 * margin);
            // From its button, on whichever bar that is on.
            let at = match layout.tray_buttons.iter().find(|(_, button)| *button == from) {
                Some(&(button, _)) => hung(button, layout.tray.unwrap_or(bar), layout.tray_above, (width, height), monitor, pad),
                None => {
                    let button = layout.buttons.iter().find(|(_, button)| *button == from).map_or(bar, |(rect, _)| *rect);
                    hung(button, bar, false, (width, height), monitor, pad)
                }
            };
            let flyout = Rect::new(at.0, at.1, width, height).kept_within(monitor.inflate(-pad));
            layout.cells = cells.into_iter().map(|(rect, cell)| (rect.moved(flyout.x + margin, flyout.y + margin), cell)).collect();
            layout.look = layout.look.map(|look| look.moved(flyout.x + margin, flyout.y + margin));
            layout.field = layout.field.map(|field| field.moved(flyout.x + margin, flyout.y + margin));
            layout.caption = layout.caption.map(|caption| caption.moved(flyout.x + margin, flyout.y + margin));
            layout.captions.iter_mut().for_each(|(at, _)| *at = (at.0 + flyout.x + margin, at.1 + flyout.y + margin));
            layout.flyout = Some(flyout);
        }
        layout
    }

    fn target_at(&mut self, (x, y): (i32, i32)) -> Target {
        let layout = self.layout();
        if let Some(flyout) = layout.flyout {
            if let Some((_, cell)) = layout.cells.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Cell(*cell);
            }
            if flyout.contains(x, y) {
                return Target::Panel;
            }
        }
        if let Some(popup) = layout.popup {
            if let Some((_, color)) = layout.swatches.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Swatch(*color);
            }
            if let Some((_, opacity)) = layout.opacities.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Opacity(*opacity);
            }
            if let Some((_, paint)) = layout.paints.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Paint(*paint);
            }
            if layout.none.is_some_and(|none| none.contains(x, y)) {
                return Target::NoPaint;
            }
            if layout.pipette.contains(x, y) {
                return Target::Pipette;
            }
            if layout.square.contains(x, y) {
                return Target::Square;
            }
            if layout.hues.inflate(3).contains(x, y) {
                return Target::Hues;
            }
            if layout.hex.contains(x, y) {
                return Target::Hex;
            }
            if popup.contains(x, y) {
                return Target::Panel;
            }
        }
        if let Some(tray) = layout.tray {
            if let Some((_, button)) = layout.tray_buttons.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Button(*button);
            }
            if tray.contains(x, y) {
                return Target::Panel;
            }
        }
        if let Some(bar) = layout.bar {
            if let Some((_, button)) = layout.buttons.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Button(*button);
            }
            if bar.contains(x, y) {
                return Target::Panel;
            }
        }
        let Some(selection) = self.selection else { return Target::Outside };
        // The shape picked out comes before the selection it is on: the
        // dots in its corners, then the corners of its box.
        if let (Some(index), false) = (self.picked(), self.pipette) {
            let reach = (layout.px as i32 / 3).max(5);
            let near = |(px, py): (i32, i32)| (x - px).abs() <= reach && (y - py).abs() <= reach;
            if let Some((across, down, _)) = self.rounders(index).into_iter().flatten().find(|&(_, _, at)| near(at)) {
                return Target::Rounder(across, down);
            }
            if let Some((point, _)) = self.ends(index).into_iter().find(|&(_, at)| near(at)) {
                return Target::End(point);
            }
            // A line has its ends to be reshaped by, and no box.
            if boxed(self.shapes[index].tool) {
                if let Some((across, down)) = GRIPS.into_iter().find(|&(across, down)| near(self.grip_of(index, across, down))) {
                    return Target::Grip(across, down);
                }
            }
            // Just outside a corner, as in Figma: it turns.
            let far = |(px, py): (i32, i32)| (x - px).abs() <= reach * 3 / 2 && (y - py).abs() <= reach * 3 / 2;
            if self.turners(index).into_iter().any(far) {
                return Target::Turn;
            }
        }
        if !self.pipette {
            let reach = (layout.px as i32 / 3).max(5);
            let near = |&(_, _, (px, py)): &(i32, i32, (i32, i32))| (x - px).abs() <= reach && (y - py).abs() <= reach;
            if let Some((across, down, _)) = self.curves().into_iter().flatten().find(near) {
                return Target::Curve(across, down);
            }
        }
        if !self.pipette {
            // The corners reach a little further than the edges, being
            // harder to find; and a selection too small for both keeps
            // its inside for moving it by.
            let reach = (layout.px as i32 / 2).max(6);
            let near = |at: i32, edge: i32, reach: i32| (at - edge).abs() <= reach;
            let pull = |at: i32, from: i32, to: i32, size: i32| {
                let reach = reach.min(size / 4).max(2);
                if near(at, from, reach) { -1 } else if near(at, to, reach) { 1 } else { 0 }
            };
            let around = selection.inflate(reach);
            if around.contains(x, y) {
                let (across, down) = (pull(x, selection.x, selection.right(), selection.w), pull(y, selection.y, selection.bottom(), selection.h));
                if across != 0 || down != 0 {
                    return Target::Handle(across, down);
                }
            }
        }
        // A zoom may reach past the selection, and is got hold of there
        // as it is anywhere on it.
        let lens = self.tool == Tool::Select && self.shapes.iter().any(|shape| shape.tool == Tool::Zoom && shape.zoomed().1.contains(x, y));
        if selection.contains(x, y) || lens {
            Target::Inside
        } else if let Some((across, down)) = self.frame_grips().into_iter().find(|&(across, down)| {
            let grip = grip_at(self.frame_rect(), across, down);
            (x - grip.0).abs() <= 8 && (y - grip.1).abs() <= 8
        }) {
            Target::FrameGrip(across, down)
        } else if self.frame_dot().is_some_and(|dot| (x - dot.0).abs() <= 7 && (y - dot.1).abs() <= 7) {
            Target::FrameCorner
        } else if self.frame_rect().contains(x, y) {
            Target::Backdrop
        } else {
            Target::Outside
        }
    }

    /// The topmost shape at a point.
    fn shape_at(&mut self, (x, y): (i32, i32)) -> Option<usize> {
        (0..self.shapes.len()).rev().find(|&index| self.shapes[index].hit(x, y, &mut self.text))
    }

    /// The shape picked out, while the move tool is the one in hand and
    /// it is the only one: the one that has handles to be reshaped by.
    fn picked(&self) -> Option<usize> {
        match self.selected[..] {
            [index] if self.tool == Tool::Select && index < self.shapes.len() => Some(index),
            _ => None,
        }
    }

    /// The shapes picked out, which with Shift or Ctrl can be several.
    fn chosen(&self) -> Vec<usize> {
        self.selected.iter().copied().filter(|&index| index < self.shapes.len()).collect()
    }

    /// The box drawn round a shape: all of it, and a little room.
    fn frame_of(&mut self, index: usize) -> Rect {
        let room = (ui_px(self.monitor().h) as i32 / 4).max(3);
        self.shapes[index].bounds(&mut self.text).inflate(room)
    }

    /// The same of a shape as if it were not turned: what it is sized by,
    /// along its own sides.
    fn local_frame_of(&mut self, index: usize) -> Rect {
        let room = (ui_px(self.monitor().h) as i32 / 4).max(3);
        self.shapes[index].local_bounds(&mut self.text).inflate(room)
    }

    /// Where one of the handles of a shape's box is, turned with it.
    fn grip_of(&mut self, index: usize, across: i32, down: i32) -> (i32, i32) {
        let (x, y) = grip_at(self.local_frame_of(index), across, down);
        let (x, y) = self.shapes[index].returned((x as f32, y as f32), &mut self.text);
        (x.round() as i32, y.round() as i32)
    }

    /// Where, just outside each corner of the shape picked out, it is
    /// taken hold of to be turned; the corners of its box, turned with
    /// it, or of what holds a line.
    fn turners(&mut self, index: usize) -> Vec<(i32, i32)> {
        if !self.shapes[index].turns() {
            return Vec::new();
        }
        let out = (ui_px(self.monitor().h) as f32 * 0.6).max(8.0);
        let angle = self.shapes[index].angle;
        [(-1, -1), (1, -1), (-1, 1), (1, 1)]
            .into_iter()
            .map(|(across, down)| {
                let (x, y) = self.grip_of(index, across, down);
                let (dx, dy) = shapes::turn_point((across as f32 * out, down as f32 * out), (0.0, 0.0), angle);
                ((x as f32 + dx).round() as i32, (y as f32 + dy).round() as i32)
            })
            .collect()
    }

    /// What a shape turns about: its own middle, or the middle of what
    /// holds a line or a stroke.
    fn pivot_of(&mut self, index: usize) -> (f32, f32) {
        if self.shapes[index].turnable() {
            return self.shapes[index].centre(&mut self.text);
        }
        let place = self.shapes[index].bounds(&mut self.text);
        (place.x as f32 + place.w as f32 / 2.0, place.y as f32 + place.h as f32 / 2.0)
    }

    /// The ends a shape is reshaped by, each with which of its points it
    /// is: both of a line or an arrow, and of a counter the tip it
    /// points with, when it points.
    fn ends(&self, index: usize) -> Vec<(usize, (i32, i32))> {
        let shape = &self.shapes[index];
        let at = |point: usize| (point, (shape.points[point].0.round() as i32, shape.points[point].1.round() as i32));
        match shape.tool {
            // And of those two, the dot half way along that bends it,
            // where it is long enough for a third dot.
            Tool::Arrow | Tool::Line => {
                let (a, b, middle) = (shape.points[0], shape.points[1], shape.middle());
                let mut ends = vec![at(0), at(1)];
                // Not an elbow, which goes its own way round its corner.
                if !shape.elbow && (a.0 - b.0).hypot(a.1 - b.1) >= 3.0 * ui_px(self.monitor().h) as f32 {
                    ends.push((2, (middle.0.round() as i32, middle.1.round() as i32)));
                }
                ends
            }
            Tool::Counter if shape.points[0] != shape.points[1] => vec![at(1)],
            // The corner the larger picture of a zoom is put down by.
            // And its far corner, which says how many times larger.
            Tool::Zoom => {
                let [middle, rim] = shape.zoom_handles();
                vec![(2, middle), (3, rim)]
            }
            _ => Vec::new(),
        }
    }

    /// Where the dots are by which the corners of the picture itself are
    /// made rounder: one inside each corner of the selection, with the
    /// move tool in hand, as a rectangle picked out has them. None on a
    /// selection too small to have them, or while it is being reshaped.
    fn curves(&self) -> Option<[(i32, i32, (i32, i32)); 4]> {
        let area = self.selection?;
        let least = ui_px(self.monitor().h) as i32;
        let busy = matches!(self.drag, Drag::Select { begun: true, .. } | Drag::Move { .. } | Drag::Resize { .. } | Drag::Draw { .. });
        if self.tool != Tool::Select || busy || area.w.min(area.h) < least * 4 {
            return None;
        }
        let inset = self.corner_radius().max(least).min(area.w / 2).min(area.h / 2);
        let at = |across: i32, down: i32| (across, down, (if across < 0 { area.x + inset } else { area.right() - inset }, if down < 0 { area.y + inset } else { area.bottom() - inset }));
        Some([at(-1, -1), at(1, -1), at(-1, 1), at(1, 1)])
    }

    /// How far in from a corner of the selection a point is, by the
    /// lesser of across and down.
    fn inward(&self, corner: (i32, i32), at: (i32, i32)) -> i32 {
        let area = self.selection.unwrap_or_default();
        let across = if corner.0 < 0 { at.0 - area.x } else { area.right() - at.0 };
        let down = if corner.1 < 0 { at.1 - area.y } else { area.bottom() - at.1 };
        across.min(down)
    }

    /// Where the dots are by which a rectangle's corners are made
    /// rounder: one inside each corner, as far in as the corner is
    /// round, or far enough to be got hold of if that is not far. Each
    /// with which corner it is. None for what is no rectangle, or one
    /// too small to have them.
    fn rounders(&self, index: usize) -> Option<[(i32, i32, (i32, i32)); 4]> {
        let shape = &self.shapes[index];
        let (left, top, width, height) = shape.frame();
        let least = ui_px(self.monitor().h) as f32;
        if !matches!(shape.tool, Tool::Rect | Tool::Spotlight) || shape.angle != 0.0 || width.min(height) < least * 3.0 {
            return None;
        }
        let inset = shape.radius.max(least).min(width / 2.0).min(height / 2.0);
        let at = |across: i32, down: i32| {
            let x = if across < 0 { left + inset } else { left + width - inset };
            let y = if down < 0 { top + inset } else { top + height - inset };
            (across, down, (x.round() as i32, y.round() as i32))
        };
        Some([at(-1, -1), at(1, -1), at(-1, 1), at(1, 1)])
    }

    pub fn cursor(&self) -> Cursor {
        if self.pipette && !matches!(self.hover, Target::Button(_) | Target::Panel | Target::Swatch(_) | Target::Pipette | Target::Cell(_)) {
            return Cursor::Crosshair;
        }
        let pull = match (self.drag, self.hover) {
            (Drag::Resize { pull, .. } | Drag::Scale { pull, .. }, _) => Some(pull),
            (Drag::None, Target::Handle(across, down) | Target::Grip(across, down) | Target::FrameGrip(across, down)) => Some((across, down)),
            (Drag::Turn { .. }, _) | (Drag::None, Target::Turn) => return Cursor::Turn,
            (Drag::None, Target::Hex) => return Cursor::Text,
            (Drag::Round { .. } | Drag::Curve { .. } | Drag::End { .. } | Drag::FrameCorner { .. }, _) | (Drag::None, Target::Rounder(..) | Target::Curve(..) | Target::End(_) | Target::FrameCorner) => return Cursor::Arrow,
            (Drag::Marquee { .. }, _) => return Cursor::Crosshair,
            _ => None,
        };
        if let Some((across, down)) = pull {
            return match (across, down) {
                (0, _) => Cursor::Upright,
                (_, 0) => Cursor::Across,
                _ if across == down => Cursor::Falling,
                _ => Cursor::Rising,
            };
        }
        match (self.drag, self.hover) {
            (Drag::Move { .. } | Drag::Shape { .. }, _) => Cursor::Move,
            (Drag::Select { .. } | Drag::Draw { .. }, _) => Cursor::Crosshair,
            (Drag::Room { .. }, _) => Cursor::Move,
            (_, Target::Button(_) | Target::Swatch(_) | Target::Pipette | Target::Panel | Target::Cell(_) | Target::Backdrop | Target::Paint(_) | Target::NoPaint | Target::Opacity(_)) => Cursor::Arrow,
            (_, Target::Inside) => match self.tool {
                Tool::Select => Cursor::Move,
                Tool::Text => Cursor::Text,
                _ => Cursor::Crosshair,
            },
            _ => Cursor::Crosshair,
        }
    }

    // ---- The pointer ----

    pub fn pointer_moved(&mut self, x: i32, y: i32, mods: Mods) {
        let at = (x.clamp(0, self.shot.width - 1), y.clamp(0, self.shot.height - 1));
        if at == self.pointer && mods == self.mods {
            return;
        }
        (self.pointer, self.mods) = (at, mods);
        let screen = self.screen();
        match &mut self.drag {
            Drag::None | Drag::Button(_) => {
                let (hover, window, hovered) = (self.hover, self.window, self.hovered);
                self.hover = self.target_at(at);
                self.window = match self.selection {
                    None => self.windows.iter().copied().find(|window| window.contains(at.0, at.1)),
                    Some(_) => None,
                };
                // With the move tool, the shape under the pointer shows
                // its box: that is what a click would pick out.
                self.hovered = match (self.tool, self.hover) {
                    (Tool::Select, Target::Inside) => self.shape_at(at),
                    _ => None,
                };
                self.dirty |= hover != self.hover || window != self.window || hovered != self.hovered || self.loupe_shows();
            }
            Drag::Scale { index, pull, from, origin } => {
                let (index, pull, origin, from) = (*index, *pull, *origin, *from);
                let Some(before) = self.scaling.clone() else { return };
                // Turned, the drag is taken as along its own sides.
                let mut local = |(x, y): (i32, i32)| {
                    let (x, y) = before.unturn((x as f32, y as f32), &mut self.text);
                    (x.round() as i32, y.round() as i32)
                };
                let (now, then) = (local(at), local(from));
                let by = (now.0 - then.0, now.1 - then.1);
                // How many times as wide and as tall its box now is; less
                // than nought when the corner has been pulled past the
                // one opposite, which turns the shape over.
                let times = |pull: i32, by: i32, size: i32| {
                    let size = size.max(1) as f32;
                    let times = if pull == 0 { 1.0 } else { (size + (pull * by) as f32) / size };
                    if times.abs() < 0.02 { 0.02f32.copysign(times) } else { times }
                };
                // With Alt, as in Figma, about its middle, which stays
                // where it is: the drag pulls both sides, so it counts
                // twice.
                let by = if mods.alt { (by.0 * 2, by.1 * 2) } else { by };
                let (mut across, mut down) = (times(pull.0, by.0, origin.w), times(pull.1, by.1, origin.h));
                let middle = (origin.x as f32 + origin.w as f32 / 2.0, origin.y as f32 + origin.h as f32 / 2.0);
                // The corner opposite the one held stays where it is; or
                // with Alt, the middle.
                let mut still = match mods.alt {
                    true => middle,
                    false => (if pull.0 < 0 { origin.right() } else { origin.x } as f32, if pull.1 < 0 { origin.bottom() } else { origin.y } as f32),
                };
                // Text keeps its shape, and anything does with Shift: by
                // a side, the other way grows alike about its middle.
                if before.tool == Tool::Text {
                    (across, down) = (across.abs().max(down.abs()), across.abs().max(down.abs()));
                } else if mods.shift {
                    match pull {
                        (0, _) => (across, still.0) = (down.abs(), middle.0),
                        (_, 0) => (down, still.1) = (across.abs(), middle.1),
                        _ => {
                            let both = across.abs().max(down.abs());
                            (across, down) = (both.copysign(across), both.copysign(down));
                        }
                    }
                }
                let mut shape = before.clone();
                for point in &mut shape.points {
                    *point = (still.0 + (point.0 - still.0) * across, still.1 + (point.1 - still.1) * down);
                }
                if shape.tool == Tool::Text {
                    shape.font_px = ((before.font_px as f32 * down).round() as u32).max(6);
                }
                shape.touch();
                // Its middle moves as it is sized, and it turns about
                // that: put back so that what stays still does on screen.
                if before.angle != 0.0 {
                    let (want, now) = (before.returned(still, &mut self.text), shape.returned(still, &mut self.text));
                    shape.translate(want.0 - now.0, want.1 - now.1);
                }
                self.shapes[index] = shape;
                self.dirty = true;
            }
            Drag::Turn { index, about, began } => {
                let (index, about, began) = (*index, *about, *began);
                let Some(before) = self.scaling.clone() else { return };
                let mut by = (at.1 as f32 - about.1).atan2(at.0 as f32 - about.0) - began;
                // With Shift, to whole fifteen degrees: of its angle, or
                // of the turn for what turns by its points.
                if mods.shift {
                    let step = 15f32.to_radians();
                    by = match before.turnable() {
                        true => ((before.angle + by) / step).round() * step - before.angle,
                        false => (by / step).round() * step,
                    };
                }
                let mut shape = before.clone();
                shape.turn(by, about, &mut self.text);
                self.shapes[index] = shape;
                self.turned = by;
                self.dirty = true;
            }
            Drag::End { index, point, from, moved } => {
                // Until it has gone far enough to be meant, it is a
                // click on the end, which is something else.
                *moved |= (at.0 - from.0).abs().max((at.1 - from.1).abs()) >= DRAG_AFTER;
                if !*moved {
                    return;
                }
                let (index, point) = (*index, *point);
                let to = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
                let shape = &mut self.shapes[index];
                // Let go of what it was stuck to while it is moved.
                if point < 2 {
                    shape.stuck[point] = None;
                }
                // With Shift, at one of eight angles from the other end.
                let other = shape.points[1 - point.min(1)];
                // A zoom's handles are not its points: one moves its
                // larger picture, the other says how many times larger.
                // Nor is the dot in the middle of a line, which bends it.
                match shape.tool {
                    Tool::Zoom => shape.zoom_to(point, to),
                    Tool::Arrow | Tool::Line if point == 2 => shape.bend_to(to),
                    _ => shape.points[point] = if mods.shift { shapes::constrained(Tool::Line, other, to) } else { to },
                }
                shape.touch();
                self.dirty = true;
            }
            Drag::Marquee { .. } => self.dirty = true,
            Drag::Pick { hue } => {
                let hue = *hue;
                self.pick(hue, at);
            }
            Drag::Room { from, slack, sides, moved, shape, .. } => {
                *moved |= (at.0 - from.0).abs().max((at.1 - from.1).abs()) >= DRAG_AFTER;
                let (slack, sides, moved, shape) = (*slack, *sides, *moved, *shape);
                if moved {
                    // As much room as the pointer is out from the
                    // selection, less where on the frame it took hold:
                    // across, down, or both.
                    let (out, scale) = (self.outward(at), self.scale());
                    let room = |out: i32, slack: i32| (((out - slack).max(0) as f32 / scale).round() as i32).clamp(0, 800);
                    if sides.0 {
                        self.frame.pad.0 = room(out.0, slack.0);
                    }
                    if sides.1 {
                        self.frame.pad.1 = room(out.1, slack.1);
                    }
                    // With Shift, the frame keeps the shape it had: the
                    // room the way it was not dragged is what makes it so,
                    // and across is the way it was, where it was both.
                    if let (true, Some(area)) = (mods.shift, self.selection) {
                        let other = |room: i32, size: i32, other: i32, shape: f32| ((((size + 2 * room) as f32 * shape - other as f32) / 2.0).max(0.0) / scale).round() as i32;
                        match sides.0 {
                            true => self.frame.pad.1 = other((self.frame.pad.0 as f32 * scale).round() as i32, area.w, area.h, 1.0 / shape).clamp(0, 800),
                            false => self.frame.pad.0 = other((self.frame.pad.1 as f32 * scale).round() as i32, area.h, area.w, shape).clamp(0, 800),
                        }
                    }
                    self.dirty = true;
                }
            }
            Drag::FrameCorner { slack } => {
                let (slack, rect) = (*slack, self.frame_rect());
                let inward = (at.0 - rect.x).min(at.1 - rect.y) - slack;
                self.frame.radius = ((inward.clamp(0, rect.w.min(rect.h) / 2) as f32 / self.scale()).round() as i32).max(0);
                self.dirty = true;
            }
            Drag::Curve { corner, slack } => {
                // Kept as it would be on a screen 1080 high, which is
                // how it is set in the config file too.
                let (corner, slack) = (*corner, *slack);
                let most = self.selection.map_or(0, |area| area.w.min(area.h) / 2);
                self.corners = (self.inward(corner, at) - slack).clamp(0, most) as f32 / self.scale();
                self.dirty = true;
            }
            Drag::Round { index, corner, slack } => {
                let (index, corner, slack) = (*index, *corner, *slack);
                let shape = &mut self.shapes[index];
                let (left, top, width, height) = shape.frame();
                // How far in from its corner the dot has been taken.
                let across = if corner.0 < 0 { at.0 as f32 - left } else { left + width - at.0 as f32 };
                let down = if corner.1 < 0 { at.1 as f32 - top } else { top + height - at.1 as f32 };
                shape.radius = (across.min(down) - slack).clamp(0.0, width.min(height) / 2.0);
                shape.touch();
                // And the next rectangle drawn has corners as round.
                let radius = shape.radius;
                self.settings.radius = radius / self.scale();
                self.dirty = true;
            }
            Drag::Select { from, begun, .. } => {
                *begun |= (at.0 - from.0).abs().max((at.1 - from.1).abs()) >= DRAG_AFTER;
                if *begun {
                    // The pixel under the pointer is taken too. The
                    // edge being placed goes to the edge of a window,
                    // when it comes near one, unless Ctrl says not.
                    let mut at = if mods.ctrl { at } else { (snapped(&self.edges.0, at.0, self.reach), snapped(&self.edges.1, at.1, self.reach)) };
                    // Or, away from any window's edge, to where what is
                    // on the screen changes, near the pointer: the edge
                    // there is between the pixel the pointer is on and
                    // the next one out.
                    // And the corner the drag began at goes to its edges
                    // in the same way: it was put down by eye too.
                    let (mut start, began) = (*from, *from);
                    if !mods.ctrl {
                        let (width, height) = (self.shot.width, self.shot.height);
                        let edge = |at: i32, start: i32| if at >= start { at + 1 } else { at };
                        let back = |edge: i32, start: i32, at: i32| if at >= start { edge - 1 } else { edge };
                        let (rows, columns) = ((start.1.min(at.1), start.1.max(at.1) + 1), (start.0.min(at.0), start.0.max(at.0) + 1));
                        if at.0 == self.pointer.0 {
                            if let Some(found) = content_edge(&self.shot.pixels, width, height, true, edge(at.0, start.0), rows, self.reach) {
                                at.0 = back(found, start.0, at.0);
                            }
                        }
                        if at.1 == self.pointer.1 {
                            if let Some(found) = content_edge(&self.shot.pixels, width, height, false, edge(at.1, start.1), columns, self.reach) {
                                at.1 = back(found, start.1, at.1);
                            }
                        }
                        // The start's edges are on its far side from the
                        // pointer, where the pixel it is on is the first
                        // in: the other way round to the pointer's.
                        let (across, down) = (at.0 >= start.0, at.1 >= start.1);
                        let first = |start: i32, onward: bool| if onward { start } else { start + 1 };
                        if let Some(found) = content_edge(&self.shot.pixels, width, height, true, first(start.0, across), rows, self.reach) {
                            start.0 = if across { found } else { found - 1 };
                        }
                        if let Some(found) = content_edge(&self.shot.pixels, width, height, false, first(start.1, down), columns, self.reach) {
                            start.1 = if down { found } else { found - 1 };
                        }
                    }
                    let (from, area) = (start, Rect::spanning(start, at));
                    let mut area = Rect::new(area.x, area.y, area.w + 1, area.h + 1);
                    // With Alt, as in Figma: about where the drag began,
                    // as far one way of it as the pointer is the other.
                    if mods.alt {
                        let (dx, dy) = ((at.0 - began.0).abs(), (at.1 - began.1).abs());
                        area = Rect::new(began.0 - dx, began.1 - dy, 2 * dx + 1, 2 * dy + 1).intersect(screen);
                    }
                    // Kept to the shape that was asked for; or, with
                    // Shift, square.
                    if let (Some(ratio), false) = (self.settings.aspect.or(mods.shift.then_some((1, 1))), mods.alt) {
                        let (left, up) = (at.0 < from.0, at.1 < from.1);
                        let room = (if left { from.0 + 1 - screen.x } else { screen.right() - from.0 }, if up { from.1 + 1 - screen.y } else { screen.bottom() - from.1 });
                        let (wide, high) = shaped(ratio, (area.w, area.h), (true, true), room);
                        area = Rect::new(if left { from.0 + 1 - wide } else { from.0 }, if up { from.1 + 1 - high } else { from.1 }, wide, high);
                    }
                    self.selection = Some(area);
                    (self.window, self.popup) = (None, None);
                    self.selected.clear();
                    self.dirty = true;
                }
            }
            Drag::Move { from, origin } => {
                // Shift keeps it to across or down, whichever it has gone
                // further.
                let by = (at.0 - from.0, at.1 - from.1);
                let by = if mods.shift { if by.0.abs() >= by.1.abs() { (by.0, 0) } else { (0, by.1) } } else { by };
                let mut moved = origin.moved(by.0, by.1).kept_within(screen);
                // Each of its sides goes to an edge it comes near, a
                // window's or one in what is on the screen, taking the
                // rest with it: by whichever of the two sides is nearer
                // one, across and down. Ctrl says not.
                if !mods.ctrl {
                    let (shot, (width, height), reach) = (&self.shot.pixels, (self.shot.width, self.shot.height), self.reach);
                    let nearest = |sides: [i32; 2], edges: &[i32], across: bool, span: (i32, i32)| {
                        sides
                            .into_iter()
                            .filter_map(|side| match snapped(edges, side, reach) {
                                same if same == side => content_edge(shot, width, height, across, side, span, reach).map(|edge| edge - side),
                                moved => Some(moved - side),
                            })
                            .min_by_key(|by: &i32| by.abs())
                    };
                    // Kept to one way by Shift, it is not taken the other.
                    let locked = (mods.shift && by.0 == 0, mods.shift && by.1 == 0);
                    let by_x = nearest([moved.x, moved.right()], &self.edges.0, true, (moved.y, moved.bottom())).filter(|_| !locked.0);
                    let by_y = nearest([moved.y, moved.bottom()], &self.edges.1, false, (moved.x, moved.right())).filter(|_| !locked.1);
                    moved = moved.moved(by_x.unwrap_or(0), by_y.unwrap_or(0)).kept_within(screen);
                }
                self.selection = Some(moved);
                self.dirty = true;
            }
            Drag::Resize { pull, from, origin } => {
                let (dx, dy) = (at.0 - from.0, at.1 - from.1);
                // An edge being pulled goes to the edge of a window, when
                // it comes near one, unless Ctrl says not.
                // Or, away from any, to where what is on the screen
                // changes, along as much of the edge as the selection has.
                let (shot, (width, height)) = (&self.shot.pixels, (self.shot.width, self.shot.height));
                let near = |edges: &[i32], value: i32, pulled: bool, across: bool, span: (i32, i32)| {
                    if !pulled || mods.ctrl {
                        return value;
                    }
                    match snapped(edges, value, self.reach) {
                        same if same == value => content_edge(shot, width, height, across, value, span, self.reach).unwrap_or(value),
                        moved => moved,
                    }
                };
                let (rows, columns) = ((origin.y, origin.bottom()), (origin.x, origin.right()));
                let left = near(&self.edges.0, origin.x + if pull.0 < 0 { dx } else { 0 }, pull.0 < 0, true, rows);
                let right = near(&self.edges.0, origin.right() + if pull.0 > 0 { dx } else { 0 }, pull.0 > 0, true, rows);
                let top = near(&self.edges.1, origin.y + if pull.1 < 0 { dy } else { 0 }, pull.1 < 0, false, columns);
                let bottom = near(&self.edges.1, origin.bottom() + if pull.1 > 0 { dy } else { 0 }, pull.1 > 0, false, columns);
                // With Alt, as in Figma: the side across from the one
                // pulled goes as far the other way, about the middle.
                let (left, right, top, bottom) = match mods.alt {
                    false => (left, right, top, bottom),
                    true => (
                        if pull.0 > 0 { origin.x - (right - origin.right()) } else { left },
                        if pull.0 < 0 { origin.right() - (left - origin.x) } else { right },
                        if pull.1 > 0 { origin.y - (bottom - origin.bottom()) } else { top },
                        if pull.1 < 0 { origin.bottom() - (top - origin.y) } else { bottom },
                    ),
                };
                // Pulled past the far side, it turns inside out.
                let mut area = Rect::new(left.min(right), top.min(bottom), (right - left).abs().max(1), (bottom - top).abs().max(1));
                // Kept to the shape that was asked for; or, with Shift,
                // to the shape it was. The corner opposite the one held
                // stays where it is, and an edge pulled leaves the
                // middle of the two beside it where that was.
                let (pull, origin) = (*pull, *origin);
                if let (Some(ratio), false) = (self.settings.aspect.or(mods.shift.then_some((origin.w, origin.h))), mods.alt) {
                    // Along one way of the screen: which side of what
                    // stays still it is on, and the room there is.
                    let along = |pull: i32, moving: i32, near: i32, far: i32, least: i32, most: i32| {
                        let middle = (near + far) / 2;
                        match pull {
                            0 => (None, 2 * (middle - least).min(most - middle)),
                            _ => {
                                let still = if pull < 0 { far } else { near };
                                let back = moving < still;
                                (Some((still, back)), if back { still - least } else { most - still })
                            }
                        }
                    };
                    let (across, room_x) = along(pull.0, if pull.0 < 0 { left } else { right }, origin.x, origin.right(), screen.x, screen.right());
                    let (down, room_y) = along(pull.1, if pull.1 < 0 { top } else { bottom }, origin.y, origin.bottom(), screen.y, screen.bottom());
                    let (wide, high) = shaped(ratio, (area.w, area.h), (pull.0 != 0, pull.1 != 0), (room_x, room_y));
                    let place = |side: Option<(i32, bool)>, middle: i32, size: i32| match side {
                        Some((still, true)) => still - size,
                        Some((still, false)) => still,
                        None => middle - size / 2,
                    };
                    area = Rect::new(place(across, origin.x + origin.w / 2, wide), place(down, origin.y + origin.h / 2, high), wide, high);
                }
                self.selection = Some(area.intersect(screen));
                self.dirty = true;
            }
            Drag::Draw { from } => {
                let from = *from;
                let to = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
                if let Some(shape) = self.shapes.last_mut() {
                    match shape.tool {
                        Tool::Pen | Tool::Marker => {
                            let last = shape.points[shape.points.len() - 1];
                            if (last.0 - to.0).hypot(last.1 - to.1) >= 1.5 {
                                shape.points.push(to);
                            }
                        }
                        // Shift: a square, a circle, a line at one of
                        // eight angles. Alt, as in Figma: about where
                        // the drag began, rather than from it.
                        tool => {
                            let end = if mods.shift { shapes::constrained(tool, from, to) } else { to };
                            let start = if mods.alt && tool != Tool::Counter { (2.0 * from.0 - end.0, 2.0 * from.1 - end.1) } else { from };
                            (shape.points[0], shape.points[1]) = (start, end);
                            if tool == Tool::Zoom {
                                shape.place_zoom();
                            }
                        }
                    }
                    shape.touch();
                }
                self.dirty = true;
            }
            Drag::Shape { index, last, moved } => {
                // `last` is where it was taken hold of: how far it has
                // gone is counted from there, and from the shapes as
                // they then were, so that going to a line and coming
                // away from it again leaves nothing behind.
                let (index, by) = (*index, (at.0 - last.0, at.1 - last.1));
                if !std::mem::replace(moved, true) {
                    // All that are picked out go with the one held.
                    let mut going = self.chosen();
                    if !going.contains(&index) {
                        going = vec![index];
                    }
                    self.checkpoint_move(&going);
                    self.moving = going.into_iter().map(|index| (index, self.shapes[index].clone())).collect();
                    // A line moved without what it is stuck to lets go of it.
                    let ids: Vec<u32> = self.moving.iter().map(|(_, shape)| shape.id).collect();
                    for (index, shape) in &mut self.moving {
                        for end in 0..2 {
                            if shape.stuck[end].is_some_and(|(id, ..)| !ids.contains(&id)) {
                                (shape.stuck[end], self.shapes[*index].stuck[end]) = (None, None);
                            }
                        }
                    }
                }
                // Shift keeps it to across or down, whichever it has gone
                // further, as Figma does.
                let by = if mods.shift { if by.0.abs() >= by.1.abs() { (by.0, 0) } else { (0, by.1) } } else { by };
                let (mut by, mut guides, mut gaps) = (by, Vec::new(), Vec::new());
                if !mods.ctrl {
                    // The box round what is moving, where it would be;
                    // and the edges and middles of everything else, and
                    // of the selection, for its own to go to; or where it
                    // is spaced evenly with the rest, as on the canvas.
                    let mut around: Option<Rect> = None;
                    for (_, shape) in &self.moving {
                        let place = shape.bounds(&mut self.text).moved(by.0, by.1);
                        around = Some(around.map_or(place, |all| Rect::spanning((all.x.min(place.x), all.y.min(place.y)), (all.right().max(place.right()), all.bottom().max(place.bottom())))));
                    }
                    let held: Vec<usize> = self.moving.iter().map(|(index, _)| *index).collect();
                    let edges = |rect: Rect| [rect.x as f32, rect.y as f32, rect.right() as f32, rect.bottom() as f32];
                    let others: Vec<spacing::Edges> = (0..self.shapes.len()).filter(|index| !held.contains(index)).map(|index| edges(self.shapes[index].bounds(&mut self.text))).collect();
                    let (mut across, mut down) = spacing::lines_of(&others);
                    let (picture_across, picture_down) = spacing::lines_of(&self.selection.map(edges).into_iter().collect::<Vec<_>>());
                    across.extend(picture_across);
                    down.extend(picture_down);
                    if let Some(around) = around {
                        let snapped = spacing::snap_box(edges(around), &across, &down, &others, 6.0);
                        // Kept to one way by Shift, it is not taken the other.
                        let (free_x, free_y) = (!mods.shift || by.1 == 0, !mods.shift || by.0 == 0);
                        if free_x {
                            by.0 += snapped.by.0.round() as i32;
                        }
                        if free_y {
                            by.1 += snapped.by.1.round() as i32;
                        }
                        guides = snapped.guides.into_iter().filter(|&(upright, _)| if upright { free_x } else { free_y }).map(|(upright, line)| (upright, line.round() as i32)).collect();
                        gaps = snapped.gaps.into_iter().filter(|gap| if gap.down { free_y } else { free_x }).collect();
                    }
                }
                for (index, before) in &self.moving {
                    let mut shape = before.clone();
                    shape.translate(by.0 as f32, by.1 as f32);
                    self.shapes[*index] = shape;
                }
                (self.guides, self.gaps) = (guides, gaps);
                self.dirty = true;
            }
        }
        // The selection, being put in place, lined up with what else is
        // on the screen along its edges: the sides that are moving, of
        // all of it when all of it is.
        let sides = match self.drag {
            // Dragged out, every side is being put in place: the ones the
            // drag began at, by eye, as much as the ones at the pointer.
            Drag::Select { begun: true, .. } => Some(([true; 4], false)),
            Drag::Resize { pull, .. } => Some(([pull.0 < 0, pull.0 > 0, pull.1 < 0, pull.1 > 0], false)),
            // Kept to one way by Shift, or drawn about its middle by Alt,
            // it is not taken anywhere else.
            Drag::Move { .. } if !mods.shift => Some(([true; 4], true)),
            _ => None,
        };
        let sides = sides.filter(|_| !mods.alt);
        self.lines.clear();
        if let (Some((moving, whole)), false) = (sides, mods.ctrl) {
            // Kept to a shape, it is only said where it lines up: going
            // to the line would take it out of its shape.
            let kept = self.settings.aspect.is_some() || (mods.shift && !whole);
            self.align(moving, whole, kept);
        }
    }

    /// Takes the sides of the selection that are moving to the edges of
    /// things elsewhere on the screen they come near, along the lines
    /// they are on, and says where they now line up. A side that is on
    /// an edge of its own already stays on it; all of the selection,
    /// when all of it is moving, goes by whichever side is nearer.
    /// `kept` goes nowhere, and says where it lines up as it stands.
    fn align(&mut self, moving: [bool; 4], whole: bool, kept: bool) {
        let Some(mut area) = self.selection else { return };
        let (pixels, (width, height)) = (&self.shot.pixels, (self.shot.width, self.shot.height));
        let least = (ui_px(self.monitor().h) as i32 * 2).max(16);
        for across in [true, false] {
            let (sides, edges, span) = match across {
                true => ([(0, area.x), (1, area.right())], &self.edges.0, (area.y, area.bottom())),
                false => ([(2, area.y), (3, area.bottom())], &self.edges.1, (area.x, area.right())),
            };
            // Each moving side, and where it would go: on an edge of
            // its own already, or kept, only where it is.
            let found: Vec<(usize, i32, i32, i32)> = sides
                .into_iter()
                .filter(|&(index, _)| moving[index])
                .filter_map(|(index, side)| {
                    // A window's edge is counted by the pixel either side
                    // of it, as the selection is put on one: by the last
                    // pixel taken, at its right and its bottom.
                    let sitting = kept || edges.contains(&side) || edges.contains(&(side - 1)) || content_edge(pixels, width, height, across, side, span, 0) == Some(side);
                    let reach = if sitting { 0 } else { self.reach };
                    aligned_edge(pixels, width, height, across, side, span, reach, least).map(|(edge, start, end)| (index, edge - side, start, end))
                })
                .collect();
            let Some(&(_, by, _, _)) = found.iter().min_by_key(|(_, by, _, _)| by.abs()) else { continue };
            for &(index, moved, start, end) in &found {
                // All of it moves by the nearer side's, and lines up by
                // that side; a side alone moves by its own.
                if whole && moved != by {
                    continue;
                }
                match index {
                    0 if !whole => area = Rect::new(area.x + moved, area.y, area.w - moved, area.h),
                    1 if !whole => area.w += moved,
                    2 if !whole => area = Rect::new(area.x, area.y + moved, area.w, area.h - moved),
                    3 if !whole => area.h += moved,
                    _ => {}
                }
                let at = if index % 2 == 0 { if across { area.x } else { area.y } } else if across { area.right() } else { area.bottom() };
                let at = if whole { at + by } else { at };
                let (along_start, along_end) = if across { (area.y, area.bottom()) } else { (area.x, area.right()) };
                self.lines.push((across, at, start.min(along_start), end.max(along_end)));
            }
            if whole {
                area = if across { area.moved(by, 0) } else { area.moved(0, by) };
            }
        }
        if area.w > 0 && area.h > 0 {
            self.selection = Some(area.intersect(self.screen()));
        }
    }

    /// A press of the left button, at the time the display gives it.
    pub fn pressed(&mut self, time: u32) -> Outcome {
        let at = self.pointer;
        let double = time.wrapping_sub(self.last_press.0) < DOUBLE_CLICK && (at.0 - self.last_press.1 .0).abs().max((at.1 - self.last_press.1 .1).abs()) <= DRAG_AFTER;
        self.last_press = (time, at);
        self.dirty = true;
        let target = self.target_at(at);
        self.hover = target;
        // A click anywhere but on it leaves a color being typed.
        if target != Target::Hex {
            self.hex = None;
        }

        if self.pipette && matches!(target, Target::Inside | Target::Outside | Target::Handle(..) | Target::Grip(..) | Target::Rounder(..) | Target::Curve(..) | Target::End(_)) {
            self.pipette = false;
            self.set_color(self.shot.pixels[(at.1 * self.shot.width + at.0) as usize] & 0xFFFFFF);
            return Outcome::Continue;
        }
        match target {
            Target::Swatch(color) => {
                self.set_color(color);
                self.popup = None;
            }
            // How opaque: for what is picked out, and what is drawn after.
            Target::Opacity(opacity) => {
                self.opacity = opacity;
                let chosen = self.chosen();
                if !chosen.is_empty() {
                    self.checkpoint();
                    chosen.iter().for_each(|&index| self.shapes[index].opacity = opacity);
                }
            }
            // The colors for the border of a box, or its fill: the square
            // and the strip start at the one it has.
            Target::Paint(paint) => {
                self.paint = paint;
                self.hsv = to_hsv(self.paint_now().unwrap_or(self.color));
            }
            Target::NoPaint => self.no_paint(),
            Target::Pipette => (self.pipette, self.popup) = (true, None),
            Target::Hex => self.hex = Some(String::new()),
            // Any color at all: by the square and the strip, the color
            // following the pointer as it is dragged. One step to undo.
            Target::Square | Target::Hues => {
                if !self.chosen().is_empty() {
                    self.checkpoint();
                }
                self.drag = Drag::Pick { hue: target == Target::Hues };
                self.pick(target == Target::Hues, at);
            }
            Target::Panel => {}
            Target::Cell(cell) => self.choose(cell),
            Target::Button(button) => self.drag = Drag::Button(button),
            // A click anywhere else puts the box of colors away, and
            // does nothing more.
            _ if self.popup.is_some() => self.popup = None,
            _ if self.flyout.is_some() => self.flyout = None,
            Target::End(point) => {
                if let Some(index) = self.picked() {
                    self.checkpoint();
                    self.drag = Drag::End { index, point, from: at, moved: false };
                }
            }
            Target::Grip(across, down) => {
                if let Some(index) = self.picked() {
                    self.checkpoint();
                    self.scaling = Some(self.shapes[index].clone());
                    self.drag = Drag::Scale { index, pull: (across, down), from: at, origin: self.local_frame_of(index) };
                }
            }
            Target::Turn => {
                if let Some(index) = self.picked() {
                    self.checkpoint();
                    self.scaling = Some(self.shapes[index].clone());
                    let about = self.pivot_of(index);
                    self.drag = Drag::Turn { index, about, began: (at.1 as f32 - about.1).atan2(at.0 as f32 - about.0) };
                }
            }
            // As with a rectangle's: moved by how far it goes.
            Target::Curve(across, down) => {
                self.commit_text();
                self.drag = Drag::Curve { corner: (across, down), slack: self.inward((across, down), at) - self.corner_radius() };
            }
            Target::Rounder(across, down) => {
                if let Some(index) = self.picked() {
                    self.checkpoint();
                    // The dot is never right in the corner, even when
                    // the corner is square: it is moved by how far it
                    // goes, not put where the pointer is.
                    let (left, top, width, height) = self.shapes[index].frame();
                    let inward = (if across < 0 { at.0 as f32 - left } else { left + width - at.0 as f32 }).min(if down < 0 { at.1 as f32 - top } else { top + height - at.1 as f32 });
                    self.drag = Drag::Round { index, corner: (across, down), slack: inward - self.shapes[index].radius };
                }
            }
            Target::Handle(across, down) => {
                self.commit_text();
                if let Some(origin) = self.selection {
                    self.drag = Drag::Resize { pull: (across, down), from: at, origin };
                }
            }
            Target::Outside => {
                self.commit_text();
                self.drag = Drag::Select { from: at, begun: false, before: self.selection };
                // An area of a set size is put down round the pointer
                // there and then, and a drag takes it elsewhere.
                if let Some((wide, high)) = self.settings.fixed {
                    let screen = self.screen();
                    let origin = Rect::new(at.0 - wide / 2, at.1 - high / 2, wide.min(screen.w), high.min(screen.h)).kept_within(screen);
                    self.selection = Some(origin);
                    (self.window, self.popup) = (None, None);
                    self.selected.clear();
                    self.drag = Drag::Move { from: at, origin };
                }
            }
            // On the frame: a drag makes it wider or narrower, and a
            // click opens what there is to choose about it.
            Target::Backdrop => {
                self.commit_text();
                let (out, room) = (self.outward(at), self.frame_room());
                self.drag = Drag::Room { from: at, slack: (out.0 - room.0, out.1 - room.1), sides: (out.0 > 0, out.1 > 0), moved: false, opens: true, shape: self.frame_shape() };
            }
            // A handle on the edge of the frame: the frame is made
            // larger or smaller that way, across, down, or both.
            Target::FrameGrip(across, down) => {
                self.commit_text();
                let (out, room) = (self.outward(at), self.frame_room());
                self.drag = Drag::Room { from: at, slack: (out.0 - room.0, out.1 - room.1), sides: (across != 0, down != 0), moved: false, opens: false, shape: self.frame_shape() };
            }
            Target::FrameCorner => {
                let rect = self.frame_rect();
                let radius = (self.frame.radius as f32 * self.scale()).round() as i32;
                self.drag = Drag::FrameCorner { slack: (at.0 - rect.x).min(at.1 - rect.y) - radius };
            }
            Target::Inside => return self.pressed_inside(at, double),
        }
        Outcome::Continue
    }

    fn pressed_inside(&mut self, at: (i32, i32), double: bool) -> Outcome {
        // A click on the text being typed puts the caret there.
        if let Some((index, _)) = self.editing {
            if self.shapes[index].hit(at.0, at.1, &mut self.text) {
                self.caret = self.caret_at(index, at);
                return Outcome::Continue;
            }
            self.commit_text();
        }
        let point = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
        // A shape is got hold of by its line; one already picked out, by
        // anywhere in its box, hollow as it may be.
        let held = match self.shape_at(at) {
            Some(index) => Some(index),
            None => self.chosen().into_iter().find(|&index| self.frame_of(index).contains(at.0, at.1)),
        };
        match self.tool {
            Tool::Select => match held {
                // With Shift or Ctrl, a click adds a shape to those
                // picked out, or takes it from them.
                Some(index) if self.mods.shift || self.mods.ctrl => match self.selected.iter().position(|&other| other == index) {
                    Some(at) => drop(self.selected.remove(at)),
                    None => self.selected.push(index),
                },
                Some(index) => {
                    // A click on one of several picked out leaves them
                    // all picked out, to be moved together.
                    if !self.selected.contains(&index) {
                        self.selected = vec![index];
                    }
                    match double && matches!(self.shapes[index].tool, Tool::Text | Tool::Badge) {
                        true => self.edit_text(index),
                        false => self.drag = Drag::Shape { index, last: at, moved: false },
                    }
                }
                // With Shift or Ctrl and no shape under the pointer, a
                // box is dragged out, and what it touches is added.
                None if self.mods.shift || self.mods.ctrl => self.drag = Drag::Marquee { from: at },
                None if double => return self.finish(self.settings.enter),
                None => {
                    self.selected.clear();
                    if let Some(origin) = self.selection {
                        self.drag = Drag::Move { from: at, origin };
                    }
                }
            },
            Tool::Text => match self.shape_at(at).filter(|&index| self.shapes[index].tool == Tool::Text) {
                Some(index) => self.edit_text(index),
                None => {
                    self.checkpoint();
                    let font_px = self.font_px();
                    let mut shape = Shape::new(Tool::Text, self.color, self.stroke(), (at.0 as f32, (at.1 - Text::line_height(font_px) / 2) as f32));
                    shape.font_px = font_px;
                    shape.face = self.face;
                    (shape.style, shape.opacity) = (self.style, self.opacity);
                    self.shapes.push(shape);
                    self.caret = 0;
                    self.editing = Some((self.shapes.len() - 1, true));
                    self.selected.clear();
                }
            },
            tool => {
                self.checkpoint();
                let mut shape = Shape::new(tool, self.color, self.stroke(), point);
                shape.filled = if tool == Tool::Zoom { self.square } else { self.filled };
                if paintable(tool) {
                    (shape.fill, shape.border) = (self.fill, self.bordered);
                }
                shape.opacity = self.opacity;
                shape.radius = self.settings.radius * self.scale();
                shape.font_px = self.font_px();
                let line = matches!(tool, Tool::Arrow | Tool::Line);
                match tool {
                    Tool::Arrow => shape.heads = self.heads,
                    Tool::Line => shape.heads = self.line_heads,
                    _ => {}
                }
                (shape.dashed, shape.dotted, shape.halo, shape.elbow) = (self.dashed && line, self.dotted && line, self.halo && line, self.elbow && line);
                (shape.ends, shape.sketch) = (self.ends, self.sketch && (line || paintable(tool)));
                shape.number = self.shapes.iter().filter(|shape| shape.tool == Tool::Counter).count() as u32 + 1;
                self.shapes.push(shape);
                self.selected.clear();
                self.drag = Drag::Draw { from: point };
            }
        }
        Outcome::Continue
    }

    /// The left button let go.
    pub fn released(&mut self) -> Outcome {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        self.lines.clear();
        self.dirty = true;
        let mut outcome = Outcome::Continue;
        match drag {
            // A click with nothing dragged out takes the window under it.
            Drag::Select { begun: false, .. } => {
                if let (None, Some(window)) = (self.selection, self.window.take()) {
                    self.selection = Some(window);
                }
            }
            Drag::Select { before, .. } => {
                if self.selection.is_some_and(|area| area.w < DRAG_AFTER && area.h < DRAG_AFTER) {
                    self.selection = before;
                }
            }
            Drag::Draw { .. } => {
                if self.shapes.last().is_some_and(Shape::is_slight) {
                    self.shapes.pop();
                    self.undo.pop();
                } else if !matches!(self.tool, Tool::Pen | Tool::Marker | Tool::Counter) {
                    // What has just been drawn is picked out, with the
                    // move tool in hand, to be put right at once. Not so
                    // a stroke of the pen, of which there are usually
                    // more to come, nor a counter, which is for putting
                    // down 1, 2, 3 one after another.
                    self.made(self.shapes.len() - 1);
                    // Its ends stuck to what they were put down on.
                    self.stick(self.shapes.len() - 1, &[0, 1]);
                }
            }
            Drag::Scale { .. } | Drag::Turn { .. } => self.scaling = None,
            Drag::End { index, point, moved: true, .. } if point < 2 => self.stick(index, &[point]),
            Drag::Shape { .. } => (self.moving, self.guides, self.gaps) = (Vec::new(), Vec::new(), Vec::new()),
            // A click on the frame, not a drag of it, opens what there
            // is to choose about it.
            Drag::Room { moved: false, opens: true, .. } => outcome = self.activate(Button::Frame),
            // A click on an end of a line or an arrow, not a drag of it,
            // gives that end an arrow's head, or takes it away; and the
            // arrows drawn after have theirs where this one now does.
            Drag::End { index, point, moved: false, .. } => {
                let shape = &mut self.shapes[index];
                // A click on the dot a line is bent by straightens it.
                if point == 2 && shape.bend().is_some() {
                    let (a, b) = (shape.points[0], shape.points[1]);
                    shape.bend_to(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
                } else if point == 2 {
                    self.undo.pop();
                } else if matches!(shape.tool, Tool::Arrow | Tool::Line) {
                    shape.heads[point] = !shape.heads[point];
                    shape.touch();
                    if shape.tool == Tool::Arrow && shape.heads != [false, false] {
                        self.heads = shape.heads;
                    }
                } else {
                    self.undo.pop();
                }
            }
            // A color picked is one taken lately, once let go of.
            Drag::Pick { .. } => self.taken_lately(self.paint_now().unwrap_or(self.color)),
            Drag::Marquee { from } => {
                let reach = Rect::spanning(from, self.pointer);
                for index in 0..self.shapes.len() {
                    if !self.selected.contains(&index) && self.shapes[index].bounds(&mut self.text).intersects(reach) {
                        self.selected.push(index);
                    }
                }
            }
            Drag::Button(button) if self.target_at(self.pointer) == Target::Button(button) => outcome = self.activate(button),
            _ => {}
        }
        self.hover = self.target_at(self.pointer);
        outcome
    }

    /// The right button: the colors, where the pointer is; or with
    /// nothing selected yet, a way out.
    pub fn right_pressed(&mut self) -> Outcome {
        self.dirty = true;
        if self.drag != Drag::None {
            return Outcome::Continue;
        }
        if self.selection.is_none() {
            return Outcome::Cancel;
        }
        self.commit_text();
        self.popup = match self.popup {
            Some(_) => None,
            None => Some(Anchor::At(self.pointer.0, self.pointer.1)),
        };
        Outcome::Continue
    }

    /// The wheel: a thicker line, or a thinner.
    pub fn scrolled(&mut self, up: bool) {
        // With the library open, it is the emoji that go up and down.
        if self.flyout == Some(Button::Badges) {
            let rows = (self.emoji_found().len() as i32 + EMOJI_ACROSS - 1) / EMOJI_ACROSS;
            self.scroll = (self.scroll + if up { -2 } else { 2 }).clamp(0, (rows - EMOJI_DOWN).max(0));
            self.dirty = true;
            return;
        }
        if self.selection.is_some() {
            let size = if up { (self.size + 1).min(STROKES.len() - 1) } else { self.size.saturating_sub(1) };
            self.set_size(size);
        }
    }

    // ---- The keys ----

    pub fn key(&mut self, key: Key, mods: Mods) -> Outcome {
        self.dirty = true;
        self.mods = mods;
        // With the library open, what is typed is what is looked for
        // among the names of all that is in it; Enter takes the first
        // found, and the arrows go up and down the emoji.
        if self.flyout == Some(Button::Badges) && !mods.ctrl {
            match key {
                Key::Char(letter) => (self.query, self.scroll) = (format!("{}{letter}", self.query), 0),
                Key::Backspace => (self.scroll, _) = (0, self.query.pop()),
                Key::Up => self.scrolled(true),
                Key::Down => self.scrolled(false),
                Key::Enter => match self.library_found().first() {
                    Some(&first) if !self.query.is_empty() || self.shelf != EMOJI_SHELF => self.choose(Cell::Badge(first)),
                    _ => self.choose(Cell::Emoji(0)),
                },
                Key::Escape => self.flyout = None,
                _ => {}
            }
            return Outcome::Continue;
        }
        // With the colors open, # or a click on the hex begins a color
        // typed: three or six of its digits, and Enter takes it.
        if self.popup.is_none() {
            self.hex = None;
        } else if self.hex.is_none() && key == Key::Char('#') && !mods.ctrl {
            self.hex = Some(String::new());
            return Outcome::Continue;
        }
        if let (Some(typed), false) = (&mut self.hex, mods.ctrl) {
            match key {
                Key::Char(digit) if digit.is_ascii_hexdigit() && typed.len() < 6 => typed.push(digit.to_ascii_uppercase()),
                Key::Backspace => _ = typed.pop(),
                Key::Escape => self.hex = None,
                Key::Enter => {
                    // Three digits are each said twice: F80 is FF8800.
                    let digits: String = match typed.len() {
                        3 => typed.chars().flat_map(|digit| [digit, digit]).collect(),
                        _ => typed.clone(),
                    };
                    if let (6, Ok(color)) = (digits.len(), u32::from_str_radix(&digits, 16)) {
                        self.set_color(color);
                        self.taken_lately(color);
                        self.hex = None;
                    }
                }
                _ => {}
            }
            return Outcome::Continue;
        }
        // While text is being typed, Enter is a new line of it; Ctrl+Enter,
        // or Esc, is the end of it, and takes no picture.
        if let (Some(_), Key::Enter, true) = (self.editing, key, mods.ctrl) {
            self.commit_text();
            return Outcome::Continue;
        }
        // And Ctrl+V is whatever text is on the clipboard, which has to
        // be asked of the display.
        if let (Some(_), Key::Char('v' | 'V'), true) = (self.editing, key, mods.ctrl) {
            return Outcome::Paste;
        }
        // Ctrl+A is all of the text picked out: what is typed next, or
        // pasted, takes its place, and Backspace or Delete leaves none.
        if let (Some(_), Key::Char('a' | 'A'), true) = (self.editing, key, mods.ctrl) {
            self.all = true;
            return Outcome::Continue;
        }
        // Shift going down, on its way to a capital letter, is a key
        // too, and is not what takes the place of what is picked out.
        if let (Some((index, _)), false, true, false) = (self.editing, mods.ctrl, self.all, key == Key::Other) {
            self.all = false;
            // Enter ends a badge, which is one line, and takes nothing
            // from it.
            let ends = key == Key::Enter && self.shapes[index].tool == Tool::Badge;
            if matches!(key, Key::Char(_) | Key::Enter | Key::Backspace | Key::Delete) && !ends {
                self.shapes[index].text.clear();
                self.caret = 0;
                if matches!(key, Key::Backspace | Key::Delete) {
                    return Outcome::Continue;
                }
            }
        }
        if let (Some((index, _)), false) = (self.editing, mods.ctrl) {
            let badge = self.shapes[index].tool == Tool::Badge;
            let text = &mut self.shapes[index].text;
            let caret = self.caret.min(text.len());
            // The line the caret is on, and how many letters along it.
            let start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
            let end = text[caret..].find('\n').map_or(text.len(), |at| caret + at);
            let along = text[start..caret].chars().count();
            // Where that many letters along another line is, or its end.
            let place = |line: &str, from: usize| from + line.char_indices().nth(along).map_or(line.len(), |(at, _)| at);
            match key {
                Key::Char(letter) => {
                    text.insert(caret, letter);
                    self.caret = caret + letter.len_utf8();
                }
                // A badge is one line: Enter is the end of it.
                Key::Enter if badge => self.commit_text(),
                Key::Enter => {
                    text.insert(caret, '\n');
                    self.caret = caret + 1;
                }
                Key::Backspace => {
                    if let Some((at, _)) = text[..caret].char_indices().next_back() {
                        text.remove(at);
                        self.caret = at;
                    }
                }
                Key::Delete if caret < text.len() => drop(text.remove(caret)),
                Key::Left => self.caret = text[..caret].char_indices().next_back().map_or(0, |(at, _)| at),
                Key::Right => self.caret = text[caret..].chars().next().map_or(caret, |letter| caret + letter.len_utf8()),
                Key::Home => self.caret = start,
                Key::End => self.caret = end,
                Key::Up if start > 0 => {
                    let above = text[..start - 1].rfind('\n').map_or(0, |at| at + 1);
                    self.caret = place(&text[above..start - 1], above);
                }
                Key::Down if end < text.len() => {
                    let below = text[end + 1..].find('\n').map_or(text.len(), |at| end + 1 + at);
                    self.caret = place(&text[end + 1..below], end + 1);
                }
                Key::Escape => self.commit_text(),
                _ => {}
            }
            return Outcome::Continue;
        }
        let key = match key {
            Key::Char(letter) => Key::Char(letter.to_ascii_lowercase()),
            other => other,
        };
        match (key, mods.ctrl) {
            (Key::Escape, _) => {
                if self.popup.take().is_none() && self.flyout.take().is_none() && !std::mem::take(&mut self.pipette) {
                    return Outcome::Cancel;
                }
            }
            // With nothing selected, Enter takes the screen the pointer is on.
            (Key::Enter, _) => match self.selection {
                Some(_) => return self.finish(self.settings.enter),
                None => self.select(self.monitor_at(self.pointer)),
            },
            (Key::Char('c'), true) => return self.finish(Finish { copy: true, ..Finish::NONE }),
            (Key::Char('s'), true) => return self.finish(Finish { save: true, ..Finish::NONE }),
            (Key::Char('p'), true) => return self.finish(Finish { pin: true, ..Finish::NONE }),
            (Key::Char('r'), true) => return self.finish(Finish { record: true, ..Finish::NONE }),
            (Key::Char('e'), true) => return self.finish(Finish { canvas: true, ..Finish::NONE }),
            (Key::Char('z'), true) if mods.shift => self.redo(),
            (Key::Char('z'), true) => self.undo(),
            (Key::Char('y'), true) => self.redo(),
            (Key::Char('a'), true) => self.select(self.screen()),
            // Heavy and slanted, for the text picked out.
            (Key::Char('b'), true) => self.toggle_style(shapes::BOLD),
            (Key::Char('i'), true) => self.toggle_style(shapes::ITALIC),
            (Key::Char('d'), true) => self.duplicate(),
            // What is picked out, over the shape above it or under the
            // one below; with Shift, over or under them all.
            (Key::Char(']'), true) => self.restack(true, mods.shift),
            (Key::Char('['), true) => self.restack(false, mods.shift),
            (Key::Char('}'), true) => self.restack(true, true),
            (Key::Char('{'), true) => self.restack(false, true),
            (Key::Delete | Key::Backspace, _) => self.delete(),
            (Key::Left, ctrl) => self.nudge(-1, 0, ctrl),
            (Key::Right, ctrl) => self.nudge(1, 0, ctrl),
            (Key::Up, ctrl) => self.nudge(0, -1, ctrl),
            (Key::Down, ctrl) => self.nudge(0, 1, ctrl),
            // The loupe, up or down, whether or not anything is selected.
            (Key::Char('x'), false) => self.loupe = !self.loupe,
            (Key::Char(letter), false) if self.selection.is_some() && !self.settings.recording => match letter {
                'v' => self.set_tool(Tool::Select),
                'a' => self.set_tool(Tool::Arrow),
                'l' => self.set_tool(Tool::Line),
                'r' => self.set_tool(Tool::Rect),
                'o' => self.set_tool(Tool::Ellipse),
                'p' => self.set_tool(Tool::Pen),
                'h' => self.set_tool(Tool::Marker),
                'e' => drop(self.activate(Button::Emoji)),
                't' => self.set_tool(Tool::Text),
                'n' => self.set_tool(Tool::Counter),
                'b' => self.set_tool(Tool::Blur),
                's' => self.set_tool(Tool::Spotlight),
                'c' => drop(self.activate(Button::Color)),
                'f' => self.toggle_fill(),
                'g' => drop(self.activate(Button::Face)),
                'z' => self.set_tool(Tool::Zoom),
                'j' => drop(self.activate(Button::Badges)),
                'w' => drop(self.activate(Button::Frame)),
                'm' => drop(self.activate(Button::Pointer)),
                'd' => self.toggle_dashed(),
                // From each roundness of the picture's corners to the
                // next, and round again to square; one that is none of
                // them, set by hand, goes to square first.
                'k' => {
                    let at = CORNERS.iter().position(|&radius| radius == self.corners);
                    self.corners = at.map_or(0.0, |at| CORNERS[(at + 1) % CORNERS.len()]);
                }
                'i' => (self.pipette, self.popup) = (!self.pipette, None),
                '1'..='4' => self.set_size(letter as usize - '1' as usize),
                '[' => self.scrolled(false),
                ']' => self.scrolled(true),
                _ => {}
            },
            _ => {}
        }
        self.hover = self.target_at(self.pointer);
        Outcome::Continue
    }

    /// Puts the shapes picked out over the one above them, or under the
    /// one below: or, with `all`, over or under every other. Those that
    /// are picked out keep their order among themselves.
    fn restack(&mut self, forward: bool, all: bool) {
        let mut chosen = self.chosen();
        chosen.sort_unstable();
        if chosen.is_empty() || chosen.len() == self.shapes.len() {
            return;
        }
        self.checkpoint();
        let count = self.shapes.len();
        // Where each shape is to go, by where it is now.
        let mut order: Vec<usize> = (0..count).collect();
        let picked = |index: usize| chosen.binary_search(&index).is_ok();
        if all {
            order.sort_by_key(|&index| picked(index) == forward);
        } else if forward {
            for at in (0..count - 1).rev() {
                if picked(order[at]) && !picked(order[at + 1]) {
                    order.swap(at, at + 1);
                }
            }
        } else {
            for at in 1..count {
                if picked(order[at]) && !picked(order[at - 1]) {
                    order.swap(at, at - 1);
                }
            }
        }
        let mut shapes: Vec<Option<Shape>> = std::mem::take(&mut self.shapes).into_iter().map(Some).collect();
        self.shapes = order.iter().map(|&index| shapes[index].take().expect("each shape goes to one place")).collect();
        self.selected = order.iter().enumerate().filter(|(_, &index)| picked(index)).map(|(at, _)| at).collect();
    }

    /// The arrow keys: the shape picked out goes a pixel that way, or ten
    /// with Shift; otherwise the selection does, or with Ctrl grows.
    fn nudge(&mut self, dx: i32, dy: i32, grow: bool) {
        let step = if self.mods.shift { 10 } else { 1 };
        let (dx, dy) = (dx * step, dy * step);
        let chosen = self.chosen();
        if !chosen.is_empty() {
            self.checkpoint_move(&chosen);
            chosen.iter().for_each(|&index| self.shapes[index].translate(dx as f32, dy as f32));
        } else if let Some(area) = self.selection {
            let screen = self.screen();
            self.selection = Some(match grow {
                true => Rect::new(area.x, area.y, (area.w + dx).max(1), (area.h + dy).max(1)).intersect(screen),
                false => area.moved(dx, dy).kept_within(screen),
            });
        }
    }

    // ---- What the buttons and keys do ----

    fn activate(&mut self, button: Button) -> Outcome {
        self.dirty = true;
        match button {
            Button::Tool(tool) => self.set_tool(tool),
            // The colors, for a line or a border, or for a fill: opened
            // at the color there is now; and the same button again, or
            // the button of the one they are open for, puts them away.
            Button::Color | Button::Border | Button::Fill => {
                let paint = if button == Button::Fill { Paint::Fill } else { Paint::Line };
                match (self.popup, paint == self.paint) {
                    (Some(_), true) => self.popup = None,
                    _ => {
                        (self.paint, self.flyout, self.popup) = (paint, None, Some(Anchor::Bar));
                        self.hsv = to_hsv(self.paint_now().unwrap_or(self.color));
                    }
                }
            }
            Button::Width => {
                self.popup = None;
                self.flyout = if self.flyout == Some(button) { None } else { Some(button) };
            }
            Button::Style(style) => self.toggle_style(style),
            Button::Start | Button::End | Button::Stroke => {
                self.popup = None;
                self.flyout = if self.flyout == Some(button) { None } else { Some(button) };
            }
            Button::Path => self.toggle_elbow(),
            Button::Sketch => self.toggle_sketch(),
            Button::Lens => self.toggle_lens(),
            Button::Forward => self.restack(true, self.mods.shift),
            Button::Backward => self.restack(false, self.mods.shift),
            Button::Duplicate => self.duplicate(),
            Button::Delete => self.delete(),
            Button::Pointer => self.arrow_shows = !self.arrow_shows && self.arrow.is_some(),
            Button::Pin => return self.finish(Finish { pin: true, ..Finish::NONE }),
            Button::Record => return self.finish(Finish { record: true, ..Finish::NONE }),
            // The next face, for the text being typed or picked out, and
            // for what is typed after.
            Button::Face => {
                self.face = (self.shown().map_or(self.face, |shape| shape.face) + 1) % FACES.len();
                let texts: Vec<usize> = match self.editing {
                    Some((index, _)) => vec![index],
                    None => self.chosen().into_iter().filter(|&index| matches!(self.shapes[index].tool, Tool::Text | Tool::Badge)).collect(),
                };
                if self.editing.is_none() && !texts.is_empty() {
                    self.checkpoint();
                }
                texts.iter().for_each(|&index| self.shapes[index].face = self.face);
            }
            // The library hangs from its own button whichever way it is
            // opened; E opens it on its emoji.
            Button::Badges | Button::Frame | Button::Places | Button::Emoji => {
                (self.query, self.scroll) = (String::new(), 0);
                self.commit_text();
                self.popup = None;
                let (box_of, shelf) = match button {
                    Button::Emoji => (Button::Badges, Some(EMOJI_SHELF)),
                    other => (other, None),
                };
                let open = self.flyout == Some(box_of) && shelf.is_none_or(|shelf| shelf == self.shelf);
                self.shelf = shelf.unwrap_or(self.shelf);
                self.flyout = if open { None } else { Some(box_of) };
            }
            Button::Undo => self.undo(),
            Button::Redo => self.redo(),
            Button::Cancel => return Outcome::Cancel,
            Button::Save => return self.finish(Finish { save: true, ..Finish::NONE }),
            Button::Copy => return self.finish(Finish { copy: true, ..Finish::NONE }),
            Button::Design => return self.finish(Finish { canvas: true, ..Finish::NONE }),
        }
        Outcome::Continue
    }

    fn finish(&mut self, how: Finish) -> Outcome {
        match self.selection {
            Some(_) => {
                self.commit_text();
                Outcome::Finish(how)
            }
            None => Outcome::Continue,
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.commit_text();
        (self.tool, self.pipette, self.popup) = (tool, false, None);
        if tool != Tool::Select {
            self.selected.clear();
        }
    }

    /// The text there is something to say about how it is set: the one
    /// being typed, or the one picked out by itself.
    fn styled(&self) -> Option<usize> {
        self.editing.map(|(index, _)| index).or(self.picked()).filter(|&index| self.shapes[index].tool == Tool::Text)
    }

    /// Sets that text one of the ways, or sets it back; and what is
    /// typed after is set as it then is. With no text to set, with the
    /// text tool in hand, only what is typed next.
    fn toggle_style(&mut self, style: u8) {
        self.dirty = true;
        let Some(index) = self.styled() else {
            if self.tool == Tool::Text && self.chosen().is_empty() {
                self.style ^= style;
            }
            return;
        };
        if self.editing.is_none() {
            self.checkpoint();
        }
        self.shapes[index].style ^= style;
        self.style = self.shapes[index].style;
    }

    /// A color picked in the square, or a hue in the strip, at a point:
    /// the color in hand becomes it, and so do the shapes picked out.
    /// The hue, saturation and value are kept apart from the color, so
    /// that a gray keeps the hue it was made at.
    fn pick(&mut self, hue: bool, (x, y): (i32, i32)) {
        let layout = self.layout();
        let place = if hue { layout.hues } else { layout.square };
        let across = ((x - place.x) as f32 / place.w.max(1) as f32).clamp(0.0, 1.0);
        let (mut h, mut s, mut v) = self.hsv;
        if hue {
            h = across * 360.0;
        } else {
            s = across;
            v = 1.0 - ((y - place.y) as f32 / place.h.max(1) as f32).clamp(0.0, 1.0);
        }
        self.hsv = (h, s, v);
        self.paint_with(from_hsv(self.hsv));
    }

    /// A color taken lately, to the front of those that are kept, and
    /// kept for next time too.
    fn taken_lately(&mut self, color: u32) {
        if PALETTE.iter().any(|&(known, _)| known == color) {
            return;
        }
        self.lately.retain(|&had| had != color);
        self.lately.insert(0, color);
        self.lately.truncate(5);
        if let Some(file) = crate::output::state_file("colors").filter(|_| !cfg!(test)) {
            let words: Vec<String> = self.lately.iter().map(|color| format!("{color:06X}")).collect();
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(file, words.join(" ") + "\n");
        }
    }

    fn set_color(&mut self, color: u32) {
        self.hsv = to_hsv(color);
        if self.editing.is_none() && !self.chosen().is_empty() {
            self.checkpoint();
        }
        self.paint_with(color);
    }

    /// A color, for what the box of colors is for: the line of what is
    /// drawn, or of the text being typed; or the border of a box or an
    /// oval, which it then has, or its fill. Given to what is picked
    /// out, and to what is drawn after.
    fn paint_with(&mut self, color: u32) {
        let fill = self.paint == Paint::Fill && self.paints_apart();
        match fill {
            true => (self.fill, self.filled) = (color, true),
            false => {
                self.color = color;
                self.bordered |= self.paints_apart();
            }
        }
        if let Some((index, _)) = self.editing {
            self.shapes[index].color = color;
        }
        let apart = self.paints_apart();
        for index in self.chosen() {
            let shape = &mut self.shapes[index];
            // A box or an oval with only a fill, given a color among
            // shapes of other kinds, is filled with it, as it always was.
            let fills = paintable(shape.tool) && (fill || (!apart && !shape.border));
            match fills {
                true => (shape.fill, shape.filled) = (color, true),
                false if paintable(shape.tool) => (shape.color, shape.border) = (color, true),
                false => shape.color = color,
            }
            shape.touch();
        }
        self.dirty = true;
    }

    /// None, in the box of colors of a box or an oval: no border, or no
    /// fill. One left with neither has the other instead, in the color
    /// it had: there would be nothing of it.
    fn no_paint(&mut self) {
        let fill = self.paint == Paint::Fill;
        let chosen: Vec<usize> = self.chosen().into_iter().filter(|&index| paintable(self.shapes[index].tool)).collect();
        if !chosen.is_empty() {
            self.checkpoint();
        }
        for index in chosen {
            let shape = &mut self.shapes[index];
            match fill {
                true => (shape.filled, shape.border) = (false, true),
                false if shape.filled => shape.border = false,
                false => (shape.border, shape.filled, shape.fill) = (false, true, shape.color),
            }
            shape.touch();
        }
        match fill {
            true => (self.filled, self.bordered) = (false, true),
            false if self.filled => self.bordered = false,
            false => (self.bordered, self.filled, self.fill) = (false, true, self.color),
        }
        self.dirty = true;
    }

    /// F: a fill for the boxes and ovals picked out, or none, and for
    /// those drawn after; in the fill color there was. A zoom picked out
    /// is made square, or round again, which is what there is to fill or
    /// not about a zoom.
    fn toggle_fill(&mut self) {
        if self.chosen().iter().any(|&index| self.shapes[index].tool == Tool::Zoom) {
            return self.toggle_lens();
        }
        let filled = self.shown_fill().is_none();
        let chosen: Vec<usize> = self.chosen().into_iter().filter(|&index| paintable(self.shapes[index].tool)).collect();
        if !chosen.is_empty() {
            self.checkpoint();
        }
        let fill = self.fill;
        for index in chosen {
            let shape = &mut self.shapes[index];
            match filled {
                true => (shape.filled, shape.fill) = (true, fill),
                false => (shape.filled, shape.border) = (false, true),
            }
            shape.touch();
        }
        self.filled = filled;
        self.bordered |= !filled;
        self.dirty = true;
    }

    /// Changes the shapes picked out of the kinds `kinds` says, as one
    /// step to undo: `change` is done to each.
    fn restyle(&mut self, kinds: fn(Tool) -> bool, change: impl Fn(&mut Shape)) {
        let picked: Vec<usize> = self.chosen().into_iter().filter(|&index| kinds(self.shapes[index].tool)).collect();
        if !picked.is_empty() {
            self.checkpoint();
        }
        for index in picked {
            change(&mut self.shapes[index]);
            self.shapes[index].touch();
        }
        self.dirty = true;
    }

    /// The head at one end of the lines and arrows picked out, and of the
    /// next: one of the `shapes::HEADS`, or past them, none.
    fn set_head(&mut self, end: usize, style: u8) {
        let on = usize::from(style) < shapes::HEADS.len();
        match self.shown().map_or(self.tool, |shape| shape.tool) {
            Tool::Line => self.line_heads[end] = on,
            _ => self.heads[end] = on,
        }
        if on {
            self.ends[end] = style;
        }
        self.restyle(lines, |shape| {
            shape.heads[end] = on;
            if on {
                shape.ends[end] = style;
            }
        });
    }

    /// The kind of line of the lines and arrows picked out, and of the
    /// next: solid, dashed or dotted; or, apart from those, an edge of
    /// white round it, or none.
    fn set_line(&mut self, kind: u8) {
        if kind == 3 {
            self.halo = !self.shown().map_or(self.halo, |shape| shape.halo);
            let halo = self.halo;
            return self.restyle(lines, |shape| shape.halo = halo);
        }
        (self.dashed, self.dotted) = (kind == 1, kind == 2);
        self.restyle(lines, |shape| (shape.dashed, shape.dotted) = (kind == 1, kind == 2));
    }

    /// D: dashes, for the lines and arrows picked out and for those drawn
    /// after; or whole lines again.
    fn toggle_dashed(&mut self) {
        let dashed = self.shown().map_or(self.dashed, |shape| shape.dashed && !shape.dotted);
        self.set_line(if dashed { 0 } else { 1 });
    }

    /// Round a corner, or straight again, for the lines and arrows picked
    /// out and the next. A line that bent no longer does: an elbow goes
    /// its own way.
    fn toggle_elbow(&mut self) {
        self.elbow = !self.shown().map_or(self.elbow, |shape| shape.elbow);
        let elbow = self.elbow;
        self.restyle(lines, |shape| {
            shape.elbow = elbow;
            shape.points.truncate(2);
        });
    }

    /// Drawn by hand, or cleanly again: the boxes, ovals, lines and
    /// arrows picked out, and the next.
    fn toggle_sketch(&mut self) {
        self.sketch = !self.shown().map_or(self.sketch, |shape| shape.sketch);
        let sketch = self.sketch;
        self.restyle(|tool| lines(tool) || paintable(tool), |shape| shape.sketch = sketch);
    }

    /// The zooms picked out made square, or round again; and the next.
    fn toggle_lens(&mut self) {
        self.square = !self.shown().map_or(self.square, |shape| shape.filled);
        let zooms: Vec<usize> = self.chosen().into_iter().filter(|&index| self.shapes[index].tool == Tool::Zoom).collect();
        if !zooms.is_empty() {
            self.checkpoint();
        }
        for index in zooms {
            self.shapes[index].filled = self.square;
            self.shapes[index].touch();
        }
        self.dirty = true;
    }

    /// A copy of each shape picked out, a little down and to the right of
    /// it, and picked out in its place.
    /// What the ends of arrows can be stuck to: the boxes, ovals and text.
    fn anchors(&mut self) -> Vec<shapes::Anchor> {
        let mut anchors = Vec::new();
        for shape in &self.shapes {
            anchors.extend(shape.anchor(&mut self.text));
        }
        anchors
    }

    /// Ends of a line stuck to what they are on, as Figma's connectors
    /// are, to go with it when it moves.
    fn stick(&mut self, index: usize, ends: &[usize]) {
        let anchors = self.anchors();
        let Some(shape) = self.shapes.get_mut(index) else { return };
        for &end in ends {
            let at = shape.points[end];
            shape.stick(end, at, &anchors, 8.0);
        }
        self.follow();
    }

    /// The ends of arrows taken to what they are stuck to, where it is now.
    fn follow(&mut self) {
        if self.shapes.iter().any(|shape| shape.stuck.iter().any(Option::is_some)) {
            let anchors = self.anchors();
            shapes::follow(self.shapes.iter_mut(), &anchors);
        }
    }

    /// The box round the shapes called these, if they are all still there.
    fn box_of(&mut self, ids: &[u32]) -> Option<spacing::Edges> {
        let mut all: Option<spacing::Edges> = None;
        for &id in ids {
            let index = self.shapes.iter().position(|shape| shape.id == id)?;
            let place = self.shapes[index].bounds(&mut self.text);
            let (x, y, right, bottom) = (place.x as f32, place.y as f32, place.right() as f32, place.bottom() as f32);
            all = Some(all.map_or([x, y, right, bottom], |was| [was[0].min(x), was[1].min(y), was[2].max(right), was[3].max(bottom)]));
        }
        all
    }

    /// The shapes picked out copied, as Figma copies: the last copies
    /// made, picked out, as far from themselves again as they are from
    /// what they were copied from; anything else beside itself, to the
    /// right or else below, inside the picture where there is room and
    /// past the other shapes. The copies are picked out.
    fn duplicate(&mut self) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        let ids: Vec<u32> = chosen.iter().map(|&index| self.shapes[index].id).collect();
        let sorted = |ids: &[u32]| {
            let mut ids = ids.to_vec();
            ids.sort_unstable();
            ids
        };
        let again = match self.repeat.take() {
            Some((sources, copies)) if sorted(&copies) == sorted(&ids) => self.box_of(&sources).zip(self.box_of(&copies)).map(|(from, to)| (to[0] - from[0], to[1] - from[1])),
            _ => None,
        };
        let (dx, dy) = match again {
            Some(by) => by,
            None => {
                let me = self.box_of(&ids).unwrap_or_default();
                let others: Vec<spacing::Edges> = (0..self.shapes.len())
                    .filter(|index| !chosen.contains(index))
                    .map(|index| {
                        let place = self.shapes[index].bounds(&mut self.text);
                        [place.x as f32, place.y as f32, place.right() as f32, place.bottom() as f32]
                    })
                    .collect();
                let area = self.selection.map(|area| [area.x as f32, area.y as f32, area.right() as f32, area.bottom() as f32]);
                // About 20 apart on a screen 1080 high, more on larger.
                let gap = (ui_px(self.monitor().h) * 3 / 2) as f32;
                spacing::beside(me, &others, area, gap)
            }
        };
        self.checkpoint();
        let mut counters = self.shapes.iter().filter(|shape| shape.tool == Tool::Counter).count() as u32;
        self.selected.clear();
        let mut copies = Vec::new();
        for index in chosen {
            let mut copy = self.shapes[index].copied();
            copy.translate(dx, dy);
            copies.push(copy.id);
            // A counter copied counts on.
            if copy.tool == Tool::Counter {
                counters += 1;
                copy.number = counters;
            }
            self.shapes.push(copy);
            self.selected.push(self.shapes.len() - 1);
        }
        self.repeat = Some((ids, copies));
        self.tool = Tool::Select;
        self.dirty = true;
    }

    /// Away with the shapes picked out.
    fn delete(&mut self) {
        let mut going = self.chosen();
        if going.is_empty() {
            return;
        }
        self.checkpoint();
        // From the last, so that the places of the rest hold.
        going.sort_unstable();
        going.iter().rev().for_each(|&index| drop(self.shapes.remove(index)));
        self.selected.clear();
        // The counters left count from one again, in order.
        for (number, shape) in self.shapes.iter_mut().filter(|shape| shape.tool == Tool::Counter).enumerate() {
            shape.number = number as u32 + 1;
        }
        self.dirty = true;
    }

    // ---- How what is picked out looks ----

    /// The shape whose looks the bar over it shows: the text being
    /// typed, or the first of those picked out. With none, the bar shows
    /// how the next is drawn.
    fn shown(&self) -> Option<&Shape> {
        let index = self.editing.map(|(index, _)| index).or_else(|| self.chosen().first().copied().filter(|_| self.tool == Tool::Select))?;
        self.shapes.get(index)
    }

    /// The line the bar over it shows: the one picked out, or one drawn
    /// as the next would be with the tool in hand.
    fn shown_line(&self) -> Shape {
        if let Some(shape) = self.shown() {
            return shape.clone();
        }
        let mut like = Shape::new(if self.tool == Tool::Line { Tool::Line } else { Tool::Arrow }, self.color, self.stroke(), (0.0, 0.0));
        like.heads = if self.tool == Tool::Line { self.line_heads } else { self.heads };
        (like.ends, like.dashed, like.dotted, like.halo, like.elbow, like.sketch) = (self.ends, self.dashed, self.dotted, self.halo, self.elbow, self.sketch);
        like
    }

    /// Whether what the colors are for is a box or an oval, or are boxes
    /// and ovals, with a border and a fill to color apart: those picked
    /// out, or the next drawn.
    fn paints_apart(&self) -> bool {
        let chosen = self.chosen();
        match (self.editing, chosen.is_empty()) {
            (Some(_), _) => false,
            (None, false) => chosen.iter().all(|&index| paintable(self.shapes[index].tool)),
            (None, true) => paintable(self.tool),
        }
    }

    /// The fill there is, and the border: for what is shown, or the next
    /// box or oval drawn.
    fn shown_fill(&self) -> Option<u32> {
        match self.shown() {
            Some(shape) => (paintable(shape.tool) && shape.filled).then_some(shape.fill),
            None => self.filled.then_some(self.fill),
        }
    }

    fn shown_border(&self) -> Option<u32> {
        match self.shown() {
            Some(shape) => (!paintable(shape.tool) || shape.border).then_some(shape.color),
            None => self.bordered.then_some(self.color),
        }
    }

    /// The color the box of colors is at: that of what it is for, or
    /// none, for a border or a fill there is not.
    fn paint_now(&self) -> Option<u32> {
        match (self.paint, self.paints_apart()) {
            (Paint::Fill, true) => self.shown_fill(),
            (Paint::Line, true) => self.shown_border(),
            _ => Some(self.shown().map_or(self.color, |shape| shape.color)),
        }
    }

    /// Which of the four widths, or sizes of text, what is shown is
    /// nearest; or the one the next is drawn at.
    fn shown_size(&self) -> usize {
        let Some(shape) = self.shown() else { return self.size };
        let scale = self.scale();
        let (sizes, now): (&[f32], f32) = match shape.tool {
            Tool::Text | Tool::Badge => (&FONT_SIZES, shape.font_px as f32),
            _ => (&STROKES, shape.stroke),
        };
        (0..sizes.len()).min_by(|&a, &b| (sizes[a] * scale - now).abs().total_cmp(&(sizes[b] * scale - now).abs())).unwrap_or(self.size)
    }

    /// What the four are called, for lines or for text.
    fn width_names(&self) -> [&'static str; 4] {
        match self.shown().map_or(self.tool, |shape| shape.tool) {
            Tool::Text | Tool::Badge => ["Small", "Medium", "Large", "Largest"],
            _ => ["Thin", "Medium", "Thick", "Heavy"],
        }
    }

    /// What the bar over what is picked out has, in order: None is a line
    /// between groups. How it looks, for the kind of shape it is, or what
    /// shapes of more than one kind all have; and, picked out, over and
    /// under the rest, a copy, and away. With nothing picked out, how the
    /// next is drawn with the tool in hand; and nothing with the move tool.
    fn tray_contents(&self) -> Vec<Option<Button>> {
        if self.settings.recording || self.selection.is_none() {
            return Vec::new();
        }
        let chosen = if self.tool == Tool::Select { self.chosen() } else { Vec::new() };
        let editing = self.editing.map(|(index, _)| index);
        let kinds: Vec<Tool> = match editing {
            Some(index) => vec![self.shapes[index].tool],
            None if !chosen.is_empty() => chosen.iter().map(|&index| self.shapes[index].tool).collect(),
            None => vec![self.tool],
        };
        // Boxes and ovals together are colored alike.
        let kinds: Vec<Tool> = kinds.into_iter().map(|tool| if tool == Tool::Ellipse { Tool::Rect } else { tool }).collect();
        let kind = kinds.iter().all(|&tool| tool == kinds[0]).then_some(kinds[0]);
        let styled = self.styled().is_some() || (editing.is_none() && chosen.is_empty());
        let mut tray: Vec<Option<Button>> = looks(kind, self.shown_border().is_some(), styled).into_iter().map(Some).collect();
        if editing.is_none() && !chosen.is_empty() {
            if !tray.is_empty() {
                tray.push(None);
            }
            tray.extend([Some(Button::Forward), Some(Button::Backward), None, Some(Button::Duplicate), Some(Button::Delete)]);
        }
        tray
    }

    /// What the bar over what is picked out is for: all that is picked
    /// out, with its handles, or the text being typed; or nothing, when it
    /// is for the next drawn, and is over the other bar.
    fn tray_around(&mut self) -> Option<Rect> {
        if let Some((index, _)) = self.editing {
            return Some(self.shapes[index].bounds(&mut self.text));
        }
        let chosen = if self.tool == Tool::Select { self.chosen() } else { Vec::new() };
        chosen.into_iter().map(|index| self.frame_of(index)).reduce(|all, frame| Rect::spanning((all.x.min(frame.x), all.y.min(frame.y)), (all.right().max(frame.right()), all.bottom().max(frame.bottom()))))
    }

    fn set_size(&mut self, size: usize) {
        self.size = size.min(STROKES.len() - 1);
        let (stroke, font_px) = (self.stroke(), self.font_px());
        let picked = match self.editing {
            Some((index, _)) => vec![index],
            None => self.chosen(),
        };
        if self.editing.is_none() && !picked.is_empty() {
            self.checkpoint();
        }
        for index in picked {
            let shape = &mut self.shapes[index];
            (shape.stroke, shape.font_px) = (stroke, font_px);
            shape.touch();
        }
        self.dirty = true;
    }

    /// Keeps the shapes as they are, to come back to with undo.
    /// Anything done but moving the last copies made forgets them.
    fn checkpoint(&mut self) {
        self.undo.push(self.shapes.clone());
        self.redo.clear();
        self.repeat = None;
    }

    /// The same, for shapes about to be moved: the last copies made, if
    /// these are those, still remembered.
    fn checkpoint_move(&mut self, going: &[usize]) {
        let mut ids: Vec<u32> = going.iter().map(|&index| self.shapes[index].id).collect();
        ids.sort_unstable();
        let kept = self.repeat.take().filter(|(_, copies)| {
            let mut copies = copies.clone();
            copies.sort_unstable();
            copies == ids
        });
        self.checkpoint();
        self.repeat = kept;
    }

    fn undo(&mut self) {
        self.commit_text();
        self.repeat = None;
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.shapes, before));
            self.selected.clear();
        }
    }

    fn redo(&mut self) {
        self.commit_text();
        self.repeat = None;
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.shapes, after));
            self.selected.clear();
        }
    }

    /// Something chosen in the box of badges and stickers: it is put
    /// in the middle of the selection, picked out, to be dragged to
    /// where it is wanted. A badge can be typed over with a double
    /// click; a sticker is text like any other, and twice the size.
    fn choose(&mut self, cell: Cell) {
        let Some(area) = self.selection else { return };
        // What the picture is set on, and where it is saved, are not
        // things put on it: they are set, and the box stays open for
        // the next of them, but for the folder, which is one choice.
        match cell {
            Cell::Backdrop(0) => self.frame.backdrop = Backdrop::None,
            Cell::Backdrop(which) => {
                self.frame.backdrop = Backdrop::Gradient(which - 1);
                // A backdrop with no room round the picture is not seen.
                self.frame.pad = (self.frame.pad.0.max(PADS[2]), self.frame.pad.1.max(PADS[2]));
            }
            Cell::Shadow => self.frame.shadow = if self.frame.shadow.on() { Shadow::NONE } else { Shadow::named(frame::SHADOW).expect("the usual shadow is one of them") },
            Cell::Shade(which) => self.frame.shadow = Shadow::of_preset(which),
            Cell::Window(which) => self.frame.window = frame::WINDOWS[which.min(frame::WINDOWS.len() - 1)].0,
            Cell::More => self.more = !self.more,
            // Asked for here, fetched by whoever has the network, and
            // set behind the picture when it comes.
            Cell::Dice => {
                let room = self.frame.room(self.scale());
                self.wish = Some((area.w + 2 * room.0.max(128), area.h + 2 * room.1.max(128)));
            }
            Cell::Own => {
                let read = self.settings.photo.as_ref().and_then(|file| std::fs::read(file).ok()).and_then(|bytes| frame::open(&bytes));
                self.photo_arrived(read);
            }
            Cell::Place(place) => (self.place, self.flyout) = (place, None),
            Cell::Width(size) => {
                self.set_size(size);
                self.flyout = None;
            }
            Cell::Head(end, style) => {
                self.set_head(end, style);
                self.flyout = None;
            }
            // The edge of white goes on and off with the box left open;
            // a kind of line is one choice.
            Cell::Line(kind) => {
                self.set_line(kind);
                if kind != 3 {
                    self.flyout = None;
                }
            }
            Cell::Shelf(shelf) => (self.shelf, self.scroll) = (shelf, 0),
            Cell::Badge(_) | Cell::Emoji(_) => {}
        }
        if !matches!(cell, Cell::Badge(_) | Cell::Emoji(_)) {
            return;
        }
        self.checkpoint();
        let (stroke, font_px) = (self.stroke(), self.font_px());
        let mut says = 0;
        let mut shape = match cell {
            // The look clicked on, saying what it does, in its own color
            // or the color in hand.
            Cell::Badge(item) => {
                let Some(&(look, _, words, color)) = LIBRARY.get(item) else { return };
                let mut shape = Shape::new(Tool::Badge, color.unwrap_or(self.color), stroke, (0.0, 0.0));
                (shape.text, shape.font_px, shape.face, shape.number) = (words.to_owned(), font_px, self.face, look as u32);
                says = words.len();
                shape
            }
            Cell::Emoji(index) => {
                let Some(&which) = self.emoji_found().get(index) else { return };
                let mut shape = Shape::new(Tool::Text, self.color, stroke, (0.0, 0.0));
                (shape.text, shape.font_px) = (emoji()[which].0.to_owned(), font_px * 2);
                shape
            }
            _ => return,
        };
        let size = shape.bounds(&mut self.text);
        let (cx, cy) = area.center();
        shape.points[0] = ((cx - size.w / 2) as f32, (cy - size.h / 2) as f32);
        let badge = shape.tool == Tool::Badge;
        self.shapes.push(shape);
        self.made(self.shapes.len() - 1);
        self.flyout = None;
        // A badge says nothing yet that was meant: what is typed next
        // is what it says, in place of what it was put down saying.
        if badge {
            self.editing = Some((self.shapes.len() - 1, false));
            (self.caret, self.all) = (says, true);
            self.selected.clear();
        }
    }

    /// A photograph is wanted from the network, of about this size:
    /// said once, to whoever asks.
    pub fn take_wish(&mut self) -> Option<(i32, i32)> {
        self.wish.take()
    }

    /// The photograph asked for has come, or has not: with one, the
    /// picture is set on it, with room round it to see it by.
    pub fn photo_arrived(&mut self, photo: Option<Photo>) {
        if let Some(photo) = photo {
            self.photo = Some(photo);
            // Another photograph, though it may be the size of the last:
            // what was made of the last is out of date.
            self.photos += 1;
            self.frame.backdrop = Backdrop::Photo;
            self.frame.pad = (self.frame.pad.0.max(PADS[2]), self.frame.pad.1.max(PADS[2]));
            self.look = None;
        }
        self.dirty = true;
    }

    fn library_found(&self) -> Vec<usize> {
        library_found(&self.query, self.shelf)
    }

    fn emoji_found(&self) -> Vec<usize> {
        emoji_found(&self.query)
    }

    /// How far outside the selection a point is: by whichever of its
    /// sides it is furthest out from.
    fn outward(&self, at: (i32, i32)) -> (i32, i32) {
        let area = self.selection.unwrap_or_default();
        ((area.x - at.0).max(at.0 - area.right()), (area.y - at.1).max(at.1 - area.bottom()))
    }

    /// How much room the frame takes beside the picture, and above and
    /// below it, on this screen; none, with no frame.
    fn frame_room(&self) -> (i32, i32) {
        if self.frame.is_plain() { (0, 0) } else { self.frame.room(self.scale()) }
    }

    /// The selection with its frame round it: the selection, with none.
    fn frame_rect(&self) -> Rect {
        let (area, room) = (self.selection.unwrap_or_default(), self.frame_room());
        Rect::new(area.x - room.0, area.y - room.1, area.w + 2 * room.0, area.h + 2 * room.1)
    }

    /// How many times as wide as it is tall the frame is.
    fn frame_shape(&self) -> f32 {
        let rect = self.frame_rect();
        rect.w.max(1) as f32 / rect.h.max(1) as f32
    }

    /// The handles the frame has on its edges, to be made larger by:
    /// all eight, with the move tool in hand and a frame to have them.
    fn frame_grips(&self) -> Vec<(i32, i32)> {
        match self.selection.is_some() && !self.frame.is_plain() && self.tool == Tool::Select {
            true => GRIPS.to_vec(),
            false => Vec::new(),
        }
    }

    /// Where the dot is by which the corners of the frame are made
    /// rounder: inside its top-left corner, as far in as they are
    /// round. None with no backdrop, which has no corners to see.
    fn frame_dot(&self) -> Option<(i32, i32)> {
        if self.frame.backdrop == Backdrop::None || self.selection.is_none() || self.tool != Tool::Select {
            return None;
        }
        let (rect, least) = (self.frame_rect(), ui_px(self.monitor().h) as i32);
        let inset = ((self.frame.radius as f32 * self.scale()).round() as i32).max(least).min(rect.w.min(rect.h) / 2);
        Some((rect.x + inset, rect.y + inset))
    }

    /// Which of the folders Save saves to.
    pub fn place(&self) -> usize {
        self.place
    }

    /// The picture that is taken, set in its frame: how wide and tall
    /// it is with the room round it, its pixels, and whether any of it
    /// is see-through, which is said in the top byte of each.
    pub fn framed(&mut self) -> Option<(Rect, i32, i32, Vec<u32>, bool)> {
        self.follow();
        let (area, mut pixels, rounded) = self.result()?;
        let (mut wide, mut high, mut clear) = (area.w, area.h, rounded);
        if !self.frame.is_plain() {
            if !rounded {
                pixels.iter_mut().for_each(|pixel| *pixel |= 0xFF00_0000);
            }
            let picture = std::mem::take(&mut pixels);
            (wide, high, pixels) = frame::set(self.frame, &picture, area.w, area.h, self.corner_radius(), self.scale(), self.photo.as_ref());
            if let Some(title) = self.title() {
                frame::title(self.frame, &mut pixels, wide, &picture, (area.w, area.h), self.scale(), &title, &mut self.text);
            }
            clear = self.frame.is_clear() || (self.frame.backdrop == Backdrop::Photo && self.photo.is_none());
        }
        // A zoom may reach past the selection, and past the frame: the
        // picture is then as large as takes it all in, with nothing
        // where there is neither picture nor zoom.
        let within = self.frame_rect();
        let mut all = within;
        for shape in self.shapes.iter().filter(|shape| shape.tool == Tool::Zoom) {
            let reach = shape.bounds(&mut self.text).inflate(shape.stroke.ceil() as i32 + 2).intersect(self.screen());
            let (x, y) = (all.x.min(reach.x), all.y.min(reach.y));
            all = Rect::new(x, y, all.right().max(reach.right()) - x, all.bottom().max(reach.bottom()) - y);
        }
        if all == within {
            return Some((area, wide, high, pixels, clear));
        }
        // Two pictures of that size: its colors, and how opaque each
        // pixel is, as a gray. What there was goes into both; and the
        // zooms are drawn on both, in their colors on the one and in
        // white on the other, which leaves the second saying how much
        // of each pixel there now is anything in.
        let (across, down) = (all.w, all.h);
        let mut colors = vec![0u32; (across * down) as usize];
        let mut grays = vec![0u32; (across * down) as usize];
        let (left, top) = (within.x - all.x, within.y - all.y);
        for row in 0..high {
            for column in 0..wide {
                let pixel = pixels[(row * wide + column) as usize];
                let opaque = if clear { pixel >> 24 } else { 255 };
                let at = ((top + row) * across + left + column) as usize;
                (colors[at], grays[at]) = (pixel & 0xFF_FFFF, opaque * 0x010101);
            }
        }
        for (buffer, white) in [(&mut colors, false), (&mut grays, true)] {
            let mut canvas = Canvas::new(buffer, across, down);
            for shape in self.shapes.iter().filter(|shape| shape.tool == Tool::Zoom) {
                let mut moved = shape.clone();
                moved.translate(-all.x as f32, -all.y as f32);
                if white {
                    moved.color = 0xFFFFFF;
                }
                moved.draw(&mut canvas, &mut self.text);
            }
        }
        let whole = colors.iter().zip(&grays).map(|(color, gray)| (gray & 0xFF) << 24 | color & 0xFF_FFFF).collect();
        Some((area, across, down, whole, true))
    }

    /// Takes up a text that is already there, to be typed into again.
    fn edit_text(&mut self, index: usize) {
        self.commit_text();
        self.checkpoint();
        self.editing = Some((index, false));
        self.caret = self.shapes[index].text.len();
        self.selected.clear();
    }

    /// Puts text in where the caret is: what was on the clipboard.
    pub fn paste(&mut self, pasted: &str) {
        if let Some((index, _)) = self.editing {
            let pasted: String = pasted.chars().filter(|&letter| letter == '\n' || !letter.is_control()).collect();
            let text = &mut self.shapes[index].text;
            if std::mem::take(&mut self.all) {
                text.clear();
            }
            let caret = self.caret.min(text.len());
            text.insert_str(caret, &pasted);
            self.caret = caret + pasted.len();
            self.dirty = true;
        }
    }

    /// Where in the text being typed a point of the screen is: before
    /// the letter it is nearest the left edge of, on the line it is on.
    fn caret_at(&mut self, index: usize, at: (i32, i32)) -> usize {
        let shape = &self.shapes[index];
        let (left, top) = shape.text_origin();
        let row = ((at.1 - top) / Text::line_height(shape.font_px).max(1)).max(0) as usize;
        let mut start = 0;
        let lines: Vec<&str> = shape.text.split('\n').collect();
        let row = row.min(lines.len() - 1);
        for line in &lines[..row] {
            start += line.len() + 1;
        }
        let line = lines[row];
        let mut nearest = (i32::MAX, 0);
        self.text.look = shape.look();
        for at_letter in line.char_indices().map(|(at, _)| at).chain([line.len()]) {
            let off = (left + self.text.width_in(shape.words_face(), &format!("{}{}", shape.lead(), &line[..at_letter]), shape.font_px) - at.0).abs();
            if off < nearest.0 {
                nearest = (off, at_letter);
            }
        }
        self.text.look = crate::text::Look::default();
        start + nearest.1
    }

    /// Done typing. Text with nothing in it is not kept; and if it was
    /// only just begun, neither is the step back to before it. Text
    /// that is kept is picked out, as anything just made is.
    fn commit_text(&mut self) {
        self.all = false;
        if let Some((index, new)) = self.editing.take() {
            if self.shapes[index].is_slight() {
                self.shapes.remove(index);
                if new {
                    self.undo.pop();
                }
            } else {
                self.made(index);
            }
            self.dirty = true;
        }
    }

    /// A shape has just been made: it is the one picked out, and the
    /// tool in hand is the one that moves things.
    fn made(&mut self, index: usize) {
        (self.tool, self.selected, self.hovered) = (Tool::Select, vec![index], None);
    }

    // ---- The picture ----

    /// Whether the magnifier is up: while an edge is being placed, if it
    /// is wanted at all, and always while a color is being picked.
    fn loupe_shows(&self) -> bool {
        let over_interface = matches!(self.hover, Target::Button(_) | Target::Panel | Target::Swatch(_) | Target::Pipette | Target::Cell(_));
        // Up when X has been pressed for it, for putting an edge in
        // place to the pixel, until X is pressed again; and with
        // --magnifier, by itself while an edge is put in place, and
        // before anything is selected.
        let placing = matches!(self.drag, Drag::Select { .. } | Drag::Resize { .. });
        let waiting = self.drag == Drag::None && self.selection.is_none();
        !over_interface && (self.pipette || self.loupe || (self.settings.magnifier && (waiting || placing)))
    }

    /// Paints the screen as it now stands into a buffer of its size.
    pub fn draw(&mut self, out: &mut [u32]) {
        self.follow();
        let layout = self.layout();
        let (width, height) = (self.shot.width, self.shot.height);
        let (px, pad) = (layout.px, layout.px as i32 / 2);
        let accent = self.settings.accent;

        canvas::copy_into(out, &self.dim);
        // The frame the picture is set in, where it will be: round the
        // selection, over whatever of the screen that takes. Not while
        // the selection is being reshaped, which changes it every frame.
        let reshaping = matches!(self.drag, Drag::Select { begun: true, .. } | Drag::Resize { .. });
        let framed = self.selection.filter(|_| !self.frame.is_plain() && !reshaping);
        if let Some(area) = framed {
            let (room, radius, scale) = (self.frame_room(), self.corner_radius(), self.scale());
            // With a window, where the selection is says how dark its bar
            // is and what it is called.
            let title = self.title().filter(|_| self.frame.window != frame::Window::None);
            let place = if self.frame.window == frame::Window::None { (0, 0) } else { (area.x, area.y) };
            let made_from = format!("{:?} {} {} {radius} {} {place:?} {title:?}", self.frame, area.w, area.h, self.photos);
            if self.surround.as_ref().is_none_or(|kept| kept.0 != made_from) {
                // Made as the picture's frame is, round the picture with
                // none of it showing: what is on the screen there, wholly
                // see-through, which is what its window's bar is made to
                // go with.
                let under: Vec<u32> = canvas::crop(&self.shot.pixels, self.shot.width, area).iter().map(|pixel| pixel & 0xFF_FFFF).collect();
                let (wide, _, mut pixels) = frame::set(self.frame, &under, area.w, area.h, radius, scale, self.photo.as_ref());
                if let Some(title) = &title {
                    frame::title(self.frame, &mut pixels, wide, &under, (area.w, area.h), scale, title, &mut self.text);
                }
                self.surround = Some((made_from, wide, pixels));
            }
            if let Some((_, wide, pixels)) = &self.surround {
                let (left, top) = (area.x - room.0, area.y - room.1);
                let part = Rect::new(left, top, *wide, pixels.len() as i32 / wide.max(&1)).intersect(Rect::new(0, 0, width, height));
                // With nothing behind the picture, what will be see-
                // through is shown as see-through things are, in light
                // squares: and a shadow falls on them where it would be
                // lost on the darkened screen.
                if self.frame.backdrop == Backdrop::None || (self.frame.backdrop == Backdrop::Photo && self.photo.is_none()) {
                    let square = (ui_px(self.monitor().h) as i32 * 3 / 4).max(6);
                    for y in part.y..part.bottom() {
                        for x in part.x..part.right() {
                            let light = ((x - left) / square + (y - top) / square) % 2 == 0;
                            out[(y * width + x) as usize] = if light { 0xD6D6D6 } else { 0xC2C2C2 };
                        }
                    }
                }
                for y in part.y..part.bottom() {
                    let from = &pixels[((y - top) * wide + part.x - left) as usize..][..part.w as usize];
                    for (pixel, over) in out[(y * width + part.x) as usize..][..part.w as usize].iter_mut().zip(from) {
                        let cover = over >> 24;
                        *pixel = canvas::mix(*pixel, over & 0xFF_FFFF, cover + (cover >> 7));
                    }
                }
            }
        }
        // What the corners of the picture show, outside their curve:
        // the frame, where there is one, and the darkened screen.
        let behind: Vec<(i32, i32, u32)> = match (self.selection, self.corner_radius()) {
            (Some(area), radius) if radius > 0 => [(area.x, area.y), (area.right() - radius, area.y), (area.x, area.bottom() - radius), (area.right() - radius, area.bottom() - radius)]
                .into_iter()
                .flat_map(|(x, y)| (0..radius * radius).map(move |at| (x + at % radius, y + at / radius)))
                .filter(|&(x, y)| x >= 0 && y >= 0 && x < width && y < height)
                .map(|(x, y)| (x, y, out[(y * width + x) as usize]))
                .collect(),
            _ => Vec::new(),
        };
        // What is selected shows as it is. Before there is a selection,
        // everything is dark, as the screen was the moment it froze;
        // the window a click would take is told by a line round it, or
        // if it is wanted so, by showing as it is too.
        let window = if self.drag == Drag::None && self.selection.is_none() { self.window } else { None };
        let lit = self.selection.or(window.filter(|_| self.settings.windows == Windows::Bright));
        let round = self.corner_radius();
        if let Some(area) = lit {
            canvas::copy_rect(out, &self.shot.pixels, width, area);
            // The mouse pointer, where it is wanted in the picture:
            // under whatever is drawn, as the screen itself is.
            if let (Some(arrow), true) = (&self.arrow, self.arrow_shows) {
                arrow.stamp_within(out, width, (0, 0), area);
            }
        }
        let mut canvas = Canvas::new(out, width, height);

        if let Some(selection) = self.selection {
            shapes::paint(&self.shapes, &mut canvas, &mut self.text, selection);
            // A zoom is not kept to the selection: where it reaches
            // past, it shows, and comes out in the picture too.
            for shape in self.shapes.iter().filter(|shape| shape.tool == Tool::Zoom) {
                if shape.bounds(&mut self.text).intersect(selection) != shape.bounds(&mut self.text) {
                    shape.draw(&mut canvas, &mut self.text);
                }
            }
            canvas.set_clip(selection);
            // The caret, where the next letter typed will go.
            if let Some((index, _)) = self.editing {
                let shape = &self.shapes[index];
                let line = Text::line_height(shape.font_px);
                let before = &shape.text[..self.caret.min(shape.text.len())];
                let last = before.rsplit('\n').next().unwrap_or("");
                let rows = before.split('\n').count() as i32;
                let origin = shape.text_origin();
                // All of it picked out, to be typed over: it shows on a
                // wash of the accent.
                if self.all {
                    let over = shape.bounds(&mut self.text);
                    canvas.round_fill(over.x, over.y, over.w, over.h, 0, accent, 90);
                }
                // As far along as the letters before it are wide, set
                // the way this text is set and after its dot if it has one.
                self.text.look = shape.look();
                let along = self.text.width_in(shape.words_face(), &format!("{}{last}", shape.lead()), shape.font_px);
                self.text.look = crate::text::Look::default();
                let x = origin.0 + along;
                let y = origin.1 + (rows - 1) * line;
                canvas.fill(x + 1, y + line / 8, (shape.font_px as i32 / 12).max(2), line - line / 4, shape.color);
            }
            canvas.reset_clip();
            // With rounded corners, what is outside the curve is no
            // part of the picture, and shows darkened like the rest.
            let kept: std::collections::HashMap<(i32, i32), u32> = behind.iter().map(|&(x, y, pixel)| ((x, y), pixel)).collect();
            canvas.round_off_with(selection, round, |x, y| kept.get(&(x, y)).copied().unwrap_or(0));
        }

        // The edge of what is lit: a line just outside it.
        let thick = (px as i32 / 12).max(1);
        if let Some(area) = lit {
            let around = area.inflate(thick);
            if round > 0 {
                canvas.round_outline(around.x, around.y, around.w, around.h, round + thick, thick, accent);
            } else {
                canvas.fill(around.x, around.y, around.w, thick, accent);
                canvas.fill(around.x, around.bottom() - thick, around.w, thick, accent);
                canvas.fill(around.x, area.y, thick, area.h, accent);
                canvas.fill(around.right() - thick, area.y, thick, area.h, accent);
            }
        } else if let Some(area) = window {
            // Just inside it, where a window as large as the screen
            // still has room for a line.
            canvas.round_outline(area.x, area.y, area.w, area.h, 0, 2 * thick, accent);
        }
        if let Some(selection) = self.selection {
            // With the move tool, the shape under the pointer shows the
            // box it is in; the one picked out shows its box with a
            // handle at each corner to resize it by, and if it is a
            // rectangle a dot inside each to round its corners by.
            let grip = (px as i32 / 5).max(3);
            let idle = !matches!(self.drag, Drag::Shape { .. } | Drag::Move { .. });
            if let Some(index) = self.hovered.filter(|&index| idle && self.tool == Tool::Select && index < self.shapes.len() && !self.selected.contains(&index)) {
                let frame = self.frame_of(index);
                canvas.round_outline(frame.x, frame.y, frame.w, frame.h, 0, thick, soft(accent));
            }
            // Each of several picked out has its box, and no more. A
            // line picked out by itself has only its two ends.
            let alone = self.picked().filter(|&index| !boxed(self.shapes[index].tool) && !self.ends(index).is_empty());
            // One turned has its box turned with it.
            if self.tool == Tool::Select {
                for index in self.chosen().into_iter().filter(|&index| alone != Some(index)) {
                    match self.shapes[index].angle {
                        0.0 => {
                            let frame = self.frame_of(index);
                            canvas.round_outline(frame.x, frame.y, frame.w, frame.h, 0, thick, accent);
                        }
                        angle => {
                            let frame = self.local_frame_of(index);
                            let mut outline = Shape::new(Tool::Rect, accent, thick as f32, (frame.x as f32 + 0.5, frame.y as f32 + 0.5));
                            outline.points[1] = (frame.right() as f32 - 0.5, frame.bottom() as f32 - 0.5);
                            outline.angle = angle;
                            outline.draw(&mut canvas, &mut self.text);
                        }
                    }
                }
            }
            // The lines a shape being moved has gone to: the edge or
            // the middle of another, or of the selection.
            for &(upright, at) in &self.guides {
                match upright {
                    true => canvas.fill(at, selection.y, 1, selection.h, soft(accent)),
                    false => canvas.fill(selection.x, at, selection.w, 1, soft(accent)),
                }
            }
            // And the gaps it is spaced evenly by, in magenta, each with
            // how wide it is.
            let gaps = self.gaps.clone();
            spacing::draw_gaps(&mut canvas, &mut self.text, &gaps, (px * 4 / 5).max(11), |(x, y)| (x.round() as i32, y.round() as i32));
            // How round, in pixels, while a corner is being rounded:
            // of a rectangle, or of the picture.
            let rounding = match self.drag {
                Drag::Round { index, .. } => self.shapes.get(index).map(|shape| format!("{} px", shape.radius.round() as i32)),
                Drag::Curve { .. } => Some(format!("{} px", self.corner_radius())),
                Drag::FrameCorner { .. } => Some(format!("{} px", (self.frame.radius as f32 * self.scale()).round() as i32)),
                // And how many times larger, while a zoom is made so.
                Drag::End { index, point: 3, .. } => self.shapes.get(index).filter(|shape| shape.tool == Tool::Zoom).map(|shape| format!("{:.1}\u{d7}", shape.zoom_times())),
                // And how far it is turned, either way of none.
                Drag::Turn { index, .. } => self.shapes.get(index).map(|shape| {
                    let degrees = if shape.turnable() { shape.angle } else { self.turned }.to_degrees().round().rem_euclid(360.0);
                    format!("{}\u{b0}", if degrees > 180.0 { degrees - 360.0 } else { degrees })
                }),
                _ => None,
            };
            if let Some(says) = rounding {
                let wide = self.text.width(&says, px) + 2 * pad;
                let high = Text::line_height(px) + pad / 2;
                let (x, y) = (self.pointer.0 + px as i32, self.pointer.1 + px as i32);
                canvas.round_fill(x, y, wide, high, pad / 2, 0x000000, 210);
                self.text.draw(&mut canvas, x + pad, y + pad / 4, px, TEXT_COLOR, &says);
            }
            // The box being dragged out to pick shapes with.
            if let Drag::Marquee { from } = self.drag {
                let reach = Rect::spanning(from, self.pointer);
                canvas.round_fill(reach.x, reach.y, reach.w, reach.h, 0, accent, 40);
                canvas.round_outline(reach.x, reach.y, reach.w, reach.h, 0, thick, accent);
            }
            if let Some(index) = self.picked() {
                let gripped = boxed(self.shapes[index].tool);
                for (across, down) in GRIPS.into_iter().filter(|_| gripped) {
                    let (x, y) = self.grip_of(index, across, down);
                    canvas.fill(x - grip - thick, y - grip - thick, 2 * (grip + thick), 2 * (grip + thick), accent);
                    canvas.fill(x - grip, y - grip, 2 * grip, 2 * grip, 0xFFFFFF);
                }
                for (_, (x, y)) in self.ends(index) {
                    canvas.disc(x, y, grip + 1 + thick, accent, 256);
                    canvas.disc(x, y, grip + 1, 0xFFFFFF, 256);
                }
                for (_, _, (x, y)) in self.rounders(index).into_iter().flatten() {
                    canvas.disc(x, y, grip + thick, accent, 256);
                    canvas.disc(x, y, grip, 0xFFFFFF, 256);
                }
            }
            // And the selection has its own dots, for the corners of the
            // picture itself.
            for (_, _, (x, y)) in self.curves().into_iter().flatten() {
                canvas.disc(x, y, grip + thick, accent, 256);
                canvas.disc(x, y, grip, 0xFFFFFF, 256);
            }
            // And the frame, one for its own; with a handle at each
            // corner and the middle of each side of it, square where
            // the selection's are round, to make it larger by: a side
            // one way, a corner both.
            if let Some((x, y)) = self.frame_dot().filter(|_| !reshaping) {
                canvas.disc(x, y, grip + thick, accent, 256);
                canvas.disc(x, y, grip, 0xFFFFFF, 256);
            }
            if !reshaping {
                let whole = self.frame_rect();
                for (across, down) in self.frame_grips() {
                    let (x, y) = grip_at(whole, across, down);
                    canvas.fill(x - grip - thick, y - grip - thick, 2 * (grip + thick), 2 * (grip + thick), accent);
                    canvas.fill(x - grip, y - grip, 2 * grip, 2 * grip, 0xFFFFFF);
                }
            }
            let _ = selection;
            // Handles at the corners and the middles of the sides, on a
            // selection large enough to have room for them.
            let knob = (px as i32 / 4).max(3);
            let drawing = matches!(self.drag, Drag::Draw { .. } | Drag::Select { begun: true, .. });
            if !drawing && selection.w >= knob * 8 && selection.h >= knob * 8 {
                for (across, down) in [(0, 0), (1, 0), (2, 0), (0, 1), (2, 1), (0, 2), (1, 2), (2, 2)] {
                    let (x, y) = (selection.x + selection.w * across / 2, selection.y + selection.h * down / 2);
                    canvas.disc(x, y, knob + thick, 0xFFFFFF, 256);
                    canvas.disc(x, y, knob, accent, 256);
                }
            }
        }

        if let Some((place, words)) = &layout.label {
            canvas.round_fill(place.x, place.y, place.w, place.h, pad / 2, 0x000000, 190);
            self.text.draw(&mut canvas, place.x + pad, place.y + pad / 4, px, TEXT_COLOR, words);
        }
        if let Some(bar) = layout.bar {
            self.draw_bar(&mut canvas, &layout, bar, &layout.buttons, &layout.dividers, false);
        }
        if let Some(tray) = layout.tray {
            self.draw_bar(&mut canvas, &layout, tray, &layout.tray_buttons, &layout.tray_dividers, layout.tray_above);
        }
        if let Some(popup) = layout.popup {
            self.draw_popup(&mut canvas, &layout, popup);
        }
        if let Some(flyout) = layout.flyout {
            let (pad, accent) = (px as i32 / 2, self.settings.accent);
            panel(&mut canvas, flyout, pad);
            for &(rect, cell) in &layout.cells {
                let inner = rect.inflate(-(px as i32 / 6).max(2));
                if self.hover == Target::Cell(cell) {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, pad / 2, 0xFFFFFF, 30);
                }
                let (cx, cy) = rect.center();
                match cell {
                    // A look of a badge, as it will be on the picture: in
                    // the color in hand, and the size it will be there.
                    // A thing in the library, as it will be put down: in
                    // its own color or the one in hand, saying what it
                    // will, a little smaller, kept to its cell.
                    Cell::Badge(item) => {
                        let Some(&(look, _, words, color)) = LIBRARY.get(item) else { continue };
                        let mut badge = Shape::new(Tool::Badge, color.unwrap_or(self.color), 1.0, (0.0, 0.0));
                        (badge.text, badge.font_px, badge.number) = (words.to_owned(), px * 7 / 8, look as u32);
                        let size = badge.bounds(&mut self.text);
                        badge.points[0] = ((cx - size.w / 2).max(inner.x) as f32, (cy - size.h / 2) as f32);
                        canvas.set_clip(inner);
                        badge.draw(&mut canvas, &mut self.text);
                        canvas.reset_clip();
                    }
                    // A shelf, by its name: lit when it is the one shown.
                    Cell::Shelf(shelf) => {
                        let shown = shelf == self.shelf && self.query.is_empty();
                        if shown {
                            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, pad / 2, accent, 256);
                        }
                        let name = SHELVES[usize::from(shelf)];
                        self.text.draw(&mut canvas, rect.x + pad, cy - Text::line_height(px) / 2, px, if shown { on(accent) } else { TEXT_COLOR }, name);
                    }
                    Cell::Emoji(index) => {
                        if let Some(&which) = self.emoji_found().get(index) {
                            self.text.draw_centered(&mut canvas, cx, cy, px * 5 / 4, TEXT_COLOR, emoji()[which].0);
                        }
                    }
                    // No backdrop: an empty square with a line through it.
                    Cell::Backdrop(0) => {
                        canvas.round_outline(inner.x, inner.y, inner.w, inner.h, pad / 2, 1, DIM_TEXT_COLOR);
                        self.text.draw_centered(&mut canvas, cx, cy, px, DIM_TEXT_COLOR, icons::CLOSE);
                    }
                    // A backdrop, as a small square of itself.
                    Cell::Backdrop(which) => {
                        for at in 0..inner.w * inner.h {
                            let (x, y) = (at % inner.w, at / inner.w);
                            let color = frame::shade(which - 1, (x as f32 + 0.5) / inner.w as f32, (y as f32 + 0.5) / inner.h as f32, 0.5);
                            canvas.blend(inner.x + x, inner.y + y, color, 255);
                        }
                        canvas.round_outline(inner.x - 1, inner.y - 1, inner.w + 2, inner.h + 2, pad / 2, 2, MENU_COLOR);
                        // One too near the box's own color to be told
                        // from it has a faint line round it.
                        canvas.round_outline(inner.x, inner.y, inner.w, inner.h, pad / 2, 1, canvas::mix(MENU_COLOR, 0xFFFFFF, 40));
                    }
                    // The dots, for the rest of them; or, with the rest
                    // showing, a mark to put them away again.
                    Cell::More => match self.more {
                        true => self.text.draw_centered(&mut canvas, cx, cy, px, TEXT_COLOR, icons::DOWN),
                        false => (-1..=1).for_each(|dot| canvas.disc(cx + dot * (px as i32 * 2 / 5).max(4), cy, (px as i32 / 9).max(1), TEXT_COLOR, 256)),
                    },
                    // No window: crossed out, as no backdrop is. A window:
                    // a small one, its bar over it, lights or buttons on it.
                    Cell::Window(0) => {
                        canvas.round_outline(inner.x, inner.y, inner.w, inner.h, pad / 2, 1, DIM_TEXT_COLOR);
                        self.text.draw_centered(&mut canvas, cx, cy, px, DIM_TEXT_COLOR, icons::CLOSE);
                    }
                    Cell::Window(which) => {
                        let window = frame::WINDOWS[which.min(frame::WINDOWS.len() - 1)].0;
                        let small = inner.inflate(-(px as i32 / 6).max(1));
                        let (body, top) = if window == frame::Window::Terminal { (0x16171A, 0x2B2D31) } else { (0xFFFFFF, 0xD9DBE0) };
                        let bar = (small.h / 3).max(4);
                        canvas.round_fill(small.x, small.y, small.w, small.h, pad / 3, body, 256);
                        canvas.round_fill(small.x, small.y, small.w, bar + pad / 3, pad / 3, top, 256);
                        canvas.fill(small.x, small.y + bar, small.w, pad / 3, body);
                        let dot = (bar / 5).max(1);
                        match window {
                            frame::Window::Desktop => canvas.fill(small.right() - 3 * dot - 2, small.y + bar / 2 - dot / 2, 2 * dot, dot.max(1), 0x4A4D57),
                            _ => {
                                for (at, light) in [(0, 0xFF5F57), (1, 0xFEBC2E), (2, 0x28C840)] {
                                    canvas.disc(small.x + 2 * dot + at * 3 * dot, small.y + bar / 2, dot, light, 256);
                                }
                                if window == frame::Window::Browser {
                                    canvas.round_fill(small.x + small.w / 2 - small.w / 6, small.y + bar / 4, small.w / 2, bar / 2, bar / 4, 0xFFFFFF, 256);
                                }
                            }
                        }
                    }
                    Cell::Dice => self.text.draw_centered(&mut canvas, cx, cy, px * 5 / 4, TEXT_COLOR, icons::DICE),
                    Cell::Own => self.text.draw_centered(&mut canvas, cx, cy, px * 5 / 4, TEXT_COLOR, icons::FRAME),
                    // A square with its shadow under it.
                    Cell::Shadow => {
                        let square = inner.inflate(-(px as i32 / 3));
                        canvas.round_fill(square.x + 2, square.y + 4, square.w, square.h, 3, 0x000000, 150);
                        canvas.round_fill(square.x, square.y, square.w, square.h, 3, TEXT_COLOR, 256);
                    }
                    // Its size by Tailwind's name for it.
                    Cell::Shade(which) => {
                        let ink = if self.frame.shadow == Shadow::of_preset(which) { TEXT_COLOR } else { DIM_TEXT_COLOR };
                        self.text.draw_centered(&mut canvas, cx, cy, px * 7 / 8, ink, frame::SHADOWS[which].0);
                    }
                    Cell::Place(place) => {
                        let line = cy - Text::line_height(px) / 2;
                        if place == self.place {
                            self.text.draw_centered(&mut canvas, rect.x + rect.h / 2, cy, px, TEXT_COLOR, icons::CHECK);
                        }
                        let name = self.settings.places[place].clone();
                        self.text.draw(&mut canvas, rect.x + rect.h, line, px, TEXT_COLOR, &name);
                    }
                    // A width, or a size of text: a tick by the one there
                    // is, a line that wide or letters that large, what it
                    // is called, and its key.
                    Cell::Width(size) => {
                        let side = layout.side;
                        if size == self.shown_size() {
                            self.text.draw_centered(&mut canvas, rect.x + rect.h / 2, cy, px, TEXT_COLOR, icons::CHECK);
                        }
                        let text = self.width_names()[0] == "Small";
                        match text {
                            true => {
                                let large = ((FONT_SIZES[size] * px as f32 / 13.0) as u32).min(side as u32);
                                self.text.draw_centered(&mut canvas, rect.x + side * 3 / 2, cy, large, TEXT_COLOR, "Aa");
                            }
                            false => {
                                let thick = ((STROKES[size] * px as f32 / 13.0).round() as i32).max(1);
                                canvas.round_fill(rect.x + side, cy - thick / 2, side, thick, thick / 2, TEXT_COLOR, 256);
                            }
                        }
                        let line = cy - Text::line_height(px) / 2;
                        self.text.draw(&mut canvas, rect.x + 2 * side + pad, line, px, TEXT_COLOR, self.width_names()[size]);
                        let key = (size + 1).to_string();
                        let wide = self.text.width(&key, px);
                        self.text.draw(&mut canvas, rect.right() - pad - wide, line, px, DIM_TEXT_COLOR, &key);
                    }
                    // A head for one end, or a kind of line: a tick by the
                    // one there is, a short line drawn with it, its name.
                    Cell::Head(..) | Cell::Line(_) => {
                        let (side, shown) = (layout.side, self.shown_line());
                        let mut like = shown.clone();
                        (like.heads, like.elbow) = ([false, false], false);
                        let (now, name) = match cell {
                            Cell::Head(end, style) => {
                                let on = usize::from(style) < shapes::HEADS.len();
                                like.heads[end] = on;
                                (like.dashed, like.dotted, like.halo) = (false, false, false);
                                if on {
                                    like.ends[end] = style;
                                }
                                let now = if shown.heads[end] { shown.ends[end] == style } else { !on };
                                (now, if on { shapes::HEADS[usize::from(style)] } else { "None" })
                            }
                            Cell::Line(kind) => {
                                (like.dashed, like.dotted, like.halo) = (kind == 1, kind == 2, kind == 3);
                                let now = match kind {
                                    3 => shown.halo,
                                    2 => shown.dotted,
                                    1 => shown.dashed && !shown.dotted,
                                    _ => !shown.dashed && !shown.dotted,
                                };
                                (now, LINE_KINDS[usize::from(kind)])
                            }
                            _ => continue,
                        };
                        if now {
                            self.text.draw_centered(&mut canvas, rect.x + rect.h / 2, cy, px, TEXT_COLOR, icons::CHECK);
                        }
                        sample(&mut canvas, &mut self.text, &like, (rect.x + side, cy), (rect.x + side * 5 / 2, cy), TEXT_COLOR, 2.5);
                        self.text.draw(&mut canvas, rect.x + 3 * side, cy - Text::line_height(px) / 2, px, TEXT_COLOR, name);
                    }
                }
                // What is chosen now has a line of the accent round it.
                let on = match cell {
                    Cell::Backdrop(0) => self.frame.backdrop == Backdrop::None,
                    Cell::Backdrop(which) => self.frame.backdrop == Backdrop::Gradient(which - 1),
                    Cell::Dice | Cell::Own => false,
                    Cell::Shadow => self.frame.shadow.on(),
                    Cell::Shade(which) => self.frame.shadow == Shadow::of_preset(which),
                    Cell::Window(which) => self.frame.window == frame::WINDOWS[which].0,
                    _ => false,
                };
                if on {
                    canvas.round_outline(rect.x + 1, rect.y + 1, rect.w - 2, rect.h - 2, pad / 2, 2, accent);
                }
            }
            // What each kind of backdrop is called, over its rows; and
            // on the first of those lines, at its far end, what the one
            // under the pointer is called, or the one chosen.
            for (index, &((x, y), kind)) in layout.captions.iter().enumerate() {
                self.text.draw(&mut canvas, x, y, px * 7 / 8, DIM_TEXT_COLOR, kind);
                let named = match (self.hover, self.frame.backdrop) {
                    (Target::Cell(Cell::Backdrop(which)), _) if which > 0 => Some(which - 1),
                    (_, Backdrop::Gradient(which)) => Some(which),
                    _ => None,
                };
                if let (0, Some(which)) = (index, named) {
                    let name = BACKDROPS[which.min(BACKDROPS.len() - 1)].0;
                    let wide = self.text.width(name, px * 7 / 8);
                    self.text.draw(&mut canvas, flyout.right() - pad - wide, y, px * 7 / 8, TEXT_COLOR, name);
                }
            }
            // The picture as it will be, made to fit the room there is
            // to show it in.
            if let Some(room) = layout.look {
                canvas.round_fill(room.x, room.y, room.w, room.h, pad / 2, 0x101010, 256);
                self.draw_look(&mut canvas, room);
            }
            // The line a search is typed in: what has been typed and a
            // caret after it, or what there is to do.
            if let Some(field) = layout.field {
                let inner = field.inflate(-(px as i32 / 6).max(2));
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0x111111, 256);
                let line = inner.y + (inner.h - Text::line_height(px)) / 2;
                match self.query.is_empty() {
                    true => self.text.draw(&mut canvas, inner.x + pad, line, px, DIM_TEXT_COLOR, "Type to search"),
                    false => {
                        let typed = self.query.clone();
                        self.text.draw(&mut canvas, inner.x + pad, line, px, TEXT_COLOR, &typed);
                    }
                }
                let after = if self.query.is_empty() { 0 } else { self.text.width(&self.query, px) };
                canvas.fill(inner.x + pad + after + 1, line + 2, 2, Text::line_height(px) - 4, accent);
            }
            // The name of the emoji the pointer is on, or how many
            // there are to choose from.
            if let Some(caption) = layout.caption {
                let found = self.emoji_found();
                let says = match self.hover {
                    // Or what the look of a badge is called, in the box
                    // of those.
                    Target::Cell(Cell::Badge(item)) => LIBRARY.get(item).map_or(String::new(), |&(look, ..)| shapes::BADGE_NAMES[look].to_owned()),
                    Target::Cell(Cell::Emoji(index)) => found.get(index).map_or(String::new(), |&which| emoji()[which].1.to_owned()),
                    _ if !self.query.is_empty() && found.is_empty() && self.library_found().is_empty() => "None by that name".to_owned(),
                    _ if self.shelf == EMOJI_SHELF && self.query.is_empty() => format!("{} emoji; the wheel goes through them", found.len()),
                    _ => "Type what it says once it is down; recolor it as any shape".to_owned(),
                };
                self.text.draw(&mut canvas, caption.x + pad / 2, caption.y, px, DIM_TEXT_COLOR, &says);
            }
        }
        if self.selection.is_none() && !matches!(self.drag, Drag::Select { begun: true, .. }) {
            self.draw_hint(&mut canvas, px);
        }
        // A frame that does not fit on the screen round the selection,
        // as round one that fills the screen it never will: the whole
        // of it is shown small, in a corner, so that it can be seen at
        // all. Not while text is being typed, which making the picture
        // would bring to an end.
        if let (Some(_), None) = (framed, self.editing) {
            let (whole, monitor) = (self.frame_rect(), self.monitor());
            if whole.intersect(monitor) != whole {
                let wide = monitor.w / 4;
                let room = Rect::new(monitor.right() - wide - px as i32, monitor.y + px as i32, wide, wide * 10 / 16);
                panel(&mut canvas, room.inflate(px as i32 / 3), px as i32 / 2);
                self.draw_look(&mut canvas, room);
            }
        }
        // Where the selection lines up with something elsewhere: a thin
        // red line along both, as Figma shows it.
        let thick = (self.scale().round() as i32).max(1);
        for &(upright, at, from, to) in &self.lines {
            match upright {
                true => canvas.fill(at - thick / 2, from, thick, to - from, GUIDE_COLOR),
                false => canvas.fill(from, at - thick / 2, to - from, thick, GUIDE_COLOR),
            }
        }
        if self.loupe_shows() {
            self.draw_loupe(&mut canvas, px);
        }
    }

    /// A bar of buttons: the one of tools, or the one over what is picked
    /// out. What the button under the pointer is goes under the bar, or
    /// over it where there is no room under or `over` would rather.
    fn draw_bar(&mut self, canvas: &mut Canvas, layout: &Layout, bar: Rect, buttons: &[(Rect, Button)], dividers: &[i32], over: bool) {
        let (px, pad, side) = (layout.px, layout.px as i32 / 2, layout.side);
        let accent = self.settings.accent;
        panel(canvas, bar, pad);
        for &x in dividers {
            canvas.fill(x, bar.y + pad, 1, bar.h - 2 * pad, MENU_EDGE_COLOR);
        }
        let pressed = match self.drag {
            Drag::Button(button) => Some(button),
            _ => None,
        };
        // How what is shown looks, and whether what is picked out can go
        // further up or down among the rest.
        let shown = self.shown().cloned();
        let color = shown.as_ref().map_or(self.color, |shape| shape.color);
        let (fill, border, size) = (self.shown_fill(), self.shown_border(), self.shown_size());
        let face = shown.as_ref().map_or(self.face, |shape| shape.face);
        let style = shown.as_ref().filter(|shape| shape.tool == Tool::Text).map_or(self.style, |shape| shape.style);
        let line = self.shown_line();
        let sketch = shown.as_ref().map_or(self.sketch, |shape| shape.sketch);
        let square = shown.as_ref().map_or(self.square, |shape| shape.filled);
        let chosen = self.chosen();
        let up = (0..self.shapes.len()).any(|index| chosen.contains(&index) && (index + 1..self.shapes.len()).any(|above| !chosen.contains(&above)));
        let down = (0..self.shapes.len()).any(|index| chosen.contains(&index) && (0..index).any(|below| !chosen.contains(&below)));
        let open = |paint: Paint| self.popup.is_some() && self.paint == paint;
        for &(rect, button) in buttons {
            let hovered = self.hover == Target::Button(button) && (pressed.is_none() || pressed == Some(button));
            let on_accent = match button {
                Button::Tool(tool) => tool == self.tool && !self.pipette,
                Button::Pointer => self.arrow_shows && self.arrow.is_some(),
                Button::Style(one) => style & one != 0,
                Button::Sketch => sketch,
                Button::Width | Button::Start | Button::End | Button::Stroke => self.flyout == Some(button),
                _ => false,
            };
            let able = match button {
                Button::Undo => !self.undo.is_empty(),
                Button::Redo => !self.redo.is_empty(),
                // Where the display would not say what the pointer
                // looked like, there is none to put in.
                Button::Pointer => self.arrow.is_some(),
                Button::Forward => up,
                Button::Backward => down,
                _ => true,
            };
            let inner = rect.inflate(-(px as i32 / 8).max(1));
            if on_accent {
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, accent, 256);
            } else if hovered && able {
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, if pressed.is_some() { 56 } else { 30 });
            }
            // The color, the border or the fill the box of colors is open
            // for has a ring of the accent round its button.
            let open_for = match button {
                Button::Color | Button::Border => open(Paint::Line) || (button == Button::Color && self.pipette),
                Button::Fill => open(Paint::Fill),
                _ => false,
            };
            if open_for {
                canvas.round_outline(inner.x, inner.y, inner.w, inner.h, pad / 2, 2, accent);
            }
            let ink = if on_accent {
                on(accent)
            } else if !able {
                MENU_EDGE_COLOR
            } else {
                TEXT_COLOR
            };
            let (cx, cy) = rect.center();
            let radius = side * 3 / 10;
            // None: a line across, of the red that says no.
            let crossed = |canvas: &mut Canvas| {
                for step in -radius..radius {
                    canvas.fill(cx + step, cy - step - 1, 2, 2, RED);
                }
            };
            match button {
                // The color drawn in, ringed so that a dark one shows.
                Button::Color => {
                    canvas.disc(cx, cy, radius + (px as i32 / 10).max(1), MENU_EDGE_COLOR, 256);
                    canvas.disc(cx, cy, radius, color, 256);
                }
                // A border: a hollow square of its color; and a fill, a
                // solid one. One there is not is crossed out.
                Button::Border => match border {
                    Some(color) => {
                        let thick = (px as i32 / 5).max(3);
                        canvas.round_outline(cx - radius - 1, cy - radius - 1, 2 * radius + 2, 2 * radius + 2, radius / 2 + 1, 1, MENU_EDGE_COLOR);
                        canvas.round_outline(cx - radius, cy - radius, 2 * radius, 2 * radius, radius / 2, thick, color);
                    }
                    None => {
                        canvas.round_outline(cx - radius, cy - radius, 2 * radius, 2 * radius, radius / 2, 1, DIM_TEXT_COLOR);
                        crossed(canvas);
                    }
                },
                Button::Fill => match fill {
                    Some(color) => {
                        canvas.round_fill(cx - radius - 1, cy - radius - 1, 2 * radius + 2, 2 * radius + 2, radius / 2 + 1, MENU_EDGE_COLOR, 256);
                        canvas.round_fill(cx - radius, cy - radius, 2 * radius, 2 * radius, radius / 2, color, 256);
                    }
                    None => {
                        canvas.round_outline(cx - radius, cy - radius, 2 * radius, 2 * radius, radius / 2, 1, DIM_TEXT_COLOR);
                        crossed(canvas);
                    }
                },
                // Two letters in the face text is typed in.
                Button::Face => {
                    let across = self.text.width_in(face, "Aa", px);
                    self.text.draw_in(face, canvas, cx - across / 2, cy - Text::line_height(px) / 2, px, ink, "Aa");
                }
                // A dot as wide as the line is, near enough.
                Button::Width => {
                    let radius = ((STROKES[size] * px as f32 / 13.0 * 0.62).round() as i32 + 1).min(side * 3 / 10);
                    canvas.disc(cx, cy, radius, ink, 256);
                }
                // A letter set the way it stands for.
                Button::Style(one) => {
                    if one == shapes::HIGHLIGHT {
                        canvas.round_fill(cx - px as i32 * 2 / 3, cy - px as i32 / 2, px as i32 * 4 / 3, px as i32, 2, 0xFFE629, 150);
                    }
                    // On a patch: the letter dark on a patch of the light.
                    let ink = if one == shapes::BOXED {
                        canvas.round_fill(cx - px as i32 * 3 / 4, cy - px as i32 * 3 / 4, px as i32 * 3 / 2, px as i32 * 3 / 2, px as i32 / 4, ink, 256);
                        on(ink)
                    } else {
                        ink
                    };
                    let letter = STYLES.iter().find(|(style, _)| *style == one).map_or("", |(_, letter)| letter);
                    self.text.look = crate::text::Look { heavy: i32::from(one == shapes::BOLD), slant: if one == shapes::ITALIC { 0.2 } else { 0.0 }, opacity: 256 };
                    self.text.draw_centered(canvas, cx, cy, px * 5 / 4, ink, letter);
                    self.text.look = crate::text::Look::default();
                }
                // A line's way, its head at either end, and its kind, each
                // drawn as it is: straight or round its corner; the head
                // there, or none; solid, dashed, dotted, and its edge.
                Button::Path => {
                    let mut way = line.clone();
                    (way.heads, way.dashed, way.dotted, way.halo) = ([false, false], false, false, false);
                    let reach = side * 3 / 10;
                    sample(canvas, &mut self.text, &way, (cx - reach, cy - reach), (cx + reach, cy + reach), ink, 2.0);
                }
                Button::Start | Button::End => {
                    let end = usize::from(button == Button::End);
                    let mut head = line.clone();
                    (head.heads, head.dashed, head.dotted, head.halo, head.elbow) = ([end == 0 && line.heads[0], end == 1 && line.heads[1]], false, false, false, false);
                    let reach = side * 3 / 8;
                    sample(canvas, &mut self.text, &head, (cx - reach, cy), (cx + reach, cy), ink, 2.0);
                }
                Button::Stroke => {
                    let mut kind = line.clone();
                    (kind.heads, kind.elbow) = ([false, false], false);
                    let reach = side * 3 / 8;
                    sample(canvas, &mut self.text, &kind, (cx - reach, cy), (cx + reach, cy), ink, 2.0);
                }
                // A zoom's lens as it is: square, or round.
                Button::Lens => self.text.draw_centered(canvas, cx, cy, px * 5 / 4, ink, if square { icons::RECT } else { icons::ELLIPSE }),
                _ => self.text.draw_centered(canvas, cx, cy, px * 5 / 4, ink, button.icon()),
            }
        }

        // What the button under the pointer is, and its key.
        if let (Target::Button(button), None, None, Drag::None) = (self.hover, layout.popup, layout.flyout, self.drag) {
            let Some((rect, _)) = buttons.iter().find(|(_, other)| *other == button) else { return };
            let (name, key) = button.says();
            // The face says which it is, where the rest say what they are.
            let name = if button == Button::Face { FACES[face] } else { name };
            let line = Text::line_height(px);
            let (name_width, key_width) = (self.text.width(name, px), self.text.width(key, px));
            let width = name_width + key_width + 3 * pad;
            let (x, y) = hung(*rect, bar, over, (width, line + pad / 2), self.monitor(), pad);
            let tip = Rect::new(x, y, width, line + pad / 2).kept_within(self.monitor());
            canvas.round_fill(tip.x, tip.y, tip.w, tip.h, pad / 2, 0x000000, 210);
            self.text.draw(canvas, tip.x + pad, tip.y + pad / 4, px, TEXT_COLOR, name);
            self.text.draw(canvas, tip.x + 2 * pad + name_width, tip.y + pad / 4, px, DIM_TEXT_COLOR, key);
        }
    }

    fn draw_popup(&mut self, canvas: &mut Canvas, layout: &Layout, popup: Rect) {
        let (px, pad, side) = (layout.px, layout.px as i32 / 2, layout.side);
        panel(canvas, popup, pad);
        // The color it is at: of the line, the border or the fill it is
        // for, or none.
        let hand = self.paint_now();
        let color = hand.unwrap_or(self.color);
        // For a box or an oval: which of its border and its fill the
        // colors are for, the one they are for lit.
        if let (Some(&(first, _)), Some(&(last, _))) = (layout.paints.first(), layout.paints.last()) {
            let whole = Rect::new(first.x, first.y, last.right() - first.x, first.h);
            canvas.round_fill(whole.x, whole.y, whole.w, whole.h, pad / 2, 0x141414, 256);
            for &(rect, paint) in &layout.paints {
                let inner = rect.inflate(-(px as i32 / 8).max(2));
                let lit = paint == self.paint;
                if lit {
                    canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0x3A3A3A, 256);
                } else if self.hover == Target::Paint(paint) {
                    canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, 20);
                }
                let (cx, cy) = rect.center();
                self.text.draw_centered(canvas, cx, cy, px, if lit { TEXT_COLOR } else { DIM_TEXT_COLOR }, if paint == Paint::Line { "Border" } else { "Fill" });
            }
        }
        // The square: saturation across, value down, at the hue in hand;
        // and a ring where the color in hand is in it.
        let (hue, saturation, value) = self.hsv;
        let square = layout.square;
        for y in 0..square.h {
            let v = 1.0 - y as f32 / square.h.max(1) as f32;
            for x in 0..square.w {
                canvas.blend(square.x + x, square.y + y, from_hsv((hue, x as f32 / square.w.max(1) as f32, v)), 255);
            }
        }
        let (mx, my) = (square.x + (saturation * square.w as f32) as i32, square.y + ((1.0 - value) * square.h as f32) as i32);
        canvas.round_outline(mx - 7, my - 7, 14, 14, 7, 2, 0x000000);
        canvas.round_outline(mx - 6, my - 6, 12, 12, 6, 2, 0xFFFFFF);
        // The hues, end to end, and a mark at the one in hand.
        let strip = layout.hues;
        for x in 0..strip.w {
            canvas.fill(strip.x + x, strip.y, 1, strip.h, from_hsv((x as f32 / strip.w.max(1) as f32 * 360.0, 1.0, 1.0)));
        }
        let hx = strip.x + (hue / 360.0 * strip.w as f32) as i32;
        canvas.round_outline(hx - 4, strip.y - 3, 8, strip.h + 6, 3, 2, 0xFFFFFF);
        // The color in hand, in hex, on a patch of itself.
        let hex = layout.hex.inflate(-(px as i32 / 6).max(2));
        // None: a light patch crossed out in red.
        canvas.round_fill(hex.x, hex.y, hex.h, hex.h, pad / 2, if hand.is_some() { color } else { 0xF2F2F2 }, 256);
        if hand.is_none() {
            for step in 2..hex.h - 3 {
                canvas.fill(hex.x + step, hex.y + hex.h - 2 - step, 2, 2, RED);
            }
        }
        let (x, y) = (hex.x + hex.h + pad / 2, hex.y + (hex.h - Text::line_height(px)) / 2);
        // Being typed: in a field of its own, with the caret after it.
        match &self.hex {
            Some(typed) => {
                let says = format!("#{typed}");
                let field = Rect::new(x - pad / 3, hex.y, hex.right() - x + pad / 3, hex.h);
                canvas.round_fill(field.x, field.y, field.w, field.h, pad / 3, 0x000000, 140);
                canvas.round_outline(field.x, field.y, field.w, field.h, pad / 3, 1, self.settings.accent);
                self.text.draw(canvas, x, y, px, TEXT_COLOR, &says);
                let caret = x + self.text.width(&says, px) + 1;
                canvas.fill(caret, y + 2, 1, Text::line_height(px) - 4, TEXT_COLOR);
            }
            None => {
                let says = hand.map_or("None".to_owned(), |color| format!("#{color:06X}"));
                self.text.draw(canvas, x, y, px, TEXT_COLOR, &says);
            }
        }
        for &(rect, swatch) in &layout.swatches {
            let (cx, cy) = rect.center();
            let radius = side * 3 / 10;
            if self.hover == Target::Swatch(swatch) {
                let inner = rect.inflate(-(px as i32 / 8).max(1));
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, 30);
            }
            let now = hand == Some(swatch);
            let ring = if now { TEXT_COLOR } else { MENU_EDGE_COLOR };
            canvas.disc(cx, cy, radius + (px as i32 / 10).max(1) + i32::from(now), ring, 256);
            canvas.disc(cx, cy, radius, swatch, 256);
        }
        // None: a white disc crossed out in red, ringed when there is none.
        if let Some(rect) = layout.none {
            let (cx, cy) = rect.center();
            let radius = side * 3 / 10;
            if self.hover == Target::NoPaint {
                let inner = rect.inflate(-(px as i32 / 8).max(1));
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, 30);
            }
            let ring = if hand.is_none() { TEXT_COLOR } else { MENU_EDGE_COLOR };
            canvas.disc(cx, cy, radius + (px as i32 / 10).max(1) + i32::from(hand.is_none()), ring, 256);
            canvas.disc(cx, cy, radius, 0xF2F2F2, 256);
            for step in -radius * 7 / 10..radius * 7 / 10 {
                canvas.fill(cx + step - 1, cy - step - 1, 3, 3, RED);
            }
        }
        // How opaque: the color in hand at each step, over a square
        // half light and half dark, to show what comes through.
        for &(rect, opacity) in &layout.opacities {
            let (cx, cy) = rect.center();
            let radius = side * 3 / 10;
            if self.hover == Target::Opacity(opacity) {
                let inner = rect.inflate(-(px as i32 / 8).max(1));
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, 30);
            }
            canvas.fill(cx - radius, cy - radius, radius, 2 * radius, 0xE0E0E0);
            canvas.fill(cx, cy - radius, radius, 2 * radius, 0x404040);
            canvas.disc(cx, cy, radius, color, opacity);
            if opacity == self.opacity {
                canvas.round_outline(cx - radius - 3, cy - radius - 3, 2 * radius + 6, 2 * radius + 6, pad / 2, 2, TEXT_COLOR);
            }
        }
        let row = layout.pipette;
        canvas.fill(popup.x + pad, row.y - pad / 4, popup.w - 2 * pad, 1, MENU_EDGE_COLOR);
        if self.hover == Target::Pipette {
            canvas.round_fill(row.x, row.y, row.w, row.h, pad / 2, self.settings.accent, 256);
        }
        let (main, dim) = if self.hover == Target::Pipette { (on(self.settings.accent), on(self.settings.accent)) } else { (TEXT_COLOR, DIM_TEXT_COLOR) };
        let line = row.y + (row.h - Text::line_height(px)) / 2;
        self.text.draw_centered(canvas, row.x + side / 2, row.y + row.h / 2, px, main, icons::PIPETTE);
        self.text.draw(canvas, row.x + side, line, px, main, "Pick from the screen");
        let key = self.text.width("I", px);
        self.text.draw(canvas, row.right() - pad - key, line, px, dim, "I");
    }

    /// The picture as it will be, framed, drawn small in the box the
    /// frame is chosen in. Made again only when something it is made
    /// from has changed: it is the whole picture each time.
    fn draw_look(&mut self, canvas: &mut Canvas, room: Rect) {
        let made_from = format!("{:?} {:?} {} {} {} {}", self.frame, self.selection, self.corners, self.shapes.len(), self.undo.len(), self.photos);
        if self.look.as_ref().is_none_or(|look| look.0 != made_from) {
            let Some((_, wide, high, pixels, _)) = self.framed() else { return };
            // No larger than the room, and the same shape as it is.
            let times = (room.w as f32 / wide as f32).min(room.h as f32 / high as f32).min(1.0);
            let (w, h) = (((wide as f32 * times) as i32).max(1), ((high as f32 * times) as i32).max(1));
            let small = (0..w * h).map(|at| pixels[((at / w) * high / h * wide + (at % w) * wide / w) as usize]).collect();
            self.look = Some((made_from, w, h, small));
        }
        let Some((_, w, h, small)) = &self.look else { return };
        let (left, top) = (room.x + (room.w - w) / 2, room.y + (room.h - h) / 2);
        for (at, pixel) in small.iter().enumerate() {
            canvas.blend(left + at as i32 % w, top + at as i32 / w, pixel & 0xFF_FFFF, (pixel >> 24) as u8);
        }
    }

    /// What there is to do, said in the middle of the screen while
    /// nothing has been selected.
    fn draw_hint(&mut self, canvas: &mut Canvas, px: u32) {
        let monitor = self.monitor();
        let pad = px as i32 / 2;
        let line = Text::line_height(px);
        let keys: &[(&str, u32)] = &[
            ("Click", TEXT_COLOR),
            (" a window    ", DIM_TEXT_COLOR),
            ("Enter", TEXT_COLOR),
            (" this screen    ", DIM_TEXT_COLOR),
            ("Esc", TEXT_COLOR),
            (" cancel", DIM_TEXT_COLOR),
        ];
        // No word of clicking a window where a click takes none.
        let keys = if self.settings.windows == Windows::Off { &keys[2..] } else { keys };
        let rows: [&[(&str, u32)]; 2] = [&[("Drag", TEXT_COLOR), (" to select an area", DIM_TEXT_COLOR)], keys];
        let widths: Vec<i32> = rows.iter().map(|row| row.iter().map(|(run, _)| self.text.width(run, px)).sum()).collect();
        let width = widths.iter().copied().max().unwrap_or(0) + 4 * pad;
        let height = line * rows.len() as i32 + 2 * pad;
        let (cx, cy) = monitor.center();
        let (left, top) = (cx - width / 2, cy - height / 2);
        canvas.round_fill(left, top, width, height, pad, 0x000000, 176);
        for (index, (row, row_width)) in rows.iter().zip(&widths).enumerate() {
            let mut x = cx - row_width / 2;
            for (run, color) in row.iter() {
                self.text.draw(canvas, x, top + pad + index as i32 * line, px, *color, run);
                x += self.text.width(run, px);
            }
        }
    }

    /// The pixels about the pointer, enlarged, with where it is or the
    /// color it is on.
    fn draw_loupe(&mut self, canvas: &mut Canvas, px: u32) {
        let monitor = self.monitor_at(self.pointer);
        let pad = px as i32 / 2;
        let cell = (px as i32 * 2 / 3).max(6);
        let image = LOUPE_CELLS * cell;
        let line = Text::line_height(px);
        let (width, height) = (image + pad, image + pad + line + pad / 4);
        let (x, y) = self.pointer;
        let away = px as i32 * 3 / 2;
        let left = if x + away + width <= monitor.right() { x + away } else { x - away - width };
        let top = if y + away + height <= monitor.bottom() { y + away } else { y - away - height };
        let place = Rect::new(left, top, width, height).kept_within(monitor);
        panel(canvas, place, pad / 2);
        let (ix, iy) = (place.x + pad / 2, place.y + pad / 2);
        canvas.enlarge(&self.shot.pixels, self.shot.width, x, y, ix, iy, LOUPE_CELLS, cell);
        // The pixel the pointer is on, framed in white within black so
        // that it shows on anything.
        let middle = Rect::new(ix + LOUPE_CELLS / 2 * cell, iy + LOUPE_CELLS / 2 * cell, cell, cell);
        let frame = (cell / 8).max(1);
        let outer = middle.inflate(2 * frame);
        canvas.round_outline(outer.x, outer.y, outer.w, outer.h, 0, frame, 0x000000);
        let inner = middle.inflate(frame);
        canvas.round_outline(inner.x, inner.y, inner.w, inner.h, 0, frame, 0xFFFFFF);
        // The selection's edges where they are in it, between the
        // pixels they are between: what is in and what is out, to the
        // pixel.
        if let (Some(area), false) = (self.selection, self.pipette) {
            let (half, thick) = (LOUPE_CELLS / 2, (cell / 6).max(2));
            canvas.set_clip(Rect::new(ix, iy, image, image));
            for edge in [area.x, area.right()] {
                let column = edge - (x - half);
                if (0..=LOUPE_CELLS).contains(&column) {
                    canvas.fill(ix + column * cell - thick / 2, iy, thick, image, self.settings.accent);
                }
            }
            for edge in [area.y, area.bottom()] {
                let row = edge - (y - half);
                if (0..=LOUPE_CELLS).contains(&row) {
                    canvas.fill(ix, iy + row * cell - thick / 2, image, thick, self.settings.accent);
                }
            }
            canvas.reset_clip();
        }

        let under = self.shot.pixels[(y * self.shot.width + x) as usize] & 0xFFFFFF;
        let row = iy + image + pad / 4;
        if self.pipette {
            let words = format!("#{under:06X}");
            let dot = line / 3;
            canvas.disc(ix + dot + 1, row + line / 2, dot + 1, MENU_EDGE_COLOR, 256);
            canvas.disc(ix + dot + 1, row + line / 2, dot, under, 256);
            self.text.draw(canvas, ix + 2 * dot + pad, row, px, TEXT_COLOR, &words);
        } else {
            let words = format!("{x}, {y}");
            let across = self.text.width(&words, px);
            self.text.draw(canvas, place.x + (place.w - across) / 2, row, px, DIM_TEXT_COLOR, &words);
        }
    }

    /// What it takes to take this picture again, as `--again` does: the
    /// area, and how it is set out, but not what is drawn on it.
    pub fn recipe(&self) -> crate::history::Recipe {
        // On a photograph only where one came: one asked for that never
        // did is no backdrop at all.
        let photo = self.photo().map(crate::history::photo_mark);
        let backdrop = if self.frame.backdrop == Backdrop::Photo && photo.is_none() { Backdrop::None } else { self.frame.backdrop };
        crate::history::Recipe { area: self.selection.unwrap_or_default(), corners: self.corners, frame: Frame { backdrop, ..self.frame }, pointer: self.arrow_shows && self.arrow.is_some(), photo }
    }

    /// What the windows are called, and where each is, the topmost first.
    pub fn set_titles(&mut self, titles: Vec<(Rect, String)>) {
        (self.titles, self.dirty) = (titles, true);
    }

    /// What the window the selection is of is called: the topmost one
    /// that is most of what is selected, and not much more.
    fn title(&self) -> Option<String> {
        let area = self.selection?;
        let size = |rect: Rect| rect.w as i64 * rect.h as i64;
        let of = self.titles.iter().find(|(place, _)| {
            let both = size(place.intersect(area));
            both * 10 >= size(area) * 8 && size(*place) * 2 <= size(area) * 3
        });
        of.map(|(_, title)| title.clone())
    }

    /// Sets the picture on a photograph from before, in the frame it
    /// already has, the room round it as it is.
    pub fn set_photo(&mut self, photo: Photo) {
        (self.photo, self.frame.backdrop, self.look, self.dirty) = (Some(photo), Backdrop::Photo, None, true);
        self.photos += 1;
    }

    /// The photograph the picture is set on, if it is set on one.
    pub fn photo(&self) -> Option<&Photo> {
        self.photo.as_ref().filter(|_| self.frame.backdrop == Backdrop::Photo)
    }

    /// What it takes to come back to this picture and go on with it:
    /// the area, what was there before anything was drawn on it, and
    /// the shapes, a line of text each.
    pub fn keepsake(&self) -> Option<(Rect, Vec<u32>, String)> {
        let area = self.selection?;
        let shapes: Vec<String> = self.shapes.iter().map(Shape::write).collect();
        Some((area, canvas::crop(&self.shot.pixels, self.shot.width, area), shapes.join("\n")))
    }

    /// Puts back the shapes of a picture come back to.
    pub fn restore(&mut self, shapes: Vec<Shape>) {
        self.shapes = shapes;
        self.dirty = true;
    }

    /// What is in hand now, as settings to begin with next time: the
    /// color, the width of line, how round a rectangle's corners and
    /// the picture's are, whether shapes are filled and in what and with
    /// a border, and the face.
    pub fn in_hand(&self) -> Settings {
        // A photograph is not kept: it was one nobody chose, or is the
        // user's own to choose again.
        let frame = Frame { backdrop: if self.frame.backdrop == Backdrop::Photo { Backdrop::None } else { self.frame.backdrop }, ..self.frame };
        Settings { color: self.color, size: self.size, corners: self.corners, filled: self.filled, fill: Some(self.fill), border: Some(self.bordered), face: self.face, frame, place: self.place, ..self.settings.clone() }
    }

    /// The last of the three says whether its corners are rounded: then
    /// the top byte of each pixel is how opaque it is, which outside the
    /// curve of a corner is not at all.
    pub fn result(&mut self) -> Option<(Rect, Vec<u32>, bool)> {
        self.commit_text();
        let selection = self.selection?;
        let arrow = self.arrow.as_ref().filter(|_| self.arrow_shows);
        let mut pixels = if self.shapes.is_empty() {
            let mut pixels = canvas::crop(&self.shot.pixels, self.shot.width, selection);
            if let Some(arrow) = arrow {
                arrow.stamp(&mut pixels, selection.w, (selection.x, selection.y));
            }
            pixels
        } else {
            // Drawn on a copy of the whole screen, where the shapes are
            // already in the right place, and then cut out of it. The
            // mouse pointer first, which is under them.
            let mut whole = self.shot.pixels.to_vec();
            if let Some(arrow) = arrow {
                arrow.stamp(&mut whole, self.shot.width, (0, 0));
            }
            let mut canvas = Canvas::new(&mut whole, self.shot.width, self.shot.height);
            shapes::paint(&self.shapes, &mut canvas, &mut self.text, selection);
            canvas::crop(&whole, self.shot.width, selection)
        };
        let round = self.corner_radius();
        if round > 0 {
            canvas::round_alpha(&mut pixels, selection.w, round);
        }
        Some((selection, pixels, round > 0))
    }
}

/// A color as its hue, of 360, and its saturation and value, of 1.
fn to_hsv(color: u32) -> (f32, f32, f32) {
    let [r, g, b] = [16, 8, 0].map(|shift| (color >> shift & 0xFF) as f32 / 255.0);
    let (most, least) = (r.max(g).max(b), r.min(g).min(b));
    let spread = most - least;
    let hue = if spread == 0.0 {
        0.0
    } else if most == r {
        60.0 * ((g - b) / spread).rem_euclid(6.0)
    } else if most == g {
        60.0 * ((b - r) / spread + 2.0)
    } else {
        60.0 * ((r - g) / spread + 4.0)
    };
    (hue, if most == 0.0 { 0.0 } else { spread / most }, most)
}

/// The color of a hue, saturation and value.
fn from_hsv((hue, saturation, value): (f32, f32, f32)) -> u32 {
    let part = |n: f32| {
        let k = (n + hue / 60.0).rem_euclid(6.0);
        let v = value - value * saturation * k.min(4.0 - k).clamp(0.0, 1.0);
        (v.clamp(0.0, 1.0) * 255.0).round() as u32
    };
    part(5.0) << 16 | part(3.0) << 8 | part(1.0)
}

/// A lighter tint of a color, for thin lines that the color itself
/// would be hard to see as.
fn soft(color: u32) -> u32 {
    let toward_white = |shift: u32| {
        let channel = color >> shift & 0xFF;
        (channel + (255 - channel) * 45 / 100) << shift
    };
    toward_white(16) | toward_white(8) | toward_white(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Mods = Mods { shift: false, ctrl: false, alt: false };

    /// A screen with something on it to tell places apart by: bands of
    /// gray, and two windows.
    fn editor(width: i32, height: i32) -> Editor {
        let mut pixels = vec![0u32; (width * height) as usize];
        for (at, pixel) in pixels.iter_mut().enumerate() {
            let (x, y) = (at as i32 % width, at as i32 / width);
            let level = (0x30 + (x / 40 + y / 40) % 2 * 0x20) as u32;
            *pixel = level << 16 | level << 8 | level;
        }
        let windows = vec![Rect::new(100, 80, 600, 400), Rect::new(500, 300, 700, 500)];
        for (window, color) in windows.iter().zip([0xF4F4F4, 0x24283B]) {
            let mut canvas = Canvas::new(&mut pixels, width, height);
            canvas.fill(window.x, window.y, window.w, window.h, color);
            canvas.fill(window.x, window.y, window.w, 28, 0x3A3F55);
        }
        let mut dim = pixels.clone();
        canvas::darken_into(&mut dim, &pixels, 128);
        let shot = Shot { width, height, pixels: Box::new(pixels) };
        // The windows are listed topmost first.
        let windows = windows.into_iter().rev().collect();
        Editor::new(shot, dim, vec![Rect::new(0, 0, width, height)], windows, Settings::default(), Text::load())
    }

    fn drag(editor: &mut Editor, from: (i32, i32), to: (i32, i32)) {
        editor.pointer_moved(from.0, from.1, PLAIN);
        editor.pressed(0);
        editor.pointer_moved((from.0 + to.0) / 2, (from.1 + to.1) / 2, PLAIN);
        editor.pointer_moved(to.0, to.1, PLAIN);
        editor.released();
    }

    fn click(editor: &mut Editor, at: (i32, i32), time: u32) -> Outcome {
        editor.pointer_moved(at.0, at.1, PLAIN);
        editor.pressed(time);
        editor.released()
    }

    /// Where a button of the bar is, or of the bar over what is picked out.
    fn button(editor: &mut Editor, button: Button) -> (i32, i32) {
        let layout = editor.layout();
        layout.buttons.iter().chain(&layout.tray_buttons).find(|(_, other)| *other == button).map(|(rect, _)| rect.center()).expect("the bar is up")
    }

    /// What the bar over what is picked out has, in order.
    fn tray(editor: &mut Editor) -> Vec<Button> {
        editor.layout().tray_buttons.iter().map(|(_, button)| *button).collect()
    }

    #[test]
    fn a_drag_selects_and_a_click_takes_the_window_under_it() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (200, 150), (500, 350));
        assert_eq!(editor.selection(), Some(Rect::new(200, 150, 301, 201)));
        // Dragged the other way, it is the same area.
        let mut editor = self::editor(1600, 900);
        drag(&mut editor, (500, 350), (200, 150));
        assert_eq!(editor.selection(), Some(Rect::new(200, 150, 301, 201)));
        // A click takes the topmost window there.
        let mut editor = self::editor(1600, 900);
        click(&mut editor, (600, 400), 0);
        assert_eq!(editor.selection(), Some(Rect::new(500, 300, 700, 500)));
        let mut editor = self::editor(1600, 900);
        click(&mut editor, (150, 100), 0);
        assert_eq!(editor.selection(), Some(Rect::new(100, 80, 600, 400)));
        // And where there is none, nothing.
        let mut editor = self::editor(1600, 900);
        click(&mut editor, (1500, 50), 0);
        assert_eq!(editor.selection(), None);

        // Until something is selected the screen is dark all over, the
        // window under the pointer too, which has only a line round it;
        // unless it is wanted bright.
        let mut out = vec![0u32; 1600 * 900];
        let mut editor = self::editor(1600, 900);
        editor.pointer_moved(150, 300, PLAIN);
        editor.draw(&mut out);
        assert_eq!((out[300 * 1600 + 400], out[80 * 1600 + 400]), (0x7A7A7A, Settings::default().accent));
        editor.settings.windows = Windows::Bright;
        editor.draw(&mut out);
        assert_eq!(out[300 * 1600 + 400], 0xF4F4F4);
    }

    #[test]
    fn the_selection_is_moved_resized_and_nudged() {
        let mut editor = editor(1600, 900);
        // With no edges of windows to go to: this is about where the
        // selection is put, to the pixel.
        editor.edges = (Vec::new(), Vec::new());
        drag(&mut editor, (200, 150), (499, 349));
        drag(&mut editor, (300, 200), (350, 260));
        assert_eq!(editor.selection(), Some(Rect::new(250, 210, 300, 200)));
        // By its bottom-right corner, and then its left edge.
        drag(&mut editor, (550, 410), (600, 500));
        assert_eq!(editor.selection(), Some(Rect::new(250, 210, 350, 290)));
        drag(&mut editor, (250, 300), (230, 320));
        assert_eq!(editor.selection(), Some(Rect::new(230, 210, 370, 290)));
        editor.key(Key::Right, PLAIN);
        editor.key(Key::Down, Mods { shift: true, ctrl: false, alt: false });
        assert_eq!(editor.selection(), Some(Rect::new(231, 220, 370, 290)));
        editor.key(Key::Left, Mods { shift: false, ctrl: true, alt: false });
        assert_eq!(editor.selection(), Some(Rect::new(231, 220, 369, 290)));
        // It cannot be moved off the screen.
        drag(&mut editor, (400, 300), (-500, -500));
        assert_eq!(editor.selection(), Some(Rect::new(0, 0, 369, 290)));
    }

    #[test]
    fn an_edge_of_the_selection_goes_to_where_what_is_on_the_screen_changes() {
        // A light panel on a dark screen, from 300 to 700 across and
        // 200 to 500 down; and a faint speck that is no edge.
        let (width, height) = (1000, 800);
        let mut pixels = vec![0x202020u32; (width * height) as usize];
        for y in 200..500 {
            for x in 300..700 {
                pixels[(y * width + x) as usize] = 0xE0E0E0;
            }
        }
        pixels[(350 * width + 310) as usize] = 0x262626;
        // Down the screen, near 296 or 704: the panel's sides; across,
        // near 503: its bottom. Not where nothing changes, nor beyond
        // reach, nor where the rows looked at are mostly not the panel's.
        assert_eq!(content_edge(&pixels, width, height, true, 296, (220, 480), 8), Some(300));
        assert_eq!(content_edge(&pixels, width, height, true, 704, (220, 480), 8), Some(700));
        assert_eq!(content_edge(&pixels, width, height, false, 503, (320, 680), 8), Some(500));
        assert_eq!(content_edge(&pixels, width, height, true, 150, (220, 480), 8), None);
        assert_eq!(content_edge(&pixels, width, height, true, 285, (220, 480), 8), None);
        assert_eq!(content_edge(&pixels, width, height, true, 296, (100, 790), 8), Some(300));
        assert_eq!(content_edge(&pixels, width, height, true, 296, (0, 210), 8), None);

        // Dragged out by hand to a few pixels off the panel, it is the
        // panel; with Ctrl, it is where it was put.
        let fresh = || {
            let shot = Shot { width, height, pixels: Box::new(pixels.clone()) };
            Editor::new(shot, pixels.clone(), vec![Rect::new(0, 0, width, height)], Vec::new(), Settings::default(), Text::load())
        };
        let mut editor = fresh();
        drag(&mut editor, (303, 204), (696, 497));
        assert_eq!(editor.selection(), Some(Rect::new(300, 200, 400, 300)));
        // The other way about, from the bottom-right corner.
        let mut editor = fresh();
        drag(&mut editor, (697, 496), (304, 203));
        assert_eq!(editor.selection(), Some(Rect::new(300, 200, 400, 300)));
        let mut editor = fresh();
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        editor.pointer_moved(303, 204, ctrl);
        editor.pressed(5000);
        editor.pointer_moved(500, 300, ctrl);
        editor.pointer_moved(696, 497, ctrl);
        editor.released();
        assert_eq!(editor.selection(), Some(Rect::new(303, 204, 394, 294)));
        // A side pulled out from the panel comes back to it, and the
        // loupe is up while it is pulled, and not after.
        let mut editor = fresh();
        drag(&mut editor, (303, 204), (696, 497));
        editor.pointer_moved(700, 350, PLAIN);
        editor.pressed(9000);
        editor.pointer_moved(705, 350, PLAIN);
        editor.pointer_moved(706, 350, PLAIN);
        // Up only when X has been pressed for it, until it is again.
        assert!(!editor.loupe_shows());
        editor.key(Key::Char('x'), PLAIN);
        assert!(editor.loupe_shows());
        editor.key(Key::Char('x'), PLAIN);
        assert!(!editor.loupe_shows());
        editor.released();
        assert_eq!(editor.selection().map(|area| area.right()), Some(700));
        // And a selection moved goes to the panel's edges as its own
        // come near them, all of it with them.
        let mut editor = fresh();
        drag(&mut editor, (100, 100), (299, 199));
        assert_eq!(editor.selection(), Some(Rect::new(100, 100, 200, 100)));
        drag(&mut editor, (150, 150), (353, 256));
        assert_eq!(editor.selection(), Some(Rect::new(300, 200, 200, 100)));
        // Ctrl held as it moves leaves it where it is put. (Held as it
        // is pressed, it would be a box to pick shapes out with.)
        let ctrl = Mods { ctrl: true, ..PLAIN };
        editor.pointer_moved(400, 250, PLAIN);
        editor.pressed(20_000);
        editor.pointer_moved(401, 251, ctrl);
        editor.pointer_moved(404, 253, ctrl);
        editor.released();
        assert_eq!(editor.selection(), Some(Rect::new(304, 203, 200, 100)));
    }

    #[test]
    fn alt_draws_and_resizes_about_the_middle_and_shift_keeps_to_one_way() {
        let (alt, shift) = (Mods { alt: true, ..PLAIN }, Mods { shift: true, ..PLAIN });
        // Pressed as it is, and held as it moves: Shift as it is pressed
        // is something else, a box to pick shapes out with, or a shape
        // added to those picked out.
        let held = |editor: &mut Editor, from: (i32, i32), to: (i32, i32), mods: Mods| {
            let pressed = if mods.shift { PLAIN } else { mods };
            editor.pointer_moved(from.0, from.1, pressed);
            editor.pressed(0);
            editor.pointer_moved((from.0 + to.0) / 2, (from.1 + to.1) / 2, mods);
            editor.pointer_moved(to.0, to.1, mods);
            editor.released();
        };
        let mut editor = editor(1600, 900);
        editor.edges = (Vec::new(), Vec::new());
        // The selection, dragged out with Alt, is about where it began.
        // (Over the plain left of the screen, where nothing is to snap to.)
        let mut plain = vec![0x303030u32; 1600 * 900];
        let shot = Shot { width: 1600, height: 900, pixels: Box::new(plain.clone()) };
        plain.iter_mut().for_each(|pixel| *pixel = 0x181818);
        let mut editor2 = Editor::new(shot, plain, vec![Rect::new(0, 0, 1600, 900)], Vec::new(), Settings::default(), Text::load());
        held(&mut editor2, (500, 400), (600, 450), alt);
        assert_eq!(editor2.selection(), Some(Rect::new(400, 350, 201, 101)));
        // A side pulled with Alt takes the other with it, the other way.
        held(&mut editor2, (601, 400), (621, 400), alt);
        assert_eq!(editor2.selection(), Some(Rect::new(380, 350, 241, 101)));
        // Moved with Shift, it goes only the way it has gone further.
        held(&mut editor2, (500, 400), (560, 410), shift);
        assert_eq!(editor2.selection(), Some(Rect::new(440, 350, 241, 101)));

        // A rectangle drawn with Alt is about where the drag began.
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('r'), PLAIN);
        held(&mut editor, (400, 400), (450, 430), alt);
        assert_eq!(editor.shapes[0].frame(), (350.5, 370.5, 100.0, 60.0));
        // Taken by a corner with Alt, it grows about its middle; by a
        // side with Shift, the other way grows alike about its middle.
        let frame = editor.frame_of(0);
        let corner = grip_at(frame, 1, 1);
        held(&mut editor, corner, (corner.0 + 10, corner.1 + 10), alt);
        let (left, top, width, height) = editor.shapes[0].frame();
        assert!((left + width / 2.0 - 400.5).abs() < 0.6 && (top + height / 2.0 - 400.5).abs() < 0.6, "{left} {top} {width} {height}");
        assert!(width > 100.0 && height > 60.0);
        let before = editor.shapes[0].frame();
        let frame = editor.frame_of(0);
        let side = grip_at(frame, 1, 0);
        held(&mut editor, side, (side.0 + 30, side.1), shift);
        let (left, top, width, height) = editor.shapes[0].frame();
        assert!(width > before.2 && height > before.3, "{:?}", editor.shapes[0].frame());
        assert!((width / height - before.2 / before.3).abs() < 0.02 && (top + height / 2.0 - (before.1 + before.3 / 2.0)).abs() < 0.6, "{left} {top} {width} {height}");
        // A shape moved with Shift goes only one way.
        let (left, top, _, _) = editor.shapes[0].frame();
        let inside = (left as i32 + 5, top as i32 + 5);
        editor.key(Key::Char('v'), PLAIN);
        held(&mut editor, inside, (inside.0 + 40, inside.1 + 7), shift);
        let moved = editor.shapes[0].frame();
        assert_eq!((moved.0, moved.1), (left + 40.0, top));
    }

    #[test]
    fn the_selection_lines_up_with_something_above_it_and_says_so() {
        // A light panel at the top, from 300 to 700 across; nothing
        // under it but the dark.
        let (width, height) = (1000, 800);
        let mut pixels = vec![0x202020u32; (width * height) as usize];
        for y in 100..200 {
            for x in 300..700 {
                pixels[(y * width + x) as usize] = 0xE0E0E0;
            }
        }
        let fresh = || {
            let shot = Shot { width, height, pixels: Box::new(pixels.clone()) };
            Editor::new(shot, pixels.clone(), vec![Rect::new(0, 0, width, height)], Vec::new(), Settings::default(), Text::load())
        };
        // Dragged out below it, a few pixels in from where its sides
        // are: the left goes to the panel's left, and a red line runs
        // from the panel down to the selection's bottom, while it is
        // being dragged; the right, by the pointer, nowhere near the
        // panel's, stays.
        let mut editor = fresh();
        editor.pointer_moved(304, 400, PLAIN);
        editor.pressed(0);
        editor.pointer_moved(400, 450, PLAIN);
        editor.pointer_moved(600, 500, PLAIN);
        assert_eq!(editor.selection(), Some(Rect::new(300, 400, 301, 101)));
        assert_eq!(editor.lines, [(true, 300, 100, 501)]);
        editor.released();
        assert!(editor.lines.is_empty());
        // Moved about, all of it goes to the line, by its right side.
        let mut editor = fresh();
        drag(&mut editor, (100, 400), (199, 499));
        editor.pointer_moved(150, 450, PLAIN);
        editor.pressed(5000);
        editor.pointer_moved(400, 450, PLAIN);
        editor.pointer_moved(647, 450, PLAIN);
        assert_eq!((editor.selection(), editor.lines.clone()), (Some(Rect::new(600, 400, 100, 100)), vec![(true, 700, 100, 500)]));
        // With Ctrl, it is left where it is, and nothing is said.
        editor.pointer_moved(645, 450, Mods { ctrl: true, ..PLAIN });
        assert!(editor.lines.is_empty() && editor.selection() == Some(Rect::new(595, 400, 100, 100)));
        editor.released();
    }

    #[test]
    fn the_selection_and_the_frame_keep_their_shape_with_shift_or_when_told_to() {
        let shift = Mods { shift: true, ctrl: false, alt: false };
        let pull = |editor: &mut Editor, from: (i32, i32), to: (i32, i32)| {
            editor.pointer_moved(from.0, from.1, PLAIN);
            editor.pressed(0);
            editor.pointer_moved(to.0, to.1, shift);
            editor.released();
            editor.pointer_moved(to.0, to.1, PLAIN);
        };
        let mut editor = editor(1600, 900);
        editor.edges = (Vec::new(), Vec::new());
        drag(&mut editor, (200, 150), (599, 349));
        // By a corner with Shift, twice as wide as tall as it was; by
        // an edge, and the middle of the edges beside it stays put.
        pull(&mut editor, (600, 350), (700, 360));
        assert_eq!(editor.selection(), Some(Rect::new(200, 150, 500, 250)));
        pull(&mut editor, (700, 275), (800, 275));
        assert_eq!(editor.selection(), Some(Rect::new(200, 125, 600, 300)));
        // Dragged out with Shift, an area is square.
        pull(&mut editor, (1000, 500), (1100, 540));
        assert_eq!(editor.selection(), Some(Rect::new(1000, 500, 101, 101)));

        // Told to keep a shape, it does with no Shift, whichever way it
        // is dragged, and is no larger than the screen has room for.
        editor.settings.aspect = Some((16, 9));
        drag(&mut editor, (100, 100), (419, 150));
        assert_eq!(editor.selection(), Some(Rect::new(100, 100, 320, 180)));
        // With Ctrl, which leaves the edges where they are put: the
        // bands of the screen here are edges to go to otherwise.
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        editor.pointer_moved(800, 700, ctrl);
        editor.pressed(5000);
        editor.pointer_moved(600, 695, ctrl);
        editor.pointer_moved(481, 690, ctrl);
        editor.released();
        assert_eq!(editor.selection(), Some(Rect::new(481, 521, 320, 180)));
        drag(&mut editor, (1500, 100), (1599, 899));
        assert_eq!(editor.selection(), Some(Rect::new(1500, 100, 100, 56)));

        // Told a size, a press puts that much down round the pointer,
        // all of it on the screen, and a drag takes it elsewhere.
        (editor.settings.aspect, editor.settings.fixed) = (None, Some((300, 200)));
        click(&mut editor, (1200, 700), 50_000);
        assert_eq!(editor.selection(), Some(Rect::new(1050, 600, 300, 200)));
        drag(&mut editor, (30, 30), (130, 80));
        assert_eq!(editor.selection(), Some(Rect::new(100, 50, 300, 200)));

        // The frame, made wider with Shift, is made taller with it.
        let mut editor = self::editor(1600, 900);
        drag(&mut editor, (400, 300), (799, 599));
        editor.choose(Cell::Backdrop(7));
        let shape = editor.frame_shape();
        pull(&mut editor, (380, 400), (340, 400));
        assert_eq!(editor.frame.pad.0, PADS[2] + 40);
        assert!(editor.frame.pad.1 > PADS[2] + 20 && (editor.frame_shape() - shape).abs() < 0.01, "{:?}", editor.frame.pad);
    }

    #[test]
    fn shapes_are_drawn_undone_and_end_up_in_the_picture() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        assert_eq!(editor.key(Key::Char('r'), PLAIN), Outcome::Continue);
        drag(&mut editor, (200, 200), (400, 300));
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (500, 500), (700, 600));
        assert_eq!(editor.shapes.len(), 2);
        // A click with a drawing tool draws nothing, and leaves nothing
        // to undo.
        click(&mut editor, (600, 300), 5000);
        assert_eq!((editor.shapes.len(), editor.undo.len()), (2, 2));

        let (area, pixels, rounded) = editor.result().unwrap();
        assert_eq!((area, rounded), (Rect::new(100, 100, 800, 600), false));
        let at = |x: i32, y: i32| pixels[((y - area.y) * area.w + x - area.x) as usize];
        // The rectangle's edge and the arrow's shaft are in red; the
        // middle of the rectangle is as it was.
        assert_eq!(at(300, 200), RED);
        assert_eq!(at(600, 550), RED);
        assert_eq!(at(300, 250), 0xF4F4F4);

        // With its corners rounded, the picture is see-through outside
        // the curve of each and nowhere else; K goes round to square.
        editor.key(Key::Char('k'), PLAIN);
        let (area, pixels, rounded) = editor.result().unwrap();
        assert!(rounded);
        let alpha = |x: i32, y: i32| pixels[(y * area.w + x) as usize] >> 24;
        assert_eq!((alpha(0, 0), alpha(area.w - 1, area.h - 1), alpha(4, 4), alpha(400, 0), alpha(400, 300)), (0, 0, 255, 255, 255));
        for _ in 0..3 {
            editor.key(Key::Char('k'), PLAIN);
        }
        assert!(!editor.result().unwrap().2);

        // The dot inside a corner of the selection does the same by
        // hand: taken forty pixels further in, the corners are forty
        // pixels round; and taken back out, square again.
        editor.selected.clear();
        let (_, _, dot) = editor.curves().unwrap()[3];
        drag(&mut editor, dot, (dot.0 - 40, dot.1 - 40));
        assert_eq!((editor.corner_radius(), editor.selection(), editor.shapes.len()), (40, Some(Rect::new(100, 100, 800, 600)), 2));
        let (_, pixels, rounded) = editor.result().unwrap();
        assert!(rounded && pixels[0] >> 24 == 0 && pixels[20 * 800 + 20] >> 24 == 255);
        let (_, _, dot) = editor.curves().unwrap()[0];
        drag(&mut editor, dot, (dot.0 - 90, dot.1 - 90));
        assert_eq!(editor.corner_radius(), 0);

        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        assert!(editor.shapes.is_empty());
        editor.key(Key::Char('Z'), Mods { shift: true, ctrl: true, alt: false });
        assert_eq!(editor.shapes.len(), 1);
        assert_eq!(editor.key(Key::Enter, PLAIN), Outcome::Finish(Finish { copy: true, ..Finish::NONE }));
    }

    #[test]
    fn text_is_typed_where_clicked_and_a_shape_is_moved_recolored_and_deleted() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('t'), PLAIN);
        click(&mut editor, (300, 300), 0);
        for letter in "Hi a".chars() {
            // Letters go into the text, and are not taken for tools.
            editor.key(Key::Char(letter), PLAIN);
        }
        editor.key(Key::Backspace, PLAIN);
        // Enter is a new line of it, and Ctrl+Enter the end of it, which
        // takes no picture.
        assert_eq!(editor.key(Key::Enter, PLAIN), Outcome::Continue);
        editor.key(Key::Char('x'), PLAIN);
        assert_eq!(editor.key(Key::Enter, Mods { shift: false, ctrl: true, alt: false }), Outcome::Continue);
        // Done, it is picked out, and the tool in hand is the move tool.
        assert_eq!((editor.shapes.len(), editor.shapes[0].text.as_str(), editor.tool, editor.selected.clone()), (1, "Hi \nx", Tool::Select, vec![0]));
        // Begun and left with nothing typed, it is as if it never was.
        editor.key(Key::Char('t'), PLAIN);
        click(&mut editor, (500, 500), 5000);
        editor.key(Key::Escape, PLAIN);
        assert_eq!((editor.shapes.len(), editor.undo.len()), (1, 1));

        editor.key(Key::Char('v'), PLAIN);
        let before = editor.shapes[0].points[0];
        drag(&mut editor, (305, 300), (345, 330));
        let after = editor.shapes[0].points[0];
        assert_eq!((after.0 - before.0, after.1 - before.1), (40.0, 30.0));
        assert_eq!(editor.selected, [0]);
        // The colors: opened from the bar, and one of them clicked.
        let color = button(&mut editor, Button::Color);
        click(&mut editor, color, 9000);
        let swatch = editor.layout().swatches[4].0.center();
        click(&mut editor, swatch, 12000);
        assert_eq!((editor.shapes[0].color, editor.popup), (PALETTE[4].0, None));
        editor.key(Key::Delete, PLAIN);
        assert!(editor.shapes.is_empty());
    }

    #[test]
    fn what_is_drawn_is_picked_out_to_be_resized_and_have_its_corners_rounded() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (200, 200), (400, 300));
        // Drawn, it is picked out, and the move tool is in hand: another
        // drag moves it, and does not draw a second.
        assert_eq!((editor.tool, editor.selected.clone(), editor.shapes.len()), (Tool::Select, vec![0], 1));
        drag(&mut editor, (300, 200), (310, 220));
        assert_eq!((editor.shapes.len(), editor.shapes[0].frame()), (1, (210.5, 220.5, 200.0, 100.0)));
        // Picked out, it is moved by its middle too, hollow as it is;
        // and the selection it is on stays where it was.
        drag(&mut editor, (300, 260), (290, 240));
        assert_eq!((editor.shapes[0].frame(), editor.selection()), ((200.5, 200.5, 200.0, 100.0), Some(Rect::new(100, 100, 800, 600))));
        drag(&mut editor, (300, 250), (310, 270));

        // By the bottom-right corner of its box, to twice the size: the
        // top-left corner of the box stays where it is.
        let frame = editor.frame_of(0);
        drag(&mut editor, (frame.right(), frame.bottom()), (frame.right() + frame.w, frame.bottom() + frame.h));
        let (left, top, width, height) = editor.shapes[0].frame();
        assert_eq!((width, height), (400.0, 200.0));
        assert_eq!((left - frame.x as f32, top - frame.y as f32), (2.0 * (210.5 - frame.x as f32), 2.0 * (220.5 - frame.y as f32)));

        // By the dot in its top-left corner, thirty pixels further in:
        // the corners are thirty pixels rounder, and no rounder than
        // half the shorter side however far it is taken.
        let before = editor.shapes[0].radius;
        let (_, _, dot) = editor.rounders(0).unwrap()[0];
        drag(&mut editor, dot, (dot.0 + 30, dot.1 + 30));
        assert_eq!(editor.shapes[0].radius, before + 30.0);
        // The next rectangle drawn has corners as round as that.
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (150, 500), (350, 650));
        assert_eq!(editor.shapes[1].radius, before + 30.0);
        editor.key(Key::Delete, PLAIN);
        let (left, top, width, _) = editor.shapes[0].frame();
        click(&mut editor, ((left + width / 2.0) as i32, top as i32), 15_000);
        assert_eq!(editor.selected, [0]);
        let (_, _, dot) = editor.rounders(0).unwrap()[0];
        drag(&mut editor, dot, (dot.0 + 500, dot.1 + 500));
        assert_eq!(editor.shapes[0].radius, 100.0);
        // Each of those is a step that undo takes back.
        assert_eq!(editor.undo.len(), 9);

        // The pointer over a shape that is not the one picked out shows
        // that one's box; a pen stroke leaves the pen in hand.
        editor.key(Key::Char('p'), PLAIN);
        drag(&mut editor, (700, 600), (760, 640));
        assert_eq!((editor.tool, editor.selected.len()), (Tool::Pen, 0));
        // So does a counter, and each one put down counts one more.
        editor.key(Key::Char('n'), PLAIN);
        for (at, time) in [((150, 150), 20_000), ((150, 250), 30_000), ((150, 350), 40_000)] {
            click(&mut editor, at, time);
        }
        let numbers: Vec<u32> = editor.shapes.iter().filter(|shape| shape.tool == Tool::Counter).map(|shape| shape.number).collect();
        assert_eq!((editor.tool, numbers), (Tool::Counter, vec![1, 2, 3]));
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        editor.key(Key::Char('v'), PLAIN);
        editor.pointer_moved(730, 620, PLAIN);
        assert_eq!(editor.hovered, Some(1));
        editor.pointer_moved(150, 650, PLAIN);
        assert_eq!(editor.hovered, None);

        // A click picks one out; with Shift or Ctrl it adds another, or
        // takes one away; and several picked out are moved, recolored
        // and deleted together.
        let shift = Mods { shift: true, ctrl: false, alt: false };
        let (left, top, _, height) = editor.shapes[0].frame();
        let edge = (left as i32, (top + height / 2.0) as i32);
        click(&mut editor, edge, 50_000);
        assert_eq!(editor.selected, [0]);
        editor.pointer_moved(730, 620, shift);
        editor.pressed(60_000);
        editor.released();
        assert_eq!(editor.selected, [0, 1]);
        let before = (editor.shapes[0].points[0], editor.shapes[1].points[0]);
        // (With Ctrl held, so that they go where they are put and not to
        // the edge of something a pixel off.)
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        editor.pointer_moved(730, 620, PLAIN);
        editor.pressed(65_000);
        editor.pointer_moved(735, 610, ctrl);
        editor.pointer_moved(740, 600, ctrl);
        editor.released();
        let after = (editor.shapes[0].points[0], editor.shapes[1].points[0]);
        assert_eq!((after.0 .0 - before.0 .0, after.0 .1 - before.0 .1, after.1 .0 - before.1 .0, after.1 .1 - before.1 .1), (10.0, -20.0, 10.0, -20.0));
        editor.set_color(PALETTE[10].0);
        assert_eq!((editor.shapes[0].color, editor.shapes[1].color), (PALETTE[10].0, PALETTE[10].0));
        editor.pointer_moved(740, 600, Mods { shift: false, ctrl: true, alt: false });
        editor.pressed(70_000);
        editor.released();
        assert_eq!(editor.selected, [0]);
        editor.pointer_moved(740, 600, shift);
        editor.pressed(80_000);
        editor.released();
        editor.key(Key::Delete, PLAIN);
        assert!(editor.shapes.is_empty() && editor.selected.is_empty());
    }

    #[test]
    fn lines_are_reshaped_by_their_ends_and_shapes_copied_and_picked_out_by_a_box() {
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (200, 200), (400, 300));
        // An arrow picked out has its two ends, and no box to be resized
        // by: its head is taken elsewhere, and its tail stays.
        assert_eq!(editor.ends(0), [(0, (201, 201)), (1, (401, 301)), (2, (301, 251))]);
        assert_eq!(editor.target_at((400, 300)), Target::End(1));
        drag(&mut editor, (400, 300), (500, 250));
        assert_eq!(editor.shapes[0].points, [(200.5, 200.5), (500.5, 250.5)]);

        // A click on an end, where a drag takes it elsewhere, gives it
        // a head or takes its head away: one at each end, here, and the
        // next arrow drawn has the same. Each click is a step to undo.
        assert_eq!(editor.shapes[0].heads, [false, true]);
        click(&mut editor, (200, 200), 40_000);
        assert_eq!((editor.shapes[0].heads, editor.shapes[0].points[0], editor.undo.len()), ([true, true], (200.5, 200.5), 3));
        let mut pixels = vec![0u32; 1600 * 900];
        editor.shapes[0].draw(&mut Canvas::new(&mut pixels, 1600, 900), &mut editor.text);
        // The head at the first end is as wide as the one at the last,
        // which a line's end alone is not.
        let across = |x: usize| (150..260).filter(|&y| pixels[y * 1600 + x] != 0).count();
        assert!(across(208) > 6 && across(350) <= 5, "{} {}", across(208), across(350));
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (300, 500), (500, 600));
        assert_eq!(editor.shapes[1].heads, [true, true]);
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        // Back to one head, at its last end, as arrows have to begin with.
        click(&mut editor, (200, 200), 45_000);
        click(&mut editor, (200, 200), 50_000);
        assert_eq!(editor.shapes[0].heads, [false, true]);

        // Ctrl+D is a copy of it, beside it to the right, and the copy
        // is what is picked out.
        editor.key(Key::Char('d'), ctrl);
        assert_eq!((editor.shapes.len(), editor.selected.clone()), (2, vec![1]));
        let (start, end) = (editor.shapes[1].points[0], editor.shapes[0].points[1]);
        assert!(start.1 == 200.5 && start.0 > end.0, "{start:?}");

        // A box dragged out with Shift held picks out what it touches.
        editor.key(Key::Escape, PLAIN);
        click(&mut editor, (800, 650), 9000);
        assert!(editor.selected.is_empty());
        editor.pointer_moved(150, 150, Mods { shift: true, ctrl: false, alt: false });
        editor.pressed(20_000);
        editor.pointer_moved(600, 400, Mods { shift: true, ctrl: false, alt: false });
        editor.released();
        assert_eq!((editor.selected.clone(), editor.selection()), (vec![0, 1], Some(Rect::new(100, 100, 800, 600))));
    }

    #[test]
    fn a_spotlight_darkens_all_of_the_picture_but_what_it_is_on() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('s'), PLAIN);
        drag(&mut editor, (150, 150), (350, 250));
        // An arrow drawn after it is as bright as ever, outside it too.
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (500, 600), (700, 600));
        let (area, pixels, _) = editor.result().unwrap();
        let at = |x: i32, y: i32| pixels[((y - area.y) * area.w + x - area.x) as usize] & 0xFFFFFF;
        // In the white window: lit inside the spotlight, dark outside.
        assert_eq!((at(250, 200), at(400, 400)), (0xF4F4F4, 0x5B5B5B));
        assert_eq!(at(600, 600), RED);
    }

    #[test]
    fn an_arrow_is_bent_by_the_dot_in_its_middle_and_straightened_by_a_click_on_it() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (200, 400), (600, 400));
        assert_eq!(editor.target_at((400, 400)), Target::End(2));
        drag(&mut editor, (400, 400), (400, 300));
        assert_eq!(editor.shapes[0].bend(), Some((400.5, 300.5)));
        // It goes through where the dot was taken, and no longer along
        // where it was straight; and its ends are where they were.
        let mut pixels = vec![0u32; 1600 * 900];
        editor.shapes[0].draw(&mut Canvas::new(&mut pixels, 1600, 900), &mut editor.text);
        assert!(pixels[300 * 1600 + 400] != 0 && pixels[400 * 1600 + 400] == 0 && pixels[400 * 1600 + 202] != 0);
        // Its head points the way it is going as it comes in: down and
        // to the right, so that above the end there is more of it than
        // below.
        let above = (585..600).filter(|&x| (380..399).any(|y| pixels[y * 1600 + x] != 0)).count();
        let below = (585..600).filter(|&x| (402..420).any(|y| pixels[y * 1600 + x] != 0)).count();
        assert!(above > below, "{above} {below}");
        // Written down and read back, it is as bent; and moved, the
        // bend goes with it.
        let mut read = Shape::read(&editor.shapes[0].write()).unwrap();
        assert_eq!(read.bend(), Some((400.5, 300.5)));
        read.translate(10.0, 0.0);
        assert_eq!(read.middle(), (410.5, 300.5));
        // A click on the dot, and it is straight again; as it is when
        // the dot is taken back to where it would be.
        click(&mut editor, (400, 300), 40_000);
        assert_eq!((editor.shapes[0].bend(), editor.shapes[0].points.len()), (None, 2));
        drag(&mut editor, (400, 400), (400, 300));
        drag(&mut editor, (400, 300), (402, 401));
        assert_eq!(editor.shapes[0].bend(), None);
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        assert!(editor.shapes[0].bend().is_some());
    }

    #[test]
    fn the_mouse_pointer_is_put_in_the_picture_and_taken_out_by_its_button() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        // With no pointer to put in, the button does nothing.
        let at = button(&mut editor, Button::Pointer);
        click(&mut editor, at, 1000);
        assert!(!editor.arrow_shows);
        // A green square for a pointer, half of it past the selection.
        editor.set_pointer(canvas::Pointer { x: 890, y: 300, w: 20, h: 20, pixels: vec![0xFF00_FF00; 400] }, false);
        let green = |editor: &mut Editor| editor.result().unwrap().1.iter().filter(|&&pixel| pixel & 0xFF_FFFF == 0x00FF00).count();
        assert_eq!(green(&mut editor), 0);
        click(&mut editor, at, 2000);
        assert_eq!((editor.arrow_shows, green(&mut editor)), (true, 200));
        // It shows as the picture will have it, and no further; and
        // what is drawn is over it.
        let mut out = vec![0u32; 1600 * 900];
        editor.draw(&mut out);
        assert_eq!((out[310 * 1600 + 895] & 0xFF_FFFF, out[310 * 1600 + 905] & 0xFF_FFFF == 0x00FF00), (0x00FF00, false));
        editor.key(Key::Char('r'), PLAIN);
        editor.key(Key::Char('f'), PLAIN);
        drag(&mut editor, (880, 290), (899, 330));
        assert_eq!(green(&mut editor), 0);
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        editor.key(Key::Char('m'), PLAIN);
        assert_eq!((editor.arrow_shows, green(&mut editor)), (false, 0));
    }

    #[test]
    fn a_badge_is_a_look_in_the_color_in_hand_and_says_what_is_typed() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        // The library: its shelves, and the labels on the first of them.
        editor.key(Key::Char('j'), PLAIN);
        let cells: Vec<Cell> = editor.layout().cells.iter().map(|(_, cell)| *cell).collect();
        let labels: Vec<Cell> = (0..LIBRARY.len()).filter(|&item| LIBRARY[item].1 == 0).map(Cell::Badge).collect();
        assert_eq!(cells, [Cell::Shelf(0), Cell::Shelf(1), Cell::Shelf(2)].into_iter().chain(labels).collect::<Vec<_>>());
        // One chosen is put down in the color in hand, saying nothing
        // meant: what is typed is what it says, and Enter ends it.
        editor.set_color(0x0090FF);
        let at = |editor: &mut Editor, look: usize| editor.layout().cells.iter().find(|(_, cell)| matches!(cell, Cell::Badge(item) if LIBRARY[*item].0 == look)).map(|(rect, _)| rect.center()).expect("it is on the shelf");
        let gloss = at(&mut editor, 6);
        click(&mut editor, gloss, 1000);
        assert_eq!((editor.shapes[0].tool, editor.shapes[0].number, editor.shapes[0].color, editor.shapes[0].text.as_str()), (Tool::Badge, 6, 0x0090FF, BADGE_SAYS));
        assert!(editor.editing.is_some() && editor.flyout.is_none());
        // Shift on its way down is a key as well, and changes nothing.
        editor.key(Key::Other, Mods { shift: true, ctrl: false, alt: false });
        for letter in "Ship it".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        assert_eq!(editor.key(Key::Enter, PLAIN), Outcome::Continue);
        assert_eq!((editor.shapes[0].text.as_str(), editor.editing, editor.selected.clone()), ("Ship it", None, vec![0]));
        // Picked out, the box of colors changes it as it does any
        // shape; and it is some of that color, and not all one color.
        editor.set_color(0x30A46C);
        assert_eq!(editor.shapes[0].color, 0x30A46C);
        let (area, pixels, _) = editor.result().unwrap();
        let place = editor.shapes[0].bounds(&mut editor.text);
        let column: Vec<u32> = (place.y + 2..place.bottom() - 2).map(|y| pixels[((y - area.y) * area.w + place.x + 4 - area.x) as usize] & 0xFF_FFFF).collect();
        assert!(column.iter().all(|&pixel| (pixel >> 8 & 0xFF) > (pixel >> 16 & 0xFF)), "{column:06x?}");
        assert!(column.first() != column.last());
        // One put down and left as it was says what it was put down
        // saying; every look draws, whatever it is given to say.
        editor.key(Key::Char('j'), PLAIN);
        editor.choose(Cell::Badge(11));
        editor.key(Key::Enter, PLAIN);
        assert_eq!((editor.shapes.len(), editor.shapes[1].text.as_str()), (2, BADGE_SAYS));
        let mut out = vec![0u32; 400 * 100];
        for look in 0..shapes::BADGE_STYLES {
            let mut canvas = Canvas::new(&mut out, 400, 100);
            shapes::draw_badge(&mut canvas, &mut editor.text, Rect::new(40, 30, 120, 28), look, 0xE5484D, 0, 16, "Aa");
            assert!(out.iter().any(|&pixel| pixel != 0), "{look}");
            out.fill(0);
        }
        // Some come in a color of their own, saying what they are for;
        // and anything in the library is found by typing.
        editor.key(Key::Char('j'), PLAIN);
        let tick = at(&mut editor, 13);
        click(&mut editor, tick, 2000);
        assert_eq!((editor.shapes[2].number, editor.shapes[2].color, editor.shapes[2].text.as_str()), (13, 0x30A46C, "Do"));
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('j'), PLAIN);
        for letter in "tooltip".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        assert_eq!(editor.library_found().iter().map(|&item| LIBRARY[item].0).collect::<Vec<_>>(), [18]);
        editor.key(Key::Enter, PLAIN);
        assert_eq!((editor.shapes[3].number, editor.shapes[3].text.as_str()), (18, "Opens in a new tab"));
    }

    #[test]
    fn what_is_picked_out_goes_over_and_under_the_other_shapes() {
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        for (at, color) in [(200, 0x111111), (300, 0x222222), (400, 0x333333)] {
            editor.key(Key::Char('v'), PLAIN);
            click(&mut editor, (800, 650), at as u32 * 1000);
            editor.set_color(color);
            editor.key(Key::Char('r'), PLAIN);
            drag(&mut editor, (at, 200), (at + 150, 400));
        }
        let colors = |editor: &Editor| editor.shapes.iter().map(|shape| shape.color).collect::<Vec<_>>();
        // The last drawn is picked out: under the one below it, then
        // under them all, where there is no further to go.
        editor.key(Key::Char('['), ctrl);
        assert_eq!((colors(&editor), editor.selected.clone()), (vec![0x111111, 0x333333, 0x222222], vec![1]));
        editor.key(Key::Char('['), ctrl);
        editor.key(Key::Char('['), ctrl);
        assert_eq!((colors(&editor), editor.selected.clone()), (vec![0x333333, 0x111111, 0x222222], vec![0]));
        // And over them all at once, with Shift; which is undone.
        editor.key(Key::Char('}'), ctrl);
        assert_eq!((colors(&editor), editor.selected.clone()), (vec![0x111111, 0x222222, 0x333333], vec![2]));
        editor.key(Key::Char('z'), ctrl);
        assert_eq!(colors(&editor), vec![0x333333, 0x111111, 0x222222]);

        // An arrow, and a rectangle drawn over it and then put under.
        let mut editor = self::editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (250, 300), (650, 300));
        editor.key(Key::Char('r'), PLAIN);
        editor.key(Key::Char('f'), PLAIN);
        drag(&mut editor, (400, 200), (500, 320));
        assert_eq!((editor.shapes[1].tool, editor.selected.clone()), (Tool::Rect, vec![1]));
        editor.key(Key::Char('['), ctrl);
        assert_eq!((editor.shapes[1].tool, editor.selected.clone()), (Tool::Arrow, vec![0]));
    }

    #[test]
    fn a_zoom_is_made_larger_by_its_rim_and_goes_past_the_selection() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('z'), PLAIN);
        drag(&mut editor, (700, 300), (760, 340));
        // Twice the size as drawn, and picked out, with its two handles.
        assert_eq!(editor.selected, vec![0]);
        assert!((editor.shapes[0].zoom_times() - 2.0).abs() < 0.1);
        let [middle, rim] = editor.shapes[0].zoom_handles();
        // The one on its rim, taken twice as far from its middle, makes
        // it four times the size, its middle where it was.
        drag(&mut editor, rim, (2 * rim.0 - middle.0, 2 * rim.1 - middle.1));
        assert!((editor.shapes[0].zoom_times() - 4.0).abs() < 0.2, "{}", editor.shapes[0].zoom_times());
        let [after, _] = editor.shapes[0].zoom_handles();
        assert!((after.0 - middle.0).abs() <= 1 && (after.1 - middle.1).abs() <= 1);
        // The one in its middle takes it elsewhere: right out of the
        // selection, as large as it was.
        drag(&mut editor, after, (1100, 320));
        let near = |at: (i32, i32), to: (i32, i32)| (at.0 - to.0).abs() <= 1 && (at.1 - to.1).abs() <= 1;
        assert!(near(editor.shapes[0].zoom_handles()[0], (1100, 320)));
        assert!((editor.shapes[0].zoom_times() - 4.0).abs() < 0.2);
        let (_, shown) = editor.shapes[0].zoomed();
        assert!(shown.x > 900);
        // Out there it is still got hold of, by anywhere on it.
        click(&mut editor, (300, 500), 30_000);
        assert!(editor.selected.is_empty());
        drag(&mut editor, (1100, 320), (1050, 320));
        assert!(editor.selected == vec![0] && near(editor.shapes[0].zoom_handles()[0], (1050, 320)));
        assert_eq!(editor.selection(), Some(Rect::new(100, 100, 800, 600)));
        // The picture is as wide as takes the zoom in, with nothing
        // between the two but the lines from the one to the other.
        let (area, wide, high, pixels, clear) = editor.framed().unwrap();
        assert_eq!((area, high, clear), (Rect::new(100, 100, 800, 600), 600, true));
        assert!(wide > 1050 + shown.w / 2 - 100, "{wide}");
        let at = |x: i32, y: i32| pixels[((y - 100) * wide + x - 100) as usize];
        assert_eq!((at(1050, 320) >> 24, at(300, 500) >> 24, at(905, 110) >> 24, at(wide + 98, 690) >> 24), (255, 255, 0, 0));
        // What the zoom shows is what it is of: the top of a window.
        assert_eq!(at(1050, 320) & 0xFFFFFF, 0x3A3F55);
    }

    #[test]
    fn text_is_typed_into_anywhere_in_it_and_pasted_into() {
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('t'), PLAIN);
        click(&mut editor, (300, 300), 0);
        for key in [Key::Char('a'), Key::Char('c'), Key::Left, Key::Char('b'), Key::End, Key::Enter, Key::Char('x'), Key::Char('y'), Key::Char('z')] {
            editor.key(key, PLAIN);
        }
        assert_eq!(editor.shapes[0].text, "abc\nxyz");
        // Up keeps as far along the line; Home and Delete, Backspace.
        for key in [Key::Left, Key::Up, Key::Char('!'), Key::Home, Key::Delete, Key::Down, Key::Right, Key::Backspace] {
            editor.key(key, PLAIN);
        }
        assert_eq!(editor.shapes[0].text, "b!c\nyz");
        // Ctrl+V asks for what is on the clipboard, which goes in at the
        // caret, less anything that is not text.
        assert_eq!(editor.key(Key::Char('v'), ctrl), Outcome::Paste);
        editor.paste("1\r\n2\t");
        assert_eq!(editor.shapes[0].text, "b!c\n1\n2yz");
        // A click in the text puts the caret there: at its very start.
        let (x, y) = (editor.shapes[0].points[0].0 as i32, editor.shapes[0].points[0].1 as i32);
        click(&mut editor, (x + 1, y + 5), 30_000);
        editor.key(Key::Char('>'), PLAIN);
        assert_eq!(editor.shapes[0].text, ">b!c\n1\n2yz");

        // G goes through the faces, for this text and what is typed
        // after; and what is in hand is what is kept for next time.
        editor.key(Key::Enter, ctrl);
        editor.key(Key::Char('g'), PLAIN);
        editor.key(Key::Char('g'), PLAIN);
        assert_eq!((editor.shapes[0].face, FACES[editor.in_hand().face]), (2, "Architects Daughter"));
        let narrow = editor.shapes[0].bounds(&mut editor.text).w;
        editor.key(Key::Char('g'), PLAIN);
        assert_ne!(editor.shapes[0].bounds(&mut editor.text).w, narrow);
    }

    #[test]
    fn an_edge_of_the_selection_goes_to_the_edge_of_a_window_near_it() {
        // The windows are at 100..700 across and 80..480 down, and at
        // 500..1200 and 300..800.
        let mut editor = editor(1600, 900);
        drag(&mut editor, (300, 200), (696, 477));
        assert_eq!(editor.selection(), Some(Rect::new(300, 200, 401, 281)));
        // Too far from any edge, it stays where it is put; and with
        // Ctrl held it does too, however near.
        let mut editor = self::editor(1600, 900);
        drag(&mut editor, (300, 200), (650, 420));
        assert_eq!(editor.selection(), Some(Rect::new(300, 200, 351, 221)));
        let mut editor = self::editor(1600, 900);
        editor.pointer_moved(300, 200, PLAIN);
        editor.pressed(0);
        editor.pointer_moved(696, 477, Mods { shift: false, ctrl: true, alt: false });
        editor.released();
        assert_eq!(editor.selection(), Some(Rect::new(300, 200, 397, 278)));

        // A picture come back to has its shapes again, to go on with.
        let mut first = self::editor(1600, 900);
        drag(&mut first, (200, 150), (900, 600));
        first.key(Key::Char('r'), PLAIN);
        drag(&mut first, (300, 250), (500, 400));
        let (area, pixels, lines) = first.keepsake().unwrap();
        assert_eq!((area, pixels.len(), lines.lines().count()), (first.selection().unwrap(), (area.w * area.h) as usize, 1));
        let mut second = self::editor(1600, 900);
        second.select(area);
        second.restore(lines.lines().filter_map(Shape::read).collect());
        assert_eq!((second.shapes.len(), second.shapes[0].frame()), (1, first.shapes[0].frame()));
        assert_eq!(second.result().unwrap().1, first.result().unwrap().1);
    }

    #[test]
    fn emoji_are_searched_by_name_scrolled_and_put_on_the_picture() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (300, 200), (900, 600));
        editor.key(Key::Char('e'), PLAIN);
        assert_eq!((editor.flyout, editor.shelf, editor.emoji_found().len()), (Some(Button::Badges), EMOJI_SHELF, emoji().len()));
        assert!(emoji().len() > 1300);
        // The wheel goes down them, and no further up than the top.
        editor.scrolled(false);
        assert_eq!(editor.scroll, 2);
        editor.scrolled(true);
        editor.scrolled(true);
        assert_eq!(editor.scroll, 0);
        // What is typed is looked for in their names, not taken for
        // the keys of tools; every word of it has to be there.
        for letter in "red heart".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        let found = editor.emoji_found();
        assert_eq!((editor.tool, found.len(), emoji()[found[0]].1), (Tool::Select, 1, "red heart"));
        editor.key(Key::Backspace, PLAIN);
        assert!(editor.emoji_found().len() >= 1);
        editor.key(Key::Char('t'), PLAIN);
        // Enter puts the first found on the picture, picked out, as
        // text twice the size, and puts the box away.
        editor.key(Key::Enter, PLAIN);
        assert_eq!((editor.flyout, editor.shapes.len(), editor.shapes[0].text.as_str(), editor.selected.clone()), (None, 1, "❤\u{fe0f}", vec![0]));
    }

    #[test]
    fn the_frame_shows_round_the_selection_and_is_dragged_wider() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (400, 300), (799, 599));
        assert!(editor.in_hand().frame.is_plain());
        // The box of backdrops has none, a few, and dots for the rest:
        // which pressed show every one of them, and pressed again go.
        editor.key(Key::Char('w'), PLAIN);
        let backdrops = |editor: &mut Editor| editor.layout().cells.iter().filter(|(_, cell)| matches!(cell, Cell::Backdrop(which) if *which > 0)).count();
        assert_eq!(backdrops(&mut editor), FIRST.len());
        let dots = editor.layout().cells.iter().find(|(_, cell)| *cell == Cell::More).map(|(rect, _)| rect.center()).unwrap();
        click(&mut editor, dots, 1000);
        assert_eq!((backdrops(&mut editor), editor.layout().captions.len(), editor.flyout), (BACKDROPS.len(), 3, Some(Button::Frame)));
        let slate = frame::named("slate").unwrap();
        let cell = editor.layout().cells.iter().find(|(_, cell)| *cell == Cell::Backdrop(slate + 1)).map(|(rect, _)| rect.center()).unwrap();
        click(&mut editor, cell, 2000);
        editor.choose(Cell::More);
        assert_eq!(backdrops(&mut editor), FIRST.len());
        editor.key(Key::Escape, PLAIN);
        // Slate, with room: the backdrop shows outside the selection,
        // where the darkened screen was, and the bar is clear of it.
        let room = editor.frame_room().0;
        assert_eq!((editor.frame.backdrop, editor.frame_room()), (Backdrop::Gradient(slate), (PADS[2], PADS[2])));
        let mut out = vec![0u32; 1600 * 900];
        editor.draw(&mut out);
        let near = |pixel: u32, color: u32| (0..3).all(|channel| ((pixel >> (channel * 8) & 0xFF) as i32 - (color >> (channel * 8) & 0xFF) as i32).abs() < 40);
        assert!(near(out[(290 * 1600 + 390) as usize], 0x334155), "{:06x}", out[(290 * 1600 + 390) as usize]);
        assert!(editor.layout().bar.unwrap().y >= 600 + room);
        assert_eq!(editor.target_at((380, 400)), Target::Backdrop);

        // Dragged outward by forty pixels beside the picture, it is
        // forty pixels wider and no taller; from under it, taller; and
        // a click on it opens what there is to choose about it.
        drag(&mut editor, (380, 400), (340, 400));
        assert_eq!(editor.frame.pad, (PADS[2] + 40, PADS[2]));
        drag(&mut editor, (600, 620), (600, 630));
        assert_eq!(editor.frame.pad, (PADS[2] + 40, PADS[2] + 10));
        assert_eq!(editor.flyout, None);
        click(&mut editor, (350, 400), 9000);
        assert_eq!(editor.flyout, Some(Button::Frame));
        editor.key(Key::Escape, PLAIN);

        // The picture comes out with that much round it, and nothing
        // of it see-through.
        let (_, wide, high, _, alpha) = editor.framed().unwrap();
        assert_eq!((wide, high, alpha), (400 + 2 * (PADS[2] + 40), 300 + 2 * (PADS[2] + 10), false));

        // The frame has handles on its edges, to be seen and taken hold
        // of: the one at its bottom-right corner makes it wider and
        // taller both, and a click on one opens nothing.
        let whole = editor.frame_rect();
        let corner = (whole.right(), whole.bottom());
        assert_eq!(editor.target_at(corner), Target::FrameGrip(1, 1));
        let before = editor.frame.pad;
        drag(&mut editor, corner, (corner.0 + 25, corner.1 + 15));
        assert_eq!(editor.frame.pad, (before.0 + 25, before.1 + 15));
        let side = grip_at(editor.frame_rect(), 1, 0);
        click(&mut editor, side, 20_000);
        assert_eq!(editor.flyout, None);
        drag(&mut editor, side, (side.0 - 25, side.1 + 50));
        assert_eq!(editor.frame.pad, (before.0, before.1 + 15));
        editor.frame.pad = before;

        // The dot inside the frame's corner rounds its corners, by as
        // much as it is taken in; and then the corners of what comes
        // out are see-through.
        let dot = editor.frame_dot().unwrap();
        assert_eq!(editor.target_at(dot), Target::FrameCorner);
        drag(&mut editor, dot, (dot.0 + 30, dot.1 + 30));
        assert_eq!(editor.frame.radius, 30);
        let (_, wide, _, pixels, alpha) = editor.framed().unwrap();
        assert!(alpha && pixels[0] >> 24 == 0 && pixels[(40 * wide + 40) as usize] >> 24 == 255);
    }

    #[test]
    fn text_is_set_heavy_slanted_and_after_dots_and_shapes_are_made_faint() {
        let ctrl = Mods { shift: false, ctrl: true, alt: false };
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('t'), PLAIN);
        click(&mut editor, (300, 300), 0);
        for letter in "one".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        // The bar over the text sets it: heavy is wider, and each line
        // after a dot wider still. The text is still being typed.
        let plain = editor.shapes[0].bounds(&mut editor.text).w;
        let bold = button(&mut editor, Button::Style(shapes::BOLD));
        click(&mut editor, bold, 5000);
        assert_eq!((editor.shapes[0].style, editor.editing.is_some()), (shapes::BOLD, true));
        let heavy = editor.shapes[0].bounds(&mut editor.text).w;
        editor.key(Key::Char('i'), ctrl);
        editor.toggle_style(shapes::BULLETS);
        let dotted = editor.shapes[0].bounds(&mut editor.text).w;
        assert!(heavy > plain && dotted > heavy);
        assert_eq!(editor.shapes[0].style, shapes::BOLD | shapes::ITALIC | shapes::BULLETS);
        editor.key(Key::Enter, ctrl);

        // How opaque: for what is picked out, and what is drawn after.
        editor.key(Key::Char('c'), PLAIN);
        let half = editor.layout().opacities[1].0.center();
        click(&mut editor, half, 9000);
        assert_eq!((editor.shapes[0].opacity, editor.opacity), (128, 128));
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('r'), PLAIN);
        editor.key(Key::Char('f'), PLAIN);
        drag(&mut editor, (500, 400), (700, 500));
        assert_eq!(editor.shapes[1].opacity, 128);
        // A red rectangle half there, over the dark window: neither.
        let (area, pixels, _) = editor.result().unwrap();
        let at = pixels[((450 - area.y) * area.w + 600 - area.x) as usize] & 0xFF_FFFF;
        assert!(at != RED && at != 0x24283B, "{at:06x}");
    }

    #[test]
    fn a_tool_in_hand_has_its_ways_over_the_bar_and_shapes_go_to_lines() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        // With the move tool and nothing picked out, there is no bar over
        // the other; with the arrow in hand, its ways are there, and a
        // head at each end is so from then on.
        assert!(tray(&mut editor).is_empty());
        editor.key(Key::Char('a'), PLAIN);
        assert_eq!(tray(&mut editor), [Button::Color, Button::Width, Button::Path, Button::Start, Button::End, Button::Stroke, Button::Sketch]);
        let (bar, over) = (editor.layout().bar.expect("the bar is up"), editor.layout().tray.expect("and the one over it"));
        assert!(over.bottom() <= bar.y);
        // The start's head, from its list: a dot.
        let start = button(&mut editor, Button::Start);
        click(&mut editor, start, 0);
        assert_eq!(editor.flyout, Some(Button::Start));
        let dot = editor.layout().cells.iter().find(|(_, cell)| *cell == Cell::Head(0, 3)).map(|(rect, _)| rect.center()).expect("the heads are up");
        click(&mut editor, dot, 500);
        assert_eq!((editor.tool, editor.heads, editor.ends[0], editor.flyout), (Tool::Arrow, [true, true], 3, None));
        drag(&mut editor, (200, 200), (400, 200));
        assert_eq!((editor.shapes[0].heads, editor.shapes[0].ends[0]), ([true, true], 3));
        // Picked out, it is made dotted, round a corner and by hand.
        let stroke = button(&mut editor, Button::Stroke);
        click(&mut editor, stroke, 800);
        let dotted = editor.layout().cells.iter().find(|(_, cell)| *cell == Cell::Line(2)).map(|(rect, _)| rect.center()).expect("the kinds are up");
        click(&mut editor, dotted, 900);
        for (index, step) in [Button::Path, Button::Sketch].into_iter().enumerate() {
            let at = button(&mut editor, step);
            click(&mut editor, at, 1000 + index as u32 * 500);
        }
        let shape = &editor.shapes[0];
        assert_eq!((shape.dotted, shape.dashed, shape.elbow, shape.sketch), (true, false, true, true));
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true, alt: false });
        assert!(!editor.shapes[0].sketch);

        // A second shape, moved until its left edge is within a few
        // pixels of the first one's, goes to it; and a line shows where.
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (300, 400), (500, 500));
        let first = editor.shapes[0].bounds(&mut editor.text);
        let second = editor.shapes[1].bounds(&mut editor.text);
        let by = first.x - second.x + 4;
        editor.pointer_moved(400, 400, PLAIN);
        editor.pressed(50_000);
        editor.pointer_moved(400 + by, 420, PLAIN);
        assert_eq!(editor.shapes[1].bounds(&mut editor.text).x, first.x);
        assert_eq!(editor.guides, [(true, first.x)]);
        // With Ctrl held it stays where it is put.
        editor.pointer_moved(400 + by, 421, Mods { shift: false, ctrl: true, alt: false });
        assert_eq!(editor.shapes[1].bounds(&mut editor.text).x, first.x + 4);
        editor.released();
        assert!(editor.guides.is_empty());
    }

    /// A shape taken hold of at a point and moved so far, with these keys.
    /// Pressed with none, as Ctrl or Shift at the press pick out by a box.
    fn move_shape(editor: &mut Editor, at: (i32, i32), by: (i32, i32), mods: Mods, time: u32) {
        editor.pointer_moved(at.0, at.1, PLAIN);
        editor.pressed(time);
        editor.pointer_moved(at.0 + by.0 / 2, at.1 + by.1 / 2, mods);
        editor.pointer_moved(at.0 + by.0, at.1 + by.1, mods);
    }

    #[test]
    fn a_shape_copied_goes_beside_itself_and_a_copy_moved_is_copied_as_far_again() {
        let mut editor = editor(1600, 900);
        let ctrl = Mods { ctrl: true, ..PLAIN };
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (200, 200), (260, 240));
        let first = editor.shapes[0].bounds(&mut editor.text);
        // Beside it to the right, a gap away, and picked out.
        editor.key(Key::Char('d'), ctrl);
        let gap = (ui_px(editor.monitor().h) * 3 / 2) as i32;
        let copy = editor.shapes[1].bounds(&mut editor.text);
        assert_eq!((copy.x, copy.y, editor.selected.clone()), (first.right() + gap, first.y, vec![1]));
        // Moved down 100 by its top edge, and copied: as far again, twice.
        let across = copy.x - first.x;
        move_shape(&mut editor, (230 + across, 200), (0, 100), ctrl, 10_000);
        editor.released();
        assert_eq!(editor.shapes[1].bounds(&mut editor.text).y, first.y + 100);
        editor.key(Key::Char('d'), ctrl);
        editor.key(Key::Char('d'), ctrl);
        let (third, fourth) = (editor.shapes[2].bounds(&mut editor.text), editor.shapes[3].bounds(&mut editor.text));
        assert_eq!((third.x, third.y, fourth.x, fourth.y), (first.x + 2 * across, first.y + 200, first.x + 3 * across, first.y + 300));
        // One step to undo each.
        editor.key(Key::Char('z'), ctrl);
        assert_eq!(editor.shapes.len(), 3);
        // Anything else done in between starts over: beside it again.
        editor.selected = vec![2];
        editor.key(Key::Char('d'), ctrl);
        editor.key(Key::Char(']'), ctrl);
        editor.key(Key::Char('d'), ctrl);
        let last = editor.shapes.len() - 1;
        let (was, now) = (editor.shapes[last - 1].bounds(&mut editor.text), editor.shapes[last].bounds(&mut editor.text));
        assert_eq!((now.x, now.y), (was.right() + gap, was.y));
    }

    #[test]
    fn a_shape_with_no_room_to_its_right_is_copied_below_it() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (499, 499));
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (400, 200), (480, 240));
        let first = editor.shapes[0].bounds(&mut editor.text);
        editor.key(Key::Char('d'), Mods { ctrl: true, ..PLAIN });
        let copy = editor.shapes[1].bounds(&mut editor.text);
        assert_eq!((copy.x, copy.y), (first.x, first.bottom() + (ui_px(editor.monitor().h) * 3 / 2) as i32));
    }

    #[test]
    fn a_shape_moved_between_two_stops_with_the_same_gap_either_side() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        for (index, (x, y)) in [(200, 500), (400, 500), (250, 650)].into_iter().enumerate() {
            editor.key(Key::Char('r'), PLAIN);
            editor.pointer_moved(x, y, PLAIN);
            editor.pressed(index as u32 * 5000);
            editor.pointer_moved(x + 60, y + 40, PLAIN);
            editor.released();
        }
        // The third, below the others, taken up level with them and to
        // within a few pixels of halfway between: halfway, the two gaps
        // marked.
        let [a, b, c] = [0, 1, 2].map(|index| editor.shapes[index].bounds(&mut editor.text));
        let halfway = a.right() + (b.x - a.right() - c.w) / 2;
        move_shape(&mut editor, (280, 650), (halfway - c.x + 3, -150), PLAIN, 50_000);
        let moved = editor.shapes[2].bounds(&mut editor.text);
        assert!((moved.x - a.right() - (b.x - moved.right())).abs() <= 1, "{a:?} {moved:?} {b:?}");
        assert_eq!(editor.gaps.len(), 2);
        // Ctrl: where it is put, no gaps.
        editor.pointer_moved(280 + halfway - c.x + 3, 500, Mods { ctrl: true, ..PLAIN });
        assert!(editor.gaps.is_empty());
        editor.released();
        assert!(editor.gaps.is_empty() && editor.guides.is_empty());
    }

    #[test]
    fn the_buttons_of_the_bar_do_what_their_keys_do() {
        let mut editor = editor(1600, 900);
        assert_eq!(editor.key(Key::Enter, PLAIN), Outcome::Continue);
        assert_eq!(editor.selection(), Some(Rect::new(0, 0, 1600, 900)));
        drag(&mut editor, (400, 300), (400, 300));
        let pen = button(&mut editor, Button::Tool(Tool::Pen));
        click(&mut editor, pen, 0);
        assert_eq!(editor.tool, Tool::Pen);
        // The width, from the list of them over the bar.
        let width = button(&mut editor, Button::Width);
        click(&mut editor, width, 1000);
        assert_eq!(editor.flyout, Some(Button::Width));
        let heavy = editor.layout().cells.iter().find(|(_, cell)| *cell == Cell::Width(3)).map(|(rect, _)| rect.center()).expect("the widths are up");
        click(&mut editor, heavy, 1500);
        assert_eq!((editor.size, editor.flyout), (3, None));
        // Pressed on one button and let go off it, nothing is done.
        let (save, copy) = (button(&mut editor, Button::Save), button(&mut editor, Button::Copy));
        editor.pointer_moved(save.0, save.1, PLAIN);
        editor.pressed(2000);
        editor.pointer_moved(save.0, save.1 - 300, PLAIN);
        assert_eq!(editor.released(), Outcome::Continue);
        assert_eq!(click(&mut editor, save, 3000), Outcome::Finish(Finish { save: true, ..Finish::NONE }));
        assert_eq!(click(&mut editor, copy, 4000), Outcome::Finish(Finish { copy: true, ..Finish::NONE }));
        let design = button(&mut editor, Button::Design);
        assert_eq!(click(&mut editor, design, 4500), Outcome::Finish(Finish { canvas: true, ..Finish::NONE }));
        let cancel = button(&mut editor, Button::Cancel);
        assert_eq!(click(&mut editor, cancel, 5000), Outcome::Cancel);
        assert_eq!(editor.key(Key::Escape, PLAIN), Outcome::Cancel);
    }

    /// Not a test of anything: paints the editor in a few of its states
    /// and writes each as a PNG into `$HOTSHOT_SNAPSHOTS`, to be looked at.
    ///
    ///     HOTSHOT_SNAPSHOTS=/tmp/shots cargo test snapshots -- --ignored
    #[test]
    #[ignore]
    fn snapshots() {
        let Some(dir) = std::env::var_os("HOTSHOT_SNAPSHOTS").map(std::path::PathBuf::from) else { return };
        let height: i32 = std::env::var("HOTSHOT_HEIGHT").ok().and_then(|height| height.parse().ok()).unwrap_or(1080);
        let width = height * 16 / 9;
        let unit = height / 108;
        let mut out = vec![0u32; (width * height) as usize];
        let mut write = |editor: &mut Editor, name: &str| {
            editor.draw(&mut out);
            let png = crate::output::encode(width as u32, height as u32, &out, false).unwrap();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{name}.png")), png).unwrap();
        };
        let mut editor = editor(width, height);
        editor.pointer_moved(60 * unit, 40 * unit, PLAIN);
        write(&mut editor, "1-start");
        editor.pointer_moved(20 * unit, 20 * unit, PLAIN);
        editor.pressed(0);
        editor.pointer_moved(70 * unit, 50 * unit, PLAIN);
        write(&mut editor, "2-dragging");
        editor.pointer_moved(150 * unit, 80 * unit, PLAIN);
        editor.released();
        write(&mut editor, "3-selected");

        let mut time = 10_000;
        let mut stroke = |editor: &mut Editor, key: char, from: (i32, i32), to: (i32, i32)| {
            editor.key(Key::Char(key), PLAIN);
            time += 1000;
            editor.pointer_moved(from.0 * unit, from.1 * unit, PLAIN);
            editor.pressed(time);
            editor.pointer_moved((from.0 + to.0) / 2 * unit, (from.1 + to.1) / 2 * unit + if key == 'p' || key == 'h' { 2 * unit } else { 0 }, PLAIN);
            editor.pointer_moved(to.0 * unit, to.1 * unit, PLAIN);
            editor.released();
        };
        stroke(&mut editor, 'r', (30, 28), (62, 44));
        stroke(&mut editor, 'a', (95, 70), (68, 48));
        editor.key(Key::Char('4'), PLAIN);
        editor.set_color(PALETTE[4].0);
        stroke(&mut editor, 'a', (120, 30), (140, 60));
        editor.key(Key::Char('2'), PLAIN);
        stroke(&mut editor, 'o', (100, 28), (130, 42));
        editor.set_color(PALETTE[2].0);
        stroke(&mut editor, 'h', (30, 60), (70, 60));
        editor.set_color(PALETTE[3].0);
        stroke(&mut editor, 'p', (30, 70), (60, 74));
        stroke(&mut editor, 'l', (75, 26), (90, 40));
        stroke(&mut editor, 'b', (105, 50), (135, 70));
        editor.set_color(PALETTE[5].0);
        stroke(&mut editor, 'n', (40, 50), (40, 50));
        stroke(&mut editor, 'n', (80, 62), (92, 54));
        editor.set_color(PALETTE[0].0);
        editor.key(Key::Char('f'), PLAIN);
        stroke(&mut editor, 'r', (136, 24), (146, 30));
        editor.key(Key::Char('f'), PLAIN);
        editor.key(Key::Char('3'), PLAIN);
        editor.key(Key::Char('t'), PLAIN);
        editor.pointer_moved(72 * unit, 74 * unit, PLAIN);
        editor.pressed(90_000);
        editor.released();
        for letter in "Look here".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        write(&mut editor, "4-drawn");
        editor.key(Key::Enter, Mods { shift: false, ctrl: true, alt: false });
        let arrow = button(&mut editor, Button::Tool(Tool::Arrow));
        editor.pointer_moved(arrow.0, arrow.1, PLAIN);
        write(&mut editor, "5-tooltip");
        editor.key(Key::Char('c'), PLAIN);
        let swatch = editor.layout().swatches[6].0.center();
        editor.pointer_moved(swatch.0, swatch.1, PLAIN);
        write(&mut editor, "6-colors");
        for digit in "#3E6".chars() {
            editor.key(Key::Char(digit), PLAIN);
        }
        write(&mut editor, "6b-hex");
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('i'), PLAIN);
        editor.pointer_moved(110 * unit, 36 * unit, PLAIN);
        write(&mut editor, "7-pipette");
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('v'), PLAIN);
        editor.pointer_moved(45 * unit, 28 * unit, PLAIN);
        editor.pressed(200_000);
        editor.released();
        write(&mut editor, "8-shape-picked");
        // Its fill, from the bar over it: which of border and fill, and
        // none among the colors.
        let fill = button(&mut editor, Button::Fill);
        editor.pointer_moved(fill.0, fill.1, PLAIN);
        editor.pressed(210_000);
        editor.released();
        write(&mut editor, "8b-fill");
        editor.key(Key::Escape, PLAIN);
        // A tool in hand and nothing picked out: how it draws, over the bar.
        editor.key(Key::Char('r'), PLAIN);
        write(&mut editor, "8c-tool");
        editor.key(Key::Char('v'), PLAIN);
        editor.key(Key::Char('k'), PLAIN);
        editor.key(Key::Char('k'), PLAIN);
        write(&mut editor, "9-rounded");
        let (area, pixels, rounded) = editor.result().unwrap();
        std::fs::write(dir.join("10-result.png"), crate::output::encode(area.w as u32, area.h as u32, &pixels, rounded).unwrap()).unwrap();

        // A spotlight, an arrow picked out, and a line in each face.
        let mut editor = self::editor(width, height);
        editor.pointer_moved(8 * unit, 6 * unit, PLAIN);
        editor.pressed(0);
        editor.pointer_moved(130 * unit, 90 * unit, PLAIN);
        editor.released();
        editor.key(Key::Char('s'), PLAIN);
        editor.pointer_moved(14 * unit, 12 * unit, PLAIN);
        editor.pressed(1000);
        editor.pointer_moved(66 * unit, 44 * unit, PLAIN);
        editor.released();
        for face in 0..FACES.len() as i32 {
            editor.key(Key::Char('t'), PLAIN);
            editor.pointer_moved(76 * unit, (56 + face * 7) * unit, PLAIN);
            editor.pressed(5000 + face as u32 * 1000);
            editor.released();
            for letter in format!("{} 0123 fi -> ok", FACES[face as usize]).chars() {
                editor.key(Key::Char(letter), PLAIN);
            }
            editor.key(Key::Enter, Mods { shift: false, ctrl: true, alt: false });
            editor.key(Key::Char('g'), PLAIN);
        }
        editor.key(Key::Char('a'), PLAIN);
        editor.pointer_moved(100 * unit, 40 * unit, PLAIN);
        editor.pressed(20_000);
        editor.pointer_moved(70 * unit, 30 * unit, PLAIN);
        editor.released();
        let forward = button(&mut editor, Button::Forward);
        editor.pointer_moved(forward.0, forward.1, PLAIN);
        write(&mut editor, "11-spotlight-faces");

        // Badges, a sticker, a zoom, a dashed arrow, and the box they
        // are chosen from.
        let mut editor = self::editor(width, height);
        editor.pointer_moved(8 * unit, 6 * unit, PLAIN);
        editor.pressed(0);
        editor.pointer_moved(130 * unit, 80 * unit, PLAIN);
        editor.released();
        // Every look of a badge, each in a color of its own and saying
        // what it is called; and two emoji.
        // Every label and note in the library, the labels of the old kind
        // each in a color of its own and saying what it is called, the
        // rest as they come; and two emoji.
        let looks = (0..LIBRARY.len()).map(|item| (Cell::Badge(item), (16 + (item as i32 % 3) * 36, 12 + (item as i32 / 3) * 8)));
        for (cell, to) in looks.chain([(Cell::Emoji(6), (140, 12)), (Cell::Emoji(40), (150, 12))]) {
            if let Cell::Badge(item) = cell {
                editor.color = PALETTE[(item * 5 + 1) % PALETTE.len()].0;
            }
            editor.choose(cell);
            if let Cell::Badge(item) = cell {
                if LIBRARY[item].2 == BADGE_SAYS {
                    for letter in shapes::BADGE_NAMES[LIBRARY[item].0].chars() {
                        editor.key(Key::Char(letter), PLAIN);
                    }
                }
                editor.key(Key::Enter, PLAIN);
            }
            let index = editor.shapes.len() - 1;
            let at = editor.shapes[index].points[0];
            editor.shapes[index].translate((to.0 * unit) as f32 - at.0, (to.1 * unit) as f32 - at.1);
        }
        editor.key(Key::Char('z'), PLAIN);
        editor.pointer_moved(14 * unit, 40 * unit, PLAIN);
        editor.pressed(1000);
        editor.pointer_moved(34 * unit, 52 * unit, PLAIN);
        editor.released();
        editor.key(Key::Char('d'), PLAIN);
        editor.key(Key::Char('a'), PLAIN);
        editor.pointer_moved(110 * unit, 70 * unit, PLAIN);
        editor.pressed(3000);
        editor.pointer_moved(90 * unit, 40 * unit, PLAIN);
        editor.released();
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('j'), PLAIN);
        write(&mut editor, "12-badges");
        editor.key(Key::Char('e'), PLAIN);
        write(&mut editor, "12b-emoji");
        for letter in "heart".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        write(&mut editor, "12c-emoji-search");
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('j'), PLAIN);
        // Some of them turned, the one picked out with its box turned too.
        editor.key(Key::Char('v'), PLAIN);
        for (index, degrees) in [(1, -12.0f32), (6, 20.0), (9, 90.0)] {
            let about = editor.shapes[index].centre(&mut editor.text);
            editor.shapes[index].turn(degrees.to_radians(), about, &mut editor.text);
        }
        editor.selected = vec![6];
        let turner = editor.turners(6)[1];
        editor.pointer_moved(turner.0, turner.1, PLAIN);
        write(&mut editor, "12d-turned");
        for index in [1, 6, 9] {
            let about = editor.shapes[index].centre(&mut editor.text);
            let angle = editor.shapes[index].angle;
            editor.shapes[index].turn(-angle, about, &mut editor.text);
        }
        editor.selected.clear();

        // The picture set on a backdrop, with room and a shadow, as the
        // box it is chosen in shows it and as it comes out.
        editor.key(Key::Escape, PLAIN);
        editor.corners = 16.0;
        for cell in [Cell::Backdrop(1), Cell::Shadow] {
            editor.choose(cell);
        }
        editor.key(Key::Char('w'), PLAIN);
        write(&mut editor, "13-frame");
        editor.choose(Cell::More);
        write(&mut editor, "13b-backdrops");
        // Every backdrop, each as wide as a card and side by side.
        let (across, wide, high) = (8, 240, 150);
        let mut sheet = vec![0u32; (across * wide * 5 * high) as usize];
        for which in 0..BACKDROPS.len() as i32 {
            for at in 0..wide * high {
                let (x, y) = (at % wide, at / wide);
                let color = frame::shade(which as usize, (x as f32 + 0.5) / wide as f32, (y as f32 + 0.5) / high as f32, ((x * 7 + y * 13) % 16) as f32 / 16.0);
                sheet[((which / across * high + y) * across * wide + which % across * wide + x) as usize] = color;
            }
        }
        std::fs::write(dir.join("13c-every-backdrop.png"), crate::output::encode((across * wide) as u32, (5 * high) as u32, &sheet, false).unwrap()).unwrap();
        editor.choose(Cell::More);
        // The frame wider than it is tall, with its own corners round.
        editor.key(Key::Escape, PLAIN);
        (editor.frame.pad, editor.frame.radius) = ((96, 40), 28);
        write(&mut editor, "13b-frame-shaped");
        // Text set heavy, slanted, gone over and after dots, with the
        // bar that sets it; a faint filled rectangle; the arrow's list.
        let mut styled = self::editor(width, height);
        styled.pointer_moved(8 * unit, 6 * unit, PLAIN);
        styled.pressed(0);
        styled.pointer_moved(130 * unit, 80 * unit, PLAIN);
        styled.released();
        for (row, style) in [(0, shapes::BOLD), (1, shapes::ITALIC), (2, shapes::HIGHLIGHT), (3, shapes::BULLETS | shapes::BOLD)] {
            styled.style = style;
            styled.key(Key::Char('3'), PLAIN);
            styled.key(Key::Char('t'), PLAIN);
            styled.pointer_moved(20 * unit, (16 + row * 9) * unit, PLAIN);
            styled.pressed(1000 * row as u32 + 1000);
            styled.released();
            for letter in "Set this way".chars() {
                styled.key(Key::Char(letter), PLAIN);
            }
            if row == 3 {
                styled.key(Key::Enter, PLAIN);
                for letter in "and this".chars() {
                    styled.key(Key::Char(letter), PLAIN);
                }
            } else {
                styled.key(Key::Enter, Mods { shift: false, ctrl: true, alt: false });
            }
        }
        write(&mut styled, "16-text-set");
        styled.key(Key::Enter, Mods { shift: false, ctrl: true, alt: false });
        styled.opacity = 128;
        styled.key(Key::Char('r'), PLAIN);
        styled.key(Key::Char('f'), PLAIN);
        styled.pointer_moved(70 * unit, 30 * unit, PLAIN);
        styled.pressed(20_000);
        styled.pointer_moved(110 * unit, 60 * unit, PLAIN);
        styled.released();
        styled.key(Key::Char('a'), PLAIN);
        let arrow = button(&mut styled, Button::Tool(Tool::Arrow));
        styled.pointer_moved(arrow.0, arrow.1, PLAIN);
        styled.pressed(30_000);
        styled.released();
        write(&mut styled, "17-ways");
        let (_, wide, high, pixels, alpha) = editor.framed().unwrap();
        std::fs::write(dir.join("14-framed.png"), crate::output::encode(wide as u32, high as u32, &pixels, alpha).unwrap()).unwrap();
        editor.choose(Cell::Backdrop(0));
        let (_, wide, high, pixels, alpha) = editor.framed().unwrap();
        std::fs::write(dir.join("15-shadow-only.png"), crate::output::encode(wide as u32, high as u32, &pixels, alpha).unwrap()).unwrap();

        // A zoom made larger by its rim, and one taken out past the
        // selection; and the picture that comes of them.
        let mut lens = self::editor(width, height);
        lens.pointer_moved(30 * unit, 20 * unit, PLAIN);
        lens.pressed(0);
        lens.pointer_moved(110 * unit, 80 * unit, PLAIN);
        lens.released();
        for (square, from) in [(false, (50, 30)), (true, (60, 60))] {
            lens.square = square;
            lens.key(Key::Char('z'), PLAIN);
            lens.pointer_moved(from.0 * unit, from.1 * unit, PLAIN);
            lens.pressed(40_000);
            lens.pointer_moved((from.0 + 12) * unit, (from.1 + 8) * unit, PLAIN);
            lens.released();
            let [middle, _] = lens.shapes.last().unwrap().zoom_handles();
            lens.pointer_moved(middle.0, middle.1, PLAIN);
            lens.pressed(50_000);
            lens.pointer_moved(125 * unit, (from.1 + 5) * unit, PLAIN);
            lens.released();
        }
        let [_, rim] = lens.shapes.last().unwrap().zoom_handles();
        lens.pointer_moved(rim.0, rim.1, PLAIN);
        lens.pressed(60_000);
        lens.pointer_moved(rim.0 + 6 * unit, rim.1 + 4 * unit, PLAIN);
        write(&mut lens, "18-zoom-past");
        lens.released();
        let (_, wide, high, pixels, alpha) = lens.framed().unwrap();
        std::fs::write(dir.join("19-zoom-picture.png"), crate::output::encode(wide as u32, high as u32, &pixels, alpha).unwrap()).unwrap();

        // Each face plain, heavy, slanted and both, and on a patch; and
        // arrows bent, one of them in dashes with a head at each end.
        let mut cuts = self::editor(width, height);
        cuts.pointer_moved(4 * unit, 4 * unit, PLAIN);
        cuts.pressed(0);
        cuts.pointer_moved(170 * unit, 90 * unit, PLAIN);
        cuts.released();
        for face in 0..FACES.len() {
            for (column, style) in [0, shapes::BOLD, shapes::ITALIC, shapes::BOLD | shapes::ITALIC, shapes::BOXED | shapes::BOLD].into_iter().enumerate() {
                let mut shape = Shape::new(Tool::Text, if style & shapes::BOXED != 0 { PALETTE[face * 4].0 } else { 0xFFFFFF }, 3.0, ((8 + column as i32 * 30) as f32 * unit as f32, (8 + face as i32 * 9) as f32 * unit as f32));
                (shape.face, shape.style, shape.font_px, shape.text) = (face, style, 3 * unit as u32, "Hamburg 123".into());
                cuts.shapes.push(shape);
            }
        }
        for (row, (heads, dashed)) in [([false, true], false), ([true, true], true), ([false, false], false)].into_iter().enumerate() {
            let y = (60 + row as i32 * 9) as f32 * unit as f32;
            let mut shape = Shape::new(Tool::Arrow, RED, 4.0, (10.0 * unit as f32, y));
            shape.points[1] = (70.0 * unit as f32, y + 4.0 * unit as f32);
            shape.bend_to((45.0 * unit as f32, y - 8.0 * unit as f32));
            (shape.heads, shape.dashed) = (heads, dashed);
            cuts.shapes.push(shape);
        }
        cuts.selected = vec![cuts.shapes.len() - 3];
        write(&mut cuts, "20-cuts-and-bends");

        // Every head, at the start and the end; elbows; dotted, dashed and
        // white-edged lines; boxes and ovals drawn by hand; and the list
        // of heads open over an arrow picked out.
        let mut kinds = self::editor(width, height);
        kinds.select(Rect::new(4 * unit, 4 * unit, 170 * unit, 96 * unit));
        for (index, style) in (0..shapes::HEADS.len() as u8).enumerate() {
            let y = (14 + index as i32 * 10) * unit;
            let mut arrow = Shape::new(Tool::Arrow, PALETTE[(index * 3) % PALETTE.len()].0, STROKES[1] * unit as f32 / 10.0, ((12 * unit) as f32, y as f32));
            arrow.points[1] = ((52 * unit) as f32, y as f32);
            (arrow.heads, arrow.ends) = ([true, true], [style, style]);
            kinds.shapes.push(arrow);
        }
        let lines = [(false, false, false, false), (true, false, false, false), (false, true, false, false), (false, false, true, false), (false, false, false, true)];
        for (index, (dashed, dotted, halo, elbow)) in lines.into_iter().enumerate() {
            let y = (14 + index as i32 * 14) * unit;
            let mut line = Shape::new(Tool::Arrow, RED, STROKES[2] * unit as f32 / 10.0, ((64 * unit) as f32, y as f32));
            line.points[1] = ((104 * unit) as f32, (y + if elbow { 10 * unit } else { 0 }) as f32);
            (line.dashed, line.dotted, line.halo, line.elbow) = (dashed, dotted, halo, elbow);
            kinds.shapes.push(line);
        }
        for (index, tool) in [Tool::Rect, Tool::Ellipse, Tool::Arrow].into_iter().enumerate() {
            let top = (14 + index as i32 * 26) * unit;
            let mut drawn = Shape::new(tool, PALETTE[9].0, STROKES[1] * unit as f32 / 10.0, ((116 * unit) as f32, top as f32));
            drawn.points[1] = ((160 * unit) as f32, (top + 18 * unit) as f32);
            (drawn.sketch, drawn.filled, drawn.fill) = (true, tool == Tool::Ellipse, PALETTE[17].0);
            kinds.shapes.push(drawn);
        }
        kinds.selected = vec![kinds.shapes.len() - 1];
        let start = button(&mut kinds, Button::Start);
        kinds.pointer_moved(start.0, start.1, PLAIN);
        kinds.pressed(1000);
        kinds.released();
        write(&mut kinds, "21-arrow-kinds");
    }

    /// Not a test either: makes the pictures the README shows, out of
    /// `docs/desk.png`, which is a capture of two terminals on a made-up
    /// desktop.
    ///
    ///     cargo test pictures -- --ignored
    #[test]
    #[ignore]
    fn pictures() {
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs");
        let desk = frame::open(&std::fs::read(docs.join("desk.png")).unwrap()).unwrap();
        let (width, height) = (desk.width, desk.height);
        let (code, shell) = (Rect::new(120, 90, 786, 658), Rect::new(820, 380, 624, 460));
        let fresh = || {
            let pixels: Vec<u32> = desk.pixels.iter().map(|pixel| pixel & 0xFF_FFFF).collect();
            let mut dim = pixels.clone();
            canvas::darken_into(&mut dim, &pixels, 128);
            let shot = Shot { width, height, pixels: Box::new(pixels) };
            Editor::new(shot, dim, vec![Rect::new(0, 0, width, height)], vec![shell, code], Settings::default(), Text::load())
        };
        let stroke = |editor: &mut Editor, key: char, from: (i32, i32), to: (i32, i32)| {
            editor.key(Key::Char(key), PLAIN);
            drag(editor, from, to);
        };
        let write = |name: &str, wide: i32, high: i32, pixels: &[u32], alpha: bool| {
            std::fs::write(docs.join(name), crate::output::encode(wide as u32, high as u32, pixels, alpha).unwrap()).unwrap();
        };

        // The editor: an area selected, drawn on, and the bar under it.
        let mut editor = fresh();
        drag(&mut editor, (90, 60), (1470, 848));
        stroke(&mut editor, 'r', (196, 248), (640, 292));
        stroke(&mut editor, 'a', (800, 150), (650, 256));
        editor.key(Key::Char('t'), PLAIN);
        click(&mut editor, (812, 138), 10_000);
        for letter in "nearest the pointer first".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        editor.key(Key::Enter, Mods { shift: false, ctrl: true, alt: false });
        editor.set_color(0xFFFFFF);
        editor.key(Key::Char('v'), PLAIN);
        click(&mut editor, (1000, 700), 30_000);
        editor.set_color(PALETTE[12].0);
        stroke(&mut editor, 'n', (150, 448), (150, 448));
        stroke(&mut editor, 'n', (150, 548), (150, 548));
        editor.set_color(0xFFC53D);
        stroke(&mut editor, 'h', (1160, 450), (1218, 450));
        stroke(&mut editor, 'b', (850, 520), (1170, 582));
        editor.key(Key::Char('v'), PLAIN);
        click(&mut editor, (1000, 700), 60_000);
        // The box round the code picked out, with the bar over it.
        click(&mut editor, (196, 270), 70_000);
        editor.pointer_moved(1300, 250, PLAIN);
        let mut out = vec![0u32; (width * height) as usize];
        editor.draw(&mut out);
        write("editor.png", width, height, &out, false);

        // A window in a frame: a backdrop, room round it, a shadow.
        let mut editor = fresh();
        click(&mut editor, (1300, 700), 0);
        editor.set_color(0xFFC53D);
        stroke(&mut editor, 'h', (1160, 450), (1218, 450));
        editor.set_color(PALETTE[1].0);
        stroke(&mut editor, 'a', (1330, 520), (1235, 460));
        editor.corners = 12.0;
        for cell in [Cell::Backdrop(frame::named("sorbet").unwrap() + 1), Cell::Shadow, Cell::Window(1)] {
            editor.choose(cell);
        }
        let (_, wide, high, pixels, alpha) = editor.framed().unwrap();
        write("frame.png", wide, high, &pixels, alpha);

        // A zoom that goes past the picture, a spotlight, and a badge.
        let mut editor = fresh();
        click(&mut editor, (300, 400), 0);
        stroke(&mut editor, 's', (150, 225), (880, 350));
        stroke(&mut editor, 'z', (330, 236), (420, 302));
        let [middle, rim] = editor.shapes[1].zoom_handles();
        drag(&mut editor, middle, (1030, 300));
        let (middle, rim) = (editor.shapes[1].zoom_handles()[0], (rim.0 - middle.0, rim.1 - middle.1));
        drag(&mut editor, (middle.0 + rim.0, middle.1 + rim.1), (middle.0 + rim.0 * 3 / 2, middle.1 + rim.1 * 3 / 2));
        let (_, wide, high, pixels, alpha) = editor.framed().unwrap();
        write("zoom.png", wide, high, &pixels, alpha);
    }

    /// Not a test: pictures of hotshot at work, to show it with, out of
    /// a capture of a made-up desktop that `$HOTSHOT_DESK` names, into
    /// `$HOTSHOT_SNAPSHOTS`. The desktop is 1920 by 1080, with a code
    /// window at 170,120 to 983,842 and a terminal at 1080,520 to
    /// 1753,978.
    ///
    ///     HOTSHOT_DESK=desk.png HOTSHOT_SNAPSHOTS=/tmp/shots cargo test showing -- --ignored
    #[test]
    #[ignore]
    fn showing() {
        let (Some(desk), Some(dir)) = (std::env::var_os("HOTSHOT_DESK"), std::env::var_os("HOTSHOT_SNAPSHOTS")) else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let desk = frame::open(&std::fs::read(desk).unwrap()).unwrap();
        let (width, height) = (desk.width, desk.height);
        let (code, shell) = (Rect::new(170, 120, 814, 723), Rect::new(1080, 520, 674, 459));
        let fresh = || {
            let pixels: Vec<u32> = desk.pixels.iter().map(|pixel| pixel & 0xFF_FFFF).collect();
            let mut dim = pixels.clone();
            canvas::darken_into(&mut dim, &pixels, 128);
            let shot = Shot { width, height, pixels: Box::new(pixels) };
            Editor::new(shot, dim, vec![Rect::new(0, 0, width, height)], vec![shell, code], Settings::default(), Text::load())
        };
        let stroke = |editor: &mut Editor, key: char, from: (i32, i32), to: (i32, i32)| {
            editor.key(Key::Char(key), PLAIN);
            drag(editor, from, to);
        };
        let type_in = |editor: &mut Editor, at: (i32, i32), words: &str| {
            editor.key(Key::Char('t'), PLAIN);
            click(editor, at, 40_000);
            for letter in words.chars() {
                editor.key(Key::Char(letter), PLAIN);
            }
            editor.key(Key::Enter, Mods { ctrl: true, ..PLAIN });
        };
        let screen = |editor: &mut Editor, name: &str| {
            let mut out = vec![0u32; (width * height) as usize];
            editor.draw(&mut out);
            std::fs::write(dir.join(name), crate::output::encode(width as u32, height as u32, &out, false).unwrap()).unwrap();
        };
        let annotated = |editor: &mut Editor| {
            // Round the failing test, an arrow to it, and a word on a
            // patch; the count gone over with the marker.
            // The color in hand is set as it is, not by the box of
            // colors, which would change what was just drawn as well.
            editor.color = PALETTE[1].0;
            stroke(editor, 'r', (1124, 670), (1430, 698));
            editor.color = 0xFFC53D;
            stroke(editor, 'h', (1116, 726), (1324, 726));
            editor.color = PALETTE[1].0;
            stroke(editor, 'a', (1610, 820), (1440, 690));
            editor.style = shapes::BOXED | shapes::BOLD;
            type_in(editor, (1540, 836), "fix this one");
            editor.style = 0;
        };

        // 1. The screen frozen and dimmed, the terminal taken with a
        // click, and the bar of tools under it.
        let mut editor = fresh();
        click(&mut editor, (1400, 800), 0);
        editor.pointer_moved(1500, 640, PLAIN);
        screen(&mut editor, "1-select.png");

        // 2. Drawn on.
        annotated(&mut editor);
        editor.key(Key::Char('v'), PLAIN);
        click(&mut editor, (1700, 600), 60_000);
        editor.pointer_moved(1700, 440, PLAIN);
        screen(&mut editor, "2-annotate.png");

        // 3. What comes of it: set on a backdrop, the corners rounded,
        // with Tailwind's 2xl for a shadow.
        editor.corners = 12.0;
        editor.frame = Frame { backdrop: Backdrop::Gradient(frame::named("sorbet").unwrap()), pad: (72, 56), radius: 18, shadow: Shadow::named("2xl").unwrap(), ..Frame::default() };
        let (_, wide, high, pixels, alpha) = editor.framed().unwrap();
        std::fs::write(dir.join("3-framed.png"), crate::output::encode(wide as u32, high as u32, &pixels, alpha).unwrap()).unwrap();

        // 4. Code: what is wrong lit and the rest dimmed, a lens on it,
        // and a badge, on the window's own area as it comes out.
        let mut editor = fresh();
        click(&mut editor, (500, 400), 0);
        stroke(&mut editor, 's', (180, 538), (760, 712));
        // A badge in the empty space under the code, and an arrow from
        // it, bent, to the line it is about.
        editor.color = PALETTE[1].0;
        editor.choose(Cell::Badge(6));
        for letter in "returns before members load".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        editor.key(Key::Enter, PLAIN);
        let last = editor.shapes.len() - 1;
        let at = editor.shapes[last].points[0];
        editor.shapes[last].translate(560.0 - at.0, 772.0 - at.1);
        stroke(&mut editor, 'a', (580, 768), (646, 662));
        let arrow = editor.shapes.len() - 1;
        editor.shapes[arrow].bend_to((680.0, 730.0));
        click(&mut editor, (900, 200), 70_000);
        editor.corners = 10.0;
        editor.frame = Frame { backdrop: Backdrop::Gradient(frame::named("midnight").unwrap()), pad: (64, 48), radius: 16, shadow: Shadow::named("xl").unwrap(), ..Frame::default() };
        let (_, wide, high, pixels, alpha) = editor.framed().unwrap();
        std::fs::write(dir.join("4-explain.png"), crate::output::encode(wide as u32, high as u32, &pixels, alpha).unwrap()).unwrap();
    }

    #[test]
    fn a_shape_turns_by_just_outside_a_corner_and_is_sized_along_its_own_sides() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (300, 300), (500, 400));
        let middle = editor.shapes[0].centre(&mut editor.text);
        let corner = editor.turners(0)[3];
        editor.pointer_moved(corner.0, corner.1, PLAIN);
        assert_eq!((editor.hover, editor.cursor()), (Target::Turn, Cursor::Turn));
        let undone = editor.undo.len();

        // Round a quarter about its middle, and a little more: with
        // Shift, to the nearest fifteen degrees, which is the quarter.
        let shift = Mods { shift: true, ..PLAIN };
        let (dx, dy) = (corner.0 as f32 - middle.0, corner.1 as f32 - middle.1);
        let a_quarter_on = |by: f32| {
            let (sin, cos) = (std::f32::consts::FRAC_PI_2 + by).sin_cos();
            ((middle.0 + dx * cos - dy * sin).round() as i32, (middle.1 + dx * sin + dy * cos).round() as i32)
        };
        editor.pressed(5000);
        let (half, far) = (a_quarter_on(-0.8), a_quarter_on(0.05));
        editor.pointer_moved(half.0, half.1, shift);
        editor.pointer_moved(far.0, far.1, shift);
        editor.released();
        let shape = &editor.shapes[0];
        assert!((shape.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{}", shape.angle);
        assert_eq!(editor.undo.len(), undone + 1);
        // Turned a quarter, it stands as tall as it was wide, about the
        // same middle.
        let place = editor.shapes[0].bounds(&mut editor.text);
        assert!((place.w - 100).abs() <= 12 && (place.h - 200).abs() <= 12, "{place:?}");
        assert_eq!(editor.shapes[0].centre(&mut editor.text), middle);

        // Its own right side is now at the bottom: pulled down by that
        // grip, it grows long, its left side, at the top, staying put.
        let top = editor.grip_of(0, -1, 0);
        let side = editor.grip_of(0, 1, 0);
        assert!(side.1 > top.1 + 150 && (side.0 - top.0).abs() <= 1, "{side:?} {top:?}");
        drag(&mut editor, side, (side.0, side.1 + 60));
        let shape = &editor.shapes[0];
        let ((_, _, w, h), place) = (shape.frame(), editor.shapes[0].bounds(&mut editor.text));
        assert!(w > 250.0 && w < 262.0 && (h - 100.0).abs() < 1.0, "{w} {h}");
        assert!((place.y - top.1).abs() <= 8 && (place.h - 265).abs() <= 12, "{place:?} {top:?}");
        editor.key(Key::Char('z'), Mods { ctrl: true, ..PLAIN });
        editor.key(Key::Char('z'), Mods { ctrl: true, ..PLAIN });
        assert_eq!(editor.shapes[0].angle, 0.0);
    }

    #[test]
    fn a_box_has_a_border_and_a_fill_each_colored_or_none_from_the_bar_over_it() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (300, 300), (500, 400));
        // Picked out, the bar over it has its fill, its border and its
        // width, and what can be done with it; and is over it.
        assert_eq!(tray(&mut editor), [Button::Fill, Button::Border, Button::Width, Button::Sketch, Button::Forward, Button::Backward, Button::Duplicate, Button::Delete]);
        let over = editor.layout().tray.expect("the bar over it");
        assert!(over.bottom() < 300);
        // The fill: the colors for it, and yellow; the border keeps its red.
        let fill = button(&mut editor, Button::Fill);
        click(&mut editor, fill, 1000);
        assert_eq!((editor.popup.is_some(), editor.paint, editor.layout().paints.len()), (true, Paint::Fill, 2));
        let yellow = editor.layout().swatches.iter().find(|(_, color)| *color == 0xFFE629).map(|(rect, _)| rect.center()).expect("yellow");
        click(&mut editor, yellow, 2000);
        let shape = &editor.shapes[0];
        assert_eq!((shape.filled, shape.fill, shape.border, shape.color), (true, 0xFFE629, true, RED));
        // No border: only the fill is left, and there is no width to have.
        let border = button(&mut editor, Button::Border);
        click(&mut editor, border, 3000);
        let none = editor.layout().none.expect("none").center();
        click(&mut editor, none, 4000);
        assert_eq!((editor.shapes[0].border, editor.shapes[0].filled), (false, true));
        assert!(!tray(&mut editor).contains(&Button::Width));
        // And no fill either: it is not left with nothing, but gets its
        // border back.
        let fills = editor.layout().paints[1].0.center();
        click(&mut editor, fills, 5000);
        let none = editor.layout().none.expect("none").center();
        click(&mut editor, none, 6000);
        assert_eq!((editor.shapes[0].border, editor.shapes[0].filled), (true, false));
        // The next is drawn as this one was left.
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (600, 300), (700, 400));
        assert_eq!((editor.shapes[1].border, editor.shapes[1].filled), (true, false));
    }

    #[test]
    fn what_is_picked_out_goes_up_and_down_from_the_bar_over_it_and_is_copied_and_deleted() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        for (from, to) in [((200, 200), (400, 300)), ((250, 250), (450, 350)), ((300, 300), (500, 400))] {
            editor.key(Key::Char('r'), PLAIN);
            drag(&mut editor, from, to);
        }
        let colors = |editor: &Editor| editor.shapes.iter().map(|shape| shape.stroke as u32).collect::<Vec<_>>();
        for (index, shape) in editor.shapes.iter_mut().enumerate() {
            shape.stroke = index as f32 + 1.0;
        }
        // The first, picked out: forward a step, then with Shift to the top.
        editor.selected = vec![0];
        let forward = button(&mut editor, Button::Forward);
        click(&mut editor, forward, 0);
        assert_eq!((colors(&editor), editor.selected.clone()), (vec![2, 1, 3], vec![1]));
        editor.pointer_moved(forward.0, forward.1, Mods { shift: true, ..PLAIN });
        editor.pressed(1000);
        editor.released();
        assert_eq!((colors(&editor), editor.selected.clone()), (vec![2, 3, 1], vec![2]));
        let backward = button(&mut editor, Button::Backward);
        editor.pointer_moved(backward.0, backward.1, PLAIN);
        editor.pressed(2000);
        editor.released();
        assert_eq!(colors(&editor), [2, 1, 3]);
        // A copy, picked out in its place; and away with it.
        let duplicate = button(&mut editor, Button::Duplicate);
        click(&mut editor, duplicate, 3000);
        assert_eq!((editor.shapes.len(), editor.selected.clone()), (4, vec![3]));
        let delete = button(&mut editor, Button::Delete);
        click(&mut editor, delete, 4000);
        assert_eq!((editor.shapes.len(), editor.selected.is_empty()), (3, true));
        // Out of the way while a shape is moved, and back after.
        editor.selected = vec![0];
        assert!(editor.layout().tray.is_some());
        editor.pointer_moved(250, 330, PLAIN);
        editor.pressed(10_000);
        editor.pointer_moved(270, 350, PLAIN);
        assert!(editor.layout().tray.is_none());
        editor.released();
        assert!(editor.layout().tray.is_some());
    }

    #[test]
    fn any_color_is_picked_in_the_square_and_the_strip_and_kept_as_taken_lately() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('r'), PLAIN);
        drag(&mut editor, (200, 200), (400, 300));
        let color = button(&mut editor, Button::Border);
        click(&mut editor, color, 9000);
        let (square, hues) = (editor.layout().square, editor.layout().hues);
        assert!(!square.is_empty() && !hues.is_empty());
        let undone = editor.undo.len();

        // A pure red, from the hue strip's start and the square's top right.
        drag(&mut editor, (hues.x + 1, hues.center().1), (hues.x, hues.center().1));
        drag(&mut editor, (square.x + square.w / 2, square.y + square.h / 2), (square.right() + 20, square.y - 20));
        assert_eq!((editor.color, editor.shapes[0].color), (0xFF0000, 0xFF0000));
        // A third of the way along the strip is green, kept as saturated.
        drag(&mut editor, (hues.x + hues.w / 2, hues.center().1), (hues.x + hues.w / 3, hues.center().1));
        let (r, g, b) = (editor.color >> 16, (editor.color >> 8) & 0xFF, editor.color & 0xFF);
        assert!(g == 0xFF && r < 0x10 && b < 0x10, "{:06X}", editor.color);
        assert_eq!(editor.shapes[0].color, editor.color);
        // Each drag is a step to undo, and the box stays open for more.
        assert_eq!((editor.undo.len(), editor.popup.is_some()), (undone + 3, true));

        // What was picked is first among those taken lately, beside the hex.
        assert_eq!(editor.lately.first(), Some(&editor.color));
        let lately = editor.layout().swatches[0];
        assert_eq!((lately.1, lately.0.y), (editor.color, editor.layout().hex.y));
        // A gray keeps the hue it was made at.
        drag(&mut editor, (square.x + 5, square.y + 5), (square.x - 10, square.center().1));
        assert!((editor.hsv.0 - 120.0).abs() < 2.0, "{:?}", editor.hsv);
        assert_eq!(editor.color & 0xFF, editor.color >> 16);

        // Typed in hex: a click on it, or #; what is no digit is passed
        // over, and three digits are each said twice.
        let hex = editor.layout().hex.center();
        click(&mut editor, hex, 30_000);
        for key in [Key::Char('1'), Key::Char('g'), Key::Char('2'), Key::Backspace, Key::Char('a'), Key::Char('b'), Key::Char('c'), Key::Char('d'), Key::Char('e')] {
            editor.key(key, PLAIN);
        }
        assert_eq!((editor.hex.as_deref(), editor.tool), (Some("1ABCDE"), Tool::Select));
        editor.key(Key::Enter, PLAIN);
        assert_eq!((editor.color, editor.shapes[0].color, editor.hex.clone(), editor.lately[0]), (0x1ABCDE, 0x1ABCDE, None, 0x1ABCDE));
        for key in [Key::Char('#'), Key::Char('f'), Key::Char('8'), Key::Char('0'), Key::Enter] {
            editor.key(key, PLAIN);
        }
        assert_eq!((editor.shapes[0].color, editor.popup.is_some()), (0xFF8800, true));
        // Esc leaves it as it was.
        for key in [Key::Char('#'), Key::Char('0'), Key::Escape] {
            editor.key(key, PLAIN);
        }
        assert_eq!((editor.shapes[0].color, editor.hex.clone()), (0xFF8800, None));
    }
}
