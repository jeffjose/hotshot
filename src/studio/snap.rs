//! Snapping, on the canvas, to the edges, middles and spacing of the rest,
//! as Figma does: of the layers of a frame, for what is moved in it; and of
//! the frames of a page, for a frame moved on it. How is in `spacing`, which
//! capture mode shares.

use super::*;
use crate::spacing;
pub(super) use crate::spacing::{nearest_size, Edges, Gap};

impl Studio {
    /// The boxes of the layers not picked out, as edges.
    pub(super) fn other_edges(&mut self) -> Vec<Edges> {
        let chosen = self.chosen();
        (0..self.doc.layers.len())
            .filter(|index| !chosen.contains(index) && !self.doc.layers[*index].hidden)
            .map(|index| {
                let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
                [x, y, x + w, y + h]
            })
            .collect()
    }

    /// The boxes of what is on the page beside what is worked in, where
    /// they are on that, as Figma snaps to everything near: in a frame,
    /// the page's own layers; on the page, the frames and the layers in
    /// each.
    pub(super) fn outside_edges(&mut self) -> Vec<Edges> {
        let (x0, y0) = (self.doc.x, self.doc.y);
        let mut boxes = Vec::new();
        let add = |boxes: &mut Vec<Edges>, layers: &[Layer], (dx, dy): (f32, f32), text: &mut Text| {
            for layer in layers.iter().filter(|layer| !layer.hidden) {
                let (x, y, w, h) = layer.bounds(text);
                boxes.push([x + dx, y + dy, x + w + dx, y + h + dy]);
            }
        };
        match self.on_page() {
            true => {
                for index in 0..self.frame_count() {
                    let frame = &self.pages[self.page].frames[index];
                    let (fx, fy, layers) = (frame.x, frame.y, frame.layers.clone());
                    boxes.push([fx, fy, fx + frame.width as f32, fy + frame.height as f32]);
                    add(&mut boxes, &layers, (fx - x0, fy - y0), &mut self.text);
                }
            }
            false => {
                let layers = self.pages[self.page].loose.layers.clone();
                add(&mut boxes, &layers, (-x0, -y0), &mut self.text);
            }
        }
        boxes
    }

    /// The boxes of the frames of the page but the one worked in, on it.
    pub(super) fn other_frames(&self) -> Vec<Edges> {
        (0..self.frame_count())
            .filter(|&index| index != self.active)
            .map(|index| {
                let frame = self.frame(index);
                [frame.x, frame.y, frame.x + frame.width as f32, frame.y + frame.height as f32]
            })
            .collect()
    }

    /// The frame worked in moved on its page to `at`, snapped to the edges
    /// and middles of the other frames, or spaced evenly with them, when
    /// near enough; with `free`, as it is. The lines and gaps it goes to
    /// are shown, on the frame where it now is, as a layer's are.
    pub(super) fn snap_frame(&mut self, at: (f32, f32), free: bool) -> (f32, f32) {
        self.guides.clear();
        self.gaps.clear();
        if free {
            return (at.0.round(), at.1.round());
        }
        let (w, h) = (self.doc.width as f32, self.doc.height as f32);
        // The other frames, and the page's own layers, which are as much
        // on the page.
        let mut others = self.other_frames();
        let loose = self.loose().layers.clone();
        for layer in loose.iter().filter(|layer| !layer.hidden) {
            let (x, y, w, h) = layer.bounds(&mut self.text);
            others.push([x, y, x + w, y + h]);
        }
        let (across, down) = spacing::lines_of(&others);
        let snapped = spacing::snap_box([at.0, at.1, at.0 + w, at.1 + h], &across, &down, &others, 6.0 / self.zoom);
        let at = ((at.0 + snapped.by.0).round(), (at.1 + snapped.by.1).round());
        self.guides = snapped.guides.into_iter().map(|(upright, line)| (upright, line - if upright { at.0 } else { at.1 })).collect();
        self.gaps = snapped.gaps.into_iter().map(|gap| gap.moved(-at.0, -at.1)).collect();
        at
    }

