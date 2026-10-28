//! Snapping to the spacing of the rest, as Figma does: what is moved
//! between two layers or beside one stops where the gaps either side are
//! the same, or the same as a gap already between two others; and what
//! is sized stops at a width or a height another layer has.

use super::*;

/// A gap shown while something is moved: across, from one edge to the
/// other, at a height; or, `down`, the same turned.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Gap {
    pub down: bool,
    pub from: f32,
    pub to: f32,
    pub at: f32,
}

/// A box as its left, top, right and bottom.
pub(super) type Edges = [f32; 4];

/// Whether two boxes share some of their height.
fn level(a: Edges, b: Edges) -> bool {
    a[1] < b[3] && b[1] < a[3]
}

/// The gaps between boxes beside each other with nothing between them,
/// each as the box on the left, the one on the right, and how wide.
fn gaps_between(others: &[Edges]) -> Vec<(Edges, Edges, f32)> {
    let mut found = Vec::new();
    for &a in others {
        for &b in others {
            let gap = b[0] - a[2];
            if gap <= 0.0 || !level(a, b) {
                continue;
            }
            let between = others.iter().any(|&c| c != a && c != b && level(a, c) && level(b, c) && c[0] >= a[2] && c[2] <= b[0]);
            if !between {
                found.push((a, b, gap));
            }
        }
    }
    found
}

/// How far across a box moved to `me` is to go to space it evenly with
/// what is beside it, if that is within `reach`: between its neighbors
/// with the same gap either side, or a gap from one of them the same as
/// one already between two others. And the gaps to show for it.
pub(super) fn equal_gap(me: Edges, others: &[Edges], reach: f32) -> Option<(f32, Vec<Gap>)> {
    let wide = me[2] - me[0];
    let beside: Vec<Edges> = others.iter().copied().filter(|&other| level(me, other)).collect();
    let left = beside.iter().copied().filter(|other| other[2] <= me[0] + reach).max_by(|a, b| a[2].total_cmp(&b[2]));
    let right = beside.iter().copied().filter(|other| other[0] >= me[2] - reach).min_by(|a, b| a[0].total_cmp(&b[0]));
    let known = gaps_between(others);
    let mut choices: Vec<(f32, f32)> = Vec::new();
    if let (Some(l), Some(r)) = (left, right) {
        let gap = (r[0] - l[2] - wide) / 2.0;
        if gap > 0.0 {
            choices.push((l[2] + gap - me[0], gap));
        }
    }
    for &(_, _, gap) in &known {
        if let Some(l) = left {
            choices.push((l[2] + gap - me[0], gap));
        }
        if let Some(r) = right {
            choices.push((r[0] - gap - wide - me[0], gap));
        }
    }
    let (by, gap) = choices.into_iter().filter(|(by, _)| by.abs() <= reach).min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))?;
    let placed = [me[0] + by, me[1], me[2] + by, me[3]];
    let middle = |a: Edges, b: Edges| (a[1].max(b[1]) + a[3].min(b[3])) / 2.0;
    let same = |width: f32| (width - gap).abs() < 0.5;
    let mut shown = Vec::new();
    if let Some(l) = left.filter(|l| same(placed[0] - l[2])) {
        shown.push(Gap { down: false, from: l[2], to: placed[0], at: middle(l, placed) });
    }
    if let Some(r) = right.filter(|r| same(r[0] - placed[2])) {
        shown.push(Gap { down: false, from: placed[2], to: r[0], at: middle(placed, r) });
    }
    for (a, b, width) in known {
        if same(width) {
            shown.push(Gap { down: false, from: a[2], to: b[0], at: middle(a, b) });
        }
    }
    Some((by, shown))
}

/// The same, down: the boxes turned on their side, and back.
pub(super) fn equal_gap_down(me: Edges, others: &[Edges], reach: f32) -> Option<(f32, Vec<Gap>)> {
    let turn = |edges: Edges| [edges[1], edges[0], edges[3], edges[2]];
    let others: Vec<Edges> = others.iter().map(|&other| turn(other)).collect();
    let (by, gaps) = equal_gap(turn(me), &others, reach)?;
    Some((by, gaps.into_iter().map(|gap| Gap { down: true, ..gap }).collect()))
}

/// The size nearest to `size` among `sizes`, if one is within `reach`.
pub(super) fn nearest_size(size: f32, sizes: &[f32], reach: f32) -> Option<f32> {
    sizes.iter().copied().filter(|other| (other - size).abs() <= reach).min_by(|a, b| (a - size).abs().total_cmp(&(b - size).abs()))
}

impl Studio {
    /// The boxes of the layers not picked out, as edges.
    pub(super) fn other_edges(&mut self) -> Vec<Edges> {
        let chosen = self.chosen();
        (0..self.doc.layers.len())
            .filter(|index| !chosen.contains(index) && !self.doc.layers[*index].hidden)
            .map(|index| {
                let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
                [x, y, x + w, y + h]
            })
            .collect()
    }

