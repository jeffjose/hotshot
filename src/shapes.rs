//! What is drawn on a screenshot: arrows, boxes, text and the rest. Each
//! is a few points and a color; its outline is worked out once, as how
//! much of every pixel it covers, and laid down from that whenever the
//! picture is redrawn.

use std::cell::RefCell;
use std::rc::Rc;

use rayon::prelude::*;

use crate::canvas::{Canvas, Mask, Rect};
use crate::text::{Look, Text};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Arrow,
    Line,
    Rect,
    Ellipse,
    Pen,
    Marker,
    Text,
    Counter,
    Blur,
    /// A rectangle left as it is while everything round it is darkened.
    Spotlight,
    /// A word or two on a pill of color: "Bug", "Fixed", "Before".
    Badge,
    /// A part of the picture shown again, larger, beside itself.
    Zoom,
}

/// How much of its brightness what is outside every spotlight keeps,
/// out of 256.
const SPOTLIGHT_KEEPS: u32 = 96;

/// Draws shapes on a picture, the first under the last, within an area
/// of it. The spotlights come before all the rest, whatever their place
/// among them: they darken the picture, and not what is drawn on it,
/// which is as often as not an arrow to what the spotlight is on.
pub fn paint(shapes: &[Shape], canvas: &mut Canvas, text: &mut Text, area: Rect) {
    let lit: Vec<(Rect, i32)> = shapes.iter().filter(|shape| shape.tool == Tool::Spotlight).map(|shape| (shape.frame_rect(), shape.radius.round() as i32)).collect();
    canvas.set_clip(area);
    if !lit.is_empty() {
        canvas.spotlight(area, &lit, SPOTLIGHT_KEEPS);
    }
    for shape in shapes {
        shape.draw(canvas, text);
    }
    canvas.reset_clip();
}

pub type Point = (f32, f32);

/// How opaque a marker's stroke is, out of 256: what is under it is
/// still to be read.
const MARKER_OPACITY: u32 = 104;
/// How near a click has to come to a shape to be on it, in pixels.
const REACH: i32 = 4;

#[derive(Clone)]
pub struct Shape {
    pub tool: Tool,
    pub color: u32,
    /// The width of its line; for a mosaic, what sets the size of its squares.
    pub stroke: f32,
    pub filled: bool,
    /// The radius of a rectangle's corners.
    pub radius: f32,
    /// The radius of each of its corners, when they are not all the same:
    /// top left, top right, bottom right and bottom left, as Figma has
    /// them. None when every corner is `radius`.
    pub corners: Option<[f32; 4]>,
    /// What a box or an oval is filled with, when it is filled with more
    /// than one color: a gradient from its `fill` to another. None for its
    /// `fill` alone.
    pub gradient: Option<Gradient>,
    /// Two for most: where the drag began and where it is. A pen stroke
    /// has as many as the pointer made; text has the top-left of its
    /// first line; a counter its middle and where it points.
    pub points: Vec<Point>,
    pub text: String,
    pub font_px: u32,
    /// Which of the faces its text is in, counted from 0.
    pub face: usize,
    /// Whether a line has an arrow's head at its first end and at its
    /// last: an arrow is a line with one at its last.
    pub heads: [bool; 2],
    /// What the head at each end is, where there is one: one of the
    /// `HEADS`, counted from 0.
    pub ends: [u8; 2],
    /// Whether a line or an arrow is drawn in dashes; in dots; with an
    /// edge of white all round it, to show on a busy picture; and going
    /// across and then down, round a corner, rather than straight.
    pub dashed: bool,
    pub dotted: bool,
    pub halo: bool,
    pub elbow: bool,
    /// Whether a box, an oval, a line or an arrow is drawn by hand: a
    /// little uneven, its line gone over twice.
    pub sketch: bool,
    /// What a rectangle or an oval is filled with, when it is `filled`;
    /// its `color` is then that of its border, if it has one.
    pub fill: u32,
    /// Whether a rectangle or an oval has a border. One that has none is
    /// filled, or there would be nothing of it.
    pub border: bool,
    /// How opaque it is, out of 256.
    pub opacity: u32,
    /// How its text is set, as some of `BOLD`, `ITALIC`, `HIGHLIGHT`
    /// and `BULLETS` together.
    pub style: u8,
    pub number: u32,
    /// How far it is turned about its middle, clockwise, in radians:
    /// boxes, ovals, text and badges. Lines and strokes are turned by
    /// their points instead.
    pub angle: f32,
    /// What it is called among the shapes it is with, for an arrow to
    /// be stuck to it by, which stays when it moves and when the rest
    /// are put in another order.
    pub id: u32,
    /// What each end of a line or an arrow is stuck to, by its `id`, and
    /// where on it: across it and down it unturned, from -1 to 1; or at
    /// `WHOLE`, stuck to it as a whole, stopping at its edge on the way
    /// to the other end.
    pub stuck: [Option<(u32, f32, f32)>; 2],
    /// How the lines of text are set: from the left, in the middle, or to
    /// the right; 0, 1 or 2.
    pub align: u8,
    /// Where the border of a box or an oval lies on its edge: across the
    /// middle of it, inside it, or outside it; `CENTER`, `INSIDE` or
    /// `OUTSIDE`.
    pub edge: u8,
    /// The shadow it casts, if it casts one, in pixels of what it is on:
    /// how far across and down, how soft, how much larger, and its color
    /// with how opaque it is.
    pub shadow: Option<crate::frame::Layer>,
    /// Whether its text is being typed into, which shows the hashes a
    /// heading begins with; not kept in its file.
    pub editing: bool,
    mask: RefCell<Option<Rc<Mask>>>,
    /// The same of what is inside the border of a rectangle or an oval
    /// that has both, which is drawn in its fill.
    inside: RefCell<Option<Rc<Mask>>>,
}

/// The ways text can be set, any of them together: heavier, slanted,
/// on a wash of yellow as if gone over with a marker, and each line
/// after a dot.
pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const HIGHLIGHT: u8 = 4;
pub const BULLETS: u8 = 8;
/// Lines under the words, lines through them, and the words in capitals.
pub const UNDERLINE: u8 = 32;
pub const STRIKE: u8 = 64;
pub const UPPER: u8 = 128;

/// How much larger than the rest of the text a line is that begins with
/// one, two or three hashes and a space: a heading, of the first, second
/// or third level.
pub const HEADINGS: [f32; 3] = [2.0, 1.5, 1.25];

/// A line of text as it is shown: its words, at what size, how far down
/// from the top of the text, how wide, and whether it is a heading.
pub struct TextLine {
    pub shown: String,
    pub px: u32,
    pub top: i32,
    pub width: i32,
    pub heading: bool,
}

/// What level of heading a line is, from the hashes it begins with, and
/// the rest of it after them.
pub fn heading_of(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if !(1..=3).contains(&hashes) {
        return None;
    }
    line[hashes..].strip_prefix(' ').map(|rest| (hashes, rest))
}

/// Where a border lies on the edge of a box or an oval.
pub const CENTER: u8 = 0;
pub const INSIDE: u8 = 1;
pub const OUTSIDE: u8 = 2;

/// A shadow as a word of a file: its numbers with colons between, or a
/// dash for none.
fn shadow_word(shadow: Option<crate::frame::Layer>) -> String {
    shadow.map_or("-".to_owned(), |drop| format!("{}:{}:{}:{}:{:08X}", drop.x, drop.y, drop.blur, drop.spread, drop.color))
}

fn shadow_read(word: &str) -> Option<crate::frame::Layer> {
    let parts: Vec<&str> = word.split(':').collect();
    let [x, y, blur, spread, color] = parts[..] else { return None };
    Some(crate::frame::Layer { x: x.parse().ok()?, y: y.parse().ok()?, blur: blur.parse::<i16>().ok()?.max(0), spread: spread.parse().ok()?, color: u32::from_str_radix(color, 16).ok()? })
}
/// The radius of each corner as a word of a file: the four with colons
/// between, or a dash when they are all the one radius.
pub fn corners_word(corners: Option<[f32; 4]>) -> String {
    corners.map_or("-".to_owned(), |[a, b, c, d]| format!("{a}:{b}:{c}:{d}"))
}

pub fn corners_read(word: &str) -> Option<[f32; 4]> {
    let parts: Vec<f32> = word.split(':').map(|part| part.parse::<f32>().ok().filter(|radius| radius.is_finite())).collect::<Option<_>>()?;
    let [a, b, c, d] = parts[..] else { return None };
    Some([a, b, c, d].map(|radius| radius.max(0.0)))
}

/// A fill of more than one color, as Figma's are: from a shape's `fill` to
/// `to`, straight across it the way `angle` says in degrees, as CSS's
/// linear-gradient goes (0 up, 90 to the right); or `radial`, out from its
/// middle to its edge.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Gradient {
    pub to: u32,
    pub angle: f32,
    pub radial: bool,
}

/// A gradient as a word of a file: its color, its angle, and l or r for
/// straight or round; or a dash for none.
fn gradient_word(gradient: Option<Gradient>) -> String {
    gradient.map_or("-".to_owned(), |gradient| format!("{:06X}:{}:{}", gradient.to, gradient.angle, if gradient.radial { "r" } else { "l" }))
}

fn gradient_read(word: &str) -> Option<Gradient> {
    let parts: Vec<&str> = word.split(':').collect();
    let [to, angle, kind] = parts[..] else { return None };
    let angle = angle.parse::<f32>().ok().filter(|angle| angle.is_finite())?;
    Some(Gradient { to: u32::from_str_radix(to, 16).ok()? & 0xFF_FFFF, angle, radial: kind == "r" })
}

/// A color part of the way from one to another, 0 to 1.
pub fn between(from: u32, to: u32, along: f32) -> u32 {
    let along = along.clamp(0.0, 1.0);
    let part = |shift: u32| {
        let (a, b) = ((from >> shift & 0xFF) as f32, (to >> shift & 0xFF) as f32);
        ((a + (b - a) * along).round() as u32).min(255) << shift
    };
    part(16) | part(8) | part(0)
}

/// How far along a gradient a point is, from 0 to 1, in a box `w` by `h`
/// with the point counted from its middle: straight across at `angle`
/// degrees as CSS has it, the line long enough to reach its corners; or
/// round, out from the middle to the edge of the oval it holds.
pub fn along_gradient(x: f32, y: f32, w: f32, h: f32, angle: f32, radial: bool) -> f32 {
    if radial {
        return (x / (w / 2.0).max(0.5)).hypot(y / (h / 2.0).max(0.5));
    }
    let (sin, cos) = angle.to_radians().sin_cos();
    let length = (w * sin).abs() + (h * cos).abs();
    (x * sin - y * cos) / length.max(1.0) + 0.5
}

/// The radius of the corner of a box nearest a point, counted from its
/// middle: of the four, top left, top right, bottom right, bottom left.
pub fn corner_of(x: f32, y: f32, radii: [f32; 4]) -> f32 {
    match (x >= 0.0, y >= 0.0) {
        (true, false) => radii[1],
        (true, true) => radii[2],
        (false, true) => radii[3],
        (false, false) => radii[0],
    }
}

/// On a patch of its own color, its letters in whichever of white and
/// black reads on that.
pub const BOXED: u8 = 16;

/// The heads a line can have at either end, by what each is called: the
/// usual solid one, two strokes, an outline, a dot, a ring, a diamond
/// and a bar across, which says where a measure stops.
pub const HEADS: [&str; 7] = ["Filled", "Open", "Hollow", "Dot", "Ring", "Diamond", "Bar"];
pub const FILLED_HEAD: u8 = 0;
const OPEN_HEAD: u8 = 1;
const HOLLOW_HEAD: u8 = 2;
const DOT_HEAD: u8 = 3;
const RING_HEAD: u8 = 4;
const DIAMOND_HEAD: u8 = 5;
const BAR_HEAD: u8 = 6;

/// What a highlight is the color of, and how opaque out of 256.
const HIGHLIGHT_COLOR: (u32, u32) = (0xFFE629, 150);

/// How many times larger a zoom shows what it is of.
const ZOOM: i32 = 2;

/// A tool by the name it is kept under, or the move tool's own.
pub fn tool_named(name: &str) -> Option<Tool> {
    match name.to_ascii_lowercase().as_str() {
        "move" | "select" => Some(Tool::Select),
        "rectangle" => Some(Tool::Rect),
        "ellipse" => Some(Tool::Ellipse),
        name => TOOL_NAMES.iter().find(|(_, known)| *known == name).map(|(tool, _)| *tool),
    }
}

/// The names the tools are kept under, in a file of shapes.
const TOOL_NAMES: [(Tool, &str); 12] = [
    (Tool::Arrow, "arrow"),
    (Tool::Line, "line"),
    (Tool::Rect, "rect"),
    (Tool::Ellipse, "oval"),
    (Tool::Pen, "pen"),
    (Tool::Marker, "marker"),
    (Tool::Text, "text"),
    (Tool::Counter, "counter"),
    (Tool::Blur, "blur"),
    (Tool::Spotlight, "spotlight"),
    (Tool::Badge, "badge"),
    (Tool::Zoom, "zoom"),
];

impl Shape {
    pub fn new(tool: Tool, color: u32, stroke: f32, at: Point) -> Shape {
        let points = match tool {
            Tool::Pen | Tool::Marker | Tool::Text | Tool::Badge => vec![at],
            Tool::Zoom => vec![at, at, at, at],
            _ => vec![at, at],
        };
        Shape { tool, color, stroke, filled: false, radius: 0.0, corners: None, gradient: None, points, text: String::new(), font_px: 16, face: 0, heads: [false, tool == Tool::Arrow], ends: [FILLED_HEAD; 2], dashed: false, dotted: false, halo: false, elbow: false, sketch: false, fill: 0xFFFFFF, border: true, opacity: 256, style: 0, number: 0, angle: 0.0, id: fresh_id(), stuck: [None; 2], align: 0, edge: CENTER, shadow: None, editing: false, mask: RefCell::new(None), inside: RefCell::new(None) }
    }

    /// The shape as a line of a file, to be read back by `read`: its
    /// fields with tabs between, its points last but for its text.
    pub fn write(&self) -> String {
        let name = TOOL_NAMES.iter().find(|(tool, _)| *tool == self.tool).map_or("", |(_, name)| name);
        let points: Vec<String> = self.points.iter().map(|point| format!("{},{}", point.0, point.1)).collect();
        let text = self.text.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', " ");
        format!(
            "{name}\t{:06X}\t{}\t{}\t{}\t{}\t{}\t{}{}\t{}\t{:06X}\t{}\t{}\t{text}",
            self.color,
            self.stroke,
            u8::from(self.filled),
            self.radius,
            self.font_px,
            self.face,
            u8::from(self.heads[0]),
            u8::from(self.heads[1]),
            format_args!(
                "{},{},{},{},{},{}{},{},{},{},{},{},{},{},{},{},{},{},{}",
                u8::from(self.dashed),
                self.opacity,
                self.style,
                self.angle,
                u8::from(self.border),
                self.ends[0],
                self.ends[1],
                u8::from(self.dotted),
                u8::from(self.halo),
                u8::from(self.elbow),
                u8::from(self.sketch),
                self.id,
                stuck_word(self.stuck[0]),
                stuck_word(self.stuck[1]),
                self.align,
                self.edge,
                shadow_word(self.shadow),
                corners_word(self.corners),
                gradient_word(self.gradient)
            ),
            self.fill,
            self.number,
            points.join(" ")
        )
    }

