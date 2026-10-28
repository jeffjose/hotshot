//! Writing a picture out in another format: JPEG XL, AVIF, JPEG, PNG or
//! WebP, each by an encoder built into hotshot; and reading each of them
//! back. Taken over from jog's, which it is kept the same as, so that
//! the two agree on what the usual settings are and read the same
//! `[convert]` table.
//!
//! The usual settings aim at the same place for every lossy format: a
//! SSIMULACRA2 score of about 85, very good to the eye, at the size each
//! encoder needs for it. They were measured on a 4K photo and a 1024px
//! illustration, and follow Jake Archibald's comparison of JPEG XL and
//! AVIF (2026): AVIF in full color (4:4:4) at a middling speed, jixel's
//! Medium effort, and a JPEG turned into JPEG XL losslessly rather than
//! encoded again.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rayon::prelude::*;

pub use crate::output::Format;

/// JPEG has no transparency, so transparent areas become this color.
const JPEG_BACKGROUND: u32 = 0xFFFFFF;

/// The score the usual settings aim at, and "Fit score" fits to.
pub const AIM: f64 = 85.0;

/// How hard jixel works: slower is smaller, at the same quality.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effort {
    Fastest,
    Fast,
    Medium,
    Slow,
    ExtraSlow,
}

impl Effort {
    pub const ALL: [Effort; 5] = [Effort::Fastest, Effort::Fast, Effort::Medium, Effort::Slow, Effort::ExtraSlow];

