//! Mermaid, read and written by a parser of hotshot's own, with no script
//! engine: text pasted on a board, or typed into a label and Ctrl+Enter,
//! becomes the board's own boxes and arrows, to move and change as any.
//! Flowcharts, with their shapes, edges, labels and subgraphs as boxes in
//! boxes; sequence diagrams, as a sequence that keeps itself laid out;
//! class, state and ER diagrams; gantt charts, pies, mind maps, timelines,
//! journeys, git graphs and quadrant charts. Flowcharts, sequences and
//! mind maps are written back as Mermaid too.

use super::*;

/// The kinds of diagram read, by the word they begin with.
pub(in crate::studio) const KINDS: [&str; 13] = ["flowchart", "graph", "sequenceDiagram", "classDiagram", "stateDiagram-v2", "stateDiagram", "erDiagram", "gantt", "pie", "mindmap", "timeline", "journey", "gitGraph"];

/// What a box of a diagram is drawn as.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Form {
    Box,
    Round,
    Circle,
    Diamond,
    Database,
    Note,
}

/// Which kind of diagram some text is, by its first word, if it is one.
pub(in crate::studio) fn kind_of(text: &str) -> Option<&'static str> {
    let first = text.lines().map(str::trim).find(|line| !line.is_empty() && !line.starts_with("%%") && !line.starts_with("---"))?;
    let word = first.split_whitespace().next()?.trim_end_matches(':');
    if word == "quadrantChart" {
        return Some("quadrantChart");
    }
    KINDS.iter().find(|known| **known == word).copied()
}

/// The lines of a diagram after its first, comments and blank ones gone.
fn body(text: &str) -> Vec<String> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("%%"));
    lines.next();
    lines.map(|line| line.split("%%").next().unwrap_or("").trim().to_owned()).filter(|line| !line.is_empty()).collect()
}

/// Words with their quotes and Mermaid's line breaks taken out.
fn plain(words: &str) -> String {
    words.trim().trim_matches('"').replace("<br/>", "\n").replace("<br>", "\n").replace("&quot;", "\"").trim().to_owned()
}

/// A node of a flowchart at the start of some text: its name, its words
/// and its shape if they are given, and how much of the text it took.
fn node_at(text: &str) -> Option<(String, Option<(String, Form)>, usize)> {
    let name_len = text.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.')).unwrap_or(text.len());
    // A name can end in a dash only if no edge follows it there.
    let mut name_len = name_len;
    while name_len > 0 && (text[..name_len].ends_with('-') || text[..name_len].ends_with('.')) {
        name_len -= 1;
    }
    if name_len == 0 {
        return None;
    }
    let name = text[..name_len].to_owned();
    let rest = &text[name_len..];
    const SHAPES: [(&str, &str, Form); 10] = [("[(", ")]", Form::Database), ("((", "))", Form::Circle), ("([", "])", Form::Round), ("[[", "]]", Form::Box), ("{{", "}}", Form::Diamond), ("[/", "/]", Form::Box), ("[\\", "\\]", Form::Box), ("[", "]", Form::Box), ("(", ")", Form::Round), ("{", "}", Form::Diamond)];
    for (open, close, form) in SHAPES {
        if let Some(inner) = rest.strip_prefix(open) {
            let end = inner.find(close)?;
            return Some((name, Some((plain(&inner[..end]), form)), name_len + open.len() + end + close.len()));
        }
    }
    if let Some(inner) = rest.strip_prefix('>') {
        let end = inner.find(']')?;
        return Some((name, Some((plain(&inner[..end]), Form::Box)), name_len + 1 + end + 1));
    }
    Some((name, None, name_len))
}

/// An edge of a flowchart at the start of some text: whether it has a
/// head at its end and at its start, is dashed, is thick, its words, and
/// how much of the text it took.
fn edge_at(text: &str) -> Option<(bool, bool, bool, bool, String, usize)> {
    const PLAIN: [&str; 14] = ["<-->", "<==>", "-.->", "-..->", "-.-", "==>", "===", "-->", "---", "--x", "--o", "-->>", "~~~", "<--"];
    let mut found: Option<(&str, usize)> = PLAIN.iter().filter(|op| text.starts_with(**op)).map(|op| (*op, op.len())).max_by_key(|(_, len)| *len);
    let mut words = String::new();
    // An edge with its words in it: -- words --> or == words ==> or -. words .->
    if found.is_none() {
        for (open, closes) in [("--", ["-->", "---", "--x", "--o"]), ("==", ["==>", "===", "==x", "==o"]), ("-.", [".->", ".-", ".->", ".-"])] {
            if let Some(after) = text.strip_prefix(open) {
                if let Some((at, close)) = closes.iter().filter_map(|close| after.find(close).map(|at| (at, *close))).min_by_key(|(at, _)| *at) {
                    words = plain(&after[..at]);
                    let op = match (open, close) {
                        ("-.", ".->") => "-.->",
                        ("-.", _) => "-.-",
                        (_, close) => close,
                    };
                    found = Some((op, open.len() + at + close.len()));
                    break;
                }
            }
        }
    }
    let (op, mut used) = found?;
    let rest = &text[used..];
    let trimmed = rest.trim_start();
    if let Some(inner) = trimmed.strip_prefix('|') {
        let end = inner.find('|')?;
        words = plain(&inner[..end]);
        used += rest.len() - trimmed.len() + 1 + end + 1;
    }
    let head = op.ends_with('>') || op.ends_with('x') || op.ends_with('o');
    Some((head, op.starts_with('<'), op.contains('.'), op.contains('='), words, used))
}

/// A flowchart read: its nodes, each with its words and shape, in order;
/// its edges, from, to, heads, dashed, thick and words; its subgraphs, each
/// with a name, a title and what is in it; and whether it goes down.
#[derive(Default, Debug)]
pub(in crate::studio) struct Flow {
    nodes: Vec<(String, String, Option<Form>)>,
    edges: Vec<(String, String, bool, bool, bool, bool, String)>,
    groups: Vec<(String, String, Vec<String>)>,
    down: bool,
}

pub(in crate::studio) fn read_flow(text: &str) -> Flow {
    let mut flow = Flow::default();
    let first = text.lines().map(str::trim).find(|line| !line.is_empty() && !line.starts_with("%%")).unwrap_or("");
    flow.down = !matches!(first.split_whitespace().nth(1), Some("LR" | "RL"));
    let mut open: Vec<usize> = Vec::new();
    let add = |flow: &mut Flow, open: &[usize], name: &str, given: Option<(String, Form)>| {
        match flow.nodes.iter_mut().find(|(known, ..)| known == name) {
            Some(node) => {
                if let Some((words, form)) = given {
                    (node.1, node.2) = (words, Some(form));
                }
            }
            None => {
                let (words, form) = given.map_or((name.to_owned(), None), |(words, form)| (words, Some(form)));
                flow.nodes.push((name.to_owned(), words, form));
            }
        }
        if let Some(&group) = open.last() {
            if !flow.groups[group].2.contains(&name.to_owned()) && flow.groups.iter().all(|(known, ..)| known != name) {
                flow.groups[group].2.push(name.to_owned());
            }
        }
    };
    for line in body(text).iter().flat_map(|line| line.split(';').map(str::trim).filter(|part| !part.is_empty()).map(str::to_owned).collect::<Vec<_>>()) {
        let first_word = line.split_whitespace().next().unwrap_or("");
        match first_word {
            "subgraph" => {
                let rest = line["subgraph".len()..].trim();
                let (name, title) = match rest.split_once('[') {
                    Some((name, title)) => (name.trim().to_owned(), plain(title.trim_end_matches(']'))),
                    None => (rest.to_owned(), plain(rest)),
                };
                if let Some(&outer) = open.last() {
                    flow.groups[outer].2.push(name.clone());
                }
                flow.groups.push((name, title, Vec::new()));
                open.push(flow.groups.len() - 1);
                continue;
            }
            "end" => {
                open.pop();
                continue;
            }
            "direction" | "classDef" | "class" | "style" | "linkStyle" | "click" | "accTitle" | "accDescr" => continue,
            _ => {}
        }
        // A chain: nodes, each group of them joined by &, between edges.
        let mut at = 0;
        let mut before: Vec<String> = Vec::new();
        let mut pending: Option<(bool, bool, bool, bool, String)> = None;
        while at < line.len() {
            let rest = line[at..].trim_start();
            at = line.len() - rest.len();
            if rest.is_empty() {
                break;
            }
            if let Some((head, tail, dashed, thick, words, used)) = edge_at(rest) {
                pending = Some((head, tail, dashed, thick, words));
                at += used;
                continue;
            }
            let mut now: Vec<String> = Vec::new();
            let mut rest_at = at;
            loop {
                let rest = line[rest_at..].trim_start();
                rest_at = line.len() - rest.len();
                let Some((name, given, used)) = node_at(rest) else { break };
                add(&mut flow, &open, &name, given);
                now.push(name);
                rest_at += used;
                let after = line[rest_at..].trim_start();
                // A class put on it, as name:::class, left out.
                let after = if let Some(class) = after.strip_prefix(":::") { &class[class.find(char::is_whitespace).unwrap_or(class.len())..] } else { after };
                rest_at = line.len() - after.len();
                match after.strip_prefix('&') {
                    Some(more) => rest_at = line.len() - more.len(),
                    None => break,
                }
            }
            if now.is_empty() {
                break;
            }
            if let Some((head, tail, dashed, thick, words)) = pending.take() {
                for from in &before {
                    for to in &now {
                        flow.edges.push((from.clone(), to.clone(), head, tail, dashed, thick, words.clone()));
                    }
                }
            }
            before = now;
            at = rest_at;
        }
    }
    flow
}

