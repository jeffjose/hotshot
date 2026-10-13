//! Pixels and the drawing done on them. A pixel is `0RGB` in a `u32`,
//! which is how the X server hands the screen over and takes it back.

use rayon::prelude::*;

/// A rectangle of the screen: its top-left corner and its size.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    /// The rectangle with two points at opposite corners.
    pub fn spanning(a: (i32, i32), b: (i32, i32)) -> Rect {
        Rect::new(a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs())
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// The part of this rectangle that is also in `other`; empty, with
    /// no width or height, when they do not meet.
    pub fn intersect(&self, other: Rect) -> Rect {
        let (x, y) = (self.x.max(other.x), self.y.max(other.y));
        let (right, bottom) = (self.right().min(other.right()), self.bottom().min(other.bottom()));
        Rect::new(x, y, (right - x).max(0), (bottom - y).max(0))
    }

    pub fn intersects(&self, other: Rect) -> bool {
        !self.intersect(other).is_empty()
    }

    /// Grown by `by` on every side, or shrunk when it is negative.
    pub fn inflate(&self, by: i32) -> Rect {
        Rect::new(self.x - by, self.y - by, (self.w + 2 * by).max(0), (self.h + 2 * by).max(0))
    }

    pub fn moved(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Moved, as little as it takes, to lie within `bounds`.
    pub fn kept_within(&self, bounds: Rect) -> Rect {
        let x = self.x.min(bounds.right() - self.w).max(bounds.x);
        let y = self.y.min(bounds.bottom() - self.h).max(bounds.y);
        Rect::new(x, y, self.w, self.h)
    }
}

/// How much of each pixel of a rectangle a shape covers, from 0 to 255:
/// a shape drawn once and laid down as often as the picture is redrawn.
pub struct Mask {
    pub bounds: Rect,
    pub alpha: Vec<u8>,
}

/// Drawing primitives over a buffer of pixels. Everything is clipped to
/// the buffer, and to a rectangle of it when one is set.
pub struct Canvas<'a> {
    pixels: &'a mut [u32],
    pub width: i32,
    pub height: i32,
    clip: Rect,
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [u32], width: i32, height: i32) -> Self {
        Canvas { pixels, width, height, clip: Rect::new(0, 0, width, height) }
    }

    /// Keeps what is drawn from here on to a rectangle.
    pub fn set_clip(&mut self, clip: Rect) {
        self.clip = clip.intersect(Rect::new(0, 0, self.width, self.height));
    }

    pub fn reset_clip(&mut self) {
        self.clip = Rect::new(0, 0, self.width, self.height);
    }

    /// Rows of the part of a rectangle that may be drawn on.
    fn rows(&mut self, x: i32, y: i32, w: i32, h: i32) -> impl Iterator<Item = &mut [u32]> {
        let part = Rect::new(x, y, w.max(0), h.max(0)).intersect(self.clip);
        let (x0, x1) = (part.x as usize, part.right() as usize);
        let rows = self.pixels.chunks_exact_mut(self.width.max(1) as usize);
        rows.skip(part.y.max(0) as usize).take(part.h as usize).map(move |row| &mut row[x0..x1])
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.rows(x, y, w, h).for_each(|row| row.fill(color));
    }

    /// An outline `thickness` wide with rounded corners, `radius` being
    /// that of its outer edge.
    #[allow(clippy::too_many_arguments)]
    pub fn round_outline(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, thickness: i32, color: u32) {
        let band = radius.max(thickness);
        for py in y..y + h {
            // Away from the top and bottom only the two sides have any.
            let full = py < y + band || py >= y + h - band;
            let spans = if full { [(x, x + w), (0, 0)] } else { [(x, x + thickness), (x + w - thickness, x + w)] };
            for (from, to) in spans {
                for px in from..to {
                    let ring = rounded_cover(px, py, x, y, w, h, radius, 0.0) - rounded_cover(px, py, x, y, w, h, radius, f64::from(thickness));
                    self.cover(px, py, color, (ring * 256.0) as u32);
                }
            }
        }
    }

    /// A filled rectangle with rounded corners, `amount`/256 opaque.
    #[allow(clippy::too_many_arguments)]
    pub fn round_fill(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32, amount: u32) {
        let radius = radius.clamp(0, w.min(h) / 2);
        for py in y..y + h {
            // Only the ends of the rows at the top and bottom curve; the
            // rest of every row is plain.
            let curved = py < y + radius || py >= y + h - radius;
            let inset = if curved { radius } else { 0 };
            for px in (x..x + inset).chain(x + w - inset..x + w) {
                let cover = rounded_cover(px, py, x, y, w, h, radius, 0.0);
                self.cover(px, py, color, (cover * f64::from(amount)) as u32);
            }
            for row in self.rows(x + inset, py, w - 2 * inset, 1) {
                row.iter_mut().for_each(|pixel| *pixel = mix(*pixel, color, amount.min(256)));
            }
        }
    }

    /// Rounds the corners of a rectangle off, by putting back outside
    /// the curve what is said to belong at each pixel there.
    pub fn round_off_with(&mut self, rect: Rect, radius: i32, under: impl Fn(i32, i32) -> u32) {
        let radius = radius.clamp(0, rect.w.min(rect.h) / 2);
        for (corner_x, corner_y) in [(rect.x, rect.y), (rect.right() - radius, rect.y), (rect.x, rect.bottom() - radius), (rect.right() - radius, rect.bottom() - radius)] {
            for py in corner_y.max(0)..(corner_y + radius).min(self.height) {
                for px in corner_x.max(0)..(corner_x + radius).min(self.width) {
                    let inside = rounded_cover(px, py, rect.x, rect.y, rect.w, rect.h, radius, 0.0);
                    self.cover(px, py, under(px, py), ((1.0 - inside) * 256.0) as u32);
                }
            }
        }
    }

    /// Darkens an area, keeping `keep`/256 of its brightness, but for
    /// some rectangles of it, each with corners of a radius, which are
    /// left as they are: spotlights on what is to be looked at.
    pub fn spotlight(&mut self, area: Rect, lit: &[(Rect, i32)], keep: u32) {
        let part = area.intersect(self.clip);
        if part.is_empty() {
            return;
        }
        let width = self.width as usize;
        let row = |(row, out): (usize, &mut [u32])| {
            let y = part.y + row as i32;
            // Dark up to each rectangle that this row crosses and after
            // the last, taking them from the left.
            let mut crossed: Vec<(Rect, i32)> = lit.iter().copied().filter(|(rect, _)| y >= rect.y && y < rect.bottom() && rect.w > 0).collect();
            crossed.sort_by_key(|(rect, _)| rect.x);
            let mut from = part.x;
            for (rect, _) in &crossed {
                let upto = rect.x.clamp(from, part.right());
                out[from as usize..upto as usize].iter_mut().for_each(|pixel| *pixel = mix(0, *pixel, keep));
                from = rect.right().clamp(from, part.right());
            }
            out[from as usize..part.right() as usize].iter_mut().for_each(|pixel| *pixel = mix(0, *pixel, keep));
            // And where the row cuts a rectangle's rounded corner, dark
            // outside the curve of it.
            for (rect, radius) in &crossed {
                let radius = (*radius).clamp(0, rect.w.min(rect.h) / 2);
                if y >= rect.y + radius && y < rect.bottom() - radius {
                    continue;
                }
                for x in (rect.x..rect.x + radius).chain(rect.right() - radius..rect.right()) {
                    if x >= part.x && x < part.right() {
                        let inside = rounded_cover(x, y, rect.x, rect.y, rect.w, rect.h, radius, 0.0);
                        let pixel = &mut out[x as usize];
                        *pixel = mix(mix(0, *pixel, keep), *pixel, (inside * 256.0) as u32);
                    }
                }
            }
        };
        let rows = &mut self.pixels[part.y as usize * width..part.bottom() as usize * width];
        match part.w as usize * part.h as usize >= SHARE {
            true => rows.par_chunks_exact_mut(width).enumerate().for_each(row),
            false => rows.chunks_exact_mut(width).enumerate().for_each(row),
        }
    }

    /// Draws what is in one rectangle again in another, larger or
    /// smaller, each pixel a blend of the four it falls among.
    ///
    /// With `round`, only as much of the second as an oval set in it
    /// covers: a lens, not a window.
    pub fn magnify(&mut self, from: Rect, into: Rect, round: bool) {
        let source = from.intersect(Rect::new(0, 0, self.width, self.height));
        let part = into.intersect(self.clip);
        if source.is_empty() || part.is_empty() || into.is_empty() {
            return;
        }
        // Taken first: where the two overlap, it is the picture as it
        // was that is wanted, not as it is being made.
        let taken = crop(self.pixels, self.width, source);
        let at = |x: i32, y: i32| taken[((y.clamp(0, source.h - 1)) * source.w + x.clamp(0, source.w - 1)) as usize];
        for y in part.y..part.bottom() {
            let down = ((y - into.y) as i64 * 256 * from.h as i64 / into.h as i64) as i32 + (from.y - source.y) * 256;
            for x in part.x..part.right() {
                if round {
                    let (dx, dy) = ((x - into.x) as f32 + 0.5 - into.w as f32 / 2.0, (y - into.y) as f32 + 0.5 - into.h as f32 / 2.0);
                    if (dx / (into.w as f32 / 2.0)).powi(2) + (dy / (into.h as f32 / 2.0)).powi(2) > 1.0 {
                        continue;
                    }
                }
                let across = ((x - into.x) as i64 * 256 * from.w as i64 / into.w as i64) as i32 + (from.x - source.x) * 256;
                let (sx, sy, fx, fy) = (across >> 8, down >> 8, (across & 255) as u32, (down & 255) as u32);
                let (upper, lower) = (mix(at(sx, sy), at(sx + 1, sy), fx), mix(at(sx, sy + 1), at(sx + 1, sy + 1), fx));
                self.pixels[(y * self.width + x) as usize] = mix(upper, lower, fy);
            }
        }
    }

    /// A filled circle about a point.
    pub fn disc(&mut self, cx: i32, cy: i32, radius: i32, color: u32, amount: u32) {
        self.round_fill(cx - radius, cy - radius, 2 * radius, 2 * radius, radius, color, amount);
    }

    fn cover(&mut self, x: i32, y: i32, color: u32, amount: u32) {
        if amount > 0 && self.clip.contains(x, y) {
            let pixel = &mut self.pixels[(y * self.width + x) as usize];
            *pixel = mix(*pixel, color, amount.min(256));
        }
    }

    /// Blends one pixel of a letter in.
    pub fn blend(&mut self, x: i32, y: i32, color: u32, coverage: u8) {
        if self.clip.contains(x, y) {
            let pixel = &mut self.pixels[(y * self.width + x) as usize];
            let amount = u32::from(coverage) + u32::from(coverage >> 7);
            *pixel = mix(*pixel, color, amount);
        }
    }

    /// Lays a shape down in a color, `amount`/256 opaque where the shape
    /// covers a pixel whole. A large one has its rows done each on its
    /// own core.
    pub fn mask(&mut self, mask: &Mask, color: u32, amount: u32) {
        let part = mask.bounds.intersect(self.clip);
        if part.is_empty() {
            return;
        }
        let (width, across) = (self.width as usize, mask.bounds.w as usize);
        let (left, skip) = (part.x as usize, (part.x - mask.bounds.x) as usize);
        let first = (part.y - mask.bounds.y) as usize;
        let amount = amount.min(256);
        let draw = |(row, out): (usize, &mut [u32])| {
            let alpha = &mask.alpha[(first + row) * across + skip..][..part.w as usize];
            for (pixel, &cover) in out[left..left + part.w as usize].iter_mut().zip(alpha) {
                if cover > 0 {
                    *pixel = mix(*pixel, color, ((u32::from(cover) + u32::from(cover >> 7)) * amount) >> 8);
                }
            }
        };
        let rows = &mut self.pixels[part.y as usize * width..part.bottom() as usize * width];
        match part.w as usize * part.h as usize >= SHARE {
            true => rows.par_chunks_exact_mut(width).enumerate().for_each(draw),
            false => rows.chunks_exact_mut(width).enumerate().for_each(draw),
        }
    }

    /// Turns what is in a rectangle into a mosaic of squares `cell`
    /// across, each the average of what was under it: what was there can
    /// no longer be read.
    pub fn pixelate(&mut self, area: Rect, cell: i32) {
        let part = area.intersect(self.clip);
        let cell = cell.max(2);
        if part.is_empty() {
            return;
        }
        // The squares are counted from the corner of the whole area, so
        // that they stay put when only a part of it is drawn.
        let width = self.width as usize;
        let mut top = area.y + (part.y - area.y) / cell * cell;
        while top < part.bottom() {
            let mut left = area.x + (part.x - area.x) / cell * cell;
            while left < part.right() {
                let square = Rect::new(left, top, cell, cell).intersect(area).intersect(Rect::new(0, 0, self.width, self.height));
                let (mut red, mut green, mut blue, mut count) = (0u32, 0u32, 0u32, 0u32);
                for y in square.y..square.bottom() {
                    for pixel in &self.pixels[y as usize * width + square.x as usize..][..square.w as usize] {
                        red += pixel >> 16 & 0xFF;
                        green += pixel >> 8 & 0xFF;
                        blue += pixel & 0xFF;
                        count += 1;
                    }
                }
                if count > 0 {
                    let color = ((red / count) << 16) | ((green / count) << 8) | (blue / count);
                    self.fill(square.x, square.y, square.w, square.h, color);
                }
                left += cell;
            }
            top += cell;
        }
    }

    /// Draws the pixels of `source` about `(cx, cy)` enlarged, each one a
    /// square `cell` across, `cells` of them each way, with the top-left
    /// of the whole at `(x, y)`. Off the edge of the source is black.
    #[allow(clippy::too_many_arguments)]
    pub fn enlarge(&mut self, source: &[u32], source_width: i32, cx: i32, cy: i32, x: i32, y: i32, cells: i32, cell: i32) {
        let source_height = source.len() as i32 / source_width.max(1);
        for row in 0..cells {
            for column in 0..cells {
                let (sx, sy) = (cx - cells / 2 + column, cy - cells / 2 + row);
                let inside = sx >= 0 && sy >= 0 && sx < source_width && sy < source_height;
                let color = if inside { source[(sy * source_width + sx) as usize] & 0xFFFFFF } else { 0 };
                self.fill(x + column * cell, y + row * cell, cell, cell, color);
            }
        }
    }
}

