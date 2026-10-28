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

/// What a note of hotshot's in a PNG is called.
const NOTE: &[u8] = b"hotshot";

/// The CRC that ends each chunk of a PNG, over its kind and what is in it.
fn crc(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { 0xEDB8_8320 ^ (crc >> 1) } else { crc >> 1 };
        }
    }
    !crc
}

/// A PNG with a note of hotshot's in it, as international text after its
/// header: what a capture of a window knows of where it came from, for
/// the canvas to read when it is opened there. Any other reader passes it
/// over.
pub fn noted(png: Vec<u8>, note: &str) -> Vec<u8> {
    // The signature, then the header: 8 bytes, and 25 of the header's.
    if note.is_empty() || png.len() < 33 || &png[12..16] != b"IHDR" {
        return png;
    }
    let mut data = NOTE.to_vec();
    data.extend_from_slice(&[0, 0, 0, 0, 0]);
    data.extend_from_slice(note.as_bytes());
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    let mut body = b"iTXt".to_vec();
    body.extend_from_slice(&data);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc(&body).to_be_bytes());
    let mut out = png[..33].to_vec();
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[33..]);
    out
}

/// The note of hotshot's in a PNG, if it has one: looked for in the
/// chunks before its pixels.
pub fn note_of(png: &[u8]) -> Option<String> {
    let mut at = 8;
    while at + 8 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().ok()?) as usize;
        let kind = &png[at + 4..at + 8];
        let data = png.get(at + 8..at + 8 + length)?;
        if kind == b"IDAT" {
            return None;
        }
        if kind == b"iTXt" && data.starts_with(NOTE) && data.get(NOTE.len()) == Some(&0) {
            // After the keyword: whether and how it is packed, its
            // language, and its keyword translated, the last two empty.
            let mut rest = &data[NOTE.len() + 3..];
            for _ in 0..2 {
                let end = rest.iter().position(|&byte| byte == 0)?;
                rest = &rest[end + 1..];
            }
            return String::from_utf8(rest.to_vec()).ok();
        }
        at += 12 + length;
    }
    None
}

/// What kind of file a picture is saved as. JPEG XL and WebP each come
/// two ways, every pixel kept or smaller and nearly the same, listed as
/// two kinds since they are chosen between as two.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Format {
    #[default]
    Png,
    Jpeg,
    Webp,
    WebpLossless,
    Avif,
    Jxl,
    JxlLossless,
}

impl Format {
    /// In the order the lists show them: those that keep every pixel,
    /// then those that are smaller and nearly the same.
    pub const ALL: [Format; 7] = [Format::Png, Format::JxlLossless, Format::WebpLossless, Format::Jxl, Format::Avif, Format::Webp, Format::Jpeg];

    /// By its name, or by what a file's name ends in.
    pub fn named(name: &str) -> Option<Format> {
        match name.to_ascii_lowercase().as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" | "jpe" | "jfif" => Some(Format::Jpeg),
            "webp" => Some(Format::Webp),
            "webp-lossless" => Some(Format::WebpLossless),
            "avif" => Some(Format::Avif),
            "jxl" | "jpegxl" | "jpeg-xl" => Some(Format::Jxl),
            "jxl-lossless" | "jpegxl-lossless" | "jpeg-xl-lossless" => Some(Format::JxlLossless),
            _ => None,
        }
    }

    /// The word it goes by in the config file, a file of the canvas and
    /// on the command line, which `named` reads back.
    pub fn word(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
            Format::WebpLossless => "webp-lossless",
            Format::Avif => "avif",
            Format::Jxl => "jxl",
            Format::JxlLossless => "jxl-lossless",
        }
    }

    /// What it is called in a list.
    pub fn title(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::Webp => "WebP",
            Format::WebpLossless => "WebP lossless",
            Format::Avif => "AVIF",
            Format::Jxl => "JPEG XL",
            Format::JxlLossless => "JPEG XL lossless",
        }
    }

    /// What the kind of file is called, whichever way it is written.
    pub fn family(self) -> &'static str {
        match self {
            Format::WebpLossless => "WebP",
            Format::JxlLossless => "JPEG XL",
            other => other.title(),
        }
    }

    /// What the name of a file of this kind ends in.
    pub fn ending(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp | Format::WebpLossless => "webp",
            Format::Avif => "avif",
            Format::Jxl | Format::JxlLossless => "jxl",
        }
    }

    /// Whether every pixel is kept.
    pub fn lossless(self) -> bool {
        matches!(self, Format::Png | Format::WebpLossless | Format::JxlLossless)
    }

    /// The same kind of file, written the other way: kept to the pixel or
    /// not. PNG is only ever lossless, and AVIF and JPEG never are.
    pub fn with_lossless(self, lossless: bool) -> Format {
        match (self, lossless) {
            (Format::Webp | Format::WebpLossless, true) => Format::WebpLossless,
            (Format::Webp | Format::WebpLossless, false) => Format::Webp,
            (Format::Jxl | Format::JxlLossless, true) => Format::JxlLossless,
            (Format::Jxl | Format::JxlLossless, false) => Format::Jxl,
            (other, _) => other,
        }
    }

    /// Named by what a file ends in, which says JPEG XL or WebP but not
    /// which way: the way the `[convert]` table says.
    pub fn as_set(self, settings: &crate::convert::Settings) -> Format {
        match self {
            Format::Jxl if settings.jxl.lossless => Format::JxlLossless,
            Format::Webp if settings.webp.lossless => Format::WebpLossless,
            other => other,
        }
    }
}