    pub fn name(self) -> &'static str {
        match self {
            Effort::Fastest => "fastest",
            Effort::Fast => "fast",
            Effort::Medium => "medium",
            Effort::Slow => "slow",
            Effort::ExtraSlow => "extraslow",
        }
    }

    pub fn parse(word: &str) -> Option<Effort> {
        Effort::ALL.into_iter().find(|effort| effort.name() == word.to_ascii_lowercase())
    }

    fn speed(self) -> jixel::Speed {
        match self {
            Effort::Fastest => jixel::Speed::Fastest,
            Effort::Fast => jixel::Speed::Fast,
            Effort::Medium => jixel::Speed::Medium,
            Effort::Slow => jixel::Speed::Slow,
            Effort::ExtraSlow => jixel::Speed::ExtraSlow,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct JxlSettings {
    /// 0 to 100; 90 is jixel's distance 1.0, the usual "visually lossless".
    pub quality: f32,
    pub effort: Effort,
    /// Every pixel kept exactly, at a much larger size.
    pub lossless: bool,
    /// A JPEG is repacked as it is, nothing lost and the JPEG recoverable
    /// bit for bit, rather than decoded and encoded again.
    pub recompress_jpeg: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AvifSettings {
    /// 0 to 100.
    pub quality: f32,
    /// 1 (slowest, smallest) to 10 (fastest).
    pub speed: u8,
    /// 8 or 10.
    pub depth: u8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct JpegSettings {
    /// 0 to 100.
    pub quality: f32,
    /// Color at full resolution (4:4:4) rather than halved (4:2:0).
    pub full_chroma: bool,
    pub progressive: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WebPSettings {
    /// 0 to 100.
    pub quality: f32,
    pub lossless: bool,
    /// 0 (fastest) to 6 (slowest, smallest).
    pub method: u8,
}

/// How each format is written.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    pub jxl: JxlSettings,
    pub avif: AvifSettings,
    pub jpeg: JpegSettings,
    pub webp: WebPSettings,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            jxl: JxlSettings { quality: 90.0, effort: Effort::Medium, lossless: false, recompress_jpeg: true },
            avif: AvifSettings { quality: 88.0, speed: 6, depth: 8 },
            jpeg: JpegSettings { quality: 90.0, full_chroma: true, progressive: true },
            webp: WebPSettings { quality: 90.0, lossless: false, method: 4 },
        }
    }
}

impl Settings {
    /// Takes in the `[convert]` table of the config file:
    ///
    /// ```toml
    /// [convert]
    /// jxl  = { quality = 90, effort = "medium", lossless = false, recompress-jpeg = true }
    /// avif = { quality = 88, speed = 6, depth = 8 }
    /// jpeg = { quality = 90, chroma = "444", progressive = true }
    /// webp = { quality = 90, lossless = false, method = 4 }
    /// ```
    pub fn apply(&mut self, table: &toml::Table) -> Result<(), String> {
        for (format, value) in table {
            let fields = value.as_table().ok_or_else(|| format!("convert.{format}: expected a table"))?;
            // jog's videos, which hotshot does not convert, are let be.
            if format == "video" {
                continue;
            }
            for (key, value) in fields {
                let wrong = || format!("convert.{format}.{key}: not a usable value");
                let number = || value.as_float().or_else(|| value.as_integer().map(|n| n as f64)).ok_or_else(wrong);
                let quality = || number().map(|n| n.clamp(0.0, 100.0) as f32);
                let flag = || value.as_bool().ok_or_else(wrong);
                match (format.as_str(), key.replace('_', "-").as_str()) {
                    ("jxl", "quality") => self.jxl.quality = quality()?,
                    ("jxl", "effort") => self.jxl.effort = value.as_str().and_then(Effort::parse).ok_or_else(wrong)?,
                    ("jxl", "lossless") => self.jxl.lossless = flag()?,
                    ("jxl", "recompress-jpeg") => self.jxl.recompress_jpeg = flag()?,
                    ("avif", "quality") => self.avif.quality = quality()?,
                    ("avif", "speed") => self.avif.speed = number()?.clamp(1.0, 10.0) as u8,
                    ("avif", "depth") => self.avif.depth = if number()? >= 10.0 { 10 } else { 8 },
                    ("jpeg", "quality") => self.jpeg.quality = quality()?,
                    ("jpeg", "chroma") => {
                        let said = value.as_str().map(str::to_string).or(value.as_integer().map(|n| n.to_string()));
                        self.jpeg.full_chroma = match said.as_deref() {
                            Some("444" | "4:4:4") => true,
                            Some("420" | "4:2:0") => false,
                            _ => return Err(wrong()),
                        }
                    }
                    ("jpeg", "progressive") => self.jpeg.progressive = flag()?,
                    ("webp", "quality") => self.webp.quality = quality()?,
                    ("webp", "lossless") => self.webp.lossless = flag()?,
                    ("webp", "method") => self.webp.method = number()?.clamp(0.0, 6.0) as u8,
                    _ => return Err(format!("convert.{format}.{key}: no such setting")),
                }
            }
        }
        Ok(())
    }

    /// Takes in the text of a `[convert]` table, without its heading.
    pub fn apply_text(&mut self, text: &str) -> Result<(), String> {
        let table = text.parse::<toml::Table>().map_err(|error| error.to_string().lines().next().unwrap_or("").to_owned())?;
        self.apply(&table)
    }

    /// Sets the quality a format is written at; lossless formats have none.
    pub fn set_quality(&mut self, format: Format, quality: f32) {
        match format {
            Format::Jxl => self.jxl.quality = quality,
            Format::Avif => self.avif.quality = quality,
            Format::Jpeg => self.jpeg.quality = quality,
            Format::Webp => self.webp.quality = quality,
            Format::Png | Format::JxlLossless | Format::WebpLossless => {}
        }
    }

    /// The quality a format is written at, if it has one.
    pub fn quality(&self, format: Format) -> Option<f32> {
        match format {
            Format::Jxl => Some(self.jxl.quality),
            Format::Avif => Some(self.avif.quality),
            Format::Jpeg => Some(self.jpeg.quality),
            Format::Webp => Some(self.webp.quality),
            Format::Png | Format::JxlLossless | Format::WebpLossless => None,
        }
    }

    /// The settings a format is written with, in a few words.
    pub fn describe(&self, format: Format) -> String {
        match format {
            Format::JxlLossless => format!("lossless, {}", self.jxl.effort.name()),
            Format::Jxl => format!("q{}, {}", self.jxl.quality, self.jxl.effort.name()),
            Format::Avif => format!("q{}, speed {}, {}-bit", self.avif.quality, self.avif.speed, self.avif.depth),
            Format::Jpeg => format!("q{}, {}", self.jpeg.quality, if self.jpeg.full_chroma { "4:4:4" } else { "4:2:0" }),
            Format::Png | Format::WebpLossless => "lossless".to_string(),
            Format::Webp => format!("q{}", self.webp.quality),
        }
    }

    /// One format's settings as the inline table the config file has for
    /// it, `{ quality = 88, speed = 6, depth = 8 }`; what is written of a
    /// row of the canvas set to its own.
    pub fn table_of(&self, format: Format) -> (&'static str, String) {
        let (jxl, avif, jpeg, webp) = (&self.jxl, &self.avif, &self.jpeg, &self.webp);
        match format {
            Format::Jxl | Format::JxlLossless => ("jxl", format!("{{ quality = {}, effort = \"{}\", lossless = {}, recompress-jpeg = {} }}", jxl.quality, jxl.effort.name(), jxl.lossless, jxl.recompress_jpeg)),
            Format::Avif => ("avif", format!("{{ quality = {}, speed = {}, depth = {} }}", avif.quality, avif.speed, avif.depth)),
            Format::Jpeg => ("jpeg", format!("{{ quality = {}, chroma = \"{}\", progressive = {} }}", jpeg.quality, if jpeg.full_chroma { "444" } else { "420" }, jpeg.progressive)),
            Format::Webp | Format::WebpLossless => ("webp", format!("{{ quality = {}, lossless = {}, method = {} }}", webp.quality, webp.lossless, webp.method)),
            Format::Png => ("png", String::new()),
        }
    }

    /// The settings in the form of the config file's `[convert]` table,
    /// without its heading.
    pub fn to_toml(&self) -> String {
        [Format::Jxl, Format::Avif, Format::Jpeg, Format::Webp]
            .iter()
            .map(|&format| {
                let (name, table) = self.table_of(format);
                format!("{name} = {table}\n")
            })
            .collect()
    }

    /// Keeps these as the settings everything is written with from now
    /// on: the config file's `[convert]` table put in place of the one
    /// there was, or added at its end.
    pub fn save_as_default(&self) -> std::io::Result<PathBuf> {
        let file = crate::options::config_file();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let before = std::fs::read_to_string(&file).unwrap_or_default();
        std::fs::write(&file, with_table(&before, &self.to_toml()))?;
        Ok(file)
    }
}

/// The text of a config file with its `[convert]` table put in place of
/// the one it had, or added at its end: what is above it and any other
/// table after it kept as they are.
fn with_table(text: &str, table: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut put = false;
    for line in text.lines() {
        let heading = line.trim_start().starts_with('[');
        if heading {
            inside = line.trim() == "[convert]";
        }
        if inside {
            if heading && !put {
                out += "[convert]\n";
                out += table;
                put = true;
            }
            continue;
        }
        out += line;
        out.push('\n');
    }
    if !put {
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out += "[convert]\n";
        out += table;
    }
    out
}

/// A picture written out: its bytes, and how, when that differs from what
/// the settings say (a JPEG repacked rather than encoded).
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub how: String,
}

/// The pixels as bytes, three or four to each, in the order the encoders
/// take them; over white, for JPEG's lack of transparency.
fn bytes(pixels: &[u32], alpha: bool, over_white: bool) -> Vec<u8> {
    let each = if alpha { 4 } else { 3 };
    let mut out = vec![0u8; pixels.len() * each];
    out.par_chunks_mut(each << 12).zip(pixels.par_chunks(1 << 12)).for_each(|(out, pixels)| {
        for (bytes, &pixel) in out.chunks_exact_mut(each).zip(pixels) {
            let pixel = if over_white { on_white(pixel) } else { pixel };
            bytes.copy_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8, (pixel >> 24) as u8][..each]);
        }
    });
    out
}

