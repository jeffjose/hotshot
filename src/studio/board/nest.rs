//! A board is fluid, not rigid: drop a box in a box and it is inside it,
//! the outer one growing round it with its words moved up to its top left
//! as a title, and shrinking back as it is dragged out; type more into a
//! box and it grows, what is to its right and under it making room and
//! keeping the gaps they had. Nesting is an auto layout stack in a loose
//! form, whose members stay where they are put and whose box only grows
//! round them. Alt held in a drag stops all of it for that drag.

use super::*;

/// The room a box keeps round what is in it.
const PAD: f32 = 20.0;
/// How wide a box grows for its words before they wrap.
pub(in crate::studio) const MOST_WIDE: f32 = 360.0;

/// A box's place, by what it is called: left, top, width and height.
pub(in crate::studio) type Frames = Vec<(u32, (f32, f32, f32, f32))>;

impl Studio {
    /// The box something is in on a board, by what each is called.
    pub(in crate::studio) fn holder_of(&self, id: u32) -> Option<u32> {
        self.doc.stacks.iter().find(|stack| stack.holder.is_some() && stack.members.contains(&id)).and_then(|stack| stack.holder)
    }

    /// Everything in a box, however deep, by what each is called.
    pub(in crate::studio) fn inside(&self, id: u32) -> Vec<u32> {
        let mut found = Vec::new();
        if let Some(stack) = self.doc.stacks.iter().find(|stack| stack.holder == Some(id)) {
            for &member in &stack.members {
                if !found.contains(&member) && member != id {
                    found.push(member);
                    found.extend(self.inside(member).into_iter().filter(|inner| *inner != id));
                }
            }
        }
        found
    }

    /// How many boxes something is in.
    fn nest_depth(&self, id: u32) -> usize {
        let mut depth = 0;
        let mut at = id;
        while let Some(outer) = self.holder_of(at) {
            depth += 1;
            at = outer;
            if depth > 64 {
                break;
            }
        }
        depth
    }

    /// The layer something is, by what it is called.
    pub(in crate::studio) fn index_of(&self, id: u32) -> Option<usize> {
        self.doc.layers.iter().position(|layer| stacks::layer_id(layer) == id)
    }

    /// Where each box is.
    pub(in crate::studio) fn box_frames(&self) -> Frames {
        (0..self.doc.layers.len()).filter(|&index| self.is_box(index)).map(|index| (self.id_of(index), self.box_frame(index))).collect()
    }

    /// A move on a board begun: what is in what is moved goes with it,
    /// and so do the arrows between them; copied, with Alt, all of it is.
    pub(in crate::studio) fn begin_carry(&mut self, copy: bool) {
        let chosen = self.chosen();
        let ids: Vec<u32> = chosen.iter().map(|&index| self.id_of(index)).collect();
        let mut inner: Vec<u32> = Vec::new();
        for &id in &ids {
            for member in self.inside(id) {
                if !ids.contains(&member) && !inner.contains(&member) {
                    inner.push(member);
                }
            }
        }
        let all: Vec<u32> = ids.iter().chain(&inner).copied().collect();
        for layer in &self.doc.layers {
            if let Kind::Shape(shape) = &layer.kind {
                let ends: Vec<u32> = shape.stuck.iter().flatten().map(|(id, ..)| *id).collect();
                if shape.sticks() && ends.len() == 2 && ends.iter().all(|id| all.contains(id)) && !all.contains(&shape.id) && !inner.is_empty() {
                    inner.push(shape.id);
                }
            }
        }
        let indices: Vec<usize> = inner.iter().filter_map(|&id| self.index_of(id)).collect();
        match copy {
            true => {
                self.selected.extend(indices);
                self.board.carried.clear();
            }
            false => self.board.carried = indices.into_iter().map(|index| (index, self.doc.layers[index].clone())).collect(),
        }
    }

    /// What is carried along with a move, moved as far; with Alt held,
    /// left where it was.
    pub(in crate::studio) fn carry(&mut self, dx: f32, dy: f32) {
        let by = if self.mods.alt { (0.0, 0.0) } else { (dx, dy) };
        for (index, layer) in self.board.carried.clone() {
            if index < self.doc.layers.len() {
                let mut layer = layer;
                layer.translate(by.0, by.1);
                self.doc.layers[index] = layer;
            }
        }
    }

