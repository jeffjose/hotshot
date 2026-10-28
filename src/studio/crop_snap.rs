//! A crop kept to the edges found in its picture: its sides snapped to
//! them as they are pulled, made tight to what is in the picture, and
//! fitted round the box of them double clicked in.

use super::edges::{Lines, Sides};
use super::line as line_between;
use super::*;

/// A crop, (x, y, w, h), by its sides; and back.
fn sides((x, y, w, h): (f32, f32, f32, f32)) -> Sides {
    (x, y, x + w, y + h)
}

fn crop((l, t, r, b): Sides) -> (f32, f32, f32, f32) {
    (l, t, r - l, b - t)
}

/// Whether two boxes are the same, to within a pixel.
fn same(a: Sides, b: Sides) -> bool {
    [a.0 - b.0, a.1 - b.1, a.2 - b.2, a.3 - b.3].iter().all(|by| by.abs() < 1.0)
}

/// Whether box `a` is inside box `b`, and not the same as it.
fn within(a: Sides, b: Sides) -> bool {
    a.0 >= b.0 - 0.5 && a.1 >= b.1 - 0.5 && a.2 <= b.2 + 0.5 && a.3 <= b.3 + 0.5 && !same(a, b)
}

/// A box made `ratio` wide to high, by growing it about its middle, kept
/// inside a picture `iw` by `ih` and made smaller if it has to be.
fn to_ratio((l, t, r, b): Sides, ratio: f32, iw: f32, ih: f32) -> Sides {
    let (mut w, mut h) = (r - l, b - t);
    if w / h.max(1e-3) < ratio {
        w = h * ratio;
    } else {
        h = w / ratio;
    }
    let shrink = (iw / w).min(ih / h).min(1.0);
    (w, h) = (w * shrink, h * shrink);
    let x = ((l + r - w) / 2.0).clamp(0.0, iw - w);
    let y = ((t + b - h) / 2.0).clamp(0.0, ih - h);
    (x, y, x + w, y + h)
}

impl Picture {
    /// Cropped to `crop`, of its own pixels, shown at the same size per
    /// pixel and with its pixels where they were on the artboard.
    pub(super) fn recrop(&mut self, crop: (f32, f32, f32, f32)) {
        let (per_x, per_y) = (self.w / self.crop.2.max(1e-3), self.h / self.crop.3.max(1e-3));
        let mut by = ((crop.0 + crop.2 / 2.0 - self.crop.0 - self.crop.2 / 2.0) * per_x, (crop.1 + crop.3 / 2.0 - self.crop.1 - self.crop.3 / 2.0) * per_y);
        if self.flip.0 {
            by.0 = -by.0;
        }
        if self.flip.1 {
            by.1 = -by.1;
        }
        let (sin, cos) = self.angle.sin_cos();
        (self.cx, self.cy) = (self.cx + by.0 * cos - by.1 * sin, self.cy + by.0 * sin + by.1 * cos);
        self.crop = crop;
        (self.w, self.h) = (crop.2 * per_x, crop.3 * per_y);
    }
}

