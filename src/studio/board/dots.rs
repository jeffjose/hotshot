//! Arrows from a box's edge, with no arrow tool: the pointer over a box
//! shows a dot on each of its sides. Drag one out and let go on another
//! box, and the two are joined; on empty board, and a new box is made
//! there, joined and typed into. A click on a dot makes the new box at the
//! usual gap that way.

use super::*;
/// How far out from a side its dot is, on the side itself, where an arrow
/// tied there meets it; and how near it a press takes it, in pixels of the
/// window.
pub(in crate::studio) const OUT: i32 = 0;
pub(in crate::studio) const NEAR: i32 = 12;
/// How near an arrow's end a press takes hold of it, in pixels of the
/// window: more than a handle's, as an end is small and often by a box.
pub(in crate::studio) const END_REACH: i32 = 9;
/// How much farther out a dot goes where an arrow's end is at it.
const AWAY: i32 = 22;
/// How far the next box goes from one, across and down.
pub(in crate::studio) const GAP_ACROSS: f32 = 80.0;
pub(in crate::studio) const GAP_DOWN: f32 = 48.0;

/// Which way each side faces: up, right, down, left.
pub(in crate::studio) const SIDES: [(f32, f32); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];

/// Whether a press is on the dot of a side: near it, and on its side's
/// line or outside it, never more than a few pixels into the box, where a
/// press moves the box.
fn on_dot(spot: (i32, i32), side: usize, at: (i32, i32)) -> bool {
    let way = SIDES[side];
    let inward = -(way.0 * (at.0 - spot.0) as f32 + way.1 * (at.1 - spot.1) as f32);
    (spot.0 - at.0).abs() <= NEAR && (spot.1 - at.1).abs() <= NEAR && inward <= 3.0
}

