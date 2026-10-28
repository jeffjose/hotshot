//! Ctrl+K: one box to find anything by typing: what can be done, the
//! layers of the frame, the pages and frames of the file, and the other
//! files of the projects; Up and Down to choose, Enter to do it, Esc to
//! close.

use super::*;
use menu::MenuAct;
use std::path::PathBuf;

/// The search being typed, which of its tabs is shown, and the row
/// chosen.
#[derive(Clone, Debug, Default)]
pub(super) struct Search {
    pub typed: String,
    pub tab: u8,
    pub row: usize,
}

/// The tabs, by what each shows.
pub(super) const TABS: [&str; 5] = ["All", "Actions", "Layers", "Pages", "Files"];

/// Something found, and what choosing it does.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum Found {
    Act(MenuAct),
    /// A tool, by where it is among `FOUND_TOOLS`.
    Tool(usize),
    Layer(usize),
    /// One of the page's own layers, while a frame is worked in.
    PageLayer(usize),
    /// A stack, of the frame worked in or of the page, by what it is
    /// called.
    Stack(u32),
    Page(usize),
    Frame(usize),
    File(PathBuf),
}

/// A row of what is found: under which heading, what it says, and a
/// word or two after it.
#[derive(Clone, Debug)]
pub(super) struct Hit {
    pub heading: &'static str,
    pub says: String,
    pub hint: String,
    pub found: Found,
}

/// How wide the box is, how high a row of it, and how many rows show.
const WIDE: i32 = 620;
const ROW: i32 = 28;
const SHOWN: usize = 12;

/// The tools, found by name or by another word for them: those of the
/// bar, as `TOOLS` has them, then the emoji, the frame and the crop, each
/// with its key.
const FOUND_TOOLS: [(&str, &str, &str); 16] = [
    ("Move", "V", "select pointer"),
    ("Rectangle", "R", "rect box square"),
    ("Oval", "O", "ellipse circle"),
    ("Line", "L", ""),
    ("Arrow", "A", ""),
    ("Pen", "P", "draw pencil"),
    ("Marker", "M", "highlighter"),
    ("Text", "T", "words type"),
    ("Counter", "N", "number step"),
    ("Blur", "B", "mosaic pixelate redact"),
    ("Spotlight", "D", "dim focus"),
    ("Zoom lens", "Z", "magnifier loupe"),
    ("Library", "J", "label badge note"),
    ("Emoji", "E", "sticker"),
    ("Frame", "F", "artboard"),
    ("Crop", "Enter", "trim"),
];

/// Where in what is said what was typed is, ignoring case, if it is.
pub(super) fn matched(says: &str, typed: &str) -> Option<(usize, usize)> {
    if typed.is_empty() {
        return Some((0, 0));
    }
    let lower = says.to_lowercase();
    let at = lower.find(&typed.to_lowercase())?;
    // Back to where that is in what is said, letter by letter.
    let begin = says.char_indices().nth(lower[..at].chars().count()).map_or(says.len(), |(index, _)| index);
    let end = says.char_indices().nth(lower[..at].chars().count() + typed.chars().count()).map_or(says.len(), |(index, _)| index);
    Some((begin, end))
}

/// The files of the projects: those in the library and in each folder of
/// it, the latest first.
fn project_files() -> Vec<PathBuf> {
    let library = crate::home::library();
    let mut dirs = vec![library.clone()];
    dirs.extend(std::fs::read_dir(&library).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.is_dir()));
    let mut files: Vec<(PathBuf, std::time::SystemTime)> = dirs
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ending| ending == "hotshot"))
        .map(|path| {
            let when = std::fs::metadata(&path).and_then(|meta| meta.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            (path, when)
        })
        .collect();
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.into_iter().map(|(path, _)| path).collect()
}

impl Studio {
    pub(super) fn open_palette(&mut self) {
        // Over everything else that was open, and what was typed kept.
        self.commit_typing();
        self.commit_name();
        self.commit_text();
        (self.menu, self.flyout, self.palette, self.styling, self.eyedropping) = (None, None, None, false, false);
        self.search = Some(Search::default());
        self.dirty = true;
    }

    /// Everything that can be found, before what was typed narrows it.
    fn everything(&mut self) -> Vec<Hit> {
        let mut hits: Vec<Hit> = FOUND_TOOLS.iter().enumerate().map(|(at, (says, key, _))| Hit { heading: "TOOLS", says: (*says).to_owned(), hint: (*key).to_owned(), found: Found::Tool(at) }).collect();
        for (says, keys, act) in self.acts() {
            hits.push(Hit { heading: "ACTIONS", says, hint: keys.into_owned(), found: Found::Act(act) });
        }
        self.found_besides(hits)
    }

