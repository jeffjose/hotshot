//! The projects: where the canvas's files are kept, and the window that
//! shows them, `hotshot --edit` with no file. A project is a folder in
//! the library, `~/Pictures/hotshot`, and a file is a `.hotshot` in one;
//! files made without a project go in its Drafts. The pictures taken in
//! capture mode are there too, to begin a file from.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::canvas::{Canvas, Rect};
use crate::editor::{Cursor, Key, Mods};
use crate::studio::{self, ACCENT, DIM_TEXT_COLOR, EDGE_COLOR, FIELD_COLOR, HOVER_COLOR, PANEL_COLOR, ROW_ON, TEXT_COLOR, VIEW_COLOR};
use crate::text::{icons, Text};

/// The folder the projects are in.
pub fn library() -> PathBuf {
    std::env::var_os("HOTSHOT_LIBRARY").map(PathBuf::from).unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("Pictures").join("hotshot"))
}

/// The folder files made without a project go in.
pub fn drafts() -> PathBuf {
    library().join("Drafts")
}

/// A file in a folder no other file has the name of: `Untitled.hotshot`,
/// or `Untitled 2.hotshot`, and so on.
pub fn fresh_file(dir: &Path, stem: &str) -> PathBuf {
    let _ = std::fs::create_dir_all(dir);
    (1..).map(|count| dir.join(if count == 1 { format!("{stem}.hotshot") } else { format!("{stem} {count}.hotshot") })).find(|path| !path.exists()).unwrap_or_else(|| dir.join(format!("{stem}.hotshot")))
}

/// How long ago a time was, as said of a file.
fn ago(when: SystemTime) -> String {
    let seconds = SystemTime::now().duration_since(when).map_or(0, |since| since.as_secs());
    match seconds {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{} h ago", seconds / 3600),
        86_400..=172_799 => "yesterday".to_owned(),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).and_then(|meta| meta.modified()).unwrap_or(SystemTime::UNIX_EPOCH)
}

/// The files of a kind in a folder, the latest first.
fn listed(dir: &Path, wanted: &dyn Fn(&Path) -> bool) -> Vec<(PathBuf, SystemTime)> {
    let mut found: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| wanted(path)).map(|path| {
        let when = modified(&path);
        (path, when)
    }).collect();
    found.sort_by(|a, b| b.1.cmp(&a.1));
    found
}

fn is_file(path: &Path) -> bool {
    path.extension().is_some_and(|ending| ending == "hotshot")
}

fn is_picture(path: &Path) -> bool {
    crate::convert::reads(path)
}

/// What the main part of the window shows.
#[derive(Clone, PartialEq, Debug)]
pub enum Place {
    Recent,
    Captures,
    Drafts,
    Project(PathBuf),
}

/// What a click in the window does.
#[derive(Clone, PartialEq, Debug)]
enum Control {
    Go(Place),
    NewProject,
    NewFile,
    Open(PathBuf),
    Capture(PathBuf),
    /// The files shown as a list, or as cards; the menu of a file, and
    /// one of its rows.
    View(bool),
    More(PathBuf),
    FileAct(u8),
    Panel,
}

/// What the window has to do for the projects, that they cannot.
#[derive(Clone, PartialEq, Debug)]
pub enum Outcome {
    Continue,
    Quit,
    /// A file to be opened in the canvas.
    Open(PathBuf),
    /// A new file in a folder, to be opened in the canvas: empty, or with
    /// a picture from a file on it.
    New(PathBuf, Option<PathBuf>),
    /// What is on the clipboard wanted: a picture, or the path of one, to
    /// begin a new file in a folder with.
    Paste(PathBuf),
}

/// A small picture of a file or a capture, and when it was made from
/// what it is of.
type Thumb = (SystemTime, i32, i32, Vec<u32>);

/// The projects, the files in them, and the window they are shown in.
pub struct Home {
    text: Text,
    width: i32,
    height: i32,
    pub place: Place,
    pointer: (i32, i32),
    scroll: i32,
    thumbs: HashMap<PathBuf, Thumb>,
    /// A project being named, and what has been typed.
    naming: Option<(PathBuf, String)>,
    /// The folder pictures are captured into.
    captures: PathBuf,
    last_press: (u32, (i32, i32)),
    /// Whether the files are a list rather than cards; the file whose menu
    /// is open, and where; a file being renamed, and what is typed; and
    /// how many pages each file has and what its frames are called.
    pub list: bool,
    file_menu: Option<(Menu, (i32, i32))>,
    /// The row of the open menu come to by the keys.
    menu_row: Option<usize>,
    renaming_file: Option<(PathBuf, String)>,
    /// The name being typed, of a project or a file, is all picked out, as
    /// it is when begun: the first key types over it.
    name_picked: bool,
    facts: HashMap<PathBuf, (SystemTime, usize, String)>,
    dirty: bool,
}

/// How wide the list down the left is.
const SIDE: i32 = 248;

impl Home {
    pub fn new(text: Text, width: i32, height: i32, captures: PathBuf) -> Home {
        Home { text, width, height, place: Place::Recent, pointer: (0, 0), scroll: 0, thumbs: HashMap::new(), naming: None, captures, last_press: (0, (-100, -100)), list: false, file_menu: None, menu_row: None, renaming_file: None, name_picked: false, facts: HashMap::new(), dirty: true }
    }

