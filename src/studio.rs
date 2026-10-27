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
use crate::editor::{Cursor, Key, Mods, PALETTE};
use crate::frame::{self, Shadow, BACKDROPS};
use crate::shapes::{self, Shape, Tool};
use crate::text::{icons, Text, FACES};

// jog's colors, as the editor has them.
const TEXT_COLOR: u32 = 0xEEEEEE;
const DIM_TEXT_COLOR: u32 = 0x9A9A9A;
const PANEL_COLOR: u32 = 0x1E1E1E;
const EDGE_COLOR: u32 = 0x333333;
const FIELD_COLOR: u32 = 0x111111;
const VIEW_COLOR: u32 = 0x262626;
const ACCENT: u32 = 0x3E63DD;

/// How wide the panels at either side are, and how tall the bar along
/// the top, in pixels of the window.
const LEFT: i32 = 232;
const RIGHT: i32 = 264;
const TOP: i32 = 44;

/// How much larger a picture's shadow is than Tailwind's of its name.
const SHADOW_SIZE: f32 = 2.0;

/// The widths of line there are, in pixels of the picture; and the sizes
/// of text that go with them.
const STROKES: [f32; 4] = [2.0, 3.5, 6.0, 10.0];
const FONT_SIZES: [u32; 4] = [18, 26, 40, 64];

/// The sizes an artboard can be set to at once, by what each is for.
const PRESETS: [(&str, i32, i32); 5] = [("X post", 1200, 675), ("Square", 1080, 1080), ("16:9", 1920, 1080), ("Story", 1080, 1920), ("A4", 1240, 1754)];

/// The tools down the side, with their keys.
const TOOLS: [(Tool, &str, &str); 9] = [
    (Tool::Select, "Move", "V"),
    (Tool::Rect, "Rectangle", "R"),
    (Tool::Ellipse, "Oval", "O"),
    (Tool::Line, "Line", "L"),
    (Tool::Arrow, "Arrow", "A"),
    (Tool::Pen, "Pen", "P"),
    (Tool::Marker, "Marker", "M"),
    (Tool::Text, "Text", "T"),
    (Tool::Counter, "Counter", "N"),
];

/// What a file of the canvas begins with.
const MAGIC: &str = "HOTSHOT CANVAS 1";

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

    /// From a PNG or a JPEG.
    pub fn open(bytes: &[u8]) -> Option<Image> {
        if let Some((width, height, pixels)) = crate::pin::read(bytes) {
            return Some(Image::with(width, height, pixels, bytes.to_vec()));
        }
        let photo = frame::open(bytes)?;
        Some(Image::new(photo.width, photo.height, photo.pixels.iter().map(|pixel| pixel | 0xFF00_0000).collect()))
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
    /// Which of the shadows there are under it: 0 for none, or one more
    /// than where it is in `frame::SHADOWS`.
    pub shadow: usize,
    pub opacity: u32,
    pub flip: (bool, bool),
}

impl Picture {
    pub fn new(image: Arc<Image>, name: String, at: (f32, f32)) -> Picture {
        let (w, h) = (image.width() as f32, image.height() as f32);
        Picture { image, name, cx: at.0, cy: at.1, w, h, angle: 0.0, crop: (0.0, 0.0, w, h), radius: 0.0, shadow: 0, opacity: 256, flip: (false, false) }
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
}

impl Layer {
    fn picture(picture: Picture) -> Layer {
        Layer { kind: Kind::Picture(picture), hidden: false, locked: false }
    }

    fn shape(shape: Shape) -> Layer {
        Layer { kind: Kind::Shape(shape), hidden: false, locked: false }
    }