    /// What can be done now, each with what it says, its keys, and what
    /// doing it is: what the menus have, the windows and styles for what is
    /// picked out, and what the panel's buttons do to it.
    pub(super) fn acts(&mut self) -> Vec<(String, std::borrow::Cow<'static, str>, MenuAct)> {
        let mut acts: Vec<(String, std::borrow::Cow<'static, str>, MenuAct)> = Vec::new();
        let gather = |items: Vec<menu::Item>, acts: &mut Vec<(String, std::borrow::Cow<'static, str>, MenuAct)>| {
            for item in items {
                if let menu::Item::Entry { says, keys, act, sub: None, .. } = item {
                    if !matches!(act, MenuAct::Nothing | MenuAct::Do(menu::Command::Palette)) && !acts.iter().any(|(known, ..)| *known == says) {
                        acts.push((says, keys, act));
                    }
                }
            }
        };
        for kind in [MenuKind::Context, MenuKind::Order, MenuKind::MainSub(0), MenuKind::MainSub(1), MenuKind::MainSub(2), MenuKind::MainSub(3), MenuKind::MainSub(4), MenuKind::Zoom] {
            let items = self.menu_items(kind);
            gather(items, &mut acts);
        }
        // The windows, for a picture picked out: "mac", "windows" or
        // "gnome" typed put that one round it.
        if self.chosen().iter().any(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_))) {
            for (at, style) in chrome::CHOICES.iter().enumerate() {
                acts.push((format!("Window: {}", style.long()), "".into(), MenuAct::Window(at as u8)));
            }
        }
        if self.styles_apply() {
            for (at, name) in style::STYLE_NAMES.iter().enumerate() {
                acts.push((format!("Style: {name}"), "".into(), MenuAct::Style(at as u8)));
            }
        }
        for (says, keys, act) in self.named_actions() {
            if !acts.iter().any(|(known, ..)| *known == says) {
                acts.push((says, keys.into(), act));
            }
        }
        acts
    }

    /// The layers, pages, frames and files found, after what is found
    /// already.
    fn found_besides(&mut self, mut hits: Vec<Hit>) -> Vec<Hit> {
        // The layers worked in, and the page's own, outside the frames,
        // said to be the page's.
        let where_ = if self.on_page() { self.pages[self.page].name.clone() } else { self.doc.name.clone() };
        for index in (0..self.doc.layers.len()).rev() {
            hits.push(Hit { heading: "LAYERS", says: self.doc.layers[index].name(), hint: where_.clone(), found: Found::Layer(index) });
        }
        if !self.on_page() {
            for index in (0..self.loose().layers.len()).rev() {
                hits.push(Hit { heading: "LAYERS", says: self.loose().layers[index].name(), hint: self.pages[self.page].name.clone(), found: Found::PageLayer(index) });
            }
        }
        // The stacks, as the list of layers has them, each said to be one.
        for stack in &self.doc.stacks {
            hits.push(Hit { heading: "LAYERS", says: stack.title(), hint: format!("auto layout \u{b7} {where_}"), found: Found::Stack(stack.id) });
        }
        if !self.on_page() {
            for stack in &self.loose().stacks {
                hits.push(Hit { heading: "LAYERS", says: stack.title(), hint: format!("auto layout \u{b7} {}", self.pages[self.page].name), found: Found::Stack(stack.id) });
            }
        }
        for (index, page) in self.pages.iter().enumerate() {
            hits.push(Hit { heading: "PAGES AND FRAMES", says: page.name.clone(), hint: "page".into(), found: Found::Page(index) });
        }
        for index in 0..self.frame_count() {
            let frame = self.frame(index);
            hits.push(Hit { heading: "PAGES AND FRAMES", says: frame.name.clone(), hint: format!("{} \u{b7} {} \u{d7} {}", self.pages[self.page].name, frame.width, frame.height), found: Found::Frame(index) });
        }
        for path in project_files() {
            if Some(&path) == self.path.as_ref() {
                continue;
            }
            let name = path.file_stem().map_or(String::new(), |stem| stem.to_string_lossy().into_owned());
            let place = path.parent().and_then(|dir| dir.file_name()).map_or(String::new(), |dir| dir.to_string_lossy().into_owned());
            hits.push(Hit { heading: "FILES", says: name, hint: place, found: Found::File(path) });
        }
        hits
    }

    /// What is found for what was typed, in the tab shown.
    pub(super) fn hits(&mut self) -> Vec<Hit> {
        let Some(search) = self.search.clone() else { return Vec::new() };
        let wanted = |hit: &Hit| match search.tab {
            1 => matches!(hit.found, Found::Act(_) | Found::Tool(_)),
            2 => matches!(hit.found, Found::Layer(_) | Found::PageLayer(_) | Found::Stack(_)),
            3 => matches!(hit.found, Found::Page(_) | Found::Frame(_)),
            4 => matches!(hit.found, Found::File(_)),
            _ => true,
        };
        let typed = search.typed.trim().to_lowercase();
        // A tool is found by another word for it too, as "box" finds the
        // rectangle; only once something is typed.
        let also = |hit: &Hit| match hit.found {
            Found::Tool(at) => !typed.is_empty() && FOUND_TOOLS[at].2.split(' ').any(|word| !word.is_empty() && word.starts_with(&typed)),
            _ => false,
        };
        // Nothing typed: what was done lately and most first, and what can
        // be done listed under them once only.
        let remembered = if typed.is_empty() && search.tab <= 1 { self.remembered_hits() } else { Vec::new() };
        let mut hits: Vec<Hit> = self.everything().into_iter().filter(|hit| wanted(hit) && (matched(&hit.says, search.typed.trim()).is_some() || also(hit))).filter(|hit| !remembered.iter().any(|known| known.says == hit.says)).collect();
        // The tools first, then what can be done, then the rest; within
        // each, what was used lately and often, then what begins with what
        // was typed, each heading kept.
        let rank = |heading: &str| match heading {
            "TOOLS" => 0,
            "ACTIONS" => 1,
            _ => 2,
        };
        let used: Vec<f32> = hits.iter().map(|hit| if matches!(hit.found, Found::Act(_)) { self.used(&hit.says) } else { 0.0 }).collect();
        let mut order: Vec<usize> = (0..hits.len()).collect();
        order.sort_by(|&a, &b| {
            let key = |at: usize| (rank(hits[at].heading), hits[at].heading, std::cmp::Reverse((used[at] * 1000.0) as i64), !hits[at].says.to_lowercase().starts_with(&typed));
            key(a).cmp(&key(b))
        });
        let mut sorted: Vec<Hit> = remembered;
        sorted.extend(order.into_iter().map(|at| hits[at].clone()));
        hits.clear();
        sorted
    }

    /// A key while the search is open: typed into it, moved through it,
    /// done, or closed.
    pub(super) fn search_key(&mut self, key: Key, mods: Mods) -> Outcome {
        let count = self.hits().len();
        let Some(search) = &mut self.search else { return Outcome::Continue };
        self.dirty = true;
        match key {
            Key::Escape => self.search = None,
            Key::Up => search.row = search.row.saturating_sub(1),
            Key::Down => search.row = (search.row + 1).min(count.saturating_sub(1)),
            Key::Char('\t') => {
                search.tab = (search.tab + if mods.shift { TABS.len() as u8 - 1 } else { 1 }) % TABS.len() as u8;
                search.row = 0;
            }
            Key::Backspace => {
                search.typed.pop();
                search.row = 0;
            }
            Key::Char('k') if mods.ctrl => self.search = None,
            Key::Char(letter) if !mods.ctrl && !letter.is_control() => {
                search.typed.push(letter);
                search.row = 0;
            }
            Key::Enter => {
                let row = search.row;
                return self.choose_hit(row);
            }
            _ => {}
        }
        Outcome::Continue
    }

    /// A tool found taken up, as its key would: the library opened for
    /// it, or its emoji; the frame tool; a crop begun of the picture
    /// picked out.
    fn take_found_tool(&mut self, at: usize) {
        match FOUND_TOOLS[at].0 {
            "Emoji" => self.open_library(Some(crate::editor::EMOJI_SHELF)),
            "Frame" => {
                if !self.framing {
                    self.press_control(Control::FrameTool, false);
                }
            }
            "Crop" => match self.picked().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_))) {
                Some(index) => {
                    self.set_tool(Tool::Select);
                    (self.selected, self.cropping) = (vec![index], Some(index));
                }
                None => self.say("Pick a picture to crop it"),
            },
            _ => match TOOLS.get(at) {
                Some(&(Tool::Badge, ..)) => self.open_library(None),
                Some(&(tool, ..)) => {
                    // A tool of a group shows on the bar as its group's.
                    match tool {
                        Tool::Rect | Tool::Ellipse | Tool::Line => self.groups[0] = tool,
                        Tool::Pen | Tool::Marker => self.groups[1] = tool,
                        _ => {}
                    }
                    self.set_tool(tool);
                }
                None => {}
            },
        }
    }

    /// A row of what is found chosen: done, and the search closed.
    pub(super) fn choose_hit(&mut self, row: usize) -> Outcome {
        let Some(hit) = self.hits().into_iter().nth(row) else { return Outcome::Continue };
        self.search = None;
        self.dirty = true;
        match hit.found {
            Found::Act(act) => return self.run_menu(act),
            Found::Tool(at) => self.take_found_tool(at),
            Found::PageLayer(index) => {
                self.activate(LOOSE);
                self.selected = vec![index];
                self.zoom_to_selection();
            }
            Found::Layer(index) => {
                self.selected = vec![index];
                self.zoom_to_selection();
            }
            Found::Stack(id) => {
                self.pick_stack(id);
                self.zoom_to_selection();
            }
            Found::Page(page) => self.go_to_page(page),
            Found::Frame(frame) => {
                self.activate(frame);
                self.selected.clear();
                self.fit();
            }
            Found::File(path) => {
                self.open_next = Some(path);
                return Outcome::Open;
            }
        }
        Outcome::Continue
    }

    /// The box, over the middle of the top of the page: what is typed,
    /// the tabs, and as many rows of what is found as fit.
    pub(super) fn layout_search(&mut self, layout: &mut Layout) {
        let Some(search) = self.search.clone() else { return };
        let hits = self.hits();
        let area = self.open_area();
        let start = search.row.saturating_sub(SHOWN - 1);
        let shown = &hits[start.min(hits.len())..(start + SHOWN).min(hits.len())];
        let headings = shown.iter().enumerate().filter(|(at, hit)| *at == 0 || shown[at - 1].heading != hit.heading).count() as i32;
        let high = 52 + 38 + shown.len() as i32 * ROW + headings * 26 + 34;
        let place = Rect::new(area.x + (area.w - WIDE) / 2, area.y + 24, WIDE, high.max(160));
        let mut front = Vec::new();
        for (at, tab) in TABS.iter().enumerate() {
            let _ = tab;
            front.push((Rect::new(place.x + 12 + at as i32 * 76, place.y + 58, 70, 24), Control::SearchTab(at as u8)));
        }
        let mut y = place.y + 92;
        for (at, hit) in shown.iter().enumerate() {
            if at == 0 || shown[at - 1].heading != hit.heading {
                y += 26;
            }
            front.push((Rect::new(place.x + 6, y, WIDE - 12, ROW - 2), Control::Found((start + at) as u16)));
            y += ROW;
        }
        front.push((place, Control::Panel));
        front.append(&mut layout.controls);
        layout.controls = front;
        layout.search = Some(place);
    }

    /// Everything of the box but its rows, which are drawn as controls.
    pub(super) fn draw_search(&mut self, canvas: &mut Canvas, place: Rect, hover: Option<Control>) {
        let Some(search) = self.search.clone() else { return };
        let middle = place.y + 26;
        self.text.draw_centered(canvas, place.x + 24, middle, 16, DIM_TEXT_COLOR, icons::SEARCH);
        let typed = if search.typed.is_empty() { "Search tools, actions, layers, pages and files".to_owned() } else { search.typed.clone() };
        self.text.draw(canvas, place.x + 44, middle - Text::line_height(15) / 2, 15, if search.typed.is_empty() { 0x777777 } else { TEXT_COLOR }, &typed);
        if !search.typed.is_empty() {
            let at = place.x + 44 + self.text.width(&search.typed, 15) + 2;
            canvas.fill(at, middle - 9, 2, 18, ACCENT);
        }
        let esc = "Esc";
        let esc_wide = self.text.width(esc, 11);
        self.text.draw(canvas, place.right() - 16 - esc_wide, middle - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, esc);
        canvas.fill(place.x + 1, place.y + 52, place.w - 2, 1, EDGE_COLOR);
        for (at, tab) in TABS.iter().enumerate() {
            let rect = Rect::new(place.x + 12 + at as i32 * 76, place.y + 58, 70, 24);
            let on = usize::from(search.tab) == at;
            if on || hover == Some(Control::SearchTab(at as u8)) {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { FIELD_COLOR } else { HOVER_COLOR }, 256);
            }
            let wide = self.text.width(tab, 12);
            self.text.draw(canvas, rect.x + (rect.w - wide) / 2, rect.y + (rect.h - Text::line_height(12)) / 2, 12, if on { TEXT_COLOR } else { DIM_TEXT_COLOR }, tab);
        }
        let hits = self.hits();
        let start = search.row.saturating_sub(SHOWN - 1);
        let mut y = place.y + 92;
        for (at, hit) in hits.iter().enumerate().skip(start).take(SHOWN) {
            if at == start || hits[at - 1].heading != hit.heading {
                self.text.draw(canvas, place.x + 14, y + 10, 10, 0x8F8F8F, hit.heading);
                y += 26;
            }
            y += ROW;
        }
        if hits.is_empty() {
            self.text.draw(canvas, place.x + 14, place.y + 104, 12, DIM_TEXT_COLOR, "Nothing found");
        }
        let foot = place.bottom() - 24;
        canvas.fill(place.x + 1, foot - 8, place.w - 2, 1, EDGE_COLOR);
        self.text.draw(canvas, place.x + 14, foot, 11, DIM_TEXT_COLOR, "\u{2191}\u{2193} to move   Enter to do it   Tab for the next tab");
    }

    /// A row of what is found: its icon, what it says with what was typed
    /// marked in it, and its hint at the right.
    pub(super) fn draw_hit(&mut self, canvas: &mut Canvas, rect: Rect, row: usize, hovered: bool) {
        let Some(search) = self.search.clone() else { return };
        let Some(hit) = self.hits().into_iter().nth(row) else { return };
        let on = row == search.row;
        if on || hovered {
            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ACCENT } else { HOVER_COLOR }, 256);
        }
        let cy = rect.y + rect.h / 2;
        let icon = match &hit.found {
            Found::Act(_) => icons::POINTER,
            Found::Tool(at) => match (TOOLS.get(*at), FOUND_TOOLS[*at].0) {
                (Some(&(tool, ..)), _) => tool_icon(tool),
                (None, "Emoji") => icons::STICKER,
                (None, "Frame") => icons::HASH,
                _ => icons::CROP,
            },
            Found::PageLayer(index) => match self.loose().layers.get(*index).map(|layer| &layer.kind) {
                Some(Kind::Picture(_)) => icons::PICTURE,
                _ => icons::RECT,
            },
            Found::Layer(index) => match self.doc.layers.get(*index).map(|layer| &layer.kind) {
                Some(Kind::Picture(_)) => icons::PICTURE,
                _ => icons::RECT,
            },
            Found::Stack(id) => self.stack_by_id(*id).map_or(icons::DIST_H, stack_rows::stack_icon),
            Found::Page(_) => icons::FILE,
            Found::Frame(_) => icons::HASH,
            Found::File(_) => icons::FILE,
        };
        self.text.draw_centered(canvas, rect.x + 16, cy, 13, if on { 0xFFFFFF } else { DIM_TEXT_COLOR }, icon);
        let top = cy - Text::line_height(12) / 2;
        let (begin, end) = matched(&hit.says, search.typed.trim()).unwrap_or((0, 0));
        let mut x = rect.x + 32;
        for (part, lit) in [(&hit.says[..begin], false), (&hit.says[begin..end], true), (&hit.says[end..], false)] {
            if part.is_empty() {
                continue;
            }
            let ink = if lit { if on { 0xFFFFFF } else { 0xFFE629 } } else if on { 0xEEF1FF } else { TEXT_COLOR };
            if lit {
                self.text.look.heavy = 1;
            }
            // Measured as heavy as it is drawn, or what comes after it would
            // run into it, its space after it lost.
            self.text.draw(canvas, x, top, 12, ink, part);
            x += self.text.width(part, 12);
            self.text.look.heavy = 0;
        }
        if !hit.hint.is_empty() {
            let hint: String = hit.hint.chars().take(40).collect();
            let wide = self.text.width(&hint, 11);
            self.text.draw(canvas, rect.right() - 10 - wide, cy - Text::line_height(11) / 2, 11, if on { 0xDDE3FF } else { DIM_TEXT_COLOR }, &hint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_typed_is_found_in_what_is_said_whatever_its_case() {
        assert_eq!(matched("Add drop shadow", "SHAD"), Some((9, 13)));
        assert_eq!(matched("Zoom in", ""), Some((0, 0)));
        assert_eq!(matched("Zoom in", "out"), None);
        assert_eq!(matched("Größe ändern", "ände"), Some((8, 13)));
    }
}
