//! Charts: gantt, pie, timeline, journey, mind map, git graph, quadrant.

use super::flow::node_at;
use super::*;

impl Studio {
    pub(super) fn build_mindmap(&mut self, text: &str, origin: (f32, f32)) {
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
    pub(super) fn build_gantt(&mut self, text: &str, origin: (f32, f32)) {
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

    pub(super) fn build_pie(&mut self, text: &str, origin: (f32, f32)) {
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

    pub(super) fn build_timeline(&mut self, text: &str, origin: (f32, f32)) {
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

    pub(super) fn build_journey(&mut self, text: &str, origin: (f32, f32)) {
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

    pub(super) fn build_git(&mut self, text: &str, origin: (f32, f32)) {
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

    pub(super) fn build_quadrant(&mut self, text: &str, origin: (f32, f32)) {
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
}
