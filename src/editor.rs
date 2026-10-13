//! Everything between the screen being frozen and the picture being
//! taken: the area dragged out, what is drawn on it, and the bar of
//! tools. It knows nothing of the display: it is told of the pointer and
//! the keys, and paints the screen into a buffer it is handed. That is
//! what lets it be tried, and timed, without a display at all.

use std::ops::Deref;

use crate::canvas::{self, Canvas, Rect};
use crate::shapes::{self, Shape, Tool};
use crate::frame::{self, Backdrop, Frame, Photo, GRADIENTS, PADS};
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
const STYLES: [(u8, &str); 4] = [(shapes::BOLD, "B"), (shapes::ITALIC, "I"), (shapes::HIGHLIGHT, "H"), (shapes::BULLETS, "\u{2022}")];

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
    pub pixels: Box<dyn Deref<Target = [u32]>>,
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
}

/// What is done with the picture once it is taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Finish {
    pub copy: bool,
    pub save: bool,
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
    /// Whether rectangles and ovals are drawn filled to begin with.
    pub filled: bool,
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings { accent: 0x3E63DD, color: RED, size: 1, magnifier: false, windows: Windows::Outline, dim: 50, radius: 6.0, corners: 0.0, filled: false, face: 0, frame: Frame::default(), photo: None, places: Vec::new(), place: 0, enter: Finish { copy: true, save: false } }
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
    Color,
    Size(usize),
    Fill,
    /// The face text is in: each press, the next of them.
    Face,
    Undo,
    Redo,
    Cancel,
    Save,
    Copy,
    /// The badges and stickers there are to put on the picture.
    Badges,
    /// What the picture is set on, and how.
    Frame,
    /// Which folder Save saves to.
    Places,
    /// Every emoji there is, to be searched and put on the picture.
    Emoji,
}

/// What can be put on the picture ready-made: a word on a pill of a
/// pale color, with a dot and the word in the strong one of the same
/// hue. The hues are Radix's, its step for a tinted surface and its
/// step for text; the look is the one envelope's badges have.
///
/// A badge is three things chosen apart: its word, its look, and its
/// hue. The words are these, any of which can be typed over once it is
/// on the picture.
const BADGE_WORDS: [&str; 14] = ["Bug", "Fixed", "New", "To do", "Important", "Note", "Before", "After", "Approved", "Rejected", "WIP", "Question", "Beta", "Tip"];

/// The hues a badge can be in: the pale of each, and its full strength.
const BADGE_HUES: [(u32, u32); 8] = [
    (0xFEEBE7, 0xE54D2E),
    (0xFFEFD6, 0xF76B15),
    (0xFFF7C2, 0xFFC53D),
    (0xE6F6EB, 0x30A46C),
    (0xE6F4FE, 0x0090FF),
    (0xF4F0FE, 0x6E56CF),
    (0xFEE9F5, 0xD6409F),
    (0xF0F0F0, 0x8D8D8D),
];

/// The emoji there are to put on it, as stickers.
fn emoji() -> &'static [(&'static str, &'static str)] {
    static LIST: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| include_str!("../assets/emoji.txt").lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect())
}

/// How many emoji there are across the box they are chosen from, and
/// how many rows of them show at once.
const EMOJI_ACROSS: i32 = 10;
const EMOJI_DOWN: i32 = 7;

/// A thing to be chosen in a box that hangs from the bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    /// One of the words a badge can say.
    Word(usize),
    /// A badge in one of its looks and hues: eight hues to a look.
    Badge(usize),
    /// An emoji: which of those the search has found.
    Emoji(usize),
    /// One of the ways a tool can draw: a tool's own short list.
    Variant(usize),
    /// What the picture is set on: nothing, or one of the gradients
    /// counted from 1.
    Backdrop(usize),
    /// A photograph of the user's own, behind the picture.
    Own,
    /// A photograph nobody chose, and a different one each time.
    Dice,
    Shadow,
    /// One of the folders Save can save to.
    Place(usize),
}

/// The bar of tools, in the groups it is divided into.
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
        Button::Emoji,
    ],
    &[Button::Color, Button::Size(0), Button::Size(1), Button::Size(2), Button::Size(3), Button::Fill, Button::Face],
    &[Button::Undo, Button::Redo],
    &[Button::Frame],
    &[Button::Cancel, Button::Save, Button::Places, Button::Copy],
];

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
            Button::Tool(Tool::Badge) | Button::Badges => ("Badges", "J"),
            Button::Emoji => ("Emoji", "M"),
            Button::Color => ("Color", "C"),
            Button::Size(0) => ("Thin", "1"),
            Button::Size(1) => ("Medium", "2"),
            Button::Size(2) => ("Thick", "3"),
            Button::Size(_) => ("Heavy", "4"),
            Button::Fill => ("Filled", "F"),
            Button::Face => ("Font", "G"),
            Button::Undo => ("Undo", "Ctrl+Z"),
            Button::Redo => ("Redo", "Ctrl+Shift+Z"),
            Button::Cancel => ("Cancel", "Esc"),
            Button::Save => ("Save", "Ctrl+S"),
            Button::Frame => ("Backdrop, room and shadow", "W"),
            Button::Places => ("Where Save saves to", ""),
            Button::Copy => ("Copy", "Ctrl+C"),
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
            Button::Fill => icons::FILLED,
            Button::Undo => icons::UNDO,
            Button::Redo => icons::REDO,
            Button::Cancel => icons::CLOSE,
            Button::Save => icons::SAVE,
            Button::Frame => icons::FRAME,
            Button::Places => icons::DOWN,
            Button::Copy => icons::COPY,
            Button::Color | Button::Size(_) | Button::Face => "",
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
    /// One of the ways text is set: heavy, slanted and the rest.
    Style(u8),
    Pipette,
    /// A part of the bar, or of the box of colors, that is not a button.
    Panel,
    /// Something to be chosen in a box hung from the bar.
    Cell(Cell),
    /// An edge or a corner of the selection: which way it pulls across
    /// and down, each -1, 0 or 1.
    Handle(i32, i32),
    /// A corner of the box round the shape picked out, the same way.
    Grip(i32, i32),
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
    Room { from: (i32, i32), slack: (i32, i32), sides: (bool, bool), moved: bool },
    /// Making the corners of the frame rounder, by the dot in one.
    FrameCorner { slack: i32 },
    /// Dragging out a box, with Shift or Ctrl held, to pick out the
    /// shapes it touches.
    Marquee { from: (i32, i32) },
    Button(Button),
}

/// Where the box of colors hangs from.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Anchor {
    Bar,
    At(i32, i32),
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
    /// The small bar over text being typed or picked out: how it is set.
    styles: Vec<(Rect, u8)>,
    styles_bar: Option<Rect>,
    pipette: Rect,
    /// The box hung from one of the bar's buttons, and what is in it.
    flyout: Option<Rect>,
    cells: Vec<(Rect, Cell)>,
    /// Where in that box the picture is shown as it will be, framed.
    look: Option<Rect>,
    /// The line to type a search in, and the line a name is said on.
    field: Option<Rect>,
    caption: Option<Rect>,
}

pub struct Editor {
    shot: Shot,
    /// The screen darkened, as it shows outside the selection.
    dim: Vec<u32>,
    monitors: Vec<Rect>,
    /// The windows on the screen, the topmost first.
    windows: Vec<Rect>,
    settings: Settings,
    text: Text,

