//! What a board's box holds besides words: a block of code, put down by
//! Ctrl+` or by pasting what reads as code, typed into as code is, its
//! indent kept and Tab four spaces; a table, put down from the right
//! click or by pasting a spreadsheet's rows, Tab and Enter going from cell
//! to cell; and the stamps a box is given and taken off again.

use super::*;

/// A new table's rows and columns, its first row its heading.
const TABLE: (usize, usize) = (3, 3);

/// How far Tab indents a line of code.
const INDENT: usize = 4;

/// The row and the column of a table's cell a place in its words is in.
fn cell_at(words: &str, at: usize) -> (usize, usize) {
    let before = &words[..at.min(words.len())];
    let row = before.matches('\n').count();
    let line = before.rsplit('\n').next().unwrap_or("");
    (row, line.matches('\t').count())
}

/// Where a cell of a table begins and ends in its words, if it has it.
fn cell_span(words: &str, row: usize, column: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    for (at, line) in words.split('\n').enumerate() {
        if at == row {
            let mut from = start;
            for (index, cell) in line.split('\t').enumerate() {
                if index == column {
                    return Some((from, from + cell.len()));
                }
                from += cell.len() + 1;
            }
            return None;
        }
        start += line.len() + 1;
    }
    None
}

/// A table's rows each given as many cells as the widest has.
fn padded(words: &str) -> String {
    let columns = words.split('\n').map(|row| row.split('\t').count()).max().unwrap_or(1);
    words.split('\n').map(|row| {
        let have = row.split('\t').count();
        format!("{row}{}", "\t".repeat(columns - have))
    }).collect::<Vec<_>>().join("\n")
}

/// The leading spaces of a line of code.
fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

impl Studio {
    /// Whether a layer is a block of code or a table.
    pub(in crate::studio) fn is_block(&self, index: usize) -> bool {
        matches!(self.doc.layers.get(index), Some(Layer { kind: Kind::Shape(shape), .. }) if shape.tool == Tool::Rect && matches!(shape.form, shapes::FORM_CODE | shapes::FORM_GRID))
    }

    /// A block of code or a table put down, as the board would draw a box:
    /// its line in the board's ink, its words in its font and as large to
    /// the eye as at 100%; about a point of the board, or the middle of
    /// what is seen; typed into at once. One step to undo.
    pub(in crate::studio) fn put_block(&mut self, form: u8, words: String, at: Option<(f32, f32)>) -> usize {
        self.commit_text();
        self.checkpoint();
        let at = at.unwrap_or_else(|| self.to_doc(self.open_area().center()));
        let mut shape = Shape::new(Tool::Rect, INK, LINE, at);
        self.board_dress(&mut shape);
        (shape.form, shape.text, shape.base, shape.nominal) = (form, words, (0.0, 0.0), 0);
        match form {
            shapes::FORM_CODE => {
                // Dark whatever the board's fill, a little off the paper.
                (shape.face, shape.filled, shape.fill, shape.border) = (shapes::MONO, true, 0x282C34, true);
                shape.shadow = Some(frame::Layer { x: 0, y: 4, blur: 14, spread: 0, color: 0x3000_0000 });
            }
            _ => {
                // Its first row in the tint of its line.
                let tint = SWATCHES.iter().find(|(line, _)| *line == shape.color).map_or(SWATCHES[7].1, |(_, tint)| *tint);
                (shape.filled, shape.fill, shape.border, shape.hatch) = (true, self.board.fill.filter(|&fill| fill != 0xFFFFFF).unwrap_or(tint), true, false);
            }
        }
        let (w, h) = shape.label_needs(&mut self.text, (0.0, 0.0), f32::MAX);
        shape.points = vec![(at.0 - w / 2.0, at.1 - h / 2.0), (at.0 + w / 2.0, at.1 + h / 2.0)];
        shape.touch();
        self.doc.layers.push(Layer::shape(shape));
        let index = self.doc.layers.len() - 1;
        (self.tool, self.selected) = (Tool::Select, vec![index]);
        self.edit_text(index);
        // The caret where the typing begins: a table's first cell, the end
        // of a block of code.
        if form == shapes::FORM_GRID {
            self.board.caret = cell_span(&self.block_words(index), 0, 0).map(|(_, end)| end);
        }
        self.changed();
        index
    }

