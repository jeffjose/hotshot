//! A flowchart by the keys, as FigJam's is: with a box picked out, or
//! typed into, Ctrl and an arrow key makes the next box that way, like
//! it, at the usual gap and clear of the rest, joined to it by an arrow
//! going the board's way, and typed into; again, and the next goes beside
//! the last. Alt and an arrow key picks out the box joined to it that way,
//! or the nearest that way where none is. Ctrl held a moment shows,
//! faintly, where each would go.

use super::*;
use dots::{GAP_ACROSS, GAP_DOWN, SIDES};

/// How long Ctrl is held alone before where the next box would go shows.
pub(in crate::studio) const CTRL_WAIT: std::time::Duration = std::time::Duration::from_millis(400);

/// How many places beside the first a new box is tried in, each way,
/// before it goes farther along instead.
const TRIES: i32 = 6;

/// Which of `SIDES` an arrow key goes: up, right, down, left.
fn way_of(key: Key) -> Option<usize> {
    match key {
        Key::Up => Some(0),
        Key::Right => Some(1),
        Key::Down => Some(2),
        Key::Left => Some(3),
        _ => None,
    }
}

impl Studio {
    /// The box the keys make the next box from, or go from: the one being
    /// typed into, or the one picked out.
    fn flow_from(&self) -> Option<usize> {
        self.editing.or_else(|| self.picked()).filter(|&index| self.is_node(index) && !self.doc.layers[index].hidden)
    }

    /// Ctrl or Alt and an arrow key on a board: the next box made that way,
    /// or the box that way picked out. Whether the key was taken.
    pub(in crate::studio) fn flow_key(&mut self, key: Key, mods: Mods) -> bool {
        let Some(way) = way_of(key) else { return false };
        if mods.shift || mods.ctrl == mods.alt || self.presenting() {
            return false;
        }
        let Some(from) = self.flow_from() else { return false };
        match mods.ctrl {
            true => self.flow_next(from, way),
            false => self.flow_step(from, way),
        }
        // Ctrl still held: where the next would go shows again in a moment.
        if self.board.ctrl.is_some() {
            self.board.ctrl = Some(std::time::Instant::now());
        }
        self.dirty = true;
        true
    }

    /// The size the next box like another is made: the size it was drawn
    /// at, which its words grew it from; a box as the box tool makes one,
    /// for a picture or a box with a title.
    pub(in crate::studio) fn like_size(&self, like: usize) -> (f32, f32) {
        let Some(Layer { kind: Kind::Shape(source), .. }) = self.doc.layers.get(like) else { return BOX };
        let (_, _, w, h) = source.frame();
        match source.base {
            (bw, bh) if bw > 0.0 && bh > 0.0 && !source.titled => (bw, bh),
            _ if source.titled => BOX,
            _ => (w, h),
        }
    }

