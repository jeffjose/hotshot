//! `hotshot --check`: each thing hotshot does, tried on this machine as
//! it is, and said to have worked or not. The tests `cargo test` runs
//! are of what hotshot works out, and need no display; this is of what
//! it asks of the display, the desktop and the programs beside it,
//! which is where one machine differs from another.
//!
//! It puts a window up for a moment, a picture in a corner of the
//! screen for a moment, and records a second of a corner; with `-c` it
//! puts a picture on the clipboard, which takes the place of what was
//! there. Nothing it makes is kept.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use x11rb::protocol::xinput::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;

use crate::canvas;
use crate::dbus::{Bus, Message, Value};
use crate::options::Options;
use crate::output::{self, Format};
use crate::text::Text;
use crate::wayland;
use crate::x11::Display;

/// How a thing tried came out: it worked, and what was seen of it; it
/// was not tried, and why; or it did not work, and what went wrong.
enum Came {
    Worked(String),
    Left(String),
    Failed(String),
}

use Came::{Failed, Left, Worked};

use crate::record::installed;

/// Starts hotshot again, told something, and waits for it to end: how
/// it ended, or nothing if it had not within a while, when it is ended.
fn again(told: &[&str], within: Duration) -> Option<bool> {
    let mut child = Command::new(std::env::current_exe().ok()?).args(told).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()?;
    let began = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status.success());
        }
        if began.elapsed() > within {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What a program wrote out, given something to read or nothing.
fn said_by(program: &str, told: &[&str]) -> Option<Vec<u8>> {
    let done = Command::new(program).args(told).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    done.status.success().then_some(done.stdout)
}

pub fn run(options: &Options) -> ExitCode {
    println!("hotshot {}, trying what it does on this machine\n", crate::version());
    let wayland = wayland::session();
    let scratch = std::env::temp_dir().join(format!("hotshot-check-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&scratch);
    let in_scratch = |name: &str| -> PathBuf { scratch.join(name) };
    let mut results: Vec<(&str, Came)> = Vec::new();

    // What kind of session this is, and whose.
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "a desktop that does not say what it is".into());
    let shown = std::env::var("DISPLAY").unwrap_or_default();
    results.push((
        "session",
        match (wayland, shown.is_empty()) {
            (true, false) => Worked(format!("Wayland, {desktop}; its X11 side is at {shown}")),
            (true, true) => Failed(format!("Wayland, {desktop}, with no X11 side (XWayland) running, which hotshot shows its window through")),
            (false, false) => Worked(format!("X11, {desktop}, at {shown}")),
            (false, true) => Failed("neither X11 nor Wayland: there is no display to be found".into()),
        },
    ));

    // The display, and the screen read off it.
    let mut display = match Display::open() {
        Ok(mut display) => {
            if wayland {
                display.manage();
            }
            // Each monitor and where it is, which is what goes wrong
            // first where there is more than one.
            let monitors: Vec<String> = display.monitors().iter().map(|monitor| format!("{}x{}+{}+{}", monitor.w, monitor.h, monitor.x, monitor.y)).collect();
            results.push(("display", Worked(format!("{} x {} in all; {}: {}", display.width, display.height, if monitors.len() == 1 { "one monitor".to_owned() } else { format!("{} monitors", monitors.len()) }, monitors.join(", ")))));
            Some(display)
        }
        Err(error) => {
            results.push(("display", Failed(error.to_string())));
            None
        }
    };
    let mut shot: Option<(i32, i32, Vec<u32>)> = None;
    if let Some(display) = display.as_mut() {
        // Under Wayland, how large the picture the desktop hands over
        // is beside how large the display counts the screen: where the
        // two differ, the desktop has made the screen larger, or laid
        // its monitors out another way.
        if wayland {
            results.push((
                "picture size",
                match wayland::whole() {
                    Ok((wide, high, _)) if (wide, high) == (display.width, display.height) => Worked(format!("{wide} x {high}, as the display counts it")),
                    Ok((wide, high, _)) => Worked(format!("{wide} x {high}, where the display counts {} x {}: it is made that size, {:.2} times across and {:.2} down", display.width, display.height, display.width as f64 / wide as f64, display.height as f64 / high as f64)),
                    Err(error) => Failed(error.to_string()),
                },
            ));
        }
        let began = Instant::now();
        results.push((
            "screen read",
            match crate::read_screen(display) {
                Ok(pixels) => {
                    let took = began.elapsed();
                    let pixels: Vec<u32> = pixels.iter().map(|pixel| pixel & 0xFF_FFFF).collect();
                    let one = pixels.iter().all(|&pixel| pixel == pixels[0]);
                    let how = match (wayland, std::env::var_os("HOTSHOT_SCREEN").is_some()) {
                        (true, true) => "taken from the file HOTSHOT_SCREEN names, not asked of the desktop",
                        (true, false) => "handed over by the desktop's portal",
                        (false, _) => "read off the display",
                    };
                    let note = if one { "; it is all one color, as a screen that is off or locked is" } else { "" };
                    shot = Some((display.width, display.height, pixels));
                    Worked(format!("{how} in {took:.0?}{note}"))
                }
                Err(error) => Failed(error.to_string()),
            },
        ));
        if wayland {
            results.push((
                "portal",
                match Bus::session().and_then(|mut bus| {
                    bus.ask(Message::call("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.DBus.Properties", "Get", vec![Value::text("org.freedesktop.portal.Screenshot"), Value::text("version")]))
                }) {
                    Ok(answer) => match answer.body.first() {
                        Some(Value::Variant(version)) => Worked(format!("takes pictures, at version {}", version.as_i32().unwrap_or(0))),
                        _ => Worked("takes pictures".into()),
                    },
                    Err(error) => Failed(format!("the desktop's portal does not take pictures: {error}")),
                },
            ));
        }
    }

    // The window: up for a moment, with the keys, and down again.
    if let Some(display) = display.as_mut() {
        let began = Instant::now();
        let up = match (wayland, &shot) {
            (false, _) => display.freeze(50).map(|frozen| if frozen { "the screen frozen by the display itself" } else { "no way to freeze the screen in the display" }),
            (true, Some((_, _, pixels))) => {
                let mut frame = display.buffer();
                canvas::darken_into(&mut frame, pixels, 128);
                display.show(&frame).map(|()| "a window the desktop manages")
            }
            (true, None) => Err("there is no picture of the screen to show in it".into()),
        };
        let mut keyboard = None;
        let window = (
            "window",
            match up {
                Ok(how) => {
                    let took = began.elapsed();
                    let _ = display.prepare();
                    // The keys: taken, on X11; given, under Wayland,
                    // which is seen by the display saying whose they are.
                    let keys = match wayland {
                        false => display.grab(),
                        true => {
                            std::thread::sleep(Duration::from_millis(300));
                            display.conn.get_input_focus().ok().and_then(|cookie| cookie.reply().ok()).is_some_and(|focus| focus.focus != x11rb::NONE && focus.focus != display.root && focus.focus != 1)
                        }
                    };
                    std::thread::sleep(Duration::from_millis(150));
                    display.hide();
                    keyboard = Some(if keys { Worked(if wayland { "the window was given the keys".into() } else { "taken for the window".into() }) } else { Failed("the window did not get the keys: Esc, Enter and the tools' letters would do nothing".into()) });
                    Worked(format!("{how}, up in {took:.0?}"))
                }
                Err(error) => Failed(error.to_string()),
            },
        );
        results.push(window);
        results.extend(keyboard.map(|came| ("keyboard", came)));
    }

    // The letters and the icons, which hotshot carries.
    let mut text = Text::load();
    results.push(("fonts", if text.width("hotshot", 16) > 0 && text.width(crate::text::icons::PIN, 16) > 0 { Worked("its own letters and icons".into()) } else { Failed("no letters to draw with".into()) }));

    // A picture packed each way, and written.
    let picture = shot.clone().unwrap_or_else(|| (64, 64, (0..64 * 64).map(|at| at * 0x010305).collect()));
    let mut packed = Vec::new();
    let mut wrong = None;
    for format in [Format::Png, Format::Jpeg, Format::Webp] {
        let began = Instant::now();
        match output::pack(format, options.quality, picture.0 as u32, picture.1 as u32, &picture.2, false) {
            Ok(bytes) => {
                let file = in_scratch(&format!("screen.{}", format.ending()));
                match std::fs::write(&file, &bytes) {
                    Ok(()) => packed.push(format!("{} {:.1} MB in {:.0?}", format.ending(), bytes.len() as f64 / 1e6, began.elapsed())),
                    Err(error) => wrong = Some(format!("cannot write {}: {error}", file.display())),
                }
            }
            Err(error) => wrong = Some(format!("{}: {error}", format.ending())),
        }
    }
    results.push(("saving", wrong.map_or_else(|| Worked(packed.join(", ")), Failed)));
    let dir = options.places.first().unwrap_or(&options.dir);
    results.push(("save folder", if dir.is_dir() || dir.parent().is_some_and(Path::is_dir) { Worked(dir.display().to_string()) } else { Failed(format!("{} is not there, nor the folder it would be made in", dir.display())) }));

    // The clipboard, which takes the place of what was on it: only asked.
    let png = in_scratch("screen.png");
    results.push((
        "clipboard",
        if !options.copy {
            Left("not tried, as it takes the place of what is on the clipboard: add -c to try it".into())
        } else if wayland && installed("wl-copy") {
            let bytes = std::fs::read(&png).unwrap_or_default();
            match wayland::copy(&bytes, "image/png") {
                true => match said_by("wl-paste", &["--type", "image/png"]) {
                    Some(back) if back == bytes => Worked("by wl-copy, and read back the same".into()),
                    Some(back) => Failed(format!("by wl-copy, but {} bytes came back of {}", back.len(), bytes.len())),
                    None => Worked("by wl-copy; not read back, there being no wl-paste".into()),
                },
                false => Failed("wl-copy would not take the picture".into()),
            }
        } else {
            let kept = in_scratch("clip.png");
            let _ = std::fs::copy(&png, &kept);
            let bytes = std::fs::read(&png).unwrap_or_default();
            match again(&["--clip", &kept.to_string_lossy()], Duration::from_secs(5)) {
                Some(true) => {
                    std::thread::sleep(Duration::from_millis(200));
                    let through = if wayland { "by X11's clipboard, there being no wl-copy; whether the desktop passes it on is the desktop's affair" } else { "by X11's clipboard" };
                    match said_by("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"]) {
                        Some(back) if back == bytes => Worked(format!("{through}, and read back the same")),
                        Some(back) => Failed(format!("{through}, but {} bytes came back of {}", back.len(), bytes.len())),
                        None => Worked(format!("{through}; not read back, there being no xclip")),
                    }
                }
                _ => Failed("the picture could not be put on the clipboard".into()),
            }
        },
    ));

    // The mouse pointer, and the keys pressed anywhere: X11's to say.
    if let Some(display) = display.as_ref() {
        results.push((
            "pointer",
            match (wayland, display.pointer_picture()) {
                (true, _) => Left("not under Wayland, where the display does not know what it looks like".into()),
                (false, Some(pointer)) => Worked(format!("the display says what it looks like, {} x {}", pointer.w, pointer.h)),
                (false, None) => Failed("the display will not say what the pointer looks like: --pointer puts none in".into()),
            },
        ));
        results.push((
            "keys said",
            match (wayland, display.conn.xinput_xi_query_version(2, 0).ok().and_then(|cookie| cookie.reply().ok())) {
                (true, _) => Left("not under Wayland, which lets no program hear the keys pressed in another".into()),
                (false, Some(_)) => Worked("the display will say what keys are pressed, for --keys".into()),
                (false, None) => Failed("the display will not say what keys are pressed: --keys shows none".into()),
            },
        ));
    }
    drop(display);

    // A picture left on the screen: there a moment later, and taken down.
    results.push((
        "pin",
        match Command::new(std::env::current_exe().unwrap_or_default()).args(["--show", &png.to_string_lossy(), "--at", "0,0"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn() {
            Ok(mut child) => {
                std::thread::sleep(Duration::from_millis(700));
                let stayed = matches!(child.try_wait(), Ok(None));
                let _ = child.kill();
                let said = child.wait_with_output().map(|done| String::from_utf8_lossy(&done.stderr).trim().to_owned()).unwrap_or_default();
                if stayed { Worked("a picture stayed on the screen until it was taken down".into()) } else { Failed(format!("the picture did not stay: {said}")) }
            }
            Err(error) => Failed(error.to_string()),
        },
    ));

    // A second of a corner of the screen, recorded.
    let film = in_scratch("second.mp4");
    results.push((
        "recording",
        if wayland {
            // What it takes there, each looked for; the recording
            // itself is not tried, as the desktop would ask the user
            // what of the screen to hand over.
            let has = |element: &str| Command::new("gst-inspect-1.0").args(["--exists", element]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success());
            let cast = Bus::session().and_then(|mut bus| bus.ask(Message::call("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.DBus.Properties", "Get", vec![Value::text("org.freedesktop.portal.ScreenCast"), Value::text("version")])));
            let mut lacking = Vec::new();
            if !installed("gst-launch-1.0") {
                lacking.push("GStreamer's gst-launch-1.0 (gstreamer1.0-tools)");
            } else if !has("pipewiresrc") {
                lacking.push("GStreamer's PipeWire plugin (gstreamer1.0-pipewire)");
            }
            if cast.is_err() {
                lacking.push("a portal that hands the screen over as it changes (ScreenCast)");
            }
            match (lacking.is_empty(), installed("ffmpeg")) {
                (false, _) => Failed(format!("it lacks {}", lacking.join(", and "))),
                (true, false) => Left("what it takes is here but for ffmpeg: it would record all of a monitor, and not an area of it. Not tried, as the desktop would ask what to share: `hotshot --record -f --duration 2` tries it".into()),
                (true, true) => Left("what it takes is here. Not tried, as the desktop would ask what to share: `hotshot --record -f --duration 2` tries it".into()),
            }
        } else if !installed("ffmpeg") {
            Failed("ffmpeg is not installed, which is what records".into())
        } else {
            match again(&["--record", "--region", "0,0,160,120", "--duration", "1", "--output", &film.to_string_lossy()], Duration::from_secs(20)) {
                Some(true) if std::fs::metadata(&film).is_ok_and(|file| file.len() > 0) => Worked(format!("a second of it, {} kB{}", std::fs::metadata(&film).map_or(0, |file| file.len()) / 1000, if installed("gifski") { "; gifski is here for GIFs" } else { "; GIFs by ffmpeg, there being no gifski" })),
                Some(true) => Failed("it ended well and left no recording".into()),
                Some(false) => Failed("the recording failed; `hotshot --record -g 0,0,160,120 --duration 1` says why".into()),
                None => Failed("the recording did not end when its time was up".into()),
            }
        },
    ));

    // A tray for the icon to go in.
    results.push((
        "tray",
        match Bus::session().and_then(|mut bus| bus.ask(Message::call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner", vec![Value::text("org.kde.StatusNotifierWatcher")]))) {
            Ok(answer) if answer.body.first() == Some(&Value::Bool(true)) => Worked("a tray is kept over the bus, for --tray".into()),
            other => {
                let old = Display::open().ok().is_some_and(|display| {
                    let name = format!("_NET_SYSTEM_TRAY_S{}", display.screen);
                    let atom = display.conn.intern_atom(false, name.as_bytes()).ok().and_then(|cookie| cookie.reply().ok()).map_or(x11rb::NONE, |reply| reply.atom);
                    display.conn.get_selection_owner(atom).ok().and_then(|cookie| cookie.reply().ok()).is_some_and(|reply| reply.owner != x11rb::NONE)
                });
                match (old, other) {
                    (true, _) => Worked("X11's tray, for --tray".into()),
                    (false, Err(error)) => Left(format!("no tray: the session's bus could not be asked ({error}); --menu is the list without one")),
                    (false, Ok(_)) => Left("no tray is kept here (GNOME has none without an extension); --menu is the list without one".into()),
                }
            }
        },
    ));

    let _ = std::fs::remove_dir_all(&scratch);
    let mut failed = 0;
    for (name, came) in &results {
        let (mark, said) = match came {
            Worked(said) => ("ok  ", said),
            Left(said) => ("--  ", said),
            Failed(said) => {
                failed += 1;
                ("FAIL", said)
            }
        };
        println!("  {mark}  {name:<12} {said}");
    }
    let tried = results.iter().filter(|(_, came)| !matches!(came, Left(_))).count();
    println!("\n{}", if failed == 0 { format!("All {tried} that were tried worked.") } else { format!("{failed} of {tried} that were tried did not work.") });
    if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
