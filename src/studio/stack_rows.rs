//! Stacks in the list of layers, as Figma lists an auto layout: a row of
//! its own, called Row, Column or Stack for the way it goes or what it is
//! renamed, with what is in it beneath it, in its order and set in, that
//! folds away. Clicked, it picks out the whole stack; its menu takes the
//! auto layout away, picks out what is in it, renames it or deletes it
//! with all of it; and a row dragged into it, out of it or along it puts
//! that in, takes it out or moves it along, laid out again at once.

use super::*;
use stacks::{layer_id, Stack};

/// A row of the list of layers: a layer, or a stack, and how deep in
/// stacks it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Row {
    Layer(usize, usize),
    Stack(u32, usize),
}

/// What a layer's row let go in the list does by stacks: nothing of
/// theirs; goes into a stack, before or after something in it or at its
/// end; or comes out of the one it is in.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum StackDrop {
    Nothing,
    Into(u32, Option<u32>),
    Out,
}

/// The stack outside all others that something is in, if it is in one.
fn outermost(doc: &Doc, id: u32) -> Option<u32> {
    let mut found = None;
    let mut at = id;
    while let Some(stack) = doc.stacks.iter().find(|stack| stack.members.contains(&at)) {
        found = Some(stack.id);
        at = stack.id;
    }
    found
}

/// The rows a layer comes to in the list, gone through from the topmost:
/// itself; or, in a stack, the whole stack where the first of it comes,
/// and nothing after.
pub(super) fn rows_for(doc: &Doc, index: usize, shown: &mut Vec<u32>, folded: &[u32], rows: &mut Vec<Row>) {
    match outermost(doc, layer_id(&doc.layers[index])) {
        Some(top) if !shown.contains(&top) => {
            shown.push(top);
            stack_rows(doc, top, 0, folded, rows);
        }
        Some(_) => {}
        None => rows.push(Row::Layer(index, 0)),
    }
}

/// A stack's row and, unless it is folded, the rows of what is in it, in
/// its order, set in a step.
fn stack_rows(doc: &Doc, id: u32, depth: usize, folded: &[u32], rows: &mut Vec<Row>) {
    rows.push(Row::Stack(id, depth));
    if folded.contains(&id) {
        return;
    }
    let Some(stack) = doc.stacks.iter().find(|stack| stack.id == id) else { return };
    for &member in &stack.members {
        if doc.stacks.iter().any(|stack| stack.id == member) {
            stack_rows(doc, member, depth + 1, folded, rows);
        } else if let Some(index) = doc.layers.iter().position(|layer| layer_id(layer) == member) {
            rows.push(Row::Layer(index, depth + 1));
        }
    }
}

/// The icon of a stack's row: which way it goes.
pub(super) fn stack_icon(stack: &Stack) -> &'static str {
    match stack.way {
        stacks::ROW => icons::DIST_H,
        stacks::COLUMN => icons::DIST_V,
        _ => icons::ALIGN_LEFT,
    }
}

impl Studio {
    /// A stack by what it is called, in the frame worked in or among the
    /// page's own layers.
    pub(super) fn stack_by_id(&self, id: u32) -> Option<&Stack> {
        self.doc.stacks.iter().chain(self.loose().stacks.iter()).find(|stack| stack.id == id)
    }

    /// A stack picked out, as a click on its row picks it out: the page's
    /// own layers worked in first, if it is theirs, and all of it picked
    /// out, which is what says it is the stack.
    pub(super) fn pick_stack(&mut self, id: u32) {
        if self.stack_index(id).is_none() && self.loose().stacks.iter().any(|stack| stack.id == id) {
            self.activate(LOOSE);
        }
        if self.stack_index(id).is_none() {
            return;
        }
        self.selected = self.leaves(id).into_iter().filter_map(|leaf| self.layer_by_id(leaf)).collect();
        self.members_picked = None;
        self.frame_picked = false;
        self.dirty = true;
    }

    /// What is in a stack picked out, each of them rather than the stack.
    pub(super) fn pick_members(&mut self, id: u32) {
        self.pick_stack(id);
        self.members_picked = Some(self.selected.clone());
    }

    /// A stack and everything in it gone, as one step to undo.
    pub(super) fn delete_stack(&mut self, id: u32) {
        self.pick_stack(id);
        self.members_picked = None;
        self.delete();
    }

