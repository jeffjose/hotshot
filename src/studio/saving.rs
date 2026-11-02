//! The canvas kept in its file without holding up the window: what is in
//! it is taken as it is, which costs next to nothing as its pictures are
//! shared rather than copied, and written out on a thread of its own,
//! beside the file first and then put in its place, so that a file half
//! written is never what is there. One is written at a time: kept again
//! while one is being written, it is written again once that one is done,
//! as it is then. Leaving the file waits for the last to be written.

use super::*;
use std::thread::JoinHandle;

/// The save being written, if one is, and whether another is wanted after.
#[derive(Default)]
pub(super) struct Saving {
    /// The thread writing it, what it is writing to, and whether to say
    /// so when it is done.
    running: Option<(JoinHandle<Result<(), String>>, PathBuf, bool)>,
    /// Kept again while it was written: to write again after it, and
    /// whether to say so.
    again: Option<bool>,
}

/// However the canvas goes, the save being written is waited for, not
/// cut off half written as hotshot ends.
impl Drop for Saving {
    fn drop(&mut self) {
        if let Some((thread, ..)) = self.running.take() {
            let _ = thread.join();
        }
    }
}

/// A file of the canvas written, its lines and then its pictures as
/// `write_pages` has them: beside it first, then put in its place.
fn write_out(lines: &str, images: &[Arc<Image>], path: &Path) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let beside = path.with_extension("hotshot~");
    let written = std::fs::File::create(&beside).and_then(|file| {
        let mut out = std::io::BufWriter::new(file);
        out.write_all(lines.as_bytes())?;
        for image in images {
            writeln!(out, "image {}", image.png.len())?;
            out.write_all(&image.png)?;
            out.write_all(b"\n")?;
        }
        out.into_inner().map_err(|error| error.into_error())?.sync_data()
    });
    written.and_then(|()| std::fs::rename(&beside, path)).map_err(|error| error.to_string())
}

impl Studio {
    /// The canvas kept in its file, or a new one in Drafts, on a thread of
    /// its own; said once it is, when `loud`. Kept again while one is being
    /// written, it is written again once that one is done.
    pub fn save(&mut self, loud: bool) {
        self.end_preview();
        if self.saving.running.is_some() {
            self.saving.again = Some(self.saving.again.unwrap_or(false) || loud);
            return;
        }
        let path = self.path.clone().unwrap_or_else(|| crate::home::fresh_file(&crate::home::drafts(), "Untitled"));
        // Its lines made here, the pictures shared with the thread as they
        // are, already PNGs.
        let (lines, images) = file_lines(&self.gathered());
        let to = path.clone();
        let began = std::time::Instant::now();
        let thread = std::thread::Builder::new().name("hotshot save".into()).spawn(move || {
            let written = write_out(&lines, &images, &to);
            debug!("canvas written in {:.1?}", began.elapsed());
            written
        });
        match thread {
            Ok(thread) => {
                // What changes from now on is behind the file again.
                (self.path, self.edited) = (Some(path.clone()), false);
                self.saving.running = Some((thread, path, loud));
            }
            // No thread to be had: written here, as it is.
            Err(_) => {
                let (lines, images) = file_lines(&self.gathered());
                let written = write_out(&lines, &images, &path);
                self.path = Some(path.clone());
                self.saved(written, &path, loud);
            }
        }
    }

    /// A save written, or not: said, when `loud` or when it could not be.
    fn saved(&mut self, written: Result<(), String>, path: &Path, loud: bool) {
        match written {
            Ok(()) => {
                if loud {
                    self.say(format!("Saved {}", shortened(path)));
                }
                debug!("canvas saved {}", path.display());
            }
            Err(error) => {
                self.say(format!("Could not save: {error}"));
                self.edited = true;
            }
        }
    }

    /// The save being written, heard from once it is done, and the next
    /// begun if it was kept again meanwhile.
    pub fn take_saved(&mut self) {
        if self.saving.running.as_ref().is_some_and(|(thread, ..)| thread.is_finished()) {
            self.finish_save();
        }
    }

