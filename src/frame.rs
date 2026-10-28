//! The picture set on something: a backdrop of colors run into each
//! other, or a photograph, with room round it and a shadow under
//! it. This is the picture as it is shown off, where the plain one is
//! the picture as it is.

use std::process::Command;

use rayon::prelude::*;

use crate::canvas::mix;

/// How a backdrop is colored.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// A color with soft patches of others on it, each with where its
    /// middle is and how far it reaches, as parts of the whole: colors
    /// that run into each other every way at once.
    Mesh(u32, &'static [(f32, f32, f32, u32)]),
    /// Colors run one into the next along a line at an angle, in
    /// degrees: 0 is left to right, 90 top to bottom.
    Line(f32, &'static [u32]),
    /// One color.
    Plain(u32),
}

/// The backdrops there are, by what each is called: sixteen meshes,
/// sixteen that run along a line, and eight plain. The box of them
/// shows them eight to a row, so each kind has rows to itself.
pub const BACKDROPS: [(&str, Kind); 40] = [
    ("Aurora", Kind::Mesh(0x172554, &[(0.15, 0.2, 0.75, 0x3B82F6), (0.85, 0.15, 0.65, 0x8B5CF6), (0.6, 0.95, 0.7, 0x14B8A6)])),
    ("Orchid", Kind::Mesh(0x2E1065, &[(0.1, 0.1, 0.75, 0xEC4899), (0.9, 0.3, 0.7, 0x8B5CF6), (0.5, 1.0, 0.7, 0x6366F1)])),
    ("Ember", Kind::Mesh(0x7C2D12, &[(0.1, 0.9, 0.85, 0xF97316), (0.9, 0.1, 0.85, 0xE11D48), (0.65, 0.5, 0.45, 0xFBBF24)])),
    ("Lagoon", Kind::Mesh(0x115E59, &[(0.2, 0.2, 0.75, 0x06B6D4), (0.9, 0.8, 0.8, 0x10B981), (0.85, 0.1, 0.5, 0x3B82F6)])),
    ("Nebula", Kind::Mesh(0x1E1B4B, &[(0.2, 0.8, 0.75, 0x7C3AED), (0.8, 0.2, 0.65, 0xDB2777), (0.95, 0.95, 0.5, 0x2563EB)])),
    ("Peacock", Kind::Mesh(0x0C4A6E, &[(0.1, 0.9, 0.8, 0x0D9488), (0.9, 0.1, 0.7, 0x6D28D9), (0.5, 0.4, 0.5, 0x0EA5E9)])),
    ("Rosewood", Kind::Mesh(0x4C0519, &[(0.1, 0.1, 0.8, 0xBE123C), (0.9, 0.9, 0.8, 0x9D174D), (0.9, 0.1, 0.5, 0xFB923C)])),
    ("Midnight", Kind::Mesh(0x020617, &[(0.1, 0.1, 0.85, 0x1E3A8A), (0.9, 0.9, 0.8, 0x312E81), (0.9, 0.1, 0.5, 0x0F766E)])),
    ("Sorbet", Kind::Mesh(0xFFF1E6, &[(0.0, 0.0, 0.85, 0xFDBA74), (1.0, 0.2, 0.75, 0xF9A8D4), (0.5, 1.0, 0.7, 0xFDE68A)])),
    ("Candy", Kind::Mesh(0xFDE2F3, &[(0.1, 0.2, 0.75, 0xFDA4AF), (0.9, 0.1, 0.7, 0xC4B5FD), (0.6, 0.95, 0.7, 0x93C5FD)])),
    ("Glacier", Kind::Mesh(0xE0F2FE, &[(0.1, 0.1, 0.75, 0x7DD3FC), (0.9, 0.2, 0.7, 0xC7D2FE), (0.5, 1.0, 0.8, 0xA5F3FC)])),
    ("Meadow", Kind::Mesh(0xECFCCB, &[(0.1, 0.9, 0.75, 0x86EFAC), (0.9, 0.1, 0.7, 0xFDE68A), (0.9, 0.9, 0.6, 0x67E8F9)])),
    ("Sunrise", Kind::Mesh(0xFEF3C7, &[(0.0, 1.0, 0.9, 0xFB7185), (1.0, 0.0, 0.8, 0xFBBF24), (0.5, 0.5, 0.45, 0xFDBA74)])),
    ("Citrus", Kind::Mesh(0xFEF9C3, &[(0.0, 0.0, 0.8, 0xBEF264), (1.0, 1.0, 0.8, 0xFDBA74), (1.0, 0.0, 0.6, 0xFDE047)])),
    ("Dune", Kind::Mesh(0xF5E6D3, &[(0.1, 0.9, 0.8, 0xE7B98A), (0.9, 0.1, 0.7, 0xF3D9B1), (0.9, 0.9, 0.6, 0xD4A373)])),
    ("Pearl", Kind::Mesh(0xFAFAF9, &[(0.1, 0.1, 0.75, 0xE7E5E4), (0.9, 0.9, 0.8, 0xDDD6FE), (0.9, 0.1, 0.6, 0xFBCFE8)])),
    ("Dusk", Kind::Line(45.0, &[0x4F46E5, 0xEC4899])),
    ("Ocean", Kind::Line(45.0, &[0x06B6D4, 0x3B82F6])),
    ("Sunset", Kind::Line(45.0, &[0xF97316, 0xEC4899])),
    ("Mint", Kind::Line(45.0, &[0x34D399, 0x06B6D4])),
    ("Peach", Kind::Line(45.0, &[0xFDBA74, 0xF9A8D4])),
    ("Violet", Kind::Line(45.0, &[0x8E4EC6, 0x3E63DD])),
    ("Iris", Kind::Line(60.0, &[0x1E1B4B, 0x4338CA, 0x7C3AED])),
    ("Arctic", Kind::Line(30.0, &[0x0EA5E9, 0x6366F1, 0xA855F7])),
    ("Grape", Kind::Line(30.0, &[0x7E22CE, 0xDB2777, 0xF97316])),
    ("Flamingo", Kind::Line(60.0, &[0xFB7185, 0xFDBA74])),
    ("Lime", Kind::Line(60.0, &[0xA3E635, 0x22C55E, 0x0D9488])),
    ("Rose", Kind::Line(90.0, &[0xFFE4E6, 0xFECDD3, 0xFDA4AF])),
    ("Sky", Kind::Line(90.0, &[0xE0F2FE, 0x7DD3FC, 0x38BDF8])),
    ("Steel", Kind::Line(90.0, &[0x64748B, 0x1E293B])),
    ("Slate", Kind::Line(45.0, &[0x334155, 0x0F172A])),
    ("Paper", Kind::Line(45.0, &[0xF8FAFC, 0xE2E8F0])),
    ("White", Kind::Plain(0xFFFFFF)),
    ("Fog", Kind::Plain(0xF1F5F9)),
    ("Sand", Kind::Plain(0xF5F0E6)),
    ("Blush", Kind::Plain(0xFCE7F3)),
    ("Navy", Kind::Plain(0x172554)),
    ("Forest", Kind::Plain(0x14532D)),
    ("Graphite", Kind::Plain(0x27272A)),
    ("Ink", Kind::Plain(0x0B0B0F)),
];

/// The backdrops the box of them shows before it is asked for the rest.
pub const FIRST: [&str; 6] = ["Aurora", "Orchid", "Sorbet", "Glacier", "Dusk", "Graphite"];

/// Which of the backdrops one is, by what it is called, in whatever case.
pub fn named(name: &str) -> Option<usize> {
    BACKDROPS.iter().position(|(known, _)| known.eq_ignore_ascii_case(name))
}

/// The color of a backdrop at a place on it: so far across and so far
/// down, each as a part of the whole. `noise`, of 0 to 1 and different
/// from one pixel to the next, keeps the steps between colors from
/// showing as bands.
pub fn shade(which: usize, across: f32, down: f32, noise: f32) -> u32 {
    let parts = |color: u32| [(color >> 16 & 0xFF) as f32, (color >> 8 & 0xFF) as f32, (color & 0xFF) as f32];
    let between = |from: [f32; 3], to: [f32; 3], amount: f32| [0, 1, 2].map(|part| from[part] + (to[part] - from[part]) * amount);
    let color = match BACKDROPS[which.min(BACKDROPS.len() - 1)].1 {
        Kind::Plain(color) => return color,
        // Each patch laid over what is there, less of it the further
        // from its middle, and none past its reach.
        Kind::Mesh(under, patches) => patches.iter().fold(parts(under), |color, &(x, y, reach, patch)| {
            let near = (1.0 - (across - x).hypot(down - y) / reach).clamp(0.0, 1.0);
            between(color, parts(patch), near * near * (3.0 - 2.0 * near))
        }),
        // How far along the line the place is, from one corner of the
        // whole to the one opposite; and between which two colors.
        Kind::Line(angle, colors) => {
            let (sine, cosine) = angle.to_radians().sin_cos();
            let along = (((across - 0.5) * cosine + (down - 0.5) * sine) / (cosine.abs() + sine.abs()) + 0.5).clamp(0.0, 1.0) * (colors.len() - 1) as f32;
            let from = (along as usize).min(colors.len() - 2);
            between(parts(colors[from]), parts(colors[from + 1]), along - from as f32)
        }
    };
    let whole = |part: f32| (part + noise - 0.5).round().clamp(0.0, 255.0) as u32;
    whole(color[0]) << 16 | whole(color[1]) << 8 | whole(color[2])
}

/// How much room there is round the picture, in pixels of a screen 1080
/// high: none, and three widths of it.
pub const PADS: [i32; 4] = [0, 32, 64, 112];

/// One shadow of the several a picture may cast, as CSS's box-shadow
/// says one: so far across and down, so blurred, and grown by so much
/// all round, in pixels of a screen 1080 high; and its color, with how
/// opaque it is in the top byte.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Layer {
    pub x: i16,
    pub y: i16,
    pub blur: i16,
    pub spread: i16,
    pub color: u32,
}

