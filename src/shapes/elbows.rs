//! The way an elbowed arrow goes between two things, as a diagram's
//! connectors go: out of a side of one square to it, a short way straight
//! on, then across and down, only ever turning a right angle, round
//! whatever else is in the way, and square into a side of the other; as
//! few turns as will do, and of those the shortest.

use super::Point;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// The ways out of a side, as `PORTS` has the sides: up, right, down and
/// left.
const WAYS: [(f32, f32); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
/// What a turn costs against a pixel of going on: more than any way could
/// be long, so that the fewest turns win and the shortest of those.
pub const TURN: f64 = 1.0e6;
/// How near a way may go to something in it, besides the stretch out of
/// a side, in pixels.
const PAD: f32 = 2.0;
/// How many places it looks at before it gives up, for a board so crowded
/// it would take too long.
const MOST: usize = 40_000;

/// A thing a way goes round, as its left, top, right and bottom.
pub type Block = [f32; 4];
/// Where a way may begin or end: the point on a side it leaves or comes
/// into, which side it is, by `WAYS`, and what going by it costs more
/// than by another, against a pixel of going on.
pub type End = (Point, usize, f64);

/// A place it has got to, the way it is going, and what that cost, in
/// the order the cheapest is looked at first.
#[derive(PartialEq)]
struct Step {
    guess: f64,
    key: usize,
}

impl Eq for Step {}

impl Ord for Step {
    fn cmp(&self, other: &Self) -> Ordering {
        other.guess.total_cmp(&self.guess).then(other.key.cmp(&self.key))
    }
}

impl PartialOrd for Step {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Whether a straight stretch from one point to another goes clear of
/// every block, a little way out from each.
pub fn clear(a: Point, b: Point, blocks: &[Block]) -> bool {
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    blocks.iter().all(|r| !(x1 > r[0] - PAD && x0 < r[2] + PAD && y1 > r[1] - PAD && y0 < r[3] + PAD))
}

/// Some numbers in order, with those within a hair of the one before
/// left out.
fn lines(mut all: Vec<f32>) -> Vec<f32> {
    all.retain(|value| value.is_finite());
    all.sort_by(f32::total_cmp);
    all.dedup_by(|b, a| (*b - *a).abs() < 1e-3);
    all
}

/// Which of some lines in order a value is on, or nearest.
fn index(lines: &[f32], value: f32) -> usize {
    let at = lines.partition_point(|&line| line < value);
    match at {
        0 => 0,
        _ if at == lines.len() => at - 1,
        _ if (lines[at] - value).abs() <= (value - lines[at - 1]).abs() => at,
        _ => at - 1,
    }
}

/// The way from one of some sides to one of others, each an `End`, the
/// cheapest of them: the fewest turns, with what each end costs more, and
/// of those the shortest. Round the blocks, the two things joined the
/// first two of them; `stub` the way straight out of a side before it may
/// turn, and how far from what it goes past it keeps where it can. Gives
/// which side it went from and which into, and the points it turns at,
/// from its first end to its last, those ends with them.
pub fn orthogonal(starts: &[End], goals: &[End], blocks: &[Block], stub: f32) -> Option<(usize, usize, Vec<Point>)> {
    if starts.is_empty() || goals.is_empty() {
        return None;
    }
    let out = |(at, side, _): End| (at.0 + WAYS[side % 4].0 * stub, at.1 + WAYS[side % 4].1 * stub);
    // The lines it can go along: out from each side's stub, a stub's
    // width off each block, and half way between the two things joined.
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for &end in starts.iter().chain(goals) {
        let at = out(end);
        xs.push(at.0);
        ys.push(at.1);
    }
    for r in blocks {
        xs.extend([r[0] - stub, r[2] + stub]);
        ys.extend([r[1] - stub, r[3] + stub]);
    }
    if let [a, b, ..] = blocks {
        for (low, high) in [(a, b), (b, a)] {
            if low[2] < high[0] {
                xs.push((low[2] + high[0]) / 2.0);
            }
            if low[3] < high[1] {
                ys.push((low[3] + high[1]) / 2.0);
            }
        }
    }
    let (xs, ys) = (lines(xs), lines(ys));
    let (wide, high) = (xs.len(), ys.len());
    let node = |at: Point| index(&xs, at.0) * high + index(&ys, at.1);
    let place = |node: usize| (xs[node / high], ys[node % high]);
    // The places it ends at, going in to each side: the opposite way to
    // out of it.
    let ends: Vec<(usize, Point, f64)> = goals.iter().map(|&goal| (node(out(goal)) * 4 + (goal.1 + 2) % 4, out(goal), goal.2)).collect();
    // How much more it must cost at least from a place, going some way:
    // as far as the nearest end across and down, and the turns there must
    // be to come into it the way it goes in: none straight on to it; one
    // where it is ahead both ways, three where not; two for a step on or a
    // turn back, and four for a step on to somewhere behind.
    let guess = |key: usize| {
        let (at, way) = (place(key / 4), key % 4);
        ends.iter()
            .map(|&(end, to, _)| {
                let into = end % 4;
                let (dx, dy) = (to.0 - at.0, to.1 - at.1);
                let (ahead, aside) = (dx * WAYS[way].0 + dy * WAYS[way].1, (dx * WAYS[way].1 - dy * WAYS[way].0).abs());
                let onward = dx * WAYS[into].0 + dy * WAYS[into].1;
                let turns = match (way + 4 - into) % 4 {
                    0 if aside < 1e-3 && ahead >= -1e-3 => 0.0,
                    0 if ahead > -1e-3 => 2.0,
                    0 => 4.0,
                    2 => 2.0,
                    _ if ahead >= -1e-3 && onward >= -1e-3 => 1.0,
                    _ => 3.0,
                };
                f64::from(dx.abs() + dy.abs()) + turns * TURN
            })
            .fold(f64::MAX, f64::min)
    };
    // What it has cost to get to each place going each way, and from
    // where; and whether it has been there.
    let states = wide * high * 4;
    let (mut cost, mut came, mut done) = (vec![f64::MAX; states], vec![usize::MAX; states], vec![false; states]);
    let mut heap = BinaryHeap::new();
    let began = |start: End| node(out(start)) * 4 + start.1 % 4;
    for &start in starts {
        cost[began(start)] = start.2;
        heap.push(Step { guess: start.2 + guess(began(start)), key: began(start) });
    }
    let mut looked = 0;
    while let Some(Step { key, .. }) = heap.pop() {
        if done[key] {
            continue;
        }
        done[key] = true;
        looked += 1;
        if looked > MOST {
            return None;
        }
        let gone = cost[key];
        if let Some(goal) = ends.iter().position(|&(end, ..)| end == key) {
            // Back the way it came, to the side it began at.
            let mut way = Vec::new();
            let mut at = key;
            loop {
                way.push(place(at / 4));
                if came[at] == usize::MAX {
                    break;
                }
                at = came[at];
            }
            way.reverse();
            let start = starts.iter().position(|&start| began(start) == at)?;
            let mut path = vec![starts[start].0];
            path.extend(way);
            path.push(goals[goal].0);
            let mut path = simplified(path);
            centre(&mut path, blocks, stub);
            return Some((start, goal, simplified(path)));
        }
        let (here, way) = (key / 4, key % 4);
        let mut reach = |next: usize, more: f64, heap: &mut BinaryHeap<Step>| {
            let total = gone + more + ends.iter().find(|end| end.0 == next).map_or(0.0, |end| end.2);
            if !done[next] && total < cost[next] {
                (cost[next], came[next]) = (total, key);
                heap.push(Step { guess: total + guess(next), key: next });
            }
        };
        // Turned a right angle either way where it is.
        for turn in [1, 3] {
            reach(here * 4 + (way + turn) % 4, TURN, &mut heap);
        }
        // On along the way it goes, to the next line, if nothing is there.
        let (ix, iy) = (here / high, here % high);
        let next = match way {
            0 => iy.checked_sub(1).map(|iy| ix * high + iy),
            1 => (ix + 1 < wide).then(|| (ix + 1) * high + iy),
            2 => (iy + 1 < high).then(|| ix * high + iy + 1),
            _ => ix.checked_sub(1).map(|ix| ix * high + iy),
        };
        if let Some(next) = next {
            let (a, b) = (place(here), place(next));
            if clear(a, b, blocks) {
                reach(next * 4 + way, f64::from((b.0 - a.0).abs() + (b.1 - a.1).abs()), &mut heap);
            }
        }
    }
    None
}

/// A way with no point twice over and none in the middle of a straight
/// stretch: only its ends and where it turns.
pub fn simplified(path: Vec<Point>) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::with_capacity(path.len());
    for point in path {
        if kept.last().is_some_and(|last| (last.0 - point.0).abs() + (last.1 - point.1).abs() < 1e-3) {
            continue;
        }
        if let [.., before, last] = kept[..] {
            let level = (before.1 - last.1).abs() < 1e-3 && (last.1 - point.1).abs() < 1e-3;
            let upright = (before.0 - last.0).abs() < 1e-3 && (last.0 - point.0).abs() < 1e-3;
            if level || upright {
                kept.pop();
            }
        }
        kept.push(point);
    }
    kept
}

/// Each stretch between two turns that go on the same way, as the middle
/// of a step does, slid to half way along the room it has: between the
/// stretches either side of it, out of the sides' stubs, and clear of
/// what is near.
fn centre(path: &mut [Point], blocks: &[Block], stub: f32) {
    let count = path.len();
    if count < 4 {
        return;
    }
    for at in 1..count - 2 {
        let (before, a, b, after) = (path[at - 1], path[at], path[at + 1], path[at + 2]);
        let upright = (a.0 - b.0).abs() < 1e-3;
        // Along the axis it slides on: where it is, and where the two
        // stretches either side of it begin and end.
        let pick = |p: Point| if upright { p.0 } else { p.1 };
        let span = |p: Point| if upright { p.1 } else { p.0 };
        let (here, from, to) = (pick(a), pick(before), pick(after));
        let (back, on) = (here - from, to - here);
        if back * on <= 0.0 {
            continue;
        }
        let sign = back.signum();
        let first = if at == 1 { from + sign * stub } else { from };
        let last = if at + 2 == count - 1 { to - sign * stub } else { to };
        let (mut low, mut high) = (first.min(last).min(here), first.max(last).max(here));
        let (top, bottom) = (span(a).min(span(b)), span(a).max(span(b)));
        for r in blocks {
            let (near, far, s0, s1) = if upright { (r[0], r[2], r[1], r[3]) } else { (r[1], r[3], r[0], r[2]) };
            // Across what it spans, it keeps a stub away from a block,
            // unless it is nearer already; along the stretches either
            // side, they may not run into one.
            let mut blocked = Vec::new();
            if s1 + PAD > top && s0 - PAD < bottom {
                blocked.push(match here > near - stub && here < far + stub {
                    true => (near - PAD, far + PAD),
                    false => (near - stub, far + stub),
                });
            }
            for line in [span(a), span(b)] {
                if line > s0 - PAD && line < s1 + PAD {
                    blocked.push((near - PAD, far + PAD));
                }
            }
            for (u0, u1) in blocked {
                if u1 <= here + 1e-3 {
                    low = low.max(u1);
                } else if u0 >= here - 1e-3 {
                    high = high.min(u0);
                }
            }
        }
        if low < high {
            let middle = (low + high) / 2.0;
            for point in [at, at + 1] {
                match upright {
                    true => path[point].0 = middle,
                    false => path[point].1 = middle,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether every stretch of a way is level or upright.
    fn square(path: &[Point]) -> bool {
        path.windows(2).all(|pair| (pair[0].0 - pair[1].0).abs() < 1e-3 || (pair[0].1 - pair[1].1).abs() < 1e-3)
    }

    #[test]
    fn a_way_between_two_boxes_in_line_goes_straight_and_one_off_to_the_side_steps_half_way() {
        let blocks = [[0.0, 0.0, 100.0, 60.0], [300.0, 0.0, 400.0, 60.0]];
        let (_, _, path) = orthogonal(&[((100.0, 30.0), 1, 0.0)], &[((300.0, 30.0), 3, 0.0)], &blocks, 16.0).unwrap();
        assert_eq!(path, vec![(100.0, 30.0), (300.0, 30.0)]);
        let blocks = [[0.0, 0.0, 100.0, 60.0], [300.0, 100.0, 400.0, 160.0]];
        let (_, _, path) = orthogonal(&[((100.0, 30.0), 1, 0.0)], &[((300.0, 130.0), 3, 0.0)], &blocks, 16.0).unwrap();
        assert!(square(&path), "{path:?}");
        assert_eq!(path.len(), 4, "{path:?}");
        assert!((path[1].0 - 200.0).abs() < 0.5, "half way across: {path:?}");
    }
}