    /// The text it draws with, handed back for the canvas to draw with.
    pub fn into_text(self) -> Text {
        self.text
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        (self.width, self.height, self.dirty) = (width.max(crate::studio::MIN_SIZE.0), height.max(crate::studio::MIN_SIZE.1), true);
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// A name being typed as it is shown: with a caret after it; or, all
    /// of it picked out, on the color of a selection.
    fn typed_shown(&mut self, canvas: &mut Canvas, (x, y): (i32, i32), px: u32, typed: String) -> String {
        if !self.name_picked {
            return format!("{typed}|");
        }
        let wide = self.text.width(&typed, px);
        canvas.round_fill(x - 2, y, wide + 4, Text::line_height(px), 3, ACCENT, 120);
        typed
    }

    pub fn title(&self) -> String {
        format!("{} \u{2014} hotshot", self.heading())
    }

    pub fn cursor(&self) -> Cursor {
        Cursor::Arrow
    }

    /// The projects, the latest changed first.
    fn projects(&self) -> Vec<PathBuf> {
        let mut found: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(library()).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.is_dir() && *path != drafts() && !path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'))).map(|path| {
            let when = modified(&path);
            (path, when)
        }).collect();
        found.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));
        found.into_iter().map(|(path, _)| path).collect()
    }

    /// The files the place shows.
    fn files(&self) -> Vec<(PathBuf, SystemTime)> {
        match &self.place {
            Place::Recent => {
                let mut all: Vec<(PathBuf, SystemTime)> = self.projects().iter().chain([drafts()].iter()).flat_map(|dir| listed(dir, &is_file)).collect();
                all.sort_by(|a, b| b.1.cmp(&a.1));
                all.truncate(24);
                all
            }
            Place::Drafts => listed(&drafts(), &is_file),
            Place::Project(dir) => listed(dir, &is_file),
            Place::Captures => Vec::new(),
        }
    }

    fn captured(&self) -> Vec<(PathBuf, SystemTime)> {
        let mut found = listed(&self.captures, &is_picture);
        found.truncate(120);
        found
    }

    /// The folder a new file goes in from where the window is.
    fn folder(&self) -> PathBuf {
        match &self.place {
            Place::Project(dir) => dir.clone(),
            _ => drafts(),
        }
    }

    fn heading(&self) -> String {
        match &self.place {
            Place::Recent => "Recent".to_owned(),
            Place::Captures => "Captures".to_owned(),
            Place::Drafts => "Drafts".to_owned(),
            Place::Project(dir) => dir.file_name().map_or("Project".to_owned(), |name| name.to_string_lossy().into_owned()),
        }
    }

    /// A small picture of a file, its first frame; or of a capture: made
    /// once, and again when the file changes.
    fn thumb(&mut self, path: &Path, wide: i32, high: i32) -> Option<&Thumb> {
        let when = modified(path);
        let fresh = self.thumbs.get(path).is_some_and(|(made, w, h, _)| *made == when && (*w, *h) == (wide, high));
        if !fresh {
            let bytes = std::fs::read(path).ok()?;
            let (w, h, pixels) = if is_file(path) {
                let pages = studio::read_pages(&bytes)?;
                let frame = pages.first()?.frames.first()?;
                let times = (wide as f32 / frame.width as f32).min(high as f32 / frame.height as f32);
                frame.render(times, &mut self.text)
            } else {
                let (w, h, pixels) = crate::pin::read(&bytes).or_else(|| crate::frame::open(&bytes).map(|photo| (photo.width, photo.height, photo.pixels.iter().map(|pixel| pixel | 0xFF00_0000).collect())))?;
                let times = (wide as f32 / w as f32).max(high as f32 / h as f32).min(1.0);
                let (sw, sh) = (((w as f32 * times).round() as i32).max(1), ((h as f32 * times).round() as i32).max(1));
                (sw, sh, crate::pin::sized(&pixels, w, h, sw, sh))
            };
            self.thumbs.insert(path.to_owned(), (when, w, h, pixels));
            let _ = (wide, high);
        }
        self.thumbs.get(path)
    }

    // ---- Where everything is ----

    fn layout(&mut self) -> Vec<(Rect, Control)> {
        let mut controls = Vec::new();
        let row = 30;
        let mut y = 64;
        for place in [Place::Recent, Place::Captures, Place::Drafts] {
            controls.push((Rect::new(10, y, SIDE - 20, row), Control::Go(place)));
            y += row + 2;
        }
        y += 22;
        controls.push((Rect::new(SIDE - 36, y - 4, 24, 24), Control::NewProject));
        y += 26;
        for project in self.projects() {
            if y + row > self.height - 60 {
                break;
            }
            controls.push((Rect::new(10, y, SIDE - 20, row), Control::Go(Place::Project(project))));
            y += row + 2;
        }
        controls.push((Rect::new(0, 0, SIDE, self.height), Control::Panel));
        if self.place != Place::Captures {
            controls.push((Rect::new(self.width - 36 - 116, 30, 116, 34), Control::NewFile));
            controls.push((Rect::new(self.width - 36 - 116 - 74, 33, 28, 28), Control::View(false)));
            controls.push((Rect::new(self.width - 36 - 116 - 44, 33, 28, 28), Control::View(true)));
        }
        for (place, control) in self.cards() {
            controls.push((place, control));
        }
        self.layout_file_menu(&mut controls);
        controls
    }

    /// The cards of the main part of the window: the files, and a new
    /// one; and the latest captures under them, or all of them.
    fn cards(&mut self) -> Vec<(Rect, Control)> {
        let (left, right) = (SIDE + 36, self.width - 36);
        let top = 120 - self.scroll;
        let mut cards = Vec::new();
        let across = if self.place == Place::Captures { 5 } else { 4 };
        let gap = 20;
        let each = (right - left - gap * (across - 1)) / across;
        let thumb = each * 3 / 5;
        let mut y = top;
        // As a list: a row each, under the names of its columns.
        if self.place != Place::Captures && self.list {
            let files = self.files();
            y += 28;
            for (path, _) in &files {
                let row = Rect::new(left, y, right - left, 52);
                cards.push((Rect::new(row.right() - 36, row.y + 13, 26, 26), Control::More(path.clone())));
                cards.push((row, Control::Open(path.clone())));
                y += 54;
            }
            y += 40;
        } else if self.place != Place::Captures {
            let files = self.files();
            let count = files.len() + usize::from(self.place != Place::Recent);
            for (at, (path, _)) in files.into_iter().enumerate() {
                let (col, line) = (at as i32 % across, at as i32 / across);
                cards.push((Rect::new(left + col * (each + gap), y + line * (thumb + 64), each, thumb + 44), Control::Open(path)));
            }
            if self.place != Place::Recent {
                let at = count as i32 - 1;
                cards.push((Rect::new(left + at % across * (each + gap), y + at / across * (thumb + 64), each, thumb + 44), Control::NewFile));
            }
            y += (count as i32 + across - 1).max(1) / across * (thumb + 64) + 40;
        }
        // The captures: the latest six under the files, or every one.
        let captures = self.captured();
        let (across, take) = if self.place == Place::Captures { (5, captures.len()) } else { (6, 6) };
        let each = (right - left - gap * (across - 1)) / across;
        let high = if self.place == Place::Captures { each * 3 / 5 } else { each * 2 / 3 };
        y += if self.place == Place::Captures { 0 } else { 30 };
        for (at, (path, _)) in captures.into_iter().take(take).enumerate() {
            let (col, line) = (at as i32 % across, at as i32 / across);
            cards.push((Rect::new(left + col * (each + gap), y + line * (high + 44), each, high), Control::Capture(path)));
        }
        cards
    }

    // ---- The pointer and the keys ----

    pub fn pointer_moved(&mut self, x: i32, y: i32) {
        self.pointer = (x, y);
        self.dirty = true;
    }

    /// A wheel or a trackpad turned by so many notches down, parts of one
    /// as well, up counted as fewer: the files scrolled as far.
    pub fn scrolled(&mut self, down: f32) {
        self.scroll = (self.scroll + (down * 60.0).round() as i32).max(0);
        self.dirty = true;
    }

    pub fn pressed(&mut self, button: u8, time: u32) -> Outcome {
        self.dirty = true;
        self.menu_row = None;
        match button {
            4 => {
                self.scroll = (self.scroll - 60).max(0);
                return Outcome::Continue;
            }
            5 => {
                self.scroll += 60;
                return Outcome::Continue;
            }
            1 | 3 => {}
            _ => return Outcome::Continue,
        }
        let at = self.pointer;
        let double = time.wrapping_sub(self.last_press.0) < 400 && (at.0 - self.last_press.1 .0).abs().max((at.1 - self.last_press.1 .1).abs()) <= 3;
        self.last_press = (time, at);
        let control = self.layout().into_iter().find(|(rect, _)| rect.contains(at.0, at.1)).map(|(_, control)| control);
        // A right click opens the menu of what is under it: a file, a
        // project, or anywhere else, the menu of the place shown.
        if button == 3 {
            if let Some((path, typed)) = self.renaming_file.take() {
                rename_file(&path, &typed);
            }
            self.commit_name();
            self.file_menu = match control {
                Some(Control::Open(path) | Control::More(path)) => Some((Menu::File(path), at)),
                Some(Control::Go(Place::Project(dir))) => Some((Menu::Project(dir), at)),
                Some(Control::FileAct(_)) => None,
                _ if self.place == Place::Captures => None,
                _ => Some((Menu::Empty, at)),
            };
            return Outcome::Continue;
        }
        // A click off the menu closes it and does nothing else.
        if self.file_menu.is_some() && !matches!(control, Some(Control::FileAct(_))) {
            if !self.file_menu_place().is_some_and(|place| place.contains(at.0, at.1)) {
                self.file_menu = None;
            }
            return Outcome::Continue;
        }
        if let Some((path, typed)) = self.renaming_file.take() {
            rename_file(&path, &typed);
        }
        if !matches!(control, Some(Control::Go(Place::Project(_)))) || !double {
            self.commit_name();
        }
        match control {
            Some(Control::Go(Place::Project(dir))) if double => {
                let name = dir.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned());
                (self.naming, self.name_picked) = (Some((dir, name)), true);
            }
            Some(Control::Go(place)) => {
                (self.place, self.scroll) = (place, 0);
            }
            Some(Control::NewProject) => self.new_project(),
            Some(Control::NewFile) => return Outcome::New(self.folder(), None),
            Some(Control::Open(path)) => return Outcome::Open(path),
            Some(Control::Capture(path)) => return Outcome::New(self.folder(), Some(path)),
            Some(Control::View(list)) => self.list = list,
            Some(Control::More(path)) => self.file_menu = Some((Menu::File(path), at)),
            Some(Control::FileAct(act)) => return self.file_act(act),
            Some(Control::Panel) | None => {}
        }
        Outcome::Continue
    }

    /// A new project, shown, its name to be typed.
    fn new_project(&mut self) {
        let dir = (1..).map(|count| library().join(if count == 1 { "New project".to_owned() } else { format!("New project {count}") })).find(|dir| !dir.exists()).unwrap_or_else(|| library().join("New project"));
        if std::fs::create_dir_all(&dir).is_ok() {
            self.place = Place::Project(dir.clone());
            self.naming = Some((dir, String::new()));
        }
    }

    /// The project being named given the name typed, its folder renamed.
    fn commit_name(&mut self) {
        let Some((dir, typed)) = self.naming.take() else { return };
        let typed = typed.trim().replace(['/', '\\'], "-");
        if typed.is_empty() || dir.file_name().is_some_and(|name| name.to_string_lossy() == typed) {
            return;
        }
        let to = library().join(&typed);
        if !to.exists() && std::fs::rename(&dir, &to).is_ok() && self.place == Place::Project(dir) {
            self.place = Place::Project(to);
        }
    }

    pub fn key(&mut self, key: Key, mods: Mods) -> Outcome {
        self.dirty = true;
        if let Some((path, typed)) = &mut self.renaming_file {
            // Shift and the like come as keys of their own, and leave it
            // picked out.
            let whole = self.name_picked;
            self.name_picked &= key == Key::Other;
            match key {
                Key::Char(letter) if !mods.ctrl => {
                    if whole {
                        typed.clear();
                    }
                    typed.push(letter);
                }
                Key::Backspace if whole => typed.clear(),
                Key::Backspace => drop(typed.pop()),
                Key::Enter => {
                    let (path, typed) = (path.clone(), typed.clone());
                    self.renaming_file = None;
                    rename_file(&path, &typed);
                }
                Key::Escape => self.renaming_file = None,
                _ => {}
            }
            return Outcome::Continue;
        }
        // An open menu takes the keys, as the canvas's do: Up and Down from
        // row to row, Enter to choose, Esc to close; Enter with no row come
        // to opens the file whose menu it is, as the menu says.
        if self.file_menu.is_some() && !mods.ctrl {
            let rows: Vec<usize> = self.menu_rows().iter().enumerate().filter(|(_, row)| matches!(row, Row::Entry(..))).map(|(at, _)| at).collect();
            let place = self.menu_row.and_then(|row| rows.iter().position(|&at| at == row));
            match key {
                Key::Escape => (self.file_menu, self.menu_row) = (None, None),
                Key::Down | Key::Up if !rows.is_empty() => {
                    let next = match (place, key == Key::Down) {
                        (None, true) => 0,
                        (None, false) => rows.len() - 1,
                        (Some(at), true) => (at + 1) % rows.len(),
                        (Some(at), false) => (at + rows.len() - 1) % rows.len(),
                    };
                    self.menu_row = Some(rows[next]);
                }
                Key::Enter => match self.menu_row.take() {
                    Some(row) => return self.file_act(row as u8),
                    None if matches!(self.file_menu, Some((Menu::File(_), _))) => return self.file_act(0),
                    None => {}
                },
                _ => {}
            }
            return Outcome::Continue;
        }
        if let Some((_, typed)) = &mut self.naming {
            // Shift and the like come as keys of their own, and leave it
            // picked out.
            let whole = self.name_picked;
            self.name_picked &= key == Key::Other;
            match key {
                Key::Char(letter) if !mods.ctrl => {
                    if whole {
                        typed.clear();
                    }
                    typed.push(letter);
                }
                Key::Backspace if whole => typed.clear(),
                Key::Backspace => drop(typed.pop()),
                Key::Enter => self.commit_name(),
                Key::Escape => self.naming = None,
                _ => {}
            }
            return Outcome::Continue;
        }
        match (key, mods.ctrl) {
            (Key::Char('q' | 'w' | 'Q' | 'W'), true) => Outcome::Quit,
            (Key::Char('n' | 'N'), true) => Outcome::New(self.folder(), None),
            (Key::Char('v' | 'V'), true) => Outcome::Paste(self.folder()),
            _ => Outcome::Continue,
        }
    }

    // ---- The picture of the window ----

    pub fn draw(&mut self, out: &mut [u32]) {
        let (width, height) = (self.width, self.height);
        let controls = self.layout();
        let hover = controls.iter().find(|(rect, _)| rect.contains(self.pointer.0, self.pointer.1)).map(|(_, control)| control.clone()).filter(|control| *control != Control::Panel);
        out.iter_mut().for_each(|pixel| *pixel = VIEW_COLOR);
        let mut canvas = Canvas::new(out, width, height);
        let line = Text::line_height(13);
        // Down the left: the places, and the projects.
        canvas.fill(0, 0, SIDE, height, PANEL_COLOR);
        canvas.fill(SIDE - 1, 0, 1, height, EDGE_COLOR);
        canvas.round_fill(18, 16, 30, 30, 8, ACCENT, 256);
        self.text.draw_centered(&mut canvas, 33, 31, 16, 0xFFFFFF, icons::CAMERA);
        self.text.draw(&mut canvas, 58, 31 - Text::line_height(15) / 2, 15, TEXT_COLOR, "hotshot");
        let captures = self.captured().len();
        let drafts = listed(&drafts(), &is_file).len();
        for (rect, control) in &controls {
            let (cx, cy) = rect.center();
            let hovered = hover.as_ref() == Some(control);
            match control {
                Control::Go(place) => {
                    let on = *place == self.place;
                    if on || hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { ROW_ON } else { HOVER_COLOR }, 256);
                    }
                    let (icon, name, count) = match place {
                        Place::Recent => (icons::CLOCK, "Recent".to_owned(), None),
                        Place::Captures => (icons::PICTURE, "Captures".to_owned(), Some(captures)),
                        Place::Drafts => (icons::DRAFTS, "Drafts".to_owned(), Some(drafts)),
                        Place::Project(dir) => {
                            let name = match self.naming.clone() {
                                Some((naming, typed)) if naming == *dir => self.typed_shown(&mut canvas, (rect.x + 34, cy - line / 2), 13, typed),
                                _ => dir.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned()),
                            };
                            (if on { icons::FOLDER_OPEN } else { icons::FOLDER }, name, Some(listed(dir, &is_file).len()))
                        }
                    };
                    self.text.draw_centered(&mut canvas, rect.x + 16, cy, 15, if on { TEXT_COLOR } else { DIM_TEXT_COLOR }, icon);
                    let name: String = name.chars().take(20).collect();
                    self.text.draw(&mut canvas, rect.x + 34, cy - line / 2, 13, TEXT_COLOR, &name);
                    if let Some(count) = count {
                        let says = count.to_string();
                        let wide = self.text.width(&says, 12);
                        self.text.draw(&mut canvas, rect.right() - 10 - wide, cy - Text::line_height(12) / 2, 12, DIM_TEXT_COLOR, &says);
                    }
                }
                Control::NewProject => {
                    if hovered {
                        canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, HOVER_COLOR, 256);
                    }
                    self.text.draw_centered(&mut canvas, cx, cy, 14, TEXT_COLOR, icons::PLUS);
                }
                _ => {}
            }
        }
        let projects_at = 64 + 3 * 32 + 22;
        self.text.draw(&mut canvas, 20, projects_at, 11, DIM_TEXT_COLOR, "PROJECTS");
        // Where the library is, home said as ~, and a long way kept to the
        // panel by leaving out its middle.
        let whole = library().display().to_string();
        let note = match std::env::var("HOME").ok().filter(|home| !home.is_empty()).and_then(|home| whole.strip_prefix(&home).map(str::to_owned)) {
            Some(rest) => format!("~{rest}/"),
            None => format!("{whole}/"),
        };
        let note = middle_out(&mut self.text, &note, 11, SIDE - 40);
        self.text.draw(&mut canvas, 20, height - 44, 11, DIM_TEXT_COLOR, "Projects are folders in");
        self.text.draw(&mut canvas, 20, height - 28, 11, TEXT_COLOR, &note);
        // The main part: a heading, and the cards under it.
        let left = SIDE + 36;
        canvas.set_clip(Rect::new(SIDE, 0, width - SIDE, height));
        if let Place::Project(_) = self.place {
            self.text.draw(&mut canvas, left, 26, 12, DIM_TEXT_COLOR, "Projects");
        }
        let heading = self.heading();
        self.text.draw(&mut canvas, left, 42, 26, TEXT_COLOR, &heading);
        self.text.draw(&mut canvas, left, 80, 12, DIM_TEXT_COLOR, "Paste a picture to start a file  \u{b7}  Ctrl V");
        let mut lowest = 0;
        for (rect, control) in &controls {
            let hovered = hover.as_ref() == Some(control);
            match control {
                Control::NewFile if rect.y < 100 => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 7, if hovered { 0x5472E4 } else { ACCENT }, 256);
                    self.text.draw_centered(&mut canvas, rect.x + 20, rect.y + rect.h / 2, 14, 0xFFFFFF, icons::PLUS);
                    self.text.draw(&mut canvas, rect.x + 34, rect.y + (rect.h - line) / 2, 13, 0xFFFFFF, "New file");
                }
                Control::NewFile => {
                    let thumb = rect.h - 44;
                    canvas.round_outline(rect.x, rect.y, rect.w, thumb, 10, 1, if hovered { 0x777777 } else { 0x4A4A4A });
                    self.text.draw_centered(&mut canvas, rect.x + rect.w / 2, rect.y + thumb / 2 - 10, 20, DIM_TEXT_COLOR, icons::PLUS);
                    self.text.draw_centered(&mut canvas, rect.x + rect.w / 2, rect.y + thumb / 2 + 16, 13, DIM_TEXT_COLOR, "New file");
                    lowest = lowest.max(rect.bottom());
                }
                Control::Open(path) if self.list => {
                    self.draw_file_row(&mut canvas, *rect, path, hovered);
                    lowest = lowest.max(rect.bottom());
                }
                Control::View(list) => {
                    let on = self.list == *list;
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, if on { FIELD_COLOR } else if hovered { HOVER_COLOR } else { VIEW_COLOR }, 256);
                    self.text.draw_centered(&mut canvas, rect.x + rect.w / 2, rect.y + rect.h / 2, 13, if on { TEXT_COLOR } else { DIM_TEXT_COLOR }, if *list { "\u{2261}" } else { icons::PICTURE });
                }
                Control::Open(path) => {
                    let thumb = rect.h - 44;
                    canvas.round_fill(rect.x, rect.y, rect.w, thumb, 10, 0x262626, 256);
                    if let Some((_, w, h, pixels)) = self.thumb(path, rect.w, thumb).cloned() {
                        blit(&mut canvas, &pixels, w, h, Rect::new(rect.x, rect.y, rect.w, thumb));
                    }
                    if hovered {
                        canvas.round_outline(rect.x, rect.y, rect.w, thumb, 10, 2, ACCENT);
                    }
                    let name = match self.renaming_file.clone() {
                        Some((known, typed)) if known == *path => self.typed_shown(&mut canvas, (rect.x, rect.y + thumb + 10), 13, typed),
                        _ => path.file_stem().map_or(String::new(), |stem| stem.to_string_lossy().into_owned()),
                    };
                    self.text.draw(&mut canvas, rect.x, rect.y + thumb + 10, 13, TEXT_COLOR, &name);
                    let meta = match (&self.place, path.parent().and_then(|dir| dir.file_name())) {
                        (Place::Recent, Some(project)) => format!("{} \u{b7} {}", project.to_string_lossy(), ago(modified(path))),
                        _ => format!("edited {}", ago(modified(path))),
                    };
                    self.text.draw(&mut canvas, rect.x, rect.y + thumb + 27, 12, DIM_TEXT_COLOR, &meta);
                    lowest = lowest.max(rect.bottom());
                }
                Control::Capture(path) => {
                    canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 8, 0x262626, 256);
                    if let Some((_, w, h, pixels)) = self.thumb(path, rect.w, rect.h).cloned() {
                        blit(&mut canvas, &pixels, w, h, *rect);
                    }
                    if hovered {
                        canvas.round_outline(rect.x, rect.y, rect.w, rect.h, 8, 2, ACCENT);
                    }
                    if self.place == Place::Captures {
                        self.text.draw(&mut canvas, rect.x, rect.bottom() + 8, 12, DIM_TEXT_COLOR, &ago(modified(path)));
                    }
                }
                _ => {}
            }
        }
        if self.place != Place::Captures {
            let first = controls.iter().find(|(_, control)| matches!(control, Control::Capture(_))).map(|(rect, _)| rect.y);
            if let Some(y) = first {
                self.text.draw(&mut canvas, left, y - 30, 15, TEXT_COLOR, "Captured lately");
                let wide = self.text.width("Captured lately", 15);
                self.text.draw(&mut canvas, left + wide + 12, y - 28, 12, DIM_TEXT_COLOR, "click one to begin a file with it");
            } else if lowest == 0 {
                self.text.draw(&mut canvas, left, 140, 13, DIM_TEXT_COLOR, "Nothing here yet.");
            }
        }
        // The names of the columns of the list, over its first row.
        if self.list {
            if let Some((row, _)) = controls.iter().find(|(rect, control)| matches!(control, Control::Open(_)) && rect.h == 52) {
                let at = row.y - 22;
                self.text.draw(&mut canvas, row.x + 90, at, 11, DIM_TEXT_COLOR, "Name");
                for (x, says) in list_columns(*row).iter().zip(["Pages", "Frames", "Size", "Edited"]) {
                    self.text.draw(&mut canvas, *x, at, 11, DIM_TEXT_COLOR, says);
                }
            }
        }
        canvas.reset_clip();
        self.draw_file_menu(&mut canvas, hover.as_ref());
        let _ = (FIELD_COLOR, lowest);
    }
}