    /// Where the next box made from one that way goes, as its frame: its
    /// near side the usual gap from the box, as far as at their own size,
    /// in line with it; where that is taken, beside it, below or to the
    /// right first, then above or to the left, and so on out; and where
    /// all of those are, on along that way.
    pub(in crate::studio) fn flow_spot(&self, from: usize, way: usize) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = self.box_frame(from);
        let (nw, nh) = self.like_size(from);
        let by = match &self.doc.layers[from].kind {
            Kind::Shape(source) => bar::drawn_larger(source),
            Kind::Picture(_) => 1.0,
        };
        let (across, down) = (GAP_ACROSS * by, GAP_DOWN * by);
        let (dx, dy) = SIDES[way.min(3)];
        let first = (x + w / 2.0 + dx * (w / 2.0 + across + nw / 2.0), y + h / 2.0 + dy * (h / 2.0 + down + nh / 2.0));
        let beside = if dx != 0.0 { (0.0, nh + down) } else { (nw + across, 0.0) };
        let along = (dx * (nw + across), dy * (nh + down));
        let frame = |(cx, cy): (f32, f32)| (cx - nw / 2.0, cy - nh / 2.0, nw, nh);
        let mut skip = self.round(from);
        skip.push(self.id_of(from));
        let room = across.min(down) / 2.0;
        let clear = |f: (f32, f32, f32, f32)| {
            !(0..self.doc.layers.len()).any(|index| {
                let layer = &self.doc.layers[index];
                let solid = self.is_node(index) || matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Text);
                if !solid || layer.hidden || skip.contains(&self.id_of(index)) {
                    return false;
                }
                let o = self.box_frame(index);
                o.0 < f.0 + f.2 + room && o.0 + o.2 > f.0 - room && o.1 < f.1 + f.3 + room && o.1 + o.3 > f.1 - room
            })
        };
        let besides = std::iter::once(0).chain((1..=TRIES).flat_map(|step| [step, -step]));
        if let Some(found) = besides.map(|step| frame((first.0 + beside.0 * step as f32, first.1 + beside.1 * step as f32))).find(|&f| clear(f)) {
            return found;
        }
        (1..=4 * TRIES).map(|step| frame((first.0 + along.0 * step as f32, first.1 + along.1 * step as f32))).find(|&f| clear(f)).unwrap_or_else(|| frame(first))
    }

    /// Ctrl and an arrow key: the next box that way, like the one it is
    /// from, joined to it, picked out and typed into; one step to undo.
    fn flow_next(&mut self, from: usize, way: usize) {
        let id = self.id_of(from);
        self.commit_text();
        let Some(from) = self.index_of(id) else { return };
        let (x, y, w, h) = self.flow_spot(from, way);
        self.board.ghost = None;
        self.checkpoint();
        let made = self.box_like(from, (x + w / 2.0, y + h / 2.0), 0);
        self.join(from, made);
        self.selected = vec![made];
        self.edit_text(made);
        self.changed();
        self.flow_into_view(made);
    }

    /// Alt and an arrow key: the box that way picked out, of those joined
    /// to it by an arrow either way, or of all where none is joined that
    /// way; the nearest, by how far along that way it is and, more, how
    /// far off to the side. Nothing that way, nothing picked out.
    fn flow_step(&mut self, from: usize, way: usize) {
        let Some(to) = self.flow_toward(from, way) else { return };
        self.commit_text();
        let Some(to) = self.index_of(to) else { return };
        self.selected = vec![to];
        self.board.typed = None;
        self.flow_into_view(to);
    }

    /// The box Alt and an arrow key goes to from one, by what it is called.
    pub(in crate::studio) fn flow_toward(&self, from: usize, way: usize) -> Option<u32> {
        let id = self.id_of(from);
        let (x, y, w, h) = self.box_frame(from);
        let middle = (x + w / 2.0, y + h / 2.0);
        let (dx, dy) = SIDES[way.min(3)];
        let joined: Vec<u32> = self.links().into_iter().filter_map(|(_, a, b)| match (a == id, b == id) {
            (true, false) => Some(b),
            (false, true) => Some(a),
            _ => None,
        }).collect();
        // How far a box is that way, if it is that way at all: within a
        // little more than a right angle's half either side.
        let score = |index: usize| {
            let (ox, oy, ow, oh) = self.box_frame(index);
            let (vx, vy) = (ox + ow / 2.0 - middle.0, oy + oh / 2.0 - middle.1);
            let (along, aside) = (vx * dx + vy * dy, (vx * dy - vy * dx).abs());
            (along > 1.0 && aside <= along * 2.0).then_some(along + 2.0 * aside)
        };
        let nearest = |only_joined: bool| {
            (0..self.doc.layers.len()).filter(|&index| index != from && self.is_node(index) && !self.doc.layers[index].hidden).filter(|&index| !only_joined || joined.contains(&self.id_of(index))).filter_map(|index| Some((score(index)?, index))).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, index)| self.id_of(index))
        };
        nearest(true).or_else(|| nearest(false))
    }

    /// A box made or gone to by the keys brought into view, if it is not
    /// already wholly in it: the view glides over until it is, with room
    /// round it.
    fn flow_into_view(&mut self, index: usize) {
        let (x, y, w, h) = self.box_frame(index);
        self.bring_into_view((x, y, w, h));
    }

    /// Ctrl pressed or let go alone, on a board: held a moment, where the
    /// next box would go shows.
    pub fn ctrl_held(&mut self, down: bool) {
        if !self.on_board() {
            return;
        }
        self.board.ctrl = down.then(std::time::Instant::now);
        self.dirty = true;
    }

    /// How soon where the next box would go is to show, while Ctrl is held.
    pub(in crate::studio) fn ctrl_wait(&self) -> Option<std::time::Duration> {
        let since = self.board.ctrl?;
        let held = since.elapsed();
        (held < CTRL_WAIT).then(|| CTRL_WAIT - held)
    }

    /// Where Ctrl and each arrow key would put the next box, while Ctrl has
    /// been held a moment with a box picked out and nothing else done: up,
    /// right, down and left.
    pub(in crate::studio) fn flow_ghosts(&self) -> Option<[(f32, f32, f32, f32); 4]> {
        let since = self.board.ctrl?;
        if since.elapsed() < CTRL_WAIT || self.drag != Drag::None || self.presenting() || self.menu.is_some() || self.search.is_some() || self.board.keys {
            return None;
        }
        let from = self.flow_from()?;
        Some([0, 1, 2, 3].map(|way| self.flow_spot(from, way)))
    }

    /// Where the next box would go, each way, faint and dashed in the
    /// accent, with which way's key makes it.
    pub(in crate::studio) fn draw_flow_ghosts(&mut self, canvas: &mut Canvas) {
        let Some(spots) = self.flow_ghosts() else { return };
        for (way, (x, y, w, h)) in spots.into_iter().enumerate() {
            let (a, b) = (self.to_window_f((x, y)), self.to_window_f((x + w, y + h)));
            let rect = Rect::new(a.0.round() as i32, a.1.round() as i32, (b.0 - a.0).round() as i32, (b.1 - a.1).round() as i32);
            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 8, ACCENT, 18);
            faint_dashes(canvas, rect, ACCENT, 150);
            // The arrow key's chevron, in its middle.
            let (cx, cy) = (rect.x as f32 + rect.w as f32 / 2.0, rect.y as f32 + rect.h as f32 / 2.0);
            let (dx, dy) = SIDES[way];
            let (px, py) = (-dy, dx);
            let tip = (cx + dx * 4.0, cy + dy * 4.0);
            let wing = |side: f32| (cx - dx * 3.0 + px * 6.0 * side, cy - dy * 3.0 + py * 6.0 * side);
            canvas.disc(cx as i32, cy as i32, 13, ACCENT, 46);
            halo(canvas, &[wing(1.0), tip, wing(-1.0)], 1.0, ACCENT, 230);
        }
    }
}

