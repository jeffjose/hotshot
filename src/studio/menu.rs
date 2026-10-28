//! The lists that open from a field, a right click, the logo, a page and
//! the zoom: what is in each, where it is, how it is drawn, and what each
//! thing in it does. A list may open another beside it, as a menu does.

use super::*;

/// Which list is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuKind {
    /// The faces text can be set in.
    Fonts,
    /// The heads an end of a line can have.
    Heads(usize),
    /// The sizes a frame can be.
    Sizes,
    /// The kinds of file an export size can be written as.
    Format(usize),
    /// What can be done with what is under a right click.
    Context,
    /// Over and under the rest.
    Order,
    /// Everything, from the logo; and each part of it.
    Main,
    MainSub(u8),
    /// What can be done with a page.
    Page(usize),
    /// How large the page is shown.
    Zoom,
    /// Where a border lies on its edge; whether a line is solid, dashed
    /// or dotted; whether it is drawn clean or by hand; the window round
    /// a picture; and the shadows there are.
    Edge,
    Dash,
    Hand,
    Window,
    Shadows,
    /// The styles of Style Mode.
    Styles,
}

/// An open list: where it hangs from, and the one open beside it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct MenuState {
    pub kind: MenuKind,
    pub at: (i32, i32),
    pub sub: Option<(usize, MenuKind)>,
}

/// One thing in a list.
pub(super) enum Item {
    Heading(&'static str),
    Rule,
    Entry { says: String, keys: &'static str, act: MenuAct, on: bool, sub: Option<MenuKind>, look: Look },
}

/// A small picture of what an entry is, beside it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Look {
    Plain,
    Face(usize),
    Head(u8),
    Icon(&'static str),
}

/// What choosing an entry does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum MenuAct {
    Nothing,
    Face(usize),
    Head(usize, u8),
    Size(usize),
    ScreenSize,
    FitContent,
    Format(usize, crate::output::Format),
    Edge(u8),
    Dash(u8),
    Hand(bool),
    Window(u8),
    Shadow(usize),
    Style(u8),
    Act(Action),
    Do(Command),
}

/// The things a menu does that are not buttons of the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Command {
    Copy,
    CopyPng,
    Paste,
    Duplicate,
    Delete,
    Front,
    Back,
    ToggleHide,
    ToggleLock,
    ExportFrame,
    ExportPage,
    Home,
    Undo,
    Redo,
    SelectAll,
    NewPage,
    NewFrame,
    Save,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomSelection,
    Zoom(u16),
    Rulers,
    Palette,
    Capture,
    Quit,
    RenamePage(usize),
    DuplicatePage(usize),
    DeletePage(usize),
}

/// The sizes a frame can be, by what each is for.
pub(super) const SIZES: [(&str, &str, i32, i32); 8] = [
    ("SOCIAL", "X post", 1200, 675),
    ("SOCIAL", "LinkedIn post", 1200, 627),
    ("SOCIAL", "Instagram post", 1080, 1350),
    ("SOCIAL", "Story", 1080, 1920),
    ("SOCIAL", "YouTube thumbnail", 1280, 720),
    ("DOCS AND SLIDES", "Slide 16:9", 1920, 1080),
    ("DOCS AND SLIDES", "README banner", 1280, 640),
    ("DOCS AND SLIDES", "A4", 1240, 1754),
];

const ROW: i32 = 26;

impl Item {
    fn entry(says: impl Into<String>, keys: &'static str, act: MenuAct) -> Item {
        Item::Entry { says: says.into(), keys, act, on: false, sub: None, look: Look::Plain }
    }

    fn sub(says: &str, kind: MenuKind) -> Item {
        Item::Entry { says: says.into(), keys: "", act: MenuAct::Nothing, on: false, sub: Some(kind), look: Look::Plain }
    }

    fn on(self, on: bool) -> Item {
        match self {
            Item::Entry { says, keys, act, sub, look, .. } => Item::Entry { says, keys, act, on, sub, look },
            other => other,
        }
    }

    fn look(self, look: Look) -> Item {
        match self {
            Item::Entry { says, keys, act, on, sub, .. } => Item::Entry { says, keys, act, on, sub, look },
            other => other,
        }
    }

