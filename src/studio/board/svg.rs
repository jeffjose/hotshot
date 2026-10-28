//! An SVG dropped or pasted on a board comes in as shapes to change: its
//! boxes as boxes, its circles and ellipses as ovals, its lines, its
//! outlined paths and polylines as pen strokes, its words as text; what
//! cannot be made a shape (a filled path, a gradient, a picture in it, a
//! turned or skewed part) is drawn as one picture under them, by resvg.
//! Right-click Keep as picture takes it back to the one picture it was,
//! and Merge into picture makes anything picked out one picture.

use super::*;

/// An SVG brought in as shapes: its words, and what its shapes are called.
#[derive(Clone, Debug)]
pub(in crate::studio) struct Brought {
    pub source: String,
    pub ids: Vec<u32>,
    pub place: (f32, f32),
}

/// What is inherited down an SVG: fill, stroke, its width, the size of
/// its words, and where it is put.
#[derive(Clone, Copy)]
struct Style {
    fill: Option<u32>,
    stroke: Option<u32>,
    width: f32,
    font: f32,
    /// Scale across and down, and how far it is moved: no turn or skew.
    at: (f32, f32, f32, f32),
}

fn color_of(word: &str) -> Option<Option<u32>> {
    let word = word.trim();
    if word == "none" || word == "transparent" {
        return Some(None);
    }
    if word.starts_with("url(") {
        return None;
    }
    if word == "currentColor" {
        return Some(Some(0x000000));
    }
    let color: svgtypes::Color = word.parse().ok()?;
    Some(Some(u32::from(color.red) << 16 | u32::from(color.green) << 8 | u32::from(color.blue)))
}

/// An SVG's words read: whether it is one, by its root.
pub(in crate::studio) fn is_svg(words: &str) -> bool {
    let start = words.trim_start();
    (start.starts_with("<svg") || start.starts_with("<?xml")) && start.contains("<svg")
}

/// An SVG drawn as a picture, as large as it says it is.
pub(in crate::studio) fn render(source: &str) -> Option<Image> {
    let tree = resvg::usvg::Tree::from_str(source, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let (w, h) = (size.width().ceil().max(1.0) as u32, size.height().ceil().max(1.0) as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w.min(8192), h.min(8192))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let pixels = pixmap
        .data()
        .chunks_exact(4)
        .map(|rgba| {
            let a = u32::from(rgba[3]);
            let un = |c: u8| if a == 0 { 0 } else { (u32::from(c) * 255 / a).min(255) };
            a << 24 | un(rgba[0]) << 16 | un(rgba[1]) << 8 | un(rgba[2])
        })
        .collect();
    Some(Image::new(pixmap.width() as i32, pixmap.height() as i32, pixels))
}

/// What an SVG is made of as shapes, where they go on a board from its top
/// left; what of its words cannot be, as an SVG of its own; and how large
/// it is.
pub(in crate::studio) fn read(source: &str) -> Option<(Vec<Shape>, Option<String>, (f32, f32))> {
    let tree = roxmltree::Document::parse(source).ok()?;
    let root = tree.root_element();
    if root.tag_name().name() != "svg" {
        return None;
    }
    let number = |name: &str| root.attribute(name).and_then(|value| value.trim_end_matches("px").parse::<f32>().ok());
    let view: Option<Vec<f32>> = root.attribute("viewBox").map(|value| value.split([' ', ',']).filter(|word| !word.is_empty()).filter_map(|word| word.parse().ok()).collect());
    let (vx, vy, vw, vh) = match view.as_deref() {
        Some(&[x, y, w, h]) => (x, y, w, h),
        _ => (0.0, 0.0, number("width").unwrap_or(300.0), number("height").unwrap_or(150.0)),
    };
    let (w, h) = (number("width").unwrap_or(vw), number("height").unwrap_or(vh));
    let (sx, sy) = (w / vw.max(1e-3), h / vh.max(1e-3));
    let style = Style { fill: Some(0x000000), stroke: None, width: 1.0, font: 16.0, at: (sx, sy, -vx * sx, -vy * sy) };
    let mut shapes = Vec::new();
    let mut made = Vec::new();
    let mut rest = false;
    walk(root, style, &mut shapes, &mut made, &mut rest);
    // The rest, as an SVG of its own: the words without what is made.
    let others = rest.then(|| {
        let mut kept = source.to_owned();
        made.sort_by_key(|range: &std::ops::Range<usize>| std::cmp::Reverse(range.start));
        for range in made {
            kept.replace_range(range, "");
        }
        kept
    });
    Some((shapes, others, (w, h)))
}

/// An element's own style, over what it inherits.
fn styled(node: roxmltree::Node, mut style: Style) -> Option<Style> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for name in ["fill", "stroke", "stroke-width", "font-size"] {
        if let Some(value) = node.attribute(name) {
            pairs.push((name.to_owned(), value.to_owned()));
        }
    }
    if let Some(inline) = node.attribute("style") {
        for part in inline.split(';') {
            if let Some((key, value)) = part.split_once(':') {
                pairs.push((key.trim().to_owned(), value.trim().to_owned()));
            }
        }
    }
    for (key, value) in pairs {
        match key.as_str() {
            "fill" => style.fill = color_of(&value)?,
            "stroke" => style.stroke = color_of(&value)?,
            "stroke-width" => style.width = value.trim_end_matches("px").parse().unwrap_or(style.width),
            "font-size" => style.font = value.trim_end_matches("px").parse().unwrap_or(style.font),
            _ => {}
        }
    }
    if let Some(transform) = node.attribute("transform") {
        let t: svgtypes::Transform = transform.parse().ok()?;
        if t.b.abs() > 1e-6 || t.c.abs() > 1e-6 {
            return None;
        }
        let (sx, sy, tx, ty) = style.at;
        style.at = (sx * t.a as f32, sy * t.d as f32, tx + sx * t.e as f32, ty + sy * t.f as f32);
    }
    Some(style)
}