/// A pixel over white, opaque.
fn on_white(pixel: u32) -> u32 {
    let opaque = pixel >> 24;
    if opaque == 255 {
        return pixel;
    }
    let mix = |shift: u32| ((pixel >> shift & 0xFF) * opaque + (JPEG_BACKGROUND >> shift & 0xFF) * (255 - opaque)) / 255;
    0xFF00_0000 | mix(16) << 16 | mix(8) << 8 | mix(0)
}

/// The pixels over white, as JPEG has them and trials are compared.
pub fn flat(pixels: &[u32]) -> Vec<u32> {
    pixels.par_iter().map(|&pixel| on_white(pixel)).collect()
}

/// Encodes a picture, each pixel with how opaque it is in its top byte
/// where `alpha` says it has any see-through part, as `format`. `source`
/// is a file's own bytes, for a JPEG that JPEG XL can repack as it is.
pub fn encode(width: u32, height: u32, pixels: &[u32], alpha: bool, source: Option<&[u8]>, format: Format, settings: &Settings) -> Result<Encoded, String> {
    if width == 0 || height == 0 || pixels.len() != (width * height) as usize {
        return Err("there is no picture to write".into());
    }
    let alpha = alpha && pixels.iter().any(|pixel| pixel >> 24 != 255);
    let (width_n, height_n) = (width as usize, height as usize);
    let how = settings.describe(format);
    let bytes = match format {
        Format::Jxl | Format::JxlLossless => {
            let jxl = &settings.jxl;
            let lossless = format == Format::JxlLossless;
            let jpeg = source.filter(|bytes| bytes.starts_with(b"\xFF\xD8\xFF"));
            if let Some(jpeg) = jpeg.filter(|_| jxl.recompress_jpeg && !lossless) {
                let config = jixel::JpegTranscodeConfig::default();
                match jixel::encode_jpeg_lossless_with_config(jpeg, &config) {
                    Ok(bytes) => return Ok(Encoded { bytes, how: "the JPEG repacked, nothing lost".to_string() }),
                    // Some JPEGs cannot be repacked: those are encoded.
                    Err(error) => eprintln!("hotshot: cannot repack the JPEG ({error:?}), encoding it instead"),
                }
            }
            let mut config = jixel::EncodeConfig::default().with_speed(jxl.effort.speed());
            config = if lossless { config.with_lossless(true) } else { config.with_quality(jxl.quality) };
            match alpha {
                true => jixel::encode_image_with_alpha(&bytes(pixels, true, false), width_n, height_n, &config),
                false => jixel::encode_image(&bytes(pixels, false, false), width_n, height_n, &config),
            }
            .map_err(|error| format!("JPEG XL: {error:?}"))?
        }
        Format::Avif => {
            use rgb::FromSlice;
            let avif = &settings.avif;
            let depth = if avif.depth >= 10 { ravif::BitDepth::Ten } else { ravif::BitDepth::Eight };
            let encoder = ravif::Encoder::new().with_quality(avif.quality).with_alpha_quality(avif.quality).with_speed(avif.speed).with_bit_depth(depth);
            let raw = bytes(pixels, alpha, false);
            let encoded = match alpha {
                true => encoder.encode_rgba(ravif::Img::new(raw.as_rgba(), width_n, height_n)),
                false => encoder.encode_rgb(ravif::Img::new(raw.as_rgb(), width_n, height_n)),
            };
            encoded.map_err(|error| format!("AVIF: {error}"))?.avif_file
        }
        Format::Jpeg => {
            let jpeg = &settings.jpeg;
            let rgb = bytes(pixels, false, true);
            // mozjpeg says what went wrong by unwinding, which with
            // panics set to abort cannot be caught: the sizes it would
            // refuse are refused here first.
            if width > 65_500 || height > 65_500 {
                return Err("too large for a JPEG".into());
            }
            let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
            compress.set_size(width_n, height_n);
            compress.set_quality(jpeg.quality);
            if jpeg.full_chroma {
                compress.set_chroma_sampling_pixel_sizes((1, 1), (1, 1));
            } else {
                compress.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
            }
            if jpeg.progressive {
                compress.set_progressive_mode();
                compress.set_optimize_scans(true);
            }
            let mut started = compress.start_compress(Vec::new()).map_err(|error| format!("JPEG: {error}"))?;
            started.write_scanlines(&rgb).map_err(|error| format!("JPEG: {error}"))?;
            started.finish().map_err(|error| format!("JPEG: {error}"))?
        }
        Format::Png => small_png(width, height, pixels, alpha)?,
        Format::Webp | Format::WebpLossless => {
            let webp = &settings.webp;
            if width > 16_383 || height > 16_383 {
                return Err("too large for a WebP".into());
            }
            let mut config = webp::WebPConfig::new().map_err(|()| "WebP: no encoder settings")?;
            config.quality = if format == Format::WebpLossless { 75.0 } else { webp.quality };
            config.method = i32::from(webp.method);
            config.lossless = i32::from(format == Format::WebpLossless);
            // Every pixel kept, the colors of see-through ones too.
            config.exact = i32::from(format == Format::WebpLossless);
            let raw = bytes(pixels, alpha, false);
            let encoder = match alpha {
                true => webp::Encoder::from_rgba(&raw, width, height),
                false => webp::Encoder::from_rgb(&raw, width, height),
            };
            encoder.encode_advanced(&config).map_err(|error| format!("WebP: {error:?}"))?.to_vec()
        }
    };
    Ok(Encoded { bytes, how })
}

