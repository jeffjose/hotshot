//! A board's panel of how it is copied and exported, opened by Options…
//! at the end of Copy as and Export as in its right click, or by
//! Ctrl+Shift+E: a small picture of what will be written, over what it is
//! written as, how large, on what, with how much room round it, and
//! whether a PNG keeps the board inside it to paste back as shapes; then
//! Copy, or Export. What is chosen is how the board is copied and
//! exported after, by its keys and its right click too, and is kept for
//! the next time hotshot runs.

use super::export::{self, Choices};
use super::*;

/// A part of the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) enum Part {
    /// What goes out: what is picked out, or the whole board.
    What(bool),
    /// The kind it is written as, by where it is among `FORMATS`.
    Format(u8),
    /// One, two or three times its size, by where it is among them.
    Size(u8),
    /// On the board's paper, on white, or on nothing.
    Ground(u8),
    /// The room round it, a slider.
    Room,
    /// A PNG with the board inside it.
    Keep,
    Copy,
    Export,
    Close,
    /// The panel, and all round it, which a click closes it on.
    Sheet,
    Scrim,
}

/// The kinds the panel offers, by their numbers in `export::KINDS`: PNG,
/// JPEG, SVG and AVIF.
pub(in crate::studio) const FORMATS: [u8; 4] = [0, 4, 3, 1];
const GROUNDS: [&str; 3] = ["Paper", "White", "None"];

/// How wide the panel is and the room inside its edge; how wide its
/// labels are, how high a row is and the gap after one; how high the
/// small picture is.
const WIDE: i32 = 344;
const PAD: i32 = 16;
const LABEL: i32 = 84;
const ROW: i32 = 28;
const GAP: i32 = 10;
const PICTURE_HIGH: i32 = 172;

/// The panel while it is open: whether it is of the whole board however
/// much is picked out; whether the room's slider is held; and the small
/// picture last drawn, with what it was drawn of and how large what is
/// written will be.
pub(in crate::studio) struct Panel {
    pub whole: bool,
    pub sliding: bool,
    preview: Option<Preview>,
}

struct Preview {
    of: (u64, Choices, bool, Vec<usize>),
    size: (u32, u32),
    w: i32,
    h: i32,
    pixels: Vec<u32>,
}

impl Studio {
    /// The panel opened, of the whole board or of what is picked out; any
    /// list closed, what is being typed kept.
    pub(in crate::studio) fn open_exporting(&mut self, whole: bool) {
        if !self.on_board() {
            return;
        }
        self.commit_text();
        (self.menu, self.menu_row, self.board.keys, self.exported) = (None, None, false, None);
        self.board.exporting = Some(Panel { whole, sliding: false, preview: None });
        self.dirty = true;
    }

    pub(in crate::studio) fn close_exporting(&mut self) {
        self.board.exporting = None;
        self.dirty = true;
    }

    /// Whether what the panel writes is the whole board: so chosen, or
    /// with nothing picked out.
    fn exporting_whole(&self) -> bool {
        self.board.exporting.as_ref().is_none_or(|panel| panel.whole) || self.chosen().is_empty()
    }

    /// Whether what is exported is a frame of the board, which is written
    /// as it is, with no room round it.
    fn exporting_frame(&self) -> bool {
        !self.on_page()
    }

    /// Where the panel is: in the middle of the board, a little high.
    fn exporting_place(&self) -> Rect {
        let high = self.exporting_rows().last().map_or(0, |&(_, rect)| rect.bottom()) + PAD;
        let open = self.open_area();
        Rect::new(open.x + (open.w - WIDE) / 2, (open.y + (open.h - high) * 2 / 5).max(open.y + 8), WIDE, high)
    }

    /// The parts of the panel, laid out from its top left at nothing: the
    /// title's row, the picture, then each row of choices, the box to
    /// keep the board, a note, and the buttons.
    fn exporting_rows(&self) -> Vec<(Part, Rect)> {
        let inner = WIDE - 2 * PAD;
        let mut rows = vec![(Part::Close, Rect::new(WIDE - PAD - 24 + 4, 10, 24, 24))];
        let mut y = 46 + PICTURE_HIGH + 8 + Text::line_height(11) + 16;
        let field = LABEL;
        let row = |rows: &mut Vec<(Part, Rect)>, y: &mut i32, parts: &[Part]| {
            let count = parts.len() as i32;
            let each = (inner - field) / count;
            for (at, &part) in parts.iter().enumerate() {
                let along = PAD + field + at as i32 * each;
                let w = if at as i32 == count - 1 { PAD + inner - along } else { each };
                rows.push((part, Rect::new(along, *y, w, ROW)));
            }
            *y += ROW + GAP;
        };
        if !self.chosen().is_empty() && !self.exporting_frame() {
            row(&mut rows, &mut y, &[Part::What(false), Part::What(true)]);
        }
        row(&mut rows, &mut y, &(0..FORMATS.len() as u8).map(Part::Format).collect::<Vec<_>>());
        row(&mut rows, &mut y, &[Part::Size(0), Part::Size(1), Part::Size(2)]);
        row(&mut rows, &mut y, &[Part::Ground(0), Part::Ground(1), Part::Ground(2)]);
        rows.push((Part::Room, Rect::new(PAD + field, y, inner - field, ROW)));
        y += ROW + GAP;
        rows.push((Part::Keep, Rect::new(PAD, y, inner, 24)));
        y += 24 + 6 + Text::line_height(11) + 18;
        rows.push((Part::Export, Rect::new(PAD + inner - 112, y, 112, 32)));
        rows.push((Part::Copy, Rect::new(PAD + inner - 112 - 8 - 96, y, 96, 32)));
        rows
    }

