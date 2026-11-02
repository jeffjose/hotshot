//! A board's bar of styles, floating by what is picked out rather than in
//! a panel: five line colors and five fills as Excalidraw keeps them, no
//! fill, by hand or not, how large the words are, a status dot, and
//! guessing on or off. With nothing picked out and a tool in hand, it sits
//! over the tools, for the next shape; and what it last set carries on to
//! the next shape drawn.

use super::*;
use std::time::{Duration, Instant};

/// The sizes of words − and + step through, as Figma and Google Docs list
/// them; any other from 1 to 999 can be typed.
pub(in crate::studio) const LADDER: [u32; 22] = [8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 48, 56, 64, 72, 96, 128];

/// How long − or + is held before it steps again by itself, and how often
/// it does then.
const HOLD: Duration = Duration::from_millis(400);
const REPEAT: Duration = Duration::from_millis(80);

/// How long after the wheel last stepped the size over its number that a
/// notch more is still the same step to undo.
const WHEEL_RUN: Duration = Duration::from_millis(700);

/// The size of words next above one on the ladder, or next below it; one
/// past either end of it left as it is.
pub(in crate::studio) fn step_px(px: u32, larger: bool) -> u32 {
    match larger {
        true => LADDER.iter().copied().find(|&size| size > px).unwrap_or(px),
        false => LADDER.iter().copied().rev().find(|&size| size < px).unwrap_or(px),
    }
}

/// How large a swatch of the bar is, the room round them, and the gap
/// between groups.
const SWATCH: i32 = 22;
const GAP: i32 = 4;
const PAD: i32 = 6;
const SPLIT: i32 = 11;
/// How wide − and + are, and the number between them.
const STEP: i32 = 20;
const NUMBER: i32 = 34;

