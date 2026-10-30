//! The caret in a board's words, as any text field has it: a click in a
//! box's or an arrow's words as they are typed puts the caret there, a
//! drag across them picks some out, and what is typed goes in at the
//! caret, over what is picked out; Left, Right, Home and End move it, with
//! Shift picking out as they go.

use super::*;

impl Studio {
    /// The words of the shape being typed into, if it is a box or an arrow
    /// of a board, whose words are laid out as runs.
    fn caret_shape(&self) -> Option<(usize, Shape)> {
        let index = self.editing?;
        match self.doc.layers.get(index) {
            Some(Layer { kind: Kind::Shape(shape), .. }) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow) => Some((index, shape.clone())),
            _ => None,
        }
    }

    /// Where in the words the caret is, kept to them and to a letter's edge.
    pub(in crate::studio) fn caret_at(&self, words: &str) -> usize {
        let mut at = self.board.caret.unwrap_or(words.len()).min(words.len());
        while !words.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// What of the words is picked out, from and to, or the caret twice.
    fn picked_words(&self, words: &str) -> (usize, usize) {
        let caret = self.caret_at(words);
        match self.board.anchor.map(|anchor| anchor.min(words.len())).filter(|&anchor| words.is_char_boundary(anchor)) {
            Some(anchor) => (anchor.min(caret), anchor.max(caret)),
            None => (caret, caret),
        }
    }

    /// The place in a shape's words nearest a point of the board, and
    /// whether the point is on them, a line's height and a little to
    /// either side.
    pub(in crate::studio) fn place_in_words(&mut self, shape: &Shape, at: (f32, f32)) -> Option<(usize, bool)> {
        let (runs, high) = shape.label_runs(&mut self.text)?;
        let last = runs.len().checked_sub(1)?;
        let row = runs.iter().position(|run| at.1 < run.3 + high as f32).unwrap_or(last);
        let (start, line, left, top, wide) = &runs[row];
        let on = at.1 >= runs[0].3 && at.1 <= runs[last].3 + high as f32 && at.0 >= left - 6.0 && at.0 <= left + *wide as f32 + 6.0;
        let along = at.0 - left;
        let mut best = (0, f32::MAX);
        for (offset, _) in line.char_indices().chain([(line.len(), ' ')]) {
            let x = self.text.width_in(shape.face, &line[..offset], shape.font_px) as f32;
            if (x - along).abs() < best.1 {
                best = (offset, (x - along).abs());
            }
        }
        let _ = top;
        Some((start + best.0, on))
    }

    /// Where a place in a shape's words is drawn: across, down, and how
    /// tall a line is.
    pub(in crate::studio) fn words_spot(&mut self, shape: &Shape, at: usize) -> Option<(f32, f32, i32)> {
        let (runs, high) = shape.label_runs(&mut self.text)?;
        let run = runs.iter().rev().find(|run| run.0 <= at)?;
        let offset = (at - run.0).min(run.1.len());
        let offset = (0..=offset).rev().find(|&offset| run.1.is_char_boundary(offset)).unwrap_or(0);
        Some((run.2 + self.text.width_in(shape.face, &run.1[..offset], shape.font_px) as f32, run.3, high))
    }

    /// Where the caret of the words being typed is drawn on a board, if it
    /// is not at their end.
    pub(in crate::studio) fn board_caret(&mut self) -> Option<(f32, f32, i32)> {
        let (_, shape) = self.caret_shape()?;
        self.board.caret?;
        let at = self.caret_at(&shape.text);
        self.words_spot(&shape, at)
    }

    /// A press on the words of the shape being typed into: the caret there,
    /// and a drag from it picks words out.
    pub(in crate::studio) fn press_words(&mut self, at: (f32, f32)) -> bool {
        let Some((_, shape)) = self.caret_shape() else { return false };
        let Some((place, true)) = self.place_in_words(&shape, at) else { return false };
        (self.board.caret, self.board.anchor) = (Some(place), Some(place));
        self.drag = Drag::Words;
        self.dirty = true;
        true
    }

    /// The pointer moved as words are picked out: to where it is.
    pub(in crate::studio) fn words_dragged(&mut self) {
        let Some((_, shape)) = self.caret_shape() else { return };
        let at = self.to_doc(self.pointer);
        if let Some((place, _)) = self.place_in_words(&shape, at) {
            self.board.caret = Some(place);
            self.dirty = true;
        }
    }

    /// Whether the pointer over the shape being typed into is over its
    /// words, where a press puts the caret rather than moving it.
    pub(in crate::studio) fn over_words(&mut self, at: (f32, f32)) -> bool {
        let Some((_, shape)) = self.caret_shape() else { return false };
        self.place_in_words(&shape, at).is_some_and(|(_, on)| on)
    }

    /// The words picked out drawn over, in the accent, faintly.
    pub(in crate::studio) fn draw_words_picked(&mut self, canvas: &mut Canvas) {
        let Some((_, shape)) = self.caret_shape() else { return };
        let (from, to) = self.picked_words(&shape.text);
        if from == to {
            return;
        }
        let Some((runs, high)) = shape.label_runs(&mut self.text) else { return };
        for (start, line, left, top, _) in runs {
            let (a, b) = (from.max(start), to.min(start + line.len()));
            if a >= b {
                continue;
            }
            let x = |offset: usize, text: &mut Text| left + text.width_in(shape.face, &line[..offset - start], shape.font_px) as f32;
            let (x0, x1) = (x(a, &mut self.text), x(b, &mut self.text));
            let (p, q) = (self.to_window((x0, top)), self.to_window((x1, top + high as f32)));
            canvas.round_fill(p.0, p.1, (q.0 - p.0).max(1), (q.1 - p.1).max(1), 0, ACCENT, 70);
        }
    }

    /// A key while a board's box or arrow is typed into, at the caret: a
    /// letter or a new line in at it, over what is picked out; Backspace
    /// and Delete what is picked out, or the letter before or after it;
    /// Left, Right, Home and End move it, Shift picking out. Whether it
    /// was one of those; `over` types over all of the words.
    pub(in crate::studio) fn caret_key(&mut self, key: Key, mods: Mods, over: bool) -> bool {
        let Some((index, shape)) = self.caret_shape() else { return false };
        let words = shape.text;
        let (from, to) = if over { (0, words.len()) } else { self.picked_words(&words) };
        let caret = self.caret_at(&words);
        let before = |at: usize| words[..at].char_indices().next_back().map_or(0, |(offset, _)| offset);
        let after = |at: usize| words[at..].chars().next().map_or(at, |letter| at + letter.len_utf8());
        let put = |studio: &mut Studio, from: usize, to: usize, with: &str| {
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = studio.doc.layers.get_mut(index) {
                shape.text.replace_range(from..to, with);
                shape.touch();
            }
            (studio.board.caret, studio.board.anchor) = (Some(from + with.len()), None);
            studio.words_changed(index);
        };
        let move_to = |studio: &mut Studio, to: usize| {
            let anchor = match mods.shift {
                true => Some(studio.board.anchor.unwrap_or(caret)),
                false => None,
            };
            (studio.board.caret, studio.board.anchor) = (Some(to), anchor);
            studio.dirty = true;
        };
        match key {
            Key::Char(letter) if !mods.ctrl => put(self, from, to, letter.encode_utf8(&mut [0; 4])),
            Key::Enter if mods.shift => put(self, from, to, "\n"),
            Key::Backspace if from < to => put(self, from, to, ""),
            // With Ctrl, the word before it.
            Key::Backspace if mods.ctrl => {
                let kept = words[..caret].trim_end().trim_end_matches(|letter: char| !letter.is_whitespace()).len();
                put(self, kept, caret, "");
            }
            Key::Backspace => put(self, before(caret), caret, ""),
            Key::Delete if from < to => put(self, from, to, ""),
            Key::Delete => put(self, caret, after(caret), ""),
            Key::Left if from < to && !mods.shift => move_to(self, from),
            Key::Right if from < to && !mods.shift => move_to(self, to),
            Key::Left => move_to(self, before(caret)),
            Key::Right => move_to(self, after(caret)),
            Key::Home => move_to(self, words[..caret].rfind('\n').map_or(0, |at| at + 1)),
            Key::End => move_to(self, words[caret..].find('\n').map_or(words.len(), |at| caret + at)),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn a_click_in_the_words_puts_the_caret_there_a_drag_picks_out_and_typing_goes_in_at_it() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 300.0), (400.0, 372.0), Mods::default());
        type_in(&mut studio, "red fox");
        // A click just before the f of fox, in the words.
        let shape = shape(&studio, 0);
        let (x, y, high) = studio.words_spot(&shape, 4).unwrap();
        tap(&mut studio, (x + 1.0, y + high as f32 / 2.0));
        assert_eq!((studio.editing, studio.board.caret), (Some(0), Some(4)), "the caret before fox");
        type_in(&mut studio, "big ");
        assert_eq!(crate::studio::board::tests::shape(&studio, 0).text, "red big fox");
        // A drag across big picks it out; typing replaces it.
        let shape = crate::studio::board::tests::shape(&studio, 0);
        let (a, ay, high) = studio.words_spot(&shape, 4).unwrap();
        let (b, _, _) = studio.words_spot(&shape, 7).unwrap();
        pull(&mut studio, (a + 1.0, ay + high as f32 / 2.0), (b, ay + high as f32 / 2.0), Mods::default());
        assert_eq!(studio.picked_words(&shape.text), (4, 7));
        assert!((crate::studio::board::tests::shape(&studio, 0).frame().0 - 100.0).abs() < 1.0, "the box did not move");
        type_in(&mut studio, "sly");
        assert_eq!(crate::studio::board::tests::shape(&studio, 0).text, "red sly fox");
        // Left, Shift+Left and Backspace at the caret.
        press_key(&mut studio, Key::Left, Mods::default());
        press_key(&mut studio, Key::Left, Mods { shift: true, ..Mods::default() });
        press_key(&mut studio, Key::Backspace, Mods::default());
        assert_eq!(crate::studio::board::tests::shape(&studio, 0).text, "red sy fox");
        // A click on the box away from its words: the caret at the end.
        tap(&mut studio, (120.0, 360.0));
        type_in(&mut studio, "!");
        assert_eq!(crate::studio::board::tests::shape(&studio, 0).text, "red sy fox!");
    }
}
