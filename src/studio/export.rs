//! The panel's Export tab: which frames of the page go out, at what sizes
//! and where; and the window that says what was written, with a small
//! picture of each, and opens their folder.

use super::*;

/// What was written by the last export: each file, a small picture of it,
/// and what it is.
pub(super) struct Exported {
    pub files: Vec<(PathBuf, i32, i32, Vec<u32>, String)>,
    pub dir: PathBuf,
}

/// How large the small pictures of what was written are, at most.
const THUMB: (i32, i32) = (180, 92);

impl Studio {
    /// The frames of the page that go out when the page is exported.
    pub(super) fn export_chosen(&self) -> Vec<usize> {
        (0..self.frame_count()).filter(|index| !self.export_off.contains(index)).collect()
    }

    /// The Export tab: the frames of the page, each to go out or not; the
    /// sizes of the frame worked in; where they go; and the button.
    pub(super) fn export_sections(&mut self, rows: &mut panel::Rows) {
        let page = self.pages[self.page].name.clone();
        let count = self.frame_count();
        rows.section("This page", &format!("{page} \u{b7} {count} {}", if count == 1 { "frame" } else { "frames" }), None);
        for index in 0..self.frame_count() {
            rows.wide(Control::ExportPick(index));
        }
        self.export_sizes(rows);
        rows.section("Where", "", None);
        let dir = shortened(&self.export_dir());
        let dir = if dir.chars().count() > 38 { format!("\u{2026}{}", dir.chars().rev().take(37).collect::<Vec<_>>().into_iter().rev().collect::<String>()) } else { dir };
        rows.note(&dir);
        let count: usize = self.export_chosen().iter().map(|&index| self.frame(index).exports.len().max(1)).sum();
        rows.wide(Control::Action(Action::ExportChosen));
        let _ = count;
    }

    /// How many pictures the chosen frames make.
    pub(super) fn export_count(&self) -> usize {
        self.export_chosen().iter().map(|&index| self.frame(index).exports.len().max(1)).sum()
    }

    /// The frames chosen written, and what was written shown.
    pub(super) fn export_page(&mut self) {
        let chosen = self.export_chosen();
        match self.export_frames(&chosen) {
            Ok(written) => self.show_exported(written),
            Err(error) => self.say(error),
        }
    }

    /// The window of what was written opened, a small picture of each
    /// made from the file.
    pub(super) fn show_exported(&mut self, written: Vec<PathBuf>) {
        let files = written
            .into_iter()
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap_or_default();
                let photo = crate::frame::open(&bytes);
                let (w, h, pixels, size) = match photo {
                    Some(photo) => {
                        let times = (THUMB.0 as f32 / photo.width as f32).min(THUMB.1 as f32 / photo.height as f32).min(1.0);
                        let (tw, th) = (((photo.width as f32 * times) as i32).max(1), ((photo.height as f32 * times) as i32).max(1));
                        let pixels: Vec<u32> = photo.pixels.iter().map(|pixel| pixel | 0xFF00_0000).collect();
                        (tw, th, crate::pin::sized(&pixels, photo.width, photo.height, tw, th), format!("{} \u{d7} {}", photo.width, photo.height))
                    }
                    None => (1, 1, vec![0xFF26_2626], String::new()),
                };
                let kb = bytes.len().div_ceil(1024);
                let weight = if kb >= 1024 { format!("{:.1} MB", kb as f64 / 1024.0) } else { format!("{kb} KB") };
                (path, w, h, pixels, format!("{size} \u{b7} {weight}"))
            })
            .collect();
        self.exported = Some(Exported { files, dir: self.export_dir() });
        self.dirty = true;
    }

    /// Where the window of what was written is.
    pub(super) fn exported_place(&self) -> Option<Rect> {
        let exported = self.exported.as_ref()?;
        let rows = exported.files.len().div_ceil(3).clamp(1, 3) as i32;
        let (wide, high) = (3 * (THUMB.0 + 16) + 32, 52 + rows * (THUMB.1 + 48) + 56);
        let area = self.open_area();
        Some(Rect::new(area.x + (area.w - wide) / 2, area.y + (area.h - high) / 3, wide, high))
    }

    pub(super) fn layout_exported(&mut self, layout: &mut Layout) {
        let Some(place) = self.exported_place() else { return };
        let mut front = vec![
            (Rect::new(place.right() - 16 - 70, place.bottom() - 42, 70, 30), Control::ExportDone(true)),
            (Rect::new(place.right() - 16 - 70 - 8 - 120, place.bottom() - 42, 120, 30), Control::ExportDone(false)),
            (place, Control::Panel),
        ];
        front.append(&mut layout.controls);
        layout.controls = front;
        layout.exported = Some(place);
    }

    /// What was written, a small picture of each and what it is.
    pub(super) fn draw_exported(&mut self, canvas: &mut Canvas, place: Rect) {
        let Some(exported) = self.exported.take() else { return };
        let title = format!("Exported {} {}", exported.files.len(), if exported.files.len() == 1 { "picture" } else { "pictures" });
        self.text.look.heavy = 1;
        self.text.draw(canvas, place.x + 16, place.y + 16, 14, TEXT_COLOR, &title);
        self.text.look.heavy = 0;
        let esc_wide = self.text.width("Esc", 11);
        self.text.draw(canvas, place.right() - 16 - esc_wide, place.y + 18, 11, DIM_TEXT_COLOR, "Esc");
        canvas.fill(place.x + 1, place.y + 44, place.w - 2, 1, EDGE_COLOR);
        for (at, (path, w, h, pixels, meta)) in exported.files.iter().take(9).enumerate() {
            let (column, row) = (at as i32 % 3, at as i32 / 3);
            let cell = Rect::new(place.x + 16 + column * (THUMB.0 + 16), place.y + 56 + row * (THUMB.1 + 48), THUMB.0, THUMB.1);
            canvas.round_fill(cell.x, cell.y, cell.w, cell.h, 8, 0x262626, 256);
            let (ox, oy) = (cell.x + (cell.w - w) / 2, cell.y + (cell.h - h) / 2);
            for y in 0..*h {
                for x in 0..*w {
                    canvas.blend(ox + x, oy + y, pixels[(y * w + x) as usize] & 0xFF_FFFF, 255);
                }
            }
            let name: String = path.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned()).chars().take(26).collect();
            self.text.draw(canvas, cell.x, cell.bottom() + 6, 12, TEXT_COLOR, &name);
            self.text.draw(canvas, cell.x, cell.bottom() + 22, 11, DIM_TEXT_COLOR, meta);
        }
        canvas.fill(place.x + 1, place.bottom() - 54, place.w - 2, 1, EDGE_COLOR);
        self.text.draw_centered(canvas, place.x + 26, place.bottom() - 27, 13, DIM_TEXT_COLOR, icons::FOLDER);
        let dir: String = shortened(&exported.dir).chars().take(44).collect();
        self.text.draw(canvas, place.x + 38, place.bottom() - 27 - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &dir);
        self.exported = Some(exported);
    }

    /// A button of the window of what was written: done, or their folder.
    pub(super) fn press_exported(&mut self, done: bool) {
        if !done {
            if let Some(exported) = &self.exported {
                let _ = std::process::Command::new("xdg-open").arg(&exported.dir).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
            }
        }
        self.exported = None;
        self.dirty = true;
    }
}
