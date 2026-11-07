//! Ctrl+F on a board: a bar at its top to find words by, as a browser's
//! is. What is typed is found in the words of every box, oval, note, text
//! and arrow, whatever their case; each place it is found is marked over
//! in yellow as a marker would, by hand in the looks drawn by hand and
//! cleanly in the others, and the one gone to is ringed in the accent.
//! Enter goes on to the next, Shift+Enter back, the view gliding over to
//! it; Esc closes the bar and the marks go with it.

use super::*;

/// The bar: how wide and tall, and the field in it.
const BAR_W: i32 = 432;
const BAR_H: i32 = 40;
const FIELD_W: i32 = 214;
/// The buttons at its right, each so wide.
const BUTTON: i32 = 28;

/// The yellow of a marker, laid over light paper as light as what is
/// under it; and the amber it is over dark paper, lightening it; and the
/// one gone to, deeper, under its ring.
const MARKER: u32 = 0xFFE066;
const MARKER_ON: u32 = 0xFFC94D;
const MARKER_DARK: u32 = 0x5E480C;
const MARKER_DARK_ON: u32 = 0x7A5A08;

/// The find bar while it is open: what is typed into it, which of what it
/// finds is gone to, whether the keys type into it, and whether all of
/// what is typed is picked out, to be typed over.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Find {
    pub typed: String,
    pub at: usize,
    pub typing: bool,
    pub picked: bool,
}

/// Something found: in which layer, and from where to where in its words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) struct Found {
    pub index: usize,
    pub from: usize,
    pub to: usize,
}

/// A line of a shape's words as it is drawn, as `word_lines` has it.
struct WordLine {
    start: usize,
    long: usize,
    shown: String,
    left: f32,
    top: f32,
    px: u32,
    hidden: usize,
}

/// Where what is typed is in some words, each time, whatever the case of
/// either, as from and to in them; none for nothing typed.
pub(in crate::studio) fn occurrences(words: &str, typed: &str) -> Vec<(usize, usize)> {
    let wanted: Vec<char> = typed.chars().collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let letters: Vec<(usize, char)> = words.char_indices().collect();
    let same = |a: char, b: char| a == b || a.to_lowercase().eq(b.to_lowercase());
    let mut found = Vec::new();
    let mut at = 0;
    while at + wanted.len() <= letters.len() {
        if wanted.iter().enumerate().all(|(k, &letter)| same(letters[at + k].1, letter)) {
            let end = letters.get(at + wanted.len()).map_or(words.len(), |(offset, _)| *offset);
            found.push((letters[at].0, end));
            at += wanted.len();
        } else {
            at += 1;
        }
    }
    found
}

impl Studio {
    /// Ctrl+F: the bar opened, or, open, its field typed into again with
    /// all of it picked out.
    pub(in crate::studio) fn open_find(&mut self) {
        self.commit_text();
        let find = self.board.find.get_or_insert_with(Find::default);
        find.typing = true;
        find.picked = !find.typed.is_empty();
        self.dirty = true;
    }

    /// The bar closed, and what it marked with it.
    pub(in crate::studio) fn close_find(&mut self) {
        self.board.find = None;
        self.dirty = true;
    }

