//! A graph laid out in layers, as Mermaid's flowcharts are: each box given
//! a rank, so that its arrows go one way, down or across; a box held in
//! each rank for every arrow that goes past it and for the words on one,
//! so that nothing is drawn through; the boxes of each rank put in the
//! order that crosses the fewest arrows, each moved towards the middle of
//! those it is joined to; and the ranks laid one after another with room
//! between them. Sugiyama's way, worked here with no script engine.

/// A point, across and down.
pub(in crate::studio) type Point = (f32, f32);

/// Which way a graph goes: down, up, right or left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) enum Dir {
    Down,
    Up,
    Right,
    Left,
}

impl Dir {
    /// The way Mermaid names it: TD, TB, BT, LR or RL.
    pub(in crate::studio) fn named(word: &str) -> Option<Dir> {
        match word.trim().to_uppercase().as_str() {
            "TD" | "TB" => Some(Dir::Down),
            "BT" => Some(Dir::Up),
            "LR" => Some(Dir::Right),
            "RL" => Some(Dir::Left),
            _ => None,
        }
    }

    /// Whether its ranks are one under another.
    pub(in crate::studio) fn vertical(self) -> bool {
        matches!(self, Dir::Down | Dir::Up)
    }
}

/// An arrow of a graph: from one box to another, by where each is in the
/// list, the size of its words, none for none, and how many ranks long
/// it is at least, as Mermaid's longer arrows are.
#[derive(Clone, Debug)]
pub(in crate::studio) struct Link {
    pub from: usize,
    pub to: usize,
    pub label: (f32, f32),
    pub length: usize,
}

/// A graph laid out: the middle of each box, the points each arrow goes
/// by between its ends and where its words go, and how large it all is,
/// from its top left at nought.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Laid {
    pub centers: Vec<Point>,
    pub ways: Vec<Vec<Point>>,
    pub labels: Vec<Option<Point>>,
    pub size: (f32, f32),
}

/// The room between boxes side by side in a rank, and between ranks.
#[derive(Clone, Copy, Debug)]
pub(in crate::studio) struct Gaps {
    pub node: f32,
    pub rank: f32,
}

/// What is laid out in the ranks: a box, or a point an arrow goes by,
/// which is as large as its words where it carries them.
#[derive(Clone, Debug)]
struct Vertex {
    across: f32,
    along: f32,
    real: bool,
    up: Vec<usize>,
    down: Vec<usize>,
}

