//! Explaining on a board, to a room or on a call, and what each of an
//! engineer, a PM, someone in sales and a designer reaches for: F lights a
//! box and what it is joined to, or with two picked out only the paths
//! between them; a laser held on L that fades; Present walks the board's
//! frames or numbered parts one at a time; G gathers notes into a box
//! titled with what they share; D votes on a note; a 2x2, Now / Next /
//! Later lanes and Before / After come ready; and a box can lay out what
//! is put in it, in a row or as a tree.

use super::*;
use crate::canvas::Mask;
use std::time::{Duration, Instant};

/// How long a laser's trail lasts, and how long a box it clicked glows;
/// and how soon after L is let go it counts as let go, as a key held
/// down repeats itself by letting go and pressing again at once.
const TRAIL: f32 = 1.0;
const GLOW: f32 = 0.8;
const REPEAT: Duration = Duration::from_millis(90);
/// The laser's red.
const LASER: u32 = 0xFF2D2D;

/// The laser: while L is held, when it was pressed, when it was let go if
/// it has been, and whether the pointer has drawn with it.
#[derive(Clone, Debug)]
pub(in crate::studio) struct Laser {
    pub since: Instant,
    pub let_go: Option<Instant>,
    pub drawn: bool,
}

/// Presenting: the parts to walk through, which is shown, and how the
/// view was before.
#[derive(Clone, Debug)]
pub(in crate::studio) struct Present {
    pub steps: Vec<(String, (f32, f32, f32, f32))>,
    pub at: usize,
    pub view: (f32, (f32, f32)),
}

/// Words too common to title a group by.
const COMMON: [&str; 24] = ["the", "and", "for", "with", "from", "that", "this", "into", "our", "out", "are", "was", "can", "not", "but", "all", "new", "add", "use", "get", "has", "its", "too", "per"];

impl Studio {
    /// F: the box picked out and what it is joined to lit, the rest of the
    /// board faded; with two picked out, only the paths between them. F
    /// again or Esc brings it all back.
    pub(in crate::studio) fn focus(&mut self) {
        if self.board.focus.take().is_some() {
            self.dirty = true;
            return;
        }
        let boxes: Vec<u32> = self.chosen().into_iter().filter(|&index| self.is_box(index)).map(|index| self.id_of(index)).collect();
        let links = self.links();
        let lit: Vec<u32> = match boxes[..] {
            [one] => {
                let mut lit = vec![one];
                for &(arrow, from, to) in &links {
                    if from == one || to == one {
                        lit.extend([arrow, from, to]);
                    }
                }
                lit
            }
            [a, b] => {
                // What can be reached from each, along the arrows.
                let reach = |start: u32, forward: bool| {
                    let mut seen = vec![start];
                    let mut at = 0;
                    while at < seen.len() {
                        let here = seen[at];
                        for &(_, from, to) in &links {
                            let (this, next) = if forward { (from, to) } else { (to, from) };
                            if this == here && !seen.contains(&next) {
                                seen.push(next);
                            }
                        }
                        at += 1;
                    }
                    seen
                };
                let mut on: Vec<u32> = Vec::new();
                for (start, end) in [(a, b), (b, a)] {
                    let (ahead, behind) = (reach(start, true), reach(end, false));
                    on.extend(ahead.iter().filter(|id| behind.contains(id)));
                }
                if !on.contains(&a) || !on.contains(&b) {
                    return self.say("Nothing joins them");
                }
                let arrows: Vec<u32> = links.iter().filter(|(_, from, to)| on.contains(from) && on.contains(to)).map(|&(arrow, ..)| arrow).collect();
                on.extend(arrows);
                on
            }
            _ => return self.say("Pick a box, or two to see the paths between them"),
        };
        // What is in what is lit is lit with it.
        let mut all = lit.clone();
        for &id in &lit {
            all.extend(self.inside(id));
        }
        self.board.focus = Some(all);
        self.dirty = true;
    }

    /// Some of the board's layers drawn again over what is in the window,
    /// at the size it is shown, as the board draws them.
    fn draw_again(&mut self, canvas: &mut Canvas, view: Rect, layers: Vec<Layer>) {
        if layers.is_empty() {
            return;
        }
        let mut sheet = Doc::for_page();
        sheet.layers = layers;
        let part = Rect::new(view.x - self.pan.0.round() as i32, view.y - self.pan.1.round() as i32, view.w, view.h);
        let pixels = sheet.render_part(self.zoom, part, &mut self.text);
        for y in 0..view.h {
            for x in 0..view.w {
                let pixel = pixels[(y * view.w + x) as usize];
                let alpha = pixel >> 24;
                if alpha > 0 {
                    canvas.blend(view.x + x, view.y + y, pixel & 0xFF_FFFF, alpha.min(255) as u8);
                }
            }
        }
    }

