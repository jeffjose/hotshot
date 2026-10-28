//! Arrows from a box's edge, with no arrow tool: the pointer over a box
//! shows a dot on each of its sides. Drag one out and let go on another
//! box, and the two are joined; on empty board, and a new box is made
//! there, joined and typed into. A click on a dot makes the new box at the
//! usual gap that way.

use super::*;

/// How far out from a side its dot is, and how near it a press takes it,
/// in pixels of the window.
const OUT: i32 = 14;
const NEAR: i32 = 9;
/// How far the next box goes from one, across and down.
pub(in crate::studio) const GAP_ACROSS: f32 = 80.0;
pub(in crate::studio) const GAP_DOWN: f32 = 48.0;

/// Which way each side faces: up, right, down, left.
pub(in crate::studio) const SIDES: [(f32, f32); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];

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
    /// bottom and left.
    pub(in crate::studio) fn dot_spots(&self, index: usize) -> [(i32, i32); 4] {
        let (x, y, w, h) = self.box_frame(index);
        let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
        let (cx, cy) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        [(cx, a.1 - OUT), (b.0 + OUT, cy), (cx, b.1 + OUT), (a.0 - OUT, cy)]
    }

    /// The pointer moved with nothing held on a board: the box whose dots
    /// show, the one it is over or still near, or the one picked out.
    pub(in crate::studio) fn board_hover(&mut self, at: (i32, i32), over_control: bool) {
        let shows = self.tool == Tool::Select && !self.space && !over_control && self.editing.is_none();
        let near = |studio: &Studio, index: usize| {
            let spots = studio.dot_spots(index);
            let (left, right) = (spots[3].0 - NEAR, spots[1].0 + NEAR);
            let (top, bottom) = (spots[0].1 - NEAR, spots[2].1 + NEAR);
            (left..=right).contains(&at.0) && (top..=bottom).contains(&at.1)
        };
        let kept = self.board.dots_of.filter(|&index| shows && self.is_box(index) && near(self, index));
        let now = kept.or_else(|| self.hovered.filter(|&index| shows && self.is_box(index))).or_else(|| self.picked().filter(|&index| shows && self.is_box(index)));
        if now != self.board.dots_of {
            self.board.dots_of = now;
            self.dirty = true;
        }
    }

    /// A press on a dot of a box's side: an arrow begun out of it.
    pub(in crate::studio) fn press_dot(&mut self, at: (i32, i32)) -> bool {
        let Some(index) = self.board.dots_of.filter(|&index| self.is_box(index)) else { return false };
        let spots = self.dot_spots(index);
        let Some(side) = spots.iter().position(|spot| (spot.0 - at.0).abs() <= NEAR && (spot.1 - at.1).abs() <= NEAR) else { return false };
        self.drag = Drag::Pull { from: index, side: side as u8, moved: false };
        self.held.clear();
        true
    }

    /// The pointer moved as an arrow is pulled out of a side.
    pub(in crate::studio) fn pull_moved(&mut self) {
        if let Drag::Pull { from, side, moved: false } = self.drag {
            let spot = self.dot_spots(from)[usize::from(side)];
            if (self.pointer.0 - spot.0).abs().max((self.pointer.1 - spot.1).abs()) > 4 {
                self.drag = Drag::Pull { from, side, moved: true };
            }
        }
        self.dirty = true;
    }

    /// The box an arrow pulled out would be let go on, but the one it is
    /// pulled out of.
    fn pulled_onto(&mut self, from: usize) -> Option<usize> {
        let at = self.to_doc(self.pointer);
        (0..self.doc.layers.len()).rev().find(|&index| index != from && self.is_box(index) && !self.doc.layers[index].hidden && {
            let (x, y, w, h) = self.box_frame(index);
            at.0 >= x && at.0 <= x + w && at.1 >= y && at.1 <= y + h
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
        let (dx, dy) = SIDES[usize::from(side)];
        self.checkpoint();
        if moved {
            if let Some(onto) = self.pulled_onto(from) {
                self.join(from, onto);
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
    pub(in crate::studio) fn draw_board(&mut self, canvas: &mut Canvas, _view: Rect) {
        if let Drag::Pull { from, side, moved: true } = self.drag {
            if from < self.doc.layers.len() {
                let spot = self.dot_spots(from)[usize::from(side)];
                let onto = self.pulled_onto(from);
                let to = match onto {
                    Some(index) => {
                        let (x, y, w, h) = self.box_frame(index);
                        let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
                        canvas.round_outline(a.0 - 3, a.1 - 3, b.0 - a.0 + 6, b.1 - a.1 + 6, 6, 2, ACCENT);
                        self.pointer
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
        if self.drag == Drag::None {
            if let Some(index) = self.board.dots_of.filter(|&index| self.is_box(index)) {
                for spot in self.dot_spots(index) {
                    canvas.disc(spot.0, spot.1, 6, ACCENT, 220);
                    canvas.disc(spot.0, spot.1, 2, 0xFFFFFF, 256);
                }
            }
        }
    }

    /// After a drag on a board is let go, as a board has it: an arrow's
    /// end let go on a box stuck to it as a whole.
    pub(in crate::studio) fn board_after_release(&mut self, drag: Drag) {
        match drag {
            Drag::Scale { pull, .. } if pull.0 == END => {
                if let Some(index) = self.picked() {
                    self.board_stick(index, &[pull.1 as usize]);
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
