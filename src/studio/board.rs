//! Board mode: a page for explaining rather than for pixels, as Excalidraw
//! is. A page is a board or a page of frames, switched at the top left as
//! Claude's Chat and Code are, or by Ctrl+M; a board is one endless sheet
//! of warm paper with dots, its boxes, ovals and diamonds drawn by hand in
//! hand lettering, its tools along the bottom and a small bar of styles by
//! what is picked out rather than the panels. The same layers underneath:
//! switched back, they are a page's own layers as any are.

use super::*;

/// The ink a board draws in, its five line colors and five fills, as
/// Excalidraw keeps them; and the gray of its dots and of a note's edge.
pub(super) const INK: u32 = 0x1E1E1E;
pub(super) const STROKES: [u32; 5] = [0x1E1E1E, 0xE03131, 0x2F9E44, 0x1971C2, 0xF08C00];
pub(super) const FILLS: [u32; 5] = [0xFFF3BF, 0xD3F9D8, 0xD0EBFF, 0xFFE3E3, 0xE5DBFF];
#[cfg(test)]
const DOTS: u32 = 0xD8D6CF;
pub(super) const QUIET: u32 = 0x868E96;
/// The lettering a board's words are in: Architects Daughter, by where it
/// is among the faces; and how large, and how wide a line is.
pub(super) const LETTERING: usize = 2;
pub(super) const WORDS_PX: u32 = 20;
pub(super) const LINE: f32 = 2.0;
/// How far apart the dots of the paper are, on the page.
const DOT_STEP: f32 = 18.0;
/// The size a box clicked out with no drag is, before two have been
/// drawn the same; and a sticky note's.
pub(super) const BOX: (f32, f32) = (160.0, 72.0);
pub(super) const STICKY: (f32, f32) = (150.0, 150.0);

/// The tools along the bottom of a board, each with its key: the pointer,
/// the hand, a box, an oval, a diamond, an arrow, a line, the pen, text, a
/// sticky note, and a picture.
pub(super) const TOOLS: [(&str, char); 11] = [("Select", 'v'), ("Hand", 'h'), ("Box", 'r'), ("Oval", 'o'), ("Diamond", 'd'), ("Arrow", 'a'), ("Line", 'l'), ("Pen", 'p'), ("Text", 't'), ("Sticky note", 's'), ("Picture", 'i')];
pub(super) const SELECT: u8 = 0;
pub(super) const HAND: u8 = 1;
pub(super) const BOX_TOOL: u8 = 2;
pub(super) const OVAL: u8 = 3;
pub(super) const DIAMOND: u8 = 4;
pub(super) const ARROW: u8 = 5;
pub(super) const LINE_TOOL: u8 = 6;
pub(super) const PEN: u8 = 7;
pub(super) const TEXT: u8 = 8;
pub(super) const STICKY_TOOL: u8 = 9;
pub(super) const PICTURE: u8 = 10;

/// What on a board is clicked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ctl {
    /// The switch at the top left: Board, or Design.
    Mode(bool),
    /// A tool of the board's bar.
    Tool(u8),
    /// The style bar: a line color, a fill or none, the look, the size of
    /// the words, guessing on or off, and the status dot.
    Stroke(u8),
    Fill(u8),
    NoFill,
    Look,
    /// What the board is on: light paper, dark gray or black.
    Ground,
    Words(u8),
    Guess,
    Status,
    /// The history: its slider, Branch from here, and Back to now.
    HistoryTrack,
    Branch,
    Now,
    /// Present, at the top right.
    Present,
}