/// The shadows a picture casts, the one over the rest first, as CSS
/// has them: none, or as many as four.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Shadow {
    pub layers: [Layer; 4],
    pub count: u8,
}

/// Black, so opaque out of 1.
const fn black(alpha: f32) -> u32 {
    ((alpha * 255.0 + 0.5) as u32) << 24
}

const fn layer(x: i16, y: i16, blur: i16, spread: i16, alpha: f32) -> Layer {
    Layer { x, y, blur, spread, color: black(alpha) }
}

/// Tailwind's shadows, by Tailwind's names for them, smallest first.
pub const SHADOWS: [(&str, &[Layer]); 7] = [
    ("2xs", &[layer(0, 1, 0, 0, 0.05)]),
    ("xs", &[layer(0, 1, 2, 0, 0.05)]),
    ("sm", &[layer(0, 1, 3, 0, 0.1), layer(0, 1, 2, -1, 0.1)]),
    ("md", &[layer(0, 4, 6, -1, 0.1), layer(0, 2, 4, -2, 0.1)]),
    ("lg", &[layer(0, 10, 15, -3, 0.1), layer(0, 4, 6, -4, 0.1)]),
    ("xl", &[layer(0, 20, 25, -5, 0.1), layer(0, 8, 10, -6, 0.1)]),
    ("2xl", &[layer(0, 25, 50, -12, 0.25)]),
];

