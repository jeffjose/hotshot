//! The colors, beside the panel at the color they are for: a square of a
//! hue from pale to dark, the hues along a strip under it, how opaque, the
//! color as hex to type, a dropper to take one off the page, the palette,
//! and the colors already in the file.

use super::*;

/// The parts of the picker that are dragged or pressed.
pub(super) const SQUARE: u8 = 0;
pub(super) const HUE: u8 = 1;
pub(super) const ALPHA: u8 = 2;
pub(super) const DROPPER: u8 = 3;

const WIDE: i32 = 252;

/// A color as hue (0 to 360), saturation and value (each 0 to 1).
pub(super) fn to_hsv(color: u32) -> (f32, f32, f32) {
    let (r, g, b) = ((color >> 16 & 0xFF) as f32 / 255.0, (color >> 8 & 0xFF) as f32 / 255.0, (color & 0xFF) as f32 / 255.0);
    let (high, low) = (r.max(g).max(b), r.min(g).min(b));
    let spread = high - low;
    let hue = if spread == 0.0 {
        0.0
    } else if high == r {
        60.0 * ((g - b) / spread).rem_euclid(6.0)
    } else if high == g {
        60.0 * ((b - r) / spread + 2.0)
    } else {
        60.0 * ((r - g) / spread + 4.0)
    };
    (hue, if high == 0.0 { 0.0 } else { spread / high }, high)
}

