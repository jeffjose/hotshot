//! The library on the canvas, as capture mode has it: J opens it over the
//! bar of tools, and E at its emoji. A line to search all of it in, its
//! shelves down its side, and the labels, notes and emoji on the shelf
//! chosen, each as it will look; one clicked is put on the frame as a
//! layer, a label ready for what it is to say to be typed over it.

use super::*;
use crate::editor::{emoji, emoji_found, library_found, EMOJI_ACROSS, EMOJI_DOWN, EMOJI_SHELF, LIBRARY, SHELVES};

/// What is typed to search it by, the shelf shown, and how far down its
/// emoji have been gone, in rows.
pub(super) struct Library {
    pub query: String,
    pub shelf: u8,
    pub scroll: i32,
}

/// The size of its words, of a cell of an emoji, and the room round it.
const PX: u32 = 13;
const SIDE: i32 = 32;
const PAD: i32 = 12;

impl Studio {
    /// The library opened, at a shelf or at the one it was last at.
    pub(super) fn open_library(&mut self, shelf: Option<u8>) {
        self.commit_text();
        (self.flyout, self.menu, self.styling) = (None, None, false);
        let shelf = shelf.unwrap_or(0);
        self.library = Some(Library { query: String::new(), shelf, scroll: 0 });
        self.dirty = true;
    }

    /// Where the library is and all that is in it, over the bar of tools.
    pub(super) fn layout_library(&mut self, layout: &mut Layout) {
        let Some(library) = &self.library else { return };
        let (rail, each, tall) = (SIDE * 4, (PX as i32 * 19 / 2).max(SIDE * 4), SIDE * 3 / 2);
        let content = (3 * each).max(EMOJI_ACROSS * SIDE);
        let across = rail + PAD + content;
        let searching = !library.query.is_empty();
        let mut cells: Vec<(Rect, Control)> = Vec::new();
        let top = SIDE + PAD / 2;
        for index in 0..SHELVES.len() {
            cells.push((Rect::new(0, top + index as i32 * SIDE, rail, SIDE), Control::Shelf(index as u8)));
        }
        let left = rail + PAD;
        let mut down = top;
        if searching || library.shelf != EMOJI_SHELF {
            let found = library_found(&library.query, library.shelf);
            for (at, &item) in found.iter().enumerate() {
                cells.push((Rect::new(left + at as i32 % 3 * each, down + at as i32 / 3 * tall, each, tall), Control::LibraryItem(item as u16)));
            }
            down += (found.len() as i32 + 2) / 3 * tall;
        }
        if searching || library.shelf == EMOJI_SHELF {
            let found = emoji_found(&library.query);
            let rows = if searching { 3 } else { EMOJI_DOWN };
            for at in 0..EMOJI_ACROSS * rows {
                let index = (library.scroll * EMOJI_ACROSS + at) as usize;
                if index < found.len() {
                    cells.push((Rect::new(left + at % EMOJI_ACROSS * SIDE, down + at / EMOJI_ACROSS * SIDE, SIDE, SIDE), Control::EmojiItem(index as u16)));
                }
            }
            down += ((found.len() as i32 - library.scroll * EMOJI_ACROSS).clamp(0, EMOJI_ACROSS * rows) + EMOJI_ACROSS - 1) / EMOJI_ACROSS * SIDE;
        }
        let down = down.max(top + SHELVES.len() as i32 * SIDE);
        let high = down + PAD / 4 + Text::line_height(PX);
        // Over the middle of the bar, or of the window with the bar hidden.
        let (middle, bottom) = match layout.dock.w {
            0 => (self.open_area().center().0, self.open_area().bottom()),
            _ => (layout.dock.center().0, layout.dock.y - 10),
        };
        let (wide, tall_all) = (across + 2 * PAD, high + 2 * PAD);
        let place = Rect::new((middle - wide / 2).clamp(INSET, (self.width - INSET - wide).max(INSET)), (bottom - tall_all).max(INSET), wide, tall_all);
        let mut front: Vec<(Rect, Control)> = cells.into_iter().map(|(rect, control)| (rect.moved(place.x + PAD, place.y + PAD), control)).collect();
        front.push((place, Control::Panel));
        front.append(&mut layout.controls);
        layout.controls = front;
        layout.library = Some(place);
    }

