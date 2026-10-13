mod bench;
mod canvas;
mod clipboard;
mod editor;
mod frame;
mod options;
mod output;
mod shapes;
mod strips;
mod text;
mod x11;

use std::process::ExitCode;
use std::time::{Duration, Instant};

use canvas::Rect;
use editor::{Editor, Finish, Outcome, Shot};
use options::{Options, Parsed, Take};
use strips::Strips;
use text::Text;
use x11::{Display, Input};

fn main() -> ExitCode {
    let started = Instant::now();
    let options = match options::parse(std::env::args_os().skip(1)) {
        Ok(Parsed::Run(options)) => *options,
        Ok(Parsed::Help) => {
            println!("{}", options::USAGE);
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Version) => {
            println!("hotshot {}", env!("CARGO_PKG_VERSION"));
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

    // Under Wayland there is still an X server to talk to, but the screen
    // it would hand over is not the one being looked at.
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|name| !name.is_empty()) {
        return Err("hotshot reads the screen through X11, and this is a Wayland session, which does not let it".into());
    }
    let mut display = Display::open()?;
    trace("display opened");
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
    let mut chosen = None;
    if matches!(options.take, Take::Choose | Take::Titled(_)) {
        let windows = display.titled();
        let found = match &options.take {
            Take::Titled(wanted) => windows.iter().find(|(_, title, _)| title.to_lowercase().contains(&wanted.to_lowercase())).ok_or_else(|| format!("no window has '{wanted}' in its title"))?,
            _ => {
                if windows.is_empty() {
                    return Err("there are no windows to choose from".into());
                }
                for (number, (_, title, place)) in windows.iter().enumerate() {
                    eprintln!("{:>3}  {:<56.56}  {} x {}", number + 1, title, place.w, place.h);
                }
                eprint!("\n> ");
                let mut typed = String::new();
                std::io::stdin().read_line(&mut typed)?;
                let number: usize = typed.trim().parse().map_err(|_| "that is not one of the numbers")?;
                windows.get(number.wrapping_sub(1)).ok_or("that is not one of the numbers")?
            }
        };
        chosen = Some((found.0, found.2.intersect(screen)));
    }
    // What was asked for by name is known already.
    let named = match options.take {
        Take::Pick => None,
        Take::Full => Some(screen),
        Take::Region(area) => Some(Some(area.intersect(screen)).filter(|area| !area.is_empty()).ok_or("that region is not on the screen")?),
        Take::Again => {
            let last = output::last_area().ok_or("there is no picture from before to take the area of again")?;
            Some(Some(last.intersect(screen)).filter(|area| !area.is_empty()).ok_or("the area of the last picture is no longer on the screen")?)
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
    let said = Finish { copy: options.copy, save: options.save || options.output.is_some() };
    if said.copy || said.save {
        options.settings.enter = said;
    }

    let taken = match (named, options.edit) {
        (Some(area), false) => {
            let how = if said.copy || said.save { said } else { Finish { copy: false, save: true } };
            // A window chosen is its own picture, if there is one to be
            // had; anything else, and a window with none, is that much
            // of the screen.
            let own = chosen.and_then(|(window, _)| display.window_picture(window));
            let (area, mut pixels) = match own {
                Some((wide, high, pixels)) => (Rect::new(area.x, area.y, wide, high), pixels),
                None => {
                    let shot = display.capture()?;
                    (area, canvas::crop(&shot, display.width, area))
                }
            };
            trace("screen read");
            // Corners as round as the editor would have made them, and
            // the frame it would have set the picture in.
            let scale = editor::scale_for(display.height);
            let round = ((options.settings.corners * scale).round() as i32).clamp(0, area.w.min(area.h) / 2);
            let frame = options.settings.frame;
            let (mut width, mut height, mut alpha) = (area.w, area.h, round > 0);
            if round > 0 || !frame.is_plain() {
                canvas::round_alpha(&mut pixels, area.w, round);
            }
            if !frame.is_plain() {
                (width, height, pixels) = frame::set(frame, &pixels, area.w, area.h, round, scale, None);
                alpha = !matches!(frame.backdrop, frame::Backdrop::Gradient(_));
            }
            Taken { area, width, height, pixels, alpha, how, place: options.settings.place }
        }
        (named, _) => match pick(&mut display, named, &options, font, &trace)? {
            Some(taken) => taken,
            None => return Ok(ExitCode::from(1)),
        },
    };
    trace("picture taken");
    output::remember_area(taken.area);
    let how = taken.how;

    let png = output::encode(taken.width as u32, taken.height as u32, &taken.pixels, taken.alpha)?;
    trace("packed as PNG");
    if how.save {
        // Into the folder Save was last pointed at, of those there are.
        let dir = options.places.get(taken.place).unwrap_or(&options.dir);
        match &options.output {
            Some(file) => output::save_as(file, &png)?,
            None => println!("{}", output::save(dir, &options.filename, &png)?.display()),
        }
        trace("saved");
    }
    if how.copy {
        clipboard::offer(display, png)?;
        trace("on the clipboard");
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
}

/// Freezes the screen and lets an area be dragged out and drawn on.
/// Gives the area, the picture of it, and what is to be done with it;
/// or nothing, if it was called off.
fn pick(display: &mut Display, named: Option<Rect>, options: &Options, font: std::thread::JoinHandle<Text>, trace: &dyn Fn(&str)) -> x11::Result<Option<Taken>> {
    // The first frame is the screen darkened and nothing else: it is
    // what says hotshot has started, and the server makes it of its own
    // copy of the screen, without a pixel having come here. The keyboard
    // is taken there and then, so that nothing typed goes to what is
    // under the window.
    let dim = options.settings.dim.min(100);
    let frozen = display.freeze(dim)?;
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
        (true, Some(segment)) => Strips::new(segment, width, height, pointer.1),
        _ => {
            shot = display.capture()?;
            trace("screen read");
            canvas::darken_into(&mut dimmed, &shot, 256 - dim * 256 / 100);
            canvas::copy_into(&mut frame, &dimmed);
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
    if let Some(area) = named {
        editor.select(area);
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
    let place = editor.place();
    Ok(how.and_then(|how| editor.framed().map(|(area, width, height, pixels, alpha)| Taken { area, width, height, pixels, alpha, how, place })))
}