/// A color from its hue, saturation and value.
pub(super) fn from_hsv((hue, saturation, value): (f32, f32, f32)) -> u32 {
    let chroma = value * saturation;
    let part = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match (hue.rem_euclid(360.0) / 60.0) as u32 {
        0 => (chroma, part, 0.0),
        1 => (part, chroma, 0.0),
        2 => (0.0, chroma, part),
        3 => (0.0, part, chroma),
        4 => (part, 0.0, chroma),
        _ => (chroma, 0.0, part),
    };
    let low = value - chroma;
    let byte = |channel: f32| (((channel + low) * 255.0).round() as u32).min(255);
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

impl Studio {
    /// The color a paint is now, if it is one.
    pub(super) fn paint_color(&self, paint: Paint) -> Option<u32> {
        let shown = self.picked().map(|index| &self.doc.layers[index].kind);
        match (paint, shown, self.doc.background) {
            (Paint::Background, _, Fill::Color(color)) => Some(color),
            (Paint::From, _, Fill::Gradient(from, ..)) => Some(from),
            (Paint::To, _, Fill::Gradient(_, to, _)) => Some(to),
            (Paint::Line, Some(Kind::Shape(shape)), _) => Some(shape.color),
            (Paint::Line, None, _) => Some(self.color),
            (Paint::Fill, Some(Kind::Shape(shape)), _) => shape.filled.then_some(shape.fill),
            (Paint::Shadow, Some(_), _) => self.picked().and_then(|index| self.doc.layers[index].drop()).map(|drop| drop.color & 0xFF_FFFF),
            (Paint::Recolor(color), _, _) => Some(color),
            _ => None,
        }
    }

    /// How opaque what a paint colors is, out of 100, where that can be set
    /// with the color: a shape, and a picture's shadow.
    fn paint_alpha(&self, paint: Paint) -> Option<f32> {
        match (paint, self.picked().map(|index| &self.doc.layers[index].kind)) {
            (Paint::Line | Paint::Fill, Some(Kind::Shape(shape))) => Some(shape.opacity as f32 * 100.0 / 256.0),
            (Paint::Shadow, Some(_)) => self.field_value(Field::DropAlpha),
            _ => None,
        }
    }

    fn field_value(&self, field: Field) -> Option<f32> {
        let index = self.picked()?;
        match field {
            Field::DropAlpha => self.doc.layers[index].drop().map(|drop| (drop.color >> 24) as f32 * 100.0 / 255.0),
            _ => None,
        }
    }

    /// The picker opened for a paint: at the color it is now.
    pub(super) fn open_picker(&mut self, paint: Paint) {
        self.palette = Some(paint);
        self.hsv = to_hsv(self.paint_color(paint).unwrap_or(0xFFFFFF));
        self.eyedropping = false;
    }

    /// The colors already used in the file: backgrounds, shapes and
    /// shadows, each once, the first found first.
    fn colors_in_file(&self) -> Vec<u32> {
        let mut found: Vec<u32> = Vec::new();
        let mut add = |color: u32| {
            if !found.contains(&color) {
                found.push(color);
            }
        };
        for page in &self.pages {
            for index in 0..page.frames.len() {
                let frame = if std::ptr::eq(page, &self.pages[self.page]) && index == self.active { &self.doc } else { &page.frames[index] };
                match frame.background {
                    Fill::Color(color) => add(color),
                    Fill::Gradient(from, to, _) => {
                        add(from);
                        add(to);
                    }
                    _ => {}
                }
                for layer in &frame.layers {
                    if let Kind::Shape(shape) = &layer.kind {
                        add(shape.color);
                        if shape.filled {
                            add(shape.fill);
                        }
                    }
                }
            }
        }
        found.truncate(18);
        found
    }

    /// The picker laid out beside the panel, level with its field: its
    /// parts first among the controls, as it is over what is under it.
    pub(super) fn layout_picker(&mut self, layout: &mut Layout, panel: Rect, field: Rect, paint: Paint) {
        let square = 150;
        let swatch = 22;
        let palette: Vec<Option<u32>> = (paint == Paint::Fill).then_some(None).into_iter().chain(PALETTE.iter().map(|&(color, _)| Some(color))).collect();
        let used = self.colors_in_file();
        let across = 9;
        let rows = |count: usize| (count as i32 + across - 1) / across;
        let alpha = self.paint_alpha(paint).is_some();
        let high = 10 + square + 12 + 12 + if alpha { 20 } else { 0 } + 12 + 26 + 14 + rows(palette.len()) * (swatch + 4) + if used.is_empty() { 0 } else { 22 + rows(used.len()) * (swatch + 4) } + 8;
        let place = Rect::new(panel.x - 8 - WIDE, (field.y - 8).clamp(INSET, self.height - INSET - high), WIDE, high);
        let (left, inner) = (place.x + 10, WIDE - 20);
        let mut front = Vec::new();
        let mut y = place.y + 10;
        front.push((Rect::new(left, y, inner, square), Control::Picker(SQUARE)));
        y += square + 12;
        front.push((Rect::new(left + 34, y - 4, inner - 34, 20), Control::Picker(HUE)));
        front.push((Rect::new(left, y - 6, 26, 26), Control::Picker(DROPPER)));
        y += 22;
        if alpha {
            front.push((Rect::new(left + 34, y - 4, inner - 34, 20), Control::Picker(ALPHA)));
            y += 20;
        }
        y += 6;
        front.push((Rect::new(left, y, inner, panel::FIELD_H), Control::Entry(Naming::Hex)));
        y += panel::FIELD_H + 14;
        for (at, &color) in palette.iter().enumerate() {
            front.push((Rect::new(left + at as i32 % across * (swatch + 4), y + at as i32 / across * (swatch + 4), swatch, swatch), Control::Swatch(paint, color)));
        }
        y += rows(palette.len()) * (swatch + 4);
        if !used.is_empty() {
            layout.words.push(((left, y + 4), "IN THIS FILE".to_owned(), 0x8F8F8F, 10));
            y += 22;
            for (at, &color) in used.iter().enumerate() {
                front.push((Rect::new(left + at as i32 % across * (swatch + 4), y + at as i32 / across * (swatch + 4), swatch, swatch), Control::Swatch(paint, Some(color))));
            }
        }
        front.push((place, Control::Panel));
        front.append(&mut layout.controls);
        layout.controls = front;
        layout.popover = Some(place);
    }

    /// The parts of the picker that are not swatches, drawn.
    pub(super) fn draw_picker(&mut self, canvas: &mut Canvas, rect: Rect, part: u8, hovered: bool) {
        let (hue, saturation, value) = self.hsv;
        match part {
            SQUARE => {
                for y in 0..rect.h {
                    for x in 0..rect.w {
                        let color = from_hsv((hue, x as f32 / (rect.w - 1) as f32, 1.0 - y as f32 / (rect.h - 1) as f32));
                        canvas.fill(rect.x + x, rect.y + y, 1, 1, color);
                    }
                }
                let (x, y) = (rect.x + (saturation * (rect.w - 1) as f32).round() as i32, rect.y + ((1.0 - value) * (rect.h - 1) as f32).round() as i32);
                canvas.round_outline(x - 7, y - 7, 14, 14, 7, 2, 0xFFFFFF);
                canvas.round_outline(x - 8, y - 8, 16, 16, 8, 1, 0x000000);
            }
            HUE | ALPHA => {
                let (track, cy) = (Rect::new(rect.x, rect.y + rect.h / 2 - 5, rect.w, 10), rect.y + rect.h / 2);
                let color = from_hsv(self.hsv);
                for x in 0..track.w {
                    let along = x as f32 / (track.w - 1) as f32;
                    let shade = match part {
                        HUE => from_hsv((along * 360.0, 1.0, 1.0)),
                        _ => {
                            let checker = if (x / 4 + 0) % 2 == 0 { 0x555555 } else { 0x333333 };
                            canvas::mix(checker, color, (along * 256.0) as u32)
                        }
                    };
                    canvas.fill(track.x + x, track.y, 1, track.h, shade);
                }
                let along = match part {
                    HUE => hue / 360.0,
                    _ => self.paint_alpha(self.palette.unwrap_or(Paint::Line)).unwrap_or(100.0) / 100.0,
                };
                let x = track.x + (along * (track.w - 1) as f32).round() as i32;
                canvas.disc(x, cy, 8, 0xFFFFFF, 256);
                canvas.disc(x, cy, 6, if part == HUE { from_hsv((hue, 1.0, 1.0)) } else { color }, 256);
            }
            _ => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 5, if self.eyedropping { ACCENT } else if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                let (cx, cy) = rect.center();
                self.text.draw_centered(canvas, cx, cy, 14, TEXT_COLOR, icons::PIPETTE);
            }
        }
    }

    /// The picker pressed at a part of it, or dragged along one.
    pub(super) fn pick_at(&mut self, part: u8, rect: Rect, at: (i32, i32), first: bool) {
        let Some(paint) = self.palette else { return };
        let along = ((at.0 - rect.x) as f32 / (rect.w - 1).max(1) as f32).clamp(0.0, 1.0);
        let down = ((at.1 - rect.y) as f32 / (rect.h - 1).max(1) as f32).clamp(0.0, 1.0);
        match part {
            SQUARE => (self.hsv.1, self.hsv.2) = (along, 1.0 - down),
            HUE => self.hsv.0 = along * 359.9,
            ALPHA => {
                match paint {
                    Paint::Shadow => self.set_field(Field::DropAlpha, along * 100.0),
                    _ => self.set_field(Field::Opacity, along * 100.0),
                }
                if !first {
                    // One step to undo for the whole drag.
                }
                return;
            }
            _ => return,
        }
        let color = from_hsv(self.hsv);
        let before = self.undo.len();
        self.paint(paint, Some(color));
        if !first && self.undo.len() > before {
            self.undo.pop();
        }
    }

    /// A color taken off the page where the pointer is, as it is shown.
    pub(super) fn take_color(&mut self, at: (i32, i32)) -> Option<u32> {
        let index = self.frame_at(self.to_page(at))?;
        let origin = {
            let frame = self.frame(index);
            self.to_window_page((frame.x, frame.y))
        };
        let (_, _, part, pixels) = self.shown.get(index)?.as_ref()?;
        let (x, y) = (at.0 - origin.0 - part.x, at.1 - origin.1 - part.y);
        if x < 0 || y < 0 || x >= part.w || y >= part.h {
            return None;
        }
        Some(pixels[(y * part.w + x) as usize] & 0xFF_FFFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_color_goes_to_hue_saturation_and_value_and_back() {
        for color in [0xFFFFFF, 0x000000, 0xE5484D, 0x3E63DD, 0x46A758, 0xFFC53D, 0x8E4EC6, 0x777777] {
            assert_eq!(from_hsv(to_hsv(color)), color, "{color:06X}");
        }
        let (hue, saturation, value) = to_hsv(0xFF0000);
        assert!((hue, saturation, value) == (0.0, 1.0, 1.0));
        assert!((to_hsv(0x0000FF).0 - 240.0).abs() < 0.01);
    }
}