/// The shadow a picture is given when it is just given one.
pub const SHADOW: &str = "2xl";

impl Shadow {
    pub const NONE: Shadow = Shadow { layers: [Layer { x: 0, y: 0, blur: 0, spread: 0, color: 0 }; 4], count: 0 };

    pub fn on(&self) -> bool {
        self.count > 0
    }

    fn of(layers: &[Layer]) -> Shadow {
        let mut shadow = Shadow::NONE;
        for (at, layer) in layers.iter().take(4).enumerate() {
            shadow.layers[at] = *layer;
        }
        shadow.count = layers.len().min(4) as u8;
        shadow
    }

    /// One of Tailwind's, by its name.
    pub fn named(name: &str) -> Option<Shadow> {
        let name = name.trim().to_ascii_lowercase();
        let name = name.strip_prefix("shadow-").unwrap_or(&name);
        SHADOWS.iter().find(|(known, _)| *known == name).map(|(_, layers)| Shadow::of(layers))
    }

    /// One of Tailwind's, by where it is among them, smallest first.
    pub fn of_preset(which: usize) -> Shadow {
        Shadow::of(SHADOWS[which.min(SHADOWS.len() - 1)].1)
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers[..usize::from(self.count.min(4))]
    }

    /// What it is called, if it is one of Tailwind's.
    pub fn name(&self) -> Option<&'static str> {
        SHADOWS.iter().find(|(_, layers)| *layers == self.layers()).map(|(name, _)| *name)
    }

    /// As CSS says it, a layer after another.
    pub fn css(&self) -> String {
        let said: Vec<String> = self
            .layers()
            .iter()
            .map(|layer| {
                let (alpha, color) = ((layer.color >> 24) as f32 / 255.0, layer.color & 0xFF_FFFF);
                let alpha = format!("{alpha:.3}").trim_end_matches('0').trim_end_matches('.').to_owned();
                format!("{}px {}px {}px {}px rgb({} {} {} / {alpha})", layer.x, layer.y, layer.blur, layer.spread, color >> 16, color >> 8 & 0xFF, color & 0xFF)
            })
            .collect();
        if said.is_empty() { "none".into() } else { said.join(", ") }
    }

    /// From what it is called, Tailwind's way (`lg`, `shadow-lg`), or
    /// as CSS's box-shadow says it: `0 10px 15px -3px rgb(0 0 0 / 0.1),
    /// 0 4px 6px -4px rgb(0 0 0 / 0.1)`. `none` is none.
    pub fn parse(text: &str) -> Result<Shadow, String> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("none") {
            return Ok(Shadow::NONE);
        }
        if let Some(shadow) = Shadow::named(text) {
            return Ok(shadow);
        }
        // The layers, at the commas that are not inside a color.
        let (mut layers, mut depth, mut begun) = (Vec::new(), 0, 0);
        for (at, letter) in text.char_indices() {
            match letter {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    layers.push(&text[begun..at]);
                    begun = at + 1;
                }
                _ => {}
            }
        }
        layers.push(&text[begun..]);
        if layers.len() > 4 {
            return Err(format!("too many shadows in '{text}': four at most"));
        }
        let mut made = Vec::new();
        for layer in layers {
            made.push(read_layer(layer.trim()).ok_or_else(|| {
                let names: Vec<&str> = SHADOWS.iter().map(|(name, _)| *name).collect();
                format!("invalid shadow '{}', expected one of {} or none, or a CSS box-shadow such as '0 10px 15px -3px rgb(0 0 0 / 0.1)'", layer.trim(), names.join(", "))
            })?);
        }
        Ok(Shadow::of(&made))
    }

    /// How far past the picture it reaches, beside it and above and
    /// below it, in pixels of a screen 1080 high: where it is as good as
    /// gone, three times its blur's deviation out.
    fn reach(&self) -> (f32, f32) {
        self.layers().iter().fold((0.0, 0.0), |(across, down), layer| {
            let out = f32::from(layer.spread) + 1.5 * f32::from(layer.blur.max(0));
            (across.max(f32::from(layer.x).abs() + out), down.max(f32::from(layer.y).abs() + out))
        })
    }
}

