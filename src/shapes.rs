//! What is drawn on a screenshot: arrows, boxes, text and the rest. Each
//! is a few points and a color; its outline is worked out once, as how
//! much of every pixel it covers, and laid down from that whenever the
//! picture is redrawn.

use std::cell::RefCell;
use std::rc::Rc;

use rayon::prelude::*;

use crate::canvas::{Canvas, Mask, Rect};
use crate::text::Text;

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
    /// Whether a line or an arrow is drawn in dashes.
    pub dashed: bool,
    /// What a badge is filled with, its `color` being that of its words.
    pub fill: u32,
    pub number: u32,
    mask: RefCell<Option<Rc<Mask>>>,
}

/// How many times larger a zoom shows what it is of.
const ZOOM: i32 = 2;

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
        Shape { tool, color, stroke, filled: false, radius: 0.0, points, text: String::new(), font_px: 16, face: 0, heads: [false, tool == Tool::Arrow], dashed: false, fill: 0xFFFFFF, number: 0, mask: RefCell::new(None) }
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
            u8::from(self.dashed),
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
        (shape.heads, shape.dashed) = ([heads.starts_with('1'), heads.ends_with('1')], dashed == "1");
        (shape.fill, shape.number) = (u32::from_str_radix(fill, 16).ok()?, number.parse().ok()?);
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

    /// The rectangle a zoom is of, and the one it shows it in: from its
    /// third point, as many times larger as its fourth is across from
    /// that, and the same shape as what it is of.
    fn zoomed(&self) -> (Rect, Rect) {
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

    /// After one of a zoom's two handles has been taken somewhere: the
    /// corner the larger picture is put down by, `by` which it went and
    /// the far corner goes with it; or the far corner, which says how
    /// many times larger and is put back on the corner that makes.
    pub fn settle_zoom(&mut self, point: usize, by: Point) {
        if point == 2 {
            self.points[3] = (self.points[3].0 + by.0, self.points[3].1 + by.1);
        }
        let (_, shown) = self.zoomed();
        self.points[3] = (shown.right() as f32, shown.bottom() as f32);
    }

    /// Where the first letter of a shape with words on it goes.
    pub fn text_origin(&self) -> (i32, i32) {
        let (x, y) = (self.points[0].0.round() as i32, self.points[0].1.round() as i32);
        match self.tool {
            // After the dot at the front of a badge.
            Tool::Badge => (x + badge_lead(self.number as usize, self.font_px as i32), y + Text::line_height(self.font_px) / 6),
            _ => (x, y),
        }
    }

    /// To be called when anything about its outline has changed.
    pub fn touch(&mut self) {
        *self.mask.borrow_mut() = None;
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
        let (a, b) = (self.points[0], self.points[self.points.len() - 1]);
        (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs())
    }

    pub fn frame_rect(&self) -> Rect {
        let (x, y, w, h) = self.frame();
        Rect::new(x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32)
    }

    /// The rectangle that holds all of it.
    pub fn bounds(&self, text: &mut Text) -> Rect {
        match self.tool {
            Tool::Text => {
                let line = Text::line_height(self.font_px);
                let lines: Vec<&str> = self.text.split('\n').collect();
                let width = lines.iter().map(|line| text.width_in(self.face, line, self.font_px)).max().unwrap_or(0);
                Rect::new(self.points[0].0.round() as i32, self.points[0].1.round() as i32, width.max(self.font_px as i32 / 4), line * lines.len() as i32)
            }
            Tool::Blur | Tool::Spotlight => self.frame_rect(),
            // A badge is as wide as its words, with a dot before them
            // and room at either end.
            Tool::Badge => {
                let line = Text::line_height(self.font_px);
                let width = text.width_in(self.face, &self.text, self.font_px) + badge_lead(self.number as usize, self.font_px as i32) + self.font_px as i32 * 3 / 4;
                Rect::new(self.points[0].0.round() as i32, self.points[0].1.round() as i32, width, line + line / 3)
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
            Tool::Arrow | Tool::Line if self.heads != [false, false] => self.head().0 + self.stroke,
            Tool::Counter => self.disc(),
            Tool::Pen | Tool::Marker => self.line_width() / 2.0,
            _ => self.stroke / 2.0,
        } + 2.0;
        let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in &self.points {
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
        let (a, b) = (self.points[0], self.points[self.points.len() - 1]);
        Outline { tool: self.tool, a, b, stroke: self.stroke, filled: self.filled, radius: self.radius, head: self.head(), heads: self.heads, dashed: self.dashed, disc: self.disc() }
    }
}

/// The few numbers a shape's outline is worked out from, apart from the
/// shape, so that every core can be at it.
#[derive(Clone, Copy)]
struct Outline {
    tool: Tool,
    a: Point,
    b: Point,
    stroke: f32,
    filled: bool,
    radius: f32,
    head: (f32, f32),
    heads: [bool; 2],
    dashed: bool,
    disc: f32,
}

impl Outline {
    /// How far a point is from a line, which if it is dashed is the
    /// dashes of it: each a little longer than the gap after it.
    fn shaft(&self, p: Point, from: Point, to: Point) -> f32 {
        let half = self.stroke / 2.0;
        let solid = segment(p, from, to) - half;
        let length = (to.0 - from.0).hypot(to.1 - from.1);
        if !self.dashed || length < 1.0 {
            return solid;
        }
        let (dash, gap) = (self.stroke * 2.5 + 4.0, self.stroke * 2.0 + 3.0);
        let along = ((p.0 - from.0) * (to.0 - from.0) + (p.1 - from.1) * (to.1 - from.1)) / length;
        let into = along.rem_euclid(dash + gap);
        // Inside a gap, it is as far from the line as from the gap's
        // nearer end; inside a dash, the other way about.
        let outside = if into > dash { (into - dash).min(dash + gap - into) } else { -into.min(dash - into) };
        solid.max(outside)
    }
}

impl Outline {
    /// How far a point is from the outline: negative inside, and nought
    /// on the edge. Not for pen strokes, which are too many lines to ask
    /// of every pixel.
    fn distance(&self, x: f32, y: f32) -> f32 {
        let (a, b) = (self.a, self.b);
        let half = self.stroke / 2.0;
        match self.tool {
            Tool::Line | Tool::Arrow => {
                let length = (b.0 - a.0).hypot(b.1 - a.1);
                if length < 0.5 || self.heads == [false, false] {
                    return self.shaft((x, y), a, b);
                }
                let (dx, dy) = ((b.0 - a.0) / length, (b.1 - a.1) / length);
                let (head, wide) = self.head;
                // A head at an end: its point there, its barbs back
                // along the line. The shaft stops inside it, where its
                // round end would otherwise blunt the point.
                let (mut from, mut to, mut nearest) = (a, b, f32::MAX);
                if self.heads[1] {
                    let base = (b.0 - dx * head, b.1 - dy * head);
                    nearest = nearest.min(triangle((x, y), b, (base.0 - dy * wide, base.1 + dx * wide), (base.0 + dy * wide, base.1 - dx * wide)) - 0.6);
                    to = (b.0 - dx * head * 0.8, b.1 - dy * head * 0.8);
                }
                if self.heads[0] {
                    let base = (a.0 + dx * head, a.1 + dy * head);
                    nearest = nearest.min(triangle((x, y), a, (base.0 - dy * wide, base.1 + dx * wide), (base.0 + dy * wide, base.1 - dx * wide)) - 0.6);
                    from = (a.0 + dx * head * 0.8, a.1 + dy * head * 0.8);
                }
                nearest.min(self.shaft((x, y), from, to))
            }
            Tool::Rect => {
                let (left, top, w, h) = (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs());
                let inside = round_box(x - left - w / 2.0, y - top - h / 2.0, w / 2.0, h / 2.0, self.radius.min(w / 2.0).min(h / 2.0));
                if self.filled { inside - half } else { inside.abs() - half }
            }
            Tool::Ellipse => {
                let (left, top, w, h) = (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs());
                let inside = ellipse(x - left - w / 2.0, y - top - h / 2.0, w / 2.0, h / 2.0);
                if self.filled { inside - half } else { inside.abs() - half }
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
        let bounds = self.extent();
        let mask = match self.tool {
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

    pub fn draw(&self, canvas: &mut Canvas, text: &mut Text) {
        match self.tool {
            // A spotlight is no mark of its own: `paint` sees to it.
            Tool::Select | Tool::Spotlight => {}
            Tool::Blur => canvas.pixelate(self.frame_rect(), (self.stroke * 3.0 + 4.0).round() as i32),
            Tool::Text => {
                let (x, y) = (self.points[0].0.round() as i32, self.points[0].1.round() as i32);
                for (index, line) in self.text.split('\n').enumerate() {
                    text.draw_in(self.face, canvas, x, y + index as i32 * Text::line_height(self.font_px), self.font_px, self.color, line);
                }
            }
            // A pill of the badge's own pale color, a dot and its words
            // in the strong one, and an edge between the two.
            Tool::Badge => {
                let place = self.bounds(text);
                draw_badge(canvas, text, place, self.number as usize, self.color, self.fill, self.face, self.font_px, &self.text);
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
            Tool::Marker => canvas.mask(&self.mask(), self.color, MARKER_OPACITY),
            Tool::Counter => {
                canvas.mask(&self.mask(), self.color, 256);
                let (cx, cy) = (self.points[0].0.round() as i32, self.points[0].1.round() as i32);
                text.draw_centered(canvas, cx, cy, self.font_px, ink_on(self.color), &self.number.to_string());
            }
            _ => canvas.mask(&self.mask(), self.color, 256),
        }
    }

    /// Whether a click at a point is on it.
    pub fn hit(&self, x: i32, y: i32, text: &mut Text) -> bool {
        match self.tool {
            Tool::Text | Tool::Blur | Tool::Spotlight | Tool::Badge => self.bounds(text).inflate(REACH).contains(x, y),
            Tool::Zoom => {
                let (of, shown) = self.zoomed();
                of.inflate(REACH).contains(x, y) || shown.inflate(REACH).contains(x, y)
            }
            _ => {
                let mask = self.mask();
                let near = Rect::new(x - REACH, y - REACH, 2 * REACH + 1, 2 * REACH + 1).intersect(mask.bounds);
                !near.is_empty() && (near.y..near.bottom()).any(|py| {
                    let row = &mask.alpha[((py - mask.bounds.y) * mask.bounds.w + near.x - mask.bounds.x) as usize..][..near.w as usize];
                    row.iter().any(|&cover| cover > 64)
                })
            }
        }
    }
}

/// How many ways a badge can look.
pub const BADGE_STYLES: usize = 6;

/// How far in from its left a badge's words begin, in one of its looks
/// and at a size of letter: after a dot, or a bar, or nothing.
fn badge_lead(style: usize, px: i32) -> i32 {
    match style {
        0 | 4 => px * 5 / 4,
        5 => px,
        _ => px * 3 / 4,
    }
}

/// Draws a badge in a place: words on a patch of color, in one of six
/// looks. `solid` is its hue at full strength and `soft` the pale of
/// it; which of the two is under the words, and what the words are in,
/// is the look.
#[allow(clippy::too_many_arguments)]
pub fn draw_badge(canvas: &mut Canvas, text: &mut Text, place: Rect, style: usize, solid: u32, soft: u32, face: usize, px: u32, words: &str) {
    let (high, size) = (place.h, px as i32);
    let ink = crate::canvas::mix(solid, 0x000000, 70);
    let dot = |canvas: &mut Canvas| canvas.disc(place.x + size * 5 / 8, place.y + high / 2, (size / 5).max(2), solid, 256);
    let color = match style {
        // Soft: the pale under the dark, with a dot and a faint edge.
        0 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 3, crate::canvas::mix(soft, solid, 70), 256);
            canvas.round_fill(place.x + 1, place.y + 1, place.w - 2, high - 2, high / 3 - 1, soft, 256);
            dot(canvas);
            ink
        }
        // Solid: the hue itself, and whichever of white and black reads on it.
        1 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 3, solid, 256);
            ink_on(solid)
        }
        // Outlined: nothing under the words but what was there.
        2 => {
            canvas.round_outline(place.x, place.y, place.w, high, high / 3, (size / 8).max(2), solid);
            solid
        }
        // A pill: solid, with ends as round as it is tall.
        3 => {
            canvas.round_fill(place.x, place.y, place.w, high, high / 2, solid, 256);
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
    text.draw_in(face, canvas, place.x + badge_lead(style, size), place.y + (high - Text::line_height(px)) / 2, px, color, words);
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
        (badge.text, badge.fill, badge.number, badge.face, badge.font_px) = ("two\nlines \\ and a slash".to_owned(), 0xFEEBE7, 4, 2, 26);
        for shape in [arrow, badge] {
            let back = Shape::read(&shape.write()).expect("read back");
            assert_eq!((back.tool, back.color, back.points.clone(), back.text.as_str()), (shape.tool, shape.color, shape.points.clone(), shape.text.as_str()));
            assert_eq!((back.heads, back.dashed, back.fill, back.number, back.face, back.font_px, back.stroke), (shape.heads, shape.dashed, shape.fill, shape.number, shape.face, shape.font_px, shape.stroke));
        }
        assert!(Shape::read("nonsense").is_none());
        assert!(Shape::read("arrow\tzz\t1\t0\t0\t16\t0\t01\t0\tFFFFFF\t0\t1,2 3,4\t").is_none());
    }

    #[test]
    fn shift_makes_squares_and_straight_lines() {
        assert_eq!(constrained(Tool::Rect, (0.0, 0.0), (30.0, -10.0)), (30.0, -30.0));
        let (x, y) = constrained(Tool::Line, (0.0, 0.0), (100.0, 8.0));
        assert!((x - 100.32).abs() < 0.1 && y.abs() < 1e-3);
    }
}