    /// The board faded but for what is lit, which is drawn again over it.
    pub(in crate::studio) fn draw_focus(&mut self, canvas: &mut Canvas, view: Rect) {
        let Some(lit) = self.board.focus.clone() else { return };
        canvas.round_fill(view.x, view.y, view.w, view.h, 0, shapes::PAPER, 200);
        let layers: Vec<Layer> = self.doc.layers.iter().filter(|layer| lit.contains(&stacks::layer_id(layer)) && !layer.hidden).cloned().collect();
        self.draw_again(canvas, view, layers);
    }

    /// Present: the board's frames, or else its numbered parts, one at a
    /// time, each zoomed to and the rest dimmed; Right or Space the next,
    /// Left the one before, Esc out.
    pub(in crate::studio) fn present(&mut self) {
        let mut steps: Vec<(String, (f32, f32, f32, f32))> = (0..self.frame_count()).map(|index| {
            let frame = self.frame(index);
            (frame.name.clone(), (frame.x, frame.y, frame.width as f32, frame.height as f32))
        }).collect();
        if steps.is_empty() {
            // The counters, by number, each with the box it is in or on.
            let mut counters: Vec<(u32, (f32, f32, f32, f32))> = self.doc.layers.iter().filter_map(|layer| match &layer.kind {
                Kind::Shape(shape) if shape.tool == Tool::Counter => Some((shape.number, shape.frame())),
                _ => None,
            }).collect();
            counters.sort_by_key(|(number, _)| *number);
            for (number, (x, y, _, _)) in counters {
                let around = self.boxes_at((x, y)).unwrap_or((x - 150.0, y - 100.0, 300.0, 200.0));
                steps.push((format!("Part {number}"), around));
            }
        }
        if steps.is_empty() {
            // Boxes numbered by their words: 1, 2. and Step 3.
            let mut numbered: Vec<(i64, String, (f32, f32, f32, f32))> = (0..self.doc.layers.len()).filter(|&index| self.is_box(index)).filter_map(|index| {
                let Kind::Shape(shape) = &self.doc.layers[index].kind else { return None };
                let first = shape.text.split_whitespace().take(2).collect::<Vec<_>>();
                let number = first.iter().find_map(|word| word.trim_end_matches(['.', ')', ':', '\u{b7}']).parse::<i64>().ok())?;
                Some((number, shape.text.lines().next().unwrap_or("").to_owned(), shape.frame()))
            }).collect();
            numbered.sort_by_key(|(number, ..)| *number);
            steps = numbered.into_iter().map(|(_, says, frame)| (says, frame)).collect();
        }
        if steps.is_empty() {
            return self.say("Number some parts of the board, or frame them, to present them");
        }
        self.commit_text();
        self.board.ghost = None;
        self.selected.clear();
        self.board.present = Some(Present { steps, at: 0, view: (self.zoom, self.pan) });
        self.present_step(0);
    }

    /// The smallest box a point is in.
    fn boxes_at(&self, (x, y): (f32, f32)) -> Option<(f32, f32, f32, f32)> {
        (0..self.doc.layers.len()).filter(|&index| self.is_box(index)).map(|index| self.box_frame(index)).filter(|f| x >= f.0 - 30.0 && x <= f.0 + f.2 + 30.0 && y >= f.1 - 30.0 && y <= f.1 + f.3 + 30.0).min_by(|a, b| (a.2 * a.3).total_cmp(&(b.2 * b.3)))
    }

    /// A step of what is presented zoomed to, with room round it.
    fn present_step(&mut self, at: usize) {
        let Some(present) = &mut self.board.present else { return };
        let at = at.min(present.steps.len() - 1);
        present.at = at;
        let (_, (x, y, w, h)) = present.steps[at].clone();
        let (wide, high) = (self.width as f32, self.height as f32);
        self.zoom = ((wide - 160.0) / w.max(1.0)).min((high - 200.0) / h.max(1.0)).clamp(MIN_ZOOM, 4.0);
        self.pan = (wide / 2.0 - (x + w / 2.0) * self.zoom, high / 2.0 - (y + h / 2.0) * self.zoom);
        self.shown_page.clear();
        self.dirty = true;
    }

