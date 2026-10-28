//! What a box or an oval is filled with, as Figma has it: one color, a
//! linear gradient across it at an angle, or a radial one out from its
//! middle; or nothing. The frame's background is chosen the same way.

use super::*;
use crate::shapes::Gradient;

/// The way a linear gradient goes to begin with: down, as Figma's does.
const DOWN: f32 = 180.0;

impl Studio {
    /// The one box or oval picked out, if one is.
    pub(super) fn picked_fillable(&self) -> Option<&Shape> {
        match &self.doc.layers.get(self.picked()?)?.kind {
            Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse) => Some(shape),
            _ => None,
        }
    }

    /// Which of the kinds of fill a box or an oval has: one color, linear
    /// or radial, 0, 1 or 2; none when it is not filled.
    pub(super) fn fill_kind(shape: &Shape) -> Option<u8> {
        shape.filled.then(|| match shape.gradient {
            None => 0,
            Some(gradient) if gradient.radial => 2,
            Some(_) => 1,
        })
    }

    /// The boxes and ovals picked out filled another kind of way, their
    /// colors kept; and the next drawn the same.
    pub(super) fn set_fill_kind(&mut self, kind: u8) {
        let remembered = self.gradient;
        let first = self.picked_fillable().map(|shape| (shape.fill, shape.gradient));
        let gradient = |fill: u32, now: Option<Gradient>| -> Option<Gradient> {
            let was = now.or(remembered);
            match kind {
                0 => None,
                _ => Some(Gradient {
                    // From its color to a paler one of it, to begin with.
                    to: was.map_or(canvas::mix(fill, 0xFFFFFF, 170), |was| was.to),
                    angle: was.map_or(DOWN, |was| was.angle),
                    radial: kind == 2,
                }),
            }
        };
        self.restyle(|layer| {
            if let Kind::Shape(shape) = &mut layer.kind {
                if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                    shape.gradient = gradient(shape.fill, shape.gradient);
                    shape.filled = true;
                }
            }
        });
        let fill = first.map_or(self.fill.unwrap_or(DEFAULT_FILL), |(fill, _)| fill);
        self.gradient = gradient(fill, first.and_then(|(_, gradient)| gradient));
        if self.fill.is_none() {
            self.fill = Some(fill);
        }
        // One color has no end for the colors to be open at.
        if kind == 0 && self.palette == Some(Paint::FillTo) {
            self.palette = Some(Paint::Fill);
        }
        self.keep_shape_style();
    }

    /// How the next box or oval is drawn kept for the next time the canvas
    /// opens, in `canvas-shapes` among hotshot's state: its fill or a dash,
    /// whether it has a border, the border's color, and its gradient or a
    /// dash. Only for a file of the user's, not while a change is only
    /// shown, and so never from the tests.
    pub(super) fn keep_shape_style(&self) {
        if self.path.is_none() || self.preview.is_some() {
            return;
        }
        let Some(file) = crate::output::state_file("canvas-shapes") else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let fill = self.fill.map_or("-".to_owned(), |fill| format!("{fill:06X}"));
        let _ = std::fs::write(file, format!("{fill} {} {:06X} {}\n", u8::from(self.border), self.color, shapes::gradient_word(self.gradient)));
    }

    /// How the next box or oval is drawn, as it was kept.
    pub(super) fn recall_shape_style(&mut self) {
        let kept = crate::output::state_file("canvas-shapes").and_then(|file| std::fs::read_to_string(file).ok());
        if let Some(kept) = kept {
            self.read_shape_style(&kept);
        }
    }

    pub(super) fn read_shape_style(&mut self, kept: &str) {
        let words: Vec<&str> = kept.split_whitespace().collect();
        let [fill, border, color, gradient] = words[..] else { return };
        let (Ok(color), fill) = (u32::from_str_radix(color, 16), u32::from_str_radix(fill, 16).ok()) else { return };
        (self.fill, self.border, self.color, self.gradient) = (fill.map(|fill| fill & 0xFF_FFFF), border == "1", color & 0xFF_FFFF, shapes::gradient_read(gradient));
    }

    /// The color the fill's gradient goes to, for what is picked out and
    /// what is drawn next.
    pub(super) fn paint_fill_to(&mut self, color: Option<u32>) {
        let Some(color) = color else { return };
        if let Some(gradient) = &mut self.gradient {
            gradient.to = color;
        }
        self.keep_shape_style();
        self.restyle(|layer| {
            if let Kind::Shape(shape) = &mut layer.kind {
                if let Some(gradient) = &mut shape.gradient {
                    gradient.to = color;
                }
            }
        });
    }

    /// The colors the fill of the one picked out goes from and to, when
    /// it is a gradient.
    pub(super) fn fill_stops(&self) -> Option<(u32, u32)> {
        let shape = self.picked_fillable().filter(|shape| shape.filled)?;
        shape.gradient.map(|gradient| (shape.fill, gradient.to))
    }

    /// Which way the fill's gradient goes, in degrees, for its field.
    pub(super) fn fill_angle(&self) -> Option<f32> {
        self.picked_fillable()?.gradient.filter(|gradient| !gradient.radial).map(|gradient| gradient.angle)
    }

    pub(super) fn set_fill_angle(&mut self, value: f32) {
        let angle = value.round().rem_euclid(360.0);
        for index in self.chosen() {
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                if let Some(gradient) = &mut shape.gradient {
                    gradient.angle = angle;
                    shape.touch();
                }
            }
        }
        if let Some(gradient) = &mut self.gradient {
            gradient.angle = angle;
        }
    }

    /// The strip of the fill's gradient pressed: the stop nearer opened.
    pub(super) fn press_fill_bar(&mut self) {
        let rect = self.control_rect(Control::FillBar).unwrap_or_default();
        let paint = if self.pointer.0 < rect.x + rect.w / 2 { Paint::Fill } else { Paint::FillTo };
        self.open_picker(paint);
    }

    /// A strip from one color to another, a stop at each end, ringed, the
    /// one whose color is open lit: a frame's background's, or a fill's.
    pub(super) fn draw_stops(&self, canvas: &mut Canvas, rect: Rect, (from, to): (u32, u32), paints: (Paint, Paint)) {
        let inner = rect.inflate(-1);
        for x in 0..inner.w {
            let along = (x * 256 / inner.w.max(1)) as u32;
            canvas.fill(inner.x + x, inner.y + 3, 1, inner.h - 6, canvas::mix(from, to, along));
        }
        for (x, paint, color) in [(rect.x + 1, paints.0, from), (rect.right() - 13, paints.1, to)] {
            let lit = self.palette == Some(paint);
            canvas.round_fill(x - 1, rect.y - 1, 14, rect.h + 2, 4, if lit { ACCENT } else { 0xFFFFFF }, 256);
            canvas.round_fill(x + 2, rect.y + 2, 8, rect.h - 4, 2, color, 256);
        }
    }

    /// The kinds of fill along the top of the colors, as Figma's has them,
    /// for a fill or a frame's background: as chips, from `y`, `wide`
    /// across. Gives how far down they go. None for any other color.
    pub(super) fn picker_kinds(&mut self, paint: Paint, (left, y): (i32, i32), wide: i32, front: &mut Vec<(Rect, Control)>, words: &mut Vec<((i32, i32), String, u32, u32)>) -> i32 {
        let kinds: Vec<(&str, Control)> = match paint {
            Paint::Fill | Paint::FillTo if self.picked_fillable().is_some() => (0..3).map(|kind| (Action::FillKind(kind).says(), Control::Action(Action::FillKind(kind)))).collect(),
            Paint::Background | Paint::From | Paint::To => vec![("Solid", Control::Ground(Ground::Color)), ("Linear", Control::Ground(Ground::Gradient)), ("Radial", Control::Ground(Ground::Radial))],
            _ => return 0,
        };
        let each = (wide - 4 * (kinds.len() as i32 - 1)) / kinds.len() as i32;
        let line = Text::line_height(12);
        for (at, (says, control)) in kinds.into_iter().enumerate() {
            let place = Rect::new(left + at as i32 * (each + 4), y, each, panel::FIELD_H);
            front.push((place, control));
            let says_wide = self.text.width(says, 12);
            words.push(((place.x + (each - says_wide) / 2, y + (panel::FIELD_H - line) / 2), says.to_owned(), TEXT_COLOR, 12));
        }
        panel::FIELD_H + 10
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> Studio {
        Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into())
    }

    fn boxed(tool: Tool) -> Shape {
        let mut shape = Shape::new(tool, 0x000000, 2.0, (20.0, 20.0));
        (shape.points[1], shape.filled, shape.fill, shape.border) = ((180.0, 180.0), true, 0x000000, false);
        shape
    }

    fn red(pixel: u32) -> u32 {
        pixel >> 16 & 0xFF
    }

    #[test]
    fn a_gradient_fills_a_box_down_it_or_out_from_its_middle_at_any_size_and_turned() {
        let mut text = Text::load();
        let mut doc = Doc::new(200, 200);
        let mut shape = boxed(Tool::Rect);
        shape.gradient = Some(Gradient { to: 0xFFFFFF, angle: 180.0, radial: false });
        doc.layers.push(Layer::shape(shape.clone()));
        for times in [1.0, 3.0] {
            let (wide, _, out) = doc.render(times, &mut text);
            let at = |x: f32, y: f32| red(out[((y * times) as i32 * wide + (x * times) as i32) as usize]);
            // Dark at the top, light at the bottom, half way in the middle.
            assert!(at(100.0, 22.0) < 10 && at(100.0, 178.0) > 245, "at {times}");
            assert!((at(100.0, 100.0) as i32 - 128).abs() < 6 && (at(30.0, 100.0) as i32 - at(170.0, 100.0) as i32).abs() <= 1, "at {times}: {} {} {}", at(100.0, 100.0), at(30.0, 100.0), at(170.0, 100.0));
        }
        // Turned a quarter, it goes across instead: from the right.
        shape.angle = std::f32::consts::FRAC_PI_2;
        doc.layers = vec![Layer::shape(shape.clone())];
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(red(out[100 * 200 + 175]) < 20 && red(out[100 * 200 + 25]) > 235);
        // Round: dark in the middle, light at the edge of the oval.
        let mut oval = boxed(Tool::Ellipse);
        oval.gradient = Some(Gradient { to: 0xFFFFFF, angle: 0.0, radial: true });
        doc.layers = vec![Layer::shape(oval.clone())];
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(red(out[100 * 200 + 100]) < 5 && red(out[100 * 200 + 176]) > 235);
        // Kept in its line; a line from before is one color.
        let back = Shape::read(&oval.write()).unwrap();
        assert_eq!(back.gradient, oval.gradient);
        assert_eq!(Shape::read(&boxed(Tool::Rect).write()).unwrap().gradient, None);
        let old = "rect\t3E63DD\t2\t1\t8\t16\t0\t00\t0,256,0,0,0\t3E63DD\t0\t20,20 180,180\t";
        assert_eq!(Shape::read(old).unwrap().gradient, None);
    }

    #[test]
    fn the_fill_is_made_linear_or_radial_from_the_panel_or_the_top_of_its_colors() {
        let mut studio = studio();
        let mut shape = boxed(Tool::Rect);
        shape.fill = 0x3E63DD;
        studio.doc.layers.push(Layer::shape(shape));
        studio.selected = vec![0];
        studio.act(Action::FillKind(1));
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        let gradient = shape.gradient.expect("a gradient");
        assert!(!gradient.radial && gradient.angle == 180.0 && shape.fill == 0x3E63DD);
        assert!(studio.control_rect(Control::FillBar).is_some() && studio.control_rect(Control::Field(Field::FillAngle)).is_some());
        // Its end colored, and turned, from the panel.
        studio.open_picker(Paint::FillTo);
        studio.paint(Paint::FillTo, Some(0xD6409F));
        studio.set_field(Field::FillAngle, 90.0);
        assert_eq!(studio.fill_stops(), Some((0x3E63DD, 0xD6409F)));
        assert_eq!(studio.field(Field::FillAngle), Some(90.0));
        // The colors have the kinds along their top; Radial keeps the two.
        let radial = studio.control_rect(Control::Action(Action::FillKind(2))).expect("Radial at the top of the colors");
        assert!(radial.y < studio.control_rect(Control::Picker(picker::SQUARE)).unwrap().y);
        let at = radial.center();
        studio.pointer_moved(at.0, at.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.released();
        assert_eq!(studio.palette, Some(Paint::FillTo));
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert!(shape.gradient.is_some_and(|gradient| gradient.radial && gradient.to == 0xD6409F));
        assert!(studio.control_rect(Control::Field(Field::FillAngle)).is_none());
        // And Solid takes it back to its one color, the colors open on it.
        studio.press_control(Control::Action(Action::FillKind(0)), false);
        assert_eq!(studio.palette, Some(Paint::Fill));
        let Kind::Shape(shape) = &studio.doc.layers[0].kind else { panic!() };
        assert_eq!((shape.gradient, shape.fill), (None, 0x3E63DD));
        // The next box drawn is filled as the last was set.
        studio.act(Action::FillKind(1));
        studio.selected.clear();
        studio.set_tool(Tool::Rect);
        let (a, b) = (studio.to_window((300.0, 300.0)), studio.to_window((400.0, 380.0)));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
        let Kind::Shape(drawn) = &studio.doc.layers.last().unwrap().kind else { panic!() };
        assert!(drawn.filled && drawn.gradient.is_some_and(|gradient| !gradient.radial));
    }

    #[test]
    fn a_frame_is_filled_out_from_its_middle_too_and_keeps_it_in_its_file() {
        let mut text = Text::load();
        let mut doc = Doc::new(100, 100);
        doc.background = Fill::Radial(0x000000, 0xFFFFFF);
        let (_, _, out) = doc.render(1.0, &mut text);
        assert!(red(out[50 * 100 + 50]) < 5 && red(out[50 * 100 + 1]) > 240);
        assert_eq!(Doc::read(&doc.write()).unwrap().background, Fill::Radial(0x000000, 0xFFFFFF));
        // From the panel, a linear one's colors kept.
        let mut studio = studio();
        studio.doc.background = Fill::Gradient(0x112233, 0x445566, 90.0);
        studio.press_control(Control::Ground(Ground::Radial), false);
        assert_eq!(studio.doc.background, Fill::Radial(0x112233, 0x445566));
        assert!(studio.control_rect(Control::Field(Field::Angle)).is_none());
    }

    #[test]
    fn a_box_is_filled_with_no_border_to_begin_with_and_after_as_the_last_was_set() {
        let mut studio = studio();
        let draw = |studio: &mut Studio| {
            studio.selected.clear();
            studio.set_tool(Tool::Ellipse);
            let (a, b) = (studio.to_window((100.0, 100.0)), studio.to_window((200.0, 180.0)));
            studio.pointer_moved(a.0, a.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            studio.pointer_moved(b.0, b.1, Mods::default());
            studio.released();
            let Kind::Shape(shape) = studio.doc.layers.last().unwrap().kind.clone() else { panic!() };
            (shape.filled, shape.fill, shape.border)
        };
        assert_eq!(draw(&mut studio), (true, DEFAULT_FILL, false));
        // A border given it, the next has one too; its fill taken away, the
        // next is an outline.
        studio.act(Action::BorderOn);
        assert_eq!(draw(&mut studio), (true, DEFAULT_FILL, true));
        studio.act(Action::FillOff);
        assert!(matches!(draw(&mut studio), (false, _, true)));
        // As kept for the next time the canvas opens.
        studio.read_shape_style("FFC53D 0 E5484D 3E63DD:90:l\n");
        assert_eq!((studio.fill, studio.border, studio.color), (Some(0xFFC53D), false, 0xE5484D));
        assert_eq!(studio.gradient, Some(Gradient { to: 0x3E63DD, angle: 90.0, radial: false }));
        studio.read_shape_style("- 1 000000 -");
        assert_eq!((studio.fill, studio.border, studio.gradient), (None, true, None));
    }
}

/// What a box or an oval drawn on the canvas is filled with to begin with,
/// as Figma fills its shapes rather than outlining them: Radix's indigo 8.
/// Its paler steps wash out on a white frame; this one reads as a color on
/// white and on a picture, and dark words still read over it.
pub(super) const DEFAULT_FILL: u32 = 0x8DA4EF;
