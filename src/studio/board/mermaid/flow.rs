//! Flowcharts: their nodes in every shape Mermaid draws, their links
//! solid, dotted, thick or unseen, with heads, circles or crosses at
//! either end and as long as their dashes say, their words, their
//! subgraphs, and the colors their classes and styles give them; laid
//! out in layers. And the graph any diagram of boxes and arrows is made
//! into, laid out and put on the board: class, state and ER diagrams too.

use super::layered::{lay_out, Gaps, Link};
use super::*;

/// A box of a diagram: what it is called, its words, its shape, its
/// colors if it is given its own, and a size of its own if it has one.
#[derive(Clone, Debug)]
pub(in crate::studio) struct Node {
    pub name: String,
    pub words: String,
    pub form: Form,
    pub fill: Option<u32>,
    pub stroke: Option<u32>,
    pub dashed: bool,
}

impl Node {
    pub(super) fn new(name: &str, words: &str, form: Form) -> Node {
        Node { name: name.to_owned(), words: words.to_owned(), form, fill: None, stroke: None, dashed: false }
    }
}

/// An arrow of a diagram: what it is from and to, by name; its words, and
/// those at either end; the head at either end, if it has one; whether it
/// is dashed, dotted, thick or not seen; how long it is at least; and a
/// color and a width of its own if it is given one.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Edge {
    pub from: String,
    pub to: String,
    pub words: String,
    pub tips: [String; 2],
    pub heads: [Option<u8>; 2],
    pub dashed: bool,
    pub dotted: bool,
    pub thick: bool,
    pub hidden: bool,
    pub length: usize,
    pub color: Option<u32>,
    pub width: Option<f32>,
}

/// A box of boxes: what it is called, its title, what is in it by name,
/// boxes and boxes of boxes, and the way it goes if its own.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Group {
    pub name: String,
    pub title: String,
    pub members: Vec<String>,
    pub dir: Option<Dir>,
}

/// A diagram of boxes and arrows, read: its boxes in order, its arrows,
/// its boxes of boxes, and the way it goes.
#[derive(Clone, Debug)]
pub(in crate::studio) struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub groups: Vec<Group>,
    pub dir: Dir,
}

impl Default for Graph {
    fn default() -> Graph {
        Graph { nodes: Vec::new(), edges: Vec::new(), groups: Vec::new(), dir: Dir::Down }
    }
}

impl Graph {
    /// A box by name, made with its name for its words if it is new;
    /// given words and a shape if they are said. Put in the innermost box
    /// of boxes open, if it is in none yet.
    pub(super) fn add(&mut self, name: &str, given: Option<(String, Form)>, open: &[usize]) {
        if self.groups.iter().any(|group| group.name == name) {
            return;
        }
        match self.nodes.iter_mut().find(|node| node.name == name) {
            Some(node) => {
                if let Some((words, form)) = given {
                    (node.words, node.form) = (words, form);
                }
            }
            None => {
                let (words, form) = given.unwrap_or((name.to_owned(), Form::Box));
                self.nodes.push(Node::new(name, &words, form));
            }
        }
        if let Some(&group) = open.last() {
            let held = self.groups.iter().any(|group| group.members.iter().any(|member| member == name));
            if !held {
                self.groups[group].members.push(name.to_owned());
            }
        }
    }

    /// The box of boxes something is directly in, by where it is listed.
    fn group_of(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|group| group.members.iter().any(|member| member == name))
    }

    /// The boxes of boxes something is in, by name, the innermost first.
    fn groups_round(&self, name: &str) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = self.group_of(name);
        while let Some(group) = at {
            if out.contains(&group) || out.len() > 64 {
                break;
            }
            out.push(group);
            at = self.group_of(&self.groups[group].name).filter(|&outer| outer != group);
        }
        out
    }

    /// How early a box, or the first box in a box of boxes, is in the
    /// text: what a cycle is broken by, the later turned back.
    fn first_of(&self, name: &str, depth: usize) -> usize {
        if let Some(at) = self.nodes.iter().position(|node| node.name == name) {
            return at;
        }
        match self.groups.iter().find(|group| group.name == name) {
            Some(group) if depth < 64 => group.members.iter().map(|member| self.first_of(member, depth + 1)).min().unwrap_or(usize::MAX),
            _ => usize::MAX,
        }
    }
}

/// A node at the start of some text: its name, its words and its shape if
/// they are given, and how much of the text it took.
pub(super) fn node_at(text: &str) -> Option<(String, Option<(String, Form)>, usize)> {
    // A name runs to a space, a bracket, or the dashes of a link.
    let mut name_len = 0;
    for (at, c) in text.char_indices() {
        let rest = &text[at..];
        let link = rest.starts_with("--") || rest.starts_with("-.") || rest.starts_with("==") || rest.starts_with("~~");
        if link || !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '$' || c == '\'') {
            break;
        }
        name_len = at + c.len_utf8();
    }
    if name_len == 0 {
        return None;
    }
    let name = text[..name_len].to_owned();
    let rest = &text[name_len..];
    // The newer way: name@{ shape: ..., label: "..." }.
    if let Some(inner) = rest.strip_prefix("@{") {
        let end = inner.find('}')?;
        let mut form = Form::Box;
        let mut words = name.clone();
        for part in inner[..end].split(',') {
            let Some((key, value)) = part.split_once(':') else { continue };
            let value = value.trim();
            match key.trim() {
                "shape" => form = named_shape(value),
                "label" => words = plain(value),
                _ => {}
            }
        }
        return Some((name, Some((words, form)), name_len + 2 + end + 1));
    }
    const SHAPES: [(&str, &str, Form); 16] = [
        ("(((", ")))", Form::Double),
        ("([", "])", Form::Stadium),
        ("((", "))", Form::Circle),
        ("[(", ")]", Form::Database),
        ("[[", "]]", Form::Subroutine),
        ("{{", "}}", Form::Hexagon),
        ("[/", "/]", Form::LeanRight),
        ("[/", "\\]", Form::Trapezoid),
        ("[\\", "\\]", Form::LeanLeft),
        ("[\\", "/]", Form::TrapezoidDown),
        ("[", "]", Form::Box),
        ("(", ")", Form::Round),
        ("{", "}", Form::Diamond),
        (">", "]", Form::Flag),
        ("[|", "|]", Form::Box),
        ("[-", "-]", Form::Box),
    ];
    for (open, close, form) in SHAPES {
        let Some(inner) = rest.strip_prefix(open) else { continue };
        // Words in quotes may hold brackets of their own.
        let skip = match inner.trim_start().strip_prefix('"') {
            Some(quoted) => inner.len() - quoted.len() + quoted.find('"').map_or(0, |end| end + 1),
            None => 0,
        };
        let Some(end) = inner[skip..].find(close).map(|end| end + skip) else { continue };
        // The other slant of the same opening is another shape.
        if matches!(open, "[/" | "[\\") && inner[..end].contains(['/', '\\']) && skip == 0 {
            continue;
        }
        return Some((name, Some((plain(&inner[..end]), form)), name_len + open.len() + end + close.len()));
    }
    Some((name, None, name_len))
}

