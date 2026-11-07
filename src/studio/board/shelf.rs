//! A board's library of pieces, as Design's library is: J, its button at
//! the end of the bar of tools, or Ctrl+K opens it over the bar. Its own
//! pieces are a database, a queue, a cloud, a user, a phone, a cache, a
//! server and a browser's window, each drawn as a board's shapes; and
//! under them those kept from the board, what was picked out saved as a
//! piece of its own. One clicked is put in the middle of what is seen, one
//! dragged where it is let go: in the board's look, its colors, its font,
//! and as large to the eye as at 100%.

use super::*;

/// A piece kept: what it is called, and its shapes, their top left at
/// nought and as large as at 100%.
#[derive(Clone)]
pub(in crate::studio) struct Piece {
    pub name: String,
    pub shapes: Vec<Shape>,
}

/// The library's own pieces: what each is called, what its box is drawn
/// as, how large it is, and whether its words are a title over what is
/// put in it, as a window's are.
const STOCK: [(&str, u8, (f32, f32), bool); 8] = [
    ("Database", shapes::FORM_CYLINDER, (120.0, 96.0), false),
    ("Queue", shapes::FORM_STADIUM, (170.0, 56.0), false),
    ("Cloud", shapes::FORM_CLOUD, (170.0, 104.0), false),
    ("User", shapes::FORM_PERSON, (96.0, 112.0), false),
    ("Phone", shapes::FORM_BOX, (84.0, 148.0), false),
    ("Cache", shapes::FORM_HEXAGON, (150.0, 64.0), false),
    ("Server", shapes::FORM_SUBROUTINE, (160.0, 72.0), false),
    ("Browser", shapes::FORM_BOX, (240.0, 160.0), true),
];

/// How many pieces a row of it has, how large each is, and the room
/// between them and round them all.
const ACROSS: i32 = 4;
const CELL: (i32, i32) = (104, 96);
const GAP: i32 = 6;
const PAD: i32 = 14;
const BUTTON: i32 = 32;

/// The file the kept pieces are in, among hotshot's state.
const KEPT: &str = "board-library";

/// The pieces kept in a file of them: each a line naming it, then a line
/// for each of its shapes.
fn read_pieces(words: &str) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    for line in words.lines() {
        match line.strip_prefix("piece ") {
            Some(name) => pieces.push(Piece { name: name.to_owned(), shapes: Vec::new() }),
            None => {
                if let (Some(piece), Some(shape)) = (pieces.last_mut(), Shape::read(line)) {
                    piece.shapes.push(shape);
                }
            }
        }
    }
    pieces.retain(|piece| !piece.shapes.is_empty());
    pieces
}

fn write_pieces(pieces: &[Piece]) -> String {
    let mut out = String::new();
    for piece in pieces {
        out += &format!("piece {}\n", one_line(&piece.name));
        for shape in &piece.shapes {
            out += &shape.write();
            out.push('\n');
        }
    }
    out
}

impl Studio {
    /// The pieces kept, read from hotshot's state the first time they are
    /// asked for; none under test, which keeps its own as it goes.
    fn kept_pieces(&mut self) -> &mut Vec<Piece> {
        if self.board.pieces.is_none() {
            let read = match cfg!(test) {
                true => Vec::new(),
                false => crate::output::state_file(KEPT).and_then(|file| std::fs::read_to_string(file).ok()).map(|words| read_pieces(&words)).unwrap_or_default(),
            };
            self.board.pieces = Some(read);
        }
        self.board.pieces.get_or_insert_with(Vec::new)
    }