/// A small picture drawn in the middle of a box, cut to the box's
/// rounded corners where it is larger.
fn blit(canvas: &mut Canvas, pixels: &[u32], w: i32, h: i32, into: Rect) {
    let (ox, oy) = (into.x + (into.w - w) / 2, into.y + (into.h - h) / 2);
    for y in 0..h {
        for x in 0..w {
            let (at_x, at_y) = (ox + x, oy + y);
            if !into.contains(at_x, at_y) {
                continue;
            }
            // Square corners cut round, as the box is.
            let corner = 8;
            let (dx, dy) = ((at_x - into.x).min(into.right() - 1 - at_x), (at_y - into.y).min(into.bottom() - 1 - at_y));
            if dx < corner && dy < corner && (corner - dx) * (corner - dx) + (corner - dy) * (corner - dy) > corner * corner {
                continue;
            }
            let color = pixels[(y * w + x) as usize];
            canvas.blend(at_x, at_y, color & 0xFF_FFFF, (color >> 24) as u8);
        }
    }
}

/// What a menu of the projects is the menu of: a file, a project, or the
/// place shown, right clicked where there is nothing.
#[derive(Clone, PartialEq, Debug)]
enum Menu {
    File(PathBuf),
    Project(PathBuf),
    Empty,
}

/// What a row of a menu of the projects does.
#[derive(Clone, PartialEq, Debug)]
enum Act {
    Open,
    OpenWindow,
    Rename,
    Duplicate,
    MoveTo(PathBuf),
    ShowFolder,
    Trash,
    NewFileIn,
    NewFile,
    NewProject,
    Paste,
    CopyPath,
}

