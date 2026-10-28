//! Mermaid, read and written by a parser of hotshot's own, with no script
//! engine: text pasted on a board, or typed into a label and Ctrl+Enter,
//! becomes the board's own boxes and arrows, to move and change as any,
//! laid out as Mermaid lays it out. Flowcharts, in layers with their
//! arrows round what is in the way, their shapes, links and subgraphs as
//! boxes in boxes; sequence diagrams, as a sequence that keeps itself laid
//! out; class, state and ER diagrams; gantt charts, pies, mind maps,
//! timelines, journeys, git graphs and quadrant charts. Flowcharts,
//! sequences and mind maps are written back as Mermaid too.

use super::*;

mod charts;
mod flow;
mod graphs;
mod layered;
mod sequence;

pub(in crate::studio) use layered::Dir;
use layered::Point;

/// The kinds of diagram read, by the word they begin with.
pub(in crate::studio) const KINDS: [&str; 14] = ["flowchart", "graph", "sequenceDiagram", "classDiagram", "stateDiagram-v2", "stateDiagram", "erDiagram", "gantt", "pie", "mindmap", "timeline", "journey", "gitGraph", "quadrantChart"];

/// What a box of a diagram is drawn as.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::studio) enum Form {
    Box,
    Round,
    Stadium,
    Subroutine,
    Database,
    Circle,
    Double,
    Flag,
    Diamond,
    Hexagon,
    LeanRight,
    LeanLeft,
    Trapezoid,
    TrapezoidDown,
    Note,
    Actor,
    Table,
    /// A state diagram's start, a dot; its end, a dot in a ring; and the
    /// bar a fork or a join is.
    Start,
    End,
    Bar,
}

/// The text after a diagram's front matter, the lines between `---`s.
fn after_front(text: &str) -> &str {
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix("---") else { return text };
    match rest.find("\n---") {
        Some(end) => rest[end + 4..].trim_start_matches(|c: char| c != '\n').trim_start_matches('\n'),
        None => text,
    }
}

/// The title its front matter gives a diagram, if it gives one.
fn front_title(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    rest[..end].lines().find_map(|line| line.trim().strip_prefix("title:").map(|title| plain(title)))
}

/// The lines of a diagram, its comments, its directives and its blank
/// lines gone; its first line, which says what it is, among them.
fn lines_of(text: &str) -> impl Iterator<Item = &str> {
    after_front(text).lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("%%"))
}

/// The line a diagram begins with, which says what it is.
fn header(text: &str) -> String {
    lines_of(text).next().unwrap_or("").to_owned()
}

/// Which kind of diagram some text is, by its first word, if it is one.
pub(in crate::studio) fn kind_of(text: &str) -> Option<&'static str> {
    let first = lines_of(text).next()?;
    let word = first.split_whitespace().next()?.trim_end_matches(':');
    let word = if word == "flowchart-elk" { "flowchart" } else { word };
    KINDS.iter().find(|known| **known == word).copied()
}

/// The lines of a diagram after its first, comments and blank ones gone,
/// and what follows `%%` on a line.
fn body(text: &str) -> Vec<String> {
    lines_of(text).skip(1).map(|line| line.split("%%").next().unwrap_or("").trim().to_owned()).filter(|line| !line.is_empty()).collect()
}

/// Words with their quotes and Mermaid's line breaks and markup taken out.
fn plain(words: &str) -> String {
    let words = words.trim();
    let words = words.strip_prefix('`').and_then(|inner| inner.strip_suffix('`')).unwrap_or(words);
    let words = words.strip_prefix('"').and_then(|inner| inner.strip_suffix('"')).unwrap_or(words);
    words
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("<br />", "\n")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("#quot;", "\"")
        .replace("**", "")
        .trim()
        .to_owned()
}

/// A color as CSS writes it, `#rgb`, `#rrggbb` or a few by name.
fn css_color(word: &str) -> Option<u32> {
    let word = word.trim().trim_end_matches(';').trim();
    if let Some(hex) = word.strip_prefix('#') {
        return match hex.len() {
            3 | 4 => {
                let digit = |at: usize| u32::from_str_radix(&hex[at..at + 1], 16).ok().map(|d| d * 17);
                Some(digit(0)? << 16 | digit(1)? << 8 | digit(2)?)
            }
            6 | 8 => u32::from_str_radix(&hex[..6], 16).ok(),
            _ => None,
        };
    }
    let named = [("red", 0xFF0000), ("green", 0x008000), ("blue", 0x0000FF), ("yellow", 0xFFFF00), ("orange", 0xFFA500), ("purple", 0x800080), ("pink", 0xFFC0CB), ("gray", 0x808080), ("grey", 0x808080), ("black", 0x000000), ("white", 0xFFFFFF), ("lightblue", 0xADD8E6), ("lightgreen", 0x90EE90), ("lightgray", 0xD3D3D3), ("lightgrey", 0xD3D3D3), ("violet", 0xEE82EE), ("cyan", 0x00FFFF), ("teal", 0x008080), ("brown", 0xA52A2A), ("gold", 0xFFD700)];
    named.iter().find(|(name, _)| name.eq_ignore_ascii_case(word)).map(|&(_, color)| color)
}