/// One layer of a CSS box-shadow: two to four lengths and a color, in
/// any order of the two, but never inset.
fn read_layer(text: &str) -> Option<Layer> {
    let (mut lengths, mut color, mut word, mut depth) = (Vec::new(), None, String::new(), 0);
    // Words, with what is inside a color's brackets kept with it.
    let mut words = Vec::new();
    for letter in text.chars() {
        match letter {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if letter.is_whitespace() && depth == 0 {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(letter);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    for word in words {
        let lower = word.to_ascii_lowercase();
        if let Some(length) = read_length(&lower) {
            lengths.push(length);
        } else if color.is_none() {
            color = Some(read_color(&lower)?);
        } else {
            return None;
        }
    }
    let [x, y] = lengths[..lengths.len().min(2)] else { return None };
    let (blur, spread) = (lengths.get(2).copied().unwrap_or(0), lengths.get(3).copied().unwrap_or(0));
    (lengths.len() <= 4 && blur >= 0).then_some(Layer { x, y, blur, spread, color: color.unwrap_or(black(1.0)) })
}

/// A length: so many pixels, or of the size text is in CSS, 16 of them.
fn read_length(word: &str) -> Option<i16> {
    let (number, times) = if let Some(number) = word.strip_suffix("px") {
        (number, 1.0)
    } else if let Some(number) = word.strip_suffix("rem").or_else(|| word.strip_suffix("em")) {
        (number, 16.0)
    } else {
        (word, 1.0)
    };
    let value = number.parse::<f32>().ok()?;
    // A bare number is only a length if it is nought, as CSS has it;
    // otherwise it is taken as pixels all the same.
    (value.is_finite() && value.abs() < 2000.0).then(|| (value * times).round() as i16)
}

/// A color as CSS says one: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb()` or `rgba()` with commas or without and how opaque after a
/// slash, or black, white or transparent. With how opaque in the top
/// byte.
fn read_color(word: &str) -> Option<u32> {
    match word {
        "black" => return Some(black(1.0)),
        "white" => return Some(0xFFFF_FFFF),
        "transparent" => return Some(0),
        _ => {}
    }
    if let Some(hex) = word.strip_prefix('#') {
        let digits: Vec<u32> = hex.chars().map(|digit| digit.to_digit(16)).collect::<Option<_>>()?;
        let [r, g, b, a] = match digits.len() {
            3 | 4 => [0, 1, 2, 3].map(|at| digits.get(at).map_or(255, |digit| digit * 17)),
            6 | 8 => [0, 2, 4, 6].map(|at| if at < digits.len() { digits[at] * 16 + digits[at + 1] } else { 255 }),
            _ => return None,
        };
        return Some(a << 24 | r << 16 | g << 8 | b);
    }
    let inside = word.strip_prefix("rgba(").or_else(|| word.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<&str> = inside.split([',', ' ', '/']).filter(|part| !part.is_empty()).collect();
    let part = |text: &str, whole: f32| -> Option<f32> {
        match text.strip_suffix('%') {
            Some(percent) => Some(percent.parse::<f32>().ok()? / 100.0 * whole),
            None => text.parse::<f32>().ok(),
        }
    };
    let (r, g, b) = (part(parts.first()?, 255.0)?, part(parts.get(1)?, 255.0)?, part(parts.get(2)?, 255.0)?);
    let a = parts.get(3).map_or(Some(1.0), |alpha| part(alpha, 1.0))?;
    let byte = |value: f32| value.round().clamp(0.0, 255.0) as u32;
    (parts.len() <= 4).then(|| byte(a * 255.0) << 24 | byte(r) << 16 | byte(g) << 8 | byte(b))
}

/// The error function, near enough for a shadow's edge: Abramowitz and
/// Stegun's, to within a ten-thousandth.
pub(crate) fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    y.copysign(x)
}

/// A picture to stand behind another: its size, and its pixels.
#[derive(Clone)]
pub struct Photo {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
}

/// What the picture is set on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Backdrop {
    /// Nothing: what is round the picture is see-through.
    #[default]
    None,
    /// One of `BACKDROPS`.
    Gradient(usize),
    /// A photograph, handed over with it.
    Photo,
}

/// A window the picture is shown in, within its frame: the bar such a
/// window has over what is in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Window {
    #[default]
    None,
    /// The three lights, and an address in a pill in the middle.
    Browser,
    /// The three lights alone.
    Mac,
    /// Dark, whatever the picture: a terminal that lost its own.
    Terminal,
    /// Three buttons at the right, the way most desktops draw them.
    Desktop,
}

/// The windows there are, by what each is called.
pub const WINDOWS: [(Window, &str); 5] = [(Window::None, "No window"), (Window::Browser, "Browser"), (Window::Mac, "Mac window"), (Window::Terminal, "Terminal"), (Window::Desktop, "Desktop window")];

impl Window {
    /// How tall its bar is, at a scale.
    pub fn bar(self, scale: f32) -> i32 {
        if self == Window::None { 0 } else { (30.0 * scale).round() as i32 }
    }
}

/// How the picture is set: on what, with how much room, whether with a
/// shadow, and in what window.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Frame {
    pub backdrop: Backdrop,
    /// How much room there is beside the picture, and above and below
    /// it, in pixels of a screen 1080 high: any amount of each.
    pub pad: (i32, i32),
    /// How round the corners of the whole are, in the same pixels.
    pub radius: i32,
    pub shadow: Shadow,
    pub window: Window,
}

impl Frame {
    /// Whether it does anything at all to the picture.
    pub fn is_plain(&self) -> bool {
        self.backdrop == Backdrop::None && self.pad == (0, 0) && !self.shadow.on() && self.window == Window::None
    }

    /// The room beside the picture, and above and below it, in pixels
    /// at a scale. A shadow has to have somewhere to fall, whatever the
    /// room asked for; and a window's bar, which goes in the room over
    /// the picture, room to be in.
    pub fn room(&self, scale: f32) -> (i32, i32) {
        let reach = self.shadow.reach();
        let asked = |pad: i32, least: f32| ((pad.max(0) as f32 * scale).round() as i32).max((least * scale).ceil() as i32);
        let bar = self.window.bar(scale);
        let top = if bar > 0 { bar + (12.0 * scale).round() as i32 } else { 0 };
        (asked(self.pad.0, reach.0), asked(self.pad.1, reach.1).max(top))
    }

    /// Whether what it makes is see-through anywhere: with nothing
    /// behind the picture, or with the corners of the whole rounded.
    pub fn is_clear(&self) -> bool {
        self.backdrop == Backdrop::None || self.radius > 0
    }
}

/// How far a point, counted from the middle of a rectangle with rounded
/// corners, is from its edge.
pub(crate) fn round_box(x: f32, y: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let (across, down) = (x.abs() - (half_w - radius), y.abs() - (half_h - radius));
    across.max(0.0).hypot(down.max(0.0)) + across.max(down).min(0.0) - radius
}