/// A PNG packed as small as it reasonably goes: for a file that is kept,
/// where the quick one is for a screenshot wanted now.
fn small_png(width: u32, height: u32, pixels: &[u32], alpha: bool) -> Result<Vec<u8>, String> {
    let raw = bytes(pixels, alpha, false);
    let mut out = Vec::with_capacity(raw.len() / 4);
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(if alpha { png::ColorType::Rgba } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer.write_image_data(&raw).map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(out)
}

/// A picture read from a file: each pixel with how opaque it is in its
/// top byte, and whether any is less than opaque.
pub struct Decoded {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
    pub alpha: bool,
}

/// Reads a PNG, a JPEG, a WebP, an AVIF or a JPEG XL, by how it begins.
pub fn decode(bytes: &[u8]) -> Option<Decoded> {
    let made = |width: i32, height: i32, pixels: Vec<u32>| {
        let alpha = pixels.iter().any(|pixel| pixel >> 24 != 255);
        (width > 0 && height > 0 && pixels.len() == (width * height) as usize).then_some(Decoded { width, height, pixels, alpha })
    };
    if bytes.starts_with(b"\x89PNG") {
        let (width, height, pixels) = crate::pin::read(bytes)?;
        return made(width, height, pixels);
    }
    if bytes.starts_with(b"\xFF\xD8") {
        let mut decoder = zune_jpeg::JpegDecoder::new(bytes);
        let data = decoder.decode().ok()?;
        let info = decoder.info()?;
        let (width, height) = (i32::from(info.width), i32::from(info.height));
        let each = data.len() / (width.max(1) * height.max(1)) as usize;
        let pixels = data.chunks_exact(each.max(1)).map(|pixel| match each {
            1 => 0xFF00_0000 | u32::from(pixel[0]) * 0x010101,
            _ => 0xFF00_0000 | u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2]),
        });
        return made(width, height, pixels.collect());
    }
    if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        let image = webp::Decoder::new(bytes).decode()?;
        let each = if image.is_alpha() { 4 } else { 3 };
        let pixels = image.chunks_exact(each).map(|pixel| {
            let opaque = if each == 4 { u32::from(pixel[3]) } else { 255 };
            opaque << 24 | u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2])
        });
        return made(image.width() as i32, image.height() as i32, pixels.collect());
    }
    if crate::avif::is_avif(bytes) {
        let (width, height, pixels, _) = crate::avif::read(bytes)?;
        return made(width, height, pixels);
    }
    if is_jxl(bytes) {
        let image = jxl_oxide::JxlImage::builder().read(bytes).ok()?;
        let render = image.render_frame(0).ok()?;
        let mut stream = render.stream();
        let (width, height, channels) = (stream.width() as i32, stream.height() as i32, stream.channels() as usize);
        let mut data = vec![0u8; width as usize * height as usize * channels];
        stream.write_to_buffer(&mut data);
        let pixels = data.chunks_exact(channels.max(1)).map(|pixel| match channels {
            1 => 0xFF00_0000 | u32::from(pixel[0]) * 0x010101,
            2 => u32::from(pixel[1]) << 24 | u32::from(pixel[0]) * 0x010101,
            3 => 0xFF00_0000 | u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2]),
            _ => u32::from(pixel[3]) << 24 | u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2]),
        });
        return made(width, height, pixels.collect());
    }
    None
}

/// Whether some bytes are a JPEG XL: a bare codestream, or one in its box.
pub fn is_jxl(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0x0A]) || bytes.starts_with(&[0, 0, 0, 0x0C, b'J', b'X', b'L', b' '])
}

/// Whether a file is a picture hotshot reads, by what its name ends in.
pub fn reads(path: &Path) -> bool {
    path.extension().is_some_and(|ending| matches!(ending.to_string_lossy().to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "jpe" | "jfif" | "webp" | "avif" | "jxl"))
}

/// A picture encoded to see what the settings make of it, as the export
/// list and the Fine-tune panel show: how large, how long it took, how
/// it looks decoded again, and how near the original that is.
#[derive(Clone)]
pub struct Trial {
    pub size: usize,
    pub how: String,
    pub took: Duration,
    /// Decoded again over white, as `flat` is.
    pub decoded: Option<Vec<u32>>,
    /// SSIMULACRA2: 90 and over is visually lossless, 70 good.
    pub score: Option<f64>,
}

