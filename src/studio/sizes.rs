//! What the frame comes to in each kind of file, tried for real: the
//! export rows each say what they will write, and the list of kinds
//! what the frame is at its own size in every one of them, so that the
//! choice is made by size and quality rather than by name. Worked out
//! on a thread of its own, the newest frame only, once it has been still
//! a moment; kept by the frame and its change, until it changes again.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::*;
use crate::convert::{self, Settings};
use crate::output::Format;

/// How long the frame is to be still before it is tried.
const STILL: Duration = Duration::from_millis(500);

/// The green a saving is said in.
pub(super) const SAVING: u32 = 0x7EE2A0;

/// One kind of file tried: how large, and how near the original it
/// looks, once that has been worked out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tried {
    pub size: usize,
    pub score: Option<f64>,
}

/// A thing for the thread to try: what it is kept by, at what size of
/// the frame, as which kind, written how, and whether it is scored.
struct Job {
    key: String,
    times: f32,
    format: Format,
    settings: Settings,
    scored: bool,
}

/// What the thread has to do: the frame, and what to try of it. A newer
/// lot replaces an older one not yet done.
#[derive(Default)]
struct Work {
    lot: u64,
    doc: Option<Vec<u8>>,
    jobs: Vec<Job>,
}

type Shared = Arc<(Mutex<Work>, Condvar)>;

/// The sizes known, and the thread that finds them.
pub(super) struct Sizes {
    known: HashMap<String, Tried>,
    shared: Option<(Shared, mpsc::Receiver<(u64, String, Tried)>)>,
    /// The lot handed over last, what it was of, and whether it is done.
    lot: u64,
    asked: String,
    busy: bool,
    /// The frame as it was last seen, and since when it has been so; and
    /// whether there is something to try once it has been still long
    /// enough.
    seen: (String, Instant),
    waiting: bool,
}

impl Default for Sizes {
    fn default() -> Sizes {
        Sizes { known: HashMap::new(), shared: None, lot: 0, asked: String::new(), busy: false, seen: (String::new(), Instant::now()), waiting: false }
    }
}

impl Sizes {
    /// The thread, started the first time it is wanted.
    fn thread(&mut self) -> &Shared {
        if self.shared.is_none() {
            let shared: Shared = Arc::new((Mutex::new(Work::default()), Condvar::new()));
            let (tell, told) = mpsc::channel();
            let theirs = shared.clone();
            let _ = std::thread::Builder::new().name("sizes".into()).spawn(move || work(theirs, tell));
            self.shared = Some((shared, told));
        }
        &self.shared.as_ref().unwrap().0
    }
}