/// A row of a menu: what it says, its keys and what it does; or a line
/// between rows.
enum Row {
    Entry(String, &'static str, Act),
    Rule,
}

/// How wide a menu is, and how high a row of it and a line between.
const MENU_W: i32 = 220;
const MENU_ROW: i32 = 28;
const MENU_RULE: i32 = 9;

/// A file renamed to what was typed, in its folder, unless there is one
/// called that already or nothing was typed.
fn rename_file(path: &Path, typed: &str) {
    let typed = typed.trim().replace(['/', '\\'], "-");
    if typed.is_empty() {
        return;
    }
    let to = path.with_file_name(format!("{typed}.hotshot"));
    if !to.exists() {
        let _ = std::fs::rename(path, to);
    }
}

/// Where a file goes when it is thrown away: a folder of the library's
/// own, out of the way, from which it can be taken back by hand.
pub fn trash() -> PathBuf {
    library().join(".trash")
}

/// A file or a project put in the trash, under a name nothing there has.
fn to_trash(path: &Path) {
    let bin = trash();
    if std::fs::create_dir_all(&bin).is_err() {
        return;
    }
    let (stem, ending) = (path.file_stem().map_or(String::new(), |stem| stem.to_string_lossy().into_owned()), path.extension().map(|ending| format!(".{}", ending.to_string_lossy())).unwrap_or_default());
    let mut to = bin.join(format!("{stem}{ending}"));
    let mut count = 2;
    while to.exists() {
        to = bin.join(format!("{stem} {count}{ending}"));
        count += 1;
    }
    let _ = std::fs::rename(path, to);
}

/// Words kept to a width by leaving out their middle, an ellipsis in its
/// place: the start and the end of a long path say the most about it.
fn middle_out(text: &mut Text, words: &str, px: u32, wide: i32) -> String {
    if text.width(words, px) <= wide {
        return words.to_owned();
    }
    let letters: Vec<char> = words.chars().collect();
    for keep in (1..letters.len()).rev() {
        let (head, tail) = (keep.div_ceil(2), keep / 2);
        let shorter: String = letters[..head].iter().chain(['\u{2026}'].iter()).chain(letters[letters.len() - tail..].iter()).collect();
        if text.width(&shorter, px) <= wide {
            return shorter;
        }
    }
    "\u{2026}".to_owned()
}

/// A folder opened in the file manager.
fn show_folder(dir: &Path) {
    let _ = std::process::Command::new("xdg-open").arg(dir).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
}

impl Home {
    /// What the open menu has in it.
    fn menu_rows(&self) -> Vec<Row> {
        let entry = |says: &str, keys: &'static str, act: Act| Row::Entry(says.to_owned(), keys, act);
        match self.file_menu.as_ref().map(|(menu, _)| menu) {
            None => Vec::new(),
            Some(Menu::File(path)) => {
                let mut rows = vec![entry("Open", "Enter", Act::Open), entry("Open in a new window", "", Act::OpenWindow), Row::Rule, entry("Rename", "", Act::Rename), entry("Duplicate", "", Act::Duplicate)];
                // Into Drafts or another project, whichever it is not in.
                let here = path.parent().map(Path::to_path_buf);
                let places: Vec<PathBuf> = std::iter::once(drafts()).chain(self.projects()).filter(|dir| Some(dir) != here.as_ref()).take(6).collect();
                if !places.is_empty() {
                    rows.push(Row::Rule);
                    for dir in places {
                        let name = dir.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned());
                        rows.push(Row::Entry(format!("Move to {name}"), "", Act::MoveTo(dir)));
                    }
                }
                rows.extend([Row::Rule, entry("Show in folder", "", Act::ShowFolder), entry("Copy path", "", Act::CopyPath), Row::Rule, entry("Move to trash", "", Act::Trash)]);
                rows
            }
            Some(Menu::Project(_)) => vec![entry("Open", "", Act::Open), entry("Rename", "Double-click", Act::Rename), entry("New file in it", "", Act::NewFileIn), Row::Rule, entry("Show in folder", "", Act::ShowFolder), entry("Copy path", "", Act::CopyPath), Row::Rule, entry("Move to trash", "", Act::Trash)],
            Some(Menu::Empty) => vec![entry("New file", "Ctrl N", Act::NewFile), entry("New project", "", Act::NewProject), Row::Rule, entry("Paste a picture", "Ctrl V", Act::Paste)],
        }
    }