    /// A key while presenting: the next part, the one before, or out.
    pub(in crate::studio) fn present_key(&mut self, key: Key) -> bool {
        let Some(present) = &self.board.present else { return false };
        let at = present.at;
        match key {
            Key::Right | Key::Down | Key::Char(' ') | Key::Enter => self.present_step(at + 1),
            Key::Left | Key::Up | Key::Backspace => self.present_step(at.saturating_sub(1)),
            Key::Escape => {
                if let Some(present) = self.board.present.take() {
                    (self.zoom, self.pan) = present.view;
                }
                self.shown_page.clear();
            }
            _ => {}
        }
        self.dirty = true;
        true
    }

    /// The part presented lit and the rest dimmed, and which of how many
    /// it is, along the bottom.
    pub(in crate::studio) fn draw_present(&mut self, canvas: &mut Canvas, view: Rect) {
        let Some(present) = self.board.present.clone() else { return };
        let (says, (x, y, w, h)) = present.steps[present.at].clone();
        let (a, b) = (self.to_window_page((x - 16.0, y - 16.0)), self.to_window_page((x + w + 16.0, y + h + 16.0)));
        let lit = Rect::new(a.0, a.1, b.0 - a.0, b.1 - a.1);
        for band in [Rect::new(view.x, view.y, view.w, (lit.y - view.y).max(0)), Rect::new(view.x, lit.bottom(), view.w, (view.bottom() - lit.bottom()).max(0)), Rect::new(view.x, lit.y, (lit.x - view.x).max(0), lit.h), Rect::new(lit.right(), lit.y, (view.right() - lit.right()).max(0), lit.h)] {
            let band = band.intersect(view);
            if !band.is_empty() {
                canvas.round_fill(band.x, band.y, band.w, band.h, 0, 0x1E1E1E, 150);
            }
        }
        let note = format!("{} of {}  \u{b7}  {says}", present.at + 1, present.steps.len());
        let wide = self.text.width(&note, 13) + 28;
        let place = Rect::new(view.center().0 - wide / 2, view.bottom() - 52, wide, 30);
        canvas.round_fill(place.x, place.y, place.w, place.h, 15, PANEL_COLOR, 240);
        self.text.draw(canvas, place.x + 14, place.y + 15 - Text::line_height(13) / 2, 13, TEXT_COLOR, &note);
    }

    /// Whether a key is L for the laser: with nothing being typed, and no
    /// box picked out to type over; or L let go while the laser is held.
    pub(in crate::studio) fn laser_wants(&self, key: Key, mods: Mods) -> bool {
        let l = matches!(key, Key::Char('l'));
        if !l || mods.ctrl || mods.alt || mods.shift {
            return false;
        }
        let typing = self.editing.is_some() || self.renaming.is_some() || self.typing.is_some() || self.search.is_some() || self.menu.is_some();
        let typed_over = self.tool == Tool::Select && self.picked().is_some_and(|index| self.has_words(index));
        self.board.laser.is_some() || (!typing && !typed_over)
    }

    /// L pressed on a board: the laser, while it is held; a key held
    /// repeating lets go and presses again at once, which is not letting go.
    pub(in crate::studio) fn laser_key(&mut self, down: bool) {
        let now = Instant::now();
        match (&mut self.board.laser, down) {
            (Some(laser), true) => laser.let_go = None,
            (None, true) => self.board.laser = Some(Laser { since: now, let_go: None, drawn: false }),
            (Some(laser), false) => laser.let_go = Some(now),
            (None, false) => {}
        }
    }

    /// The laser let go for good, once long enough after: a tap of L, with
    /// nothing drawn, takes up the line tool as the key always did.
    pub(in crate::studio) fn board_tick(&mut self) {
        let now = Instant::now();
        if let Some(laser) = &self.board.laser {
            if laser.let_go.is_some_and(|when| now.duration_since(when) >= REPEAT) {
                let tapped = !laser.drawn && laser.let_go.is_some_and(|when| when.duration_since(laser.since) < Duration::from_millis(350));
                self.board.laser = None;
                if tapped {
                    self.take_board_tool(LINE_TOOL);
                }
            }
        }
        let trail = self.board.trail.len();
        self.board.trail.retain(|(_, when)| now.duration_since(*when).as_secs_f32() < TRAIL);
        let glowing = self.board.glow.is_some_and(|(_, when)| now.duration_since(when).as_secs_f32() < GLOW);
        if !glowing {
            self.board.glow = None;
        }
        if trail > 0 || glowing {
            self.dirty = true;
        }
    }

