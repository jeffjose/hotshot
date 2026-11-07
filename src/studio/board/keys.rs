//! The sheet of a board's keys, opened by ? at the bottom left or by
//! typing ?: what each does, by group, over the board dimmed; Esc, ? or a
//! click off it closes it. Its tools are read from `TOOLS`, which is what
//! a letter takes up; the rest are as `Studio::key` and `board_key` have
//! them, each tried by the tests below.

use super::*;

/// The groups of the sheet, each with what a key does and the key.
const EDITING: [(&str, &str); 18] = [
    ("Type into what is picked out", "Enter"),
    ("Type over a box picked out", "A letter"),
    ("A new line in a box", "Shift Enter"),
    ("Done typing", "Esc"),
    ("Undo", "Ctrl Z"),
    ("Redo", "Ctrl Shift Z"),
    ("Go back through the history", "Ctrl Alt Z"),
    ("Copy, paste", "Ctrl C, Ctrl V"),
    ("Duplicate", "Ctrl D"),
    ("Pick out everything", "Ctrl A"),
    ("Delete", "Delete"),
    ("Nudge, or by 10", "Arrows, Shift"),
    ("Bring forward, send back", "Ctrl ], Ctrl ["),
    ("Color: a line, a fill, none", "C, then 1\u{2013}5, Q\u{2013}T, X"),
    ("Words larger, smaller", "Ctrl Shift >, <"),
    ("Make a box another shape", "Shift S"),
    ("Gather notes into a box", "G"),
    ("A vote on a note, one fewer", "D, Shift D"),
];

const ARROWS: [(&str, &str); 8] = [
    ("Pull an arrow out of a box", "Drag a side's dot"),
    ("A box joined after it", "Tab"),
    ("Back to the box before", "Shift Tab"),
    ("The next box of a map", "Enter, typing"),
    ("Keep what is guessed", "Tab"),
    ("Tidy the map it is in", "Shift L"),
    ("Light what it is joined to", "F"),
    ("Copy with what is in it", "Alt drag"),
];

const VIEW: [(&str, &str); 14] = [
    ("Move the board", "Space drag"),
    ("Zoom about the pointer", "Ctrl wheel"),
    ("Zoom in, out", "Ctrl +, Ctrl \u{2212}"),
    ("Zoom to fit", "Ctrl 0"),
    ("Zoom to 100%", "Ctrl 1"),
    ("Zoom to what is picked out", "Shift 2"),
    ("Search everything", "Ctrl K"),
    ("Export, as last chosen", "Ctrl E"),
    ("Export options", "Ctrl Shift E"),
    ("Design or Board", "Ctrl M"),
    ("Hide everything but the board", "Ctrl \\"),
    ("The left panel, the right", "Ctrl Shift \\, Ctrl Alt \\"),
    ("Presenting: next, back, out", "Space, \u{2190}, Esc"),
    ("These keys", "?"),
];

/// The sheet's rows are so high, its columns so wide and so far apart,
/// its edge so far in.
const ROW: i32 = 24;
const COLUMN: i32 = 300;
const BETWEEN: i32 = 28;
const EDGE: i32 = 24;
const HEADING: i32 = 34;

/// The groups, in the columns they are in.
fn groups() -> Vec<Vec<(&'static str, Vec<(String, String)>)>> {
    let own = |rows: &[(&str, &str)]| rows.iter().map(|(says, key)| ((*says).to_owned(), (*key).to_owned())).collect::<Vec<_>>();
    let mut tools: Vec<(String, String)> = TOOLS.iter().map(|(name, key)| ((*name).to_owned(), key.to_ascii_uppercase().to_string())).collect();
    tools.extend(own(&[("Laser, while it is held", "Hold L"), ("Hand, while it is held", "Hold Space")]));
    vec![vec![("Tools", tools)], vec![("Editing", own(&EDITING))], vec![("Arrows", own(&ARROWS)), ("View", own(&VIEW))]]
}

