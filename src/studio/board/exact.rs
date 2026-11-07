//! The exact size and place of what is picked out on a board, at the foot
//! of its panel: its width, height, left and top, each a number to type
//! into, Enter to set it, Up and Down a pixel at a time, ten with Shift;
//! and Snap to the dots, the board's own and kept with it, by which what
//! is drawn, moved and sized lands on its dots, as arrows go with it.

use super::*;

/// What each of the numbers is called, as the panel says it.
pub(in crate::studio) const EXACT: [&str; 4] = ["W", "H", "X", "Y"];

/// A number as the panel shows it: whole where it is, else to a tenth.
fn shown(value: f32) -> String {
    let tenth = (value * 10.0).round() / 10.0;
    match tenth.fract() == 0.0 {
        true => format!("{}", tenth as i64),
        false => format!("{tenth:.1}"),
    }
}

impl Studio {
    /// What a layer covers as it is placed, not its words past its edge
    /// nor its stamps: a shape's frame, words as they are set, a picture.
    pub(in crate::studio) fn layer_frame(&mut self, index: usize) -> (f32, f32, f32, f32) {
        match &self.doc.layers[index].kind {
            Kind::Shape(shape) if matches!(shape.tool, Tool::Text | Tool::Badge | Tool::Counter) => {
                let place = shape.local_bounds(&mut self.text);
                (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
            }
            Kind::Shape(shape) => shape.frame(),
            Kind::Picture(picture) => (picture.cx - picture.w / 2.0, picture.cy - picture.h / 2.0, picture.w, picture.h),
        }
    }

    /// What all that is picked out covers, as one.
    fn exact_frame(&mut self) -> Option<(f32, f32, f32, f32)> {
        let frames: Vec<(f32, f32, f32, f32)> = self.chosen().into_iter().map(|index| self.layer_frame(index)).collect();
        crate::studio::actions::union(&frames)
    }

    /// The width, height, left or top of what is picked out, on the board.
    pub(in crate::studio) fn exact_value(&mut self, which: u8) -> Option<f32> {
        let (x, y, w, h) = self.exact_frame()?;
        let (ox, oy) = (self.doc.x, self.doc.y);
        Some([w, h, x + ox, y + oy][usize::from(which.min(3))])
    }

    /// What is picked out made so wide, so high, or put so far across or
    /// down, as one step to undo: one box sized from its top left, its
    /// words fitted to it as when sized by hand; more than one sized as
    /// one from their top left; and what is joined to them following.
    pub(in crate::studio) fn set_exact(&mut self, which: u8, value: f32) {
        let Some((x, y, w, h)) = self.exact_frame() else { return };
        if !value.is_finite() {
            return;
        }
        let chosen = self.chosen();
        self.checkpoint();
        match which {
            2 | 3 => {
                let (ox, oy) = (self.doc.x, self.doc.y);
                let (dx, dy) = if which == 2 { (value - (x + ox), 0.0) } else { (0.0, value - (y + oy)) };
                for &index in &chosen {
                    self.doc.layers[index].translate(dx, dy);
                }
            }
            _ => {
                let value = value.max(1.0);
                let single = match &chosen[..] {
                    [one] if self.is_box(*one) => Some(*one),
                    _ => None,
                };
                match single {
                    Some(index) => {
                        if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                            let (bx, by, bw, bh) = shape.frame();
                            let (nw, nh) = if which == 0 { (value, bh) } else { (bw, value) };
                            shape.points = vec![(bx, by), (bx + nw, by + nh)];
                            shape.base = (nw, nh);
                            shape.touch();
                        }
                        // Its words fitted to the size it is now.
                        if matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.nominal > 0) || self.is_block(index) {
                            self.fit_words(index, false);
                        }
                    }
                    None => {
                        let by = if which == 0 { (value / w.max(1e-3), 1.0) } else { (1.0, value / h.max(1e-3)) };
                        for &index in &chosen {
                            crate::studio::actions::scale_layer(&mut self.doc.layers[index], (x, y), by);
                        }
                    }
                }
            }
        }
        self.changed();
        self.say(format!("{} {}", EXACT[usize::from(which.min(3))], shown(value)));
    }

    /// A number of the panel clicked: typed into, what it was shown faintly
    /// to type over; and the switch, the dots snapped to or not.
    pub(in crate::studio) fn press_exact(&mut self, ctl: Ctl) {
        match ctl {
            Ctl::Exact(which) => {
                if self.board.exact_typed.as_ref().is_none_or(|(typed, _)| *typed != which) {
                    self.commit_exact();
                    self.board.exact_typed = Some((which, String::new()));
                }
            }
            Ctl::Snap => self.toggle_snap(),
            _ => {}
        }
        self.dirty = true;
    }

    /// What was typed into a number set, if it is one.
    pub(in crate::studio) fn commit_exact(&mut self) {
        let Some((which, typed)) = self.board.exact_typed.take() else { return };
        if let Ok(value) = typed.trim().parse::<f32>() {
            self.set_exact(which, value);
        }
    }

    /// A key while a number of the panel is typed into: digits, a point and
    /// a minus; Backspace; Enter to set it, Tab to set it and go on to the
    /// next; Up and Down a pixel more or less at once, ten with Shift; Esc
    /// to leave it as it was. Whether it was taken.
    pub(in crate::studio) fn exact_key(&mut self, key: Key, mods: Mods) -> bool {
        let Some((which, typed)) = self.board.exact_typed.clone() else { return false };
        if mods.ctrl {
            return false;
        }
        match key {
            Key::Char(letter) if (letter.is_ascii_digit() || letter == '.' || letter == '-') && typed.len() < 8 => self.board.exact_typed = Some((which, format!("{typed}{letter}"))),
            Key::Backspace => self.board.exact_typed = Some((which, typed[..typed.len().saturating_sub(1)].to_owned())),
            Key::Enter => self.commit_exact(),
            Key::Tab => {
                self.commit_exact();
                let next = if mods.shift { (which + 3) % 4 } else { (which + 1) % 4 };
                self.board.exact_typed = Some((next, String::new()));
            }
            Key::Up | Key::Down => {
                let now = typed.trim().parse::<f32>().ok().or_else(|| self.exact_value(which));
                if let Some(now) = now {
                    let step = if mods.shift { 10.0 } else { 1.0 } * if key == Key::Up { 1.0 } else { -1.0 };
                    let value = (now + step).round();
                    self.set_exact(which, value);
                    self.board.exact_typed = Some((which, String::new()));
                }
            }
            Key::Escape => self.board.exact_typed = None,
            _ => {}
        }
        self.dirty = true;
        true
    }

    /// Whether what is drawn, moved and sized on the board shown lands on
    /// its dots.
    pub(in crate::studio) fn snap_on(&self) -> bool {
        self.on_board() && self.pages[self.page].snap
    }

    /// Snap to the dots, on or off, for the board shown, kept with it.
    pub(in crate::studio) fn toggle_snap(&mut self) {
        let page = &mut self.pages[self.page];
        page.snap = !page.snap;
        let on = page.snap;
        self.edited = true;
        self.dirty = true;
        self.say(if on { "Snapping to the dots" } else { "Snapping off" });
    }

    /// A point of what is worked in moved to the nearest dot of the paper.
    pub(in crate::studio) fn to_dots(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let (ox, oy) = (self.doc.x, self.doc.y);
        let near = |at: f32, origin: f32| ((at + origin) / DOT_STEP).round() * DOT_STEP - origin;
        (near(x, ox), near(y, oy))
    }

    /// How far what is held is to go, moved by so much, for its top left
    /// to be on a dot.
    pub(in crate::studio) fn dots_by(&mut self, dx: f32, dy: f32) -> (f32, f32) {
        let frames: Vec<(f32, f32, f32, f32)> = self.held.iter().map(|layer| match &layer.kind {
            Kind::Shape(shape) if !matches!(shape.tool, Tool::Text | Tool::Badge | Tool::Counter) => shape.frame(),
            Kind::Shape(shape) => {
                let place = shape.local_bounds(&mut self.text);
                (place.x as f32, place.y as f32, place.w as f32, place.h as f32)
            }
            Kind::Picture(picture) => (picture.cx - picture.w / 2.0, picture.cy - picture.h / 2.0, picture.w, picture.h),
        }).collect();
        let Some((x, y, _, _)) = crate::studio::actions::union(&frames) else { return (dx, dy) };
        let (tx, ty) = self.to_dots((x + dx, y + dy));
        (tx - x, ty - y)
    }

    /// The sides of the box picked out that a handle pulled, put on the
    /// nearest dots.
    pub(in crate::studio) fn snap_pulled(&mut self, pull: (i32, i32)) {
        let Some(index) = self.picked().filter(|&index| self.is_box(index)) else { return };
        let Kind::Shape(shape) = &self.doc.layers[index].kind else { return };
        if shape.angle != 0.0 {
            return;
        }
        let (x, y, w, h) = shape.frame();
        let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
        let (a, b) = (self.to_dots((left, top)), self.to_dots((right, bottom)));
        match pull.0 {
            -1 => left = a.0.min(right - 1.0),
            1 => right = b.0.max(left + 1.0),
            _ => {}
        }
        match pull.1 {
            -1 => top = a.1.min(bottom - 1.0),
            1 => bottom = b.1.max(top + 1.0),
            _ => {}
        }
        if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
            shape.points = vec![(left, top), (right, bottom)];
            shape.touch();
        }
    }

    /// A number of the panel, or the switch, drawn: the number's letter
    /// faint at its left and its value after, a field lit as it is typed
    /// into, a dash where nothing is picked out; the switch a pill, lit and
    /// its knob at the right while on.
    pub(in crate::studio) fn draw_exact_control(&mut self, canvas: &mut Canvas, rect: Rect, ctl: Ctl, hovered: bool) {
        let (_, cy) = rect.center();
        match ctl {
            Ctl::Exact(which) => {
                let typed = self.board.exact_typed.clone().filter(|(at, _)| *at == which).map(|(_, typed)| typed);
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                if typed.is_some() || hovered {
                    canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if typed.is_some() { ACCENT } else { 0x555555 });
                }
                let letter = EXACT[usize::from(which.min(3))];
                self.text.draw(canvas, rect.x + 9, cy - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, letter);
                let (says, color) = match typed.as_deref() {
                    Some(typed) if !typed.is_empty() => (typed.to_owned(), TEXT_COLOR),
                    shown_now => (self.exact_value(which).map_or_else(|| "\u{2013}".to_owned(), shown), if shown_now.is_some() { DIM_TEXT_COLOR } else { TEXT_COLOR }),
                };
                let left = rect.x + 24;
                self.text.draw(canvas, left, cy - Text::line_height(12) / 2, 12, color, &says);
                if typed.is_some() {
                    let at = if typed.as_deref() == Some("") { left - 2 } else { left + self.text.width(&says, 12) + 1 };
                    canvas.fill(at, cy - 7, 1, 14, ACCENT);
                }
            }
            Ctl::Snap => {
                let on = self.pages[self.page].snap;
                if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                // A grid of four dots, and what it says.
                for (dx, dy) in [(0, 0), (6, 0), (0, 6), (6, 6)] {
                    canvas.disc(rect.x + 9 + dx, cy - 3 + dy, 1, if on { ACCENT_TEXT } else { DIM_TEXT_COLOR }, 256);
                }
                self.text.draw(canvas, rect.x + 26, cy - Text::line_height(11) / 2, 11, TEXT_COLOR, "Snap to the dots");
                let (pw, ph) = (26, 14);
                let pill = Rect::new(rect.right() - pw - 6, cy - ph / 2, pw, ph);
                canvas.round_fill(pill.x, pill.y, pill.w, pill.h, ph / 2, if on { ACCENT } else { 0x4A4A4A }, 256);
                let knob = if on { pill.right() - ph / 2 } else { pill.x + ph / 2 };
                canvas.disc(knob, cy, ph / 2 - 2, 0xFFFFFF, 256);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    /// A box drawn on a board, picked out, the pointer in hand.
    fn drawn(studio: &mut Studio, from: (f32, f32), to: (f32, f32)) -> usize {
        press_key(studio, Key::Char('r'), Mods::default());
        pull(studio, from, to, Mods::default());
        press_key(studio, Key::Escape, Mods::default());
        studio.doc.layers.iter().rposition(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Rect)).unwrap()
    }

    #[test]
    fn the_width_height_and_place_of_what_is_picked_out_are_typed_into_its_panel() {
        let mut studio = board();
        let a = drawn(&mut studio, (100.0, 100.0), (260.0, 172.0));
        let b = drawn(&mut studio, (500.0, 100.0), (660.0, 172.0));
        let arrow = studio.join(a, b);
        studio.selected = vec![a];
        // W clicked, typed and Enter: so wide, from its left; the arrow
        // joined following its side.
        let field = |studio: &mut Studio, which: u8| {
            studio.layout();
            studio.control_rect(Control::Board(Ctl::Exact(which))).expect("the field").center()
        };
        let undos = studio.undo.len();
        let w = field(&mut studio, 0);
        click(&mut studio, w);
        type_in(&mut studio, "240");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!((shape(&studio, a).frame(), studio.undo.len()), ((100.5, 100.5, 240.0, 72.0), undos + 1));
        assert!((shape(&studio, arrow).points[0].0 - 340.5).abs() < 1.5, "the arrow from its new side: {:?}", shape(&studio, arrow).points);
        // Tab on to H; Up a pixel more, Shift+Down ten fewer.
        click(&mut studio, w);
        press_key(&mut studio, Key::Tab, Mods::default());
        assert_eq!(studio.board.exact_typed.as_ref().map(|(which, _)| *which), Some(1));
        press_key(&mut studio, Key::Up, Mods::default());
        press_key(&mut studio, Key::Down, Mods { shift: true, ..Mods::default() });
        assert_eq!(shape(&studio, a).frame().3, 63.0);
        // Esc leaves it as it was.
        type_in(&mut studio, "500");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!((shape(&studio, a).frame().3, studio.board.exact_typed.clone()), (63.0, None));
        // X and Y of two picked out move both, as one.
        studio.selected = vec![a, b];
        let x = field(&mut studio, 2);
        click(&mut studio, x);
        type_in(&mut studio, "0");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!((shape(&studio, a).frame().0, shape(&studio, b).frame().0), (0.0, 400.0));
        assert_eq!(studio.exact_value(2), Some(0.0));
        // Clicked away from, what was typed is set.
        let y = field(&mut studio, 3);
        click(&mut studio, y);
        type_in(&mut studio, "-36");
        tap(&mut studio, (800.0, 600.0));
        assert_eq!(shape(&studio, b).frame().1, -36.0);
        // One step each to undo.
        studio.undo();
        assert_eq!(shape(&studio, b).frame().1, 100.5);
    }

    #[test]
    fn snapping_to_the_dots_lands_what_is_drawn_moved_and_sized_on_them() {
        let mut studio = board();
        let on = |at: f32| (at / DOT_STEP).fract().abs() < 1e-3 || (at / DOT_STEP).fract().abs() > 1.0 - 1e-3;
        // Off, a box is where it is drawn.
        let first = drawn(&mut studio, (100.0, 100.0), (250.0, 171.0));
        assert!(!on(shape(&studio, first).frame().0));
        // Ctrl+': on, and said in the panel's switch.
        press_key(&mut studio, Key::Char('\''), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.snap_on());
        let boxed = drawn(&mut studio, (103.0, 205.0), (250.0, 271.0));
        let (x, y, w, h) = shape(&studio, boxed).frame();
        assert!([x, y, x + w, y + h].iter().all(|&at| on(at)), "drawn from dot to dot: {:?}", (x, y, w, h));
        // Moved: its top left on a dot.
        pull(&mut studio, (150.0, 230.0), (377.0, 333.0), Mods::default());
        let (x, y, _, _) = shape(&studio, boxed).frame();
        assert!(on(x) && on(y) && x > 300.0, "moved onto the dots: {x},{y}");
        // Its corner pulled: on a dot.
        studio.selected = vec![boxed];
        let (x, y, w, h) = shape(&studio, boxed).frame();
        let corner = studio.to_window((x + w, y + h));
        studio.pointer_moved(corner.0, corner.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.to_window((x + w + 41.0, y + h + 30.0));
        studio.pointer_moved(to.0 - 5, to.1 - 5, Mods::default());
        studio.pointer_moved(to.0, to.1, Mods::default());
        studio.released();
        let (nx, ny, nw, nh) = shape(&studio, boxed).frame();
        assert!(on(nx + nw) && on(ny + nh) && nw > w && (nx, ny) == (x, y), "sized onto the dots: {:?}", (nx, ny, nw, nh));
        // Kept with the board in its file, and off again.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert!(back[0].snap);
        press_key(&mut studio, Key::Char('\''), Mods { ctrl: true, ..Mods::default() });
        assert!(!read_pages(&write_pages(&studio.gathered())).unwrap()[0].snap);
        // The panel's switch says so and sets it.
        studio.selected = vec![boxed];
        studio.layout();
        let switch = studio.control_rect(Control::Board(Ctl::Snap)).expect("the switch").center();
        click(&mut studio, switch);
        assert!(studio.snap_on());
    }
}
