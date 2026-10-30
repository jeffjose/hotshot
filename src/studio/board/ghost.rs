//! Guessing what comes next, as Figma's third duplicate does but for
//! drawing: once there is a pattern, the board offers the next step faintly,
//! dashed in the accent: the next box of a row, a column or a grid; the
//! rest of a timeline's ticks, labeled as its series goes; the next arrow
//! of a chain, or the one back to its first; the next number of a box;
//! a branch mirrored; the next child of a tree, a third lane, a 2x2's axes.
//! One at a time; Tab or a click keeps it and Tab again the next, Shift+Tab
//! the rest of a timeline; Esc or anything else lets it go. It never moves
//! what is there, and takes the look of what it follows.

use super::*;

/// What is offered.
#[derive(Clone)]
pub(in crate::studio) struct Ghost {
    pub kind: Guess,
    /// What keeping it adds, on the board.
    pub layers: Vec<Layer>,
    /// The words it offers a box being typed into, by its layer.
    pub label: Option<(usize, String)>,
    /// Which of what it adds is typed into once it is kept.
    pub type_into: Option<usize>,
}

/// The kinds of guess, for the rest of a timeline to be filled at once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) enum Guess {
    Number,
    Row,
    Timeline,
    Chain,
    Loop,
    Mirror,
    Child,
    Lane,
    Axis,
}

/// How near is near enough to be in line, and how long a tick can be.
const LEVEL: f32 = 6.0;
const TICK: f32 = 60.0;

