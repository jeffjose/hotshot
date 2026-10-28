//! Drawing, then typing, on a board: a box drawn is already being typed
//! into; a box picked out is typed over, as a spreadsheet's cell is, and
//! Enter types into it instead; the box tool stays in hand, as boxes come
//! in runs, and a click with it makes one the size of the last two.

use super::*;

/// How near two sizes are to be the same, for a click to make another.
const SAME: f32 = 4.0;

impl Studio {
    /// Whether a layer is a board's box, oval, diamond or note.
    pub(in crate::studio) fn is_box(&self, index: usize) -> bool {
        matches!(self.doc.layers.get(index), Some(Layer { kind: Kind::Shape(shape), .. }) if matches!(shape.tool, Tool::Rect | Tool::Ellipse))
    }

    /// Whether a layer has words of its own a board types into: a box, or
    /// an arrow or a line.
    pub(in crate::studio) fn has_words(&self, index: usize) -> bool {
        matches!(self.doc.layers.get(index), Some(Layer { kind: Kind::Shape(shape), locked: false, .. }) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow))
    }

    /// The size of the last two boxes drawn, if they were the same: what a
    /// click with the box tool makes the next.
    pub(in crate::studio) fn run_size(&self) -> Option<(f32, f32)> {
        match self.board.sizes[..] {
            [.., a, b] if (a.0 - b.0).abs() <= SAME && (a.1 - b.1).abs() <= SAME => Some(b),
            _ => None,
        }
    }

    /// A box's size remembered as the latest drawn.
    pub(in crate::studio) fn note_size(&mut self, size: (f32, f32)) {
        self.board.sizes.push(size);
        if self.board.sizes.len() > 2 {
            self.board.sizes.remove(0);
        }
    }

    /// A press on a board, before the pointer's own: a dot on a box's side
    /// to pull an arrow out of; a double click on a box or an arrow, its
    /// words to type into; on nothing, a label begun there.
    pub(in crate::studio) fn board_pressed(&mut self, at: (f32, f32), at_window: (i32, i32), double: bool) -> bool {
        if self.tool != Tool::Select || self.space {
            return false;
        }
        if self.press_dot(at_window) {
            return true;
        }
        if !double {
            return false;
        }
        match self.layer_at(at) {
            Some(index) if self.has_words(index) => {
                self.checkpoint();
                self.selected = vec![index];
                self.edit_text(index);
                self.drag = Drag::None;
                true
            }
            Some(_) => false,
            None => {
                self.checkpoint();
                let mut shape = Shape::new(Tool::Text, INK, LINE, at);
                self.board_dress(&mut shape);
                shape.points[0].1 = at.1 - shape.font_px as f32 / 2.0;
                self.doc.layers.push(Layer::shape(shape));
                let index = self.doc.layers.len() - 1;
                self.selected = vec![index];
                self.edit_text(index);
                self.drag = Drag::None;
                self.changed();
                true
            }
        }
    }

    /// What a drag on a board let go of did, where it is a board's own: a
    /// shape drawn, typed into at once; an arrow pulled out of a side.
    pub(in crate::studio) fn board_released(&mut self, drag: Drag) -> bool {
        match drag {
            Drag::Draw { from } => {
                self.board_drawn(from);
                true
            }
            Drag::Pull { from, side, moved } => {
                self.pull_released(from, side, moved);
                true
            }
            _ => false,
        }
    }

    /// A shape drawn on a board, let go: a box typed into, the box tool
    /// kept in hand; a click with no drag a box the size of the last two,
    /// or, before there are two, the pointer back; an arrow's ends tied to
    /// what they are on, and the pointer back.
    fn board_drawn(&mut self, from: (f32, f32)) {
        let Some(index) = self.doc.layers.len().checked_sub(1) else { return };
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return };
        let boxy = matches!(shape.tool, Tool::Rect | Tool::Ellipse);
        let (slight, sticky) = (shape.is_slight(), shape.form == shapes::FORM_STICKY);
        if slight {
            let run = if boxy { self.run_size() } else { None };
            let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) else { return };
            match (run, sticky) {
                (_, true) | (Some(_), false) => {
                    let size = if sticky { run.unwrap_or(STICKY) } else { run.unwrap_or(BOX) };
                    shape.points = vec![(from.0 - size.0 / 2.0, from.1 - size.1 / 2.0), (from.0 + size.0 / 2.0, from.1 + size.1 / 2.0)];
                    shape.touch();
                }
                (None, false) => {
                    // A click on empty board: the pointer back.
                    self.doc.layers.pop();
                    self.undo.pop();
                    self.set_tool(Tool::Select);
                    self.changed();
                    return;
                }
            }
        }
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) else { return };
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                let (_, _, w, h) = shape.frame();
                shape.base = (w, h);
                self.note_size((w, h));
                self.selected = vec![index];
                self.edit_text(index);
                self.changed();
            }
            Tool::Arrow | Tool::Line => {
                self.selected = vec![index];
                self.tool = Tool::Select;
                self.board_stick(index, &[0, 1]);
            }
            Tool::Pen | Tool::Marker => self.changed(),
            _ => {
                self.selected = vec![index];
                self.tool = Tool::Select;
                self.changed();
            }
        }
    }

    /// Ends of a line on a board stuck to what they are put down on: a
    /// box as a whole, wherever on it, for the arrow to go from side to
    /// side as it moves; a picture at the point, to point at what is in it.
    pub(in crate::studio) fn board_stick(&mut self, index: usize, ends: &[usize]) {
        self.stick(index, ends);
        let boxes: Vec<u32> = self.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Text | Tool::Badge) => Some(shape.id),
            _ => None,
        }).collect();
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            let same = shape.stuck[0].map(|(id, ..)| id) == shape.stuck[1].map(|(id, ..)| id);
            for &end in ends {
                if let Some((id, _, _)) = shape.stuck[end].filter(|(id, ..)| boxes.contains(id) && !same) {
                    shape.stuck[end] = Some((id, shapes::WHOLE, shapes::WHOLE));
                }
            }
            shape.routed = true;
        }
        self.changed();
    }

    /// A key while a board's words are typed: Enter ends them, or with
    /// Shift begins a new line; letters and Backspace type, the box growing
    /// to hold them. Nothing for the rest, which are typed as text is.
    pub(in crate::studio) fn board_typing_key(&mut self, key: Key, mods: Mods) -> Option<Outcome> {
        let index = self.editing?;
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return None };
        let worded = matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow);
        if !worded || mods.ctrl {
            return None;
        }
        let picked = std::mem::take(&mut self.words_picked) && key != Key::Other;
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) else { return None };
        match key {
            Key::Enter if mods.shift => shape.text.push('\n'),
            Key::Enter => {
                self.commit_text();
                return Some(Outcome::Continue);
            }
            Key::Char(letter) => {
                if picked {
                    shape.text.clear();
                }
                shape.text.push(letter);
            }
            Key::Backspace if picked => shape.text.clear(),
            Key::Backspace => drop(shape.text.pop()),
            _ => return None,
        }
        shape.touch();
        self.words_changed(index);
        Some(Outcome::Continue)
    }

    /// A box's words changed as they are typed.
    pub(in crate::studio) fn words_changed(&mut self, index: usize) {
        self.changed_layers(&[index]);
        self.changed();
    }

    /// A key on a board, before Design's would take it: with a box picked
    /// out, a letter types over its words and Enter types into them; C and
    /// a key colors; and a letter takes up one of the board's tools.
    /// Nothing, for what is not a board's.
    pub(in crate::studio) fn board_key(&mut self, key: Key, lower: Key, mods: Mods) -> Option<Outcome> {
        if mods.ctrl || mods.alt {
            return None;
        }
        if std::mem::take(&mut self.board.coloring) {
            if let Some(ctl) = color_key(lower) {
                self.press_style(ctl);
            }
            return Some(Outcome::Continue);
        }
        // Typed over only with the pointer in hand: with the box tool, the
        // box just drawn is picked out, and a letter takes up a tool.
        let picked = self.picked().filter(|&index| self.tool == Tool::Select && self.has_words(index));
        if let (Some(index), Key::Char(letter)) = (picked, key) {
            if !self.keeps_key(index, letter) {
                self.checkpoint();
                if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                    shape.text = letter.to_string();
                    shape.touch();
                }
                self.edit_text(index);
                self.words_changed(index);
                return Some(Outcome::Continue);
            }
        }
        if let (Some(index), Key::Enter) = (picked, key) {
            self.checkpoint();
            self.edit_text(index);
            self.changed();
            return Some(Outcome::Continue);
        }
        let Key::Char(letter) = lower else { return None };
        if mods.shift {
            return None;
        }
        if letter == 'c' {
            self.board.coloring = true;
            self.say("Color: 1\u{2013}5 a line, Q W E R T a fill, X no fill");
            return Some(Outcome::Continue);
        }
        let which = TOOLS.iter().position(|(_, key)| *key == letter)?;
        Some(self.take_board_tool(which as u8))
    }

    /// Whether a letter pressed with a box picked out does what it does,
    /// rather than typing over its words: C colors, F lights what it is
    /// joined to, D votes on a note, Shift+S makes it another shape and
    /// Shift+L tidies the map it is in.
    fn keeps_key(&self, index: usize, letter: char) -> bool {
        let note = matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.form == shapes::FORM_STICKY);
        matches!(letter, 'c' | 'f' | 'S' | 'L') || (letter == 'd' && note) || (letter == 'D' && note)
    }
}