/// What a board keeps of its own, in the window rather than the file.
pub(super) struct Board {
    /// How the next box, oval, diamond, note, arrow or line is drawn: its
    /// line's color, its fill, whether by hand, how large its words are
    /// and how wide its line; carried on from the last one changed.
    pub stroke: u32,
    pub fill: Option<u32>,
    pub sketch: bool,
    pub px: u32,
    pub width: f32,
    /// What the box tool draws: a box, a diamond or a note.
    pub making: u8,
    /// The sizes of the last two boxes drawn, the latest last.
    pub sizes: Vec<(f32, f32)>,
    /// Whether the left panel and the right are shown on a board: neither,
    /// until asked for.
    pub left: bool,
    pub right: bool,
    /// Whether it guesses what comes next.
    pub guessing: bool,
    /// The box whose sides show a dot each to pull an arrow out of, by its
    /// layer: the one the pointer is over or near, or the one picked out.
    pub dots_of: Option<usize>,
    /// C pressed: the next key a color of the style bar.
    pub coloring: bool,
    /// What is in what is being moved, carried along with it, each with
    /// how it was when the move began.
    pub carried: Vec<(usize, Layer)>,
    /// What was moved is being let go, and not yet in or out of a box.
    pub letting_go: bool,
    /// What it guesses comes next, offered faintly until kept or let go.
    pub ghost: Option<ghost::Ghost>,
    /// The arrow a branch is being pulled from the middle of.
    pub branching: Option<usize>,
    /// The board's history, while it is gone through.
    pub history: Option<change::History>,
    /// What is lit, by what each is called, the rest faded; the parts being
    /// presented; and the laser, its trail, and a box it made glow.
    pub focus: Option<Vec<u32>>,
    pub present: Option<explain::Present>,
    pub laser: Option<explain::Laser>,
    pub trail: Vec<((i32, i32), std::time::Instant)>,
    pub glow: Option<(u32, std::time::Instant)>,
    /// What the box tool would draw round, outlined.
    pub pick: Option<pick::Pick>,
    /// What letting go of the box being moved would do to an arrow.
    pub drop: Option<change::Drop>,
    /// The SVG last brought in as shapes, to keep as a picture instead.
    pub svg: Option<svg::Brought>,
}

impl Board {
    pub(super) fn new(_board: bool) -> Board {
        Board { stroke: INK, fill: Some(FILLS[2]), sketch: true, px: WORDS_PX, width: LINE, making: shapes::FORM_BOX, sizes: Vec::new(), left: false, right: false, guessing: true, dots_of: None, coloring: false, carried: Vec::new(), letting_go: false, ghost: None, branching: None, history: None, focus: None, present: None, laser: None, trail: Vec::new(), glow: None, pick: None, drop: None, svg: None }
    }
}

mod bar;
mod dots;
mod nest;
mod map;
mod ghost;
mod change;
mod explain;
mod pick;
pub(super) mod svg;
mod export;
pub(super) mod look;
mod act;
pub(super) use act::Act;
mod typing;

/// Whether the last canvas switched to Board, kept in `canvas-mode` among
/// hotshot's state, for a new file to begin as one.
pub(super) fn last_was_board() -> bool {
    crate::output::state_file("canvas-mode").and_then(|file| std::fs::read_to_string(file).ok()).is_some_and(|kept| kept.trim() == "board")
}

fn keep_mode(board: bool) {
    if cfg!(test) {
        return;
    }
    let Some(file) = crate::output::state_file("canvas-mode") else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, if board { "board\n" } else { "design\n" });
}

/// A file begun as a board: one page, with nothing on it.
pub(super) fn board_pages() -> Vec<Page> {
    let mut page = Page::new("Board 1", Vec::new());
    page.board = true;
    vec![page]
}

impl Studio {
    /// Whether the page shown is a board.
    pub(super) fn on_board(&self) -> bool {
        self.pages[self.page].board
    }

    /// Whether the board is being presented, a step at a time.
    pub(super) fn presenting(&self) -> bool {
        self.board.present.is_some()
    }

    /// The page shown made a board, or a page of frames again: its layers
    /// as they were, worked in on the page; the mode kept for a new file.
    pub(super) fn set_mode(&mut self, board: bool) {
        if self.on_board() == board {
            return;
        }
        self.commit_text();
        self.commit_name();
        self.checkpoint();
        self.pages[self.page].board = board;
        keep_mode(board);
        if board {
            self.activate(LOOSE);
        }
        (self.tool, self.framing) = (Tool::Select, false);
        self.selected.clear();
        self.shown.clear();
        self.shown_page.clear();
        self.changed();
        self.say(if board { "Board: for explaining" } else { "Design: for pixels" });
    }

