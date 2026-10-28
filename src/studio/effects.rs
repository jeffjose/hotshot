//! Blurs and spotlights on the canvas: layers that draw nothing of their
//! own, but change what is under them in the frame. Each works on what
//! the frame shows
//! beneath it at the size it is drawn at, pictures and layers alike, so
//! that a frame exported at twice the size is blurred from pixels drawn
//! at twice the size too, and a part of the frame drawn on its own comes
//! out as it does in the whole.

use super::*;

/// Whether a tool's layers change what is beneath them, rather than
/// draw over it.
pub(super) fn changes_beneath(tool: Tool) -> bool {
    matches!(tool, Tool::Blur | Tool::Spotlight)
}

/// The two ways a blur hides what is under it: a mosaic of squares, as
/// capture mode's does, or smoothly, as frosted glass does. Kept in the
/// shape's `style`, which a blur has no other use for.
pub(super) const MOSAIC: u8 = 0;
pub(super) const SMOOTH: u8 = 1;

/// How strong a blur is, in pixels of the frame: how large the squares
/// of a mosaic are, or how far a smooth blur reaches. Capture mode's,
/// from its width of line.
pub(super) fn strength(shape: &Shape) -> f32 {
    (shape.stroke * 3.0 + 4.0).max(4.0)
}

/// The width of line that gives a blur a strength.
pub(super) fn stroke_for(strength: f32) -> f32 {
    (strength.clamp(4.0, 244.0) - 4.0) / 3.0
}

/// How much a spotlight darkens what it is not on, in percent: kept in
/// its `number`, which a spotlight has no other use for, none there
/// meaning capture mode's, which keeps 96 of 256 of the brightness.
pub(super) fn dim(shape: &Shape) -> u32 {
    match shape.number {
        1..=100 => shape.number,
        _ => 63,
    }
}

/// What a spotlight is on: its box, how round its corners are, and
/// whether it is the oval in its box instead (kept in `filled`).
struct Lit {
    frame: (f32, f32, f32, f32),
    radius: f32,
    round: bool,
}

impl Lit {
    /// How much of a point it is on, with an edge a pixel soft.
    fn cover(&self, x: f32, y: f32) -> f32 {
        let (left, top, w, h) = self.frame;
        let (hw, hh) = (w / 2.0, h / 2.0);
        let (dx, dy) = ((x - left - hw).abs(), (y - top - hh).abs());
        if dx > hw + 1.0 || dy > hh + 1.0 {
            return 0.0;
        }
        let away = match self.round {
            true => {
                let (rx, ry) = (hw.max(0.5), hh.max(0.5));
                ((dx / rx).hypot(dy / ry) - 1.0) * rx.min(ry)
            }
            false => {
                let radius = self.radius.clamp(0.0, hw.min(hh));
                let (qx, qy) = (dx - hw + radius, dy - hh + radius);
                qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
            }
        };
        (0.5 - away).clamp(0.0, 1.0)
    }
}

/// A pixel with black laid over it, `amount` of it: what is see-through
/// becomes as dark as what is not.
fn darkened(pixel: u32, amount: f32) -> u32 {
    let alpha = (pixel >> 24) as f32 / 255.0;
    let shown = alpha + amount * (1.0 - alpha);
    if shown <= 0.0 {
        return pixel;
    }
    let keep = alpha * (1.0 - amount) / shown;
    let channel = |shift: u32| (((pixel >> shift & 255) as f32 * keep).round() as u32).min(255) << shift;
    ((shown * 255.0).round() as u32).min(255) << 24 | channel(16) | channel(8) | channel(0)
}

impl Doc {
    /// A layer that changes what is under it, laid over the part of the
    /// frame drawn so far, `times` its size.
    pub(super) fn change_beneath(&self, index: usize, out: &mut [u32], part: Rect, times: f32, text: &mut Text) {
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.layers.get(index) else { return };
        let shape = if times == 1.0 { shape.clone() } else { scaled(shape, times) };
        let whole = Rect::new(0, 0, ((self.width as f32 * times).round() as i32).max(1), ((self.height as f32 * times).round() as i32).max(1));
        match shape.tool {
            Tool::Blur => self.blur(index, &shape, out, (part, whole, times), text),
            Tool::Spotlight => self.spotlight(index, out, part, times),
            _ => {}
        }
    }

