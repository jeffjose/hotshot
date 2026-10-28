//! What the newer buttons of the panel, and the menus, do: lining up and
//! spacing, frames round things and frames of a size, how text is set, the
//! ends and paths of lines, shadows set number by number, export sizes,
//! crops kept to a shape, pages copied and done away with.

use super::*;

/// The shadow set number by number that looks like one of the presets,
/// to begin from: its last layer, the main one, in pixels of an artboard.
pub(super) fn custom_drop(preset: usize) -> frame::Layer {
    match preset {
        0 | CUSTOM_SHADOW => DROP,
        which => {
            let layers = frame::SHADOWS[(which - 1).min(frame::SHADOWS.len() - 1)].1;
            let main = layers[layers.len() - 1];
            let times = SHADOW_SIZE as i16;
            frame::Layer { x: main.x * times, y: main.y * times, blur: main.blur * times, spread: main.spread * times, color: main.color }
        }
    }
}

/// The shadow a shape is given when it is just given one.
pub(super) const SHAPE_DROP: frame::Layer = frame::Layer { x: 0, y: 6, blur: 16, spread: 0, color: 0x4000_0000 };

/// The box round some boxes.
pub(super) fn union(boxes: &[(f32, f32, f32, f32)]) -> Option<(f32, f32, f32, f32)> {
    let first = *boxes.first()?;
    let (mut left, mut top, mut right, mut bottom) = (first.0, first.1, first.0 + first.2, first.1 + first.3);
    for &(x, y, w, h) in boxes {
        (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x + w), bottom.max(y + h));
    }
    Some((left, top, right - left, bottom - top))
}

/// A shape mirrored across its middle, or down it.
fn flip_shape(shape: &mut Shape, across: bool) {
    if matches!(shape.tool, Tool::Text | Tool::Badge | Tool::Counter) {
        return;
    }
    let (mut low, mut high) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for point in &shape.points {
        (low, high) = ((low.0.min(point.0), low.1.min(point.1)), (high.0.max(point.0), high.1.max(point.1)));
    }
    let middle = ((low.0 + high.0) / 2.0, (low.1 + high.1) / 2.0);
    for point in &mut shape.points {
        if across {
            point.0 = 2.0 * middle.0 - point.0;
        } else {
            point.1 = 2.0 * middle.1 - point.1;
        }
    }
    shape.angle = (-shape.angle).rem_euclid(std::f32::consts::TAU);
    shape.touch();
}