/// How much of the pixel at `(px, py)`, from 0 to 1, lies within a
/// rectangle with corners of `radius`, shrunk by `inset` on every side.
/// Fractions along the edge are what make the curve smooth.
#[allow(clippy::too_many_arguments)]
fn rounded_cover(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32, radius: i32, inset: f64) -> f64 {
    let (half_w, half_h) = (f64::from(w) / 2.0 - inset, f64::from(h) / 2.0 - inset);
    let radius = (f64::from(radius) - inset).clamp(0.0, half_w.min(half_h).max(0.0));
    // Distance from the middle, less the straight part of each side.
    let across = (f64::from(px) + 0.5 - f64::from(x) - f64::from(w) / 2.0).abs() - (half_w - radius);
    let down = (f64::from(py) + 0.5 - f64::from(y) - f64::from(h) / 2.0).abs() - (half_h - radius);
    let outside = across.max(0.0).hypot(down.max(0.0)) + across.max(down).min(0.0) - radius;
    (0.5 - outside).clamp(0.0, 1.0)
}

/// Says in the top byte of each pixel of a picture how opaque it is:
/// wholly, but for corners rounded to `radius`, which fade to nothing
/// outside the curve.
pub fn round_alpha(pixels: &mut [u32], width: i32, radius: i32) {
    let height = pixels.len() as i32 / width.max(1);
    let radius = radius.clamp(0, width.min(height) / 2);
    pixels.iter_mut().for_each(|pixel| *pixel |= 0xFF00_0000);
    for (corner_x, corner_y) in [(0, 0), (width - radius, 0), (0, height - radius), (width - radius, height - radius)] {
        for py in corner_y..corner_y + radius {
            for px in corner_x..corner_x + radius {
                let inside = (rounded_cover(px, py, 0, 0, width, height, radius, 0.0) * 255.0).round() as u32;
                let pixel = &mut pixels[(py * width + px) as usize];
                *pixel = (*pixel & 0xFF_FFFF) | (inside << 24);
            }
        }
    }
}