    /// The panel laid out over everything, and all round it, which takes
    /// a click to close it.
    pub(in crate::studio) fn layout_exporting(&mut self, layout: &mut Layout) {
        if self.board.exporting.is_none() || !self.on_board() {
            return;
        }
        let place = self.exporting_place();
        let mut front: Vec<(Rect, Control)> = self.exporting_rows().into_iter().map(|(part, rect)| (Rect::new(place.x + rect.x, place.y + rect.y, rect.w, rect.h), Control::Board(Ctl::Exporting(part)))).collect();
        front.push((place, Control::Board(Ctl::Exporting(Part::Sheet))));
        front.push((Rect::new(0, 0, self.width, self.height), Control::Board(Ctl::Exporting(Part::Scrim))));
        front.append(&mut layout.controls);
        layout.controls = front;
    }

    /// The choices changed as `change` changes them, kept for the next
    /// copy and export, and the next time hotshot runs.
    fn choose(&mut self, change: impl Fn(&mut Choices)) {
        change(&mut self.board.choices);
        export::keep_choices(self.board.choices);
        self.dirty = true;
    }

    /// Whether a part does nothing as things are: the size of an SVG,
    /// which is sharp at any; the room round a frame, which is written as
    /// it is; keeping the board in anything but a PNG; and copying an
    /// AVIF, which the clipboard does not take.
    fn exporting_idle(&self, part: Part) -> bool {
        let kind = self.board.choices.kind;
        match part {
            Part::Size(_) => kind == 3,
            Part::Room => self.exporting_frame(),
            Part::Keep => kind != 0,
            Part::Copy => kind == 1,
            _ => false,
        }
    }

    pub(in crate::studio) fn press_exporting(&mut self, part: Part) -> Outcome {
        self.dirty = true;
        if self.exporting_idle(part) {
            if part == Part::Copy {
                self.say("An AVIF can't go on the clipboard: export it instead");
            }
            return Outcome::Continue;
        }
        match part {
            Part::What(whole) => {
                if let Some(panel) = &mut self.board.exporting {
                    panel.whole = whole;
                }
            }
            Part::Format(at) => self.choose(|choices| choices.kind = FORMATS[usize::from(at).min(FORMATS.len() - 1)]),
            Part::Size(at) => self.choose(|choices| choices.times = at.min(2) + 1),
            Part::Ground(at) => self.choose(|choices| choices.ground = at.min(2)),
            Part::Keep => self.choose(|choices| choices.keep = !choices.keep),
            Part::Room => {
                if let Some(panel) = &mut self.board.exporting {
                    panel.sliding = true;
                }
                self.room_at(self.pointer.0);
            }
            Part::Copy => {
                let (kind, whole) = (self.board.choices.kind, self.exporting_whole());
                self.close_exporting();
                self.copy_board(kind, whole);
            }
            Part::Export => {
                let (kind, whole) = (self.board.choices.kind, self.exporting_whole());
                self.close_exporting();
                self.export_board_of(kind, whole);
            }
            Part::Close | Part::Scrim => self.close_exporting(),
            Part::Sheet => {}
        }
        Outcome::Continue
    }

    /// The room round what is exported, by where along its slider a point
    /// of the window is: whole pixels, in steps of four.
    pub(in crate::studio) fn room_at(&mut self, x: i32) {
        let Some(track) = self.control_rect(Control::Board(Ctl::Exporting(Part::Room))).map(room_track) else { return };
        let along = ((x - track.x) as f32 / track.w.max(1) as f32).clamp(0.0, 1.0);
        let room = ((along * f32::from(export::MOST_ROOM) / 4.0).round() * 4.0) as u16;
        if room != self.board.choices.room {
            self.board.choices.room = room;
            self.dirty = true;
        }
    }

    /// The pointer moved while the room's slider is held: the room as it
    /// says. Whether it was held.
    pub(in crate::studio) fn exporting_slide(&mut self, x: i32) -> bool {
        if !self.board.exporting.as_ref().is_some_and(|panel| panel.sliding) {
            return false;
        }
        self.room_at(x);
        true
    }

