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

/// Asks something of the desktop's portal and waits for what comes of
/// it, which may be after the user has been asked: the names and
/// values it answers with. What is asked is some values and then the
/// ways it is to be done, each under a name.
pub fn request(bus: &mut Bus, interface: &str, member: &str, mut asked: Vec<Value>, mut how: Vec<(&str, Value)>) -> dbus::Result<Vec<Value>> {
    // The answer comes as a signal from a place named for whoever
    // asked and a word of their choosing: listened for before asking,
    // so that it cannot come and go unheard.
    static ASKED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let token = format!("hotshot{}n{}", std::process::id(), ASKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
    let from = bus.name.trim_start_matches(':').replace('.', "_");
    let place = format!("{PORTAL_PATH}/request/{from}/{token}");
    let rule = format!("type='signal',interface='org.freedesktop.portal.Request',member='Response',path='{place}'");
    bus.ask(Message::call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "AddMatch", vec![Value::text(&rule)]))?;
    how.push(("handle_token", Value::text(&token)));
    asked.push(Value::named(how));
    let handle = bus.ask(Message::call(PORTAL, PORTAL_PATH, interface, member, asked)).map_err(|error| format!("the desktop's portal would not ({member}): {error}"))?;
    // An older portal answers from a place of its own choosing.
    let place = handle.body.first().and_then(Value::as_str).map_or(place.clone(), str::to_owned);
    let began = Instant::now();
    loop {
        if began.elapsed() > Duration::from_secs(120) {
            return Err(format!("the desktop did not answer ({member})").into());
        }
        let Some(said) = bus.next(Some(Duration::from_secs(1)))? else { continue };
        if said.kind != dbus::SIGNAL || said.member != "Response" || said.path != place {
            continue;
        }
        return match (said.body.first().and_then(Value::as_i32), said.body.get(1)) {
            (Some(0), Some(Value::Array(_, results))) => Ok(results.clone()),
            (Some(0), _) => Ok(Vec::new()),
            (Some(1), _) => Err("it was called off".into()),
            _ => Err(format!("the desktop could not ({member})").into()),
        };
    }
}

/// What the portal answered under a name, among what it answered.
pub fn answered<'a>(results: &'a [Value], wanted: &str) -> Option<&'a Value> {
    results.iter().find_map(|entry| match entry {
        Value::Entry(name, value) if name.as_str() == Some(wanted) => match &**value {
            Value::Variant(inner) => Some(&**inner),
            other => Some(other),
        },
        _ => None,
    })
}

/// Asks the desktop for a picture of the screen, and waits for it: the
/// file it is in. The desktop is asked not to put questions to the
/// user, which some do all the same the first time.
pub fn asked_of_portal() -> dbus::Result<PathBuf> {
    let mut bus = Bus::session()?;
    let results = request(&mut bus, "org.freedesktop.portal.Screenshot", "Screenshot", vec![Value::text("")], vec![("modal", Value::Bool(false)), ("interactive", Value::Bool(false))]).map_err(|error| format!("no picture of the screen: {error}"))?;
    answered(&results, "uri").and_then(Value::as_str).and_then(path_of).ok_or_else(|| "the desktop handed over a picture hotshot cannot find".into())
}

/// The screen being handed over as it changes, for a recording: by
/// which of PipeWire's streams, and where on the whole screen and how
/// large what it shows is, as the desktop counts. The bus is part of
/// it: the desktop stops handing over when whoever asked goes.
pub struct Cast {
    pub bus: Bus,
    pub node: u32,
    pub place: Rect,
}

/// Asks the desktop to hand the screen over as it changes: the monitor
/// an area is on, of those the user lets it. The desktop asks the user
/// which the first time, and is asked to remember, so that it need
/// not again.
pub fn cast(area: Rect, pointer: bool) -> dbus::Result<Cast> {
    const CAST: &str = "org.freedesktop.portal.ScreenCast";
    let mut bus = Bus::session()?;
    let session = format!("hotshot{}", std::process::id());
    let made = request(&mut bus, CAST, "CreateSession", Vec::new(), vec![("session_handle_token", Value::text(&session))])?;
    let handle = Value::Path(answered(&made, "session_handle").and_then(Value::as_str).ok_or("the desktop began no handing over of the screen")?.to_owned());
    // What it remembered of last time, if anything, to ask nothing now.
    let kept = crate::output::state_file("screencast");
    let token = kept.as_ref().and_then(|file| std::fs::read_to_string(file).ok()).map(|token| token.trim().to_owned()).filter(|token| !token.is_empty());
    let mut how = vec![("types", Value::U32(1)), ("multiple", Value::Bool(true)), ("cursor_mode", Value::U32(if pointer { 2 } else { 1 })), ("persist_mode", Value::U32(2))];
    if let Some(token) = &token {
        how.push(("restore_token", Value::text(token)));
    }
    request(&mut bus, CAST, "SelectSources", vec![handle.clone()], how)?;
    let begun = request(&mut bus, CAST, "Start", vec![handle, Value::text("")], Vec::new())?;
    if let (Some(file), Some(token)) = (&kept, answered(&begun, "restore_token").and_then(Value::as_str)) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, token);
    }
    // Each monitor let be seen is a stream: the one the middle of the
    // area is on, or failing that the first.
    let Some(Value::Array(_, streams)) = answered(&begun, "streams") else { return Err("the desktop handed over none of the screen".into()) };
    let pair = |value: Option<&Value>| match value {
        Some(Value::Struct(parts)) => Some((parts.first()?.as_i32()?, parts.get(1)?.as_i32()?)),
        _ => None,
    };
    let known: Vec<(u32, Option<Rect>)> = streams
        .iter()
        .filter_map(|stream| match stream {
            Value::Struct(parts) => {
                let node = parts.first()?.as_i32()? as u32;
                let Some(Value::Array(_, said)) = parts.get(1) else { return Some((node, None)) };
                let place = pair(answered(said, "position")).zip(pair(answered(said, "size"))).map(|((x, y), (wide, high))| Rect::new(x, y, wide, high));
                Some((node, place))
            }
            _ => None,
        })
        .collect();
    let middle = area.center();
    let (node, place) = known.iter().copied().find(|(_, place)| place.is_some_and(|place| place.contains(middle.0, middle.1))).or(known.first().copied()).ok_or("the desktop handed over none of the screen")?;
    Ok(Cast { bus, node, place: place.unwrap_or(area) })
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

