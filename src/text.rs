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
    pub const FILLED: &str = "\u{e015}";
    pub const SPOTLIGHT: &str = "\u{e016}";
    pub const ZOOM: &str = "\u{e017}";
    pub const BADGE: &str = "\u{e018}";
    pub const FRAME: &str = "\u{e019}";
    pub const STICKER: &str = "\u{e01a}";
    pub const DOWN: &str = "\u{e01b}";
    pub const DICE: &str = "\u{e01c}";
    pub const POINTER: &str = "\u{e01d}";
    pub const CHECK: &str = "\u{e010}";
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

/// Where the system's emoji usually are.
const EMOJI_PATHS: &[&str] = &["/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf", "/usr/share/fonts/noto/NotoColorEmoji.ttf", "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf", "/usr/share/fonts/noto-emoji/NotoColorEmoji.ttf"];

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

    /// An emoji as a picture `px` high, from the system's emoji, which
    /// keeps each as a small PNG. Nothing where there are none, or none
    /// of that one.
    fn picture(&mut self, c: char, px: u32) -> Option<&(i32, i32, Vec<u32>)> {
        if !self.pictures.contains_key(&(c, px)) {
            if !std::mem::replace(&mut self.emoji_sought, true) {
                self.emoji = EMOJI_PATHS.iter().find_map(|path| std::fs::read(path).ok());
            }
            let made = (|| {
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
            })();
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
        for c in text.chars() {
            let which = self.font_with(c, face);
            if which == EMOJI {
                width += self.picture(c, px).map_or(0.0, |picture| picture.0 as f32 * 1.08);
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
        for c in text.chars() {
            // An emoji is laid down as the picture it is, in the middle
            // of the line, whatever color the words are in.
            let which = self.font_with(c, face);
            if which == EMOJI {
                if let Some((wide, high, pixels)) = self.picture(c, px) {
                    let (left, top) = (pen.round() as i32, y + (Self::line_height(px) - high) / 2);
                    for (at, pixel) in pixels.iter().enumerate() {
                        canvas.blend(left + at as i32 % wide, top + at as i32 / wide, pixel & 0xFF_FFFF, (pixel >> 24) as u8);
                    }
                    pen += *wide as f32 * 1.08;
                }
                continue;
            }
            if !self.glyphs.contains_key(&(which, c, px)) {
                let Some(font) = self.font(which) else { continue };
                let glyph = font.rasterize(c, Self::size(which, px));
                self.glyphs.insert((which, c, px), glyph);
            }
            let (metrics, coverage) = &self.glyphs[&(which, c, px)];
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
