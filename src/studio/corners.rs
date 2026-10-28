//! Rounding corners on the canvas, as Figma does: a dot inside each corner
//! of the box or picture picked out, dragged in to make all its corners
//! rounder, or with Alt only the one; and in the panel a radius for all of
//! them, or one for each.

use super::*;

/// Which way into a box each corner is, counted as Figma counts them: top
/// left, top right, bottom right, bottom left.
pub(super) const INWARD: [(f32, f32); 4] = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];

/// How far in from its corner a dot is at the least, on the screen, to be
/// got hold of when the corner is square or nearly.
const LEAST: f32 = 12.0;

/// How small on the screen a box can be and still have the dots, as Figma
/// leaves them off a small one, where they would crowd its handles.
const SMALLEST: f32 = 40.0;

/// The box of a layer whose corners can be rounded, unturned about its
/// middle: where its middle is, half its width and its height, how far it
/// is turned, and the radius of each corner.
#[derive(Clone, Copy)]
pub(super) struct Rounded {
    pub centre: (f32, f32),
    pub half: (f32, f32),
    pub angle: f32,
    pub radii: [f32; 4],
}

impl Rounded {
    pub fn of(layer: &Layer) -> Option<Rounded> {
        match &layer.kind {
            Kind::Picture(picture) => Some(Rounded { centre: (picture.cx, picture.cy), half: (picture.w / 2.0, picture.h / 2.0), angle: picture.angle, radii: picture.radii() }),
            Kind::Shape(shape) if shape.tool == Tool::Rect => {
                let (x, y, w, h) = shape.frame();
                Some(Rounded { centre: (x + w / 2.0, y + h / 2.0), half: (w / 2.0, h / 2.0), angle: shape.angle, radii: shape.radii() })
            }
            Kind::Shape(_) => None,
        }
    }

    /// A point of the artboard as it is from the middle of the box, the
    /// box unturned.
    pub fn local(&self, p: (f32, f32)) -> (f32, f32) {
        let unturned = shapes::turn_point(p, self.centre, -self.angle);
        (unturned.0 - self.centre.0, unturned.1 - self.centre.1)
    }

    /// The other way: a point from the middle of the box, on the artboard.
    pub fn placed(&self, p: (f32, f32)) -> (f32, f32) {
        shapes::turn_point((self.centre.0 + p.0, self.centre.1 + p.1), self.centre, self.angle)
    }

    /// The most a corner can be rounded: half the shorter side.
    pub fn most(&self) -> f32 {
        self.half.0.min(self.half.1).max(0.0)
    }

    /// How far in from a corner a point is, by the lesser of across and
    /// down, the box unturned.
    pub fn inward(&self, corner: usize, p: (f32, f32)) -> f32 {
        let (lx, ly) = self.local(p);
        let (sx, sy) = INWARD[corner];
        (lx * sx + self.half.0).min(ly * sy + self.half.1)
    }
}

/// The radius of each corner of a layer made these, each no more than
/// half its shorter side.
pub(super) fn round_layer(layer: &mut Layer, radii: [f32; 4]) {
    let most = Rounded::of(layer).map_or(f32::MAX, |rounded| rounded.most());
    let radii = radii.map(|radius| radius.clamp(0.0, most).round());
    match &mut layer.kind {
        Kind::Picture(picture) => picture.set_radii(radii),
        Kind::Shape(shape) => shape.set_radii(radii),
    }
}

impl Studio {
    /// The dots inside the corners of the one box or picture picked out,
    /// in the window, each with which corner it is: as far in as the
    /// corner is round, or far enough in to be got hold of. None while
    /// anything else is going on, or on one too small on the screen.
    pub(super) fn radius_handles(&mut self) -> Vec<((i32, i32), usize)> {
        let busy = !matches!(self.drag, Drag::None | Drag::Radius { .. });
        if self.tool != Tool::Select || busy || self.cropping.is_some() || self.editing.is_some() || self.framing {
            return Vec::new();
        }
        let Some(index) = self.picked() else { return Vec::new() };
        let Some(rounded) = Rounded::of(&self.doc.layers[index]) else { return Vec::new() };
        if rounded.half.0.min(rounded.half.1) * 2.0 * self.zoom < SMALLEST {
            return Vec::new();
        }
        (0..4)
            .map(|corner| {
                let inset = rounded.radii[corner].max(LEAST / self.zoom).min(rounded.most());
                let (sx, sy) = INWARD[corner];
                let spot = rounded.placed((-sx * rounded.half.0 + sx * inset, -sy * rounded.half.1 + sy * inset));
                (self.to_window(spot), corner)
            })
            .collect()
    }

