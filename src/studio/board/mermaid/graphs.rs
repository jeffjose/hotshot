//! Class, state and entity diagrams, read into graphs and laid out in
//! layers as flowcharts are. A class a table of its name, its attributes
//! and its methods between rules, its relations ending in hollow
//! triangles, diamonds or arrows with how many at each end; a state
//! rounded, the start a dot and the end a dot in a ring, forks and joins
//! bars, choices diamonds, composite states boxes of their own with their
//! own start and end, and notes beside them; an entity a table of its
//! attributes, its relationships ending in crow's feet with their words
//! between.

use super::flow::{Edge, Graph, Group, Node, GAPS};
use super::*;

/// A class read: its name, what it is marked as, its attributes and its
/// methods.
#[derive(Clone, Debug, Default)]
struct Class {
    name: String,
    label: String,
    marks: Vec<String>,
    attributes: Vec<String>,
    methods: Vec<String>,
}

/// Mermaid's generics, `List~int~`, as they are written, `List<int>`.
fn generic(words: &str) -> String {
    let mut out = String::new();
    let mut open = false;
    for c in words.chars() {
        if c == '~' {
            out.push(if open { '>' } else { '<' });
            open = !open;
        } else {
            out.push(c);
        }
    }
    out
}

/// A relation's end as a head: an inherited class's hollow triangle, a
/// whole's filled or hollow diamond, or an arrow.
fn class_head(mark: &str) -> Option<u8> {
    match mark {
        "<|" | "|>" => Some(shapes::HOLLOW_HEAD),
        "*" => Some(shapes::DIAMOND_HEAD),
        "o" => Some(shapes::HOLLOW_DIAMOND_HEAD),
        "<" | ">" => Some(shapes::OPEN_HEAD),
        _ => None,
    }
}

/// A relation between classes in a line, if it has one: what is left of
/// its link and right of it, the marks at each end, and whether it is
/// dashed.
fn relation(line: &str) -> Option<(String, String, String, String, bool)> {
    let (head, _) = line.split_once(" : ").or_else(|| line.split_once(':')).unwrap_or((line, ""));
    let at = head.find("--").or_else(|| head.find(".."))?;
    let dashed = head[at..].starts_with("..");
    let mut start = at;
    let left_mark = ["<|", "*", "o", "<"].iter().find(|mark| head[..at].ends_with(**mark) && (mark.len() > 1 || **mark == "<" || head[..at - mark.len()].ends_with(char::is_whitespace) || at == mark.len())).copied().unwrap_or("");
    start -= left_mark.len();
    let mut end = at + 2;
    let after = &head[end..];
    let right_mark = ["|>", "*", "o", ">"].iter().find(|mark| after.starts_with(**mark) && (mark.len() > 1 || **mark == ">" || after[mark.len()..].starts_with(char::is_whitespace) || after.len() == mark.len())).copied().unwrap_or("");
    end += right_mark.len();
    Some((head[..start].to_owned(), head[end..].to_owned(), left_mark.to_owned(), right_mark.to_owned(), dashed))
}

/// What is said of one end of a relation: the class, and how many, `"1"`.
fn end_of(side: &str, near_end: bool) -> (String, String) {
    let side = side.trim();
    let quoted = |words: &str| words.split('"').nth(1).map(str::to_owned).unwrap_or_default();
    match near_end {
        // The class on the left, how many after it: Name "1" --
        true => match side.find('"') {
            Some(at) => (side[..at].trim().to_owned(), quoted(&side[at..])),
            None => (side.to_owned(), String::new()),
        },
        // How many before the class on the right: -- "many" Name
        false => match side.rfind('"') {
            Some(at) => (side[at + 1..].trim().to_owned(), quoted(side)),
            None => (side.to_owned(), String::new()),
        },
    }
}

