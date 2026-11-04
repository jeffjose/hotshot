//! Arrows tied to what they are let go near, as Excalidraw binds them: an
//! end dragged over a box, or within a finger's width of one, is tied to
//! it, the box glowing and the end drawn at its edge while it is dragged;
//! the nearest wins, and the box inside another before the one round it.
//! Ctrl held ties it to nothing, to leave the end where it is, as in
//! Excalidraw; Alt is a copy on a board, as everywhere.

use super::*;

/// How near a shape an arrow's end is tied to it, in pixels of the window.
pub(in crate::studio) const REACH: f32 = 20.0;
/// How near a plain box's edge an end let go is tied at that point of it,
/// rather than at the middle of a side, and how far from those it has to
/// be: in pixels of the window.
const ON_EDGE: f32 = 6.0;
const CLEAR: f32 = 24.0;
/// How near a box's dot an end of an arrow moved whole is drawn to it, in
/// pixels of the window.
pub(in crate::studio) const MAGNET: f32 = 16.0;

/// The key held while an end is dragged that ties it to nothing.
fn unbound(mods: Mods) -> bool {
    mods.ctrl
}

impl Studio {
    /// What an arrow's end at a point of the board would be tied to, by its
    /// layer: of the shapes it is on or within reach of, the nearest, a box
    /// before the one it is in, the topmost of those as near; none of those
    /// called as `skip` says, and only boxes where `boxes` says. Nothing
    /// with Ctrl held.
    pub(in crate::studio) fn bind_target(&mut self, at: (f32, f32), skip: &[u32], boxes: bool) -> Option<usize> {
        if unbound(self.mods) {
            return None;
        }
        let reach = REACH / self.zoom;
        let anchors: Vec<(usize, shapes::Anchor)> = self.anchors().into_iter().filter(|anchor| !skip.contains(&anchor.id)).filter_map(|anchor| Some((self.index_of(anchor.id)?, anchor))).filter(|&(index, _)| !boxes || self.is_node(index)).collect();
        let away = |anchor: &shapes::Anchor| {
            let (lx, ly) = shapes::turn_point(at, anchor.middle, -anchor.angle);
            let (dx, dy) = (((lx - anchor.middle.0).abs() - anchor.half.0).max(0.0), ((ly - anchor.middle.1).abs() - anchor.half.1).max(0.0));
            dx.hypot(dy)
        };
        let near: Vec<(usize, shapes::Anchor, f32)> = anchors.into_iter().map(|(index, anchor)| (index, anchor, away(&anchor))).filter(|&(_, _, away)| away <= reach).collect();
        // One round another that is also near gives way to it, as what is
        // in a box is what is aimed at, the box being all round it.
        let holds = |outer: &shapes::Anchor, inner: &shapes::Anchor| {
            outer.id != inner.id && (inner.middle.0 - outer.middle.0).abs() + inner.half.0 <= outer.half.0 + 0.5 && (inner.middle.1 - outer.middle.1).abs() + inner.half.1 <= outer.half.1 + 0.5
        };
        near.iter().filter(|(_, outer, _)| !near.iter().any(|(_, inner, _)| holds(outer, inner))).min_by(|a, b| a.2.total_cmp(&b.2).then(b.0.cmp(&a.0))).map(|&(index, ..)| index)
    }

    /// The boxes round one, by what each is called: what an arrow pulled
    /// out of it is not tied to.
    pub(in crate::studio) fn round(&self, index: usize) -> Vec<u32> {
        let (x, y, w, h) = self.box_frame(index);
        (0..self.doc.layers.len()).filter(|&other| other != index && self.is_box(other)).filter(|&other| {
            let (ox, oy, ow, oh) = self.box_frame(other);
            ox <= x && oy <= y && ox + ow >= x + w && oy + oh >= y + h
        }).map(|other| self.id_of(other)).collect()
    }

