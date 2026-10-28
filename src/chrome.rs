//! The window a picture is shown in: the bar, buttons, corners and shadow
//! each system gives a window, macOS's, Windows 11's, GNOME's, KDE's,
//! Ubuntu's, a browser's and a terminal's, each light or dark, drawn here
//! at any size from a few numbers and colors, with no pictures to carry.
//! And what a capture of a window knows of the bar it had of its own,
//! which comes off when another is put on.

use crate::canvas::{mix, Canvas};
use crate::frame::{round_box, Layer};
use crate::text::Text;

/// Which window it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Style {
    /// None: the contents alone, rounded or not.
    #[default]
    None,
    Mac,
    Windows,
    Gnome,
    Kde,
    Ubuntu,
    /// A tab with the page's title, and an address under it.
    Browser,
    /// Dark whatever is in it, its title in a typewriter's letters.
    Terminal,
    /// Three buttons at the right and the title at the left, the way most
    /// desktops draw them: the window hotshot drew before the others.
    Desktop,
}

/// The windows to choose from, in the order they are offered.
pub const CHOICES: [Style; 8] = [Style::Mac, Style::Windows, Style::Gnome, Style::Kde, Style::Ubuntu, Style::Browser, Style::Terminal, Style::None];

/// Whether a window is light or dark: as its contents are, or as said.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl Theme {
    pub fn named(word: &str) -> Option<Theme> {
        match word.trim().to_ascii_lowercase().as_str() {
            "auto" | "system" => Some(Theme::Auto),
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Theme::Auto => "auto",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn says(self) -> &'static str {
        match self {
            Theme::Auto => "Auto",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }
}

/// A shadow of CSS's, so far down, so blurred, grown by so much, so
/// opaque: in pixels of a screen 1080 high.
const fn cast(y: i16, blur: i16, spread: i16, alpha: f32) -> Layer {
    Layer { x: 0, y, blur, spread, color: ((alpha * 255.0 + 0.5) as u32) << 24 }
}

impl Style {
    /// What a chip of it says.
    pub fn says(self) -> &'static str {
        match self {
            Style::None => "None",
            Style::Mac => "macOS",
            Style::Windows => "Windows",
            Style::Gnome => "GNOME",
            Style::Kde => "KDE",
            Style::Ubuntu => "Ubuntu",
            Style::Browser => "Browser",
            Style::Terminal => "Terminal",
            Style::Desktop => "Desktop",
        }
    }

    /// Its whole name, with what it is, for a list.
    pub fn long(self) -> &'static str {
        match self {
            Style::None => "No window",
            Style::Mac => "macOS",
            Style::Windows => "Windows 11",
            Style::Gnome => "GNOME (Adwaita)",
            Style::Kde => "KDE Plasma (Breeze)",
            Style::Ubuntu => "Ubuntu (Yaru)",
            Style::Browser => "Browser",
            Style::Terminal => "Terminal",
            Style::Desktop => "Desktop window",
        }
    }

    /// The word for it in the config file and on the command line.
    pub fn word(self) -> &'static str {
        match self {
            Style::None => "none",
            Style::Mac => "macos",
            Style::Windows => "windows",
            Style::Gnome => "gnome",
            Style::Kde => "kde",
            Style::Ubuntu => "ubuntu",
            Style::Browser => "browser",
            Style::Terminal => "terminal",
            Style::Desktop => "desktop",
        }
    }

    /// By its word, or another word for it: "mac" is macOS, as it was
    /// before there were others, and "adwaita" GNOME.
    pub fn named(word: &str) -> Option<Style> {
        Some(match word.trim().to_ascii_lowercase().as_str() {
            "none" | "no" | "off" | "no window" => Style::None,
            "mac" | "macos" | "osx" | "mac window" | "apple" => Style::Mac,
            "windows" | "win" | "windows11" | "windows 11" | "win11" => Style::Windows,
            "gnome" | "adwaita" => Style::Gnome,
            "kde" | "plasma" | "breeze" | "kde plasma" => Style::Kde,
            "ubuntu" | "yaru" => Style::Ubuntu,
            "browser" | "chrome" | "web" => Style::Browser,
            "terminal" | "term" | "shell" => Style::Terminal,
            "desktop" | "desktop window" => Style::Desktop,
            _ => return None,
        })
    }

    /// How tall its bar is, in pixels of a screen 1080 high.
    pub fn bar(self) -> f32 {
        match self {
            Style::None => 0.0,
            Style::Mac | Style::Kde => 28.0,
            Style::Windows | Style::Desktop => 30.0,
            Style::Gnome => 36.0,
            Style::Ubuntu => 34.0,
            Style::Browser => 56.0,
            Style::Terminal => 24.0,
        }
    }

    /// How round its corners are, in the same pixels.
    pub fn radius(self) -> f32 {
        match self {
            Style::None => 0.0,
            Style::Mac | Style::Ubuntu | Style::Browser | Style::Desktop => 10.0,
            Style::Windows | Style::Terminal => 8.0,
            Style::Gnome => 12.0,
            Style::Kde => 6.0,
        }
    }

    /// The shadow the system gives such a window: a wide soft one, and a
    /// faint line round its edge.
    pub fn shadow(self) -> &'static [Layer] {
        const MAC: [Layer; 2] = [cast(18, 40, 0, 0.35), cast(0, 0, 1, 0.25)];
        const WINDOWS: [Layer; 2] = [cast(12, 32, 0, 0.32), cast(0, 0, 1, 0.2)];
        const GNOME: [Layer; 2] = [cast(14, 36, 0, 0.35), cast(0, 0, 1, 0.18)];
        const KDE: [Layer; 2] = [cast(12, 30, 0, 0.3), cast(0, 0, 1, 0.2)];
        const BROWSER: [Layer; 2] = [cast(16, 36, 0, 0.33), cast(0, 0, 1, 0.2)];
        const TERMINAL: [Layer; 2] = [cast(14, 34, 0, 0.4), cast(0, 0, 1, 0.3)];
        const DESKTOP: [Layer; 2] = [cast(14, 34, 0, 0.3), cast(0, 0, 1, 0.2)];
        match self {
            Style::None => &[],
            Style::Mac => &MAC,
            Style::Windows => &WINDOWS,
            Style::Gnome | Style::Ubuntu => &GNOME,
            Style::Kde => &KDE,
            Style::Browser => &BROWSER,
            Style::Terminal => &TERMINAL,
            Style::Desktop => &DESKTOP,
        }
    }
}