/// A shape by the name Mermaid's newer way gives it.
fn named_shape(name: &str) -> Form {
    match name.trim().trim_matches('"') {
        "rounded" | "event" => Form::Round,
        "stadium" | "pill" | "terminal" => Form::Stadium,
        "subroutine" | "fr-rect" | "subprocess" | "framed-rectangle" => Form::Subroutine,
        "cyl" | "cylinder" | "database" | "db" => Form::Database,
        "circle" | "circ" | "sm-circ" | "small-circle" | "start" => Form::Circle,
        "dbl-circ" | "double-circle" | "fr-circ" | "stop" => Form::Double,
        "diam" | "diamond" | "decision" | "question" => Form::Diamond,
        "hex" | "hexagon" | "prepare" => Form::Hexagon,
        "lean-r" | "lean-right" | "in-out" => Form::LeanRight,
        "lean-l" | "lean-left" | "out-in" => Form::LeanLeft,
        "trap-b" | "trapezoid" | "priority" | "trapezoid-bottom" => Form::Trapezoid,
        "trap-t" | "inv-trapezoid" | "manual" | "trapezoid-top" => Form::TrapezoidDown,
        "odd" | "flag" | "paper-tape" => Form::Flag,
        "doc" | "document" | "notch-rect" | "card" => Form::Note,
        _ => Form::Box,
    }
}

/// A link at the start of some text, as an edge with no ends yet, and how
/// much of the text it took: its heads, dashes, words and length.
pub(super) fn edge_at(text: &str) -> Option<(Edge, usize)> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut edge = Edge::default();
    let head_of = |c: u8| match c {
        b'>' | b'<' => Some(shapes::FILLED_HEAD),
        b'o' => Some(shapes::RING_HEAD),
        b'x' => Some(shapes::CROSS_HEAD),
        _ => None,
    };
    // A head at its start: <, o or x before its dashes.
    if let (Some(&first), Some(&second)) = (bytes.first(), bytes.get(1)) {
        if matches!(first, b'<' | b'o' | b'x') && matches!(second, b'-' | b'=') && (first == b'<' || bytes.get(2).is_some_and(|&c| c == b'-' || c == b'=')) {
            edge.heads[0] = head_of(first);
            at = 1;
        }
    }
    let line = *bytes.get(at)?;
    let (count, stop) = match line {
        b'~' => {
            let run = bytes[at..].iter().take_while(|&&c| c == b'~').count();
            if run < 3 {
                return None;
            }
            edge.hidden = true;
            edge.length = run - 2;
            return Some((edge, at + run));
        }
        b'-' | b'=' => {
            let run = bytes[at..].iter().take_while(|&&c| c == line).count();
            (run, at + run)
        }
        _ => return None,
    };
    let mut at = stop;
    edge.thick = line == b'=';
    // Dotted: -.- or -..->, the dots counting its length.
    if line == b'-' && count == 1 && bytes.get(at) == Some(&b'.') {
        let dots = bytes[at..].iter().take_while(|&&c| c == b'.').count();
        if bytes.get(at + dots) == Some(&b'-') {
            at += dots + 1;
            edge.dotted = true;
            edge.length = dots;
            if let Some(head) = bytes.get(at).and_then(|&c| head_of(c).filter(|_| c != b'<')) {
                if bytes[at] == b'>' || c_ends_word(bytes, at) {
                    edge.heads[1] = Some(head);
                    at += 1;
                }
            }
            return Some(with_words(text, edge, at));
        }
        // Words in it: -. words .->
        let after = &text[at..];
        let close = after.find(".-")?;
        edge.words = plain(&after[1..close]);
        edge.dotted = true;
        edge.length = 1;
        let mut end = at + close + 2;
        if let Some(head) = bytes.get(end).and_then(|&c| head_of(c)) {
            edge.heads[1] = Some(head);
            end += 1;
        }
        return Some((edge, end));
    }
    if count < 2 {
        return None;
    }
    // Words in it: -- words --> or == words ==>.
    if count == 2 && bytes.get(at) == Some(&b' ') {
        let after = &text[at..];
        let pair = if line == b'=' { "==" } else { "--" };
        if let Some(close) = after.find(pair) {
            let closing = &after[close..];
            let run = closing.bytes().take_while(|&c| c == line).count();
            edge.words = plain(&after[..close]);
            let mut end = at + close + run;
            let head = bytes.get(end).and_then(|&c| head_of(c).filter(|_| c != b'<'));
            if let Some(head) = head {
                edge.heads[1] = Some(head);
                end += 1;
            }
            edge.length = run.saturating_sub(if head.is_some() { 1 } else { 2 }).max(1);
            return Some((edge, end));
        }
    }
    let head = bytes.get(at).and_then(|&c| head_of(c).filter(|_| c != b'<'));
    match head {
        Some(head) if bytes[at] == b'>' || c_ends_word(bytes, at) => {
            edge.heads[1] = Some(head);
            at += 1;
            edge.length = (count - 1).max(1);
        }
        _ => edge.length = count.saturating_sub(2).max(1),
    }
    Some(with_words(text, edge, at))
}

/// Whether an `o` or an `x` after a link's dashes is its head rather than
/// the start of the next name: nothing of a name after it.
fn c_ends_word(bytes: &[u8], at: usize) -> bool {
    bytes.get(at + 1).is_none_or(|&c| !(c.is_ascii_alphanumeric() || c == b'_'))
}

