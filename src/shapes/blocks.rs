//! What a board's box can hold besides words: a block of code, dark and
//! rounded under a slim strip with three dots and the name of its file,
//! its lines in a monospaced face colored by what each word is; a table
//! of cells, its columns as wide as their widest cell and its first row a
//! tint of its color; and the stamps a box can be given, round badges at
//! its top right.

use super::*;

/// The face a block of code is in: Fira Code, by where it is among them.
pub const MONO: usize = 3;

/// A block of code's colors, as One Dark has them: its body, its strip and
/// the rule under that; and its words, plain, a keyword, a type, a call, a
/// string, a number and a comment.
const CODE_BODY: u32 = 0x282C34;
const CODE_STRIP: u32 = 0x21252B;
const CODE_RULE: u32 = 0x1A1D22;
const CODE_PLAIN: u32 = 0xABB2BF;
const CODE_KEYWORD: u32 = 0xC678DD;
const CODE_TYPE: u32 = 0xE5C07B;
const CODE_CALL: u32 = 0x61AFEF;
const CODE_STRING: u32 = 0x98C379;
const CODE_NUMBER: u32 = 0xD19A66;
const CODE_COMMENT: u32 = 0x7F848E;
/// Its file's name, and what it says before one is typed.
const CODE_NAME: u32 = 0x9DA5B4;
const CODE_UNNAMED: u32 = 0x5C6370;
/// The three dots at the left of its strip: red, amber and green.
const LIGHTS: [u32; 3] = [0xFF5F57, 0xFEBC2E, 0x28C840];
/// How many letters across a block of code is at least.
const CODE_LEAST: usize = 24;

/// The words a line of code is colored as keywords by, of the languages
/// most pasted: Rust, Python, JavaScript and TypeScript, Go, C and Java,
/// shell and SQL.
const KEYWORDS: [&str; 96] = [
    "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "crate", "def", "default", "defer", "del", "do", "dyn", "elif", "else", "enum", "except", "export", "extends", "extern", "false", "finally", "fn", "for", "from", "func", "function", "go", "if", "impl", "import", "in", "interface", "is", "lambda", "let", "loop", "match", "mod", "move", "mut", "new", "nil", "None", "not", "null", "or", "and", "package", "pass", "private", "protected", "pub", "public", "raise", "abstract", "ref", "return", "select", "self", "Self", "static", "struct", "super", "switch", "this", "throw", "trait", "true", "True", "False", "try", "type", "typeof", "undefined", "unsafe", "use", "var", "void", "where", "while", "with", "yield", "chan", "delete", "echo", "then", "fi", "done", "SELECT", "FROM", "WHERE", "JOIN",
];

/// The stamps a box can be given, each what it is called and its color:
/// yes, a green check; add, a blue plus; a question, an orange mark; no, a
/// red cross; and love, a violet heart.
pub const STAMPS: [(&str, u32); 5] = [("Yes", 0x2F9E44), ("Add", 0x1971C2), ("Question", 0xF08C00), ("No", 0xE03131), ("Love", 0x7048E8)];

/// A block of code's words as the name of its file, its first line, and
/// its lines of code, the rest.
pub fn code_parts(words: &str) -> (&str, &str) {
    words.split_once('\n').unwrap_or((words, ""))
}

/// A table's words as its rows, each its cells.
pub fn grid_rows(words: &str) -> Vec<Vec<&str>> {
    words.split('\n').map(|row| row.split('\t').collect()).collect()
}

/// Whether some words pasted read as code: two lines or more, enough of
/// them ending or beginning as code does, or indented under one that does.
pub fn looks_like_code(words: &str) -> bool {
    let lines: Vec<&str> = words.lines().filter(|line| !line.trim().is_empty()).collect();
    if lines.len() < 2 {
        return false;
    }
    let starts = ["fn ", "pub ", "let ", "const ", "def ", "class ", "import ", "from ", "return ", "if (", "for (", "while (", "function ", "var ", "#include", "use ", "func ", "package ", "SELECT ", "struct ", "impl ", "export ", "async ", "} else", "//", "/*", "# ", "@"];
    let ends = [";", "{", "}", "):", "=>", "};", ");", "*/"];
    let coded = lines.iter().filter(|line| {
        let line = line.trim();
        starts.iter().any(|start| line.starts_with(start)) || ends.iter().any(|end| line.ends_with(end)) || line.contains(" = ") && line.contains(['(', '[', '{', '"'])
    }).count();
    let indented = lines.iter().filter(|line| line.starts_with("    ") || line.starts_with('\t')).count();
    coded * 3 >= lines.len() && (coded >= 2 || indented >= 1)
}

