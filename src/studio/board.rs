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
/// The colors a board picks by name, each a line and the pale tint of it
/// a shape is filled with, so that one choice colors both: black, gray,
/// red, orange, yellow, green, teal, blue, violet and pink. On a dark
/// ground each is shown as its dark pair.
pub(super) const SWATCHES: [(u32, u32); 10] = [
    (0x1E1E1E, 0xF1F3F5),
    (0x868E96, 0xE9ECEF),
    (0xE03131, 0xFFE3E3),
    (0xF08C00, 0xFFE8CC),
    (0xF59F00, 0xFFF3BF),
    (0x2F9E44, 0xD3F9D8),
    (0x0C8599, 0xC5F6FA),
    (0x1971C2, 0xD0EBFF),
    (0x7048E8, 0xE5DBFF),
    (0xC2255C, 0xFFDEEB),
];
/// The names of the colors, as the panel says them.
pub(super) const SWATCH_NAMES: [&str; 10] = ["Black", "Gray", "Red", "Orange", "Yellow", "Green", "Teal", "Blue", "Violet", "Pink"];
/// The four sizes, S M L XL: how wide a line is and how large its words.
pub(super) const SIZES: [(f32, u32); 4] = [(1.5, 13), (2.0, 16), (3.5, 22), (6.0, 32)];
/// The heads an arrow on a board can have at either end, as its panel
/// offers them: none, open, filled, a dot, a diamond, a bar.
pub(super) const HEAD_CHOICES: [Option<u8>; 6] = [None, Some(shapes::OPEN_HEAD), Some(shapes::FILLED_HEAD), Some(shapes::DOT_HEAD), Some(shapes::DIAMOND_HEAD), Some(shapes::BAR_HEAD)];
pub(super) const FILLS: [u32; 5] = [0xFFF3BF, 0xD3F9D8, 0xD0EBFF, 0xFFE3E3, 0xE5DBFF];
#[cfg(test)]
const DOTS: u32 = 0xD8D6CF;
pub(super) const QUIET: u32 = 0x868E96;
/// The lettering a new board's words are in: Architects Daughter, by where
/// it is among the faces; and how large, and how wide a line is.
pub(super) const LETTERING: usize = 2;
pub(super) const WORDS_PX: u32 = 16;
/// The largest words can be typed as, in pixels.
pub(super) const MOST_PX: u32 = 999;
pub(super) const LINE: f32 = 2.0;
/// How far apart the dots of the paper are, on the page.
const DOT_STEP: f32 = 18.0;
/// The size a box clicked out with no drag is, before two have been
/// drawn the same; and a sticky note's.
pub(super) const BOX: (f32, f32) = (160.0, 72.0);
pub(super) const STICKY: (f32, f32) = (150.0, 150.0);

/// The tools along the bottom of a board, each with its key: the pointer,
/// the hand, a box, an oval, a diamond, an arrow, a line, the pen, text, a
/// sticky note, a picture, and the laser.
pub(super) const TOOLS: [(&str, char); 12] = [("Select", 'v'), ("Hand", 'h'), ("Box", 'r'), ("Oval", 'o'), ("Diamond", 'd'), ("Arrow", 'a'), ("Line", 'l'), ("Pen", 'p'), ("Text", 't'), ("Sticky note", 's'), ("Picture", 'i'), ("Laser", 'k')];
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
pub(super) const LASER_TOOL: u8 = 11;

/// What on a board is clicked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ctl {
    /// The switch at the top left: Board, or Design.
    Mode(bool),
    /// A tool of the board's bar.
    Tool(u8),
    /// The style panel: a color by name, which colors a line and its fill;
    /// what a fill is (none, paper, a tint or hatched); a line whole,
    /// dashed or dotted; a size, S M L XL; the look, the size of the
    /// words, guessing on or off, and the status dot.
    Color(u8),
    FillKind(u8),
    Dash(u8),
    Size(u8),
    /// A selected arrow's head at its start (0) or its end (1), one of
    /// `HEAD_CHOICES`; and the way it goes: curved, straight or elbowed.
    Head(u8, u8),
    Route(u8),
    Look,
    /// The font its words are in, a list of the faces, apart from the look.
    Font,
    /// What the board is on: light paper, dark gray or black.
    Ground,
    /// How large the words are: smaller (0), the number of pixels, which a
    /// click types into (1), and larger (2).
    Words(u8),
    Guess,
    Status,
    /// The history: its slider, Branch from here, and Back to now.
    HistoryTrack,
    Branch,
    Now,
    /// Present, at the top right.
    Present,
    /// Neaten, by the pen.
    Neaten,
    /// The zoom at the bottom left: smaller, how large, which a click
    /// makes 100%, larger, all of the board fitted, and its keys.
    ZoomOut,
    ZoomShown,
    ZoomIn,
    Fit,
    Keys,
    /// The sheet of the board's keys, while it is open, and all round it,
    /// which a click closes it on.
    Sheet,
    Scrim,
}