    /// The pieces kept written to hotshot's state, but under test.
    fn keep_pieces(&mut self) {
        if cfg!(test) {
            return;
        }
        let words = write_pieces(self.kept_pieces());
        let Some(file) = crate::output::state_file(KEPT) else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, words);
    }

    /// J, its button or Ctrl+K: the library open, or shut.
    pub(in crate::studio) fn toggle_shelf(&mut self) {
        self.commit_text();
        self.board.shelf = !self.board.shelf;
        self.menu = None;
        self.dirty = true;
    }

    /// How many pieces there are of each: the library's own, and those kept.
    fn piece_counts(&mut self) -> (usize, usize) {
        (STOCK.len(), self.kept_pieces().len())
    }

    /// A piece's shapes, about to be put down, as the board would draw them
    /// now: one of its own in the board's line, fill and font, as large to
    /// the eye as at 100%; one kept in its own colors, in the board's font
    /// and as large. Their top left at nought, and what each is called new.
    pub(in crate::studio) fn piece_shapes(&mut self, which: usize) -> Vec<Shape> {
        let by = self.dynamic();
        let font = self.board_font();
        if let Some(&(name, form, (w, h), titled)) = STOCK.get(which) {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, (0.0, 0.0));
            self.board_dress(&mut shape);
            (shape.form, shape.text, shape.titled, shape.heading) = (form, name.to_owned(), titled, titled);
            shape.points = vec![(0.0, 0.0), (w * by, h * by)];
            shape.base = (w * by, h * by);
            shape.touch();
            return vec![shape];
        }
        let Some(piece) = self.kept_pieces().get(which - STOCK.len()).cloned() else { return Vec::new() };
        // Each named anew, what an arrow is stuck to followed.
        let named: Vec<(u32, Shape)> = piece.shapes.iter().map(|shape| (shape.id, shape.copied())).collect();
        named.iter().zip(&piece.shapes).map(|((_, made), was)| {
            let mut shape = scaled(made, by);
            shape.stuck = was.stuck.map(|end| end.and_then(|(id, u, v)| named.iter().find(|(old, _)| *old == id).map(|(_, new)| (new.id, u, v))));
            if shape.tool != Tool::Rect || shape.form != shapes::FORM_CODE {
                shape.face = font;
            }
            shape.touch();
            shape
        }).collect()
    }

    /// A piece put on the board, its middle at a point of it: picked out,
    /// the pointer in hand, the library shut. One step to undo.
    pub(in crate::studio) fn put_piece(&mut self, which: usize, at: (f32, f32)) {
        let mut shapes = self.piece_shapes(which);
        if shapes.is_empty() {
            return;
        }
        self.commit_text();
        self.checkpoint();
        let frames: Vec<(f32, f32, f32, f32)> = shapes.iter().map(Shape::frame).collect();
        let Some((x, y, w, h)) = crate::studio::actions::union(&frames) else { return };
        let start = self.doc.layers.len();
        for shape in &mut shapes {
            shape.translate(at.0 - (x + w / 2.0), at.1 - (y + h / 2.0));
        }
        self.doc.layers.extend(shapes.into_iter().map(Layer::shape));
        (self.selected, self.tool, self.board.shelf) = ((start..self.doc.layers.len()).collect(), Tool::Select, false);
        self.changed();
        let name = STOCK.get(which).map_or_else(|| self.kept_pieces().get(which - STOCK.len()).map_or(String::new(), |piece| piece.name.clone()), |stock| stock.0.to_owned());
        self.say(format!("{name} from the library"));
    }

    /// A piece pressed: put down under the pointer and taken hold of, to be
    /// dragged where it goes; let go where it was pressed, it goes in the
    /// middle of what is seen.
    fn press_piece(&mut self, which: usize) {
        let at = self.to_doc(self.pointer);
        self.put_piece(which, at);
        if self.selected.is_empty() {
            return;
        }
        self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
        self.drag = Drag::Move { from: at, moved: true, copy: false };
        self.board.placing = Some(self.pointer);
    }

    /// A piece let go: where it was pressed, it is put in the middle of
    /// what is seen instead.
    pub(in crate::studio) fn piece_let_go(&mut self) {
        let Some(from) = self.board.placing.take() else { return };
        if (self.pointer.0 - from.0).abs().max((self.pointer.1 - from.1).abs()) > 3 {
            return;
        }
        let frames: Vec<(f32, f32, f32, f32)> = self.chosen().iter().map(|&index| self.layer_frame(index)).collect();
        let Some((x, y, w, h)) = crate::studio::actions::union(&frames) else { return };
        let middle = self.to_doc(self.open_area().center());
        let (dx, dy) = ((middle.0 - (x + w / 2.0)).round(), (middle.1 - (y + h / 2.0)).round());
        for index in self.chosen() {
            self.doc.layers[index].translate(dx, dy);
        }
        self.changed();
    }

    /// What is picked out kept in the library as a piece of its own: its
    /// shapes, as large as at 100%, named for the first words among them.
    pub(in crate::studio) fn save_piece(&mut self) {
        let by = self.dynamic();
        let shapes: Vec<Shape> = self.chosen().into_iter().filter_map(|index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(scaled(shape, 1.0 / by)),
            Kind::Picture(_) => None,
        }).collect();
        if shapes.is_empty() {
            self.say("Pick out some shapes to keep in the library");
            return;
        }
        let frames: Vec<(f32, f32, f32, f32)> = shapes.iter().map(Shape::frame).collect();
        let (x, y, _, _) = crate::studio::actions::union(&frames).unwrap_or_default();
        let shapes: Vec<Shape> = shapes.into_iter().map(|mut shape| {
            shape.translate(-x, -y);
            shape.editing = false;
            shape
        }).collect();
        let count = self.kept_pieces().len();
        let words = shapes.iter().map(|shape| shape.text.lines().next().unwrap_or("").trim().to_owned()).find(|words| !words.is_empty());
        let name = words.map_or_else(|| format!("Piece {}", count + 1), |words| words.chars().take(24).collect());
        self.kept_pieces().push(Piece { name: name.clone(), shapes });
        self.keep_pieces();
        self.board.shelf = true;
        self.dirty = true;
        self.say(format!("Kept in the library: {name}"));
    }

    /// The library's place, over the middle of the bar of tools, and its
    /// pieces: its own in rows, then those kept, then the button to keep
    /// what is picked out.
    pub(in crate::studio) fn layout_shelf(&mut self, layout: &mut Layout) {
        if !self.board.shelf || self.zen || self.presenting() {
            return;
        }
        let (own, kept) = self.piece_counts();
        let rows = |count: usize| (count as i32 + ACROSS - 1) / ACROSS;
        let inner = ACROSS * CELL.0 + (ACROSS - 1) * GAP;
        let heading = Text::line_height(13) + 12;
        let kept_heading = if kept > 0 { Text::line_height(11) + 10 } else { 0 };
        let grid = |count: usize| rows(count) * (CELL.1 + GAP);
        let high = PAD + heading + grid(own) + kept_heading + grid(kept) + 4 + BUTTON + 8 + Text::line_height(11) + PAD;
        let (middle, bottom) = match layout.dock.w {
            0 => (self.open_area().center().0, self.open_area().bottom()),
            _ => (layout.dock.center().0, layout.dock.y - 10),
        };
        let wide = inner + 2 * PAD;
        let place = Rect::new((middle - wide / 2).clamp(INSET, (self.width - INSET - wide).max(INSET)), (bottom - high).max(INSET), wide, high);
        let mut front = Vec::new();
        let mut down = place.y + PAD + heading;
        for (first, count) in [(0, own), (own, kept)] {
            if first > 0 && count > 0 {
                down += kept_heading;
            }
            for at in 0..count {
                let (column, row) = (at as i32 % ACROSS, at as i32 / ACROSS);
                front.push((Rect::new(place.x + PAD + column * (CELL.0 + GAP), down + row * (CELL.1 + GAP), CELL.0, CELL.1), Control::Board(Ctl::Piece((first + at) as u16))));
            }
            down += grid(count);
        }
        front.push((Rect::new(place.x + PAD, down + 4, inner, BUTTON), Control::Board(Ctl::SavePiece)));
        front.push((place, Control::Panel));
        front.append(&mut layout.controls);
        layout.controls = front;
        layout.shelf = Some(place);
    }

    /// The library drawn: its name, each piece on a patch of the board's
    /// paper as the board would draw it, with its name under it; the
    /// heading of those kept; the button; and what the one the pointer is
    /// on is, or what to do.
    pub(in crate::studio) fn draw_shelf(&mut self, canvas: &mut Canvas, layout: &Layout, place: Rect, hover: Option<Control>) {
        let (own, kept) = self.piece_counts();
        self.text.draw(canvas, place.x + PAD, place.y + PAD - 2, 13, TEXT_COLOR, "Library");
        let key = "J";
        let cap = self.text.width(key, 11) + 12;
        canvas.round_fill(place.right() - PAD - cap, place.y + PAD - 3, cap, 20, 5, FIELD_COLOR, 256);
        self.text.draw(canvas, place.right() - PAD - cap + 6, place.y + PAD + 7 - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, key);
        let (look, ground) = (self.pages[self.page].look, self.pages[self.page].ground);
        let (paper, _) = look::paper_of(look, ground);
        let mut first_kept = None;
        for &(rect, control) in layout.controls.iter().filter(|(rect, _)| place.contains(rect.x, rect.y)) {
            let lit = hover == Some(control);
            match control {
                Control::Board(Ctl::Piece(which)) => {
                    let which = usize::from(which);
                    if which >= own && first_kept.is_none() {
                        first_kept = Some(rect.y);
                    }
                    let tile = Rect::new(rect.x, rect.y, rect.w, rect.h - 20);
                    canvas.round_fill(tile.x, tile.y, tile.w, tile.h, 8, paper, 256);
                    canvas.round_outline(tile.x, tile.y, tile.w, tile.h, 8, if lit { 2 } else { 1 }, if lit { ACCENT } else { EDGE_COLOR });
                    // The library's own without their words, too small to
                    // read there and said under each.
                    let mut shapes = self.piece_shapes(which);
                    if which < own {
                        shapes.iter_mut().for_each(|shape| shape.text.clear());
                    }
                    self.draw_piece(canvas, tile.inflate(-6), &shapes, look, ground, layout.view);
                    let name = match STOCK.get(which) {
                        Some(stock) => stock.0.to_owned(),
                        None => self.kept_pieces().get(which - own).map_or(String::new(), |piece| piece.name.clone()),
                    };
                    let name = fit_name(&mut self.text, &name, rect.w - 4);
                    let wide = self.text.width(&name, 11);
                    self.text.draw(canvas, rect.x + (rect.w - wide) / 2, rect.bottom() - 16, 11, if lit { TEXT_COLOR } else { DIM_TEXT_COLOR }, &name);
                }
                Control::Board(Ctl::SavePiece) => {
                    let can = !self.chosen().is_empty();
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if lit && can { HOVER_COLOR } else { FIELD_COLOR }, 256);
                    let says = "Save what is picked out";
                    let wide = self.text.width(says, 12) + 18;
                    let left = rect.x + (rect.w - wide) / 2;
                    let color = if can { TEXT_COLOR } else { 0x6A6A6A };
                    self.text.draw_centered(canvas, left + 6, rect.y + rect.h / 2, 12, color, icons::PLUS);
                    self.text.draw(canvas, left + 18, rect.y + (rect.h - Text::line_height(12)) / 2, 12, color, says);
                }
                _ => {}
            }
        }
        if let (Some(top), true) = (first_kept, kept > 0) {
            self.text.draw(canvas, place.x + PAD, top - Text::line_height(11) - 6, 11, DIM_TEXT_COLOR, "SAVED");
        }
        let says = match hover {
            Some(Control::Board(Ctl::SavePiece)) if self.chosen().is_empty() => "Pick out shapes on the board to keep them here".to_owned(),
            Some(Control::Board(Ctl::SavePiece)) => "Keep what is picked out as a piece of the library".to_owned(),
            Some(Control::Board(Ctl::Piece(_))) => "Click to put it in the middle, or drag it where it goes".to_owned(),
            _ => "Pieces drawn in the board's look and colors".to_owned(),
        };
        self.text.draw(canvas, place.x + PAD, place.bottom() - PAD - Text::line_height(11) + 2, 11, DIM_TEXT_COLOR, &says);
    }

    /// A piece drawn into a patch, as the board's look draws it, as large
    /// as fits and no larger than it is.
    fn draw_piece(&mut self, canvas: &mut Canvas, within: Rect, shapes: &[Shape], look: u8, ground: u8, view: Rect) {
        let frames: Vec<(f32, f32, f32, f32)> = shapes.iter().map(|shape| {
            let place = shape.local_bounds(&mut self.text);
            (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
        }).collect();
        let Some((x, y, w, h)) = crate::studio::actions::union(&frames) else { return };
        let by = (within.w as f32 / w.max(1.0)).min(within.h as f32 / h.max(1.0)).min(1.0);
        let (left, top) = (within.x as f32 + (within.w as f32 - w * by) / 2.0, within.y as f32 + (within.h as f32 - h * by) / 2.0);
        canvas.set_clip(within.inflate(6).intersect(view));
        for shape in shapes {
            let mut shape = scaled(shape, by);
            shape.translate(left - x * by, top - y * by);
            look::dress(&mut shape, look, ground, by);
            shape.draw(canvas, &mut self.text);
        }
        canvas.set_clip(view);
    }

    /// A piece of the library pressed, or its button.
    pub(in crate::studio) fn press_shelf(&mut self, ctl: Ctl) {
        match ctl {
            Ctl::Piece(which) => self.press_piece(usize::from(which)),
            Ctl::SavePiece => self.save_piece(),
            _ => {}
        }
    }

    /// A press while the library is open, off it and off its button: the
    /// library shut, and nothing else done. Whether it took the press.
    pub(in crate::studio) fn shelf_press(&mut self) -> bool {
        if !self.board.shelf {
            return false;
        }
        let control = self.control_at(self.pointer);
        let on = self.layout().shelf.is_some_and(|place| place.contains(self.pointer.0, self.pointer.1)) || control == Some(Control::Board(Ctl::Tool(LIBRARY)));
        if !on {
            self.board.shelf = false;
            self.dirty = true;
        }
        !on
    }
}

