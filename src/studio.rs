//! The canvas, `hotshot --canvas`: a window to make a picture in, rather
//! than mark one up. Pictures are layers on an artboard, put there from
//! files, the clipboard or the screen, and moved, sized, turned, cropped,
//! rounded and shadowed; the shapes of the markup editor go over them;
//! and the artboard is set on a color or a backdrop, and saved as a
//! picture or as a file of its own that keeps all of it to change later.
//!
//! As with the editor, what is here knows nothing of the display: it is
//! told of the pointer and the keys, and paints the window into a buffer
//! it is handed. `run` is what has a window for it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use crate::canvas::{self, Canvas, Rect};
use crate::chrome;
use crate::editor::{Cursor, Key, Mods, PALETTE};
use crate::frame::{self, Shadow, BACKDROPS};
use crate::shapes::{self, Shape, Tool};
use crate::text::{icons, Text, FACES};

mod actions;
mod arrange;
mod board;
mod chromes;
mod corners;
mod crop_bar;
mod crop_snap;
mod edges;
mod fills;
mod turning;
mod drawn;
mod duplicate;
mod effects;
mod export;
mod fine;
mod library;
mod menu;
mod order;
pub use menu::parse_frame_size;
pub use export::export_file;
mod panel;
mod picker;
mod place;
/// The smallest the canvas and the projects are drawn.
pub const MIN_SIZE: (i32, i32) = place::MIN;
mod preview;
mod recent;
mod rulers;
mod snap;
mod stacks;
mod stack_rows;
mod view;
mod search;
mod sizes;
mod style;
mod wheel;
#[cfg(test)]
mod perf;


use menu::{MenuKind, MenuState};

// Figma's dark colors, which the editor's are near.
pub(crate) const TEXT_COLOR: u32 = 0xF0F0F0;
pub(crate) const DIM_TEXT_COLOR: u32 = 0xA8A8A8;
pub(crate) const PANEL_COLOR: u32 = 0x2C2C2C;
pub(crate) const EDGE_COLOR: u32 = 0x383838;
pub(crate) const FIELD_COLOR: u32 = 0x383838;
pub(crate) const HOVER_COLOR: u32 = 0x444444;
pub(crate) const VIEW_COLOR: u32 = 0x1E1E1E;
pub(crate) const ACCENT: u32 = 0x3E63DD;
/// What is picked out, in a list; and the accent as text on the dark.
pub(crate) const ROW_ON: u32 = 0x2E3D73;
const ACCENT_TEXT: u32 = 0x9DB1F5;
/// The lines things snap to, as Figma draws them.
const GUIDE: u32 = 0xF2555A;

/// How far the panels float in from the window's edges, and how wide
/// each is; and how much of the window's bottom the toolbar keeps.
const INSET: i32 = 12;
const LEFT_W: i32 = 252;
const RIGHT_W: i32 = 268;
const DOCK_ROOM: i32 = 72;
/// How far down the left panel begins under the switch between Design and
/// Board, at its top.
const SWITCH_ROOM: i32 = 40;

/// How far a notch of the wheel moves the page, in pixels of the window;
/// and how much larger a notch with Ctrl makes it, about the pointer:
/// 1.15 times, near what Figma's does for a notch of a mouse's wheel.
/// A trackpad's strokes come as parts of a notch, each zooming by as
/// large a part of that, so that many small ones add up to the same.
const WHEEL_STEP: f32 = 60.0;
/// How far out and in the page zooms: 2% to 25600%, Figma's furthest in,
/// where a pixel of a screenshot is 256 of the window's.
const MIN_ZOOM: f32 = 0.02;
const MAX_ZOOM: f32 = 256.0;
/// How many pixels of the window a picture's pixel is shown as before it
/// is drawn as a hard square, nearest its middle, to be looked at pixel
/// by pixel; and before a faint grid shows between the page's pixels.
const HARD_PIXELS: f32 = 8.0;
const PIXEL_GRID: f32 = 16.0;
const WHEEL_ZOOM: f32 = 1.15;

/// How much larger so many notches of the wheel down make the page:
/// smaller, for down, and larger for up.
fn wheel_zoom(down: f32) -> f32 {
    (-WHEEL_ZOOM.ln() * down).exp()
}

/// How long the page has to be still after it is zoomed or moved before
/// it is drawn sharp again, rather than shown from what was drawn before.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(130);

/// How far round what is seen of a frame it is drawn, in pixels of the
/// window, for a little panning to need nothing drawn: no more, as each
/// pixel of it is as long drawing as one seen.
const ROOM: i32 = 256;

/// Whether two zooms are the same, but for what notches of the wheel in
/// and as many out leave over: too little to move a pixel.
fn same_zoom(a: f32, b: f32) -> bool {
    (a / b - 1.0).abs() < 1e-5
}

/// Whether a part of a frame drawn at one zoom reaches over a part of it
/// as it is seen at another, to a pixel: each counted from the frame's
/// corner, at its own zoom.
fn reaches(part: Rect, zoom: f32, seen: Rect, now: f32) -> bool {
    let by = zoom / now;
    let near = |a: f32, b: i32| a >= b as f32 - 1.0;
    near(seen.x as f32 * by, part.x) && near(seen.y as f32 * by, part.y) && near(part.right() as f32, (seen.right() as f32 * by).floor() as i32) && near(part.bottom() as f32, (seen.bottom() as f32 * by).floor() as i32)
}

/// Which way a press of buttons 4 to 7, a notch of the wheel, turns it:
/// so many notches right and down.
fn wheel_notch(button: u8) -> (f32, f32) {
    match button {
        4 => (0.0, -1.0),
        5 => (0.0, 1.0),
        6 => (-1.0, 0.0),
        7 => (1.0, 0.0),
        _ => (0.0, 0.0),
    }
}

/// How much larger a picture's shadow is than Tailwind's of its name.
const SHADOW_SIZE: f32 = 2.0;

/// How a handle at an end of a line pulls: that end, and nothing else.
const END: i32 = 9;

/// The widths of line there are, in pixels of the picture; and the sizes
/// of text that go with them.
const STROKES: [f32; 4] = [2.0, 3.5, 6.0, 10.0];
const FONT_SIZES: [u32; 4] = [18, 26, 40, 64];

/// The sizes an artboard can be set to at once, by what each is for.

/// The tools, with their keys: those along the bottom, and after them
/// those the More tools button holds, with capture mode's keys where the
/// canvas has no other use for them.
const TOOLS: [(Tool, &str, &str); 13] = [
    (Tool::Select, "Move", "V"),
    (Tool::Rect, "Rectangle", "R"),
    (Tool::Ellipse, "Oval", "O"),
    (Tool::Line, "Line", "L"),
    (Tool::Arrow, "Arrow", "A"),
    (Tool::Pen, "Pen", "P"),
    (Tool::Marker, "Marker", "M"),
    (Tool::Text, "Text", "T"),
    (Tool::Counter, "Counter", "N"),
    (Tool::Blur, "Blur", "B"),
    (Tool::Spotlight, "Spotlight", "D"),
    (Tool::Zoom, "Zoom lens", "Z"),
    (Tool::Badge, "Library", "J"),
];

/// The tools kept behind the More tools button, at the end of the bar,
/// in the order its list has them.
const MORE_TOOLS: [Tool; 4] = [Tool::Blur, Tool::Spotlight, Tool::Zoom, Tool::Badge];
/// The group the More tools button opens, after the shapes' and the pens'.
const MORE: u8 = 2;

/// What a shape drawn with a tool is called: the tool's name, and for
/// those brought from capture mode that the canvas has no tool for,
/// theirs.
fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Blur => "Blur",
        Tool::Spotlight => "Spotlight",
        Tool::Zoom => "Zoom",
        Tool::Badge => "Label",
        tool => TOOLS.iter().find(|(known, ..)| *known == tool).map_or("Shape", |(_, name, _)| name),
    }
}

/// The icon a tool is shown by.
fn tool_icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => icons::SELECT,
        Tool::Rect => icons::RECT,
        Tool::Ellipse => icons::ELLIPSE,
        Tool::Line => icons::LINE,
        Tool::Arrow => icons::ARROW,
        Tool::Pen => icons::PEN,
        Tool::Marker => icons::MARKER,
        Tool::Text => icons::TEXT,
        Tool::Blur => icons::BLUR,
        Tool::Spotlight => icons::SPOTLIGHT,
        Tool::Zoom => icons::ZOOM,
        Tool::Badge => icons::BADGE,
        Tool::Counter => icons::COUNTER,
    }
}

/// What a file of the canvas begins with.
const MAGIC: &str = "HOTSHOT CANVAS 2";
/// What a file from before pages and frames begins with.
const OLD_MAGIC: &str = "HOTSHOT CANVAS 1";

// ---- What is made ----

/// A picture as it was brought in, and smaller copies of it, each half
/// the size of the last: what it is drawn from when it is drawn small,
/// without the shimmer of drawing a large picture from every other pixel.
pub struct Image {
    levels: Vec<(i32, i32, Vec<u32>)>,
    /// Its bytes as a PNG, which is what a file of the canvas keeps.
    png: Vec<u8>,
}

impl Image {
    /// From the pixels of a picture, each with how opaque it is in its
    /// top byte.
    pub fn new(width: i32, height: i32, pixels: Vec<u32>) -> Image {
        let png = crate::output::encode(width as u32, height as u32, &pixels, true).unwrap_or_default();
        Image::with(width, height, pixels, png)
    }

    fn with(width: i32, height: i32, pixels: Vec<u32>, png: Vec<u8>) -> Image {
        let mut levels = vec![(width, height, pixels)];
        while let Some(&(w, h, ref last)) = levels.last() {
            if w.min(h) < 96 {
                break;
            }
            let smaller = crate::pin::sized(last, w, h, (w / 2).max(1), (h / 2).max(1));
            levels.push(((w / 2).max(1), (h / 2).max(1), smaller));
        }
        Image { levels, png }
    }

    /// From a PNG, a JPEG, a WebP, an AVIF or a JPEG XL, what is
    /// see-through in it kept.
    pub fn open(bytes: &[u8]) -> Option<Image> {
        if let Some((width, height, pixels)) = crate::pin::read(bytes) {
            return Some(Image::with(width, height, pixels, bytes.to_vec()));
        }
        let picture = crate::convert::decode(bytes)?;
        Some(Image::new(picture.width, picture.height, picture.pixels))
    }

    pub fn width(&self) -> i32 {
        self.levels[0].0
    }

    pub fn height(&self) -> i32 {
        self.levels[0].1
    }

    /// The copy to draw from when each of its pixels covers `times` of
    /// the picture's: the smallest that is still no smaller than needed.
    fn level(&self, times: f32) -> &(i32, i32, Vec<u32>) {
        let mut chosen = &self.levels[0];
        for level in &self.levels[1..] {
            if level.0 as f32 >= self.width() as f32 / times.max(1e-3) {
                chosen = level;
            }
        }
        chosen
    }
}

/// A picture on the artboard: where its middle is, how large it is shown,
/// how far it is turned, what part of it is shown, and how.
#[derive(Clone)]
pub struct Picture {
    pub image: Arc<Image>,
    pub name: String,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
    pub angle: f32,
    /// The part of it that is shown, in its own pixels.
    pub crop: (f32, f32, f32, f32),
    pub radius: f32,
    /// The radius of each corner when they differ, as a box's are.
    pub corners: Option<[f32; 4]>,
    /// Which of the shadows there are under it: 0 for none, or one more
    /// than where it is in `frame::SHADOWS`.
    pub shadow: usize,
    pub opacity: u32,
    pub flip: (bool, bool),
    /// What it is called, for the ends of arrows to be stuck to it by.
    pub id: u32,
    /// The shadow it casts when `shadow` is `CUSTOM_SHADOW`: set number by
    /// number, in pixels of the artboard.
    pub drop: frame::Layer,
    /// How much lighter, more contrasted and more colorful it is shown,
    /// each from -1 to 1, nothing at 0.
    pub adjust: [f32; 3],
    /// A window drawn round it, its bar over the top of it, once one has
    /// been chosen; and what the window is called, in its bar.
    pub chrome: Option<Chrome>,
    pub title: String,
    /// How far in from each side of it, in its own pixels, the frame the
    /// window manager drew round the window it is of went: its own title
    /// bar and borders, to come off under a window of hotshot's. None
    /// until known, from the capture or looked for in it; nothing at all
    /// for a picture that is no window.
    pub insets: Option<[i32; 4]>,
    /// How many of its pixels a pixel of a screen 1080 high was, for its
    /// window's bar to be as large on it as its own was.
    pub screen_scale: f32,
}

/// A window drawn round a picture: which, light or dark, whether the
/// picture's own title bar comes off under it, whose shadow it casts, and
/// a browser's address.
#[derive(Clone, PartialEq, Debug)]
pub struct Chrome {
    pub style: chrome::Style,
    pub theme: chrome::Theme,
    /// Whether the window's own bar and borders come off and this one goes
    /// on: "Keep own" leaves its own on and draws nothing.
    pub swap: bool,
    /// The shadow: the system's for such a window, none, or the picture's
    /// own as its effects set it.
    pub shadow: u8,
    pub address: String,
}

/// The shadows a window casts: the system's, none, or the picture's own.
pub const SYSTEM_SHADOW: u8 = 0;
pub const NO_SHADOW: u8 = 1;
pub const OWN_SHADOW: u8 = 2;

impl Chrome {
    pub fn new(style: chrome::Style) -> Chrome {
        Chrome { style, theme: chrome::Theme::Auto, swap: true, shadow: SYSTEM_SHADOW, address: String::new() }
    }
}

/// A picture's `shadow` when its shadow is set number by number.
const CUSTOM_SHADOW: usize = frame::SHADOWS.len() + 1;

/// The shadow a picture is given to begin with, when its numbers are first
/// set: Tailwind's 2xl, as an artboard is mostly seen at half its size.
const DROP: frame::Layer = frame::Layer { x: 0, y: 50, blur: 100, spread: -24, color: 0x4000_0000 };

impl Picture {
    pub fn new(image: Arc<Image>, name: String, at: (f32, f32)) -> Picture {
        let (w, h) = (image.width() as f32, image.height() as f32);
        // A capture of a window says what it knows of it in its file.
        let notes = crate::output::note_of(&image.png).map(|note| chrome::Notes::read(&note)).unwrap_or_default();
        let screen_scale = if notes.scale > 0.0 { notes.scale } else { 1.0 };
        Picture { image, name, cx: at.0, cy: at.1, w, h, angle: 0.0, crop: (0.0, 0.0, w, h), radius: 0.0, corners: None, shadow: 0, opacity: 256, flip: (false, false), id: shapes::fresh_id(), drop: DROP, adjust: [0.0; 3], chrome: None, title: notes.title, insets: notes.insets, screen_scale }
    }

    /// The radius of each of its corners: top left, top right, bottom
    /// right, bottom left.
    pub fn radii(&self) -> [f32; 4] {
        self.corners.unwrap_or([self.radius; 4])
    }

    /// Its corners made as round as these, as a box's are.
    pub fn set_radii(&mut self, radii: [f32; 4]) {
        let radii = radii.map(|radius| radius.max(0.0));
        self.radius = radii[0];
        self.corners = radii.iter().any(|&radius| radius != radii[0]).then_some(radii);
    }


    /// A point of the artboard as a point of the picture unturned, from
    /// its middle.
    fn local(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let (sin, cos) = (-self.angle).sin_cos();
        let (dx, dy) = (x - self.cx, y - self.cy);
        (dx * cos - dy * sin, dx * sin + dy * cos)
    }

    /// A point of the picture unturned, from its middle, as a point of
    /// the artboard.
    fn placed(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let (sin, cos) = self.angle.sin_cos();
        (self.cx + x * cos - y * sin, self.cy + x * sin + y * cos)
    }

    /// Its corners on the artboard.
    fn corners(&self) -> [(f32, f32); 4] {
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|corner| self.placed(corner))
    }
}

/// What a layer is: a picture, or a shape of the editor's.
#[derive(Clone)]
pub enum Kind {
    Picture(Picture),
    Shape(Shape),
}

#[derive(Clone)]
pub struct Layer {
    pub kind: Kind,
    pub hidden: bool,
    pub locked: bool,
    /// What a shape has been renamed, in place of what it is called for
    /// what it is; a picture keeps its own name.
    pub named: Option<String>,
}

impl Layer {
    fn picture(picture: Picture) -> Layer {
        Layer { kind: Kind::Picture(picture), hidden: false, locked: false, named: None }
    }

    fn shape(shape: Shape) -> Layer {
        Layer { kind: Kind::Shape(shape), hidden: false, locked: false, named: None }
    }

    /// What the list of layers calls it.
    fn name(&self) -> String {
        match &self.kind {
            Kind::Picture(picture) => picture.name.clone(),
            Kind::Shape(_) if self.named.is_some() => self.named.clone().unwrap_or_default(),
            // A board's box or arrow by its words.
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Arrow | Tool::Line) && !shape.text.trim().is_empty() => shape.text.lines().next().unwrap_or("").chars().take(24).collect(),
            Kind::Shape(shape) => match shape.tool {
                Tool::Text | Tool::Badge => shape.text.lines().next().unwrap_or("Text").chars().take(24).collect(),
                tool => tool_name(tool).to_owned(),
            },
        }
    }

    /// It renamed, where it is listed.
    fn rename(&mut self, name: String) {
        match &mut self.kind {
            Kind::Picture(picture) => picture.name = name,
            Kind::Shape(_) => self.named = Some(name),
        }
    }

    /// The box round all of it on the artboard.
    fn bounds(&self, text: &mut Text) -> (f32, f32, f32, f32) {
        match &self.kind {
            Kind::Picture(picture) => {
                let corners = picture.corners();
                let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for (x, y) in corners {
                    (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
                }
                (left, top, right - left, bottom - top)
            }
            Kind::Shape(shape) => {
                let place = shape.bounds(text);
                (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
            }
        }
    }

    fn hit(&self, at: (f32, f32), text: &mut Text) -> bool {
        match &self.kind {
            Kind::Picture(picture) => {
                let (x, y) = picture.local(at);
                x.abs() <= picture.w / 2.0 && y.abs() <= picture.h / 2.0
            }
            Kind::Shape(shape) => shape.hit(at.0.round() as i32, at.1.round() as i32, text),
        }
    }

    /// The shadow it casts, number by number, if it casts one: a
    /// picture's from its preset unless its numbers were set.
    fn drop(&self) -> Option<frame::Layer> {
        match &self.kind {
            Kind::Picture(picture) => match picture.shadow {
                0 => None,
                CUSTOM_SHADOW => Some(picture.drop),
                which => Some(actions::custom_drop(which)),
            },
            Kind::Shape(shape) => shape.shadow,
        }
    }

    /// Its shadow changed number by number: a picture's preset made its
    /// numbers first. Nothing, if it casts none.
    fn change_drop(&mut self, change: impl Fn(&mut frame::Layer)) {
        match &mut self.kind {
            Kind::Picture(picture) => {
                if picture.shadow == 0 {
                    return;
                }
                if picture.shadow != CUSTOM_SHADOW {
                    picture.drop = actions::custom_drop(picture.shadow);
                    picture.shadow = CUSTOM_SHADOW;
                }
                change(&mut picture.drop);
            }
            Kind::Shape(shape) => {
                if let Some(drop) = &mut shape.shadow {
                    change(drop);
                }
            }
        }
    }

    fn translate(&mut self, dx: f32, dy: f32) {
        match &mut self.kind {
            Kind::Picture(picture) => (picture.cx, picture.cy) = (picture.cx + dx, picture.cy + dy),
            Kind::Shape(shape) => shape.translate(dx, dy),
        }
    }
}

/// What the artboard is filled with behind everything on it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fill {
    None,
    Color(u32),
    /// One of the backdrops the frame box has, by where it is in
    /// `frame::BACKDROPS`.
    Backdrop(usize),
    /// From one color to another, going the way an angle in degrees
    /// says, as CSS's linear-gradient does.
    Gradient(u32, u32, f32),
    /// From one color in the middle to another at the edges, as CSS's
    /// radial-gradient does.
    Radial(u32, u32),
}

impl Fill {
    /// The colors a gradient goes from and to, if it is one.
    fn stops(self) -> Option<(u32, u32)> {
        match self {
            Fill::Gradient(from, to, _) | Fill::Radial(from, to) => Some((from, to)),
            _ => None,
        }
    }

    /// The same gradient between two other colors.
    fn with_stops(self, from: u32, to: u32) -> Fill {
        match self {
            Fill::Gradient(_, _, angle) => Fill::Gradient(from, to, angle),
            Fill::Radial(..) => Fill::Radial(from, to),
            other => other,
        }
    }
}

/// The artboard: how large, on what, and what is on it, the first layer
/// at the back.
#[derive(Clone)]
pub struct Doc {
    /// What it is called, and where it is on its page.
    pub name: String,
    pub x: f32,
    pub y: f32,
    /// The change it was last changed at, which says when what is shown
    /// of it is behind; not kept in its file.
    pub stamp: u64,
    pub width: i32,
    pub height: i32,
    pub background: Fill,
    pub layers: Vec<Layer>,
    /// The sizes it is exported at, each with what its file's name ends in
    /// before the kind, and the kind of file.
    pub exports: Vec<ExportSize>,
    /// Whether it is the layers of a page outside its frames, drawn on the
    /// page with nothing behind them and kept to no size; not kept in its
    /// file as a frame is.
    pub page: bool,
    /// The stacks of its layers, each laid out as its auto layout says.
    pub stacks: Vec<stacks::Stack>,
    /// Where a frame is among the things on its page, bottom to top: over
    /// how many of the page's own layers, and among the frames over as
    /// many, its place, the lower first. A file from before says neither,
    /// and has its frames under all the page's own layers.
    pub below: usize,
    pub tier: u32,
}

/// A size a frame is exported at.
#[derive(Clone, PartialEq, Debug)]
pub struct ExportSize {
    pub times: f32,
    pub suffix: String,
    pub format: crate::output::Format,
    /// How it is written, where it was fine-tuned and that was used for
    /// it, in place of the config file's `[convert]`.
    pub tuned: Option<crate::convert::Settings>,
}

impl ExportSize {
    fn new(times: f32) -> ExportSize {
        ExportSize { times, suffix: String::new(), format: crate::output::Format::Png, tuned: None }
    }

    /// Its kind of file as a word in a file of the canvas, with how it is
    /// written when that is its own: `avif{quality=70;speed=6;depth=8}`.
    fn format_word(&self) -> String {
        match self.tuned.map(|tuned| tuned.table_of(self.format).1).filter(|table| !table.is_empty()) {
            Some(table) => format!("{}{}", self.format.word(), table.replace(' ', "").replace(',', ";")),
            None => self.format.word().to_owned(),
        }
    }

    /// The kind of file and how it is written, back from its word.
    fn read_format(word: &str) -> Option<(crate::output::Format, Option<crate::convert::Settings>)> {
        let Some((name, table)) = word.split_once('{') else { return Some((crate::output::Format::named(word)?, None)) };
        let format = crate::output::Format::named(name)?;
        let key = crate::convert::Settings::default().table_of(format).0;
        let mut tuned = crate::convert::Settings::default();
        tuned.apply_text(&format!("{key} = {{{}", table.replace(';', ","))).ok()?;
        Some((format, Some(tuned)))
    }

    /// What its file's name ends in: the suffix given it, or with none, at
    /// any size but its own, "@2x" for twice it, as Figma names them.
    fn file_suffix(&self) -> String {
        match self.suffix.is_empty() && self.times != 1.0 {
            true => format!("@{}x", self.times),
            false => self.suffix.clone(),
        }
    }
}

/// A shape made larger or smaller by a factor, about the artboard's top
/// left: how one is drawn into a picture exported at twice the size.
fn scaled(shape: &Shape, times: f32) -> Shape {
    let mut shape = shape.clone();
    for point in &mut shape.points {
        *point = (point.0 * times, point.1 * times);
    }
    (shape.stroke, shape.radius) = (shape.stroke * times, shape.radius * times);
    shape.corners = shape.corners.map(|corners| corners.map(|radius| radius * times));
    shape.font_px = ((shape.font_px as f32 * times).round() as u32).max(1);
    if let Some(drop) = &mut shape.shadow {
        let by = |value: i16| (f32::from(value) * times).round() as i16;
        (drop.x, drop.y, drop.blur, drop.spread) = (by(drop.x), by(drop.y), by(drop.blur), by(drop.spread));
    }
    shape.touch();
    shape
}

impl Doc {
    pub fn new(width: i32, height: i32) -> Doc {
        Doc { name: "Frame 1".into(), x: 0.0, y: 0.0, stamp: 0, width, height, background: Fill::Color(0xFFFFFF), layers: Vec::new(), exports: vec![ExportSize::new(1.0)], page: false, stacks: Vec::new(), below: 0, tier: 0 }
    }

    /// The layers of a page outside its frames, as Figma has them: on
    /// nothing, where the page's own points are theirs, of no size.
    pub fn for_page() -> Doc {
        Doc { name: "Page".into(), background: Fill::None, exports: Vec::new(), page: true, ..Doc::new(1, 1) }
    }

    /// All of it at `times` its size, from its corner: a frame's size, or
    /// for a page's own layers as far as anything could be.
    pub(super) fn whole(&self, times: f32) -> Rect {
        match self.page {
            true => Rect::new(-(1 << 28), -(1 << 28), 1 << 29, 1 << 29),
            false => Rect::new(0, 0, ((self.width as f32 * times).round() as i32).max(1), ((self.height as f32 * times).round() as i32).max(1)),
        }
    }

    /// The artboard as a picture, `times` its size: each pixel with how
    /// opaque it is in its top byte, which with nothing behind is not at
    /// all where nothing is on it.
    pub fn render(&self, times: f32, text: &mut Text) -> (i32, i32, Vec<u32>) {
        let (wide, high) = (((self.width as f32 * times).round() as i32).max(1), ((self.height as f32 * times).round() as i32).max(1));
        (wide, high, self.render_part(times, Rect::new(0, 0, wide, high), text))
    }

    /// The same as `render_part`, drawn a band of rows at a time, and given
    /// up between two, with nothing, when `stop` says to.
    #[cfg(test)]
    pub fn render_part_stopping(&self, times: f32, part: Rect, text: &mut Text, stop: &mut dyn FnMut() -> bool) -> Option<Vec<u32>> {
        self.render_range_stopping(drawn::ALL, times, part, text, stop)
    }

    /// The same of some of its layers alone, from one up to before another.
    pub(super) fn render_range_stopping(&self, range: (usize, usize), times: f32, part: Rect, text: &mut Text, stop: &mut dyn FnMut() -> bool) -> Option<Vec<u32>> {
        let mut out = vec![0u32; (part.w.max(0) * part.h.max(0)) as usize];
        let band = ((part.h + 7) / 8).max(128);
        for top in (0..part.h.max(0)).step_by(band as usize) {
            if stop() {
                return None;
            }
            let rows = Rect::new(part.x, part.y + top, part.w, band.min(part.h - top));
            let pixels = self.render_range(range, times, rows, text);
            out[(top * part.w) as usize..][..pixels.len()].copy_from_slice(&pixels);
        }
        Some(out)
    }

    /// Part of the artboard as a picture, `times` its size: the pixels of
    /// `part` of that, everything drawn afresh at that size, so that the
    /// shapes and the text on it are as sharp however large it is shown.
    pub fn render_part(&self, times: f32, part: Rect, text: &mut Text) -> Vec<u32> {
        self.render_under(self.layers.len(), times, part, text)
    }

    /// The same of the background and only so many of the layers, the
    /// first of them: what a blur, a spotlight or a zoom works on.
    fn render_under(&self, count: usize, times: f32, part: Rect, text: &mut Text) -> Vec<u32> {
        self.render_between(0, count, times, part, text)
    }

    /// The same of some of the page's own layers, from one up to before
    /// another: those between two frames on the page, drawn apart.
    pub(super) fn render_range(&self, (from, to): (usize, usize), times: f32, part: Rect, text: &mut Text) -> Vec<u32> {
        self.render_between(from, to, times, part, text)
    }

    /// The background and the layers from one up to before another.
    fn render_between(&self, from: usize, count: usize, times: f32, part: Rect, text: &mut Text) -> Vec<u32> {
        let (wide, high) = (((self.width as f32 * times).round() as i32).max(1) as f32, ((self.height as f32 * times).round() as i32).max(1) as f32);
        let mut out = vec![0u32; (part.w.max(0) * part.h.max(0)) as usize];
        if out.is_empty() {
            return out;
        }
        match self.background {
            Fill::None => {}
            Fill::Color(color) => out.iter_mut().for_each(|pixel| *pixel = 0xFF00_0000 | color),
            Fill::Backdrop(which) => out.par_chunks_mut(part.w as usize).enumerate().for_each(|(y, row)| {
                let y = y + part.y as usize;
                for (x, pixel) in row.iter_mut().enumerate() {
                    let x = x + part.x as usize;
                    let noise = ((x * 7 + y * 13) % 16) as f32 / 16.0;
                    *pixel = 0xFF00_0000 | frame::shade(which, (x as f32 + 0.5) / wide, (y as f32 + 0.5) / high, noise);
                }
            }),
            Fill::Gradient(from, to, angle) => out.par_chunks_mut(part.w as usize).enumerate().for_each(|(y, row)| {
                let y = y + part.y as usize;
                for (x, pixel) in row.iter_mut().enumerate() {
                    let x = x + part.x as usize;
                    *pixel = 0xFF00_0000 | gradient(from, to, angle, (x as f32 + 0.5, y as f32 + 0.5), (wide, high), (x * 7 + y * 13) % 16);
                }
            }),
            Fill::Radial(from, to) => out.par_chunks_mut(part.w as usize).enumerate().for_each(|(y, row)| {
                let y = y + part.y as usize;
                for (x, pixel) in row.iter_mut().enumerate() {
                    let x = x + part.x as usize;
                    let noise = (((x * 7 + y * 13) % 16) as f32 / 16.0 - 0.5) / 255.0;
                    let along = shapes::along_gradient(x as f32 + 0.5 - wide / 2.0, y as f32 + 0.5 - high / 2.0, wide, high, 0.0, true);
                    *pixel = 0xFF00_0000 | shapes::between(from, to, along + noise);
                }
            }),
        }
        // A stack's fill under the first of what is in it.
        let firsts = stacks::fill_firsts(self);
        let count = count.min(self.layers.len());
        for (index, layer) in self.layers.iter().enumerate().take(count).skip(from.min(count)).filter(|(_, layer)| !layer.hidden) {
            if !firsts.is_empty() {
                stacks::draw_fills(&self.stacks, stacks::layer_id(layer), &firsts, &mut out, part, times);
            }
            match &layer.kind {
                Kind::Picture(picture) => draw_picture(&mut out, part, picture, times, text),
                Kind::Shape(shape) if effects::changes_beneath(shape.tool) => self.change_beneath_at_most(index, &mut out, part, times, text),
                Kind::Shape(shape) => {
                    let mut shape = if times == 1.0 { shape.clone() } else { scaled(shape, times) };
                    if (part.x, part.y) != (0, 0) {
                        shape.translate(-part.x as f32, -part.y as f32);
                    }
                    draw_shape(&mut out, part.w, part.h, &shape, text);
                }
            }
        }
        out
    }

    /// The frame as a file of its own: one page with it on it.
    pub fn write(&self) -> Vec<u8> {
        write_pages(&[Page::new("Page 1", vec![self.clone()])])
    }

    /// The first frame of a file, or nothing from anything else.
    #[cfg(test)]
    pub fn read(bytes: &[u8]) -> Option<Doc> {
        read_pages(bytes)?.into_iter().flat_map(|page| page.frames).next()
    }

    /// What it is, line by line: its size, its background and its layers,
    /// each picture by where it is among `images`, which it is added to.
    fn write_lines(&self, out: &mut String, images: &mut Vec<Arc<Image>>) {
        *out += &format!("size {} {}\n", self.width, self.height);
        let sizes: Vec<String> = self.exports.iter().map(|size| format!("{}:{}:{}", size.times, size.format_word(), size.suffix.replace([' ', ':'], "_"))).collect();
        *out += &format!("exports {}\n", sizes.join(" "));
        *out += &match self.background {
            Fill::None => "background none\n".to_owned(),
            Fill::Color(color) => format!("background color {color:06X}\n"),
            Fill::Backdrop(which) => format!("background backdrop {}\n", BACKDROPS[which.min(BACKDROPS.len() - 1)].0),
            Fill::Gradient(from, to, angle) => format!("background gradient {from:06X} {to:06X} {angle}\n"),
            Fill::Radial(from, to) => format!("background radial {from:06X} {to:06X}\n"),
        };
        self.write_layers(out, images);
    }

    /// Its layers, line by line, the first at the back.
    fn write_layers(&self, out: &mut String, images: &mut Vec<Arc<Image>>) {
        for layer in &self.layers {
            let flags = format!("{} {}", u8::from(layer.hidden), u8::from(layer.locked));
            match &layer.kind {
                Kind::Picture(picture) => {
                    let index = images.iter().position(|image| Arc::ptr_eq(image, &picture.image)).unwrap_or_else(|| {
                        images.push(picture.image.clone());
                        images.len() - 1
                    });
                    let (cx, cy, cw, ch) = picture.crop;
                    *out += &format!(
                        "picture {flags} {index} {} {} {} {} {} {cx} {cy} {cw} {ch} {} {} {} {} {} {} {}\n",
                        picture.cx,
                        picture.cy,
                        picture.w,
                        picture.h,
                        picture.angle,
                        picture.radius,
                        picture.shadow,
                        picture.opacity,
                        u8::from(picture.flip.0),
                        u8::from(picture.flip.1),
                        picture.id,
                        one_line(&picture.name)
                    );
                    let drop = picture.drop;
                    *out += &format!("drop {} {} {} {} {:08X}\n", drop.x, drop.y, drop.blur, drop.spread, drop.color);
                    if let Some([a, b, c, d]) = picture.corners {
                        *out += &format!("corners {a} {b} {c} {d}\n");
                    }
                    if picture.adjust != [0.0; 3] {
                        *out += &format!("adjust {} {} {}\n", picture.adjust[0], picture.adjust[1], picture.adjust[2]);
                    }
                    if let Some(chrome) = &picture.chrome {
                        *out += &format!("chrome {} {} {} {} {}\n", chrome.style.word(), chrome.theme.word(), u8::from(chrome.swap), chrome.shadow, one_line(&chrome.address));
                    }
                    if !picture.title.is_empty() {
                        *out += &format!("title {}\n", one_line(&picture.title));
                    }
                    if let Some([l, r, t, b]) = picture.insets {
                        *out += &format!("insets {l} {r} {t} {b} {}\n", picture.screen_scale);
                    }
                }
                Kind::Shape(shape) => *out += &format!("shape {flags} {}\n", shape.write()),
            }
            // What a shape was renamed, on a line after its own, which a
            // file from before names did without.
            if let (Kind::Shape(_), Some(name)) = (&layer.kind, &layer.named) {
                *out += &format!("named {}\n", one_line(name));
            }
        }
        // The stacks after the layers they hold, which a file from before
        // them has none of.
        for stack in &self.stacks {
            *out += &format!("{}\n", stack.write());
            if let Some(name) = &stack.name {
                *out += &format!("stack-named {}\n", one_line(name));
            }
            if let Some(holder) = stack.holder {
                *out += &format!("stack-holds {holder}\n");
            }
        }
    }

    /// One line of what `write_lines` wrote, read into it; or nothing, if
    /// the line is not one of those.
    fn read_line(&mut self, line: &str, images: &[Arc<Image>]) -> Option<()> {
        let mut words = line.split(' ');
        match words.next()? {
            "size" => (self.width, self.height) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?),
            "background" => {
                self.background = match words.next()? {
                    "color" => Fill::Color(u32::from_str_radix(words.next()?, 16).ok()?),
                    "backdrop" => Fill::Backdrop(frame::named(words.next()?)?),
                    "gradient" => Fill::Gradient(u32::from_str_radix(words.next()?, 16).ok()?, u32::from_str_radix(words.next()?, 16).ok()?, words.next()?.parse().ok()?),
                    "radial" => Fill::Radial(u32::from_str_radix(words.next()?, 16).ok()?, u32::from_str_radix(words.next()?, 16).ok()?),
                    _ => Fill::None,
                }
            }
            "picture" => {
                let (hidden, locked) = (words.next()? == "1", words.next()? == "1");
                let image = images.get(words.next()?.parse::<usize>().ok()?)?.clone();
                let mut number = || words.next().and_then(|word| word.parse::<f32>().ok());
                let (cx, cy, w, h, angle) = (number()?, number()?, number()?, number()?, number()?);
                let crop = (number()?, number()?, number()?, number()?);
                let (radius, shadow, opacity) = (number()?, number()? as usize, number()? as u32);
                let flip = (number()? == 1.0, number()? == 1.0);
                let id = number()? as u32;
                shapes::seen_id(id);
                let name = words.collect::<Vec<_>>().join(" ");
                let picture = Picture { shadow: shadow.min(CUSTOM_SHADOW), opacity: opacity.min(256), flip, id, radius, crop, angle, ..Picture::new(image, name, (cx, cy)) };
                let picture = Picture { w, h, ..picture };
                self.layers.push(Layer { kind: Kind::Picture(picture), hidden, locked, named: None });
            }
            // What a picture has besides, on lines after its own.
            "drop" | "adjust" | "window" | "corners" | "chrome" | "title" | "insets" => {
                let Some(Layer { kind: Kind::Picture(picture), .. }) = self.layers.last_mut() else { return None };
                match line.split(' ').next()? {
                    "corners" => {
                        let mut number = || words.next().and_then(|word| word.parse::<f32>().ok()).filter(|radius| radius.is_finite());
                        let radii = [number()?, number()?, number()?, number()?];
                        picture.set_radii(radii);
                    }
                    "drop" => {
                        let mut number = || words.next().and_then(|word| word.parse::<i16>().ok());
                        let (x, y, blur, spread) = (number()?, number()?, number()?, number()?);
                        picture.drop = frame::Layer { x, y, blur, spread, color: u32::from_str_radix(words.next()?, 16).ok()? };
                    }
                    "adjust" => {
                        for value in &mut picture.adjust {
                            *value = words.next()?.parse::<f32>().ok()?.clamp(-1.0, 1.0);
                        }
                    }
                    "chrome" => {
                        let style = chrome::Style::named(words.next()?)?;
                        let theme = chrome::Theme::named(words.next()?)?;
                        let swap = words.next()? == "1";
                        let shadow = words.next()?.parse::<u8>().ok()?.min(OWN_SHADOW);
                        picture.chrome = Some(Chrome { style, theme, swap, shadow, address: words.collect::<Vec<_>>().join(" ") });
                    }
                    "title" => picture.title = line["title ".len().min(line.len())..].to_owned(),
                    "insets" => {
                        let mut number = || words.next().and_then(|word| word.parse::<f32>().ok()).filter(|value| value.is_finite());
                        picture.insets = Some([number()?, number()?, number()?, number()?].map(|side| side.max(0.0) as i32));
                        picture.screen_scale = number().filter(|scale| *scale > 0.0).unwrap_or(1.0);
                    }
                    // A window as files had it before there were more of
                    // them: none, a browser, a Mac's, a terminal or a
                    // desktop's, by number, and its title after.
                    _ => {
                        let style = [chrome::Style::None, chrome::Style::Browser, chrome::Style::Mac, chrome::Style::Terminal, chrome::Style::Desktop].get(words.next()?.parse::<usize>().ok()?).copied()?;
                        picture.chrome = (style != chrome::Style::None).then(|| Chrome { shadow: OWN_SHADOW, ..Chrome::new(style) });
                        picture.title = words.collect::<Vec<_>>().join(" ");
                    }
                }
            }
            "exports" => {
                let mut sizes = Vec::new();
                for word in words.filter(|word| !word.is_empty()) {
                    let mut parts = word.splitn(3, ':');
                    let times = parts.next()?.parse::<f32>().ok()?.clamp(0.1, 8.0);
                    let (format, tuned) = ExportSize::read_format(parts.next()?)?;
                    sizes.push(ExportSize { times, format, suffix: parts.next().unwrap_or("").to_owned(), tuned });
                }
                self.exports = sizes;
            }
            "named" => {
                let Some(Layer { kind: Kind::Shape(_), named, .. }) = self.layers.last_mut() else { return None };
                *named = Some(line["named ".len().min(line.len())..].to_owned());
            }
            "stack" => self.stacks.push(stacks::Stack::read(&line["stack ".len().min(line.len())..])?),
            "stack-named" => self.stacks.last_mut()?.name = Some(line["stack-named ".len().min(line.len())..].to_owned()),
            "stack-holds" => self.stacks.last_mut()?.holder = Some(words.next()?.parse().ok()?),
            "stacking" => (self.below, self.tier) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?),
            "shape" => {
                let rest = line.splitn(4, ' ').collect::<Vec<_>>();
                let shape = Shape::read(rest.get(3)?)?;
                self.layers.push(Layer { kind: Kind::Shape(shape), hidden: rest[1] == "1", locked: rest[2] == "1", named: None });
            }
            _ => return None,
        }
        Some(())
    }
}

/// A name kept to one line of a file.
fn one_line(name: &str) -> String {
    name.replace(['\n', '\t', '\r'], " ")
}

/// A file of the canvas: what it is, line by line, page by page and frame
/// by frame; and then each picture on it as the PNG it is, after its own
/// line saying how long it is, once however many times it is used.
pub fn write_pages(pages: &[Page]) -> Vec<u8> {
    let mut out = format!("{MAGIC}\n");
    let mut images: Vec<Arc<Image>> = Vec::new();
    for page in pages {
        out += &format!("page {}\n", one_line(&page.name));
        if page.board {
            out += "board\n";
        } else if page.frames.is_empty() {
            // A board made a page of frames again before it had any.
            out += "no-frames\n";
        }
        for frame in &page.frames {
            out += &format!("frame {} {} {}\n", frame.x, frame.y, one_line(&frame.name));
            frame.write_lines(&mut out, &mut images);
            // Where it is among the page's own layers and the other
            // frames, when that is not under them all as files had it.
            if (frame.below, frame.tier) != (0, 0) {
                out += &format!("stacking {} {}\n", frame.below, frame.tier);
            }
        }
        // The page's own layers, outside its frames, after them, where a
        // file from before them has none.
        if !page.loose.layers.is_empty() {
            out += "page-layers\n";
            page.loose.write_layers(&mut out, &mut images);
        }
    }
    let mut bytes = out.into_bytes();
    for image in images {
        bytes.extend_from_slice(format!("image {}\n", image.png.len()).as_bytes());
        bytes.extend_from_slice(&image.png);
        bytes.push(b'\n');
    }
    bytes
}

/// The pages of a file `write_pages` made, or of one from before pages,
/// which was one frame; or nothing from any other.
pub fn read_pages(bytes: &[u8]) -> Option<Vec<Page>> {
    // The lines come first, the pictures after: the lines are read up to
    // the first picture, which is where the bytes begin.
    let text_end = find(bytes, b"\nimage ").map_or(bytes.len(), |at| at + 1);
    let text = std::str::from_utf8(&bytes[..text_end]).ok()?;
    let mut lines = text.lines();
    let first = lines.next()?;
    if first != MAGIC && first != OLD_MAGIC {
        return None;
    }
    let mut images = Vec::new();
    let mut at = text_end;
    while let Some(rest) = bytes.get(at..).filter(|rest| rest.starts_with(b"image ")) {
        let line_end = rest.iter().position(|&byte| byte == b'\n')?;
        let length: usize = std::str::from_utf8(&rest[6..line_end]).ok()?.trim().parse().ok()?;
        let png = rest.get(line_end + 1..line_end + 1 + length)?;
        images.push(Arc::new(Image::open(png)?));
        at += line_end + 1 + length + 1;
    }
    let mut pages: Vec<Page> = Vec::new();
    // Whether the lines are of the page's own layers, which come after
    // its frames, until another page or frame.
    let mut own = false;
    let mut frameless: Vec<usize> = Vec::new();
    for line in lines {
        if let Some(name) = line.strip_prefix("page ") {
            pages.push(Page::new(name.to_owned(), Vec::new()));
            own = false;
            continue;
        }
        if pages.is_empty() {
            pages.push(Page::new("Page 1", Vec::new()));
        }
        let page = pages.last_mut()?;
        if line == "board" && page.frames.is_empty() && !own {
            page.board = true;
            continue;
        }
        if line == "no-frames" && page.frames.is_empty() && !own {
            frameless.push(pages.len() - 1);
            continue;
        }
        if line == "page-layers" {
            own = true;
            continue;
        }
        if own && !line.starts_with("frame ") {
            let _ = page.loose.read_line(line, &images);
            continue;
        }
        own = false;
        if let Some(rest) = line.strip_prefix("frame ") {
            let mut words = rest.splitn(3, ' ');
            let mut frame = Doc::new(1, 1);
            (frame.x, frame.y) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?);
            frame.name = words.next().unwrap_or("Frame").to_owned();
            page.frames.push(frame);
            continue;
        }
        // A file from before frames is one frame, without saying so.
        if page.frames.is_empty() {
            page.frames.push(Doc::new(1, 1));
        }
        let _ = page.frames.last_mut()?.read_line(line, &images);
    }
    for (index, page) in pages.iter_mut().enumerate() {
        page.frames.retain(|frame| frame.width > 0 && frame.height > 0);
        if page.frames.is_empty() && !page.board && !frameless.contains(&index) {
            page.frames.push(Doc::new(1200, 675));
        }
        page.seen = page.loose.layers.iter().map(stacks::layer_id).collect();
    }
    (!pages.is_empty()).then_some(pages)
}

/// A page of a file: an endless canvas, with frames on it, each a picture
/// to export; and layers of its own outside them, drawn on the page and
/// exported with none of the frames, as Figma has them.
#[derive(Clone)]
pub struct Page {
    pub name: String,
    pub frames: Vec<Doc>,
    pub loose: Doc,
    /// The page's own layers as they were when where the frames are among
    /// them was last worked out, by what each is called: for the frames
    /// to keep their places as layers come and go.
    pub seen: Vec<u32>,
    /// Whether it is a board, for explaining, rather than a page of frames
    /// to design in: its own look, tools and ways, and frames only when
    /// part of it is wanted as a picture.
    pub board: bool,
}

impl Page {
    pub fn new(name: impl Into<String>, frames: Vec<Doc>) -> Page {
        Page { name: name.into(), frames, loose: Doc::for_page(), seen: Vec::new(), board: false }
    }
}

/// One of the things on a page, in the order they are drawn: a frame, or
/// one of the page's own layers, each by where it is among its kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Child {
    Frame(usize),
    Layer(usize),
}

/// Where among a page's frames its own layers are said to be, outside
/// them all: the frame worked in, when it is they that are worked in.
const LOOSE: usize = usize::MAX;

/// A frame of a page by where it is among them, or at `LOOSE` its own
/// layers.
fn slot(page: &mut Page, index: usize) -> &mut Doc {
    match index {
        LOOSE => &mut page.loose,
        index => &mut page.frames[index],
    }
}


/// A path as said to someone: with the home folder as ~.
pub(crate) fn shortened(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// A canvas of a picture from the editor, and the shapes drawn on it,
/// written as the editor writes them, still to be moved and changed:
/// the picture just as it was taken, in a frame just its size with nothing
/// behind it, and the shapes moved from where they were on the screen to
/// where it is in the frame.
pub fn from_markup(area: Rect, pixels: &[u32], shapes: &str, notes: &chrome::Notes) -> Doc {
    let mut doc = Doc::new(area.w, area.h);
    // Exported at its own size, pixel for pixel, as a screenshot is wanted.
    (doc.background, doc.exports) = (Fill::None, vec![ExportSize::new(1.0)]);
    // The shapes read first, so that the picture is called something
    // none of them is.
    let drawn: Vec<Shape> = shapes.lines().filter_map(Shape::read).collect();
    let image = Image::new(area.w, area.h, pixels.iter().map(|pixel| pixel | 0xFF00_0000).collect());
    let middle = (area.w as f32 / 2.0, area.h as f32 / 2.0);
    let mut picture = Picture::new(Arc::new(image), "Screenshot".into(), middle);
    // A window knows its title, where its own bar is, and how large a
    // pixel of its screen was; and is put in the window chosen last, if
    // one has been.
    (picture.title, picture.insets) = (notes.title.clone(), notes.insets);
    if notes.scale > 0.0 {
        picture.screen_scale = notes.scale;
    }
    doc.layers.push(Layer::picture(picture));
    // Spotlights first, just over the picture: in capture mode they darken
    // the picture and not what is drawn on it.
    let (lit, rest): (Vec<Shape>, Vec<Shape>) = drawn.into_iter().partition(|shape| shape.tool == Tool::Spotlight);
    for mut shape in lit.into_iter().chain(rest) {
        shape.translate(-area.x as f32, -area.y as f32);
        doc.layers.push(Layer::shape(shape));
    }
    // The frame grown round its window, what is drawn on the picture
    // staying on it.
    if let (Some(style), true) = (chromes::kept_window(), notes.insets.is_some()) {
        if let Kind::Picture(picture) = &mut doc.layers[0].kind {
            let was = chromes::picture_box(picture);
            picture.set_chrome(Some(Chrome::new(style)));
            let now = chromes::picture_box(picture);
            doc.follow_picture(was, now);
            (doc.x, doc.y) = (0.0, 0.0);
        }
    }
    doc
}

/// Where some bytes first come in others.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

/// The color of a gradient at a point of a box, as CSS's linear-gradient
/// has it: along a line through its middle the way the angle says, long
/// enough that its ends just reach the far corners. A little noise keeps
/// a wide one from showing bands.
pub fn gradient(from: u32, to: u32, angle: f32, (x, y): (f32, f32), (wide, high): (f32, f32), noise: usize) -> u32 {
    let (sin, cos) = angle.to_radians().sin_cos();
    let length = (wide * sin).abs() + (high * cos).abs();
    let along = ((x - wide / 2.0) * sin - (y - high / 2.0) * cos) / length.max(1.0) + 0.5;
    let along = (along + (noise as f32 / 16.0 - 0.5) / 255.0).clamp(0.0, 1.0);
    let part = |shift: u32| {
        let (a, b) = ((from >> shift & 0xFF) as f32, (to >> shift & 0xFF) as f32);
        ((a + (b - a) * along).round() as u32).min(255) << shift
    };
    part(16) | part(8) | part(0)
}

/// A color over another, each with how opaque it is in its top byte, by
/// how much of the one on top there is, out of 1.
fn over(under: u32, color: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= 0.0 {
        return under;
    }
    let (under_a, top_a) = ((under >> 24) as f32 / 255.0, amount);
    let out_a = top_a + under_a * (1.0 - top_a);
    if out_a <= 0.0 {
        return 0;
    }
    let part = |shift: u32| {
        let (u, c) = ((under >> shift & 0xFF) as f32, (color >> shift & 0xFF) as f32);
        ((c * top_a + u * under_a * (1.0 - top_a)) / out_a).round().clamp(0.0, 255.0) as u32
    };
    ((out_a * 255.0).round() as u32) << 24 | part(16) << 8 << 8 | part(8) << 8 | part(0)
}

/// A picture drawn onto the artboard at `times` its size: its shadow, then
/// it, each pixel of it from the four nearest of the copy of it of about
/// the size it is shown at, kept to its rounded corners; and the window
/// round it, if it is in one, its bar over the top of it.
fn draw_picture(out: &mut [u32], part: Rect, picture: &Picture, times: f32, text: &mut Text) {
    let (wide, high) = (part.w, part.h);
    let mut at = picture.clone();
    (at.cx, at.cy, at.w, at.h, at.radius) = (picture.cx * times - part.x as f32, picture.cy * times - part.y as f32, picture.w * times, picture.h * times, picture.radius * times);
    at.corners = picture.corners.map(|corners| corners.map(|radius| radius * times));
    let picture = &at;
    let custom = [picture.drop];
    let preset = Shadow::of_preset(picture.shadow.saturating_sub(1));
    // Tailwind's shadows are for a page seen at its size; an artboard is
    // most often seen at half its own, and its shadows are twice as large.
    // One set number by number is in pixels of the artboard. A window's
    // own is in pixels of the screen it was on, as large as its bar is.
    let (layers, lift) = match picture.chrome.as_ref().map(|chrome| chrome.shadow) {
        Some(SYSTEM_SHADOW) => (picture.system_shadow(), picture.screen_scale * picture.content_scale()),
        Some(NO_SHADOW) => (&[][..], 1.0),
        _ => match picture.shadow {
            0 => (&[][..], 1.0),
            CUSTOM_SHADOW => (&custom[..], times),
            _ => (preset.layers(), times * SHADOW_SIZE),
        },
    };
    // How far out from it anything of it goes: its shadow's reach.
    let reach = layers.iter().map(|layer| (f32::from(layer.blur) * 1.5 + f32::from(layer.spread) + f32::from(layer.x.abs().max(layer.y.abs()))) * lift).fold(0.0f32, f32::max);
    let corners = picture.corners();
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in corners {
        (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
    }
    let area = Rect::new((left - reach).floor() as i32, (top - reach).floor() as i32, (right - left + 2.0 * reach).ceil() as i32 + 2, (bottom - top + 2.0 * reach).ceil() as i32 + 2).intersect(Rect::new(0, 0, wide, high));
    if area.is_empty() || picture.w < 0.5 || picture.h < 0.5 {
        return;
    }
    let (hw, hh) = (picture.w / 2.0, picture.h / 2.0);
    // A window's bar over the top of it, and the picture in what is left;
    // its corners as round as the window's are, at least.
    let style = picture.drawn_style();
    let bar = picture.bar();
    let least = picture.window_radius();
    let radii = picture.radii().map(|radius| radius.max(least).clamp(0.0, hw.min(hh)));
    let (cx, cy, cw, ch) = picture.shown_crop();
    let level = picture.image.level(cw / picture.w);
    let (lw, lh, pixels) = (level.0, level.1, &level.2);
    let (ratio_x, ratio_y) = (lw as f32 / picture.image.width() as f32, lh as f32 / picture.image.height() as f32);
    let opacity = picture.opacity as f32 / 256.0;
    let adjusted = picture.adjust != [0.0; 3];
    // Each pixel of it shown as many: drawn as hard squares, as Figma
    // shows a screenshot zoomed far in, rather than smoothed between.
    let hard = (picture.w / cw.max(1e-3)).min(picture.h / ch.max(1e-3)) >= HARD_PIXELS;
    // The bar, drawn once: square to the artboard, just the part of it
    // that is here, pixel for pixel; turned, all of it, to be sampled as
    // the picture is.
    let bar_drawn = (bar > 0.0).then(|| {
        let chrome = picture.chrome.clone().unwrap_or_else(|| Chrome::new(style));
        let drawn = chrome::Bar { style, dark: chrome::is_dark(style, chrome.theme, pixels, lw, lh), title: &picture.title, address: &chrome.address };
        let scale = bar / style.bar();
        if picture.angle == 0.0 {
            let (x0, y0) = (picture.cx - hw, picture.cy - hh);
            let place = Rect::new(x0.floor() as i32, y0.floor() as i32, picture.w.ceil() as i32 + 2, bar.ceil() as i32 + 2).intersect(area);
            let mut drawn_pixels = vec![0xFF00_0000; (place.w.max(0) * place.h.max(0)) as usize];
            chrome::draw_bar(&mut drawn_pixels, (place.w, place.h), (x0 - place.x as f32, y0 - place.y as f32), (picture.w, bar), scale, drawn, Some(text));
            (place, 0.0, drawn_pixels)
        } else {
            let fine = (4096.0 / picture.w).min(1.0);
            let (bw, bh) = ((picture.w * fine).ceil() as i32, (bar * fine).ceil() as i32);
            let drawn_pixels = chrome::bar_pixels(bw, bh, scale * fine, drawn, Some(text));
            (Rect::new(0, 0, bw, bh), fine, drawn_pixels)
        }
    });
    out[(area.y * wide) as usize..(area.bottom() * wide) as usize].par_chunks_mut(wide as usize).enumerate().for_each(|(row, line)| {
        let y = area.y + row as i32;
        for x in area.x..area.right() {
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            let mut pixel = line[x as usize];
            for layer in layers.iter().rev() {
                let shifted = (point.0 - f32::from(layer.x) * lift, point.1 - f32::from(layer.y) * lift);
                let (sx, sy) = picture.local(shifted);
                let grow = f32::from(layer.spread) * lift;
                let away = frame::round_box(sx, sy, (hw + grow).max(0.0), (hh + grow).max(0.0), (shapes::corner_of(sx, sy, radii) + grow).max(0.0));
                let deviation = f32::from(layer.blur.max(0)) * lift / 2.0;
                let near = if deviation < 0.5 { (0.5 - away).clamp(0.0, 1.0) } else { 0.5 * (1.0 - frame::erf(away / (deviation * std::f32::consts::SQRT_2))) };
                pixel = over(pixel, layer.color & 0xFF_FFFF, near * (layer.color >> 24) as f32 / 255.0 * opacity);
            }
            let (lx, ly) = picture.local(point);
            let inside = frame::round_box(lx, ly, hw, hh, shapes::corner_of(lx, ly, radii));
            let cover = (0.5 - inside).clamp(0.0, 1.0);
            if cover > 0.0 {
                if ly + hh < bar {
                    let color = match &bar_drawn {
                        Some((place, fine, drawn)) if *fine == 0.0 => drawn.get(((y - place.y) * place.w + x - place.x) as usize).copied().unwrap_or(0xFF00_0000),
                        Some((place, fine, drawn)) => sample(drawn, place.w, place.h, (lx + hw) * fine - 0.5, (ly + hh) * fine - 0.5),
                        None => 0xFF00_0000,
                    };
                    pixel = over(pixel, color & 0xFF_FFFF, cover * opacity);
                } else {
                    // Where in the picture's own pixels, flipped if it is.
                    let (mut u, mut v) = ((lx + hw) / picture.w, (ly + hh - bar) / (picture.h - bar));
                    if picture.flip.0 {
                        u = 1.0 - u;
                    }
                    if picture.flip.1 {
                        v = 1.0 - v;
                    }
                    let (fx, fy) = ((cx + u * cw) * ratio_x - 0.5, (cy + v * ch) * ratio_y - 0.5);
                    let mut color = match hard {
                        true => pixels[((fy + 0.5).floor().clamp(0.0, (lh - 1) as f32) as i32 * lw + (fx + 0.5).floor().clamp(0.0, (lw - 1) as f32) as i32) as usize],
                        false => sample(pixels, lw, lh, fx, fy),
                    };
                    if adjusted {
                        color = adjust(color, picture.adjust);
                    }
                    pixel = over(pixel, color & 0xFF_FFFF, cover * (color >> 24) as f32 / 255.0 * opacity);
                }
            }
            line[x as usize] = pixel;
        }
    });
}

/// A color of a picture made lighter or darker, more or less contrasted,
/// and more or less colorful, each by -1 to 1.
fn adjust(color: u32, [exposure, contrast, saturation]: [f32; 3]) -> u32 {
    let gain = 2f32.powf(exposure * 1.5);
    let spread = 1.0 + contrast * if contrast > 0.0 { 1.5 } else { 0.9 };
    let channel = |shift: u32| (color >> shift & 0xFF) as f32 / 255.0;
    let (mut r, mut g, mut b) = (channel(16), channel(8), channel(0));
    for value in [&mut r, &mut g, &mut b] {
        *value = ((*value * gain - 0.5) * spread + 0.5).clamp(0.0, 1.0);
    }
    let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let color_by = 1.0 + saturation;
    let back = |value: f32| (((gray + (value - gray) * color_by).clamp(0.0, 1.0)) * 255.0).round() as u32;
    (color & 0xFF00_0000) | back(r) << 16 | back(g) << 8 | back(b)
}

/// A picture's color between its pixels, from the four nearest.
fn sample(pixels: &[u32], width: i32, height: i32, x: f32, y: f32) -> u32 {
    let (x0, y0) = (x.floor().clamp(0.0, (width - 1) as f32) as i32, y.floor().clamp(0.0, (height - 1) as f32) as i32);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (tx, ty) = ((x - x0 as f32).clamp(0.0, 1.0), (y - y0 as f32).clamp(0.0, 1.0));
    let at = |x: i32, y: i32| pixels[(y * width + x) as usize];
    let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    let mut out = 0;
    for shift in [24, 16, 8, 0] {
        let part = |pixel: u32| (pixel >> shift & 0xFF) as f32;
        let top = part(a) * (1.0 - tx) + part(b) * tx;
        let bottom = part(c) * (1.0 - tx) + part(d) * tx;
        out |= ((top * (1.0 - ty) + bottom * ty).round() as u32).min(255) << shift;
    }
    out
}

/// A shape drawn onto the artboard as the editor draws it: and what it
/// covers made opaque, which the editor's drawing, on a screen, has no
/// need to say.
fn draw_shape(out: &mut [u32], wide: i32, high: i32, shape: &Shape, text: &mut Text) {
    if let Some(drop) = shape.shadow {
        draw_shape_shadow(out, wide, high, shape, drop, text);
    }
    let place = shape.bounds(text).inflate((shape.stroke * 3.0) as i32 + 8).intersect(Rect::new(0, 0, wide, high));
    if place.is_empty() {
        return;
    }
    let before: Vec<u32> = (place.y..place.bottom()).flat_map(|y| out[(y * wide + place.x) as usize..(y * wide + place.right()) as usize].to_vec()).collect();
    shape.draw(&mut Canvas::new(out, wide, high), text);
    for (at, &was) in before.iter().enumerate() {
        let (x, y) = (place.x + at as i32 % place.w, place.y + at as i32 / place.w);
        let pixel = &mut out[(y * wide + x) as usize];
        // Drawn on, it is opaque; and over nothing, its color is the
        // color drawn, and not that color mixed with black.
        *pixel = if *pixel & 0xFF_FFFF != was & 0xFF_FFFF { 0xFF00_0000 | (*pixel & 0xFF_FFFF) } else { was };
    }
}

/// The shadow a shape casts, under it: what it covers, drawn apart over
/// black and over white to say how much of each pixel that is, grown or
/// shrunk by the shadow's spread, blurred, moved, and laid down in the
/// shadow's color.
fn draw_shape_shadow(out: &mut [u32], wide: i32, high: i32, shape: &Shape, drop: frame::Layer, text: &mut Text) {
    let (blur, spread) = (f32::from(drop.blur.max(0)), f32::from(drop.spread));
    let reach = (blur * 1.5 + spread.max(0.0)).ceil() as i32 + 2;
    let body = shape.bounds(text).inflate((shape.stroke * 3.0) as i32 + 8);
    // Only as much of it as could fall on the picture, as far again round
    // that as it is blurred: zoomed far in, a little of a large shadow.
    let sheet = body.inflate(reach).intersect(Rect::new(-i32::from(drop.x), -i32::from(drop.y), wide, high).inflate(reach));
    let moved = Rect::new(sheet.x + i32::from(drop.x), sheet.y + i32::from(drop.y), sheet.w, sheet.h);
    if moved.intersect(Rect::new(0, 0, wide, high)).is_empty() || i64::from(sheet.w) * i64::from(sheet.h) > 16_000_000 {
        return;
    }
    let (w, h) = (sheet.w, sheet.h);
    let mut flat = shape.clone();
    (flat.shadow, flat.opacity) = (None, 256);
    flat.translate(-sheet.x as f32, -sheet.y as f32);
    let (mut dark, mut light) = (vec![0u32; (w * h) as usize], vec![0xFFFFFFu32; (w * h) as usize]);
    flat.draw(&mut Canvas::new(&mut dark, w, h), text);
    flat.draw(&mut Canvas::new(&mut light, w, h), text);
    let mut cover: Vec<f32> = dark.iter().zip(&light).map(|(&over, &under)| {
        let gone: u32 = [16, 8, 0].iter().map(|shift| (under >> shift & 255) - (over >> shift & 255).min(under >> shift & 255)).sum();
        1.0 - gone as f32 / (3.0 * 255.0)
    }).collect();
    let grow = spread.round() as i32;
    if grow != 0 {
        cover = spread_cover(&cover, w, h, grow);
    }
    let deviation = blur / 2.0;
    if deviation >= 0.5 {
        let radius = (deviation * 1.2).round().max(1.0) as usize;
        for _ in 0..3 {
            cover = box_blur(&cover, w as usize, h as usize, radius);
        }
    }
    let strength = (drop.color >> 24) as f32 / 255.0 * shape.opacity as f32 / 256.0;
    let color = drop.color & 0xFF_FFFF;
    let area = moved.intersect(Rect::new(0, 0, wide, high));
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let (sx, sy) = (x - moved.x, y - moved.y);
            let amount = cover[(sy * w + sx) as usize] * strength;
            if amount > 0.002 {
                let pixel = &mut out[(y * wide + x) as usize];
                *pixel = over(*pixel, color, amount);
            }
        }
    }
}

/// How much of each pixel is covered, grown by so many pixels all round,
/// or shrunk by as many if they are fewer than none: the most, or the
/// least, of what is near each.
fn spread_cover(cover: &[f32], w: i32, h: i32, by: i32) -> Vec<f32> {
    let reach = by.abs();
    let pick = |a: f32, b: f32| if by > 0 { a.max(b) } else { a.min(b) };
    let mut across = cover.to_vec();
    for y in 0..h {
        for x in 0..w {
            let mut best = cover[(y * w + x) as usize];
            for dx in (x - reach).max(0)..=(x + reach).min(w - 1) {
                best = pick(best, cover[(y * w + dx) as usize]);
            }
            across[(y * w + x) as usize] = best;
        }
    }
    let mut out = across.clone();
    for y in 0..h {
        for x in 0..w {
            let mut best = across[(y * w + x) as usize];
            for dy in (y - reach).max(0)..=(y + reach).min(h - 1) {
                best = pick(best, across[(dy * w + x) as usize]);
            }
            out[(y * w + x) as usize] = best;
        }
    }
    out
}

/// Each value the average of those within a radius of it, across and
/// then down: three times over, near enough a gaussian blur.
fn box_blur(values: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let pass = |from: &[f32], count: usize, stride: usize, lines: usize, step: usize| -> Vec<f32> {
        let mut out = vec![0.0; from.len()];
        let span = (2 * radius + 1) as f32;
        for line in 0..lines {
            let at = |i: isize| -> f32 {
                if i < 0 || i as usize >= count { 0.0 } else { from[line * step + i as usize * stride] }
            };
            let mut sum: f32 = (-(radius as isize)..=radius as isize).map(at).sum();
            for i in 0..count {
                out[line * step + i * stride] = sum / span;
                sum += at(i as isize + radius as isize + 1) - at(i as isize - radius as isize);
            }
        }
        out
    };
    let across = pass(values, w, 1, h, w);
    pass(&across, h, w, w, 1)
}

// ---- The canvas ----

/// What is being done with the button down.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    /// A new frame dragged out on the page, from a point of it.
    NewFrame { from: (f32, f32) },
    /// The frame worked in moved by its name, from a point of the window,
    /// from where it was.
    /// With `copy`, Alt held as it was pressed, a copy of it is what moves.
    MoveFrame { from: (i32, i32), origin: (f32, f32), moved: bool, copy: bool },
    None,
    /// The view moved, by the hand, the middle button or Space.
    Pan { from: (i32, i32), origin: (f32, f32) },
    /// What is picked out, moved: `moved` once it has gone anywhere; with
    /// `copy`, a copy of it, left where it was, as Alt does in Figma.
    Move { from: (f32, f32), moved: bool, copy: bool },
    /// The one picked out made larger or smaller by a handle, which pulls
    /// each way -1, 0 or 1.
    Scale { pull: (i32, i32), from: (f32, f32) },
    /// The one picked out turned about its middle.
    Turn { about: (f32, f32), began: f32 },
    /// A shape being drawn: the last of the layers.
    Draw { from: (f32, f32) },
    /// A box dragged out to pick out what it touches.
    Marquee { from: (f32, f32) },
    /// A number dragged along by its label, as Figma's are.
    Scrub { field: Field, from: i32, start: f32 },
    /// The crop of the picture being cropped, by a handle or, inside it,
    /// all of it.
    Crop { pull: (i32, i32), from: (f32, f32) },
    /// A slider dragged along, from where its track begins, so wide.
    Slide { field: Field, left: i32, wide: i32 },
    /// A guide dragged out of a ruler, or taken hold of.
    Guide { index: usize },
    /// A layer dragged up or down its list, to go over or under others.
    Reorder { index: usize, from: i32, moved: bool },
    /// A frame dragged up or down the list, to go over or under the rest
    /// of what is on the page.
    ReorderFrame { index: usize, from: i32, moved: bool },
    /// A part of the color picker dragged along.
    Picker { part: u8, rect: Rect },
    /// A corner of the box or picture picked out made rounder by its dot:
    /// the one corner with `single`, or all of them; taken hold of `slack`
    /// further in than its radius.
    Radius { corner: usize, single: bool, slack: f32 },
    /// A new arrow pulled out of a side of a box on a board, by its
    /// dot, from the box by its layer: `moved` once it has gone anywhere.
    Pull { from: usize, side: u8, moved: bool },
}

/// The last copy made, and what it was made from.
#[derive(Clone, PartialEq, Debug)]
enum Repeat {
    /// A frame of a page, by where each is among its frames.
    Frame { page: usize, source: usize, copy: usize },
    /// Layers of a frame, by what each is called.
    Layers { frame: (usize, usize), sources: Vec<u32>, copies: Vec<u32> },
}

/// A number in the panel of properties.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    X,
    Y,
    W,
    H,
    Turn,
    Radius,
    Opacity,
    ArtW,
    ArtH,
    /// Which way the background's gradient goes, as CSS says it: 0 up,
    /// 90 to the right, 180 down.
    Angle,
    /// How a picture is shown: lighter, more contrasted, more colorful;
    /// each a slider from -100 to 100.
    Exposure,
    Contrast,
    Saturation,
    /// A picture's shadow, number by number.
    DropY,
    DropBlur,
    DropSpread,
    DropAlpha,
    /// How large text is; how wide a line is.
    FontSize,
    Stroke,
    /// Where the ends of a line are.
    StartX,
    StartY,
    EndX,
    EndY,
    /// How many times larger the frame is exported, at one of its sizes.
    ExportTimes(u8),
    /// Where the frame is on its page; how long a line is, and which way
    /// it goes from its start.
    FrameX,
    FrameY,
    Length,
    LineAngle,
    /// How strong a blur is; how much a spotlight darkens the rest.
    Strength,
    Dim,
    /// How many times larger a zoom lens shows what it is of.
    Magnify,
    /// The radius of one corner of a box or a picture: top left, top
    /// right, bottom right, bottom left.
    Corner(u8),
    /// Which way the gradient a box or an oval is filled with goes.
    FillAngle,
    /// The gap between what is in the stack picked out, and the padding
    /// round it.
    StackGap,
    StackPad,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Field::X | Field::StartX | Field::EndX | Field::FrameX => "X",
            Field::Y | Field::StartY | Field::EndY | Field::FrameY => "Y",
            Field::W | Field::ArtW => "W",
            Field::H | Field::ArtH => "H",
            Field::Turn => "\u{b0}",
            Field::Radius => "Radius",
            Field::Opacity => "Opacity",
            Field::Angle => "Angle",
            Field::Exposure => "Exposure",
            Field::Contrast => "Contrast",
            Field::Saturation => "Saturation",
            Field::DropY => "Y",
            Field::DropBlur => "Blur",
            Field::DropSpread => "Spread",
            Field::DropAlpha => "%",
            Field::FontSize => "Size",
            Field::Stroke => "W",
            Field::Length => "Length",
            Field::LineAngle => "\u{b0}",
            Field::Strength => "Strength",
            Field::Dim => "Dim",
            Field::Magnify => "Larger",
            Field::ExportTimes(_) => "\u{d7}",
            Field::Corner(_) => "",
            Field::FillAngle => "Angle",
            Field::StackGap => "Gap",
            Field::StackPad => "Pad",
        }
    }

    /// Whether it is shown as a slider, rather than a number.
    fn slides(self) -> bool {
        matches!(self, Field::Exposure | Field::Contrast | Field::Saturation)
    }
}

/// What the background is: nothing, a color, a backdrop.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ground {
    None,
    Color,
    Backdrop,
    Gradient,
    Radial,
}

/// What a color in the panel is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Paint {
    Line,
    Fill,
    /// The color the gradient a box or an oval is filled with goes to; it
    /// goes from its `Fill`.
    FillTo,
    Background,
    /// The colors a gradient goes from and to.
    From,
    To,
    /// The color of a picture's shadow.
    Shadow,
    /// One color among what is picked out, to be another in all of them.
    Recolor(u32),
    /// What the stack picked out is filled with.
    Stack,
}

/// What a click on the panels, or the bar along the top, does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Control {
    Tool(Tool),
    Layer(usize),
    /// A layer of the page itself, outside the frames, listed while the
    /// frame worked in is one of them: clicked, the page is worked in.
    PageLayer(usize),
    /// A stack's row in the list of layers, by what it is called; and the
    /// caret on it that folds it.
    StackRow(u32),
    StackFold(u32),
    Eye(usize),
    Lock(usize),
    Field(Field),
    Ground(Ground),
    Backdrop(usize),
    Swatch(Paint, Option<u32>),
    Shadow(usize),
    Action(Action),
    /// The way back to the projects; the pages, and a new one; the frames
    /// of the page, in the list of layers; the frame tool; the list of a
    /// group of tools, and one of the tools in it; the zoom, which fits;
    /// and a color, which opens the colors.
    Home,
    Page(usize),
    NewPage,
    Frame(usize),
    FrameTool,
    Group(u8),
    Pick(Tool),
    Zoom,
    ColorField(Paint),
    /// A row of an open list, the list a level deep and the row in it; and
    /// a field that opens a list.
    MenuRow(u8, u16),
    Menu(MenuKind),
    /// One of the windows a picture can be set in, by where it is in
    /// `chrome::CHOICES`; whether it is light, dark or as the picture is,
    /// by where that is in `chromes::THEMES`; whether its own bar is
    /// swapped for it; and its shadow, the system's, none or the
    /// picture's own. A name to type, in a field; and what an end of a
    /// line is stuck to.
    Window(u8),
    WindowTheme(u8),
    WindowBar(bool),
    WindowShadow(u8),
    /// Which way the stack picked out goes, how large its box is, and how
    /// what is in it is lined up, one of nine.
    StackWay(u8),
    StackSizing(u8),
    StackAlign(u8),
    Entry(Naming),
    Stuck(u8),
    /// A part of the color picker: its square, its strips, its dropper.
    Picker(u8),
    /// The arrow that opens a color of the selection to its layers, and
    /// one of those layers, with what of it the color colors.
    ColorOpen(u32),
    /// A style of the grid of them, by where it is in it.
    Style(u8),
    /// A tab of the search, and a row of what it found.
    SearchTab(u8),
    Found(u16),
    /// A tab of the right panel; a frame of the page to export or not;
    /// and the window of what was written closed, or its folder opened.
    Tab(u8),
    ExportPick(usize),
    ExportDone(bool),
    ColorLayer(usize, &'static str),
    /// The strip a gradient is shown on, its stops at its ends.
    GradientBar,
    /// The same of the fill of the box or oval picked out.
    FillBar,
    /// The button that hides or shows a panel: the left one, or the right.
    SidePanel(u8),
    /// A shelf of the library; one of the labels and notes in it, by
    /// where it is in the editor's `LIBRARY`; and one of the emoji found.
    Shelf(u8),
    LibraryItem(u16),
    EmojiItem(u16),
    /// Something of a board's, or the switch between Design and Board.
    Board(board::Ctl),
    /// A part of a panel that is no control.
    Panel,
}

/// The buttons of the panels and the bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    Export,
    Copy,
    Capture,
    Crop,
    FlipAcross,
    FlipDown,
    Actual,
    Forward,
    Backward,
    Duplicate,
    Delete,
    /// Lining up what is picked out: left, middle, right, top, middle,
    /// bottom.
    Align(u8),
    Halo,
    /// A new frame round what is picked out, and them in it.
    FrameThem,
    /// Width and height kept to each other, in the fields.
    LockRatio,
    /// What is picked out spaced evenly, across or down.
    Distribute(u8),
    /// The heads at the ends of a line swapped; its path straight, curved
    /// or round a corner; an end let go of what it is stuck to.
    Swap,
    Path(u8),
    Unstick(u8),
    /// An export size more, or one fewer; the frame turned sideways.
    AddExport,
    RemoveExport(u8),
    Sideways,
    /// How text is set.
    Bold,
    Italic,
    Highlight,
    Bullets,
    Underline,
    Strike,
    Upper,
    Pill(bool),
    TextAlign(u8),
    /// A shadow under a picture, or none; and its adjustments undone.
    ShadowOn,
    ShadowOff,
    ResetAdjust,
    /// A fill for a box or an oval, or none; a border, or none.
    FillOn,
    FillOff,
    BorderOn,
    BorderOff,
    /// The shape a crop is kept to, and the crop undone.
    Aspect(u8),
    ResetCrop,
    /// The crop made tight to what is in the picture, and on in to the
    /// next lines in it each time; with Shift, back out.
    Tight,
    /// The frames of the page chosen in the Export tab written.
    ExportChosen,
    /// What is picked out set side by side with no gaps, across or down;
    /// made the width, the height, the size, the corners, the shadow or
    /// the whole look of the first picked; each lined up with the frame,
    /// six ways or in its middle; and turned by so many degrees.
    Pack(u8),
    Match(u8),
    ToFrame(u8),
    Turn(i16),
    /// An export size copied, under it.
    DuplicateExport(u8),
    /// A blur made a mosaic, or smooth; a spotlight round, or square.
    BlurStyle(u8),
    Round(bool),
    /// A radius for each corner shown in the panel, or one for them all.
    Corners,
    /// Fine-tune opened for an export row; and the frame exported and
    /// the file put on the clipboard, for programs that take files.
    FineTune(u8),
    CopyFile,
    /// A box or an oval filled in one color, a linear gradient or a radial
    /// one: 0, 1 or 2.
    FillKind(u8),
    /// What is picked out made a stack, laid out as one; and the stack
    /// picked out undone.
    AddStack,
    RemoveStack,
}

impl Action {
    /// The icon a square button of it shows, if it is one.
    fn icon(self) -> Option<&'static str> {
        Some(match self {
            Action::Align(0) => icons::ALIGN_LEFT,
            Action::Align(1) => icons::ALIGN_HCENTER,
            Action::Align(2) => icons::ALIGN_RIGHT,
            Action::Align(3) => icons::ALIGN_TOP,
            Action::Align(4) => icons::ALIGN_VCENTER,
            Action::Align(_) => icons::ALIGN_BOTTOM,
            Action::ToFrame(0) => icons::ALIGN_LEFT,
            Action::ToFrame(1) => icons::ALIGN_HCENTER,
            Action::ToFrame(2) => icons::ALIGN_RIGHT,
            Action::ToFrame(3) => icons::ALIGN_TOP,
            Action::ToFrame(4) => icons::ALIGN_VCENTER,
            Action::ToFrame(5) => icons::ALIGN_BOTTOM,
            Action::Distribute(0) => icons::DIST_H,
            Action::Distribute(_) => icons::DIST_V,
            Action::LockRatio => icons::RATIO,
            Action::FlipAcross | Action::FlipDown => icons::FLIP,
            Action::Crop => icons::CROP,
            Action::Swap => icons::SWAP,
            Action::Unstick(_) => icons::LINK,
            Action::RemoveExport(_) | Action::ShadowOff | Action::FillOff | Action::BorderOff | Action::RemoveStack => icons::MINUS,
            Action::AddExport | Action::ShadowOn | Action::FillOn | Action::BorderOn | Action::AddStack => icons::PLUS,
            Action::ResetAdjust => icons::UNDO,
            Action::Sideways => icons::ROTATE,
            Action::TextAlign(0) => icons::ALIGN_LEFT,
            Action::TextAlign(1) => icons::ALIGN_HCENTER,
            Action::TextAlign(_) => icons::ALIGN_RIGHT,
            _ => return None,
        })
    }

    fn says(self) -> &'static str {
        match self {
            Action::Export => "Export",
            Action::Copy => "Copy",
            Action::FineTune(_) => "Fine-tune\u{2026}",
            Action::CopyFile => "Copy as file",
            Action::Capture => "Capture",
            Action::Crop => "Crop",
            Action::FlipAcross => "Flip \u{2194}",
            Action::FlipDown => "Flip \u{2195}",
            Action::Actual => "1:1",
            Action::FrameThem => "Frame them",
            Action::LockRatio => "Keep proportions",
            Action::Distribute(0) => "Equal gaps \u{2194}",
            Action::Distribute(_) => "Equal gaps \u{2195}",
            Action::Swap => "Swap ends",
            Action::Path(0) => "Straight",
            Action::Path(1) => "Curved",
            Action::Path(_) => "Elbow",
            Action::Unstick(_) => "Let go",
            Action::AddExport => "Add a size",
            Action::RemoveExport(_) => "Remove",
            Action::Sideways => "Turn sideways",
            Action::Bold => "B",
            Action::Italic => "I",
            Action::Highlight => "M",
            Action::Bullets => "\u{2022}",
            Action::Underline => "U",
            Action::Strike => "S",
            Action::Upper => "AA",
            Action::Pill(true) => "Pill",
            Action::Pill(false) => "None",
            Action::TextAlign(0) => "Left",
            Action::TextAlign(1) => "Center",
            Action::TextAlign(_) => "Right",
            Action::ShadowOn => "Add a shadow",
            Action::ShadowOff => "No shadow",
            Action::ResetAdjust => "Undo adjustments",
            Action::FillOn => "Add a fill",
            Action::FillOff => "No fill",
            Action::BorderOn => "Add a border",
            Action::BorderOff => "No border",
            Action::Aspect(_) => "Aspect",
            Action::ResetCrop => "Reset",
            Action::Tight => "Tight",
            Action::ExportChosen => "Export",
            Action::Pack(0) => "Pack \u{2194}",
            Action::Pack(_) => "Pack \u{2195}",
            Action::Match(0) => "Width",
            Action::Match(1) => "Height",
            Action::Match(2) => "Size",
            Action::Match(3) => "Corners",
            Action::Match(4) => "Shadow",
            Action::Match(_) => "Whole look",
            Action::ToFrame(6) => "Center",
            Action::ToFrame(_) => "To the frame",
            Action::Turn(-90) => "\u{21ba} 90\u{b0}",
            Action::Turn(90) => "\u{21bb} 90\u{b0}",
            Action::Turn(_) => "180\u{b0}",
            Action::Forward => "Forward",
            Action::Backward => "Backward",
            Action::Duplicate => "Duplicate",
            Action::Delete => "Delete",
            Action::Align(0) => "\u{21e4}",
            Action::Align(1) => "\u{2194}",
            Action::Align(2) => "\u{21e5}",
            Action::Align(3) => "\u{2912}",
            Action::Align(4) => "\u{2195}",
            Action::Align(_) => "\u{2913}",
            Action::Halo => "White edge",
            Action::DuplicateExport(_) => "Duplicate",
            Action::BlurStyle(effects::SMOOTH) => "Smooth",
            Action::BlurStyle(_) => "Mosaic",
            Action::Round(true) => "Round",
            Action::Round(false) => "Square",
            Action::Corners => "Independent corners",
            Action::FillKind(0) => "Solid",
            Action::FillKind(1) => "Linear",
            Action::FillKind(_) => "Radial",
            Action::AddStack => "Add auto layout",
            Action::RemoveStack => "Remove auto layout",
        }
    }
}

/// Where everything is, for one frame.
#[derive(Default)]
struct Layout {
    view: Rect,
    controls: Vec<(Rect, Control)>,
    /// What is said where, which is no control: headings, labels, the
    /// names in the lists; at a size, in a color.
    words: Vec<((i32, i32), String, u32, u32)>,
    /// Icons, by where their middles are.
    icons: Vec<((i32, i32), &'static str, u32, u32)>,
    /// The lines between the sections of a panel.
    rules: Vec<Rect>,
    /// The panels floating over the page, the bar of tools along its
    /// bottom, the list a group of tools opens, and the colors.
    panels: Vec<Rect>,
    dock: Rect,
    flyout: Option<Rect>,
    popover: Option<Rect>,
    /// The lists open, and where each is.
    /// The grid of styles, while it is open.
    styles: Option<Rect>,
    /// The library, while it is open.
    library: Option<Rect>,
    /// The search over everything, while it is open.
    search: Option<Rect>,
    /// The bar over a picture being cropped.
    crop_bar: Option<Rect>,
    /// The window of what an export wrote.
    exported: Option<Rect>,
    menus: Vec<(Rect, MenuKind)>,
    /// A board's bar of styles, by what is picked out.
    board_bar: Option<Rect>,
}

/// The whole file as it was, to come back to with undo: its pages, the
/// page shown, and the frame worked in.
#[derive(Clone)]
struct Snapshot {
    pages: Vec<Page>,
    page: usize,
    active: usize,
    /// When it was, for a board's history to say.
    when: std::time::SystemTime,
}

/// What a name being typed is the name of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Naming {
    Frame,
    Page(usize),
    /// The title in the bar of a picture's window, by its layer; and what
    /// the file of one of the frame's export sizes ends in.
    Title(usize),
    Suffix(usize),
    /// The address in a picture's browser, by its layer.
    Address(usize),
    /// A picture's name, in the list of layers; and what the list of
    /// layers is narrowed to.
    Layer(usize),
    Filter,
    /// A color as hex, in the picker.
    Hex,
    /// A stack's name, in the list of layers, by what it is called.
    Stack(u32),
}

/// What the window has to do for the canvas, that the canvas cannot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Continue,
    /// The window is to go.
    Quit,
    /// What is on the clipboard is wanted: a picture, or failing that
    /// text, to be handed to `pasted`.
    Paste,
    /// A picture of the screen is wanted, from a capture with the window
    /// out of the way, to be handed to `captured`.
    Capture,
    /// Another file is wanted in place of this one, kept: `open_next`.
    Open,
    /// The projects are wanted, this file kept.
    Home,
}

/// The artboard, the layers, and everything about the window they are in.
pub struct Studio {
    /// The frame being worked in: taken out of its page, which holds
    /// nothing in its place until it is put back.
    doc: Doc,
    /// The pages of the file, and which of them is shown, and which of
    /// its frames is `doc`.
    pages: Vec<Page>,
    page: usize,
    active: usize,
    /// The frame tool in hand, to drag out a new frame with; and whether
    /// the frame worked in is itself what is picked out.
    framing: bool,
    frame_picked: bool,
    /// The group of tools whose list is open over the toolbar; the colors
    /// open beside the panel, and what they color; how far down the panel
    /// of properties is scrolled; and a name being typed, and for what.
    flyout: Option<u8>,
    palette: Option<Paint>,
    scroll: i32,
    renaming: Option<(Naming, String)>,
    /// The name being typed is all picked out, as it is when begun: the
    /// first key types over it, as Figma's do.
    name_picked: bool,
    /// Which tool of each group shows in the toolbar: the last one used.
    groups: [Tool; 2],
    /// The list open, if one is; how large the screen is, for the frame
    /// size of it; whether rulers are shown; whether width and height are
    /// kept to each other; layers copied; and the shape crops keep to.
    menu: Option<MenuState>,
    /// The row of the open list come to by the keys: the list a level
    /// deep, and the row in it.
    menu_row: Option<(u8, usize)>,
    pub screen: (i32, i32),
    rulers: bool,
    /// Whether the panel down the left, and the one down the right, are
    /// shown: each hidden and brought back by its own button, which stays
    /// where it is either way.
    left_open: bool,
    right_open: bool,
    /// Everything but the page hidden, as Figma's Ctrl+\ hides it: the
    /// panels, the bar of tools and the rulers, all but the button that
    /// brings them back.
    zen: bool,
    keep_ratio: bool,
    copied: Vec<Layer>,
    crop_aspect: usize,
    /// The color being picked, as hue, saturation and value; and whether
    /// the next click takes a color off the page.
    hsv: (f32, f32, f32),
    eyedropping: bool,
    text: Text,
    width: i32,
    height: i32,
    zoom: f32,
    /// Where the artboard's top left corner is, in the view.
    pan: (f32, f32),
    tool: Tool,
    selected: Vec<usize>,
    hovered: Option<usize>,
    /// One of the page's own layers under the pointer, while a frame is
    /// worked in.
    hovered_page: Option<usize>,
    /// The control the pointer was last over, as drawn.
    hover_shown: Option<Control>,
    drag: Drag,
    pointer: (i32, i32),
    mods: Mods,
    /// Space held: the hand, for as long as it is.
    space: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The layers as they were when something was taken hold of, to
    /// change from rather than from what the last move left.
    held: Vec<Layer>,
    /// What a press inside the box of what is picked out was on, beneath
    /// it: picked out instead if the press is let go without a move.
    under_press: Option<usize>,
    /// The text being typed into, by its layer.
    editing: Option<usize>,
    /// The picture being cropped, by its layer.
    cropping: Option<usize>,
    /// The lines found in the picture last cropped, known by its picture:
    /// found once for each, as cropping it begins.
    crop_edges: Option<(std::sync::Weak<Image>, Arc<edges::Lines>)>,
    /// The lines of its picture a crop being pulled has snapped to, in the
    /// picture's own pixels: up and down at an x, or across at a y.
    crop_snaps: Vec<(bool, f32)>,
    /// Which of the tight crops was last gone to, and the crop it made.
    tight: Option<(usize, (f32, f32, f32, f32))>,
    /// How the next shape is drawn.
    color: u32,
    fill: Option<u32>,
    /// The gradient the next box or oval is filled with, from its `fill`;
    /// and whether it has a border, in `color`, as well.
    gradient: Option<shapes::Gradient>,
    border: bool,
    size: usize,
    /// How round the corners of the next rectangle are.
    radius: f32,
    face: usize,
    /// A number being typed into its field, and what has been typed.
    typing: Option<(Field, String)>,
    /// How many changes there have been, which says when what is shown
    /// is behind.
    version: u64,
    /// The part of the artboard drawn last, at the size it is shown: at
    /// which change, at which zoom, and which part.
    /// Each frame of the page as drawn last, at the size it is shown: at
    /// which change to it, at which zoom, and which part of it.
    shown: Vec<Option<drawn::Drawn>>,
    /// The same of the page's own layers, outside its frames.
    shown_page: Vec<Option<drawn::Drawn>>,
    /// The layers of the frame worked in that alone have changed since
    /// the change it was at, up to the change it is at now, if that is
    /// all that has: where they are is all that needs drawing again.
    touched: Option<(u64, u64, Vec<usize>)>,
    /// When the page was last zoomed or moved by the wheel or the hand,
    /// while it is shown from what was drawn before rather than drawn
    /// sharp: until it has been still a moment.
    moving: Option<std::time::Instant>,
    /// Where it is kept, if it is; and whether it has changed since.
    pub path: Option<PathBuf>,
    edited: bool,
    /// Where pictures go, and what they are called.
    dir: PathBuf,
    filename: String,
    /// How each kind of file is written, by the config file's [convert].
    pub convert: crate::convert::Settings,
    /// What the frame comes to in each kind of file, tried on a thread.
    sizes: sizes::Sizes,
    /// Fine-tune, open for an export row.
    fine: Option<fine::Fine>,
    /// What was last done, said along the bottom for a while.
    note: Option<(String, std::time::Instant)>,
    last_press: (u32, (i32, i32)),
    /// The lines what is being moved has snapped to, on the artboard:
    /// up and down at an x, or across at a y.
    guides: Vec<(bool, f32)>,
    /// What is shown as if it were done, while the pointer rests on it.
    preview: Option<preview::Preview>,
    /// The gaps what is being moved is spaced by, shown.
    gaps: Vec<snap::Gap>,
    /// The last copy made and what it was made from, for the next copy of
    /// it to go as far from it again, as Figma's do: until anything but
    /// moving that copy is done.
    repeat: Option<Repeat>,
    /// Whether Shift+1 last fitted the frame, to fit what is picked out next.
    fitted_frame: bool,
    /// When Shift and Alt were last seen held: out of a drag they hold a
    /// moment after they are let go.
    seen_mods: (Option<std::time::Instant>, Option<std::time::Instant>),
    /// The guides dragged out of the rulers: up and down at an x, or
    /// across at a y, in pixels of the frame.
    user_guides: Vec<(bool, f32)>,
    /// What the list of layers is narrowed to.
    layer_filter: String,
    /// Which tab of the right panel is shown: Design, or Export; the frames
    /// of the page left out of its export; and what the last export wrote.
    panel_tab: u8,
    export_off: Vec<usize>,
    exported: Option<export::Exported>,
    /// The color of the selection opened to its layers, if one is.
    color_open: Option<u32>,
    /// Whether the grid of styles is open.
    styling: bool,
    /// Whether the panel shows a radius for each corner, not one for all.
    corners_open: bool,
    /// How far what is being turned has gone round; and the pointer for
    /// where it is and what it is doing, worked out as it moves.
    turned_by: f32,
    pointer_cursor: Option<Cursor>,
    /// The library, while it is open; and whether what a label put down
    /// from it says is all picked out, for the first letter to type over.
    library: Option<library::Library>,
    words_picked: bool,
    /// The search over everything, while it is open; and a file it chose
    /// to be opened in place of this one.
    search: Option<search::Search>,
    pub open_next: Option<PathBuf>,
    /// The sizes of frames of the user's own, each with what it measures.
    frame_sizes: Vec<(String, i32, i32, &'static str)>,
    /// Which change it was at when the button went down, to say whether a
    /// drag changed anything.
    pressed_at: u64,
    /// Where the pill saying how large what is picked out is was drawn
    /// last, for what is said along the bottom to keep off it.
    pill: Option<Rect>,
    /// The window of the picture being cropped, by what the picture is
    /// called: off it while it is cropped, and put back after.
    crop_chrome: Option<(u32, Chrome)>,
    /// The stack the pointer is over, by what it is called; and whether
    /// laying the stacks out moved anything the last time anything changed.
    stack_hovered: Option<u32>,
    stacks_moved: bool,
    /// The stacks whose rows are folded in the list of layers; and what
    /// was picked out as what is in a stack, each of it, rather than the
    /// stack, while it still is what is picked out.
    folded: Vec<u32>,
    members_picked: Option<Vec<usize>>,
    /// What was done lately, for Ctrl+K to list first and Ctrl+Shift+K to
    /// do again; and the file it is kept in, which a test has none of.
    commands: Vec<recent::Done>,
    commands_file: Option<PathBuf>,
    dirty: bool,
    /// What a board has of its own: how the next box is drawn, what it
    /// guesses comes next, what is lit, and the rest.
    board: board::Board,
}

impl Studio {
    #[cfg(test)]
    pub fn new(doc: Doc, text: Text, width: i32, height: i32, dir: PathBuf, filename: String) -> Studio {
        Studio::with_pages(vec![Page::new("Page 1", vec![doc])], text, width, height, dir, filename)
    }

    /// The canvas, with a file's pages, at the first frame of the first.
    pub fn with_pages(pages: Vec<Page>, text: Text, width: i32, height: i32, dir: PathBuf, filename: String) -> Studio {
        let mut studio = Studio {
            doc: Doc::new(1, 1),
            text,
            width,
            height,
            zoom: 1.0,
            pan: (0.0, 0.0),
            tool: Tool::Select,
            selected: Vec::new(),
            hovered: None,
            hovered_page: None,
            hover_shown: None,
            drag: Drag::None,
            pointer: (0, 0),
            mods: Mods::default(),
            space: false,
            undo: Vec::new(),
            redo: Vec::new(),
            held: Vec::new(),
            under_press: None,
            editing: None,
            cropping: None,
            crop_edges: None,
            crop_snaps: Vec::new(),
            tight: None,
            color: 0xE5484D,
            // Filled with no border, as Figma draws a box; capture mode
            // outlines one instead.
            fill: Some(fills::DEFAULT_FILL),
            gradient: None,
            border: false,
            size: 1,
            radius: 8.0,
            face: 0,
            typing: None,
            version: 0,
            shown: Vec::new(),
            shown_page: Vec::new(),
            moving: None,
            touched: None,
            pages: if pages.is_empty() { vec![Page::new("Page 1", vec![Doc::new(1200, 675)])] } else { pages },
            page: 0,
            active: 0,
            framing: false,
            frame_picked: false,
            flyout: None,
            palette: None,
            scroll: 0,
            renaming: None,
            name_picked: false,
            groups: [Tool::Rect, Tool::Pen],
            menu: None,
            menu_row: None,
            screen: (1920, 1080),
            rulers: false,
            left_open: true,
            right_open: true,
            zen: false,
            keep_ratio: false,
            copied: Vec::new(),
            crop_aspect: 0,
            hsv: (0.0, 0.0, 1.0),
            eyedropping: false,
            path: None,
            edited: false,
            dir,
            filename,
            convert: crate::convert::Settings::default(),
            sizes: sizes::Sizes::default(),
            fine: None,
            note: None,
            last_press: (0, (-100, -100)),
            guides: Vec::new(),
            preview: None,
            gaps: Vec::new(),
            repeat: None,
            fitted_frame: false,
            seen_mods: (None, None),
            user_guides: Vec::new(),
            layer_filter: String::new(),
            panel_tab: 0,
            export_off: Vec::new(),
            exported: None,
            color_open: None,
            styling: false,
            corners_open: false,
            turned_by: 0.0,
            pointer_cursor: None,
            library: None,
            words_picked: false,
            search: None,
            open_next: None,
            frame_sizes: Vec::new(),
            pressed_at: 0,
            pill: None,
            crop_chrome: None,
            stack_hovered: None,
            stacks_moved: false,
            folded: Vec::new(),
            members_picked: None,
            commands: Vec::new(),
            commands_file: None,
            dirty: true,
            board: board::Board::new(false),
        };
        // A board with no frames is worked in on its page.
        studio.active = if studio.pages[0].frames.is_empty() { LOOSE } else { 0 };
        studio.doc = std::mem::replace(slot(&mut studio.pages[0], studio.active), Doc::new(1, 1));
        studio.board = board::Board::new(studio.pages[0].board);
        studio.fit();
        studio
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        if (width, height) != (self.width, self.height) {
            let before = self.layout().view;
            (self.width, self.height) = (width.max(place::MIN.0), height.max(place::MIN.1));
            // The artboard stays where it was in the middle of the view.
            let after = self.layout().view;
            self.pan = (self.pan.0 + (after.w - before.w) as f32 / 2.0, self.pan.1 + (after.h - before.h) as f32 / 2.0);
            self.dirty = true;
        }
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// How long what was last said shows for: briefly with everything
    /// hidden, where it is mostly the tool taken up.
    fn note_lasts(&self) -> f32 {
        if self.zen { 1.4 } else { 3.0 }
    }

    /// How long what was last said has left to show, if it shows; gone,
    /// and drawn again without it, once its time is up.
    pub fn note_left(&mut self) -> Option<std::time::Duration> {
        let (_, when) = self.note.as_ref()?;
        match std::time::Duration::from_secs_f32(self.note_lasts()).checked_sub(when.elapsed()) {
            Some(left) => Some(left),
            None => {
                self.note = None;
                self.dirty = true;
                None
            }
        }
    }

    /// What the window is called: the file, and whether it has changed.
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|path| path.file_stem()).map_or("Untitled".to_owned(), |name| name.to_string_lossy().into_owned());
        format!("{name}{} \u{2014} hotshot", if self.edited { " \u{2022}" } else { "" })
    }

    /// A name begun to be typed, all of it picked out for the first key to
    /// type over.
    fn start_naming(&mut self, naming: Naming, now: String) {
        self.name_picked = !now.is_empty();
        self.renaming = Some((naming, now));
    }

    /// A name being typed as it is shown: with a caret after it; or, all
    /// of it picked out, on the color of a selection.
    fn typed_shown(&mut self, canvas: &mut Canvas, (x, y): (i32, i32), px: u32, typed: String) -> String {
        if !self.name_picked {
            return format!("{typed}|");
        }
        let wide = self.text.width(&typed, px);
        canvas.round_fill(x - 2, y, wide + 4, Text::line_height(px), 3, ACCENT, 120);
        typed
    }

    fn say(&mut self, what: impl Into<String>) {
        let what = what.into();
        debug!("canvas says: {what}");
        self.note = Some((what, std::time::Instant::now()));
        self.dirty = true;
    }

    /// Something about to change: the whole file kept, to come back to
    /// with undo.
    /// Anything done but moving the last copy made forgets it.
    fn checkpoint(&mut self) {
        let now = self.snapshot();
        self.undo.push(now);
        self.redo.clear();
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.repeat = None;
    }

    /// The same, for what is picked out about to be moved: the last copy
    /// made, if that is what it is, still remembered.
    fn checkpoint_move(&mut self) {
        let kept = self.repeat.take().filter(|repeat| self.is_last_copy(repeat));
        self.checkpoint();
        self.repeat = kept;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { pages: self.gathered(), page: self.page, active: self.active, when: std::time::SystemTime::now() }
    }

    /// The pages, with the frame worked in back in its place.
    pub fn gathered(&self) -> Vec<Page> {
        let mut pages = self.pages.clone();
        *slot(&mut pages[self.page], self.active) = self.doc.clone();
        self.put_back_crop_window(&mut pages);
        pages
    }

    /// The file as it was at a snapshot, the frame worked in taken out.
    fn restore(&mut self, snapshot: Snapshot) {
        self.pages = snapshot.pages;
        self.page = snapshot.page.min(self.pages.len() - 1);
        self.active = match snapshot.active {
            LOOSE => LOOSE,
            _ if self.pages[self.page].frames.is_empty() => LOOSE,
            active => active.min(self.pages[self.page].frames.len() - 1),
        };
        self.doc = std::mem::replace(slot(&mut self.pages[self.page], self.active), Doc::new(1, 1));
        self.selected.retain(|&index| index < self.doc.layers.len());
        (self.cropping, self.editing, self.repeat) = (None, None, None);
        self.frame_picked &= self.active != LOOSE;
        self.shown.clear();
        self.shown_page.clear();
    }

    /// Whether what is worked in is the page's own layers, outside its
    /// frames, rather than a frame.
    fn on_page(&self) -> bool {
        self.active == LOOSE
    }

    /// Another frame of the page worked in, or at `LOOSE` the page's own
    /// layers, the one before put back.
    fn activate(&mut self, frame: usize) {
        if frame == self.active || (frame >= self.pages[self.page].frames.len() && frame != LOOSE) {
            return;
        }
        // A name being typed goes to what it names before that is put away.
        self.commit_name();
        self.commit_text();
        let page = &mut self.pages[self.page];
        std::mem::swap(&mut self.doc, slot(page, self.active));
        std::mem::swap(&mut self.doc, slot(page, frame));
        self.active = frame;
        self.selected.clear();
        (self.cropping, self.editing, self.frame_picked) = (None, None, false);
        self.dirty = true;
    }

    /// Another page shown, at its first frame.
    fn go_to_page(&mut self, page: usize) {
        if page == self.page || page >= self.pages.len() {
            return;
        }
        self.commit_name();
        self.commit_text();
        let back = std::mem::replace(&mut self.doc, Doc::new(1, 1));
        *slot(&mut self.pages[self.page], self.active) = back;
        (self.page, self.active) = (page, if self.pages[page].frames.is_empty() { LOOSE } else { 0 });
        self.doc = std::mem::replace(slot(&mut self.pages[page], self.active), Doc::new(1, 1));
        self.board_page_shown();
        self.selected.clear();
        (self.cropping, self.editing, self.frame_picked) = (None, None, false);
        self.shown.clear();
        self.shown_page.clear();
        self.fit();
    }

    /// The frames of the page shown, the one worked in among them; and
    /// at `LOOSE`, the page's own layers.
    fn frame(&self, index: usize) -> &Doc {
        match index == self.active {
            true => &self.doc,
            false if index == LOOSE => &self.pages[self.page].loose,
            false => &self.pages[self.page].frames[index],
        }
    }

    /// The page's own layers, outside its frames, whether they are worked
    /// in or not.
    fn loose(&self) -> &Doc {
        self.frame(LOOSE)
    }

    fn frame_count(&self) -> usize {
        self.pages[self.page].frames.len()
    }

    /// The frame worked in, or with the page's own layers worked in, the
    /// last of its frames: what a new frame or page is made like.
    fn frame_like(&self) -> &Doc {
        match self.on_page() {
            true => self.pages[self.page].frames.last().unwrap_or(&self.doc),
            false => &self.doc,
        }
    }

    /// A new page, with a frame on it the size of the one worked in, white
    /// whatever that one is on.
    fn new_page(&mut self) {
        self.checkpoint();
        // A page of the same kind as the one shown: a board, with nothing
        // on it, or a page with a frame as large as the one worked in.
        let board = self.on_board();
        let (width, height) = match self.frame_count() {
            0 => (1200, 675),
            _ => (self.frame_like().width, self.frame_like().height),
        };
        let name = format!("{} {}", if board { "Board" } else { "Page" }, self.pages.len() + 1);
        let mut page = Page::new(name, if board { Vec::new() } else { vec![Doc::new(width, height)] });
        page.board = board;
        self.pages.push(page);
        self.go_to_page(self.pages.len() - 1);
        self.renaming = Some((Naming::Page(self.page), String::new()));
        self.changed();
    }

    /// A new frame on the page, where it was dragged out, worked in.
    fn new_frame(&mut self, at: (f32, f32), size: (f32, f32)) {
        self.checkpoint();
        let mut frame = Doc::new((size.0.round() as i32).max(16), (size.1.round() as i32).max(16));
        (frame.x, frame.y) = (at.0.round(), at.1.round());
        frame.name = format!("Frame {}", self.frame_count() + 1);
        self.pages[self.page].frames.push(frame);
        let index = self.frame_count() - 1;
        self.place_new_frame(index, None);
        self.activate(index);
        self.frame_picked = true;
        self.changed();
    }

    /// The frame worked in, gone, where there are others on the page.
    fn delete_frame(&mut self) {
        if self.on_page() {
            return self.say("Pick a frame to delete it");
        }
        if self.frame_count() < 2 && !self.on_board() {
            return self.say("A page keeps one frame");
        }
        self.checkpoint();
        let gone = self.active;
        let next = match gone {
            // A board's last frame gone, its page is worked in.
            _ if self.frame_count() < 2 => LOOSE,
            0 => 1,
            _ => gone - 1,
        };
        self.activate(next);
        self.pages[self.page].frames.remove(gone);
        if self.active > gone && self.active != LOOSE {
            self.active -= 1;
        }
        self.shown.clear();
        self.changed();
    }

    /// Which frame of the page a point of it is in, the topmost.
    fn frame_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        self.page_order().into_iter().rev().find_map(|child| match child {
            Child::Frame(index) => {
                let frame = self.frame(index);
                (x >= frame.x && y >= frame.y && x < frame.x + frame.width as f32 && y < frame.y + frame.height as f32).then_some(index)
            }
            Child::Layer(_) => None,
        })
    }

    /// Which of the page's own layers a point of the page is on, the
    /// topmost, whether they are worked in or not; none where a frame is
    /// over it, as everything on the page is hit in the order it is drawn.
    fn page_layer_at(&mut self, at: (f32, f32)) -> Option<usize> {
        match self.top_at(at) {
            Some(Child::Layer(index)) => Some(index),
            _ => None,
        }
    }

    /// Whether a point of the window is on what is picked out, its box and
    /// a little round it, where its handles are and it is turned from.
    fn near_selection(&mut self, at: (i32, i32)) -> bool {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return false;
        }
        let corners: Vec<(i32, i32)> = chosen.into_iter().flat_map(|index| self.box_corners(index)).collect();
        let (left, right) = (corners.iter().map(|c| c.0).min().unwrap_or(0), corners.iter().map(|c| c.0).max().unwrap_or(0));
        let (top, bottom) = (corners.iter().map(|c| c.1).min().unwrap_or(0), corners.iter().map(|c| c.1).max().unwrap_or(0));
        Rect::new(left, top, right - left + 1, bottom - top + 1).inflate(16).contains(at.0, at.1)
    }

    /// What a press on the page is to work in, if not what is worked in
    /// now: whatever is topmost under it as the page is drawn, a layer of
    /// the page's own or the frame it is in; or outside everything, the
    /// page's own layers, as a shape drawn there is one of them, as in
    /// Figma. Nothing while a picture is cropped, or on or by what is
    /// picked out.
    fn press_target(&mut self, at: (i32, i32)) -> Option<usize> {
        // On a board, everything is the page's, frames or not.
        if self.on_board() && self.cropping.is_none() {
            return (!self.on_page()).then_some(LOOSE);
        }
        if self.cropping.is_some() || self.near_selection(at) {
            return None;
        }
        match self.top_at(self.to_page(at)) {
            Some(Child::Frame(frame)) => (frame != self.active).then_some(frame),
            Some(Child::Layer(_)) | None => (!self.on_page()).then_some(LOOSE),
        }
    }

    /// Layers of what is worked in moved into a frame, or onto the page,
    /// where they are on the page kept, and on top there; that worked in,
    /// with them picked out. What a layer dragged into a frame, or out of
    /// one, is let go into.
    fn move_layers_to(&mut self, target: usize, chosen: &[usize]) {
        if target == self.active || chosen.is_empty() {
            return;
        }
        let (dx, dy) = (self.doc.x - self.frame(target).x, self.doc.y - self.frame(target).y);
        let mut sorted: Vec<usize> = chosen.iter().copied().filter(|&index| index < self.doc.layers.len()).collect();
        sorted.sort_unstable();
        sorted.dedup();
        let mut moving: Vec<Layer> = sorted.iter().rev().map(|&index| self.doc.layers.remove(index)).collect();
        moving.reverse();
        for layer in &mut moving {
            layer.translate(dx, dy);
        }
        self.selected.clear();
        self.changed();
        self.activate(target);
        let start = self.doc.layers.len();
        self.doc.layers.extend(moving);
        self.selected = (start..self.doc.layers.len()).collect();
        self.changed();
    }

    /// What was dragged on the page let go: into the frame the pointer is
    /// over, or onto the page outside them all, if that is not where it
    /// was, as Figma moves a layer let go over a frame into it.
    fn drop_into_place(&mut self) {
        // A board's frames are only what of it is exported: what is moved
        // on a board stays the page's.
        if self.on_board() {
            return;
        }
        let point = self.to_page(self.pointer);
        let target = self.frame_at(point).unwrap_or(LOOSE);
        let chosen = self.chosen();
        self.move_layers_to(target, &chosen);
    }

    /// Something has changed: drawn again, and the file is behind.
    fn changed(&mut self) {
        self.touched = None;
        // The frames kept over what they were over of the page's own
        // layers, as those come and go.
        self.keep_order();
        // The stacks laid out again, before what is stuck to what is in
        // them follows it.
        self.stacks_moved = self.reflow();
        // On a board, a box holding others grown round them.
        if self.on_board() {
            self.settle_nests();
        }
        self.follow();
        self.version += 1;
        self.doc.stamp = self.version;
        self.edited = true;
        self.dirty = true;
    }

    // ---- The view ----

    fn layout(&mut self) -> Layout {
        let view = Rect::new(0, 0, self.width, self.height);
        let mut layout = Layout { view, ..Layout::default() };
        // Presenting, the board and nothing else.
        if self.presenting() {
            return layout;
        }
        self.toolbar(&mut layout);
        self.left_panel(&mut layout);
        self.right_panel(&mut layout);
        // The frames' names on the page, to pick a frame by.
        for (index, place, _) in self.frame_labels() {
            if self.over_page(place.center()) {
                layout.controls.push((place, Control::Frame(index)));
            }
        }
        self.layout_crop_bar(&mut layout);
        self.layout_exported(&mut layout);
        self.layout_styles(&mut layout);
        self.layout_library(&mut layout);
        self.layout_search(&mut layout);
        if self.on_board() {
            self.layout_style_bar(&mut layout);
            self.layout_history(&mut layout);
        }
        self.layout_menus(&mut layout);
        layout
    }

    /// The tools, floating along the bottom of the page, grouped as
    /// Figma's are: a caret beside a group opens a list of the rest.
    fn toolbar(&mut self, layout: &mut Layout) {
        if self.zen {
            return;
        }
        if self.on_board() {
            return self.board_toolbar(layout);
        }
        let (side, caret, gap, pad) = (38, 14, 2, 5);
        let items: Vec<(Control, i32)> = vec![
            (Control::Tool(Tool::Select), side),
            (Control::FrameTool, side),
            (Control::Tool(self.groups[0]), side),
            (Control::Group(0), caret),
            (Control::Tool(Tool::Arrow), side),
            (Control::Tool(self.groups[1]), side),
            (Control::Group(1), caret),
            (Control::Tool(Tool::Text), side),
            (Control::Tool(Tool::Counter), side),
            (Control::Group(MORE), side),
            (Control::Panel, 9),
            (Control::Action(Action::Capture), side),
        ];
        let wide = items.iter().map(|(_, wide)| wide + gap).sum::<i32>() - gap + 2 * pad;
        let open = self.open_area();
        let dock = Rect::new(open.x + (open.w - wide) / 2, self.height - 20 - side - 2 * pad, wide, side + 2 * pad);
        let mut x = dock.x + pad;
        let mut group_at = [0; 3];
        for (control, w) in items {
            // A group's list over the tool before its caret; the More
            // tools list over its own button.
            if let Control::Group(group) = control {
                group_at[usize::from(group)] = if group == MORE { x } else { x - side - gap };
            }
            if control != Control::Panel {
                layout.controls.push((Rect::new(x, dock.y + pad, w, side), control));
            }
            x += w + gap;
        }
        // The list a group opens, over its button.
        if let Some(group) = self.flyout {
            let tools: &[Tool] = match group {
                0 => &[Tool::Rect, Tool::Ellipse, Tool::Line],
                MORE => &MORE_TOOLS,
                _ => &[Tool::Pen, Tool::Marker],
            };
            let (row, w) = (30, 190);
            let list = Rect::new(group_at[usize::from(group)].min(self.width - INSET - w), dock.y - 8 - tools.len() as i32 * row - 10, w, tools.len() as i32 * row + 10);
            for (at, &tool) in tools.iter().enumerate() {
                layout.controls.push((Rect::new(list.x + 5, list.y + 5 + at as i32 * row, w - 10, row), Control::Pick(tool)));
            }
            layout.controls.push((list, Control::Panel));
            layout.flyout = Some(list);
        }
        layout.controls.push((dock, Control::Panel));
        layout.dock = dock;
    }

    /// The panel down the left: the file, its pages, and the layers of the
    /// page shown, by frame.
    fn left_panel(&mut self, layout: &mut Layout) {
        // Its button first, where it is whether the panel shows or not;
        // hidden, the panel is that button and nothing else.
        layout.controls.push((self.side_button(0), Control::SidePanel(0)));
        self.layout_switch(layout);
        if !self.shows_left() {
            return;
        }
        let panel = Rect::new(INSET, INSET, LEFT_W, self.height - 2 * INSET);
        layout.panels.push(panel);
        let (left, inner) = (panel.x + 12, panel.w - 24);
        let line = Text::line_height(12);
        // The file: the way home, its project, and its name.
        // Under the switch between Design and Board, which is beside the
        // button that hides the panel.
        let home = self.side_button(0).x;
        layout.controls.push((Rect::new(home, panel.y + 12 + SWITCH_ROOM, 28, 28), Control::Home));
        let project = self.path.as_ref().and_then(|path| path.parent()).and_then(|dir| dir.file_name()).map_or("Drafts".to_owned(), |name| name.to_string_lossy().into_owned());
        layout.words.push(((home + 38, panel.y + 10 + SWITCH_ROOM), project, DIM_TEXT_COLOR, 11));
        let name = self.title().trim_end_matches(" \u{2014} hotshot").to_owned();
        layout.words.push(((home + 38, panel.y + 25 + SWITCH_ROOM), name, TEXT_COLOR, 13));
        // The pages.
        let mut y = panel.y + 56 + SWITCH_ROOM;
        layout.rules.push(Rect::new(panel.x, y, panel.w, 1));
        y += 10;
        layout.words.push(((left, y + (22 - line) / 2), "Pages".to_owned(), TEXT_COLOR, 12));
        layout.controls.push((Rect::new(left + inner - 22, y, 22, 22), Control::NewPage));
        y += 28;
        let row = 28;
        for index in 0..self.pages.len() {
            layout.controls.push((Rect::new(left - 4, y, inner + 8, row), Control::Page(index)));
            y += row;
        }
        // The layers of the page: each frame, and the layers of the one
        // worked in, the topmost first.
        y += 8;
        layout.rules.push(Rect::new(panel.x, y, panel.w, 1));
        y += 10;
        layout.words.push(((left, y + (22 - line) / 2), "Layers".to_owned(), TEXT_COLOR, 12));
        layout.controls.push((Rect::new(left + 60, y, inner - 60, 22), Control::Entry(Naming::Filter)));
        let filter = match &self.renaming {
            Some((Naming::Filter, typed)) => typed.to_lowercase(),
            _ => self.layer_filter.to_lowercase(),
        };
        y += 28;
        // Everything on the page in the order it is drawn, the topmost
        // first, frames and the page's own layers among each other as in
        // Figma: those worked in as a frame's layers are, and otherwise
        // each a row that works in them when clicked. A stack is a row of
        // its own, what is in it set in under it, where the first of it
        // comes; narrowed by what is typed, the layers alone are listed.
        let on_page = self.on_page();
        let flat = !filter.trim().is_empty();
        let matches = |layer: &Layer| layer.name().to_lowercase().contains(filter.trim());
        // Each row: the frame it is, or a row of the page's own layers or of
        // the frame worked in's.
        let mut listed: Vec<(Option<usize>, stack_rows::Row, bool)> = Vec::new();
        let mut shown = Vec::new();
        for child in self.page_order().into_iter().rev() {
            match child {
                Child::Layer(index) => {
                    let loose = self.loose();
                    let mut rows = Vec::new();
                    match flat {
                        true if matches(&loose.layers[index]) => rows.push(stack_rows::Row::Layer(index, 0)),
                        true => {}
                        false => stack_rows::rows_for(loose, index, &mut shown, &self.folded, &mut rows),
                    }
                    listed.extend(rows.into_iter().map(|row| (None, row, true)));
                }
                Child::Frame(frame) => {
                    listed.push((Some(frame), stack_rows::Row::Layer(0, 0), false));
                    if frame != self.active {
                        continue;
                    }
                    let mut rows = Vec::new();
                    let mut shown = Vec::new();
                    for index in (0..self.doc.layers.len()).rev() {
                        match flat {
                            true if matches(&self.doc.layers[index]) => rows.push(stack_rows::Row::Layer(index, 0)),
                            true => {}
                            false => stack_rows::rows_for(&self.doc, index, &mut shown, &self.folded, &mut rows),
                        }
                    }
                    listed.extend(rows.into_iter().map(|row| (None, row, false)));
                }
            }
        }
        for (frame, row_of, of_page) in listed {
            if y + row > panel.bottom() - 8 {
                break;
            }
            if let Some(frame) = frame {
                layout.controls.push((Rect::new(left - 4, y, inner + 8, row), Control::Frame(frame)));
                y += row;
                continue;
            }
            let (x0, w0) = if of_page { (left - 4, inner + 8) } else { (left + 14, inner - 10) };
            match row_of {
                stack_rows::Row::Layer(index, depth) => {
                    let place = Rect::new(x0 + 14 * depth as i32, y, w0 - 14 * depth as i32, row);
                    match !of_page || on_page {
                        true => {
                            layout.controls.push((Rect::new(left + inner - 44, y, 22, row), Control::Eye(index)));
                            layout.controls.push((Rect::new(left + inner - 20, y, 22, row), Control::Lock(index)));
                            layout.controls.push((place, Control::Layer(index)));
                        }
                        false => layout.controls.push((place, Control::PageLayer(index))),
                    }
                }
                stack_rows::Row::Stack(id, depth) => {
                    let place = Rect::new(x0 + 14 * depth as i32, y, w0 - 14 * depth as i32, row);
                    layout.controls.push((Rect::new(place.x, y, 18, row), Control::StackFold(id)));
                    layout.controls.push((place, Control::StackRow(id)));
                }
            }
            y += row;
        }
        layout.controls.push((panel, Control::Panel));
    }

    /// The panel down the right: the zoom and the way out at its top, and
    /// under them what there is to set of what is picked out, or of the
    /// frame, a section at a time, scrolled if it is long.
    fn right_panel(&mut self, layout: &mut Layout) {
        // With everything hidden, the left one's button is all there is.
        if !self.zen {
            layout.controls.push((self.side_button(1), Control::SidePanel(1)));
        }
        if !self.shows_right() {
            return;
        }
        let panel = Rect::new(self.width - INSET - RIGHT_W, INSET, RIGHT_W, self.height - 2 * INSET);
        layout.panels.push(panel);
        let (left, inner) = (panel.x + 14, panel.w - 28);
        layout.controls.push((Rect::new(left - 4, panel.y + 10, 80, 30), Control::Zoom));
        layout.controls.push((Rect::new(self.side_button(1).x - 6 - 92, panel.y + 10, 92, 30), Control::Action(Action::Export)));
        layout.controls.push((Rect::new(left - 4, panel.y + 46, 70, 26), Control::Tab(0)));
        layout.controls.push((Rect::new(left + 70, panel.y + 46, 70, 26), Control::Tab(1)));
        let top = panel.y + 80;
        // The sections, laid out apart and then kept to what of them shows.
        let mut part = Layout::default();
        let end = self.properties(&mut part, left, inner, top - self.scroll, panel.w);
        let most = (end + self.scroll - panel.bottom() + 16).max(0);
        if self.scroll > most {
            self.scroll = most;
            return self.right_panel(layout);
        }
        let shows = |rect: Rect| rect.y >= top && rect.bottom() <= panel.bottom() - 4;
        let field = self.palette.and_then(|paint| part.controls.iter().find(|(_, control)| *control == Control::ColorField(paint)).map(|(rect, _)| *rect));
        layout.controls.extend(part.controls.into_iter().filter(|(rect, _)| shows(*rect)));
        layout.words.extend(part.words.into_iter().filter(|((_, y), ..)| *y >= top && *y + 14 <= panel.bottom()));
        layout.icons.extend(part.icons.into_iter().filter(|((_, y), ..)| *y - 8 >= top && *y + 8 <= panel.bottom()));
        layout.rules.extend(part.rules.into_iter().filter(|rule| rule.y >= top && rule.y < panel.bottom()));
        // The colors, beside the panel at the color they are for.
        if let (Some(paint), Some(field)) = (self.palette, field) {
            self.layout_picker(layout, panel, field, paint);
        }
        layout.controls.push((panel, Control::Panel));
    }

    /// The part of the window between the panels and over the toolbar,
    /// where what is fitted to the view goes.
    fn open_area(&self) -> Rect {
        let left = if self.shows_left() { self.page_left() } else { INSET };
        let right = if self.shows_right() { self.page_right() } else { self.width - INSET };
        let bottom = if self.zen { INSET } else { DOCK_ROOM };
        Rect::new(left, INSET, (right - left).max(100), (self.height - INSET - bottom).max(100))
    }

    /// Whether each panel is shown: as it was left, unless everything is
    /// hidden.
    /// A board shows neither unless asked, with the same keys and buttons.
    fn shows_left(&self) -> bool {
        !self.zen && !self.presenting() && if self.on_board() { self.board.left } else { self.left_open }
    }

    fn shows_right(&self) -> bool {
        !self.zen && !self.presenting() && if self.on_board() { self.board.right } else { self.right_open }
    }

    /// Where the page begins and ends across the window: past the panels
    /// that are shown, and at the window's edges where none is.
    fn page_left(&self) -> i32 {
        if self.shows_left() { INSET + LEFT_W + INSET } else { 0 }
    }

    fn page_right(&self) -> i32 {
        if self.shows_right() { self.width - INSET - RIGHT_W - INSET } else { self.width }
    }

    /// The button that hides and shows a panel, at the top of it: the
    /// same place whether the panel is there or not, so that nothing
    /// moves, as Claude's own sidebar button does not.
    fn side_button(&self, side: u8) -> Rect {
        match side {
            0 => Rect::new(INSET + 12, INSET + 12, 28, 28),
            _ => Rect::new(self.width - INSET - 14 - 28, INSET + 12, 28, 28),
        }
    }

    /// A panel hidden or shown, and kept so for the next time; what was
    /// being typed in it kept first, and the colors beside the right one
    /// closed with it.
    fn toggle_panel(&mut self, side: u8) {
        self.commit_name();
        self.commit_typing();
        (self.palette, self.eyedropping) = (None, false);
        // A board's panels are hidden and shown on their own, kept for
        // boards alone.
        if self.on_board() && !self.zen {
            match side {
                0 => self.board.left = !self.board.left,
                _ => self.board.right = !self.board.right,
            }
            self.dirty = true;
            return;
        }
        // With everything hidden, everything comes back, that panel too.
        if self.zen {
            self.zen = false;
            match side {
                0 => self.left_open = true,
                _ => self.right_open = true,
            }
        } else {
            match side {
                0 => self.left_open = !self.left_open,
                _ => self.right_open = !self.right_open,
            }
        }
        if !cfg!(test) {
            keep_panels(self.left_open, self.right_open);
        }
        self.dirty = true;
    }

    /// Everything but the page hidden, or brought back: Ctrl+\, as in
    /// Figma. Esc leaves it as it is; only this, or the button left at the
    /// top, brings everything back.
    fn toggle_zen(&mut self) {
        self.commit_name();
        self.commit_typing();
        (self.palette, self.eyedropping, self.flyout) = (None, false, None);
        self.zen = !self.zen;
        self.dirty = true;
    }

    /// The tool taken up said for a moment, while there is no bar of
    /// tools to show it.
    fn say_tool(&mut self) {
        if !self.zen {
            return;
        }
        let says = if self.framing { "Frame  F".to_owned() } else { TOOLS.iter().find(|(known, ..)| *known == self.tool).map_or_else(|| tool_name(self.tool).to_owned(), |(_, name, key)| format!("{name}  {key}")) };
        self.say(says);
    }

    /// The frame worked in made to fit the open part of the window, with
    /// room round it.
    /// With the page's own layers worked in, all of them.
    fn fit(&mut self) {
        let open = self.open_area();
        let (x, y, w, h) = match self.on_page() {
            true => {
                let layers = self.doc.layers.clone();
                let boxes: Vec<(f32, f32, f32, f32)> = layers.iter().map(|layer| layer.bounds(&mut self.text)).collect();
                let Some(all) = actions::union(&boxes) else { return };
                all
            }
            false => (self.doc.x, self.doc.y, self.doc.width as f32, self.doc.height as f32),
        };
        let (w, h) = (w.max(1.0), h.max(1.0));
        self.zoom = (open.w as f32 * 0.86 / w).min(open.h as f32 * 0.86 / h).clamp(MIN_ZOOM, MAX_ZOOM);
        let middle = (open.x as f32 + open.w as f32 / 2.0, open.y as f32 + open.h as f32 / 2.0);
        self.pan = (middle.0 - (x + w / 2.0) * self.zoom, middle.1 - (y + h / 2.0) * self.zoom);
        self.dirty = true;
    }

    /// Larger or smaller, about a point of the window, which stays over
    /// the same point of the page.
    fn zoom_about(&mut self, times: f32, at: (i32, i32)) {
        let before = self.to_page(at);
        self.zoom = (self.zoom * times).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = (at.0 as f32 + 0.5 - before.0 * self.zoom, at.1 as f32 + 0.5 - before.1 * self.zoom);
        self.dirty = true;
    }

    /// The wheel turned, or a trackpad stroked, by so many notches down
    /// and so many to the right, parts of one as well: the panel of
    /// properties scrolled over it; with Ctrl, larger or smaller at once
    /// about the pointer, as Figma's and Chrome's are, down smaller; and
    /// otherwise the page moved, across with Shift.
    pub fn scrolled(&mut self, right: f32, down: f32) {
        self.end_preview();
        // With the search open, the wheel waits.
        if self.search.is_some() || (right == 0.0 && down == 0.0) {
            return;
        }
        self.dirty = true;
        if self.shows_right() && self.pointer.0 >= self.page_right() {
            // Over the panel of properties: that, scrolled.
            self.scroll = (self.scroll + (down * 48.0).round() as i32).max(0);
            return;
        }
        if self.pointer.0 < self.page_left() {
            return;
        }
        if self.mods.ctrl {
            self.zoom_about(wheel_zoom(down), self.pointer);
        } else if self.mods.shift {
            self.pan.0 -= (right + down) * WHEEL_STEP;
        } else {
            self.pan.0 -= right * WHEEL_STEP;
            self.pan.1 -= down * WHEEL_STEP;
        }
        self.moved_quickly();
    }

    /// A point of the window as a point of the page, and back.
    fn to_page(&self, (x, y): (i32, i32)) -> (f32, f32) {
        ((x as f32 + 0.5 - self.pan.0) / self.zoom, (y as f32 + 0.5 - self.pan.1) / self.zoom)
    }

    fn to_window_page(&self, (x, y): (f32, f32)) -> (i32, i32) {
        ((x * self.zoom + self.pan.0).round() as i32, (y * self.zoom + self.pan.1).round() as i32)
    }

    /// A point of the window as a point of the frame worked in, and back.
    fn to_doc(&self, at: (i32, i32)) -> (f32, f32) {
        let (x, y) = self.to_page(at);
        (x - self.doc.x, y - self.doc.y)
    }

    fn to_window(&self, (x, y): (f32, f32)) -> (i32, i32) {
        self.to_window_page((x + self.doc.x, y + self.doc.y))
    }

    /// Where a frame's name is, over its top left corner, in the window.
    fn frame_label(&mut self, index: usize) -> (Rect, String) {
        let frame = self.frame(index);
        let (name, at) = (frame.name.clone(), (frame.x, frame.y));
        let corner = self.to_window_page(at);
        let wide = self.text.width(&name, 12).max(24) + 4;
        (Rect::new(corner.0, corner.1 - Text::line_height(12) - 4, wide, Text::line_height(12)), name)
    }

    /// The frames' names that show, with where each is: those that would
    /// run into one shown already left out, as frames far apart crowd
    /// together zoomed far out; the one worked in first, always shown.
    fn frame_labels(&mut self) -> Vec<(usize, Rect, String)> {
        let order = std::iter::once(self.active).chain((0..self.frame_count()).filter(|&index| index != self.active));
        let mut shown: Vec<(usize, Rect, String)> = Vec::new();
        for index in order.filter(|&index| index < self.frame_count()).collect::<Vec<_>>() {
            let (place, name) = self.frame_label(index);
            if !shown.iter().any(|(_, other, _)| !other.inflate(3).intersect(place).is_empty()) {
                shown.push((index, place, name));
            }
        }
        shown
    }

    /// Whether a point of the window is over the page, and not a panel.
    fn over_page(&self, (x, _): (i32, i32)) -> bool {
        x >= self.page_left() && x < self.page_right()
    }

    fn chosen(&self) -> Vec<usize> {
        self.selected.iter().copied().filter(|&index| index < self.doc.layers.len()).collect()
    }

    /// The one layer picked out, if just one is.
    fn picked(&self) -> Option<usize> {
        match self.chosen()[..] {
            [index] => Some(index),
            _ => None,
        }
    }

    /// The handles of what is picked out, in the window: each corner and
    /// the middle of each side of its box, turned with it, by which way
    /// each pulls.
    fn handles(&mut self) -> Vec<((i32, i32), (i32, i32))> {
        let Some(index) = self.picked() else { return Vec::new() };
        let pulls = [(-1, -1), (1, -1), (1, 1), (-1, 1), (0, -1), (1, 0), (0, 1), (-1, 0)];
        match self.doc.layers[index].kind.clone() {
            Kind::Picture(picture) => pulls.iter().map(|&(px, py)| (self.to_window(picture.placed((px as f32 * picture.w / 2.0, py as f32 * picture.h / 2.0))), (px, py))).collect(),
            // A box or an oval by its frame, and text by its corners,
            // turned with it.
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Text | Tool::Badge | Tool::Blur | Tool::Spotlight) => {
                let (x, y, w, h) = match shape.tool {
                    Tool::Text | Tool::Badge => {
                        let place = shape.local_bounds(&mut self.text);
                        (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
                    }
                    _ => shape.frame(),
                };
                let count = if matches!(shape.tool, Tool::Text | Tool::Badge) { 4 } else { 8 };
                let spots: Vec<((f32, f32), (i32, i32))> = pulls[..count].iter().map(|&(px, py)| (shape.returned((x + w * (px + 1) as f32 / 2.0, y + h * (py + 1) as f32 / 2.0), &mut self.text), (px, py))).collect();
                spots.into_iter().map(|(spot, pull)| (self.to_window(spot), pull)).collect()
            }
            // A line or an arrow by its ends, which pull as END does.
            Kind::Shape(shape) if matches!(shape.tool, Tool::Line | Tool::Arrow) => {
                [0, 1].iter().map(|&end| (self.to_window(shape.points[end as usize]), (END, end))).collect()
            }
            // A zoom by the corners of what it is of, and its lens by its
            // middle and its rim, which pull as LENS does.
            Kind::Shape(shape) if shape.tool == Tool::Zoom => {
                let (of, _) = shape.zoomed();
                let corners = [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|(px, py)| ((if px < 0 { of.x } else { of.right() } as f32, if py < 0 { of.y } else { of.bottom() } as f32), (px, py)));
                let [middle, rim] = shape.zoom_handles();
                let lens = [((middle.0 as f32, middle.1 as f32), (effects::LENS, 2)), ((rim.0 as f32, rim.1 as f32), (effects::LENS, 3))];
                corners.into_iter().chain(lens).map(|(spot, pull)| (self.to_window(spot), pull)).collect()
            }
            Kind::Shape(_) => Vec::new(),
        }
    }

    /// What is under a point of the window: a control, a handle of what
    /// is picked out, just outside one of its corners, or a layer.
    fn control_at(&mut self, (x, y): (i32, i32)) -> Option<Control> {
        let layout = self.layout();
        layout.controls.iter().find(|(rect, _)| rect.contains(x, y)).map(|(_, control)| *control)
    }

    fn layer_at(&mut self, at: (f32, f32)) -> Option<usize> {
        (0..self.doc.layers.len()).rev().find(|&index| !self.doc.layers[index].hidden && !self.doc.layers[index].locked && self.doc.layers[index].hit(at, &mut self.text))
    }

    pub fn cursor(&self) -> Cursor {
        // Turning, or sizing, or about to.
        if let (Some(cursor), Drag::None | Drag::Turn { .. } | Drag::Scale { .. }) = (self.pointer_cursor, self.drag) {
            return cursor;
        }
        match self.drag {
            Drag::Pan { .. } => return Cursor::Move,
            Drag::Turn { .. } => return Cursor::Turn,
            Drag::Draw { .. } | Drag::Marquee { .. } | Drag::NewFrame { .. } => return Cursor::Crosshair,
            Drag::MoveFrame { .. } => return Cursor::Move,
            Drag::Scrub { .. } => return Cursor::Across,
            _ => {}
        }
        if self.space {
            return Cursor::Move;
        }
        if !self.over_page(self.pointer) {
            return Cursor::Arrow;
        }
        if self.framing || self.eyedropping {
            return Cursor::Crosshair;
        }
        match self.tool {
            Tool::Select => Cursor::Arrow,
            Tool::Text => Cursor::Text,
            _ => Cursor::Crosshair,
        }
    }

    // ---- The pointer ----

    /// Where the pointer is as the wheel turns, and the keys held: only
    /// that, for the zoom to keep the point under it still; what it is
    /// over is found by the next move.
    pub fn wheel_at(&mut self, x: i32, y: i32, mods: Mods) {
        self.pointer = (x, y);
        self.mods = self.steady_mods(mods);
    }

    pub fn pointer_moved(&mut self, x: i32, y: i32, mods: Mods) {
        self.crop_window();
        let mods = self.steady_mods(mods);
        (self.pointer, self.mods) = ((x, y), mods);
        if self.on_board() && self.drag == Drag::None && self.laser_moved((x, y)) {
            return;
        }
        if self.fine.is_some() && self.drag == Drag::None {
            return self.fine_moved(x, y);
        }
        self.pointer_cursor = self.pointer_shape();
        let at = self.to_doc((x, y));
        match self.drag {
            Drag::Picker { part, rect } => self.pick_at(part, rect, (x, y), false),
            Drag::Slide { field, left, wide } => self.slide_to(field, left, wide, x),
            Drag::Guide { index } => self.drag_guide(index, (x, y)),
            Drag::Reorder { index, from, moved: false } if (y - from).abs() > 4 => self.drag = Drag::Reorder { index, from, moved: true },
            Drag::Reorder { .. } | Drag::ReorderFrame { moved: true, .. } => self.dirty = true,
            Drag::ReorderFrame { index, from, moved: false } => {
                if (y - from).abs() > 4 {
                    self.drag = Drag::ReorderFrame { index, from, moved: true };
                }
            }
            Drag::None => {
                // What a control the pointer rests on would do, shown.
                let over = self.control_at((x, y));
                self.hover_preview(over);
                // Over an open list, the list a row of it opens.
                if self.menu.is_some() {
                    let control = self.control_at((x, y));
                    self.hover_menu(control);
                }
                let selecting = self.tool == Tool::Select && self.over_page((x, y)) && !self.framing && over.is_none();
                // One of the page's own layers, where it is over the frames,
                // before one of the frame worked in.
                let hovered_page = match selecting && !self.on_page() {
                    true => self.page_layer_at(self.to_page((x, y))),
                    false => None,
                };
                let hovered = match selecting && hovered_page.is_none() {
                    // On the page, none of its own under a frame over it.
                    true if self.on_page() => self.page_layer_at(self.to_page((x, y))),
                    true => self.layer_at(at),
                    false => None,
                };
                // Drawn again only for what shows where the pointer is: the
                // layer or the control it is over, or the text tool's caret
                // that follows it; a move over nothing new draws nothing.
                // And the stack it is over, whose gaps and padding show.
                let stack = if self.over_page((x, y)) && over.is_none() && hovered.is_none() && hovered_page.is_none() && self.cropping.is_none() { self.stack_hover(at) } else { None };
                self.dirty |= hovered != self.hovered || hovered_page != self.hovered_page || over != self.hover_shown || self.tool == Tool::Text || stack != self.stack_hovered;
                (self.hovered, self.hovered_page, self.hover_shown, self.stack_hovered) = (hovered, hovered_page, over, stack);
                if self.on_board() {
                    self.board_hover((x, y), over.is_some());
                }
            }
            Drag::NewFrame { .. } => self.dirty = true,
            Drag::MoveFrame { from, origin, moved, copy } => {
                let (dx, dy) = ((x - from.0) as f32 / self.zoom, (y - from.1) as f32 / self.zoom);
                if !moved && (x - from.0).abs().max((y - from.1).abs()) < 3 {
                    return;
                }
                if !moved {
                    // With Alt, a copy left where it was is what moves.
                    match copy {
                        true => self.copy_frame(origin),
                        false => self.checkpoint_move(),
                    }
                    self.drag = Drag::MoveFrame { from, origin, moved: true, copy };
                }
                // Snapped to the other frames, as a layer is to the rest,
                // but for along the way Shift holds it to, or with Ctrl.
                let across = dx.abs() >= dy.abs();
                let (dx, dy) = match (mods.shift, across) {
                    (true, true) => (dx, 0.0),
                    (true, false) => (0.0, dy),
                    _ => (dx, dy),
                };
                let mut at = self.snap_frame((origin.0 + dx, origin.1 + dy), mods.ctrl);
                if mods.shift {
                    match across {
                        true => at.1 = origin.1,
                        false => at.0 = origin.0,
                    }
                    self.guides.retain(|&(upright, _)| upright == across);
                    self.gaps.retain(|gap| gap.down != across);
                }
                (self.doc.x, self.doc.y) = at;
                self.changed();
            }
            Drag::Pan { from, origin } => {
                self.pan = (origin.0 + (x - from.0) as f32, origin.1 + (y - from.1) as f32);
                self.moved_quickly();
            }
            Drag::Move { from, moved, copy } => {
                let (mut dx, mut dy) = (at.0 - from.0, at.1 - from.1);
                if mods.shift {
                    if dx.abs() >= dy.abs() { dy = 0.0 } else { dx = 0.0 }
                }
                if !moved && (dx.abs() + dy.abs()) * self.zoom < 3.0 {
                    return;
                }
                if !moved {
                    // With Alt, a copy is made where it is, and that moved.
                    // On a board, what is in a box goes with it.
                    if self.on_board() {
                        self.begin_carry(copy);
                    }
                    match copy {
                        true => {
                            self.copy_layers_by((0.0, 0.0));
                            self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
                        }
                        false => self.checkpoint_move(),
                    }
                    self.drag = Drag::Move { from, moved: true, copy };
                    self.let_go();
                }
                // Snapped to the artboard and to the rest, but for along
                // the way Shift holds it to, or with Ctrl held.
                // On a board, a soft settle: near enough is level, with no
                // guides drawn; with Alt, exactly where it is.
                let free = mods.ctrl || (self.on_board() && mods.alt);
                let (mut dx, mut dy) = if free { (dx.round(), dy.round()) } else { self.snap(dx.round(), dy.round()) };
                if self.on_board() {
                    self.guides.clear();
                    self.gaps.clear();
                }
                if mods.shift && (at.0 - from.0).abs() >= (at.1 - from.1).abs() {
                    dy = 0.0;
                } else if mods.shift {
                    dx = 0.0;
                }
                for (index, layer) in self.held.clone().into_iter().enumerate() {
                    if let Some(&chosen) = self.chosen().get(index) {
                        let mut layer = layer;
                        layer.translate(dx, dy);
                        self.doc.layers[chosen] = layer;
                    }
                }
                if self.on_board() {
                    self.carry(dx, dy);
                }
                let chosen = self.chosen();
                self.changed_layers(&chosen);
            }
            Drag::Scale { pull, from } => self.scale_to(pull, from, at, mods),
            Drag::Turn { about, began } => self.drag_turn(about, began, at, mods.shift),
            Drag::Draw { from } => {
                let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.last_mut() else { return };
                match shape.tool {
                    Tool::Pen | Tool::Marker => shape.points.push(at),
                    tool => {
                        let to = if mods.shift { shapes::constrained(tool, from, at) } else { at };
                        shape.points[1] = to;
                        // A zoom's lens beside what it is of, as capture
                        // mode puts it.
                        if tool == Tool::Zoom {
                            shape.place_zoom();
                        }
                    }
                }
                shape.touch();
                let last = self.doc.layers.len() - 1;
                self.changed_layers(&[last]);
            }
            Drag::Marquee { .. } => self.dirty = true,
            Drag::Scrub { field, from, start } => {
                let step = match field {
                    Field::Turn => 0.5,
                    Field::Opacity => 1.0,
                    Field::Magnify => 0.05,
                    _ => 1.0,
                } * if mods.shift { 10.0 } else { 1.0 };
                self.set_field(field, start + (x - from) as f32 * step);
            }
            Drag::Crop { pull, from } => self.crop_to(pull, from, at, mods),
            Drag::Radius { corner, single, slack } => self.drag_radius(corner, single, slack, at),
            Drag::Pull { .. } => self.pull_moved(),
        }
    }

    pub fn pressed(&mut self, button: u8, time: u32) -> Outcome {
        self.crop_window();
        self.end_preview();
        self.menu_row = None;
        // Fine-tune open takes a click, and closes with one off it.
        if self.fine.is_some() {
            if let Some(outcome) = self.fine_pressed(button) {
                return outcome;
            }
        }
        // The window of what was written closes with a click off it.
        if self.exported.is_some() && button <= 3 && !matches!(self.control_at(self.pointer), Some(Control::ExportDone(_) | Control::Panel)) {
            self.exported = None;
            self.dirty = true;
            return Outcome::Continue;
        }
        // With the search open, a click off it closes it; the wheel waits.
        if self.search.is_some() && (4..=7).contains(&button) {
            return Outcome::Continue;
        }
        if self.search.is_some() && !matches!(self.control_at(self.pointer), Some(Control::Found(_) | Control::SearchTab(_) | Control::Panel)) {
            self.search = None;
            self.dirty = true;
            return Outcome::Continue;
        }
        // With the library open, the wheel goes through its emoji, and a
        // click off it closes it and does nothing else.
        if self.library.is_some() && button <= 7 && self.library_press(button) {
            return Outcome::Continue;
        }
        let at_window = self.pointer;
        let double =time.wrapping_sub(self.last_press.0) < 400 && (at_window.0 - self.last_press.1 .0).abs().max((at_window.1 - self.last_press.1 .1).abs()) <= 3;
        self.last_press = (time, at_window);
        self.dirty = true;
        self.pressed_at = self.version;
        match button {
            // A notch of the wheel: up, down, left or right.
            4..=7 => {
                let (right, down) = wheel_notch(button);
                self.scrolled(right, down);
                return Outcome::Continue;
            }
            2 => {
                self.drag = Drag::Pan { from: at_window, origin: self.pan };
                return Outcome::Continue;
            }
            3 => return self.right_click(at_window),
            1 => {}
            _ => return Outcome::Continue,
        }
        if self.click_away(at_window) {
            return Outcome::Continue;
        }
        if self.typing.is_some() && self.control_at(at_window) != self.typing.as_ref().map(|(field, _)| Control::Field(*field)) {
            self.commit_typing();
        }
        // Another layer clicked in the list ends the typing of text first,
        // so that what is typed next goes nowhere it should not.
        if let (Some(editing), Some(Control::Layer(index))) = (self.editing, self.control_at(at_window)) {
            if index != editing {
                self.commit_text();
            }
        }
        let mut control = self.control_at(at_window);
        // A row of the page's own layers works in them first, and is then
        // the row it is among them.
        if let Some(Control::PageLayer(_)) = control {
            self.activate(LOOSE);
            control = self.control_at(at_window);
        }
        if let Some(control) = control {
            let outcome = self.press_control(control, double);
            // A frame's name on the page is what it is moved by.
            if let (Control::Layer(index), false, false) = (control, double, self.mods.shift) {
                self.drag = Drag::Reorder { index, from: at_window.1, moved: false };
            }
            match (control, double, self.over_page(at_window)) {
                (Control::Frame(_), false, true) => self.drag = Drag::MoveFrame { from: at_window, origin: (self.doc.x, self.doc.y), moved: false, copy: self.mods.alt },
                // Its row in the list, to put it over or under the rest.
                (Control::Frame(index), false, false) => self.drag = Drag::ReorderFrame { index, from: at_window.1, moved: false },
                _ => {}
            }
            return outcome;
        }
        // A ruler drags out a guide; a guide is taken hold of.
        if self.press_rulers(at_window) {
            return Outcome::Continue;
        }
        // A color taken off the page, for the paint the picker is open for.
        if self.eyedropping {
            self.eyedropping = false;
            if let (Some(paint), Some(color)) = (self.palette, self.take_color(at_window)) {
                self.paint(paint, Some(color));
                self.hsv = picker::to_hsv(color);
            }
            return Outcome::Continue;
        }
        // On the page.
        self.commit_name();
        (self.palette, self.flyout) = (None, None);
        let off_everything = self.frame_at(self.to_page(at_window)).is_none() && self.page_layer_at(self.to_page(at_window)).is_none() && !self.near_selection(at_window);
        if self.space || self.tool == Tool::Select && self.mods.alt && off_everything {
            self.drag = Drag::Pan { from: at_window, origin: self.pan };
            return Outcome::Continue;
        }
        if self.framing {
            self.drag = Drag::NewFrame { from: self.to_page(at_window) };
            return Outcome::Continue;
        }
        // In another frame, that one worked in; on the page's own layers,
        // or outside every frame, they.
        if let Some(target) = self.press_target(at_window) {
            self.activate(target);
        }
        self.frame_picked = false;
        let at = self.to_doc(at_window);
        if let Some(index) = self.cropping {
            return self.press_crop(index, at, double);
        }
        if let Some(index) = self.editing {
            if self.doc.layers[index].hit(at, &mut self.text) {
                return Outcome::Continue;
            }
            self.commit_text();
        }
        if self.on_board() && self.board_pressed(at, at_window, double) {
            return Outcome::Continue;
        }
        match self.tool {
            Tool::Select => self.press_select(at, at_window, double),
            Tool::Text => {
                self.checkpoint();
                // On text already there, that typed into, as Figma's text
                // tool does, rather than new text over it.
                let on_text = self.layer_at(at).filter(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge)));
                if let Some(index) = on_text {
                    self.edit_text(index);
                    self.selected = vec![index];
                    self.changed();
                    return Outcome::Continue;
                }
                let mut shape = Shape::new(Tool::Text, self.color, STROKES[self.size], (at.0, at.1 - FONT_SIZES[self.size] as f32 / 2.0));
                (shape.font_px, shape.face) = (FONT_SIZES[self.size], self.face);
                if self.on_board() {
                    self.board_dress(&mut shape);
                    shape.points[0].1 = at.1 - shape.font_px as f32 / 2.0;
                }
                self.doc.layers.push(Layer::shape(shape));
                self.edit_text(self.doc.layers.len() - 1);
                self.selected = vec![self.doc.layers.len() - 1];
                self.changed();
            }
            tool => {
                self.checkpoint();
                let mut shape = Shape::new(tool, self.color, STROKES[self.size], at);
                if matches!(tool, Tool::Rect | Tool::Ellipse) {
                    (shape.filled, shape.fill) = (self.fill.is_some(), self.fill.unwrap_or(0xFFFFFF));
                    shape.gradient = self.gradient.filter(|_| shape.filled);
                    // One with no fill has its border, or there would be
                    // nothing of it.
                    shape.border = self.border || !shape.filled;
                    shape.radius = if tool == Tool::Rect { self.radius } else { 0.0 };
                }
                shape.number = self.doc.layers.iter().filter(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Counter)).count() as u32 + 1;
                // A spotlight's number is how dark it makes the rest, and
                // none is capture mode's.
                if tool == Tool::Spotlight {
                    shape.number = 0;
                }
                shape.font_px = FONT_SIZES[self.size];
                if self.on_board() {
                    self.board_dress(&mut shape);
                }
                self.doc.layers.push(Layer::shape(shape));
                self.selected.clear();
                self.drag = Drag::Draw { from: at };
                self.changed();
            }
        }
        Outcome::Continue
    }

    /// A click off an open list, off the list of a group of tools, or off
    /// the grid of styles: that closed, and the click does nothing else,
    /// as Figma's do. A group's own button still opens or closes its list.
    fn click_away(&mut self, at: (i32, i32)) -> bool {
        if self.menu.is_none() && self.flyout.is_none() && !self.styling {
            return false;
        }
        let layout = self.layout();
        let inside = |rect: &Rect| rect.contains(at.0, at.1);
        let control = layout.controls.iter().find(|(rect, _)| inside(rect)).map(|(_, control)| *control);
        let mut closed = false;
        if self.menu.is_some() && !layout.menus.iter().any(|(rect, _)| inside(rect)) {
            self.menu = None;
            closed = true;
        }
        if self.flyout.is_some() && !layout.flyout.is_some_and(|rect| inside(&rect)) && !matches!(control, Some(Control::Group(_))) {
            self.flyout = None;
            closed = true;
        }
        if self.styling && !layout.styles.is_some_and(|rect| inside(&rect)) {
            self.styling = false;
            closed = true;
        }
        self.dirty |= closed;
        closed
    }

    fn press_select(&mut self, at: (f32, f32), at_window: (i32, i32), double: bool) {
        // The handles of the one picked out, and just outside its corners
        // to turn it, come before what is under them.
        let near = |a: (i32, i32), b: (i32, i32), reach: i32| (a.0 - b.0).abs() <= reach && (a.1 - b.1).abs() <= reach;
        // The dots inside the corners of a box, which round them, first.
        if !self.on_board() && self.press_radius(at, at_window) {
            return;
        }
        let handles = self.handles();
        if let Some(&(_, pull)) = handles.iter().find(|&&(spot, _)| near(spot, at_window, 6)) {
            self.checkpoint();
            self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
            self.drag = Drag::Scale { pull, from: at };
            return;
        }
        // A zoom's lens is moved on its own, from anywhere inside it.
        if let Some(index) = self.layer_at(at).filter(|_| !self.mods.shift && !double) {
            if let Kind::Shape(shape) = &self.doc.layers[index].kind {
                let (of, shown) = if shape.tool == Tool::Zoom { shape.zoomed() } else { (Rect::default(), Rect::default()) };
                let (x, y) = (at.0.round() as i32, at.1.round() as i32);
                if shown.contains(x, y) && !of.contains(x, y) {
                    self.selected = vec![index];
                    self.checkpoint();
                    self.held = vec![self.doc.layers[index].clone()];
                    self.drag = Drag::Scale { pull: (effects::LENS, 2), from: at };
                    return;
                }
            }
        }
        // A side is pulled anywhere along it, as Figma has it: only the
        // corners have handles drawn.
        if handles.len() == 8 {
            let off = |a: (i32, i32), b: (i32, i32), p: (i32, i32)| {
                let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
                let along = (((p.0 - a.0) as f32 * dx + (p.1 - a.1) as f32 * dy) / (dx * dx + dy * dy).max(1.0)).clamp(0.0, 1.0);
                ((p.0 as f32 - a.0 as f32 - along * dx).powi(2) + (p.1 as f32 - a.1 as f32 - along * dy).powi(2)).sqrt()
            };
            if let Some(side) = (0..4).find(|&side| off(handles[side].0, handles[(side + 1) % 4].0, at_window) <= 4.0) {
                self.checkpoint();
                self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
                self.drag = Drag::Scale { pull: handles[4 + side].1, from: at };
                return;
            }
        }
        // Just outside a corner of what is picked out, one layer or more,
        // it is turned.
        if self.press_turn(at, at_window) {
            return;
        }
        let under = self.layer_at(at);
        // Inside the box of what is picked out a drag moves all of it, as
        // Figma has it, though the press is on nothing of it: inside a box
        // with no fill, or between several picked out. Also where the press
        // is on what lies beneath all of it, as a picture under a box drawn
        // on it does; a click let go there without a move picks that out.
        self.under_press = None;
        let beneath = under.filter(|index| self.chosen().iter().all(|chosen| chosen > index));
        if !self.mods.shift && !double && (under.is_none_or(|index| self.selected.contains(&index)) || beneath.is_some()) && !self.chosen().is_empty() {
            let corners: Vec<(i32, i32)> = self.chosen().into_iter().flat_map(|index| self.box_corners(index)).collect();
            let (left, right) = (corners.iter().map(|c| c.0).min().unwrap_or(0), corners.iter().map(|c| c.0).max().unwrap_or(0));
            let (top, bottom) = (corners.iter().map(|c| c.1).min().unwrap_or(0), corners.iter().map(|c| c.1).max().unwrap_or(0));
            if (left..=right).contains(&at_window.0) && (top..=bottom).contains(&at_window.1) {
                self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
                self.drag = Drag::Move { from: at, moved: false, copy: self.mods.alt };
                self.under_press = beneath;
                return;
            }
        }
        match under {
            Some(index) => {
                if self.mods.shift {
                    match self.selected.iter().position(|&other| other == index) {
                        Some(at) => drop(self.selected.remove(at)),
                        None => self.selected.push(index),
                    }
                    return;
                }
                if !self.selected.contains(&index) {
                    self.selected = vec![index];
                }
                if double {
                    match &self.doc.layers[index].kind {
                        // The double click that begins a crop is not half of
                        // the next, or a third click quickly after it would
                        // fit the crop at once.
                        Kind::Picture(_) => (self.cropping, self.last_press.1) = (Some(index), (i32::MIN / 2, i32::MIN / 2)),
                        Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge) => {
                            self.checkpoint();
                            self.edit_text(index);
                        }
                        Kind::Shape(_) => {}
                    }
                    return;
                }
                self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
                self.drag = Drag::Move { from: at, moved: false, copy: self.mods.alt };
            }
            // Alt on nothing pans, as it does off the frames; on a layer it
            // copies it.
            None if self.mods.alt => self.drag = Drag::Pan { from: at_window, origin: self.pan },
            // In a stack's padding or between what is in it, as in Figma's
            // auto layout frame: all of it picked out, to be moved.
            None if !self.mods.shift && self.stack_at(at).is_some() => {
                let id = self.stack_at(at).map(|index| self.doc.stacks[index].id).unwrap_or_default();
                let leaves = self.leaves(id);
                self.selected = (0..self.doc.layers.len()).filter(|&index| leaves.contains(&stacks::layer_id(&self.doc.layers[index])) && !self.doc.layers[index].locked).collect();
                self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
                self.drag = Drag::Move { from: at, moved: false, copy: false };
            }
            None => {
                if !self.mods.shift {
                    self.selected.clear();
                }
                self.drag = Drag::Marquee { from: at };
            }
        }
    }

    /// A press while a picture is cropped: on a corner or a side of its
    /// crop, that; inside it, all of it, or with a double click the box of
    /// lines round where it is; outside, the end of cropping.
    fn press_crop(&mut self, index: usize, at: (f32, f32), double: bool) -> Outcome {
        let Kind::Picture(picture) = &self.doc.layers[index].kind else {
            self.cropping = None;
            return Outcome::Continue;
        };
        let (lx, ly) = picture.local(at);
        let (hw, hh) = (picture.w / 2.0, picture.h / 2.0);
        let reach = 8.0 / self.zoom;
        let pull = |at: f32, half: f32| if (at + half).abs() <= reach { -1 } else if (at - half).abs() <= reach { 1 } else { 0 };
        let (px, py) = (pull(lx, hw), pull(ly, hh));
        let inside = lx.abs() <= hw + reach && ly.abs() <= hh + reach;
        if !inside {
            self.cropping = None;
            return Outcome::Continue;
        }
        if double && (px, py) == (0, 0) {
            self.crop_around(index, at);
            return Outcome::Continue;
        }
        self.checkpoint();
        self.held = vec![self.doc.layers[index].clone()];
        self.drag = Drag::Crop { pull: (px, py), from: at };
        Outcome::Continue
    }

    fn press_control(&mut self, control: Control, double: bool) -> Outcome {
        // A name being typed is kept first: a hex color needs the colors
        // still open to know what it colors.
        if !matches!(control, Control::Page(_) | Control::Frame(_)) || matches!(self.renaming, Some((Naming::Hex, _))) {
            self.commit_name();
        }
        // The colors stay open while colors are picked, and a group's list
        // while it is picked from; anything else closes them.
        // So do they while the kind of fill they are for is changed, from
        // their top or the panel, or a gradient's stop is pressed.
        let kind_of_fill = match control {
            Control::Action(Action::FillKind(_)) | Control::FillBar => matches!(self.palette, Some(Paint::Fill | Paint::FillTo)),
            Control::Ground(_) | Control::GradientBar => matches!(self.palette, Some(Paint::Background | Paint::From | Paint::To)),
            _ => false,
        };
        if !matches!(control, Control::Swatch(..) | Control::ColorField(_) | Control::Picker(_) | Control::Entry(Naming::Hex)) && !kind_of_fill {
            self.palette = None;
        }
        if !matches!(control, Control::Group(_) | Control::Pick(_)) {
            self.flyout = None;
        }
        if !matches!(control, Control::MenuRow(..)) {
            self.menu = None;
        }
        match control {
            Control::StackWay(_) | Control::StackSizing(_) | Control::StackAlign(_) => self.press_stack(control),
            Control::SidePanel(_) if self.zen => self.toggle_zen(),
            Control::SidePanel(side) => self.toggle_panel(side),
            Control::Home => {
                let place = self.control_rect(control).unwrap_or_default();
                self.menu = Some(MenuState { kind: MenuKind::Main, at: (place.x, place.bottom() + 6), sub: None });
            }
            Control::Zoom => {
                let place = self.control_rect(control).unwrap_or_default();
                self.menu = Some(MenuState { kind: MenuKind::Zoom, at: (place.x, place.bottom() + 4), sub: None });
            }
            Control::MenuRow(level, row) => return self.press_menu(level, row),
            Control::Menu(kind) => {
                let place = self.control_rect(control).unwrap_or_default();
                // Beside the panel, level with the field it opens from.
                let at = (place.x - 8 - 280, place.y - 6);
                self.menu = Some(MenuState { kind, at, sub: None });
            }
            Control::Window(which) => {
                let says = self.says_of(menu::MenuAct::Window(which));
                self.put_window(chrome::CHOICES[usize::from(which).min(chrome::CHOICES.len() - 1)]);
                self.remember(says);
            }
            Control::WindowTheme(which) => self.put_window_theme(chromes::THEMES[usize::from(which).min(2)]),
            Control::WindowBar(swap) => self.put_window_bar(swap),
            Control::WindowShadow(shadow) => self.put_window_shadow(shadow.min(OWN_SHADOW)),
            Control::Entry(naming) => {
                let now = match naming {
                    Naming::Title(index) => match self.doc.layers.get(index).map(|layer| &layer.kind) {
                        Some(Kind::Picture(picture)) => picture.title.clone(),
                        _ => String::new(),
                    },
                    Naming::Address(index) => match self.doc.layers.get(index).map(|layer| &layer.kind) {
                        Some(Kind::Picture(picture)) => picture.chrome.as_ref().map_or(String::new(), |chrome| chrome.address.clone()),
                        _ => String::new(),
                    },
                    Naming::Suffix(index) => self.doc.exports.get(index).map_or(String::new(), |size| size.suffix.clone()),
                    Naming::Frame => self.doc.name.clone(),
                    Naming::Page(page) => self.pages[page].name.clone(),
                    Naming::Hex => format!("{:06X}", self.palette.and_then(|paint| self.paint_color(paint)).unwrap_or(0)),
                    Naming::Filter => self.layer_filter.clone(),
                    Naming::Layer(index) => self.doc.layers.get(index).map_or(String::new(), Layer::name),
                    Naming::Stack(id) => self.stack_by_id(id).map_or(String::new(), stacks::Stack::title),
                };
                self.start_naming(naming, now);
            }
            Control::StackRow(id) => self.press_stack_row(id, double),
            Control::StackFold(id) => self.fold_stack(id),
            // What an end is stuck to, picked out.
            Control::Stuck(end) => {
                let stuck = self.picked().and_then(|index| match &self.doc.layers[index].kind {
                    Kind::Shape(shape) => shape.stuck[usize::from(end).min(1)].map(|(id, ..)| id),
                    Kind::Picture(_) => None,
                });
                if let Some(at) = stuck.and_then(|id| self.layer_by_id(id)) {
                    self.selected = vec![at];
                }
            }

            Control::Page(index) => match double {
                true => self.start_naming(Naming::Page(index), self.pages[index].name.clone()),
                false => self.go_to_page(index),
            },
            Control::NewPage => self.new_page(),
            Control::Frame(index) => {
                self.activate(index);
                (self.frame_picked, self.tool, self.framing) = (true, Tool::Select, false);
                self.selected.clear();
                if double {
                    self.start_naming(Naming::Frame, self.doc.name.clone());
                }
            }
            Control::FrameTool => {
                self.commit_text();
                (self.framing, self.tool) = (!self.framing, Tool::Select);
                self.selected.clear();
                self.say_tool();
            }
            Control::Group(group) => self.flyout = if self.flyout == Some(group) { None } else { Some(group) },
            Control::Pick(tool) => {
                if !MORE_TOOLS.contains(&tool) {
                    let group = usize::from(matches!(tool, Tool::Pen | Tool::Marker));
                    self.groups[group] = tool;
                }
                self.flyout = None;
                match tool {
                    Tool::Badge => self.open_library(None),
                    tool => self.set_tool(tool),
                }
            }
            Control::Shelf(_) | Control::LibraryItem(_) | Control::EmojiItem(_) => self.press_library(control),
            Control::Style(style) => {
                self.put_style(style);
                self.styling = false;
            }
            Control::SearchTab(tab) => {
                if let Some(search) = &mut self.search {
                    (search.tab, search.row) = (tab, 0);
                }
            }
            Control::Found(row) => return self.choose_hit(usize::from(row)),
            Control::Tab(tab) => (self.panel_tab, self.scroll) = (tab, 0),
            Control::ExportPick(index) => match self.export_off.iter().position(|&off| off == index) {
                Some(at) => drop(self.export_off.remove(at)),
                None => self.export_off.push(index),
            },
            Control::ExportDone(done) => self.press_exported(done),
            Control::ColorOpen(color) => self.color_open = if self.color_open == Some(color) { None } else { Some(color) },
            Control::ColorLayer(index, _) => self.selected = vec![index],
            Control::FillBar => self.press_fill_bar(),
            Control::GradientBar => {
                // The stop nearer where it was pressed, to color.
                let rect = self.control_rect(control).unwrap_or_default();
                let paint = if self.pointer.0 < rect.x + rect.w / 2 { Paint::From } else { Paint::To };
                self.open_picker(paint);
            }
            Control::ColorField(paint) => match self.palette == Some(paint) {
                true => self.palette = None,
                false => self.open_picker(paint),
            },
            Control::Picker(picker::DROPPER) => self.eyedropping = !self.eyedropping,
            Control::Picker(part) => {
                let rect = self.control_rect(control).unwrap_or_default();
                self.drag = Drag::Picker { part, rect };
                let at = self.pointer;
                self.pick_at(part, rect, at, true);
            }
            Control::Tool(tool) => self.set_tool(tool),
            // The page's own layers worked in, and that one picked out.
            Control::PageLayer(index) => {
                self.activate(LOOSE);
                return self.press_control(Control::Layer(index), double);
            }
            Control::Layer(index) => {
                match self.mods.shift {
                    true => match self.selected.iter().position(|&other| other == index) {
                        Some(at) => drop(self.selected.remove(at)),
                        None => self.selected.push(index),
                    },
                    false => self.selected = vec![index],
                }
                if double {
                    // Text typed into; anything else renamed where it is
                    // listed, a shape as well as a picture.
                    match &self.doc.layers[index].kind {
                        Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge) => {
                            self.checkpoint();
                            self.edit_text(index);
                        }
                        _ => self.start_naming(Naming::Layer(index), self.doc.layers[index].name()),
                    }
                }
            }
            Control::Eye(index) => {
                self.checkpoint();
                self.doc.layers[index].hidden = !self.doc.layers[index].hidden;
                self.changed();
            }
            Control::Lock(index) => {
                // A step to undo, as hiding is, and as the menu's lock is.
                self.checkpoint();
                self.doc.layers[index].locked = !self.doc.layers[index].locked;
                self.selected.retain(|&other| other != index);
                self.changed();
            }
            Control::Field(field) if field.slides() => {
                // A slider: the value where it is pressed, and dragged.
                let place = self.control_rect(control).unwrap_or_default();
                self.checkpoint();
                self.drag = Drag::Slide { field, left: place.x + 80, wide: (place.w - 84).max(20) };
                let (x, mods) = (self.pointer.0, self.mods);
                self.slide_to(field, place.x + 80, (place.w - 84).max(20), x);
                let _ = mods;
            }
            Control::Field(field) => match double {
                true => self.typing = Some((field, String::new())),
                false => {
                    self.checkpoint();
                    let start = self.field(field).unwrap_or(0.0);
                    self.drag = Drag::Scrub { field, from: self.pointer.0, start };
                }
            },
            Control::Ground(ground) => {
                self.checkpoint();
                self.doc.background = match ground {
                    Ground::None => Fill::None,
                    Ground::Color => Fill::Color(match self.doc.background {
                        Fill::Color(color) => color,
                        _ => 0xFFFFFF,
                    }),
                    Ground::Backdrop => Fill::Backdrop(match self.doc.background {
                        Fill::Backdrop(which) => which,
                        _ => frame::named("aurora").unwrap_or(0),
                    }),
                    // Indigo to pink, down and to the right, to begin with.
                    Ground::Gradient => match self.doc.background {
                        Fill::Gradient(..) => self.doc.background,
                        Fill::Radial(from, to) => Fill::Gradient(from, to, 135.0),
                        _ => Fill::Gradient(0x3E63DD, 0xD6409F, 135.0),
                    },
                    // A gradient's colors kept, out from the middle.
                    Ground::Radial => match self.doc.background.stops() {
                        Some((from, to)) => Fill::Radial(from, to),
                        None => Fill::Radial(0xE1E9FF, 0x3E63DD),
                    },
                };
                // The colors, open from the top of them, go on to what
                // there is to color now.
                self.palette = match (self.palette, self.doc.background) {
                    (Some(Paint::From | Paint::To), Fill::Color(_)) => Some(Paint::Background),
                    (Some(Paint::Background), Fill::Gradient(..) | Fill::Radial(..)) => Some(Paint::From),
                    (palette, _) => palette,
                };
                self.changed();
            }
            Control::Backdrop(which) => {
                self.checkpoint();
                self.doc.background = Fill::Backdrop(which);
                self.changed();
            }
            Control::Swatch(paint, color) => {
                self.paint(paint, color);
                if let Some(color) = color {
                    self.hsv = picker::to_hsv(color);
                }
            }
            Control::Shadow(which) => self.restyle(|layer| match &mut layer.kind {
                Kind::Picture(picture) => picture.shadow = which,
                Kind::Shape(shape) => shape.shadow = Some(actions::custom_drop(which)),
            }),
            Control::Board(ctl) => return self.press_board(ctl),
            Control::Action(action) => return self.act(action),
            Control::Panel => {}
        }
        self.dirty = true;
        Outcome::Continue
    }

    /// Changes each layer picked out, as one step to undo.
    fn restyle(&mut self, change: impl Fn(&mut Layer)) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        self.checkpoint();
        for index in chosen {
            change(&mut self.doc.layers[index]);
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                shape.touch();
            }
        }
        self.changed();
    }

    /// A color for the line or the fill of what is picked out, and of
    /// what is drawn next; or for the background.
    fn paint(&mut self, paint: Paint, color: Option<u32>) {
        match paint {
            Paint::Stack => self.change_stack(|stack, _| stack.fill = color),
            Paint::Background => {
                self.checkpoint();
                self.doc.background = Fill::Color(color.unwrap_or(0xFFFFFF));
                self.changed();
            }
            Paint::From | Paint::To => {
                let Some((from, to)) = self.doc.background.stops() else { return };
                let color = color.unwrap_or(0xFFFFFF);
                self.checkpoint();
                self.doc.background = if paint == Paint::From { self.doc.background.with_stops(color, to) } else { self.doc.background.with_stops(from, color) };
                self.changed();
            }
            Paint::Line => {
                let color = color.unwrap_or(self.color);
                self.color = color;
                // A border given a box is one the next box has too.
                let boxes = self.chosen().iter().any(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse)));
                if boxes || (self.chosen().is_empty() && matches!(self.tool, Tool::Rect | Tool::Ellipse)) {
                    self.border = true;
                }
                self.keep_shape_style();
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        (shape.color, shape.border) = (color, true);
                    }
                });
            }
            Paint::Fill => {
                self.fill = color;
                self.keep_shape_style();
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        match color {
                            Some(color) => (shape.filled, shape.fill) = (true, color),
                            None => (shape.filled, shape.border) = (false, true),
                        }
                    }
                });
            }
            Paint::FillTo => self.paint_fill_to(color),
            Paint::Shadow => {
                let color = color.unwrap_or(0) & 0xFF_FFFF;
                self.restyle(|layer| layer.change_drop(|drop| drop.color = (drop.color & 0xFF00_0000) | color));
            }
            // One color among what is picked out made another, in each of
            // them, line and fill; and the colors follow it.
            Paint::Recolor(was) => {
                let Some(color) = color else { return };
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        if shape.color == was {
                            shape.color = color;
                        }
                        if shape.filled && shape.fill == was {
                            shape.fill = color;
                        }
                        if let Some(gradient) = shape.gradient.as_mut().filter(|gradient| gradient.to == was) {
                            gradient.to = color;
                        }
                    }
                });
                if self.palette == Some(Paint::Recolor(was)) {
                    self.palette = Some(Paint::Recolor(color));
                }
            }
        }
    }

    fn act(&mut self, action: Action) -> Outcome {
        // Remembered for Ctrl+K, by what it says there.
        let says = self.says_of(menu::MenuAct::Act(action));
        let outcome = self.act_now(action);
        self.remember(says);
        outcome
    }

    fn act_now(&mut self, action: Action) -> Outcome {
        self.dirty = true;
        match action {
            Action::Export => self.export(),
            Action::Copy => self.copy(),
            Action::FineTune(row) => self.open_fine(usize::from(row)),
            Action::CopyFile => self.copy_file(),
            Action::Capture => return Outcome::Capture,
            Action::Crop => {
                self.cropping = match self.cropping {
                    Some(_) => None,
                    None => self.picked().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_))),
                }
            }
            Action::FlipAcross | Action::FlipDown => {
                let across = action == Action::FlipAcross;
                let pictures = self.chosen().iter().any(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_)));
                if pictures {
                    self.restyle(|layer| {
                        if let Kind::Picture(picture) = &mut layer.kind {
                            if across {
                                picture.flip.0 = !picture.flip.0;
                            } else {
                                picture.flip.1 = !picture.flip.1;
                            }
                        }
                    });
                } else {
                    self.flip_shapes(across);
                }
            }
            // The size its crop is in its own pixels: one to one.
            Action::Actual => self.restyle(|layer| {
                if let Kind::Picture(picture) = &mut layer.kind {
                    // Its window's bar as large as on its own screen.
                    let (_, _, w, h) = picture.shown_crop();
                    (picture.w, picture.h) = (w, h + picture.drawn_style().bar() * picture.screen_scale);
                }
            }),
            Action::AddStack => self.add_stack(),
            Action::RemoveStack => self.remove_stack(),
            Action::Forward => self.restack(true),
            Action::Backward => self.restack(false),
            Action::Duplicate => self.duplicate(),
            Action::Delete => self.delete(),
            Action::Align(way) => self.align(way),
            Action::Halo => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.halo = !shape.halo;
                }
            }),
            other => return self.act_more(other),
        }
        Outcome::Continue
    }

    fn set_tool(&mut self, tool: Tool) {
        self.commit_text();
        self.cropping = None;
        let changed = (self.tool, self.framing) != (tool, false);
        (self.tool, self.framing) = (tool, false);
        if tool != Tool::Select {
            self.frame_picked = false;
        }
        if tool != Tool::Select {
            self.selected.clear();
        }
        if changed {
            self.say_tool();
        }
        self.dirty = true;
    }

    pub fn released(&mut self) {
        self.crop_window();
        self.fine_released();
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        self.dirty = true;
        // On a board, what was drawn or pulled out is typed into.
        if self.on_board() && self.board_released(drag) {
            self.held.clear();
            self.guides.clear();
            self.gaps.clear();
            return;
        }
        // A handle, a crop or a number taken hold of and let go of where it
        // was leaves no step to undo that would undo nothing.
        if matches!(drag, Drag::Scale { .. } | Drag::Turn { .. } | Drag::Crop { .. } | Drag::Scrub { .. } | Drag::Radius { .. }) && self.version == self.pressed_at {
            self.undo.pop();
        }
        match drag {
            Drag::Reorder { index, moved: true, .. } => self.drop_layer(index),
            Drag::ReorderFrame { index, moved: true, .. } => self.drop_frame(index),
            Drag::Guide { index } => {
                let at = self.pointer;
                self.drop_guide(index, at);
            }
            // A frame dragged out; a click with the frame tool, one the size
            // of the one worked in, there.
            Drag::NewFrame { from } => {
                let to = self.to_page(self.pointer);
                let (w, h) = ((to.0 - from.0).abs(), (to.1 - from.1).abs());
                let size = if w * self.zoom < 4.0 && h * self.zoom < 4.0 { (self.frame_like().width as f32, self.frame_like().height as f32) } else { (w, h) };
                self.new_frame((from.0.min(to.0), from.1.min(to.1)), size);
                self.framing = false;
            }
            Drag::Draw { .. } => {
                let slight = matches!(self.doc.layers.last(), Some(Layer { kind: Kind::Shape(shape), .. }) if shape.is_slight());
                if slight {
                    self.doc.layers.pop();
                    self.undo.pop();
                } else if !matches!(self.tool, Tool::Pen | Tool::Marker | Tool::Counter) {
                    self.selected = vec![self.doc.layers.len() - 1];
                    self.tool = Tool::Select;
                    // Its ends stuck to what they were put down on.
                    self.stick(self.doc.layers.len() - 1, &[0, 1]);
                }
            }
            Drag::Scale { pull, .. } if pull.0 == END => {
                if let Some(index) = self.picked() {
                    self.stick(index, &[pull.1 as usize]);
                }
            }
            Drag::Marquee { from } => {
                let to = self.to_doc(self.pointer);
                let (left, top, right, bottom) = (from.0.min(to.0), from.1.min(to.1), from.0.max(to.0), from.1.max(to.1));
                for index in 0..self.doc.layers.len() {
                    let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
                    let layer = &self.doc.layers[index];
                    if !layer.hidden && !layer.locked && x < right && x + w > left && y < bottom && y + h > top && !self.selected.contains(&index) {
                        self.selected.push(index);
                    }
                }
                if !self.mods.ctrl {
                    self.marquee_stacks((left, top, right, bottom));
                }
            }
            // Let go in a stack, or well out of the one it was in, laid out
            // again either way; and in the frame it is let go over.
            Drag::Move { moved: true, .. } => {
                if !self.doc.stacks.is_empty() {
                    self.drop_into_stacks();
                    self.changed();
                }
                self.drop_into_place();
            }
            Drag::Move { moved: false, .. } => {
                if let Some(index) = self.under_press.take() {
                    self.selected = vec![index];
                }
            }
            Drag::Scale { .. } if !self.doc.stacks.is_empty() => self.changed(),
            _ => {}
        }
        if self.on_board() {
            self.board_after_release(drag);
        }
        self.held.clear();
        self.guides.clear();
        self.gaps.clear();
        self.crop_snaps.clear();
    }

    /// What the ends of arrows can be stuck to: the pictures, and the
    /// boxes, ovals and text, that are to be seen.
    fn anchors(&mut self) -> Vec<shapes::Anchor> {
        let mut anchors = Vec::new();
        for layer in self.doc.layers.iter().filter(|layer| !layer.hidden) {
            match &layer.kind {
                Kind::Picture(picture) => anchors.push(shapes::Anchor { id: picture.id, middle: (picture.cx, picture.cy), half: (picture.w / 2.0, picture.h / 2.0), angle: picture.angle, round: false }),
                Kind::Shape(shape) => anchors.extend(shape.anchor(&mut self.text)),
            }
        }
        anchors
    }

    /// Ends of a line stuck to what they are on, as Figma's connectors
    /// are, to go with it when it moves.
    fn stick(&mut self, index: usize, ends: &[usize]) {
        let anchors = self.anchors();
        let reach = 8.0 / self.zoom;
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            for &end in ends {
                let at = shape.points[end];
                shape.stick(end, at, &anchors, reach);
            }
        }
        self.changed();
    }

    /// The ends of arrows taken to what they are stuck to, where it is now.
    fn follow(&mut self) {
        let stuck = |layer: &Layer| matches!(&layer.kind, Kind::Shape(shape) if shape.stuck.iter().any(Option::is_some));
        if self.doc.layers.iter().any(stuck) {
            let anchors = self.anchors();
            let lines = self.doc.layers.iter_mut().filter_map(|layer| match &mut layer.kind {
                Kind::Shape(shape) => Some(shape),
                Kind::Picture(_) => None,
            });
            shapes::follow(lines, &anchors);
        }
    }

    /// The lines among what is held let go of what they are stuck to that
    /// is not held with them, as they are moved without it.
    fn let_go(&mut self) {
        let id = |layer: &Layer| match &layer.kind {
            Kind::Picture(picture) => picture.id,
            Kind::Shape(shape) => shape.id,
        };
        let ids: Vec<u32> = self.held.iter().map(id).collect();
        let chosen = self.chosen();
        for (at, layer) in self.held.iter_mut().enumerate() {
            let Kind::Shape(shape) = &mut layer.kind else { continue };
            for end in 0..2 {
                if shape.stuck[end].is_some_and(|(id, ..)| !ids.contains(&id)) {
                    shape.stuck[end] = None;
                    if let Some(Layer { kind: Kind::Shape(placed), .. }) = chosen.get(at).and_then(|&index| self.doc.layers.get_mut(index)) {
                        placed.stuck[end] = None;
                    }
                }
            }
        }
    }

    /// The one picked out pulled by a handle to a point: the side or
    /// corner held goes there, across the picture unturned, and the one
    /// across from it stays where it is; with Shift, it keeps its shape.
    fn scale_to(&mut self, pull: (i32, i32), from: (f32, f32), at: (f32, f32), mods: Mods) {
        let Some(index) = self.picked() else { return };
        let Some(before) = self.held.first().cloned() else { return };
        // Widths and heights the rest have, for a side pulled to stop at,
        // in what is worked in and on the page round it.
        let mut others = self.other_edges();
        others.extend(self.outside_edges());
        let (widths, heights): (Vec<f32>, Vec<f32>) = others.iter().map(|edges| (edges[2] - edges[0], edges[3] - edges[1])).unzip();
        let reach = 6.0 / self.zoom;
        let fit = |size: f32, sizes: &[f32], pulled: bool| if pulled { snap::nearest_size(size, sizes, reach).unwrap_or(size) } else { size };
        let mut layer = before.clone();
        match &mut layer.kind {
            Kind::Picture(picture) => {
                let Kind::Picture(was) = &before.kind else { return };
                let (fx, fy) = was.local(from);
                let (tx, ty) = was.local(at);
                let (mut w, mut h) = (was.w + pull.0 as f32 * (tx - fx), was.h + pull.1 as f32 * (ty - fy));
                if was.angle == 0.0 {
                    (w, h) = (fit(w, &widths, pull.0 != 0), fit(h, &heights, pull.1 != 0 && pull.0 == 0));
                }
                // Keeping its shape, as Figma does with Shift; a corner of a
                // picture is held to its shape unless Shift says not.
                let keep = mods.shift != (pull.0 != 0 && pull.1 != 0);
                if keep {
                    let ratio = was.w / was.h.max(1e-3);
                    match pull {
                        (0, _) => w = h * ratio,
                        (_, 0) => h = w / ratio,
                        _ => {
                            if (w / was.w) > (h / was.h) {
                                h = w / ratio
                            } else {
                                w = h * ratio
                            }
                        }
                    }
                }
                (w, h) = (w.max(4.0), h.max(4.0));
                // What stays where it was: the side across from the one
                // held, or with neither held that way, the middle.
                let still = (-pull.0 as f32 * was.w / 2.0, -pull.1 as f32 * was.h / 2.0);
                let now = (-pull.0 as f32 * w / 2.0, -pull.1 as f32 * h / 2.0);
                let (sin, cos) = was.angle.sin_cos();
                let (dx, dy) = (still.0 - now.0, still.1 - now.1);
                (picture.w, picture.h) = (w, h);
                (picture.cx, picture.cy) = (was.cx + dx * cos - dy * sin, was.cy + dx * sin + dy * cos);
            }
            Kind::Shape(shape) if shape.tool == Tool::Zoom => {
                if let Kind::Shape(was) = &before.kind {
                    effects::pull_zoom(shape, was, pull, from, at);
                }
            }
            // An end of a line where the pointer is; with Shift, along
            // one of the eight ways from the other end.
            Kind::Shape(shape) if pull.0 == END => {
                // Let go of what it was stuck to while it is moved; it is
                // stuck to what it is let go over.
                let end = pull.1 as usize;
                shape.stuck[end] = None;
                let to = if mods.shift { shapes::constrained(Tool::Line, shape.points[1 - end], at) } else { at };
                shape.points[end] = to;
                shape.touch();
            }
            // Text by a corner: larger or smaller, by how much farther
            // from its middle the corner is.
            Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge) => {
                let middle = shape.centre(&mut self.text);
                let away = |(x, y): (f32, f32)| ((x - middle.0).powi(2) + (y - middle.1).powi(2)).sqrt().max(1.0);
                let times = away(at) / away(from);
                let before = shape.font_px;
                shape.font_px = ((before as f32 * times).round() as u32).clamp(6, 600);
                shape.touch();
                // Grown about its middle, which stays where it was.
                let now = shape.centre(&mut self.text);
                shape.translate(middle.0 - now.0, middle.1 - now.1);
            }
            // A box or an oval by a side or a corner, across it as it is
            // unturned, the side across from it staying where it was; with
            // Shift, keeping its shape.
            Kind::Shape(shape) => {
                let (x, y, w, h) = shape.frame();
                let middle = (x + w / 2.0, y + h / 2.0);
                let unturned = |point: (f32, f32)| shapes::turn_point(point, middle, -shape.angle);
                let (a, b) = (unturned(from), unturned(at));
                let (fx, fy) = (b.0 - a.0, b.1 - a.1);
                let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
                match pull.0 {
                    -1 => left = (left + fx).min(right - 1.0),
                    1 => right = (right + fx).max(left + 1.0),
                    _ => {}
                }
                match pull.1 {
                    -1 => top = (top + fy).min(bottom - 1.0),
                    1 => bottom = (bottom + fy).max(top + 1.0),
                    _ => {}
                }
                // A side pulled stops at a width or a height another has;
                // their boxes are a little larger than their frames, by
                // half the line round them and a little.
                if shape.angle == 0.0 {
                    let margin = shape.stroke + 4.0;
                    let width = fit(right - left + margin, &widths, pull.0 != 0) - margin;
                    let height = fit(bottom - top + margin, &heights, pull.1 != 0) - margin;
                    if pull.0 < 0 { left = right - width } else if pull.0 > 0 { right = left + width }
                    if pull.1 < 0 { top = bottom - height } else if pull.1 > 0 { bottom = top + height }
                }
                if mods.shift && pull.0 != 0 && pull.1 != 0 && w > 0.0 && h > 0.0 {
                    let ratio = w / h;
                    let (nw, nh) = (right - left, bottom - top);
                    let (nw, nh) = if nw / w > nh / h { (nw, nw / ratio) } else { (nh * ratio, nh) };
                    if pull.0 < 0 { left = right - nw } else { right = left + nw }
                    if pull.1 < 0 { top = bottom - nh } else { bottom = top + nh }
                }
                // The middle of what it is now, as it is turned; and the
                // box about it, which turns about it.
                let now = shapes::turn_point(((left + right) / 2.0, (top + bottom) / 2.0), middle, shape.angle);
                let (hw, hh) = ((right - left) / 2.0, (bottom - top) / 2.0);
                shape.points[0] = (now.0 - hw, now.1 - hh);
                let far = shape.points.len() - 1;
                shape.points[far.min(1)] = (now.0 + hw, now.1 + hh);
                shape.touch();
            }
        }
        self.doc.layers[index] = layer;
        self.changed_layers(&[index]);
    }

    /// The crop of the picture being cropped pulled by a handle, or moved
    /// whole: in the picture's own pixels, kept within it.
    fn crop_to(&mut self, pull: (i32, i32), from: (f32, f32), at: (f32, f32), mods: Mods) {
        let Some(index) = self.cropping else { return };
        let Some(Layer { kind: Kind::Picture(was), .. }) = self.held.first().cloned() else { return };
        let (fx, fy) = was.local(from);
        let (tx, ty) = was.local(at);
        // How many of its own pixels each of the artboard's is.
        let (per_x, per_y) = (was.crop.2 / was.w.max(1e-3), was.crop.3 / was.h.max(1e-3));
        let (dx, dy) = ((tx - fx) * per_x * if was.flip.0 { -1.0 } else { 1.0 }, (ty - fy) * per_y * if was.flip.1 { -1.0 } else { 1.0 });
        let (iw, ih) = (was.image.width() as f32, was.image.height() as f32);
        let (mut left, mut top, mut right, mut bottom) = (was.crop.0, was.crop.1, was.crop.0 + was.crop.2, was.crop.1 + was.crop.3);
        match pull {
            (0, 0) => {
                // The picture dragged one way inside it, the crop goes the
                // other over the picture, as far as its edge.
                let (dx, dy) = (dx.clamp(right - iw, left), dy.clamp(bottom - ih, top));
                (left, right, top, bottom) = (left - dx, right - dx, top - dy, bottom - dy);
            }
            _ => {
                match pull.0 {
                    -1 => left = (left + dx).clamp(0.0, right - 8.0),
                    1 => right = (right + dx).clamp(left + 8.0, iw),
                    _ => {}
                }
                match pull.1 {
                    -1 => top = (top + dy).clamp(0.0, bottom - 8.0),
                    1 => bottom = (bottom + dy).clamp(top + 8.0, ih),
                    _ => {}
                }
            }
        }
        // Onto a line in the picture near enough, unless Ctrl is held; the
        // side or corner pulled first, the other kept to its shape after.
        self.crop_snaps.clear();
        if !mods.ctrl {
            let reach = (6.0 / self.zoom * per_x, 6.0 / self.zoom * per_y);
            (left, top, right, bottom) = self.snap_crop(&was.image, (left, top, right, bottom), pull, reach);
        }
        // Kept to a shape: the one chosen over the picture, or with Shift
        // the one it had, as Figma keeps it. The side or corner across
        // stays where it was, and a side pulled keeps its middle.
        let ratio = match panel::CROP_ASPECTS[self.crop_aspect].1 {
            chosen if chosen > 0.0 => Some(chosen),
            chosen if chosen < 0.0 => Some(iw / ih),
            _ if mods.shift => Some(was.crop.2 / was.crop.3.max(1e-3)),
            _ => None,
        };
        if let (Some(ratio), false) = (ratio, pull == (0, 0)) {
            let (mut w, mut h) = (right - left, bottom - top);
            match pull {
                (_, 0) => h = w / ratio,
                (0, _) => w = h * ratio,
                _ if w / was.crop.2.max(1e-3) > h / was.crop.3.max(1e-3) => h = w / ratio,
                _ => w = h * ratio,
            }
            let (mx, my) = (was.crop.0 + was.crop.2 / 2.0, was.crop.1 + was.crop.3 / 2.0);
            let room = |pulled: i32, low: f32, high: f32, middle: f32, whole: f32| match pulled {
                -1 => high,
                1 => whole - low,
                _ => 2.0 * middle.min(whole - middle),
            };
            let (room_w, room_h) = (room(pull.0, left, right, mx, iw), room(pull.1, top, bottom, my, ih));
            let shrink = (room_w / w.max(1e-3)).min(room_h / h.max(1e-3)).min(1.0);
            (w, h) = ((w * shrink).max(8.0), (h * shrink).max(8.0));
            let place = |pulled: i32, low: f32, high: f32, middle: f32, size: f32| match pulled {
                -1 => (high - size, high),
                1 => (low, low + size),
                _ => (middle - size / 2.0, middle + size / 2.0),
            };
            (left, right) = place(pull.0, left, right, mx, w);
            (top, bottom) = place(pull.1, top, bottom, my, h);
        }
        if !mods.ctrl {
            self.show_crop_snaps(&was, (left, top, right, bottom));
        }
        let mut picture = was.clone();
        picture.crop = (left, top, right - left, bottom - top);
        // Shown at the same size per pixel, the side across from the one
        // pulled staying where it was.
        (picture.w, picture.h) = (picture.crop.2 / per_x, picture.crop.3 / per_y);
        if pull != (0, 0) {
            let still = (-pull.0 as f32 * was.w / 2.0, -pull.1 as f32 * was.h / 2.0);
            let now = (-pull.0 as f32 * picture.w / 2.0, -pull.1 as f32 * picture.h / 2.0);
            let (sin, cos) = was.angle.sin_cos();
            let (sx, sy) = (still.0 - now.0, still.1 - now.1);
            (picture.cx, picture.cy) = (was.cx + sx * cos - sy * sin, was.cy + sx * sin + sy * cos);
        }
        self.doc.layers[index].kind = Kind::Picture(picture);
        self.changed_layers(&[index]);
    }

    /// A number of what is picked out, or of the artboard.
    fn field(&mut self, field: Field) -> Option<f32> {
        if let Field::StackGap | Field::StackPad = field {
            let stack = &self.doc.stacks[self.picked_stack()?];
            return Some(if field == Field::StackGap { stack.gap } else { stack.pad });
        }
        // A stack picked out is as large as its box.
        if let (Field::W | Field::H, Some(index)) = (field, self.picked_stack()) {
            let stack = &self.doc.stacks[index];
            return Some(if field == Field::W { stack.w } else { stack.h });
        }
        if let Field::ArtW | Field::ArtH = field {
            return Some(if field == Field::ArtW { self.doc.width } else { self.doc.height } as f32);
        }
        if field == Field::Angle {
            return match self.doc.background {
                Fill::Gradient(_, _, angle) => Some(angle),
                _ => None,
            };
        }
        if let Field::ExportTimes(index) = field {
            return self.doc.exports.get(usize::from(index)).map(|size| size.times);
        }
        if field == Field::FillAngle {
            return self.fill_angle();
        }
        if let Field::FrameX | Field::FrameY = field {
            return Some(if field == Field::FrameX { self.doc.x } else { self.doc.y });
        }
        let chosen = self.chosen();
        if chosen.len() > 1 {
            let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
            let all = actions::union(&boxes)?;
            return match field {
                Field::X => Some(all.0),
                Field::Y => Some(all.1),
                Field::W => Some(all.2),
                Field::H => Some(all.3),
                _ => None,
            };
        }
        let index = *chosen.first()?;
        if let Field::DropY | Field::DropBlur | Field::DropSpread | Field::DropAlpha = field {
            let drop = self.doc.layers[index].drop()?;
            return Some(match field {
                Field::DropY => f32::from(drop.y),
                Field::DropBlur => f32::from(drop.blur),
                Field::DropSpread => f32::from(drop.spread),
                _ => (drop.color >> 24) as f32 * 100.0 / 255.0,
            });
        }
        let bounds = self.doc.layers[index].bounds(&mut self.text);
        Some(match &self.doc.layers[index].kind {
            Kind::Picture(picture) => match field {
                Field::X => picture.cx - picture.w / 2.0,
                Field::Y => picture.cy - picture.h / 2.0,
                Field::W => picture.w,
                Field::H => picture.h,
                Field::Turn => picture.angle.to_degrees(),
                Field::Radius => return self.radius_field(None),
                Field::Corner(corner) => return self.radius_field(Some(usize::from(corner))),
                Field::Opacity => picture.opacity as f32 * 100.0 / 256.0,
                Field::Exposure => picture.adjust[0] * 100.0,
                Field::Contrast => picture.adjust[1] * 100.0,
                Field::Saturation => picture.adjust[2] * 100.0,
                _ => return None,
            },
            Kind::Shape(shape) => match field {
                Field::X => bounds.0,
                Field::Y => bounds.1,
                Field::W => shape.frame().2,
                Field::H => shape.frame().3,
                Field::Turn => shape.angle.to_degrees(),
                Field::Radius => return self.radius_field(None),
                Field::Corner(corner) => return self.radius_field(Some(usize::from(corner))),
                Field::Opacity => shape.opacity as f32 * 100.0 / 256.0,
                Field::FontSize => shape.font_px as f32,
                Field::Stroke => shape.stroke,
                Field::Strength => effects::strength(shape),
                Field::Dim => effects::dim(shape) as f32,
                Field::Magnify if shape.tool == Tool::Zoom => shape.zoom_times(),
                Field::StartX => shape.points[0].0,
                Field::StartY => shape.points[0].1,
                Field::EndX => shape.points[1.min(shape.points.len() - 1)].0,
                Field::EndY => shape.points[1.min(shape.points.len() - 1)].1,
                Field::Length => {
                    let (a, b) = (shape.points[0], shape.points[1.min(shape.points.len() - 1)]);
                    (b.0 - a.0).hypot(b.1 - a.1)
                }
                Field::LineAngle => {
                    let (a, b) = (shape.points[0], shape.points[1.min(shape.points.len() - 1)]);
                    (b.1 - a.1).atan2(b.0 - a.0).to_degrees()
                }
                _ => return None,
            },
        })
    }

    fn set_field(&mut self, field: Field, value: f32) {
        match field {
            Field::StackGap | Field::StackPad => {
                if let Some(index) = self.picked_stack() {
                    let stack = &mut self.doc.stacks[index];
                    match field {
                        Field::StackGap => stack.gap = value.round().clamp(-1000.0, 1000.0),
                        _ => stack.pad = value.round().clamp(0.0, 1000.0),
                    }
                }
            }
            // A stack's box made a size of its own: no longer hugging what
            // is in it, as Figma's goes Fixed.
            Field::W | Field::H if self.picked_stack().is_some() => {
                if let Some(index) = self.picked_stack() {
                    let stack = &mut self.doc.stacks[index];
                    stack.sizing = stacks::FIXED;
                    match field {
                        Field::W => stack.w = value.round().max(1.0),
                        _ => stack.h = value.round().max(1.0),
                    }
                }
            }
            // All the corners, or one; and the next rectangle as round.
            Field::Radius => self.set_radius_field(None, value),
            Field::Corner(corner) => self.set_radius_field(Some(usize::from(corner)), value),
            Field::FillAngle => self.set_fill_angle(value),
            Field::ExportTimes(index) => {
                if let Some(size) = self.doc.exports.get_mut(usize::from(index)) {
                    size.times = (value * 2.0).round().clamp(1.0, 16.0) / 2.0;
                }
            }
            Field::FrameX => self.doc.x = value.round(),
            Field::FrameY => self.doc.y = value.round(),
            Field::W | Field::H if self.chosen().len() > 1 => self.scale_selection(field == Field::W, value),
            Field::ArtW => self.doc.width = (value.round() as i32).clamp(16, 8000),
            Field::ArtH => self.doc.height = (value.round() as i32).clamp(16, 8000),
            Field::Angle => {
                if let Fill::Gradient(from, to, _) = self.doc.background {
                    self.doc.background = Fill::Gradient(from, to, value.round().rem_euclid(360.0));
                }
            }
            _ => {
                let Some(now) = self.field(field) else { return };
                let chosen = self.chosen();
                let keep = self.keep_ratio;
                for index in chosen {
                    let layer = &mut self.doc.layers[index];
                    match (&mut layer.kind, field) {
                        (_, Field::X) => layer.translate((value - now).round(), 0.0),
                        (_, Field::Y) => layer.translate(0.0, (value - now).round()),
                        (Kind::Picture(picture), Field::W) => {
                            let ratio = picture.h / picture.w.max(1e-3);
                            picture.w = value.max(4.0);
                            if keep {
                                picture.h = picture.w * ratio;
                            }
                        }
                        (Kind::Picture(picture), Field::H) => {
                            let ratio = picture.w / picture.h.max(1e-3);
                            picture.h = value.max(4.0);
                            if keep {
                                picture.w = picture.h * ratio;
                            }
                        }
                        (Kind::Picture(picture), Field::Exposure | Field::Contrast | Field::Saturation) => {
                            let which = [Field::Exposure, Field::Contrast, Field::Saturation].iter().position(|known| *known == field).unwrap_or(0);
                            picture.adjust[which] = (value / 100.0).clamp(-1.0, 1.0);
                        }
                        (_, Field::DropY | Field::DropBlur | Field::DropSpread | Field::DropAlpha) => {
                            let number = value.round().clamp(-400.0, 400.0) as i16;
                            layer.change_drop(|drop| match field {
                                Field::DropY => drop.y = number,
                                Field::DropBlur => drop.blur = number.max(0),
                                Field::DropSpread => drop.spread = number,
                                _ => drop.color = (drop.color & 0xFF_FFFF) | (((value.clamp(0.0, 100.0) * 2.55).round() as u32) << 24),
                            });
                        }
                        (Kind::Shape(shape), Field::Length | Field::LineAngle) => {
                            let far = 1.min(shape.points.len() - 1);
                            let (a, b) = (shape.points[0], shape.points[far]);
                            let (length, angle) = ((b.0 - a.0).hypot(b.1 - a.1), (b.1 - a.1).atan2(b.0 - a.0));
                            let (length, angle) = if field == Field::Length { (value.max(1.0), angle) } else { (length, value.to_radians()) };
                            shape.points[far] = ((a.0 + length * angle.cos()).round(), (a.1 + length * angle.sin()).round());
                            shape.points.truncate(2);
                            shape.stuck[1] = None;
                        }
                        (Kind::Shape(shape), Field::FontSize) => shape.font_px = (value.round() as u32).clamp(6, 400),
                        (Kind::Shape(shape), Field::Stroke) => shape.stroke = (value * 2.0).round().clamp(1.0, 80.0) / 2.0,
                        (Kind::Shape(shape), Field::Strength) => shape.stroke = effects::stroke_for(value),
                        (Kind::Shape(shape), Field::Dim) => shape.number = value.round().clamp(1.0, 100.0) as u32,
                        (Kind::Shape(shape), Field::Magnify) if shape.tool == Tool::Zoom => effects::magnify(shape, value),
                        (Kind::Shape(shape), Field::StartX | Field::StartY | Field::EndX | Field::EndY) => {
                            let at = if matches!(field, Field::StartX | Field::StartY) { 0 } else { 1.min(shape.points.len() - 1) };
                            if matches!(field, Field::StartX | Field::EndX) {
                                shape.points[at].0 = value.round();
                            } else {
                                shape.points[at].1 = value.round();
                            }
                            if let Some(end) = [0, 1].get(at) {
                                shape.stuck[*end] = None;
                            }
                        }
                        (Kind::Picture(picture), Field::Turn) => picture.angle = value.to_radians(),
                        (Kind::Picture(picture), Field::Opacity) => picture.opacity = ((value.clamp(0.0, 100.0) * 2.56).round() as u32).min(256),
                        (Kind::Shape(shape), Field::W | Field::H) => {
                            let (x, y, w, h) = shape.frame();
                            let (mut w2, mut h2) = if field == Field::W { (value.max(1.0), h) } else { (w, value.max(1.0)) };
                            if keep && w > 0.0 && h > 0.0 {
                                if field == Field::W {
                                    h2 = w2 * h / w;
                                } else {
                                    w2 = h2 * w / h;
                                }
                            }
                            shape.points[0] = (x, y);
                            shape.points[1] = (x + w2, y + h2);
                        }
                        (Kind::Shape(shape), Field::Turn) => shape.angle = value.to_radians(),
                        (Kind::Shape(shape), Field::Opacity) => shape.opacity = ((value.clamp(0.0, 100.0) * 2.56).round() as u32).min(256),
                        _ => {}
                    }
                    if let Kind::Shape(shape) = &mut layer.kind {
                        shape.touch();
                    }
                }
            }
        }
        self.changed();
    }

    fn commit_typing(&mut self) {
        if let Some((field, typed)) = self.typing.take() {
            if let Ok(value) = typed.parse::<f32>() {
                self.checkpoint();
                self.set_field(field, value);
            }
        }
    }

    /// The name being typed given to what it names; none given, none.
    fn commit_name(&mut self) {
        let Some((naming, typed)) = self.renaming.take() else { return };
        let typed = typed.trim().to_owned();
        // A frame or a page is called something; a title or a suffix may
        // be nothing.
        if naming == Naming::Filter {
            self.layer_filter = typed;
            return;
        }
        if typed.is_empty() && matches!(naming, Naming::Frame | Naming::Page(_) | Naming::Layer(_) | Naming::Stack(_)) {
            return;
        }
        // A color typed as hex: the paint the picker is open for made it.
        if naming == Naming::Hex {
            let hex = typed.trim_start_matches('#');
            if let (Some(paint), Ok(color)) = (self.palette, u32::from_str_radix(hex, 16)) {
                let color = if hex.len() == 3 { (color >> 8 & 0xF) * 0x110000 | (color >> 4 & 0xF) * 0x1100 | (color & 0xF) * 0x11 } else { color & 0xFF_FFFF };
                self.paint(paint, Some(color));
                self.hsv = picker::to_hsv(color);
            }
            return;
        }
        self.checkpoint();
        match naming {
            Naming::Frame => self.doc.name = typed,
            Naming::Page(page) => {
                if let Some(page) = self.pages.get_mut(page) {
                    page.name = typed;
                }
            }
            Naming::Title(index) => {
                if let Some(Layer { kind: Kind::Picture(picture), .. }) = self.doc.layers.get_mut(index) {
                    picture.title = typed;
                }
            }
            Naming::Address(index) => {
                if let Some(Layer { kind: Kind::Picture(picture), .. }) = self.doc.layers.get_mut(index) {
                    if let Some(chrome) = &mut picture.chrome {
                        chrome.address = typed;
                    }
                }
            }
            Naming::Hex | Naming::Filter => {}
            Naming::Stack(id) => self.rename_stack(id, typed),
            Naming::Layer(index) => {
                if let Some(layer) = self.doc.layers.get_mut(index) {
                    layer.rename(typed);
                }
            }
            Naming::Suffix(index) => {
                if let Some(size) = self.doc.exports.get_mut(index) {
                    size.suffix = typed.replace([' ', ':', '/'], "_");
                }
            }
        }
        self.changed();
    }

    /// Text taken to be typed into, by its layer: its headings show
    /// their hashes while it is.
    fn edit_text(&mut self, index: usize) {
        (self.editing, self.words_picked) = (Some(index), false);
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.editing = true;
            shape.touch();
        }
    }

    fn commit_text(&mut self) {
        self.words_picked = false;
        if let Some(index) = self.editing.take() {
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                shape.editing = false;
                shape.touch();
            }
            if matches!(&self.doc.layers.get(index), Some(Layer { kind: Kind::Shape(shape), .. }) if shape.is_slight()) {
                self.doc.layers.remove(index);
                self.selected.clear();
            }
            // The text tool puts down one text, as Figma's does: once it is
            // done with, by Esc or a click off it, the pointer is back.
            if self.tool == Tool::Text {
                self.tool = Tool::Select;
            }
            self.changed();
        }
    }

    // ---- What is done to what is picked out ----

    fn restack(&mut self, forward: bool) {
        // The text typed into and the picture cropped are known by where
        // they are among the layers, which is about to change.
        self.commit_text();
        self.cropping = None;
        // A frame picked out, or the page's own layers, among everything
        // on the page, frames and layers alike.
        if self.restack_page(forward, self.mods.shift) {
            return;
        }
        let mut chosen = self.chosen();
        chosen.sort_unstable();
        if chosen.is_empty() {
            return;
        }
        self.checkpoint();
        let all = self.mods.shift;
        let count = self.doc.layers.len();
        let mut order: Vec<usize> = (0..count).collect();
        let picked = |index: usize| chosen.binary_search(&index).is_ok();
        if all {
            order.sort_by_key(|&index| picked(index) == forward);
        } else if forward {
            for at in (0..count.saturating_sub(1)).rev() {
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
        let layers = std::mem::take(&mut self.doc.layers);
        self.doc.layers = order.iter().map(|&index| layers[index].clone()).collect();
        self.selected = order.iter().enumerate().filter(|(_, &index)| picked(index)).map(|(at, _)| at).collect();
        self.changed();
    }

    fn delete(&mut self) {
        let mut chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        self.checkpoint();
        chosen.sort_unstable();
        for index in chosen.into_iter().rev() {
            self.doc.layers.remove(index);
        }
        self.selected.clear();
        (self.cropping, self.editing) = (None, None);
        self.changed();
    }

    /// What is picked out lined up by its edges or middles: with others,
    /// on the box round them all; alone, on the artboard.
    fn align(&mut self, way: u8) {
        let chosen = self.chosen();
        // One of the page's own layers alone has no frame to line up on.
        if chosen.is_empty() || (chosen.len() == 1 && self.on_page()) {
            return;
        }
        let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let (left, top, right, bottom) = match boxes.len() {
            1 => (0.0, 0.0, self.doc.width as f32, self.doc.height as f32),
            _ => boxes.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(l, t, r, b), &(x, y, w, h)| (l.min(x), t.min(y), r.max(x + w), b.max(y + h))),
        };
        self.checkpoint();
        for (&index, &(x, y, w, h)) in chosen.iter().zip(&boxes) {
            let (dx, dy) = match way {
                0 => (left - x, 0.0),
                1 => ((left + right) / 2.0 - (x + w / 2.0), 0.0),
                2 => (right - (x + w), 0.0),
                3 => (0.0, top - y),
                4 => (0.0, (top + bottom) / 2.0 - (y + h / 2.0)),
                _ => (0.0, bottom - (y + h)),
            };
            self.doc.layers[index].translate(dx.round(), dy.round());
        }
        self.changed();
    }

    // ---- In and out ----

    /// A picture put on the artboard: in the middle of the view, made to
    /// fit in it if it is larger, and picked out.
    pub fn add_picture(&mut self, image: Image, name: &str) {
        self.drop_picture(image, name, None);
    }

    /// A picture put where it was dropped in the window, in the frame
    /// there or on the page outside every frame; or with nowhere said, in
    /// the middle of the view.
    pub fn drop_picture(&mut self, image: Image, name: &str, at: Option<(i32, i32)>) {
        self.checkpoint();
        let at = at.filter(|&at| self.over_page(at)).unwrap_or_else(|| self.open_area().center());
        self.work_at(at);
        let middle = self.to_doc(at);
        let image = Arc::new(image);
        let mut picture = Picture::new(image, name.to_owned(), middle);
        // Kept inside a frame; on the page, as large as it is.
        let most = match self.on_page() {
            true => 1.0,
            false => (self.doc.width as f32 * 0.9 / picture.w).min(self.doc.height as f32 * 0.9 / picture.h).min(1.0),
        };
        (picture.w, picture.h) = (picture.w * most, picture.h * most);
        self.doc.layers.push(Layer::picture(picture));
        self.window_as_last(self.doc.layers.len() - 1);
        self.selected = vec![self.doc.layers.len() - 1];
        self.tool = Tool::Select;
        self.changed();
    }

    /// What is in the middle of the view worked in, for what is put there:
    /// the frame there, or outside every frame the page's own layers.
    fn work_where_seen(&mut self) {
        self.work_at(self.open_area().center());
    }

    /// The frame at a point of the window worked in, or outside every
    /// frame the page's own layers.
    fn work_at(&mut self, at: (i32, i32)) {
        let target = self.frame_at(self.to_page(at)).unwrap_or(LOOSE);
        self.activate(target);
    }

    /// What the clipboard had: a picture, put on the artboard; or text,
    /// put on it as text.
    pub fn pasted(&mut self, picture: Option<Vec<u8>>, text: Option<String>) {
        if let Some(image) = picture.as_deref().and_then(Image::open) {
            return self.add_picture(image, "Pasted");
        }
        if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
            if let Some(index) = self.editing {
                if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                    shape.text.push_str(&text);
                    shape.touch();
                }
            } else {
                self.checkpoint();
                self.work_where_seen();
                let middle = self.to_doc(self.open_area().center());
                let mut shape = Shape::new(Tool::Text, self.color, STROKES[self.size], middle);
                (shape.text, shape.font_px, shape.face) = (text, FONT_SIZES[self.size], self.face);
                self.doc.layers.push(Layer::shape(shape));
                self.selected = vec![self.doc.layers.len() - 1];
            }
            self.changed();
            return;
        }
        self.say("Nothing to paste");
    }

    /// What a capture of the screen came back with.
    pub fn captured(&mut self, file: &Path) {
        match std::fs::read(file).ok().as_deref().and_then(Image::open) {
            Some(image) => self.add_picture(image, "Capture"),
            None => self.say("No capture"),
        }
        let _ = std::fs::remove_file(file);
    }

    /// The artboard as a PNG, at the first size it is exported at.
    fn png(&mut self) -> Option<Vec<u8>> {
        let times = self.doc.exports.first().map_or(1.0, |size| size.times);
        let (wide, high, pixels) = self.doc.render(times, &mut self.text);
        let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
        crate::output::encode(wide as u32, high as u32, &pixels, clear).ok()
    }

    /// The folder exports go in: beside the file, in `exports` and a folder
    /// for its page; or, for a file kept nowhere yet, the pictures folder.
    fn export_dir(&self) -> PathBuf {
        match self.path.as_ref().and_then(|path| path.parent()) {
            Some(dir) => dir.join("exports").join(one_line(&self.pages[self.page].name).replace('/', "-")),
            None => self.dir.clone(),
        }
    }

    /// Frames of the page shown written out, each at every size it is set
    /// to be exported at, named for the frame and the size: in place of
    /// what was written for them last. Gives what was written.
    fn export_frames(&mut self, frames: &[usize]) -> Result<Vec<PathBuf>, String> {
        let dir = self.export_dir();
        std::fs::create_dir_all(&dir).map_err(|error| format!("cannot make {}: {error}", dir.display()))?;
        let mut written = Vec::new();
        let count = self.frame_count();
        for &index in frames.iter().filter(|&&index| index < count) {
            let frame = if index == self.active { self.doc.clone() } else { self.pages[self.page].frames[index].clone() };
            let sizes = if frame.exports.is_empty() { vec![ExportSize::new(1.0)] } else { frame.exports.clone() };
            for size in sizes {
                let (wide, high, pixels) = frame.render(size.times, &mut self.text);
                let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
                let bytes = crate::convert::encode(wide as u32, high as u32, &pixels, clear, None, size.format, &size.tuned.unwrap_or(self.convert))?.bytes;
                let stem = match self.path {
                    Some(_) => one_line(&frame.name).replace('/', "-"),
                    None => format!("{}-{}", crate::output::dated(&self.filename), one_line(&frame.name).replace('/', "-")),
                };
                let name = format!("{stem}{}.{}", size.file_suffix(), size.format.ending());
                let path = dir.join(name);
                std::fs::write(&path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
                debug!("canvas exported {}", path.display());
                written.push(path);
            }
        }
        Ok(written)
    }

    fn export(&mut self) {
        // The page's own layers go in no export, as Figma's do not.
        if self.on_page() {
            return self.say("Pick a frame to export it");
        }
        let active = self.active;
        match self.export_frames(&[active]) {
            Ok(written) if !written.is_empty() => self.show_exported(written),
            Ok(written) => self.say(format!("Exported {} pictures to {}", written.len(), shortened(&self.export_dir()))),
            Err(error) => self.say(error),
        }
    }

    /// The artboard on the clipboard, kept there by a hotshot of its own,
    /// as the pin does.
    fn copy(&mut self) {
        if self.on_page() {
            return self.say("Pick a frame to copy it");
        }
        let Some(png) = self.png() else { return };
        let kept = crate::output::state_file("pins").and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos());
            let file = dir.join(format!("{}-{stamp}-canvas.png", std::process::id()));
            std::fs::write(&file, &png).ok()?;
            Some(file)
        });
        match kept.map(|file| crate::pin::again(&["--clip".as_ref(), file.as_os_str(), "--forget".as_ref()])) {
            Some(Ok(())) => self.say("Copied"),
            _ => self.say("Could not copy"),
        }
    }

    /// Whatever is being typed kept, as the window goes or the file is
    /// left: a name being typed, a number, text.
    pub fn finish(&mut self) {
        self.commit_name();
        self.commit_typing();
        self.commit_text();
    }

    /// The canvas kept in its file, or a new one among the pictures.
    /// The file kept where it is kept, or as a new one in Drafts: written
    /// beside it first and put in its place, so that a file half written
    /// is never what is there. Said, when `loud`.
    pub fn save(&mut self, loud: bool) {
        self.end_preview();
        let path = self.path.clone().unwrap_or_else(|| crate::home::fresh_file(&crate::home::drafts(), "Untitled"));
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let beside = path.with_extension("hotshot~");
        match std::fs::write(&beside, write_pages(&self.gathered())).and_then(|()| std::fs::rename(&beside, &path)) {
            Ok(()) => {
                if loud {
                    self.say(format!("Saved {}", shortened(&path)));
                }
                debug!("canvas saved {}", path.display());
                (self.path, self.edited) = (Some(path), false);
            }
            Err(error) => self.say(format!("Could not save: {error}")),
        }
    }

    /// The text it draws with, handed back for the projects to draw with.
    pub fn into_text(self) -> Text {
        self.text
    }

    // ---- The keys ----

    pub fn key(&mut self, key: Key, mods: Mods, down: bool) -> Outcome {
        self.crop_window();
        if down {
            self.end_preview();
        }
        if down && self.on_board() {
            self.ghost_keys(key, mods);
        }
        // Alt with the box tool over nested boxes: the one round it.
        if down && self.on_board() && key == Key::Other && self.pick_out() {
            return Outcome::Continue;
        }
        // The history open takes the keys.
        if down && self.on_board() && self.history_key(key) {
            return Outcome::Continue;
        }
        if down && self.on_board() && self.present_key(key) {
            return Outcome::Continue;
        }
        // L held on a board is a laser.
        if self.on_board() && self.laser_wants(key, mods) {
            self.laser_key(down);
            return Outcome::Continue;
        }
        if self.search.is_some() {
            self.mods = mods;
            return if down { self.search_key(key, mods) } else { Outcome::Continue };
        }
        if let Some(outcome) = self.fine_key(key, mods, down) {
            self.mods = mods;
            return outcome;
        }
        self.mods = mods;
        self.dirty = true;
        // The library open takes what is typed, to search it by.
        if self.library.is_some() && !mods.ctrl {
            if down {
                self.library_key(key);
            }
            return Outcome::Continue;
        }
        // Undo and redo close any list open first, the colors and a group
        // of tools' too, as what they were open over may be gone.
        if down && mods.ctrl && matches!(key, Key::Char('z' | 'Z' | 'y' | 'Y')) {
            (self.menu, self.menu_row, self.flyout, self.palette, self.styling, self.eyedropping) = (None, None, None, None, false, false);
        }
        // An open list takes the keys, as Ctrl+K does: Up and Down, Enter,
        // Right and Left into and out of the list a row opens, and Esc;
        // a letter switches no tool while it is open. Ctrl keys still do
        // what they say.
        if self.menu.is_some() && !mods.ctrl {
            return if down { self.menu_key(key) } else { Outcome::Continue };
        }
        if key == Key::Char(' ') && self.editing.is_none() && self.typing.is_none() && self.renaming.is_none() {
            self.space = down;
            return Outcome::Continue;
        }
        if !down {
            return Outcome::Continue;
        }
        // A name being typed.
        if let Some((_, typed)) = &mut self.renaming {
            // Shift and the like come as keys of their own, and leave it
            // picked out.
            let whole = self.name_picked;
            self.name_picked &= key == Key::Other;
            match key {
                Key::Char(letter) if !mods.ctrl => {
                    if whole {
                        typed.clear();
                    }
                    typed.push(letter);
                }
                // Ctrl+Q and Ctrl+W still close the window, the name kept.
                Key::Char('q' | 'w' | 'Q' | 'W') if mods.ctrl => return Outcome::Quit,
                Key::Backspace if whole => typed.clear(),
                Key::Backspace => drop(typed.pop()),
                Key::Enter => self.commit_name(),
                Key::Escape => self.renaming = None,
                _ => {}
            }
            return Outcome::Continue;
        }
        // A number being typed into its field.
        if let Some((_, typed)) = &mut self.typing {
            match key {
                Key::Char(digit) if digit.is_ascii_digit() || digit == '.' || digit == '-' => typed.push(digit),
                Key::Backspace => drop(typed.pop()),
                Key::Enter => self.commit_typing(),
                Key::Escape => self.typing = None,
                _ => {}
            }
            return Outcome::Continue;
        }
        if self.editing.is_some() && self.on_board() {
            if let Some(outcome) = self.board_typing_key(key, mods) {
                return outcome;
            }
        }
        // Text being typed.
        if let (Some(index), false) = (self.editing, mods.ctrl) {
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                // What a label from the library says to begin with is
                // typed over by the first letter, as capture mode has it.
                let was = self.words_picked;
                let picked = was && key != Key::Other;
                self.words_picked = was && key == Key::Other;
                if picked && matches!(key, Key::Char(_) | Key::Backspace) {
                    shape.text.clear();
                }
                match key {
                    Key::Char(letter) => shape.text.push(letter),
                    Key::Backspace if picked => {}
                    Key::Enter => shape.text.push('\n'),
                    Key::Backspace => drop(shape.text.pop()),
                    Key::Escape => self.commit_text(),
                    _ => {}
                }
                if let Kind::Shape(shape) = &mut self.doc.layers.get_mut(index).map_or_else(|| Kind::Picture(Picture::new(Arc::new(Image::new(1, 1, vec![0])), String::new(), (0.0, 0.0))), |layer| layer.kind.clone()) {
                    shape.touch();
                }
                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                    shape.touch();
                }
                // A letter typed changes that text alone; Esc may have
                // done more.
                match key {
                    Key::Escape => self.changed(),
                    _ => self.changed_layers(&[index]),
                }
            }
            return Outcome::Continue;
        }
        // With Ctrl, while typing: Backspace takes the last word, a paste
        // is of text, and what would be done to the layers is not.
        if let (Some(index), true) = (self.editing, mods.ctrl) {
            match key {
                Key::Backspace => {
                    if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                        let kept = shape.text.trim_end().trim_end_matches(|letter: char| !letter.is_whitespace()).len();
                        shape.text.truncate(kept);
                        shape.touch();
                    }
                    self.changed();
                    return Outcome::Continue;
                }
                Key::Char('v' | 'V') => return Outcome::Paste,
                Key::Delete | Key::Char('a' | 'A' | 'c' | 'C' | 'd' | 'D' | 'x' | 'X' | '[' | ']' | '{' | '}' | 'g' | 'G') => return Outcome::Continue,
                _ => {}
            }
        }
        let lower = match key {
            Key::Char(letter) => Key::Char(letter.to_ascii_lowercase()),
            other => other,
        };
        if self.on_board() {
            if let Some(outcome) = self.board_key(key, lower, mods) {
                return outcome;
            }
        }
        match (lower, mods.ctrl) {
            // Style Mode: S opens the grid of styles, a letter puts one on.
            (Key::Escape, _) if self.exported.is_some() => self.exported = None,
            (Key::Escape, _) if self.styling => self.styling = false,
            (Key::Char(letter), false) if self.styling && style::STYLE_KEYS.contains(&letter) => {
                let at = style::STYLE_KEYS.iter().position(|&known| known == letter).unwrap_or(0);
                self.put_style(at as u8);
                self.styling = false;
            }
            (Key::Char('s'), false) if !self.styling && self.styles_apply() => self.styling = true,
            (Key::Char('q' | 'w'), true) => return Outcome::Quit,
            (Key::Char('m'), true) => self.flip_mode(),
            (Key::Char('z'), true) if mods.alt && self.on_board() => self.open_history(),
            (Key::Char('z'), true) if mods.shift => self.redo(),
            (Key::Char('z'), true) => self.undo(),
            (Key::Char('y'), true) => self.redo(),
            (Key::Char('a'), true) => self.selected = (0..self.doc.layers.len()).filter(|&index| !self.doc.layers[index].locked).collect(),
            (Key::Char('d'), true) => drop(self.run_menu(menu::MenuAct::Do(menu::Command::Duplicate))),
            (Key::Char('c'), true) if mods.shift => self.copy(),
            (Key::Char('c'), true) if !self.chosen().is_empty() => self.copy_layers(),
            (Key::Char('c'), true) => self.copy(),
            // Ctrl+Shift+K does again what was last done; Ctrl+K searches.
            (Key::Char('k'), true) if mods.shift => return self.again(),
            (Key::Char('k'), true) => self.open_palette(),
            (Key::Char('e'), true) if mods.shift => drop(self.run_command(menu::Command::ExportPage)),
            (Key::Char('v'), true) if !self.copied.is_empty() && !mods.shift => self.paste_layers(),
            (Key::Char('v'), true) => return Outcome::Paste,
            (Key::Char('s'), true) => self.save(true),
            (Key::Char('e'), true) => drop(self.run_menu(menu::MenuAct::Do(menu::Command::ExportFrame))),
            // The keys the menus say: Ctrl+Alt+G frames what is picked out,
            // Ctrl+Shift+H and L hide and lock it.
            (Key::Char('g'), true) if mods.alt => drop(self.act(Action::FrameThem)),
            (Key::Char('h'), true) if mods.shift => drop(self.run_command(menu::Command::ToggleHide)),
            (Key::Char('l'), true) if mods.shift => drop(self.run_command(menu::Command::ToggleLock)),
            (Key::Char('g'), true) => return Outcome::Capture,
            (Key::Char(']' | '}'), true) => self.restack(true),
            (Key::Char('[' | '{'), true) => self.restack(false),
            // Ctrl+Shift+\ hides or shows the left panel, Ctrl+Alt+\ the
            // right; Shift gives | on most keyboards. Ctrl+\ alone hides
            // everything but the page, as Figma's does.
            (Key::Char('\\' | '|'), true) if mods.alt => self.toggle_panel(1),
            (Key::Char('\\' | '|'), true) if mods.shift => self.toggle_panel(0),
            (Key::Char('\\'), true) => self.toggle_zen(),
            (Key::Char('0'), true) => self.fit(),
            (Key::Char('1'), true) => {
                let middle = self.open_area().center();
                self.zoom_about(1.0 / self.zoom, middle);
            }
            (Key::Char('=' | '+'), true) => {
                let middle = self.open_area().center();
                self.zoom_about(1.25, middle);
            }
            (Key::Char('-'), true) => {
                let middle = self.open_area().center();
                self.zoom_about(0.8, middle);
            }
            (Key::Delete | Key::Backspace, _) if self.frame_picked && self.chosen().is_empty() => self.delete_frame(),
            (Key::Delete | Key::Backspace, _) => self.delete(),
            // Esc closes what is open over the page before it lets go of
            // what is picked out: a list, a group's list, the colors.
            (Key::Escape, _) if self.menu.is_some() => self.menu = None,
            (Key::Escape, _) if self.flyout.is_some() => self.flyout = None,
            (Key::Escape, _) if self.palette.is_some() || self.eyedropping => (self.palette, self.eyedropping) = (None, false),
            (Key::Escape, _) => {
                if self.cropping.take().is_none() {
                    self.selected.clear();
                }
            }
            (Key::Enter, _) => {
                if self.cropping.take().is_none() {
                    if let Some(index) = self.picked() {
                        match &self.doc.layers[index].kind {
                            Kind::Picture(_) => self.cropping = Some(index),
                            Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge) => {
                                self.checkpoint();
                                self.edit_text(index);
                            }
                            Kind::Shape(_) => {}
                        }
                    }
                }
            }
            (Key::Left | Key::Right | Key::Up | Key::Down, false) => {
                let step = if mods.shift { 10.0 } else { 1.0 };
                let (dx, dy) = match lower {
                    Key::Left => (-step, 0.0),
                    Key::Right => (step, 0.0),
                    Key::Up => (0.0, -step),
                    _ => (0.0, step),
                };
                let chosen = self.chosen();
                if !chosen.is_empty() {
                    self.checkpoint_move();
                    chosen.iter().for_each(|&index| self.doc.layers[index].translate(dx, dy));
                    self.changed();
                }
            }
            // Shift+1 fits the frame, and again what is picked out, and
            // back; Shift+2 what is picked out.
            (Key::Char('1' | '!'), false) if mods.shift => {
                if self.fitted_frame && !self.chosen().is_empty() {
                    self.zoom_to_selection();
                    self.fitted_frame = false;
                } else {
                    self.fit();
                    self.fitted_frame = true;
                }
            }
            (Key::Char('2' | '@'), false) if mods.shift => self.zoom_to_selection(),
            (Key::Char('r'), false) if mods.shift => self.rulers = !self.rulers,
            // Shift+H and Shift+V flip what is picked out, as the menus say,
            // rather than taking up the hand and the move tool.
            (Key::Char('h'), false) if mods.shift => drop(self.act(Action::FlipAcross)),
            // Shift+A makes what is picked out a stack, as Figma's does.
            (Key::Char('a'), false) if mods.shift => drop(self.act(Action::AddStack)),
            (Key::Char('v'), false) if mods.shift => drop(self.act(Action::FlipDown)),
            (Key::Char(letter), false) => {
                // J opens the library, and E its emoji, as in capture mode.
                match TOOLS.iter().find(|(_, _, key)| key.to_ascii_lowercase().starts_with(letter)) {
                    Some(&(Tool::Badge, ..)) => self.open_library(None),
                    Some(&(tool, ..)) => self.set_tool(tool),
                    None if letter == 'e' => self.open_library(Some(crate::editor::EMOJI_SHELF)),
                    None => {}
                }
                if letter == 'f' {
                    self.press_control(Control::FrameTool, false);
                }
                if letter == 'h' {
                    self.space = !self.space;
                }
            }
            _ => {}
        }
        Outcome::Continue
    }

    fn undo(&mut self) {
        self.commit_text();
        if let Some(before) = self.undo.pop() {
            let now = self.snapshot();
            self.redo.push(now);
            self.restore(before);
            self.changed();
        }
    }

    fn redo(&mut self) {
        if let Some(after) = self.redo.pop() {
            let now = self.snapshot();
            self.undo.push(now);
            self.restore(after);
            self.changed();
        }
    }

    // ---- The picture of the window ----

    /// A frame as it is shown, drawn at the size it is shown at: the part
    /// of it in the view and a little round that, so that a little
    /// panning needs nothing drawn. Drawn again when it changes, when the
    /// zoom does, or when the view goes past what was drawn. Gives where
    /// its corner is in the window, and how large it is shown.
    ///
    /// While the page is being zoomed or moved, as Figma does, what was
    /// drawn last is shown larger or smaller rather than drawn afresh
    /// each step, so long as it reaches over what is seen; where it does
    /// not, as the page grows smaller or is moved far, it is drawn at half
    /// the size it is shown and over more round it, for the next steps.
    /// Once the page is still, it is drawn sharp again.
    ///
    /// The page's own layers, at `LOOSE`, are drawn the same, as far as
    /// they reach rather than to a frame's edges, in bands: those between
    /// two frames in the order of the page, each kept as its own `band`.
    fn shown_frame(&mut self, index: usize, band: usize, range: (usize, usize), view: Rect, stop: &mut dyn FnMut() -> bool) -> ((i32, i32), Rect) {
        let frame = self.frame(index);
        let origin = self.to_window_page((frame.x, frame.y));
        let stamp = frame.stamp;
        let whole = self.extent(index, range, self.zoom);
        // What of it is in the view, counted from its corner.
        let seen = view.moved(-origin.0, -origin.1).intersect(whole);
        if self.shown.len() < self.frame_count() {
            self.shown.resize_with(self.frame_count(), || None);
        }
        let quick = self.moving.is_some();
        let fresh = self.drawn_of(index, band).is_some_and(|drawn| {
            drawn.stamp == stamp
                && drawn.range == range
                && (seen.is_empty()
                    || match same_zoom(drawn.zoom, self.zoom) {
                        true => drawn.part.intersect(seen) == seen,
                        false => quick && reaches(drawn.part, drawn.zoom, seen, self.zoom),
                    })
        });
        if !fresh && !seen.is_empty() && !self.redraw_touched(index, band, range, seen) {
            let (times, part) = match quick {
                true => {
                    let times = self.zoom / 2.0;
                    let around = seen.inflate(view.w.max(view.h) / 4);
                    let halved = Rect::new(around.x.div_euclid(2), around.y.div_euclid(2), around.w / 2 + 1, around.h / 2 + 1);
                    (times, halved.intersect(self.extent(index, range, times)))
                }
                false => (self.zoom, seen.inflate(ROOM).intersect(whole)),
            };
            // Drawn sharp where only the view has changed since: a band at
            // a time, and given up for the wheel or a key that comes
            // meanwhile, what was drawn shown until it is done.
            let only_view = !quick && self.drawn_of(index, band).is_some_and(|drawn| drawn.stamp == stamp && drawn.range == range);
            let doc = match index == self.active {
                true => &self.doc,
                false => match index {
                    LOOSE => &self.pages[self.page].loose,
                    index => &self.pages[self.page].frames[index],
                },
            };
            let pixels = match only_view {
                true => match doc.render_range_stopping(range, times, part, &mut self.text, stop) {
                    Some(pixels) => pixels,
                    // Given up: drawn sharp again once what came is heard.
                    None => {
                        self.dirty = true;
                        return (origin, whole);
                    }
                },
                false => doc.render_range(range, times, part, &mut self.text),
            };
            let reaches = if index == self.active { self.reaches(times) } else { Vec::new() };
            *self.drawn_slot(index, band) = Some(drawn::Drawn { stamp, zoom: times, part, pixels, reaches, range });
            if index == self.active {
                self.touched = None;
            }
        }
        (origin, whole)
    }

    /// How far a frame reaches, at `times` its size, from its corner: its
    /// size; or for some of the page's own layers, as far as anything of
    /// them is drawn, shadows and all.
    fn extent(&mut self, index: usize, (from, to): (usize, usize), times: f32) -> Rect {
        if index != LOOSE {
            let frame = self.frame(index);
            return Rect::new(0, 0, ((frame.width as f32 * times).round() as i32).max(1), ((frame.height as f32 * times).round() as i32).max(1));
        }
        let layers: Vec<Layer> = self.loose().layers.iter().take(to).skip(from).filter(|layer| !layer.hidden).cloned().collect();
        layers.iter().map(|layer| drawn::reach(layer, times, &mut self.text)).fold(Rect::default(), drawn::both)
    }

    /// What was drawn last of a frame, or of a band of the page's own
    /// layers, between two frames.
    fn drawn_of(&self, index: usize, band: usize) -> Option<&drawn::Drawn> {
        match index {
            LOOSE => self.shown_page.get(band).and_then(Option::as_ref),
            index => self.shown.get(index).and_then(Option::as_ref),
        }
    }

    fn drawn_slot(&mut self, index: usize, band: usize) -> &mut Option<drawn::Drawn> {
        let (kept, at) = match index {
            LOOSE => (&mut self.shown_page, band),
            index => (&mut self.shown, index),
        };
        if kept.len() <= at {
            kept.resize_with(at + 1, || None);
        }
        &mut kept[at]
    }

    /// The page zoomed or moved by the wheel or the hand just now: shown
    /// from what was drawn last until it is still.
    fn moved_quickly(&mut self) {
        self.moving = Some(std::time::Instant::now());
        self.dirty = true;
    }

    /// How long until the page, zoomed or moved, has been still long
    /// enough to be drawn sharp, if it is waiting to be.
    pub fn settle_in(&self) -> Option<std::time::Duration> {
        let since = self.moving?.elapsed();
        Some(SETTLE.saturating_sub(since).max(std::time::Duration::from_millis(1)))
    }

    /// The page drawn sharp again, once it has been still long enough.
    pub fn settle(&mut self) {
        if self.moving.is_some_and(|when| when.elapsed() >= SETTLE) {
            self.moving = None;
            self.dirty = true;
        }
    }

    #[cfg(test)]
    pub fn draw(&mut self, out: &mut [u32]) {
        self.draw_stopping(out, &mut || false);
    }

    /// The window drawn, a frame drawn sharp given up whenever `stop` says
    /// something has come that it should be given up for.
    pub fn draw_stopping(&mut self, out: &mut [u32], stop: &mut dyn FnMut() -> bool) {
        self.crop_window();
        let (width, height) = (self.width, self.height);
        let layout = self.layout();
        let view = layout.view;
        self.pill = None;
        // The page: dots on its ground, the frames on that, each over light
        // squares where it is see-through. A board is on its paper.
        if self.on_board() {
            self.draw_paper(out, view);
        } else {
            out.par_chunks_mut(1 << 16).for_each(|part| part.fill(VIEW_COLOR));
            let dots = 24;
            let (ox, oy) = (self.pan.0.rem_euclid(dots as f32) as i32, self.pan.1.rem_euclid(dots as f32) as i32);
            for y in (view.y + oy..view.bottom()).step_by(dots as usize) {
                for x in (view.x + ox..view.right()).step_by(dots as usize) {
                    out[(y * width + x) as usize] = 0x2E2E2E;
                }
            }
        }
        // Everything on the page in its order, bottom to top: the frames,
        // and the page's own layers between them in bands, each drawn
        // apart, as nothing of them is in a frame.
        self.keep_order();
        for piece in self.pieces() {
            let (index, band, range) = match piece {
                order::Piece::Frame(index) => (index, 0, drawn::ALL),
                order::Piece::Band(band, range) => (LOOSE, band, range),
                // The page's own spotlights darken everything drawn so far,
                // frames and all, within the view.
                order::Piece::Spot => {
                    self.loose().light_page(out, width, view, (self.zoom, self.pan));
                    continue;
                }
            };
            let (origin, whole) = self.shown_frame(index, band, range, view, stop);
            let shown = whole.moved(origin.0, origin.1).intersect(view);
            let Some(drawn::Drawn { zoom: kept_zoom, part, pixels, .. }) = self.drawn_of(index, band) else { continue };
            // Drawn at another zoom while the page moves: where each pixel
            // of the window falls in what was drawn, by the nearest one.
            let by = if same_zoom(*kept_zoom, self.zoom) { 1.0 } else { kept_zoom / self.zoom };
            let reach = match by == 1.0 {
                true => part.moved(origin.0, origin.1),
                false => {
                    let (left, top) = ((part.x as f32 / by).ceil() as i32, (part.y as f32 / by).ceil() as i32);
                    Rect::new(origin.0 + left, origin.1 + top, (part.right() as f32 / by).floor() as i32 - left, (part.bottom() as f32 / by).floor() as i32 - top)
                }
            };
            let drawn = reach.intersect(shown);
            // Beside what is shown, nothing; its corner may be past the end
            // of what was drawn, which no row reaches to.
            if drawn.is_empty() {
                continue;
            }
            let at = |along: i32, start: i32, size: i32| (((along as f32 + 0.5) * by) as i32 - start).clamp(0, size - 1) as usize;
            let columns: Vec<usize> = (drawn.x..drawn.right()).map(|x| at(x - origin.0, part.x, part.w)).collect();
            // The rows shared out among the cores, as a 4K window has
            // millions of pixels to go through.
            let rows = &mut out[(drawn.y * width) as usize..(drawn.bottom() * width) as usize];
            rows.par_chunks_mut(width as usize).enumerate().for_each(|(down, line)| {
                let y = drawn.y + down as i32;
                let row = &mut line[drawn.x as usize..drawn.right() as usize];
                let from = &pixels[at(y - origin.1, part.y, part.h) * part.w as usize..][..part.w as usize];
                let light_row = (y - origin.1) / 10;
                for (step, (pixel, &column)) in row.iter_mut().zip(&columns).enumerate() {
                    let color = from[column];
                    let alpha = color >> 24;
                    *pixel = match alpha {
                        255 => color & 0xFF_FFFF,
                        // The page's own layers over the page, with nothing
                        // behind them but it and the frames.
                        _ if index == LOOSE => canvas::mix(*pixel, color & 0xFF_FFFF, alpha + (alpha >> 7)),
                        _ => {
                            let light = ((drawn.x + step as i32 - origin.0) / 10 + light_row) % 2 == 0;
                            canvas::mix(if light { 0xFFFFFF } else { 0xE6E6E6 }, color & 0xFF_FFFF, alpha + (alpha >> 7))
                        }
                    };
                }
            });
        }
        // Each frame's name over its corner; the one worked in, and the one
        // picked out, lit.
        let mut canvas = Canvas::new(out, width, height);
        canvas.set_clip(view);
        self.draw_pixel_grid(&mut canvas, view);
        for (index, place, name) in self.frame_labels() {
            // On a board's paper, in its ink.
            let (lit, dim) = if self.on_board() { (board::INK, board::QUIET) } else { (TEXT_COLOR, DIM_TEXT_COLOR) };
            let color = if index == self.active && self.frame_picked { if self.on_board() { ACCENT } else { ACCENT_TEXT } } else if index == self.active { lit } else { dim };
            let name = match self.renaming.clone() {
                Some((Naming::Frame, typed)) if index == self.active => self.typed_shown(&mut canvas, (place.x, place.y), 12, typed),
                _ => name,
            };
            self.text.draw(&mut canvas, place.x, place.y, 12, color, &name);
        }
        if self.frame_picked {
            let (a, b) = (self.to_window((0.0, 0.0)), self.to_window((self.doc.width as f32, self.doc.height as f32)));
            canvas.round_outline(a.0, a.1, b.0 - a.0, b.1 - a.1, 0, 1, ACCENT);
            self.size_label(&mut canvas, (a.0 + b.0) / 2, b.1, self.doc.width as f32, self.doc.height as f32);
        }
        if let Drag::NewFrame { from } = self.drag {
            let reach = Rect::spanning(self.to_window_page(from), self.pointer);
            canvas.round_fill(reach.x, reach.y, reach.w, reach.h, 0, 0xFFFFFF, 30);
            canvas.round_outline(reach.x, reach.y, reach.w, reach.h, 0, 1, ACCENT);
        }
        if let Some(index) = self.hovered.filter(|index| !self.selected.contains(index) && *index < self.doc.layers.len()) {
            self.outline(&mut canvas, index, 2);
        }
        // One of the page's own layers, while a frame is worked in, ringed
        // by its box.
        if let Some(layer) = self.hovered_page.filter(|_| !self.on_page()).and_then(|index| self.loose().layers.get(index).cloned()) {
            let (x, y, w, h) = layer.bounds(&mut self.text);
            let (a, b) = (self.to_window_page((x, y)), self.to_window_page((x + w, y + h)));
            canvas.round_outline(a.0, a.1, b.0 - a.0, b.1 - a.1, 0, 2, ACCENT);
        }
        // A color of the selection hovered in the panel: the layers in it
        // ringed, and the rest of the frame dimmed.
        let lit: Vec<usize> = match layout.controls.iter().find(|(rect, _)| rect.contains(self.pointer.0, self.pointer.1)).map(|(_, control)| *control) {
            Some(Control::ColorField(Paint::Recolor(color)) | Control::ColorOpen(color)) => self.color_uses(color).into_iter().map(|(index, _)| index).collect(),
            Some(Control::ColorLayer(index, _)) => vec![index],
            _ => Vec::new(),
        };
        if !lit.is_empty() {
            self.dim_except(&mut canvas, &lit);
        }
        // While a change is previewed, what it changes is shown bare.
        let previewing = self.preview.is_some() || !lit.is_empty();
        for index in self.chosen().into_iter().filter(|_| !previewing) {
            self.outline(&mut canvas, index, 1);
        }
        // As Figma has it: several picked out are boxed together as well,
        // and what is picked out says how large it is under it.
        let chosen = self.chosen();
        if !chosen.is_empty() && self.cropping.is_none() && self.editing.is_none() && !previewing {
            let corners: Vec<(i32, i32)> = chosen.iter().flat_map(|&index| self.box_corners(index)).collect();
            let (left, right) = (corners.iter().map(|c| c.0).min().unwrap_or(0), corners.iter().map(|c| c.0).max().unwrap_or(0));
            let (top, bottom) = (corners.iter().map(|c| c.1).min().unwrap_or(0), corners.iter().map(|c| c.1).max().unwrap_or(0));
            let size = match chosen[..] {
                [index] => self.box_size(index),
                _ => {
                    let spans: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
                    let (x0, y0) = (spans.iter().map(|s| s.0).fold(f32::MAX, f32::min), spans.iter().map(|s| s.1).fold(f32::MAX, f32::min));
                    let (x1, y1) = (spans.iter().map(|s| s.0 + s.2).fold(f32::MIN, f32::max), spans.iter().map(|s| s.1 + s.3).fold(f32::MIN, f32::max));
                    Some((x1 - x0, y1 - y0))
                }
            };
            if chosen.len() > 1 {
                canvas.round_outline(left, top, right - left + 1, bottom - top + 1, 0, 1, ACCENT);
            }
            if let Some((w, h)) = size.filter(|_| !self.on_board()) {
                self.size_label(&mut canvas, (left + right) / 2, bottom, w, h);
            }
        }
        if self.cropping.is_none() && !previewing {
            // Corners only, small white squares edged in the accent; the
            // ends of a line the same.
            for (spot, pull) in self.handles() {
                if pull.0 == END || (pull.0 != 0 && pull.1 != 0) {
                    canvas.fill(spot.0 - 4, spot.1 - 4, 8, 8, ACCENT);
                    canvas.fill(spot.0 - 3, spot.1 - 3, 6, 6, 0xFFFFFF);
                }
            }
            // A board keeps to what it needs: no dots to round corners by.
            if !self.on_board() {
                self.draw_radius_handles(&mut canvas);
            }
            self.draw_turn_label(&mut canvas);
        }
        if let Some(index) = self.cropping {
            self.draw_crop(&mut canvas, index);
        }
        self.draw_gaps(&mut canvas);
        let hover = self.hover_shown;
        self.draw_stacks(&mut canvas, hover);
        self.draw_user_guides(&mut canvas);
        // The lines what is moved has snapped to, across the view.
        for &(upright, at) in &self.guides {
            match upright {
                true => canvas.fill(self.to_window((at, 0.0)).0, view.y, 1, view.h, GUIDE),
                false => canvas.fill(view.x, self.to_window((0.0, at)).1, view.w, 1, GUIDE),
            }
        }
        if let Drag::Marquee { from } = self.drag {
            let (a, b) = (self.to_window(from), self.pointer);
            let reach = Rect::spanning(a, b);
            canvas.round_fill(reach.x, reach.y, reach.w, reach.h, 0, ACCENT, 40);
            canvas.round_outline(reach.x, reach.y, reach.w, reach.h, 0, 1, ACCENT);
        }
        // The caret at the end of text being typed.
        if let Some(index) = self.editing {
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index).cloned() {
                let place = shape.bounds(&mut self.text);
                let (along, top, line) = match shape.tool {
                    Tool::Text => {
                        let lines = shape.text_lines(&mut self.text);
                        let widest = lines.iter().map(|line| line.width).max().unwrap_or(0);
                        lines.last().map_or((0, 0, Text::line_height(shape.font_px)), |last| (last.width + (widest - last.width) * i32::from(shape.align.min(2)) / 2, last.top, Text::line_height(last.px)))
                    }
                    // A box's or an arrow's words, after the last of them.
                    Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow => {
                        let (x, y, high) = shape.label_caret(&mut self.text);
                        (x.round() as i32 - place.x, y.round() as i32 - place.y, high)
                    }
                    _ => {
                        let last = shape.text.rsplit('\n').next().unwrap_or("");
                        let line = Text::line_height(shape.font_px);
                        // A label's words begin after its dot, as capture
                        // mode puts its caret.
                        let (from_x, from_y) = shape.text_origin();
                        (from_x - place.x + self.text.width_in(shape.words_face(), last, shape.font_px), from_y - place.y + (shape.text.split('\n').count() as i32 - 1) * line, line)
                    }
                };
                let (x, y) = self.to_window(((place.x + along) as f32, (place.y + top) as f32));
                canvas.fill(x + 1, y, 2, (line as f32 * self.zoom) as i32, ACCENT);
            }
        }
        // With the text tool, a faint caret where a click would begin
        // text, as tall as its letters would be.
        let over_control = layout.controls.iter().any(|(rect, _)| rect.contains(self.pointer.0, self.pointer.1));
        if self.tool == Tool::Text && !self.framing && self.editing.is_none() && self.drag == Drag::None && !over_control && self.over_page(self.pointer) {
            let high = (Text::line_height(FONT_SIZES[self.size]) as f32 * self.zoom).round() as i32;
            canvas.round_fill(self.pointer.0 - 1, self.pointer.1 - high / 2 - 1, 4, high + 2, 0, 0x000000, 70);
            canvas.round_fill(self.pointer.0, self.pointer.1 - high / 2, 2, high, 0, 0xFFFFFF, 190);
        }
        // Fine-tune open: the frame split, the original against the
        // converted picture.
        // What a board shows over everything on it.
        if self.on_board() {
            self.draw_board(&mut canvas, view);
        }
        self.draw_split(&mut canvas, view);
        canvas.reset_clip();
        self.draw_rulers(&mut canvas);
        self.draw_panels(&mut canvas, &layout);
        self.draw_fine(&mut canvas);
    }

    /// Zoomed far in, a faint grid between the pixels of the page over
    /// each frame, as Figma draws one, to count them by.
    fn draw_pixel_grid(&mut self, canvas: &mut Canvas, view: Rect) {
        if self.zoom < PIXEL_GRID {
            return;
        }
        for index in 0..self.frame_count() {
            let frame = self.frame(index);
            let (x0, y0, x1, y1) = (frame.x, frame.y, frame.x + frame.width as f32, frame.y + frame.height as f32);
            let (a, b) = (self.to_window_page((x0, y0)), self.to_window_page((x1, y1)));
            let area = Rect::new(a.0, a.1, b.0 - a.0, b.1 - a.1).intersect(view);
            if area.is_empty() {
                continue;
            }
            // The page's pixels whose edges are in what is seen.
            let first = self.to_page((area.x, area.y));
            let (from_x, from_y) = (first.0.ceil() as i64, first.1.ceil() as i64);
            let mut x = from_x;
            while let Some(at) = Some((x as f32 * self.zoom + self.pan.0).round() as i32).filter(|&at| at < area.right()) {
                canvas.round_fill(at, area.y, 1, area.h, 0, 0x8A8A8A, 56);
                x += 1;
            }
            let mut y = from_y;
            while let Some(at) = Some((y as f32 * self.zoom + self.pan.1).round() as i32).filter(|&at| at < area.bottom()) {
                canvas.round_fill(area.x, at, area.w, 1, 0, 0x8A8A8A, 56);
                y += 1;
            }
        }
    }

    /// A layer's box, as Figma draws it: a box or an oval by its own
    /// frame and not its border, text by its lines, turned with them; a
    /// line or an arrow by the line itself.
    fn outline(&mut self, canvas: &mut Canvas, index: usize, thick: i32) {
        let corners = match &self.doc.layers[index].kind {
            Kind::Shape(shape) if matches!(shape.tool, Tool::Line | Tool::Arrow) => shape.points.iter().map(|&point| self.to_window(point)).collect::<Vec<_>>(),
            _ => {
                let corners = self.box_corners(index);
                corners.iter().chain(corners.first()).copied().collect()
            }
        };
        for pair in corners.windows(2) {
            for nudge in 0..thick {
                line(canvas, (pair[0].0 + nudge, pair[0].1 + nudge), (pair[1].0 + nudge, pair[1].1 + nudge), ACCENT);
            }
        }
    }

    /// Where the frame worked in is in the window; with the page's own
    /// layers worked in, all of the window, as they are kept to nothing.
    fn frame_on_screen(&self) -> Rect {
        if self.on_page() {
            return Rect::new(0, 0, self.width, self.height);
        }
        let (a, b) = (self.to_window((0.0, 0.0)), self.to_window((self.doc.width as f32, self.doc.height as f32)));
        Rect::new(a.0, a.1, b.0 - a.0, b.1 - a.1)
    }

    /// The corners of a layer's box in the window, turned with it.
    fn box_corners(&mut self, index: usize) -> [(i32, i32); 4] {
        let corners = match self.doc.layers[index].kind.clone() {
            Kind::Picture(picture) => picture.corners(),
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Text | Tool::Badge | Tool::Blur | Tool::Spotlight) => {
                let (x, y, w, h) = match shape.tool {
                    Tool::Text | Tool::Badge => {
                        let place = shape.local_bounds(&mut self.text);
                        (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
                    }
                    _ => shape.frame(),
                };
                [(x, y), (x + w, y), (x + w, y + h), (x, y + h)].map(|corner| shape.returned(corner, &mut self.text))
            }
            Kind::Shape(_) => {
                let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
                [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
            }
        };
        corners.map(|corner| self.to_window(corner))
    }

    /// How large a layer is, as its box says, for the label under it; a
    /// line or an arrow says nothing.
    fn box_size(&mut self, index: usize) -> Option<(f32, f32)> {
        match self.doc.layers[index].kind.clone() {
            Kind::Picture(picture) => Some((picture.w, picture.h)),
            Kind::Shape(shape) => match shape.tool {
                Tool::Line | Tool::Arrow => None,
                Tool::Text | Tool::Badge => {
                    let place = shape.local_bounds(&mut self.text);
                    Some((place.w as f32, place.h as f32))
                }
                Tool::Rect | Tool::Ellipse => Some((shape.frame().2, shape.frame().3)),
                _ => {
                    let (_, _, w, h) = self.doc.layers[index].bounds(&mut self.text);
                    Some((w, h))
                }
            },
        }
    }

    /// Where what was last done is said, so wide: over the bar of tools,
    /// or near the bottom with no bar; and beside the pill saying how large
    /// what is picked out is rather than over it, to its right, or to its
    /// left where there is no room.
    fn note_at(&self, dock: Rect, wide: i32) -> (i32, i32) {
        let (x, y) = match dock.w {
            0 => ((self.width - wide) / 2, self.height - 64),
            _ => (dock.x + (dock.w - wide) / 2, dock.y - 44),
        };
        let Some(pill) = self.pill.filter(|pill| !pill.inflate(4).intersect(Rect::new(x, y, wide, 32)).is_empty()) else { return (x, y) };
        let open = self.open_area();
        match pill.right() + 12 + wide <= open.right() {
            true => (pill.right() + 12, y),
            false => ((pill.x - 12 - wide).max(open.x), y),
        }
    }

    /// "W × H" in a small pill of the accent, under what is picked out.
    fn size_label(&mut self, canvas: &mut Canvas, cx: i32, bottom: i32, w: f32, h: f32) {
        let says = format!("{} \u{d7} {}", w.abs().round() as i32, h.abs().round() as i32);
        let wide = self.text.width(&says, 11) + 10;
        let (x, y) = (cx - wide / 2, bottom + 8);
        canvas.round_fill(x, y, wide, 18, 3, ACCENT, 256);
        self.pill = Some(Rect::new(x, y, wide, 18));
        self.text.draw(canvas, x + 5, y + (18 - Text::line_height(11)) / 2, 11, 0xFFFFFF, &says);
    }

    /// The picture being cropped: all of it faint, the crop bright, and
    /// its corners to pull.
    fn draw_crop(&mut self, canvas: &mut Canvas, index: usize) {
        let Some(Layer { kind: Kind::Picture(picture), .. }) = self.doc.layers.get(index).cloned() else { return };
        // Its edges found as it is first shown cropped, for its sides to
        // snap to when pulled.
        self.crop_lines(&picture.image);
        // The whole of it, where it would be, in outline.
        let (per_x, per_y) = (picture.w / picture.crop.2.max(1e-3), picture.h / picture.crop.3.max(1e-3));
        let (iw, ih) = (picture.image.width() as f32 * per_x, picture.image.height() as f32 * per_y);
        let (cx, cy) = (picture.crop.0 * per_x, picture.crop.1 * per_y);
        let whole = [(-picture.w / 2.0 - cx, -picture.h / 2.0 - cy), (-picture.w / 2.0 - cx + iw, -picture.h / 2.0 - cy), (-picture.w / 2.0 - cx + iw, -picture.h / 2.0 - cy + ih), (-picture.w / 2.0 - cx, -picture.h / 2.0 - cy + ih)].map(|corner| self.to_window(picture.placed(corner)));
        for side in 0..4 {
            line(canvas, whole[side], whole[(side + 1) % 4], 0x8B8D98);
        }
        let corners = picture.corners().map(|corner| self.to_window(corner));
        // Square to the frame, the rest of the frame dimmed round the crop,
        // a grid of thirds in it, and how large it is under it.
        if picture.angle == 0.0 {
            let crop = Rect::spanning(corners[0], corners[2]);
            let frame = self.frame_on_screen();
            for part in [
                Rect::new(frame.x, frame.y, frame.w, crop.y - frame.y),
                Rect::new(frame.x, crop.bottom(), frame.w, frame.bottom() - crop.bottom()),
                Rect::new(frame.x, crop.y, crop.x - frame.x, crop.h),
                Rect::new(crop.right(), crop.y, frame.right() - crop.right(), crop.h),
            ] {
                let part = part.intersect(frame);
                if !part.is_empty() {
                    canvas.round_fill(part.x, part.y, part.w, part.h, 0, 0x000000, 140);
                }
            }
            for third in 1..3 {
                canvas.round_fill(crop.x + crop.w * third / 3, crop.y, 1, crop.h, 0, 0xFFFFFF, 90);
                canvas.round_fill(crop.x, crop.y + crop.h * third / 3, crop.w, 1, 0, 0xFFFFFF, 90);
            }
            let says = format!("{} \u{d7} {}", picture.crop.2.round() as i32, picture.crop.3.round() as i32);
            let wide = self.text.width(&says, 11) + 12;
            let (x, y) = (crop.x + (crop.w - wide) / 2, crop.bottom() + 12);
            canvas.round_fill(x, y, wide, 18, 4, ACCENT, 256);
            self.pill = Some(Rect::new(x, y, wide, 18));
            self.text.draw(canvas, x + 6, y + (18 - Text::line_height(11)) / 2, 11, 0xFFFFFF, &says);
        }
        for side in 0..4 {
            line(canvas, corners[side], corners[(side + 1) % 4], 0xFFFFFF);
        }
        // A bracket at each corner, pointing out along its two sides.
        for at in 0..4 {
            let corner = corners[at];
            for other in [corners[(at + 1) % 4], corners[(at + 3) % 4]] {
                let (dx, dy) = ((other.0 - corner.0) as f32, (other.1 - corner.1) as f32);
                let length = dx.hypot(dy).max(1.0);
                let reach = 16.0f32.min(length / 3.0);
                let to = (corner.0 + (dx / length * reach) as i32, corner.1 + (dy / length * reach) as i32);
                for thick in -1..=1 {
                    line(canvas, (corner.0 + thick, corner.1), (to.0 + thick, to.1), 0xFFFFFF);
                    line(canvas, (corner.0, corner.1 + thick), (to.0, to.1 + thick), 0xFFFFFF);
                }
            }
        }
        self.draw_crop_snaps(canvas, &picture);
    }

    fn draw_panels(&mut self, canvas: &mut Canvas, layout: &Layout) {
        let px = 12;
        let line_high = Text::line_height(px);
        let hover = layout.controls.iter().find(|(rect, _)| rect.contains(self.pointer.0, self.pointer.1)).map(|(_, control)| *control).filter(|control| *control != Control::Panel);
        let chosen = self.chosen();
        let shown = self.picked().map(|index| self.doc.layers[index].clone());
        // The panels, and the bar of tools on a soft shadow.
        for &panel in &layout.panels {
            canvas.round_fill(panel.x, panel.y, panel.w, panel.h, 12, PANEL_COLOR, 256);
            canvas.round_outline(panel.x, panel.y, panel.w, panel.h, 12, 1, EDGE_COLOR);
        }
        for place in [Some(layout.dock).filter(|dock| dock.w > 0), layout.flyout, layout.popover, layout.styles, layout.library, layout.search, layout.crop_bar, layout.exported, layout.board_bar].into_iter().flatten() {
            soft_shadow(canvas, place, 12);
            canvas.round_fill(place.x, place.y, place.w, place.h, 12, PANEL_COLOR, 256);
            canvas.round_outline(place.x, place.y, place.w, place.h, 12, 1, EDGE_COLOR);
        }
        for rule in &layout.rules {
            canvas.fill(rule.x + 1, rule.y, rule.w - 2, 1, EDGE_COLOR);
        }
        if let Some(place) = layout.exported {
            self.draw_exported(canvas, place);
        }
        if let Some(place) = layout.styles {
            self.draw_styles_frame(canvas, place);
        }
        if let Some(place) = layout.library {
            self.draw_library(canvas, layout, place, hover);
        }
        if let Some(place) = layout.search {
            self.draw_search(canvas, place, hover);
        }
        // The group's divider in the bar of tools.
        let divider = layout.controls.iter().find(|(_, control)| *control == Control::Action(Action::Capture)).map(|(rect, _)| rect.x - 6);
        if let Some(x) = divider {
            canvas.fill(x, layout.dock.y + 14, 1, layout.dock.h - 28, HOVER_COLOR);
        }
        let field_text = |text: &mut Text, canvas: &mut Canvas, rect: Rect, label: &str, icon: bool, value: &str, label_color: u32| {
            let middle = rect.y + rect.h / 2;
            let at = if icon {
                text.draw_centered(canvas, rect.x + 15, middle, 14, label_color, label);
                rect.x + 28
            } else {
                text.draw(canvas, rect.x + 9, middle - Text::line_height(11) / 2, 11, label_color, label);
                rect.x + 16 + text.width(label, 11)
            };
            text.draw(canvas, at, middle - line_high / 2, px, TEXT_COLOR, value);
        };
        for &(rect, control) in &layout.controls {
            let hovered = hover == Some(control);
            let (cx, cy) = rect.center();
            match control {
                Control::Tool(tool) => {
                    let on = self.tool == tool && !self.framing;
                    if on {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, ACCENT, 256);
                    } else if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, HOVER_COLOR, 256);
                    }
                    self.text.draw_centered(canvas, cx, cy, 17, if on { 0xFFFFFF } else { TEXT_COLOR }, tool_icon(tool));
                }
                Control::FrameTool => {
                    if self.framing {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, ACCENT, 256);
                    } else if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, HOVER_COLOR, 256);
                    }
                    self.text.draw_centered(canvas, cx, cy, 17, if self.framing { 0xFFFFFF } else { TEXT_COLOR }, icons::HASH);
                }
                // The More tools button: the one of them in hand, lit, or
                // three dots, as Figma's has.
                Control::Group(MORE) => {
                    // The library, while it is open, is the tool in hand.
                    let open = self.flyout == Some(MORE);
                    let tool = if self.library.is_some() { Tool::Badge } else { self.tool };
                    let on = MORE_TOOLS.contains(&tool) && !self.framing;
                    if on {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, ACCENT, 256);
                    } else if hovered || open {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, HOVER_COLOR, 256);
                    }
                    match on {
                        true => self.text.draw_centered(canvas, cx, cy, 17, 0xFFFFFF, tool_icon(tool)),
                        false => (-1..=1).for_each(|at| canvas.disc(cx + at * 6, cy, 2, TEXT_COLOR, 256)),
                    }
                }
                Control::Group(group) => {
                    let open = self.flyout == Some(group);
                    self.text.draw_centered(canvas, cx - 2, cy, 10, if open || hovered { TEXT_COLOR } else { DIM_TEXT_COLOR }, icons::DOWN);
                }
                Control::Pick(tool) => {
                    let on = self.tool == tool;
                    if on || hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    self.text.draw_centered(canvas, rect.x + 16, cy, 15, TEXT_COLOR, tool_icon(tool));
                    if let Some((_, name, key)) = TOOLS.iter().find(|(known, ..)| *known == tool) {
                        self.text.draw(canvas, rect.x + 34, cy - line_high / 2, px, TEXT_COLOR, name);
                        let wide = self.text.width(key, px);
                        self.text.draw(canvas, rect.right() - 10 - wide, cy - line_high / 2, px, DIM_TEXT_COLOR, key);
                    }
                }
                Control::Action(Action::Capture) if rect.y >= layout.dock.y => {
                    if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, HOVER_COLOR, 256);
                    }
                    self.text.draw_centered(canvas, cx, cy, 17, TEXT_COLOR, icons::CAMERA);
                }
                Control::Action(Action::Export) if rect.y < INSET + 50 => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { 0x5472E4 } else { ACCENT }, 256);
                    self.text.draw_centered(canvas, rect.x + 18, cy, 14, 0xFFFFFF, icons::EXPORT);
                    self.text.draw(canvas, rect.x + 32, cy - line_high / 2, px, 0xFFFFFF, "Export");
                }
                Control::Zoom => {
                    if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                    }
                    let zoom = format!("{}%", (self.zoom * 100.0).round() as i32);
                    self.text.draw(canvas, rect.x + 8, cy - line_high / 2, px, TEXT_COLOR, &zoom);
                    let wide = self.text.width(&zoom, px);
                    self.text.draw_centered(canvas, rect.x + 18 + wide, cy, 10, DIM_TEXT_COLOR, icons::DOWN);
                }
                Control::SidePanel(side) => {
                    let open = if side == 0 { self.shows_left() } else { self.shows_right() };
                    // With its panel hidden, the button floats on its own,
                    // on a little panel of its own.
                    if !open {
                        soft_shadow(canvas, rect.inflate(4), 10);
                        canvas.round_fill(rect.x - 4, rect.y - 4, rect.w + 8, rect.h + 8, 10, PANEL_COLOR, 256);
                        canvas.round_outline(rect.x - 4, rect.y - 4, rect.w + 8, rect.h + 8, 10, 1, EDGE_COLOR);
                    }
                    if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                    }
                    draw_side_icon(canvas, rect, side, open, if hovered { TEXT_COLOR } else { DIM_TEXT_COLOR });
                }
                Control::Home => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 8, if hovered { 0x5472E4 } else { ACCENT }, 256);
                    self.text.draw_centered(canvas, cx, cy, 15, 0xFFFFFF, if hovered { icons::HOME } else { icons::CAMERA });
                }
                Control::NewPage => {
                    if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                    }
                    self.text.draw_centered(canvas, cx, cy, 14, TEXT_COLOR, icons::PLUS);
                }
                Control::Page(index) => {
                    let on = index == self.page;
                    if on || hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    self.text.draw_centered(canvas, rect.x + 14, cy, 14, if on { TEXT_COLOR } else { DIM_TEXT_COLOR }, icons::FILE);
                    let name = match self.renaming.clone() {
                        Some((Naming::Page(page), typed)) if page == index => self.typed_shown(canvas, (rect.x + 28, cy - line_high / 2), px, typed),
                        _ => self.pages[index].name.clone(),
                    };
                    self.text.draw(canvas, rect.x + 28, cy - line_high / 2, px, TEXT_COLOR, &name);
                    if on {
                        let count = format!("{}", self.frame_count());
                        let wide = self.text.width(&count, 11);
                        self.text.draw(canvas, rect.right() - 10 - wide, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &count);
                    }
                }
                Control::Frame(index) if self.shows_left() && rect.right() <= INSET + LEFT_W => {
                    let on = index == self.active && (self.frame_picked || chosen.is_empty());
                    if on || hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    let open = index == self.active;
                    self.text.draw_centered(canvas, rect.x + 10, cy, 10, DIM_TEXT_COLOR, if open { icons::DOWN } else { icons::RIGHT });
                    self.text.draw_centered(canvas, rect.x + 26, cy, 14, TEXT_COLOR, icons::HASH);
                    let frame = self.frame(index);
                    let (name, size) = (frame.name.clone(), format!("{} \u{d7} {}", frame.width, frame.height));
                    let name: String = name.chars().take(16).collect();
                    self.text.draw(canvas, rect.x + 40, cy - line_high / 2, px, TEXT_COLOR, &name);
                    let wide = self.text.width(&size, 11);
                    self.text.draw(canvas, rect.right() - 10 - wide, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &size);
                    self.draw_drop_mark(canvas, rect, control);
                }
                Control::Frame(_) => {}
                // A layer of the page itself, while a frame is worked in,
                // is listed as one of the frame's is.
                Control::Layer(index) | Control::PageLayer(index) => {
                    let of_page = matches!(control, Control::PageLayer(_));
                    let on = !of_page && chosen.contains(&index);
                    let row_hovered = match of_page {
                        true => hover == Some(Control::PageLayer(index)),
                        false => matches!(hover, Some(Control::Layer(at) | Control::Eye(at) | Control::Lock(at)) if at == index),
                    };
                    if on || row_hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    // Its eye and lock, which come before it among the
                    // controls, to be clicked first, are drawn over it here,
                    // where its color does not hide them.
                    let layer = match of_page {
                        true => self.pages[self.page].loose.layers[index].clone(),
                        false => self.doc.layers[index].clone(),
                    };
                    let renamed = !of_page && matches!(&self.renaming, Some((Naming::Layer(at), _)) if *at == index);
                    let (eye, lock) = (rect.right() - 37, rect.right() - 13);
                    if layer.hidden || row_hovered {
                        self.text.draw_centered(canvas, eye, cy, 14, if layer.hidden { DIM_TEXT_COLOR } else { TEXT_COLOR }, if layer.hidden { icons::EYE_OFF } else { icons::EYE });
                    }
                    if layer.locked || row_hovered {
                        self.text.draw_centered(canvas, lock, cy, 14, if layer.locked { TEXT_COLOR } else { DIM_TEXT_COLOR }, icons::LOCK);
                    }
                    let icon = match &layer.kind {
                        Kind::Picture(_) => icons::PICTURE,
                        Kind::Shape(shape) => match shape.tool {
                            Tool::Badge => icons::BADGE,
                            tool => tool_icon(tool),
                        },
                    };
                    let (named, faint) = (layer.name(), layer.hidden);
                    let name = match (self.renaming.clone(), renamed) {
                        (Some((_, typed)), true) => self.typed_shown(canvas, (rect.x + 28, cy - line_high / 2), px, typed),
                        _ => named,
                    };
                    // Where a layer dragged in the list would go.
                    self.draw_drop_mark(canvas, rect, control);
                    let ink = if faint { DIM_TEXT_COLOR } else { TEXT_COLOR };
                    self.text.draw_centered(canvas, rect.x + 12, cy, 14, ink, icon);
                    let name: String = name.chars().take(18).collect();
                    self.text.draw(canvas, rect.x + 28, cy - line_high / 2, px, ink, &name);
                }
                // A stack's row: its caret, which way it goes, and its name,
                // lit while all of it is what is picked out.
                Control::StackRow(id) => {
                    let Some(stack) = self.stack_by_id(id).cloned() else { continue };
                    let on = self.picked_stack().is_some_and(|index| self.doc.stacks[index].id == id);
                    let row_hovered = matches!(hover, Some(Control::StackRow(at) | Control::StackFold(at)) if at == id);
                    if on || row_hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    let open = !self.folded.contains(&id);
                    self.text.draw_centered(canvas, rect.x + 8, cy, 10, DIM_TEXT_COLOR, if open { icons::DOWN } else { icons::RIGHT });
                    self.text.draw_centered(canvas, rect.x + 24, cy, 14, TEXT_COLOR, stack_rows::stack_icon(&stack));
                    let name = match self.renaming.clone() {
                        Some((Naming::Stack(at), typed)) if at == id => self.typed_shown(canvas, (rect.x + 38, cy - line_high / 2), px, typed),
                        _ => stack.title(),
                    };
                    let name: String = name.chars().take(18).collect();
                    self.text.draw(canvas, rect.x + 38, cy - line_high / 2, px, TEXT_COLOR, &name);
                    self.draw_drop_mark(canvas, rect, control);
                }
                // Drawn with the row they belong to.
                Control::Eye(_) | Control::Lock(_) | Control::StackFold(_) => {}
                Control::Field(field) if field.slides() => {
                    // A slider: its name, and a track with a knob on it at
                    // its value, the middle being none.
                    self.text.draw(canvas, rect.x, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, field.label());
                    let (left, wide) = (rect.x + 80, (rect.w - 84).max(20));
                    canvas.round_fill(left, cy - 2, wide, 4, 2, 0x4A4A4A, 256);
                    let value = self.field(field).unwrap_or(0.0);
                    let at = left + ((value + 100.0) / 200.0 * wide as f32).round() as i32;
                    let middle = left + wide / 2;
                    canvas.fill(at.min(middle), cy - 2, (at - middle).abs(), 4, ACCENT);
                    canvas.disc(at, cy, 7, if hovered { 0xFFFFFF } else { 0xE8E8E8 }, 256);
                }
                Control::Field(field) => {
                    let typing = self.typing.as_ref().filter(|(typed, _)| *typed == field).map(|(_, typed)| typed.clone());
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                    if typing.is_some() || hovered {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if typing.is_some() { ACCENT } else { 0x555555 });
                    }
                    let value = match typing {
                        Some(typed) => format!("{typed}|"),
                        // Corners that differ, as Figma says of them.
                        None if field == Field::Radius && self.radius_mixed() => "Mixed".to_owned(),
                        None => self.field(field).map_or("\u{2014}".to_owned(), |value| match field {
                            Field::Opacity | Field::Dim => format!("{}%", value.round() as i32),
                            Field::Magnify => format!("{:.1}\u{d7}", value),
                            Field::Turn | Field::Angle | Field::LineAngle | Field::FillAngle => format!("{}\u{b0}", value.round() as i32),
                            _ => format!("{}", value.round() as i32),
                        }),
                    };
                    let (label, icon) = match field {
                        Field::Turn | Field::Angle | Field::LineAngle | Field::FillAngle => (icons::ROTATE, true),
                        Field::Radius => (icons::RADIUS, true),
                        Field::Corner(_) => ("", true),
                        Field::Opacity => (icons::EYE, true),
                        other => (other.label(), false),
                    };
                    field_text(&mut self.text, canvas, rect, label, icon, &value, DIM_TEXT_COLOR);
                    if let Field::Corner(corner) = field {
                        let (sx, sy) = corners::INWARD[usize::from(corner).min(3)];
                        corners::corner_glyph(canvas, (rect.x + 15 - (sx * 5.0) as i32, rect.y + rect.h / 2 - (sy * 5.0) as i32), usize::from(corner), 10, DIM_TEXT_COLOR);
                    }
                }
                Control::Style(style) => self.draw_style_tile(canvas, rect, style, hovered),
                // The library draws its own.
                Control::Shelf(_) | Control::LibraryItem(_) | Control::EmojiItem(_) => {}
                Control::Found(row) => self.draw_hit(canvas, rect, usize::from(row), hovered),
                Control::SearchTab(_) => {}
                Control::Tab(tab) => {
                    let on = self.panel_tab == tab;
                    if on || hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { FIELD_COLOR } else { HOVER_COLOR }, 256);
                    }
                    let says = if tab == 0 { "Design" } else { "Export" };
                    let wide = self.text.width(says, 12);
                    self.text.draw(canvas, rect.x + (rect.w - wide) / 2, cy - line_high / 2, px, if on { TEXT_COLOR } else { DIM_TEXT_COLOR }, says);
                }
                Control::ExportPick(index) => {
                    let on = !self.export_off.contains(&index);
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                    let tick = Rect::new(rect.x + 8, cy - 7, 14, 14);
                    canvas.round_fill(tick.x, tick.y, tick.w, tick.h, 3, if on { ACCENT } else { 0x4A4A4A }, 256);
                    if on {
                        self.text.draw_centered(canvas, tick.x + 7, tick.y + 7, 10, 0xFFFFFF, icons::CHECK);
                    }
                    let frame = self.frame(index);
                    let (name, sizes) = (frame.name.clone(), frame.exports.len().max(1));
                    self.text.draw_centered(canvas, rect.x + 34, cy, 12, DIM_TEXT_COLOR, icons::HASH);
                    self.text.draw(canvas, rect.x + 46, cy - line_high / 2, px, TEXT_COLOR, &name);
                    let says = format!("{sizes} {}", if sizes == 1 { "size" } else { "sizes" });
                    let wide = self.text.width(&says, 11);
                    self.text.draw(canvas, rect.right() - 10 - wide, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &says);
                }
                Control::ExportDone(done) => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if done { if hovered { 0x5472E4 } else { ACCENT } } else if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                    let says = if done { "Done" } else { "Show in folder" };
                    let wide = self.text.width(says, 12);
                    self.text.draw(canvas, rect.x + (rect.w - wide) / 2, cy - line_high / 2, px, if done { 0xFFFFFF } else { TEXT_COLOR }, says);
                }
                Control::Action(Action::ExportChosen) => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { 0x5472E4 } else { ACCENT }, 256);
                    let count = self.export_count();
                    let says = format!("Export {count} {}", if count == 1 { "picture" } else { "pictures" });
                    let wide = self.text.width(&says, 12);
                    self.text.draw(canvas, rect.x + (rect.w - wide) / 2, cy - line_high / 2, px, 0xFFFFFF, &says);
                }
                Control::ColorOpen(color) => {
                    let open = self.color_open == Some(color);
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                    self.text.draw_centered(canvas, cx, cy, 10, if open || hovered { TEXT_COLOR } else { DIM_TEXT_COLOR }, if open { icons::DOWN } else { icons::RIGHT });
                }
                Control::ColorLayer(index, role) => {
                    if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                    }
                    if let Some(layer) = self.doc.layers.get(index) {
                        let icon = match &layer.kind {
                            Kind::Picture(_) => icons::PICTURE,
                            Kind::Shape(shape) => match shape.tool {
                                Tool::Badge => icons::BADGE,
                                tool => tool_icon(tool),
                            },
                        };
                        let name: String = layer.name().chars().take(22).collect();
                        self.text.draw_centered(canvas, rect.x + 30, cy, 13, DIM_TEXT_COLOR, icon);
                        self.text.draw(canvas, rect.x + 44, cy - line_high / 2, px, TEXT_COLOR, &name);
                        let wide = self.text.width(role, 11);
                        self.text.draw(canvas, rect.right() - 8 - wide, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, role);
                    }
                }
                Control::GradientBar => {
                    if let Some(stops) = self.doc.background.stops() {
                        self.draw_stops(canvas, rect, stops, (Paint::From, Paint::To));
                    }
                }
                Control::FillBar => {
                    if let Some(stops) = self.fill_stops() {
                        self.draw_stops(canvas, rect, stops, (Paint::Fill, Paint::FillTo));
                    }
                }
                Control::ColorField(paint) => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                    if hovered || self.palette == Some(paint) {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if self.palette == Some(paint) { ACCENT } else { 0x555555 });
                    }
                    let now = match (paint, &shown, self.doc.background) {
                        (Paint::Background, _, Fill::Color(color)) => Some(color),
                        (Paint::From, _, Fill::Gradient(from, ..) | Fill::Radial(from, _)) => Some(from),
                        (Paint::To, _, Fill::Gradient(_, to, _) | Fill::Radial(_, to)) => Some(to),
                        (Paint::Line, Some(Layer { kind: Kind::Shape(shape), .. }), _) => Some(shape.color),
                        (Paint::Fill, Some(Layer { kind: Kind::Shape(shape), .. }), _) => shape.filled.then_some(shape.fill),
                        (Paint::FillTo, _, _) => self.fill_stops().map(|(_, to)| to),
                        (Paint::Shadow, Some(layer), _) => layer.drop().map(|drop| drop.color & 0xFF_FFFF),
                        (Paint::Recolor(color), _, _) => Some(color),
                        (Paint::Stack, _, _) => self.paint_color(Paint::Stack),
                        _ => None,
                    };
                    let swatch = Rect::new(rect.x + 7, cy - 8, 16, 16);
                    match now {
                        Some(color) => canvas.round_fill(swatch.x, swatch.y, swatch.w, swatch.h, 4, color, 256),
                        None => {
                            canvas.round_fill(swatch.x, swatch.y, swatch.w, swatch.h, 4, 0xF2F2F2, 256);
                            for step in 2..14 {
                                canvas.fill(swatch.x + step, swatch.bottom() - step - 1, 1, 1, 0xE5484D);
                            }
                        }
                    }
                    canvas.round_outline(swatch.x, swatch.y, swatch.w, swatch.h, 4, 1, 0x555555);
                    let says = match (now, paint) {
                        (Some(color), _) => format!("{color:06X}"),
                        (None, _) => "None".to_owned(),
                    };
                    let label = match paint {
                        Paint::From => "From".to_owned(),
                        Paint::To | Paint::FillTo => "To".to_owned(),
                        Paint::Fill if self.fill_stops().is_some() => "From".to_owned(),
                        Paint::Recolor(color) => {
                            let count = self.selection_colors().iter().find(|(known, _)| *known == color).map_or(0, |(_, count)| *count);
                            format!("{count} {}", if count == 1 { "layer" } else { "layers" })
                        }
                        _ => String::new(),
                    };
                    self.text.draw(canvas, rect.x + 32, cy - line_high / 2, px, TEXT_COLOR, &says);
                    if !label.is_empty() {
                        let wide = self.text.width(&label, 11);
                        self.text.draw(canvas, rect.right() - 10 - wide, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &label);
                    }
                }
                Control::Swatch(paint, color) => {
                    let now = match (paint, &shown, self.doc.background) {
                        (Paint::Background, _, Fill::Color(c)) => Some(c) == color,
                        (Paint::From, _, Fill::Gradient(c, ..) | Fill::Radial(c, _)) => Some(c) == color,
                        (Paint::To, _, Fill::Gradient(_, c, _) | Fill::Radial(_, c)) => Some(c) == color,
                        (Paint::Line, Some(Layer { kind: Kind::Shape(shape), .. }), _) => Some(shape.color) == color,
                        (Paint::Fill, Some(Layer { kind: Kind::Shape(shape), .. }), _) => if shape.filled { Some(shape.fill) == color } else { color.is_none() },
                        (Paint::FillTo, _, _) => self.fill_stops().map(|(_, to)| to) == color,
                        _ => false,
                    };
                    if now || hovered {
                        canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 7, 2, if now { 0xFFFFFF } else { 0x777777 });
                    }
                    match color {
                        Some(color) => canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 5, color, 256),
                        None => {
                            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 5, 0xF2F2F2, 256);
                            for step in 3..rect.w - 3 {
                                canvas.fill(rect.x + step, rect.bottom() - step - 1, 2, 2, 0xE5484D);
                            }
                        }
                    }
                }
                Control::Backdrop(which) => {
                    for at in 0..rect.w * rect.h {
                        let (x, y) = (at % rect.w, at / rect.w);
                        let color = frame::shade(which, (x as f32 + 0.5) / rect.w as f32, (y as f32 + 0.5) / rect.h as f32, 0.5);
                        canvas.blend(rect.x + x, rect.y + y, color, 255);
                    }
                    if self.doc.background == Fill::Backdrop(which) {
                        canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 4, 2, 0xFFFFFF);
                    }
                }
                Control::Board(ctl) => self.draw_board_control(canvas, rect, ctl, hovered),
                Control::Panel => {}
                Control::Menu(kind) => {
                    let open = self.menu.is_some_and(|menu| menu.kind == kind);
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                    if hovered || open {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if open { ACCENT } else { 0x555555 });
                    }
                    let (look, says, hint) = self.menu_value(kind);
                    let mut x = rect.x + 9;
                    match look {
                        menu::Look::Head(style) => {
                            menu::draw_head_sample(canvas, x, cy, style, TEXT_COLOR);
                            x += 30;
                        }
                        menu::Look::Face(face) => {
                            self.text.draw_in(face, canvas, x, cy - Text::line_height(13) / 2, 13, TEXT_COLOR, &says);
                            x = rect.right();
                        }
                        _ => {}
                    }
                    if x < rect.right() {
                        self.text.draw(canvas, x, cy - line_high / 2, px, TEXT_COLOR, &says);
                    }
                    let mut right = rect.right() - 14;
                    self.text.draw_centered(canvas, right, cy, 10, DIM_TEXT_COLOR, icons::DOWN);
                    if !hint.is_empty() {
                        right -= 10 + self.text.width(&hint, 11);
                        self.text.draw(canvas, right, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &hint);
                    }
                }
                Control::Entry(naming) => {
                    let typing = self.renaming.as_ref().filter(|(known, _)| *known == naming).map(|(_, typed)| typed.clone());
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                    if typing.is_some() || hovered {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if typing.is_some() { ACCENT } else { 0x555555 });
                    }
                    let (label, now) = match naming {
                        Naming::Title(index) => ("", match self.doc.layers.get(index).map(|layer| &layer.kind) {
                            Some(Kind::Picture(picture)) => picture.title.clone(),
                            _ => String::new(),
                        }),
                        Naming::Address(index) => ("", match self.doc.layers.get(index).map(|layer| &layer.kind) {
                            Some(Kind::Picture(picture)) => picture.chrome.as_ref().map_or(String::new(), |chrome| chrome.address.clone()),
                            _ => String::new(),
                        }),
                        Naming::Suffix(index) => ("", self.doc.exports.get(index).map_or(String::new(), |size| size.suffix.clone())),
                        Naming::Hex => ("Hex", format!("{:06X}", self.palette.and_then(|paint| self.paint_color(paint)).unwrap_or(0))),
                        Naming::Filter => ("", self.layer_filter.clone()),
                        _ => ("", String::new()),
                    };
                    let mut x = rect.x + 9;
                    if !label.is_empty() {
                        self.text.draw(canvas, x, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, label);
                        x += self.text.width(label, 11) + 8;
                    }
                    match typing {
                        Some(typed) => {
                            let shown = self.typed_shown(canvas, (x, cy - line_high / 2), px, typed);
                            self.text.draw(canvas, x, cy - line_high / 2, px, TEXT_COLOR, &shown);
                        }
                        None if now.is_empty() => {
                            // A size with no suffix says, faintly, what its
                            // file's name ends in all the same.
                            let faint = match naming {
                                Naming::Filter => "Filter layers".to_owned(),
                                Naming::Title(_) => "No title".to_owned(),
                                Naming::Address(_) => "example.com".to_owned(),
                                Naming::Suffix(index) => self.doc.exports.get(index).map(ExportSize::file_suffix).filter(|suffix| !suffix.is_empty()).unwrap_or_else(|| "suffix".to_owned()),
                                _ if label.is_empty() => "suffix".to_owned(),
                                _ => "none".to_owned(),
                            };
                            self.text.draw(canvas, x, cy - line_high / 2, px, 0x777777, &faint);
                        }
                        None => self.text.draw(canvas, x, cy - line_high / 2, px, TEXT_COLOR, &now),
                    }
                }
                Control::Picker(part) => self.draw_picker(canvas, rect, part, hovered),
                // A way of lining up, as a short bar in its place in the
                // grid of them, the grid's box drawn under the first.
                Control::StackAlign(at) => {
                    let (cell, pad) = (panel::ALIGN_CELL, panel::ALIGN_PAD);
                    if at == 0 {
                        canvas.round_fill(rect.x - pad, rect.y - pad, 3 * cell.0 + 2 * pad, 3 * cell.1 + 2 * pad, 6, FIELD_COLOR, 256);
                    }
                    let ink = if self.stack_chip_on(control) { ACCENT_TEXT } else if hovered { 0x9A9A9A } else { 0x666666 };
                    canvas.fill(cx - 7, cy - 2, 14, 4, ink);
                }
                Control::Stuck(end) => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                    self.text.draw(canvas, rect.x + 9, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, if end == 0 { "Start" } else { "End" });
                    let stuck = match &shown {
                        Some(Layer { kind: Kind::Shape(shape), .. }) => shape.stuck[usize::from(end).min(1)],
                        _ => None,
                    };
                    let says = match stuck.and_then(|(id, u, _)| self.layer_by_id(id).map(|index| (index, u))) {
                        Some((index, u)) => format!("{}{}", self.doc.layers[index].name(), if u == shapes::WHOLE { "" } else { " \u{b7} edge" }),
                        None => "nothing".to_owned(),
                    };
                    let name: String = says.chars().take(26).collect();
                    self.text.draw(canvas, rect.x + 48, cy - line_high / 2, px, if stuck.is_some() { TEXT_COLOR } else { 0x777777 }, &name);
                }
                Control::Action(action) if rect.w <= 30 && rect.h <= 30 && rect.x > self.width / 2 => {
                    // A square button of the panel: its icon, or its letter.
                    let on = self.action_on(action, &shown);
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 5, if on { ROW_ON } else if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                    let ink = if on { 0xFFFFFF } else { TEXT_COLOR };
                    match action.icon() {
                        // Four corners apart, as Figma's button for them is.
                        None if action == Action::Corners => {
                            for (corner, (dx, dy)) in [(-6, -6), (5, -6), (5, 5), (-6, 5)].into_iter().enumerate() {
                                corners::corner_glyph(canvas, (cx + dx, cy + dy), corner, 4, ink);
                            }
                        }
                        Some(icon) => {
                            if action == Action::FlipDown {
                                // The flip drawn across, turned: down.
                                self.text.draw_centered(canvas, cx, cy, 14, ink, icons::FLIP);
                                canvas.fill(rect.x + 4, cy, rect.w - 8, 1, FIELD_COLOR);
                            } else {
                                self.text.draw_centered(canvas, cx, cy, 14, ink, icon);
                            }
                        }
                        None => self.text.draw_centered(canvas, cx, cy, 12, ink, action.says()),
                    }
                }
                _ => {
                    // A chip: lit when it is what there is now.
                    let on = match control {
                        Control::Ground(ground) => match self.doc.background {
                            Fill::None => ground == Ground::None,
                            Fill::Color(_) => ground == Ground::Color,
                            Fill::Backdrop(_) => ground == Ground::Backdrop,
                            Fill::Gradient(..) => ground == Ground::Gradient,
                            Fill::Radial(..) => ground == Ground::Radial,
                        },
                        Control::Shadow(which) => matches!(&shown, Some(Layer { kind: Kind::Picture(picture), .. }) if picture.shadow == which),
                        Control::Action(Action::Halo) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.halo),
                        Control::Action(Action::Crop) => self.cropping.is_some(),
                        Control::Window(_) | Control::WindowTheme(_) | Control::WindowBar(_) | Control::WindowShadow(_) => self.window_chip_on(control, &shown),
                        Control::StackWay(_) | Control::StackSizing(_) => self.stack_chip_on(control),
                        Control::Action(action) => self.action_on(action, &shown),
                        _ => false,
                    };
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ACCENT } else if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                }
            }
        }
        for ((x, y), words, color, size) in &layout.words {
            self.text.draw(canvas, *x, *y, *size, *color, words);
        }
        for ((x, y), icon, color, size) in &layout.icons {
            self.text.draw_centered(canvas, *x, *y, *size, *color, icon);
        }
        // Over the bar of tools: what was last done, and the name and key
        // of the button under the pointer; with everything hidden, near
        // the bottom where the bar was.
        if let Some((note, when)) = &self.note {
            if when.elapsed().as_secs_f32() < self.note_lasts() {
                let note = note.clone();
                let wide = self.text.width(&note, px) + 24;
                let (x, y) = self.note_at(layout.dock, wide);
                canvas.round_fill(x, y, wide, 32, 8, 0x000000, 210);
                self.text.draw(canvas, x + 12, y + (32 - line_high) / 2, px, TEXT_COLOR, &note);
            }
        }
        let says = match hover {
            Some(Control::Tool(tool)) => TOOLS.iter().find(|(known, ..)| *known == tool).map(|(_, name, key)| format!("{name}  {key}")),
            Some(Control::FrameTool) => Some("Frame  F".to_owned()),
            Some(Control::Group(MORE)) => Some("More tools".to_owned()),
            Some(Control::Action(Action::Capture)) if self.pointer.1 >= layout.dock.y => Some("Capture the screen  Ctrl+G".to_owned()),
            _ => None,
        };
        if let (Some(says), None) = (says, self.flyout) {
            let wide = self.text.width(&says, px) + 16;
            let (x, y) = (self.pointer.0 - wide / 2, layout.dock.y - line_high - 18);
            canvas.round_fill(x.max(4), y, wide, line_high + 8, 6, 0x000000, 220);
            self.text.draw(canvas, x.max(4) + 8, y + 4, px, TEXT_COLOR, &says);
        }
        self.draw_menus(canvas, layout, hover);
    }
}

/// The picture on the button that hides and shows a panel: a window with
/// a strip down the side the panel is on, filled in while it shows.
fn draw_side_icon(canvas: &mut Canvas, rect: Rect, side: u8, open: bool, color: u32) {
    let (w, h) = (16, 14);
    let (x, y) = (rect.x + (rect.w - w) / 2, rect.y + (rect.h - h) / 2);
    canvas.round_outline(x, y, w, h, 3, 1, color);
    let strip = if side == 0 { x + 5 } else { x + w - 6 };
    canvas.fill(strip, y + 1, 1, h - 2, color);
    if open {
        let (from, wide) = if side == 0 { (x + 1, 4) } else { (strip + 1, 4) };
        canvas.round_fill(from, y + 1, wide, h - 2, 1, color, 120);
    }
}

/// A line a pixel wide, from one point to another, in the window.

/// The shadow a panel, the bar of tools or a list floats on: faint and
/// wide, a little below it, fading out evenly, as Figma's are; not a dark
/// rim round it.
fn soft_shadow(canvas: &mut Canvas, place: Rect, radius: i32) {
    for grow in (1..=10).rev() {
        canvas.round_fill(place.x - grow, place.y - grow + 2, place.w + 2 * grow, place.h + 2 * grow, radius + grow, 0x000000, 5);
    }
}
fn line(canvas: &mut Canvas, from: (i32, i32), to: (i32, i32), color: u32) {
    let steps = (to.0 - from.0).abs().max((to.1 - from.1).abs()).max(1);
    for step in 0..=steps {
        let x = from.0 + (to.0 - from.0) * step / steps;
        let y = from.1 + (to.1 - from.1) * step / steps;
        canvas.fill(x, y, 1, 1, color);
    }
}

// ---- The window ----

/// What the window shows: the projects, or a file in the canvas.
enum Shown {
    Home(crate::home::Home),
    Studio(Box<Studio>),
}

/// A file of the canvas from its pages, in a window so large, kept at a
/// path; with pictures, a new frame just the size of the first of them,
/// with nothing behind it, and them in it.
fn opened(pages: Option<Vec<Page>>, mut pictures: Vec<(Image, String)>, path: PathBuf, text: Text, size: (i32, i32), options: &crate::options::Options) -> Studio {
    debug!("canvas opens {} ({}, {} pictures to add)", path.display(), if pages.is_some() { "pages read" } else { "new" }, pictures.len());
    // The first picture is the new frame, when there is no file of pages.
    let fresh = !pictures.is_empty();
    let first = (pages.is_none() && fresh).then(|| pictures.remove(0));
    // A new file with nothing in it begins as the last was switched to: a
    // board, or a page with a frame.
    let pages = pages.unwrap_or_else(|| match first {
        None if board::last_was_board() => board::board_pages(),
        first => {
            let frame = first.map_or_else(|| Doc::new(1200, 675), |(image, name)| framed(image, &name));
            vec![Page::new("Page 1", vec![frame])]
        }
    });
    let dir = options.places.first().cloned().unwrap_or_else(|| options.dir.clone());
    let mut studio = Studio::with_pages(pages, text, size.0, size.1, dir, options.filename.clone());
    for (name, w, h) in &options.frame_sizes {
        studio.add_size(name.clone(), *w, *h);
    }
    studio.path = Some(path);
    studio.convert = options.convert;
    (studio.left_open, studio.right_open) = kept_panels();
    // Boxes drawn as the last were set to be, in any file; and what was
    // done lately, in any, for Ctrl+K.
    studio.recall_shape_style();
    studio.recall_commands();
    if !studio.left_open || !studio.right_open {
        studio.fit();
    }
    // Shown where it was left, if it has been opened before.
    studio.recall_view();
    // Pictures after the first a little down and to the right of it.
    for (index, (image, name)) in pictures.into_iter().enumerate() {
        studio.add_picture(image, &name);
        let (at, by) = (studio.doc.layers.len() - 1, 40.0 * (index + 1) as f32);
        studio.doc.layers[at].translate(by, by);
    }
    (studio.undo, studio.edited) = (Vec::new(), false);
    if fresh {
        studio.save(false);
    }
    studio
}

/// Whether the left panel and the right one were showing when last left,
/// kept in `canvas-panels` among hotshot's state as two words, `left
/// right`, each 1 or 0; both, the first time.
fn kept_panels() -> (bool, bool) {
    let kept = crate::output::state_file("canvas-panels").and_then(|file| std::fs::read_to_string(file).ok());
    read_panels(&kept.unwrap_or_default())
}

fn read_panels(text: &str) -> (bool, bool) {
    let mut words = text.split_whitespace();
    (words.next() != Some("0"), words.next() != Some("0"))
}

fn keep_panels(left: bool, right: bool) {
    let Some(file) = crate::output::state_file("canvas-panels") else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, format!("{} {}\n", u8::from(left), u8::from(right)));
}

/// The canvas in a window of its own, until it is closed. With no files,
/// the projects; with a file of the canvas, that; with pictures, a new
/// file in Drafts with them on it.
pub fn run(files: &[PathBuf], options: &crate::options::Options) -> crate::x11::Result<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::protocol::Event;
    use x11rb::wrapper::ConnectionExt as _;

    let mut display = crate::x11::Display::open()?;
    display.prepare()?;
    let text = Text::load();
    // Where it was when it was last closed, and as large.
    let place = place::last((display.width, display.height));
    let (width, height) = (place.w.max(place::MIN.0), place.h.max(place::MIN.1));

    let mut pages = None;
    let mut pictures = Vec::new();
    let mut path = None;
    for file in files {
        let bytes = std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        if let Some(read) = read_pages(&bytes) {
            (pages, path) = (Some(read), Some(file.clone()));
            continue;
        }
        let image = Image::open(&bytes).ok_or_else(|| format!("{} is neither a picture nor a canvas", file.display()))?;
        let name = file.file_stem().map_or("Picture".to_owned(), |stem| stem.to_string_lossy().into_owned());
        pictures.push((image, name));
    }
    let mut screen = match (pages.is_some(), pictures.is_empty()) {
        (false, true) => {
            let mut home = crate::home::Home::new(text, width, height, options.dir.clone());
            home.convert = options.convert;
            Shown::Home(home)
        }
        _ => {
            let path = path.unwrap_or_else(|| crate::home::fresh_file(&crate::home::drafts(), "Untitled"));
            Shown::Studio(Box::new(opened(pages, pictures, path, text, (width, height), options)))
        }
    };

    let conn = &display.conn;
    let atom = |name: &str| -> crate::x11::Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let window = conn.generate_id()?;
    let events = EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION | EventMask::KEY_PRESS | EventMask::KEY_RELEASE | EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE;
    let look = CreateWindowAux::new().background_pixel(VIEW_COLOR).event_mask(events);
    conn.create_window(x11rb::COPY_FROM_PARENT as u8, window, display.root, place.x as i16, place.y as i16, width as u16, height as u16, 0, WindowClass::INPUT_OUTPUT, x11rb::COPY_FROM_PARENT, &look)?;
    place::ask(conn, window, place)?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"hotshot\0Hotshot\0")?;
    let (protocols, delete) = (atom("WM_PROTOCOLS")?, atom("WM_DELETE_WINDOW")?);
    conn.change_property32(PropMode::REPLACE, window, protocols, AtomEnum::ATOM, &[delete])?;
    let (utf8, net_name) = (atom("UTF8_STRING")?, atom("_NET_WM_NAME")?);
    // Files dropped on it from a file manager, by XDND: a picture of any
    // kind hotshot reads goes on the canvas, or begins a file.
    let dnd = Dnd { enter: atom("XdndEnter")?, position: atom("XdndPosition")?, status: atom("XdndStatus")?, drop: atom("XdndDrop")?, finished: atom("XdndFinished")?, selection: atom("XdndSelection")?, copy: atom("XdndActionCopy")?, uris: atom("text/uri-list")? };
    conn.change_property32(PropMode::REPLACE, window, atom("XdndAware")?, AtomEnum::ATOM, &[5])?;
    let mut dropped: Option<(u32, u32)> = None;
    // Where in the window what is dragged over it was last, for what is
    // dropped to go there.
    let mut drop_at: Option<(i32, i32)> = None;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
    conn.map_window(window)?;
    // The pointer from XInput 2.1, for a trackpad's scrolling in parts of
    // a notch; otherwise the old events, as ever.
    let mut smooth = wheel::start(conn, window);
    debug!("canvas scrolls {}", if smooth.is_some() { "smoothly, by XInput 2.1" } else { "a notch at a time" });
    conn.flush()?;

    // The window's pixels, in memory shared with the server where it
    // will have that, for them to be sent without going through the socket.
    let mut frame = display.pixels((width * height) as usize);
    let (mut size, mut title, mut cursor) = ((width, height), String::new(), None::<crate::editor::Cursor>);
    let mut time = 0u32;
    let mut saved = std::time::Instant::now();
    // What came while a frame was being drawn sharp, still to be heard.
    let mut held: Vec<Event> = Vec::new();
    loop {
        // What has happened: a run of the pointer's movements told as
        // where it last was, and a run of the wheel's notches all taken
        // before it is drawn once. While something said shows, a look
        // every 50 ms, to take it away when its time is up; and once the
        // page zoomed or moved has been still a moment, a look then, to
        // draw it sharp. Otherwise nothing until something happens.
        let wait = match &mut screen {
            Shown::Studio(studio) => [studio.note_left().map(|left| left.min(std::time::Duration::from_millis(50))), studio.settle_in(), studio.sizes_wait(), studio.fine_wait(), studio.board_wait()].into_iter().flatten().min(),
            Shown::Home(home) => home.wait(),
        };
        let mut events: Vec<Event> = match (held.is_empty(), wait) {
            (false, _) => std::mem::take(&mut held),
            (true, Some(wait)) => event_within(conn, wait)?.into_iter().collect(),
            (true, None) => vec![conn.wait_for_event()?],
        };
        if let Shown::Studio(studio) = &mut screen {
            studio.note_left();
            studio.settle();
            studio.board_tick();
            studio.take_sizes();
            studio.take_fine();
        }
        if let Shown::Home(home) = &mut screen {
            home.take_batch();
        }
        while let Some(event) = conn.poll_for_event()? {
            events.push(event);
        }
        let (mut outcome, mut homeward) = (Outcome::Continue, crate::home::Outcome::Continue);
        let mut motion = None;
        let moved = |screen: &mut Shown, (x, y, mods): (i32, i32, Mods)| match screen {
            Shown::Home(home) => home.pointer_moved(x, y),
            Shown::Studio(studio) => studio.pointer_moved(x, y, mods),
        };
        for event in events {
            // From XInput where the server has it: the old kind of event,
            // or a scrolling axis moved, taken as the wheel at once.
            let heard = match smooth.as_mut() {
                Some(smooth) => smooth.hear(conn, event),
                None => wheel::Heard::Core(event),
            };
            let event = match heard {
                wheel::Heard::Core(event) => event,
                wheel::Heard::Scroll(x, y, state, right, down) => {
                    let at = (x, y, crate::x11::mods(state));
                    match &mut screen {
                        Shown::Home(home) => home.scrolled(down),
                        Shown::Studio(studio) => {
                            studio.wheel_at(x, y, at.2);
                            studio.scrolled(right, down);
                        }
                    }
                    motion = Some(at);
                    continue;
                }
                wheel::Heard::Nothing => continue,
            };
            match event {
                Event::MotionNotify(motion_event) => motion = Some((i32::from(motion_event.event_x), i32::from(motion_event.event_y), crate::x11::mods(motion_event.state.into()))),
                // A notch of the wheel on the canvas: the pointer only put
                // where it is, and what it is over found once after the
                // run of them, not again for each.
                Event::ButtonPress(press) if (4..=7).contains(&press.detail) && matches!(screen, Shown::Studio(_)) => {
                    let at = (i32::from(press.event_x), i32::from(press.event_y), crate::x11::mods(press.state.into()));
                    if let Shown::Studio(studio) = &mut screen {
                        studio.wheel_at(at.0, at.1, at.2);
                        outcome = studio.pressed(press.detail, press.time);
                    }
                    motion = Some(at);
                }
                Event::ButtonPress(press) => {
                    if let Some(at) = motion.take() {
                        moved(&mut screen, at);
                    }
                    time = press.time;
                    moved(&mut screen, (i32::from(press.event_x), i32::from(press.event_y), crate::x11::mods(press.state.into())));
                    match &mut screen {
                        Shown::Home(home) => {
                            home.set_mods(crate::x11::mods(press.state.into()));
                            homeward = home.pressed(press.detail, press.time);
                            debug!("home button {} at {},{} -> {homeward:?}", press.detail, press.event_x, press.event_y);
                        }
                        Shown::Studio(studio) => {
                            outcome = studio.pressed(press.detail, press.time);
                            debug!("canvas button {} at {},{} -> {outcome:?}", press.detail, press.event_x, press.event_y);
                        }
                    }
                }
                Event::ButtonRelease(release) if release.detail <= 3 => {
                    if let Some(at) = motion.take() {
                        moved(&mut screen, at);
                    }
                    if let Shown::Studio(studio) = &mut screen {
                        studio.released();
                    }
                }
                Event::KeyPress(press) | Event::KeyRelease(press) => {
                    let down = event_is_press(&event);
                    let state: u16 = press.state.into();
                    time = press.time;
                    let key = display.key(press.detail, state);
                    match &mut screen {
                        Shown::Home(home) if down => {
                            homeward = home.key(key, crate::x11::mods(state));
                            debug!("home key {key:?} {:?} -> {homeward:?}", crate::x11::mods(state));
                        }
                        Shown::Home(_) => {}
                        Shown::Studio(studio) => {
                            outcome = studio.key(key, crate::x11::mods(state), down);
                            // Shift or Alt pressed or let go partway through a
                            // drag changes it then: the keys as they are now,
                            // which the event, told as they were, does not say.
                            if studio.dragging() {
                                if let Ok(Ok(now)) = conn.query_pointer(window).map(|cookie| cookie.reply()) {
                                    studio.mods_changed(crate::x11::mods(now.mask.into()));
                                }
                            }
                            if down {
                                debug!("canvas key {key:?} {:?} -> {outcome:?}", crate::x11::mods(state));
                            }
                        }
                    }
                }
                Event::Expose(_) => match &mut screen {
                    Shown::Home(home) => home.resize(size.0, size.1),
                    Shown::Studio(studio) => studio.dirty = true,
                },
                Event::ConfigureNotify(configured) if configured.window == window => {
                    // Never drawn smaller than the canvas and the projects are
                    // laid out for, however small the window is made: what
                    // does not fit is cut off by the window.
                    let now = (i32::from(configured.width).max(place::MIN.0), i32::from(configured.height).max(place::MIN.1));
                    if now != size {
                        size = now;
                        crate::x11::let_go(conn, std::mem::replace(&mut frame, display.pixels((now.0 * now.1) as usize)));
                        match &mut screen {
                            Shown::Home(home) => home.resize(now.0, now.1),
                            Shown::Studio(studio) => studio.resize(now.0, now.1),
                        }
                    }
                }
                Event::ClientMessage(message) if message.type_ == protocols && message.data.as_data32()[0] == delete => outcome = Outcome::Quit,
                // Something dragged over the window is said to be welcome,
                // to be copied; let go, what it is is asked for after.
                Event::ClientMessage(message) if message.type_ == dnd.position => {
                    let source = message.data.as_data32()[0];
                    let packed = message.data.as_data32()[2];
                    let (root_x, root_y) = ((packed >> 16) as i16, (packed & 0xFFFF) as i16);
                    drop_at = conn.translate_coordinates(display.root, window, root_x, root_y).ok().and_then(|cookie| cookie.reply().ok()).map(|reply| (i32::from(reply.dst_x), i32::from(reply.dst_y)));
                    let status = ClientMessageEvent::new(32, source, dnd.status, [window, 1, 0, 0, dnd.copy]);
                    conn.send_event(false, source, EventMask::NO_EVENT, status)?;
                }
                Event::ClientMessage(message) if message.type_ == dnd.drop => {
                    let data = message.data.as_data32();
                    dropped = Some((data[0], data[2]));
                }
                Event::ClientMessage(message) if message.type_ == dnd.enter => {}
                _ => {}
            }
            if outcome != Outcome::Continue || homeward != crate::home::Outcome::Continue {
                break;
            }
        }
        if let Some(at) = motion {
            moved(&mut screen, at);
        }
        // Files dropped: the paths asked for, the one who dropped them
        // told it is done, and the pictures among them put on the
        // canvas, or a new file begun with them on the projects; a file
        // of the canvas dropped is opened.
        let mut dropped_pictures = Vec::new();
        if let Some((source, when)) = dropped.take() {
            let words = selection(conn, window, dnd.selection, dnd.uris, atom("HOTSHOT_CANVAS")?, atom("INCR")?, when);
            let finished = ClientMessageEvent::new(32, source, dnd.finished, [window, u32::from(words.is_some()), dnd.copy, 0, 0]);
            conn.send_event(false, source, EventMask::NO_EVENT, finished)?;
            let paths = words.map(|words| pasted_paths(&String::from_utf8_lossy(&words))).unwrap_or_default();
            debug!("dropped {paths:?}");
            let canvas_file = paths.iter().find(|path| path.extension().is_some_and(|ending| ending == "hotshot")).cloned();
            dropped_pictures = paths.iter().filter(|path| crate::convert::reads(path)).filter_map(|path| Some((Image::open(&std::fs::read(path).ok()?)?, path.file_stem()?.to_string_lossy().into_owned()))).collect();
            match &mut screen {
                Shown::Studio(studio) => {
                    for (image, name) in dropped_pictures.drain(..) {
                        studio.drop_picture(image, &name, drop_at);
                    }
                    if let Some(path) = canvas_file {
                        studio.open_next = Some(path);
                        outcome = Outcome::Open;
                    }
                }
                Shown::Home(_) => {
                    if let Some(path) = canvas_file.filter(|_| dropped_pictures.is_empty()) {
                        homeward = crate::home::Outcome::Open(path);
                    }
                }
            }
        }
        // Between the screens: a file opened or begun from the projects,
        // and the projects again from a file, which is kept first.
        screen = match (screen, homeward) {
            (Shown::Home(home), _) if !dropped_pictures.is_empty() => {
                let path = crate::home::fresh_file(&home.folder(), "Dropped");
                let mut studio = opened(None, std::mem::take(&mut dropped_pictures), path, home.into_text(), size, options);
                studio.save(false);
                Shown::Studio(Box::new(studio))
            }
            (Shown::Home(home), crate::home::Outcome::Open(path)) => {
                let pages = std::fs::read(&path).ok().and_then(|bytes| read_pages(&bytes));
                Shown::Studio(Box::new(opened(pages, Vec::new(), path, home.into_text(), size, options)))
            }
            (Shown::Home(home), crate::home::Outcome::New(dir, picture)) => {
                let pictures: Vec<(Image, String)> = picture.iter().filter_map(|file| Some((Image::open(&std::fs::read(file).ok()?)?, "Capture".to_owned()))).collect();
                let path = crate::home::fresh_file(&dir, "Untitled");
                let mut studio = opened(None, pictures, path, home.into_text(), size, options);
                studio.save(false);
                Shown::Studio(Box::new(studio))
            }
            // A picture pasted on the projects: a new file in the folder
            // shown, its frame the picture's size, holding it.
            (Shown::Home(home), crate::home::Outcome::Paste(dir)) => {
                let (clipboard, png, slot, incr, uris) = (atom("CLIPBOARD")?, atom("image/png")?, atom("HOTSHOT_CANVAS")?, atom("INCR")?, atom("text/uri-list")?);
                let picture = selection(conn, window, clipboard, png, slot, incr, time).and_then(|bytes| Image::open(&bytes));
                let picture = picture.or_else(|| {
                    let words = selection(conn, window, clipboard, uris, slot, incr, time)
                        .or_else(|| selection(conn, window, clipboard, utf8, slot, incr, time))?;
                    let path = pasted_path(&String::from_utf8_lossy(&words))?;
                    Image::open(&std::fs::read(path).ok()?)
                });
                match picture {
                    Some(image) => {
                        let path = crate::home::fresh_file(&dir, "Pasted");
                        let mut studio = opened(Some(vec![Page::new("Page 1", vec![framed(image, "Pasted")])]), Vec::new(), path, home.into_text(), size, options);
                        studio.save(false);
                        studio.fit();
                        Shown::Studio(Box::new(studio))
                    }
                    None => Shown::Home(home),
                }
            }
            (Shown::Home(_), crate::home::Outcome::Quit) => break,
            (Shown::Studio(mut studio), _) if outcome == Outcome::Open && studio.open_next.is_some() => {
                studio.finish();
                studio.keep_view();
                studio.save(false);
                let path = studio.open_next.take().unwrap_or_default();
                let pages = std::fs::read(&path).ok().and_then(|bytes| read_pages(&bytes));
                Shown::Studio(Box::new(opened(pages, Vec::new(), path, studio.into_text(), size, options)))
            }
            (Shown::Studio(mut studio), _) if outcome == Outcome::Home => {
                studio.finish();
                studio.keep_view();
                studio.save(false);
                let place = studio.path.as_ref().and_then(|path| path.parent()).map(Path::to_path_buf);
                let mut home = crate::home::Home::new(studio.into_text(), size.0, size.1, options.dir.clone());
                home.convert = options.convert;
                if let Some(dir) = place.clone().filter(|dir| *dir != crate::home::drafts() && dir.parent() == Some(crate::home::library().as_path())) {
                    home.place = crate::home::Place::Project(dir);
                } else if place.is_some() {
                    home.place = crate::home::Place::Drafts;
                }
                Shown::Home(home)
            }
            (screen, _) => screen,
        };
        if let Shown::Studio(studio) = &mut screen {
            match outcome {
                Outcome::Quit => {
                    studio.finish();
                    studio.keep_view();
                    studio.save(false);
                    break;
                }
                Outcome::Paste => {
                    let picture = selection(conn, window, atom("CLIPBOARD")?, atom("image/png")?, atom("HOTSHOT_CANVAS")?, atom("INCR")?, time);
                    // A picture of a kind other than PNG, or a file copied
                    // in a file manager, an AVIF or a JPEG XL as well.
                    let picture = match picture {
                        Some(picture) => Some(picture),
                        None => [atom("image/avif")?, atom("image/jxl")?, atom("image/webp")?, atom("image/jpeg")?].into_iter().find_map(|kind| selection(conn, window, atom("CLIPBOARD").ok()?, kind, atom("HOTSHOT_CANVAS").ok()?, atom("INCR").ok()?, time)).or_else(|| {
                            let words = selection(conn, window, atom("CLIPBOARD").ok()?, atom("text/uri-list").ok()?, atom("HOTSHOT_CANVAS").ok()?, atom("INCR").ok()?, time)?;
                            let path = pasted_path(&String::from_utf8_lossy(&words)).filter(|path| crate::convert::reads(path))?;
                            std::fs::read(path).ok()
                        }),
                    };
                    let words = match picture {
                        Some(_) => None,
                        None => selection(conn, window, atom("CLIPBOARD")?, utf8, atom("HOTSHOT_CANVAS")?, atom("INCR")?, time).and_then(|bytes| String::from_utf8(bytes).ok()),
                    };
                    studio.pasted(picture, words);
                }
                Outcome::Capture => {
                    // Out of the way, the screen captured by a hotshot of
                    // its own into a file, and the window back.
                    conn.unmap_window(window)?;
                    conn.flush()?;
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let file = std::env::temp_dir().join(format!("hotshot-canvas-{}.png", std::process::id()));
                    let _ = std::process::Command::new(std::env::current_exe()?).arg("-o").arg(&file).status();
                    conn.map_window(window)?;
                    conn.flush()?;
                    studio.captured(&file);
                }
                Outcome::Continue | Outcome::Home | Outcome::Open => {}
            }
            // Kept as it is changed, as Figma keeps its files: once it
            // has been still a moment.
            if studio.edited && studio.drag == Drag::None && saved.elapsed().as_secs_f32() > 1.5 {
                studio.save(false);
                saved = std::time::Instant::now();
            }
        } else if outcome == Outcome::Quit {
            break;
        }
        let (now, wanted, dirty) = match &mut screen {
            Shown::Home(home) => (home.title(), home.cursor(), home.take_dirty()),
            Shown::Studio(studio) => (studio.title(), studio.cursor(), studio.take_dirty()),
        };
        if now != title {
            conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, now.as_bytes())?;
            conn.change_property8(PropMode::REPLACE, window, net_name, utf8, now.as_bytes())?;
            title = now;
        }
        if cursor != Some(wanted) {
            if let Some(id) = display.cursor_of(wanted) {
                conn.change_window_attributes(window, &ChangeWindowAttributesAux::new().cursor(id))?;
            }
            cursor = Some(wanted);
        }
        if dirty {
            match &mut screen {
                Shown::Home(home) => home.draw(&mut frame),
                // Taken off the display while a frame is drawn sharp, to be
                // heard next; the wheel, a click or a key gives it up.
                Shown::Studio(studio) => studio.draw_stopping(&mut frame, &mut || {
                    while let Ok(Some(event)) = conn.poll_for_event() {
                        let stops = stops_drawing(&event);
                        held.push(event);
                        if stops {
                            return true;
                        }
                    }
                    false
                }),
            }
            crate::x11::put_shared(conn, window, gc, size.0, size.1, &frame)?;
        }
        conn.flush()?;
    }
    // Kept for the next time, as it is now.
    place::keep(conn, window, display.root);
    Ok(())
}
/// A frame just the size of a picture, holding it.
fn framed(image: Image, name: &str) -> Doc {
    let (w, h) = (image.width(), image.height());
    let mut frame = Doc::new(w, h);
    // Exported at its own size, pixel for pixel, as a screenshot is wanted.
    (frame.background, frame.exports) = (Fill::None, vec![ExportSize::new(1.0)]);
    frame.layers.push(Layer::picture(Picture::new(Arc::new(image), name.to_owned(), (w as f32 / 2.0, h as f32 / 2.0))));
    frame
}

/// The file a copied path or URI names, from the first line of it that
/// names one that is there.
fn pasted_path(words: &str) -> Option<PathBuf> {
    pasted_paths(words).into_iter().next()
}

/// Every file a list of copied or dropped paths or URIs names that is there.
fn pasted_paths(words: &str) -> Vec<PathBuf> {
    words.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).filter_map(|line| {
        let path = match line.strip_prefix("file://") {
            Some(rest) => {
                // Past the host, if one is said; and %20 and the rest put back.
                let rest = if rest.starts_with('/') { rest } else { rest.find('/').map_or(rest, |at| &rest[at..]) };
                let bytes = rest.as_bytes();
                let mut out = Vec::new();
                let mut at = 0;
                while at < bytes.len() {
                    match (bytes[at], bytes.get(at + 1..at + 3).and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok())) {
                        (b'%', Some(byte)) => {
                            out.push(byte);
                            at += 3;
                        }
                        (byte, _) => {
                            out.push(byte);
                            at += 1;
                        }
                    }
                }
                PathBuf::from(String::from_utf8_lossy(&out).into_owned())
            }
            None => PathBuf::from(line),
        };
        path.is_file().then_some(path)
    })
    .collect()
}


/// The next event, if one comes within so long; nothing if none does,
/// and nothing done meanwhile but wait for one.
pub(crate) fn event_within(conn: &x11rb::rust_connection::RustConnection, wait: std::time::Duration) -> crate::x11::Result<Option<x11rb::protocol::Event>> {
    use std::os::fd::AsRawFd;
    use x11rb::connection::Connection;
    let until = std::time::Instant::now() + wait;
    loop {
        if let Some(event) = conn.poll_for_event()? {
            return Ok(Some(event));
        }
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        let mut socket = libc::pollfd { fd: conn.stream().as_raw_fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: one pollfd, which lives through the call.
        unsafe { libc::poll(&mut socket, 1, left.as_millis().clamp(1, i32::MAX as u128) as i32) };
    }
}

/// Whether something come while a frame is drawn sharp is worth giving
/// that up for, to see to at once: the wheel or a trackpad's scrolling,
/// a click, a key, a drag with the middle button, or the window changed;
/// not the pointer only moved.
fn stops_drawing(event: &x11rb::protocol::Event) -> bool {
    use x11rb::protocol::Event;
    match event {
        Event::ButtonPress(_) | Event::KeyPress(_) | Event::ConfigureNotify(_) | Event::ClientMessage(_) | Event::XinputButtonPress(_) => true,
        Event::MotionNotify(motion) => u16::from(motion.state) & 0x200 != 0,
        // Scrolling axes are past the first two, across and down.
        Event::XinputMotion(motion) => motion.valuator_mask.first().is_some_and(|mask| mask & !0b11 != 0) || motion.valuator_mask.iter().skip(1).any(|&mask| mask != 0) || motion.button_mask.first().is_some_and(|mask| mask & 0b100 != 0),
        _ => false,
    }
}

/// The atoms of XDND, by which files are dragged in from other programs.
struct Dnd {
    enter: u32,
    position: u32,
    status: u32,
    drop: u32,
    finished: u32,
    selection: u32,
    copy: u32,
    uris: u32,
}

fn event_is_press(event: &x11rb::protocol::Event) -> bool {
    matches!(event, x11rb::protocol::Event::KeyPress(_))
}

/// What whoever has a selection hands over of a kind, if they do within
/// a second: in one piece, or in as many as it takes, for a large one.
fn selection(conn: &x11rb::rust_connection::RustConnection, window: u32, which: u32, kind: u32, slot: u32, incr: u32, time: u32) -> Option<Vec<u8>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::protocol::Event;
    conn.convert_selection(window, which, kind, slot, time).ok()?;
    conn.flush().ok()?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match conn.poll_for_event().ok()? {
            Some(Event::SelectionNotify(notice)) if notice.property == x11rb::NONE => return None,
            Some(Event::SelectionNotify(_)) => break,
            Some(_) => {}
            None if std::time::Instant::now() > until => return None,
            None => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    let reply = conn.get_property(true, window, slot, AtomEnum::ANY, 0, u32::MAX / 4).ok()?.reply().ok()?;
    if reply.type_ != incr {
        return (reply.type_ == kind && !reply.value.is_empty()).then_some(reply.value);
    }
    // Large: piece after piece, each a new value of the property, until
    // one with nothing in it.
    conn.flush().ok()?;
    let mut bytes = Vec::new();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(4);
    loop {
        match conn.poll_for_event().ok()? {
            Some(Event::PropertyNotify(notice)) if notice.atom == slot && notice.state == Property::NEW_VALUE => {
                let piece = conn.get_property(true, window, slot, AtomEnum::ANY, 0, u32::MAX / 4).ok()?.reply().ok()?;
                conn.flush().ok()?;
                if piece.value.is_empty() {
                    return Some(bytes);
                }
                bytes.extend_from_slice(&piece.value);
            }
            Some(_) => {}
            None if std::time::Instant::now() > until => return None,
            None => std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> Studio {
        Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into())
    }

    fn checker(width: i32, height: i32) -> Image {
        Image::new(width, height, (0..width * height).map(|at| if (at % width / 10 + at / width / 10) % 2 == 0 { 0xFFFF0000 } else { 0xFF0000FF }).collect())
    }

    #[test]
    fn shapes_from_capture_mode_are_listed_by_what_they_are() {
        for (tool, name) in [(Tool::Blur, "Blur"), (Tool::Spotlight, "Spotlight"), (Tool::Zoom, "Zoom"), (Tool::Rect, "Rectangle")] {
            assert_eq!(Layer::shape(Shape::new(tool, 0xFF0000, 2.0, (10.0, 10.0))).name(), name);
        }
    }

    #[test]
    fn a_shape_casts_its_shadow_and_several_are_sized_together() {
        let mut text = Text::load();
        let mut doc = Doc::new(200, 200);
        doc.background = Fill::Color(0xFFFFFF);
        let mut boxed = Shape::new(Tool::Rect, 0x3E63DD, 2.0, (50.0, 50.0));
        (boxed.points[1], boxed.filled, boxed.fill) = ((150.0, 120.0), true, 0x3E63DD);
        boxed.shadow = Some(frame::Layer { x: 0, y: 20, blur: 10, spread: 0, color: 0x8000_0000 });
        doc.layers.push(Layer::shape(boxed.clone()));
        let (_, _, out) = doc.render(1.0, &mut text);
        // Below it, its shadow; far off, nothing; drawn at twice, the same.
        assert!(out[132 * 200 + 100] & 0xFF < 0xC0, "{:06x}", out[132 * 200 + 100]);
        assert_eq!(out[180 * 200 + 20] & 0xFF_FFFF, 0xFFFFFF);
        assert_eq!(out[100 * 200 + 100] & 0xFF_FFFF, 0x3E63DD);
        let (_, _, twice) = doc.render(2.0, &mut text);
        assert!(twice[264 * 400 + 200] & 0xFF < 0xC0);
        // Two picked out, made twice as wide together from their left.
        let mut studio = studio();
        let mut other = boxed.clone();
        other.translate(200.0, 0.0);
        studio.doc.layers = vec![Layer::shape(boxed), Layer::shape(other)];
        studio.selected = vec![0, 1];
        let wide = studio.field(Field::W).unwrap();
        let left = studio.field(Field::X).unwrap();
        studio.set_field(Field::W, wide * 2.0);
        let Kind::Shape(moved) = studio.doc.layers[1].kind.clone() else { panic!() };
        assert_eq!(moved.points[0].0, left + (250.0 - left) * 2.0);
        assert!((studio.field(Field::W).unwrap() - wide * 2.0).abs() < 8.0);
        // A line made longer, and turned, from its start.
        let mut line = Shape::new(Tool::Arrow, 0, 2.0, (10.0, 10.0));
        line.points[1] = (40.0, 50.0);
        studio.doc.layers = vec![Layer::shape(line)];
        studio.selected = vec![0];
        assert_eq!(studio.field(Field::Length), Some(50.0));
        studio.set_field(Field::Length, 100.0);
        studio.set_field(Field::LineAngle, 90.0);
        let Kind::Shape(line) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!(line.points[1], (10.0, 110.0));
    }

    #[test]
    fn several_are_matched_to_the_first_picked_put_to_the_frame_and_turned() {
        let mut studio = studio();
        let small = Picture::new(Arc::new(checker(100, 50)), "small".into(), (100.0, 100.0));
        let mut large = Picture::new(Arc::new(checker(300, 200)), "large".into(), (500.0, 300.0));
        large.radius = 12.0;
        studio.doc.layers = vec![Layer::picture(small), Layer::picture(large)];
        // Picked large first: the small one takes after it.
        studio.selected = vec![1, 0];
        studio.act(Action::Match(2));
        studio.act(Action::Match(3));
        let Kind::Picture(small) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert_eq!((small.w, small.h, small.radius), (300.0, 200.0, 12.0));
        studio.act(Action::ToFrame(6));
        let Kind::Picture(small) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert_eq!((small.cx, small.cy), (400.0, 300.0));
        studio.selected = vec![0];
        studio.act(Action::Turn(90));
        let Kind::Picture(small) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert!((small.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4 && (small.cx, small.cy) == (400.0, 300.0));
    }

    #[test]
    fn the_colors_of_the_selection_are_each_once_with_the_layers_that_use_them() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 2.0, (0.0, 0.0));
        (boxed.points[1], boxed.filled, boxed.fill) = ((50.0, 50.0), true, 0xE5484D);
        let arrow = Shape::new(Tool::Arrow, 0xE5484D, 2.0, (100.0, 0.0));
        let mut words = Shape::new(Tool::Text, 0x1A1A1A, 2.0, (0.0, 100.0));
        words.text = "hi".into();
        studio.doc.layers = vec![Layer::shape(boxed), Layer::shape(arrow), Layer::shape(words)];
        studio.selected = vec![0, 1, 2];
        // The box's line and fill are one color: it counts once.
        assert_eq!(studio.selection_colors(), vec![(0xE5484D, 2), (0x1A1A1A, 1)]);
        assert_eq!(studio.color_uses(0xE5484D), vec![(0, "stroke"), (0, "fill"), (1, "stroke")]);
        assert_eq!(studio.color_uses(0x1A1A1A), vec![(2, "text")]);
        studio.paint(Paint::Recolor(0xE5484D), Some(0xFFC53D));
        assert_eq!(studio.selection_colors(), vec![(0xFFC53D, 2), (0x1A1A1A, 1)]);
    }

    #[test]
    fn a_pasted_path_or_uri_names_its_file_and_a_picture_gets_a_frame_its_size() {
        let dir = std::env::temp_dir().join(format!("hotshot-paste-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a shot.png");
        std::fs::write(&file, b"x").unwrap();
        let uri = format!("# copied\nfile://{}\n", file.display().to_string().replace(' ', "%20"));
        assert_eq!(pasted_path(&uri), Some(file.clone()));
        assert_eq!(pasted_path(&format!("{}\n", file.display())), Some(file.clone()));
        assert_eq!(pasted_path("not a path"), None);
        std::fs::remove_dir_all(&dir).unwrap();
        let frame = framed(checker(120, 80), "Pasted");
        assert_eq!((frame.width, frame.height, frame.layers.len(), frame.background), (120, 80, 1, Fill::None));
        let Kind::Picture(picture) = &frame.layers[0].kind else { panic!() };
        assert_eq!((picture.cx, picture.cy, picture.w), (60.0, 40.0, 120.0));
    }

    #[test]
    fn a_size_saved_is_named_for_its_frame_never_for_its_size_and_kept_in_the_tests_own_config() {
        let mut studio = studio();
        let file = crate::options::config_file();
        assert!(file.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_file(&file);
        studio.run_menu(menu::MenuAct::SaveSize);
        studio.doc.name = "Phone shot".into();
        studio.run_menu(menu::MenuAct::SaveSize);
        let kept = std::fs::read_to_string(&file).unwrap();
        assert_eq!(kept, "frame-size = \"My size 1 = 800x600\"\nframe-size = \"Phone shot = 800x600\"\n");
        assert_eq!(studio.frame_sizes.len(), 2);
        let _ = std::fs::remove_dir_all(file.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn layers_are_dragged_up_and_down_their_list_renamed_and_filtered() {
        let mut studio = studio();
        for name in ["back", "middle", "front"] {
            studio.add_picture(checker(20, 20), name);
        }
        let row = |studio: &mut Studio, index: usize| studio.control_rect(Control::Layer(index)).unwrap().center();
        // The front one, listed first, dragged down onto the back one.
        let (from, to) = (row(&mut studio, 2), row(&mut studio, 0));
        studio.pointer_moved(from.0, from.1, Mods::default());
        studio.pressed(1, 1000);
        studio.pointer_moved(to.0, to.1 - 3, Mods::default());
        studio.pointer_moved(to.0, to.1, Mods::default());
        studio.released();
        let names: Vec<String> = studio.doc.layers.iter().map(Layer::name).collect();
        assert_eq!((names, studio.selected.clone()), (vec!["front".to_owned(), "back".to_owned(), "middle".to_owned()], vec![0]));
        // Double-clicked, a picture is renamed where it is listed.
        let at = row(&mut studio, 1);
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.pressed(1, 5000);
        studio.released();
        studio.pressed(1, 5100);
        studio.released();
        assert_eq!(studio.renaming.as_ref().map(|(naming, _)| *naming), Some(Naming::Layer(1)));
        studio.renaming = Some((Naming::Layer(1), "Behind".into()));
        studio.commit_name();
        assert_eq!(studio.doc.layers[1].name(), "Behind");
        // Narrowed to those whose names have what is typed.
        studio.layer_filter = "mid".into();
        let listed: Vec<usize> = studio.layout().controls.iter().filter_map(|(_, control)| match control {
            Control::Layer(index) => Some(*index),
            _ => None,
        }).collect();
        assert_eq!(listed, vec![2]);
    }

    #[test]
    fn a_shape_is_renamed_where_it_is_listed_and_keeps_its_name_in_its_file() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        boxed.points[1] = (200.0, 160.0);
        let mut arrow = Shape::new(Tool::Arrow, 0xE5484D, 3.5, (300.0, 100.0));
        arrow.points[1] = (380.0, 160.0);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.doc.layers.push(Layer::shape(arrow));
        studio.changed();
        // Double-clicked in the list, the box's name is typed over.
        let at = studio.control_rect(Control::Layer(0)).unwrap().center();
        studio.pointer_moved(at.0, at.1, Mods::default());
        for time in [5000, 5100] {
            studio.pressed(1, time);
            studio.released();
        }
        assert_eq!(studio.renaming.clone(), Some((Naming::Layer(0), "Rectangle".to_owned())));
        for letter in "Card".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!(studio.doc.layers[0].name(), "Card");
        // The arrow from its menu.
        let at = studio.control_rect(Control::Layer(1)).unwrap().center();
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.pressed(3, 6000);
        choose(&mut studio, "Rename");
        studio.renaming = Some((Naming::Layer(1), "Look here".into()));
        studio.commit_name();
        // Kept in the file; undone, the name it had.
        let back = Doc::read(&studio.doc.write()).unwrap();
        assert_eq!(back.layers.iter().map(Layer::name).collect::<Vec<_>>(), ["Card", "Look here"]);
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert_eq!(studio.doc.layers[1].name(), "Arrow");
        // A file from before names has them called for what they are.
        let old = String::from_utf8_lossy(&studio.doc.write()).lines().filter(|line| !line.starts_with("named ")).collect::<Vec<_>>().join("\n");
        let back = Doc::read(old.as_bytes()).unwrap();
        assert_eq!(back.layers.iter().map(Layer::name).collect::<Vec<_>>(), ["Rectangle", "Arrow"]);
    }

    #[test]
    fn undo_closes_an_open_list_first_and_crowded_frame_names_are_left_out() {
        let mut studio = studio();
        studio.add_picture(checker(40, 40), "Shot");
        let at = studio.control_rect(Control::Layer(0)).unwrap().center();
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.pressed(3, 1000);
        assert!(studio.menu.is_some());
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert!(studio.menu.is_none() && studio.doc.layers.is_empty(), "closed, and undone");
        // Frames side by side, seen from far off: the names that would
        // run into each other are not drawn.
        for at in 1..6 {
            studio.new_frame((at as f32 * 900.0, 0.0), (800.0, 600.0));
        }
        studio.zoom = 1.0;
        assert_eq!(studio.frame_labels().len(), 6);
        studio.zoom = 0.02;
        let shown = studio.frame_labels();
        assert!(shown.len() < 6 && shown[0].0 == studio.active);
        for (at, (_, a, _)) in shown.iter().enumerate() {
            assert!(shown[at + 1..].iter().all(|(_, b, _)| a.intersect(*b).is_empty()));
        }
    }

    #[test]
    fn what_is_said_along_the_bottom_keeps_off_the_size_of_what_is_picked_out() {
        let mut studio = studio();
        studio.add_picture(checker(400, 300), "Shot");
        // The picture low in the view, its size just over the bar of tools.
        let dock = studio.layout().dock;
        let (x, y, w, h) = studio.doc.layers[0].bounds(&mut studio.text);
        studio.pan.1 = dock.y as f32 - 60.0 - (studio.doc.y + y + h) * studio.zoom;
        studio.pan.0 = (dock.x + dock.w / 2) as f32 - (studio.doc.x + x + w / 2.0) * studio.zoom;
        studio.say("Copied");
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let pill = studio.pill.expect("a size shown");
        let wide = studio.text.width("Copied", 12) + 24;
        let (x, y) = (dock.x + (dock.w - wide) / 2, dock.y - 44);
        assert!(!pill.intersect(Rect::new(x, y, wide, 32)).is_empty(), "where it would be, it is over {pill:?}");
        let note = studio.note_at(dock, wide);
        assert!(pill.intersect(Rect::new(note.0, note.1, wide, 32)).is_empty(), "the note at {note:?} is beside the pill at {pill:?}");
    }

    #[test]
    fn the_export_tab_chooses_frames_and_says_what_was_written() {
        let dir = std::env::temp_dir().join(format!("hotshot-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut studio = Studio::new(Doc::new(120, 80), Text::load(), 1400, 900, dir.clone(), "shot.png".into());
        studio.path = Some(dir.join("File.hotshot"));
        studio.new_frame((300.0, 0.0), (60.0, 40.0));
        studio.press_control(Control::Tab(1), false);
        let controls: Vec<Control> = studio.layout().controls.iter().map(|(_, control)| *control).collect();
        assert!(controls.contains(&Control::ExportPick(0)) && controls.contains(&Control::ExportPick(1)) && controls.contains(&Control::Action(Action::ExportChosen)));
        // The first left out: only the second is written, and shown.
        studio.press_control(Control::ExportPick(0), false);
        assert_eq!(studio.export_chosen(), vec![1]);
        studio.act(Action::ExportChosen);
        let exported = studio.exported.as_ref().expect("what was written is shown");
        assert_eq!(exported.files.len(), studio.frame(1).exports.len().max(1));
        assert!(exported.files.iter().all(|(path, ..)| path.exists()));
        studio.press_control(Control::ExportDone(true), false);
        assert!(studio.exported.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_frame_exports_at_its_size_and_a_larger_one_is_named_at_for_how_large() {
        let dir = std::env::temp_dir().join(format!("hotshot-export-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut studio = Studio::new(Doc::new(120, 80), Text::load(), 1400, 900, dir.clone(), "shot.png".into());
        studio.path = Some(dir.join("File.hotshot"));
        studio.new_frame((300.0, 0.0), (60.0, 40.0));
        assert_eq!(studio.doc.exports, vec![ExportSize::new(1.0)]);
        // Another size added says nothing of its own, and is named for it;
        // one given a suffix keeps it.
        studio.act(Action::AddExport);
        studio.act(Action::AddExport);
        studio.doc.exports[2].suffix = "-big".into();
        let written = studio.export_frames(&[1]).unwrap();
        let names: Vec<String> = written.iter().map(|path| path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["Frame 2.png", "Frame 2@2x.png", "Frame 2-big.png"]);
        assert_eq!(ExportSize::new(1.5).file_suffix(), "@1.5x");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ctrl_k_finds_actions_and_layers_by_what_is_typed() {
        let mut studio = studio();
        studio.add_picture(checker(40, 40), "Shadow catcher");
        studio.selected.clear();
        studio.key(Key::Char('k'), Mods { ctrl: true, ..Mods::default() }, true);
        for letter in "catch".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        let hits = studio.hits();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].found, search::Found::Layer(0));
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!((studio.search.is_none(), studio.selected.clone()), (true, vec![0]));
        // An action, done from it: what is picked out deleted.
        studio.key(Key::Char('k'), Mods { ctrl: true, ..Mods::default() }, true);
        for letter in "delete".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        assert!(matches!(studio.hits()[0].found, search::Found::Act(_)));
        studio.key(Key::Enter, Mods::default(), true);
        assert!(studio.doc.layers.is_empty());
    }

    #[test]
    fn ctrl_k_finds_the_tools_by_name_or_another_word_and_takes_one_up() {
        let mut studio = studio();
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        let find = |studio: &mut Studio, typed: &str| {
            studio.key(Key::Char('k'), ctrl, true);
            for letter in typed.chars() {
                studio.key(Key::Char(letter), Mods::default(), true);
            }
            let first = studio.hits().into_iter().next().expect("something found");
            assert!(matches!(first.found, search::Found::Tool(_)), "{typed} finds {:?} first", first.says);
            (first.says, first.hint)
        };
        for (typed, says, key) in [
            ("rect", "Rectangle", "R"),
            ("box", "Rectangle", "R"),
            ("oval", "Oval", "O"),
            ("arrow", "Arrow", "A"),
            ("line", "Line", "L"),
            ("pen", "Pen", "P"),
            ("marker", "Marker", "M"),
            ("text", "Text", "T"),
            ("counter", "Counter", "N"),
            ("blur", "Blur", "B"),
            ("spotlight", "Spotlight", "D"),
            ("zoom lens", "Zoom lens", "Z"),
            ("library", "Library", "J"),
            ("emoji", "Emoji", "E"),
            ("frame", "Frame", "F"),
            ("crop", "Crop", "Enter"),
        ] {
            assert_eq!(find(&mut studio, typed), (says.to_owned(), key.to_owned()));
            studio.key(Key::Escape, Mods::default(), true);
        }
        // Enter takes it up, a tool of a group shown as its group's.
        find(&mut studio, "oval");
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!((studio.search.is_none(), studio.tool, studio.groups[0]), (true, Tool::Ellipse, Tool::Ellipse));
        find(&mut studio, "frame");
        studio.key(Key::Enter, Mods::default(), true);
        assert!(studio.framing);
        // Crop begins a crop of the picture picked out.
        studio.add_picture(checker(40, 40), "Shot");
        find(&mut studio, "crop");
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!(studio.cropping, Some(0));
        // Nothing typed lists the tools after what was done lately, which
        // here is nothing.
        studio.key(Key::Escape, Mods::default(), true);
        studio.key(Key::Char('k'), ctrl, true);
        assert!(matches!(studio.hits().first().map(|hit| &hit.found), Some(search::Found::Tool(_))));
    }

    #[test]
    fn s_opens_the_styles_and_a_letter_puts_one_on() {
        let mut studio = studio();
        studio.doc.background = Fill::Color(0x111111);
        let mut boxed = Shape::new(Tool::Rect, 0x30A46C, 2.0, (100.0, 100.0));
        boxed.points[1] = (200.0, 150.0);
        studio.doc.layers = vec![Layer::shape(boxed)];
        studio.key(Key::Char('s'), Mods::default(), true);
        assert!(!studio.styling, "nothing picked out, nothing to style");
        studio.selected = vec![0];
        studio.key(Key::Char('s'), Mods::default(), true);
        assert!(studio.styling && studio.layout().controls.iter().any(|(_, control)| *control == Control::Style(style::GLOW)));
        studio.key(Key::Char('g'), Mods::default(), true);
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert!(!studio.styling && shape.filled && shape.shadow.is_some_and(|drop| drop.color & 0xFF_FFFF == 0x30A46C));
        assert_eq!((studio.undo.len(), studio.tool), (1, Tool::Select));
        // On something light, there is no glow.
        studio.doc.background = Fill::Color(0xFFFFFF);
        studio.put_style(style::GLOW);
        assert_eq!(studio.undo.len(), 1);
    }

    #[test]
    fn a_button_hovered_shows_what_it_does_and_only_a_click_keeps_it() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xFF0000, 2.0, (100.0, 100.0));
        boxed.points[1] = (200.0, 150.0);
        let mut other = boxed.clone();
        other.translate(150.0, 200.0);
        studio.doc.layers = vec![Layer::shape(boxed), Layer::shape(other)];
        studio.selected = vec![0, 1];
        let left_of = |studio: &Studio| match &studio.doc.layers[1].kind {
            Kind::Shape(shape) => shape.points[0].0,
            Kind::Picture(_) => 0.0,
        };
        let button = studio.control_rect(Control::Action(Action::Align(0))).unwrap().center();
        let away = (button.0 - 300, button.1 + 300);
        studio.pointer_moved(button.0, button.1, Mods::default());
        assert!(studio.preview.is_some());
        assert_eq!((left_of(&studio), studio.undo.len()), (100.0, 0));
        studio.pointer_moved(away.0, away.1, Mods::default());
        assert_eq!((left_of(&studio), studio.preview.is_none()), (250.0, true));
        // Clicked, it is kept, as one step to undo.
        studio.pointer_moved(button.0, button.1, Mods::default());
        studio.pressed(1, 1000);
        studio.released();
        assert_eq!((left_of(&studio), studio.undo.len()), (100.0, 1));
        studio.pointer_moved(away.0, away.1, Mods::default());
        assert_eq!(left_of(&studio), 100.0);
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert_eq!(left_of(&studio), 250.0);
    }

    #[test]
    fn a_picture_is_drawn_where_it_is_turned_rounded_and_shadowed() {
        let mut text = Text::load();
        let mut doc = Doc::new(200, 200);
        doc.background = Fill::Color(0xFFFFFF);
        let mut picture = Picture::new(Arc::new(checker(100, 100)), "pic".into(), (100.0, 100.0));
        picture.radius = 20.0;
        doc.layers.push(Layer::picture(picture.clone()));
        let (_, _, out) = doc.render(1.0, &mut text);
        // Its middle is the picture; its corner, rounded, is the ground.
        assert!(out[100 * 200 + 100] & 0xFF_FFFF != 0xFFFFFF);
        assert_eq!(out[51 * 200 + 51] & 0xFF_FFFF, 0xFFFFFF);
        // Turned a quarter, a tall picture lies along.
        picture = Picture::new(Arc::new(checker(40, 120)), "tall".into(), (100.0, 100.0));
        picture.angle = std::f32::consts::FRAC_PI_2;
        doc.layers = vec![Layer::picture(picture.clone())];
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(out[100 * 200 + 155] & 0xFF_FFFF != 0xFFFFFF && out[155 * 200 + 100] & 0xFF_FFFF == 0xFFFFFF);
        // With a shadow, it is darker just under it than far from it.
        picture.angle = 0.0;
        picture.shadow = 4;
        doc.layers = vec![Layer::picture(picture)];
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(out[(100 + 64) * 200 + 100] & 0xFF < 0xF0, "{:06x}", out[(100 + 64) * 200 + 100]);
        assert_eq!(out[5 * 200 + 5] & 0xFF_FFFF, 0xFFFFFF);
    }

    #[test]
    fn nothing_behind_is_see_through_and_export_is_larger() {
        let mut text = Text::load();
        let mut doc = Doc::new(100, 50);
        doc.background = Fill::None;
        let mut boxed = Shape::new(Tool::Rect, 0xFF0000, 4.0, (10.0, 10.0));
        boxed.points[1] = (60.0, 40.0);
        doc.layers.push(Layer::shape(boxed));
        let (wide, high, out) = doc.render(2.0, &mut text);
        assert_eq!((wide, high), (200, 100));
        assert_eq!(out[0] >> 24, 0);
        assert_eq!(out[20 * 200 + 60], 0xFFFF0000);
    }

    /// A canvas zoomed out to half, its frame at the left of the view and
    /// the page bare to its right.
    fn with_room_beside() -> Studio {
        let mut studio = studio();
        (studio.zoom, studio.pan) = (0.5, (300.0, 200.0));
        studio
    }

    /// Where a box's or an oval's frame begins, in what it is in.
    fn corner_of(layer: &Layer) -> (f32, f32) {
        let Kind::Shape(shape) = &layer.kind else { panic!("a shape") };
        let (x, y, ..) = shape.frame();
        (x, y)
    }

    /// A drag from a point of the page to another.
    fn drag_page(studio: &mut Studio, from: (f32, f32), to: (f32, f32)) {
        let (a, b) = (studio.to_window_page(from), studio.to_window_page(to));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((a.0 + b.0) / 2, (a.1 + b.1) / 2, Mods::default());
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
    }

    /// What the window shows at a point of the page.
    fn shown_at(studio: &mut Studio, at: (f32, f32)) -> u32 {
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let (x, y) = studio.to_window_page(at);
        out[(y * studio.width + x) as usize] & 0xFF_FFFF
    }

    #[test]
    fn a_box_drawn_outside_the_frames_is_the_pages_own_drawn_picked_out_and_styled() {
        let mut studio = with_room_beside();
        studio.set_tool(Tool::Rect);
        drag_page(&mut studio, (900.0, 100.0), (1100.0, 250.0));
        // On the page, not in the frame, where it was drawn, and picked out.
        assert!(studio.on_page());
        assert_eq!((studio.doc.layers.len(), studio.pages[0].frames[0].layers.len()), (1, 0));
        assert!((corner_of(&studio.doc.layers[0]).0 - 900.0).abs() <= 2.0);
        assert_eq!(studio.selected, vec![0]);
        // Drawn on the page, where nothing but the page was.
        let page = shown_at(&mut studio, (1000.0, 300.0));
        assert_ne!(shown_at(&mut studio, (1000.0, 180.0)), page);
        // Styled from the panel as a frame's box is.
        studio.paint(Paint::Fill, Some(0x00AA00));
        assert_eq!(shown_at(&mut studio, (1000.0, 180.0)), 0x00AA00);
        // Picked out again by a click from in the frame, and moved.
        let at = studio.to_window_page((400.0, 300.0));
        click(&mut studio, at);
        assert!(!studio.on_page() && studio.selected.is_empty());
        let at = studio.to_window_page((1000.0, 180.0));
        studio.pointer_moved(at.0, at.1, Mods::default());
        assert_eq!((studio.hovered_page, studio.hovered), (Some(0), None), "ringed as the pointer comes over it");
        click(&mut studio, at);
        assert!(studio.on_page() && studio.selected == vec![0]);
        let before = corner_of(&studio.doc.layers[0]);
        drag_page(&mut studio, (1000.0, 180.0), (1050.0, 400.0));
        let after = corner_of(&studio.doc.layers[0]);
        assert!(((after.0 - before.0) - 50.0).abs() <= 2.0 && ((after.1 - before.1) - 220.0).abs() <= 2.0, "moved from {before:?} to {after:?}");
        assert!(studio.on_page(), "still the page's, off every frame");
        // Undone, back where it was; undone again, never drawn.
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        let layer = studio.loose().layers[0].clone();
        assert_eq!(corner_of(&layer), before);
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert!(studio.loose().layers.is_empty());
        // Exported, the frame has nothing of it.
        studio.key(Key::Char('y'), Mods { ctrl: true, ..Mods::default() }, true);
        assert_eq!(studio.loose().layers.len(), 1);
        let frame = studio.pages[0].frames.first().cloned().unwrap_or_else(|| studio.doc.clone());
        let (_, _, pixels) = frame.render(1.0, &mut studio.text);
        assert!(pixels.iter().all(|&pixel| pixel == 0xFFFF_FFFF));
    }

    #[test]
    fn a_layer_dragged_into_a_frame_goes_into_it_and_out_again_onto_the_page() {
        let mut studio = with_room_beside();
        studio.set_tool(Tool::Ellipse);
        drag_page(&mut studio, (900.0, 100.0), (1000.0, 200.0));
        assert!(studio.on_page());
        // Into the frame: one of its layers, where it was let go on the page.
        drag_page(&mut studio, (950.0, 150.0), (350.0, 250.0));
        assert!(!studio.on_page());
        assert_eq!((studio.doc.layers.len(), studio.loose().layers.len(), studio.selected.clone()), (1, 0, vec![0]));
        let (x, y) = corner_of(&studio.doc.layers[0]);
        assert!((x + studio.doc.x - 300.0).abs() <= 6.0 && (y + studio.doc.y - 200.0).abs() <= 6.0, "where it was let go on the page");
        // Clipped to the frame now, and in its export.
        let (_, _, pixels) = studio.doc.render(1.0, &mut studio.text);
        assert!(pixels.iter().any(|&pixel| pixel != 0xFFFF_FFFF));
        // Out again, onto the page; one step to undo each way.
        drag_page(&mut studio, (350.0, 250.0), (1200.0, 450.0));
        assert!(studio.on_page());
        assert_eq!((studio.loose().layers.len(), studio.pages[0].frames[0].layers.len()), (1, 0));
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert_eq!((studio.loose().layers.len(), studio.frame(0).layers.len()), (0, 1));
    }

    #[test]
    fn the_pages_own_layers_are_listed_above_its_frames_and_kept_in_its_file() {
        let mut studio = with_room_beside();
        studio.set_tool(Tool::Rect);
        drag_page(&mut studio, (900.0, 100.0), (1100.0, 250.0));
        studio.add_picture(checker(40, 40), "In the frame");
        // The picture went where the view's middle is: the frame.
        studio.activate(0);
        studio.add_picture(checker(40, 40), "Shot");
        assert_eq!(studio.doc.layers.iter().map(Layer::name).collect::<Vec<_>>().last().map(String::as_str), Some("Shot"));
        // With the frame worked in, the page's box is a row of its own over
        // the frame's, and clicked, works in the page with it picked out.
        let controls: Vec<Control> = studio.layout().controls.iter().map(|(_, control)| *control).filter(|control| matches!(control, Control::PageLayer(_) | Control::Frame(_) | Control::Layer(_))).collect();
        let row = controls.iter().position(|control| *control == Control::PageLayer(0)).expect("listed");
        assert!(row < controls.iter().position(|control| matches!(control, Control::Frame(_))).unwrap());
        let at = studio.control_rect(Control::PageLayer(0)).unwrap().center();
        click(&mut studio, at);
        assert!(studio.on_page() && studio.selected == vec![0]);
        // Kept in the file and read back; a file from before has none.
        let bytes = write_pages(&studio.gathered());
        let back = read_pages(&bytes).unwrap();
        assert_eq!((back[0].loose.layers.len(), back[0].frames[0].layers.len()), (studio.loose().layers.len(), studio.frame(0).layers.len()));
        let old: Vec<u8> = String::from_utf8_lossy(&bytes).split("page-layers\n").next().unwrap().bytes().collect();
        let back = read_pages(&old).unwrap();
        assert!(back[0].loose.layers.is_empty());
        // Opened with the page's own layers worked in, it still draws.
        let mut again = Studio::with_pages(read_pages(&bytes).unwrap(), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        again.activate(LOOSE);
        let mut out = vec![0u32; 1400 * 900];
        again.draw(&mut out);
    }

    #[test]
    fn frames_and_the_pages_own_layers_go_over_and_under_each_other_in_one_order() {
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        let front = Mods { ctrl: true, shift: true, alt: false };
        let mut studio = with_room_beside();
        studio.set_tool(Tool::Rect);
        drag_page(&mut studio, (900.0, 100.0), (1100.0, 250.0));
        studio.paint(Paint::Fill, Some(0x00AA00));
        // Half over the frame, which ends at 800.
        if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
            shape.points = vec![(700.0, 100.0), (900.0, 250.0)];
            shape.touch();
        }
        studio.changed();
        assert_eq!(studio.page_order(), vec![Child::Frame(0), Child::Layer(0)]);
        assert_eq!(shown_at(&mut studio, (750.0, 150.0)), 0x00AA00, "over the frame, as files had it");
        // The frame brought to the front: over the box, drawn and hit so.
        studio.press_control(Control::Frame(0), false);
        studio.key(Key::Char('}'), front, true);
        assert_eq!(studio.page_order(), vec![Child::Layer(0), Child::Frame(0)]);
        assert_eq!(shown_at(&mut studio, (750.0, 150.0)), 0xFFFFFF);
        assert_eq!(shown_at(&mut studio, (850.0, 150.0)), 0x00AA00);
        let at = studio.to_window_page((750.0, 150.0));
        click(&mut studio, at);
        assert!(!studio.on_page(), "the frame, over it, is what is clicked");
        let at = studio.to_window_page((850.0, 150.0));
        studio.pointer_moved(at.0, at.1, Mods::default());
        assert_eq!(studio.hovered_page, Some(0));
        click(&mut studio, at);
        assert!(studio.on_page() && studio.selected == vec![0]);
        let at = studio.to_window_page((750.0, 150.0));
        studio.pointer_moved(at.0, at.1, Mods::default());
        assert_eq!(studio.hovered, None, "under the frame, not ringed");
        // Listed in that order, the topmost first.
        let rows: Vec<Control> = studio.layout().controls.iter().filter(|(rect, _)| rect.x < LEFT_W).map(|(_, control)| *control).filter(|control| matches!(control, Control::Frame(_) | Control::Layer(_))).collect();
        assert_eq!(rows, vec![Control::Frame(0), Control::Layer(0)]);
        // The box brought forward, a step past the frame; undone, under it.
        studio.key(Key::Char(']'), ctrl, true);
        assert_eq!(studio.page_order(), vec![Child::Frame(0), Child::Layer(0)]);
        studio.key(Key::Char('z'), ctrl, true);
        assert_eq!(studio.page_order(), vec![Child::Layer(0), Child::Frame(0)]);
        // A box new on the page goes on top of everything; the frame stays
        // over the first.
        studio.set_tool(Tool::Rect);
        drag_page(&mut studio, (900.0, 400.0), (1000.0, 500.0));
        assert_eq!(studio.page_order(), vec![Child::Layer(0), Child::Frame(0), Child::Layer(1)]);
        // Kept in the file; a file from before has the frames under them all.
        let bytes = write_pages(&studio.gathered());
        let mut again = Studio::with_pages(read_pages(&bytes).unwrap(), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        assert_eq!(again.page_order(), vec![Child::Layer(0), Child::Frame(0), Child::Layer(1)]);
        (again.zoom, again.pan) = (studio.zoom, studio.pan);
        assert_eq!(shown_at(&mut again, (750.0, 150.0)), 0xFFFFFF);
        let old = String::from_utf8_lossy(&bytes).lines().filter(|line| !line.starts_with("stacking")).collect::<Vec<_>>().join("\n");
        let old_pages = read_pages(old.as_bytes()).unwrap();
        assert_eq!(old_pages[0].frames[0].below, 0);
        // A new frame goes over everything, a copy just over its frame.
        studio.run_command(menu::Command::NewFrame);
        assert_eq!(studio.page_order().last(), Some(&Child::Frame(1)));
        studio.activate(0);
        studio.run_command(menu::Command::DuplicateFrame);
        assert_eq!(studio.page_order(), vec![Child::Layer(0), Child::Frame(0), Child::Frame(2), Child::Layer(1), Child::Frame(1)]);
    }

    #[test]
    fn a_spotlight_of_the_pages_own_darkens_everything_under_it_on_the_page_and_no_export() {
        let mut studio = with_room_beside();
        studio.set_tool(Tool::Spotlight);
        drag_page(&mut studio, (900.0, 100.0), (1000.0, 300.0));
        assert!(studio.on_page());
        if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
            shape.points = vec![(100.0, 100.0), (300.0, 300.0)];
            shape.touch();
        }
        studio.changed();
        // The frame under it darkened but where it is on, and the page
        // round the frame too.
        assert_eq!(shown_at(&mut studio, (200.0, 200.0)), 0xFFFFFF);
        let dimmed = shown_at(&mut studio, (500.0, 400.0));
        assert!(dimmed & 0xFF < 0x80, "darkened: {dimmed:06X}");
        assert!(shown_at(&mut studio, (1100.0, 700.0)) & 0xFF < VIEW_COLOR & 0xFF);
        // A box of the page's own over it is not darkened.
        studio.set_tool(Tool::Rect);
        drag_page(&mut studio, (1200.0, 100.0), (1300.0, 200.0));
        studio.paint(Paint::Fill, Some(0x00AA00));
        assert_eq!(shown_at(&mut studio, (1250.0, 150.0)), 0x00AA00);
        // The frame put over it is not darkened either.
        studio.press_control(Control::Frame(0), false);
        studio.key(Key::Char('}'), Mods { ctrl: true, shift: true, alt: false }, true);
        assert_eq!(shown_at(&mut studio, (500.0, 400.0)), 0xFFFFFF);
        // Exported, the frame is as it is.
        let (_, _, pixels) = studio.doc.render(1.0, &mut studio.text);
        assert!(pixels.iter().all(|&pixel| pixel == 0xFFFF_FFFF));
    }

    #[test]
    fn a_frame_dragged_in_the_list_goes_over_or_under_the_pages_own_layers() {
        let mut studio = with_room_beside();
        studio.set_tool(Tool::Rect);
        drag_page(&mut studio, (900.0, 100.0), (1100.0, 250.0));
        studio.activate(0);
        assert_eq!(studio.page_order(), vec![Child::Frame(0), Child::Layer(0)]);
        // The frame's row dragged by the top of the box's: over it.
        let from = studio.control_rect(Control::Frame(0)).filter(|rect| rect.x < LEFT_W).unwrap();
        let to = studio.control_rect(Control::PageLayer(0)).unwrap();
        studio.pointer_moved(from.center().0, from.center().1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(to.center().0, to.y + 2, Mods::default());
        assert_eq!(studio.drop_mark(to, Control::PageLayer(0)), Some(0));
        studio.released();
        assert_eq!(studio.page_order(), vec![Child::Layer(0), Child::Frame(0)]);
        // The box's row dragged by the top of the frame's: over it again.
        let from = studio.control_rect(Control::PageLayer(0)).unwrap();
        studio.pointer_moved(from.center().0, from.center().1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.control_rect(Control::Frame(0)).filter(|rect| rect.x < LEFT_W).unwrap();
        studio.pointer_moved(to.center().0, to.y + 2, Mods::default());
        studio.released();
        assert!(studio.on_page());
        assert_eq!(studio.page_order(), vec![Child::Frame(0), Child::Layer(0)]);
        assert_eq!(studio.loose().layers.len(), 1, "by the frame's edge, not into it");
        // Into it by the middle of its row.
        let from = studio.control_rect(Control::Layer(0)).filter(|rect| rect.x < LEFT_W).unwrap();
        studio.pointer_moved(from.center().0, from.center().1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.control_rect(Control::Frame(0)).filter(|rect| rect.x < LEFT_W).unwrap();
        studio.pointer_moved(to.center().0, to.center().1, Mods::default());
        studio.released();
        assert_eq!((studio.loose().layers.len(), studio.frame(0).layers.len()), (0, 1));
    }

    #[test]
    fn a_picture_pasted_with_the_page_in_view_goes_on_the_page_at_its_size() {
        let mut studio = with_room_beside();
        // The view's middle off the frame, on the bare page.
        studio.pan = (-900.0, 200.0);
        studio.add_picture(checker(1000, 800), "Big");
        assert!(studio.on_page());
        let Kind::Picture(picture) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((picture.w, picture.h), (1000.0, 800.0));
        // Exporting and copying are of frames alone.
        studio.act(Action::Export);
        assert!(studio.exported.is_none());
        // One dropped on the frame goes in it, where it was let go, and one
        // dropped beside it on the page.
        studio.pan = (300.0, 200.0);
        let on_frame = studio.to_window_page((200.0, 150.0));
        studio.drop_picture(checker(40, 40), "Dropped", Some(on_frame));
        assert_eq!(studio.active, 0);
        let Kind::Picture(picture) = &studio.doc.layers.last().unwrap().kind else { panic!() };
        assert!((picture.cx - 200.0).abs() <= 2.0 && (picture.cy - 150.0).abs() <= 2.0);
        let beside = studio.to_window_page((1000.0, 150.0));
        studio.drop_picture(checker(40, 40), "Beside", Some(beside));
        assert!(studio.on_page() && studio.doc.layers.len() == 2);
    }

    #[test]
    fn a_canvas_is_written_down_and_read_back_whole() {
        let mut doc = Doc::new(640, 360);
        doc.background = Fill::Backdrop(frame::named("dusk").unwrap());
        let image = Arc::new(checker(30, 20));
        let mut picture = Picture::new(image.clone(), "two words".into(), (100.0, 80.0));
        (picture.angle, picture.radius, picture.shadow, picture.crop, picture.flip) = (0.5, 6.0, 3, (2.0, 3.0, 20.0, 10.0), (true, false));
        doc.layers.push(Layer::picture(picture.clone()));
        doc.layers.push(Layer::picture(Picture::new(image, "again".into(), (10.0, 10.0))));
        let mut arrow = Shape::new(Tool::Arrow, 0x30A46C, 3.5, (1.0, 2.0));
        arrow.points[1] = (50.0, 60.0);
        doc.layers.push(Layer { kind: Kind::Shape(arrow), hidden: true, locked: true, named: None });
        let back = Doc::read(&doc.write()).expect("read back");
        assert_eq!((back.width, back.height, back.background, back.layers.len()), (640, 360, doc.background, 3));
        let Kind::Picture(first) = &back.layers[0].kind else { panic!("a picture first") };
        assert_eq!((first.name.as_str(), first.angle, first.shadow, first.crop, first.flip, first.image.width()), ("two words", 0.5, 3, (2.0, 3.0, 20.0, 10.0), (true, false), 30));
        let Kind::Picture(second) = &back.layers[1].kind else { panic!("a picture second") };
        assert!(Arc::ptr_eq(&first.image, &second.image), "a picture on it twice is kept once");
        assert!(back.layers[2].hidden && back.layers[2].locked && matches!(&back.layers[2].kind, Kind::Shape(shape) if shape.tool == Tool::Arrow));
        assert!(Doc::read(b"not a canvas").is_none());
    }

    #[test]
    fn a_picture_is_picked_out_moved_sized_turned_and_cropped() {
        let mut studio = studio();
        studio.add_picture(checker(200, 100), "pic");
        assert_eq!(studio.selected, [0]);
        let middle = studio.to_window((400.0, 300.0));
        // Moved by a drag.
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.pressed(1, 1000);
        studio.pointer_moved(middle.0 + 50, middle.1 + 20, Mods::default());
        studio.released();
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert!((picture.cx - 400.0 - 50.0 / studio.zoom).abs() < 2.0, "{}", picture.cx);
        // Its width typed into its field keeps its height as it was; its
        // corner dragged keeps its shape.
        studio.set_field(Field::W, 300.0);
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert_eq!((picture.w, picture.h), (300.0, 100.0));
        let corner = studio.handles()[2].0;
        studio.pointer_moved(corner.0, corner.1, Mods::default());
        studio.pressed(1, 5000);
        studio.pointer_moved(corner.0 + 30, corner.1, Mods::default());
        studio.released();
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert!((picture.w / picture.h - 3.0).abs() < 0.01, "{} by {}", picture.w, picture.h);
        // Cropped from its right, it is narrower and shows less of itself.
        studio.cropping = Some(0);
        let right = studio.to_window(picture.placed((picture.w / 2.0, 0.0)));
        studio.pointer_moved(right.0, right.1, Mods::default());
        studio.pressed(1, 9000);
        studio.pointer_moved(right.0 - 40, right.1, Mods::default());
        studio.released();
        let Kind::Picture(cropped) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert!(cropped.w < picture.w && cropped.crop.2 < 200.0 && cropped.crop.0 == 0.0, "{:?}", cropped.crop);
        // And undo takes each step back.
        for _ in 0..3 {
            studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        }
        let Kind::Picture(back) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert_eq!((back.w, back.h), (200.0, 100.0));
    }

    #[test]
    fn shapes_are_drawn_with_the_tools_and_the_panel_sets_them() {
        let mut studio = studio();
        studio.key(Key::Char('r'), Mods::default(), true);
        let (a, b) = (studio.to_window((100.0, 100.0)), studio.to_window((300.0, 200.0)));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.pressed(1, 1000);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
        assert_eq!((studio.doc.layers.len(), studio.selected.clone(), studio.tool), (1, vec![0], Tool::Select));
        // Filled, with no border, as Figma draws one, in a soft indigo.
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((shape.filled, shape.fill, shape.border), (true, fills::DEFAULT_FILL, false));
        assert!(studio.control_rect(Control::Action(Action::BorderOn)).is_some());
        // Its fill: the colors its fill opens.
        let field = studio.layout().controls.iter().find(|(_, control)| *control == Control::ColorField(Paint::Fill)).map(|(rect, _)| rect.center()).expect("its fill is in the panel");
        studio.pointer_moved(field.0, field.1, Mods::default());
        studio.pressed(1, 2000);
        studio.released();
        let swatch = studio.layout().controls.iter().find(|(_, control)| *control == Control::Swatch(Paint::Fill, Some(0xFFE629))).map(|(rect, _)| rect.center()).expect("the fill swatches are up");
        studio.pointer_moved(swatch.0, swatch.1, Mods::default());
        studio.pressed(1, 3000);
        studio.released();
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((shape.filled, shape.fill), (true, 0xFFE629));
        // With nothing picked out, the panel is the artboard's: the first
        // Esc closes the colors, the second lets go of the box.
        studio.key(Key::Escape, Mods::default(), true);
        studio.key(Key::Escape, Mods::default(), true);
        // Its size from the list of them, as Figma has it.
        assert!(studio.layout().controls.iter().any(|(_, control)| *control == Control::Menu(MenuKind::Sizes)));
        studio.press_control(Control::Menu(MenuKind::Sizes), false);
        assert!(studio.menu.is_some());
        // A section of them opens beside it: docs and slides, 16:9.
        studio.press_menu(0, 2);
        studio.press_menu(1, 1);
        assert_eq!((studio.doc.width, studio.doc.height, studio.menu.is_none()), (1920, 1080, true));
        studio.press_control(Control::Ground(Ground::Backdrop), false);
        assert!(matches!(studio.doc.background, Fill::Backdrop(_)));
    }

    #[test]
    fn a_picture_from_the_editor_keeps_its_shapes_where_they_were_on_it() {
        let area = Rect::new(300, 200, 400, 250);
        let mut arrow = Shape::new(Tool::Arrow, 0xE5484D, 3.5, (310.0, 220.0));
        arrow.points[1] = (500.0, 300.0);
        let doc = from_markup(area, &vec![0x123456; 400 * 250], &arrow.write(), &chrome::Notes::default());
        // In a frame just its size, with nothing behind it.
        assert_eq!((doc.width, doc.height, doc.layers.len(), doc.background), (400, 250, 2, Fill::None));
        let Kind::Shape(moved) = &doc.layers[1].kind else { panic!("the arrow over the picture") };
        assert_eq!(moved.points[..2], [(10.0, 20.0), (200.0, 100.0)]);
        let (_, _, out) = doc.render(1.0, &mut Text::load());
        assert_eq!(out[220 * 400 + 320], 0xFF123456);
    }

    #[test]
    fn layers_line_up_go_over_each_other_and_draw_into_the_window() {
        let mut studio = studio();
        studio.add_picture(checker(100, 100), "one");
        studio.add_picture(checker(60, 60), "two");
        studio.doc.layers[1].translate(150.0, 80.0);
        studio.selected = vec![0, 1];
        studio.align(0);
        let lefts: Vec<f32> = (0..2).map(|index| studio.doc.layers[index].bounds(&mut studio.text).0).collect();
        assert!((lefts[0] - lefts[1]).abs() < 1.0, "{lefts:?}");
        studio.selected = vec![0];
        studio.restack(true);
        assert_eq!((studio.doc.layers[1].name(), studio.selected.clone()), ("one".to_owned(), vec![1]));
        let mut out = vec![0u32; 1400 * 900];
        studio.draw(&mut out);
        assert!(out.iter().any(|&pixel| pixel == 0xFF0000) && out[5] == VIEW_COLOR && out[((INSET + 80) * 1400 + INSET + 200) as usize] == PANEL_COLOR);
    }

    /// A drag in the view from one point of the artboard to another.
    fn drag(studio: &mut Studio, from: (f32, f32), to: (f32, f32), mods: Mods) {
        let (a, b) = (studio.to_window(from), studio.to_window(to));
        studio.pointer_moved(a.0, a.1, mods);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((a.0 + b.0) / 2, (a.1 + b.1) / 2, mods);
        studio.pointer_moved(b.0, b.1, mods);
        studio.released();
    }

    #[test]
    fn what_is_moved_snaps_to_the_artboard_and_to_the_rest() {
        let mut studio = studio();
        studio.add_picture(checker(100, 100), "pic");
        studio.set_field(Field::X, 10.0);
        studio.set_field(Field::Y, 200.0);
        // Dragged to within a few pixels of the left of the artboard, it
        // goes the rest of the way; with Ctrl held, it does not.
        let step = 3.0 / studio.zoom;
        drag(&mut studio, (60.0, 250.0), (60.0 - 10.0 + step, 250.0), Mods::default());
        assert_eq!(studio.field(Field::X), Some(0.0));
        drag(&mut studio, (50.0, 250.0), (50.0 + 2.0 * step, 250.0), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.field(Field::X).unwrap() > 0.5);
        assert!(studio.guides.is_empty(), "the guides go when it is let go of");
    }

    /// A box on what is worked in, from one corner to the other.
    fn boxed(studio: &mut Studio, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) -> usize {
        let mut shape = Shape::new(Tool::Rect, 0xE5484D, 3.0, (x0, y0));
        shape.points[1] = (x1, y1);
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        studio.doc.layers.len() - 1
    }

    #[test]
    fn a_layer_in_a_frame_snaps_to_the_pages_own_and_they_to_the_layers_in_frames() {
        let mut studio = studio();
        // A box of the page's own, beside the frame.
        studio.activate(LOOSE);
        boxed(&mut studio, (900.0, 150.0), (1000.0, 250.0));
        // In the frame: spaced evenly between one of the frame's and the
        // page's box, its top on theirs.
        studio.activate(0);
        boxed(&mut studio, (100.0, 150.0), (200.0, 250.0));
        let moved = boxed(&mut studio, (497.0, 153.0), (597.0, 253.0));
        studio.selected = vec![moved];
        studio.held = vec![studio.doc.layers[moved].clone()];
        assert_eq!(studio.snap(0.0, 0.0), (3.0, -3.0), "an even gap with the page's box");
        let widths: Vec<f32> = studio.gaps.iter().map(|gap| gap.to - gap.from).collect();
        assert!(widths.len() == 2 && (widths[0] - widths[1]).abs() < 0.5 && studio.gaps[1].to > 890.0, "{:?}", studio.gaps);
        // On the page, its box goes to the bottom of a frame's layer.
        studio.activate(LOOSE);
        studio.selected = vec![0];
        studio.held = vec![studio.doc.layers[0].clone()];
        let (_, dy) = studio.snap(0.0, 98.0);
        assert_eq!(dy, 100.0, "its top to the bottom of the frame's layer at 250");
        // And a frame moved goes to the page's box's edge.
        studio.activate(0);
        let left = studio.loose().layers[0].clone().bounds(&mut studio.text).0;
        let at = studio.snap_frame((left - 800.0 + 2.0, 3.0), false);
        assert_eq!(at, ((left - 800.0).round(), 3.0), "its right to the box's left");
    }

    #[test]
    fn a_turned_box_is_sized_across_itself() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (300.0, 200.0));
        boxed.points[1] = (400.0, 240.0);
        boxed.angle = std::f32::consts::FRAC_PI_2;
        studio.doc.layers.push(Layer::shape(boxed));
        studio.selected = vec![0];
        // Its right handle, turned a quarter, is below its middle; pulled
        // further down, the box is longer and as tall as it was.
        let (spot, pull) = studio.handles()[5];
        assert_eq!(pull, (1, 0));
        let at = studio.to_doc(spot);
        assert!((at.0 - 350.0).abs() < 2.0 && (at.1 - 270.0).abs() < 2.0, "{at:?}");
        drag(&mut studio, at, (at.0, at.1 + 40.0), Mods::default());
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        let (_, _, w, h) = shape.frame();
        assert!((w - 140.0).abs() < 2.0 && (h - 40.0).abs() < 0.5, "{w} by {h}");
    }

    #[test]
    fn an_arrow_drawn_onto_a_picture_stays_stuck_to_it() {
        let mut studio = studio();
        studio.add_picture(checker(100, 100), "pic");
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        studio.key(Key::Char('a'), Mods::default(), true);
        drag(&mut studio, (100.0, 100.0), (picture.cx, picture.cy), Mods::default());
        let Kind::Shape(arrow) = studio.doc.layers[1].kind.clone() else { panic!() };
        assert_eq!(arrow.stuck[1].map(|(id, ..)| id), Some(picture.id));
        // The picture moved, the arrow's head goes with it.
        studio.selected = vec![0];
        studio.held = vec![studio.doc.layers[0].clone()];
        studio.checkpoint();
        studio.set_field(Field::X, 500.0);
        let Kind::Shape(after) = studio.doc.layers[1].kind.clone() else { panic!() };
        assert!(after.points[1].0 > arrow.points[1].0 + 100.0, "{:?} then {:?}", arrow.points[1], after.points[1]);
    }

    #[test]
    fn a_box_drawn_on_a_picture_is_moved_from_inside_it_and_a_click_there_picks_the_picture() {
        let mut studio = studio();
        studio.add_picture(checker(400, 300), "pic");
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        // A box with no fill: its border alone.
        studio.fill = None;
        studio.key(Key::Char('r'), Mods::default(), true);
        drag(&mut studio, (picture.cx - 50.0, picture.cy - 30.0), (picture.cx + 50.0, picture.cy + 30.0), Mods::default());
        assert_eq!(studio.selected, vec![1]);
        let Kind::Shape(before) = studio.doc.layers[1].kind.clone() else { panic!() };
        assert!(!before.filled);
        // A drag from the middle of the box, where only the picture is.
        drag(&mut studio, (picture.cx, picture.cy), (picture.cx + 40.0, picture.cy + 20.0), Mods::default());
        let Kind::Shape(after) = studio.doc.layers[1].kind.clone() else { panic!() };
        let Kind::Picture(still) = studio.doc.layers[0].kind.clone() else { panic!() };
        assert!((after.points[0].0 - before.points[0].0 - 40.0).abs() < 4.0, "{:?} then {:?}", before.points, after.points);
        assert_eq!((still.cx, still.cy), (picture.cx, picture.cy));
        assert_eq!(studio.selected, vec![1]);
        // A click there, let go without a move, picks out the picture.
        let middle = studio.to_window((picture.cx + 40.0, picture.cy + 20.0));
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
        assert_eq!(studio.selected, vec![0]);
    }

    #[test]
    fn a_frame_made_for_a_picture_exports_it_pixel_for_pixel() {
        let frame = framed(checker(120, 80), "pic");
        assert_eq!(frame.exports.iter().map(|size| size.times).collect::<Vec<_>>(), vec![1.0]);
        let captured = from_markup(Rect::new(10, 10, 120, 80), &vec![0xFF0000FF; 120 * 80], "", &chrome::Notes::default());
        assert_eq!(captured.exports.iter().map(|size| size.times).collect::<Vec<_>>(), vec![1.0]);
        // What is copied is at the size it is exported at.
        let mut studio = Studio::with_pages(vec![Page::new("Page 1", vec![frame])], Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        let png = studio.png().unwrap();
        assert_eq!(Image::open(&png).map(|image| (image.width(), image.height())), Some((120, 80)));
    }

    #[test]
    fn the_text_tool_puts_down_one_text_and_gives_back_the_pointer() {
        let mut studio = studio();
        let click = |studio: &mut Studio, at: (f32, f32)| {
            let at = studio.to_window(at);
            studio.pointer_moved(at.0, at.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            studio.released();
        };
        studio.key(Key::Char('t'), Mods::default(), true);
        click(&mut studio, (100.0, 100.0));
        for letter in "Hi".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        studio.key(Key::Escape, Mods::default(), true);
        assert_eq!((studio.tool, studio.editing, studio.doc.layers.len()), (Tool::Select, None, 1));
        // A click elsewhere then puts down no more text.
        click(&mut studio, (500.0, 400.0));
        assert_eq!(studio.doc.layers.len(), 1);
        // Nor does a click off text being typed, which ends it.
        studio.key(Key::Char('t'), Mods::default(), true);
        click(&mut studio, (300.0, 300.0));
        studio.key(Key::Char('Y'), Mods::default(), true);
        click(&mut studio, (600.0, 500.0));
        assert_eq!((studio.tool, studio.editing, studio.doc.layers.len()), (Tool::Select, None, 2));
    }

    #[test]
    fn a_name_renamed_is_typed_over_not_added_to() {
        let mut studio = studio();
        studio.press_control(Control::Page(0), true);
        // Shift, held for the capital, comes first as a key of its own.
        studio.key(Key::Other, Mods { shift: true, ..Mods::default() }, true);
        for letter in "Home".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!(studio.pages[0].name, "Home");
        // Backspace first clears it all; then letters are added as ever.
        studio.press_control(Control::Frame(0), true);
        studio.key(Key::Backspace, Mods::default(), true);
        for letter in "Hero".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        studio.key(Key::Backspace, Mods::default(), true);
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!(studio.doc.name, "Her");
    }

    #[test]
    fn an_arrow_drawn_on_a_picture_is_kept_as_drawn() {
        let mut studio = studio();
        studio.add_picture(checker(400, 300), "pic");
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        // From near its middle to a spot further in: both ends on it.
        let (from, to) = ((picture.cx - 10.0, picture.cy - 10.0), (picture.cx + 80.0, picture.cy + 60.0));
        studio.key(Key::Char('a'), Mods::default(), true);
        drag(&mut studio, from, to, Mods::default());
        let Kind::Shape(arrow) = studio.doc.layers[1].kind.clone() else { panic!() };
        let length = |shape: &Shape| (shape.points[1].0 - shape.points[0].0).hypot(shape.points[1].1 - shape.points[0].1);
        assert!((length(&arrow) - 90f32.hypot(70.0)).abs() < 2.0, "{:?}", arrow.points);
        // And it goes with the picture, as drawn.
        studio.selected = vec![0];
        studio.held = vec![studio.doc.layers[0].clone()];
        studio.checkpoint();
        studio.set_field(Field::X, 300.0);
        let Kind::Shape(after) = studio.doc.layers[1].kind.clone() else { panic!() };
        assert!((length(&after) - length(&arrow)).abs() < 2.0, "{:?} then {:?}", arrow.points, after.points);
        assert!((after.points[0].0 - arrow.points[0].0 - (after.points[1].0 - arrow.points[1].0)).abs() < 1.0);
    }

    #[test]
    fn pages_and_frames_are_made_worked_in_undone_and_kept() {
        let mut studio = studio();
        studio.add_picture(checker(100, 80), "pic");
        // A frame dragged out beside the first, with the frame tool.
        (studio.zoom, studio.pan) = (0.5, (300.0, 100.0));
        studio.press_control(Control::FrameTool, false);
        let (a, b) = (studio.to_window_page((900.0, 0.0)), studio.to_window_page((1200.0, 400.0)));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.pressed(1, 1000);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
        assert_eq!((studio.frame_count(), studio.active, studio.doc.width, studio.doc.height), (2, 1, 300, 400));
        assert!((studio.doc.x - 900.0).abs() <= 1.0, "{}", studio.doc.x);
        assert!(studio.doc.layers.is_empty() && studio.frame_picked);
        // A click in the first works in it again, with its picture.
        let inside = studio.to_window_page((50.0, 50.0));
        studio.pointer_moved(inside.0, inside.1, Mods::default());
        studio.pressed(1, 5000);
        studio.released();
        assert_eq!((studio.active, studio.doc.layers.len()), (0, 1));
        // A page more, its frame white whatever the one worked in is on;
        // then undone, and done again.
        studio.doc.background = Fill::Color(0x18181B);
        studio.press_control(Control::NewPage, false);
        studio.commit_name();
        assert_eq!((studio.pages.len(), studio.page), (2, 1));
        assert_eq!(studio.doc.background, Fill::Color(0xFFFFFF));
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert_eq!((studio.pages.len(), studio.page, studio.doc.layers.len()), (1, 0, 1));
        studio.key(Key::Char('z'), Mods { ctrl: true, shift: true, ..Mods::default() }, true);
        assert_eq!(studio.pages.len(), 2);
        // Kept, and read back: every page, every frame, where it was.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].frames.iter().map(|frame| (frame.name.clone(), frame.x, frame.width)).collect::<Vec<_>>(), [("Frame 1".to_owned(), 0.0, 800), ("Frame 2".to_owned(), studio.pages[0].frames[1].x, 300)]);
        assert_eq!(back[0].frames[0].layers.len(), 1);
        // A file from before pages is one page with its one frame.
        let old = studio.doc.write();
        let old = [OLD_MAGIC.as_bytes(), &old[old.iter().position(|&byte| byte == b'\n').unwrap()..]].concat();
        let old: Vec<u8> = String::from_utf8_lossy(&old).lines().filter(|line| !line.starts_with("page ") && !line.starts_with("frame ")).collect::<Vec<_>>().join("\n").into_bytes();
        assert_eq!(read_pages(&old).map(|pages| (pages.len(), pages[0].frames.len())), Some((1, 1)));
    }

    #[test]
    fn a_frame_a_picture_fills_grows_round_its_window_and_one_it_does_not_fill_stays() {
        let mut studio = Studio::new(framed(checker(400, 300), "Shot"), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.0, (50.0, 50.0));
        boxed.points[1] = (100.0, 100.0);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.changed();
        let (y0, steps) = (studio.doc.y, studio.undo.len());
        let on_page = |studio: &Studio, at: usize| corner_of(&studio.doc.layers[at]).1 + studio.doc.y;
        let box_at = on_page(&studio, 1);
        studio.selected = vec![0];
        studio.put_window(chrome::Style::Browser);
        let Kind::Picture(picture) = &studio.doc.layers[0].kind else { panic!("a picture") };
        let (bar, edges) = (picture.bar(), chromes::picture_box(picture));
        assert!(bar > 10.0, "a browser's bar, {bar}");
        // As large as the picture in its window, which begins at its top.
        assert_eq!((studio.doc.width, studio.doc.height), (400, (300.0 + bar).round() as i32));
        assert!(edges.1.abs() < 0.01 && (edges.3 - studio.doc.height as f32).abs() < 0.5, "{edges:?}");
        // What is in it where it was on the page, on the picture's pixels.
        assert!((studio.doc.y - (y0 - bar)).abs() < 0.01);
        assert!((on_page(&studio, 1) - box_at).abs() < 0.01);
        // One step to undo, the window and the frame both.
        assert_eq!(studio.undo.len(), steps + 1);
        studio.undo();
        assert_eq!((studio.doc.width, studio.doc.height, studio.doc.y), (400, 300, y0));
        // Taken off again, the frame shrinks with it.
        studio.selected = vec![0];
        studio.put_window(chrome::Style::Mac);
        studio.put_window(chrome::Style::None);
        assert_eq!((studio.doc.width, studio.doc.height), (400, 300));
        // In a frame it does not fill, nothing moves.
        let mut doc = Doc::new(900, 700);
        doc.layers.push(Layer::picture(Picture::new(Arc::new(checker(400, 300)), "Small".into(), (300.0, 300.0))));
        let mut studio = Studio::new(doc, Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        studio.selected = vec![0];
        studio.put_window(chrome::Style::Browser);
        assert_eq!((studio.doc.width, studio.doc.height, studio.doc.y), (900, 700, 0.0));
    }

    #[test]
    fn a_picture_keeps_its_shadow_numbers_adjustments_and_window() {
        let mut text = Text::load();
        let mut doc = Doc::new(300, 200);
        let mut picture = Picture::new(Arc::new(checker(200, 120)), "pic".into(), (150.0, 100.0));
        (picture.shadow, picture.drop) = (CUSTOM_SHADOW, frame::Layer { x: 4, y: 20, blur: 30, spread: -2, color: 0x80102030 });
        (picture.adjust, picture.chrome, picture.title) = ([0.5, -0.25, 1.0], Some(Chrome::new(chrome::Style::Mac)), "main.rs".into());
        doc.layers.push(Layer::picture(picture));
        let mut tuned = crate::convert::Settings::default();
        tuned.avif.quality = 70.0;
        doc.exports = vec![ExportSize { times: 1.0, suffix: String::new(), format: crate::output::Format::Png, tuned: None }, ExportSize { times: 3.0, suffix: "@3x".into(), format: crate::output::Format::Webp, tuned: None }, ExportSize { times: 2.0, suffix: String::new(), format: crate::output::Format::Avif, tuned: Some(tuned) }];
        let back = Doc::read(&doc.write()).expect("read back");
        let Kind::Picture(read) = &back.layers[0].kind else { panic!() };
        assert_eq!((read.shadow, read.drop, read.adjust, read.chrome.clone(), read.title.as_str()), (CUSTOM_SHADOW, frame::Layer { x: 4, y: 20, blur: 30, spread: -2, color: 0x80102030 }, [0.5, -0.25, 1.0], Some(Chrome::new(chrome::Style::Mac)), "main.rs"));
        assert_eq!(back.exports, doc.exports);
        // The window's bar is over the top of it, and lighter is lighter.
        let (_, _, out) = back.render(1.0, &mut text);
        let top = out[(40 + 4) * 300 + 150];
        assert!(top & 0xFF_FFFF == 0xECECEE || top & 0xFF_FFFF == 0x2B2B30, "{top:08x}");
        assert!(adjust(0xFF808080, [0.5, 0.0, 0.0]) & 0xFF > 0x80 && adjust(0xFFFF0000, [0.0, 0.0, -1.0]) & 0xFF == (adjust(0xFFFF0000, [0.0, 0.0, -1.0]) >> 8) & 0xFF);
    }




    #[test]
    fn the_wheel_zooms_far_in_and_out_over_everything_without_falling_over() {
        let mut studio = studio();
        studio.resize(1776, 1540);
        studio.add_picture(checker(300, 200), "one");
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (500.0, 100.0));
        boxed.points[1] = (780.0, 580.0);
        boxed.shadow = Some(frame::Layer { x: 0, y: 12, blur: 16, spread: 2, color: 0x6000_0000 });
        let mut arrow = Shape::new(Tool::Arrow, 0x3E63DD, 3.5, (20.0, 580.0));
        arrow.points[1] = (790.0, 10.0);
        let mut words = Shape::new(Tool::Text, 0x1A1A1A, 2.0, (700.0, 500.0));
        words.text = "At the edge \u{1F600}".into();
        let mut blur = Shape::new(Tool::Blur, 0x1A1A1A, 2.0, (40.0, 40.0));
        blur.points[1] = (140.0, 120.0);
        let mut lens = Shape::new(Tool::Zoom, 0x3E63DD, 3.5, (200.0, 300.0));
        lens.points[1] = (260.0, 360.0);
        lens.place_zoom();
        let mut spot = Shape::new(Tool::Spotlight, 0x1A1A1A, 2.0, (320.0, 20.0));
        spot.points[1] = (420.0, 90.0);
        let mut pen = Shape::new(Tool::Pen, 0x3E63DD, 6.0, (100.0, 400.0));
        pen.points.extend([(160.0, 430.0), (220.0, 390.0)]);
        let counter = Shape::new(Tool::Counter, 0xE5484D, 3.5, (600.0, 60.0));
        for shape in [boxed, arrow, words, blur, lens, spot, pen, counter] {
            studio.doc.layers.push(Layer::shape(shape));
        }
        studio.selected = vec![0, 1, 2, 3];
        let mut out = vec![0u32; 1776 * 1540];
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        // Far enough in, every way, to reach Figma's furthest, 25600%, and
        // drawn sharp there as well as while it moves.
        for (at, turns, up) in [((1770, 10), 70, true), ((5, 1535), 80, false), ((900, 770), 70, true), ((700, 560), 4, true), ((1775, 1539), 20, false), ((1000, 300), 90, false)] {
            studio.pointer_moved(at.0, at.1, ctrl);
            for turn in 0..turns {
                studio.mods.ctrl = true;
                studio.pressed(if up { 4 } else { 5 }, turn);
                studio.draw(&mut out);
            }
            if at == (900, 770) {
                assert_eq!(studio.zoom, MAX_ZOOM);
            }
            studio.moving = None;
            studio.draw(&mut out);
        }
        assert_eq!(studio.zoom, MIN_ZOOM);
        // The menu's zooms, fit and Shift+1 from far in and far out.
        for zoom in [MAX_ZOOM, MIN_ZOOM] {
            studio.zoom = zoom;
            studio.run_command(menu::Command::Zoom(25600));
            studio.draw(&mut out);
            studio.key(Key::Char('!'), Mods { shift: true, ..Mods::default() }, true);
            studio.draw(&mut out);
            studio.run_command(menu::Command::ZoomIn);
            studio.draw(&mut out);
        }
    }

    #[test]
    fn a_picture_zoomed_far_in_is_drawn_as_hard_pixels_with_a_grid_between_them() {
        let mut studio = studio();
        studio.add_picture(Image::new(4, 4, (0..16).map(|at| if (at % 4 + at / 4) % 2 == 0 { 0xFFFF0000 } else { 0xFF0000FF }).collect()), "tiny");
        let Kind::Picture(picture) = studio.doc.layers[0].kind.clone() else { panic!() };
        let colors: Vec<u32> = picture.image.levels[0].2.clone();
        // Drawn twenty times its size: every pixel one of its own.
        let mut out = vec![0u32; 80 * 80];
        let mut placed = picture.clone();
        (placed.cx, placed.cy) = (2.0, 2.0);
        draw_picture(&mut out, Rect::new(0, 0, 80, 80), &placed, 20.0, &mut studio.text);
        assert!(out.iter().all(|pixel| colors.contains(pixel)), "nothing between its colors");
        // Twice its size, smoothed between them as ever.
        let mut out = vec![0u32; 8 * 8];
        draw_picture(&mut out, Rect::new(0, 0, 8, 8), &placed, 2.0, &mut studio.text);
        assert!(out.iter().any(|pixel| !colors.contains(pixel)));
        // At 1600% and more, a grid between the page's pixels.
        let shot = |studio: &mut Studio, zoom: f32| {
            studio.zoom = zoom;
            studio.pan = (300.0, 300.0);
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            // A row of the frame's white, away from the picture.
            let y = (studio.to_window((10.0, 10.0)).1 + 3) as usize;
            out[y * studio.width as usize + 400..y * studio.width as usize + 900].iter().filter(|pixel| **pixel & 0xFF_FFFF != 0xFFFFFF).count()
        };
        studio.doc.layers.clear();
        studio.changed();
        assert_eq!(shot(&mut studio, 8.0), 0);
        assert!(shot(&mut studio, 20.0) >= 20);
    }



    #[test]
    fn a_crop_keeps_its_shape_with_shift_or_the_shape_chosen() {
        let mut studio = studio();
        studio.doc.layers.push(Layer::picture(Picture::new(Arc::new(checker(400, 200)), "pic".into(), (300.0, 200.0))));
        studio.cropping = Some(0);
        let crop = |studio: &Studio| match &studio.doc.layers[0].kind {
            Kind::Picture(picture) => picture.crop,
            Kind::Shape(_) => panic!(),
        };
        let start = |studio: &mut Studio| {
            if let Kind::Picture(picture) = &mut studio.doc.layers[0].kind {
                picture.crop = (0.0, 0.0, 400.0, 200.0);
                (picture.w, picture.h) = (400.0, 200.0);
            }
            studio.held = vec![studio.doc.layers[0].clone()];
        };
        // The bottom right corner pulled in, mostly across: free, it goes
        // out of shape; with Shift it keeps two to one.
        start(&mut studio);
        studio.crop_to((1, 1), (500.0, 300.0), (400.0, 290.0), Mods::default());
        let (_, _, w, h) = crop(&studio);
        assert!((w / h - 2.0).abs() > 0.2, "{w}x{h}");
        start(&mut studio);
        studio.crop_to((1, 1), (500.0, 300.0), (400.0, 290.0), Mods { shift: true, ..Mods::default() });
        let (x, y, w, h) = crop(&studio);
        assert!((w / h - 2.0).abs() < 0.01 && (x, y) == (0.0, 0.0), "{x},{y} {w}x{h}");
        // A shape chosen, 1:1, holds without Shift; a side pulled keeps
        // its middle, and it never leaves the picture.
        studio.crop_aspect = panel::CROP_ASPECTS.iter().position(|(name, _)| *name == "1:1").unwrap();
        start(&mut studio);
        studio.crop_to((1, 0), (500.0, 200.0), (300.0, 200.0), Mods::default());
        let (x, y, w, h) = crop(&studio);
        assert!((w - h).abs() < 0.01 && y >= 0.0 && y + h <= 200.01 && x == 0.0, "{x},{y} {w}x{h}");
    }
    /// A studio cropping a picture of a gray margin round a card with a
    /// panel in it, shown a pixel to a pixel of the artboard, its pixels
    /// where the artboard's are.
    fn cropping_a_card() -> Studio {
        let mut studio = studio();
        let pixels = edges::tests::card_picture(400, 300, (50, 40, 350, 260), (100, 80, 300, 200));
        studio.doc.layers.push(Layer::picture(Picture::new(Arc::new(Image::new(400, 300, pixels)), "card".into(), (200.0, 150.0))));
        studio.zoom = 1.0;
        studio.cropping = Some(0);
        studio.held = vec![studio.doc.layers[0].clone()];
        studio
    }

    fn crop_of(studio: &Studio) -> (f32, f32, f32, f32) {
        match &studio.doc.layers[0].kind {
            Kind::Picture(picture) => picture.crop,
            Kind::Shape(_) => panic!(),
        }
    }

    #[test]
    fn a_side_of_a_crop_snaps_to_a_line_in_the_picture_unless_ctrl_is_held() {
        let mut studio = cropping_a_card();
        studio.crop_to((1, 0), (400.0, 150.0), (304.0, 150.0), Mods::default());
        assert_eq!(crop_of(&studio), (0.0, 0.0, 300.0, 300.0), "onto the panel's right edge");
        assert_eq!(studio.crop_snaps, [(true, 300.0)]);
        studio.crop_to((1, 0), (400.0, 150.0), (304.0, 150.0), Mods { ctrl: true, ..Mods::default() });
        assert_eq!(crop_of(&studio), (0.0, 0.0, 304.0, 300.0));
        assert!(studio.crop_snaps.is_empty());
        // Too far from any line, it is where it is pulled to.
        studio.crop_to((1, 0), (400.0, 150.0), (320.0, 150.0), Mods::default());
        assert_eq!(crop_of(&studio).2, 320.0);
        // A corner snaps both ways, each to the nearer side of the card's
        // border.
        studio.crop_to((-1, -1), (0.0, 0.0), (47.0, 43.0), Mods::default());
        assert_eq!(crop_of(&studio), (50.0, 41.0, 350.0, 259.0));
        assert_eq!(studio.crop_snaps, [(true, 50.0), (false, 41.0)]);
        // Moved whole, it snaps by whichever side is nearest a line.
        if let Kind::Picture(picture) = &mut studio.doc.layers[0].kind {
            (picture.crop, picture.w, picture.h) = ((10.0, 10.0, 100.0, 100.0), 100.0, 100.0);
        }
        studio.held = vec![studio.doc.layers[0].clone()];
        studio.crop_to((0, 0), (0.0, 0.0), (-38.0, 0.0), Mods::default());
        assert_eq!(crop_of(&studio).0, 50.0, "its left side onto the card's edge");
        // Held more than six pixels of the window away, at half size it
        // still snaps, as the reach is of the window.
        let mut studio = cropping_a_card();
        studio.zoom = 0.5;
        studio.crop_to((1, 0), (400.0, 150.0), (310.0, 150.0), Mods::default());
        assert_eq!(crop_of(&studio).2, 300.0);
        studio.released();
        assert!(studio.crop_snaps.is_empty(), "the line goes when it is let go of");
    }

    #[test]
    fn tight_trims_the_margin_then_goes_in_and_back_out_a_step_to_undo_each() {
        let mut studio = cropping_a_card();
        let steps = studio.undo.len();
        studio.act(Action::Tight);
        assert_eq!(crop_of(&studio), (50.0, 40.0, 300.0, 220.0), "the gray margin trimmed off");
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Tight \u{b7} 1 of 2"));
        // Its pixels stay where they were on the artboard.
        let Kind::Picture(picture) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((picture.cx, picture.cy, picture.w, picture.h), (200.0, 150.0, 300.0, 220.0));
        studio.act(Action::Tight);
        assert_eq!(crop_of(&studio), (100.0, 80.0, 200.0, 120.0), "in to the panel");
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Tight \u{b7} 2 of 2"));
        studio.act(Action::Tight);
        assert_eq!(crop_of(&studio), (50.0, 40.0, 300.0, 220.0), "round to the first again");
        studio.mods.shift = true;
        studio.act(Action::Tight);
        assert_eq!(crop_of(&studio), (0.0, 0.0, 400.0, 300.0), "back out to all of it");
        assert_eq!(studio.undo.len(), steps + 4);
        studio.mods.shift = false;
        // From a crop of its own, in to the first inside it.
        if let Kind::Picture(picture) = &mut studio.doc.layers[0].kind {
            picture.recrop((80.0, 60.0, 240.0, 160.0));
        }
        studio.act(Action::Tight);
        assert_eq!(crop_of(&studio), (100.0, 80.0, 200.0, 120.0));
    }

    #[test]
    fn a_double_click_in_a_crop_fits_it_round_the_box_clicked_in_then_the_one_round_that() {
        let mut studio = cropping_a_card();
        studio.press_crop(0, (150.0, 150.0), true);
        assert_eq!(crop_of(&studio), (100.0, 80.0, 200.0, 120.0), "the panel");
        assert!(matches!(studio.drag, Drag::None));
        studio.press_crop(0, (150.0, 150.0), true);
        assert_eq!(crop_of(&studio), (50.0, 40.0, 300.0, 220.0), "the card round it, with its border");
        // A single press inside it takes hold of it as ever.
        studio.press_crop(0, (150.0, 150.0), false);
        assert!(matches!(studio.drag, Drag::Crop { pull: (0, 0), .. }));
    }

    #[test]
    fn a_third_click_quickly_after_the_double_click_that_begins_a_crop_does_not_fit_it() {
        let mut studio = cropping_a_card();
        (studio.cropping, studio.held) = (None, Vec::new());
        let at = studio.to_window((150.0, 150.0));
        studio.pointer_moved(at.0, at.1, Mods::default());
        for time in [1000, 1100] {
            studio.pressed(1, time);
            studio.released();
        }
        assert_eq!(studio.cropping, Some(0));
        studio.pressed(1, 1200);
        studio.released();
        assert_eq!(crop_of(&studio), (0.0, 0.0, 400.0, 300.0), "the whole picture still");
        // Two more, and that is a double click in the crop.
        studio.pressed(1, 1300);
        studio.released();
        assert_eq!(crop_of(&studio), (100.0, 80.0, 200.0, 120.0));
    }

    #[test]
    fn tight_keeps_to_the_shape_chosen_and_begins_a_crop_of_the_picture_picked_out() {
        let mut studio = cropping_a_card();
        studio.cropping = None;
        studio.selected = vec![0];
        studio.crop_aspect = panel::CROP_ASPECTS.iter().position(|(name, _)| *name == "1:1").unwrap();
        studio.act(Action::Tight);
        assert_eq!(studio.cropping, Some(0));
        let (x, y, w, h) = crop_of(&studio);
        assert_eq!((w, h), (300.0, 300.0), "grown about its middle to be square, inside the picture");
        assert!(x <= 50.0 && x + w >= 350.0 && y == 0.0, "{x},{y}");
        // Pressed again, it knows where it is, though kept square.
        studio.act(Action::Tight);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Tight \u{b7} 2 of 2"));
    }

    #[test]
    fn a_crop_kept_to_its_shape_snaps_the_side_pulled_and_follows_with_the_other() {
        let mut studio = cropping_a_card();
        studio.crop_to((1, 0), (400.0, 150.0), (304.0, 150.0), Mods { shift: true, ..Mods::default() });
        let (x, y, w, h) = crop_of(&studio);
        assert_eq!((x, w, h), (0.0, 300.0, 225.0), "{x},{y} {w}x{h}");
        assert!((y + h / 2.0 - 150.0).abs() < 0.01, "about its middle");
    }

    #[test]
    fn shift_pressed_partway_through_a_resize_keeps_the_shape_until_let_go() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 2.0, (100.0, 100.0));
        boxed.points[1] = (300.0, 200.0);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.fit();
        studio.selected = vec![0];
        let corner = studio.handles()[2].0;
        let size = |studio: &Studio| {
            let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
            let (_, _, w, h) = shape.frame();
            (w.round(), h.round())
        };
        studio.pointer_moved(corner.0, corner.1, Mods::default());
        studio.pressed(1, 1000);
        // Pulled out of shape: wider, not taller.
        let far = studio.to_window((400.0, 210.0));
        studio.pointer_moved(far.0, far.1, Mods::default());
        let (w, h) = size(&studio);
        assert!((w / h - 2.0).abs() > 0.3, "{w}x{h}");
        // Shift pressed, the pointer still: at once its shape again.
        studio.mods_changed(Mods { shift: true, ..Mods::default() });
        let (w, h) = size(&studio);
        assert!((w / h - 2.0).abs() < 0.02, "{w}x{h}");
        // Let go: free again, as it was.
        studio.mods_changed(Mods::default());
        let (w, h) = size(&studio);
        assert!((w / h - 2.0).abs() > 0.3, "{w}x{h}");
        studio.released();
    }
    #[test]
    fn a_frame_beside_the_view_is_drawn_as_nothing() {
        let mut studio = studio();
        studio.resize(1776, 1540);
        studio.new_frame((1000.0, 0.0), (800.0, 600.0));
        studio.fit();
        let mut out = vec![0u32; 1776 * 1540];
        // Both drawn, then the view moved so the second is wholly to the
        // right of it, level with it: what was drawn of it is kept, and
        // shown as nothing.
        studio.draw(&mut out);
        studio.pan.0 -= 3000.0;
        studio.draw(&mut out);
        studio.pan.0 += 6000.0;
        studio.draw(&mut out);
    }

    /// A frame's name pressed and dragged so far across the window, held
    /// there, not yet let go of, to be looked at.
    fn drag_frame(studio: &mut Studio, frame: usize, by: (i32, i32), mods: Mods) {
        let label = studio.frame_label(frame).0.center();
        studio.pointer_moved(label.0, label.1, mods);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(label.0 + by.0 / 2, label.1 + by.1 / 2, mods);
        studio.pointer_moved(label.0 + by.0, label.1 + by.1, mods);
    }

    #[test]
    fn a_frame_moved_on_its_page_snaps_to_the_others_and_spaces_evenly_with_them() {
        let mut studio = studio();
        // Two more beside the first, the first two 100 apart and the third
        // 200 from the second.
        studio.new_frame((900.0, 0.0), (400.0, 300.0));
        studio.new_frame((1500.0, 0.0), (400.0, 300.0));
        (studio.zoom, studio.pan) = (0.5, (50.0, 300.0));
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        let shift = Mods { shift: true, ..Mods::default() };
        // Down 8, within 6 of the window of the others' tops: back to them,
        // with a line across where.
        drag_frame(&mut studio, 2, (0, 4), Mods::default());
        assert_eq!((studio.doc.x, studio.doc.y), (1500.0, 0.0));
        assert_eq!(studio.guides, [(false, 0.0)]);
        studio.released();
        assert!(studio.guides.is_empty() && studio.gaps.is_empty());
        // Left 96, 104 from the second: to 100 from it, the gap the first
        // two have, both gaps shown.
        drag_frame(&mut studio, 2, (-48, 0), Mods::default());
        assert_eq!(studio.doc.x, 1400.0);
        assert!(studio.gaps.len() >= 2 && studio.gaps.iter().all(|gap| (gap.to - gap.from - 100.0).abs() < 0.5), "{:?}", studio.gaps);
        studio.released();
        // Ctrl: where it is put.
        drag_frame(&mut studio, 2, (2, 3), ctrl);
        assert_eq!((studio.doc.x, studio.doc.y), (1404.0, 6.0));
        assert!(studio.guides.is_empty() && studio.gaps.is_empty());
        studio.released();
        // Shift: only across, the way it went further.
        drag_frame(&mut studio, 2, (60, 20), shift);
        assert_eq!((studio.doc.x, studio.doc.y), (1524.0, 6.0));
        studio.released();
        // One step to undo each move.
        studio.key(Key::Char('z'), ctrl, true);
        assert_eq!((studio.doc.x, studio.doc.y), (1404.0, 6.0));
    }
    #[test]
    fn drawing_sharp_after_a_zoom_is_given_up_for_what_comes() {
        let mut studio = studio();
        studio.add_picture(checker(800, 600), "one");
        let mut words = Shape::new(Tool::Text, 0x1A1A1A, 2.0, (100.0, 500.0));
        words.text = "In bands".into();
        studio.doc.layers.push(Layer::shape(words));
        studio.changed();
        // Drawn a band at a time, the same as all at once.
        let part = Rect::new(30, 20, 700, 560);
        let whole = studio.doc.render_part(1.3, part, &mut studio.text);
        assert_eq!(studio.doc.render_part_stopping(1.3, part, &mut studio.text, &mut || false), Some(whole));
        let mut bands = 0;
        assert_eq!(studio.doc.render_part_stopping(1.3, part, &mut studio.text, &mut || { bands += 1; bands > 2 }), None);
        let mut out = vec![0u32; 1400 * 900];
        studio.draw(&mut out);
        let before = studio.zoom;
        studio.wheel_at(700, 450, Mods { ctrl: true, ..Mods::default() });
        studio.scrolled(0.0, -3.0);
        studio.draw(&mut out);
        studio.moving = Some(std::time::Instant::now() - SETTLE);
        studio.settle();
        studio.take_dirty();
        // Something came: what was drawn is shown still, and drawn sharp
        // the next time round.
        studio.draw_stopping(&mut out, &mut || true);
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(before));
        assert!(studio.take_dirty());
        studio.draw(&mut out);
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(studio.zoom));
        // The same zoom but for a speck left over by notches in and out
        // is the same zoom.
        assert!(same_zoom(studio.zoom, studio.zoom * (1.0 + 1e-6)) && !same_zoom(studio.zoom, studio.zoom * 1.001));
    }
    #[test]
    fn zooming_shows_what_was_drawn_until_the_page_is_still() {
        let mut studio = studio();
        // Red on the left half, blue on the right.
        studio.add_picture(Image::new(800, 600, (0..800 * 600).map(|at| if at % 800 < 400 { 0xFFFF0000 } else { 0xFF0000FF }).collect()), "halves");
        let mut out = vec![0u32; 1400 * 900];
        studio.draw(&mut out);
        let sharp = studio.zoom;
        let at = (560, 450);
        let before = out[(at.1 * 1400 + at.0) as usize];
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(sharp));
        // Zoomed in by the wheel: what was drawn, shown larger, the point
        // under the pointer the same color still; nothing drawn afresh.
        studio.wheel_at(at.0, at.1, Mods { ctrl: true, ..Mods::default() });
        studio.scrolled(0.0, -12.0);
        assert!(studio.settle_in().is_some_and(|wait| wait <= SETTLE));
        studio.draw(&mut out);
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(sharp));
        assert_eq!(out[(at.1 * 1400 + at.0) as usize], before);
        // Still a moment: drawn sharp at the zoom it is at, and no more
        // waiting to be.
        studio.settle();
        assert!(studio.moving.is_some());
        studio.moving = Some(std::time::Instant::now() - SETTLE);
        studio.settle();
        assert!(studio.moving.is_none() && studio.settle_in().is_none() && studio.dirty);
        studio.draw(&mut out);
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(studio.zoom));
        assert_eq!(out[(at.1 * 1400 + at.0) as usize], before);
        // Zoomed far out, past what was drawn: drawn again, at half the
        // size it is shown, for the next steps out to keep to.
        studio.scrolled(0.0, 12.0);
        studio.draw(&mut out);
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(studio.zoom / 2.0));
        let half = studio.zoom / 2.0;
        studio.scrolled(0.0, 2.0);
        studio.draw(&mut out);
        assert_eq!(studio.shown[0].as_ref().map(|kept| kept.zoom), Some(half));
        // A change to the frame is drawn at once, moving or not.
        studio.doc.layers[0].hidden = true;
        studio.changed();
        studio.draw(&mut out);
        assert!(studio.shown[0].as_ref().is_some_and(|kept| kept.stamp == studio.doc.stamp));
        assert_eq!(out[(at.1 * 1400 + at.0) as usize] & 0xFF_FFFF, 0xFFFFFF);
    }
    #[test]
    fn the_wheel_zooms_at_once_and_keeps_the_point_under_the_pointer() {
        let mut studio = studio();
        let (before, at) = (studio.zoom, (700, 450));
        let under = studio.to_page(at);
        studio.wheel_at(at.0, at.1, Mods { ctrl: true, ..Mods::default() });
        studio.mods.ctrl = true;
        // Two notches up: larger by 1.15 twice, there at once.
        studio.pressed(4, 1000);
        studio.pressed(4, 1010);
        assert!((studio.zoom / before - WHEEL_ZOOM * WHEEL_ZOOM).abs() < 1e-4, "{}", studio.zoom / before);
        let now = studio.to_page(at);
        assert!((now.0 - under.0).abs() < 0.5 && (now.1 - under.1).abs() < 0.5);
        // A notch down undoes a notch up; and a trackpad's ten tenths of
        // one come to the same as the notch.
        studio.pressed(5, 1020);
        assert!((studio.zoom / before - WHEEL_ZOOM).abs() < 1e-4);
        let notched = studio.zoom;
        studio.pressed(5, 1030);
        for _ in 0..10 {
            studio.scrolled(0.0, -0.1);
        }
        assert!((studio.zoom / notched - 1.0).abs() < 1e-4, "{}", studio.zoom / notched);
        assert!((wheel_zoom(1.0) * WHEEL_ZOOM - 1.0).abs() < 1e-6 && wheel_zoom(0.0) == 1.0);
        assert!((wheel_zoom(0.5) * wheel_zoom(0.5) - wheel_zoom(1.0)).abs() < 1e-6);
        // Without Ctrl, the page moved: down by a notch, across with Shift
        // or a notch to the side, and by parts of one as well.
        studio.mods.ctrl = false;
        let pan = studio.pan;
        let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3;
        studio.pressed(5, 1040);
        assert!(near(studio.pan, (pan.0, pan.1 - WHEEL_STEP)));
        studio.pressed(7, 1050);
        assert!(near(studio.pan, (pan.0 - WHEEL_STEP, pan.1 - WHEEL_STEP)));
        studio.mods.shift = true;
        studio.pressed(4, 1060);
        assert!(near(studio.pan, (pan.0, pan.1 - WHEEL_STEP)));
        studio.mods.shift = false;
        studio.scrolled(0.25, -0.5);
        assert!(near(studio.pan, (pan.0 - WHEEL_STEP / 4.0, pan.1 - WHEEL_STEP / 2.0)), "{:?}", studio.pan);
    }
    #[test]
    fn a_drag_inside_what_is_picked_out_moves_it_though_nothing_is_under_it() {
        let mut studio = studio();
        for at in [100.0, 400.0] {
            let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 2.0, (at, 100.0));
            boxed.points[1] = (at + 200.0, 300.0);
            studio.doc.layers.push(Layer::shape(boxed));
        }
        studio.fit();
        studio.selected = vec![0, 1];
        // In the middle of the first box, which has no fill, and dragged.
        let (inside, gap) = (studio.to_window((200.0, 200.0)), studio.to_window((350.0, 200.0)));
        for (from, by) in [(inside, 40), (gap, 40)] {
            studio.pointer_moved(from.0, from.1, Mods::default());
            studio.pressed(1, 1000);
            studio.pointer_moved(from.0 + by, from.1, Mods::default());
            studio.released();
            assert_eq!(studio.selected, vec![0, 1]);
        }
        let left = studio.doc.layers[0].bounds(&mut studio.text).0;
        assert!(left > 120.0, "{left}");
        // Outside them, a press lets go of them.
        let away = studio.to_window((750.0, 550.0));
        studio.pointer_moved(away.0, away.1, Mods::default());
        studio.pressed(1, 5000);
        studio.released();
        assert!(studio.selected.is_empty());
    }
    #[test]
    fn layers_are_spaced_framed_copied_and_pasted_and_right_clicked() {
        let mut studio = studio();
        for at in [0.0, 100.0, 400.0] {
            let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (at, 50.0));
            boxed.points[1] = (at + 50.0, 100.0);
            studio.doc.layers.push(Layer::shape(boxed));
        }
        studio.selected = vec![0, 1, 2];
        studio.act(Action::Distribute(0));
        let lefts: Vec<f32> = (0..3).map(|index| studio.doc.layers[index].bounds(&mut studio.text).0).collect();
        assert!((lefts[1] - lefts[0] - (lefts[2] - lefts[1])).abs() <= 1.5, "{lefts:?}");
        // Recolored at once, as the colors of the selection are.
        studio.paint(Paint::Recolor(0xE5484D), Some(0x3E63DD));
        assert_eq!(studio.selection_colors(), vec![(0x3E63DD, 3)]);
        // Copied and pasted: three more, each its own.
        studio.copy_layers();
        studio.paste_layers();
        assert_eq!((studio.doc.layers.len(), studio.selected.clone()), (6, vec![3, 4, 5]));
        // Framed: a new frame just round them, them in it.
        studio.act(Action::FrameThem);
        assert_eq!((studio.frame_count(), studio.doc.layers.len()), (2, 3));
        assert!(studio.doc.width >= 450 && studio.doc.width <= 470, "{}", studio.doc.width);
        // A right click on one of them: the menu for it, and its order.
        studio.fit();
        let middle = studio.to_window((1.0, 25.0));
        studio.right_click(middle);
        assert!(matches!(studio.menu, Some(MenuState { kind: MenuKind::Context, .. })) && studio.selected.len() == 3);
        let items = studio.menu_items(MenuKind::Context);
        assert!(items.iter().any(|item| matches!(item, menu::Item::Entry { sub: Some(MenuKind::Order), .. })));
    }

    #[test]
    fn a_click_off_an_open_list_closes_it_and_does_nothing_else_and_esc_closes_it() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        (boxed.points[1], boxed.filled) = ((200.0, 200.0), true);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.fit();
        let on_box = studio.to_window((150.0, 150.0));
        let empty = studio.to_window((600.0, 500.0));
        let click = |studio: &mut Studio, at: (i32, i32)| {
            studio.pointer_moved(at.0, at.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            studio.released();
        };
        // The right click's menu, closed by a click on the box: the box
        // is not picked out by it.
        studio.right_click(empty);
        assert!(studio.menu.is_some() && studio.selected.is_empty());
        click(&mut studio, on_box);
        assert!(studio.menu.is_none() && studio.selected.is_empty());
        // Picked out, and the menu closed by a click off it: still picked out.
        studio.right_click(on_box);
        assert_eq!(studio.selected, vec![0]);
        click(&mut studio, empty);
        assert_eq!((studio.menu.is_none(), studio.selected.clone()), (true, vec![0]));
        // The zoom's list, closed by its own button.
        let zoom = studio.control_rect(Control::Zoom).unwrap().center();
        click(&mut studio, zoom);
        assert!(matches!(studio.menu, Some(MenuState { kind: MenuKind::Zoom, .. })));
        click(&mut studio, zoom);
        assert!(studio.menu.is_none());
        // A group's list, closed by a click on the page, which draws nothing.
        studio.flyout = Some(0);
        click(&mut studio, empty);
        assert_eq!((studio.flyout, studio.doc.layers.len()), (None, 1));
        // The grid of styles, closed by a click off it.
        studio.key(Key::Char('s'), Mods::default(), true);
        assert!(studio.styling);
        click(&mut studio, empty);
        assert_eq!((studio.styling, studio.selected.clone()), (false, vec![0]));
        // Esc closes the list, then the colors, before letting go of the box.
        studio.right_click(on_box);
        studio.palette = Some(Paint::Line);
        studio.key(Key::Escape, Mods::default(), true);
        assert!(studio.menu.is_none() && studio.palette.is_some());
        studio.key(Key::Escape, Mods::default(), true);
        assert_eq!((studio.palette, studio.selected.clone()), (None, vec![0]));
        studio.key(Key::Escape, Mods::default(), true);
        assert!(studio.selected.is_empty());
    }

    #[test]
    fn the_keys_the_menus_say_do_what_they_say_and_ctrl_keys_while_typing_keep_to_the_text() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        boxed.points[1] = (200.0, 150.0);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.selected = vec![0];
        let shift = Mods { shift: true, ..Mods::default() };
        let ctrl_shift = Mods { shift: true, ctrl: true, ..Mods::default() };
        studio.key(Key::Char('H'), shift, true);
        assert_eq!((studio.space, studio.undo.len()), (false, 1));
        studio.key(Key::Char('V'), shift, true);
        assert_eq!(studio.undo.len(), 2);
        studio.key(Key::Char('H'), ctrl_shift, true);
        assert!(studio.doc.layers[0].hidden);
        studio.key(Key::Char('L'), ctrl_shift, true);
        assert!(studio.doc.layers[0].locked && studio.selected.is_empty());
        // Typing: Ctrl+Backspace takes a word, never the text itself; and
        // Ctrl+V pastes text into it, not the layers copied.
        let mut words = Shape::new(Tool::Text, 0x000000, 2.0, (300.0, 300.0));
        words.text = "two words".into();
        studio.doc.layers.push(Layer::shape(words));
        studio.selected = vec![1];
        studio.copy_layers();
        studio.edit_text(1);
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        studio.key(Key::Backspace, ctrl, true);
        studio.key(Key::Char('d'), ctrl, true);
        assert_eq!(studio.doc.layers.len(), 2);
        let Kind::Shape(shape) = &studio.doc.layers[1].kind else { panic!() };
        assert_eq!(shape.text, "two ");
        assert_eq!(studio.key(Key::Char('v'), ctrl, true), Outcome::Paste);
    }

    #[test]
    fn a_name_being_typed_goes_to_its_own_frame_when_another_is_clicked() {
        let mut studio = studio();
        studio.new_frame((1000.0, 0.0), (400.0, 300.0));
        studio.press_control(Control::Frame(0), false);
        studio.press_control(Control::Frame(0), true);
        assert!(matches!(studio.renaming, Some((Naming::Frame, _))));
        studio.renaming = Some((Naming::Frame, "Hero".into()));
        studio.press_control(Control::Frame(1), false);
        assert_eq!((studio.renaming.is_none(), studio.doc.name.as_str(), studio.frame(0).name.as_str()), (true, "Frame 2", "Hero"));
    }

    #[test]
    fn a_right_click_keeps_what_is_typed_and_the_bar_of_tools_has_no_menu() {
        let mut studio = studio();
        let mut words = Shape::new(Tool::Text, 0x000000, 2.0, (100.0, 100.0));
        words.text = "Hi".into();
        studio.doc.layers.push(Layer::shape(words));
        studio.selected = vec![0];
        studio.edit_text(0);
        studio.fit();
        let empty = studio.to_window((600.0, 500.0));
        studio.right_click(empty);
        assert!(studio.editing.is_none() && studio.menu.is_some() && studio.selected.is_empty());
        let tool = studio.control_rect(Control::Tool(Tool::Rect)).or_else(|| studio.control_rect(Control::Group(0))).unwrap().center();
        studio.flyout = Some(0);
        studio.right_click(tool);
        assert_eq!((studio.menu, studio.flyout), (None, None));
    }

    #[test]
    fn a_handle_clicked_and_let_go_leaves_nothing_to_undo_and_a_lock_can_be_undone() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        (boxed.points[1], boxed.filled) = ((300.0, 200.0), true);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.selected = vec![0];
        studio.fit();
        let (corner, _) = studio.handles()[0];
        studio.pointer_moved(corner.0, corner.1, Mods::default());
        studio.pressed(1, 100_000);
        assert!(matches!(studio.drag, Drag::Scale { .. }));
        studio.released();
        assert!(studio.undo.is_empty());
        // Dragged, it is a step.
        studio.pressed(1, 200_000);
        studio.pointer_moved(corner.0 - 30, corner.1 - 30, Mods::default());
        studio.released();
        assert_eq!(studio.undo.len(), 1);
        studio.press_control(Control::Lock(0), false);
        assert!(studio.doc.layers[0].locked);
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        assert!(!studio.doc.layers[0].locked);
    }

    #[test]
    fn text_being_typed_ends_when_the_layers_are_put_in_another_order() {
        let mut studio = studio();
        let mut words = Shape::new(Tool::Text, 0x000000, 2.0, (100.0, 100.0));
        words.text = "Hi".into();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (300.0, 300.0));
        boxed.points[1] = (400.0, 400.0);
        studio.doc.layers = vec![Layer::shape(words), Layer::shape(boxed)];
        studio.selected = vec![0];
        studio.edit_text(0);
        studio.act(Action::Forward);
        assert_eq!((studio.editing, studio.selected.clone()), (None, vec![1]));
        studio.key(Key::Char('x'), Mods::default(), true);
        let Kind::Shape(shape) = &studio.doc.layers[1].kind else { panic!() };
        assert_eq!(shape.text, "Hi");
        // Another layer clicked in the list ends it too.
        studio.edit_text(1);
        let row = studio.control_rect(Control::Layer(0)).unwrap().center();
        studio.pointer_moved(row.0, row.1, Mods::default());
        studio.pressed(1, 100_000);
        studio.released();
        assert_eq!((studio.editing, studio.selected.clone()), (None, vec![0]));
    }

    #[test]
    fn ctrl_k_closes_the_rest_and_the_wheel_leaves_it_open() {
        let mut studio = studio();
        let mut words = Shape::new(Tool::Text, 0x000000, 2.0, (100.0, 100.0));
        words.text = "Hi".into();
        studio.doc.layers.push(Layer::shape(words));
        studio.selected = vec![0];
        studio.edit_text(0);
        (studio.flyout, studio.palette) = (Some(0), Some(Paint::Line));
        studio.key(Key::Char('k'), Mods { ctrl: true, ..Mods::default() }, true);
        assert!(studio.search.is_some() && studio.editing.is_none() && studio.flyout.is_none() && studio.palette.is_none());
        studio.pointer_moved(700, 800, Mods::default());
        studio.pressed(5, 1000);
        assert!(studio.search.is_some());
        studio.pressed(1, 2000);
        studio.released();
        assert!(studio.search.is_none());
    }

    #[test]
    fn the_text_tool_on_text_already_there_types_into_it() {
        let mut studio = studio();
        let mut words = Shape::new(Tool::Text, 0x000000, 2.0, (100.0, 100.0));
        (words.text, words.font_px) = ("Hello".into(), 32);
        studio.doc.layers.push(Layer::shape(words));
        studio.fit();
        studio.key(Key::Char('t'), Mods::default(), true);
        let (x, y, w, h) = studio.doc.layers[0].bounds(&mut studio.text);
        let on = studio.to_window((x + w / 2.0, y + h / 2.0));
        studio.pointer_moved(on.0, on.1, Mods::default());
        studio.pressed(1, 100_000);
        studio.released();
        assert_eq!((studio.doc.layers.len(), studio.editing), (1, Some(0)));
        studio.key(Key::Char('!'), Mods::default(), true);
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!(shape.text, "Hello!");
    }

    #[test]
    fn a_hex_color_typed_is_kept_when_the_colors_are_clicked_off() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        boxed.points[1] = (200.0, 150.0);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.selected = vec![0];
        studio.fit();
        let color = |studio: &Studio| match &studio.doc.layers[0].kind {
            Kind::Shape(shape) => shape.color,
            Kind::Picture(_) => 0,
        };
        for (typed, wanted, off) in [("00FF00", 0x00FF00, false), ("0000FF", 0x0000FF, true)] {
            studio.selected = vec![0];
            studio.open_picker(Paint::Line);
            studio.renaming = Some((Naming::Hex, typed.into()));
            if off {
                // Off it onto another control: the zoom.
                studio.press_control(Control::Zoom, false);
            } else {
                // Off it onto the page, a little way from the box.
                let empty = studio.to_window((10.0, 590.0));
                studio.pointer_moved(empty.0, empty.1, Mods::default());
                studio.pressed(1, 100_000);
                studio.released();
            }
            assert_eq!((color(&studio), studio.palette), (wanted, None));
        }
    }

    /// The row of the open list that says something, chosen.
    fn choose(studio: &mut Studio, says: &str) -> Outcome {
        let kind = studio.menu.expect("a list is open").kind;
        let row = studio.menu_items(kind).iter().position(|item| matches!(item, menu::Item::Entry { says: known, .. } if known == says)).unwrap_or_else(|| panic!("no {says} in {kind:?}"));
        studio.press_menu(0, row as u16)
    }

    #[test]
    fn the_rows_of_the_panels_and_the_page_have_menus_that_pick_out_what_is_right_clicked() {
        let mut studio = studio();
        for at in [100.0, 300.0] {
            let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (at, 100.0));
            boxed.points[1] = (at + 50.0, 150.0);
            studio.doc.layers.push(Layer::shape(boxed));
        }
        let mut other = Shape::new(Tool::Rect, 0x3E63DD, 3.5, (500.0, 100.0));
        other.points[1] = (550.0, 150.0);
        studio.doc.layers.push(Layer::shape(other));
        studio.fit();
        // A layer's row: picked out by the right click, and hidden from it.
        let row = studio.control_rect(Control::Layer(1)).unwrap().center();
        studio.right_click(row);
        assert_eq!((studio.menu.map(|menu| menu.kind), studio.selected.clone()), (Some(MenuKind::Layer(1)), vec![1]));
        choose(&mut studio, "Hide");
        assert!(studio.doc.layers[1].hidden && studio.menu.is_none());
        studio.key(Key::Char('z'), Mods { ctrl: true, ..Mods::default() }, true);
        // Its color, and everything else in it.
        studio.right_click(row);
        choose(&mut studio, "Select same color");
        assert_eq!(studio.selected, vec![0, 1]);
        // The same from a color of several picked out.
        studio.selected = vec![0, 2];
        let swatch = studio.control_rect(Control::ColorOpen(0xE5484D)).unwrap().center();
        studio.right_click(swatch);
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Color(0xE5484D)));
        choose(&mut studio, "Select all with this color");
        assert_eq!(studio.selected, vec![0, 1]);
        // The page, empty: zoom and rulers.
        let empty = studio.to_window((700.0, 500.0));
        studio.right_click(empty);
        assert!(studio.selected.is_empty());
        choose(&mut studio, "Rulers");
        assert!(studio.rulers);
        // A page's row: a new page.
        let page = studio.control_rect(Control::Page(0)).unwrap().center();
        studio.right_click(page);
        choose(&mut studio, "New page");
        assert_eq!((studio.pages.len(), studio.page), (2, 1));
        studio.commit_name();
        // A frame's row: picked out, and copied beside itself.
        let frame = studio.control_rect(Control::Frame(0)).unwrap().center();
        studio.right_click(frame);
        assert!(studio.frame_picked && matches!(studio.menu, Some(MenuState { kind: MenuKind::Frame(0), .. })));
        choose(&mut studio, "Duplicate frame");
        assert_eq!((studio.frame_count(), studio.active), (2, 1));
        assert!(studio.doc.name.ends_with("copy") && studio.doc.x > studio.frame(0).x);
    }

    /// A click of the left button at a point of the window, never taken
    /// for the second of a double click.
    fn click(studio: &mut Studio, at: (i32, i32)) -> Outcome {
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        let outcome = studio.pressed(1, 100_000);
        studio.released();
        outcome
    }

    #[test]
    fn a_panel_is_hidden_and_shown_by_a_button_that_stays_where_it_is() {
        let mut studio = studio();
        let ctrl_shift = Mods { ctrl: true, shift: true, ..Mods::default() };
        let left = studio.control_rect(Control::SidePanel(0)).unwrap();
        let right = studio.control_rect(Control::SidePanel(1)).unwrap();
        let in_panel = (INSET + 100, 400);
        assert!(!studio.over_page(in_panel) && studio.control_rect(Control::Home).is_some());
        // Ctrl+Shift+\ hides the left panel: the page reaches the edge,
        // and its button is the one thing left there, where it was.
        studio.key(Key::Char('|'), ctrl_shift, true);
        assert!(!studio.left_open && studio.over_page(in_panel) && studio.control_rect(Control::Home).is_none());
        assert_eq!(studio.control_rect(Control::SidePanel(0)), Some(left));
        assert_eq!(studio.control_at(in_panel), None);
        assert!(studio.open_area().x < INSET + LEFT_W);
        let mut out = vec![0u32; (1400 * 900) as usize];
        studio.draw(&mut out);
        assert_ne!(out[(400 * 1400 + INSET + 100) as usize] & 0xFFFFFF, PANEL_COLOR);
        // Its button brings it back.
        click(&mut studio, left.center());
        assert!(studio.left_open && studio.control_rect(Control::Home).is_some());
        // The right one the same, by Ctrl+Alt+\ and its own button.
        studio.key(Key::Char('\\'), Mods { ctrl: true, alt: true, ..Mods::default() }, true);
        assert!(!studio.right_open && studio.control_rect(Control::Zoom).is_none());
        assert_eq!(studio.control_rect(Control::SidePanel(1)), Some(right));
        assert!(studio.over_page((1400 - INSET - 100, 400)));
        click(&mut studio, right.center());
        assert!(studio.right_open && studio.control_rect(Control::Zoom).is_some());
        // Kept between runs as two words, both shown when nothing is kept.
        assert_eq!((read_panels("0 1\n"), read_panels("1 0"), read_panels("")), ((false, true), (true, false), (true, true)));
    }

    #[test]
    fn ctrl_backslash_hides_everything_but_the_page_and_the_button_that_brings_it_back() {
        let mut studio = studio();
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        studio.rulers = true;
        let button = studio.control_rect(Control::SidePanel(0)).unwrap();
        studio.key(Key::Char('\\'), ctrl, true);
        assert!(studio.zen);
        // Nothing left but the page and the one button, where it was, and
        // the switch between Design and Board beside it.
        let layout = studio.layout();
        assert!(layout.panels.is_empty() && layout.dock.w == 0);
        let controls: Vec<Control> = layout.controls.iter().map(|(_, control)| *control).filter(|control| !matches!(control, Control::Frame(_))).collect();
        assert_eq!(controls, vec![Control::SidePanel(0), Control::Board(board::Ctl::Mode(false)), Control::Board(board::Ctl::Mode(true))]);
        assert_eq!(studio.control_rect(Control::SidePanel(0)), Some(button));
        assert!(!studio.press_rulers((700, 5)) && studio.over_page((5, 450)) && studio.over_page((1395, 450)));
        // Esc leaves it as it is; the tools' keys still work, each said
        // for a moment.
        studio.key(Key::Escape, Mods::default(), true);
        assert!(studio.zen);
        studio.key(Key::Char('r'), Mods::default(), true);
        assert_eq!(studio.tool, Tool::Rect);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Rectangle  R"));
        assert!(studio.note_left().is_some_and(|left| left.as_secs_f32() <= 1.4));
        let mut out = vec![0u32; (1400 * 900) as usize];
        studio.draw(&mut out);
        // The button brings everything back, as Ctrl+\ again does.
        click(&mut studio, button.center());
        assert!(!studio.zen && studio.left_open && studio.layout().dock.w > 0);
        studio.key(Key::Char('\\'), ctrl, true);
        studio.key(Key::Char('\\'), ctrl, true);
        assert!(!studio.zen);
        // It is in the menus, and so in Ctrl+K.
        studio.open_palette();
        for letter in "zen".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        studio.key(Key::Enter, Mods::default(), true);
        assert!(studio.zen && studio.search.is_none());
    }

    #[test]
    fn the_keys_go_through_an_open_list_and_a_letter_takes_up_no_tool() {
        let mut studio = studio();
        let none = Mods::default();
        let says = |studio: &mut Studio| {
            let (level, row) = studio.menu_row.expect("a row come to");
            let menu = studio.menu.expect("a list is open");
            let kind = if level == 0 { menu.kind } else { menu.sub.unwrap().1 };
            match studio.menu_items(kind).into_iter().nth(row) {
                Some(menu::Item::Entry { says, .. }) => says,
                _ => panic!("not an entry"),
            }
        };
        // The main menu: Down to the first row, Up round to the last.
        let home = studio.control_rect(Control::Home).unwrap().center();
        click(&mut studio, home);
        studio.key(Key::Down, none, true);
        assert_eq!(says(&mut studio), "Back to projects");
        studio.key(Key::Up, none, true);
        assert_eq!(says(&mut studio), "Close");
        // A letter does nothing while it is open: no tool taken up.
        studio.key(Key::Char('r'), none, true);
        assert!(studio.tool == Tool::Select && studio.menu.is_some());
        // Down round to the top and past the rule to File; Right into it,
        // Left back out.
        studio.key(Key::Down, none, true);
        studio.key(Key::Down, none, true);
        assert_eq!(says(&mut studio), "File");
        studio.key(Key::Right, none, true);
        assert_eq!((says(&mut studio), studio.menu.unwrap().sub.map(|(_, kind)| kind)), ("New page".to_owned(), Some(MenuKind::MainSub(0))));
        studio.key(Key::Left, none, true);
        assert_eq!((says(&mut studio), studio.menu.unwrap().sub), ("File".to_owned(), None));
        // Into View, down to Rulers, and Enter does it and closes the list.
        studio.key(Key::Down, none, true);
        studio.key(Key::Down, none, true);
        studio.key(Key::Enter, none, true);
        assert_eq!(studio.menu.unwrap().sub.map(|(_, kind)| kind), Some(MenuKind::MainSub(2)));
        while says(&mut studio) != "Rulers" {
            studio.key(Key::Down, none, true);
        }
        studio.key(Key::Enter, none, true);
        assert!(studio.rulers && studio.menu.is_none());
        // The right click's list the same, and Esc closes it.
        let empty = studio.to_window((700.0, 500.0));
        studio.right_click(empty);
        studio.key(Key::Down, none, true);
        assert_eq!(says(&mut studio), "Paste here");
        studio.key(Key::Escape, none, true);
        assert!(studio.menu.is_none());
        studio.key(Key::Char('r'), none, true);
        assert_eq!(studio.tool, Tool::Rect);
    }

    #[test]
    fn a_color_right_clicked_copies_its_hex() {
        let mut studio = studio();
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        boxed.points[1] = (200.0, 200.0);
        studio.doc.layers.push(Layer::shape(boxed));
        studio.selected = vec![0];
        // The color of what is picked out.
        let field = studio.control_rect(Control::ColorField(Paint::Line)).expect("its color").center();
        studio.right_click(field);
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Hex(0xE5484D)));
        choose(&mut studio, "Copy hex");
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Copied #E5484D"));
        // A color among several picked out has it too, under the rest.
        studio.menu = Some(MenuState { kind: MenuKind::Color(0x3E63DD), at: (600, 300), sub: None });
        choose(&mut studio, "Copy hex");
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Copied #3E63DD"));
    }

    #[test]
    fn a_gradient_goes_the_way_it_is_turned_and_is_kept() {
        let mut text = Text::load();
        let mut doc = Doc::new(200, 100);
        doc.background = Fill::Gradient(0x000000, 0xFFFFFF, 90.0);
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(out[50 * 200 + 2] & 0xFF < 0x10 && out[50 * 200 + 197] & 0xFF > 0xF0);
        doc.background = Fill::Gradient(0xFF0000, 0x0000FF, 180.0);
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(out[2 * 200 + 100] & 0xFF_FFFF > 0xF00000 && out[97 * 200 + 100] & 0xFF > 0xF0);
        assert_eq!(Doc::read(&doc.write()).unwrap().background, doc.background);
    }

    /// The canvas as it is in a window, at a few moments, each written as
    /// a PNG into `$HOTSHOT_SNAPSHOTS`, to be looked at.
    ///
    ///     HOTSHOT_SNAPSHOTS=/tmp/shots cargo test canvas_snapshots -- --ignored
    #[test]
    #[ignore]
    fn canvas_snapshots() {
        let Some(dir) = std::env::var_os("HOTSHOT_SNAPSHOTS").map(PathBuf::from) else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let (width, height) = (1600, 1000);
        let mut out = vec![0u32; (width * height) as usize];
        let mut write = |studio: &mut Studio, name: &str| {
            studio.draw(&mut out);
            std::fs::write(dir.join(format!("{name}.png")), crate::output::encode(width as u32, height as u32, &out, false).unwrap()).unwrap();
        };
        let mut doc = Doc::new(1200, 675);
        doc.background = Fill::Backdrop(frame::named("aurora").unwrap());
        let mut studio = Studio::new(doc, Text::load(), width, height, std::env::temp_dir(), "shot".into());
        write(&mut studio, "c1-empty");
        let desk = Image::open(&std::fs::read("docs/desk.png").unwrap()).unwrap();
        studio.add_picture(desk, "desk");
        studio.set_field(Field::W, 900.0);
        studio.set_field(Field::H, 506.0);
        studio.set_field(Field::Radius, 14.0);
        studio.press_control(Control::Shadow(7), false);
        write(&mut studio, "c2-picture");
        studio.key(Key::Char('a'), Mods::default(), true);
        let (a, b) = (studio.to_window((700.0, 120.0)), studio.to_window((900.0, 260.0)));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.pressed(1, 1000);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
        write(&mut studio, "c3-arrow");
        // A rounded box and some text, close up: drawn at that size, sharp.
        studio.key(Key::Char('r'), Mods::default(), true);
        let (a, b) = (studio.to_window((180.0, 520.0)), studio.to_window((330.0, 600.0)));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.pressed(1, 20_000);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
        let mut words = Shape::new(Tool::Text, 0x111111, 3.5, (200.0, 540.0));
        (words.text, words.font_px) = ("Sharp".into(), 26);
        studio.doc.layers.push(Layer::shape(words));
        studio.changed();
        write(&mut studio, "c3b-shapes");
        let near = studio.to_window((255.0, 560.0));
        studio.zoom_about(5.0, near);
        write(&mut studio, "c3c-zoomed");
        studio.fit();
        studio.selected = vec![0];
        studio.cropping = Some(0);
        write(&mut studio, "c4-crop");
        studio.cropping = None;
        studio.selected.clear();
        let (_, _, pixels) = studio.doc.render(1.0, &mut studio.text);
        std::fs::write(dir.join("c5-export.png"), crate::output::encode(1200, 675, &pixels, false).unwrap()).unwrap();
    }
}
