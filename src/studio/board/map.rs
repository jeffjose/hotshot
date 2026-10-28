//! Mind maps from the keyboard: with a box picked out or typed into, Tab
//! adds a child, joined and placed, and Enter, typing in a box of a map,
//! the next one under it; Shift+Tab goes back to the parent. Shift+L lays
//! a map or a tree out neatly when asked, left to right or top down, its
//! order kept and nothing else moved. An indented list typed as text and
//! Ctrl+Enter becomes a map, a box a line.

use super::*;

/// How far apart a map's levels are, and the boxes of one level.
const LEVEL: f32 = 80.0;
const SIBLING: f32 = 24.0;

impl Studio {
    /// The arrows of the board joining two boxes, as what is called what:
    /// the arrow, where it is from and where it goes.
    pub(in crate::studio) fn links(&self) -> Vec<(u32, u32, u32)> {
        let boxes: Vec<u32> = (0..self.doc.layers.len()).filter(|&index| self.is_box(index)).map(|index| self.id_of(index)).collect();
        self.doc
            .layers
            .iter()
            .filter_map(|layer| match &layer.kind {
                Kind::Shape(shape) if shape.sticks() => match shape.stuck {
                    [Some((from, ..)), Some((to, ..))] if from != to && boxes.contains(&from) && boxes.contains(&to) => Some((shape.id, from, to)),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// The box an arrow comes into something from, in a map.
    pub(in crate::studio) fn parent_box(&self, id: u32) -> Option<u32> {
        self.links().into_iter().find(|&(_, _, to)| to == id).map(|(_, from, _)| from)
    }

    /// The boxes something has arrows out to, in the order they are in:
    /// down, or across for a tree that goes down.
    pub(in crate::studio) fn child_boxes(&self, id: u32) -> Vec<u32> {
        let mut children: Vec<u32> = self.links().into_iter().filter(|&(_, from, _)| from == id).map(|(_, _, to)| to).collect();
        children.dedup();
        let down = self.goes_down(id);
        let key = |studio: &Studio, child: u32| studio.index_of(child).map_or((0.0, 0.0), |index| {
            let (x, y, w, h) = studio.box_frame(index);
            if down { (x + w / 2.0, y) } else { (y + h / 2.0, x) }
        });
        children.sort_by(|&a, &b| key(self, a).partial_cmp(&key(self, b)).unwrap_or(std::cmp::Ordering::Equal));
        children
    }

    /// Whether a map goes down from a box, rather than across: its
    /// children mostly under it.
    fn goes_down(&self, id: u32) -> bool {
        let Some(index) = self.index_of(id) else { return false };
        let (x, y, w, h) = self.box_frame(index);
        let children: Vec<(f32, f32, f32, f32)> = self.links().into_iter().filter(|&(_, from, _)| from == id).filter_map(|(_, _, to)| self.index_of(to)).map(|index| self.box_frame(index)).collect();
        if children.is_empty() {
            return self.parent_box(id).and_then(|parent| self.index_of(parent)).is_some_and(|parent| {
                let (px, py, pw, ph) = self.box_frame(parent);
                (y - (py + ph)).abs() > 0.0 && ((y + h / 2.0) - (py + ph / 2.0)).abs() > ((x + w / 2.0) - (px + pw / 2.0)).abs()
            });
        }
        let (across, down): (f32, f32) = children.iter().fold((0.0, 0.0), |(a, d), c| (a + ((c.0 + c.2 / 2.0) - (x + w / 2.0)).abs(), d + ((c.1 + c.3 / 2.0) - (y + h / 2.0)).abs()));
        down > across
    }

    /// The box picked out, or being typed into, on a board.
    fn map_box(&self) -> Option<usize> {
        self.editing.or_else(|| self.picked()).filter(|&index| self.is_box(index))
    }

    /// Tab: a child of the box picked out, after any it has, joined to it,
    /// in its look a shade lighter, and typed into.
    pub(in crate::studio) fn add_child(&mut self) -> bool {
        let Some(parent) = self.map_box() else { return false };
        self.commit_text();
        let Some(parent) = (parent < self.doc.layers.len()).then_some(parent) else { return false };
        let id = self.id_of(parent);
        let (x, y, w, h) = self.box_frame(parent);
        let size = match &self.doc.layers[parent].kind {
            Kind::Shape(shape) if shape.base.0 > 0.0 && !shape.titled => shape.base,
            _ => (w, h),
        };
        let down = self.goes_down(id);
        let center = match self.child_boxes(id).last().and_then(|&last| self.index_of(last)) {
            Some(last) => {
                let (lx, ly, lw, lh) = self.box_frame(last);
                match down {
                    true => (lx + lw + SIBLING + size.0 / 2.0, ly + lh / 2.0),
                    false => (lx + lw / 2.0, ly + lh + SIBLING + size.1 / 2.0),
                }
            }
            None => match down {
                true => (x + w / 2.0, y + h + LEVEL + size.1 / 2.0),
                false => (x + w + LEVEL + size.0 / 2.0, y + h / 2.0),
            },
        };
        self.checkpoint();
        let made = self.box_like(parent, center, 1);
        self.fit_box(made, size);
        self.join(parent, made);
        self.selected = vec![made];
        self.edit_text(made);
        self.changed();
        true
    }

    /// Enter, typing in a box of a map: the next one under it, with the
    /// same parent, typed into. Nothing for a box of no map.
    pub(in crate::studio) fn add_sibling(&mut self) -> bool {
        let Some(index) = self.map_box() else { return false };
        let id = self.id_of(index);
        let Some(parent) = self.parent_box(id).and_then(|parent| self.index_of(parent)) else { return false };
        self.commit_text();
        let parent_id = self.id_of(parent);
        let down = self.goes_down(parent_id);
        // After the last of the parent's children that comes after it.
        let (x, y, w, h) = self.box_frame(index);
        let after = self.child_boxes(parent_id).into_iter().filter_map(|child| self.index_of(child)).map(|child| self.box_frame(child)).filter(|frame| if down { frame.0 >= x && (frame.1 - y).abs() < h } else { frame.1 >= y && (frame.0 - x).abs() < w }).fold((x, y, w, h), |last, frame| if down { if frame.0 > last.0 { frame } else { last } } else if frame.1 > last.1 { frame } else { last });
        let center = match down {
            true => (after.0 + after.2 + SIBLING + w / 2.0, after.1 + after.3 / 2.0),
            false => (after.0 + after.2 / 2.0, after.1 + after.3 + SIBLING + h / 2.0),
        };
        self.checkpoint();
        let made = self.box_like(index, center, 0);
        self.join(parent, made);
        self.selected = vec![made];
        self.edit_text(made);
        self.changed();
        true
    }

    /// Shift+Tab: the parent of the box picked out, picked out.
    pub(in crate::studio) fn to_parent(&mut self) -> bool {
        let Some(index) = self.map_box() else { return false };
        let id = self.id_of(index);
        self.commit_text();
        match self.parent_box(id).and_then(|parent| self.index_of(parent)) {
            Some(parent) => self.selected = vec![parent],
            None => self.selected = self.index_of(id).into_iter().collect(),
        }
        self.dirty = true;
        true
    }

    /// A box made a size, from its middle.
    fn fit_box(&mut self, index: usize, (w, h): (f32, f32)) {
        if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
            let (x, y, ow, oh) = shape.frame();
            let (cx, cy) = (x + ow / 2.0, y + oh / 2.0);
            shape.points = vec![(cx - w / 2.0, cy - h / 2.0), (cx + w / 2.0, cy + h / 2.0)];
            shape.base = (w, h);
            shape.touch();
        }
    }

    /// The top of the map something is in, up its arrows.
    fn map_root(&self, id: u32) -> u32 {
        let mut at = id;
        let mut seen = vec![id];
        while let Some(parent) = self.parent_box(at) {
            if seen.contains(&parent) {
                break;
            }
            seen.push(parent);
            at = parent;
        }
        at
    }

    /// Shift+L: the map or tree the box picked out is in laid out neatly,
    /// from its top: left to right, or top down where it goes down, each
    /// level's boxes in their order with even gaps, each parent in the
    /// middle of its children; a mind map's two sides kept. Nothing else on
    /// the board moves. One step to undo.
    pub(in crate::studio) fn tidy(&mut self) {
        let Some(index) = self.picked().or(self.board.dots_of).filter(|&index| self.is_box(index)) else {
            return self.say("Pick a box of the map to tidy it");
        };
        let root = self.map_root(self.id_of(index));
        if self.child_boxes(root).is_empty() {
            return self.say("Nothing joined to it to tidy");
        }
        self.checkpoint();
        self.lay_out_tree(root, self.goes_down(root), true);
        self.changed();
        self.say("Tidied");
    }

    /// A tree laid out from its top, which stays where it is.
    /// With `sides`, a mind map's children either side of it stay on their
    /// side; without, all go after it.
    pub(in crate::studio) fn lay_out_tree(&mut self, root: u32, down: bool, sides: bool) {
        let Some(at) = self.index_of(root) else { return };
        let (rx, ry, rw, rh) = self.box_frame(at);
        let middle = (rx + rw / 2.0, ry + rh / 2.0);
        let children = self.child_boxes(root);
        // A mind map's children either side of it stay on their side.
        let (before, after): (Vec<u32>, Vec<u32>) = children.into_iter().partition(|&child| {
            sides && self.index_of(child).is_some_and(|index| {
                let (x, y, w, h) = self.box_frame(index);
                if down { y + h / 2.0 < middle.1 } else { x + w / 2.0 < middle.0 }
            })
        });
        let mut seen = vec![root];
        for (side, kids) in [(-1.0, before), (1.0, after)] {
            if kids.is_empty() {
                continue;
            }
            let spans: Vec<f32> = kids.iter().map(|&kid| self.span(kid, down, &mut seen.clone())).collect();
            let total = spans.iter().sum::<f32>() + SIBLING * (kids.len() - 1) as f32;
            let mut along = if down { middle.0 } else { middle.1 } - total / 2.0;
            let reach = if down { rh / 2.0 } else { rw / 2.0 };
            for (&kid, span) in kids.iter().zip(spans) {
                self.place(kid, down, side, if down { middle.1 } else { middle.0 } + side * (reach + self.board.level), along, span, &mut seen);
                along += span + SIBLING;
            }
        }
    }

    /// How much room a box and all under it take across the way the tree
    /// goes: as tall as its children are stacked, or as itself.
    /// How wide a tree that goes down is under a box.
    pub(in crate::studio) fn span_of(&self, id: u32) -> f32 {
        self.span(id, true, &mut Vec::new())
    }

    pub(in crate::studio) fn span(&self, id: u32, down: bool, seen: &mut Vec<u32>) -> f32 {
        let Some(index) = self.index_of(id) else { return 0.0 };
        seen.push(id);
        let (_, _, w, h) = self.box_frame(index);
        let own = if down { w } else { h };
        let kids: Vec<u32> = self.child_boxes(id).into_iter().filter(|kid| !seen.contains(kid)).collect();
        if kids.is_empty() {
            return own;
        }
        let stacked = kids.iter().map(|&kid| self.span(kid, down, seen)).sum::<f32>() + SIBLING * (kids.len() - 1) as f32;
        stacked.max(own)
    }

    /// A box put with its near edge at `level` (away from its parent on
    /// `side`), in the middle of the room `along` to `along + span` it has,
    /// and its children after it, the same way.
    #[allow(clippy::too_many_arguments)]
    fn place(&mut self, id: u32, down: bool, side: f32, level: f32, along: f32, span: f32, seen: &mut Vec<u32>) {
        if seen.contains(&id) {
            return;
        }
        seen.push(id);
        let Some(index) = self.index_of(id) else { return };
        let (x, y, w, h) = self.box_frame(index);
        let (deep, wide) = if down { (h, w) } else { (w, h) };
        let near = if side > 0.0 { level } else { level - deep };
        let mid = along + span / 2.0;
        let (nx, ny) = if down { (mid - wide / 2.0, near) } else { (near, mid - wide / 2.0) };
        let mut all = self.inside(id);
        all.push(id);
        for member in all {
            if let Some(member) = self.index_of(member) {
                self.doc.layers[member].translate(nx - x, ny - y);
            }
        }
        let kids: Vec<u32> = self.child_boxes(id).into_iter().filter(|kid| !seen.contains(kid)).collect();
        if kids.is_empty() {
            return;
        }
        let spans: Vec<f32> = kids.iter().map(|&kid| self.span(kid, down, &mut seen.clone())).collect();
        let total = spans.iter().sum::<f32>() + SIBLING * (kids.len() - 1) as f32;
        let mut at = mid - total / 2.0;
        let next = if side > 0.0 { near + deep + self.board.level } else { near - self.board.level };
        for (&kid, kid_span) in kids.iter().zip(spans) {
            self.place(kid, down, side, next, at, kid_span, seen);
            at += kid_span + SIBLING;
        }
    }

    /// Ctrl+Enter in text on a board: each line of an indented list a box,
    /// each joined to the line it is indented under, laid out as a map
    /// where the text was; the text gone. One step to undo.
    pub(in crate::studio) fn list_to_map(&mut self, index: usize) -> bool {
        let Some(Layer { kind: Kind::Shape(text), .. }) = self.doc.layers.get(index) else { return false };
        if text.tool != Tool::Text {
            return false;
        }
        let origin = text.points[0];
        // Mermaid typed: the diagram it says, the words gone.
        if mermaid::kind_of(&text.text).is_some() {
            let words = text.text.clone();
            self.commit_text();
            if index < self.doc.layers.len() {
                self.doc.layers.remove(index);
            }
            self.selected.clear();
            return self.bring_mermaid(&words);
        }
        let lines: Vec<(usize, String)> = text
            .text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let indent = line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>();
                let words = line.trim().trim_start_matches(['-', '*', '\u{2022}']).trim().to_owned();
                (indent, words)
            })
            .collect();
        if lines.len() < 2 {
            return false;
        }
        self.commit_text();
        self.checkpoint();
        if index < self.doc.layers.len() {
            self.doc.layers.remove(index);
        }
        let mut stack: Vec<(usize, usize)> = Vec::new();
        let mut roots = Vec::new();
        let mut top = origin.1;
        for (indent, words) in lines {
            let mut shape = Shape::new(Tool::Rect, self.board.stroke, LINE, (origin.0, top));
            self.board_dress(&mut shape);
            shape.form = shapes::FORM_BOX;
            shape.points[1] = (origin.0 + BOX.0, top + BOX.1 * 0.7);
            shape.base = (BOX.0, BOX.1 * 0.7);
            shape.text = words;
            self.doc.layers.push(Layer::shape(shape));
            let made = self.doc.layers.len() - 1;
            self.grow_for_words(made);
            top += BOX.1;
            while stack.last().is_some_and(|&(depth, _)| depth >= indent) {
                stack.pop();
            }
            match stack.last() {
                Some(&(_, parent)) => {
                    self.join(parent, made);
                }
                None => roots.push(made),
            }
            stack.push((indent, made));
        }
        self.changed();
        for root in roots {
            let id = self.id_of(root);
            self.lay_out_tree(id, false, true);
        }
        self.selected.clear();
        self.changed();
        self.say("A map, a box a line");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn frame_of(studio: &Studio, id: u32) -> (f32, f32, f32, f32) {
        studio.box_frame(studio.index_of(id).unwrap())
    }

    #[test]
    fn tab_adds_a_child_enter_the_next_and_shift_tab_goes_back_up() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('o'), Mods::default());
        pull(&mut studio, (100.0, 300.0), (250.0, 370.0), Mods::default());
        type_in(&mut studio, "hotshot");
        let root = shape(&studio, 0).id;
        // Tab while typing: a child to its right, joined, typed into.
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Capture");
        let first = studio.doc.layers.iter().rev().find_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.text == "Capture" => Some(shape.clone()),
            _ => None,
        }).unwrap();
        assert_eq!(studio.parent_box(first.id), Some(root));
        assert!(first.frame().0 > 250.0 + 40.0, "to the right of it");
        assert_ne!(first.fill, shape(&studio, 0).fill, "a shade lighter");
        // Enter: the next under it, the same parent's, typed into.
        press_key(&mut studio, Key::Enter, Mods::default());
        type_in(&mut studio, "Design");
        press_key(&mut studio, Key::Enter, Mods::default());
        type_in(&mut studio, "Board");
        let kids = studio.child_boxes(root);
        assert_eq!(kids.len(), 3);
        let words: Vec<String> = kids.iter().map(|&kid| shape(&studio, studio.index_of(kid).unwrap()).text).collect();
        assert_eq!(words, ["Capture", "Design", "Board"]);
        assert!(frame_of(&studio, kids[1]).1 > frame_of(&studio, kids[0]).1 + frame_of(&studio, kids[0]).3, "under it");
        // A grandchild, and Shift+Tab back up to its parent.
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "mind maps");
        press_key(&mut studio, Key::Tab, Mods { shift: true, ..Mods::default() });
        assert!(studio.editing.is_none());
        assert_eq!(studio.selected, vec![studio.index_of(kids[2]).unwrap()]);
        // A box of no map: Enter just ends the typing.
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (900.0, 600.0), (1000.0, 650.0), Mods::default());
        let count = studio.doc.layers.len();
        press_key(&mut studio, Key::Enter, Mods::default());
        assert!(studio.editing.is_none() && studio.doc.layers.len() == count);
    }

    #[test]
    fn shift_l_lays_a_map_out_neatly_and_nothing_else_moves() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (400.0, 300.0), (540.0, 350.0), Mods::default());
        type_in(&mut studio, "Trip");
        for (words, at) in [("Flights", (700.0, 120.0)), ("Hotels", (760.0, 420.0)), ("Rail", (650.0, 600.0))] {
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, at, (at.0 + 120.0, at.1 + 46.0), Mods::default());
            type_in(&mut studio, words);
        }
        // A box on its own, which tidying leaves alone.
        press_key(&mut studio, Key::Escape, Mods::default());
        pull(&mut studio, (100.0, 700.0), (200.0, 740.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let alone = shape(&studio, 4).frame();
        for child in 1..4 {
            studio.join(0, child);
        }
        studio.changed();
        press_key(&mut studio, Key::Char('v'), Mods::default());
        studio.selected = vec![2];
        press_key(&mut studio, Key::Char('L'), Mods { shift: true, ..Mods::default() });
        let root = shape(&studio, 0).frame();
        let kids: Vec<(f32, f32, f32, f32)> = (1..4).map(|index| shape(&studio, index).frame()).collect();
        // A column to its right, in their order, evenly spaced, the root in
        // the middle of them.
        assert!(kids.iter().all(|kid| (kid.0 - kids[0].0).abs() < 0.5 && kid.0 > root.0 + root.2));
        assert!(kids[0].1 < kids[1].1 && kids[1].1 < kids[2].1);
        let gaps: Vec<f32> = kids.windows(2).map(|pair| pair[1].1 - (pair[0].1 + pair[0].3)).collect();
        assert!((gaps[0] - gaps[1]).abs() < 0.5);
        let middle = (kids[0].1 + kids[2].1 + kids[2].3) / 2.0;
        assert!((middle - (root.1 + root.3 / 2.0)).abs() < 1.0);
        assert_eq!(shape(&studio, 4).frame(), alone);
        // One step to undo.
        studio.undo();
        assert_eq!(shape(&studio, 1).frame().1, 120.5);
    }

    #[test]
    fn an_indented_list_and_ctrl_enter_is_a_map() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('t'), Mods::default());
        tap(&mut studio, (200.0, 200.0));
        type_in(&mut studio, "Japan\n  Flights\n  Hotels\n    Kyoto\n  Food");
        press_key(&mut studio, Key::Enter, Mods { ctrl: true, ..Mods::default() });
        let boxes: Vec<Shape> = studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Rect => Some(shape.clone()),
            _ => None,
        }).collect();
        assert_eq!(boxes.iter().map(|shape| shape.text.as_str()).collect::<Vec<_>>(), ["Japan", "Flights", "Hotels", "Kyoto", "Food"]);
        assert!(!studio.doc.layers.iter().any(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Text)));
        assert_eq!(studio.child_boxes(boxes[0].id).len(), 3);
        assert_eq!(studio.parent_box(boxes[3].id), Some(boxes[2].id));
    }
}