    /// The slider let go: the room it was left at kept.
    pub(in crate::studio) fn exporting_released(&mut self) {
        if let Some(panel) = self.board.exporting.as_mut().filter(|panel| panel.sliding) {
            panel.sliding = false;
            export::keep_choices(self.board.choices);
        }
    }

    /// A key while the panel is open: Esc closes it, Enter and Ctrl+E
    /// export, Ctrl+Shift+C copies, and Ctrl+Shift+E closes it again; a
    /// key with Ctrl besides closes it and does what it does, and the rest
    /// wait. Whether the key was taken.
    pub(in crate::studio) fn exporting_key(&mut self, key: Key, mods: Mods) -> bool {
        if self.board.exporting.is_none() {
            return false;
        }
        self.dirty = true;
        let lower = match key {
            Key::Char(letter) => Key::Char(letter.to_ascii_lowercase()),
            other => other,
        };
        match (lower, mods.ctrl, mods.shift) {
            (Key::Escape, ..) | (Key::Char('e'), true, true) => self.close_exporting(),
            (Key::Enter, false, _) | (Key::Char('e'), true, false) => drop(self.press_exporting(Part::Export)),
            (Key::Char('c'), true, true) => drop(self.press_exporting(Part::Copy)),
            (_, true, _) => {
                self.close_exporting();
                return false;
            }
            _ => {}
        }
        true
    }

    /// The small picture of what the panel would write, drawn afresh only
    /// when that changes; and how large what is written is.
    fn exporting_preview(&mut self, most: (i32, i32)) -> Option<((u32, u32), i32, i32, Vec<u32>)> {
        let whole = self.exporting_whole();
        let of = (self.version, self.board.choices, whole, self.selected.clone());
        if let Some(preview) = self.board.exporting.as_ref()?.preview.as_ref().filter(|preview| preview.of == of) {
            return Some((preview.size, preview.w, preview.h, preview.pixels.clone()));
        }
        let (layers, part, _) = self.export_whole_or_part(whole)?;
        let kind = self.board.choices.kind;
        let mut sheet = self.part_sheet(&layers, part);
        sheet.background = self.export_ground(kind).map_or(Fill::None, Fill::Color);
        let times = if kind == 3 { 1.0 } else { self.part_times(part.2, part.3) };
        let size = ((part.2 * times).round().max(1.0) as u32, (part.3 * times).round().max(1.0) as u32);
        let fit = (most.0 as f32 / part.2.max(1.0)).min(most.1 as f32 / part.3.max(1.0)).min(times);
        let (w, h, pixels) = sheet.render(fit, &mut self.text);
        let (w, h) = (w as i32, h as i32);
        if let Some(panel) = &mut self.board.exporting {
            panel.preview = Some(Preview { of, size, w, h, pixels: pixels.clone() });
        }
        Some((size, w, h, pixels))
    }

    /// What a part of the panel says as it is hovered.
    fn exporting_tip(&self, part: Part) -> Option<String> {
        let idle = self.exporting_idle(part);
        Some(match part {
            Part::What(false) => "What is picked out".to_owned(),
            Part::What(true) => "All of the board".to_owned(),
            Part::Format(at) => ["PNG: sharp, and can keep the board in it", "JPEG: smaller, on a ground", "SVG: shapes and words, sharp at any size", "AVIF: smallest, to export"][usize::from(at).min(3)].to_owned(),
            Part::Size(_) if idle => "An SVG is sharp at any size".to_owned(),
            Part::Size(at) => ["Its own size", "Twice its size, sharp on a fine screen", "Three times its size"][usize::from(at).min(2)].to_owned(),
            Part::Ground(at) => ["On the board's paper", "On white", "On nothing: see-through"][usize::from(at).min(2)].to_owned(),
            Part::Room if idle => "A frame is written as it is".to_owned(),
            Part::Room => "Room round it, in pixels of the board".to_owned(),
            Part::Keep if idle => "Only a PNG can keep the board in it".to_owned(),
            Part::Keep => "Pasted or dropped on a board, it opens as shapes".to_owned(),
            Part::Copy if idle => "An AVIF can't go on the clipboard".to_owned(),
            Part::Copy => "Copy to the clipboard  Ctrl+Shift+C".to_owned(),
            Part::Export => "Write it to a file  Ctrl+E".to_owned(),
            Part::Close => "Close  Esc".to_owned(),
            Part::Sheet | Part::Scrim => return None,
        })
    }

