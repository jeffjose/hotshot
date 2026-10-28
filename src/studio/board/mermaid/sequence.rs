//! Sequence diagrams, as a box that keeps them laid out as Mermaid does:
//! the people along its top, a box or a stick figure each, and again
//! along its foot; a dashed lifeline down from each; and what happens
//! down them in order at an even beat, each message tied to whom it is
//! between, with room over it for its words, and to its own sender a
//! loop. Boxes round the parts that are alternatives, loops, options and
//! the like, each with its kind and condition in a tab at its corner and
//! a dashed line between its branches; notes beside a lifeline or over a
//! few; a narrow bar on a lifeline while someone is busy; and each
//! message numbered, if asked. The people are spaced by the words that
//! go between them. A message dragged above another goes before it, and
//! a person dragged past another goes after; it all lays itself out
//! again.

use super::*;

/// What happens in a sequence, in order.
#[derive(Clone, Debug)]
enum Step {
    /// From, to, words, its line dashed, the head at its end, and whether
    /// it makes its receiver busy or ends its sender's being busy.
    Message { from: String, to: String, words: String, dashed: bool, head: Option<u8>, both: bool, busy: bool, done: bool },
    /// Where it goes, `right of A`, `left of A` or `over A,B`, and its words.
    Note(String, String),
    Open(String, Option<u32>),
    Branch(String),
    Close,
    Busy(String),
    Done(String),
}

/// A sequence read: its title, its people, each with its words and
/// whether it is a stick figure, what happens, and whether its messages
/// are numbered.
#[derive(Clone, Debug, Default)]
struct Sequence {
    title: String,
    people: Vec<(String, String, bool)>,
    steps: Vec<Step>,
    numbered: bool,
}

fn read_sequence(text: &str) -> Sequence {
    let mut sequence = Sequence { title: front_title(text).unwrap_or_default(), ..Sequence::default() };
    let person = |people: &mut Vec<(String, String, bool)>, name: &str| {
        if !name.is_empty() && !people.iter().any(|(known, ..)| known == name) {
            people.push((name.to_owned(), name.to_owned(), false));
        }
    };
    // What each `end` closes: a part, or a box of people, which is not
    // drawn.
    let mut opened: Vec<bool> = Vec::new();
    for line in body(text) {
        let lower = line.to_lowercase();
        let word = lower.split_whitespace().next().unwrap_or("");
        let rest = line[word.len().min(line.len())..].trim();
        match word {
            "title" | "title:" => {
                sequence.title = plain(rest.trim_start_matches(':'));
                continue;
            }
            "autonumber" => {
                sequence.numbered = !rest.eq_ignore_ascii_case("off");
                continue;
            }
            "participant" | "actor" | "create" => {
                let (actor, rest) = match word {
                    "create" => {
                        let kind = rest.split_whitespace().next().unwrap_or("").to_lowercase();
                        (kind == "actor", rest[kind.len().min(rest.len())..].trim())
                    }
                    _ => (word == "actor", rest),
                };
                let rest = rest.split("@{").next().unwrap_or(rest).trim();
                let (name, alias) = rest.split_once(" as ").map_or((rest, rest), |(name, alias)| (name.trim(), alias.trim()));
                let name = plain(name);
                match sequence.people.iter_mut().find(|(known, ..)| *known == name) {
                    Some(known) => (known.1, known.2) = (plain(alias), actor),
                    None => sequence.people.push((name, plain(alias), actor)),
                }
                continue;
            }
            "destroy" | "links" | "link" | "properties" | "details" => continue,
            "box" => {
                opened.push(false);
                continue;
            }
            "note" => {
                if let Some((place, words)) = line.split_once(':') {
                    let place = place.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
                    let lowered = place.to_lowercase();
                    let (side, who) = ["right of", "left of", "over"].iter().find_map(|side| lowered.starts_with(side).then(|| (*side, place[side.len()..].trim()))).unwrap_or(("over", place.as_str()));
                    let who: Vec<String> = who.split(',').map(|name| plain(name)).filter(|name| !name.is_empty()).collect();
                    for name in &who {
                        person(&mut sequence.people, name);
                    }
                    sequence.steps.push(Step::Note(format!("{side} {}", who.join(",")), plain(words)));
                }
                continue;
            }
            "alt" | "loop" | "opt" | "par" | "critical" | "break" | "rect" => {
                opened.push(true);
                let fill = (word == "rect").then(|| rgb_of(rest)).flatten();
                let title = if word == "rect" { String::new() } else if rest.is_empty() { word.to_owned() } else { format!("{word} [{}]", plain(rest)) };
                sequence.steps.push(Step::Open(title, fill));
                continue;
            }
            "else" | "and" | "option" => {
                sequence.steps.push(Step::Branch(if rest.is_empty() { String::new() } else { format!("[{}]", plain(rest)) }));
                continue;
            }
            "end" => {
                if opened.pop().unwrap_or(true) {
                    sequence.steps.push(Step::Close);
                }
                continue;
            }
            "activate" => {
                sequence.steps.push(Step::Busy(plain(rest)));
                continue;
            }
            "deactivate" => {
                sequence.steps.push(Step::Done(plain(rest)));
                continue;
            }
            _ => {}
        }
        // A message: from, its arrow, to, and its words after a colon.
        const ARROWS: [(&str, bool, Option<u8>, bool); 12] = [
            ("<<-->>", true, Some(shapes::FILLED_HEAD), true),
            ("<<->>", false, Some(shapes::FILLED_HEAD), true),
            ("-->>", true, Some(shapes::FILLED_HEAD), false),
            ("->>", false, Some(shapes::FILLED_HEAD), false),
            ("--x", true, Some(shapes::CROSS_HEAD), false),
            ("-x", false, Some(shapes::CROSS_HEAD), false),
            ("--)", true, Some(shapes::OPEN_HEAD), false),
            ("-)", false, Some(shapes::OPEN_HEAD), false),
            ("-->", true, None, false),
            ("->", false, None, false),
            ("--", true, None, false),
            ("-", false, None, false),
        ];
        let (head_part, words) = line.split_once(':').map_or((line.as_str(), ""), |(head, words)| (head, words));
        let found = ARROWS.iter().filter_map(|arrow| head_part.find(arrow.0).map(|at| (at, arrow))).min_by_key(|(at, arrow)| (*at, std::cmp::Reverse(arrow.0.len())));
        let Some((at, &(arrow, dashed, head, both))) = found else { continue };
        let from = plain(&head_part[..at]);
        let to_part = head_part[at + arrow.len()..].trim();
        let (busy, done) = (to_part.starts_with('+'), to_part.starts_with('-'));
        let to = plain(to_part.trim_start_matches(['+', '-']));
        if from.is_empty() || to.is_empty() {
            continue;
        }
        person(&mut sequence.people, &from);
        person(&mut sequence.people, &to);
        sequence.steps.push(Step::Message { from, to, words: plain(words), dashed, head, both, busy, done });
    }
    sequence
}

