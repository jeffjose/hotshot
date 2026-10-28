//! Drawing a box round what is there, with the box tool: the pointer over
//! a board's box, or over a part of a screenshot that the edge finder sees
//! as a box, outlines it at once; a click draws the new box exactly round
//! it, and a drag still draws freely. Where boxes are nested and share
//! edges, Alt goes out to the one round it, and on round again.

use super::*;

/// The boxes round a point, the smallest first, and which is outlined.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Pick {
    pub around: Vec<(f32, f32, f32, f32)>,
    pub at: usize,
}

impl Studio {
    /// Every box round a point of the board: its boxes, and the boxes of
    /// lines in a picture under it, the smallest first.
    pub(in crate::studio) fn boxes_round(&mut self, at: (f32, f32)) -> Vec<(f32, f32, f32, f32)> {
        let mut found: Vec<(f32, f32, f32, f32)> = (0..self.doc.layers.len()).filter(|&index| self.is_box(index) && !self.doc.layers[index].hidden).map(|index| self.box_frame(index)).filter(|f| at.0 >= f.0 && at.0 <= f.0 + f.2 && at.1 >= f.1 && at.1 <= f.1 + f.3).collect();
        for index in (0..self.doc.layers.len()).rev() {
            let Kind::Picture(picture) = &self.doc.layers[index].kind else { continue };
            if self.doc.layers[index].hidden || picture.angle != 0.0 {
                continue;
            }
            let (lx, ly) = picture.local(at);
            let (hw, hh) = (picture.w / 2.0, picture.h / 2.0);
            if lx.abs() > hw || ly.abs() > hh {
                continue;
            }
            let (cx, cy, cw, ch) = picture.shown_crop();
            let image = picture.image.clone();
            let (left, top, w, h) = (picture.cx - hw, picture.cy - hh, picture.w, picture.h);
            let lines = self.crop_lines(&image);
            let point = (cx + (lx + hw) / w * cw, cy + (ly + hh) / h * ch);
            for (l, t, r, b) in lines.around(point) {
                let page = |px: f32, py: f32| (left + (px - cx) / cw * w, top + (py - cy) / ch * h);
                let (a, z) = (page(l, t), page(r, b));
                found.push((a.0, a.1, z.0 - a.0, z.1 - a.1));
            }
            found.push((left, top, w, h));
            break;
        }
        found.sort_by(|a, b| (a.2 * a.3).total_cmp(&(b.2 * b.3)));
        found.dedup_by(|a, b| (a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0 && (a.2 - b.2).abs() < 1.0 && (a.3 - b.3).abs() < 1.0);
        found
    }

    /// The pointer moved with the box tool in hand: what it is over
    /// outlined, the depth Alt went to kept while it is still there.
    pub(in crate::studio) fn hover_pick(&mut self, at: (i32, i32), over_control: bool) {
        if self.tool != Tool::Rect || over_control || self.editing.is_some() || !self.over_page(at) {
            if self.board.pick.take().is_some() {
                self.dirty = true;
            }
            return;
        }
        let page = self.to_doc(at);
        let around = self.boxes_round(page);
        let kept = self.board.pick.as_ref().filter(|pick| pick.around == around).map_or(0, |pick| pick.at);
        let pick = (!around.is_empty()).then(|| Pick { around, at: kept });
        if pick.as_ref().map(|pick| (pick.around.clone(), pick.at)) != self.board.pick.as_ref().map(|pick| (pick.around.clone(), pick.at)) {
            self.board.pick = pick;
            self.dirty = true;
        }
    }

    /// Alt: the box round the one outlined, and from the largest back to
    /// the smallest.
    pub(in crate::studio) fn pick_out(&mut self) -> bool {
        let Some(pick) = &mut self.board.pick else { return false };
        if pick.around.len() < 2 {
            return false;
        }
        pick.at = (pick.at + 1) % pick.around.len();
        self.dirty = true;
        true
    }

    /// The box outlined, if a point is in it: what a click draws round.
    pub(in crate::studio) fn picked_round(&self, at: (f32, f32)) -> Option<(f32, f32, f32, f32)> {
        let pick = self.board.pick.as_ref()?;
        let f = *pick.around.get(pick.at)?;
        (at.0 >= f.0 - 1.0 && at.0 <= f.0 + f.2 + 1.0 && at.1 >= f.1 - 1.0 && at.1 <= f.1 + f.3 + 1.0).then_some(f)
    }

    pub(in crate::studio) fn draw_pick(&mut self, canvas: &mut Canvas) {
        if self.drag != Drag::None {
            return;
        }
        let Some(pick) = &self.board.pick else { return };
        let Some(&(x, y, w, h)) = pick.around.get(pick.at) else { return };
        let (a, b) = (self.to_window_page((x, y)), self.to_window_page((x + w, y + h)));
        canvas.round_outline(a.0 - 1, a.1 - 1, b.0 - a.0 + 2, b.1 - a.1 + 2, 3, 2, ACCENT);
        if pick.around.len() > 1 {
            let says = format!("{} of {} \u{b7} Alt", pick.at + 1, pick.around.len());
            let wide = self.text.width(&says, 11) + 12;
            canvas.round_fill(a.0, a.1 - 22, wide, 18, 4, ACCENT, 256);
            self.text.draw(canvas, a.0 + 6, a.1 - 22 + 9 - Text::line_height(11) / 2, 11, 0xFFFFFF, &says);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn the_box_tool_outlines_what_it_is_over_and_a_click_draws_round_it_alt_going_out() {
        let mut studio = board();
        // A screenshot with a card in it, and a panel in the card.
        let pixels = crate::studio::edges::tests::card_picture(400, 300, (40, 40, 360, 260), (80, 80, 200, 160));
        studio.add_picture(Image::new(400, 300, pixels), "shot");
        let index = studio.doc.layers.len() - 1;
        if let Kind::Picture(picture) = &mut studio.doc.layers[index].kind {
            (picture.cx, picture.cy) = (300.0, 250.0);
        }
        studio.changed();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        let over = studio.to_window_page((150.0 + 140.0 - 100.0, 100.0 + 120.0 - 50.0 + 20.0));
        studio.pointer_moved(over.0, over.1, Mods::default());
        let pick = studio.board.pick.clone().expect("outlined");
        let (x, y, w, h) = pick.around[0];
        assert!((x - 180.0).abs() < 3.0 && (y - 180.0).abs() < 3.0 && (w - 120.0).abs() < 3.0 && (h - 80.0).abs() < 3.0, "the panel: {:?}", pick.around[0]);
        // Alt: the card round it.
        studio.key(Key::Other, Mods::default(), true);
        let (_, _, w, _) = studio.board.pick.clone().unwrap().around[1];
        assert!((w - 320.0).abs() < 4.0);
        // A click draws a box exactly round the card.
        tap(&mut studio, (190.0, 190.0));
        let made = shape(&studio, studio.doc.layers.len() - 1);
        let (mx, _, mw, _) = made.frame();
        assert!((mx - 140.0).abs() < 4.0 && (mw - 320.0).abs() < 4.0, "{:?}", made.frame());
        assert_eq!(studio.editing, Some(studio.doc.layers.len() - 1));
    }
}
