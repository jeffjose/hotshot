//! Many captures converted at once, from the projects: each written beside
//! its original with the new ending, on a thread of its own, the bottom of
//! the window counting them through, "9 of 14 · 38 MB → 11 MB"; with
//! "replace" ticked, each original moved to the library's trash after.
//! Undo takes away what was written and brings the originals back.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::convert::{self, Settings};
use crate::output::Format;

/// What the thread says of each file as it goes.
enum Step {
    Converted { from: u64, to: PathBuf, size: u64, trashed: Option<(PathBuf, PathBuf)> },
    Failed(String),
}

/// A conversion under way, or done and still to be undone.
pub(super) struct Batch {
    told: mpsc::Receiver<Step>,
    format: Format,
    total: usize,
    done: usize,
    failed: usize,
    before: u64,
    after: u64,
    written: Vec<PathBuf>,
    /// Each original moved to the trash, and where to.
    trashed: Vec<(PathBuf, PathBuf)>,
    finished: bool,
    /// The last thing that went wrong, to be said.
    wrong: Option<String>,
}

impl Batch {
    /// Each of `paths` converted to `format`, beside it, on a thread; and
    /// with `replace`, each original put in the trash once it is.
    pub(super) fn start(paths: Vec<PathBuf>, format: Format, settings: Settings, replace: bool) -> Batch {
        let (tell, told) = mpsc::channel();
        let total = paths.len();
        std::thread::spawn(move || {
            for path in paths {
                let from = convert::size(&path);
                let step = match convert::convert(&path, format, &settings) {
                    Ok((to, _)) => {
                        let size = convert::size(&to);
                        let trashed = replace.then(|| to_trash(&path).map(|bin| (path.clone(), bin))).flatten();
                        Step::Converted { from, to, size, trashed }
                    }
                    Err(error) => Step::Failed(error),
                };
                if tell.send(step).is_err() {
                    return;
                }
            }
        });
        Batch { told, format, total, done: 0, failed: 0, before: 0, after: 0, written: Vec::new(), trashed: Vec::new(), finished: total == 0, wrong: None }
    }

    /// What has come back, kept; whether there was any.
    fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok(step) = self.told.try_recv() {
            any = true;
            self.done += 1;
            match step {
                Step::Converted { from, to, size, trashed } => {
                    (self.before, self.after) = (self.before + from, self.after + size);
                    self.written.push(to);
                    self.trashed.extend(trashed);
                }
                Step::Failed(error) => {
                    self.failed += 1;
                    self.wrong = Some(error);
                }
            }
        }
        self.finished = self.done >= self.total;
        any
    }

    /// What the bottom of the window says of it.
    pub(super) fn words(&self) -> String {
        let sizes = if self.before > 0 { format!(" \u{b7} {} \u{2192} {}", convert::size_words(self.before as usize), convert::size_words(self.after as usize)) } else { String::new() };
        let failed = match (&self.wrong, self.failed) {
            (Some(wrong), 1) => format!(" \u{b7} 1 not: {wrong}"),
            (_, 0) => String::new(),
            (_, count) => format!(" \u{b7} {count} not"),
        };
        let replaced = if self.trashed.is_empty() { "" } else { ", originals in the trash" };
        match self.finished {
            false => format!("Converting to {} \u{b7} {} of {}{sizes}", self.format.title(), self.done + 1, self.total),
            true => format!("Converted {} to {}{replaced}{sizes}{failed}", self.written.len(), self.format.title()),
        }
    }

    /// Whether it is done and wrote something, to be undone.
    pub(super) fn undoable(&self) -> bool {
        self.finished && !self.written.is_empty()
    }

    /// What was written taken away, and the originals brought back from
    /// the trash.
    pub(super) fn undo(&mut self) -> usize {
        for path in self.written.drain(..) {
            let _ = std::fs::remove_file(path);
        }
        let back = self.trashed.len();
        for (original, bin) in self.trashed.drain(..) {
            if !original.exists() {
                let _ = std::fs::rename(bin, original);
            }
        }
        back
    }
}

impl Home {
    /// The captures picked out, or the one under a right click if it is
    /// not among them.
    pub(super) fn picked_or(&self, path: &Path) -> Vec<PathBuf> {
        if self.picked.iter().any(|known| known == path) {
            self.picked.clone()
        } else {
            vec![path.to_owned()]
        }
    }

    /// A capture clicked with Ctrl, picked out or put back; with Shift,
    /// those from the last picked to it.
    pub(super) fn pick(&mut self, path: PathBuf, range: bool) {
        let all: Vec<PathBuf> = self.captured().into_iter().map(|(path, _)| path).collect();
        match (range, self.picked.last().and_then(|last| all.iter().position(|known| known == last)), all.iter().position(|known| *known == path)) {
            (true, Some(from), Some(to)) => {
                for between in &all[from.min(to)..=from.max(to)] {
                    if !self.picked.contains(between) {
                        self.picked.push(between.clone());
                    }
                }
            }
            _ => match self.picked.iter().position(|known| *known == path) {
                Some(at) => drop(self.picked.remove(at)),
                None => self.picked.push(path),
            },
        }
    }