/// Sets a picture in its frame. The picture has in the top byte of each
/// pixel how opaque it is; so has what comes back, which is see-through
/// round the picture where there is no backdrop. `round` is the radius
/// of the picture's corners, which its shadow has too.
pub fn set(frame: Frame, picture: &[u32], width: i32, height: i32, round: i32, scale: f32, photo: Option<&Photo>) -> (i32, i32, Vec<u32>) {
    let room = frame.room(scale);
    let (wide, high) = (width + 2 * room.0, height + 2 * room.1);
    let outer = ((frame.radius.max(0) as f32 * scale).round()).min(wide.min(high) as f32 / 2.0);
    let mut out = vec![0u32; (wide * high) as usize];
    let radius = (round as f32).max(2.0 * scale);
    // In a window, its bar is over the picture, in the room there; and the
    // shadow and the rounding of the corners are the window's, bar and all.
    // Its bar is dark over a dark picture, as an app in dark mode has it.
    let bar = frame.window.bar(scale);
    let corner = if bar > 0 { radius.max(10.0 * scale) } else { radius };
    let (box_x, box_y, box_w, box_h) = (room.0 as f32, (room.1 - bar) as f32, width as f32, (height + bar) as f32);
    let (half_w, half_h) = (box_w / 2.0, box_h / 2.0);
    let dark = bar_dark(frame, picture, width, height);
    out.par_chunks_mut(wide as usize).enumerate().for_each(|(y, row)| {
        let y = y as i32;
        for (x, pixel) in row.iter_mut().enumerate() {
            let x = x as i32;
            // What is behind: a color between the two, by how far the
            // pixel is from one corner towards the other, with a little
            // noise so that the steps between colors do not show.
            let behind = match frame.backdrop {
                Backdrop::None => None,
                Backdrop::Gradient(which) => Some(shade(which, (x as f32 + 0.5) / wide as f32, (y as f32 + 0.5) / high as f32, ((x * 7 + y * 13) % 16) as f32 / 16.0)),
                // The photograph, as large as it takes to cover, and its
                // middle in the middle.
                Backdrop::Photo => photo.filter(|photo| photo.width > 0 && photo.height > 0).map(|photo| {
                    let times = (wide as f32 / photo.width as f32).max(high as f32 / photo.height as f32);
                    let sx = ((x as f32 - wide as f32 / 2.0) / times + photo.width as f32 / 2.0).clamp(0.0, photo.width as f32 - 1.0) as i32;
                    let sy = ((y as f32 - high as f32 / 2.0) / times + photo.height as f32 / 2.0).clamp(0.0, photo.height as f32 - 1.0) as i32;
                    photo.pixels[(sy * photo.width + sx) as usize] & 0xFF_FFFF
                }),
            };
            // The shadows here, as CSS casts them: each the picture's
            // shape moved, grown, and blurred as by a Gaussian, whose
            // edge falls off as the error function does; the first of
            // them over the rest. What comes of them all is a color,
            // made fainter by how opaque it is, and how opaque.
            let (mut cast, mut cover) = ([0.0f32; 3], 0.0f32);
            for layer in frame.shadow.layers().iter().rev() {
                let (dx, dy, grow) = (f32::from(layer.x) * scale, f32::from(layer.y) * scale, f32::from(layer.spread) * scale);
                let (wide_half, high_half) = ((half_w + grow).max(0.0), (half_h + grow).max(0.0));
                let away = round_box(x as f32 + 0.5 - box_x - half_w - dx, y as f32 + 0.5 - box_y - half_h - dy, wide_half, high_half, (corner + grow).clamp(0.0, wide_half.min(high_half)));
                let deviation = f32::from(layer.blur.max(0)) * scale / 2.0;
                let near = if deviation < 0.5 { (0.5 - away).clamp(0.0, 1.0) } else { 0.5 * (1.0 - erf(away / (deviation * std::f32::consts::SQRT_2))) };
                let alpha = near * (layer.color >> 24) as f32 / 255.0;
                for (part, shift) in cast.iter_mut().zip([16, 8, 0]) {
                    *part = (layer.color >> shift & 0xFF) as f32 * alpha + *part * (1.0 - alpha);
                }
                cover = alpha + cover * (1.0 - alpha);
            }
            let whole = |parts: [f32; 3]| parts.iter().zip([16, 8, 0]).fold(0, |color, (part, shift)| color | (part.round().clamp(0.0, 255.0) as u32) << shift);
            let (mut color, mut opaque) = match behind {
                Some(color) => (color, 255),
                None => (0, 0),
            };
            if cover > 0.0 {
                (color, opaque) = match behind {
                    Some(color) => (whole([0, 1, 2].map(|at| ((color >> [16, 8, 0][at] & 0xFF) as f32) * (1.0 - cover) + cast[at])), 255),
                    None => (whole(cast.map(|part| part / cover)), (cover * 255.0).round() as u32),
                };
            }
            // And the picture over it, where the pixel is in it; or the
            // window's bar. In a window, nothing of either outside the
            // curve of its corners.
            let (px, py) = (x - room.0, y - room.1);
            let shown = if px >= 0 && py >= 0 && px < width && py < height {
                Some(picture[(py * width + px) as usize])
            } else if bar > 0 && px >= 0 && px < width && py < 0 && py >= -bar {
                Some(0xFF00_0000 | window_bar(frame.window, dark, px as f32 + 0.5, (py + bar) as f32 + 0.5, width as f32, bar as f32, scale))
            } else {
                None
            };
            if let Some(over) = shown {
                let mut cover = over >> 24;
                if bar > 0 {
                    let inside = round_box(x as f32 + 0.5 - box_x - half_w, y as f32 + 0.5 - box_y - half_h, half_w, half_h, corner);
                    cover = (cover as f32 * (0.5 - inside).clamp(0.0, 1.0)) as u32;
                }
                color = mix(color, over & 0xFF_FFFF, cover + (cover >> 7));
                opaque = opaque + (255 - opaque) * cover / 255;
            }
            // The corners of the whole, rounded: nothing outside the
            // curve of them.
            if outer > 0.0 {
                let away = round_box(x as f32 + 0.5 - wide as f32 / 2.0, y as f32 + 0.5 - high as f32 / 2.0, wide as f32 / 2.0, high as f32 / 2.0, outer);
                opaque = (opaque as f32 * (0.5 - away).clamp(0.0, 1.0)) as u32;
            }
            *pixel = opaque << 24 | color;
        }
    });
    (wide, high, out)
}

