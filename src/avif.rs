//! An AVIF read: its box opened by avif-parse, the AV1 inside it decoded
//! by rav1d, the Rust port of dav1d, and the YUV it gives turned into the
//! pixels hotshot draws with. Done as kornelski's avif-decode does it,
//! which cannot be used itself: it asks for rav1d's assembly, which needs
//! NASM to build.

use std::mem::MaybeUninit;
use std::ptr::NonNull;

use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::headers::*;
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib::{dav1d_close, dav1d_data_create, dav1d_data_unref, dav1d_default_settings, dav1d_get_picture, dav1d_open, dav1d_picture_unref, dav1d_send_data};

/// Whether some bytes are an AVIF, by the kind its first box says it is.
pub fn is_avif(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[4..8] == b"ftyp" && matches!(&bytes[8..12], b"avif" | b"avis" | b"mif1" | b"miaf")
}

/// The picture in an AVIF, each pixel with how opaque it is in its top
/// byte; and whether it has a see-through part at all.
pub fn read(bytes: &[u8]) -> Option<(i32, i32, Vec<u32>, bool)> {
    let avif = avif_parse::read_avif(&mut &bytes[..]).ok()?;
    let mut decoder = Decoder::new()?;
    let color = decoder.decode(&avif.primary_item)?;
    let (width, height) = (color.p.w as usize, color.p.h as usize);
    let mut pixels = to_rgb(&color)?;
    let alpha = avif.alpha_item.as_ref().and_then(|alpha| {
        let plane = decoder.decode(alpha)?;
        let opacity = luma(&plane)?;
        (plane.p.w as usize == width && plane.p.h as usize == height).then_some(opacity)
    });
    let see_through = alpha.is_some();
    if let Some(alpha) = alpha {
        for (pixel, opacity) in pixels.iter_mut().zip(alpha) {
            let mut rgb = *pixel & 0xFF_FFFF;
            // Colors stored already multiplied by how opaque they are
            // are divided out again.
            if avif.premultiplied_alpha && opacity > 0 && opacity < 255 {
                let undo = |part: u32| ((part * 255 + u32::from(opacity) / 2) / u32::from(opacity)).min(255);
                rgb = undo(rgb >> 16) << 16 | undo(rgb >> 8 & 0xFF) << 8 | undo(rgb & 0xFF);
            }
            *pixel = u32::from(opacity) << 24 | rgb;
        }
    }
    Some((width as i32, height as i32, pixels, see_through))
}

/// rav1d, open for one picture or two.
struct Decoder {
    context: Option<Dav1dContext>,
    picture: Option<Dav1dPicture>,
}

impl Decoder {
    fn new() -> Option<Decoder> {
        // SAFETY: the settings are filled in by rav1d before they are read,
        // and the context is closed when the decoder is dropped.
        unsafe {
            let mut settings = MaybeUninit::<Dav1dSettings>::uninit();
            dav1d_default_settings(NonNull::from(&mut settings).cast());
            let settings = settings.assume_init_mut();
            settings.n_threads = std::thread::available_parallelism().map_or(4, |count| count.get()).min(32) as _;
            // One picture, not a video: nothing to hold back for.
            settings.max_frame_delay = 1;
            settings.apply_grain = 1;
            let mut context = None;
            let result = dav1d_open(Some(NonNull::from(&mut context)), Some(NonNull::from(settings)));
            (result.0 == 0).then_some(Decoder { context, picture: None })
        }
    }

    /// One item of the AVIF decoded: the last picture it makes, which for
    /// one in layers is the whole of it. Kept until the next.
    fn decode(&mut self, av1: &[u8]) -> Option<&Dav1dPicture> {
        let context = self.context?;
        if let Some(old) = &mut self.picture.take() {
            // SAFETY: a picture rav1d gave, given back once.
            unsafe { dav1d_picture_unref(Some(NonNull::from(old))) };
        }
        // SAFETY: the data is made by rav1d and filled to its length; each
        // picture taken is given back but the last, which is kept.
        unsafe {
            let mut data = Dav1dData::default();
            let buffer = NonNull::new(dav1d_data_create(Some(NonNull::from(&mut data)), av1.len()))?;
            std::ptr::copy_nonoverlapping(av1.as_ptr(), buffer.as_ptr(), av1.len());
            let mut last: Option<Dav1dPicture> = None;
            let mut sent = false;
            for _ in 0..64 {
                if dav1d_send_data(Some(context), Some(NonNull::from(&mut data))).0 == 0 {
                    sent = true;
                    break;
                }
                // Still holding a picture: taken out, so the rest can go in.
                let mut pending = Dav1dPicture::default();
                if dav1d_get_picture(Some(context), Some(NonNull::from(&mut pending))).0 != 0 {
                    dav1d_picture_unref(Some(NonNull::from(&mut pending)));
                    break;
                }
                if let Some(mut before) = last.replace(pending) {
                    dav1d_picture_unref(Some(NonNull::from(&mut before)));
                }
            }
            dav1d_data_unref(Some(NonNull::from(&mut data)));
            if !sent && last.is_none() {
                return None;
            }
            loop {
                let mut picture = Dav1dPicture::default();
                if dav1d_get_picture(Some(context), Some(NonNull::from(&mut picture))).0 != 0 {
                    dav1d_picture_unref(Some(NonNull::from(&mut picture)));
                    break;
                }
                if let Some(mut before) = last.replace(picture) {
                    dav1d_picture_unref(Some(NonNull::from(&mut before)));
                }
            }
            self.picture = last;
        }
        self.picture.as_ref()
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the picture and the context are rav1d's, given back once.
        unsafe {
            if let Some(picture) = &mut self.picture {
                dav1d_picture_unref(Some(NonNull::from(picture)));
            }
            dav1d_close(Some(NonNull::from(&mut self.context)));
        }
    }
}

