//! What a board draws a box as besides a box: a diamond, a database, a
//! person and a cloud, each by its outline as lines a hand could draw;
//! and the words inside a box or along an arrow, wrapped to it, with the
//! dots a box can carry and the rough pieces of a screen it can stand for.

use super::*;

/// A board's paper: a light, warm white, which an arrow's words sit on.
pub const PAPER: u32 = 0xF8F7F4;

/// The colors of a box's status dot: red, amber and green.
const STATUS: [u32; 3] = [0xE03131, 0xF08C00, 0x2F9E44];
/// The color of a vote, and of the gray parts of a rough piece of a screen.
const VOTE: u32 = 0x1971C2;
const GRAY: u32 = 0x868E96;

/// The rough piece of a screen some words on a box make it stand for: a
/// button, a field to type in, a card or a bar of links; by number.
pub fn ui_kind(words: &str) -> Option<u8> {
    match words.trim().to_lowercase().as_str() {
        "button" => Some(0),
        "input" => Some(1),
        "card" => Some(2),
        "nav" => Some(3),
        _ => None,
    }
}

/// Words wrapped to a width, a line at a time, each with how wide it is:
/// broken at spaces where a line would be too long, and at the new lines
/// typed; a word longer than the width alone on its line.
pub fn label_wrap(text: &mut Text, face: usize, px: u32, width: i32, words: &str) -> Vec<(String, i32)> {
    let mut lines = Vec::new();
    for paragraph in words.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let tried = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && text.width_in(face, &tried, px) > width {
                let wide = text.width_in(face, &line, px);
                lines.push((std::mem::replace(&mut line, word.to_owned()), wide));
            } else {
                line = tried;
            }
        }
        let wide = text.width_in(face, &line, px);
        lines.push((line, wide));
    }
    lines
}

/// How far a point is from the nearest of some lines, each a run of
/// points.
fn nearest(p: Point, paths: &[Vec<Point>]) -> f32 {
    paths.iter().flat_map(|path| path.windows(2)).map(|pair| segment(p, pair[0], pair[1])).fold(f32::MAX, f32::min)
}