/// A picture as a file of one kind or another, each written as the
/// `[convert]` table says. A PNG is packed quickly, as a screenshot is
/// wanted now; the rest by the encoders `convert` has. A JPEG has
/// nothing see-through in it: what would be is white.
pub fn pack(format: Format, settings: &crate::convert::Settings, width: u32, height: u32, pixels: &[u32], alpha: bool) -> Result<Vec<u8>, String> {
    match format {
        Format::Png => encode(width, height, pixels, alpha),
        _ => crate::convert::encode(width, height, pixels, alpha, None, format, settings).map(|encoded| encoded.bytes),
    }
}

/// What kind of file a recording is saved as.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Video {
    #[default]
    Mp4,
    Gif,
    Apng,
    Webp,
}

impl Video {
    /// By its name, or by what a file's name ends in.
    pub fn named(name: &str) -> Option<Video> {
        match name.to_ascii_lowercase().as_str() {
            "mp4" => Some(Video::Mp4),
            "gif" => Some(Video::Gif),
            "apng" | "png" => Some(Video::Apng),
            "webp" => Some(Video::Webp),
            _ => None,
        }
    }

    /// What the name of a file of this kind ends in.
    pub fn ending(self) -> &'static str {
        match self {
            Video::Mp4 => "mp4",
            Video::Gif => "gif",
            Video::Apng => "apng",
            Video::Webp => "webp",
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
/// It is read where there is nothing in the history yet, which is
/// where hotshot keeps them now.
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
    fn a_note_in_a_png_is_read_back_and_the_png_still_reads() {
        let png = encode(4, 2, &[0xFF11_2233; 8], false).unwrap();
        assert_eq!(note_of(&png), None);
        let noted = noted(png, "insets 0 0 24 0\ntitle Größe \u{2014} Acme\n");
        assert_eq!(note_of(&noted).as_deref(), Some("insets 0 0 24 0\ntitle Größe \u{2014} Acme\n"));
        let (w, h, pixels) = crate::pin::read(&noted).or_else(|| crate::frame::open(&noted).map(|photo| (photo.width, photo.height, photo.pixels))).expect("still a PNG");
        assert_eq!((w, h, pixels[0] & 0xFF_FFFF), (4, 2, 0x11_2233));
    }

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
        assert_eq!((Format::named("AVIF"), Format::named("jxl"), Format::named("jxl-lossless")), (Some(Format::Avif), Some(Format::Jxl), Some(Format::JxlLossless)));
        for format in Format::ALL {
            assert_eq!(Format::named(format.word()), Some(format));
        }
        let settings = crate::convert::Settings::default();
        // Half red and half see-through, which in a JPEG is white.
        let pixels: Vec<u32> = (0..64 * 64).map(|at| if at % 64 < 32 { 0xFFFF_0000 } else { 0 }).collect();
        let jpeg = pack(Format::Jpeg, &settings, 64, 64, &pixels, true).unwrap();
        let mut decoder = zune_jpeg::JpegDecoder::new(&jpeg[..]);
        let bytes = decoder.decode().unwrap();
        assert_eq!(decoder.dimensions(), Some((64, 64)));
        let near = |at: usize, to: [u8; 3]| bytes[at * 3..at * 3 + 3].iter().zip(to).all(|(byte, to)| byte.abs_diff(to) < 24);
        assert!(near(32 * 64 + 8, [255, 0, 0]) && near(32 * 64 + 56, [255, 255, 255]));
        // A lossless WebP is to the pixel, and what is see-through still is.
        let webp = pack(Format::WebpLossless, &settings, 64, 64, &pixels, true).unwrap();
        let back = crate::convert::decode(&webp).unwrap();
        assert_eq!((back.width, back.height, back.pixels[0], back.pixels[63]), (64, 64, 0xFFFF_0000, 0));
    }

    #[test]
    fn the_area_last_taken_as_it_was_kept_before_is_still_read() {
        let dir = std::env::temp_dir().join(format!("hotshot-state-{}", std::process::id()));
        std::env::set_var("HOTSHOT_STATE", &dir);
        assert_eq!(last_area(), None);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("last"), "10 -20 300 400\n").unwrap();
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