/// Boxes of these sizes, joined by these arrows, laid out going this way.
pub(in crate::studio) fn lay_out(sizes: &[Point], links: &[Link], dir: Dir, gaps: Gaps) -> Laid {
    let count = sizes.len();
    if count == 0 {
        return Laid { ways: vec![Vec::new(); links.len()], labels: vec![None; links.len()], ..Laid::default() };
    }
    let vertical = dir.vertical();
    let turn = |(w, h): Point| if vertical { (w, h) } else { (h, w) };
    // Arrows that go round to the box they came from are drawn after.
    let edges: Vec<(usize, usize, usize)> = links.iter().enumerate().filter(|(_, link)| link.from != link.to && link.from < count && link.to < count).map(|(at, link)| (link.from, link.to, at)).collect();
    // Each cycle broken by turning round the arrows that close it.
    let reversed = back_edges(count, &edges);
    let directed: Vec<(usize, usize, usize, bool)> = edges.iter().zip(&reversed).map(|(&(from, to, at), &back)| if back { (to, from, at, true) } else { (from, to, at, false) }).collect();
    let rank = ranks(count, &directed, links);
    // The vertices: the boxes, then a point for each rank an arrow
    // crosses, one of them carrying its words.
    let mut vertices: Vec<Vertex> = sizes.iter().map(|&size| {
        let (across, along) = turn(size);
        Vertex { across, along, real: true, up: Vec::new(), down: Vec::new() }
    }).collect();
    let mut ranked: Vec<usize> = rank.clone();
    let mut chains: Vec<(usize, Vec<usize>, Option<usize>, bool)> = Vec::new();
    for &(from, to, at, back) in &directed {
        let span = rank[to] - rank[from];
        let words = links[at].label;
        let wordy = words.0 > 0.0 && words.1 > 0.0;
        let middle = rank[from] + span / 2;
        let mut chain = Vec::new();
        let mut label = None;
        let mut before = from;
        for level in rank[from] + 1..rank[to] {
            let carries = wordy && level == middle;
            let (across, along) = if carries { turn(words) } else { (0.0, 0.0) };
            vertices.push(Vertex { across, along, real: false, up: vec![before], down: Vec::new() });
            let made = vertices.len() - 1;
            vertices[before].down.push(made);
            ranked.push(level);
            if carries {
                label = Some(made);
            }
            chain.push(made);
            before = made;
        }
        vertices[before].down.push(to);
        vertices[to].up.push(before);
        chains.push((at, chain, label, back));
    }
    let deepest = ranked.iter().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); deepest + 1];
    // First in the order they are reached from the top, each box's
    // children after it, as they were written.
    let mut seen = vec![false; vertices.len()];
    let mut starts: Vec<usize> = (0..count).filter(|&v| vertices[v].up.is_empty()).collect();
    starts.extend(0..count);
    for start in starts {
        let mut stack = vec![start];
        while let Some(v) = stack.pop() {
            if seen[v] {
                continue;
            }
            seen[v] = true;
            layers[ranked[v]].push(v);
            for &next in vertices[v].down.iter().rev() {
                if !seen[next] {
                    stack.push(next);
                }
            }
        }
    }
    order(&mut layers, &vertices);
    let across = place_across(&layers, &vertices, gaps);
    // The ranks one after another, each as deep as its deepest, with the
    // room between half on each side of the ranks that hold only words.
    let mut tops = vec![0.0f32; layers.len()];
    let mut thick = vec![0.0f32; layers.len()];
    let mut down = 0.0;
    for (level, layer) in layers.iter().enumerate() {
        thick[level] = layer.iter().map(|&v| vertices[v].along).fold(0.0, f32::max);
        tops[level] = down;
        down += thick[level] + gaps.rank / 2.0;
    }
    let along_size = (down - gaps.rank / 2.0).max(0.0);
    let across_size = layers.iter().flatten().map(|&v| across[v] + vertices[v].across / 2.0).fold(0.0, f32::max);
    let along_of = |v: usize| tops[ranked[v]] + thick[ranked[v]] / 2.0;
    let place = |v: usize| -> Point {
        let (a, b) = (across[v], along_of(v));
        match dir {
            Dir::Down => (a, b),
            Dir::Up => (a, along_size - b),
            Dir::Right => (b, a),
            Dir::Left => (along_size - b, a),
        }
    };
    let mut laid = Laid {
        centers: (0..count).map(place).collect(),
        ways: vec![Vec::new(); links.len()],
        labels: vec![None; links.len()],
        size: if vertical { (across_size, along_size) } else { (along_size, across_size) },
    };
    for (at, chain, label, back) in chains {
        let mut way: Vec<Point> = chain.iter().map(|&v| place(v)).collect();
        if back {
            way.reverse();
        }
        laid.ways[at] = way;
        laid.labels[at] = label.map(place);
    }
    // An arrow from a box to itself: a loop out of its far side and back.
    for (at, link) in links.iter().enumerate().filter(|(_, link)| link.from == link.to && link.from < count) {
        let (cx, cy) = laid.centers[link.from];
        let (w, h) = sizes[link.from];
        let out = 28.0;
        laid.ways[at] = match vertical {
            true => vec![(cx + w / 2.0 + out, cy - h * 0.3), (cx + w / 2.0 + out, cy + h * 0.3)],
            false => vec![(cx - w * 0.3, cy + h / 2.0 + out), (cx + w * 0.3, cy + h / 2.0 + out)],
        };
        laid.labels[at] = (links[at].label.0 > 0.0).then(|| match vertical {
            true => (cx + w / 2.0 + out + links[at].label.0 / 2.0 + 4.0, cy),
            false => (cx, cy + h / 2.0 + out + links[at].label.1 / 2.0 + 4.0),
        });
        laid.size.0 = laid.size.0.max(cx + w / 2.0 + out + links[at].label.0 + 8.0);
        laid.size.1 = laid.size.1.max(cy + h / 2.0 + out + links[at].label.1 + 8.0);
    }
    laid
}

