//! Exporting a board, or part of it: all of it, a frame drawn round part
//! of it, or what is picked out, as PNG, AVIF or JPEG XL as frames are,
//! and as SVG, since a board is all shapes: each drawn as its hand drew
//! it, its words as words, a picture in it as a picture.

use super::*;
use crate::output::Format;

/// The kinds a board exports as, by number: PNG, AVIF, JPEG XL, and SVG.
pub(in crate::studio) const KINDS: [&str; 5] = ["PNG", "AVIF", "JPEG XL", "SVG", "JPEG"];

/// The kinds, by their numbers in `KINDS`, a board is copied as, in the
/// order offered: the ones a chat or a document takes from the clipboard,
/// and an SVG as its words.
pub(in crate::studio) const COPY_KINDS: [u8; 3] = [0, 4, 3];
/// The kinds it is exported as, in the order offered: PNG and JPEG first,
/// as most often wanted, then the smaller AVIF and JPEG XL, and SVG.
pub(in crate::studio) const EXPORT_KINDS: [u8; 5] = [0, 4, 1, 2, 3];

/// The most room its panel leaves round what is exported of a board.
pub(in crate::studio) const MOST_ROOM: u16 = 160;

/// How a board is copied and exported, as last chosen in its panel of
/// options: as which of `KINDS`; one, two or three times its size; on its
/// paper (0), on white (1) or on nothing (2); with so much room round
/// it; and, a PNG, with the board inside it, to paste back as shapes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::studio) struct Choices {
    pub kind: u8,
    pub times: u8,
    pub ground: u8,
    pub room: u16,
    pub keep: bool,
}

impl Default for Choices {
    fn default() -> Choices {
        Choices { kind: 0, times: 2, ground: 0, room: 32, keep: true }
    }
}

impl Choices {
    /// As a line of `board-export`, and read back from one, each thing
    /// kept to what it can be.
    pub(in crate::studio) fn word(self) -> String {
        format!("{} {} {} {} {}\n", self.kind, self.times, self.ground, self.room, u8::from(self.keep))
    }

    pub(in crate::studio) fn read(line: &str) -> Choices {
        let mut words = line.split_whitespace().map(|word| word.parse::<u16>().ok());
        let mut next = |or: u16| words.next().flatten().unwrap_or(or);
        let was = Choices::default();
        Choices {
            kind: next(u16::from(was.kind)).min(KINDS.len() as u16 - 1) as u8,
            times: next(u16::from(was.times)).clamp(1, 3) as u8,
            ground: next(u16::from(was.ground)).min(2) as u8,
            room: next(was.room).min(MOST_ROOM),
            keep: next(1) == 1,
        }
    }

    /// The kind Ctrl+Shift+C copies as: what was chosen, where it can go
    /// on the clipboard; a PNG where it cannot.
    pub(in crate::studio) fn copy_kind(self) -> u8 {
        if matches!(self.kind, 0 | 3 | 4) { self.kind } else { 0 }
    }
}

/// What is said of a JPEG said to go on nothing.
pub(in crate::studio) const LOST: &str = "A JPEG can't be see-through: it goes on white";

/// What some layers are, counted by kind as a board's tools name them:
/// "2 boxes, 1 arrow".
pub(in crate::studio) fn counted(layers: &[Layer]) -> String {
    let mut counts: Vec<(&str, &str, usize)> = Vec::new();
    for layer in layers {
        let (one, many) = match &layer.kind {
            Kind::Picture(_) => ("picture", "pictures"),
            Kind::Shape(shape) => match shape.tool {
                Tool::Rect if shape.form == shapes::FORM_STICKY => ("note", "notes"),
                Tool::Rect if shape.form == shapes::FORM_DIAMOND => ("diamond", "diamonds"),
                Tool::Rect => ("box", "boxes"),
                Tool::Ellipse => ("oval", "ovals"),
                Tool::Arrow => ("arrow", "arrows"),
                Tool::Line => ("line", "lines"),
                Tool::Pen | Tool::Marker => ("drawing", "drawings"),
                Tool::Text => ("text", "texts"),
                _ => ("shape", "shapes"),
            },
        };
        match counts.iter_mut().find(|(known, ..)| *known == one) {
            Some(count) => count.2 += 1,
            None => counts.push((one, many, 1)),
        }
    }
    counts.iter().map(|&(one, many, count)| format!("{count} {}", if count == 1 { one } else { many })).collect::<Vec<_>>().join(", ")
}

