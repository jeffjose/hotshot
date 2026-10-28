//! Style Mode: S with a shape, a callout or an arrow picked out opens a
//! grid of styles worked out from its own color and from whether what is
//! behind it is dark; a letter puts one on, hovering one shows it, and
//! Esc closes the grid.

use super::*;

/// The letters the styles are put on with, in the order of the grid.
pub(super) const STYLE_KEYS: [char; 10] = ['q', 'w', 'e', 'r', 't', 'a', 's', 'd', 'g', 'h'];

/// What each style is called.
pub(super) const STYLE_NAMES: [&str; 10] = ["Solid", "+ Light edge", "Outline", "Dashed", "Tint + edge", "+ Shadow", "Colored shadow", "Pill", "Glow", "By hand"];

/// The style that glows, which only shows on something dark.
pub(super) const GLOW: u8 = 8;

/// How large a tile of the grid is, and the room round them.
const TILE: (i32, i32) = (84, 52);
const GAP: i32 = 8;
const PAD: i32 = 12;

/// The color a shape is mostly seen in: a box's fill when it is filled
/// and has no border, otherwise its line, its letters, its pill.
pub(super) fn main_color(shape: &Shape) -> u32 {
    match shape.tool {
        Tool::Rect | Tool::Ellipse if shape.filled && !shape.border => shape.fill,
        _ => shape.color,
    }
}

/// A shape set in one of the styles, in a color, on something dark or
/// light: its fill, border, line, dashes and shadow as the style says,
/// whatever it had of them before.
pub(super) fn apply_style(shape: &mut Shape, style: u8, base: u32, dark: bool) {
    let light = crate::canvas::mix(base, 0xFFFFFF, 110);
    let tint = if dark { crate::canvas::mix(base, 0x111111, 170) } else { crate::canvas::mix(base, 0xFFFFFF, 190) };
    let lift = frame::Layer { x: 0, y: 6, blur: 16, spread: 0, color: 0x4000_0000 };
    let colored = frame::Layer { x: 0, y: 8, blur: 20, spread: 0, color: 0x9900_0000 | (base & 0xFF_FFFF) };
    let glow = frame::Layer { x: 0, y: 0, blur: 28, spread: 2, color: 0xCC00_0000 | (base & 0xFF_FFFF) };
    (shape.dashed, shape.dotted, shape.sketch, shape.shadow) = (false, false, false, None);
    shape.stroke = shape.stroke.max(2.0);
    match shape.tool {
        Tool::Rect | Tool::Ellipse => {
            shape.edge = shapes::CENTER;
            let solid = |shape: &mut Shape| (shape.filled, shape.fill, shape.border, shape.color) = (true, base, false, base);
            let outline = |shape: &mut Shape| (shape.filled, shape.border, shape.color) = (false, true, base);
            let tinted = |shape: &mut Shape| (shape.filled, shape.fill, shape.border, shape.color) = (true, tint, true, base);
            match style {
                0 => solid(shape),
                1 => (shape.filled, shape.fill, shape.border, shape.color) = (true, base, true, light),
                2 => outline(shape),
                3 => {
                    outline(shape);
                    shape.dashed = true;
                }
                4 => tinted(shape),
                5 => {
                    solid(shape);
                    shape.shadow = Some(lift);
                }
                6 => {
                    solid(shape);
                    shape.shadow = Some(colored);
                }
                7 => {
                    solid(shape);
                    let (_, _, w, h) = shape.frame();
                    shape.radius = w.min(h) / 2.0;
                }
                GLOW => {
                    tinted(shape);
                    shape.shadow = Some(glow);
                }
                _ => {
                    outline(shape);
                    shape.sketch = true;
                }
            }
        }
        Tool::Text | Tool::Badge => {
            shape.color = base;
            let boxed = matches!(style, 0 | 1 | 4 | 5 | 6 | 7 | GLOW);
            if shape.tool == Tool::Text {
                shape.style = if boxed { shape.style | shapes::BOXED } else { shape.style & !shapes::BOXED };
            }
            shape.shadow = match style {
                5 => Some(lift),
                6 => Some(colored),
                GLOW => Some(glow),
                _ => None,
            };
        }
        _ => {
            shape.color = base;
            shape.halo = matches!(style, 1 | 7);
            match style {
                3 => shape.dashed = true,
                4 => shape.dotted = true,
                5 | 7 => shape.shadow = Some(lift),
                6 => shape.shadow = Some(colored),
                GLOW => shape.shadow = Some(glow),
                9 => shape.sketch = true,
                _ => {}
            }
        }
    }
    shape.touch();
}