/// What a board keeps of its own, in the window rather than the file.
pub(super) struct Board {
    /// How the next box, oval, diamond, note, arrow or line is drawn: its
    /// line's color, its fill, whether by hand and how wide its line;
    /// carried on from the last one changed. How large its words are is
    /// the page's own.
    pub stroke: u32,
    pub fill: Option<u32>,
    pub sketch: bool,
    pub width: f32,
    /// What the box tool draws: a box, a diamond or a note.
    pub making: u8,
    /// The sizes of the last two boxes drawn, the latest last.
    pub sizes: Vec<(f32, f32)>,
    /// Whether the left panel is shown on a board: not until asked for.
    /// There is no right one: a board's styles are in its own panel.
    pub left: bool,
    /// Whether it guesses what comes next.
    pub guessing: bool,
    /// The box whose sides show a dot each to pull an arrow out of, by its
    /// layer: the one the pointer is over or near, or the one picked out.
    pub dots_of: Option<usize>,
    /// C pressed: the next key a color of the style bar.
    pub coloring: bool,
    /// How the next shape's fill and line are drawn: hatched, and whole,
    /// dashed or dotted.
    pub hatch: bool,
    /// The key of the tool in hand typed first into the box it drew, by
    /// that box's name: taken out if the next thing done is another box.
    pub tool_letter: Option<(u32, char)>,
    pub dash: u8,
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
    /// The trail, each point with when it was laid down and the stroke it
    /// is of, a stroke being one press of the button or of L; and how
    /// many strokes there have been.
    pub trail: Vec<((i32, i32), std::time::Instant, u32)>,
    pub strokes: u32,
    pub glow: Option<(u32, std::time::Instant)>,
    /// The laser is the tool in hand; and its button is held down, the
    /// pointer drawing its trail.
    pub laser_tool: bool,
    pub beam: bool,
    /// The sheet of its keys is open.
    pub keys: bool,
    /// What the box tool would draw round, outlined.
    pub pick: Option<pick::Pick>,
    /// What letting go of the box being moved would do to an arrow.
    pub drop: Option<change::Drop>,
    /// The SVG last brought in as shapes, to keep as a picture instead.
    pub svg: Option<svg::Brought>,
    /// How far apart a map's levels are laid out.
    pub level: f32,
    /// Whether the pen neatens what it draws: off until asked for.
    pub neaten: bool,
    /// What the end of an arrow being dragged would be tied to, glowing.
    pub bind: Option<usize>,
    /// Where an end dragged, or each end of an arrow moved, would be put
    /// on a box, by its layer: the middle of a side, lit.
    pub ports: Vec<(usize, (f32, f32))>,
    /// A box typed into was moved or sized: the next letter typed is a
    /// step of its own to undo.
    pub type_step: bool,
    /// Esc has just ended the typing of the box picked out: a letter is a
    /// tool's, not typed over its words.
    pub typed: Option<usize>,
    /// Where in the words being typed the caret is, none being their end;
    /// and the other end of the words picked out by a drag, if any are.
    pub caret: Option<usize>,
    pub anchor: Option<usize>,
    /// − or + of the size of the words held down: which, and when it next
    /// steps again by itself.
    pub stepping: Option<(bool, std::time::Instant)>,
    /// The size of the words being typed into the bar, once its number is
    /// clicked.
    pub px_typed: Option<String>,
    /// How far the wheel has gone over that number toward the next step,
    /// and when it last stepped, for a run of it to be one step to undo.
    pub wheeled: (f32, Option<std::time::Instant>),
}

impl Board {
    pub(super) fn new(_board: bool) -> Board {
        Board { stroke: INK, fill: Some(FILLS[2]), sketch: true, width: LINE, making: shapes::FORM_BOX, sizes: Vec::new(), left: false, guessing: true, dots_of: None, coloring: false, hatch: false, tool_letter: None, dash: 0, carried: Vec::new(), letting_go: false, ghost: None, branching: None, history: None, focus: None, present: None, laser: None, trail: Vec::new(), strokes: 0, glow: None, laser_tool: false, beam: false, keys: false, pick: None, drop: None, svg: None, level: 80.0, neaten: false, bind: None, ports: Vec::new(), type_step: false, typed: None, caret: None, anchor: None, stepping: None, px_typed: None, wheeled: (0.0, None) }
    }
}