    /// What the list of layers calls it.
    fn name(&self) -> String {
        match &self.kind {
            Kind::Picture(picture) => picture.name.clone(),
            Kind::Shape(shape) => match shape.tool {
                Tool::Text | Tool::Badge => shape.text.lines().next().unwrap_or("Text").chars().take(24).collect(),
                tool => TOOLS.iter().find(|(known, ..)| *known == tool).map_or("Shape", |(_, name, _)| name).to_owned(),
            },
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
}

/// The artboard: how large, on what, and what is on it, the first layer
/// at the back.
#[derive(Clone)]
pub struct Doc {
    pub width: i32,
    pub height: i32,
    pub background: Fill,
    pub layers: Vec<Layer>,
}

/// A shape made larger or smaller by a factor, about the artboard's top
/// left: how one is drawn into a picture exported at twice the size.
fn scaled(shape: &Shape, times: f32) -> Shape {
    let mut shape = shape.clone();
    for point in &mut shape.points {
        *point = (point.0 * times, point.1 * times);
    }
    (shape.stroke, shape.radius) = (shape.stroke * times, shape.radius * times);
    shape.font_px = ((shape.font_px as f32 * times).round() as u32).max(1);
    shape.touch();
    shape
}

impl Doc {
    pub fn new(width: i32, height: i32) -> Doc {
        Doc { width, height, background: Fill::Color(0xFFFFFF), layers: Vec::new() }
    }

    /// The artboard as a picture, `times` its size: each pixel with how
    /// opaque it is in its top byte, which with nothing behind is not at
    /// all where nothing is on it.
    pub fn render(&self, times: f32, text: &mut Text) -> (i32, i32, Vec<u32>) {
        let (wide, high) = (((self.width as f32 * times).round() as i32).max(1), ((self.height as f32 * times).round() as i32).max(1));
        let mut out = vec![0u32; (wide * high) as usize];
        match self.background {
            Fill::None => {}
            Fill::Color(color) => out.iter_mut().for_each(|pixel| *pixel = 0xFF00_0000 | color),
            Fill::Backdrop(which) => out.par_chunks_mut(wide as usize).enumerate().for_each(|(y, row)| {
                for (x, pixel) in row.iter_mut().enumerate() {
                    let noise = ((x * 7 + y * 13) % 16) as f32 / 16.0;
                    *pixel = 0xFF00_0000 | frame::shade(which, (x as f32 + 0.5) / wide as f32, (y as f32 + 0.5) / high as f32, noise);
                }
            }),
        }
        for layer in self.layers.iter().filter(|layer| !layer.hidden) {
            match &layer.kind {
                Kind::Picture(picture) => draw_picture(&mut out, wide, high, picture, times),
                Kind::Shape(shape) => {
                    let shape = if times == 1.0 { shape.clone() } else { scaled(shape, times) };
                    draw_shape(&mut out, wide, high, &shape, text);
                }
            }
        }
        (wide, high, out)
    }

    /// The canvas as a file: what it is, line by line, and each picture
    /// on it as the PNG it is, after its own line saying how long it is.
    pub fn write(&self) -> Vec<u8> {
        let mut out = format!("{MAGIC}\nsize {} {}\n", self.width, self.height);
        out += &match self.background {
            Fill::None => "background none\n".to_owned(),
            Fill::Color(color) => format!("background color {color:06X}\n"),
            Fill::Backdrop(which) => format!("background backdrop {}\n", BACKDROPS[which.min(BACKDROPS.len() - 1)].0),
        };
        let mut images: Vec<Arc<Image>> = Vec::new();
        for layer in &self.layers {
            let flags = format!("{} {}", u8::from(layer.hidden), u8::from(layer.locked));
            match &layer.kind {
                Kind::Picture(picture) => {
                    let index = images.iter().position(|image| Arc::ptr_eq(image, &picture.image)).unwrap_or_else(|| {
                        images.push(picture.image.clone());
                        images.len() - 1
                    });
                    let (cx, cy, cw, ch) = picture.crop;
                    out += &format!(
                        "picture {flags} {index} {} {} {} {} {} {cx} {cy} {cw} {ch} {} {} {} {} {} {}\n",
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
                        picture.name.replace(['\n', '\t'], " ")
                    );
                }
                Kind::Shape(shape) => out += &format!("shape {flags} {}\n", shape.write()),
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

    /// A canvas from a file `write` made, or nothing from any other.
    pub fn read(bytes: &[u8]) -> Option<Doc> {
        // The lines come first, the pictures after: the lines are read
        // up to the first picture, which is where the bytes begin.
        let text_end = find(bytes, b"\nimage ").map_or(bytes.len(), |at| at + 1);
        let text = std::str::from_utf8(&bytes[..text_end]).ok()?;
        let mut lines = text.lines();
        if lines.next()? != MAGIC {
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
        let mut doc = Doc::new(1, 1);
        for line in lines {
            let mut words = line.split(' ');
            match words.next()? {
                "size" => (doc.width, doc.height) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?),
                "background" => {
                    doc.background = match words.next()? {
                        "color" => Fill::Color(u32::from_str_radix(words.next()?, 16).ok()?),
                        "backdrop" => Fill::Backdrop(frame::named(words.next()?)?),
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
                    let name = words.collect::<Vec<_>>().join(" ");
                    let picture = Picture { image, name, cx, cy, w, h, angle, crop, radius, shadow: shadow.min(frame::SHADOWS.len()), opacity: opacity.min(256), flip };
                    doc.layers.push(Layer { kind: Kind::Picture(picture), hidden, locked });
                }
                "shape" => {
                    let rest = line.splitn(4, ' ').collect::<Vec<_>>();
                    let shape = Shape::read(rest.get(3)?)?;
                    doc.layers.push(Layer { kind: Kind::Shape(shape), hidden: rest[1] == "1", locked: rest[2] == "1" });
                }
                _ => {}
            }
        }
        (doc.width > 0 && doc.height > 0).then_some(doc)
    }
}

/// A path as said to someone: with the home folder as ~.
fn shortened(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// A canvas of a picture from the editor, and the shapes drawn on it,
/// written as the editor writes them, still to be moved and changed:
/// the picture on a backdrop, with room round it, and the shapes moved
/// from where they were on the screen to where it is on the artboard.
pub fn from_markup(area: Rect, pixels: &[u32], shapes: &str) -> Doc {
    let room = 80;
    let mut doc = Doc::new(area.w + 2 * room, area.h + 2 * room);
    doc.background = Fill::Backdrop(frame::named("aurora").unwrap_or(0));
    let image = Image::new(area.w, area.h, pixels.iter().map(|pixel| pixel | 0xFF00_0000).collect());
    let middle = (room as f32 + area.w as f32 / 2.0, room as f32 + area.h as f32 / 2.0);
    let mut picture = Picture::new(Arc::new(image), "Screenshot".into(), middle);
    (picture.radius, picture.shadow) = (10.0, frame::SHADOWS.len());
    doc.layers.push(Layer::picture(picture));
    for mut shape in shapes.lines().filter_map(Shape::read) {
        shape.translate((room - area.x) as f32, (room - area.y) as f32);
        doc.layers.push(Layer::shape(shape));
    }
    doc
}

/// Where some bytes first come in others.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
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
/// the size it is shown at, kept to its rounded corners.
fn draw_picture(out: &mut [u32], wide: i32, high: i32, picture: &Picture, times: f32) {
    let mut at = picture.clone();
    (at.cx, at.cy, at.w, at.h, at.radius) = (picture.cx * times, picture.cy * times, picture.w * times, picture.h * times, picture.radius * times);
    let picture = &at;
    let shadow = Shadow::of_preset(picture.shadow.saturating_sub(1));
    let layers = if picture.shadow == 0 { &[][..] } else { shadow.layers() };
    // Tailwind's shadows are for a page seen at its size; an artboard is
    // most often seen at half its own, and its shadows are twice as large.
    let lift = times * SHADOW_SIZE;
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
    let radius = picture.radius.clamp(0.0, hw.min(hh));
    let (cx, cy, cw, ch) = picture.crop;
    let level = picture.image.level(cw / picture.w);
    let (lw, lh, pixels) = (level.0, level.1, &level.2);
    let (ratio_x, ratio_y) = (lw as f32 / picture.image.width() as f32, lh as f32 / picture.image.height() as f32);
    let opacity = picture.opacity as f32 / 256.0;
    out[(area.y * wide) as usize..(area.bottom() * wide) as usize].par_chunks_mut(wide as usize).enumerate().for_each(|(row, line)| {
        let y = area.y + row as i32;
        for x in area.x..area.right() {
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            let mut pixel = line[x as usize];
            for layer in layers.iter().rev() {
                let shifted = (point.0 - f32::from(layer.x) * lift, point.1 - f32::from(layer.y) * lift);
                let (sx, sy) = picture.local(shifted);
                let grow = f32::from(layer.spread) * lift;
                let away = frame::round_box(sx, sy, (hw + grow).max(0.0), (hh + grow).max(0.0), (radius + grow).max(0.0));
                let deviation = f32::from(layer.blur.max(0)) * lift / 2.0;
                let near = if deviation < 0.5 { (0.5 - away).clamp(0.0, 1.0) } else { 0.5 * (1.0 - frame::erf(away / (deviation * std::f32::consts::SQRT_2))) };
                pixel = over(pixel, layer.color & 0xFF_FFFF, near * (layer.color >> 24) as f32 / 255.0 * opacity);
            }
            let (lx, ly) = picture.local(point);
            let inside = frame::round_box(lx, ly, hw, hh, radius);
            let cover = (0.5 - inside).clamp(0.0, 1.0);
            if cover > 0.0 {
                // Where in the picture's own pixels, flipped if it is.
                let (mut u, mut v) = ((lx + hw) / picture.w, (ly + hh) / picture.h);
                if picture.flip.0 {
                    u = 1.0 - u;
                }
                if picture.flip.1 {
                    v = 1.0 - v;
                }
                let (fx, fy) = ((cx + u * cw) * ratio_x - 0.5, (cy + v * ch) * ratio_y - 0.5);
                let color = sample(pixels, lw, lh, fx, fy);
                pixel = over(pixel, color & 0xFF_FFFF, cover * (color >> 24) as f32 / 255.0 * opacity);
            }
            line[x as usize] = pixel;
        }
    });
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

// ---- The canvas ----

/// What is being done with the button down.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    None,
    /// The view moved, by the hand, the middle button or Space.
    Pan { from: (i32, i32), origin: (f32, f32) },
    /// What is picked out, moved: `moved` once it has gone anywhere.
    Move { from: (f32, f32), moved: bool },
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
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Field::X => "X",
            Field::Y => "Y",
            Field::W | Field::ArtW => "W",
            Field::H | Field::ArtH => "H",
            Field::Turn => "\u{b0}",
            Field::Radius => "Round",
            Field::Opacity => "Opacity",
        }
    }
}

/// What the background is: nothing, a color, a backdrop.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ground {
    None,
    Color,
    Backdrop,
}

/// What a color in the panel is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Paint {
    Line,
    Fill,
    Background,
}

/// What a click on the panels, or the bar along the top, does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Control {
    Tool(Tool),
    Layer(usize),
    Eye(usize),
    Lock(usize),
    Field(Field),
    Preset(usize),
    Ground(Ground),
    Backdrop(usize),
    Swatch(Paint, Option<u32>),
    Scale(u32),
    Shadow(usize),
    Width(usize),
    Head(usize, u8),
    Line(u8),
    Face(usize),
    Action(Action),
    /// A part of a panel that is no control.
    Panel,
}

/// The buttons of the panels and the bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    Export,
    Copy,
    Save,
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
    Border,
    Halo,
    Elbow,
    Sketch,
    Fit,
}

impl Action {
    fn says(self) -> &'static str {
        match self {
            Action::Export => "Export",
            Action::Copy => "Copy",
            Action::Save => "Save",
            Action::Capture => "Capture",
            Action::Crop => "Crop",
            Action::FlipAcross => "Flip \u{2194}",
            Action::FlipDown => "Flip \u{2195}",
            Action::Actual => "100%",
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
            Action::Border => "Border",
            Action::Halo => "White edge",
            Action::Elbow => "Elbow",
            Action::Sketch => "Sketch",
            Action::Fit => "Fit",
        }
    }
}

/// Where everything is, for one frame.
#[derive(Default)]
struct Layout {
    view: Rect,
    controls: Vec<(Rect, Control)>,
    /// What each control is, said where it is drawn: the panels' headings
    /// and labels, which are no controls.
    words: Vec<((i32, i32), String, u32)>,
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
}

/// The artboard, the layers, and everything about the window they are in.
pub struct Studio {
    doc: Doc,
    text: Text,
    width: i32,
    height: i32,
    zoom: f32,
    /// Where the artboard's top left corner is, in the view.
    pan: (f32, f32),
    tool: Tool,
    selected: Vec<usize>,
    hovered: Option<usize>,
    drag: Drag,
    pointer: (i32, i32),
    mods: Mods,
    /// Space held: the hand, for as long as it is.
    space: bool,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    /// The layers as they were when something was taken hold of, to
    /// change from rather than from what the last move left.
    held: Vec<Layer>,
    /// The text being typed into, by its layer.
    editing: Option<usize>,
    /// The picture being cropped, by its layer.
    cropping: Option<usize>,
    /// How the next shape is drawn.
    color: u32,
    fill: Option<u32>,
    size: usize,
    face: usize,
    /// How much larger a picture is exported than the artboard.
    export_scale: u32,
    /// A number being typed into its field, and what has been typed.
    typing: Option<(Field, String)>,
    /// The artboard as drawn last, and at the size it is shown, each with
    /// the change it was drawn at.
    version: u64,
    rendered: Option<(u64, i32, i32, Vec<u32>)>,
    shown: Option<(u64, i32, i32, Vec<u32>)>,
    /// Where it is kept, if it is; and whether it has changed since.
    pub path: Option<PathBuf>,
    edited: bool,
    /// Where pictures go, and what they are called.
    dir: PathBuf,
    filename: String,
    /// What was last done, said along the bottom for a while.
    note: Option<(String, std::time::Instant)>,
    last_press: (u32, (i32, i32)),
    dirty: bool,
}

impl Studio {
    pub fn new(doc: Doc, text: Text, width: i32, height: i32, dir: PathBuf, filename: String) -> Studio {
        let mut studio = Studio {
            doc,
            text,
            width,
            height,
            zoom: 1.0,
            pan: (0.0, 0.0),
            tool: Tool::Select,
            selected: Vec::new(),
            hovered: None,
            drag: Drag::None,
            pointer: (0, 0),
            mods: Mods::default(),
            space: false,
            undo: Vec::new(),
            redo: Vec::new(),
            held: Vec::new(),
            editing: None,
            cropping: None,
            color: 0xE5484D,
            fill: None,
            size: 1,
            face: 0,
            export_scale: 2,
            typing: None,
            version: 0,
            rendered: None,
            shown: None,
            path: None,
            edited: false,
            dir,
            filename,
            note: None,
            last_press: (0, (-100, -100)),
            dirty: true,
        };
        studio.fit();
        studio
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        if (width, height) != (self.width, self.height) {
            let before = self.layout().view;
            (self.width, self.height) = (width.max(400), height.max(300));
            // The artboard stays where it was in the middle of the view.
            let after = self.layout().view;
            self.pan = (self.pan.0 + (after.w - before.w) as f32 / 2.0, self.pan.1 + (after.h - before.h) as f32 / 2.0);
            self.dirty = true;
        }
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty) || self.note.as_ref().is_some_and(|(_, when)| when.elapsed().as_secs_f32() < 3.2)
    }