/// The screen as hotshot is to show it: the picture the desktop handed
/// over, of every monitor, made the size the display counts it.
pub fn screen(display: &Display) -> Result<Vec<u32>> {
    let (width, height, pixels) = whole()?;
    let all = (display.width, display.height);
    Ok(fitted(&pixels, width, height, all, Rect::new(0, 0, all.0, all.1)))
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

    /// The same way, and for the same reason not run unless asked:
    ///
    ///     dbus-run-session -- cargo test handed_over -- --ignored
    ///
    /// The stand-in is asked the three things it takes to have the
    /// screen handed over as it changes, and answers with two monitors.
    #[test]
    #[ignore]
    fn the_screen_is_handed_over_by_the_monitor_the_area_is_on() {
        let state = std::env::temp_dir().join(format!("hotshot-cast-{}", std::process::id()));
        std::env::set_var("HOTSHOT_STATE", &state);
        let mut desktop = Bus::session().unwrap();
        desktop.ask(Message::call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", vec![Value::text(PORTAL), Value::U32(0)])).unwrap();
        let stand_in = std::thread::spawn(move || {
            let mut heard = Vec::new();
            while heard.len() < 3 {
                let Some(asked) = desktop.next(Some(Duration::from_secs(10))).unwrap() else { panic!("the portal was asked {heard:?} and no more") };
                if asked.kind != dbus::CALL || asked.interface != "org.freedesktop.portal.ScreenCast" {
                    continue;
                }
                let Some(Value::Array(_, options)) = asked.body.last() else { panic!("no options") };
                let token = answered(options, "handle_token").and_then(Value::as_str).unwrap().to_owned();
                let request = format!("{PORTAL_PATH}/request/{}/{token}", asked.sender.trim_start_matches(':').replace('.', "_"));
                let results = match asked.member.as_str() {
                    "CreateSession" => vec![("session_handle", Value::text("/org/freedesktop/portal/desktop/session/x/y"))],
                    // Asked for monitors, more than one, to be remembered.
                    "SelectSources" => {
                        assert_eq!(asked.body.first(), Some(&Value::Path("/org/freedesktop/portal/desktop/session/x/y".into())));
                        assert_eq!((answered(options, "types"), answered(options, "persist_mode"), answered(options, "cursor_mode")), (Some(&Value::U32(1)), Some(&Value::U32(2)), Some(&Value::U32(2))));
                        Vec::new()
                    }
                    "Start" => {
                        let monitor = |node: u32, x: i32| Value::Struct(vec![Value::U32(node), Value::named(vec![("position", Value::Struct(vec![Value::I32(x), Value::I32(0)])), ("size", Value::Struct(vec![Value::I32(1920), Value::I32(1080)]))])]);
                        vec![("streams", Value::Array("(ua{sv})".into(), vec![monitor(41, 0), monitor(42, 1920)])), ("restore_token", Value::text("remembered"))]
                    }
                    other => panic!("asked {other}"),
                };
                heard.push(asked.member.clone());
                desktop.answer(&asked, vec![Value::Path(request.clone())]).unwrap();
                let mut said = Message::signal(&request, "org.freedesktop.portal.Request", "Response", vec![Value::U32(0), Value::named(results)]);
                said.destination = asked.sender.clone();
                desktop.send(&said).unwrap();
            }
            heard
        });
        // An area on the second monitor is recorded by its stream.
        let cast = cast(Rect::new(2000, 100, 300, 200), true).unwrap();
        assert_eq!((cast.node, cast.place), (42, Rect::new(1920, 0, 1920, 1080)));
        assert_eq!(stand_in.join().unwrap(), ["CreateSession", "SelectSources", "Start"]);
        // And what the desktop said to remember is kept for next time.
        assert_eq!(std::fs::read_to_string(state.join("screencast")).unwrap(), "remembered");
        std::fs::remove_dir_all(state).unwrap();
    }
}
