//! Edges found in a picture, for its crop to snap to and to be made tight
//! by: lines across and down it where many pixels side by side change
//! sharply from those next to them, as the borders, dividers and edges of a
//! screenshot's windows, bars and cards do; and the part of it inside a
//! margin of one color.

use rayon::prelude::*;

/// How far a pixel is from the next, in any of its colors or how opaque it
/// is, out of 255, for there to be an edge between them.
const SHARP: i32 = 14;
/// How many pixels in a row a line can be without an edge and go on.
const GAP: u32 = 2;
/// How far a margin's pixels may be from its color, out of 255.
const MARGIN: i32 = 10;
/// How far in from a side, in pixels, a line has to be to be another one
/// for a tight crop to go on to: the two sides of a thin border are one.
const APART: f32 = 3.0;

/// A line between two rows or two columns of pixels.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Line {
    /// Where it is: before the row or column of pixels of this number.
    pub at: f32,
    /// How many of its pixels are in runs long enough to count.
    pub strength: f32,
    /// Where along it each of those runs is, from and to.
    pub runs: Vec<(f32, f32)>,
}

impl Line {
    /// Whether it goes past `along`, give or take `slack`.
    fn covers(&self, along: f32, slack: f32) -> bool {
        self.runs.iter().any(|&(from, to)| from - slack <= along && along <= to + slack)
    }

    /// How much of it is between `from` and `to`.
    fn within(&self, from: f32, to: f32) -> f32 {
        self.runs.iter().map(|&(a, b)| (b.min(to) - a.max(from)).max(0.0)).sum()
    }
}

/// A box of a picture's own pixels by its sides: left, top, right, bottom.
pub(super) type Sides = (f32, f32, f32, f32);

/// The lines found in a picture, up and down (at an x) and across (at a
/// y), each in order; and the part of it inside a margin of one color, or
/// all of it.
#[derive(Clone, Debug)]
pub(super) struct Lines {
    pub width: f32,
    pub height: f32,
    pub upright: Vec<Line>,
    pub across: Vec<Line>,
    pub content: Sides,
}

/// How unlike two pixels are: the most any of their parts differ by. Two
/// that cannot be seen are alike, whatever color they would be.
fn differ(a: u32, b: u32) -> i32 {
    if a >> 24 == 0 && b >> 24 == 0 {
        return 0;
    }
    (0..4).map(|byte| ((a >> (byte * 8)) as u8 as i32 - (b >> (byte * 8)) as u8 as i32).abs()).max().unwrap_or(0)
}

/// How long a run along a line has to be to count, for a picture this
/// long that way: long enough that text, which is edges everywhere but in
/// short runs, makes no lines.
fn shortest(long: i32) -> u32 {
    (long / 40).clamp(20, 64) as u32
}

/// The runs of edges along a line, as they are come to one by one.
#[derive(Clone, Copy, Default)]
struct Run {
    open: bool,
    from: u32,
    last: u32,
}

impl Run {
    /// The next pixel along: an edge or not. Gives a run that has ended.
    fn step(&mut self, at: u32, edge: bool, shortest: u32) -> Option<(u32, u32)> {
        let mut ended = None;
        if self.open && at > self.last + GAP + 1 {
            ended = self.end(shortest);
        }
        if edge {
            if !self.open {
                (self.open, self.from) = (true, at);
            }
            self.last = at;
        }
        ended
    }

    fn end(&mut self, shortest: u32) -> Option<(u32, u32)> {
        let run = (self.from, self.last + 1);
        self.open = false;
        (run.1 - run.0 >= shortest).then_some(run)
    }
}

/// A line from its runs, if it has any.
fn line(at: usize, runs: Vec<(u32, u32)>) -> Option<Line> {
    let strength = runs.iter().map(|&(from, to)| (to - from) as f32).sum::<f32>();
    (strength > 0.0).then(|| Line { at: at as f32, strength, runs: runs.into_iter().map(|(from, to)| (from as f32, to as f32)).collect() })
}

