//! A board's panel of styles, docked at its right under Present, where it
//! covers nothing picked out: ten colors by name, each a line and its tint
//! at once; a fill of none, the paper, a tint or hatched; a line whole,
//! dashed or dotted; S M L XL; the font, how large the words are, and a
//! status dot. The board's theme, paper and guessing are at its bottom
//! right, being the whole board's. With nothing picked out and
//! a tool in hand it sets the next shape; and what it last set carries on
//! to the next shape drawn.

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

/// How wide the panel is and the room inside its edge; how high a swatch
/// and a row of the panel are, the gap between parts and between rows.
const PANEL_W: i32 = 184;
const PANEL_PAD: i32 = 12;
const SWATCH: i32 = 26;
const ROW: i32 = 28;
const GAP: i32 = 4;
const SPLIT: i32 = 12;
/// How wide − and + are.
const STEP: i32 = 28;

/// What each button of the bar over two or more picked out does, in its
/// order: lined up left, centers, right, top, middles, bottom; spaced
/// evenly across and down; made the size of the first picked.
pub(in crate::studio) const ARRANGE: [(Action, &str); 9] = [
    (Action::Align(0), "Align left"),
    (Action::Align(1), "Align centers"),
    (Action::Align(2), "Align right"),
    (Action::Align(3), "Align top"),
    (Action::Align(4), "Align middles"),
    (Action::Align(5), "Align bottom"),
    (Action::Distribute(0), "Space evenly across"),
    (Action::Distribute(1), "Space evenly down"),
    (Action::Match(2), "Match size"),
];

/// Where the bar's buttons are parted: after the three across, the three
/// down, and the spacing.
const ARRANGE_GROUPS: [usize; 3] = [3, 6, 8];

/// Which of S M L XL a line so wide with words so large is, at whatever
/// zoom it was drawn at: the one whose line is to its words as these are,
/// within a few in a hundred; none for another.
pub(in crate::studio) fn size_at(width: f32, px: f32) -> Option<u8> {
    let ratio = width / px;
    (0..SIZES.len()).find(|&at| {
        let (line, words) = SIZES[at];
        (ratio / (line / words as f32) - 1.0).abs() < 0.03
    }).map(|at| at as u8)
}

/// How much larger on the page a shape was drawn than at its own size, by
/// the zoom it was drawn at: its line against the line of its size, or of
/// the usual size where it is none of them.
pub(in crate::studio) fn drawn_larger(shape: &Shape) -> f32 {
    let line = size_at(shape.stroke, shape.font_px as f32).map_or(LINE, |at| SIZES[usize::from(at)].0);
    (shape.stroke / line).clamp(0.125, 8.0)
}

