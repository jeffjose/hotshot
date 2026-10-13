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
pub const FACES: [&str; 4] = ["Geist", "Instrument Serif", "Architects Daughter", "Fira Code"];

/// The files of the faces after the first.
const FACE_FILES: [&[u8]; 3] = [
    include_bytes!("../assets/InstrumentSerif-Regular.ttf"),
    include_bytes!("../assets/ArchitectsDaughter-Regular.ttf"),
    include_bytes!("../assets/FiraCode-Regular.ttf"),
];

/// Text rendering with the font hotshot carries. Loading it takes a few
/// milliseconds, so it is done on a thread of its own while the screen
/// is being read.
pub struct Text {
    font: Option<fontdue::Font>,
    icons: Option<fontdue::Font>,
    /// The faces after the first, each read the first time something is
    /// to be drawn in it: most pictures have no text on them at all.
    faces: [Option<fontdue::Font>; 3],
    /// Fonts for the letters the main one lacks, Korean say, each found
    /// through fontconfig the first time such a letter is typed.
    others: Vec<fontdue::Font>,
    /// Letters a font has been looked for, so each is only asked about once.
    sought: HashSet<char>,
    glyphs: HashMap<(usize, char, u32), (fontdue::Metrics, Vec<u8>)>,
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
        Text { font, icons, faces: [None, None, None], others: Vec::new(), sought: HashSet::new(), glyphs: HashMap::new() }
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

    fn font(&self, which: usize) -> Option<&fontdue::Font> {
        match which {
            0 => self.font.as_ref(),
            ICONS => self.icons.as_ref(),
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
        for c in text.chars() {
            let which = self.font_with(c, face);
            width += self.font(which).map_or(0.0, |font| font.metrics(c, Self::size(which, px)).advance_width);
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
        for c in text.chars() {
            if !self.glyphs.contains_key(&(face, c, px)) {
                let which = self.font_with(c, face);
                let Some(font) = self.font(which) else { continue };
                let glyph = font.rasterize(c, Self::size(which, px));
                self.glyphs.insert((face, c, px), glyph);
            }
            let (metrics, coverage) = &self.glyphs[&(face, c, px)];
            let left = pen.round() as i32 + metrics.xmin;
            // An icon sits in the middle of the line, whatever its font
            // takes for a baseline; letters stand on the text's.
            let icon = ('\u{e000}'..='\u{f8ff}').contains(&c);
            let top = match icon {
                true => y + (Self::line_height(px) - metrics.height as i32) / 2,
                false => baseline - metrics.ymin - metrics.height as i32,
            };
            for (row, line) in coverage.chunks_exact(metrics.width.max(1)).enumerate() {
                for (column, &amount) in line.iter().enumerate() {
                    if amount > 0 {
                        canvas.blend(left + column as i32, top + row as i32, color, amount);
                    }
                }
            }
            pen += metrics.advance_width;
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