/// Of lines in order, those not beside one much stronger: an edge blurred
/// over two or three pixels is one line. Both sides of a thin border are
/// kept, as they are as strong as each other.
fn thinned(lines: Vec<Line>) -> Vec<Line> {
    (0..lines.len())
        .filter(|&index| {
            let this = &lines[index];
            !lines[index.saturating_sub(2)..(index + 3).min(lines.len())].iter().any(|other| (other.at - this.at).abs() <= 2.0 && other.strength > this.strength * 1.5)
        })
        .map(|index| lines[index].clone())
        .collect()
}

impl Lines {
    /// The lines of a picture, `width` by `height`, of these pixels.
    pub fn find(width: i32, height: i32, pixels: &[u32]) -> Lines {
        let (w, h) = (width.max(0) as usize, height.max(0) as usize);
        let empty = Lines { width: width as f32, height: height as f32, upright: Vec::new(), across: Vec::new(), content: (0.0, 0.0, width as f32, height as f32) };
        if w < 2 || h < 2 || pixels.len() < w * h {
            return empty;
        }
        // Across: each row against the one over it, a row to a thread.
        let long = shortest(width);
        let across: Vec<Line> = (1..h)
            .into_par_iter()
            .filter_map(|y| {
                let (over, row) = (&pixels[(y - 1) * w..y * w], &pixels[y * w..(y + 1) * w]);
                let mut run = Run::default();
                let mut runs = Vec::new();
                for x in 0..w {
                    runs.extend(run.step(x as u32, differ(over[x], row[x]) > SHARP, long));
                }
                if run.open {
                    runs.extend(run.end(long));
                }
                line(y, runs)
            })
            .collect();
        // Up and down: each column against the one left of it, the rows
        // gone down in order for a band of columns to a thread.
        let long = shortest(height);
        let bands = (rayon::current_num_threads() * 4).max(1);
        let band = w.div_ceil(bands).max(16);
        let upright: Vec<Line> = (0..w.div_ceil(band))
            .into_par_iter()
            .flat_map_iter(|part| {
                let (from, to) = ((part * band).max(1), ((part + 1) * band).min(w));
                let mut runs = vec![Run::default(); to.saturating_sub(from)];
                let mut found: Vec<Vec<(u32, u32)>> = vec![Vec::new(); runs.len()];
                for y in 0..h {
                    let row = &pixels[y * w..(y + 1) * w];
                    for x in from..to {
                        let edge = differ(row[x - 1], row[x]) > SHARP;
                        found[x - from].extend(runs[x - from].step(y as u32, edge, long));
                    }
                }
                for (index, run) in runs.iter_mut().enumerate() {
                    if run.open {
                        found[index].extend(run.end(long));
                    }
                }
                found.into_iter().enumerate().filter_map(move |(index, runs)| line(from + index, runs)).collect::<Vec<_>>()
            })
            .collect();
        Lines { upright: thinned(upright), across: thinned(across), content: content(w, h, pixels), ..empty }
    }

    /// The line nearest `at`, no further than `reach`, of these.
    pub fn nearest(lines: &[Line], at: f32, reach: f32) -> Option<f32> {
        lines.iter().map(|line| line.at).filter(|line| (line - at).abs() <= reach).min_by(|a, b| (a - at).abs().total_cmp(&(b - at).abs()))
    }