/// The thread: each lot as it comes, a kind at a time, given up for a
/// newer one between any two.
fn work(shared: Shared, tell: mpsc::Sender<(u64, String, Tried)>) {
    let mut text = Text::load();
    loop {
        let (lot, doc, jobs) = {
            let (lock, ready) = &*shared;
            let mut work = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            while work.doc.is_none() {
                work = ready.wait(work).unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            (work.lot, work.doc.take().unwrap(), std::mem::take(&mut work.jobs))
        };
        // Handed over as its file, since what a shape keeps of how it
        // was drawn last stays on the thread that drew it.
        let Some(doc) = read_pages(&doc).and_then(|pages| pages.into_iter().flat_map(|page| page.frames).next()) else { continue };
        let newer = || shared.0.lock().map_or(true, |work| work.lot != lot);
        let mut drawn: Vec<(f32, (i32, i32, Vec<u32>), Vec<u32>)> = Vec::new();
        for job in jobs {
            if newer() {
                break;
            }
            if !drawn.iter().any(|(times, ..)| *times == job.times) {
                let picture = doc.render(job.times, &mut text);
                let flat = convert::flat(&picture.2);
                drawn.push((job.times, picture, flat));
            }
            let (_, (wide, high, pixels), flat) = drawn.iter().find(|(times, ..)| *times == job.times).unwrap();
            let clear = pixels.iter().any(|pixel| pixel >> 24 != 255);
            let Ok(trial) = convert::trial(*wide as u32, *high as u32, pixels, clear, flat, None, job.format, &job.settings, false) else { continue };
            let lossless = job.format.lossless();
            let first = Tried { size: trial.size, score: lossless.then_some(100.0) };
            if tell.send((lot, job.key.clone(), first)).is_err() {
                return;
            }
            if job.scored && !lossless && !newer() {
                let score = convert::trial_score(*wide as u32, *high as u32, flat, &trial, job.format);
                if tell.send((lot, job.key, Tried { size: trial.size, score })).is_err() {
                    return;
                }
            }
        }
        if tell.send((lot, String::new(), Tried { size: 0, score: None })).is_err() {
            return;
        }
    }
}

impl Studio {
    /// How the kinds in the list are tried: as they are written, but AVIF
    /// at speed 8, for the list to fill in quickly; the export itself is
    /// at the speed chosen.
    pub(super) fn list_settings(&self) -> Settings {
        let mut settings = self.convert;
        settings.avif.speed = settings.avif.speed.max(8);
        settings
    }

    /// The frame worked in, and the change it was last changed at.
    fn frame_key(&self) -> String {
        format!("{}:{}:{}", self.page, self.active, self.doc.stamp)
    }

    /// What a size of the frame as a kind of file, written so, is kept by.
    fn size_key(&self, times: f32, format: Format, settings: &Settings) -> String {
        let how = if format == Format::Png { String::new() } else { settings.table_of(format).1 };
        format!("{}|{times}|{}|{how}", self.frame_key(), format.word())
    }

    /// What is known of the frame at its own size as a kind, in the list.
    pub(super) fn listed_size(&self, format: Format) -> Option<Tried> {
        self.sizes.known.get(&self.size_key(1.0, format, &self.list_settings())).copied()
    }

    /// What is known of what an export row writes.
    pub(super) fn row_size(&self, row: &ExportSize) -> Option<Tried> {
        self.sizes.known.get(&self.size_key(row.times, row.format, &row.tuned.unwrap_or(self.convert))).copied()
    }

    /// What the same row would be as a PNG, to say how much smaller it is.
    pub(super) fn row_png(&self, row: &ExportSize) -> Option<Tried> {
        self.sizes.known.get(&self.size_key(row.times, Format::Png, &self.convert)).copied()
    }

    /// The frame's rows and its list tried, if they are not known and the
    /// frame has been still a moment: called as the rows are laid out,
    /// so only while they are shown.
    pub(super) fn want_sizes(&mut self) {
        let key = self.frame_key();
        if self.sizes.seen.0 != key {
            self.sizes.seen = (key.clone(), Instant::now());
        }
        let mut jobs = Vec::new();
        let add = |studio: &Studio, jobs: &mut Vec<Job>, times: f32, format: Format, settings: Settings| {
            let key = studio.size_key(times, format, &settings);
            if !studio.sizes.known.get(&key).is_some_and(|tried| tried.score.is_some() || format == Format::Png) && !jobs.iter().any(|job: &Job| job.key == key) {
                jobs.push(Job { key, times, format, settings, scored: true });
            }
        };
        // The rows first, as they are shown; then the list.
        for row in self.doc.exports.clone() {
            add(self, &mut jobs, row.times, row.format, row.tuned.unwrap_or(self.convert));
            add(self, &mut jobs, row.times, Format::Png, self.convert);
        }
        let listed = self.list_settings();
        for format in Format::ALL {
            add(self, &mut jobs, 1.0, format, listed);
        }
        let asked: String = jobs.iter().map(|job| job.key.as_str()).collect::<Vec<_>>().join(";");
        self.sizes.waiting = false;
        if jobs.is_empty() || asked == self.sizes.asked {
            return;
        }
        if self.sizes.seen.1.elapsed() < STILL {
            self.sizes.waiting = true;
            return;
        }
        self.sizes.asked = asked;
        self.sizes.lot += 1;
        self.sizes.busy = true;
        let (lot, doc) = (self.sizes.lot, self.doc.write());
        let shared = self.sizes.thread().clone();
        let (lock, ready) = &*shared;
        let handed = lock.lock().map(|mut work| *work = Work { lot, doc: Some(doc), jobs });
        if handed.is_ok() {
            ready.notify_one();
        }
    }

    /// What the thread has found since last asked, kept; and whether
    /// there was anything, for the panel to be drawn again.
    pub fn take_sizes(&mut self) -> bool {
        // Still long enough now: laid out again, and so tried.
        if self.sizes.waiting && self.sizes.seen.1.elapsed() >= STILL {
            self.sizes.waiting = false;
            self.dirty = true;
        }
        let Some((_, told)) = &self.sizes.shared else { return false };
        let mut any = false;
        while let Ok((lot, key, tried)) = told.try_recv() {
            // Nothing by nothing says a lot is done.
            if key.is_empty() {
                self.sizes.busy &= lot != self.sizes.lot;
                continue;
            }
            self.sizes.known.insert(key, tried);
            any = true;
        }
        if any {
            self.dirty = true;
        }
        any
    }

    /// How soon to look again: while the thread is at work, often; while
    /// the frame is waiting to be still, when it will be.
    pub fn sizes_wait(&self) -> Option<Duration> {
        match (self.sizes.busy, self.sizes.waiting) {
            (true, _) => Some(Duration::from_millis(80)),
            (false, true) => Some(STILL.saturating_sub(self.sizes.seen.1.elapsed()).max(Duration::from_millis(20))),
            _ => None,
        }
    }

    /// What a kind of file comes to for this frame, as the list says it.
    pub(super) fn listed_words(&self, format: Format) -> String {
        let Some(tried) = self.listed_size(format) else { return "\u{2014}".to_owned() };
        let png = self.listed_size(Format::Png);
        let size = convert::size_words(tried.size);
        match (format, tried.score) {
            (Format::Png, _) => size,
            (_, _) if format.lossless() => match png {
                Some(png) => format!("{size} \u{b7} {}", convert::change_words(png.size, tried.size)),
                None => size,
            },
            (_, Some(score)) => format!("{size} \u{b7} score {}", score.round() as i64),
            (_, None) => format!("{size} \u{b7} \u{2026}"),
        }
    }

    /// The line under an export row: what it writes and how large, and on
    /// the right how much smaller than a PNG it is, or how good.
    pub(super) fn row_words(&self, row: &ExportSize) -> (String, String, u32) {
        let settings = row.tuned.unwrap_or(self.convert);
        let how = match settings.quality(row.format) {
            Some(quality) => format!("Quality {}", quality.round() as i64),
            None => "Lossless".to_owned(),
        };
        let Some(tried) = self.row_size(row) else { return (format!("{how} \u{b7} \u{2026}"), String::new(), DIM_TEXT_COLOR) };
        let left = format!("{how} \u{b7} {}", convert::size_words(tried.size));
        let right = match (row.format, tried.score, self.row_png(row)) {
            (Format::Png, ..) => (String::new(), DIM_TEXT_COLOR),
            (format, _, Some(png)) if format.lossless() => {
                let smaller = tried.size < png.size;
                (format!("{} vs PNG", convert::change_words(png.size, tried.size)), if smaller { SAVING } else { DIM_TEXT_COLOR })
            }
            (_, Some(score), _) => (format!("score {}", score.round() as i64), 0xB4B4B4),
            _ => (String::new(), DIM_TEXT_COLOR),
        };
        (left, right.0, right.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_and_the_list_say_what_the_frame_comes_to() {
        let mut studio = Studio::new(Doc::new(200, 120), Text::load(), 1400, 900, std::env::temp_dir(), "t.png".into());
        studio.doc.exports = vec![ExportSize { times: 1.0, suffix: String::new(), format: Format::JxlLossless, tuned: None }];
        assert_eq!(studio.listed_words(Format::Avif), "\u{2014}", "nothing known yet");
        // Still a moment, then tried, and waited for.
        studio.sizes.seen = (studio.frame_key(), Instant::now() - STILL * 2);
        studio.want_sizes();
        let until = Instant::now() + Duration::from_secs(60);
        while studio.listed_size(Format::Jpeg).and_then(|tried| tried.score).is_none() && Instant::now() < until {
            studio.take_sizes();
            std::thread::sleep(Duration::from_millis(20));
        }
        studio.take_sizes();
        let png = studio.listed_size(Format::Png).expect("a PNG's size");
        let jxl = studio.listed_size(Format::JxlLossless).expect("a lossless JPEG XL's size");
        assert!(png.size > 0 && jxl.score == Some(100.0));
        assert!(studio.listed_words(Format::Jpeg).contains("score"), "{}", studio.listed_words(Format::Jpeg));
        let (left, right, _) = studio.row_words(&studio.doc.exports[0].clone());
        assert!(left.starts_with("Lossless \u{b7} ") && right.ends_with("vs PNG"), "{left} / {right}");
        // Changed, the frame is another, not yet known.
        studio.changed();
        assert_eq!(studio.listed_words(Format::Avif), "\u{2014}");
    }
}