    /// A shape from a line `write` made, or nothing from any other.
    pub fn read(line: &str) -> Option<Shape> {
        let fields: Vec<&str> = line.splitn(12, '\t').collect();
        let [name, color, stroke, filled, radius, font_px, face, heads, dashed, fill, number, rest] = fields[..] else { return None };
        let (points, text) = rest.split_once('\t')?;
        let tool = TOOL_NAMES.iter().find(|(_, known)| *known == name)?.0;
        let points: Vec<Point> = points.split(' ').map(|point| point.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))).collect::<Option<_>>()?;
        let needed = Shape::new(tool, 0, 1.0, (0.0, 0.0)).points.len();
        if points.len() < needed || points.iter().any(|point: &Point| !point.0.is_finite() || !point.1.is_finite()) {
            return None;
        }
        let mut shape = Shape::new(tool, u32::from_str_radix(color, 16).ok()?, stroke.parse().ok()?, points[0]);
        shape.points = points;
        (shape.filled, shape.radius, shape.font_px, shape.face) = (filled == "1", radius.parse().ok()?, font_px.parse().ok()?, face.parse::<usize>().ok()?.min(crate::text::FACES.len() - 1));
        // With whether it is dashed go how opaque it is and how its
        // text is set, which a file from before either was has not.
        let mut more = dashed.split(',');
        (shape.heads, shape.dashed) = ([heads.starts_with('1'), heads.ends_with('1')], more.next() == Some("1"));
        shape.opacity = more.next().and_then(|opacity| opacity.parse().ok()).unwrap_or(256).min(256);
        shape.style = more.next().and_then(|style| style.parse().ok()).unwrap_or(0);
        shape.angle = more.next().and_then(|angle| angle.parse().ok()).filter(|angle: &f32| angle.is_finite()).unwrap_or(0.0);
        (shape.fill, shape.number) = (u32::from_str_radix(fill, 16).ok()?, number.parse().ok()?);
        // Whether it has a border, which a file from before borders and
        // fills had their own colors has not: a shape filled then was
        // filled in its one color, and had no border.
        match more.next() {
            Some(border) => shape.border = border == "1",
            None if shape.filled && matches!(tool, Tool::Rect | Tool::Ellipse) => (shape.fill, shape.border) = (shape.color, false),
            None => {}
        }
        // And what each head is, dots, a white edge, an elbow and a
        // hand-drawn look, which a file from before them has not.
        if let Some(ends) = more.next() {
            let mut digits = ends.chars().filter_map(|digit| digit.to_digit(10)).map(|digit| (digit as u8).min(HEADS.len() as u8 - 1));
            shape.ends = [digits.next().unwrap_or(FILLED_HEAD), digits.next().unwrap_or(FILLED_HEAD)];
        }
        let mut flag = || more.next() == Some("1");
        (shape.dotted, shape.halo, shape.elbow, shape.sketch) = (flag(), flag(), flag(), flag());
        // And what it is called, and what its ends are stuck to, which a
        // file from before arrows stuck has not: such a shape is given a
        // name of its own.
        if let Some(id) = more.next().and_then(|id| id.parse::<u32>().ok()).filter(|&id| id > 0) {
            shape.id = id;
            seen_id(id);
        }
        shape.stuck = [more.next().and_then(stuck_read), more.next().and_then(stuck_read)];
        shape.align = more.next().and_then(|align| align.parse::<u8>().ok()).unwrap_or(0).min(2);
        // And where its border lies and the shadow it casts, which a file
        // from before either has not.
        shape.edge = more.next().and_then(|edge| edge.parse::<u8>().ok()).unwrap_or(CENTER).min(OUTSIDE);
        shape.shadow = more.next().and_then(shadow_read);
        // And the radius of each corner, when they differ, which a file
        // from before has not: all of them are its one radius then.
        shape.corners = more.next().and_then(corners_read);
        // And what it is filled with besides one color, which a file from
        // before has not: it is filled in its one color.
        shape.gradient = more.next().and_then(gradient_read);
        // The text has its new lines and its backslashes said in two
        // letters each: put back one at a time, in order.
        let mut letters = text.chars();
        while let Some(letter) = letters.next() {
            shape.text.push(match (letter, letter == '\\').1.then(|| letters.next()).flatten() {
                Some('n') => '\n',
                Some(other) => other,
                None => letter,
            });
        }
        Some(shape)
    }

    /// How its letters are drawn: heavier, slanted and faint as it is.
    pub fn look(&self) -> Look {
        Look {
            heavy: if self.style & BOLD != 0 { (self.font_px as i32 / 14).max(1) } else { 0 },
            slant: if self.style & ITALIC != 0 { 0.2 } else { 0.0 },
            opacity: self.opacity,
        }
    }

    /// How the letters of a line at a size are drawn: a heading heavy
    /// whether or not the rest is.
    pub fn look_at(&self, px: u32, heading: bool) -> Look {
        Look { heavy: if self.style & BOLD != 0 || heading { (px as i32 / 14).max(1) } else { 0 }, ..self.look() }
    }

    /// The lines of its text as they are shown: a heading larger, its
    /// hashes left out unless it is being typed into; in capitals if it is
    /// set in them; each after its dot, with bullets.
    pub fn text_lines(&self, text: &mut Text) -> Vec<TextLine> {
        let mut lines = Vec::new();
        let mut top = 0;
        for line in self.text.split('\n') {
            let (px, words, heading) = match heading_of(line) {
                Some((level, rest)) => (((self.font_px as f32 * HEADINGS[level - 1]).round() as u32).max(1), if self.editing { line } else { rest }, true),
                None => (self.font_px, line, false),
            };
            let words = if self.style & UPPER != 0 { words.to_uppercase() } else { words.to_owned() };
            let shown = format!("{}{words}", self.lead());
            text.look = self.look_at(px, heading);
            let width = text.width_in(self.face, &shown, px);
            text.look = Look::default();
            lines.push(TextLine { shown, px, top, width, heading });
            top += Text::line_height(px);
        }
        lines
    }

    /// What each line of its text begins with: a dot, with bullets.
    pub fn lead(&self) -> &'static str {
        if self.tool == Tool::Text && self.style & BULLETS != 0 { "\u{2022} " } else { "" }
    }

    /// The rectangle a zoom is of, and the one it shows it in: from its
    /// third point, as many times larger as its fourth is across from
    /// that, and the same shape as what it is of.
    pub fn zoomed(&self) -> (Rect, Rect) {
        let (a, b) = (self.points[0], self.points[1]);
        let of = Rect::new(a.0.min(b.0).round() as i32, a.1.min(b.1).round() as i32, (a.0 - b.0).abs().round() as i32, (a.1 - b.1).abs().round() as i32);
        let times = ((self.points[3].0 - self.points[2].0) / of.w.max(1) as f32).clamp(1.25, 12.0);
        (of, Rect::new(self.points[2].0.round() as i32, self.points[2].1.round() as i32, (of.w as f32 * times).round() as i32, (of.h as f32 * times).round() as i32))
    }

    /// Puts a zoom's larger picture where it usually goes: to the right
    /// of what it is of, a little above the middle of it, and twice
    /// the size.
    pub fn place_zoom(&mut self) {
        let (of, _) = self.zoomed();
        self.points[2] = ((of.right() + of.w / 2 + 24) as f32, (of.y - of.h / 2) as f32);
        self.points[3] = (self.points[2].0 + (of.w * ZOOM) as f32, self.points[2].1 + (of.h * ZOOM) as f32);
    }

    /// Where a zoom's two handles are: in the middle of its larger
    /// picture, which moves it, and on its rim, down and to the right,
    /// which says how many times larger: on the ring of a round one, at
    /// the corner of a square one.
    pub fn zoom_handles(&self) -> [(i32, i32); 2] {
        let (_, shown) = self.zoomed();
        let middle = shown.center();
        let out = if self.filled { 0.5 } else { std::f32::consts::FRAC_1_SQRT_2 / 2.0 };
        [middle, (middle.0 + (shown.w as f32 * out).round() as i32, middle.1 + (shown.h as f32 * out).round() as i32)]
    }

    /// How many times larger a zoom shows what it is of.
    pub fn zoom_times(&self) -> f32 {
        let (of, shown) = self.zoomed();
        shown.w as f32 / of.w.max(1) as f32
    }

    /// One of a zoom's two handles taken to a point: the one in its
    /// middle takes the larger picture there, as large as it was; the
    /// one on its rim makes it as many times larger as puts the rim
    /// under the point, its middle staying where it is.
    pub fn zoom_to(&mut self, handle: usize, to: Point) {
        let (of, shown) = self.zoomed();
        // Its middle from its points, not from the pixels it was rounded
        // to, or it would creep as it was made larger.
        let (from, far) = (self.points[2], self.points[3]);
        let (mut middle, mut size) = (((from.0 + far.0) / 2.0, (from.1 + far.1) / 2.0), (shown.w as f32, shown.h as f32));
        if handle == 2 {
            middle = to;
        } else {
            let out = if self.filled { 0.5 } else { std::f32::consts::FRAC_1_SQRT_2 / 2.0 };
            let across = (to.0 - middle.0).abs() / out / of.w.max(1) as f32;
            let down = (to.1 - middle.1).abs() / out / of.h.max(1) as f32;
            let times = across.max(down).clamp(1.25, 12.0);
            size = (of.w as f32 * times, of.h as f32 * times);
        }
        self.points[2] = (middle.0 - size.0 / 2.0, middle.1 - size.1 / 2.0);
        self.points[3] = (middle.0 + size.0 / 2.0, middle.1 + size.1 / 2.0);
    }

    /// The face its words are drawn in: its own, but that some looks of a
    /// badge keep to one of their own.
    pub fn words_face(&self) -> usize {
        if self.tool == Tool::Badge { badge_face(self.number as usize, self.face) } else { self.face }
    }

    /// Where the first letter of a shape with words on it goes.
    pub fn text_origin(&self) -> (i32, i32) {
        let (x, y) = (self.points[0].0.round() as i32, self.points[0].1.round() as i32);
        match self.tool {
            // After the dot at the front of a badge.
            // A pin's words are in its middle, which takes their width to
            // know: put near enough for the caret.
            Tool::Badge => {
                let (lead, _, _, top) = badge_layout(self.number as usize, 0, self.font_px as i32, Text::line_height(self.font_px));
                (x + if self.number == 24 { self.font_px as i32 / 2 } else { lead }, y + top)
            }
            _ => (x, y),
        }
    }

    /// The radius of each of its corners: top left, top right, bottom
    /// right, bottom left.
    pub fn radii(&self) -> [f32; 4] {
        self.corners.unwrap_or([self.radius; 4])
    }

    /// Its corners made as round as these: one radius, if they are all
    /// the same, or each its own. Its `radius` is the first of them.
    pub fn set_radii(&mut self, radii: [f32; 4]) {
        let radii = radii.map(|radius| radius.max(0.0));
        self.radius = radii[0];
        self.corners = radii.iter().any(|&radius| radius != radii[0]).then_some(radii);
        self.touch();
    }

    /// To be called when anything about its outline has changed.
    pub fn touch(&mut self) {
        *self.mask.borrow_mut() = None;
        *self.inside.borrow_mut() = None;
    }

    /// Whether it is a rectangle or an oval with both a border and a
    /// fill, which are drawn one over the other in their own colors.
    fn bordered_fill(&self) -> bool {
        matches!(self.tool, Tool::Rect | Tool::Ellipse) && self.filled && self.border
    }

    /// Whether its fill is drawn apart from its line: with a border too,
    /// or drawn by hand, whose line is strokes and not an outline.
    fn fill_apart(&self) -> bool {
        matches!(self.tool, Tool::Rect | Tool::Ellipse) && self.filled && (self.border || self.sketch)
    }

    /// Whether it is drawn by hand: what can be, a box, an oval, a line
    /// or an arrow, when it is said to be.
    fn sketched(&self) -> bool {
        self.sketch && matches!(self.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow)
    }

    /// How wide the edge of white round a line is, when it has one.
    fn halo_width(&self) -> f32 {
        if self.halo && matches!(self.tool, Tool::Line | Tool::Arrow) { self.stroke * 0.8 + 2.0 } else { 0.0 }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        for point in &mut self.points {
            *point = (point.0 + dx, point.1 + dy);
        }
        self.touch();
    }

    /// The width a pen or a marker draws at.
    fn line_width(&self) -> f32 {
        if self.tool == Tool::Marker { self.stroke * 3.0 + 6.0 } else { self.stroke }
    }

    /// The radius of a counter's disc.
    fn disc(&self) -> f32 {
        self.font_px as f32 * 0.85
    }

    /// Too small to have been meant: a click where a drag was wanted.
    pub fn is_slight(&self) -> bool {
        match self.tool {
            Tool::Arrow | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Blur | Tool::Spotlight => {
                let (a, b) = (self.points[0], self.points[1]);
                (a.0 - b.0).hypot(a.1 - b.1) < 3.0
            }
            Tool::Text | Tool::Badge => self.text.trim().is_empty(),
            Tool::Zoom => {
                let (of, _) = self.zoomed();
                of.w < 4 || of.h < 4
            }
            _ => false,
        }
    }

    /// The rectangle between its two points.
    pub fn frame(&self) -> (f32, f32, f32, f32) {
        let (a, b) = (self.points[0], self.points[self.far()]);
        (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs())
    }

    pub fn frame_rect(&self) -> Rect {
        let (x, y, w, h) = self.frame();
        Rect::new(x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32)
    }

    /// Whether it turns by an angle of its own, rather than by its points.
    pub fn turnable(&self) -> bool {
        matches!(self.tool, Tool::Rect | Tool::Ellipse | Tool::Text | Tool::Badge)
    }

    /// Whether it can be turned at all: not a mosaic, a spotlight, a zoom
    /// or a counter, which are square to the picture or round.
    pub fn turns(&self) -> bool {
        self.turnable() || matches!(self.tool, Tool::Pen | Tool::Marker | Tool::Arrow | Tool::Line)
    }

    /// What it turns about: the middle of it as it is unturned.
    pub fn centre(&self, text: &mut Text) -> Point {
        match self.tool {
            Tool::Text | Tool::Badge => {
                let place = self.local_bounds(text);
                (place.x as f32 + place.w as f32 / 2.0, place.y as f32 + place.h as f32 / 2.0)
            }
            _ => self.centre_of_frame(),
        }
    }

    fn centre_of_frame(&self) -> Point {
        let (x, y, w, h) = self.frame();
        (x + w / 2.0, y + h / 2.0)
    }

    /// A point of the picture as it would be if it were not turned.
    pub fn unturn(&self, p: Point, text: &mut Text) -> Point {
        if self.angle == 0.0 {
            return p;
        }
        turn_point(p, self.centre(text), -self.angle)
    }

    /// A point of it as unturned, where it is with it turned.
    pub fn returned(&self, p: Point, text: &mut Text) -> Point {
        if self.angle == 0.0 {
            return p;
        }
        turn_point(p, self.centre(text), self.angle)
    }

    /// Turned by `by` about `about`: by its angle, its middle taken round
    /// with it; or by its points.
    pub fn turn(&mut self, by: f32, about: Point, text: &mut Text) {
        if self.turnable() {
            let from = self.centre(text);
            let to = turn_point(from, about, by);
            self.translate(to.0 - from.0, to.1 - from.1);
            self.angle = (self.angle + by).rem_euclid(std::f32::consts::TAU);
            // Near enough square is square, after a turn round and back.
            if self.angle.min(std::f32::consts::TAU - self.angle) < 1e-4 {
                self.angle = 0.0;
            }
        } else {
            for point in &mut self.points {
                *point = turn_point(*point, about, by);
            }
        }
        self.touch();
    }

    /// The rectangle that holds all of it, turned as it is.
    pub fn bounds(&self, text: &mut Text) -> Rect {
        let place = self.local_bounds(text);
        if self.angle == 0.0 || !self.turnable() {
            return place;
        }
        turned_around(place, self.centre(text), self.angle)
    }

    /// The rectangle that holds all of it as if it were not turned.
    pub fn local_bounds(&self, text: &mut Text) -> Rect {
        match self.tool {
            Tool::Text => {
                let lines = self.text_lines(text);
                let width = lines.iter().map(|line| line.width).max().unwrap_or(0);
                let height = lines.last().map_or(Text::line_height(self.font_px), |line| line.top + Text::line_height(line.px));
                Rect::new(self.points[0].0.round() as i32, self.points[0].1.round() as i32, width.max(self.font_px as i32 / 4), height)
            }
            Tool::Blur | Tool::Spotlight => self.frame_rect(),
            // A badge is as wide as its words, with a dot before them
            // and room at either end.
            Tool::Badge => {
                let style = self.number as usize;
                let words = text.width_in(badge_face(style, self.face), &self.text, self.font_px);
                let (_, width, height, _) = badge_layout(style, words, self.font_px as i32, Text::line_height(self.font_px));
                Rect::new(self.points[0].0.round() as i32, self.points[0].1.round() as i32, width, height)
            }
            Tool::Zoom => {
                let (of, shown) = self.zoomed();
                let (x, y) = (of.x.min(shown.x), of.y.min(shown.y));
                Rect::new(x, y, of.right().max(shown.right()) - x, of.bottom().max(shown.bottom()) - y)
            }
            _ => self.extent(),
        }
    }

    /// The rectangle its outline is worked out over.
    fn extent(&self) -> Rect {
        let margin = match self.tool {
            Tool::Arrow | Tool::Line if self.heads != [false, false] => self.head().0 + self.stroke + self.halo_width() + if self.sketch { 4.0 } else { 0.0 },
            Tool::Arrow | Tool::Line => self.stroke / 2.0 + self.halo_width() + if self.sketch { 4.0 } else { 0.0 },
            Tool::Rect | Tool::Ellipse if self.sketch => self.stroke + 8.0,
            Tool::Rect | Tool::Ellipse if self.edge == OUTSIDE => self.stroke,
            Tool::Counter => self.disc(),
            Tool::Pen | Tool::Marker => self.line_width() / 2.0,
            _ => self.stroke / 2.0,
        } + 2.0;
        let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        // A bent line goes where its curve goes, which may be a little
        // past the point it bends through.
        let curve = self.curve();
        for &(x, y) in self.points.iter().chain(curve.iter().flatten()) {
            (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
        }
        let (x, y) = ((left - margin).floor() as i32, (top - margin).floor() as i32);
        Rect::new(x, y, (right + margin).ceil() as i32 - x, (bottom + margin).ceil() as i32 - y)
    }

    /// The length of an arrow's head and half its width.
    fn head(&self) -> (f32, f32) {
        let (a, b) = (self.points[0], self.points[1]);
        // No longer than most of the line; or with a head at each end,
        // than a little under half of it, so that they do not meet.
        let most = if self.heads == [true, true] { 0.45 } else { 0.7 };
        let length = (self.stroke * 3.2 + 9.0).min((a.0 - b.0).hypot(a.1 - b.1) * most).max(1.0);
        (length, length * 0.46)
    }

    /// What its outline is worked out from.
    fn outline(&self) -> Outline {
        let (a, b) = (self.points[0], self.points[self.far()]);
        let (curve, head) = (self.curve(), self.head());
        let back = |points: &mut dyn Iterator<Item = Point>| {
            let (mut last, mut gone) = (None::<Point>, 0.0);
            for point in points {
                gone += last.map_or(0.0, |last| (point.0 - last.0).hypot(point.1 - last.1));
                last = Some(point);
                if gone >= head.0 {
                    break;
                }
            }
            last.unwrap_or(a)
        };
        let backs = curve.map_or([b, a], |curve| [back(&mut curve.into_iter()), back(&mut curve.into_iter().rev())]);
        let (x, y, w, h) = self.frame();
        let angle = if matches!(self.tool, Tool::Rect | Tool::Ellipse) { self.angle } else { 0.0 };
        // With a border as well as a fill, the outline is the border's:
        // what is inside it has a mask of its own.
        let filled = self.filled && !self.bordered_fill();
        Outline { tool: self.tool, a, b, stroke: self.stroke, filled, radii: self.radii(), head, heads: self.heads, ends: self.ends, dashed: self.dashed && !self.dotted, dotted: self.dotted, disc: self.disc(), curve, backs, angle, about: (x + w / 2.0, y + h / 2.0), edge: self.edge }
    }

    /// Which of its points is the far one of the two it is between: the
    /// last, but that a line or an arrow may have a third it bends by.
    fn far(&self) -> usize {
        if matches!(self.tool, Tool::Line | Tool::Arrow) { 1 } else { self.points.len() - 1 }
    }

    /// The point a line or an arrow bends through, half way along it,
    /// if it bends.
    pub fn bend(&self) -> Option<Point> {
        self.points.get(2).copied().filter(|_| matches!(self.tool, Tool::Line | Tool::Arrow))
    }

    /// Where the dot is that a line or an arrow is bent by: on its
    /// bend, or half way along it while it is straight.
    /// The way an elbow goes, as connectors in a diagram go: across from
    /// its first end, round a rounded corner, and down to its last; as
    /// points evenly along it, which is what a bent line is drawn by.
    fn elbow_path(&self) -> [Point; BENDS + 1] {
        let (a, b) = (self.points[0], self.points[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let radius = 16f32.min(dx.abs() / 2.0).min(dy.abs() / 2.0);
        let (sx, sy) = (dx.signum(), dy.signum());
        let first = dx.abs() - radius;
        // Most of the points go round the corner, which is where they are
        // needed: four along each straight part, the rest the curve.
        let (along, round) = (4, BENDS - 8);
        let (cx, cy) = (a.0 + sx * first, a.1 + sy * radius);
        let mut points = [a; BENDS + 1];
        for (step, point) in points.iter_mut().enumerate() {
            *point = if step <= along {
                (a.0 + sx * first * step as f32 / along as f32, a.1)
            } else if step < along + round {
                let angle = std::f32::consts::FRAC_PI_2 * (step - along) as f32 / round as f32;
                (cx + sx * radius * angle.sin(), cy - sy * radius * angle.cos())
            } else {
                let t = (step - along - round) as f32 / (BENDS - along - round) as f32;
                (b.0, a.1 + sy * (radius + (dy.abs() - radius) * t))
            };
        }
        points
    }

    pub fn middle(&self) -> Point {
        let (a, b) = (self.points[0], self.points[1]);
        self.bend().unwrap_or(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))
    }

    /// Bends a line or an arrow through a point; or, taken back to
    /// within a few pixels of where it would be straight, straightens
    /// it again.
    pub fn bend_to(&mut self, to: Point) {
        let (a, b) = (self.points[0], self.points[1]);
        self.points.truncate(2);
        if segment(to, a, b) > 4.0 {
            self.points.push(to);
        }
        self.touch();
    }

    /// A bent line as the short straight ones it is drawn as: a curve
    /// from one end to the other, through its bend half way.
    fn curve(&self) -> Option<[Point; BENDS + 1]> {
        if self.elbow && matches!(self.tool, Tool::Line | Tool::Arrow) && self.points.len() >= 2 {
            return Some(self.elbow_path());
        }
        // Asked of every shape, which may have but one point: a pen's
        // as it is begun. Only a line or an arrow that bends has one.
        let through = self.bend()?;
        let (a, b) = (self.points[0], self.points[1]);
        // What the curve is pulled towards, which is twice as far from
        // the straight line as the curve gets.
        let pull = (2.0 * through.0 - (a.0 + b.0) / 2.0, 2.0 * through.1 - (a.1 + b.1) / 2.0);
        let mut points = [a; BENDS + 1];
        for (step, point) in points.iter_mut().enumerate() {
            let (t, u) = (step as f32 / BENDS as f32, 1.0 - step as f32 / BENDS as f32);
            *point = (u * u * a.0 + 2.0 * u * t * pull.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * pull.1 + t * t * b.1);
        }
        Some(points)
    }
}

/// The next name a shape or a picture is given: one no other in this
/// run has, nor any read from a file.
static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

pub fn fresh_id() -> u32 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A name read from a file, which none given after is to be.
pub fn seen_id(id: u32) {
    NEXT_ID.fetch_max(id + 1, std::sync::atomic::Ordering::Relaxed);
}

/// Where on what it is stuck to an end stuck as a whole is.
pub const WHOLE: f32 = 9.0;

fn stuck_word(stuck: Option<(u32, f32, f32)>) -> String {
    stuck.map_or("-".to_owned(), |(id, u, v)| format!("{id}:{u}:{v}"))
}

fn stuck_read(word: &str) -> Option<(u32, f32, f32)> {
    let mut parts = word.split(':');
    let (id, u, v) = (parts.next()?.parse().ok()?, parts.next()?.parse::<f32>().ok()?, parts.next()?.parse::<f32>().ok()?);
    (u.is_finite() && v.is_finite()).then_some((id, u, v))
}

/// Something an end of an arrow can be stuck to: a box of a size about a
/// middle, turned so far, and whether it is round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub id: u32,
    pub middle: Point,
    pub half: (f32, f32),
    pub angle: f32,
    pub round: bool,
}

