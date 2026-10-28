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

mod actions;
mod arrange;
mod menu;
mod panel;
mod picker;
mod preview;
mod snap;
mod search;
mod style;


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

/// How much larger a picture's shadow is than Tailwind's of its name.
const SHADOW_SIZE: f32 = 2.0;

/// How a handle at an end of a line pulls: that end, and nothing else.
const END: i32 = 9;

/// The widths of line there are, in pixels of the picture; and the sizes
/// of text that go with them.
const STROKES: [f32; 4] = [2.0, 3.5, 6.0, 10.0];
const FONT_SIZES: [u32; 4] = [18, 26, 40, 64];

/// The sizes an artboard can be set to at once, by what each is for.

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
    /// What it is called, for the ends of arrows to be stuck to it by.
    pub id: u32,
    /// The shadow it casts when `shadow` is `CUSTOM_SHADOW`: set number by
    /// number, in pixels of the artboard.
    pub drop: frame::Layer,
    /// How much lighter, more contrasted and more colorful it is shown,
    /// each from -1 to 1, nothing at 0.
    pub adjust: [f32; 3],
    /// A window drawn round it, its bar over the top of it; and what the
    /// window is called, in its bar.
    pub window: frame::Window,
    pub title: String,
}

/// A picture's `shadow` when its shadow is set number by number.
const CUSTOM_SHADOW: usize = frame::SHADOWS.len() + 1;

/// The shadow a picture is given to begin with, when its numbers are first
/// set: Tailwind's 2xl, as an artboard is mostly seen at half its size.
const DROP: frame::Layer = frame::Layer { x: 0, y: 50, blur: 100, spread: -24, color: 0x4000_0000 };

impl Picture {
    pub fn new(image: Arc<Image>, name: String, at: (f32, f32)) -> Picture {
        let (w, h) = (image.width() as f32, image.height() as f32);
        Picture { image, name, cx: at.0, cy: at.1, w, h, angle: 0.0, crop: (0.0, 0.0, w, h), radius: 0.0, shadow: 0, opacity: 256, flip: (false, false), id: shapes::fresh_id(), drop: DROP, adjust: [0.0; 3], window: frame::Window::None, title: String::new() }
    }

    /// How tall the bar of its window is, in pixels of the artboard: none
    /// without a window.
    fn bar(&self) -> f32 {
        match self.window {
            frame::Window::None => 0.0,
            _ => (self.w * 0.04).clamp(18.0, 44.0).min(self.h / 3.0),
        }
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
}

/// A size a frame is exported at.
#[derive(Clone, PartialEq, Debug)]
pub struct ExportSize {
    pub times: f32,
    pub suffix: String,
    pub format: crate::output::Format,
}

impl ExportSize {
    fn new(times: f32) -> ExportSize {
        ExportSize { times, suffix: String::new(), format: crate::output::Format::Png }
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
        Doc { name: "Frame 1".into(), x: 0.0, y: 0.0, stamp: 0, width, height, background: Fill::Color(0xFFFFFF), layers: Vec::new(), exports: vec![ExportSize::new(2.0)] }
    }

    /// The artboard as a picture, `times` its size: each pixel with how
    /// opaque it is in its top byte, which with nothing behind is not at
    /// all where nothing is on it.
    pub fn render(&self, times: f32, text: &mut Text) -> (i32, i32, Vec<u32>) {
        let (wide, high) = (((self.width as f32 * times).round() as i32).max(1), ((self.height as f32 * times).round() as i32).max(1));
        (wide, high, self.render_part(times, Rect::new(0, 0, wide, high), text))
    }

