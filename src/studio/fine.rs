//! Fine-tune on the canvas: jog's Convert panel floated by an export row,
//! and the frame itself as the comparison. While it is open the frame is
//! split by a line that follows the pointer, the original to its left
//! and, to its right, the converted picture decoded again, both at the
//! size the row exports at and drawn at the canvas's zoom; Z holds a
//! loupe at the pointer. Esc or a click off the panel puts the frame
//! back.

use std::time::Duration;

use super::*;
use crate::tune::{Asked, Source, Tuner, Words};

/// How many times larger the loupe shows what is under it.
const LOUPE: i32 = 8;

pub(super) struct Fine {
    pub tuner: Tuner,
    /// The row it is for, of which frame of which page, at what size.
    row: usize,
    page: usize,
    frame: usize,
    /// Where the line across the frame is, in the window; and whether
    /// the loupe is held.
    divider: Option<i32>,
    loupe: bool,
    /// The two sides made smaller, for a frame shown smaller than it is.
    shrunk: crate::tune::Shrunk,
}

impl Studio {
    /// Fine-tune opened for an export row of the frame worked in: the
    /// frame drawn at the row's size, and tried as the row writes it.
    pub(super) fn open_fine(&mut self, row: usize) {
        let Some(size) = self.doc.exports.get(row).cloned() else { return };
        let (wide, high, pixels) = self.doc.render(size.times, &mut self.text);
        let suffix = size.file_suffix();
        let title = match suffix.is_empty() {
            true => format!("Export {}\u{d7}", size.times),
            false => format!("Export {}\u{d7} \u{b7} {suffix}", size.times),
        };
        let words = Words { title, subtitle: self.doc.name.clone(), primary: "Use for this row", secondary: Some("Export now") };
        let tuner = Tuner::new(Source::new(wide as u32, high as u32, pixels, None), size.format, size.tuned.unwrap_or(self.convert), words);
        (self.menu, self.menu_row) = (None, None);
        self.fine = Some(Fine { tuner, row, page: self.page, frame: self.active, divider: None, loupe: false, shrunk: Default::default() });
        self.dirty = true;
    }

    pub(super) fn close_fine(&mut self) {
        self.fine = None;
        self.dirty = true;
    }

    /// Where the panel floats: beside the right panel, level with the row
    /// it is for, kept within the window.
    fn fine_place(&mut self) -> Option<Rect> {
        let (row, high) = self.fine.as_ref().map(|fine| (fine.row, fine.tuner.height()))?;
        let by = self.control_rect(Control::Menu(MenuKind::Format(row)));
        let right = if self.right_open { self.width - RIGHT_W - 2 * INSET } else { self.width - INSET };
        let y = by.map_or(self.height / 2 - high / 2, |by| by.y - 60);
        Some(Rect::new(right - crate::tune::WIDTH, y.clamp(INSET, (self.height - high - INSET).max(INSET)), crate::tune::WIDTH, high))
    }

    /// The row's format and settings made those of the panel; and with
    /// `export`, the frame exported with them too.
    fn use_fine(&mut self, export: bool) {
        let Some(fine) = self.fine.take() else { return };
        if (fine.page, fine.frame) == (self.page, self.active) && fine.row < self.doc.exports.len() {
            self.checkpoint();
            let tuned = Some(fine.tuner.settings).filter(|settings| *settings != self.convert);
            (self.doc.exports[fine.row].format, self.doc.exports[fine.row].tuned) = (fine.tuner.format, tuned);
            self.changed();
            if export {
                self.export();
            } else {
                self.say(format!("Export row {} writes {}", fine.row + 1, fine.tuner.short()));
            }
        }
        self.dirty = true;
    }

    /// The pointer moved while the panel is open: over the panel, its
    /// buttons lit; over the frame, the line follows it.
    pub(super) fn fine_moved(&mut self, x: i32, y: i32) {
        let place = self.fine_place();
        let (a, b) = (self.to_window((0.0, 0.0)), self.to_window((self.doc.width as f32, self.doc.height as f32)));
        let Some(fine) = &mut self.fine else { return };
        if fine.tuner.moved(x, y) {
            self.dirty = true;
        }
        let over_panel = place.is_some_and(|place| place.contains(x, y));
        if !over_panel && x >= a.0 && x < b.0 && y >= a.1 - 40 && y < b.1 + 40 {
            fine.divider = Some(x);
            self.dirty = true;
        }
        if fine.loupe {
            self.dirty = true;
        }
    }