impl Anchor {
    /// A point of it, from across and down it unturned, each -1 to 1.
    fn at(&self, u: f32, v: f32) -> Point {
        turn_point((self.middle.0 + u * self.half.0, self.middle.1 + v * self.half.1), self.middle, self.angle)
    }

    /// Where a line from its middle to a point leaves it, a little out
    /// from its edge.
    fn edge_towards(&self, to: Point) -> Point {
        let (lx, ly) = turn_point(to, self.middle, -self.angle);
        let (dx, dy) = (lx - self.middle.0, ly - self.middle.1);
        let long = (dx * dx + dy * dy).sqrt();
        if long < 1e-3 {
            return self.middle;
        }
        let (hw, hh) = (self.half.0.max(1e-3), self.half.1.max(1e-3));
        let reach = match self.round {
            true => 1.0 / ((dx / hw).powi(2) + (dy / hh).powi(2)).sqrt(),
            false => (hw / dx.abs().max(1e-6)).min(hh / dy.abs().max(1e-6)),
        };
        // Beyond its edge by a gap, unless the point is inside it.
        let out = (reach + STUCK_GAP / long).min(1.0);
        turn_point((self.middle.0 + dx * out, self.middle.1 + dy * out), self.middle, self.angle)
    }
}

/// How far short of what it is stuck to as a whole an arrow stops.
const STUCK_GAP: f32 = 6.0;

impl Shape {
    /// What an arrow can be stuck to of it: a box, an oval or text.
    pub fn anchor(&self, text: &mut Text) -> Option<Anchor> {
        let (x, y, w, h) = match self.tool {
            Tool::Rect | Tool::Ellipse => self.frame(),
            Tool::Text | Tool::Badge => {
                let place = self.local_bounds(text);
                (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
            }
            _ => return None,
        };
        Some(Anchor { id: self.id, middle: (x + w / 2.0, y + h / 2.0), half: (w / 2.0, h / 2.0), angle: self.angle, round: self.tool == Tool::Ellipse })
    }

    /// A copy of it, called something of its own, with its ends stuck to
    /// nothing.
    pub fn copied(&self) -> Shape {
        Shape { id: fresh_id(), stuck: [None; 2], ..self.clone() }
    }

    /// Whether it is a line that can have its ends stuck.
    pub fn sticks(&self) -> bool {
        matches!(self.tool, Tool::Line | Tool::Arrow)
    }

    /// An end of a line put down at a point: stuck to the topmost of the
    /// anchors that the point is on or near, but itself; as a whole, if
    /// the point is well inside it, or at the point, kept to its edge;
    /// or to nothing.
    pub fn stick(&mut self, end: usize, at: Point, anchors: &[Anchor], reach: f32) {
        self.stuck[end] = None;
        if !self.sticks() {
            return;
        }
        for anchor in anchors.iter().rev().filter(|anchor| anchor.id != self.id) {
            let (lx, ly) = turn_point(at, anchor.middle, -anchor.angle);
            let (dx, dy) = (lx - anchor.middle.0, ly - anchor.middle.1);
            let (hw, hh) = (anchor.half.0.max(1.0), anchor.half.1.max(1.0));
            if dx.abs() > hw + reach || dy.abs() > hh + reach {
                continue;
            }
            let spot = |(x, y): Point| {
                let (lx, ly) = turn_point((x, y), anchor.middle, -anchor.angle);
                (anchor.id, ((lx - anchor.middle.0) / hw).clamp(-1.0, 1.0), ((ly - anchor.middle.1) / hh).clamp(-1.0, 1.0))
            };
            // Both ends on the same thing is a line drawn on it, as an
            // arrow is drawn on a picture: each end kept at its spot, to
            // go with it, and not pointed at its middle, which would
            // leave no line at all.
            let other = 1 - end.min(1);
            if self.stuck[other].is_some_and(|(id, ..)| id == anchor.id) {
                self.stuck[end] = Some(spot(at));
                self.stuck[other] = Some(spot(self.points[other]));
                break;
            }
            self.stuck[end] = Some(match dx.abs() < hw * 0.6 && dy.abs() < hh * 0.6 {
                true => (anchor.id, WHOLE, WHOLE),
                false => spot(at),
            });
            break;
        }
    }
}

/// The ends of the lines among some shapes moved to what they are stuck
/// to, where it now is; and an end stuck to something no longer there
/// let go. Says whether any moved.
pub fn follow<'a>(lines: impl Iterator<Item = &'a mut Shape>, anchors: &[Anchor]) -> bool {
    let find = |id: u32| anchors.iter().find(|anchor| anchor.id == id);
    let mut moved = false;
    for shape in lines.filter(|shape| shape.sticks() && shape.stuck.iter().any(Option::is_some)) {
        let mut this = false;
        for _ in 0..2 {
            for end in 0..2 {
                let Some((id, u, v)) = shape.stuck[end] else { continue };
                let Some(anchor) = find(id) else {
                    shape.stuck[end] = None;
                    continue;
                };
                // The ends are the first two points; a third is where the
                // line is bent through, which an end stuck as a whole
                // points itself towards.
                let (at, near) = (end, if shape.points.len() > 2 && !shape.elbow { 2 } else { 1 - end });
                let to = match u == WHOLE {
                    // Towards the other end, or what that is stuck to as a
                    // whole; or towards a bend, where there is one.
                    true => {
                        let other = match (shape.stuck[1 - end], shape.points.len()) {
                            (Some((other, whole, _)), 2) if whole == WHOLE => find(other).map_or(shape.points[near], |anchor| anchor.middle),
                            _ => shape.points[near],
                        };
                        anchor.edge_towards(other)
                    }
                    false => anchor.at(u, v),
                };
                if (to.0 - shape.points[at].0).abs() + (to.1 - shape.points[at].1).abs() > 0.01 {
                    shape.points[at] = to;
                    this = true;
                }
            }
        }
        if this {
            moved = true;
            shape.touch();
        }
    }
    moved
}

/// `p` turned by `by` radians, clockwise on the screen, about `about`.
pub fn turn_point(p: Point, about: Point, by: f32) -> Point {
    let (sin, cos) = by.sin_cos();
    let (dx, dy) = (p.0 - about.0, p.1 - about.1);
    (about.0 + dx * cos - dy * sin, about.1 + dx * sin + dy * cos)
}