fn walk(node: roxmltree::Node, inherited: Style, shapes: &mut Vec<Shape>, made: &mut Vec<std::ops::Range<usize>>, rest: &mut bool) {
    for child in node.children().filter(roxmltree::Node::is_element) {
        let name = child.tag_name().name();
        if matches!(name, "defs" | "title" | "desc" | "metadata" | "style" | "linearGradient" | "radialGradient" | "clipPath" | "mask" | "filter" | "pattern" | "symbol" | "marker") {
            continue;
        }
        let drawable = matches!(name, "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" | "path" | "text" | "image" | "use" | "g" | "a" | "svg" | "switch" | "foreignObject");
        let clipped = child.attribute("clip-path").is_some() || child.attribute("mask").is_some() || child.attribute("filter").is_some();
        let Some(style) = styled(child, inherited).filter(|_| !clipped) else {
            *rest |= drawable;
            continue;
        };
        if matches!(name, "g" | "a") {
            walk(child, style, shapes, made, rest);
            continue;
        }
        match shape_of(child, style) {
            Some(shape) => {
                shapes.push(shape);
                made.push(child.range());
            }
            None => *rest |= drawable,
        }
    }
}

/// One element as a shape, if it can be one.
fn shape_of(node: roxmltree::Node, style: Style) -> Option<Shape> {
    let get = |name: &str| node.attribute(name).and_then(|value| value.trim_end_matches("px").parse::<f32>().ok());
    let (sx, sy, tx, ty) = style.at;
    let at = |x: f32, y: f32| (x * sx + tx, y * sy + ty);
    let width = style.width * (sx.abs() + sy.abs()) / 2.0;
    let dressed = |tool: Tool, color: u32| {
        let mut shape = Shape::new(tool, color, width.max(0.5), (0.0, 0.0));
        shape.face = 0;
        shape
    };
    match node.tag_name().name() {
        "rect" | "circle" | "ellipse" => {
            let (x0, y0, x1, y1) = match node.tag_name().name() {
                "rect" => (get("x").unwrap_or(0.0), get("y").unwrap_or(0.0), get("x").unwrap_or(0.0) + get("width")?, get("y").unwrap_or(0.0) + get("height")?),
                "circle" => {
                    let (cx, cy, r) = (get("cx").unwrap_or(0.0), get("cy").unwrap_or(0.0), get("r")?);
                    (cx - r, cy - r, cx + r, cy + r)
                }
                _ => {
                    let (cx, cy, rx, ry) = (get("cx").unwrap_or(0.0), get("cy").unwrap_or(0.0), get("rx")?, get("ry")?);
                    (cx - rx, cy - ry, cx + rx, cy + ry)
                }
            };
            if style.fill.is_none() && style.stroke.is_none() {
                return None;
            }
            let tool = if node.tag_name().name() == "rect" { Tool::Rect } else { Tool::Ellipse };
            let mut shape = dressed(tool, style.stroke.unwrap_or(0));
            shape.points = vec![at(x0, y0), at(x1, y1)];
            (shape.filled, shape.fill, shape.border) = (style.fill.is_some(), style.fill.unwrap_or(0xFFFFFF), style.stroke.is_some());
            shape.radius = get("rx").filter(|_| tool == Tool::Rect).unwrap_or(0.0) * sx;
            Some(shape)
        }
        "line" => {
            let color = style.stroke?;
            let mut shape = dressed(Tool::Line, color);
            shape.points = vec![at(get("x1").unwrap_or(0.0), get("y1").unwrap_or(0.0)), at(get("x2").unwrap_or(0.0), get("y2").unwrap_or(0.0))];
            Some(shape)
        }
        "polyline" | "polygon" | "path" => {
            // Outlines alone: a filled one is drawn as a picture.
            if style.fill.is_some() {
                return None;
            }
            let color = style.stroke?;
            let mut points: Vec<(f32, f32)> = Vec::new();
            if node.tag_name().name() == "path" {
                let mut last = (0.0, 0.0);
                let mut first = (0.0, 0.0);
                for segment in svgtypes::SimplifyingPathParser::from(node.attribute("d")?) {
                    match segment.ok()? {
                        svgtypes::SimplePathSegment::MoveTo { x, y } => {
                            // A second run of it is its own stroke: drawn as a picture.
                            if !points.is_empty() {
                                return None;
                            }
                            last = (x as f32, y as f32);
                            first = last;
                            points.push(at(last.0, last.1));
                        }
                        svgtypes::SimplePathSegment::LineTo { x, y } => {
                            last = (x as f32, y as f32);
                            points.push(at(last.0, last.1));
                        }
                        svgtypes::SimplePathSegment::CurveTo { x1, y1, x2, y2, x, y } => {
                            for step in 1..=16 {
                                let t = step as f32 / 16.0;
                                let u = 1.0 - t;
                                let p = |a: f32, b: f32, c: f32, d: f32| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
                                points.push(at(p(last.0, x1 as f32, x2 as f32, x as f32), p(last.1, y1 as f32, y2 as f32, y as f32)));
                            }
                            last = (x as f32, y as f32);
                        }
                        svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                            for step in 1..=12 {
                                let t = step as f32 / 12.0;
                                let u = 1.0 - t;
                                let p = |a: f32, b: f32, c: f32| u * u * a + 2.0 * u * t * b + t * t * c;
                                points.push(at(p(last.0, x1 as f32, x as f32), p(last.1, y1 as f32, y as f32)));
                            }
                            last = (x as f32, y as f32);
                        }
                        svgtypes::SimplePathSegment::ClosePath => {
                            points.push(at(first.0, first.1));
                            last = first;
                        }
                    }
                }
            } else {
                let numbers: Vec<f32> = node.attribute("points")?.split([' ', ',', '\n', '\t']).filter(|word| !word.is_empty()).filter_map(|word| word.parse().ok()).collect();
                points = numbers.chunks_exact(2).map(|pair| at(pair[0], pair[1])).collect();
                if node.tag_name().name() == "polygon" {
                    points.extend(points.first().copied());
                }
            }
            if points.len() < 2 {
                return None;
            }
            let mut shape = dressed(Tool::Pen, color);
            shape.points = points;
            Some(shape)
        }
        "text" => {
            let words: String = node.descendants().filter(roxmltree::Node::is_text).filter_map(|text| text.text()).collect::<Vec<_>>().join("");
            let words = words.trim();
            if words.is_empty() || node.children().any(|child| child.is_element() && child.has_attribute("x")) {
                return None;
            }
            let px = (style.font * sy.abs()).round().max(4.0) as u32;
            let mut shape = dressed(Tool::Text, style.fill.unwrap_or(0));
            shape.text = words.to_owned();
            shape.font_px = px;
            // An SVG's words stand on their baseline.
            let (x, y) = at(get("x").unwrap_or(0.0), get("y").unwrap_or(0.0));
            shape.points = vec![(x, y - px as f32 * 0.85)];
            Some(shape)
        }
        _ => None,
    }
}

