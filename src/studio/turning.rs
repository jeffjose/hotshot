//! Turning on the canvas, as Figma does: what is picked out, one layer or
//! several about the middle of them all, turned by a drag from just outside
//! a corner, in steps of 15° with Shift; and the pointer saying what a drag
//! there would do before it is pressed: turn, or size by a corner or a side.

use super::*;
use crate::cursors;

/// How far outside a corner a press still turns, in pixels of the window,
/// and how near the corner it is still the corner's handle.
const REACH: i32 = 18;
const HANDLE: i32 = 6;

impl Studio {
    /// The corners of what is picked out, in the window and in order round
    /// it, to turn it by: the box of the one layer, turned with it; or the
    /// box round all of several. None for a line, which turns by its ends,
    /// or for what does not turn.
    pub(super) fn turn_corners(&mut self) -> Vec<(i32, i32)> {
        let chosen = self.chosen();
        let turns = |layer: &Layer| match &layer.kind {
            Kind::Picture(_) => true,
            Kind::Shape(shape) => shape.turns() && !matches!(shape.tool, Tool::Line | Tool::Arrow),
        };
        if chosen.is_empty() || self.cropping.is_some() || self.editing.is_some() || !chosen.iter().all(|&index| turns(&self.doc.layers[index])) {
            return Vec::new();
        }
        if let [index] = chosen[..] {
            return self.box_corners(index).to_vec();
        }
        let corners: Vec<(i32, i32)> = chosen.iter().flat_map(|&index| self.box_corners(index)).collect();
        let (left, right) = (corners.iter().map(|c| c.0).min().unwrap_or(0), corners.iter().map(|c| c.0).max().unwrap_or(0));
        let (top, bottom) = (corners.iter().map(|c| c.1).min().unwrap_or(0), corners.iter().map(|c| c.1).max().unwrap_or(0));
        vec![(left, top), (right, top), (right, bottom), (left, bottom)]
    }

    /// The corner a point of the window is just outside of, to turn from:
    /// near it, but not on its handle, and not inside the box. Which of
    /// the four it is: top left, top right, bottom right, bottom left.
    pub(super) fn turn_corner_at(&mut self, at: (i32, i32)) -> Option<usize> {
        if self.tool != Tool::Select || self.framing {
            return None;
        }
        let corners = self.turn_corners();
        if corners.len() != 4 {
            return None;
        }
        let near = |corner: (i32, i32), reach: i32| (corner.0 - at.0).abs() <= reach && (corner.1 - at.1).abs() <= reach;
        // Inside a box is on the same side of each of its edges.
        let side = |a: (i32, i32), b: (i32, i32)| ((b.0 - a.0) as i64 * (at.1 - a.1) as i64 - (b.1 - a.1) as i64 * (at.0 - a.0) as i64).signum();
        let sides: Vec<i64> = (0..4).map(|edge| side(corners[edge], corners[(edge + 1) % 4])).collect();
        let inside = sides.iter().all(|&s| s >= 0) || sides.iter().all(|&s| s <= 0);
        (0..4).find(|&at| near(corners[at], REACH) && !near(corners[at], HANDLE) && !inside)
    }

    /// What is picked out, all of it, about the middle of the box round it:
    /// the middle of the one layer, or of the box round several.
    fn turn_middle(&mut self) -> (f32, f32) {
        let chosen = self.chosen();
        if let [index] = chosen[..] {
            return match &self.doc.layers[index].kind {
                Kind::Picture(picture) => (picture.cx, picture.cy),
                Kind::Shape(shape) => shape.centre(&mut self.text),
            };
        }
        let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let all = actions::union(&boxes).unwrap_or_default();
        (all.0 + all.2 / 2.0, all.1 + all.3 / 2.0)
    }

    /// A press just outside a corner of what is picked out: it taken hold
    /// of to turn. Says whether it was.
    pub(super) fn press_turn(&mut self, at: (f32, f32), at_window: (i32, i32)) -> bool {
        if self.turn_corner_at(at_window).is_none() {
            return false;
        }
        self.checkpoint();
        self.held = self.chosen().iter().map(|&index| self.doc.layers[index].clone()).collect();
        let about = self.turn_middle();
        self.drag = Drag::Turn { about, began: (at.1 - about.1).atan2(at.0 - about.0) };
        true
    }