/// The rectangle that holds another turned by `by` about `about`.
pub fn turned_around(place: Rect, about: Point, by: f32) -> Rect {
    let corners = [(place.x, place.y), (place.right(), place.y), (place.right(), place.bottom()), (place.x, place.bottom())].map(|(x, y)| turn_point((x as f32, y as f32), about, by));
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in corners {
        (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
    }
    let (x, y) = (left.floor() as i32, top.floor() as i32);
    Rect::new(x, y, right.ceil() as i32 - x, bottom.ceil() as i32 - y)
}

/// How many straight lines a bent one is drawn as.
const BENDS: usize = 24;

/// The few numbers a shape's outline is worked out from, apart from the
/// shape, so that every core can be at it.
#[derive(Clone, Copy)]
struct Outline {
    tool: Tool,
    a: Point,
    b: Point,
    stroke: f32,
    filled: bool,
    /// The radius of each corner of a box: top left, top right, bottom
    /// right, bottom left.
    radii: [f32; 4],
    head: (f32, f32),
    heads: [bool; 2],
    ends: [u8; 2],
    dashed: bool,
    dotted: bool,
    disc: f32,
    curve: Option<[Point; BENDS + 1]>,
    /// Where a bent line is a head's length back from either end: the
    /// way it is going there is the way the head at that end points.
    backs: [Point; 2],
    /// How far a box or an oval is turned, and about where.
    angle: f32,
    about: Point,
    /// Where a box's or an oval's border lies on its edge.
    edge: u8,
}

impl Outline {
    /// How far a point is from a line, which if it is dashed is the
    /// dashes of it: each a little longer than the gap after it.
    fn shaft(&self, p: Point, from: Point, to: Point) -> f32 {
        let half = self.stroke / 2.0;
        let solid = segment(p, from, to) - half;
        let length = (to.0 - from.0).hypot(to.1 - from.1);
        if !(self.dashed || self.dotted) || length < 1.0 {
            return solid;
        }
        let along = ((p.0 - from.0) * (to.0 - from.0) + (p.1 - from.1) * (to.1 - from.1)) / length;
        if self.dotted {
            let across = ((p.0 - from.0) * (to.1 - from.1) - (p.1 - from.1) * (to.0 - from.0)).abs() / length;
            return self.dot(across, along, 0.0, length);
        }
        let (dash, gap) = (self.stroke * 2.5 + 4.0, self.stroke * 2.0 + 3.0);
        let into = along.rem_euclid(dash + gap);
        // Inside a gap, it is as far from the line as from the gap's
        // nearer end; inside a dash, the other way about.
        let outside = if into > dash { (into - dash).min(dash + gap - into) } else { -into.min(dash - into) };
        solid.max(outside)
    }

    /// The same of a bent line, less `trim` of it at either end, where
    /// an arrow's head is: the nearest of the short lines it is made of,
    /// and its dashes counted along all of them.
    fn bent(&self, p: Point, curve: &[Point; BENDS + 1], trim: (f32, f32)) -> f32 {
        let half = self.stroke / 2.0;
        let lengths = curve.windows(2).map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1));
        let whole: f32 = lengths.clone().sum();
        let (first, last) = (trim.0.min(whole / 2.0), whole - trim.1.min(whole / 2.0));
        let (dash, gap) = (self.stroke * 2.5 + 4.0, self.stroke * 2.0 + 3.0);
        let (mut nearest, mut gone) = (f32::MAX, 0.0f32);
        for (pair, length) in curve.windows(2).zip(lengths) {
            let (begins, ends) = (gone, gone + length);
            gone = ends;
            if ends <= first || begins >= last || length <= 0.0 {
                continue;
            }
            // As much of this one as is not under a head.
            let at = |along: f32| (pair[0].0 + (pair[1].0 - pair[0].0) * along / length, pair[0].1 + (pair[1].1 - pair[0].1) * along / length);
            let (from, to) = (at((first - begins).max(0.0)), at((last - begins).min(length)));
            let mut far = segment(p, from, to) - half;
            let run = (to.0 - from.0).hypot(to.1 - from.1).max(1e-3);
            if self.dotted {
                let along = ((p.0 - from.0) * (to.0 - from.0) + (p.1 - from.1) * (to.1 - from.1)) / run;
                let across = ((p.0 - from.0) * (to.1 - from.1) - (p.1 - from.1) * (to.0 - from.0)).abs() / run;
                far = self.dot(across, along, begins.max(first) - first, run);
            } else if self.dashed {
                let along = (((p.0 - from.0) * (to.0 - from.0) + (p.1 - from.1) * (to.1 - from.1)) / run).clamp(0.0, run);
                let into = (begins.max(first) - first + along).rem_euclid(dash + gap);
                far = far.max(if into > dash { (into - dash).min(dash + gap - into) } else { -into.min(dash - into) });
            }
            nearest = nearest.min(far);
        }
        nearest
    }

    /// How far a point is from the nearest of the dots a dotted line is
    /// drawn as: `across` the line from it and `along` a stretch of it
    /// `length` long, which begins `offset` along the whole line, so that
    /// the dots keep their spacing from one stretch to the next.
    fn dot(&self, across: f32, along: f32, offset: f32, length: f32) -> f32 {
        let gap = self.stroke * 2.2 + 2.0;
        let nearest = (((along + offset) / gap).round() * gap - offset).clamp(0.0, length);
        across.hypot(along - nearest) - self.stroke / 2.0
    }

    /// How far a point is from the head at one end of a line, of the
    /// kind it is: its tip, and the way the line goes as it comes to it.
    /// And how much of the line to leave out at that end, under it.
    fn tip(&self, p: Point, style: u8, tip: Point, way: (f32, f32)) -> (f32, f32) {
        let (head, wide) = self.head;
        let half = self.stroke / 2.0;
        let (dx, dy) = way;
        let back = |by: f32| (tip.0 - dx * by, tip.1 - dy * by);
        let side = |at: Point, by: f32| (at.0 - dy * by, at.1 + dx * by);
        let base = back(head);
        match style {
            OPEN_HEAD => (segment(p, tip, side(base, wide)).min(segment(p, tip, side(base, -wide))) - half, half),
            HOLLOW_HEAD => (triangle(p, tip, side(base, wide), side(base, -wide)).abs() - half, head),
            DOT_HEAD => ((p.0 - tip.0).hypot(p.1 - tip.1) - wide * 0.75, 0.0),
            RING_HEAD => {
                let (radius, middle) = (wide * 0.75, back(wide * 0.75));
                (((p.0 - middle.0).hypot(p.1 - middle.1) - radius).abs() - half, 2.0 * radius)
            }
            DIAMOND_HEAD => {
                let (middle, far) = (back(head * 0.55), back(head * 1.1));
                (triangle(p, tip, side(middle, wide * 0.8), far).min(triangle(p, tip, far, side(middle, -wide * 0.8))) - 0.6, head)
            }
            BAR_HEAD => (segment(p, side(tip, wide), side(tip, -wide)) - half, 0.0),
            // A head at an end: its point there, its barbs back along the
            // line. The line stops inside it, where its round end would
            // otherwise blunt the point.
            _ => (triangle(p, tip, side(base, wide), side(base, -wide)) - 0.6, head * 0.8),
        }
    }
}

impl Outline {
    /// How far a point is from the outline: negative inside, and nought
    /// on the edge. Not for pen strokes, which are too many lines to ask
    /// of every pixel.
    fn distance(&self, x: f32, y: f32) -> f32 {
        // Turned, the point is taken round the other way, onto it unturned.
        let (x, y) = if self.angle != 0.0 { turn_point((x, y), self.about, -self.angle) } else { (x, y) };
        let (a, b) = (self.a, self.b);
        let half = self.stroke / 2.0;
        match self.tool {
            Tool::Line | Tool::Arrow => {
                let p = (x, y);
                // The way the line goes as it comes to each end: bent,
                // from a head's length back along it.
                let way = |tip: Point, from: Point| {
                    let length = (tip.0 - from.0).hypot(tip.1 - from.1).max(1e-3);
                    ((tip.0 - from.0) / length, (tip.1 - from.1) / length)
                };
                let (to_b, to_a) = match self.curve {
                    Some(_) => (way(b, self.backs[1]), way(a, self.backs[0])),
                    None => (way(b, a), way(a, b)),
                };
                let length = (b.0 - a.0).hypot(b.1 - a.1);
                if self.curve.is_none() && length < 0.5 {
                    return self.shaft(p, a, b);
                }
                let (mut nearest, mut trim) = (f32::MAX, (0.0, 0.0));
                if self.heads[1] {
                    let (far, cut) = self.tip(p, self.ends[1], b, to_b);
                    (nearest, trim.1) = (nearest.min(far), cut);
                }
                if self.heads[0] {
                    let (far, cut) = self.tip(p, self.ends[0], a, to_a);
                    (nearest, trim.0) = (nearest.min(far), cut);
                }
                match &self.curve {
                    Some(curve) => nearest.min(self.bent(p, curve, trim)),
                    None => {
                        let (dx, dy) = to_b;
                        nearest.min(self.shaft(p, (a.0 + dx * trim.0, a.1 + dy * trim.0), (b.0 - dx * trim.1, b.1 - dy * trim.1)))
                    }
                }
            }
            Tool::Rect | Tool::Ellipse => {
                let (left, top, w, h) = (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs());
                let (px, py, hw, hh) = (x - left - w / 2.0, y - top - h / 2.0, w / 2.0, h / 2.0);
                // Each corner no rounder than half the shorter side.
                let radii = self.radii.map(|radius| radius.min(hw).min(hh).max(0.0));
                let inside = match self.tool {
                    Tool::Rect => round_box(px, py, hw, hh, corner_of(px, py, radii)),
                    _ => ellipse(px, py, hw, hh),
                };
                if self.filled {
                    return inside - half;
                }
                // The border moved in or out by half its width, to lie
                // inside the edge or outside it.
                let band = match self.edge {
                    INSIDE => inside + half,
                    OUTSIDE => inside - half,
                    _ => inside,
                };
                let solid = band.abs() - half;
                if !(self.dashed || self.dotted) {
                    return solid;
                }
                // Dashes and dots counted along the edge, spaced to go
                // round it a whole number of times.
                let (along, whole) = match self.tool {
                    Tool::Rect => perimeter_along(px, py, hw, hh, radii),
                    _ => {
                        let turn = (py / hh.max(0.5)).atan2(px / hw.max(0.5)) + std::f32::consts::PI;
                        (turn * (hw + hh) / 2.0, std::f32::consts::TAU * (hw + hh) / 2.0)
                    }
                };
                if self.dotted {
                    let gap = self.stroke * 2.2 + 2.0;
                    let gap = whole / (whole / gap).round().max(1.0);
                    let nearest = (along / gap).round() * gap;
                    return band.abs().hypot(along - nearest) - half;
                }
                let period = self.stroke * 4.5 + 7.0;
                let period = whole / (whole / period).round().max(1.0);
                let dash = period * 0.55;
                let into = along.rem_euclid(period);
                let outside = if into > dash { (into - dash).min(period - into) } else { -into.min(dash - into) };
                solid.max(outside)
            }
            Tool::Counter => {
                let radius = self.disc;
                let disc = (x - a.0).hypot(y - a.1) - radius;
                let length = (b.0 - a.0).hypot(b.1 - a.1);
                if length < radius * 1.4 {
                    return disc;
                }
                // A tail from the disc to what it points at.
                let (dx, dy) = ((b.0 - a.0) / length, (b.1 - a.1) / length);
                let wide = radius * 0.55;
                disc.min(triangle((x, y), b, (a.0 - dy * wide, a.1 + dx * wide), (a.0 + dy * wide, a.1 - dx * wide)) - 0.6)
            }
            _ => f32::MAX,
        }
    }
}

impl Shape {
    /// How much of each pixel it covers, worked out the first time it is
    /// asked for and kept until the shape changes.
    fn mask(&self) -> Rc<Mask> {
        if let Some(mask) = self.mask.borrow().as_ref() {
            return mask.clone();
        }
        // A box or an oval turned covers what its corners turned do.
        let bounds = match self.tool {
            Tool::Rect | Tool::Ellipse if self.angle != 0.0 => turned_around(self.extent(), self.centre_of_frame(), self.angle),
            _ => self.extent(),
        };
        let mask = match self.tool {
            _ if self.sketched() => self.sketch_mask(bounds),
            Tool::Pen | Tool::Marker => {
                let mut mask = Mask { bounds, alpha: vec![0; bounds.w as usize * bounds.h as usize] };
                let half = self.line_width() / 2.0;
                if self.points.len() == 1 {
                    stamp(&mut mask, self.points[0], self.points[0], half);
                }
                for pair in self.points.windows(2) {
                    stamp(&mut mask, pair[0], pair[1], half);
                }
                mask
            }
            _ => {
                let outline = self.outline();
                covered(bounds, move |x, y| outline.distance(x, y))
            }
        };
        let mask = Rc::new(mask);
        *self.mask.borrow_mut() = Some(mask.clone());
        mask
    }

    /// The edge of white round a line that has one: its outline, grown
    /// all round. Kept as the inside of a box is, which a line has none of.
    fn halo_mask(&self) -> Rc<Mask> {
        if let Some(mask) = self.inside.borrow().as_ref() {
            return mask.clone();
        }
        let grow = self.halo_width();
        let mask = match self.sketched() {
            // Drawn by hand: its strokes, each made wider.
            true => {
                let mut wide = self.clone();
                (wide.stroke, wide.halo) = (self.stroke + 2.0 * grow, false);
                wide.sketch_mask(self.extent())
            }
            false => {
                let outline = self.outline();
                covered(self.extent(), move |x, y| outline.distance(x, y) - grow)
            }
        };
        let mask = Rc::new(mask);
        *self.inside.borrow_mut() = Some(mask.clone());
        mask
    }