    /// The spotlights: all that is under the topmost of them darkened but
    /// what any of them is on, once, as capture mode does, so that two
    /// spotlights do not each darken what the other is on. The topmost
    /// says how dark.
    fn spotlight(&self, index: usize, out: &mut [u32], part: Rect, times: f32) {
        let lit: Vec<(usize, Shape)> = self.layers.iter().enumerate().filter_map(|(at, layer)| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Spotlight && !layer.hidden => Some((at, if times == 1.0 { shape.clone() } else { scaled(shape, times) })),
            _ => None,
        }).collect();
        let Some((top, shape)) = lit.last() else { return };
        if *top != index {
            return;
        }
        let dark = dim(shape) as f32 / 100.0;
        let shapes: Vec<Lit> = lit.iter().map(|(_, shape)| Lit { frame: shape.frame(), radius: shape.radius, round: shape.filled }).collect();
        out.par_chunks_mut(part.w.max(1) as usize).enumerate().for_each(|(row, pixels)| {
            let y = (part.y + row as i32) as f32 + 0.5;
            for (column, pixel) in pixels.iter_mut().enumerate() {
                let x = (part.x + column as i32) as f32 + 0.5;
                let inside = shapes.iter().map(|lit| lit.cover(x, y)).fold(0.0f32, f32::max);
                let amount = dark * (1.0 - inside);
                if amount > 0.0 {
                    *pixel = darkened(*pixel, amount);
                }
            }
        });
    }

    /// What is beneath a layer in a rectangle of the frame as drawn: from
    /// the part drawn so far, if it holds all of it, or drawn afresh.
    fn beneath(&self, index: usize, out: &[u32], part: Rect, want: Rect, times: f32, text: &mut Text) -> Vec<u32> {
        match part.intersect(want) == want {
            true => canvas::crop(out, part.w, want.moved(-part.x, -part.y)),
            false => self.render_under(index, times, want, text),
        }
    }

    /// A blur: what is under its box, as a mosaic or smoothly. A mosaic's
    /// squares are counted from the corner of its box, and a smooth blur
    /// takes in what is round the box too, so that any part of it drawn
    /// on its own is as it is in the whole.
    fn blur(&self, index: usize, shape: &Shape, out: &mut [u32], (part, whole, times): (Rect, Rect, f32), text: &mut Text) {
        let area = shape.frame_rect().intersect(whole);
        let seen = area.intersect(part);
        if seen.is_empty() {
            return;
        }
        let reach = strength(shape);
        match shape.style {
            SMOOTH => {
                // Three passes of a box each way are near enough a
                // gaussian, and each reaches its radius further.
                let radius = ((reach * 0.6).round() as i32).max(1);
                let want = seen.inflate(3 * radius).intersect(whole);
                let mut planes = premultiplied(&self.beneath(index, out, part, want, times, text));
                for plane in &mut planes {
                    for _ in 0..3 {
                        *plane = box_pass(plane, want.w as usize, want.h as usize, radius as usize, true);
                    }
                    for _ in 0..3 {
                        *plane = box_pass(plane, want.w as usize, want.h as usize, radius as usize, false);
                    }
                }
                for y in seen.y..seen.bottom() {
                    for x in seen.x..seen.right() {
                        let at = ((y - want.y) * want.w + x - want.x) as usize;
                        out[((y - part.y) * part.w + x - part.x) as usize] = unpremultiplied([planes[0][at], planes[1][at], planes[2][at], planes[3][at]]);
                    }
                }
            }
            _ => {
                let cell = (reach.round() as i32).max(2);
                let left = area.x + (seen.x - area.x) / cell * cell;
                let top = area.y + (seen.y - area.y) / cell * cell;
                let right = (area.x + (seen.right() - area.x + cell - 1) / cell * cell).min(area.right());
                let bottom = (area.y + (seen.bottom() - area.y + cell - 1) / cell * cell).min(area.bottom());
                let want = Rect::new(left, top, right - left, bottom - top);
                let under = self.beneath(index, out, part, want, times, text);
                for square_y in (top..bottom).step_by(cell as usize) {
                    for square_x in (left..right).step_by(cell as usize) {
                        let square = Rect::new(square_x, square_y, cell, cell).intersect(want);
                        let mut sum = [0i64; 4];
                        for y in square.y..square.bottom() {
                            for x in square.x..square.right() {
                                let pixel = premultiply(under[((y - want.y) * want.w + x - want.x) as usize]);
                                (0..4).for_each(|channel| sum[channel] += pixel[channel]);
                            }
                        }
                        let count = i64::from((square.w * square.h).max(1));
                        let color = unpremultiplied(sum.map(|value| (value + count / 2) / count));
                        let shown = square.intersect(seen);
                        for y in shown.y..shown.bottom() {
                            let row = ((y - part.y) * part.w) as usize;
                            out[row + (shown.x - part.x) as usize..row + (shown.right() - part.x) as usize].fill(color);
                        }
                    }
                }
            }
        }
    }
}