/// Encodes a picture as `format`, decodes it again, and with `scored`
/// scores it against `flat`, the original over white. A lossless format
/// is perfect, and is not scored.
#[allow(clippy::too_many_arguments)]
pub fn trial(width: u32, height: u32, pixels: &[u32], alpha: bool, flat: &[u32], source: Option<&[u8]>, format: Format, settings: &Settings, scored: bool) -> Result<Trial, String> {
    let started = Instant::now();
    let encoded = encode(width, height, pixels, alpha, source, format, settings)?;
    let took = started.elapsed();
    let decoded = decode(&encoded.bytes).filter(|back| (back.width as u32, back.height as u32) == (width, height)).map(|back| self::flat(&back.pixels));
    let mut trial = Trial { size: encoded.bytes.len(), how: encoded.how, took, decoded, score: None };
    if scored {
        trial.score = trial_score(width, height, flat, &trial, format);
    }
    Ok(trial)
}

/// How near a trial stays to the original. A lossless one is perfect, and
/// a JPEG repacked loses nothing: neither has a score to work out.
pub fn trial_score(width: u32, height: u32, flat: &[u32], trial: &Trial, format: Format) -> Option<f64> {
    if format.lossless() || trial.how.contains("repacked") {
        return Some(100.0);
    }
    score(width, height, flat, trial.decoded.as_deref()?)
}

/// SSIMULACRA2 of a picture against the original, both opaque.
pub fn score(width: u32, height: u32, original: &[u32], distorted: &[u32]) -> Option<f64> {
    use ssimulacra2::{ColorPrimaries, Rgb, TransferCharacteristic};
    if original.len() != distorted.len() || width < 8 || height < 8 {
        return None;
    }
    let frame = |pixels: &[u32]| {
        let data = pixels.iter().map(|pixel| [(pixel >> 16 & 0xFF) as f32 / 255.0, (pixel >> 8 & 0xFF) as f32 / 255.0, (pixel & 0xFF) as f32 / 255.0]).collect();
        Rgb::new(data, width as usize, height as usize, TransferCharacteristic::SRGB, ColorPrimaries::BT709).ok()
    };
    ssimulacra2::compute_frame_ssimulacra2(frame(original)?, frame(distorted)?).ok()
}

/// What a score means, in a few words.
pub fn verdict(score: f64) -> &'static str {
    match score {
        s if s >= 99.0 => "every pixel kept",
        s if s >= 90.0 => "visually lossless",
        s if s >= 80.0 => "very good to the eye",
        s if s >= 70.0 => "good",
        s if s >= 50.0 => "softer, still clear",
        _ => "plainly worse",
    }
}

/// A size said as people say it: 712 KB, 1.2 MB.
pub fn size_words(bytes: usize) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1 << 20) as f64),
        b if b >= 1 << 10 => format!("{} KB", (b + 512) >> 10),
        b => format!("{b} B"),
    }
}

/// A change in size from one to another, as a percentage: −41%, +8%.
pub fn change_words(from: usize, to: usize) -> String {
    let change = (to as f64 / from.max(1) as f64 - 1.0) * 100.0;
    let rounded = change.round() as i64;
    match rounded {
        r if r < 0 => format!("\u{2212}{}%", -r),
        r => format!("+{r}%"),
    }
}

/// A size typed, as 200 KB, 1.5 MB, 300k or a number of bytes.
pub fn parse_size(text: &str) -> Option<usize> {
    let text = text.trim().to_ascii_lowercase().replace(' ', "");
    let (number, times) = if let Some(number) = text.strip_suffix("mb").or_else(|| text.strip_suffix('m')) {
        (number, 1 << 20)
    } else if let Some(number) = text.strip_suffix("kb").or_else(|| text.strip_suffix('k')) {
        (number, 1 << 10)
    } else {
        (text.strip_suffix('b').unwrap_or(&text), 1)
    };
    let value: f64 = number.parse().ok()?;
    (value > 0.0 && value.is_finite()).then(|| (value * times as f64) as usize)
}

/// The quality, a whole number, at which a lossy format is as small as
/// it can be while what `good` says of its trial still holds, the
/// higher the quality the more it holds; or the highest it holds at, if
/// it never stops. Halving, seven trials or so. `step` is told each
/// trial as it is made, and stops the search by saying false.
fn search(format: Format, settings: &Settings, mut try_at: impl FnMut(&Settings) -> Option<Trial>, good: impl Fn(&Trial) -> bool, lowest_good: bool, step: &mut dyn FnMut(usize) -> bool) -> Option<(f32, Trial)> {
    let (mut low, mut high) = (1i32, 100i32);
    let mut best: Option<(f32, Trial)> = None;
    let mut count = 0;
    while low <= high {
        let middle = (low + high) / 2;
        let mut tried = *settings;
        tried.set_quality(format, middle as f32);
        count += 1;
        if !step(count) {
            return None;
        }
        let trial = try_at(&tried)?;
        let holds = good(&trial);
        // Fitting a score: the lowest that scores enough. Fitting a size:
        // the highest that still fits.
        match (holds, lowest_good) {
            (true, true) => {
                best = Some((middle as f32, trial));
                high = middle - 1;
            }
            (false, true) => low = middle + 1,
            (true, false) => {
                best = Some((middle as f32, trial));
                low = middle + 1;
            }
            (false, false) => high = middle - 1,
        }
    }
    best
}