/// Whether the bar of a picture's window is dark: a terminal's always,
/// and any other's over a picture dark along its top.
fn bar_dark(frame: Frame, picture: &[u32], width: i32, height: i32) -> bool {
    frame.window == Window::Terminal || (frame.window != Window::None && darkish(picture, width, height))
}

/// What a window is called, written in its bar, in a picture `set` made
/// of a picture `width` by `height`: in the middle of a Mac window's or a
/// terminal's, in the address of a browser's, and at the left of a
/// desktop's; cut short with an ellipsis where it would not fit.
pub fn title(frame: Frame, framed: &mut [u32], wide: i32, picture: &[u32], (width, height): (i32, i32), scale: f32, title: &str, text: &mut crate::text::Text) {
    let bar = frame.window.bar(scale);
    let title = title.trim();
    if bar == 0 || title.is_empty() {
        return;
    }
    let dark = bar_dark(frame, picture, width, height);
    let room = frame.room(scale);
    let (left, top) = (room.0, room.1 - bar);
    let px = ((12.5 * scale).round() as u32).max(8);
    let high = framed.len() as i32 / wide.max(1);
    let mut canvas = crate::canvas::Canvas::new(framed, wide, high);
    canvas.set_clip(crate::canvas::Rect::new(left, top, width, bar));
    // As much of it as fits in so much room.
    let mut fitted = |most: f32| -> String {
        let most = most.max(0.0) as i32;
        if text.width(title, px) <= most {
            return title.to_owned();
        }
        let mut letters: Vec<char> = title.chars().collect();
        while !letters.is_empty() && text.width(&format!("{}\u{2026}", letters.iter().collect::<String>()), px) > most {
            letters.pop();
        }
        format!("{}\u{2026}", letters.iter().collect::<String>())
    };
    let ink = if dark { 0xC9CCD3 } else { 0x4A4D57 };
    let middle = top + bar / 2;
    match frame.window {
        Window::Browser => {
            // Over the faint line that stands for an address.
            let half = (width as f32 * 0.25).min(240.0 * scale).max(40.0 * scale);
            let says = fitted(2.0 * half - 24.0 * scale);
            let wipe = (half * 0.45).max(12.0 * scale) as i32 + 2;
            canvas.fill(left + width / 2 - wipe, middle - (3.0 * scale).ceil() as i32 - 1, 2 * wipe, (6.0 * scale).ceil() as i32 + 2, if dark { 0x3A3C42 } else { 0xFFFFFF });
            text.draw_centered(&mut canvas, left + width / 2, middle, px, ink, &says);
        }
        Window::Mac | Window::Terminal => {
            let says = fitted(width as f32 - 160.0 * scale);
            text.draw_centered(&mut canvas, left + width / 2, middle, px, ink, &says);
        }
        Window::Desktop => {
            let says = fitted(width as f32 - 130.0 * scale);
            let line = crate::text::Text::line_height(px);
            text.draw(&mut canvas, left + (12.0 * scale) as i32, middle - line / 2, px, ink, &says);
        }
        Window::None => {}
    }
    // Drawing on it left its pixels without how opaque they are, which
    // in the bar is wholly.
    for y in top.max(0)..(top + bar).min(high) {
        for pixel in &mut framed[(y * wide + left.max(0)) as usize..(y * wide + (left + width).min(wide)) as usize] {
            *pixel |= 0xFF00_0000;
        }
    }
}

/// Whether a picture is dark along its top, where a window's bar would
/// meet it: a few hundred of its pixels looked at.
pub(crate) fn darkish(picture: &[u32], width: i32, height: i32) -> bool {
    let rows = height.clamp(1, 24);
    let step = ((width * rows) / 300).max(1) as usize;
    let looked: Vec<u32> = picture[..(width * rows) as usize].iter().step_by(step).map(|pixel| ((pixel >> 16 & 0xFF) * 3 + (pixel >> 8 & 0xFF) * 6 + (pixel & 0xFF)) / 10).collect();
    looked.iter().sum::<u32>() < 110 * looked.len().max(1) as u32
}

