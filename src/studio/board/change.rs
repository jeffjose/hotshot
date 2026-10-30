//! Changing your mind at the speed you change it: an arrow's end dragged
//! to another box ties it there; a drag from an arrow's middle branches
//! it, "and it also goes there"; a box dropped on an arrow goes between
//! its two ends, what comes after moving over to make room; "?" or "TBD"
//! in a box leaves it dashed and hollow, and listed in Ctrl+K; Shift+S
//! makes a box another shape, keeping its words and arrows; and Ctrl+Alt+Z
//! goes back through the board's history, to branch from a point of it,
//! the later version kept as a page beside it.

use super::*;

/// The shapes Shift+S goes round: a box, an oval, a diamond, a database,
/// a person and a cloud.
pub(in crate::studio) const SHAPES: [&str; 6] = ["Box", "Oval", "Diamond", "Database", "Person", "Cloud"];

/// How far apart boxes are kept along an arrow, when one goes between.
const ROOM: f32 = 48.0;

/// The history being gone through: the board as it is now, to come back
/// to, and how far back it is, by how many steps of undo.
#[derive(Clone)]
pub(in crate::studio) struct History {
    pub now: Snapshot,
    pub back: usize,
    pub sliding: bool,
}

impl Studio {
    /// Which of `SHAPES` a box is.
    pub(in crate::studio) fn shape_of(shape: &Shape) -> usize {
        match (shape.tool, shape.form) {
            (Tool::Ellipse, _) => 1,
            (_, shapes::FORM_DIAMOND) => 2,
            (_, shapes::FORM_CYLINDER) => 3,
            (_, shapes::FORM_PERSON) => 4,
            (_, shapes::FORM_CLOUD) => 5,
            _ => 0,
        }
    }

    /// The boxes picked out made one of `SHAPES`, or each the next of them,
    /// their words and arrows kept. One step to undo.
    pub(in crate::studio) fn reshape(&mut self, to: Option<usize>) {
        let boxes: Vec<usize> = self.chosen().into_iter().filter(|&index| self.is_box(index)).collect();
        let Some(&first) = boxes.first() else { return self.say("Pick a box to make it another shape") };
        let Kind::Shape(shape) = &self.doc.layers[first].kind else { return };
        let to = to.unwrap_or((Studio::shape_of(shape) + 1) % SHAPES.len());
        self.restyle_shapes(|shape| {
            if !matches!(shape.tool, Tool::Rect | Tool::Ellipse) || shape.form == shapes::FORM_STICKY {
                return;
            }
            (shape.tool, shape.form) = match to {
                1 => (Tool::Ellipse, shapes::FORM_BOX),
                2 => (Tool::Rect, shapes::FORM_DIAMOND),
                3 => (Tool::Rect, shapes::FORM_CYLINDER),
                4 => (Tool::Rect, shapes::FORM_PERSON),
                5 => (Tool::Rect, shapes::FORM_CLOUD),
                _ => (Tool::Rect, shapes::FORM_BOX),
            };
        });
        self.say(format!("Shape: {}", SHAPES[to]));
    }