impl Studio {
    /// The buttons the first match of `act` leaves to this.
    pub(super) fn act_more(&mut self, action: Action) -> Outcome {
        let set_style = |studio: &mut Studio, bit: u8| {
            studio.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.style ^= bit;
                }
            })
        };
        match action {
            Action::FrameThem => self.frame_them(),
            Action::ExportChosen => self.export_page(),
            Action::Pack(_) | Action::Match(_) | Action::ToFrame(_) | Action::Turn(_) => self.arrange(action),
            Action::LockRatio => self.keep_ratio = !self.keep_ratio,
            Action::Distribute(way) => self.distribute(way == 0),
            Action::Swap => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.heads.swap(0, 1);
                    shape.ends.swap(0, 1);
                }
            }),
            Action::Path(path) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.elbow = path == 2;
                    match path {
                        1 if shape.points.len() < 3 => {
                            // Bent a fifth of its length to one side.
                            let (a, b) = (shape.points[0], shape.points[1]);
                            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                            shape.bend_to(((a.0 + b.0) / 2.0 - dy * 0.2, (a.1 + b.1) / 2.0 + dx * 0.2));
                        }
                        1 => {}
                        _ => shape.points.truncate(2),
                    }
                }
            }),
            Action::Unstick(end) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.stuck[usize::from(end).min(1)] = None;
                }
            }),
            Action::AddExport => {
                self.checkpoint();
                let times = self.doc.exports.last().map_or(1.0, |size| (size.times + 1.0).min(4.0));
                let suffix = if times == 1.0 { String::new() } else { format!("@{times}x") };
                self.doc.exports.push(ExportSize { suffix, ..ExportSize::new(times) });
                self.changed();
            }
            // A copy of a size, under it.
            Action::DuplicateExport(index) => {
                if let Some(size) = self.doc.exports.get(usize::from(index)).cloned() {
                    self.checkpoint();
                    self.doc.exports.insert(usize::from(index) + 1, size);
                    self.changed();
                }
            }
            Action::RemoveExport(index) => {
                if usize::from(index) < self.doc.exports.len() {
                    self.checkpoint();
                    self.doc.exports.remove(usize::from(index));
                    self.changed();
                }
            }
            Action::Sideways => {
                let (w, h) = (self.doc.height, self.doc.width);
                self.resize_frame(w, h);
            }
            Action::Bold => set_style(self, shapes::BOLD),
            Action::Italic => set_style(self, shapes::ITALIC),
            Action::Highlight => set_style(self, shapes::HIGHLIGHT),
            Action::Bullets => set_style(self, shapes::BULLETS),
            Action::Underline => set_style(self, shapes::UNDERLINE),
            Action::Strike => set_style(self, shapes::STRIKE),
            Action::Upper => set_style(self, shapes::UPPER),
            Action::Pill(on) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.style = if on { shape.style | shapes::BOXED } else { shape.style & !shapes::BOXED };
                }
            }),
            Action::TextAlign(align) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.align = align.min(2);
                }
            }),
            Action::ShadowOn => self.restyle(|layer| match &mut layer.kind {
                Kind::Picture(picture) => picture.shadow = frame::SHADOWS.len(),
                Kind::Shape(shape) => shape.shadow = Some(SHAPE_DROP),
            }),
            Action::ShadowOff => self.restyle(|layer| match &mut layer.kind {
                Kind::Picture(picture) => picture.shadow = 0,
                Kind::Shape(shape) => shape.shadow = None,
            }),
            Action::FillOn => {
                let color = self.fill.unwrap_or(0xFFFFFF);
                self.paint(Paint::Fill, Some(color));
            }
            Action::FillOff => self.paint(Paint::Fill, None),
            Action::BorderOn | Action::BorderOff => {
                let on = action == Action::BorderOn;
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        // One without a border is filled, or there would
                        // be nothing of it.
                        shape.border = on || !shape.filled;
                    }
                });
            }
            Action::ResetAdjust => self.restyle(|layer| {
                if let Kind::Picture(picture) = &mut layer.kind {
                    picture.adjust = [0.0; 3];
                }
            }),
            Action::Aspect(which) => {
                self.crop_aspect = usize::from(which).min(panel::CROP_ASPECTS.len() - 1);
                self.keep_crop_to_aspect();
            }
            Action::ResetCrop => {
                let Some(index) = self.cropping else { return Outcome::Continue };
                self.checkpoint();
                if let Kind::Picture(picture) = &mut self.doc.layers[index].kind {
                    let per = picture.w / picture.crop.2.max(1.0);
                    let (iw, ih) = (picture.image.width() as f32, picture.image.height() as f32);
                    picture.crop = (0.0, 0.0, iw, ih);
                    (picture.w, picture.h) = (iw * per, ih * per);
                }
                self.changed();
            }
            Action::Corners => self.corners_open = !self.corners_open,
            other => self.act_effect(other),
        }
        self.dirty = true;
        Outcome::Continue
    }

    /// A shape mirrored, as `act` flips a picture.
    pub(super) fn flip_shapes(&mut self, across: bool) {
        self.restyle(|layer| {
            if let Kind::Shape(shape) = &mut layer.kind {
                flip_shape(shape, across);
            }
        });
    }

    /// The crop of the picture being cropped kept to the shape chosen for
    /// it: as large as it can be about its middle, in the picture.
    pub(super) fn keep_crop_to_aspect(&mut self) {
        let Some(index) = self.cropping else { return };
        let ratio = panel::CROP_ASPECTS[self.crop_aspect].1;
        if ratio == 0.0 {
            return;
        }
        self.checkpoint();
        if let Kind::Picture(picture) = &mut self.doc.layers[index].kind {
            let (iw, ih) = (picture.image.width() as f32, picture.image.height() as f32);
            let ratio = if ratio < 0.0 { iw / ih } else { ratio };
            let per = picture.w / picture.crop.2.max(1.0);
            let (cx, cy, cw, ch) = picture.crop;
            let middle = (cx + cw / 2.0, cy + ch / 2.0);
            let (mut w, mut h) = (cw, cw / ratio);
            if h > ch {
                (w, h) = (ch * ratio, ch);
            }
            let (w, h) = (w.min(iw), h.min(ih));
            let x = (middle.0 - w / 2.0).clamp(0.0, iw - w);
            let y = (middle.1 - h / 2.0).clamp(0.0, ih - h);
            picture.crop = (x, y, w, h);
            (picture.w, picture.h) = (w * per, h * per);
        }
        self.changed();
    }

    /// The frame worked in made a size, and fitted to the view.
    pub(super) fn resize_frame(&mut self, width: i32, height: i32) {
        self.checkpoint();
        (self.doc.width, self.doc.height) = (width.clamp(16, 8000), height.clamp(16, 8000));
        self.changed();
        self.fit();
    }

    /// The frame made as large as what is in it, and no larger.
    pub(super) fn fit_frame_to_content(&mut self) {
        let boxes: Vec<_> = (0..self.doc.layers.len()).filter(|&index| !self.doc.layers[index].hidden).map(|index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((x, y, w, h)) = union(&boxes) else { return self.say("Nothing in the frame to fit") };
        self.checkpoint();
        for layer in &mut self.doc.layers {
            layer.translate(-x.round(), -y.round());
        }
        (self.doc.x, self.doc.y) = (self.doc.x + x.round(), self.doc.y + y.round());
        (self.doc.width, self.doc.height) = ((w.ceil() as i32).max(16), (h.ceil() as i32).max(16));
        self.changed();
    }

    /// A new frame just round what is picked out, and them moved into it,
    /// where they were on the page.
    pub(super) fn frame_them(&mut self) {
        let mut chosen = self.chosen();
        let boxes: Vec<_> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((x, y, w, h)) = union(&boxes) else { return };
        self.checkpoint();
        chosen.sort_unstable();
        let mut moved = Vec::new();
        for &index in chosen.iter().rev() {
            let mut layer = self.doc.layers.remove(index);
            layer.translate(-x.round(), -y.round());
            moved.push(layer);
        }
        moved.reverse();
        let mut frame = Doc::new((w.ceil() as i32).max(16), (h.ceil() as i32).max(16));
        (frame.x, frame.y, frame.background) = (self.doc.x + x.round(), self.doc.y + y.round(), Fill::None);
        frame.name = format!("Frame {}", self.frame_count() + 1);
        frame.layers = moved;
        let count = frame.layers.len();
        self.pages[self.page].frames.push(frame);
        let index = self.frame_count() - 1;
        self.activate(index);
        self.selected = (0..count).collect();
        self.changed();
    }

    /// What is picked out spaced evenly, across or down: the first and the
    /// last where they are, the same gap between each and the next.
    pub(super) fn distribute(&mut self, across: bool) {
        let chosen = self.chosen();
        if chosen.len() < 3 {
            return self.say("Space three or more");
        }
        let mut boxes: Vec<(usize, (f32, f32, f32, f32))> = chosen.iter().map(|&index| (index, self.doc.layers[index].bounds(&mut self.text))).collect();
        let key = |b: &(f32, f32, f32, f32)| if across { b.0 } else { b.1 };
        let size = |b: &(f32, f32, f32, f32)| if across { b.2 } else { b.3 };
        boxes.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
        let (first, last) = (boxes[0].1, boxes[boxes.len() - 1].1);
        let span = key(&last) + size(&last) - key(&first);
        let taken: f32 = boxes.iter().map(|(_, b)| size(b)).sum();
        let gap = (span - taken) / (boxes.len() - 1) as f32;
        self.checkpoint();
        let mut at = key(&first);
        for (index, b) in boxes {
            let by = (at - key(&b)).round();
            if across {
                self.doc.layers[index].translate(by, 0.0);
            } else {
                self.doc.layers[index].translate(0.0, by);
            }
            at += size(&b) + gap;
        }
        self.changed();
    }

    /// What is picked out kept, to paste.
    pub(super) fn copy_layers(&mut self) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        self.copied = chosen.iter().map(|&index| self.doc.layers[index].clone()).collect();
        self.say(format!("Copied {} {}", self.copied.len(), if self.copied.len() == 1 { "layer" } else { "layers" }));
    }

    /// What was copied put in the frame worked in, a little down and to
    /// the right of where it was, each a copy of its own.
    pub(super) fn paste_layers(&mut self) {
        if self.copied.is_empty() {
            return;
        }
        self.checkpoint();
        self.selected.clear();
        for layer in self.copied.clone() {
            let mut layer = layer;
            match &mut layer.kind {
                Kind::Picture(picture) => picture.id = shapes::fresh_id(),
                Kind::Shape(shape) => *shape = shape.copied(),
            }
            layer.translate(16.0, 16.0);
            self.doc.layers.push(layer);
            self.selected.push(self.doc.layers.len() - 1);
        }
        self.changed();
    }

    /// The view made to fit what is picked out, or the frame.
    pub(super) fn zoom_to_selection(&mut self) {
        let boxes: Vec<_> = self.chosen().iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((x, y, w, h)) = union(&boxes) else { return self.fit() };
        let open = self.open_area();
        self.zoom = (open.w as f32 * 0.7 / w.max(1.0)).min(open.h as f32 * 0.7 / h.max(1.0)).clamp(0.02, 16.0);
        let middle = (open.x as f32 + open.w as f32 / 2.0, open.y as f32 + open.h as f32 / 2.0);
        self.pan = (middle.0 - (self.doc.x + x + w / 2.0) * self.zoom, middle.1 - (self.doc.y + y + h / 2.0) * self.zoom);
        self.dirty = true;
    }

    /// A copy of a page, after it.
    pub(super) fn duplicate_page(&mut self, page: usize) {
        if page >= self.pages.len() {
            return;
        }
        self.checkpoint();
        let mut copy = self.gathered()[page].clone();
        copy.name = format!("{} copy", copy.name);
        self.pages.insert(page + 1, copy);
        if self.page > page {
            self.page += 1;
        }
        self.changed();
    }

    /// A page done away with, where there are others.
    pub(super) fn delete_page(&mut self, page: usize) {
        if self.pages.len() < 2 || page >= self.pages.len() {
            return self.say("A file keeps one page");
        }
        self.checkpoint();
        let mut pages = self.gathered();
        pages.remove(page);
        let now = if self.page == page { page.min(pages.len() - 1) } else if self.page > page { self.page - 1 } else { self.page };
        let active = if self.page == page { 0 } else { self.active };
        self.restore(Snapshot { pages, page: now, active });
        self.changed();
        self.fit();
    }
}