/// The quality at which a lossy format is smallest while scoring at least
/// `aim`; or nothing for a lossless one, or where none scores it.
#[allow(clippy::too_many_arguments)]
pub fn fit_score(width: u32, height: u32, pixels: &[u32], alpha: bool, flat: &[u32], format: Format, settings: &Settings, aim: f64, step: &mut dyn FnMut(usize) -> bool) -> Option<(f32, Trial)> {
    settings.quality(format)?;
    let try_at = |tried: &Settings| trial(width, height, pixels, alpha, flat, None, format, tried, true).ok();
    search(format, settings, try_at, |trial| trial.score.is_some_and(|score| score >= aim), true, step)
}

/// The quality at which a lossy format is best while still no larger than
/// `most` bytes; or nothing for a lossless one, or where even the lowest
/// is larger.
#[allow(clippy::too_many_arguments)]
pub fn fit_under(width: u32, height: u32, pixels: &[u32], alpha: bool, flat: &[u32], format: Format, settings: &Settings, most: usize, step: &mut dyn FnMut(usize) -> bool) -> Option<(f32, Trial)> {
    settings.quality(format)?;
    let try_at = |tried: &Settings| trial(width, height, pixels, alpha, flat, None, format, tried, false).ok();
    let (quality, mut found) = search(format, settings, try_at, |trial| trial.size <= most, false, step)?;
    found.score = trial_score(width, height, flat, &found, format);
    Some((quality, found))
}

/// Where a copy of the picture at `path` goes as `format`: beside it,
/// with the new ending.
pub fn target(path: &Path, format: Format) -> PathBuf {
    path.with_extension(format.ending())
}

/// Writes a copy of the picture at `path` as `format`, next to it, and
/// says where and how. An existing file is never overwritten.
pub fn convert(path: &Path, format: Format, settings: &Settings) -> Result<(PathBuf, String), String> {
    let ending = path.extension().map(|ending| ending.to_string_lossy().to_ascii_lowercase());
    if ending.as_deref().and_then(Format::named).is_some_and(|known| known.ending() == format.ending()) {
        return Err(format!("already {}", format.family()));
    }
    let target = target(path, format);
    if target.exists() {
        return Err(format!("{} is there already", target.display()));
    }
    let source = std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let picture = decode(&source).ok_or_else(|| format!("{} is not a picture hotshot reads", path.display()))?;
    let encoded = encode(picture.width as u32, picture.height as u32, &picture.pixels, picture.alpha, Some(&source), format, settings)?;
    // Creating the file fails if the name is taken, so nothing is clobbered.
    let mut file = OpenOptions::new().write(true).create_new(true).open(&target).map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    if let Err(error) = file.write_all(&encoded.bytes) {
        let _ = std::fs::remove_file(&target);
        return Err(format!("cannot write {}: {error}", target.display()));
    }
    Ok((target, encoded.how))
}

/// Converts each of `paths` to `format`, one after another, saying how it
/// goes on stdout and stderr; for `hotshot --convert`. False if any failed.
pub fn convert_all(paths: &[PathBuf], format: Format, settings: &Settings) -> bool {
    // Pictures in the format already are not what is meant, and those
    // converted before are done: run again over a folder, only what is
    // new is converted.
    let already = |path: &PathBuf| path.extension().is_some_and(|ending| ending.to_string_lossy().eq_ignore_ascii_case(format.ending()));
    let paths: Vec<&PathBuf> = paths.iter().filter(|path| !already(path)).collect();
    let (todo, done): (Vec<&PathBuf>, Vec<&PathBuf>) = paths.into_iter().partition(|path| !target(path, format).exists());
    // Two pictures that would make the same file, photo.jpg and photo.png
    // both photo.jxl: which is the original cannot be told, and taking the
    // wrong one makes a copy of a copy. Neither is converted.
    let mut targets: std::collections::HashMap<PathBuf, Vec<&PathBuf>> = std::collections::HashMap::new();
    for path in &todo {
        targets.entry(target(path, format)).or_default().push(path);
    }
    let mut clashes: Vec<_> = targets.into_iter().filter(|(_, sources)| sources.len() > 1).collect();
    clashes.sort();
    for (target, sources) in &clashes {
        let names: Vec<String> = sources.iter().map(|path| path.display().to_string()).collect();
        let all = if names.len() == 2 { "both" } else { "all" };
        eprintln!("hotshot: {} would {all} become {}: left alone", names.join(" and "), target.display());
    }
    let clashing: Vec<&PathBuf> = clashes.iter().flat_map(|(_, sources)| sources.iter().copied()).collect();
    let todo: Vec<&PathBuf> = todo.into_iter().filter(|path| !clashing.contains(path)).collect();
    let (total, mut failed, mut before, mut after) = (todo.len(), clashing.len(), 0, 0);
    let started = Instant::now();
    let skipped = if done.is_empty() { String::new() } else { format!(", {} done before", done.len()) };
    eprintln!("hotshot: converting {total} to {} ({}){skipped}", format.title(), settings.describe(format));
    for (index, path) in todo.iter().enumerate() {
        let count = format!("[{}/{total}]", index + 1);
        match convert(path, format, settings) {
            Ok((to, how)) => {
                let (from_size, to_size) = (size(path), size(&to));
                (before, after) = (before + from_size, after + to_size);
                let change = (to_size as f64 / from_size.max(1) as f64 - 1.0) * 100.0;
                println!("{count} {} -> {}  {change:+.0}%  ({how})", path.display(), to.display());
            }
            Err(error) => {
                eprintln!("{count} {}: {error}", path.display());
                failed += 1;
            }
        }
    }
    let change = if before > 0 { format!(", {:+.0}%", (after as f64 / before as f64 - 1.0) * 100.0) } else { String::new() };
    let converted = total + clashing.len() - failed;
    match failed {
        0 => eprintln!("hotshot: {converted} converted in {:.0?}{change}", started.elapsed()),
        _ => eprintln!("hotshot: {converted} converted, {failed} not, in {:.0?}{change}", started.elapsed()),
    }
    failed == 0
}

