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
    /// The sizes a frame can be; and one section of them.
    Sizes,
    SizeGroup(u8),
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
    /// What can be done with a layer, a frame, and a color of the
    /// selection, right clicked in the panels.
    Layer(usize),
    Frame(usize),
    Color(u32),
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
    /// One of the sizes of the user's own; and the size of the frame
    /// kept as one of them.
    MySize(usize),
    SaveSize,
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
    /// A picture off the clipboard, whatever layers were copied; and a
    /// layer named or typed into, by where it is.
    PastePicture,
    RenameLayer(usize),
    EditText(usize),
    /// The frame worked in named, copied beside itself, done away with,
    /// and fitted to the view.
    RenameFrame,
    DuplicateFrame,
    DeleteFrame,
    ZoomFrame,
    /// Every layer with a color in it picked out; and that color, to be
    /// another in what is picked out.
    SelectColor(u32),
    ReplaceColor(u32),
}

/// The sizes a frame can be, by what each is for.
pub(super) const SIZES: [(&str, &str, i32, i32); 34] = [
    ("SOCIAL", "X post", 1200, 675),
    ("SOCIAL", "X header", 1500, 500),
    ("SOCIAL", "LinkedIn post", 1200, 627),
    ("SOCIAL", "LinkedIn banner", 1584, 396),
    ("SOCIAL", "Instagram post", 1080, 1350),
    ("SOCIAL", "Instagram square", 1080, 1080),
    ("SOCIAL", "Story / Reel", 1080, 1920),
    ("SOCIAL", "Facebook post", 1200, 630),
    ("SOCIAL", "YouTube thumbnail", 1280, 720),
    ("SOCIAL", "YouTube banner", 2560, 1440),
    ("SOCIAL", "Pinterest pin", 1000, 1500),
    ("SOCIAL", "Open Graph image", 1200, 630),
    ("SOCIAL", "Dribbble shot", 1600, 1200),
    ("SOCIAL", "Product Hunt gallery", 1270, 760),
    ("DEVICES", "iPhone 16 Pro", 1206, 2622),
    ("DEVICES", "iPhone 16 Pro Max", 1320, 2868),
    ("DEVICES", "iPhone SE", 750, 1334),
    ("DEVICES", "Pixel 9", 1080, 2424),
    ("DEVICES", "Pixel 9 Pro XL", 1344, 2992),
    ("DEVICES", "iPad Pro 13\"", 2064, 2752),
    ("DEVICES", "iPad mini", 1488, 2266),
    ("DEVICES", "MacBook Air 13\"", 2560, 1664),
    ("DEVICES", "MacBook Pro 14\"", 3024, 1964),
    ("DEVICES", "MacBook Pro 16\"", 3456, 2234),
    ("DEVICES", "1080p", 1920, 1080),
    ("DEVICES", "1440p", 2560, 1440),
    ("DEVICES", "4K", 3840, 2160),
    ("DOCS AND SLIDES", "Slide 16:9", 1920, 1080),
    ("DOCS AND SLIDES", "Slide 4:3", 1600, 1200),
    ("DOCS AND SLIDES", "README banner", 1280, 640),
    ("DOCS AND SLIDES", "Blog header", 1600, 900),
    ("DOCS AND SLIDES", "A4", 1240, 1754),
    ("DOCS AND SLIDES", "US Letter", 1275, 1650),
    ("DOCS AND SLIDES", "Docs screenshot", 1440, 900),
];

