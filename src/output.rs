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
    // PNG has the colours of a pixel the other way round, and no fourth
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

/// Writes a PNG into a folder under a name made from the time, making
/// the folder if need be. A name already taken, by a picture of the same
/// second, has a number put after it.
pub fn save(dir: &Path, template: &str, png: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot make {}: {error}", dir.display()))?;
    let name = dated(template);
    for copy in 1.. {
        let path = dir.join(if copy == 1 { format!("{name}.png") } else { format!("{name}-{copy}.png") });
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

/// Writes a PNG to a file by name, or to standard output for `-`.
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
        let first = save(&dir, "same", b"one").unwrap();
        let second = save(&dir, "same", b"two").unwrap();
        assert_eq!(first.file_name().unwrap(), "same.png");
        assert_eq!(second.file_name().unwrap(), "same-2.png");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