    /// The crops a tight crop goes through, from the outside in: first
    /// the part inside a margin of one color; then the largest box of
    /// lines inside the last, as panels and cards are in each other; or
    /// where there is none, each side in to the nearest strong line.
    pub fn tight(&self) -> Vec<Sides> {
        let whole = (0.0, 0.0, self.width, self.height);
        let (l, t, r, b) = self.content;
        let margin = l >= 2.0 || t >= 2.0 || self.width - r >= 2.0 || self.height - b >= 2.0;
        let mut crops = Vec::new();
        let mut now = if margin { self.content } else { whole };
        if margin {
            crops.push(now);
        }
        while crops.len() < 8 {
            // From the whole picture, a line a pixel in is one to go to;
            // from a crop on a line, it is the other side of a border.
            let apart = if now == whole { 0.5 } else { APART };
            let moved = |(a, b, c, d): Sides| (a - now.0).max(b - now.1).max(now.2 - c).max(now.3 - d) > apart;
            let largest = self.boxes(now).into_iter().filter(|&found| moved(found)).max_by(|a, b| ((a.2 - a.0) * (a.3 - a.1)).total_cmp(&((b.2 - b.0) * (b.3 - b.1))));
            let Some(next) = largest.or_else(|| self.inward(now, apart)) else { break };
            crops.push(next);
            now = next;
        }
        crops
    }

    /// The boxes inside `sides` whose four sides are lines, each running
    /// along most of its side: the windows, panels and cards of a
    /// screenshot. The strongest lines are tried, not every one.
    pub fn boxes(&self, (l, t, r, b): Sides) -> Vec<Sides> {
        fn strongest(lines: &[Line], from: f32, to: f32) -> Vec<&Line> {
            let mut inside: Vec<&Line> = lines.iter().filter(|line| line.at >= from && line.at <= to).collect();
            inside.sort_by(|a, b| b.strength.total_cmp(&a.strength));
            inside.truncate(150);
            inside.sort_by(|a, b| a.at.total_cmp(&b.at));
            inside
        }
        let (upright, across) = (strongest(&self.upright, l, r), strongest(&self.across, t, b));
        // Most of a side and near both its ends, so that rounded corners
        // leave enough, but a line stopping well short is another box's.
        let along = |line: &Line, from: f32, to: f32| {
            let inset = 16.0f32.min((to - from) * 0.15);
            line.within(from, to) >= (to - from) * 0.7 && line.covers(from + inset, 1.0) && line.covers(to - inset, 1.0)
        };
        let mut found = Vec::new();
        for (index, left) in upright.iter().enumerate() {
            for right in upright[index + 1..].iter().filter(|right| right.at - left.at >= 16.0) {
                let edges: Vec<&&Line> = across.iter().filter(|line| along(line, left.at, right.at)).collect();
                for (index, top) in edges.iter().enumerate() {
                    for bottom in edges[index + 1..].iter().filter(|bottom| bottom.at - top.at >= 16.0) {
                        if along(left, top.at, bottom.at) && along(right, top.at, bottom.at) {
                            found.push((left.at, top.at, right.at, bottom.at));
                        }
                    }
                }
            }
        }
        found
    }

    /// The box inside `sides`, each side moved in to the first line past
    /// it, by more than `apart`, that runs along a quarter of it or more:
    /// none if none is.
    fn inward(&self, (l, t, r, b): Sides, apart: f32) -> Option<Sides> {
        let along = |lines: &[Line], from: f32, to: f32, side: f32, other: f32, forward: bool| -> f32 {
            let strong = |line: &&Line| line.within(from, to) >= (to - from) * 0.25;
            let found = match forward {
                true => lines.iter().filter(|line| line.at > side + apart && line.at < other - 16.0).find(strong),
                false => lines.iter().rev().filter(|line| line.at < side - apart && line.at > other + 16.0).find(strong),
            };
            found.map_or(side, |line| line.at)
        };
        let left = along(&self.upright, t, b, l, r, true);
        let right = along(&self.upright, t, b, r, left, false);
        let top = along(&self.across, l, r, t, b, true);
        let bottom = along(&self.across, l, r, b, top, false);
        let next = (left, top, right, bottom);
        (next != (l, t, r, b) && right - left >= 16.0 && bottom - top >= 16.0).then_some(next)
    }