/// Blends `amount`/256 of `b` into `a`, two channels at a time.
pub fn mix(a: u32, b: u32, amount: u32) -> u32 {
    let lerp = |mask: u32| (((a & mask) * (256 - amount) + (b & mask) * amount) >> 8) & mask;
    lerp(0xFF00FF) | lerp(0x00FF00)
}

/// How many pixels each core is handed at a time when a whole picture is
/// worked over.
const SHARE: usize = 1 << 16;

/// Copies one whole picture over another of the same size, on every core:
/// at 4K that is 33 MB, which one core takes several milliseconds over.
pub fn copy_into(into: &mut [u32], from: &[u32]) {
    into.par_chunks_mut(SHARE).zip(from.par_chunks(SHARE)).for_each(|(into, from)| into.copy_from_slice(from));
}

/// Writes a darkened copy of a picture, keeping `keep`/256 of its
/// brightness, on every core.
pub fn darken_into(into: &mut [u32], from: &[u32], keep: u32) {
    into.par_chunks_mut(SHARE).zip(from.par_chunks(SHARE)).for_each(|(into, from)| {
        for (pixel, from) in into.iter_mut().zip(from) {
            *pixel = mix(0, *from, keep);
        }
    });
}

/// Copies a rectangle of one picture into the same place in another of
/// the same size.
pub fn copy_rect(into: &mut [u32], from: &[u32], width: i32, rect: Rect) {
    let height = into.len() as i32 / width.max(1);
    let part = rect.intersect(Rect::new(0, 0, width, height));
    if part.is_empty() {
        return;
    }
    let (width, left, across) = (width as usize, part.x as usize, part.w as usize);
    let rows = part.y as usize * width..part.bottom() as usize * width;
    let copy = |(into, from): (&mut [u32], &[u32])| into[left..left + across].copy_from_slice(&from[left..left + across]);
    match across * part.h as usize >= SHARE {
        true => into[rows.clone()].par_chunks_exact_mut(width).zip(from[rows].par_chunks_exact(width)).for_each(copy),
        false => into[rows.clone()].chunks_exact_mut(width).zip(from[rows].chunks_exact(width)).for_each(copy),
    }
}