    /// A row of the open menu chosen, by where it is among the rows.
    fn file_act(&mut self, row: u8) -> Outcome {
        let act = match self.menu_rows().into_iter().nth(usize::from(row)) {
            Some(Row::Entry(_, _, act)) => act,
            _ => return Outcome::Continue,
        };
        let Some((menu, _)) = self.file_menu.take() else { return Outcome::Continue };
        self.dirty = true;
        let stem = |path: &Path| path.file_stem().map_or(String::new(), |stem| stem.to_string_lossy().into_owned());
        match (menu, act) {
            (Menu::File(path), Act::Open) => return Outcome::Open(path),
            (Menu::Project(dir), Act::Open) => (self.place, self.scroll) = (Place::Project(dir), 0),
            (Menu::File(path), Act::OpenWindow) => {
                // A canvas of its own, as a hotshot of its own.
                if let Ok(me) = std::env::current_exe() {
                    let _ = std::process::Command::new(me).arg("--canvas").arg(&path).spawn();
                }
            }
            (Menu::File(path), Act::Rename) => (self.renaming_file, self.name_picked) = (Some((path.clone(), stem(&path))), true),
            (Menu::Project(dir), Act::Rename) => {
                let name = dir.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned());
                (self.naming, self.name_picked) = (Some((dir, name)), true);
            }
            (Menu::File(path), Act::Duplicate) => {
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(drafts);
                let _ = std::fs::copy(&path, fresh_file(&dir, &format!("{} copy", stem(&path))));
            }
            (Menu::File(path), Act::MoveTo(dir)) => {
                if std::fs::create_dir_all(&dir).is_ok() {
                    let _ = std::fs::rename(&path, fresh_file(&dir, &stem(&path)));
                }
            }
            (Menu::File(path), Act::ShowFolder) => show_folder(path.parent().unwrap_or(&path)),
            (Menu::Project(dir), Act::ShowFolder) => show_folder(&dir),
            // Where it is, as text on the clipboard; tests leave the
            // clipboard be.
            (Menu::File(path) | Menu::Project(path), Act::CopyPath) if !cfg!(test) => drop(crate::clipboard::copy_text(&path.to_string_lossy())),
            (Menu::File(path), Act::Trash) => to_trash(&path),
            (Menu::Project(dir), Act::Trash) => {
                to_trash(&dir);
                if self.place == Place::Project(dir) {
                    self.place = Place::Recent;
                }
            }
            (Menu::Project(dir), Act::NewFileIn) => return Outcome::New(dir, None),
            (_, Act::NewFile) => return Outcome::New(self.folder(), None),
            (_, Act::NewProject) => self.new_project(),
            (_, Act::Paste) => return Outcome::Paste(self.folder()),
            _ => {}
        }
        Outcome::Continue
    }

    /// How many pages a file has and what its frames are called, read
    /// once and again when it changes.
    fn facts(&mut self, path: &Path) -> (usize, String) {
        let when = modified(path);
        if !self.facts.get(path).is_some_and(|(made, ..)| *made == when) {
            let pages = std::fs::read(path).ok().and_then(|bytes| studio::read_pages(&bytes)).unwrap_or_default();
            let names: Vec<String> = pages.iter().flat_map(|page| page.frames.iter().map(|frame| frame.name.clone())).collect();
            let frames = match names.len() {
                0 => String::new(),
                1..=2 => names.join(", "),
                count => format!("{}, {}, +{}", names[0], names[1], count - 2),
            };
            self.facts.insert(path.to_owned(), (when, pages.len(), frames));
        }
        let (_, pages, frames) = &self.facts[path];
        (*pages, frames.clone())
    }

    /// The open menu, where it opens, and each of its rows: where it is,
    /// and the row it is.
    fn menu_place(&self) -> Option<(Rect, Vec<(Rect, usize)>)> {
        let (_, at) = self.file_menu.as_ref()?;
        let rows = self.menu_rows();
        let high = rows.iter().map(|row| if let Row::Rule = row { MENU_RULE } else { MENU_ROW }).sum::<i32>() + 12;
        let place = Rect::new(at.0.min(self.width - MENU_W - 8), at.1.min(self.height - high - 8), MENU_W, high);
        let mut y = place.y + 6;
        let mut entries = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            match row {
                Row::Rule => y += MENU_RULE,
                Row::Entry(..) => {
                    entries.push((Rect::new(place.x + 5, y, place.w - 10, MENU_ROW - 2), index));
                    y += MENU_ROW;
                }
            }
        }
        Some((place, entries))
    }

    fn file_menu_place(&self) -> Option<Rect> {
        self.menu_place().map(|(place, _)| place)
    }

    /// The rows of the open menu, over everything else.
    fn layout_file_menu(&self, controls: &mut Vec<(Rect, Control)>) {
        let Some((place, entries)) = self.menu_place() else { return };
        let mut front: Vec<(Rect, Control)> = entries.into_iter().map(|(rect, index)| (rect, Control::FileAct(index as u8))).collect();
        front.push((place, Control::Panel));
        front.append(controls);
        *controls = front;
    }

    /// The open menu drawn.
    fn draw_file_menu(&mut self, canvas: &mut Canvas, hover: Option<&Control>) {
        let Some((place, entries)) = self.menu_place() else { return };
        for grow in (1..=5).rev() {
            canvas.round_fill(place.x - grow, place.y - grow + 3, place.w + 2 * grow, place.h + 2 * grow, 10 + grow, 0x000000, 16);
        }
        canvas.round_fill(place.x, place.y, place.w, place.h, 10, PANEL_COLOR, 256);
        canvas.round_outline(place.x, place.y, place.w, place.h, 10, 1, EDGE_COLOR);
        let rows = self.menu_rows();
        let mut y = place.y + 6;
        for row in &rows {
            if let Row::Rule = row {
                canvas.fill(place.x + 9, y + 4, place.w - 18, 1, EDGE_COLOR);
                y += MENU_RULE;
            } else {
                y += MENU_ROW;
            }
        }
        for (rect, index) in entries {
            let Some(Row::Entry(says, keys, act)) = rows.get(index) else { continue };
            if hover == Some(&Control::FileAct(index as u8)) || self.menu_row == Some(index) {
                canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 6, ACCENT, 256);
            }
            let ink = if *act == Act::Trash { 0xFF6B6B } else { TEXT_COLOR };
            self.text.draw(canvas, rect.x + 10, rect.y + (rect.h - Text::line_height(12)) / 2, 12, ink, says);
            let wide = self.text.width(keys, 11);
            self.text.draw(canvas, rect.right() - 10 - wide, rect.y + (rect.h - Text::line_height(11)) / 2, 11, DIM_TEXT_COLOR, keys);
        }
    }

    /// A file as a row of the list: a small picture of it, its name, how
    /// many pages, its frames, how large, and when it was changed.
    fn draw_file_row(&mut self, canvas: &mut Canvas, rect: Rect, path: &Path, hovered: bool) {
        if hovered {
            canvas.round_fill(rect.x, rect.y, rect.w, rect.h, 8, HOVER_COLOR, 256);
        }
        let thumb = Rect::new(rect.x + 10, rect.y + 6, 64, rect.h - 12);
        canvas.round_fill(thumb.x, thumb.y, thumb.w, thumb.h, 6, 0x262626, 256);
        if let Some((_, w, h, pixels)) = self.thumb(path, thumb.w, thumb.h).cloned() {
            blit(canvas, &pixels, w, h, thumb);
        }
        let middle = rect.y + (rect.h - Text::line_height(13)) / 2;
        let name = match self.renaming_file.clone() {
            Some((known, typed)) if known == *path => self.typed_shown(canvas, (rect.x + 90, middle), 13, typed),
            _ => path.file_stem().map_or(String::new(), |stem| stem.to_string_lossy().into_owned()),
        };
        self.text.draw(canvas, rect.x + 90, middle, 13, TEXT_COLOR, &name);
        let (pages, frames) = self.facts(path);
        let size = std::fs::metadata(path).map_or(0, |meta| meta.len());
        let size = if size >= 1 << 20 { format!("{:.1} MB", size as f64 / (1 << 20) as f64) } else { format!("{} KB", size.div_ceil(1024)) };
        let columns = list_columns(rect);
        let small = rect.y + (rect.h - Text::line_height(12)) / 2;
        let frames: String = frames.chars().take(28).collect();
        for (x, says) in columns.iter().zip([pages.to_string(), frames, size, ago(modified(path))]) {
            self.text.draw(canvas, *x, small, 12, DIM_TEXT_COLOR, &says);
        }
        let more = Rect::new(rect.right() - 36, rect.y + (rect.h - 26) / 2, 26, 26);
        self.text.draw_centered(canvas, more.x + 13, more.y + 13, 12, if hovered { TEXT_COLOR } else { DIM_TEXT_COLOR }, "\u{2022}\u{2022}\u{2022}");
    }
}

