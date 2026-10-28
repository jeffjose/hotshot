//! Arranging what is picked out, beyond lining up and spacing: packed
//! side by side, matched to the first picked, lined up with the frame,
//! and turned; and whether they lie in a row or a column, which says
//! which of the buttons lead.

use super::*;

/// Whether boxes lie more across than down: their middles spread wider
/// than they spread high.
pub(super) fn in_a_row(boxes: &[(f32, f32, f32, f32)]) -> bool {
    let middles: Vec<(f32, f32)> = boxes.iter().map(|b| (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0)).collect();
    let spread = |pick: fn(&(f32, f32)) -> f32| {
        let values: Vec<f32> = middles.iter().map(pick).collect();
        values.iter().copied().fold(f32::MIN, f32::max) - values.iter().copied().fold(f32::MAX, f32::min)
    };
    spread(|m| m.0) >= spread(|m| m.1)
}

/// Where each box goes, packed: the first, by where it begins, stays,
/// and each after it begins where the one before ends. Gives how far
/// each moves, in the order given.
pub(super) fn packed(boxes: &[(f32, f32, f32, f32)], across: bool) -> Vec<f32> {
    let key = |b: &(f32, f32, f32, f32)| if across { b.0 } else { b.1 };
    let size = |b: &(f32, f32, f32, f32)| if across { b.2 } else { b.3 };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| key(&boxes[a]).total_cmp(&key(&boxes[b])));
    let mut moves = vec![0.0; boxes.len()];
    let Some(&first) = order.first() else { return moves };
    let mut at = key(&boxes[first]);
    for index in order {
        moves[index] = at - key(&boxes[index]);
        at += size(&boxes[index]);
    }
    moves
}

impl Studio {
    pub(super) fn arrange(&mut self, action: Action) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        match action {
            Action::Pack(way) => {
                let moves = packed(&boxes, way == 0);
                self.checkpoint();
                for (&index, by) in chosen.iter().zip(moves) {
                    let by = by.round();
                    self.doc.layers[index].translate(if way == 0 { by } else { 0.0 }, if way == 0 { 0.0 } else { by });
                }
            }
            Action::ToFrame(way) => {
                let (fw, fh) = (self.doc.width as f32, self.doc.height as f32);
                self.checkpoint();
                for (&index, &(x, y, w, h)) in chosen.iter().zip(&boxes) {
                    let (dx, dy) = match way {
                        0 => (-x, 0.0),
                        1 => (fw / 2.0 - (x + w / 2.0), 0.0),
                        2 => (fw - (x + w), 0.0),
                        3 => (0.0, -y),
                        4 => (0.0, fh / 2.0 - (y + h / 2.0)),
                        5 => (0.0, fh - (y + h)),
                        _ => (fw / 2.0 - (x + w / 2.0), fh / 2.0 - (y + h / 2.0)),
                    };
                    self.doc.layers[index].translate(dx.round(), dy.round());
                }
            }
            Action::Match(what) => {
                if chosen.len() < 2 {
                    return self.say("Pick two or more: the rest are matched to the first");
                }
                let first = self.doc.layers[chosen[0]].clone();
                let (_, _, tw, th) = boxes[0];
                self.checkpoint();
                for (&index, &(x, y, w, h)) in chosen.iter().zip(&boxes).skip(1) {
                    let layer = &mut self.doc.layers[index];
                    let (sx, sy) = (tw / w.max(1.0), th / h.max(1.0));
                    match what {
                        0 => actions::scale_layer(layer, (x, y), (sx, 1.0)),
                        1 => actions::scale_layer(layer, (x, y), (1.0, sy)),
                        2 => actions::scale_layer(layer, (x, y), (sx, sy)),
                        3 => match_corners(layer, &first),
                        4 => match_shadow(layer, &first),
                        _ => match_look(layer, &first),
                    }
                }
            }
            Action::Turn(degrees) => {
                let Some((left, top, w, h)) = actions::union(&boxes) else { return };
                let (about, by) = ((left + w / 2.0, top + h / 2.0), f32::from(degrees).to_radians());
                self.checkpoint();
                for &index in &chosen {
                    match &mut self.doc.layers[index].kind {
                        Kind::Picture(picture) => {
                            (picture.cx, picture.cy) = shapes::turn_point((picture.cx, picture.cy), about, by);
                            picture.angle = (picture.angle + by).rem_euclid(std::f32::consts::TAU);
                            if picture.angle.min(std::f32::consts::TAU - picture.angle) < 1e-4 {
                                picture.angle = 0.0;
                            }
                        }
                        Kind::Shape(shape) => shape.turn(by, about, &mut self.text),
                    }
                }
            }
            _ => return,
        }
        self.changed();
    }
}