/// Whether some words pasted are rows of a spreadsheet: two lines or more,
/// each the same number of cells parted by tabs, or by commas where no
/// line has a tab.
pub fn looks_tabular(words: &str) -> bool {
    let rows = tabular_rows(words);
    rows.len() >= 2 && rows[0].len() >= 2 && rows.iter().all(|row| row.len() == rows[0].len())
}

/// The cells of rows pasted, by tabs, or by commas as CSV has them, with
/// a cell in quotes kept whole and two quotes in it one.
pub fn tabular_rows(words: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = words.lines().filter(|line| !line.trim().is_empty()).collect();
    let tabbed = lines.iter().any(|line| line.contains('\t'));
    lines.iter().map(|line| match tabbed {
        true => line.split('\t').map(|cell| cell.trim().to_owned()).collect(),
        false => {
            let (mut cells, mut cell, mut quoted, mut letters) = (Vec::new(), String::new(), false, line.chars().peekable());
            while let Some(letter) = letters.next() {
                match letter {
                    '"' if quoted && letters.peek() == Some(&'"') => {
                        cell.push('"');
                        letters.next();
                    }
                    '"' => quoted = !quoted,
                    ',' if !quoted => cells.push(std::mem::take(&mut cell).trim().to_owned()),
                    other => cell.push(other),
                }
            }
            cells.push(cell.trim().to_owned());
            cells
        }
    }).collect()
}

/// A line of code in runs of what each part is, each its start, its end
/// and its color: a comment to the end of the line, or what of one begun
/// on a line before is in it; a string; a number; a word, a keyword, a
/// type, a call or plain; and the marks between. Whether a comment is
/// open at its end is carried on to the next.
pub fn code_runs(line: &str, open: &mut bool) -> Vec<(usize, usize, u32)> {
    let mut runs = Vec::new();
    let letters: Vec<(usize, char)> = line.char_indices().collect();
    let end_of = |at: usize| letters.get(at).map_or(line.len(), |&(offset, _)| offset);
    let mut at = 0;
    let rest = |at: usize| &line[end_of(at)..];
    while at < letters.len() {
        let (start, letter) = letters[at];
        if *open {
            let close = rest(at).find("*/").map(|found| start + found + 2);
            *open = close.is_none();
            let end = close.unwrap_or(line.len());
            runs.push((start, end, CODE_COMMENT));
            at = letters.iter().position(|&(offset, _)| offset >= end).unwrap_or(letters.len());
            continue;
        }
        let after = rest(at);
        if after.starts_with("/*") {
            *open = true;
            continue;
        }
        let trimmed = line.trim_start();
        let directive = ["#include", "#define", "#if", "#endif", "#pragma", "#import", "#else"].iter().any(|word| trimmed.starts_with(word));
        let hash = letter == '#' && !after.starts_with("#[") && !after.starts_with("#!") && !directive;
        if after.starts_with("//") || hash || after.starts_with("-- ") {
            runs.push((start, line.len(), CODE_COMMENT));
            break;
        }
        if matches!(letter, '"' | '\'' | '`') {
            // Closed on the line by the same mark not after a backslash;
            // a lone quote, as Rust's lifetimes are, only a mark.
            let mut close = None;
            let mut escaped = false;
            for (along, &(offset, next)) in letters.iter().enumerate().skip(at + 1) {
                match next {
                    '\\' if !escaped => escaped = true,
                    _ if next == letter && !escaped => {
                        close = Some((along, offset + next.len_utf8()));
                        break;
                    }
                    _ => escaped = false,
                }
            }
            match close {
                Some((along, end)) => {
                    runs.push((start, end, CODE_STRING));
                    at = along + 1;
                }
                None if letter == '\'' => {
                    runs.push((start, start + 1, CODE_PLAIN));
                    at += 1;
                }
                None => {
                    runs.push((start, line.len(), CODE_STRING));
                    break;
                }
            }
            continue;
        }
        let word_letter = |c: char| c.is_alphanumeric() || c == '_';
        let after_word = at > 0 && word_letter(letters[at - 1].1);
        if letter.is_ascii_digit() && !after_word {
            let mut along = at;
            while along < letters.len() && (word_letter(letters[along].1) || letters[along].1 == '.' && letters.get(along + 1).is_some_and(|&(_, next)| next.is_ascii_digit())) {
                along += 1;
            }
            runs.push((start, end_of(along), CODE_NUMBER));
            at = along;
            continue;
        }
        if word_letter(letter) {
            let mut along = at;
            while along < letters.len() && word_letter(letters[along].1) {
                along += 1;
            }
            let (end, word) = (end_of(along), &line[start..end_of(along)]);
            let next = line[end..].trim_start().chars().next();
            let color = match word {
                _ if KEYWORDS.contains(&word) || directive && start == line.len() - trimmed.len() + 1 => CODE_KEYWORD,
                _ if matches!(next, Some('(' | '!')) => CODE_CALL,
                _ if word.chars().next().is_some_and(char::is_uppercase) && word.chars().any(char::is_lowercase) => CODE_TYPE,
                _ => CODE_PLAIN,
            };
            runs.push((start, end, color));
            at = along;
            continue;
        }
        // A directive's hash, as the word after it.
        let color = if letter == '#' && directive { CODE_KEYWORD } else { CODE_PLAIN };
        runs.push((start, start + letter.len_utf8(), color));
        at += 1;
    }
    runs
}

