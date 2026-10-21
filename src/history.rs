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
use crate::frame::{self, Backdrop, Frame, Photo, Shadow, BACKDROPS};
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
    /// On a photograph: what tells it from any other, and the name of
    /// the copy of it kept to set the picture on again.
    pub photo: Option<u64>,
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
/// photograph is `photo-` and the name of the copy kept of it, since
/// it was one nobody chose, or one of the user's own that may change.
fn backdrop_word(backdrop: Backdrop, photo: Option<u64>) -> String {
    match (backdrop, photo) {
        (Backdrop::Gradient(which), _) => BACKDROPS.get(which).map_or("none".into(), |(name, _)| name.to_lowercase()),
        (Backdrop::Photo, Some(photo)) => format!("photo-{photo:016x}"),
        _ => "none".into(),
    }
}

/// The folder the copies of the photographs are kept in, beside the
/// file of the list.
fn photos_beside(file: &Path) -> Option<PathBuf> {
    Some(file.parent()?.join("photos"))
}

/// What tells a photograph from any other: FNV-1a over its size and
/// its pixels.
pub fn photo_mark(photo: &Photo) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let size = [photo.width as u32, photo.height as u32];
    for word in size.iter().chain(&photo.pixels) {
        hash = (hash ^ u64::from(*word & 0xFF_FFFF)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The copy kept of the photograph a picture was set on.
pub fn kept_photo(mark: u64) -> Option<Photo> {
    let dir = photos_beside(&file()?)?;
    frame::open(&std::fs::read(dir.join(format!("{mark:016x}.png"))).ok()?)
}

/// What a shadow is called in the file: Tailwind's name for it, or as
/// CSS says it with no spaces in it, or none.
fn shadow_word(shadow: Shadow) -> String {
    match (shadow.on(), shadow.name()) {
        (false, _) => "none".into(),
        (true, Some(name)) => name.into(),
        (true, None) => shadow.css().replace(' ', "_"),
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
            backdrop_word(frame.backdrop, self.photo),
            frame.pad.0,
            frame.pad.1,
            frame.radius,
            shadow_word(frame.shadow),
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
        match frame.backdrop {
            Backdrop::None => {}
            Backdrop::Photo => said.push("on a photograph".into()),
            backdrop => said.push(format!("on {}", backdrop_word(backdrop, None))),
        }
        if frame.pad != (0, 0) {
            said.push(if frame.pad.0 == frame.pad.1 { format!("room {}", frame.pad.0) } else { format!("room {} by {}", frame.pad.0, frame.pad.1) });
        }
        if frame.radius > 0 && frame.backdrop != Backdrop::None {
            said.push(format!("frame corners {}", frame.radius));
        }
        if frame.shadow.on() {
            said.push(format!("shadow {}", frame.shadow.name().map_or_else(|| frame.shadow.css(), str::to_owned)));
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
    let photo = backdrop.strip_prefix("photo-").and_then(|mark| u64::from_str_radix(mark, 16).ok());
    let backdrop = match photo {
        Some(_) => Backdrop::Photo,
        None => frame::named(backdrop).map_or(Backdrop::None, Backdrop::Gradient),
    };
    // A shadow was once on or off, which is what 1 and 0 still say.
    let shadow = match shadow {
        "0" => Shadow::NONE,
        "1" => Shadow::named(frame::SHADOW)?,
        said => Shadow::parse(&said.replace('_', " ")).ok()?,
    };
    let frame = Frame { backdrop, pad: (number(across)?, number(down)?), radius: number(radius)?, shadow };
    let recipe = Recipe { area, corners: corners.parse().ok()?, frame, pointer: pointer == "1", photo };
    Some(Taken { name: name.into(), when: when.parse().ok()?, recipe })
}

/// All of them in a file, the latest first.
fn all_in(file: &Path) -> Vec<Taken> {
    std::fs::read_to_string(file).map(|text| text.lines().filter_map(read_line).collect()).unwrap_or_default()
}

/// Writes one down as taken now, at the top: where it was further
/// down, it is moved up, and the oldest go past the most kept. The
/// photograph it is set on is kept beside the list, and those of the
/// ones gone from it go too.
fn remember_in(file: &Path, recipe: &Recipe, photo: Option<&Photo>, now: u64) {
    let name = recipe.name();
    let mut all: Vec<Taken> = all_in(file).into_iter().filter(|taken| taken.name != name).collect();
    all.insert(0, Taken { name, when: now, recipe: *recipe });
    all.truncate(KEPT);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let (Some(mark), Some(photo), Some(dir)) = (recipe.photo, photo, photos_beside(file)) {
        let copy = dir.join(format!("{mark:016x}.png"));
        if !copy.exists() {
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(png) = output::encode(photo.width as u32, photo.height as u32, &photo.pixels, false) {
                let _ = std::fs::write(copy, png);
            }
        }
    }
    let text: String = all.iter().map(|taken| format!("{} {} {}\n", taken.name, taken.when, taken.recipe.words())).collect();
    let _ = std::fs::write(file, text);
    // The copies no picture in the list is set on any more.
    let wanted: Vec<String> = all.iter().filter_map(|taken| taken.recipe.photo).map(|mark| format!("{mark:016x}.png")).collect();
    if let Some(Ok(copies)) = photos_beside(file).map(std::fs::read_dir) {
        for copy in copies.flatten() {
            if !wanted.iter().any(|name| copy.file_name() == name.as_str()) {
                let _ = std::fs::remove_file(copy.path());
            }
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// Writes down a picture taken just now, and the photograph it is set
/// on, if it is.
pub fn remember(recipe: &Recipe, photo: Option<&Photo>) {
    if let Some(file) = file() {
        remember_in(&file, recipe, photo, now());
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
    output::last_area().map(|area| Recipe { area, corners: 0.0, frame: Frame::default(), pointer: false, photo: None }).map(|recipe| Taken { name: recipe.name(), when: 0, recipe }).into_iter().collect()
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
        Recipe { area: Rect::new(x, 20, 300, 200), corners, frame: Frame { backdrop: Backdrop::Gradient(frame::named("dusk").unwrap()), pad: (64, 40), radius: 12, shadow: Shadow::named("lg").unwrap() }, pointer: true, photo: None }
    }

    #[test]
    fn what_was_taken_is_kept_by_a_name_of_its_own_and_found_by_it_or_by_how_long_ago() {
        let dir = std::env::temp_dir().join(format!("hotshot-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("history");
        assert!(find_in(&all_in(&file), None).is_err());
        let (first, second) = (recipe(10, 16.0), recipe(400, 0.0));
        remember_in(&file, &first, None, 1000);
        remember_in(&file, &second, None, 2000);
        let all = all_in(&file);
        // Read back as it was written, the latest first.
        assert_eq!(all.iter().map(|taken| taken.recipe).collect::<Vec<_>>(), [second, first]);
        assert_eq!((find_in(&all, None).unwrap().recipe, find_in(&all, Some("@2")).unwrap().recipe), (second, first));
        assert_eq!(find_in(&all, Some(&first.name())).unwrap().recipe, first);
        assert!(find_in(&all, Some("@3")).is_err() && find_in(&all, Some("@0")).is_err() && find_in(&all, Some("zzzz")).is_err());
        // The same again is not a second of it, but the first moved up,
        // under the name it had.
        remember_in(&file, &first, None, 3000);
        let all = all_in(&file);
        assert_eq!((all.len(), all[0].name.clone(), all[0].when), (2, first.name(), 3000));
        // Names differ with what they are, and are of four letters.
        assert_ne!(first.name(), second.name());
        assert_eq!(first.name().len(), 4);
        assert_eq!(first.said(), "300 x 200 at 10,20, corners 16, on dusk, room 64 by 40, frame corners 12, shadow lg, pointer");
        // A shadow of its own, said as CSS, is kept as it was said; and
        // one from before shadows had sizes is read as the usual one.
        let own = Recipe { frame: Frame { shadow: Shadow::parse("0 8px 24px -4px rgb(20 30 200 / 0.3)").unwrap(), ..first.frame }, ..first };
        remember_in(&file, &own, None, 5000);
        assert_eq!(all_in(&file)[0].recipe, own);
        std::fs::write(&file, "abcd 1 0 0 10 10 0 none 0 0 0 1 0\n").unwrap();
        assert_eq!(all_in(&file)[0].recipe.frame.shadow, Shadow::named(frame::SHADOW).unwrap());
        // No more are kept than the most there are to be.
        for x in 0..60 {
            remember_in(&file, &recipe(x, 0.0), None, 4000 + x as u64);
        }
        assert_eq!(all_in(&file).len(), KEPT);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_photograph_a_picture_was_set_on_is_kept_while_the_picture_is_in_the_list() {
        let dir = std::env::temp_dir().join(format!("hotshot-photos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("history");
        let photo = Photo { width: 4, height: 3, pixels: (0..12).map(|at| 0xFF00_0000 | at * 0x10_2030).collect() };
        let mark = photo_mark(&photo);
        let on = Recipe { frame: Frame { backdrop: Backdrop::Photo, ..recipe(10, 8.0).frame }, photo: Some(mark), ..recipe(10, 8.0) };
        remember_in(&file, &on, Some(&photo), 1000);
        // Read back on the same photograph, under a name of its own.
        let back = all_in(&file)[0].recipe;
        assert_eq!(back, on);
        assert_ne!(on.name(), Recipe { photo: Some(mark ^ 1), ..on }.name());
        assert_eq!(on.said(), "300 x 200 at 10,20, corners 8, on a photograph, room 64 by 40, frame corners 12, shadow lg, pointer");
        let copy = dir.join("photos").join(format!("{mark:016x}.png"));
        let kept = frame::open(&std::fs::read(&copy).unwrap()).unwrap();
        assert_eq!((kept.width, kept.height, photo_mark(&kept)), (4, 3, mark));
        // Gone from the list, and the copy goes too.
        for x in 0..KEPT as i32 {
            remember_in(&file, &recipe(100 + x, 0.0), None, 2000 + x as u64);
        }
        assert!(!copy.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