    /// The boxes of lines round a point of the picture, the smallest
    /// first; or with none round it, the box of the nearest line on each
    /// side that runs past it, or the part inside the margin.
    pub fn around(&self, (x, y): (f32, f32)) -> Vec<Sides> {
        let mut boxes: Vec<Sides> = self.boxes((0.0, 0.0, self.width, self.height)).into_iter().filter(|(l, t, r, b)| *l < x && x < *r && *t < y && y < *b).collect();
        boxes.sort_by(|a, b| ((b.2 - b.0) * (b.3 - b.1)).total_cmp(&((a.2 - a.0) * (a.3 - a.1))));
        // A box and the one just inside its border are the same box, kept
        // with its border.
        boxes.dedup_by(|inner, outer| (inner.0 - outer.0).abs().max((inner.1 - outer.1).abs()).max((inner.2 - outer.2).abs()).max((inner.3 - outer.3).abs()) <= APART);
        boxes.reverse();
        if boxes.is_empty() {
            boxes.push(self.nearest_round((x, y)));
        }
        boxes
    }

    /// On each side of a point, the nearest line that runs past it, or
    /// the part inside the margin; in the margin, the edge of the picture.
    fn nearest_round(&self, (x, y): (f32, f32)) -> Sides {
        let slack = 12.0;
        let (l, t, r, b) = self.content;
        let (l, t, r, b) = if l <= x && x <= r && t <= y && y <= b { (l, t, r, b) } else { (0.0, 0.0, self.width, self.height) };
        let left = self.upright.iter().rev().find(|line| line.at < x && line.at > l && line.covers(y, slack)).map_or(l, |line| line.at);
        let right = self.upright.iter().find(|line| line.at > x && line.at < r && line.covers(y, slack)).map_or(r, |line| line.at);
        let top = self.across.iter().rev().find(|line| line.at < y && line.at > t && line.covers(x, slack)).map_or(t, |line| line.at);
        let bottom = self.across.iter().find(|line| line.at > y && line.at < b && line.covers(x, slack)).map_or(b, |line| line.at);
        (left, top, right, bottom)
    }
}

/// The part of a picture inside a margin of one color, when its four
/// corners are all of it, as the trim of an image editor does; else all
/// of it. A pixel in two hundred may be off, for the noise of a JPEG.
fn content(w: usize, h: usize, pixels: &[u32]) -> Sides {
    let at = |x: usize, y: usize| pixels[y * w + x];
    let ground = at(0, 0);
    let whole = (0.0, 0.0, w as f32, h as f32);
    if [at(w - 1, 0), at(0, h - 1), at(w - 1, h - 1)].iter().any(|&corner| differ(corner, ground) > MARGIN) {
        return whole;
    }
    let plain = |count: usize, pixel: &dyn Fn(usize) -> u32| (0..count).filter(|&i| differ(pixel(i), ground) > MARGIN).count() <= count / 200;
    let (mut l, mut t, mut r, mut b) = (0, 0, w, h);
    while t < b && plain(r - l, &|i| at(l + i, t)) {
        t += 1;
    }
    if t == b {
        return whole;
    }
    while b > t + 1 && plain(r - l, &|i| at(l + i, b - 1)) {
        b -= 1;
    }
    while l < r && plain(b - t, &|i| at(l, t + i)) {
        l += 1;
    }
    while r > l + 1 && plain(b - t, &|i| at(r - 1, t + i)) {
        r -= 1;
    }
    (l as f32, t as f32, r as f32, b as f32)
}