/// A rectangle of a picture, as a picture of its own.
pub fn crop(from: &[u32], width: i32, rect: Rect) -> Vec<u32> {
    let mut out = Vec::with_capacity(rect.w as usize * rect.h as usize);
    for y in rect.y..rect.bottom() {
        out.extend_from_slice(&from[(y * width + rect.x) as usize..][..rect.w as usize]);
    }
    out
}

/// The mouse pointer as the server draws it: where on the screen its
/// top-left corner is, how wide and tall it is, and its pixels, each
/// with how opaque it is in its top byte and its colors already made
/// that much fainter.
#[derive(Clone, Debug, PartialEq)]
pub struct Pointer {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub pixels: Vec<u32>,
}

impl Pointer {
    /// Draws it into a picture so many pixels wide whose top-left corner
    /// is at `origin` on the screen. What the picture has in the top
    /// byte of each pixel is left as it is.
    pub fn stamp(&self, into: &mut [u32], width: i32, origin: (i32, i32)) {
        let height = into.len() as i32 / width.max(1);
        let part = Rect::new(self.x - origin.0, self.y - origin.1, self.w, self.h).intersect(Rect::new(0, 0, width, height));
        for y in part.y..part.bottom() {
            for x in part.x..part.right() {
                let over = self.pixels[((y + origin.1 - self.y) * self.w + x + origin.0 - self.x) as usize];
                let under = &mut into[(y * width + x) as usize];
                let through = 255 - (over >> 24);
                let part = |shift: u32| (((over >> shift) & 0xFF) + ((*under >> shift) & 0xFF) * through / 255).min(255) << shift;
                *under = *under & 0xFF00_0000 | part(16) | part(8) | part(0);
            }
        }
    }
}