    /// Another page shown: a board's tool back to the pointer.
    pub(super) fn board_page_shown(&mut self) {
        if self.on_board() && !matches!(self.tool, Tool::Select) {
            self.tool = Tool::Select;
        }
    }

    /// The switch at the top left, beside the button that hides the left
    /// panel, where it is whether the panel is shown or not.
    pub(super) fn switch_place(&self) -> Rect {
        let beside = self.side_button(0);
        Rect::new(beside.right() + 10, beside.y - 1, 164, 30)
    }

    pub(super) fn layout_switch(&mut self, layout: &mut Layout) {
        let place = self.switch_place();
        if self.on_board() && !self.zen && !self.shows_right() {
            let beside = self.side_button(1);
            layout.controls.push((Rect::new(beside.x - 14 - 96, beside.y - 1, 96, 30), Control::Board(Ctl::Present)));
        }
        layout.controls.push((Rect::new(place.x, place.y, place.w / 2, place.h), Control::Board(Ctl::Mode(false))));
        layout.controls.push((Rect::new(place.x + place.w / 2, place.y, place.w - place.w / 2, place.h), Control::Board(Ctl::Mode(true))));
    }

    /// The bar of a board's tools, along the bottom as Design's is.
    pub(super) fn board_toolbar(&mut self, layout: &mut Layout) {
        let (side, gap, pad) = (36, 2, 5);
        let wide = TOOLS.len() as i32 * (side + gap) - gap + 2 * pad;
        let open = self.open_area();
        let dock = Rect::new(open.x + (open.w - wide) / 2, self.height - 20 - side - 2 * pad, wide, side + 2 * pad);
        for at in 0..TOOLS.len() {
            layout.controls.push((Rect::new(dock.x + pad + at as i32 * (side + gap), dock.y + pad, side, side), Control::Board(Ctl::Tool(at as u8))));
        }
        layout.controls.push((dock, Control::Panel));
        layout.dock = dock;
    }

    /// Which of the board's tools is in hand.
    pub(super) fn board_tool(&self) -> u8 {
        if self.space {
            return HAND;
        }
        match self.tool {
            Tool::Rect if self.board.making == shapes::FORM_DIAMOND => DIAMOND,
            Tool::Rect if self.board.making == shapes::FORM_STICKY => STICKY_TOOL,
            Tool::Rect => BOX_TOOL,
            Tool::Ellipse => OVAL,
            Tool::Arrow => ARROW,
            Tool::Line => LINE_TOOL,
            Tool::Pen | Tool::Marker => PEN,
            Tool::Text => TEXT,
            _ => SELECT,
        }
    }

    /// One of the board's tools taken up; a picture asked of the clipboard.
    pub(super) fn take_board_tool(&mut self, which: u8) -> Outcome {
        self.space = false;
        let (tool, making) = match which {
            HAND => {
                self.set_tool(Tool::Select);
                self.space = true;
                return Outcome::Continue;
            }
            PICTURE => {
                self.say("A picture from the clipboard; or drop one on the board");
                return Outcome::Paste;
            }
            BOX_TOOL => (Tool::Rect, shapes::FORM_BOX),
            DIAMOND => (Tool::Rect, shapes::FORM_DIAMOND),
            STICKY_TOOL => (Tool::Rect, shapes::FORM_STICKY),
            OVAL => (Tool::Ellipse, shapes::FORM_BOX),
            ARROW => (Tool::Arrow, self.board.making),
            LINE_TOOL => (Tool::Line, self.board.making),
            PEN => (Tool::Pen, self.board.making),
            TEXT => (Tool::Text, self.board.making),
            _ => (Tool::Select, self.board.making),
        };
        self.board.making = making;
        self.set_tool(tool);
        Outcome::Continue
    }