    /// A press on one of the dots: its corner taken hold of, and with Alt
    /// it alone. Says whether it was on one.
    pub(super) fn press_radius(&mut self, at: (f32, f32), at_window: (i32, i32)) -> bool {
        let near = |spot: (i32, i32)| (spot.0 - at_window.0).abs() <= 5 && (spot.1 - at_window.1).abs() <= 5;
        let Some(&(_, corner)) = self.radius_handles().iter().find(|(spot, _)| near(*spot)) else { return false };
        let Some(index) = self.picked() else { return false };
        let Some(rounded) = Rounded::of(&self.doc.layers[index]) else { return false };
        self.checkpoint();
        self.held = vec![self.doc.layers[index].clone()];
        // Taken hold of a little in from where the corner's curve would
        // put it, it keeps that much in hand rather than jumping.
        let slack = rounded.inward(corner, at) - rounded.radii[corner];
        self.drag = Drag::Radius { corner, single: self.mods.alt, slack };
        if self.mods.alt {
            self.corners_open = true;
        }
        true
    }

    /// A dot dragged: its corner, or all of them, as far in as it is.
    pub(super) fn drag_radius(&mut self, corner: usize, single: bool, slack: f32, at: (f32, f32)) {
        let Some(index) = self.picked() else { return };
        let Some(mut layer) = self.held.first().cloned() else { return };
        let Some(rounded) = Rounded::of(&layer) else { return };
        let radius = (rounded.inward(corner, at) - slack).clamp(0.0, rounded.most());
        let radii = match single {
            true => {
                let mut radii = rounded.radii;
                radii[corner] = radius;
                radii
            }
            false => [radius; 4],
        };
        round_layer(&mut layer, radii);
        // The next box drawn is as round, when all its corners are.
        if !single && matches!(&layer.kind, Kind::Shape(_)) {
            self.radius = radius.round();
        }
        self.doc.layers[index] = layer;
        self.changed_layers(&[index]);
    }

    /// The dots drawn: white, ringed in the accent, as Figma's are; the
    /// one being dragged filled with the accent.
    pub(super) fn draw_radius_handles(&mut self, canvas: &mut Canvas) {
        let held = match self.drag {
            Drag::Radius { corner, .. } => Some(corner),
            _ => None,
        };
        let pointer = self.pointer;
        for (spot, corner) in self.radius_handles() {
            // Larger under the pointer, to say it can be taken hold of.
            let hovered = held.is_none() && (spot.0 - pointer.0).abs() <= 5 && (spot.1 - pointer.1).abs() <= 5;
            let size = if hovered { 6 } else { 5 };
            canvas.disc(spot.0, spot.1, size, ACCENT, 256);
            canvas.disc(spot.0, spot.1, size - 2, if held == Some(corner) { ACCENT } else { 0xFFFFFF }, 256);
        }
        // How round, by the pointer, while a dot is dragged.
        if let (Some(corner), Some(index)) = (held, self.picked()) {
            if let Some(rounded) = Rounded::of(&self.doc.layers[index]) {
                let radius = rounded.radii[corner].min(rounded.most()).round() as i32;
                self.pointer_label(canvas, &format!("Radius {radius}"));
            }
        }
    }

    /// A few words in a pill of the accent just below and to the right of
    /// the pointer, as Figma says how far a drag has gone.
    pub(super) fn pointer_label(&mut self, canvas: &mut Canvas, says: &str) {
        let wide = self.text.width(says, 11) + 12;
        let (x, y) = (self.pointer.0 + 14, self.pointer.1 + 18);
        canvas.round_fill(x, y, wide, 18, 4, ACCENT, 256);
        self.text.draw(canvas, x + 6, y + (18 - Text::line_height(11)) / 2, 11, 0xFFFFFF, says);
    }

    /// How round a corner of the one picked out is, or all of them when
    /// they are the same: none for several, or corners that differ.
    pub(super) fn radius_field(&self, corner: Option<usize>) -> Option<f32> {
        let index = self.picked()?;
        let rounded = Rounded::of(&self.doc.layers[index])?;
        match corner {
            Some(corner) => Some(rounded.radii[corner.min(3)]),
            None => rounded.radii.iter().all(|&radius| radius == rounded.radii[0]).then_some(rounded.radii[0]),
        }
    }

