//! A board's bar of styles, floating by what is picked out rather than in
//! a panel: five line colors and five fills as Excalidraw keeps them, no
//! fill, by hand or not, how large the words are, a status dot, and
//! guessing on or off. With nothing picked out and a tool in hand, it sits
//! over the tools, for the next shape; and what it last set carries on to
//! the next shape drawn.

use super::*;

/// How large the words can be: small, medium, large and huge.
pub(in crate::studio) const WORD_SIZES: [u32; 4] = [16, 20, 28, 36];

/// How large a swatch of the bar is, the room round them, and the gap
/// between groups.
const SWATCH: i32 = 22;
const GAP: i32 = 4;
const PAD: i32 = 6;
const SPLIT: i32 = 11;

impl Studio {
    /// The shapes picked out the bar styles.
    fn bar_styled(&self) -> Vec<usize> {
        self.chosen().into_iter().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Shape(_))).collect()
    }

    /// The bar's place and its parts, over what is picked out, or under it
    /// near the top of the window; or over the tools for the next shape.
    pub(in crate::studio) fn layout_style_bar(&mut self, layout: &mut Layout) {
        let dragging = matches!(self.drag, Drag::Move { moved: true, .. } | Drag::Scale { .. } | Drag::Turn { .. } | Drag::Draw { .. } | Drag::Marquee { .. } | Drag::Pull { .. });
        if self.zen || self.presenting() || dragging || self.search.is_some() || self.menu.is_some() {
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
        parts.push((Ctl::Ground, SWATCH));
        parts.push((Ctl::Words(0), 40));
        if boxes {
            parts.push((Ctl::Status, SWATCH + 4));
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
                let top = if above - 14 - high >= view.y + 40 { above - 14 - high } else { below + 30 };
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
    fn shown_style(&self) -> (u32, Option<u32>, bool, u32, u8) {
        let first = self.bar_styled().first().and_then(|&index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.clone()),
            Kind::Picture(_) => None,
        });
        match first {
            Some(shape) => (shape.color, shape.filled.then_some(shape.fill), shape.sketch, shape.font_px, shape.status),
            None => (self.board.stroke, self.board.fill, self.board.sketch, self.board.px, 0),
        }
    }

    pub(in crate::studio) fn draw_style_control(&mut self, canvas: &mut Canvas, rect: Rect, ctl: Ctl, hovered: bool) {
        let (stroke, fill, sketch, px, status) = self.shown_style();
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
            Ctl::Ground => {
                let (paper, dots) = self.paper();
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 11, paper, 256);
                canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 11, 1, if hovered { TEXT_COLOR } else { dots });
            }
            Ctl::NoFill => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, 0xFFFFFF, 256);
                line(canvas, (rect.x + 4, rect.bottom() - 5), (rect.right() - 5, rect.y + 4), 0xE03131);
                line(canvas, (rect.x + 5, rect.bottom() - 5), (rect.right() - 5, rect.y + 5), 0xE03131);
                ring(canvas, fill.is_none());
            }
            Ctl::Look => {
                if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                // How the board draws: rounded, sketchy or clean.
                let says = look::LOOKS[usize::from(self.pages[self.page].look.min(2))];
                let _ = sketch;
                self.text.draw_in(LETTERING, canvas, rect.x + 7, cy - Text::line_height(13) / 2, 13, TEXT_COLOR, says);
                self.text.draw_centered(canvas, rect.right() - 8, cy, 9, DIM_TEXT_COLOR, icons::DOWN);
            }
            Ctl::Words(_) => {
                if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                let which = WORD_SIZES.iter().position(|&size| size >= px).unwrap_or(WORD_SIZES.len() - 1);
                self.text.draw(canvas, rect.x + 6, cy - line_high / 2 - 1, 13, TEXT_COLOR, "Aa");
                self.text.draw(canvas, rect.x + 26, cy - line_high / 2 + 1, 10, DIM_TEXT_COLOR, ["S", "M", "L", "XL"][which]);
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
            Ctl::Look => {
                let next = (self.pages[self.page].look + 1) % 3;
                self.set_look(Some(next), None);
            }
            Ctl::Ground => {
                let next = (self.pages[self.page].ground + 1) % 3;
                self.set_look(None, Some(next));
            }
            Ctl::Words(_) => {
                let (_, _, _, px, _) = self.shown_style();
                let next = WORD_SIZES[(WORD_SIZES.iter().position(|&size| size >= px).unwrap_or(0) + 1) % WORD_SIZES.len()];
                self.board.px = next;
                self.restyle_shapes(|shape| shape.font_px = next);
            }
            Ctl::Status => {
                let (_, _, _, _, status) = self.shown_style();
                let next = (status + 1) % 4;
                self.restyle_shapes(|shape| {
                    if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                        shape.status = next;
                    }
                });
                self.say(["No status", "Status: red", "Status: amber", "Status: green"][usize::from(next)]);
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

    /// Each shape picked out changed, as one step to undo.
    pub(in crate::studio) fn restyle_shapes(&mut self, change: impl Fn(&mut Shape)) {
        self.restyle(|layer| {
            if let Kind::Shape(shape) = &mut layer.kind {
                change(shape);
            }
        });
    }
}