    fn high(&self) -> i32 {
        match self {
            Item::Heading(_) => 22,
            Item::Rule => 9,
            Item::Entry { .. } => ROW,
        }
    }
}

impl Studio {
    /// What is in a list, as things are now.
    pub(super) fn menu_items(&mut self, kind: MenuKind) -> Vec<Item> {
        let do_ = |says: &str, keys: &'static str, command: Command| Item::entry(says, keys, MenuAct::Do(command));
        let picked = self.picked().map(|index| self.doc.layers[index].kind.clone());
        match kind {
            MenuKind::Fonts => {
                let now = match &picked {
                    Some(Kind::Shape(shape)) => shape.face,
                    _ => self.face,
                };
                let mut items = vec![Item::Heading("BUILT IN")];
                for (face, name) in FACES.iter().enumerate() {
                    let what = ["sans", "serif", "hand", "mono", "impact"][face];
                    items.push(Item::Entry { says: name.to_string(), keys: what, act: MenuAct::Face(face), on: face == now, sub: None, look: Look::Face(face) });
                }
                items
            }
            MenuKind::Heads(end) => {
                let (heads, ends) = match &picked {
                    Some(Kind::Shape(shape)) => (shape.heads, shape.ends),
                    _ => ([false, true], [shapes::FILLED_HEAD; 2]),
                };
                let none = shapes::HEADS.len() as u8;
                let mut items = vec![Item::Heading(if end == 0 { "START" } else { "END" })];
                items.push(Item::entry("None", "", MenuAct::Head(end, none)).on(!heads[end]).look(Look::Head(none)));
                for style in 0..none {
                    items.push(Item::entry(shapes::HEADS[usize::from(style)], "", MenuAct::Head(end, style)).on(heads[end] && ends[end] == style).look(Look::Head(style)));
                }
                items
            }
            MenuKind::Sizes => {
                let mut items = Vec::new();
                let mut section = "";
                for (index, (heading, name, w, h)) in SIZES.iter().enumerate() {
                    if *heading != section {
                        if !section.is_empty() {
                            items.push(Item::Rule);
                        }
                        items.push(Item::Heading(heading));
                        section = heading;
                    }
                    let on = (self.doc.width, self.doc.height) == (*w, *h);
                    items.push(Item::Entry { says: name.to_string(), keys: "", act: MenuAct::Size(index), on, sub: None, look: Look::Plain });
                }
                items.push(Item::Rule);
                items.push(Item::Heading("FROM THE SCREEN"));
                items.push(Item::entry(format!("This display · {} × {}", self.screen.0, self.screen.1), "", MenuAct::ScreenSize));
                items.push(Item::entry("Fit what is in it", "", MenuAct::FitContent));
                items
            }
            MenuKind::Format(index) => {
                let now = self.doc.exports.get(index).map(|size| size.format);
                [crate::output::Format::Png, crate::output::Format::Jpeg, crate::output::Format::Webp].iter().map(|&format| Item::entry(format.ending().to_ascii_uppercase(), "", MenuAct::Format(index, format)).on(Some(format) == now)).collect()
            }
            MenuKind::Styles => style::STYLE_NAMES.iter().enumerate().map(|(at, name)| Item::entry(*name, ["Q", "W", "E", "R", "T", "A", "S", "D", "G", "H"][at], MenuAct::Style(at as u8))).collect(),
            MenuKind::Edge => {
                let now = match &picked {
                    Some(Kind::Shape(shape)) => shape.edge,
                    _ => shapes::CENTER,
                };
                EDGES.iter().enumerate().map(|(edge, says)| Item::entry(*says, "", MenuAct::Edge(edge as u8)).on(edge as u8 == now)).collect()
            }
            MenuKind::Dash => {
                let now = match &picked {
                    Some(Kind::Shape(shape)) => dash_of(shape),
                    _ => 0,
                };
                DASHES.iter().enumerate().map(|(dash, says)| Item::entry(*says, "", MenuAct::Dash(dash as u8)).on(dash as u8 == now)).collect()
            }
            MenuKind::Hand => {
                let now = matches!(&picked, Some(Kind::Shape(shape)) if shape.sketch);
                vec![Item::entry("Clean", "", MenuAct::Hand(false)).on(!now), Item::entry("By hand", "", MenuAct::Hand(true)).on(now)]
            }
            MenuKind::Window => {
                let now = match &picked {
                    Some(Kind::Picture(picture)) => picture.window,
                    _ => frame::Window::None,
                };
                frame::WINDOWS.iter().enumerate().map(|(which, (window, says))| Item::entry(*says, "", MenuAct::Window(which as u8)).on(*window == now)).collect()
            }
            MenuKind::Shadows => {
                let now = match &picked {
                    Some(Kind::Picture(picture)) => picture.shadow,
                    Some(Kind::Shape(shape)) => shape.shadow.and_then(|drop| (1..=frame::SHADOWS.len()).find(|&which| actions::custom_drop(which) == drop)).unwrap_or(0),
                    None => 0,
                };
                let mut items = vec![Item::Heading("DROP SHADOW")];
                for which in 1..=frame::SHADOWS.len() {
                    items.push(Item::entry(frame::SHADOWS[which - 1].0, "", MenuAct::Shadow(which)).on(which == now));
                }
                items
            }
            MenuKind::Context => {
                let mut items = Vec::new();
                let chosen = !self.chosen().is_empty();
                if chosen {
                    items.push(do_("Copy", "Ctrl C", Command::Copy));
                    items.push(do_("Copy frame as PNG", "Ctrl Shift C", Command::CopyPng));
                }
                items.push(do_("Paste here", "Ctrl V", Command::Paste));
                if chosen {
                    items.push(do_("Duplicate", "Ctrl D", Command::Duplicate));
                    items.push(do_("Delete", "Del", Command::Delete));
                    items.push(Item::Rule);
                    items.push(Item::sub("Order", MenuKind::Order));
                    if self.chosen().len() > 1 {
                        items.push(Item::entry("Frame selection", "Ctrl Alt G", MenuAct::Act(Action::FrameThem)));
                    }
                    if let Some(Kind::Picture(_)) = picked {
                        items.push(Item::Rule);
                        items.push(Item::entry("Crop", "Enter", MenuAct::Act(Action::Crop)));
                        items.push(Item::entry("Flip across", "Shift H", MenuAct::Act(Action::FlipAcross)));
                        items.push(Item::entry("Flip down", "Shift V", MenuAct::Act(Action::FlipDown)));
                        items.push(Item::entry("Actual size", "", MenuAct::Act(Action::Actual)));
                    } else if matches!(picked, Some(Kind::Shape(_))) {
                        items.push(Item::sub("Apply style", MenuKind::Styles));
                        items.push(Item::Rule);
                        items.push(Item::entry("Flip across", "Shift H", MenuAct::Act(Action::FlipAcross)));
                        items.push(Item::entry("Flip down", "Shift V", MenuAct::Act(Action::FlipDown)));
                    }
                    items.push(Item::Rule);
                    items.push(do_("Show / hide", "Ctrl Shift H", Command::ToggleHide));
                    items.push(do_("Lock / unlock", "Ctrl Shift L", Command::ToggleLock));
                } else {
                    items.push(do_("Select all", "Ctrl A", Command::SelectAll));
                    items.push(Item::Rule);
                    items.push(do_("Capture the screen", "Ctrl G", Command::Capture));
                }
                items.push(Item::Rule);
                items.push(do_("Export frame", "Ctrl E", Command::ExportFrame));
                items
            }
            MenuKind::Order => vec![do_("Bring to front", "Ctrl Shift ]", Command::Front), Item::entry("Bring forward", "Ctrl ]", MenuAct::Act(Action::Forward)), Item::entry("Send backward", "Ctrl [", MenuAct::Act(Action::Backward)), do_("Send to back", "Ctrl Shift [", Command::Back)],
            MenuKind::Main => vec![
                do_("Back to projects", "", Command::Home).look(Look::Icon(icons::HOME)),
                Item::Rule,
                Item::sub("File", MenuKind::MainSub(0)),
                Item::sub("Edit", MenuKind::MainSub(1)),
                Item::sub("View", MenuKind::MainSub(2)),
                Item::sub("Object", MenuKind::MainSub(3)),
                Item::sub("Capture", MenuKind::MainSub(4)),
                Item::Rule,
                do_("Search actions", "Ctrl K", Command::Palette).look(Look::Icon(icons::SEARCH)),
                do_("Close", "Ctrl Q", Command::Quit),
            ],
            MenuKind::MainSub(0) => vec![do_("New page", "", Command::NewPage), do_("New frame", "F", Command::NewFrame), Item::Rule, do_("Save", "Ctrl S", Command::Save), do_("Export frame", "Ctrl E", Command::ExportFrame), do_("Export every frame of the page", "Ctrl Shift E", Command::ExportPage)],
            MenuKind::MainSub(1) => vec![do_("Undo", "Ctrl Z", Command::Undo), do_("Redo", "Ctrl Shift Z", Command::Redo), Item::Rule, do_("Copy", "Ctrl C", Command::Copy), do_("Copy frame as PNG", "Ctrl Shift C", Command::CopyPng), do_("Paste", "Ctrl V", Command::Paste), do_("Duplicate", "Ctrl D", Command::Duplicate), do_("Delete", "Del", Command::Delete), Item::Rule, do_("Select all", "Ctrl A", Command::SelectAll)],
            MenuKind::MainSub(2) => vec![do_("Zoom in", "Ctrl +", Command::ZoomIn), do_("Zoom out", "Ctrl \u{2212}", Command::ZoomOut), do_("Zoom to fit", "Ctrl 0", Command::ZoomFit), do_("Zoom to selection", "Shift 2", Command::ZoomSelection), do_("Actual size", "Ctrl 1", Command::Zoom(100)), Item::Rule, do_("Rulers", "Shift R", Command::Rulers).on(self.rulers)],
            MenuKind::MainSub(3) => vec![Item::sub("Order", MenuKind::Order), Item::entry("Frame selection", "Ctrl Alt G", MenuAct::Act(Action::FrameThem)), Item::Rule, Item::entry("Flip across", "Shift H", MenuAct::Act(Action::FlipAcross)), Item::entry("Flip down", "Shift V", MenuAct::Act(Action::FlipDown)), Item::entry("Crop", "Enter", MenuAct::Act(Action::Crop)), Item::Rule, do_("Show / hide", "Ctrl Shift H", Command::ToggleHide), do_("Lock / unlock", "Ctrl Shift L", Command::ToggleLock)],
            MenuKind::MainSub(_) => vec![do_("Capture the screen", "Ctrl G", Command::Capture), do_("Paste a picture", "Ctrl V", Command::Paste)],
            MenuKind::Page(page) => vec![do_("Rename", "Double-click", Command::RenamePage(page)), do_("Duplicate page", "", Command::DuplicatePage(page)), do_("Export every frame", "", Command::ExportPage), Item::Rule, do_("Delete page", "", Command::DeletePage(page))],
            MenuKind::Zoom => vec![
                do_("Zoom in", "Ctrl +", Command::ZoomIn),
                do_("Zoom out", "Ctrl \u{2212}", Command::ZoomOut),
                do_("Zoom to fit", "Ctrl 0", Command::ZoomFit),
                do_("Zoom to selection", "Shift 2", Command::ZoomSelection),
                do_("Zoom to 50%", "", Command::Zoom(50)),
                do_("Zoom to 100%", "Ctrl 1", Command::Zoom(100)),
                do_("Zoom to 200%", "", Command::Zoom(200)),
                Item::Rule,
                do_("Rulers", "Shift R", Command::Rulers).on(self.rulers),
            ],
        }
    }