impl Studio {
    /// The shapes picked out the bar styles.
    fn bar_styled(&self) -> Vec<usize> {
        self.chosen().into_iter().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Shape(_))).collect()
    }

    /// The bar's place and its parts, over what is picked out, or under it
    /// near the top of the window; or over the tools for the next shape.
    pub(in crate::studio) fn layout_style_bar(&mut self, layout: &mut Layout) {
        let dragging = matches!(self.drag, Drag::Move { moved: true, .. } | Drag::Scale { .. } | Drag::Turn { .. } | Drag::Draw { .. } | Drag::Marquee { .. } | Drag::Pull { .. });
        // Kept while its own list of fonts is open from it, which says so.
        let menu = self.menu.is_some_and(|menu| !matches!(menu.kind, MenuKind::Fonts | MenuKind::Looks | MenuKind::Grounds));
        if self.zen || self.presenting() || dragging || self.search.is_some() || menu {
            return;
        }
        let styled = self.bar_styled();
        let boxes = styled.iter().any(|&index| self.is_box(index));
        let mut parts: Vec<(Ctl, i32)> = Vec::new();
        parts.extend((0..5).map(|at| (Ctl::Stroke(at), SWATCH)));
        parts.push((Ctl::Guess, -SPLIT));
        parts.extend((0..5).map(|at| (Ctl::Fill(at), SWATCH)));
        parts.push((Ctl::NoFill, SWATCH));
        parts.push((Ctl::Guess, -SPLIT));
        parts.push((Ctl::Look, 74));
        parts.push((Ctl::Ground, SWATCH + 18));
        parts.push((Ctl::Guess, -SPLIT));
        // As wide as the font's name, in itself, and its arrow.
        let face = self.shown_font();
        parts.push((Ctl::Font, self.text.width_in(face, FACES[face], 13) + 30));
        // How large the words are: −, the number, and +.
        parts.extend([(Ctl::Words(0), STEP), (Ctl::Words(1), NUMBER), (Ctl::Words(2), STEP)]);
        if boxes {
            parts.push((Ctl::Status, SWATCH + 4));
        }
        if self.tool == Tool::Pen {
            parts.push((Ctl::Neaten, 64));
        }
        parts.push((Ctl::Guess, SWATCH + 4));
        let wide = parts.iter().map(|(_, w)| w.abs() + GAP).sum::<i32>() - GAP + 2 * PAD;
        let high = SWATCH + 2 * PAD;
        let view = self.open_area();
        let (cx, top) = match styled.is_empty() {
            true if self.tool != Tool::Select && layout.dock.w > 0 => (layout.dock.center().0, layout.dock.y - 10 - high),
            true => return,
            false => {
                let corners: Vec<(i32, i32)> = styled.iter().flat_map(|&index| self.box_corners(index)).collect();
                let (left, right) = (corners.iter().map(|c| c.0).min().unwrap_or(0), corners.iter().map(|c| c.0).max().unwrap_or(0));
                let (above, below) = (corners.iter().map(|c| c.1).min().unwrap_or(0), corners.iter().map(|c| c.1).max().unwrap_or(0));
                // Clear of the dots of its sides, which are pulled from;
                // over it, or under it where that covers less of the rest,
                // so that the bar takes no click meant for another shape.
                let x = ((left + right) / 2 - wide / 2).clamp(view.x + 4, (view.right() - wide - 4).max(view.x + 4));
                let (over, under) = (above - 30 - high, below + 30);
                let rest: Vec<usize> = (0..self.doc.layers.len()).filter(|index| !styled.contains(index) && !self.doc.layers[*index].hidden).collect();
                let others: Vec<Rect> = rest.into_iter().map(|index| {
                    let (bx, by, bw, bh) = self.doc.layers[index].bounds(&mut self.text);
                    Rect::spanning(self.to_window((bx, by)), self.to_window((bx + bw, by + bh)))
                }).collect();
                let covers = |top: i32| {
                    let bar = Rect::new(x, top, wide, high);
                    others.iter().map(|other| {
                        let both = bar.intersect(*other);
                        if both.w > 0 && both.h > 0 { both.w * both.h } else { 0 }
                    }).sum::<i32>()
                };
                let fits = |top: i32| top >= view.y + 40 && top + high <= view.bottom() - 4;
                // Then over or under each box round it, from the nearest out,
                // as both of its own would be over the box it is in.
                let mut holders: Vec<(f32, f32, f32, f32)> = styled.first().map(|&index| self.round(index)).unwrap_or_default().into_iter().filter_map(|id| self.index_of(id)).map(|index| self.box_frame(index)).collect();
                holders.sort_by(|a, b| (a.2 * a.3).total_cmp(&(b.2 * b.3)));
                let mut places = vec![over, under];
                for (_, y, _, h) in holders {
                    places.push(self.to_window((0.0, y)).1 - 30 - high);
                    places.push(self.to_window((0.0, y + h)).1 + 30);
                }
                let top = places.iter().copied().filter(|&top| fits(top)).min_by_key(|&top| covers(top)).unwrap_or(under);
                ((left + right) / 2, top)
            }
        };
        let x = (cx - wide / 2).clamp(view.x + 4, (view.right() - wide - 4).max(view.x + 4));
        let bar = Rect::new(x, top.clamp(view.y + 40, (self.height - high - 4).max(view.y)), wide, high);
        let mut along = bar.x + PAD;
        for (ctl, w) in parts {
            // A gap between groups, the rest each a part of it.
            if w > 0 {
                layout.controls.push((Rect::new(along, bar.y + PAD, w, SWATCH), Control::Board(ctl)));
            }
            along += w.abs() + GAP;
        }
        layout.controls.push((bar, Control::Panel));
        layout.board_bar = Some(bar);
    }

    /// What the bar shows as set: of the first shape picked out, or of the
    /// next to be drawn.
    fn shown_style(&self) -> (u32, Option<u32>, bool, u8) {
        let first = self.bar_styled().first().and_then(|&index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.clone()),
            Kind::Picture(_) => None,
        });
        match first {
            Some(shape) => (shape.color, shape.filled.then_some(shape.fill), shape.sketch, shape.status),
            None => (self.board.stroke, self.board.fill, self.board.sketch, 0),
        }
    }

    /// How large the board's next words are.
    pub(in crate::studio) fn board_px(&self) -> u32 {
        self.pages[self.page].px
    }

    /// The shapes whose words − and + make larger or smaller: those picked
    /// out, or the one being typed into.
    fn sized(&self) -> Vec<usize> {
        let styled = self.bar_styled();
        match (styled.is_empty(), self.editing) {
            (true, Some(index)) if matches!(self.doc.layers.get(index).map(|layer| &layer.kind), Some(Kind::Shape(_))) => vec![index],
            _ => styled,
        }
    }

    /// How large the words the bar shows are: of what is picked out, none
    /// where they differ; or of the next to be drawn.
    pub(in crate::studio) fn shown_px(&self) -> Option<u32> {
        let mut sizes = self.sized().into_iter().filter_map(|index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.font_px),
            Kind::Picture(_) => None,
        });
        match sizes.next() {
            Some(first) => sizes.all(|px| px == first).then_some(first),
            None => Some(self.board_px()),
        }
    }

    /// The words picked out, or being typed, each made as large as `to`
    /// makes its own size, and a board's next words so as well; a step to
    /// undo when `undo`, and not as it goes on held down or wheeled.
    pub(in crate::studio) fn size_words(&mut self, to: impl Fn(u32) -> u32, undo: bool) {
        let sized = self.sized();
        let first = sized.first().and_then(|&index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.font_px),
            Kind::Picture(_) => None,
        });
        let next = to(first.unwrap_or_else(|| self.board_px())).clamp(1, MOST_PX);
        if !sized.is_empty() {
            if undo {
                self.checkpoint();
            }
            for &index in &sized {
                if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                    shape.font_px = to(shape.font_px).clamp(1, MOST_PX);
                    shape.touch();
                }
            }
            self.changed();
        }
        if self.on_board() {
            self.pages[self.page].px = next;
        } else if !sized.is_empty() {
            self.say(format!("Text size: {next}"));
        }
        self.dirty = true;
    }

    /// Ctrl+Shift+> or <: the words picked out a size larger or smaller.
    pub(in crate::studio) fn step_words(&mut self, larger: bool) {
        self.size_words(|px| step_px(px, larger), true);
    }

    /// − or + held: a step more each so often, once held a moment.
    pub(in crate::studio) fn step_held(&mut self, now: Instant) {
        let Some((larger, when)) = self.board.stepping else { return };
        if now >= when {
            self.size_words(|px| step_px(px, larger), false);
            self.board.stepping = Some((larger, now + REPEAT));
        }
    }

    /// How soon − or + held steps again.
    pub(in crate::studio) fn step_wait(&self) -> Option<Duration> {
        self.board.stepping.map(|(_, when)| when.saturating_duration_since(Instant::now()))
    }

    /// The wheel over the number of the size of the words: a step for
    /// each notch, up larger, a run of them one step to undo.
    pub(in crate::studio) fn wheel_words(&mut self, down: f32) {
        let (gone, last) = self.board.wheeled;
        let fresh = last.is_none_or(|when| when.elapsed() > WHEEL_RUN);
        let gone = if fresh { -down } else { gone - down };
        let steps = gone.trunc();
        self.board.wheeled.0 = gone - steps;
        for at in 0..steps.abs() as usize {
            self.size_words(|px| step_px(px, steps > 0.0), fresh && at == 0);
        }
        if steps != 0.0 || fresh {
            self.board.wheeled.1 = Some(Instant::now());
        }
    }

    /// A key while the size of the words is typed into the bar: a digit,
    /// Backspace, Enter to set it, Esc to leave it as it was. Whether it
    /// was taken.
    pub(in crate::studio) fn px_typed_key(&mut self, key: Key, mods: Mods) -> bool {
        let Some(typed) = &mut self.board.px_typed else { return false };
        if mods.ctrl {
            return false;
        }
        match key {
            Key::Char(digit) if digit.is_ascii_digit() && typed.len() < 3 => typed.push(digit),
            Key::Backspace => drop(typed.pop()),
            Key::Enter => self.commit_px(),
            Key::Escape => self.board.px_typed = None,
            _ => {}
        }
        self.dirty = true;
        true
    }

    /// The size typed into the bar given to the words, if it is one.
    pub(in crate::studio) fn commit_px(&mut self) {
        let typed = self.board.px_typed.take().and_then(|typed| typed.parse::<u32>().ok());
        if let Some(px) = typed.filter(|px| (1..=MOST_PX).contains(px)) {
            self.size_words(|_| px, true);
        }
    }

    /// The font the bar shows: of the first shape picked out, or the
    /// board's, for the next.
    pub(in crate::studio) fn shown_font(&self) -> usize {
        self.bar_styled().first().and_then(|&index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.face.min(FACES.len() - 1)),
            Kind::Picture(_) => None,
        }).unwrap_or_else(|| self.board_font())
    }

    pub(in crate::studio) fn draw_style_control(&mut self, canvas: &mut Canvas, rect: Rect, ctl: Ctl, hovered: bool) {
        let (stroke, fill, sketch, status) = self.shown_style();
        let (cx, cy) = rect.center();
        let ring = |canvas: &mut Canvas, on: bool| {
            if on {
                canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 8, 2, ACCENT);
            } else if hovered {
                canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 8, 1, DIM_TEXT_COLOR);
            }
        };
        let line_high = Text::line_height(11);
        match ctl {
            Ctl::Stroke(at) => {
                let color = STROKES[usize::from(at)];
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, color, 256);
                canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, 0x555555);
                ring(canvas, stroke == color);
                if self.board.coloring {
                    self.text.draw_centered(canvas, cx, cy, 10, 0xFFFFFF, &(at + 1).to_string());
                }
            }
            Ctl::Fill(at) => {
                let color = FILLS[usize::from(at)];
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, color, 256);
                ring(canvas, fill == Some(color));
                if self.board.coloring {
                    self.text.draw_centered(canvas, cx, cy, 10, INK, &["Q", "W", "E", "R", "T"][usize::from(at)]);
                }
            }
            // The ground's paper, and the arrow of the list it opens.
            Ctl::Ground => {
                if hovered || self.bar_open(MenuKind::Grounds) {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                let (paper, _) = self.paper();
                canvas.disc(rect.x + 11, cy, 8, 0x8F8F8F, 256);
                canvas.disc(rect.x + 11, cy, 7, paper, 256);
                self.text.draw_centered(canvas, rect.right() - 8, cy, 9, DIM_TEXT_COLOR, icons::DOWN);
            }
            Ctl::NoFill => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, 0xFFFFFF, 256);
                line(canvas, (rect.x + 4, rect.bottom() - 5), (rect.right() - 5, rect.y + 4), 0xE03131);
                line(canvas, (rect.x + 5, rect.bottom() - 5), (rect.right() - 5, rect.y + 5), 0xE03131);
                ring(canvas, fill.is_none());
            }
            Ctl::Look => {
                if hovered || self.bar_open(MenuKind::Looks) {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                // How the board draws its shapes and lines: rounded,
                // sketchy or clean, said plainly, as it has nothing to do
                // with the font.
                let says = look::LOOKS[usize::from(self.pages[self.page].look.min(2))];
                let _ = sketch;
                self.text.draw(canvas, rect.x + 7, cy - Text::line_height(12) / 2, 12, TEXT_COLOR, says);
                self.text.draw_centered(canvas, rect.right() - 8, cy, 9, DIM_TEXT_COLOR, icons::DOWN);
            }
            Ctl::Font => {
                let open = matches!(self.menu, Some(MenuState { kind: MenuKind::Fonts, .. }));
                if hovered || open {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                // The font's name, in itself.
                let face = self.shown_font();
                self.text.draw_in(face, canvas, rect.x + 7, cy - Text::line_height(13) / 2, 13, TEXT_COLOR, FACES[face]);
                self.text.draw_centered(canvas, rect.right() - 8, cy, 9, DIM_TEXT_COLOR, icons::DOWN);
            }
            // − and +, a bar and a cross, dim where they would do nothing.
            Ctl::Words(side @ (0 | 2)) => {
                let larger = side == 2;
                let px = self.shown_px();
                let stuck = px.is_some_and(|px| step_px(px, larger) == px);
                if hovered && !stuck || self.board.stepping.is_some_and(|(held, _)| held == larger) {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                let color = if stuck { 0x5C5C5C } else { TEXT_COLOR };
                canvas.fill(cx - 5, cy - 1, 10, 2, color);
                if larger {
                    canvas.fill(cx - 1, cy - 5, 2, 10, color);
                }
            }
            // The number of pixels, a field to type into once clicked, and
            // a dash where what is picked out differs.
            Ctl::Words(_) => {
                let typed = self.board.px_typed.clone();
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, FIELD_COLOR, 256);
                if typed.is_some() || hovered {
                    canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 1, if typed.is_some() { ACCENT } else { 0x555555 });
                }
                // Nothing typed yet: the size it was, faintly, to type over.
                let (says, color) = match typed.as_deref() {
                    Some(typed) if !typed.is_empty() => (typed.to_owned(), TEXT_COLOR),
                    shown => (self.shown_px().map_or_else(|| "\u{2013}".to_owned(), |px| px.to_string()), if shown.is_some() { DIM_TEXT_COLOR } else { TEXT_COLOR }),
                };
                let wide = self.text.width(&says, 12);
                let left = cx - wide / 2;
                self.text.draw(canvas, left, cy - Text::line_height(12) / 2, 12, color, &says);
                if typed.is_some() {
                    let at = if typed.as_deref() == Some("") { left - 2 } else { left + wide + 1 };
                    canvas.fill(at, cy - 7, 1, 14, ACCENT);
                }
            }
            Ctl::Neaten => {
                if self.board.neaten {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, ROW_ON, 256);
                } else if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                self.text.draw(canvas, rect.x + 8, cy - line_high / 2, 11, if self.board.neaten { ACCENT_TEXT } else { TEXT_COLOR }, "Neaten");
            }
            Ctl::Status => {
                if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                match status {
                    1..=3 => canvas.disc(cx, cy, 7, [0xE03131, 0xF08C00, 0x2F9E44][usize::from(status - 1)], 256),
                    _ => canvas.round_outline(cx - 7, cy - 7, 14, 14, 7, 1, DIM_TEXT_COLOR),
                }
            }
            Ctl::Guess => {
                let on = self.board.guessing;
                if on {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, ROW_ON, 256);
                } else if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                // A spark of four points: what comes next, guessed.
                let color = if on { ACCENT_TEXT } else { DIM_TEXT_COLOR };
                for (dx, dy) in [(0, -7), (7, 0), (0, 7), (-7, 0)] {
                    line(canvas, (cx, cy), (cx + dx, cy + dy), color);
                }
                for (dx, dy) in [(-3, -3), (3, -3), (3, 3), (-3, 3)] {
                    line(canvas, (cx, cy), (cx + dx, cy + dy), color);
                }
            }
            _ => {}
        }
    }

    /// A part of the bar pressed: what is picked out styled so, as one
    /// step to undo, and the next shape drawn so as well.
    pub(in crate::studio) fn press_style(&mut self, ctl: Ctl) {
        let shapes = !self.bar_styled().is_empty();
        match ctl {
            Ctl::Stroke(at) => {
                let color = STROKES[usize::from(at)];
                self.board.stroke = color;
                self.restyle_shapes(|shape| shape.color = color);
            }
            Ctl::Fill(at) => {
                let color = FILLS[usize::from(at)];
                self.board.fill = Some(color);
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        (shape.filled, shape.fill) = (true, color);
                    }
                });
            }
            Ctl::NoFill => {
                self.board.fill = None;
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        (shape.filled, shape.border) = (false, true);
                    }
                });
            }
            // Each a list, under the bar, or over it where it is low; a
            // second click on it closes it.
            Ctl::Look => self.open_bar_menu(ctl, MenuKind::Looks),
            Ctl::Font => self.open_bar_menu(ctl, MenuKind::Fonts),
            Ctl::Ground => self.open_bar_menu(ctl, MenuKind::Grounds),
            // − or +, a step at once and then more while held; the number
            // typed into.
            Ctl::Words(side @ (0 | 2)) => {
                let larger = side == 2;
                self.step_words(larger);
                self.board.stepping = Some((larger, Instant::now() + HOLD));
            }
            Ctl::Words(_) => {
                if self.board.px_typed.is_none() {
                    self.board.px_typed = Some(String::new());
                }
            }
            Ctl::Status => {
                let (_, _, _, status) = self.shown_style();
                let next = (status + 1) % 4;
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        shape.status = next;
                    }
                });
                self.say(["No status", "Status: red", "Status: amber", "Status: green"][usize::from(next)]);
            }
            Ctl::Neaten => {
                self.board.neaten = !self.board.neaten;
                self.say(if self.board.neaten { "Neaten: a loop an oval, a square a box, a V an arrow" } else { "Neaten off" });
            }
            Ctl::Guess => {
                self.board.guessing = !self.board.guessing;
                self.say(if self.board.guessing { "Guessing what comes next" } else { "Guessing off" });
            }
            _ => {}
        }
        if !shapes {
            self.dirty = true;
        }
    }

    /// Whether a list of the bar is open from it.
    fn bar_open(&self, kind: MenuKind) -> bool {
        self.menu.is_some_and(|menu| menu.kind == kind)
    }

    /// A list of the bar opened from its part, under it, or over it where
    /// the bar is low; closed, if it was open.
    fn open_bar_menu(&mut self, ctl: Ctl, kind: MenuKind) {
        if self.bar_open(kind) {
            self.menu = None;
            return;
        }
        let place = self.control_rect(Control::Board(ctl)).unwrap_or_default();
        let high = self.menu_high(kind);
        let y = match place.bottom() + 4 + high <= self.height - 6 {
            true => place.bottom() + 8,
            false => place.y - 8 - high,
        };
        self.menu = Some(MenuState { kind, at: (place.x - 6, y), sub: None });
        self.menu_row = None;
        self.dirty = true;
    }

    /// Each shape picked out changed, as one step to undo.
    pub(in crate::studio) fn restyle_shapes(&mut self, change: impl Fn(&mut Shape)) {
        self.restyle(|layer| {
            if let Kind::Shape(shape) = &mut layer.kind {
                change(shape);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    /// Where −, the number or + of the size of the words is.
    fn words_at(studio: &mut Studio, side: u8) -> (i32, i32) {
        studio.layout();
        studio.control_rect(Control::Board(Ctl::Words(side))).expect("the size of the words").center()
    }

    fn sizes(studio: &Studio) -> Vec<u32> {
        (0..studio.doc.layers.len()).map(|index| shape(studio, index).font_px).collect()
    }

    #[test]
    fn the_size_of_words_steps_through_the_usual_sizes_from_its_own() {
        // From one on the ladder or between two, to the next; past its
        // ends, as it is.
        assert_eq!([step_px(16, true), step_px(16, false), step_px(15, true), step_px(15, false), step_px(23, true)], [18, 14, 16, 14, 24]);
        assert_eq!([step_px(128, true), step_px(8, false), step_px(200, false), step_px(3, true)], [128, 8, 128, 8]);
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!((sizes(&studio), studio.shown_px()), (vec![WORDS_PX], Some(WORDS_PX)));
        // + a step larger, each click a step to undo, and the board's next
        // words so too.
        let undos = studio.undo.len();
        let plus = words_at(&mut studio, 2);
        click(&mut studio, plus);
        click(&mut studio, plus);
        assert_eq!((sizes(&studio), studio.pages[0].px, studio.undo.len()), (vec![20], 20, undos + 2));
        let minus = words_at(&mut studio, 0);
        click(&mut studio, minus);
        assert_eq!(sizes(&studio), vec![18]);
        studio.undo();
        assert_eq!(sizes(&studio), vec![20]);
        // Held, it steps again by itself, and all of it is one step to undo.
        let undos = studio.undo.len();
        studio.pointer_moved(plus.0, plus.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        assert!(studio.board_wait().is_some_and(|wait| wait > Duration::from_millis(300)), "a moment before it goes on");
        for _ in 0..3 {
            studio.board.stepping = studio.board.stepping.map(|(larger, _)| (larger, Instant::now()));
            studio.board_tick();
        }
        studio.released();
        assert_eq!((sizes(&studio), studio.undo.len(), studio.board.stepping), (vec![32], undos + 1, None));
        studio.board_tick();
        assert_eq!(sizes(&studio), vec![32], "let go, it stops");
        studio.undo();
        assert_eq!(sizes(&studio), vec![20]);
        // Ctrl+Shift+> and < as well; and the wheel over the number, a run
        // of it one step to undo.
        let ctrl_shift = Mods { ctrl: true, shift: true, alt: false };
        press_key(&mut studio, Key::Char('>'), ctrl_shift);
        assert_eq!(sizes(&studio), vec![22]);
        press_key(&mut studio, Key::Char('<'), ctrl_shift);
        press_key(&mut studio, Key::Char('<'), ctrl_shift);
        assert_eq!(sizes(&studio), vec![18]);
        let undos = studio.undo.len();
        let number = words_at(&mut studio, 1);
        studio.pointer_moved(number.0, number.1, Mods::default());
        studio.scrolled(0.0, -1.0);
        studio.scrolled(0.0, -0.5);
        studio.scrolled(0.0, -0.5);
        assert_eq!((sizes(&studio), studio.undo.len()), (vec![22], undos + 1));
        studio.scrolled(0.0, 1.0);
        assert_eq!(sizes(&studio), vec![20]);
    }

    #[test]
    fn the_size_of_words_is_typed_and_says_where_what_is_picked_out_differs() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        // The number clicked is typed into: Enter sets it, Esc does not,
        // and nor does a size it cannot be.
        let number = words_at(&mut studio, 1);
        click(&mut studio, number);
        type_in(&mut studio, "23");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!((sizes(&studio), studio.pages[0].px, studio.board.px_typed.clone()), (vec![23], 23, None));
        click(&mut studio, number);
        type_in(&mut studio, "5");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!((sizes(&studio), studio.chosen().len()), (vec![23], 1), "Esc leaves it, and what is picked out");
        click(&mut studio, number);
        type_in(&mut studio, "0");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!(sizes(&studio), vec![23]);
        // Clicked away from, what was typed is set.
        click(&mut studio, number);
        type_in(&mut studio, "1234");
        assert_eq!(studio.board.px_typed.as_deref(), Some("123"), "three digits at most");
        tap(&mut studio, (700.0, 600.0));
        assert_eq!((sizes(&studio), studio.board.px_typed.clone()), (vec![123], None));
        // − and + step each of two that differ from its own size; the
        // number a dash meanwhile.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 300.0), (260.0, 372.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(sizes(&studio), vec![123, 123], "the next box drawn as the last was set");
        studio.pages[0].px = 13;
        if let Kind::Shape(shape) = &mut studio.doc.layers[1].kind {
            shape.font_px = 13;
        }
        studio.selected = vec![0, 1];
        assert_eq!(studio.shown_px(), None);
        let plus = words_at(&mut studio, 2);
        click(&mut studio, plus);
        assert_eq!(sizes(&studio), vec![128, 14]);
        // And kept with the board in its file; a board from before it was
        // kept, as a new one is.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert_eq!(back[0].px, studio.pages[0].px);
        let then = String::from_utf8(write_pages(&studio.gathered())).unwrap().replace(&format!(" {}\n", studio.pages[0].px), "\n");
        assert_eq!(read_pages(then.as_bytes()).unwrap()[0].px, WORDS_PX);
    }
}