    pub(super) fn press_board(&mut self, ctl: Ctl) -> Outcome {
        self.dirty = true;
        match ctl {
            Ctl::Mode(board) => self.set_mode(board),
            Ctl::Tool(which) => return self.take_board_tool(which),
            Ctl::Present => self.present(),
            Ctl::HistoryTrack => {
                let x = self.pointer.0;
                if let Some(history) = &mut self.board.history {
                    history.sliding = true;
                }
                self.history_at(x);
            }
            Ctl::Branch => self.branch_here(),
            Ctl::Now => self.history_now(),
            other => self.press_style(other),
        }
        Outcome::Continue
    }

    /// Ctrl+M: the page shown the other kind.
    pub(super) fn flip_mode(&mut self) {
        let board = !self.on_board();
        self.set_mode(board);
    }

    /// A shape about to be drawn on a board, made as a board draws one: by
    /// hand, in its ink and fill, its words lettered; a box the kind the
    /// box tool makes; an arrow routed between what it joins.
    pub(super) fn board_dress(&self, shape: &mut Shape) {
        let board = &self.board;
        (shape.color, shape.stroke, shape.face, shape.font_px) = (board.stroke, board.width, LETTERING, board.px);
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                (shape.sketch, shape.border, shape.radius) = (board.sketch, true, 6.0);
                (shape.filled, shape.fill) = (board.fill.is_some(), board.fill.unwrap_or(0xFFFFFF));
                if shape.tool == Tool::Rect {
                    shape.form = board.making;
                }
                if shape.form == shapes::FORM_STICKY {
                    // A note: yellow unless a fill was chosen, its edge a
                    // quiet gray, on a soft shadow.
                    (shape.color, shape.filled) = (QUIET, true);
                    if board.fill.is_none() || board.fill == Some(FILLS[2]) {
                        shape.fill = FILLS[0];
                    }
                    shape.shadow = Some(frame::Layer { x: 0, y: 3, blur: 8, spread: 0, color: 0x2E00_0000 });
                }
            }
            Tool::Arrow | Tool::Line => (shape.sketch, shape.routed) = (board.sketch, true),
            Tool::Text => shape.color = INK,
            _ => {}
        }
    }

    /// The paper of a board under everything on it, with its dots, which
    /// go as the page is moved and grow as it is zoomed; fewer as it is
    /// zoomed out, so they never crowd.
    pub(super) fn draw_paper(&self, out: &mut [u32], view: Rect) {
        let width = self.width;
        let (paper, dots) = self.paper();
        out.par_chunks_mut(1 << 16).for_each(|part| part.fill(paper));
        let mut step = DOT_STEP * self.zoom;
        while step < 9.0 {
            step *= 2.0;
        }
        let big = if self.zoom >= 1.6 { 2 } else { 1 };
        let first = |pan: f32| pan.rem_euclid(step);
        let (ox, oy) = (first(self.pan.0), first(self.pan.1));
        let mut y = view.y as f32 + oy;
        while y < view.bottom() as f32 {
            let mut x = view.x as f32 + ox;
            while x < view.right() as f32 {
                let (px, py) = (x as i32, y as i32);
                for dy in 0..big {
                    for dx in 0..big {
                        if px + dx < view.right() && py + dy < view.bottom() {
                            out[((py + dy) * width + px + dx) as usize] = dots;
                        }
                    }
                }
                x += step;
            }
            y += step;
        }
    }

    /// A control of the board's drawn: the switch, a tool, or a part of the
    /// style bar.
    pub(super) fn draw_board_control(&mut self, canvas: &mut Canvas, rect: Rect, ctl: Ctl, hovered: bool) {
        let (cx, cy) = rect.center();
        match ctl {
            Ctl::Mode(board) => {
                // The pill both halves are on, drawn with the first.
                if !board {
                    let place = self.switch_place();
                    soft_shadow(canvas, place, 9);
                    canvas.round_fill(place.x, place.y, place.w, place.h, 9, PANEL_COLOR, 256);
                    canvas.round_outline(place.x, place.y, place.w, place.h, 9, 1, EDGE_COLOR);
                }
                let on = self.on_board() == board;
                let inner = Rect::new(rect.x + 3, rect.y + 3, rect.w - 6, rect.h - 6);
                if on {
                    canvas.round_fill(inner.x, inner.y, inner.w, inner.h, 6, ACCENT, 256);
                } else if hovered {
                    canvas.round_fill(inner.x, inner.y, inner.w, inner.h, 6, HOVER_COLOR, 256);
                }
                let color = if on { 0xFFFFFF } else if hovered { TEXT_COLOR } else { DIM_TEXT_COLOR };
                let words = if board { "Board" } else { "Design" };
                let wide = self.text.width(words, 12) + 20;
                let left = cx - wide / 2;
                self.text.draw_centered(canvas, left + 7, cy, 13, color, if board { icons::PEN } else { icons::FRAME });
                self.text.draw(canvas, left + 18, cy - Text::line_height(12) / 2, 12, color, words);
            }
            Ctl::Tool(which) => {
                let on = self.board_tool() == which;
                if on {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, ACCENT, 256);
                } else if hovered {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, HOVER_COLOR, 256);
                }
                let color = if on { 0xFFFFFF } else { TEXT_COLOR };
                match which {
                    HAND => draw_hand(canvas, cx, cy, color),
                    // A note, its corner turned down.
                    STICKY_TOOL => {
                        canvas.round_outline(cx - 8, cy - 8, 16, 16, 2, 2, color);
                        canvas.round_fill(cx + 2, cy + 2, 7, 7, 1, if on { ACCENT } else { PANEL_COLOR }, 256);
                        line(canvas, (cx + 2, cy + 8), (cx + 8, cy + 2), color);
                        line(canvas, (cx + 2, cy + 2), (cx + 2, cy + 8), color);
                        line(canvas, (cx + 2, cy + 2), (cx + 8, cy + 2), color);
                    }
                    DIAMOND => {
                        let r = 8;
                        for (a, b) in [((cx, cy - r), (cx + r, cy)), ((cx + r, cy), (cx, cy + r)), ((cx, cy + r), (cx - r, cy)), ((cx - r, cy), (cx, cy - r))] {
                            line(canvas, a, b, color);
                            line(canvas, (a.0, a.1 + 1), (b.0, b.1 + 1), color);
                        }
                    }
                    _ => {
                        let icon = match which {
                            BOX_TOOL => icons::RECT,
                            OVAL => icons::ELLIPSE,
                            ARROW => icons::ARROW,
                            LINE_TOOL => icons::LINE,
                            PEN => icons::PEN,
                            TEXT => icons::TEXT,
                            STICKY_TOOL => icons::STICKER,
                            PICTURE => icons::PICTURE,
                            _ => icons::SELECT,
                        };
                        self.text.draw_centered(canvas, cx, cy, 17, color, icon);
                    }
                }
                // Its key, small, in its corner, while it is hovered.
                if hovered {
                    let key = TOOLS[usize::from(which)].1.to_ascii_uppercase().to_string();
                    self.text.draw(canvas, rect.right() - 9, rect.bottom() - 14, 9, if on { 0xFFFFFF } else { DIM_TEXT_COLOR }, &key);
                }
            }
            Ctl::HistoryTrack | Ctl::Branch | Ctl::Now => self.draw_history_control(canvas, rect, ctl, hovered),
            Ctl::Present => {
                soft_shadow(canvas, rect, 9);
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 9, if hovered { HOVER_COLOR } else { PANEL_COLOR }, 256);
                canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 9, 1, EDGE_COLOR);
                // A play mark, and the word.
                for row in -5..=5 {
                    let reach = 8 - (row as i32).abs() * 8 / 5;
                    canvas.fill(rect.x + 14, cy + row, reach, 1, TEXT_COLOR);
                }
                self.text.draw(canvas, rect.x + 30, cy - Text::line_height(12) / 2, 12, TEXT_COLOR, "Present");
            }
            other => self.draw_style_control(canvas, rect, other, hovered),
        }
    }

}