    /// An end of a line on a board tied to what is at or near a point: a
    /// box, an oval or a picture at a dot of a side, a point of its edge
    /// or as a whole, to go from side to side as it moves; words as a
    /// whole; or to nothing. Not to the box the other end is tied to,
    /// as an arrow from a box back into it is no arrow. Gives what it is
    /// tied to.
    pub(in crate::studio) fn bind_end(&mut self, index: usize, end: usize, at: (f32, f32)) -> Option<usize> {
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return None };
        if !shape.sticks() {
            return None;
        }
        // Not to what the other end is tied to.
        let other = shape.stuck[1 - end].map(|(id, ..)| id);
        let target = self.bind_target(at, &other.into_iter().collect::<Vec<_>>(), false);
        let anchor = target.and_then(|target| self.doc.layers[target].kind.clone().anchor_of(&mut self.text));
        let spot = match (target, anchor) {
            (Some(target), Some(anchor)) => Some(match &self.doc.layers[target].kind {
                // A box or a picture at a dot of a side, or a point of its
                // edge, or as a whole.
                _ if self.is_node(target) => {
                    let (u, v) = self.port_at(&anchor, at);
                    (anchor.id, u, v)
                }
                _ => (anchor.id, shapes::WHOLE, shapes::WHOLE),
            }),
            _ => None,
        };
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) else { return None };
        shape.stuck[end] = spot;
        shape.routed = true;
        // A bend is the route's between two boxes, and goes with it; an
        // end aimed at a middle goes straight.
        let sided = |stuck: Option<(u32, f32, f32)>| stuck.is_some_and(|(_, u, v)| (u == shapes::WHOLE && v != shapes::AIMED) || shapes::port_of(u, v).is_some());
        if !(sided(shape.stuck[0]) && sided(shape.stuck[1])) && shape.points.len() > 2 {
            shape.points.truncate(2);
            shape.touch();
        }
        target
    }

    /// Where on a box an end let go at a point is tied: in the middle half
    /// of it, aimed at its middle, on its outline on the way there from
    /// the other end; else at the middle of the side nearest it, where its
    /// dot is; or, on a plain box's edge well away from those, at that
    /// point of the edge. Across and down it, -1 to 1.
    pub(in crate::studio) fn port_at(&self, anchor: &shapes::Anchor, at: (f32, f32)) -> (f32, f32) {
        let (lx, ly) = shapes::turn_point(at, anchor.middle, -anchor.angle);
        let (hw, hh) = (anchor.half.0.max(1.0), anchor.half.1.max(1.0));
        let (dx, dy) = (lx - anchor.middle.0, ly - anchor.middle.1);
        // In the middle half of it each way, aimed at its middle.
        if dx.abs() < hw * 0.5 && dy.abs() < hh * 0.5 {
            return (shapes::WHOLE, shapes::AIMED);
        }
        if !anchor.round && !anchor.pointed {
            let (on, clear) = (ON_EDGE / self.zoom, CLEAR / self.zoom);
            let within = dx.abs() <= hw + on && dy.abs() <= hh + on;
            let (across, down) = ((dx.abs() - hw).abs(), (dy.abs() - hh).abs());
            let edge = match across <= down {
                true => (dx.signum(), (dy / hh).clamp(-1.0, 1.0)),
                false => ((dx / hw).clamp(-1.0, 1.0), dy.signum()),
            };
            let far = shapes::PORTS.iter().all(|&(u, v)| (dx - u * hw).hypot(dy - v * hh) > clear);
            if within && across.min(down) <= on && far {
                return edge;
            }
        }
        *shapes::PORTS.iter().min_by(|a, b| (dx - a.0 * hw).hypot(dy - a.1 * hh).total_cmp(&(dx - b.0 * hw).hypot(dy - b.1 * hh))).unwrap_or(&(0.0, 0.0))
    }

    /// An end of a line being dragged on a board: tied as it would be let
    /// go, there and then, so that it is drawn at the edge of what it
    /// would be tied to, and that glows.
    pub(in crate::studio) fn bind_live(&mut self, index: usize, end: usize, at: (f32, f32)) {
        let target = self.bind_end(index, end, at);
        self.follow();
        let mut ports = Vec::new();
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.touch();
            if let (Some(target), Some(&point)) = (target, shape.points.get(end)) {
                ports.push((target, point));
            }
        }
        if ports != self.board.ports {
            self.board.ports = ports;
            self.dirty = true;
        }
        self.changed_layers(&[index]);
        if target != self.board.bind {
            self.board.bind = target;
            self.dirty = true;
        }
    }

    /// An arrow or a line moved whole on a board, its ends tied to nothing
    /// as it goes, as Excalidraw lets go of them: an end that comes near a
    /// box's dot is drawn to it, the arrow moving with it so that the end
    /// is on the dot, and tied there, the box glowing and the dot lit; the
    /// nearer end first, then the other if it is then near a dot of
    /// another box. Ctrl held, neither is.
    pub(in crate::studio) fn magnet(&mut self, index: usize) {
        let mut ports = Vec::new();
        let target = match self.mods.ctrl {
            true => None,
            false => self.magnet_ends(index, &mut ports),
        };
        if ports != self.board.ports || target != self.board.bind {
            (self.board.ports, self.board.bind) = (ports, target);
            self.dirty = true;
        }
    }

    fn magnet_ends(&mut self, index: usize, ports: &mut Vec<(usize, (f32, f32))>) -> Option<usize> {
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return None };
        if !shape.sticks() || shape.points.len() < 2 || shape.stuck.iter().any(Option::is_some) {
            return None;
        }
        let ends = [shape.points[0], shape.points[1]];
        let reach = MAGNET / self.zoom;
        let picked: Vec<usize> = (0..self.doc.layers.len()).filter(|&other| other != index && self.is_node(other) && !self.doc.layers[other].hidden).collect();
        let boxes: Vec<(usize, shapes::Anchor)> = picked.into_iter().filter_map(|other| Some((other, self.doc.layers[other].kind.clone().anchor_of(&mut self.text)?))).collect();
        // The dot nearest a point, within reach, but of a box left out.
        let nearest = |at: (f32, f32), not: Option<usize>| {
            boxes.iter().filter(|(other, _)| Some(*other) != not).flat_map(|(other, anchor)| (0..4).map(move |side| (*other, anchor.id, side, shapes::side_point(anchor, side).0))).map(|(other, id, side, spot)| (other, id, side, spot, (spot.0 - at.0).hypot(spot.1 - at.1))).filter(|found| found.4 <= reach).min_by(|a, b| a.4.total_cmp(&b.4))
        };
        let found = [nearest(ends[0], None), nearest(ends[1], None)];
        let first = match (found[0], found[1]) {
            (Some(a), Some(b)) => usize::from(b.4 < a.4),
            (Some(_), None) => 0,
            (None, Some(_)) => 1,
            (None, None) => return None,
        };
        let (target, id, side, spot, _) = found[first]?;
        let by = (spot.0 - ends[first].0, spot.1 - ends[first].1);
        let other = 1 - first;
        let second = nearest((ends[other].0 + by.0, ends[other].1 + by.1), Some(target));
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) else { return None };
        for point in &mut shape.points {
            (point.0, point.1) = (point.0 + by.0, point.1 + by.1);
        }
        let port = |id: u32, side: usize| (id, shapes::PORTS[side].0, shapes::PORTS[side].1);
        shape.stuck[first] = Some(port(id, side));
        ports.push((target, spot));
        if let Some((target, id, side, spot, _)) = second {
            shape.stuck[other] = Some(port(id, side));
            shape.points[other] = spot;
            ports.push((target, spot));
        }
        shape.routed = true;
        shape.touch();
        self.follow();
        Some(target)
    }

    /// What an end dragged would be tied to, glowing round its edge, with
    /// the middles of its sides an end is put at shown on it, the one it
    /// would be put at filled.
    pub(in crate::studio) fn draw_bind(&mut self, canvas: &mut Canvas) {
        if !matches!(self.drag, Drag::Draw { .. } | Drag::Scale { .. } | Drag::Pull { .. } | Drag::Move { moved: true, .. }) {
            return;
        }
        let mut lit: Vec<(usize, Option<(f32, f32)>)> = self.board.ports.iter().map(|&(target, at)| (target, Some(at))).collect();
        if let Some(target) = self.board.bind.filter(|target| !lit.iter().any(|(lit, _)| lit == target)) {
            lit.push((target, None));
        }
        for (target, at) in lit {
            if target >= self.doc.layers.len() {
                continue;
            }
            // On its outline, where an end tied to it meets it: a box by
            // its own frame, anything else by what it covers; a soft glow
            // just outside, touching it.
            let (x, y, w, h) = match self.is_node(target) {
                true => self.box_frame(target),
                false => self.doc.layers[target].bounds(&mut self.text),
            };
            match self.is_node(target) && self.turned(target) != 0.0 {
                // Turned, round its turned outline.
                true => {
                    let corners = self.box_corners(target);
                    let path: Vec<(f32, f32)> = corners.iter().chain(corners.first()).map(|&(x, y)| (x as f32, y as f32)).collect();
                    halo(canvas, &path, 4.5, ACCENT, 90);
                    for pair in path.windows(2) {
                        for nudge in 0..2 {
                            line(canvas, (pair[0].0 as i32 + nudge, pair[0].1 as i32), (pair[1].0 as i32 + nudge, pair[1].1 as i32), ACCENT);
                        }
                    }
                }
                false => {
                    let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
                    canvas.round_outline(a.0 - 4, a.1 - 4, b.0 - a.0 + 8, b.1 - a.1 + 8, 6, 3, canvas::mix(ACCENT, 0xFFFFFF, 170));
                    canvas.round_outline(a.0 - 1, a.1 - 1, b.0 - a.0 + 2, b.1 - a.1 + 2, 3, 2, ACCENT);
                }
            }
            if !self.is_node(target) {
                continue;
            }
            let Some(anchor) = self.doc.layers[target].kind.clone().anchor_of(&mut self.text) else { continue };
            let at = at.map(|at| self.to_window(at));
            for side in 0..4 {
                let spot = self.to_window(shapes::side_point(&anchor, side).0);
                match at.is_some_and(|at| (at.0 - spot.0).abs() <= 1 && (at.1 - spot.1).abs() <= 1) {
                    true => {
                        canvas.disc(spot.0, spot.1, 8, canvas::mix(ACCENT, 0xFFFFFF, 140), 256);
                        canvas.disc(spot.0, spot.1, 6, ACCENT, 256);
                        canvas.disc(spot.0, spot.1, 2, 0xFFFFFF, 256);
                    }
                    false => {
                        canvas.disc(spot.0, spot.1, 5, ACCENT, 256);
                        canvas.disc(spot.0, spot.1, 3, 0xFFFFFF, 256);
                    }
                }
            }
        }
    }
}

