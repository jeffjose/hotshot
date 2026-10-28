//! Fine-tune: jog's Convert panel, the settings of each kind of file
//! tried on a picture before it is written, with what they make of it:
//! its size against a PNG's, how near the original it stays as a
//! SSIMULACRA2 score, and how long it took; and two searches, for the
//! smallest file that still scores 85 and for the best that fits under
//! a size typed. The canvas floats it by an export row and splits the
//! frame itself to compare; the window of its own shows it beside the
//! picture where there is no canvas.
//!
//! Each change is tried on another thread, the latest only: an AVIF of a
//! large picture takes seconds, and settings changed meanwhile make the
//! one under way old news.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use crate::canvas::{Canvas, Rect};
use crate::convert::{self, Effort, Settings, Trial};
use crate::output::Format;
use crate::studio::{ACCENT, DIM_TEXT_COLOR, EDGE_COLOR, FIELD_COLOR, HOVER_COLOR, PANEL_COLOR, TEXT_COLOR};
use crate::text::{icons, Text};

/// How wide the panel is.
pub const WIDTH: i32 = 470;
const PAD: i32 = 14;
const ROW: i32 = 34;
const SAVING: u32 = 0x7EE2A0;

/// The picture tried: as it is, over white as trials are compared with
/// it, and a JPEG's own bytes, which JPEG XL can repack as they are.
pub struct Source {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
    pub alpha: bool,
    pub flat: Vec<u32>,
    pub jpeg: Option<Vec<u8>>,
}

impl Source {
    pub fn new(width: u32, height: u32, pixels: Vec<u32>, jpeg: Option<Vec<u8>>) -> Source {
        let alpha = pixels.iter().any(|pixel| pixel >> 24 != 255);
        let flat = convert::flat(&pixels);
        Source { width, height, pixels, alpha, flat, jpeg }
    }
}

/// The kinds of file across the top, each by the way it was last set.
const FAMILIES: [Format; 5] = [Format::Png, Format::Jxl, Format::Avif, Format::Webp, Format::Jpeg];

/// A setting of a kind of file, on a row of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Knob {
    Mode,
    Quality,
    Effort,
    Speed,
    Color,
    Depth,
    Progressive,
}

/// What a click lands on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Family(usize),
    Choice(usize, usize),
    Slider(usize),
    FitScore,
    FitUnder,
    Reset,
    Save,
    Primary,
    Secondary,
    Close,
    Panel,
}

/// What the panel asks of whoever shows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Asked {
    Nothing,
    Close,
    Primary,
    Secondary,
}

enum Job {
    Try(u64, Format, Settings),
    Fit(u64, Format, Settings, f64),
    Under(u64, Format, Settings, usize),
}

enum Event {
    Tried(u64, Format, Result<Trial, String>),
    Scored(u64, Option<f64>),
    Sized(String, usize),
    Step(u64, usize),
    Fitted(u64, Option<(f32, Trial)>),
}

/// What the panel says along its top and on its two buttons.
pub struct Words {
    pub title: String,
    pub subtitle: String,
    pub primary: &'static str,
    pub secondary: Option<&'static str>,
}

pub struct Tuner {
    pub source: Arc<Source>,
    pub format: Format,
    pub settings: Settings,
    pub words: Words,
    /// The way each kind across the top was last set, lossless or not.
    ways: [Format; 5],
    asked: u64,
    trial: Option<(u64, Format, Result<Trial, String>)>,
    /// How large each kind comes out as it is set, by what it was.
    sizes: HashMap<String, usize>,
    sizing: Vec<String>,
    jobs: mpsc::Sender<Job>,
    sizer: mpsc::Sender<(String, Format, Settings)>,
    events: mpsc::Receiver<Event>,
    /// A search under way: which, and how many trials in.
    fitting: Option<(&'static str, usize)>,
    /// A size being typed for "Fit under…".
    pub typing: Option<String>,
    /// The quality slider being dragged, and where it is.
    dragging: Option<Rect>,
    /// Something said, under the buttons.
    pub said: Option<String>,
    drawn: Vec<(Rect, Hit)>,
    pub hovered: Option<Hit>,
}

impl Tuner {
    /// A panel on a picture, opening on a kind of file written so.
    pub fn new(source: Source, format: Format, settings: Settings, words: Words) -> Tuner {
        let source = Arc::new(source);
        let (jobs, job_inbox) = mpsc::channel::<Job>();
        let (sizer, size_inbox) = mpsc::channel::<(String, Format, Settings)>();
        let (tell, events) = mpsc::channel();
        let mine = source.clone();
        let told = tell.clone();
        let _ = std::thread::Builder::new().name("tune".into()).spawn(move || tries(mine, job_inbox, told));
        let mine = source.clone();
        let _ = std::thread::Builder::new().name("tune-sizes".into()).spawn(move || {
            while let Ok(first) = size_inbox.recv() {
                let mut asked = vec![first];
                asked.extend(size_inbox.try_iter());
                for (key, format, settings) in asked {
                    let source_jpeg = mine.jpeg.as_deref();
                    let Ok(encoded) = convert::encode(mine.width, mine.height, &mine.pixels, mine.alpha, source_jpeg, format, &settings) else { continue };
                    if tell.send(Event::Sized(key, encoded.bytes.len())).is_err() {
                        return;
                    }
                }
            }
        });
        let mut ways = [Format::Png, Format::Jxl, Format::Avif, Format::Webp, Format::Jpeg];
        if let Some(at) = FAMILIES.iter().position(|family| family.family() == format.family()) {
            ways[at] = format;
        }
        let mut tuner = Tuner { source, format, settings, words, ways, asked: 0, trial: None, sizes: HashMap::new(), sizing: Vec::new(), jobs, sizer, events, fitting: None, typing: None, dragging: None, said: None, drawn: Vec::new(), hovered: None };
        tuner.try_now();
        tuner
    }