    /// An arrow of the board's whose middle a point is near, in the window,
    /// stuck at its start to a box: to branch from.
    pub(in crate::studio) fn arrow_middle_at(&self, at: (i32, i32)) -> Option<usize> {
        (0..self.doc.layers.len()).rev().find(|&index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) if shape.sticks() && shape.routed && !self.doc.layers[index].hidden => {
                let middle = self.to_window(shape.middle());
                (middle.0 - at.0).abs() <= 9 && (middle.1 - at.1).abs() <= 9 && shape.stuck[0].and_then(|(id, ..)| self.index_of(id)).is_some_and(|from| self.is_box(from))
            }
            _ => false,
        })
    }

    /// A press on an arrow's middle: a branch begun from where it comes
    /// from.
    pub(in crate::studio) fn press_branch(&mut self, at: (i32, i32)) -> bool {
        let Some(arrow) = self.arrow_middle_at(at) else { return false };
        let Kind::Shape(shape) = &self.doc.layers[arrow].kind else { return false };
        let Some(from) = shape.stuck[0].and_then(|(id, ..)| self.index_of(id)) else { return false };
        self.board.branching = Some(arrow);
        self.drag = Drag::Pull { from, side: BRANCH, moved: false };
        true
    }

    /// Where an arrow being pulled out comes from, in the window: a side's
    /// dot, or the middle of the arrow it branches.
    pub(in crate::studio) fn pull_start(&self, from: usize, side: u8) -> (i32, i32) {
        match (side, self.board.branching.and_then(|arrow| self.doc.layers.get(arrow))) {
            (BRANCH, Some(Layer { kind: Kind::Shape(shape), .. })) => self.to_window(shape.middle()),
            _ => self.dot_spots(from)[usize::from(side.min(3))],
        }
    }

    /// A box let go on an arrow between two others, joined to neither: it
    /// goes between them, the arrow to it and another on from it; what the
    /// arrow went to, and what comes after that, moving over along it if
    /// there is not room. Whether it did.
    pub(in crate::studio) fn split_arrow(&mut self, index: usize, (arrow, from, to): (usize, u32, u32)) -> bool {
        let id = self.id_of(index);
        let (x, y, w, h) = self.box_frame(index);
        let middle = (x + w / 2.0, y + h / 2.0);
        let (Some(a), Some(b)) = (self.index_of(from), self.index_of(to)) else { return false };
        let (fa, fb) = (self.box_frame(a), self.box_frame(b));
        // The way the arrow goes, across or down, and the room there is.
        let across = ((fb.0 + fb.2 / 2.0) - (fa.0 + fa.2 / 2.0)).abs() >= ((fb.1 + fb.3 / 2.0) - (fa.1 + fa.3 / 2.0)).abs();
        let sign = if across { ((fb.0 + fb.2 / 2.0) - (fa.0 + fa.2 / 2.0)).signum() } else { ((fb.1 + fb.3 / 2.0) - (fa.1 + fa.3 / 2.0)).signum() };
        let room = match (across, sign > 0.0) {
            (true, true) => fb.0 - (fa.0 + fa.2),
            (true, false) => fa.0 - (fb.0 + fb.2),
            (false, true) => fb.1 - (fa.1 + fa.3),
            (false, false) => fa.1 - (fb.1 + fb.3),
        };
        let need = if across { w } else { h } + 2.0 * ROOM;
        if room < need {
            let by = (need - room) * sign;
            let shift = if across { (by, 0.0) } else { (0.0, by) };
            // What it went to and on from there, but not back to the start.
            let mut after = vec![to];
            let mut at = 0;
            while at < after.len() {
                for child in self.child_boxes(after[at]) {
                    if child != from && child != id && !after.contains(&child) {
                        after.push(child);
                    }
                }
                at += 1;
            }
            for moved in after {
                let mut all = self.inside(moved);
                all.push(moved);
                for member in all {
                    if let Some(member) = self.index_of(member) {
                        self.doc.layers[member].translate(shift.0, shift.1);
                    }
                }
            }
        }
        // In the middle of the room there now is, in line with the start.
        let fb = self.box_frame(b);
        let center = match (across, sign > 0.0) {
            (true, true) => ((fa.0 + fa.2 + fb.0) / 2.0, fa.1 + fa.3 / 2.0),
            (true, false) => ((fb.0 + fb.2 + fa.0) / 2.0, fa.1 + fa.3 / 2.0),
            (false, true) => (fa.0 + fa.2 / 2.0, (fa.1 + fa.3 + fb.1) / 2.0),
            (false, false) => (fa.0 + fa.2 / 2.0, (fb.1 + fb.3 + fa.1) / 2.0),
        };
        let mut all = self.inside(id);
        all.push(id);
        for member in all {
            if let Some(member) = self.index_of(member) {
                self.doc.layers[member].translate(center.0 - middle.0, center.1 - middle.1);
            }
        }
        if let Kind::Shape(shape) = &mut self.doc.layers[arrow].kind {
            shape.stuck[1] = Some((id, shapes::WHOLE, shapes::WHOLE));
        }
        let index = self.index_of(id).unwrap_or(index);
        self.join(index, b);
        self.changed();
        self.say("In between");
        true
    }

    /// Where a box being moved would go if let go, over an arrow between
    /// two others: right on its line, between its ends; a little above or
    /// below it, beside it, as a branch off where it comes from. The zone it
    /// is in now holds a few pixels longer, so it does not flicker.
    pub(in crate::studio) fn drop_target(&mut self) -> Option<Drop> {
        let index = self.picked().filter(|&index| self.is_box(index))?;
        let id = self.id_of(index);
        let links = self.links();
        if links.iter().any(|&(_, from, to)| from == id || to == id) || !self.inside(id).is_empty() {
            return None;
        }
        let (x, y, w, h) = self.box_frame(index);
        let middle = (x + w / 2.0, y + h / 2.0);
        let was = self.board.drop;
        let mut best: Option<(f32, Drop)> = None;
        for &(arrow, from, to) in &links {
            let Some(at) = self.index_of(arrow) else { continue };
            let Kind::Shape(shape) = &self.doc.layers[at].kind else { continue };
            // How far its middle is from the arrow's path, and whether it is
            // along it rather than past an end.
            let mut path: Vec<(f32, f32)> = vec![shape.points[0]];
            path.extend_from_slice(&shape.points[2..]);
            path.push(shape.points[1]);
            let mut far = f32::MAX;
            let mut along = false;
            for pair in path.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let t = ((middle.0 - a.0) * dx + (middle.1 - a.1) * dy) / (dx * dx + dy * dy).max(1.0);
                let tc = t.clamp(0.0, 1.0);
                let d = (a.0 + tc * dx - middle.0).hypot(a.1 + tc * dy - middle.1);
                if d < far {
                    (far, along) = (d, (-0.05..=1.05).contains(&t));
                }
            }
            if !along {
                continue;
            }
            let held = |split: bool| was.is_some_and(|drop: Drop| drop.arrow == at && drop.split == split);
            let on_line = 14.0 + if held(true) { 8.0 } else { 0.0 };
            let beside = h.max(w) / 2.0 + 60.0 + if held(false) { 12.0 } else { 0.0 };
            let drop = match far {
                far if far <= on_line => Drop { arrow: at, from, to, split: true },
                far if far <= beside => Drop { arrow: at, from, to, split: false },
                _ => continue,
            };
            if best.is_none_or(|(near, _)| far < near) {
                best = Some((far, drop));
            }
        }
        best.map(|(_, drop)| drop)
    }

    /// A box let go beside an arrow: joined as a branch off where the arrow
    /// comes from, the arrow kept as it was.
    pub(in crate::studio) fn route_around(&mut self, index: usize, from: u32) -> bool {
        let Some(start) = self.index_of(from) else { return false };
        self.join(start, index);
        self.changed();
        self.say("A branch beside it");
        true
    }

    /// What letting go where the box being moved is would do to an arrow,
    /// shown faintly: the arrow broken round it, or a branch to it.
    pub(in crate::studio) fn draw_drop(&mut self, canvas: &mut Canvas) {
        let (Some(drop), Drag::Move { moved: true, .. }) = (self.board.drop, self.drag) else { return };
        let Some(index) = self.picked() else { return };
        let (Some(a), Some(b)) = (self.index_of(drop.from), self.index_of(drop.to)) else { return };
        let (x, y, w, h) = self.box_frame(index);
        let mid = self.to_window_page((x + w / 2.0, y + h / 2.0));
        let center = |studio: &Studio, i: usize| {
            let (fx, fy, fw, fh) = studio.box_frame(i);
            studio.to_window_page((fx + fw / 2.0, fy + fh / 2.0))
        };
        let (ca, cb) = (center(self, a), center(self, b));
        let dashed = |canvas: &mut Canvas, p: (i32, i32), q: (i32, i32), color: u32| {
            let long = (((q.0 - p.0) as f32).hypot((q.1 - p.1) as f32)).max(1.0);
            let mut t = 0.0;
            while t < long {
                let s = (t + 7.0).min(long);
                let at = |u: f32| (p.0 + ((q.0 - p.0) as f32 * u / long) as i32, p.1 + ((q.1 - p.1) as f32 * u / long) as i32);
                for shift in [0, 1] {
                    line(canvas, (at(t).0, at(t).1 + shift), (at(s).0, at(s).1 + shift), color);
                }
                t += 12.0;
            }
        };
        let (color, says) = match drop.split {
            true => {
                // The arrow broken where the box goes: a gap of paper across it.
                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(drop.arrow) {
                    let gap = self.to_window(shape.middle());
                    canvas.disc(gap.0, gap.1, 9, shapes::PAPER, 256);
                }
                dashed(canvas, ca, mid, ACCENT);
                dashed(canvas, mid, cb, ACCENT);
                (ACCENT, "Between")
            }
            false => {
                dashed(canvas, ca, mid, 0xF08C00);
                (0xF08C00, "Branch beside")
            }
        };
        let wide = self.text.width(says, 11) + 12;
        let (cx, top) = (mid.0 - wide / 2, (mid.1 - (h * self.zoom / 2.0) as i32 - 26).max(4));
        canvas.round_fill(cx, top, wide, 18, 4, color, 256);
        self.text.draw(canvas, cx + 6, top + 9 - Text::line_height(11) / 2, 11, 0xFFFFFF, says);
    }

    /// The boxes still to be filled in: those whose words are "?" or TBD.
    pub(in crate::studio) fn placeholders(&self) -> Vec<usize> {
        (0..self.doc.layers.len()).filter(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.placeholder())).collect()
    }

    /// Ctrl+Alt+Z: the board's history, to go back through.
    pub(in crate::studio) fn open_history(&mut self) {
        self.commit_text();
        if self.undo.is_empty() {
            return self.say("Nothing to go back through yet");
        }
        self.board.ghost = None;
        self.board.history = Some(History { now: self.snapshot(), back: 0, sliding: false });
        self.selected.clear();
        self.dirty = true;
    }

    /// The history shown so many steps back.
    pub(in crate::studio) fn history_to(&mut self, back: usize) {
        let Some(history) = self.board.history.clone() else { return };
        let back = back.min(self.undo.len());
        let shown = match back {
            0 => history.now.clone(),
            back => self.undo[self.undo.len() - back].clone(),
        };
        self.restore(shown);
        self.selected.clear();
        if let Some(history) = &mut self.board.history {
            history.back = back;
        }
        self.changed();
    }

    /// Back to now, the history closed.
    pub(in crate::studio) fn history_now(&mut self) {
        let Some(history) = self.board.history.take() else { return };
        self.restore(history.now);
        self.changed();
    }

    /// The board as it was where the history is, from now on; as it is now
    /// kept as a page beside it, called its later version. One step to
    /// undo, which brings back now as it was.
    pub(in crate::studio) fn branch_here(&mut self) {
        let Some(history) = self.board.history.take() else { return };
        if history.back == 0 {
            return self.say("This is now: go back first");
        }
        let mut later = history.now.pages[history.now.page].clone();
        later.name = format!("{} \u{b7} later", later.name);
        self.undo.push(history.now);
        self.redo.clear();
        let at = self.page + 1;
        self.pages.insert(at, later);
        self.changed();
        self.say("Branched: the later version is the page beside it");
    }

    /// The bar of the history, over the middle of the top of the board: a
    /// slider from the first of it to now, and its buttons.
    pub(in crate::studio) fn layout_history(&mut self, layout: &mut Layout) {
        if self.board.history.is_none() {
            return;
        }
        let open = self.open_area();
        let bar = Rect::new(open.center().0 - 260, open.y + 8, 520, 92);
        layout.controls.push((Rect::new(bar.x + 16, bar.y + 12, bar.w - 32, 22), Control::Board(Ctl::HistoryTrack)));
        layout.controls.push((Rect::new(bar.right() - 16 - 112, bar.bottom() - 36, 112, 26), Control::Board(Ctl::Now)));
        layout.controls.push((Rect::new(bar.right() - 16 - 112 - 8 - 132, bar.bottom() - 36, 132, 26), Control::Board(Ctl::Branch)));
        layout.controls.push((bar, Control::Panel));
        layout.popover = Some(bar);
    }

    /// A point of the history's track, as how many steps back.
    pub(in crate::studio) fn history_at(&mut self, x: i32) {
        let Some(track) = self.control_rect(Control::Board(Ctl::HistoryTrack)) else { return };
        let steps = self.undo.len().max(1);
        let along = ((x - track.x) as f32 / track.w.max(1) as f32).clamp(0.0, 1.0);
        let back = ((1.0 - along) * steps as f32).round() as usize;
        if self.board.history.as_ref().is_some_and(|history| history.back != back) {
            self.history_to(back);
        }
    }

    /// What it was, when: as minutes and hours ago.
    fn ago(when: std::time::SystemTime) -> String {
        let secs = when.elapsed().map_or(0, |since| since.as_secs());
        match secs {
            0..=59 => "just now".to_owned(),
            60..=3599 => format!("{} min ago", secs / 60),
            _ => format!("{} h ago", secs / 3600),
        }
    }

    pub(in crate::studio) fn draw_history_control(&mut self, canvas: &mut Canvas, rect: Rect, ctl: Ctl, hovered: bool) {
        let Some(history) = self.board.history.clone() else { return };
        let (cx, cy) = rect.center();
        let line_high = Text::line_height(12);
        match ctl {
            Ctl::HistoryTrack => {
                let steps = self.undo.len().max(1);
                let along = 1.0 - history.back as f32 / steps as f32;
                let knob = rect.x + (along * rect.w as f32).round() as i32;
                canvas.round_fill(rect.x, cy - 3, rect.w, 6, 3, 0x4A4A4A, 256);
                canvas.round_fill(rect.x, cy - 3, (knob - rect.x).max(6), 6, 3, ACCENT, 256);
                canvas.disc(knob, cy, 8, 0xFFFFFF, 256);
                canvas.disc(knob, cy, 6, if hovered { 0x5472E4 } else { ACCENT }, 256);
                let first = self.undo.first().map_or("", |_| "first");
                let shown = match history.back {
                    0 => "now".to_owned(),
                    back => format!("{} steps back \u{b7} {}", back, Studio::ago(self.undo[self.undo.len() - back].when)),
                };
                self.text.draw(canvas, rect.x, rect.bottom() + 6, 11, DIM_TEXT_COLOR, first);
                let wide = self.text.width(&shown, 12);
                self.text.draw(canvas, cx - wide / 2, rect.bottom() + 5, 12, TEXT_COLOR, &shown);
                let wide = self.text.width("now", 11);
                self.text.draw(canvas, rect.right() - wide, rect.bottom() + 6, 11, DIM_TEXT_COLOR, "now");
            }
            Ctl::Branch | Ctl::Now => {
                let primary = ctl == Ctl::Branch;
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if primary { if hovered { 0x5472E4 } else { ACCENT } } else if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                let says = if primary { "Branch from here" } else { "Back to now" };
                let wide = self.text.width(says, 12);
                self.text.draw(canvas, cx - wide / 2, cy - line_high / 2, 12, if primary { 0xFFFFFF } else { TEXT_COLOR }, says);
            }
            _ => {}
        }
    }
}

