//! Text, in the font hotshot carries, and the icons, which are letters of
//! a font of their own.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

use crate::canvas::Canvas;

/// The icons, by the letter the icon font has each at. They are Solar's,
/// as jog's are, with a few plain shapes drawn to match; see
/// `tools/icon_font.py`.
pub mod icons {
    pub const SELECT: &str = "\u{e000}";
    pub const ARROW: &str = "\u{e001}";
    pub const LINE: &str = "\u{e002}";
    pub const RECT: &str = "\u{e003}";
    pub const ELLIPSE: &str = "\u{e004}";
    pub const PEN: &str = "\u{e005}";
    pub const MARKER: &str = "\u{e006}";
    pub const TEXT: &str = "\u{e007}";
    pub const BLUR: &str = "\u{e008}";
    pub const COUNTER: &str = "\u{e009}";
    pub const PIPETTE: &str = "\u{e00a}";
    pub const UNDO: &str = "\u{e00b}";
    pub const REDO: &str = "\u{e00c}";
    pub const COPY: &str = "\u{e00d}";
    pub const SAVE: &str = "\u{e00e}";
    pub const CLOSE: &str = "\u{e00f}";
    pub const SPOTLIGHT: &str = "\u{e016}";
    pub const ZOOM: &str = "\u{e017}";
    pub const BADGE: &str = "\u{e018}";
    pub const FRAME: &str = "\u{e019}";
    pub const STICKER: &str = "\u{e01a}";
    pub const DOWN: &str = "\u{e01b}";
    pub const DICE: &str = "\u{e01c}";
    pub const POINTER: &str = "\u{e01d}";
    pub const PIN: &str = "\u{e012}";
    pub const RECORD: &str = "\u{e01e}";
    pub const CHECK: &str = "\u{e010}";
    pub const TRASH: &str = "\u{e011}";
    pub const FORWARD: &str = "\u{e01f}";
    pub const BACKWARD: &str = "\u{e020}";
    pub const DUPLICATE: &str = "\u{e023}";
    pub const FOLDER: &str = "\u{e014}";
    pub const ALIGN_LEFT: &str = "\u{e049}";
    pub const ALIGN_HCENTER: &str = "\u{e04a}";
    pub const ALIGN_RIGHT: &str = "\u{e04b}";
    pub const ALIGN_TOP: &str = "\u{e04c}";
    pub const ALIGN_VCENTER: &str = "\u{e04d}";
    pub const ALIGN_BOTTOM: &str = "\u{e04e}";
    pub const DIST_H: &str = "\u{e04f}";
    pub const DIST_V: &str = "\u{e050}";
    pub const SWAP: &str = "\u{e051}";
    pub const MINUS: &str = "\u{e053}";
    pub const HAND: &str = "\u{e055}";
    pub const STICKY: &str = "\u{e056}";
    pub const DIAMOND: &str = "\u{e057}";
    pub const RATIO: &str = "\u{e041}";
    pub const FLIP: &str = "\u{e045}";
    pub const LINK: &str = "\u{e042}";
    pub const CROP: &str = "\u{e044}";
    pub const SEARCH: &str = "\u{e03e}";
    pub const CAMERA: &str = "\u{e030}";
    pub const HASH: &str = "\u{e031}";
    pub const PLUS: &str = "\u{e032}";
    pub const EYE: &str = "\u{e033}";
    pub const EYE_OFF: &str = "\u{e034}";
    pub const LOCK: &str = "\u{e035}";
    pub const FILE: &str = "\u{e036}";
    pub const PICTURE: &str = "\u{e037}";
    pub const FOLDER_OPEN: &str = "\u{e038}";
    pub const HOME: &str = "\u{e039}";
    pub const CLOCK: &str = "\u{e03a}";
    pub const DRAFTS: &str = "\u{e03b}";
    pub const EXPORT: &str = "\u{e03d}";
    pub const ROTATE: &str = "\u{e03f}";
    pub const RADIUS: &str = "\u{e040}";
    pub const RIGHT: &str = "\u{e043}";
}

const FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
];

/// Which font the icons are, where the others are counted from 0.
const ICONS: usize = usize::MAX;

/// The faces text can be typed in, by what each is called. The first is
/// the one the interface itself is in.
pub const FACES: [&str; 5] = ["Geist", "Instrument Serif", "Architects Daughter", "Fira Code", "Anton"];