impl Studio {
    /// The sheet, in the middle of the window, as high as its longest
    /// column; the whole window round it, which a click closes it on.
    pub(in crate::studio) fn layout_keys(&mut self, layout: &mut Layout) {
        if !self.board.keys {
            return;
        }
        let columns = groups();
        let tallest = columns.iter().map(|column| column.iter().map(|(_, rows)| HEADING + rows.len() as i32 * ROW).sum::<i32>() + (column.len() as i32 - 1) * 14).max().unwrap_or(0);
        let (w, h) = (columns.len() as i32 * COLUMN - BETWEEN + 2 * EDGE, tallest + 2 * EDGE + 44);
        let sheet = Rect::new((self.width - w) / 2, ((self.height - h) / 2).max(INSET), w, h);
        layout.controls.insert(0, (Rect::new(0, 0, self.width, self.height), Control::Board(Ctl::Scrim)));
        layout.controls.insert(0, (sheet, Control::Board(Ctl::Sheet)));
    }

    /// `?` typed, or a key while the sheet is open: Esc and ? close it, a
    /// key with Ctrl closes it and does what it does, and the rest wait.
    /// Whether the key was taken.
    pub(in crate::studio) fn keys_key(&mut self, key: Key, mods: Mods) -> bool {
        let asked = key == Key::Char('?') && !mods.ctrl && !mods.alt;
        if !self.board.keys {
            let typing = self.editing.is_some() || self.renaming.is_some() || self.typing.is_some() || self.board.px_typed.is_some() || self.search.is_some() || self.menu.is_some() || self.library.is_some();
            if asked && !typing {
                self.board.keys = true;
                self.dirty = true;
                return true;
            }
            return false;
        }
        self.dirty = true;
        if mods.ctrl {
            self.board.keys = false;
            return false;
        }
        if asked || key == Key::Escape {
            self.board.keys = false;
        }
        true
    }