impl Studio {
    /// Where a control is in the panel, as it is laid out now.
    pub(super) fn control_rect(&mut self, control: Control) -> Option<Rect> {
        self.layout().controls.iter().find(|(_, known)| *known == control).map(|(rect, _)| *rect)
    }

    /// A layer of the frame worked in, by what it is called.
    pub(super) fn layer_by_id(&self, id: u32) -> Option<usize> {
        self.doc.layers.iter().position(|layer| match &layer.kind {
            Kind::Picture(picture) => picture.id == id,
            Kind::Shape(shape) => shape.id == id,
        })
    }

    /// A slider set to where along it the pointer is.
    pub(super) fn slide_to(&mut self, field: Field, left: i32, wide: i32, x: i32) {
        let along = ((x - left) as f32 / wide as f32).clamp(0.0, 1.0);
        self.set_field(field, along * 200.0 - 100.0);
    }
}

impl Studio {
    /// A right click: on a page, what can be done with it; on the page of
    /// frames, on what is under it, picked out first, what can be done
    /// with that; on nothing, what can be done anyway.
    pub(super) fn right_click(&mut self, at: (i32, i32)) -> Outcome {
        // Whatever else is open closes, and what is being typed is kept.
        let mut control = self.control_at(at);
        self.commit_typing();
        self.commit_name();
        (self.menu, self.flyout, self.palette, self.styling, self.eyedropping) = (None, None, None, false, false);
        self.menu_row = None;
        self.dirty = true;
        // A row of the layers ends the typing of text, which may take a
        // row away: the row is found again after.
        if matches!(control, Some(Control::Layer(_) | Control::Eye(_) | Control::Lock(_) | Control::ColorLayer(..) | Control::Frame(_))) && self.editing.is_some() {
            self.commit_text();
            control = self.control_at(at);
        }
        // A row right clicked is picked out first, as Figma does, and its
        // menu opens.
        let kind = match control {
            Some(Control::Page(page)) => {
                self.go_to_page(page);
                Some(MenuKind::Page(page))
            }
            Some(Control::Layer(index) | Control::Eye(index) | Control::Lock(index) | Control::ColorLayer(index, _)) => {
                if !self.selected.contains(&index) {
                    self.selected = vec![index];
                }
                self.frame_picked = false;
                Some(MenuKind::Layer(index))
            }
            Some(Control::Frame(index)) => {
                self.activate(index);
                (self.frame_picked, self.tool, self.framing) = (true, Tool::Select, false);
                self.selected.clear();
                Some(MenuKind::Frame(index))
            }
            Some(Control::ColorField(Paint::Recolor(color)) | Control::ColorOpen(color)) => Some(MenuKind::Color(color)),
            // A size the frame is exported at, by any part of its row.
            Some(Control::Field(Field::ExportTimes(index)) | Control::Action(Action::RemoveExport(index))) => Some(MenuKind::Export(usize::from(index))),
            Some(Control::Entry(Naming::Suffix(index)) | Control::Menu(MenuKind::Format(index))) => Some(MenuKind::Export(index)),
            // Any other color: its hex, to copy.
            Some(Control::ColorField(paint)) => self.paint_color(paint).map(MenuKind::Hex),
            _ => None,
        };
        if let Some(kind) = kind {
            self.menu = Some(MenuState { kind, at, sub: None });
            return Outcome::Continue;
        }
        // Over the bar of tools, or anything else that floats on the page,
        // there is nothing to right click.
        if !self.over_page(at) || control.is_some() {
            return Outcome::Continue;
        }
        self.commit_text();
        if let Some(frame) = self.frame_at(self.to_page(at)).filter(|&frame| frame != self.active) {
            self.activate(frame);
        }
        let point = self.to_doc(at);
        match self.layer_at(point) {
            Some(index) if !self.selected.contains(&index) => self.selected = vec![index],
            Some(_) => {}
            None => self.selected.clear(),
        }
        self.menu = Some(MenuState { kind: MenuKind::Context, at, sub: None });
        self.dirty = true;
        Outcome::Continue
    }
}