/// Whether a window round a picture is dark: a terminal's always, one
/// said to be light or dark as it is said, and otherwise as dark as what
/// is in it, measured from a few thousand of its pixels.
pub fn is_dark(style: Style, theme: Theme, pixels: &[u32], width: i32, height: i32) -> bool {
    match (style, theme) {
        (Style::Terminal, _) | (_, Theme::Dark) => true,
        (_, Theme::Light) => false,
        _ => luma(pixels, width, height) < 112.0,
    }
}

/// How light a picture is on the whole, out of 255.
pub fn luma(pixels: &[u32], width: i32, height: i32) -> f32 {
    let count = (width.max(0) * height.max(0)) as usize;
    if count == 0 || pixels.len() < count {
        return 255.0;
    }
    let step = (count / 3000).max(1);
    let (mut sum, mut seen) = (0u64, 0u64);
    for pixel in pixels[..count].iter().step_by(step) {
        sum += u64::from(((pixel >> 16 & 0xFF) * 3 + (pixel >> 8 & 0xFF) * 6 + (pixel & 0xFF)) / 10);
        seen += 1;
    }
    sum as f32 / seen.max(1) as f32
}

/// What a capture of a window knows of where it came from: how far in
/// from each side its own frame went, the title bar and borders the
/// window manager drew round it (left, right, top, bottom, in its own
/// pixels); what the window was called; and how many of its pixels a
/// pixel of a screen 1080 high was.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Notes {
    pub insets: Option<[i32; 4]>,
    pub title: String,
    pub scale: f32,
}