impl Studio {
    /// A box of a diagram, its words in it, sized to them, drawn as a
    /// board draws one; where its middle is.
    fn diagram_box(&mut self, words: &str, form: Form, middle: (f32, f32)) -> usize {
        let tool = if form == Form::Circle { Tool::Ellipse } else { Tool::Rect };
        let mut shape = Shape::new(tool, INK, LINE, middle);
        let making = self.board.making;
        self.board.making = match form {
            Form::Diamond => shapes::FORM_DIAMOND,
            Form::Database => shapes::FORM_CYLINDER,
            Form::Note => shapes::FORM_STICKY,
            _ => shapes::FORM_BOX,
        };
        self.board_dress(&mut shape);
        self.board.making = making;
        if form == Form::Round {
            shape.radius = 24.0;
        }
        let least = match form {
            Form::Diamond => (150.0, 90.0),
            Form::Circle => (100.0, 100.0),
            Form::Database => (130.0, 80.0),
            _ => (140.0, 56.0),
        };
        shape.text = words.to_owned();
        shape.points = vec![(middle.0 - least.0 / 2.0, middle.1 - least.1 / 2.0), (middle.0 + least.0 / 2.0, middle.1 + least.1 / 2.0)];
        let (w, h) = shape.label_needs(&mut self.text, least, nest::MOST_WIDE);
        let (w, h) = if form == Form::Circle { (w.max(h), w.max(h)) } else { (w, h) };
        shape.points = vec![(middle.0 - w / 2.0, middle.1 - h / 2.0), (middle.0 + w / 2.0, middle.1 + h / 2.0)];
        shape.base = (w, h);
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// Words on the board, from their top left.
    fn diagram_words(&mut self, words: &str, at: (f32, f32), px: u32) -> usize {
        let mut shape = Shape::new(Tool::Text, INK, LINE, at);
        self.board_dress(&mut shape);
        (shape.text, shape.font_px) = (words.to_owned(), px);
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// A line or an arrow from one point to another, dressed as a board's.
    fn diagram_line(&mut self, from: (f32, f32), to: (f32, f32), head: bool, dashed: bool) -> usize {
        let mut shape = Shape::new(if head { Tool::Arrow } else { Tool::Line }, INK, LINE, from);
        self.board_dress(&mut shape);
        shape.points[1] = to;
        (shape.dashed, shape.routed) = (dashed, false);
        self.doc.layers.push(Layer::shape(shape));
        self.doc.layers.len() - 1
    }

    /// An arrow joining two of a diagram's boxes, styled as its edge says.
    fn diagram_edge(&mut self, from: usize, to: usize, head: bool, tail: bool, dashed: bool, thick: bool, words: &str) -> usize {
        let arrow = self.join(from, to);
        if let Kind::Shape(shape) = &mut self.doc.layers[arrow].kind {
            if !head && !tail {
                shape.tool = Tool::Line;
                shape.heads = [false, false];
            }
            shape.heads[0] = tail;
            (shape.dashed, shape.text) = (dashed, words.to_owned());
            if thick {
                shape.stroke *= 1.8;
            }
            shape.touch();
        }
        arrow
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
            "flowchart" | "graph" => self.build_flow(read_flow(text), origin),
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
        self.selected = (start..self.doc.layers.len()).collect();
        self.changed();
        self.say(format!("Mermaid {kind}: {} shapes", self.doc.layers.len() - start));
        true
    }

    /// Boxes joined as a graph laid out as trees from their tops, those
    /// with nothing coming in, one after another; cycles from their first.
    fn lay_out_graph(&mut self, boxes: &[usize], down: bool, origin: (f32, f32)) {
        let ids: Vec<u32> = boxes.iter().map(|&index| self.id_of(index)).collect();
        let mut placed: Vec<u32> = Vec::new();
        let mut along = 0.0;
        let mut roots: Vec<u32> = ids.iter().copied().filter(|&id| self.parent_box(id).is_none_or(|parent| !ids.contains(&parent))).collect();
        let rest: Vec<u32> = ids.iter().copied().filter(|id| !roots.contains(id)).collect();
        roots.extend(rest);
        for root in roots {
            if placed.contains(&root) {
                continue;
            }
            let Some(index) = self.index_of(root) else { continue };
            let (x, y, w, h) = self.box_frame(index);
            let span = if down { self.span_of(root) } else { self.span(root, false, &mut Vec::new()) };
            let target = if down { (origin.0 + along + span / 2.0 - w / 2.0, origin.1) } else { (origin.0, origin.1 + along + span / 2.0 - h / 2.0) };
            self.doc.layers[index].translate(target.0 - x, target.1 - y);
            self.lay_out_tree(root, down, false);
            // What it reached, so as not to place it again.
            let mut reach = vec![root];
            let mut at = 0;
            while at < reach.len() {
                for child in self.child_boxes(reach[at]) {
                    if !reach.contains(&child) {
                        reach.push(child);
                    }
                }
                at += 1;
            }
            placed.extend(reach);
            along += span + 80.0;
        }
    }

    fn build_flow(&mut self, flow: Flow, origin: (f32, f32)) {
        let mut made: Vec<(String, usize)> = Vec::new();
        for (name, words, form) in &flow.nodes {
            if flow.groups.iter().any(|(group, ..)| group == name) {
                continue;
            }
            let index = self.diagram_box(words, form.unwrap_or(Form::Box), origin);
            made.push((name.clone(), index));
        }
        let find = |made: &[(String, usize)], name: &str| made.iter().find(|(known, _)| known == name).map(|&(_, index)| index);
        let mut later = Vec::new();
        for edge in &flow.edges {
            match (find(&made, &edge.0), find(&made, &edge.1)) {
                (Some(a), Some(b)) => drop(self.diagram_edge(a, b, edge.2, edge.3, edge.4, edge.5, &edge.6)),
                _ => later.push(edge.clone()),
            }
        }
        // Room between levels for the words on the edges.
        let worded = flow.edges.iter().map(|edge| edge.6.chars().count()).max().unwrap_or(0) as f32;
        let level = self.board.level;
        self.board.level = (80.0 + worded * 9.0).min(260.0);
        self.changed();
        let boxes: Vec<usize> = made.iter().map(|&(_, index)| index).collect();
        self.lay_out_graph(&boxes, flow.down, origin);
        self.board.level = level;
        self.changed();
        // Subgraphs, the innermost first, as boxes round what is in them.
        for (name, title, members) in flow.groups.iter().rev() {
            let inside: Vec<usize> = members.iter().filter_map(|member| find(&made, member)).collect();
            let frames: Vec<(f32, f32, f32, f32)> = inside.iter().map(|&index| self.box_frame(index)).collect();
            let Some((x, y, w, h)) = actions::union(&frames) else { continue };
            let holder = self.diagram_box(title, Form::Box, (x + w / 2.0, y + h / 2.0));
            if let Kind::Shape(shape) = &mut self.doc.layers[holder].kind {
                shape.points = vec![(x - 20.0, y - 56.0), (x + w + 20.0, y + h + 20.0)];
                (shape.titled, shape.heading, shape.filled) = (true, true, false);
                shape.base = (w + 40.0, h + 76.0);
            }
            // Under what it holds.
            let layer = self.doc.layers.remove(holder);
            let at = inside.iter().min().copied().unwrap_or(0);
            self.doc.layers.insert(at, layer);
            for entry in &mut made {
                if entry.1 >= at {
                    entry.1 += 1;
                }
            }
            let holder_id = self.id_of(at);
            for member in members {
                if let Some(index) = find(&made, member) {
                    let id = self.id_of(index);
                    self.nest(id, holder_id);
                }
            }
            made.push((name.clone(), at));
        }
        for edge in later {
            if let (Some(a), Some(b)) = (find(&made, &edge.0), find(&made, &edge.1)) {
                self.diagram_edge(a, b, edge.2, edge.3, edge.4, edge.5, &edge.6);
            }
        }
    }

    /// A class, state or entity diagram's boxes and joins, laid out.
    fn build_graph(&mut self, boxes: Vec<(String, String, Form)>, joins: Vec<(String, String, bool, bool, String)>, origin: (f32, f32), down: bool) {
        let mut made: Vec<(String, usize)> = Vec::new();
        for (name, words, form) in boxes {
            let index = self.diagram_box(&words, form, origin);
            made.push((name, index));
        }
        for (from, to, head, dashed, words) in joins {
            let find = |name: &str| made.iter().find(|(known, _)| known == name).map(|&(_, index)| index);
            if let (Some(a), Some(b)) = (find(&from), find(&to)) {
                self.diagram_edge(a, b, head, false, dashed, false, &words);
            }
        }
        self.changed();
        let indices: Vec<usize> = made.iter().map(|&(_, index)| index).collect();
        self.lay_out_graph(&indices, down, origin);
    }

    fn build_class(&mut self, text: &str, origin: (f32, f32)) {
        let mut classes: Vec<(String, Vec<String>)> = Vec::new();
        let mut joins = Vec::new();
        let mut open: Option<usize> = None;
        let class = |classes: &mut Vec<(String, Vec<String>)>, name: &str| match classes.iter().position(|(known, _)| known == name) {
            Some(at) => at,
            None => {
                classes.push((name.to_owned(), Vec::new()));
                classes.len() - 1
            }
        };
        const LINKS: [(&str, bool); 10] = [("<|--", true), ("--|>", true), ("*--", false), ("o--", false), ("-->", true), ("<--", true), ("..|>", true), ("..>", true), ("--", false), ("..", false)];
        for line in body(text) {
            if line == "}" {
                open = None;
                continue;
            }
            if let Some(at) = open {
                classes[at].1.push(line.clone());
                continue;
            }
            if let Some(rest) = line.strip_prefix("class ") {
                let name = rest.trim_end_matches('{').trim().split(['<', ' ']).next().unwrap_or("").to_owned();
                let at = class(&mut classes, &name);
                if line.ends_with('{') {
                    open = Some(at);
                }
                continue;
            }
            if let Some((link, head)) = LINKS.iter().find(|(link, _)| line.contains(*link)) {
                let (left, right) = line.split_once(*link).unwrap_or(("", ""));
                let (right, words) = right.split_once(':').map_or((right, ""), |(right, words)| (right, words));
                let clean = |side: &str| side.trim().split_whitespace().filter(|word| !word.starts_with('"')).last().unwrap_or("").to_owned();
                let (mut a, mut b) = (clean(left), clean(right));
                if link.starts_with('<') || link.starts_with('*') || link.starts_with('o') {
                    std::mem::swap(&mut a, &mut b);
                }
                class(&mut classes, &a);
                class(&mut classes, &b);
                joins.push((a, b, *head, link.contains(".."), plain(words)));
                continue;
            }
            // Name : member
            if let Some((name, member)) = line.split_once(':') {
                let at = class(&mut classes, name.trim());
                classes[at].1.push(member.trim().to_owned());
            }
        }
        let boxes = classes.into_iter().map(|(name, members)| {
            let words = if members.is_empty() { name.clone() } else { format!("{name}\n{}", members.join("\n")) };
            (name, words, Form::Box)
        }).collect();
        self.build_graph(boxes, joins, origin, true);
    }

    fn build_state(&mut self, text: &str, origin: (f32, f32)) {
        let mut states: Vec<(String, String, Form)> = Vec::new();
        let mut joins = Vec::new();
        let mut starts = 0;
        for line in body(text) {
            if let Some((left, right)) = line.split_once("-->") {
                let (right, words) = right.split_once(':').map_or((right, ""), |(right, words)| (right, words));
                let mut name = |side: &str, first: bool| -> String {
                    let side = side.trim();
                    if side == "[*]" {
                        starts += 1;
                        let name = format!("[*]{starts}");
                        states.push((name.clone(), if first { "\u{25cf}".into() } else { "\u{25c9}".into() }, Form::Circle));
                        return name;
                    }
                    if !states.iter().any(|(known, ..)| known == side) {
                        states.push((side.to_owned(), side.to_owned(), Form::Round));
                    }
                    side.to_owned()
                };
                let (a, b) = (name(left, true), name(right, false));
                joins.push((a, b, true, false, plain(words)));
            } else if let Some(rest) = line.strip_prefix("state ") {
                let name = rest.split_whitespace().last().unwrap_or("").trim_end_matches('{').to_owned();
                if !name.is_empty() && !states.iter().any(|(known, ..)| *known == name) {
                    states.push((name.clone(), name, Form::Round));
                }
            } else if let Some((name, words)) = line.split_once(':') {
                if let Some(state) = states.iter_mut().find(|(known, ..)| known == name.trim()) {
                    state.1 = format!("{}\n{}", state.1, words.trim());
                }
            }
        }
        self.build_graph(states, joins, origin, true);
    }

    fn build_er(&mut self, text: &str, origin: (f32, f32)) {
        let mut entities: Vec<(String, Vec<String>)> = Vec::new();
        let mut joins = Vec::new();
        let mut open: Option<usize> = None;
        let entity = |entities: &mut Vec<(String, Vec<String>)>, name: &str| match entities.iter().position(|(known, _)| known == name) {
            Some(at) => at,
            None => {
                entities.push((name.to_owned(), Vec::new()));
                entities.len() - 1
            }
        };
        for line in body(text) {
            if line == "}" {
                open = None;
                continue;
            }
            if let Some(at) = open {
                entities[at].1.push(line.split_whitespace().take(2).collect::<Vec<_>>().join(" "));
                continue;
            }
            if let Some(name) = line.strip_suffix('{') {
                open = Some(entity(&mut entities, name.trim()));
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.len() >= 3 && (words[1].contains("--") || words[1].contains("..")) {
                let (a, link, b) = (words[0], words[1], words[2]);
                entity(&mut entities, a);
                entity(&mut entities, b);
                let label = line.split_once(':').map_or(String::new(), |(_, label)| plain(label));
                let ends = |part: &str| match part {
                    "||" => "1",
                    "|o" | "o|" => "0..1",
                    "}o" | "o{" => "0..*",
                    "}|" | "|{" => "1..*",
                    _ => "",
                };
                let (left, right) = (ends(&link[..2.min(link.len())]), ends(&link[link.len().saturating_sub(2)..]));
                joins.push((a.to_owned(), b.to_owned(), false, link.contains(".."), format!("{left} {label} {right}").trim().to_owned()));
            }
        }
        let boxes = entities.into_iter().map(|(name, attributes)| {
            let words = if attributes.is_empty() { name.clone() } else { format!("{name}\n{}", attributes.join("\n")) };
            (name, words, Form::Box)
        }).collect();
        self.build_graph(boxes, joins, origin, false);
    }

    fn build_mindmap(&mut self, text: &str, origin: (f32, f32)) {
        let lines: Vec<(usize, String)> = text.lines().skip_while(|line| !line.trim().starts_with("mindmap")).skip(1).filter(|line| !line.trim().is_empty() && !line.trim().starts_with("%%")).map(|line| {
            let indent = line.chars().take_while(|c| c.is_whitespace()).count();
            let words = line.trim();
            let words = match node_at(words) {
                Some((_, Some((words, _)), _)) => words,
                _ => words.trim_start_matches("::icon(").to_owned(),
            };
            (indent, words)
        }).filter(|(_, words)| !words.starts_with("::icon")).collect();
        let mut stack: Vec<(usize, usize)> = Vec::new();
        let mut root = None;
        for (depth, (indent, words)) in lines.into_iter().enumerate() {
            let form = if depth == 0 { Form::Circle } else { Form::Round };
            let index = self.diagram_box(&words, form, origin);
            while stack.last().is_some_and(|&(level, _)| level >= indent) {
                stack.pop();
            }
            match stack.last() {
                Some(&(_, parent)) => drop(self.join(parent, index)),
                None => root = root.or(Some(index)),
            }
            stack.push((indent, index));
        }
        self.changed();
        if let Some(root) = root {
            // A mind map's branches either side of it, half to each.
            let id = self.id_of(root);
            let kids = self.child_boxes(id);
            for (at, kid) in kids.iter().enumerate() {
                if let Some(index) = self.index_of(*kid) {
                    let side = if at % 2 == 0 { 1.0 } else { -1.0 };
                    let (x, ..) = self.box_frame(index);
                    self.doc.layers[index].translate(origin.0 + side * 300.0 - x, 0.0);
                }
            }
            self.lay_out_tree(id, false, true);
        }
    }

    fn build_sequence(&mut self, text: &str, origin: (f32, f32)) {
        let mut people: Vec<(String, String, bool)> = Vec::new();
        let mut steps: Vec<Step> = Vec::new();
        let person = |people: &mut Vec<(String, String, bool)>, name: &str| {
            if !people.iter().any(|(known, ..)| known == name) {
                people.push((name.to_owned(), name.to_owned(), false));
            }
        };
        for line in body(text) {
            let lower = line.to_lowercase();
            if lower.starts_with("participant ") || lower.starts_with("actor ") {
                let actor = lower.starts_with("actor ");
                let rest = line.split_once(' ').map_or("", |(_, rest)| rest).trim();
                let (name, alias) = rest.split_once(" as ").map_or((rest, rest), |(name, alias)| (name.trim(), alias.trim()));
                people.retain(|(known, ..)| known != name);
                people.push((name.to_owned(), plain(alias), actor));
                continue;
            }
            if lower.starts_with("note ") {
                if let Some((place, words)) = line.split_once(':') {
                    let over: Vec<String> = place.split_whitespace().skip(3).collect::<Vec<_>>().join(" ").split(',').map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()).collect();
                    let over = if over.is_empty() { place.split_whitespace().skip(2).map(str::to_owned).collect() } else { over };
                    steps.push(Step::Note(over, plain(words)));
                }
                continue;
            }
            if let Some(word) = ["alt", "loop", "opt", "par", "critical", "break", "rect"].iter().find(|word| lower.split_whitespace().next() == Some(**word)) {
                steps.push(Step::Open(format!("{word} {}", line[word.len()..].trim())));
                continue;
            }
            if lower.starts_with("else") || lower.starts_with("and ") {
                steps.push(Step::Else);
                continue;
            }
            if lower == "end" {
                steps.push(Step::Close);
                continue;
            }
            if lower.starts_with("activate ") || lower.starts_with("deactivate ") || lower.starts_with("autonumber") {
                continue;
            }
            const ARROWS: [(&str, bool, bool); 8] = [("-->>", true, true), ("->>", false, true), ("-->", true, true), ("->", false, true), ("--x", true, true), ("-x", false, true), ("--)", true, true), ("-)", false, true)];
            if let Some((arrow, dashed, _)) = ARROWS.iter().find(|(arrow, ..)| line.contains(*arrow)) {
                let (from, rest) = line.split_once(*arrow).unwrap_or(("", ""));
                let (to, words) = rest.split_once(':').map_or((rest, ""), |(to, words)| (to, words));
                let (from, to) = (from.trim().to_owned(), to.trim().trim_start_matches(['+', '-']).to_owned());
                person(&mut people, &from);
                person(&mut people, &to);
                steps.push(Step::Message(from, to, plain(words), *dashed));
            }
        }
        self.make_sequence(&people, &steps, origin);
    }

    /// A sequence: a box round it that keeps it laid out, its people along
    /// its top, a dashed line down from each, and its messages in order
    /// down them, each tied to the two it is between.
    fn make_sequence(&mut self, people: &[(String, String, bool)], steps: &[Step], origin: (f32, f32)) {
        let frame = self.diagram_box("Sequence", Form::Box, origin);
        if let Kind::Shape(shape) = &mut self.doc.layers[frame].kind {
            (shape.titled, shape.heading, shape.filled, shape.dashed, shape.color, shape.region, shape.sketch) = (true, true, false, true, QUIET, SEQUENCE, false);
        }
        let holder = self.id_of(frame);
        let mut boxes: Vec<(String, usize)> = Vec::new();
        for (at, (name, words, actor)) in people.iter().enumerate() {
            let index = self.diagram_box(words, if *actor { Form::Round } else { Form::Box }, (origin.0 + at as f32 * 200.0, origin.1));
            if *actor {
                if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                    shape.form = shapes::FORM_PERSON;
                    let (x, y, w, h) = shape.frame();
                    shape.points = vec![(x, y), (x + w, y + h + 50.0)];
                }
            }
            let id = self.id_of(index);
            self.nest(id, holder);
            boxes.push((name.clone(), index));
        }
        let find = |boxes: &[(String, usize)], name: &str| boxes.iter().find(|(known, _)| known == name).map(|&(_, index)| index);
        // A lifeline down from each.
        for &(_, index) in &boxes {
            let id = self.id_of(index);
            let line = self.diagram_line((0.0, 0.0), (0.0, 1.0), false, true);
            if let Kind::Shape(shape) = &mut self.doc.layers[line].kind {
                (shape.color, shape.stuck) = (QUIET, [Some((id, 0.0, 1.0)), Some((id, 0.0, 4.0))]);
            }
            let line_id = self.id_of(line);
            self.nest(line_id, holder);
        }
        let mut fragments: Vec<(String, u32)> = Vec::new();
        let mut order = 0;
        for step in steps {
            match step {
                Step::Message(from, to, words, dashed) => {
                    let (Some(a), Some(b)) = (find(&boxes, from), find(&boxes, to)) else { continue };
                    let arrow = self.diagram_line((0.0, 0.0), (1.0, 0.0), true, *dashed);
                    let (ida, idb) = (self.id_of(a), self.id_of(b));
                    if let Kind::Shape(shape) = &mut self.doc.layers[arrow].kind {
                        shape.text = words.clone();
                        shape.number = order;
                        // Its own messages loop back, a little to the right.
                        shape.stuck = if ida == idb { [Some((ida, 0.0, 2.0)), Some((idb, 0.4, 2.6))] } else { [Some((ida, 0.0, 2.0)), Some((idb, 0.0, 2.0))] };
                    }
                    let id = self.id_of(arrow);
                    self.nest(id, holder);
                    order += 1;
                }
                Step::Note(over, words) => {
                    let index = self.diagram_box(words, Form::Note, origin);
                    if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                        shape.number = order;
                    }
                    // Whom it is by, by their words, as its name.
                    self.doc.layers[index].named = over.first().and_then(|name| people.iter().find(|(known, ..)| known == name)).map(|(_, words, _)| words.clone());
                    let id = self.id_of(index);
                    self.nest(id, holder);
                    order += 1;
                }
                Step::Open(title) => fragments.push((title.clone(), order)),
                Step::Else => {}
                Step::Close => {
                    if let Some((title, from)) = fragments.pop() {
                        let index = self.diagram_box(&title, Form::Box, origin);
                        if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                            (shape.titled, shape.heading, shape.filled, shape.dashed, shape.sketch, shape.color) = (true, true, false, true, false, QUIET);
                            (shape.number, shape.votes) = (from, (order - from).min(255) as u8);
                            shape.form = shapes::FORM_BOX;
                            shape.status = 0;
                            shape.region = FRAGMENT;
                        }
                        let id = self.id_of(index);
                        self.nest(id, holder);
                    }
                }
            }
        }
        self.changed();
    }

    /// A sequence laid out: its people along its top at even gaps in the
    /// order they are across, and what happens down from them in order, a
    /// step apart, each tied to whom it is between; a box round a part of
    /// it round those steps.
    pub(in crate::studio) fn arrange_sequence(&mut self, holder: u32, dragged: &[u32]) {
        let Some(at) = self.index_of(holder) else { return };
        let (x, y, _, _) = self.box_frame(at);
        let members: Vec<u32> = self.doc.stacks.iter().find(|stack| stack.holder == Some(holder)).map(|stack| stack.members.clone()).unwrap_or_default();
        let shape_of = |studio: &Studio, id: u32| studio.index_of(id).and_then(|index| match &studio.doc.layers[index].kind {
            Kind::Shape(shape) => Some((index, shape.clone())),
            Kind::Picture(_) => None,
        });
        let mut people: Vec<(u32, usize, Shape)> = members.iter().filter_map(|&id| shape_of(self, id).map(|(index, shape)| (id, index, shape))).filter(|(_, _, shape)| matches!(shape.tool, Tool::Rect | Tool::Ellipse) && shape.form != shapes::FORM_STICKY && shape.region != FRAGMENT).collect();
        people.sort_by(|a, b| a.2.frame().0.total_cmp(&b.2.frame().0));
        let top = y + Text::line_height(WORDS_PX) as f32 + 30.0;
        let mut along = x + 30.0;
        let mut tallest: f32 = 0.0;
        for (id, index, shape) in &people {
            let (fx, fy, fw, fh) = shape.frame();
            if !dragged.contains(id) {
                self.doc.layers[*index].translate(along - fx, top - fy);
            }
            along += fw.max(120.0) + 80.0;
            tallest = tallest.max(fh);
        }
        // The steps, in order down: messages and notes by where they are.
        let mut steps: Vec<(u32, usize, Shape, f32)> = members.iter().filter_map(|&id| shape_of(self, id).map(|(index, shape)| (id, index, shape))).filter(|(_, _, shape)| shape.tool == Tool::Arrow || shape.form == shapes::FORM_STICKY).map(|(id, index, shape)| {
            let mid = if shape.tool == Tool::Arrow { shape.middle().1 } else { shape.frame().1 + shape.frame().3 / 2.0 };
            (id, index, shape, mid)
        }).collect();
        steps.sort_by(|a, b| a.3.total_cmp(&b.3).then(a.2.number.cmp(&b.2.number)));
        let first = top + tallest + 50.0;
        let step = 56.0;
        let person_at = |studio: &Studio, id: u32| studio.index_of(id).map(|index| studio.box_frame(index));
        for (order, (id, index, shape, _)) in steps.iter().enumerate() {
            let level = first + order as f32 * step;
            if shape.tool == Tool::Arrow {
                // A message let go of whom it was between, as when it is
                // dragged, between those nearest its ends.
                let nearest = |x: f32| people.iter().min_by(|p, q| ((p.2.frame().0 + p.2.frame().2 / 2.0) - x).abs().total_cmp(&((q.2.frame().0 + q.2.frame().2 / 2.0) - x).abs())).map(|(id, ..)| *id);
                let ends = match (shape.stuck[0], shape.stuck[1]) {
                    (Some((a, ..)), Some((b, ..))) => Some((a, b)),
                    _ => nearest(shape.points[0].0).zip(nearest(shape.points[1].0)),
                };
                if let Some((a, b)) = ends {
                    let (Some(fa), Some(fb)) = (person_at(self, a), person_at(self, b)) else { continue };
                    let v = |f: (f32, f32, f32, f32), y: f32| (y - (f.1 + f.3 / 2.0)) / (f.3 / 2.0).max(1.0);
                    if let Kind::Shape(placed) = &mut self.doc.layers[*index].kind {
                        let (ua, ub) = (shape.stuck[0].map_or(0.0, |s| s.1), shape.stuck[1].map_or(0.0, |s| s.1));
                        placed.stuck = [Some((a, ua, v(fa, level))), Some((b, ub, v(fb, if a == b { level + 18.0 } else { level })))];
                        placed.number = order as u32;
                        placed.routed = false;
                        placed.points.truncate(2);
                    }
                }
            } else if !dragged.contains(id) {
                let (fx, fy, fw, fh) = shape.frame();
                let by = self.doc.layers[*index].named.clone().unwrap_or_default();
                let over = people.iter().find(|(_, _, person)| person.text == by).map(|(_, index, _)| self.box_frame(*index));
                let left = over.map_or(fx, |f| f.0 + f.2 / 2.0 + 20.0);
                self.doc.layers[*index].translate(left - fx, level - fh / 2.0 - fy);
                if let Kind::Shape(placed) = &mut self.doc.layers[*index].kind {
                    placed.number = order as u32;
                }
                let _ = fw;
            }
        }
        let bottom = first + steps.len().max(1) as f32 * step;
        // Each lifeline down to the last step.
        for &id in &members {
            let Some((index, shape)) = shape_of(self, id) else { continue };
            if shape.tool == Tool::Line {
                if let (Some((person, ..)), Some(f)) = (shape.stuck[0], shape.stuck[0].and_then(|(person, ..)| person_at(self, person))) {
                    if let Kind::Shape(placed) = &mut self.doc.layers[index].kind {
                        placed.stuck = [Some((person, 0.0, 1.0)), Some((person, 0.0, (bottom - (f.1 + f.3 / 2.0)) / (f.3 / 2.0).max(1.0)))];
                    }
                }
            }
        }
        // A box round a part, round the steps it holds and as wide as all.
        for &id in &members {
            let Some((index, shape)) = shape_of(self, id) else { continue };
            if shape.region == FRAGMENT {
                let from = first + shape.number as f32 * step - step / 2.0 - 24.0;
                let to = first + (shape.number + u32::from(shape.votes)) as f32 * step - step / 2.0 + 8.0;
                if let Kind::Shape(placed) = &mut self.doc.layers[index].kind {
                    placed.points = vec![(x + 22.0, from), (along - 50.0, to.max(from + 40.0))];
                    placed.touch();
                }
            }
        }
    }

    /// Tab on a message of a sequence: the next, back the other way, under
    /// it, its words typed. Whether there was one.
    pub(in crate::studio) fn next_message(&mut self) -> bool {
        let Some(index) = self.picked().or(self.editing) else { return false };
        let Kind::Shape(shape) = &self.doc.layers[index].kind else { return false };
        let id = self.id_of(index);
        let Some(holder) = self.holder_of(id) else { return false };
        let is_sequence = self.index_of(holder).is_some_and(|at| matches!(&self.doc.layers[at].kind, Kind::Shape(box_) if box_.region == SEQUENCE));
        if !is_sequence || shape.tool != Tool::Arrow {
            return false;
        }
        let (Some(a), Some(b)) = (shape.stuck[0], shape.stuck[1]) else { return false };
        let below = shape.middle().1 + 30.0;
        let number = shape.number;
        self.commit_text();
        self.checkpoint();
        let arrow = self.diagram_line((0.0, below), (1.0, below), true, false);
        if let Kind::Shape(made) = &mut self.doc.layers[arrow].kind {
            made.stuck = [Some((b.0, 0.0, b.2 + 0.2)), Some((a.0, 0.0, a.2 + 0.2))];
            made.points = vec![(0.0, below), (1.0, below)];
            made.number = number + 1;
        }
        let made_id = self.id_of(arrow);
        self.nest(made_id, holder);
        self.changed();
        self.selected = vec![arrow];
        self.edit_text(arrow);
        true
    }

    fn build_gantt(&mut self, text: &str, origin: (f32, f32)) {
        let mut section = String::new();
        let mut rows: Vec<(String, String, f32, f32)> = Vec::new();
        let mut ends: Vec<(String, f32)> = Vec::new();
        let mut title = String::new();
        let date = |word: &str| -> Option<f32> {
            let parts: Vec<i64> = word.split('-').filter_map(|part| part.trim().parse().ok()).collect();
            let [y, m, d] = parts[..] else { return None };
            // Days since a fixed day, near enough for spacing.
            Some((y * 372 + (m - 1) * 31 + d) as f32)
        };
        let mut last_end = 0.0;
        for line in body(text) {
            if let Some(rest) = line.strip_prefix("title") {
                title = rest.trim().to_owned();
                continue;
            }
            if let Some(rest) = line.strip_prefix("section") {
                section = rest.trim().to_owned();
                continue;
            }
            if line.starts_with("dateFormat") || line.starts_with("axisFormat") || line.starts_with("excludes") || line.starts_with("todayMarker") || line.starts_with("tickInterval") {
                continue;
            }
            let Some((name, spec)) = line.split_once(':') else { continue };
            let parts: Vec<&str> = spec.split(',').map(str::trim).filter(|part| !matches!(*part, "done" | "active" | "crit" | "milestone")).collect();
            let mut start = last_end;
            let mut length = 1.0;
            let mut id = String::new();
            for part in &parts {
                if let Some(after) = part.strip_prefix("after ") {
                    start = after.split_whitespace().filter_map(|known| ends.iter().find(|(name, _)| name == known).map(|(_, end)| *end)).fold(start, f32::max);
                } else if let Some(day) = date(part) {
                    start = day;
                } else if let Some(days) = part.strip_suffix('d').and_then(|n| n.parse::<f32>().ok()) {
                    length = days;
                } else if let Some(weeks) = part.strip_suffix('w').and_then(|n| n.parse::<f32>().ok()) {
                    length = weeks * 7.0;
                } else if !part.is_empty() && part.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    id = (*part).to_owned();
                }
            }
            last_end = start + length;
            if !id.is_empty() {
                ends.push((id, last_end));
            }
            rows.push((section.clone(), name.trim().to_owned(), start, length));
        }
        let earliest = rows.iter().map(|row| row.2).fold(f32::MAX, f32::min);
        let day = 14.0;
        let (left, mut y) = (origin.0, origin.1);
        if !title.is_empty() {
            self.diagram_words(&title, (left, y - 48.0), 28);
        }
        let mut shown_section = String::new();
        for (section, name, start, length) in rows {
            if section != shown_section {
                self.diagram_words(&section, (left - 180.0, y), WORDS_PX);
                shown_section = section;
            }
            let index = self.diagram_box(&name, Form::Box, (0.0, 0.0));
            let (x0, w) = (left + (start - earliest) * day, (length * day).max(24.0));
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                shape.points = vec![(x0, y), (x0 + w, y + 36.0)];
                shape.base = (w, 36.0);
                shape.font_px = 16;
            }
            y += 48.0;
        }
    }

    fn build_pie(&mut self, text: &str, origin: (f32, f32)) {
        let mut slices: Vec<(String, f32)> = Vec::new();
        let mut title = String::new();
        let first = text.lines().map(str::trim).find(|line| line.starts_with("pie")).unwrap_or("");
        if let Some(rest) = first.split_once("title") {
            title = rest.1.trim().to_owned();
        }
        for line in body(text) {
            if let Some(rest) = line.strip_prefix("title") {
                title = rest.trim().to_owned();
                continue;
            }
            if let Some((name, value)) = line.rsplit_once(':') {
                if let Ok(value) = value.trim().parse::<f32>() {
                    slices.push((plain(name), value.max(0.0)));
                }
            }
        }
        let total: f32 = slices.iter().map(|slice| slice.1).sum::<f32>().max(1e-3);
        // The pie itself, drawn as a picture; its key as words beside it.
        let size = 360;
        let mut pixels = vec![0u32; (size * size) as usize];
        let colors = [0x4C6EF5, 0xFAB005, 0x40C057, 0xFA5252, 0x7950F2, 0x15AABF, 0xFD7E14, 0xE64980];
        let r = size as f32 / 2.0 - 4.0;
        for py in 0..size {
            for px in 0..size {
                let (dx, dy) = (px as f32 + 0.5 - size as f32 / 2.0, py as f32 + 0.5 - size as f32 / 2.0);
                let far = dx.hypot(dy);
                if far > r + 1.0 {
                    continue;
                }
                let turn = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0) * total;
                let mut gone = 0.0;
                let mut at = 0;
                for (index, slice) in slices.iter().enumerate() {
                    gone += slice.1;
                    if turn < gone {
                        at = index;
                        break;
                    }
                }
                let cover = ((r + 0.5 - far).clamp(0.0, 1.0) * 255.0) as u32;
                pixels[(py * size + px) as usize] = cover << 24 | colors[at % colors.len()];
            }
        }
        let picture = Picture::new(Arc::new(Image::new(size, size, pixels)), "Pie".into(), (origin.0, origin.1));
        self.doc.layers.push(Layer::picture(picture));
        if !title.is_empty() {
            self.diagram_words(&title, (origin.0 - 180.0, origin.1 - 240.0), 28);
        }
        for (at, (name, value)) in slices.iter().enumerate() {
            let y = origin.1 - 150.0 + at as f32 * 34.0;
            let dot = self.diagram_box("", Form::Box, (origin.0 + 230.0, y + 12.0));
            if let Kind::Shape(shape) = &mut self.doc.layers[dot].kind {
                shape.points = vec![(origin.0 + 220.0, y + 2.0), (origin.0 + 240.0, y + 22.0)];
                (shape.fill, shape.filled, shape.sketch) = (colors[at % colors.len()], true, false);
            }
            self.diagram_words(&format!("{name}  {:.0}%", value / total * 100.0), (origin.0 + 252.0, y), 18);
        }
    }

    fn build_timeline(&mut self, text: &str, origin: (f32, f32)) {
        let mut periods: Vec<(String, Vec<String>)> = Vec::new();
        let mut title = String::new();
        for line in body(text) {
            if let Some(rest) = line.strip_prefix("title") {
                title = rest.trim().to_owned();
                continue;
            }
            if line.starts_with("section") {
                continue;
            }
            let mut parts = line.split(':').map(str::trim);
            let first = parts.next().unwrap_or("");
            let events: Vec<String> = parts.filter(|part| !part.is_empty()).map(str::to_owned).collect();
            match (first.is_empty(), periods.last_mut()) {
                (true, Some(last)) => last.1.extend(events),
                _ => periods.push((first.to_owned(), events)),
            }
        }
        let step = 220.0;
        let (left, y) = (origin.0, origin.1);
        if !title.is_empty() {
            self.diagram_words(&title, (left, y - 100.0), 28);
        }
        self.diagram_line((left - 40.0, y), (left + step * periods.len() as f32, y), true, false);
        for (at, (period, events)) in periods.iter().enumerate() {
            let x = left + at as f32 * step + step / 2.0 - 40.0;
            self.diagram_line((x, y - 12.0), (x, y + 12.0), false, false);
            self.diagram_words(period, (x - 20.0, y - 50.0), WORDS_PX);
            for (row, event) in events.iter().enumerate() {
                self.diagram_box(event, Form::Note, (x, y + 80.0 + row as f32 * 90.0));
            }
        }
    }

    fn build_journey(&mut self, text: &str, origin: (f32, f32)) {
        let mut column = -1.0;
        let mut row = 0.0;
        for line in body(text) {
            if let Some(rest) = line.strip_prefix("title") {
                self.diagram_words(rest.trim(), (origin.0, origin.1 - 100.0), 28);
                continue;
            }
            if let Some(rest) = line.strip_prefix("section") {
                column += 1.0;
                row = 0.0;
                self.diagram_words(rest.trim(), (origin.0 + column * 220.0, origin.1 - 50.0), WORDS_PX);
                continue;
            }
            let parts: Vec<&str> = line.split(':').map(str::trim).collect();
            let [task, score, who @ ..] = &parts[..] else { continue };
            let (task, score) = (*task, *score);
            let score: u8 = score.parse().unwrap_or(3).clamp(1, 5);
            let face = ["\u{1F61E}", "\u{1F641}", "\u{1F610}", "\u{1F642}", "\u{1F600}"][usize::from(score - 1)];
            let words = format!("{task}\n{face} {}", who.join(", "));
            let index = self.diagram_box(&words, Form::Note, (origin.0 + column.max(0.0) * 220.0 + 80.0, origin.1 + row * 120.0));
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                shape.fill = [FILLS[3], FILLS[3], FILLS[0], FILLS[1], FILLS[1]][usize::from(score - 1)];
            }
            row += 1.0;
        }
    }

    fn build_git(&mut self, text: &str, origin: (f32, f32)) {
        let mut branches: Vec<String> = vec!["main".into()];
        let mut current = 0usize;
        let mut heads: Vec<Option<usize>> = vec![None];
        let mut column = 0.0;
        let colors = [0x1971C2, 0x2F9E44, 0xE03131, 0xF08C00, 0x7950F2];
        for line in body(text) {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.first().copied() {
                Some("branch") => {
                    let name = words.get(1).copied().unwrap_or("branch").to_owned();
                    branches.push(name);
                    heads.push(heads[current]);
                    current = branches.len() - 1;
                }
                Some("checkout" | "switch") => {
                    if let Some(at) = branches.iter().position(|branch| Some(branch.as_str()) == words.get(1).copied()) {
                        current = at;
                    }
                }
                Some(kind @ ("commit" | "merge")) => {
                    let lane = current as f32;
                    let at = (origin.0 + column * 90.0, origin.1 + lane * 90.0);
                    column += 1.0;
                    let label = line.split_once("id:").map(|(_, id)| plain(id.split_whitespace().next().unwrap_or(""))).unwrap_or_default();
                    let dot = self.diagram_box(&label, Form::Circle, at);
                    if let Kind::Shape(shape) = &mut self.doc.layers[dot].kind {
                        shape.points = vec![(at.0 - 16.0, at.1 - 16.0), (at.0 + 16.0, at.1 + 16.0)];
                        (shape.fill, shape.font_px, shape.text) = (colors[current % colors.len()], 12, String::new());
                    }
                    if !label.is_empty() {
                        self.diagram_words(&label, (at.0 - 20.0, at.1 + 22.0), 13);
                    }
                    if let Some(previous) = heads[current] {
                        self.join(previous, dot);
                    }
                    if kind == "merge" {
                        if let Some(from) = branches.iter().position(|branch| Some(branch.as_str()) == words.get(1).copied()).and_then(|at| heads[at]) {
                            self.join(from, dot);
                        }
                    }
                    heads[current] = Some(dot);
                }
                _ => {}
            }
        }
        for (lane, name) in branches.iter().enumerate() {
            self.diagram_words(name, (origin.0 - 120.0, origin.1 + lane as f32 * 90.0 - 12.0), 16);
        }
    }

    fn build_quadrant(&mut self, text: &str, origin: (f32, f32)) {
        let (w, h) = (260.0, 220.0);
        let mut names = [String::new(), String::new(), String::new(), String::new()];
        let mut axes = (String::new(), String::new());
        let mut points: Vec<(String, f32, f32)> = Vec::new();
        for line in body(text) {
            if let Some(rest) = line.strip_prefix("title") {
                self.diagram_words(rest.trim(), (origin.0 - w, origin.1 - h - 80.0), 28);
            } else if let Some(rest) = line.strip_prefix("x-axis") {
                axes.0 = rest.trim().replace("-->", "\u{2192}");
            } else if let Some(rest) = line.strip_prefix("y-axis") {
                axes.1 = rest.trim().replace("-->", "\u{2191}");
            } else if let Some((which, rest)) = line.strip_prefix("quadrant-").and_then(|rest| rest.split_once(' ')) {
                if let Ok(at) = which.parse::<usize>() {
                    names[(at.clamp(1, 4)) - 1] = rest.trim().to_owned();
                }
            } else if let Some((name, at)) = line.split_once(':') {
                let numbers: Vec<f32> = at.trim().trim_matches(['[', ']']).split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if let [px, py] = numbers[..] {
                    points.push((plain(name), px, py));
                }
            }
        }
        // Mermaid's quadrants: 1 top right, 2 top left, 3 bottom left, 4 bottom right.
        for (at, (dx, dy)) in [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)].iter().enumerate() {
            let index = self.diagram_box(&names[at], Form::Box, (0.0, 0.0));
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                let (x, y) = (origin.0 - w + dx * w, origin.1 - h + dy * h);
                shape.points = vec![(x, y), (x + w, y + h)];
                (shape.titled, shape.heading, shape.filled) = (true, true, false);
            }
        }
        self.diagram_words(&axes.0, (origin.0 - 80.0, origin.1 + h + 12.0), 18);
        self.diagram_words(&axes.1, (origin.0 - w - 200.0, origin.1 - 12.0), 18);
        for (name, px, py) in points {
            let at = (origin.0 - w + px * 2.0 * w, origin.1 + h - py * 2.0 * h);
            let dot = self.diagram_box("", Form::Circle, at);
            if let Kind::Shape(shape) = &mut self.doc.layers[dot].kind {
                shape.points = vec![(at.0 - 7.0, at.1 - 7.0), (at.0 + 7.0, at.1 + 7.0)];
                (shape.fill, shape.sketch) = (0x1971C2, false);
            }
            self.diagram_words(&name, (at.0 + 12.0, at.1 - 12.0), 15);
        }
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
            let members = self.inside(holder);
            let mut people: Vec<(u32, f32)> = members.iter().filter_map(|&id| self.index_of(id).and_then(|index| match &self.doc.layers[index].kind {
                Kind::Shape(shape) if matches!(shape.tool, Tool::Rect | Tool::Ellipse) && shape.form != shapes::FORM_STICKY && shape.region != FRAGMENT => Some((id, shape.frame().0)),
                _ => None,
            })).collect();
            people.sort_by(|a, b| a.1.total_cmp(&b.1));
            let name = |id: u32| format!("P{}", people.iter().position(|(known, _)| *known == id).unwrap_or(0) + 1);
            let mut out = "sequenceDiagram\n".to_owned();
            for &(id, _) in &people {
                out += &format!("    participant {} as {}\n", name(id), words_of(id));
            }
            let mut steps: Vec<(f32, String)> = Vec::new();
            for &id in &members {
                let Some(index) = self.index_of(id) else { continue };
                let Kind::Shape(shape) = &self.doc.layers[index].kind else { continue };
                if shape.tool == Tool::Arrow {
                    if let (Some((a, ..)), Some((b, ..))) = (shape.stuck[0], shape.stuck[1]) {
                        steps.push((shape.middle().1, format!("    {}{}{}: {}\n", name(a), if shape.dashed { "-->>" } else { "->>" }, name(b), shape.text.replace('\n', " "))));
                    }
                } else if shape.form == shapes::FORM_STICKY {
                    let by = self.doc.layers[index].named.clone().unwrap_or_default();
                    let over = people.iter().find(|(person, _)| words_of(*person) == by).map_or_else(|| name(people.first().map_or(0, |p| p.0)), |&(person, _)| name(person));
                    steps.push((shape.frame().1, format!("    Note right of {over}: {}\n", shape.text.replace('\n', " "))));
                }
            }
            steps.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, line) in steps {
                out += &line;
            }
            return out;
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
            match (shape.tool, shape.form) {
                (Tool::Ellipse, _) => format!("{}((\"{words}\"))", name(id)),
                (_, shapes::FORM_DIAMOND) => format!("{}{{\"{words}\"}}", name(id)),
                (_, shapes::FORM_CYLINDER) => format!("{}[(\"{words}\")]", name(id)),
                _ => format!("{}[\"{words}\"]", name(id)),
            }
        };
        let mut written: Vec<u32> = Vec::new();
        let mut write_box = |out: &mut String, id: u32, depth: usize, written: &mut Vec<u32>| {
            if !written.contains(&id) {
                *out += &format!("{}{}\n", "    ".repeat(depth), node(id));
                written.push(id);
            }
        };
        // Boxes holding others as subgraphs, what is in them inside.
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
            let op = match (shape.heads[1] || shape.tool == Tool::Arrow, shape.dashed, shape.stroke > LINE * 1.5) {
                (true, true, _) => "-.->",
                (true, _, true) => "==>",
                (true, _, _) => "-->",
                (false, true, _) => "-.-",
                (false, _, _) => "---",
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

/// What happens in a sequence, in order.
#[derive(Clone, Debug)]
enum Step {
    Message(String, String, String, bool),
    Note(Vec<String>, String),
    Open(String),
    Else,
    Close,
}

/// What a box lays out, as a sequence; and a box round part of one.
pub(in crate::studio) const SEQUENCE: u8 = 3;
pub(in crate::studio) const FRAGMENT: u8 = 4;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    fn shapes_of(studio: &Studio) -> Vec<Shape> {
        studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) => Some(shape.clone()),
            Kind::Picture(_) => None,
        }).collect()
    }

    fn words(studio: &Studio) -> Vec<String> {
        shapes_of(studio).into_iter().filter(|shape| matches!(shape.tool, Tool::Rect | Tool::Ellipse)).map(|shape| shape.text).collect()
    }

    const FLOW: &str = "flowchart LR\n    A[Christmas] -->|Get money| B(Go shopping)\n    B --> C{Let me think}\n    C -->|One| D[Laptop]\n    C -->|Two| E[iPhone]\n    C -.->|Three| F[(Car DB)]\n    subgraph shop [The shop]\n      D\n      E\n    end\n    F ==> G((Done)) & H\n";

    #[test]
    fn a_flowchart_is_read_into_boxes_and_arrows_and_written_back() {
        let flow = read_flow(FLOW);
        assert!(!flow.down);
        assert_eq!(flow.nodes.iter().map(|(name, ..)| name.as_str()).collect::<Vec<_>>(), ["A", "B", "C", "D", "E", "F", "G", "H"]);
        assert_eq!(flow.edges.len(), 7);
        assert!(flow.edges.iter().any(|edge| edge.0 == "C" && edge.1 == "F" && edge.4 && edge.6 == "Three"), "dashed, with its words");
        assert!(flow.edges.iter().any(|edge| edge.0 == "F" && edge.1 == "G" && edge.5), "thick");
        assert_eq!(flow.groups[0].2, ["D", "E"]);
        let mut studio = board();
        studio.pasted(None, Some(FLOW.to_owned()));
        let shapes = shapes_of(&studio);
        let boxes = words(&studio);
        assert!(boxes.contains(&"Christmas".to_owned()) && boxes.contains(&"The shop".to_owned()), "{boxes:?}");
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_DIAMOND && shape.text == "Let me think"));
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_CYLINDER));
        assert!(shapes.iter().any(|shape| shape.tool == Tool::Ellipse && shape.text == "Done"));
        assert_eq!(studio.links().len(), 7);
        // Laid out left to right: Christmas left of Go shopping.
        let x_of = |name: &str| shapes.iter().find(|shape| shape.text == name).unwrap().frame().0;
        assert!(x_of("Christmas") < x_of("Go shopping") && x_of("Go shopping") < x_of("Let me think"));
        // The shop holds Laptop and iPhone.
        let shop = shapes.iter().find(|shape| shape.text == "The shop").unwrap().id;
        assert_eq!(studio.inside(shop).len(), 2);
        // Written back: the same nodes and edges, read again.
        studio.selected.clear();
        let back = read_flow(&studio.to_mermaid(false));
        assert_eq!((back.nodes.len(), back.edges.len(), back.groups.len()), (8, 7, 1));
        assert!(back.edges.iter().any(|edge| edge.4 && edge.6 == "Three"));
    }

    const SEQUENCE_TEXT: &str = "sequenceDiagram\n    participant A as Alice\n    actor J as John\n    A->>J: Hello John, how are you?\n    loop Every minute\n        J-->>A: Great!\n    end\n    Note right of J: Rational thoughts\n    A-xJ: Bye\n";

    #[test]
    fn a_sequence_is_people_along_the_top_and_messages_down_in_order_and_keeps_so() {
        let mut studio = board();
        studio.pasted(None, Some(SEQUENCE_TEXT.to_owned()));
        let shapes = shapes_of(&studio);
        let alice = shapes.iter().find(|shape| shape.text == "Alice").unwrap().clone();
        let john = shapes.iter().find(|shape| shape.text == "John").unwrap().clone();
        assert!(john.form == shapes::FORM_PERSON && alice.frame().0 < john.frame().0 && (alice.frame().1 - john.frame().1).abs() < 1.0);
        let messages: Vec<Shape> = shapes.iter().filter(|shape| shape.tool == Tool::Arrow).cloned().collect();
        assert_eq!(messages.len(), 3);
        let level = |name: &str| messages.iter().find(|shape| shape.text == name).unwrap().points[0].1;
        assert!(level("Hello John, how are you?") < level("Great!") && level("Great!") < level("Bye"));
        assert!(messages.iter().find(|shape| shape.text == "Great!").unwrap().dashed);
        assert!(shapes.iter().any(|shape| shape.text.starts_with("loop") && shape.region == FRAGMENT));
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_STICKY && shape.text == "Rational thoughts"));
        assert_eq!(shapes.iter().filter(|shape| shape.tool == Tool::Line && shape.dashed).count(), 2, "a lifeline each");
        // A message dragged above the first goes first; the rest stay a step apart.
        studio.selected.clear();
        let bye = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "Bye")).unwrap();
        let hello_y = level("Hello John, how are you?");
        let mid = shape(&studio, bye).middle();
        pull(&mut studio, mid, (mid.0, hello_y - 30.0), Mods::default());
        let now = shapes_of(&studio);
        let level = |name: &str| now.iter().find(|shape| shape.text == name).unwrap().points[0].1;
        assert!(level("Bye") < level("Hello John, how are you?"), "reordered");
        // A participant added further right spreads them out.
        // Written back as a sequence diagram.
        studio.selected.clear();
        let text = studio.to_mermaid(false);
        assert!(text.starts_with("sequenceDiagram") && text.contains("as Alice") && text.contains("-->>") && text.contains("Note right of"), "{text}");
        // Tab on a message: the next, back the other way.
        let hello = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text.starts_with("Hello"))).unwrap();
        studio.selected = vec![hello];
        press_key(&mut studio, Key::Tab, Mods::default());
        type_in(&mut studio, "Fine");
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(shapes_of(&studio).iter().filter(|shape| shape.tool == Tool::Arrow).count(), 4);
    }

    #[test]
    fn the_other_kinds_are_read_as_board_shapes() {
        let cases: [(&str, &[&str]); 9] = [
            ("classDiagram\n    Animal <|-- Duck\n    Animal : +int age\n    class Duck{\n      +swim()\n    }\n", &["Animal\n+int age", "Duck\n+swim()"]),
            ("stateDiagram-v2\n    [*] --> Still\n    Still --> Moving : push\n    Moving --> [*]\n", &["Still", "Moving"]),
            ("erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER {\n      string id\n    }\n", &["CUSTOMER", "ORDER\nstring id"]),
            ("mindmap\n  root((Trip))\n    Flights\n    Hotels\n      Kyoto\n", &["Trip", "Flights", "Hotels", "Kyoto"]),
            ("gantt\n    title Plan\n    dateFormat YYYY-MM-DD\n    section Build\n    Design :a1, 2024-01-01, 10d\n    Code :after a1, 20d\n", &["Design", "Code"]),
            ("timeline\n    title History\n    2002 : LinkedIn\n    2004 : Facebook : Google\n", &["LinkedIn", "Facebook", "Google"]),
            ("journey\n    title My day\n    section Work\n      Make tea: 5: Me\n      Do work: 1: Me, Cat\n", &[]),
            ("gitGraph\n    commit\n    branch develop\n    commit\n    checkout main\n    merge develop\n", &[]),
            ("quadrantChart\n    title Reach\n    x-axis Low --> High\n    y-axis Low --> High\n    quadrant-1 Expand\n    quadrant-2 Promote\n    quadrant-3 Re-evaluate\n    quadrant-4 Improve\n    Campaign A: [0.3, 0.6]\n", &["Expand", "Promote", "Re-evaluate", "Improve"]),
        ];
        for (text, expected) in cases {
            let mut studio = board();
            assert!(studio.bring_mermaid(text), "{text}");
            let boxes = words(&studio);
            for want in expected {
                assert!(boxes.iter().any(|box_words| box_words == want), "{want:?} in {boxes:?}");
            }
            assert!(studio.doc.layers.len() > 2, "{text}");
        }
        // A pie: its pie a picture, its key as words.
        let mut studio = board();
        assert!(studio.bring_mermaid("pie title Pets\n    \"Dogs\" : 386\n    \"Cats\" : 85\n"));
        assert!(studio.doc.layers.iter().any(|layer| matches!(layer.kind, Kind::Picture(_))));
        assert!(shapes_of(&studio).iter().any(|shape| shape.text.starts_with("Dogs  82%")));
        // A mind map written back as one.
        let mut map = board();
        map.bring_mermaid("mindmap\n  root((Trip))\n    Flights\n    Hotels\n");
        let root = map.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "Trip")).unwrap();
        map.selected = vec![root];
        let text = map.to_mermaid(true);
        assert!(text.starts_with("mindmap\n  ((Trip))") && text.contains("    Flights") && text.contains("    Hotels"), "{text}");
    }
}
