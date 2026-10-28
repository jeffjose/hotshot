//! Rulers along the top and the left of the page, in pixels of the frame
//! worked in, what is picked out marked on them; and guides dragged out
//! of them, which what is moved snaps to, and dragged back to go.

use super::*;

/// How thick a ruler is.
pub(super) const RULER: i32 = 20;

const GUIDE_COLOR: u32 = 0xF04050;

/// The spacing of a ruler's numbers: the least of these that leaves room
/// between them, at a zoom.
pub(super) fn ruler_step(zoom: f32) -> f32 {
    [1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 150.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0].into_iter().find(|step| step * zoom >= 64.0).unwrap_or(10000.0)
}

impl Studio {
    /// Where the rulers are: along the top, and down the left, between
    /// the panels.
    pub(super) fn ruler_bands(&self) -> (Rect, Rect) {
        let left = if self.shows_left() { INSET + LEFT_W + 6 } else { 0 };
        let right = if self.shows_right() { self.width - INSET - RIGHT_W - 6 } else { self.width };
        (Rect::new(left, 0, right - left, RULER), Rect::new(left, RULER, RULER, self.height - RULER))
    }

    /// A press on a ruler, which drags out a guide; or on a guide, which
    /// takes hold of it. Whether it was either.
    pub(super) fn press_rulers(&mut self, at: (i32, i32)) -> bool {
        if !self.rulers || self.zen {
            return false;
        }
        let (top, side) = self.ruler_bands();
        let point = self.to_doc(at);
        if top.contains(at.0, at.1) {
            self.user_guides.push((false, point.1));
        } else if side.contains(at.0, at.1) {
            self.user_guides.push((true, point.0));
        } else {
            // On a guide already there, within a few pixels of it.
            let near = self.user_guides.iter().position(|&(upright, place)| {
                let window = self.to_window(if upright { (place, 0.0) } else { (0.0, place) });
                if upright { (window.0 - at.0).abs() <= 3 } else { (window.1 - at.1).abs() <= 3 }
            });
            let Some(index) = near else { return false };
            self.drag = Drag::Guide { index };
            return true;
        }
        self.drag = Drag::Guide { index: self.user_guides.len() - 1 };
        true
    }

    /// A guide being dragged, to where the pointer is, a whole pixel.
    pub(super) fn drag_guide(&mut self, index: usize, at: (i32, i32)) {
        let point = self.to_doc(at);
        if let Some(guide) = self.user_guides.get_mut(index) {
            guide.1 = if guide.0 { point.0.round() } else { point.1.round() };
        }
        self.dirty = true;
    }

    /// A guide let go of: gone, if it was let go of over a ruler.
    pub(super) fn drop_guide(&mut self, index: usize, at: (i32, i32)) {
        let (top, side) = self.ruler_bands();
        if (top.contains(at.0, at.1) || side.contains(at.0, at.1)) && index < self.user_guides.len() {
            self.user_guides.remove(index);
        }
    }

    /// The guides, across the page, each with where it is while held.
    pub(super) fn draw_user_guides(&mut self, canvas: &mut Canvas) {
        let held = match self.drag {
            Drag::Guide { index } => Some(index),
            _ => None,
        };
        for (index, &(upright, place)) in self.user_guides.clone().iter().enumerate() {
            let window = self.to_window(if upright { (place, 0.0) } else { (0.0, place) });
            match upright {
                true => canvas.fill(window.0, 0, 1, self.height, GUIDE_COLOR),
                false => canvas.fill(0, window.1, self.width, 1, GUIDE_COLOR),
            }
            if held == Some(index) {
                let says = format!("{} {}", if upright { "x" } else { "y" }, place.round() as i32);
                let wide = self.text.width(&says, 11) + 10;
                let (x, y) = if upright { (window.0 + 6, self.pointer.1 + 8) } else { (self.pointer.0 + 8, window.1 - 20) };
                canvas.round_fill(x, y, wide, 16, 4, GUIDE_COLOR, 256);
                self.text.draw(canvas, x + 5, y + (16 - Text::line_height(11)) / 2, 11, 0xFFFFFF, &says);
            }
        }
    }