/// How a board was last copied and exported, kept in `board-export` among
/// hotshot's state, for the next to be so; as it begins, under test.
pub(in crate::studio) fn last_choices() -> Choices {
    if cfg!(test) {
        return Choices::default();
    }
    crate::output::state_file("board-export").and_then(|file| std::fs::read_to_string(file).ok()).map_or_else(Choices::default, |kept| Choices::read(&kept))
}

pub(in crate::studio) fn keep_choices(choices: Choices) {
    if cfg!(test) {
        return;
    }
    let Some(file) = crate::output::state_file("board-export") else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, choices.word());
}

/// The longest side of a picture of a board, and how many pixels it may
/// have in all: a board drawn far zoomed out is many thousands across, and
/// twice that would take long to draw and more memory than there is.
const LONGEST: f32 = 8192.0;
const MOST_PIXELS: f32 = 32.0e6;

/// How much larger than itself a part so wide and high is drawn to be
/// exported or copied: `wanted`, made smaller where that would be more
/// than `LONGEST` across or `MOST_PIXELS` in all.
pub(in crate::studio) fn export_times(wanted: f32, w: f32, h: f32) -> f32 {
    let (w, h) = (w.max(1.0), h.max(1.0));
    wanted.min(LONGEST / w.max(h)).min((MOST_PIXELS / (w * h)).sqrt())
}

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

