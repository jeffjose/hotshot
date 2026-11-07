//! A board shows its zoom nowhere: the pill at its top left, the logo's
//! menu, has it, with Design or Board, its theme, paper, guessing and
//! keys, and the zoom is said for a moment as it changes. So the board
//! itself has nothing on it but the bar of tools, to draw on.

use super::*;

impl Studio {
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
            Ctl::Arrange(at) => bar::ARRANGE.get(usize::from(at))?.1.to_owned(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;
    use crate::studio::menu::Item;

    /// The pill at the top left clicked, its menu open.
    fn open_menu(studio: &mut Studio) {
        let pill = studio.control_rect(Control::Home).unwrap();
        click(studio, pill.center());
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Main));
    }

    /// The row of the open menu, at its level, that says so much.
    fn row(studio: &mut Studio, level: u8, says: &str) -> Rect {
        let menu = studio.menu.expect("a menu open");
        let kind = if level == 0 { menu.kind } else { menu.sub.expect("its list open").1 };
        let at = studio.menu_items(kind).iter().position(|item| matches!(item, Item::Entry { says: row, .. } if row == says)).unwrap_or_else(|| panic!("{says} in the list"));
        studio.control_rect(Control::MenuRow(level, at as u16)).unwrap()
    }

    #[test]
    fn a_board_has_one_pill_at_its_top_left_present_at_its_right_and_the_bar_of_tools() {
        let mut studio = board();
        let layout = studio.layout();
        let dock = layout.dock;
        let shown: Vec<Control> = layout.controls.iter().filter(|(rect, _)| !dock.contains(rect.center().0, rect.center().1)).map(|(_, control)| *control).filter(|control| !matches!(control, Control::Panel)).collect();
        assert_eq!(shown, vec![Control::Home, Control::Board(Ctl::Present)], "nothing else on the board");
        let pill = studio.control_rect(Control::Home).unwrap();
        assert!(pill.x < 30 && pill.y < 30 && pill.w < 240, "{pill:?}");
        // The same in Design, the panel shown or hidden.
        studio.set_mode(false);
        studio.layout();
        let again = studio.control_rect(Control::Home).unwrap();
        assert_eq!((again.x, again.y), (pill.x, pill.y));
    }

    #[test]
    fn the_pill_s_menu_switches_design_and_board_zooms_and_shows_the_panel() {
        let mut studio = board();
        open_menu(&mut studio);
        let design = row(&mut studio, 0, "Switch to Design");
        click(&mut studio, design.center());
        assert!(!studio.on_board());
        press_key(&mut studio, Key::Char('m'), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.on_board());
        // The zoom: a list beside its row, which says how large it is.
        open_menu(&mut studio);
        assert!(studio.menu_items(MenuKind::Main).iter().any(|item| matches!(item, Item::Entry { says, keys, sub: Some(MenuKind::Zoom), .. } if says == "Zoom" && keys == "100%")));
        let zoom = row(&mut studio, 0, "Zoom");
        studio.pointer_moved(zoom.center().0, zoom.center().1, Mods::default());
        let to = row(&mut studio, 1, "Zoom to 200%");
        studio.pointer_moved(to.center().0, to.center().1, Mods::default());
        click(&mut studio, to.center());
        assert!((studio.zoom - 2.0).abs() < 1e-4);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("200%"), "said for a moment");
        // Pages and layers: the left panel, with its own button to hide it.
        open_menu(&mut studio);
        let panel = row(&mut studio, 0, "Pages and layers");
        click(&mut studio, panel.center());
        assert!(studio.shows_left());
        let hide = studio.control_rect(Control::SidePanel(0)).expect("its button, on it");
        click(&mut studio, hide.center());
        assert!(!studio.shows_left());
        assert!(studio.control_rect(Control::SidePanel(0)).is_none());
    }

    #[test]
    fn the_wheel_says_a_board_s_zoom_and_fit_keeps_it_at_100_or_less() {
        let mut studio = board();
        studio.zoom_about(1.25, (700, 400));
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("125%"));
        let mut small = board();
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (100.0, 100.0));
        shape.points[1] = (140.0, 120.0);
        small.doc.layers.push(Layer::shape(shape));
        small.changed();
        small.zoom = 0.3;
        press_key(&mut small, Key::Char('0'), Mods { ctrl: true, ..Mods::default() });
        assert_eq!(small.zoom, 1.0, "a small board is fitted at 100%, not blown up");
        assert_eq!(small.note.as_ref().map(|(says, _)| says.as_str()), Some("100%"));
    }

    #[test]
    fn over_the_pill_the_bar_or_the_panel_the_pointer_is_an_arrow_whatever_tool_is_in_hand() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        let pill = studio.control_rect(Control::Home).unwrap();
        studio.pointer_moved(pill.center().0, pill.center().1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Arrow, "over the pill");
        let dock = studio.layout().dock;
        studio.pointer_moved(dock.center().0, dock.center().1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Arrow, "over the bar of tools");
        // On the board, the tool's.
        studio.pointer_moved(700, 400, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Crosshair, "drawing a box");
    }

    #[test]
    fn the_logo_s_menu_on_a_board_has_its_theme_paper_guessing_and_keys() {
        let mut studio = board();
        open_menu(&mut studio);
        let items = studio.menu_items(MenuKind::Main);
        let says: Vec<&str> = items.iter().filter_map(|item| match item {
            Item::Entry { says, .. } => Some(says.as_str()),
            _ => None,
        }).collect();
        for want in ["Theme", "Paper", "Guess what comes next", "Keyboard shortcuts"] {
            assert!(says.contains(&want), "{want} in {says:?}");
        }
        let guessing = studio.board.guessing;
        let guess = row(&mut studio, 0, "Guess what comes next");
        click(&mut studio, guess.center());
        assert_eq!(studio.board.guessing, !guessing);
        open_menu(&mut studio);
        let keys = row(&mut studio, 0, "Keyboard shortcuts");
        click(&mut studio, keys.center());
        assert!(studio.board.keys);
        // Design's has none of them.
        studio.set_mode(false);
        assert!(!studio.menu_items(MenuKind::Main).iter().any(|item| matches!(item, Item::Entry { says, .. } if says == "Theme")));
    }
}