    /// The settings as they are, tried; and the kinds across the top
    /// sized again where their settings changed.
    fn try_now(&mut self) {
        self.asked += 1;
        let _ = self.jobs.send(Job::Try(self.asked, self.format, self.settings));
        for format in self.ways {
            let key = self.size_key(format);
            if !self.sizes.contains_key(&key) && !self.sizing.contains(&key) {
                self.sizing.push(key.clone());
                let _ = self.sizer.send((key, format, self.settings));
            }
        }
    }

    fn size_key(&self, format: Format) -> String {
        format!("{}:{}", format.word(), self.settings.describe(format))
    }

    fn size_of(&self, format: Format) -> Option<usize> {
        self.sizes.get(&self.size_key(format)).copied()
    }

    /// What the thread has come back with; and whether there was any.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok(event) = self.events.try_recv() {
            any = true;
            match event {
                Event::Tried(asked, format, trial) => {
                    if asked == self.asked {
                        if let Ok(trial) = &trial {
                            self.sizes.insert(self.size_key(format), trial.size);
                        }
                    }
                    if self.trial.as_ref().is_none_or(|(had, ..)| *had <= asked) {
                        self.trial = Some((asked, format, trial));
                    }
                }
                Event::Scored(asked, score) => {
                    if let Some((had, _, Ok(trial))) = &mut self.trial {
                        if *had == asked {
                            trial.score = score;
                        }
                    }
                }
                Event::Sized(key, size) => {
                    self.sizing.retain(|known| *known != key);
                    self.sizes.insert(key, size);
                }
                Event::Step(asked, count) => {
                    if asked == self.asked {
                        if let Some((_, at)) = &mut self.fitting {
                            *at = count;
                        }
                    }
                }
                Event::Fitted(asked, found) => {
                    if asked != self.asked {
                        continue;
                    }
                    let what = self.fitting.take().map_or("", |(what, _)| what);
                    match found {
                        Some((quality, trial)) => {
                            self.settings.set_quality(self.format, quality);
                            self.sizes.insert(self.size_key(self.format), trial.size);
                            self.said = Some(format!("{what}: quality {quality}"));
                            self.trial = Some((asked, self.format, Ok(trial)));
                            self.try_now();
                        }
                        None => self.said = Some(format!("{what}: no quality does it")),
                    }
                }
            }
        }
        any
    }

    /// Whether the thread is at work on something to show.
    pub fn busy(&self) -> bool {
        self.fitting.is_some() || !self.sizing.is_empty() || self.trial.as_ref().is_none_or(|(had, _, trial)| *had != self.asked || trial.as_ref().is_ok_and(|trial| trial.score.is_none() && !self.format.lossless()))
    }

    /// The latest trial decoded again, over white: what the split shows
    /// on the right of its line.
    pub fn preview(&self) -> Option<(&[u32], Format, usize)> {
        match &self.trial {
            Some((_, format, Ok(trial))) => trial.decoded.as_deref().map(|pixels| (pixels, *format, trial.size)),
            _ => None,
        }
    }

    /// What the original is as a PNG, for the split's other side.
    pub fn png_size(&self) -> Option<usize> {
        self.size_of(Format::Png)
    }

    /// The settings of the kind chosen, in a few words, for the split's
    /// label: "AVIF q88".
    pub fn short(&self) -> String {
        match self.settings.quality(self.format) {
            Some(quality) => format!("{} q{}", self.format.family(), quality.round() as i64),
            None => self.format.title().to_owned(),
        }
    }

    fn jpeg_source(&self) -> bool {
        self.source.jpeg.is_some()
    }

    fn knobs(&self) -> Vec<Knob> {
        match self.format {
            Format::Png => Vec::new(),
            Format::Jxl if self.jpeg_source() && self.settings.jxl.recompress_jpeg => vec![Knob::Mode],
            Format::Jxl | Format::Webp => vec![Knob::Mode, Knob::Quality, Knob::Effort],
            Format::JxlLossless | Format::WebpLossless => vec![Knob::Mode, Knob::Effort],
            Format::Avif => vec![Knob::Quality, Knob::Speed, Knob::Color, Knob::Depth],
            Format::Jpeg => vec![Knob::Quality, Knob::Color, Knob::Progressive],
        }
    }

    fn label(knob: Knob, format: Format) -> &'static str {
        match (knob, format) {
            (Knob::Mode, _) => "Mode",
            (Knob::Quality, _) => "Quality",
            (Knob::Effort, _) => "Effort",
            (Knob::Speed, _) => "Speed",
            (Knob::Color, _) => "Color",
            (Knob::Depth, _) => "Bit depth",
            (Knob::Progressive, _) => "Progressive",
        }
    }

    /// The choices on a row, and which is chosen; or none, for the slider.
    fn choices(&self, knob: Knob) -> Option<(Vec<&'static str>, usize, &'static str)> {
        let s = &self.settings;
        let nearest = |values: &[u8], value: u8| values.iter().enumerate().min_by_key(|(_, v)| v.abs_diff(value)).map_or(0, |(at, _)| at);
        Some(match (knob, self.format) {
            (Knob::Mode, Format::Jxl | Format::JxlLossless) if self.jpeg_source() => {
                let chosen = if self.format == Format::JxlLossless { 2 } else if s.jxl.recompress_jpeg { 0 } else { 1 };
                (vec!["Repack the JPEG", "Lossy", "Lossless"], chosen, if chosen == 0 { "nothing lost, smaller" } else { "" })
            }
            (Knob::Mode, format) => (vec!["Lossy", "Lossless"], usize::from(format.lossless()), ""),
            (Knob::Effort, Format::Jxl | Format::JxlLossless) => {
                let chosen = Effort::ALL.iter().position(|effort| *effort == s.jxl.effort).unwrap_or(2);
                let note = match s.jxl.effort {
                    Effort::Medium => "usual",
                    Effort::Slow | Effort::ExtraSlow => "slower, a little smaller",
                    _ => "quicker, larger",
                };
                (vec!["Fastest", "Fast", "Medium", "Slow", "Slowest"], chosen, note)
            }
            (Knob::Effort, _) => {
                let chosen = nearest(&[2, 4, 6], s.webp.method);
                (vec!["Fast", "Middling", "Small"], chosen, if s.webp.method == 4 { "usual" } else { "" })
            }
            (Knob::Speed, _) => {
                let chosen = nearest(&[8, 6, 3], s.avif.speed);
                (vec!["Fast", "Middling", "Small"], chosen, if s.avif.speed == 6 { "usual" } else if s.avif.speed < 4 { "much slower" } else { "" })
            }
            (Knob::Color, Format::Avif) => (vec!["Full 4:4:4"], 0, "as ravif writes it, sharp for text"),
            (Knob::Color, _) => (vec!["Full 4:4:4", "4:2:0"], usize::from(!s.jpeg.full_chroma), if s.jpeg.full_chroma { "keep 4:4:4 for text" } else { "smaller, color edges soften" }),
            (Knob::Depth, _) => (vec!["8", "10"], usize::from(s.avif.depth >= 10), ""),
            (Knob::Progressive, _) => (vec!["On", "Off"], usize::from(!s.jpeg.progressive), ""),
            (Knob::Quality, _) => return None,
        })
    }

    /// A choice picked on a row.
    fn choose(&mut self, knob: Knob, choice: usize) {
        let s = &mut self.settings;
        match (knob, self.format) {
            (Knob::Mode, Format::Jxl | Format::JxlLossless) if self.source.jpeg.is_some() => {
                s.jxl.recompress_jpeg = choice == 0;
                s.jxl.lossless = choice == 2;
                self.format = self.format.with_lossless(choice == 2);
            }
            (Knob::Mode, format) => {
                self.format = format.with_lossless(choice == 1);
                match self.format {
                    Format::Jxl | Format::JxlLossless => s.jxl.lossless = choice == 1,
                    _ => s.webp.lossless = choice == 1,
                }
            }
            (Knob::Effort, Format::Jxl | Format::JxlLossless) => s.jxl.effort = Effort::ALL[choice.min(4)],
            (Knob::Effort, _) => s.webp.method = [2, 4, 6][choice.min(2)],
            (Knob::Speed, _) => s.avif.speed = [8, 6, 3][choice.min(2)],
            (Knob::Color, Format::Jpeg) => s.jpeg.full_chroma = choice == 0,
            (Knob::Depth, _) => s.avif.depth = if choice == 1 { 10 } else { 8 },
            (Knob::Progressive, _) => s.jpeg.progressive = choice == 0,
            _ => return,
        }
        if let Some(at) = FAMILIES.iter().position(|family| family.family() == self.format.family()) {
            self.ways[at] = self.format;
        }
        self.try_now();
    }

    /// A kind across the top chosen, as it was last set.
    pub fn choose_family(&mut self, at: usize) {
        self.format = self.ways[at.min(4)];
        self.typing = None;
        self.try_now();
    }

    /// The quality set by where along the slider `x` is.
    fn slide(&mut self, track: Rect, x: i32) {
        let quality = (((x - track.x) as f32 / track.w.max(1) as f32).clamp(0.0, 1.0) * 100.0).round().max(1.0);
        if self.settings.quality(self.format) != Some(quality) {
            self.settings.set_quality(self.format, quality);
            self.try_now();
        }
    }

    /// The quality moved by a step, or ten with Shift.
    pub fn nudge(&mut self, by: f32) {
        if let Some(quality) = self.settings.quality(self.format) {
            self.settings.set_quality(self.format, (quality + by).clamp(1.0, 100.0));
            self.try_now();
        }
    }

    /// The kind chosen put back to the usual settings.
    pub fn reset(&mut self) {
        let usual = Settings::default();
        match self.format {
            Format::Jxl | Format::JxlLossless => self.settings.jxl = crate::convert::JxlSettings { lossless: self.format == Format::JxlLossless, ..usual.jxl },
            Format::Avif => self.settings.avif = usual.avif,
            Format::Jpeg => self.settings.jpeg = usual.jpeg,
            Format::Webp | Format::WebpLossless => self.settings.webp = crate::convert::WebPSettings { lossless: self.format == Format::WebpLossless, ..usual.webp },
            Format::Png => {}
        }
        self.said = Some(format!("{} back to the usual settings", self.format.family()));
        self.try_now();
    }

    /// These settings kept as the config file's `[convert]` table.
    pub fn save(&mut self) -> Option<Settings> {
        match self.settings.save_as_default() {
            Ok(path) => {
                self.said = Some(format!("Saved as default in {}", crate::studio::shortened(&path)));
                Some(self.settings)
            }
            Err(error) => {
                self.said = Some(format!("Not saved: {error}"));
                None
            }
        }
    }

    /// The smallest file that still scores 85, searched for.
    pub fn fit_score(&mut self) {
        if self.settings.quality(self.format).is_none() {
            self.said = Some("Lossless: every pixel is kept already".into());
            return;
        }
        self.asked += 1;
        self.fitting = Some(("Fit score 85", 0));
        let _ = self.jobs.send(Job::Fit(self.asked, self.format, self.settings, convert::AIM));
    }

    /// The best file under the size typed, searched for.
    pub fn fit_under(&mut self, typed: &str) {
        let Some(most) = convert::parse_size(typed) else {
            self.said = Some(format!("\"{typed}\" is not a size: try 200 KB or 1.5 MB"));
            return;
        };
        if self.settings.quality(self.format).is_none() {
            self.said = Some("Lossless has no quality to fit: choose a lossy kind".into());
            return;
        }
        self.asked += 1;
        self.fitting = Some(("Fit under", 0));
        let _ = self.jobs.send(Job::Under(self.asked, self.format, self.settings, most));
    }

    /// A click; what it asks of whoever shows the panel.
    pub fn press(&mut self, x: i32, y: i32) -> Asked {
        let Some(hit) = self.hit(x, y) else { return Asked::Nothing };
        if self.typing.is_some() && hit != Hit::FitUnder {
            self.typing = None;
        }
        let knobs = self.knobs();
        match hit {
            Hit::Family(at) => self.choose_family(at),
            Hit::Choice(row, choice) => {
                if let Some(&knob) = knobs.get(row) {
                    self.choose(knob, choice);
                }
            }
            Hit::Slider(row) => {
                if let Some(track) = self.drawn.iter().find(|(_, known)| *known == Hit::Slider(row)).map(|(place, _)| *place) {
                    self.dragging = Some(track);
                    self.slide(track, x);
                }
            }
            Hit::FitScore => self.fit_score(),
            Hit::FitUnder => match self.typing.take() {
                Some(typed) if !typed.is_empty() => self.fit_under(&typed),
                Some(_) => {}
                None => self.typing = Some(String::new()),
            },
            Hit::Reset => self.reset(),
            Hit::Save => {
                self.save();
            }
            Hit::Primary => return Asked::Primary,
            Hit::Secondary => return Asked::Secondary,
            Hit::Close => return Asked::Close,
            Hit::Panel => {}
        }
        Asked::Nothing
    }

    /// The pointer moved, with the button down or not.
    pub fn moved(&mut self, x: i32, y: i32) -> bool {
        if let Some(track) = self.dragging {
            self.slide(track, x);
            return true;
        }
        let now = self.hit(x, y);
        let changed = now != self.hovered;
        self.hovered = now;
        changed
    }

    pub fn released(&mut self) {
        self.dragging = None;
    }

    /// A letter typed into "Fit under…", or Enter, Backspace or Esc; true
    /// if it was taken.
    pub fn typed(&mut self, key: &crate::editor::Key) -> bool {
        use crate::editor::Key;
        let Some(typing) = &mut self.typing else { return false };
        match key {
            Key::Char(letter) if letter.is_ascii_alphanumeric() || *letter == '.' || *letter == ' ' => typing.push(*letter),
            Key::Backspace => drop(typing.pop()),
            Key::Enter => {
                let typed = std::mem::take(typing);
                self.typing = None;
                if !typed.trim().is_empty() {
                    self.fit_under(&typed);
                }
            }
            Key::Escape => self.typing = None,
            _ => return false,
        }
        true
    }

    fn hit(&self, x: i32, y: i32) -> Option<Hit> {
        self.drawn.iter().find(|(place, _)| place.contains(x, y)).map(|(_, hit)| *hit)
    }

    /// How tall the panel is, with the rows the kind chosen has.
    pub fn height(&self) -> i32 {
        let rows = self.knobs().len().max(1) as i32;
        PAD + 26 + 52 + rows * ROW + 6 + 58 + 12 + 30 + 8 + 32 + 26 + PAD
    }

    /// The panel drawn into `place`, which is `WIDTH` wide and `height`
    /// tall, and what can be clicked in it kept.
    pub fn draw(&mut self, canvas: &mut Canvas, text: &mut Text, place: Rect) {
        let mut drawn = vec![];
        canvas.round_fill(place.x + 2, place.y + 10, place.w, place.h, 14, 0x000000, 70);
        canvas.round_fill(place.x, place.y, place.w, place.h, 12, PANEL_COLOR, 256);
        canvas.round_outline(place.x, place.y, place.w, place.h, 12, 1, EDGE_COLOR);
        let (left, right) = (place.x + PAD, place.right() - PAD);
        let mut y = place.y + PAD;
        // The heading: what is being tuned, and a cross to close it.
        text.draw_centered(canvas, left + 7, y + 9, 13, TEXT_COLOR, icons::EXPORT);
        text.look.heavy = 1;
        text.draw(canvas, left + 22, y + 9 - Text::line_height(13) / 2, 13, TEXT_COLOR, &self.words.title);
        text.look.heavy = 0;
        let close = Rect::new(right - 20, y - 1, 22, 22);
        let sub_wide = text.width(&self.words.subtitle, 11).min(place.w / 2);
        let subtitle: String = self.words.subtitle.chars().take(40).collect();
        text.draw(canvas, close.x - 10 - sub_wide, y + 9 - Text::line_height(11) / 2, 11, DIM_TEXT_COLOR, &subtitle);
        if self.hovered == Some(Hit::Close) {
            canvas.round_fill(close.x, close.y, close.w, close.h, 6, HOVER_COLOR, 256);
        }
        text.draw_centered(canvas, close.x + 11, close.y + 11, 12, DIM_TEXT_COLOR, icons::CLOSE);
        drawn.push((close, Hit::Close));
        y += 26;
        // The kinds across the top: PNG's size, the one chosen's, and how
        // much smaller than a PNG each of the rest is.
        let png = self.size_of(Format::Png);
        let each = (right - left - 4 * 6) / 5;
        for (at, &way) in self.ways.iter().enumerate() {
            let tile = Rect::new(left + at as i32 * (each + 6), y, each, 40);
            let chosen = way.family() == self.format.family();
            let fill = if chosen { ACCENT } else if self.hovered == Some(Hit::Family(at)) { HOVER_COLOR } else { FIELD_COLOR };
            canvas.round_fill(tile.x, tile.y, tile.w, tile.h, 8, fill, 256);
            text.draw(canvas, tile.x + 8, tile.y + 5, 12, if chosen { 0xFFFFFF } else { TEXT_COLOR }, way.family());
            let size = self.size_of(if chosen { self.format } else { way });
            let (sub, ink) = match (size, png) {
                (Some(size), _) if chosen || way == Format::Png => (convert::size_words(size), if chosen { 0xDCE3FB } else { DIM_TEXT_COLOR }),
                (Some(size), Some(png)) => (convert::change_words(png, size), if size < png { SAVING } else { DIM_TEXT_COLOR }),
                _ => ("\u{2026}".to_owned(), DIM_TEXT_COLOR),
            };
            text.draw(canvas, tile.x + 8, tile.y + 22, 11, ink, &sub);
            drawn.push((tile, Hit::Family(at)));
        }
        y += 52;
        // A row for each setting: choices, or the quality's slider.
        let knobs = self.knobs();
        if knobs.is_empty() {
            text.draw(canvas, left, y + 6, 12, DIM_TEXT_COLOR, "Every pixel kept: PNG has nothing to set");
            y += ROW;
        }
        for (row, &knob) in knobs.iter().enumerate() {
            text.draw(canvas, left, y + (26 - Text::line_height(12)) / 2, 12, DIM_TEXT_COLOR, Tuner::label(knob, self.format));
            let start = left + 84;
            match self.choices(knob) {
                Some((labels, chosen, note)) => {
                    let mut x = start;
                    for (at, label) in labels.iter().enumerate() {
                        let wide = text.width(label, 12) + 18;
                        let chip = Rect::new(x, y, wide, 26);
                        let fill = if at == chosen { 0x4A4A4A } else if self.hovered == Some(Hit::Choice(row, at)) { HOVER_COLOR } else { 0x323232 };
                        canvas.round_fill(chip.x, chip.y, chip.w, chip.h, 6, fill, 256);
                        if at == chosen {
                            canvas.round_outline(chip.x, chip.y, chip.w, chip.h, 6, 1, 0x5A5A5A);
                        }
                        text.draw(canvas, chip.x + 9, chip.y + (26 - Text::line_height(12)) / 2, 12, if at == chosen { TEXT_COLOR } else { DIM_TEXT_COLOR }, label);
                        drawn.push((chip, Hit::Choice(row, at)));
                        x += wide + 4;
                    }
                    if !note.is_empty() {
                        let wide = text.width(note, 11);
                        if x + 8 + wide <= right {
                            text.draw(canvas, right - wide, y + (26 - Text::line_height(11)) / 2, 11, DIM_TEXT_COLOR, note);
                        }
                    }
                }
                None => {
                    // The quality, with a tick where the usual one is.
                    let quality = self.settings.quality(self.format).unwrap_or(0.0);
                    let usual = Settings::default().quality(self.format).unwrap_or(90.0);
                    let track = Rect::new(start + 4, y, right - 96 - start - 4, 26);
                    let at = |value: f32| track.x + (value / 100.0 * track.w as f32).round() as i32;
                    canvas.round_fill(track.x, y + 11, track.w, 4, 2, 0x1E1E1E, 256);
                    canvas.round_fill(track.x, y + 11, at(quality) - track.x, 4, 2, 0x5472E4, 256);
                    canvas.fill(at(usual), y + 5, 1, 16, 0x8F8F8F);
                    canvas.disc(at(quality), y + 13, 7, 0xFFFFFF, 256);
                    let says = if quality == usual { format!("{} \u{b7} usual", quality.round() as i64) } else { format!("{}", quality.round() as i64) };
                    let wide = text.width(&says, 12);
                    text.draw(canvas, right - wide, y + (26 - Text::line_height(12)) / 2, 12, TEXT_COLOR, &says);
                    drawn.push((Rect::new(track.x - 8, track.y, track.w + 16, track.h), Hit::Slider(row)));
                }
            }
            y += ROW;
        }
        y += 6;
        // The numbers: how large against a PNG, how long, how good.
        let numbers = Rect::new(left, y, right - left, 58);
        canvas.round_fill(numbers.x, numbers.y, numbers.w, numbers.h, 10, 0x232323, 256);
        let line = |row: i32| numbers.y + 10 + row * 20;
        match &self.trial {
            Some((_, format, Ok(trial))) => {
                let mut x = numbers.x + 12;
                if let Some(png) = png.filter(|_| *format != Format::Png) {
                    let from = format!("{} \u{2192} ", convert::size_words(png));
                    text.draw(canvas, x, line(0), 12, DIM_TEXT_COLOR, &from);
                    x += text.width(&from, 12);
                }
                let size = convert::size_words(trial.size);
                text.look.heavy = 1;
                text.draw(canvas, x, line(0), 12, TEXT_COLOR, &size);
                text.look.heavy = 0;
                x += text.width(&size, 12) + 8;
                if let Some(png) = png.filter(|_| *format != Format::Png) {
                    text.draw(canvas, x, line(0), 12, if trial.size < png { SAVING } else { 0xF2A65A }, &convert::change_words(png, trial.size));
                }
                let took = format!("{} in {} ms", if trial.how.contains("repacked") { "repacked" } else { "encoded" }, trial.took.as_millis());
                let wide = text.width(&took, 11);
                text.draw(canvas, numbers.right() - 12 - wide, line(0) + 1, 11, DIM_TEXT_COLOR, &took);
                let (score, verdict) = match trial.score {
                    _ if format.lossless() => ("lossless".to_owned(), "every pixel kept".to_owned()),
                    _ if trial.how.contains("repacked") => ("the JPEG as it was".to_owned(), "nothing lost".to_owned()),
                    Some(score) => (format!("{score:.1}"), convert::verdict(score).to_owned()),
                    None => ("\u{2026}".to_owned(), "scoring".to_owned()),
                };
                text.draw(canvas, numbers.x + 12, line(1), 12, DIM_TEXT_COLOR, "SSIMULACRA2");
                let x = numbers.x + 12 + text.width("SSIMULACRA2 ", 12);
                text.look.heavy = 1;
                text.draw(canvas, x, line(1), 12, TEXT_COLOR, &score);
                text.look.heavy = 0;
                let x = x + text.width(&score, 12) + 6;
                text.draw(canvas, x, line(1), 12, DIM_TEXT_COLOR, &format!("\u{b7} {verdict}"));
            }
            Some((_, _, Err(error))) => text.draw(canvas, numbers.x + 12, line(0), 12, 0xF2A65A, &format!("Could not write it: {error}")),
            None => text.draw(canvas, numbers.x + 12, line(0), 12, DIM_TEXT_COLOR, "Trying\u{2026}"),
        }
        y += 58 + 12;
        // The searches, and what is kept.
        let button = |text: &mut Text, canvas: &mut Canvas, x: i32, says: &str, hit: Hit, lit: bool, drawn: &mut Vec<(Rect, Hit)>| -> i32 {
            let wide = text.width(says, 12) + 22;
            let place = Rect::new(x, y, wide, 30);
            let fill = if lit { ACCENT } else if self.hovered == Some(hit) { HOVER_COLOR } else { FIELD_COLOR };
            canvas.round_fill(place.x, place.y, place.w, place.h, 7, fill, 256);
            text.draw(canvas, place.x + 11, place.y + (30 - Text::line_height(12)) / 2, 12, TEXT_COLOR, says);
            drawn.push((place, hit));
            wide
        };
        let mut x = left;
        let fit = match self.fitting {
            Some(("Fit score 85", at)) => format!("Fitting\u{2026} {at} of 7"),
            _ => "Fit score 85".to_owned(),
        };
        x += button(text, canvas, x, &fit, Hit::FitScore, false, &mut drawn) + 6;
        let under = match (&self.typing, self.fitting) {
            (Some(typed), _) => format!("Under {typed}\u{2502}  \u{23ce}"),
            (None, Some(("Fit under", at))) => format!("Fitting\u{2026} {at} of 7"),
            _ => "Fit under\u{2026}".to_owned(),
        };
        button(text, canvas, x, &under, Hit::FitUnder, self.typing.is_some(), &mut drawn);
        let save_wide = text.width("Save as default", 12) + 22;
        let reset_wide = text.width("Reset", 12) + 22;
        button(text, canvas, right - save_wide, "Save as default", Hit::Save, false, &mut drawn);
        button(text, canvas, right - save_wide - 6 - reset_wide, "Reset", Hit::Reset, false, &mut drawn);
        y += 30 + 8;
        // What is done with them: the first button the main one.
        let second = self.words.secondary.map(|says| (says, text.width(says, 12) + 26));
        let primary = Rect::new(left, y, right - left - second.map_or(0, |(_, wide)| wide + 6), 32);
        canvas.round_fill(primary.x, primary.y, primary.w, primary.h, 8, if self.hovered == Some(Hit::Primary) { 0x5472E4 } else { ACCENT }, 256);
        text.draw_centered(canvas, primary.x + primary.w / 2, primary.y + 16, 12, 0xFFFFFF, self.words.primary);
        drawn.push((primary, Hit::Primary));
        if let Some((says, wide)) = second {
            let place = Rect::new(right - wide, y, wide, 32);
            canvas.round_fill(place.x, place.y, place.w, place.h, 8, if self.hovered == Some(Hit::Secondary) { HOVER_COLOR } else { FIELD_COLOR }, 256);
            text.draw_centered(canvas, place.x + wide / 2, place.y + 16, 12, TEXT_COLOR, says);
            drawn.push((place, Hit::Secondary));
        }
        y += 32 + 8;
        let said = self.said.clone().unwrap_or_else(|| format!("{} \u{b7} Esc closes", self.settings.describe(self.format)));
        let said: String = said.chars().take(70).collect();
        text.draw(canvas, left, y, 11, DIM_TEXT_COLOR, &said);
        drawn.push((place, Hit::Panel));
        self.drawn = drawn;
    }
}