    /// How wide a list is: as wide as the widest thing in it.
    fn menu_width(&mut self, items: &[Item]) -> i32 {
        let mut wide = 150;
        for item in items {
            if let Item::Entry { says, keys, look, sub, .. } = item {
                let lead = if *look == Look::Plain { 0 } else { 30 };
                let tail = if keys.is_empty() { 0 } else { self.text.width(keys, 11) + 18 } + if sub.is_some() { 14 } else { 0 };
                let px = if let Look::Face(_) = look { 14 } else { 12 };
                wide = wide.max(self.text.width(says, px) + lead + tail + 34);
            }
        }
        wide.min(420)
    }

    /// The lists that are open, laid out first so that they come before
    /// what is under them: each row a control, each list a panel.
    pub(super) fn layout_menus(&mut self, layout: &mut Layout) {
        let Some(menu) = self.menu else { return };
        let mut front = Vec::new();
        let mut levels = vec![(menu.kind, menu.at)];
        let first = self.menu_items(menu.kind);
        let first_wide = self.menu_width(&first);
        let first_high = first.iter().map(Item::high).sum::<i32>() + 10;
        // Kept on the window: shifted in from its right and bottom edges.
        let at = (menu.at.0.min(self.width - first_wide - 6).max(4), menu.at.1.min(self.height - first_high - 6).max(4));
        levels[0].1 = at;
        if let Some((row, kind)) = menu.sub {
            let y = at.1 + 5 + first.iter().take(row).map(Item::high).sum::<i32>() - 5;
            levels.push((kind, (at.0 + first_wide + 4, y)));
        }
        for (level, (kind, at)) in levels.into_iter().enumerate() {
            let items = self.menu_items(kind);
            let wide = self.menu_width(&items);
            let high = items.iter().map(Item::high).sum::<i32>() + 10;
            let at = (at.0.min(self.width - wide - 4).max(4), at.1.min(self.height - high - 4).max(4));
            let mut y = at.1 + 5;
            for (index, item) in items.iter().enumerate() {
                if let Item::Entry { .. } = item {
                    front.push((Rect::new(at.0 + 5, y, wide - 10, ROW), Control::MenuRow(level as u8, index as u16)));
                }
                y += item.high();
            }
            let place = Rect::new(at.0, at.1, wide, high);
            front.push((place, Control::Panel));
            layout.menus.push((place, kind));
        }
        front.append(&mut layout.controls);
        layout.controls = front;
    }