/// Where the parts of a table are, fitted to its frame: the left of each
/// column, the top of each row and how high it is, the
/// room inside a cell either side, and how high a line of its words is.
pub struct Grid {
    pub lefts: Vec<f32>,
    pub tops: Vec<f32>,
    pub heights: Vec<f32>,
    pub pad: f32,
    pub high: i32,
}

impl Shape {
    /// How far in a block of code's lines are from its sides, how high its
    /// strip is, and how high a line is.
    fn code_room(&self) -> (f32, f32, i32) {
        let px = self.font_px as f32;
        let high = Text::line_height(self.font_px);
        ((px * 0.95).max(6.0), (high as f32 + px * 0.75).round(), high)
    }

    /// The three dots of its strip: the middle of each, and how large.
    fn code_lights(&self) -> ([Point; 3], f32) {
        let (x, y, _, _) = self.frame();
        let (pad, strip, _) = self.code_room();
        let radius = (self.font_px as f32 * 0.22).max(2.0);
        let first = x + pad * 0.75 + radius;
        ([0.0, 1.0, 2.0].map(|at| (first + at * radius * 3.3, y + strip / 2.0)), radius)
    }

    /// How large a block of code needs to be for all of its lines, none of
    /// them wrapped: as wide as the longest, and never narrower than a few
    /// words, nor than its strip needs for its name.
    pub fn code_needs(&self, text: &mut Text) -> (f32, f32) {
        let (pad, strip, high) = self.code_room();
        let (name, code) = code_parts(&self.text);
        let lines: Vec<&str> = code.split('\n').collect();
        let longest = lines.iter().map(|line| text.width_in(MONO, line, self.font_px)).max().unwrap_or(0) as f32;
        let least = text.width_in(MONO, &"0".repeat(CODE_LEAST), self.font_px) as f32;
        let (lights, radius) = self.code_lights();
        let lit = lights[2].0 + radius - self.frame().0;
        let named = text.width_in(MONO, name, self.font_px) as f32 + 2.0 * (lit + pad * 0.8);
        let wide = (longest.max(least) + 2.0 * pad).max(named);
        (wide.ceil(), (strip + pad * 1.6 + (lines.len() as i32 * high) as f32).ceil())
    }