/// The thread that tries: the latest asked, the rest forgotten; a score
/// after each, unless something newer has come meanwhile; and the
/// searches, a trial at a time.
fn tries(source: Arc<Source>, inbox: mpsc::Receiver<Job>, tell: mpsc::Sender<Event>) {
    let mut waiting: Option<Job> = None;
    loop {
        let Some(mut job) = waiting.take().or_else(|| inbox.recv().ok()) else { return };
        while let Ok(later) = inbox.try_recv() {
            job = later;
        }
        let jpeg = source.jpeg.as_deref();
        let (w, h) = (source.width, source.height);
        match job {
            Job::Try(asked, format, settings) => {
                let result = convert::trial(w, h, &source.pixels, source.alpha, &source.flat, jpeg, format, &settings, false);
                let scoring = result.as_ref().ok().filter(|trial| !format.lossless() && !trial.how.contains("repacked")).cloned();
                if tell.send(Event::Tried(asked, format, result)).is_err() {
                    return;
                }
                if let Some(trial) = scoring {
                    if let Ok(later) = inbox.try_recv() {
                        waiting = Some(later);
                        continue;
                    }
                    let score = convert::trial_score(w, h, &source.flat, &trial, format);
                    if tell.send(Event::Scored(asked, score)).is_err() {
                        return;
                    }
                }
            }
            Job::Fit(asked, format, settings, aim) => {
                let mut step = |count: usize| tell.send(Event::Step(asked, count)).is_ok();
                let found = convert::fit_score(w, h, &source.pixels, source.alpha, &source.flat, format, &settings, aim, &mut step);
                if tell.send(Event::Fitted(asked, found)).is_err() {
                    return;
                }
            }
            Job::Under(asked, format, settings, most) => {
                let mut step = |count: usize| tell.send(Event::Step(asked, count)).is_ok();
                let found = convert::fit_under(w, h, &source.pixels, source.alpha, &source.flat, format, &settings, most, &mut step);
                if tell.send(Event::Fitted(asked, found)).is_err() {
                    return;
                }
            }
        }
    }
}

