//! The pictures taken before, as what it takes to take each again: the
//! area, and how the picture was set out (its corners, its frame, the
//! pointer in it or not), though not what was drawn on it, which was
//! of what was on the screen then. `--again` takes one of them again,
//! the last by itself; `--history` lists them to choose from.
//!
//! Each has a name of four letters made from what it is, which stays
//! its own however many are taken after it: the same area set out the
//! same way is always the same name, and is listed once.

use std::path::{Path, PathBuf};

use crate::canvas::Rect;
use crate::frame::{self, Backdrop, Frame, BACKDROPS};
use crate::output;

/// How many are kept.
const KEPT: usize = 50;

/// What it takes to take a picture again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub area: Rect,
    /// How round the picture's own corners are, in pixels of a screen
    /// 1080 high.
    pub corners: f32,
    pub frame: Frame,
    pub pointer: bool,
}

/// One of the pictures taken before: its name, when it was last taken,
/// in seconds since 1970, and what it takes to take it again.
#[derive(Clone, Debug, PartialEq)]
pub struct Taken {
    pub name: String,
    pub when: u64,
    pub recipe: Recipe,
}

fn file() -> Option<PathBuf> {
    output::state_file("history")
}

/// What a backdrop is called in the file: its name, or none. A
/// photograph is not kept, being one nobody chose, or one of the
/// user's own that may have changed.
fn backdrop_word(backdrop: Backdrop) -> String {
    match backdrop {
        Backdrop::Gradient(which) => BACKDROPS.get(which).map_or("none".into(), |(name, _)| name.to_lowercase()),
        _ => "none".into(),
    }
}

impl Recipe {
    /// As the words of a line of the file, but for its name and when.
    fn words(&self) -> String {
        let (area, frame) = (self.area, self.frame);
        format!(
            "{} {} {} {} {} {} {} {} {} {} {}",
            area.x,
            area.y,
            area.w,
            area.h,
            self.corners,
            backdrop_word(frame.backdrop),
            frame.pad.0,
            frame.pad.1,
            frame.radius,
            u8::from(frame.shadow),
            u8::from(self.pointer)
        )
    }

    /// Its name: four letters out of what it is.
    pub fn name(&self) -> String {
        // FNV-1a, which is short and spreads what it is given well.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.words().bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        // Of letters that are not taken for one another: no l or 1,
        // no o or 0.
        const LETTERS: &[u8; 32] = b"23456789abcdefghijkmnpqrstuvwxyz";
        (0..4).map(|at| LETTERS[(hash >> (at * 5)) as usize & 31] as char).collect()
    }

    /// What it is, said shortly, for the list of them.
    pub fn said(&self) -> String {
        let mut said = vec![format!("{} x {} at {},{}", self.area.w, self.area.h, self.area.x, self.area.y)];
        if self.corners > 0.0 {
            said.push(format!("corners {}", self.corners.round()));
        }
        let frame = self.frame;
        if frame.backdrop != Backdrop::None {
            said.push(format!("on {}", backdrop_word(frame.backdrop)));
        }
        if frame.pad != (0, 0) {
            said.push(if frame.pad.0 == frame.pad.1 { format!("room {}", frame.pad.0) } else { format!("room {} by {}", frame.pad.0, frame.pad.1) });
        }
        if frame.radius > 0 && frame.backdrop != Backdrop::None {
            said.push(format!("frame corners {}", frame.radius));
        }
        if frame.shadow {
            said.push("shadow".into());
        }
        if self.pointer {
            said.push("pointer".into());
        }
        said.join(", ")
    }
}

/// A line of the file, read: or nothing, from one that is not one.
fn read_line(line: &str) -> Option<Taken> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let [name, when, x, y, w, h, corners, backdrop, across, down, radius, shadow, pointer] = words[..] else { return None };
    let number = |word: &str| word.parse::<i32>().ok();
    let area = Rect::new(number(x)?, number(y)?, number(w)?, number(h)?);
    if area.w <= 0 || area.h <= 0 {
        return None;
    }
    let backdrop = frame::named(backdrop).map_or(Backdrop::None, Backdrop::Gradient);
    let frame = Frame { backdrop, pad: (number(across)?, number(down)?), radius: number(radius)?, shadow: shadow == "1" };
    let recipe = Recipe { area, corners: corners.parse().ok()?, frame, pointer: pointer == "1" };
    Some(Taken { name: name.into(), when: when.parse().ok()?, recipe })
}

/// All of them in a file, the latest first.
fn all_in(file: &Path) -> Vec<Taken> {
    std::fs::read_to_string(file).map(|text| text.lines().filter_map(read_line).collect()).unwrap_or_default()
}