/// Series of words a board knows the next of.
const SERIES: [&[&str]; 8] = [
    &["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    &["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    &["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    &["Now", "Next", "Later"],
    &["To do", "Doing", "Done"],
    &["Before", "After"],
    &["Q1", "Q2", "Q3", "Q4"],
];

const ROMAN: [&str; 12] = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii"];

/// A number in words, the next by a step: "Step 2" after "Step 1", "B"
/// after "A", "iii" after "ii", "3." after "2.", "v3" after "v2", "2026"
/// after "2025", "30%" after "20%", "Mar" after "Feb"; with two, as far on
/// as they are apart. Only the first word or two of what is said.
pub(in crate::studio) fn next_label(said: &[&str]) -> Option<String> {
    let last = said.last()?.trim();
    let first_line = last.lines().next()?.trim();
    // A word of a series.
    for series in SERIES {
        if let Some(at) = series.iter().position(|word| word.eq_ignore_ascii_case(first_line)) {
            let step = said.len().checked_sub(2).and_then(|before| series.iter().position(|word| word.eq_ignore_ascii_case(said[before].trim()))).map_or(1, |was| (at + series.len() - was) % series.len()).max(1);
            if series.len() <= 3 && at + step >= series.len() {
                return None;
            }
            return Some(series[(at + step) % series.len()].to_owned());
        }
    }
    let head: String = first_line.split([':', '\u{b7}', '-', '\u{2014}']).next()?.trim().to_owned();
    let words: Vec<&str> = head.split_whitespace().collect();
    // A word and a number: Step 1, Phase 2, Part 3.
    if let [word, number, ..] = words[..] {
        if let Ok(number) = number.parse::<i64>() {
            let step = said.len().checked_sub(2).and_then(|before| said[before].split_whitespace().nth(1).and_then(|n| n.trim_end_matches(':').parse::<i64>().ok())).map_or(1, |was| (number - was).max(1));
            return Some(format!("{word} {}", number + step));
        }
    }
    // One word, or a number or a letter marked as one, and words after it.
    let token = match words[..] {
        [token] => token,
        [token, ..] if token.ends_with(['.', ')']) => token,
        _ => return None,
    };
    // 1. or 1) or 1, and a percent, and a year or a plain number.
    for end in [".", ")", "%", ""] {
        if let Some(number) = token.strip_suffix(end).filter(|_| !end.is_empty() || token.chars().all(|c| c.is_ascii_digit())).and_then(|n| n.parse::<i64>().ok()) {
            let step = said.len().checked_sub(2).and_then(|before| said[before].trim().split_whitespace().next().and_then(|n| n.trim_end_matches(['.', ')', '%']).parse::<i64>().ok())).map_or(1, |was| (number - was).max(1));
            return Some(format!("{}{end}", number + step));
        }
    }
    // v1, v2.
    if let Some(number) = token.strip_prefix(['v', 'V']).and_then(|n| n.parse::<i64>().ok()) {
        return Some(format!("{}{}", &token[..1], number + 1));
    }
    // i, ii, iii; I, II, III.
    let lower = token.trim_end_matches(['.', ')']).to_lowercase();
    if let Some(at) = ROMAN.iter().position(|roman| *roman == lower).filter(|&at| at + 1 < ROMAN.len() && (lower.len() > 1 || said.len() > 1 || at == 0)) {
        let next = ROMAN[at + 1];
        let next = if token.chars().next().is_some_and(char::is_uppercase) { next.to_uppercase() } else { next.to_owned() };
        return Some(format!("{next}{}", &token[token.trim_end_matches(['.', ')']).len()..]));
    }
    // A, B; a, b; A. B.
    let bare = token.trim_end_matches(['.', ')']);
    if bare.len() == 1 {
        let letter = bare.chars().next()?;
        if letter.is_ascii_alphabetic() && !letter.eq_ignore_ascii_case(&'z') {
            return Some(format!("{}{}", (letter as u8 + 1) as char, &token[1..]));
        }
    }
    None
}

impl Studio {
    /// The boxes of the board, each with where it is, the latest drawn last.
    fn boxes(&self) -> Vec<(usize, (f32, f32, f32, f32))> {
        (0..self.doc.layers.len()).filter(|&index| self.is_box(index) && !self.doc.layers[index].hidden).map(|index| (index, self.box_frame(index))).collect()
    }

    fn words_of(&self, index: usize) -> String {
        match &self.doc.layers[index].kind {
            Kind::Shape(shape) => shape.text.clone(),
            Kind::Picture(_) => String::new(),
        }
    }

    /// Whether a place is free of every box.
    fn free(&self, (x, y, w, h): (f32, f32, f32, f32)) -> bool {
        self.boxes().iter().all(|&(_, (bx, by, bw, bh))| x + w <= bx + 1.0 || bx + bw <= x + 1.0 || y + h <= by + 1.0 || by + bh <= y + 1.0)
    }

    /// A box like another, where a frame says, with words; called
    /// something of its own.
    fn ghost_box(&self, like: usize, (x, y, w, h): (f32, f32, f32, f32), words: String) -> Layer {
        let mut layer = self.doc.layers[like].clone();
        if let Kind::Shape(shape) = &mut layer.kind {
            *shape = shape.copied();
            shape.points = vec![(x, y), (x + w, y + h)];
            (shape.text, shape.status, shape.votes, shape.editing, shape.titled) = (words, 0, 0, false, shape.heading);
            shape.base = (w, h);
            shape.touch();
        }
        layer.hidden = false;
        layer
    }

    /// A board's arrow from one thing to another, by what each is called,
    /// routed between where they are.
    fn ghost_arrow(&self, from: u32, to: u32, frames: ((f32, f32, f32, f32), (f32, f32, f32, f32))) -> Layer {
        let (a, b) = frames;
        let mut arrow = Shape::new(Tool::Arrow, self.board.stroke, LINE, (a.0 + a.2 / 2.0, a.1 + a.3 / 2.0));
        self.board_dress(&mut arrow);
        arrow.points[1] = (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
        arrow.stuck = [Some((from, shapes::WHOLE, shapes::WHOLE)), Some((to, shapes::WHOLE, shapes::WHOLE))];
        let anchor = |(x, y, w, h): (f32, f32, f32, f32), id: u32| shapes::Anchor { id, middle: (x + w / 2.0, y + h / 2.0), half: (w / 2.0, h / 2.0), angle: 0.0, round: false, pointed: false };
        shapes::route(&mut arrow, &anchor(a, from), &anchor(b, to));
        Layer::shape(arrow)
    }

    /// The next thing guessed, if guessing is on and there is a pattern.
    /// `drawn` says it follows a shape just drawn, rather than a box picked.
    pub(in crate::studio) fn guess(&mut self, drawn: bool) {
        self.board.ghost = None;
        if !self.board.guessing || !self.on_board() {
            return;
        }
        let ghost = match drawn {
            true => self.guess_number().or_else(|| self.guess_shapes()),
            false => self.guess_loop().or_else(|| self.guess_mirror()),
        };
        self.board.ghost = ghost;
        self.dirty = true;
    }

    /// What comes next of what is drawn, besides a number: a tick, an
    /// arrow, a child, a box of a grid, a lane, a branch, an axis.
    fn guess_shapes(&self) -> Option<Ghost> {
        self.guess_timeline().or_else(|| self.guess_chain()).or_else(|| self.guess_child()).or_else(|| self.guess_lane()).or_else(|| self.guess_axis()).or_else(|| self.guess_grid()).or_else(|| self.guess_mirror())
    }

    /// The box being typed into, just drawn and still empty, after one
    /// numbered: its number offered.
    fn guess_number(&self) -> Option<Ghost> {
        let editing = self.editing.filter(|&index| self.is_box(index) && self.words_of(index).is_empty())?;
        let before = self.boxes().into_iter().map(|(index, _)| index).filter(|&index| index != editing).filter(|&index| !self.words_of(index).trim().is_empty()).last()?;
        let words = self.words_of(before);
        let earlier: Vec<String> = self.boxes().into_iter().map(|(index, _)| index).filter(|&index| index < before && index != editing).map(|index| self.words_of(index)).filter(|words| !words.trim().is_empty()).collect();
        let said: Vec<&str> = earlier.last().map(String::as_str).into_iter().chain(std::iter::once(words.as_str())).collect();
        let label = next_label(&said).or_else(|| next_label(&[words.as_str()]))?;
        Some(Ghost { kind: Guess::Number, layers: Vec::new(), label: Some((editing, label)), type_into: None })
    }

    /// Two boxes the same size in line: a third at the same gap; boxes in
    /// rows and columns: the next of the grid, or of its next row.
    fn guess_grid(&self) -> Option<Ghost> {
        let boxes = self.boxes();
        let &(latest, b) = boxes.last()?;
        let same = |f: &(f32, f32, f32, f32)| (f.2 - b.2).abs() <= 4.0 && (f.3 - b.3).abs() <= 4.0;
        let alike: Vec<(usize, (f32, f32, f32, f32))> = boxes.iter().copied().filter(|(_, f)| same(f)).collect();
        // Rows and columns, by where they begin.
        let mut xs: Vec<f32> = Vec::new();
        let mut ys: Vec<f32> = Vec::new();
        for (_, f) in &alike {
            if !xs.iter().any(|x| (x - f.0).abs() <= LEVEL) {
                xs.push(f.0);
            }
            if !ys.iter().any(|y| (y - f.1).abs() <= LEVEL) {
                ys.push(f.1);
            }
        }
        xs.sort_by(f32::total_cmp);
        ys.sort_by(f32::total_cmp);
        let at = |x: f32, y: f32| alike.iter().find(|(_, f)| (f.0 - x).abs() <= LEVEL && (f.1 - y).abs() <= LEVEL).map(|&(index, _)| index);
        let label_after = |studio: &Studio, indices: &[usize]| {
            let words: Vec<String> = indices.iter().map(|&index| studio.words_of(index)).filter(|words| !words.trim().is_empty()).collect();
            let said: Vec<&str> = words.iter().rev().take(2).rev().map(String::as_str).collect();
            next_label(&said).unwrap_or_default()
        };
        if xs.len() >= 2 && ys.len() >= 2 {
            // The first cell missing, row by row, up to the last row; and
            // with all full, the next row's first.
            let cells = alike.len();
            if cells >= 3 {
                for &y in &ys {
                    for &x in &xs {
                        if at(x, y).is_none() {
                            let frame = (x, y, b.2, b.3);
                            if self.free(frame) {
                                let row: Vec<usize> = xs.iter().filter_map(|&x| at(x, y)).collect();
                                let column: Vec<usize> = ys.iter().filter_map(|&y| at(x, y)).collect();
                                let words = if row.len() >= 2 { label_after(self, &row) } else { label_after(self, &column) };
                                return Some(self.box_ghost(latest, frame, words, Guess::Row));
                            }
                        }
                    }
                }
                if ys.len() >= 2 && xs.iter().all(|&x| ys.iter().all(|&y| at(x, y).is_some())) {
                    let step = ys[ys.len() - 1] - ys[ys.len() - 2];
                    let frame = (xs[0], ys[ys.len() - 1] + step, b.2, b.3);
                    if self.free(frame) {
                        let column: Vec<usize> = ys.iter().filter_map(|&y| at(xs[0], y)).collect();
                        return Some(self.box_ghost(latest, frame, label_after(self, &column), Guess::Row));
                    }
                }
            }
        }
        // The two latest the same size, in line: the next as far on.
        let &(before, a) = alike.iter().rev().nth(1)?;
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let in_row = dy.abs() <= LEVEL && dx.abs() > b.2;
        let in_column = dx.abs() <= LEVEL && dy.abs() > b.3;
        if !in_row && !in_column {
            return None;
        }
        let frame = if in_row { (b.0 + dx, b.1, b.2, b.3) } else { (b.0, b.1 + dy, b.2, b.3) };
        self.free(frame).then(|| self.box_ghost(latest, frame, label_after(self, &[before, latest]), Guess::Row))
    }

    fn box_ghost(&self, like: usize, frame: (f32, f32, f32, f32), words: String, kind: Guess) -> Ghost {
        Ghost { kind, layers: vec![self.ghost_box(like, frame, words)], label: None, type_into: Some(0) }
    }

    /// A line and ticks across it at a spacing: the next tick as far on, to
    /// the line's end, labeled as the ticks' labels go.
    fn guess_timeline(&self) -> Option<Ghost> {
        let lines: Vec<(usize, Shape)> = self.doc.layers.iter().enumerate().filter_map(|(index, layer)| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Line && !layer.hidden => Some((index, shape.clone())),
            _ => None,
        }).collect();
        let long = |shape: &Shape| {
            let (a, b) = (shape.points[0], shape.points[1]);
            (b.0 - a.0).hypot(b.1 - a.1)
        };
        for (_, axis) in lines.iter().filter(|(_, shape)| long(shape) > TICK * 3.0) {
            let (a, b) = (axis.points[0], axis.points[1]);
            let across = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
            let (start, end) = if across { (a.0.min(b.0), a.0.max(b.0)) } else { (a.1.min(b.1), a.1.max(b.1)) };
            let level = |t: f32| if across { a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0).max(1e-3) } else { a.0 + (b.0 - a.0) * (t - a.1) / (b.1 - a.1).max(1e-3) };
            // Its ticks: short lines across it, by where along it.
            let mut ticks: Vec<(f32, Shape)> = lines
                .iter()
                .filter(|(_, tick)| long(tick) <= TICK && tick.id != axis.id)
                .filter_map(|(_, tick)| {
                    let (p, q) = (tick.points[0], tick.points[1]);
                    let mid = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
                    let (along, off) = if across { (mid.0, (mid.1 - level(mid.0)).abs()) } else { (mid.1, (mid.0 - level(mid.1)).abs()) };
                    let square = if across { (q.1 - p.1).abs() > (q.0 - p.0).abs() } else { (q.0 - p.0).abs() > (q.1 - p.1).abs() };
                    (square && off < 16.0 && along >= start - 2.0 && along <= end + 2.0).then(|| (along, tick.clone()))
                })
                .collect();
            if ticks.len() < 2 {
                continue;
            }
            ticks.sort_by(|p, q| p.0.total_cmp(&q.0));
            let step = ticks[1].0 - ticks[0].0;
            if step < 10.0 || ticks.windows(2).any(|pair| (pair[1].0 - pair[0].0 - step).abs() > 4.0) {
                continue;
            }
            let (last, tick) = ticks.last().cloned()?;
            let next = last + step;
            if next > end + 1.0 {
                continue;
            }
            let mut made = tick.copied();
            made.translate(if across { step } else { 0.0 }, if across { 0.0 } else { step });
            let mut layers = vec![Layer::shape(made)];
            // The ticks' labels: words near each, the next as they go.
            let labels: Vec<Shape> = ticks
                .iter()
                .filter_map(|(along, _)| {
                    self.doc.layers.iter().filter_map(|layer| match &layer.kind {
                        Kind::Shape(shape) if shape.tool == Tool::Text && !shape.text.trim().is_empty() => Some(shape.clone()),
                        _ => None,
                    }).find(|text| {
                        let at = text.points[0];
                        let mid = if across { at.0 + self.board.px as f32 } else { at.1 };
                        (mid - along).abs() < step / 2.0 && if across { (at.1 - level(*along)).abs() < 70.0 } else { (at.0 - level(*along)).abs() < 160.0 }
                    })
                })
                .collect();
            if labels.len() >= 2 {
                let said: Vec<&str> = labels.iter().rev().take(2).rev().map(|shape| shape.text.as_str()).collect();
                if let Some(words) = next_label(&said) {
                    let mut label = labels.last()?.copied();
                    label.translate(if across { step } else { 0.0 }, if across { 0.0 } else { step });
                    label.text = words;
                    layers.push(Layer::shape(label));
                }
            }
            return Some(Ghost { kind: Guess::Timeline, layers, label: None, type_into: None });
        }
        None
    }

    /// A chain of arrows, A to B and B to C, and a box drawn joined to
    /// nothing: the arrow from the chain's end to it.
    fn guess_chain(&self) -> Option<Ghost> {
        let boxes = self.boxes();
        let &(latest, frame) = boxes.last()?;
        let id = self.id_of(latest);
        let links = self.links();
        if links.iter().any(|&(_, from, to)| from == id || to == id) {
            return None;
        }
        let ends: Vec<u32> = links.iter().map(|&(_, _, to)| to).filter(|&to| !links.iter().any(|&(_, from, _)| from == to)).filter(|&end| self.chain_length(end) >= 2).collect();
        let end = ends.into_iter().min_by(|&a, &b| {
            let far = |id: u32| self.index_of(id).map_or(f32::MAX, |index| {
                let f = self.box_frame(index);
                (f.0 - frame.0).hypot(f.1 - frame.1)
            });
            far(a).total_cmp(&far(b))
        })?;
        let from = self.box_frame(self.index_of(end)?);
        Some(Ghost { kind: Guess::Chain, layers: vec![self.ghost_arrow(end, id, (from, frame))], label: None, type_into: None })
    }

    /// How many arrows lead back up a chain to something.
    fn chain_length(&self, id: u32) -> usize {
        let mut count = 0;
        let mut at = id;
        while let Some(parent) = self.parent_box(at) {
            count += 1;
            at = parent;
            if count > 64 {
                break;
            }
        }
        count
    }

    /// The end of a chain of three or more picked out: the arrow back to
    /// its first, to close the loop.
    fn guess_loop(&self) -> Option<Ghost> {
        let end = self.picked().filter(|&index| self.is_box(index))?;
        let id = self.id_of(end);
        let links = self.links();
        if links.iter().any(|&(_, from, _)| from == id) || self.chain_length(id) < 2 {
            return None;
        }
        let mut first = id;
        while let Some(parent) = self.parent_box(first) {
            first = parent;
            if first == id {
                return None;
            }
        }
        let start = self.index_of(first)?;
        Some(Ghost { kind: Guess::Loop, layers: vec![self.ghost_arrow(id, first, (self.box_frame(end), self.box_frame(start)))], label: None, type_into: None })
    }

    /// A parent and two children or more in line, the latest drawn one of
    /// them: the next child, as far on, joined.
    fn guess_child(&self) -> Option<Ghost> {
        let &(latest, b) = self.boxes().last()?;
        let parent = self.parent_box(self.id_of(latest))?;
        let kids: Vec<(usize, (f32, f32, f32, f32))> = self.child_boxes(parent).into_iter().filter_map(|kid| self.index_of(kid)).map(|index| (index, self.box_frame(index))).collect();
        if kids.len() < 2 {
            return None;
        }
        let column = kids.iter().all(|(_, f)| (f.0 - b.0).abs() <= LEVEL * 2.0);
        let row = kids.iter().all(|(_, f)| (f.1 - b.1).abs() <= LEVEL * 2.0);
        let (last, l) = *kids.last()?;
        let (prev, p) = kids[kids.len() - 2];
        let frame = match (column, row) {
            (true, _) => (l.0, l.1 + (l.1 - p.1), l.2, l.3),
            (_, true) => (l.0 + (l.0 - p.0), l.1, l.2, l.3),
            _ => return None,
        };
        if !self.free(frame) {
            return None;
        }
        let words = next_label(&[self.words_of(prev).as_str(), self.words_of(last).as_str()]).unwrap_or_default();
        let made = self.ghost_box(last, frame, words);
        let made_id = stacks::layer_id(&made);
        let from = self.box_frame(self.index_of(parent)?);
        Some(Ghost { kind: Guess::Child, layers: vec![made, self.ghost_arrow(parent, made_id, (from, frame))], label: None, type_into: Some(0) })
    }

    /// Two lanes side by side, tall and alike: a third as far on.
    fn guess_lane(&self) -> Option<Ghost> {
        let boxes = self.boxes();
        let &(latest, b) = boxes.last()?;
        if b.3 < b.2 * 1.6 {
            return None;
        }
        let (before, a) = boxes.iter().rev().skip(1).find(|(_, f)| (f.2 - b.2).abs() <= 4.0 && (f.3 - b.3).abs() <= 4.0 && (f.1 - b.1).abs() <= LEVEL).copied()?;
        let step = b.0 - a.0;
        if step.abs() < b.2 {
            return None;
        }
        let frame = (b.0 + step, b.1, b.2, b.3);
        let words = next_label(&[self.words_of(before).as_str(), self.words_of(latest).as_str()]).unwrap_or_default();
        self.free(frame).then(|| self.box_ghost(latest, frame, words, Guess::Lane))
    }

    /// A center with two branches or more, all on one side: the first not
    /// yet mirrored, mirrored on the other side, joined.
    fn guess_mirror(&self) -> Option<Ghost> {
        let latest = self.picked().filter(|&index| self.is_box(index)).or_else(|| self.boxes().last().map(|&(index, _)| index))?;
        let id = self.id_of(latest);
        let center = self.parent_box(id).filter(|&parent| self.child_boxes(parent).len() >= 2).or_else(|| (self.child_boxes(id).len() >= 2).then_some(id))?;
        let m = self.box_frame(self.index_of(center)?);
        let mid = m.0 + m.2 / 2.0;
        let kids: Vec<(usize, (f32, f32, f32, f32))> = self.child_boxes(center).into_iter().filter_map(|kid| self.index_of(kid)).map(|index| (index, self.box_frame(index))).collect();
        let right = kids.iter().filter(|(_, f)| f.0 + f.2 / 2.0 > mid).count();
        let left = kids.len() - right;
        if kids.len() < 2 || (left > 0 && right > 0 && left == right) {
            return None;
        }
        let (more_right, few) = (right > left, right.min(left));
        let side: Vec<&(usize, (f32, f32, f32, f32))> = kids.iter().filter(|(_, f)| (f.0 + f.2 / 2.0 > mid) == more_right).collect();
        let &&(kid, f) = side.get(few)?;
        let frame = (2.0 * mid - (f.0 + f.2), f.1, f.2, f.3);
        if !self.free(frame) {
            return None;
        }
        let made = self.ghost_box(kid, frame, String::new());
        let made_id = stacks::layer_id(&made);
        Some(Ghost { kind: Guess::Mirror, layers: vec![made, self.ghost_arrow(center, made_id, (m, frame))], label: None, type_into: Some(0) })
    }

    /// Four boxes alike in two rows and two columns, touching or near: its
    /// axes, the one along the bottom and then the one up its side.
    fn guess_axis(&self) -> Option<Ghost> {
        let boxes = self.boxes();
        let &(_, b) = boxes.last()?;
        let alike: Vec<(f32, f32, f32, f32)> = boxes.iter().map(|&(_, f)| f).filter(|f| (f.2 - b.2).abs() <= 4.0 && (f.3 - b.3).abs() <= 4.0).collect();
        if alike.len() != 4 {
            return None;
        }
        let (left, top) = (alike.iter().map(|f| f.0).fold(f32::MAX, f32::min), alike.iter().map(|f| f.1).fold(f32::MAX, f32::min));
        let (right, bottom) = (alike.iter().map(|f| f.0 + f.2).fold(f32::MIN, f32::max), alike.iter().map(|f| f.1 + f.3).fold(f32::MIN, f32::max));
        if right - left > b.2 * 2.0 + 24.0 || bottom - top > b.3 * 2.0 + 24.0 {
            return None;
        }
        let has = |words: &str| self.doc.layers.iter().any(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Text && shape.text.contains(words)));
        let mut label = Shape::new(Tool::Text, INK, LINE, (0.0, 0.0));
        self.board_dress(&mut label);
        let px = label.font_px as f32;
        if !has("Effort") {
            label.text = "Effort \u{2192}".into();
            label.points[0] = ((left + right) / 2.0 - px * 2.5, bottom + px * 0.6);
        } else if !has("Value") {
            label.text = "\u{2191} Value".into();
            label.points[0] = (left - px * 5.0, (top + bottom) / 2.0 - px / 2.0);
        } else {
            return None;
        }
        Some(Ghost { kind: Guess::Axis, layers: vec![Layer::shape(label)], label: None, type_into: None })
    }

    /// The ghost kept: what it adds added, a box of it typed into, words it
    /// offers given; and the next guessed. Shift keeps the rest of a
    /// timeline at once. One step to undo.
    pub(in crate::studio) fn keep_ghost(&mut self, all: bool) -> bool {
        let Some(ghost) = self.board.ghost.take() else { return false };
        if let Some((index, words)) = ghost.label.clone() {
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                shape.text = format!("{words} ");
                shape.touch();
            }
            self.words_changed(index);
            self.guess(true);
            if self.board.ghost.as_ref().is_some_and(|next| next.kind == Guess::Number) {
                self.board.ghost = None;
            }
            return true;
        }
        self.commit_text();
        self.checkpoint();
        let start = self.doc.layers.len();
        self.doc.layers.extend(ghost.layers);
        self.changed();
        match ghost.type_into {
            Some(at) if start + at < self.doc.layers.len() => {
                let index = start + at;
                self.selected = vec![index];
                // Words it came with are added to, the caret after them.
                if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get_mut(index) {
                    if !shape.text.is_empty() {
                        shape.text.push(' ');
                    }
                }
                self.edit_text(index);
            }
            _ => self.selected.clear(),
        }
        self.guess(ghost.kind != Guess::Loop);
        if all {
            for _ in 0..200 {
                match &self.board.ghost {
                    Some(next) if next.kind == Guess::Timeline => {
                        let layers = self.board.ghost.take().map(|next| next.layers).unwrap_or_default();
                        self.doc.layers.extend(layers);
                        self.changed();
                        self.guess(true);
                    }
                    _ => break,
                }
            }
        }
        true
    }

    /// A key with something guessed: Tab keeps it, and typing into the box
    /// being typed into leaves it be, but for words it offers that box,
    /// which typing replaces; anything else lets it go.
    pub(in crate::studio) fn ghost_keys(&mut self, key: Key, mods: Mods) {
        let Some(ghost) = &self.board.ghost else { return };
        let typing = self.editing.is_some() && !mods.ctrl && !mods.alt;
        let keeps = match key {
            Key::Tab | Key::Other => true,
            Key::Char(_) | Key::Backspace => typing && ghost.label.is_none(),
            Key::Enter => typing && ghost.label.is_none(),
            _ => false,
        };
        if !keeps {
            // A number typed over, what else comes next is still offered.
            let replaced = ghost.label.is_some() && typing && matches!(key, Key::Char(_));
            self.board.ghost = if replaced { self.guess_shapes() } else { None };
            self.dirty = true;
        }
    }

    /// Whether a press is on the ghost, to keep it.
    pub(in crate::studio) fn on_ghost(&mut self, at: (f32, f32)) -> bool {
        let Some(ghost) = self.board.ghost.clone() else { return false };
        ghost.layers.iter().any(|layer| {
            let (x, y, w, h) = layer.bounds(&mut self.text);
            at.0 >= x - 6.0 && at.0 <= x + w + 6.0 && at.1 >= y - 6.0 && at.1 <= y + h + 6.0
        })
    }

    /// The ghost, faint and dashed in the accent, with what Tab does by it;
    /// words offered in a box, faint in it.
    pub(in crate::studio) fn draw_ghost(&mut self, canvas: &mut Canvas) {
        let Some(ghost) = self.board.ghost.clone() else { return };
        let mut corner: Option<(i32, i32)> = None;
        for layer in &ghost.layers {
            let Kind::Shape(shape) = &layer.kind else { continue };
            let mut faint = scaled(shape, self.zoom);
            faint.translate(self.pan.0, self.pan.1);
            (faint.color, faint.sketch, faint.dashed, faint.opacity, faint.shadow, faint.border) = (ACCENT, false, matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow), 190, None, true);
            if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                (faint.filled, faint.fill) = (true, canvas::mix(shapes::PAPER, ACCENT, 16));
                faint.stroke = 1.6;
            }
            faint.touch();
            faint.draw(canvas, &mut self.text);
            let place = faint.bounds(&mut self.text);
            corner = Some(corner.map_or((place.right(), place.y), |(x, y)| (x.max(place.right()), y.min(place.y))));
        }
        if let Some((index, words)) = &ghost.label {
            if let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(*index) {
                let mut faint = scaled(shape, self.zoom);
                faint.translate(self.pan.0, self.pan.1);
                (faint.text, faint.color, faint.opacity, faint.border, faint.filled) = (words.clone(), ACCENT, 170, true, false);
                faint.touch();
                let (lines, _) = faint.label_lines(&mut self.text);
                self.text.look = crate::text::Look { opacity: 170, ..crate::text::Look::default() };
                for (line, left, top, _) in lines {
                    self.text.draw_in(faint.face, canvas, left.round() as i32, top.round() as i32, faint.font_px, ACCENT, &line);
                }
                self.text.look = crate::text::Look::default();
                let place = faint.bounds(&mut self.text);
                corner = Some((place.right(), place.y));
            }
        }
        // What Tab does, by it.
        if let Some((x, y)) = corner {
            let says = if ghost.kind == Guess::Timeline { "Tab \u{b7} add   \u{21e7}Tab \u{b7} all" } else { "Tab \u{b7} add" };
            let wide = self.text.width(says, 11) + 14;
            let (left, top) = (x + 6, (y - 24).max(4));
            canvas.round_fill(left, top, wide, 20, 5, ACCENT, 256);
            self.text.draw(canvas, left + 7, top + 10 - Text::line_height(11) / 2, 11, 0xFFFFFF, says);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    /// A box of the board's, where a frame says, with words.
    fn put(studio: &mut Studio, (x, y, w, h): (f32, f32, f32, f32), words: &str) -> usize {
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
        studio.board_dress(&mut shape);
        shape.points[1] = (x + w, y + h);
        (shape.text, shape.base) = (words.into(), (w, h));
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        studio.doc.layers.len() - 1
    }

    fn line(studio: &mut Studio, a: (f32, f32), b: (f32, f32)) {
        let mut shape = Shape::new(Tool::Line, INK, LINE, a);
        studio.board_dress(&mut shape);
        shape.points[1] = b;
        studio.doc.layers.push(Layer::shape(shape));
    }

    fn label(studio: &mut Studio, at: (f32, f32), words: &str) {
        let mut shape = Shape::new(Tool::Text, INK, LINE, at);
        studio.board_dress(&mut shape);
        shape.text = words.into();
        studio.doc.layers.push(Layer::shape(shape));
    }

    fn kind(studio: &Studio) -> Option<Guess> {
        studio.board.ghost.as_ref().map(|ghost| ghost.kind)
    }

    #[test]
    fn two_boxes_in_a_row_offer_a_third_tab_keeps_it_and_tab_again_the_next() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (240.0, 160.0), Mods::default());
        type_in(&mut studio, "Read");
        press_key(&mut studio, Key::Enter, Mods::default());
        assert!(studio.board.ghost.is_none(), "one box is no pattern");
        pull(&mut studio, (300.0, 100.0), (440.0, 160.0), Mods::default());
        assert_eq!(kind(&studio), Some(Guess::Row), "a third waits at the same gap");
        type_in(&mut studio, "Parse");
        assert!(studio.board.ghost.is_some(), "typing in the box leaves it be");
        press_key(&mut studio, Key::Tab, Mods::default());
        assert_eq!(studio.doc.layers.len(), 3);
        let third = shape(&studio, 2);
        assert!((third.frame().0 - 500.0).abs() < 1.0 && third.frame().1 == shape(&studio, 1).frame().1);
        assert_eq!(studio.editing, Some(2), "the caret ready in it");
        type_in(&mut studio, "Check");
        press_key(&mut studio, Key::Tab, Mods::default());
        assert_eq!(studio.doc.layers.len(), 4, "Tab again keeps the next");
        // Esc lets it go; nothing is ever moved by it.
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.ghost.is_none());
        // Turned off in the bar, nothing is guessed.
        studio.press_style(Ctl::Guess);
        pull(&mut studio, (900.0, 100.0), (1040.0, 160.0), Mods::default());
        assert!(studio.board.ghost.is_none());
    }

    #[test]
    fn rows_of_boxes_offer_the_next_of_the_grid_and_then_its_next_row() {
        let mut studio = board();
        for (x, y) in [(0.0, 0.0), (200.0, 0.0), (400.0, 0.0), (0.0, 120.0), (200.0, 120.0)] {
            put(&mut studio, (x + 100.0, y + 100.0, 160.0, 80.0), "");
        }
        studio.guess(true);
        let ghost = studio.board.ghost.clone().unwrap();
        assert!(matches!(&ghost.layers[0].kind, Kind::Shape(shape) if shape.frame().0 == 500.0 && shape.frame().1 == 220.0), "the third of the second row");
        put(&mut studio, (500.0, 220.0, 160.0, 80.0), "");
        studio.guess(true);
        let next = studio.board.ghost.clone().unwrap();
        let (x, y, ..) = match &next.layers[0].kind {
            Kind::Shape(shape) => shape.frame(),
            Kind::Picture(_) => panic!(),
        };
        assert_eq!((x, y), (100.0, 340.0), "the next row's first");
    }

    #[test]
    fn a_line_with_two_ticks_offers_the_rest_labeled_and_shift_tab_fills_it() {
        let mut studio = board();
        line(&mut studio, (100.0, 300.0), (800.0, 300.0));
        for (x, words) in [(150.0, "Q1"), (250.0, "Q2")] {
            line(&mut studio, (x, 288.0), (x, 312.0));
            label(&mut studio, (x - 12.0, 320.0), words);
        }
        studio.changed();
        studio.guess(true);
        let ghost = studio.board.ghost.clone().unwrap();
        assert_eq!(ghost.kind, Guess::Timeline);
        let words: Vec<String> = ghost.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Text => Some(shape.text.clone()),
            _ => None,
        }).collect();
        assert_eq!(words, ["Q3"]);
        // Shift+Tab: every tick to the line's end, a step to undo.
        press_key(&mut studio, Key::Tab, Mods { shift: true, ..Mods::default() });
        let ticks = studio.doc.layers.iter().filter(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Line && shape.frame().3 > 10.0)).count();
        assert_eq!(ticks, 7, "150 to 750, a hundred apart");
        let last = studio.doc.layers.iter().rev().find_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Text => Some(shape.text.clone()),
            _ => None,
        });
        assert_eq!(last.as_deref(), Some("Q3"), "Q4 wraps to Q1 and on");
    }

    #[test]
    fn a_chain_offers_the_next_arrow_and_its_end_the_way_back() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "API");
        let b = put(&mut studio, (300.0, 100.0, 120.0, 60.0), "Queue");
        let c = put(&mut studio, (500.0, 100.0, 120.0, 60.0), "Worker");
        studio.join(a, b);
        studio.join(b, c);
        studio.changed();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (500.0, 300.0), (620.0, 360.0), Mods::default());
        type_in(&mut studio, "Store");
        assert_eq!(kind(&studio), Some(Guess::Chain));
        press_key(&mut studio, Key::Tab, Mods::default());
        let d = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "Store")).unwrap();
        assert_eq!(studio.parent_box(studio.id_of(d)), Some(studio.id_of(c)));
        // The end of the chain picked out: the way back to its first.
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('v'), Mods::default());
        tap(&mut studio, (560.0, 340.0));
        assert_eq!(kind(&studio), Some(Guess::Loop));
        press_key(&mut studio, Key::Tab, Mods::default());
        assert_eq!(studio.parent_box(studio.id_of(a)), Some(studio.id_of(d)), "the loop closed");
    }

    #[test]
    fn a_box_after_a_numbered_one_is_offered_its_number_which_typing_replaces() {
        let mut studio = board();
        put(&mut studio, (100.0, 100.0, 140.0, 60.0), "Step 1: Plan");
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 300.0), (240.0, 360.0), Mods::default());
        assert_eq!(studio.board.ghost.as_ref().and_then(|ghost| ghost.label.clone()).map(|(_, words)| words).as_deref(), Some("Step 2"));
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Build");
        assert_eq!(shape(&studio, 1).text, "Step 2 Build");
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 500.0), (240.0, 560.0), Mods::default());
        assert!(studio.board.ghost.as_ref().is_some_and(|ghost| ghost.label.is_some()));
        type_in(&mut studio, "Ship");
        assert!(studio.board.ghost.as_ref().is_none_or(|ghost| ghost.label.is_none()) && shape(&studio, 2).text == "Ship", "typing replaces it");
    }

    #[test]
    fn a_tree_a_mind_map_lanes_and_a_two_by_two_grow_the_way_they_go() {
        // A parent and two children in a column: a third.
        let mut studio = board();
        let parent = put(&mut studio, (100.0, 200.0, 120.0, 60.0), "Root");
        let one = put(&mut studio, (320.0, 100.0, 120.0, 60.0), "One");
        let two = put(&mut studio, (320.0, 200.0, 120.0, 60.0), "Two");
        studio.join(parent, one);
        studio.join(parent, two);
        studio.changed();
        studio.guess(true);
        assert_eq!(kind(&studio), Some(Guess::Child));
        // A center with its branches all to one side: one mirrored.
        studio.board.ghost = None;
        studio.selected = vec![one];
        studio.guess(false);
        let ghost = studio.board.ghost.clone().unwrap();
        assert_eq!(ghost.kind, Guess::Mirror);
        let (x, ..) = ghost.layers[0].bounds(&mut studio.text);
        assert!(x < 100.0, "on the other side of the center");
        // Two lanes, Now and Next: Later.
        let mut lanes = board();
        put(&mut lanes, (100.0, 100.0, 200.0, 480.0), "Now");
        put(&mut lanes, (320.0, 100.0, 200.0, 480.0), "Next");
        lanes.guess(true);
        let lane = lanes.board.ghost.clone().unwrap();
        assert_eq!(lane.kind, Guess::Lane);
        assert!(matches!(&lane.layers[0].kind, Kind::Shape(shape) if shape.text == "Later" && (shape.frame().0 - 540.0).abs() < 1.0));
        // Four alike in two rows of two: its axes, one then the other.
        let mut square = board();
        for (x, y) in [(100.0, 100.0), (320.0, 100.0), (100.0, 300.0), (320.0, 300.0)] {
            put(&mut square, (x, y, 200.0, 180.0), "");
        }
        square.guess(true);
        assert_eq!(kind(&square), Some(Guess::Axis));
        press_key(&mut square, Key::Tab, Mods::default());
        assert_eq!(kind(&square), Some(Guess::Axis), "Tab again, the other axis");
        press_key(&mut square, Key::Tab, Mods::default());
        let words: Vec<String> = square.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Text => Some(shape.text.clone()),
            _ => None,
        }).collect();
        assert_eq!(words, ["Effort \u{2192}", "\u{2191} Value"]);
    }
}