impl Studio {
    /// What is picked out made wider or taller as a whole, to a width or
    /// a height, from its top left: each layer moved and sized with it,
    /// both ways when proportions are kept.
    pub(super) fn scale_selection(&mut self, across: bool, value: f32) {
        let chosen = self.chosen();
        let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((left, top, w, h)) = union(&boxes) else { return };
        let by = value.max(1.0) / if across { w } else { h }.max(1.0);
        let (sx, sy) = match (across, self.keep_ratio) {
            (_, true) => (by, by),
            (true, false) => (by, 1.0),
            (false, false) => (1.0, by),
        };
        for index in chosen {
            scale_layer(&mut self.doc.layers[index], (left, top), (sx, sy));
        }
    }
}

/// A layer made larger or smaller about a point, each way by its own
/// factor: a picture's middle and size; a shape's points, but that words
/// and counters are only moved.
pub(super) fn scale_layer(layer: &mut Layer, (ox, oy): (f32, f32), (sx, sy): (f32, f32)) {
    let to = |(x, y): (f32, f32)| (ox + (x - ox) * sx, oy + (y - oy) * sy);
    match &mut layer.kind {
        Kind::Picture(picture) => {
            (picture.cx, picture.cy) = to((picture.cx, picture.cy));
            (picture.w, picture.h) = ((picture.w * sx).max(4.0), (picture.h * sy).max(4.0));
        }
        Kind::Shape(shape) => {
            match shape.tool {
                Tool::Text | Tool::Badge | Tool::Counter => {
                    let (x, y) = shape.points[0];
                    let (nx, ny) = to((x, y));
                    shape.translate(nx - x, ny - y);
                }
                _ => shape.points.iter_mut().for_each(|point| *point = to(*point)),
            }
            shape.touch();
        }
    }
}