impl Studio {
    /// The picture being cropped, or the one picked out begun to be: its
    /// layer, and it.
    fn crop_picture(&mut self) -> Option<(usize, Picture)> {
        if self.cropping.is_none() {
            self.cropping = self.picked().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_)));
        }
        let index = self.cropping?;
        match &self.doc.layers.get(index)?.kind {
            Kind::Picture(picture) => Some((index, picture.clone())),
            Kind::Shape(_) => None,
        }
    }

    /// The picture being cropped cropped to these sides, kept to the shape
    /// chosen for its crops if there is one: a step to undo.
    fn crop_to_sides(&mut self, index: usize, picture: &Picture, mut to: Sides) {
        let (iw, ih) = (picture.image.width() as f32, picture.image.height() as f32);
        let ratio = match panel::CROP_ASPECTS[self.crop_aspect].1 {
            chosen if chosen < 0.0 => iw / ih,
            chosen => chosen,
        };
        if ratio > 0.0 {
            to = to_ratio(to, ratio, iw, ih);
        }
        self.checkpoint();
        let mut picture = picture.clone();
        picture.recrop(crop(to));
        self.doc.layers[index].kind = Kind::Picture(picture);
        self.changed();
    }

    /// The crop made tight: in to the first of the crops `Lines::tight`
    /// goes through inside it, or on to the next one in from the one it
    /// is on, round to the first again after the last; with `outward`,
    /// back out to the one before, and from the first to all of the
    /// picture. Each is a step to undo, and says which of them it is.
    pub(super) fn tight_crop(&mut self, outward: bool) {
        let Some((index, picture)) = self.crop_picture() else {
            self.say("Pick out a picture to crop tight");
            return;
        };
        let lines = self.crop_lines(&picture.image);
        let began = std::time::Instant::now();
        let crops = lines.tight();
        debug!("canvas found {} tight crops in {:.1?}", crops.len(), began.elapsed());
        if crops.is_empty() {
            self.say("Nothing in the picture to crop tight to");
            return;
        }
        let now = sides(picture.crop);
        let whole = (0.0, 0.0, picture.image.width() as f32, picture.image.height() as f32);
        // Which it is on: the one last gone to, if it is still as that
        // left it, kept to a shape or not; or the one it is the same as.
        let on = match self.tight {
            Some((at, made)) if at < crops.len() && same(made, now) => Some(at),
            _ => crops.iter().position(|&crop| same(crop, now)),
        };
        let next = match (on, outward) {
            (Some(at), false) => Some((at + 1) % crops.len()),
            (Some(at), true) => at.checked_sub(1),
            (None, false) => Some(crops.iter().position(|&crop| within(crop, now)).unwrap_or(0)),
            (None, true) => crops.iter().rposition(|&crop| within(now, crop)),
        };
        let to = next.map_or(whole, |at| crops[at]);
        self.crop_to_sides(index, &picture, to);
        let made = match &self.doc.layers[index].kind {
            Kind::Picture(picture) => sides(picture.crop),
            Kind::Shape(_) => to,
        };
        self.tight = next.map(|at| (at, made));
        match next {
            Some(at) => self.say(format!("Tight \u{b7} {} of {}", at + 1, crops.len())),
            None => self.say("Tight \u{b7} all of the picture"),
        }
    }

    /// The crop of the picture being cropped fitted to the smallest box of
    /// lines round a point of the artboard double clicked in it; and from
    /// a box, to the next larger round it. A step to undo.
    pub(super) fn crop_around(&mut self, index: usize, at: (f32, f32)) {
        let Some(Layer { kind: Kind::Picture(picture), .. }) = self.doc.layers.get(index).cloned() else { return };
        let boxes = self.crop_lines(&picture.image).around(Self::pixel_at(&picture, at));
        let now = sides(picture.crop);
        let next = match boxes.iter().position(|&found| same(found, now)) {
            Some(on) => boxes.get(on + 1).or(boxes.first()),
            None => boxes.first(),
        };
        if let Some(&to) = next {
            self.crop_to_sides(index, &picture, to);
            self.tight = None;
        }
    }

    /// The lines of a picture: found the first time it is cropped, and
    /// kept for as long as it is the one cropped.
    pub(super) fn crop_lines(&mut self, image: &Arc<Image>) -> Arc<Lines> {
        if let Some((known, lines)) = &self.crop_edges {
            if std::ptr::eq(known.as_ptr(), Arc::as_ptr(image)) {
                return lines.clone();
            }
        }
        let began = std::time::Instant::now();
        let (w, h, pixels) = &image.levels[0];
        let lines = Arc::new(Lines::find(*w, *h, pixels));
        debug!("canvas found {} lines down and {} across a picture {w} by {h} in {:.1?}", lines.upright.len(), lines.across.len(), began.elapsed());
        self.crop_edges = Some((Arc::downgrade(image), lines.clone()));
        lines
    }

    /// The sides of a crop being pulled by `pull`, or moved whole, brought
    /// onto the nearest lines of its picture within `reach` of its own
    /// pixels, across and down: the sides pulled each on their own, or
    /// all of it by whichever side is nearest one.
    pub(super) fn snap_crop(&mut self, image: &Arc<Image>, (mut left, mut top, mut right, mut bottom): Sides, pull: (i32, i32), reach: (f32, f32)) -> Sides {
        let lines = self.crop_lines(image);
        let (iw, ih) = (image.width() as f32, image.height() as f32);
        if pull == (0, 0) {
            let by = |lines: &[edges::Line], low: f32, high: f32, reach: f32, whole: f32| {
                [low, high]
                    .into_iter()
                    .filter_map(|side| Lines::nearest(lines, side, reach).map(|to| to - side))
                    .filter(|by| low + by >= 0.0 && high + by <= whole)
                    .min_by(|a, b| a.abs().total_cmp(&b.abs()))
                    .unwrap_or(0.0)
            };
            let (dx, dy) = (by(&lines.upright, left, right, reach.0, iw), by(&lines.across, top, bottom, reach.1, ih));
            return (left + dx, top + dy, right + dx, bottom + dy);
        }
        let near = |lines: &[edges::Line], side: f32, reach: f32| Lines::nearest(lines, side, reach).unwrap_or(side);
        match pull.0 {
            -1 => left = near(&lines.upright, left, reach.0).min(right - 8.0),
            1 => right = near(&lines.upright, right, reach.0).max(left + 8.0),
            _ => {}
        }
        match pull.1 {
            -1 => top = near(&lines.across, top, reach.1).min(bottom - 8.0),
            1 => bottom = near(&lines.across, bottom, reach.1).max(top + 8.0),
            _ => {}
        }
        (left, top, right, bottom)
    }

    /// The sides of a crop moved from where `was` had them that are on a
    /// line of its picture, to be drawn across it.
    pub(super) fn show_crop_snaps(&mut self, was: &Picture, (left, top, right, bottom): Sides) {
        let lines = self.crop_lines(&was.image);
        let (l, t, w, h) = was.crop;
        let on = |lines: &[edges::Line], side: f32| lines.iter().any(|line| (line.at - side).abs() < 0.01);
        self.crop_snaps.clear();
        for (upright, side, before) in [(true, left, l), (true, right, l + w), (false, top, t), (false, bottom, t + h)] {
            if side != before && on(if upright { &lines.upright } else { &lines.across }, side) && !self.crop_snaps.contains(&(upright, side)) {
                self.crop_snaps.push((upright, side));
            }
        }
    }

    /// A point of a picture's own pixels, where it is in the window.
    fn pixel_shown(&self, picture: &Picture, (x, y): (f32, f32)) -> (i32, i32) {
        let (per_x, per_y) = (picture.w / picture.crop.2.max(1e-3), picture.h / picture.crop.3.max(1e-3));
        let (mut lx, mut ly) = ((x - picture.crop.0) * per_x - picture.w / 2.0, (y - picture.crop.1) * per_y - picture.h / 2.0);
        if picture.flip.0 {
            lx = -lx;
        }
        if picture.flip.1 {
            ly = -ly;
        }
        self.to_window(picture.placed((lx, ly)))
    }

    /// A point of the artboard, where it is in a picture's own pixels.
    fn pixel_at(picture: &Picture, at: (f32, f32)) -> (f32, f32) {
        let (mut lx, mut ly) = picture.local(at);
        if picture.flip.0 {
            lx = -lx;
        }
        if picture.flip.1 {
            ly = -ly;
        }
        let (per_x, per_y) = (picture.crop.2 / picture.w.max(1e-3), picture.crop.3 / picture.h.max(1e-3));
        (picture.crop.0 + (lx + picture.w / 2.0) * per_x, picture.crop.1 + (ly + picture.h / 2.0) * per_y)
    }

    /// Over a picture whose crop is being pulled: each line it has
    /// snapped to drawn across all of the picture, with a dot where the
    /// crop meets it; and short and faint, by the pointer, the other lines
    /// near the sides pulled, that it would snap to.
    pub(super) fn draw_crop_snaps(&mut self, canvas: &mut Canvas, picture: &Picture) {
        let Drag::Crop { pull, .. } = self.drag else { return };
        if self.mods.ctrl {
            return;
        }
        let lines = self.crop_lines(&picture.image);
        let (iw, ih) = (picture.image.width() as f32, picture.image.height() as f32);
        let (l, t, w, h) = picture.crop;
        let pointer = Self::pixel_at(picture, self.to_doc(self.pointer));
        // How many of its pixels the ticks are long, and how far from the
        // sides pulled lines are shown: so many of the window's.
        let per = picture.crop.2 / picture.w.max(1e-3) / self.zoom;
        let (tick, near) = (6.0 * per, 60.0 * per);
        for (upright, side) in [(true, if pull.0 < 0 { l } else { l + w }), (false, if pull.1 < 0 { t } else { t + h })] {
            if pull == (0, 0) || (upright && pull.0 == 0) || (!upright && pull.1 == 0) {
                continue;
            }
            let list = if upright { &lines.upright } else { &lines.across };
            for line in list.iter().filter(|line| (line.at - side).abs() <= near && !self.crop_snaps.contains(&(upright, line.at))) {
                let (a, b) = match upright {
                    true => ((line.at, pointer.1 - tick), (line.at, pointer.1 + tick)),
                    false => ((pointer.0 - tick, line.at), (pointer.0 + tick, line.at)),
                };
                line_between(canvas, self.pixel_shown(picture, a), self.pixel_shown(picture, b), 0xB4B6C0);
            }
        }
        for &(upright, at) in &self.crop_snaps {
            let (a, b, middle) = match upright {
                true => ((at, 0.0), (at, ih), (at, t + h / 2.0)),
                false => ((0.0, at), (iw, at), (l + w / 2.0, at)),
            };
            line_between(canvas, self.pixel_shown(picture, a), self.pixel_shown(picture, b), GUIDE);
            let dot = self.pixel_shown(picture, middle);
            canvas.round_fill(dot.0 - 4, dot.1 - 4, 9, 9, 5, GUIDE, 256);
            canvas.round_fill(dot.0 - 2, dot.1 - 2, 5, 5, 3, 0xFFFFFF, 256);
        }
    }
}