    /// The lists that are open, drawn over everything.
    pub(super) fn draw_menus(&mut self, canvas: &mut Canvas, layout: &Layout, hover: Option<Control>) {
        for (level, &(place, kind)) in layout.menus.iter().enumerate() {
            for grow in (1..=5).rev() {
                canvas.round_fill(place.x - grow, place.y - grow + 4, place.w + 2 * grow, place.h + 2 * grow, 9 + grow, 0x000000, 16);
            }
            canvas.round_fill(place.x, place.y, place.w, place.h, 9, 0x1F1F1F, 256);
            canvas.round_outline(place.x, place.y, place.w, place.h, 9, 1, 0x3A3A3A);
            let items = self.menu_items(kind);
            let open = self.menu.and_then(|menu| menu.sub).filter(|_| level == 0).map(|(row, _)| row);
            let mut y = place.y + 5;
            let line = Text::line_height(12);
            for (index, item) in items.iter().enumerate() {
                match item {
                    Item::Heading(says) => self.text.draw(canvas, place.x + 13, y + 8, 10, 0x8F8F8F, says),
                    Item::Rule => canvas.fill(place.x + 7, y + 4, place.w - 14, 1, 0x3A3A3A),
                    Item::Entry { says, keys, on, sub, look, .. } => {
                        let lit = hover == Some(Control::MenuRow(level as u8, index as u16)) || open == Some(index);
                        if lit {
                            canvas.round_fill(place.x + 5, y, place.w - 10, ROW, 5, ACCENT, 256);
                        }
                        let mid = y + ROW / 2;
                        let mut x = place.x + 13;
                        match look {
                            Look::Face(face) => {
                                let px = 14;
                                let high = Text::line_height(px);
                                self.text.draw_in(*face, canvas, x, mid - high / 2, px, 0xFFFFFF, says);
                                x = place.x + place.w;
                            }
                            Look::Head(style) => {
                                draw_head_sample(canvas, x, mid, *style, if lit { 0xFFFFFF } else { TEXT_COLOR });
                                x += 34;
                            }
                            Look::Icon(icon) => {
                                self.text.draw_centered(canvas, x + 7, mid, 14, TEXT_COLOR, icon);
                                x += 24;
                            }
                            Look::Plain => {}
                        }
                        if x < place.x + place.w {
                            self.text.draw(canvas, x, mid - line / 2, 12, if lit { 0xFFFFFF } else { TEXT_COLOR }, says);
                        }
                        let mut right = place.x + place.w - 13;
                        if sub.is_some() {
                            self.text.draw_centered(canvas, right - 3, mid, 10, if lit { 0xFFFFFF } else { 0x8F8F8F }, icons::RIGHT);
                            right -= 14;
                        }
                        if *on {
                            self.text.draw_centered(canvas, right - 5, mid, 12, if lit { 0xFFFFFF } else { ACCENT_TEXT }, icons::CHECK);
                            right -= 18;
                        }
                        if !keys.is_empty() {
                            let wide = self.text.width(keys, 11);
                            self.text.draw(canvas, right - wide, mid - Text::line_height(11) / 2, 11, if lit { 0xDCE3FB } else { 0x8F8F8F }, keys);
                        }
                    }
                }
                y += item.high();
            }
        }
    }