    /// The strokes of a box, an oval, a line or an arrow drawn by hand:
    /// each line a little bowed, going a little past its corner, and gone
    /// over a second time a little off the first. Uneven the same way
    /// every time it is drawn, by a seed taken from where it is.
    fn sketch_mask(&self, bounds: Rect) -> Mask {
        let mut mask = Mask { bounds, alpha: vec![0; bounds.w as usize * bounds.h as usize] };
        let mut seed = self.points.iter().take(2).fold(0x9E37_79B9u32, |seed, point| seed ^ (point.0.to_bits().rotate_left(7) ^ point.1.to_bits().rotate_left(19)).wrapping_mul(0x85EB_CA6B));
        let mut wobble = move |amount: f32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * amount
        };
        let reach = (self.stroke * 1.2 + 3.0).min(10.0);
        // A stroke from one point to another, bowed out a little at its
        // middle, as a list of points close enough to be drawn straight.
        let stroke = |from: Point, to: Point, wobble: &mut dyn FnMut(f32) -> f32| -> Vec<Point> {
            let length = (to.0 - from.0).hypot(to.1 - from.1).max(1.0);
            let (nx, ny) = (-(to.1 - from.1) / length, (to.0 - from.0) / length);
            let bow = wobble((length * 0.015).min(reach * 0.6));
            (0..=12)
                .map(|step| {
                    let t = step as f32 / 12.0;
                    let lift = bow * 4.0 * t * (1.0 - t);
                    (from.0 + (to.0 - from.0) * t + nx * lift, from.1 + (to.1 - from.1) * t + ny * lift)
                })
                .collect()
        };
        let mut strokes: Vec<(Vec<Point>, f32)> = Vec::new();
        let half = self.stroke / 2.0;
        let (left, top, width, height) = self.frame();
        for pass in 0..2 {
            let thin = if pass == 0 { half } else { (half * 0.7).max(0.6) };
            let off = if pass == 0 { 0.0 } else { reach * 0.35 };
            match self.tool {
                Tool::Rect => {
                    let corners = [(left, top), (left + width, top), (left + width, top + height), (left, top + height)];
                    for side in 0..4 {
                        let (from, to) = (corners[side], corners[(side + 1) % 4]);
                        let length = (to.0 - from.0).hypot(to.1 - from.1).max(1.0);
                        let (dx, dy) = ((to.0 - from.0) / length, (to.1 - from.1) / length);
                        let (start, end) = (wobble(reach * 0.5).abs(), wobble(reach).abs());
                        let from = (from.0 - dx * start + wobble(off), from.1 - dy * start + wobble(off));
                        let to = (to.0 + dx * end + wobble(off), to.1 + dy * end + wobble(off));
                        strokes.push((stroke(from, to, &mut wobble), thin));
                    }
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) = (left + width / 2.0, top + height / 2.0, width / 2.0, height / 2.0);
                    let (begin, sweep) = (wobble(std::f32::consts::PI), std::f32::consts::TAU * (1.06 + wobble(0.04)));
                    let (phase, swell) = (wobble(3.0), 0.025 + wobble(0.012).abs());
                    let points = (0..=64)
                        .map(|step| {
                            let angle = begin + sweep * step as f32 / 64.0;
                            let grow = 1.0 + swell * (angle * 2.0 + phase).sin() + off / rx.max(ry).max(1.0);
                            (cx + rx * grow * angle.cos(), cy + ry * grow * angle.sin())
                        })
                        .collect();
                    strokes.push((points, thin));
                }
                _ => {
                    let (a, b) = (self.points[0], self.points[1]);
                    let path: Vec<Point> = match self.curve() {
                        Some(curve) => curve.iter().map(|point| (point.0 + wobble(off * 0.5), point.1 + wobble(off * 0.5))).collect(),
                        None => stroke((a.0 + wobble(off), a.1 + wobble(off)), (b.0 + wobble(off), b.1 + wobble(off)), &mut wobble),
                    };
                    // Each head two strokes back from the tip, as a hand
                    // makes one, whatever the kind it is drawn as clean.
                    let (head, wide) = self.head();
                    let (head, wide) = (head * 1.25, wide * 1.4);
                    for (end, tip, from) in [(1, path[path.len() - 1], path[path.len().saturating_sub(4)]), (0, path[0], path[3.min(path.len() - 1)])] {
                        if !self.heads[end] {
                            continue;
                        }
                        let length = (tip.0 - from.0).hypot(tip.1 - from.1).max(1e-3);
                        let (dx, dy) = ((tip.0 - from.0) / length, (tip.1 - from.1) / length);
                        let base = (tip.0 - dx * head * 1.1, tip.1 - dy * head * 1.1);
                        for side in [1.0, -1.0] {
                            let barb = (base.0 - dy * wide * side + wobble(off), base.1 + dx * wide * side + wobble(off));
                            strokes.push((stroke(barb, tip, &mut wobble), thin));
                        }
                    }
                    strokes.push((path, thin));
                }
            }
        }
        for (points, thin) in strokes {
            for pair in points.windows(2) {
                stamp(&mut mask, pair[0], pair[1], thin);
            }
        }
        mask
    }

    /// What is inside the border of a rectangle or an oval, as far as the
    /// middle of its line, which the border is drawn over: kept as its
    /// mask is.
    fn inside_mask(&self) -> Rc<Mask> {
        if let Some(mask) = self.inside.borrow().as_ref() {
            return mask.clone();
        }
        let bounds = match self.angle {
            0.0 => self.extent(),
            angle => turned_around(self.extent(), self.centre_of_frame(), angle),
        };
        let outline = Outline { filled: true, stroke: 0.0, ..self.outline() };
        let mask = Rc::new(covered(bounds, move |x, y| outline.distance(x, y)));
        *self.inside.borrow_mut() = Some(mask.clone());
        mask
    }

    /// What a box or an oval covers laid down in its fill: its one color,
    /// or its gradient, worked out where each pixel is in it as it is
    /// unturned, so that it turns with it and is as smooth at any size.
    fn lay_fill(&self, canvas: &mut Canvas, mask: &Mask) {
        let Some(gradient) = self.gradient else { return canvas.mask(mask, self.fill, self.opacity) };
        let (x, y, w, h) = self.frame();
        let centre = (x + w / 2.0, y + h / 2.0);
        let (sin, cos) = (-self.angle).sin_cos();
        let (from, to) = (self.fill, gradient.to);
        canvas.mask_painted(mask, self.opacity, move |px, py| {
            // The pixel's middle, from the box's, turned back square.
            let (dx, dy) = (px as f32 + 0.5 - centre.0, py as f32 + 0.5 - centre.1);
            let (lx, ly) = (dx * cos - dy * sin, dx * sin + dy * cos);
            // A little unevenness, less than a step of color, so that a
            // wide gradient shows no bands.
            let noise = (((px * 7 + py * 13).rem_euclid(16)) as f32 / 16.0 - 0.5) / 255.0;
            between(from, to, along_gradient(lx, ly, w, h, gradient.angle, gradient.radial) + noise)
        });
    }

    pub fn draw(&self, canvas: &mut Canvas, text: &mut Text) {
        match self.tool {
            // Words turned: drawn straight on a sheet of their own, and
            // the sheet laid down turned.
            Tool::Text | Tool::Badge if self.angle != 0.0 => self.draw_turned(canvas, text),
            // A spotlight is no mark of its own: `paint` sees to it.
            Tool::Select | Tool::Spotlight => {}
            Tool::Blur => canvas.pixelate(self.frame_rect(), (self.stroke * 3.0 + 4.0).round() as i32),
            Tool::Text => {
                let (x, y) = (self.points[0].0.round() as i32, self.points[0].1.round() as i32);
                let high = Text::line_height(self.font_px);
                // On a patch, the patch first: round all of its lines,
                // with a little room, and the letters in what reads on it.
                let mut ink = self.color;
                if self.style & BOXED != 0 && !self.text.is_empty() {
                    let (place, room) = (self.local_bounds(text), high / 4);
                    canvas.round_fill(place.x - room * 3 / 2, place.y - room / 2, place.w + room * 3, place.h + room, room, self.color, self.opacity);
                    ink = ink_on(self.color);
                }
                // Lines set from the left, in the middle, or to the right
                // of the widest of them.
                let lines = self.text_lines(text);
                let widest = lines.iter().map(|line| line.width).max().unwrap_or(0);
                for line in &lines {
                    let (px, high) = (line.px, Text::line_height(line.px));
                    text.look = self.look_at(px, line.heading);
                    let top = y + line.top;
                    let x = x + (widest - line.width) * i32::from(self.align.min(2)) / 2;
                    // Gone over with a marker first, as wide as the
                    // words of the line are.
                    if self.style & HIGHLIGHT != 0 && !line.shown.is_empty() {
                        canvas.round_fill(x - high / 8, top + high / 10, line.width + high / 4, high - high / 5, high / 8, HIGHLIGHT_COLOR.0, HIGHLIGHT_COLOR.1 * self.opacity / 256);
                    }
                    text.draw_in(self.face, canvas, x, top, px, ink, &line.shown);
                    // Lines under the words, or through them.
                    let thick = (px as i32 / 14).max(1);
                    for (bit, down) in [(UNDERLINE, 0.86), (STRIKE, 0.56)] {
                        if self.style & bit != 0 && line.width > 0 {
                            canvas.round_fill(x, top + (high as f32 * down) as i32, line.width, thick, 0, ink, self.opacity);
                        }
                    }
                }
                text.look = Look::default();
            }
            // A pill of the badge's own pale color, a dot and its words
            // in the strong one, and an edge between the two.
            Tool::Badge => {
                let place = self.local_bounds(text);
                draw_badge(canvas, text, place, self.number as usize, self.color, self.face, self.font_px, &self.text);
            }
            // What it is of, ringed; lines from that to the larger
            // picture of it; and the larger picture, ringed, over them.
            // Round, as a lens is, unless it is wanted square.
            Tool::Zoom => {
                let (of, shown) = self.zoomed();
                let thick = (self.stroke.round() as i32).max(1);
                let corners = |rect: Rect| [(rect.x, rect.y), (rect.right(), rect.y), (rect.x, rect.bottom()), (rect.right(), rect.bottom())].map(|(x, y)| (x as f32, y as f32));
                // Square, from each corner to the same corner; round,
                // the two lines that touch both rings on either side.
                let lines: Vec<(Point, Point)> = match self.filled {
                    true => corners(of).into_iter().zip(corners(shown)).collect(),
                    false => {
                        let (from, to) = (of.center(), shown.center());
                        let (dx, dy) = ((to.0 - from.0) as f32, (to.1 - from.1) as f32);
                        let far = dx.hypot(dy).max(1.0);
                        let (nx, ny) = (-dy / far, dx / far);
                        let edge = |rect: Rect, side: f32| ((rect.center().0 as f32) + nx * side * rect.w as f32 / 2.0, (rect.center().1 as f32) + ny * side * rect.h as f32 / 2.0);
                        vec![(edge(of, 1.0), edge(shown, 1.0)), (edge(of, -1.0), edge(shown, -1.0))]
                    }
                };
                for (from, to) in lines {
                    let reach = Rect::spanning((from.0 as i32, from.1 as i32), (to.0 as i32, to.1 as i32)).inflate(thick + 2);
                    let mut line = Mask { bounds: reach, alpha: vec![0; reach.w as usize * reach.h as usize] };
                    stamp(&mut line, from, to, self.stroke / 4.0 + 0.5);
                    canvas.mask(&line, self.color, 256);
                }
                canvas.magnify(of, shown, !self.filled);
                for rect in [of, shown] {
                    match self.filled {
                        true => {
                            let around = rect.inflate(thick);
                            canvas.round_outline(around.x, around.y, around.w, around.h, thick + 3, thick, self.color);
                        }
                        false => {
                            let mut ring = Shape::new(Tool::Ellipse, self.color, thick as f32, (rect.x as f32, rect.y as f32));
                            ring.points[1] = (rect.right() as f32, rect.bottom() as f32);
                            canvas.mask(&ring.mask(), self.color, 256);
                        }
                    }
                }
            }
            Tool::Marker => canvas.mask(&self.mask(), self.color, MARKER_OPACITY * self.opacity / 256),
            // Its fill, and its border over that; or the one it has.
            Tool::Rect | Tool::Ellipse => {
                if self.fill_apart() {
                    self.lay_fill(canvas, &self.inside_mask());
                }
                match self.border {
                    true => canvas.mask(&self.mask(), self.color, self.opacity),
                    false if !self.sketch => self.lay_fill(canvas, &self.mask()),
                    false => {}
                }
            }
            // An edge of white first, a little wider all round, and the
            // line over it.
            Tool::Line | Tool::Arrow if self.halo => {
                canvas.mask(&self.halo_mask(), 0xFFFFFF, self.opacity);
                canvas.mask(&self.mask(), self.color, self.opacity);
            }
            Tool::Counter => {
                canvas.mask(&self.mask(), self.color, self.opacity);
                let (cx, cy) = (self.points[0].0.round() as i32, self.points[0].1.round() as i32);
                text.look = Look { opacity: self.opacity, ..Look::default() };
                text.draw_centered(canvas, cx, cy, self.font_px, ink_on(self.color), &self.number.to_string());
                text.look = Look::default();
            }
            _ => canvas.mask(&self.mask(), self.color, self.opacity),
        }
    }

    /// Words that cannot be drawn turned where they are: drawn unturned on
    /// a sheet twice, over black and over white, which between them say
    /// how much of each pixel is covered and in what; and the sheet laid
    /// over the picture turned about their middle, each pixel a blend of
    /// the four under it.
    fn draw_turned(&self, canvas: &mut Canvas, text: &mut Text) {
        let mut flat = self.clone();
        flat.angle = 0.0;
        // Room for a patch, a highlight and slanted letters, which go a
        // little past the box the words are in.
        let room = Text::line_height(self.font_px) / 2 + 2;
        let sheet = flat.local_bounds(text).inflate(room);
        if sheet.is_empty() || i64::from(sheet.w) * i64::from(sheet.h) > 16_000_000 {
            return;
        }
        flat.translate(-sheet.x as f32, -sheet.y as f32);
        let (w, h) = (sheet.w, sheet.h);
        let (mut dark, mut light) = (vec![0u32; (w * h) as usize], vec![0xFFFFFFu32; (w * h) as usize]);
        flat.draw(&mut Canvas::new(&mut dark, w, h), text);
        flat.draw(&mut Canvas::new(&mut light, w, h), text);
        // Each pixel of the sheet as its color times how much is covered,
        // which is what is over black, and how much.
        let sample = |x: i32, y: i32| -> ([u32; 3], u32) {
            if x < 0 || y < 0 || x >= w || y >= h {
                return ([0; 3], 0);
            }
            let at = (y * w + x) as usize;
            let (over, under) = (dark[at], light[at]);
            let shown: u32 = [16, 8, 0].iter().map(|shift| 255 - ((under >> shift & 255) - (over >> shift & 255).min(under >> shift & 255))).sum::<u32>() / 3;
            ([16, 8, 0].map(|shift| over >> shift & 255), shown)
        };
        let centre = self.centre(text);
        let (sin, cos) = (-self.angle).sin_cos();
        let area = turned_around(sheet, centre, self.angle).intersect(Rect::new(0, 0, canvas.width, canvas.height));
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                // Where on the sheet this pixel's middle comes from.
                let (dx, dy) = (x as f32 + 0.5 - centre.0, y as f32 + 0.5 - centre.1);
                let (fx, fy) = (centre.0 + dx * cos - dy * sin - sheet.x as f32 - 0.5, centre.1 + dx * sin + dy * cos - sheet.y as f32 - 0.5);
                let (ax, ay) = (fx.floor() as i32, fy.floor() as i32);
                let (tx, ty) = (fx - ax as f32, fy - ay as f32);
                let corners = [(sample(ax, ay), (1.0 - tx) * (1.0 - ty)), (sample(ax + 1, ay), tx * (1.0 - ty)), (sample(ax, ay + 1), (1.0 - tx) * ty), (sample(ax + 1, ay + 1), tx * ty)];
                let (mut sum, mut shown) = ([0.0f32; 3], 0.0f32);
                for ((color, cover), weight) in corners {
                    (0..3).for_each(|channel| sum[channel] += color[channel] as f32 * weight);
                    shown += cover as f32 * weight;
                }
                if shown < 0.5 {
                    continue;
                }
                // The color as it is where it covers all of a pixel.
                let channel = |at: usize| ((sum[at] * 255.0 / shown).round() as u32).min(255);
                canvas.blend(x, y, channel(0) << 16 | channel(1) << 8 | channel(2), shown.round().min(255.0) as u8);
            }
        }
    }

    /// Whether a click at a point is on it.
    pub fn hit(&self, x: i32, y: i32, text: &mut Text) -> bool {
        // Words turned are hit where they are turned to: the point taken
        // round the other way, onto them unturned.
        if self.angle != 0.0 && matches!(self.tool, Tool::Text | Tool::Badge) {
            let (ux, uy) = self.unturn((x as f32, y as f32), text);
            return self.local_bounds(text).inflate(REACH).contains(ux.round() as i32, uy.round() as i32);
        }
        match self.tool {
            Tool::Text | Tool::Blur | Tool::Spotlight | Tool::Badge => self.bounds(text).inflate(REACH).contains(x, y),
            Tool::Zoom => {
                let (of, shown) = self.zoomed();
                of.inflate(REACH).contains(x, y) || shown.inflate(REACH).contains(x, y)
            }
            // On its line, or with a fill inside its border, there too.
            _ => {
                let mut masks = vec![self.mask()];
                masks.extend(self.fill_apart().then(|| self.inside_mask()));
                masks.iter().any(|mask| {
                    let near = Rect::new(x - REACH, y - REACH, 2 * REACH + 1, 2 * REACH + 1).intersect(mask.bounds);
                    !near.is_empty() && (near.y..near.bottom()).any(|py| {
                        let row = &mask.alpha[((py - mask.bounds.y) * mask.bounds.w + near.x - mask.bounds.x) as usize..][..near.w as usize];
                        row.iter().any(|&cover| cover > 64)
                    })
                })
            }
        }
    }
}

/// A triangle filled with a color, its edges smoothed.
fn filled_triangle(canvas: &mut Canvas, a: Point, b: Point, c: Point, color: u32) {
    let (left, top) = (a.0.min(b.0).min(c.0).floor() as i32 - 1, a.1.min(b.1).min(c.1).floor() as i32 - 1);
    let (right, bottom) = (a.0.max(b.0).max(c.0).ceil() as i32 + 1, a.1.max(b.1).max(c.1).ceil() as i32 + 1);
    for y in top..bottom {
        for x in left..right {
            let away = triangle((x as f32 + 0.5, y as f32 + 0.5), a, b, c);
            let cover = ((0.5 - away).clamp(0.0, 1.0) * 255.0) as u8;
            if cover > 0 {
                canvas.blend(x, y, color, cover);
            }
        }
    }
}

/// How many ways a badge can look.
pub const BADGE_STYLES: usize = 26;

