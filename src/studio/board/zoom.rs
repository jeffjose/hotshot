//! The zoom of a board, at its bottom left, as Excalidraw keeps it: −, how
//! large it is shown, which a click makes 100%, +, all of it fitted, and
//! ? for the sheet of its keys; on the same dark as the bar of tools.

use super::*;

/// How wide each of its buttons is, how far apart, and its edge round them.
const SIDE: i32 = 28;
const SHOWN_W: i32 = 52;
const GAP: i32 = 2;
const PAD: i32 = 4;
/// Where the rule between the zoom and the rest goes, and its room.
const RULE: i32 = 9;

impl Studio {
    /// The zoom laid out at the bottom left of what is open, its middle
    /// level with the bar of tools'; not where it would reach the bar, as
    /// in a narrow window.
    pub(in crate::studio) fn layout_zoom(&mut self, layout: &mut Layout) {
        let parts = [(Ctl::ZoomOut, SIDE), (Ctl::ZoomShown, SHOWN_W), (Ctl::ZoomIn, SIDE), (Ctl::Fit, SIDE), (Ctl::Keys, SIDE)];
        let wide = parts.iter().map(|(_, w)| w + GAP).sum::<i32>() - GAP + RULE - GAP + 2 * PAD;
        let high = SIDE + 2 * PAD;
        let dock = layout.dock;
        let place = Rect::new(self.open_area().x + 12, dock.y + (dock.h - high) / 2, wide, high);
        if place.right() + 12 > dock.x {
            return;
        }
        let mut x = place.x + PAD;
        for (ctl, w) in parts {
            if ctl == Ctl::Fit {
                x += RULE - GAP;
            }
            layout.controls.push((Rect::new(x, place.y + PAD, w, SIDE), Control::Board(ctl)));
            x += w + GAP;
        }
        layout.controls.push((place, Control::Panel));
        layout.board_zoom = Some(place);
    }

    /// A button of the zoom pressed.
    pub(in crate::studio) fn press_zoom(&mut self, ctl: Ctl) {
        let middle = self.open_area().center();
        match ctl {
            Ctl::ZoomOut => self.zoom_about(0.8, middle),
            Ctl::ZoomIn => self.zoom_about(1.25, middle),
            Ctl::ZoomShown => self.zoom_about(1.0 / self.zoom, middle),
            Ctl::Fit if self.doc.layers.is_empty() && self.frame_count() == 0 => self.say("Nothing on the board to fit"),
            Ctl::Fit => self.fit(),
            Ctl::Keys => self.board.keys = !self.board.keys,
            // A click round the sheet closes it; on it, nothing.
            Ctl::Scrim => self.board.keys = false,
            _ => {}
        }
        self.dirty = true;
    }

    /// A button of the zoom drawn: − and +, how large as a percentage, the
    /// four corners of what is fitted, and a question mark; the rule
    /// before the last two drawn with the first.
    pub(in crate::studio) fn draw_zoom_control(&mut self, canvas: &mut Canvas, rect: Rect, ctl: Ctl, hovered: bool) {
        let (cx, cy) = rect.center();
        let on = ctl == Ctl::Keys && self.board.keys;
        if on {
            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, ACCENT, 256);
        } else if hovered {
            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, HOVER_COLOR, 256);
        }
        let color = if on { 0xFFFFFF } else { TEXT_COLOR };
        match ctl {
            Ctl::ZoomOut => self.text.draw_centered(canvas, cx, cy, 15, color, icons::MINUS),
            Ctl::ZoomIn => self.text.draw_centered(canvas, cx, cy, 15, color, icons::PLUS),
            Ctl::ZoomShown => {
                let says = zoom_says(self.zoom);
                let wide = self.text.width(&says, 12);
                self.text.draw(canvas, cx - wide / 2, cy - Text::line_height(12) / 2, 12, color, &says);
            }
            Ctl::Fit => {
                canvas.fill(rect.x - (RULE - GAP) / 2 - 1, rect.y + 6, 1, rect.h - 12, HOVER_COLOR);
                // Four corners round what is fitted.
                let (w, h, arm) = (14, 12, 4);
                let (left, top) = (cx - w / 2, cy - h / 2);
                for (x, y, across, down) in [(left, top, 1, 1), (left + w - 1, top, -1, 1), (left, top + h - 1, 1, -1), (left + w - 1, top + h - 1, -1, -1)] {
                    for step in 0..arm {
                        for thick in 0..2 {
                            canvas.fill(x + step * across, y + thick * down, 1, 1, color);
                            canvas.fill(x + thick * across, y + step * down, 1, 1, color);
                        }
                    }
                }
            }
            _ => {
                let wide = self.text.width("?", 14);
                self.text.draw(canvas, cx - wide / 2, cy - Text::line_height(14) / 2, 14, color, "?");
            }
        }
    }

    /// What a control of a board says as it is hovered: a tool, its name
    /// and key; a button of the zoom, what it does and its key.
    pub(in crate::studio) fn board_tip(&self, ctl: Ctl) -> Option<String> {
        Some(match ctl {
            Ctl::Tool(which) => TOOLS.get(usize::from(which)).map(|(name, key)| format!("{name}  {}", key.to_ascii_uppercase()))?,
            Ctl::ZoomOut => "Zoom out  Ctrl \u{2212}".to_owned(),
            Ctl::ZoomIn => "Zoom in  Ctrl +".to_owned(),
            Ctl::ZoomShown => "Zoom to 100%  Ctrl 1".to_owned(),
            Ctl::Fit => "Zoom to fit  Ctrl 0".to_owned(),
            Ctl::Keys => "Keyboard shortcuts  ?".to_owned(),
            _ => return None,
        })
    }
}