    /// How soon the board wants drawing again by itself, while the laser's
    /// trail fades or L is waited on.
    pub fn board_wait(&self) -> Option<Duration> {
        (self.board.laser.is_some() || !self.board.trail.is_empty() || self.board.glow.is_some()).then_some(Duration::from_millis(16))
    }

    /// The pointer moved with the laser held: its trail.
    pub(in crate::studio) fn laser_moved(&mut self, at: (i32, i32)) -> bool {
        let Some(laser) = &mut self.board.laser else { return false };
        laser.drawn = true;
        self.board.trail.push((at, Instant::now()));
        self.dirty = true;
        true
    }

    /// A press with the laser held: the box under it glows a moment.
    pub(in crate::studio) fn laser_press(&mut self, at: (f32, f32)) -> bool {
        if self.board.laser.is_none() {
            return false;
        }
        if let Some(laser) = &mut self.board.laser {
            laser.drawn = true;
        }
        self.board.glow = (0..self.doc.layers.len()).rev().find(|&index| self.is_box(index) && {
            let (x, y, w, h) = self.box_frame(index);
            at.0 >= x && at.0 <= x + w && at.1 >= y && at.1 <= y + h
        }).map(|index| (self.id_of(index), Instant::now()));
        self.dirty = true;
        true
    }

    /// The laser's trail, fading from its tip, and a box it clicked aglow.
    pub(in crate::studio) fn draw_laser(&mut self, canvas: &mut Canvas) {
        let now = Instant::now();
        if let Some((id, when)) = self.board.glow {
            let fade = 1.0 - now.duration_since(when).as_secs_f32() / GLOW;
            if let (Some(index), true) = (self.index_of(id), fade > 0.0) {
                let (x, y, w, h) = self.box_frame(index);
                let (a, b) = (self.to_window_page((x, y)), self.to_window_page((x + w, y + h)));
                for grow in 0..6 {
                    canvas.round_outline(a.0 - 3 - grow, a.1 - 3 - grow, b.0 - a.0 + 6 + 2 * grow, b.1 - a.1 + 6 + 2 * grow, 8 + grow, 1, canvas::mix(shapes::PAPER, LASER, (fade * (220 - grow * 30) as f32) as u32));
                }
            }
        }
        for pair in self.board.trail.windows(2) {
            let ((a, _), (b, when)) = (pair[0], pair[1]);
            let fade = 1.0 - now.duration_since(when).as_secs_f32() / TRAIL;
            if fade <= 0.0 {
                continue;
            }
            let wide = (5.0 * fade).max(1.0);
            let mut stroke = Mask { bounds: Rect::spanning(a, b).inflate(wide as i32 + 6), alpha: Vec::new() };
            stroke.alpha = vec![0; (stroke.bounds.w * stroke.bounds.h) as usize];
            shapes::stamp_line(&mut stroke, (a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32), wide + 3.0);
            canvas.mask(&stroke, LASER, (60.0 * fade) as u32);
            let mut core = Mask { bounds: stroke.bounds, alpha: vec![0; stroke.alpha.len()] };
            shapes::stamp_line(&mut core, (a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32), wide * 0.5);
            canvas.mask(&core, LASER, (255.0 * fade) as u32);
        }
    }