    /// What was moved on a board let go: each box into the smallest box
    /// it is let go in, or out of the one it was in; the boxes holding them
    /// grown or shrunk round them, and what is beside those making room.
    /// Nothing of it with Alt held.
    pub(in crate::studio) fn nest_moved(&mut self) {
        self.board.carried.clear();
        self.board.letting_go = false;
        if self.mods.alt {
            return;
        }
        let chosen = self.chosen();
        let ids: Vec<u32> = chosen.iter().map(|&index| self.id_of(index)).collect();
        let tops: Vec<usize> = chosen.into_iter().filter(|&index| self.is_box(index) && self.holder_of(self.id_of(index)).is_none_or(|outer| !ids.contains(&outer))).collect();
        let before = self.box_frames();
        let mut changed = false;
        let single = tops.len() == 1;
        for index in tops {
            // One box let go on an arrow goes between its ends.
            if single && self.split_arrow(index) {
                changed = true;
                continue;
            }
            let id = self.id_of(index);
            let (x, y, w, h) = self.box_frame(index);
            let middle = (x + w / 2.0, y + h / 2.0);
            let mut mine = self.inside(id);
            mine.push(id);
            let target = (0..self.doc.layers.len())
                .filter(|&other| self.is_box(other) && !self.doc.layers[other].hidden && !mine.contains(&self.id_of(other)))
                .filter(|&other| {
                    let (ox, oy, ow, oh) = self.box_frame(other);
                    middle.0 >= ox && middle.0 <= ox + ow && middle.1 >= oy && middle.1 <= oy + oh && ow * oh > w * h
                })
                .min_by(|&a, &b| {
                    let area = |index: usize| self.box_frame(index).2 * self.box_frame(index).3;
                    area(a).total_cmp(&area(b))
                })
                .map(|other| self.id_of(other));
            if target != self.holder_of(id) {
                self.unnest(id);
                if let Some(holder) = target {
                    self.nest(id, holder);
                }
                changed = true;
            }
        }
        if changed {
            self.changed();
            self.make_room_after(&before);
        }
    }

    /// Something put in a box, as the last of what it holds.
    pub(in crate::studio) fn nest(&mut self, id: u32, holder: u32) {
        let Some(at) = self.index_of(holder) else { return };
        let frame = self.box_frame(at);
        if let Kind::Shape(shape) = &mut self.doc.layers[at].kind {
            if shape.base == (0.0, 0.0) {
                shape.base = (frame.2, frame.3);
            }
        }
        match self.doc.stacks.iter_mut().find(|stack| stack.holder == Some(holder)) {
            Some(stack) => stack.members.push(id),
            None => self.doc.stacks.push(stacks::Stack {
                id: shapes::fresh_id(),
                members: vec![id],
                way: stacks::COLUMN,
                gap: 0.0,
                pad: PAD,
                align: 0,
                sizing: stacks::HUG,
                x: frame.0,
                y: frame.1,
                w: frame.2,
                h: frame.3,
                fill: None,
                radius: 0.0,
                seen: Vec::new(),
                name: None,
                holder: Some(holder),
            }),
        }
    }

    /// Something taken out of the box it is in.
    pub(in crate::studio) fn unnest(&mut self, id: u32) {
        for stack in self.doc.stacks.iter_mut().filter(|stack| stack.holder.is_some()) {
            stack.members.retain(|&member| member != id);
        }
        self.doc.stacks.retain(|stack| stack.holder.is_none() || !stack.members.is_empty());
    }

