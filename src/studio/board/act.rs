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
}

impl Studio {
    /// What a board does, as Ctrl+K lists it: what it says and its keys.
    pub(in crate::studio) fn board_acts(&self) -> Vec<(String, &'static str, Act)> {
        let mut acts: Vec<(String, &'static str, Act)> = vec![
            ("History: go back and branch".to_owned(), "Ctrl+Alt+Z", Act::History),
            (if self.board.guessing { "Stop guessing what comes next" } else { "Guess what comes next" }.to_owned(), "", Act::Guessing),
        ];
        if self.chosen().iter().any(|&index| self.is_box(index)) {
            acts.push(("Next shape".to_owned(), "Shift+S", Act::NextShape));
            for (at, name) in change::SHAPES.iter().enumerate() {
                acts.push((format!("Shape: {name}"), "", Act::Shape(at as u8)));
            }
            acts.push(("Tidy the map".to_owned(), "Shift+L", Act::Tidy));
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