/// What each look of a badge is called. The first twelve are labels with
/// some finish to them; those after are plainer, borrowed from what
/// interfaces show: a status, a verdict, a reference, a key, a priority,
/// a tooltip, code, a card, a note, a bubble, a pin.
pub const BADGE_NAMES: [&str; BADGE_STYLES] = [
    "Soft", "Solid", "Outline", "Pill", "Dark", "Label", "Gloss", "Blend", "Glow", "Tag", "Marker", "Sticker", "Status", "Do", "Don't", "Chip", "Key", "Priority", "Tooltip", "Error", "Code", "Card", "Sticky note", "Bubble", "Pin", "Glass key",
];

/// The face some looks keep to whatever face is in hand: code and a
/// chip in mono, a sticky note in the hand-drawn face.
fn badge_face(style: usize, face: usize) -> usize {
    match style {
        15 | 20 => 3,
        22 => 2,
        _ => face,
    }
}

/// How a badge of a look is laid out round words `words` wide, in
/// letters `px` high on lines `line` high: its width and height, and
/// where its words go, from its top left.
fn badge_layout(style: usize, words: i32, px: i32, line: i32) -> (i32, i32, i32, i32) {
    let standard = line + line / 3;
    let (lead, trail, high, top) = match style {
        12 => (px * 5 / 4, px * 3 / 4, standard, (standard - line) / 2),
        13 | 14 => (px * 2, px * 3 / 4, standard, (standard - line) / 2),
        15 => (px / 2, px / 2, line + line / 6, line / 12),
        16 | 25 => (px * 3 / 5, px * 3 / 5, standard + line / 8, (standard - line) / 2),
        17 => (px / 3, px / 3, line, 0),
        // A tooltip has its point under it; an error, over it at the left.
        18 => (px * 3 / 4, px * 3 / 4, line * 3 / 2 + line / 3, line / 4),
        19 => (px * 3 / 4, px * 3 / 4, line * 3 / 2 + line / 3, line / 3 + line / 4),
        20 => (px * 3 / 2, px * 3 / 4, line + line * 2 / 3, line / 3),
        21 => (px, px, line * 2, line / 2),
        22 => (px * 3 / 4, px * 3 / 2, line * 5 / 2, line / 2),
        // A bubble has its tail under it.
        23 => (px, px, line * 3 / 2 + line / 2, line / 4),
        // A pin: a round head as wide as its words, and a point under it.
        24 => {
            let round = (words + px).max(line * 3 / 2);
            return ((round - words) / 2, round, round * 7 / 5, (round - line) / 2);
        }
        _ => (badge_lead(style, px), px * 3 / 4, standard, (standard - line) / 2),
    };
    (lead, words + lead + trail, high, top)
}

/// How far in from its left a badge's words begin, in one of its looks
/// and at a size of letter: after a dot, or a bar, or nothing.
fn badge_lead(style: usize, px: i32) -> i32 {
    match style {
        0 | 4 => px * 5 / 4,
        5 => px,
        9 => px * 3 / 2,
        10 => px / 2,
        _ => px * 3 / 4,
    }
}

/// A color turned so many degrees round the wheel of them, as light
/// and as strong as it was.
fn turned(color: u32, degrees: f32) -> u32 {
    let [r, g, b] = [16, 8, 0].map(|shift| (color >> shift & 0xFF) as f32 / 255.0);
    let (most, least) = (r.max(g).max(b), r.min(g).min(b));
    let (light, spread) = ((most + least) / 2.0, most - least);
    if spread < 1e-4 {
        return color;
    }
    let strength = spread / (1.0 - (2.0 * light - 1.0).abs()).max(1e-4);
    let hue = if most == r { ((g - b) / spread).rem_euclid(6.0) } else if most == g { (b - r) / spread + 2.0 } else { (r - g) / spread + 4.0 };
    let hue = (hue * 60.0 + degrees).rem_euclid(360.0);
    let part = |turn: f32| {
        let at = (turn + hue / 30.0).rem_euclid(12.0);
        let value = light - strength * light.min(1.0 - light) * (at - 3.0).min(9.0 - at).clamp(-1.0, 1.0);
        (value.clamp(0.0, 1.0) * 255.0).round() as u32
    };
    part(0.0) << 16 | part(8.0) << 8 | part(4.0)
}

/// Fills a rectangle with rounded corners, each pixel in the color a
/// function gives for where in it the pixel is: so far across and so
/// far down, each of 0 to 1. `grown` makes it that much larger all
/// round, and `soft` fades its edge over that many pixels, as a shadow
/// or a glow has it; `amount` is how opaque, of 256.
fn patch(canvas: &mut Canvas, place: Rect, radius: i32, grown: i32, soft: f32, amount: u32, color: impl Fn(f32, f32) -> u32) {
    let (half_w, half_h) = (place.w as f32 / 2.0 + grown as f32, place.h as f32 / 2.0 + grown as f32);
    let (cx, cy) = (place.x as f32 + place.w as f32 / 2.0, place.y as f32 + place.h as f32 / 2.0);
    let radius = (radius + grown).min(half_w as i32).min(half_h as i32) as f32;
    let reach = grown + soft.ceil() as i32 + 1;
    for y in place.y - reach..place.bottom() + reach {
        for x in place.x - reach..place.right() + reach {
            let away = round_box(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy, half_w, half_h, radius);
            let cover = if soft > 0.0 { (0.5 - away / soft).clamp(0.0, 1.0).powi(2) } else { (0.5 - away).clamp(0.0, 1.0) };
            if cover > 0.0 {
                let (across, down) = ((x - place.x) as f32 / place.w.max(1) as f32, (y - place.y) as f32 / place.h.max(1) as f32);
                canvas.blend(x, y, color(across.clamp(0.0, 1.0), down.clamp(0.0, 1.0)), (cover * amount.min(256) as f32 * 255.0 / 256.0) as u8);
            }
        }
    }
}

/// Draws a badge in a place: words on a patch, in one of twelve looks,
/// all of them made of the one color it is given. Which of that color,
/// the pale of it and the dark of it is under the words, and what the
/// words are in, is the look.
#[allow(clippy::too_many_arguments)]
pub fn draw_badge(canvas: &mut Canvas, text: &mut Text, place: Rect, style: usize, solid: u32, face: usize, px: u32, words: &str) {
    use crate::canvas::mix;
    let (high, size) = (place.h, px as i32);
    let soft = mix(solid, 0xFFFFFF, 216);
    let ink = mix(solid, 0x000000, 70);
    let dot = |canvas: &mut Canvas| canvas.disc(place.x + size * 5 / 8, place.y + high / 2, (size / 5).max(2), solid, 256);
    let color = match style {
        // Gloss: lighter at the top than the bottom, with a pale line
        // along its top edge, as a button has that is lit from above.
        6 => {
            let (top, bottom) = (mix(solid, 0xFFFFFF, 60), mix(solid, 0x000000, 50));
            patch(canvas, place, high / 3, 0, 0.0, 256, |_, _| mix(solid, 0x000000, 90));
            patch(canvas, place.inflate(-1), high / 3 - 1, 0, 0.0, 256, |_, down| mix(top, bottom, (down * 256.0) as u32));
            patch(canvas, Rect::new(place.x + high / 3, place.y + 1, place.w - 2 * (high / 3), 1), 0, 0, 0.0, 90, |_, _| 0xFFFFFF);
            ink_on(solid)
        }
        // Blend: from the color to the one a little round the wheel
        // from it, end to end, and ends as round as it is tall.
        7 => {
            let far = turned(solid, 50.0);
            patch(canvas, place, high / 2, 0, 0.0, 256, |across, down| mix(solid, far, ((across * 0.85 + down * 0.15) * 256.0) as u32));
            ink_on(solid)
        }
        // Glow: the color, with light of the same color round it.
        8 => {
            patch(canvas, Rect::new(place.x, place.y + size / 6, place.w, high), high / 2, size / 6, size as f32 * 0.6, 150, |_, _| solid);
            patch(canvas, place, high / 2, 0, 0.0, 256, |_, down| mix(mix(solid, 0xFFFFFF, 36), solid, (down * 256.0) as u32));
            ink_on(solid)
        }
        // A tag: pointed at its left end, with a hole to hang it by.
        9 => {
            let (point, middle) = (high / 2, place.y + high / 2);
            patch(canvas, Rect::new(place.x + point, place.y, place.w - point, high), high / 5, 0, 0.0, 256, |_, _| solid);
            for y in place.y..place.bottom() {
                for x in place.x..place.x + point + high / 5 + 1 {
                    let away = triangle((x as f32 + 0.5, y as f32 + 0.5), (place.x as f32, middle as f32), ((place.x + point + high / 5 + 1) as f32, place.y as f32), ((place.x + point + high / 5 + 1) as f32, place.bottom() as f32));
                    canvas.blend(x, y, solid, ((0.5 - away).clamp(0.0, 1.0) * 255.0) as u8);
                }
            }
            canvas.disc(place.x + point + size / 6, middle, (size / 6).max(2), ink_on(solid), 256);
            ink_on(solid)
        }
        // Marker: gone over with a wide pen, a little past the words
        // at either end and not quite as tall, the words as dark as
        // what a marker is used on.
        10 => {
            let swipe = Rect::new(place.x, place.y + high / 5, place.w, high - high * 2 / 5);
            patch(canvas, swipe, high / 6, 0, 0.0, 256, |across, _| mix(mix(solid, 0xFFFFFF, 110), mix(solid, 0xFFFFFF, 60), (across * 256.0) as u32));
            mix(solid, 0x000000, 190)
        }
        // A sticker: the color with white all round it, and a shadow
        // under that, as if it were stuck on.
        11 => {
            let edge = (size / 6).max(2);
            patch(canvas, Rect::new(place.x, place.y + edge, place.w, high), high / 3, edge, size as f32 * 0.5, 110, |_, _| 0x000000);
            patch(canvas, place, high / 3, edge, 0.0, 256, |_, _| 0xFFFFFF);
            patch(canvas, place, high / 3, 0, 0.0, 256, |_, down| mix(mix(solid, 0xFFFFFF, 30), mix(solid, 0x000000, 24), (down * 256.0) as u32));
            ink_on(solid)
        }
        // Soft: the pale under the dark, with a dot and a faint edge.
        0 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 3, crate::canvas::mix(soft, solid, 70), 256);
            canvas.round_fill(place.x + 1, place.y + 1, place.w - 2, high - 2, high / 3 - 1, soft, 256);
            dot(canvas);
            ink
        }
        // Solid: the hue itself, and whichever of white and black reads on it.
        // A shade lighter at its top than its bottom, as each of the
        // filled ones is: enough not to look cut out of paper.
        1 => {
            patch(canvas, place, high / 3, 0, 0.0, 256, |_, down| mix(mix(solid, 0xFFFFFF, 26), mix(solid, 0x000000, 20), (down * 256.0) as u32));
            ink_on(solid)
        }
        // Outlined: nothing under the words but what was there.
        2 => {
            canvas.round_outline(place.x, place.y, place.w, high, high / 3, (size / 8).max(2), solid);
            solid
        }
        // A pill: solid, with ends as round as it is tall.
        3 => {
            patch(canvas, place, high / 2, 0, 0.0, 256, |_, down| mix(mix(solid, 0xFFFFFF, 26), mix(solid, 0x000000, 20), (down * 256.0) as u32));
            ink_on(solid)
        }
        // A status: a dot of the color and the words, on a plain white
        // ground so they read on anything.
        12 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 2, 0xD9DBE0, 256);
            canvas.round_fill(place.x + 1, place.y + 1, place.w - 2, high - 2, high / 2 - 1, 0xFFFFFF, 256);
            canvas.disc(place.x + size * 5 / 8, place.y + high / 2, (size / 4).max(2), solid, 256);
            0x161616
        }
        // Do, and don't: a tick or a cross on a disc of the color, and
        // the words in a darker shade of it.
        13 | 14 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 2, 0xD9DBE0, 256);
            canvas.round_fill(place.x + 1, place.y + 1, place.w - 2, high - 2, high / 2 - 1, 0xFFFFFF, 256);
            let (cx, cy, radius) = (place.x + size, place.y + high / 2, size * 3 / 5);
            canvas.disc(cx, cy, radius, solid, 256);
            let reach = Rect::new(cx - radius, cy - radius, 2 * radius + 1, 2 * radius + 1);
            let mut mark = Mask { bounds: reach, alpha: vec![0; (reach.w * reach.h) as usize] };
            let (x, y, r, thick) = (cx as f32 + 0.5, cy as f32 + 0.5, radius as f32, (size as f32 / 9.0).max(1.2));
            if style == 13 {
                stamp(&mut mark, (x - r * 0.45, y + r * 0.02), (x - r * 0.12, y + r * 0.35), thick);
                stamp(&mut mark, (x - r * 0.12, y + r * 0.35), (x + r * 0.45, y - r * 0.3), thick);
            } else {
                stamp(&mut mark, (x - r * 0.35, y - r * 0.35), (x + r * 0.35, y + r * 0.35), thick);
                stamp(&mut mark, (x + r * 0.35, y - r * 0.35), (x - r * 0.35, y + r * 0.35), thick);
            }
            canvas.mask(&mark, 0xFFFFFF, 256);
            mix(solid, 0x000000, 110)
        }
        // A chip: a reference, in mono on a pale tint of the color.
        15 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 4, mix(solid, 0xFFFFFF, 215), 256);
            0x1C2024
        }
        // A key, as keyboards draw them: white, edged, deeper at the
        // bottom; and one of dark glass, as screencasts show them.
        16 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 4, 0xB9BBC6, 256);
            canvas.round_fill(place.x + 1, place.y + 1, place.w - 2, high - 4, high / 4 - 1, 0xFFFFFF, 256);
            0x1C2024
        }
        25 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 4, 0x141416, 230);
            canvas.round_outline(place.x, place.y, place.w, high, high / 4, 1, 0x4A4D57);
            0xFFFFFF
        }
        // A priority: the color, square, small.
        17 => {
            canvas.round_fill(place.x, place.y, place.w, high, (size / 6).max(2), solid, 256);
            ink_on(solid)
        }
        // A tooltip: the color, with a point under its middle; and one
        // with its point over it near its left, as a form shows an error.
        18 | 19 => {
            let point = Text::line_height(px) / 3;
            let (body, tip) = match style {
                18 => (Rect::new(place.x, place.y, place.w, high - point), (place.x + place.w / 2, place.bottom())),
                _ => (Rect::new(place.x, place.y + point, place.w, high - point), (place.x + size * 3 / 2, place.y)),
            };
            canvas.round_fill(body.x, body.y, body.w, body.h, size / 3, solid, 256);
            let base = if style == 18 { body.bottom() } else { body.y };
            filled_triangle(canvas, (tip.0 as f32, tip.1 as f32), ((tip.0 - point) as f32, base as f32), ((tip.0 + point) as f32, base as f32), solid);
            ink_on(solid)
        }
        // Code: light mono on near black, after a dim prompt.
        20 => {
            canvas.round_fill(place.x, place.y, place.w, high, size / 3, 0x1C2024, 256);
            let line = Text::line_height(px);
            text.draw_in(3, canvas, place.x + size / 2, place.y + line / 3, px, 0x8B8D98, "$");
            0xE6E8EC
        }
        // A card: white, softly shadowed, its words heavy.
        21 => {
            patch(canvas, Rect::new(place.x, place.y + size / 4, place.w, high), size / 2, size / 4, size as f32 * 0.8, 70, |_, _| 0x000000);
            canvas.round_fill(place.x, place.y, place.w, high, size / 2, 0xE1E3E8, 256);
            canvas.round_fill(place.x + 1, place.y + 1, place.w - 2, high - 2, size / 2 - 1, 0xFFFFFF, 256);
            0x161616
        }
        // A sticky note: a square of the color with a curl of shadow
        // under its lower edge, in the hand-drawn face.
        22 => {
            patch(canvas, Rect::new(place.x + size / 3, place.y + size / 2, place.w - size * 2 / 3, high), 2, size / 3, size as f32 * 0.7, 110, |_, _| 0x000000);
            canvas.round_fill(place.x, place.y, place.w, high, 2, solid, 256);
            mix(ink_on(solid), solid, 40)
        }
        // A bubble: the color, edged in dark, with a tail under its left.
        23 => {
            let tail = Text::line_height(px) / 2;
            let body = Rect::new(place.x, place.y, place.w, high - tail);
            let edge = (size / 8).max(2);
            let (tip, left, right) = ((place.x + size, place.bottom()), (place.x + size * 3 / 4, body.bottom() - edge), (place.x + size * 2, body.bottom() - edge));
            filled_triangle(canvas, (tip.0 as f32, tip.1 as f32), (left.0 as f32, left.1 as f32), (right.0 as f32, right.1 as f32), 0x161616);
            canvas.round_fill(body.x, body.y, body.w, body.h, body.h / 2, 0x161616, 256);
            canvas.round_fill(body.x + edge, body.y + edge, body.w - 2 * edge, body.h - 2 * edge, body.h / 2 - edge, solid, 256);
            let inset = edge as f32 * 1.8;
            filled_triangle(canvas, (tip.0 as f32 + inset * 0.3, tip.1 as f32 - inset * 1.6), (left.0 as f32 + inset, left.1 as f32 - 1.0), (right.0 as f32 - inset * 0.8, right.1 as f32 - 1.0), solid);
            ink_on(solid)
        }
        // A pin: a round head of the color over a point, its words in
        // the middle of the head.
        24 => {
            let round = place.w;
            let (cx, cy) = (place.x + round / 2, place.y + round / 2);
            filled_triangle(canvas, (cx as f32, place.bottom() as f32), (cx as f32 - round as f32 * 0.42, cy as f32 + round as f32 * 0.2), (cx as f32 + round as f32 * 0.42, cy as f32 + round as f32 * 0.2), solid);
            canvas.disc(cx, cy, round / 2, solid, 256);
            ink_on(solid)
        }
        // Dark: near black, with a dot of the hue.
        4 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 3, 0x161616, 256);
            dot(canvas);
            0xEEEEEE
        }
        // A label: white, with a bar of the hue down its left side.
        _ => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 5, solid, 256);
            canvas.fill(place.x + size * 2 / 5, place.y, place.w - size * 2 / 5 - high / 5, high, 0xFFFFFF);
            canvas.round_fill(place.x + place.w / 2, place.y, place.w - place.w / 2, high, high / 5, 0xFFFFFF, 256);
            0x161616
        }
    };
    let face = badge_face(style, face);
    let (lead, _, _, top) = badge_layout(style, text.width_in(face, words, px), size, Text::line_height(px));
    text.look.heavy = i32::from(style == 21);
    text.draw_in(face, canvas, place.x + lead, place.y + top, px, color, words);
    text.look.heavy = 0;
}