    /// A block of code's lines as they are shown, each with where in its
    /// words it begins, its left, its top and how wide it is: its name in
    /// the middle of its strip, clear of the dots, and its lines of code
    /// under that from the left.
    pub fn code_lines(&self, text: &mut Text) -> Vec<(usize, String, f32, f32, i32)> {
        let (x, y, w, _) = self.frame();
        let (pad, strip, high) = self.code_room();
        let (name, code) = code_parts(&self.text);
        let (lights, radius) = self.code_lights();
        let named = text.width_in(MONO, name, self.font_px);
        let left = (x + (w - named as f32) / 2.0).max(lights[2].0 + radius + pad * 0.8).round();
        let mut lines = vec![(0, name.to_owned(), left, (y + (strip - high as f32) / 2.0).round(), named)];
        let mut start = name.len() + 1;
        for (at, line) in code.split('\n').enumerate() {
            let top = (y + strip + pad * 0.8).round() + (at as i32 * high) as f32;
            lines.push((start, line.to_owned(), (x + pad).round(), top, text.width_in(MONO, line, self.font_px)));
            start += line.len() + 1;
        }
        lines
    }

    /// A block of code: its body dark, the same in every look but for its
    /// edge, which is the look's own box: gone round by hand, its corners a
    /// little off, in the hand-drawn ones, and clean in the rest; its strip,
    /// its dots, its name, and its lines colored word by word.
    pub(super) fn draw_code(&self, canvas: &mut Canvas, text: &mut Text) {
        let (x, y, w, _) = self.frame();
        let (_, strip, _) = self.code_room();
        let (xi, yi, wi) = (x.round() as i32, y.round() as i32, w.round() as i32);
        let opacity = self.opacity;
        let body = self.inside_mask();
        canvas.mask(&body, CODE_BODY, opacity);
        // The strip, rounded at the top only, and a rule under it.
        let band = strip.round() as i32;
        let old = canvas.clip();
        canvas.set_clip(Rect::new(xi - 8, yi - 8, wi + 16, band + 8).intersect(old));
        canvas.mask(&body, CODE_STRIP, opacity);
        canvas.set_clip(old);
        canvas.set_clip(Rect::new(xi - 8, yi + band, wi + 16, self.scale.max(1e-3).round().max(1.0) as i32).intersect(old));
        canvas.mask(&body, CODE_RULE, opacity);
        canvas.set_clip(old);
        let (lights, dot) = self.code_lights();
        for (at, &(cx, cy)) in lights.iter().enumerate() {
            let mut light = Mask { bounds: Rect::new((cx - dot - 2.0).floor() as i32, (cy - dot - 2.0).floor() as i32, (dot * 2.0 + 4.0).ceil() as i32 + 1, (dot * 2.0 + 4.0).ceil() as i32 + 1), alpha: Vec::new() };
            light.alpha = vec![0; (light.bounds.w * light.bounds.h).max(0) as usize];
            stamp(&mut light, (cx, cy), (cx, cy), dot);
            canvas.mask(&light, LIGHTS[at], opacity);
        }
        let lines = self.code_lines(text);
        text.look = Look { opacity, ..Look::default() };
        let mut open = false;
        for (at, (_, line, left, top, _)) in lines.into_iter().enumerate() {
            let (left, top) = (left as i32, top as i32);
            if at == 0 {
                match line.is_empty() {
                    // Nothing named yet: what to type there, while it is typed.
                    true if self.editing => {
                        let wide = text.width_in(MONO, "untitled", self.font_px) as f32;
                        text.draw_in(MONO, canvas, (x + w / 2.0 - wide / 2.0).round() as i32, top, self.font_px, CODE_UNNAMED, "untitled");
                    }
                    true => {}
                    false => text.draw_in(MONO, canvas, left, top, self.font_px, CODE_NAME, &line),
                }
                continue;
            }
            for (start, end, color) in code_runs(&line, &mut open) {
                let along = text.width_in(MONO, &line[..start], self.font_px);
                text.draw_in(MONO, canvas, left + along, top, self.font_px, color, &line[start..end]);
            }
        }
        text.look = Look::default();
    }

    /// How wide each of a table's columns needs to be for its widest cell,
    /// and how high a row is.
    fn grid_needs_each(&self, text: &mut Text) -> (Vec<f32>, f32) {
        let px = self.font_px as f32;
        let pad = (px * 0.7).max(5.0);
        let high = Text::line_height(self.font_px) as f32;
        let mut widths: Vec<f32> = Vec::new();
        for row in grid_rows(&self.text) {
            for (at, cell) in row.iter().enumerate() {
                let wide = (text.width_in(self.face, cell, self.font_px) as f32 + 2.0 * pad).max(px * 4.5);
                match widths.get_mut(at) {
                    Some(known) => *known = known.max(wide),
                    None => widths.push(wide),
                }
            }
        }
        (widths.into_iter().map(f32::ceil).collect(), (high + px * 0.9).round())
    }

