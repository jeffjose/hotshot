//! A crop kept to the edges found in its picture: its sides snapped to
//! them as they are pulled, made tight to what is in the picture, and
//! fitted round the box of them double clicked in.

use super::edges::{Lines, Sides};
use super::line as line_between;
use super::*;

impl Studio {
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

    /// A point of the window, where it is in a picture's own pixels.
    fn shown_pixel(&self, picture: &Picture, at: (i32, i32)) -> (f32, f32) {
        let (mut lx, mut ly) = picture.local(self.to_doc(at));
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
        let pointer = self.shown_pixel(picture, self.pointer);
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