pub fn size(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |meta| meta.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hotshot-convert-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Something with detail, so that lossy encoders have work to do.
    fn photo() -> (u32, u32, Vec<u32>) {
        let (w, h) = (96u32, 64u32);
        let pixels = (0..w * h).map(|at| {
            let (x, y) = (at % w, at / w);
            0xFF00_0000 | (x * 2 & 0xFF) << 16 | (y * 3 & 0xFF) << 8 | ((x + y) * 2 + (x ^ y) % 8 & 0xFF)
        });
        (w, h, pixels.collect())
    }

    fn write_png(path: &Path, (w, h, pixels): &(u32, u32, Vec<u32>)) {
        std::fs::write(path, crate::output::encode(*w, *h, pixels, pixels.iter().any(|pixel| pixel >> 24 != 255)).unwrap()).unwrap();
    }

    #[test]
    fn every_format_writes_a_picture_that_reads_back() {
        let (w, h, pixels) = photo();
        let settings = Settings::default();
        for format in Format::ALL {
            let encoded = encode(w, h, &pixels, false, None, format, &settings).unwrap();
            assert!(encoded.bytes.len() > 20, "{format:?} wrote something");
            let back = decode(&encoded.bytes).unwrap_or_else(|| panic!("{format:?} reads back"));
            assert_eq!((back.width, back.height), (96, 64), "{format:?}");
            // Lossy, but near: a pixel in the middle is about as it was.
            let (was, now) = (pixels[32 * 96 + 40], back.pixels[32 * 96 + 40]);
            let near = (0..3).all(|part| ((was >> (part * 8)) & 0xFF).abs_diff((now >> (part * 8)) & 0xFF) < 40);
            assert!(near, "{format:?}: {was:06X} came back {now:06X}");
            if format.lossless() {
                assert_eq!(back.pixels, pixels, "{format:?} keeps every pixel");
            }
        }
        assert!(avif_first(&encode(w, h, &pixels, false, None, Format::Avif, &settings).unwrap().bytes));
    }

    fn avif_first(bytes: &[u8]) -> bool {
        bytes[4..12] == *b"ftypavif"
    }

    #[test]
    fn lossless_formats_keep_every_pixel_and_transparency() {
        let pixels: Vec<u32> = (0..64u32).map(|at| if at < 32 { 0xFFFF_0000 } else { 0 }).collect();
        let settings = Settings::default();
        for format in [Format::Png, Format::WebpLossless, Format::JxlLossless] {
            let back = decode(&encode(8, 8, &pixels, true, None, format, &settings).unwrap().bytes).unwrap();
            assert_eq!((back.pixels.clone(), back.alpha), (pixels.clone(), true), "{format:?}");
        }
        // AVIF keeps what is see-through as see-through, near enough.
        let back = decode(&encode(8, 8, &pixels, true, None, Format::Avif, &settings).unwrap().bytes).unwrap();
        assert!(back.alpha && back.pixels[60] >> 24 < 16 && back.pixels[3] >> 24 > 240);
        // JPEG has no transparency: it goes white.
        let back = decode(&encode(8, 8, &pixels, true, None, Format::Jpeg, &settings).unwrap().bytes).unwrap();
        assert!(!back.alpha && back.pixels[60] & 0xFF_FFFF > 0xE0_E0E0);
    }

    #[test]
    fn a_jpeg_becomes_jpeg_xl_without_loss() {
        let dir = scratch("jpeg");
        let source = dir.join("photo.jpg");
        let (w, h, pixels) = photo();
        std::fs::write(&source, encode(w, h, &pixels, false, None, Format::Jpeg, &Settings::default()).unwrap().bytes).unwrap();
        let (target, how) = convert(&source, Format::Jxl, &Settings::default()).unwrap();
        assert!(how.contains("repacked"), "{how}");
        // The JPEG comes back out of it byte for byte.
        let jxl = std::fs::read(&target).unwrap();
        let image = jxl_oxide::JxlImage::builder().read(&jxl[..]).unwrap();
        let mut rebuilt = Vec::new();
        image.reconstruct_jpeg(&mut rebuilt).unwrap();
        assert!(rebuilt == std::fs::read(&source).unwrap(), "the very same JPEG");
        assert!(jxl.len() < rebuilt.len(), "and smaller");
        // And it opens as a picture too.
        assert_eq!(decode(&jxl).map(|back| (back.width, back.height)), Some((96, 64)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nothing_is_overwritten_or_converted_to_itself() {
        let dir = scratch("clobber");
        let source = dir.join("picture.png");
        write_png(&source, &photo());
        let settings = Settings::default();
        assert!(convert(&source, Format::Png, &settings).is_err(), "already a PNG");
        std::fs::write(dir.join("picture.jpg"), b"mine").unwrap();
        assert!(convert(&source, Format::Jpeg, &settings).is_err());
        assert_eq!(std::fs::read(dir.join("picture.jpg")).unwrap(), b"mine");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_converted_again_converts_only_what_is_new() {
        let dir = scratch("again");
        write_png(&dir.join("a.png"), &photo());
        let settings = Settings::default();
        let list = |dir: &Path| -> Vec<PathBuf> {
            let mut all: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().path()).collect();
            all.sort();
            all
        };
        assert!(convert_all(&list(&dir), Format::Webp, &settings));
        let first = std::fs::metadata(dir.join("a.webp")).unwrap().modified().unwrap();
        write_png(&dir.join("b.png"), &photo());
        assert!(convert_all(&list(&dir), Format::Webp, &settings), "nothing failed");
        assert!(dir.join("b.webp").exists());
        assert_eq!(std::fs::metadata(dir.join("a.webp")).unwrap().modified().unwrap(), first, "a.webp left alone");
        std::fs::remove_dir_all(&dir).unwrap();

        // Two pictures of one name: which is the original is not guessed.
        let dir = scratch("clash");
        write_png(&dir.join("a.png"), &photo());
        write_png(&dir.join("c.png"), &photo());
        let (w, h, pixels) = photo();
        std::fs::write(dir.join("c.jpg"), encode(w, h, &pixels, false, None, Format::Jpeg, &settings).unwrap().bytes).unwrap();
        assert!(!convert_all(&list(&dir), Format::Jxl, &settings), "said so");
        assert!(!dir.join("c.jxl").exists());
        assert!(dir.join("a.jxl").exists(), "the rest done all the same");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_trial_says_how_large_and_how_good() {
        let (w, h, pixels) = photo();
        let flat = flat(&pixels);
        let settings = Settings::default();
        let jxl = trial(w, h, &pixels, false, &flat, None, Format::Jxl, &settings, true).unwrap();
        assert!(jxl.size > 0 && jxl.decoded.is_some());
        let good = jxl.score.unwrap();
        let mut worse = settings;
        worse.jxl.quality = 30.0;
        let low = trial(w, h, &pixels, false, &flat, None, Format::Jxl, &worse, true).unwrap();
        assert!(low.size < jxl.size && low.score.unwrap() < good, "{} < {good}", low.score.unwrap());
        let png = trial(w, h, &pixels, false, &flat, None, Format::Png, &settings, true).unwrap();
        assert_eq!(png.score, Some(100.0), "lossless is perfect");
        // Fitted to a score, the file scores it and one step lower would not.
        let (quality, fitted) = fit_score(w, h, &pixels, false, &flat, Format::Jxl, &settings, 70.0, &mut |_| true).unwrap();
        assert!(fitted.score.unwrap() >= 70.0, "{quality}: {:?}", fitted.score);
        let mut lower = settings;
        lower.set_quality(Format::Jxl, quality - 1.0);
        assert!(quality <= 1.0 || trial(w, h, &pixels, false, &flat, None, Format::Jxl, &lower, true).unwrap().score.unwrap() < 70.0);
        // Fitted under a size, it is under it.
        let most = jxl.size * 2 / 3;
        let (_, under) = fit_under(w, h, &pixels, false, &flat, Format::Jxl, &settings, most, &mut |_| true).unwrap();
        assert!(under.size <= most);
        assert!(fit_score(w, h, &pixels, false, &flat, Format::Png, &settings, 85.0, &mut |_| true).is_none(), "nothing to fit");
    }

    #[test]
    fn saved_settings_read_back_the_same() {
        let mut settings = Settings::default();
        settings.jxl.effort = Effort::Slow;
        settings.avif.depth = 10;
        settings.jpeg.full_chroma = false;
        settings.webp.lossless = true;
        let mut back = Settings::default();
        back.apply_text(&settings.to_toml()).unwrap();
        assert_eq!(back, settings);
    }

    #[test]
    fn settings_come_from_the_config_file() {
        let mut settings = Settings::default();
        settings.apply_text("jxl = { quality = 80, effort = \"slow\" }\navif = { speed = 4, depth = 10 }\njpeg = { chroma = \"420\" }\nwebp = { lossless = true }").unwrap();
        assert_eq!((settings.jxl.quality, settings.jxl.effort), (80.0, Effort::Slow));
        assert_eq!((settings.avif.speed, settings.avif.depth), (4, 10));
        assert!(!settings.jpeg.full_chroma && settings.webp.lossless);
        assert_eq!(Format::Webp.as_set(&settings), Format::WebpLossless);
        assert!(settings.apply_text("jxl = { color = 3 }").is_err());
    }

    #[test]
    fn the_table_saved_takes_the_place_of_the_one_there_was() {
        let table = Settings::default().to_toml();
        let fresh = with_table("dir = \"~/shots\"\n", &table);
        assert!(fresh.starts_with("dir = \"~/shots\"\n\n[convert]\njxl = {"), "{fresh}");
        let mut other = Settings::default();
        other.avif.quality = 70.0;
        let again = with_table(&format!("{fresh}[later]\nx = 1\n"), &other.to_toml());
        assert_eq!(again.matches("[convert]").count(), 1);
        assert!(again.contains("quality = 70") && again.ends_with("[later]\nx = 1\n") && again.starts_with("dir = "), "{again}");
    }

    #[test]
    fn sizes_are_said_and_read_as_people_say_them() {
        assert_eq!((size_words(712 * 1024), size_words(1_258_291), size_words(900)), ("712 KB".to_owned(), "1.2 MB".to_owned(), "900 B".to_owned()));
        assert_eq!((change_words(100, 59), change_words(100, 108)), ("\u{2212}41%".to_owned(), "+8%".to_owned()));
        assert_eq!((parse_size("200 KB"), parse_size("1.5mb"), parse_size("300k"), parse_size("nope")), (Some(204_800), Some(1_572_864), Some(307_200), None));
    }
}