/// The files of the faces after the first. The last is the heavy,
/// narrow one captions are set in, as Impact used to be.
const FACE_FILES: [&[u8]; 4] = [
    include_bytes!("../assets/InstrumentSerif-Regular.ttf"),
    include_bytes!("../assets/ArchitectsDaughter-Regular.ttf"),
    include_bytes!("../assets/FiraCode-Regular.ttf"),
    include_bytes!("../assets/Anton-Regular.ttf"),
];

/// The heavy and the slanted cuts there are of the faces: which face,
/// whether heavy, whether slanted, and the file. A face with no cut of
/// its own for what is asked has it made of its letters instead: laid
/// down twice for heavy, each row moved over for slanted.
const CUTS: [(usize, bool, bool, &[u8]); 5] = [
    (0, true, false, include_bytes!("../assets/Geist-Bold.ttf")),
    (0, false, true, include_bytes!("../assets/Geist-Italic.ttf")),
    (0, true, true, include_bytes!("../assets/Geist-BoldItalic.ttf")),
    (1, false, true, include_bytes!("../assets/InstrumentSerif-Italic.ttf")),
    (3, true, false, include_bytes!("../assets/FiraCode-Bold.ttf")),
];

/// Which font the first of the cuts is, the rest counted back from it.
const CUT: usize = usize::MAX - 32;

/// Text rendering with the font hotshot carries. Loading it takes a few
/// milliseconds, so it is done on a thread of its own while the screen
/// is being read.
pub struct Text {
    font: Option<fontdue::Font>,
    icons: Option<fontdue::Font>,
    /// The faces after the first, each read the first time something is
    /// to be drawn in it: most pictures have no text on them at all.
    faces: [Option<fontdue::Font>; 4],
    /// Their heavy and slanted cuts, each read the same way; and
    /// whether it has been tried.
    cuts: [Option<fontdue::Font>; CUTS.len()],
    cuts_read: [bool; CUTS.len()],
    /// Fonts for the letters the main one lacks, Korean say, each found
    /// through fontconfig the first time such a letter is typed.
    others: Vec<fontdue::Font>,
    /// Letters a font has been looked for, so each is only asked about once.
    sought: HashSet<char>,
    glyphs: HashMap<(usize, char, u32), (fontdue::Metrics, Vec<u8>)>,
    /// The file of the system's emoji, read the first time one is to be
    /// drawn; and whether it has been looked for.
    emoji: Option<Vec<u8>>,
    emoji_sought: bool,
    /// The emoji drawn so far, each at a size: how wide and tall, and
    /// its pixels, each with how opaque it is in its top byte.
    pictures: HashMap<(char, u32), (i32, i32, Vec<u32>)>,
    /// How what is drawn next is to look, apart from its face and size.
    pub look: Look,
}

/// How letters are drawn, apart from their face and their size: heavy,
/// slanted, or faint. Where a face has a heavy or a slanted cut of its
/// own, that is what is drawn; where it has not, heavy is the letter
/// laid down again beside itself, and slanted is each row of it moved
/// over, which is how a typewriter would have to.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Look {
    /// How many pixels further right each letter is laid down again.
    pub heavy: i32,
    /// How far right a row is moved for each pixel it is above the line.
    pub slant: f32,
    /// How opaque, out of 256.
    pub opacity: u32,
}

impl Default for Look {
    fn default() -> Self {
        Look { heavy: 0, slant: 0.0, opacity: 256 }
    }
}

/// Which font an emoji is said to be in: none of the others, since it
/// is no outline to be filled but a small picture in colors.
const EMOJI: usize = usize::MAX - 16;
/// The largest a letter is kept once made, and the largest an emoji is
/// drawn: text on the canvas zoomed far in would otherwise be made, and
/// kept, thousands of pixels high.
const BIG_LETTER: u32 = 480;
const BIG_EMOJI: u32 = 1024;

/// Where the system's emoji usually are.
const EMOJI_PATHS: &[&str] = &["/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf", "/usr/share/fonts/noto/NotoColorEmoji.ttf", "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf", "/usr/share/fonts/noto-emoji/NotoColorEmoji.ttf"];

