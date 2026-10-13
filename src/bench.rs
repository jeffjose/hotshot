//! `--benchmark`: hotshot going through its paces by itself, on the
//! display it is on, timing each. The screen is frozen as ever; an area
//! is dragged out, moved and drawn on by calls where a hand would have
//! made them; and the picture is packed. Nothing is saved or copied, and
//! neither the keyboard nor the pointer is taken.

use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::canvas;
use crate::editor::{Editor, Key, Mods, Shot};
use crate::options::Options;
use crate::output;
use crate::text::Text;
use crate::x11::{Display, Result};

/// How many frames each kind of movement is timed over.
const STEPS: i32 = 90;

fn figures(name: &str, times: &mut [Duration]) {
    if times.is_empty() {
        return;
    }
    times.sort();
    let at = |part: usize| times[(times.len() * part / 100).min(times.len() - 1)];
    println!("  {name:<34} median {:>7.2?}   90% {:>7.2?}   most {:>7.2?}", at(50), at(90), times[times.len() - 1]);
}

pub fn run(mut display: Display, options: Options, font: JoinHandle<Text>, started: Instant) -> Result<()> {
    let opened = started.elapsed();
    // As it is done in earnest: frozen by the server, and only then read.
    let dim = options.settings.dim.min(100);
    let by_server = display.freeze(dim)?;
    let mut frozen = started.elapsed();
    if by_server {
        display.prepare()?;
    }
    let debug = options.debug;
    let trace = |what: &str| {
        if debug {
            eprintln!("hotshot: {:>7.1?} {what}", started.elapsed());
        }
    };
    trace("frozen, cursors and keys read");
    let before = Instant::now();
    let shot = display.capture()?;
    let read = before.elapsed();
    trace("screen read");
    let mut dimmed = vec![0u32; shot.len()];
    canvas::darken_into(&mut dimmed, &shot, 256 - dim * 256 / 100);
    trace("darkened");
    let mut frame = display.buffer();
    canvas::copy_into(&mut frame, &dimmed);
    trace("frame buffer made");
    if !by_server {
        display.show(&frame)?;
        frozen = started.elapsed();
        display.prepare()?;
    }

    let (width, height) = (display.width, display.height);
    let dim = dimmed;
    let (monitors, windows) = (display.monitors(), display.windows());
    trace("monitors and windows asked after");
    let text = font.join().map_err(|_| "the font could not be read")?;
    let shot = Shot { width, height, pixels: Box::new(shot) };
    let mut editor = Editor::new(shot, dim, monitors, windows, options.settings.clone(), text);
    let mut back = vec![0u32; frame.len()];
    trace("editor made");
    // The first frame: over the server's darkened screen go only the
    // parts of it that are not that.
    editor.draw(&mut back);
    trace("first frame drawn");
    let parts = canvas::changed(&back, &mut frame, width as usize);
    display.present(&frame, &parts)?;
    trace("first frame up");
    let ready = started.elapsed();
    let mut frames = |editor: &mut Editor, display: &Display, times: &mut Vec<Duration>| -> Result<()> {
        let began = Instant::now();
        editor.draw(&mut back);
        let parts = canvas::changed(&back, &mut frame, width as usize);
        display.present(&frame, &parts)?;
        times.push(began.elapsed());
        Ok(())
    };

    println!("hotshot {} on a {width} x {height} screen", env!("CARGO_PKG_VERSION"));
    println!("Starting, from the process being launched:");
    println!("  {:<34} {:>7.1?}", "display opened", opened);
    println!("  {:<34} {:>7.1?}", "frozen screen showing", frozen);
    println!("  {:<34} {:>7.1?}   (the screen read out of the server: {read:.1?})", "ready to be dragged on", ready);

    // The pointer wandering before anything is selected, the magnifier
    // and the window under it following.
    let plain = Mods::default();
    let along = |step: i32, from: i32, to: i32| from + (to - from) * step / STEPS;
    let mut times = Vec::new();
    for step in 0..STEPS {
        editor.pointer_moved(along(step, width / 8, width * 7 / 8), along(step, height / 8, height * 7 / 8), plain);
        frames(&mut editor, &display, &mut times)?;
    }
    println!("A frame, from the pointer moving to its being on the screen:");
    figures("looking for where to start", &mut times);

    // An area dragged out from one corner to the other.
    let mut times = Vec::new();
    editor.pointer_moved(width / 5, height / 5, plain);
    editor.pressed(0);
    for step in 1..=STEPS {
        editor.pointer_moved(along(step, width / 5, width * 4 / 5), along(step, height / 5, height * 4 / 5), plain);
        frames(&mut editor, &display, &mut times)?;
    }
    editor.released();
    figures("dragging out an area", &mut times);

    // The area made smaller by a corner, then moved about.
    let mut times = Vec::new();
    editor.pressed(1000);
    for step in 1..=STEPS {
        editor.pointer_moved(along(step, width * 4 / 5, width * 3 / 5), along(step, height * 4 / 5, height * 3 / 5), plain);
        frames(&mut editor, &display, &mut times)?;
    }
    editor.released();
    figures("resizing it", &mut times);
    let mut times = Vec::new();
    editor.pointer_moved(width * 2 / 5, height * 2 / 5, plain);
    editor.pressed(2000);
    for step in 1..=STEPS {
        editor.pointer_moved(along(step, width * 2 / 5, width / 2), along(step, height * 2 / 5, height / 2), plain);
        frames(&mut editor, &display, &mut times)?;
    }
    editor.released();
    figures("moving it", &mut times);

    // Something of each kind drawn across it.
    let Some(area) = editor.selection() else { return Err("the benchmark lost its selection".into()) };
    for (key, name) in [('a', "drawing an arrow"), ('r', "drawing a rectangle"), ('e', "drawing an ellipse"), ('p', "drawing with the pen"), ('b', "blurring")] {
        let mut times = Vec::new();
        editor.key(Key::Char(key), plain);
        let (from, to) = ((area.x + area.w / 8, area.y + area.h / 8), (area.x + area.w * 7 / 8, area.y + area.h * 7 / 8));
        editor.pointer_moved(from.0, from.1, plain);
        editor.pressed(3000);
        for step in 1..=STEPS {
            // The pen wanders; the rest go straight across.
            let wobble = if key == 'p' { ((step as f32 * 0.4).sin() * area.h as f32 / 6.0) as i32 } else { 0 };
            editor.pointer_moved(along(step, from.0, to.0), along(step, from.1, to.1) + wobble, plain);
            frames(&mut editor, &display, &mut times)?;
        }
        editor.released();
        figures(name, &mut times);
    }
    display.hide();

    // And the picture of it, as it would be saved or copied.
    let began = Instant::now();
    let (area, pixels) = editor.result().ok_or("the benchmark lost its selection")?;
    let drawn = began.elapsed();
    let began = Instant::now();
    let png = output::encode(area.w as u32, area.h as u32, &pixels)?;
    println!("The picture, {} x {} with five shapes on it:", area.w, area.h);
    println!("  {:<34} {:>7.1?}", "drawn", drawn);
    println!("  {:<34} {:>7.1?}   {:.1} MB", "packed as PNG", began.elapsed(), png.len() as f64 / 1e6);
    Ok(())
}