/// A color as `rgb(r, g, b)` or `rgba(...)` gives it, as the board's
/// nearest fill.
fn rgb_of(words: &str) -> Option<u32> {
    let inner = words.split_once('(')?.1.split(')').next()?;
    let parts: Vec<u32> = inner.split(',').filter_map(|part| part.trim().parse::<f32>().ok()).map(|value| value.clamp(0.0, 255.0) as u32).collect();
    let [r, g, b, ..] = parts[..] else { return None };
    Some(board_fill(r << 16 | g << 8 | b))
}

/// The room a sequence keeps: between people at least, over a message's
/// words, under its line, and how far a busy bar is across.
const APART: f32 = 40.0;
const OVER: f32 = 8.0;
const UNDER: f32 = 22.0;
const BAR: f32 = 10.0;

impl Studio {
    pub(super) fn build_sequence(&mut self, text: &str, origin: Point) {
        let sequence = read_sequence(text);
        self.make_sequence(&sequence, origin);
    }

    /// A sequence made: a box round it that keeps it laid out, its people
    /// along its top and its foot, a dashed line down from each, and what
    /// happens down them in order, each tied to whom it is between.
    fn make_sequence(&mut self, sequence: &Sequence, origin: Point) {
        let title = if sequence.title.is_empty() { "Sequence" } else { sequence.title.as_str() };
        let frame = self.diagram_box(title, Form::Box, origin);
        if let Some(shape) = self.shape_mut(frame) {
            (shape.titled, shape.heading, shape.filled, shape.dashed, shape.color, shape.region, shape.sketch) = (true, true, false, true, INK, SEQUENCE, false);
            shape.votes = u8::from(sequence.numbered);
            (shape.base, shape.stroke, shape.font_px) = ((1.0, 1.0), 1.0, 22);
        }
        let holder = self.id_of(frame);
        let mut heads: Vec<(String, u32)> = Vec::new();
        for (at, (name, words, actor)) in sequence.people.iter().enumerate() {
            let middle = (origin.0 + at as f32 * 200.0, origin.1);
            let index = self.diagram_box(words, if *actor { Form::Actor } else { Form::Box }, middle);
            let id = self.id_of(index);
            self.nest(id, holder);
            heads.push((name.clone(), id));
        }
        // The same along its foot, and a lifeline from each to its own.
        let mut feet = Vec::new();
        for (at, (_, words, actor)) in sequence.people.iter().enumerate() {
            let index = self.diagram_box(words, if *actor { Form::Actor } else { Form::Box }, (origin.0 + at as f32 * 200.0, origin.1 + 300.0));
            if let Some(shape) = self.shape_mut(index) {
                (shape.region, shape.number) = (FOOT, heads[at].1);
            }
            let id = self.id_of(index);
            self.nest(id, holder);
            feet.push(id);
        }
        for (&(_, head), &foot) in heads.iter().zip(&feet) {
            let line = self.diagram_line((0.0, 0.0), (0.0, 1.0), false, true);
            if let Some(shape) = self.shape_mut(line) {
                (shape.color, shape.region, shape.sketch, shape.stuck) = (QUIET, FOOT, false, [Some((head, 0.0, 1.0)), Some((foot, 0.0, -1.0))]);
            }
            let id = self.id_of(line);
            self.nest(id, holder);
        }
        let find = |name: &str| heads.iter().find(|(known, _)| known == name).map(|&(_, id)| id);
        // Boxes round parts, lines between branches and busy bars go under
        // the messages, made after them and put back here.
        let under = self.doc.layers.len();
        // Each with the order it is put in: parts by when they opened, so
        // that one round another is under it, then the lines between
        // branches, then the bars.
        let mut later: Vec<((u8, u32), Layer)> = Vec::new();
        let mut opened = 0u32;
        let mut parts: Vec<(String, Option<u32>, u32, u32)> = Vec::new();
        let mut busy: Vec<(u32, u32)> = Vec::new();
        let mut order: u32 = 0;
        let mut last_message: Option<u32> = None;
        // A busy bar on someone's lifeline, which it keeps to by being
        // nearest it.
        let bar = |studio: &mut Studio, later: &mut Vec<((u8, u32), Layer)>, who: u32, from: u32, to: u32| {
            let across = studio.index_of(who).map_or(origin.0, |index| {
                let (x, _, w, _) = studio.box_frame(index);
                x + w / 2.0
            });
            let index = studio.diagram_box("", Form::Box, origin);
            if let Some(shape) = studio.shape_mut(index) {
                (shape.region, shape.number, shape.votes, shape.fill, shape.filled, shape.sketch) = (BUSY, from, (to.saturating_sub(from)).min(255) as u8, 0xFFFFFF, true, false);
                shape.points = vec![(across - BAR / 2.0, origin.1), (across + BAR / 2.0, origin.1 + 20.0)];
            }
            later.push(((0, from), studio.doc.layers.remove(index)));
        };
        // Each step put a little under the one before, so that they are
        // in order down from the first time it is laid out.
        let beat = |order: u32| origin.1 + 80.0 + order as f32 * 40.0;
        for step in &sequence.steps {
            match step {
                Step::Message { from, to, words, dashed, head, both, busy: starts, done } => {
                    let (Some(a), Some(b)) = (find(from), find(to)) else { continue };
                    let arrow = self.diagram_line((origin.0, beat(order)), (origin.0 + 1.0, beat(order)), head.is_some(), *dashed);
                    let down = |studio: &Studio, id: u32| studio.index_of(id).map_or(2.0, |index| {
                        let (_, y, _, h) = studio.box_frame(index);
                        (beat(order) - (y + h / 2.0)) / (h / 2.0).max(1.0)
                    });
                    let (va, vb) = (down(self, a), down(self, b));
                    if let Some(shape) = self.shape_mut(arrow) {
                        shape.text = words.clone();
                        (shape.number, shape.titled) = (order, true);
                        shape.heads = [*both, head.is_some()];
                        shape.ends = [shapes::FILLED_HEAD, head.unwrap_or(shapes::FILLED_HEAD)];
                        shape.stuck = [Some((a, 0.0, va)), Some((b, 0.0, vb))];
                    }
                    let id = self.id_of(arrow);
                    self.nest(id, holder);
                    if *starts {
                        busy.push((b, order));
                    }
                    if *done {
                        if let Some(at) = busy.iter().rposition(|&(who, _)| who == a) {
                            let (_, from_step) = busy.remove(at);
                            bar(self, &mut later, a, from_step, order);
                        }
                    }
                    last_message = Some(order);
                    order += 1;
                }
                Step::Note(place, words) => {
                    let index = self.diagram_box(words, Form::Note, (origin.0, beat(order)));
                    if let Some(shape) = self.shape_mut(index) {
                        (shape.number, shape.shadow) = (order, None);
                    }
                    self.doc.layers[index].named = Some(self.note_place_words(place, &sequence.people));
                    let id = self.id_of(index);
                    self.nest(id, holder);
                    order += 1;
                }
                Step::Open(title, fill) => {
                    parts.push((title.clone(), *fill, order, opened));
                    opened += 1;
                }
                Step::Branch(words) => {
                    let line = self.diagram_line((0.0, 0.0), (1.0, 0.0), false, true);
                    if let Some(shape) = self.shape_mut(line) {
                        (shape.text, shape.region, shape.number, shape.color, shape.sketch, shape.font_px) = (words.clone(), BRANCH, order, QUIET, false, 16);
                    }
                    later.push(((2, order), self.doc.layers.remove(line)));
                }
                Step::Close => {
                    if let Some((title, fill, from, when)) = parts.pop() {
                        let index = self.diagram_box(&title, Form::Box, origin);
                        if let Some(shape) = self.shape_mut(index) {
                            (shape.titled, shape.heading, shape.dashed, shape.sketch, shape.color, shape.stroke) = (true, true, false, false, INK, 1.25);
                            (shape.filled, shape.fill) = (fill.is_some(), fill.unwrap_or(0xFFFFFF));
                            (shape.number, shape.votes, shape.form, shape.status, shape.region, shape.font_px) = (from, (order - from).min(255) as u8, shapes::FORM_BOX, 0, FRAGMENT, 16);
                        }
                        later.push(((1, when), self.doc.layers.remove(index)));
                    }
                }
                Step::Busy(who) => {
                    if let Some(id) = find(who) {
                        busy.push((id, last_message.unwrap_or(order)));
                    }
                }
                Step::Done(who) => {
                    if let Some(id) = find(who) {
                        if let Some(at) = busy.iter().rposition(|&(known, _)| known == id) {
                            let (_, from_step) = busy.remove(at);
                            bar(self, &mut later, id, from_step, last_message.unwrap_or(order));
                        }
                    }
                }
            }
        }
        for (who, from_step) in std::mem::take(&mut busy) {
            bar(self, &mut later, who, from_step, order.saturating_sub(1).max(from_step));
        }
        // The bars, then the parts, the outermost first, under what they
        // hold; the lines between branches over them.
        later.sort_by_key(|(key, _)| *key);
        for (k, (_, layer)) in later.into_iter().enumerate() {
            let id = stacks::layer_id(&layer);
            self.doc.layers.insert(under + k, layer);
            self.nest(id, holder);
        }
        // What is tied followed once, so that it is all in order down.
        self.follow();
        self.changed();
    }