/// A link's words between bars after it, `-->|words|`, taken in.
fn with_words(text: &str, mut edge: Edge, at: usize) -> (Edge, usize) {
    let rest = &text[at..];
    let trimmed = rest.trim_start();
    if let Some(inner) = trimmed.strip_prefix('|') {
        if let Some(end) = inner.find('|') {
            edge.words = plain(&inner[..end]);
            return (edge, at + rest.len() - trimmed.len() + 1 + end + 1);
        }
    }
    (edge, at)
}

/// A style as CSS writes it, `fill:#f9f,stroke:#333,stroke-width:4px`: its
/// fill, its line's color, how wide that is, and whether it is dashed.
fn style_of(words: &str) -> (Option<u32>, Option<u32>, Option<f32>, bool) {
    let (mut fill, mut stroke, mut width, mut dashed) = (None, None, None, false);
    for part in words.split([',', ';']) {
        let Some((key, value)) = part.split_once(':') else { continue };
        match key.trim() {
            "fill" => fill = css_color(value),
            "stroke" => stroke = css_color(value),
            "stroke-width" => width = value.trim().trim_end_matches("px").parse().ok(),
            "stroke-dasharray" => dashed = true,
            _ => {}
        }
    }
    (fill, stroke, width, dashed)
}

/// A flowchart read.
pub(in crate::studio) fn read_flow(text: &str) -> Graph {
    let mut graph = Graph { dir: header(text).split_whitespace().nth(1).and_then(Dir::named).unwrap_or(Dir::Down), ..Graph::default() };
    let mut open: Vec<usize> = Vec::new();
    let mut classes: Vec<(String, String)> = Vec::new();
    let mut given: Vec<(String, String)> = Vec::new();
    let mut styles: Vec<(String, String)> = Vec::new();
    let mut link_styles: Vec<(String, String)> = Vec::new();
    let statements: Vec<String> = body(text).iter().flat_map(|line| split_statements(line)).collect();
    for line in statements {
        let first_word = line.split_whitespace().next().unwrap_or("");
        let rest = line[first_word.len()..].trim();
        match first_word {
            "subgraph" => {
                let (name, title) = match rest.split_once('[') {
                    Some((name, title)) => (name.trim().to_owned(), plain(title.trim_end_matches(']'))),
                    None if rest.starts_with('"') => (plain(rest), plain(rest)),
                    None => (rest.split_whitespace().next().unwrap_or("").to_owned(), plain(rest)),
                };
                if let Some(&outer) = open.last() {
                    graph.groups[outer].members.push(name.clone());
                }
                graph.groups.push(Group { name, title, ..Group::default() });
                open.push(graph.groups.len() - 1);
                continue;
            }
            "end" => {
                open.pop();
                continue;
            }
            "direction" => {
                let dir = Dir::named(rest);
                match open.last() {
                    Some(&group) => graph.groups[group].dir = dir,
                    None => graph.dir = dir.unwrap_or(graph.dir),
                }
                continue;
            }
            "classDef" => {
                let (names, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                for name in names.split(',') {
                    classes.push((name.trim().to_owned(), style.trim().to_owned()));
                }
                continue;
            }
            "class" => {
                let (names, class) = rest.rsplit_once(char::is_whitespace).unwrap_or((rest, ""));
                for name in names.split(',') {
                    given.push((name.trim().to_owned(), class.trim().to_owned()));
                }
                continue;
            }
            "style" => {
                let (name, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                styles.push((name.trim().to_owned(), style.trim().to_owned()));
                continue;
            }
            "linkStyle" => {
                let (which, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                link_styles.push((which.trim().to_owned(), style.trim().to_owned()));
                continue;
            }
            "click" | "accTitle" | "accDescr" | "accTitle:" | "accDescr:" => continue,
            _ => {}
        }
        read_chain(&mut graph, &line, &open, &mut given);
    }
    // Colors by class, then by style, mapped to the board's own.
    for (name, class) in &given {
        if let Some((_, style)) = classes.iter().find(|(known, _)| known == class) {
            styles.insert(0, (name.clone(), style.clone()));
        }
    }
    for (name, style) in styles {
        let (fill, stroke, _, dashed) = style_of(&style);
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.name == name) {
            node.fill = fill.map(board_fill).or(node.fill);
            node.stroke = stroke.map(board_stroke).or(node.stroke);
            node.dashed |= dashed;
        }
    }
    let seen: Vec<usize> = (0..graph.edges.len()).filter(|&at| !graph.edges[at].hidden).collect();
    for (which, style) in link_styles {
        let (_, stroke, width, dashed) = style_of(&style);
        let picked: Vec<usize> = match which.as_str() {
            "default" => (0..graph.edges.len()).collect(),
            _ => which.split(',').filter_map(|number| number.trim().parse::<usize>().ok()).filter_map(|number| seen.get(number).copied().or(Some(number)).filter(|&at| at < graph.edges.len())).collect(),
        };
        for at in picked {
            let edge = &mut graph.edges[at];
            edge.color = stroke.map(board_stroke).or(edge.color);
            edge.width = width.or(edge.width);
            edge.dashed |= dashed;
        }
    }
    graph
}

/// A line of a flowchart split at its semicolons, but those in quotes.
fn split_statements(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (mut current, mut quoted) = (String::new(), false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts.into_iter().map(|part| part.trim().to_owned()).filter(|part| !part.is_empty()).collect()
}

/// A chain read into a graph: nodes, each group of them joined by &,
/// between links.
fn read_chain(graph: &mut Graph, line: &str, open: &[usize], given: &mut Vec<(String, String)>) {
    let mut at = 0;
    let mut before: Vec<String> = Vec::new();
    let mut pending: Option<Edge> = None;
    while at < line.len() {
        let rest = line[at..].trim_start();
        at = line.len() - rest.len();
        if rest.is_empty() {
            break;
        }
        // A link's own name before it, e1@-->, left out.
        let rest_link = rest.find("@").filter(|&sign| rest[..sign].chars().all(|c| c.is_alphanumeric()) && rest[sign + 1..].starts_with(['-', '=', '~', '<'])).map_or(rest, |sign| &rest[sign + 1..]);
        if !before.is_empty() && pending.is_none() {
            if let Some((edge, used)) = edge_at(rest_link) {
                pending = Some(edge);
                at += rest.len() - rest_link.len() + used;
                continue;
            }
        }
        let mut now: Vec<String> = Vec::new();
        let mut rest_at = at;
        loop {
            let rest = line[rest_at..].trim_start();
            rest_at = line.len() - rest.len();
            let Some((name, shape, used)) = node_at(rest) else { break };
            graph.add(&name, shape, open);
            rest_at += used;
            let after = line[rest_at..].trim_start();
            // A class put on it, as name:::class.
            let after = match after.strip_prefix(":::") {
                Some(class) => {
                    let end = class.find(|c: char| c.is_whitespace() || c == '&').unwrap_or(class.len());
                    given.push((name.clone(), class[..end].to_owned()));
                    &class[end..]
                }
                None => after,
            };
            now.push(name);
            rest_at = line.len() - after.len();
            match after.trim_start().strip_prefix('&') {
                Some(more) => rest_at = line.len() - more.len(),
                None => break,
            }
        }
        if now.is_empty() {
            break;
        }
        if let Some(edge) = pending.take() {
            for from in &before {
                for to in &now {
                    graph.edges.push(Edge { from: from.clone(), to: to.clone(), ..edge.clone() });
                }
            }
        }
        before = now;
        at = rest_at;
    }
}

/// Where a graph's boxes, boxes of boxes and arrows go: the middle of
/// each box, the frame of each box of boxes, and the points each arrow
/// goes by and where its words go; and how large it all is.
#[derive(Clone, Debug, Default)]
pub(in crate::studio) struct Placement {
    pub centers: Vec<Point>,
    pub groups: Vec<(f32, f32, f32, f32)>,
    pub ways: Vec<Vec<Point>>,
    pub labels: Vec<Option<Point>>,
    pub size: (f32, f32),
}

/// What is laid out in a box of boxes, or in the whole: a box, or a box
/// of boxes as one.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Item {
    Node(usize),
    Group(usize),
}

/// A graph's boxes of these sizes and its arrows' words of these laid out:
/// each box of boxes laid out first, inside out, and laid out as one box
/// in what holds it, its title over what is in it and room round all.
pub(in crate::studio) fn place(graph: &Graph, sizes: &[Point], words: &[Point], titles: &[Point], gaps: Gaps) -> Placement {
    let mut placement = Placement {
        centers: vec![(0.0, 0.0); graph.nodes.len()],
        groups: vec![(0.0, 0.0, 0.0, 0.0); graph.groups.len()],
        ways: vec![Vec::new(); graph.edges.len()],
        labels: vec![None; graph.edges.len()],
        size: (0.0, 0.0),
    };
    // What each thing is directly in: none for the whole.
    let parent_of_node: Vec<Option<usize>> = graph.nodes.iter().map(|node| graph.group_of(&node.name)).collect();
    let parent_of_group: Vec<Option<usize>> = graph.groups.iter().enumerate().map(|(at, group)| graph.group_of(&group.name).filter(|&outer| outer != at)).collect();
    let item_of = |name: &str| -> Option<Item> {
        graph.nodes.iter().position(|node| node.name == name).map(Item::Node).or_else(|| graph.groups.iter().position(|group| group.name == name).map(Item::Group))
    };
    let parent = |item: Item| match item {
        Item::Node(at) => parent_of_node[at],
        Item::Group(at) => parent_of_group[at],
    };
    // The boxes of boxes something is in, outermost first.
    let chain = |item: Item| -> Vec<Option<usize>> {
        let mut out = vec![None];
        let mut at = parent(item);
        let mut inner = Vec::new();
        while let Some(group) = at {
            if inner.contains(&Some(group)) || inner.len() > 64 {
                break;
            }
            inner.push(Some(group));
            at = parent_of_group[group];
        }
        inner.reverse();
        out.extend(inner);
        out
    };
    // Each arrow at the level of the innermost box holding both its ends,
    // between what stands for each end there.
    let ends: Vec<Option<(Option<usize>, Item, Item)>> = graph.edges.iter().map(|edge| {
        let (from, to) = (item_of(&edge.from)?, item_of(&edge.to)?);
        let (a, b) = (chain(from), chain(to));
        let mut level = 0;
        while level + 1 < a.len() && level + 1 < b.len() && a[level + 1] == b[level + 1] {
            level += 1;
        }
        let scope = a[level];
        // What stands for an end there: itself, or the box of boxes it is
        // in that is directly in the scope.
        let stand = |item: Item, path: &[Option<usize>]| match path.get(level + 1) {
            Some(Some(group)) if Item::Group(*group) != item => Item::Group(*group),
            _ => item,
        };
        let (from_there, to_there) = (stand(from, &a), stand(to, &b));
        // An arrow to a box of boxes from inside it is laid out in it.
        Some((scope, from_there, to_there))
    }).collect();
    // Laid out inside out: each scope's items, at their sizes, relative
    // to its top left.
    let mut local: Vec<Vec<(Item, Point)>> = vec![Vec::new(); graph.groups.len() + 1];
    let mut group_sizes: Vec<Point> = vec![(0.0, 0.0); graph.groups.len()];
    let mut offsets: Vec<Point> = vec![(0.0, 0.0); graph.groups.len()];
    let mut local_ways: Vec<(Option<usize>, Vec<Point>, Option<Point>)> = vec![(None, Vec::new(), None); graph.edges.len()];
    let depth = |group: usize| chain(Item::Group(group)).len();
    let mut order: Vec<usize> = (0..graph.groups.len()).collect();
    order.sort_by_key(|&group| std::cmp::Reverse(depth(group)));
    let scopes: Vec<Option<usize>> = order.into_iter().map(Some).chain(std::iter::once(None)).collect();
    for scope in scopes {
        let mut items: Vec<Item> = (0..graph.nodes.len()).filter(|&at| parent_of_node[at] == scope).map(Item::Node).collect();
        items.extend((0..graph.groups.len()).filter(|&at| parent_of_group[at] == scope && Some(at) != scope).map(Item::Group));
        // In the order they first come in the text, which cycles are
        // broken by.
        items.sort_by_key(|&item| match item {
            Item::Node(at) => at,
            Item::Group(at) => graph.first_of(&graph.groups[at].name, 0),
        });
        let size_of = |item: Item| match item {
            Item::Node(at) => sizes[at],
            Item::Group(at) => group_sizes[at],
        };
        let item_sizes: Vec<Point> = items.iter().map(|&item| size_of(item)).collect();
        let mut links = Vec::new();
        let mut which = Vec::new();
        for (at, end) in ends.iter().enumerate() {
            let Some((edge_scope, from, to)) = *end else { continue };
            if edge_scope != scope {
                continue;
            }
            let (Some(a), Some(b)) = (items.iter().position(|&item| item == from), items.iter().position(|&item| item == to)) else { continue };
            // An arrow between a box of boxes and what is in it is not
            // laid out, only drawn.
            if a == b && from != to {
                continue;
            }
            links.push(Link { from: a, to: b, label: words[at], length: graph.edges[at].length.max(1) });
            which.push(at);
        }
        let dir = scope.and_then(|group| graph.groups[group].dir).unwrap_or(graph.dir);
        let laid = lay_out(&item_sizes, &links, dir, gaps);
        for (k, &at) in which.iter().enumerate() {
            local_ways[at] = (scope, laid.ways[k].clone(), laid.labels[k]);
        }
        local[scope.map_or(0, |group| group + 1)] = items.iter().copied().zip(laid.centers.iter().copied()).collect();
        if let Some(group) = scope {
            // Room round what is in it, its title over it, and as wide as
            // its title; what is in it in the middle across.
            let title = titles[group];
            let inner = laid.size;
            let wide = (inner.0 + 2.0 * PAD).max(title.0 + 2.0 * PAD);
            let tall = inner.1 + 2.0 * PAD + title.1;
            group_sizes[group] = (wide, tall);
            offsets[group] = ((wide - inner.0) / 2.0, PAD + title.1);
        } else {
            placement.size = laid.size;
        }
    }
    // Then outside in: each item where its scope is, plus where it is in it.
    fn put(scope: Option<usize>, origin: Point, local: &[Vec<(Item, Point)>], group_sizes: &[Point], offsets: &[Point], placement: &mut Placement, sizes: &[Point], scope_origin: &mut Vec<Point>) {
        scope_origin[scope.map_or(0, |group| group + 1)] = origin;
        for &(item, (cx, cy)) in &local[scope.map_or(0, |group| group + 1)] {
            let middle = (origin.0 + cx, origin.1 + cy);
            match item {
                Item::Node(at) => placement.centers[at] = middle,
                Item::Group(at) => {
                    let (w, h) = group_sizes[at];
                    let frame = (middle.0 - w / 2.0, middle.1 - h / 2.0, w, h);
                    placement.groups[at] = frame;
                    put(Some(at), (frame.0 + offsets[at].0, frame.1 + offsets[at].1), local, group_sizes, offsets, placement, sizes, scope_origin);
                }
            }
        }
    }
    let mut scope_origin = vec![(0.0, 0.0); graph.groups.len() + 1];
    put(None, (0.0, 0.0), &local, &group_sizes, &offsets, &mut placement, sizes, &mut scope_origin);
    for (at, (scope, way, label)) in local_ways.into_iter().enumerate() {
        let origin = scope_origin[scope.map_or(0, |group| group + 1)];
        placement.ways[at] = way.into_iter().map(|(x, y)| (x + origin.0, y + origin.1)).collect();
        placement.labels[at] = label.map(|(x, y)| (x + origin.0, y + origin.1));
    }
    placement
}

/// A frame moved by so much.
fn at_frame((x, y, w, h): (f32, f32, f32, f32), shift: Point) -> (f32, f32, f32, f32) {
    (x + shift.0, y + shift.1, w, h)
}

/// Whether a straight line from one point to another goes through any of
/// some boxes, a little in from their edges.
fn crosses(a: Point, b: Point, frames: &[(f32, f32, f32, f32)]) -> bool {
    let steps = ((b.0 - a.0).hypot(b.1 - a.1) / 4.0).ceil().max(1.0) as usize;
    (0..=steps).any(|step| {
        let t = step as f32 / steps as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        frames.iter().any(|&(fx, fy, fw, fh)| x > fx + 2.0 && x < fx + fw - 2.0 && y > fy + 2.0 && y < fy + fh - 2.0)
    })
}

/// Where an arrow from a point inside a box of boxes goes out of it, to
/// get to another point outside without going through what else is in
/// it: straight out of its nearer side across the way the graph goes,
/// and round its corner there if the other point is not that side of it,
/// the points it goes by from the inside out. Nothing where the straight
/// way is clear, or no side is.
fn way_out(from: Point, to: Point, group: (f32, f32, f32, f32), dir: Dir, frames: &[(f32, f32, f32, f32)]) -> Vec<Point> {
    if !crosses(from, to, frames) {
        return Vec::new();
    }
    let (x, y, w, h) = group;
    let room = 16.0;
    // Each side: where it leaves through it, and whether the other point
    // is past that side already, or which corner to go round to it.
    let sides: [(Point, bool, Point); 2] = match dir.vertical() {
        true => {
            let corner_y = if to.1 < from.1 { y - room } else { y + h + room };
            [((x - room, from.1), to.0 <= x - room, (x - room, corner_y)), ((x + w + room, from.1), to.0 >= x + w + room, (x + w + room, corner_y))]
        }
        false => {
            let corner_x = if to.0 < from.0 { x - room } else { x + w + room };
            [((from.0, y - room), to.1 <= y - room, (corner_x, y - room)), ((from.0, y + h + room), to.1 >= y + h + room, (corner_x, y + h + room))]
        }
    };
    let length = |points: &[Point]| points.windows(2).map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1)).sum::<f32>();
    sides
        .into_iter()
        .filter(|&(out, ..)| !crosses(from, out, frames))
        .map(|(out, past, corner)| if past { vec![out] } else { vec![out, corner] })
        .min_by(|a, b| {
            let whole = |way: &Vec<Point>| length(&[&[from][..], way, &[to]].concat());
            whole(a).total_cmp(&whole(b))
        })
        .unwrap_or_default()
}