/// How large a board is shown, as a whole percentage.
fn zoom_says(zoom: f32) -> String {
    format!("{}%", (zoom * 100.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn press(studio: &mut Studio, ctl: Ctl) {
        let at = studio.control_rect(Control::Board(ctl)).unwrap().center();
        click(studio, at);
    }

    #[test]
    fn the_zoom_at_the_bottom_left_steps_resets_to_100_and_fits_the_board() {
        let mut studio = board();
        let place = studio.layout().board_zoom.unwrap();
        let dock = studio.layout().dock;
        assert!(place.x < 40 && place.right() < dock.x && place.y >= dock.y && place.bottom() <= dock.bottom(), "at the bottom left, level with the bar: {place:?} {dock:?}");
        press(&mut studio, Ctl::ZoomIn);
        assert!((studio.zoom - 1.25).abs() < 1e-4);
        press(&mut studio, Ctl::ZoomOut);
        press(&mut studio, Ctl::ZoomOut);
        assert!((studio.zoom - 0.8).abs() < 1e-4);
        assert_eq!(zoom_says(studio.zoom), "80%");
        assert_eq!(zoom_says(0.6249), "62%");
        // The percentage clicked: 100%, about the middle of what is open.
        press(&mut studio, Ctl::ZoomShown);
        assert!((studio.zoom - 1.0).abs() < 1e-4);
        // Nothing to fit, nothing done but saying so.
        press(&mut studio, Ctl::Fit);
        assert_eq!(studio.zoom, 1.0);
        // Two boxes far apart, and a frame further: all of them in view,
        // with room round them, and never past 100%.
        for (x, y) in [(-800.0, -300.0), (1900.0, 900.0)] {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
            shape.points[1] = (x + 160.0, y + 72.0);
            studio.doc.layers.push(Layer::shape(shape));
        }
        studio.changed();
        press(&mut studio, Ctl::Fit);
        let open = studio.open_area();
        let (a, b) = (studio.to_window_page((-800.0, -300.0)), studio.to_window_page((2060.0, 972.0)));
        assert!(a.0 > open.x && a.1 > open.y && b.0 < open.right() && b.1 < open.bottom(), "{a:?} {b:?} in {open:?}");
        assert!(studio.zoom < 0.5);
        let mut small = board();
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (100.0, 100.0));
        shape.points[1] = (140.0, 120.0);
        small.doc.layers.push(Layer::shape(shape));
        small.changed();
        small.zoom = 0.3;
        press(&mut small, Ctl::Fit);
        assert_eq!(small.zoom, 1.0, "a small board is fitted at 100%, not blown up");
        // Ctrl+0 is the same.
        small.zoom = 0.3;
        press_key(&mut small, Key::Char('0'), Mods { ctrl: true, ..Mods::default() });
        assert_eq!(small.zoom, 1.0);
    }

    #[test]
    fn a_press_on_the_zoom_never_reaches_the_board() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        let place = studio.layout().board_zoom.unwrap();
        // Between two of its buttons, on its own edge.
        let at = (place.x + 1, place.y + 1);
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(at.0 + 120, at.1 - 200, Mods::default());
        studio.released();
        assert!(studio.doc.layers.is_empty(), "no box drawn from it");
        // Hovered, each says what it does.
        assert_eq!(studio.board_tip(Ctl::Fit).as_deref(), Some("Zoom to fit  Ctrl 0"));
        assert_eq!(studio.board_tip(Ctl::Tool(LASER_TOOL)).as_deref(), Some("Laser  K"));
    }

    #[test]
    fn the_zoom_is_hidden_with_everything_else_and_while_presenting() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.layout().board_zoom.is_none());
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.layout().board_zoom.is_some());
    }
}
