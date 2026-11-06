//! Exporting a board, or part of it: all of it, a frame drawn round part
//! of it, or what is picked out, as PNG, AVIF or JPEG XL as frames are,
//! and as SVG, since a board is all shapes: each drawn as its hand drew
//! it, its words as words, a picture in it as a picture.

use super::*;
use crate::output::Format;

/// The kinds a board exports as, by number: PNG, AVIF, JPEG XL, and SVG.
pub(in crate::studio) const KINDS: [&str; 5] = ["PNG", "AVIF", "JPEG XL", "SVG", "JPEG"];

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
        self.export_board_of(kind, false);
    }

    /// The same, of the whole board however much is picked out, when
    /// `whole`, as the right click on the board offers it.
    pub(in crate::studio) fn export_board_of(&mut self, kind: u8, whole: bool) {
        let (bytes, ending, name) = match self.board_picture(kind, whole) {
            Ok(picture) => picture,
            Err(error) => return self.say(error),
        };
        let dir = self.export_dir();
        if let Err(error) = std::fs::create_dir_all(&dir) {
            return self.say(format!("Could not make {}: {error}", dir.display()));
        }
        let stem = one_line(&name).replace('/', "-");
        let path = dir.join(format!("{stem}.{ending}"));
        match std::fs::write(&path, bytes) {
            Ok(()) => self.show_exported(vec![path]),
            Err(error) => self.say(format!("Could not write {}: {error}", path.display())),
        }
    }

    /// What is picked out, or the board where nothing is or `whole`, put
    /// on the clipboard as a PNG or a JPEG (`kind` 0 or 4), twice its size
    /// to stay sharp, to paste into a chat or a document.
    pub(in crate::studio) fn copy_board(&mut self, kind: u8, whole: bool) {
        let kind = if kind == 4 { 4 } else { 0 };
        let (bytes, ending, _) = match self.board_picture(kind, whole) {
            Ok(picture) => picture,
            Err(error) => return self.say(error),
        };
        let what = if whole || self.chosen().is_empty() { "Board" } else { "Selection" };
        let said = format!("{what} copied as {}", if kind == 4 { "JPG" } else { "PNG" });
        // Under test, kept to be looked at, and nothing started nor
        // written where hotshot keeps its things.
        if cfg!(test) {
            self.copied_picture = Some(bytes);
            return self.say(said);
        }
        let kept = crate::output::state_file("pins").and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos());
            let file = dir.join(format!("{}-{stamp}-board.{ending}", std::process::id()));
            std::fs::write(&file, &bytes).ok()?;
            Some(file)
        });
        match kept.map(|file| crate::pin::again(&["--clip".as_ref(), file.as_os_str(), "--forget".as_ref()])) {
            Some(Ok(())) => self.say(said),
            _ => self.say("Could not copy"),
        }
    }

    /// The board, a frame of it, or what is picked out, as the bytes of a
    /// picture of a kind, its ending and its name; of the whole board,
    /// however much is picked out, when `whole`.
    fn board_picture(&mut self, kind: u8, whole: bool) -> Result<(Vec<u8>, &'static str, String), String> {
        let kept = whole.then(|| std::mem::take(&mut self.selected));
        let part = self.export_part();
        if let Some(kept) = kept {
            self.selected = kept;
        }
        let Some((layers, part, name)) = part else { return Err("Nothing to export".into()) };
        Ok(match kind {
            3 => (self.board_svg(&layers, part).into_bytes(), "svg", name),
            _ => {
                let format = match kind {
                    1 => Format::Avif,
                    2 => Format::Jxl,
                    4 => Format::Jpeg,
                    _ => Format::Png,
                };
                let sheet = self.part_sheet(&layers, part);
                let times = if self.on_board() { 2.0 } else { 1.0 };
                let (w, h, pixels) = sheet.render(times, &mut self.text);
                let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
                let encoded = crate::convert::encode(w as u32, h as u32, &pixels, clear, None, format, &self.convert).map_err(|error| error.to_string())?;
                (encoded.bytes, format.ending(), name)
            }
        })
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
        // Strokes drawn by hand: each the edge of its ink, filled, in one
        // group seen through as one, so that where they cross they are no
        // darker.
        let inks = |inks: Vec<shapes::Ink>| -> String {
            let mut group = format!("  <g fill=\"{ink}\" opacity=\"{opacity:.2}\">\n");
            for one in &inks {
                group += &format!("    <path d=\"{}\"/>\n", path_of(&one.outline(), true));
                if one.filled && one.points.len() > 2 {
                    group += &format!("    <path d=\"{}\"/>\n", path_of(&one.points, true));
                }
            }
            group + "  </g>\n"
        };
        let (x, y, w, h) = shape.frame();
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                let forms = shape.has_form();
                let filled = shape.filled && !shape.placeholder();
                if filled {
                    let fill = hex(shape.fill);
                    let body = match (forms, shape.tool) {
                        (true, _) => shape.form_paths().0.iter().map(|path| format!("  <path d=\"{}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", path_of(path, true))).collect::<String>(),
                        (false, Tool::Ellipse) => format!("  <ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0),
                        _ => match shape.sketch_fill() {
                            Some(path) => format!("  <path d=\"{}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", path_of(&path, true)),
                            None => format!("  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"{:.1}\" fill=\"{fill}\" opacity=\"{opacity:.2}\"/>\n", if shape.sketch { 0.0 } else { shape.radius }),
                        },
                    };
                    out += &body;
                    // Hatched: fine lines up to the right in its line's
                    // color, kept to its fill.
                    if shape.hatch {
                        let clip = format!("hatch{}", shape.id);
                        out += &format!("  <clipPath id=\"{clip}\">\n  {}  </clipPath>\n  <g clip-path=\"url(#{clip})\" stroke=\"{ink}\" stroke-width=\"1.6\" opacity=\"{:.2}\">\n", body.trim_start(), opacity * 150.0 / 256.0);
                        let every = 8.0 * std::f32::consts::SQRT_2;
                        let mut along = x + y;
                        while along <= x + w + y + h {
                            out += &format!("    <line x1=\"{:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n", along - y, along - (y + h), y + h);
                            along += every;
                        }
                        out += "  </g>\n";
                    }
                }
                if shape.border || shape.placeholder() {
                    if shape.sketch && !shape.placeholder() {
                        let strokes = if forms { shape.form_strokes() } else { shape.sketch_strokes() };
                        out += &inks(strokes);
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
                    out += &inks(shape.sketch_strokes());
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
    fn a_right_click_on_a_board_copies_or_exports_it_and_what_is_picked_out_as_png_or_jpg() {
        let mut studio = board();
        let dir = std::env::temp_dir().join(format!("hotshot-board-export-{}", std::process::id()));
        studio.dir = dir.clone();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (600.0, 400.0), (760.0, 472.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let says = |studio: &mut Studio| -> Vec<String> {
            studio.menu_items(MenuKind::Context).into_iter().filter_map(|item| match item {
                crate::studio::menu::Item::Entry { says, .. } => Some(says),
                _ => None,
            }).collect()
        };
        // Nothing picked out: the board, copied or written, as either.
        studio.selected.clear();
        let offered = says(&mut studio);
        for want in ["Copy board as PNG", "Copy board as JPG", "Export board as PNG", "Export board as JPG"] {
            assert!(offered.contains(&want.to_owned()), "{want} in {offered:?}");
        }
        assert!(!offered.iter().any(|said| said == "Copy as PNG"));
        // A box picked out: that, copied or written, and the whole board
        // in a list beside.
        studio.selected = vec![0];
        let offered = says(&mut studio);
        for want in ["Copy", "Copy as PNG", "Copy as JPG", "Export as PNG", "Export as JPG", "Whole board"] {
            assert!(offered.contains(&want.to_owned()), "{want} in {offered:?}");
        }
        assert!(!offered.iter().any(|said| said == "Copy frame as PNG"), "no frame to copy on a board");
        // Copied: Ctrl+Shift+C, the box alone as a PNG; and the whole
        // board as a JPG, wider.
        press_key(&mut studio, Key::Char('C'), Mods { ctrl: true, shift: true, ..Mods::default() });
        let png = studio.copied_picture.take().expect("a picture copied");
        assert!(crate::clipboard::picture_kind(&png) == Some("image/png"));
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Selection copied as PNG"));
        studio.run_menu(crate::studio::menu::MenuAct::Board(crate::studio::board::Act::CopyBoard(4)));
        let jpg = studio.copied_picture.take().expect("the board copied");
        assert!(crate::clipboard::picture_kind(&jpg) == Some("image/jpeg"));
        let (boxed, board) = (crate::convert::decode(&png).unwrap().width, crate::convert::decode(&jpg).unwrap().width);
        assert!(board > boxed, "the board {board} wider than the box {boxed}");
        assert_eq!(studio.selected, vec![0], "still picked out");
        // The whole board as a JPG, though a box is picked out: both boxes in
        // it, wider than the one box.
        studio.export_board_of(4, true);
        let written: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|entry| entry.path()).collect();
        let jpg = written.iter().find(|path| path.extension().is_some_and(|ending| ending == "jpg")).expect("a JPEG written");
        let bytes = std::fs::read(jpg).unwrap();
        assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF], "a JPEG");
        let whole = crate::convert::decode(&bytes).map(|decoded| decoded.width);
        // The selection alone as a PNG: narrower.
        studio.export_board_of(0, false);
        let png = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|entry| entry.path()).find(|path| path.extension().is_some_and(|ending| ending == "png")).expect("a PNG written");
        let part = crate::convert::decode(&std::fs::read(&png).unwrap()).map(|decoded| decoded.width);
        let (whole, part) = (whole.expect("the JPEG read"), part.expect("the PNG read"));
        assert!(whole > part, "the board {whole} wider than the box {part}");
        let _ = std::fs::remove_dir_all(&dir);
    }
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
        assert!(svg.contains("<g fill=") && svg.matches("<path").count() > 4, "drawn by hand, stroke by stroke");
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
