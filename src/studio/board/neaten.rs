//! Neaten, a pen option off until asked for: a rough loop drawn with the
//! pen becomes an oval, a rough square a box, and a stroke ending in a V
//! an arrow tied to what it touches; anything else stays as drawn.

use super::*;

/// What a stroke is seen as, if anything.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::studio) enum Seen {
    Box,
    Oval,
    Arrow,
}

/// What a pen stroke looks like it was meant to be.
pub(in crate::studio) fn seen(points: &[(f32, f32)]) -> Option<Seen> {
    if points.len() < 6 {
        return None;
    }
    let (left, top) = (points.iter().map(|p| p.0).fold(f32::MAX, f32::min), points.iter().map(|p| p.1).fold(f32::MAX, f32::min));
    let (right, bottom) = (points.iter().map(|p| p.0).fold(f32::MIN, f32::max), points.iter().map(|p| p.1).fold(f32::MIN, f32::max));
    let (w, h) = (right - left, bottom - top);
    let size = w.max(h);
    if size < 20.0 {
        return None;
    }
    let (first, last) = (points[0], points[points.len() - 1]);
    let closed = (first.0 - last.0).hypot(first.1 - last.1) < size * 0.25;
    if closed && w > 12.0 && h > 12.0 {
        let (cx, cy, rx, ry) = ((left + right) / 2.0, (top + bottom) / 2.0, w / 2.0, h / 2.0);
        // How far each point is from an oval's line, and from a box's.
        let oval = points.iter().map(|p| ((((p.0 - cx) / rx).powi(2) + ((p.1 - cy) / ry).powi(2)).sqrt() - 1.0).abs()).sum::<f32>() / points.len() as f32;
        let boxy = points.iter().map(|p| ((p.0 - left).abs().min((p.0 - right).abs()) / w).min((p.1 - top).abs().min((p.1 - bottom).abs()) / h)).sum::<f32>() / points.len() as f32;
        return Some(if boxy < oval * 0.6 { Seen::Box } else { Seen::Oval });
    }
    // Open: a straight run, its end turned back as a V.
    let far = |p: &(f32, f32)| (p.0 - first.0).hypot(p.1 - first.1);
    let most = points.iter().map(far).fold(0.0f32, f32::max);
    // The tip: where it first comes as far as it goes.
    let at = points.iter().position(|p| far(p) >= most * 0.97)?;
    let tip_point = points[at];
    let run = (tip_point.0 - first.0).hypot(tip_point.1 - first.1);
    let straight = points[..=at].iter().all(|p| {
        let (dx, dy) = (tip_point.0 - first.0, tip_point.1 - first.1);
        ((p.0 - first.0) * dy - (p.1 - first.1) * dx).abs() / run.max(1.0) < run * 0.12 + 6.0
    });
    let back = points.len() - 1 - at;
    (straight && back >= 2 && run > 40.0 && (last.0 - tip_point.0).hypot(last.1 - tip_point.1) < run * 0.4).then_some(Seen::Arrow)
}

impl Studio {
    /// A pen stroke just drawn on a board, with Neaten on: made what it
    /// looks like it was meant to be, in its place.
    pub(in crate::studio) fn neaten(&mut self, index: usize) {
        if !self.board.neaten {
            return;
        }
        let Some(Layer { kind: Kind::Shape(stroke), .. }) = self.doc.layers.get(index) else { return };
        let points = stroke.points.clone();
        let Some(meant) = seen(&points) else { return };
        let (left, top) = (points.iter().map(|p| p.0).fold(f32::MAX, f32::min), points.iter().map(|p| p.1).fold(f32::MAX, f32::min));
        let (right, bottom) = (points.iter().map(|p| p.0).fold(f32::MIN, f32::max), points.iter().map(|p| p.1).fold(f32::MIN, f32::max));
        let tool = match meant {
            Seen::Box => Tool::Rect,
            Seen::Oval => Tool::Ellipse,
            Seen::Arrow => Tool::Arrow,
        };
        let mut shape = Shape::new(tool, self.board.stroke, LINE, (left, top));
        let making = self.board.making;
        self.board.making = shapes::FORM_BOX;
        self.board_dress(&mut shape);
        self.board.making = making;
        match meant {
            Seen::Arrow => {
                let far = points.iter().copied().max_by(|a, b| (a.0 - points[0].0).hypot(a.1 - points[0].1).total_cmp(&(b.0 - points[0].0).hypot(b.1 - points[0].1))).unwrap_or(points[0]);
                shape.points = vec![points[0], far];
            }
            _ => {
                shape.points = vec![(left, top), (right, bottom)];
                shape.base = (right - left, bottom - top);
            }
        }
        self.doc.layers[index] = Layer::shape(shape);
        if meant == Seen::Arrow {
            self.board_stick(index, &[0, 1]);
        }
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn stroke(studio: &mut Studio, points: &[(f32, f32)]) {
        let from = studio.to_window_page(points[0]);
        studio.pointer_moved(from.0, from.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        for &point in &points[1..] {
            let at = studio.to_window_page(point);
            studio.pointer_moved(at.0, at.1, Mods::default());
        }
        studio.released();
    }

    #[test]
    fn with_neaten_on_a_rough_loop_is_an_oval_a_square_a_box_and_a_v_an_arrow() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('p'), Mods::default());
        let circle: Vec<(f32, f32)> = (0..=40).map(|step| {
            let angle = step as f32 / 40.0 * std::f32::consts::TAU;
            (300.0 + 80.0 * angle.cos() + (step % 3) as f32, 300.0 + 60.0 * angle.sin())
        }).collect();
        stroke(&mut studio, &circle);
        assert_eq!(shape(&studio, 0).tool, Tool::Pen, "off until asked for");
        studio.board.neaten = true;
        stroke(&mut studio, &circle);
        assert_eq!(shape(&studio, 1).tool, Tool::Ellipse);
        let square: Vec<(f32, f32)> = [(500.0, 100.0), (560.0, 101.0), (620.0, 100.0), (621.0, 160.0), (620.0, 220.0), (560.0, 221.0), (500.0, 220.0), (501.0, 160.0), (500.0, 103.0)].to_vec();
        stroke(&mut studio, &square);
        assert_eq!(shape(&studio, 2).tool, Tool::Rect);
        let arrow: Vec<(f32, f32)> = [(100.0, 600.0), (150.0, 601.0), (200.0, 600.0), (250.0, 601.0), (300.0, 600.0), (285.0, 588.0), (300.0, 600.0), (286.0, 612.0)].to_vec();
        stroke(&mut studio, &arrow);
        assert_eq!(shape(&studio, 3).tool, Tool::Arrow);
        let scribble: Vec<(f32, f32)> = [(700.0, 600.0), (720.0, 660.0), (760.0, 610.0), (790.0, 700.0), (830.0, 590.0), (870.0, 690.0)].to_vec();
        stroke(&mut studio, &scribble);
        assert_eq!(shape(&studio, 4).tool, Tool::Pen, "anything else as drawn");
    }
}