/// The arrows that close a cycle, found going down from each box in turn:
/// those that go back to a box still being gone down from.
fn back_edges(count: usize, edges: &[(usize, usize, usize)]) -> Vec<bool> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (at, &(from, ..)) in edges.iter().enumerate() {
        out[from].push(at);
    }
    // 0 not yet reached, 1 being gone down from, 2 done.
    let mut state = vec![0u8; count];
    let mut back = vec![false; edges.len()];
    let mut has_in = vec![false; count];
    for &(_, to, _) in edges {
        has_in[to] = true;
    }
    let mut starts: Vec<usize> = (0..count).filter(|&v| !has_in[v]).collect();
    starts.extend(0..count);
    for start in starts {
        if state[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        state[start] = 1;
        while let Some(&mut (v, ref mut next)) = stack.last_mut() {
            if *next < out[v].len() {
                let edge = out[v][*next];
                *next += 1;
                let to = edges[edge].1;
                match state[to] {
                    0 => {
                        state[to] = 1;
                        stack.push((to, 0));
                    }
                    1 => back[edge] = true,
                    _ => {}
                }
            } else {
                state[v] = 2;
                stack.pop();
            }
        }
    }
    back
}

/// Each box's rank: as far down as the longest way to it, each arrow two
/// ranks a step so that its words have one between; then those with
/// nothing coming in brought down to just over what they go to.
fn ranks(count: usize, directed: &[(usize, usize, usize, bool)], links: &[Link]) -> Vec<usize> {
    let length = |at: usize| 2 * links[at].length.max(1);
    let mut incoming = vec![0usize; count];
    for &(_, to, ..) in directed {
        incoming[to] += 1;
    }
    let mut ready: Vec<usize> = (0..count).filter(|&v| incoming[v] == 0).collect();
    ready.reverse();
    let mut topo = Vec::new();
    let mut left = incoming.clone();
    while let Some(v) = ready.pop() {
        topo.push(v);
        for &(from, to, ..) in directed {
            if from == v {
                left[to] -= 1;
                if left[to] == 0 {
                    ready.push(to);
                }
            }
        }
    }
    let mut rank = vec![0usize; count];
    for &v in &topo {
        for &(from, to, at, _) in directed {
            if from == v {
                rank[to] = rank[to].max(rank[v] + length(at));
            }
        }
    }
    for _ in 0..3 {
        for &v in topo.iter().rev() {
            if incoming[v] > 0 {
                continue;
            }
            let lowest = directed.iter().filter(|edge| edge.0 == v).map(|&(_, to, at, _)| rank[to].saturating_sub(length(at))).min();
            if let Some(lowest) = lowest {
                rank[v] = rank[v].max(lowest);
            }
        }
    }
    // Every step even, so the ranks between hold only words and arrows.
    let least = rank.iter().copied().min().unwrap_or(0);
    rank.iter().map(|&r| r - least).collect()
}

/// How many arrows cross between each rank and the next.
fn crossings(layers: &[Vec<usize>], vertices: &[Vertex], place: &[usize]) -> usize {
    let mut total = 0;
    for pair in layers.windows(2) {
        let mut ends: Vec<(usize, usize)> = Vec::new();
        for &v in &pair[0] {
            for &w in &vertices[v].down {
                ends.push((place[v], place[w]));
            }
        }
        for (i, a) in ends.iter().enumerate() {
            for b in &ends[i + 1..] {
                if (a.0 < b.0 && a.1 > b.1) || (a.0 > b.0 && a.1 < b.1) {
                    total += 1;
                }
            }
        }
    }
    total
}

