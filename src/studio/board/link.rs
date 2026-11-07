//! Links on a board's shapes, as FigJam's are: a web address, or another
//! shape or frame of the board. Set from the right click, by typing or
//! pasting an address, or by Link to… and a click on what it goes to.
//! A shape with one wears a small round badge at its top right, blue with
//! an arrow out for the web, green with an arrow along for the board;
//! hovered, a card says where it goes, with Edit, Copy and Remove. Ctrl
//! and a click on the shape, or a click on its badge, or presenting a
//! click on it at all, follows it: the web in the browser, the board's
//! view gliding over to what it goes to, which is picked out.

use super::*;

/// A link being set, on the layer called so: its address typed, with all
/// of it picked out to be typed over; or what it goes to being picked out
/// on the board.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::studio) enum Linking {
    Typing { id: u32, typed: String, picked: bool },
    Picking { id: u32 },
}

/// The badge's radius, in pixels of the window; the colors of a link to
/// the web and of one to the board, and as the Diagram look has them.
const BADGE: f32 = 9.5;
const WEB: u32 = 0x1971C2;
const ON_BOARD: u32 = 0x2F9E44;

/// How wide the card a link's badge shows is at most, and its rows; and
/// the field a link is typed into.
const CARD_MOST: i32 = 340;
const CARD_ROW: i32 = 34;
const EDITOR_W: i32 = 360;
const EDITOR_H: i32 = 44;

/// Where a link goes, said plainly: a web address without its `https://`.
fn shown_address(link: &str) -> &str {
    link.strip_prefix("https://").or_else(|| link.strip_prefix("http://")).unwrap_or(link).trim_end_matches('/')
}

/// What is typed as a link made one: a layer of the board as it is, a web
/// address as it is, or with `https://` before it where it says none, as
/// a browser takes `example.com`.
pub(in crate::studio) fn as_link(typed: &str) -> String {
    let typed = typed.trim();
    if typed.is_empty() || typed.starts_with('#') || typed.starts_with("frame:") || typed.contains("://") || typed.starts_with("mailto:") {
        return typed.to_owned();
    }
    format!("https://{typed}")
}

/// Whether a link goes somewhere on the board rather than to the web.
fn on_board(link: &str) -> bool {
    link.starts_with('#') || link.starts_with("frame:")
}

impl Studio {
    /// Where a layer links to, if anywhere.
    pub(in crate::studio) fn link_of(&self, index: usize) -> Option<&str> {
        match self.doc.layers.get(index).map(|layer| &layer.kind) {
            Some(Kind::Shape(shape)) if !shape.link.is_empty() => Some(&shape.link),
            _ => None,
        }
    }