/// The sections of the sizes, each a list of its own beside the first.
pub(super) const SIZE_GROUPS: [(&str, &str); 3] = [("SOCIAL", "Social"), ("DEVICES", "Devices"), ("DOCS AND SLIDES", "Docs and slides")];

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
                if !self.frame_sizes.is_empty() {
                    items.push(Item::Heading("YOURS"));
                    for (index, (name, w, h, label)) in self.frame_sizes.iter().enumerate() {
                        let on = (self.doc.width, self.doc.height) == (*w, *h);
                        items.push(Item::entry(name.clone(), label, MenuAct::MySize(index)).on(on));
                    }
                    items.push(Item::Rule);
                }
                for (group, (heading, says)) in SIZE_GROUPS.iter().enumerate() {
                    let on = SIZES.iter().any(|(known, _, w, h)| known == heading && (self.doc.width, self.doc.height) == (*w, *h));
                    items.push(Item::sub(says, MenuKind::SizeGroup(group as u8)).on(on));
                }
                items.push(Item::Rule);
                items.push(Item::Heading("FROM THE SCREEN"));
                items.push(Item::entry(format!("This display · {} × {}", self.screen.0, self.screen.1), "", MenuAct::ScreenSize));
                items.push(Item::entry("Fit what is in it", "", MenuAct::FitContent));
                items.push(Item::Rule);
                items.push(Item::entry(format!("Save {} × {} as a size", self.doc.width, self.doc.height), "", MenuAct::SaveSize).look(Look::Icon(icons::PLUS)));
                items
            }
            MenuKind::SizeGroup(group) => {
                let (heading, _) = SIZE_GROUPS[usize::from(group).min(SIZE_GROUPS.len() - 1)];
                let mut items = vec![Item::Heading(heading)];
                for (index, (known, name, w, h)) in SIZES.iter().enumerate() {
                    if known == &heading {
                        let on = (self.doc.width, self.doc.height) == (*w, *h);
                        items.push(Item::entry(*name, size_label(index), MenuAct::Size(index)).on(on));
                    }
                }
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
                    if !self.copied.is_empty() {
                        items.push(do_("Paste picture", "Ctrl Shift V", Command::PastePicture));
                    }
                    items.push(do_("Select all", "Ctrl A", Command::SelectAll));
                    items.push(Item::Rule);
                    items.push(do_("Zoom to fit", "Ctrl 0", Command::ZoomFit));
                    items.push(do_("Rulers", "Shift R", Command::Rulers).on(self.rulers));
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
            MenuKind::Page(page) => vec![do_("Rename", "Double-click", Command::RenamePage(page)), do_("Duplicate page", "", Command::DuplicatePage(page)), do_("Export every frame", "Ctrl Shift E", Command::ExportPage), Item::Rule, do_("New page", "", Command::NewPage), Item::Rule, do_("Delete page", "", Command::DeletePage(page))],
            MenuKind::Layer(index) => {
                let layer = self.doc.layers.get(index);
                let (hidden, locked) = layer.map_or((false, false), |layer| (layer.hidden, layer.locked));
                let mut items = Vec::new();
                match layer.map(|layer| &layer.kind) {
                    Some(Kind::Picture(_)) => items.push(do_("Rename", "Double-click", Command::RenameLayer(index))),
                    Some(Kind::Shape(shape)) if matches!(shape.tool, Tool::Text | Tool::Badge) => items.push(do_("Edit text", "Enter", Command::EditText(index))),
                    _ => {}
                }
                if !items.is_empty() {
                    items.push(Item::Rule);
                }
                items.push(do_("Copy", "Ctrl C", Command::Copy));
                items.push(do_("Paste", "Ctrl V", Command::Paste));
                items.push(do_("Duplicate", "Ctrl D", Command::Duplicate));
                items.push(Item::Rule);
                items.push(Item::sub("Order", MenuKind::Order));
                if self.chosen().len() > 1 {
                    items.push(Item::entry("Frame selection", "Ctrl Alt G", MenuAct::Act(Action::FrameThem)));
                }
                if let Some(Kind::Shape(shape)) = layer.map(|layer| &layer.kind) {
                    items.push(do_("Select same color", "", Command::SelectColor(style::main_color(shape))));
                }
                items.push(Item::Rule);
                items.push(do_(if hidden { "Show" } else { "Hide" }, "Ctrl Shift H", Command::ToggleHide));
                items.push(do_(if locked { "Unlock" } else { "Lock" }, "Ctrl Shift L", Command::ToggleLock));
                items.push(Item::Rule);
                items.push(do_("Delete", "Del", Command::Delete));
                items
            }
            MenuKind::Frame(_) => vec![
                do_("Rename", "Double-click", Command::RenameFrame),
                do_("Duplicate frame", "", Command::DuplicateFrame),
                do_("Export frame", "Ctrl E", Command::ExportFrame),
                do_("Zoom to frame", "Shift 1", Command::ZoomFrame),
                Item::Rule,
                do_("Delete frame", "Del", Command::DeleteFrame),
            ],
            MenuKind::Color(color) => vec![do_("Select all with this color", "", Command::SelectColor(color)), do_("Replace color\u{2026}", "", Command::ReplaceColor(color))],
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
            MenuAct::MySize(index) => {
                if let Some(&(_, w, h, _)) = self.frame_sizes.get(index) {
                    self.resize_frame(w, h);
                }
            }
            MenuAct::SaveSize => {
                let (w, h) = (self.doc.width, self.doc.height);
                // Called what the frame is, unless that is only its number:
                // then a name of its own, never the size it already says.
                let numbered = self.doc.name.strip_prefix("Frame ").is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()));
                let name = if numbered || self.doc.name.trim().is_empty() { format!("My size {}", self.frame_sizes.len() + 1) } else { self.doc.name.trim().to_owned() };
                match crate::options::add_frame_size(&name, w, h) {
                    Ok(()) => {
                        self.add_size(name.clone(), w, h);
                        self.say(format!("Saved {name} as a size"));
                    }
                    Err(error) => self.say(format!("Could not save the size: {error}")),
                }
            }
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
            Command::RenamePage(page) => self.start_naming(Naming::Page(page), self.pages[page].name.clone()),
            Command::DuplicatePage(page) => self.duplicate_page(page),
            Command::DeletePage(page) => self.delete_page(page),
            Command::PastePicture => return Outcome::Paste,
            Command::RenameLayer(index) => {
                if let Some(Kind::Picture(picture)) = self.doc.layers.get(index).map(|layer| &layer.kind) {
                    self.renaming = Some((Naming::Layer(index), picture.name.clone()));
                }
            }
            Command::EditText(index) => {
                if index < self.doc.layers.len() {
                    self.checkpoint();
                    self.selected = vec![index];
                    self.edit_text(index);
                }
            }
            Command::RenameFrame => self.renaming = Some((Naming::Frame, self.doc.name.clone())),
            Command::DuplicateFrame => self.duplicate_frame(),
            Command::DeleteFrame => self.delete_frame(),
            Command::ZoomFrame => self.fit(),
            Command::SelectColor(color) => {
                self.selected = (0..self.doc.layers.len())
                    .filter(|&index| {
                        let layer = &self.doc.layers[index];
                        !layer.hidden && !layer.locked && matches!(&layer.kind, Kind::Shape(shape) if shape.color == color || shape.filled && shape.fill == color)
                    })
                    .collect();
            }
            Command::ReplaceColor(color) => self.open_picker(Paint::Recolor(color)),
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