    /// The pointer over a row of an open list: the list it opens, opened
    /// beside it.
    pub(super) fn hover_menu(&mut self, control: Option<Control>) {
        let Some(mut menu) = self.menu else { return };
        if let Some(Control::MenuRow(0, row)) = control {
            let items = self.menu_items(menu.kind);
            let sub = match items.get(usize::from(row)) {
                Some(Item::Entry { sub: Some(kind), .. }) => Some((usize::from(row), *kind)),
                _ => None,
            };
            if sub != menu.sub {
                menu.sub = sub;
                self.menu = Some(menu);
                self.dirty = true;
            }
        }
    }

    /// A row of an open list chosen.
    pub(super) fn press_menu(&mut self, level: u8, row: u16) -> Outcome {
        let Some(menu) = self.menu else { return Outcome::Continue };
        let kind = if level == 0 { menu.kind } else { menu.sub.map_or(menu.kind, |(_, kind)| kind) };
        let items = self.menu_items(kind);
        let Some(Item::Entry { act, sub, .. }) = items.get(usize::from(row)) else { return Outcome::Continue };
        if let Some(kind) = sub {
            self.menu = Some(MenuState { sub: Some((usize::from(row), *kind)), ..menu });
            return Outcome::Continue;
        }
        let act = *act;
        self.menu = None;
        self.dirty = true;
        self.run_menu(act)
    }