/// A color's hue in degrees, how strong it is and how light, each 0 to 1.
fn hue_of(color: u32) -> (f32, f32, f32) {
    let part = |shift: u32| (color >> shift & 0xFF) as f32 / 255.0;
    let (r, g, b) = (part(16), part(8), part(0));
    let (most, least) = (r.max(g).max(b), r.min(g).min(b));
    let light = (most + least) / 2.0;
    let chroma = most - least;
    if chroma < 1e-3 {
        return (0.0, 0.0, light);
    }
    let strong = chroma / (1.0 - (2.0 * light - 1.0).abs()).max(1e-3);
    let hue = if most == r { ((g - b) / chroma).rem_euclid(6.0) } else if most == g { (b - r) / chroma + 2.0 } else { (r - g) / chroma + 4.0 } * 60.0;
    (hue, strong.min(1.0), light)
}

/// The board's fill nearest a color in hue: white for a gray.
fn board_fill(color: u32) -> u32 {
    let (hue, strong, _) = hue_of(color);
    if strong < 0.12 {
        return 0xFFFFFF;
    }
    let near = |of: u32| {
        let (other, ..) = hue_of(of);
        let apart = (hue - other).abs();
        apart.min(360.0 - apart)
    };
    FILLS.iter().copied().min_by(|a, b| near(*a).total_cmp(&near(*b))).unwrap_or(FILLS[2])
}

/// The board's line color nearest a color: its ink for a dark gray, its
/// quiet gray for a light one, else the nearest in hue.
fn board_stroke(color: u32) -> u32 {
    let (hue, strong, light) = hue_of(color);
    if strong < 0.15 {
        return if light > 0.45 { QUIET } else { INK };
    }
    let near = |of: u32| {
        let (other, ..) = hue_of(of);
        let apart = (hue - other).abs();
        apart.min(360.0 - apart)
    };
    STROKES[1..].iter().copied().min_by(|a, b| near(*a).total_cmp(&near(*b))).unwrap_or(INK)
}

/// The colors a diagram's parts are told apart by, one after another, as
/// Mermaid gives its pie slices and its sections theirs: the board's own
/// fills and lines, then a few more of the same lightness.
pub(in crate::studio) const SERIES: [(u32, u32); 8] = [(0xD0EBFF, 0x1971C2), (0xFFF3BF, 0xF08C00), (0xD3F9D8, 0x2F9E44), (0xFFE3E3, 0xE03131), (0xE5DBFF, 0x7048E8), (0xC5F6FA, 0x0C8599), (0xFFE8CC, 0xE8590C), (0xFFDEEB, 0xC2255C)];

/// Room kept round a diagram's own boxes, as the board keeps round what
/// is in a box.
const PAD: f32 = 20.0;

impl Studio {
    /// How wide some words are on a board, in whichever of its faces is
    /// wider, so that they fit however the board is drawn.
    fn words_wide(&mut self, words: &str, px: u32) -> f32 {
        words.split('\n').map(|line| self.text.width_in(LETTERING, line, px).max(self.text.width_in(0, line, px))).max().unwrap_or(0) as f32
    }

    /// How tall some lines of words are.
    fn words_tall(words: &str, px: u32) -> f32 {
        (words.split('\n').count() as i32 * Text::line_height(px)) as f32
    }