    /// G: the boxes picked out gathered into a box round them, titled with
    /// the word their words share most, and in it.
    pub(in crate::studio) fn group(&mut self) {
        let picked: Vec<usize> = self.chosen().into_iter().filter(|&index| self.is_box(index)).collect();
        if picked.len() < 2 {
            return self.say("Pick some notes to gather them");
        }
        let frames: Vec<(f32, f32, f32, f32)> = picked.iter().map(|&index| self.box_frame(index)).collect();
        let Some((x, y, w, h)) = actions::union(&frames) else { return };
        // The word most of them share, as their title.
        let mut counts: Vec<(String, usize)> = Vec::new();
        for &index in &picked {
            let mut seen: Vec<String> = Vec::new();
            for word in self.words_of_box(index).split(|c: char| !c.is_alphanumeric()).filter(|word| word.len() > 2) {
                let word = word.to_lowercase();
                if COMMON.contains(&word.as_str()) || seen.contains(&word) {
                    continue;
                }
                seen.push(word.clone());
                match counts.iter_mut().find(|(known, _)| *known == word) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((word, 1)),
                }
            }
        }
        let title = counts.iter().filter(|(_, count)| *count >= 2).max_by_key(|(_, count)| *count).map(|(word, _)| {
            let mut letters = word.chars();
            letters.next().map_or(String::new(), |first| first.to_uppercase().chain(letters).collect())
        }).unwrap_or_else(|| "Group".to_owned());
        self.checkpoint();
        let mut shape = Shape::new(Tool::Rect, QUIET, LINE, (x - 24.0, y - 60.0));
        self.board_dress(&mut shape);
        (shape.form, shape.color, shape.filled, shape.shadow) = (shapes::FORM_BOX, QUIET, false, None);
        shape.points[1] = (x + w + 24.0, y + h + 24.0);
        shape.base = (w + 48.0, h + 84.0);
        (shape.text, shape.titled, shape.heading) = (title.clone(), true, true);
        // Under what it gathers.
        self.doc.layers.insert(0, Layer::shape(shape));
        let holder = self.id_of(0);
        let ids: Vec<u32> = picked.iter().map(|&index| self.id_of(index + 1)).collect();
        for id in ids {
            self.unnest(id);
            self.nest(id, holder);
        }
        self.selected = vec![0];
        self.changed();
        self.say(format!("Gathered as {title}"));
    }

    fn words_of_box(&self, index: usize) -> String {
        match &self.doc.layers[index].kind {
            Kind::Shape(shape) => shape.text.clone(),
            Kind::Picture(_) => String::new(),
        }
    }

    /// D on a note: a vote more; Shift+D, one fewer.
    pub(in crate::studio) fn vote(&mut self, more: bool) {
        self.restyle_shapes(|shape| {
            if shape.form == shapes::FORM_STICKY {
                shape.votes = if more { (shape.votes + 1).min(12) } else { shape.votes.saturating_sub(1) };
            }
        });
    }

    /// A box laid out ready, in the middle of what is seen: a 2x2, Now /
    /// Next / Later lanes, or Before and After.
    pub(in crate::studio) fn template(&mut self, which: u8) {
        self.commit_text();
        self.checkpoint();
        let open = self.open_area();
        let middle = self.to_page(open.center());
        let start = self.doc.layers.len();
        let made = |studio: &mut Studio, (x, y, w, h): (f32, f32, f32, f32), words: &str, fill: Option<u32>| {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
            studio.board_dress(&mut shape);
            shape.form = shapes::FORM_BOX;
            shape.points[1] = (x + w, y + h);
            shape.base = (w, h);
            (shape.filled, shape.fill) = (fill.is_some(), fill.unwrap_or(0xFFFFFF));
            if !words.is_empty() {
                (shape.text, shape.titled, shape.heading) = (words.to_owned(), true, true);
            }
            studio.doc.layers.push(Layer::shape(shape));
            studio.doc.layers.len() - 1
        };
        match which {
            // A 2x2, its axes along its bottom and up its side.
            0 => {
                let (w, h) = (240.0, 200.0);
                let (x0, y0) = (middle.0 - w, middle.1 - h);
                for (dx, dy) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
                    made(self, (x0 + dx, y0 + dy, w, h), "", None);
                }
                for (words, at) in [("Effort \u{2192}", (middle.0 - 40.0, y0 + 2.0 * h + 12.0)), ("\u{2191} Value", (x0 - 110.0, middle.1 - 12.0))] {
                    let mut label = Shape::new(Tool::Text, INK, LINE, at);
                    self.board_dress(&mut label);
                    label.text = words.into();
                    self.doc.layers.push(Layer::shape(label));
                }
            }
            // Three lanes, Now, Next and Later, each its title at its top.
            1 => {
                let (w, h, gap) = (260.0, 460.0, 24.0);
                let x0 = middle.0 - (3.0 * w + 2.0 * gap) / 2.0;
                for (at, words) in ["Now", "Next", "Later"].iter().enumerate() {
                    let index = made(self, (x0 + at as f32 * (w + gap), middle.1 - h / 2.0, w, h), words, None);
                    if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                        shape.region = 0;
                    }
                }
            }
            // Before and After, side by side, an arrow from one to the other.
            _ => {
                let (w, h, gap) = (420.0, 320.0, 120.0);
                let x0 = middle.0 - (2.0 * w + gap) / 2.0;
                let before = made(self, (x0, middle.1 - h / 2.0, w, h), "Before", Some(FILLS[3]));
                let after = made(self, (x0 + w + gap, middle.1 - h / 2.0, w, h), "After", Some(FILLS[1]));
                self.join(before, after);
            }
        }
        self.selected = (start..self.doc.layers.len()).filter(|&index| self.is_box(index)).collect();
        self.changed();
    }

    /// The box picked out made to lay out what is put in it: where it is
    /// put, in a row left to right, or as a tree top down.
    pub(in crate::studio) fn set_region(&mut self, region: u8) {
        if !self.chosen().iter().any(|&index| self.is_box(index)) {
            return self.say("Pick a box to lay out what is in it");
        }
        self.restyle_shapes(|shape| {
            if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                shape.region = region;
                if region > 0 {
                    (shape.titled, shape.heading) = (true, true);
                }
            }
        });
        self.say(["Laid out as it is put", "Laid out left to right", "Laid out as a tree, top down"][usize::from(region.min(2))]);
    }

    /// What is in a box that lays it out, laid out: in a row, or as trees
    /// top down, in from its top left under its title.
    pub(in crate::studio) fn arrange_region(&mut self, holder: u32, dragged: &[u32]) {
        let Some(at) = self.index_of(holder) else { return };
        let Kind::Shape(shape) = &self.doc.layers[at].kind else { return };
        let region = shape.region;
        if region == 0 {
            return;
        }
        if region == mermaid::SEQUENCE {
            return self.arrange_sequence(holder, dragged);
        }
        if region > 2 {
            return;
        }
        let (x, y, _, _) = shape.frame();
        let top = y + Text::line_height(shape.font_px) as f32 + 10.0 + 20.0;
        let members: Vec<u32> = self.doc.stacks.iter().find(|stack| stack.holder == Some(holder)).map(|stack| stack.members.clone()).unwrap_or_default();
        let mut placed: Vec<(u32, (f32, f32, f32, f32))> = members.iter().filter(|member| !dragged.contains(member)).filter_map(|&member| self.index_of(member).filter(|&index| self.is_box(index)).map(|index| (member, self.box_frame(index)))).collect();
        placed.sort_by(|a, b| a.1 .0.total_cmp(&b.1 .0));
        let roots: Vec<(u32, (f32, f32, f32, f32))> = match region {
            2 => placed.iter().copied().filter(|&(id, _)| self.parent_box(id).is_none_or(|parent| !members.contains(&parent))).collect(),
            _ => placed,
        };
        let mut along = x + 20.0;
        for (id, (fx, fy, fw, _)) in roots {
            let span = if region == 2 { self.span_of(id).max(fw) } else { fw };
            let to = (along + (span - fw) / 2.0, top);
            let mut all = self.inside(id);
            all.push(id);
            for member in all {
                if let Some(index) = self.index_of(member) {
                    self.doc.layers[index].translate(to.0 - fx, to.1 - fy);
                }
            }
            if region == 2 {
                self.lay_out_tree(id, true, false);
            }
            along += span + 40.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn put(studio: &mut Studio, (x, y, w, h): (f32, f32, f32, f32), words: &str, form: u8) -> usize {
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
        studio.board.making = form;
        studio.board_dress(&mut shape);
        studio.board.making = shapes::FORM_BOX;
        shape.points[1] = (x + w, y + h);
        (shape.text, shape.base) = (words.into(), (w, h));
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        studio.doc.layers.len() - 1
    }

    #[test]
    fn f_lights_what_a_box_is_joined_to_and_two_the_paths_between_them() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "Client", 0);
        let b = put(&mut studio, (300.0, 100.0, 120.0, 60.0), "Server", 0);
        let c = put(&mut studio, (500.0, 100.0, 120.0, 60.0), "DB", 0);
        let d = put(&mut studio, (300.0, 400.0, 120.0, 60.0), "Admin", 0);
        studio.join(a, b);
        studio.join(b, c);
        studio.changed();
        let ids: Vec<u32> = [a, b, c, d].iter().map(|&index| studio.id_of(index)).collect();
        studio.selected = vec![b];
        press_key(&mut studio, Key::Char('f'), Mods::default());
        let lit = studio.board.focus.clone().unwrap();
        assert!(lit.contains(&ids[0]) && lit.contains(&ids[1]) && lit.contains(&ids[2]) && !lit.contains(&ids[3]));
        // The rest faded toward the paper; what is lit as it was.
        studio.selected.clear();
        let faded = shown(&mut studio, (360.0, 430.0));
        assert!(faded != FILLS[2] && (faded & 0xFF) > 0xE0, "{faded:06X}");
        assert_eq!(shown(&mut studio, (115.0, 150.0)), FILLS[2]);
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.focus.is_none());
        // Two picked out: only the paths between them.
        studio.selected = vec![a, c];
        studio.focus();
        let lit = studio.board.focus.clone().unwrap();
        assert!(lit.contains(&ids[1]) && !lit.contains(&ids[3]));
        studio.board.focus = None;
        studio.selected = vec![a, d];
        studio.focus();
        assert!(studio.board.focus.is_none(), "nothing joins them");
    }

    #[test]
    fn present_walks_the_numbered_parts_zoomed_and_dimmed_and_esc_comes_back() {
        let mut studio = board();
        put(&mut studio, (900.0, 100.0, 200.0, 100.0), "3 \u{b7} Pricing", 0);
        put(&mut studio, (100.0, 100.0, 200.0, 100.0), "1 \u{b7} Today", 0);
        put(&mut studio, (500.0, 100.0, 200.0, 100.0), "2 \u{b7} With Acme", 0);
        let view = (studio.zoom, studio.pan);
        let present = studio.control_rect(Control::Board(Ctl::Present)).unwrap().center();
        click(&mut studio, present);
        let steps: Vec<String> = studio.board.present.as_ref().unwrap().steps.iter().map(|(says, _)| says.clone()).collect();
        assert_eq!(steps, ["1 \u{b7} Today", "2 \u{b7} With Acme", "3 \u{b7} Pricing"]);
        assert!(studio.layout().controls.is_empty() && !studio.shows_left(), "nothing but the board");
        // Zoomed to the first, the rest dimmed.
        let middle = studio.to_window_page((200.0, 150.0));
        assert!((middle.0 - studio.width / 2).abs() <= 2 && studio.zoom > 2.0);
        press_key(&mut studio, Key::Char(' '), Mods::default());
        assert_eq!(studio.board.present.as_ref().unwrap().at, 1);
        press_key(&mut studio, Key::Left, Mods::default());
        assert_eq!(studio.board.present.as_ref().unwrap().at, 0);
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.present.is_none() && (studio.zoom, studio.pan) == view);
    }

    #[test]
    fn l_held_is_a_laser_that_fades_and_a_tap_of_it_the_line_tool() {
        let mut studio = board();
        studio.key(Key::Char('l'), Mods::default(), true);
        assert!(studio.board.laser.is_some());
        studio.pointer_moved(300, 300, Mods::default());
        studio.pointer_moved(340, 320, Mods::default());
        // A key held repeats by letting go and pressing again at once.
        studio.key(Key::Char('l'), Mods::default(), false);
        studio.key(Key::Char('l'), Mods::default(), true);
        studio.board_tick();
        assert!(studio.board.laser.is_some() && studio.board.trail.len() == 2 && studio.board_wait().is_some());
        studio.key(Key::Char('l'), Mods::default(), false);
        if let Some(laser) = &mut studio.board.laser {
            laser.let_go = Some(Instant::now() - Duration::from_millis(200));
        }
        studio.board_tick();
        assert!(studio.board.laser.is_none() && studio.board_tool() == SELECT, "drawn with, the tool is as it was");
        assert!(studio.doc.layers.is_empty(), "nothing added to the board");
        // A tap: the line tool.
        studio.key(Key::Char('l'), Mods::default(), true);
        studio.key(Key::Char('l'), Mods::default(), false);
        if let Some(laser) = &mut studio.board.laser {
            laser.let_go = Some(laser.since + Duration::from_millis(80));
            laser.since -= Duration::from_millis(300);
            laser.let_go = Some(laser.since + Duration::from_millis(100));
        }
        studio.board_tick();
        assert_eq!(studio.board_tool(), LINE_TOOL);
    }

    #[test]
    fn notes_are_gathered_titled_voted_on_and_lanes_come_ready() {
        let mut studio = board();
        let one = put(&mut studio, (100.0, 100.0, 120.0, 100.0), "Dark mode", shapes::FORM_STICKY);
        let two = put(&mut studio, (260.0, 120.0, 120.0, 100.0), "Dark icons", shapes::FORM_STICKY);
        studio.selected = vec![one, two];
        press_key(&mut studio, Key::Char('g'), Mods::default());
        let group = shape(&studio, 0);
        assert_eq!((group.text.as_str(), group.titled), ("Dark", true));
        assert_eq!(studio.holder_of(shape(&studio, 1).id), Some(group.id));
        // Votes on a note: D, and Shift+D takes one away.
        studio.selected = vec![1];
        press_key(&mut studio, Key::Char('d'), Mods::default());
        press_key(&mut studio, Key::Char('d'), Mods::default());
        press_key(&mut studio, Key::Char('D'), Mods { shift: true, ..Mods::default() });
        assert_eq!(shape(&studio, 1).votes, 1);
        // Now / Next / Later.
        let mut lanes = board();
        lanes.template(1);
        let titles: Vec<String> = (0..3).map(|index| shape(&lanes, index).text).collect();
        assert_eq!(titles, ["Now", "Next", "Later"]);
        assert!(shape(&lanes, 0).titled);
        // A 2x2 and its axes; Before and After, joined.
        let mut square = board();
        square.template(0);
        assert_eq!(square.doc.layers.len(), 6);
        let mut split = board();
        split.template(2);
        assert_eq!(split.links().len(), 1);
    }

    #[test]
    fn a_box_laid_out_left_to_right_puts_what_is_dropped_in_it_in_a_row() {
        let mut studio = board();
        let region = put(&mut studio, (100.0, 100.0, 600.0, 200.0), "Pipeline", 0);
        studio.selected = vec![region];
        studio.set_region(1);
        let read = put(&mut studio, (900.0, 100.0, 100.0, 50.0), "Read", 0);
        let parse = put(&mut studio, (900.0, 300.0, 100.0, 50.0), "Parse", 0);
        studio.selected.clear();
        pull(&mut studio, (950.0, 125.0), (500.0, 200.0), Mods::default());
        pull(&mut studio, (950.0, 325.0), (130.0, 220.0), Mods::default());
        let (fr, fp) = (shape(&studio, read).frame(), shape(&studio, parse).frame());
        assert!((fr.1 - fp.1).abs() < 0.5, "in a row: {fr:?} {fp:?}");
        assert!(fp.0 < fr.0 && (fr.0 - (fp.0 + fp.2) - 40.0).abs() < 0.5, "in order by where each was let go, at an even gap: {fp:?} {fr:?}");
        assert!((fp.0 - 120.0).abs() < 0.5, "from its left, inside it");
    }

    #[test]
    fn words_on_a_box_make_it_a_rough_piece_of_a_screen_and_a_status_dot_shows() {
        let mut studio = board();
        let card = put(&mut studio, (100.0, 100.0, 200.0, 200.0), "card", 0);
        studio.selected.clear();
        // A card: a picture crossed out, in gray, where its words would be.
        let crossed = shown(&mut studio, (200.0, 166.0));
        assert_ne!(crossed, FILLS[2]);
        studio.selected = vec![card];
        studio.board_act(Act::Status(1));
        studio.selected.clear();
        let (x, y, w, _) = shape(&studio, card).frame();
        let dot = (x + w - shape(&studio, card).font_px as f32 * 0.32 * 2.2, y + shape(&studio, card).font_px as f32 * 0.32 * 2.2);
        assert_eq!(shown(&mut studio, dot), 0xE03131, "a red dot at its top right");
    }

    #[test]
    fn alt_dragging_a_box_with_boxes_in_it_copies_all_of_it_as_option_b() {
        let mut studio = board();
        let outer = put(&mut studio, (100.0, 100.0, 300.0, 200.0), "Option A", 0);
        let a = put(&mut studio, (130.0, 160.0, 100.0, 50.0), "Login", 0);
        let b = put(&mut studio, (270.0, 160.0, 100.0, 50.0), "Home", 0);
        let (id, ida, idb) = (studio.id_of(outer), studio.id_of(a), studio.id_of(b));
        studio.nest(ida, id);
        studio.nest(idb, id);
        studio.join(a, b);
        studio.changed();
        let before = studio.doc.layers.len();
        let from = studio.to_window_page((120.0, 290.0));
        studio.pointer_moved(from.0, from.1, Mods { alt: true, ..Mods::default() });
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let to = studio.to_window_page((620.0, 290.0));
        studio.pointer_moved(from.0 + 200, from.1, Mods { alt: true, ..Mods::default() });
        studio.pointer_moved(to.0, to.1, Mods { alt: true, ..Mods::default() });
        studio.released();
        assert_eq!(studio.doc.layers.len(), before + 4, "the box, both in it, and the arrow between them");
        let copy = studio.doc.layers.iter().rposition(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "Option A")).unwrap();
        assert_ne!(copy, outer);
        let inside = studio.inside(studio.id_of(copy));
        assert_eq!(inside.len(), 2, "what is in it is in the copy");
        assert_eq!(studio.links().len(), 2, "the copy's arrow joins the copies");
        assert!(shape(&studio, outer).frame().0 < 101.0, "Option A stays where it was");
    }
}