impl Studio {
    /// The panel with a blur picked out: where it is and how large, and
    /// how it hides what is under it.
    pub(super) fn effect_sections(&mut self, rows: &mut panel::Rows, shape: &Shape) {
        let act = Control::Action;
        rows.section(tool_name(shape.tool), "over what is under it", None);
        rows.fields(&[Field::X, Field::Y], &[]);
        rows.fields(&[Field::W, Field::H], &[act(Action::LockRatio)]);
        if shape.tool == Tool::Blur {
            rows.section("Style", "", None);
            rows.chips(&mut self.text, [MOSAIC, SMOOTH].map(|style| (Action::BlurStyle(style).says().to_owned(), act(Action::BlurStyle(style)))).to_vec());
            rows.fields(&[Field::Strength], &[]);
        }
        if shape.tool == Tool::Spotlight {
            rows.section("Shape", "", None);
            rows.chips(&mut self.text, [false, true].map(|round| (Action::Round(round).says().to_owned(), act(Action::Round(round)))).to_vec());
            match shape.filled {
                true => rows.fields(&[Field::Dim], &[]),
                false => rows.fields(&[Field::Dim, Field::Radius], &[]),
            }
            rows.note("The rest of the frame under it is darkened");
        }
    }

    /// The buttons of the panel for a blur and the like.
    pub(super) fn act_effect(&mut self, action: Action) {
        match action {
            Action::BlurStyle(style) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    if shape.tool == Tool::Blur {
                        shape.style = style;
                    }
                }
            }),
            // A round spotlight is filled; a round zoom is not, as capture
            // mode has them.
            Action::Round(round) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    match shape.tool {
                        Tool::Spotlight => shape.filled = round,
                        Tool::Zoom => shape.filled = !round,
                        _ => {}
                    }
                }
            }),
            _ => {}
        }
    }
}

/// A pixel as how opaque it is, out of 65535, and its colors times how
/// opaque it is out of 255. In whole numbers, so that sums of them come
/// out the same whichever pixel they were begun from.
fn premultiply(pixel: u32) -> [i64; 4] {
    let alpha = i64::from((pixel >> 24) as u8);
    [alpha * 257, i64::from((pixel >> 16) as u8) * alpha, i64::from((pixel >> 8) as u8) * alpha, i64::from(pixel as u8) * alpha]
}

/// A picture as four planes: how opaque, and red, green and blue times that.
fn premultiplied(pixels: &[u32]) -> [Vec<i64>; 4] {
    let mut planes: [Vec<i64>; 4] = std::array::from_fn(|_| Vec::with_capacity(pixels.len()));
    for &pixel in pixels {
        let channels = premultiply(pixel);
        (0..4).for_each(|channel| planes[channel].push(channels[channel]));
    }
    planes
}

/// A pixel back from how opaque it is and its colors times that.
fn unpremultiplied([alpha, red, green, blue]: [i64; 4]) -> u32 {
    if alpha < 129 {
        return 0;
    }
    let channel = |value: i64| ((value * 257 + alpha / 2) / alpha).clamp(0, 255) as u32;
    (((alpha + 128) / 257).clamp(0, 255) as u32) << 24 | channel(red) << 16 | channel(green) << 8 | channel(blue)
}