    /// A press while the panel is open; what it comes to, or nothing for
    /// the wheel, which still zooms and moves the page.
    pub(super) fn fine_pressed(&mut self, button: u8) -> Option<Outcome> {
        if (4..=7).contains(&button) {
            return None;
        }
        let place = self.fine_place()?;
        let (x, y) = self.pointer;
        if !place.contains(x, y) {
            self.close_fine();
            return Some(Outcome::Continue);
        }
        let asked = self.fine.as_mut()?.tuner.press(x, y);
        // Saved as the config file's table: what everything is written
        // with from now on, here as well.
        if let Some(fine) = &self.fine {
            if fine.tuner.said.as_deref().is_some_and(|said| said.starts_with("Saved as default")) {
                self.convert = fine.tuner.settings;
            }
        }
        match asked {
            Asked::Close => self.close_fine(),
            Asked::Primary => self.use_fine(false),
            Asked::Secondary => self.use_fine(true),
            Asked::Nothing => {}
        }
        self.dirty = true;
        Some(Outcome::Continue)
    }

    pub(super) fn fine_released(&mut self) {
        if let Some(fine) = &mut self.fine {
            fine.tuner.released();
        }
    }

    /// A key while the panel is open: typed into "Fit under…", Esc to
    /// close, Enter to use it, the arrows for the quality, Z for the
    /// loupe. Keys with Ctrl go on as ever.
    pub(super) fn fine_key(&mut self, key: Key, mods: Mods, down: bool) -> Option<Outcome> {
        let fine = self.fine.as_mut()?;
        if mods.ctrl {
            return None;
        }
        if matches!(key, Key::Char('z' | 'Z')) && fine.tuner.typing.is_none() {
            fine.loupe = down;
            self.dirty = true;
            return Some(Outcome::Continue);
        }
        if !down {
            return Some(Outcome::Continue);
        }
        if fine.tuner.typed(&key) {
            self.dirty = true;
            return Some(Outcome::Continue);
        }
        match key {
            Key::Escape => self.close_fine(),
            Key::Enter => self.use_fine(false),
            Key::Left => fine.tuner.nudge(if mods.shift { -10.0 } else { -1.0 }),
            Key::Right => fine.tuner.nudge(if mods.shift { 10.0 } else { 1.0 }),
            _ => {}
        }
        self.dirty = true;
        Some(Outcome::Continue)
    }

    /// What the panel's thread has found, taken; true if there was any.
    pub fn take_fine(&mut self) -> bool {
        let found = self.fine.as_mut().is_some_and(|fine| fine.tuner.poll());
        if found {
            self.dirty = true;
        }
        found
    }

    /// How soon to look at the panel's thread again.
    pub fn fine_wait(&self) -> Option<Duration> {
        self.fine.as_ref().filter(|fine| fine.tuner.busy()).map(|_| crate::tune::POLL)
    }