    /// A layer's link set, or taken away with nothing; one step to undo.
    pub(in crate::studio) fn set_link(&mut self, id: u32, link: String) {
        let Some(index) = self.index_of(id) else { return };
        if self.link_of(index).unwrap_or("") == link {
            return;
        }
        self.checkpoint();
        if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
            shape.link = link;
        }
        self.board.link_card = None;
        self.changed();
    }

    /// Where a board's link goes on the board, as a frame of it, and what
    /// it is called: the layer it names, or the frame; none where it is
    /// gone.
    fn link_target(&mut self, link: &str) -> Option<(Option<usize>, (f32, f32, f32, f32), String)> {
        if let Some(name) = link.strip_prefix("frame:") {
            let frame = self.pages[self.page].frames.iter().find(|frame| frame.name == name)?;
            let place = (frame.x - self.doc.x, frame.y - self.doc.y, frame.width as f32, frame.height as f32);
            return Some((None, place, name.to_owned()));
        }
        let index = self.index_of(link.strip_prefix('#')?.parse().ok()?)?;
        let place = self.doc.layers[index].bounds(&mut self.text);
        Some((Some(index), place, self.doc.layers[index].name()))
    }

    /// A layer's link followed: on the board, the view gliding over to what
    /// it goes to, picked out; on the web, opened in the browser.
    pub(in crate::studio) fn follow_link(&mut self, index: usize) {
        let Some(link) = self.link_of(index).map(str::to_owned) else { return };
        self.board.link_card = None;
        self.dirty = true;
        if !on_board(&link) {
            self.say(format!("Opening {}", shown_address(&link)));
            self.board.opened = Some(link.clone());
            if !cfg!(test) {
                let _ = std::process::Command::new("xdg-open").arg(&link).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
            }
            return;
        }
        let Some((target, place, _)) = self.link_target(&link) else { return self.say("What it linked to is gone") };
        self.commit_text();
        self.selected = target.into_iter().collect();
        self.view_onto(place, true);
    }

    /// A press on a board with Ctrl held, or anywhere while presenting, on
    /// a shape that links somewhere: the link followed. Whether it was.
    pub(in crate::studio) fn press_link(&mut self, at: (f32, f32)) -> bool {
        if !(self.mods.ctrl || self.presenting()) {
            return false;
        }
        let under = (0..self.doc.layers.len()).rev().find(|&index| !self.doc.layers[index].hidden && self.doc.layers[index].hit(at, &mut self.text));
        match under.filter(|&index| self.link_of(index).is_some()) {
            Some(index) => {
                self.follow_link(index);
                true
            }
            None => false,
        }
    }

    /// The right click's Add link…: a field to type or paste an address
    /// into, by the shape picked out, with what it has typed in already.
    pub(in crate::studio) fn begin_link(&mut self) {
        let Some(index) = self.picked().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Shape(_))) else { return };
        self.commit_text();
        let id = self.id_of(index);
        let had = self.link_of(index).filter(|link| !on_board(link)).unwrap_or("").to_owned();
        let picked = !had.is_empty();
        self.board.linking = Some(Linking::Typing { id, typed: had, picked });
        self.board.link_card = None;
        self.dirty = true;
    }

    /// Link to…: what a click on the board picks out next is what the shape
    /// picked out links to.
    pub(in crate::studio) fn begin_link_to(&mut self, index: Option<usize>) {
        let Some(index) = index.or_else(|| self.picked()).filter(|&index| matches!(self.doc.layers.get(index).map(|layer| &layer.kind), Some(Kind::Shape(_)))) else { return };
        self.commit_text();
        self.board.linking = Some(Linking::Picking { id: self.id_of(index) });
        self.board.link_card = None;
        self.say("Click what it links to, or Esc");
    }

    /// A press on the board while what a link goes to is being picked
    /// out: what is there linked to, a layer or a frame; nothing there,
    /// nothing linked. Whether the press was taken.
    pub(in crate::studio) fn press_link_to(&mut self, at: (f32, f32)) -> bool {
        let Some(Linking::Picking { id }) = self.board.linking.clone() else { return false };
        self.board.linking = None;
        let from = self.index_of(id);
        let under = (0..self.doc.layers.len()).rev().find(|&index| Some(index) != from && !self.doc.layers[index].hidden && self.doc.layers[index].hit(at, &mut self.text));
        let page = (at.0 + self.doc.x, at.1 + self.doc.y);
        let frame = self.pages[self.page].frames.iter().find(|frame| page.0 >= frame.x && page.1 >= frame.y && page.0 <= frame.x + frame.width as f32 && page.1 <= frame.y + frame.height as f32).map(|frame| frame.name.clone());
        let link = match (under, frame) {
            (Some(index), _) => format!("#{}", self.id_of(index)),
            (None, Some(name)) => format!("frame:{name}"),
            (None, None) => {
                self.say("Nothing there to link to");
                return true;
            }
        };
        self.set_link(id, link.clone());
        if let Some((_, _, name)) = self.link_target(&link) {
            self.say(format!("Linked to {name}"));
        }
        self.selected = from.into_iter().collect();
        true
    }

    /// A key while a link is typed, or what it goes to picked out: letters
    /// and Backspace type, Enter keeps it and Esc leaves it as it was; Esc
    /// stops the picking. Whether the key was taken.
    pub(in crate::studio) fn link_key(&mut self, key: Key, mods: Mods) -> bool {
        match self.board.linking.clone() {
            None => false,
            Some(Linking::Picking { .. }) => {
                if key == Key::Escape {
                    self.board.linking = None;
                    self.dirty = true;
                    return true;
                }
                false
            }
            Some(Linking::Typing { id, mut typed, picked }) => {
                self.dirty = true;
                let mut still = false;
                match key {
                    Key::Escape => {}
                    Key::Enter => self.set_link(id, as_link(&typed)),
                    Key::Char('a' | 'A') if mods.ctrl => still = picked || !typed.is_empty(),
                    Key::Backspace if picked => typed.clear(),
                    Key::Backspace if mods.ctrl => typed.truncate(typed.trim_end().trim_end_matches(|letter: char| !letter.is_whitespace() && letter != '/').len()),
                    Key::Backspace => drop(typed.pop()),
                    Key::Char(letter) if !mods.ctrl && !mods.alt => {
                        if picked {
                            typed.clear();
                        }
                        typed.push(letter);
                    }
                    _ => {
                        self.board.linking = Some(Linking::Typing { id, typed, picked: picked && key == Key::Other });
                        return !mods.ctrl;
                    }
                }
                let ctrl_a = matches!(key, Key::Char('a' | 'A')) && mods.ctrl;
                self.board.linking = match matches!(key, Key::Escape | Key::Enter) {
                    true => None,
                    false => Some(Linking::Typing { id, typed, picked: ctrl_a && still }),
                };
                true
            }
        }
    }

    /// Words pasted while a link is typed, into it. Whether they were.
    pub(in crate::studio) fn link_pasted(&mut self, words: &str) -> bool {
        let Some(Linking::Typing { typed, picked, .. }) = &mut self.board.linking else { return false };
        if std::mem::take(picked) {
            typed.clear();
        }
        typed.push_str(words.trim());
        self.dirty = true;
        true
    }

    /// Where a layer's badge is, in the window: on its top right corner, a
    /// little out from it; picked out, farther out, clear of the handle
    /// there.
    fn badge_at(&mut self, index: usize) -> (f32, f32) {
        let (x, y, w, _) = match &self.doc.layers[index].kind {
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse) => shape.frame(),
            _ => self.doc.layers[index].bounds(&mut self.text),
        };
        let corner = self.to_window_f((x + w, y));
        let out = if self.selected.contains(&index) { 15.0 } else { 2.0 };
        (corner.0 + out, corner.1 - out)
    }

    /// The badges of the shapes that link somewhere and are in view, each
    /// with its layer, where it is, and whether it goes to the web.
    fn badges(&mut self) -> Vec<(usize, (f32, f32), bool)> {
        let view = Rect::new(0, 0, self.width, self.height);
        let linked: Vec<(usize, bool)> = (0..self.doc.layers.len()).filter(|&index| !self.doc.layers[index].hidden).filter_map(|index| Some((index, !on_board(self.link_of(index)?)))).collect();
        linked.into_iter().filter_map(|(index, web)| {
            let at = self.badge_at(index);
            view.inflate(16).contains(at.0 as i32, at.1 as i32).then_some((index, at, web))
        }).collect()
    }

    /// Where the card a link's badge shows is, by the badge: under it and
    /// to its right, as wide as the address it says, within the window.
    fn link_card_rect(&mut self, index: usize) -> Option<Rect> {
        let link = self.link_of(index)?.to_owned();
        let says = self.link_says(&link);
        let wide = (self.text.width(&says, 12) + 52).clamp(220, CARD_MOST);
        let (bx, by) = self.badge_at(index);
        let rect = Rect::new(bx as i32 - 18, by as i32 + BADGE as i32 + 8, wide, 2 * CARD_ROW + 1);
        Some(rect.kept_within(Rect::new(INSET, INSET, self.width - 2 * INSET, self.height - 2 * INSET)))
    }

    /// What a link's card says of where it goes.
    fn link_says(&mut self, link: &str) -> String {
        match on_board(link) {
            true => match self.link_target(link) {
                Some((_, _, name)) => format!("Go to {name}"),
                None => "What it linked to is gone".to_owned(),
            },
            false => shown_address(link).to_owned(),
        }
    }

    /// Where the field a link is typed into is: under the shape, from its
    /// left, clear of it and of what it says.
    fn link_editor_rect(&mut self, id: u32) -> Option<Rect> {
        let index = self.index_of(id)?;
        let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
        let (a, b) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
        let left = if b.0 - a.0 >= EDITOR_W { a.0 } else { (a.0 + b.0) / 2 - EDITOR_W / 2 };
        let rect = Rect::new(left, b.1 + 12, EDITOR_W, EDITOR_H);
        Some(rect.kept_within(Rect::new(INSET, INSET, self.width - 2 * INSET, self.height - 2 * INSET)))
    }

    /// The badges, the card of the one hovered with its buttons, and the
    /// field a link is typed into with its Link to… button.
    pub(in crate::studio) fn layout_links(&mut self, layout: &mut Layout) {
        if let Some(Linking::Typing { id, .. }) = self.board.linking {
            if let Some(editor) = self.link_editor_rect(id) {
                let pick = Rect::new(editor.right() - 8 - 76, editor.y + 8, 76, editor.h - 16);
                layout.controls.push((pick, Control::Board(Ctl::LinkPick)));
                layout.controls.push((editor, Control::Board(Ctl::LinkEditor)));
            }
        }
        if let Some(index) = self.board.link_card.and_then(|id| self.index_of(id)).filter(|_| self.board.linking.is_none()) {
            if let Some(card) = self.link_card_rect(index) {
                let third = (card.w - 12) / 3;
                for (at, ctl) in [Ctl::LinkEdit, Ctl::LinkCopy, Ctl::LinkRemove].into_iter().enumerate() {
                    layout.controls.push((Rect::new(card.x + 6 + at as i32 * third, card.y + CARD_ROW + 4, third, CARD_ROW - 8), Control::Board(ctl)));
                }
                layout.controls.push((card, Control::Board(Ctl::LinkCard)));
            }
        }
        for (index, (x, y), _) in self.badges() {
            let reach = BADGE as i32 + 3;
            layout.controls.push((Rect::new(x as i32 - reach, y as i32 - reach, 2 * reach, 2 * reach), Control::Board(Ctl::LinkBadge(self.id_of(index)))));
        }
    }

    /// The pointer moved on a board: the card of the badge it is over shown,
    /// and kept while it goes from the badge onto the card.
    pub(in crate::studio) fn link_hover(&mut self, over: Option<Control>) {
        let now = match over {
            Some(Control::Board(Ctl::LinkBadge(id))) => Some(id),
            Some(Control::Board(Ctl::LinkCard | Ctl::LinkEdit | Ctl::LinkCopy | Ctl::LinkRemove)) => self.board.link_card,
            _ => {
                // Between the badge and its card, still shown.
                let between = self.board.link_card.and_then(|id| self.index_of(id)).and_then(|index| {
                    let card = self.link_card_rect(index)?;
                    let (bx, by) = self.badge_at(index);
                    let gap = Rect::spanning((bx as i32 - 14, by as i32), (bx as i32 + 14, card.y + 2));
                    gap.contains(self.pointer.0, self.pointer.1).then_some(())
                });
                between.and(self.board.link_card)
            }
        };
        if now != self.board.link_card {
            self.board.link_card = now;
            self.dirty = true;
        }
    }

    /// A part of a link's badge, card or field pressed.
    pub(in crate::studio) fn press_link_ctl(&mut self, ctl: Ctl) {
        self.dirty = true;
        let card = self.board.link_card.and_then(|id| self.index_of(id));
        match ctl {
            Ctl::LinkBadge(id) => {
                if let Some(index) = self.index_of(id) {
                    self.follow_link(index);
                }
            }
            Ctl::LinkEdit => {
                if let Some(index) = card {
                    self.selected = vec![index];
                    match self.link_of(index).is_some_and(on_board) {
                        true => self.begin_link_to(Some(index)),
                        false => self.begin_link(),
                    }
                }
            }
            Ctl::LinkCopy => {
                if let Some(link) = card.and_then(|index| self.link_of(index)).map(str::to_owned) {
                    let copied = if cfg!(test) { Ok(()) } else { crate::clipboard::copy_text(&link) };
                    self.board.copied_link = Some(link);
                    match copied {
                        Ok(()) => self.say("Link copied"),
                        Err(error) => self.say(format!("Could not copy: {error}")),
                    }
                }
            }
            Ctl::LinkRemove => {
                if let Some(index) = card {
                    let id = self.id_of(index);
                    self.set_link(id, String::new());
                    self.say("Link removed");
                }
            }
            Ctl::LinkPick => {
                if let Some(Linking::Typing { id, .. }) = self.board.linking {
                    let index = self.index_of(id);
                    self.board.linking = None;
                    self.begin_link_to(index);
                }
            }
            _ => {}
        }
    }

    /// What a link's control says, hovered.
    pub(in crate::studio) fn link_tip(&mut self, ctl: Ctl) -> Option<String> {
        match ctl {
            Ctl::LinkPick => Some("Click a shape or frame on the board to link to".to_owned()),
            _ => None,
        }
    }

    /// The badges over the board, each on its shape's corner: drawn by hand
    /// in the looks drawn by hand, a little uneven and edged in a deeper
    /// shade of itself; cleanly in the others, on a ring of white with a
    /// soft shadow under it.
    pub(in crate::studio) fn draw_badges(&mut self, canvas: &mut Canvas) {
        let by_hand = self.pages[self.page].look < 2;
        let diagram = self.pages[self.page].look == look::DIAGRAM;
        for (index, (cx, cy), web) in self.badges() {
            let hovered = self.board.link_card == Some(self.id_of(index));
            let color = match (web, diagram) {
                (true, false) => WEB,
                (false, false) => ON_BOARD,
                (true, true) => look::figjam(WEB),
                (false, true) => look::figjam(ON_BOARD),
            };
            let color = if hovered { canvas::mix(color, 0x000000, 40) } else { color };
            let r = BADGE;
            let seed = (self.id_of(index) % 97) as f32;
            match by_hand {
                true => {
                    let ring: Vec<(f32, f32)> = (0..=40).map(|step| {
                        let angle = std::f32::consts::TAU * step as f32 / 40.0;
                        let wobble = 1.0 + 0.045 * (angle * 3.0 + seed).sin() + 0.03 * (angle * 5.0 + seed * 1.3).sin();
                        (cx + angle.cos() * r * wobble, cy + angle.sin() * r * wobble)
                    }).collect();
                    let bounds = Rect::new((cx - r * 1.2).floor() as i32, (cy - r * 1.2).floor() as i32, (r * 2.4).ceil() as i32 + 2, (r * 2.4).ceil() as i32 + 2);
                    canvas.mask(&shapes::fill_path(bounds, &ring), color, 256);
                    // Its edge drawn round once and a little past, deeper.
                    let edge: Vec<(f32, f32)> = (0..=46).map(|step| {
                        let angle = std::f32::consts::TAU * step as f32 / 40.0 - 0.8;
                        let wobble = 1.0 + 0.05 * (angle * 2.0 + seed * 0.7).sin();
                        (cx + angle.cos() * r * wobble, cy + angle.sin() * r * wobble)
                    }).collect();
                    halo(canvas, &edge, 0.7, canvas::mix(color, 0x000000, 90), 230);
                }
                false => {
                    canvas.disc(cx.round() as i32, cy.round() as i32 + 1, r as i32 + 3, 0x000000, 26);
                    canvas.disc(cx.round() as i32, cy.round() as i32, r as i32 + 2, 0xFFFFFF, 256);
                    canvas.disc(cx.round() as i32, cy.round() as i32, r as i32, color, 256);
                }
            }
            // An arrow out for the web, an arrow along for the board.
            let jitter = |x: f32, y: f32, k: f32| if by_hand { (x + 0.35 * (seed + k).sin(), y + 0.35 * (seed * 1.7 + k).cos()) } else { (x, y) };
            let (stem, head): ([(f32, f32); 2], [(f32, f32); 3]) = match web {
                true => ([jitter(cx - 3.5, cy + 3.5, 1.0), jitter(cx + 3.2, cy - 3.2, 2.0)], [jitter(cx - 0.6, cy - 3.6, 3.0), jitter(cx + 3.6, cy - 3.6, 4.0), jitter(cx + 3.6, cy + 0.6, 5.0)]),
                false => ([jitter(cx - 4.2, cy, 1.0), jitter(cx + 3.8, cy, 2.0)], [jitter(cx + 0.6, cy - 3.4, 3.0), jitter(cx + 4.0, cy, 4.0), jitter(cx + 0.6, cy + 3.4, 5.0)]),
            };
            halo(canvas, &stem, 0.85, 0xFFFFFF, 256);
            halo(canvas, &head, 0.85, 0xFFFFFF, 256);
        }
    }

    /// The card of the badge hovered, and the field a link is typed into:
    /// hotshot's own dark, over the board.
    pub(in crate::studio) fn draw_link_card(&mut self, canvas: &mut Canvas, layout: &Layout, hover: Option<Control>) {
        let rect_of = |ctl: Ctl| layout.controls.iter().find(|(_, control)| *control == Control::Board(ctl)).map(|(rect, _)| *rect);
        if let (Some(editor), Some(Linking::Typing { typed, picked, .. })) = (rect_of(Ctl::LinkEditor), self.board.linking.clone()) {
            soft_shadow(canvas, editor, 10);
            canvas.round_fill(editor.x, editor.y, editor.w, editor.h, 10, PANEL_COLOR, 256);
            canvas.round_outline(editor.x, editor.y, editor.w, editor.h, 10, 1, 0x4A4A4A);
            let cy = editor.y + editor.h / 2;
            self.text.draw_centered(canvas, editor.x + 20, cy, 14, TEXT_COLOR, icons::LINK);
            let pick = rect_of(Ctl::LinkPick).unwrap_or(editor);
            let field = Rect::new(editor.x + 36, editor.y + 6, pick.x - 8 - editor.x - 36, editor.h - 12);
            let (px, line) = (13, Text::line_height(13));
            let top = cy - line / 2;
            canvas.set_clip(field.intersect(canvas.clip()));
            let wide = if typed.is_empty() { 0 } else { self.text.width(&typed, px) };
            let x = field.x + (field.w - 6 - wide).min(0);
            match typed.is_empty() {
                true => self.text.draw(canvas, field.x + 5, top, px, 0x7A7A7A, "Paste or type a link"),
                false => {
                    if picked {
                        canvas.round_fill(x - 2, top, wide + 4, line, 3, ACCENT, 140);
                    }
                    self.text.draw(canvas, x, top, px, TEXT_COLOR, &typed);
                }
            }
            if !picked {
                canvas.fill(x + wide + 1, top + 1, 2, line - 2, ACCENT_TEXT);
            }
            canvas.reset_clip();
            let on = hover == Some(Control::Board(Ctl::LinkPick));
            canvas.round_fill(pick.x, pick.y, pick.w, pick.h, 7, if on { HOVER_COLOR } else { FIELD_COLOR }, 256);
            self.text.draw_centered(canvas, pick.x + pick.w / 2, pick.y + pick.h / 2, 12, TEXT_COLOR, "Link to\u{2026}");
        }
        let Some(card) = rect_of(Ctl::LinkCard) else { return };
        let Some(index) = self.board.link_card.and_then(|id| self.index_of(id)) else { return };
        let Some(link) = self.link_of(index).map(str::to_owned) else { return };
        soft_shadow(canvas, card, 10);
        canvas.round_fill(card.x, card.y, card.w, card.h, 10, PANEL_COLOR, 256);
        canvas.round_outline(card.x, card.y, card.w, card.h, 10, 1, 0x444444);
        // Where it goes, after a small mark of which kind it is.
        let web = !on_board(&link);
        let cy = card.y + CARD_ROW / 2 + 1;
        let mark = if web { WEB } else { ON_BOARD };
        canvas.disc(card.x + 20, cy, 8, mark, 256);
        let (mx, my) = (card.x as f32 + 20.0, cy as f32);
        match web {
            true => {
                halo(canvas, &[(mx - 2.8, my + 2.8), (mx + 2.6, my - 2.6)], 0.7, 0xFFFFFF, 256);
                halo(canvas, &[(mx - 0.4, my - 2.9), (mx + 2.9, my - 2.9), (mx + 2.9, my + 0.4)], 0.7, 0xFFFFFF, 256);
            }
            false => {
                halo(canvas, &[(mx - 3.4, my), (mx + 3.0, my)], 0.7, 0xFFFFFF, 256);
                halo(canvas, &[(mx + 0.4, my - 2.7), (mx + 3.2, my), (mx + 0.4, my + 2.7)], 0.7, 0xFFFFFF, 256);
            }
        }
        let says = self.link_says(&link);
        let room = card.w - 36 - 14;
        // Cut short with an ellipsis where it is too long for the card.
        let mut shown = says.clone();
        if self.text.width(&shown, 12) > room {
            while shown.chars().count() > 1 && self.text.width(&format!("{shown}\u{2026}"), 12) > room {
                shown.pop();
            }
            shown.push('\u{2026}');
        }
        self.text.draw(canvas, card.x + 36, cy - Text::line_height(12) / 2, 12, TEXT_COLOR, &shown);
        canvas.fill(card.x + 1, card.y + CARD_ROW, card.w - 2, 1, EDGE_COLOR);
        for (ctl, says) in [(Ctl::LinkEdit, "Edit"), (Ctl::LinkCopy, "Copy"), (Ctl::LinkRemove, "Remove")] {
            let Some(button) = rect_of(ctl) else { continue };
            let on = hover == Some(Control::Board(ctl));
            if on {
                canvas.round_fill(button.x, button.y, button.w, button.h, 6, HOVER_COLOR, 256);
            }
            let ink = match (ctl, on) {
                (Ctl::LinkRemove, true) => 0xFF8A80,
                (_, true) => TEXT_COLOR,
                _ => DIM_TEXT_COLOR,
            };
            self.text.draw_centered(canvas, button.x + button.w / 2, button.y + button.h / 2, 12, ink, says);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;
    use crate::studio::menu::Item;

    const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };

    /// A board with two boxes, the first picked out.
    fn two_boxes() -> Studio {
        let mut studio = board();
        for (from, to, words) in [((100.0, 300.0), (260.0, 372.0), "Docs"), ((2400.0, 1600.0), (2560.0, 1672.0), "Far away")] {
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, from, to, Mods::default());
            type_in(&mut studio, words);
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
        }
        studio.selected = vec![0];
        studio
    }

    /// The right click's row that says so, clicked.
    fn context_row(studio: &mut Studio, says: &str) {
        let at = studio.to_window_page((180.0, 336.0));
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(3, 100_000);
        studio.released();
        let row = studio.menu_items(MenuKind::Context).iter().position(|item| matches!(item, Item::Entry { says: row, .. } if row == says)).unwrap_or_else(|| panic!("{says} in the menu"));
        let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
        click(studio, rect.center());
    }

    #[test]
    fn a_link_typed_from_the_right_click_is_kept_in_the_file_and_one_step_to_undo() {
        let mut studio = two_boxes();
        context_row(&mut studio, "Add link\u{2026}");
        assert!(matches!(studio.board.linking, Some(Linking::Typing { .. })));
        type_in(&mut studio, "example.com/docs");
        // Typed into the field, no letter took up a tool or typed over it.
        assert_eq!(shape(&studio, 0).text, "Docs");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!(shape(&studio, 0).link, "https://example.com/docs");
        assert!(studio.board.linking.is_none());
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        let kept = back[0].loose.layers.iter().find_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.text == "Docs" => Some(shape.link.clone()),
            _ => None,
        });
        assert_eq!(kept.as_deref(), Some("https://example.com/docs"));
        studio.undo();
        assert_eq!(shape(&studio, 0).link, "", "one step");
        // Esc leaves it as it was.
        studio.redo();
        studio.selected = vec![0];
        context_row(&mut studio, "Edit link\u{2026}");
        type_in(&mut studio, "other.org");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(shape(&studio, 0).link, "https://example.com/docs");
        // And a new box made from it by the keys has none of its own.
        studio.selected = vec![0];
        press_key(&mut studio, Key::Right, CTRL);
        let made = studio.picked().unwrap();
        assert_eq!(shape(&studio, made).link, "");
    }

    #[test]
    fn link_to_then_a_click_on_the_board_links_to_what_is_there_and_following_it_goes_there() {
        let mut studio = two_boxes();
        context_row(&mut studio, "Link to\u{2026}");
        assert!(matches!(studio.board.linking, Some(Linking::Picking { .. })));
        // Far off: zoomed out to click it, and back.
        let (zoom, pan) = (studio.zoom, studio.pan);
        (studio.zoom, studio.pan) = (0.25, (0.0, 0.0));
        tap(&mut studio, (2480.0, 1636.0));
        assert_eq!(shape(&studio, 0).link, format!("#{}", studio.id_of(1)));
        assert_eq!(studio.selected, vec![0], "the shape linked from still picked out");
        (studio.zoom, studio.pan) = (zoom, pan);
        // Ctrl and a click on it: the view over to the other, picked out.
        studio.selected.clear();
        let at = studio.to_window_page((180.0, 336.0));
        studio.pointer_moved(at.0, at.1, CTRL);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
        assert_eq!(studio.selected, vec![1]);
        studio.end_glide();
        let middle = studio.to_window((2480.0, 1636.0));
        assert!(studio.open_area().contains(middle.0, middle.1), "{middle:?}");
        // A plain click on it does not follow it.
        let mut plain = two_boxes();
        plain.doc.layers.iter_mut().take(1).for_each(|layer| if let Kind::Shape(shape) = &mut layer.kind { shape.link = "https://example.com".into() });
        tap(&mut plain, (180.0, 336.0));
        assert!(plain.board.opened.is_none());
        // Ctrl and a click on a web link opens it, as the browser would.
        let at = plain.to_window_page((180.0, 336.0));
        plain.pointer_moved(at.0, at.1, CTRL);
        plain.last_press.0 = 0;
        plain.pressed(1, 100_000);
        plain.released();
        assert_eq!(plain.board.opened.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn a_badge_shows_on_a_linked_shape_and_its_card_edits_copies_and_removes() {
        for look in 0..=look::DIAGRAM {
            let mut studio = two_boxes();
            studio.set_look(Some(look), None);
            studio.selected.clear();
            let mut before = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut before);
            if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
                shape.link = "https://example.com/a".into();
            }
            studio.changed();
            let mut after = vec![0u32; before.len()];
            studio.draw(&mut after);
            // On its top right corner, blue.
            let badge = studio.control_rect(Control::Board(Ctl::LinkBadge(studio.id_of(0)))).unwrap();
            let corner = studio.to_window_page((260.5, 300.5));
            assert!(badge.contains(corner.0, corner.1), "look {look}: {badge:?}");
            let (cx, cy) = badge.center();
            let pixel = after[(cy * studio.width + cx - 5) as usize];
            let blue = |pixel: u32| (pixel & 0xFF) > 150 && (pixel >> 16 & 0xFF) < 120;
            assert!(blue(pixel) && before[(cy * studio.width + cx - 5) as usize] != pixel, "look {look}: blue there {pixel:06X}");
            // Hovered, its card; the pointer onto the card keeps it.
            studio.pointer_moved(cx, cy, Mods::default());
            assert_eq!(studio.board.link_card, Some(studio.id_of(0)));
            let copy = studio.control_rect(Control::Board(Ctl::LinkCopy)).unwrap();
            studio.pointer_moved(cx, (cy + copy.y) / 2, Mods::default());
            studio.pointer_moved(copy.center().0, copy.center().1, Mods::default());
            assert_eq!(studio.board.link_card, Some(studio.id_of(0)), "look {look}: kept on the way");
            click(&mut studio, copy.center());
            assert_eq!(studio.board.copied_link.as_deref(), Some("https://example.com/a"));
            // Edit: the field, with the address picked out to type over.
            studio.pointer_moved(cx, cy, Mods::default());
            let edit = studio.control_rect(Control::Board(Ctl::LinkEdit)).unwrap();
            studio.pointer_moved(edit.center().0, edit.center().1, Mods::default());
            click(&mut studio, edit.center());
            type_in(&mut studio, "hotshot.dev");
            press_key(&mut studio, Key::Enter, Mods::default());
            assert_eq!(shape(&studio, 0).link, "https://hotshot.dev");
            // Remove: gone, and so is its badge, out from the corner now as
            // the shape is picked out, clear of its handle.
            let (cx, cy) = studio.control_rect(Control::Board(Ctl::LinkBadge(studio.id_of(0)))).unwrap().center();
            assert!(cx > corner.0 + 6 && cy < corner.1 - 6, "look {look}: out from the corner");
            studio.pointer_moved(cx, cy, Mods::default());
            let remove = studio.control_rect(Control::Board(Ctl::LinkRemove)).unwrap();
            studio.pointer_moved(remove.center().0, remove.center().1, Mods::default());
            click(&mut studio, remove.center());
            assert_eq!(shape(&studio, 0).link, "");
            assert!(studio.control_rect(Control::Board(Ctl::LinkBadge(studio.id_of(0)))).is_none());
            // Away from both, no card.
            if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
                shape.link = "#1".into();
            }
            studio.pointer_moved(cx, cy, Mods::default());
            studio.pointer_moved(700, 700, Mods::default());
            assert_eq!(studio.board.link_card, None, "look {look}");
        }
    }

    #[test]
    fn a_link_to_a_frame_of_the_board_goes_to_the_frame_and_ctrl_k_finds_links_and_find() {
        let mut studio = two_boxes();
        let mut frame = Doc::new(600, 400);
        (frame.name, frame.x, frame.y) = ("Hero".into(), 1000.0, -1200.0);
        studio.pages[0].frames.push(frame);
        // Ctrl+K has it, for the shape picked out.
        let acts = studio.board_acts();
        assert!(acts.iter().any(|(says, _, act)| says == "Link to a shape or frame" && *act == Act::LinkTo));
        assert!(acts.iter().any(|(says, keys, act)| says == "Find on the board" && *keys == "Ctrl+F" && *act == Act::Find));
        studio.run_menu(crate::studio::menu::MenuAct::Board(Act::LinkTo));
        (studio.zoom, studio.pan) = (0.25, (100.0, 400.0));
        tap(&mut studio, (1300.0, -1000.0));
        assert_eq!(shape(&studio, 0).link, "frame:Hero");
        // Followed, the frame in the middle of the view.
        (studio.zoom, studio.pan) = (1.0, (0.0, 0.0));
        studio.follow_link(0);
        studio.end_glide();
        let middle = studio.to_window_page((1300.0, -1000.0));
        let open = studio.open_area();
        assert!((middle.0 - open.center().0).abs() < 4 && open.contains(middle.0, middle.1), "{middle:?}");
        // Gone, it says so and goes nowhere.
        studio.pages[0].frames.clear();
        let view = (studio.zoom, studio.pan);
        studio.follow_link(0);
        assert_eq!((studio.zoom, studio.pan, studio.board.glide.is_none()), (view.0, view.1, true));
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("What it linked to is gone"));
        studio.run_menu(crate::studio::menu::MenuAct::Board(Act::Find));
        assert!(studio.board.find.is_some());
    }

    #[test]
    fn presenting_a_click_on_a_linked_shape_follows_it() {
        let mut studio = two_boxes();
        if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
            shape.link = "https://example.com".into();
        }
        studio.present();
        let at = studio.to_window_page((180.0, 336.0));
        click(&mut studio, at);
        assert_eq!(studio.board.opened.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn what_is_typed_as_a_link_is_made_one() {
        assert_eq!(as_link(" example.com "), "https://example.com");
        assert_eq!(as_link("http://a.b"), "http://a.b");
        assert_eq!(as_link("mailto:me@x.org"), "mailto:me@x.org");
        assert_eq!(as_link("#12"), "#12");
        assert_eq!(as_link(""), "");
        assert_eq!(shown_address("https://example.com/"), "example.com");
    }

    /// A box with a link to the web and one with a link on the board, the
    /// first's card shown, and the field a link is typed into, in each look:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test link_snapshot -- --ignored
    #[test]
    #[ignore]
    fn link_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        for (look, ground) in [(0u8, 0u8), (1, 0), (2, 0), (look::DIAGRAM, 0), (0, 1)] {
            let mut studio = board();
            studio.set_look(Some(look), Some(ground));
            for (from, till, words) in [((300.0, 200.0), (460.0, 272.0), "Design doc"), ((620.0, 200.0), (780.0, 272.0), "See: Auth")] {
                press_key(&mut studio, Key::Char('r'), Mods::default());
                pull(&mut studio, from, till, Mods::default());
                type_in(&mut studio, words);
                press_key(&mut studio, Key::Escape, Mods::default());
                press_key(&mut studio, Key::Escape, Mods::default());
            }
            let other = studio.id_of(0);
            if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
                shape.link = "https://docs.example.com/design/hotshot-board".into();
            }
            if let Kind::Shape(shape) = &mut studio.doc.layers[1].kind {
                shape.link = format!("#{other}");
            }
            studio.selected.clear();
            studio.changed();
            let badge = studio.control_rect(Control::Board(Ctl::LinkBadge(studio.id_of(0)))).unwrap();
            studio.pointer_moved(badge.center().0, badge.center().1, Mods::default());
            let edit = studio.control_rect(Control::Board(Ctl::LinkEdit)).unwrap();
            studio.pointer_moved(badge.center().0, (badge.center().1 + edit.y) / 2, Mods::default());
            studio.pointer_moved(edit.center().0, edit.center().1, Mods::default());
            let shoot = |studio: &mut Studio, name: String| {
                let mut out = vec![0u32; (studio.width * studio.height) as usize];
                studio.draw(&mut out);
                let (x, y, w, h) = (240, 140, 640, 280);
                let crop: Vec<u32> = (y..y + h).flat_map(|row| out[(row * studio.width + x) as usize..(row * studio.width + x + w) as usize].to_vec()).collect();
                std::fs::write(to.join(format!("{name}.png")), crate::output::encode(w as u32, h as u32, &crop, false).unwrap()).unwrap();
            };
            shoot(&mut studio, format!("link-{look}-{ground}"));
            studio.selected = vec![1];
            studio.board.link_card = None;
            studio.begin_link();
            type_in(&mut studio, "figma.com/file/abc");
            studio.pointer_moved(10, 600, Mods::default());
            shoot(&mut studio, format!("link-typing-{look}-{ground}"));
        }
    }
}