/// What C and a key colors with: 1 to 5 a line, Q W E R T a fill, X none.
fn color_key(key: Key) -> Option<Ctl> {
    let Key::Char(letter) = key else { return None };
    if let Some(at) = "12345".find(letter) {
        return Some(Ctl::Stroke(at as u8));
    }
    if let Some(at) = "qwert".find(letter) {
        return Some(Ctl::Fill(at as u8));
    }
    (letter == 'x').then_some(Ctl::NoFill)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn a_box_drawn_is_typed_into_and_the_box_tool_stays_for_the_next() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        assert_eq!(studio.editing, Some(0), "the caret is already in it");
        type_in(&mut studio, "Browser");
        assert_eq!(shape(&studio, 0).text, "Browser");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert!(studio.editing.is_none() && studio.board_tool() == BOX_TOOL, "Enter ends the typing; the box tool stays");
        // A new line with Shift+Enter, and an empty box simply stays empty.
        pull(&mut studio, (100.0, 300.0), (260.0, 372.0), Mods::default());
        type_in(&mut studio, "Two");
        press_key(&mut studio, Key::Enter, Mods { shift: true, ..Mods::default() });
        type_in(&mut studio, "lines");
        assert_eq!(shape(&studio, 1).text, "Two\nlines");
        // Starting the next box ends the typing; a click with no drag makes
        // one the size of the last two.
        tap(&mut studio, (600.0, 336.0));
        assert_eq!(studio.doc.layers.len(), 3);
        let (_, _, w, h) = shape(&studio, 2).frame();
        assert!((w - 160.0).abs() < 1.0 && (h - 72.0).abs() < 1.0, "made {w}x{h}");
        assert_eq!((studio.editing, shape(&studio, 1).text.as_str()), (Some(2), "Two\nlines"));
        type_in(&mut studio, "Third");
        press_key(&mut studio, Key::Escape, Mods::default());
        // V back to the pointer; and with the box tool and no run of boxes
        // yet, a click on empty board is back to the pointer too.
        press_key(&mut studio, Key::Char('v'), Mods::default());
        assert_eq!(studio.board_tool(), SELECT);
        let mut fresh = board();
        press_key(&mut fresh, Key::Char('o'), Mods::default());
        tap(&mut fresh, (300.0, 300.0));
        assert!(fresh.doc.layers.is_empty() && fresh.board_tool() == SELECT);
    }

    #[test]
    fn a_box_picked_out_is_typed_over_and_enter_types_into_it() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        type_in(&mut studio, "Queue");
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('v'), Mods::default());
        tap(&mut studio, (180.0, 136.0));
        assert_eq!(studio.selected, vec![0]);
        type_in(&mut studio, "Worker");
        assert_eq!(shape(&studio, 0).text, "Worker", "as a spreadsheet's cell");
        press_key(&mut studio, Key::Enter, Mods::default());
        press_key(&mut studio, Key::Enter, Mods::default());
        type_in(&mut studio, "s");
        assert_eq!(shape(&studio, 0).text, "Workers", "Enter types at the end");
        press_key(&mut studio, Key::Escape, Mods::default());
        // One step to undo the typing over.
        studio.undo();
        studio.undo();
        assert_eq!(shape(&studio, 0).text, "Queue");
    }

    #[test]
    fn what_the_bar_set_last_carries_on_to_the_next_box() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        // With nothing picked out, the bar sits over the tools.
        studio.selected.clear();
        let layout = studio.layout();
        let bar = layout.board_bar.expect("the bar");
        assert!(bar.bottom() <= layout.dock.y);
        let fill = studio.control_rect(Control::Board(Ctl::Fill(1))).unwrap().center();
        click(&mut studio, fill);
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        assert_eq!(shape(&studio, 0).fill, FILLS[1]);
        // Picked out, by it; C and a key colors it, and the next.
        press_key(&mut studio, Key::Escape, Mods::default());
        let layout = studio.layout();
        let (_, top, ..) = shape(&studio, 0).frame();
        assert!(layout.board_bar.unwrap().bottom() <= studio.to_window_page((0.0, top)).1);
        press_key(&mut studio, Key::Char('c'), Mods::default());
        press_key(&mut studio, Key::Char('2'), Mods::default());
        assert_eq!((shape(&studio, 0).color, studio.board.stroke), (STROKES[1], STROKES[1]));
        let look = studio.control_rect(Control::Board(Ctl::Look)).unwrap().center();
        click(&mut studio, look);
        assert!(!shape(&studio, 0).sketch && !studio.board.sketch);
        pull(&mut studio, (400.0, 100.0), (500.0, 200.0), Mods::default());
        let next = shape(&studio, 1);
        assert_eq!((next.color, next.fill, next.sketch), (STROKES[1], FILLS[1], false));
    }

    #[test]
    fn a_box_let_go_near_a_line_settles_on_it_without_guides_and_alt_puts_it_exactly() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        pull(&mut studio, (400.0, 300.0), (560.0, 372.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('v'), Mods::default());
        let drag_to = |studio: &mut Studio, to: (f32, f32), alt: bool| {
            let from = studio.to_window_page((480.0, 336.0));
            studio.pointer_moved(from.0, from.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            let to = studio.to_window_page(to);
            studio.pointer_moved(to.0, to.1 + 2, Mods { alt, ..Mods::default() });
            studio.pointer_moved(to.0, to.1, Mods { alt, ..Mods::default() });
            assert!(studio.guides.is_empty() && studio.gaps.is_empty(), "no guides drawn");
            studio.released();
        };
        drag_to(&mut studio, (480.0, 139.0), false);
        assert_eq!(shape(&studio, 1).frame().1, shape(&studio, 0).frame().1, "settled level with its neighbor");
        studio.undo();
        drag_to(&mut studio, (480.0, 139.0), true);
        assert_eq!(shape(&studio, 1).frame().1, shape(&studio, 0).frame().1 + 3.0, "Alt puts it where it is");
    }

    #[test]
    fn a_dot_on_a_side_pulls_out_an_arrow_to_a_new_box_or_to_another() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        pull(&mut studio, (600.0, 400.0), (760.0, 472.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('v'), Mods::default());
        studio.selected.clear();
        // The pointer over a box: a dot on each side.
        let over = studio.to_window_page((180.0, 136.0));
        studio.pointer_moved(over.0, over.1, Mods::default());
        assert_eq!(studio.board.dots_of, Some(0));
        let right = studio.dot_spots(0)[1];
        // A click on the right one: a box the usual gap to the right, joined
        // and typed into.
        click(&mut studio, right);
        assert_eq!(studio.doc.layers.len(), 4);
        let made = shape(&studio, 2);
        let source = shape(&studio, 0).frame();
        assert!((made.frame().0 - (source.0 + source.2 + dots::GAP_ACROSS)).abs() < 1.0 && made.frame().1 == source.1);
        let arrow = shape(&studio, 3);
        assert_eq!((arrow.stuck[0].unwrap().1, arrow.stuck[1].unwrap().0), (shapes::WHOLE, made.id));
        assert_eq!(studio.editing, Some(2));
        press_key(&mut studio, Key::Escape, Mods::default());
        // Dragged out onto the other box: the two joined.
        studio.pointer_moved(over.0, over.1, Mods::default());
        let bottom = studio.dot_spots(0)[2];
        studio.pointer_moved(bottom.0, bottom.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let onto = studio.to_window_page((680.0, 436.0));
        studio.pointer_moved(onto.0 - 30, onto.1 - 30, Mods::default());
        studio.pointer_moved(onto.0, onto.1, Mods::default());
        studio.released();
        let joined = shape(&studio, 4);
        assert_eq!((joined.stuck[0].unwrap().0, joined.stuck[1].unwrap().0), (shape(&studio, 0).id, shape(&studio, 1).id));
        // Its ends at the sides of each nearest the other, curving.
        assert!(joined.points[0].1 > 172.0 && joined.points[1].1 < 400.0 && joined.points.len() == 3, "from {:?} to {:?}", joined.points[0], joined.points[1]);
        // Dragged out onto empty board: a new box there.
        studio.pointer_moved(over.0, over.1, Mods::default());
        let left = studio.dot_spots(0)[3];
        studio.pointer_moved(left.0, left.1, Mods::default());
        studio.pressed(1, 300_000);
        let empty = studio.to_window_page((100.0, 500.0));
        studio.pointer_moved(empty.0, empty.1 - 40, Mods::default());
        studio.pointer_moved(empty.0, empty.1, Mods::default());
        studio.released();
        assert_eq!(studio.doc.layers.len(), 7);
        let (x, y, w, h) = shape(&studio, 5).frame();
        assert!(((x + w / 2.0) - 100.0).abs() < 1.0 && ((y + h / 2.0) - 500.0).abs() < 1.0);
    }

    #[test]
    fn a_double_click_on_an_arrow_writes_on_it() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('a'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (400.0, 100.0), Mods::default());
        assert_eq!(studio.board_tool(), SELECT);
        let on = studio.to_window_page((250.0, 100.0));
        studio.pointer_moved(on.0, on.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
        studio.pressed(1, 100_100);
        studio.released();
        assert_eq!(studio.editing, Some(0));
        type_in(&mut studio, "yes");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!(shape(&studio, 0).text, "yes");
        // On empty board, a label.
        let empty = studio.to_window_page((250.0, 300.0));
        studio.pointer_moved(empty.0, empty.1, Mods::default());
        studio.pressed(1, 200_000);
        studio.released();
        studio.pressed(1, 200_100);
        studio.released();
        type_in(&mut studio, "Note");
        assert_eq!((shape(&studio, 1).tool, shape(&studio, 1).text.as_str(), shape(&studio, 1).face), (Tool::Text, "Note", LETTERING));
    }
}