    /// Part of the artboard as a picture, `times` its size: the pixels of
    /// `part` of that, everything drawn afresh at that size, so that the
    /// shapes and the text on it are as sharp however large it is shown.
    pub fn render_part(&self, times: f32, part: Rect, text: &mut Text) -> Vec<u32> {
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
        }
        for layer in self.layers.iter().filter(|layer| !layer.hidden) {
            match &layer.kind {
                Kind::Picture(picture) => draw_picture(&mut out, part, picture, times, text),
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
        write_pages(&[Page { name: "Page 1".into(), frames: vec![self.clone()] }])
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
        let sizes: Vec<String> = self.exports.iter().map(|size| format!("{}:{}:{}", size.times, size.format.ending(), size.suffix.replace([' ', ':'], "_"))).collect();
        *out += &format!("exports {}\n", sizes.join(" "));
        *out += &match self.background {
            Fill::None => "background none\n".to_owned(),
            Fill::Color(color) => format!("background color {color:06X}\n"),
            Fill::Backdrop(which) => format!("background backdrop {}\n", BACKDROPS[which.min(BACKDROPS.len() - 1)].0),
            Fill::Gradient(from, to, angle) => format!("background gradient {from:06X} {to:06X} {angle}\n"),
        };
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
                    if picture.adjust != [0.0; 3] {
                        *out += &format!("adjust {} {} {}\n", picture.adjust[0], picture.adjust[1], picture.adjust[2]);
                    }
                    if picture.window != frame::Window::None {
                        let which = frame::WINDOWS.iter().position(|(window, _)| *window == picture.window).unwrap_or(0);
                        *out += &format!("window {which} {}\n", one_line(&picture.title));
                    }
                }
                Kind::Shape(shape) => *out += &format!("shape {flags} {}\n", shape.write()),
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
                self.layers.push(Layer { kind: Kind::Picture(picture), hidden, locked });
            }
            // What a picture has besides, on lines after its own.
            "drop" | "adjust" | "window" => {
                let Some(Layer { kind: Kind::Picture(picture), .. }) = self.layers.last_mut() else { return None };
                match line.split(' ').next()? {
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
                    _ => {
                        picture.window = frame::WINDOWS.get(words.next()?.parse::<usize>().ok()?)?.0;
                        picture.title = words.collect::<Vec<_>>().join(" ");
                    }
                }
            }
            "exports" => {
                let mut sizes = Vec::new();
                for word in words.filter(|word| !word.is_empty()) {
                    let mut parts = word.splitn(3, ':');
                    let times = parts.next()?.parse::<f32>().ok()?.clamp(0.1, 8.0);
                    let format = crate::output::Format::named(parts.next()?)?;
                    sizes.push(ExportSize { times, format, suffix: parts.next().unwrap_or("").to_owned() });
                }
                self.exports = sizes;
            }
            "shape" => {
                let rest = line.splitn(4, ' ').collect::<Vec<_>>();
                let shape = Shape::read(rest.get(3)?)?;
                self.layers.push(Layer { kind: Kind::Shape(shape), hidden: rest[1] == "1", locked: rest[2] == "1" });
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
        for frame in &page.frames {
            out += &format!("frame {} {} {}\n", frame.x, frame.y, one_line(&frame.name));
            frame.write_lines(&mut out, &mut images);
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
    for line in lines {
        if let Some(name) = line.strip_prefix("page ") {
            pages.push(Page { name: name.to_owned(), frames: Vec::new() });
            continue;
        }
        if pages.is_empty() {
            pages.push(Page { name: "Page 1".into(), frames: Vec::new() });
        }
        let page = pages.last_mut()?;
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
    for page in &mut pages {
        page.frames.retain(|frame| frame.width > 0 && frame.height > 0);
        if page.frames.is_empty() {
            page.frames.push(Doc::new(1200, 675));
        }
    }
    (!pages.is_empty()).then_some(pages)
}

/// A page of a file: an endless canvas, with frames on it, each a picture
/// to export.
#[derive(Clone)]
pub struct Page {
    pub name: String,
    pub frames: Vec<Doc>,
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
    // The shapes read first, so that the picture is called something
    // none of them is.
    let drawn: Vec<Shape> = shapes.lines().filter_map(Shape::read).collect();
    let image = Image::new(area.w, area.h, pixels.iter().map(|pixel| pixel | 0xFF00_0000).collect());
    let middle = (room as f32 + area.w as f32 / 2.0, room as f32 + area.h as f32 / 2.0);
    let mut picture = Picture::new(Arc::new(image), "Screenshot".into(), middle);
    (picture.radius, picture.shadow) = (10.0, frame::SHADOWS.len());
    doc.layers.push(Layer::picture(picture));
    for mut shape in drawn {
        shape.translate((room - area.x) as f32, (room - area.y) as f32);
        doc.layers.push(Layer::shape(shape));
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
/// the size it is shown at, kept to its rounded corners.
fn draw_picture(out: &mut [u32], part: Rect, picture: &Picture, times: f32, text: &mut Text) {
    let (wide, high) = (part.w, part.h);
    let mut at = picture.clone();
    (at.cx, at.cy, at.w, at.h, at.radius) = (picture.cx * times - part.x as f32, picture.cy * times - part.y as f32, picture.w * times, picture.h * times, picture.radius * times);
    let picture = &at;
    let custom = [picture.drop];
    let preset = Shadow::of_preset(picture.shadow.saturating_sub(1));
    let layers = match picture.shadow {
        0 => &[][..],
        CUSTOM_SHADOW => &custom[..],
        _ => preset.layers(),
    };
    // Tailwind's shadows are for a page seen at its size; an artboard is
    // most often seen at half its own, and its shadows are twice as large.
    // One set number by number is in pixels of the artboard.
    let lift = if picture.shadow == CUSTOM_SHADOW { times } else { times * SHADOW_SIZE };
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
    // A window's bar over the top of it, and the picture in what is left.
    let bar = picture.bar() * times;
    let radius = if bar > 0.0 { picture.radius.max(10.0 * times) } else { picture.radius }.clamp(0.0, hw.min(hh));
    let (cx, cy, cw, ch) = picture.crop;
    let level = picture.image.level(cw / picture.w);
    let (lw, lh, pixels) = (level.0, level.1, &level.2);
    let (ratio_x, ratio_y) = (lw as f32 / picture.image.width() as f32, lh as f32 / picture.image.height() as f32);
    let opacity = picture.opacity as f32 / 256.0;
    let dark = picture.window == frame::Window::Terminal || (bar > 0.0 && frame::darkish(pixels, lw, lh));
    let bar_scale = bar / 30.0;
    let adjusted = picture.adjust != [0.0; 3];
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
                if ly + hh < bar {
                    let color = frame::window_bar(picture.window, dark, lx + hw, ly + hh, picture.w, bar, bar_scale);
                    pixel = over(pixel, color, cover * opacity);
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
                    let mut color = sample(pixels, lw, lh, fx, fy);
                    if adjusted {
                        color = adjust(color, picture.adjust);
                    }
                    pixel = over(pixel, color & 0xFF_FFFF, cover * (color >> 24) as f32 / 255.0 * opacity);
                }
            }
            line[x as usize] = pixel;
        }
    });
    // What the window is called, in the middle of its bar, square to the
    // artboard only.
    if bar >= 12.0 && !picture.title.is_empty() && picture.angle == 0.0 {
        let px = ((bar * 0.42).round() as u32).max(7);
        let (mid_x, mid_y) = ((picture.cx).round() as i32, (picture.cy - hh + bar / 2.0).round() as i32);
        let band = Rect::new((picture.cx - hw) as i32 + (bar * 2.6) as i32, (picture.cy - hh) as i32, (picture.w - bar * 5.2).max(0.0) as i32, bar.ceil() as i32).intersect(Rect::new(0, 0, wide, high));
        if !band.is_empty() {
            let ink = if dark { 0xC9CCD3 } else { 0x4A4D57 };
            let mut canvas = Canvas::new(out, wide, high);
            canvas.set_clip(band);
            text.draw_centered(&mut canvas, mid_x, mid_y, px, ink, &picture.title);
            // Drawing on it left its pixels without how opaque they are.
            for y in band.y..band.bottom() {
                for pixel in &mut out[(y * wide + band.x) as usize..(y * wide + band.right()) as usize] {
                    *pixel |= 0xFF00_0000;
                }
            }
        }
    }
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
    let sheet = body.inflate(reach);
    // Only as much of it as could fall on the picture.
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
    MoveFrame { from: (i32, i32), origin: (f32, f32), moved: bool },
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
    /// A slider dragged along, from where its track begins, so wide.
    Slide { field: Field, left: i32, wide: i32 },
    /// A part of the color picker dragged along.
    Picker { part: u8, rect: Rect },
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
            Field::ExportTimes(_) => "\u{d7}",
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
}

/// What a color in the panel is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Paint {
    Line,
    Fill,
    Background,
    /// The colors a gradient goes from and to.
    From,
    To,
    /// The color of a picture's shadow.
    Shadow,
    /// One color among what is picked out, to be another in all of them.
    Recolor(u32),
}

/// What a click on the panels, or the bar along the top, does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Control {
    Tool(Tool),
    Layer(usize),
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
    /// `frame::WINDOWS`; a name to type, in a field; and what an end of a
    /// line is stuck to.
    Window(u8),
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
    ColorLayer(usize, &'static str),
    /// The strip a gradient is shown on, its stops at its ends.
    GradientBar,
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
    /// What is picked out set side by side with no gaps, across or down;
    /// made the width, the height, the size, the corners, the shadow or
    /// the whole look of the first picked; each lined up with the frame,
    /// six ways or in its middle; and turned by so many degrees.
    Pack(u8),
    Match(u8),
    ToFrame(u8),
    Turn(i16),
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
            Action::RemoveExport(_) | Action::ShadowOff | Action::FillOff | Action::BorderOff => icons::MINUS,
            Action::AddExport | Action::ShadowOn | Action::FillOn | Action::BorderOn => icons::PLUS,
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
    /// The search over everything, while it is open.
    search: Option<Rect>,
    menus: Vec<(Rect, MenuKind)>,
}

/// The whole file as it was, to come back to with undo: its pages, the
/// page shown, and the frame worked in.
#[derive(Clone)]
struct Snapshot {
    pages: Vec<Page>,
    page: usize,
    active: usize,
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
    /// A color as hex, in the picker.
    Hex,
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
    /// Which tool of each group shows in the toolbar: the last one used.
    groups: [Tool; 2],
    /// The list open, if one is; how large the screen is, for the frame
    /// size of it; whether rulers are shown; whether width and height are
    /// kept to each other; layers copied; and the shape crops keep to.
    menu: Option<MenuState>,
    pub screen: (i32, i32),
    rulers: bool,
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
    /// The text being typed into, by its layer.
    editing: Option<usize>,
    /// The picture being cropped, by its layer.
    cropping: Option<usize>,
    /// How the next shape is drawn.
    color: u32,
    fill: Option<u32>,
    size: usize,
    /// How round the corners of the next rectangle are.
    radius: f32,
    face: usize,
    /// How much larger a picture is exported than the artboard.
    export_scale: u32,
    /// A number being typed into its field, and what has been typed.
    typing: Option<(Field, String)>,
    /// How many changes there have been, which says when what is shown
    /// is behind.
    version: u64,
    /// The part of the artboard drawn last, at the size it is shown: at
    /// which change, at which zoom, and which part.
    /// Each frame of the page as drawn last, at the size it is shown: at
    /// which change to it, at which zoom, and which part of it.
    shown: Vec<Option<(u64, f32, Rect, Vec<u32>)>>,
    /// Where it is kept, if it is; and whether it has changed since.
    pub path: Option<PathBuf>,
    edited: bool,
    /// Where pictures go, and what they are called.
    dir: PathBuf,
    filename: String,
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
    /// Whether Shift+1 last fitted the frame, to fit what is picked out next.
    fitted_frame: bool,
    /// The color of the selection opened to its layers, if one is.
    color_open: Option<u32>,
    /// Whether the grid of styles is open.
    styling: bool,
    /// The search over everything, while it is open; and a file it chose
    /// to be opened in place of this one.
    search: Option<search::Search>,
    pub open_next: Option<PathBuf>,
    dirty: bool,
}

impl Studio {
    #[cfg(test)]
    pub fn new(doc: Doc, text: Text, width: i32, height: i32, dir: PathBuf, filename: String) -> Studio {
        Studio::with_pages(vec![Page { name: "Page 1".into(), frames: vec![doc] }], text, width, height, dir, filename)
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
            radius: 8.0,
            face: 0,
            export_scale: 2,
            typing: None,
            version: 0,
            shown: Vec::new(),
            pages: if pages.is_empty() { vec![Page { name: "Page 1".into(), frames: vec![Doc::new(1200, 675)] }] } else { pages },
            page: 0,
            active: 0,
            framing: false,
            frame_picked: false,
            flyout: None,
            palette: None,
            scroll: 0,
            renaming: None,
            groups: [Tool::Rect, Tool::Pen],
            menu: None,
            screen: (1920, 1080),
            rulers: false,
            keep_ratio: false,
            copied: Vec::new(),
            crop_aspect: 0,
            hsv: (0.0, 0.0, 1.0),
            eyedropping: false,
            path: None,
            edited: false,
            dir,
            filename,
            note: None,
            last_press: (0, (-100, -100)),
            guides: Vec::new(),
            preview: None,
            gaps: Vec::new(),
            fitted_frame: false,
            color_open: None,
            styling: false,
            search: None,
            open_next: None,
            dirty: true,
        };
        studio.doc = std::mem::replace(&mut studio.pages[0].frames[0], Doc::new(1, 1));
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
        let name = self.path.as_ref().and_then(|path| path.file_stem()).map_or("Untitled".to_owned(), |name| name.to_string_lossy().into_owned());
        format!("{name}{} \u{2014} hotshot", if self.edited { " \u{2022}" } else { "" })
    }

    fn say(&mut self, what: impl Into<String>) {
        self.note = Some((what.into(), std::time::Instant::now()));
        self.dirty = true;
    }