    pointer: (i32, i32),
    mods: Mods,
    selection: Option<Rect>,
    drag: Drag,
    tool: Tool,
    color: u32,
    size: usize,
    filled: bool,
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
    /// Whether lines and arrows are drawn in dashes.
    dashed: bool,
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
    /// Which of the folders Save saves to.
    place: usize,
    /// The frame as it shows round the selection: what it was made
    /// from, how wide it is, and its pixels, each with how opaque.
    surround: Option<(String, i32, Vec<u32>)>,
    /// Which of the words the next badge says.
    word: usize,
    /// How opaque what is drawn is, out of 256; and how text is set.
    opacity: u32,
    style: u8,
    /// Whether the next zoom is square, where it is usually round.
    square: bool,
    /// The shapes being moved, as they were when taken hold of; and the
    /// lines they have gone to, across and down, to be shown.
    moving: Vec<(usize, Shape)>,
    guides: Vec<(bool, i32)>,
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

/// The ways a tool can draw, for the tools that have more than one:
/// what is behind the small mark in the corner of its button.
fn variants(tool: Tool) -> &'static [&'static str] {
    match tool {
        Tool::Arrow => &["One head", "A head at each end", "Dashed"],
        Tool::Line => &["Solid", "Dashed"],
        Tool::Rect | Tool::Ellipse => &["Outlined", "Filled"],
        Tool::Zoom => &["Round", "Square"],
        _ => &[],
    }
}