impl Studio {
    /// An SVG put on the board, in the middle of a point of the window or
    /// of what is seen: as shapes, with what cannot be shapes as a picture
    /// under them, all picked out. One step to undo.
    pub(in crate::studio) fn bring_svg(&mut self, source: &str, name: &str, at: Option<(i32, i32)>) -> bool {
        let Some((shapes, others, (w, h))) = read(source) else { return false };
        self.commit_text();
        self.checkpoint();
        let middle = self.to_doc(at.unwrap_or_else(|| self.open_area().center()));
        let place = (middle.0 - w / 2.0, middle.1 - h / 2.0);
        let start = self.doc.layers.len();
        if let Some(image) = others.as_deref().and_then(render) {
            let mut picture = Picture::new(Arc::new(image), name.to_owned(), (middle.0, middle.1));
            (picture.w, picture.h) = (w, h);
            self.doc.layers.push(Layer::picture(picture));
        }
        for mut shape in shapes {
            shape.translate(place.0, place.1);
            self.doc.layers.push(Layer::shape(shape));
        }
        let ids: Vec<u32> = (start..self.doc.layers.len()).map(|index| self.id_of(index)).collect();
        self.selected = (start..self.doc.layers.len()).collect();
        self.board.svg = Some(Brought { source: source.to_owned(), ids, place });
        self.changed();
        self.say(match others {
            Some(_) => "The SVG as shapes; what could not be, a picture under them",
            None => "The SVG as shapes",
        });
        true
    }