    /// Captures converted, the note at the bottom counting them through.
    pub(super) fn convert_picked(&mut self, paths: Vec<PathBuf>, format: Format) {
        let format = format.as_set(&self.convert);
        self.batch = Some(Batch::start(paths, format, self.convert, self.replace));
        self.picked.clear();
    }

    /// The window of its own that tries each kind's settings on the
    /// first of them, and converts them all.
    pub(super) fn fine_tune(&mut self, paths: &[PathBuf]) {
        if cfg!(test) {
            return;
        }
        let mut words: Vec<&std::ffi::OsStr> = vec!["--fine-tune".as_ref()];
        words.extend(paths.iter().map(|path| path.as_os_str()));
        if let Err(error) = crate::pin::again(&words) {
            eprintln!("hotshot: cannot open Fine-tune: {error}");
        }
    }

    /// The last conversion undone.
    pub(super) fn undo_batch(&mut self) {
        if let Some(batch) = &mut self.batch {
            let written = batch.written.len();
            let back = batch.undo();
            self.said = Some(match back {
                0 => format!("Took away the {written} converted"),
                back => format!("Took away the {written} converted and brought back {back} from the trash"),
            });
        }
        self.batch = None;
        self.thumbs.clear();
        self.dirty = true;
    }

    /// What the thread has done since last asked; whether to look again
    /// soon.
    pub fn take_batch(&mut self) -> bool {
        let Some(batch) = &mut self.batch else { return false };
        if batch.poll() {
            self.dirty = true;
        }
        !batch.finished
    }

    /// How soon to look again, while a conversion is under way.
    pub fn wait(&self) -> Option<Duration> {
        self.batch.as_ref().filter(|batch| !batch.finished).map(|_| Duration::from_millis(80))
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(home: &Home, says: &str) -> u8 {
        home.menu_rows().iter().position(|row| matches!(row, Row::Entry(known, ..) if known == says)).unwrap_or_else(|| panic!("no {says}")) as u8
    }

    #[test]
    fn captures_picked_out_are_converted_beside_them_and_undone() {
        let dir = std::env::temp_dir().join(format!("hotshot-batch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pixels: Vec<u32> = (0..40 * 30).map(|at| 0xFF00_0000 | at * 2711).collect();
        for name in ["a.png", "b.png", "c.png"] {
            std::fs::write(dir.join(name), crate::output::encode(40, 30, &pixels, false).unwrap()).unwrap();
        }
        let mut home = Home::new(Text::load(), 1200, 800, dir.clone());
        home.place = Place::Captures;
        // Two picked out with Ctrl, the menu of one of them for both.
        home.pick(dir.join("a.png"), false);
        home.pick(dir.join("c.png"), false);
        assert_eq!(home.picked_or(&dir.join("a.png")).len(), 2);
        assert_eq!(home.picked_or(&dir.join("b.png")), vec![dir.join("b.png")]);
        home.file_menu = Some((Menu::Captures(home.picked.clone()), (300, 300)));
        home.file_act(row(&home, "Convert to"));
        assert!(matches!(&home.file_menu, Some((Menu::Convert(paths), _)) if paths.len() == 2));
        home.file_act(row(&home, "JPEG XL lossless"));
        let until = std::time::Instant::now() + Duration::from_secs(30);
        while home.take_batch() && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(dir.join("a.jxl").exists() && dir.join("c.jxl").exists() && !dir.join("b.jxl").exists());
        assert!(home.batch.as_ref().unwrap().words().starts_with("Converted 2 to JPEG XL lossless"), "{}", home.batch.as_ref().unwrap().words());
        assert!(dir.join("a.png").exists(), "originals kept");
        home.undo_batch();
        assert!(!dir.join("a.jxl").exists() && !dir.join("c.jxl").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undone_the_originals_come_back_from_the_trash() {
        let dir = std::env::temp_dir().join(format!("hotshot-batch-trash-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/a.png"), b"original").unwrap();
        std::fs::write(dir.join("a.jxl"), b"converted").unwrap();
        let (_, told) = mpsc::channel();
        let mut batch = Batch { told, format: Format::JxlLossless, total: 1, done: 1, failed: 0, before: 8, after: 9, written: vec![dir.join("a.jxl")], trashed: vec![(dir.join("a.png"), dir.join("bin/a.png"))], finished: true, wrong: None };
        assert!(batch.undoable());
        assert_eq!(batch.undo(), 1);
        assert_eq!(std::fs::read(dir.join("a.png")).unwrap(), b"original");
        assert!(!dir.join("a.jxl").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