    /// A stack's row pressed: the stack picked out, and with a double click
    /// renamed where it is listed.
    pub(super) fn press_stack_row(&mut self, id: u32, double: bool) {
        self.commit_text();
        self.pick_stack(id);
        if double {
            let title = self.stack_by_id(id).map(Stack::title).unwrap_or_default();
            self.start_naming(Naming::Stack(id), title);
        }
    }

    /// A stack's row folded away, or opened.
    pub(super) fn fold_stack(&mut self, id: u32) {
        match self.folded.iter().position(|&folded| folded == id) {
            Some(at) => drop(self.folded.remove(at)),
            None => self.folded.push(id),
        }
        self.dirty = true;
    }

    /// A stack renamed, from what was typed.
    pub(super) fn rename_stack(&mut self, id: u32, name: String) {
        if let Some(index) = self.stack_index(id) {
            self.doc.stacks[index].name = Some(name);
        }
    }

    /// A stack laid out by the order it holds things in, each moved to
    /// where that says, rather than put in order by where each is: what a
    /// row dragged along it, or into it, wants.
    pub(super) fn lay_out_in_order(&mut self, id: u32) {
        let Some(index) = self.stack_index(id) else { return };
        let mut stack = self.doc.stacks[index].clone();
        let mut units = Vec::new();
        for &member in &stack.members {
            if let Some(bounds) = self.unit_bounds(member) {
                units.push((member, bounds));
            }
        }
        let frame = (self.doc.width as f32, self.doc.height as f32);
        let sizes: Vec<(f32, f32)> = units.iter().map(|(_, b)| (b.2, b.3)).collect();
        let (bounds, tops) = stacks::arrange(&stack, &sizes, frame);
        for (&(unit, b), top) in units.iter().zip(&tops) {
            self.move_unit(unit, (top.0 - b.0, top.1 - b.1));
        }
        (stack.x, stack.y, stack.w, stack.h) = bounds;
        stack.seen = units.iter().map(|&(unit, b)| (unit, b.2, b.3)).collect();
        self.doc.stacks[index] = stack;
    }

    /// What a layer's row let go in the list does by stacks: on a stack's
    /// row, it goes into that stack, at its end; on the row of something
    /// in a stack, into that stack there, before it or after it, or along
    /// it if it was in it already; from a stack onto a row of none, out of
    /// it, to go where that row says.
    pub(super) fn stack_drop(&self, index: usize, control: Control) -> StackDrop {
        let id = layer_id(&self.doc.layers[index]);
        let (into, by) = match control {
            Control::StackRow(stack) if self.stack_index(stack).is_some() => (stack, None),
            Control::Layer(target) | Control::Eye(target) | Control::Lock(target) if target != index => {
                let other = layer_id(&self.doc.layers[target]);
                match self.stack_of(other) {
                    Some(stack) => (self.doc.stacks[stack].id, Some(other)),
                    None if self.stack_of(id).is_some() => return StackDrop::Out,
                    None => return StackDrop::Nothing,
                }
            }
            _ if self.stack_of(id).is_some() && matches!(control, Control::Frame(_) | Control::PageLayer(_)) => return StackDrop::Out,
            _ => return StackDrop::Nothing,
        };
        match into == id || self.leaves(id).contains(&into) {
            true => StackDrop::Nothing,
            false => StackDrop::Into(into, by),
        }
    }

    /// A layer taken out of the stack it is in, if it is in one; the rest
    /// close up as they are laid out again.
    pub(super) fn take_out_of_stack(&mut self, index: usize) {
        let id = layer_id(&self.doc.layers[index]);
        if let Some(from) = self.stack_of(id) {
            self.doc.stacks[from].members.retain(|&member| member != id);
        }
    }

    /// A layer put into a stack, before or after something in it or at its
    /// end, out of the one it was in, and laid out there.
    pub(super) fn put_in_stack(&mut self, index: usize, into: u32, by: Option<u32>, over: bool) {
        let id = layer_id(&self.doc.layers[index]);
        self.take_out_of_stack(index);
        let Some(stack) = self.stack_index(into) else { return };
        let members = &mut self.doc.stacks[stack].members;
        let at = by.and_then(|by| members.iter().position(|&member| member == by)).map_or(members.len(), |at| if over { at } else { at + 1 });
        members.insert(at.min(members.len()), id);
        self.lay_out_in_order(into);
        self.selected = self.layer_by_id(id).into_iter().collect();
        self.changed();
    }