/// How long to wait before looking at the thread again, while it works.
pub const POLL: Duration = Duration::from_millis(60);

#[cfg(test)]
mod tests {
    use super::*;

    fn tuner(format: Format) -> Tuner {
        let (w, h) = (96u32, 64u32);
        let pixels = (0..w * h).map(|at| 0xFF00_0000 | (at % w * 2) << 16 | (at / w * 3) << 8 | (at % 7 * 20)).collect();
        let words = Words { title: "Export 1\u{d7}".into(), subtitle: "Frame 1".into(), primary: "Use for this row", secondary: Some("Export now") };
        Tuner::new(Source::new(w, h, pixels, None), format, Settings::default(), words)
    }

    fn settle(tuner: &mut Tuner) {
        let until = std::time::Instant::now() + Duration::from_secs(60);
        while (tuner.busy() || tuner.fitting.is_some()) && std::time::Instant::now() < until {
            tuner.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
        tuner.poll();
    }

    #[test]
    fn a_change_is_tried_and_said_with_its_size_and_score() {
        let mut tuner = tuner(Format::Avif);
        settle(&mut tuner);
        let (pixels, format, size) = tuner.preview().expect("a trial to show");
        assert_eq!((pixels.len(), format), (96 * 64, Format::Avif));
        assert!(size > 0 && tuner.png_size().is_some());
        // Lower, smaller.
        tuner.nudge(-40.0);
        settle(&mut tuner);
        assert!(tuner.preview().unwrap().2 < size);
        // Lossless by its mode, and back.
        tuner.choose_family(1);
        assert_eq!(tuner.format, Format::Jxl);
        tuner.choose(Knob::Mode, 1);
        assert_eq!(tuner.format, Format::JxlLossless);
        settle(&mut tuner);
        assert_eq!(tuner.preview().map(|(pixels, ..)| pixels == &tuner.source.flat[..]), Some(true), "every pixel kept");
    }

    #[test]
    fn fitting_to_a_score_and_under_a_size_sets_the_quality() {
        let mut tuner = tuner(Format::Jpeg);
        tuner.fit_score();
        settle(&mut tuner);
        let fitted = tuner.settings.jpeg.quality;
        assert!(tuner.said.as_deref().is_some_and(|said| said.contains("quality")), "{:?}", tuner.said);
        settle(&mut tuner);
        let score = tuner.trial.as_ref().and_then(|(_, _, trial)| trial.as_ref().ok()?.score);
        assert!(score.is_some_and(|score| score >= convert::AIM), "{fitted}: {score:?}");
        tuner.typing = Some(String::new());
        for letter in "3 kb".chars() {
            tuner.typed(&crate::editor::Key::Char(letter));
        }
        tuner.typed(&crate::editor::Key::Enter);
        settle(&mut tuner);
        assert!(tuner.preview().is_some_and(|(_, _, size)| size <= 3 * 1024) || tuner.said.as_deref().is_some_and(|said| said.contains("no quality")), "{:?}", tuner.said);
    }
}
