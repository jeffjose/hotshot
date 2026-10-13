//! A Wayland session, where no program may read the screen for itself:
//! it is asked of the desktop, through the portal every desktop keeps
//! for that, which hands back a picture in a file. That is slower than
//! X11's way by the time it takes to write and read a PNG, and the
//! desktop may ask the user first; but it is the one way there is.
//!
//! The window the picture is shown in is still an X11 one, by way of
//! the X server a Wayland session runs for such programs: the editor
//! knows nothing of either.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::canvas::Rect;
use crate::dbus::{self, Bus, Message, Value};
use crate::pin;
use crate::x11::{Display, Result};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// Whether this is a Wayland session. `HOTSHOT_BACKEND` says so itself,
/// as `wayland` or `x11`, for trying one on the other.
pub fn session() -> bool {
    match std::env::var("HOTSHOT_BACKEND").as_deref() {
        Ok("wayland") => true,
        Ok("x11") => false,
        _ => std::env::var_os("WAYLAND_DISPLAY").is_some_and(|name| !name.is_empty()),
    }
}

/// A path out of a link to a file, with what was said by number put
/// back as the letter it is.
fn path_of(link: &str) -> Option<PathBuf> {
    let rest = link.strip_prefix("file://")?;
    let (mut bytes, mut letters) = (Vec::new(), rest.bytes());
    while let Some(byte) = letters.next() {
        bytes.push(match byte {
            b'%' => u8::from_str_radix(std::str::from_utf8(&[letters.next()?, letters.next()?]).ok()?, 16).ok()?,
            other => other,
        });
    }
    Some(PathBuf::from(String::from_utf8(bytes).ok()?))
}

/// Asks the desktop for a picture of the screen, and waits for it: the
/// file it is in. The desktop is asked not to put questions to the
/// user, which some do all the same the first time.
pub fn asked_of_portal() -> dbus::Result<PathBuf> {
    let mut bus = Bus::session()?;
    // The answer comes as a signal from a place named for whoever
    // asked and a word of their choosing: listened for before asking,
    // so that it cannot come and go unheard.
    let token = format!("hotshot{}", std::process::id());
    let from = bus.name.trim_start_matches(':').replace('.', "_");
    let request = format!("{PORTAL_PATH}/request/{from}/{token}");
    let rule = format!("type='signal',interface='org.freedesktop.portal.Request',member='Response',path='{request}'");
    bus.ask(Message::call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "AddMatch", vec![Value::text(&rule)]))?;
    let how = Value::named(vec![("handle_token", Value::text(&token)), ("modal", Value::Bool(false)), ("interactive", Value::Bool(false))]);
    let handle = bus.ask(Message::call(PORTAL, PORTAL_PATH, "org.freedesktop.portal.Screenshot", "Screenshot", vec![Value::text(""), how])).map_err(|error| format!("the desktop's portal would not take a picture of the screen: {error}"))?;
    // An older portal answers from a place of its own choosing.
    let request = handle.body.first().and_then(Value::as_str).map_or(request.clone(), str::to_owned);
    let asked = Instant::now();
    loop {
        let waited = asked.elapsed();
        if waited > Duration::from_secs(90) {
            return Err("the desktop did not hand over a picture of the screen".into());
        }
        let Some(said) = bus.next(Some(Duration::from_secs(1)))? else { continue };
        if said.kind != dbus::SIGNAL || said.member != "Response" || said.path != request {
            continue;
        }
        return match said.body.first().and_then(Value::as_i32) {
            Some(0) => {
                let Some(Value::Array(_, results)) = said.body.get(1) else { return Err("the desktop handed over nothing".into()) };
                let link = results.iter().find_map(|entry| match entry {
                    Value::Entry(name, value) if name.as_str() == Some("uri") => match &**value {
                        Value::Variant(link) => link.as_str().map(str::to_owned),
                        _ => None,
                    },
                    _ => None,
                });
                link.as_deref().and_then(path_of).ok_or_else(|| "the desktop handed over a picture hotshot cannot find".into())
            }
            Some(1) => Err("the picture of the screen was called off".into()),
            _ => Err("the desktop could not take a picture of the screen".into()),
        };
    }
}

/// A picture of the whole screen, as the desktop hands it over: how
/// wide and tall, and its pixels. `HOTSHOT_SCREEN` names a file to be
/// taken for it instead, for trying this with no desktop to ask.
pub fn whole() -> Result<(i32, i32, Vec<u32>)> {
    let (file, ours) = match std::env::var_os("HOTSHOT_SCREEN") {
        Some(file) => (PathBuf::from(file), false),
        None => (asked_of_portal()?, true),
    };
    let bytes = std::fs::read(&file).map_err(|error| format!("cannot read the picture of the screen at {}: {error}", file.display()))?;
    // The file was made for this and nothing else.
    if ours {
        let _ = std::fs::remove_file(&file);
    }
    let photo = crate::frame::open(&bytes).ok_or("the picture of the screen is not one hotshot can read")?;
    Ok((photo.width, photo.height, photo.pixels))
}