/// A rectangle's edge in short dashes, two pixels wide, faint: where
/// something would go.
fn faint_dashes(canvas: &mut Canvas, rect: Rect, color: u32, amount: u32) {
    let (dash, gap) = (6, 5);
    for (x, y, long, across) in [(rect.x, rect.y, rect.w, true), (rect.x, rect.bottom() - 2, rect.w, true), (rect.x, rect.y, rect.h, false), (rect.right() - 2, rect.y, rect.h, false)] {
        let mut along = 0;
        while along < long {
            let run = dash.min(long - along);
            match across {
                true => canvas.round_fill(x + along, y, run, 2, 0, color, amount),
                false => canvas.round_fill(x, y + along, 2, run, 0, color, amount),
            }
            along += dash + gap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };
    const ALT: Mods = Mods { ctrl: false, shift: false, alt: true };

    /// A board with one box on it, called so, typed into.
    fn one_box(words: &str) -> Studio {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (400.0, 300.0), (560.0, 372.0), Mods::default());
        type_in(&mut studio, words);
        studio
    }

    #[test]
    fn ctrl_and_an_arrow_key_makes_the_next_box_that_way_joined_and_typed_into() {
        let mut studio = one_box("Start");
        // Typed into: Ctrl+Right makes the next, to the right at the gap.
        press_key(&mut studio, Key::Right, CTRL);
        assert_eq!(studio.doc.layers.len(), 3, "a box and an arrow");
        let made = studio.picked().unwrap();
        assert_eq!(studio.editing, Some(made), "typed into");
        let (x, y, w, h) = studio.box_frame(made);
        assert_eq!((x, y + h / 2.0, w, h), (560.5 + GAP_ACROSS, 336.5, 160.0, 72.0));
        assert_eq!(studio.links().len(), 1);
        type_in(&mut studio, "Next");
        assert_eq!(shape(&studio, made).text, "Next");
        assert_eq!(shape(&studio, 0).text, "Start", "the first as it was");
        // Down from that one, picked out and not typed into.
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Down, CTRL);
        let below = studio.picked().unwrap();
        let (bx, by, ..) = studio.box_frame(below);
        assert_eq!((bx, by), (x, y + h + GAP_DOWN));
        // Left and up, each at its gap; and the plain arrows still nudge.
        press_key(&mut studio, Key::Left, CTRL);
        let left = studio.picked().unwrap();
        assert_eq!(studio.box_frame(left).0 + 160.0 + GAP_ACROSS, bx);
        press_key(&mut studio, Key::Escape, Mods::default());
        let was = studio.box_frame(left);
        press_key(&mut studio, Key::Up, Mods::default());
        assert_eq!(studio.box_frame(left).1, was.1 - 1.0, "nudged");
        assert_eq!(studio.links().len(), 3);
        // Nothing picked out, nothing made.
        studio.selected.clear();
        let count = studio.doc.layers.len();
        press_key(&mut studio, Key::Right, CTRL);
        assert_eq!(studio.doc.layers.len(), count);
    }

    #[test]
    fn the_next_box_goes_beside_the_last_where_that_way_is_taken() {
        let mut studio = one_box("Decide");
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Right, CTRL);
        let first = studio.picked().unwrap();
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected = vec![0];
        press_key(&mut studio, Key::Right, CTRL);
        let second = studio.picked().unwrap();
        let (a, b) = (studio.box_frame(first), studio.box_frame(second));
        assert_eq!(a.0, b.0, "in the same column");
        assert_eq!(b.1, a.1 + a.3 + GAP_DOWN, "below the first, at the gap");
        // And a third, above it.
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected = vec![0];
        press_key(&mut studio, Key::Right, CTRL);
        let third = studio.box_frame(studio.picked().unwrap());
        assert_eq!((third.0, third.1), (a.0, a.1 - a.3 - GAP_DOWN));
        // All three joined from the first.
        let id = studio.id_of(0);
        assert_eq!(studio.links().iter().filter(|(_, from, _)| *from == id).count(), 3);
    }

    #[test]
    fn alt_and_an_arrow_key_picks_out_the_box_joined_that_way_or_the_nearest() {
        let mut studio = one_box("Hub");
        press_key(&mut studio, Key::Right, CTRL);
        let right = studio.picked().unwrap();
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected = vec![0];
        press_key(&mut studio, Key::Down, CTRL);
        let below = studio.picked().unwrap();
        press_key(&mut studio, Key::Escape, Mods::default());
        // A box nearer to the right, joined to nothing.
        studio.selected.clear();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (575.0, 350.0), (615.0, 380.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let loose = studio.doc.layers.len() - 1;
        studio.selected = vec![0];
        let before = studio.box_frame(0);
        press_key(&mut studio, Key::Right, ALT);
        assert_eq!(studio.selected, vec![right], "the joined one, not the nearer");
        assert_eq!(studio.box_frame(0), before, "nothing moved");
        press_key(&mut studio, Key::Left, ALT);
        assert_eq!(studio.selected, vec![0], "and back");
        press_key(&mut studio, Key::Down, ALT);
        assert_eq!(studio.selected, vec![below]);
        // From one joined to nothing that way, the nearest box that way.
        press_key(&mut studio, Key::Right, ALT);
        assert_eq!(studio.selected, vec![loose]);
        // Typed into, it is done with and the other picked out.
        studio.selected = vec![0];
        press_key(&mut studio, Key::Enter, Mods::default());
        assert_eq!(studio.editing, Some(0));
        press_key(&mut studio, Key::Right, ALT);
        assert_eq!((studio.editing, studio.selected.clone()), (None, vec![right]));
    }

    #[test]
    fn ctrl_held_a_moment_shows_where_each_arrow_key_would_put_the_next_box() {
        let mut studio = one_box("Here");
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.ctrl_held(true);
        assert!(studio.flow_ghosts().is_none(), "not at once");
        assert!(studio.ctrl_wait().is_some());
        let mut before = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut before);
        studio.board.ctrl = Some(std::time::Instant::now() - CTRL_WAIT);
        let ghosts = studio.flow_ghosts().expect("shown once held");
        let mut after = vec![0u32; before.len()];
        studio.draw(&mut after);
        let (x, y, w, h) = ghosts[1];
        let spot = studio.to_window((x + 1.0, y + h / 2.0));
        assert_ne!(before[(spot.1 * studio.width + spot.0) as usize], after[(spot.1 * studio.width + spot.0) as usize], "drawn there");
        let _ = w;
        // Each as the key then makes it.
        for (way, key) in [Key::Up, Key::Right, Key::Down, Key::Left].into_iter().enumerate() {
            let mut each = one_box("Here");
            press_key(&mut each, Key::Escape, Mods::default());
            each.ctrl_held(true);
            each.board.ctrl = Some(std::time::Instant::now() - CTRL_WAIT);
            let shown = each.flow_ghosts().unwrap()[way];
            press_key(&mut each, key, CTRL);
            assert_eq!(each.box_frame(each.picked().unwrap()), shown, "{key:?}");
        }
        // Let go, nothing shows.
        studio.ctrl_held(false);
        assert!(studio.flow_ghosts().is_none());
    }

    /// A box with one made to its right by Ctrl+Right, and a branch under
    /// that by Ctrl+Right again, then Ctrl held over the first, in each look:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test flow_snapshot -- --ignored
    #[test]
    #[ignore]
    fn flow_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        for look in 0..=look::DIAGRAM {
            let mut studio = one_box("Request");
            studio.set_look(Some(look), None);
            press_key(&mut studio, Key::Right, CTRL);
            type_in(&mut studio, "Valid?");
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected = vec![0];
            press_key(&mut studio, Key::Right, CTRL);
            type_in(&mut studio, "Reject");
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected = vec![0];
            studio.ctrl_held(true);
            studio.board.ctrl = Some(std::time::Instant::now() - CTRL_WAIT);
            let mut out = vec![0u32; (studio.width * studio.height) as usize];
            studio.draw(&mut out);
            let (x, y, w, h) = (150, 120, 900, 460);
            let crop: Vec<u32> = (y..y + h).flat_map(|row| out[(row * studio.width + x) as usize..(row * studio.width + x + w) as usize].to_vec()).collect();
            std::fs::write(to.join(format!("flow-{look}.png")), crate::output::encode(w as u32, h as u32, &crop, false).unwrap()).unwrap();
            // And one made below, and one to the left of that.
            press_key(&mut studio, Key::Down, CTRL);
            type_in(&mut studio, "Log");
            press_key(&mut studio, Key::Left, CTRL);
            type_in(&mut studio, "Done");
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected.clear();
            studio.ctrl_held(false);
            studio.draw(&mut out);
            let crop: Vec<u32> = (y..y + h).flat_map(|row| out[(row * studio.width + x) as usize..(row * studio.width + x + w) as usize].to_vec()).collect();
            std::fs::write(to.join(format!("flow-made-{look}.png")), crate::output::encode(w as u32, h as u32, &crop, false).unwrap()).unwrap();
        }
    }
}