mod bar;
mod bind;
mod caret;
mod dots;
mod nest;
mod map;
mod ghost;
mod change;
mod explain;
mod pick;
pub(super) mod svg;
mod export;
mod mermaid;
mod neaten;
mod zoom;
mod keys;
/// Whether a frame is the one a new page of frames begins with, as it
/// began: called Frame 1, where it was put, its size, white, empty, and
/// exported as it was.
fn untouched(frame: &Doc) -> bool {
    let new = Doc::new(1200, 675);
    frame.name == new.name && (frame.x, frame.y, frame.width, frame.height) == (new.x, new.y, new.width, new.height) && frame.background == new.background && frame.layers.is_empty() && frame.stacks.is_empty() && frame.exports == new.exports
}

/// A soft band of a color along a way through points of the window, as
/// far out from it each side as `wide`, `amount`/256 of it, its edges
/// smooth; each pixel laid once, however many pieces of the way pass it.
pub(super) fn halo(canvas: &mut Canvas, path: &[(f32, f32)], wide: f32, color: u32, amount: u32) {
    if path.is_empty() {
        return;
    }
    let reach = wide + 1.0;
    let clip = canvas.clip();
    let left = (path.iter().map(|p| p.0).fold(f32::MAX, f32::min) - reach).floor().max(clip.x as f32) as i32;
    let top = (path.iter().map(|p| p.1).fold(f32::MAX, f32::min) - reach).floor().max(clip.y as f32) as i32;
    let right = (path.iter().map(|p| p.0).fold(f32::MIN, f32::max) + reach).ceil().min(clip.right() as f32) as i32;
    let bottom = (path.iter().map(|p| p.1).fold(f32::MIN, f32::max) + reach).ceil().min(clip.bottom() as f32) as i32;
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 {
        return;
    }
    let mut cover = vec![0u8; (w * h) as usize];
    let pieces: Vec<((f32, f32), (f32, f32))> = match path.len() {
        1 => vec![(path[0], path[0])],
        _ => path.windows(2).map(|pair| (pair[0], pair[1])).collect(),
    };
    for (a, b) in pieces {
        let x0 = ((a.0.min(b.0) - reach).floor() as i32).max(left);
        let x1 = ((a.0.max(b.0) + reach).ceil() as i32).min(right);
        let y0 = ((a.1.min(b.1) - reach).floor() as i32).max(top);
        let y1 = ((a.1.max(b.1) + reach).ceil() as i32).min(bottom);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let long = dx * dx + dy * dy;
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = if long > 0.0 { (((px - a.0) * dx + (py - a.1) * dy) / long).clamp(0.0, 1.0) } else { 0.0 };
                let away = (px - a.0 - t * dx).hypot(py - a.1 - t * dy);
                let on = ((wide + 0.5 - away).clamp(0.0, 1.0) * 255.0) as u8;
                let cell = &mut cover[((y - top) * w + (x - left)) as usize];
                *cell = (*cell).max(on);
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let on = u32::from(cover[(y * w + x) as usize]);
            if on > 0 {
                canvas.blend(left + x, top + y, color, ((on * amount) / 256).min(255) as u8);
            }
        }
    }
}

/// A rectangle's edge in dashes a pixel wide, as a frame of a board is
/// drawn: 6 pixels on and 5 off, from each corner.
pub(super) fn dashed_rect(canvas: &mut Canvas, rect: Rect, color: u32) {
    let (on, step) = (6, 11);
    for side in [(rect.x, rect.y, rect.w, true), (rect.x, rect.bottom() - 1, rect.w, true), (rect.x, rect.y, rect.h, false), (rect.right() - 1, rect.y, rect.h, false)] {
        let (x, y, long, across) = side;
        for start in (0..long.max(0)).step_by(step) {
            let run = on.min(long - start);
            match across {
                true => canvas.fill(x + start, y, run, 1, color),
                false => canvas.fill(x, y + start, 1, run, color),
            }
        }
    }
}

/// Whether some text is Mermaid a board can read.
pub(super) fn is_mermaid(text: &str) -> bool {
    mermaid::kind_of(text).is_some()
}
pub(super) mod look;
pub use look::board_preview;
mod act;
pub(super) use act::Act;
mod typing;

/// Whether the last canvas switched to Board, kept in `canvas-mode` among
/// hotshot's state, for a new file to begin as one.
pub(super) fn last_was_board() -> bool {
    crate::output::state_file("canvas-mode").and_then(|file| std::fs::read_to_string(file).ok()).is_some_and(|kept| kept.trim() == "board")
}