/// A name cut short with an ellipsis to fit so wide.
fn fit_name(text: &mut Text, name: &str, wide: i32) -> String {
    if text.width(name, 11) <= wide {
        return name.to_owned();
    }
    let mut cut: String = name.to_owned();
    while !cut.is_empty() && text.width(&format!("{cut}\u{2026}"), 11) > wide {
        cut.pop();
    }
    format!("{}\u{2026}", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn j_opens_the_library_and_a_piece_clicked_goes_in_the_middle_in_the_board_s_colors() {
        let mut studio = board();
        studio.press_style(Ctl::Color(5));
        press_key(&mut studio, Key::Char('j'), Mods::default());
        assert!(studio.board.shelf && studio.layout().shelf.is_some());
        // Over the bar of tools, its button lit.
        let place = studio.layout().shelf.unwrap();
        let dock = studio.layout().dock;
        assert!(place.bottom() <= dock.y && (place.center().0 - dock.center().0).abs() <= 1, "{place:?} over {dock:?}");
        let database = studio.control_rect(Control::Board(Ctl::Piece(0))).unwrap().center();
        click(&mut studio, database);
        assert!(!studio.board.shelf, "shut once one is taken");
        let made = shape(&studio, 0);
        assert_eq!((made.form, made.text.as_str(), made.color, made.fill, made.face), (shapes::FORM_CYLINDER, "Database", SWATCHES[5].0, SWATCHES[5].1, studio.board_font()));
        let middle = studio.to_doc(studio.open_area().center());
        let (x, y, w, h) = made.frame();
        assert!(((x + w / 2.0) - middle.0).abs() < 1.0 && ((y + h / 2.0) - middle.1).abs() < 1.0, "in the middle");
        assert_eq!(studio.selected, vec![0]);
        // One step to undo.
        studio.undo();
        assert!(studio.doc.layers.is_empty());
        // Dragged out, where it is let go; zoomed out, as large to the eye.
        studio.zoom = 0.5;
        studio.changed();
        press_key(&mut studio, Key::Char('j'), Mods::default());
        let cloud = studio.control_rect(Control::Board(Ctl::Piece(2))).unwrap().center();
        studio.pointer_moved(cloud.0, cloud.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.to_window((300.0, 200.0));
        studio.pointer_moved((cloud.0 + to.0) / 2, (cloud.1 + to.1) / 2, Mods::default());
        studio.pointer_moved(to.0, to.1, Mods::default());
        studio.released();
        let made = shape(&studio, 0);
        let (x, y, w, h) = made.frame();
        assert!(((x + w / 2.0) - 300.0).abs() < 2.5 && ((y + h / 2.0) - 200.0).abs() < 2.5, "where it was let go: {:?}", made.frame());
        assert_eq!((made.form, (w / 2.0).round(), made.stroke), (shapes::FORM_CLOUD, 170.0, LINE * 2.0));
        studio.undo();
        assert!(studio.doc.layers.is_empty(), "the drag one step with it");
        // Esc, or a click off it, shuts it.
        press_key(&mut studio, Key::Char('j'), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(!studio.board.shelf);
        press_key(&mut studio, Key::Char('j'), Mods::default());
        click(&mut studio, (100, 300));
        assert!(!studio.board.shelf && studio.doc.layers.is_empty());
    }

    #[test]
    fn what_is_picked_out_is_kept_in_the_library_and_put_down_again_joined_as_it_was() {
        let mut studio = board();
        let put = |studio: &mut Studio, x: f32, words: &str| {
            let mut shape = Shape::new(Tool::Rect, 0x7048E8, LINE, (x, 100.0));
            studio.board_dress(&mut shape);
            (shape.points[1], shape.text, shape.color) = ((x + 120.0, 160.0), words.into(), 0x7048E8);
            studio.doc.layers.push(Layer::shape(shape));
            studio.doc.layers.len() - 1
        };
        let (a, b) = (put(&mut studio, 100.0, "Client"), put(&mut studio, 400.0, "Server"));
        let arrow = studio.join(a, b);
        studio.selected = vec![a, b, arrow];
        // From the right click.
        let at = studio.to_window((160.0, 130.0));
        studio.right_click(at);
        let row = studio.menu_items(MenuKind::Context).iter().position(|item| matches!(item, crate::studio::menu::Item::Entry { says, .. } if says == "Save to library")).unwrap();
        let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
        click(&mut studio, rect.center());
        assert!(studio.board.shelf, "shown kept");
        let kept = STOCK.len() as u16;
        assert_eq!(studio.board.pieces.as_ref().map(|pieces| pieces[0].name.clone()), Some("Client".to_owned()));
        // Put down again: its own colors, its arrow joining the new two.
        let piece = studio.control_rect(Control::Board(Ctl::Piece(kept))).unwrap().center();
        click(&mut studio, piece);
        let made: Vec<Shape> = (3..studio.doc.layers.len()).map(|index| shape(&studio, index)).collect();
        assert_eq!(made.len(), 3);
        assert!(made.iter().filter(|shape| shape.tool == Tool::Rect).all(|shape| shape.color == 0x7048E8), "in its own colors");
        let ids: Vec<u32> = made.iter().map(|shape| shape.id).collect();
        let joined = made.iter().find(|shape| shape.tool == Tool::Arrow).unwrap();
        assert!(joined.stuck.iter().all(|end| end.is_some_and(|(id, ..)| ids.contains(&id))), "joined to the new two");
        assert!(made.iter().all(|shape| shape.id != studio.id_of(a) && shape.id != studio.id_of(b)));
    }

    #[test]
    fn pieces_kept_are_read_back_as_they_were_written() {
        let mut first = Shape::new(Tool::Rect, 0x1971C2, 2.0, (0.0, 0.0));
        first.points[1] = (120.0, 60.0);
        first.text = "API\tgateway".into();
        let pieces = vec![Piece { name: "API".into(), shapes: vec![first.clone()] }, Piece { name: "Empty".into(), shapes: Vec::new() }];
        let back = read_pieces(&write_pieces(&pieces));
        assert_eq!(back.len(), 1, "a piece of nothing is not kept");
        assert_eq!((back[0].name.as_str(), back[0].shapes[0].text.as_str(), back[0].shapes[0].frame()), ("API", "API\tgateway", first.frame()));
    }
}