/// A plane of a picture as numbers, each row `stride` long; and how many
/// bits each has.
fn plane(picture: &Dav1dPicture, which: usize) -> Option<(Vec<u16>, usize, usize, usize)> {
    let (sideways, down) = match (which, picture.p.layout) {
        (0, _) | (_, DAV1D_PIXEL_LAYOUT_I444) => (0, 0),
        (_, DAV1D_PIXEL_LAYOUT_I420) => (1, 1),
        (_, DAV1D_PIXEL_LAYOUT_I422) => (1, 0),
        _ => return None,
    };
    let (w, h) = ((picture.p.w as usize + sideways) >> sideways, (picture.p.h as usize + down) >> down);
    let wide = picture.p.bpc > 8;
    let stride = picture.stride[usize::from(which != 0)];
    let each = if wide { 2 } else { 1 };
    if stride <= 0 || (stride as usize) < w * each {
        return None;
    }
    let start = picture.data[which]?;
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        // SAFETY: rav1d allocates whole rows, `stride` bytes apart, for
        // every row of the plane.
        unsafe {
            let row = start.as_ptr().cast::<u8>().add(y * stride as usize);
            match wide {
                true => out.extend(std::slice::from_raw_parts(row.cast::<u16>(), w)),
                false => out.extend(std::slice::from_raw_parts(row, w).iter().map(|&value| u16::from(value))),
            }
        }
    }
    Some((out, w, h, picture.p.bpc as usize))
}

/// The first plane of a picture as 0 to 255, full range: an alpha item's.
fn luma(picture: &Dav1dPicture) -> Option<Vec<u8>> {
    let (values, ..) = plane(picture, 0)?;
    let full = sequence(picture).is_some_and(|header| header.color_range != 0);
    let depth = picture.p.bpc as u32;
    let top = ((1u32 << depth) - 1) as f32;
    Some(
        values
            .iter()
            .map(|&value| {
                let value = f32::from(value);
                let unit = if full { value / top } else { (value - (16 << (depth - 8)) as f32) / (219 << (depth - 8)) as f32 };
                (unit.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            })
            .collect(),
    )
}

fn sequence(picture: &Dav1dPicture) -> Option<&Dav1dSequenceHeader> {
    // SAFETY: the header lives as long as the picture holding it.
    picture.seq_hdr.map(|header| unsafe { header.as_ref() })
}

/// A picture's YUV as opaque pixels, by the matrix and range its header
/// says, the color planes stretched to the size of the first.
fn to_rgb(picture: &Dav1dPicture) -> Option<Vec<u32>> {
    let header = sequence(picture);
    let full = header.is_some_and(|header| header.color_range != 0);
    let matrix = header.map_or(DAV1D_MC_BT601, |header| header.mtrx);
    let depth = picture.p.bpc as u32;
    let (y, w, h, _) = plane(picture, 0)?;
    let top = ((1u32 << depth) - 1) as f32;
    let half = (1u32 << (depth - 1)) as f32;
    let shade = |value: u16| match full {
        true => f32::from(value) / top,
        false => (f32::from(value) - (16 << (depth - 8)) as f32) / (219 << (depth - 8)) as f32,
    };
    let byte = |unit: f32| (unit.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    if picture.p.layout == DAV1D_PIXEL_LAYOUT_I400 {
        return Some(y.iter().map(|&value| 0xFF00_0000 | byte(shade(value)) * 0x01_0101).collect());
    }
    let (u, cw, ch, _) = plane(picture, 1)?;
    let (v, ..) = plane(picture, 2)?;
    let tint = |value: u16| match full {
        true => (f32::from(value) - half) / top,
        false => (f32::from(value) - half) / (224 << (depth - 8)) as f32,
    };
    // How much red and blue are of the brightness, for each matrix.
    let (kr, kb) = match matrix {
        DAV1D_MC_BT709 => (0.2126, 0.0722),
        DAV1D_MC_FCC => (0.30, 0.11),
        DAV1D_MC_SMPTE240 => (0.212, 0.087),
        DAV1D_MC_BT2020_NCL | DAV1D_MC_BT2020_CL => (0.2627, 0.0593),
        _ => (0.299, 0.114),
    };
    let kg = 1.0 - kr - kb;
    let (sx, sy) = ((w + cw - 1) / cw.max(1), (h + ch - 1) / ch.max(1));
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        for column in 0..w {
            let at = (row / sy).min(ch - 1) * cw + (column / sx).min(cw - 1);
            let (luma, cb, cr) = (y[row * w + column], u[at], v[at]);
            let (r, g, b) = match matrix {
                // Green, blue and red, as they are.
                DAV1D_MC_IDENTITY => (shade(cr), shade(luma), shade(cb)),
                DAV1D_MC_SMPTE_YCGCO => {
                    let (l, cg, co) = (shade(luma), tint(cb), tint(cr));
                    let t = l - cg;
                    (t + co, l + cg, t - co)
                }
                _ => {
                    let (l, cb, cr) = (shade(luma), tint(cb), tint(cr));
                    let r = l + 2.0 * (1.0 - kr) * cr;
                    let b = l + 2.0 * (1.0 - kb) * cb;
                    (r, (l - kr * r - kb * b) / kg, b)
                }
            };
            out.push(0xFF00_0000 | byte(r) << 16 | byte(g) << 8 | byte(b));
        }
    }
    Some(out)
}
