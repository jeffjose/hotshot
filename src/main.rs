mod bench;
mod canvas;
mod clipboard;
mod editor;
mod options;
mod output;
mod shapes;
mod text;
mod x11;

use std::process::ExitCode;
use std::time::{Duration, Instant};

use canvas::Rect;
use editor::{Editor, Finish, Outcome, Shot};
use options::{Options, Parsed, Take};
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
    // What was asked for by name is known already.
    let named = match options.take {
        Take::Pick => None,
        Take::Full => Some(screen),
        Take::Region(area) => Some(Some(area.intersect(screen)).filter(|area| !area.is_empty()).ok_or("that region is not on the screen")?),
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

    let (area, pixels, how) = match (named, options.edit) {
        (Some(area), false) => {
            let how = if said.copy || said.save { said } else { Finish { copy: false, save: true } };
            let shot = display.capture()?;
            trace("screen read");
            (area, canvas::crop(&shot, display.width, area), how)
        }
        (named, _) => match pick(&mut display, named, &options, font, &trace)? {
            Some(taken) => taken,
            None => return Ok(ExitCode::from(1)),
        },
    };
    trace("picture taken");

    let png = output::encode(area.w as u32, area.h as u32, &pixels)?;
    trace("packed as PNG");
    if how.save {
        match &options.output {
            Some(file) => output::save_as(file, &png)?,
            None => println!("{}", output::save(&options.dir, &options.filename, &png)?.display()),
        }
        trace("saved");
    }
    if how.copy {
        clipboard::offer(display, png)?;
        trace("on the clipboard");
    }
    Ok(ExitCode::SUCCESS)
}

/// Freezes the screen and lets an area be dragged out and drawn on.
/// Gives the area, the picture of it, and what is to be done with it;
/// or nothing, if it was called off.
fn pick(display: &mut Display, named: Option<Rect>, options: &Options, font: std::thread::JoinHandle<Text>, trace: &dyn Fn(&str)) -> x11::Result<Option<(Rect, Vec<u32>, Finish)>> {
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
    let shot = display.capture()?;
    trace("screen read");
    let mut dimmed = vec![0u32; shot.len()];
    canvas::darken_into(&mut dimmed, &shot, 256 - dim * 256 / 100);
    let mut frame = display.buffer();
    canvas::copy_into(&mut frame, &dimmed);
    if !frozen {
        display.show(&frame)?;
        trace("frozen screen up");
        display.prepare()?;
        grabbed = display.grab();
    }

    let shot = Shot { width: display.width, height: display.height, pixels: Box::new(shot) };
    let (monitors, windows, pointer) = (display.monitors(), display.windows(), display.pointer());
    let text = font.join().map_err(|_| "the font could not be read")?;
    let width = shot.width as usize;
    let dim = dimmed;
    let mut editor = Editor::new(shot, dim, monitors, windows, options.settings.clone(), text);
    editor.pointer_moved(pointer.0, pointer.1, editor::Mods::default());
    if let Some(area) = named {
        editor.select(area);
    }
    let mut back = vec![0u32; frame.len()];
    trace("editor ready");

    let mut first = true;
    let how = 'events: loop {
        if editor.take_dirty() {
            editor.draw(&mut back);
            let parts = canvas::changed(&back, &mut frame, width);
            // Only what has changed goes up, the first time too: under
            // it is the screen as the server darkened it, which is the
            // screen as it was darkened here to within a shade, and a
            // whole 4K frame is a long time going up.
            display.present(&frame, &parts)?;
            if std::mem::take(&mut first) {
                trace("first frame of the editor up");
            }
        }
        display.set_cursor(editor.cursor());
        // Until the keyboard is had, it is asked for again every so often.
        let inputs = display.inputs(if grabbed { None } else { Some(20) })?;
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
                // Whatever was over the window has gone: all of it again.
                Input::Expose => {
                    display.present(&frame, &[Rect::new(0, 0, display.width, display.height)])?;
                    Outcome::Continue
                }
                _ => Outcome::Continue,
            };
            match outcome {
                Outcome::Continue => {}
                Outcome::Cancel => break 'events None,
                Outcome::Finish(how) => break 'events Some(how),
            }
        }
    };
    // Down at once: the packing and saving are not to be watched.
    display.hide();
    trace("window down");
    Ok(how.and_then(|how| editor.result().map(|(area, pixels)| (area, pixels, how))))
}