/// What each of `SIZES` measures, said as its keys are.
pub(super) fn size_label(index: usize) -> &'static str {
    static LABELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    LABELS.get_or_init(|| SIZES.iter().map(|(_, _, w, h)| format!("{w} \u{d7} {h}")).collect())[index].as_str()
}

/// A size of the user's own from a line of the config file, as
/// `Pixel 11 = 1080x2424`: what it is called, and how wide and high.
pub fn parse_frame_size(text: &str) -> Option<(String, i32, i32)> {
    let (name, size) = text.rsplit_once('=')?;
    let (w, h) = size.trim().split_once(['x', 'X', '\u{d7}'])?;
    let (w, h) = (w.trim().parse::<i32>().ok()?, h.trim().parse::<i32>().ok()?);
    let name = name.trim();
    (!name.is_empty() && (16..=16000).contains(&w) && (16..=16000).contains(&h)).then(|| (name.to_owned(), w, h))
}

impl Studio {
    /// A size of the user's own, to be in the list of sizes, unless it is
    /// there already.
    pub fn add_size(&mut self, name: String, width: i32, height: i32) {
        if self.frame_sizes.iter().any(|(known, w, h, _)| *known == name && (*w, *h) == (width, height)) {
            return;
        }
        // What it measures, said once and kept for as long as the window.
        let label: &'static str = Box::leak(format!("{width} \u{d7} {height}").into_boxed_str());
        self.frame_sizes.push((name, width, height, label));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_of_ones_own_is_read_from_its_line_and_the_sizes_are_each_in_a_section() {
        assert_eq!(parse_frame_size("Pixel 11 = 1080x2424"), Some(("Pixel 11".into(), 1080, 2424)));
        assert_eq!(parse_frame_size("A = b = 300 x 200"), Some(("A = b".into(), 300, 200)));
        assert_eq!((parse_frame_size("1080x2424"), parse_frame_size("Tiny = 2x2")), (None, None));
        assert!(SIZES.iter().all(|(heading, ..)| SIZE_GROUPS.iter().any(|(known, _)| known == heading)));
        assert_eq!(size_label(0), "1200 \u{d7} 675");
    }
}