/// Each value the average of those within a radius of it along its row,
/// or down its column: past the ends, the value at the end.
fn box_pass(values: &[i64], w: usize, h: usize, radius: usize, across: bool) -> Vec<i64> {
    let (count, lines, stride, step) = if across { (w, h, 1, w) } else { (h, w, w, 1) };
    let span = (2 * radius + 1) as i64;
    let mut out = vec![0; values.len()];
    if count == 0 {
        return out;
    }
    for line in 0..lines {
        let at = |i: isize| values[line * step + i.clamp(0, count as isize - 1) as usize * stride];
        let mut sum: i64 = (-(radius as isize)..=radius as isize).map(at).sum();
        for i in 0..count {
            out[line * step + i * stride] = (sum + span / 2) / span;
            sum += at(i as isize + radius as isize + 1) - at(i as isize - radius as isize);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> Studio {
        Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into())
    }

    /// Red and blue squares ten pixels across.
    fn checker(width: i32, height: i32) -> Image {
        Image::new(width, height, (0..width * height).map(|at| if (at % width / 10 + at / width / 10) % 2 == 0 { 0xFFFF0000 } else { 0xFF0000FF }).collect())
    }

    /// A frame of red and blue squares with a blur over part of it.
    fn blurred(style: u8) -> Doc {
        let mut doc = Doc::new(200, 200);
        doc.layers.push(Layer::picture(Picture::new(Arc::new(checker(200, 200)), "squares".into(), (100.0, 100.0))));
        let mut blur = Shape::new(Tool::Blur, 0, stroke_for(20.0), (40.0, 40.0));
        (blur.points[1], blur.style) = ((160.0, 160.0), style);
        doc.layers.push(Layer::shape(blur));
        doc
    }

    /// A press, a drag and a let go on the artboard.
    fn drag(studio: &mut Studio, from: (f32, f32), to: (f32, f32)) {
        let (a, b) = (studio.to_window(from), studio.to_window(to));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
    }

    /// A click on a control.
    fn press(studio: &mut Studio, control: Control) {
        let at = studio.control_rect(control).unwrap_or_else(|| panic!("no {control:?}")).center();
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
    }

    #[test]
    fn a_blur_makes_a_mosaic_or_a_smooth_wash_of_what_is_under_it_and_nothing_else() {
        let mut text = Text::load();
        let (_, _, mosaic) = blurred(MOSAIC).render(1.0, &mut text);
        // Outside it, the squares as they were; inside, squares of twenty
        // each one color, the red and blue under it averaged.
        assert_eq!((mosaic[10 * 200 + 10], mosaic[10 * 200 + 25]), (0xFFFF0000, 0xFF0000FF));
        let cell = mosaic[45 * 200 + 45];
        assert!((40..60).all(|y| (40..60).all(|x| mosaic[y * 200 + x] == cell)), "one square is one color");
        assert!((cell >> 16 & 255).abs_diff(cell & 255) < 8 && cell >> 24 == 255, "{cell:08X}");
        let (_, _, smooth) = blurred(SMOOTH).render(1.0, &mut text);
        assert_eq!(smooth[10 * 200 + 10], 0xFFFF0000);
        let middle = smooth[100 * 200 + 104];
        assert!((60..200).contains(&(middle >> 16 & 255)) && (60..200).contains(&(middle & 255)), "red and blue run together: {middle:08X}");
    }

    #[test]
    fn a_blur_drawn_a_part_at_a_time_and_larger_is_as_it_is_whole() {
        let mut text = Text::load();
        for style in [MOSAIC, SMOOTH] {
            let doc = blurred(style);
            let (wide, _, whole) = doc.render(2.0, &mut text);
            assert_eq!(wide, 400);
            // A part cut through the middle of the blur, as the window
            // draws only what it shows.
            let part = Rect::new(150, 130, 120, 90);
            let drawn = doc.render_part(2.0, part, &mut text);
            for y in 0..part.h {
                for x in 0..part.w {
                    assert_eq!(drawn[(y * part.w + x) as usize], whole[((part.y + y) * 400 + part.x + x) as usize], "style {style} at {x},{y}");
                }
            }
            // At twice the size, a mosaic's squares are twice as large.
            if style == MOSAIC {
                assert_eq!(whole[81 * 400 + 81], whole[119 * 400 + 119]);
            }
        }
    }

    #[test]
    fn b_or_the_more_tools_button_takes_up_the_blur_and_the_panel_sets_it() {
        let mut studio = studio();
        studio.add_picture(checker(200, 200), "squares");
        // The button opens its list, which has the blur.
        press(&mut studio, Control::Group(MORE));
        assert_eq!(studio.flyout, Some(MORE));
        press(&mut studio, Control::Pick(Tool::Blur));
        assert_eq!((studio.tool, studio.flyout, studio.groups), (Tool::Blur, None, [Tool::Rect, Tool::Pen]));
        // V puts it down, and B takes it up again without the list.
        studio.key(Key::Char('v'), Mods::default(), true);
        studio.key(Key::Char('b'), Mods::default(), true);
        assert_eq!(studio.tool, Tool::Blur);
        let middle = (studio.doc.width as f32 / 2.0, studio.doc.height as f32 / 2.0);
        drag(&mut studio, (middle.0 - 50.0, middle.1 - 40.0), (middle.0 + 50.0, middle.1 + 40.0));
        assert_eq!((studio.doc.layers.len(), studio.selected.clone(), studio.tool), (2, vec![1], Tool::Select));
        assert_eq!(studio.doc.layers[1].name(), "Blur");
        assert_eq!(studio.handles().len(), 8);
        // Smooth, from its chip; and stronger, from its number.
        press(&mut studio, Control::Action(Action::BlurStyle(SMOOTH)));
        studio.set_field(Field::Strength, 40.0);
        let Kind::Shape(shape) = &studio.doc.layers[1].kind else { panic!() };
        assert_eq!((shape.style, strength(shape).round()), (SMOOTH, 40.0));
        assert!(studio.colors_of(1).is_empty());
        // And it is kept in the file.
        let back = read_pages(&write_pages(&studio.gathered())).expect("it reads back");
        let Kind::Shape(kept) = &back[0].frames[0].layers[1].kind else { panic!() };
        assert_eq!((kept.tool, kept.style), (Tool::Blur, SMOOTH));
    }

    /// A spotlight from one corner of the frame to another.
    fn spotlight(from: (f32, f32), to: (f32, f32)) -> Shape {
        let mut lit = Shape::new(Tool::Spotlight, 0, 2.0, from);
        (lit.points[1], lit.number) = (to, 50);
        lit
    }

    #[test]
    fn spotlights_darken_the_rest_of_the_frame_once_and_not_what_is_over_them() {
        let mut text = Text::load();
        let mut doc = Doc::new(200, 200);
        doc.layers.push(Layer::shape(spotlight((20.0, 20.0), (80.0, 80.0))));
        let mut round = spotlight((120.0, 120.0), (180.0, 180.0));
        round.filled = true;
        doc.layers.push(Layer::shape(round));
        // A box over them, which they leave as it is.
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 2.0, (150.0, 10.0));
        (boxed.points[1], boxed.filled, boxed.fill) = ((190.0, 50.0), true, 0xE5484D);
        doc.layers.push(Layer::shape(boxed));
        let (_, _, out) = doc.render(1.0, &mut text);
        // The white frame half as bright outside both, and white in each.
        assert_eq!(out[100 * 200 + 5], 0xFF808080);
        assert_eq!((out[50 * 200 + 50], out[150 * 200 + 150]), (0xFFFFFFFF, 0xFFFFFFFF));
        // The corner of the round one's box is outside its oval.
        assert_eq!(out[122 * 200 + 122], 0xFF808080);
        assert_eq!(out[30 * 200 + 170], 0xFFE5484D);
        // And at twice the size, a part as the whole.
        let (_, _, whole) = doc.render(2.0, &mut text);
        let part = Rect::new(100, 100, 200, 150);
        let drawn = doc.render_part(2.0, part, &mut text);
        assert!((0..part.h).all(|y| (0..part.w).all(|x| drawn[(y * part.w + x) as usize] == whole[((part.y + y) * 400 + part.x + x) as usize])));
    }

    #[test]
    fn d_draws_a_spotlight_the_panel_rounds_and_dims_and_one_from_capture_mode_goes_under_the_rest() {
        let mut studio = studio();
        studio.key(Key::Char('d'), Mods::default(), true);
        assert_eq!(studio.tool, Tool::Spotlight);
        drag(&mut studio, (100.0, 100.0), (300.0, 250.0));
        assert_eq!((studio.selected.clone(), studio.doc.layers[0].name()), (vec![0], "Spotlight".to_owned()));
        assert_eq!(studio.field(Field::Dim), Some(63.0));
        press(&mut studio, Control::Action(Action::Round(true)));
        studio.set_field(Field::Dim, 80.0);
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((shape.filled, dim(shape)), (true, 80));
        let (_, _, out) = studio.doc.render(1.0, &mut studio.text);
        assert_eq!((out[10 * 800 + 10], out[175 * 800 + 200]), (0xFF333333, 0xFFFFFFFF));
        // From capture mode, a spotlight drawn after an arrow is put under it.
        let mut arrow = Shape::new(Tool::Arrow, 0xE5484D, 3.5, (10.0, 10.0));
        arrow.points[1] = (60.0, 60.0);
        let lit = spotlight((20.0, 20.0), (50.0, 50.0));
        let doc = from_markup(Rect::new(0, 0, 100, 100), &vec![0x123456; 100 * 100], &format!("{}\n{}", arrow.write(), lit.write()));
        let names: Vec<String> = doc.layers.iter().map(Layer::name).collect();
        assert_eq!(names, ["Screenshot", "Spotlight", "Arrow"]);
    }
}
