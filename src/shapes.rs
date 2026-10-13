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
    pub number: u32,
    mask: RefCell<Option<Rc<Mask>>>,
}

impl Shape {
    pub fn new(tool: Tool, color: u32, stroke: f32, at: Point) -> Shape {
        let points = if matches!(tool, Tool::Pen | Tool::Marker | Tool::Text) { vec![at] } else { vec![at, at] };
        Shape { tool, color, stroke, filled: false, radius: 0.0, points, text: String::new(), font_px: 16, face: 0, heads: [false, tool == Tool::Arrow], number: 0, mask: RefCell::new(None) }
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
            Tool::Text => self.text.trim().is_empty(),
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
        Outline { tool: self.tool, a, b, stroke: self.stroke, filled: self.filled, radius: self.radius, head: self.head(), heads: self.heads, disc: self.disc() }
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
    disc: f32,
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
                    return segment((x, y), a, b) - half;
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
                nearest.min(segment((x, y), from, to) - half)
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
            Tool::Text | Tool::Blur | Tool::Spotlight => self.bounds(text).inflate(REACH).contains(x, y),
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

/// Where a drag from `from` should be taken to end, with Shift held:
/// a square, a circle, or a line at one of eight angles.
pub fn constrained(tool: Tool, from: Point, to: Point) -> Point {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    match tool {
        Tool::Rect | Tool::Ellipse | Tool::Blur | Tool::Spotlight => {
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
    fn shift_makes_squares_and_straight_lines() {
        assert_eq!(constrained(Tool::Rect, (0.0, 0.0), (30.0, -10.0)), (30.0, -30.0));
        let (x, y) = constrained(Tool::Line, (0.0, 0.0), (100.0, 8.0));
        assert!((x - 100.32).abs() < 0.1 && y.abs() < 1e-3);
    }
}