    /// Each box holding others grown round them, the innermost first, its
    /// words a title at its top left; and a box that holds nothing now its
    /// words back in its middle and its size back to what it was drawn.
    pub(in crate::studio) fn settle_nests(&mut self) {
        let ids: Vec<u32> = self.doc.layers.iter().map(stacks::layer_id).collect();
        for stack in self.doc.stacks.iter_mut().filter(|stack| stack.holder.is_some()) {
            stack.members.retain(|member| ids.contains(member));
        }
        self.doc.stacks.retain(|stack| stack.holder.is_none_or(|holder| ids.contains(&holder) && !stack.members.is_empty()));
        let mut held: Vec<u32> = self.doc.stacks.iter().filter_map(|stack| stack.holder).collect();
        held.sort_by_key(|&holder| std::cmp::Reverse(self.nest_depth(holder)));
        let dragged: Vec<u32> = match self.drag {
            _ if self.board.letting_go => self.chosen().iter().map(|&index| self.id_of(index)).collect(),
            Drag::Move { moved: true, .. } => self.chosen().iter().map(|&index| self.id_of(index)).collect(),
            _ => Vec::new(),
        };
        for holder in &held {
            // A box that lays out what is in it does, before it grows.
            self.arrange_region(*holder, &dragged);
            self.grow_round(*holder);
        }
        for index in 0..self.doc.layers.len() {
            let id = self.id_of(index);
            if held.contains(&id) {
                continue;
            }
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                if shape.titled && !shape.heading && matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                    shape.titled = false;
                    let (x, y, _, _) = shape.frame();
                    if shape.base.0 > 0.0 && shape.base.1 > 0.0 {
                        shape.points = vec![(x, y), (x + shape.base.0, y + shape.base.1)];
                    }
                    shape.touch();
                }
            }
        }
    }

    /// A box grown round what is in it, keeping room round them and over
    /// them for its title; no smaller than it was drawn.
    fn grow_round(&mut self, holder: u32) {
        let Some(at) = self.index_of(holder) else { return };
        let Some(stack) = self.doc.stacks.iter().find(|stack| stack.holder == Some(holder)).cloned() else { return };
        // What is being dragged is not grown round until it is let go,
        // in or out.
        let dragged: Vec<u32> = match self.drag {
            _ if self.board.letting_go => self.chosen().iter().map(|&index| self.id_of(index)).collect(),
            Drag::Move { moved: true, .. } => self.chosen().iter().map(|&index| self.id_of(index)).collect(),
            _ => Vec::new(),
        };
        let mut boxes = Vec::new();
        for &member in stack.members.iter().filter(|member| !dragged.contains(member)) {
            if let Some(index) = self.index_of(member) {
                // A box by its own edges, not the room its wobble takes.
                boxes.push(if self.is_box(index) { self.box_frame(index) } else { self.doc.layers[index].bounds(&mut self.text) });
            }
        }
        let Some((cx, cy, cw, ch)) = actions::union(&boxes) else { return };
        let Kind::Shape(shape) = &mut self.doc.layers[at].kind else { return };
        let (x, y, w, h) = shape.frame();
        if shape.base == (0.0, 0.0) {
            shape.base = (w, h);
        }
        let title = Text::line_height(shape.font_px) as f32 + 10.0;
        let need = (cx - PAD, cy - PAD - title, cx + cw + PAD, cy + ch + PAD);
        let own = (x, y, x + shape.base.0, y + shape.base.1);
        let grown = (own.0.min(need.0), own.1.min(need.1), own.2.max(need.2), own.3.max(need.3));
        let points = vec![(grown.0, grown.1), (grown.2, grown.3)];
        if shape.points[..2] != points[..] || !shape.titled {
            shape.points = points;
            shape.titled = true;
            shape.touch();
        }
        if let Some(stack) = self.doc.stacks.iter_mut().find(|stack| stack.holder == Some(holder)) {
            (stack.x, stack.y, stack.w, stack.h) = (grown.0, grown.1, grown.2 - grown.0, grown.3 - grown.1);
        }
    }

    /// The boxes that grew or shrank since they were where `before` says
    /// make room: what is to the right of each, level with it, moves as far
    /// as its right side did, and what is under it as far as its bottom,
    /// keeping the gaps they had; the innermost first.
    pub(in crate::studio) fn make_room_after(&mut self, before: &Frames) {
        let mut grown: Vec<(u32, (f32, f32, f32, f32), (f32, f32, f32, f32))> = Vec::new();
        for &(id, was) in before {
            let Some(index) = self.index_of(id) else { continue };
            let now = self.box_frame(index);
            if (now.2 - was.2).abs() > 0.5 || (now.3 - was.3).abs() > 0.5 {
                grown.push((id, was, now));
            }
        }
        grown.sort_by_key(|&(id, ..)| std::cmp::Reverse(self.nest_depth(id)));
        let mut moved = false;
        for (id, was, now) in grown {
            moved |= self.make_room(id, was, now);
        }
        if moved {
            self.changed();
        }
    }

    /// Room made for a box that was one size and is now another: its
    /// neighbors at the same level, beside it or under it, moved by as much
    /// as it grew that way, with what is in them.
    fn make_room(&mut self, id: u32, was: (f32, f32, f32, f32), now: (f32, f32, f32, f32)) -> bool {
        let (dw, dh) = ((now.0 + now.2) - (was.0 + was.2), (now.1 + now.3) - (was.1 + was.3));
        if dw.abs() < 0.5 && dh.abs() < 0.5 {
            return false;
        }
        let level = self.holder_of(id);
        let mut family = self.inside(id);
        family.push(id);
        let mut at = id;
        while let Some(outer) = self.holder_of(at) {
            family.push(outer);
            at = outer;
            if family.len() > 256 {
                break;
            }
        }
        let mut shifts: Vec<(u32, (f32, f32))> = Vec::new();
        for index in 0..self.doc.layers.len() {
            let other = self.id_of(index);
            if !self.is_box(index) || family.contains(&other) || self.holder_of(other) != level {
                continue;
            }
            let (x, y, w, h) = self.box_frame(index);
            let beside = x >= was.0 + was.2 - 1.0 && y < was.1 + was.3 && y + h > was.1;
            let under = y >= was.1 + was.3 - 1.0 && x < was.0 + was.2 && x + w > was.0;
            let by = (if beside { dw } else { 0.0 }, if under { dh } else { 0.0 });
            if by != (0.0, 0.0) {
                shifts.push((other, by));
            }
        }
        for &(other, by) in &shifts {
            let mut all = self.inside(other);
            all.push(other);
            for member in all {
                if let Some(index) = self.index_of(member) {
                    self.doc.layers[index].translate(by.0, by.1);
                }
            }
        }
        !shifts.is_empty()
    }

    /// A box's words changed on a board: it grown to hold them, as wide as
    /// they are up to a limit and then as tall as they wrap to, never
    /// smaller than it was drawn; what is beside it making room.
    pub(in crate::studio) fn grow_for_words(&mut self, index: usize) {
        if !self.is_box(index) {
            return;
        }
        let before = self.box_frames();
        let Kind::Shape(shape) = &self.doc.layers[index].kind else { return };
        let (x, y, w, h) = shape.frame();
        let base = if shape.base.0 > 0.0 && shape.base.1 > 0.0 { shape.base } else { (w, h) };
        let (nw, nh) = shape.label_needs(&mut self.text, base, MOST_WIDE.max(base.0));
        if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
            shape.base = base;
            if (nw - w).abs() > 0.5 || (nh - h).abs() > 0.5 {
                shape.points = vec![(x, y), (x + nw, y + nh)];
                shape.touch();
            }
        }
        self.changed();
        self.make_room_after(&before);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    /// A box drawn from one corner to another, with words, the pointer
    /// back in hand.
    pub(in crate::studio) fn draw_box(studio: &mut Studio, from: (f32, f32), to: (f32, f32), words: &str) -> usize {
        press_key(studio, Key::Char('r'), Mods::default());
        pull(studio, from, to, Mods::default());
        type_in(studio, words);
        press_key(studio, Key::Escape, Mods::default());
        press_key(studio, Key::Char('v'), Mods::default());
        studio.selected.clear();
        studio.doc.layers.iter().rposition(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Rect)).unwrap()
    }

    #[test]
    fn a_box_dropped_in_a_box_is_inside_it_its_title_moves_up_and_it_shrinks_back() {
        let mut studio = board();
        let backend = draw_box(&mut studio, (100.0, 100.0), (280.0, 200.0), "Backend");
        let auth = draw_box(&mut studio, (400.0, 120.0), (500.0, 166.0), "Auth");
        let was = shape(&studio, backend).frame();
        // Dropped in: inside it, the box grown round it, its words a title.
        pull(&mut studio, (450.0, 143.0), (220.0, 170.0), Mods::default());
        assert_eq!(studio.holder_of(shape(&studio, auth).id), Some(shape(&studio, backend).id));
        let grown = shape(&studio, backend);
        let inner = shape(&studio, auth).frame();
        assert!(grown.titled && grown.frame().0 + grown.frame().2 >= inner.0 + inner.2 + 19.0, "grown round {inner:?}: {:?}", grown.frame());
        assert!(grown.frame().3 > was.3);
        // The outer one moved, the inner goes with it, and stays inside.
        let (before, outer_before) = (shape(&studio, auth).frame(), shape(&studio, backend).frame());
        // By its bottom: the bar of styles floats over its top.
        pull(&mut studio, (120.0, 208.0), (220.0, 508.0), Mods::default());
        let (after, outer_after) = (shape(&studio, auth).frame(), shape(&studio, backend).frame());
        assert!(after.1 - before.1 > 250.0, "moved down with it");
        assert!(((after.0 - before.0) - (outer_after.0 - outer_before.0)).abs() < 0.01 && ((after.1 - before.1) - (outer_after.1 - outer_before.1)).abs() < 0.01);
        assert_eq!(studio.holder_of(shape(&studio, auth).id), Some(shape(&studio, backend).id));
        // Dragged out: back to the size it was drawn, its words in its middle.
        let at = shape(&studio, auth).frame();
        pull(&mut studio, (at.0 + 50.0, at.1 + 23.0), (900.0, 700.0), Mods::default());
        assert_eq!(studio.holder_of(shape(&studio, auth).id), None);
        let back = shape(&studio, backend);
        assert!(!back.titled && (back.frame().2 - was.2).abs() < 1.0 && (back.frame().3 - was.3).abs() < 1.0);
        // Kept in the file.
        pull(&mut studio, (900.0, 700.0), (230.0, 470.0), Mods::default());
        let pages = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert!(pages[0].loose.stacks.iter().any(|stack| stack.holder == Some(back.id)));
    }

    #[test]
    fn alt_in_a_drag_nests_nothing_and_carries_nothing() {
        let mut studio = board();
        let outer = draw_box(&mut studio, (100.0, 100.0), (400.0, 300.0), "Cloud");
        let inner = draw_box(&mut studio, (600.0, 120.0), (700.0, 166.0), "App");
        let alt = Mods { alt: true, ..Mods::default() };
        let press = studio.to_window_page((650.0, 143.0));
        studio.pointer_moved(press.0, press.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.to_window_page((250.0, 200.0));
        studio.pointer_moved(to.0 + 10, to.1, alt);
        studio.pointer_moved(to.0, to.1, alt);
        studio.released();
        assert_eq!(studio.holder_of(shape(&studio, inner).id), None, "nothing nests with Alt");
        let _ = outer;
    }

    #[test]
    fn typing_grows_a_box_and_what_is_beside_it_makes_room() {
        let mut studio = board();
        let queue = draw_box(&mut studio, (100.0, 100.0), (220.0, 150.0), "Queue");
        let worker = draw_box(&mut studio, (300.0, 100.0), (420.0, 150.0), "Worker");
        let below = draw_box(&mut studio, (100.0, 260.0), (220.0, 310.0), "Logs");
        let gap = shape(&studio, worker).frame().0 - (shape(&studio, queue).frame().0 + shape(&studio, queue).frame().2);
        let drop_gap = shape(&studio, below).frame().1 - (shape(&studio, queue).frame().1 + shape(&studio, queue).frame().3);
        tap(&mut studio, (160.0, 125.0));
        press_key(&mut studio, Key::Enter, Mods::default());
        type_in(&mut studio, ", with retries and a dead-letter box for what keeps failing");
        let grown = shape(&studio, queue).frame();
        assert!(grown.2 > 200.0 && grown.3 > 50.0, "grew to {grown:?}");
        assert!(grown.2 <= MOST_WIDE + 1.0, "as wide as it goes, then taller");
        let now_gap = shape(&studio, worker).frame().0 - (grown.0 + grown.2);
        assert!((now_gap - gap).abs() < 1.0, "Worker kept its gap: {gap} then {now_gap}");
        let now_drop = shape(&studio, below).frame().1 - (grown.1 + grown.3);
        assert!((now_drop - drop_gap).abs() < 1.0);
        // The words shrunk, it closes up again.
        press_key(&mut studio, Key::Escape, Mods::default());
        type_in(&mut studio, "Q");
        let small = shape(&studio, queue).frame();
        assert!((small.2 - 120.0).abs() < 1.0 && (small.3 - 50.0).abs() < 1.0, "back to {small:?}");
        assert!((shape(&studio, worker).frame().0 - (small.0 + small.2) - gap).abs() < 1.0);
    }

    #[test]
    fn an_arrow_follows_a_box_round_to_its_nearest_side() {
        let mut studio = board();
        let api = draw_box(&mut studio, (100.0, 100.0), (210.0, 152.0), "API");
        let db = draw_box(&mut studio, (400.0, 100.0), (520.0, 152.0), "Database");
        studio.join(api, db);
        studio.changed();
        let arrow = studio.doc.layers.len() - 1;
        assert!(shape(&studio, arrow).points[1].0 < 400.0, "into the left side of Database");
        // Database dragged under API: the arrow now goes down into its top.
        pull(&mut studio, (460.0, 126.0), (160.0, 426.0), Mods::default());
        let routed = shape(&studio, arrow);
        let top = shape(&studio, db).frame().1;
        assert!(routed.points[1].1 < top && routed.points[1].1 > top - 12.0 && routed.points[0].1 > 152.0, "{:?}", routed.points);
    }
}