#[cfg(test)]
mod series {
    use super::*;

    #[test]
    fn the_next_of_a_series_follows_what_is_said() {
        assert_eq!(next_label(&["Step 1: Plan"]).as_deref(), Some("Step 2"));
        assert_eq!(next_label(&["Phase 3"]).as_deref(), Some("Phase 4"));
        assert_eq!(next_label(&["A"]).as_deref(), Some("B"));
        assert_eq!(next_label(&["b."]).as_deref(), Some("c."));
        assert_eq!(next_label(&["ii"]).as_deref(), Some("iii"));
        assert_eq!(next_label(&["IV."]).as_deref(), Some("V."));
        assert_eq!(next_label(&["1."]).as_deref(), Some("2."));
        assert_eq!(next_label(&["Q4"]).as_deref(), Some("Q1"));
        assert_eq!(next_label(&["Feb", "Mar"]).as_deref(), Some("Apr"));
        assert_eq!(next_label(&["Mon"]).as_deref(), Some("Tue"));
        assert_eq!(next_label(&["v1", "v2"]).as_deref(), Some("v3"));
        assert_eq!(next_label(&["2024", "2026"]).as_deref(), Some("2028"));
        assert_eq!(next_label(&["10%", "20%"]).as_deref(), Some("30%"));
        assert_eq!(next_label(&["Now", "Next"]).as_deref(), Some("Later"));
        assert_eq!(next_label(&["Later"]), None);
        assert_eq!(next_label(&["Database"]), None);
    }
}