    /// The library drawn: its search line, its shelves, what is on the
    /// shelf as it will look, and under all of it the name of the one the
    /// pointer is on.
    pub(super) fn draw_library(&mut self, canvas: &mut Canvas, layout: &Layout, place: Rect, hover: Option<Control>) {
        let Some(library) = &self.library else { return };
        let (query, shelf) = (library.query.clone(), library.shelf);
        let origin = (place.x + PAD, place.y + PAD);
        let across = place.w - 2 * PAD;
        let line = Text::line_height(PX);
        // The line a search is typed in, with a caret.
        let field = Rect::new(origin.0, origin.1, across, SIDE).inflate(-2);
        canvas.round_fill(field.x, field.y, field.w, field.h, 6, FIELD_COLOR, 256);
        let words_y = field.y + (field.h - line) / 2;
        self.text.draw_centered(canvas, field.x + 14, field.y + field.h / 2, 13, DIM_TEXT_COLOR, icons::SEARCH);
        match query.is_empty() {
            true => self.text.draw(canvas, field.x + 28, words_y, PX, DIM_TEXT_COLOR, "Type to search"),
            false => self.text.draw(canvas, field.x + 28, words_y, PX, TEXT_COLOR, &query),
        }
        let after = if query.is_empty() { 0 } else { self.text.width(&query, PX) };
        canvas.fill(field.x + 28 + after + 1, words_y + 2, 2, line - 4, ACCENT);
        let found = emoji_found(&query);
        for &(rect, control) in layout.controls.iter().filter(|(rect, _)| place.contains(rect.x, rect.y)) {
            let (cx, cy) = rect.center();
            let inner = rect.inflate(-2);
            if hover == Some(control) && !matches!(control, Control::Panel) {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
            }
            match control {
                // A shelf, by its name: lit when it is the one shown.
                Control::Shelf(index) => {
                    let shown = index == shelf && query.is_empty();
                    if shown {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, ROW_ON, 256);
                    }
                    self.text.draw(canvas, rect.x + 10, cy - line / 2, PX, if shown { TEXT_COLOR } else { DIM_TEXT_COLOR }, SHELVES[usize::from(index)]);
                }
                // A label or a note as it will be put down: in its own
                // color or the one in hand, saying what it will, a little
                // smaller, kept to its cell.
                Control::LibraryItem(item) => {
                    let Some(&(look, _, words, color)) = LIBRARY.get(usize::from(item)) else { continue };
                    let mut badge = Shape::new(Tool::Badge, color.unwrap_or(self.color), 1.0, (0.0, 0.0));
                    (badge.text, badge.font_px, badge.number) = (words.to_owned(), PX * 7 / 8, look as u32);
                    let size = badge.bounds(&mut self.text);
                    badge.points[0] = ((cx - size.w / 2).max(inner.x) as f32, (cy - size.h / 2) as f32);
                    canvas.set_clip(inner.intersect(layout.view));
                    badge.draw(canvas, &mut self.text);
                    canvas.set_clip(layout.view);
                }
                Control::EmojiItem(index) => {
                    if let Some(&which) = found.get(usize::from(index)) {
                        self.text.draw_centered(canvas, cx, cy, PX * 5 / 4, TEXT_COLOR, emoji()[which].0);
                    }
                }
                _ => {}
            }
        }
        // The name of what the pointer is on, or what there is to do.
        let says = match hover {
            Some(Control::LibraryItem(item)) => LIBRARY.get(usize::from(item)).map_or(String::new(), |&(look, ..)| shapes::BADGE_NAMES[look].to_owned()),
            Some(Control::EmojiItem(index)) => found.get(usize::from(index)).map_or(String::new(), |&which| emoji()[which].1.to_owned()),
            _ if !query.is_empty() && found.is_empty() && library_found(&query, shelf).is_empty() => "None by that name".to_owned(),
            _ if shelf == EMOJI_SHELF && query.is_empty() => format!("{} emoji; the wheel goes through them", found.len()),
            _ => "Click one to put it on the frame, then type what it says".to_owned(),
        };
        self.text.draw(canvas, origin.0 + 4, place.bottom() - PAD - line, PX, DIM_TEXT_COLOR, &says);
    }

    /// A key while the library is open: what is typed searches it, Enter
    /// puts down the first found, the arrows go up and down the emoji,
    /// and Esc closes it.
    pub(super) fn library_key(&mut self, key: Key) {
        let Some(library) = &mut self.library else { return };
        match key {
            Key::Char(letter) => (library.query, library.scroll) = (format!("{}{letter}", library.query), 0),
            Key::Backspace => (library.scroll, _) = (0, library.query.pop()),
            Key::Up => self.library_scroll(true),
            Key::Down => self.library_scroll(false),
            Key::Enter => {
                let first = library_found(&library.query, library.shelf).first().copied();
                match first {
                    Some(item) if !library.query.is_empty() || library.shelf != EMOJI_SHELF => self.put_from_library(Control::LibraryItem(item as u16)),
                    _ => self.put_from_library(Control::EmojiItem(0)),
                }
            }
            Key::Escape => self.library = None,
            _ => {}
        }
    }

    /// The emoji gone up or down two rows.
    fn library_scroll(&mut self, up: bool) {
        let Some(library) = &mut self.library else { return };
        let rows = (emoji_found(&library.query).len() as i32 + EMOJI_ACROSS - 1) / EMOJI_ACROSS;
        library.scroll = (library.scroll + if up { -2 } else { 2 }).clamp(0, (rows - EMOJI_DOWN).max(0));
    }

    /// A press while the library is open: the wheel goes through its
    /// emoji, and a click off it closes it. Whether it took the press.
    pub(super) fn library_press(&mut self, button: u8) -> bool {
        if (4..=7).contains(&button) {
            self.library_scroll(matches!(button, 4 | 6));
            return true;
        }
        let inside = self.layout().library.is_some_and(|place| place.contains(self.pointer.0, self.pointer.1));
        if !inside {
            self.library = None;
        }
        !inside
    }

    /// A shelf of the library, or something on it, clicked.
    pub(super) fn press_library(&mut self, control: Control) {
        match control {
            Control::Shelf(shelf) => {
                if let Some(library) = &mut self.library {
                    (library.shelf, library.scroll, library.query) = (shelf, 0, String::new());
                }
            }
            other => self.put_from_library(other),
        }
    }

    /// A label, a note or an emoji put in the middle of what is seen of
    /// the frame, picked out; a label or a note ready for what it says to
    /// be typed over what it says to begin with.
    fn put_from_library(&mut self, control: Control) {
        let (stroke, font_px) = (STROKES[self.size], FONT_SIZES[self.size]);
        let mut shape = match control {
            Control::LibraryItem(item) => {
                let Some(&(look, _, words, color)) = LIBRARY.get(usize::from(item)) else { return };
                let mut shape = Shape::new(Tool::Badge, color.unwrap_or(self.color), stroke, (0.0, 0.0));
                (shape.text, shape.font_px, shape.face, shape.number) = (words.to_owned(), font_px, self.face, look as u32);
                shape
            }
            Control::EmojiItem(index) => {
                let query = self.library.as_ref().map_or(String::new(), |library| library.query.clone());
                let Some(&which) = emoji_found(&query).get(usize::from(index)) else { return };
                let mut shape = Shape::new(Tool::Text, self.color, stroke, (0.0, 0.0));
                (shape.text, shape.font_px) = (emoji()[which].0.to_owned(), font_px * 2);
                shape
            }
            _ => return,
        };
        self.checkpoint();
        self.work_where_seen();
        let size = shape.bounds(&mut self.text);
        let middle = self.to_doc(self.open_area().center());
        let middle = match self.on_page() {
            true => middle,
            false => (middle.0.clamp(0.0, self.doc.width as f32), middle.1.clamp(0.0, self.doc.height as f32)),
        };
        shape.points[0] = ((middle.0 - size.w as f32 / 2.0).round(), (middle.1 - size.h as f32 / 2.0).round());
        let badge = shape.tool == Tool::Badge;
        self.doc.layers.push(Layer::shape(shape));
        let index = self.doc.layers.len() - 1;
        (self.selected, self.tool, self.library) = (vec![index], Tool::Select, None);
        if badge {
            self.edit_text(index);
            self.words_picked = true;
        }
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> Studio {
        Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into())
    }

    fn click(studio: &mut Studio, control: Control) {
        let at = studio.control_rect(control).unwrap_or_else(|| panic!("no {control:?}")).center();
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
    }

    fn typed(studio: &mut Studio, words: &str) {
        for letter in words.chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
    }

    #[test]
    fn j_opens_the_library_and_a_label_from_it_is_put_down_to_be_typed_over() {
        let mut studio = studio();
        studio.key(Key::Char('j'), Mods::default(), true);
        assert!(studio.library.is_some());
        // Its shelves, and the labels on the first.
        let controls: Vec<Control> = studio.layout().controls.iter().map(|(_, control)| *control).collect();
        assert!((0..3).all(|shelf| controls.contains(&Control::Shelf(shelf))));
        let labels = controls.iter().filter(|control| matches!(control, Control::LibraryItem(_))).count();
        assert_eq!(labels, LIBRARY.iter().filter(|item| item.1 == 0).count());
        // Typed, it searches; a click off it closes it and does nothing else.
        typed(&mut studio, "tab");
        let found: Vec<Control> = studio.layout().controls.iter().map(|(_, control)| *control).filter(|control| matches!(control, Control::LibraryItem(_))).collect();
        assert_eq!(found, [Control::LibraryItem(20)], "Opens in a new tab");
        let off = studio.to_window((50.0, 50.0));
        studio.pointer_moved(off.0, off.1, Mods::default());
        studio.pressed(1, 100_000);
        studio.released();
        assert!(studio.library.is_none() && studio.doc.layers.is_empty());
        // From the More tools button, a label clicked is put down on the
        // frame, saying what it says until a letter is typed over it.
        click(&mut studio, Control::Group(MORE));
        click(&mut studio, Control::Pick(Tool::Badge));
        assert!(studio.library.is_some());
        click(&mut studio, Control::LibraryItem(0));
        assert!(studio.library.is_none());
        assert_eq!((studio.doc.layers.len(), studio.selected.clone(), studio.editing), (1, vec![0], Some(0)));
        typed(&mut studio, "Shipped");
        studio.key(Key::Escape, Mods::default(), true);
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((shape.tool, shape.text.as_str(), shape.number), (Tool::Badge, "Shipped", 12));
        let (x, y, w, h) = studio.doc.layers[0].bounds(&mut studio.text);
        assert!(x > 0.0 && y > 0.0 && x + w < 800.0 && y + h < 600.0, "on the frame");
        // Enter types into it again, after what it says.
        studio.key(Key::Enter, Mods::default(), true);
        typed(&mut studio, "!");
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!(shape.text, "Shipped!");
    }

    #[test]
    fn e_opens_the_emoji_and_the_wheel_and_enter_choose_among_them() {
        let mut studio = studio();
        studio.key(Key::Char('e'), Mods::default(), true);
        assert_eq!(studio.library.as_ref().map(|library| library.shelf), Some(EMOJI_SHELF));
        let first = studio.layout().controls.iter().any(|(_, control)| *control == Control::EmojiItem(0));
        assert!(first);
        let middle = studio.layout().library.unwrap().center();
        studio.pointer_moved(middle.0, middle.1, Mods::default());
        studio.pressed(5, 100_000);
        assert_eq!(studio.library.as_ref().map(|library| library.scroll), Some(2));
        typed(&mut studio, "fire");
        studio.key(Key::Enter, Mods::default(), true);
        let Some(Layer { kind: Kind::Shape(shape), .. }) = studio.doc.layers.first() else { panic!("an emoji is put down") };
        let fire = emoji_found("fire")[0];
        assert_eq!((shape.tool, shape.text.as_str(), studio.editing), (Tool::Text, emoji()[fire].0, None));
    }
}