/// The gaps a diagram's layers are laid out with: between boxes side by
/// side, and between ranks.
pub(super) const GAPS: Gaps = Gaps { node: 44.0, rank: 64.0 };

impl Studio {
    pub(super) fn build_flow(&mut self, text: &str, origin: Point) {
        let graph = read_flow(text);
        let title = front_title(text);
        self.build_graph(&graph, origin, title.as_deref(), GAPS);
    }

    /// A graph put on the board laid out, its middle at `origin`: a box for
    /// each of its boxes, a box round each box of boxes, under what it
    /// holds and holding it, and an arrow for each of its arrows, going
    /// by the points kept for it; a title over it all if it has one. The
    /// layer of each of its boxes, in order.
    pub(super) fn build_graph(&mut self, graph: &Graph, origin: Point, title: Option<&str>, gaps: Gaps) -> Vec<usize> {
        let start = self.doc.layers.len();
        let px = self.board.px;
        let mut boxes: Vec<u32> = Vec::new();
        for node in &graph.nodes {
            let index = self.diagram_box(&node.words, node.form, origin);
            if let Some(shape) = self.shape_mut(index) {
                if let Some(fill) = node.fill {
                    (shape.fill, shape.filled) = (fill, true);
                }
                if let Some(stroke) = node.stroke {
                    shape.color = stroke;
                }
                shape.dashed |= node.dashed;
            }
            boxes.push(self.id_of(index));
        }
        let sizes: Vec<Point> = boxes.iter().map(|&id| self.index_of(id).map_or((0.0, 0.0), |index| {
            let (_, _, w, h) = self.box_frame(index);
            (w, h)
        })).collect();
        let words: Vec<Point> = graph.edges.iter().map(|edge| match edge.words.is_empty() {
            true => (0.0, 0.0),
            false => (self.words_wide(&edge.words, px) + px as f32 * 0.9, Self::words_tall(&edge.words, px) + px as f32 * 0.3),
        }).collect();
        let titles: Vec<Point> = graph.groups.iter().map(|group| (self.words_wide(&group.title, px) + px as f32, Text::line_height(px) as f32 + 10.0)).collect();
        let placement = place(graph, &sizes, &words, &titles, gaps);
        let shift = (origin.0 - placement.size.0 / 2.0, origin.1 - placement.size.1 / 2.0);
        let at = |(x, y): Point| (x + shift.0, y + shift.1);
        for (k, &id) in boxes.iter().enumerate() {
            if let Some(index) = self.index_of(id) {
                self.center_at(index, at(placement.centers[k]));
            }
        }
        // The boxes of boxes, the outermost first, each under all of it.
        let mut holders: Vec<u32> = vec![0; graph.groups.len()];
        let depth = |group: usize| {
            let mut count = 0;
            let mut name = &graph.groups[group].name;
            while let Some(outer) = graph.groups.iter().position(|other| other.members.contains(name)) {
                count += 1;
                name = &graph.groups[outer].name;
                if count > 64 {
                    break;
                }
            }
            count
        };
        let mut order: Vec<usize> = (0..graph.groups.len()).collect();
        order.sort_by_key(|&group| depth(group));
        for (made, &group) in order.iter().enumerate() {
            let index = self.diagram_box(&graph.groups[group].title, Form::Box, origin);
            let (x, y, w, h) = placement.groups[group];
            let (x, y) = at((x, y));
            self.frame_to(index, (x, y, w, h));
            if let Some(shape) = self.shape_mut(index) {
                (shape.titled, shape.heading, shape.filled) = (true, true, false);
            }
            let layer = self.doc.layers.remove(index);
            self.doc.layers.insert(start + made, layer);
            holders[group] = self.id_of(start + made);
        }
        for (group, holder) in graph.groups.iter().zip(&holders) {
            for member in &group.members {
                let id = graph.nodes.iter().position(|node| &node.name == member).map(|at| boxes[at]).or_else(|| graph.groups.iter().position(|other| &other.name == member).map(|at| holders[at]));
                if let Some(id) = id {
                    self.nest(id, *holder);
                }
            }
        }
        // The arrows, over it all.
        let find = |name: &str| graph.nodes.iter().position(|node| node.name == name).map(|at| boxes[at]).or_else(|| graph.groups.iter().position(|group| group.name == name).map(|at| holders[at]));
        let mut tips: Vec<(u32, [String; 2])> = Vec::new();
        for (k, edge) in graph.edges.iter().enumerate() {
            if edge.hidden {
                continue;
            }
            let (Some(from), Some(to)) = (find(&edge.from), find(&edge.to)) else { continue };
            let (Some(a), Some(b)) = (self.index_of(from), self.index_of(to)) else { continue };
            let middle = |frame: (f32, f32, f32, f32)| (frame.0 + frame.2 / 2.0, frame.1 + frame.3 / 2.0);
            let (pa, pb) = (middle(self.box_frame(a)), middle(self.box_frame(b)));
            let headed = edge.heads.iter().any(Option::is_some);
            let arrow = self.diagram_line(pa, pb, headed, edge.dashed);
            let mut way: Vec<Point> = placement.ways[k].iter().map(|&point| at(point)).collect();
            // A single point nearly on the straight way, with no words to
            // carry, is no bend at all.
            if way.len() == 1 && edge.words.is_empty() && from != to {
                let (p, q, r) = (pa, way[0], pb);
                let long = (r.0 - p.0).hypot(r.1 - p.1).max(1.0);
                let off = ((q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)).abs() / long;
                if off < 6.0 {
                    way.clear();
                }
            }
            // Out of a box of boxes it leaves, and into one it comes in to,
            // round its side rather than through what else is in it.
            let node_frames: Vec<(f32, f32, f32, f32)> = boxes.iter().filter(|&&id| id != from && id != to).filter_map(|&id| self.index_of(id)).map(|index| self.box_frame(index)).collect();
            let (round_from, round_to) = (graph.groups_round(&edge.from), graph.groups_round(&edge.to));
            let scope = round_from.iter().find(|group| round_to.contains(group)).copied();
            let dir = scope.and_then(|group| graph.groups[group].dir).unwrap_or(graph.dir);
            let outermost = |round: &[usize]| round.iter().take_while(|&&group| Some(group) != scope).last().copied();
            if let Some(group) = outermost(&round_from) {
                let next = way.first().copied().unwrap_or(pb);
                let out = way_out(pa, next, at_frame(placement.groups[group], shift), dir, &node_frames);
                way.splice(0..0, out);
            }
            if let Some(group) = outermost(&round_to) {
                let before = way.last().copied().unwrap_or(pa);
                let out = way_out(pb, before, at_frame(placement.groups[group], shift), dir, &node_frames);
                way.extend(out.into_iter().rev());
            }
            if let Some(shape) = self.shape_mut(arrow) {
                shape.heads = [edge.heads[0].is_some(), edge.heads[1].is_some()];
                shape.ends = [edge.heads[0].unwrap_or(shapes::FILLED_HEAD), edge.heads[1].unwrap_or(shapes::FILLED_HEAD)];
                shape.dotted = edge.dotted && !edge.dashed;
                shape.dashed = edge.dashed || (edge.dotted && shape.sketch);
                if edge.thick {
                    shape.stroke *= 2.0;
                }
                if let Some(width) = edge.width {
                    shape.stroke = (width * 0.9).clamp(1.0, 8.0);
                }
                if let Some(color) = edge.color {
                    shape.color = color;
                }
                shape.text = edge.words.clone();
                shape.stuck = [Some((from, shapes::WHOLE, shapes::WHOLE)), Some((to, shapes::WHOLE, shapes::WHOLE))];
                shape.points.extend(way);
                shape.touch();
            }
            if edge.tips.iter().any(|tip| !tip.is_empty()) {
                tips.push((self.id_of(arrow), edge.tips.clone()));
            }
        }
        self.follow();
        // The words at an arrow's ends, beside where it leaves and comes in.
        for (arrow, words) in tips {
            let Some(index) = self.index_of(arrow) else { continue };
            let Kind::Shape(shape) = self.doc.layers[index].kind.clone() else { continue };
            let path = shape.path();
            for (end, words) in words.iter().enumerate().filter(|(_, words)| !words.is_empty()) {
                let (tip, from) = if end == 0 { (path[0], path[(path.len() / 6).max(1).min(path.len() - 1)]) } else { (path[path.len() - 1], path[path.len() - 1 - (path.len() / 6).max(1).min(path.len() - 1)]) };
                let long = (from.0 - tip.0).hypot(from.1 - tip.1).max(1e-3);
                let (dx, dy) = ((from.0 - tip.0) / long, (from.1 - tip.1) / long);
                let small = (px * 4 / 5).max(12);
                let (wide, tall) = (self.words_wide(words, small), Self::words_tall(words, small));
                // Along the arrow a little, and off to its side.
                let along = 14.0 + if dx.abs() > dy.abs() { wide / 2.0 } else { tall / 2.0 };
                let side = 8.0 + if dx.abs() > dy.abs() { tall / 2.0 } else { wide / 2.0 };
                let middle = (tip.0 + dx * along - dy * side, tip.1 + dy * along + dx * side);
                self.words_at(words, middle, small);
            }
        }
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            let top = origin.1 - placement.size.1 / 2.0 - 16.0;
            self.diagram_title(title, origin.0, top);
        }
        boxes.iter().filter_map(|&id| self.index_of(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::check::*;
    use super::super::tests::shapes_of;
    use super::*;
    use crate::studio::board::tests::*;

    const FLOW: &str = "flowchart LR\n    A[Christmas] -->|Get money| B(Go shopping)\n    B --> C{Let me think}\n    C -->|One| D[Laptop]\n    C -->|Two| E[iPhone]\n    C -.->|Three| F[(Car DB)]\n    subgraph shop [The shop]\n      D\n      E\n    end\n    F ==> G((Done)) & H\n";

    #[test]
    fn links_are_read_with_their_heads_dashes_words_and_lengths() {
        let link = |text: &str| edge_at(text).map(|(edge, used)| (edge.heads, edge.dashed || edge.dotted, edge.thick, edge.length, edge.words, used));
        assert_eq!(link("--> B"), Some(([None, Some(shapes::FILLED_HEAD)], false, false, 1, String::new(), 3)));
        assert_eq!(link("---->B").unwrap().3, 3);
        assert_eq!(link("-.-> B").unwrap().1, true);
        assert_eq!(link("-..-> B").unwrap().3, 2);
        assert_eq!(link("==> B").unwrap().2, true);
        assert_eq!(link("--o B").unwrap().0, [None, Some(shapes::RING_HEAD)]);
        assert_eq!(link("--x B").unwrap().0, [None, Some(shapes::CROSS_HEAD)]);
        assert_eq!(link("<--> B").unwrap().0, [Some(shapes::FILLED_HEAD), Some(shapes::FILLED_HEAD)]);
        assert_eq!(link("x--x B").unwrap().0, [Some(shapes::CROSS_HEAD), Some(shapes::CROSS_HEAD)]);
        assert_eq!(link("-- words --> B").unwrap().4, "words");
        assert_eq!(link("-. maybe .-> B").unwrap().4, "maybe");
        assert_eq!(link("==|big|B").unwrap().4, "big");
        assert_eq!(link("-->|yes| B").unwrap().4, "yes");
        assert!(edge_at("~~~ B").unwrap().0.hidden);
        assert_eq!(link("--- B").unwrap().0, [None, None]);
        // An o after the dashes that begins a name is the name.
        assert_eq!(link("---oB").unwrap().0, [None, None]);
    }

    #[test]
    fn every_shape_of_node_is_read() {
        let form = |text: &str| node_at(text).and_then(|(_, given, _)| given).map(|(words, form)| (words, form));
        assert_eq!(form("A[Box]"), Some(("Box".into(), Form::Box)));
        assert_eq!(form("A(Round)").unwrap().1, Form::Round);
        assert_eq!(form("A([Stadium])").unwrap().1, Form::Stadium);
        assert_eq!(form("A[[Sub]]").unwrap().1, Form::Subroutine);
        assert_eq!(form("A[(DB)]").unwrap().1, Form::Database);
        assert_eq!(form("A((Circle))").unwrap().1, Form::Circle);
        assert_eq!(form("A(((Double)))").unwrap().1, Form::Double);
        assert_eq!(form("A>Flag]").unwrap().1, Form::Flag);
        assert_eq!(form("A{Diamond}").unwrap().1, Form::Diamond);
        assert_eq!(form("A{{Hex}}").unwrap().1, Form::Hexagon);
        assert_eq!(form("A[/Right/]").unwrap().1, Form::LeanRight);
        assert_eq!(form("A[\\Left\\]").unwrap().1, Form::LeanLeft);
        assert_eq!(form("A[/Trap\\]").unwrap().1, Form::Trapezoid);
        assert_eq!(form("A[\\Down/]").unwrap().1, Form::TrapezoidDown);
        assert_eq!(form("A[\"Quoted [x]\"]"), Some(("Quoted [x]".into(), Form::Box)));
        assert_eq!(form("A@{ shape: cyl, label: \"Store\" }"), Some(("Store".into(), Form::Database)));
        assert_eq!(node_at("my-node-->B").unwrap().0, "my-node");
        assert_eq!(node_at("A---B").unwrap().0, "A");
    }

    #[test]
    fn a_flowchart_is_read_into_boxes_and_arrows_and_written_back() {
        let flow = read_flow(FLOW);
        assert_eq!(flow.dir, Dir::Right);
        assert_eq!(flow.nodes.iter().map(|node| node.name.as_str()).collect::<Vec<_>>(), ["A", "B", "C", "D", "E", "F", "G", "H"]);
        assert_eq!(flow.edges.len(), 7);
        assert!(flow.edges.iter().any(|edge| edge.from == "C" && edge.to == "F" && edge.dotted && edge.words == "Three"), "dotted, with its words");
        assert!(flow.edges.iter().any(|edge| edge.from == "F" && edge.to == "G" && edge.thick), "thick");
        assert_eq!(flow.groups[0].members, ["D", "E"]);
        let mut studio = board();
        studio.pasted(None, Some(FLOW.to_owned()));
        let shapes = shapes_of(&studio);
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_DIAMOND && shape.text == "Let me think"));
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_CYLINDER));
        assert!(shapes.iter().any(|shape| shape.tool == Tool::Ellipse && shape.text == "Done"));
        assert_eq!(studio.links().len(), 7);
        // Laid out left to right: Christmas left of Go shopping.
        let x_of = |name: &str| shapes.iter().find(|shape| shape.text == name).unwrap().frame().0;
        assert!(x_of("Christmas") < x_of("Go shopping") && x_of("Go shopping") < x_of("Let me think"));
        // The shop holds Laptop and iPhone, and hugs them.
        let shop = shapes.iter().find(|shape| shape.text == "The shop").unwrap().id;
        assert_eq!(studio.inside(shop).len(), 2);
        assert_clear(&mut studio);
        assert_words_fit(&mut studio);
        assert_labels_clear(&mut studio);
        assert_hugs(&mut studio, 60.0);
        // Written back: the same nodes and edges, read again.
        studio.selected.clear();
        let back = read_flow(&studio.to_mermaid(false));
        assert_eq!((back.nodes.len(), back.edges.len(), back.groups.len()), (8, 7, 1));
        assert!(back.edges.iter().any(|edge| (edge.dotted || edge.dashed) && edge.words == "Three"));
    }

    /// Mermaid's own example of a flowchart with everything in it.
    const DOCS: &str = "---\ntitle: Order flow\n---\nflowchart TD\n    Start([Start]) --> Input[/Read order/]\n    Input --> Check{In stock?}\n    Check -->|Yes| Pack[[Pack it]]\n    Check -->|No| Back{{Back order}}\n    Back -.-> Wait[(Queue)]\n    Wait --> Check\n    Pack ==> Ship>Ship it]\n    Ship --> Done(((Done)))\n    Pack --o Note[\\Label\\]\n    Note --x Trap[/Box\\]\n    Ship ~~~ Trap\n    subgraph warehouse [Warehouse]\n      direction LR\n      Pack\n      Note\n    end\n    classDef hot fill:#f96,stroke:#333\n    class Back hot\n    style Done fill:#9f6\n    linkStyle 0 stroke:#f00,stroke-width:3px\n";

    #[test]
    fn a_flowchart_of_every_shape_and_link_is_laid_out_in_layers_clear_of_itself() {
        let graph = read_flow(DOCS);
        assert_eq!(graph.dir, Dir::Down);
        assert_eq!(graph.groups[0].dir, Some(Dir::Right));
        assert!(graph.edges.iter().any(|edge| edge.hidden));
        assert_eq!(graph.nodes.iter().find(|node| node.name == "Back").unwrap().fill, Some(FILLS[3]));
        assert_eq!(graph.nodes.iter().find(|node| node.name == "Done").unwrap().fill, Some(FILLS[1]));
        assert_eq!(graph.edges[0].color, Some(STROKES[1]));
        let mut studio = board();
        assert!(studio.bring_mermaid(DOCS));
        let shapes = shapes_of(&studio);
        for form in [shapes::FORM_STADIUM, shapes::FORM_LEAN_RIGHT, shapes::FORM_SUBROUTINE, shapes::FORM_HEXAGON, shapes::FORM_FLAG, shapes::FORM_LEAN_LEFT, shapes::FORM_TRAPEZOID] {
            assert!(shapes.iter().any(|shape| shape.form == form), "form {form}");
        }
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_DOUBLE && shape.tool == Tool::Ellipse));
        assert!(shapes.iter().any(|shape| shape.text == "Order flow" && shape.tool == Tool::Text), "its title");
        // Down its ranks: Start over Read order over In stock.
        let y_of = |name: &str| shapes.iter().find(|shape| shape.text == name).unwrap().frame().1;
        assert!(y_of("Start") < y_of("Read order") && y_of("Read order") < y_of("In stock?"));
        assert!(shapes.iter().any(|shape| shape.ends[1] == shapes::CROSS_HEAD && shape.heads[1]));
        assert!(shapes.iter().any(|shape| shape.ends[1] == shapes::RING_HEAD && shape.heads[1]));
        assert_clear(&mut studio);
        assert_words_fit(&mut studio);
        assert_labels_clear(&mut studio);
        assert_hugs(&mut studio, 60.0);
    }
}