    /// How soon to look again whether the save being written is done.
    pub fn save_wait(&self) -> Option<std::time::Duration> {
        self.saving.running.as_ref().map(|_| std::time::Duration::from_millis(20))
    }

    /// The save being written waited for, and any kept again meanwhile
    /// written and waited for too: all of it in the file.
    pub fn wait_saved(&mut self) {
        while self.saving.running.is_some() {
            self.finish_save();
        }
    }

    /// The save being written waited for and heard from, and the next
    /// begun if it was kept again meanwhile.
    fn finish_save(&mut self) {
        let Some((thread, path, loud)) = self.saving.running.take() else { return };
        let written = thread.join().unwrap_or_else(|_| Err("the save went wrong".into()));
        self.saved(written, &path, loud);
        if let Some(loud) = self.saving.again.take() {
            self.save(loud);
        }
    }

    /// Whatever is being typed kept, the view remembered and the file kept,
    /// waiting for it to be written: as the file is left.
    pub fn close(&mut self) {
        self.finish();
        self.keep_view();
        self.save(false);
        self.wait_saved();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grainy(width: i32, height: i32) -> Image {
        let mut seed = 0x9E37_79B9u32;
        Image::new(width, height, (0..width * height).map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            0xFF00_0000 | (seed & 0x00FF_FFFF)
        }).collect())
    }

    #[test]
    fn edits_made_while_a_save_is_written_are_in_the_file_once_it_is_left() {
        let dir = std::env::temp_dir().join(format!("hotshot-saving-{}", std::process::id()));
        let path = dir.join("Kept.hotshot");
        let mut studio = board::tests::board();
        studio.path = Some(path.clone());
        studio.add_picture(grainy(1600, 1000), "grain");
        studio.changed();
        studio.save(false);
        assert!(!studio.edited, "behind the file no more once it is being written");
        // Changed while it is written, and kept again: written after it.
        let mut shape = Shape::new(Tool::Rect, 0x3E63DD, 2.0, (100.0, 100.0));
        shape.points[1] = (200.0, 160.0);
        shape.text = "After".into();
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        assert!(studio.edited);
        studio.save(false);
        // Not heard from yet, as the window has not looked: written after.
        assert!(studio.saving.running.is_some() && studio.saving.again == Some(false));
        // Changed again and left, as quitting does.
        let mut shape = Shape::new(Tool::Rect, 0x3E63DD, 2.0, (300.0, 100.0));
        shape.points[1] = (400.0, 160.0);
        shape.text = "Last".into();
        studio.doc.layers.push(Layer::shape(shape));
        studio.changed();
        studio.save(false);
        studio.wait_saved();
        assert!(studio.saving.running.is_none() && studio.saving.again.is_none() && !studio.edited);
        let pages = read_pages(&std::fs::read(&path).unwrap()).unwrap();
        let words: Vec<String> = pages.iter().flat_map(|page| page.loose.layers.iter()).filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) => Some(shape.text.clone()),
            Kind::Picture(_) => None,
        }).collect();
        assert!(words.contains(&"After".to_owned()) && words.contains(&"Last".to_owned()), "{words:?}");
        assert!(pages.iter().flat_map(|page| page.loose.layers.iter()).any(|layer| matches!(layer.kind, Kind::Picture(_))));
        assert!(!path.with_extension("hotshot~").exists(), "nothing left beside it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_that_cannot_be_written_is_said_and_left_to_do_again() {
        let mut studio = board::tests::board();
        studio.path = Some(PathBuf::from("/proc/hotshot-cannot/Kept.hotshot"));
        studio.changed();
        studio.save(false);
        studio.wait_saved();
        assert!(studio.edited, "still behind the file");
        assert!(studio.note.as_ref().is_some_and(|(said, _)| said.starts_with("Could not save")));
    }
}