    /// The panel, over the board faintly dimmed and all that is over it.
    pub(in crate::studio) fn draw_exporting(&mut self, canvas: &mut Canvas, layout: &Layout, hover: Option<Control>) {
        if self.board.exporting.is_none() || !self.on_board() {
            return;
        }
        let Some(&(place, _)) = layout.controls.iter().find(|(_, control)| *control == Control::Board(Ctl::Exporting(Part::Sheet))) else { return };
        canvas.round_fill(0, 0, self.width, self.height, 0, 0x000000, 70);
        soft_shadow(canvas, place, 12);
        canvas.round_fill(place.x, place.y, place.w, place.h, 12, PANEL_COLOR, 256);
        canvas.round_outline(place.x, place.y, place.w, place.h, 12, 1, EDGE_COLOR);
        let inner = WIDE - 2 * PAD;
        let left = place.x + PAD;
        // Its title: what it exports.
        let title = match (self.exporting_frame(), self.exporting_whole()) {
            (true, _) => format!("Export {}", self.doc.name),
            (false, true) => "Export board".to_owned(),
            (false, false) => "Export selection".to_owned(),
        };
        self.text.look.heavy = 1;
        self.text.draw(canvas, left, place.y + 16, 14, TEXT_COLOR, &title);
        self.text.look.heavy = 0;
        // The small picture, in a well, on squares where it is see-through.
        let well = Rect::new(left, place.y + 46, inner, PICTURE_HIGH);
        canvas.round_fill(well.x, well.y, well.w, well.h, 8, 0x1F1F1F, 256);
        let preview = self.exporting_preview((well.w - 24, well.h - 24));
        let kind = self.board.choices.kind;
        if let Some((_, w, h, pixels)) = &preview {
            let (ox, oy) = (well.x + (well.w - w) / 2, well.y + (well.h - h) / 2);
            if self.export_ground(kind).is_none() {
                for y in 0..*h {
                    for x in 0..*w {
                        let light = ((x / 6) + (y / 6)) % 2 == 0;
                        canvas.fill(ox + x, oy + y, 1, 1, if light { 0x3A3A3A } else { 0x2E2E2E });
                    }
                }
            }
            for y in 0..*h {
                for x in 0..*w {
                    let pixel = pixels[(y * w + x) as usize];
                    canvas.blend(ox + x, oy + y, pixel & 0xFF_FFFF, (pixel >> 24) as u8);
                }
            }
            canvas.round_outline(ox - 1, oy - 1, w + 2, h + 2, 2, 1, 0x4A4A4A);
        }
        // Under it, how large what is written is.
        let line_high = Text::line_height(11);
        if let Some(((w, h), ..)) = preview {
            self.text.draw(canvas, left, well.bottom() + 8, 11, DIM_TEXT_COLOR, &format!("{w} \u{d7} {h} px"));
        }
        let parts: Vec<(Rect, Part)> = layout.controls.iter().filter_map(|&(rect, control)| match control {
            Control::Board(Ctl::Exporting(part)) if !matches!(part, Part::Sheet | Part::Scrim) => Some((rect, part)),
            _ => None,
        }).collect();
        // Each row's label, and the track of each row of choices.
        let mut labeled: Vec<i32> = Vec::new();
        for &(rect, part) in &parts {
            let label = match part {
                Part::What(_) => "Export",
                Part::Format(_) => "Format",
                Part::Size(_) => "Size",
                Part::Ground(_) => "Background",
                Part::Room => "Room",
                _ => continue,
            };
            if labeled.contains(&rect.y) {
                continue;
            }
            labeled.push(rect.y);
            let idle = self.exporting_idle(part);
            self.text.draw(canvas, left, rect.y + (rect.h - line_high) / 2, 11, if idle { 0x6A6A6A } else { DIM_TEXT_COLOR }, label);
            if part != Part::Room {
                canvas.round_fill(left + LABEL, rect.y, inner - LABEL, rect.h, 7, FIELD_COLOR, 256);
            }
        }
        let hovered = parts.iter().copied().find(|(_, part)| hover == Some(Control::Board(Ctl::Exporting(*part))));
        for &(rect, part) in &parts {
            self.draw_exporting_part(canvas, rect, part, hovered.is_some_and(|(_, over)| over == part));
        }
        // A note: what the choice under the pointer does, where it is a
        // choice, so that nothing covers the rows round it; else what
        // cannot be as chosen, or where an export goes.
        let note_y = parts.iter().find(|(_, part)| *part == Part::Keep).map_or(0, |(rect, _)| rect.bottom() + 6);
        let button = |part: Part| matches!(part, Part::Copy | Part::Export | Part::Close);
        match hovered.filter(|&(_, part)| !button(part)).and_then(|(_, part)| self.exporting_tip(part)) {
            Some(says) => self.text.draw(canvas, left, note_y, 11, TEXT_COLOR, &says),
            None if self.ground_lost(kind) => self.text.draw(canvas, left, note_y, 11, 0xF0B429, export::LOST),
            None => {
                self.text.draw_centered(canvas, left + 6, note_y + line_high / 2, 11, DIM_TEXT_COLOR, icons::FOLDER);
                let dir: String = shortened(&self.export_dir()).chars().take(46).collect();
                self.text.draw(canvas, left + 18, note_y, 11, DIM_TEXT_COLOR, &dir);
            }
        }
        canvas.fill(place.x + 1, note_y + line_high + 8, place.w - 2, 1, EDGE_COLOR);
        // A button's name and keys over it, or under the ×.
        if let Some((rect, part)) = hovered.filter(|&(_, part)| button(part)) {
            if let Some(says) = self.exporting_tip(part) {
                let px = 12;
                let high = Text::line_height(px) + 8;
                let wide = self.text.width(&says, px) + 16;
                let x = (rect.center().0 - wide / 2).min(place.right() - wide).max(4);
                let y = if part == Part::Close { rect.bottom() + 6 } else { rect.y - high - 6 };
                canvas.round_fill(x, y, wide, high, 6, 0x000000, 220);
                self.text.draw(canvas, x + 8, y + 4, px, TEXT_COLOR, &says);
            }
        }
    }


