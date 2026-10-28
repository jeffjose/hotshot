//! What a board does that Ctrl+K finds by name, each with its keys.

use super::*;

/// Something only a board does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) enum Act {
    /// The boxes picked out made one of the shapes, or the next of them.
    Shape(u8),
    NextShape,
    /// The board's history gone back through.
    History,
    /// The map picked out laid out neatly.
    Tidy,
    /// Guessing what comes next, on or off.
    Guessing,
    /// A status dot for the boxes picked out: none, red, amber, green.
    Status(u8),
    /// What is joined to what is picked out lit; the board presented; a
    /// 2x2, lanes or Before / After laid out; notes gathered; a vote; and
    /// how a box lays out what is put in it.
    Focus,
    Present,
    Template(u8),
    Group,
    Vote(bool),
    Region(u8),
    /// An SVG brought in as shapes kept as the picture it was; and what is
    /// picked out made one picture.
    KeepPicture,
    MergePicture,
    /// The board, a frame of it, or what is picked out written as one of
    /// `export::KINDS`.
    Export(u8),
}

impl Studio {
    /// What a board does, as Ctrl+K lists it: what it says and its keys.
    pub(in crate::studio) fn board_acts(&self) -> Vec<(String, &'static str, Act)> {
        let mut acts: Vec<(String, &'static str, Act)> = vec![
            ("History: go back and branch".to_owned(), "Ctrl+Alt+Z", Act::History),
            ("Present the board".to_owned(), "", Act::Present),
            ("Insert a 2\u{d7}2".to_owned(), "", Act::Template(0)),
            ("Insert Now / Next / Later lanes".to_owned(), "", Act::Template(1)),
            ("Insert Before / After".to_owned(), "", Act::Template(2)),
            (if self.board.guessing { "Stop guessing what comes next" } else { "Guess what comes next" }.to_owned(), "", Act::Guessing),
        ];
        let what = if !self.on_page() { "frame" } else if self.chosen().is_empty() { "board" } else { "selection" };
        for (kind, name) in export::KINDS.iter().enumerate() {
            acts.push((format!("Export {what} as {name}"), if kind == 0 { "Ctrl+E" } else { "" }, Act::Export(kind as u8)));
        }
        if self.chosen().iter().any(|&index| self.is_box(index)) {
            acts.push(("Next shape".to_owned(), "Shift+S", Act::NextShape));
            for (at, name) in change::SHAPES.iter().enumerate() {
                acts.push((format!("Shape: {name}"), "", Act::Shape(at as u8)));
            }
            acts.push(("Tidy the map".to_owned(), "Shift+L", Act::Tidy));
            acts.push(("Focus: light what it is joined to".to_owned(), "F", Act::Focus));
            acts.push(("Lay out what is in it: as it is put".to_owned(), "", Act::Region(0)));
            acts.push(("Lay out what is in it: left to right".to_owned(), "", Act::Region(1)));
            acts.push(("Lay out what is in it: as a tree, top down".to_owned(), "", Act::Region(2)));
            if self.chosen().len() > 1 {
                acts.push(("Gather into a group".to_owned(), "G", Act::Group));
            }
            if self.chosen().iter().any(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.form == shapes::FORM_STICKY)) {
                acts.push(("Vote".to_owned(), "D", Act::Vote(true)));
                acts.push(("Take a vote away".to_owned(), "Shift+D", Act::Vote(false)));
            }
            for (at, name) in ["No status", "Status: red", "Status: amber", "Status: green"].iter().enumerate() {
                acts.push(((*name).to_owned(), "", Act::Status(at as u8)));
            }
        }
        acts
    }

    pub(in crate::studio) fn board_act(&mut self, act: Act) -> Outcome {
        self.dirty = true;
        match act {
            Act::Shape(shape) => self.reshape(Some(usize::from(shape))),
            Act::NextShape => self.reshape(None),
            Act::History => self.open_history(),
            Act::Tidy => self.tidy(),
            Act::Guessing => self.press_style(Ctl::Guess),
            Act::Focus => self.focus(),
            Act::Present => self.present(),
            Act::Template(which) => self.template(which),
            Act::Group => self.group(),
            Act::Vote(more) => self.vote(more),
            Act::Region(region) => self.set_region(region),
            Act::KeepPicture => self.keep_svg_picture(),
            Act::MergePicture => self.merge_picture(),
            Act::Export(kind) => self.export_board(kind),
            Act::Status(status) => {
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        shape.status = status;
                    }
                });
            }
        }
        Outcome::Continue
    }

    /// A key while the history is open: Left and Right go back and on a
    /// step, Enter branches from where it is, Esc comes back to now.
    pub(in crate::studio) fn history_key(&mut self, key: Key) -> bool {
        let Some(history) = &self.board.history else { return false };
        let back = history.back;
        match key {
            Key::Left => self.history_to(back + 1),
            Key::Right => self.history_to(back.saturating_sub(1)),
            Key::Enter => self.branch_here(),
            Key::Escape => self.history_now(),
            _ => {}
        }
        self.dirty = true;
        true
    }
}
