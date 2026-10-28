//! Where each file was looked at when it was left: its page, the frame
//! worked in, the zoom, and the point of the page in the middle of the
//! window, to go back to when it is opened again, as Figma does. Kept
//! among hotshot's state in `views` rather than in the file, so that
//! looking round a file never changes it: a line a file, the latest last.

use std::path::{Path, PathBuf};

use super::*;

/// How many files' views are kept; the oldest go first.
const KEPT: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct View {
    pub page: usize,
    pub frame: usize,
    pub zoom: f32,
    /// The point of the page in the middle of where the page is shown,
    /// so that a window of another size still shows what was there.
    pub middle: (f32, f32),
}

fn file() -> Option<PathBuf> {
    crate::output::state_file("views")
}

/// A line of `views`: the numbers first, and the path last, as it may
/// have spaces in it.
fn line(path: &Path, view: View) -> String {
    format!("{} {} {} {} {} {}", view.page, view.frame, view.zoom, view.middle.0, view.middle.1, path.display())
}

fn parse(line: &str) -> Option<(PathBuf, View)> {
    let mut words = line.splitn(6, ' ');
    let mut number = || words.next()?.parse::<f32>().ok().filter(|number| number.is_finite());
    let (page, frame, zoom, x, y) = (number()?, number()?, number()?, number()?, number()?);
    let path = PathBuf::from(words.next()?);
    Some((path, View { page: page as usize, frame: frame as usize, zoom, middle: (x, y) }))
}

/// The view a file was last left in, from the text of `views`.
pub(super) fn recalled(text: &str, path: &Path) -> Option<View> {
    text.lines().filter_map(parse).filter(|(kept, _)| kept == path).map(|(_, view)| view).last()
}

/// The text of `views` with a file's view the latest, and its older one
/// gone.
pub(super) fn remembered(text: &str, path: &Path, view: View) -> String {
    let mut lines: Vec<String> = text.lines().filter(|kept| parse(kept).is_some_and(|(kept, _)| kept != path)).map(str::to_owned).collect();
    lines.push(line(path, view));
    let skip = lines.len().saturating_sub(KEPT);
    lines[skip..].iter().map(|kept| format!("{kept}\n")).collect()
}

impl Studio {
    /// Where the file is being looked at now.
    pub(super) fn view(&self) -> View {
        View { page: self.page, frame: self.active, zoom: self.zoom, middle: self.to_page(self.open_area().center()) }
    }

    /// Shown again as it was: the page, the frame, the zoom and what was
    /// in the middle, as far as the file still has them.
    pub(super) fn restore_view(&mut self, view: View) {
        self.go_to_page(view.page);
        self.activate(view.frame);
        if view.zoom > 0.0 {
            self.zoom = view.zoom.clamp(0.02, 16.0);
            let middle = self.open_area().center();
            self.pan = (middle.0 as f32 + 0.5 - view.middle.0 * self.zoom, middle.1 as f32 + 0.5 - view.middle.1 * self.zoom);
            self.dirty = true;
        }
    }

    /// The view written down as the file is left, for the next time it
    /// is opened. Tests keep theirs to themselves.
    pub fn keep_view(&self) {
        let (Some(path), Some(views)) = (self.path.as_ref(), file()) else { return };
        if cfg!(test) {
            return;
        }
        let text = std::fs::read_to_string(&views).unwrap_or_default();
        if let Some(dir) = views.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&views, remembered(&text, path, self.view()));
    }

    /// Opened as it was last left, if it has been opened before.
    pub fn recall_view(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let text = file().and_then(|views| std::fs::read_to_string(views).ok()).unwrap_or_default();
        if let Some(view) = recalled(&text, &path) {
            self.restore_view(view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_shown_again_where_it_was_left() {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        studio.pages.push(Page { name: "Page 2".into(), frames: vec![Doc::new(400, 300), Doc::new(500, 500)] });
        studio.go_to_page(1);
        studio.activate(1);
        studio.zoom = 3.0;
        studio.pan = (-200.0, -150.0);
        let left = studio.view();
        let path = Path::new("/somewhere/a file.hotshot");
        let text = remembered("", path, left);
        // Another file, and this one again, later: only the latest kept.
        let text = remembered(&text, Path::new("/elsewhere/b.hotshot"), View { page: 0, frame: 0, zoom: 1.0, middle: (0.0, 0.0) });
        let text = remembered(&text, path, left);
        assert_eq!(text.lines().count(), 2);
        assert_eq!(recalled(&text, path), Some(left));
        assert_eq!(recalled(&text, Path::new("/never/opened.hotshot")), None);
        // Opened again, in a window of another size: the same page, frame
        // and zoom, the same point in the middle.
        let mut again = Studio::with_pages(studio.gathered(), Text::load(), 1000, 700, std::env::temp_dir(), "t.png".into());
        again.restore_view(recalled(&text, path).unwrap());
        assert_eq!((again.page, again.active, again.zoom), (1, 1, 3.0));
        let middle = again.to_page(again.open_area().center());
        assert!((middle.0 - left.middle.0).abs() < 1.0 && (middle.1 - left.middle.1).abs() < 1.0, "{middle:?} {:?}", left.middle);
        // A view of pages and frames since gone shows what there is.
        again.restore_view(View { page: 9, frame: 9, zoom: 1.0, middle: (0.0, 0.0) });
        assert_eq!((again.page, again.active), (1, 1));
        // Too many files: the oldest go.
        let mut many = String::new();
        for at in 0..KEPT + 5 {
            many = remembered(&many, Path::new(&format!("/f/{at}.hotshot")), left);
        }
        assert_eq!(many.lines().count(), KEPT);
        assert_eq!(recalled(&many, Path::new("/f/0.hotshot")), None);
    }
}