    pub(super) fn run_menu(&mut self, act: MenuAct) -> Outcome {
        match act {
            MenuAct::Nothing => {}
            MenuAct::Face(face) => {
                self.face = face;
                self.restyle(|layer| {
                    if let Kind::Shape(shape) = &mut layer.kind {
                        shape.face = face;
                    }
                });
            }
            MenuAct::Head(end, style) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    let on = usize::from(style) < shapes::HEADS.len();
                    shape.heads[end] = on;
                    if on {
                        shape.ends[end] = style;
                    }
                }
            }),
            MenuAct::Size(index) => self.resize_frame(SIZES[index].2, SIZES[index].3),
            MenuAct::ScreenSize => self.resize_frame(self.screen.0, self.screen.1),
            MenuAct::FitContent => self.fit_frame_to_content(),
            MenuAct::Format(index, format) => {
                if index < self.doc.exports.len() {
                    self.checkpoint();
                    self.doc.exports[index].format = format;
                    self.changed();
                }
            }
            MenuAct::Edge(edge) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.edge = edge;
                }
            }),
            MenuAct::Dash(dash) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    (shape.dashed, shape.dotted) = (dash == 1, dash == 2);
                }
            }),
            MenuAct::Hand(on) => self.restyle(|layer| {
                if let Kind::Shape(shape) = &mut layer.kind {
                    shape.sketch = on;
                }
            }),
            MenuAct::Window(which) => return self.press_control(Control::Window(which), false),
            MenuAct::Shadow(which) => return self.press_control(Control::Shadow(which), false),
            MenuAct::Style(style) => self.put_style(style),
            MenuAct::Act(action) => return self.act(action),
            MenuAct::Do(command) => return self.run_command(command),
        }
        Outcome::Continue
    }

    /// What a menu says to do.
    pub(super) fn run_command(&mut self, command: Command) -> Outcome {
        self.dirty = true;
        match command {
            Command::Copy => self.copy_layers(),
            Command::CopyPng => self.copy(),
            Command::Paste if !self.copied.is_empty() => self.paste_layers(),
            Command::Paste => return Outcome::Paste,
            Command::Duplicate => self.duplicate(),
            Command::Delete => self.delete(),
            Command::Front | Command::Back => {
                let mods = self.mods;
                self.mods.shift = true;
                self.restack(command == Command::Front);
                self.mods = mods;
            }
            Command::ToggleHide | Command::ToggleLock => {
                let chosen = self.chosen();
                if !chosen.is_empty() {
                    self.checkpoint();
                    for index in chosen {
                        let layer = &mut self.doc.layers[index];
                        if command == Command::ToggleHide {
                            layer.hidden = !layer.hidden;
                        } else {
                            layer.locked = !layer.locked;
                        }
                    }
                    if command == Command::ToggleLock {
                        self.selected.clear();
                    }
                    self.changed();
                }
            }
            Command::ExportFrame => self.export(),
            Command::ExportPage => {
                let all: Vec<usize> = (0..self.frame_count()).collect();
                match self.export_frames(&all) {
                    Ok(written) => self.say(format!("Exported {} pictures to {}", written.len(), shortened(&self.export_dir()))),
                    Err(error) => self.say(error),
                }
            }
            Command::Home => return Outcome::Home,
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::SelectAll => self.selected = (0..self.doc.layers.len()).filter(|&index| !self.doc.layers[index].locked).collect(),
            Command::NewPage => self.new_page(),
            Command::NewFrame => {
                // Beside the frame worked in, the same size.
                let at = (self.doc.x + self.doc.width as f32 + 80.0, self.doc.y);
                let size = (self.doc.width as f32, self.doc.height as f32);
                self.new_frame(at, size);
                self.fit();
            }
            Command::Save => self.save(true),
            Command::ZoomIn => {
                let middle = self.open_area().center();
                self.zoom_about(1.25, middle);
            }
            Command::ZoomOut => {
                let middle = self.open_area().center();
                self.zoom_about(0.8, middle);
            }
            Command::ZoomFit => self.fit(),
            Command::ZoomSelection => self.zoom_to_selection(),
            Command::Zoom(percent) => {
                let middle = self.open_area().center();
                self.zoom_about(f32::from(percent) / 100.0 / self.zoom, middle);
            }
            Command::Rulers => self.rulers = !self.rulers,
            Command::Palette => self.open_palette(),
            Command::Capture => return Outcome::Capture,
            Command::Quit => return Outcome::Quit,
            Command::RenamePage(page) => self.renaming = Some((Naming::Page(page), self.pages[page].name.clone())),
            Command::DuplicatePage(page) => self.duplicate_page(page),
            Command::DeletePage(page) => self.delete_page(page),
        }
        Outcome::Continue
    }
}

