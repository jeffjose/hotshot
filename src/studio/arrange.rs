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
            // The page's own layers have no frame to be put to.
            Action::ToFrame(_) if self.on_page() => return,
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
    let radii = match &like.kind {
        Kind::Picture(picture) => picture.radii(),
        Kind::Shape(shape) => shape.radii(),
    };
    match &mut layer.kind {
        Kind::Picture(picture) => picture.set_radii(radii),
        Kind::Shape(shape) => shape.set_radii(radii),
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
            (picture.opacity, picture.adjust) = (other.opacity, other.adjust);
            if picture.chrome != other.chrome {
                picture.find_insets();
                picture.set_chrome(other.chrome.clone());
            }
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

impl Studio {
    /// Where a layer dragged in its list would go: the layer under the
    /// pointer, if it is over one of the list.
    pub(super) fn reorder_target(&mut self) -> Option<usize> {
        match self.control_at(self.pointer) {
            Some(Control::Layer(target) | Control::Eye(target) | Control::Lock(target)) => Some(target),
            _ => None,
        }
    }

    /// The row of a list the pointer is over, and where it is.
    fn row_here(&mut self) -> Option<(Rect, Control)> {
        let (x, y) = self.pointer;
        self.layout().controls.into_iter().find(|(rect, _)| rect.contains(x, y))
    }

    /// A layer dragged in its list let go: put where the layer under the
    /// pointer was, over it or under it, as one step to undo.
    /// Let go on the middle of a frame's row, it goes into that frame; on
    /// a row of the page's own layers, or by the top or the bottom of a
    /// frame's row, onto the page, just over or under that among
    /// everything on it, each where it was on the page, as Figma's list
    /// moves a layer.
    pub(super) fn drop_layer(&mut self, index: usize) {
        if index >= self.doc.layers.len() {
            return;
        }
        let Some((rect, control)) = self.row_here() else { return };
        let along = (self.pointer.1 - rect.y) as f32 / rect.h.max(1) as f32;
        let (over, middle) = (along < 0.5, (0.25..0.75).contains(&along));
        // What is typed into or cropped is known by where it is, which moves.
        let count = self.doc.layers.len();
        self.commit_text();
        self.cropping = None;
        if self.doc.layers.len() != count {
            return;
        }
        // Into a stack, along it or out of it, by the row it is let go on.
        let mut stepped = false;
        match self.stack_drop(index, control) {
            stack_rows::StackDrop::Into(into, by) => {
                self.checkpoint();
                return self.put_in_stack(index, into, by, over);
            }
            stack_rows::StackDrop::Out => {
                self.checkpoint();
                self.take_out_of_stack(index);
                stepped = true;
            }
            stack_rows::StackDrop::Nothing => {}
        }
        // Among the page's own things, by what it is let go by there.
        let by = match control {
            Control::Frame(frame) if middle => {
                if frame != self.active {
                    if !stepped {
                        self.checkpoint();
                    }
                    self.move_layers_to(frame, &[index]);
                } else if stepped {
                    self.changed();
                }
                return;
            }
            Control::Frame(frame) => Child::Frame(frame),
            Control::PageLayer(target) => Child::Layer(target),
            Control::Layer(target) | Control::Eye(target) | Control::Lock(target) if self.on_page() => Child::Layer(target),
            _ => return self.reorder_layer(index, stepped),
        };
        if !stepped {
            self.checkpoint();
        }
        // Out of a frame first, onto the page, the row let go by known by
        // what its layer is called, as the page's own layers grow by one.
        let by_id = match by {
            Child::Layer(target) => Some(stacks::layer_id(&self.loose().layers[target])),
            Child::Frame(_) => None,
        };
        let moved = match self.on_page() {
            true => index,
            false => {
                self.move_layers_to(LOOSE, &[index]);
                match self.selected[..] {
                    [moved] => moved,
                    _ => return,
                }
            }
        };
        let by = match by_id {
            Some(id) => match self.loose().layers.iter().position(|layer| stacks::layer_id(layer) == id) {
                Some(target) => Child::Layer(target),
                None => return,
            },
            None => by,
        };
        self.move_child(Child::Layer(moved), by, over);
    }

    /// A layer dragged in its list let go over another of the same frame:
    /// put where that one was; in the step to undo already begun, if one
    /// was, as it came out of a stack.
    fn reorder_layer(&mut self, index: usize, stepped: bool) {
        let target = self.reorder_target().filter(|&target| target != index && index < self.doc.layers.len());
        let Some(target) = target else {
            if stepped {
                self.changed();
            }
            return;
        };
        if !stepped {
            self.checkpoint();
        }
        let layer = self.doc.layers.remove(index);
        self.doc.layers.insert(target, layer);
        self.selected = vec![target];
        self.changed();
    }

    /// A frame dragged in the list let go by another frame's row or one of
    /// the page's own layers: put just over or under it among everything
    /// on the page, as one step to undo.
    pub(super) fn drop_frame(&mut self, frame: usize) {
        let Some((rect, control)) = self.row_here() else { return };
        let over = self.pointer.1 < rect.y + rect.h / 2;
        let by = match control {
            Control::Frame(other) if other != frame => Child::Frame(other),
            Control::PageLayer(target) => Child::Layer(target),
            Control::Layer(target) | Control::Eye(target) | Control::Lock(target) if self.on_page() => Child::Layer(target),
            _ => return,
        };
        self.checkpoint();
        self.move_child(Child::Frame(frame), by, over);
    }
}

impl Studio {
    /// Where what is dragged in the list would go by the row the pointer
    /// is on: 0 just over it, 1 into it or where it is, 2 just under it.
    pub(super) fn drop_mark(&self, rect: Rect, control: Control) -> Option<u8> {
        if !rect.contains(self.pointer.0, self.pointer.1) {
            return None;
        }
        let along = (self.pointer.1 - rect.y) as f32 / rect.h.max(1) as f32;
        let side = if along < 0.5 { 0 } else { 2 };
        let on_page = self.on_page();
        match (self.drag, control) {
            (Drag::Reorder { moved: true, .. }, Control::Frame(frame)) if (0.25..0.75).contains(&along) => (frame != self.active).then_some(1),
            (Drag::Reorder { moved: true, .. }, Control::Frame(_) | Control::PageLayer(_)) => Some(side),
            // Before or after something in a stack, or out of one, as well.
            (Drag::Reorder { moved: true, index, .. }, Control::Layer(target)) if target != index => {
                let stacked = |at: usize| self.doc.layers.get(at).is_some_and(|layer| self.stack_of(stacks::layer_id(layer)).is_some());
                Some(if on_page || stacked(index) || stacked(target) { side } else { 1 })
            }
            (Drag::Reorder { moved: true, .. }, Control::StackRow(_)) => Some(1),
            (Drag::ReorderFrame { moved: true, index, .. }, Control::Frame(frame)) if frame != index => Some(side),
            (Drag::ReorderFrame { moved: true, .. }, Control::PageLayer(_)) => Some(side),
            (Drag::ReorderFrame { moved: true, .. }, Control::Layer(_)) if on_page => Some(side),
            _ => None,
        }
    }

    /// The mark of where what is dragged in the list would go, on a row.
    pub(super) fn draw_drop_mark(&self, canvas: &mut Canvas, rect: Rect, control: Control) {
        match self.drop_mark(rect, control) {
            Some(1) => canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 6, 2, ACCENT),
            Some(0) => canvas.fill(rect.x, rect.y - 1, rect.w, 2, ACCENT),
            Some(_) => canvas.fill(rect.x, rect.bottom() - 1, rect.w, 2, ACCENT),
            None => {}
        }
    }
}