    /// How large a table needs to be for all of its cells.
    pub fn grid_needs(&self, text: &mut Text) -> (f32, f32) {
        let (widths, row) = self.grid_needs_each(text);
        (widths.iter().sum::<f32>(), row * self.text.split('\n').count() as f32)
    }

    /// A table laid out in its frame: its columns and rows as they need to
    /// be, what room is over shared out among them evenly, and what is short
    /// taken from each as it is wide.
    pub fn grid(&self, text: &mut Text) -> Grid {
        let (x, y, w, h) = self.frame();
        let (widths, _) = self.grid_needs_each(text);
        let rows = self.text.split('\n').count().max(1);
        let need = widths.iter().sum::<f32>().max(1.0);
        let widths: Vec<f32> = match w >= need {
            true => widths.iter().map(|wide| wide + (w - need) / widths.len().max(1) as f32).collect(),
            false => widths.iter().map(|wide| wide * w / need).collect(),
        };
        let height = (h / rows as f32).max(1.0);
        let lefts = widths.iter().scan(x, |left, wide| {
            let at = *left;
            *left += wide;
            Some(at)
        }).collect();
        Grid { lefts, tops: (0..rows).map(|at| y + at as f32 * height).collect(), heights: vec![height; rows], pad: (self.font_px as f32 * 0.7).max(5.0), high: Text::line_height(self.font_px) }
    }

    /// A table's cells as they are shown, in the order of its words: each
    /// with where in them it begins, its left, its top and how wide it is.
    pub fn grid_lines(&self, text: &mut Text) -> Vec<(usize, String, f32, f32, i32)> {
        let grid = self.grid(text);
        let mut cells = Vec::new();
        let mut start = 0;
        for (row, line) in self.text.split('\n').enumerate() {
            for (column, cell) in line.split('\t').enumerate() {
                let (left, top) = (grid.lefts.get(column).copied().unwrap_or(0.0) + grid.pad, grid.tops[row] + (grid.heights[row] - grid.high as f32) / 2.0);
                cells.push((start, cell.to_owned(), left.round(), top.round(), text.width_in(self.face, cell, self.font_px)));
                start += cell.len() + 1;
            }
        }
        cells
    }

    /// What a table's cells are filled with: under its first row, a white
    /// a hair off the paper on a dark one; its first row a tint of its line,
    /// its fill where it has one.
    fn grid_fills(&self) -> (u32, u32) {
        let body = if self.paper == 0 { 0xFFFFFF } else { crate::canvas::mix(self.paper, 0xFFFFFF, 18) };
        let head = match self.filled && self.fill != 0xFFFFFF {
            true => self.fill,
            false => crate::canvas::mix(body, self.color, 46),
        };
        (body, head)
    }