/// Whether a point is inside a closed run of points, by how many of its
/// sides a line to its right crosses.
fn inside(p: Point, path: &[Point]) -> bool {
    // A hair off the line through it, never through a corner exactly.
    let p = (p.0, p.1 + 1e-3);
    let mut odd = false;
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

/// Points round an oval, from one angle to another, so many of them.
fn arc(middle: Point, radii: (f32, f32), from: f32, to: f32, count: usize) -> Vec<Point> {
    (0..=count).map(|step| {
        let angle = from + (to - from) * step as f32 / count as f32;
        (middle.0 + radii.0 * angle.cos(), middle.1 + radii.1 * angle.sin())
    }).collect()
}

impl Shape {
    /// Whether it is drawn by an outline of its own, rather than as a box
    /// or an oval.
    pub(super) fn has_form(&self) -> bool {
        self.tool == Tool::Rect && matches!(self.form, FORM_DIAMOND | FORM_CYLINDER | FORM_PERSON | FORM_CLOUD)
    }

    /// Whether its words say it is not known yet, "?" or "TBD": drawn
    /// dashed and hollow, as something still to be filled in.
    pub fn placeholder(&self) -> bool {
        let words = self.text.trim();
        matches!(self.tool, Tool::Rect | Tool::Ellipse) && (words == "?" || words.eq_ignore_ascii_case("tbd"))
    }

    /// What its hand-drawn wobble is worked out from: what it is called,
    /// so that it is the same wherever it is moved, and no two alike.
    pub(super) fn seed(&self) -> u32 {
        (self.id ^ 0x9E37_79B9).wrapping_mul(0x85EB_CA6B).rotate_left(13) ^ 0xC2B2_AE35
    }

    /// Its outline as closed runs of points, which are what is inside it,
    /// and the lines besides that are drawn but close nothing: the front
    /// of a database's top.
    pub fn form_paths(&self) -> (Vec<Vec<Point>>, Vec<Vec<Point>>) {
        let (x, y, w, h) = self.frame();
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let pi = std::f32::consts::PI;
        let (closed, open): (Vec<Vec<Point>>, Vec<Vec<Point>>) = match self.form {
            FORM_DIAMOND => (vec![vec![(cx, y), (x + w, cy), (cx, y + h), (x, cy), (cx, y)]], Vec::new()),
            FORM_CYLINDER => {
                let ry = (h * 0.12).min(w * 0.18).max(2.0);
                let rx = w / 2.0;
                let mut body = arc((cx, y + ry), (rx, ry), pi, 2.0 * pi, 24);
                body.extend(arc((cx, y + h - ry), (rx, ry), 0.0, pi, 24));
                body.push(body[0]);
                (vec![body], vec![arc((cx, y + ry), (rx, ry), 0.0, pi, 24)])
            }
            FORM_PERSON => {
                // A head over shoulders, in the upper part of it; its
                // words go under them.
                let figure = h * 0.62;
                let radius = (figure * 0.24).min(w * 0.22);
                let head = arc((cx, y + radius + 1.0), (radius, radius), 0.0, 2.0 * pi, 32);
                let (shoulders, bottom) = (y + radius * 2.0 + radius * 0.3, y + figure);
                let half = (radius * 1.9).min(w / 2.0);
                let mut body = arc((cx, bottom), (half, bottom - shoulders), pi, 2.0 * pi, 32);
                body.push(body[0]);
                (vec![head, body], Vec::new())
            }
            FORM_CLOUD => {
                // Scallops round an oval, each a bump out between two dips.
                let points: Vec<Point> = (0..=96)
                    .map(|step| {
                        let angle = 2.0 * pi * step as f32 / 96.0;
                        let bump = 0.86 + 0.14 * (angle * 4.0).sin().abs();
                        (cx + w / 2.0 * bump * angle.cos(), cy + h / 2.0 * bump * angle.sin())
                    })
                    .collect();
                (vec![points], Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        };
        let turn = |paths: Vec<Vec<Point>>| -> Vec<Vec<Point>> {
            match self.angle {
                0.0 => paths,
                angle => paths.into_iter().map(|path| path.into_iter().map(|p| turn_point(p, (cx, cy), angle)).collect()).collect(),
            }
        };
        (turn(closed), turn(open))
    }

    /// What its outline covers: drawn by hand, its lines gone over twice
    /// a little off; or clean, its lines, or with no border its inside.
    pub(super) fn form_mask(&self, bounds: Rect) -> Mask {
        let (closed, open) = self.form_paths();
        let half = self.stroke / 2.0;
        if self.sketched() {
            let mut mask = Mask { bounds, alpha: vec![0; bounds.w.max(0) as usize * bounds.h.max(0) as usize] };
            for (points, thin) in self.form_strokes() {
                for pair in points.windows(2) {
                    stamp(&mut mask, pair[0], pair[1], thin);
                }
            }
            return mask;
        }
        if self.filled && !self.border {
            return covered(bounds, move |px, py| {
                let far = nearest((px, py), &closed);
                if closed.iter().any(|path| inside((px, py), path)) { -far } else { far }
            });
        }
        let mut lines = closed;
        lines.extend(open);
        covered(bounds, move |px, py| nearest((px, py), &lines) - half)
    }

    /// The strokes a hand draws its outline with, each run of points with
    /// half its width.
    pub fn form_strokes(&self) -> Vec<(Vec<Point>, f32)> {
        let (closed, open) = self.form_paths();
        let half = self.stroke / 2.0;
        let mut strokes = Vec::new();
        {
            let mut seed = self.seed();
            let mut wobble = move |amount: f32| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * amount
            };
            let reach = (self.stroke * 1.2 + 3.0).min(10.0);
            let (x, y, w, h) = self.frame();
            let middle = (x + w / 2.0, y + h / 2.0);
            for pass in 0..2 {
                let thin = if pass == 0 { half } else { (half * 0.7).max(0.6) };
                let (shift, grow) = ((wobble(reach * 0.4), wobble(reach * 0.4)), 1.0 + wobble(0.02));
                for path in closed.iter().chain(&open) {
                    let moved: Vec<Point> = path.iter().map(|&(px, py)| (middle.0 + (px - middle.0) * grow + shift.0 * pass as f32, middle.1 + (py - middle.1) * grow + shift.1 * pass as f32)).collect();
                    // A few points of it each a little off, and the line
                    // between them bowed: a diamond's corners overshoot.
                    for pair in moved.windows(2) {
                        let jitter = if moved.len() < 8 { reach * 0.3 } else { 0.0 };
                        let a = (pair[0].0 + wobble(jitter), pair[0].1 + wobble(jitter));
                        let b = (pair[1].0 + wobble(jitter), pair[1].1 + wobble(jitter));
                        strokes.push((vec![a, b], thin));
                    }
                }
            }
        }
        strokes
    }

    /// What is inside its outline, which its fill is laid in.
    pub(super) fn form_inside(&self, bounds: Rect) -> Mask {
        let (closed, _) = self.form_paths();
        covered(bounds, move |px, py| {
            let far = nearest((px, py), &closed);
            if closed.iter().any(|path| inside((px, py), path)) { -far } else { far }
        })
    }

    /// Where its words go: the part of it they are wrapped to, as its left,
    /// top, width and height.
    pub fn label_area(&self) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = self.frame();
        let pad = (self.font_px as f32 * 0.6).max(6.0);
        if self.titled {
            return (x + pad, y + pad * 0.6, (w - 2.0 * pad).max(8.0), h);
        }
        let (across, down) = match (self.tool, self.form) {
            (Tool::Ellipse, _) => (0.72, 0.72),
            (_, FORM_DIAMOND) => (0.56, 0.6),
            (_, FORM_CLOUD) => (0.7, 0.66),
            _ => (1.0, 1.0),
        };
        match self.form {
            FORM_PERSON if self.tool == Tool::Rect => (x, y + h * 0.64, w, h * 0.36),
            FORM_CYLINDER if self.tool == Tool::Rect => {
                let top = (h * 0.24).min(w * 0.36);
                (x + pad, y + top, (w - 2.0 * pad).max(8.0), h - top)
            }
            _ => {
                let (iw, ih) = (w * across - 2.0 * pad * across, h * down);
                (x + (w - iw) / 2.0, y + (h - ih) / 2.0, iw.max(8.0), ih)
            }
        }
    }

    /// Its words as they are shown in it, a line at a time, each with
    /// where it begins across and down, and how wide it is; and how tall a
    /// line is.
    pub fn label_lines(&self, text: &mut Text) -> (Vec<(String, f32, f32, i32)>, i32) {
        let px = self.font_px;
        let high = Text::line_height(px);
        if self.text.is_empty() {
            return (Vec::new(), high);
        }
        if matches!(self.tool, Tool::Line | Tool::Arrow) {
            let lines = label_wrap(text, self.face, px, (px as i32 * 12).max(120), &self.text);
            let (mx, my) = self.middle();
            let top = my - (lines.len() as i32 * high) as f32 / 2.0;
            return (lines.into_iter().enumerate().map(|(at, (line, wide))| (line, mx - wide as f32 / 2.0, top + (at as i32 * high) as f32, wide)).collect(), high);
        }
        let (x, y, w, h) = self.label_area();
        let lines = label_wrap(text, self.face, px, w.floor() as i32, &self.text);
        let tall = (lines.len() as i32 * high) as f32;
        let top = if self.titled { y } else { y + (h - tall) / 2.0 };
        let shown = lines
            .into_iter()
            .enumerate()
            .map(|(at, (line, wide))| {
                let left = if self.titled { x } else { x + (w - wide as f32) / 2.0 };
                (line, left, top + (at as i32 * high) as f32, wide)
            })
            .collect();
        (shown, high)
    }

    /// Where the caret goes at the end of its words: across, down, and how
    /// tall.
    pub fn label_caret(&self, text: &mut Text) -> (f32, f32, i32) {
        let (lines, high) = self.label_lines(text);
        match lines.last() {
            Some((_, left, top, wide)) => (left + *wide as f32, *top, high),
            None => match matches!(self.tool, Tool::Line | Tool::Arrow) {
                true => {
                    let (mx, my) = self.middle();
                    (mx, my - high as f32 / 2.0, high)
                }
                false => {
                    let (x, y, w, h) = self.label_area();
                    (if self.titled { x } else { x + w / 2.0 }, if self.titled { y } else { y + (h - high as f32) / 2.0 }, high)
                }
            },
        }
    }

    /// How large it would need to be for its words, as a box grows to hold
    /// them: as wide as the longest line unwrapped, up to `most` across,
    /// and then as tall as they wrap to; never smaller than `least`.
    pub fn label_needs(&self, text: &mut Text, least: (f32, f32), most: f32) -> (f32, f32) {
        if self.text.is_empty() || self.titled || !matches!(self.tool, Tool::Rect | Tool::Ellipse) || ui_kind(&self.text).is_some() {
            return least;
        }
        let (x, y, _, _) = self.frame();
        let probe = |w: f32, h: f32| {
            let mut probe = self.clone();
            probe.points = vec![(x, y), (x + w, y + h)];
            probe.label_area()
        };
        // How much more room its words get for each pixel more across and
        // down it is.
        let (_, _, first_w, first_h) = probe(least.0, least.1);
        let (_, _, more_w, more_h) = probe(least.0 + 100.0, least.1 + 100.0);
        let (by_w, by_h) = (((more_w - first_w) / 100.0).max(0.2), ((more_h - first_h) / 100.0).max(0.2));
        let longest = self.text.split('\n').map(|line| text.width_in(self.face, line, self.font_px)).max().unwrap_or(0) as f32 + 2.0;
        let wide = (least.0 + (longest - first_w).max(0.0) / by_w).min(most).max(least.0);
        let (_, _, area, _) = probe(wide, least.1);
        let lines = label_wrap(text, self.face, self.font_px, area.floor() as i32, &self.text).len() as i32;
        let tall = (lines * Text::line_height(self.font_px)) as f32 + self.font_px as f32 * 0.6;
        (wide, (least.1 + (tall - first_h).max(0.0) / by_h).max(least.1))
    }

    /// Its words, and on a board's box what it stands for and the dots it
    /// carries: a rough piece of a screen in place of the words that name
    /// it, a status dot at its top right, and its votes along its top.
    pub(super) fn draw_label(&self, canvas: &mut Canvas, text: &mut Text) {
        let boxed = matches!(self.tool, Tool::Rect | Tool::Ellipse);
        if boxed {
            if let Some(kind) = ui_kind(&self.text).filter(|_| self.tool == Tool::Rect && self.form == FORM_BOX) {
                self.draw_piece(canvas, text, kind);
            } else {
                self.draw_words(canvas, text);
            }
            let (x, y, w, _) = self.frame();
            let dot = (self.font_px as f32 * 0.32).max(4.0);
            if (1..=3).contains(&self.status) {
                let (cx, cy) = ((x + w - dot * 2.2).round() as i32, (y + dot * 2.2).round() as i32);
                canvas.disc(cx, cy, dot.round() as i32 + 1, 0xFFFFFF, self.opacity);
                canvas.disc(cx, cy, dot.round() as i32, STATUS[usize::from(self.status - 1)], self.opacity);
            }
            for vote in 0..i32::from(self.votes.min(12)) {
                let (cx, cy) = ((x + dot * 2.2) as i32 + vote * (dot * 2.4) as i32, (y + dot * 2.2) as i32);
                canvas.disc(cx, cy, dot.round() as i32, VOTE, self.opacity);
            }
        } else if !self.text.is_empty() {
            // An arrow's words, on a patch of the paper, across its middle.
            let (lines, high) = self.label_lines(text);
            if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
                let left = lines.iter().map(|line| line.1).fold(f32::MAX, f32::min);
                let wide = lines.iter().map(|line| line.3).max().unwrap_or(0);
                let room = high / 5;
                canvas.round_fill(left as i32 - room * 2, first.2 as i32 - room / 2, wide + room * 4, (last.2 - first.2) as i32 + high + room, room * 2, if self.paper == 0 { PAPER } else { self.paper }, self.opacity);
            }
            self.draw_words(canvas, text);
        }
    }

    /// Its words where `label_lines` puts them, in its face, in what reads
    /// on it.
    fn draw_words(&self, canvas: &mut Canvas, text: &mut Text) {
        let ink = match self.tool {
            _ if self.placeholder() => GRAY,
            Tool::Rect | Tool::Ellipse if self.border && self.form != FORM_STICKY => self.color,
            Tool::Rect | Tool::Ellipse if self.filled => ink_on(self.fill),
            _ => self.color,
        };
        let (lines, _) = self.label_lines(text);
        text.look = Look { opacity: self.opacity, heavy: if self.titled { (self.font_px as i32 / 18).max(0) } else { 0 }, ..Look::default() };
        for (line, left, top, _) in lines {
            text.draw_in(self.face, canvas, left.round() as i32, top.round() as i32, self.font_px, ink, &line);
        }
        text.look = Look::default();
    }

    /// A rough piece of a screen, drawn in a box: a button, a field to type
    /// in, a card, or a bar of links.
    fn draw_piece(&self, canvas: &mut Canvas, text: &mut Text, kind: u8) {
        let (x, y, w, h) = self.frame();
        let px = self.font_px;
        let high = Text::line_height(px);
        let ink = if self.border { self.color } else { ink_on(self.fill) };
        let (xi, yi, wi, hi) = (x.round() as i32, y.round() as i32, w.round() as i32, h.round() as i32);
        let bar = |canvas: &mut Canvas, left: i32, top: i32, wide: i32| canvas.round_fill(left, top, wide.max(4), (px as i32 / 3).max(4), (px as i32 / 6).max(2), GRAY, 200);
        match kind {
            // A button: a pill inside, and its word in the middle of it.
            0 => {
                let inset = (h * 0.18).min(10.0) as i32;
                let (bw, bh) = (wi - 2 * inset, hi - 2 * inset);
                canvas.round_outline(xi + inset, yi + inset, bw, bh, bh / 2, (self.stroke * 0.7).round().max(1.0) as i32, ink);
                let wide = text.width_in(self.face, "Button", px);
                text.draw_in(self.face, canvas, xi + wi / 2 - wide / 2, yi + hi / 2 - high / 2, px, ink, "Button");
            }
            // A field: what to type, faint, and a caret before it.
            1 => {
                let left = xi + (px as i32).max(10);
                canvas.fill(left, yi + hi / 2 - high / 2, 2, high, ink);
                text.draw_in(self.face, canvas, left + 8, yi + hi / 2 - high / 2, px, GRAY, "Type here\u{2026}");
            }
            // A card: a picture crossed out over two lines of words.
            2 => {
                let pad = (w.min(h) * 0.08).max(6.0) as i32;
                let (pw, ph) = (wi - 2 * pad, (hi as f32 * 0.5) as i32);
                canvas.round_outline(xi + pad, yi + pad, pw, ph, 3, 1, GRAY);
                let mut cross = Mask { bounds: Rect::new(xi + pad - 2, yi + pad - 2, pw + 4, ph + 4), alpha: vec![0; ((pw + 4).max(0) * (ph + 4).max(0)) as usize] };
                let (a, b) = (((xi + pad) as f32, (yi + pad) as f32), ((xi + pad + pw) as f32, (yi + pad + ph) as f32));
                stamp(&mut cross, a, b, 0.7);
                stamp(&mut cross, (a.0, b.1), (b.0, a.1), 0.7);
                canvas.mask(&cross, GRAY, 200);
                let below = yi + pad + ph + pad;
                bar(canvas, xi + pad, below, pw * 7 / 10);
                bar(canvas, xi + pad, below + (px as i32 * 3 / 4).max(8), pw * 4 / 10);
            }
            // A bar of links: a mark at its left, and links at its right.
            _ => {
                let mid = yi + hi / 2;
                let dot = (h * 0.2).min(12.0) as i32;
                canvas.disc(xi + dot * 2, mid, dot, ink, 256);
                let link = (wi / 9).max(14);
                for at in 0..3 {
                    bar(canvas, xi + wi - (3 - at) * (link + link / 2) - dot, mid - (px as i32 / 6).max(2), link);
                }
            }
        }
    }
}