    /// The sheet, over the board dimmed and all that is over it.
    pub(in crate::studio) fn draw_keys(&mut self, canvas: &mut Canvas, layout: &Layout) {
        let Some(&(sheet, _)) = layout.controls.iter().find(|(_, control)| *control == Control::Board(Ctl::Sheet)) else { return };
        canvas.round_fill(0, 0, self.width, self.height, 0, 0x000000, 110);
        soft_shadow(canvas, sheet, 14);
        canvas.round_fill(sheet.x, sheet.y, sheet.w, sheet.h, 14, PANEL_COLOR, 256);
        canvas.round_outline(sheet.x, sheet.y, sheet.w, sheet.h, 14, 1, EDGE_COLOR);
        self.text.draw(canvas, sheet.x + EDGE, sheet.y + EDGE - 4, 16, TEXT_COLOR, "Keyboard shortcuts");
        let close = "Esc to close";
        let wide = self.text.width(close, 11);
        self.text.draw(canvas, sheet.right() - EDGE - wide, sheet.y + EDGE, 11, DIM_TEXT_COLOR, close);
        for (at, column) in groups().into_iter().enumerate() {
            let left = sheet.x + EDGE + at as i32 * COLUMN;
            let mut y = sheet.y + EDGE + 44;
            for (heading, rows) in column {
                self.text.draw(canvas, left, y, 11, ACCENT_TEXT, &heading.to_uppercase());
                y += HEADING;
                for (says, key) in rows {
                    let middle = y + ROW / 2 - 4;
                    self.text.draw(canvas, left, middle - Text::line_height(12) / 2, 12, TEXT_COLOR, &says);
                    // The key in a cap of its own at the right of the column.
                    let cap = self.text.width(&key, 11) + 12;
                    let right = left + COLUMN - BETWEEN;
                    canvas.round_fill(right - cap, middle - 10, cap, 20, 5, FIELD_COLOR, 256);
                    self.text.draw(canvas, right - cap + 6, middle - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &key);
                    y += ROW;
                }
                y += 14;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn question_mark_opens_the_sheet_of_keys_and_esc_or_a_click_off_it_closes_it() {
        let mut studio = board();
        studio.run_menu(crate::studio::menu::MenuAct::Board(crate::studio::board::Act::Keys));
        assert!(studio.board.keys);
        let sheet = studio.control_rect(Control::Board(Ctl::Sheet)).unwrap();
        assert!(sheet.x > 0 && sheet.right() < studio.width && sheet.bottom() <= studio.height, "{sheet:?}");
        assert!(((sheet.center().0 - studio.width / 2).abs()) <= 1, "in the middle");
        // A click on it leaves it; a key waits; Esc closes it.
        click(&mut studio, (sheet.x + 30, sheet.y + 80));
        assert!(studio.board.keys && studio.doc.layers.is_empty());
        press_key(&mut studio, Key::Char('r'), Mods::default());
        assert!(studio.board.keys && studio.board_tool() == SELECT, "a letter takes up no tool under it");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(!studio.board.keys);
        // ? typed opens it, and a click off it closes it and does nothing
        // else: nothing drawn, nothing picked out.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        press_key(&mut studio, Key::Char('?'), Mods { shift: true, ..Mods::default() });
        assert!(studio.board.keys);
        click(&mut studio, (40, 300));
        assert!(!studio.board.keys && studio.doc.layers.is_empty());
        // ? again closes it; and drawn, it is over everything.
        press_key(&mut studio, Key::Char('?'), Mods { shift: true, ..Mods::default() });
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let inside = studio.control_rect(Control::Board(Ctl::Sheet)).unwrap();
        assert_eq!(out[((inside.y + inside.h / 2) * studio.width + inside.x + 4) as usize] & 0xFF_FFFF, PANEL_COLOR);
        press_key(&mut studio, Key::Char('?'), Mods { shift: true, ..Mods::default() });
        assert!(!studio.board.keys);
    }

    #[test]
    fn the_sheet_says_each_tool_by_the_key_that_takes_it_up() {
        let tools = &groups()[0][0].1;
        for (at, (name, key)) in TOOLS.iter().enumerate() {
            assert!(tools.contains(&((*name).to_owned(), key.to_ascii_uppercase().to_string())));
            let mut studio = board();
            press_key(&mut studio, Key::Char(*key), Mods::default());
            // L is the laser while held: a tap of it, once let go.
            if let Some(laser) = &mut studio.board.laser {
                laser.since -= std::time::Duration::from_millis(300);
                laser.let_go = Some(laser.since + std::time::Duration::from_millis(100));
                studio.board_tick();
            }
            let which = at as u8;
            if which != PICTURE {
                assert_eq!(studio.board_tool(), which, "{name} on {key}");
            }
        }
    }

    #[test]
    fn the_keys_the_sheet_says_do_what_it_says() {
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        let shift = Mods { shift: true, ..Mods::default() };
        let mut studio = board();
        // Ctrl 1, Ctrl +, Ctrl −.
        studio.zoom = 0.5;
        press_key(&mut studio, Key::Char('1'), ctrl);
        assert!((studio.zoom - 1.0).abs() < 1e-4);
        press_key(&mut studio, Key::Char('='), ctrl);
        assert!(studio.zoom > 1.2);
        press_key(&mut studio, Key::Char('-'), ctrl);
        assert!((studio.zoom - 1.0).abs() < 1e-4);
        // A box: Enter types into it, Esc is done, Ctrl D duplicates, Shift
        // S makes it another shape, Delete takes it away, Ctrl Z undoes.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.editing.is_none() && studio.selected == vec![0]);
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!(studio.editing, Some(0));
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('d'), ctrl);
        assert_eq!(studio.doc.layers.len(), 2);
        let was = shape(&studio, 1);
        press_key(&mut studio, Key::Char('S'), shift);
        let now = shape(&studio, 1);
        assert_ne!((now.tool, now.form), (was.tool, was.form));
        press_key(&mut studio, Key::Delete, Mods::default());
        assert_eq!(studio.doc.layers.len(), 1);
        press_key(&mut studio, Key::Char('z'), ctrl);
        assert_eq!(studio.doc.layers.len(), 2);
        // Tab, a box joined after it.
        studio.selected = vec![0];
        studio.board.typed = None;
        press_key(&mut studio, Key::Tab, Mods::default());
        assert_eq!(studio.links().len(), 1);
        // F lights it.
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected = vec![0];
        studio.board.typed = Some(0);
        press_key(&mut studio, Key::Char('f'), Mods::default());
        assert!(studio.board.focus.is_some());
    }
}