pub(super) fn keep_mode(board: bool) {
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
    look::first_look(&mut page);
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
        // Switched straight back with nothing done, the frame nobody had
        // touched is back as it was.
        let parked = self.parked.take().filter(|(page, _, version)| !board && *page == self.page && *version == self.version);
        self.checkpoint();
        self.pages[self.page].board = board;
        if board {
            look::first_look(&mut self.pages[self.page]);
        }
        keep_mode(board);
        if board {
            self.activate(LOOSE);
        }
        // On a board, the frame a new page of frames begins with, which
        // nobody has touched, is gone; any other is the part of the board
        // to export, drawn as a dashed edge with what is in it as it was.
        let mut gone = None;
        if board {
            if let Some(at) = self.pages[self.page].frames.iter().position(untouched) {
                gone = Some(self.pages[self.page].frames.remove(at));
            }
        }
        let areas: Vec<String> = match board {
            true => self.pages[self.page].frames.iter().map(|frame| frame.name.clone()).collect(),
            false => Vec::new(),
        };
        if let Some((_, frame, _)) = parked {
            self.pages[self.page].frames.insert(0, frame);
            self.keep_order();
        }
        (self.tool, self.framing) = (Tool::Select, false);
        self.selected.clear();
        self.shown.clear();
        self.shown_page.clear();
        self.changed();
        if let Some(frame) = gone {
            self.parked = Some((self.page, frame, self.version));
        }
        self.say(match (board, &areas[..]) {
            (true, []) => "Board: for explaining".to_owned(),
            (true, [name]) => format!("{name} is now an export area on this board"),
            (true, names) => format!("{} frames are now export areas on this board", names.len()),
            (false, _) => "Design: for pixels".to_owned(),
        });
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
            layout.controls.push((Rect::new(beside.right() - 96, beside.y - 1, 96, 30), Control::Board(Ctl::Present)));
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
        self.layout_zoom(layout);
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
            Tool::Select if self.board.laser_tool => LASER_TOOL,
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
            // The laser: a drag draws a trail that fades, and nothing is
            // picked out or moved while it is in hand.
            LASER_TOOL => {
                self.set_tool(Tool::Select);
                self.selected.clear();
                self.board.laser_tool = true;
                return Outcome::Continue;
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
            Ctl::ZoomOut | Ctl::ZoomShown | Ctl::ZoomIn | Ctl::Fit | Ctl::Keys | Ctl::Sheet | Ctl::Scrim => self.press_zoom(ctl),
            other => self.press_style(other),
        }
        Outcome::Continue
    }

    /// Ctrl+M: the page shown the other kind.
    pub(super) fn flip_mode(&mut self) {
        let board = !self.on_board();
        self.set_mode(board);
    }

    /// How much larger on the page a shape drawn now is than at its own
    /// size, for it to look as large in the window: one at 100%, four at
    /// 25%; none past an eighth or eight times.
    pub(super) fn dynamic(&self) -> f32 {
        (1.0 / self.zoom.max(1e-3)).clamp(0.125, 8.0)
    }

    /// A shape about to be drawn on a board, made as a board draws one: by
    /// hand, in its ink and fill, its words in the board's font; a box the
    /// kind the box tool makes; an arrow routed between what it joins.
    pub(super) fn board_dress(&self, shape: &mut Shape) {
        let board = &self.board;
        // As large to the eye as at its own size, whatever the zoom it is
        // drawn at: drawn zoomed out, its line and words larger on the
        // page, as tldraw's dynamic size has them.
        let by = self.dynamic();
        (shape.color, shape.stroke, shape.face, shape.font_px) = (board.stroke, board.width * by, self.board_font(), ((self.board_px() as f32 * by).round() as u32).max(1));
        if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Arrow | Tool::Line) {
            (shape.dashed, shape.dotted) = (board.dash == 1, board.dash == 2);
        }
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                (shape.sketch, shape.border, shape.radius) = (board.sketch, true, 6.0);
                (shape.filled, shape.fill) = (board.fill.is_some(), board.fill.unwrap_or(0xFFFFFF));
                shape.hatch = board.hatch && shape.filled;
                // Its words fitted to it, at the board's size or smaller.
                shape.nominal = shape.font_px;
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
            // The pen smooth, a little thinner at either end.
            Tool::Pen => shape.sketch = board.sketch,
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
                // Each from the icon font, drawn in the same line as the
                // rest of hotshot's icons.
                let icon = match which {
                    HAND => icons::HAND,
                    BOX_TOOL => icons::RECT,
                    OVAL => icons::ELLIPSE,
                    DIAMOND => icons::DIAMOND,
                    ARROW => icons::ARROW,
                    LINE_TOOL => icons::LINE,
                    PEN => icons::PEN,
                    TEXT => icons::TEXT,
                    STICKY_TOOL => icons::STICKY,
                    PICTURE => icons::PICTURE,
                    // A beam of light: the font has no laser of its own.
                    LASER_TOOL => icons::SPOTLIGHT,
                    _ => icons::SELECT,
                };
                self.text.draw_centered(canvas, cx, cy, 17, color, icon);
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
            Ctl::ZoomOut | Ctl::ZoomShown | Ctl::ZoomIn | Ctl::Fit | Ctl::Keys => self.draw_zoom_control(canvas, rect, ctl, hovered),
            // Drawn over everything, once the rest is.
            Ctl::Sheet | Ctl::Scrim => {}
            other => self.draw_style_control(canvas, rect, other, hovered),
        }
    }

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

    /// A page of frames, as a new file is when Design was last used.
    fn design() -> Studio {
        let mut studio = Studio::with_pages(vec![Page::new("Page 1", vec![Doc::new(1200, 675)])], Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (0.5, (100.0, 100.0));
        studio
    }

    #[test]
    fn the_frame_nobody_touched_goes_as_a_page_becomes_a_board_and_comes_back_if_nothing_was_done() {
        let mut studio = design();
        assert_eq!(studio.frame_count(), 1);
        studio.set_mode(true);
        assert_eq!(studio.frame_count(), 0, "a board begins with no frame");
        studio.set_mode(false);
        assert_eq!((studio.frame_count(), studio.pages[0].frames[0].name.as_str()), (1, "Frame 1"), "back as it was");
        // Something done on the board, it stays gone.
        studio.set_mode(true);
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.set_mode(false);
        assert_eq!(studio.frame_count(), 0);
        // A new page chosen as a board begins with none; as Design, with one.
        studio.new_page_of(true);
        assert!(studio.on_board() && studio.frame_count() == 0);
        studio.new_page_of(false);
        assert!(!studio.on_board() && studio.frame_count() == 1);
    }

    #[test]
    fn a_frame_on_a_board_is_an_export_area_drawn_dashed_with_all_that_is_in_it_and_no_background() {
        let mut studio = design();
        studio.doc.name = "Hero".into();
        // A box in the frame, half out past its left edge.
        let mut inside = Shape::new(Tool::Rect, 0x1971C2, 2.0, (-60.0, 100.0));
        inside.points[1] = (60.0, 200.0);
        (inside.filled, inside.fill) = (true, 0x1971C2);
        studio.doc.layers.push(Layer::shape(inside));
        studio.changed();
        studio.set_mode(true);
        assert_eq!(studio.frame_count(), 1, "kept, as the part to export");
        assert_eq!(studio.note.as_ref().map(|(said, _)| said.as_str()), Some("Hero is now an export area on this board"));
        // No white of its own: the board's paper shows in it.
        assert!(matches!(shown(&mut studio, (600.0, 500.0)), shapes::PAPER | DOTS));
        // What is in it is shown whole, past its edge too, where it was.
        assert_eq!(shown(&mut studio, (-30.0, 150.0)), 0x1971C2);
        assert_eq!(shown(&mut studio, (30.0, 150.0)), 0x1971C2);
        // Exported, it is exactly its part of the board, with what is in it.
        studio.activate(0);
        let (layers, part, name) = studio.export_part().unwrap();
        assert_eq!((part, name.as_str()), ((0.0, 0.0, 1200.0, 675.0), "Hero"));
        assert!(layers.iter().any(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.fill == 0x1971C2 && shape.frame().0 == -60.0)));
        // A page of frames again: a frame with what is in it inside it.
        studio.set_mode(false);
        assert_eq!((studio.frame_count(), studio.pages[0].frames.first().map_or(studio.doc.layers.len(), |frame| frame.layers.len().max(studio.doc.layers.len()))), (1, 1));
        assert_ne!(shown(&mut studio, (-30.0, 150.0)), 0x1971C2, "cut off at its edge again");
        assert_eq!(shown(&mut studio, (600.0, 500.0)), 0xFFFFFF, "on its white again");
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
        // The left panel, hidden, is one key away, as in Design; there is no
        // right one, nor its button, the board's styles in its own panel.
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, shift: true, ..Mods::default() });
        assert!(studio.shows_left());
        press_key(&mut studio, Key::Char('\\'), Mods { ctrl: true, alt: true, ..Mods::default() });
        assert!(!studio.shows_right());
        studio.layout();
        assert!(studio.control_rect(Control::SidePanel(1)).is_none(), "no button for it");
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