/// Base64 as the bytes it is; nothing, from anything else.
fn unbase64(words: &str) -> Option<Vec<u8>> {
    let value = |letter: u8| match letter {
        b'A'..=b'Z' => Some(letter - b'A'),
        b'a'..=b'z' => Some(letter - b'a' + 26),
        b'0'..=b'9' => Some(letter - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let letters: Vec<u8> = words.bytes().filter(|letter| !letter.is_ascii_whitespace() && *letter != b'=').collect();
    let mut out = Vec::with_capacity(letters.len() * 3 / 4);
    for chunk in letters.chunks(4) {
        let mut n = 0u32;
        for (at, &letter) in chunk.iter().enumerate() {
            n |= u32::from(value(letter)?) << (18 - 6 * at);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len().saturating_sub(1)]);
    }
    Some(out)
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
        // As much room round it as its panel last said.
        let room = f32::from(self.board.choices.room);
        Some((layers, (x - room, y - room, w + 2.0 * room, h + 2.0 * room), name))
    }

    /// Some layers as a frame of their own, a part of the board in size,
    /// on a board on what its panel last said it goes on, and on nothing
    /// elsewhere.
    pub(in crate::studio) fn part_sheet(&self, layers: &[Layer], (x, y, w, h): (f32, f32, f32, f32)) -> Doc {
        let mut sheet = Doc::new(w.ceil().max(1.0) as i32, h.ceil().max(1.0) as i32);
        sheet.background = self.export_ground(self.board.choices.kind).map_or(Fill::None, Fill::Color);
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

    /// What a board written as a kind goes on: its paper, white, or
    /// nothing, as its panel last said; white for a JPEG said to go on
    /// nothing, which it cannot. Nothing, off a board.
    pub(in crate::studio) fn export_ground(&self, kind: u8) -> Option<u32> {
        if !self.on_board() {
            return None;
        }
        match self.board.choices.ground {
            0 => Some(self.board_ground()),
            2 if kind != 4 => None,
            _ => Some(0xFFFFFF),
        }
    }

    /// Whether a board written as a kind goes on white though said to go
    /// on nothing, as a JPEG does.
    pub(in crate::studio) fn ground_lost(&self, kind: u8) -> bool {
        kind == 4 && self.board.choices.ground == 2
    }

    /// The board, a frame of it, or what is picked out, written: as a
    /// picture of a kind, at the size its panel last said; or as an SVG.
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
            Ok(()) => {
                self.show_exported(vec![path]);
                if self.ground_lost(kind) {
                    self.say(LOST);
                }
            }
            Err(error) => self.say(format!("Could not write {}: {error}", path.display())),
        }
    }

    /// What is picked out, or the board where nothing is or `whole`, put
    /// on the clipboard as a PNG, a JPEG or an SVG's words (`kind` 0, 4 or
    /// 3), at the size its panel last said, to paste into a chat or a
    /// document; anything else as a PNG.
    pub(in crate::studio) fn copy_board(&mut self, kind: u8, whole: bool) {
        let kind = if matches!(kind, 3 | 4) { kind } else { 0 };
        let (bytes, ending, _) = match self.board_picture(kind, whole) {
            Ok(picture) => picture,
            Err(error) => return self.say(error),
        };
        let what = if whole || self.chosen().is_empty() { "Board" } else { "Selection" };
        let mut said = format!("{what} copied as {}", match kind {
            4 => "JPG",
            3 => "SVG",
            _ => "PNG",
        });
        if self.ground_lost(kind) {
            said += ", on white";
        }
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

    /// What is exported, and the part of the board it is in; of the whole
    /// board, however much is picked out, when `whole`.
    pub(in crate::studio) fn export_whole_or_part(&mut self, whole: bool) -> Option<(Vec<Layer>, (f32, f32, f32, f32), String)> {
        let kept = whole.then(|| std::mem::take(&mut self.selected));
        let part = self.export_part();
        if let Some(kept) = kept {
            self.selected = kept;
        }
        part
    }

    /// How many times its size a part so wide and high is drawn: as many
    /// as the panel last said on a board, a frame as it is; either smaller
    /// where that would be too large to draw.
    pub(in crate::studio) fn part_times(&self, w: f32, h: f32) -> f32 {
        let wanted = if self.on_board() { f32::from(self.board.choices.times) } else { 1.0 };
        export_times(wanted, w, h)
    }

    /// The board, a frame of it, or what is picked out, as the bytes of a
    /// picture of a kind, its ending and its name; of the whole board,
    /// however much is picked out, when `whole`. A PNG has what it shows
    /// inside it as well, where the panel last said to keep it.
    fn board_picture(&mut self, kind: u8, whole: bool) -> Result<(Vec<u8>, &'static str, String), String> {
        let Some((layers, part, name)) = self.export_whole_or_part(whole) else { return Err("Nothing to export".into()) };
        Ok(match kind {
            3 => (self.board_svg(&layers, part).into_bytes(), "svg", name),
            _ => {
                let format = match kind {
                    1 => Format::Avif,
                    2 => Format::Jxl,
                    4 => Format::Jpeg,
                    _ => Format::Png,
                };
                let mut sheet = self.part_sheet(&layers, part);
                sheet.background = self.export_ground(kind).map_or(Fill::None, Fill::Color);
                let times = self.part_times(part.2, part.3);
                let (w, h, pixels) = sheet.render(times, &mut self.text);
                let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
                let encoded = crate::convert::encode(w as u32, h as u32, &pixels, clear, None, format, &self.convert).map_err(|error| error.to_string())?;
                let bytes = match format == Format::Png && self.board.choices.keep && self.on_board() {
                    true => crate::output::with_board(encoded.bytes, &self.board_inside(&layers)),
                    false => encoded.bytes,
                };
                (bytes, format.ending(), name)
            }
        })
    }

    /// Some layers of the board as a file of the canvas of one board, to go
    /// inside a PNG of them: in the board's look, with the boxes that hold
    /// others among them still holding them; its lines as they are, or
    /// where it has pictures, the whole of it as base64.
    pub(in crate::studio) fn board_inside(&self, layers: &[Layer]) -> String {
        let ids: Vec<u32> = layers.iter().map(stacks::layer_id).collect();
        let shown = &self.pages[self.page];
        let mut page = Page::new(shown.name.clone(), Vec::new());
        (page.board, page.styled, page.look, page.ground, page.font, page.px) = (true, true, shown.look, shown.ground, shown.font, shown.px);
        page.loose.layers = layers.to_vec();
        page.loose.stacks = self.doc.stacks.iter().filter(|stack| stack.holder.is_some_and(|holder| ids.contains(&holder)) && stack.members.iter().all(|member| ids.contains(member))).cloned().collect();
        String::from_utf8(write_pages(&[page])).unwrap_or_else(|error| base64(error.as_bytes()))
    }

    /// A PNG with a board inside it, pasted or dropped on a board: what is
    /// inside it put back as the shapes it was, each called something of
    /// its own, as far apart as they were, in the middle of where a
    /// picture would go; picked out, as one step to undo. Whether it had
    /// a board inside it.
    pub(in crate::studio) fn bring_board_png(&mut self, png: &[u8], at: Option<(i32, i32)>) -> bool {
        let Some(words) = crate::output::board_of(png) else { return false };
        let bytes = match words.starts_with("HOTSHOT CANVAS") {
            true => Some(words.into_bytes()),
            false => unbase64(&words),
        };
        let Some(page) = bytes.as_deref().and_then(read_pages).and_then(|pages| pages.into_iter().find(|page| page.board)) else { return false };
        let (mut layers, mut nests) = (page.loose.layers, page.loose.stacks);
        let boxes: Vec<(f32, f32, f32, f32)> = layers.iter().map(|layer| layer.bounds(&mut self.text)).collect();
        let Some((x, y, w, h)) = actions::union(&boxes) else { return false };
        self.commit_text();
        self.checkpoint();
        let at = at.filter(|&at| self.over_page(at)).unwrap_or_else(|| self.open_area().center());
        self.work_at(at);
        let middle = self.to_doc(at);
        let by = ((middle.0 - (x + w / 2.0)).round(), (middle.1 - (y + h / 2.0)).round());
        // Each called something of its own, what its ends were stuck to
        // kept, to be stuck to the same again by its new name.
        let mut names: Vec<(u32, u32)> = Vec::new();
        for layer in &mut layers {
            let was = stacks::layer_id(layer);
            match &mut layer.kind {
                Kind::Picture(picture) => picture.id = shapes::fresh_id(),
                Kind::Shape(shape) => {
                    let stuck = shape.stuck;
                    *shape = shape.copied();
                    shape.stuck = stuck;
                }
            }
            layer.translate(by.0, by.1);
            names.push((was, stacks::layer_id(layer)));
        }
        let renamed = |id: u32| names.iter().find(|(was, _)| *was == id).map(|&(_, now)| now);
        for layer in &mut layers {
            if let Kind::Shape(shape) = &mut layer.kind {
                shape.stuck = shape.stuck.map(|end| end.and_then(|(id, u, v)| Some((renamed(id)?, u, v))));
            }
        }
        // And the boxes holding others hold their copies.
        nests.retain_mut(|stack| {
            let members: Option<Vec<u32>> = stack.members.iter().map(|&member| renamed(member)).collect();
            match (members, stack.holder.and_then(renamed)) {
                (Some(members), Some(holder)) => {
                    (stack.id, stack.members, stack.holder, stack.seen) = (shapes::fresh_id(), members, Some(holder), Vec::new());
                    (stack.x, stack.y) = (stack.x + by.0, stack.y + by.1);
                    true
                }
                _ => false,
            }
        });
        let said = counted(&layers);
        let start = self.doc.layers.len();
        self.doc.layers.extend(layers);
        self.doc.stacks.extend(nests);
        self.selected = (start..self.doc.layers.len()).collect();
        self.tool = Tool::Select;
        self.changed();
        self.say(format!("Opened as shapes: {said}"));
        true
    }

    /// Some layers of the board as an SVG, the part given its whole: each
    /// shape as its hand drew it, words as words, the rest as pictures.
    pub(in crate::studio) fn board_svg(&mut self, layers: &[Layer], (x, y, w, h): (f32, f32, f32, f32)) -> String {
        // Sharp at any size, so at its own; on what the panel last said.
        let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"{x:.1} {y:.1} {w:.1} {h:.1}\">\n");
        if let Some(ground) = self.export_ground(3) {
            out += &format!("  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"{}\"/>\n", hex(ground));
        }
        for layer in layers {
            match &layer.kind {
                Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow | Tool::Pen | Tool::Text) && shape.angle == 0.0 => out += &self.shape_svg(shape),
                _ => {
                    // Anything else drawn as it is, as a picture in its place.
                    let (bx, by, bw, bh) = layer.bounds(&mut self.text);
                    let sheet = self.part_sheet(std::slice::from_ref(layer), (bx, by, bw, bh));
                    let (pw, ph, pixels) = sheet.render(export_times(2.0, bw, bh), &mut self.text);
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
                        match shape.pattern {
                            // Dots in rows, every other row half along.
                            shapes::PATTERN_DOTS => {
                                let step = 8.0 * 0.85;
                                for (row, down) in (0..=(h / step) as i32 + 1).map(|row| (row, y + row as f32 * step + step / 2.0)) {
                                    let shift = if row % 2 == 1 { step / 2.0 } else { 0.0 };
                                    let mut across = x - step + shift + step / 2.0;
                                    while across <= x + w + step {
                                        out += &format!("    <circle cx=\"{across:.1}\" cy=\"{down:.1}\" r=\"1.15\" fill=\"{ink}\" stroke=\"none\"/>\n");
                                        across += step;
                                    }
                                }
                            }
                            pattern => {
                                let mut along = x + y;
                                while along <= x + w + y + h {
                                    out += &format!("    <line x1=\"{:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n", along - y, along - (y + h), y + h);
                                    along += every;
                                }
                                // Crossed: the lines down to the right as well.
                                if pattern == shapes::PATTERN_CROSS {
                                    let mut along = x - (y + h);
                                    while along <= x + w - y {
                                        out += &format!("    <line x1=\"{:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n", along + y, along + y + h, y + h);
                                        along += every;
                                    }
                                }
                            }
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
        let says = |studio: &mut Studio, kind: MenuKind| -> Vec<String> {
            studio.menu_items(kind).into_iter().map(|item| match item {
                crate::studio::menu::Item::Entry { says, .. } => says.to_string(),
                crate::studio::menu::Item::Heading(says) => format!("[{says}]"),
                crate::studio::menu::Item::Rule => "-".to_owned(),
            }).collect()
        };
        // Nothing picked out: the board copied or exported, a list of the
        // kinds beside each, and how the board looks.
        studio.selected.clear();
        let offered = says(&mut studio, MenuKind::Context);
        for want in ["Paste here", "Copy board as", "Export board as", "Theme", "Paper"] {
            assert!(offered.contains(&want.to_owned()), "{want} in {offered:?}");
        }
        assert_eq!(says(&mut studio, MenuKind::CopyAs), ["PNG", "JPEG", "SVG", "-", "Options\u{2026}"]);
        assert_eq!(says(&mut studio, MenuKind::ExportAs), ["PNG", "JPEG", "AVIF", "JPEG XL", "SVG", "-", "Options\u{2026}"]);
        // A box picked out: that, and the whole board after it, in each.
        studio.selected = vec![0];
        let offered = says(&mut studio, MenuKind::Context);
        for want in ["Copy", "Copy as", "Export as", "Order"] {
            assert!(offered.contains(&want.to_owned()), "{want} in {offered:?}");
        }
        for gone in ["Copy frame as PNG", "Add auto layout", "Export frame", "Apply style", "Show / hide"] {
            assert!(!offered.contains(&gone.to_owned()), "no {gone} on a board: {offered:?}");
        }
        assert_eq!(says(&mut studio, MenuKind::CopyAs), ["[SELECTION]", "PNG", "JPEG", "SVG", "[WHOLE BOARD]", "PNG", "JPEG", "SVG", "-", "Options\u{2026}"]);
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
    fn a_board_drawn_far_zoomed_out_is_copied_at_a_size_that_can_be_drawn() {
        // Boxes drawn at a few percent are thousands across: twice that
        // would be tens of thousands of pixels, and hang.
        let mut studio = board();
        studio.zoom = 0.1;
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (0.0, 0.0), (6000.0, 3000.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        let began = std::time::Instant::now();
        studio.copy_board(0, true);
        let png = studio.copied_picture.take().expect("copied");
        let decoded = crate::convert::decode(&png).expect("a PNG");
        assert!(decoded.width <= 8192 && decoded.height <= 8192, "{}x{}", decoded.width, decoded.height);
        assert!(decoded.width as u64 * decoded.height as u64 <= 32_100_000, "{}x{}", decoded.width, decoded.height);
        assert!(decoded.width >= 4000, "still large enough to be sharp: {}", decoded.width);
        assert!(began.elapsed().as_secs_f32() < 10.0);
        // However far it is zoomed out or in when copied, the same size.
        let mut sizes = Vec::new();
        for zoom in [1.0, 0.1, 3.0] {
            let mut studio = board();
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, 100.0), (900.0, 700.0), Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.zoom = zoom;
            studio.copy_board(0, true);
            let decoded = crate::convert::decode(&studio.copied_picture.take().unwrap()).unwrap();
            sizes.push((decoded.width, decoded.height));
        }
        assert!(sizes.iter().all(|&size| size == sizes[0]), "{sizes:?}");
        // A small board, twice its size as before.
        assert_eq!(export_times(2.0, 400.0, 300.0), 2.0);
        assert!(export_times(2.0, 9000.0, 9000.0) < 1.0);
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
        for bytes in [&b"Man"[..], b"Ma", b"M", b"", b"\x00\xFF\x80hotshot"] {
            assert_eq!(unbase64(&base64(bytes)).as_deref(), Some(bytes));
        }
        assert_eq!(unbase64("not base64!"), None);
    }

    /// A board of two boxes with words, joined by an arrow.
    fn joined() -> Studio {
        let mut studio = board();
        for (from, to, words) in [((100.0, 300.0), (260.0, 372.0), "Proxy"), ((500.0, 300.0), (660.0, 372.0), "Auth")] {
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, from, to, Mods::default());
            type_in(&mut studio, words);
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
        }
        studio.join(0, 1);
        studio.selected.clear();
        studio
    }

    #[test]
    fn a_board_copied_as_a_png_has_the_board_in_it_and_pastes_back_as_its_shapes() {
        let mut studio = joined();
        studio.copy_board(0, true);
        let png = studio.copied_picture.take().unwrap();
        assert!(crate::output::board_of(&png).is_some_and(|inside| inside.starts_with("HOTSHOT CANVAS")), "the board in it, as its file's lines");
        // Pasted: the same three shapes again, each its own, the arrow
        // joining the new boxes, picked out, in the middle of the view.
        let (before, undos) = (studio.doc.layers.len(), studio.undo.len());
        studio.pasted(Some(png.clone()), None);
        assert_eq!(studio.doc.layers.len(), before + 3);
        assert_eq!(studio.selected, vec![3, 4, 5]);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Opened as shapes: 2 boxes, 1 arrow"));
        let (old, new): (Vec<Shape>, Vec<Shape>) = ((0..3).map(|at| shape(&studio, at)).collect(), (3..6).map(|at| shape(&studio, at)).collect());
        assert!(new.iter().all(|copy| old.iter().all(|was| was.id != copy.id)), "called something of its own");
        assert_eq!(new[2].stuck.map(|end| end.map(|(id, ..)| id)), [Some(new[0].id), Some(new[1].id)], "the arrow joins the new boxes");
        assert_eq!((new[0].text.as_str(), new[1].text.as_str()), ("Proxy", "Auth"));
        let by = (new[0].frame().0 - old[0].frame().0, new[0].frame().1 - old[0].frame().1);
        for (was, copy) in old.iter().zip(&new) {
            let (a, b) = (was.frame(), copy.frame());
            assert!((b.0 - a.0 - by.0).abs() < 0.5 && (b.1 - a.1 - by.1).abs() < 0.5 && (b.2 - a.2).abs() < 0.5 && (b.3 - a.3).abs() < 0.5, "as far apart: {a:?} {b:?}");
        }
        let boxes: Vec<(f32, f32, f32, f32)> = (3..6).map(|at| studio.doc.layers[at].bounds(&mut studio.text)).collect();
        let (x, y, w, h) = actions::union(&boxes).unwrap();
        let middle = studio.to_doc(studio.open_area().center());
        assert!((x + w / 2.0 - middle.0).abs() <= 1.0 && (y + h / 2.0 - middle.1).abs() <= 1.0, "in the middle of the view");
        // One step to undo.
        assert_eq!(studio.undo.len(), undos + 1);
        studio.undo();
        assert_eq!(studio.doc.layers.len(), before);
        // Dropped from a file manager: the same, where it was dropped.
        let at = studio.to_window_page((900.0, 700.0));
        studio.drop_file(&png, "Board 1", Some(at));
        assert_eq!(studio.doc.layers.len(), before + 3);
        let boxes: Vec<(f32, f32, f32, f32)> = (3..6).map(|at| studio.doc.layers[at].bounds(&mut studio.text)).collect();
        let (x, y, w, h) = actions::union(&boxes).unwrap();
        assert!((x + w / 2.0 - 900.0).abs() <= 1.0 && (y + h / 2.0 - 700.0).abs() <= 1.0, "where it was dropped");
        studio.undo();
        // Asked for as a picture, Ctrl+Shift+V, it is one.
        let ctrl_shift = Mods { ctrl: true, shift: true, ..Mods::default() };
        assert_eq!(studio.key(Key::Char('V'), ctrl_shift, true), Outcome::Paste);
        studio.key(Key::Char('V'), ctrl_shift, false);
        studio.pasted(Some(png), None);
        assert!(matches!(studio.doc.layers.last().map(|layer| &layer.kind), Some(Kind::Picture(_))));
        studio.undo();
        // Not kept in it, or a JPEG: a picture.
        studio.board.choices.keep = false;
        studio.copy_board(0, true);
        let plain = studio.copied_picture.take().unwrap();
        assert_eq!(crate::output::board_of(&plain), None);
        studio.board.choices.keep = true;
        studio.copy_board(4, true);
        let jpeg = studio.copied_picture.take().unwrap();
        for picture in [plain, jpeg] {
            studio.pasted(Some(picture), None);
            assert_eq!(studio.doc.layers.len(), before + 1);
            assert!(matches!(studio.doc.layers[before].kind, Kind::Picture(_)));
            studio.undo();
        }
        // Into a page of frames: a picture, as it shows.
        studio.copy_board(0, true);
        let png = studio.copied_picture.take().unwrap();
        studio.set_mode(false);
        let count = studio.doc.layers.len();
        studio.pasted(Some(png), None);
        assert!(studio.doc.layers.len() == count + 1 && matches!(studio.doc.layers[count].kind, Kind::Picture(_)));
    }

    #[test]
    fn a_picture_in_a_box_goes_with_the_board_in_its_png_and_is_in_the_box_pasted_back() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (280.0, 200.0), Mods::default());
        type_in(&mut studio, "Backend");
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let image = Arc::new(Image::new(120, 60, vec![0xFF3E_63DD; 120 * 60]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, "Screenshot".into(), (600.0, 150.0))));
        studio.changed();
        pull(&mut studio, (600.0, 150.0), (200.0, 170.0), Mods::default());
        let held = studio.id_of(1);
        assert_eq!(studio.holder_of(held), Some(studio.id_of(0)), "in the box");
        studio.selected = vec![0];
        studio.copy_board(0, false);
        let png = studio.copied_picture.take().unwrap();
        assert!(crate::output::board_of(&png).is_some_and(|inside| !inside.starts_with("HOTSHOT")), "with a picture, as base64");
        studio.pasted(Some(png), None);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Opened as shapes: 1 box, 1 picture"));
        let (boxed, picture) = (studio.id_of(2), studio.id_of(3));
        assert!(matches!(&studio.doc.layers[3].kind, Kind::Picture(picture) if picture.image.width() == 120 && picture.image.levels[0].2[0] & 0xFF_FFFF == 0x3E63DD));
        assert_eq!(studio.holder_of(picture), Some(boxed), "the picture in the new box");
        assert_eq!(studio.holder_of(held), Some(studio.id_of(0)), "and the first still in its own");
    }
}
