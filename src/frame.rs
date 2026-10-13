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

/// The shadow under the picture: four of them, each further down, more
/// spread and a little darker than the last, which together fall off
/// the way a real one does, close and sharp at the edge and wide and
/// faint beyond it. How far down, how spread, and how dark out of 1, on
/// a screen 1080 high.
const SHADOWS: [(f32, f32, f32); 4] = [(1.0, 3.0, 0.10), (4.0, 12.0, 0.10), (12.0, 32.0, 0.13), (28.0, 72.0, 0.17)];

/// A picture to stand behind another: its size, and its pixels.
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

/// How the picture is set: on what, with how much room, and whether
/// with a shadow.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Frame {
    pub backdrop: Backdrop,
    /// How much room there is beside the picture, and above and below
    /// it, in pixels of a screen 1080 high: any amount of each.
    pub pad: (i32, i32),
    /// How round the corners of the whole are, in the same pixels.
    pub radius: i32,
    pub shadow: bool,
}

impl Frame {
    /// Whether it does anything at all to the picture.
    pub fn is_plain(&self) -> bool {
        self.backdrop == Backdrop::None && self.pad == (0, 0) && !self.shadow
    }

    /// The room beside the picture, and above and below it, in pixels
    /// at a scale. A shadow has to have somewhere to fall, whatever the
    /// room asked for.
    pub fn room(&self, scale: f32) -> (i32, i32) {
        let least = if self.shadow { (56.0 * scale).round() as i32 } else { 0 };
        let asked = |pad: i32| ((pad.max(0) as f32 * scale).round() as i32).max(least);
        (asked(self.pad.0), asked(self.pad.1))
    }

    /// Whether what it makes is see-through anywhere: with nothing
    /// behind the picture, or with the corners of the whole rounded.
    pub fn is_clear(&self) -> bool {
        self.backdrop == Backdrop::None || self.radius > 0
    }
}

/// How far a point, counted from the middle of a rectangle with rounded
/// corners, is from its edge.
fn round_box(x: f32, y: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
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
    let (half_w, half_h, radius) = (width as f32 / 2.0, height as f32 / 2.0, (round as f32).max(2.0 * scale));
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
            // How dark the shadow is here: each of the four taken off
            // what light the ones before it left.
            let mut light = 1.0f32;
            if frame.shadow {
                for (down, spread, dark) in SHADOWS {
                    let (down, spread) = (down * scale, spread * scale);
                    let away = round_box(x as f32 + 0.5 - room.0 as f32 - half_w, y as f32 + 0.5 - room.1 as f32 - half_h - down, half_w, half_h, radius);
                    let near = (0.5 - away / spread).clamp(0.0, 1.0);
                    light *= 1.0 - dark * near * near * (3.0 - 2.0 * near);
                }
            }
            let shade = ((1.0 - light) * 256.0) as u32;
            let (mut color, mut opaque) = match behind {
                Some(color) => (mix(color, 0, shade), 255),
                None => (0, shade.min(255)),
            };
            // And the picture over it, where the pixel is in it.
            let (px, py) = (x - room.0, y - room.1);
            if px >= 0 && py >= 0 && px < width && py < height {
                let over = picture[(py * width + px) as usize];
                let cover = over >> 24;
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
    fn a_plain_frame_is_the_picture_and_a_backdrop_is_all_round_it() {
        let picture = vec![0xFF11_2233u32; 4 * 2];
        let (wide, high, plain) = set(Frame::default(), &picture, 4, 2, 0, 1.0, None);
        assert_eq!((wide, high, plain[0]), (4, 2, 0xFF11_2233));
        assert!(Frame::default().is_plain());

        let slate = named("slate").unwrap();
        let frame = Frame { backdrop: Backdrop::Gradient(slate), pad: (PADS[1], PADS[1]), radius: 0, shadow: false };
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
        let frame = Frame { backdrop: Backdrop::None, pad: (0, 0), radius: 0, shadow: true };
        let (wide, high, framed) = set(frame, &picture, 200, 100, 8, 1.0, None);
        assert_eq!((wide, high), (200 + 112, 100 + 112));
        let alpha = |x: i32, y: i32| framed[(y * wide + x) as usize] >> 24;
        // Nothing at the far corner; something just under the bottom
        // edge, more than at the same distance above the top one; and
        // the picture itself whole.
        assert_eq!(alpha(0, 0), 0);
        assert!(alpha(wide / 2, 56 + 100 + 6) > alpha(wide / 2, 56 - 6) && alpha(wide / 2, 56 + 100 + 6) > 40);
        assert_eq!(framed[(100 * wide + 150) as usize], 0xFFFF_FFFF);
        // Rounded corners of the picture are still see-through to the
        // shadow, not to nothing.
        assert!(alpha(56, 56) > 0 && alpha(56, 56) < 255);
    }

    #[test]
    fn a_png_is_read_as_a_photograph() {
        let png = crate::output::encode(2, 1, &[0xFF0000, 0x0000FF], false).unwrap();
        let photo = open(&png).unwrap();
        assert_eq!((photo.width, photo.height, photo.pixels), (2, 1, vec![0xFF0000, 0x0000FF]));
        assert!(open(b"not a picture").is_none());
    }
}