/// Twemoji, as Discord and X have them, in Mozilla's font of them, whose
/// emoji are outlines filled with flat colors, one over another: drawn
/// at any size, sharp. jog's.
const TWEMOJI: &[u8] = include_bytes!("../assets/Twemoji.Mozilla.ttf");

/// The layers of an emoji as the font has them: which outline, in which
/// color, the first lowest.
#[derive(Default)]
struct Layers {
    outline: Option<ttf_parser::GlyphId>,
    layers: Vec<(ttf_parser::GlyphId, ttf_parser::RgbaColor)>,
}

impl<'a> ttf_parser::colr::Painter<'a> for Layers {
    fn outline_glyph(&mut self, glyph: ttf_parser::GlyphId) {
        self.outline = Some(glyph);
    }

    fn paint(&mut self, paint: ttf_parser::colr::Paint<'a>) {
        // The font is of flat colors only.
        if let (Some(glyph), ttf_parser::colr::Paint::Solid(color)) = (self.outline, paint) {
            self.layers.push((glyph, color));
        }
    }

    fn push_clip(&mut self) {}
    fn push_clip_box(&mut self, _: ttf_parser::colr::ClipBox) {}
    fn pop_clip(&mut self) {}
    fn push_layer(&mut self, _: ttf_parser::colr::CompositeMode) {}
    fn pop_layer(&mut self) {}
    fn push_transform(&mut self, _: ttf_parser::Transform) {}
    fn pop_transform(&mut self) {}
}

/// An outline of the font laid onto a rasterizer: its points taken from
/// the font's units, up being up, to the square's pixels, down being down.
struct Trace<'a> {
    fill: &'a mut ab_glyph_rasterizer::Rasterizer,
    at: (f32, f32),
    start: (f32, f32),
    fit: f32,
    left: f32,
    base: f32,
}

impl Trace<'_> {
    fn point(&self, x: f32, y: f32) -> ab_glyph_rasterizer::Point {
        ab_glyph_rasterizer::point(self.left + x * self.fit, self.base - y * self.fit)
    }
}