impl Notes {
    /// As the lines kept in a PNG of it.
    pub fn write(&self) -> String {
        let mut out = String::new();
        if let Some([l, r, t, b]) = self.insets {
            out += &format!("insets {l} {r} {t} {b}\n");
        }
        if self.scale > 0.0 {
            out += &format!("scale {}\n", self.scale);
        }
        if !self.title.is_empty() {
            out += &format!("title {}\n", self.title.replace(['\n', '\r'], " "));
        }
        out
    }

    /// What `write` wrote, read; lines it does not know left out.
    pub fn read(text: &str) -> Notes {
        let mut notes = Notes::default();
        for line in text.lines() {
            let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
            match word {
                "insets" => {
                    let numbers: Vec<i32> = rest.split_whitespace().filter_map(|word| word.parse().ok()).collect();
                    if let [l, r, t, b] = numbers[..] {
                        notes.insets = Some([l, r, t, b].map(|side| side.max(0)));
                    }
                }
                "scale" => notes.scale = rest.trim().parse().unwrap_or(0.0),
                "title" => notes.title = rest.to_owned(),
                _ => {}
            }
        }
        notes
    }
}

/// The insets of a window's frame for a part of the screen taken of it:
/// how far in from each side of `area` the window inside its frame is,
/// `inner`, none past an edge.
pub fn insets_within(area: crate::canvas::Rect, inner: crate::canvas::Rect) -> [i32; 4] {
    [(inner.x - area.x).max(0), (area.right() - inner.right()).max(0), (inner.y - area.y).max(0), (area.bottom() - inner.bottom()).max(0)]
}

/// The colors a bar is drawn in: its ground, its words and its fainter
/// words.
fn inks(style: Style, dark: bool) -> (u32, u32, u32) {
    let (ink, dim) = if dark { (0xE4E4E7, 0xA1A1AA) } else { (0x3F3F46, 0x52525B) };
    let ground = match (style, dark) {
        (Style::Windows, true) => 0x202020,
        (Style::Windows, false) => 0xF3F3F3,
        (Style::Gnome, true) => 0x303030,
        (Style::Gnome | Style::Ubuntu, false) => 0xEBEBEB,
        (Style::Kde, true) => 0x2A2E32,
        (Style::Kde, false) => 0xDEE0E2,
        (Style::Ubuntu, true) => 0x2C2C2C,
        (Style::Browser, true) => 0x202124,
        (Style::Browser, false) => 0xDEE1E6,
        (Style::Terminal, _) => 0x2D2D2D,
        (Style::Desktop, true) => 0x2B2D31,
        (Style::Desktop, false) => 0xE8E9EC,
        (_, true) => 0x2B2B30,
        (_, false) => 0xECECEE,
    };
    (ground, ink, dim)
}

/// The color the inside of such a window is, under a picture with none.
pub fn body(style: Style, dark: bool) -> u32 {
    match (style, dark) {
        (Style::Terminal, _) => 0x16171A,
        (_, true) => 0x1C1C1F,
        (_, false) => 0xFFFFFF,
    }
}

/// Pixels being drawn on, each wholly opaque: shapes laid on them
/// smooth-edged, by how far each pixel is from the shape's edge.
struct Paper<'a> {
    pixels: &'a mut [u32],
    wide: i32,
    high: i32,
}