    /// Keep as picture: the SVG last brought in, its shapes put back as
    /// the one picture it was.
    pub(in crate::studio) fn keep_svg_picture(&mut self) {
        let Some(brought) = self.board.svg.take() else { return };
        let Some(image) = render(&brought.source) else { return self.say("The SVG could not be drawn") };
        self.checkpoint();
        let (w, h) = (image.width() as f32, image.height() as f32);
        self.doc.layers.retain(|layer| !brought.ids.contains(&stacks::layer_id(layer)));
        let picture = Picture::new(Arc::new(image), "SVG".into(), (brought.place.0 + w / 2.0, brought.place.1 + h / 2.0));
        self.doc.layers.push(Layer::picture(picture));
        self.selected = vec![self.doc.layers.len() - 1];
        self.changed();
    }

    /// Merge into picture: what is picked out drawn as one picture, at
    /// twice its size to stay sharp, in its place.
    pub(in crate::studio) fn merge_picture(&mut self) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return;
        }
        let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
        let Some((x, y, w, h)) = actions::union(&boxes) else { return };
        let mut sheet = Doc::new(w.ceil() as i32, h.ceil() as i32);
        sheet.background = Fill::None;
        sheet.layers = chosen.iter().map(|&index| {
            let mut layer = self.doc.layers[index].clone();
            layer.translate(-x, -y);
            layer
        }).collect();
        let (pw, ph, pixels) = sheet.render(2.0, &mut self.text);
        self.checkpoint();
        let at = *chosen.iter().min().unwrap_or(&0);
        let mut sorted = chosen.clone();
        sorted.sort_unstable();
        for index in sorted.into_iter().rev() {
            self.doc.layers.remove(index);
        }
        let mut picture = Picture::new(Arc::new(Image::new(pw, ph, pixels)), "Merged".into(), (x + w / 2.0, y + h / 2.0));
        (picture.w, picture.h) = (w, h);
        self.doc.layers.insert(at.min(self.doc.layers.len()), Layer::picture(picture));
        self.selected = vec![at.min(self.doc.layers.len() - 1)];
        self.board.svg = None;
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    const DRAWING: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 400 200">
  <rect x="10" y="10" width="100" height="60" rx="8" fill="#D0EBFF" stroke="#1E1E1E" stroke-width="2"/>
  <g transform="translate(200 0)"><circle cx="50" cy="50" r="40" fill="none" stroke="red"/></g>
  <line x1="0" y1="150" x2="400" y2="150" stroke="black"/>
  <path d="M 10 180 C 50 120 100 240 150 180" fill="none" stroke="#2F9E44" stroke-width="4"/>
  <text x="300" y="120" font-size="20" fill="#333">Hello</text>
  <path d="M 300 20 L 380 20 L 340 80 Z" fill="#F08C00"/>
</svg>"##;

    #[test]
    fn an_svg_comes_in_as_shapes_the_rest_a_picture_and_goes_back_to_one() {
        let mut studio = board();
        studio.pasted(None, Some(DRAWING.to_owned()));
        let tools: Vec<Tool> = studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) => Some(shape.tool),
            Kind::Picture(_) => None,
        }).collect();
        assert_eq!(tools, [Tool::Rect, Tool::Ellipse, Tool::Line, Tool::Pen, Tool::Text]);
        assert!(matches!(studio.doc.layers[0].kind, Kind::Picture(_)), "the filled triangle, as a picture under them");
        // At half its size, as its width says against its viewBox.
        let rect = studio.doc.layers.iter().find_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.tool == Tool::Rect => Some(shape.clone()),
            _ => None,
        }).unwrap();
        let (_, _, w, h) = rect.frame();
        assert_eq!((w, h, rect.fill, rect.border), (50.0, 30.0, 0xD0EBFF, true));
        // Its words, changeable.
        assert!(studio.doc.layers.iter().any(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "Hello")));
        // Kept as the picture it was, from its right click.
        let at = studio.to_window_page((studio.to_doc(studio.open_area().center()).0, studio.to_doc(studio.open_area().center()).1));
        studio.right_click(at);
        let row = studio.menu_items(MenuKind::Context).iter().position(|item| matches!(item, menu::Item::Entry { says, .. } if says == "Keep as picture"));
        assert!(row.is_some());
        studio.menu = None;
        studio.board_act(Act::KeepPicture);
        assert_eq!(studio.doc.layers.len(), 1);
        assert!(matches!(&studio.doc.layers[0].kind, Kind::Picture(picture) if picture.w == 200.0));
        // Anything made one picture.
        let mut other = board();
        other.pasted(None, Some(DRAWING.to_owned()));
        other.selected = (0..other.doc.layers.len()).collect();
        other.board_act(Act::MergePicture);
        assert_eq!(other.doc.layers.len(), 1);
    }
}
