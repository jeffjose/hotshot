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
fn on_dot(spot: (i32, i32), way: (f32, f32), at: (i32, i32)) -> bool {
    let inward = -(way.0 * (at.0 - spot.0) as f32 + way.1 * (at.1 - spot.1) as f32);
    (spot.0 - at.0).abs() <= NEAR && (spot.1 - at.1).abs() <= NEAR && inward <= 3.0
}

impl Studio {
    /// A box's frame on the board: its left, top, width and height.
    pub(in crate::studio) fn box_frame(&self, index: usize) -> (f32, f32, f32, f32) {
        match &self.doc.layers[index].kind {
            Kind::Shape(shape) => shape.frame(),
            // A picture by the part of it shown, unturned, as a box's frame
            // is.
            Kind::Picture(picture) => (picture.cx - picture.w / 2.0, picture.cy - picture.h / 2.0, picture.w, picture.h),
        }
    }

    /// How far a box or a picture is turned, which its dots turn with.
    pub(in crate::studio) fn turned(&self, index: usize) -> f32 {
        match &self.doc.layers[index].kind {
            Kind::Shape(shape) => shape.angle,
            Kind::Picture(picture) => picture.angle,
        }
    }

    /// Which way each of a box's sides faces, turned with it: up, right,
    /// down, left.
    pub(in crate::studio) fn dot_ways(&self, index: usize) -> [(f32, f32); 4] {
        let (sin, cos) = self.turned(index).sin_cos();
        SIDES.map(|(x, y)| (x * cos - y * sin, x * sin + y * cos))
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
        let ends = self.arrow_ends();
        let taken = |spot: (i32, i32)| ends.iter().any(|&(end, ..)| (end.0 - spot.0).pow(2) + (end.1 - spot.1).pow(2) <= (END_REACH + NEAR).pow(2));
        let angle = self.turned(index);
        let mut spots = match angle == 0.0 {
            true => {
                let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
                let (cx, cy) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
                [(cx, a.1 - OUT), (b.0 + OUT, cy), (cx, b.1 + OUT), (a.0 - OUT, cy)]
            }
            // Turned, the middles of its sides turned with it.
            false => {
                let middle = (x + w / 2.0, y + h / 2.0);
                SIDES.map(|(u, v)| self.to_window(shapes::turn_point((middle.0 + u * w / 2.0, middle.1 + v * h / 2.0), middle, angle)))
            }
        };
        for (spot, way) in spots.iter_mut().zip(self.dot_ways(index)) {
            if taken(*spot) {
                *spot = (spot.0 + (way.0 * AWAY as f32).round() as i32, spot.1 + (way.1 * AWAY as f32).round() as i32);
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
            let (left, right) = (spots.iter().map(|s| s.0).min().unwrap_or(0) - NEAR, spots.iter().map(|s| s.0).max().unwrap_or(0) + NEAR);
            let (top, bottom) = (spots.iter().map(|s| s.1).min().unwrap_or(0) - NEAR, spots.iter().map(|s| s.1).max().unwrap_or(0) + NEAR);
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
        self.is_node(index) && !self.space && match self.editing {
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
            if self.dot_spots(index).into_iter().zip(self.dot_ways(index)).any(|(spot, way)| on_dot(spot, way, at)) {
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
        let Some(side) = spots.into_iter().zip(self.dot_ways(index)).position(|(spot, way)| on_dot(spot, way, at)) else { return false };
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
        if let Drag::Pull { from, side, moved: true } = self.drag {
            self.board.bind = self.pulled_onto(from, side);
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
        self.board_route(&mut arrow);
        shapes::follow(std::iter::once(&mut arrow), &self.anchors());
        Some((arrow.points[1], (u, v)))
    }

    /// Where the box an arrow pulled out onto empty board would be made: as
    /// large as the one it comes from, hung off the pointer the way the
    /// arrow is pulled, its near side at the pointer, so that the arrow
    /// goes on that way into it rather than turning back round it.
    pub(in crate::studio) fn pulled_box(&self, from: usize, side: u8) -> (f32, f32, f32, f32) {
        let (_, _, w, h) = self.box_frame(from);
        let at = self.to_doc(self.pointer);
        let start = self.to_doc(self.pull_start(from, side));
        let (dx, dy) = (at.0 - start.0, at.1 - start.1);
        // Out of a side, and not pulled back past it: on that side, its near
        // side at the pointer, slid along with it across, and clear of the
        // box it comes from: pulled as far across as along, or a little
        // more, it is still that way; much farther across, the other way.
        if let Some(&way) = SIDES.get(usize::from(side)) {
            let (along, across) = (dx * way.0 + dy * way.1, (dx * way.1 - dy * way.0).abs());
            if along > -6.0 / self.zoom && across <= 2.5 * along.max(0.0) + 40.0 / self.zoom {
                let (sx, sy, sw, sh) = self.box_frame(from);
                let gap = 12.0 / self.zoom;
                return match way {
                    (x, _) if x > 0.0 => (at.0.max(sx + sw + gap), at.1 - h / 2.0, w, h),
                    (x, _) if x < 0.0 => ((at.0 - w).min(sx - gap - w), at.1 - h / 2.0, w, h),
                    (_, y) if y > 0.0 => (at.0 - w / 2.0, at.1.max(sy + sh + gap), w, h),
                    _ => (at.0 - w / 2.0, (at.1 - h).min(sy - gap - h), w, h),
                };
            }
        }
        // Pulled back past its side, or from an arrow's middle: across or
        // down, whichever it has gone more; barely moved, the way the side
        // it comes out of faces.
        let across = match dx.abs().max(dy.abs()) * self.zoom < 6.0 {
            true => SIDES.get(usize::from(side)).is_none_or(|way| way.0 != 0.0),
            false => dx.abs() >= dy.abs(),
        };
        let forward = match across {
            true => if dx.abs() * self.zoom < 6.0 { SIDES.get(usize::from(side)).map_or(1.0, |way| way.0) >= 0.0 } else { dx >= 0.0 },
            false => if dy.abs() * self.zoom < 6.0 { SIDES.get(usize::from(side)).map_or(1.0, |way| way.1) >= 0.0 } else { dy >= 0.0 },
        };
        match (across, forward) {
            (true, true) => (at.0, at.1 - h / 2.0, w, h),
            (true, false) => (at.0 - w, at.1 - h / 2.0, w, h),
            (false, true) => (at.0 - w / 2.0, at.1, w, h),
            (false, false) => (at.0 - w / 2.0, at.1 - h, w, h),
        }
    }

    /// The arrow being pulled out of a box as it will be when let go: tied
    /// to the box it is over, at the port the pointer is nearest, or to the
    /// box that will be made under the pointer; routed between them as
    /// letting go routes it, in the board's look.
    pub(in crate::studio) fn pulled_arrow(&mut self, from: usize, onto: Option<usize>) -> Option<Shape> {
        let start = self.doc.layers.get(from)?.kind.clone().anchor_of(&mut self.text)?;
        let a = self.box_frame(from);
        let mut arrow = Shape::new(Tool::Arrow, self.board.stroke, LINE, (a.0 + a.2 / 2.0, a.1 + a.3 / 2.0));
        self.board_dress(&mut arrow);
        self.arrow_like(&mut arrow, from);
        self.board_route(&mut arrow);
        match onto {
            Some(index) => {
                let (end, _) = self.pulled_port(from, index)?;
                let anchor = self.doc.layers.get(index)?.kind.clone().anchor_of(&mut self.text)?;
                let (u, v) = self.port_at(&anchor, self.to_doc(self.pointer));
                arrow.points[1] = end;
                arrow.stuck = [Some((start.id, shapes::WHOLE, shapes::WHOLE)), Some((anchor.id, u, v))];
                // Back to itself: out of the side it is pulled from.
                if let (true, Drag::Pull { side, .. }) = (index == from, self.drag) {
                    let (su, sv) = shapes::PORTS[usize::from(side % 4)];
                    arrow.stuck[0] = Some((start.id, su, sv));
                }
                shapes::follow(std::iter::once(&mut arrow), &self.anchors());
            }
            None => {
                // The box that would be made, hung off the pointer.
                let side = if let Drag::Pull { side, .. } = self.drag { side } else { 1 };
                let (bx, by, bw, bh) = self.pulled_box(from, side);
                let middle = (bx + bw / 2.0, by + bh / 2.0);
                let made = shapes::Anchor { id: u32::MAX, middle, half: (a.2 / 2.0, a.3 / 2.0), angle: 0.0, round: false, pointed: false };
                arrow.points[1] = middle;
                arrow.stuck = [Some((start.id, shapes::WHOLE, shapes::WHOLE)), Some((made.id, shapes::WHOLE, shapes::WHOLE))];
                // Laid out as letting go lays it out, round corners in the
                // Diagram look and curved in the others, rather than always
                // curved until let go.
                let mut anchors = self.anchors();
                anchors.push(made);
                shapes::follow(std::iter::once(&mut arrow), &anchors);
            }
        }
        Some(arrow)
    }

    /// The box an arrow pulled out would be let go on or near, but the one
    /// it is pulled out of and those round that.
    fn pulled_onto(&mut self, from: usize, side: u8) -> Option<usize> {
        if from >= self.doc.layers.len() {
            return None;
        }
        let at = self.to_doc(self.pointer);
        let mut skip = self.round(from);
        skip.push(self.id_of(from));
        if let Some(onto) = self.bind_target(at, &skip, true) {
            return Some(onto);
        }
        // Back onto itself, nearest another of its sides than the one it is
        // pulled out of: a loop, from the box to itself.
        {
            let (x, y, w, h) = self.box_frame(from);
            let room = 12.0 / self.zoom;
            let inside = at.0 > x - room && at.0 < x + w + room && at.1 > y - room && at.1 < y + h + room;
            let anchor = self.doc.layers[from].kind.clone().anchor_of(&mut self.text);
            if let (true, Some(anchor)) = (inside, anchor) {
                let (u, v) = self.port_at(&anchor, at);
                if shapes::port_of(u, v).is_some_and(|port| port != usize::from(side % 4)) {
                    return Some(from);
                }
            }
        }
        // Nor is a new box made over another: the box it would be made on
        // is what it was meant for, the one nearest the pointer.
        if self.mods.ctrl {
            return None;
        }
        let made = self.pulled_box(from, side);
        // How far the pointer is from a box, and how large it is, for the
        // smaller of two as near.
        let away = |f: (f32, f32, f32, f32)| ((at.0 - at.0.clamp(f.0, f.0 + f.2)).hypot(at.1 - at.1.clamp(f.1, f.1 + f.3)), f.2 * f.3);
        (0..self.doc.layers.len()).filter(|&index| self.is_node(index) && !self.doc.layers[index].hidden && !skip.contains(&self.id_of(index))).filter(|&index| {
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
            if let Some(onto) = self.pulled_onto(from, side) {
                // Into the other where it was let go, as the dot lit there
                // showed.
                let port = self.pulled_port(from, onto);
                let arrow = self.join(from, onto);
                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(arrow) {
                    if let (Some((id, ..)), Some((_, (u, v)))) = (shape.stuck[1], port) {
                        shape.stuck[1] = Some((id, u, v));
                    }
                    // Back to itself: out of the side it was pulled from.
                    if onto == from {
                        if let Some((id, ..)) = shape.stuck[0] {
                            let (u, v) = shapes::PORTS[usize::from(side % 4)];
                            shape.stuck[0] = Some((id, u, v));
                        }
                    }
                }
                self.follow();
                self.selected = vec![onto];
                self.changed();
                return;
            }
        }
        let center = match moved {
            // Hung off the pointer the way it was pulled, as it was shown.
            true => {
                let (bx, by, bw, bh) = self.pulled_box(from, side);
                (bx + bw / 2.0, by + bh / 2.0)
            }
            // As far beside it as at its own size, drawn zoomed out or in.
            false => {
                let by = match &self.doc.layers[from].kind {
                    Kind::Shape(source) => bar::drawn_larger(source),
                    Kind::Picture(_) => 1.0,
                };
                (x + w / 2.0 + dx * (w + GAP_ACROSS * by), y + h / 2.0 + dy * (h + GAP_DOWN * by))
            }
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
        // Like a picture, a box as the box tool makes one.
        let Kind::Shape(source) = &self.doc.layers[like].kind else {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, (center.0 - BOX.0 / 2.0, center.1 - BOX.1 / 2.0));
            self.board_dress(&mut shape);
            shape.points[1] = (center.0 + BOX.0 / 2.0, center.1 + BOX.1 / 2.0);
            shape.base = BOX;
            shape.touch();
            self.doc.layers.push(Layer::shape(shape));
            return self.doc.layers.len() - 1;
        };
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

    /// An arrow from a box as heavy as that box's line, and its words as
    /// large as the box's, rather than as the zoom it is drawn at says: it
    /// joins what was drawn, at whatever size that was.
    pub(in crate::studio) fn arrow_like(&self, arrow: &mut Shape, from: usize) {
        if let Some(Kind::Shape(source)) = self.doc.layers.get(from).map(|layer| &layer.kind) {
            if matches!(source.tool, Tool::Rect | Tool::Ellipse) {
                (arrow.stroke, arrow.font_px) = (source.stroke, source.font_px);
            }
        }
    }

    /// How a board's new connector goes: round corners in the Diagram look,
    /// as a diagram's do, and bowed in the others.
    pub(in crate::studio) fn board_route(&self, arrow: &mut Shape) {
        let kind = self.board_route_kind();
        (arrow.routed, arrow.elbow) = (kind == 0, kind == 2);
    }

    /// How a board's next arrow goes: as last chosen in its panel, curved
    /// (0), straight (1) or round corners (2); with none chosen, round
    /// corners in the Diagram look and curved in the others.
    pub(in crate::studio) fn board_route_kind(&self) -> u8 {
        let diagram = self.pages[self.page].look == super::look::DIAGRAM;
        self.board.route.unwrap_or(if diagram { 2 } else { 0 })
    }

    /// Two boxes joined by a board's arrow, from one to the other, going
    /// from side to side as they move. Gives the arrow's layer.
    pub(in crate::studio) fn join(&mut self, from: usize, to: usize) -> usize {
        let (a, b) = (self.box_frame(from), self.box_frame(to));
        let mut arrow = Shape::new(Tool::Arrow, self.board.stroke, LINE, (a.0 + a.2 / 2.0, a.1 + a.3 / 2.0));
        self.board_dress(&mut arrow);
        self.arrow_like(&mut arrow, from);
        arrow.points[1] = (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
        arrow.stuck = [Some((self.id_of(from), shapes::WHOLE, shapes::WHOLE)), Some((self.id_of(to), shapes::WHOLE, shapes::WHOLE))];
        self.board_route(&mut arrow);
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
                let onto = self.board.bind.filter(|&index| self.is_node(index));
                match onto {
                    // The box it would be tied to glows.
                    Some(_) => self.draw_bind(canvas),
                    None => {
                        let (bx, by, bw, bh) = self.pulled_box(from, side);
                        let (a, b) = (self.to_window((bx, by)), self.to_window((bx + bw, by + bh)));
                        dashed_box(canvas, Rect::new(a.0, a.1, b.0 - a.0, b.1 - a.1), ACCENT);
                    }
                }
                // The arrow as it will be once let go: routed and curved as
                // it will be, and its head; a straight line where it cannot
                // be worked out.
                match self.pulled_arrow(from, onto) {
                    Some(arrow) => {
                        // Worked out at the zoom it is seen at, as the arrow
                        // itself is drawn, so that its head is as large as it
                        // will be: past the page's corner, by the pan.
                        let mut seen = super::scaled(&arrow, self.zoom);
                        let shift = (self.doc.x * self.zoom + self.pan.0, self.doc.y * self.zoom + self.pan.1);
                        // In the board's look, as it is drawn once let go:
                        // clean and square in Clean and Diagram, rather than
                        // by hand until it is let go.
                        let page = &self.pages[self.page];
                        look::dress(&mut seen, page.look, 0, self.zoom);
                        if !seen.sketch {
                            let filled = |end: usize| seen.heads[end] && seen.ends[end] == shapes::FILLED_HEAD;
                            let strokes = seen.line_strokes();
                            for (at, stroke) in strokes.iter().enumerate() {
                                let points: Vec<(f32, f32)> = stroke.iter().map(|&(x, y)| (x + shift.0, y + shift.1)).collect();
                                // A head filled, as it will be: the heads
                                // follow the way, the end's own first.
                                let head_of = if seen.heads[1] { [1, 0] } else { [0, 1] };
                                if at > 0 && points.len() > 2 && filled(head_of[(at - 1).min(1)]) {
                                    let (left, top) = points.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
                                    let (right, bottom) = points.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
                                    let bounds = Rect::new(left.floor() as i32 - 1, top.floor() as i32 - 1, (right - left).ceil() as i32 + 3, (bottom - top).ceil() as i32 + 3);
                                    canvas.mask(&shapes::fill_path(bounds, &points), ACCENT, 256);
                                }
                                halo(canvas, &points, 1.1, ACCENT, 256);
                            }
                        } else {
                            for ink in seen.sketch_strokes() {
                                let mut points: Vec<(f32, f32)> = ink.points.iter().map(|&(x, y)| (x + shift.0, y + shift.1)).collect();
                                // A head filled, as it will be.
                                if ink.filled && points.len() > 2 {
                                    points.push(points[0]);
                                    let ring: Vec<(f32, f32)> = points.clone();
                                    let (left, top) = ring.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
                                    let (right, bottom) = ring.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
                                    let bounds = Rect::new(left.floor() as i32 - 1, top.floor() as i32 - 1, (right - left).ceil() as i32 + 3, (bottom - top).ceil() as i32 + 3);
                                    canvas.mask(&shapes::fill_path(bounds, &ring), ACCENT, 256);
                                }
                                halo(canvas, &points, 1.1, ACCENT, 256);
                            }
                        }
                    }
                    None => {
                        for shift in [0, 1] {
                            line(canvas, (spot.0, spot.1 + shift), (self.pointer.0, self.pointer.1 + shift), ACCENT);
                        }
                    }
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
        if self.drag == Drag::None && self.board_tool() != LASER_TOOL {
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
        // A box sized by hand: its words fitted to it again, larger where
        // there is room now, smaller where there is less.
        if let (Drag::Scale { .. }, true) = (drag, changed) {
            for index in self.chosen() {
                if matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.nominal > 0 && matches!(shape.tool, Tool::Rect | Tool::Ellipse)) {
                    self.fit_words(index, false);
                }
            }
        }
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
#[cfg(test)]
mod preview_tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn an_arrow_pulled_out_in_diagram_or_clean_is_shown_clean_as_it_will_be_not_by_hand() {
        // Pulled straight out to the right: the line shown along the row of
        // the box's middle, as drawn once let go; by hand it wanders off it.
        for (look, clean) in [(crate::studio::board::look::DIAGRAM, true), (2, true), (1, false)] {
            let mut studio = board();
            studio.set_look(Some(look), None);
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, 300.0), (260.0, 372.0), Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected.clear();
            let over = studio.to_window_page((180.0, 336.0));
            studio.pointer_moved(over.0, over.1, Mods::default());
            let right = studio.dot_spots(0)[1];
            studio.pointer_moved(right.0, right.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            for step in 1..=20 {
                studio.pointer_moved(right.0 + step * 20, right.1, Mods::default());
            }
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            // Each column's middle of the line, by how blue each row is.
            let blue = |x: i32, y: i32| {
                let pixel = out[(y * studio.width + x) as usize] & 0xFF_FFFF;
                let (r, g, b) = ((pixel >> 16 & 0xFF) as f32, (pixel >> 8 & 0xFF) as f32, (pixel & 0xFF) as f32);
                (b - (r + g) / 2.0).max(0.0)
            };
            let middles: Vec<f32> = (right.0 + 24..right.0 + 140).filter_map(|x| {
                let (weight, sum) = (right.1 - 30..right.1 + 30).fold((0.0, 0.0), |(w, s), y| (w + blue(x, y), s + blue(x, y) * y as f32));
                (weight > 50.0).then(|| sum / weight)
            }).collect();
            assert!(middles.len() > 60, "look {look}: the arrow shown");
            let (low, high) = middles.iter().fold((f32::MAX, f32::MIN), |(l, h), &m| (l.min(m), h.max(m)));
            assert_eq!(high - low < 0.6, clean, "look {look}: its middle from {low} to {high}");
            studio.released();
        }
    }

    #[test]
    fn an_arrow_pulled_out_into_the_open_is_shown_laid_out_as_it_will_be_once_let_go() {
        // Down and to the right of the box, in Diagram: round corners while
        // pulled, as once let go; and curved in Rounded the same way.
        for look in [crate::studio::board::look::DIAGRAM, 0] {
            let mut studio = board();
            studio.set_look(Some(look), None);
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected.clear();
            let over = studio.to_window_page((180.0, 136.0));
            studio.pointer_moved(over.0, over.1, Mods::default());
            let right = studio.dot_spots(0)[1];
            studio.pointer_moved(right.0, right.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            for step in 1..=20 {
                studio.pointer_moved(right.0 + step * 18, right.1 + step * 12, Mods::default());
            }
            let ghost = studio.pulled_arrow(0, None).expect("a ghost");
            studio.released();
            let made = studio.doc.layers.iter().rev().find_map(|layer| match &layer.kind {
                Kind::Shape(shape) if shape.tool == Tool::Arrow => Some(shape.clone()),
                _ => None,
            }).expect("an arrow made");
            assert_eq!((ghost.elbow, ghost.routed), (made.elbow, made.routed), "look {look}");
            let near = |a: &[(f32, f32)], b: &[(f32, f32)]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p.0 - q.0).abs() < 2.0 && (p.1 - q.1).abs() < 2.0);
            assert!(near(&ghost.path(), &made.path()), "look {look}: the ghost {:?} as made {:?}", ghost.points, made.points);
        }
    }
}