    /// The rulers: numbers in pixels of the frame, from its corner, and
    /// what is picked out marked along each.
    pub(super) fn draw_rulers(&mut self, canvas: &mut Canvas) {
        if !self.rulers || self.zen {
            return;
        }
        let (top, side) = self.ruler_bands();
        canvas.fill(top.x, top.y, top.w, top.h, PANEL_COLOR);
        canvas.fill(side.x, side.y, side.w, side.h, PANEL_COLOR);
        canvas.fill(top.x, top.bottom() - 1, top.w, 1, EDGE_COLOR);
        canvas.fill(side.right() - 1, side.y, 1, side.h, EDGE_COLOR);
        // What is picked out, as a band on each.
        let boxes: Vec<(f32, f32, f32, f32)> = self.chosen().iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let picked = actions::union(&boxes);
        if let Some((x, y, w, h)) = picked {
            let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
            canvas.round_fill(a.0.max(top.x), top.y, (b.0 - a.0.max(top.x)).max(0), top.h - 1, 0, ACCENT, 70);
            canvas.round_fill(side.x, a.1.max(side.y), side.w - 1, (b.1 - a.1.max(side.y)).max(0), 0, ACCENT, 70);
        }
        let step = ruler_step(self.zoom);
        let (first, last) = (self.to_doc((top.x, 0)).0, self.to_doc((top.right(), 0)).0);
        let mut at = (first / step).floor() * step;
        while at <= last {
            let x = self.to_window((at, 0.0)).0;
            if x >= top.x + RULER {
                canvas.fill(x, top.bottom() - 6, 1, 5, 0x6A6A6A);
                self.text.draw(canvas, x + 3, top.y + 3, 10, DIM_TEXT_COLOR, &format!("{}", at.round() as i32));
            }
            at += step;
        }
        let (first, last) = (self.to_doc((0, side.y)).1, self.to_doc((0, side.bottom())).1);
        let mut at = (first / step).floor() * step;
        while at <= last {
            let y = self.to_window((0.0, at)).1;
            if y >= side.y {
                canvas.fill(side.right() - 6, y, 5, 1, 0x6A6A6A);
                // Said down the ruler a digit at a time, which fits.
                for (row, digit) in format!("{}", at.round() as i32).chars().enumerate() {
                    self.text.draw(canvas, side.x + 4, y + 2 + row as i32 * 9, 9, DIM_TEXT_COLOR, &digit.to_string());
                }
            }
            at += step;
        }
        // The edges of what is picked out, said where they are.
        if let Some((x, y, w, h)) = picked {
            for edge in [x, x + w] {
                let wx = self.to_window((edge, 0.0)).0;
                if wx > top.x + RULER && wx < top.right() {
                    let says = format!("{}", edge.round() as i32);
                    let wide = self.text.width(&says, 10) + 6;
                    canvas.round_fill(wx - wide / 2, top.y + 2, wide, 14, 3, ACCENT, 256);
                    self.text.draw(canvas, wx - wide / 2 + 3, top.y + 3, 10, 0xFFFFFF, &says);
                }
            }
            let _ = (y, h);
        }
        canvas.fill(side.x, top.y, RULER, RULER, PANEL_COLOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ruler_spaces_its_numbers_by_the_zoom_and_guides_come_and_go() {
        assert_eq!((ruler_step(1.0), ruler_step(0.5), ruler_step(8.0)), (100.0, 150.0, 10.0));
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        studio.rulers = true;
        let (top, _) = studio.ruler_bands();
        let start = (top.x + 200, top.y + 5);
        assert!(studio.press_rulers(start));
        let Drag::Guide { index } = studio.drag else { panic!() };
        let down = studio.to_window((0.0, 300.0));
        studio.drag_guide(index, (start.0, down.1));
        assert!(studio.user_guides.len() == 1 && !studio.user_guides[0].0 && (studio.user_guides[0].1 - 300.0).abs() <= 1.0);
        // What is moved snaps to it.
        assert_eq!(studio.guide_lines(false), vec![studio.user_guides[0].1]);
        studio.drop_guide(index, start);
        assert!(studio.user_guides.is_empty());
    }
}

impl Studio {
    /// Where the guides are that go one way: up and down, at an x, or
    /// across, at a y.
    pub(super) fn guide_lines(&self, upright: bool) -> Vec<f32> {
        self.user_guides.iter().filter(|(way, _)| *way == upright).map(|(_, place)| *place).collect()
    }
}
