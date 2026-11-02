//! Pictures as they were drawn last at the size they are shown, kept to
//! be put down again rather than drawn afresh from their pixels each
//! time: while one is dragged, or a box with one in it, all of it is
//! drawn again with each step, and drawing a large one afresh, each of
//! its pixels from four of the picture's, takes far longer than this.

use super::*;
use std::sync::Mutex;

/// A picture drawn on nothing at a size: how it looked, where its middle
/// was in what it was drawn at that size on, and where in that it was
/// drawn, with its pixels.
struct Kept {
    picture: Picture,
    times: f32,
    middle: (f32, f32),
    area: Rect,
    pixels: Arc<Vec<u32>>,
}

/// So many kept, the last drawn first: one for each of a few dragged
/// together.
const MOST_KEPT: usize = 4;

/// The largest kept, in pixels: about a 4K screen of them, 32 MB. One
/// shown larger than that, zoomed far into, is drawn afresh each time,
/// only as much of it as is seen.
const LARGEST: i64 = 1 << 23;

/// All that is kept, in pixels: so many of the largest.
const ALL_KEPT: i64 = 2 * LARGEST;

static KEPT: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

#[cfg(test)]
thread_local! {
    /// Every picture drawn afresh, on this thread, for what is kept to be
    /// told from what it stands for.
    pub(super) static AFRESH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether two pictures are drawn alike, wherever they are: everything
/// that goes into how one is drawn but where it is the same.
fn same_look(a: &Picture, b: &Picture) -> bool {
    // Each said, for one added to a picture to have to be said here too.
    let Picture { image, name: _, cx: _, cy: _, w, h, angle, crop, radius, corners, shadow, opacity, flip, id: _, drop, adjust, chrome, title, insets, screen_scale } = a;
    Arc::ptr_eq(image, &b.image)
        && (*w, *h, *angle, *crop, *radius) == (b.w, b.h, b.angle, b.crop, b.radius)
        && *corners == b.corners
        && (*shadow, *opacity, *flip, *drop) == (b.shadow, b.opacity, b.flip, b.drop)
        && *adjust == b.adjust
        && *chrome == b.chrome
        && *title == b.title
        && *insets == b.insets
        && *screen_scale == b.screen_scale
}

/// A picture drawn onto part of the artboard at `times` its size as it was
/// drawn last, if it was drawn so and is no different but for where it
/// is, by whole pixels; or, where most of it is in this part, drawn on
/// nothing now and kept, then put down. Whether it was: if not, it is
/// for drawing afresh, as only a little of it is.
pub(super) fn draw_kept(out: &mut [u32], part: Rect, picture: &Picture, times: f32, text: &mut Text) -> bool {
    #[cfg(test)]
    if AFRESH.with(std::cell::Cell::get) {
        return false;
    }
    let at = picture_at(picture, times, (0, 0));
    let (layers, lift) = picture_shadows(&at, times);
    let area = picture_area(&at, &layers, lift);
    let seen = area.intersect(part);
    if seen.is_empty() || at.w < 0.5 || at.h < 0.5 {
        return false;
    }
    // Put down by whole pixels, it is drawn exactly as afresh, but for
    // the last shade where it is partly see-through.
    let near = |kept: &Kept| {
        let by = ((at.cx - kept.middle.0).round(), (at.cy - kept.middle.1).round());
        let close = (at.cx - kept.middle.0 - by.0).abs() < 1.0 / 256.0 && (at.cy - kept.middle.1 - by.1).abs() < 1.0 / 256.0;
        (close && kept.times == times && same_look(&kept.picture, picture)).then_some((by.0 as i32, by.1 as i32))
    };
    let found = {
        let mut kept = KEPT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        kept.iter().position(|one| near(one).is_some()).map(|at| {
            let one = kept.remove(at);
            let found = (near(&one).unwrap_or_default(), one.area, one.pixels.clone());
            kept.insert(0, one);
            found
        })
    };
    let (by, from, pixels) = match found {
        Some(found) => found,
        None => {
            let size = i64::from(area.w) * i64::from(area.h);
            if size > LARGEST || 2 * i64::from(seen.w) * i64::from(seen.h) < size {
                return false;
            }
            let mut pixels = vec![0u32; size as usize];
            paint_picture(&mut pixels, area, picture, times, text);
            let pixels = Arc::new(pixels);
            let mut kept = KEPT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            kept.insert(0, Kept { picture: picture.clone(), times, middle: (at.cx, at.cy), area, pixels: pixels.clone() });
            let mut all = 0;
            kept.retain(|one| {
                all += i64::from(one.area.w) * i64::from(one.area.h);
                all <= ALL_KEPT
            });
            kept.truncate(MOST_KEPT);
            ((0, 0), area, pixels)
        }
    };
    let placed = from.moved(by.0, by.1);
    let into = placed.intersect(part);
    if into.is_empty() {
        return true;
    }
    let wide = part.w as usize;
    out[((into.y - part.y) * part.w) as usize..((into.bottom() - part.y) * part.w) as usize].par_chunks_mut(wide).enumerate().for_each(|(row, line)| {
        let y = into.y + row as i32;
        let source = &pixels[((y - placed.y) * placed.w + into.x - placed.x) as usize..][..into.w as usize];
        let target = &mut line[(into.x - part.x) as usize..][..into.w as usize];
        for (pixel, &color) in target.iter_mut().zip(source) {
            match color >> 24 {
                0 => {}
                255 => *pixel = color,
                alpha => *pixel = over(*pixel, color & 0xFF_FFFF, alpha as f32 / 255.0),
            }
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each pixel as it is shown on paper: the same, to a shade, or where
    /// the first that is not.
    fn alike(kept: &[u32], afresh: &[u32], part: Rect) -> Result<(), String> {
        let paper = 0xF7F5F0;
        let shown = |color: u32| canvas::mix(paper, color & 0xFF_FFFF, (color >> 24) + (color >> 31));
        match kept.iter().zip(afresh).position(|(a, b)| [0, 8, 16].iter().any(|shift| ((shown(*a) >> shift & 0xFF) as i32 - (shown(*b) >> shift & 0xFF) as i32).abs() > 2)) {
            None => Ok(()),
            Some(at) => Err(format!("at {},{}: {:08x} for {:08x}", part.x + at as i32 % part.w, part.y + at as i32 / part.w, kept[at], afresh[at])),
        }
    }

    #[test]
    fn a_picture_put_down_as_it_was_kept_is_as_if_drawn_afresh() {
        let mut text = Text::load();
        let (w, h) = (300, 200);
        // Colored all over, and see-through towards its foot.
        let image = Arc::new(Image::new(w, h, (0..w * h).map(|at| {
            let (x, y) = (at % w, at / w);
            let alpha = if y > 150 { 255 - (y - 150) as u32 * 4 } else { 255 };
            alpha << 24 | (x as u32 * 255 / w as u32) << 16 | (y as u32 * 255 / h as u32) << 8 | ((x * 7 ^ y * 13) & 0xFF) as u32
        }).collect()));
        let plain = Picture::new(image.clone(), "one".into(), (200.0, 150.0));
        let mut shadowed = plain.clone();
        (shadowed.w, shadowed.h, shadowed.shadow, shadowed.radius) = (240.0, 160.0, 3, 16.0);
        let mut turned = plain.clone();
        (turned.angle, turned.opacity, turned.flip) = (0.3, 180, (true, false));
        let mut dropped = plain.clone();
        (dropped.shadow, dropped.drop) = (CUSTOM_SHADOW, frame::Layer { x: 6, y: 10, blur: 12, spread: 2, color: 0x7000_0000 });
        // Under it, some opaque, some partly see-through, some nothing.
        let under: Vec<u32> = (0..500 * 400).map(|at| match (at % 500 / 125, at / 500 / 100) {
            (0, _) => 0xFF20_6080,
            (1, row) if row % 2 == 0 => 0x80C0_4020,
            _ => 0,
        }).collect();
        for picture in [plain, shadowed, turned, dropped] {
            for times in [1.0, 1.5] {
                // Drawn and kept, then moved by whole pixels as drawn and
                // put down, all of it and a little of it.
                for (by, part) in [((0, 0), Rect::new(0, 0, 500, 400)), ((7, -3), Rect::new(0, 0, 500, 400)), ((-12, 5), Rect::new(130, 90, 70, 55))] {
                    let mut moved = picture.clone();
                    (moved.cx, moved.cy) = (picture.cx + by.0 as f32 / times, picture.cy + by.1 as f32 / times);
                    let cut = |all: &[u32]| -> Vec<u32> { (part.y..part.bottom()).flat_map(|y| all[(y * 500 + part.x) as usize..(y * 500 + part.right()) as usize].to_vec()).collect() };
                    let (mut kept, mut afresh) = (cut(&under), cut(&under));
                    // Most of it in what is drawn, it is kept if it was not;
                    // a little of it, it is put down as kept unless what
                    // others draw meanwhile have taken its place.
                    let drawn = draw_kept(&mut kept, part, &moved, times, &mut text);
                    assert!(drawn || part.w < 500, "not drawn as kept");
                    if !drawn {
                        paint_picture(&mut kept, part, &moved, times, &mut text);
                    }
                    paint_picture(&mut afresh, part, &moved, times, &mut text);
                    if let Err(off) = alike(&kept, &afresh, part) {
                        panic!("moved by {by:?} at {times}, turned {} with shadow {}: {off}", picture.angle, picture.shadow);
                    }
                }
            }
        }
    }
}