/// Where a drag from `from` should be taken to end, with Shift held:
/// a square, a circle, or a line at one of eight angles.
pub fn constrained(tool: Tool, from: Point, to: Point) -> Point {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    match tool {
        Tool::Rect | Tool::Ellipse | Tool::Blur | Tool::Spotlight | Tool::Zoom => {
            let side = dx.abs().max(dy.abs());
            (from.0 + side.copysign(dx), from.1 + side.copysign(dy))
        }
        Tool::Arrow | Tool::Line => {
            let step = std::f32::consts::FRAC_PI_4;
            let angle = (dy.atan2(dx) / step).round() * step;
            let length = dx.hypot(dy);
            (from.0 + length * angle.cos(), from.1 + length * angle.sin())
        }
        _ => to,
    }
}

/// The color for a number on a disc of `color`: white, or near black
/// on one light enough that white would be lost on it.
pub fn ink_on(color: u32) -> u32 {
    let channel = |shift: u32| f64::from(color >> shift & 0xFF);
    let brightness = 0.299 * channel(16) + 0.587 * channel(8) + 0.114 * channel(0);
    if brightness > 160.0 { 0x161616 } else { 0xFFFFFF }
}

/// How much of each pixel of a rectangle is inside an outline, given how
/// far any point is from it. A large one has its rows done each on its
/// own core.
fn covered(bounds: Rect, distance: impl Fn(f32, f32) -> f32 + Sync) -> Mask {
    let mut alpha = vec![0u8; bounds.w.max(0) as usize * bounds.h.max(0) as usize];
    let row = |(row, line): (usize, &mut [u8])| {
        let y = (bounds.y + row as i32) as f32 + 0.5;
        for (column, cover) in line.iter_mut().enumerate() {
            let far = distance((bounds.x + column as i32) as f32 + 0.5, y);
            *cover = ((0.5 - far).clamp(0.0, 1.0) * 255.0) as u8;
        }
    };
    let width = bounds.w.max(1) as usize;
    match alpha.len() >= 1 << 14 {
        true => alpha.par_chunks_mut(width).enumerate().for_each(row),
        false => alpha.chunks_mut(width).enumerate().for_each(row),
    }
    Mask { bounds, alpha }
}

/// Adds a line with round ends to a mask: each pixel takes the most it
/// has been covered by any line, so that where two of a stroke's lines
/// meet it is no darker than along them.
fn stamp(mask: &mut Mask, a: Point, b: Point, half: f32) {
    let reach = half + 1.0;
    let (left, top) = ((a.0.min(b.0) - reach).floor() as i32, (a.1.min(b.1) - reach).floor() as i32);
    let (right, bottom) = ((a.0.max(b.0) + reach).ceil() as i32, (a.1.max(b.1) + reach).ceil() as i32);
    let part = Rect::new(left, top, right - left, bottom - top).intersect(mask.bounds);
    for y in part.y..part.bottom() {
        let row = &mut mask.alpha[((y - mask.bounds.y) * mask.bounds.w + part.x - mask.bounds.x) as usize..][..part.w as usize];
        for (column, cover) in row.iter_mut().enumerate() {
            let far = segment(((part.x + column as i32) as f32 + 0.5, y as f32 + 0.5), a, b) - half;
            *cover = (*cover).max(((0.5 - far).clamp(0.0, 1.0) * 255.0) as u8);
        }
    }
}

/// How far a point is from a line between two others.
fn segment(p: Point, a: Point, b: Point) -> f32 {
    let (px, py, bx, by) = (p.0 - a.0, p.1 - a.1, b.0 - a.0, b.1 - a.1);
    let along = if bx == 0.0 && by == 0.0 { 0.0 } else { ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0) };
    (px - bx * along).hypot(py - by * along)
}

/// How far a point is from the edge of a triangle, negative inside.
fn triangle(p: Point, a: Point, b: Point, c: Point) -> f32 {
    let edge = |from: Point, to: Point| {
        let (ex, ey, vx, vy) = (to.0 - from.0, to.1 - from.1, p.0 - from.0, p.1 - from.1);
        let along = ((vx * ex + vy * ey) / (ex * ex + ey * ey).max(1e-6)).clamp(0.0, 1.0);
        let (qx, qy) = (vx - ex * along, vy - ey * along);
        (qx * qx + qy * qy, vx * ey - vy * ex)
    };
    let turn = ((b.0 - a.0) * (a.1 - c.1) - (b.1 - a.1) * (a.0 - c.0)).signum();
    let (first, second, third) = (edge(a, b), edge(b, c), edge(c, a));
    let nearest = first.0.min(second.0).min(third.0);
    let within = (turn * first.1).min(turn * second.1).min(turn * third.1);
    -nearest.sqrt() * within.signum()
}

/// How far a point, counted from the middle of a rectangle with rounded
/// corners, is from its edge.
fn round_box(x: f32, y: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let (across, down) = (x.abs() - (half_w - radius), y.abs() - (half_h - radius));
    across.max(0.0).hypot(down.max(0.0)) + across.max(down).min(0.0) - radius
}

/// How far round the edge of a box with round corners a point is, from
/// where its top edge begins at the left and going clockwise; and how
/// far it is all the way round. Counted from the middle of the box, whose
/// corners have each a radius of their own: top left, top right, bottom
/// right, bottom left.
fn perimeter_along(x: f32, y: f32, half_w: f32, half_h: f32, radii: [f32; 4]) -> (f32, f32) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let [tl, tr, br, bl] = radii;
    // Each edge between its corners, and each corner, going round from
    // the top edge's left end: where each begins.
    let (top, right) = ((2.0 * half_w - tl - tr).max(0.0), (2.0 * half_h - tr - br).max(0.0));
    let (bottom, left) = ((2.0 * half_w - br - bl).max(0.0), (2.0 * half_h - bl - tl).max(0.0));
    let at_tr = top;
    let at_right = at_tr + tr * FRAC_PI_2;
    let at_br = at_right + right;
    let at_bottom = at_br + br * FRAC_PI_2;
    let at_bl = at_bottom + bottom;
    let at_left = at_bl + bl * FRAC_PI_2;
    let at_tl = at_left + left;
    let whole = at_tl + tl * FRAC_PI_2;
    let radius = corner_of(x, y, radii);
    let (inner_w, inner_h) = ((half_w - radius).max(0.0), (half_h - radius).max(0.0));
    let along = if x.abs() > inner_w && y.abs() > inner_h && radius > 0.0 {
        // On a corner: how far round it, about its middle.
        let (cx, cy) = (inner_w.copysign(x), inner_h.copysign(y));
        let angle = (y - cy).atan2(x - cx);
        match (x >= 0.0, y >= 0.0) {
            (true, false) => at_tr + (angle + FRAC_PI_2) * radius,
            (true, true) => at_br + angle * radius,
            (false, true) => at_bl + (angle - FRAC_PI_2) * radius,
            (false, false) => at_tl + (angle.rem_euclid(2.0 * PI) - PI) * radius,
        }
    } else if y.abs() - half_h >= x.abs() - half_w {
        match y < 0.0 {
            true => x + half_w - tl,
            false => at_bottom + (half_w - br - x),
        }
    } else {
        match x > 0.0 {
            true => at_right + (y + half_h - tr),
            false => at_left + (half_h - bl - y),
        }
    };
    (along.clamp(0.0, whole), whole.max(1.0))
}

