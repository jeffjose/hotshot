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
    path.extension().is_some_and(|ending| matches!(ending.to_string_lossy().to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
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
    dirty: bool,
}

/// How wide the list down the left is.
const SIDE: i32 = 248;

impl Home {
    pub fn new(text: Text, width: i32, height: i32, captures: PathBuf) -> Home {
        Home { text, width, height, place: Place::Recent, pointer: (0, 0), scroll: 0, thumbs: HashMap::new(), naming: None, captures, last_press: (0, (-100, -100)), dirty: true }
    }

    /// The text it draws with, handed back for the canvas to draw with.
    pub fn into_text(self) -> Text {
        self.text
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        (self.width, self.height, self.dirty) = (width.max(400), height.max(300), true);
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn title(&self) -> String {
        format!("{} \u{2014} hotshot", self.heading())
    }

    pub fn cursor(&self) -> Cursor {
        Cursor::Arrow
    }

    /// The projects, the latest changed first.
    fn projects(&self) -> Vec<PathBuf> {
        let mut found: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(library()).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.is_dir() && *path != drafts()).map(|path| {
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
        }
        for (place, control) in self.cards() {
            controls.push((place, control));
        }
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
        if self.place != Place::Captures {
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

    pub fn pressed(&mut self, button: u8, time: u32) -> Outcome {
        self.dirty = true;
        match button {
            4 => {
                self.scroll = (self.scroll - 60).max(0);
                return Outcome::Continue;
            }
            5 => {
                self.scroll += 60;
                return Outcome::Continue;
            }
            1 => {}
            _ => return Outcome::Continue,
        }
        let at = self.pointer;
        let double = time.wrapping_sub(self.last_press.0) < 400 && (at.0 - self.last_press.1 .0).abs().max((at.1 - self.last_press.1 .1).abs()) <= 3;
        self.last_press = (time, at);
        let control = self.layout().into_iter().find(|(rect, _)| rect.contains(at.0, at.1)).map(|(_, control)| control);
        if !matches!(control, Some(Control::Go(Place::Project(_)))) || !double {
            self.commit_name();
        }
        match control {
            Some(Control::Go(Place::Project(dir))) if double => {
                let name = dir.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned());
                self.naming = Some((dir, name));
            }
            Some(Control::Go(place)) => {
                (self.place, self.scroll) = (place, 0);
            }
            Some(Control::NewProject) => {
                let dir = (1..).map(|count| library().join(if count == 1 { "New project".to_owned() } else { format!("New project {count}") })).find(|dir| !dir.exists()).unwrap_or_else(|| library().join("New project"));
                if std::fs::create_dir_all(&dir).is_ok() {
                    self.place = Place::Project(dir.clone());
                    self.naming = Some((dir, String::new()));
                }
            }
            Some(Control::NewFile) => return Outcome::New(self.folder(), None),
            Some(Control::Open(path)) => return Outcome::Open(path),
            Some(Control::Capture(path)) => return Outcome::New(self.folder(), Some(path)),
            Some(Control::Panel) | None => {}
        }
        Outcome::Continue
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
        if let Some((_, typed)) = &mut self.naming {
            match key {
                Key::Char(letter) if !mods.ctrl => typed.push(letter),
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
                            let name = match &self.naming {
                                Some((naming, typed)) if naming == dir => format!("{typed}|"),
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
        let note = library().display().to_string().replace(&std::env::var("HOME").unwrap_or_default(), "~");
        self.text.draw(&mut canvas, 20, height - 44, 11, DIM_TEXT_COLOR, "Projects are folders in");
        self.text.draw(&mut canvas, 20, height - 28, 11, TEXT_COLOR, &format!("{note}/"));
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
                Control::Open(path) => {
                    let thumb = rect.h - 44;
                    canvas.round_fill(rect.x, rect.y, rect.w, thumb, 10, 0x262626, 256);
                    if let Some((_, w, h, pixels)) = self.thumb(path, rect.w, thumb).cloned() {
                        blit(&mut canvas, &pixels, w, h, Rect::new(rect.x, rect.y, rect.w, thumb));
                    }
                    if hovered {
                        canvas.round_outline(rect.x, rect.y, rect.w, thumb, 10, 2, ACCENT);
                    }
                    let name = path.file_stem().map_or(String::new(), |stem| stem.to_string_lossy().into_owned());
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