    /// A part of the panel drawn: a choice in its row, lit when it is the
    /// one chosen; the slider; the box; a button.
    fn draw_exporting_part(&mut self, canvas: &mut Canvas, rect: Rect, part: Part, hovered: bool) {
        let (cx, cy) = rect.center();
        let choices = self.board.choices;
        let idle = self.exporting_idle(part);
        let hovered = hovered && !idle;
        let ink = if idle { 0x6A6A6A } else { TEXT_COLOR };
        let line_high = Text::line_height(12);
        let choice = |studio: &mut Studio, canvas: &mut Canvas, on: bool, says: &str| {
            let chip = Rect::new(rect.x + 2, rect.y + 2, rect.w - 4, rect.h - 4);
            if on && !idle {
                canvas.round_fill(chip.x, chip.y, chip.w, chip.h, 5, ROW_ON, 256);
            } else if hovered {
                canvas.round_fill(chip.x, chip.y, chip.w, chip.h, 5, HOVER_COLOR, 256);
            }
            let color = if on && !idle { 0xFFFFFF } else { ink };
            let wide = studio.text.width(says, 12);
            studio.text.draw(canvas, cx - wide / 2, cy - line_high / 2, 12, color, says);
        };
        match part {
            Part::What(whole) => choice(self, canvas, self.exporting_whole() == whole, if whole { "Whole board" } else { "Selection" }),
            Part::Format(at) => choice(self, canvas, choices.kind == FORMATS[usize::from(at)], export::KINDS[usize::from(FORMATS[usize::from(at)])]),
            Part::Size(at) => choice(self, canvas, choices.times == at + 1, ["1\u{d7}", "2\u{d7}", "3\u{d7}"][usize::from(at)]),
            Part::Ground(at) => {
                let on = choices.ground == at;
                choice(self, canvas, on, "");
                // A swatch of what it goes on before its name.
                let says = GROUNDS[usize::from(at)];
                let wide = self.text.width(says, 12) + 18;
                let (x, y) = (cx - wide / 2, cy - 6);
                match at {
                    0 => canvas.round_fill(x, y, 12, 12, 3, self.board_ground(), 256),
                    1 => canvas.round_fill(x, y, 12, 12, 3, 0xFFFFFF, 256),
                    _ => {
                        for py in 0..12 {
                            for px in 0..12 {
                                canvas.fill(x + px, y + py, 1, 1, if (px / 4 + py / 4) % 2 == 0 { 0xBDBDBD } else { 0x7A7A7A });
                            }
                        }
                    }
                }
                canvas.round_outline(x, y, 12, 12, 3, 1, 0x5A5A5A);
                self.text.draw(canvas, x + 18, cy - line_high / 2, 12, if on { 0xFFFFFF } else { ink }, says);
            }
            Part::Room => {
                let track = room_track(rect);
                let along = f32::from(choices.room) / f32::from(export::MOST_ROOM);
                let knob = track.x + (along * track.w as f32).round() as i32;
                let lit = if idle { 0x4A4A4A } else { ACCENT };
                canvas.round_fill(track.x, cy - 2, track.w, 4, 2, 0x4A4A4A, 256);
                canvas.round_fill(track.x, cy - 2, (knob - track.x).max(4), 4, 2, lit, 256);
                if !idle {
                    canvas.disc(knob, cy, 8, 0xFFFFFF, 256);
                    canvas.disc(knob, cy, 6, if hovered || self.board.exporting.as_ref().is_some_and(|panel| panel.sliding) { 0x5472E4 } else { ACCENT }, 256);
                }
                let says = format!("{} px", choices.room);
                let wide = self.text.width(&says, 12);
                self.text.draw(canvas, rect.right() - wide, cy - line_high / 2, 12, ink, &says);
            }
            Part::Keep => {
                let on = choices.keep && !idle;
                let tick = Rect::new(rect.x, cy - 8, 16, 16);
                match on {
                    true => {
                        canvas.round_fill(tick.x, tick.y, tick.w, tick.h, 4, if hovered { 0x5472E4 } else { ACCENT }, 256);
                        self.text.draw_centered(canvas, tick.x + 8, tick.y + 8, 11, 0xFFFFFF, icons::CHECK);
                    }
                    false => canvas.round_outline(tick.x, tick.y, tick.w, tick.h, 4, 1, if idle { 0x555555 } else if hovered { TEXT_COLOR } else { DIM_TEXT_COLOR }),
                }
                self.text.draw(canvas, rect.x + 26, cy - line_high / 2, 12, if idle { 0x6A6A6A } else { TEXT_COLOR }, "Keep the board in it, to edit when pasted back");
            }
            Part::Copy => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { HOVER_COLOR } else { FIELD_COLOR }, 256);
                let wide = self.text.width("Copy", 12) + 20;
                self.text.draw_centered(canvas, cx - wide / 2 + 6, cy, 13, ink, icons::COPY);
                self.text.draw(canvas, cx - wide / 2 + 20, cy - line_high / 2, 12, ink, "Copy");
            }
            Part::Export => {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { 0x5472E4 } else { ACCENT }, 256);
                let wide = self.text.width("Export\u{2026}", 12) + 20;
                self.text.draw_centered(canvas, cx - wide / 2 + 6, cy, 13, 0xFFFFFF, icons::EXPORT);
                self.text.draw(canvas, cx - wide / 2 + 20, cy - line_high / 2, 12, 0xFFFFFF, "Export\u{2026}");
            }
            Part::Close => {
                if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                }
                self.text.draw_centered(canvas, cx, cy, 12, DIM_TEXT_COLOR, icons::CLOSE);
            }
            Part::Sheet | Part::Scrim => {}
        }
    }
}