impl Paper<'_> {
    /// A shape, in `color` and `alpha` of it, inside a box, by how far a
    /// point is outside its edge: 0 on it, less inside.
    fn lay(&mut self, (left, top, right, bottom): (f32, f32, f32, f32), color: u32, alpha: f32, away: impl Fn(f32, f32) -> f32) {
        let (x0, y0) = ((left.floor() as i32 - 1).max(0), (top.floor() as i32 - 1).max(0));
        let (x1, y1) = ((right.ceil() as i32 + 1).min(self.wide), (bottom.ceil() as i32 + 1).min(self.high));
        for y in y0..y1 {
            for x in x0..x1 {
                let cover = (0.5 - away(x as f32 + 0.5, y as f32 + 0.5)).clamp(0.0, 1.0) * alpha;
                if cover > 0.0 {
                    let pixel = &mut self.pixels[(y * self.wide + x) as usize];
                    *pixel = 0xFF00_0000 | mix(*pixel, color, (cover * 256.0).round() as u32);
                }
            }
        }
    }

    /// A box with rounded corners, from its top left, so wide and tall.
    fn round(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32, alpha: f32) {
        let (hw, hh) = (w / 2.0, h / 2.0);
        let radius = radius.clamp(0.0, hw.min(hh));
        self.lay((x, y, x + w, y + h), color, alpha, |px, py| round_box(px - x - hw, py - y - hh, hw, hh, radius));
    }

    /// The same with only its top corners rounded, as a tab has.
    fn tab(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        let (hw, hh) = (w / 2.0, h / 2.0 + radius);
        let radius = radius.clamp(0.0, hw);
        self.lay((x, y, x + w, y + h), color, 1.0, |px, py| round_box(px - x - hw, py - y - hh, hw, hh, radius).max(py - y - h));
    }

    fn disc(&mut self, cx: f32, cy: f32, radius: f32, color: u32, alpha: f32) {
        self.lay((cx - radius, cy - radius, cx + radius, cy + radius), color, alpha, |px, py| (px - cx).hypot(py - cy) - radius);
    }

    /// A line from one point to another, so thick, with round ends.
    fn line(&mut self, from: (f32, f32), to: (f32, f32), thick: f32, color: u32) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let reach = thick;
        let bounds = (from.0.min(to.0) - reach, from.1.min(to.1) - reach, from.0.max(to.0) + reach, from.1.max(to.1) + reach);
        self.lay(bounds, color, 1.0, |px, py| {
            let along = (((px - from.0) * dx + (py - from.1) * dy) / (dx * dx + dy * dy).max(1e-4)).clamp(0.0, 1.0);
            (px - from.0 - dx * along).hypot(py - from.1 - dy * along) - thick / 2.0
        });
    }

    /// A square drawn round, not filled, about a point.
    fn square(&mut self, cx: f32, cy: f32, half: f32, thick: f32, color: u32) {
        self.lay((cx - half - thick, cy - half - thick, cx + half + thick, cy + half + thick), color, 1.0, |px, py| round_box(px - cx, py - cy, half, half, half * 0.2).abs() - thick / 2.0);
    }

    /// The marks on a window's buttons: a cross, a line, a square, and a
    /// chevron down or up; about a point, so large.
    fn mark(&mut self, mark: char, cx: f32, cy: f32, size: f32, thick: f32, color: u32) {
        let half = size / 2.0;
        match mark {
            'x' => {
                self.line((cx - half, cy - half), (cx + half, cy + half), thick, color);
                self.line((cx + half, cy - half), (cx - half, cy + half), thick, color);
            }
            '-' => self.line((cx - half, cy), (cx + half, cy), thick, color),
            'o' => self.square(cx, cy, half, thick, color),
            'v' => {
                self.line((cx - half, cy - half * 0.45), (cx, cy + half * 0.45), thick, color);
                self.line((cx, cy + half * 0.45), (cx + half, cy - half * 0.45), thick, color);
            }
            _ => {
                self.line((cx - half, cy + half * 0.45), (cx, cy - half * 0.45), thick, color);
                self.line((cx, cy - half * 0.45), (cx + half, cy + half * 0.45), thick, color);
            }
        }
    }
}

/// What is written in a bar, and in which face, how heavy.
struct Words<'a> {
    says: &'a str,
    px: f32,
    heavy: bool,
    face: usize,
    ink: u32,
}