    /// A table: its cells filled, its first row tinted, the rules between
    /// its rows and columns drawn by hand in the hand-drawn looks and clean
    /// in the others, its edge over them, and its words in each cell.
    pub(super) fn draw_grid(&self, canvas: &mut Canvas, text: &mut Text) {
        let grid = self.grid(text);
        let (x, y, w, h) = self.frame();
        let (body, head) = self.grid_fills();
        let inside = self.inside_mask();
        canvas.mask(&inside, body, self.opacity);
        let old = canvas.clip();
        let band = Rect::new(x.floor() as i32 - 4, y.floor() as i32 - 4, w.ceil() as i32 + 8, (grid.tops[0] + grid.heights[0] - y).round() as i32 + 4);
        canvas.set_clip(band.intersect(old));
        canvas.mask(&inside, head, self.opacity);
        canvas.set_clip(old);
        // The rules: between the columns faint, between the rows fainter
        // but under the first, which is as strong as its edge is faint.
        let half = (self.stroke * 0.35).max(0.5 * self.scale.max(1e-3));
        let mut rules: Vec<(Point, Point, u32)> = Vec::new();
        for &left in grid.lefts.iter().skip(1) {
            rules.push(((left, y), (left, y + h), 150));
        }
        for (at, &top) in grid.tops.iter().enumerate().skip(1) {
            rules.push(((x, top), (x + w, top), if at == 1 { 230 } else { 110 }));
        }
        let reach = Rect::new(x.floor() as i32 - 4, y.floor() as i32 - 4, w.ceil() as i32 + 8, h.ceil() as i32 + 8);
        let (reach, _) = seen_part(reach);
        for (at, (a, b, strength)) in rules.into_iter().enumerate() {
            let mask = match self.sketched() {
                true => {
                    let path = vec![a, ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), b];
                    lay_inks(reach, &[self.ink(&path, false, half, 0x3C6E_F372 ^ (at as u32).wrapping_mul(0x9E37_79B9))])
                }
                false => {
                    let mut mask = Mask { bounds: reach, alpha: vec![0; (reach.w * reach.h).max(0) as usize] };
                    stamp(&mut mask, a, b, half);
                    mask
                }
            };
            canvas.mask(&mask, self.color, self.opacity * strength / 256);
        }
        if self.border {
            canvas.mask(&self.mask(), self.color, self.opacity);
        }
        // Its words: the first row's a deep shade of its line, the rest in
        // what reads on white; what the look sets on light paper, and what
        // reads on each on dark.
        let ink = |first: bool| match (self.words, first) {
            (words, _) if words != 0 && self.paper == 0 => words,
            (_, true) if self.paper == 0 => crate::canvas::mix(0x1E1E1E, self.color, 90),
            (_, true) => ink_on(head),
            _ => ink_on(body),
        };
        let first = self.text.split('\n').next().map_or(0, |row| row.split('\t').count());
        text.look = Look { opacity: self.opacity, ..Look::default() };
        for (at, (_, cell, left, top, _)) in self.grid_lines(text).into_iter().enumerate() {
            if !cell.is_empty() {
                text.draw_in(self.face, canvas, left as i32, top as i32, self.font_px, ink(at < first), &cell);
            }
        }
        text.look = Look::default();
    }

    /// How large a box's stamps are, across half.
    pub fn stamp_radius(&self) -> f32 {
        (self.font_px as f32 * 0.72).max(8.0)
    }

    /// The middle of each of a box's stamps, side by side along its top
    /// from its right corner, the first stamped at the corner.
    pub fn stamp_spots(&self) -> Vec<(u8, Point)> {
        if !matches!(self.tool, Tool::Rect | Tool::Ellipse) {
            return Vec::new();
        }
        let (x, y, w, _) = self.frame();
        let radius = self.stamp_radius();
        self.stamps.iter().enumerate().map(|(at, &kind)| (kind, (x + w - radius * 0.35 - at as f32 * radius * 2.3, y + radius * 0.2))).collect()
    }

    /// What a box's stamps cover, past its edge as they are.
    pub(super) fn stamp_reach(&self) -> Option<Rect> {
        let spots = self.stamp_spots();
        let radius = self.stamp_radius() + 3.0 * self.scale.max(1e-3) + 2.0;
        let (left, top) = spots.iter().fold((f32::MAX, f32::MAX), |(l, t), &(_, (cx, cy))| (l.min(cx - radius), t.min(cy - radius)));
        let (right, bottom) = spots.iter().fold((f32::MIN, f32::MIN), |(r, b), &(_, (cx, cy))| (r.max(cx + radius), b.max(cy + radius + 2.0)));
        (!spots.is_empty()).then(|| Rect::new(left.floor() as i32, top.floor() as i32, (right - left).ceil() as i32 + 1, (bottom - top).ceil() as i32 + 1))
    }

    /// A box's stamps, the first stamped over the rest.
    pub(super) fn draw_stamps(&self, canvas: &mut Canvas) {
        let radius = self.stamp_radius();
        for &(kind, at) in self.stamp_spots().iter().rev() {
            draw_stamp(canvas, kind, at, radius, self.opacity, self.sketched().then(|| self.seed() ^ u32::from(kind).wrapping_mul(0x85EB_CA6B)));
        }
    }
}