/// The side an arrow is pulled from when it branches another, rather than
/// out of a box's side.
pub(in crate::studio) const BRANCH: u8 = 4;

/// A box let go over an arrow: the arrow, the boxes it joins, and whether
/// the box goes between them or beside, as a branch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) struct Drop {
    pub arrow: usize,
    pub from: u32,
    pub to: u32,
    pub split: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn put(studio: &mut Studio, (x, y, w, h): (f32, f32, f32, f32), words: &str) -> usize {
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
        studio.board_dress(&mut shape);
        shape.points[1] = (x + w, y + h);
        (shape.text, shape.base) = (words.into(), (w, h));
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        studio.doc.layers.len() - 1
    }

    /// A drag between two points of the window.
    fn drag_window(studio: &mut Studio, from: (i32, i32), to: (i32, i32)) {
        studio.pointer_moved(from.0, from.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((from.0 + to.0) / 2, (from.1 + to.1) / 2, Mods::default());
        studio.pointer_moved(to.0, to.1, Mods::default());
        studio.released();
    }

    #[test]
    fn an_arrows_end_is_tied_again_and_its_middle_branches_it() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "Client");
        let b = put(&mut studio, (400.0, 100.0, 120.0, 60.0), "Server");
        let c = put(&mut studio, (400.0, 400.0, 120.0, 60.0), "Cache");
        let arrow = studio.join(a, b);
        studio.changed();
        // Its head dragged onto another box: tied there, routed to it.
        studio.selected = vec![arrow];
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        let onto = studio.to_window_page((460.0, 430.0));
        drag_window(&mut studio, head, onto);
        assert_eq!(shape(&studio, arrow).stuck[1], Some((shape(&studio, c).id, shapes::WHOLE, shapes::WHOLE)));
        assert_eq!(studio.parent_box(shape(&studio, c).id), Some(shape(&studio, a).id));
        // From its middle, onto empty board: a new box, from Client too.
        studio.selected.clear();
        let middle = studio.to_window(shape(&studio, arrow).middle());
        let empty = studio.to_window_page((150.0, 500.0));
        drag_window(&mut studio, middle, empty);
        assert_eq!(studio.child_boxes(shape(&studio, a).id).len(), 2, "and it also goes there");
        assert!(studio.editing.is_some());
        let _ = b;
    }

    #[test]
    fn a_box_dropped_on_an_arrow_goes_between_its_ends_and_what_follows_makes_room() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "A");
        let b = put(&mut studio, (300.0, 100.0, 120.0, 60.0), "B");
        let c = put(&mut studio, (500.0, 100.0, 120.0, 60.0), "C");
        studio.join(a, b);
        studio.join(b, c);
        let new = put(&mut studio, (200.0, 400.0, 120.0, 60.0), "Validate");
        studio.changed();
        let from = studio.to_window_page((260.0, 430.0));
        let onto = studio.to_window_page((260.0, 133.0));
        drag_window(&mut studio, from, onto);
        let (ida, idb, idn) = (shape(&studio, a).id, shape(&studio, b).id, shape(&studio, new).id);
        assert_eq!((studio.parent_box(idn), studio.parent_box(idb)), (Some(ida), Some(idn)), "A to new to B");
        // B, and C after it, moved over to make room, keeping their gap.
        let (fb, fc, fnew) = (shape(&studio, b).frame(), shape(&studio, c).frame(), shape(&studio, new).frame());
        assert!(fnew.0 > 220.0 && fb.0 > fnew.0 + fnew.2 && (fc.0 - fb.0 - 200.0).abs() < 0.5);
        assert!((fnew.1 - 100.0).abs() < 0.5, "in line with what it is between");
    }

    #[test]
    fn a_question_mark_is_a_placeholder_listed_in_ctrl_k_and_shift_s_reshapes() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "API");
        let b = put(&mut studio, (400.0, 100.0, 120.0, 60.0), "?");
        studio.join(a, b);
        studio.changed();
        assert!(shape(&studio, b).placeholder() && !shape(&studio, a).placeholder());
        studio.open_palette();
        let found = studio.hits().into_iter().find(|hit| hit.heading == "PLACEHOLDERS").expect("listed");
        assert!(found.hint.contains("API"));
        studio.search = None;
        // Drawn dashed and hollow: its middle is the paper.
        studio.selected.clear();
        assert!(matches!(shown(&mut studio, (430.0, 115.0)), shapes::PAPER | 0xD8D6CF));
        // Shift+S: an oval, a diamond, a database, a person, a cloud, a box,
        // its words and arrows kept.
        studio.selected = vec![a];
        let id = shape(&studio, a).id;
        for (tool, form) in [(Tool::Ellipse, shapes::FORM_BOX), (Tool::Rect, shapes::FORM_DIAMOND), (Tool::Rect, shapes::FORM_CYLINDER), (Tool::Rect, shapes::FORM_PERSON), (Tool::Rect, shapes::FORM_CLOUD), (Tool::Rect, shapes::FORM_BOX)] {
            press_key(&mut studio, Key::Char('S'), Mods { shift: true, ..Mods::default() });
            let now = shape(&studio, a);
            assert_eq!((now.tool, now.form, now.text.as_str(), now.id), (tool, form, "API", id));
            assert_eq!(studio.parent_box(shape(&studio, b).id), Some(id));
            studio.selected.clear();
            let _ = shown(&mut studio, (160.0, 130.0));
            studio.selected = vec![a];
        }
    }

    #[test]
    fn a_box_let_go_just_beside_an_arrow_branches_off_it_the_arrow_kept() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "A");
        let b = put(&mut studio, (500.0, 100.0, 120.0, 60.0), "B");
        studio.join(a, b);
        let new = put(&mut studio, (200.0, 400.0, 100.0, 40.0), "Audit");
        studio.changed();
        // Dragged to a little under the line: the branch is what is shown.
        let from = studio.to_window_page((250.0, 420.0));
        studio.pointer_moved(from.0, from.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let near = studio.to_window_page((360.0, 190.0));
        studio.pointer_moved(near.0, near.1 + 20, Mods::default());
        studio.pointer_moved(near.0, near.1, Mods::default());
        assert_eq!(studio.board.drop.map(|drop| drop.split), Some(false));
        // Right on it: between; and back out a hair, still between.
        let on = studio.to_window_page((360.0, 133.0));
        studio.pointer_moved(on.0, on.1, Mods::default());
        assert_eq!(studio.board.drop.map(|drop| drop.split), Some(true));
        studio.pointer_moved(on.0, on.1 + 12, Mods::default());
        assert_eq!(studio.board.drop.map(|drop| drop.split), Some(true), "it holds a little past");
        studio.pointer_moved(near.0, near.1, Mods::default());
        studio.released();
        let (ida, idb, idn) = (shape(&studio, a).id, shape(&studio, b).id, shape(&studio, new).id);
        assert_eq!(studio.parent_box(idb), Some(ida), "the arrow kept");
        assert_eq!(studio.parent_box(idn), Some(ida), "a branch to it");
    }

    #[test]
    fn the_history_goes_back_and_branches_keeping_the_later_version_beside_it() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (220.0, 160.0), Mods::default());
        type_in(&mut studio, "One");
        press_key(&mut studio, Key::Enter, Mods::default());
        pull(&mut studio, (300.0, 100.0), (420.0, 160.0), Mods::default());
        type_in(&mut studio, "Two");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(studio.doc.layers.len(), 2);
        press_key(&mut studio, Key::Char('z'), Mods { ctrl: true, alt: true, ..Mods::default() });
        assert!(studio.board.history.is_some());
        // Back a step at a time with Left, until Two is not yet drawn.
        while studio.doc.layers.len() == 2 {
            press_key(&mut studio, Key::Left, Mods::default());
        }
        assert_eq!(studio.doc.layers.len(), 1);
        // Back to now, as it was.
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.history.is_none() && studio.doc.layers.len() == 2);
        // Back again, and branched: this page as it was, the later version
        // kept as a page beside it; undone, as it was before.
        press_key(&mut studio, Key::Char('z'), Mods { ctrl: true, alt: true, ..Mods::default() });
        while studio.doc.layers.len() == 2 {
            press_key(&mut studio, Key::Left, Mods::default());
        }
        let branch = studio.control_rect(Control::Board(Ctl::Branch)).unwrap().center();
        click(&mut studio, branch);
        assert_eq!((studio.pages.len(), studio.doc.layers.len()), (2, 1));
        assert!(studio.pages[1].name.ends_with("later") && studio.pages[1].loose.layers.len() == 2 && studio.pages[1].board);
        studio.undo();
        assert_eq!((studio.pages.len(), studio.doc.layers.len()), (1, 2));
    }
}