    /// What the window is called: the file, and whether it has changed.
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|path| path.file_name()).map_or("Untitled".to_owned(), |name| name.to_string_lossy().into_owned());
        format!("{name}{} \u{2014} hotshot", if self.edited { " \u{2022}" } else { "" })
    }

    fn say(&mut self, what: impl Into<String>) {
        self.note = Some((what.into(), std::time::Instant::now()));
        self.dirty = true;
    }

    /// Something about to change: kept, to come back to with undo.
    fn checkpoint(&mut self) {
        self.undo.push(self.doc.clone());
        self.redo.clear();
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }

    /// Something has changed: drawn again, and the file is behind.
    fn changed(&mut self) {
        self.version += 1;
        self.edited = true;
        self.dirty = true;
    }

    // ---- The view ----

    fn layout(&mut self) -> Layout {
        let view = Rect::new(LEFT, TOP, (self.width - LEFT - RIGHT).max(100), (self.height - TOP).max(100));
        let mut layout = Layout { view, ..Layout::default() };
        let px = 13;
        let line = Text::line_height(px);
        // Along the top: the tools, then the zoom and what the picture
        // can be done with.
        let side = 30;
        let mut x = 12;
        for (tool, ..) in TOOLS {
            layout.controls.push((Rect::new(x, (TOP - side) / 2, side, side), Control::Tool(tool)));
            x += side + 2;
        }
        let mut right = self.width - 12;
        for action in [Action::Export, Action::Copy, Action::Save, Action::Capture, Action::Fit] {
            let wide = self.text.width(action.says(), px) + 20;
            right -= wide;
            layout.controls.push((Rect::new(right, (TOP - side) / 2, wide, side), Control::Action(action)));
            right -= 6;
        }
        // Down the left: the layers, the topmost first.
        let mut y = TOP + 14;
        layout.words.push(((14, y), "Layers".to_owned(), DIM_TEXT_COLOR));
        y += line + 6;
        let row = 30;
        for index in (0..self.doc.layers.len()).rev() {
            if y + row > self.height - 8 {
                break;
            }
            layout.controls.push((Rect::new(LEFT - 56, y, 22, row), Control::Eye(index)));
            layout.controls.push((Rect::new(LEFT - 30, y, 22, row), Control::Lock(index)));
            layout.controls.push((Rect::new(8, y, LEFT - 16, row), Control::Layer(index)));
            y += row;
        }
        layout.controls.push((Rect::new(0, TOP, LEFT, self.height - TOP), Control::Panel));
        // Down the right: the properties of what is picked out, or of
        // the artboard.
        self.properties(&mut layout);
        layout.controls.push((Rect::new(self.width - RIGHT, TOP, RIGHT, self.height - TOP), Control::Panel));
        layout.controls.push((Rect::new(0, 0, self.width, TOP), Control::Panel));
        layout
    }

    /// The panel of properties, laid out a row at a time.
    fn properties(&mut self, layout: &mut Layout) {
        let px = 13;
        let line = Text::line_height(px);
        let (left, inner) = (self.width - RIGHT + 14, RIGHT - 28);
        let mut y = TOP + 14;
        let heading = |layout: &mut Layout, y: &mut i32, says: &str| {
            *y += 6;
            layout.words.push(((left, *y), says.to_owned(), DIM_TEXT_COLOR));
            *y += line + 6;
        };
        // Two numbers to a row, each with its label to drag it by.
        let fields = |layout: &mut Layout, y: &mut i32, pair: &[Field]| {
            let each = (inner - 8) / 2;
            for (index, &field) in pair.iter().enumerate() {
                layout.controls.push((Rect::new(left + index as i32 * (each + 8), *y, each, 28), Control::Field(field)));
            }
            *y += 34;
        };
        // Chips, as many to a row as fit, each as wide as what it says.
        let chips = |layout: &mut Layout, text: &mut Text, y: &mut i32, items: Vec<(String, Control)>| {
            let mut x = left;
            for (says, control) in items {
                let wide = text.width(&says, px) + 18;
                if x + wide > left + inner && x > left {
                    (x, *y) = (left, *y + 32);
                }
                layout.controls.push((Rect::new(x, *y, wide, 28), control));
                layout.words.push(((x + 9, *y + (28 - line) / 2), says, TEXT_COLOR));
                x += wide + 6;
            }
            *y += 34;
        };
        let swatches = |layout: &mut Layout, y: &mut i32, paint: Paint, none: bool| {
            // Nine to a row, the none and the palette's twenty-five in three.
            let each = (inner + 3) / 9 - 3;
            let mut x = left;
            let colors: Vec<Option<u32>> = none.then_some(None).into_iter().chain(PALETTE.iter().map(|&(color, _)| Some(color))).collect();
            for color in colors {
                if x + each > left + inner {
                    (x, *y) = (left, *y + each + 4);
                }
                layout.controls.push((Rect::new(x, *y, each, each), Control::Swatch(paint, color)));
                x += each + 3;
            }
            *y += each + 10;
        };
        if let Some(index) = self.cropping {
            heading(layout, &mut y, "Crop");
            layout.words.push(((left, y), "Drag the corners; Enter or Esc".to_owned(), TEXT_COLOR));
            y += line + 10;
            chips(layout, &mut self.text, &mut y, vec![("Done".to_owned(), Control::Action(Action::Crop))]);
            let _ = index;
            return;
        }
        let chosen = self.chosen();
        match chosen[..] {
            [] => {
                heading(layout, &mut y, "Artboard");
                let presets = PRESETS.iter().enumerate().map(|(index, (name, ..))| (name.to_string(), Control::Preset(index))).collect();
                chips(layout, &mut self.text, &mut y, presets);
                fields(layout, &mut y, &[Field::ArtW, Field::ArtH]);
                heading(layout, &mut y, "Background");
                chips(layout, &mut self.text, &mut y, vec![("None".into(), Control::Ground(Ground::None)), ("Color".into(), Control::Ground(Ground::Color)), ("Backdrop".into(), Control::Ground(Ground::Backdrop))]);
                match self.doc.background {
                    Fill::Color(_) => swatches(layout, &mut y, Paint::Background, false),
                    Fill::Backdrop(_) => {
                        let (each, across) = (inner / 6, 6);
                        for which in 0..24.min(BACKDROPS.len()) {
                            layout.controls.push((Rect::new(left + which as i32 % across * each, y + which as i32 / across * each, each - 4, each - 4), Control::Backdrop(which)));
                        }
                        y += 4 * each + 6;
                    }
                    Fill::None => {}
                }
                heading(layout, &mut y, "Export");
                chips(layout, &mut self.text, &mut y, (1..=3).map(|times| (format!("{times}\u{d7}"), Control::Scale(times))).collect());
                chips(layout, &mut self.text, &mut y, [Action::Export, Action::Copy, Action::Save].iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect());
            }
            [index] => match self.doc.layers[index].kind.clone() {
                Kind::Picture(_) => {
                    heading(layout, &mut y, "Picture");
                    fields(layout, &mut y, &[Field::X, Field::Y]);
                    fields(layout, &mut y, &[Field::W, Field::H]);
                    fields(layout, &mut y, &[Field::Turn, Field::Radius]);
                    fields(layout, &mut y, &[Field::Opacity]);
                    heading(layout, &mut y, "Shadow");
                    let shadows = (0..=frame::SHADOWS.len()).map(|which| (if which == 0 { "None".to_owned() } else { frame::SHADOWS[which - 1].0.to_owned() }, Control::Shadow(which))).collect();
                    chips(layout, &mut self.text, &mut y, shadows);
                    heading(layout, &mut y, "Picture");
                    chips(layout, &mut self.text, &mut y, [Action::Crop, Action::FlipAcross, Action::FlipDown, Action::Actual].iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect());
                    self.layer_controls(layout, &mut y, &chips, &heading);
                }
                Kind::Shape(shape) => {
                    heading(layout, &mut y, "Shape");
                    fields(layout, &mut y, &[Field::X, Field::Y]);
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        fields(layout, &mut y, &[Field::W, Field::H]);
                    }
                    if shape.turnable() {
                        fields(layout, &mut y, &[Field::Turn, Field::Opacity]);
                    } else {
                        fields(layout, &mut y, &[Field::Opacity]);
                    }
                    let paintable = matches!(shape.tool, Tool::Rect | Tool::Ellipse);
                    heading(layout, &mut y, if paintable { "Border" } else { "Color" });
                    swatches(layout, &mut y, Paint::Line, false);
                    if paintable {
                        heading(layout, &mut y, "Fill");
                        swatches(layout, &mut y, Paint::Fill, true);
                        chips(layout, &mut self.text, &mut y, vec![("Border".into(), Control::Action(Action::Border)), ("Sketch".into(), Control::Action(Action::Sketch))]);
                    }
                    match shape.tool {
                        Tool::Text | Tool::Badge => {
                            heading(layout, &mut y, "Font");
                            chips(layout, &mut self.text, &mut y, FACES.iter().enumerate().map(|(face, name)| (name.split(' ').next().unwrap_or(name).to_owned(), Control::Face(face))).collect());
                            heading(layout, &mut y, "Size");
                            chips(layout, &mut self.text, &mut y, FONT_SIZES.iter().enumerate().map(|(size, px)| (px.to_string(), Control::Width(size))).collect());
                        }
                        _ => {
                            heading(layout, &mut y, "Width");
                            chips(layout, &mut self.text, &mut y, STROKES.iter().enumerate().map(|(size, width)| (format!("{width}"), Control::Width(size))).collect());
                        }
                    }
                    if matches!(shape.tool, Tool::Line | Tool::Arrow) {
                        let heads = |end: usize| -> Vec<(String, Control)> {
                            let none = shapes::HEADS.len() as u8;
                            [none].into_iter().chain(0..none).map(|style| (if style == none { "None".to_owned() } else { shapes::HEADS[usize::from(style)].to_owned() }, Control::Head(end, style))).collect()
                        };
                        heading(layout, &mut y, "Start");
                        chips(layout, &mut self.text, &mut y, heads(0));
                        heading(layout, &mut y, "End");
                        chips(layout, &mut self.text, &mut y, heads(1));
                        heading(layout, &mut y, "Line");
                        chips(layout, &mut self.text, &mut y, ["Solid", "Dashed", "Dotted"].iter().enumerate().map(|(kind, name)| (name.to_string(), Control::Line(kind as u8))).collect());
                        chips(layout, &mut self.text, &mut y, [Action::Halo, Action::Elbow, Action::Sketch].iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect());
                    }
                    self.layer_controls(layout, &mut y, &chips, &heading);
                }
            },
            _ => {
                heading(layout, &mut y, &format!("{} layers", chosen.len()));
                fields(layout, &mut y, &[Field::X, Field::Y]);
                heading(layout, &mut y, "Line up");
                chips(layout, &mut self.text, &mut y, (0..6).map(|way| (Action::Align(way).says().to_owned(), Control::Action(Action::Align(way)))).collect());
                self.layer_controls(layout, &mut y, &chips, &heading);
            }
        }
    }

    /// What can be done with any layer: over and under the rest, a copy,
    /// and away.
    fn layer_controls(&mut self, layout: &mut Layout, y: &mut i32, chips: &dyn Fn(&mut Layout, &mut Text, &mut i32, Vec<(String, Control)>), heading: &dyn Fn(&mut Layout, &mut i32, &str)) {
        heading(layout, y, "Layer");
        let items = [Action::Forward, Action::Backward, Action::Duplicate, Action::Delete].iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect();
        chips(layout, &mut self.text, y, items);
    }

    /// The artboard made to fit the view, with room round it.
    fn fit(&mut self) {
        let view = Rect::new(LEFT, TOP, (self.width - LEFT - RIGHT).max(100), (self.height - TOP).max(100));
        self.zoom = (view.w as f32 * 0.86 / self.doc.width as f32).min(view.h as f32 * 0.86 / self.doc.height as f32).clamp(0.02, 8.0);
        self.pan = ((view.w as f32 - self.doc.width as f32 * self.zoom) / 2.0, (view.h as f32 - self.doc.height as f32 * self.zoom) / 2.0);
        self.dirty = true;
    }

    /// Larger or smaller, about a point of the window, which stays over
    /// the same point of the artboard.
    fn zoom_about(&mut self, times: f32, at: (i32, i32)) {
        let view = self.layout().view;
        let before = self.to_doc(at);
        self.zoom = (self.zoom * times).clamp(0.02, 16.0);
        self.pan = ((at.0 - view.x) as f32 - before.0 * self.zoom, (at.1 - view.y) as f32 - before.1 * self.zoom);
        self.dirty = true;
    }

    /// A point of the window as a point of the artboard, and back.
    fn to_doc(&self, (x, y): (i32, i32)) -> (f32, f32) {
        (((x - LEFT) as f32 + 0.5 - self.pan.0) / self.zoom, ((y - TOP) as f32 + 0.5 - self.pan.1) / self.zoom)
    }

    fn to_window(&self, (x, y): (f32, f32)) -> (i32, i32) {
        ((x * self.zoom + self.pan.0).round() as i32 + LEFT, (y * self.zoom + self.pan.1).round() as i32 + TOP)
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
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse) && shape.angle == 0.0 => {
                let (x, y, w, h) = shape.frame();
                pulls.iter().map(|&(px, py)| (self.to_window((x + w * (px + 1) as f32 / 2.0, y + h * (py + 1) as f32 / 2.0)), (px, py))).collect()
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
        match self.drag {
            Drag::Pan { .. } => return Cursor::Move,
            Drag::Turn { .. } => return Cursor::Turn,
            Drag::Draw { .. } | Drag::Marquee { .. } => return Cursor::Crosshair,
            Drag::Scrub { .. } => return Cursor::Across,
            _ => {}
        }
        if self.space {
            return Cursor::Move;
        }
        let view = Rect::new(LEFT, TOP, self.width - LEFT - RIGHT, self.height - TOP);
        if !view.contains(self.pointer.0, self.pointer.1) {
            return Cursor::Arrow;
        }
        match self.tool {
            Tool::Select => Cursor::Arrow,
            Tool::Text => Cursor::Text,
            _ => Cursor::Crosshair,
        }
    }

    // ---- The pointer ----

    pub fn pointer_moved(&mut self, x: i32, y: i32, mods: Mods) {
        (self.pointer, self.mods) = ((x, y), mods);
        let at = self.to_doc((x, y));
        match self.drag {
            Drag::None => {
                let hovered = match (self.tool, self.layout().view.contains(x, y)) {
                    (Tool::Select, true) => self.layer_at(at),
                    _ => None,
                };
                self.dirty |= hovered != self.hovered;
                self.hovered = hovered;
                self.dirty = true;
            }
            Drag::Pan { from, origin } => {
                self.pan = (origin.0 + (x - from.0) as f32, origin.1 + (y - from.1) as f32);
                self.dirty = true;
            }
            Drag::Move { from, moved } => {
                let (mut dx, mut dy) = (at.0 - from.0, at.1 - from.1);
                if mods.shift {
                    if dx.abs() >= dy.abs() { dy = 0.0 } else { dx = 0.0 }
                }
                if !moved && (dx.abs() + dy.abs()) * self.zoom < 3.0 {
                    return;
                }
                if !moved {
                    self.checkpoint();
                    self.drag = Drag::Move { from, moved: true };
                }
                for (index, layer) in self.held.clone().into_iter().enumerate() {
                    if let Some(&chosen) = self.chosen().get(index) {
                        let mut layer = layer;
                        layer.translate(dx.round(), dy.round());
                        self.doc.layers[chosen] = layer;
                    }
                }
                self.changed();
            }
            Drag::Scale { pull, from } => self.scale_to(pull, from, at, mods),
            Drag::Turn { about, began } => {
                let Some(index) = self.picked() else { return };
                let mut by = (at.1 - about.1).atan2(at.0 - about.0) - began;
                let mut layer = self.held[0].clone();
                if mods.shift {
                    let step = 15f32.to_radians();
                    let start = match &layer.kind {
                        Kind::Picture(picture) => picture.angle,
                        Kind::Shape(shape) => shape.angle,
                    };
                    by = ((start + by) / step).round() * step - start;
                }
                match &mut layer.kind {
                    Kind::Picture(picture) => picture.angle += by,
                    Kind::Shape(shape) => shape.turn(by, about, &mut self.text),
                }
                self.doc.layers[index] = layer;
                self.changed();
            }
            Drag::Draw { from } => {
                let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.last_mut() else { return };
                match shape.tool {
                    Tool::Pen | Tool::Marker => shape.points.push(at),
                    tool => {
                        let to = if mods.shift { shapes::constrained(tool, from, at) } else { at };
                        shape.points[1] = to;
                    }
                }
                shape.touch();
                self.changed();
            }
            Drag::Marquee { .. } => self.dirty = true,
            Drag::Scrub { field, from, start } => {
                let step = match field {
                    Field::Turn => 0.5,
                    Field::Opacity => 1.0,
                    _ => 1.0,
                } * if mods.shift { 10.0 } else { 1.0 };
                self.set_field(field, start + (x - from) as f32 * step);
            }
            Drag::Crop { pull, from } => self.crop_to(pull, from, at),
        }
    }

    pub fn pressed(&mut self, button: u8, time: u32) -> Outcome {
        let at_window = self.pointer;
        let at = self.to_doc(at_window);
        let double = time.wrapping_sub(self.last_press.0) < 400 && (at_window.0 - self.last_press.1 .0).abs().max((at_window.1 - self.last_press.1 .1).abs()) <= 3;
        self.last_press = (time, at_window);
        self.dirty = true;
        match button {
            // The wheel: up and down, or with Shift across; with Ctrl,
            // larger and smaller about the pointer.
            4 | 5 | 6 | 7 => {
                let up = matches!(button, 4 | 6);
                if self.mods.ctrl {
                    self.zoom_about(if up { 1.15 } else { 1.0 / 1.15 }, at_window);
                } else if self.mods.shift || button > 5 {
                    self.pan.0 += if up { 60.0 } else { -60.0 };
                } else {
                    self.pan.1 += if up { 60.0 } else { -60.0 };
                }
                return Outcome::Continue;
            }
            2 => {
                self.drag = Drag::Pan { from: at_window, origin: self.pan };
                return Outcome::Continue;
            }
            1 => {}
            _ => return Outcome::Continue,
        }
        if self.typing.is_some() && self.control_at(at_window) != self.typing.as_ref().map(|(field, _)| Control::Field(*field)) {
            self.commit_typing();
        }
        let control = self.control_at(at_window);
        if let Some(control) = control {
            return self.press_control(control, double);
        }
        // In the view.
        if self.space || self.tool == Tool::Select && self.mods.alt {
            self.drag = Drag::Pan { from: at_window, origin: self.pan };
            return Outcome::Continue;
        }
        if let Some(index) = self.cropping {
            return self.press_crop(index, at);
        }
        if let Some(index) = self.editing {
            if self.doc.layers[index].hit(at, &mut self.text) {
                return Outcome::Continue;
            }
            self.commit_text();
        }
        match self.tool {
            Tool::Select => self.press_select(at, at_window, double),
            Tool::Text => {
                self.checkpoint();
                let mut shape = Shape::new(Tool::Text, self.color, STROKES[self.size], (at.0, at.1 - FONT_SIZES[self.size] as f32 / 2.0));
                (shape.font_px, shape.face) = (FONT_SIZES[self.size], self.face);
                self.doc.layers.push(Layer::shape(shape));
                self.editing = Some(self.doc.layers.len() - 1);
                self.selected = vec![self.doc.layers.len() - 1];
                self.changed();
            }
            tool => {
                self.checkpoint();
                let mut shape = Shape::new(tool, self.color, STROKES[self.size], at);
                if matches!(tool, Tool::Rect | Tool::Ellipse) {
                    (shape.filled, shape.fill) = (self.fill.is_some(), self.fill.unwrap_or(0xFFFFFF));
                }
                shape.number = self.doc.layers.iter().filter(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Counter)).count() as u32 + 1;
                shape.font_px = FONT_SIZES[self.size];
                self.doc.layers.push(Layer::shape(shape));
                self.selected.clear();
                self.drag = Drag::Draw { from: at };
                self.changed();
            }
        }
        Outcome::Continue
    }

    fn press_select(&mut self, at: (f32, f32), at_window: (i32, i32), double: bool) {
        // The handles of the one picked out, and just outside its corners
        // to turn it, come before what is under them.
        let near = |a: (i32, i32), b: (i32, i32), reach: i32| (a.0 - b.0).abs() <= reach && (a.1 - b.1).abs() <= reach;
        let handles = self.handles();
        if let Some(&(_, pull)) = handles.iter().find(|&&(spot, _)| near(spot, at_window, 6)) {
            self.checkpoint();
            self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
            self.drag = Drag::Scale { pull, from: at };
            return;
        }
        if let Some(index) = self.picked() {
            let corners: Vec<(i32, i32)> = handles.iter().take(4).map(|&(spot, _)| spot).collect();
            let turnable = match &self.doc.layers[index].kind {
                Kind::Picture(_) => true,
                Kind::Shape(shape) => shape.turns(),
            };
            if turnable && corners.iter().any(|&corner| near(corner, at_window, 18) && !near(corner, at_window, 6)) {
                self.checkpoint();
                self.held = vec![self.doc.layers[index].clone()];
                let about = match &self.doc.layers[index].kind {
                    Kind::Picture(picture) => (picture.cx, picture.cy),
                    Kind::Shape(shape) => shape.centre(&mut self.text),
                };
                self.drag = Drag::Turn { about, began: (at.1 - about.1).atan2(at.0 - about.0) };
                return;
            }
        }
        match self.layer_at(at) {
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
                        Kind::Picture(_) => self.cropping = Some(index),
                        Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge) => {
                            self.checkpoint();
                            self.editing = Some(index);
                        }
                        Kind::Shape(_) => {}
                    }
                    return;
                }
                self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
                self.drag = Drag::Move { from: at, moved: false };
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
    /// crop, that; inside it, all of it; outside, the end of cropping.
    fn press_crop(&mut self, index: usize, at: (f32, f32)) -> Outcome {
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
        self.checkpoint();
        self.held = vec![self.doc.layers[index].clone()];
        self.drag = Drag::Crop { pull: (px, py), from: at };
        Outcome::Continue
    }

    fn press_control(&mut self, control: Control, double: bool) -> Outcome {
        match control {
            Control::Tool(tool) => self.set_tool(tool),
            Control::Layer(index) => {
                match self.mods.shift {
                    true => match self.selected.iter().position(|&other| other == index) {
                        Some(at) => drop(self.selected.remove(at)),
                        None => self.selected.push(index),
                    },
                    false => self.selected = vec![index],
                }
                if double {
                    self.cropping = matches!(self.doc.layers[index].kind, Kind::Picture(_)).then_some(index);
                }
            }
            Control::Eye(index) => {
                self.checkpoint();
                self.doc.layers[index].hidden = !self.doc.layers[index].hidden;
                self.changed();
            }
            Control::Lock(index) => {
                self.doc.layers[index].locked = !self.doc.layers[index].locked;
                self.selected.retain(|&other| other != index);
                self.dirty = true;
            }
            Control::Field(field) => match double {
                true => self.typing = Some((field, String::new())),
                false => {
                    self.checkpoint();
                    let start = self.field(field).unwrap_or(0.0);
                    self.drag = Drag::Scrub { field, from: self.pointer.0, start };
                }
            },
            Control::Preset(which) => {
                self.checkpoint();
                (self.doc.width, self.doc.height) = (PRESETS[which].1, PRESETS[which].2);
                self.changed();
                self.fit();
            }
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
                };
                self.changed();
            }
            Control::Backdrop(which) => {
                self.checkpoint();
                self.doc.background = Fill::Backdrop(which);
                self.changed();
            }
            Control::Swatch(paint, color) => self.paint(paint, color),
            Control::Scale(times) => self.export_scale = times,
            Control::Shadow(which) => self.restyle(|layer| {
                if let Kind::Picture(picture) = &mut layer.kind {
                    picture.shadow = which;
                }
            }),
            Control::Width(size) => {
                self.size = size;
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        (shape.stroke, shape.font_px) = (STROKES[size], FONT_SIZES[size]);
                    }
                })
            }
            Control::Head(end, style) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    let on = usize::from(style) < shapes::HEADS.len();
                    shape.heads[end] = on;
                    if on {
                        shape.ends[end] = style;
                    }
                }
            }),
            Control::Line(kind) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    (shape.dashed, shape.dotted) = (kind == 1, kind == 2);
                }
            }),
            Control::Face(face) => {
                self.face = face;
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        shape.face = face;
                    }
                })
            }
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
            Paint::Background => {
                self.checkpoint();
                self.doc.background = Fill::Color(color.unwrap_or(0xFFFFFF));
                self.changed();
            }
            Paint::Line => {
                let color = color.unwrap_or(self.color);
                self.color = color;
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        (shape.color, shape.border) = (color, true);
                    }
                });
            }
            Paint::Fill => {
                self.fill = color;
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        match color {
                            Some(color) => (shape.filled, shape.fill) = (true, color),
                            None => (shape.filled, shape.border) = (false, true),
                        }
                    }
                });
            }
        }
    }

    fn act(&mut self, action: Action) -> Outcome {
        self.dirty = true;
        match action {
            Action::Export => self.export(),
            Action::Copy => self.copy(),
            Action::Save => self.save(),
            Action::Capture => return Outcome::Capture,
            Action::Fit => self.fit(),
            Action::Crop => {
                self.cropping = match self.cropping {
                    Some(_) => None,
                    None => self.picked().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_))),
                }
            }
            Action::FlipAcross => self.restyle(|layer| {
                if let Kind::Picture(picture) = &mut layer.kind {
                    picture.flip.0 = !picture.flip.0;
                }
            }),
            Action::FlipDown => self.restyle(|layer| {
                if let Kind::Picture(picture) = &mut layer.kind {
                    picture.flip.1 = !picture.flip.1;
                }
            }),
            // The size its crop is in its own pixels: one to one.
            Action::Actual => self.restyle(|layer| {
                if let Kind::Picture(picture) = &mut layer.kind {
                    (picture.w, picture.h) = (picture.crop.2, picture.crop.3);
                }
            }),
            Action::Forward => self.restack(true),
            Action::Backward => self.restack(false),
            Action::Duplicate => self.duplicate(),
            Action::Delete => self.delete(),
            Action::Align(way) => self.align(way),
            Action::Border => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.border = !shape.border || !shape.filled;
                }
            }),
            Action::Halo => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.halo = !shape.halo;
                }
            }),
            Action::Elbow => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.elbow = !shape.elbow;
                    shape.points.truncate(2);
                }
            }),
            Action::Sketch => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.sketch = !shape.sketch;
                }
            }),
        }
        Outcome::Continue
    }

    fn set_tool(&mut self, tool: Tool) {
        self.commit_text();
        self.cropping = None;
        self.tool = tool;
        if tool != Tool::Select {
            self.selected.clear();
        }
        self.dirty = true;
    }

    pub fn released(&mut self) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        self.dirty = true;
        match drag {
            Drag::Draw { .. } => {
                let slight = matches!(self.doc.layers.last(), Some(Layer { kind: Kind::Shape(shape), .. }) if shape.is_slight());
                if slight {
                    self.doc.layers.pop();
                    self.undo.pop();
                } else if !matches!(self.tool, Tool::Pen | Tool::Marker | Tool::Counter) {
                    self.selected = vec![self.doc.layers.len() - 1];
                    self.tool = Tool::Select;
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
            }
            Drag::Move { moved: false, .. } => {}
            _ => {}
        }
        self.held.clear();
    }

    /// The one picked out pulled by a handle to a point: the side or
    /// corner held goes there, across the picture unturned, and the one
    /// across from it stays where it is; with Shift, it keeps its shape.
    fn scale_to(&mut self, pull: (i32, i32), from: (f32, f32), at: (f32, f32), mods: Mods) {
        let Some(index) = self.picked() else { return };
        let Some(before) = self.held.first().cloned() else { return };
        let mut layer = before.clone();
        match &mut layer.kind {
            Kind::Picture(picture) => {
                let Kind::Picture(was) = &before.kind else { return };
                let (fx, fy) = was.local(from);
                let (tx, ty) = was.local(at);
                let (mut w, mut h) = (was.w + pull.0 as f32 * (tx - fx), was.h + pull.1 as f32 * (ty - fy));
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
            Kind::Shape(shape) => {
                let (x, y, w, h) = shape.frame();
                let (fx, fy) = (at.0 - from.0, at.1 - from.1);
                let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
                match pull.0 {
                    -1 => left += fx,
                    1 => right += fx,
                    _ => {}
                }
                match pull.1 {
                    -1 => top += fy,
                    1 => bottom += fy,
                    _ => {}
                }
                shape.points[0] = (left, top);
                let far = shape.points.len() - 1;
                shape.points[far.min(1)] = (right, bottom);
                shape.touch();
            }
        }
        self.doc.layers[index] = layer;
        self.changed();
    }

    /// The crop of the picture being cropped pulled by a handle, or moved
    /// whole: in the picture's own pixels, kept within it.
    fn crop_to(&mut self, pull: (i32, i32), from: (f32, f32), at: (f32, f32)) {
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
                let (dx, dy) = (dx.clamp(-left, iw - right), dy.clamp(-top, ih - bottom));
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
        self.changed();
    }

    /// A number of what is picked out, or of the artboard.
    fn field(&mut self, field: Field) -> Option<f32> {
        if let Field::ArtW | Field::ArtH = field {
            return Some(if field == Field::ArtW { self.doc.width } else { self.doc.height } as f32);
        }
        let chosen = self.chosen();
        if chosen.len() > 1 {
            let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
            return match field {
                Field::X => boxes.iter().map(|b| b.0).reduce(f32::min),
                Field::Y => boxes.iter().map(|b| b.1).reduce(f32::min),
                _ => None,
            };
        }
        let index = *chosen.first()?;
        let bounds = self.doc.layers[index].bounds(&mut self.text);
        Some(match &self.doc.layers[index].kind {
            Kind::Picture(picture) => match field {
                Field::X => picture.cx - picture.w / 2.0,
                Field::Y => picture.cy - picture.h / 2.0,
                Field::W => picture.w,
                Field::H => picture.h,
                Field::Turn => picture.angle.to_degrees(),
                Field::Radius => picture.radius,
                Field::Opacity => picture.opacity as f32 * 100.0 / 256.0,
                _ => 0.0,
            },
            Kind::Shape(shape) => match field {
                Field::X => bounds.0,
                Field::Y => bounds.1,
                Field::W => shape.frame().2,
                Field::H => shape.frame().3,
                Field::Turn => shape.angle.to_degrees(),
                Field::Radius => shape.radius,
                Field::Opacity => shape.opacity as f32 * 100.0 / 256.0,
                _ => 0.0,
            },
        })
    }

    fn set_field(&mut self, field: Field, value: f32) {
        match field {
            Field::ArtW => self.doc.width = (value.round() as i32).clamp(16, 8000),
            Field::ArtH => self.doc.height = (value.round() as i32).clamp(16, 8000),
            _ => {
                let Some(now) = self.field(field) else { return };
                let chosen = self.chosen();
                for index in chosen {
                    let layer = &mut self.doc.layers[index];
                    match (&mut layer.kind, field) {
                        (_, Field::X) => layer.translate((value - now).round(), 0.0),
                        (_, Field::Y) => layer.translate(0.0, (value - now).round()),
                        (Kind::Picture(picture), Field::W) => picture.w = value.max(4.0),
                        (Kind::Picture(picture), Field::H) => picture.h = value.max(4.0),
                        (Kind::Picture(picture), Field::Turn) => picture.angle = value.to_radians(),
                        (Kind::Picture(picture), Field::Radius) => picture.radius = value.max(0.0),
                        (Kind::Picture(picture), Field::Opacity) => picture.opacity = ((value.clamp(0.0, 100.0) * 2.56).round() as u32).min(256),
                        (Kind::Shape(shape), Field::W) => {
                            let x = shape.frame().0;
                            shape.points[1].0 = x + value.max(1.0);
                            shape.points[0].0 = x;
                        }
                        (Kind::Shape(shape), Field::H) => {
                            let y = shape.frame().1;
                            shape.points[1].1 = y + value.max(1.0);
                            shape.points[0].1 = y;
                        }
                        (Kind::Shape(shape), Field::Turn) => shape.angle = value.to_radians(),
                        (Kind::Shape(shape), Field::Radius) => shape.radius = value.max(0.0),
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

    fn commit_text(&mut self) {
        if let Some(index) = self.editing.take() {
            if matches!(&self.doc.layers.get(index), Some(Layer { kind: Kind::Shape(shape), .. }) if shape.is_slight()) {
                self.doc.layers.remove(index);
                self.selected.clear();
            }
            self.changed();
        }
    }

    // ---- What is done to what is picked out ----

    fn restack(&mut self, forward: bool) {
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

    fn duplicate(&mut self) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        self.checkpoint();
        self.selected.clear();
        for index in chosen {
            let mut copy = self.doc.layers[index].clone();
            copy.translate(16.0, 16.0);
            self.doc.layers.push(copy);
            self.selected.push(self.doc.layers.len() - 1);
        }
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
        if chosen.is_empty() {
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
        self.checkpoint();
        let view = self.layout().view;
        let middle = self.to_doc(view.center());
        let image = Arc::new(image);
        let mut picture = Picture::new(image, name.to_owned(), middle);
        let most = (self.doc.width as f32 * 0.9 / picture.w).min(self.doc.height as f32 * 0.9 / picture.h).min(1.0);
        (picture.w, picture.h) = (picture.w * most, picture.h * most);
        self.doc.layers.push(Layer::picture(picture));
        self.selected = vec![self.doc.layers.len() - 1];
        self.tool = Tool::Select;
        self.changed();
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
                let view = self.layout().view;
                let middle = self.to_doc(view.center());
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

    /// The artboard as a PNG, at the size export is set to.
    fn png(&mut self) -> Option<Vec<u8>> {
        let (wide, high, pixels) = self.doc.render(self.export_scale as f32, &mut self.text);
        let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
        crate::output::encode(wide as u32, high as u32, &pixels, clear).ok()
    }

    fn export(&mut self) {
        let Some(png) = self.png() else { return };
        match crate::output::save(&self.dir, &self.filename, "png", &png) {
            Ok(path) => self.say(format!("Exported {}", shortened(&path))),
            Err(error) => self.say(error),
        }
    }

    /// The artboard on the clipboard, kept there by a hotshot of its own,
    /// as the pin does.
    fn copy(&mut self) {
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

    /// The canvas kept in its file, or a new one among the pictures.
    fn save(&mut self) {
        let path = match &self.path {
            Some(path) => path.clone(),
            None => {
                let name = format!("{}.hotshot", crate::output::dated(&self.filename).trim_end_matches(".png"));
                self.dir.join(name)
            }
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&path, self.doc.write()) {
            Ok(()) => {
                self.say(format!("Saved {}", shortened(&path)));
                (self.path, self.edited) = (Some(path), false);
            }
            Err(error) => self.say(format!("Could not save: {error}")),
        }
    }

    // ---- The keys ----

    pub fn key(&mut self, key: Key, mods: Mods, down: bool) -> Outcome {
        self.mods = mods;
        self.dirty = true;
        if key == Key::Char(' ') && self.editing.is_none() && self.typing.is_none() {
            self.space = down;
            return Outcome::Continue;
        }
        if !down {
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
        // Text being typed.
        if let (Some(index), false) = (self.editing, mods.ctrl) {
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                match key {
                    Key::Char(letter) => shape.text.push(letter),
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
                self.changed();
            }
            return Outcome::Continue;
        }
        let lower = match key {
            Key::Char(letter) => Key::Char(letter.to_ascii_lowercase()),
            other => other,
        };
        match (lower, mods.ctrl) {
            (Key::Char('q' | 'w'), true) => return Outcome::Quit,
            (Key::Char('z'), true) if mods.shift => self.redo(),
            (Key::Char('z'), true) => self.undo(),
            (Key::Char('y'), true) => self.redo(),
            (Key::Char('a'), true) => self.selected = (0..self.doc.layers.len()).filter(|&index| !self.doc.layers[index].locked).collect(),
            (Key::Char('d'), true) => self.duplicate(),
            (Key::Char('c'), true) => self.copy(),
            (Key::Char('v'), true) => return Outcome::Paste,
            (Key::Char('s'), true) => self.save(),
            (Key::Char('e'), true) => self.export(),
            (Key::Char('g'), true) => return Outcome::Capture,
            (Key::Char(']' | '}'), true) => self.restack(true),
            (Key::Char('[' | '{'), true) => self.restack(false),
            (Key::Char('0'), true) => self.fit(),
            (Key::Char('1'), true) => {
                let view = self.layout().view;
                let middle = view.center();
                self.zoom_about(1.0 / self.zoom, middle);
            }
            (Key::Char('=' | '+'), true) => {
                let middle = self.layout().view.center();
                self.zoom_about(1.25, middle);
            }
            (Key::Char('-'), true) => {
                let middle = self.layout().view.center();
                self.zoom_about(0.8, middle);
            }
            (Key::Delete | Key::Backspace, _) => self.delete(),
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
                                self.editing = Some(index);
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
                    self.checkpoint();
                    chosen.iter().for_each(|&index| self.doc.layers[index].translate(dx, dy));
                    self.changed();
                }
            }
            (Key::Char(letter), false) => {
                if let Some(&(tool, ..)) = TOOLS.iter().find(|(_, _, key)| key.to_ascii_lowercase().starts_with(letter)) {
                    self.set_tool(tool);
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
            self.redo.push(std::mem::replace(&mut self.doc, before));
            self.selected.retain(|&index| index < self.doc.layers.len());
            self.cropping = None;
            self.changed();
        }
    }

    fn redo(&mut self) {
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, after));
            self.selected.retain(|&index| index < self.doc.layers.len());
            self.changed();
        }
    }

    // ---- The picture of the window ----

    /// The artboard as it is shown: drawn again only when it has changed,
    /// and made smaller only when it or the zoom has.
    fn shown_artboard(&mut self) -> (i32, i32) {
        if self.rendered.as_ref().is_none_or(|kept| kept.0 != self.version) {
            let (wide, high, pixels) = self.doc.render(1.0, &mut self.text);
            self.rendered = Some((self.version, wide, high, pixels));
        }
        let (wide, high) = (((self.doc.width as f32 * self.zoom).round() as i32).max(1), ((self.doc.height as f32 * self.zoom).round() as i32).max(1));
        if self.zoom < 1.0 && self.shown.as_ref().is_none_or(|kept| (kept.0, kept.1, kept.2) != (self.version, wide, high)) {
            let Some((_, w, h, pixels)) = &self.rendered else { return (wide, high) };
            self.shown = Some((self.version, wide, high, crate::pin::sized(pixels, *w, *h, wide, high)));
        }
        (wide, high)
    }

    pub fn draw(&mut self, out: &mut [u32]) {
        let (width, height) = (self.width, self.height);
        let layout = self.layout();
        let view = layout.view;
        let (wide, high) = self.shown_artboard();
        out.iter_mut().for_each(|pixel| *pixel = VIEW_COLOR);
        // The view: dots on its ground, the artboard on that, over light
        // squares where it is see-through.
        let dots = 24;
        let (ox, oy) = (self.pan.0.rem_euclid(dots as f32) as i32, self.pan.1.rem_euclid(dots as f32) as i32);
        for y in (view.y + oy..view.bottom()).step_by(dots as usize) {
            for x in (view.x + ox..view.right()).step_by(dots as usize) {
                out[(y * width + x) as usize] = 0x3A3A3A;
            }
        }
        let origin = (view.x + self.pan.0.round() as i32, view.y + self.pan.1.round() as i32);
        let shown = Rect::new(origin.0, origin.1, wide, high).intersect(view);
        let source = if self.zoom < 1.0 { self.shown.as_ref().map(|kept| (kept.1, kept.3.as_slice())) } else { self.rendered.as_ref().map(|kept| (kept.1, kept.3.as_slice())) };
        if let Some((source_w, pixels)) = source {
            let full = self.zoom >= 1.0;
            for y in shown.y..shown.bottom() {
                let row = &mut out[(y * width + shown.x) as usize..(y * width + shown.right()) as usize];
                let sy = if full { (((y - origin.1) as f32 + 0.5) / self.zoom) as i32 } else { y - origin.1 };
                for (at, pixel) in row.iter_mut().enumerate() {
                    let x = shown.x + at as i32;
                    let sx = if full { (((x - origin.0) as f32 + 0.5) / self.zoom) as i32 } else { x - origin.0 };
                    let color = pixels.get((sy * source_w + sx) as usize).copied().unwrap_or(0);
                    let alpha = color >> 24;
                    let light = ((x - origin.0) / 10 + (y - origin.1) / 10) % 2 == 0;
                    let ground = if light { 0xFFFFFF } else { 0xE6E6E6 };
                    *pixel = if alpha == 255 { color & 0xFF_FFFF } else { canvas::mix(ground, color & 0xFF_FFFF, alpha + (alpha >> 7)) };
                }
            }
        }
        // What the pointer is over, and what is picked out, with handles.
        let mut canvas = Canvas::new(out, width, height);
        canvas.set_clip(view);
        if let Some(index) = self.hovered.filter(|index| !self.selected.contains(index) && *index < self.doc.layers.len()) {
            self.outline(&mut canvas, index, 0x7A93EA);
        }
        for index in self.chosen() {
            self.outline(&mut canvas, index, ACCENT);
        }
        if self.cropping.is_none() {
            for (spot, _) in self.handles() {
                canvas.fill(spot.0 - 5, spot.1 - 5, 10, 10, ACCENT);
                canvas.fill(spot.0 - 4, spot.1 - 4, 8, 8, 0xFFFFFF);
            }
        }
        if let Some(index) = self.cropping {
            self.draw_crop(&mut canvas, index);
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
                let line = Text::line_height(shape.font_px);
                let last = shape.text.rsplit('\n').next().unwrap_or("");
                let rows = shape.text.split('\n').count() as i32;
                let along = self.text.width_in(shape.words_face(), last, shape.font_px);
                let (x, y) = self.to_window(((place.x + along) as f32, (place.y + (rows - 1) * line) as f32));
                canvas.fill(x + 1, y, 2, (line as f32 * self.zoom) as i32, ACCENT);
            }
        }
        canvas.reset_clip();
        self.draw_panels(&mut canvas, &layout);
    }

    /// The box round a layer, in the window: turned with it, if it is a
    /// picture that is.
    fn outline(&mut self, canvas: &mut Canvas, index: usize, color: u32) {
        match self.doc.layers[index].kind.clone() {
            Kind::Picture(picture) => {
                let corners = picture.corners().map(|corner| self.to_window(corner));
                for side in 0..4 {
                    line(canvas, corners[side], corners[(side + 1) % 4], color);
                }
            }
            Kind::Shape(_) => {
                let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
                let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
                canvas.round_outline(a.0 - 2, a.1 - 2, b.0 - a.0 + 4, b.1 - a.1 + 4, 0, 1, color);
            }
        }
    }

    /// The picture being cropped: all of it faint, the crop bright, and
    /// its corners to pull.
    fn draw_crop(&mut self, canvas: &mut Canvas, index: usize) {
        let Some(Layer { kind: Kind::Picture(picture), .. }) = self.doc.layers.get(index).cloned() else { return };
        // The whole of it, where it would be, in outline.
        let (per_x, per_y) = (picture.w / picture.crop.2.max(1e-3), picture.h / picture.crop.3.max(1e-3));
        let (iw, ih) = (picture.image.width() as f32 * per_x, picture.image.height() as f32 * per_y);
        let (cx, cy) = (picture.crop.0 * per_x, picture.crop.1 * per_y);
        let whole = [(-picture.w / 2.0 - cx, -picture.h / 2.0 - cy), (-picture.w / 2.0 - cx + iw, -picture.h / 2.0 - cy), (-picture.w / 2.0 - cx + iw, -picture.h / 2.0 - cy + ih), (-picture.w / 2.0 - cx, -picture.h / 2.0 - cy + ih)].map(|corner| self.to_window(picture.placed(corner)));
        for side in 0..4 {
            line(canvas, whole[side], whole[(side + 1) % 4], 0x8B8D98);
        }
        let corners = picture.corners().map(|corner| self.to_window(corner));
        for side in 0..4 {
            line(canvas, corners[side], corners[(side + 1) % 4], 0xFFFFFF);
        }
        for corner in corners {
            canvas.fill(corner.0 - 6, corner.1 - 6, 12, 12, 0xFFFFFF);
            canvas.fill(corner.0 - 4, corner.1 - 4, 8, 8, ACCENT);
        }
    }

    fn draw_panels(&mut self, canvas: &mut Canvas, layout: &Layout) {
        let (width, height) = (self.width, self.height);
        let px = 13;
        let line_high = Text::line_height(px);
        canvas.fill(0, 0, width, TOP, PANEL_COLOR);
        canvas.fill(0, TOP - 1, width, 1, EDGE_COLOR);
        canvas.fill(0, TOP, LEFT, height - TOP, PANEL_COLOR);
        canvas.fill(LEFT - 1, TOP, 1, height - TOP, EDGE_COLOR);
        canvas.fill(width - RIGHT, TOP, RIGHT, height - TOP, PANEL_COLOR);
        canvas.fill(width - RIGHT, TOP, 1, height - TOP, EDGE_COLOR);
        let hover = layout.controls.iter().find(|(rect, control)| rect.contains(self.pointer.0, self.pointer.1) && *control != Control::Panel).map(|(_, control)| *control);
        let chosen = self.chosen();
        let shown = self.picked().map(|index| self.doc.layers[index].clone());
        // The rows of layers first, for their eyes and locks to go over.
        let rows = layout.controls.iter().filter(|(_, control)| matches!(control, Control::Layer(_)));
        for &(rect, control) in rows.chain(layout.controls.iter().filter(|(_, control)| !matches!(control, Control::Layer(_)))) {
            let hovered = hover == Some(control);
            match control {
                Control::Tool(tool) => {
                    let on = self.tool == tool;
                    if on {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, ACCENT, 256);
                    } else if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, 0xFFFFFF, 30);
                    }
                    let icon = match tool {
                        Tool::Select => icons::SELECT,
                        Tool::Rect => icons::RECT,
                        Tool::Ellipse => icons::ELLIPSE,
                        Tool::Line => icons::LINE,
                        Tool::Arrow => icons::ARROW,
                        Tool::Pen => icons::PEN,
                        Tool::Marker => icons::MARKER,
                        Tool::Text => icons::TEXT,
                        _ => icons::COUNTER,
                    };
                    let (cx, cy) = rect.center();
                    self.text.draw_centered(canvas, cx, cy, 16, if on { 0xFFFFFF } else { TEXT_COLOR }, icon);
                }
                Control::Action(action) if rect.y < TOP => {
                    let primary = action == Action::Export;
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if primary { ACCENT } else if hovered { 0x3A3A3A } else { 0x2A2A2A }, 256);
                    self.text.draw(canvas, rect.x + 10, rect.y + (rect.h - line_high) / 2, px, TEXT_COLOR, action.says());
                }
                Control::Layer(index) => {
                    let on = chosen.contains(&index);
                    if on {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, ACCENT, 256);
                    } else if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, 0xFFFFFF, 20);
                    }
                    let layer = &self.doc.layers[index];
                    let (name, faint) = (layer.name(), layer.hidden);
                    // A small picture of what it is: the picture itself
                    // in a square, or a mark for a shape.
                    let icon = Rect::new(rect.x + 6, rect.y + 6, 18, rect.h - 12);
                    match &layer.kind {
                        Kind::Picture(_) => {
                            canvas.round_fill(icon.x, icon.y, icon.w, icon.h, 3, 0xDDDDDD, 256);
                            canvas.fill(icon.x + 3, icon.y + 5, icon.w - 6, 2, 0x8B8D98);
                            canvas.fill(icon.x + 3, icon.y + 9, icon.w - 9, 2, 0x8B8D98);
                        }
                        Kind::Shape(shape) => canvas.round_fill(icon.x + 3, icon.y + 3, icon.w - 6, icon.h - 6, 3, shape.color, 256),
                    }
                    let shortened: String = name.chars().take(18).collect();
                    self.text.draw(canvas, rect.x + 32, rect.y + (rect.h - line_high) / 2, px, if faint { DIM_TEXT_COLOR } else { TEXT_COLOR }, &shortened);
                }
                Control::Eye(index) => {
                    let (cx, cy) = rect.center();
                    let on = !self.doc.layers[index].hidden;
                    canvas.disc(cx, cy, 4, if on { 0xC9CCD3 } else { 0x555555 }, 256);
                    if !on {
                        canvas.fill(cx - 6, cy, 12, 1, 0x888888);
                    }
                }
                Control::Lock(index) => {
                    if self.doc.layers[index].locked {
                        let (cx, cy) = rect.center();
                        canvas.round_fill(cx - 4, cy - 1, 9, 7, 2, 0xC9CCD3, 256);
                        canvas.round_outline(cx - 3, cy - 6, 7, 8, 3, 1, 0xC9CCD3);
                    }
                }
                Control::Field(field) => {
                    let typing = self.typing.as_ref().filter(|(typed, _)| *typed == field).map(|(_, typed)| typed.clone());
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                    if typing.is_some() || hovered {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if typing.is_some() { ACCENT } else { 0x444444 });
                    }
                    let label = field.label();
                    self.text.draw(canvas, rect.x + 8, rect.y + (rect.h - line_high) / 2, px, DIM_TEXT_COLOR, label);
                    let value = match typing {
                        Some(typed) => format!("{typed}|"),
                        None => self.field(field).map_or("\u{2014}".to_owned(), |value| format!("{}", value.round() as i32)),
                    };
                    let label_w = self.text.width(label, px);
                    self.text.draw(canvas, rect.x + 16 + label_w, rect.y + (rect.h - line_high) / 2, px, TEXT_COLOR, &value);
                }
                Control::Swatch(paint, color) => {
                    let now = match (paint, &shown) {
                        (Paint::Background, _) => matches!(self.doc.background, Fill::Color(c) if Some(c) == color),
                        (Paint::Line, Some(Layer { kind: Kind::Shape(shape), .. })) => Some(shape.color) == color,
                        (Paint::Fill, Some(Layer { kind: Kind::Shape(shape), .. })) => if shape.filled { Some(shape.fill) == color } else { color.is_none() },
                        _ => false,
                    };
                    let (cx, cy) = rect.center();
                    let radius = rect.w / 2 - 3;
                    canvas.disc(cx, cy, radius + 2, if now { 0xFFFFFF } else { 0x444444 }, 256);
                    match color {
                        Some(color) => canvas.disc(cx, cy, radius, color, 256),
                        None => {
                            canvas.disc(cx, cy, radius, 0xF2F2F2, 256);
                            for step in -radius * 7 / 10..radius * 7 / 10 {
                                canvas.fill(cx + step - 1, cy - step - 1, 2, 2, 0xE5484D);
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
                Control::Panel => {}
                _ => {
                    // A chip: lit when it is what there is now.
                    let on = match control {
                        Control::Preset(which) => (self.doc.width, self.doc.height) == (PRESETS[which].1, PRESETS[which].2),
                        Control::Ground(ground) => match self.doc.background {
                            Fill::None => ground == Ground::None,
                            Fill::Color(_) => ground == Ground::Color,
                            Fill::Backdrop(_) => ground == Ground::Backdrop,
                        },
                        Control::Scale(times) => self.export_scale == times,
                        Control::Shadow(which) => matches!(&shown, Some(Layer { kind: Kind::Picture(picture), .. }) if picture.shadow == which),
                        Control::Width(size) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if if matches!(shape.tool, Tool::Text | Tool::Badge) { shape.font_px == FONT_SIZES[size] } else { shape.stroke == STROKES[size] }),
                        Control::Head(end, style) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if if shape.heads[end] { shape.ends[end] == style } else { usize::from(style) == shapes::HEADS.len() }),
                        Control::Line(kind) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if match kind { 2 => shape.dotted, 1 => shape.dashed && !shape.dotted, _ => !shape.dashed && !shape.dotted }),
                        Control::Face(face) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.face == face),
                        Control::Action(Action::Halo) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.halo),
                        Control::Action(Action::Elbow) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.elbow),
                        Control::Action(Action::Sketch) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.sketch),
                        Control::Action(Action::Border) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.border),
                        Control::Action(Action::Crop) => self.cropping.is_some(),
                        _ => false,
                    };
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ACCENT } else if hovered { 0x3A3A3A } else { 0x2A2A2A }, 256);
                }
            }
        }
        for ((x, y), words, color) in &layout.words {
            self.text.draw(canvas, *x, *y, px, *color, words);
        }
        // Along the bottom of the view: how large it is shown, and what
        // was last done.
        let zoom = format!("{}%", (self.zoom * 100.0).round() as i32);
        let wide = self.text.width(&zoom, px) + 16;
        let view = layout.view;
        canvas.round_fill(view.right() - wide - 10, view.bottom() - 34, wide, 24, 6, PANEL_COLOR, 230);
        self.text.draw(canvas, view.right() - wide - 2, view.bottom() - 34 + (24 - line_high) / 2, px, TEXT_COLOR, &zoom);
        if let Some((note, when)) = &self.note {
            if when.elapsed().as_secs_f32() < 3.0 {
                let note = note.clone();
                let wide = self.text.width(&note, px) + 24;
                let (x, y) = (view.x + (view.w - wide) / 2, view.bottom() - 48);
                canvas.round_fill(x, y, wide, 32, 8, 0x000000, 210);
                self.text.draw(canvas, x + 12, y + (32 - line_high) / 2, px, TEXT_COLOR, &note);
            }
        }
        // The name of the button under the pointer, and its key.
        if let Some(Control::Tool(tool)) = hover {
            if let Some((_, name, key)) = TOOLS.iter().find(|(known, ..)| *known == tool) {
                let says = format!("{name}  {key}");
                let wide = self.text.width(&says, px) + 16;
                let (x, y) = (self.pointer.0 - wide / 2, TOP + 6);
                canvas.round_fill(x.max(4), y, wide, line_high + 8, 6, 0x000000, 220);
                self.text.draw(canvas, x.max(4) + 8, y + 4, px, TEXT_COLOR, &says);
            }
        }
        let _ = DIM_TEXT_COLOR;
    }
}