/// The boxes of each rank put in the order that crosses fewest arrows:
/// each by the middle of those it is joined to in the rank before, going
/// down and then up, and neighbors swapped where that crosses fewer; the
/// best order found kept.
fn order(layers: &mut [Vec<usize>], vertices: &[Vertex]) {
    let mut place = vec![0usize; vertices.len()];
    let index = |layers: &[Vec<usize>], place: &mut Vec<usize>| {
        for layer in layers {
            for (at, &v) in layer.iter().enumerate() {
                place[v] = at;
            }
        }
    };
    index(layers, &mut place);
    let mut best = layers.to_vec();
    let mut fewest = crossings(layers, vertices, &place);
    for round in 0..16 {
        if fewest == 0 {
            break;
        }
        let downward = round % 2 == 0;
        let levels: Vec<usize> = if downward { (1..layers.len()).collect() } else { (0..layers.len().saturating_sub(1)).rev().collect() };
        for level in levels {
            let mut keyed: Vec<(f32, usize)> = layers[level].iter().map(|&v| {
                let near = if downward { &vertices[v].up } else { &vertices[v].down };
                let key = match near.is_empty() {
                    true => place[v] as f32,
                    false => near.iter().map(|&w| place[w] as f32).sum::<f32>() / near.len() as f32,
                };
                (key, v)
            }).collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            layers[level] = keyed.into_iter().map(|(_, v)| v).collect();
            for (at, &v) in layers[level].iter().enumerate() {
                place[v] = at;
            }
        }
        transpose(layers, vertices, &mut place);
        let now = crossings(layers, vertices, &place);
        if now < fewest {
            (fewest, best) = (now, layers.to_vec());
        }
    }
    layers.clone_from_slice(&best);
}

/// Neighbors in a rank swapped wherever that crosses fewer arrows, until
/// no swap does.
fn transpose(layers: &mut [Vec<usize>], vertices: &[Vertex], place: &mut [usize]) {
    // How many of the arrows of `v` cross those of `w`, `v` before `w`.
    let crossed = |v: usize, w: usize, place: &[usize]| -> usize {
        let count = |mine: &[usize], theirs: &[usize]| mine.iter().map(|&a| theirs.iter().filter(|&&b| place[a] > place[b]).count()).sum::<usize>();
        count(&vertices[v].up, &vertices[w].up) + count(&vertices[v].down, &vertices[w].down)
    };
    for _ in 0..4 {
        let mut better = false;
        for layer in layers.iter_mut() {
            for at in 0..layer.len().saturating_sub(1) {
                let (v, w) = (layer[at], layer[at + 1]);
                if crossed(w, v, place) < crossed(v, w, place) {
                    layer.swap(at, at + 1);
                    (place[v], place[w]) = (at + 1, at);
                    better = true;
                }
            }
        }
        if !better {
            break;
        }
    }
}

/// Where each vertex is across: in its rank's order, kept apart by half of
/// each and the gap between, and as near as that allows to the middle of
/// those it is joined to, again and again until it settles; points an
/// arrow goes by pulled hardest, to keep long arrows straight.
fn place_across(layers: &[Vec<usize>], vertices: &[Vertex], gaps: Gaps) -> Vec<f32> {
    let mut across = vec![0.0f32; vertices.len()];
    let apart = |a: usize, b: usize| {
        let gap = if vertices[a].real && vertices[b].real { gaps.node } else { gaps.node * 0.5 };
        (vertices[a].across + vertices[b].across) / 2.0 + gap
    };
    for layer in layers {
        let mut at = 0.0;
        for (i, &v) in layer.iter().enumerate() {
            if i > 0 {
                at += apart(layer[i - 1], v);
            }
            across[v] = at;
        }
    }
    let weight = |a: usize, b: usize| match (vertices[a].real, vertices[b].real) {
        (false, false) => 8.0,
        (true, true) => 1.0,
        _ => 2.0,
    };
    for round in 0..40 {
        let levels: Vec<usize> = if round % 2 == 0 { (0..layers.len()).collect() } else { (0..layers.len()).rev().collect() };
        for level in levels {
            let layer = &layers[level];
            if layer.is_empty() {
                continue;
            }
            let wanted: Vec<f32> = layer.iter().map(|&v| {
                let (mut sum, mut total) = (0.0, 0.0);
                for &w in vertices[v].up.iter().chain(&vertices[v].down) {
                    let weight = weight(v, w);
                    sum += across[w] * weight;
                    total += weight;
                }
                if total > 0.0 { sum / total } else { across[v] }
            }).collect();
            // Kept apart: as near the wanted places as can be with each at
            // least its gap after the one before.
            let mut offsets = vec![0.0f32; layer.len()];
            for i in 1..layer.len() {
                offsets[i] = offsets[i - 1] + apart(layer[i - 1], layer[i]);
            }
            let shifted: Vec<f32> = wanted.iter().zip(&offsets).map(|(want, offset)| want - offset).collect();
            for (i, value) in rising(&shifted).into_iter().enumerate() {
                across[layer[i]] = value + offsets[i];
            }
        }
    }
    let least = layers.iter().flatten().map(|&v| across[v] - vertices[v].across / 2.0).fold(f32::MAX, f32::min);
    if least.is_finite() {
        for value in &mut across {
            *value -= least;
        }
    }
    across
}

