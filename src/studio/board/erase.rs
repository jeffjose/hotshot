//! The eraser and the lasso. The eraser, E: a drag across the board
//! leaves a faint red trail, and what the trail goes over fades, gone once
//! the button is let go, all of it one step to undo; Esc while it is held
//! keeps everything; a click takes what is under it. The lasso: Alt and a
//! drag on empty board with the pointer in hand draws a loop round what is
//! to be picked out, and letting go picks out all that is wholly in it.

use super::*;

/// How much of how opaque it was what the eraser has gone over is shown,
/// out of 256: a quarter.
const FADED: u32 = 64;
/// The eraser's trail: how far out it reaches either side at its tip, in
/// pixels of the window, how many of the points it went by it is drawn
/// through, and its tint, out of 256.
const TRAIL_WIDE: f32 = 7.0;
const TRAIL_POINTS: usize = 28;
const TRAIL_TINT: u32 = 0xE03131;
const TRAIL_AMOUNT: u32 = 56;

/// What the eraser has gone over, while its button is held.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Erasing {
    /// The points it went through, on the board.
    pub trail: Vec<(f32, f32)>,
    /// What it has gone over, by what each is called, each with how
    /// opaque it was.
    pub over: Vec<(u32, u32)>,
}

/// How opaque a layer is, out of 256.
fn opacity_of(layer: &Layer) -> u32 {
    match &layer.kind {
        Kind::Shape(shape) => shape.opacity,
        Kind::Picture(picture) => picture.opacity,
    }
}

fn set_opacity(layer: &mut Layer, opacity: u32) {
    match &mut layer.kind {
        Kind::Shape(shape) => shape.opacity = opacity,
        Kind::Picture(picture) => picture.opacity = opacity,
    }
}

