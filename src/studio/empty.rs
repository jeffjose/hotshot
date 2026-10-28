//! A frame with nothing in it says how to put something in it: a place
//! for a picture with buttons to capture the screen, paste, or go to the
//! captures; looks to start from; and the keys for what else there is.

use super::*;

/// How large the place for a picture is, in the window.
const BOX: (i32, i32) = (400, 168);

/// The looks a frame can start from: what each is called, and its
/// background.
pub(super) const LOOKS: [(&str, Fill); 3] = [("Launch", Fill::Gradient(0x3E63DD, 0xD6409F, 135.0)), ("Docs", Fill::Color(0xF4F4F5)), ("Dark", Fill::Color(0x18181B))];

/// The keys said under an empty frame, and what each does.
const HINTS: [(&str, &str); 4] = [("F", "another frame"), ("A", "arrow"), ("T", "text"), ("Ctrl K", "anything else")];

impl Studio {
    /// Where the place for a picture is, if the frame worked in is empty
    /// and shown large enough to hold it.
    pub(super) fn empty_place(&self) -> Option<Rect> {
        if !self.doc.layers.is_empty() || self.cropping.is_some() {
            return None;
        }
        let (a, b) = (self.to_window((0.0, 0.0)), self.to_window((self.doc.width as f32, self.doc.height as f32)));
        let frame = Rect::new(a.0, a.1, b.0 - a.0, b.1 - a.1);
        if frame.w < BOX.0 + 40 || frame.h < BOX.1 + 70 {
            return None;
        }
        let (cx, cy) = frame.center();
        Some(Rect::new(cx - BOX.0 / 2, cy - BOX.1 / 2 - 16, BOX.0, BOX.1))
    }

    /// The buttons of an empty frame.
    pub(super) fn layout_empty(&mut self, layout: &mut Layout) {
        let Some(place) = self.empty_place() else { return };
        let says = ["Capture the screen", "Paste \u{b7} Ctrl V", "From captures"];
        let wides: Vec<i32> = says.iter().enumerate().map(|(at, text)| self.text.width(text, 12) + if at == 0 { 40 } else { 22 }).collect();
        let total: i32 = wides.iter().sum::<i32>() + 8 * 2;
        let mut x = place.x + (place.w - total) / 2;
        for (at, wide) in wides.into_iter().enumerate() {
            layout.controls.push((Rect::new(x, place.y + 112, wide, 28), Control::Start(at as u8)));
            x += wide + 8;
        }
        // The looks, under the place, after what they are.
        let lead = self.text.width("or start from a look:", 12) + 10;
        let looks: Vec<i32> = LOOKS.iter().map(|(name, _)| 30 + self.text.width(name, 12) + 14).collect();
        let total = lead + looks.iter().sum::<i32>();
        let mut x = place.x + (place.w - total) / 2 + lead;
        for (at, wide) in looks.into_iter().enumerate() {
            layout.controls.push((Rect::new(x, place.bottom() + 16, wide, 22), Control::Start(3 + at as u8)));
            x += wide;
        }
        layout.empty = Some(place);
    }

    /// What a button of an empty frame does.
    pub(super) fn press_start(&mut self, which: u8) -> Outcome {
        match which {
            0 => Outcome::Capture,
            1 => Outcome::Paste,
            2 => Outcome::Home,
            look => {
                if let Some((_, fill)) = LOOKS.get(usize::from(look - 3)) {
                    self.checkpoint();
                    self.doc.background = *fill;
                    self.changed();
                }
                Outcome::Continue
            }
        }
    }

    /// The place for a picture: a dashed edge, an icon, what to do, and
    /// under the frame the keys.
    pub(super) fn draw_empty(&mut self, canvas: &mut Canvas, place: Rect) {
        let dark = self.empty_is_dark();
        let (ink, faint, edge) = if dark { (0xF4F4F5, 0xC8C8CE, 0xFFFFFF) } else { (0x1A1A1A, 0x6B6B70, 0xB8B8BE) };
        canvas.round_fill(place.x, place.y, place.w, place.h, 14, if dark { 0x000000 } else { 0xFFFFFF }, if dark { 40 } else { 120 });
        // The edge in dashes, along each side.
        let dash = |canvas: &mut Canvas, x: i32, y: i32, across: bool, length: i32| {
            for at in (0..length).step_by(10) {
                let run = 6.min(length - at);
                if across {
                    canvas.round_fill(x + at, y, run, 1, 0, edge, 180);
                } else {
                    canvas.round_fill(x, y + at, 1, run, 0, edge, 180);
                }
            }
        };
        dash(canvas, place.x + 12, place.y, true, place.w - 24);
        dash(canvas, place.x + 12, place.bottom() - 1, true, place.w - 24);
        dash(canvas, place.x, place.y + 12, false, place.h - 24);
        dash(canvas, place.right() - 1, place.y + 12, false, place.h - 24);
        let cx = place.x + place.w / 2;
        canvas.round_fill(cx - 16, place.y + 22, 32, 32, 8, ACCENT, 50);
        self.text.draw_centered(canvas, cx, place.y + 38, 17, ACCENT, icons::PICTURE);
        let title = "Put a picture in this frame";
        let wide = self.text.width(title, 15);
        self.text.look.heavy = 1;
        self.text.draw(canvas, cx - wide / 2, place.y + 64, 15, ink, title);
        self.text.look.heavy = 0;
        let line = "Paste one, or take one now";
        let wide = self.text.width(line, 12);
        self.text.draw(canvas, cx - wide / 2, place.y + 88, 12, faint, line);
        let lead = "or start from a look:";
        let looks_wide: i32 = self.text.width(lead, 12) + 10 + LOOKS.iter().map(|(name, _)| 44 + self.text.width(name, 12)).sum::<i32>();
        self.text.draw(canvas, place.x + (place.w - looks_wide) / 2, place.bottom() + 16 + (22 - Text::line_height(12)) / 2, 12, faint, lead);
        // The keys, under the frame.
        let bottom = self.to_window((0.0, self.doc.height as f32)).1 + 34;
        let widths: Vec<i32> = HINTS.iter().map(|(key, what)| self.text.width(key, 11) + 12 + 6 + self.text.width(what, 11) + 18).collect();
        let mut x = cx - widths.iter().sum::<i32>() / 2;
        for ((key, what), wide) in HINTS.iter().zip(widths) {
            let key_wide = self.text.width(key, 11) + 12;
            canvas.round_outline(x, bottom, key_wide, 18, 4, 1, 0x555555);
            self.text.draw(canvas, x + 6, bottom + (18 - Text::line_height(11)) / 2, 11, TEXT_COLOR, key);
            self.text.draw(canvas, x + key_wide + 6, bottom + (18 - Text::line_height(11)) / 2, 11, DIM_TEXT_COLOR, what);
            x += wide;
        }
    }