    /// The words of a layer, if it is a shape.
    pub(in crate::studio) fn block_words(&self, index: usize) -> String {
        match self.doc.layers.get(index).map(|layer| &layer.kind) {
            Some(Kind::Shape(shape)) => shape.text.clone(),
            _ => String::new(),
        }
    }

    /// Ctrl+` or Insert code block: an empty block of code.
    pub(in crate::studio) fn insert_code(&mut self, at: Option<(f32, f32)>) {
        self.put_block(shapes::FORM_CODE, "\n".to_owned(), at);
        self.say("Code block: type, Tab indents, Esc when done");
    }

    /// Insert table: three rows of three empty cells, the first a heading.
    pub(in crate::studio) fn insert_table(&mut self, at: Option<(f32, f32)>) {
        let row = vec![""; TABLE.1].join("\t");
        self.put_block(shapes::FORM_GRID, vec![row; TABLE.0].join("\n"), at);
        self.say("Table: Tab to the next cell, Enter the next row");
    }

    /// Words pasted on a board, not into anything: a spreadsheet's rows a
    /// table, and what reads as code a block of it. Whether it was either.
    pub(in crate::studio) fn paste_block(&mut self, words: &str) -> bool {
        if shapes::looks_tabular(words) {
            let rows = shapes::tabular_rows(words);
            let cells: Vec<String> = rows.iter().map(|row| row.iter().map(|cell| cell.replace(['\t', '\n'], " ")).collect::<Vec<_>>().join("\t")).collect();
            let index = self.put_block(shapes::FORM_GRID, padded(&cells.join("\n")), None);
            self.commit_text();
            self.selected = vec![index];
            self.say(format!("Pasted as a table of {} rows", rows.len()));
            return true;
        }
        if shapes::looks_like_code(words) {
            let code = words.trim_end_matches(['\n', '\r']).replace('\r', "").replace('\t', &" ".repeat(INDENT));
            let index = self.put_block(shapes::FORM_CODE, format!("\n{code}"), None);
            self.commit_text();
            self.selected = vec![index];
            self.say("Pasted as a code block");
            return true;
        }
        false
    }