    /// Everything on the board what is typed is found in, in the order it
    /// is read: down the board, then across, and on through each one's
    /// words.
    pub(in crate::studio) fn find_all(&mut self) -> Vec<Found> {
        let Some(typed) = self.board.find.as_ref().map(|find| find.typed.clone()).filter(|typed| !typed.is_empty()) else { return Vec::new() };
        let mut found: Vec<((i32, i32), Found)> = Vec::new();
        for index in 0..self.doc.layers.len() {
            let layer = &self.doc.layers[index];
            let Kind::Shape(shape) = &layer.kind else { continue };
            if layer.hidden || shape.text.is_empty() || !matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Text | Tool::Arrow | Tool::Line) {
                continue;
            }
            let spots = occurrences(&shape.text, &typed);
            if spots.is_empty() {
                continue;
            }
            let place = self.doc.layers[index].bounds(&mut self.text);
            let place = (place.1.round() as i32, place.0.round() as i32);
            found.extend(spots.into_iter().map(|(from, to)| (place, Found { index, from, to })));
        }
        found.sort_by_key(|(place, found)| (*place, found.index, found.from));
        found.into_iter().map(|(_, found)| found).collect()
    }

    /// The lines of a shape's words as they are drawn, each with where in
    /// its words it begins and how long it is there, what it shows, where
    /// it is across and down, its size, and how many of its letters are
    /// not shown before what is: a heading's hashes. None for words that
    /// are not laid out in lines, a table's.
    fn word_lines(&mut self, shape: &Shape) -> Option<Vec<WordLine>> {
        if shape.tool != Tool::Text {
            let (runs, _) = shape.label_runs(&mut self.text)?;
            return Some(runs.into_iter().map(|(start, line, left, top, _)| WordLine { start, long: line.len(), shown: line, left, top, px: shape.font_px, hidden: 0 }).collect());
        }
        // Text: by its own lines, a heading's hashes left out as it shows
        // them, each line set as its text is aligned, after its dot.
        let place = shape.local_bounds(&mut self.text);
        let lines = shape.text_lines(&mut self.text);
        let widest = lines.iter().map(|line| line.width).max().unwrap_or(0);
        let lead = shape.lead();
        let mut start = 0;
        let mut runs = Vec::new();
        for (raw, line) in shape.text.split('\n').zip(lines) {
            let left = place.x as f32 + ((widest - line.width) * i32::from(shape.align.min(2)) / 2) as f32;
            let hidden = match (shapes::heading_of(raw), shape.editing) {
                (Some((level, _)), false) => level + 1,
                _ => 0,
            };
            let lead_wide = self.text.width_in(shape.words_face(), lead, line.px) as f32;
            let shown = line.shown.get(lead.len()..).unwrap_or("").to_owned();
            runs.push(WordLine { start, long: raw.len(), shown, left: left + lead_wide, top: (place.y + line.top) as f32, px: line.px, hidden });
            start += raw.len() + 1;
        }
        Some(runs)
    }

    /// Where something found is drawn, on the board: a piece for each line
    /// it is on, from left to right, its top and how tall. Nothing for a
    /// shape that is turned, or a table's words.
    pub(in crate::studio) fn found_pieces(&mut self, found: Found) -> Vec<(f32, f32, f32, f32)> {
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(found.index).cloned() else { return Vec::new() };
        if shape.angle != 0.0 {
            return Vec::new();
        }
        let Some(runs) = self.word_lines(&shape) else { return Vec::new() };
        let face = shape.words_face();
        let mut pieces = Vec::new();
        for run in runs {
            let (from, to) = (found.from.max(run.start), found.to.min(run.start + run.long));
            if from >= to {
                continue;
            }
            // How far across a place in the line is, by its letters: what
            // is shown may be in capitals, so counted rather than cut.
            let mut across = |at: usize| {
                let count = shape.text.get(run.start..at).map_or(0, |before| before.chars().count()).saturating_sub(run.hidden);
                let shown: String = run.shown.chars().take(count).collect();
                run.left + self.text.width_in(face, &shown, run.px) as f32
            };
            let (x0, x1) = (across(from), across(to));
            pieces.push((x0, run.top, (x1 - x0).max(1.0), Text::line_height(run.px) as f32));
        }
        pieces
    }

    /// Where on the board something found is, as one frame round all of it,
    /// or the frame of what it is in.
    fn found_frame(&mut self, found: Found) -> (f32, f32, f32, f32) {
        let pieces = self.found_pieces(found);
        actions::union(&pieces).unwrap_or_else(|| self.doc.layers[found.index].bounds(&mut self.text))
    }

    /// On to the next of what is found, or back to the one before, round
    /// from the last to the first; the view over to it.
    pub(in crate::studio) fn find_step(&mut self, forward: bool) {
        let all = self.find_all();
        let Some(find) = &mut self.board.find else { return };
        if all.is_empty() {
            return;
        }
        let count = all.len();
        find.at = match forward {
            true => (find.at.min(count - 1) + 1) % count,
            false => (find.at.min(count - 1) + count - 1) % count,
        };
        self.show_found();
    }

    /// The view over to what is gone to, to its middle, where it is not
    /// in view.
    fn show_found(&mut self) {
        let all = self.find_all();
        let Some(at) = self.board.find.as_ref().map(|find| find.at) else { return };
        let Some(&found) = all.get(at) else { return };
        let frame = self.found_frame(found);
        self.view_onto(frame, true);
        self.dirty = true;
    }

    /// A key while the bar is open. Typing into it: letters, Backspace,
    /// Enter and Shift+Enter or Down and Up to go on and back, and Esc to
    /// close it; Ctrl's keys but Ctrl+F, Ctrl+A and Ctrl+Backspace go on to
    /// do what they do. Not typing into it, Esc closes it, and Ctrl+F types
    /// into it again. Whether the key was taken.
    pub(in crate::studio) fn find_key(&mut self, key: Key, mods: Mods) -> bool {
        if mods.ctrl && !mods.alt && matches!(key, Key::Char('f' | 'F')) && self.on_board() && !self.presenting() {
            self.open_find();
            return true;
        }
        let Some(find) = &mut self.board.find else { return false };
        if !find.typing {
            if key == Key::Escape && self.editing.is_none() && self.menu.is_none() {
                self.close_find();
                return true;
            }
            return false;
        }
        self.dirty = true;
        let was = find.typed.clone();
        let picked = std::mem::take(&mut find.picked);
        match key {
            Key::Escape => {
                self.close_find();
                return true;
            }
            Key::Enter | Key::Down | Key::Up => {
                find.picked = picked;
                let forward = match key {
                    Key::Enter => !mods.shift,
                    other => other == Key::Down,
                };
                self.find_step(forward);
                return true;
            }
            Key::Char('a' | 'A') if mods.ctrl => find.picked = !find.typed.is_empty(),
            Key::Backspace if picked => find.typed.clear(),
            Key::Backspace if mods.ctrl => {
                let kept = find.typed.trim_end().trim_end_matches(|letter: char| !letter.is_whitespace()).len();
                find.typed.truncate(kept);
            }
            Key::Backspace => drop(find.typed.pop()),
            Key::Char(letter) if !mods.ctrl && !mods.alt => {
                if picked {
                    find.typed.clear();
                }
                find.typed.push(letter);
            }
            // Shift and the like leave it picked out.
            Key::Other => find.picked = picked,
            _ if mods.ctrl => {
                find.picked = picked;
                return false;
            }
            _ => {}
        }
        // Typed into: the first of what it now finds gone to.
        if find.typed != was {
            find.at = 0;
            self.show_found();
        }
        true
    }

    /// Words pasted while the bar is typed into, into it. Whether they were.
    pub(in crate::studio) fn find_pasted(&mut self, words: &str) -> bool {
        let Some(find) = self.board.find.as_mut().filter(|find| find.typing) else { return false };
        if std::mem::take(&mut find.picked) {
            find.typed.clear();
        }
        find.typed.push_str(&words.replace(['\n', '\t'], " "));
        find.at = 0;
        self.show_found();
        true
    }

    /// Where the bar is: at the top, in the middle of the board, in line
    /// with the pill at the top left.
    fn find_bar(&mut self) -> Rect {
        let open = self.open_area();
        let pill = self.logo_place();
        Rect::new(open.x + (open.w - BAR_W) / 2, pill.y + pill.h / 2 - BAR_H / 2, BAR_W, BAR_H)
    }

    /// The bar's parts: its field, back, on and close; and all of it.
    pub(in crate::studio) fn layout_find(&mut self, layout: &mut Layout) {
        if self.board.find.is_none() {
            return;
        }
        let bar = self.find_bar();
        let cy = bar.y + bar.h / 2;
        let close = Rect::new(bar.right() - 6 - BUTTON, cy - BUTTON / 2, BUTTON, BUTTON);
        let next = Rect::new(close.x - 13 - BUTTON, close.y, BUTTON, BUTTON);
        let back = Rect::new(next.x - 2 - BUTTON, close.y, BUTTON, BUTTON);
        layout.controls.push((Rect::new(bar.x + 34, bar.y + 4, FIELD_W, bar.h - 8), Control::Board(Ctl::FindField)));
        layout.controls.push((back, Control::Board(Ctl::FindBack)));
        layout.controls.push((next, Control::Board(Ctl::FindNext)));
        layout.controls.push((close, Control::Board(Ctl::FindClose)));
        layout.controls.push((bar, Control::Board(Ctl::FindBar)));
    }

    /// A part of the bar pressed.
    pub(in crate::studio) fn press_find(&mut self, ctl: Ctl) {
        match ctl {
            Ctl::FindBack | Ctl::FindNext => self.find_step(ctl == Ctl::FindNext),
            Ctl::FindClose => self.close_find(),
            _ => {
                self.commit_text();
                if let Some(find) = &mut self.board.find {
                    (find.typing, find.picked) = (true, false);
                }
            }
        }
        self.dirty = true;
    }

    /// What a part of the bar says, hovered.
    pub(in crate::studio) fn find_tip(&self, ctl: Ctl) -> Option<String> {
        Some(match ctl {
            Ctl::FindBack => "Previous  Shift+Enter",
            Ctl::FindNext => "Next  Enter",
            Ctl::FindClose => "Close  Esc",
            _ => return None,
        }.to_owned())
    }

    /// A press on the board itself, not the bar: the keys are the board's
    /// again, the bar and its marks left open.
    pub(in crate::studio) fn find_let_go(&mut self) {
        if let Some(find) = &mut self.board.find {
            (find.typing, find.picked) = (false, false);
        }
    }

    /// Everything found marked over, as a marker would mark it: by hand in
    /// the looks drawn by hand, a little uneven, its ends rounded; cleanly
    /// in the others. The one gone to deeper, and ringed in the accent: by
    /// hand, a loop round it as a pen would make; cleanly, a rounded box.
    pub(in crate::studio) fn draw_found(&mut self, canvas: &mut Canvas) {
        let all = self.find_all();
        let Some(at) = self.board.find.as_ref().map(|find| find.at) else { return };
        let by_hand = self.pages[self.page].look < 2;
        let dark = self.pages[self.page].ground > 0;
        let view = canvas.clip();
        for (order, found) in all.iter().enumerate() {
            let on = order == at.min(all.len().saturating_sub(1));
            let pieces = self.found_pieces(*found);
            for &(x, y, w, h) in &pieces {
                let (a, b) = (self.to_window_f((x, y)), self.to_window_f((x + w, y + h)));
                // Past the letters a little, by hand; cleanly, hugging them,
                // as the spaces between words are narrower there.
                let pad = if by_hand { 2.0 + 1.5 * self.zoom.min(2.0) } else { 0.5 + 0.5 * self.zoom.min(2.0) };
                let high = b.1 - a.1;
                let (x0, x1) = (a.0 - pad, b.0 + pad);
                if x1 < view.x as f32 || x0 > view.right() as f32 || b.1 < view.y as f32 || a.1 > view.bottom() as f32 {
                    continue;
                }
                let (top, bottom) = (a.1 + high * 0.08, b.1 - high * 0.04);
                let seed = (found.index * 31 + found.from) as f32;
                let edge = mark_edge(x0, x1, top, bottom, by_hand, seed);
                let color = match (dark, on) {
                    (false, false) => MARKER,
                    (false, true) => MARKER_ON,
                    (true, false) => MARKER_DARK,
                    (true, true) => MARKER_DARK_ON,
                };
                canvas.marker(x0, x1, edge, color, dark);
            }
            if on && !pieces.is_empty() {
                let (x, y, w, h) = actions::union(&pieces).unwrap_or_default();
                let (a, b) = (self.to_window_f((x, y)), self.to_window_f((x + w, y + h)));
                let room = if by_hand { (6.0, 3.0) } else { (3.0, 2.0) };
                ring(canvas, (a.0 - room.0, a.1 - room.1, b.0 + room.0, b.1 + room.1), by_hand, (found.index * 7 + found.from) as f32);
            }
        }
    }

    /// The bar: a dark pill of hotshot's own at the top of the board, a
    /// magnifier, the field, which of how many is gone to, back and on,
    /// and close.
    pub(in crate::studio) fn draw_find(&mut self, canvas: &mut Canvas, layout: &Layout, hover: Option<Control>) {
        let Some(find) = self.board.find.clone() else { return };
        let rect_of = |ctl: Ctl| layout.controls.iter().find(|(_, control)| *control == Control::Board(ctl)).map(|(rect, _)| *rect);
        let (Some(bar), Some(field), Some(back), Some(next), Some(close)) = (rect_of(Ctl::FindBar), rect_of(Ctl::FindField), rect_of(Ctl::FindBack), rect_of(Ctl::FindNext), rect_of(Ctl::FindClose)) else { return };
        let all = self.find_all();
        let cy = bar.y + bar.h / 2;
        let radius = bar.h / 2;
        soft_shadow(canvas, bar, radius);
        canvas.round_fill(bar.x, bar.y, bar.w, bar.h, radius, PANEL_COLOR, 256);
        canvas.round_outline(bar.x, bar.y, bar.w, bar.h, radius, 1, if find.typing { 0x4A4A4A } else { EDGE_COLOR });
        self.text.draw_centered(canvas, bar.x + 20, cy, 15, if find.typing { TEXT_COLOR } else { DIM_TEXT_COLOR }, icons::SEARCH);
        // The field: what is typed, its tail where it is longer than the
        // field, the caret after it; faintly, what it is for.
        let px = 13;
        let line = Text::line_height(px);
        let top = cy - line / 2;
        canvas.set_clip(Rect::new(field.x, field.y, field.w, field.h).intersect(canvas.clip()));
        if find.typed.is_empty() {
            self.text.draw(canvas, field.x + 5, top, px, 0x7A7A7A, "Find on board");
        } else {
            let wide = self.text.width(&find.typed, px);
            let x = field.x + (field.w - 6 - wide).min(0);
            if find.picked {
                canvas.round_fill(x - 2, top, wide + 4, line, 3, ACCENT, 140);
            }
            self.text.draw(canvas, x, top, px, TEXT_COLOR, &find.typed);
        }
        if find.typing && !find.picked {
            let wide = if find.typed.is_empty() { 0 } else { self.text.width(&find.typed, px) };
            let x = field.x + wide.min(field.w - 6) + 1;
            canvas.fill(x, top + 1, 2, line - 2, ACCENT_TEXT);
        }
        canvas.reset_clip();
        // Which of how many, or that there are none.
        let says = match (find.typed.is_empty(), all.len()) {
            (true, _) => String::new(),
            (false, 0) => "No results".to_owned(),
            (false, count) => format!("{} of {count}", find.at.min(count - 1) + 1),
        };
        if !says.is_empty() {
            let wide = self.text.width(&says, 12);
            let color = if all.is_empty() { 0xE5807A } else { DIM_TEXT_COLOR };
            self.text.draw(canvas, back.x - 10 - wide, cy - Text::line_height(12) / 2, 12, color, &says);
        }
        // Back and on, as chevrons up and down; close, past a rule.
        let any = !all.is_empty();
        for (rect, ctl, up) in [(back, Ctl::FindBack, true), (next, Ctl::FindNext, false)] {
            let hovered = hover == Some(Control::Board(ctl)) && any;
            if hovered {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 8, HOVER_COLOR, 256);
            }
            let ink = if any { TEXT_COLOR } else { 0x5E5E5E };
            let (mx, my) = (rect.x as f32 + rect.w as f32 / 2.0, rect.y as f32 + rect.h as f32 / 2.0);
            let turn = if up { -1.0 } else { 1.0 };
            halo(canvas, &[(mx - 5.0, my - 2.5 * turn), (mx, my + 2.5 * turn), (mx + 5.0, my - 2.5 * turn)], 0.85, ink, 256);
        }
        canvas.fill(close.x - 7, cy - 9, 1, 18, 0x484848);
        if hover == Some(Control::Board(Ctl::FindClose)) {
            canvas.round_fill(close.x, close.y, close.w, close.h, 8, HOVER_COLOR, 256);
        }
        self.text.draw_centered(canvas, close.x + close.w / 2, close.y + close.h / 2, 13, TEXT_COLOR, icons::CLOSE);
    }
}