/// A small picture of an end of a line, as a list of heads shows it.
pub(super) fn draw_head_sample(canvas: &mut Canvas, x: i32, mid: i32, style: u8, color: u32) {
    canvas.fill(x, mid - 1, 20, 2, color);
    let tip = x + 24;
    match style {
        0 => {
            for step in 0..9 {
                canvas.fill(tip - 9 + step, mid - (9 - step) / 2, 1, (9 - step).max(1), color);
            }
        }
        1 => {
            for step in 0..7 {
                canvas.fill(tip - 8 + step, mid - 7 + step, 2, 2, color);
                canvas.fill(tip - 8 + step, mid + 6 - step, 2, 2, color);
            }
        }
        2 => canvas.round_outline(tip - 9, mid - 5, 9, 10, 1, 1, color),
        3 => canvas.disc(tip - 4, mid, 4, color, 256),
        4 => canvas.round_outline(tip - 9, mid - 4, 9, 9, 4, 1, color),
        5 => {
            for step in 0..5 {
                canvas.fill(tip - 9 + step, mid - step, 1, 2 * step + 1, color);
                canvas.fill(tip - 1 - step, mid - step, 1, 2 * step + 1, color);
            }
        }
        6 => canvas.fill(tip - 3, mid - 6, 2, 12, color),
        _ => {}
    }
}

/// Where a border can lie on its edge, by `shapes::CENTER` and the rest.
pub(super) const EDGES: [&str; 3] = ["Center", "Inside", "Outside"];

/// How a line can be drawn: solid, in dashes, in dots.
pub(super) const DASHES: [&str; 3] = ["Solid", "Dashed", "Dotted"];

/// Which of `DASHES` a shape's line is.
pub(super) fn dash_of(shape: &Shape) -> u8 {
    if shape.dotted {
        2
    } else if shape.dashed {
        1
    } else {
        0
    }
}