fn read_class(text: &str) -> Graph {
    let mut graph = Graph::default();
    let mut classes: Vec<Class> = Vec::new();
    let mut open: Option<usize> = None;
    let mut spaces: Vec<usize> = Vec::new();
    let mut notes: Vec<(Option<String>, String)> = Vec::new();
    let mut styles: Vec<(String, String)> = Vec::new();
    let mut given: Vec<(String, String)> = Vec::new();
    let class_at = |classes: &mut Vec<Class>, graph: &mut Graph, spaces: &[usize], name: &str| -> usize {
        let name = name.trim().split(":::").next().unwrap_or("").trim().to_owned();
        let (name, label) = match name.split_once('[') {
            Some((name, label)) => (name.trim().to_owned(), plain(label.trim_end_matches(']'))),
            None => (name.clone(), String::new()),
        };
        match classes.iter().position(|class| class.name == name) {
            Some(at) => {
                if !label.is_empty() {
                    classes[at].label = label;
                }
                at
            }
            None => {
                classes.push(Class { name: name.clone(), label, ..Class::default() });
                if let Some(&space) = spaces.last() {
                    graph.groups[space].members.push(name);
                }
                classes.len() - 1
            }
        }
    };
    let add_member = |class: &mut Class, member: &str| {
        let member = generic(member.trim().trim_end_matches(['$', '*']).trim());
        if member.is_empty() {
            return;
        }
        if member.starts_with("<<") {
            class.marks.push(member.replace("<<", "\u{ab}").replace(">>", "\u{bb}"));
        } else if member.contains('(') {
            class.methods.push(member);
        } else {
            class.attributes.push(member);
        }
    };
    for line in body(text) {
        if line == "}" {
            match open.take() {
                Some(_) => {}
                None => {
                    spaces.pop();
                }
            }
            continue;
        }
        if let Some(at) = open {
            add_member(&mut classes[at], &line);
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        let rest = line[first.len()..].trim();
        match first {
            "direction" => {
                graph.dir = Dir::named(rest).unwrap_or(graph.dir);
                continue;
            }
            "namespace" => {
                let name = rest.trim_end_matches('{').trim().to_owned();
                graph.groups.push(Group { name: name.clone(), title: name, ..Group::default() });
                if let Some(&outer) = spaces.last() {
                    let inner = graph.groups.len() - 1;
                    let name = graph.groups[inner].name.clone();
                    graph.groups[outer].members.push(name);
                }
                spaces.push(graph.groups.len() - 1);
                continue;
            }
            "class" => {
                let block = rest.ends_with('{');
                let name = rest.trim_end_matches('{').trim();
                let at = class_at(&mut classes, &mut graph, &spaces, &generic(name).replace(['<', '>'], "~").split('~').next().unwrap_or(name).to_owned());
                if name.contains('~') {
                    classes[at].label = generic(name.split('[').next().unwrap_or(name).trim());
                }
                if let Some((_, class)) = name.split_once(":::") {
                    given.push((classes[at].name.clone(), class.trim().to_owned()));
                }
                if block {
                    open = Some(at);
                }
                continue;
            }
            "note" => {
                let target = rest.strip_prefix("for ").map(|rest| rest.split_whitespace().next().unwrap_or("").to_owned());
                notes.push((target, plain(&rest[rest.find('"').unwrap_or(rest.len())..]).replace("\\n", "\n")));
                continue;
            }
            "classDef" => {
                let (names, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                for name in names.split(',') {
                    styles.push((format!("class:{}", name.trim()), style.to_owned()));
                }
                continue;
            }
            "style" => {
                let (name, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                styles.push((name.to_owned(), style.to_owned()));
                continue;
            }
            "cssClass" | "click" | "callback" | "link" => continue,
            _ => {}
        }
        if line.starts_with("<<") {
            if let Some((mark, name)) = line.split_once(">>") {
                let at = class_at(&mut classes, &mut graph, &spaces, name);
                classes[at].marks.push(format!("\u{ab}{}\u{bb}", mark.trim_start_matches("<<").trim()));
            }
            continue;
        }
        if let Some((left, right, left_mark, right_mark, dashed)) = relation(&line) {
            let words = line.split_once(" : ").or_else(|| line.split_once(':')).map_or(String::new(), |(_, words)| plain(words));
            let (a, a_many) = end_of(&left, true);
            let (b, b_many) = end_of(&right, false);
            if a.is_empty() || b.is_empty() {
                continue;
            }
            class_at(&mut classes, &mut graph, &spaces, &a);
            class_at(&mut classes, &mut graph, &spaces, &b);
            graph.edges.push(Edge { from: a, to: b, words, tips: [a_many, b_many], heads: [class_head(&left_mark), class_head(&right_mark)], dashed, length: 1, ..Edge::default() });
            continue;
        }
        // Name : member
        if let Some((name, member)) = line.split_once(':') {
            let at = class_at(&mut classes, &mut graph, &spaces, name);
            add_member(&mut classes[at], member);
        }
    }
    for class in &classes {
        let mut words: Vec<String> = class.marks.clone();
        words.push(if class.label.is_empty() { class.name.clone() } else { class.label.clone() });
        words.push("---".into());
        words.extend(class.attributes.iter().cloned());
        words.push("---".into());
        words.extend(class.methods.iter().cloned());
        graph.nodes.push(Node::new(&class.name, &words.join("\n"), Form::Table));
    }
    for (at, (target, words)) in notes.into_iter().enumerate() {
        let name = format!("note {at}");
        graph.nodes.push(Node::new(&name, &words, Form::Note));
        if let Some(target) = target.filter(|target| graph.nodes.iter().any(|node| &node.name == target)) {
            graph.edges.push(Edge { from: name, to: target, dashed: true, length: 1, ..Edge::default() });
        }
    }
    apply_styles(&mut graph, &styles, &given);
    graph
}

/// Colors from `classDef` and `style` lines, mapped to the board's.
fn apply_styles(graph: &mut Graph, styles: &[(String, String)], given: &[(String, String)]) {
    let fill_of = |style: &str| style.split([',', ';']).find_map(|part| part.split_once(':').filter(|(key, _)| key.trim() == "fill").and_then(|(_, value)| css_color(value)));
    let stroke_of = |style: &str| style.split([',', ';']).find_map(|part| part.split_once(':').filter(|(key, _)| key.trim() == "stroke").and_then(|(_, value)| css_color(value)));
    let mut apply = |name: &str, style: &str| {
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.name == name) {
            node.fill = fill_of(style).map(board_fill).or(node.fill);
            node.stroke = stroke_of(style).map(board_stroke).or(node.stroke);
        }
    };
    for (name, class) in given {
        if let Some((_, style)) = styles.iter().find(|(key, _)| *key == format!("class:{class}")) {
            apply(name, style);
        }
    }
    for (name, style) in styles.iter().filter(|(key, _)| !key.starts_with("class:")) {
        apply(name, style);
    }
}

/// A state diagram read: each `[*]` a start or an end of its own in the
/// composite state it is in.
fn read_state(text: &str) -> Graph {
    let mut graph = Graph::default();
    let mut scopes: Vec<usize> = Vec::new();
    let mut styles: Vec<(String, String)> = Vec::new();
    let mut given: Vec<(String, String)> = Vec::new();
    let mut note: Option<(String, String, Vec<String>)> = None;
    let mut notes = 0;
    let mut descriptions: Vec<(String, Vec<String>)> = Vec::new();
    // Regions of a composite state, split by `--`, each a box of its own.
    let scope_name = |scopes: &[usize], graph: &Graph| scopes.last().map_or("root".to_owned(), |&at| graph.groups[at].name.clone());
    let state = |graph: &mut Graph, scopes: &[usize], name: &str, start: bool| -> String {
        let name = name.trim().split(":::").next().unwrap_or("").trim();
        if name == "[*]" {
            let scope = scopes.last().map_or("root".to_owned(), |&at| graph.groups[at].name.clone());
            let made = format!("{}:{scope}", if start { "start" } else { "end" });
            if !graph.nodes.iter().any(|node| node.name == made) {
                graph.add(&made, Some((String::new(), if start { Form::Start } else { Form::End })), scopes);
            }
            return made;
        }
        if graph.groups.iter().any(|group| group.name == name) {
            return name.to_owned();
        }
        if !graph.nodes.iter().any(|node| node.name == name) {
            graph.add(name, Some((name.to_owned(), Form::Round)), scopes);
        }
        name.to_owned()
    };
    for line in body(text) {
        if let Some((target, side, lines)) = &mut note {
            if line.eq_ignore_ascii_case("end note") {
                let words = lines.join("\n");
                let name = format!("note {notes}");
                notes += 1;
                graph.add(&name, Some((words, Form::Note)), &scopes);
                let (from, to) = if side == "left" { (name.clone(), target.clone()) } else { (target.clone(), name.clone()) };
                graph.edges.push(Edge { from, to, dashed: true, length: 1, ..Edge::default() });
                note = None;
            } else {
                lines.push(line.clone());
            }
            continue;
        }
        if line == "}" {
            scopes.pop();
            // Out of a region, out of its composite state too.
            if scopes.last().is_some_and(|&at| graph.groups[at].title.is_empty() && graph.groups[at].name.contains(" region ")) {
                scopes.pop();
            }
            continue;
        }
        if line == "--" {
            // A new region of the composite state it is in.
            if let Some(&inner) = scopes.last() {
                let region_of = |graph: &Graph, at: usize| graph.groups[at].name.contains(" region ");
                let (composite, first) = match region_of(&graph, inner) {
                    true => (scopes[scopes.len() - 2], false),
                    false => (inner, true),
                };
                let base = graph.groups[composite].name.clone();
                if first {
                    // What is in it so far becomes its first region.
                    let members = std::mem::take(&mut graph.groups[composite].members);
                    let name = format!("{base} region 0");
                    graph.groups.push(Group { name: name.clone(), members, ..Group::default() });
                    graph.groups[composite].members.push(name);
                    scopes.push(graph.groups.len() - 1);
                }
                let count = graph.groups[composite].members.len();
                let name = format!("{base} region {count}");
                graph.groups.push(Group { name: name.clone(), ..Group::default() });
                graph.groups[composite].members.push(name);
                scopes.pop();
                scopes.push(graph.groups.len() - 1);
            }
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        let rest = line[first.len()..].trim();
        match first {
            "direction" => {
                let dir = Dir::named(rest);
                match scopes.last() {
                    Some(&at) => graph.groups[at].dir = dir,
                    None => graph.dir = dir.unwrap_or(graph.dir),
                }
                continue;
            }
            "classDef" => {
                let (names, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                styles.push((format!("class:{}", names.trim()), style.to_owned()));
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
                styles.push((name.to_owned(), style.to_owned()));
                continue;
            }
            "note" => {
                let lower = rest.to_lowercase();
                let side = if lower.starts_with("left") { "left" } else { "right" };
                let after = rest.splitn(3, ' ').nth(2).unwrap_or("");
                match after.split_once(':') {
                    Some((target, words)) => {
                        let name = format!("note {notes}");
                        notes += 1;
                        let target = state(&mut graph, &scopes, target, false);
                        graph.add(&name, Some((plain(words), Form::Note)), &scopes);
                        let (from, to) = if side == "left" { (name, target) } else { (target, name) };
                        graph.edges.push(Edge { from, to, dashed: true, length: 1, ..Edge::default() });
                    }
                    None => {
                        let target = state(&mut graph, &scopes, after, false);
                        note = Some((target, side.to_owned(), Vec::new()));
                    }
                }
                continue;
            }
            "state" => {
                // state "Words" as Name, state Name {, state Name <<fork>>.
                let opens = rest.ends_with('{');
                let rest = rest.trim_end_matches('{').trim();
                let (words, name) = match rest.split_once(" as ") {
                    Some((words, name)) if words.trim().starts_with('"') => (plain(words), name.trim().to_owned()),
                    Some((name, words)) => (plain(words), name.trim().to_owned()),
                    None => (String::new(), rest.split_whitespace().next().unwrap_or("").to_owned()),
                };
                let kind = rest.split("<<").nth(1).map(|kind| kind.trim_end_matches(">>").trim().to_lowercase());
                if opens {
                    let title = if words.is_empty() { name.clone() } else { words };
                    if let Some(&outer) = scopes.last() {
                        graph.groups[outer].members.push(name.clone());
                    }
                    graph.nodes.retain(|node| node.name != name);
                    graph.groups.push(Group { name, title, ..Group::default() });
                    scopes.push(graph.groups.len() - 1);
                    continue;
                }
                let form = match kind.as_deref() {
                    Some("fork" | "join") => Form::Bar,
                    Some("choice") => Form::Diamond,
                    _ => Form::Round,
                };
                let shown = if matches!(form, Form::Bar | Form::Diamond) { String::new() } else if words.is_empty() { name.clone() } else { words };
                graph.add(&name, Some((shown, form)), &scopes);
                continue;
            }
            _ => {}
        }
        if let Some((left, right)) = line.split_once("-->") {
            let (right, words) = right.split_once(':').map_or((right, ""), |(right, words)| (right, words));
            let a = state(&mut graph, &scopes, left, true);
            let b = state(&mut graph, &scopes, right, false);
            graph.edges.push(Edge { from: a, to: b, words: plain(words), heads: [None, Some(shapes::FILLED_HEAD)], length: 1, ..Edge::default() });
            continue;
        }
        // Name : description
        if let Some((name, words)) = line.split_once(':') {
            let name = state(&mut graph, &scopes, name, false);
            match descriptions.iter_mut().find(|(known, _)| *known == name) {
                Some((_, lines)) => lines.push(plain(words)),
                None => descriptions.push((name, vec![plain(words)])),
            }
            continue;
        }
        // A state named on its own.
        if !line.contains(' ') {
            state(&mut graph, &scopes, &line, false);
        }
        let _ = scope_name(&scopes, &graph);
    }
    for (name, lines) in descriptions {
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.name == name) {
            node.words = format!("{}\n---\n{}", node.words, lines.join("\n"));
            node.form = Form::Table;
        }
    }
    apply_styles(&mut graph, &styles, &given);
    graph
}

/// The crow's foot one end of an entity relationship says, from the two
/// marks at that end.
fn crow(mark: &str) -> u8 {
    match mark {
        "||" => shapes::ONE_HEAD,
        "|o" | "o|" => shapes::ZERO_ONE_HEAD,
        "}|" | "|{" => shapes::ONE_MANY_HEAD,
        _ => shapes::ZERO_MANY_HEAD,
    }
}

/// How many a relationship's end says in words, as Mermaid's newer way
/// writes it, as the two marks it stands for.
fn crow_words(words: &str) -> Option<&'static str> {
    match words.trim().to_lowercase().as_str() {
        "one" | "only one" | "1" => Some("||"),
        "zero or one" => Some("|o"),
        "one or more" | "one or many" | "many(1)" | "1+" => Some("}|"),
        "zero or more" | "zero or many" | "many(0)" | "0+" => Some("}o"),
        _ => None,
    }
}

fn read_er(text: &str) -> Graph {
    let mut graph = Graph::default();
    let mut entities: Vec<(String, String, Vec<Vec<String>>)> = Vec::new();
    let mut open: Option<usize> = None;
    let entity = |entities: &mut Vec<(String, String, Vec<Vec<String>>)>, name: &str| -> (usize, String) {
        let name = name.trim();
        let (name, label) = match name.split_once('[') {
            Some((name, label)) => (name.trim().to_owned(), plain(label.trim_end_matches(']'))),
            None => (plain(name), String::new()),
        };
        match entities.iter().position(|(known, ..)| *known == name) {
            Some(at) => {
                if !label.is_empty() {
                    entities[at].1 = label;
                }
                (at, name)
            }
            None => {
                entities.push((name.clone(), label, Vec::new()));
                (entities.len() - 1, name)
            }
        }
    };
    for line in body(text) {
        if line == "}" {
            open = None;
            continue;
        }
        if let Some(at) = open {
            // type name PK, FK "comment"
            let (cells, comment) = match line.split_once('"') {
                Some((cells, comment)) => (cells, comment.trim_end_matches('"').to_owned()),
                None => (line.as_str(), String::new()),
            };
            let words: Vec<&str> = cells.split_whitespace().collect();
            let mut row: Vec<String> = words.iter().take(2).map(|word| generic(word)).collect();
            let keys: Vec<&str> = words.iter().skip(2).map(|word| word.trim_matches(',')).filter(|word| !word.is_empty()).collect();
            row.push(keys.join(","));
            if !comment.is_empty() {
                row.push(comment);
            }
            entities[at].2.push(row);
            continue;
        }
        if line.starts_with("direction") {
            graph.dir = Dir::named(line["direction".len()..].trim()).unwrap_or(graph.dir);
            continue;
        }
        if let Some(name) = line.strip_suffix('{') {
            open = Some(entity(&mut entities, name).0);
            continue;
        }
        // A ||--o{ B : label
        let (head, label) = line.split_once(':').map_or((line.as_str(), ""), |(head, label)| (head, label));
        let words: Vec<&str> = head.split_whitespace().collect();
        let found = words.iter().position(|word| word.len() >= 4 && word.chars().all(|c| "|o}{-.".contains(c)) && (word.contains("--") || word.contains("..") || word.contains(".-") || word.contains("-.")));
        let parsed = match found {
            Some(at) if at > 0 && at + 1 < words.len() => {
                let link = words[at];
                Some((words[..at].join(" "), link[..2].to_owned(), link[link.len() - 2..].to_owned(), link.contains('.'), words[at + 1..].join(" ")))
            }
            _ => {
                // A one or more--zero or more B, in words.
                let link = head.find("--").or_else(|| head.find(".."));
                link.and_then(|at| {
                    let (left, right) = (&head[..at], &head[at + 2..]);
                    let left_words: Vec<&str> = left.split_whitespace().collect();
                    let right_words: Vec<&str> = right.split_whitespace().collect();
                    let (a, a_many) = (1..left_words.len()).find_map(|k| crow_words(&left_words[k..].join(" ")).map(|mark| (left_words[..k].join(" "), mark)))?;
                    let (b_many, b) = (1..right_words.len()).rev().find_map(|k| crow_words(&right_words[..k].join(" ")).map(|mark| (mark, right_words[k..].join(" "))))?;
                    let right_mark: String = b_many.chars().rev().map(|c| match c { '}' => '{', other => other }).collect();
                    Some((a, a_many.to_owned(), right_mark, head[at..].starts_with(".."), b))
                })
            }
        };
        let Some((a, left, right, dashed, b)) = parsed else {
            if !line.contains(' ') {
                entity(&mut entities, &line);
            }
            continue;
        };
        let (_, a) = entity(&mut entities, &a);
        let (_, b) = entity(&mut entities, &b);
        graph.edges.push(Edge { from: a, to: b, words: plain(label), heads: [Some(crow(&left)), Some(crow(&right))], dashed, length: 1, ..Edge::default() });
    }
    for (name, label, rows) in &entities {
        let mut words = vec![if label.is_empty() { name.clone() } else { label.clone() }];
        if !rows.is_empty() {
            words.push("---".into());
            words.extend(rows.iter().map(|row| row.join("\t")));
        }
        graph.nodes.push(Node::new(name, &words.join("\n"), Form::Table));
    }
    graph
}

impl Studio {
    pub(super) fn build_class(&mut self, text: &str, origin: Point) {
        let graph = read_class(text);
        self.build_graph(&graph, origin, front_title(text).as_deref(), GAPS);
    }

    pub(super) fn build_state(&mut self, text: &str, origin: Point) {
        let graph = read_state(text);
        self.build_graph(&graph, origin, front_title(text).as_deref(), GAPS);
    }

    pub(super) fn build_er(&mut self, text: &str, origin: Point) {
        let graph = read_er(text);
        self.build_graph(&graph, origin, front_title(text).as_deref(), layered::Gaps { node: 60.0, rank: 90.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::super::check::*;
    use super::super::tests::shapes_of;
    use super::*;
    use crate::studio::board::tests::*;

    /// Mermaid's own class diagram example.
    const CLASSES: &str = "classDiagram\n    note \"From Duck till Zebra\"\n    Animal <|-- Duck\n    note for Duck \"can fly\\ncan swim\"\n    Animal <|-- Fish\n    Animal <|-- Zebra\n    Animal : +int age\n    Animal : +String gender\n    Animal: +isMammal()\n    Animal: +mate()\n    class Duck{\n        +String beakColor\n        +swim()\n        +quack()\n    }\n    class Fish{\n        -int sizeInFeet\n        -canEat()\n    }\n    class Zebra{\n        <<interface>>\n        +bool is_wild\n        +run()\n    }\n    Customer \"1\" --> \"*\" Ticket : buys\n    Zoo *-- Animal\n    Zoo o-- Keeper\n    Keeper ..> Ticket\n    class List~int~\n";

    #[test]
    fn a_class_diagram_is_classes_in_compartments_and_relations_with_their_ends() {
        let graph = read_class(CLASSES);
        let duck = graph.nodes.iter().find(|node| node.name == "Duck").unwrap();
        assert_eq!(duck.words, "Duck\n---\n+String beakColor\n---\n+swim()\n+quack()");
        assert!(graph.nodes.iter().find(|node| node.name == "Zebra").unwrap().words.starts_with("\u{ab}interface\u{bb}\nZebra"));
        let edge = |from: &str, to: &str| graph.edges.iter().find(|edge| edge.from == from && edge.to == to).unwrap().clone();
        assert_eq!(edge("Animal", "Duck").heads, [Some(shapes::HOLLOW_HEAD), None]);
        assert_eq!(edge("Zoo", "Animal").heads, [Some(shapes::DIAMOND_HEAD), None]);
        assert_eq!(edge("Zoo", "Keeper").heads, [Some(shapes::HOLLOW_DIAMOND_HEAD), None]);
        let buys = edge("Customer", "Ticket");
        assert_eq!((buys.tips.clone(), buys.words.as_str(), buys.heads), (["1".to_owned(), "*".to_owned()], "buys", [None, Some(shapes::OPEN_HEAD)]));
        assert!(edge("Keeper", "Ticket").dashed);
        assert!(graph.nodes.iter().any(|node| node.words.starts_with("List<int>")));
        assert!(graph.nodes.iter().any(|node| node.form == Form::Note && node.words == "can fly\ncan swim"));
        let mut studio = board();
        assert!(studio.bring_mermaid(CLASSES));
        let shapes = shapes_of(&studio);
        // Inherited from: Animal over Duck, the triangle at Animal.
        let frame = |name: &str| shapes.iter().find(|shape| shape.text.lines().any(|line| line == name) && shape.form == shapes::FORM_TABLE).unwrap().frame();
        assert!(frame("Animal").1 + frame("Animal").3 <= frame("Duck").1);
        assert!(shapes.iter().any(|shape| shape.text == "1" && shape.tool == Tool::Text), "how many, at the end");
        assert_clear(&mut studio);
        assert_words_fit(&mut studio);
        assert_labels_clear(&mut studio);
    }

    /// Mermaid's own state diagram example, with a composite state, a
    /// fork and a join, a choice and notes.
    const STATES: &str = "stateDiagram-v2\n    [*] --> Still\n    Still --> [*]\n    Still --> Moving\n    Moving --> Still\n    Moving --> Crash\n    Crash --> [*]\n    state Moving {\n        direction LR\n        [*] --> Slow\n        Slow --> Fast : speed up\n        Fast --> Slow : slow down\n    }\n    state fork_state <<fork>>\n    Crash --> fork_state\n    fork_state --> Report\n    fork_state --> Tow\n    state join_state <<join>>\n    Report --> join_state\n    Tow --> join_state\n    join_state --> Check\n    state if_state <<choice>>\n    Check --> if_state\n    if_state --> Fixed : cheap\n    if_state --> Scrapped : costly\n    Fixed : back on the road\n    note right of Crash : Call the insurer\n    note left of Check\n        The garage looks\n        at the damage\n    end note\n";

    #[test]
    fn a_state_diagram_has_its_dots_bars_choices_composites_and_notes() {
        let graph = read_state(STATES);
        assert!(graph.nodes.iter().any(|node| node.name == "start:root" && node.form == Form::Start));
        assert!(graph.nodes.iter().any(|node| node.name == "start:Moving" && node.form == Form::Start));
        assert!(graph.nodes.iter().any(|node| node.name == "end:root" && node.form == Form::End));
        assert_eq!(graph.nodes.iter().find(|node| node.name == "fork_state").unwrap().form, Form::Bar);
        assert_eq!(graph.nodes.iter().find(|node| node.name == "if_state").unwrap().form, Form::Diamond);
        assert_eq!(graph.nodes.iter().find(|node| node.name == "Fixed").unwrap().words, "Fixed\n---\nback on the road");
        let moving = graph.groups.iter().find(|group| group.name == "Moving").unwrap();
        assert!(moving.members.contains(&"Slow".to_owned()) && moving.dir == Some(Dir::Right));
        assert!(graph.nodes.iter().any(|node| node.form == Form::Note && node.words == "The garage looks\nat the damage"));
        let mut studio = board();
        assert!(studio.bring_mermaid(STATES));
        let shapes = shapes_of(&studio);
        assert!(shapes.iter().any(|shape| shape.tool == Tool::Ellipse && shape.form == shapes::FORM_DOUBLE && shape.filled), "an end");
        assert_clear(&mut studio);
        assert_words_fit(&mut studio);
        assert_labels_clear(&mut studio);
        assert_hugs(&mut studio, 60.0);
    }

    /// Mermaid's own entity relationship example.
    const ENTITIES: &str = "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER ||--|{ LINE-ITEM : contains\n    CUSTOMER }|..|{ DELIVERY-ADDRESS : uses\n    PRODUCT |o--o| LINE-ITEM : \"is in\"\n    CUSTOMER {\n        string name\n        string custNumber PK \"the key\"\n        string sector\n    }\n    ORDER {\n        int orderNumber PK\n        string deliveryAddress FK\n    }\n    LINE-ITEM {\n        string productCode\n        int quantity\n        float pricePerUnit\n    }\n";

    #[test]
    fn an_er_diagram_is_tables_joined_by_crows_feet() {
        let graph = read_er(ENTITIES);
        let edge = |from: &str| graph.edges.iter().find(|edge| edge.from == from).unwrap().clone();
        assert_eq!(edge("CUSTOMER").heads, [Some(shapes::ONE_HEAD), Some(shapes::ZERO_MANY_HEAD)]);
        assert_eq!(edge("ORDER").heads, [Some(shapes::ONE_HEAD), Some(shapes::ONE_MANY_HEAD)]);
        assert_eq!(edge("PRODUCT").heads, [Some(shapes::ZERO_ONE_HEAD), Some(shapes::ZERO_ONE_HEAD)]);
        assert_eq!(edge("PRODUCT").words, "is in");
        assert!(graph.edges.iter().any(|edge| edge.dashed));
        let customer = graph.nodes.iter().find(|node| node.name == "CUSTOMER").unwrap();
        assert_eq!(customer.words, "CUSTOMER\n---\nstring\tname\t\nstring\tcustNumber\tPK\tthe key\nstring\tsector\t");
        assert!(read_er("erDiagram\n  A one or more--zero or more B : has\n").edges[0].heads == [Some(shapes::ONE_MANY_HEAD), Some(shapes::ZERO_MANY_HEAD)]);
        let mut studio = board();
        assert!(studio.bring_mermaid(ENTITIES));
        assert_eq!(shapes_of(&studio).iter().filter(|shape| shape.form == shapes::FORM_TABLE).count(), 5);
        assert_clear(&mut studio);
        assert_words_fit(&mut studio);
        assert_labels_clear(&mut studio);
    }
}
