//! How large a board is shown, at its top left after the switch of
//! Design and Board, a list of zooms; the menu of everything, the logo's,
//! before them, has the board's theme, paper, guessing and keys. So the
//! board itself has nothing on it but the bar of tools, to draw on.

use super::*;

/// How high the zoom is and how wide, and the edge round it.
const SIDE: i32 = 28;
const ZOOM_W: i32 = 72;
const PAD: i32 = 4;

impl Studio {
    /// The zoom laid out after the switch, on a little panel of its own.
    pub(in crate::studio) fn layout_corner(&mut self, layout: &mut Layout) {
        let switch = self.switch_place();
        let place = Rect::new(switch.right() + 10, switch.y + (switch.h - SIDE - 2 * PAD) / 2, ZOOM_W + 2 * PAD, SIDE + 2 * PAD);
        layout.controls.push((Rect::new(place.x + PAD, place.y + PAD, ZOOM_W, SIDE), Control::Zoom));
        layout.controls.push((place, Control::Panel));
        layout.board_zoom = Some(place);
    }

    /// The sheet of keys pressed: on it, nothing; round it, closed.
    pub(in crate::studio) fn press_zoom(&mut self, ctl: Ctl) {
        if ctl == Ctl::Scrim {
            self.board.keys = false;
        }
        self.dirty = true;
    }

    /// What a control of a board says as it is hovered: a tool, its name
    /// and key.
    pub(in crate::studio) fn board_tip(&self, ctl: Ctl) -> Option<String> {
        Some(match ctl {
            Ctl::Tool(which) => TOOLS.get(usize::from(which)).map(|(name, key)| format!("{name}  {}", key.to_ascii_uppercase()))?,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;
    use crate::studio::menu::{self, Item};

    /// A row of an open list clicked, by what it says.
    fn pick(studio: &mut Studio, says: &str) {
        let kind = studio.menu.map(|menu| menu.kind).expect("a list open");
        let row = studio.menu_items(kind).iter().position(|item| matches!(item, Item::Entry { says: row, .. } if row == says)).unwrap_or_else(|| panic!("{says} in the list"));
        let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
        studio.pointer_moved(rect.center().0, rect.center().1, Mods::default());
        click(studio, rect.center());
    }

    #[test]
    fn a_board_has_nothing_on_it_but_the_bar_of_tools_the_rest_at_its_top_left() {
        let mut studio = board();
        let layout = studio.layout();
        let dock = layout.dock;
        // Along the bottom, only the bar of tools.
        let low: Vec<Rect> = layout.controls.iter().map(|(rect, _)| *rect).filter(|rect| rect.bottom() > dock.y - 40 && !dock.contains(rect.center().0, rect.center().1)).collect();
        assert!(low.is_empty(), "nothing else along the bottom: {low:?}");
        // At the top left: the logo first, and after the switch the zoom.
        let corner = layout.board_zoom.expect("the corner");
        let (logo, zoom) = (studio.control_rect(Control::Home).unwrap(), studio.control_rect(Control::Zoom).unwrap());
        assert!(corner.x > studio.switch_place().right() && corner.y < 60, "{corner:?}");
        assert!(logo.x < 40 && logo.y < 40 && corner.contains(zoom.center().0, zoom.center().1), "{logo:?} {zoom:?}");
    }

    #[test]
    fn the_zoom_at_the_top_left_lists_the_zooms_and_fits_the_board() {
        let mut studio = board();
        let zoom = studio.control_rect(Control::Zoom).unwrap();
        click(&mut studio, zoom.center());
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Zoom));
        pick(&mut studio, "Zoom to 200%");
        assert!((studio.zoom - 2.0).abs() < 1e-4);
        // Two boxes far apart: all of them in view, never past 100%.
        for (x, y) in [(-800.0, -300.0), (1900.0, 900.0)] {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
            shape.points[1] = (x + 160.0, y + 72.0);
            studio.doc.layers.push(Layer::shape(shape));
        }
        studio.changed();
        click(&mut studio, zoom.center());
        pick(&mut studio, "Zoom to fit");
        let open = studio.open_area();
        let (a, b) = (studio.to_window_page((-800.0, -300.0)), studio.to_window_page((2060.0, 972.0)));
        assert!(a.0 > open.x && a.1 > open.y && b.0 < open.right() && b.1 < open.bottom(), "{a:?} {b:?} in {open:?}");
        let mut small = board();
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (100.0, 100.0));
        shape.points[1] = (140.0, 120.0);
        small.doc.layers.push(Layer::shape(shape));
        small.changed();
        small.zoom = 0.3;
        press_key(&mut small, Key::Char('0'), Mods { ctrl: true, ..Mods::default() });
        assert_eq!(small.zoom, 1.0, "a small board is fitted at 100%, not blown up");
    }

    #[test]
    fn the_logo_s_menu_on_a_board_has_its_theme_paper_guessing_and_keys() {
        let mut studio = board();
        let logo = studio.control_rect(Control::Home).unwrap();
        click(&mut studio, logo.center());
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Main));
        let items = studio.menu_items(MenuKind::Main);
        let says: Vec<&str> = items.iter().filter_map(|item| match item {
            Item::Entry { says, .. } => Some(says.as_str()),
            _ => None,
        }).collect();
        for want in ["Theme", "Paper", "Guess what comes next", "Keyboard shortcuts"] {
            assert!(says.contains(&want), "{want} in {says:?}");
        }
        assert!(items.iter().any(|item| matches!(item, Item::Entry { says, sub: Some(MenuKind::Looks), .. } if says == "Theme")));
        let guessing = studio.board.guessing;
        pick(&mut studio, "Guess what comes next");
        assert_eq!(studio.board.guessing, !guessing);
        click(&mut studio, logo.center());
        pick(&mut studio, "Keyboard shortcuts");
        assert!(studio.board.keys);
        // Design's has none of them.
        studio.set_mode(false);
        let says: Vec<String> = studio.menu_items(MenuKind::Main).iter().filter_map(|item| match item {
            menu::Item::Entry { says, .. } => Some(says.clone()),
            _ => None,
        }).collect();
        assert!(!says.iter().any(|says| says == "Theme"));
    }

    #[test]
    fn the_corner_is_hidden_with_everything_else() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.layout().board_zoom.is_none());
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.layout().board_zoom.is_some());
    }
}
