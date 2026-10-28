//! The charts Mermaid draws, drawn as Mermaid draws them and made of the
//! board's own shapes: a gantt chart's sections in bands, its tasks bars
//! along a time axis of dates, done, active, critical or milestones; a
//! pie of slices, each its share, with its key beside it; a timeline's
//! periods along a line with their events under them; a journey's tasks
//! in their sections, each with a face as glad as its score; a mind map
//! out to either side of its root, each branch its own color; a git
//! graph's branches as lanes of commits, branching off and merging; and
//! a quadrant chart's four quadrants, axes and points.

use super::flow::node_at;
use super::*;

/// The colors slices, sections and branches are told apart by, strong
/// enough to fill a slice: the board's blue, yellow, green, red and
/// violet, then cyan, orange and pink, a shade deeper than its fills.
const STRONG: [u32; 8] = [0x74C0FC, 0xFFD43B, 0x8CE99A, 0xFFA8A8, 0xB197FC, 0x66D9E8, 0xFFC078, 0xFAA2C1];

/// The words after a keyword at the start of a line, if it starts so.
fn after<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(word)?;
    (rest.is_empty() || rest.starts_with([' ', ':', '\t'])).then(|| rest.trim_start_matches(':').trim())
}

/// Days since 1970-01-01 of a day in the calendar, and back.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let of_era = z - era * 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 { shifted + 3 } else { shifted - 9 };
    (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}

/// A date and time as a gantt chart's `dateFormat` says it is written, as
/// days since 1970, the time of day the part of a day after the point.
fn read_date(words: &str, format: &str) -> Option<f64> {
    let (mut year, mut month, mut day, mut hour, mut minute) = (1970i64, 1i64, 1i64, 0i64, 0i64);
    let (format, words) = (format.as_bytes(), words.trim().as_bytes());
    let (mut at, mut read) = (0, 0);
    let number = |read: &mut usize, most: usize| -> Option<i64> {
        let start = *read;
        while *read < words.len() && *read - start < most && words[*read].is_ascii_digit() {
            *read += 1;
        }
        std::str::from_utf8(&words[start..*read]).ok()?.parse().ok()
    };
    let mut any = false;
    while at < format.len() {
        let rest = &format[at..];
        let token = ["YYYY", "YY", "MM", "M", "DD", "D", "HH", "H", "mm", "ss"].iter().find(|token| rest.starts_with(token.as_bytes()));
        match token {
            Some(&token) => {
                let value = number(&mut read, if token.len() == 1 { 2 } else { token.len() })?;
                match token {
                    "YYYY" => year = value,
                    "YY" => year = 2000 + value,
                    "MM" | "M" => month = value,
                    "DD" | "D" => day = value,
                    "HH" | "H" => hour = value,
                    "mm" => minute = value,
                    _ => {}
                }
                at += token.len();
                any = true;
            }
            None => {
                if read < words.len() && words[read] != format[at] {
                    return None;
                }
                read += 1;
                at += 1;
            }
        }
    }
    (any && read >= words.len() && (1..=12).contains(&month) && (1..=31).contains(&day)).then(|| days_from_civil(year, month, day) as f64 + (hour as f64 + minute as f64 / 60.0) / 24.0)
}

/// A length of time as a gantt chart writes it, `3d`, `2w`, `12h`, in days.
fn read_length(words: &str) -> Option<f64> {
    let words = words.trim();
    let split = words.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let value: f64 = words[..split].parse().ok()?;
    let days = match &words[split..] {
        "d" => value,
        "w" => value * 7.0,
        "h" => value / 24.0,
        "m" => value / 1440.0,
        "s" => value / 86_400.0,
        "ms" => value / 86_400_000.0,
        _ => return None,
    };
    Some(days)
}