/// A stamp: a round badge of its color on a ring of white, a little
/// shadow under it, and its mark in white over it: a check, a plus, a
/// question mark, a cross or a heart. Drawn by hand, from a seed, its
/// ring and its mark a little uneven, as a hand would make them.
pub fn draw_stamp(canvas: &mut Canvas, kind: u8, at: Point, radius: f32, opacity: u32, hand: Option<u32>) {
    let (cx, cy) = at;
    let (_, color) = STAMPS[usize::from(kind).min(STAMPS.len() - 1)];
    let mut seed = hand.unwrap_or(0) | 1;
    let mut wobble = |amount: f32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        match hand {
            Some(_) => (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * amount,
            None => 0.0,
        }
    };
    let ring = (radius * 0.16).max(1.5);
    let reach = Rect::new((cx - radius - ring - 3.0).floor() as i32, (cy - radius - ring - 3.0).floor() as i32, ((radius + ring + 3.0) * 2.0).ceil() as i32 + 2, ((radius + ring + 3.0) * 2.0).ceil() as i32 + 4);
    // Its edge: round, or by hand a little off round, slowly.
    let phase = wobble(std::f32::consts::PI);
    let lumps = (wobble(0.05), wobble(0.04));
    let edge = |grow: f32, dy: f32| -> Vec<Point> {
        (0..=48).map(|step| {
            // Gone all the way round, back to where it began, for the
            // fill to close it.
            let angle = std::f32::consts::TAU * step as f32 / 48.0;
            let far = (radius + grow) * (1.0 + lumps.0 * (2.0 * angle + phase).sin() + lumps.1 * (3.0 * angle - phase).cos());
            (cx + far * angle.cos(), cy + dy + far * angle.sin())
        }).collect()
    };
    let disc = |grow: f32, dy: f32| match hand {
        Some(_) => fill_path(reach, &edge(grow, dy)),
        None => covered(reach, move |px, py| (px - cx).hypot(py - cy - dy) - radius - grow),
    };
    canvas.mask(&disc(ring * 0.6, ring * 0.9), 0x000000, opacity * 40 / 256);
    canvas.mask(&disc(ring, 0.0), 0xFFFFFF, opacity);
    canvas.mask(&disc(0.0, 0.0), color, opacity);
    // Its mark, in white, as heavy as a fifth of it.
    let half = (radius * 0.11).max(1.0);
    let mut point = |x: f32, y: f32| (cx + radius * x + wobble(radius * 0.04), cy + radius * y + wobble(radius * 0.04));
    let inks: Vec<Ink> = match kind {
        0 => vec![Ink::even(vec![point(-0.4, 0.02), point(-0.12, 0.3), point(0.42, -0.3)], half, false)],
        1 => vec![Ink::even(vec![point(-0.44, 0.0), point(0.44, 0.0)], half, false), Ink::even(vec![point(0.0, -0.44), point(0.0, 0.44)], half, false)],
        3 => vec![Ink::even(vec![point(-0.32, -0.32), point(0.32, 0.32)], half, false), Ink::even(vec![point(-0.32, 0.32), point(0.32, -0.32)], half, false)],
        4 => {
            let heart: Vec<Point> = (0..=40).map(|step| {
                let t = std::f32::consts::TAU * step as f32 / 40.0;
                let (sx, sy) = (16.0 * t.sin().powi(3), -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos()));
                (cx + radius * 0.58 * sx / 16.0, cy + radius * 0.06 + radius * 0.58 * sy / 16.0)
            }).collect();
            vec![Ink::even(heart, half * 0.4, true)]
        }
        _ => {
            // A question mark: its hook, and its dot under it.
            let hook: Vec<Point> = (0..=14).map(|step| {
                let t = step as f32 / 14.0;
                let angle = std::f32::consts::PI * (1.1 - 1.55 * t);
                (cx + radius * 0.26 * angle.cos(), cy - radius * 0.2 - radius * 0.24 * angle.sin())
            }).chain([point(0.03, 0.06), point(0.0, 0.2)]).collect();
            vec![Ink::even(hook, half, false), Ink::even(vec![point(0.0, 0.42), point(0.0, 0.42)], half * 1.2, false)]
        }
    };
    canvas.mask(&lay_inks(reach, &inks), 0xFFFFFF, opacity);
}