    /// Words pasted while a block of code or a table is typed into: in at
    /// the caret, over what is picked out, a tab in code four spaces.
    /// Whether it was so.
    pub(in crate::studio) fn paste_into_block(&mut self, words: &str) -> bool {
        let Some(index) = self.editing.filter(|&index| self.is_block(index)) else { return false };
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return false };
        let words = match shape.form {
            shapes::FORM_CODE => words.replace('\r', "").replace('\t', &" ".repeat(INDENT)),
            _ => words.replace('\r', ""),
        };
        self.put_at_caret(index, &words);
        if shapes::FORM_GRID == self.form_of(index) {
            let padded = padded(&self.block_words(index));
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                shape.text = padded;
                shape.touch();
            }
            self.words_changed(index);
        }
        true
    }

    fn form_of(&self, index: usize) -> u8 {
        match self.doc.layers.get(index).map(|layer| &layer.kind) {
            Some(Kind::Shape(shape)) => shape.form,
            _ => 0,
        }
    }

    /// Some words in at the caret of what is typed into, over what is
    /// picked out of it, the caret after them.
    fn put_at_caret(&mut self, index: usize, with: &str) {
        let words = self.block_words(index);
        let caret = self.caret_at(&words);
        let anchor = self.board.anchor.map(|anchor| anchor.min(words.len())).filter(|&anchor| words.is_char_boundary(anchor)).unwrap_or(caret);
        let (from, to) = (anchor.min(caret), anchor.max(caret));
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.text.replace_range(from..to, with);
            shape.touch();
        }
        (self.board.caret, self.board.anchor) = (Some(from + with.len()), None);
        self.words_changed(index);
    }

    /// The caret put at a place in the words typed into.
    fn caret_to(&mut self, at: usize) {
        (self.board.caret, self.board.anchor) = (Some(at), None);
        self.dirty = true;
    }

    /// A key while a block of code or a table is typed into, before a
    /// box's: in code, Enter a new line as indented as the last and Tab
    /// four spaces; in a table, Tab and Shift+Tab to the next cell and the
    /// last, Tab past the end a row more, Enter and Shift+Enter a row down
    /// and up, and nothing that would join two cells. Whether it was taken.
    pub(in crate::studio) fn block_key(&mut self, index: usize, key: Key, mods: Mods) -> bool {
        if !self.is_block(index) || mods.ctrl || mods.alt {
            return false;
        }
        let words = self.block_words(index);
        let caret = self.caret_at(&words);
        let picked = self.board.anchor.is_some_and(|anchor| anchor != caret);
        match self.form_of(index) {
            shapes::FORM_CODE => {
                let (name, _) = shapes::code_parts(&words);
                let code_from = name.len() + 1;
                let line_start = words[..caret].rfind('\n').map_or(0, |at| at + 1);
                let line = &words[line_start..caret];
                match key {
                    // In the name, Enter goes down into the code.
                    Key::Enter if caret <= name.len() => {
                        if !words.contains('\n') {
                            let end = words.len();
                            self.caret_to(end);
                            self.put_at_caret(index, "\n");
                        }
                        self.caret_to(code_from);
                    }
                    Key::Enter => {
                        let deeper = line.trim_end().ends_with(['{', '(', '[', ':']);
                        let indent = format!("{}{}", indent_of(line), if deeper { " ".repeat(INDENT) } else { String::new() });
                        self.put_at_caret(index, &format!("\n{indent}"));
                    }
                    Key::Tab if mods.shift => {
                        let line_end = words[line_start..].find('\n').map_or(words.len(), |at| line_start + at);
                        let whole = &words[line_start..line_end];
                        let gone = whole.len() - whole.trim_start_matches(' ').len();
                        let gone = gone.min(INDENT);
                        if gone > 0 && line_start >= code_from {
                            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                                shape.text.replace_range(line_start..line_start + gone, "");
                                shape.touch();
                            }
                            self.caret_to(caret.saturating_sub(gone).max(line_start));
                            self.words_changed(index);
                        }
                    }
                    Key::Tab if caret >= code_from => {
                        let column = line.chars().count();
                        self.put_at_caret(index, &" ".repeat(INDENT - column % INDENT));
                    }
                    Key::Tab => self.caret_to(code_from),
                    // The name and the code are kept apart.
                    Key::Backspace if caret == code_from && !picked => {}
                    Key::Delete if caret == name.len() && !picked => {}
                    Key::Up | Key::Down => self.caret_line(&words, caret, key == Key::Down),
                    _ => return false,
                }
                true
            }
            _ => {
                let (row, column) = cell_at(&words, caret);
                let rows = words.split('\n').count();
                let columns = words.split('\n').map(|line| line.split('\t').count()).max().unwrap_or(1);
                let (cell_start, cell_end) = cell_span(&words, row, column).unwrap_or((caret, caret));
                let go = |studio: &mut Studio, row: usize, column: usize| {
                    let words = studio.block_words(index);
                    if let Some((_, end)) = cell_span(&words, row, column) {
                        studio.caret_to(end);
                    }
                };
                match key {
                    Key::Tab if mods.shift => match (row, column) {
                        (0, 0) => {}
                        (row, 0) => go(self, row - 1, columns - 1),
                        (row, column) => go(self, row, column - 1),
                    },
                    Key::Tab => match (row + 1 == rows, column + 1 >= columns) {
                        // Past the last cell: a row more, and its first cell.
                        (true, true) => {
                            let more = format!("{}\n{}", padded(&words), vec![""; columns].join("\t"));
                            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                                shape.text = more;
                                shape.touch();
                            }
                            go(self, row + 1, 0);
                            self.words_changed(index);
                        }
                        (false, true) => go(self, row + 1, 0),
                        _ => {
                            // A short row is given the cell it lacks.
                            if cell_span(&words, row, column + 1).is_none() {
                                let full = padded(&words);
                                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                                    shape.text = full;
                                    shape.touch();
                                }
                                self.words_changed(index);
                            }
                            go(self, row, column + 1);
                        }
                    },
                    // Enter a row down, done at the last; Shift+Enter up.
                    Key::Enter if mods.shift => {
                        if row > 0 {
                            go(self, row - 1, column);
                        }
                    }
                    Key::Enter if row + 1 == rows => {
                        self.selected = vec![index];
                        self.commit_text();
                    }
                    Key::Enter | Key::Down => {
                        if row + 1 < rows {
                            go(self, row + 1, column);
                        }
                    }
                    Key::Up => {
                        if row > 0 {
                            go(self, row - 1, column);
                        }
                    }
                    // What would join two cells does nothing.
                    Key::Backspace if caret == cell_start && !picked => {}
                    Key::Delete if caret == cell_end && !picked => {}
                    _ => return false,
                }
                true
            }
        }
    }

    /// The caret a line up or down in a block of code, as far along it as
    /// it can be.
    fn caret_line(&mut self, words: &str, caret: usize, down: bool) {
        let start = words[..caret].rfind('\n').map_or(0, |at| at + 1);
        let along = words[start..caret].chars().count();
        let target = match down {
            true => words[caret..].find('\n').map(|at| caret + at + 1),
            false => (start > 0).then(|| words[..start - 1].rfind('\n').map_or(0, |at| at + 1)),
        };
        let Some(from) = target else { return };
        let end = words[from..].find('\n').map_or(words.len(), |at| from + at);
        let at = words[from..end].char_indices().nth(along).map_or(end, |(offset, _)| from + offset);
        self.caret_to(at);
    }

    /// A block of code or a table made as large as its lines and cells
    /// need, or as large as it was made where that is more; `by_hand`, its
    /// size as it was just made kept as the least it is.
    pub(in crate::studio) fn fit_block(&mut self, index: usize, by_hand: bool) {
        let before = self.box_frames();
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return };
        let (x, y, w, h) = shape.frame();
        let base = if by_hand { (w, h) } else { shape.base };
        let (nw, nh) = shape.label_needs(&mut self.text, base, f32::MAX);
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.base = base;
            if (nw - w).abs() > 0.5 || (nh - h).abs() > 0.5 {
                shape.points = vec![(x, y), (x + nw, y + nh)];
            }
            shape.touch();
        }
        self.changed();
        self.make_room_after(&before);
    }

    /// A stamp put on each box picked out, or taken off them all where each
    /// has it: one step to undo.
    pub(in crate::studio) fn stamp(&mut self, kind: u8) {
        let boxes: Vec<usize> = self.chosen().into_iter().filter(|&index| self.is_box(index)).collect();
        if boxes.is_empty() {
            self.say("Pick out a box to stamp");
            return;
        }
        let all = boxes.iter().all(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.stamps.contains(&kind)));
        self.restyle_shapes(|shape| {
            if !matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                return;
            }
            match all {
                true => shape.stamps.retain(|&had| had != kind),
                false if !shape.stamps.contains(&kind) => shape.stamps.push(kind),
                false => {}
            }
            shape.touch();
        });
        let name = shapes::STAMPS[usize::from(kind).min(shapes::STAMPS.len() - 1)].0;
        self.say(if all { format!("Stamp taken off: {name}") } else { format!("Stamped: {name}") });
    }

    /// Whether each box picked out has a stamp, for a list to show it on.
    pub(in crate::studio) fn stamped(&self, kind: u8) -> bool {
        let boxes: Vec<usize> = self.chosen().into_iter().filter(|&index| self.is_box(index)).collect();
        !boxes.is_empty() && boxes.iter().all(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.stamps.contains(&kind)))
    }

    /// Shift+E with boxes picked out: the stamps, as a list by the top
    /// right of the first.
    pub(in crate::studio) fn open_stamps(&mut self) {
        let Some(&first) = self.chosen().iter().find(|&&index| self.is_box(index)) else { return };
        let (x, y, w, _) = self.box_frame(first);
        let at = self.to_window((x + w, y));
        self.menu = Some(MenuState { kind: MenuKind::Stamps, at: (at.0 + 8, at.1), sub: None });
        self.menu_row = Some((0, 0));
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn ctrl() -> Mods {
        Mods { ctrl: true, ..Mods::default() }
    }

    /// A line of code's runs, each its words and its color.
    fn colors(line: &str, open: &mut bool) -> Vec<(String, u32)> {
        shapes::code_runs(line, open).into_iter().map(|(start, end, color)| (line[start..end].to_owned(), color)).collect()
    }

    #[test]
    fn ctrl_backtick_puts_down_a_block_of_code_typed_into_as_code_is() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('`'), ctrl());
        let index = studio.doc.layers.len() - 1;
        assert_eq!((shape(&studio, index).form, shape(&studio, index).face, studio.editing), (shapes::FORM_CODE, shapes::MONO, Some(index)));
        let (_, _, narrow, low) = shape(&studio, index).frame();
        // Enter keeps the indent, and goes in once more after a brace; Tab
        // is spaces to the next four.
        type_in(&mut studio, "fn main() {");
        press_key(&mut studio, Key::Enter, Mods::default());
        type_in(&mut studio, "let x = 1;");
        press_key(&mut studio, Key::Enter, Mods::default());
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "y");
        assert_eq!(shape(&studio, index).text, "\nfn main() {\n    let x = 1;\n        y");
        assert_eq!(studio.editing, Some(index), "Enter is a new line, not done");
        // Shift+Tab takes an indent off the line.
        press_key(&mut studio, Key::Tab, Mods { shift: true, ..Mods::default() });
        assert_eq!(shape(&studio, index).text, "\nfn main() {\n    let x = 1;\n    y");
        // Taller for each line, never wrapped, and wider for a long one.
        let (_, _, w, h) = shape(&studio, index).frame();
        assert!((h - low - 2.0 * Text::line_height(WORDS_PX) as f32).abs() < 1.0 && (w - narrow).abs() < 1.0, "{w}x{h} from {narrow}x{low}");
        type_in(&mut studio, " + some_long_function_name(with, many, arguments);");
        let (_, _, wide, high) = shape(&studio, index).frame();
        assert!(wide > w + 100.0 && (high - h).abs() < 1.0, "{wide}x{high}");
        // Up goes to the line before, as far along as it can.
        press_key(&mut studio, Key::Up, Mods::default());
        let words = shape(&studio, index).text;
        assert_eq!(studio.board.caret.map(|at| &words[..at]), Some("\nfn main() {\n    let x = 1;"));
        // The name of its file is its first line, typed in its strip.
        let name = studio.words_spot(&shape(&studio, index), 0).unwrap();
        let (x, y, _, _) = shape(&studio, index).frame();
        assert!(name.1 < y + 12.0 && name.0 > x + 40.0, "in the strip, past the dots: {name:?}");
        studio.board.caret = Some(0);
        type_in(&mut studio, "main.rs");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert!(shape(&studio, index).text.starts_with("main.rs\nfn main"));
        assert_eq!(studio.board.caret, Some("main.rs\n".len()), "Enter in the name goes down to the code");
        press_key(&mut studio, Key::Backspace, Mods::default());
        assert!(shape(&studio, index).text.starts_with("main.rs\nfn"), "the name and the code kept apart");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.editing.is_none());
        // Kept in the file, and a font chosen leaves it monospaced.
        studio.selected = vec![index];
        studio.set_font(1);
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        let kept = match &back[0].loose.layers[index].kind {
            Kind::Shape(shape) => shape.clone(),
            Kind::Picture(_) => panic!("a shape"),
        };
        assert_eq!((kept.form, kept.text.clone(), kept.face), (shapes::FORM_CODE, shape(&studio, index).text, shapes::MONO));
        // As an SVG, its words light on its dark.
        let layers = studio.doc.layers.clone();
        let svg = studio.board_svg(&layers, (0.0, 0.0, 1400.0, 900.0));
        let line = svg.lines().find(|line| line.contains(">main.rs</text>")).expect("its name");
        assert!(line.contains("fill=\"#FFFFFF\""), "{line}");
    }

    #[test]
    fn a_line_of_code_is_colored_by_what_each_word_is() {
        let mut open = false;
        let of = |runs: &[(String, u32)], word: &str| runs.iter().find(|(text, _)| text == word).map(|(_, color)| *color);
        let runs = colors("let name = \"Ada\"; // who", &mut open);
        assert_eq!(of(&runs, "let"), Some(0xC678DD), "a keyword");
        assert_eq!(of(&runs, "\"Ada\""), Some(0x98C379), "a string");
        assert_eq!(of(&runs, "// who"), Some(0x7F848E), "a comment");
        assert_eq!(of(&runs, "name"), Some(0xABB2BF), "plain");
        let runs = colors("print(len(items), 42, 3.5)", &mut open);
        assert_eq!((of(&runs, "print"), of(&runs, "42"), of(&runs, "3.5")), (Some(0x61AFEF), Some(0xD19A66), Some(0xD19A66)));
        assert_eq!(of(&colors("# a note", &mut open), "# a note"), Some(0x7F848E));
        assert_eq!(of(&colors("#[derive(Debug)]", &mut open), "derive"), Some(0x61AFEF), "an attribute is no comment");
        assert_eq!(of(&colors("fn f<'a>(x: &'a str) -> Vec<User>", &mut open), "User"), Some(0xE5C07B), "a type, past a lifetime");
        // A comment begun on one line goes on into the next.
        colors("x /* begun", &mut open);
        assert!(open);
        let next = colors("still */ y", &mut open);
        assert_eq!((next[0].0.as_str(), next[0].1, open), ("still */", 0x7F848E, false));
    }

    #[test]
    fn pasted_code_is_a_block_of_code_and_pasted_rows_a_table() {
        let mut studio = board();
        studio.pasted(None, Some("def greet(name):\n\tprint(f\"hi {name}\")\n\treturn name\n".into()));
        let code = shape(&studio, 0);
        assert_eq!((code.form, code.text.as_str()), (shapes::FORM_CODE, "\ndef greet(name):\n    print(f\"hi {name}\")\n    return name"));
        assert_eq!((studio.editing, studio.selected.clone()), (None, vec![0]));
        // A spreadsheet's rows, by tabs; a CSV's by commas, a quoted one whole.
        studio.pasted(None, Some("Name\tRole\nAda\tEngineer\nGrace\tAdmiral\n".into()));
        assert_eq!((shape(&studio, 1).form, shape(&studio, 1).text.as_str()), (shapes::FORM_GRID, "Name\tRole\nAda\tEngineer\nGrace\tAdmiral"));
        studio.pasted(None, Some("city,people\n\"Paris, France\",2100000\nOslo,700000".into()));
        assert_eq!(shape(&studio, 2).text, "city\tpeople\nParis, France\t2100000\nOslo\t700000");
        // Words that are neither are words.
        studio.pasted(None, Some("Remember the milk\nand the eggs".into()));
        assert_eq!(shape(&studio, 3).tool, Tool::Text);
        // One step each to undo.
        studio.undo();
        studio.undo();
        assert_eq!(studio.doc.layers.len(), 2);
        assert!(!shapes::looks_like_code("Dear Ada,\nthanks for the notes.") && shapes::looks_like_code("const a = 1;\nconst b = 2;"));
    }

    #[test]
    fn a_table_is_typed_into_cell_by_cell_its_columns_as_wide_as_their_widest() {
        let mut studio = board();
        studio.run_menu(crate::studio::menu::MenuAct::Board(Act::InsertTable));
        let index = studio.doc.layers.len() - 1;
        assert_eq!((shape(&studio, index).form, shape(&studio, index).text.as_str(), studio.editing), (shapes::FORM_GRID, "\t\t\n\t\t\n\t\t", Some(index)));
        // Tab to the next cell, Shift+Tab back, Enter a row down.
        type_in(&mut studio, "Name");
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Role");
        press_key(&mut studio, Key::Tab, Mods::default());
        press_key(&mut studio, Key::Tab, Mods { shift: true, ..Mods::default() });
        type_in(&mut studio, "s");
        press_key(&mut studio, Key::Enter, Mods::default());
        type_in(&mut studio, "Engineer");
        assert_eq!(shape(&studio, index).text, "Name\tRoles\t\n\tEngineer\t\n\t\t");
        // Past the last cell, Tab adds a row and goes to its first.
        studio.board.caret = Some(shape(&studio, index).text.len());
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Ada");
        assert_eq!(shape(&studio, index).text, "Name\tRoles\t\n\tEngineer\t\n\t\t\nAda\t\t");
        // Backspace at a cell's start joins nothing.
        studio.board.caret = cell_span(&shape(&studio, index).text, 1, 1).map(|(start, _)| start);
        press_key(&mut studio, Key::Backspace, Mods::default());
        assert_eq!(shape(&studio, index).text.split('\n').nth(1), Some("\tEngineer\t"));
        // Its middle column as wide as Engineer, the others as wide as least.
        let table = shape(&studio, index);
        let cells = table.grid_lines(&mut studio.text);
        let (left, middle, right) = (cells[0].2, cells[1].2, cells[2].2);
        let engineer = studio.text.width_in(table.face, "Engineer", table.font_px) as f32;
        assert!(right - middle >= engineer + 8.0 && right - middle > middle - left, "{left} {middle} {right}");
        // A click in a cell puts the caret there.
        let (x, y, w, h) = table.frame();
        tap(&mut studio, (x + w - 10.0, y + h / 8.0 + 2.0));
        assert_eq!(studio.board.caret.map(|at| cell_at(&shape(&studio, index).text, at)), Some((0, 2)));
        // Enter on the last row is done with it.
        studio.board.caret = Some(shape(&studio, index).text.len());
        press_key(&mut studio, Key::Enter, Mods::default());
        assert!(studio.editing.is_none());
    }

    #[test]
    fn a_right_click_on_the_board_inserts_a_table_where_it_was() {
        let mut studio = board();
        let at = studio.to_window((500.0, 400.0));
        studio.right_click(at);
        let row = studio.menu_items(MenuKind::Context).iter().position(|item| matches!(item, crate::studio::menu::Item::Entry { says, .. } if says == "Insert table")).expect("Insert table");
        let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
        click(&mut studio, rect.center());
        let (x, y, w, h) = shape(&studio, 0).frame();
        assert!(((x + w / 2.0) - 500.0).abs() < 1.0 && ((y + h / 2.0) - 400.0).abs() < 1.0, "{:?}", (x, y, w, h));
        assert_eq!(shape(&studio, 0).form, shapes::FORM_GRID);
    }

    #[test]
    fn a_stamp_goes_on_a_box_s_top_right_side_by_side_and_comes_off_again() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 300.0), (460.0, 372.0), Mods::default());
        type_in(&mut studio, "Ship it");
        press_key(&mut studio, Key::Escape, Mods::default());
        // Shift+E, the list of stamps by it; 1 the first.
        press_key(&mut studio, Key::Char('E'), Mods { shift: true, ..Mods::default() });
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Stamps));
        assert_eq!(shape(&studio, 0).text, "Ship it", "not typed over");
        press_key(&mut studio, Key::Char('1'), Mods::default());
        assert_eq!(shape(&studio, 0).stamps, vec![0]);
        studio.stamp(4);
        assert_eq!(shape(&studio, 0).stamps, vec![0, 4]);
        // Drawn in its color at the top right, the next to its left.
        studio.selected.clear();
        let spots = shape(&studio, 0).stamp_spots();
        assert!(spots[0].1 .0 > spots[1].1 .0 && spots[0].1 .0 > 440.0 && spots[0].1 .1 < 310.0, "{spots:?}");
        assert_eq!(shown(&mut studio, (spots[1].1 .0 + 3.0, spots[1].1 .1 - 6.0)), shapes::STAMPS[4].1, "the heart's badge");
        // All of each inside what the box is drawn again over, past the
        // edge of a small one with nothing written in it.
        let mut small = shape(&studio, 0);
        (small.points[1], small.text, small.stamps) = ((340.0, 330.0), String::new(), vec![0, 1, 2, 3, 4]);
        let (place, radius) = (small.bounds(&mut studio.text), small.stamp_radius());
        for (_, (cx, cy)) in small.stamp_spots() {
            assert!(place.contains((cx - radius) as i32, (cy - radius) as i32) && place.contains((cx + radius) as i32, (cy + radius) as i32), "{place:?} holds the stamp at {cx},{cy}");
        }
        // It moves with the box.
        pull(&mut studio, (380.0, 350.0), (480.0, 450.0), Mods::default());
        assert!((shape(&studio, 0).stamp_spots()[0].1 .0 - spots[0].1 .0 - 100.0).abs() < 1.0);
        // Again: off. One step to undo each.
        studio.selected = vec![0];
        studio.stamp(0);
        assert_eq!(shape(&studio, 0).stamps, vec![4]);
        studio.undo();
        assert_eq!(shape(&studio, 0).stamps, vec![0, 4]);
        // Kept in the file.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert!(matches!(&back[0].loose.layers[0].kind, Kind::Shape(shape) if shape.stamps == vec![0, 4]));
        // On the right click of a box, and found by Ctrl+K.
        assert!(studio.board_acts().iter().any(|(says, _, act)| says == "Stamp: Love" && *act == Act::Stamp(4)));
        let at = studio.to_window((420.0, 400.0));
        studio.right_click(at);
        assert!(studio.menu_items(MenuKind::Context).iter().any(|item| matches!(item, crate::studio::menu::Item::Entry { says, sub: Some(MenuKind::Stamps), .. } if says == "Stamp")));
    }

    /// A block of code, a table and stamped boxes in each look and on dark
    /// paper; and the window with the library open, and with the size and
    /// place of a box in its panel:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test blocks_snapshot -- --ignored
    #[test]
    #[ignore]
    fn blocks_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        let mut studio = board();
        studio.pasted(None, Some("use std::fs;\n\n/// Reads the notes, or none.\nfn notes(path: &str) -> Vec<String> {\n    let words = fs::read_to_string(path).unwrap_or_default();\n    words.lines().map(String::from).collect() // 42\n}".into()));
        if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
            shape.text = format!("notes.rs{}", shape.text);
        }
        studio.fit_block(0, false);
        studio.pasted(None, Some("Service\tOwner\tStatus\nAuth\tAda\tLive\nBilling\tGrace\tIn review\nSearch\tLinus\tPlanned".into()));
        let put = |studio: &mut Studio, at: (f32, f32), words: &str, stamps: Vec<u8>| {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, at);
            studio.board_dress(&mut shape);
            shape.points[1] = (at.0 + 160.0, at.1 + 72.0);
            (shape.text, shape.stamps) = (words.into(), stamps);
            shape.touch();
            studio.doc.layers.push(Layer::shape(shape));
        };
        put(&mut studio, (0.0, 0.0), "Ship Friday", vec![0, 4]);
        put(&mut studio, (0.0, 0.0), "Rewrite it?", vec![2, 3, 1]);
        studio.selected.clear();
        studio.commit_text();
        let frames: Vec<(f32, f32, f32, f32)> = (0..studio.doc.layers.len()).map(|index| studio.layer_frame(index)).collect();
        for (index, (x, y)) in [(40.0, 60.0), (40.0, 300.0), (520.0, 330.0), (520.0, 460.0)].into_iter().enumerate() {
            let (fx, fy, ..) = frames[index];
            studio.doc.layers[index].translate(x - fx, y - fy);
        }
        studio.changed();
        for (look, ground) in [(0u8, 0u8), (1, 0), (2, 0), (look::DIAGRAM, 0), (0, 1), (look::DIAGRAM, 2)] {
            (studio.pages[0].look, studio.pages[0].ground) = (look, ground);
            studio.changed();
            let layers = studio.doc.layers.clone();
            let sheet = studio.part_sheet(&layers, (10.0, 20.0, 720.0, 560.0));
            let (pw, ph, pixels) = sheet.render(std::env::var("HOTSHOT_TIMES").ok().and_then(|times| times.parse().ok()).unwrap_or(1.0), &mut studio.text);
            std::fs::write(to.join(format!("blocks-{look}-{ground}.png")), crate::output::encode(pw as u32, ph as u32, &pixels, false).unwrap()).unwrap();
        }
        // The window: the library open, then a box picked out.
        for look in [0u8, look::DIAGRAM] {
            (studio.pages[0].look, studio.pages[0].ground) = (look, 0);
            studio.changed();
            studio.selected = vec![2];
            studio.board.shelf = false;
            studio.save_piece();
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            let hovered = studio.control_rect(Control::Board(Ctl::Piece(2))).unwrap().center();
            studio.pointer_moved(hovered.0, hovered.1, Mods::default());
            studio.draw(&mut out);
            std::fs::write(to.join(format!("library-{look}.png")), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
            studio.board.shelf = false;
            studio.board.pieces = None;
            studio.selected = vec![2];
            studio.board.exact_typed = Some((0, "2".into()));
            studio.pointer_moved(10, 10, Mods::default());
            studio.draw(&mut out);
            std::fs::write(to.join(format!("exact-{look}.png")), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
            studio.board.exact_typed = None;
            // The stamps' list by the box.
            studio.open_stamps();
            studio.draw(&mut out);
            std::fs::write(to.join(format!("stamps-menu-{look}.png")), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
            studio.menu = None;
        }
        // Typed into: a block of code just put down, and a table.
        for look in [1u8, 2] {
            let mut studio = board();
            studio.set_look(Some(look), None);
            studio.insert_table(Some((300.0, 600.0)));
            type_in(&mut studio, "Step");
            press_key(&mut studio, Key::Tab, Mods::default());
            type_in(&mut studio, "Who");
            press_key(&mut studio, Key::Enter, Mods::default());
            studio.insert_code(Some((600.0, 300.0)));
            type_in(&mut studio, "for step in plan {");
            press_key(&mut studio, Key::Enter, Mods::default());
            type_in(&mut studio, "run(step);");
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            std::fs::write(to.join(format!("typing-{look}.png")), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
        }
    }
}