    /// The whole of each stack the box dragged out touches picked out with
    /// what it touches, as Figma's box picks out an auto layout as one
    /// thing; with Ctrl, what is in them alone, as it is.
    pub(super) fn marquee_stacks(&mut self, (left, top, right, bottom): (f32, f32, f32, f32)) {
        let tops: Vec<u32> = self.doc.stacks.iter().filter(|stack| self.stack_of(stack.id).is_none()).map(|stack| stack.id).collect();
        for top_id in tops {
            let Some(stack) = self.stack_index(top_id).map(|index| self.doc.stacks[index].clone()) else { continue };
            let leaves: Vec<usize> = self.leaves(top_id).into_iter().filter_map(|leaf| self.layer_by_id(leaf)).collect();
            let touched = stack.x < right && stack.x + stack.w > left && stack.y < bottom && stack.y + stack.h > top || leaves.iter().any(|index| self.selected.contains(index));
            if !touched {
                continue;
            }
            for index in leaves {
                let layer = &self.doc.layers[index];
                if !layer.hidden && !layer.locked && !self.selected.contains(&index) {
                    self.selected.push(index);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> Studio {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (1.0, (300.0, 100.0));
        studio
    }

    /// A picture so large, its top left at a point of the frame.
    fn put(studio: &mut Studio, (x, y): (f32, f32), (w, h): (f32, f32), name: &str) -> usize {
        let image = Arc::new(Image::new(w as i32, h as i32, vec![0xFF3E63DD; (w * h) as usize]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, name.into(), (x + w / 2.0, y + h / 2.0))));
        studio.doc.layers.len() - 1
    }

    /// The rows of the list of layers, as what each is, and how far in.
    fn rows(studio: &mut Studio) -> Vec<(Control, i32)> {
        studio.layout().controls.iter().filter(|(rect, control)| rect.x < LEFT_W && matches!(control, Control::Layer(_) | Control::StackRow(_) | Control::PageLayer(_))).map(|(rect, control)| (*control, rect.x)).collect()
    }

    /// A row of the list dragged by the pointer onto the top or the
    /// bottom of another.
    fn drag_row(studio: &mut Studio, from: Control, to: Control, top: bool) {
        let a = studio.control_rect(from).unwrap();
        let b = studio.control_rect(to).unwrap();
        studio.pointer_moved(a.x + 40, a.center().1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let y = if top { b.y + 3 } else { b.bottom() - 3 };
        studio.pointer_moved(b.x + 40, y, Mods::default());
        studio.released();
    }

    /// What is in the first stack, by name, in its order.
    fn order(studio: &Studio) -> Vec<String> {
        studio.doc.stacks[0].members.iter().filter_map(|&member| studio.layer_by_id(member)).map(|index| studio.doc.layers[index].name()).collect()
    }

    /// Three boxes in a row stacked, and one beside in none.
    fn stacked() -> (Studio, u32) {
        let mut studio = studio();
        for (at, which) in [(100.0, "a"), (200.0, "b"), (300.0, "c")] {
            put(&mut studio, (at, 100.0), (50.0, 50.0), which);
        }
        put(&mut studio, (100.0, 400.0), (50.0, 50.0), "loose");
        studio.changed();
        studio.selected = vec![0, 1, 2];
        studio.add_stack();
        let id = studio.doc.stacks[0].id;
        studio.selected.clear();
        (studio, id)
    }

    /// The row of a list right clicked, and an entry of its menu chosen.
    fn from_menu(studio: &mut Studio, control: Control, says: &str) {
        let at = studio.control_rect(control).unwrap();
        studio.pointer_moved(at.x + 60, at.center().1, Mods::default());
        studio.pressed(3, 200_000);
        let kind = studio.menu.expect("a menu").kind;
        let row = studio.menu_items(kind).iter().position(|item| matches!(item, menu::Item::Entry { says: known, .. } if known == says)).unwrap();
        studio.press_menu(0, row as u16);
    }

    #[test]
    fn a_stack_is_a_row_of_the_list_with_what_is_in_it_under_it_folded_renamed_and_picked_out() {
        let (mut studio, id) = stacked();
        // Listed as a row, called Row for the way it goes, what is in it
        // under it in its order, set in a step; the layer in none on its own.
        let listed = rows(&mut studio);
        assert_eq!(listed.iter().map(|(control, _)| *control).collect::<Vec<_>>(), vec![Control::Layer(3), Control::StackRow(id), Control::Layer(0), Control::Layer(1), Control::Layer(2)]);
        assert_eq!(listed[2].1 - listed[1].1, 14);
        assert_eq!(studio.stack_by_id(id).map(Stack::title).as_deref(), Some("Row"));
        // Clicked, the stack is what is picked out.
        let at = studio.control_rect(Control::StackRow(id)).unwrap();
        studio.pointer_moved(at.x + 60, at.center().1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
        assert_eq!(studio.picked_stack(), Some(0));
        // Folded by its caret, and opened again.
        studio.press_control(Control::StackFold(id), false);
        assert_eq!(rows(&mut studio).len(), 2);
        studio.press_control(Control::StackFold(id), false);
        // Renamed with a double click, and kept in the file.
        studio.press_control(Control::StackRow(id), true);
        for letter in "Cards".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!(studio.stack_by_id(id).map(Stack::title).as_deref(), Some("Cards"));
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert_eq!(back[0].frames[0].stacks[0].name.as_deref(), Some("Cards"));
        // Found by Ctrl+K, and picked out from there.
        studio.selected.clear();
        studio.open_palette();
        for letter in "cards".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        let row = studio.hits().iter().position(|hit| hit.found == search::Found::Stack(id)).expect("found");
        studio.choose_hit(row);
        assert_eq!(studio.picked_stack(), Some(0));
        // Its menu picks out what is in it, each of it, not the stack.
        from_menu(&mut studio, Control::StackRow(id), "Select members");
        assert_eq!((studio.selected.len(), studio.picked_stack()), (3, None));
        // A box dragged out over one of it picks out all of it; with Ctrl,
        // only what it touches.
        for (mods, picked) in [(Mods { ctrl: true, ..Mods::default() }, 1), (Mods::default(), 3)] {
            studio.selected.clear();
            let (a, b) = (studio.to_window((190.0, 30.0)), studio.to_window((230.0, 130.0)));
            studio.pointer_moved(a.0, a.1, mods);
            studio.last_press.0 = 0;
            studio.pressed(1, 300_000);
            studio.pointer_moved(b.0, b.1, mods);
            studio.released();
            assert_eq!(studio.selected.len(), picked);
        }
        assert_eq!(studio.picked_stack(), Some(0));
        // Deleted from its menu, with everything in it, as one step.
        from_menu(&mut studio, Control::StackRow(id), "Delete stack and members");
        assert_eq!((studio.doc.layers.len(), studio.doc.stacks.len()), (1, 0));
        studio.undo();
        assert_eq!((studio.doc.layers.len(), studio.doc.stacks.len()), (4, 1));
        // And its auto layout taken away from there, what was in it kept.
        from_menu(&mut studio, Control::StackRow(id), "Remove auto layout");
        assert_eq!((studio.doc.layers.len(), studio.doc.stacks.len()), (4, 0));
    }

    #[test]
    fn rows_dragged_along_a_stack_into_it_and_out_of_it_lay_it_out_again() {
        let (mut studio, id) = stacked();
        let first = studio.doc.layers[0].bounds(&mut studio.text).0;
        // The last dragged to before the first: first along the row.
        drag_row(&mut studio, Control::Layer(2), Control::Layer(0), true);
        assert_eq!(order(&studio), ["c", "a", "b"]);
        assert_eq!(studio.doc.layers[2].bounds(&mut studio.text).0, first, "laid out where the first was");
        studio.undo();
        assert_eq!(order(&studio), ["a", "b", "c"]);
        // The one in none dragged onto the bottom of the row of one in it:
        // in it, after that one, laid out in the row.
        drag_row(&mut studio, Control::Layer(3), Control::Layer(0), false);
        assert_eq!(order(&studio), ["a", "loose", "b", "c"]);
        let (_, y, ..) = studio.doc.layers[3].bounds(&mut studio.text);
        assert!((y - 100.0).abs() < 1.0, "laid out in the row, at {y}");
        // Onto the stack's own row: at its end.
        drag_row(&mut studio, Control::Layer(3), Control::StackRow(id), true);
        assert_eq!(order(&studio), ["a", "b", "c", "loose"]);
        // Out again onto a row of none: no longer in it, as one step.
        put(&mut studio, (600.0, 400.0), (50.0, 50.0), "other");
        studio.changed();
        let steps = studio.undo.len();
        drag_row(&mut studio, Control::Layer(1), Control::Layer(4), true);
        assert_eq!(order(&studio), ["a", "c", "loose"]);
        assert_eq!(studio.undo.len(), steps + 1, "one step");
        studio.undo();
        assert_eq!(order(&studio).len(), 4);
    }
}