/// How far a point, counted from the middle of an ellipse, is from its
/// edge: near enough for a line a few pixels wide.
fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> f32 {
    let (rx, ry) = (rx.max(0.5), ry.max(0.5));
    let outward = (x / rx).hypot(y / ry);
    let slope = (x / (rx * rx)).hypot(y / (ry * ry));
    if slope < 1e-6 { -rx.min(ry) } else { outward * (outward - 1.0) / slope }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixels a shape covers on a dark canvas so large: where each is
    /// lit, and with what.
    fn drawn(shape: &Shape, wide: i32, high: i32) -> Vec<u32> {
        let mut text = Text::load();
        let mut pixels = vec![0u32; (wide * high) as usize];
        shape.draw(&mut Canvas::new(&mut pixels, wide, high), &mut text);
        pixels
    }

    #[test]
    fn each_end_of_an_arrow_has_the_head_it_is_given() {
        let mut arrow = Shape::new(Tool::Arrow, 0xFF0000, 4.0, (20.0, 40.0));
        arrow.points[1] = (180.0, 40.0);
        // The middle of a head is solid, or empty where it is an outline;
        // a dot sits on the end; a bar stands across it.
        assert_eq!(drawn(&arrow, 200, 80)[40 * 200 + 168], 0xFF0000);
        arrow.ends[1] = HOLLOW_HEAD;
        arrow.touch();
        assert_eq!(drawn(&arrow, 200, 80)[40 * 200 + 168], 0);
        arrow.ends[1] = DOT_HEAD;
        arrow.touch();
        assert_eq!(drawn(&arrow, 200, 80)[40 * 200 + 180], 0xFF0000);
        arrow.ends[1] = BAR_HEAD;
        arrow.touch();
        let pixels = drawn(&arrow, 200, 80);
        assert!(pixels[32 * 200 + 180] != 0 && pixels[32 * 200 + 160] == 0);
        // Both ends, each its own.
        (arrow.heads, arrow.ends) = ([true, true], [RING_HEAD, OPEN_HEAD]);
        arrow.touch();
        let pixels = drawn(&arrow, 200, 80);
        assert!(pixels[40 * 200 + 20] != 0 && pixels[40 * 200 + 28] == 0, "a ring is hollow");
    }

    #[test]
    fn an_elbow_goes_across_then_down_and_a_dotted_line_is_dots() {
        let mut line = Shape::new(Tool::Line, 0xFFFFFF, 4.0, (20.0, 20.0));
        (line.points[1], line.elbow) = ((120.0, 100.0), true);
        let pixels = drawn(&line, 140, 120);
        // Along the top, down the right, and nothing across the middle.
        assert!(pixels[20 * 140 + 60] != 0 && pixels[70 * 140 + 120] != 0 && pixels[60 * 140 + 70] == 0);
        let mut dots = Shape::new(Tool::Line, 0xFFFFFF, 4.0, (10.0, 10.0));
        (dots.points[1], dots.dotted) = ((110.0, 10.0), true);
        let row: Vec<bool> = drawn(&dots, 120, 20)[10 * 120..11 * 120].iter().map(|&pixel| pixel != 0).collect();
        let runs = row.windows(2).filter(|pair| pair[0] && !pair[1]).count();
        assert!(runs >= 8, "{runs} dots");
    }

    #[test]
    fn a_box_border_lies_inside_or_outside_and_is_dashed_or_dotted_round_it() {
        let mut boxed = Shape::new(Tool::Rect, 0xFFFFFF, 8.0, (20.0, 20.0));
        boxed.points[1] = (120.0, 100.0);
        // Across the edge, half in and half out; then all inside; then out.
        let row = |shape: &Shape| drawn(shape, 140, 120)[60 * 140..61 * 140].to_vec();
        let lit = |row: &[u32], x: usize| row[x] != 0;
        let middle = row(&boxed);
        assert!(lit(&middle, 17) && lit(&middle, 22) && !lit(&middle, 27));
        boxed.edge = INSIDE;
        boxed.touch();
        let inside = row(&boxed);
        assert!(!lit(&inside, 17) && lit(&inside, 22) && lit(&inside, 26));
        boxed.edge = OUTSIDE;
        boxed.touch();
        let outside = row(&boxed);
        assert!(lit(&outside, 13) && lit(&outside, 18) && !lit(&outside, 22));
        // Dashed, the top edge goes on and off; dotted, more often.
        (boxed.edge, boxed.dashed) = (CENTER, true);
        boxed.touch();
        let runs = |shape: &Shape| {
            let top: Vec<bool> = drawn(shape, 140, 120)[20 * 140..21 * 140].iter().map(|&pixel| pixel != 0).collect();
            top.windows(2).filter(|pair| pair[0] && !pair[1]).count()
        };
        let dashes = runs(&boxed);
        boxed.dotted = true;
        boxed.touch();
        let dots = runs(&boxed);
        assert!(dashes >= 3 && dots > dashes, "{dashes} dashes, {dots} dots");
        // Round the corners too, evenly, with a round box or an oval.
        assert!((perimeter_along(0.0, -10.0, 20.0, 10.0, [0.0; 4]).0 - 20.0).abs() < 0.01);
        assert!((perimeter_along(20.0, 0.0, 20.0, 10.0, [0.0; 4]).0 - 50.0).abs() < 0.01);
        assert!((perimeter_along(0.0, 10.0, 20.0, 10.0, [0.0; 4]).0 - 80.0).abs() < 0.01);
        assert!((perimeter_along(-20.0, 0.0, 20.0, 10.0, [0.0; 4]).0 - 110.0).abs() < 0.01);
        assert!((perimeter_along(0.0, 0.0, 20.0, 10.0, [4.0; 4]).1 - (2.0 * 32.0 + 2.0 * 12.0 + std::f32::consts::TAU * 4.0)).abs() < 0.01);
    }

    #[test]
    fn lines_after_hashes_are_headings_and_text_can_be_underlined_or_in_capitals() {
        let mut text = Text::load();
        assert_eq!(heading_of("## Two"), Some((2, "Two")));
        assert_eq!((heading_of("#no"), heading_of("#### four"), heading_of("plain")), (None, None, None));
        let mut words = Shape::new(Tool::Text, 0xFFFFFF, 2.0, (10.0, 10.0));
        (words.text, words.font_px) = ("# Big\nsmall".into(), 20);
        let lines = words.text_lines(&mut text);
        assert_eq!((lines[0].px, lines[0].shown.as_str(), lines[0].heading, lines[1].px), (40, "Big", true, 20));
        assert_eq!(lines[1].top, Text::line_height(40));
        assert_eq!(words.local_bounds(&mut text).h, Text::line_height(40) + Text::line_height(20));
        // Typed into, the hashes show; kept as typed either way.
        words.editing = true;
        assert_eq!(words.text_lines(&mut text)[0].shown, "# Big");
        assert_eq!(Shape::read(&words.write()).unwrap().text, "# Big\nsmall");
        words.editing = false;
        let narrow = words.text_lines(&mut text)[1].width;
        words.style |= UPPER | UNDERLINE;
        let lines = words.text_lines(&mut text);
        assert!(lines[1].shown == "SMALL" && lines[1].width > narrow);
        // The line under it is drawn below the words.
        words.text = "small".into();
        let pixels = drawn(&words, 200, 60);
        let under = 10 + (Text::line_height(20) as f32 * 0.86) as i32;
        assert!(pixels[(under * 200 + 20) as usize] != 0);
    }

    #[test]
    fn where_a_border_lies_and_a_shadow_are_kept_in_its_line() {
        let mut boxed = Shape::new(Tool::Rect, 0x123456, 3.0, (1.0, 2.0));
        boxed.points[1] = (50.0, 60.0);
        (boxed.edge, boxed.shadow) = (OUTSIDE, Some(crate::frame::Layer { x: 2, y: 8, blur: 20, spread: -3, color: 0x4011_2233 }));
        let back = Shape::read(&boxed.write()).unwrap();
        assert_eq!((back.edge, back.shadow, back.align), (OUTSIDE, boxed.shadow, boxed.align));
        // And one from before either has neither.
        let plain = Shape::new(Tool::Rect, 0x123456, 3.0, (1.0, 2.0)).write();
        let older = plain.replacen(",0,-,-,-\t", "\t", 1);
        assert_ne!(older, plain);
        let back = Shape::read(&older).unwrap();
        assert_eq!((back.edge, back.shadow), (CENTER, None));
    }

    #[test]
    fn a_white_edge_goes_round_a_line_and_a_sketch_is_drawn_by_hand() {
        let mut line = Shape::new(Tool::Line, 0xFF0000, 4.0, (10.0, 30.0));
        (line.points[1], line.halo) = ((110.0, 30.0), true);
        let pixels = drawn(&line, 120, 60);
        assert_eq!((pixels[30 * 120 + 60], pixels[34 * 120 + 60]), (0xFF0000, 0xFFFFFF));
        // Sketched, a box is still where it was drawn, each time the same.
        let mut boxed = Shape::new(Tool::Rect, 0xFF0000, 3.0, (30.0, 30.0));
        (boxed.points[1], boxed.sketch) = ((130.0, 90.0), true);
        let once = drawn(&boxed, 160, 120);
        assert!(once.iter().filter(|&&pixel| pixel != 0).count() > 400);
        assert!(once[60 * 160 + 80] == 0 && once == drawn(&boxed, 160, 120));
        assert!(boxed.hit(30, 60, &mut Text::load()));
    }

    #[test]
    fn heads_lines_elbows_and_sketches_are_written_down_and_read_back() {
        let mut arrow = Shape::new(Tool::Arrow, 0x0090FF, 3.5, (10.0, 20.0));
        arrow.points[1] = (90.0, 70.0);
        (arrow.heads, arrow.ends, arrow.dotted, arrow.halo, arrow.elbow, arrow.sketch) = ([true, true], [DIAMOND_HEAD, BAR_HEAD], true, true, true, true);
        let back = Shape::read(&arrow.write()).expect("read back");
        assert_eq!((back.ends, back.dotted, back.halo, back.elbow, back.sketch), ([DIAMOND_HEAD, BAR_HEAD], true, true, true, true));
    }

    #[test]
    fn distances_are_negative_inside_and_grow_outside() {
        assert!((segment((0.0, 3.0), (-5.0, 0.0), (5.0, 0.0)) - 3.0).abs() < 1e-4);
        assert!((segment((8.0, 4.0), (-5.0, 0.0), (5.0, 0.0)) - 5.0).abs() < 1e-4);
        let (a, b, c) = ((0.0, 0.0), (10.0, 0.0), (0.0, 10.0));
        assert!(triangle((2.0, 2.0), a, b, c) < 0.0);
        assert!((triangle((5.0, -3.0), a, b, c) - 3.0).abs() < 1e-4);
        // The same whichever way round its corners are given.
        assert!(triangle((2.0, 2.0), a, c, b) < 0.0);
        assert!((round_box(12.0, 0.0, 10.0, 5.0, 2.0) - 2.0).abs() < 1e-4);
        assert!(round_box(0.0, 0.0, 10.0, 5.0, 2.0) < 0.0);
        assert!(ellipse(0.0, 0.0, 10.0, 5.0) < 0.0);
        assert!(ellipse(10.0, 0.0, 10.0, 5.0).abs() < 1e-4);
        assert!((ellipse(12.0, 0.0, 10.0, 5.0) - 2.0).abs() < 0.5);
    }

    #[test]
    fn every_shape_as_it_is_begun_is_drawn_and_hit_and_bounded() {
        // As each is when the pointer has only just been pressed: a pen
        // and a marker with one point, the rest with their points all in
        // one place. None of them is to fall over.
        let mut text = Text::load();
        let mut pixels = vec![0u32; 200 * 200];
        for (tool, _) in TOOL_NAMES {
            let shape = Shape::new(tool, 0xFF0000, 4.0, (100.0, 100.0));
            shape.draw(&mut Canvas::new(&mut pixels, 200, 200), &mut text);
            let _ = (shape.bounds(&mut text), shape.hit(100, 100, &mut text), shape.is_slight());
        }
    }

    #[test]
    fn a_line_covers_the_pixels_along_it_and_is_hit_near_it() {
        let mut text = Text::load();
        let mut line = Shape::new(Tool::Line, 0xFF0000, 4.0, (10.0, 10.0));
        line.points[1] = (50.0, 10.0);
        let mut pixels = vec![0u32; 60 * 20];
        line.draw(&mut Canvas::new(&mut pixels, 60, 20), &mut text);
        assert_eq!(pixels[10 * 60 + 30], 0xFF0000);
        assert_eq!(pixels[2 * 60 + 30], 0);
        assert!(line.hit(30, 12, &mut text));
        assert!(!line.hit(30, 19, &mut text));
        // Moved, it is drawn where it now is.
        line.translate(0.0, 5.0);
        let mut pixels = vec![0u32; 60 * 20];
        line.draw(&mut Canvas::new(&mut pixels, 60, 20), &mut text);
        assert_eq!(pixels[15 * 60 + 30], 0xFF0000);
        assert_eq!(pixels[9 * 60 + 30], 0);
    }

    #[test]
    fn a_shape_written_down_is_read_back_the_same() {
        let mut arrow = Shape::new(Tool::Arrow, 0x0090FF, 3.5, (10.5, 20.0));
        (arrow.points[1], arrow.heads, arrow.dashed) = ((-4.0, 99.25), [true, true], true);
        let mut badge = Shape::new(Tool::Badge, 0xE54D2E, 2.0, (5.0, 6.0));
        (badge.text, badge.fill, badge.number, badge.face, badge.font_px, badge.angle) = ("two\nlines \\ and a slash".to_owned(), 0xFEEBE7, 4, 2, 26, 0.75);
        for shape in [arrow, badge] {
            let back = Shape::read(&shape.write()).expect("read back");
            assert_eq!((back.tool, back.color, back.points.clone(), back.text.as_str(), back.angle), (shape.tool, shape.color, shape.points.clone(), shape.text.as_str(), shape.angle));
            assert_eq!((back.heads, back.dashed, back.fill, back.number, back.face, back.font_px, back.stroke), (shape.heads, shape.dashed, shape.fill, shape.number, shape.face, shape.font_px, shape.stroke));
        }
        let mut boxed = Shape::new(Tool::Rect, 0x161616, 3.5, (1.0, 2.0));
        (boxed.points[1], boxed.filled, boxed.fill) = ((40.0, 30.0), true, 0xFFE629);
        let back = Shape::read(&boxed.write()).expect("read back");
        assert_eq!((back.filled, back.border, back.fill, back.color), (true, true, 0xFFE629, 0x161616));
        assert!(Shape::read("nonsense").is_none());
        assert!(Shape::read("arrow\tzz\t1\t0\t0\t16\t0\t01\t0\tFFFFFF\t0\t1,2 3,4\t").is_none());
    }

    #[test]
    fn a_box_filled_before_fills_had_their_own_color_is_filled_in_its_color_with_no_border() {
        let old = "rect\tE5484D\t3.5\t1\t6\t18\t0\t01\t0,256,0,0\tFFFFFF\t0\t10,10 50,40\t";
        let shape = Shape::read(old).expect("read");
        assert_eq!((shape.filled, shape.border, shape.fill), (true, false, 0xE5484D));
        let outlined = Shape::read(&old.replacen("\t1\t6\t", "\t0\t6\t", 1)).expect("read");
        assert_eq!((outlined.filled, outlined.border), (false, true));
    }

    #[test]
    fn a_box_with_a_border_and_a_fill_has_each_in_its_own_color_and_is_hit_inside() {
        let mut text = Text::load();
        let mut boxed = Shape::new(Tool::Rect, 0xFF0000, 4.0, (10.0, 10.0));
        (boxed.points[1], boxed.filled, boxed.fill) = ((70.0, 50.0), true, 0x00FF00);
        let mut pixels = vec![0u32; 80 * 60];
        boxed.draw(&mut Canvas::new(&mut pixels, 80, 60), &mut text);
        assert_eq!((pixels[30 * 80 + 40], pixels[30 * 80 + 10], pixels[5 * 80 + 40]), (0x00FF00, 0xFF0000, 0));
        assert!(boxed.hit(40, 30, &mut text));
        boxed.filled = false;
        boxed.touch();
        assert!(!boxed.hit(40, 30, &mut text) && boxed.hit(10, 30, &mut text));
    }

    #[test]
    fn a_turned_box_is_drawn_and_hit_where_it_is_turned_to() {
        // A wide, low bar about (60, 50), turned a quarter: tall and narrow.
        let mut text = Text::load();
        let mut bar = Shape::new(Tool::Rect, 0xFF0000, 2.0, (20.0, 45.0));
        (bar.points[1], bar.filled, bar.border, bar.fill, bar.angle) = ((100.0, 55.0), true, false, 0xFF0000, std::f32::consts::FRAC_PI_2);
        assert!(bar.hit(60, 20, &mut text));
        assert!(!bar.hit(25, 50, &mut text));
        let mut pixels = vec![0u32; 120 * 100];
        bar.draw(&mut Canvas::new(&mut pixels, 120, 100), &mut text);
        assert_eq!((pixels[20 * 120 + 60], pixels[50 * 120 + 25]), (0xFF0000, 0));
        let place = bar.bounds(&mut text);
        assert!(place.w < 20 && place.h > 70, "{place:?}");
    }

    #[test]
    fn turned_words_are_drawn_turned_and_a_line_turns_by_its_points() {
        let mut text = Text::load();
        let mut words = Shape::new(Tool::Text, 0xFFFFFF, 2.0, (40.0, 110.0));
        (words.text, words.font_px) = ("Hello world".to_owned(), 20);
        let extent = |words: &Shape, text: &mut Text| {
            let mut pixels = vec![0u32; 240 * 240];
            words.draw(&mut Canvas::new(&mut pixels, 240, 240), text);
            let lit: Vec<(usize, usize)> = pixels.iter().enumerate().filter(|(_, p)| **p != 0).map(|(at, _)| (at % 240, at / 240)).collect();
            let span = |pick: fn(&(usize, usize)) -> usize| lit.iter().map(pick).max().unwrap_or(0) - lit.iter().map(pick).min().unwrap_or(0);
            (span(|p| p.0), span(|p| p.1))
        };
        let (wide, high) = extent(&words, &mut text);
        assert!(wide > high * 2);
        let middle = words.centre(&mut text);
        words.turn(std::f32::consts::FRAC_PI_2, middle, &mut text);
        let (wide, high) = extent(&words, &mut text);
        assert!(high > wide * 2, "{wide} \u{d7} {high}");
        // Hit where the turned words are, and not where they were.
        let (x, y) = (middle.0.round() as i32, middle.1.round() as i32);
        assert!(words.hit(x, y - 40, &mut text) && !words.hit(x - 50, y, &mut text));

        let mut line = Shape::new(Tool::Line, 0, 2.0, (0.0, 0.0));
        line.points[1] = (100.0, 0.0);
        line.turn(std::f32::consts::FRAC_PI_2, (50.0, 0.0), &mut text);
        assert_eq!(line.angle, 0.0);
        assert!((line.points[0].0 - 50.0).abs() < 0.01 && (line.points[0].1 + 50.0).abs() < 0.01);
    }

    #[test]
    fn shift_makes_squares_and_straight_lines() {
        assert_eq!(constrained(Tool::Rect, (0.0, 0.0), (30.0, -10.0)), (30.0, -30.0));
        let (x, y) = constrained(Tool::Line, (0.0, 0.0), (100.0, 8.0));
        assert!((x - 100.32).abs() < 0.1 && y.abs() < 1e-3);
    }

    #[test]
    fn an_arrow_stuck_to_a_box_goes_with_it_and_stops_at_its_edge() {
        let mut text = Text::load();
        let mut boxed = Shape::new(Tool::Rect, 0, 2.0, (100.0, 100.0));
        boxed.points[1] = (200.0, 160.0);
        let mut arrow = Shape::new(Tool::Arrow, 0, 2.0, (0.0, 130.0));
        arrow.points[1] = (150.0, 130.0);
        // Put down well inside it, it is stuck to it as a whole, and stops
        // short of its left side on the way from the other end.
        let anchors = [boxed.anchor(&mut text).unwrap()];
        arrow.stick(1, arrow.points[1], &anchors, 8.0);
        assert_eq!(arrow.stuck[1], Some((boxed.id, WHOLE, WHOLE)));
        assert!(follow(std::iter::once(&mut arrow), &anchors));
        assert!((arrow.points[1].0 - (100.0 - STUCK_GAP)).abs() < 0.01 && (arrow.points[1].1 - 130.0).abs() < 0.01, "{:?}", arrow.points[1]);
        // The box moved, the arrow goes with it.
        boxed.translate(300.0, 0.0);
        follow(std::iter::once(&mut arrow), &[boxed.anchor(&mut text).unwrap()]);
        assert!((arrow.points[1].0 - (400.0 - STUCK_GAP)).abs() < 0.01, "{:?}", arrow.points[1]);
        // Put down at its edge, at that spot of it.
        arrow.stick(0, (400.0, 100.0), &[boxed.anchor(&mut text).unwrap()], 8.0);
        assert_eq!(arrow.stuck[0], Some((boxed.id, -1.0, -1.0)));
        // Saved and read back, it is still stuck, and the box is still
        // what it is stuck to; and with the box gone, it lets go.
        let back = Shape::read(&arrow.write()).unwrap();
        assert_eq!((back.id, back.stuck), (arrow.id, arrow.stuck));
        assert_eq!(Shape::read(&boxed.write()).unwrap().id, boxed.id);
        follow(std::iter::once(&mut arrow), &[]);
        assert_eq!(arrow.stuck, [None, None]);
        assert_ne!(boxed.copied().id, boxed.id);
    }
}