/// As much of some words as fits in so much room, with an ellipsis where
/// it was cut short.
fn fitted(text: &mut Text, face: usize, says: &str, px: u32, room: f32) -> String {
    let room = room.max(0.0) as i32;
    if text.width_in(face, says, px) <= room {
        return says.to_owned();
    }
    let mut letters: Vec<char> = says.chars().collect();
    while !letters.is_empty() && text.width_in(face, &format!("{}\u{2026}", letters.iter().collect::<String>()), px) > room {
        letters.pop();
    }
    if letters.is_empty() {
        return String::new();
    }
    format!("{}\u{2026}", letters.iter().collect::<String>())
}

/// Some words laid in a bar: centered on `x` or begun at it, in the
/// middle of the bar down at `y`, cut short to fit between `from` and
/// `to` across it.
fn write(pixels: &mut [u32], (wide, high): (i32, i32), text: &mut Text, words: &Words, (x, y): (f32, f32), centered: bool, (from, to): (f32, f32)) {
    let px = words.px.round() as u32;
    if px < 6 || words.says.trim().is_empty() {
        return;
    }
    let room = if centered { 2.0 * (x - from).min(to - x) } else { to - x };
    let look = text.look;
    text.look.heavy = i32::from(words.heavy);
    let says = fitted(text, words.face, words.says.trim(), px, room);
    let shown = text.width_in(words.face, &says, px);
    let left = if centered { x - shown as f32 / 2.0 } else { x };
    let mut canvas = Canvas::new(pixels, wide, high);
    canvas.set_clip(crate::canvas::Rect::new(from.floor() as i32, 0, (to - from).ceil() as i32, high));
    text.draw_in(words.face, &mut canvas, left.round() as i32, (y - Text::line_height(px) as f32 / 2.0).round() as i32, px, words.ink, &says);
    text.look = look;
}

/// What is to be drawn in a bar: which window, light or dark, its title,
/// and a browser's address.
#[derive(Clone, Copy)]
pub struct Bar<'a> {
    pub style: Style,
    pub dark: bool,
    pub title: &'a str,
    pub address: &'a str,
}