    /// A button of an empty frame.
    pub(super) fn draw_start(&mut self, canvas: &mut Canvas, rect: Rect, which: u8, hovered: bool) {
        let middle = rect.y + (rect.h - Text::line_height(12)) / 2;
        match which {
            0 => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { 0x5472E4 } else { ACCENT }, 256);
                self.text.draw_centered(canvas, rect.x + 17, rect.y + rect.h / 2, 13, 0xFFFFFF, icons::CAMERA);
                self.text.draw(canvas, rect.x + 30, middle, 12, 0xFFFFFF, "Capture the screen");
            }
            1 | 2 => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { 0xE4E4E7 } else { 0xF0F0F2 }, 256);
                self.text.draw(canvas, rect.x + 11, middle, 12, 0x1A1A1A, if which == 1 { "Paste \u{b7} Ctrl V" } else { "From captures" });
            }
            look => {
                let Some((name, fill)) = LOOKS.get(usize::from(look - 3)) else { return };
                let swatch = Rect::new(rect.x, rect.y + 2, 24, 18);
                match *fill {
                    Fill::Gradient(from, to, _) => {
                        for x in 0..swatch.w {
                            canvas.fill(swatch.x + x, swatch.y, 1, swatch.h, crate::canvas::mix(from, to, (x * 256 / swatch.w) as u32));
                        }
                    }
                    Fill::Color(color) => canvas.round_fill(swatch.x, swatch.y, swatch.w, swatch.h, 4, color, 256),
                    _ => {}
                }
                canvas.round_outline(swatch.x, swatch.y, swatch.w, swatch.h, 4, 1, if hovered { ACCENT } else { 0x888888 });
                let ink = if self.empty_is_dark() { 0xF4F4F5 } else { 0x3A3A3F };
                self.text.draw(canvas, rect.x + 30, middle, 12, ink, name);
            }
        }
    }
}

impl Studio {
    /// Whether an empty frame's background is dark, for what is said on it.
    fn empty_is_dark(&self) -> bool {
        match self.doc.background {
            Fill::Color(color) => (color >> 16 & 255) + (color >> 8 & 255) + (color & 255) < 384,
            Fill::None => false,
            _ => true,
        }
    }
}

impl Studio {
    /// Over the picture being cropped, the shapes the crop can keep to and
    /// a reset, floating.
    pub(super) fn layout_crop_bar(&mut self, layout: &mut Layout) {
        let Some(Layer { kind: Kind::Picture(picture), .. }) = self.cropping.and_then(|index| self.doc.layers.get(index)).cloned() else { return };
        let top = picture.corners().map(|corner| self.to_window(corner));
        let (left, right, high) = (top.iter().map(|c| c.0).min().unwrap_or(0), top.iter().map(|c| c.0).max().unwrap_or(0), top.iter().map(|c| c.1).min().unwrap_or(0));
        let mut items: Vec<(String, Control)> = panel::CROP_ASPECTS.iter().enumerate().map(|(at, (name, _))| (name.to_string(), Control::Action(Action::Aspect(at as u8)))).collect();
        items.push(("Reset".into(), Control::Action(Action::ResetCrop)));
        let wides: Vec<i32> = items.iter().map(|(says, _)| self.text.width(says, 12) + 16).collect();
        let wide = wides.iter().sum::<i32>() + 4 * (wides.len() as i32 - 1) + 12;
        let room = self.open_area();
        let x = ((left + right) / 2 - wide / 2).clamp(room.x + 4, (room.right() - wide - 4).max(room.x));
        let y = (high - 52).max(room.y + 4);
        let bar = Rect::new(x, y, wide, 34);
        let mut at = x + 6;
        let line = Text::line_height(12);
        for ((says, control), w) in items.into_iter().zip(wides) {
            layout.controls.insert(0, (Rect::new(at, y + 5, w, 24), control));
            layout.words.push(((at + 8, y + 5 + (24 - line) / 2), says, TEXT_COLOR, 12));
            at += w + 4;
        }
        layout.controls.push((bar, Control::Panel));
        layout.crop_bar = Some(bar);
    }
}