/// The values nearest to these that never go down, each run that would
/// go down made its mean: adjacent pools pooled.
fn rising(values: &[f32]) -> Vec<f32> {
    let mut pools: Vec<(f32, usize)> = Vec::new();
    for &value in values {
        pools.push((value, 1));
        while pools.len() >= 2 {
            let (b, a) = (pools[pools.len() - 1], pools[pools.len() - 2]);
            if a.0 / a.1 as f32 <= b.0 / b.1 as f32 {
                break;
            }
            pools.pop();
            let last = pools.len() - 1;
            pools[last] = (a.0 + b.0, a.1 + b.1);
        }
    }
    pools.into_iter().flat_map(|(sum, count)| std::iter::repeat_n(sum / count as f32, count)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(from: usize, to: usize) -> Link {
        Link { from, to, label: (0.0, 0.0), length: 1 }
    }

    fn apart(laid: &Laid, sizes: &[Point]) -> bool {
        for i in 0..sizes.len() {
            for j in i + 1..sizes.len() {
                let (a, b) = (laid.centers[i], laid.centers[j]);
                let clear_x = (a.0 - b.0).abs() >= (sizes[i].0 + sizes[j].0) / 2.0;
                let clear_y = (a.1 - b.1).abs() >= (sizes[i].1 + sizes[j].1) / 2.0;
                if !clear_x && !clear_y {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn a_graph_goes_down_its_ranks_its_boxes_apart_and_crossing_little() {
        let sizes = vec![(100.0, 50.0); 6];
        // A diamond, and a tail that crosses unless it is put right.
        let links = vec![link(0, 1), link(0, 2), link(1, 3), link(2, 3), link(3, 4), link(0, 5), link(5, 4)];
        let gaps = Gaps { node: 40.0, rank: 60.0 };
        let laid = lay_out(&sizes, &links, Dir::Down, gaps);
        assert!(apart(&laid, &sizes));
        let y = |v: usize| laid.centers[v].1;
        assert!(y(0) < y(1) && (y(1) - y(2)).abs() < 0.1 && y(1) < y(3) && y(3) < y(4));
        // 0 to 5 to 4 goes past rank 3: through a point held for it.
        assert!(!laid.ways[6].is_empty() || !laid.ways[5].is_empty());
        // The same, going right: x in place of y.
        let right = lay_out(&sizes, &links, Dir::Right, gaps);
        assert!(right.centers[0].0 < right.centers[1].0 && apart(&right, &sizes));
        // Up: the first at the bottom.
        let up = lay_out(&sizes, &links, Dir::Up, gaps);
        assert!(up.centers[0].1 > up.centers[4].1);
    }

    #[test]
    fn a_cycle_is_laid_out_and_words_get_room_of_their_own() {
        let sizes = vec![(80.0, 40.0); 3];
        let mut links = vec![link(0, 1), link(1, 2), link(2, 0)];
        links[0].label = (120.0, 24.0);
        let laid = lay_out(&sizes, &links, Dir::Down, Gaps { node: 40.0, rank: 60.0 });
        assert!(apart(&laid, &sizes));
        let words = laid.labels[0].unwrap();
        // Between the two it joins, clear of both.
        assert!(words.1 > laid.centers[0].1 + 20.0 + 12.0 - 0.1 && words.1 < laid.centers[1].1 - 20.0 - 12.0 + 0.1, "{words:?} {:?}", laid.centers);
        assert!(laid.size.0 > 0.0 && laid.size.1 > 0.0);
    }

    #[test]
    fn values_kept_rising_are_pooled() {
        assert_eq!(rising(&[1.0, 3.0, 2.0, 4.0]), vec![1.0, 2.5, 2.5, 4.0]);
    }
}
