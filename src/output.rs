//! The picture as a file: a PNG, and a name with the time in it.

use std::ffi::CString;
use std::io::Write;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::canvas::Rect;

/// A picture as a PNG. Packed quickly rather than small: it is a
/// screenshot, wanted now, and the difference is a few per cent.
///
/// With `alpha`, the top byte of each pixel says how opaque it is, and
/// goes into the PNG: that is how a picture has rounded corners.
pub fn encode(width: u32, height: u32, pixels: &[u32], alpha: bool) -> Result<Vec<u8>, String> {
    // PNG has the colors of a pixel the other way round, and no fourth
    // byte unless it means something; turning 8 million of them over is
    // worth every core.
    let each = if alpha { 4 } else { 3 };
    let mut bytes = vec![0u8; pixels.len() * each];
    bytes.par_chunks_mut(each << 12).zip(pixels.par_chunks(1 << 12)).for_each(|(bytes, pixels)| {
        for (out, pixel) in bytes.chunks_exact_mut(each).zip(pixels) {
            out.copy_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8, (pixel >> 24) as u8][..each]);
        }
    });
    let mut png = Vec::with_capacity(bytes.len() / 4);
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(if alpha { png::ColorType::Rgba } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer.write_image_data(&bytes).map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(png)
}

/// What kind of file a picture is saved as.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Format {
    #[default]
    Png,
    Jpeg,
    Webp,
}

impl Format {
    /// By its name, or by what a file's name ends in.
    pub fn named(name: &str) -> Option<Format> {
        match name.to_ascii_lowercase().as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" => Some(Format::Jpeg),
            "webp" => Some(Format::Webp),
            _ => None,
        }
    }

    /// What the name of a file of this kind ends in.
    pub fn ending(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
        }
    }
}

/// A picture as a file of one kind or another. A JPEG is as good as
/// `quality` says, of 100, and has nothing see-through in it: what
/// would be is white. A WebP loses nothing, as a PNG loses nothing, and
/// is smaller.
pub fn pack(format: Format, quality: u8, width: u32, height: u32, pixels: &[u32], alpha: bool) -> Result<Vec<u8>, String> {
    let bytes = |each: usize, of: &(dyn Fn(u32) -> [u8; 4] + Sync)| {
        let mut bytes = vec![0u8; pixels.len() * each];
        bytes.par_chunks_mut(each << 12).zip(pixels.par_chunks(1 << 12)).for_each(|(bytes, pixels)| {
            for (out, pixel) in bytes.chunks_exact_mut(each).zip(pixels) {
                out.copy_from_slice(&of(*pixel)[..each]);
            }
        });
        bytes
    };
    let plain = |pixel: u32| [(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8, (pixel >> 24) as u8];
    match format {
        Format::Png => encode(width, height, pixels, alpha),
        Format::Jpeg => {
            let (wide, high) = (u16::try_from(width).map_err(|_| "too wide for a JPEG")?, u16::try_from(height).map_err(|_| "too tall for a JPEG")?);
            let on_white = |pixel: u32| {
                let opaque = pixel >> 24;
                plain(pixel).map(|part| ((part as u32 * opaque + 255 * (255 - opaque)) / 255) as u8)
            };
            let bytes = if alpha { bytes(3, &on_white) } else { bytes(3, &plain) };
            let mut out = Vec::with_capacity(bytes.len() / 8);
            let encoder = jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100));
            encoder.encode(&bytes, wide, high, jpeg_encoder::ColorType::Rgb).map_err(|error| error.to_string())?;
            Ok(out)
        }
        Format::Webp => {
            let each = if alpha { 4 } else { 3 };
            let bytes = bytes(each, &plain);
            let mut out = Vec::with_capacity(bytes.len() / 4);
            let kind = if alpha { image_webp::ColorType::Rgba8 } else { image_webp::ColorType::Rgb8 };
            image_webp::WebPEncoder::new(&mut out).encode(&bytes, width, height, kind).map_err(|error| error.to_string())?;
            Ok(out)
        }
    }
}

/// The time now, set out as `strftime` would: `%Y-%m-%d` and the like.
pub fn dated(template: &str) -> String {
    let Ok(format) = CString::new(template) else { return template.to_owned() };
    let mut out = [0u8; 256];
    // SAFETY: the buffers are of the sizes said, and `tm` is filled in
    // before it is read.
    let written = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::zeroed::<libc::tm>();
        libc::localtime_r(&now, &mut tm);
        libc::strftime(out.as_mut_ptr().cast(), out.len(), format.as_ptr(), &tm)
    };
    String::from_utf8_lossy(&out[..written]).into_owned()
}

/// Writes a picture into a folder under a name made from the time, making
/// the folder if need be. A name already taken, by a picture of the same
/// second, has a number put after it.
pub fn save(dir: &Path, template: &str, ending: &str, png: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot make {}: {error}", dir.display()))?;
    let name = dated(template);
    for copy in 1.. {
        let path = dir.join(if copy == 1 { format!("{name}.{ending}") } else { format!("{name}-{copy}.{ending}") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(png).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot write {}: {error}", path.display())),
        }
    }
    unreachable!()
}

/// Where the area last taken is kept: `$HOTSHOT_STATE`, or `hotshot`
/// under `$XDG_STATE_HOME` or `~/.local/state`.
fn last_file() -> Option<PathBuf> {
    state_file("last")
}

/// A file of what hotshot keeps from one run to the next, by its name:
/// in `$HOTSHOT_STATE`, or `hotshot` under `$XDG_STATE_HOME` or
/// `~/.local/state`.
pub fn state_file(name: &str) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HOTSHOT_STATE") {
        return Some(PathBuf::from(dir).join(name));
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".local/state")))?;
    Some(base.join("hotshot").join(name))
}