    /// Something about to change: the whole file kept, to come back to
    /// with undo.
    fn checkpoint(&mut self) {
        let now = self.snapshot();
        self.undo.push(now);
        self.redo.clear();
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { pages: self.gathered(), page: self.page, active: self.active }
    }

    /// The pages, with the frame worked in back in its place.
    pub fn gathered(&self) -> Vec<Page> {
        let mut pages = self.pages.clone();
        pages[self.page].frames[self.active] = self.doc.clone();
        pages
    }

    /// The file as it was at a snapshot, the frame worked in taken out.
    fn restore(&mut self, snapshot: Snapshot) {
        self.pages = snapshot.pages;
        self.page = snapshot.page.min(self.pages.len() - 1);
        self.active = snapshot.active.min(self.pages[self.page].frames.len() - 1);
        self.doc = std::mem::replace(&mut self.pages[self.page].frames[self.active], Doc::new(1, 1));
        self.selected.retain(|&index| index < self.doc.layers.len());
        (self.cropping, self.editing) = (None, None);
        self.shown.clear();
    }

    /// Another frame of the page worked in, the one before put back.
    fn activate(&mut self, frame: usize) {
        if frame == self.active || frame >= self.pages[self.page].frames.len() {
            return;
        }
        self.commit_text();
        let frames = &mut self.pages[self.page].frames;
        std::mem::swap(&mut self.doc, &mut frames[self.active]);
        std::mem::swap(&mut self.doc, &mut frames[frame]);
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
        self.commit_text();
        let back = std::mem::replace(&mut self.doc, Doc::new(1, 1));
        self.pages[self.page].frames[self.active] = back;
        (self.page, self.active) = (page, 0);
        self.doc = std::mem::replace(&mut self.pages[page].frames[0], Doc::new(1, 1));
        self.selected.clear();
        (self.cropping, self.editing, self.frame_picked) = (None, None, false);
        self.shown.clear();
        self.fit();
    }

    /// The frames of the page shown, the one worked in among them.
    fn frame(&self, index: usize) -> &Doc {
        if index == self.active { &self.doc } else { &self.pages[self.page].frames[index] }
    }

    fn frame_count(&self) -> usize {
        self.pages[self.page].frames.len()
    }

    /// A new page, with a frame on it the size of the one worked in.
    fn new_page(&mut self) {
        self.checkpoint();
        let mut frame = Doc::new(self.doc.width, self.doc.height);
        frame.background = self.doc.background;
        let name = format!("Page {}", self.pages.len() + 1);
        self.pages.push(Page { name, frames: vec![frame] });
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
        self.activate(index);
        self.frame_picked = true;
        self.changed();
    }

    /// The frame worked in, gone, where there are others on the page.
    fn delete_frame(&mut self) {
        if self.frame_count() < 2 {
            return self.say("A page keeps one frame");
        }
        self.checkpoint();
        let gone = self.active;
        let next = if gone == 0 { 1 } else { gone - 1 };
        self.activate(next);
        self.pages[self.page].frames.remove(gone);
        if self.active > gone {
            self.active -= 1;
        }
        self.shown.clear();
        self.changed();
    }