/// A place along the screen, moved to the nearest of some edges if one
/// is within reach of it, and otherwise left where it is.
fn snapped(edges: &[i32], value: i32, reach: i32) -> i32 {
    edges.iter().copied().filter(|edge| (edge - value).abs() <= reach).min_by_key(|edge| (edge - value).abs()).unwrap_or(value)
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
            color: settings.color,
            corners: settings.corners,
            filled: settings.filled,
            face: settings.face.min(FACES.len() - 1),
            frame: settings.frame,
            place: settings.place.min(settings.places.len().saturating_sub(1)),
            photo: None,
            wish: None,
            photos: 0,
            look: None,
            surround: None,
            word: 0,
            opacity: 256,
            style: 0,
            square: false,
            moving: Vec::new(),
            guides: Vec::new(),
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
            tool: Tool::Select,
            shapes: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            selected: Vec::new(),
            hovered: None,
            scaling: None,
            editing: None,
            caret: 0,
            heads: [false, true],
            popup: None,
            flyout: None,
            dashed: false,
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
        let count: i32 = GROUPS.iter().map(|group| group.len() as i32).sum();
        let width = 2 * margin + count * side + (GROUPS.len() as i32 - 1) * gap;
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
            for (index, group) in GROUPS.iter().enumerate() {
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

        // Over the text being typed, or picked out by itself, a small
        // bar of the ways it can be set: heavy, slanted, gone over
        // with a marker, and each line after a dot. Under it, where
        // there is no room over.
        if let (Some(index), false) = (self.styled(), reshaping) {
            let around = self.shapes[index].bounds(&mut self.text);
            let (wide, high) = (STYLES.len() as i32 * side + 2 * margin, side + 2 * margin);
            let top = if around.y - high - pad >= monitor.y { around.y - high - pad } else { around.bottom() + pad };
            let place = Rect::new(around.x, top, wide, high).kept_within(monitor.inflate(-pad));
            for (index, (style, _)) in STYLES.iter().enumerate() {
                layout.styles.push((Rect::new(place.x + margin + index as i32 * side, place.y + margin, side, side), *style));
            }
            layout.styles_bar = Some(place);
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
            // A row more than the colors and the pipette: how opaque.
            let height = rows * side + 2 * side + 2 * margin + pad / 2;
            let at = match anchor {
                Anchor::At(x, y) => (x + pad / 2, y + pad / 2),
                Anchor::Bar => {
                    let button = layout.buttons.iter().find(|(_, button)| *button == Button::Color).map_or(bar, |(rect, _)| *rect);
                    let under = bar.bottom() + pad / 2;
                    (button.x + button.w / 2 - width / 2, if under + height <= monitor.bottom() - pad { under } else { bar.y - pad / 2 - height })
                }
            };
            let popup = Rect::new(at.0, at.1, width, height).kept_within(monitor.inflate(-pad));
            let across = (width - 2 * margin) / columns;
            for (index, &(color, _)) in PALETTE.iter().enumerate() {
                let (column, row) = (index as i32 % columns, index as i32 / columns);
                layout.swatches.push((Rect::new(popup.x + margin + column * across, popup.y + margin + row * side, across, side), color));
            }
            // How opaque: four steps of it, under the colors.
            let each = (width - 2 * margin) / OPACITIES.len() as i32;
            for (index, &opacity) in OPACITIES.iter().enumerate() {
                layout.opacities.push((Rect::new(popup.x + margin + index as i32 * each, popup.y + margin + rows * side, each, side), opacity));
            }
            layout.pipette = Rect::new(popup.x + margin, popup.y + margin + (rows + 1) * side + pad / 2, width - 2 * margin, side);
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
                    let across = (GRADIENTS.len() as i32 + 1) * side;
                    // The picture itself shows what is chosen, where it
                    // is, so there is nothing here but the choosing.
                    let look = -pad;
                    for index in 0..=GRADIENTS.len() {
                        cells.push((Rect::new(index as i32 * side, look + pad, side, side), Cell::Backdrop(index)));
                    }
                    let mut row = vec![Cell::Dice];
                    row.extend(self.settings.photo.is_some().then_some(Cell::Own));
                    row.push(Cell::Shadow);
                    for (index, cell) in row.into_iter().enumerate() {
                        cells.push((Rect::new(index as i32 * side, look + pad + side, side, side), cell));
                    }
                    (across, look + pad + 2 * side)
                }
                // Every emoji there is, or those whose names have in
                // them what has been typed: a line to type in, as many
                // rows of them as show at once, and under those the
                // name of the one the pointer is on.
                Button::Emoji => {
                    let across = EMOJI_ACROSS * side;
                    layout.field = Some(Rect::new(0, 0, across, side));
                    let found = self.emoji_found();
                    let top = side + pad / 2;
                    for at in 0..EMOJI_ACROSS * EMOJI_DOWN {
                        let index = (self.scroll * EMOJI_ACROSS + at) as usize;
                        if index < found.len() {
                            cells.push((Rect::new(at % EMOJI_ACROSS * side, top + at / EMOJI_ACROSS * side, side, side), Cell::Emoji(index)));
                        }
                    }
                    let line = Text::line_height(px);
                    layout.caption = Some(Rect::new(0, top + EMOJI_DOWN * side + pad / 4, across, line));
                    (across, top + EMOJI_DOWN * side + pad / 4 + line)
                }
                // The ways a tool can draw, one to a row.
                Button::Tool(tool) => {
                    let ways = variants(tool);
                    let widest = ways.iter().map(|way| self.text.width(way, px)).max().unwrap_or(0);
                    let across = widest + side + 2 * pad;
                    for index in 0..ways.len() {
                        cells.push((Rect::new(0, index as i32 * side, across, side), Cell::Variant(index)));
                    }
                    (across, ways.len() as i32 * side)
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
                // The badges as they will look, one after another for
                // as far as a row goes, and the stickers in rows of
                // eight.
                _ => {
                    // The words first, to choose what it says; then every
                    // look of it in every hue, a look to a row; then
                    // the stickers.
                    let hues = BADGE_HUES.len() as i32;
                    let each = side * 3 / 2;
                    let across = hues * each;
                    let (mut x, mut y) = (0, 0);
                    for (index, word) in BADGE_WORDS.iter().enumerate() {
                        let wide = self.text.width(word, px) + 2 * pad;
                        if x > 0 && x + wide > across {
                            (x, y) = (0, y + side);
                        }
                        cells.push((Rect::new(x, y, wide, side), Cell::Word(index)));
                        x += wide;
                    }
                    y += side + pad / 2;
                    for index in 0..(shapes::BADGE_STYLES as i32 * hues) {
                        cells.push((Rect::new(index % hues * each, y + index / hues * side, each, side), Cell::Badge(index as usize)));
                    }
                    (across, y + shapes::BADGE_STYLES as i32 * side)
                }
            };
            let (width, height) = (across + 2 * margin, down + 2 * margin);
            let button = layout.buttons.iter().find(|(_, button)| *button == from).map_or(bar, |(rect, _)| *rect);
            let under = bar.bottom() + pad / 2;
            let top = if under + height <= monitor.bottom() - pad { under } else { bar.y - pad / 2 - height };
            let flyout = Rect::new(button.x + button.w / 2 - width / 2, top, width, height).kept_within(monitor.inflate(-pad));
            layout.cells = cells.into_iter().map(|(rect, cell)| (rect.moved(flyout.x + margin, flyout.y + margin), cell)).collect();
            layout.look = layout.look.map(|look| look.moved(flyout.x + margin, flyout.y + margin));
            layout.field = layout.field.map(|field| field.moved(flyout.x + margin, flyout.y + margin));
            layout.caption = layout.caption.map(|caption| caption.moved(flyout.x + margin, flyout.y + margin));
            layout.flyout = Some(flyout);
        }
        layout
    }

    fn target_at(&mut self, (x, y): (i32, i32)) -> Target {
        let layout = self.layout();
        if let Some(bar) = layout.styles_bar {
            if let Some((_, style)) = layout.styles.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Style(*style);
            }
            if bar.contains(x, y) {
                return Target::Panel;
            }
        }
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
            if layout.pipette.contains(x, y) {
                return Target::Pipette;
            }
            if popup.contains(x, y) {
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
                let frame = self.frame_of(index);
                if let Some((across, down)) = GRIPS.into_iter().find(|&(across, down)| near(grip_at(frame, across, down))) {
                    return Target::Grip(across, down);
                }
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
        if selection.contains(x, y) {
            Target::Inside
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

    /// The ends a shape is reshaped by, each with which of its points it
    /// is: both of a line or an arrow, and of a counter the tip it
    /// points with, when it points.
    fn ends(&self, index: usize) -> Vec<(usize, (i32, i32))> {
        let shape = &self.shapes[index];
        let at = |point: usize| (point, (shape.points[point].0.round() as i32, shape.points[point].1.round() as i32));
        match shape.tool {
            Tool::Arrow | Tool::Line => vec![at(0), at(1)],
            Tool::Counter if shape.points[0] != shape.points[1] => vec![at(1)],
            // The corner the larger picture of a zoom is put down by.
            // And its far corner, which says how many times larger.
            Tool::Zoom => vec![at(2), at(3)],
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
        if !matches!(shape.tool, Tool::Rect | Tool::Spotlight) || width.min(height) < least * 3.0 {
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
            (Drag::None, Target::Handle(across, down) | Target::Grip(across, down)) => Some((across, down)),
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
            (_, Target::Button(_) | Target::Swatch(_) | Target::Pipette | Target::Panel | Target::Cell(_) | Target::Backdrop | Target::Style(_) | Target::Opacity(_)) => Cursor::Arrow,
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
                let (index, pull, origin, by) = (*index, *pull, *origin, (at.0 - from.0, at.1 - from.1));
                let Some(before) = self.scaling.clone() else { return };
                // How many times as wide and as tall its box now is; less
                // than nought when the corner has been pulled past the
                // one opposite, which turns the shape over.
                let times = |pull: i32, by: i32, size: i32| {
                    let size = size.max(1) as f32;
                    let times = if pull == 0 { 1.0 } else { (size + (pull * by) as f32) / size };
                    if times.abs() < 0.02 { 0.02f32.copysign(times) } else { times }
                };
                let (mut across, mut down) = (times(pull.0, by.0, origin.w), times(pull.1, by.1, origin.h));
                // Text keeps its shape, and anything does with Shift.
                if before.tool == Tool::Text {
                    (across, down) = (across.abs().max(down.abs()), across.abs().max(down.abs()));
                } else if mods.shift {
                    let both = across.abs().max(down.abs());
                    (across, down) = (both.copysign(across), both.copysign(down));
                }
                // The corner opposite the one held stays where it is.
                let still = (if pull.0 < 0 { origin.right() } else { origin.x } as f32, if pull.1 < 0 { origin.bottom() } else { origin.y } as f32);
                let mut shape = before.clone();
                for point in &mut shape.points {
                    *point = (still.0 + (point.0 - still.0) * across, still.1 + (point.1 - still.1) * down);
                }
                if shape.tool == Tool::Text {
                    shape.font_px = ((before.font_px as f32 * down).round() as u32).max(6);
                }
                shape.touch();
                self.shapes[index] = shape;
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
                // With Shift, at one of eight angles from the other end.
                let other = shape.points[1 - point.min(1)];
                let was = shape.points[point];
                shape.points[point] = if mods.shift && shape.tool != Tool::Zoom { shapes::constrained(Tool::Line, other, to) } else { to };
                if shape.tool == Tool::Zoom {
                    shape.settle_zoom(point, (to.0 - was.0, to.1 - was.1));
                }
                shape.touch();
                self.dirty = true;
            }
            Drag::Marquee { .. } => self.dirty = true,
            Drag::Room { from, slack, sides, moved } => {
                *moved |= (at.0 - from.0).abs().max((at.1 - from.1).abs()) >= DRAG_AFTER;
                let (slack, sides, moved) = (*slack, *sides, *moved);
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
                    let at = if mods.ctrl { at } else { (snapped(&self.edges.0, at.0, self.reach), snapped(&self.edges.1, at.1, self.reach)) };
                    let area = Rect::spanning(*from, at);
                    self.selection = Some(Rect::new(area.x, area.y, area.w + 1, area.h + 1));
                    (self.window, self.popup) = (None, None);
                    self.selected.clear();
                    self.dirty = true;
                }
            }
            Drag::Move { from, origin } => {
                self.selection = Some(origin.moved(at.0 - from.0, at.1 - from.1).kept_within(screen));
                self.dirty = true;
            }
            Drag::Resize { pull, from, origin } => {
                let (dx, dy) = (at.0 - from.0, at.1 - from.1);
                // An edge being pulled goes to the edge of a window, when
                // it comes near one, unless Ctrl says not.
                let near = |edges: &[i32], value: i32, pulled: bool| if pulled && !mods.ctrl { snapped(edges, value, self.reach) } else { value };
                let left = near(&self.edges.0, origin.x + if pull.0 < 0 { dx } else { 0 }, pull.0 < 0);
                let right = near(&self.edges.0, origin.right() + if pull.0 > 0 { dx } else { 0 }, pull.0 > 0);
                let top = near(&self.edges.1, origin.y + if pull.1 < 0 { dy } else { 0 }, pull.1 < 0);
                let bottom = near(&self.edges.1, origin.bottom() + if pull.1 > 0 { dy } else { 0 }, pull.1 > 0);
                // Pulled past the far side, it turns inside out.
                let area = Rect::new(left.min(right), top.min(bottom), (right - left).abs().max(1), (bottom - top).abs().max(1));
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
                        tool => {
                            shape.points[1] = if mods.shift { shapes::constrained(tool, from, to) } else { to };
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
                    self.checkpoint();
                    // All that are picked out go with the one held.
                    let mut going = self.chosen();
                    if !going.contains(&index) {
                        going = vec![index];
                    }
                    self.moving = going.into_iter().map(|index| (index, self.shapes[index].clone())).collect();
                }
                let (mut by, mut guides) = (by, Vec::new());
                if !mods.ctrl {
                    // The box round what is moving, where it would be;
                    // and the edges and middles of everything else, and
                    // of the selection, for its own to go to.
                    let mut around: Option<Rect> = None;
                    for (_, shape) in &self.moving {
                        let place = shape.bounds(&mut self.text).moved(by.0, by.1);
                        around = Some(around.map_or(place, |all| Rect::spanning((all.x.min(place.x), all.y.min(place.y)), (all.right().max(place.right()), all.bottom().max(place.bottom())))));
                    }
                    let held: Vec<usize> = self.moving.iter().map(|(index, _)| *index).collect();
                    let mut others: Vec<Rect> = (0..self.shapes.len()).filter(|index| !held.contains(index)).map(|index| self.shapes[index].bounds(&mut self.text)).collect();
                    others.extend(self.selection);
                    if let Some(around) = around {
                        let lines = |from: i32, size: i32| [from, from + size / 2, from + size];
                        let nearest = |mine: [i32; 3], theirs: Vec<i32>| mine.iter().flat_map(|&at| theirs.iter().map(move |&line| (line - at, line))).filter(|(off, _)| off.abs() <= 6).min_by_key(|(off, _)| off.abs());
                        if let Some((off, line)) = nearest(lines(around.x, around.w), others.iter().flat_map(|rect| lines(rect.x, rect.w)).collect()) {
                            by.0 += off;
                            guides.push((true, line));
                        }
                        if let Some((off, line)) = nearest(lines(around.y, around.h), others.iter().flat_map(|rect| lines(rect.y, rect.h)).collect()) {
                            by.1 += off;
                            guides.push((false, line));
                        }
                    }
                }
                for (index, before) in &self.moving {
                    let mut shape = before.clone();
                    shape.translate(by.0 as f32, by.1 as f32);
                    self.shapes[*index] = shape;
                }
                self.guides = guides;
                self.dirty = true;
            }
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
            Target::Style(style) => self.toggle_style(style),
            Target::Pipette => (self.pipette, self.popup) = (true, None),
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
                    self.drag = Drag::Scale { index, pull: (across, down), from: at, origin: self.frame_of(index) };
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
            }
            // On the frame: a drag makes it wider or narrower, and a
            // click opens what there is to choose about it.
            Target::Backdrop => {
                self.commit_text();
                let (out, room) = (self.outward(at), self.frame_room());
                self.drag = Drag::Room { from: at, slack: (out.0 - room.0, out.1 - room.1), sides: (out.0 > 0, out.1 > 0), moved: false };
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
                shape.opacity = self.opacity;
                shape.radius = self.settings.radius * self.scale();
                shape.font_px = self.font_px();
                if tool == Tool::Arrow {
                    shape.heads = self.heads;
                }
                shape.dashed = self.dashed && matches!(tool, Tool::Arrow | Tool::Line);
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
                }
            }
            Drag::Scale { .. } => self.scaling = None,
            Drag::Shape { .. } => (self.moving, self.guides) = (Vec::new(), Vec::new()),
            // A click on the frame, not a drag of it, opens what there
            // is to choose about it.
            Drag::Room { moved: false, .. } => outcome = self.activate(Button::Frame),
            // A click on an end of a line or an arrow, not a drag of it,
            // gives that end an arrow's head, or takes it away; and the
            // arrows drawn after have theirs where this one now does.
            Drag::End { index, point, moved: false, .. } => {
                let shape = &mut self.shapes[index];
                if matches!(shape.tool, Tool::Arrow | Tool::Line) {
                    shape.heads[point] = !shape.heads[point];
                    shape.touch();
                    if shape.tool == Tool::Arrow && shape.heads != [false, false] {
                        self.heads = shape.heads;
                    }
                } else {
                    self.undo.pop();
                }
            }
            Drag::Marquee { from } => {
                let reach = Rect::spanning(from, self.pointer);
                for index in 0..self.shapes.len() {
                    if !self.selected.contains(&index) && self.shapes[index].bounds(&mut self.text).intersects(reach) {
                        self.selected.push(index);
                    }
                }
            }
            Drag::Button(button) => {
                if self.target_at(self.pointer) == Target::Button(button) {
                    // A press on the mark in the corner of a tool's
                    // button is for the list of its ways, whichever
                    // tool is in hand: it is taken in hand first.
                    let place = self.layout().buttons.iter().find(|(_, other)| *other == button).map(|(rect, _)| *rect);
                    let corner = place.is_some_and(|rect| self.pointer.0 > rect.x + rect.w * 3 / 5 && self.pointer.1 > rect.y + rect.h * 3 / 5);
                    if let (Button::Tool(tool), true) = (button, corner) {
                        if !variants(tool).is_empty() && self.tool != tool {
                            self.set_tool(tool);
                        }
                    }
                    outcome = self.activate(button);
                }
            }
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
        // With the emoji open, it is the emoji that go up and down.
        if self.flyout == Some(Button::Emoji) {
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
        // With the emoji open, what is typed is what is looked for
        // among their names; Enter takes the first found, and the
        // arrows go up and down them.
        if self.flyout == Some(Button::Emoji) && !mods.ctrl {
            match key {
                Key::Char(letter) => (self.query, self.scroll) = (format!("{}{letter}", self.query), 0),
                Key::Backspace => (self.scroll, _) = (0, self.query.pop()),
                Key::Up => self.scrolled(true),
                Key::Down => self.scrolled(false),
                Key::Enter => self.choose(Cell::Emoji(0)),
                Key::Escape => self.flyout = None,
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
        if let (Some((index, _)), false, true) = (self.editing, mods.ctrl, self.all) {
            self.all = false;
            if matches!(key, Key::Char(_) | Key::Enter | Key::Backspace | Key::Delete) {
                self.shapes[index].text.clear();
                self.caret = 0;
                if matches!(key, Key::Backspace | Key::Delete) {
                    return Outcome::Continue;
                }
            }
        }
        if let (Some((index, _)), false) = (self.editing, mods.ctrl) {
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
            (Key::Char('c'), true) => return self.finish(Finish { copy: true, save: false }),
            (Key::Char('s'), true) => return self.finish(Finish { copy: false, save: true }),
            (Key::Char('z'), true) if mods.shift => self.redo(),
            (Key::Char('z'), true) => self.undo(),
            (Key::Char('y'), true) => self.redo(),
            (Key::Char('a'), true) => self.select(self.screen()),
            // A copy of each shape picked out, a little down and to the
            // right of it, and picked out in its place.
            // Heavy and slanted, for the text picked out.
            (Key::Char('b'), true) => self.toggle_style(shapes::BOLD),
            (Key::Char('i'), true) => self.toggle_style(shapes::ITALIC),
            (Key::Char('d'), true) => {
                let chosen = self.chosen();
                if !chosen.is_empty() {
                    self.checkpoint();
                    let by = ui_px(self.monitor().h) as f32;
                    let mut counters = self.shapes.iter().filter(|shape| shape.tool == Tool::Counter).count() as u32;
                    self.selected.clear();
                    for index in chosen {
                        let mut copy = self.shapes[index].clone();
                        copy.translate(by, by);
                        // A counter copied counts on.
                        if copy.tool == Tool::Counter {
                            counters += 1;
                            copy.number = counters;
                        }
                        self.shapes.push(copy);
                        self.selected.push(self.shapes.len() - 1);
                    }
                    self.tool = Tool::Select;
                }
            }
            (Key::Delete | Key::Backspace, _) => {
                let mut going = self.chosen();
                if !going.is_empty() {
                    self.checkpoint();
                    // From the last, so that the places of the rest hold.
                    going.sort_unstable();
                    going.iter().rev().for_each(|&index| drop(self.shapes.remove(index)));
                    self.selected.clear();
                    // The counters left count from one again, in order.
                    for (number, shape) in self.shapes.iter_mut().filter(|shape| shape.tool == Tool::Counter).enumerate() {
                        shape.number = number as u32 + 1;
                    }
                }
            }
            (Key::Left, ctrl) => self.nudge(-1, 0, ctrl),
            (Key::Right, ctrl) => self.nudge(1, 0, ctrl),
            (Key::Up, ctrl) => self.nudge(0, -1, ctrl),
            (Key::Down, ctrl) => self.nudge(0, 1, ctrl),
            (Key::Char(letter), false) if self.selection.is_some() => match letter {
                'v' => self.set_tool(Tool::Select),
                'a' => self.set_tool(Tool::Arrow),
                'l' => self.set_tool(Tool::Line),
                'r' => self.set_tool(Tool::Rect),
                'o' | 'e' => self.set_tool(Tool::Ellipse),
                'p' => self.set_tool(Tool::Pen),
                'h' => self.set_tool(Tool::Marker),
                'm' => drop(self.activate(Button::Emoji)),
                't' => self.set_tool(Tool::Text),
                'n' => self.set_tool(Tool::Counter),
                'b' => self.set_tool(Tool::Blur),
                's' => self.set_tool(Tool::Spotlight),
                'c' => drop(self.activate(Button::Color)),
                'f' => drop(self.activate(Button::Fill)),
                'g' => drop(self.activate(Button::Face)),
                'z' => self.set_tool(Tool::Zoom),
                'j' => drop(self.activate(Button::Badges)),
                'w' => drop(self.activate(Button::Frame)),
                // Dashes, for the lines and arrows picked out and for
                // those drawn after.
                'd' => {
                    self.dashed = !self.dashed;
                    let lines: Vec<usize> = self.chosen().into_iter().filter(|&index| matches!(self.shapes[index].tool, Tool::Line | Tool::Arrow)).collect();
                    if !lines.is_empty() {
                        self.checkpoint();
                    }
                    for index in lines {
                        self.shapes[index].dashed = self.dashed;
                        self.shapes[index].touch();
                    }
                }
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

    /// The arrow keys: the shape picked out goes a pixel that way, or ten
    /// with Shift; otherwise the selection does, or with Ctrl grows.
    fn nudge(&mut self, dx: i32, dy: i32, grow: bool) {
        let step = if self.mods.shift { 10 } else { 1 };
        let (dx, dy) = (dx * step, dy * step);
        let chosen = self.chosen();
        if !chosen.is_empty() {
            self.checkpoint();
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
            // A tool with more than one way of drawing, pressed when it
            // is already the one in hand, opens the list of them.
            Button::Tool(tool) if self.tool == tool && !variants(tool).is_empty() => {
                self.commit_text();
                self.popup = None;
                self.flyout = if self.flyout == Some(button) { None } else { Some(button) };
            }
            Button::Tool(tool) => self.set_tool(tool),
            Button::Color => {
                self.popup = match self.popup {
                    Some(_) => None,
                    None => Some(Anchor::Bar),
                }
            }
            Button::Size(size) => self.set_size(size),
            Button::Fill => {
                // A zoom picked out is made square, or round again: that
                // is what there is to fill or not about a zoom, and it
                // is nothing to do with how the next rectangle is drawn.
                let zooms: Vec<usize> = self.chosen().into_iter().filter(|&index| self.shapes[index].tool == Tool::Zoom).collect();
                if !zooms.is_empty() {
                    self.checkpoint();
                    for index in zooms {
                        self.shapes[index].filled = !self.shapes[index].filled;
                    }
                    return Outcome::Continue;
                }
                self.filled = !self.filled;
                let chosen: Vec<usize> = self.chosen().into_iter().filter(|&index| matches!(self.shapes[index].tool, Tool::Rect | Tool::Ellipse)).collect();
                if !chosen.is_empty() {
                    self.checkpoint();
                    for index in chosen {
                        self.shapes[index].filled = self.filled;
                        self.shapes[index].touch();
                    }
                }
            }
            // The next face, for the text being typed or picked out, and
            // for what is typed after.
            Button::Face => {
                self.face = (self.face + 1) % FACES.len();
                let texts: Vec<usize> = match self.editing {
                    Some((index, _)) => vec![index],
                    None => self.chosen().into_iter().filter(|&index| matches!(self.shapes[index].tool, Tool::Text | Tool::Badge)).collect(),
                };
                if self.editing.is_none() && !texts.is_empty() {
                    self.checkpoint();
                }
                texts.iter().for_each(|&index| self.shapes[index].face = self.face);
            }
            Button::Badges | Button::Frame | Button::Places | Button::Emoji => {
                (self.query, self.scroll) = (String::new(), 0);
                self.commit_text();
                self.popup = None;
                self.flyout = if self.flyout == Some(button) { None } else { Some(button) };
            }
            Button::Undo => self.undo(),
            Button::Redo => self.redo(),
            Button::Cancel => return Outcome::Cancel,
            Button::Save => return self.finish(Finish { copy: false, save: true }),
            Button::Copy => return self.finish(Finish { copy: true, save: false }),
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
    /// typed after is set as it then is.
    fn toggle_style(&mut self, style: u8) {
        let Some(index) = self.styled() else { return };
        if self.editing.is_none() {
            self.checkpoint();
        }
        self.shapes[index].style ^= style;
        self.style = self.shapes[index].style;
        self.dirty = true;
    }

    fn set_color(&mut self, color: u32) {
        self.color = color;
        let chosen = self.chosen();
        if let Some((index, _)) = self.editing {
            self.shapes[index].color = color;
        } else if !chosen.is_empty() {
            self.checkpoint();
            chosen.iter().for_each(|&index| self.shapes[index].color = color);
        }
        self.dirty = true;
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
    fn checkpoint(&mut self) {
        self.undo.push(self.shapes.clone());
        self.redo.clear();
    }

    fn undo(&mut self) {
        self.commit_text();
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.shapes, before));
            self.selected.clear();
        }
    }

    fn redo(&mut self) {
        self.commit_text();
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
            Cell::Shadow => self.frame.shadow = !self.frame.shadow,
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
            Cell::Word(word) => self.word = word,
            // The way the tool whose list this is draws from now on;
            // and that tool is the one in hand.
            Cell::Variant(way) => {
                if let Some(Button::Tool(tool)) = self.flyout {
                    match tool {
                        Tool::Arrow => (self.heads, self.dashed) = ([way == 1, true], way == 2),
                        Tool::Line => self.dashed = way == 1,
                        Tool::Rect | Tool::Ellipse => self.filled = way == 1,
                        Tool::Zoom => self.square = way == 1,
                        _ => {}
                    }
                    self.set_tool(tool);
                    self.flyout = None;
                }
            }
            Cell::Badge(_) | Cell::Emoji(_) => {}
        }
        if !matches!(cell, Cell::Badge(_) | Cell::Emoji(_)) {
            return;
        }
        self.checkpoint();
        let (stroke, font_px) = (self.stroke(), self.font_px());
        let mut shape = match cell {
            // The word in hand, in the look and the hue clicked on.
            Cell::Badge(index) => {
                let (soft, solid) = BADGE_HUES[index % BADGE_HUES.len()];
                let mut shape = Shape::new(Tool::Badge, solid, stroke, (0.0, 0.0));
                (shape.fill, shape.text, shape.font_px, shape.face) = (soft, BADGE_WORDS[self.word].to_owned(), font_px, self.face);
                shape.number = (index / BADGE_HUES.len()) as u32;
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
        self.shapes.push(shape);
        self.made(self.shapes.len() - 1);
        self.flyout = None;
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

    /// The emoji whose names have in them every word typed, by their
    /// places in the list of them all; all of them, with nothing typed.
    fn emoji_found(&self) -> Vec<usize> {
        let typed = self.query.to_lowercase();
        let words: Vec<&str> = typed.split_whitespace().collect();
        emoji().iter().enumerate().filter(|(_, (_, name))| words.iter().all(|word| name.contains(word))).map(|(index, _)| index).collect()
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
        let (area, mut pixels, rounded) = self.result()?;
        if self.frame.is_plain() {
            return Some((area, area.w, area.h, pixels, rounded));
        }
        if !rounded {
            pixels.iter_mut().for_each(|pixel| *pixel |= 0xFF00_0000);
        }
        let (wide, high, set) = frame::set(self.frame, &pixels, area.w, area.h, self.corner_radius(), self.scale(), self.photo.as_ref());
        Some((area, wide, high, set, self.frame.is_clear() || (self.frame.backdrop == Backdrop::Photo && self.photo.is_none())))
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
            let off = (left + self.text.width_in(shape.face, &format!("{}{}", shape.lead(), &line[..at_letter]), shape.font_px) - at.0).abs();
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
        let placing = match self.drag {
            Drag::None => self.selection.is_none(),
            Drag::Select { .. } | Drag::Resize { .. } => true,
            _ => false,
        };
        !over_interface && (self.pipette || (self.settings.magnifier && placing))
    }

    /// Paints the screen as it now stands into a buffer of its size.
    pub fn draw(&mut self, out: &mut [u32]) {
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
            let made_from = format!("{:?} {} {} {radius} {}", self.frame, area.w, area.h, self.photos);
            if self.surround.as_ref().is_none_or(|kept| kept.0 != made_from) {
                // Made as the picture's frame is, round a picture with
                // nothing in it.
                let nothing = vec![0u32; (area.w * area.h) as usize];
                let (wide, _, pixels) = frame::set(self.frame, &nothing, area.w, area.h, radius, scale, self.photo.as_ref());
                self.surround = Some((made_from, wide, pixels));
            }
            if let Some((_, wide, pixels)) = &self.surround {
                let (left, top) = (area.x - room.0, area.y - room.1);
                let part = Rect::new(left, top, *wide, pixels.len() as i32 / wide.max(&1)).intersect(Rect::new(0, 0, width, height));
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
        }
        let mut canvas = Canvas::new(out, width, height);

        if let Some(selection) = self.selection {
            shapes::paint(&self.shapes, &mut canvas, &mut self.text, selection);
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
                let along = self.text.width_in(shape.face, &format!("{}{last}", shape.lead()), shape.font_px);
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
            if self.tool == Tool::Select {
                for index in self.chosen().into_iter().filter(|&index| alone != Some(index)) {
                    let frame = self.frame_of(index);
                    canvas.round_outline(frame.x, frame.y, frame.w, frame.h, 0, thick, accent);
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
            // How round, in pixels, while a corner is being rounded:
            // of a rectangle, or of the picture.
            let rounding = match self.drag {
                Drag::Round { index, .. } => self.shapes.get(index).map(|shape| shape.radius.round() as i32),
                Drag::Curve { .. } => Some(self.corner_radius()),
                Drag::FrameCorner { .. } => Some((self.frame.radius as f32 * self.scale()).round() as i32),
                _ => None,
            };
            if let Some(radius) = rounding {
                let says = format!("{radius} px");
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
                let frame = self.frame_of(index);
                for (across, down) in GRIPS.into_iter().filter(|_| boxed(self.shapes[index].tool)) {
                    let (x, y) = grip_at(frame, across, down);
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
            // And the frame, one for its own.
            if let Some((x, y)) = self.frame_dot().filter(|_| !reshaping) {
                canvas.disc(x, y, grip + thick, accent, 256);
                canvas.disc(x, y, grip, 0xFFFFFF, 256);
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
            self.draw_bar(&mut canvas, &layout, bar);
        }
        // The bar over text: each way it can be set, by a letter set
        // that way, on the accent when the text is.
        if let (Some(place), Some(index)) = (layout.styles_bar, self.styled()) {
            let pad = px as i32 / 2;
            panel(&mut canvas, place, pad);
            let set = self.shapes[index].style;
            for (&(rect, style), (_, letter)) in layout.styles.iter().zip(STYLES) {
                let inner = rect.inflate(-(px as i32 / 8).max(1));
                let ink = if set & style != 0 {
                    canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, accent, 256);
                    on(accent)
                } else {
                    if self.hover == Target::Style(style) {
                        canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, 30);
                    }
                    TEXT_COLOR
                };
                let (cx, cy) = rect.center();
                if style == shapes::HIGHLIGHT {
                    canvas.round_fill(cx - px as i32 * 2 / 3, cy - px as i32 / 2, px as i32 * 4 / 3, px as i32, 2, 0xFFE629, 150);
                }
                self.text.look = crate::text::Look { heavy: i32::from(style == shapes::BOLD), slant: if style == shapes::ITALIC { 0.2 } else { 0.0 }, opacity: 256 };
                self.text.draw_centered(&mut canvas, cx, cy, px * 5 / 4, ink, letter);
                self.text.look = crate::text::Look::default();
            }
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
                    // As it will look on the picture, but small.
                    // A word it can say; the one in hand is on the accent.
                    Cell::Word(index) => {
                        let ink = if index == self.word {
                            canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, accent, 256);
                            on(accent)
                        } else {
                            TEXT_COLOR
                        };
                        self.text.draw_centered(&mut canvas, cx, cy, px, ink, BADGE_WORDS[index]);
                    }
                    // A look of it in a hue, with two letters for words.
                    Cell::Badge(index) => {
                        let (soft, solid) = BADGE_HUES[index % BADGE_HUES.len()];
                        let high = Text::line_height(px) + px as i32 / 4;
                        let pill = Rect::new(inner.x, cy - high / 2, inner.w, high);
                        shapes::draw_badge(&mut canvas, &mut self.text, pill, index / BADGE_HUES.len(), solid, soft, 0, px, "Ab");
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
                    // A gradient, as a small square of itself.
                    Cell::Backdrop(which) => {
                        let (_, from, to) = GRADIENTS[which - 1];
                        for row in 0..inner.h {
                            canvas.fill(inner.x, inner.y + row, inner.w, 1, canvas::mix(from, to, (row * 256 / inner.h.max(1)) as u32));
                        }
                        canvas.round_outline(inner.x - 1, inner.y - 1, inner.w + 2, inner.h + 2, pad / 2, 2, MENU_COLOR);
                    }
                    Cell::Dice => self.text.draw_centered(&mut canvas, cx, cy, px * 5 / 4, TEXT_COLOR, icons::DICE),
                    Cell::Own => self.text.draw_centered(&mut canvas, cx, cy, px * 5 / 4, TEXT_COLOR, icons::FRAME),
                    // A square with its shadow under it.
                    Cell::Shadow => {
                        let square = inner.inflate(-(px as i32 / 3));
                        canvas.round_fill(square.x + 2, square.y + 4, square.w, square.h, 3, 0x000000, 150);
                        canvas.round_fill(square.x, square.y, square.w, square.h, 3, TEXT_COLOR, 256);
                    }
                    Cell::Place(place) => {
                        let line = cy - Text::line_height(px) / 2;
                        if place == self.place {
                            self.text.draw_centered(&mut canvas, rect.x + rect.h / 2, cy, px, TEXT_COLOR, icons::CHECK);
                        }
                        let name = self.settings.places[place].clone();
                        self.text.draw(&mut canvas, rect.x + rect.h, line, px, TEXT_COLOR, &name);
                    }
                    // A way a tool draws, with a tick by the one in hand.
                    Cell::Variant(way) => {
                        let Some(Button::Tool(tool)) = self.flyout else { continue };
                        let now = match tool {
                            Tool::Arrow => if self.dashed { 2 } else { usize::from(self.heads == [true, true]) },
                            Tool::Line => usize::from(self.dashed),
                            Tool::Zoom => usize::from(self.square),
                            _ => usize::from(self.filled),
                        };
                        if way == now {
                            self.text.draw_centered(&mut canvas, rect.x + rect.h / 2, cy, px, TEXT_COLOR, icons::CHECK);
                        }
                        self.text.draw(&mut canvas, rect.x + rect.h, cy - Text::line_height(px) / 2, px, TEXT_COLOR, variants(tool)[way]);
                    }
                }
                // What is chosen now has a line of the accent round it.
                let on = match cell {
                    Cell::Backdrop(0) => self.frame.backdrop == Backdrop::None,
                    Cell::Backdrop(which) => self.frame.backdrop == Backdrop::Gradient(which - 1),
                    Cell::Dice | Cell::Own => false,
                    Cell::Shadow => self.frame.shadow,
                    _ => false,
                };
                if on {
                    canvas.round_outline(rect.x + 1, rect.y + 1, rect.w - 2, rect.h - 2, pad / 2, 2, accent);
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
                    Target::Cell(Cell::Emoji(index)) => found.get(index).map_or(String::new(), |&which| emoji()[which].1.to_owned()),
                    _ if found.is_empty() => "None by that name".to_owned(),
                    _ => format!("{} emoji; the wheel goes through them", found.len()),
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
        if self.loupe_shows() {
            self.draw_loupe(&mut canvas, px);
        }
    }

    fn draw_bar(&mut self, canvas: &mut Canvas, layout: &Layout, bar: Rect) {
        let (px, pad, side) = (layout.px, layout.px as i32 / 2, layout.side);
        let accent = self.settings.accent;
        panel(canvas, bar, pad);
        for &x in &layout.dividers {
            canvas.fill(x, bar.y + pad, 1, bar.h - 2 * pad, MENU_EDGE_COLOR);
        }
        let pressed = match self.drag {
            Drag::Button(button) => Some(button),
            _ => None,
        };
        for &(rect, button) in &layout.buttons {
            let hovered = self.hover == Target::Button(button) && (pressed.is_none() || pressed == Some(button));
            let chosen = match button {
                Button::Tool(tool) => tool == self.tool && !self.pipette,
                Button::Size(size) => size == self.size,
                Button::Fill => self.filled,
                Button::Color => self.popup.is_some() || self.pipette,
                _ => false,
            };
            let able = match button {
                Button::Undo => !self.undo.is_empty(),
                Button::Redo => !self.redo.is_empty(),
                _ => true,
            };
            let inner = rect.inflate(-(px as i32 / 8).max(1));
            if chosen && button != Button::Color {
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, accent, 256);
            } else if hovered && able {
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, if pressed.is_some() { 56 } else { 30 });
            }
            let ink = if chosen && button != Button::Color {
                on(accent)
            } else if !able {
                MENU_EDGE_COLOR
            } else {
                TEXT_COLOR
            };
            let (cx, cy) = rect.center();
            match button {
                // The color drawn in, ringed so that a dark one shows.
                Button::Color => {
                    let radius = side * 3 / 10;
                    canvas.disc(cx, cy, radius + (px as i32 / 10).max(1), if chosen { TEXT_COLOR } else { MENU_EDGE_COLOR }, 256);
                    canvas.disc(cx, cy, radius, self.color, 256);
                }
                // Two letters in the face text is typed in.
                Button::Face => {
                    let across = self.text.width_in(self.face, "Aa", px);
                    self.text.draw_in(self.face, canvas, cx - across / 2, cy - Text::line_height(px) / 2, px, ink, "Aa");
                }
                // A dot as wide as the line it stands for, near enough.
                Button::Size(size) => {
                    let radius = ((STROKES[size] * px as f32 / 13.0 * 0.62).round() as i32 + 1).min(side * 3 / 10);
                    canvas.disc(cx, cy, radius, ink, 256);
                }
                _ => {
                    self.text.draw_centered(canvas, cx, cy, px * 5 / 4, ink, button.icon());
                    // A small mark in the corner of a tool that has
                    // more than one way of drawing: press it, or the
                    // tool again, for the list of them.
                    if let Button::Tool(tool) = button {
                        if !variants(tool).is_empty() {
                            let mark = (px as i32 / 4).max(3);
                            for row in 0..mark {
                                canvas.fill(rect.right() - mark - 3 + (mark - 1 - row), rect.bottom() - mark - 3 + row, row + 1, 1, ink);
                            }
                        }
                    }
                }
            }
        }

        // What the button under the pointer is, and its key.
        if let (Target::Button(button), None, Drag::None) = (self.hover, layout.popup, self.drag) {
            let Some((rect, _)) = layout.buttons.iter().find(|(_, other)| *other == button) else { return };
            let (name, key) = button.says();
            // The face says which it is, where the rest say what they are.
            let name = if button == Button::Face { FACES[self.face] } else { name };
            let line = Text::line_height(px);
            let (name_width, key_width) = (self.text.width(name, px), self.text.width(key, px));
            let width = name_width + key_width + 3 * pad;
            let monitor = self.monitor();
            let under = bar.bottom() + pad / 2;
            let y = if under + line + pad <= monitor.bottom() { under } else { bar.y - pad / 2 - line - pad / 2 };
            let tip = Rect::new(rect.x + rect.w / 2 - width / 2, y, width, line + pad / 2).kept_within(monitor);
            canvas.round_fill(tip.x, tip.y, tip.w, tip.h, pad / 2, 0x000000, 210);
            self.text.draw(canvas, tip.x + pad, tip.y + pad / 4, px, TEXT_COLOR, name);
            self.text.draw(canvas, tip.x + 2 * pad + name_width, tip.y + pad / 4, px, DIM_TEXT_COLOR, key);
        }
    }

    fn draw_popup(&mut self, canvas: &mut Canvas, layout: &Layout, popup: Rect) {
        let (px, pad, side) = (layout.px, layout.px as i32 / 2, layout.side);
        panel(canvas, popup, pad);
        for &(rect, color) in &layout.swatches {
            let (cx, cy) = rect.center();
            let radius = side * 3 / 10;
            if self.hover == Target::Swatch(color) {
                let inner = rect.inflate(-(px as i32 / 8).max(1));
                canvas.round_fill(inner.x, inner.y, inner.w, inner.h, pad / 2, 0xFFFFFF, 30);
            }
            let ring = if color == self.color { TEXT_COLOR } else { MENU_EDGE_COLOR };
            canvas.disc(cx, cy, radius + (px as i32 / 10).max(1) + i32::from(color == self.color), ring, 256);
            canvas.disc(cx, cy, radius, color, 256);
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
            canvas.disc(cx, cy, radius, self.color, opacity);
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

    /// The picture that is taken: what is selected, with what was drawn
    /// on it, and nothing of the interface.
    ///
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
    /// the picture's are, whether shapes are filled, and the face.
    pub fn in_hand(&self) -> Settings {
        // A photograph is not kept: it was one nobody chose, or is the
        // user's own to choose again.
        let frame = Frame { backdrop: if self.frame.backdrop == Backdrop::Photo { Backdrop::None } else { self.frame.backdrop }, ..self.frame };
        Settings { color: self.color, size: self.size, corners: self.corners, filled: self.filled, face: self.face, frame, place: self.place, ..self.settings.clone() }
    }

    /// The last of the three says whether its corners are rounded: then
    /// the top byte of each pixel is how opaque it is, which outside the
    /// curve of a corner is not at all.
    pub fn result(&mut self) -> Option<(Rect, Vec<u32>, bool)> {
        self.commit_text();
        let selection = self.selection?;
        let mut pixels = if self.shapes.is_empty() {
            canvas::crop(&self.shot.pixels, self.shot.width, selection)
        } else {
            // Drawn on a copy of the whole screen, where the shapes are
            // already in the right place, and then cut out of it.
            let mut whole = self.shot.pixels.to_vec();
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

    const PLAIN: Mods = Mods { shift: false, ctrl: false };

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

    /// Where a button of the bar is.
    fn button(editor: &mut Editor, button: Button) -> (i32, i32) {
        editor.layout().buttons.iter().find(|(_, other)| *other == button).map(|(rect, _)| rect.center()).expect("the bar is up")
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
        editor.key(Key::Down, Mods { shift: true, ctrl: false });
        assert_eq!(editor.selection(), Some(Rect::new(231, 220, 370, 290)));
        editor.key(Key::Left, Mods { shift: false, ctrl: true });
        assert_eq!(editor.selection(), Some(Rect::new(231, 220, 369, 290)));
        // It cannot be moved off the screen.
        drag(&mut editor, (400, 300), (-500, -500));
        assert_eq!(editor.selection(), Some(Rect::new(0, 0, 369, 290)));
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

        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true });
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true });
        assert!(editor.shapes.is_empty());
        editor.key(Key::Char('Z'), Mods { shift: true, ctrl: true });
        assert_eq!(editor.shapes.len(), 1);
        assert_eq!(editor.key(Key::Enter, PLAIN), Outcome::Finish(Finish { copy: true, save: false }));
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
        assert_eq!(editor.key(Key::Enter, Mods { shift: false, ctrl: true }), Outcome::Continue);
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
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true });
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true });
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true });
        editor.key(Key::Char('v'), PLAIN);
        editor.pointer_moved(730, 620, PLAIN);
        assert_eq!(editor.hovered, Some(1));
        editor.pointer_moved(150, 650, PLAIN);
        assert_eq!(editor.hovered, None);

        // A click picks one out; with Shift or Ctrl it adds another, or
        // takes one away; and several picked out are moved, recolored
        // and deleted together.
        let shift = Mods { shift: true, ctrl: false };
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
        let ctrl = Mods { shift: false, ctrl: true };
        editor.pointer_moved(730, 620, PLAIN);
        editor.pressed(65_000);
        editor.pointer_moved(735, 610, ctrl);
        editor.pointer_moved(740, 600, ctrl);
        editor.released();
        let after = (editor.shapes[0].points[0], editor.shapes[1].points[0]);
        assert_eq!((after.0 .0 - before.0 .0, after.0 .1 - before.0 .1, after.1 .0 - before.1 .0, after.1 .1 - before.1 .1), (10.0, -20.0, 10.0, -20.0));
        editor.set_color(PALETTE[10].0);
        assert_eq!((editor.shapes[0].color, editor.shapes[1].color), (PALETTE[10].0, PALETTE[10].0));
        editor.pointer_moved(740, 600, Mods { shift: false, ctrl: true });
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
        let ctrl = Mods { shift: false, ctrl: true };
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        editor.key(Key::Char('a'), PLAIN);
        drag(&mut editor, (200, 200), (400, 300));
        // An arrow picked out has its two ends, and no box to be resized
        // by: its head is taken elsewhere, and its tail stays.
        assert_eq!(editor.ends(0), [(0, (201, 201)), (1, (401, 301))]);
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
        editor.key(Key::Char('z'), Mods { shift: false, ctrl: true });
        // Back to one head, at its last end, as arrows have to begin with.
        click(&mut editor, (200, 200), 45_000);
        click(&mut editor, (200, 200), 50_000);
        assert_eq!(editor.shapes[0].heads, [false, true]);

        // Ctrl+D is a copy of it, a little down and to the right, and
        // the copy is what is picked out.
        editor.key(Key::Char('d'), ctrl);
        assert_eq!((editor.shapes.len(), editor.selected.clone()), (2, vec![1]));
        assert_eq!(editor.shapes[1].points[0], (213.5, 213.5));

        // A box dragged out with Shift held picks out what it touches.
        editor.key(Key::Escape, PLAIN);
        click(&mut editor, (800, 650), 9000);
        assert!(editor.selected.is_empty());
        editor.pointer_moved(150, 150, Mods { shift: true, ctrl: false });
        editor.pressed(20_000);
        editor.pointer_moved(600, 400, Mods { shift: true, ctrl: false });
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
    fn text_is_typed_into_anywhere_in_it_and_pasted_into() {
        let ctrl = Mods { shift: false, ctrl: true };
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
        editor.pointer_moved(696, 477, Mods { shift: false, ctrl: true });
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
        editor.key(Key::Char('m'), PLAIN);
        assert_eq!((editor.flyout, editor.emoji_found().len()), (Some(Button::Emoji), emoji().len()));
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
        editor.choose(Cell::Backdrop(7));
        // Slate, with room: the backdrop shows outside the selection,
        // where the darkened screen was, and the bar is clear of it.
        let room = editor.frame_room().0;
        assert_eq!((editor.frame.backdrop, editor.frame_room()), (Backdrop::Gradient(6), (PADS[2], PADS[2])));
        let mut out = vec![0u32; 1600 * 900];
        editor.draw(&mut out);
        let near = |pixel: u32, color: u32| (0..3).all(|channel| ((pixel >> (channel * 8) & 0xFF) as i32 - (color >> (channel * 8) & 0xFF) as i32).abs() < 40);
        assert!(near(out[(290 * 1600 + 390) as usize], GRADIENTS[6].1), "{:06x}", out[(290 * 1600 + 390) as usize]);
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
        let ctrl = Mods { shift: false, ctrl: true };
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
        let bold = editor.layout().styles[0].0.center();
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
    fn a_tool_with_more_ways_than_one_has_a_list_of_them_and_shapes_go_to_lines() {
        let mut editor = editor(1600, 900);
        drag(&mut editor, (100, 100), (899, 699));
        // The arrow, pressed again when it is in hand, opens its list;
        // the second of it is a head at each end, from then on.
        editor.key(Key::Char('a'), PLAIN);
        let arrow = button(&mut editor, Button::Tool(Tool::Arrow));
        click(&mut editor, arrow, 0);
        assert_eq!(editor.flyout, Some(Button::Tool(Tool::Arrow)));
        editor.choose(Cell::Variant(1));
        assert_eq!((editor.flyout, editor.tool, editor.heads), (None, Tool::Arrow, [true, true]));
        drag(&mut editor, (200, 200), (400, 200));
        assert_eq!(editor.shapes[0].heads, [true, true]);

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
        editor.pointer_moved(400 + by, 421, Mods { shift: false, ctrl: true });
        assert_eq!(editor.shapes[1].bounds(&mut editor.text).x, first.x + 4);
        editor.released();
        assert!(editor.guides.is_empty());
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
        let heavy = button(&mut editor, Button::Size(3));
        click(&mut editor, heavy, 1000);
        assert_eq!(editor.size, 3);
        // Pressed on one button and let go off it, nothing is done.
        let (save, copy) = (button(&mut editor, Button::Save), button(&mut editor, Button::Copy));
        editor.pointer_moved(save.0, save.1, PLAIN);
        editor.pressed(2000);
        editor.pointer_moved(save.0, save.1 - 300, PLAIN);
        assert_eq!(editor.released(), Outcome::Continue);
        assert_eq!(click(&mut editor, save, 3000), Outcome::Finish(Finish { copy: false, save: true }));
        assert_eq!(click(&mut editor, copy, 4000), Outcome::Finish(Finish { copy: true, save: false }));
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
        editor.key(Key::Enter, Mods { shift: false, ctrl: true });
        let arrow = button(&mut editor, Button::Tool(Tool::Arrow));
        editor.pointer_moved(arrow.0, arrow.1, PLAIN);
        write(&mut editor, "5-tooltip");
        editor.key(Key::Char('c'), PLAIN);
        let swatch = editor.layout().swatches[6].0.center();
        editor.pointer_moved(swatch.0, swatch.1, PLAIN);
        write(&mut editor, "6-colors");
        editor.key(Key::Char('i'), PLAIN);
        editor.pointer_moved(110 * unit, 36 * unit, PLAIN);
        write(&mut editor, "7-pipette");
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('v'), PLAIN);
        editor.pointer_moved(45 * unit, 28 * unit, PLAIN);
        editor.pressed(200_000);
        editor.released();
        write(&mut editor, "8-shape-picked");
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
            editor.key(Key::Enter, Mods { shift: false, ctrl: true });
            editor.key(Key::Char('g'), PLAIN);
        }
        editor.key(Key::Char('a'), PLAIN);
        editor.pointer_moved(100 * unit, 40 * unit, PLAIN);
        editor.pressed(20_000);
        editor.pointer_moved(70 * unit, 30 * unit, PLAIN);
        editor.released();
        let face = button(&mut editor, Button::Face);
        editor.pointer_moved(face.0, face.1, PLAIN);
        write(&mut editor, "11-spotlight-faces");

        // Badges, a sticker, a zoom, a dashed arrow, and the box they
        // are chosen from.
        let mut editor = self::editor(width, height);
        editor.pointer_moved(8 * unit, 6 * unit, PLAIN);
        editor.pressed(0);
        editor.pointer_moved(130 * unit, 80 * unit, PLAIN);
        editor.released();
        for (cell, to) in [(Cell::Badge(0), (20, 14)), (Cell::Badge(11), (40, 14)), (Cell::Badge(20), (20, 22)), (Cell::Badge(29), (40, 22)), (Cell::Badge(37), (20, 30)), (Cell::Badge(46), (40, 30)), (Cell::Emoji(6), (60, 12)), (Cell::Emoji(40), (70, 12))] {
            editor.word = (editor.word + 1) % BADGE_WORDS.len();
            editor.choose(cell);
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
        editor.key(Key::Char('m'), PLAIN);
        write(&mut editor, "12b-emoji");
        for letter in "heart".chars() {
            editor.key(Key::Char(letter), PLAIN);
        }
        write(&mut editor, "12c-emoji-search");
        editor.key(Key::Escape, PLAIN);
        editor.key(Key::Char('j'), PLAIN);

        // The picture set on a backdrop, with room and a shadow, as the
        // box it is chosen in shows it and as it comes out.
        editor.key(Key::Escape, PLAIN);
        editor.corners = 16.0;
        for cell in [Cell::Backdrop(1), Cell::Shadow] {
            editor.choose(cell);
        }
        editor.key(Key::Char('w'), PLAIN);
        write(&mut editor, "13-frame");
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
                styled.key(Key::Enter, Mods { shift: false, ctrl: true });
            }
        }
        write(&mut styled, "16-text-set");
        styled.key(Key::Enter, Mods { shift: false, ctrl: true });
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
    }
}