/// As much of a picture of the whole screen as one monitor is of it,
/// made the size that monitor is to the display: the picture may be of
/// more pixels than the display counts, on a screen it has made larger.
pub fn fitted(pixels: &[u32], width: i32, height: i32, whole: (i32, i32), monitor: Rect) -> Vec<u32> {
    let (across, down) = (width as f64 / whole.0.max(1) as f64, height as f64 / whole.1.max(1) as f64);
    let part = Rect::new((monitor.x as f64 * across).round() as i32, (monitor.y as f64 * down).round() as i32, (monitor.w as f64 * across).round() as i32, (monitor.h as f64 * down).round() as i32).intersect(Rect::new(0, 0, width, height));
    if part.is_empty() {
        return vec![0; (monitor.w * monitor.h) as usize];
    }
    let cut = crate::canvas::crop(pixels, width, part);
    match (part.w, part.h) == (monitor.w, monitor.h) {
        true => cut,
        false => pin::sized(&cut.iter().map(|pixel| pixel | 0xFF00_0000).collect::<Vec<u32>>(), part.w, part.h, monitor.w, monitor.h).into_iter().map(|pixel| pixel & 0xFF_FFFF).collect(),
    }
}

/// The screen as hotshot is to show it: the monitor the display is
/// kept to, out of the picture the desktop handed over.
pub fn screen(display: &Display) -> Result<Vec<u32>> {
    let (width, height, pixels) = whole()?;
    Ok(fitted(&pixels, width, height, display.whole, Rect::new(display.origin.0, display.origin.1, display.width, display.height)))
}

/// Puts something on the Wayland clipboard, by `wl-copy`, which keeps
/// it there itself. Whether it could: where there is no `wl-copy`, the
/// X11 clipboard is what is left, which a Wayland desktop may or may
/// not pass on.
pub fn copy(bytes: &[u8], kind: &str) -> bool {
    let Ok(mut child) = Command::new("wl-copy").args(["--type", kind]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return false };
    let sent = child.stdin.take().is_some_and(|mut pipe| pipe.write_all(bytes).is_ok());
    sent && child.wait().is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_read_as_the_path_it_is_to() {
        assert_eq!(path_of("file:///home/me/Pictures/Screenshot%20from%202026.png"), Some(PathBuf::from("/home/me/Pictures/Screenshot from 2026.png")));
        assert_eq!((path_of("https://example.com/a.png"), path_of("file:///a%2")), (None, None));
    }

    #[test]
    fn one_monitor_is_cut_out_of_the_whole_and_made_the_size_it_is() {
        // Two monitors side by side, 4 by 2 each; the picture of them at
        // twice the pixels, the left half dark and the right light.
        let (width, height) = (16, 4);
        let pixels: Vec<u32> = (0..width * height).map(|at| if at % width < 8 { 0x101010 } else { 0xE0E0E0 }).collect();
        let right = fitted(&pixels, width, height, (8, 2), Rect::new(4, 0, 4, 2));
        assert_eq!(right, vec![0xE0E0E0; 8]);
        // And at the size it is, cut out and nothing more.
        let same: Vec<u32> = (0..8 * 2).collect();
        assert_eq!(fitted(&same, 8, 2, (8, 2), Rect::new(4, 0, 4, 2)), vec![4, 5, 6, 7, 12, 13, 14, 15]);
    }

    /// Not run unless asked: it wants a session bus with nobody on it
    /// keeping a portal, which is what `dbus-run-session` gives.
    ///
    ///     dbus-run-session -- cargo test portal -- --ignored
    ///
    /// A stand-in for the desktop's portal answers on one connection
    /// to the bus, and is asked for a picture over another.
    #[test]
    #[ignore]
    fn the_portal_is_asked_and_its_answer_waited_for() {
        let file = std::env::temp_dir().join(format!("hotshot portal {}.png", std::process::id()));
        std::fs::write(&file, b"not a picture, but a file").unwrap();
        let link = format!("file://{}", file.to_string_lossy().replace(' ', "%20"));
        let mut desktop = Bus::session().unwrap();
        desktop.ask(Message::call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", vec![Value::text(PORTAL), Value::U32(0)])).unwrap();
        let stand_in = std::thread::spawn(move || loop {
            let Some(asked) = desktop.next(Some(Duration::from_secs(10))).unwrap() else { panic!("nothing was asked of the portal") };
            if asked.kind != dbus::CALL || asked.member != "Screenshot" {
                continue;
            }
            // What it was asked with: no window, and a word to answer by.
            assert_eq!(asked.body.first().and_then(Value::as_str), Some(""));
            let Some(Value::Array(_, options)) = asked.body.get(1) else { panic!("no options") };
            let token = options.iter().find_map(|entry| match entry {
                Value::Entry(name, value) if name.as_str() == Some("handle_token") => match &**value {
                    Value::Variant(token) => token.as_str().map(str::to_owned),
                    _ => None,
                },
                _ => None,
            });
            let request = format!("{PORTAL_PATH}/request/{}/{}", asked.sender.trim_start_matches(':').replace('.', "_"), token.unwrap());
            desktop.answer(&asked, vec![Value::Path(request.clone())]).unwrap();
            let mut said = Message::signal(&request, "org.freedesktop.portal.Request", "Response", vec![Value::U32(0), Value::named(vec![("uri", Value::text(&link))])]);
            said.destination = asked.sender.clone();
            desktop.send(&said).unwrap();
            break;
        });
        assert_eq!(asked_of_portal().unwrap(), file);
        stand_in.join().unwrap();
        std::fs::remove_file(file).unwrap();
    }
}