    /// What is held turned by as far as the pointer has gone round its
    /// middle: the one layer to a 15° step with Shift, several by steps of
    /// 15° together.
    pub(super) fn drag_turn(&mut self, about: (f32, f32), began: f32, at: (f32, f32), shift: bool) {
        let chosen = self.chosen();
        if chosen.len() != self.held.len() || chosen.is_empty() {
            return;
        }
        let mut by = (at.1 - about.1).atan2(at.0 - about.0) - began;
        if shift {
            let step = 15f32.to_radians();
            // One layer goes to a step from square; several go round by
            // steps from where they were.
            let start = match (&self.held[..], chosen.len()) {
                ([layer], 1) => layer_angle(layer),
                _ => 0.0,
            };
            by = ((start + by) / step).round() * step - start;
        }
        for (index, layer) in chosen.iter().zip(self.held.clone()) {
            let mut layer = layer;
            match &mut layer.kind {
                Kind::Picture(picture) => {
                    // Its middle taken round with it, about the middle of all.
                    let (cx, cy) = shapes::turn_point((picture.cx, picture.cy), about, by);
                    (picture.cx, picture.cy) = (cx, cy);
                    picture.angle = (picture.angle + by).rem_euclid(std::f32::consts::TAU);
                }
                Kind::Shape(shape) => shape.turn(by, about, &mut self.text),
            }
            self.doc.layers[*index] = layer;
        }
        self.turned_by = by;
        self.changed_layers(&chosen);
    }