impl Kind {
    /// What an arrow can be tied to of a layer, as `Studio::anchors` has it.
    pub(in crate::studio) fn anchor_of(self, text: &mut Text) -> Option<shapes::Anchor> {
        match self {
            Kind::Picture(picture) => Some(shapes::Anchor { id: picture.id, middle: (picture.cx, picture.cy), half: (picture.w / 2.0, picture.h / 2.0), angle: picture.angle, round: false, pointed: false }),
            Kind::Shape(shape) => shape.anchor(text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;
    use crate::studio::board::dots::{END_REACH, NEAR, OUT};

    /// Two boxes drawn on a board, A at the left and B at the right, with
    /// the pointer in hand and nothing picked out.
    fn two_boxes() -> Studio {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Enter, Mods::default());
        pull(&mut studio, (600.0, 100.0), (760.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!((studio.board_tool(), studio.chosen().len()), (SELECT, 0));
        studio
    }

    /// A drag from a box's right dot to a point of the board, its middle
    /// looked at before it is let go.
    fn pull_dot(studio: &mut Studio, from: usize, to: (f32, f32), mods: Mods, looked: impl FnOnce(&mut Studio)) {
        let (x, y, w, h) = studio.box_frame(from);
        let over = studio.to_window_page((x + w / 2.0, y + h / 2.0));
        studio.pointer_moved(over.0, over.1, Mods::default());
        let dot = studio.dot_spots(from)[1];
        studio.pointer_moved(dot.0, dot.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.to_window_page(to);
        studio.pointer_moved((dot.0 + to.0) / 2, (dot.1 + to.1) / 2, mods);
        studio.pointer_moved(to.0, to.1, mods);
        looked(studio);
        studio.released();
    }



    #[test]
    fn a_click_on_empty_board_inside_a_curved_arrow_s_box_is_not_on_the_arrow() {
        let mut studio = two_boxes();
        // Out of A's right dot into the top of B: a curve.
        let (bx, by, bw, _) = studio.box_frame(1);
        pull_dot(&mut studio, 0, (bx + bw / 2.0, by + 2.0), Mods::default(), |_| {});
        let arrow = studio.doc.layers.len() - 1;
        studio.selected = vec![arrow];
        let shape = shape(&studio, arrow);
        // A point inside the box round the curve, well off the curve.
        let (xs, ys): (Vec<f32>, Vec<f32>) = shape.points.iter().copied().unzip();
        let (left, right) = (xs.iter().copied().fold(f32::MAX, f32::min), xs.iter().copied().fold(f32::MIN, f32::max));
        let (top, bottom) = (ys.iter().copied().fold(f32::MAX, f32::min), ys.iter().copied().fold(f32::MIN, f32::max));
        let mut off = None;
        for i in 0..=20 {
            for j in 0..=20 {
                let p = (left + (right - left) * i as f32 / 20.0, top + (bottom - top) * j as f32 / 20.0);
                let clear = (0..studio.doc.layers.len()).all(|index| !studio.doc.layers[index].clone().hit(p, &mut studio.text));
                if off.is_none() && clear && (p.1 - (top + bottom) / 2.0).abs() < (bottom - top) / 3.0 {
                    off = Some(p);
                }
            }
        }
        let Some(off) = off else { return };
        let at = studio.to_window_page(off);
        click(&mut studio, at);
        assert!(!studio.selected.contains(&arrow), "clicked off the curve at {off:?}");
    }


    #[test]
    fn the_knob_at_the_middle_of_an_arrow_picked_out_bends_it_through_the_pointer() {
        let (mut studio, arrow) = joined();
        studio.selected = vec![arrow];
        let before = shape(&studio, arrow);
        let knob = studio.to_window(before.middle());
        let to = (before.middle().0, before.middle().1 + 120.0);
        studio.pointer_moved(knob.0, knob.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        assert!(matches!(studio.drag, Drag::Scale { pull: (BEND, 0), .. }), "{:?}", studio.drag);
        let at = studio.to_window(to);
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.released();
        let bent = shape(&studio, arrow);
        assert_eq!(studio.doc.layers.len(), 4, "no branch");
        assert_eq!((bent.points.len(), bent.routed), (3, false));
        assert!((bent.middle().1 - to.1).abs() < 2.0, "through the pointer: {:?} {to:?}", bent.middle());
        assert_eq!((bent.stuck[0].map(|s| s.0), bent.stuck[1].map(|s| s.0)), (before.stuck[0].map(|s| s.0), before.stuck[1].map(|s| s.0)), "still joined");
        // B moved: the arrow keeps its bend, its end goes with B.
        studio.selected.clear();
        let (bx, by, _, _) = studio.box_frame(1);
        pull(&mut studio, (bx + 20.0, by + 20.0), (bx + 20.0, by + 80.0), Mods::default());
        let after = shape(&studio, arrow);
        assert_eq!(after.points.len(), 3, "the bend kept");
        assert!((after.points[1].1 - bent.points[1].1 - 60.0).abs() < 2.0, "{:?} {:?}", after.points[1], bent.points[1]);
    }


    /// CUJ: a box drawn, an arrow pulled a little out of its right side:
    /// the new box it would make, and the arrow into it, go on right, the
    /// arrow never turning back left round a box made over the first.
    #[test]
    fn an_arrow_pulled_a_little_right_of_a_box_goes_right_into_the_box_it_makes() {
        for by in [12.0, 30.0, 60.0] {
            let mut studio = board();
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            let (x, y, w, h) = studio.box_frame(0);
            pull_dot(&mut studio, 0, (x + w + by, y + h / 2.0), Mods::default(), |studio| {
                let ghost = studio.pulled_box(0, 1);
                assert!(ghost.0 >= x + w - 0.5, "{by}: the box that would be made right of the first: {ghost:?}");
                let arrow = studio.pulled_arrow(0, None).expect("the arrow shown");
                assert!(arrow.points[1].0 > arrow.points[0].0, "{by}: shown pointing right: {:?}", arrow.points);
            });
            let made = studio.box_frame(1);
            assert!(made.0 >= x + w - 0.5, "{by}: made right of the first: {made:?}");
            let arrow = shape(&studio, 2);
            assert!(arrow.points[1].0 > arrow.points[0].0, "{by}: pointing right: {:?}", arrow.points);
        }
        // Pulled right and as far down, from the right side: still right,
        // slid down with the pointer, never over the first box.
        for down in [20.0, 60.0, 110.0] {
            let mut studio = board();
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            let (x, y, w, h) = studio.box_frame(0);
            pull_dot(&mut studio, 0, (x + w + 30.0, y + h / 2.0 + down), Mods::default(), |studio| {
                let ghost = studio.pulled_box(0, 1);
                assert!(ghost.0 >= x + w, "{down}: right of the first: {ghost:?}");
                assert!((ghost.1 + ghost.3 / 2.0 - (y + h / 2.0 + down)).abs() < 2.0, "{down}: level with the pointer: {ghost:?}");
                let arrow = studio.pulled_arrow(0, None).expect("the arrow shown");
                assert!(arrow.points[1].0 > arrow.points[0].0, "{down}: shown pointing right: {:?}", arrow.points);
            });
            let arrow = shape(&studio, 2);
            assert!(arrow.points[1].0 > arrow.points[0].0, "{down}: pointing right: {:?}", arrow.points);
        }
    }

    #[test]
    fn an_arrow_picked_out_has_its_heads_and_its_way_set_from_the_panel() {
        let (mut studio, arrow) = joined();
        // Only with an arrow picked out are its heads offered.
        studio.selected = vec![0];
        studio.layout();
        assert!(studio.control_rect(Control::Board(Ctl::Head(1, 0))).is_none());
        studio.selected = vec![arrow];
        studio.layout();
        let press = |studio: &mut Studio, ctl: Ctl| {
            let at = studio.control_rect(Control::Board(ctl)).expect("offered").center();
            click(studio, at);
        };
        // No head at its end, a dot at its start.
        press(&mut studio, Ctl::Head(1, 0));
        press(&mut studio, Ctl::Head(0, 3));
        let headed = shape(&studio, arrow);
        assert_eq!((headed.heads, headed.ends[0]), ([true, false], shapes::DOT_HEAD));
        // Round a corner; straight; curved again.
        press(&mut studio, Ctl::Route(2));
        assert!(shape(&studio, arrow).elbow);
        press(&mut studio, Ctl::Route(1));
        let straight = shape(&studio, arrow);
        assert!(!straight.elbow && !straight.routed && straight.points.len() == 2);
        press(&mut studio, Ctl::Route(0));
        assert!(shape(&studio, arrow).routed);
        // Each a step to undo.
        studio.undo();
        assert!(!shape(&studio, arrow).routed);
    }

    #[test]
    fn an_arrow_pulled_from_a_dot_back_into_its_own_box_loops_round_to_another_side() {
        let mut studio = two_boxes();
        let (x, y, w, h) = studio.box_frame(0);
        // Out of A's right side and back in near its bottom: A glows, and
        // its preview is the loop as it will be.
        pull_dot(&mut studio, 0, (x + w / 2.0, y + h - 4.0), Mods::default(), |studio| {
            assert_eq!(studio.board.bind, Some(0), "A glows");
            let preview = studio.pulled_arrow(0, Some(0)).expect("a preview");
            assert_eq!(preview.points.len(), 5, "round a corner");
        });
        assert_eq!(studio.doc.layers.len(), 3, "an arrow, no third box");
        let arrow = shape(&studio, 2);
        let id = shape(&studio, 0).id;
        assert_eq!((arrow.stuck[0], arrow.stuck[1]), (Some((id, 1.0, 0.0)), Some((id, 0.0, 1.0))));
        assert_eq!(arrow.points[0], (x + w, y + h / 2.0));
        assert_eq!(arrow.points[1], (x + w / 2.0, y + h));
        // Out past the corner between them, not across the box.
        assert!(arrow.points[3].0 > x + w && arrow.points[3].1 > y + h);
        // A moved, it goes with it.
        studio.selected.clear();
        pull(&mut studio, (x + 30.0, y + 20.0), (x + 70.0, y + 20.0), Mods::default());
        let arrow = shape(&studio, 2);
        assert!((arrow.points[0].0 - (x + w + 40.0)).abs() < 1.0, "{:?}", arrow.points[0]);
    }

    #[test]
    fn an_arrow_pulled_from_a_dot_is_tied_to_a_box_it_is_let_go_near_not_only_on() {
        let mut studio = two_boxes();
        // Let go a little short of B, above it: B glows, and is joined.
        pull_dot(&mut studio, 0, (590.0, 92.0), Mods::default(), |studio| assert_eq!(studio.board.bind, Some(1), "B glows"));
        assert_eq!(studio.doc.layers.len(), 3, "no third box");
        let arrow = shape(&studio, 2);
        assert_eq!((arrow.stuck[0].map(|s| s.0), arrow.stuck[1].map(|s| s.0)), (Some(shape(&studio, 0).id), Some(shape(&studio, 1).id)));
        assert_eq!(studio.board.bind, None, "nothing glows once let go");
        // Farther off, where the new box would be made over B: B, again.
        pull_dot(&mut studio, 0, (550.0, 150.0), Mods::default(), |studio| assert_eq!(studio.board.bind, Some(1)));
        assert_eq!(studio.doc.layers.len(), 4, "no box made over another");
        studio.doc.layers.pop();
        // Well clear of everything: a new box there, as before.
        pull_dot(&mut studio, 0, (400.0, 500.0), Mods::default(), |_| {});
        assert_eq!(studio.doc.layers.len(), 5);
        // Near B with Ctrl held: tied to nothing, so a new box.
        studio.commit_text();
        pull_dot(&mut studio, 0, (590.0, 92.0), Mods { ctrl: true, ..Mods::default() }, |studio| assert_eq!(studio.board.bind, None));
        assert_eq!(studio.doc.layers.len(), 7);
    }

    #[test]
    fn a_box_deleted_leaves_its_arrows_where_they_were_tied_to_nothing_and_undo_ties_them_again() {
        let mut studio = two_boxes();
        pull_dot(&mut studio, 0, (680.0, 136.0), Mods::default(), |_| {});
        assert_eq!(studio.doc.layers.len(), 3);
        studio.commit_text();
        let before = shape(&studio, 2);
        let (a, b) = (shape(&studio, 0).id, shape(&studio, 1).id);
        studio.selected = vec![1];
        press_key(&mut studio, Key::Delete, Mods::default());
        assert_eq!(studio.doc.layers.len(), 2, "only B gone");
        let left = shape(&studio, 1);
        assert_eq!((left.id, left.stuck[0].map(|s| s.0), left.stuck[1]), (before.id, Some(a), None), "its head tied to nothing, its tail still to A");
        assert_eq!(left.points, before.points, "drawn where it was");
        // Moving A still takes the tail along, and the head stays.
        studio.selected = vec![0];
        let middle = studio.to_window_page((180.0, 136.0));
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(middle.0, middle.1 + 40, Mods::default());
        studio.pointer_moved(middle.0, middle.1 + 80, Mods::default());
        studio.released();
        let moved = shape(&studio, 1);
        assert_eq!(moved.points[1], before.points[1], "the free head stays put");
        assert_ne!(moved.points[0], before.points[0], "the tail goes with A");
        studio.undo();
        studio.undo();
        assert_eq!(studio.doc.layers.len(), 3, "B back");
        assert_eq!((shape(&studio, 2).stuck[0].map(|s| s.0), shape(&studio, 2).stuck[1].map(|s| s.0)), (Some(a), Some(b)), "tied to both again in the same step");
    }

    #[test]
    fn the_pointer_says_what_a_press_will_do() {
        let mut studio = two_boxes();
        let at = |studio: &mut Studio, page: (f32, f32)| {
            let window = studio.to_window_page(page);
            studio.pointer_moved(window.0, window.1, Mods::default());
            studio.cursor()
        };
        assert_eq!(at(&mut studio, (180.0, 120.0)), Cursor::Move, "a shape moves");
        assert_eq!(at(&mut studio, (400.0, 400.0)), Cursor::Arrow, "empty board");
        let dot = studio.dot_spots(0)[1];
        studio.pointer_moved(dot.0, dot.1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Crosshair, "a dot pulls an arrow");
        // Picked out: its corner sizes it.
        studio.selected = vec![0];
        assert_eq!(at(&mut studio, (260.0, 172.0)), Cursor::Falling);
        // Typed into: its words the I-beam, the rest of it the move.
        studio.edit_text(0);
        type_in(&mut studio, "Queue");
        assert_eq!(at(&mut studio, (180.0, 136.0)), Cursor::Text);
        assert_eq!(at(&mut studio, (110.0, 110.0)), Cursor::Move);
    }

    #[test]
    fn the_panel_of_styles_stays_docked_at_the_right_whatever_is_picked_out() {
        let mut studio = two_boxes();
        // A box just under A: the panel is at the right, clear of both.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 230.0), (260.0, 290.0), Mods::default());
        let layout = studio.layout();
        let bar = layout.board_bar.unwrap();
        let a = Rect::spanning(studio.to_window_page((100.0, 100.0)), studio.to_window_page((260.0, 290.0)));
        assert!(!bar.intersects(a) && bar.right() > studio.width - 220, "{bar:?} docked, clear of {a:?}");
        // A box inside another: the panel is in the same place, clear of
        // the one round it, where a click on that box is not taken by it.
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (500.0, 300.0), (560.0, 340.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let inner = studio.doc.layers.len() - 1;
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 200.0), (800.0, 500.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected = vec![inner];
        let layout = studio.layout();
        let bar = layout.board_bar.unwrap();
        let outer = Rect::spanning(studio.to_window_page((300.0, 200.0)), studio.to_window_page((800.0, 500.0)));
        assert!(!bar.intersects(outer), "{bar:?} clear of {outer:?}");
    }

    #[test]
    fn dots_show_for_where_the_pointer_is_now_not_where_it_last_rested() {
        let mut studio = two_boxes();
        let over = studio.to_window_page((680.0, 136.0));
        studio.pointer_moved(over.0, over.1, Mods::default());
        assert_eq!(studio.board.dots_of, Some(1));
        // A marquee dragged round both, let go well away.
        pull(&mut studio, (200.0, 250.0), (700.0, 150.0), Mods::default());
        let away = studio.to_window_page((900.0, 500.0));
        studio.pointer = away;
        assert_eq!(studio.chosen().len(), 2);
        let pointer = studio.pointer;
        assert_eq!(studio.dots_now(pointer, false), None, "no dots left on B");
        // Just outside a box, short of its dots: they show, and a press on
        // one pulls.
        let near = studio.to_window_page((272.0, 136.0));
        studio.pointer_moved(near.0, near.1, Mods::default());
        assert_eq!(studio.board.dots_of, Some(0));
    }

    #[test]
    fn the_nearest_shape_wins_and_a_box_inside_another_before_the_one_round_it() {
        let mut studio = two_boxes();
        // A small box drawn inside B.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (700.0, 300.0), (760.0, 340.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let small = studio.doc.layers.len() - 1;
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (600.0, 260.0), (900.0, 420.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let big = studio.doc.layers.len() - 1;
        let skip = [];
        // Inside the big box near the small one: the small one.
        assert_eq!(studio.bind_target((690.0, 320.0), &skip, false), Some(small));
        // Inside the big box away from it: the big one.
        assert_eq!(studio.bind_target((620.0, 400.0), &skip, false), Some(big));
        // Between A and B, nearer B: B.
        assert_eq!(studio.bind_target((585.0, 136.0), &skip, false), Some(1));
        assert_eq!(studio.bind_target((275.0, 136.0), &skip, false), Some(0));
        // Out of reach: nothing.
        assert_eq!(studio.bind_target((430.0, 136.0), &skip, false), None);
    }

    #[test]
    fn the_arrow_tool_ties_both_ends_to_boxes_near_them_and_ends_freely_on_empty_board() {
        let mut studio = two_boxes();
        press_key(&mut studio, Key::Char('a'), Mods::default());
        // From just right of A to just left of B, on neither.
        let (from, to) = (studio.to_window_page((272.0, 140.0)), studio.to_window_page((588.0, 130.0)));
        studio.pointer_moved(from.0, from.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(to.0 - 100, to.1, Mods::default());
        studio.pointer_moved(to.0, to.1, Mods::default());
        assert_eq!(studio.board.bind, Some(1), "B glows as the end nears it");
        let live = shape(&studio, 2);
        let left = shape(&studio, 1).frame();
        assert_eq!(live.points[1], (left.0, left.1 + left.3 / 2.0), "drawn touching B at the middle of its left side, its dot");
        studio.released();
        let arrow = shape(&studio, 2);
        assert_eq!((arrow.stuck[0].map(|s| s.0), arrow.stuck[1].map(|s| s.0)), (Some(shape(&studio, 0).id), Some(shape(&studio, 1).id)));
        // Onto empty board: the arrow ends there, and no box is made.
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (300.0, 400.0), (500.0, 500.0), Mods::default());
        assert_eq!(studio.doc.layers.len(), 4);
        assert_eq!(shape(&studio, 3).stuck, [None, None]);
    }

    /// A press at a point of the window, dragged in small steps to a point
    /// of the board, looked at before it is let go.
    fn drag_from(studio: &mut Studio, from: (i32, i32), to: (f32, f32), mods: Mods, looked: impl FnOnce(&mut Studio)) {
        studio.pointer_moved(from.0, from.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.to_window_page(to);
        for step in 1..=6 {
            let along = |a: i32, b: i32| a + (b - a) * step / 6;
            studio.pointer_moved(along(from.0, to.0), along(from.1, to.1), mods);
        }
        looked(studio);
        studio.released();
    }

    /// A, B and C, with A joined to B by an arrow pulled out of A's right
    /// dot, nothing picked out: the arrow's layer.
    fn joined() -> (Studio, usize) {
        let mut studio = two_boxes();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (600.0, 400.0), (760.0, 472.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        // Into B by its left dot, let go just off it.
        pull_dot(&mut studio, 0, (592.0, 136.0), Mods::default(), |_| {});
        studio.commit_text();
        studio.selected.clear();
        let arrow = studio.doc.layers.len() - 1;
        assert!(shape(&studio, arrow).sticks());
        (studio, arrow)
    }

    #[test]
    fn an_arrows_end_at_a_boxs_dot_is_taken_hold_of_rather_than_a_new_arrow_pulled_out() {
        let (mut studio, arrow) = joined();
        let count = studio.doc.layers.len();
        let tail = studio.to_window(shape(&studio, arrow).points[0]);
        // Where A's right dot would be is right by the arrow's tail: the
        // tail wins, from anywhere a few pixels round it.
        let (x, y, w, h) = studio.box_frame(0);
        let side = studio.to_window_page((x + w, y + h / 2.0));
        assert!((side.0 + OUT - tail.0).abs() <= OUT.max(2) && (side.1 - tail.1).abs() <= NEAR, "the dot by the tail, {side:?} {tail:?}");
        drag_from(&mut studio, (tail.0 + 3, tail.1 - 2), (680.0, 390.0), Mods::default(), |studio| {
            assert!(matches!(studio.drag, Drag::Scale { pull: (END, 0), .. }), "the tail held, not a new arrow: {:?}", studio.drag);
            assert_eq!(studio.board.bind, Some(2), "C glows");
        });
        assert_eq!(studio.doc.layers.len(), count, "no new arrow, no new box");
        let moved = shape(&studio, arrow);
        assert_eq!(moved.stuck[0].map(|s| s.0), Some(shape(&studio, 2).id), "tied to C");
        assert_eq!(moved.stuck[1].map(|s| s.0), Some(shape(&studio, 1).id), "still to B");
        assert_eq!(studio.chosen(), vec![arrow], "the arrow picked out");
        // Its head, by B's left dot, the same; let go on empty board with
        // Ctrl held, tied to nothing there.
        studio.selected.clear();
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (900.0, 600.0), Mods { ctrl: true, ..Mods::default() }, |studio| assert_eq!(studio.board.bind, None));
        let free = shape(&studio, arrow);
        assert_eq!((studio.doc.layers.len(), free.stuck[1]), (count, None));
        assert!((free.points[1].0 - 900.0).abs() < 1.0 && (free.points[1].1 - 600.0).abs() < 1.0);
        // And back onto A, from empty board.
        let head = studio.to_window(free.points[1]);
        drag_from(&mut studio, head, (180.0, 136.0), Mods::default(), |studio| assert_eq!(studio.board.bind, Some(0)));
        assert_eq!(shape(&studio, arrow).stuck[1].map(|s| s.0), Some(shape(&studio, 0).id));
    }

    #[test]
    fn an_arrows_end_partway_along_a_side_is_taken_hold_of_rather_than_the_box() {
        let mut studio = two_boxes();
        // An arrow tied to nothing, its end on A's right side well above
        // its middle.
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (420.0, 300.0), (260.0, 110.0), Mods { ctrl: true, ..Mods::default() });
        let arrow = studio.doc.layers.len() - 1;
        assert_eq!(shape(&studio, arrow).stuck, [None, None]);
        studio.selected.clear();
        let a = shape(&studio, 0).frame();
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        // The pointer over it says the end moves; a press there takes it.
        studio.pointer_moved(head.0 - 4, head.1 + 3, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Move);
        drag_from(&mut studio, (head.0 - 4, head.1 + 3), (680.0, 120.0), Mods::default(), |studio| assert_eq!(studio.board.bind, Some(1)));
        assert_eq!(shape(&studio, arrow).stuck[1].map(|s| s.0), Some(shape(&studio, 1).id));
        assert_eq!(shape(&studio, 0).frame(), a, "A not moved");
    }

    #[test]
    fn a_boxs_dot_where_an_arrows_end_is_goes_farther_out_and_still_pulls_another() {
        let (mut studio, arrow) = joined();
        let tail = studio.to_window(shape(&studio, arrow).points[0]);
        let over = studio.to_window_page((180.0, 136.0));
        studio.pointer_moved(over.0, over.1, Mods::default());
        assert_eq!(studio.board.dots_of, Some(0));
        let (x, y, w, h) = studio.box_frame(0);
        let (right, bottom) = (studio.to_window_page((x + w, y + h)).0, studio.to_window_page((x + w, y + h)).1);
        let dots = studio.dot_spots(0);
        assert!((dots[1].0 - tail.0).abs() > END_REACH + NEAR, "the right dot, where the tail is, clear of it: {:?} {tail:?}", dots[1]);
        assert_eq!(dots[2].1, bottom + OUT, "the rest where they were");
        assert!(dots[1].0 > right + OUT);
        // Between the two, the cursor says which: the end moves, the dot
        // pulls.
        studio.pointer_moved(tail.0, tail.1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Move);
        studio.pointer_moved(dots[1].0, dots[1].1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Crosshair);
        // The dot moved out pulls another arrow out of that side.
        let count = studio.doc.layers.len();
        drag_from(&mut studio, dots[1], (420.0, 420.0), Mods::default(), |studio| assert!(matches!(studio.drag, Drag::Pull { .. })));
        assert_eq!(studio.doc.layers.len(), count + 2, "a new box and an arrow to it");
    }

    #[test]
    fn an_arrow_picked_out_is_lit_along_its_curve_not_along_a_straight_line_and_a_box_by_one_crisp_pixel() {
        let (mut studio, arrow) = joined();
        // Bent, as B is moved down from A.
        studio.selected = vec![1];
        let middle = studio.to_window_page((680.0, 136.0));
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(middle.0, middle.1 + 100, Mods::default());
        studio.pointer_moved(middle.0, middle.1 + 200, Mods::default());
        studio.released();
        let path = shape(&studio, arrow).path();
        assert!(path.len() > 4, "curved");
        let pixels = |studio: &mut Studio| -> Vec<u32> {
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            out.into_iter().map(|pixel| pixel & 0xFF_FFFF).collect()
        };
        let blue = |pixel: u32| (pixel & 0xFF) as i32 - ((pixel >> 16) & 0xFF) as i32;
        studio.pointer = (5, 5);
        studio.selected.clear();
        let bare = pixels(&mut studio);
        studio.selected = vec![arrow];
        let picked = pixels(&mut studio);
        let width = studio.width;
        let (zoom, pan, origin) = (studio.zoom, studio.pan, (studio.doc.x, studio.doc.y));
        let window = move |p: (f32, f32)| (((p.0 + origin.0) * zoom + pan.0).round() as i32, ((p.1 + origin.1) * zoom + pan.1).round() as i32);
        let at = |pixels: &[u32], p: (f32, f32), dy: i32| pixels[((window(p).1 + dy) * width + window(p).0) as usize];
        // Beside the curve, a little off its ink: tinted blue, as it was not.
        let along = path[path.len() / 3];
        assert!(blue(at(&picked, along, 4)) > blue(at(&bare, along, 4)) + 20, "lit beside the curve");
        // On the straight line from its tail to its head, off the curve:
        // nothing.
        let (a, b) = (path[0], path[path.len() - 1]);
        let chord = ((a.0 * 2.0 + b.0) / 3.0, (a.1 * 2.0 + b.1) / 3.0);
        let off = path.iter().map(|p| (p.0 - chord.0).hypot(p.1 - chord.1)).fold(f32::MAX, f32::min);
        assert!(off > 12.0, "the chord well off the curve, {off}");
        assert_eq!(at(&picked, chord, 0), at(&bare, chord, 0), "no straight outline");
        // Hovered, not picked out: lit too, but fainter.
        studio.selected.clear();
        studio.hovered = Some(arrow);
        let hovered = pixels(&mut studio);
        let (lit, faint) = (blue(at(&picked, along, 4)), blue(at(&hovered, along, 4)));
        assert!(faint > blue(at(&bare, along, 4)) && faint < lit, "fainter: {faint} against {lit}");
        // A box picked out, or hovered: one pixel of the accent across its
        // left side, over its line rather than beside it.
        studio.hovered = None;
        studio.selected = vec![0];
        let (x, y, _, h) = studio.box_frame(0);
        let edge = studio.to_window((x, y + h / 4.0));
        let across = |pixels: &[u32]| (edge.0 - 4..=edge.0 + 4).filter(|&x| pixels[(edge.1 * width + x) as usize] == ACCENT).collect::<Vec<i32>>();
        let inked = |x: i32| (bare[(edge.1 * width + x) as usize] & 0xFF) < 0x90;
        let picked = pixels(&mut studio);
        let lit = across(&picked);
        assert!(lit.len() == 1 && (lit[0] - 1..=lit[0] + 1).any(inked), "one pixel, on its line, which wobbles a pixel either way as it is drawn by hand: {lit:?}");
        studio.selected.clear();
        studio.hovered = Some(0);
        let hovered = pixels(&mut studio);
        assert_eq!(across(&hovered), lit, "hovered the same");
    }

    #[test]
    fn an_arrow_picked_out_shows_round_handles_at_its_ends_and_its_middle_and_no_straight_outline_while_an_end_is_dragged() {
        let (mut studio, arrow) = joined();
        // Bent, as it goes from A down to C.
        let (x, y, w, h) = studio.box_frame(1);
        studio.selected = vec![1];
        let middle = studio.to_window_page((x + w / 2.0, y + h / 2.0));
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(middle.0, middle.1 + 100, Mods::default());
        studio.pointer_moved(middle.0, middle.1 + 200, Mods::default());
        studio.released();
        studio.selected = vec![arrow];
        let shape_now = shape(&studio, arrow);
        let ends = [studio.to_window(shape_now.points[0]), studio.to_window(shape_now.points[1])];
        let knob = studio.to_window(shape_now.middle());
        let pixels: Vec<u32> = { let mut out = vec![0u32; (studio.width * studio.height) as usize]; studio.draw(&mut out); out.into_iter().map(|pixel| pixel & 0xFF_FFFF).collect() };
        let width = studio.width;
        let at = |p: (i32, i32)| pixels[(p.1 * width + p.0) as usize];
        for end in ends {
            assert_eq!(at(end), 0xFFFFFF, "a white handle on its end");
            // Round and larger than a square handle: its edge some five
            // pixels out every way, but not out at its corners.
            let edged = |dx: i32, dy: i32| (4..=6).any(|d| at((end.0 + dx * d, end.1 + dy * d)) == ACCENT);
            assert!(edged(1, 0) && edged(-1, 0) && edged(0, 1) && edged(0, -1), "edged in the accent, round");
            assert_ne!(at((end.0 + 5, end.1 + 5)), ACCENT, "no corner");
        }
        assert_eq!((at(knob), at((knob.0 - 3, knob.1))), (0xFFFFFF, ACCENT), "its middle knob");
        // The straight line from its tail to its head is not drawn over it
        // as it is dragged.
        let (a, b) = (ends[0], ends[1]);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 300_000);
        studio.pointer_moved(b.0 + 30, b.1 + 30, Mods::default());
        studio.pointer_moved(b.0 + 60, b.1 + 60, Mods::default());
        let pixels: Vec<u32> = { let mut out = vec![0u32; (studio.width * studio.height) as usize]; studio.draw(&mut out); out.into_iter().map(|pixel| pixel & 0xFF_FFFF).collect() };
        let straight = (3..17).map(|t| (a.0 + (b.0 - a.0) * t / 20, a.1 + (b.1 - a.1) * t / 20)).filter(|&(x, y)| pixels[(y * width + x) as usize] == ACCENT).count();
        studio.released();
        assert_eq!(straight, 0, "no outline of the route it had");
    }

    #[test]
    fn an_end_dragged_to_a_third_box_is_tied_there_and_to_nothing_on_empty_board() {
        let mut studio = two_boxes();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (600.0, 400.0), (760.0, 472.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        pull_dot(&mut studio, 0, (680.0, 136.0), Mods::default(), |_| {});
        let arrow = studio.doc.layers.len() - 1;
        studio.selected = vec![arrow];
        // Its head dragged near the third box.
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        studio.pointer_moved(head.0, head.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let near = studio.to_window_page((680.0, 390.0));
        studio.pointer_moved(near.0, near.1 - 40, Mods::default());
        studio.pointer_moved(near.0, near.1, Mods::default());
        assert_eq!(studio.board.bind, Some(2));
        studio.released();
        let tied = shape(&studio, arrow);
        assert_eq!(tied.stuck[1].map(|s| s.0), Some(shape(&studio, 2).id));
        // Moved with the box.
        let before = tied.points[1];
        studio.selected = vec![2];
        let middle = studio.to_window_page((680.0, 436.0));
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.pressed(1, 300_000);
        studio.pointer_moved(middle.0 + 50, middle.1 + 30, Mods::default());
        studio.pointer_moved(middle.0 + 100, middle.1 + 60, Mods::default());
        studio.released();
        assert!(shape(&studio, arrow).points[1] != before && shape(&studio, arrow).stuck[1].is_some(), "the end goes with the box");
        // Dragged off to empty board: tied to nothing, straight, there.
        studio.selected = vec![arrow];
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        studio.pointer_moved(head.0, head.1, Mods::default());
        studio.pressed(1, 500_000);
        let empty = studio.to_window_page((300.0, 600.0));
        studio.pointer_moved(empty.0, empty.1 - 40, Mods::default());
        studio.pointer_moved(empty.0, empty.1, Mods::default());
        studio.released();
        let free = shape(&studio, arrow);
        assert_eq!((free.stuck[1], free.points.len()), (None, 2));
        assert!((free.points[1].0 - 300.0).abs() < 1.0 && (free.points[1].1 - 600.0).abs() < 1.0);
    }

    #[test]
    fn an_end_dragged_near_a_box_is_put_at_the_dot_of_its_nearest_side_lit_as_it_is_dragged() {
        let mut studio = two_boxes();
        // A free arrow off to the left of B.
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (420.0, 300.0), (440.0, 320.0), Mods { ctrl: true, ..Mods::default() });
        let arrow = studio.doc.layers.len() - 1;
        studio.selected.clear();
        let b = shape(&studio, 1).frame();
        let (left, top) = ((b.0, b.1 + b.3 / 2.0), (b.0 + b.2 / 2.0, b.1));
        // Its head dragged just above B, a little left of the middle of its
        // top: put at the dot there, which is lit, drawn touching it.
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (b.0 + b.2 / 2.0 - 15.0, b.1 - 10.0), Mods::default(), |studio| {
            assert_eq!(studio.board.bind, Some(1));
            assert_eq!(shape(studio, arrow).points[1], top, "drawn at the top dot");
            assert_eq!(studio.board.ports, vec![(1, top)], "that dot lit");
            let lit = studio.to_window(top);
            let pixels: Vec<u32> = { let mut out = vec![0u32; (studio.width * studio.height) as usize]; studio.draw(&mut out); out.into_iter().map(|pixel| pixel & 0xFF_FFFF).collect() };
            let at = |p: (i32, i32)| pixels[(p.1 * studio.width + p.0) as usize];
            assert_eq!(at((lit.0 + 4, lit.1)), ACCENT, "filled in the accent");
            let other = studio.to_window(left);
            assert_eq!(at(other), 0xFFFFFF, "the other dots shown, hollow");
            assert!((3..=5).any(|d| at((other.0 + d, other.1)) == ACCENT), "edged in the accent");
        });
        assert_eq!(shape(&studio, arrow).stuck[1], Some((shape(&studio, 1).id, 0.0, -1.0)), "tied at the middle of the top");
        assert_eq!(shape(&studio, arrow).points[1], top);
        // Right on B's left edge, well away from its dots: at that point.
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (b.0 + 1.0, b.1 + 6.0), Mods::default(), |_| {});
        let (_, u, v) = shape(&studio, arrow).stuck[1].unwrap();
        assert_eq!(u, -1.0);
        assert!(v < -0.5 && v > -1.0, "partway up the left side, {v}");
        // Just off that edge, within reach but not on it: its dot.
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (b.0 - 14.0, b.1 + 6.0), Mods::default(), |_| {});
        assert_eq!(shape(&studio, arrow).stuck[1].map(|(_, u, v)| (u, v)), Some((-1.0, 0.0)));
        assert_eq!(shape(&studio, arrow).points[1], left);
    }

    #[test]
    fn an_end_let_go_near_a_boxs_middle_is_aimed_at_it_on_its_outline_and_follows_it() {
        let mut studio = two_boxes();
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (420.0, 300.0), (440.0, 320.0), Mods { ctrl: true, ..Mods::default() });
        let arrow = studio.doc.layers.len() - 1;
        studio.selected.clear();
        let id = shape(&studio, 1).id;
        // On the outline, on the way from the tail to the box's middle.
        let aimed_at = |studio: &Studio| {
            let (x, y, w, h) = shape(studio, 1).frame();
            let (tail, head) = (shape(studio, arrow).points[0], shape(studio, arrow).points[1]);
            let middle = (x + w / 2.0, y + h / 2.0);
            let off_line = ((head.0 - tail.0) * (middle.1 - tail.1) - (head.1 - tail.1) * (middle.0 - tail.0)).abs() / (middle.0 - tail.0).hypot(middle.1 - tail.1);
            let on_outline = ((head.0 - x).abs().min((head.0 - x - w).abs()) < 0.5 && (y..=y + h).contains(&head.1)) || ((head.1 - y).abs().min((head.1 - y - h).abs()) < 0.5 && (x..=x + w).contains(&head.0));
            assert!(off_line < 0.5, "on the way to its middle, {off_line} off");
            assert!(on_outline, "on its outline, at {head:?}");
            assert_eq!(shape(studio, arrow).points.len(), 2, "straight");
        };
        // Let go a little off B's middle, within the middle half of it.
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (660.0, 145.0), Mods::default(), |studio| assert_eq!(studio.board.bind, Some(1)));
        assert_eq!(shape(&studio, arrow).stuck[1], Some((id, shapes::WHOLE, shapes::AIMED)));
        aimed_at(&studio);
        let (b, head) = (shape(&studio, 1).frame(), shape(&studio, arrow).points[1]);
        assert!((head.1 - b.1 - b.3).abs() < 0.5 && head.0 > b.0 + 10.0, "into its bottom, short of its left side: {head:?}");
        // Kept so in a file.
        let kept = Shape::read(&shape(&studio, arrow).write()).unwrap();
        assert_eq!(kept.stuck[1], Some((id, shapes::WHOLE, shapes::AIMED)));
        // B moved: the end goes with it, still aimed at its middle.
        let middle = studio.to_window_page((740.0, 120.0));
        studio.selected = vec![1];
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.pressed(1, 300_000);
        studio.pointer_moved(middle.0 + 40, middle.1 + 100, Mods::default());
        studio.pointer_moved(middle.0 + 80, middle.1 + 200, Mods::default());
        studio.released();
        assert!(shape(&studio, 1).frame().1 > 250.0, "B moved down");
        assert_eq!(shape(&studio, arrow).stuck[1], Some((id, shapes::WHOLE, shapes::AIMED)));
        aimed_at(&studio);
        // Inside it but out of its middle half: as before, at the dot of
        // the side nearest, its right.
        studio.selected.clear();
        let (x, y, w, h) = shape(&studio, 1).frame();
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (x + w / 2.0 + w * 0.28, y + h / 2.0), Mods::default(), |_| {});
        assert_eq!(shape(&studio, arrow).stuck[1], Some((id, 1.0, 0.0)));
        // Ctrl held: tied to nothing, where it is let go.
        let head = studio.to_window(shape(&studio, arrow).points[1]);
        drag_from(&mut studio, head, (x + w / 2.0 + 5.0, y + h / 2.0), Mods { ctrl: true, ..Mods::default() }, |_| {});
        assert_eq!(shape(&studio, arrow).stuck[1], None);
    }

    #[test]
    fn an_arrow_moved_whole_is_drawn_to_the_dots_its_ends_near_and_tied_there() {
        let mut studio = two_boxes();
        let a = shape(&studio, 0).frame();
        let b = shape(&studio, 1).frame();
        let (a_right, b_left) = ((a.0 + a.2, a.1 + a.3 / 2.0), (b.0, b.1 + b.3 / 2.0));
        // A lone arrow on empty board, under the two.
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (300.0, 400.0), (420.0, 420.0), Mods { ctrl: true, ..Mods::default() });
        let arrow = studio.doc.layers.len() - 1;
        studio.selected.clear();
        assert_eq!(shape(&studio, arrow).stuck, [None, None]);
        // Its body dragged so its tail comes a few pixels from A's right dot.
        let grab = studio.to_window((330.0, 405.0));
        let tail = shape(&studio, arrow).points[0];
        let to = (330.0 + a_right.0 + 7.0 - tail.0, 405.0 + a_right.1 - 5.0 - tail.1);
        drag_from(&mut studio, grab, to, Mods::default(), |studio| {
            assert!(matches!(studio.drag, Drag::Move { moved: true, .. }), "{:?}", studio.drag);
            assert_eq!(shape(studio, arrow).points[0], a_right, "the tail drawn onto the dot");
            assert_eq!((studio.board.bind, studio.board.ports.clone()), (Some(0), vec![(0, a_right)]), "A glows, its dot lit");
        });
        let moved = shape(&studio, arrow);
        assert_eq!(moved.stuck, [Some((shape(&studio, 0).id, 1.0, 0.0)), None], "tied at A's right dot, the head free");
        assert_eq!(moved.points[0], a_right);
        assert_eq!((studio.board.bind, studio.board.ports.len()), (None, 0), "nothing lit once let go");
        // Dragged off again, by its body: let go of A, as Excalidraw does,
        // and free on empty board.
        studio.selected.clear();
        let body = studio.to_window(moved.points[1]);
        let grab = (body.0 - 30, body.1 - 4);
        drag_from(&mut studio, grab, (500.0, 650.0), Mods::default(), |_| {});
        assert_eq!(shape(&studio, arrow).stuck, [None, None]);
        // A straight arrow as long as from A to B, both ends near their
        // dots: tied to both.
        let both = studio.doc.layers.len();
        studio.selected.clear();
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (a_right.0, 600.0), (b_left.0, 600.0), Mods { ctrl: true, ..Mods::default() });
        studio.selected.clear();
        let grab = studio.to_window(((a_right.0 + b_left.0) / 2.0 - 40.0, 600.0));
        drag_from(&mut studio, grab, ((a_right.0 + b_left.0) / 2.0 - 34.0, a_right.1 + 8.0), Mods::default(), |studio| {
            assert_eq!(studio.board.ports.len(), 2, "both dots lit: {:?} {:?}", studio.drag, shape(studio, both).points);
        });
        let joined = shape(&studio, both);
        assert_eq!(joined.stuck, [Some((shape(&studio, 0).id, 1.0, 0.0)), Some((shape(&studio, 1).id, -1.0, 0.0))]);
        assert_eq!((joined.points[0], joined.points[1]), (a_right, b_left));
        // With Ctrl held, it is not drawn to them.
        studio.selected.clear();
        let grab = studio.to_window(((a_right.0 + b_left.0) / 2.0 - 40.0, a_right.1));
        drag_from(&mut studio, grab, ((a_right.0 + b_left.0) / 2.0 - 40.0, a_right.1 + 300.0), Mods::default(), |_| {});
        assert_eq!(shape(&studio, both).stuck, [None, None]);
        let grab = studio.to_window(((a_right.0 + b_left.0) / 2.0 - 40.0, a_right.1 + 300.0));
        drag_from(&mut studio, grab, ((a_right.0 + b_left.0) / 2.0 - 34.0, a_right.1 + 8.0), Mods { ctrl: true, ..Mods::default() }, |studio| assert!(studio.board.ports.is_empty()));
        assert_eq!(shape(&studio, both).stuck, [None, None]);
    }