/// A pixel of a window's bar, `x` along it and `y` down it, of a bar so
/// wide and tall: its ground, and on it the three lights at its left, an
/// address in a pill, or three buttons at its right.
pub(crate) fn window_bar(window: Window, dark: bool, x: f32, y: f32, wide: f32, high: f32, scale: f32) -> u32 {
    let ground = if dark { 0x2B2D31 } else { 0xE8E9EC };
    let middle = high / 2.0;
    // Something round, or a line, drawn smooth: how much of the pixel it
    // covers, from how far the pixel is from its edge.
    let mut color = ground;
    let mut over = |away: f32, ink: u32| {
        let cover = (0.5 - away).clamp(0.0, 1.0);
        if cover > 0.0 {
            color = mix(color, ink, (cover * 256.0) as u32);
        }
    };
    let dot = |cx: f32| (x - cx).hypot(y - middle) - 6.0 * scale;
    match window {
        Window::Browser | Window::Mac | Window::Terminal => {
            for (at, light) in [(16.0, 0xFF5F57), (36.0, 0xFEBC2E), (56.0, 0x28C840)] {
                over(dot(at * scale), light);
            }
            if window == Window::Browser {
                // The address: a pill as wide as half the window, at most,
                // in its middle, with a faint line for what is typed in it.
                let half = (wide * 0.25).min(240.0 * scale).max(40.0 * scale);
                let pill = round_box(x - wide / 2.0, y - middle, half, high * 0.3, high * 0.3);
                over(pill, if dark { 0x3A3C42 } else { 0xFFFFFF });
                let typed = round_box(x - wide / 2.0, y - middle, (half * 0.45).max(12.0 * scale), 2.0 * scale, 2.0 * scale);
                over(typed, if dark { 0x6E7079 } else { 0xC2C4CC });
            }
        }
        Window::Desktop => {
            let ink = if dark { 0xC9CCD3 } else { 0x4A4D57 };
            let size = 5.0 * scale;
            let (close, grow, shrink) = (wide - 22.0 * scale, wide - 56.0 * scale, wide - 90.0 * scale);
            let line = |from: (f32, f32), to: (f32, f32)| {
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                let along = (((x - from.0) * dx + (y - from.1) * dy) / (dx * dx + dy * dy).max(1e-3)).clamp(0.0, 1.0);
                (x - from.0 - dx * along).hypot(y - from.1 - dy * along) - 0.7 * scale
            };
            over(line((close - size, middle - size), (close + size, middle + size)).min(line((close + size, middle - size), (close - size, middle + size))), ink);
            over((round_box(x - grow, y - middle, size, size, 0.0).abs()) - 0.7 * scale, ink);
            over(line((shrink - size, middle), (shrink + size, middle)), ink);
        }
        Window::None => {}
    }
    color
}

/// A photograph from a file: a PNG or a JPEG.
pub fn open(bytes: &[u8]) -> Option<Photo> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().ok()?;
        let mut data = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut data).ok()?;
        let each = info.color_type.samples();
        let pixels = data[..info.buffer_size()].chunks_exact(each).map(|pixel| match each {
            1 | 2 => u32::from(pixel[0]) * 0x010101,
            _ => u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2]),
        });
        return Some(Photo { width: info.width as i32, height: info.height as i32, pixels: pixels.collect() });
    }
    let mut decoder = zune_jpeg::JpegDecoder::new(bytes);
    let data = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (width, height) = (i32::from(info.width), i32::from(info.height));
    let each = data.len() / (width.max(1) * height.max(1)) as usize;
    let pixels = data.chunks_exact(each.max(1)).map(|pixel| match each {
        1 => u32::from(pixel[0]) * 0x010101,
        _ => u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2]),
    });
    Some(Photo { width, height, pixels: pixels.collect() })
}