    /// Which frame of the page a point of it is in, the topmost.
    fn frame_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        (0..self.frame_count()).rev().find(|&index| {
            let frame = self.frame(index);
            x >= frame.x && y >= frame.y && x < frame.x + frame.width as f32 && y < frame.y + frame.height as f32
        })
    }


    /// Something has changed: drawn again, and the file is behind.
    fn changed(&mut self) {
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
        self.toolbar(&mut layout);
        self.left_panel(&mut layout);
        self.right_panel(&mut layout);
        // The frames' names on the page, to pick a frame by.
        for index in 0..self.frame_count() {
            let (place, _) = self.frame_label(index);
            if self.over_page(place.center()) {
                layout.controls.push((place, Control::Frame(index)));
            }
        }
        self.layout_styles(&mut layout);
        self.layout_search(&mut layout);
        self.layout_menus(&mut layout);
        layout
    }

    /// The tools, floating along the bottom of the page, grouped as
    /// Figma's are: a caret beside a group opens a list of the rest.
    fn toolbar(&mut self, layout: &mut Layout) {
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
            (Control::Panel, 9),
            (Control::Action(Action::Capture), side),
        ];
        let wide = items.iter().map(|(_, wide)| wide + gap).sum::<i32>() - gap + 2 * pad;
        let open = self.open_area();
        let dock = Rect::new(open.x + (open.w - wide) / 2, self.height - 20 - side - 2 * pad, wide, side + 2 * pad);
        let mut x = dock.x + pad;
        let mut group_at = [0; 2];
        for (control, w) in items {
            if let Control::Group(group) = control {
                group_at[usize::from(group)] = x - side - gap;
            }
            if control != Control::Panel {
                layout.controls.push((Rect::new(x, dock.y + pad, w, side), control));
            }
            x += w + gap;
        }
        // The list a group opens, over its button.
        if let Some(group) = self.flyout {
            let tools: &[Tool] = if group == 0 { &[Tool::Rect, Tool::Ellipse, Tool::Line] } else { &[Tool::Pen, Tool::Marker] };
            let (row, w) = (30, 190);
            let list = Rect::new(group_at[usize::from(group)], dock.y - 8 - tools.len() as i32 * row - 10, w, tools.len() as i32 * row + 10);
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
        let panel = Rect::new(INSET, INSET, LEFT_W, self.height - 2 * INSET);
        layout.panels.push(panel);
        let (left, inner) = (panel.x + 12, panel.w - 24);
        let line = Text::line_height(12);
        // The file: the way home, its project, and its name.
        layout.controls.push((Rect::new(left, panel.y + 12, 28, 28), Control::Home));
        let project = self.path.as_ref().and_then(|path| path.parent()).and_then(|dir| dir.file_name()).map_or("Drafts".to_owned(), |name| name.to_string_lossy().into_owned());
        layout.words.push(((left + 38, panel.y + 10), project, DIM_TEXT_COLOR, 11));
        let name = self.title().trim_end_matches(" \u{2014} hotshot").to_owned();
        layout.words.push(((left + 38, panel.y + 25), name, TEXT_COLOR, 13));
        // The pages.
        let mut y = panel.y + 56;
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
        y += 28;
        for frame in 0..self.frame_count() {
            if y + row > panel.bottom() - 8 {
                break;
            }
            layout.controls.push((Rect::new(left - 4, y, inner + 8, row), Control::Frame(frame)));
            y += row;
            if frame != self.active {
                continue;
            }
            for index in (0..self.doc.layers.len()).rev() {
                if y + row > panel.bottom() - 8 {
                    break;
                }
                layout.controls.push((Rect::new(left + inner - 44, y, 22, row), Control::Eye(index)));
                layout.controls.push((Rect::new(left + inner - 20, y, 22, row), Control::Lock(index)));
                layout.controls.push((Rect::new(left + 14, y, inner - 10, row), Control::Layer(index)));
                y += row;
            }
        }
        layout.controls.push((panel, Control::Panel));
    }

    /// The panel down the right: the zoom and the way out at its top, and
    /// under them what there is to set of what is picked out, or of the
    /// frame, a section at a time, scrolled if it is long.
    fn right_panel(&mut self, layout: &mut Layout) {
        let panel = Rect::new(self.width - INSET - RIGHT_W, INSET, RIGHT_W, self.height - 2 * INSET);
        layout.panels.push(panel);
        let (left, inner) = (panel.x + 14, panel.w - 28);
        layout.controls.push((Rect::new(left - 4, panel.y + 10, 64, 30), Control::Zoom));
        layout.controls.push((Rect::new(left + inner - 92, panel.y + 10, 92, 30), Control::Action(Action::Export)));
        let top = panel.y + 50;
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
        let left = INSET + LEFT_W + INSET;
        let right = self.width - INSET - RIGHT_W - INSET;
        Rect::new(left, INSET, (right - left).max(100), (self.height - INSET - DOCK_ROOM).max(100))
    }

    /// The frame worked in made to fit the open part of the window, with
    /// room round it.
    fn fit(&mut self) {
        let open = self.open_area();
        let (w, h) = (self.doc.width as f32, self.doc.height as f32);
        self.zoom = (open.w as f32 * 0.86 / w).min(open.h as f32 * 0.86 / h).clamp(0.02, 8.0);
        let middle = (open.x as f32 + open.w as f32 / 2.0, open.y as f32 + open.h as f32 / 2.0);
        self.pan = (middle.0 - (self.doc.x + w / 2.0) * self.zoom, middle.1 - (self.doc.y + h / 2.0) * self.zoom);
        self.dirty = true;
    }

    /// Larger or smaller, about a point of the window, which stays over
    /// the same point of the page.
    fn zoom_about(&mut self, times: f32, at: (i32, i32)) {
        let before = self.to_page(at);
        self.zoom = (self.zoom * times).clamp(0.02, 16.0);
        self.pan = (at.0 as f32 + 0.5 - before.0 * self.zoom, at.1 as f32 + 0.5 - before.1 * self.zoom);
        self.dirty = true;
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

    /// Whether a point of the window is over the page, and not a panel.
    fn over_page(&self, (x, _): (i32, i32)) -> bool {
        let left = x < INSET + LEFT_W + INSET;
        let right = x >= self.width - INSET - RIGHT_W - INSET;
        !(left || right)
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
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Text | Tool::Badge) => {
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

    pub fn pointer_moved(&mut self, x: i32, y: i32, mods: Mods) {
        (self.pointer, self.mods) = ((x, y), mods);
        let at = self.to_doc((x, y));
        match self.drag {
            Drag::Picker { part, rect } => self.pick_at(part, rect, (x, y), false),
            Drag::Slide { field, left, wide } => self.slide_to(field, left, wide, x),
            Drag::None => {
                // What a control the pointer rests on would do, shown.
                let over = self.control_at((x, y));
                self.hover_preview(over);
                // Over an open list, the list a row of it opens.
                if self.menu.is_some() {
                    let control = self.control_at((x, y));
                    self.hover_menu(control);
                }
                let hovered = match (self.tool, self.over_page((x, y)) && !self.framing) {
                    (Tool::Select, true) => self.layer_at(at),
                    _ => None,
                };
                self.dirty |= hovered != self.hovered;
                self.hovered = hovered;
                self.dirty = true;
            }
            Drag::NewFrame { .. } => self.dirty = true,
            Drag::MoveFrame { from, origin, moved } => {
                let (dx, dy) = ((x - from.0) as f32 / self.zoom, (y - from.1) as f32 / self.zoom);
                if !moved && (x - from.0).abs().max((y - from.1).abs()) < 3 {
                    return;
                }
                if !moved {
                    self.checkpoint();
                    self.drag = Drag::MoveFrame { from, origin, moved: true };
                }
                (self.doc.x, self.doc.y) = ((origin.0 + dx).round(), (origin.1 + dy).round());
                self.changed();
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
                    self.let_go();
                }
                // Snapped to the artboard and to the rest, but for along
                // the way Shift holds it to, or with Ctrl held.
                let (mut dx, mut dy) = if mods.ctrl { (dx.round(), dy.round()) } else { self.snap(dx.round(), dy.round()) };
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
        self.end_preview();
        // With the search open, a click off it closes it.
        if self.search.is_some() && !matches!(self.control_at(self.pointer), Some(Control::Found(_) | Control::SearchTab(_) | Control::Panel)) {
            self.search = None;
            self.dirty = true;
            return Outcome::Continue;
        }
        let at_window = self.pointer;
        let double = time.wrapping_sub(self.last_press.0) < 400 && (at_window.0 - self.last_press.1 .0).abs().max((at_window.1 - self.last_press.1 .1).abs()) <= 3;
        self.last_press = (time, at_window);
        self.dirty = true;
        match button {
            // The wheel: up and down, or with Shift across; with Ctrl,
            // larger and smaller about the pointer.
            4 | 5 | 6 | 7 if self.pointer.0 >= self.width - INSET - RIGHT_W - INSET => {
                // Over the panel of properties: that, scrolled.
                self.scroll = (self.scroll + if button == 4 { -48 } else if button == 5 { 48 } else { 0 }).max(0);
                return Outcome::Continue;
            }
            4 | 5 | 6 | 7 if self.pointer.0 < INSET + LEFT_W + INSET => return Outcome::Continue,
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
            3 => return self.right_click(at_window),
            1 => {}
            _ => return Outcome::Continue,
        }
        if self.typing.is_some() && self.control_at(at_window) != self.typing.as_ref().map(|(field, _)| Control::Field(*field)) {
            self.commit_typing();
        }
        let control = self.control_at(at_window);
        if let Some(control) = control {
            let outcome = self.press_control(control, double);
            // A frame's name on the page is what it is moved by.
            if let (Control::Frame(_), false, true) = (control, double, self.over_page(at_window)) {
                self.drag = Drag::MoveFrame { from: at_window, origin: (self.doc.x, self.doc.y), moved: false };
            }
            return outcome;
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
        (self.palette, self.flyout) = (None, None);
        self.commit_name();
        if self.space || self.tool == Tool::Select && self.mods.alt {
            self.drag = Drag::Pan { from: at_window, origin: self.pan };
            return Outcome::Continue;
        }
        if self.framing {
            self.drag = Drag::NewFrame { from: self.to_page(at_window) };
            return Outcome::Continue;
        }
        // In another frame, that one worked in.
        if let Some(frame) = self.frame_at(self.to_page(at_window)).filter(|&frame| frame != self.active && self.cropping.is_none()) {
            self.activate(frame);
        }
        self.frame_picked = false;
        let at = self.to_doc(at_window);
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
                self.edit_text(self.doc.layers.len() - 1);
                self.selected = vec![self.doc.layers.len() - 1];
                self.changed();
            }
            tool => {
                self.checkpoint();
                let mut shape = Shape::new(tool, self.color, STROKES[self.size], at);
                if matches!(tool, Tool::Rect | Tool::Ellipse) {
                    (shape.filled, shape.fill) = (self.fill.is_some(), self.fill.unwrap_or(0xFFFFFF));
                    shape.radius = if tool == Tool::Rect { self.radius } else { 0.0 };
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
            let corners: Vec<(i32, i32)> = handles.iter().filter(|(_, pull)| pull.0 != END).take(4).map(|&(spot, _)| spot).collect();
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
                            self.edit_text(index);
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
        // The colors stay open while colors are picked, and a group's list
        // while it is picked from; anything else closes them.
        if !matches!(control, Control::Swatch(..) | Control::ColorField(_) | Control::Picker(_) | Control::Entry(Naming::Hex)) {
            self.palette = None;
        }
        if !matches!(control, Control::Group(_) | Control::Pick(_)) {
            self.flyout = None;
        }
        if !matches!(control, Control::Page(_) | Control::Frame(_)) {
            self.commit_name();
        }
        if !matches!(control, Control::MenuRow(..)) {
            self.menu = None;
        }
        match control {
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
                let window = frame::WINDOWS[usize::from(which).min(frame::WINDOWS.len() - 1)].0;
                self.restyle(|layer| {
                    if let Kind::Picture(picture) = &mut layer.kind {
                        picture.window = window;
                    }
                });
            }
            Control::Entry(naming) => {
                let now = match naming {
                    Naming::Title(index) => match self.doc.layers.get(index).map(|layer| &layer.kind) {
                        Some(Kind::Picture(picture)) => picture.title.clone(),
                        _ => String::new(),
                    },
                    Naming::Suffix(index) => self.doc.exports.get(index).map_or(String::new(), |size| size.suffix.clone()),
                    Naming::Frame => self.doc.name.clone(),
                    Naming::Page(page) => self.pages[page].name.clone(),
                    Naming::Hex => format!("{:06X}", self.palette.and_then(|paint| self.paint_color(paint)).unwrap_or(0)),
                };
                self.renaming = Some((naming, now));
            }
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
                true => self.renaming = Some((Naming::Page(index), self.pages[index].name.clone())),
                false => self.go_to_page(index),
            },
            Control::NewPage => self.new_page(),
            Control::Frame(index) => {
                self.activate(index);
                (self.frame_picked, self.tool, self.framing) = (true, Tool::Select, false);
                self.selected.clear();
                if double {
                    self.renaming = Some((Naming::Frame, self.doc.name.clone()));
                }
            }
            Control::FrameTool => {
                self.commit_text();
                (self.framing, self.tool) = (!self.framing, Tool::Select);
                self.selected.clear();
            }
            Control::Group(group) => self.flyout = if self.flyout == Some(group) { None } else { Some(group) },
            Control::Pick(tool) => {
                let group = usize::from(matches!(tool, Tool::Pen | Tool::Marker));
                self.groups[group] = tool;
                self.flyout = None;
                self.set_tool(tool);
            }
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
            Control::ColorOpen(color) => self.color_open = if self.color_open == Some(color) { None } else { Some(color) },
            Control::ColorLayer(index, _) => self.selected = vec![index],
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
                        _ => Fill::Gradient(0x3E63DD, 0xD6409F, 135.0),
                    },
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
            Paint::From | Paint::To => {
                let Fill::Gradient(from, to, angle) = self.doc.background else { return };
                let color = color.unwrap_or(0xFFFFFF);
                self.checkpoint();
                self.doc.background = if paint == Paint::From { Fill::Gradient(color, to, angle) } else { Fill::Gradient(from, color, angle) };
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
                    }
                });
                if self.palette == Some(Paint::Recolor(was)) {
                    self.palette = Some(Paint::Recolor(color));
                }
            }
        }
    }

    fn act(&mut self, action: Action) -> Outcome {
        self.dirty = true;
        match action {
            Action::Export => self.export(),
            Action::Copy => self.copy(),
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
                    (picture.w, picture.h) = (picture.crop.2, picture.crop.3);
                }
            }),
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
        (self.tool, self.framing) = (tool, false);
        if tool != Tool::Select {
            self.frame_picked = false;
        }
        if tool != Tool::Select {
            self.selected.clear();
        }
        self.dirty = true;
    }

    pub fn released(&mut self) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        self.dirty = true;
        match drag {
            // A frame dragged out; a click with the frame tool, one the size
            // of the one worked in, there.
            Drag::NewFrame { from } => {
                let to = self.to_page(self.pointer);
                let (w, h) = ((to.0 - from.0).abs(), (to.1 - from.1).abs());
                let size = if w * self.zoom < 4.0 && h * self.zoom < 4.0 { (self.doc.width as f32, self.doc.height as f32) } else { (w, h) };
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
            }
            Drag::Move { moved: false, .. } => {}
            _ => {}
        }
        self.held.clear();
        self.guides.clear();
        self.gaps.clear();
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

    /// How far what is held is to go, made to bring its edges or its
    /// middle onto those of the artboard or of another layer near enough,
    /// as Figma's do: and the lines it is brought onto, to show.
    fn snap(&mut self, dx: f32, dy: f32) -> (f32, f32) {
        self.guides.clear();
        self.gaps.clear();
        let held: Vec<(f32, f32, f32, f32)> = self.held.clone().iter().map(|layer| layer.bounds(&mut self.text)).collect();
        let Some(&(mut left, mut top, _, _)) = held.first() else { return (dx, dy) };
        let (mut right, mut bottom) = (f32::MIN, f32::MIN);
        for &(x, y, w, h) in &held {
            (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x + w), bottom.max(y + h));
        }
        let chosen = self.chosen();
        let (mut across, mut down) = (vec![0.0, self.doc.width as f32 / 2.0, self.doc.width as f32], vec![0.0, self.doc.height as f32 / 2.0, self.doc.height as f32]);
        for index in 0..self.doc.layers.len() {
            if chosen.contains(&index) || self.doc.layers[index].hidden {
                continue;
            }
            let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
            across.extend([x, x + w / 2.0, x + w]);
            down.extend([y, y + h / 2.0, y + h]);
        }
        let reach = 6.0 / self.zoom;
        // The nearest of its edges and middle to any of the lines.
        let nearest = |mine: [f32; 3], lines: &[f32]| -> Option<(f32, f32)> {
            let mut best: Option<(f32, f32)> = None;
            for &line in lines {
                for &at in &mine {
                    let by = line - at;
                    if by.abs() <= reach && best.is_none_or(|(was, _)| by.abs() < was.abs()) {
                        best = Some((by, line));
                    }
                }
            }
            best
        };
        // Or, nearer still, where it is spaced evenly with what is beside it.
        let others = self.other_edges();
        let moved = [left + dx, top + dy, right + dx, bottom + dy];
        let (mut dx, mut dy) = (dx, dy);
        let line_x = nearest([left + dx, (left + right) / 2.0 + dx, right + dx], &across);
        match (line_x, snap::equal_gap(moved, &others, reach)) {
            (line, Some((by, gaps))) if line.is_none_or(|(line_by, _)| by.abs() < line_by.abs()) => {
                dx += by;
                self.gaps.extend(gaps);
            }
            (Some((by, line)), _) => {
                dx += by;
                self.guides.push((true, line));
            }
            _ => {}
        }
        let line_y = nearest([top + dy, (top + bottom) / 2.0 + dy, bottom + dy], &down);
        match (line_y, snap::equal_gap_down(moved, &others, reach)) {
            (line, Some((by, gaps))) if line.is_none_or(|(line_by, _)| by.abs() < line_by.abs()) => {
                dy += by;
                self.gaps.extend(gaps);
            }
            (Some((by, line)), _) => {
                dy += by;
                self.guides.push((false, line));
            }
            _ => {}
        }
        (dx, dy)
    }

    /// The one picked out pulled by a handle to a point: the side or
    /// corner held goes there, across the picture unturned, and the one
    /// across from it stays where it is; with Shift, it keeps its shape.
    fn scale_to(&mut self, pull: (i32, i32), from: (f32, f32), at: (f32, f32), mods: Mods) {
        let Some(index) = self.picked() else { return };
        let Some(before) = self.held.first().cloned() else { return };
        // Widths and heights the rest have, for a side pulled to stop at.
        let others = self.other_edges();
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
        if field == Field::Angle {
            return match self.doc.background {
                Fill::Gradient(_, _, angle) => Some(angle),
                _ => None,
            };
        }
        if let Field::ExportTimes(index) = field {
            return self.doc.exports.get(usize::from(index)).map(|size| size.times);
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
                Field::Radius => picture.radius,
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
                Field::Radius => shape.radius,
                Field::Opacity => shape.opacity as f32 * 100.0 / 256.0,
                Field::FontSize => shape.font_px as f32,
                Field::Stroke => shape.stroke,
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
            // The next rectangle as round as this one is made.
            Field::Radius if self.picked().is_some_and(|index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.tool == Tool::Rect)) => {
                self.radius = value.max(0.0);
                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.picked().and_then(|index| self.doc.layers.get_mut(index)) {
                    shape.radius = value.max(0.0);
                    shape.touch();
                }
            }
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
                        (Kind::Picture(picture), Field::Radius) => picture.radius = value.max(0.0),
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

    /// The name being typed given to what it names; none given, none.
    fn commit_name(&mut self) {
        let Some((naming, typed)) = self.renaming.take() else { return };
        let typed = typed.trim().to_owned();
        // A frame or a page is called something; a title or a suffix may
        // be nothing.
        if typed.is_empty() && matches!(naming, Naming::Frame | Naming::Page(_)) {
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
            Naming::Hex => {}
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
        self.editing = Some(index);
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.editing = true;
            shape.touch();
        }
    }

    fn commit_text(&mut self) {
        if let Some(index) = self.editing.take() {
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                shape.editing = false;
                shape.touch();
            }
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
            match &mut copy.kind {
                Kind::Picture(picture) => picture.id = shapes::fresh_id(),
                Kind::Shape(shape) => *shape = shape.copied(),
            }
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
        let middle = self.to_doc(self.open_area().center());
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

    /// The artboard as a PNG, at the size export is set to.
    fn png(&mut self) -> Option<Vec<u8>> {
        let (wide, high, pixels) = self.doc.render(self.export_scale as f32, &mut self.text);
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
                let bytes = crate::output::pack(size.format, 90, wide as u32, high as u32, &pixels, clear)?;
                let stem = match self.path {
                    Some(_) => one_line(&frame.name).replace('/', "-"),
                    None => format!("{}-{}", crate::output::dated(&self.filename), one_line(&frame.name).replace('/', "-")),
                };
                let name = format!("{stem}{}.{}", size.suffix, size.format.ending());
                let path = dir.join(name);
                std::fs::write(&path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
                written.push(path);
            }
        }
        Ok(written)
    }

    fn export(&mut self) {
        let active = self.active;
        match self.export_frames(&[active]) {
            Ok(written) if written.len() == 1 => self.say(format!("Exported {}", shortened(&written[0]))),
            Ok(written) => self.say(format!("Exported {} pictures to {}", written.len(), shortened(&self.export_dir()))),
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
        if down {
            self.end_preview();
        }
        if self.search.is_some() {
            self.mods = mods;
            return if down { self.search_key(key, mods) } else { Outcome::Continue };
        }
        self.mods = mods;
        self.dirty = true;
        if key == Key::Char(' ') && self.editing.is_none() && self.typing.is_none() && self.renaming.is_none() {
            self.space = down;
            return Outcome::Continue;
        }
        if !down {
            return Outcome::Continue;
        }
        // A name being typed.
        if let Some((_, typed)) = &mut self.renaming {
            match key {
                Key::Char(letter) if !mods.ctrl => typed.push(letter),
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
            // Style Mode: S opens the grid of styles, a letter puts one on.
            (Key::Escape, _) if self.styling => self.styling = false,
            (Key::Char(letter), false) if self.styling && style::STYLE_KEYS.contains(&letter) => {
                let at = style::STYLE_KEYS.iter().position(|&known| known == letter).unwrap_or(0);
                self.put_style(at as u8);
                self.styling = false;
            }
            (Key::Char('s'), false) if !self.styling && self.styles_apply() => self.styling = true,
            (Key::Char('q' | 'w'), true) => return Outcome::Quit,
            (Key::Char('z'), true) if mods.shift => self.redo(),
            (Key::Char('z'), true) => self.undo(),
            (Key::Char('y'), true) => self.redo(),
            (Key::Char('a'), true) => self.selected = (0..self.doc.layers.len()).filter(|&index| !self.doc.layers[index].locked).collect(),
            (Key::Char('d'), true) => self.duplicate(),
            (Key::Char('c'), true) if mods.shift => self.copy(),
            (Key::Char('c'), true) if !self.chosen().is_empty() => self.copy_layers(),
            (Key::Char('c'), true) => self.copy(),
            (Key::Char('k'), true) => self.open_palette(),
            (Key::Char('e'), true) if mods.shift => drop(self.run_command(menu::Command::ExportPage)),
            (Key::Char('v'), true) if !self.copied.is_empty() && !mods.shift => self.paste_layers(),
            (Key::Char('v'), true) => return Outcome::Paste,
            (Key::Char('s'), true) => self.save(true),
            (Key::Char('e'), true) => self.export(),
            (Key::Char('g'), true) => return Outcome::Capture,
            (Key::Char(']' | '}'), true) => self.restack(true),
            (Key::Char('[' | '{'), true) => self.restack(false),
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
            (Key::Escape, _) if self.menu.is_some() => self.menu = None,
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
                    self.checkpoint();
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
            (Key::Char(letter), false) => {
                if let Some(&(tool, ..)) = TOOLS.iter().find(|(_, _, key)| key.to_ascii_lowercase().starts_with(letter)) {
                    self.set_tool(tool);
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
    /// of it in the view and as much again round that, so that a little
    /// panning needs nothing drawn. Drawn again when it changes, when the
    /// zoom does, or when the view goes past what was drawn. Gives where
    /// its corner is in the window, and how large it is shown.
    fn shown_frame(&mut self, index: usize, view: Rect) -> ((i32, i32), i32, i32) {
        let frame = self.frame(index);
        let (wide, high) = (((frame.width as f32 * self.zoom).round() as i32).max(1), ((frame.height as f32 * self.zoom).round() as i32).max(1));
        let origin = self.to_window_page((frame.x, frame.y));
        let stamp = frame.stamp;
        // What of it is in the view, counted from its corner.
        let seen = view.moved(-origin.0, -origin.1).intersect(Rect::new(0, 0, wide, high));
        if self.shown.len() < self.frame_count() {
            self.shown.resize_with(self.frame_count(), || None);
        }
        let fresh = self.shown[index].as_ref().is_some_and(|(kept, zoom, part, _)| *kept == stamp && *zoom == self.zoom && (seen.is_empty() || part.intersect(seen) == seen));
        if !fresh && !seen.is_empty() {
            let part = seen.inflate(view.w.max(view.h) / 2).intersect(Rect::new(0, 0, wide, high));
            let pixels = if index == self.active { self.doc.render_part(self.zoom, part, &mut self.text) } else { self.pages[self.page].frames[index].render_part(self.zoom, part, &mut self.text) };
            self.shown[index] = Some((stamp, self.zoom, part, pixels));
        }
        (origin, wide, high)
    }

    pub fn draw(&mut self, out: &mut [u32]) {
        let (width, height) = (self.width, self.height);
        let layout = self.layout();
        let view = layout.view;
        out.iter_mut().for_each(|pixel| *pixel = VIEW_COLOR);
        // The page: dots on its ground, the frames on that, each over light
        // squares where it is see-through.
        let dots = 24;
        let (ox, oy) = (self.pan.0.rem_euclid(dots as f32) as i32, self.pan.1.rem_euclid(dots as f32) as i32);
        for y in (view.y + oy..view.bottom()).step_by(dots as usize) {
            for x in (view.x + ox..view.right()).step_by(dots as usize) {
                out[(y * width + x) as usize] = 0x2E2E2E;
            }
        }
        for index in 0..self.frame_count() {
            let (origin, wide, high) = self.shown_frame(index, view);
            let shown = Rect::new(origin.0, origin.1, wide, high).intersect(view);
            let Some(Some((_, _, part, pixels))) = self.shown.get(index) else { continue };
            let drawn = part.moved(origin.0, origin.1).intersect(shown);
            for y in drawn.y..drawn.bottom() {
                let row = &mut out[(y * width + drawn.x) as usize..(y * width + drawn.right()) as usize];
                let from = &pixels[((y - origin.1 - part.y) * part.w + drawn.x - origin.0 - part.x) as usize..][..drawn.w as usize];
                for (at, (pixel, &color)) in row.iter_mut().zip(from).enumerate() {
                    let alpha = color >> 24;
                    let x = drawn.x + at as i32;
                    let light = ((x - origin.0) / 10 + (y - origin.1) / 10) % 2 == 0;
                    let ground = if light { 0xFFFFFF } else { 0xE6E6E6 };
                    *pixel = if alpha == 255 { color & 0xFF_FFFF } else { canvas::mix(ground, color & 0xFF_FFFF, alpha + (alpha >> 7)) };
                }
            }
        }
        // Each frame's name over its corner; the one worked in, and the one
        // picked out, lit.
        let mut canvas = Canvas::new(out, width, height);
        canvas.set_clip(view);
        for index in 0..self.frame_count() {
            let (place, name) = self.frame_label(index);
            let color = if index == self.active && self.frame_picked { ACCENT_TEXT } else if index == self.active { TEXT_COLOR } else { DIM_TEXT_COLOR };
            let name = match &self.renaming {
                Some((Naming::Frame, typed)) if index == self.active => format!("{typed}|"),
                _ => name,
            };
            self.text.draw(&mut canvas, place.x, place.y, 12, color, &name);
        }
        if self.frame_picked {
            let (a, b) = (self.to_window((0.0, 0.0)), self.to_window((self.doc.width as f32, self.doc.height as f32)));
            canvas.round_outline(a.0 - 1, a.1 - 1, b.0 - a.0 + 2, b.1 - a.1 + 2, 0, 2, ACCENT);
        }
        if let Drag::NewFrame { from } = self.drag {
            let reach = Rect::spanning(self.to_window_page(from), self.pointer);
            canvas.round_fill(reach.x, reach.y, reach.w, reach.h, 0, 0xFFFFFF, 30);
            canvas.round_outline(reach.x, reach.y, reach.w, reach.h, 0, 1, ACCENT);
        }
        if let Some(index) = self.hovered.filter(|index| !self.selected.contains(index) && *index < self.doc.layers.len()) {
            self.outline(&mut canvas, index, 0x7A93EA);
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
            self.outline(&mut canvas, index, ACCENT);
        }
        // Several picked out, each says when it was: matching takes after
        // the first.
        let chosen = self.chosen();
        if chosen.len() > 1 && !previewing {
            for (order, &index) in chosen.iter().enumerate() {
                let (x, y, _, _) = self.doc.layers[index].bounds(&mut self.text);
                let (wx, wy) = self.to_window((x, y));
                let says = format!("{}", order + 1);
                let wide = self.text.width(&says, 10) + 8;
                canvas.round_fill(wx + 3, wy + 3, wide, 14, 3, ACCENT, 256);
                self.text.draw(&mut canvas, wx + 7, wy + 3 + (14 - Text::line_height(10)) / 2, 10, 0xFFFFFF, &says);
            }
        }
        if self.cropping.is_none() && !previewing {
            for (spot, _) in self.handles() {
                canvas.fill(spot.0 - 5, spot.1 - 5, 10, 10, ACCENT);
                canvas.fill(spot.0 - 4, spot.1 - 4, 8, 8, 0xFFFFFF);
            }
        }
        if let Some(index) = self.cropping {
            self.draw_crop(&mut canvas, index);
        }
        self.draw_gaps(&mut canvas);
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
                    _ => {
                        let last = shape.text.rsplit('\n').next().unwrap_or("");
                        let line = Text::line_height(shape.font_px);
                        (self.text.width_in(shape.words_face(), last, shape.font_px), (shape.text.split('\n').count() as i32 - 1) * line, line)
                    }
                };
                let (x, y) = self.to_window(((place.x + along) as f32, (place.y + top) as f32));
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
            // A shape turned by an angle of its own, by its own box turned.
            Kind::Shape(shape) if shape.turnable() && shape.angle != 0.0 => {
                let (x, y, w, h) = match shape.tool {
                    Tool::Text | Tool::Badge => {
                        let place = shape.local_bounds(&mut self.text);
                        (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
                    }
                    _ => shape.frame(),
                };
                let corners = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)].map(|corner| shape.returned(corner, &mut self.text));
                let corners = corners.map(|corner| self.to_window(corner));
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
        for place in [Some(layout.dock), layout.flyout, layout.popover, layout.styles, layout.search].into_iter().flatten() {
            for grow in (1..=6).rev() {
                canvas.round_fill(place.x - grow, place.y - grow + 3, place.w + 2 * grow, place.h + 2 * grow, 12 + grow, 0x000000, 14);
            }
            canvas.round_fill(place.x, place.y, place.w, place.h, 12, PANEL_COLOR, 256);
            canvas.round_outline(place.x, place.y, place.w, place.h, 12, 1, EDGE_COLOR);
        }
        for rule in &layout.rules {
            canvas.fill(rule.x + 1, rule.y, rule.w - 2, 1, EDGE_COLOR);
        }
        if let Some(place) = layout.styles {
            self.draw_styles_frame(canvas, place);
        }
        if let Some(place) = layout.search {
            self.draw_search(canvas, place, hover);
        }
        // The group's divider in the bar of tools.
        let divider = layout.controls.iter().find(|(_, control)| *control == Control::Action(Action::Capture)).map(|(rect, _)| rect.x - 6);
        if let Some(x) = divider {
            canvas.fill(x, layout.dock.y + 14, 1, layout.dock.h - 28, HOVER_COLOR);
        }
        let tool_icon = |tool: Tool| match tool {
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
                    let name = match &self.renaming {
                        Some((Naming::Page(page), typed)) if *page == index => format!("{typed}|"),
                        _ => self.pages[index].name.clone(),
                    };
                    self.text.draw(canvas, rect.x + 28, cy - line_high / 2, px, TEXT_COLOR, &name);
                    if on {
                        let count = format!("{}", self.frame_count());
                        let wide = self.text.width(&count, 11);
                        self.text.draw(canvas, rect.right() - 10 - wide, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &count);
                    }
                }
                Control::Frame(index) if rect.x < INSET + LEFT_W => {
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
                }
                Control::Frame(_) => {}
                Control::Layer(index) => {
                    let on = chosen.contains(&index);
                    if on || hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    let layer = &self.doc.layers[index];
                    let icon = match &layer.kind {
                        Kind::Picture(_) => icons::PICTURE,
                        Kind::Shape(shape) => match shape.tool {
                            Tool::Badge => icons::BADGE,
                            tool => tool_icon(tool),
                        },
                    };
                    let (name, faint) = (layer.name(), layer.hidden);
                    let ink = if faint { DIM_TEXT_COLOR } else { TEXT_COLOR };
                    self.text.draw_centered(canvas, rect.x + 12, cy, 14, ink, icon);
                    let name: String = name.chars().take(18).collect();
                    self.text.draw(canvas, rect.x + 28, cy - line_high / 2, px, ink, &name);
                }
                Control::Eye(index) => {
                    let layer = &self.doc.layers[index];
                    let row_hovered = matches!(hover, Some(Control::Layer(at) | Control::Eye(at) | Control::Lock(at)) if at == index);
                    if layer.hidden || row_hovered {
                        self.text.draw_centered(canvas, cx, cy, 14, if layer.hidden { DIM_TEXT_COLOR } else { TEXT_COLOR }, if layer.hidden { icons::EYE_OFF } else { icons::EYE });
                    }
                }
                Control::Lock(index) => {
                    let layer = &self.doc.layers[index];
                    let row_hovered = matches!(hover, Some(Control::Layer(at) | Control::Eye(at) | Control::Lock(at)) if at == index);
                    if layer.locked || row_hovered {
                        self.text.draw_centered(canvas, cx, cy, 14, if layer.locked { TEXT_COLOR } else { DIM_TEXT_COLOR }, icons::LOCK);
                    }
                }
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
                        None => self.field(field).map_or("\u{2014}".to_owned(), |value| match field {
                            Field::Opacity => format!("{}%", value.round() as i32),
                            Field::Turn | Field::Angle | Field::LineAngle => format!("{}\u{b0}", value.round() as i32),
                            _ => format!("{}", value.round() as i32),
                        }),
                    };
                    let (label, icon) = match field {
                        Field::Turn | Field::Angle | Field::LineAngle => (icons::ROTATE, true),
                        Field::Radius => (icons::RADIUS, true),
                        Field::Opacity => (icons::EYE, true),
                        other => (other.label(), false),
                    };
                    field_text(&mut self.text, canvas, rect, label, icon, &value, DIM_TEXT_COLOR);
                }
                Control::Style(style) => self.draw_style_tile(canvas, rect, style, hovered),
                Control::Found(row) => self.draw_hit(canvas, rect, usize::from(row), hovered),
                Control::SearchTab(_) => {}
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
                    if let Fill::Gradient(from, to, _) = self.doc.background {
                        let inner = rect.inflate(-1);
                        for x in 0..inner.w {
                            let along = (x * 256 / inner.w.max(1)) as u32;
                            canvas.fill(inner.x + x, inner.y + 3, 1, inner.h - 6, crate::canvas::mix(from, to, along));
                        }
                        // A stop at each end, ringed; lit while its color is open.
                        for (x, paint, color) in [(rect.x + 1, Paint::From, from), (rect.right() - 13, Paint::To, to)] {
                            let lit = self.palette == Some(paint);
                            canvas.round_fill(x - 1, rect.y - 1, 14, rect.h + 2, 4, if lit { ACCENT } else { 0xFFFFFF }, 256);
                            canvas.round_fill(x + 2, rect.y + 2, 8, rect.h - 4, 2, color, 256);
                        }
                    }
                }
                Control::ColorField(paint) => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                    if hovered || self.palette == Some(paint) {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if self.palette == Some(paint) { ACCENT } else { 0x555555 });
                    }
                    let now = match (paint, &shown, self.doc.background) {
                        (Paint::Background, _, Fill::Color(color)) => Some(color),
                        (Paint::From, _, Fill::Gradient(from, ..)) => Some(from),
                        (Paint::To, _, Fill::Gradient(_, to, _)) => Some(to),
                        (Paint::Line, Some(Layer { kind: Kind::Shape(shape), .. }), _) => Some(shape.color),
                        (Paint::Fill, Some(Layer { kind: Kind::Shape(shape), .. }), _) => shape.filled.then_some(shape.fill),
                        (Paint::Shadow, Some(layer), _) => layer.drop().map(|drop| drop.color & 0xFF_FFFF),
                        (Paint::Recolor(color), _, _) => Some(color),
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
                        Paint::To => "To".to_owned(),
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
                        (Paint::From, _, Fill::Gradient(c, ..)) => Some(c) == color,
                        (Paint::To, _, Fill::Gradient(_, c, _)) => Some(c) == color,
                        (Paint::Line, Some(Layer { kind: Kind::Shape(shape), .. }), _) => Some(shape.color) == color,
                        (Paint::Fill, Some(Layer { kind: Kind::Shape(shape), .. }), _) => if shape.filled { Some(shape.fill) == color } else { color.is_none() },
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
                        Naming::Title(index) => ("Title", match self.doc.layers.get(index).map(|layer| &layer.kind) {
                            Some(Kind::Picture(picture)) => picture.title.clone(),
                            _ => String::new(),
                        }),
                        Naming::Suffix(index) => ("", self.doc.exports.get(index).map_or(String::new(), |size| size.suffix.clone())),
                        Naming::Hex => ("Hex", format!("{:06X}", self.palette.and_then(|paint| self.paint_color(paint)).unwrap_or(0))),
                        _ => ("", String::new()),
                    };
                    let mut x = rect.x + 9;
                    if !label.is_empty() {
                        self.text.draw(canvas, x, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, label);
                        x += self.text.width(label, 11) + 8;
                    }
                    match typing {
                        Some(typed) => self.text.draw(canvas, x, cy - line_high / 2, px, TEXT_COLOR, &format!("{typed}|")),
                        None if now.is_empty() => self.text.draw(canvas, x, cy - line_high / 2, px, 0x777777, if label.is_empty() { "suffix" } else { "none" }),
                        None => self.text.draw(canvas, x, cy - line_high / 2, px, TEXT_COLOR, &now),
                    }
                }
                Control::Picker(part) => self.draw_picker(canvas, rect, part, hovered),
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
                        },
                        Control::Shadow(which) => matches!(&shown, Some(Layer { kind: Kind::Picture(picture), .. }) if picture.shadow == which),
                        Control::Action(Action::Halo) => matches!(&shown, Some(Layer { kind: Kind::Shape(shape), .. }) if shape.halo),
                        Control::Action(Action::Crop) => self.cropping.is_some(),
                        Control::Window(which) => matches!(&shown, Some(Layer { kind: Kind::Picture(picture), .. }) if frame::WINDOWS[usize::from(which).min(frame::WINDOWS.len() - 1)].0 == picture.window),
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
        // of the button under the pointer.
        if let Some((note, when)) = &self.note {
            if when.elapsed().as_secs_f32() < 3.0 {
                let note = note.clone();
                let wide = self.text.width(&note, px) + 24;
                let (x, y) = (layout.dock.x + (layout.dock.w - wide) / 2, layout.dock.y - 44);
                canvas.round_fill(x, y, wide, 32, 8, 0x000000, 210);
                self.text.draw(canvas, x + 12, y + (32 - line_high) / 2, px, TEXT_COLOR, &note);
            }
        }
        let says = match hover {
            Some(Control::Tool(tool)) => TOOLS.iter().find(|(known, ..)| *known == tool).map(|(_, name, key)| format!("{name}  {key}")),
            Some(Control::FrameTool) => Some("Frame  F".to_owned()),
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

/// What the window shows: the projects, or a file in the canvas.
enum Shown {
    Home(crate::home::Home),
    Studio(Box<Studio>),
}

/// A file of the canvas from its pages, in a window so large, kept at a
/// path; with pictures, a new frame round the first of them, on a
/// backdrop, and them on it.
fn opened(pages: Option<Vec<Page>>, pictures: Vec<(Image, String)>, path: PathBuf, text: Text, size: (i32, i32), options: &crate::options::Options) -> Studio {
    let pages = pages.unwrap_or_else(|| {
        let frame = match pictures.first() {
            Some((image, _)) => {
                let mut frame = Doc::new(image.width() + 160, image.height() + 160);
                frame.background = Fill::Backdrop(frame::named("aurora").unwrap_or(0));
                frame
            }
            None => Doc::new(1200, 675),
        };
        vec![Page { name: "Page 1".into(), frames: vec![frame] }]
    });
    let dir = options.places.first().cloned().unwrap_or_else(|| options.dir.clone());
    let mut studio = Studio::with_pages(pages, text, size.0, size.1, dir, options.filename.clone());
    studio.path = Some(path);
    let fresh = !pictures.is_empty();
    for (index, (image, name)) in pictures.into_iter().enumerate() {
        studio.add_picture(image, &name);
        // Pictures after the first a little down and to the right.
        if index > 0 {
            let at = studio.doc.layers.len() - 1;
            studio.doc.layers[at].translate(40.0 * index as f32, 40.0 * index as f32);
        }
    }
    (studio.undo, studio.edited) = (Vec::new(), false);
    if fresh {
        studio.save(false);
    }
    studio
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
    let (width, height) = ((display.width * 4 / 5).clamp(900, 2400), (display.height * 4 / 5).clamp(600, 1600));

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
        (false, true) => Shown::Home(crate::home::Home::new(text, width, height, options.dir.clone())),
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
    let mut saved = std::time::Instant::now();
    loop {
        // What has happened: a run of the pointer's movements told as
        // where it last was.
        let mut events = vec![conn.wait_for_event()?];
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
            match event {
                Event::MotionNotify(motion_event) => motion = Some((i32::from(motion_event.event_x), i32::from(motion_event.event_y), crate::x11::mods(motion_event.state.into()))),
                Event::ButtonPress(press) => {
                    if let Some(at) = motion.take() {
                        moved(&mut screen, at);
                    }
                    time = press.time;
                    moved(&mut screen, (i32::from(press.event_x), i32::from(press.event_y), crate::x11::mods(press.state.into())));
                    match &mut screen {
                        Shown::Home(home) => homeward = home.pressed(press.detail, press.time),
                        Shown::Studio(studio) => outcome = studio.pressed(press.detail, press.time),
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
                        Shown::Home(home) if down => homeward = home.key(key, crate::x11::mods(state)),
                        Shown::Home(_) => {}
                        Shown::Studio(studio) => outcome = studio.key(key, crate::x11::mods(state), down),
                    }
                }
                Event::Expose(_) => match &mut screen {
                    Shown::Home(home) => home.resize(size.0, size.1),
                    Shown::Studio(studio) => studio.dirty = true,
                },
                Event::ConfigureNotify(configured) if configured.window == window => {
                    let now = (i32::from(configured.width), i32::from(configured.height));
                    if now != size {
                        size = now;
                        frame = vec![0u32; (now.0 * now.1) as usize];
                        match &mut screen {
                            Shown::Home(home) => home.resize(now.0, now.1),
                            Shown::Studio(studio) => studio.resize(now.0, now.1),
                        }
                    }
                }
                Event::ClientMessage(message) if message.type_ == protocols && message.data.as_data32()[0] == delete => outcome = Outcome::Quit,
                _ => {}
            }
            if outcome != Outcome::Continue || homeward != crate::home::Outcome::Continue {
                break;
            }
        }
        if let Some(at) = motion {
            moved(&mut screen, at);
        }
        // Between the screens: a file opened or begun from the projects,
        // and the projects again from a file, which is kept first.
        screen = match (screen, homeward) {
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
                        let mut studio = opened(Some(vec![Page { name: "Page 1".into(), frames: vec![framed(image, "Pasted")] }]), Vec::new(), path, home.into_text(), size, options);
                        studio.save(false);
                        studio.fit();
                        Shown::Studio(Box::new(studio))
                    }
                    None => Shown::Home(home),
                }
            }
            (Shown::Home(_), crate::home::Outcome::Quit) => break,
            (Shown::Studio(mut studio), _) if outcome == Outcome::Open && studio.open_next.is_some() => {
                studio.save(false);
                let path = studio.open_next.take().unwrap_or_default();
                let pages = std::fs::read(&path).ok().and_then(|bytes| read_pages(&bytes));
                Shown::Studio(Box::new(opened(pages, Vec::new(), path, studio.into_text(), size, options)))
            }
            (Shown::Studio(mut studio), _) if outcome == Outcome::Home => {
                studio.save(false);
                let place = studio.path.as_ref().and_then(|path| path.parent()).map(Path::to_path_buf);
                let mut home = crate::home::Home::new(studio.into_text(), size.0, size.1, options.dir.clone());
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
                    studio.save(false);
                    break;
                }
                Outcome::Paste => {
                    let picture = selection(conn, window, atom("CLIPBOARD")?, atom("image/png")?, atom("HOTSHOT_CANVAS")?, atom("INCR")?, time);
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
                Shown::Studio(studio) => studio.draw(&mut frame),
            }
            crate::x11::put(conn, window, gc, false, size.0, size.1, &frame)?;
        }
        conn.flush()?;
    }
    Ok(())
}
/// A frame just the size of a picture, holding it.
fn framed(image: Image, name: &str) -> Doc {
    let (w, h) = (image.width(), image.height());
    let mut frame = Doc::new(w, h);
    frame.background = Fill::None;
    frame.layers.push(Layer::picture(Picture::new(Arc::new(image), name.to_owned(), (w as f32 / 2.0, h as f32 / 2.0))));
    frame
}

