//! Everything between the screen being frozen and the picture being
//! taken: the area dragged out, what is drawn on it, and the bar of
//! tools. It knows nothing of the display: it is told of the pointer and
//! the keys, and paints the screen into a buffer it is handed. That is
//! what lets it be tried, and timed, without a display at all.

use std::ops::Deref;

use crate::canvas::{self, Canvas, Rect};
use crate::shapes::{self, Shape, Tool};
use crate::text::{icons, Text, FACES};

// jog's colours, so that the two look like one family.
const TEXT_COLOR: u32 = 0xEEEEEE;
const DIM_TEXT_COLOR: u32 = 0x9A9A9A;
const MENU_COLOR: u32 = 0x1E1E1E;
const MENU_EDGE_COLOR: u32 = 0x555555;

/// The colours to draw in: Radix's, at the step meant for solid
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
    // the bright ones, and the ones that are no colour.
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
    /// The colour drawn in to begin with.
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
    /// What Enter, and a double click, do.
    pub enter: Finish,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { accent: 0x3E63DD, color: RED, size: 1, magnifier: false, windows: Windows::Outline, dim: 50, radius: 6.0, corners: 0.0, filled: false, face: 0, enter: Finish { copy: true, save: false } }
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
}

/// The bar of tools, in the groups it is divided into.
const GROUPS: [&[Button]; 4] = [
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
    ],
    &[Button::Color, Button::Size(0), Button::Size(1), Button::Size(2), Button::Size(3), Button::Fill, Button::Face],
    &[Button::Undo, Button::Redo],
    &[Button::Cancel, Button::Save, Button::Copy],
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
            Button::Color => ("Colour", "C"),
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
            Button::Fill => icons::FILLED,
            Button::Undo => icons::UNDO,
            Button::Redo => icons::REDO,
            Button::Cancel => icons::CLOSE,
            Button::Save => icons::SAVE,
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
    Pipette,
    /// A part of the bar, or of the box of colours, that is not a button.
    Panel,
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
    End { index: usize, point: usize },
    /// Dragging out a box, with Shift or Ctrl held, to pick out the
    /// shapes it touches.
    Marquee { from: (i32, i32) },
    Button(Button),
}

/// Where the box of colours hangs from.
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
    pipette: Rect,
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
    popup: Option<Anchor>,
    /// Picking a colour off the screen.
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

/// The colour for an icon on the accent: white, or near black on an
/// accent light enough that white would be lost on it.
fn on(color: u32) -> u32 {
    shapes::ink_on(color)
}

/// The handles on the box of a shape picked out: its corners, which
/// resize it both ways, and the middles of its sides, which do one.
/// Each as which way it pulls across and down.
const GRIPS: [(i32, i32); 8] = [(-1, -1), (1, -1), (-1, 1), (1, 1), (0, -1), (0, 1), (-1, 0), (1, 0)];