    /// The frame split: the original left of the line, the converted
    /// picture right of it, each at the row's size drawn at the canvas's
    /// zoom, the nearest pixel, so hard squares close up; a label over
    /// each side; and the loupe.
    pub(super) fn draw_split(&mut self, canvas: &mut Canvas, view: Rect) {
        let (a, b) = (self.to_window((0.0, 0.0)), self.to_window((self.doc.width as f32, self.doc.height as f32)));
        let Some(fine) = &mut self.fine else { return };
        if (fine.page, fine.frame) != (self.page, self.active) {
            return;
        }
        let source = &fine.tuner.source;
        let Some((after, format, size)) = fine.tuner.preview() else { return };
        let frame = Rect::new(a.0, a.1, b.0 - a.0, b.1 - a.1);
        let shown = frame.intersect(view);
        if shown.is_empty() || frame.w <= 0 || frame.h <= 0 {
            return;
        }
        let divider = fine.divider.unwrap_or(frame.x + frame.w / 2).clamp(frame.x, frame.right());
        let (sw, sh) = (source.width as i32, source.height as i32);
        let before = &source.flat;
        let at = |x: i32, y: i32| -> usize {
            let sx = (((x - frame.x) as f32 + 0.5) * sw as f32 / frame.w as f32) as i32;
            let sy = (((y - frame.y) as f32 + 0.5) * sh as f32 / frame.h as f32) as i32;
            (sy.clamp(0, sh - 1) * sw + sx.clamp(0, sw - 1)) as usize
        };
        // Shown smaller than it is, each side averaged rather than picked
        // from, which would shimmer on text; larger, the nearest pixel.
        if frame.w < sw && frame.h < sh {
            let (small_before, small_after) = fine.shrunk.of(&fine.tuner, frame.w, frame.h);
            let small_after = small_after.unwrap_or(small_before);
            let small = |x: i32, y: i32| ((y - frame.y).clamp(0, frame.h - 1) * frame.w + (x - frame.x).clamp(0, frame.w - 1)) as usize;
            canvas.paint(shown, |x, y| if x < divider { small_before[small(x, y)] } else { small_after[small(x, y)] } & 0xFF_FFFF);
        } else {
            canvas.paint(shown, |x, y| if x < divider { before[at(x, y)] } else { after[at(x, y)] } & 0xFF_FFFF);
        }
        // The line, with a handle across its middle.
        canvas.fill(divider - 1, shown.y, 2, shown.h, 0xFFFFFF);
        let middle = (frame.y + frame.h / 2).clamp(shown.y + 14, (shown.bottom() - 14).max(shown.y + 14));
        canvas.disc(divider, middle + 2, 14, 0x000000, 70);
        canvas.disc(divider, middle, 13, 0xFFFFFF, 256);
        for side in [-1, 1] {
            for step in 0..4 {
                let x = divider + side * (3 + step);
                canvas.fill(x, middle - (4 - step), 1, 2 * (4 - step), 0x1A1A1A);
            }
        }
        // What each side is.
        let png = fine.tuner.png_size().map_or(String::new(), |png| format!(" \u{b7} {}", crate::convert::size_words(png)));
        let left = format!("PNG{png}");
        let right = format!("{} \u{b7} {}", if format == fine.tuner.format { fine.tuner.short() } else { format.title().to_owned() }, crate::convert::size_words(size));
        let label = |text: &mut Text, canvas: &mut Canvas, x: i32, says: &str, from_right: bool| {
            let wide = text.width(says, 11) + 16;
            let x = if from_right { x - wide } else { x };
            canvas.round_fill(x, shown.y + 12, wide, 22, 5, 0x000000, 184);
            text.draw(canvas, x + 8, shown.y + 12 + (22 - Text::line_height(11)) / 2, 11, 0xFFFFFF, says);
        };
        label(&mut self.text, canvas, shown.x + 12, &left, false);
        label(&mut self.text, canvas, shown.right() - 12, &right, true);
        if fine.loupe {
            let (px, py) = self.pointer;
            let lens = Rect::new(px + 24, py + 24, 220, 150);
            let lens = Rect::new(lens.x.min(view.right() - lens.w - 8), lens.y.min(view.bottom() - lens.h - 8), lens.w, lens.h);
            canvas.round_fill(lens.x - 2, lens.y - 2, lens.w + 4, lens.h + 4, 10, 0xFFFFFF, 256);
            let (cx, cy) = (lens.x + lens.w / 2, lens.y + lens.h / 2);
            // Each pixel of the picture under the pointer LOUPE screen
            // pixels wide, on the side of the line it falls.
            let to_source = |x: i32, along: i32, size: i32| ((x - along) as f32 + 0.5) * size as f32;
            let (sx, sy) = ((to_source(px, frame.x, sw) / frame.w as f32) as i32, (to_source(py, frame.y, sh) / frame.h as f32) as i32);
            let line_at = (to_source(divider, frame.x, sw) / frame.w as f32) as i32;
            canvas.paint(lens, |x, y| {
                let (ax, ay) = (sx + (x - cx).div_euclid(LOUPE), sy + (y - cy).div_euclid(LOUPE));
                if ax < 0 || ay < 0 || ax >= sw || ay >= sh {
                    return 0x1E1E1E;
                }
                let at = (ay * sw + ax) as usize;
                (if ax < line_at { before[at] } else { after[at] }) & 0xFF_FFFF
            });
            let line = cx + (line_at - sx) * LOUPE;
            if line > lens.x && line < lens.right() {
                canvas.fill(line - 1, lens.y, 2, lens.h, ACCENT);
            }
            let says = format!("{}%", LOUPE * 100);
            self.text.draw(canvas, lens.x + 6, lens.bottom() - 18, 10, ACCENT, &says);
        }
    }

    /// The panel itself, over everything.
    pub(super) fn draw_fine(&mut self, canvas: &mut Canvas) {
        let Some(place) = self.fine_place() else { return };
        let Some(fine) = &mut self.fine else { return };
        fine.tuner.draw(canvas, &mut self.text, place);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_fine_tuned_writes_what_the_panel_says() {
        let mut studio = Studio::new(Doc::new(160, 100), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        studio.open_fine(0);
        let fine = studio.fine.as_mut().expect("open");
        fine.tuner.choose_family(2);
        fine.tuner.nudge(-20.0);
        studio.use_fine(false);
        assert!(studio.fine.is_none());
        let row = studio.doc.exports[0].clone();
        assert_eq!((row.format, row.tuned.map(|tuned| tuned.avif.quality)), (crate::output::Format::Avif, Some(68.0)));
        // Esc closes it, and the row stays as it was.
        studio.open_fine(0);
        studio.key(Key::Escape, Mods::default(), true);
        assert!(studio.fine.is_none() && studio.doc.exports[0] == row);
    }
}