/// How many rows are looked at together when telling what has changed.
const BAND: usize = 32;
/// More rectangles than this are sent as the one that holds them all.
const MOST_PARTS: usize = 48;

/// Where one picture differs from another of the same size, as a few
/// rectangles, bringing the other up to date as it goes. Handing the
/// display only these is what keeps a small change cheap on a large
/// screen: the whole of a 4K frame is 33 MB.
pub fn changed(now: &[u32], was: &mut [u32], width: usize) -> Vec<Rect> {
    if width == 0 || now.len() != was.len() {
        return Vec::new();
    }
    // For each band of rows, the columns between which something changed.
    let bands: Vec<Option<(usize, usize)>> = now
        .par_chunks(width * BAND)
        .zip(was.par_chunks_mut(width * BAND))
        .map(|(now, was)| {
            if now == was {
                return None;
            }
            let mut span: Option<(usize, usize)> = None;
            for (now, was) in now.chunks(width).zip(was.chunks(width)) {
                let Some(first) = now.iter().zip(was).position(|(a, b)| a != b) else { continue };
                let last = now.iter().zip(was).rposition(|(a, b)| a != b).unwrap_or(first);
                span = Some(span.map_or((first, last), |(left, right)| (left.min(first), right.max(last))));
            }
            was.copy_from_slice(now);
            span
        })
        .collect();
    let height = now.len() / width;
    let mut parts: Vec<Rect> = Vec::new();
    let mut open: Option<(usize, usize, usize)> = None;
    let mut close = |open: &mut Option<(usize, usize, usize)>, end: usize| {
        if let Some((top, left, right)) = open.take() {
            let bottom = (end * BAND).min(height);
            parts.push(Rect::new(left as i32, (top * BAND) as i32, (right - left + 1) as i32, (bottom - top * BAND) as i32));
        }
    };
    for (band, span) in bands.iter().enumerate() {
        match (span, &mut open) {
            // Bands one under another are one rectangle, as wide as the
            // widest, unless that would take in far more than changed.
            (Some((first, last)), Some((_, left, right))) if first.abs_diff(*left) < 64 && last.abs_diff(*right) < 64 => {
                (*left, *right) = ((*left).min(*first), (*right).max(*last));
            }
            (Some((first, last)), _) => {
                close(&mut open, band);
                open = Some((band, *first, *last));
            }
            (None, _) => close(&mut open, band),
        }
    }
    close(&mut open, bands.len());
    if parts.len() > MOST_PARTS {
        let left = parts.iter().map(|part| part.x).min().unwrap_or(0);
        let right = parts.iter().map(Rect::right).max().unwrap_or(0);
        let (top, bottom) = (parts[0].y, parts[parts.len() - 1].bottom());
        parts = vec![Rect::new(left, top, right - left, bottom - top)];
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_is_drawn_where_it_was_and_no_further_than_the_picture() {
        // Two pixels: one all white, one half black; half off the top.
        let pointer = Pointer { x: 5, y: -1, w: 1, h: 2, pixels: vec![0xFFFF_FFFF, 0x8000_0000] };
        let mut picture = vec![0xAB20_4060u32; 4 * 2];
        pointer.stamp(&mut picture, 4, (3, 0));
        assert_eq!(picture[2], 0xAB0F_1F2F);
        assert_eq!(picture.iter().filter(|&&pixel| pixel == 0xAB20_4060).count(), 7);
    }

    #[test]
    fn only_what_differs_is_reported_and_then_nothing() {
        let (width, height) = (200, 100);
        let mut was = vec![0u32; width * height];
        let mut now = was.clone();
        assert!(changed(&now, &mut was, width).is_empty());
        // A dot near the top and a bar lower down, in different bands.
        now[3 * width + 10] = 1;
        now[70 * width + 50..70 * width + 90].fill(2);
        assert_eq!(changed(&now, &mut was, width), [Rect::new(10, 0, 1, 32), Rect::new(50, 64, 40, 32)]);
        assert_eq!(was, now);
        assert!(changed(&now, &mut was, width).is_empty());
        // Rows of a last band that is short of full are counted as they are.
        now[99 * width + 199] = 3;
        assert_eq!(changed(&now, &mut was, width), [Rect::new(199, 96, 1, 4)]);
    }

    #[test]
    fn drawing_stays_inside_the_clip() {
        let mut pixels = vec![0u32; 10 * 10];
        let mut canvas = Canvas::new(&mut pixels, 10, 10);
        canvas.set_clip(Rect::new(2, 2, 3, 3));
        canvas.fill(-5, -5, 30, 30, 7);
        canvas.round_fill(0, 0, 10, 10, 0, 7, 256);
        assert_eq!(pixels.iter().filter(|&&pixel| pixel == 7).count(), 9);
        assert_eq!(pixels[2 * 10 + 2], 7);
        assert_eq!(pixels[5 * 10 + 5], 0);
    }

    #[test]
    fn a_mosaic_is_the_average_of_what_was_under_each_square() {
        let mut pixels = vec![0u32; 4 * 2];
        pixels[..4].copy_from_slice(&[0x000000, 0x202020, 0xFF0000, 0xFF0000]);
        pixels[4..].copy_from_slice(&[0x202020, 0x000000, 0xFF0000, 0xFF0000]);
        Canvas::new(&mut pixels, 4, 2).pixelate(Rect::new(0, 0, 4, 2), 2);
        assert_eq!(pixels, [0x101010, 0x101010, 0xFF0000, 0xFF0000, 0x101010, 0x101010, 0xFF0000, 0xFF0000]);
    }

    #[test]
    fn rectangles_meet_and_stay_within_bounds() {
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(a.intersect(Rect::new(5, 5, 10, 10)), Rect::new(5, 5, 5, 5));
        assert!(a.intersect(Rect::new(20, 0, 5, 5)).is_empty());
        assert_eq!(Rect::spanning((8, 2), (3, 9)), Rect::new(3, 2, 5, 7));
        assert_eq!(Rect::new(95, -4, 10, 10).kept_within(Rect::new(0, 0, 100, 100)), Rect::new(90, 0, 10, 10));
    }
}