/// A window's bar drawn into pixels `wide` by `high`, where the bar is
/// `size` large with its top left at `origin`, both in those pixels, and
/// `scale` of them to each pixel of a screen 1080 high. What is outside
/// the bar is left as it is; what is inside is wholly opaque. Without
/// `text`, its words are left out.
#[allow(clippy::too_many_arguments)]
pub fn draw_bar(pixels: &mut [u32], (wide, high): (i32, i32), origin: (f32, f32), size: (f32, f32), scale: f32, bar: Bar, text: Option<&mut Text>) {
    if bar.style == Style::None || size.0 <= 0.0 || size.1 <= 0.0 {
        return;
    }
    let (ox, oy) = origin;
    let (bw, bh) = size;
    let s = scale.max(0.01);
    let dark = bar.dark;
    let (ground, ink, dim) = inks(bar.style, dark);
    let mut paper = Paper { pixels, wide, high };
    paper.lay((ox, oy, ox + bw, oy + bh), ground, 1.0, |px, py| (ox - px).max(px - ox - bw).max(oy - py).max(py - oy - bh));
    let middle = oy + bh / 2.0;
    let right = ox + bw;
    let lights = |paper: &mut Paper, first: f32, gap: f32, radius: f32, cy: f32| {
        for (at, color) in [0xFF5F57, 0xFEBC2E, 0x28C840].into_iter().enumerate() {
            paper.disc(ox + first + at as f32 * gap, cy, radius, color, 1.0);
        }
    };
    let gray = 0x7F7F7F;
    let thin = (1.0 * s).max(0.8);
    // Where the words go, once the shapes are down: what, where, whether
    // in the middle, and between what.
    let mut writing: Vec<(Words, (f32, f32), bool, (f32, f32))> = Vec::new();
    match bar.style {
        Style::Mac => {
            lights(&mut paper, 15.5 * s, 18.0 * s, 5.5 * s, middle);
            writing.push((Words { says: bar.title, px: 10.5 * s, heavy: true, face: 0, ink }, (ox + bw / 2.0, middle), true, (ox + 64.0 * s, right - 64.0 * s)));
        }
        Style::Windows => {
            paper.round(ox + 10.0 * s, middle - 7.0 * s, 14.0 * s, 14.0 * s, 3.0 * s, 0x3E63DD, 1.0);
            for (at, mark) in ['-', 'o', 'x'].into_iter().enumerate() {
                let cx = right - (100.0 - at as f32 * 40.0) * s;
                paper.mark(mark, cx, middle, 9.0 * s, thin, ink);
            }
            writing.push((Words { says: bar.title, px: 11.0 * s, heavy: false, face: 0, ink }, (ox + 32.0 * s, middle), false, (ox + 32.0 * s, right - 128.0 * s)));
        }
        Style::Gnome => {
            paper.round(ox + 8.0 * s, middle - 12.0 * s, 24.0 * s, 24.0 * s, 6.0 * s, gray, 0.18);
            let cx = right - 18.0 * s;
            paper.disc(cx, middle, 10.0 * s, gray, 0.22);
            paper.mark('x', cx, middle, 7.0 * s, thin * 1.2, ink);
            writing.push((Words { says: bar.title, px: 11.0 * s, heavy: true, face: 0, ink }, (ox + bw / 2.0, middle), true, (ox + 44.0 * s, right - 44.0 * s)));
        }
        Style::Kde => {
            paper.round(ox + 6.0 * s, middle - 7.0 * s, 14.0 * s, 14.0 * s, 3.0 * s, 0x3DAEE9, 1.0);
            let close = right - 15.0 * s;
            paper.disc(close, middle, 9.0 * s, 0xDA4453, 1.0);
            paper.mark('x', close, middle, 6.0 * s, thin * 1.2, 0xFFFFFF);
            paper.mark('^', close - 24.0 * s, middle, 8.0 * s, thin * 1.2, ink);
            paper.mark('v', close - 48.0 * s, middle, 8.0 * s, thin * 1.2, ink);
            writing.push((Words { says: bar.title, px: 11.0 * s, heavy: false, face: 0, ink }, (ox + bw / 2.0, middle), true, (ox + 26.0 * s, right - 76.0 * s)));
        }
        Style::Ubuntu => {
            let close = right - 19.0 * s;
            for (at, mark) in ['x', 'o', '-'].into_iter().enumerate() {
                let cx = close - at as f32 * 26.0 * s;
                match mark {
                    'x' => paper.disc(cx, middle, 9.0 * s, 0xE95420, 1.0),
                    _ => paper.disc(cx, middle, 9.0 * s, gray, 0.2),
                }
                paper.mark(mark, cx, middle, if mark == 'o' { 6.0 } else { 6.5 } * s, thin * 1.1, if mark == 'x' { 0xFFFFFF } else { ink });
            }
            writing.push((Words { says: bar.title, px: 11.0 * s, heavy: true, face: 0, ink }, (ox + bw / 2.0, middle), true, (ox + 84.0 * s, right - 84.0 * s)));
        }
        Style::Browser => {
            let (tab_ink, address) = if dark { (0x35363A, 0x282A2D) } else { (0xFFFFFF, 0xF1F3F4) };
            lights(&mut paper, 14.5 * s, 14.0 * s, 4.5 * s, oy + 14.5 * s);
            let tab = (bw - 67.0 * s).clamp(0.0, 150.0 * s);
            paper.tab(ox + 59.0 * s, oy + 6.0 * s, tab, 22.0 * s, 7.0 * s, tab_ink);
            paper.lay((ox, oy + 28.0 * s, right, oy + bh), tab_ink, 1.0, |px, py| (ox - px).max(px - right).max(oy + 28.0 * s - py).max(py - oy - bh));
            paper.round(ox + 10.0 * s, oy + 33.0 * s, (bw - 20.0 * s).max(0.0), 18.0 * s, 9.0 * s, address, 1.0);
            writing.push((Words { says: bar.title, px: 10.0 * s, heavy: false, face: 0, ink }, (ox + 67.0 * s, oy + 17.0 * s), false, (ox + 67.0 * s, ox + 51.0 * s + tab)));
            writing.push((Words { says: bar.address, px: 10.0 * s, heavy: false, face: 0, ink: dim }, (ox + 20.0 * s, oy + 42.0 * s), false, (ox + 20.0 * s, right - 20.0 * s)));
        }
        Style::Terminal => {
            lights(&mut paper, 12.5 * s, 15.0 * s, 4.5 * s, middle);
            writing.push((Words { says: bar.title, px: 10.0 * s, heavy: false, face: 3, ink: 0xA1A1AA }, (ox + bw / 2.0, middle), true, (ox + 56.0 * s, right - 56.0 * s)));
        }
        Style::Desktop => {
            let ink = if dark { 0xC9CCD3 } else { 0x4A4D57 };
            for (at, mark) in ['x', 'o', '-'].into_iter().enumerate() {
                paper.mark(mark, right - (22.0 + at as f32 * 34.0) * s, middle, 10.0 * s, thin * 1.4, ink);
            }
            writing.push((Words { says: bar.title, px: 12.5 * s, heavy: false, face: 0, ink }, (ox + 12.0 * s, middle), false, (ox + 12.0 * s, right - 120.0 * s)));
        }
        Style::None => {}
    }
    if let Some(text) = text {
        for (words, at, centered, between) in &writing {
            write(pixels, (wide, high), text, words, *at, *centered, *between);
        }
        // Drawing words leaves their pixels without how opaque they are,
        // which in the bar is wholly.
        let (x0, y0) = ((ox.floor() as i32).max(0), (oy.floor() as i32).max(0));
        let (x1, y1) = (((ox + bw).ceil() as i32).min(wide), ((oy + bh).ceil() as i32).min(high));
        for y in y0..y1 {
            for pixel in &mut pixels[(y * wide + x0) as usize..(y * wide + x1.max(x0)) as usize] {
                *pixel |= 0xFF00_0000;
            }
        }
    }
}