/// The top and bottom of a marker's stroke at each column, from `x0` to
/// `x1`: its ends rounded, as a marker's tip is; by hand, its edges a
/// little uneven and the whole of it a touch aslant, the same each time it
/// is drawn for the same words.
fn mark_edge(x0: f32, x1: f32, top: f32, bottom: f32, by_hand: bool, seed: f32) -> impl Fn(f32) -> (f32, f32) {
    let high = bottom - top;
    let round = (high * if by_hand { 0.45 } else { 0.22 }).min((x1 - x0) / 2.0);
    let long = (x1 - x0).max(1.0);
    move |x: f32| {
        // The ends rounded off.
        let from_end = (x - x0).min(x1 - x);
        let cap = match from_end < round {
            true => round - (round * round - (round - from_end).powi(2)).max(0.0).sqrt(),
            false => 0.0,
        };
        let cap = if by_hand { cap } else { cap * 0.9 };
        match by_hand {
            false => (top + cap, bottom - cap),
            true => {
                let t = (x - x0) / long;
                let slant = (t - 0.5) * high * 0.10;
                let wobble = |phase: f32| ((x * 0.045 + seed + phase).sin() * 0.55 + (x * 0.11 + seed * 1.7 + phase).sin() * 0.35) * high * 0.06;
                (top + cap - slant + wobble(0.0), bottom - cap * 1.1 - slant + wobble(2.1))
            }
        }
    }
}