    #[test]
    fn an_arrow_pulled_from_a_dot_to_near_a_box_goes_into_the_dot_nearest_where_it_is_let_go() {
        let mut studio = two_boxes();
        let b = shape(&studio, 1).frame();
        let bottom = (b.0 + b.2 / 2.0, b.1 + b.3);
        pull_dot(&mut studio, 0, (bottom.0 + 10.0, bottom.1 + 12.0), Mods::default(), |studio| {
            assert_eq!(studio.board.ports, vec![(1, bottom)], "its bottom dot lit");
        });
        let arrow = shape(&studio, 2);
        assert_eq!(arrow.stuck[1], Some((shape(&studio, 1).id, 0.0, 1.0)));
        assert_eq!(arrow.points[1], bottom, "into the bottom, touching it");
        assert_eq!(arrow.stuck[0].map(|s| s.1), Some(shapes::WHOLE), "out of A from its side facing B");
    }

    /// Box A at the left and a picture 300 by 150 at the right, its middle
    /// at (700, 136), with the pointer in hand and nothing picked out.
    fn box_and_picture() -> Studio {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let image = Arc::new(Image::new(300, 150, vec![0xFF3E_63DD; 300 * 150]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, "Screenshot".into(), (700.0, 136.0))));
        studio.selected.clear();
        studio.changed();
        studio
    }

    fn picture(studio: &Studio, index: usize) -> Picture {
        match &studio.doc.layers[index].kind {
            Kind::Picture(picture) => picture.clone(),
            Kind::Shape(_) => panic!("a picture"),
        }
    }

    /// An arrow's end at a point of the board taken hold of and let go at
    /// another, what is lit looked at before it is let go.
    fn drag_end(studio: &mut Studio, end: (f32, f32), to: (f32, f32), looked: impl FnOnce(&mut Studio)) {
        let (a, b) = (studio.to_window(end), studio.to_window(to));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((a.0 + b.0) / 2, (a.1 + b.1) / 2, Mods::default());
        studio.pointer_moved(b.0, b.1, Mods::default());
        looked(studio);
        studio.released();
    }

    #[test]
    fn a_picture_shows_dots_on_its_sides_as_a_box_does_and_an_arrow_is_pulled_out_of_one() {
        let mut studio = box_and_picture();
        let over = studio.to_window_page((700.0, 136.0));
        studio.pointer_moved(over.0, over.1, Mods::default());
        assert_eq!(studio.dots_now(over, false), Some(1), "its dots show");
        let spots = studio.dot_spots(1);
        assert_eq!(spots, [studio.to_window((700.0, 61.0)), studio.to_window((850.0, 136.0)), studio.to_window((700.0, 211.0)), studio.to_window((550.0, 136.0))], "on its outline");
        assert_eq!(shown(&mut studio, (550.0, 136.0)), 0xFFFFFF, "a dot drawn there, its middle white");
        // A press a few pixels into the picture by its dot moves it, rather
        // than pulling an arrow out.
        studio.pointer_moved(spots[3].0 + 6, spots[3].1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Move);
        studio.pointer_moved(spots[3].0 - 2, spots[3].1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Crosshair, "on its dot, an arrow");
        // Pulled out onto A: the two joined.
        pull_dot(&mut studio, 1, (180.0, 136.0), Mods::default(), |studio| assert_eq!(studio.board.bind, Some(0), "A glows"));
        assert_eq!(studio.doc.layers.len(), 3);
        let arrow = shape(&studio, 2);
        assert_eq!((arrow.stuck[0].map(|s| s.0), arrow.stuck[1].map(|s| s.0)), (Some(picture(&studio, 1).id), Some(shape(&studio, 0).id)));
        // Pulled out onto empty board: a new box there, joined.
        pull_dot(&mut studio, 1, (1100.0, 500.0), Mods::default(), |_| {});
        studio.commit_text();
        assert_eq!(studio.doc.layers.len(), 5);
        assert!(studio.is_box(3), "a box made");
        assert_eq!(shape(&studio, 4).stuck[0].map(|s| s.0), Some(picture(&studio, 1).id));
    }

    #[test]
    fn an_arrow_pulled_from_a_box_to_a_picture_goes_into_its_dot_and_follows_it_as_it_moves_is_sized_and_turned() {
        let mut studio = box_and_picture();
        let left = (550.0, 136.0);
        pull_dot(&mut studio, 0, (540.0, 140.0), Mods::default(), |studio| {
            assert_eq!(studio.board.bind, Some(1), "the picture glows");
            assert_eq!(studio.board.ports, vec![(1, left)], "its left dot lit");
        });
        assert_eq!(studio.doc.layers.len(), 3, "no box made");
        let id = picture(&studio, 1).id;
        let arrow = shape(&studio, 2);
        assert_eq!(arrow.stuck[1], Some((id, -1.0, 0.0)));
        assert_eq!(arrow.points[1], left, "touching it");
        // Kept in the file, tied to it there.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        let layers = &back[0].loose.layers;
        assert!(matches!(&layers[1].kind, Kind::Picture(kept) if kept.id == id));
        assert!(matches!(&layers[2].kind, Kind::Shape(kept) if kept.stuck[1] == Some((id, -1.0, 0.0))));
        // Moved: the end goes with it.
        studio.selected.clear();
        pull(&mut studio, (700.0, 136.0), (700.0, 336.0), Mods::default());
        // As far as it went, or a pixel or two over onto A's line.
        let cy = picture(&studio, 1).cy;
        assert!((cy - 336.0).abs() <= 4.0, "{cy}");
        assert_eq!(shape(&studio, 2).points[1], (550.0, cy), "still at its left dot");
        // Sized, and turned, still at the middle of its left side.
        if let Kind::Picture(picture) = &mut studio.doc.layers[1].kind {
            (picture.w, picture.h) = (200.0, 100.0);
        }
        studio.follow();
        assert_eq!(shape(&studio, 2).points[1], (600.0, cy));
        if let Kind::Picture(picture) = &mut studio.doc.layers[1].kind {
            picture.angle = std::f32::consts::FRAC_PI_2;
        }
        studio.follow();
        let end = shape(&studio, 2).points[1];
        assert!((end.0 - 700.0).abs() < 0.01 && (end.1 - (cy - 100.0)).abs() < 0.01, "turned with it: {end:?}");
        // Its dots turn with it too: its top now at its right.
        assert_eq!(studio.dot_spots(1)[0], studio.to_window((750.0, cy)));
    }

    #[test]
    fn tab_on_a_picture_adds_a_box_joined_to_its_right_and_mermaid_calls_the_picture_by_its_name() {
        let mut studio = box_and_picture();
        studio.selected = vec![1];
        press_key(&mut studio, Key::Tab, Mods::default());
        assert_eq!(studio.doc.layers.len(), 4, "a box and its arrow");
        assert!(studio.is_box(2) && studio.editing == Some(2), "a box, typed into");
        let (x, y, w, h) = studio.box_frame(2);
        assert!(x > 850.0 && (y + h / 2.0 - 136.0).abs() < 1.0 && (w, h) == BOX, "to its right, level with it: {:?}", (x, y, w, h));
        let id = picture(&studio, 1).id;
        assert_eq!(shape(&studio, 3).stuck.map(|stuck| stuck.map(|s| s.0)), [Some(id), Some(shape(&studio, 2).id)]);
        type_in(&mut studio, "Notes");
        studio.commit_text();
        studio.selected.clear();
        let mermaid = studio.to_mermaid(false);
        assert!(mermaid.contains("[\"Screenshot\"]") && mermaid.contains("[\"Notes\"]") && mermaid.contains("-->"), "{mermaid}");
    }

    #[test]
    fn an_end_taken_from_a_box_to_a_picture_and_back_is_tied_to_each_and_a_picture_deleted_leaves_it_free() {
        let mut studio = two_boxes();
        pull_dot(&mut studio, 0, (680.0, 136.0), Mods::default(), |_| {});
        studio.commit_text();
        let image = Arc::new(Image::new(200, 100, vec![0xFF3E_63DD; 200 * 100]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, "Screenshot".into(), (500.0, 500.0))));
        let id = picture(&studio, 3).id;
        studio.selected.clear();
        // The arrow's head taken off B and let go by the picture's top.
        let head = shape(&studio, 2).points[1];
        drag_end(&mut studio, head, (505.0, 440.0), |studio| assert_eq!(studio.board.bind, Some(3), "the picture glows"));
        let arrow = shape(&studio, 2);
        assert_eq!(arrow.stuck[1], Some((id, 0.0, -1.0)), "at its top dot");
        assert_eq!(arrow.points[1], (500.0, 450.0));
        // And back onto B.
        drag_end(&mut studio, arrow.points[1], (600.0, 136.0), |_| {});
        assert_eq!(shape(&studio, 2).stuck[1].map(|s| s.0), Some(shape(&studio, 1).id), "tied to B again");
        // Onto the picture again, then the picture deleted: the arrow stays,
        // drawn where it was, its head tied to nothing.
        let head = shape(&studio, 2).points[1];
        drag_end(&mut studio, head, (505.0, 440.0), |_| {});
        let before = shape(&studio, 2);
        assert_eq!(before.stuck[1].map(|s| s.0), Some(id));
        studio.selected = vec![3];
        press_key(&mut studio, Key::Delete, Mods::default());
        assert_eq!(studio.doc.layers.len(), 3, "only the picture gone");
        let left = shape(&studio, 2);
        assert_eq!((left.stuck[1], left.points.clone()), (None, before.points), "free, where it was");
        studio.undo();
        assert_eq!(shape(&studio, 2).stuck[1].map(|s| s.0), Some(id), "undo ties it again");
    }

    /// A box drawn on a board from one corner to another, the pointer in
    /// hand again after it.
    fn draw_box(studio: &mut Studio, from: (f32, f32), to: (f32, f32)) -> usize {
        press_key(studio, Key::Char('r'), Mods::default());
        pull(studio, from, to, Mods::default());
        press_key(studio, Key::Escape, Mods::default());
        press_key(studio, Key::Escape, Mods::default());
        studio.doc.layers.len() - 1
    }

    /// The arrows picked out made to go round corners, from the panel.
    fn elbowed(studio: &mut Studio, arrows: Vec<usize>) {
        studio.selected = arrows;
        studio.layout();
        let at = studio.control_rect(Control::Board(Ctl::Route(2))).expect("offered").center();
        click(studio, at);
        studio.selected.clear();
    }

    /// The points an arrow goes by, from its first end to its last.
    fn stops(arrow: &Shape) -> Vec<(f32, f32)> {
        let mut stops = vec![arrow.points[0]];
        stops.extend_from_slice(&arrow.points[2..]);
        stops.push(arrow.points[1]);
        stops
    }

    /// Whether any stretch of a way goes inside a box.
    fn goes_into(stops: &[(f32, f32)], (x, y, w, h): (f32, f32, f32, f32)) -> bool {
        stops.windows(2).any(|pair| {
            let (a, b) = (pair[0], pair[1]);
            a.0.max(b.0) > x && a.0.min(b.0) < x + w && a.1.max(b.1) > y && a.1.min(b.1) < y + h
        })
    }

    /// Whether a point is on the edge of a box.
    fn on_edge((px, py): (f32, f32), (x, y, w, h): (f32, f32, f32, f32)) -> bool {
        let inside = px >= x - 0.5 && px <= x + w + 0.5 && py >= y - 0.5 && py <= y + h + 0.5;
        inside && ((px - x).abs() < 0.5 || (px - x - w).abs() < 0.5 || (py - y).abs() < 0.5 || (py - y - h).abs() < 0.5)
    }

    /// An elbow from one box to another, as a diagram's connector goes:
    /// each stretch level or upright, each turn square, from an edge of
    /// the one to an edge of the other, round every other box.
    fn assert_routed(studio: &Studio, arrow: usize, from: usize, to: usize) -> Vec<(f32, f32)> {
        let shape = shape(studio, arrow);
        let stops = stops(&shape);
        assert!(shape.elbow, "an elbow");
        for (index, pair) in stops.windows(2).enumerate() {
            let (level, upright) = ((pair[0].1 - pair[1].1).abs() < 0.01, (pair[0].0 - pair[1].0).abs() < 0.01);
            assert!(level != upright, "stretch {index} level or upright, and some length: {stops:?}");
        }
        for (index, three) in stops.windows(3).enumerate() {
            let (a, b) = ((three[1].0 - three[0].0, three[1].1 - three[0].1), (three[2].0 - three[1].0, three[2].1 - three[1].1));
            assert!((a.0 * b.0 + a.1 * b.1).abs() < 0.01, "turn {index} square: {stops:?}");
        }
        assert!(on_edge(stops[0], studio.box_frame(from)) && on_edge(stops[stops.len() - 1], studio.box_frame(to)), "from edge to edge: {stops:?}");
        for index in (0..studio.doc.layers.len()).filter(|&index| studio.is_node(index)) {
            assert!(!goes_into(&stops, studio.box_frame(index)), "clear of box {index} {:?}: {stops:?}", studio.box_frame(index));
        }
        stops
    }

    /// CUJ: two boxes with a taller one between them, an arrow pulled from
    /// the first into the last and made to go round corners: it goes
    /// round the one between, turning only square; and goes round it again
    /// as the boxes move.
    #[test]
    fn an_elbow_between_two_boxes_goes_round_one_between_them_and_again_as_they_move() {
        let mut studio = two_boxes();
        draw_box(&mut studio, (380.0, 60.0), (480.0, 212.0));
        pull_dot(&mut studio, 0, (680.0, 136.0), Mods::default(), |_| {});
        studio.commit_text();
        let arrow = studio.doc.layers.len() - 1;
        elbowed(&mut studio, vec![arrow]);
        let first = assert_routed(&studio, arrow, 0, 1);
        assert!(first.len() >= 4, "round the box between, not through it: {first:?}");
        // B taken well below: a way of its own to B where it is now.
        pull(&mut studio, (700.0, 110.0), (700.0, 410.0), Mods::default());
        let lower = assert_routed(&studio, arrow, 0, 1);
        assert_ne!(lower, first, "routed again");
        // A box drawn in that way, and A moved a little: round that too.
        studio.selected.clear();
        let (x, y) = lower[1];
        draw_box(&mut studio, (x - 50.0, y - 120.0), (x + 50.0, y - 60.0));
        studio.selected.clear();
        pull(&mut studio, (200.0, 150.0), (200.0, 160.0), Mods::default());
        let round = assert_routed(&studio, arrow, 0, 1);
        assert_ne!(round[1..round.len() - 1], lower[1..lower.len() - 1], "out of the way of the box drawn: {round:?}");
    }
}
