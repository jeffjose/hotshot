//! Snapping and spacing, as Figma does them, for the canvas and capture
//! mode alike: what is moved stops with an edge or its middle on another's,
//! between two with the gaps either side the same, or beside one with a
//! gap two others already have; what is sized stops at a width or a height
//! another has; and a copy goes beside what it is a copy of, not on it.

use crate::canvas::Canvas;
use crate::text::Text;

/// A gap shown while something is moved: across, from one edge to the
/// other, at a height; or, `down`, the same turned.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Gap {
    pub down: bool,
    pub from: f32,
    pub to: f32,
    pub at: f32,
}

impl Gap {
    /// The same gap, moved.
    pub fn moved(self, dx: f32, dy: f32) -> Gap {
        match self.down {
            false => Gap { from: self.from + dx, to: self.to + dx, at: self.at + dy, ..self },
            true => Gap { from: self.from + dy, to: self.to + dy, at: self.at + dx, ..self },
        }
    }
}

/// A box as its left, top, right and bottom.
pub type Edges = [f32; 4];

/// Whether two boxes share some of their height.
fn level(a: Edges, b: Edges) -> bool {
    a[1] < b[3] && b[1] < a[3]
}

/// Whether two boxes cover some of the same ground; touching is not.
fn overlap(a: Edges, b: Edges) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
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
pub fn equal_gap(me: Edges, others: &[Edges], reach: f32) -> Option<(f32, Vec<Gap>)> {
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
pub fn equal_gap_down(me: Edges, others: &[Edges], reach: f32) -> Option<(f32, Vec<Gap>)> {
    let turn = |edges: Edges| [edges[1], edges[0], edges[3], edges[2]];
    let others: Vec<Edges> = others.iter().map(|&other| turn(other)).collect();
    let (by, gaps) = equal_gap(turn(me), &others, reach)?;
    Some((by, gaps.into_iter().map(|gap| Gap { down: true, ..gap }).collect()))
}

/// The size nearest to `size` among `sizes`, if one is within `reach`.
pub fn nearest_size(size: f32, sizes: &[f32], reach: f32) -> Option<f32> {
    sizes.iter().copied().filter(|other| (other - size).abs() <= reach).min_by(|a, b| (a - size).abs().total_cmp(&(b - size).abs()))
}

/// Where a box being moved has snapped to: how much further it goes, the
/// lines it has gone to, up and down at an x or across at a y, and the
/// gaps it is spaced by.
#[derive(Default, Debug)]
pub struct Snapped {
    pub by: (f32, f32),
    pub guides: Vec<(bool, f32)>,
    pub gaps: Vec<Gap>,
}

/// A box moved to `me` snapped, each way, to whichever is nearer within
/// `reach`: one of its edges or its middle onto one of the lines, up and
/// down at the xs of `across` and across at the ys of `down`; or a place
/// where it is spaced evenly with `others`.
pub fn snap_box(me: Edges, across: &[f32], down: &[f32], others: &[Edges], reach: f32) -> Snapped {
    // The nearest of its edges and middle to any of the lines.
    let nearest = |mine: [f32; 3], lines: &[f32]| -> Option<(f32, f32)> {
        let mut best: Option<(f32, f32)> = None;
        for &line in lines {
            for &at in &mine {
                let by = line - at;
                if by.abs() <= reach && best.is_none_or(|(was, _)| by.abs() < was.abs()) {
                    best = Some((by, line));
                }
            }
        }
        best
    };
    let mut snapped = Snapped::default();
    let line_x = nearest([me[0], (me[0] + me[2]) / 2.0, me[2]], across);
    match (line_x, equal_gap(me, others, reach)) {
        (line, Some((by, gaps))) if line.is_none_or(|(line_by, _)| by.abs() < line_by.abs()) => {
            snapped.by.0 = by;
            snapped.gaps.extend(gaps);
        }
        (Some((by, line)), _) => {
            snapped.by.0 = by;
            snapped.guides.push((true, line));
        }
        _ => {}
    }
    let line_y = nearest([me[1], (me[1] + me[3]) / 2.0, me[3]], down);
    match (line_y, equal_gap_down(me, others, reach)) {
        (line, Some((by, gaps))) if line.is_none_or(|(line_by, _)| by.abs() < line_by.abs()) => {
            snapped.by.1 = by;
            snapped.gaps.extend(gaps);
        }
        (Some((by, line)), _) => {
            snapped.by.1 = by;
            snapped.guides.push((false, line));
        }
        _ => {}
    }
    snapped
}

/// The edges and middles of boxes, across and down, for others to snap to.
pub fn lines_of(boxes: &[Edges]) -> (Vec<f32>, Vec<f32>) {
    boxes.iter().map(|b| ([b[0], (b[0] + b[2]) / 2.0, b[2]], [b[1], (b[1] + b[3]) / 2.0, b[3]])).fold((Vec::new(), Vec::new()), |(mut across, mut down), (x, y)| {
        across.extend(x);
        down.extend(y);
        (across, down)
    })
}

/// How far a first copy of the boxes round `me` goes, as Figma puts one:
/// beside it to the right, `gap` away, and on past anything already there;
/// or, if that would leave `area`, below it the same way. What `me` sits
/// on, a box round all of it, is not in the way. With nowhere free inside,
/// the first place beside it that is inside; and when it is too large for
/// any, as far to the right as `area` has room for, or else down, or else
/// a step down and to the right: never on top of it, and never where a
/// frame would hide it.
pub fn beside(me: Edges, others: &[Edges], area: Option<Edges>, gap: f32) -> (f32, f32) {
    let (wide, high) = (me[2] - me[0], me[3] - me[1]);
    let inside = |b: Edges| area.is_none_or(|a| b[0] >= a[0] - 0.5 && b[1] >= a[1] - 0.5 && b[2] <= a[2] + 0.5 && b[3] <= a[3] + 0.5);
    let under = |o: &Edges| o[0] <= me[0] && o[1] <= me[1] && o[2] >= me[2] && o[3] >= me[3];
    let in_way: Vec<Edges> = others.iter().copied().filter(|o| !under(o)).collect();
    let moved = |(dx, dy): (f32, f32)| [me[0] + dx, me[1] + dy, me[2] + dx, me[3] + dy];
    // Along one way, from the first place, past each thing in the way.
    let along = |down: bool| -> Option<(f32, f32)> {
        let mut by = if down { (0.0, high + gap) } else { (wide + gap, 0.0) };
        for _ in 0..=in_way.len() {
            let place = moved(by);
            if !inside(place) {
                return None;
            }
            let hit: Vec<Edges> = in_way.iter().copied().filter(|&o| overlap(o, place)).collect();
            if hit.is_empty() {
                return Some(by);
            }
            match down {
                false => by.0 = hit.iter().map(|o| o[2]).fold(f32::MIN, f32::max) + gap - me[0],
                true => by.1 = hit.iter().map(|o| o[3]).fold(f32::MIN, f32::max) + gap - me[1],
            }
        }
        None
    };
    if let Some(by) = along(false).or_else(|| along(true)) {
        return by;
    }
    if let Some(by) = [(wide + gap, 0.0), (0.0, high + gap), (-wide - gap, 0.0), (0.0, -high - gap)].into_iter().find(|&by| inside(moved(by))) {
        return by;
    }
    let Some(a) = area else { return (wide + gap, 0.0) };
    let (right, below) = (a[2] - me[2], a[3] - me[3]);
    if right >= gap {
        (right.min(wide + gap), 0.0)
    } else if below >= gap {
        (0.0, below.min(high + gap))
    } else {
        (gap, gap)
    }
}

/// The gaps shown while moving, in magenta, each with how wide it is, its
/// number `px` high; `to_window` says where a point of them is drawn.
pub fn draw_gaps(canvas: &mut Canvas, text: &mut Text, gaps: &[Gap], px: u32, to_window: impl Fn((f32, f32)) -> (i32, i32)) {
    const MAGENTA: u32 = 0xF0359A;
    let tick = (px as i32 * 4 / 11).max(4);
    for gap in gaps {
        let (a, b) = match gap.down {
            false => (to_window((gap.from, gap.at)), to_window((gap.to, gap.at))),
            true => (to_window((gap.at, gap.from)), to_window((gap.at, gap.to))),
        };
        match gap.down {
            false => {
                canvas.fill(a.0, a.1, b.0 - a.0, 1, MAGENTA);
                canvas.fill(a.0, a.1 - tick, 1, 2 * tick + 1, MAGENTA);
                canvas.fill(b.0, b.1 - tick, 1, 2 * tick + 1, MAGENTA);
            }
            true => {
                canvas.fill(a.0, a.1, 1, b.1 - a.1, MAGENTA);
                canvas.fill(a.0 - tick, a.1, 2 * tick + 1, 1, MAGENTA);
                canvas.fill(b.0 - tick, b.1, 2 * tick + 1, 1, MAGENTA);
            }
        }
        let says = format!("{}", (gap.to - gap.from).round() as i32);
        let high = Text::line_height(px) + 2;
        let wide = text.width(&says, px) + 8;
        let (cx, cy) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        let (x, y) = if gap.down { (cx + 6, cy - high / 2) } else { (cx - wide / 2, cy - high - 4) };
        canvas.round_fill(x, y, wide, high, 4, MAGENTA, 256);
        text.draw(canvas, x + 4, y + (high - Text::line_height(px)) / 2, px, 0xFFFFFF, &says);
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

    #[test]
    fn a_box_snaps_to_the_nearer_of_a_line_and_an_even_gap() {
        let others = [[0.0, 0.0, 100.0, 100.0], [300.0, 0.0, 400.0, 100.0]];
        let (across, down) = lines_of(&others);
        // Its top 4 under the others' top goes up to it; and it sits
        // 3 off halfway between the two, goes there.
        let snapped = snap_box([147.0, 4.0, 247.0, 104.0], &across, &down, &others, 6.0);
        assert_eq!(snapped.by, (3.0, -4.0));
        assert_eq!(snapped.guides, [(false, 0.0)]);
        assert_eq!(snapped.gaps.len(), 2);
        // Nothing near: nothing.
        let snapped = snap_box([160.0, 200.0, 200.0, 240.0], &across, &down, &others, 6.0);
        assert_eq!((snapped.by, snapped.guides.len(), snapped.gaps.len()), ((0.0, 0.0), 0, 0));
    }

    #[test]
    fn a_first_copy_goes_beside_past_what_is_there_or_below() {
        let me = [10.0, 10.0, 60.0, 40.0];
        // Room: to the right, 20 away.
        assert_eq!(beside(me, &[], Some([0.0, 0.0, 500.0, 500.0]), 20.0), (70.0, 0.0));
        // Something there: past it.
        assert_eq!(beside(me, &[[90.0, 0.0, 200.0, 30.0]], Some([0.0, 0.0, 500.0, 500.0]), 20.0), (210.0, 0.0));
        // No room to the right: below; what it sits on is not in the way.
        assert_eq!(beside(me, &[[0.0, 0.0, 100.0, 100.0]], Some([0.0, 0.0, 100.0, 100.0]), 20.0), (0.0, 50.0));
        // No room at all and no room to the right: down as far as there is.
        assert_eq!(beside(me, &[], Some([0.0, 0.0, 70.0, 70.0]), 20.0), (0.0, 30.0));
        // No room at all, the box nearly as wide as the frame: as far right
        // as the frame has room for, still in it, not on top.
        assert_eq!(beside(me, &[], Some([0.0, 0.0, 90.0, 50.0]), 20.0), (30.0, 0.0));
        // A box as large as the frame worked in, a step down and right.
        assert_eq!(beside([46.0, 46.0, 605.0, 404.0], &[], Some([0.0, 0.0, 620.0, 420.0]), 20.0), (20.0, 20.0));
        // Unbounded, frames go on past each other.
        assert_eq!(beside([0.0, 0.0, 100.0, 100.0], &[[150.0, 50.0, 300.0, 150.0], [450.0, 0.0, 500.0, 10.0]], None, 100.0), (600.0, 0.0));
    }
}