/// Whether a shape picked out has a box with handles to resize it by.
/// Not a line or an arrow, which have their two ends; nor a counter,
/// which is the size it is.
fn boxed(tool: Tool) -> bool {
    !matches!(tool, Tool::Arrow | Tool::Line | Tool::Counter)
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
        let windows = windows.into_iter().map(|window| window.intersect(screen)).filter(|window| wanted && window.w > 8 && window.h > 8).collect();
        Editor {
            dim,
            monitors: if monitors.is_empty() { vec![screen] } else { monitors },
            windows,
            color: settings.color,
            corners: settings.corners,
            filled: settings.filled,
            face: settings.face.min(FACES.len() - 1),
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
            popup: None,
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
        let left = selection.x + (selection.w - width) / 2;
        let below = selection.bottom() + pad;
        let above = selection.y - pad - height;
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

        // The size of the selection, over its top-left corner, or inside
        // it where there is no room or the bar is in the way.
        let words = format!("{} \u{d7} {}", selection.w, selection.h);
        let line = Text::line_height(px);
        let size = (self.text.width(&words, px) + 2 * pad, line + pad / 2);
        let outside = Rect::new(selection.x, selection.y - size.1 - pad, size.0, size.1);
        let clear = outside.y >= monitor.y && !layout.bar.is_some_and(|bar| bar.intersects(outside));
        let place = if clear { outside } else { Rect::new(selection.x + pad, selection.y + pad, size.0, size.1) };
        layout.label = Some((place.kept_within(monitor), words));

        // The box of colours: two rows of them, and under those the
        // pipette.
        if let (Some(anchor), false) = (self.popup, hidden) {
            let columns = PALETTE_COLUMNS;
            let rows = (PALETTE.len() as i32 + columns - 1) / columns;
            let words = self.text.width("Pick from the screen", px) + self.text.width("I", px) + side + 3 * pad;
            let width = (columns * side).max(words) + 2 * margin;
            let height = rows * side + side + 2 * margin + pad / 2;
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
            layout.pipette = Rect::new(popup.x + margin, popup.y + margin + rows * side + pad / 2, width - 2 * margin, side);
            layout.popup = Some(popup);
        }
        layout
    }

    fn target_at(&mut self, (x, y): (i32, i32)) -> Target {
        let layout = self.layout();
        if let Some(popup) = layout.popup {
            if let Some((_, color)) = layout.swatches.iter().find(|(rect, _)| rect.contains(x, y)) {
                return Target::Swatch(*color);
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
        if selection.contains(x, y) { Target::Inside } else { Target::Outside }
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
        if self.pipette && !matches!(self.hover, Target::Button(_) | Target::Panel | Target::Swatch(_) | Target::Pipette) {
            return Cursor::Crosshair;
        }
        let pull = match (self.drag, self.hover) {
            (Drag::Resize { pull, .. } | Drag::Scale { pull, .. }, _) => Some(pull),
            (Drag::None, Target::Handle(across, down) | Target::Grip(across, down)) => Some((across, down)),
            (Drag::Round { .. } | Drag::Curve { .. } | Drag::End { .. }, _) | (Drag::None, Target::Rounder(..) | Target::Curve(..) | Target::End(_)) => return Cursor::Arrow,
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
            (_, Target::Button(_) | Target::Swatch(_) | Target::Pipette | Target::Panel) => Cursor::Arrow,
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
            Drag::End { index, point } => {
                let (index, point) = (*index, *point);
                let to = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
                let shape = &mut self.shapes[index];
                // With Shift, at one of eight angles from the other end.
                let other = shape.points[1 - point.min(1)];
                shape.points[point] = if mods.shift { shapes::constrained(Tool::Line, other, to) } else { to };
                shape.touch();
                self.dirty = true;
            }
            Drag::Marquee { .. } => self.dirty = true,
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
                    // The pixel under the pointer is taken too.
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
                let left = origin.x + if pull.0 < 0 { dx } else { 0 };
                let right = origin.right() + if pull.0 > 0 { dx } else { 0 };
                let top = origin.y + if pull.1 < 0 { dy } else { 0 };
                let bottom = origin.bottom() + if pull.1 > 0 { dy } else { 0 };
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
                        tool => shape.points[1] = if mods.shift { shapes::constrained(tool, from, to) } else { to },
                    }
                    shape.touch();
                }
                self.dirty = true;
            }
            Drag::Shape { index, last, moved } => {
                let (index, by) = (*index, (at.0 - last.0, at.1 - last.1));
                let first = !*moved;
                (*last, *moved) = (at, true);
                if first {
                    self.checkpoint();
                }
                // All that are picked out go with the one that is held.
                let mut going = self.chosen();
                if !going.contains(&index) {
                    going = vec![index];
                }
                for index in going {
                    if let Some(shape) = self.shapes.get_mut(index) {
                        shape.translate(by.0 as f32, by.1 as f32);
                    }
                }
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
            Target::Pipette => (self.pipette, self.popup) = (true, None),
            Target::Panel => {}
            Target::Button(button) => self.drag = Drag::Button(button),
            // A click anywhere else puts the box of colours away, and
            // does nothing more.
            _ if self.popup.is_some() => self.popup = None,
            Target::End(point) => {
                if let Some(index) = self.picked() {
                    self.checkpoint();
                    self.drag = Drag::End { index, point };
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
                    match double && self.shapes[index].tool == Tool::Text {
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
                    self.shapes.push(shape);
                    self.caret = 0;
                    self.editing = Some((self.shapes.len() - 1, true));
                    self.selected.clear();
                }
            },
            tool => {
                self.checkpoint();
                let mut shape = Shape::new(tool, self.color, self.stroke(), point);
                shape.filled = self.filled;
                shape.radius = self.settings.radius * self.scale();
                shape.font_px = self.font_px();
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
                    outcome = self.activate(button);
                }
            }
            _ => {}
        }
        self.hover = self.target_at(self.pointer);
        outcome
    }

    /// The right button: the colours, where the pointer is; or with
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
        if self.selection.is_some() {
            let size = if up { (self.size + 1).min(STROKES.len() - 1) } else { self.size.saturating_sub(1) };
            self.set_size(size);
        }
    }

    // ---- The keys ----

    pub fn key(&mut self, key: Key, mods: Mods) -> Outcome {
        self.dirty = true;
        self.mods = mods;
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
                if self.popup.take().is_none() && !std::mem::take(&mut self.pipette) {
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
                'h' | 'm' => self.set_tool(Tool::Marker),
                't' => self.set_tool(Tool::Text),
                'n' => self.set_tool(Tool::Counter),
                'b' => self.set_tool(Tool::Blur),
                's' => self.set_tool(Tool::Spotlight),
                'c' => drop(self.activate(Button::Color)),
                'f' => drop(self.activate(Button::Fill)),
                'g' => drop(self.activate(Button::Face)),
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
            Button::Tool(tool) => self.set_tool(tool),
            Button::Color => {
                self.popup = match self.popup {
                    Some(_) => None,
                    None => Some(Anchor::Bar),
                }
            }
            Button::Size(size) => self.set_size(size),
            Button::Fill => {
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
                    None => self.chosen().into_iter().filter(|&index| self.shapes[index].tool == Tool::Text).collect(),
                };
                if self.editing.is_none() && !texts.is_empty() {
                    self.checkpoint();
                }
                texts.iter().for_each(|&index| self.shapes[index].face = self.face);
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
        let (left, top) = (shape.points[0].0.round() as i32, shape.points[0].1.round() as i32);
        let row = ((at.1 - top) / Text::line_height(shape.font_px).max(1)).max(0) as usize;
        let mut start = 0;
        let lines: Vec<&str> = shape.text.split('\n').collect();
        let row = row.min(lines.len() - 1);
        for line in &lines[..row] {
            start += line.len() + 1;
        }
        let line = lines[row];
        let mut nearest = (i32::MAX, 0);
        for at_letter in line.char_indices().map(|(at, _)| at).chain([line.len()]) {
            let off = (left + self.text.width_in(shape.face, &line[..at_letter], shape.font_px) - at.0).abs();
            if off < nearest.0 {
                nearest = (off, at_letter);
            }
        }
        start + nearest.1
    }

    /// Done typing. Text with nothing in it is not kept; and if it was
    /// only just begun, neither is the step back to before it. Text
    /// that is kept is picked out, as anything just made is.
    fn commit_text(&mut self) {
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
    /// is wanted at all, and always while a colour is being picked.
    fn loupe_shows(&self) -> bool {
        let over_interface = matches!(self.hover, Target::Button(_) | Target::Panel | Target::Swatch(_) | Target::Pipette);
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
                let x = shape.points[0].0.round() as i32 + self.text.width_in(shape.face, last, shape.font_px);
                let y = shape.points[0].1.round() as i32 + (rows - 1) * line;
                canvas.fill(x + 1, y + line / 8, (shape.font_px as i32 / 12).max(2), line - line / 4, shape.color);
            }
            canvas.reset_clip();
            // With rounded corners, what is outside the curve is no
            // part of the picture, and shows darkened like the rest.
            canvas.round_off(selection, round, &self.dim);
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
        if let Some(popup) = layout.popup {
            self.draw_popup(&mut canvas, &layout, popup);
        }
        if self.selection.is_none() && !matches!(self.drag, Drag::Select { begun: true, .. }) {
            self.draw_hint(&mut canvas, px);
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
                // The colour drawn in, ringed so that a dark one shows.
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
                _ => self.text.draw_centered(canvas, cx, cy, px * 5 / 4, ink, button.icon()),
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
    /// colour it is on.
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
    /// What is in hand now, as settings to begin with next time: the
    /// colour, the width of line, how round a rectangle's corners and
    /// the picture's are, whether shapes are filled, and the face.
    pub fn in_hand(&self) -> Settings {
        Settings { color: self.color, size: self.size, corners: self.corners, filled: self.filled, face: self.face, ..self.settings.clone() }
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

/// A lighter tint of a colour, for thin lines that the colour itself
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
    /// grey, and two windows.
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
    fn text_is_typed_where_clicked_and_a_shape_is_moved_recoloured_and_deleted() {
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
        // The colours: opened from the bar, and one of them clicked.
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
        // takes one away; and several picked out are moved, recoloured
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
        drag(&mut editor, (730, 620), (740, 600));
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
        write(&mut editor, "6-colours");
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
    }
}