    /// The pointer for where it is: turning just outside a corner, bowed
    /// the way that corner points; sizing on a handle or a side, the way it
    /// pulls as the layer is turned; while turning or sizing, the same.
    pub(super) fn pointer_shape(&mut self) -> Option<Cursor> {
        let at = self.pointer;
        match self.drag {
            Drag::Turn { about, .. } => {
                let middle = self.to_window(about);
                return Some(Cursor::TurnAt(cursors::turn_way((at.0 - middle.0) as f32, (at.1 - middle.1) as f32)));
            }
            Drag::Scale { pull, .. } => return self.pull_cursor(pull),
            Drag::None => {}
            _ => return None,
        }
        if self.tool != Tool::Select || !self.over_page(at) || self.space || self.framing {
            return None;
        }
        // The dots that round corners keep the arrow.
        if self.radius_handles().iter().any(|(spot, _)| (spot.0 - at.0).abs() <= 5 && (spot.1 - at.1).abs() <= 5) {
            return None;
        }
        let handles = self.handles();
        if let Some(&(_, pull)) = handles.iter().find(|(spot, _)| (spot.0 - at.0).abs() <= HANDLE && (spot.1 - at.1).abs() <= HANDLE) {
            return self.pull_cursor(pull);
        }
        if handles.len() == 8 {
            let off = |a: (i32, i32), b: (i32, i32)| {
                let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
                let along = (((at.0 - a.0) as f32 * dx + (at.1 - a.1) as f32 * dy) / (dx * dx + dy * dy).max(1.0)).clamp(0.0, 1.0);
                (at.0 as f32 - a.0 as f32 - along * dx).hypot(at.1 as f32 - a.1 as f32 - along * dy)
            };
            if let Some(side) = (0..4).find(|&side| off(handles[side].0, handles[(side + 1) % 4].0) <= 4.0) {
                return self.pull_cursor(handles[4 + side].1);
            }
        }
        // Bowed out along the corner's diagonal, turned as the layer is.
        if let Some(corner) = self.turn_corner_at(at) {
            let (px, py) = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)][corner];
            let angle = self.picked().map_or(0.0, |index| layer_angle(&self.doc.layers[index]));
            let way = py.atan2(px) + angle;
            return Some(Cursor::TurnAt(cursors::turn_way(way.cos(), way.sin())));
        }
        None
    }

    /// The sizing pointer for a handle that pulls a way, that way turned as
    /// the one picked out is.
    fn pull_cursor(&self, pull: (i32, i32)) -> Option<Cursor> {
        if pull.0 == END || pull.0 == effects::LENS || pull == (0, 0) {
            return None;
        }
        let angle = self.picked().map_or(0.0, |index| layer_angle(&self.doc.layers[index]));
        let way = (pull.1 as f32).atan2(pull.0 as f32) + angle;
        // Which of the four lines through the middle it lies nearest.
        let quarter = ((way / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(4);
        Some(match quarter {
            0 => Cursor::Across,
            1 => Cursor::Falling,
            2 => Cursor::Upright,
            _ => Cursor::Rising,
        })
    }

    /// While turning, how far, by the pointer: the angle the one layer is
    /// at, or how far several have gone round.
    pub(super) fn draw_turn_label(&mut self, canvas: &mut Canvas) {
        if !matches!(self.drag, Drag::Turn { .. }) {
            return;
        }
        let own = self.picked().filter(|&index| match &self.doc.layers[index].kind {
            Kind::Picture(_) => true,
            Kind::Shape(shape) => shape.turnable(),
        });
        let angle = match own {
            Some(index) => layer_angle(&self.doc.layers[index]),
            None => self.turned_by,
        };
        let mut degrees = angle.to_degrees().round() as i32 % 360;
        if degrees > 180 {
            degrees -= 360;
        } else if degrees <= -180 {
            degrees += 360;
        }
        self.pointer_label(canvas, &format!("{degrees}\u{b0}"));
    }
}

/// How far a layer is turned, in radians.
fn layer_angle(layer: &Layer) -> f32 {
    match &layer.kind {
        Kind::Picture(picture) => picture.angle,
        Kind::Shape(shape) => shape.angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> Studio {
        Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into())
    }

    fn boxed(studio: &mut Studio, from: (f32, f32), to: (f32, f32)) -> usize {
        let mut shape = Shape::new(Tool::Rect, 0x3E63DD, 2.0, from);
        (shape.points[1], shape.filled, shape.fill) = (to, true, 0x3E63DD);
        studio.doc.layers.push(Layer::shape(shape));
        studio.doc.layers.len() - 1
    }

    fn angle(studio: &Studio, index: usize) -> f32 {
        layer_angle(&studio.doc.layers[index]).to_degrees()
    }

    #[test]
    fn just_outside_a_corner_the_pointer_turns_and_a_drag_there_turns_in_steps_with_shift() {
        let mut studio = studio();
        let index = boxed(&mut studio, (100.0, 100.0), (300.0, 200.0));
        studio.selected = vec![index];
        let corner = studio.box_corners(index)[1];
        // Over the top right corner's handle, the pointer sizes along the
        // rising diagonal; just outside it, it turns, bowed up and right.
        studio.pointer_moved(corner.0, corner.1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::Rising);
        let outside = (corner.0 + 10, corner.1 - 10);
        studio.pointer_moved(outside.0, outside.1, Mods::default());
        assert_eq!(studio.cursor(), Cursor::TurnAt(7));
        // Just inside it, it does not.
        studio.pointer_moved(corner.0 - 12, corner.1 + 12, Mods::default());
        assert_ne!(studio.cursor(), Cursor::TurnAt(7));
        // Dragged a little way round with Shift, it goes to 15° from square.
        let middle = studio.to_window((200.0, 150.0));
        studio.pointer_moved(outside.0, outside.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        assert!(matches!(studio.drag, Drag::Turn { .. }));
        let (dx, dy) = ((outside.0 - middle.0) as f32, (outside.1 - middle.1) as f32);
        let (sin, cos) = 17f32.to_radians().sin_cos();
        let to = (middle.0 + (dx * cos - dy * sin) as i32, middle.1 + (dx * sin + dy * cos) as i32);
        let shift = Mods { shift: true, ..Mods::default() };
        studio.pointer_moved(to.0, to.1, shift);
        assert!(matches!(studio.cursor(), Cursor::TurnAt(_)));
        studio.released();
        assert!((angle(&studio, index) - 15.0).abs() < 0.01, "{}", angle(&studio, index));
        // Its middle stays where it was.
        let Kind::Shape(shape) = &studio.doc.layers[index].kind else { panic!() };
        let (x, y, w, h) = shape.frame();
        assert!(((x + w / 2.0) - 200.0).abs() < 0.01 && ((y + h / 2.0) - 150.0).abs() < 0.01);
        // And the number in the panel says so, and sets it.
        assert_eq!(studio.field(Field::Turn).map(f32::round), Some(15.0));
        studio.set_field(Field::Turn, 30.0);
        assert!((angle(&studio, index) - 30.0).abs() < 0.01);
    }

    #[test]
    fn several_picked_out_turn_together_about_the_middle_of_them_all() {
        let mut studio = studio();
        let first = boxed(&mut studio, (100.0, 100.0), (200.0, 200.0));
        let second = boxed(&mut studio, (300.0, 100.0), (400.0, 200.0));
        studio.selected = vec![first, second];
        // The box round them both, from (100, 100) to (400, 200): turned
        // half a turn about its middle, (250, 150), they change places.
        let corners = studio.turn_corners();
        assert_eq!(corners.len(), 4);
        let outside = (corners[2].0 + 10, corners[2].1 + 10);
        studio.pointer_moved(outside.0, outside.1, Mods::default());
        assert!(matches!(studio.cursor(), Cursor::TurnAt(1)));
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        let middle = studio.to_window((250.0, 150.0));
        let opposite = (2 * middle.0 - outside.0, 2 * middle.1 - outside.1);
        studio.pointer_moved((middle.0 + outside.0) / 2 + 40, (middle.1 + outside.1) / 2 - 80, Mods::default());
        studio.pointer_moved(opposite.0, opposite.1, Mods::default());
        studio.released();
        let Kind::Shape(moved) = &studio.doc.layers[first].kind else { panic!() };
        let (x, y, w, h) = moved.frame();
        assert!(((x + w / 2.0) - 350.0).abs() < 1.0 && ((y + h / 2.0) - 150.0).abs() < 1.0, "{x} {y}");
        assert!((angle(&studio, first) - 180.0).abs() < 1.0 && (angle(&studio, second) - 180.0).abs() < 1.0);
        // One step to undo puts both back.
        studio.undo();
        assert_eq!((angle(&studio, first), angle(&studio, second)), (0.0, 0.0));
    }
}