    /// How far what is held is to go, made to bring its edges or its
    /// middle onto those of the artboard, a guide or another layer near
    /// enough, or to space it evenly with the rest, as Figma's do: and the
    /// lines and gaps it is brought onto, to show.
    pub(super) fn snap(&mut self, dx: f32, dy: f32) -> (f32, f32) {
        self.guides.clear();
        self.gaps.clear();
        let held: Vec<(f32, f32, f32, f32)> = self.held.clone().iter().map(|layer| layer.bounds(&mut self.text)).collect();
        let Some(&(mut left, mut top, _, _)) = held.first() else { return (dx, dy) };
        let (mut right, mut bottom) = (f32::MIN, f32::MIN);
        for &(x, y, w, h) in &held {
            (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x + w), bottom.max(y + h));
        }
        let mut others = self.other_edges();
        others.extend(self.outside_edges());
        // The frame's edges and middles; on the page, outside the frames,
        // there is none, and the frames' own edges instead.
        let (mut across, mut down) = match self.on_page() {
            true => (0..self.frame_count()).map(|index| self.frame(index)).fold((Vec::new(), Vec::new()), |(mut across, mut down), frame| {
                let (w, h) = (frame.width as f32, frame.height as f32);
                across.extend([frame.x, frame.x + w / 2.0, frame.x + w]);
                down.extend([frame.y, frame.y + h / 2.0, frame.y + h]);
                (across, down)
            }),
            false => (vec![0.0, self.doc.width as f32 / 2.0, self.doc.width as f32], vec![0.0, self.doc.height as f32 / 2.0, self.doc.height as f32]),
        };
        across.extend(self.guide_lines(true));
        down.extend(self.guide_lines(false));
        let (theirs_across, theirs_down) = spacing::lines_of(&others);
        across.extend(theirs_across);
        down.extend(theirs_down);
        let snapped = spacing::snap_box([left + dx, top + dy, right + dx, bottom + dy], &across, &down, &others, 6.0 / self.zoom);
        (self.guides, self.gaps) = (snapped.guides, snapped.gaps);
        (dx + snapped.by.0, dy + snapped.by.1)
    }

    /// The gaps shown while moving, in magenta, each with how wide it is.
    pub(super) fn draw_gaps(&mut self, canvas: &mut Canvas) {
        let gaps = self.gaps.clone();
        let (zoom, pan, origin) = (self.zoom, self.pan, (self.doc.x, self.doc.y));
        let to_window = |(x, y): (f32, f32)| (((x + origin.0) * zoom + pan.0).round() as i32, ((y + origin.1) * zoom + pan.1).round() as i32);
        spacing::draw_gaps(canvas, &mut self.text, &gaps, 11, to_window);
    }
}

/// How long Shift or Alt still counts as held after it is let go.
pub(super) const MODS_HOLD: std::time::Duration = std::time::Duration::from_millis(150);

impl Studio {
    /// The keys held as they should be taken: in a drag, just as they are,
    /// so that Shift pressed or let go partway changes it then, as in
    /// Figma; out of one, Shift and Alt a moment after they are let go,
    /// as a hand lets go of them a little before the button.
    pub(super) fn steady_mods(&mut self, mut mods: Mods) -> Mods {
        let now = std::time::Instant::now();
        if mods.shift {
            self.seen_mods.0 = Some(now);
        }
        if mods.alt {
            self.seen_mods.1 = Some(now);
        }
        if self.dragging() {
            return mods;
        }
        let recent = |seen: Option<std::time::Instant>| seen.is_some_and(|when| now.duration_since(when) < MODS_HOLD);
        mods.shift |= recent(self.seen_mods.0);
        mods.alt |= recent(self.seen_mods.1);
        mods
    }

    /// Whether something is being dragged on the canvas, which the keys
    /// held change as they change.
    pub fn dragging(&self) -> bool {
        !matches!(self.drag, Drag::None | Drag::Pan { .. } | Drag::Picker { .. } | Drag::Slide { .. } | Drag::Scrub { .. })
    }

    /// A key held changed in a drag: the drag again, where the pointer is,
    /// as the keys are now.
    pub fn mods_changed(&mut self, mods: Mods) {
        if self.dragging() {
            let (x, y) = self.pointer;
            self.pointer_moved(x, y, mods);
        }
    }
}

#[cfg(test)]
mod held {
    use super::*;

    #[test]
    fn shift_holds_a_moment_after_it_is_let_go_but_in_a_drag_is_as_it_is() {
        let mut studio = Studio::new(Doc::new(100, 100), Text::load(), 800, 600, std::env::temp_dir(), "t.png".into());
        let shift = Mods { shift: true, ..Mods::default() };
        assert!(!studio.steady_mods(Mods::default()).shift);
        assert!(studio.steady_mods(shift).shift);
        // Let go just now: still held.
        assert!(studio.steady_mods(Mods::default()).shift);
        studio.seen_mods.0 = Some(std::time::Instant::now() - MODS_HOLD * 2);
        assert!(!studio.steady_mods(Mods::default()).shift);
        // In a drag, as it is: let go, at once let go.
        studio.drag = Drag::Draw { from: (0.0, 0.0) };
        assert!(studio.steady_mods(shift).shift);
        assert!(!studio.steady_mods(Mods::default()).shift);
        studio.drag = Drag::None;
        studio.seen_mods.0 = Some(std::time::Instant::now() - MODS_HOLD * 2);
        assert!(!studio.steady_mods(Mods::default()).shift);
    }
}