/// Writes one down as taken now, at the top: where it was further
/// down, it is moved up, and the oldest go past the most kept.
fn remember_in(file: &Path, recipe: &Recipe, now: u64) {
    let name = recipe.name();
    let mut all: Vec<Taken> = all_in(file).into_iter().filter(|taken| taken.name != name).collect();
    all.insert(0, Taken { name, when: now, recipe: *recipe });
    all.truncate(KEPT);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: String = all.iter().map(|taken| format!("{} {} {}\n", taken.name, taken.when, taken.recipe.words())).collect();
    let _ = std::fs::write(file, text);
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// Writes down a picture taken just now.
pub fn remember(recipe: &Recipe) {
    if let Some(file) = file() {
        remember_in(&file, recipe, now());
    }
}

/// All the pictures taken before, the latest first. Where there are
/// none written down this way, the area the last was of, as hotshot
/// kept it before it kept more than that.
pub fn all() -> Vec<Taken> {
    let all = file().map(|file| all_in(&file)).unwrap_or_default();
    if !all.is_empty() {
        return all;
    }
    output::last_area().map(|area| Recipe { area, corners: 0.0, frame: Frame::default(), pointer: false }).map(|recipe| Taken { name: recipe.name(), when: 0, recipe }).into_iter().collect()
}

/// One of them by how it is asked for: `@1` for the latest, `@2` for
/// the one before, and so on; or by its name; or, asked for by
/// nothing, the latest.
pub fn find_in(all: &[Taken], asked: Option<&str>) -> Result<Taken, String> {
    if all.is_empty() {
        return Err("there is no picture from before to take again".into());
    }
    let asked = asked.map(str::trim).filter(|asked| !asked.is_empty());
    let found = match asked {
        None => all.first(),
        Some(asked) => match asked.strip_prefix('@') {
            Some(count) => count.parse::<usize>().ok().filter(|&count| count >= 1).and_then(|count| all.get(count - 1)),
            None => all.iter().find(|taken| taken.name == asked.to_lowercase()),
        },
    };
    found.cloned().ok_or_else(|| format!("there is no picture '{}' from before; `hotshot --history` lists them", asked.unwrap_or("")))
}

/// When, as the list says it: the time today, the day and time this
/// year, and the date before that.
fn when_said(when: u64, now: u64) -> String {
    if when == 0 {
        return "before".into();
    }
    let at = |seconds: u64, format: &std::ffi::CStr| -> String {
        let mut out = [0u8; 64];
        // SAFETY: the buffers are of the sizes said, and `tm` is
        // filled in before it is read.
        let written = unsafe {
            let time = seconds as libc::time_t;
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::localtime_r(&time, &mut tm);
            libc::strftime(out.as_mut_ptr().cast(), out.len(), format.as_ptr(), &tm)
        };
        String::from_utf8_lossy(&out[..written]).into_owned()
    };
    if at(when, c"%Y-%m-%d") == at(now, c"%Y-%m-%d") {
        at(when, c"today %H:%M")
    } else if at(when, c"%Y") == at(now, c"%Y") {
        at(when, c"%b %e %H:%M")
    } else {
        at(when, c"%Y-%m-%d")
    }
}

/// The list of them, a line each, the latest first: how to ask for
/// each, when it was last taken, and what it is.
pub fn listed(all: &[Taken]) -> String {
    let now = now();
    all.iter().enumerate().map(|(at, taken)| format!("  {:>4}  {}  {:<14}  {}\n", format!("@{}", at + 1), taken.name, when_said(taken.when, now), taken.recipe.said())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(x: i32, corners: f32) -> Recipe {
        Recipe { area: Rect::new(x, 20, 300, 200), corners, frame: Frame { backdrop: Backdrop::Gradient(frame::named("dusk").unwrap()), pad: (64, 40), radius: 12, shadow: true }, pointer: true }
    }

    #[test]
    fn what_was_taken_is_kept_by_a_name_of_its_own_and_found_by_it_or_by_how_long_ago() {
        let file = std::env::temp_dir().join(format!("hotshot-history-{}", std::process::id()));
        let _ = std::fs::remove_file(&file);
        assert!(find_in(&all_in(&file), None).is_err());
        let (first, second) = (recipe(10, 16.0), recipe(400, 0.0));
        remember_in(&file, &first, 1000);
        remember_in(&file, &second, 2000);
        let all = all_in(&file);
        // Read back as it was written, the latest first.
        assert_eq!(all.iter().map(|taken| taken.recipe).collect::<Vec<_>>(), [second, first]);
        assert_eq!((find_in(&all, None).unwrap().recipe, find_in(&all, Some("@2")).unwrap().recipe), (second, first));
        assert_eq!(find_in(&all, Some(&first.name())).unwrap().recipe, first);
        assert!(find_in(&all, Some("@3")).is_err() && find_in(&all, Some("@0")).is_err() && find_in(&all, Some("zzzz")).is_err());
        // The same again is not a second of it, but the first moved up,
        // under the name it had.
        remember_in(&file, &first, 3000);
        let all = all_in(&file);
        assert_eq!((all.len(), all[0].name.clone(), all[0].when), (2, first.name(), 3000));
        // Names differ with what they are, and are of four letters.
        assert_ne!(first.name(), second.name());
        assert_eq!(first.name().len(), 4);
        assert_eq!(first.said(), "300 x 200 at 10,20, corners 16, on dusk, room 64 by 40, frame corners 12, shadow, pointer");
        // No more are kept than the most there are to be.
        for x in 0..60 {
            remember_in(&file, &recipe(x, 0.0), 4000 + x as u64);
        }
        assert_eq!(all_in(&file).len(), KEPT);
        std::fs::remove_file(file).unwrap();
    }
}