/// A ring in the accent round what is gone to, from its left top to its
/// right bottom: by hand, a loop drawn once round as a pen would, its end
/// running on a little past its start; cleanly, a rounded box.
fn ring(canvas: &mut Canvas, (left, top, right, bottom): (f32, f32, f32, f32), by_hand: bool, seed: f32) {
    if !by_hand {
        let (x, y) = (left.round() as i32, top.round() as i32);
        let (w, h) = ((right - left).round() as i32, (bottom - top).round() as i32);
        canvas.round_outline(x, y, w, h, 6, 2, ACCENT);
        return;
    }
    let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
    let (rx, ry) = ((right - left) / 2.0 + 4.0, (bottom - top) / 2.0 + 3.0);
    let steps = 64;
    let points: Vec<(f32, f32)> = (0..=steps + 7).map(|step| {
        let t = step as f32 / steps as f32;
        let angle = std::f32::consts::TAU * t - 2.4;
        // A little larger as it goes round, so its end passes its start.
        let grow = 1.0 + 0.07 * t + 0.025 * (angle * 3.0 + seed).sin();
        (cx + angle.cos() * rx * grow, cy + angle.sin() * ry * grow)
    }).collect();
    halo(canvas, &points, 1.0, ACCENT, 256);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };

    /// A board with words in a box, an oval, a note, text and an arrow.
    pub(in crate::studio) fn worded() -> Studio {
        let mut studio = board();
        for (tool, from, to, words) in [('r', (100.0, 100.0), (300.0, 172.0), "Cache layer"), ('o', (500.0, 100.0), (680.0, 190.0), "cache miss"), ('s', (100.0, 400.0), (250.0, 550.0), "No CACHE here")] {
            press_key(&mut studio, Key::Char(tool), Mods::default());
            pull(&mut studio, from, to, Mods::default());
            type_in(&mut studio, words);
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
        }
        press_key(&mut studio, Key::Char('t'), Mods::default());
        tap(&mut studio, (500.0, 420.0));
        type_in(&mut studio, "Notes");
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let arrow = studio.join(0, 1);
        if let Kind::Shape(shape) = &mut studio.doc.layers[arrow].kind {
            shape.text = "warm the cache".into();
            shape.touch();
        }
        studio.changed();
        studio.selected.clear();
        studio
    }

    #[test]
    fn what_is_typed_is_found_in_any_case_in_every_kind_of_words() {
        assert_eq!(occurrences("Cache cACHE", "cache"), vec![(0, 5), (6, 11)]);
        assert_eq!(occurrences("aaa", "aa"), vec![(0, 2)]);
        assert!(occurrences("anything", "").is_empty());
        assert_eq!(occurrences("Café au CAFÉ", "café"), vec![(0, 5), (9, 14)]);
        let mut studio = worded();
        press_key(&mut studio, Key::Char('f'), CTRL);
        assert!(studio.board.find.as_ref().is_some_and(|find| find.typing));
        type_in(&mut studio, "cache");
        let all = studio.find_all();
        let kinds: Vec<Tool> = all.iter().map(|found| shape(&studio, found.index).tool).collect();
        assert_eq!(all.len(), 4, "box, oval, note and arrow: {all:?}");
        assert!(kinds.contains(&Tool::Arrow) && kinds.contains(&Tool::Ellipse));
        // Typed into, no letter took up a tool or typed over a box.
        assert_eq!(studio.board_tool(), SELECT);
        assert_eq!(shape(&studio, 0).text, "Cache layer");
        // Text too.
        press_key(&mut studio, Key::Char('f'), CTRL);
        type_in(&mut studio, "NOTES");
        assert_eq!(studio.find_all().len(), 1);
        let first = studio.find_all()[0].index;
        assert_eq!(shape(&studio, first).tool, Tool::Text);
    }

    #[test]
    fn enter_and_shift_enter_go_on_and_back_and_the_view_goes_over_to_it() {
        let mut studio = worded();
        // Something far off the board's right.
        let mut far = Shape::new(Tool::Rect, INK, LINE, (3000.0, 2000.0));
        studio.board_dress(&mut far);
        far.points[1] = (3160.0, 2072.0);
        far.text = "Cache far away".into();
        studio.doc.layers.push(Layer::shape(far));
        studio.changed();
        press_key(&mut studio, Key::Char('f'), CTRL);
        type_in(&mut studio, "cache");
        let count = studio.find_all().len();
        assert_eq!(count, 5);
        assert_eq!(studio.board.find.as_ref().unwrap().at, 0);
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!(studio.board.find.as_ref().unwrap().at, 1);
        press_key(&mut studio, Key::Enter, Mods { shift: true, ..Mods::default() });
        press_key(&mut studio, Key::Enter, Mods { shift: true, ..Mods::default() });
        let at = studio.board.find.as_ref().unwrap().at;
        assert_eq!(at, count - 1, "round to the last");
        let last = studio.find_all()[at];
        assert_eq!(shape(&studio, last.index).text, "Cache far away", "the last down the board");
        // The view glides over to it, and it is in the middle.
        assert!(studio.board.glide.is_some());
        studio.end_glide();
        let (x, y, w, h) = studio.found_frame(last);
        let middle = studio.to_window((x + w / 2.0, y + h / 2.0));
        let open = studio.open_area();
        assert!(open.contains(middle.0, middle.1) && (middle.0 - open.center().0).abs() < 4, "{middle:?}");
        // The bar's buttons do the same.
        let next = studio.control_rect(Control::Board(Ctl::FindNext)).unwrap();
        click(&mut studio, next.center());
        assert_eq!(studio.board.find.as_ref().unwrap().at, 0);
        let back = studio.control_rect(Control::Board(Ctl::FindBack)).unwrap();
        click(&mut studio, back.center());
        assert_eq!(studio.board.find.as_ref().unwrap().at, count - 1);
    }

    #[test]
    fn what_is_found_is_marked_over_on_the_board_and_esc_takes_the_marks_away() {
        for look in 0..=look::DIAGRAM {
            let mut studio = worded();
            studio.set_look(Some(look), None);
            let mut before = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut before);
            press_key(&mut studio, Key::Char('f'), CTRL);
            type_in(&mut studio, "cache");
            let all = studio.find_all();
            let mut marked = vec![0u32; before.len()];
            studio.draw(&mut marked);
            // Yellow under each, its letters still dark over it.
            for found in &all {
                let (x, y, w, h) = studio.found_pieces(*found)[0];
                let (a, b) = (studio.to_window((x, y)), studio.to_window((x + w, y + h)));
                let yellow = (a.1..b.1).flat_map(|py| (a.0..b.0).map(move |px| (px, py))).filter(|&(px, py)| {
                    let pixel = marked[(py * studio.width + px) as usize];
                    let (r, g, bl) = (pixel >> 16 & 0xFF, pixel >> 8 & 0xFF, pixel & 0xFF);
                    r > 200 && g > 160 && bl < 150
                }).count();
                let area = ((b.0 - a.0) * (b.1 - a.1)) as usize;
                assert!(yellow > area / 3, "look {look}: {found:?} marked, {yellow} of {area}");
            }
            // The one gone to, ringed in the accent.
            let (x, y, w, h) = studio.found_frame(all[0]);
            let (a, b) = (studio.to_window((x, y)), studio.to_window((x + w, y + h)));
            let ringed = (a.1 - 12..b.1 + 12).flat_map(|py| (a.0 - 14..b.0 + 14).map(move |px| (px, py))).any(|(px, py)| marked[(py * studio.width + px) as usize] & 0xFF_FFFF == ACCENT);
            assert!(ringed, "look {look}: ringed");
            // Esc: closed, and drawn as before.
            press_key(&mut studio, Key::Escape, Mods::default());
            assert!(studio.board.find.is_none());
            let mut after = vec![0u32; before.len()];
            studio.draw(&mut after);
            assert!(after == before, "look {look}: the marks gone");
        }
    }

    #[test]
    fn a_click_on_the_board_gives_the_keys_back_and_esc_still_closes_the_bar() {
        let mut studio = worded();
        press_key(&mut studio, Key::Char('f'), CTRL);
        type_in(&mut studio, "notes");
        // The bar's field and its count are where they are drawn.
        let bar = studio.control_rect(Control::Board(Ctl::FindBar)).unwrap();
        assert!((bar.center().0 - studio.open_area().center().0).abs() <= 1 && bar.y < 40, "at the top, in the middle: {bar:?}");
        tap(&mut studio, (900.0, 700.0));
        assert!(!studio.board.find.as_ref().unwrap().typing);
        press_key(&mut studio, Key::Char('r'), Mods::default());
        assert_eq!(studio.board_tool(), BOX_TOOL, "a letter is the board's again");
        assert_eq!(studio.board.find.as_ref().unwrap().typed, "notes");
        press_key(&mut studio, Key::Char('v'), Mods::default());
        // Clicked, the field is typed into again; Ctrl+F picks all of it.
        let field = studio.control_rect(Control::Board(Ctl::FindField)).unwrap();
        click(&mut studio, field.center());
        type_in(&mut studio, "s");
        assert_eq!(studio.board.find.as_ref().unwrap().typed, "notess");
        press_key(&mut studio, Key::Char('f'), CTRL);
        type_in(&mut studio, "cache");
        assert_eq!(studio.board.find.as_ref().unwrap().typed, "cache", "typed over");
        tap(&mut studio, (900.0, 700.0));
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.find.is_none());
        // Its close button closes it too.
        press_key(&mut studio, Key::Char('f'), CTRL);
        let close = studio.control_rect(Control::Board(Ctl::FindClose)).unwrap();
        click(&mut studio, close.center());
        assert!(studio.board.find.is_none());
    }

    /// The find bar over a board, what it found marked, in each look and on
    /// dark paper; and the bar with nothing typed, and with nothing found:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test find_snapshot -- --ignored
    #[test]
    #[ignore]
    fn find_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        let shoot = |studio: &mut Studio, name: &str| {
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            let (x, y, w, h) = (0, 0, 1000, 640);
            let crop: Vec<u32> = (y..y + h).flat_map(|row| out[(row * studio.width + x) as usize..(row * studio.width + x + w) as usize].to_vec()).collect();
            std::fs::write(to.join(format!("{name}.png")), crate::output::encode(w as u32, h as u32, &crop, false).unwrap()).unwrap();
        };
        for (look, ground) in [(0u8, 0u8), (1, 0), (2, 0), (look::DIAGRAM, 0), (0, 1)] {
            let mut studio = worded();
            studio.set_look(Some(look), Some(ground));
            press_key(&mut studio, Key::Char('f'), CTRL);
            type_in(&mut studio, "cache");
            press_key(&mut studio, Key::Enter, Mods::default());
            let next = studio.control_rect(Control::Board(Ctl::FindNext)).unwrap();
            studio.pointer_moved(next.center().0, next.center().1, Mods::default());
            shoot(&mut studio, &format!("find-{look}-{ground}"));
        }
        let mut studio = worded();
        press_key(&mut studio, Key::Char('f'), CTRL);
        shoot(&mut studio, "find-empty");
        type_in(&mut studio, "nowhere");
        shoot(&mut studio, "find-none");
    }
}
