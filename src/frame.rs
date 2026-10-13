//! The picture set on something: a backdrop of two colors run into
//! each other, or a photograph, with room round it and a shadow under
//! it. This is the picture as it is shown off, where the plain one is
//! the picture as it is.

use std::process::Command;

use rayon::prelude::*;

use crate::canvas::mix;

/// The backdrops there are of two colors: what each is called, the
/// color at its top-left and the one at its bottom-right.
pub const GRADIENTS: [(&str, u32, u32); 8] = [
    ("Dusk", 0x4F46E5, 0xEC4899),
    ("Ocean", 0x06B6D4, 0x3B82F6),
    ("Sunset", 0xF97316, 0xEC4899),
    ("Mint", 0x34D399, 0x06B6D4),
    ("Peach", 0xFDBA74, 0xF9A8D4),
    ("Violet", 0x8E4EC6, 0x3E63DD),
    ("Slate", 0x334155, 0x0F172A),
    ("Paper", 0xF8FAFC, 0xE2E8F0),
];

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
    /// One of `GRADIENTS`.
    Gradient(usize),
    /// A photograph, handed over with it.
    Photo,
}

/// How the picture is set: on what, with how much room, and whether
/// with a shadow.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Frame {
    pub backdrop: Backdrop,
    /// How much room there is round the picture, in pixels of a screen
    /// 1080 high: any amount, of which `PADS` are the usual ones.
    pub pad: i32,
    pub shadow: bool,
}

impl Frame {
    /// Whether it does anything at all to the picture.
    pub fn is_plain(&self) -> bool {
        self.backdrop == Backdrop::None && self.pad == 0 && !self.shadow
    }

    /// The room round the picture, in pixels at a scale. A shadow has
    /// to have somewhere to fall, whatever the room asked for.
    pub fn room(&self, scale: f32) -> i32 {
        let asked = (self.pad.max(0) as f32 * scale).round() as i32;
        if self.shadow { asked.max((56.0 * scale).round() as i32) } else { asked }
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
    let (wide, high) = (width + 2 * room, height + 2 * room);
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
                Backdrop::Gradient(which) => {
                    let (_, from, to) = GRADIENTS[which.min(GRADIENTS.len() - 1)];
                    let along = (x as f32 / wide as f32 + y as f32 / high as f32) * 128.0;
                    let noise = ((x * 7 + y * 13) % 16) as f32 / 16.0;
                    Some(mix(from, to, (along + noise).clamp(0.0, 256.0) as u32))
                }
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
                    let away = round_box(x as f32 + 0.5 - room as f32 - half_w, y as f32 + 0.5 - room as f32 - half_h - down, half_w, half_h, radius);
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
            let (px, py) = (x - room, y - room);
            if px >= 0 && py >= 0 && px < width && py < height {
                let over = picture[(py * width + px) as usize];
                let cover = over >> 24;
                color = mix(color, over & 0xFF_FFFF, cover + (cover >> 7));
                opaque = opaque + (255 - opaque) * cover / 255;
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
    let from = format!("https://picsum.photos/{}/{}.jpg", width.clamp(64, 2560), height.clamp(64, 2560));
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

        let frame = Frame { backdrop: Backdrop::Gradient(6), pad: PADS[1], shadow: false };
        let (wide, high, framed) = set(frame, &picture, 4, 2, 0, 1.0, None);
        assert_eq!((wide, high), (4 + 64, 2 + 64));
        // Opaque everywhere; the picture in the middle of it, and at
        // the two far corners the two colors, near enough.
        assert!(framed.iter().all(|pixel| pixel >> 24 == 255));
        assert_eq!(framed[(32 * wide + 32) as usize], 0xFF11_2233);
        let near = |pixel: u32, color: u32| (0..3).all(|channel| ((pixel >> (channel * 8) & 0xFF) as i32 - (color >> (channel * 8) & 0xFF) as i32).abs() < 8);
        assert!(near(framed[0], GRADIENTS[6].1) && near(framed[framed.len() - 1], GRADIENTS[6].2));
    }

    #[test]
    fn a_shadow_makes_its_own_room_and_is_darkest_under_the_picture() {
        let mut picture = vec![0x00FF_FFFFu32; 200 * 100];
        crate::canvas::round_alpha(&mut picture, 200, 8);
        let frame = Frame { backdrop: Backdrop::None, pad: 0, shadow: true };
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