/// How light a color is, from 0 to 1.
fn lightness(color: u32) -> f32 {
    let channel = |shift: u32| (color >> shift & 0xFF) as f32 / 255.0;
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

impl Studio {
    /// Whether Style Mode can open: something it styles is picked out.
    pub(super) fn styles_apply(&self) -> bool {
        self.chosen().iter().any(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if !matches!(shape.tool, Tool::Blur | Tool::Spotlight | Tool::Zoom | Tool::Select)))
    }

    /// Whether what is behind a layer is dark: the picture under its
    /// middle, the topmost, or failing one the frame's background.
    pub(super) fn behind_is_dark(&mut self, index: usize) -> bool {
        let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
        let middle = (x + w / 2.0, y + h / 2.0);
        for under in (0..index).rev() {
            if let Kind::Picture(picture) = &self.doc.layers[under].kind {
                let (lx, ly) = picture.local(middle);
                if lx.abs() <= picture.w / 2.0 && ly.abs() <= picture.h / 2.0 {
                    let (cx, cy, cw, ch) = picture.crop;
                    let (u, v) = ((lx + picture.w / 2.0) / picture.w, (ly + picture.h / 2.0) / picture.h);
                    let (lw, lh, pixels) = &picture.image.levels[0];
                    let (px, py) = (((cx + u * cw) as i32).clamp(0, lw - 1), ((cy + v * ch) as i32).clamp(0, lh - 1));
                    return lightness(pixels[(py * lw + px) as usize]) < 0.5;
                }
            }
        }
        let ground = match self.doc.background {
            Fill::Color(color) => color,
            Fill::Gradient(from, to, _) => crate::canvas::mix(from, to, 128),
            Fill::Backdrop(which) => frame::shade(which, 0.5, 0.5, 0.5),
            Fill::None => 0xFFFFFF,
        };
        lightness(ground) < 0.5
    }

    /// The first shape picked out that styles apply to, and whether what
    /// is behind it is dark.
    fn styled(&mut self) -> Option<(Shape, bool)> {
        let index = self.chosen().into_iter().find(|&index| matches!(&self.doc.layers[index].kind, Kind::Shape(_)))?;
        let Kind::Shape(shape) = self.doc.layers[index].kind.clone() else { return None };
        let dark = self.behind_is_dark(index);
        Some((shape, dark))
    }

    /// A style put on each shape picked out, as one step to undo; glow
    /// only where what is behind is dark.
    pub(super) fn put_style(&mut self, style: u8) {
        let chosen = self.chosen();
        let darks: Vec<bool> = chosen.iter().map(|&index| self.behind_is_dark(index)).collect();
        if style == GLOW && !darks.iter().any(|&dark| dark) {
            return self.say("Glow shows on something dark");
        }
        self.checkpoint();
        for (&index, dark) in chosen.iter().zip(darks) {
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                let base = main_color(shape);
                apply_style(shape, style, base, dark);
            }
        }
        self.changed();
    }

    /// The grid of styles, beside what is picked out, if it is open.
    pub(super) fn layout_styles(&mut self, layout: &mut Layout) {
        if self.styling && !self.styles_apply() {
            self.styling = false;
        }
        if !self.styling {
            return;
        }
        let boxes: Vec<(f32, f32, f32, f32)> = self.chosen().iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((x, y, w, _)) = actions::union(&boxes) else { return };
        let (wide, high) = (PAD * 2 + TILE.0 * 5 + GAP * 4, PAD * 2 + 26 + (TILE.1 + 22) * 2 + 24);
        let (right, top) = (self.to_window((x + w, y)), self.to_window((x, y)));
        let mut place = Rect::new(right.0 + 16, top.1, wide, high);
        let room = self.open_area();
        if place.right() > room.right() {
            place.x = (top.0 - 16 - wide).max(room.x);
        }
        place.y = place.y.clamp(room.y, (room.bottom() - high).max(room.y));
        let mut front = Vec::new();
        for style in 0..10u8 {
            let (column, row) = (i32::from(style) % 5, i32::from(style) / 5);
            let tile = Rect::new(place.x + PAD + column * (TILE.0 + GAP), place.y + PAD + 26 + row * (TILE.1 + 22), TILE.0, TILE.1);
            front.push((tile, Control::Style(style)));
        }
        front.push((place, Control::Panel));
        front.append(&mut layout.controls);
        layout.controls = front;
        layout.styles = Some(place);
    }

    /// The heading and the foot of the grid.
    pub(super) fn draw_styles_frame(&mut self, canvas: &mut Canvas, place: Rect) {
        let kind = match self.styled() {
            Some((shape, _)) => match shape.tool {
                Tool::Rect => "box",
                Tool::Ellipse => "oval",
                Tool::Text | Tool::Badge => "callout",
                Tool::Arrow => "arrow",
                Tool::Line => "line",
                _ => "shape",
            },
            None => "shape",
        };
        let dark = self.styled().is_some_and(|(_, dark)| dark);
        let title = format!("Styles for this {kind}");
        self.text.draw(canvas, place.x + PAD, place.y + 10, 13, TEXT_COLOR, &title);
        let wide = self.text.width(&title, 13);
        self.text.draw(canvas, place.x + PAD + wide + 8, place.y + 12, 11, DIM_TEXT_COLOR, if dark { "on something dark" } else { "on something light" });
        let foot = place.bottom() - 22;
        canvas.fill(place.x + PAD, foot - 6, place.w - 2 * PAD, 1, EDGE_COLOR);
        self.text.draw(canvas, place.x + PAD, foot, 11, DIM_TEXT_COLOR, "Letters apply \u{b7} hover previews \u{b7} Esc closes");
    }

    /// A tile of the grid: the style drawn on a small one of the same kind,
    /// its letter, and what it is called under it.
    pub(super) fn draw_style_tile(&mut self, canvas: &mut Canvas, rect: Rect, style: u8, hovered: bool) {
        let Some((shape, dark)) = self.styled() else { return };
        let off = style == GLOW && !dark;
        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 8, if dark { 0x2A2A2E } else { 0xF2F2F2 }, 256);
        if hovered && !off {
            canvas.round_outline(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4, 9, 2, ACCENT);
        }
        let (cx, cy) = rect.center();
        let at = |x: i32, y: i32| (x as f32, y as f32);
        let mut sample = match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                let mut sample = Shape::new(shape.tool, shape.color, 2.0, at(cx - 28, cy - 14));
                sample.points[1] = at(cx + 28, cy + 14);
                sample.radius = 6.0;
                sample
            }
            Tool::Text | Tool::Badge => {
                let mut sample = Shape::new(Tool::Text, shape.color, 2.0, at(cx - 14, cy - 10));
                (sample.text, sample.font_px, sample.style) = ("Aa".into(), 15, shapes::BOLD);
                sample
            }
            _ => {
                let mut sample = Shape::new(Tool::Arrow, shape.color, 3.0, at(cx - 28, cy + 10));
                sample.points[1] = at(cx + 28, cy - 10);
                sample
            }
        };
        if !off {
            // Drawn on a sheet of the tile's own, opaque, and laid in it.
            apply_style(&mut sample, style, main_color(&shape), dark);
            sample.translate(-rect.x as f32, -rect.y as f32);
            let ground = 0xFF00_0000 | if dark { 0x2A2A2E } else { 0xF2F2F2 };
            let mut sheet = vec![ground; (rect.w * rect.h) as usize];
            draw_shape(&mut sheet, rect.w, rect.h, &sample, &mut self.text);
            for y in 0..rect.h {
                for x in 0..rect.w {
                    let pixel = sheet[(y * rect.w + x) as usize];
                    if pixel != ground {
                        canvas.blend(rect.x + x, rect.y + y, pixel & 0xFF_FFFF, 255);
                    }
                }
            }
        }
        let letter = STYLE_KEYS[usize::from(style)].to_ascii_uppercase().to_string();
        self.text.draw(canvas, rect.x + 6, rect.y + 4, 10, 0x8F8F8F, &letter);
        let name = STYLE_NAMES[usize::from(style)];
        let wide = self.text.width(name, 11);
        self.text.draw(canvas, cx - wide / 2, rect.bottom() + 5, 11, if off { 0x6A6A6A } else { TEXT_COLOR }, name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_style_sets_a_box_its_own_way_from_its_color() {
        let mut boxed = Shape::new(Tool::Rect, 0x30A46C, 3.0, (0.0, 0.0));
        boxed.points[1] = (100.0, 40.0);
        apply_style(&mut boxed, 0, 0x30A46C, true);
        assert!(boxed.filled && !boxed.border && boxed.fill == 0x30A46C && boxed.shadow.is_none());
        apply_style(&mut boxed, 3, 0x30A46C, true);
        assert!(!boxed.filled && boxed.border && boxed.dashed);
        apply_style(&mut boxed, 6, 0x30A46C, true);
        assert_eq!(boxed.shadow.map(|drop| drop.color & 0xFF_FFFF), Some(0x30A46C));
        assert!(!boxed.dashed, "a style starts over");
        apply_style(&mut boxed, 7, 0x30A46C, true);
        assert_eq!(boxed.radius, 20.0);
        apply_style(&mut boxed, 9, 0x30A46C, false);
        assert!(boxed.sketch && boxed.shadow.is_none());
        // An arrow takes what an arrow can.
        let mut arrow = Shape::new(Tool::Arrow, 0xE5484D, 3.0, (0.0, 0.0));
        apply_style(&mut arrow, 1, 0xE5484D, false);
        assert!(arrow.halo);
        apply_style(&mut arrow, 3, 0xE5484D, false);
        assert!(arrow.dashed && !arrow.halo);
        assert_eq!((lightness(0x000000), lightness(0xFFFFFF)), (0.0, 1.0));
    }
}