/// A photograph nobody chose: a different one each time, of about a
/// size, from Lorem Picsum, which hands them out without being asked
/// who is asking. Fetched by `curl`, which is nearly everywhere, where
/// a program that spoke to the network itself would be several times
/// the size of this one. Nothing, where there is no network or no curl.
pub fn random(width: i32, height: i32) -> Option<Photo> {
    // Asked for with the time in it, so that nothing between here and
    // there takes it for the one it handed over last.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_millis());
    let from = format!("https://picsum.photos/{}/{}.jpg?random={now}", width.clamp(64, 2560), height.clamp(64, 2560));
    let fetched = Command::new("curl").args(["-sL", "--max-time", "12", &from]).output().ok()?;
    if !fetched.status.success() {
        return None;
    }
    open(&fetched.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_has_its_bar_over_the_picture_light_or_dark_by_the_picture() {
        let (width, height) = (300, 200);
        let light = vec![0xFFF0F0F0u32; (width * height) as usize];
        let frame = Frame { pad: (40, 50), window: Window::Browser, ..Frame::default() };
        let (wide, high, out) = set(frame, &light, width, height, 0, 1.0, None);
        assert_eq!((wide, high), (width + 80, height + 100));
        // The bar is right over the picture: its ground, its lights at
        // the left, and the address in the middle.
        let at = |x: i32, y: i32| out[(y * wide + x) as usize] & 0xFF_FFFF;
        assert_eq!(at(40 + 100, 50 - 25), 0xE8E9EC);
        assert_eq!(at(40 + 16, 50 - 15), 0xFF5F57);
        assert_eq!(at(40 + 200, 50 - 15), 0xFFFFFF);
        assert_eq!(out[(10 * wide + 10) as usize] >> 24, 0, "nothing behind it but the window");
        // Over a dark picture, a dark bar; and a terminal is dark anyway.
        let dark = vec![0xFF101010u32; (width * height) as usize];
        let (_, _, out) = set(frame, &dark, width, height, 0, 1.0, None);
        assert_eq!(out[((50 - 25) * wide + 140) as usize] & 0xFF_FFFF, 0x2B2D31);
        let (_, _, out) = set(Frame { window: Window::Terminal, ..frame }, &light, width, height, 0, 1.0, None);
        assert_eq!(out[((50 - 25) * wide + 140) as usize] & 0xFF_FFFF, 0x2B2D31);
        // With no room asked for, the bar makes its own.
        assert!(Frame { window: Window::Mac, ..Frame::default() }.room(1.0).1 >= 30);
        // What the window is called, in the middle of a Mac window's bar,
        // in ink on its ground, the bar still wholly opaque.
        let mac = Frame { window: Window::Mac, ..frame };
        let (wide, _, mut out) = set(mac, &light, width, height, 0, 1.0, None);
        let bar = |out: &[u32]| (130..170).flat_map(|x| (30..45).map(move |y| (x, y))).filter(|&(x, y)| out[(y * wide + 40 + x) as usize] & 0xFF_FFFF != 0xE8E9EC).count();
        assert_eq!(bar(&out), 0);
        title(mac, &mut out, wide, &light, (width, height), 1.0, "Notes", &mut crate::text::Text::load());
        assert!(bar(&out) > 20 && out[(35 * wide + 140) as usize] >> 24 == 255);
    }

    #[test]
    fn a_plain_frame_is_the_picture_and_a_backdrop_is_all_round_it() {
        let picture = vec![0xFF11_2233u32; 4 * 2];
        let (wide, high, plain) = set(Frame::default(), &picture, 4, 2, 0, 1.0, None);
        assert_eq!((wide, high, plain[0]), (4, 2, 0xFF11_2233));
        assert!(Frame::default().is_plain());

        let slate = named("slate").unwrap();
        let frame = Frame { backdrop: Backdrop::Gradient(slate), pad: (PADS[1], PADS[1]), radius: 0, shadow: Shadow::NONE, window: Window::None };
        let (wide, high, framed) = set(frame, &picture, 4, 2, 0, 1.0, None);
        assert_eq!((wide, high), (4 + 64, 2 + 64));
        // Opaque everywhere; the picture in the middle of it, and at
        // the two far corners the two colors, near enough.
        assert!(framed.iter().all(|pixel| pixel >> 24 == 255));
        assert_eq!(framed[(32 * wide + 32) as usize], 0xFF11_2233);
        let near = |pixel: u32, color: u32| (0..3).all(|channel| ((pixel >> (channel * 8) & 0xFF) as i32 - (color >> (channel * 8) & 0xFF) as i32).abs() < 8);
        assert!(near(framed[0], 0x334155) && near(framed[framed.len() - 1], 0x0F172A));
        // Every backdrop is called something no other is, and is a
        // color everywhere; a mesh is its patches' colors near them.
        for (which, (name, _)) in BACKDROPS.iter().enumerate() {
            assert_eq!(named(&name.to_uppercase()), Some(which));
        }
        assert!(FIRST.iter().all(|name| named(name).is_some()));
        assert!(near(shade(named("aurora").unwrap(), 0.15, 0.2, 0.5), 0x3B82F6));
        assert_eq!(shade(named("ink").unwrap(), 0.3, 0.7, 0.9), 0x0B0B0F);
    }

    #[test]
    fn a_shadow_makes_its_own_room_and_is_darkest_under_the_picture() {
        let mut picture = vec![0x00FF_FFFFu32; 200 * 100];
        crate::canvas::round_alpha(&mut picture, 200, 8);
        // Tailwind's 2xl: 25 down, 50 blurred, 12 drawn in all round,
        // so that beside the picture it is all but gone, and under it
        // reaches 25 - 12 + 75 out.
        let frame = Frame { backdrop: Backdrop::None, pad: (0, 0), radius: 0, shadow: Shadow::named("2xl").unwrap(), window: Window::None };
        let (wide, high, framed) = set(frame, &picture, 200, 100, 8, 1.0, None);
        let room = frame.room(1.0);
        assert_eq!((room, wide, high), ((63, 88), 200 + 2 * 63, 100 + 2 * 88));
        let alpha = |x: i32, y: i32| framed[(y * wide + x) as usize] >> 24;
        // Nothing at the far corner; something just under the bottom
        // edge, more than at the same distance above the top one, and
        // no more than a quarter of the way to black; and the picture
        // itself whole.
        let (under, over) = (alpha(wide / 2, room.1 + 100 + 6), alpha(wide / 2, room.1 - 6));
        assert_eq!(alpha(0, 0), 0);
        assert!(under > over && under > 20 && under <= 64, "{under} {over}");
        assert_eq!(framed[((room.1 + 50) * wide + room.0 + 100) as usize], 0xFFFF_FFFF);
        // Rounded corners of the picture are still see-through to the
        // shadow, not to nothing; and on a backdrop it darkens it.
        assert!(alpha(room.0, room.1 + 99) > 0 && alpha(room.0, room.1 + 99) < 255);
        let on = Frame { backdrop: Backdrop::Gradient(named("white").unwrap()), ..frame };
        let (wide, _, framed) = set(on, &picture, 200, 100, 8, 1.0, None);
        let pixel = framed[((room.1 + 100 + 6) * wide + wide / 2) as usize];
        assert!(pixel >> 24 == 255 && pixel & 0xFF < 0xF8 && framed[0] & 0xFF_FFFF == 0xFF_FFFF);
    }

    #[test]
    fn a_shadow_is_one_of_tailwinds_by_its_name_or_as_css_says_it() {
        let lg = Shadow::named("lg").unwrap();
        assert_eq!((lg.count, lg.name(), Shadow::named("shadow-LG"), Shadow::named("huge")), (2, Some("lg"), Some(lg), None));
        // Said as CSS, it is what Tailwind's is, with commas inside its
        // colors and without.
        assert_eq!(Shadow::parse("0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1)").unwrap(), lg);
        assert_eq!(Shadow::parse(&lg.css()).unwrap(), lg);
        let colored = Shadow::parse("2px -4px 0.5rem #3E63DD80").unwrap();
        assert_eq!(colored.layers(), [Layer { x: 2, y: -4, blur: 8, spread: 0, color: 0x803E63DD }]);
        assert_eq!(Shadow::parse("0 0 12px white").unwrap().layers()[0].color, 0xFFFF_FFFF);
        assert_eq!(Shadow::parse("none").unwrap(), Shadow::NONE);
        for wrong in ["5px", "0 0 12px inset", "0 0 -3px black", "1 2 3 4 5", "0 0 1px blue", "a, b, c, d, e"] {
            assert!(Shadow::parse(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn a_png_is_read_as_a_photograph() {
        let png = crate::output::encode(2, 1, &[0xFF0000, 0x0000FF], false).unwrap();
        let photo = open(&png).unwrap();
        assert_eq!((photo.width, photo.height, photo.pixels), (2, 1, vec![0xFF0000, 0x0000FF]));
        assert!(open(b"not a picture").is_none());
    }
}