impl ttf_parser::OutlineBuilder for Trace<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        (self.at, self.start) = ((x, y), (x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (from, to) = (self.point(self.at.0, self.at.1), self.point(x, y));
        self.fill.draw_line(from, to);
        self.at = (x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (from, via, to) = (self.point(self.at.0, self.at.1), self.point(x1, y1), self.point(x, y));
        self.fill.draw_quad(from, via, to);
        self.at = (x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (from, a, b, to) = (self.point(self.at.0, self.at.1), self.point(x1, y1), self.point(x2, y2), self.point(x, y));
        self.fill.draw_cubic(from, a, b, to);
        self.at = (x, y);
    }

    fn close(&mut self) {
        let (x, y) = self.start;
        self.line_to(x, y);
    }
}

/// An emoji `side` pixels square from Twemoji, each layer's outline filled
/// at that size and laid over the ones before; its alpha in the top byte,
/// its colors as they are. `None` for one the font has no colors for.
fn twemoji(c: char, side: u32) -> Option<(i32, i32, Vec<u32>)> {
    let face = ttf_parser::Face::parse(TWEMOJI, 0).ok()?;
    let glyph = face.glyph_index(c)?;
    let mut painted = Layers::default();
    face.paint_color_glyph(glyph, 0, ttf_parser::RgbaColor::new(0, 0, 0, 255), &mut painted)?;
    if painted.layers.is_empty() {
        return None;
    }
    // All the layers together, in the font's units: the square is fitted
    // to that, so that every emoji fills it the same.
    let mut around = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for (layer, _) in &painted.layers {
        if let Some(rect) = face.glyph_bounding_box(*layer) {
            around = [around[0].min(rect.x_min.into()), around[1].min(rect.y_min.into()), around[2].max(rect.x_max.into()), around[3].max(rect.y_max.into())];
        }
    }
    let (wide, high) = ((around[2] - around[0]).max(1) as f32, (around[3] - around[1]).max(1) as f32);
    let fit = side as f32 / wide.max(high);
    // Where the font's origin falls in the square, the emoji in the middle.
    let left = (side as f32 - wide * fit) / 2.0 - around[0] as f32 * fit;
    let base = (side as f32 - high * fit) / 2.0 + around[3] as f32 * fit;
    let side = side as usize;
    // Colors kept weighed by how much of each pixel they cover, as layers
    // are laid one over another; weighed back out at the end.
    let mut sum = vec![[0f32; 4]; side * side];
    for (layer, color) in &painted.layers {
        let mut fill = ab_glyph_rasterizer::Rasterizer::new(side, side);
        face.outline_glyph(*layer, &mut Trace { fill: &mut fill, at: (0.0, 0.0), start: (0.0, 0.0), fit, left, base })?;
        let opacity = f32::from(color.alpha) / 255.0;
        fill.for_each_pixel(|at, cover| {
            let a = cover.min(1.0) * opacity;
            if a <= 0.0 {
                return;
            }
            let pixel = &mut sum[at];
            for (channel, value) in [color.red, color.green, color.blue].into_iter().enumerate() {
                pixel[channel] = pixel[channel] * (1.0 - a) + f32::from(value) * a;
            }
            pixel[3] = pixel[3] * (1.0 - a) + a;
        });
    }
    let pixels = sum
        .iter()
        .map(|[r, g, b, a]| {
            if *a <= 0.0 {
                return 0;
            }
            let channel = |v: f32| (v / a).round().clamp(0.0, 255.0) as u32;
            ((a * 255.0).round() as u32) << 24 | channel(*r) << 16 | channel(*g) << 8 | channel(*b)
        })
        .collect();
    Some((side as i32, side as i32, pixels))
}

impl Text {
    pub fn load() -> Self {
        // The interface font is carried along, so that hotshot looks the
        // same everywhere; `HOTSHOT_FONT` names another, and the
        // system's are what is left if neither can be used.
        let load = |bytes: Vec<u8>| fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok();
        let chosen = std::env::var_os("HOTSHOT_FONT").and_then(|path| load(std::fs::read(path).ok()?));
        let font = chosen
            .or_else(|| load(include_bytes!("../assets/Geist-Regular.ttf").to_vec()))
            .or_else(|| FONT_PATHS.iter().find_map(|path| load(std::fs::read(path).ok()?)));
        if font.is_none() {
            eprintln!("hotshot: no usable font found, text will not be drawn (set HOTSHOT_FONT to a .ttf file)");
        }
        let icons = load(include_bytes!("../assets/icons.ttf").to_vec());
        Text { font, icons, faces: [None, None, None, None], cuts: Default::default(), cuts_read: [false; CUTS.len()], others: Vec::new(), sought: HashSet::new(), glyphs: HashMap::new(), emoji: None, emoji_sought: false, pictures: HashMap::new(), look: Look::default() }
    }

    /// An emoji as a picture `px` high: from Twemoji, sharp at any size,
    /// or for one it lacks from the system's emoji, which keeps each as a
    /// small PNG. Nothing where there are none, or none of that one.
    fn picture(&mut self, c: char, px: u32) -> Option<&(i32, i32, Vec<u32>)> {
        if !self.pictures.contains_key(&(c, px)) {
            if !std::mem::replace(&mut self.emoji_sought, true) {
                self.emoji = EMOJI_PATHS.iter().find_map(|path| std::fs::read(path).ok());
            }
            let made = twemoji(c, (px as f32 * 1.2).round().max(1.0) as u32).or_else(|| {
                let face = ttf_parser::Face::parse(self.emoji.as_deref()?, 0).ok()?;
                let image = face.glyph_raster_image(face.glyph_index(c)?, u16::MAX)?;
                if image.format != ttf_parser::RasterImageFormat::PNG {
                    return None;
                }
                let mut decoder = png::Decoder::new(std::io::Cursor::new(image.data));
                decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
                let mut reader = decoder.read_info().ok()?;
                let mut bytes = vec![0; reader.output_buffer_size()?];
                let info = reader.next_frame(&mut bytes).ok()?;
                if info.color_type != png::ColorType::Rgba {
                    return None;
                }
                // Made smaller by taking, for each pixel wanted, the
                // average of those it is made from.
                let (wide, high) = (info.width as i32, info.height as i32);
                let side = (px as f32 * 1.2).round().max(1.0) as i32;
                let (w, h) = ((wide * side / high.max(1)).max(1), side);
                let mut pixels = Vec::with_capacity((w * h) as usize);
                for y in 0..h {
                    for x in 0..w {
                        let (x0, x1, y0, y1) = (x * wide / w, ((x + 1) * wide / w).max(x * wide / w + 1), y * high / h, ((y + 1) * high / h).max(y * high / h + 1));
                        let (mut sum, mut count) = ([0u32; 4], 0u32);
                        for sy in y0..y1.min(high) {
                            for sx in x0..x1.min(wide) {
                                let at = ((sy * wide + sx) * 4) as usize;
                                let alpha = u32::from(bytes[at + 3]);
                                (0..3).for_each(|channel| sum[channel] += u32::from(bytes[at + channel]) * alpha);
                                sum[3] += alpha;
                                count += 1;
                            }
                        }
                        let alpha = sum[3] / count.max(1);
                        let color = |channel: usize| sum[channel] / sum[3].max(1);
                        pixels.push(alpha << 24 | color(0) << 16 | color(1) << 8 | color(2));
                    }
                }
                Some((w, h, pixels))
            });
            self.pictures.insert((c, px), made.unwrap_or((0, 0, Vec::new())));
        }
        self.pictures.get(&(c, px)).filter(|picture| picture.0 > 0)
    }

    pub fn line_height(px: u32) -> i32 {
        (px * 4 / 3) as i32
    }

    /// Which font has letter `c`, for text in a face: 0 for the main
    /// one, otherwise one of the others, counted from 1, or the face
    /// itself, counted back from the icons. Looks for a font that has
    /// the letter if none does.
    fn font_with(&mut self, c: char, face: usize) -> usize {
        // The icons have a font, and a part of Unicode, to themselves.
        if ('\u{e000}'..='\u{f8ff}').contains(&c) {
            return ICONS;
        }
        let has = |font: &fontdue::Font| font.lookup_glyph_index(c) != 0;
        // Heavy or slanted, it is the face's own cut for that, where
        // there is one and it has the letter.
        if let Some(cut) = self.cut(face) {
            if !std::mem::replace(&mut self.cuts_read[cut], true) {
                self.cuts[cut] = fontdue::Font::from_bytes(CUTS[cut].3, fontdue::FontSettings::default()).ok();
            }
            if self.cuts[cut].as_ref().is_some_and(has) {
                return CUT - cut;
            }
        }
        // A face has the letters it has; the rest are the main font's.
        if (1..=FACE_FILES.len()).contains(&face) {
            let slot = &mut self.faces[face - 1];
            if slot.is_none() {
                *slot = fontdue::Font::from_bytes(FACE_FILES[face - 1], fontdue::FontSettings::default()).ok();
            }
            if slot.as_ref().is_some_and(has) {
                return ICONS - face;
            }
        }
        if self.font.as_ref().is_none_or(has) || c.is_control() {
            return 0;
        }
        // What says how the emoji before it is to be drawn, or joins
        // two of them, is nothing to be drawn itself.
        if matches!(c, '\u{fe0f}' | '\u{200d}') || self.picture(c, 16).is_some() {
            return EMOJI;
        }
        if let Some(at) = self.others.iter().position(has) {
            return at + 1;
        }
        if self.sought.insert(c) {
            if let Some(font) = font_for(c) {
                self.others.push(font);
                return self.others.len();
            }
        }
        0
    }

    /// The same of a letter with the one after it: one that a font of
    /// letters has too, ☺ or ❤, is an emoji when what comes after it says
    /// it is to be drawn as one.
    fn font_before(&mut self, c: char, next: Option<char>, face: usize) -> usize {
        if next == Some('\u{fe0f}') && self.picture(c, 16).is_some() {
            return EMOJI;
        }
        self.font_with(c, face)
    }

    /// The cut of a face that does the most of what the look in hand
    /// asks, heavy and slanted, and nothing it does not ask.
    fn cut(&self, face: usize) -> Option<usize> {
        let (heavy, slanted) = (self.look.heavy > 0, self.look.slant > 0.0);
        let does = |&(_, cut): &(usize, &(usize, bool, bool, &[u8]))| cut.0 == face && (cut.1 || cut.2) && (!cut.1 || heavy) && (!cut.2 || slanted);
        CUTS.iter().enumerate().filter(does).max_by_key(|(_, cut)| u8::from(cut.1) + u8::from(cut.2)).map(|(at, _)| at)
    }

    /// What is left to be made of the letters themselves, of the look
    /// in hand: how heavy and how slanted, less what the face's own cut
    /// is already.
    fn made(&self, face: usize) -> (i32, f32) {
        let (heavy, slant) = (self.look.heavy.max(0), self.look.slant);
        match self.cut(face).filter(|&cut| self.cuts[cut].is_some() || !self.cuts_read[cut]) {
            Some(cut) => (if CUTS[cut].1 { 0 } else { heavy }, if CUTS[cut].2 { 0.0 } else { slant }),
            None => (heavy, slant),
        }
    }

    fn font(&self, which: usize) -> Option<&fontdue::Font> {
        match which {
            0 => self.font.as_ref(),
            ICONS => self.icons.as_ref(),
            cut if (CUT + 1 - CUTS.len()..=CUT).contains(&cut) => self.cuts[CUT - cut].as_ref(),
            face if face >= ICONS - FACE_FILES.len() => self.faces[ICONS - face - 1].as_ref(),
            other => self.others.get(other - 1),
        }
    }

    /// The size a letter is drawn at: icons a little larger than the text
    /// they sit in, which they would otherwise look small beside.
    fn size(which: usize, px: u32) -> f32 {
        if which == ICONS { px as f32 * 1.15 } else { px as f32 }
    }

    pub fn width(&mut self, text: &str, px: u32) -> i32 {
        self.width_in(0, text, px)
    }

    /// How wide text is in one of the faces.
    pub fn width_in(&mut self, face: usize, text: &str, px: u32) -> i32 {
        let mut width = 0.0;
        let heavy = self.made(face).0;
        let mut letters = text.chars().peekable();
        while let Some(c) = letters.next() {
            let which = self.font_before(c, letters.peek().copied(), face);
            if which == EMOJI {
                width += self.picture(c, px.min(BIG_EMOJI)).map_or(0.0, |picture| picture.0 as f32 * 1.08);
                continue;
            }
            width += self.font(which).map_or(0.0, |font| font.metrics(c, Self::size(which, px)).advance_width) + heavy as f32;
        }
        width.ceil() as i32
    }

    /// Draws one line of text whose line box has its top-left at `(x, y)`.
    pub fn draw(&mut self, canvas: &mut Canvas, x: i32, y: i32, px: u32, color: u32, text: &str) {
        self.draw_in(0, canvas, x, y, px, color, text);
    }

    /// Draws one line of text in one of the faces.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_in(&mut self, face: usize, canvas: &mut Canvas, x: i32, y: i32, px: u32, color: u32, text: &str) {
        // The line stands where the face it is in would have it stand.
        let standing = self.font_with('x', face);
        let Some(font) = self.font(standing).or(self.font.as_ref()) else { return };
        let (ascent, descent) = font
            .horizontal_line_metrics(px as f32)
            .map_or((px as f32 * 0.8, px as f32 * -0.2), |m| (m.ascent, m.descent));
        let baseline = y + ((Self::line_height(px) as f32 + ascent + descent) / 2.0).round() as i32;

        let mut pen = x as f32;
        let (heavy, slant) = self.made(face);
        let mut letters = text.chars().peekable();
        while let Some(c) = letters.next() {
            // An emoji is laid down as the picture it is, in the middle
            // of the line, whatever color the words are in.
            let which = self.font_before(c, letters.peek().copied(), face);
            if which == EMOJI {
                if let Some((wide, high, pixels)) = self.picture(c, px.min(BIG_EMOJI)) {
                    let (left, top) = (pen.round() as i32, y + (Self::line_height(px) - high) / 2);
                    for (at, pixel) in pixels.iter().enumerate() {
                        canvas.blend(left + at as i32 % wide, top + at as i32 / wide, pixel & 0xFF_FFFF, (pixel >> 24) as u8);
                    }
                    pen += *wide as f32 * 1.08;
                }
                continue;
            }
            // A letter wholly off what can be drawn on is passed over, not
            // made: text on the canvas zoomed far in is drawn thousands of
            // pixels high, a letter or two of it in the window.
            let size = Self::size(which, px);
            let Some(outline) = self.font(which).map(|font| font.metrics(c, size)) else { continue };
            let room = crate::canvas::Rect::new(pen.floor() as i32 - 2 - (size * slant.abs()) as i32, y - Self::line_height(px), outline.advance_width.ceil() as i32 + outline.width as i32 + 4 + heavy + 2 * (size * slant.abs()) as i32, 3 * Self::line_height(px));
            if room.intersect(canvas.clip()).is_empty() {
                pen += outline.advance_width + heavy as f32;
                continue;
            }
            // Letters that large are made each time rather than kept, or
            // each step of a zoom would keep more of them.
            let made;
            let (metrics, coverage) = match px > BIG_LETTER {
                true => {
                    let Some(font) = self.font(which) else { continue };
                    made = font.rasterize(c, size);
                    (&made.0, &made.1)
                }
                false => {
                    if !self.glyphs.contains_key(&(which, c, px)) {
                        let Some(font) = self.font(which) else { continue };
                        let glyph = font.rasterize(c, size);
                        self.glyphs.insert((which, c, px), glyph);
                    }
                    let (metrics, coverage) = &self.glyphs[&(which, c, px)];
                    (metrics, coverage)
                }
            };
            let left = pen.round() as i32 + metrics.xmin;
            // An icon sits in the middle of the line, whatever its font
            // takes for a baseline; letters stand on the text's.
            let icon = ('\u{e000}'..='\u{f8ff}').contains(&c);
            let top = match icon {
                true => y + (Self::line_height(px) - metrics.height as i32) / 2,
                false => baseline - metrics.ymin - metrics.height as i32,
            };
            // The look in hand: each row of a slanted letter a little
            // further right the higher it is; a heavy one laid down
            // again beside itself; a faint one, less of each pixel.
            let faint = self.look.opacity.min(256);
            for (row, line) in coverage.chunks_exact(metrics.width.max(1)).enumerate() {
                let lean = ((baseline - top - row as i32) as f32 * slant).round() as i32;
                for (column, &amount) in line.iter().enumerate() {
                    if amount > 0 {
                        let amount = ((u32::from(amount) * faint) >> 8) as u8;
                        for beside in 0..=heavy {
                            canvas.blend(left + column as i32 + lean + beside, top + row as i32, color, amount);
                        }
                    }
                }
            }
            pen += metrics.advance_width + heavy as f32;
        }
    }

    /// Draws one line with its middle at `(cx, cy)`.
    pub fn draw_centered(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, px: u32, color: u32, text: &str) {
        let width = self.width(text, px);
        self.draw(canvas, cx - width / 2, cy - Self::line_height(px) / 2, px, color, text);
    }
}

/// A font that has letter `c`, as fontconfig picks one. The file may hold
/// several fonts, in which case fontconfig says which.
fn font_for(c: char) -> Option<fontdue::Font> {
    let pattern = format!("sans:charset={:x}", c as u32);
    let output = Command::new("fc-match").args(["-f", "%{file}\n%{index}", &pattern]).output().ok()?;
    let answer = String::from_utf8(output.stdout).ok()?;
    let (file, index) = answer.split_once('\n')?;
    let settings = fontdue::FontSettings { collection_index: index.trim().parse().unwrap_or(0), ..Default::default() };
    let font = fontdue::Font::from_bytes(std::fs::read(PathBuf::from(file)).ok()?, settings).ok()?;
    (font.lookup_glyph_index(c) != 0).then_some(font)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twemoji_are_drawn_in_their_colors_and_sharp_when_large() {
        // A red heart: red in the middle, clear in the corners.
        let (w, h, pixels) = twemoji('\u{2764}', 64).expect("drawn");
        assert_eq!((w, h), (64, 64));
        let middle = pixels[32 * 64 + 32];
        assert_eq!(middle >> 24, 255);
        assert!((middle >> 16 & 255) > 180 && (middle >> 8 & 255) < 90);
        assert_eq!(pixels[0] >> 24, 0);
        // Large, its edge is a pixel or two of part cover, not a blur.
        let (_, _, big) = twemoji('\u{1f600}', 512).expect("drawn");
        let partly = big.iter().filter(|p| (1..255).contains(&(**p >> 24))).count();
        assert!(partly < 512 * 8, "{partly}");
        // Every emoji there is to choose has its picture.
        let list = include_str!("../assets/emoji.txt").lines().filter(|line| !line.starts_with('#'));
        let missing: Vec<&str> = list.filter(|line| twemoji(line.chars().next().unwrap_or(' '), 16).is_none()).collect();
        assert!(missing.is_empty(), "{missing:?}");
    }
}