impl Studio {
    /// A box's frame on the board: its left, top, width and height.
    pub(in crate::studio) fn box_frame(&self, index: usize) -> (f32, f32, f32, f32) {
        match &self.doc.layers[index].kind {
            Kind::Shape(shape) => shape.frame(),
            Kind::Picture(_) => (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// What a layer is called, which arrows are stuck to it by.
    pub(in crate::studio) fn id_of(&self, index: usize) -> u32 {
        stacks::layer_id(&self.doc.layers[index])
    }

    /// Where the dots of a box's sides are, in the window: top, right,
    /// bottom and left. A dot where an arrow's end is goes farther out,
    /// clear of it, as a press on the end takes hold of the end: one place
    /// offers one thing, and another arrow can still be pulled out of
    /// that side.
    pub(in crate::studio) fn dot_spots(&self, index: usize) -> [(i32, i32); 4] {
        let (x, y, w, h) = self.box_frame(index);
        let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
        let (cx, cy) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        let ends = self.arrow_ends();
        let taken = |spot: (i32, i32)| ends.iter().any(|&(end, ..)| (end.0 - spot.0).pow(2) + (end.1 - spot.1).pow(2) <= (END_REACH + NEAR).pow(2));
        let mut spots = [(cx, a.1 - OUT), (b.0 + OUT, cy), (cx, b.1 + OUT), (a.0 - OUT, cy)];
        for (spot, way) in spots.iter_mut().zip(SIDES) {
            if taken(*spot) {
                *spot = (spot.0 + way.0 as i32 * AWAY, spot.1 + way.1 as i32 * AWAY);
            }
        }
        spots
    }

    /// The ends of the board's arrows and lines in the window, each with
    /// its layer and which end it is.
    fn arrow_ends(&self) -> Vec<((i32, i32), usize, usize)> {
        let mut ends = Vec::new();
        for (index, layer) in self.doc.layers.iter().enumerate() {
            if let Kind::Shape(shape) = &layer.kind {
                if shape.sticks() && !layer.hidden && !layer.locked && shape.points.len() >= 2 {
                    ends.extend((0..2).map(|end| (self.to_window(shape.points[end]), index, end)));
                }
            }
        }
        ends
    }

    /// The end of an arrow or a line a press at a point of the window
    /// takes hold of, by its layer and which end: the nearest within reach,
    /// of the one picked out before the rest, then the topmost.
    pub(in crate::studio) fn arrow_end_at(&self, at: (i32, i32)) -> Option<(usize, usize)> {
        if self.tool != Tool::Select || self.space {
            return None;
        }
        let picked = self.picked();
        self.arrow_ends().into_iter().map(|(spot, index, end)| ((spot.0 - at.0).pow(2) + (spot.1 - at.1).pow(2), index, end)).filter(|&(away, ..)| away <= END_REACH.pow(2)).min_by_key(|&(away, index, _)| (picked != Some(index), away, std::cmp::Reverse(index))).map(|(_, index, end)| (index, end))
    }

    /// A press on an arrow's end: the arrow picked out and that end taken
    /// hold of, to be let go on another box, or on nothing. Before a dot
    /// of the box it is at, and before the box.
    pub(in crate::studio) fn press_arrow_end(&mut self, at: (f32, f32), at_window: (i32, i32)) -> bool {
        let Some((arrow, end)) = self.arrow_end_at(at_window) else { return false };
        self.selected = vec![arrow];
        self.checkpoint();
        self.held = vec![self.doc.layers[arrow].clone()];
        self.drag = Drag::Scale { pull: (END, end as i32), from: at };
        true
    }

    /// The pointer moved with nothing held on a board: the box whose dots
    /// show, the one it is over or still near, or the one picked out.
    pub(in crate::studio) fn board_hover(&mut self, at: (i32, i32), over_control: bool) {
        if self.board.history.as_ref().is_some_and(|history| history.sliding) {
            return self.history_at(at.0);
        }
        self.hover_pick(at, over_control);
        let now = self.dots_now(at, over_control);
        if now != self.board.dots_of {
            self.board.dots_of = now;
            self.dirty = true;
        }
    }

    /// The box whose dots show with the pointer at a point: the one shown
    /// while the pointer is still near it, else the smallest it is over or
    /// near, else the one picked out. Worked out afresh wherever it is
    /// asked, as what is picked out and where the boxes are change under a
    /// pointer that has not moved.
    pub(in crate::studio) fn dots_now(&self, at: (i32, i32), over_control: bool) -> Option<usize> {
        if self.space || over_control {
            return None;
        }
        // Out as far as its dots and a little more: near enough to show them.
        let near = |index: usize| {
            let spots = self.dot_spots(index);
            let (left, right) = (spots[3].0 - NEAR, spots[1].0 + NEAR);
            let (top, bottom) = (spots[0].1 - NEAR, spots[2].1 + NEAR);
            (left..=right).contains(&at.0) && (top..=bottom).contains(&at.1)
        };
        let kept = self.board.dots_of.filter(|&index| self.dots_show(index) && !self.doc.layers[index].hidden && near(index));
        // A box the pointer is over or near, hollow or not: the smallest.
        let area = |index: usize| self.box_frame(index).2 * self.box_frame(index).3;
        let under = || (0..self.doc.layers.len()).filter(|&index| self.dots_show(index) && !self.doc.layers[index].hidden && near(index)).min_by(|&a, &b| area(a).total_cmp(&area(b)));
        kept.or_else(under).or_else(|| self.picked().filter(|&index| self.dots_show(index)))
    }

    /// Whether a box's dots are to be shown: with the pointer in hand and
    /// nothing typed, or on the box being typed into, whatever is in hand.
    pub(in crate::studio) fn dots_show(&self, index: usize) -> bool {
        self.is_box(index) && !self.space && match self.editing {
            Some(editing) => editing == index,
            None => self.tool == Tool::Select,
        }
    }

    /// The pointer on a board for what a press there would do, past the
    /// handles: on a dot, or the middle of an arrow, the cross an arrow is
    /// pulled out with; on a shape, or the box being typed into, the move.
    pub(in crate::studio) fn board_cursor(&mut self) -> Option<Cursor> {
        let at = self.pointer;
        if self.control_at(at).is_some() {
            return None;
        }
        // An arrow's end, picked out or not: taken hold of to tie it
        // elsewhere, before the dot or the box it is at.
        if self.arrow_end_at(at).is_some() {
            return Some(Cursor::Move);
        }
        if let Some(index) = self.dots_now(at, false) {
            if self.dot_spots(index).iter().enumerate().any(|(side, &spot)| on_dot(spot, side, at)) {
                return Some(Cursor::Crosshair);
            }
        }
        let page = self.to_doc(at);
        if let Some(index) = self.editing {
            let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
            let on = page.0 >= x && page.0 <= x + w && page.1 >= y && page.1 <= y + h && self.layer_at(page).is_none_or(|top| top <= index);
            // Its words the I-beam, where a press puts the caret; the rest
            // of it the move.
            return match on {
                true if self.over_words(page) => Some(Cursor::Text),
                true => Some(Cursor::Move),
                false => None,
            };
        }
        if self.tool != Tool::Select {
            return None;
        }
        if self.arrow_middle_at(at).is_some() {
            return Some(Cursor::Crosshair);
        }
        self.layer_at(page).is_some().then_some(Cursor::Move)
    }

    /// A press on a dot of a box's side: an arrow begun out of it.
    pub(in crate::studio) fn press_dot(&mut self, at: (i32, i32)) -> bool {
        let Some(index) = self.dots_now(at, false) else { return false };
        let spots = self.dot_spots(index);
        let Some(side) = spots.iter().enumerate().position(|(side, &spot)| on_dot(spot, side, at)) else { return false };
        self.drag = Drag::Pull { from: index, side: side as u8, moved: false };
        self.held.clear();
        true
    }

    /// The pointer moved as an arrow is pulled out of a side.
    pub(in crate::studio) fn pull_moved(&mut self) {
        if let Drag::Pull { from, side, moved: false } = self.drag {
            let spot = self.pull_start(from, side);
            if (self.pointer.0 - spot.0).abs().max((self.pointer.1 - spot.1).abs()) > 4 {
                self.drag = Drag::Pull { from, side, moved: true };
            }
        }
        if let Drag::Pull { from, moved: true, .. } = self.drag {
            self.board.bind = self.pulled_onto(from);
            self.board.ports = self.board.bind.and_then(|onto| Some((onto, self.pulled_port(from, onto)?.0))).into_iter().collect();
        }
        self.dirty = true;
    }

    /// Where on a box an arrow pulled out of another would be put, with
    /// the pointer where it is: the middle of the side nearest it, or of
    /// the side facing the other where it is well inside, or a point of a
    /// plain box's edge it is right on; and that across and down it, as it
    /// is tied.
    fn pulled_port(&mut self, from: usize, onto: usize) -> Option<((f32, f32), (f32, f32))> {
        let anchor = self.doc.layers.get(onto)?.kind.clone().anchor_of(&mut self.text)?;
        let (u, v) = self.port_at(&anchor, self.to_doc(self.pointer));
        let (a, b) = (self.box_frame(from), self.box_frame(onto));
        let mut arrow = Shape::new(Tool::Arrow, INK, LINE, (a.0 + a.2 / 2.0, a.1 + a.3 / 2.0));
        arrow.points[1] = (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
        arrow.stuck = [Some((self.id_of(from), shapes::WHOLE, shapes::WHOLE)), Some((anchor.id, u, v))];
        arrow.routed = true;
        shapes::follow(std::iter::once(&mut arrow), &self.anchors());
        Some((arrow.points[1], (u, v)))
    }

    /// The box an arrow pulled out would be let go on or near, but the one
    /// it is pulled out of and those round that.
    fn pulled_onto(&mut self, from: usize) -> Option<usize> {
        if from >= self.doc.layers.len() {
            return None;
        }
        let at = self.to_doc(self.pointer);
        let mut skip = self.round(from);
        skip.push(self.id_of(from));
        if let Some(onto) = self.bind_target(at, &skip, true) {
            return Some(onto);
        }
        // Nor is a new box made over another: the box it would be made on
        // is what it was meant for, the one nearest the pointer.
        if self.mods.ctrl {
            return None;
        }
        let (_, _, w, h) = self.box_frame(from);
        let made = (at.0 - w / 2.0, at.1 - h / 2.0, w, h);
        // How far the pointer is from a box, and how large it is, for the
        // smaller of two as near.
        let away = |f: (f32, f32, f32, f32)| ((at.0 - at.0.clamp(f.0, f.0 + f.2)).hypot(at.1 - at.1.clamp(f.1, f.1 + f.3)), f.2 * f.3);
        (0..self.doc.layers.len()).filter(|&index| self.is_box(index) && !self.doc.layers[index].hidden && !skip.contains(&self.id_of(index))).filter(|&index| {
            let f = self.box_frame(index);
            f.0 < made.0 + made.2 && f.0 + f.2 > made.0 && f.1 < made.1 + made.3 && f.1 + f.3 > made.1
        }).min_by(|&a, &b| {
            let (a, b) = (away(self.box_frame(a)), away(self.box_frame(b)));
            a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
        })
    }

    /// An arrow pulled out of a side let go: onto a box, the two joined;
    /// on empty board, a new box there; with no drag, a new box the usual
    /// gap away that way. A new box is typed into.
    pub(in crate::studio) fn pull_released(&mut self, from: usize, side: u8, moved: bool) {
        if from >= self.doc.layers.len() {
            return;
        }
        let (x, y, w, h) = self.box_frame(from);
        // A click on an arrow's middle picks the arrow out.
        let branch = self.board.branching.take();
        if let (Some(arrow), false) = (branch, moved) {
            self.selected = vec![arrow];
            self.guess(false);
            return;
        }
        let (dx, dy) = SIDES[usize::from(side.min(3))];
        self.checkpoint();
        if moved {
            if let Some(onto) = self.pulled_onto(from) {
                // Into the other where it was let go, as the dot lit there
                // showed.
                let port = self.pulled_port(from, onto);
                let arrow = self.join(from, onto);
                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(arrow) {
                    if let (Some((id, ..)), Some((_, (u, v)))) = (shape.stuck[1], port) {
                        shape.stuck[1] = Some((id, u, v));
                    }
                }
                self.follow();
                self.selected = vec![onto];
                self.changed();
                return;
            }
        }
        let center = match moved {
            true => self.to_doc(self.pointer),
            false => (x + w / 2.0 + dx * (w + GAP_ACROSS), y + h / 2.0 + dy * (h + GAP_DOWN)),
        };
        let made = self.box_like(from, center, 0);
        self.join(from, made);
        self.selected = vec![made];
        self.edit_text(made);
        self.changed();
        self.guess(true);
    }

    /// A new box like another, its words none, with its middle at a point;
    /// `lighter` shades of its fill lighter, for each level down a map.
    pub(in crate::studio) fn box_like(&mut self, like: usize, center: (f32, f32), lighter: u32) -> usize {
        let Kind::Shape(source) = &self.doc.layers[like].kind else { return like };
        let mut shape = source.copied();
        let (_, _, w, h) = source.frame();
        let (w, h) = match source.base {
            (bw, bh) if bw > 0.0 && bh > 0.0 && !source.titled => (bw, bh),
            _ if source.titled => BOX,
            _ => (w, h),
        };
        (shape.text, shape.titled, shape.status, shape.votes, shape.editing, shape.angle) = (String::new(), false, 0, 0, false, 0.0);
        shape.points = vec![(center.0 - w / 2.0, center.1 - h / 2.0), (center.0 + w / 2.0, center.1 + h / 2.0)];
        shape.base = (w, h);
        if shape.filled {
            for _ in 0..lighter {
                shape.fill = canvas::mix(shape.fill, 0xFFFFFF, 90);
            }
        }
        shape.touch();
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// Two boxes joined by a board's arrow, from one to the other, going
    /// from side to side as they move. Gives the arrow's layer.
    pub(in crate::studio) fn join(&mut self, from: usize, to: usize) -> usize {
        let (a, b) = (self.box_frame(from), self.box_frame(to));
        let mut arrow = Shape::new(Tool::Arrow, self.board.stroke, LINE, (a.0 + a.2 / 2.0, a.1 + a.3 / 2.0));
        self.board_dress(&mut arrow);
        arrow.points[1] = (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
        arrow.stuck = [Some((self.id_of(from), shapes::WHOLE, shapes::WHOLE)), Some((self.id_of(to), shapes::WHOLE, shapes::WHOLE))];
        arrow.routed = true;
        self.doc.layers.push(Layer::shape(arrow));
        self.follow();
        self.doc.layers.len() - 1
    }

    /// What a board shows over everything on it: the dots of a box's sides,
    /// and the arrow being pulled out of one, with the box it would make.
    pub(in crate::studio) fn draw_board(&mut self, canvas: &mut Canvas, view: Rect) {
        self.draw_focus(canvas, view);
        if self.presenting() {
            self.draw_present(canvas, view);
            self.draw_laser(canvas);
            return;
        }
        if let Drag::Pull { from, side, moved: true } = self.drag {
            if from < self.doc.layers.len() {
                let spot = self.pull_start(from, side);
                let onto = self.board.bind.filter(|&index| self.is_box(index));
                let to = match onto {
                    // To the edge of the box it would be tied to, which glows.
                    Some(index) => {
                        let port = self.pulled_port(from, index).map_or(self.to_doc(self.pointer), |(at, _)| at);
                        self.draw_bind(canvas);
                        self.to_window(port)
                    }
                    None => {
                        let (_, _, w, h) = self.box_frame(from);
                        let (hw, hh) = ((w * self.zoom / 2.0) as i32, (h * self.zoom / 2.0) as i32);
                        dashed_box(canvas, Rect::new(self.pointer.0 - hw, self.pointer.1 - hh, hw * 2, hh * 2), ACCENT);
                        self.pointer
                    }
                };
                for shift in [0, 1] {
                    line(canvas, (spot.0, spot.1 + shift), (to.0, to.1 + shift), ACCENT);
                }
            }
        }
        if matches!(self.drag, Drag::Draw { .. } | Drag::Scale { .. } | Drag::Move { moved: true, .. }) {
            self.draw_bind(canvas);
        }
        self.draw_ghost(canvas);
        self.draw_pick(canvas);
        self.draw_drop(canvas);
        self.draw_laser(canvas);
        if self.drag == Drag::None {
            if let Some(index) = self.dots_now(self.pointer, self.hover_shown.is_some()) {
                for spot in self.dot_spots(index) {
                    canvas.disc(spot.0, spot.1, 6, ACCENT, 220);
                    canvas.disc(spot.0, spot.1, 2, 0xFFFFFF, 256);
                }
            }
        }
    }

    /// The arrow or line picked out on a board: a round handle at each end,
    /// white edged in the accent, and a smaller knob at its middle, the
    /// one an end held drawn larger; so it is plain that the arrow is
    /// what is taken hold of, as Excalidraw shows it.
    pub(in crate::studio) fn draw_arrow_handles(&mut self, canvas: &mut Canvas) {
        let Some(index) = self.picked() else { return };
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return };
        if !shape.sticks() || shape.points.len() < 2 || self.editing == Some(index) {
            return;
        }
        let held = match self.drag {
            Drag::Scale { pull: (END, end), .. } => Some(end as usize),
            _ => None,
        };
        let (ends, middle) = ([self.to_window(shape.points[0]), self.to_window(shape.points[1])], self.to_window(shape.middle()));
        if held.is_none() {
            canvas.disc(middle.0, middle.1, 4, ACCENT, 256);
            canvas.disc(middle.0, middle.1, 2, 0xFFFFFF, 256);
        }
        for (end, spot) in ends.into_iter().enumerate() {
            let big = if held == Some(end) || self.arrow_end_at(self.pointer) == Some((index, end)) { 7 } else { 6 };
            canvas.disc(spot.0, spot.1, big, ACCENT, 256);
            canvas.disc(spot.0, spot.1, big - 2, 0xFFFFFF, 256);
        }
    }

    /// After a drag on a board is let go, as a board has it: an arrow's
    /// end let go on a box stuck to it as a whole.
    pub(in crate::studio) fn board_after_release(&mut self, drag: Drag) {
        if let Some(history) = &mut self.board.history {
            history.sliding = false;
        }
        self.board.bind = None;
        self.board.ports.clear();
        // A box moved or sized as it is typed into: what is typed next is a
        // step of its own.
        // Whether the pointer went anywhere from where it was pressed.
        let now = self.to_doc(self.pointer);
        let changed = match drag {
            Drag::Move { moved, .. } => moved,
            Drag::Scale { from, .. } => (now.0 - from.0).hypot(now.1 - from.1) * self.zoom >= 3.0,
            _ => false,
        };
        if self.editing.is_some() && changed {
            self.board.type_step = true;
        }
        match drag {
            // Tied to what it is let go on or near, where the pointer is.
            Drag::Scale { pull, .. } if pull.0 == END => {
                if let Some(index) = self.picked().filter(|_| changed) {
                    let at = self.to_doc(self.pointer);
                    self.bind_end(index, pull.1 as usize, at);
                    self.changed();
                }
            }
            // A box sized by hand is that size from now on, its words
            // growing it from there.
            Drag::Scale { .. } => {
                for index in self.chosen() {
                    let (_, _, w, h) = self.box_frame(index);
                    if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                        if !shape.titled {
                            shape.base = (w, h);
                        }
                    }
                }
            }
            Drag::Move { moved: true, .. } => self.nest_moved(),
            // A box picked out: the end of a chain is offered its way back.
            Drag::Move { moved: false, .. } => {
                self.board.carried.clear();
                // On the box typed into, off its words: the caret to the end.
                if self.editing.is_some() {
                    (self.board.caret, self.board.anchor) = (None, None);
                }
                self.guess(false);
            }
            // A click in the words, with no drag across them, picks none.
            Drag::Words => {
                if self.board.anchor == self.board.caret {
                    self.board.anchor = None;
                }
            }
            _ => self.board.carried.clear(),
        }
    }
}

/// A box drawn in dashes, a pixel wide: what a pull would make.
pub(in crate::studio) fn dashed_box(canvas: &mut Canvas, rect: Rect, color: u32) {
    let (dash, gap) = (6, 5);
    let mut along = 0;
    while along < rect.w {
        let run = dash.min(rect.w - along);
        canvas.fill(rect.x + along, rect.y, run, 2, color);
        canvas.fill(rect.x + along, rect.bottom() - 2, run, 2, color);
        along += dash + gap;
    }
    along = 0;
    while along < rect.h {
        let run = dash.min(rect.h - along);
        canvas.fill(rect.x, rect.y + along, 2, run, color);
        canvas.fill(rect.right() - 2, rect.y + along, 2, run, color);
        along += dash + gap;
    }
}