    /// Whether what is picked out has corners that differ, for its radius
    /// to say Mixed, as Figma's does.
    pub(super) fn radius_mixed(&self) -> bool {
        self.picked().and_then(|index| Rounded::of(&self.doc.layers[index])).is_some_and(|rounded| rounded.radii.iter().any(|&radius| radius != rounded.radii[0]))
    }

    /// A radius typed or dragged in the panel: for all the corners of each
    /// layer picked out, or for one.
    pub(super) fn set_radius_field(&mut self, corner: Option<usize>, value: f32) {
        let value = value.max(0.0);
        for index in self.chosen() {
            let layer = &mut self.doc.layers[index];
            let Some(rounded) = Rounded::of(layer) else { continue };
            let radii = match corner {
                Some(corner) => {
                    let mut radii = rounded.radii;
                    radii[corner.min(3)] = value;
                    radii
                }
                None => [value; 4],
            };
            round_layer(layer, radii);
            // The next box drawn as round as this one is made.
            if corner.is_none() && matches!(&layer.kind, Kind::Shape(_)) {
                self.radius = value.min(rounded.most()).round();
            }
        }
    }
}

/// A corner's curve, small, as the label of its field: a quarter of a
/// rounded square, the part of it at that corner.
/// Its corner is at `(px, py)`, and each arm of it `arm` long.
pub(super) fn corner_glyph(canvas: &mut Canvas, (px, py): (i32, i32), corner: usize, arm: i32, color: u32) {
    let (sx, sy) = INWARD[corner.min(3)];
    let (sx, sy) = (sx as i32, sy as i32);
    // From the corner, a short edge each way, and between them the curve:
    // what lies within half a pixel of a circle about (curve, curve).
    let curve = (arm / 2).max(2);
    for along in curve..arm {
        canvas.blend(px + along * sx, py, color, 255);
        canvas.blend(px, py + along * sy, color, 255);
    }
    for y in 0..curve {
        for x in 0..curve {
            let away = (x as f32 + 0.5 - curve as f32).hypot(y as f32 + 0.5 - curve as f32);
            let on = 1.0 - ((away - curve as f32 + 0.5).abs() - 0.3).clamp(0.0, 1.0);
            if on > 0.0 {
                canvas.blend(px + x * sx, py + y * sy, color, (on * 255.0) as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_counts_in_from_its_own_corner() {
        let rounded = Rounded { centre: (50.0, 30.0), half: (50.0, 30.0), angle: 0.0, radii: [0.0; 4] };
        assert_eq!(rounded.inward(0, (10.0, 20.0)), 10.0);
        assert_eq!(rounded.inward(2, (90.0, 50.0)), 10.0);
        assert_eq!(rounded.most(), 30.0);
    }

    fn studio() -> Studio {
        Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into())
    }

    /// A box picked out, filled in one color with no border.
    fn boxed(studio: &mut Studio) -> usize {
        let mut shape = Shape::new(Tool::Rect, 0x3E63DD, 2.0, (100.0, 100.0));
        (shape.points[1], shape.filled, shape.fill, shape.border) = ((300.0, 240.0), true, 0x3E63DD, false);
        studio.doc.layers.push(Layer::shape(shape));
        studio.selected = vec![studio.doc.layers.len() - 1];
        studio.doc.layers.len() - 1
    }

    fn radii(studio: &Studio, index: usize) -> [f32; 4] {
        Rounded::of(&studio.doc.layers[index]).unwrap().radii
    }

    /// A press on a point of the window, a drag to another, and a let go.
    fn drag(studio: &mut Studio, from: (i32, i32), to: (i32, i32), mods: Mods) {
        studio.pointer_moved(from.0, from.1, mods);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((from.0 + to.0) / 2, (from.1 + to.1) / 2, mods);
        studio.pointer_moved(to.0, to.1, mods);
        studio.released();
    }

    #[test]
    fn a_dot_inside_a_corner_rounds_them_all_and_with_alt_only_its_own() {
        let mut studio = studio();
        let index = boxed(&mut studio);
        let dots = studio.radius_handles();
        assert_eq!(dots.len(), 4);
        // Dragged 20 further in, at a zoom of 1: all four 20 rounder than
        // the least a dot sits in, which a square corner is held at.
        let (spot, _) = dots[0];
        let by = (20.0 * studio.zoom).round() as i32;
        drag(&mut studio, spot, (spot.0 + by, spot.1 + by), Mods::default());
        let all = radii(&studio, index);
        assert!(all.iter().all(|&radius| radius == all[0]) && all[0] >= 18.0, "{all:?}");
        // One step to undo, back to square.
        studio.undo();
        assert_eq!(radii(&studio, index), [0.0; 4]);
        // With Alt, the bottom right alone, and the panel shows each.
        let (spot, corner) = studio.radius_handles()[2];
        assert_eq!(corner, 2);
        drag(&mut studio, spot, (spot.0 - by, spot.1 - by), Mods { alt: true, ..Mods::default() });
        let each = radii(&studio, index);
        assert!(each[2] >= 18.0 && each[0] == 0.0 && each[1] == 0.0 && each[3] == 0.0, "{each:?}");
        assert!(studio.corners_open && studio.radius_mixed() && studio.field(Field::Radius).is_none());
        // No rounder than half the shorter side, however far it goes; Alt
        // let go of a while ago.
        studio.seen_mods = (None, None);
        let (spot, _) = studio.radius_handles()[0];
        drag(&mut studio, spot, (spot.0 + 400, spot.1 + 400), Mods::default());
        assert_eq!(radii(&studio, index), [70.0; 4]);
    }

    #[test]
    fn the_panel_sets_a_radius_for_all_corners_or_for_one() {
        let mut studio = studio();
        let index = boxed(&mut studio);
        studio.set_field(Field::Radius, 12.0);
        assert_eq!((radii(&studio, index), studio.field(Field::Radius)), ([12.0; 4], Some(12.0)));
        studio.act(Action::Corners);
        assert!(studio.control_rect(Control::Field(Field::Corner(3))).is_some());
        studio.set_field(Field::Corner(1), 30.0);
        assert_eq!(radii(&studio, index), [12.0, 30.0, 12.0, 12.0]);
        assert_eq!(studio.field(Field::Corner(1)), Some(30.0));
        // The radius typed again makes them all the same once more.
        studio.set_field(Field::Radius, 4.0);
        assert!(!studio.radius_mixed());
        // A picture's corners the same way.
        let picture = Picture::new(Arc::new(Image::new(200, 100, vec![0xFFFF0000; 200 * 100])), "red".into(), (500.0, 400.0));
        studio.doc.layers.push(Layer::picture(picture));
        studio.selected = vec![studio.doc.layers.len() - 1];
        studio.set_field(Field::Corner(0), 25.0);
        assert_eq!(radii(&studio, studio.doc.layers.len() - 1), [25.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn corners_of_their_own_are_drawn_each_as_round_and_kept_in_the_file() {
        let mut text = Text::load();
        let mut doc = Doc::new(200, 200);
        let mut shape = Shape::new(Tool::Rect, 0x3E63DD, 2.0, (20.0, 20.0));
        (shape.points[1], shape.filled, shape.fill, shape.border) = ((180.0, 180.0), true, 0x3E63DD, false);
        shape.set_radii([40.0, 0.0, 0.0, 0.0]);
        doc.layers.push(Layer::shape(shape));
        let picture = Picture { radius: 0.0, corners: Some([0.0, 0.0, 30.0, 0.0]), ..Picture::new(Arc::new(Image::new(60, 60, vec![0xFFFF0000; 3600])), "red".into(), (100.0, 100.0)) };
        doc.layers.push(Layer::picture(picture));
        for times in [1.0, 2.0] {
            let (wide, _, out) = doc.render(times, &mut text);
            let at = |x: f32, y: f32| out[((y * times) as i32 * wide + (x * times) as i32) as usize] & 0xFF_FFFF;
            // The top left is cut round; the top right is square.
            assert_eq!(at(23.0, 23.0), 0xFFFFFF, "at {times}");
            assert_eq!(at(177.0, 22.0), 0x3E63DD, "at {times}");
            // The picture's bottom right is round, its top left square.
            assert_eq!(at(129.0, 129.0), 0x3E63DD, "at {times}");
            assert_eq!(at(71.0, 71.0), 0xFF0000, "at {times}");
        }
        // Read back as written; a line from before reads as one radius.
        let again = Doc::read(&doc.write()).unwrap();
        let Kind::Shape(shape) = &again.layers[0].kind else { panic!() };
        let Kind::Picture(picture) = &again.layers[1].kind else { panic!() };
        assert_eq!((shape.radii(), picture.radii()), ([40.0, 0.0, 0.0, 0.0], [0.0, 0.0, 30.0, 0.0]));
        let old = "rect\t3E63DD\t2\t1\t8\t16\t0\t00\t0,256,0,0,0\t3E63DD\t0\t20,20 180,180\t";
        let old = Shape::read(old).unwrap();
        assert_eq!((old.radii(), old.corners), ([8.0; 4], None));
        assert!(Shape::read(&old.write()).unwrap().corners.is_none());
    }
}
