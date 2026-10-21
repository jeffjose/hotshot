mod bench;
mod canvas;
mod check;
mod clipboard;
mod dbus;
mod editor;
mod frame;
mod history;
mod options;
mod output;
mod pin;
mod record;
mod shapes;
mod strips;
mod text;
mod tray;
mod wayland;
mod x11;

use std::process::ExitCode;
use std::time::{Duration, Instant};

use canvas::Rect;
use editor::{Editor, Finish, Outcome, Shot};
use options::{Options, Parsed, Take};
use strips::Strips;
use text::Text;
use x11::{Display, Input};

/// The version and the commit it was built from: `0.2.0-abc1234`, with
/// `-dirty` if the tree had changes not committed.
pub fn version() -> String {
    // Unset where git could not be asked, or vergen's stand-in.
    let Some(sha) = option_env!("VERGEN_GIT_SHA").filter(|sha| !sha.is_empty() && !sha.contains("VERGEN")) else {
        return env!("CARGO_PKG_VERSION").to_string();
    };
    let dirty = if option_env!("VERGEN_GIT_DIRTY") == Some("true") { "-dirty" } else { "" };
    format!("{}-{sha}{dirty}", env!("CARGO_PKG_VERSION"))
}

fn main() -> ExitCode {
    let started = Instant::now();
    let options = match options::parse(std::env::args_os().skip(1)) {
        Ok(Parsed::Run(options)) => *options,
        Ok(Parsed::Help) => {
            println!("{}", options::USAGE);
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Version) => {
            println!("hotshot {}", version());
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Config) => {
            print!("{}", include_str!("../assets/config.toml"));
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("hotshot: {error}\nTry 'hotshot --help' for the options.");
            return ExitCode::from(2);
        }
    };
    match run(options, started) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("hotshot: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut options: Options, started: Instant) -> x11::Result<ExitCode> {
    let debug = options.debug;
    let trace = |what: &str| {
        if debug {
            eprintln!("hotshot: {:>7.1?} {what}", started.elapsed());
        }
    };
    // The font is read while the screen is: neither waits on the other,
    // and the first frame, which has no words in it, on neither.
    let font = std::thread::spawn(Text::load);
    if options.delay > 0.0 {
        std::thread::sleep(Duration::from_secs_f64(options.delay));
    }

    // Each thing hotshot does, tried, and said to have worked or not.
    if options.check {
        return Ok(check::run(&options));
    }
    // The pictures taken before, listed; and where there is someone at
    // a terminal to choose, the one chosen taken again, as --again
    // would, with whatever else the command line says.
    if options.history {
        let all = history::all();
        if all.is_empty() {
            println!("No pictures taken yet.");
            return Ok(ExitCode::SUCCESS);
        }
        print!("{}", history::listed(&all));
        // SAFETY: asks whether a descriptor is a terminal, and no more.
        if unsafe { libc::isatty(0) } != 1 {
            return Ok(ExitCode::SUCCESS);
        }
        eprint!("\nWhich, to take again (its @number or name; nothing to leave it)? ");
        let mut typed = String::new();
        std::io::stdin().read_line(&mut typed)?;
        let typed = typed.trim();
        if typed.is_empty() {
            return Ok(ExitCode::SUCCESS);
        }
        // A bare number is the one with that @number.
        let which = if typed.chars().all(|letter| letter.is_ascii_digit()) { format!("@{typed}") } else { typed.to_owned() };
        history::find_in(&all, Some(&which))?;
        options.take = Take::Again(Some(which));
    }
    // A picture taken before, to be taken again as it was set out: its
    // corners, frame and pointer are what this one has, but for what
    // the command line or the config file says otherwise.
    let again = match &options.take {
        Take::Again(which) => Some(history::find_in(&history::all(), which.as_deref())?.recipe),
        _ => None,
    };
    // On a photograph, it is set on the copy kept of the same one.
    let mut again_photo = None;
    if let Some(again) = again {
        if options.plain_corners == 0.0 {
            options.plain_corners = again.corners;
            options.settings.corners = again.corners;
        }
        if options.settings.frame.is_plain() {
            options.settings.frame = again.frame;
            again_photo = again.photo.and_then(history::kept_photo);
            if again_photo.is_some() {
                options.settings.fetch = false;
            }
        }
        options.pointer |= again.pointer;
    }
    // A recording being made is stopped by being asked for again: one
    // key starts and stops.
    if options.record && record::stop_other() {
        return Ok(ExitCode::SUCCESS);
    }
    // What is in a file, to the clipboard; and a picture in a file,
    // left on the screen: neither takes a picture of anything.
    if let Some(file) = &options.clip {
        let png = std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        if options.forget {
            let _ = std::fs::remove_file(file);
        }
        clipboard::offer(Display::open()?, png)?;
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(file) = &options.show {
        pin::show(file, options.at, &options)?;
        return Ok(ExitCode::SUCCESS);
    }
    // The icon in the tray, which stays; and its list, which does not.
    if options.tray {
        tray::run(&options)?;
        return Ok(ExitCode::SUCCESS);
    }
    if options.menu {
        tray::menu_once(&options)?;
        return Ok(ExitCode::SUCCESS);
    }
    if options.record {
        options.settings.recording = true;
        options.settings.enter = Finish { record: true, ..Finish::NONE };
    }

    // Under Wayland there is still an X server to talk to, and to show
    // a window by, but the screen it would hand over is not the one
    // being looked at: that is asked of the desktop. And the window is
    // one the desktop manages, one to each monitor, or it is given no keys.
    let wayland = wayland::session();
    if wayland && options.benchmark {
        return Err("under Wayland hotshot does not time itself: what takes the time there is the desktop's".into());
    }
    let mut display = Display::open().map_err(|error| if wayland { format!("{error}; under Wayland hotshot shows its window through XWayland, which this session is not running").into() } else { error })?;
    if wayland {
        display.manage();
    }
    trace("display opened");
    // The pointer as it is now, before it is made hotshot's own: to go
    // in the picture if it is wanted there, now or by the button for it.
    // Under Wayland the display does not know what it looks like.
    let arrow = if wayland { None } else { display.pointer_picture() };
    if options.list_monitors {
        for (number, monitor) in display.monitors().iter().enumerate() {
            println!("{number}: {}x{}+{}+{}", monitor.w, monitor.h, monitor.x, monitor.y);
        }
        return Ok(ExitCode::SUCCESS);
    }
    if options.benchmark {
        return bench::run(display, options, font, started).map(|()| ExitCode::SUCCESS);
    }

    let screen = Rect::new(0, 0, display.width, display.height);
    // A window by its number in a list, or by its title: found here,
    // and taken as its own picture where the display will give one.
    let mut chosen: Option<(Option<u32>, Rect)> = None;
    if matches!(options.take, Take::Choose | Take::Titled(_)) {
        let windows = display.titled();
        chosen = Some(match &options.take {
            Take::Titled(wanted) => {
                let found = windows.iter().find(|(_, title, _)| title.to_lowercase().contains(&wanted.to_lowercase())).ok_or_else(|| format!("no window has '{wanted}' in its title"))?;
                (Some(found.0), found.2.intersect(screen))
            }
            // The list: the whole screen first, then each display where
            // there is more than one, then the windows, the topmost
            // first. What is chosen is opened to be drawn on, not
            // taken there and then.
            _ => {
                let monitors = display.monitors();
                let mut list: Vec<(String, Option<u32>, Rect)> = vec![("The whole screen".to_owned(), None, screen)];
                if monitors.len() > 1 {
                    list.extend(monitors.iter().enumerate().map(|(number, monitor)| (format!("Display {}", number + 1), None, *monitor)));
                }
                list.extend(windows.iter().map(|(window, title, place)| (title.clone(), Some(*window), *place)));
                for (number, (name, _, place)) in list.iter().enumerate() {
                    eprintln!("{:>3}  {:<56.56}  {} x {}", number + 1, name, place.w, place.h);
                }
                eprint!("\n> ");
                let mut typed = String::new();
                std::io::stdin().read_line(&mut typed)?;
                let number: usize = typed.trim().parse().map_err(|_| "that is not one of the numbers")?;
                let (_, window, place) = list.get(number.wrapping_sub(1)).ok_or("that is not one of the numbers")?;
                options.edit = true;
                (*window, place.intersect(screen))
            }
        });
    }
    // What was asked for by name is known already.
    let mut reopened = None;
    let named = match options.take {
        Take::Pick => None,
        Take::Full => Some(screen),
        Take::Region(area) => Some(Some(area.intersect(screen)).filter(|area| !area.is_empty()).ok_or("that region is not on the screen")?),
        Take::Reopen => {
            let kept = output::kept_for_reopening().ok_or("there is no picture from before to open again")?;
            let area = kept.0.kept_within(screen).intersect(screen);
            reopened = Some(kept);
            Some(area)
        }
        Take::Again(_) => {
            let last = again.map(|again| again.area).ok_or("there is no picture from before to take again")?;
            Some(Some(last.intersect(screen)).filter(|area| !area.is_empty()).ok_or("the area of that picture is no longer on the screen")?)
        }
        Take::Choose | Take::Titled(_) => chosen.map(|(_, place)| place).filter(|place| !place.is_empty()),
        Take::Window => Some(display.active_window().map(|window| window.intersect(screen)).filter(|window| !window.is_empty()).ok_or("no window has the keyboard")?),
        Take::Monitor(number) => {
            let monitors = display.monitors();
            let pointer = display.pointer();
            Some(match number {
                Some(number) => *monitors.get(number).ok_or_else(|| format!("there is no monitor {number}; --monitors lists them"))?,
                None => monitors.iter().copied().find(|monitor| monitor.contains(pointer.0, pointer.1)).unwrap_or(monitors[0]),
            })
        }
    };
    // Said on the command line, what to do with the picture is also what
    // Enter does.
    let said = Finish { copy: options.copy, save: options.save || options.output.is_some(), pin: options.pin, record: false };
    if said != Finish::NONE {
        options.settings.enter = said;
    }

    // An area named to be recorded is recorded, with no stop on the way
    // to choose or draw on it.
    if let (true, Some(area), false) = (options.record, named, options.edit) {
        drop(display);
        return recorded(area, screen, &options);
    }
    let taken = match (named, options.edit) {
        (Some(area), false) => {
            let how = if said != Finish::NONE { said } else { Finish { save: true, ..Finish::NONE } };
            // A window chosen is its own picture, if there is one to be
            // had; anything else, and a window with none, is that much
            // of the screen.
            let own = chosen.and_then(|(window, _)| window).and_then(|window| display.window_picture(window));
            let (area, mut pixels) = match own {
                Some((wide, high, pixels)) => (Rect::new(area.x, area.y, wide, high), pixels),
                None => {
                    let shot = read_screen(&mut display)?;
                    (area, canvas::crop(&shot, display.width, area))
                }
            };
            trace("screen read");
            if let Some(arrow) = arrow.as_ref().filter(|_| options.pointer) {
                arrow.stamp(&mut pixels, area.w, (area.x, area.y));
            }
            // Corners as round as the editor would have made them, and
            // the frame it would have set the picture in.
            let scale = editor::scale_for(display.height);
            let round = ((options.plain_corners * scale).round() as i32).clamp(0, area.w.min(area.h) / 2);
            let frame = options.settings.frame;
            let (mut width, mut height, mut alpha) = (area.w, area.h, round > 0);
            if round > 0 || !frame.is_plain() {
                canvas::round_alpha(&mut pixels, area.w, round);
            }
            let mut photo = None;
            if !frame.is_plain() {
                // On a photograph: the one taken again, the user's own,
                // or one nobody chose, fetched now, about the size it
                // will be.
                photo = match (frame.backdrop == frame::Backdrop::Photo, options.settings.fetch) {
                    (false, _) => None,
                    (true, _) if again_photo.is_some() => again_photo.take(),
                    (true, true) => {
                        let room = frame.room(scale);
                        frame::random(area.w + 2 * room.0, area.h + 2 * room.1)
                    }
                    (true, false) => options.settings.photo.as_ref().and_then(|file| std::fs::read(file).ok()).and_then(|bytes| frame::open(&bytes)),
                };
                (width, height, pixels) = frame::set(frame, &pixels, area.w, area.h, round, scale, photo.as_ref());
                alpha = frame.is_clear() || (frame.backdrop == frame::Backdrop::Photo && photo.is_none());
            }
            // On a photograph only where there was one to be had.
            let mark = photo.as_ref().map(history::photo_mark);
            let backdrop = if frame.backdrop == frame::Backdrop::Photo && mark.is_none() { frame::Backdrop::None } else { frame.backdrop };
            let recipe = history::Recipe { area, corners: options.plain_corners, frame: frame::Frame { backdrop, ..frame }, pointer: options.pointer, photo: mark };
            Taken { area, width, height, pixels, alpha, how, place: options.settings.place, keep: None, recipe, photo }
        }
        (named, _) => match pick(&mut display, named, reopened, again_photo, arrow, &options, font, &trace)? {
            Some(taken) => taken,
            None => return Ok(ExitCode::from(1)),
        },
    };
    trace("picture taken");
    // Written down, but for one taken again just as it was: that one
    // stays where it is in the list, so that `--again @2` is the same
    // picture however many times it is taken.
    if again.is_none_or(|again| again.name() != taken.recipe.name()) {
        history::remember(&taken.recipe, taken.photo.as_ref());
    }
    let how = taken.how;
    // Not a picture after all: the area is recorded, once the display
    // has had a moment to show the screen as it is again.
    if how.record {
        drop(display);
        return recorded(taken.area, screen, &options);
    }

    // A file is of the kind its name says, or the kind asked for; what
    // goes to the clipboard is a PNG whatever the file is.
    let named = options.output.as_ref().and_then(|file| file.extension()).and_then(|ending| output::Format::named(&ending.to_string_lossy()));
    let format = named.unwrap_or(options.format);
    let pack = |format| output::pack(format, options.quality, taken.width as u32, taken.height as u32, &taken.pixels, taken.alpha);
    let png = if how.copy || how.pin || format == output::Format::Png { pack(output::Format::Png)? } else { Vec::new() };
    trace("packed");
    // Left on the screen where it was taken, its frame round where the
    // area was: by a hotshot of its own, which outlives this one.
    if how.pin {
        pin::leave(&png, (taken.area.x - (taken.width - taken.area.w) / 2, taken.area.y - (taken.height - taken.area.h) / 2))?;
        trace("left on the screen");
    }
    if how.save {
        let other = if format == output::Format::Png { None } else { Some(pack(format)?) };
        let file = other.as_ref().unwrap_or(&png);
        // Into the folder Save was last pointed at, of those there are.
        let dir = options.places.get(taken.place).unwrap_or(&options.dir);
        match &options.output {
            Some(path) => output::save_as(path, file)?,
            None => println!("{}", output::save(dir, &options.filename, format.ending(), file)?.display()),
        }
        trace("saved");
    }
    // What it takes to open this one again, for `--reopen`.
    if let Some((area, pixels, shapes)) = &taken.keep {
        output::keep_for_reopening(*area, pixels, shapes);
        trace("kept to be opened again");
    }
    if how.copy {
        // Under Wayland by the clipboard's own keeper there, where
        // that is installed; and by X11's otherwise, as ever.
        if !(wayland && wayland::copy(&png, "image/png")) {
            clipboard::offer(display, png)?;
        }
        trace("on the clipboard");
    }
    Ok(ExitCode::SUCCESS)
}

/// The screen as it is now: read off the display, or under Wayland
/// asked of the desktop.
fn read_screen(display: &mut Display) -> x11::Result<x11::Buffer> {
    match display.managed {
        true => Ok(x11::Buffer::Own(wayland::screen(display)?)),
        false => display.capture(),
    }
}

/// Records an area until it is stopped, says where the recording is,
/// and puts the file on the clipboard if that was asked.
fn recorded(area: Rect, screen: Rect, options: &Options) -> x11::Result<ExitCode> {
    std::thread::sleep(Duration::from_millis(120));
    // Under Wayland by having the desktop hand the screen over; on
    // X11 by reading it.
    let file = match wayland::session() {
        true => record::run_cast(area, screen, options)?,
        false => record::run(area, options, Text::load())?,
    };
    println!("{}", file.display());
    if options.copy {
        clipboard::offer_file(Display::open()?, &file)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// A picture taken: the area of the screen it is of, how wide and tall
/// it came out with whatever it is set in, its pixels, whether any are
/// see-through, what is to be done with it, and which folder it goes to
/// if it is saved.
struct Taken {
    area: Rect,
    width: i32,
    height: i32,
    pixels: Vec<u32>,
    alpha: bool,
    how: Finish,
    place: usize,
    /// What it takes to come back to it: the area, what was there
    /// before anything was drawn, and the shapes. For one that was
    /// drawn on at all.
    keep: Option<(Rect, Vec<u32>, String)>,
    /// What it takes to take it again, for `--again`, and the
    /// photograph it is set on, to be kept for that.
    recipe: history::Recipe,
    photo: Option<frame::Photo>,
}

/// Freezes the screen and lets an area be dragged out and drawn on.
/// Gives the area, the picture of it, and what is to be done with it;
/// or nothing, if it was called off.
#[allow(clippy::too_many_arguments)]
fn pick(display: &mut Display, named: Option<Rect>, reopened: Option<(Rect, frame::Photo, Vec<shapes::Shape>)>, photo: Option<frame::Photo>, mut arrow: Option<canvas::Pointer>, options: &Options, font: std::thread::JoinHandle<Text>, trace: &dyn Fn(&str)) -> x11::Result<Option<Taken>> {
    // The first frame is the screen darkened and nothing else: it is
    // what says hotshot has started, and the server makes it of its own
    // copy of the screen, without a pixel having come here. The keyboard
    // is taken there and then, so that nothing typed goes to what is
    // under the window.
    let dim = options.settings.dim.min(100);
    // Not under Wayland, where the server has nothing of the screen to
    // freeze: the picture is asked for first, and then shown.
    let frozen = !display.managed && display.freeze(dim)?;
    let mut grabbed = false;
    if frozen {
        trace("frozen screen up");
        display.prepare()?;
        display.set_cursor(editor::Cursor::Crosshair);
        grabbed = display.grab();
    }
    // Now the pixels, which on some graphics cards are a while coming.
    // Where the screen is frozen in the server and its memory shared,
    // they are read a strip at a time, the editor up and answering the
    // pointer from the first of them. Otherwise all at once, and then
    // the window.
    let (monitors, windows, pointer) = (display.monitors(), display.windows(), display.pointer());
    let (width, height) = (display.width, display.height);
    let mut shot = display.buffer();
    let mut frame = display.buffer();
    let mut dimmed = vec![0u32; shot.len()];
    let mut strips = match (frozen, shot.segment()) {
        (true, Some(segment)) if reopened.is_none() => Strips::new(segment, width, height, pointer.1),
        _ => {
            shot = read_screen(display)?;
            trace("screen read");
            // A picture come back to has, or has not, the pointer it
            // had: the one there is now is nothing to do with it.
            if reopened.is_some() {
                arrow = None;
            }
            // A picture come back to is put where it was, over the
            // screen as it is now: all that is left of how it was then.
            if let (Some(area), Some((_, photo, _))) = (named, &reopened) {
                for row in 0..area.h.min(photo.height) {
                    let across = area.w.min(photo.width) as usize;
                    shot[((area.y + row) * width + area.x) as usize..][..across].copy_from_slice(&photo.pixels[(row * photo.width) as usize..][..across]);
                }
            }
            canvas::darken_into(&mut dimmed, &shot, 256 - dim * 256 / 100);
            canvas::copy_into(&mut frame, &dimmed);
            if let (true, Some(area), true) = (frozen, named, reopened.is_some()) {
                display.present(&frame, &[area])?;
            }
            if !frozen {
                display.show(&frame)?;
                trace("frozen screen up");
                display.prepare()?;
                grabbed = display.grab();
            }
            // Every strip had already: nothing more to read.
            let mut all = Strips::new(0, width, height, 0);
            all.have_all();
            all
        }
    };

    let shot = Shot { width, height, pixels: Box::new(shot) };
    let text = font.join().map_err(|_| "the font could not be read")?;
    let mut editor = Editor::new(shot, dimmed, monitors, windows, options.settings.clone(), text);
    editor.pointer_moved(pointer.0, pointer.1, editor::Mods::default());
    if let Some(arrow) = arrow {
        editor.set_pointer(arrow, options.pointer);
    }
    if let Some(area) = named {
        editor.select(area);
    }
    if let Some((_, _, shapes)) = reopened {
        editor.restore(shapes);
    }
    // The photograph a picture taken again was set on.
    if let Some(photo) = photo {
        editor.set_photo(photo);
    }
    let mut back = vec![0u32; frame.len()];
    trace("editor ready");

    let mut first = true;
    // How many strips have come since a frame was last drawn: drawing
    // one is not nothing, and is not done for every strip.
    let mut unseen = 0;
    let mut fetching: Option<std::sync::mpsc::Receiver<Option<frame::Photo>>> = None;
    let how = 'events: loop {
        if !strips.done() {
            strips.read(display, &mut editor, &mut frame)?;
            unseen += 1;
            if strips.done() {
                trace("screen read");
            }
        }
        if editor.take_dirty() || unseen >= 4 || (unseen > 0 && strips.done()) {
            unseen = 0;
            editor.draw(&mut back);
            // Only what has changed goes up, the first time too: under
            // it is the screen as the server darkened it, which is the
            // screen as it was darkened here to within a shade, and a
            // whole 4K frame is a long time going up.
            let parts = strips.changed(&back, &mut frame);
            display.present(&frame, &parts)?;
            if std::mem::take(&mut first) {
                trace("first frame of the editor up");
            }
        }
        display.set_cursor(editor.cursor());
        // With strips still to come, only what is already waiting is
        // heard, and then the next strip is read. Until the keyboard is
        // had, it is asked for again every so often.
        // A photograph asked for is fetched on a thread of its own, the
        // network being what it is, and looked for every so often.
        if let Some((wide, high)) = editor.take_wish() {
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || drop(sender.send(frame::random(wide, high))));
            fetching = Some(receiver);
        }
        if let Some(Ok(photo)) = fetching.as_ref().map(|receiver| receiver.try_recv()) {
            editor.photo_arrived(photo);
            fetching = None;
        }
        let inputs = display.inputs(if !strips.done() { Some(0) } else if fetching.is_some() { Some(50) } else if grabbed { None } else { Some(20) })?;
        if !grabbed {
            grabbed = display.grab();
        }
        for input in inputs {
            let outcome = match input {
                Input::Motion(x, y, mods) => {
                    editor.pointer_moved(x, y, mods);
                    Outcome::Continue
                }
                Input::Press(1, x, y, mods, time) => {
                    editor.pointer_moved(x, y, mods);
                    editor.pressed(time)
                }
                Input::Press(3, x, y, mods, _) => {
                    editor.pointer_moved(x, y, mods);
                    editor.right_pressed()
                }
                Input::Press(button @ (4 | 5), ..) => {
                    editor.scrolled(button == 4);
                    Outcome::Continue
                }
                Input::Release(1, x, y, mods) => {
                    editor.pointer_moved(x, y, mods);
                    editor.released()
                }
                Input::Key(key, mods) => editor.key(key, mods),
                // Whatever was over the window has gone: all of it
                // again, as far as it has been read.
                Input::Expose => {
                    display.present(&frame, &strips.had())?;
                    Outcome::Continue
                }
                _ => Outcome::Continue,
            };
            match outcome {
                Outcome::Continue => {}
                Outcome::Cancel => break 'events None,
                Outcome::Finish(how) => break 'events Some(how),
                Outcome::Paste => {
                    if let Some(text) = display.paste() {
                        editor.paste(&text);
                    }
                }
            }
        }
    };
    // Down at once: the packing and saving are not to be watched.
    display.hide();
    trace("window down");
    // A picture asked for before all of the screen had been read waits
    // for the rest of it.
    while how.is_some() && !strips.done() {
        strips.read(display, &mut editor, &mut frame)?;
    }
    if options.remember {
        options::remember_tools(&editor.in_hand(), options.places.get(editor.place()).map(|place| place.as_path()));
    }
    let (place, keep) = (editor.place(), editor.keepsake());
    let (recipe, photo) = (editor.recipe(), editor.photo().cloned());
    Ok(how.and_then(|how| editor.framed().map(|(area, width, height, pixels, alpha)| Taken { area, width, height, pixels, alpha, how, place, keep, recipe: history::Recipe { area, ..recipe }, photo })))
}