/// How tall the title bar a window manager drew over a window is, in a
/// picture of the window taken with it, when nothing said so as it was
/// taken: the first line from near its top that goes across all of it,
/// with a band over it mostly of one color, the bar's own, the title and
/// the buttons on it being the rest. None in a picture with no such bar.
pub(super) fn title_bar(width: i32, height: i32, pixels: &[u32]) -> Option<i32> {
    let (w, h) = (width.max(0) as usize, height.max(0) as usize);
    if w < 64 || h < 48 || pixels.len() < w * h {
        return None;
    }
    let lowest = (h / 3).min(90);
    for y in 12..=lowest {
        let (over, row) = (&pixels[(y - 1) * w..y * w], &pixels[y * w..(y + 1) * w]);
        let sharp = over.iter().zip(row).filter(|(a, b)| differ(**a, **b) > SHARP).count();
        if sharp * 10 < w * 9 {
            continue;
        }
        // The band over it: the color most of it is, and how much of it
        // that is, and that its edge is not merely a line inside a page.
        let band = &pixels[w..(y - 1) * w];
        let ground = pixels[w + w / 2];
        let same = band.iter().step_by(3).filter(|&&pixel| differ(pixel, ground) <= MARGIN).count();
        if same * 3 * 10 >= band.len() * 6 {
            return Some(y as i32);
        }
        return None;
    }
    None
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[test]
    fn a_title_bar_drawn_over_a_window_is_found_and_none_where_there_is_none() {
        // A dark gray bar 24 high with a light title in it and buttons, over
        // a white window with a card in it.
        let (w, h) = (400, 300);
        let mut pixels = card_picture(w, h, (40, 60, 360, 260), (60, 80, 200, 140));
        for y in 0..24 {
            for x in 0..w {
                let title = (150..230).contains(&x) && (8..16).contains(&y) && x % 3 == 0;
                let button = x > w - 60 && (6..18).contains(&y) && x % 20 < 10;
                pixels[(y * w + x) as usize] = if title || button { 0xFFD4_D4D4 } else { 0xFF3C_3F41 };
            }
        }
        assert_eq!(title_bar(w, h, &pixels), Some(24));
        let plain = card_picture(w, h, (40, 60, 360, 260), (60, 80, 200, 140));
        assert_eq!(title_bar(w, h, &plain), None);
    }

    /// A picture `w` by `h` of a light gray, with a white card in it from
    /// `card`, edged in a darker gray a pixel wide, and in the card a
    /// panel of blue.
    pub(in super::super) fn card_picture(w: i32, h: i32, card: (i32, i32, i32, i32), panel: (i32, i32, i32, i32)) -> Vec<u32> {
        let mut pixels = vec![0xFFF0_F0F0u32; (w * h) as usize];
        let inside = |(x, y): (i32, i32), (l, t, r, b): (i32, i32, i32, i32)| x >= l && x < r && y >= t && y < b;
        for y in 0..h {
            for x in 0..w {
                let pixel = &mut pixels[(y * w + x) as usize];
                if inside((x, y), card) {
                    *pixel = if inside((x, y), (card.0 + 1, card.1 + 1, card.2 - 1, card.3 - 1)) { 0xFFFF_FFFF } else { 0xFF90_9090 };
                }
                if inside((x, y), panel) {
                    *pixel = 0xFF33_66CC;
                }
            }
        }
        pixels
    }

    #[test]
    fn lines_are_found_at_the_edges_of_a_card_and_a_panel_in_it() {
        let pixels = card_picture(400, 300, (50, 40, 350, 260), (100, 80, 300, 200));
        let lines = Lines::find(400, 300, &pixels);
        let xs: Vec<f32> = lines.upright.iter().map(|line| line.at).collect();
        let ys: Vec<f32> = lines.across.iter().map(|line| line.at).collect();
        assert_eq!(xs, [50.0, 51.0, 100.0, 300.0, 349.0, 350.0]);
        assert_eq!(ys, [40.0, 41.0, 80.0, 200.0, 259.0, 260.0]);
        assert_eq!(lines.content, (50.0, 40.0, 350.0, 260.0), "the margin is trimmed to the card");
        let card = &lines.upright[0];
        assert_eq!(card.runs, [(40.0, 260.0)]);
    }

    #[test]
    fn text_and_noise_make_no_lines() {
        // Short dark strokes, as letters are, and a little noise.
        let (w, h) = (300, 200);
        let mut pixels = vec![0xFFFF_FFFFu32; (w * h) as usize];
        for y in (20..180).step_by(12) {
            for x in (10..290).step_by(9) {
                for dy in 0..7 {
                    for dx in 0..((x + y) % 5 + 2) {
                        pixels[((y + dy) * w + x + dx) as usize] = 0xFF20_2020;
                    }
                }
            }
        }
        for (i, pixel) in pixels.iter_mut().enumerate() {
            if *pixel == 0xFFFF_FFFF && i % 7 == 0 {
                *pixel = 0xFFFA_FAFA;
            }
        }
        let lines = Lines::find(w, h, &pixels);
        assert!(lines.upright.is_empty() && lines.across.is_empty(), "{:?} {:?}", lines.upright.len(), lines.across.len());
    }

    #[test]
    fn a_tight_crop_trims_the_margin_then_goes_in_to_the_panel() {
        let pixels = card_picture(400, 300, (50, 40, 350, 260), (100, 80, 300, 200));
        let lines = Lines::find(400, 300, &pixels);
        assert_eq!(lines.tight(), [(50.0, 40.0, 350.0, 260.0), (100.0, 80.0, 300.0, 200.0)]);
    }

    #[test]
    fn without_a_margin_a_tight_crop_goes_in_to_the_nearest_lines() {
        // The card filling all but a band of another color down the left.
        let mut pixels = card_picture(400, 300, (0, 0, 400, 300), (100, 80, 300, 200));
        for y in 0..300 {
            for x in 0..30 {
                pixels[y * 400 + x] = 0xFF20_2020;
            }
        }
        let lines = Lines::find(400, 300, &pixels);
        assert_eq!(lines.content, (0.0, 0.0, 400.0, 300.0));
        let tight = lines.tight();
        assert_eq!(tight[0], (30.0, 1.0, 399.0, 299.0), "{tight:?}");
        assert_eq!(tight[1], (100.0, 80.0, 300.0, 200.0));
    }

    #[test]
    fn a_screenshot_as_large_as_a_4k_screen_has_its_lines_found_in_good_time() {
        let (w, h) = (3840, 2160);
        let mut pixels = card_picture(w, h, (200, 150, 3640, 2010), (400, 300, 1800, 1800));
        // Specks over it, as text and icons are.
        for (i, pixel) in pixels.iter_mut().enumerate() {
            if (i * 2_654_435_761) % 97 == 0 {
                *pixel = 0xFF20_2020;
            }
        }
        let began = std::time::Instant::now();
        let lines = Lines::find(w, h, &pixels);
        println!("4K: {} lines down, {} across, in {:.1?}", lines.upright.len(), lines.across.len(), began.elapsed());
        assert!(lines.upright.iter().any(|line| line.at == 400.0) && lines.across.iter().any(|line| line.at == 2010.0));
    }

    #[test]
    fn the_box_round_a_point_is_the_smallest_of_lines() {
        let pixels = card_picture(400, 300, (50, 40, 350, 260), (100, 80, 300, 200));
        let lines = Lines::find(400, 300, &pixels);
        assert_eq!(lines.around((150.0, 150.0)), [(100.0, 80.0, 300.0, 200.0), (50.0, 40.0, 350.0, 260.0)], "the panel, then the card with its border");
        assert_eq!(lines.around((70.0, 60.0)), [(50.0, 40.0, 350.0, 260.0)], "in the card, round the panel");
        assert_eq!(lines.around((20.0, 20.0)), [(0.0, 0.0, 400.0, 300.0)], "in the margin, round nothing");
        assert_eq!(Lines::nearest(&lines.upright, 104.0, 6.0), Some(100.0));
        assert_eq!(Lines::nearest(&lines.upright, 110.0, 6.0), None);
    }
}
