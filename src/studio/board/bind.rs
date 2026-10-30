//! Arrows tied to what they are let go near, as Excalidraw binds them: an
//! end dragged over a box, or within a finger's width of one, is tied to
//! it, the box glowing and the end drawn at its edge while it is dragged;
//! the nearest wins, and the box inside another before the one round it.
//! Ctrl held ties it to nothing, to leave the end where it is, as in
//! Excalidraw; Alt is a copy on a board, as everywhere.

use super::*;

/// How near a shape an arrow's end is tied to it, in pixels of the window.
pub(in crate::studio) const REACH: f32 = 20.0;

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
        let anchors: Vec<(usize, shapes::Anchor)> = self.anchors().into_iter().filter(|anchor| !skip.contains(&anchor.id)).filter_map(|anchor| Some((self.index_of(anchor.id)?, anchor))).filter(|&(index, _)| !boxes || self.is_box(index)).collect();
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
    /// box, an oval or words as a whole, to go from side to side as it
    /// moves; a picture at the point, kept to its edge, to point at what
    /// is in it; or to nothing. Not to the box the other end is tied to,
    /// as an arrow from a box back into it is no arrow. Gives what it is
    /// tied to.
    pub(in crate::studio) fn bind_end(&mut self, index: usize, end: usize, at: (f32, f32)) -> Option<usize> {
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return None };
        if !shape.sticks() {
            return None;
        }
        let skip: Vec<u32> = shape.stuck[1 - end].filter(|&(_, u, _)| u == shapes::WHOLE).map(|(id, ..)| id).into_iter().collect();
        let target = self.bind_target(at, &skip, false);
        let anchor = target.and_then(|target| self.doc.layers[target].kind.clone().anchor_of(&mut self.text));
        let whole = target.is_some_and(|target| !matches!(self.doc.layers[target].kind, Kind::Picture(_)));
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) else { return None };
        shape.stuck[end] = anchor.map(|anchor| match whole {
            true => (anchor.id, shapes::WHOLE, shapes::WHOLE),
            false => {
                let (lx, ly) = shapes::turn_point(at, anchor.middle, -anchor.angle);
                let (hw, hh) = (anchor.half.0.max(1.0), anchor.half.1.max(1.0));
                (anchor.id, ((lx - anchor.middle.0) / hw).clamp(-1.0, 1.0), ((ly - anchor.middle.1) / hh).clamp(-1.0, 1.0))
            }
        });
        shape.routed = true;
        // A bend is the route's between two boxes, and goes with it.
        let routed = matches!((shape.stuck[0], shape.stuck[1]), (Some((_, u, _)), Some((_, v, _))) if u == shapes::WHOLE && v == shapes::WHOLE);
        if !routed && shape.points.len() > 2 {
            shape.points.truncate(2);
            shape.touch();
        }
        target
    }

    /// An end of a line being dragged on a board: tied as it would be let
    /// go, there and then, so that it is drawn at the edge of what it
    /// would be tied to, and that glows.
    pub(in crate::studio) fn bind_live(&mut self, index: usize, end: usize, at: (f32, f32)) {
        let target = self.bind_end(index, end, at);
        self.follow();
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.touch();
        }
        self.changed_layers(&[index]);
        if target != self.board.bind {
            self.board.bind = target;
            self.dirty = true;
        }
    }

    /// What an end dragged would be tied to, glowing round its edge.
    pub(in crate::studio) fn draw_bind(&mut self, canvas: &mut Canvas) {
        let Some(target) = self.board.bind.filter(|&target| target < self.doc.layers.len()) else { return };
        if !matches!(self.drag, Drag::Draw { .. } | Drag::Scale { .. } | Drag::Pull { .. }) {
            return;
        }
        let (x, y, w, h) = self.doc.layers[target].bounds(&mut self.text);
        let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
        canvas.round_outline(a.0 - 7, a.1 - 7, b.0 - a.0 + 14, b.1 - a.1 + 14, 9, 3, canvas::mix(ACCENT, 0xFFFFFF, 170));
        canvas.round_outline(a.0 - 4, a.1 - 4, b.0 - a.0 + 8, b.1 - a.1 + 8, 6, 2, ACCENT);
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
        assert!(live.points[1].0 < 600.0 && live.points[1].0 > 580.0, "drawn to B's edge, {:?}", live.points[1]);
        studio.released();
        let arrow = shape(&studio, 2);
        assert_eq!((arrow.stuck[0].map(|s| s.0), arrow.stuck[1].map(|s| s.0)), (Some(shape(&studio, 0).id), Some(shape(&studio, 1).id)));
        // Onto empty board: the arrow ends there, and no box is made.
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (300.0, 400.0), (500.0, 500.0), Mods::default());
        assert_eq!(studio.doc.layers.len(), 4);
        assert_eq!(shape(&studio, 3).stuck, [None, None]);
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
}