/// How round a layer's corners are, made another's.
fn match_corners(layer: &mut Layer, like: &Layer) {
    let radius = match &like.kind {
        Kind::Picture(picture) => picture.radius,
        Kind::Shape(shape) => shape.radius,
    };
    match &mut layer.kind {
        Kind::Picture(picture) => picture.radius = radius,
        Kind::Shape(shape) => {
            shape.radius = radius;
            shape.touch();
        }
    }
}

/// The shadow a layer casts, made another's.
fn match_shadow(layer: &mut Layer, like: &Layer) {
    let drop = like.drop();
    match (&mut layer.kind, &like.kind) {
        (Kind::Picture(picture), Kind::Picture(other)) => (picture.shadow, picture.drop) = (other.shadow, other.drop),
        (Kind::Picture(picture), _) => match drop {
            Some(drop) => (picture.shadow, picture.drop) = (CUSTOM_SHADOW, drop),
            None => picture.shadow = 0,
        },
        (Kind::Shape(shape), _) => {
            shape.shadow = drop;
            shape.touch();
        }
    }
}

/// Everything about how a layer looks made another's, as far as they are
/// the same kind of thing: corners, shadow and how opaque; a picture's
/// window and adjustments; a shape's colors, line and how its text is set.
fn match_look(layer: &mut Layer, like: &Layer) {
    match_corners(layer, like);
    match_shadow(layer, like);
    match (&mut layer.kind, &like.kind) {
        (Kind::Picture(picture), Kind::Picture(other)) => {
            (picture.opacity, picture.window, picture.adjust) = (other.opacity, other.window, other.adjust);
        }
        (Kind::Shape(shape), Kind::Shape(other)) => {
            (shape.color, shape.opacity, shape.stroke) = (other.color, other.opacity, other.stroke);
            (shape.dashed, shape.dotted, shape.sketch, shape.halo) = (other.dashed, other.dotted, other.sketch, other.halo);
            if matches!(shape.tool, Tool::Rect | Tool::Ellipse) == matches!(other.tool, Tool::Rect | Tool::Ellipse) {
                (shape.filled, shape.fill, shape.border, shape.edge) = (other.filled, other.fill, other.border, other.edge);
            }
            if matches!(shape.tool, Tool::Text | Tool::Badge) && matches!(other.tool, Tool::Text | Tool::Badge) {
                (shape.face, shape.font_px, shape.style, shape.align) = (other.face, other.font_px, other.style, other.align);
            }
            if matches!(shape.tool, Tool::Line | Tool::Arrow) && matches!(other.tool, Tool::Line | Tool::Arrow) {
                (shape.heads, shape.ends) = (other.heads, other.ends);
            }
            shape.touch();
        }
        (Kind::Picture(picture), Kind::Shape(other)) => picture.opacity = other.opacity,
        (Kind::Shape(shape), Kind::Picture(other)) => {
            shape.opacity = other.opacity;
            shape.touch();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_are_packed_from_the_first_and_said_to_lie_in_a_row() {
        let boxes = [(300.0, 0.0, 50.0, 50.0), (0.0, 10.0, 100.0, 50.0), (150.0, 0.0, 40.0, 40.0)];
        assert_eq!(packed(&boxes, true), vec![-160.0, 0.0, -50.0]);
        assert!(in_a_row(&boxes));
        assert!(!in_a_row(&[(0.0, 0.0, 10.0, 10.0), (5.0, 100.0, 10.0, 10.0)]));
    }
}