/// A whole bar as pixels, `wide` by `high`, for a window `wide` across.
pub fn bar_pixels(wide: i32, high: i32, scale: f32, bar: Bar, text: Option<&mut Text>) -> Vec<u32> {
    let mut pixels = vec![0xFF00_0000; (wide.max(0) * high.max(0)) as usize];
    draw_bar(&mut pixels, (wide, high), (0.0, 0.0), (wide as f32, high as f32), scale, bar, text);
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(pixels: &[u32], wide: i32, (x, y): (i32, i32)) -> u32 {
        pixels[(y * wide + x) as usize] & 0xFF_FFFF
    }

    #[test]
    fn the_old_names_and_the_new_find_the_same_windows() {
        assert_eq!(Style::named("mac"), Some(Style::Mac));
        assert_eq!(Style::named("macOS"), Some(Style::Mac));
        assert_eq!(Style::named("Windows"), Some(Style::Windows));
        assert_eq!(Style::named("adwaita"), Some(Style::Gnome));
        assert_eq!(Style::named("desktop"), Some(Style::Desktop));
        assert_eq!(Style::named("tv"), None);
        for style in CHOICES {
            assert_eq!(Style::named(style.word()), Some(style));
        }
        assert_eq!(Theme::named("dark"), Some(Theme::Dark));
    }

    #[test]
    fn each_bar_has_its_ground_lights_and_buttons_light_and_dark() {
        let mut text = Text::load();
        let (wide, high) = (400, 30);
        // macOS: the three lights at the left, on its light gray.
        let mac = bar_pixels(wide, 28, 1.0, Bar { style: Style::Mac, dark: false, title: "Settings", address: "" }, Some(&mut text));
        assert_eq!(color(&mac, wide, (15, 14)), 0xFF5F57);
        assert_eq!(color(&mac, wide, (51, 14)), 0x28C840);
        assert_eq!(color(&mac, wide, (100, 3)), 0xECECEE);
        // Its title is written in the middle, in dark letters.
        assert!((180..220).any(|x| (8..20).any(|y| color(&mac, wide, (x, y)) < 0x808080)));
        let dark = bar_pixels(wide, 28, 1.0, Bar { style: Style::Mac, dark: true, title: "", address: "" }, None);
        assert_eq!(color(&dark, wide, (100, 3)), 0x2B2B30);
        // KDE's close is red, Ubuntu's orange, both at the right.
        let kde = bar_pixels(wide, high, 1.0, Bar { style: Style::Kde, dark: false, title: "", address: "" }, None);
        assert_eq!(color(&kde, wide, (wide - 15 - 7, 15)), 0xDA4453);
        let ubuntu = bar_pixels(wide, 34, 1.0, Bar { style: Style::Ubuntu, dark: false, title: "", address: "" }, None);
        assert_eq!(color(&ubuntu, wide, (wide - 19 - 6, 17)), 0xE95420);
        // Windows 11: its blue mark at the left.
        let windows = bar_pixels(wide, high, 1.0, Bar { style: Style::Windows, dark: true, title: "", address: "" }, None);
        assert_eq!(color(&windows, wide, (17, 15)), 0x3E63DD);
        assert_eq!(color(&windows, wide, (200, 2)), 0x202020);
        // A browser: a white tab over its gray, and the address row white.
        let browser = bar_pixels(wide, 56, 1.0, Bar { style: Style::Browser, dark: false, title: "Acme", address: "acme.app" }, Some(&mut text));
        assert_eq!(color(&browser, wide, (180, 3)), 0xDEE1E6);
        assert_eq!(color(&browser, wide, (180, 30)), 0xFFFFFF);
        assert_eq!(color(&browser, wide, (300, 42)), 0xF1F3F4);
        // A terminal is dark however it is asked.
        assert!(is_dark(Style::Terminal, Theme::Light, &[0xFFFFFF], 1, 1));
        let terminal = bar_pixels(wide, 24, 1.0, Bar { style: Style::Terminal, dark: true, title: "", address: "" }, None);
        assert_eq!(color(&terminal, wide, (200, 2)), 0x2D2D2D);
        // Twice the size, the lights are twice as far in.
        let large = bar_pixels(2 * wide, 56, 2.0, Bar { style: Style::Mac, dark: false, title: "", address: "" }, None);
        assert_eq!(color(&large, 2 * wide, (31, 28)), 0xFF5F57);
        assert_eq!(color(&large, 2 * wide, (103, 28)), 0x28C840);
    }

    #[test]
    fn auto_takes_the_theme_of_what_is_in_the_window() {
        let light = vec![0xF4F4F5; 100];
        let dark = vec![0x18181B; 100];
        assert!(!is_dark(Style::Mac, Theme::Auto, &light, 10, 10));
        assert!(is_dark(Style::Mac, Theme::Auto, &dark, 10, 10));
        assert!(is_dark(Style::Mac, Theme::Dark, &light, 10, 10));
        assert!(!is_dark(Style::Gnome, Theme::Light, &dark, 10, 10));
    }

    #[test]
    fn notes_of_a_capture_are_written_and_read_back() {
        let notes = Notes { insets: Some([2, 2, 28, 2]), title: "Settings \u{2014} Acme".into(), scale: 1.5 };
        assert_eq!(Notes::read(&notes.write()), notes);
        assert_eq!(Notes::read(""), Notes::default());
        let area = crate::canvas::Rect::new(100, 100, 400, 300);
        assert_eq!(insets_within(area, crate::canvas::Rect::new(102, 128, 396, 270)), [2, 2, 28, 2]);
    }
}