/// A line a pixel wide, from one point to another, in the window.
fn line(canvas: &mut Canvas, from: (i32, i32), to: (i32, i32), color: u32) {
    let steps = (to.0 - from.0).abs().max((to.1 - from.1).abs()).max(1);
    for step in 0..=steps {
        let x = from.0 + (to.0 - from.0) * step / steps;
        let y = from.1 + (to.1 - from.1) * step / steps;
        canvas.fill(x, y, 1, 1, color);
    }
}

// ---- The window ----

/// The canvas in a window of its own, until it is closed: with the files
/// given, a canvas saved before or pictures to put on a new one.
pub fn run(files: &[PathBuf], options: &crate::options::Options) -> crate::x11::Result<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::protocol::Event;
    use x11rb::wrapper::ConnectionExt as _;

    let mut display = crate::x11::Display::open()?;
    display.prepare()?;
    let text = Text::load();

    // A canvas from its file; or a new one, the size of the first picture
    // given, or of the screen's half, with the pictures on it.
    let mut doc = None;
    let mut pictures = Vec::new();
    let mut path = None;
    for file in files {
        let bytes = std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        if let Some(read) = Doc::read(&bytes) {
            // One the editor handed over is for this once, and is kept
            // anew where it is saved.
            if options.forget {
                let _ = std::fs::remove_file(file);
            }
            (doc, path) = (Some(read), (!options.forget).then(|| file.clone()));
            continue;
        }
        let image = Image::open(&bytes).ok_or_else(|| format!("{} is neither a picture nor a canvas", file.display()))?;
        let name = file.file_stem().map_or("Picture".to_owned(), |stem| stem.to_string_lossy().into_owned());
        pictures.push((image, name));
    }
    let doc = doc.unwrap_or_else(|| match pictures.first() {
        Some((image, _)) => {
            let mut doc = Doc::new(image.width() + 160, image.height() + 160);
            doc.background = Fill::Backdrop(frame::named("aurora").unwrap_or(0));
            doc
        }
        None => Doc::new(1200, 675),
    });
    let (width, height) = ((display.width * 4 / 5).clamp(900, 2400), (display.height * 4 / 5).clamp(600, 1600));
    let dir = options.places.get(0).cloned().unwrap_or_else(|| options.dir.clone());
    let mut studio = Studio::new(doc, text, width, height, dir, options.filename.clone());
    studio.path = path;
    for (index, (image, name)) in pictures.into_iter().enumerate() {
        studio.add_picture(image, &name);
        // Pictures after the first a little down and to the right.
        if index > 0 {
            let at = studio.doc.layers.len() - 1;
            studio.doc.layers[at].translate(40.0 * index as f32, 40.0 * index as f32);
        }
    }
    (studio.undo, studio.edited) = (Vec::new(), false);

    let conn = &display.conn;
    let atom = |name: &str| -> crate::x11::Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let window = conn.generate_id()?;
    let events = EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION | EventMask::KEY_PRESS | EventMask::KEY_RELEASE | EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE;
    let look = CreateWindowAux::new().background_pixel(VIEW_COLOR).event_mask(events);
    conn.create_window(x11rb::COPY_FROM_PARENT as u8, window, display.root, ((display.width - width) / 2) as i16, ((display.height - height) / 2) as i16, width as u16, height as u16, 0, WindowClass::INPUT_OUTPUT, x11rb::COPY_FROM_PARENT, &look)?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"hotshot\0Hotshot\0")?;
    let (protocols, delete) = (atom("WM_PROTOCOLS")?, atom("WM_DELETE_WINDOW")?);
    conn.change_property32(PropMode::REPLACE, window, protocols, AtomEnum::ATOM, &[delete])?;
    let (utf8, net_name) = (atom("UTF8_STRING")?, atom("_NET_WM_NAME")?);
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
    conn.map_window(window)?;
    conn.flush()?;

    let mut frame = vec![0u32; (width * height) as usize];
    let (mut size, mut title, mut cursor) = ((width, height), String::new(), None::<crate::editor::Cursor>);
    let mut time = 0u32;
    loop {
        // What has happened: a run of the pointer's movements told as
        // where it last was.
        let mut events = vec![conn.wait_for_event()?];
        while let Some(event) = conn.poll_for_event()? {
            events.push(event);
        }
        let mut outcome = Outcome::Continue;
        let mut motion = None;
        for event in events {
            match event {
                Event::MotionNotify(moved) => motion = Some((i32::from(moved.event_x), i32::from(moved.event_y), crate::x11::mods(moved.state.into()))),
                Event::ButtonPress(press) => {
                    if let Some((x, y, mods)) = motion.take() {
                        studio.pointer_moved(x, y, mods);
                    }
                    time = press.time;
                    studio.pointer_moved(i32::from(press.event_x), i32::from(press.event_y), crate::x11::mods(press.state.into()));
                    outcome = studio.pressed(press.detail, press.time);
                }
                Event::ButtonRelease(release) if release.detail <= 3 => {
                    if let Some((x, y, mods)) = motion.take() {
                        studio.pointer_moved(x, y, mods);
                    }
                    studio.released();
                }
                Event::KeyPress(press) | Event::KeyRelease(press) => {
                    let down = event_is_press(&event);
                    let state: u16 = press.state.into();
                    time = press.time;
                    outcome = studio.key(display.key(press.detail, state), crate::x11::mods(state), down);
                }
                Event::Expose(_) => studio.dirty = true,
                Event::ConfigureNotify(configured) if configured.window == window => {
                    let now = (i32::from(configured.width), i32::from(configured.height));
                    if now != size {
                        size = now;
                        frame = vec![0u32; (now.0 * now.1) as usize];
                        studio.resize(now.0, now.1);
                    }
                }
                Event::ClientMessage(message) if message.type_ == protocols && message.data.as_data32()[0] == delete => outcome = Outcome::Quit,
                _ => {}
            }
            if outcome != Outcome::Continue {
                break;
            }
        }
        if let Some((x, y, mods)) = motion {
            studio.pointer_moved(x, y, mods);
        }
        match outcome {
            Outcome::Quit => break,
            Outcome::Paste => {
                let picture = selection(conn, window, atom("CLIPBOARD")?, atom("image/png")?, atom("HOTSHOT_CANVAS")?, atom("INCR")?, time);
                let words = match picture {
                    Some(_) => None,
                    None => selection(conn, window, atom("CLIPBOARD")?, utf8, atom("HOTSHOT_CANVAS")?, atom("INCR")?, time).and_then(|bytes| String::from_utf8(bytes).ok()),
                };
                studio.pasted(picture, words);
            }
            Outcome::Capture => {
                // Out of the way, the screen captured by a hotshot of its
                // own into a file, and the window back.
                conn.unmap_window(window)?;
                conn.flush()?;
                std::thread::sleep(std::time::Duration::from_millis(250));
                let file = std::env::temp_dir().join(format!("hotshot-canvas-{}.png", std::process::id()));
                let _ = std::process::Command::new(std::env::current_exe()?).arg("-o").arg(&file).status();
                conn.map_window(window)?;
                conn.flush()?;
                studio.captured(&file);
            }
            Outcome::Continue => {}
        }
        let now = studio.title();
        if now != title {
            conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, now.as_bytes())?;
            conn.change_property8(PropMode::REPLACE, window, net_name, utf8, now.as_bytes())?;
            title = now;
        }
        let wanted = studio.cursor();
        if cursor != Some(wanted) {
            if let Some(id) = display.cursor_of(wanted) {
                conn.change_window_attributes(window, &ChangeWindowAttributesAux::new().cursor(id))?;
            }
            cursor = Some(wanted);
        }
        if studio.take_dirty() {
            studio.draw(&mut frame);
            crate::x11::put(conn, window, gc, false, size.0, size.1, &frame)?;
        }
        conn.flush()?;
    }
    Ok(())
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
        doc.layers.push(Layer { kind: Kind::Shape(arrow), hidden: true, locked: true });
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
        // Its fill, from the panel.
        let swatch = studio.layout().controls.iter().find(|(_, control)| *control == Control::Swatch(Paint::Fill, Some(0xFFE629))).map(|(rect, _)| rect.center()).expect("the fill swatches are up");
        studio.pointer_moved(swatch.0, swatch.1, Mods::default());
        studio.pressed(1, 3000);
        studio.released();
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((shape.filled, shape.fill), (true, 0xFFE629));
        // With nothing picked out, the panel is the artboard's.
        studio.key(Key::Escape, Mods::default(), true);
        assert!(studio.layout().controls.iter().any(|(_, control)| *control == Control::Preset(0)));
        studio.press_control(Control::Preset(2), false);
        assert_eq!((studio.doc.width, studio.doc.height), (1920, 1080));
        studio.press_control(Control::Ground(Ground::Backdrop), false);
        assert!(matches!(studio.doc.background, Fill::Backdrop(_)));
    }

    #[test]
    fn a_picture_from_the_editor_keeps_its_shapes_where_they_were_on_it() {
        let area = Rect::new(300, 200, 400, 250);
        let mut arrow = Shape::new(Tool::Arrow, 0xE5484D, 3.5, (310.0, 220.0));
        arrow.points[1] = (500.0, 300.0);
        let doc = from_markup(area, &vec![0x123456; 400 * 250], &arrow.write());
        assert_eq!((doc.width, doc.height, doc.layers.len()), (560, 410, 2));
        let Kind::Shape(moved) = &doc.layers[1].kind else { panic!("the arrow over the picture") };
        assert_eq!(moved.points[..2], [(90.0, 100.0), (280.0, 180.0)]);
        let (_, _, out) = doc.render(1.0, &mut Text::load());
        assert_eq!(out[300 * 560 + 400], 0xFF123456);
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
        assert!(out.iter().any(|&pixel| pixel == 0xFF0000) && out[5] == PANEL_COLOR);
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
        studio.selected = vec![0];
        studio.cropping = Some(0);
        write(&mut studio, "c4-crop");
        studio.cropping = None;
        studio.selected.clear();
        let (_, _, pixels) = studio.doc.render(1.0, &mut studio.text);
        std::fs::write(dir.join("c5-export.png"), crate::output::encode(1200, 675, &pixels, false).unwrap()).unwrap();
    }
}