    /// Where a note goes, as Mermaid says it, by the words of its people.
    fn note_place_words(&self, place: &str, people: &[(String, String, bool)]) -> String {
        let (side, who) = ["right of", "left of", "over"].iter().find_map(|side| place.strip_prefix(side).map(|who| (*side, who.trim()))).unwrap_or(("over", place));
        let words: Vec<String> = who.split(',').map(|name| people.iter().find(|(known, ..)| known == name).map_or(name.to_owned(), |(_, words, _)| words.clone())).collect();
        format!("{side} {}", words.join(","))
    }

    /// A sequence laid out: its people along its top in the order they are
    /// across, spaced by what goes between them, and along its foot; what
    /// happens down from them in order at an even beat, each tied to whom
    /// it is between; a box round each part round the steps it holds, a
    /// line across between its branches, and the busy bars; the box round
    /// it all hugging it.
    pub(in crate::studio) fn arrange_sequence(&mut self, holder: u32, dragged: &[u32]) {
        let Some(at) = self.index_of(holder) else { return };
        let Kind::Shape(frame) = &self.doc.layers[at].kind else { return };
        let (x, y, _, _) = frame.frame();
        let numbered = frame.votes > 0;
        let title_room = Text::line_height(frame.font_px) as f32 + 10.0;
        let members: Vec<u32> = self.doc.stacks.iter().find(|stack| stack.holder == Some(holder)).map(|stack| stack.members.clone()).unwrap_or_default();
        let mut all: Vec<(u32, usize, Shape)> = members.iter().filter_map(|&id| self.index_of(id).and_then(|index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some((id, index, shape.clone())),
            Kind::Picture(_) => None,
        })).collect();
        all.retain(|(_, index, _)| !self.doc.layers[*index].hidden);
        let is_person = |shape: &Shape| matches!(shape.tool, Tool::Rect | Tool::Ellipse) && shape.form != shapes::FORM_STICKY && shape.region == 0;
        let mut people: Vec<(u32, usize, Shape)> = all.iter().filter(|(_, _, shape)| is_person(shape)).cloned().collect();
        people.sort_by(|a, b| (a.2.frame().0 + a.2.frame().2 / 2.0).total_cmp(&(b.2.frame().0 + b.2.frame().2 / 2.0)));
        if people.is_empty() {
            return;
        }
        let px = frame.font_px;
        let high = Text::line_height(px) as f32;
        let small = 16;
        let small_high = Text::line_height(small) as f32;
        // What happens, in order down: messages and notes by where they are.
        let mut steps: Vec<(u32, usize, Shape)> = all.iter().filter(|(_, _, shape)| (shape.sticks() && shape.region == 0) || shape.form == shapes::FORM_STICKY).cloned().collect();
        let level_of = |shape: &Shape| if shape.sticks() { shape.middle().1 } else { shape.frame().1 + shape.frame().3 / 2.0 };
        steps.sort_by(|a, b| level_of(&a.2).total_cmp(&level_of(&b.2)).then(a.2.number.cmp(&b.2.number)));
        let person_of = |id: u32| people.iter().position(|(known, ..)| *known == id);
        let nearest = |x: f32| (0..people.len()).min_by(|&p, &q| ((people[p].2.frame().0 + people[p].2.frame().2 / 2.0) - x).abs().total_cmp(&((people[q].2.frame().0 + people[q].2.frame().2 / 2.0) - x).abs())).unwrap_or(0);
        // Whom each step is between, and how wide its words are.
        #[derive(Clone, Copy)]
        enum Between {
            Message(usize, usize),
            Note(u8, usize, usize),
        }
        let mut between: Vec<Between> = Vec::new();
        let mut widths: Vec<(f32, f32)> = Vec::new();
        for (_, index, shape) in &steps {
            if shape.sticks() {
                let ends = match (shape.stuck[0].and_then(|(id, ..)| person_of(id)), shape.stuck[1].and_then(|(id, ..)| person_of(id))) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (nearest(shape.points[0].0), nearest(shape.points[1].0)),
                };
                between.push(Between::Message(ends.0, ends.1));
                let wide = if shape.text.is_empty() { 0.0 } else { self.words_wide(&shape.text, shape.font_px) };
                widths.push((wide, if shape.text.is_empty() { 0.0 } else { Self::words_tall(&shape.text, shape.font_px) }));
            } else {
                let place = self.doc.layers[*index].named.clone().unwrap_or_default();
                let (side, who) = ["right of", "left of", "over"].iter().enumerate().find_map(|(at, side)| place.strip_prefix(side).map(|who| (at as u8, who.trim().to_owned()))).unwrap_or((0, String::new()));
                let found: Vec<usize> = who.split(',').filter_map(|name| people.iter().position(|(_, _, person)| person.text == name.trim())).collect();
                let (a, b) = match (found.first(), found.last()) {
                    (Some(&a), Some(&b)) => (a.min(b), a.max(b)),
                    _ => {
                        let near = nearest(shape.frame().0);
                        (near, near)
                    }
                };
                between.push(Between::Note(side, a, b));
                let (_, _, w, h) = shape.frame();
                widths.push((w, h));
            }
        }
        // Where each person is across: as far from the one before as both
        // need, and as each message, loop and note between them needs.
        let count = people.len();
        let half: Vec<f32> = people.iter().map(|person| person.2.frame().2 / 2.0).collect();
        let mut need: Vec<Vec<(usize, f32)>> = vec![Vec::new(); count];
        let mut left_room: f32 = 0.0;
        let mut right_room: f32 = 0.0;
        for (step, how) in between.iter().enumerate() {
            let (wide, _) = widths[step];
            match *how {
                Between::Message(a, b) if a != b => need[a.max(b)].push((a.min(b), wide + 2.0 * BAR + 28.0)),
                Between::Message(a, _) => {
                    let reach = Self::loop_reach(wide) + wide / 2.0 + 16.0 + BAR;
                    if a + 1 < count {
                        need[a + 1].push((a, reach + half[a + 1].min(20.0)));
                    } else {
                        right_room = right_room.max(reach - half[a]);
                    }
                }
                Between::Note(0, a, _) => {
                    if a + 1 < count {
                        need[a + 1].push((a, wide + 24.0 + BAR));
                    } else {
                        right_room = right_room.max(wide + 12.0 + BAR - half[a]);
                    }
                }
                Between::Note(1, a, _) => {
                    if a > 0 {
                        need[a].push((a - 1, wide + 24.0 + BAR));
                    } else {
                        left_room = left_room.max(wide + 12.0 + BAR - half[a]);
                    }
                }
                Between::Note(_, a, b) => {
                    if a == b {
                        if a + 1 < count {
                            need[a + 1].push((a, wide / 2.0 + 16.0));
                        }
                        if a > 0 {
                            need[a].push((a - 1, wide / 2.0 + 16.0));
                        }
                        right_room = right_room.max(wide / 2.0 - half[a]);
                        left_room = left_room.max(wide / 2.0 - half[a]);
                    } else {
                        need[b].push((a, wide - 40.0));
                    }
                }
            }
        }
        let mut centers = vec![0.0f32; count];
        for j in 1..count {
            let mut at = centers[j - 1] + half[j - 1] + APART + half[j];
            for &(i, wide) in &need[j] {
                at = at.max(centers[i] + wide);
            }
            centers[j] = at;
        }
        // The parts; one round another from the same step is under it.
        let mut parts: Vec<(u32, usize, Shape)> = all.iter().filter(|(_, _, shape)| shape.region == FRAGMENT).cloned().collect();
        parts.sort_by_key(|part| part.1);
        // Where they are across is settled once the rest is: all of it is
        // moved across then to keep its room from the box's left.
        let shift = x + PAD + half[0] + left_room;
        for center in &mut centers {
            *center += shift;
        }
        // The people along the top, their tops level.
        let top = y + title_room + PAD;
        let tallest = people.iter().map(|person| person.2.frame().3).fold(0.0, f32::max);
        for (k, (id, index, shape)) in people.iter().enumerate() {
            let (fx, fy, fw, _) = shape.frame();
            if !dragged.contains(id) {
                self.doc.layers[*index].translate(centers[k] - (fx + fw / 2.0), top - fy);
            }
        }
        let head_middle = |k: usize| (top + people[k].2.frame().3 / 2.0, people[k].2.frame().3 / 2.0);
        // Down: each step at its beat, with rows for the tabs of parts as
        // they open, for branches, and room as parts close.
        let branches: Vec<(u32, usize, Shape)> = all.iter().filter(|(_, _, shape)| shape.region == BRANCH).cloned().collect();
        let bars: Vec<(u32, usize, Shape)> = all.iter().filter(|(_, _, shape)| shape.region == BUSY).cloned().collect();
        let mut cursor = top + tallest + 24.0;
        let mut level = vec![0.0f32; steps.len()];
        let mut part_top = vec![0.0f32; parts.len()];
        let mut part_bottom = vec![0.0f32; parts.len()];
        let mut branch_at = vec![0.0f32; branches.len()];
        let tab = small_high + 12.0;
        let closes_after = |k: usize, part: &Shape| part.number as usize + part.votes as usize == k + 1 || (part.votes == 0 && part.number as usize == k);
        for k in 0..=steps.len() {
            // Parts opening here, outermost first.
            let mut opening: Vec<usize> = (0..parts.len()).filter(|&p| parts[p].2.number as usize == k && (parts[p].2.votes > 0 || k == steps.len())).collect();
            opening.sort_by_key(|&p| (std::cmp::Reverse(parts[p].2.votes), p));
            for p in opening {
                part_top[p] = cursor;
                cursor += if parts[p].2.text.is_empty() { 10.0 } else { tab + 6.0 };
            }
            for (b, branch) in branches.iter().enumerate() {
                if branch.2.number as usize == k {
                    branch_at[b] = cursor + 4.0;
                    cursor += 8.0 + if branch.2.text.is_empty() { 0.0 } else { small_high + 4.0 };
                }
            }
            if k == steps.len() {
                break;
            }
            let (_, tall) = widths[k];
            match between[k] {
                Between::Message(a, b) if a != b => {
                    cursor += tall.max(high * 0.6) + OVER;
                    level[k] = cursor;
                    cursor += UNDER;
                }
                Between::Message(..) => {
                    cursor += tall.max(high * 0.6) + OVER;
                    level[k] = cursor;
                    cursor += Self::LOOP_TALL + UNDER;
                }
                Between::Note(..) => {
                    cursor += 6.0;
                    level[k] = cursor;
                    cursor += tall + 16.0;
                }
            }
            // Parts closing after it, innermost first.
            let mut closing: Vec<usize> = (0..parts.len()).filter(|&p| parts[p].2.votes > 0 && closes_after(k, &parts[p].2)).collect();
            closing.sort_by_key(|&p| (parts[p].2.votes, std::cmp::Reverse(p)));
            for p in closing {
                part_bottom[p] = cursor + 2.0;
                cursor += 14.0;
            }
        }
        let foot_top = cursor + 16.0;
        // Who is busy at each step, how deep, for where messages meet them.
        let busy_at = |person: usize, step: usize| -> usize {
            bars.iter().filter(|(_, _, bar)| {
                let near = nearest(bar.frame().0 + bar.frame().2 / 2.0);
                near == person && (bar.number as usize) <= step && step <= bar.number as usize + bar.votes as usize
            }).count()
        };
        let edge = |person: usize, step: usize, toward: f32| {
            let deep = busy_at(person, step);
            if deep == 0 { 0.0 } else { toward.signum() * (BAR / 2.0 + (deep - 1) as f32 * BAR / 2.0) }
        };
        // Each message tied to whom it is between at its level; each note
        // beside them or over them.
        let mut number = 0u8;
        let mut extents: Vec<(f32, f32)> = vec![(0.0, 0.0); steps.len()];
        for (k, (id, index, shape)) in steps.iter().enumerate() {
            match between[k] {
                Between::Message(a, b) => {
                    let (ca, cb) = (centers[a], centers[b]);
                    let (mid_a, half_a) = head_middle(a);
                    let (mid_b, half_b) = head_middle(b);
                    let v = |mid: f32, half: f32, at: f32| (at - mid) / half.max(1.0);
                    let (wide, _) = widths[k];
                    let u = |k_person: usize, offset: f32| offset / half[k_person].max(1.0);
                    let placed = &mut self.doc.layers[*index];
                    let Kind::Shape(arrow) = &mut placed.kind else { continue };
                    let (ida, idb) = (people[a].0, people[b].0);
                    arrow.number = k as u32;
                    arrow.routed = false;
                    arrow.titled = true;
                    arrow.points.truncate(2);
                    if numbered {
                        number = number.saturating_add(1);
                        arrow.votes = number;
                    } else {
                        arrow.votes = 0;
                    }
                    if a == b {
                        // A loop out to the right and back, its words beside it.
                        let off = edge(a, k, 1.0);
                        let reach = Self::loop_reach(wide);
                        let (from, to) = ((ca + off, level[k]), (ca + off, level[k] + Self::LOOP_TALL));
                        arrow.stuck = [Some((ida, u(a, off), v(mid_a, half_a, from.1))), Some((ida, u(a, off), v(mid_a, half_a, to.1)))];
                        arrow.points = vec![from, to, (ca + off + reach, level[k]), (ca + off + reach, level[k] + Self::LOOP_TALL)];
                        extents[k] = (ca, ca + off + reach + wide / 2.0 + 8.0);
                    } else {
                        let toward = cb - ca;
                        let (oa, ob) = (edge(a, k, toward), edge(b, k, -toward));
                        arrow.stuck = [Some((ida, u(a, oa), v(mid_a, half_a, level[k]))), Some((idb, u(b, ob), v(mid_b, half_b, level[k])))];
                        arrow.points = vec![(ca + oa, level[k]), (cb + ob, level[k])];
                        extents[k] = (ca.min(cb), ca.max(cb));
                    }
                    arrow.touch();
                    let _ = id;
                }
                Between::Note(side, a, b) => {
                    let (fx, fy, fw, fh) = shape.frame();
                    let left = match side {
                        0 => centers[a] + BAR / 2.0 + 8.0,
                        1 => centers[a] - BAR / 2.0 - 8.0 - fw,
                        _ if a == b => centers[a] - fw / 2.0,
                        _ => centers[a] - 20.0,
                    };
                    let wide = if side == 2 && a != b { (centers[b] - centers[a] + 40.0).max(fw) } else { fw };
                    let left = if side == 2 && a != b { (centers[a] + centers[b]) / 2.0 - wide / 2.0 } else { left };
                    if !dragged.contains(id) {
                        if let Kind::Shape(note) = &mut self.doc.layers[*index].kind {
                            note.points = vec![(left, level[k]), (left + wide, level[k] + fh)];
                            note.number = k as u32;
                            note.touch();
                        }
                    }
                    let _ = (fx, fy);
                    extents[k] = (left, left + wide);
                }
            }
        }
        // The boxes round parts: round the people their steps are between,
        // past each part inside them; their tabs no narrower than their words.
        let mut part_x: Vec<(f32, f32)> = vec![(f32::MAX, f32::MIN); parts.len()];
        let mut by_size: Vec<usize> = (0..parts.len()).collect();
        by_size.sort_by_key(|&p| (parts[p].2.votes, std::cmp::Reverse(p)));
        for &p in &by_size {
            let (from, count_of) = (parts[p].2.number as usize, parts[p].2.votes as usize);
            let mut span = (f32::MAX, f32::MIN);
            for k in from..(from + count_of).min(steps.len()) {
                span = (span.0.min(extents[k].0), span.1.max(extents[k].1));
            }
            for &q in &by_size {
                let (inner_from, inner_count) = (parts[q].2.number as usize, parts[q].2.votes as usize);
                if q != p && inner_from >= from && inner_from + inner_count <= from + count_of && ((inner_count, inner_from) != (count_of, from) || q > p) && part_x[q].0 < f32::MAX {
                    span = (span.0.min(part_x[q].0 - 10.0), span.1.max(part_x[q].1 + 10.0));
                }
            }
            if span.0 == f32::MAX {
                span = (centers[0], centers[0]);
            }
            let words = if parts[p].2.text.is_empty() { 0.0 } else { self.words_wide(&parts[p].2.text, small) + 24.0 };
            let (left, right) = (span.0 - 22.0, (span.1 + 22.0).max(span.0 - 22.0 + words));
            part_x[p] = (left, right);
            if let Kind::Shape(box_) = &mut self.doc.layers[parts[p].1].kind {
                box_.points = vec![(left, part_top[p]), (right, part_bottom[p].max(part_top[p] + tab + 10.0))];
                box_.base = (right - left, box_.points[1].1 - part_top[p]);
                box_.touch();
            }
        }
        // A line across its part between its branches.
        for (b, (_, index, branch)) in branches.iter().enumerate() {
            let owner = (0..parts.len()).filter(|&p| parts[p].2.number <= branch.number && branch.number < parts[p].2.number + u32::from(parts[p].2.votes).max(1)).min_by_key(|&p| parts[p].2.votes);
            let (left, right) = owner.map_or((centers[0] - 30.0, centers[count - 1] + 30.0), |p| part_x[p]);
            if let Kind::Shape(line) = &mut self.doc.layers[*index].kind {
                line.points = vec![(left, branch_at[b]), (right, branch_at[b])];
                (line.titled, line.stuck) = (false, [None, None]);
                line.touch();
            }
        }
        // The busy bars, from the step that began it to the one that ended it.
        let mut stacked: Vec<(usize, f32, f32)> = Vec::new();
        for (_, index, bar) in &bars {
            let person = nearest(bar.frame().0 + bar.frame().2 / 2.0);
            let from = (bar.number as usize).min(steps.len().saturating_sub(1));
            let to = (bar.number as usize + bar.votes as usize).min(steps.len().saturating_sub(1));
            let (top_at, bottom_at) = (level.get(from).copied().unwrap_or(cursor) - 4.0, level.get(to).copied().unwrap_or(cursor) + 4.0 + if matches!(between.get(to), Some(Between::Message(a, b)) if a == b) { Self::LOOP_TALL } else { 0.0 });
            let deep = stacked.iter().filter(|(who, a, b)| *who == person && *a <= top_at && top_at <= *b).count() as f32;
            stacked.push((person, top_at, bottom_at));
            let left = centers[person] - BAR / 2.0 + deep * BAR / 2.0;
            if let Kind::Shape(shape) = &mut self.doc.layers[*index].kind {
                shape.points = vec![(left, top_at), (left + BAR, bottom_at.max(top_at + 16.0))];
                shape.touch();
            }
        }
        // The people again along the foot, under each its own.
        for (_, index, foot) in all.iter().filter(|(_, _, shape)| shape.region == FOOT && matches!(shape.tool, Tool::Rect | Tool::Ellipse)) {
            let Some(k) = people.iter().position(|(id, ..)| *id == foot.number) else { continue };
            let (fx, fy, fw, _) = foot.frame();
            let head = &people[k].2;
            self.doc.layers[*index].translate(centers[k] - (fx + fw / 2.0), foot_top - fy);
            if let Kind::Shape(shape) = &mut self.doc.layers[*index].kind {
                if shape.text != head.text {
                    shape.text = head.text.clone();
                }
            }
        }
        // The box round it all, hugging it: its own size what it holds needs.
        let lowest = all.iter().filter(|(_, _, shape)| shape.region == FOOT && matches!(shape.tool, Tool::Rect | Tool::Ellipse)).map(|(_, _, shape)| foot_top + shape.frame().3).fold(foot_top, f32::max);
        let rightmost = (0..count).map(|k| centers[k] + half[k]).chain(extents.iter().map(|extent| extent.1)).chain(part_x.iter().map(|extent| extent.1)).fold(f32::MIN, f32::max).max(centers[count - 1] + half[count - 1] + right_room);
        // All of it moved across to keep its own room from the box's left,
        // whatever reaches furthest that way.
        let leftmost = (0..count).map(|k| centers[k] - half[k]).chain(extents.iter().map(|extent| extent.0)).chain(part_x.iter().map(|extent| extent.0)).fold(f32::MAX, f32::min);
        let across = x + PAD - leftmost;
        if across.abs() > 0.01 {
            for (id, index, _) in &all {
                if !dragged.contains(id) {
                    self.doc.layers[*index].translate(across, 0.0);
                }
            }
        }
        let rightmost = rightmost + across;
        if let Kind::Shape(box_) = &mut self.doc.layers[at].kind {
            let size = (rightmost + PAD - x, lowest + PAD - y);
            box_.points = vec![(x, y), (x + size.0, y + size.1)];
            box_.base = size;
            box_.touch();
        }
    }

    /// How tall a message to its own sender loops.
    const LOOP_TALL: f32 = 30.0;

    /// How far out a message to its own sender loops: far enough for its
    /// words, set on its far side, to clear the lifeline.
    fn loop_reach(wide: f32) -> f32 {
        (wide / 2.0 + 14.0).max(34.0)
    }

    /// The sequence diagram a sequence on the board says.
    pub(super) fn sequence_mermaid(&self, holder: u32) -> String {
        let members = self.inside(holder);
        let shape_of = |id: u32| self.index_of(id).and_then(|index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some((index, shape)),
            Kind::Picture(_) => None,
        });
        let mut people: Vec<(u32, f32, bool)> = members.iter().filter_map(|&id| shape_of(id).and_then(|(_, shape)| (matches!(shape.tool, Tool::Rect | Tool::Ellipse) && shape.form != shapes::FORM_STICKY && shape.region == 0).then(|| (id, shape.frame().0, shape.form == shapes::FORM_STICK)))).collect();
        people.sort_by(|a, b| a.1.total_cmp(&b.1));
        let words_of = |id: u32| shape_of(id).map_or(String::new(), |(_, shape)| shape.text.replace('\n', "<br/>"));
        let name = |id: u32| format!("P{}", people.iter().position(|(known, ..)| *known == id).unwrap_or(0) + 1);
        let mut out = "sequenceDiagram\n".to_owned();
        if let Some((_, frame)) = shape_of(holder) {
            if frame.text != "Sequence" && !frame.text.is_empty() {
                out += &format!("    title {}\n", frame.text);
            }
            if frame.votes > 0 {
                out += "    autonumber\n";
            }
        }
        for &(id, _, actor) in &people {
            out += &format!("    {} {} as {}\n", if actor { "actor" } else { "participant" }, name(id), words_of(id));
        }
        // What happens, by where it is down, with the parts round it.
        let mut lines: Vec<(f32, u8, String)> = Vec::new();
        for &id in &members {
            let Some((index, shape)) = shape_of(id) else { continue };
            match shape.region {
                0 if shape.sticks() => {
                    if let (Some((a, ..)), Some((b, ..))) = (shape.stuck[0], shape.stuck[1]) {
                        let arrow = match (shape.heads[1], shape.ends[1], shape.dashed) {
                            (false, _, false) => "->",
                            (false, _, true) => "-->",
                            (true, shapes::CROSS_HEAD, dashed) => if dashed { "--x" } else { "-x" },
                            (true, shapes::OPEN_HEAD, dashed) => if dashed { "--)" } else { "-)" },
                            (true, _, dashed) => if dashed { "-->>" } else { "->>" },
                        };
                        let arrow = if shape.heads[0] { format!("<<{arrow}") } else { arrow.to_owned() };
                        lines.push((shape.points[0].1, 1, format!("{}{arrow}{}: {}", name(a), name(b), shape.text.replace('\n', " "))));
                    }
                }
                0 if shape.form == shapes::FORM_STICKY => {
                    let place = self.doc.layers[index].named.clone().unwrap_or_default();
                    let (side, who) = ["right of", "left of", "over"].iter().find_map(|side| place.strip_prefix(side).map(|who| (*side, who.trim().to_owned()))).unwrap_or(("right of", String::new()));
                    let names: Vec<String> = who.split(',').filter_map(|words| people.iter().find(|(person, ..)| words_of(*person) == words.trim()).map(|&(person, ..)| name(person))).collect();
                    let names = if names.is_empty() { people.first().map_or(String::new(), |&(person, ..)| name(person)) } else { names.join(",") };
                    lines.push((shape.frame().1, 1, format!("Note {side} {names}: {}", shape.text.replace('\n', " "))));
                }
                FRAGMENT => {
                    let (_, top, _, h) = shape.frame();
                    let (word, rest) = shape.text.split_once(' ').unwrap_or((shape.text.as_str(), ""));
                    let opener = if shape.text.is_empty() { "rect rgb(240, 240, 255)".to_owned() } else { format!("{word} {}", rest.trim().trim_start_matches('[').trim_end_matches(']')) };
                    lines.push((top, 0, opener.trim().to_owned()));
                    lines.push((top + h, 2, "end".to_owned()));
                }
                BRANCH => lines.push((shape.points[0].1, 0, format!("else {}", shape.text.trim_start_matches('[').trim_end_matches(']')).trim().to_owned())),
                _ => {}
            }
        }
        lines.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut depth = 1;
        for (_, kind, line) in lines {
            if kind == 2 || line.starts_with("else") {
                depth = (depth - usize::from(kind == 2)).max(1);
            }
            out += &format!("{}{line}\n", "    ".repeat(if line.starts_with("else") { depth.saturating_sub(1).max(1) } else { depth }));
            if kind == 0 && !line.starts_with("else") {
                depth += 1;
            }
        }
        out
    }

    /// Tab on a message of a sequence: the next, back the other way, under
    /// it, its words typed. Whether there was one.
    pub(in crate::studio) fn next_message(&mut self) -> bool {
        let Some(index) = self.picked().or(self.editing) else { return false };
        let Kind::Shape(shape) = &self.doc.layers[index].kind else { return false };
        let id = self.id_of(index);
        let Some(holder) = self.holder_of(id) else { return false };
        let is_sequence = self.index_of(holder).is_some_and(|at| matches!(&self.doc.layers[at].kind, Kind::Shape(box_) if box_.region == SEQUENCE));
        if !is_sequence || !shape.sticks() || shape.region != 0 {
            return false;
        }
        let (Some(a), Some(b)) = (shape.stuck[0], shape.stuck[1]) else { return false };
        let below = shape.points[0].1.max(shape.points[1].1) + 30.0;
        let number = shape.number;
        self.commit_text();
        self.checkpoint();
        let arrow = self.diagram_line((0.0, below), (1.0, below), true, true);
        if let Some(made) = self.shape_mut(arrow) {
            made.stuck = [Some((b.0, 0.0, b.2 + 0.2)), Some((a.0, 0.0, a.2 + 0.2))];
            made.points = vec![(0.0, below), (1.0, below)];
            (made.number, made.titled) = (number + 1, true);
        }
        let made_id = self.id_of(arrow);
        self.nest(made_id, holder);
        self.changed();
        self.selected = vec![arrow];
        self.edit_text(arrow);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::check::*;
    use super::super::tests::shapes_of;
    use super::*;
    use crate::studio::board::tests::*;

    const SEQUENCE_TEXT: &str = "sequenceDiagram\n    participant A as Alice\n    actor J as John\n    A->>J: Hello John, how are you?\n    loop Every minute\n        J-->>A: Great!\n    end\n    Note right of J: Rational thoughts\n    A-xJ: Bye\n";

    /// Mermaid's own example, with every part a sequence has.
    const DOCS: &str = "sequenceDiagram\n    title Logging in\n    autonumber\n    participant C as Client\n    participant S as Server\n    participant D as DB\n    actor U as Admin\n    C->>+S: POST /login\n    S->>D: find user\n    D-->>S: user row\n    alt valid password\n        S-->>C: 200 token\n        S->>S: cache session\n    else wrong password\n        S--xC: 401\n    end\n    Note right of S: logs the attempt\n    Note over C,D: all over TLS\n    opt suspicious\n        loop three times\n            S-)U: alert\n        end\n    end\n    S-->>-C: done\n    Note left of C: shows a banner\n";

    #[test]
    fn a_sequence_is_people_along_the_top_and_messages_down_in_order_and_keeps_so() {
        let mut studio = board();
        studio.pasted(None, Some(SEQUENCE_TEXT.to_owned()));
        let shapes = shapes_of(&studio);
        let alice = shapes.iter().find(|shape| shape.text == "Alice" && shape.region == 0).unwrap().clone();
        let john = shapes.iter().find(|shape| shape.text == "John" && shape.region == 0).unwrap().clone();
        assert!(john.form == shapes::FORM_STICK && alice.frame().0 < john.frame().0 && (alice.frame().1 - john.frame().1).abs() < 1.0);
        let messages: Vec<Shape> = shapes.iter().filter(|shape| shape.sticks() && shape.region == 0).cloned().collect();
        assert_eq!(messages.len(), 3);
        let level = |name: &str| messages.iter().find(|shape| shape.text == name).unwrap().points[0].1;
        assert!(level("Hello John, how are you?") < level("Great!") && level("Great!") < level("Bye"));
        assert!(messages.iter().find(|shape| shape.text == "Great!").unwrap().dashed);
        assert!(shapes.iter().any(|shape| shape.text == "loop [Every minute]" && shape.region == FRAGMENT));
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_STICKY && shape.text == "Rational thoughts"));
        assert_eq!(shapes.iter().filter(|shape| shape.tool == Tool::Line && shape.region == FOOT).count(), 2, "a lifeline each");
        assert_eq!(shapes.iter().filter(|shape| shape.region == FOOT && shape.tool == Tool::Rect).count(), 2, "and each again at the foot");
        // A message dragged above the first goes first; the rest stay a step apart.
        studio.selected.clear();
        let bye = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "Bye")).unwrap();
        let hello_y = level("Hello John, how are you?");
        // Zoomed out, so that it is all in the window, clear of the bar of tools.
        studio.zoom = 0.6;
        let mid = shape(&studio, bye).middle();
        pull(&mut studio, mid, (mid.0, hello_y - 30.0), Mods::default());
        let now = shapes_of(&studio);
        let level = |name: &str| now.iter().find(|shape| shape.text == name).unwrap().points[0].1;
        assert!(level("Bye") < level("Hello John, how are you?"), "reordered: {:?} {:?} {:?} {:?}", level("Bye"), level("Hello John, how are you?"), (mid, hello_y), studio.selected);
        assert_clear(&mut studio);
        // Written back as a sequence diagram.
        studio.selected.clear();
        let text = studio.to_mermaid(false);
        assert!(text.starts_with("sequenceDiagram") && text.contains("as Alice") && text.contains("actor P2 as John") && text.contains("-->>") && text.contains("Note right of") && text.contains("loop Every minute"), "{text}");
        // Tab on a message: the next, back the other way.
        let hello = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text.starts_with("Hello"))).unwrap();
        studio.selected = vec![hello];
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Fine");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(shapes_of(&studio).iter().filter(|shape| shape.sticks() && shape.region == 0).count(), 4);
    }

    #[test]
    fn a_sequence_of_every_part_is_laid_out_clear_of_itself_in_a_box_that_hugs_it() {
        let read = read_sequence(DOCS);
        assert_eq!((read.title.as_str(), read.numbered, read.people.len()), ("Logging in", true, 4));
        assert!(read.people[3].2, "an actor");
        let mut studio = board();
        assert!(studio.bring_mermaid(DOCS));
        let shapes = shapes_of(&studio);
        let frame = shapes.iter().find(|shape| shape.region == SEQUENCE).unwrap().clone();
        assert_eq!(frame.text, "Logging in");
        // Numbered, a disc each.
        let messages: Vec<Shape> = shapes.iter().filter(|shape| shape.sticks() && shape.region == 0).cloned().collect();
        let mut numbers: Vec<u8> = messages.iter().map(|shape| shape.votes).collect();
        numbers.sort();
        assert_eq!(numbers, (1..=messages.len() as u8).collect::<Vec<_>>());
        // A loop back to itself goes out to the right and back.
        let own = messages.iter().find(|shape| shape.text == "cache session").unwrap();
        assert!(own.points.len() > 3 && own.points[2].0 > own.points[0].0 + 20.0);
        // The parts: alt with its branch line, opt round loop.
        let part = |words: &str| shapes.iter().find(|shape| shape.region == FRAGMENT && shape.text == words).unwrap().frame();
        let (alt, opt, inner) = (part("alt [valid password]"), part("opt [suspicious]"), part("loop [three times]"));
        assert!(opt.0 < inner.0 && opt.1 < inner.1 && opt.0 + opt.2 > inner.0 + inner.2 && opt.1 + opt.3 > inner.1 + inner.3, "opt round loop");
        let branch = shapes.iter().find(|shape| shape.region == BRANCH).unwrap();
        assert!(branch.points[0].1 > alt.1 && branch.points[0].1 < alt.1 + alt.3 && (branch.points[0].0 - alt.0).abs() < 1.0);
        // The alt box holds exactly its three messages: clear of the one
        // before it and the note after.
        let level = |words: &str| messages.iter().find(|shape| shape.text == words).unwrap().points[0].1;
        let inside = |y: f32| y > alt.1 && y < alt.1 + alt.3;
        assert!(inside(level("200 token")) && inside(level("cache session")) && inside(level("401")));
        assert!(!inside(level("user row")) && !inside(level("user row") - 24.0), "clear of the message over it and its words");
        // A busy bar on the Server, from POST to done.
        let bar = shapes.iter().find(|shape| shape.region == BUSY).unwrap().frame();
        assert!(bar.1 <= level("POST /login") && bar.1 + bar.3 >= level("done"));
        assert_clear(&mut studio);
        assert_labels_clear(&mut studio);
        assert_hugs(&mut studio, 40.0);
        // Its box hugs it: as tall as down to the feet and room.
        let feet_bottom = shapes.iter().filter(|shape| shape.region == FOOT && shape.tool != Tool::Line).map(|shape| shape.frame().1 + shape.frame().3).fold(0.0, f32::max);
        assert!((frame.frame().1 + frame.frame().3 - feet_bottom - PAD).abs() < 2.0);
    }
}