/// Where along the room's row its slider's track is: all but the number
/// at its end.
fn room_track(rect: Rect) -> Rect {
    Rect::new(rect.x + 8, rect.y, rect.w - 8 - 56, rect.h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    /// A board of one box with words, nothing picked out.
    fn boxed() -> Studio {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 300.0), (460.0, 372.0), Mods::default());
        type_in(&mut studio, "Proxy");
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        studio
    }

    /// Where a part of the open panel is.
    fn at(studio: &mut Studio, part: Part) -> (i32, i32) {
        studio.control_rect(Control::Board(Ctl::Exporting(part))).unwrap_or_else(|| panic!("{part:?} in the panel")).center()
    }

    /// The panel's Copy clicked: what it copied, read.
    fn copied(studio: &mut Studio) -> crate::convert::Decoded {
        let copy = at(studio, Part::Copy);
        click(studio, copy);
        crate::convert::decode(&studio.copied_picture.take().expect("copied")).expect("a picture")
    }

    /// The panel opened if it is not, and a part of it clicked.
    fn choose(studio: &mut Studio, part: Part) {
        if studio.board.exporting.is_none() {
            press_key(studio, Key::Char('E'), CTRL_SHIFT);
        }
        let spot = at(studio, part);
        click(studio, spot);
    }

    const CTRL_SHIFT: Mods = Mods { ctrl: true, shift: true, alt: false };

    #[test]
    fn options_under_copy_as_and_ctrl_shift_e_open_the_panel_and_esc_or_a_click_off_it_close_it() {
        let mut studio = boxed();
        // From the right click's Copy as, its last row.
        studio.right_click(studio.to_window((800.0, 600.0)));
        let copy_as = studio.menu_items(MenuKind::Context).iter().position(|item| matches!(item, menu::Item::Entry { says, .. } if says == "Copy board as")).unwrap();
        let row = studio.control_rect(Control::MenuRow(0, copy_as as u16)).unwrap();
        studio.pointer_moved(row.center().0, row.center().1, Mods::default());
        let options = studio.menu_items(MenuKind::CopyAs).iter().position(|item| matches!(item, menu::Item::Entry { says, keys, .. } if says == "Options\u{2026}" && keys == "Ctrl Shift E")).expect("Options… in Copy as");
        let row = studio.control_rect(Control::MenuRow(1, options as u16)).unwrap();
        click(&mut studio, row.center());
        assert!(studio.board.exporting.is_some() && studio.menu.is_none(), "open, the list closed");
        let layout = studio.layout();
        assert!(matches!(layout.controls.first(), Some((_, Control::Board(Ctl::Exporting(_))))), "over everything");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.exporting.is_none());
        // Ctrl+Shift+E opens it, a click on it keeps it, off it closes it
        // and does nothing else.
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        assert!(studio.board.exporting.is_some());
        let sheet = studio.control_rect(Control::Board(Ctl::Exporting(Part::Sheet))).unwrap();
        click(&mut studio, (sheet.x + 30, sheet.y + 20));
        assert!(studio.board.exporting.is_some());
        let layers = studio.doc.layers.len();
        let off = studio.to_window((380.0, 330.0));
        click(&mut studio, off);
        assert!(studio.board.exporting.is_none() && studio.selected.is_empty() && studio.doc.layers.len() == layers, "closed, the box under it not picked");
        // Its ×, and Ctrl+Shift+E again.
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        let close = at(&mut studio, Part::Close);
        click(&mut studio, close);
        assert!(studio.board.exporting.is_none());
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        assert!(studio.board.exporting.is_none());
        // What is picked out, or the whole board, as it is opened.
        studio.selected = vec![0];
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        assert!(studio.board.exporting.as_ref().is_some_and(|panel| !panel.whole) && !studio.exporting_whole());
        choose(&mut studio, Part::What(true));
        assert!(studio.exporting_whole() && studio.selected == vec![0], "the whole board, still picked out");
        // Design's Ctrl+Shift+E is as it was.
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.set_mode(false);
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        assert!(studio.board.exporting.is_none());
    }

    #[test]
    fn each_choice_of_the_panel_changes_what_is_written_as_it_says_and_nothing_else() {
        let mut studio = boxed();
        let (file, undos, version) = (write_pages(&studio.gathered()), studio.undo.len(), studio.version);
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        let first = copied(&mut studio);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Board copied as PNG"));
        assert!(studio.board.exporting.is_none(), "closed once copied");
        // As it begins: twice its size, on its paper, 32 round it.
        let (_, part, _) = studio.export_whole_or_part(true).unwrap();
        assert_eq!((first.width, first.height), ((part.2 * 2.0).round() as i32, (part.3 * 2.0).round() as i32));
        assert_eq!(first.pixels[0] & 0xFF_FFFF, studio.board_ground());
        // 1×: half as wide and high; 3×, three times that.
        choose(&mut studio, Part::Size(0));
        let one = copied(&mut studio);
        assert_eq!((one.width * 2, one.height * 2), (first.width, first.height));
        choose(&mut studio, Part::Size(2));
        let three = copied(&mut studio);
        assert_eq!((three.width, three.height), (one.width * 3, one.height * 3));
        choose(&mut studio, Part::Size(0));
        // White, and nothing: see-through where nothing is drawn.
        choose(&mut studio, Part::Ground(1));
        let white = copied(&mut studio);
        assert_eq!((white.width, white.pixels[0] & 0xFF_FFFF), (one.width, 0xFFFFFF));
        choose(&mut studio, Part::Ground(2));
        let clear = copied(&mut studio);
        assert_eq!((clear.width, clear.pixels[0] >> 24), (one.width, 0));
        // A JPEG can't be: on white, and said so.
        choose(&mut studio, Part::Format(1));
        assert!(studio.ground_lost(4));
        let jpeg = copied(&mut studio);
        assert!(jpeg.pixels[0] & 0xFF_FFFF > 0xF0F0F0 && jpeg.pixels[0] >> 24 == 255, "on white: {:08X}", jpeg.pixels[0]);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Board copied as JPG, on white"));
        // The room: dragged along its slider, as many pixels more each side.
        choose(&mut studio, Part::Format(0));
        choose(&mut studio, Part::Ground(0));
        let row = studio.control_rect(Control::Board(Ctl::Exporting(Part::Room))).unwrap();
        let track = room_track(row);
        studio.pointer_moved(track.x + track.w / 2, row.center().1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        assert_eq!(studio.board.choices.room, export::MOST_ROOM / 2, "where it was pressed");
        studio.pointer_moved(track.right() + 40, row.center().1, Mods::default());
        studio.released();
        assert_eq!(studio.board.choices.room, export::MOST_ROOM, "as far as it goes");
        let roomy = copied(&mut studio);
        let more = 2 * (i32::from(export::MOST_ROOM) - 32);
        assert!((roomy.width - one.width - more).abs() <= 1 && (roomy.height - one.height - more).abs() <= 1, "{}x{} from {}x{}", roomy.width, roomy.height, one.width, one.height);
        // The board kept in it, or not.
        assert!(studio.board.choices.keep);
        choose(&mut studio, Part::Keep);
        let copy = at(&mut studio, Part::Copy);
        click(&mut studio, copy);
        assert_eq!(crate::output::board_of(&studio.copied_picture.take().unwrap()), None);
        // None of it changed the board, nor was a step to undo.
        assert_eq!((studio.undo.len(), studio.version), (undos, version));
        assert!(write_pages(&studio.gathered()) == file, "the board as it was");
    }

    #[test]
    fn what_the_panel_chose_is_how_the_keys_and_the_right_click_copy_and_export_after() {
        let mut studio = boxed();
        let dir = std::env::temp_dir().join(format!("hotshot-board-options-{}", std::process::id()));
        studio.dir = dir.clone();
        for part in [Part::Format(1), Part::Size(0), Part::Ground(1)] {
            choose(&mut studio, part);
        }
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(studio.board.choices, Choices { kind: 4, times: 1, ground: 1, room: 32, keep: true });
        // Ctrl+Shift+C: a JPEG, at its own size; the menu says so on JPEG.
        press_key(&mut studio, Key::Char('C'), CTRL_SHIFT);
        let by_key = studio.copied_picture.take().unwrap();
        assert_eq!(crate::clipboard::picture_kind(&by_key), Some("image/jpeg"));
        let (_, part, _) = studio.export_whole_or_part(true).unwrap();
        assert_eq!(crate::convert::decode(&by_key).unwrap().width, part.2.round() as i32);
        let keyed: Vec<String> = studio.menu_items(MenuKind::CopyAs).into_iter().filter_map(|item| match item {
            menu::Item::Entry { says, keys, .. } if !keys.is_empty() => Some(format!("{says} {keys}")),
            _ => None,
        }).collect();
        assert_eq!(keyed, ["JPEG Ctrl Shift C", "Options\u{2026} Ctrl Shift E"]);
        // The right click's PNG: at its own size, on white as well.
        studio.run_menu(crate::studio::menu::MenuAct::Board(Act::CopyBoard(0)));
        let png = crate::convert::decode(&studio.copied_picture.take().unwrap()).unwrap();
        assert_eq!((png.width, png.pixels[0] & 0xFF_FFFF), (part.2.round() as i32, 0xFFFFFF));
        // Ctrl+E writes a JPEG; the panel's Export, an SVG once chosen,
        // on white.
        press_key(&mut studio, Key::Char('e'), Mods { ctrl: true, ..Mods::default() });
        choose(&mut studio, Part::Format(2));
        press_key(&mut studio, Key::Enter, Mods::default());
        assert!(studio.board.exporting.is_none());
        let written: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|entry| entry.path()).collect();
        let ending = |end: &str| written.iter().find(|path| path.extension().is_some_and(|ending| ending == end)).cloned();
        assert!(ending("jpg").is_some(), "Ctrl+E as a JPEG: {written:?}");
        let svg = std::fs::read_to_string(ending("svg").expect("an SVG written")).unwrap();
        assert!(svg.contains("fill=\"#FFFFFF\"/>") && svg.contains("Proxy"));
        let _ = std::fs::remove_dir_all(&dir);
        // An AVIF can't be copied: the panel's Copy says so and copies
        // nothing; Ctrl+Shift+C copies a PNG.
        studio.board.choices.kind = 1;
        choose(&mut studio, Part::Copy);
        assert!(studio.copied_picture.is_none() && studio.board.exporting.is_some());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('C'), CTRL_SHIFT);
        assert_eq!(studio.copied_picture.as_deref().and_then(crate::clipboard::picture_kind), Some("image/png"));
    }

    #[test]
    fn the_choices_are_kept_as_a_line_and_read_back_kept_to_what_they_can_be() {
        let choices = Choices { kind: 3, times: 1, ground: 2, room: 64, keep: false };
        assert_eq!(Choices::read(&choices.word()), choices);
        assert_eq!(Choices::read(""), Choices::default());
        assert_eq!(Choices::read("99 9 7 9999 1"), Choices { kind: export::KINDS.len() as u8 - 1, times: 3, ground: 2, room: export::MOST_ROOM, keep: true });
        // Never read nor written under test.
        assert_eq!(export::last_choices(), Choices::default());
    }

    #[test]
    fn the_panel_s_picture_is_drawn_afresh_only_when_what_it_shows_changes() {
        let mut studio = boxed();
        press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
        let (size, w, h, _) = studio.exporting_preview((300, 150)).unwrap();
        assert!(w <= 300 && h <= 150 && (w >= 298 || h >= 148), "fitted: {w}x{h}");
        let (_, part, _) = studio.export_whole_or_part(true).unwrap();
        assert_eq!(size, ((part.2 * 2.0).round() as u32, (part.3 * 2.0).round() as u32), "says how large what is written is");
        let kept = studio.board.exporting.as_ref().unwrap().preview.as_ref().map(|preview| preview.pixels.as_ptr());
        studio.exporting_preview((300, 150));
        assert_eq!(studio.board.exporting.as_ref().unwrap().preview.as_ref().map(|preview| preview.pixels.as_ptr()), kept, "the same, not drawn again");
        choose(&mut studio, Part::Ground(2));
        let (_, _, _, pixels) = studio.exporting_preview((300, 150)).unwrap();
        assert_eq!(pixels[0] >> 24, 0, "drawn again, see-through");
    }

    /// The panel over a board, in each look, a part of it hovered:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test exporting_snapshot -- --ignored
    #[test]
    #[ignore]
    fn exporting_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        for look in 0..=look::DIAGRAM {
            let mut studio = board();
            studio.set_look(Some(look), None);
            assert!(studio.bring_mermaid("flowchart LR\n  API --> Database\n  API --> Cache"));
            studio.selected.clear();
            press_key(&mut studio, Key::Char('E'), CTRL_SHIFT);
            let hovered = at(&mut studio, [Part::Format(0), Part::Ground(2), Part::Export, Part::Close][usize::from(look)]);
            if look == 1 {
                choose(&mut studio, Part::Ground(2));
            }
            if look == 3 {
                studio.selected = vec![0];
                studio.board.choices.kind = 4;
                studio.board.choices.ground = 2;
            }
            studio.pointer_moved(hovered.0, hovered.1, Mods::default());
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            std::fs::write(to.join(format!("exporting-{}.png", look::LOOKS[usize::from(look)].to_lowercase())), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
        }
    }
}