/// A hand, for the tool that moves the board: a palm and four fingers.
fn draw_hand(canvas: &mut Canvas, cx: i32, cy: i32, color: u32) {
    canvas.round_fill(cx - 6, cy - 1, 12, 9, 4, color, 256);
    for (at, tall) in [(-6, 7), (-3, 9), (0, 9), (3, 8)] {
        canvas.round_fill(cx + at, cy - tall, 3, tall + 2, 1, color, 256);
    }
    canvas.round_fill(cx - 9, cy - 2, 4, 6, 2, color, 256);
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A canvas on a board, as a new file is when Board was last used.
    pub(in crate::studio) fn board() -> Studio {
        let mut studio = Studio::with_pages(board_pages(), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (1.0, (0.0, 0.0));
        studio
    }

    pub(in crate::studio) fn press_key(studio: &mut Studio, key: Key, mods: Mods) {
        studio.key(key, mods, true);
        studio.key(key, mods, false);
    }

    pub(in crate::studio) fn type_in(studio: &mut Studio, words: &str) {
        for letter in words.chars() {
            let key = if letter == '\n' { Key::Enter } else { Key::Char(letter) };
            press_key(studio, key, Mods { shift: letter.is_uppercase(), ..Mods::default() });
        }
    }

    /// A drag on the board from one point of it to another.
    pub(in crate::studio) fn pull(studio: &mut Studio, from: (f32, f32), to: (f32, f32), mods: Mods) {
        let (a, b) = (studio.to_window_page(from), studio.to_window_page(to));
        studio.pointer_moved(a.0, a.1, mods);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((a.0 + b.0) / 2, (a.1 + b.1) / 2, mods);
        studio.pointer_moved(b.0, b.1, mods);
        studio.released();
    }

    /// A click at a point of the board.
    pub(in crate::studio) fn tap(studio: &mut Studio, at: (f32, f32)) {
        let at = studio.to_window_page(at);
        click(studio, at);
    }

    pub(in crate::studio) fn click(studio: &mut Studio, at: (i32, i32)) {
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
    }

    pub(in crate::studio) fn shape(studio: &Studio, index: usize) -> Shape {
        match &studio.doc.layers[index].kind {
            Kind::Shape(shape) => shape.clone(),
            Kind::Picture(_) => panic!("a shape"),
        }
    }

    pub(in crate::studio) fn shown(studio: &mut Studio, at: (f32, f32)) -> u32 {
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let (x, y) = studio.to_window_page(at);
        out[(y * studio.width + x) as usize] & 0xFF_FFFF
    }

    #[test]
    fn a_page_is_a_board_or_a_page_of_frames_switched_at_the_top_left_and_kept() {
        let mut studio = board();
        // A board: no frames, worked in on its page, on its paper, with
        // neither panel.
        assert!(studio.on_board() && studio.on_page() && studio.frame_count() == 0);
        assert!(!studio.shows_left() && !studio.shows_right());
        assert!(matches!(shown(&mut studio, (300.0, 300.0)), shapes::PAPER | DOTS));
        // The switch: Design, and back with Ctrl+M.
        let design = studio.control_rect(Control::Board(Ctl::Mode(false))).unwrap().center();
        let board_half = studio.control_rect(Control::Board(Ctl::Mode(true))).unwrap();
        assert!(board_half.x < 300 && board_half.y < 60, "at the top left, where it always is");
        click(&mut studio, design);
        assert!(!studio.on_board());
        assert_ne!(shown(&mut studio, (300.0, 300.0)), shapes::PAPER, "Design is on the dark");
        press_key(&mut studio, Key::Char('m'), Mods { ctrl: true, ..Mods::default() });
        assert!(studio.on_board());
        // The panels, hidden, are one key away, as in Design.
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, alt: true, ..Mods::default() });
        assert!(studio.shows_right());
        // A new page is a board too; the file keeps which is which.
        studio.new_page();
        studio.commit_name();
        assert!(studio.on_board() && studio.pages.len() == 2 && studio.frame_count() == 0);
        studio.set_mode(false);
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert!(back[0].board && back[0].frames.is_empty());
        assert!(!back[1].board && back[1].frames.is_empty(), "a page made Design before it had a frame keeps none");
        // Undone, the switch is too.
        studio.undo();
        assert!(studio.on_board());
    }

    #[test]
    fn a_boards_tools_draw_by_hand_in_its_ink_and_lettering() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        assert_eq!(studio.board_tool(), BOX_TOOL);
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        studio.commit_text();
        press_key(&mut studio, Key::Char('d'), Mods::default());
        assert_eq!(studio.board_tool(), DIAMOND, "D is the diamond on a board, not a spotlight");
        pull(&mut studio, (400.0, 100.0), (540.0, 200.0), Mods::default());
        studio.commit_text();
        press_key(&mut studio, Key::Char('s'), Mods::default());
        assert_eq!(studio.board_tool(), STICKY_TOOL, "S a sticky note, not the styles");
        pull(&mut studio, (700.0, 100.0), (850.0, 250.0), Mods::default());
        studio.commit_text();
        let shapes: Vec<Shape> = (0..3).map(|index| shape(&studio, index)).collect();
        assert!(shapes.iter().all(|shape| shape.sketch && shape.face == LETTERING && shape.font_px == WORDS_PX));
        assert_eq!((shapes[0].color, shapes[0].form, shapes[1].form, shapes[2].form), (INK, shapes::FORM_BOX, shapes::FORM_DIAMOND, shapes::FORM_STICKY));
        assert_eq!(shapes[2].fill, FILLS[0], "a note is yellow");
        // A diamond is drawn as one: its corners are empty paper.
        studio.selected.clear();
        assert!(matches!(shown(&mut studio, (404.0, 104.0)), shapes::PAPER | DOTS));
        assert_eq!(shown(&mut studio, (470.0, 150.0)), FILLS[2]);
        // Each drawn by hand the same wherever it is moved.
        let mut moved = shapes[0].clone();
        moved.translate(37.0, 11.0);
        let (mut a, mut b) = (vec![0u32; 300 * 200], vec![0u32; 300 * 200]);
        let mut first = shapes[0].clone();
        first.translate(-60.0, -60.0);
        moved.translate(-97.0, -71.0);
        first.draw(&mut Canvas::new(&mut a, 300, 200), &mut studio.text);
        moved.draw(&mut Canvas::new(&mut b, 300, 200), &mut studio.text);
        assert!(a == b, "the same wobble where it is moved to");
        // Its words, lettered, in its middle.
        let mut boxed = shapes[0].clone();
        boxed.text = "Cache".into();
        let blank = shapes[0].clone();
        let (x, y, w, h) = boxed.frame();
        let mut with = vec![0xFFFFFFu32; 600 * 400];
        let mut without = with.clone();
        boxed.draw(&mut Canvas::new(&mut with, 600, 400), &mut studio.text);
        blank.draw(&mut Canvas::new(&mut without, 600, 400), &mut studio.text);
        let differs = |x0: f32, x1: f32| (y as i32 + 10..(y + h) as i32 - 10).any(|py| (x0 as i32..x1 as i32).any(|px| with[(py * 600 + px) as usize] != without[(py * 600 + px) as usize]));
        assert!(differs(x + w * 0.3, x + w * 0.7) && !differs(x + 8.0, x + w * 0.15));
        // Kept in the file, with what it is drawn as.
        let back = Shape::read(&shapes[1].write()).unwrap();
        assert_eq!((back.form, back.sketch, back.face), (shapes::FORM_DIAMOND, true, LETTERING));
    }
}
