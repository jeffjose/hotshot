//! The picture as a file: a PNG, and a name with the time in it.

use std::ffi::CString;
use std::io::Write;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

/// A picture as a PNG. Packed quickly rather than small: it is a
/// screenshot, wanted now, and the difference is a few per cent.
pub fn encode(width: u32, height: u32, pixels: &[u32]) -> Result<Vec<u8>, String> {
    // PNG has the colours of a pixel the other way round, and no fourth
    // byte; turning 8 million of them over is worth every core.
    let mut bytes = vec![0u8; pixels.len() * 3];
    bytes.par_chunks_mut(3 << 12).zip(pixels.par_chunks(1 << 12)).for_each(|(bytes, pixels)| {
        for (out, pixel) in bytes.chunks_exact_mut(3).zip(pixels) {
            out.copy_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]);
        }
    });
    let mut png = Vec::with_capacity(bytes.len() / 4);
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(png::ColorType::Rgb);
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
        let png = encode(2, 2, &pixels).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(&bytes[..12], &[255, 0, 0, 0, 255, 0, 0, 0, 255, 0x12, 0x34, 0x56]);
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