/// Where the columns of the list begin: pages, frames, size, edited.
fn list_columns(row: Rect) -> [i32; 4] {
    let right = row.right() - 50;
    [right - 470, right - 400, right - 170, right - 90]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_renamed_and_duplicated_from_its_menu() {
        let dir = std::env::temp_dir().join(format!("hotshot-home-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Launch.hotshot");
        std::fs::write(&file, b"HOTSHOT CANVAS 2\n").unwrap();
        rename_file(&file, "Launch post");
        let renamed = dir.join("Launch post.hotshot");
        assert!(renamed.exists() && !file.exists());
        rename_file(&renamed, "  ");
        assert!(renamed.exists(), "nothing typed, nothing renamed");
        let mut home = Home::new(Text::load(), 1200, 800, dir.clone());
        home.file_menu = Some((Menu::File(renamed.clone()), (10, 10)));
        assert_eq!(home.file_act(row(&home, "Duplicate")), Outcome::Continue);
        assert!(dir.join("Launch post copy.hotshot").exists());
        home.file_menu = Some((Menu::File(renamed.clone()), (10, 10)));
        assert_eq!(home.file_act(0), Outcome::Open(renamed.clone()));
        // Renamed from the menu, what is typed takes the old name's place.
        home.file_menu = Some((Menu::File(renamed.clone()), (10, 10)));
        home.file_act(row(&home, "Rename"));
        for letter in "Teaser".chars() {
            home.key(Key::Char(letter), Mods::default());
        }
        home.key(Key::Enter, Mods::default());
        assert!(dir.join("Teaser.hotshot").exists() && !renamed.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_click_off_a_files_menu_closes_it_and_does_nothing_else() {
        let mut home = Home::new(Text::load(), 1200, 800, std::env::temp_dir());
        let file = std::env::temp_dir().join("Nowhere.hotshot");
        // Every place along the side, clicked with the menu open: each only
        // closes it, and the place shown stays.
        for y in (0..800).step_by(20) {
            home.file_menu = Some((Menu::File(file.clone()), (600, 300)));
            home.pointer_moved(40, y);
            assert_eq!(home.pressed(1, 0), Outcome::Continue);
            assert!(home.file_menu.is_none() && home.place == Place::Recent && home.naming.is_none());
        }
        // Inside it, it stays; a right click off any file opens the menu
        // of the place in its stead.
        home.file_menu = Some((Menu::File(file.clone()), (600, 300)));
        home.pointer_moved(602, 302);
        home.pressed(1, 0);
        assert!(home.file_menu.is_some());
        home.pointer_moved(40, 790);
        home.pressed(3, 0);
        assert!(matches!(home.file_menu, Some((Menu::Empty, _))));
        home.key(Key::Escape, Mods::default());
        assert!(home.file_menu.is_none());
    }

    /// Where the row of the open menu that says something is.
    fn row(home: &Home, says: &str) -> u8 {
        home.menu_rows().iter().position(|row| matches!(row, Row::Entry(known, ..) if known == says)).unwrap_or_else(|| panic!("no {says}")) as u8
    }

    #[test]
    fn a_project_and_the_place_shown_have_menus_of_their_own() {
        let mut home = Home::new(Text::load(), 1200, 800, std::env::temp_dir());
        let project = library().join("Somewhere");
        home.file_menu = Some((Menu::Project(project.clone()), (600, 300)));
        assert_eq!(home.file_act(row(&home, "New file in it")), Outcome::New(project.clone(), None));
        home.file_menu = Some((Menu::Project(project.clone()), (600, 300)));
        home.file_act(row(&home, "Open"));
        assert_eq!((home.place.clone(), home.file_menu.is_none()), (Place::Project(project), true));
        // A right click where there is nothing: a new file, or a picture
        // pasted, in the place shown.
        home.pointer_moved(900, 700);
        home.pressed(3, 0);
        assert!(matches!(home.file_menu, Some((Menu::Empty, _))));
        assert_eq!(home.file_act(row(&home, "Paste a picture")), Outcome::Paste(home.folder()));
        // A file not in Drafts can be moved there.
        home.file_menu = Some((Menu::File(std::env::temp_dir().join("Elsewhere.hotshot")), (600, 300)));
        row(&home, "Move to Drafts");
        assert_eq!(home.key(Key::Enter, Mods::default()), Outcome::Open(std::env::temp_dir().join("Elsewhere.hotshot")));
    }

    #[test]
    fn a_long_path_is_kept_to_its_room_by_leaving_out_its_middle() {
        let mut text = Text::load();
        assert_eq!(middle_out(&mut text, "~/Pictures/hotshot/", 11, 400), "~/Pictures/hotshot/");
        let long = "/tmp/claude-1000/-home-someone/b500279e-ad30-5e62-b927-32199aa2916e/scratchpad/zen/library/";
        let short = middle_out(&mut text, long, 11, SIDE - 40);
        assert!(text.width(&short, 11) <= SIDE - 40 && short.contains('\u{2026}'));
        assert!(short.starts_with("/tmp/claude") && short.ends_with("library/"), "{short}");
        assert_eq!(middle_out(&mut text, long, 11, 1), "\u{2026}");
    }

    #[test]
    fn a_menu_of_the_projects_is_gone_through_with_the_arrow_keys() {
        let mut home = Home::new(Text::load(), 1200, 800, std::env::temp_dir());
        let none = Mods::default();
        home.pointer_moved(900, 700);
        home.pressed(3, 0);
        // Down to the first row, Down past the rule, Up round to the last.
        home.key(Key::Down, none);
        assert_eq!(home.menu_row, Some(usize::from(row(&home, "New file"))));
        home.key(Key::Up, none);
        assert_eq!(home.menu_row, Some(usize::from(row(&home, "Paste a picture"))));
        home.key(Key::Up, none);
        assert_eq!(home.menu_row, Some(usize::from(row(&home, "New project"))));
        // A letter does nothing; Enter does what the row says.
        home.key(Key::Char('x'), none);
        assert!(home.file_menu.is_some());
        home.key(Key::Down, none);
        assert_eq!(home.key(Key::Enter, none), Outcome::Paste(home.folder()));
        assert!(home.file_menu.is_none() && home.menu_row.is_none());
        // A file's menu, and a project's, copy where it is.
        for menu in [Menu::File(std::env::temp_dir().join("Here.hotshot")), Menu::Project(library().join("Somewhere"))] {
            home.file_menu = Some((menu, (600, 300)));
            assert_eq!(home.file_act(row(&home, "Copy path")), Outcome::Continue);
            assert!(home.file_menu.is_none());
        }
    }
}