    /// A box of a diagram, its words in it, sized to them in either face,
    /// drawn as a board draws one; where its middle is.
    fn diagram_box(&mut self, words: &str, form: Form, middle: Point) -> usize {
        let tool = if matches!(form, Form::Circle | Form::Double | Form::Start | Form::End) { Tool::Ellipse } else { Tool::Rect };
        let mut shape = Shape::new(tool, INK, LINE, middle);
        let making = self.board.making;
        self.board.making = match form {
            Form::Diamond => shapes::FORM_DIAMOND,
            Form::Database => shapes::FORM_CYLINDER,
            Form::Note => shapes::FORM_STICKY,
            Form::Stadium => shapes::FORM_STADIUM,
            Form::Subroutine => shapes::FORM_SUBROUTINE,
            Form::Hexagon => shapes::FORM_HEXAGON,
            Form::LeanRight => shapes::FORM_LEAN_RIGHT,
            Form::LeanLeft => shapes::FORM_LEAN_LEFT,
            Form::Trapezoid => shapes::FORM_TRAPEZOID,
            Form::TrapezoidDown => shapes::FORM_TRAPEZOID_DOWN,
            Form::Flag => shapes::FORM_FLAG,
            Form::Actor => shapes::FORM_STICK,
            Form::Table => shapes::FORM_TABLE,
            _ => shapes::FORM_BOX,
        };
        self.board_dress(&mut shape);
        self.board.making = making;
        match form {
            Form::Round => shape.radius = 16.0,
            Form::Double => shape.form = shapes::FORM_DOUBLE,
            Form::Actor => shape.filled = false,
            Form::Start | Form::Bar => (shape.filled, shape.fill, shape.border) = (true, INK, false),
            Form::End => (shape.form, shape.filled, shape.fill) = (shapes::FORM_DOUBLE, true, INK),
            _ => {}
        }
        let least = match form {
            // A choice: a small diamond with no words.
            Form::Diamond if words.is_empty() => (36.0, 36.0),
            Form::Diamond => (120.0, 76.0),
            Form::Circle | Form::Double => (64.0, 64.0),
            Form::Database => (100.0, 72.0),
            Form::Actor => (64.0, 104.0),
            Form::Note => (110.0, 52.0),
            Form::Start => (24.0, 24.0),
            Form::End => (28.0, 28.0),
            Form::Bar => (80.0, 9.0),
            Form::Hexagon | Form::LeanLeft | Form::LeanRight | Form::Trapezoid | Form::TrapezoidDown => (110.0, 52.0),
            _ => (90.0, 52.0),
        };
        shape.text = words.to_owned();
        shape.points = vec![(middle.0 - least.0 / 2.0, middle.1 - least.1 / 2.0), (middle.0 + least.0 / 2.0, middle.1 + least.1 / 2.0)];
        // As large as its words need in the board's lettering and in the
        // plain face the clean look draws them in, whichever needs more.
        let mut needs = shape.label_needs(&mut self.text, least, nest::MOST_WIDE);
        let face = shape.face;
        shape.face = 0;
        let plain_needs = shape.label_needs(&mut self.text, least, nest::MOST_WIDE);
        shape.face = face;
        // And a little more across, for letters drawn a little wider zoomed.
        needs = (needs.0.max(plain_needs.0) + if words.is_empty() { 0.0 } else { 10.0 }, needs.1.max(plain_needs.1));
        let (w, h) = match form {
            Form::Circle | Form::Double => (needs.0.max(needs.1), needs.0.max(needs.1)),
            Form::Start | Form::End | Form::Bar => least,
            Form::Diamond if words.is_empty() => least,
            _ => needs,
        };
        shape.points = vec![(middle.0 - w / 2.0, middle.1 - h / 2.0), (middle.0 + w / 2.0, middle.1 + h / 2.0)];
        shape.base = (w, h);
        shape.touch();
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// A shape of the board's, by its layer, to change.
    fn shape_mut(&mut self, index: usize) -> Option<&mut Shape> {
        match &mut self.doc.layers.get_mut(index)?.kind {
            Kind::Shape(shape) => Some(shape),
            Kind::Picture(_) => None,
        }
    }

    /// A box of a diagram put with its middle at a point.
    fn center_at(&mut self, index: usize, middle: Point) {
        let (x, y, w, h) = self.box_frame(index);
        self.doc.layers[index].translate(middle.0 - (x + w / 2.0), middle.1 - (y + h / 2.0));
    }

    /// A box of a diagram given a place and a size of its own.
    fn frame_to(&mut self, index: usize, (x, y, w, h): (f32, f32, f32, f32)) {
        if let Some(shape) = self.shape_mut(index) {
            shape.points = vec![(x, y), (x + w, y + h)];
            shape.base = (w, h);
            shape.touch();
        }
    }

    /// Words on the board, from their top left.
    fn diagram_words(&mut self, words: &str, at: Point, px: u32) -> usize {
        let mut shape = Shape::new(Tool::Text, INK, LINE, at);
        self.board_dress(&mut shape);
        (shape.text, shape.font_px) = (words.to_owned(), px);
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// Words on the board with their middle at a point.
    fn words_at(&mut self, words: &str, middle: Point, px: u32) -> usize {
        let (wide, tall) = (self.text.width_in(LETTERING, words.split('\n').max_by_key(|line| line.len()).unwrap_or(""), px) as f32, Self::words_tall(words, px));
        let index = self.diagram_words(words, (middle.0 - wide / 2.0, middle.1 - tall / 2.0), px);
        if let Some(shape) = self.shape_mut(index) {
            shape.align = 1;
        }
        index
    }

    /// A title over a diagram, large, its middle across at `x` and its
    /// bottom at `bottom`.
    fn diagram_title(&mut self, title: &str, x: f32, bottom: f32) -> usize {
        let px = 28;
        let wide = self.words_wide(title, px);
        self.diagram_words(title, (x - wide / 2.0, bottom - Text::line_height(px) as f32), px)
    }

    /// A line or an arrow from one point to another, dressed as a board's.
    fn diagram_line(&mut self, from: Point, to: Point, head: bool, dashed: bool) -> usize {
        let mut shape = Shape::new(if head { Tool::Arrow } else { Tool::Line }, INK, LINE, from);
        self.board_dress(&mut shape);
        shape.points[1] = to;
        (shape.dashed, shape.routed) = (dashed, false);
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// Mermaid put on the board, in the middle of what is seen, as the
    /// board's own shapes, picked out; one step to undo. Whether it was
    /// Mermaid.
    pub(in crate::studio) fn bring_mermaid(&mut self, text: &str) -> bool {
        let Some(kind) = kind_of(text) else { return false };
        self.commit_text();
        self.checkpoint();
        let start = self.doc.layers.len();
        let origin = self.to_doc(self.open_area().center());
        match kind {
            "flowchart" | "graph" => self.build_flow(text, origin),
            "sequenceDiagram" => self.build_sequence(text, origin),
            "classDiagram" => self.build_class(text, origin),
            "stateDiagram-v2" | "stateDiagram" => self.build_state(text, origin),
            "erDiagram" => self.build_er(text, origin),
            "mindmap" => self.build_mindmap(text, origin),
            "gantt" => self.build_gantt(text, origin),
            "pie" => self.build_pie(text, origin),
            "timeline" => self.build_timeline(text, origin),
            "journey" => self.build_journey(text, origin),
            "gitGraph" => self.build_git(text, origin),
            _ => self.build_quadrant(text, origin),
        }
        // Laid out until it is still: what is tied follows what it is tied
        // to, and the boxes round them grow round where it all ends up.
        for _ in 0..3 {
            self.changed();
        }
        // All of it moved to have its middle where it was put.
        let made: Vec<(f32, f32, f32, f32)> = (start..self.doc.layers.len()).map(|index| self.doc.layers[index].bounds(&mut self.text)).collect();
        if let Some((x, y, w, h)) = actions::union(&made) {
            let by = (origin.0 - (x + w / 2.0), origin.1 - (y + h / 2.0));
            for index in start..self.doc.layers.len() {
                self.doc.layers[index].translate(by.0, by.1);
            }
            self.changed();
        }
        self.selected = (start..self.doc.layers.len()).collect();
        self.changed();
        self.say(format!("Mermaid {kind}: {} shapes", self.doc.layers.len() - start));
        true
    }

    /// Mermaid of what is picked out, or all of the board: a sequence as a
    /// sequence diagram, `mindmap` a map from the box picked out as one,
    /// and anything else as a flowchart of its boxes and arrows.
    pub(in crate::studio) fn to_mermaid(&self, mindmap: bool) -> String {
        let chosen = self.chosen();
        let mut ids: Vec<u32> = match chosen.is_empty() {
            true => self.doc.layers.iter().map(stacks::layer_id).collect(),
            false => chosen.iter().map(|&index| self.id_of(index)).collect(),
        };
        for id in ids.clone() {
            ids.extend(self.inside(id));
        }
        let words_of = |id: u32| self.index_of(id).map_or(String::new(), |index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => shape.text.replace('\n', "<br/>").replace('"', "'"),
            Kind::Picture(picture) => picture.name.clone(),
        });
        // A sequence in it.
        let sequence = ids.iter().copied().find(|&id| self.index_of(id).is_some_and(|index| matches!(&self.doc.layers[index].kind, Kind::Shape(shape) if shape.region == SEQUENCE)));
        if let Some(holder) = sequence.filter(|_| !mindmap) {
            return self.sequence_mermaid(holder);
        }
        let boxes: Vec<u32> = ids.iter().copied().filter(|&id| self.index_of(id).is_some_and(|index| self.is_box(index))).collect();
        let links: Vec<(u32, u32, u32)> = self.links().into_iter().filter(|(_, from, to)| boxes.contains(from) && boxes.contains(to)).collect();
        if mindmap {
            let start = chosen.first().map(|&index| self.id_of(index)).or_else(|| boxes.first().copied()).unwrap_or(0);
            let mut out = "mindmap\n".to_owned();
            let mut stack = vec![(start, 1usize)];
            let mut seen = Vec::new();
            while let Some((id, depth)) = stack.pop() {
                if seen.contains(&id) {
                    continue;
                }
                seen.push(id);
                let words = words_of(id).replace("<br/>", " ");
                out += &format!("{}{}\n", "  ".repeat(depth), if depth == 1 { format!("(({words}))") } else { words });
                for child in self.child_boxes(id).into_iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
            return out;
        }
        let name = |id: u32| format!("n{}", boxes.iter().position(|&known| known == id).unwrap_or(0) + 1);
        let down = boxes.first().is_some_and(|&id| self.goes_down_from(id));
        let mut out = format!("flowchart {}\n", if down { "TD" } else { "LR" });
        let held: Vec<u32> = boxes.iter().copied().filter(|&id| !self.inside(id).is_empty()).collect();
        let node = |id: u32| -> String {
            let Some(index) = self.index_of(id) else { return name(id) };
            let Kind::Shape(shape) = &self.doc.layers[index].kind else { return name(id) };
            let words = words_of(id);
            let (open, close) = match (shape.tool, shape.form) {
                (Tool::Ellipse, shapes::FORM_DOUBLE) => ("(((", ")))"),
                (Tool::Ellipse, _) => ("((", "))"),
                (_, shapes::FORM_DIAMOND) => ("{", "}"),
                (_, shapes::FORM_CYLINDER) => ("[(", ")]"),
                (_, shapes::FORM_STADIUM) => ("([", "])"),
                (_, shapes::FORM_SUBROUTINE) => ("[[", "]]"),
                (_, shapes::FORM_HEXAGON) => ("{{", "}}"),
                (_, shapes::FORM_LEAN_RIGHT) => ("[/", "/]"),
                (_, shapes::FORM_LEAN_LEFT) => ("[\\", "\\]"),
                (_, shapes::FORM_TRAPEZOID) => ("[/", "\\]"),
                (_, shapes::FORM_TRAPEZOID_DOWN) => ("[\\", "/]"),
                (_, shapes::FORM_FLAG) => (">", "]"),
                _ if shape.radius >= 14.0 && !shape.sketch => ("(", ")"),
                _ => ("[", "]"),
            };
            format!("{}{open}\"{words}\"{close}", name(id))
        };
        let mut written: Vec<u32> = Vec::new();
        let mut write_box = |out: &mut String, id: u32, depth: usize, written: &mut Vec<u32>| {
            if !written.contains(&id) {
                *out += &format!("{}{}\n", "    ".repeat(depth), node(id));
                written.push(id);
            }
        };
        // Boxes holding others as subgraphs, what is in them inside.
        #[allow(clippy::too_many_arguments)]
        fn group(studio: &Studio, out: &mut String, id: u32, depth: usize, held: &[u32], boxes: &[u32], name: &dyn Fn(u32) -> String, words_of: &dyn Fn(u32) -> String, write_box: &mut dyn FnMut(&mut String, u32, usize, &mut Vec<u32>), written: &mut Vec<u32>) {
            *out += &format!("{}subgraph {}[\"{}\"]\n", "    ".repeat(depth), name(id), words_of(id));
            written.push(id);
            let members: Vec<u32> = studio.doc.stacks.iter().find(|stack| stack.holder == Some(id)).map(|stack| stack.members.clone()).unwrap_or_default();
            for member in members.into_iter().filter(|member| boxes.contains(member)) {
                if held.contains(&member) {
                    group(studio, out, member, depth + 1, held, boxes, name, words_of, write_box, written);
                } else {
                    write_box(out, member, depth + 1, written);
                }
            }
            *out += &format!("{}end\n", "    ".repeat(depth));
        }
        for &id in &held {
            if self.holder_of(id).is_none_or(|outer| !boxes.contains(&outer)) {
                group(self, &mut out, id, 1, &held, &boxes, &name, &words_of, &mut write_box, &mut written);
            }
        }
        for &id in &boxes {
            write_box(&mut out, id, 1, &mut written);
        }
        for (arrow, from, to) in links {
            let Some(index) = self.index_of(arrow) else { continue };
            let Kind::Shape(shape) = &self.doc.layers[index].kind else { continue };
            let headed = shape.heads[1] || shape.tool == Tool::Arrow;
            let op = match (headed, shape.dashed || shape.dotted, shape.stroke > LINE * 1.5) {
                (true, true, _) => "-.->",
                (true, _, true) => "==>",
                (true, _, _) => "-->",
                (false, true, _) => "-.-",
                (false, _, true) => "===",
                (false, _, _) => "---",
            };
            let op = match (headed, shape.ends[1]) {
                (true, shapes::CROSS_HEAD) => op.replace('>', "x"),
                (true, shapes::RING_HEAD) => op.replace('>', "o"),
                _ => op.to_owned(),
            };
            let label = if shape.text.trim().is_empty() { String::new() } else { format!("|\"{}\"|", shape.text.replace('\n', " ").replace('"', "'")) };
            out += &format!("    {} {op}{label} {}\n", name(from), name(to));
        }
        out
    }

    /// Whether a map goes down from a box.
    fn goes_down_from(&self, id: u32) -> bool {
        let Some(index) = self.index_of(id) else { return false };
        let (x, y, w, h) = self.box_frame(index);
        let kids: Vec<(f32, f32, f32, f32)> = self.child_boxes(id).into_iter().filter_map(|kid| self.index_of(kid)).map(|kid| self.box_frame(kid)).collect();
        let (across, down) = kids.iter().fold((0.0f32, 0.0f32), |(a, d), k| (a + ((k.0 + k.2 / 2.0) - (x + w / 2.0)).abs(), d + ((k.1 + k.3 / 2.0) - (y + h / 2.0)).abs()));
        down > across
    }

    /// Mermaid of what is picked out put on the clipboard and written
    /// beside the file as an `.mmd`.
    pub(in crate::studio) fn copy_mermaid(&mut self, mindmap: bool) {
        let text = self.to_mermaid(mindmap);
        let dir = self.export_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.mmd", one_line(&self.pages[self.page].name).replace('/', "-")));
        let wrote = if cfg!(test) { true } else { std::fs::write(&path, &text).is_ok() };
        let copied = if cfg!(test) { Ok(()) } else { crate::clipboard::copy_text(&text) };
        match (copied, wrote) {
            (Ok(()), true) => self.say(format!("Mermaid copied, and written to {}", shortened(&path))),
            (Ok(()), false) => self.say("Mermaid copied"),
            (Err(error), _) => self.say(format!("Could not copy: {error}")),
        }
    }
}

/// What a box lays out, as a sequence; and the parts of one a sequence
/// lays out besides its people, messages and notes: a box round part of
/// it, a line across one between its branches, a bar on a lifeline while
/// someone is busy, and the people again along its foot.
pub(in crate::studio) const SEQUENCE: u8 = 3;
pub(in crate::studio) const FRAGMENT: u8 = 4;
pub(in crate::studio) const BRANCH: u8 = 5;
pub(in crate::studio) const BUSY: u8 = 6;
pub(in crate::studio) const FOOT: u8 = 7;

#[cfg(test)]
pub(in crate::studio) mod check {
    //! What a diagram brought onto a board is checked for: no two of its
    //! boxes or words over each other that should not be, and each box
    //! round others hugging them.
    use super::*;

    /// A thing on the board that should be clear of others: its place,
    /// what it says, and whether it holds others.
    #[derive(Clone, Debug)]
    pub(in crate::studio) struct Placed {
        pub id: u32,
        pub frame: (f32, f32, f32, f32),
        pub words: String,
        pub holds: bool,
        pub label: bool,
    }

    /// The boxes and words of a board: each box, each piece of text, and
    /// each arrow's words where they are drawn.
    pub(in crate::studio) fn placed(studio: &mut Studio) -> Vec<Placed> {
        let mut out = Vec::new();
        for index in 0..studio.doc.layers.len() {
            let id = studio.id_of(index);
            let holds = !studio.inside(id).is_empty();
            let Kind::Shape(shape) = studio.doc.layers[index].kind.clone() else { continue };
            match shape.tool {
                Tool::Rect | Tool::Ellipse => out.push(Placed { id, frame: shape.frame(), words: shape.text.clone(), holds: holds || shape.region == FRAGMENT || shape.region == BUSY, label: false }),
                Tool::Text => {
                    let place = shape.bounds(&mut studio.text);
                    out.push(Placed { id, frame: (place.x as f32 + 2.0, place.y as f32 + 2.0, place.w as f32 - 4.0, place.h as f32 - 4.0), words: shape.text.clone(), holds: false, label: true });
                }
                Tool::Line | Tool::Arrow if !shape.text.is_empty() => {
                    let (lines, high) = shape.label_lines(&mut studio.text);
                    let left = lines.iter().map(|line| line.1).fold(f32::MAX, f32::min);
                    let right = lines.iter().map(|line| line.1 + line.3 as f32).fold(f32::MIN, f32::max);
                    let top = lines.iter().map(|line| line.2).fold(f32::MAX, f32::min);
                    let bottom = lines.iter().map(|line| line.2 + high as f32).fold(f32::MIN, f32::max);
                    out.push(Placed { id, frame: (left, top, right - left, bottom - top), words: shape.text.clone(), holds: false, label: true });
                }
                _ => {}
            }
        }
        out
    }

    /// How far two frames go into each other, across and down; nothing if
    /// they are clear.
    pub(in crate::studio) fn overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> Option<(f32, f32)> {
        let across = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
        let down = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
        (across > 0.5 && down > 0.5).then_some((across, down))
    }

    /// No two of a board's boxes and words over each other, but a box
    /// round what it holds; a box's own words in it are its own.
    pub(in crate::studio) fn assert_clear(studio: &mut Studio) {
        let all = placed(studio);
        for (at, a) in all.iter().enumerate() {
            for b in &all[at + 1..] {
                let nested = studio.inside(a.id).contains(&b.id) || studio.inside(b.id).contains(&a.id);
                if nested || a.holds || b.holds {
                    continue;
                }
                if let Some(by) = overlap(a.frame, b.frame) {
                    panic!("{:?} {:?} and {:?} {:?} overlap by {by:?}", a.words, a.frame, b.words, b.frame);
                }
            }
        }
    }

    /// Every box's words inside it, in the board's lettering and in the
    /// plain face the clean look draws them in.
    pub(in crate::studio) fn assert_words_fit(studio: &mut Studio) {
        for layer in studio.doc.layers.clone() {
            let Kind::Shape(shape) = &layer.kind else { continue };
            if !matches!(shape.tool, Tool::Rect | Tool::Ellipse) || shape.text.is_empty() {
                continue;
            }
            let (x, y, w, h) = shape.frame();
            for face in [LETTERING, 0] {
                let mut drawn = shape.clone();
                drawn.face = face;
                let (lines, high) = drawn.label_lines(&mut studio.text);
                for (line, left, top, wide) in lines {
                    let inside = left >= x - 0.5 && left + wide as f32 <= x + w + 0.5 && top >= y - 0.5 && top + high as f32 <= y + h + 2.0;
                    assert!(inside, "{line:?} of {:?} in face {face} at {:?} is out of {:?}", shape.text, (left, top, wide, high), (x, y, w, h));
                }
            }
        }
    }

    /// Words that should be clear of every box but the one they are in.
    pub(in crate::studio) fn assert_labels_clear(studio: &mut Studio) {
        let all = placed(studio);
        for label in all.iter().filter(|thing| thing.label) {
            for other in all.iter().filter(|thing| thing.id != label.id && !thing.holds) {
                if let Some(by) = overlap(label.frame, other.frame) {
                    panic!("the words {:?} {:?} are over {:?} {:?} by {by:?}", label.words, label.frame, other.words, other.frame);
                }
            }
        }
    }

    /// Each box holding others round them with room on every side, no
    /// more than `most` on any side but its top, where its title is.
    pub(in crate::studio) fn assert_hugs(studio: &mut Studio, most: f32) {
        for index in 0..studio.doc.layers.len() {
            let id = studio.id_of(index);
            let members = studio.inside(id);
            if members.is_empty() {
                continue;
            }
            // What it holds by its boxes, its lines' points and their words.
            let words: Vec<Placed> = placed(studio);
            let mut frames: Vec<(f32, f32, f32, f32)> = Vec::new();
            for &member in &members {
                let Some(at) = studio.index_of(member) else { continue };
                let Kind::Shape(shape) = &studio.doc.layers[at].kind else { continue };
                match shape.tool {
                    Tool::Rect | Tool::Ellipse => frames.push(shape.frame()),
                    _ => {
                        let path = if shape.sticks() { shape.path() } else { shape.points.clone() };
                        let (left, top) = path.iter().fold((f32::MAX, f32::MAX), |(l, t), p| (l.min(p.0), t.min(p.1)));
                        let (right, bottom) = path.iter().fold((f32::MIN, f32::MIN), |(r, b), p| (r.max(p.0), b.max(p.1)));
                        frames.push((left, top, right - left, bottom - top));
                        frames.extend(words.iter().filter(|thing| thing.id == member).map(|thing| thing.frame));
                    }
                }
            }
            let (cx, cy, cw, ch) = actions::union(&frames).unwrap();
            let (x, y, w, h) = studio.box_frame(index);
            let sides = [cx - x, (x + w) - (cx + cw), (y + h) - (cy + ch)];
            assert!(sides.iter().all(|&side| side >= 4.0 && side <= most) && cy - y >= 4.0 && cy - y <= most + 60.0, "{:?} round its {} is {:?} for {:?}: sides {sides:?}, top {}", match &studio.doc.layers[index].kind { Kind::Shape(shape) => shape.text.clone(), _ => String::new() }, members.len(), (x, y, w, h), (cx, cy, cw, ch), cy - y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    pub(super) fn shapes_of(studio: &Studio) -> Vec<Shape> {
        studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) => Some(shape.clone()),
            Kind::Picture(_) => None,
        }).collect()
    }

    #[test]
    fn mermaid_is_known_by_its_first_word_after_its_front_matter() {
        assert_eq!(kind_of("---\ntitle: Hello\n---\nflowchart LR\n  A --> B"), Some("flowchart"));
        assert_eq!(front_title("---\ntitle: \"Hello\"\n---\nflowchart LR\n"), Some("Hello".to_owned()));
        assert_eq!(kind_of("%%{init: {}}%%\nsequenceDiagram\n"), Some("sequenceDiagram"));
        assert_eq!(kind_of("quadrantChart\n"), Some("quadrantChart"));
        assert_eq!(kind_of("gitGraph:\n commit"), Some("gitGraph"));
        assert_eq!(kind_of("hello there"), None);
        assert_eq!(body("pie\n  %% a comment\n  \"A\" : 1 %% trailing\n"), vec!["\"A\" : 1".to_owned()]);
    }

    #[test]
    fn colors_are_the_boards_nearest() {
        assert_eq!(css_color("#f9f"), Some(0xFF99FF));
        assert_eq!(board_fill(0xFF99FF), FILLS[4]);
        assert_eq!(board_fill(0x99FF99), FILLS[1]);
        assert_eq!(board_fill(0xEEEEEE), 0xFFFFFF);
        assert_eq!(board_stroke(0x333333), INK);
        assert_eq!(board_stroke(0xFF0000), STROKES[1]);
    }

    /// Each `.mmd` in `$HOTSHOT_MERMAID` brought onto a board and written
    /// as a PNG into `$HOTSHOT_SNAPSHOTS`, rounded on light paper, and with
    /// `$HOTSHOT_LOOKS` set, sketchy on dark gray and clean on black too.
    ///
    ///     HOTSHOT_MERMAID=mmd HOTSHOT_SNAPSHOTS=shots cargo test mermaid_snapshots -- --ignored
    #[test]
    #[ignore]
    fn mermaid_snapshots() {
        let (Some(from), Some(to)) = (std::env::var_os("HOTSHOT_MERMAID"), std::env::var_os("HOTSHOT_SNAPSHOTS")) else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(from).unwrap().filter_map(|entry| entry.ok().map(|entry| entry.path())).filter(|path| path.extension().is_some_and(|ending| ending == "mmd")).collect();
        files.sort();
        let looks: &[(u8, u8, &str)] = if std::env::var_os("HOTSHOT_LOOKS").is_some() { &[(0, 0, ""), (1, 1, "-sketchy-dark"), (2, 2, "-clean-black")] } else { &[(0, 0, "")] };
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            for &(look, ground, suffix) in looks {
                let mut studio = board();
                (studio.pages[0].look, studio.pages[0].ground) = (look, ground);
                assert!(studio.bring_mermaid(&text), "{}", file.display());
                studio.selected.clear();
                studio.changed();
                let layers: Vec<Layer> = studio.doc.layers.clone();
                let boxes: Vec<(f32, f32, f32, f32)> = layers.iter().map(|layer| layer.bounds(&mut studio.text)).collect();
                let (x, y, w, h) = actions::union(&boxes).unwrap();
                let sheet = studio.part_sheet(&layers, (x - 24.0, y - 24.0, w + 48.0, h + 48.0));
                let (pw, ph, pixels) = sheet.render(1.0, &mut studio.text);
                let name = file.file_stem().unwrap().to_string_lossy().to_string();
                std::fs::write(to.join(format!("{name}{suffix}.png")), crate::output::encode(pw as u32, ph as u32, &pixels, false).unwrap()).unwrap();
            }
        }
    }

    #[test]
    fn every_kind_comes_in_on_a_board() {
        for text in ["flowchart LR\n A --> B", "sequenceDiagram\n A->>B: hi", "classDiagram\n A <|-- B", "stateDiagram-v2\n [*] --> A", "erDiagram\n A ||--o{ B : has", "gantt\n section S\n T :a, 2024-01-01, 3d", "pie\n \"A\" : 1", "mindmap\n root\n  child", "timeline\n 2020 : thing", "journey\n section S\n Do: 5: Me", "gitGraph\n commit", "quadrantChart\n A: [0.5, 0.5]"] {
            let mut studio = board();
            assert!(studio.bring_mermaid(text), "{text}");
            assert!(!shapes_of(&studio).is_empty(), "{text}");
        }
    }
}