/// A date as an axis's format writes it: %Y, %y, %m, %d, %e, %b, %B, %a,
/// %H and %M.
fn write_date(days: f64, format: &str) -> String {
    let whole = days.floor() as i64;
    let (year, month, day) = civil_from_days(whole);
    let minutes = ((days - whole as f64) * 1440.0).round() as i64;
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out += &year.to_string(),
            Some('y') => out += &format!("{:02}", year % 100),
            Some('m') => out += &format!("{month:02}"),
            Some('d') => out += &format!("{day:02}"),
            Some('e') => out += &day.to_string(),
            Some('b') => out += &MONTHS[(month - 1) as usize][..3],
            Some('B') => out += MONTHS[(month - 1) as usize],
            Some('a') => out += DAYS[(whole + 4).rem_euclid(7) as usize],
            Some('H') => out += &format!("{:02}", minutes / 60),
            Some('M') => out += &format!("{:02}", minutes % 60),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Whether a day is a Saturday or a Sunday.
fn weekend(days: f64) -> bool {
    matches!((days.floor() as i64 + 4).rem_euclid(7), 0 | 6)
}

/// A gantt chart's task: its section, words, where it starts and ends in
/// days, and whether it is done, active, critical or a milestone.
#[derive(Clone, Debug, Default)]
struct Task {
    section: String,
    words: String,
    start: f64,
    end: f64,
    done: bool,
    active: bool,
    crit: bool,
    milestone: bool,
}

/// A gantt chart read: its title, its tasks, how its axis writes dates
/// and how often, and whether to mark today.
#[derive(Clone, Debug, Default)]
struct Gantt {
    title: String,
    tasks: Vec<Task>,
    axis: String,
    every: Option<f64>,
    today: bool,
}

fn read_gantt(text: &str) -> Gantt {
    let mut gantt = Gantt { title: front_title(text).unwrap_or_default(), axis: "%Y-%m-%d".into(), today: true, ..Gantt::default() };
    let mut format = "YYYY-MM-DD".to_owned();
    let mut section = String::new();
    let mut weekends = false;
    let mut ends: Vec<(String, f64, f64)> = Vec::new();
    let mut last_end: Option<f64> = None;
    let mut untils: Vec<(usize, String)> = Vec::new();
    for line in body(text) {
        if let Some(rest) = after(&line, "title") {
            gantt.title = rest.to_owned();
            continue;
        }
        if let Some(rest) = after(&line, "section") {
            section = rest.to_owned();
            continue;
        }
        if let Some(rest) = after(&line, "dateFormat") {
            format = rest.to_owned();
            continue;
        }
        if let Some(rest) = after(&line, "axisFormat") {
            gantt.axis = rest.to_owned();
            continue;
        }
        if let Some(rest) = after(&line, "excludes") {
            weekends |= rest.contains("weekends");
            continue;
        }
        if let Some(rest) = after(&line, "todayMarker") {
            gantt.today = rest.trim() != "off";
            continue;
        }
        if let Some(rest) = after(&line, "tickInterval") {
            gantt.every = match rest.trim() {
                every if every.ends_with("day") => every.trim_end_matches("day").parse::<f64>().ok(),
                every if every.ends_with("week") => every.trim_end_matches("week").parse::<f64>().ok().map(|weeks| weeks * 7.0),
                every if every.ends_with("month") => every.trim_end_matches("month").parse::<f64>().ok().map(|months| months * 30.0),
                _ => None,
            };
            continue;
        }
        if ["includes", "weekday", "weekend", "inclusiveEndDates", "topAxis", "displayMode", "accTitle", "accDescr", "click"].iter().any(|word| after(&line, word).is_some()) {
            continue;
        }
        let Some((name, spec)) = line.split_once(':') else { continue };
        let mut task = Task { section: section.clone(), words: plain(name), ..Task::default() };
        let mut parts: Vec<&str> = Vec::new();
        for part in spec.split(',').map(str::trim) {
            match part {
                "done" => task.done = true,
                "active" => task.active = true,
                "crit" => task.crit = true,
                "milestone" => task.milestone = true,
                "" => {}
                _ => parts.push(part),
            }
        }
        let is_start = |part: &str| part.starts_with("after ") || read_date(part, &format).is_some();
        let (id, start, length) = match parts[..] {
            [length] => (None, None, Some(length)),
            [first, length] if is_start(first) => (None, Some(first), Some(length)),
            [id, length] => (Some(id), None, Some(length)),
            [id, start, length, ..] => (Some(id), Some(start), Some(length)),
            [] => (None, None, None),
        };
        let begin = match start {
            Some(start) if start.starts_with("after ") => start["after ".len()..].split_whitespace().filter_map(|known| ends.iter().find(|(name, ..)| name == known).map(|&(_, _, end)| end)).fold(f64::MIN, f64::max),
            Some(start) => read_date(start, &format).unwrap_or(last_end.unwrap_or(0.0)),
            None => last_end.unwrap_or(0.0),
        };
        let begin = if begin == f64::MIN { last_end.unwrap_or(0.0) } else { begin };
        let end = match length {
            Some(length) if length.starts_with("until ") => length["until ".len()..].split_whitespace().filter_map(|known| ends.iter().find(|(name, ..)| name == known).map(|&(_, start, _)| start)).fold(f64::MAX, f64::min),
            Some(length) => match (read_length(length), read_date(length, &format)) {
                (Some(days), _) if weekends => {
                    // Its days counted on working days only.
                    let (mut left, mut at) = (days, begin);
                    while left > 1e-9 {
                        if weekend(at) {
                            at = at.floor() + 1.0;
                        } else {
                            let take = left.min(at.floor() + 1.0 - at).max(1e-6);
                            (left, at) = (left - take, at + take);
                        }
                    }
                    at
                }
                (Some(days), _) => begin + days,
                (None, Some(date)) => date,
                _ => begin + 1.0,
            },
            None => begin + 1.0,
        };
        // Until a task not read yet: worked out once all are.
        let later = (end == f64::MAX).then(|| length.unwrap_or("").trim_start_matches("until ").to_owned());
        let end = if end == f64::MAX { begin + 1.0 } else { end };
        task.start = begin;
        task.end = if task.milestone { begin } else { end.max(begin) };
        if let Some(id) = id {
            ends.push((id.to_owned(), task.start, task.end));
        }
        last_end = Some(task.end);
        gantt.tasks.push(task);
        if let Some(until) = later {
            untils.push((gantt.tasks.len() - 1, until));
        }
    }
    for (at, until) in untils {
        if let Some(start) = until.split_whitespace().filter_map(|known| ends.iter().find(|(name, ..)| name == known).map(|&(_, start, _)| start)).reduce(f64::min) {
            gantt.tasks[at].end = start.max(gantt.tasks[at].start);
        }
    }
    gantt
}

/// A mind map's node: how deep it is, its words and its shape.
type MapNode = (usize, String, Form);

fn read_mindmap(text: &str) -> Vec<MapNode> {
    let mut nodes: Vec<MapNode> = Vec::new();
    let mut indents: Vec<usize> = Vec::new();
    for line in after_front(text).lines().skip_while(|line| !line.trim_start().starts_with("mindmap")).skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed.starts_with("::icon") || trimmed.starts_with(":::") {
            continue;
        }
        let indent = line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>();
        while indents.last().is_some_and(|&known| known > indent) {
            indents.pop();
        }
        if indents.last() != Some(&indent) {
            indents.push(indent);
        }
        let depth = indents.len() - 1;
        let words = trimmed.split(":::").next().unwrap_or(trimmed).trim();
        // Its shape: [square], (rounded), ((circle)), ))bang((, )cloud(,
        // {{hexagon}}, or none.
        let shaped = [("))", "((", Form::Stadium), (")", "(", Form::Round), ("((", "))", Form::Circle), ("[", "]", Form::Box), ("(", ")", Form::Round), ("{{", "}}", Form::Hexagon)].iter().find_map(|&(open, close, form)| {
            let start = words.find(open)?;
            let inner = &words[start + open.len()..];
            let end = inner.rfind(close)?;
            (start == 0 || words[..start].chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')).then(|| (plain(&inner[..end]), form))
        });
        let (words, form) = match shaped {
            Some((words, Form::Stadium)) => (words, Form::Stadium),
            Some(found) => found,
            None => match node_at(words) {
                Some((_, Some(found), used)) if used == words.len() => found,
                _ => (plain(words), Form::Round),
            },
        };
        nodes.push((depth, words, form));
    }
    nodes
}

impl Studio {
    /// A plain box of a diagram, of a size and place of its own, filled
    /// with a color, its words in it.
    fn chart_box(&mut self, words: &str, frame: (f32, f32, f32, f32), fill: u32) -> usize {
        let index = self.diagram_box(words, Form::Box, (frame.0 + frame.2 / 2.0, frame.1 + frame.3 / 2.0));
        self.frame_to(index, frame);
        if let Some(shape) = self.shape_mut(index) {
            (shape.fill, shape.filled) = (fill, true);
        }
        index
    }

    /// A dot of a color with its middle at a point.
    fn chart_dot(&mut self, middle: Point, radius: f32, fill: u32) -> usize {
        let index = self.diagram_box("", Form::Circle, middle);
        self.frame_to(index, (middle.0 - radius, middle.1 - radius, radius * 2.0, radius * 2.0));
        if let Some(shape) = self.shape_mut(index) {
            (shape.fill, shape.filled, shape.color) = (fill, true, INK);
        }
        index
    }

    /// A thin quiet line, not drawn by hand: a grid's, or a lane's.
    fn chart_rule(&mut self, from: Point, to: Point, color: u32, dashed: bool) -> usize {
        let line = self.diagram_line(from, to, false, dashed);
        if let Some(shape) = self.shape_mut(line) {
            (shape.color, shape.sketch, shape.stroke) = (color, false, 1.0);
        }
        line
    }

    pub(super) fn build_gantt(&mut self, text: &str, origin: Point) {
        let gantt = read_gantt(text);
        if gantt.tasks.is_empty() {
            return;
        }
        let px = self.board.px;
        let small = 16;
        let first = gantt.tasks.iter().map(|task| task.start).fold(f64::MAX, f64::min);
        let last = gantt.tasks.iter().map(|task| task.end).fold(f64::MIN, f64::max);
        let span = (last - first).max(1.0);
        // So many across a day that the whole is about as wide as a page.
        let day = (900.0 / span as f32).clamp(4.0, 80.0);
        let sections: Vec<String> = gantt.tasks.iter().fold(Vec::new(), |mut seen, task| {
            if !seen.contains(&task.section) {
                seen.push(task.section.clone());
            }
            seen
        });
        let label_room = sections.iter().map(|section| self.words_wide(section, px)).fold(0.0, f32::max) + if sections.iter().any(|section| !section.is_empty()) { 32.0 } else { 0.0 };
        let (row, bar) = (Text::line_height(px) as f32 + 22.0, Text::line_height(px) as f32 + 10.0);
        let left = origin.0 + label_room;
        let x_of = |when: f64| left + ((when - first) as f32) * day;
        let width = x_of(last) - left;
        let top = origin.1;
        let rows = gantt.tasks.len() as f32;
        let bottom = top + rows * row;
        // The ticks of the axis: days, weeks or months, as many as fit.
        let every = gantt.every.unwrap_or(match span {
            span if span <= 10.0 => 1.0,
            span if span <= 70.0 => 7.0,
            span if span <= 400.0 => 30.0,
            _ => 91.0,
        });
        let mut ticks: Vec<f64> = Vec::new();
        let mut tick = first.floor();
        if every >= 28.0 {
            // From the first of a month.
            let (year, month, _) = civil_from_days(tick as i64);
            tick = days_from_civil(year, month, 1) as f64;
            while tick <= last {
                if tick >= first {
                    ticks.push(tick);
                }
                let (year, month, _) = civil_from_days(tick as i64);
                let step = (every / 30.0).round().max(1.0) as i64;
                let (year, month) = (year + (month - 1 + step) / 12, (month - 1 + step) % 12 + 1);
                tick = days_from_civil(year, month, 1) as f64;
            }
        } else {
            while tick <= last + 1e-6 {
                ticks.push(tick);
                tick += every;
            }
        }
        let tick_words: Vec<String> = ticks.iter().map(|&tick| write_date(tick, &gantt.axis)).collect();
        let widest_tick = tick_words.iter().map(|words| self.words_wide(words, small)).fold(0.0, f32::max) + 12.0;
        let step = (ticks.len() as f32 * widest_tick / width.max(1.0)).ceil().max(1.0) as usize;
        // The sections, each a band of its own color across the chart.
        let mut at = 0usize;
        for (k, section) in sections.iter().enumerate() {
            let count = gantt.tasks.iter().filter(|task| &task.section == section).count();
            let band = (origin.0, top + at as f32 * row, label_room + width, count as f32 * row);
            let index = self.chart_box("", band, if k % 2 == 0 { 0xF1F3F5 } else { 0xFFFFFF });
            if let Some(shape) = self.shape_mut(index) {
                (shape.border, shape.sketch) = (false, false);
            }
            if !section.is_empty() {
                let tall = Self::words_tall(section, px);
                self.diagram_words(section, (origin.0 + 12.0, band.1 + band.3 / 2.0 - tall / 2.0), px);
            }
            at += count;
        }
        // The grid, a line down at each tick, its date under it.
        for (k, (&tick, words)) in ticks.iter().zip(&tick_words).enumerate() {
            let x = x_of(tick);
            self.chart_rule((x, top), (x, bottom + 6.0), 0xCED4DA, false);
            if k % step == 0 {
                self.words_at(words, (x, bottom + 8.0 + Text::line_height(small) as f32 / 2.0), small);
            }
        }
        // Today, a red line, where it falls inside.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |since| since.as_secs_f64() / 86_400.0);
        if gantt.today && now >= first && now <= last {
            self.chart_rule((x_of(now), top - 8.0), (x_of(now), bottom + 8.0), STROKES[1], false);
        }
        // The tasks, by section in order, each a bar from its start to its
        // end, its words in it where they fit and after it where not.
        let mut row_at = 0usize;
        for section in &sections {
            for task in gantt.tasks.iter().filter(|task| &task.section == section) {
                let y = top + row_at as f32 * row + (row - bar) / 2.0;
                let (x0, x1) = (x_of(task.start), x_of(task.end));
                let (fill, stroke) = match (task.done, task.active, task.crit) {
                    (true, _, crit) => (0xE9ECEF, if crit { STROKES[1] } else { QUIET }),
                    (_, true, crit) => (FILLS[4], if crit { STROKES[1] } else { 0x7048E8 }),
                    (_, _, true) => (FILLS[3], STROKES[1]),
                    _ => (FILLS[2], STROKES[3]),
                };
                let wide = self.words_wide(&task.words, small) + 32.0;
                if task.milestone {
                    let index = self.diagram_box("", Form::Diamond, (x0, y + bar / 2.0));
                    self.frame_to(index, (x0 - bar / 2.0, y, bar, bar));
                    if let Some(shape) = self.shape_mut(index) {
                        (shape.fill, shape.filled, shape.color) = (fill, true, stroke);
                    }
                    self.diagram_words(&task.words, (x0 + bar / 2.0 + 8.0, y + (bar - Text::line_height(small) as f32) / 2.0), small);
                } else {
                    let fits = x1 - x0 >= wide;
                    let index = self.chart_box(if fits { &task.words } else { "" }, (x0, y, (x1 - x0).max(4.0), bar), fill);
                    if let Some(shape) = self.shape_mut(index) {
                        (shape.color, shape.font_px) = (stroke, small);
                    }
                    if !fits {
                        self.diagram_words(&task.words, (x1 + 8.0, y + (bar - Text::line_height(small) as f32) / 2.0), small);
                    }
                }
                row_at += 1;
            }
        }
        if !gantt.title.is_empty() {
            self.diagram_title(&gantt.title, origin.0 + (label_room + width) / 2.0, top - 16.0);
        }
    }

    pub(super) fn build_pie(&mut self, text: &str, origin: Point) {
        let mut slices: Vec<(String, f32)> = Vec::new();
        let head = header(text);
        let shows = head.contains("showData");
        let mut title = front_title(text).unwrap_or_default();
        if let Some((_, rest)) = head.split_once("title") {
            title = rest.trim().to_owned();
        }
        for line in body(text) {
            if let Some(rest) = after(&line, "title") {
                title = rest.to_owned();
                continue;
            }
            if let Some((name, value)) = line.rsplit_once(':') {
                if let Ok(value) = value.trim().parse::<f32>() {
                    slices.push((plain(name), value.max(0.0)));
                }
            }
        }
        let total: f32 = slices.iter().map(|slice| slice.1).sum::<f32>().max(1e-3);
        let radius = 170.0;
        let px = self.board.px;
        // Each slice its share of the turn, from the top round clockwise,
        // its share written in it.
        let mut from = 0.0f32;
        for (at, (_, value)) in slices.iter().enumerate() {
            let sweep = value / total * 360.0;
            let index = self.diagram_box("", Form::Circle, origin);
            self.frame_to(index, (origin.0 - radius, origin.1 - radius, radius * 2.0, radius * 2.0));
            if let Some(shape) = self.shape_mut(index) {
                (shape.form, shape.corners, shape.fill, shape.filled) = (shapes::FORM_WEDGE, Some([from, sweep, 0.0, 0.0]), STRONG[at % STRONG.len()], true);
            }
            if sweep >= 8.0 {
                let middle = (from + sweep / 2.0 - 90.0).to_radians();
                let reach = if slices.len() == 1 { 0.0 } else { radius * 0.62 };
                self.words_at(&format!("{:.0}%", value / total * 100.0), (origin.0 + reach * middle.cos(), origin.1 + reach * middle.sin()), px);
            }
            from += sweep;
        }
        // Its key: a square of each color and its name, beside it.
        let row = Text::line_height(px) as f32 + 12.0;
        let key_top = origin.1 - row * slices.len() as f32 / 2.0;
        for (at, (name, value)) in slices.iter().enumerate() {
            let y = key_top + at as f32 * row;
            let square = self.chart_box("", (origin.0 + radius + 48.0, y + row / 2.0 - 10.0, 20.0, 20.0), STRONG[at % STRONG.len()]);
            if let Some(shape) = self.shape_mut(square) {
                shape.sketch = false;
            }
            let words = if shows { format!("{name} [{value}]") } else { name.clone() };
            self.diagram_words(&words, (origin.0 + radius + 80.0, y + (row - Text::line_height(px) as f32) / 2.0), px);
        }
        if !title.is_empty() {
            self.diagram_title(&title, origin.0, origin.1 - radius - 24.0);
        }
    }

    pub(super) fn build_timeline(&mut self, text: &str, origin: Point) {
        // Its sections, each with its periods, each with its events.
        let mut sections: Vec<(String, Vec<(String, Vec<String>)>)> = Vec::new();
        let mut title = front_title(text).unwrap_or_default();
        for line in body(text) {
            if let Some(rest) = after(&line, "title") {
                title = rest.to_owned();
                continue;
            }
            if let Some(rest) = after(&line, "section") {
                sections.push((rest.to_owned(), Vec::new()));
                continue;
            }
            if sections.is_empty() {
                sections.push((String::new(), Vec::new()));
            }
            let periods = &mut sections.last_mut().unwrap().1;
            let mut parts = line.split(" : ").map(str::trim);
            let first = parts.next().unwrap_or("").trim_start_matches(':').trim();
            let events: Vec<String> = parts.chain(std::iter::once("")).filter(|part| !part.is_empty()).map(plain).collect();
            match (line.trim_start().starts_with(':'), periods.last_mut()) {
                (true, Some(last)) => last.1.extend(std::iter::once(plain(first)).chain(events)),
                _ => periods.push((plain(first), events)),
            }
        }
        let px = self.board.px;
        let has_sections = sections.iter().any(|(name, _)| !name.is_empty());
        // Each column as wide as its widest words.
        let mut columns: Vec<(usize, &String, &Vec<String>)> = Vec::new();
        for (k, (_, periods)) in sections.iter().enumerate() {
            for (period, events) in periods {
                columns.push((k, period, events));
            }
        }
        let widths: Vec<f32> = columns.iter().map(|(_, period, events)| {
            let words = std::iter::once(*period).chain(events.iter()).map(|words| self.words_wide(words, px)).fold(0.0, f32::max);
            (words + 36.0).clamp(130.0, nest::MOST_WIDE)
        }).collect();
        let gap = 20.0;
        let (box_tall, event_gap) = (Text::line_height(px) as f32 + 24.0, 14.0);
        let section_top = origin.1;
        let period_top = section_top + if has_sections { box_tall + 16.0 } else { 0.0 };
        let line_y = period_top + box_tall + 30.0;
        let mut x = origin.0;
        let mut spans: Vec<(f32, f32)> = vec![(f32::MAX, f32::MIN); sections.len()];
        for (column, ((k, period, events), &wide)) in columns.iter().zip(&widths).enumerate() {
            // A color a section, or with none a color a period.
            let color = if has_sections { *k } else { column };
            let fill = SERIES[color % SERIES.len()].0;
            spans[*k] = (spans[*k].0.min(x), spans[*k].1.max(x + wide));
            self.chart_box(period, (x, period_top, wide, box_tall), fill);
            // Its events under the line, a dashed line down to them.
            let mut y = line_y + 30.0;
            let mut last_bottom = y;
            for event in events.iter() {
                let index = self.diagram_box(event, Form::Box, (x + wide / 2.0, y));
                let (_, _, _, tall) = self.box_frame(index);
                self.frame_to(index, (x + 8.0, y, wide - 16.0, tall));
                if let Some(shape) = self.shape_mut(index) {
                    (shape.fill, shape.filled) = (fill, true);
                }
                last_bottom = y + tall;
                y += tall + event_gap;
            }
            if !events.is_empty() {
                let rule = self.chart_rule((x + wide / 2.0, period_top + box_tall), (x + wide / 2.0, line_y + 30.0), QUIET, true);
                let _ = (rule, last_bottom);
            }
            x += wide + gap;
        }
        // The line through time, under the periods.
        let arrow = self.diagram_line((origin.0 - 10.0, line_y), (x + 10.0, line_y), true, false);
        if let Some(shape) = self.shape_mut(arrow) {
            shape.stroke = LINE * 1.5;
        }
        if has_sections {
            for (k, (name, _)) in sections.iter().enumerate() {
                let (from, to) = spans[k];
                if from < to {
                    self.chart_box(name, (from, section_top, to - from, box_tall), SERIES[k % SERIES.len()].0);
                }
            }
        }
        if !title.is_empty() {
            self.diagram_title(&title, (origin.0 + x - gap) / 2.0, origin.1 - 20.0);
        }
    }

    pub(super) fn build_journey(&mut self, text: &str, origin: Point) {
        let mut title = front_title(text).unwrap_or_default();
        let mut sections: Vec<(String, Vec<(String, u8, Vec<String>)>)> = Vec::new();
        let mut actors: Vec<String> = Vec::new();
        for line in body(text) {
            if let Some(rest) = after(&line, "title") {
                title = rest.to_owned();
                continue;
            }
            if let Some(rest) = after(&line, "section") {
                sections.push((rest.to_owned(), Vec::new()));
                continue;
            }
            let parts: Vec<&str> = line.split(':').map(str::trim).collect();
            let [task, score, who @ ..] = &parts[..] else { continue };
            let who: Vec<String> = who.iter().flat_map(|names| names.split(',')).map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()).collect();
            for name in &who {
                if !actors.contains(name) {
                    actors.push(name.clone());
                }
            }
            if sections.is_empty() {
                sections.push((String::new(), Vec::new()));
            }
            sections.last_mut().unwrap().1.push((plain(task), score.parse::<u8>().unwrap_or(3).clamp(1, 5), who));
        }
        let px = self.board.px;
        let small = 16;
        let tasks: Vec<(usize, &(String, u8, Vec<String>))> = sections.iter().enumerate().flat_map(|(k, (_, tasks))| tasks.iter().map(move |task| (k, task))).collect();
        let widths: Vec<f32> = tasks.iter().map(|(_, (words, ..))| (self.words_wide(words, px) + 36.0).clamp(130.0, 240.0)).collect();
        let gap = 16.0;
        // Its people on the left, a dot of a color each.
        let legend = actors.iter().map(|name| self.words_wide(name, small)).fold(0.0, f32::max) + if actors.is_empty() { 0.0 } else { 52.0 };
        let left = origin.0 + legend;
        let box_tall = Text::line_height(px) as f32 * 2.0 + 20.0;
        let (section_top, task_top) = (origin.1, origin.1 + Text::line_height(px) as f32 + 32.0);
        let faces_top = task_top + box_tall + 40.0;
        let face = 40.0;
        let step = face * 0.8;
        let actor_color = |name: &str| STRONG[actors.iter().position(|known| known == name).unwrap_or(0) % STRONG.len()];
        for (at, name) in actors.iter().enumerate() {
            let y = task_top + 12.0 + at as f32 * 28.0;
            self.chart_dot((origin.0 + 10.0, y), 8.0, actor_color(name));
            self.diagram_words(name, (origin.0 + 26.0, y - Text::line_height(small) as f32 / 2.0), small);
        }
        let mut x = left;
        let mut spans: Vec<(f32, f32)> = vec![(f32::MAX, f32::MIN); sections.len()];
        for ((k, (words, score, who)), &wide) in tasks.iter().zip(&widths) {
            let fill = SERIES[k % SERIES.len()].0;
            spans[*k] = (spans[*k].0.min(x), spans[*k].1.max(x + wide));
            self.chart_box(words, (x, task_top, wide, box_tall), fill);
            // Who did it, dots along its top.
            for (at, name) in who.iter().enumerate() {
                self.chart_dot((x + 14.0 + at as f32 * 18.0, task_top), 7.0, actor_color(name));
            }
            // A face as glad as its score, higher the gladder, a dashed
            // line down to it.
            let middle = (x + wide / 2.0, faces_top + face / 2.0 + f32::from(5 - score) * step);
            self.chart_rule((middle.0, task_top + box_tall), (middle.0, middle.1 - face / 2.0), QUIET, true);
            let index = self.diagram_box("", Form::Circle, middle);
            self.frame_to(index, (middle.0 - face / 2.0, middle.1 - face / 2.0, face, face));
            if let Some(shape) = self.shape_mut(index) {
                (shape.form, shape.votes, shape.fill, shape.filled) = (shapes::FORM_FACE, *score, FILLS[0], true);
            }
            x += wide + gap;
        }
        let floor = faces_top + face + 4.0 * step + 24.0;
        let arrow = self.diagram_line((left - 8.0, floor), (x, floor), true, false);
        let _ = arrow;
        for (k, (name, _)) in sections.iter().enumerate() {
            let (from, to) = spans[k];
            if from < to && !name.is_empty() {
                self.chart_box(name, (from, section_top, to - from, Text::line_height(px) as f32 + 16.0), SERIES[k % SERIES.len()].0);
            }
        }
        if !title.is_empty() {
            self.diagram_title(&title, (left + x - gap) / 2.0, origin.1 - 20.0);
        }
    }

    pub(super) fn build_mindmap(&mut self, text: &str, origin: Point) {
        let nodes = read_mindmap(text);
        if nodes.is_empty() {
            return;
        }
        // Each node's parent: the last before it one shallower.
        let mut parents: Vec<Option<usize>> = Vec::new();
        for (at, (depth, ..)) in nodes.iter().enumerate() {
            parents.push((0..at).rev().find(|&before| nodes[before].0 < *depth));
        }
        let children = |of: usize| -> Vec<usize> { (0..nodes.len()).filter(|&at| parents[at] == Some(of)).collect() };
        let mut made: Vec<usize> = Vec::new();
        let mut branch: Vec<usize> = vec![0; nodes.len()];
        for at in 0..nodes.len() {
            if parents[at] == Some(0) {
                branch[at] = children(0).iter().position(|&child| child == at).unwrap_or(0);
            } else if let Some(parent) = parents[at] {
                branch[at] = branch[parent];
            }
        }
        for (at, (depth, words, form)) in nodes.iter().enumerate() {
            let form = if at == 0 && *form == Form::Round { Form::Circle } else { *form };
            let index = self.diagram_box(words, form, origin);
            if let Some(shape) = self.shape_mut(index) {
                match *depth {
                    0 => (shape.fill, shape.filled) = (0xE5DBFF, true),
                    _ => (shape.fill, shape.filled) = (SERIES[branch[at] % SERIES.len()].0, true),
                }
            }
            made.push(index);
        }
        // Its branches either side of it, the larger halves balanced; each
        // side a tree out from it, each child in the middle of the room its
        // own children take.
        let size = |studio: &Studio, at: usize| {
            let (_, _, w, h) = studio.box_frame(made[at]);
            (w, h)
        };
        fn span(studio: &Studio, at: usize, children: &dyn Fn(usize) -> Vec<usize>, size: &dyn Fn(&Studio, usize) -> (f32, f32)) -> f32 {
            let kids = children(at);
            let own = size(studio, at).1;
            if kids.is_empty() {
                return own;
            }
            (kids.iter().map(|&kid| span(studio, kid, children, size)).sum::<f32>() + 18.0 * (kids.len() - 1) as f32).max(own)
        }
        let first: Vec<usize> = children(0);
        let spans: Vec<f32> = first.iter().map(|&kid| span(self, kid, &children, &size)).collect();
        let total: f32 = spans.iter().sum();
        let (mut right, mut left): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        let mut right_span = 0.0;
        for (k, &kid) in first.iter().enumerate() {
            if right_span < total / 2.0 || right.is_empty() {
                right.push(kid);
                right_span += spans[k];
            } else {
                left.push(kid);
            }
        }
        let root = size(self, 0);
        self.center_at(made[0], origin);
        let level = 70.0;
        #[allow(clippy::too_many_arguments)]
        fn place(studio: &mut Studio, at: usize, side: f32, near: f32, top: f32, made: &[usize], children: &dyn Fn(usize) -> Vec<usize>, size: &dyn Fn(&Studio, usize) -> (f32, f32)) {
            let (w, _) = size(studio, at);
            let room = span(studio, at, children, size);
            let middle = (near + side * w / 2.0, top + room / 2.0);
            studio.center_at(made[at], middle);
            let kids = children(at);
            let inner: f32 = kids.iter().map(|&kid| span(studio, kid, children, size)).sum::<f32>() + 18.0 * kids.len().saturating_sub(1) as f32;
            let mut y = middle.1 - inner / 2.0;
            for kid in kids {
                let room = span(studio, kid, children, size);
                place(studio, kid, side, near + side * (w + 60.0), y, made, children, size);
                y += room + 18.0;
            }
        }
        for (side, kids) in [(1.0, right), (-1.0, left)] {
            let rooms: Vec<f32> = kids.iter().map(|&kid| span(self, kid, &children, &size)).collect();
            let height = rooms.iter().sum::<f32>() + 18.0 * kids.len().saturating_sub(1) as f32;
            let mut y = origin.1 - height / 2.0;
            for (&kid, room) in kids.iter().zip(rooms) {
                place(self, kid, side, origin.0 + side * (root.0 / 2.0 + level), y, &made, &children, &size);
                y += room + 18.0;
            }
        }
        // Each joined to its parent by a line curving into its side, in its
        // branch's color, heavier nearer the root.
        for at in 1..nodes.len() {
            let Some(parent) = parents[at] else { continue };
            let line = self.join(made[parent], made[at]);
            if let Some(shape) = self.shape_mut(line) {
                (shape.tool, shape.heads) = (Tool::Line, [false, false]);
                shape.color = SERIES[branch[at] % SERIES.len()].1;
                shape.stroke = if nodes[at].0 == 1 { LINE * 1.75 } else { LINE * 1.25 };
                shape.touch();
            }
        }
    }

    pub(super) fn build_git(&mut self, text: &str, origin: Point) {
        let head = header(text);
        let down = head.contains("TB") || head.contains("BT");
        let mut branches: Vec<(String, i32)> = vec![("main".into(), 0)];
        let mut current = 0usize;
        let mut heads: Vec<Option<usize>> = vec![None];
        // Each commit: its lane, its parents, its id, its tag and its kind.
        let mut commits: Vec<(usize, Vec<usize>, String, String, String)> = Vec::new();
        let field = |line: &str, key: &str| -> Option<String> {
            let at = line.find(&format!("{key}:"))?;
            let rest = line[at + key.len() + 1..].trim_start();
            Some(match rest.strip_prefix('"') {
                Some(quoted) => quoted.split('"').next().unwrap_or("").to_owned(),
                None => rest.split_whitespace().next().unwrap_or("").to_owned(),
            })
        };
        for line in body(text) {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.first().copied() {
                Some("branch") => {
                    let name = words.get(1).copied().unwrap_or("branch").trim_matches('"').to_owned();
                    let order = field(&line, "order").and_then(|order| order.parse().ok()).unwrap_or(branches.len() as i32);
                    branches.push((name, order));
                    heads.push(heads[current]);
                    current = branches.len() - 1;
                }
                Some("checkout" | "switch") => {
                    if let Some(at) = branches.iter().position(|(name, _)| Some(name.as_str()) == words.get(1).map(|word| word.trim_matches('"'))) {
                        current = at;
                    }
                }
                Some(kind @ ("commit" | "merge" | "cherry-pick")) => {
                    let mut parents: Vec<usize> = heads[current].into_iter().collect();
                    if kind == "merge" {
                        if let Some(from) = branches.iter().position(|(name, _)| Some(name.as_str()) == words.get(1).map(|word| word.trim_matches('"'))).and_then(|at| heads[at]) {
                            parents.push(from);
                        }
                    }
                    if kind == "cherry-pick" {
                        if let Some(from) = field(&line, "id").and_then(|id| commits.iter().position(|commit| commit.2 == id)) {
                            parents.push(from);
                        }
                    }
                    let id = if kind == "cherry-pick" { String::new() } else { field(&line, "id").unwrap_or_default() };
                    let kind = if kind == "merge" { "MERGE".to_owned() } else if kind == "cherry-pick" { "PICK".to_owned() } else { field(&line, "type").unwrap_or_else(|| "NORMAL".into()) };
                    commits.push((current, parents, id, field(&line, "tag").unwrap_or_default(), kind));
                    heads[current] = Some(commits.len() - 1);
                }
                _ => {}
            }
        }
        // The lanes in their order, each a color.
        let mut order: Vec<usize> = (0..branches.len()).collect();
        order.sort_by_key(|&at| (branches[at].1, at));
        let lane_of = |branch: usize| order.iter().position(|&at| at == branch).unwrap_or(0) as f32;
        let px = self.board.px;
        let small = 16;
        let (lane, step, dot) = (80.0, 64.0, 14.0);
        let label_room = branches.iter().map(|(name, _)| self.words_wide(name, small)).fold(0.0, f32::max) + 40.0;
        let place = |k: usize, branch: usize| -> Point {
            let (along, across) = (origin.0 + label_room + 40.0 + k as f32 * step, origin.1 + lane_of(branch) * lane);
            if down { (origin.0 + lane_of(branch) * lane + label_room, origin.1 + 60.0 + k as f32 * step) } else { (along, across) }
        };
        let end_of = commits.len().max(1);
        // The lanes: a thin line along each in its color, its name at its
        // start on a tag of the same.
        for (at, (name, _)) in branches.iter().enumerate() {
            let color = SERIES[at % SERIES.len()];
            let start = place(0, at);
            let finish = place(end_of, at);
            let line = self.chart_rule(start, finish, color.1, true);
            if let Some(shape) = self.shape_mut(line) {
                shape.opacity = 160;
            }
            let wide = self.words_wide(name, small) + 20.0;
            let tall = Text::line_height(small) as f32 + 10.0;
            let frame = if down { (start.0 - wide / 2.0, origin.1 - tall / 2.0, wide, tall) } else { (origin.0, start.1 - tall / 2.0, wide, tall) };
            let index = self.chart_box(name, frame, color.0);
            if let Some(shape) = self.shape_mut(index) {
                (shape.font_px, shape.color) = (small, color.1);
            }
        }
        // The commits, each a dot on its lane in its color; a merge a dot
        // in a ring, a highlight a square, a reversal crossed.
        let mut dots: Vec<usize> = Vec::new();
        for (k, (branch, _, id, tag, kind)) in commits.iter().enumerate() {
            let at = place(k + 1, *branch);
            let color = SERIES[*branch % SERIES.len()];
            let index = match kind.as_str() {
                "HIGHLIGHT" => {
                    let index = self.chart_box("", (at.0 - dot, at.1 - dot, dot * 2.0, dot * 2.0), color.1);
                    if let Some(shape) = self.shape_mut(index) {
                        shape.color = color.1;
                    }
                    index
                }
                _ => {
                    let index = self.chart_dot(at, dot, color.1);
                    if let Some(shape) = self.shape_mut(index) {
                        shape.color = color.1;
                        match kind.as_str() {
                            "MERGE" => (shape.form, shape.fill, shape.color) = (shapes::FORM_DOUBLE, color.1, INK),
                            "REVERSE" => shape.text = "\u{d7}".into(),
                            "PICK" => (shape.fill, shape.dashed) = (color.0, true),
                            _ => {}
                        }
                        if kind == "REVERSE" {
                            (shape.font_px, shape.fill, shape.color) = (small, color.0, color.1);
                        }
                    }
                    index
                }
            };
            dots.push(index);
            if !id.is_empty() {
                let y = if down { at.1 } else { at.1 + dot + 6.0 + Text::line_height(small) as f32 / 2.0 };
                let x = if down { at.0 + dot + 10.0 + self.words_wide(id, small) / 2.0 } else { at.0 };
                self.words_at(id, (x, y), small);
            }
            if !tag.is_empty() {
                let wide = self.words_wide(tag, small) + 16.0;
                let tall = Text::line_height(small) as f32 + 8.0;
                let frame = if down { (at.0 - dot - 12.0 - wide, at.1 - tall / 2.0, wide, tall) } else { (at.0 - wide / 2.0, at.1 - dot - 10.0 - tall, wide, tall) };
                let index = self.chart_box(tag, frame, 0xFFFFFF);
                if let Some(shape) = self.shape_mut(index) {
                    (shape.font_px, shape.color, shape.sketch) = (small, QUIET, false);
                }
            }
        }
        let _ = px;
        // Each joined to its parents: along its lane, or round a corner
        // from the lane it branched off or onto the one it merges into.
        for (k, (branch, parents, ..)) in commits.iter().enumerate() {
            for (n, &parent) in parents.iter().enumerate() {
                let line = self.join(dots[parent], dots[k]);
                let color = SERIES[if n == 0 { *branch } else { commits[parent].0 } % SERIES.len()].1;
                let (from, to) = (place(parent + 1, commits[parent].0), place(k + 1, *branch));
                if let Some(shape) = self.shape_mut(line) {
                    (shape.tool, shape.heads, shape.routed, shape.color, shape.stroke) = (Tool::Line, [false, false], false, color, LINE * 1.5);
                    if commits[parent].0 != *branch {
                        // Straight across the lanes from where it leaves
                        // and round a corner onto the lane of the next, as
                        // a branch does; along its own lane and round a
                        // corner into the one it merges into, as a merge.
                        let r = 18.0;
                        let toward = |a: f32, b: f32| (b - a).signum();
                        let corner = match (n == 0, down) {
                            (true, false) => vec![(from.0, to.1 - r * toward(from.1, to.1)), (from.0 + r, to.1)],
                            (true, true) => vec![(to.0 - r * toward(from.0, to.0), from.1), (to.0, from.1 + r)],
                            (false, false) => vec![(to.0 - r, from.1), (to.0, from.1 + r * toward(from.1, to.1))],
                            (false, true) => vec![(from.0, to.1 - r), (from.0 + r * toward(from.0, to.0), to.1)],
                        };
                        shape.points.truncate(2);
                        shape.points.extend(corner);
                    }
                    shape.touch();
                }
            }
        }
    }

    pub(super) fn build_quadrant(&mut self, text: &str, origin: Point) {
        let mut title = front_title(text).unwrap_or_default();
        let mut names = [String::new(), String::new(), String::new(), String::new()];
        let mut axes = [String::new(), String::new(), String::new(), String::new()];
        let mut points: Vec<(String, f32, f32, Option<u32>, Option<f32>)> = Vec::new();
        let mut classes: Vec<(String, String)> = Vec::new();
        for line in body(text) {
            if let Some(rest) = after(&line, "title") {
                title = rest.to_owned();
            } else if let Some(rest) = after(&line, "x-axis") {
                let (low, high) = rest.split_once("-->").unwrap_or((rest, ""));
                (axes[0], axes[1]) = (plain(low), plain(high));
            } else if let Some(rest) = after(&line, "y-axis") {
                let (low, high) = rest.split_once("-->").unwrap_or((rest, ""));
                (axes[2], axes[3]) = (plain(low), plain(high));
            } else if let Some((which, rest)) = line.strip_prefix("quadrant-").and_then(|rest| rest.split_once(' ')) {
                if let Ok(at) = which.parse::<usize>() {
                    names[at.clamp(1, 4) - 1] = plain(rest);
                }
            } else if let Some(rest) = after(&line, "classDef") {
                let (name, style) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                classes.push((name.to_owned(), style.to_owned()));
            } else if let Some((name, at)) = line.split_once(':') {
                let (name, class) = name.split_once(":::").map_or((name, None), |(name, class)| (name, Some(class.trim())));
                let inner = at.split(']').next().unwrap_or("").trim().trim_start_matches('[');
                let numbers: Vec<f32> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                let style = class.and_then(|class| classes.iter().find(|(known, _)| known == class)).map_or(String::new(), |(_, style)| style.clone()) + "," + at.split(']').nth(1).unwrap_or("");
                let color = style.split(',').find_map(|part| part.split_once(':').filter(|(key, _)| key.trim() == "color").and_then(|(_, value)| css_color(value)));
                let radius = style.split(',').find_map(|part| part.split_once(':').filter(|(key, _)| key.trim() == "radius").and_then(|(_, value)| value.trim().parse().ok()));
                if let [px, py] = numbers[..] {
                    points.push((plain(name), px.clamp(0.0, 1.0), py.clamp(0.0, 1.0), color.map(board_stroke), radius));
                }
            }
        }
        let side = 500.0;
        let (left, top) = (origin.0 - side / 2.0, origin.1 - side / 2.0);
        let px = self.board.px;
        // Mermaid's quadrants: 1 top right, 2 top left, 3 bottom left, 4
        // bottom right; each a shade of one color, its name at its top.
        let fills = [0xD0EBFF, 0xE7F5FF, 0xF3F9FF, 0xE7F5FF];
        for (at, (dx, dy)) in [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)].iter().enumerate() {
            let frame = (left + dx * side / 2.0, top + dy * side / 2.0, side / 2.0, side / 2.0);
            let index = self.chart_box("", frame, fills[at]);
            if let Some(shape) = self.shape_mut(index) {
                (shape.color, shape.sketch) = (QUIET, false);
            }
            if !names[at].is_empty() {
                self.words_at(&names[at], (frame.0 + frame.2 / 2.0, frame.1 + 12.0 + Text::line_height(px) as f32 / 2.0), px);
            }
        }
        let small = 16;
        let tall = Text::line_height(small) as f32;
        // The x axis's ends under it, the y axis's beside it, turned.
        if !axes[0].is_empty() {
            self.words_at(&axes[0], (left + side / 4.0, top + side + 10.0 + tall / 2.0), small);
        }
        if !axes[1].is_empty() {
            self.words_at(&axes[1], (left + side * 0.75, top + side + 10.0 + tall / 2.0), small);
        }
        for (words, y) in [(&axes[2], top + side * 0.75), (&axes[3], top + side / 4.0)] {
            if words.is_empty() {
                continue;
            }
            let index = self.words_at(words, (left - 12.0 - tall / 2.0, y), small);
            if let Some(shape) = self.shape_mut(index) {
                shape.angle = -std::f32::consts::FRAC_PI_2;
            }
        }
        // Its points, each a dot with its name under it.
        for (name, x, y, color, radius) in points {
            let at = (left + x * side, top + (1.0 - y) * side);
            let radius = radius.unwrap_or(8.0).clamp(3.0, 30.0);
            self.chart_dot(at, radius, color.unwrap_or(0x1971C2));
            if let Some(shape) = self.doc.layers.last_mut().and_then(|layer| match &mut layer.kind {
                Kind::Shape(shape) => Some(shape),
                Kind::Picture(_) => None,
            }) {
                (shape.color, shape.sketch) = (color.unwrap_or(0x1971C2), false);
            }
            self.words_at(&name, (at.0, at.1 + radius + 4.0 + tall / 2.0), small);
        }
        if !title.is_empty() {
            self.diagram_title(&title, origin.0, top - 16.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::check::*;
    use super::super::tests::shapes_of;
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn dates_go_to_days_and_back_and_are_written_as_asked() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(read_date("2014-01-06", "YYYY-MM-DD"), Some(days_from_civil(2014, 1, 6) as f64));
        assert_eq!(read_date("06/01/2014", "DD/MM/YYYY"), Some(days_from_civil(2014, 1, 6) as f64));
        assert_eq!(read_length("2w"), Some(14.0));
        assert_eq!(write_date(days_from_civil(2014, 1, 6) as f64, "%b %e, %a"), "Jan 6, Mon");
        assert!(weekend(days_from_civil(2024, 6, 1) as f64) && !weekend(days_from_civil(2024, 6, 3) as f64));
    }

    /// Mermaid's own gantt example.
    const GANTT: &str = "gantt\n    dateFormat  YYYY-MM-DD\n    title       Adding GANTT diagram functionality to mermaid\n    excludes    weekends\n    section A section\n    Completed task            :done,    des1, 2014-01-06,2014-01-08\n    Active task               :active,  des2, 2014-01-09, 3d\n    Future task               :         des3, after des2, 5d\n    Future task2              :         des4, after des3, 5d\n    section Critical tasks\n    Completed task in the critical line :crit, done, 2014-01-06,24h\n    Implement parser and jison          :crit, done, after des1, 2d\n    Create tests for parser             :crit, active, 3d\n    Future task in critical line        :crit, 5d\n    Create tests for renderer           :2d\n    Add to mermaid                      :until isadded\n    Functionality added                 :milestone, isadded, 2014-01-25, 0d\n";

    #[test]
    fn a_gantt_chart_is_bars_along_its_dates_in_bands_by_section() {
        let gantt = read_gantt(GANTT);
        assert_eq!(gantt.tasks.len(), 11);
        let day = |y, m, d| days_from_civil(y, m, d) as f64;
        let task = |name: &str| gantt.tasks.iter().find(|task| task.words == name).unwrap().clone();
        assert_eq!((task("Completed task").start, task("Completed task").end), (day(2014, 1, 6), day(2014, 1, 8)));
        assert_eq!(task("Future task").start, task("Active task").end);
        // Five working days from a Wednesday end the next Wednesday.
        assert!(task("Future task").end - task("Future task").start >= 7.0);
        assert!(task("Functionality added").milestone && task("Functionality added").start == day(2014, 1, 25));
        assert_eq!(task("Add to mermaid").end, day(2014, 1, 25));
        let mut studio = board();
        assert!(studio.bring_mermaid(GANTT));
        let shapes = shapes_of(&studio);
        assert!(shapes.iter().any(|shape| shape.text == "Adding GANTT diagram functionality to mermaid"));
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_DIAMOND), "a milestone");
        // Later tasks further right, a row each further down.
        let x = |name: &str| shapes.iter().filter(|shape| shape.tool == Tool::Rect && shape.frame().2 > 4.0).find(|shape| shape.text == name).map(|shape| shape.frame());
        let (done, active) = (x("Completed task"), x("Active task"));
        if let (Some(done), Some(active)) = (done, active) {
            assert!(done.0 < active.0 && done.1 < active.1);
        }
        assert_labels_clear_of_bars(&mut studio);
    }

    /// No words over another's, nor over a bar but their own.
    fn assert_labels_clear_of_bars(studio: &mut Studio) {
        let all = placed(studio);
        let words: Vec<&Placed> = all.iter().filter(|thing| thing.label).collect();
        for (at, a) in words.iter().enumerate() {
            for b in &words[at + 1..] {
                assert!(overlap(a.frame, b.frame).is_none(), "{:?} {:?} over {:?} {:?}", a.words, a.frame, b.words, b.frame);
            }
        }
    }

    #[test]
    fn a_pie_is_slices_of_the_board_each_its_share_with_its_key() {
        let mut studio = board();
        assert!(studio.bring_mermaid("pie showData title Key elements in Product X\n    \"Calcium\" : 42.96\n    \"Potassium\" : 50.05\n    \"Magnesium\" : 10.01\n    \"Iron\" :  5\n"));
        let shapes = shapes_of(&studio);
        assert!(studio.doc.layers.iter().all(|layer| matches!(layer.kind, Kind::Shape(_))), "no picture");
        let slices: Vec<&Shape> = shapes.iter().filter(|shape| shape.form == shapes::FORM_WEDGE).collect();
        assert_eq!(slices.len(), 4);
        let sweep: f32 = slices.iter().map(|shape| shape.corners.unwrap()[1]).sum();
        assert!((sweep - 360.0).abs() < 0.1);
        assert!(shapes.iter().any(|shape| shape.text == "Calcium [42.96]"));
        assert!(shapes.iter().any(|shape| shape.text == "46%"));
        assert!(shapes.iter().any(|shape| shape.text == "Key elements in Product X"));
        assert_labels_clear_of_bars(&mut studio);
    }

    #[test]
    fn a_timeline_is_its_periods_along_a_line_and_their_events_under_them() {
        let text = "timeline\n    title History of Social Media Platform\n    section Early\n      2002 : LinkedIn\n      2004 : Facebook : Google\n    section Later\n      2005 : YouTube\n      2006 : Twitter\n           : Spotify\n";
        let mut studio = board();
        assert!(studio.bring_mermaid(text));
        let shapes = shapes_of(&studio);
        let frame = |words: &str| shapes.iter().find(|shape| shape.text == words && shape.tool == Tool::Rect).unwrap().frame();
        assert!(frame("2002").0 < frame("2004").0 && frame("2004").0 < frame("2005").0);
        assert!(frame("Facebook").1 > frame("2004").1 && frame("Google").1 > frame("Facebook").1);
        assert!((frame("Facebook").0 + frame("Facebook").2 / 2.0 - (frame("2004").0 + frame("2004").2 / 2.0)).abs() < 1.0, "under its period");
        assert!(frame("Spotify").1 > frame("Twitter").1, "a line of its own goes on the period before");
        assert!(frame("Early").0 <= frame("2002").0 && frame("Early").0 + frame("Early").2 >= frame("2004").0 + frame("2004").2, "a section over its periods");
        assert_clear(&mut studio);
    }

    #[test]
    fn a_journey_is_tasks_in_sections_with_faces_as_glad_as_their_scores() {
        let text = "journey\n    title My working day\n    section Go to work\n      Make tea: 5: Me\n      Go upstairs: 3: Me\n      Do work: 1: Me, Cat\n    section Go home\n      Go downstairs: 5: Me\n      Sit down: 5: Me\n";
        let mut studio = board();
        assert!(studio.bring_mermaid(text));
        let shapes = shapes_of(&studio);
        let faces: Vec<&Shape> = shapes.iter().filter(|shape| shape.form == shapes::FORM_FACE).collect();
        assert_eq!(faces.len(), 5);
        let glad = faces.iter().find(|shape| shape.votes == 5).unwrap().frame().1;
        let sad = faces.iter().find(|shape| shape.votes == 1).unwrap().frame().1;
        assert!(glad < sad, "the gladder higher");
        assert!(shapes.iter().any(|shape| shape.text == "Cat" && shape.tool == Tool::Text), "who, in the key");
        assert!(shapes.iter().any(|shape| shape.text == "Go to work" && shape.tool == Tool::Rect));
        assert_labels_clear_of_bars(&mut studio);
    }

    #[test]
    fn a_mind_map_goes_out_either_side_of_its_root_in_its_shapes() {
        let text = "mindmap\n  root((mindmap))\n    Origins\n      Long history\n      ::icon(fa fa-book)\n      Popularisation\n        British popular psychology author Tony Buzan\n    Research\n      On effectiveness<br/>and features\n      On Automatic creation\n        Uses\n            Creative techniques\n            Strategic planning\n            Argument mapping\n    Tools\n      Pen and paper\n      Mermaid\n    id)I am a cloud(\n    id2{{Hexagon}}\n    id3[Square]\n";
        let nodes = read_mindmap(text);
        assert_eq!(nodes[0], (0, "mindmap".to_owned(), Form::Circle));
        assert!(nodes.iter().any(|node| node.1 == "Hexagon" && node.2 == Form::Hexagon));
        assert!(nodes.iter().any(|node| node.1 == "Square" && node.2 == Form::Box));
        assert!(nodes.iter().any(|node| node.1 == "On effectiveness\nand features"));
        let mut studio = board();
        assert!(studio.bring_mermaid(text));
        let shapes = shapes_of(&studio);
        let middle = |words: &str| {
            let (x, _, w, _) = shapes.iter().find(|shape| shape.text == words && shape.tool != Tool::Line).unwrap().frame();
            x + w / 2.0
        };
        let root = middle("mindmap");
        assert!((middle("Origins") - root).signum() != (middle("Tools") - root).signum() || (middle("Research") - root).signum() != (middle("Tools") - root).signum(), "both sides");
        assert!((middle("Long history") - root).abs() > (middle("Origins") - root).abs(), "children further out");
        assert_clear(&mut studio);
        // Still a map: written back as one.
        let at = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.text == "mindmap")).unwrap();
        studio.selected = vec![at];
        assert!(studio.to_mermaid(true).contains("Origins"));
    }

    #[test]
    fn a_git_graph_is_lanes_of_commits_branching_and_merging() {
        let text = "gitGraph\n    commit id: \"init\"\n    commit tag: \"v1.0\"\n    branch develop\n    checkout develop\n    commit id: \"feat\"\n    commit type: HIGHLIGHT\n    checkout main\n    commit type: REVERSE\n    merge develop tag: \"v1.1\"\n    branch hotfix order: 3\n    commit\n    checkout main\n    cherry-pick id: \"feat\"\n";
        let mut studio = board();
        assert!(studio.bring_mermaid(text));
        let shapes = shapes_of(&studio);
        let lanes: Vec<f32> = ["main", "develop", "hotfix"].iter().map(|name| shapes.iter().find(|shape| shape.text == *name).unwrap().frame().1).collect();
        assert!(lanes[0] < lanes[1] && lanes[1] < lanes[2]);
        assert!(shapes.iter().any(|shape| shape.form == shapes::FORM_DOUBLE), "a merge");
        assert!(shapes.iter().any(|shape| shape.text == "v1.1"), "its tag");
        assert!(shapes.iter().any(|shape| shape.text == "feat" && shape.tool == Tool::Text));
        // A line from develop's last into the merge, round a corner.
        assert!(shapes.iter().any(|shape| shape.tool == Tool::Line && shape.points.len() == 4));
        assert_labels_clear_of_bars(&mut studio);
    }

    #[test]
    fn a_quadrant_chart_is_four_quadrants_its_axes_and_its_points() {
        let text = "quadrantChart\n    title Reach and engagement of campaigns\n    x-axis Low Reach --> High Reach\n    y-axis Low Engagement --> High Engagement\n    quadrant-1 We should expand\n    quadrant-2 Need to promote\n    quadrant-3 Re-evaluate\n    quadrant-4 May be improved\n    Campaign A: [0.3, 0.6]\n    Campaign B: [0.45, 0.23]\n    Campaign C: [0.57, 0.69] radius: 12, color: #ff3300\n    Campaign D: [0.78, 0.34]\n";
        let mut studio = board();
        assert!(studio.bring_mermaid(text));
        let shapes = shapes_of(&studio);
        let at = |words: &str| shapes.iter().find(|shape| shape.text == words).unwrap().clone();
        assert!(at("We should expand").points[0].0 > at("Need to promote").points[0].0);
        assert!(at("Re-evaluate").points[0].1 > at("Need to promote").points[0].1);
        assert!(at("Low Engagement").angle != 0.0, "the y axis turned");
        let dot = shapes.iter().find(|shape| shape.tool == Tool::Ellipse && shape.color == STROKES[1]).unwrap();
        assert!((dot.frame().2 - 24.0).abs() < 0.1, "its own color and size");
        assert_labels_clear_of_bars(&mut studio);
    }
}