/// The area the last picture was of, for `--again`.
pub fn last_area() -> Option<Rect> {
    let text = std::fs::read_to_string(last_file()?).ok()?;
    let numbers: Vec<i32> = text.split_whitespace().map(|number| number.parse().ok()).collect::<Option<_>>()?;
    match numbers[..] {
        [x, y, w, h] if w > 0 && h > 0 => Some(Rect::new(x, y, w, h)),
        _ => None,
    }
}

/// Keeps the area a picture was of, to be taken again.
pub fn remember_area(area: Rect) {
    let Some(file) = last_file() else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, format!("{} {} {} {}\n", area.x, area.y, area.w, area.h));
}

/// Keeps what it takes to come back to a picture and go on with it:
/// what was there before anything was drawn, as a PNG, and beside it
/// where it was on the screen and each shape drawn on it.
pub fn keep_for_reopening(area: Rect, pixels: &[u32], shapes: &str) {
    let (Some(picture), Some(notes)) = (state_file("reopen.png"), state_file("reopen")) else { return };
    if let Some(dir) = notes.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(png) = encode(area.w as u32, area.h as u32, pixels, false) {
        let _ = std::fs::write(picture, png);
        let _ = std::fs::write(notes, format!("{} {} {} {}\n{shapes}\n", area.x, area.y, area.w, area.h));
    }
}

/// What was kept of the last picture: where it was, what was there,
/// and the shapes that were on it.
pub fn kept_for_reopening() -> Option<(Rect, crate::frame::Photo, Vec<crate::shapes::Shape>)> {
    let notes = std::fs::read_to_string(state_file("reopen")?).ok()?;
    let photo = crate::frame::open(&std::fs::read(state_file("reopen.png")?).ok()?)?;
    let mut lines = notes.lines();
    let numbers: Vec<i32> = lines.next()?.split_whitespace().map(|number| number.parse().ok()).collect::<Option<_>>()?;
    let [x, y, w, h] = numbers[..] else { return None };
    if (w, h) != (photo.width, photo.height) {
        return None;
    }
    Some((Rect::new(x, y, w, h), photo, lines.filter_map(crate::shapes::Shape::read).collect()))
}

/// Writes a picture to a file by name, or to standard output for `-`.
pub fn save_as(path: &Path, png: &[u8]) -> Result<(), String> {
    if path == Path::new("-") {
        return std::io::stdout().write_all(png).map_err(|error| format!("cannot write the picture out: {error}"));
    }
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|error| format!("cannot make {}: {error}", dir.display()))?;
    }
    std::fs::write(path, png).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_png_has_the_pixels_it_was_made_from() {
        let pixels = [0xFF0000, 0x00FF00, 0x0000FF, 0x123456];
        let png = encode(2, 2, &pixels, false).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(&bytes[..12], &[255, 0, 0, 0, 255, 0, 0, 0, 255, 0x12, 0x34, 0x56]);
    }

    #[test]
    fn a_picture_is_packed_as_a_jpeg_or_a_webp_too() {
        assert_eq!((Format::named("JPG"), Format::named("webp"), Format::named("gif")), (Some(Format::Jpeg), Some(Format::Webp), None));
        // Half red and half see-through, which in a JPEG is white.
        let pixels: Vec<u32> = (0..64 * 64).map(|at| if at % 64 < 32 { 0xFFFF_0000 } else { 0 }).collect();
        let jpeg = pack(Format::Jpeg, 90, 64, 64, &pixels, true).unwrap();
        let mut decoder = zune_jpeg::JpegDecoder::new(&jpeg[..]);
        let bytes = decoder.decode().unwrap();
        assert_eq!(decoder.dimensions(), Some((64, 64)));
        let near = |at: usize, to: [u8; 3]| bytes[at * 3..at * 3 + 3].iter().zip(to).all(|(byte, to)| byte.abs_diff(to) < 24);
        assert!(near(32 * 64 + 8, [255, 0, 0]) && near(32 * 64 + 56, [255, 255, 255]));
        // A WebP is to the pixel, and what is see-through still is.
        let webp = pack(Format::Webp, 90, 64, 64, &pixels, true).unwrap();
        let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(&webp[..])).unwrap();
        let mut bytes = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.read_image(&mut bytes).unwrap();
        assert_eq!((decoder.dimensions(), &bytes[..4], &bytes[63 * 4..64 * 4]), ((64, 64), &[255, 0, 0, 255][..], &[0, 0, 0, 0][..]));
    }

    #[test]
    fn the_area_last_taken_is_kept_for_next_time() {
        let dir = std::env::temp_dir().join(format!("hotshot-state-{}", std::process::id()));
        std::env::set_var("HOTSHOT_STATE", &dir);
        assert_eq!(last_area(), None);
        remember_area(Rect::new(10, -20, 300, 400));
        assert_eq!(last_area(), Some(Rect::new(10, -20, 300, 400)));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_name_is_made_from_the_time_and_never_reused() {
        assert_eq!(dated("shot-%Y").len(), 9);
        let dir = std::env::temp_dir().join(format!("hotshot-test-{}", std::process::id()));
        let first = save(&dir, "same", "png", b"one").unwrap();
        let second = save(&dir, "same", "png", b"two").unwrap();
        assert_eq!(first.file_name().unwrap(), "same.png");
        assert_eq!(second.file_name().unwrap(), "same-2.png");
        assert_eq!(save(&dir, "same", Format::Jpeg.ending(), b"three").unwrap().file_name().unwrap(), "same.jpg");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