impl Studio {
    /// The shapes picked out the bar styles.
    fn bar_styled(&self) -> Vec<usize> {
        self.chosen().into_iter().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Shape(_))).collect()
    }

    /// The panel's place and its parts: docked at the right of the board,
    /// under Present, always in the one place, so that it covers nothing
    /// that is picked out and is where the hand expects it; shown with
    /// something picked out, being typed into, or a tool in hand for the
    /// next shape. Rows by what they set: the color, the fill, the line,
    /// the size; then the look, the font, the size of the words, and the
    /// ground, the status and guessing.
    pub(in crate::studio) fn layout_style_bar(&mut self, layout: &mut Layout) {
        let dragging = matches!(self.drag, Drag::Move { moved: true, .. } | Drag::Scale { .. } | Drag::Turn { .. } | Drag::Draw { .. } | Drag::Marquee { .. } | Drag::Pull { .. });
        // Kept while its own list of fonts is open from it, which says so.
        let menu = self.menu.is_some_and(|menu| !matches!(menu.kind, MenuKind::Fonts | MenuKind::Looks | MenuKind::Grounds));
        if self.zen || self.presenting() || dragging || self.search.is_some() || menu {
            return;
        }
        let styled = self.bar_styled();
        if styled.is_empty() && self.tool == Tool::Select && self.editing.is_none() {
            return;
        }
        let boxes = styled.iter().any(|&index| self.is_box(index)) || (styled.is_empty() && matches!(self.tool, Tool::Rect | Tool::Ellipse));
        let view = self.open_area();
        let inner = PANEL_W - 2 * PANEL_PAD;
        let x = view.right() - PANEL_W - 12;
        let top = match self.shows_right() {
            true => view.y + 12,
            false => self.side_button(1).bottom() + 14,
        };
        let left = x + PANEL_PAD;
        let mut y = top + PANEL_PAD;
        let heading = |layout: &mut Layout, y: &mut i32, says: &str| {
            layout.words.push(((left, *y), says.to_owned(), DIM_TEXT_COLOR, 11));
            *y += Text::line_height(11) + 6;
        };
        // A row of so many parts, each as wide, across the panel.
        let row = |layout: &mut Layout, y: &mut i32, ctls: &[Ctl], high: i32| {
            let count = ctls.len() as i32;
            let each = (inner - (count - 1) * GAP) / count;
            for (at, &ctl) in ctls.iter().enumerate() {
                let along = left + at as i32 * (each + GAP);
                let w = if at as i32 == count - 1 { left + inner - along } else { each };
                layout.controls.push((Rect::new(along, *y, w, high), Control::Board(ctl)));
            }
            *y += high;
        };
        heading(layout, &mut y, "Color");
        for line in 0..2u8 {
            row(layout, &mut y, &(0..5).map(|at| Ctl::Color(line * 5 + at)).collect::<Vec<_>>(), SWATCH);
            y += GAP + 2;
        }
        y += SPLIT - GAP - 2;
        if boxes {
            heading(layout, &mut y, "Fill");
            row(layout, &mut y, &[Ctl::FillKind(0), Ctl::FillKind(1), Ctl::FillKind(2), Ctl::FillKind(3)], ROW);
            y += SPLIT;
        }
        heading(layout, &mut y, "Line");
        row(layout, &mut y, &[Ctl::Dash(0), Ctl::Dash(1), Ctl::Dash(2)], ROW);
        y += SPLIT;
        // An arrow picked out: its heads, and the way it goes.
        let arrows = styled.iter().any(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if matches!(shape.tool, Tool::Line | Tool::Arrow))) || (styled.is_empty() && matches!(self.tool, Tool::Line | Tool::Arrow));
        if arrows {
            for (end, says) in [(0u8, "Start"), (1, "End")] {
                heading(layout, &mut y, says);
                row(layout, &mut y, &(0..HEAD_CHOICES.len() as u8).map(|at| Ctl::Head(end, at)).collect::<Vec<_>>(), ROW);
                y += SPLIT;
            }
            heading(layout, &mut y, "Route");
            row(layout, &mut y, &[Ctl::Route(0), Ctl::Route(1), Ctl::Route(2)], ROW);
            y += SPLIT;
        }
        heading(layout, &mut y, "Size");
        row(layout, &mut y, &[Ctl::Size(0), Ctl::Size(1), Ctl::Size(2), Ctl::Size(3)], ROW);
        y += SPLIT;
        layout.rules.push(Rect::new(x, y - SPLIT / 2, PANEL_W, 1));
        y += SPLIT / 2;
        row(layout, &mut y, &[Ctl::Font], ROW);
        y += GAP;
        // How large the words are: −, the number, and +.
        layout.controls.push((Rect::new(left, y, STEP, ROW), Control::Board(Ctl::Words(0))));
        layout.controls.push((Rect::new(left + STEP + GAP, y, inner - 2 * (STEP + GAP), ROW), Control::Board(Ctl::Words(1))));
        layout.controls.push((Rect::new(left + inner - STEP, y, STEP, ROW), Control::Board(Ctl::Words(2))));
        y += ROW;
        // What is the shape's own besides: its status, and a line drawn by
        // hand neatened. The board's theme, paper and guessing are the
        // board's, at its bottom right.
        let mut last = vec![];
        if boxes {
            last.push(Ctl::Status);
        }
        if self.tool == Tool::Pen {
            last.push(Ctl::Neaten);
        }
        if !last.is_empty() {
            y += GAP;
            row(layout, &mut y, &last, ROW);
        }
        y += PANEL_PAD;
        let panel = Rect::new(x, top, PANEL_W, y - top);
        layout.controls.push((panel, Control::Panel));
        layout.board_bar = Some(panel);
    }

    /// The bar over two or more picked out on a board, as Figma's over a
    /// selection: lined up, spaced, made one size. Above them, or below
    /// where there is no room above; not while they are moved, typed into,
    /// or a list is open.
    pub(in crate::studio) fn layout_arrange(&mut self, layout: &mut Layout) {
        const SIDE: i32 = 28;
        const PAD: i32 = 4;
        const PART: i32 = 9;
        if self.zen || self.presenting() || self.drag != Drag::None || self.menu.is_some() || self.editing.is_some() || self.board.history.is_some() {
            return;
        }
        let chosen = self.chosen();
        if chosen.len() < 2 {
            return;
        }
        let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((x, y, w, h)) = crate::studio::actions::union(&boxes) else { return };
        let (top_left, bottom_right) = (self.to_window_page((x, y)), self.to_window_page((x + w, y + h)));
        let wide = ARRANGE.len() as i32 * (SIDE + 2) - 2 + ARRANGE_GROUPS.len() as i32 * (PART - 2) + 2 * PAD;
        let high = SIDE + 2 * PAD;
        let open = self.open_area();
        let middle = (top_left.0 + bottom_right.0) / 2;
        let left = (middle - wide / 2).clamp(open.x + 8, (open.right() - wide - 8).max(open.x + 8));
        let above = top_left.1 - 14 - high;
        let top = if above >= open.y + 56 { above } else { (bottom_right.1 + 14).min(open.bottom() - high - 80) };
        let place = Rect::new(left, top, wide, high);
        let mut at = place.x + PAD;
        for (index, _) in ARRANGE.iter().enumerate() {
            if ARRANGE_GROUPS.contains(&index) {
                at += PART - 2;
            }
            layout.controls.push((Rect::new(at, place.y + PAD, SIDE, SIDE), Control::Board(Ctl::Arrange(index as u8))));
            at += SIDE + 2;
        }
        layout.controls.push((place, Control::Panel));
        layout.arrange_bar = Some(place);
    }

    /// A button of that bar drawn: its icon, the one for the same size two
    /// boxes alike; a rule before each part of it but the first.
    pub(in crate::studio) fn draw_arrange(&mut self, canvas: &mut Canvas, rect: Rect, at: u8, hovered: bool) {
        let (cx, cy) = rect.center();
        if ARRANGE_GROUPS.contains(&usize::from(at)) {
            canvas.fill(rect.x - 5, rect.y + 6, 1, rect.h - 12, HOVER_COLOR);
        }
        if hovered {
            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
        }
        let (action, _) = ARRANGE[usize::from(at).min(ARRANGE.len() - 1)];
        match action.icon() {
            Some(icon) => self.text.draw_centered(canvas, cx, cy, 14, TEXT_COLOR, icon),
            // Two boxes the same size.
            None => {
                canvas.round_outline(cx - 9, cy - 4, 8, 8, 2, 1, TEXT_COLOR);
                canvas.round_outline(cx + 1, cy - 4, 8, 8, 2, 1, TEXT_COLOR);
            }
        }
    }

    /// What the panel shows as set: of the first shape picked out, or of
    /// the next to be drawn: its line, its fill if it has one, whether it
    /// is drawn by hand, its status, whether its fill is hatched, how its
    /// line is broken, and how wide its line is.
    fn shown_style(&self) -> (u32, Option<u32>, bool, u8, bool, u8, f32) {
        let first = self.bar_styled().first().and_then(|&index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.clone()),
            Kind::Picture(_) => None,
        });
        match first {
            Some(shape) => (shape.color, shape.filled.then_some(shape.fill), shape.sketch, shape.status, shape.hatch, if shape.dotted { 2 } else { u8::from(shape.dashed) }, shape.stroke),
            None => (self.board.stroke, self.board.fill, self.board.sketch, 0, self.board.hatch, self.board.dash, self.board.width),
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
                    // A size set by hand is kept: the box grows to hold it.
                    shape.nominal = 0;
                    shape.touch();
                }
            }
            if self.on_board() {
                for &index in &sized {
                    self.grow_for_words(index);
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
        let (stroke, fill, _, status, hatch, dash, _) = self.shown_style();
        let (cx, cy) = rect.center();
        let ring = |canvas: &mut Canvas, on: bool| {
            if on {
                canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 8, 2, ACCENT);
            } else if hovered {
                canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 8, 1, DIM_TEXT_COLOR);
            }
        };
        // A part of a row of choices: lit when it is the one set.
        let choice = |canvas: &mut Canvas, on: bool| {
            if on {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, ROW_ON, 256);
            } else if hovered {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
            }
        };
        let ground = self.pages[self.page].ground;
        let line_high = Text::line_height(11);
        match ctl {
            // A color by name: its tint, with a disc of its line in it, as
            // the ground shows them.
            Ctl::Color(at) => {
                let (line, tint) = SWATCHES[usize::from(at)];
                // As the board draws them: FigJam's in the Diagram look.
                let diagram = self.pages[self.page].look == look::DIAGRAM;
                let seen = |color: u32| if diagram { look::figjam(color) } else { color };
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, look::shade(seen(tint), ground), 256);
                canvas.disc(cx, cy, 6, look::shade(seen(line), ground), 256);
                ring(canvas, stroke == line);
                if self.board.coloring {
                    let key = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"][usize::from(at)];
                    self.text.draw_centered(canvas, rect.right() - 6, rect.y + 7, 9, TEXT_COLOR, key);
                }
            }
            // What a fill is, each a small box as it is filled: none, the
            // paper, a tint, hatched.
            Ctl::FillKind(kind) => {
                let now = match (fill, hatch) {
                    (None, _) => 0,
                    (Some(0xFFFFFF), false) => 1,
                    (Some(_), true) => 3,
                    _ => 2,
                };
                choice(canvas, now == kind);
                // Its own tint where it has one, else that of its line.
                let pen = stroke;
                let tint = fill.filter(|&fill| fill != 0xFFFFFF).or_else(|| SWATCHES.iter().find(|(of, _)| *of == stroke).map(|(_, tint)| *tint)).unwrap_or(SWATCHES[7].1);
                let swatch = Rect::new(cx - 8, cy - 8, 16, 16);
                match kind {
                    0 => {}
                    1 => canvas.round_fill(swatch.x, swatch.y, swatch.w, swatch.h, 4, look::shade(0xFFFFFF, ground), 256),
                    _ => canvas.round_fill(swatch.x, swatch.y, swatch.w, swatch.h, 4, look::shade(tint, ground), 256),
                }
                if kind == 3 {
                    // Lines up to the right, across the swatch.
                    let pen = look::shade(pen, ground);
                    for py in swatch.y + 1..swatch.bottom() - 1 {
                        for px in swatch.x + 1..swatch.right() - 1 {
                            if (px - swatch.x + py - swatch.y) % 5 == 0 {
                                canvas.fill(px, py, 1, 1, pen);
                            }
                        }
                    }
                }
                canvas.round_outline(swatch.x, swatch.y, swatch.w, swatch.h, 4, 1, if kind == 0 { DIM_TEXT_COLOR } else { look::shade(pen, ground) });
                if kind == 0 {
                    line(canvas, (swatch.x + 3, swatch.bottom() - 4), (swatch.right() - 4, swatch.y + 3), 0xE03131);
                }
            }
            // A line whole, dashed or dotted, drawn so.
            Ctl::Dash(kind) => {
                choice(canvas, dash == kind);
                let (from, to) = (rect.x + 10, rect.right() - 10);
                let mut x = from;
                while x < to {
                    match kind {
                        0 => canvas.fill(x, cy - 1, (to - x).min(4), 2, TEXT_COLOR),
                        1 => canvas.fill(x, cy - 1, 6.min(to - x), 2, TEXT_COLOR),
                        _ => canvas.disc(x + 1, cy, 1, TEXT_COLOR, 256),
                    }
                    x += match kind {
                        0 => 4,
                        1 => 10,
                        _ => 5,
                    };
                }
            }
            // A head, drawn on the end of a short arrow, lit as the first
            // arrow picked out has it.
            Ctl::Head(end, at) => {
                let first = self.bar_styled().into_iter().find_map(|index| match &self.doc.layers[index].kind {
                    Kind::Shape(shape) if matches!(shape.tool, Tool::Line | Tool::Arrow) => Some(shape.clone()),
                    _ => None,
                });
                let now = first.map(|shape| if shape.heads[usize::from(end)] { Some(shape.ends[usize::from(end)]) } else { None });
                choice(canvas, now == Some(HEAD_CHOICES[usize::from(at)]));
                let (left, right) = ((rect.x + 6) as f32, (rect.right() - 6) as f32);
                let mut sample = Shape::new(Tool::Arrow, TEXT_COLOR, 1.5, if end == 0 { (right, cy as f32) } else { (left, cy as f32) });
                sample.points[1] = if end == 0 { (left, cy as f32) } else { (right, cy as f32) };
                sample.heads = [false, HEAD_CHOICES[usize::from(at)].is_some()];
                sample.ends = [shapes::FILLED_HEAD, HEAD_CHOICES[usize::from(at)].unwrap_or(shapes::FILLED_HEAD)];
                sample.draw(canvas, &mut self.text);
            }
            // The way it goes: curved, straight, round a corner.
            Ctl::Route(kind) => {
                let first = self.bar_styled().into_iter().find_map(|index| match &self.doc.layers[index].kind {
                    Kind::Shape(shape) if matches!(shape.tool, Tool::Line | Tool::Arrow) => Some(shape.clone()),
                    _ => None,
                });
                let now = first.map(|shape| if shape.elbow { 2 } else if shape.routed || shape.points.len() > 2 { 0 } else { 1 }).or_else(|| self.bar_styled().is_empty().then(|| self.board_route_kind()));
                choice(canvas, now == Some(kind));
                let (a, b) = (((rect.x + 8) as f32, (rect.bottom() - 7) as f32), ((rect.right() - 8) as f32, (rect.y + 7) as f32));
                let path: Vec<(f32, f32)> = match kind {
                    0 => (0..=12).map(|step| {
                        let t = step as f32 / 12.0;
                        let pull = (a.0, b.1);
                        let s = 1.0 - t;
                        (s * s * a.0 + 2.0 * s * t * pull.0 + t * t * b.0, s * s * a.1 + 2.0 * s * t * pull.1 + t * t * b.1)
                    }).collect(),
                    1 => vec![a, b],
                    _ => vec![a, (a.0, b.1), b],
                };
                board::halo(canvas, &path, 0.9, TEXT_COLOR, 256);
            }
            // S M L XL.
            Ctl::Size(at) => {
                // Lit when the words are that size, as large to the eye at the
                // zoom they are seen at as that size at 100%.
                let wanted = ((SIZES[usize::from(at)].1 as f32 * self.dynamic()).round() as u32).max(1);
                choice(canvas, self.shown_px() == Some(wanted));
                let says = ["S", "M", "L", "XL"][usize::from(at)];
                self.text.draw_centered(canvas, cx, cy, 12, TEXT_COLOR, says);
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
            // A color by name: a line in it, and a fill that is a tint the
            // tint of it; a note, which keeps its quiet edge, its tint.
            Ctl::Color(at) => {
                let (line, tint) = SWATCHES[usize::from(at)];
                self.board.stroke = line;
                if self.board.fill.is_some_and(|fill| fill != 0xFFFFFF) {
                    self.board.fill = Some(tint);
                }
                self.restyle_shapes(|shape| {
                    let tinted = shape.filled && shape.fill != 0xFFFFFF && matches!(shape.tool, Tool::Rect | Tool::Ellipse);
                    if shape.form == shapes::FORM_STICKY && shape.tool == Tool::Rect {
                        shape.fill = tint;
                        return;
                    }
                    shape.color = line;
                    if tinted {
                        shape.fill = tint;
                    }
                });
                self.say(format!("Color: {}", SWATCH_NAMES[usize::from(at)]));
            }
            // What a fill is: none, the paper, a tint of its line, or that
            // tint hatched in its line.
            Ctl::FillKind(kind) => {
                let tint_of = |line: u32, now: Option<u32>| SWATCHES.iter().find(|(of, _)| *of == line).map(|(_, tint)| *tint).or(now.filter(|&fill| fill != 0xFFFFFF)).unwrap_or(SWATCHES[7].1);
                self.board.fill = match kind {
                    0 => None,
                    1 => Some(0xFFFFFF),
                    _ => Some(tint_of(self.board.stroke, self.board.fill)),
                };
                self.board.hatch = kind == 3;
                self.restyle_shapes(|shape| {
                    if !matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        return;
                    }
                    let tint = tint_of(shape.color, shape.filled.then_some(shape.fill));
                    match kind {
                        0 => (shape.filled, shape.border, shape.hatch) = (false, true, false),
                        1 => (shape.filled, shape.fill, shape.hatch) = (true, 0xFFFFFF, false),
                        _ => (shape.filled, shape.fill, shape.hatch) = (true, tint, kind == 3),
                    }
                });
                self.say(["No fill", "Filled with the paper", "Filled with a tint", "Hatched"][usize::from(kind)]);
            }
            // A head at an end of the arrows picked out, or none.
            Ctl::Head(end, at) => {
                let choice = HEAD_CHOICES[usize::from(at)];
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Line | Tool::Arrow) {
                        shape.heads[usize::from(end)] = choice.is_some();
                        if let Some(kind) = choice {
                            shape.ends[usize::from(end)] = kind;
                        }
                        shape.touch();
                    }
                });
                self.say(match choice {
                    None => "No head".to_owned(),
                    Some(kind) => format!("{} head", shapes::HEADS[usize::from(kind)]),
                });
            }
            // The way the arrows picked out go: curved between what they
            // join, as a board routes them; straight; or round a corner.
            Ctl::Route(kind) => {
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Line | Tool::Arrow) {
                        shape.points.truncate(2);
                        (shape.routed, shape.elbow) = (kind == 0, kind == 2);
                        shape.touch();
                    }
                });
                self.follow();
                // And the next arrows so, as the panel's other choices carry on.
                self.board.route = Some(kind);
                self.say(["Curved", "Straight", "Round a corner"][usize::from(kind)]);
            }
            // A line whole, dashed or dotted.
            Ctl::Dash(kind) => {
                self.board.dash = kind;
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Arrow | Tool::Line) {
                        (shape.dashed, shape.dotted) = (kind == 1, kind == 2);
                    }
                });
                self.say(["A whole line", "A dashed line", "A dotted line"][usize::from(kind)]);
            }
            // S M L XL: how wide the line is and how large the words, the
            // two together.
            Ctl::Size(at) => {
                // The size of the words, as large to the eye as that size at
                // 100% at the zoom it is chosen at; the line left as it is,
                // its border no thicker nor thinner.
                let by = self.dynamic();
                let px = ((SIZES[usize::from(at)].1 as f32 * by).round() as u32).max(1);
                self.restyle_shapes(|shape| {
                    // The size its words are meant to be, fitted to its box.
                    (shape.font_px, shape.nominal) = (px, if shape.nominal > 0 || matches!(shape.tool, Tool::Rect | Tool::Ellipse) { px } else { 0 });
                });
                if self.on_board() {
                    self.pages[self.page].px = px;
                }
                for index in self.bar_styled() {
                    self.grow_for_words(index);
                }
                self.say(format!("Size: {}", ["S", "M", "L", "XL"][usize::from(at)]));
            }
            // Each a list, under the bar, or over it where it is low; a
            // second click on it closes it.
            Ctl::Font => self.open_bar_menu(ctl, MenuKind::Fonts),
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
                let (_, _, _, status, ..) = self.shown_style();
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

    #[test]
    fn drawn_zoomed_out_a_box_and_its_arrow_are_as_large_to_the_eye_as_at_100() {
        let mut studio = board();
        studio.zoom = 0.25;
        studio.changed();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        let (a, b) = (studio.to_doc((600, 400)), studio.to_doc((700, 450)));
        pull(&mut studio, a, b, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let box_ = shape(&studio, 0);
        assert_eq!((box_.stroke, box_.font_px), (LINE * 4.0, WORDS_PX * 4));
        // Still M, at that zoom, in the panel's sense.
        assert_eq!(size_at(box_.stroke, box_.font_px as f32), Some(1));
        // An arrow joining it to another is as heavy as it.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        let (a, b) = (studio.to_doc((900, 400)), studio.to_doc((1000, 450)));
        pull(&mut studio, a, b, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let arrow = studio.join(0, 1);
        assert_eq!(shape(&studio, arrow).stroke, LINE * 4.0);
        // Back at 100%, a new box is drawn as before.
        studio.zoom = 1.0;
        studio.changed();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 600.0), (260.0, 672.0), Mods::default());
        assert_eq!(shape(&studio, studio.doc.layers.len() - 1).stroke, LINE);
    }

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
