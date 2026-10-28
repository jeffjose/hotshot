//! Hover to preview, click to keep: while the pointer rests on a button,
//! a swatch or a row of a list that changes what is picked out, the
//! change is shown on the canvas as if it were made; moving off puts
//! things back as they were, and only a click makes it, as one step to
//! undo.

use super::*;

/// What is being shown as if it were done, and everything as it was
/// before, to go back to.
pub(super) struct Preview {
    pub key: Control,
    doc: Doc,
    undo: usize,
    redo: Vec<Snapshot>,
    color: u32,
    fill: Option<u32>,
    face: usize,
    edited: bool,
}

impl Studio {
    /// Whether a control is one whose change shows while it is hovered:
    /// lining up and spacing, flips and turns, colors, shadows, and the
    /// rows of the lists of how a line or a shadow looks.
    pub(super) fn previews(&mut self, control: Control) -> bool {
        if self.chosen().is_empty() {
            return false;
        }
        match control {
            Control::Action(action) => matches!(action, Action::Align(_) | Action::Distribute(_) | Action::Pack(_) | Action::Match(_) | Action::ToFrame(_) | Action::Turn(_) | Action::FlipAcross | Action::FlipDown | Action::Halo | Action::Pill(_) | Action::TextAlign(_) | Action::Path(_) | Action::BlurStyle(_)),
            Control::Swatch(paint, _) => !matches!(paint, Paint::Background | Paint::From | Paint::To),
            Control::Shadow(_) | Control::Window(_) | Control::Style(_) => true,
            Control::MenuRow(level, row) => {
                let Some(menu) = self.menu else { return false };
                let kind = if level == 0 { menu.kind } else { menu.sub.map_or(menu.kind, |(_, kind)| kind) };
                if !matches!(kind, MenuKind::Shadows | MenuKind::Styles | MenuKind::Dash | MenuKind::Edge | MenuKind::Hand | MenuKind::Window | MenuKind::Heads(_) | MenuKind::Fonts) {
                    return false;
                }
                matches!(self.menu_items(kind).get(usize::from(row)), Some(menu::Item::Entry { sub: None, .. }))
            }
            _ => false,
        }
    }

    /// The pointer over a control, or over none: what it previews shown,
    /// and what was shown before put back.
    pub(super) fn hover_preview(&mut self, control: Option<Control>) {
        let wanted = control.filter(|&control| self.previews(control));
        if self.preview.as_ref().map(|preview| preview.key) == wanted {
            return;
        }
        self.end_preview();
        let Some(key) = wanted else { return };
        self.preview = Some(Preview { key, doc: self.doc.clone(), undo: self.undo.len(), redo: self.redo.clone(), color: self.color, fill: self.fill, face: self.face, edited: self.edited });
        let (menu, palette, hsv) = (self.menu, self.palette, self.hsv);
        match key {
            Control::MenuRow(level, row) => {
                let menu = self.menu.unwrap_or(MenuState { kind: MenuKind::Main, at: (0, 0), sub: None });
                let kind = if level == 0 { menu.kind } else { menu.sub.map_or(menu.kind, |(_, kind)| kind) };
                if let Some(menu::Item::Entry { act, .. }) = self.menu_items(kind).get(usize::from(row)) {
                    let act = *act;
                    self.run_menu(act);
                }
            }
            Control::Swatch(paint, color) => self.paint(paint, color),
            Control::Style(style) => self.put_style(style),
            Control::Action(action) => drop(self.act(action)),
            other => drop(self.press_control(other, false)),
        }
        (self.menu, self.palette, self.hsv) = (menu, palette, hsv);
        // Nothing of it is a step to undo until it is clicked.
        if let Some(preview) = &self.preview {
            self.undo.truncate(preview.undo);
            self.redo = preview.redo.clone();
        }
        self.dirty = true;
    }

    /// Whatever is being previewed put back as it was.
    pub(super) fn end_preview(&mut self) {
        let Some(preview) = self.preview.take() else { return };
        self.doc = preview.doc;
        (self.color, self.fill, self.face, self.edited) = (preview.color, preview.fill, preview.face, preview.edited);
        self.undo.truncate(preview.undo);
        self.redo = preview.redo;
        self.version += 1;
        self.doc.stamp = self.version;
        self.dirty = true;
    }
}