/// The file a copied path or URI names, from the first line of it that
/// names one that is there.
fn pasted_path(words: &str) -> Option<PathBuf> {
    words.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).find_map(|line| {
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
        // Its fill: a plus for one, then the colors its fill opens.
        let plus = studio.control_rect(Control::Action(Action::FillOn)).expect("a fill can be added").center();
        studio.pointer_moved(plus.0, plus.1, Mods::default());
        studio.pressed(1, 1500);
        studio.released();
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
        // With nothing picked out, the panel is the artboard's.
        studio.key(Key::Escape, Mods::default(), true);
        // Its size from the list of them, as Figma has it.
        assert!(studio.layout().controls.iter().any(|(_, control)| *control == Control::Menu(MenuKind::Sizes)));
        studio.press_control(Control::Menu(MenuKind::Sizes), false);
        assert!(studio.menu.is_some());
        studio.press_menu(0, 8);
        assert_eq!((studio.doc.width, studio.doc.height, studio.menu.is_none()), (1920, 1080, true));
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
        // A page more, then undone, and done again.
        studio.press_control(Control::NewPage, false);
        studio.commit_name();
        assert_eq!((studio.pages.len(), studio.page), (2, 1));
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
    fn a_picture_keeps_its_shadow_numbers_adjustments_and_window() {
        let mut text = Text::load();
        let mut doc = Doc::new(300, 200);
        let mut picture = Picture::new(Arc::new(checker(200, 120)), "pic".into(), (150.0, 100.0));
        (picture.shadow, picture.drop) = (CUSTOM_SHADOW, frame::Layer { x: 4, y: 20, blur: 30, spread: -2, color: 0x80102030 });
        (picture.adjust, picture.window, picture.title) = ([0.5, -0.25, 1.0], frame::Window::Mac, "main.rs".into());
        doc.layers.push(Layer::picture(picture));
        doc.exports = vec![ExportSize { times: 1.0, suffix: String::new(), format: crate::output::Format::Png }, ExportSize { times: 3.0, suffix: "@3x".into(), format: crate::output::Format::Webp }];
        let back = Doc::read(&doc.write()).expect("read back");
        let Kind::Picture(read) = &back.layers[0].kind else { panic!() };
        assert_eq!((read.shadow, read.drop, read.adjust, read.window, read.title.as_str()), (CUSTOM_SHADOW, frame::Layer { x: 4, y: 20, blur: 30, spread: -2, color: 0x80102030 }, [0.5, -0.25, 1.0], frame::Window::Mac, "main.rs"));
        assert_eq!(back.exports, doc.exports);
        // The window's bar is over the top of it, and lighter is lighter.
        let (_, _, out) = back.render(1.0, &mut text);
        let top = out[(40 + 4) * 300 + 150];
        assert!(top & 0xFF_FFFF == 0xE8E9EC || top & 0xFF_FFFF == 0x2B2D31, "{top:08x}");
        assert!(adjust(0xFF808080, [0.5, 0.0, 0.0]) & 0xFF > 0x80 && adjust(0xFFFF0000, [0.0, 0.0, -1.0]) & 0xFF == (adjust(0xFFFF0000, [0.0, 0.0, -1.0]) >> 8) & 0xFF);
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