/// Whether a point is inside a loop of points, closed from its last back
/// to its first: by how many of its sides a line to its right crosses.
pub(in crate::studio) fn in_loop(p: (f32, f32), points: &[(f32, f32)]) -> bool {
    let mut odd = false;
    for at in 0..points.len() {
        let (a, b) = (points[at], points[(at + 1) % points.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

/// The eraser, as the bar of tools shows it: a block of rubber on a slant,
/// its tip filled, over the line it rubs along; in the line the icon font's
/// icons are drawn in.
pub(in crate::studio) fn draw_eraser_icon(canvas: &mut Canvas, (cx, cy): (f32, f32), color: u32) {
    let (along, across) = ((0.7071, -0.7071), (0.7071, 0.7071));
    let middle = (cx + 1.0, cy - 1.5);
    let at = |a: f32, b: f32| (middle.0 + along.0 * a + across.0 * b, middle.1 + along.1 * a + across.1 * b);
    let (long, half, tip) = (6.5, 3.4, -1.6);
    let body = [at(-long, -half), at(long, -half), at(long, half), at(-long, half), at(-long, -half)];
    let tip_part = [at(-long, -half), at(tip, -half), at(tip, half), at(-long, half)];
    let (left, top) = tip_part.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
    let (right, bottom) = tip_part.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
    let bounds = Rect::new(left.floor() as i32 - 1, top.floor() as i32 - 1, (right - left).ceil() as i32 + 3, (bottom - top).ceil() as i32 + 3);
    canvas.mask(&shapes::fill_path(bounds, &tip_part), color, 256);
    halo(canvas, &round_off(&body, 1.2), 0.8, color, 256);
    halo(canvas, &[at(tip, -half), at(tip, half)], 0.8, color, 256);
    // The line it rubs along, from under its tip out to the right.
    let foot = at(-long, half).1 + 1.6;
    halo(canvas, &[(cx - 2.5, foot), (cx + 7.0, foot)], 0.8, color, 256);
}

/// The highlighter, as the bar of tools shows it: a felt tip on a slant,
/// its chisel filled, over the broad stroke it lays, half as strong.
pub(in crate::studio) fn draw_highlighter_icon(canvas: &mut Canvas, (cx, cy): (f32, f32), color: u32) {
    let (along, across) = ((0.7071, -0.7071), (0.7071, 0.7071));
    let middle = (cx + 1.0, cy - 2.0);
    let at = |a: f32, b: f32| (middle.0 + along.0 * a + across.0 * b, middle.1 + along.1 * a + across.1 * b);
    let (back, neck, half) = (8.5, -1.5, 3.0);
    let body = [at(neck, -half), at(back, -half), at(back, half), at(neck, half), at(neck, -half)];
    halo(canvas, &round_off(&body, 1.2), 0.8, color, 256);
    // The band where its cap goes on.
    halo(canvas, &[at(4.5, -half), at(4.5, half)], 0.8, color, 256);
    let tip = [at(neck, -half + 0.4), at(neck, half - 0.4), at(neck - 4.0, half * 0.55), at(neck - 4.0, -half * 0.25)];
    let (left, top) = tip.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
    let (right, bottom) = tip.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
    let bounds = Rect::new(left.floor() as i32 - 1, top.floor() as i32 - 1, (right - left).ceil() as i32 + 3, (bottom - top).ceil() as i32 + 3);
    canvas.mask(&shapes::fill_path(bounds, &tip), color, 256);
    // The stroke it lays, broad and half as strong.
    let foot = cy + 7.0;
    halo(canvas, &[(cx - 8.0, foot), (cx + 3.0, foot)], 1.6, color, 140);
}

/// A closed run of corners with each rounded off by so much, as a run of
/// points for a line to go through.
fn round_off(corners: &[(f32, f32)], radius: f32) -> Vec<(f32, f32)> {
    let corners = &corners[..corners.len() - 1];
    let mut path = shapes::round_corners(corners, radius);
    if let Some(&first) = path.first() {
        path.push(first);
    }
    path
}

impl Studio {
    /// A press on a board with the eraser in hand, or with Alt and the
    /// pointer on empty board: the eraser's trail begun, what is under it
    /// fading; or a lasso begun. Whether it was taken.
    pub(in crate::studio) fn erase_or_lasso_press(&mut self, at_window: (i32, i32), off_everything: bool) -> bool {
        if self.presenting() || self.board.history.is_some() || self.space {
            return false;
        }
        let at = self.to_doc(at_window);
        if self.board_tool() == ERASER {
            self.commit_text();
            self.board.erasing = Some(Erasing { trail: vec![at], over: Vec::new() });
            self.erase_at(at);
            self.dirty = true;
            return true;
        }
        if self.tool == Tool::Select && self.mods.alt && off_everything && !self.board.laser_tool && self.layer_at(at).is_none() {
            self.commit_text();
            self.board.lasso = Some(vec![at]);
            self.dirty = true;
            return true;
        }
        false
    }

    /// The pointer moved with the eraser's button held, or a lasso being
    /// drawn: the trail, and what it goes over faded; or the lasso on to
    /// it. Whether it was.
    pub(in crate::studio) fn erase_or_lasso_moved(&mut self, at_window: (i32, i32)) -> bool {
        let at = self.to_doc(at_window);
        let step = 3.0 / self.zoom.max(1e-3);
        if let Some(lasso) = &mut self.board.lasso {
            if lasso.last().is_none_or(|last| (last.0 - at.0).hypot(last.1 - at.1) >= step) {
                lasso.push(at);
            }
            self.dirty = true;
            return true;
        }
        let Some(erasing) = &mut self.board.erasing else {
            // The eraser in hand, its ring goes with the pointer.
            if self.board.eraser_tool {
                self.dirty = true;
            }
            return false;
        };
        // Every few pixels along the way from where it was, so that a quick
        // stroke misses nothing it went over.
        let from = erasing.trail.last().copied().unwrap_or(at);
        let long = (at.0 - from.0).hypot(at.1 - from.1);
        let steps = ((long / step).ceil() as usize).clamp(1, 400);
        erasing.trail.push(at);
        for piece in 1..=steps {
            let t = piece as f32 / steps as f32;
            self.erase_at((from.0 + (at.0 - from.0) * t, from.1 + (at.1 - from.1) * t));
        }
        self.dirty = true;
        true
    }

    /// What is under a point of the eraser's way, the topmost of it, faded
    /// to a quarter of how opaque it was, if it is not already.
    fn erase_at(&mut self, at: (f32, f32)) {
        let Some(index) = self.layer_at(at) else { return };
        let id = self.id_of(index);
        let Some(erasing) = &mut self.board.erasing else { return };
        if erasing.over.iter().any(|&(over, _)| over == id) {
            return;
        }
        let was = opacity_of(&self.doc.layers[index]);
        erasing.over.push((id, was));
        set_opacity(&mut self.doc.layers[index], (was * FADED / 256).max(1));
        self.changed_layers(&[index]);
    }

    /// What the eraser went over put back as it was: each as opaque as it
    /// was. Gives what each is called.
    fn unfade(&mut self) -> Vec<u32> {
        let Some(erasing) = self.board.erasing.take() else { return Vec::new() };
        let mut back = Vec::new();
        for &(id, was) in &erasing.over {
            if let Some(index) = self.index_of(id) {
                set_opacity(&mut self.doc.layers[index], was);
                back.push(index);
            }
        }
        if !back.is_empty() {
            self.changed_layers(&back);
        }
        erasing.over.iter().map(|&(id, _)| id).collect()
    }

    /// The eraser's button or the lasso let go: what the eraser went over
    /// gone, as one step to undo, the arrows tied to it let go of it as
    /// Delete leaves them; what is wholly in the lasso picked out. Whether
    /// either was.
    pub(in crate::studio) fn erase_or_lasso_released(&mut self) -> bool {
        if let Some(lasso) = self.board.lasso.take() {
            self.lasso_pick(&lasso);
            self.dirty = true;
            return true;
        }
        if self.board.erasing.is_none() {
            return false;
        }
        let gone = self.unfade();
        if gone.is_empty() {
            return true;
        }
        self.selected = gone.iter().filter_map(|&id| self.index_of(id)).collect();
        self.delete();
        self.say(format!("Erased {}", gone.len()));
        true
    }

    /// Esc while the eraser's button is held, or while a lasso is drawn:
    /// everything kept as it was. Whether either was.
    pub(in crate::studio) fn erase_or_lasso_cancelled(&mut self) -> bool {
        if self.board.lasso.take().is_some() {
            self.dirty = true;
            return true;
        }
        if self.board.erasing.is_none() {
            return false;
        }
        self.unfade();
        self.dirty = true;
        true
    }

    /// All that is wholly inside a lasso picked out, or added to what is
    /// with Shift: a line or a stroke by where it goes, anything else by
    /// the edge of the box round it.
    fn lasso_pick(&mut self, lasso: &[(f32, f32)]) {
        if lasso.len() < 3 {
            return;
        }
        if !self.mods.shift {
            self.selected.clear();
        }
        for index in 0..self.doc.layers.len() {
            let layer = &self.doc.layers[index];
            if layer.hidden || layer.locked || self.selected.contains(&index) {
                continue;
            }
            let spots: Vec<(f32, f32)> = match &layer.kind {
                Kind::Shape(shape) if matches!(shape.tool, Tool::Line | Tool::Arrow) => shape.line_strokes().into_iter().flatten().collect(),
                Kind::Shape(shape) if matches!(shape.tool, Tool::Pen | Tool::Marker) => shape.points.clone(),
                _ => {
                    // A box by its frame, as it is seen, not the room kept
                    // round it for a tag or a shadow.
                    let (x, y, w, h) = match &layer.kind {
                        Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse) && shape.angle == 0.0 => shape.frame(),
                        _ => self.doc.layers[index].bounds(&mut self.text),
                    };
                    (0..=6).flat_map(|step| {
                        let t = step as f32 / 6.0;
                        [(x + w * t, y), (x + w * t, y + h), (x, y + h * t), (x + w, y + h * t)]
                    }).collect()
                }
            };
            if !spots.is_empty() && spots.iter().all(|&spot| in_loop(spot, lasso)) {
                self.selected.push(index);
            }
        }
    }

    /// The eraser's trail, fading and thinning back from its tip; a ring
    /// where it would go with nothing held; and the lasso as it is drawn,
    /// dashed in the accent round a faint wash of it.
    pub(in crate::studio) fn draw_erase_and_lasso(&mut self, canvas: &mut Canvas) {
        if let Some(erasing) = &self.board.erasing {
            let tail: Vec<(f32, f32)> = erasing.trail[erasing.trail.len().saturating_sub(TRAIL_POINTS)..].iter().map(|&at| self.to_window_f(at)).collect();
            let count = tail.len().max(2) as f32;
            let halves: Vec<f32> = (0..tail.len()).map(|at| TRAIL_WIDE * (0.35 + 0.65 * (at as f32 + 1.0) / count)).collect();
            let ink = shapes::Ink { points: tail, halves, filled: false };
            let reach = TRAIL_WIDE as i32 + 2;
            let (left, top) = ink.points.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
            let (right, bottom) = ink.points.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
            let bounds = Rect::new(left.floor() as i32 - reach, top.floor() as i32 - reach, (right - left).ceil() as i32 + 2 * reach + 1, (bottom - top).ceil() as i32 + 2 * reach + 1).intersect(canvas.clip());
            if !bounds.is_empty() {
                canvas.mask(&shapes::lay_inks(bounds, &[ink]), TRAIL_TINT, TRAIL_AMOUNT);
            }
        } else if self.board_tool() == ERASER && self.drag == Drag::None && self.pointer_cursor.is_none() && self.over_page(self.pointer) {
            let (x, y) = self.pointer;
            let ring = if self.pages[self.page].ground == 0 { 0x868E96 } else { 0xADB5BD };
            canvas.round_fill(x - 7, y - 7, 14, 14, 7, TRAIL_TINT, 28);
            canvas.round_outline(x - 7, y - 7, 14, 14, 7, 1, ring);
        }
        let Some(lasso) = &self.board.lasso else { return };
        if lasso.len() < 2 {
            return;
        }
        let points: Vec<(f32, f32)> = lasso.iter().map(|&at| self.to_window_f(at)).collect();
        let (left, top) = points.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
        let (right, bottom) = points.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
        let bounds = Rect::new(left.floor() as i32 - 1, top.floor() as i32 - 1, (right - left).ceil() as i32 + 3, (bottom - top).ceil() as i32 + 3).intersect(canvas.clip());
        if points.len() > 2 && !bounds.is_empty() {
            let mut shut = points.clone();
            shut.push(points[0]);
            canvas.mask(&shapes::fill_path(bounds, &shut), ACCENT, 26);
        }
        // Dashes along the way it went, and fainter back from its end to
        // its start, which letting go closes it by.
        let mut way = points.clone();
        way.push(points[0]);
        let (on, off) = (6.0, 4.0);
        let mut gone = 0.0f32;
        let mut dash: Vec<(f32, f32)> = Vec::new();
        let last = points.len() - 1;
        for (at, pair) in way.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            let long = (b.0 - a.0).hypot(b.1 - a.1);
            let amount = if at == last { 130 } else { 256 };
            let mut t = 0.0f32;
            while t < long {
                let into = gone.rem_euclid(on + off);
                let left_of = if into < on { on - into } else { on + off - into };
                let run = left_of.min(long - t);
                let (p, q) = ((a.0 + (b.0 - a.0) * t / long.max(1e-6), a.1 + (b.1 - a.1) * t / long.max(1e-6)), (a.0 + (b.0 - a.0) * (t + run) / long.max(1e-6), a.1 + (b.1 - a.1) * (t + run) / long.max(1e-6)));
                if into < on {
                    if dash.is_empty() {
                        dash.push(p);
                    }
                    dash.push(q);
                } else if !dash.is_empty() {
                    halo(canvas, &std::mem::take(&mut dash), 0.75, ACCENT, amount);
                }
                gone += run;
                t += run;
            }
            // A dash goes on round a corner, but not on into the fainter
            // part.
            if !dash.is_empty() && at + 1 >= last {
                halo(canvas, &std::mem::take(&mut dash), 0.75, ACCENT, amount);
                if at + 1 == last {
                    dash.push(b);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    /// A box put on a board as its box tool would draw it.
    fn put(studio: &mut Studio, (x, y, w, h): (f32, f32, f32, f32), words: &str) -> usize {
        let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
        studio.board_dress(&mut shape);
        shape.points[1] = (x + w, y + h);
        (shape.text, shape.base) = (words.into(), (w, h));
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        studio.doc.layers.len() - 1
    }

    /// The button pressed at the first of some points of the board, moved
    /// through the rest, and held there.
    fn hold(studio: &mut Studio, points: &[(f32, f32)], mods: Mods) {
        let first = studio.to_window_page(points[0]);
        studio.pointer_moved(first.0, first.1, mods);
        studio.mods = mods;
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        for &point in &points[1..] {
            let at = studio.to_window_page(point);
            studio.pointer_moved(at.0, at.1, mods);
        }
    }

    fn opacities(studio: &Studio) -> Vec<u32> {
        studio.doc.layers.iter().map(opacity_of).collect()
    }

    #[test]
    fn the_eraser_on_e_fades_what_it_goes_over_and_takes_it_away_once_let_go_as_one_step() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 160.0, 72.0), "Client");
        let b = put(&mut studio, (100.0, 300.0, 160.0, 72.0), "Server");
        let c = put(&mut studio, (500.0, 100.0, 160.0, 72.0), "Cache");
        let arrow = studio.join(a, c);
        let (ids, undo) = ((studio.id_of(a), studio.id_of(b), studio.id_of(c)), studio.undo.len());
        press_key(&mut studio, Key::Char('e'), Mods::default());
        assert_eq!((studio.board_tool(), TOOLS[usize::from(ERASER)]), (ERASER, ("Eraser", 'e')));
        // Dragged down through the two boxes at the left, held: those two
        // a quarter as opaque, the rest as they were, nothing gone yet.
        hold(&mut studio, &[(180.0, 80.0), (180.0, 200.0), (180.0, 340.0), (180.0, 420.0)], Mods::default());
        assert_eq!(opacities(&studio), vec![64, 64, 256, 256]);
        assert_eq!((studio.doc.layers.len(), studio.undo.len()), (4, undo), "nothing gone, no step yet");
        // Its trail drawn in a faint red, over the board.
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let at = studio.to_window_page((180.0, 410.0));
        let tint = out[(at.1 * studio.width + at.0) as usize] & 0xFF_FFFF;
        assert!(tint >> 16 > (tint >> 8 & 0xFF) + 12 && tint >> 16 > 0xD0, "a red tint: {tint:06X}");
        // Let go: both gone, the arrow left with the end that was tied to
        // the first let go of it, and said so.
        studio.released();
        let left: Vec<u32> = studio.doc.layers.iter().map(stacks::layer_id).collect();
        assert!(!left.contains(&ids.0) && !left.contains(&ids.1) && left.contains(&ids.2), "{left:?}");
        let joined = studio.doc.layers.iter().find_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Arrow => Some(shape.stuck.map(|end| end.map(|(id, ..)| id))),
            _ => None,
        });
        assert_eq!(joined, Some([None, Some(ids.2)]));
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Erased 2"));
        assert!(studio.selected.is_empty() && studio.board_tool() == ERASER, "the eraser still in hand");
        // One step back: all of it as it was, as opaque, the arrow tied.
        studio.undo();
        assert_eq!((studio.doc.layers.len(), opacities(&studio)), (4, vec![256; 4]));
        assert!(matches!(&studio.doc.layers[arrow].kind, Kind::Shape(shape) if shape.stuck[0].is_some_and(|(id, ..)| id == ids.0)));
    }

    #[test]
    fn esc_while_erasing_keeps_everything_and_a_click_takes_only_what_is_under_it() {
        let mut studio = board();
        let under = put(&mut studio, (100.0, 100.0, 400.0, 300.0), "Lane");
        put(&mut studio, (200.0, 200.0, 120.0, 60.0), "Inside");
        let undo = studio.undo.len();
        press_key(&mut studio, Key::Char('e'), Mods::default());
        hold(&mut studio, &[(260.0, 150.0), (260.0, 230.0), (300.0, 230.0)], Mods::default());
        assert_eq!(opacities(&studio), vec![64, 64]);
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.released();
        assert_eq!((studio.doc.layers.len(), opacities(&studio), studio.undo.len()), (2, vec![256, 256], undo), "all kept, nothing to undo");
        assert_eq!(studio.board_tool(), ERASER, "Esc while held lets the stroke go, not the eraser");
        // A click on the box inside the other: that box, not the one under.
        tap(&mut studio, (260.0, 230.0));
        assert_eq!(studio.doc.layers.len(), 1);
        assert_eq!(shape(&studio, under).text, "Lane");
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Erased 1"));
        // On empty board, nothing, and no step.
        let undo = studio.undo.len();
        tap(&mut studio, (900.0, 700.0));
        assert_eq!((studio.doc.layers.len(), studio.undo.len()), (1, undo));
        // Esc, not held, puts it down.
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(studio.board_tool(), SELECT);
    }

    #[test]
    fn alt_and_a_drag_on_empty_board_draws_a_lasso_that_picks_out_all_wholly_in_it() {
        let mut studio = board();
        let a = put(&mut studio, (100.0, 100.0, 120.0, 60.0), "One");
        let b = put(&mut studio, (300.0, 140.0, 120.0, 60.0), "Two");
        let partly = put(&mut studio, (460.0, 100.0, 160.0, 60.0), "Half");
        let far = put(&mut studio, (100.0, 500.0, 120.0, 60.0), "Far");
        let arrow = studio.join(a, b);
        studio.selected.clear();
        let pan = studio.pan;
        // A loop round the first two and the arrow between them, and half
        // of the third, begun on empty board with Alt.
        let alt = Mods { alt: true, ..Mods::default() };
        let round: Vec<(f32, f32)> = (0..=32).map(|step| {
            // Begun under them, on empty board.
            let angle = std::f32::consts::TAU * step as f32 / 32.0 + std::f32::consts::FRAC_PI_2;
            (300.0 + 270.0 * angle.cos(), 150.0 + 125.0 * angle.sin())
        }).collect();
        hold(&mut studio, &round, alt);
        assert!(studio.board.lasso.as_ref().is_some_and(|lasso| lasso.len() > 20), "a lasso drawn");
        assert_eq!(studio.pan, pan, "not the hand: Space is the hand");
        // Drawn over a faint wash of the accent.
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let inside = studio.to_window_page((300.0, 235.0));
        assert_ne!(out[(inside.1 * studio.width + inside.0) as usize] & 0xFF_FFFF, shapes::PAPER, "washed");
        studio.released();
        let mut picked = studio.chosen();
        picked.sort_unstable();
        assert_eq!(picked, vec![a, b, arrow]);
        assert!(!picked.contains(&partly) && !picked.contains(&far));
        assert!(studio.board.lasso.is_none());
        // With Shift as well, added to what is picked out.
        let shift_alt = Mods { alt: true, shift: true, ..Mods::default() };
        let round_far: Vec<(f32, f32)> = (0..=24).map(|step| {
            let angle = std::f32::consts::TAU * step as f32 / 24.0;
            (160.0 + 120.0 * angle.cos(), 530.0 + 60.0 * angle.sin())
        }).collect();
        hold(&mut studio, &round_far, shift_alt);
        studio.released();
        assert_eq!(studio.chosen().len(), 4);
        // Esc while drawing one, away from what is picked out: what is
        // picked out stays, and so does all else.
        studio.mods = Mods::default();
        let count = studio.doc.layers.len();
        let elsewhere: Vec<(f32, f32)> = round_far.iter().map(|&(x, y)| (x + 800.0, y)).collect();
        hold(&mut studio, &elsewhere, alt);
        assert!(studio.board.lasso.is_some());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(studio.board.lasso.is_none());
        studio.released();
        assert_eq!((studio.chosen().len(), studio.doc.layers.len()), (4, count));
        // Alt and a drag on a box still copies it, as before.
        studio.selected.clear();
        let count = studio.doc.layers.len();
        hold(&mut studio, &[(160.0, 530.0), (200.0, 600.0), (260.0, 650.0)], alt);
        studio.released();
        assert!(studio.doc.layers.len() > count && studio.board.lasso.is_none(), "copied");
    }
}