    /// The gaps shown while moving, in magenta, each with how wide it is.
    pub(super) fn draw_gaps(&mut self, canvas: &mut Canvas) {
        const MAGENTA: u32 = 0xF0359A;
        for gap in self.gaps.clone() {
            let (a, b) = match gap.down {
                false => (self.to_window((gap.from, gap.at)), self.to_window((gap.to, gap.at))),
                true => (self.to_window((gap.at, gap.from)), self.to_window((gap.at, gap.to))),
            };
            match gap.down {
                false => {
                    canvas.fill(a.0, a.1, b.0 - a.0, 1, MAGENTA);
                    canvas.fill(a.0, a.1 - 4, 1, 9, MAGENTA);
                    canvas.fill(b.0, b.1 - 4, 1, 9, MAGENTA);
                }
                true => {
                    canvas.fill(a.0, a.1, 1, b.1 - a.1, MAGENTA);
                    canvas.fill(a.0 - 4, a.1, 9, 1, MAGENTA);
                    canvas.fill(b.0 - 4, b.1, 9, 1, MAGENTA);
                }
            }
            let says = format!("{}", (gap.to - gap.from).round() as i32);
            let wide = self.text.width(&says, 11) + 8;
            let (cx, cy) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
            let (x, y) = if gap.down { (cx + 6, cy - 8) } else { (cx - wide / 2, cy - 20) };
            canvas.round_fill(x, y, wide, 16, 4, MAGENTA, 256);
            self.text.draw(canvas, x + 4, y + (16 - Text::line_height(11)) / 2, 11, 0xFFFFFF, &says);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_moved_between_two_stops_with_the_same_gap_either_side() {
        let others = [[0.0, 0.0, 100.0, 50.0], [300.0, 0.0, 400.0, 50.0]];
        // 100 wide, between gaps of 50; a little off, it is pulled there.
        let (by, gaps) = equal_gap([153.0, 10.0, 253.0, 40.0], &others, 6.0).unwrap();
        assert_eq!(by, -3.0);
        assert_eq!(gaps.len(), 2);
        assert!(gaps.iter().all(|gap| (gap.to - gap.from - 50.0).abs() < 0.01));
        // Too far off, it is left be; and level with neither, too.
        assert!(equal_gap([170.0, 10.0, 270.0, 40.0], &others, 6.0).is_none());
        assert!(equal_gap([150.0, 80.0, 250.0, 120.0], &others, 6.0).is_none());
    }

    #[test]
    fn what_is_moved_beside_one_takes_a_gap_two_others_have() {
        // Two 40 apart; a third beside the second stops 40 from it.
        let others = [[0.0, 0.0, 100.0, 50.0], [140.0, 0.0, 240.0, 50.0]];
        let (by, gaps) = equal_gap([282.0, 0.0, 330.0, 50.0], &others, 6.0).unwrap();
        assert_eq!(by, -2.0);
        assert_eq!(gaps.len(), 2);
        // Down works the same, turned.
        let others = [[0.0, 0.0, 50.0, 100.0], [0.0, 140.0, 50.0, 240.0]];
        let (by, gaps) = equal_gap_down([0.0, 283.0, 50.0, 330.0], &others, 6.0).unwrap();
        assert_eq!((by, gaps[0].down), (-3.0, true));
    }

    #[test]
    fn a_size_snaps_to_the_nearest_size_another_has() {
        assert_eq!(nearest_size(197.0, &[50.0, 199.0, 194.0], 4.0), Some(199.0));
        assert_eq!(nearest_size(197.0, &[50.0, 200.0], 4.0), Some(200.0));
        assert_eq!(nearest_size(197.0, &[50.0, 190.0], 4.0), None);
    }
}

/// How long Shift or Alt still counts as held after it is let go.
pub(super) const MODS_HOLD: std::time::Duration = std::time::Duration::from_millis(150);

impl Studio {
    /// The keys held as they should be taken: in a drag, just as they are,
    /// so that Shift pressed or let go partway changes it then, as in
    /// Figma; out of one, Shift and Alt a moment after they are let go,
    /// as a hand lets go of them a little before the button.
    pub(super) fn steady_mods(&mut self, mut mods: Mods) -> Mods {
        let now = std::time::Instant::now();
        if mods.shift {
            self.seen_mods.0 = Some(now);
        }
        if mods.alt {
            self.seen_mods.1 = Some(now);
        }
        if self.dragging() {
            return mods;
        }
        let recent = |seen: Option<std::time::Instant>| seen.is_some_and(|when| now.duration_since(when) < MODS_HOLD);
        mods.shift |= recent(self.seen_mods.0);
        mods.alt |= recent(self.seen_mods.1);
        mods
    }

    /// Whether something is being dragged on the canvas, which the keys
    /// held change as they change.
    pub fn dragging(&self) -> bool {
        !matches!(self.drag, Drag::None | Drag::Pan { .. } | Drag::Picker { .. } | Drag::Slide { .. } | Drag::Scrub { .. })
    }

    /// A key held changed in a drag: the drag again, where the pointer is,
    /// as the keys are now.
    pub fn mods_changed(&mut self, mods: Mods) {
        if self.dragging() {
            let (x, y) = self.pointer;
            self.pointer_moved(x, y, mods);
        }
    }
}

#[cfg(test)]
mod held {
    use super::*;

    #[test]
    fn shift_holds_a_moment_after_it_is_let_go_but_in_a_drag_is_as_it_is() {
        let mut studio = Studio::new(Doc::new(100, 100), Text::load(), 800, 600, std::env::temp_dir(), "t.png".into());
        let shift = Mods { shift: true, ..Mods::default() };
        assert!(!studio.steady_mods(Mods::default()).shift);
        assert!(studio.steady_mods(shift).shift);
        // Let go just now: still held.
        assert!(studio.steady_mods(Mods::default()).shift);
        studio.seen_mods.0 = Some(std::time::Instant::now() - MODS_HOLD * 2);
        assert!(!studio.steady_mods(Mods::default()).shift);
        // In a drag, as it is: let go, at once let go.
        studio.drag = Drag::Draw { from: (0.0, 0.0) };
        assert!(studio.steady_mods(shift).shift);
        assert!(!studio.steady_mods(Mods::default()).shift);
        studio.drag = Drag::None;
        studio.seen_mods.0 = Some(std::time::Instant::now() - MODS_HOLD * 2);
        assert!(!studio.steady_mods(Mods::default()).shift);
    }
}
