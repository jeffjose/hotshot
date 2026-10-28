//! Exporting a board, or part of it: all of it, a frame drawn round part
//! of it, or what is picked out, as PNG, AVIF or JPEG XL as frames are,
//! and as SVG, since a board is all shapes: each drawn as its hand drew
//! it, its words as words, a picture in it as a picture.

use super::*;
use crate::output::Format;

/// The kinds a board exports as, by number: PNG, AVIF, JPEG XL, and SVG.
pub(in crate::studio) const KINDS: [&str; 4] = ["PNG", "AVIF", "JPEG XL", "SVG"];

/// Room left round what is exported of a board.
const MARGIN: f32 = 32.0;

/// Bytes as base64, for a picture inside an SVG.
fn base64(bytes: &[u8]) -> String {
    const LETTERS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = u32::from(chunk[0]) << 16 | u32::from(*chunk.get(1).unwrap_or(&0)) << 8 | u32::from(*chunk.get(2).unwrap_or(&0));
        for at in 0..4 {
            out.push(if at <= chunk.len() { LETTERS[(n >> (18 - 6 * at) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn hex(color: u32) -> String {
    format!("#{:06X}", color & 0xFF_FFFF)
}

/// Words made safe to put in an SVG.
fn escaped(words: &str) -> String {
    words.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A run of points as an SVG path's data.
fn path_of(points: &[(f32, f32)], closed: bool) -> String {
    let mut data = String::new();
    for (at, (x, y)) in points.iter().enumerate() {
        data += &format!("{}{:.1} {:.1} ", if at == 0 { "M" } else { "L" }, x, y);
    }
    if closed {
        data += "Z";
    }
    data
}

impl Studio {
    /// What is exported, and the part of the board it is in: what is
    /// picked out; with a frame worked in, everything in its place; or
    /// all of the board.
    pub(in crate::studio) fn export_part(&mut self) -> Option<(Vec<Layer>, (f32, f32, f32, f32), String)> {
        let chosen = self.chosen();
        if !self.on_page() && self.on_board() {
            let (x, y, w, h) = (self.doc.x, self.doc.y, self.doc.width as f32, self.doc.height as f32);
            // The board there, and over it what is in the frame itself,
            // as it was in Design.
            let mut layers: Vec<Layer> = self.loose().layers.iter().filter(|layer| !layer.hidden).cloned().collect();
            layers.extend(self.doc.layers.iter().filter(|layer| !layer.hidden).map(|layer| {
                let mut layer = layer.clone();
                layer.translate(x, y);
                layer
            }));
            return Some((layers, (x, y, w, h), self.doc.name.clone()));
        }
        let (layers, name): (Vec<Layer>, String) = match chosen.is_empty() {
            true => (self.doc.layers.iter().filter(|layer| !layer.hidden).cloned().collect(), self.pages[self.page].name.clone()),
            false => {
                // What is in what is picked out goes with it.
                let mut ids: Vec<u32> = chosen.iter().map(|&index| self.id_of(index)).collect();
                for index in &chosen {
                    ids.extend(self.inside(self.id_of(*index)));
                }
                (self.doc.layers.iter().filter(|layer| ids.contains(&stacks::layer_id(layer))).cloned().collect(), "Selection".to_owned())
            }
        };
        let boxes: Vec<(f32, f32, f32, f32)> = layers.iter().map(|layer| layer.bounds(&mut self.text)).collect();
        let (x, y, w, h) = actions::union(&boxes)?;
        Some((layers, (x - MARGIN, y - MARGIN, w + 2.0 * MARGIN, h + 2.0 * MARGIN), name))
    }

    /// Some layers as a frame of their own, a part of the board in size,
    /// on its paper on a board and on nothing elsewhere.
    pub(in crate::studio) fn part_sheet(&self, layers: &[Layer], (x, y, w, h): (f32, f32, f32, f32)) -> Doc {
        let mut sheet = Doc::new(w.ceil().max(1.0) as i32, h.ceil().max(1.0) as i32);
        sheet.background = if self.on_board() { Fill::Color(self.board_ground()) } else { Fill::None };
        sheet.layers = layers.iter().map(|layer| {
            let mut layer = layer.clone();
            layer.translate(-x, -y);
            layer
        }).collect();
        sheet.board = self.loose().board;
        sheet
    }

    /// The board's paper, as exports have it.
    pub(in crate::studio) fn board_ground(&self) -> u32 {
        self.paper().0
    }

    /// The board, a frame of it, or what is picked out, written: as a
    /// picture of a kind, twice its size to stay sharp; or as an SVG.
    pub(in crate::studio) fn export_board(&mut self, kind: u8) {
        let Some((layers, part, name)) = self.export_part() else { return self.say("Nothing to export") };
        let dir = self.export_dir();
        if let Err(error) = std::fs::create_dir_all(&dir) {
            return self.say(format!("Could not make {}: {error}", dir.display()));
        }
        let stem = one_line(&name).replace('/', "-");
        let (bytes, ending) = match kind {
            3 => (self.board_svg(&layers, part).into_bytes(), "svg"),
            _ => {
                let format = [Format::Png, Format::Avif, Format::Jxl][usize::from(kind.min(2))];
                let sheet = self.part_sheet(&layers, part);
                let times = if self.on_board() { 2.0 } else { 1.0 };
                let (w, h, pixels) = sheet.render(times, &mut self.text);
                let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
                match crate::convert::encode(w as u32, h as u32, &pixels, clear, None, format, &self.convert) {
                    Ok(encoded) => (encoded.bytes, format.ending()),
                    Err(error) => return self.say(error),
                }
            }
        };
        let path = dir.join(format!("{stem}.{ending}"));
        match std::fs::write(&path, bytes) {
            Ok(()) => self.show_exported(vec![path]),
            Err(error) => self.say(format!("Could not write {}: {error}", path.display())),
        }
    }

    /// Some layers of the board as an SVG, the part given its whole: each
    /// shape as its hand drew it, words as words, the rest as pictures.
    pub(in crate::studio) fn board_svg(&mut self, layers: &[Layer], (x, y, w, h): (f32, f32, f32, f32)) -> String {
        let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"{x:.1} {y:.1} {w:.1} {h:.1}\">\n");
        if self.on_board() {
            out += &format!("  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"{}\"/>\n", hex(self.board_ground()));
        }
        for layer in layers {
            match &layer.kind {
                Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow | Tool::Pen | Tool::Text) && shape.angle == 0.0 => out += &self.shape_svg(shape),
                _ => {
                    // Anything else drawn as it is, as a picture in its place.
                    let (bx, by, bw, bh) = layer.bounds(&mut self.text);
                    let sheet = self.part_sheet(std::slice::from_ref(layer), (bx, by, bw, bh));
                    let (pw, ph, pixels) = sheet.render(2.0, &mut self.text);
                    if let Ok(png) = crate::output::encode(pw as u32, ph as u32, &pixels, true) {
                        out += &format!("  <image x=\"{bx:.1}\" y=\"{by:.1}\" width=\"{bw:.1}\" height=\"{bh:.1}\" href=\"data:image/png;base64,{}\"/>\n", base64(&png));
                    }
                }
            }
        }
        out += "</svg>\n";
        out
    }

    /// One shape as SVG.
    fn shape_svg(&mut self, shape: &Shape) -> String {
        let mut out = String::new();
        let opacity = shape.opacity as f32 / 256.0;
        let ink = hex(shape.color);
        // A stroke, laid down as strongly as it is drawn.
        let stroke = |points: &[(f32, f32)], width: f32, strength: f32| format!("  <path d=\"{}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"{width:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" opacity=\"{:.2}\"/>\n", path_of(points, false), opacity * strength);
        let line = |points: &[(f32, f32)], width: f32| stroke(points, width, 1.0);
        let (x, y, w, h) = shape.frame();
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                let forms = shape.has_form();
                let filled = shape.filled && !shape.placeholder();
                if filled {
                    let fill = hex(shape.fill);
                    out += &match (forms, shape.tool) {
                        (true, _) => shape.form_paths().0.iter().map(|path| format!("  <path d=\"{}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", path_of(path, true))).collect::<String>(),
                        (false, Tool::Ellipse) => format!("  <ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0),
                        _ => match shape.sketch_fill() {
                            Some(path) => format!("  <path d=\"{}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", path_of(&path, true)),
                            None => format!("  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"{:.1}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", if shape.sketch { 0.0 } else { shape.radius }),
                        },
                    };
                }
                if shape.border || shape.placeholder() {
                    if shape.sketch && !shape.placeholder() {
                        let strokes = if forms { shape.form_strokes() } else { shape.sketch_strokes() };
                        for (points, half, strength) in strokes {
                            out += &stroke(&points, half * 2.0, strength);
                        }
                    } else {
                        let dash = if shape.placeholder() || shape.dashed { " stroke-dasharray=\"8 6\"" } else { "" };
                        let common = format!("fill=\"none\" stroke=\"{ink}\" stroke-width=\"{:.2}\"{dash} opacity=\"{opacity:.2}\"", shape.stroke);
                        out += &match (forms, shape.tool) {
                            (true, _) => {
                                let (closed, open) = shape.form_paths();
                                closed.iter().map(|path| format!("  <path d=\"{}\" {common}/>\n", path_of(path, true))).chain(open.iter().map(|path| format!("  <path d=\"{}\" {common}/>\n", path_of(path, false)))).collect::<String>()
                            }
                            (false, Tool::Ellipse) => format!("  <ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" {common}/>\n", x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0),
                            _ => format!("  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"{:.1}\" {common}/>\n", shape.radius),
                        };
                    }
                }
                // A table's rules.
                if shape.form == shapes::FORM_TABLE && shape.tool == Tool::Rect && !shape.titled {
                    let (_, rules, _) = shape.table(&mut self.text);
                    for down in rules {
                        out += &line(&[(x, down), (x + w, down)], (shape.stroke * 0.7).max(1.0));
                    }
                }
            }
            Tool::Line | Tool::Arrow => {
                if shape.sketch {
                    for (points, half, strength) in shape.sketch_strokes() {
                        out += &stroke(&points, half * 2.0, strength);
                    }
                } else {
                    // Its way, dashed as it is, and its heads.
                    let dash = if shape.dashed { " stroke-dasharray=\"8 6\"" } else if shape.dotted { " stroke-dasharray=\"1 5\"" } else { "" };
                    for (at, points) in shape.line_strokes().into_iter().enumerate() {
                        let drawn = line(&points, shape.stroke);
                        out += &if at == 0 { drawn.replacen(" opacity=", &format!("{dash} opacity="), 1) } else { drawn };
                    }
                }
                // Its number, where it is counted, on a disc at its start.
                if shape.votes > 0 {
                    let (at, radius) = (shape.points[0], shape.number_radius());
                    out += &format!("  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{radius:.1}\" fill=\"{ink}\" opacity=\"{opacity:.2}\"/>\n", at.0, at.1);
                    out += &format!("  <text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-family=\"Geist, sans-serif\" font-size=\"{:.0}\" fill=\"{}\">{}</text>\n", at.0, at.1, radius * 1.15, hex(shapes::ink_on(shape.color)), shape.votes);
                }
            }
            // A pen's line as smooth as it is drawn, by hand.
            Tool::Pen if shape.sketch => out += &line(&shapes::smooth_through(&shape.points, 2.0), shape.stroke),
            Tool::Pen => out += &line(&shape.points, shape.stroke),
            _ => {}
        }
        // Its words, where they are drawn, in its face.
        let face = crate::text::FACES.get(shape.face).copied().unwrap_or("Geist");
        let words: Vec<(String, f32, f32, u32)> = match shape.tool {
            Tool::Text => {
                let lines = shape.text_lines(&mut self.text);
                lines.into_iter().map(|line| (line.shown, shape.points[0].0, shape.points[0].1 + line.top as f32, line.px)).collect()
            }
            _ => {
                let (lines, _) = shape.label_lines(&mut self.text);
                lines.into_iter().map(|(line, left, top, _)| (line, left, top, shape.font_px)).collect()
            }
        };
        let fill = match shape.tool {
            Tool::Rect | Tool::Ellipse if shape.border && shape.form != shapes::FORM_STICKY => shape.color,
            Tool::Rect | Tool::Ellipse if shape.filled => shapes::ink_on(shape.fill),
            _ => shape.color,
        };
        for (line, left, top, px) in words.into_iter().filter(|(line, ..)| !line.trim().is_empty()) {
            out += &format!("  <text x=\"{left:.1}\" y=\"{:.1}\" font-family=\"{face}, sans-serif\" font-size=\"{px}\" fill=\"{}\" opacity=\"{opacity:.2}\">{}</text>\n", top + px as f32 * 0.95, hex(fill), escaped(&line));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn a_board_is_written_as_an_svg_of_its_hand_drawn_shapes_and_words() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        type_in(&mut studio, "Cache & DB");
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Disk");
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        let (layers, part, name) = studio.export_part().unwrap();
        assert_eq!(name, "Board 1");
        let svg = studio.board_svg(&layers, part);
        assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("Cache &amp; DB") && svg.contains("Disk") && svg.contains("Architects Daughter"));
        assert!(svg.matches("<path").count() > 8, "drawn by hand, stroke by stroke");
        // Read back by the same parser a dropped SVG is.
        assert!(roxmltree::Document::parse(&svg).is_ok());
        // What is picked out alone.
        studio.selected = vec![0];
        let (layers, _, name) = studio.export_part().unwrap();
        assert_eq!((layers.len(), name.as_str()), (1, "Selection"));
        // As a picture: twice its size, on its paper.
        let sheet = studio.part_sheet(&layers, (90.0, 90.0, 200.0, 100.0));
        let (w, _, pixels) = sheet.render(2.0, &mut studio.text);
        assert_eq!((w, pixels[0] & 0xFF_FFFF), (400, shapes::PAPER));
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }
}
