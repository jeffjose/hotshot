//! The Fine-tune window, `hotshot --fine-tune FILE...`: jog's Convert
//! window nearly as it is, in hotshot's colors, for where there is no
//! frame on a canvas to split: capture mode's Fine-tune… and the
//! projects'. The picture large on the left, split by a line that follows
//! the pointer, the original to its left and the converted picture to its
//! right; the panel on the right. Convert writes each file beside its
//! original, or with `--into`, a picture just taken into the screenshots
//! folder; Esc closes it.

use std::path::{Path, PathBuf};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;

use crate::canvas::{Canvas, Rect};
use crate::convert;
use crate::editor::Key;
use crate::output::Format;
use crate::studio::{DIM_TEXT_COLOR, VIEW_COLOR};
use crate::text::Text;
use crate::tune::{Asked, Source, Tuner, Words};

const PAD: i32 = 16;
/// How many times larger the loupe shows the picture.
const LOUPE: i32 = 8;

/// Where the kind last converted to is kept, to open on next time.
fn last_format() -> Option<PathBuf> {
    crate::output::state_file("convert-format")
}

/// The window, until it is closed or Convert has written what it was
/// opened on. `into` is a folder to write into under a name made from
/// the time, rather than beside each file; `forget` does away with the
/// files after, for a picture handed over to be converted.
pub fn run(files: &[PathBuf], into: Option<&Path>, forget: bool, options: &crate::options::Options) -> crate::x11::Result<()> {
    let result = shown(files, into, options);
    if forget {
        for file in files {
            let _ = std::fs::remove_file(file);
        }
    }
    result
}

fn shown(files: &[PathBuf], into: Option<&Path>, options: &crate::options::Options) -> crate::x11::Result<()> {
    let first = files.first().ok_or("--fine-tune needs a picture")?;
    let bytes = std::fs::read(first).map_err(|error| format!("cannot read {}: {error}", first.display()))?;
    let picture = convert::decode(&bytes).ok_or_else(|| format!("{} is not a picture hotshot reads", first.display()))?;
    let jpeg = bytes.starts_with(b"\xFF\xD8\xFF").then_some(bytes);
    // The kind chosen last time; else JPEG XL lossless, which keeps every
    // pixel of a screenshot at a fraction of a PNG. Never the picture's own.
    let own = first.extension().and_then(|ending| Format::named(&ending.to_string_lossy())).map(Format::ending);
    let kept = last_format().and_then(|file| std::fs::read_to_string(file).ok()).and_then(|word| Format::named(word.trim()));
    let format = match kept.filter(|kept| Some(kept.ending()) != own) {
        Some(kept) => kept,
        None if own == Some("jxl") => Format::Avif,
        None => Format::JxlLossless,
    };
    // A picture handed over from capture mode has a name of no meaning.
    let name = match into {
        Some(_) => "Capture".to_owned(),
        None => first.file_name().map_or(String::new(), |name| name.to_string_lossy().into_owned()),
    };
    let subtitle = match files.len() {
        1 => format!("{name} \u{b7} {} \u{d7} {}", picture.width, picture.height),
        count => format!("{count} pictures \u{b7} tried on {name}"),
    };
    let words = Words { title: "Convert".into(), subtitle, primary: if files.len() == 1 { "Convert  \u{23ce}" } else { "Convert all  \u{23ce}" }, secondary: None };
    let mut tuner = Tuner::new(Source::new(picture.width as u32, picture.height as u32, picture.pixels, jpeg), format, options.convert, words);

    let mut display = crate::x11::Display::open()?;
    display.prepare()?;
    let mut text = Text::load();
    let conn = &display.conn;
    let atom = |name: &str| -> crate::x11::Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let (mut width, mut height) = ((display.width - 80).min(1240), (display.height - 120).min(720));
    let window = conn.generate_id()?;
    let events = EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION | EventMask::KEY_PRESS | EventMask::KEY_RELEASE | EventMask::STRUCTURE_NOTIFY;
    let look = CreateWindowAux::new().background_pixel(VIEW_COLOR).event_mask(events);
    let (x, y) = ((display.width - width) / 2, (display.height - height) / 2);
    conn.create_window(x11rb::COPY_FROM_PARENT as u8, window, display.root, x as i16, y as i16, width as u16, height as u16, 0, WindowClass::INPUT_OUTPUT, x11rb::COPY_FROM_PARENT, &look)?;
    let title = format!("Convert \u{2014} {name} \u{2014} hotshot");
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes())?;
    conn.change_property8(PropMode::REPLACE, window, atom("_NET_WM_NAME")?, atom("UTF8_STRING")?, title.as_bytes())?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"hotshot\0Hotshot\0")?;
    let (protocols, delete) = (atom("WM_PROTOCOLS")?, atom("WM_DELETE_WINDOW")?);
    conn.change_property32(PropMode::REPLACE, window, protocols, AtomEnum::ATOM, &[delete])?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
    conn.map_window(window)?;
    conn.flush()?;

    let mut frame = display.pixels((width * height) as usize);
    let (mut pointer, mut divider, mut loupe) = ((0, 0), None::<i32>, false);
    let mut dirty = true;
    let mut shrunk = crate::tune::Shrunk::default();
    loop {
        let wait = tuner.busy().then_some(crate::tune::POLL);
        let mut events: Vec<Event> = match wait {
            Some(wait) => crate::studio::event_within(conn, wait)?.into_iter().collect(),
            None => vec![conn.wait_for_event()?],
        };
        while let Some(event) = conn.poll_for_event()? {
            events.push(event);
        }
        dirty |= tuner.poll();
        let mut asked = Asked::Nothing;
        for event in events {
            match event {
                Event::Expose(_) => dirty = true,
                Event::ConfigureNotify(configured) if configured.window == window => {
                    let now = (i32::from(configured.width).max(720), i32::from(configured.height).max(560));
                    if now != (width, height) {
                        (width, height) = now;
                        crate::x11::let_go(conn, std::mem::replace(&mut frame, display.pixels((width * height) as usize)));
                        dirty = true;
                    }
                }
                Event::MotionNotify(motion) => {
                    pointer = (i32::from(motion.event_x), i32::from(motion.event_y));
                    dirty |= tuner.moved(pointer.0, pointer.1);
                    let area = picture_area(width, height);
                    if area.contains(pointer.0, pointer.1) {
                        divider = Some(pointer.0);
                        dirty = true;
                    }
                }
                Event::ButtonPress(press) if press.detail == 1 => {
                    pointer = (i32::from(press.event_x), i32::from(press.event_y));
                    asked = tuner.press(pointer.0, pointer.1);
                    dirty = true;
                }
                Event::ButtonRelease(release) if release.detail == 1 => tuner.released(),
                Event::KeyPress(press) | Event::KeyRelease(press) => {
                    let down = matches!(event, Event::KeyPress(_));
                    let state: u16 = press.state.into();
                    let key = display.key(press.detail, state);
                    let mods = crate::x11::mods(state);
                    if matches!(key, Key::Char('z' | 'Z')) && tuner.typing.is_none() {
                        loupe = down;
                        dirty = true;
                        continue;
                    }
                    if !down {
                        continue;
                    }
                    dirty = true;
                    if tuner.typed(&key) {
                        continue;
                    }
                    match key {
                        Key::Escape => asked = Asked::Close,
                        Key::Char('q' | 'w' | 'Q' | 'W') if mods.ctrl => asked = Asked::Close,
                        Key::Enter => asked = Asked::Primary,
                        Key::Left => tuner.nudge(if mods.shift { -10.0 } else { -1.0 }),
                        Key::Right => tuner.nudge(if mods.shift { 10.0 } else { 1.0 }),
                        Key::Char('r' | 'R') => tuner.reset(),
                        _ => {}
                    }
                }
                Event::ClientMessage(message) if message.type_ == protocols && message.data.as_data32()[0] == delete => asked = Asked::Close,
                _ => {}
            }
        }
        match asked {
            Asked::Close => break,
            Asked::Primary | Asked::Secondary => {
                if let Some(file) = last_format() {
                    let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
                    let _ = std::fs::write(file, tuner.format.word());
                }
                match write(files, into, &tuner, options) {
                    Ok(written) => {
                        for path in written {
                            println!("{}", path.display());
                        }
                        break;
                    }
                    Err(error) => tuner.said = Some(error),
                }
            }
            Asked::Nothing => {}
        }
        if dirty {
            dirty = false;
            draw(&mut frame, width, height, &mut tuner, &mut shrunk, &mut text, pointer, divider, loupe);
            crate::x11::put_shared(conn, window, gc, width, height, &frame)?;
        }
        conn.flush()?;
    }
    Ok(())
}

/// Where the picture is shown: all of the window left of the panel.
fn picture_area(width: i32, height: i32) -> Rect {
    Rect::new(PAD, PAD, width - crate::tune::WIDTH - 3 * PAD, height - 2 * PAD)
}

/// The files written with the panel's settings: beside each, or the one
/// picture into the folder named; what was written, or why not.
fn write(files: &[PathBuf], into: Option<&Path>, tuner: &Tuner, options: &crate::options::Options) -> Result<Vec<PathBuf>, String> {
    let (format, settings) = (tuner.format, tuner.settings);
    if let Some(dir) = into {
        let source = &tuner.source;
        let bytes = convert::encode(source.width, source.height, &source.pixels, source.alpha, source.jpeg.as_deref(), format, &settings)?.bytes;
        return Ok(vec![crate::output::save(dir, &options.filename, format.ending(), &bytes)?]);
    }
    let mut written = Vec::new();
    let mut failed = Vec::new();
    for file in files {
        match convert::convert(file, format, &settings) {
            Ok((path, _)) => written.push(path),
            Err(error) => failed.push(error),
        }
    }
    match failed.first() {
        Some(error) if written.is_empty() => Err(error.clone()),
        Some(error) => Err(format!("{} written, {} not: {error}", written.len(), failed.len())),
        None => Ok(written),
    }
}

/// The window drawn: the picture split, its labels and the loupe, and the
/// panel beside it.
#[allow(clippy::too_many_arguments)]
fn draw(out: &mut [u32], width: i32, height: i32, tuner: &mut Tuner, shrunk: &mut crate::tune::Shrunk, text: &mut Text, pointer: (i32, i32), divider: Option<i32>, loupe: bool) {
    out.iter_mut().for_each(|pixel| *pixel = VIEW_COLOR);
    let mut canvas = Canvas::new(out, width, height);
    let area = picture_area(width, height);
    canvas.round_fill(area.x, area.y, area.w, area.h, 10, 0x171717, 256);
    let source = tuner.source.clone();
    let (sw, sh) = (source.width as i32, source.height as i32);
    let times = (area.w as f32 / sw as f32).min(area.h as f32 / sh as f32).min(1.0);
    let (wide, high) = (((sw as f32 * times) as i32).max(1), ((sh as f32 * times) as i32).max(1));
    let shown = Rect::new(area.x + (area.w - wide) / 2, area.y + (area.h - high) / 2, wide, high);
    let line = divider.unwrap_or(shown.x + shown.w / 2).clamp(shown.x, shown.right());
    let before = &source.flat;
    let preview = tuner.preview().map(|(pixels, format, size)| (pixels.to_vec(), format, size));
    // Made smaller to fit, each side averaged rather than picked from.
    let (small_before, small_after) = shrunk.of(tuner, wide, high);
    let small = |x: i32, y: i32| ((y - shown.y).clamp(0, high - 1) * wide + (x - shown.x).clamp(0, wide - 1)) as usize;
    match small_after {
        Some(after) => canvas.paint(shown, |x, y| if x < line { small_before[small(x, y)] } else { after[small(x, y)] } & 0xFF_FFFF),
        None => canvas.paint(shown, |x, y| small_before[small(x, y)] & 0xFF_FFFF),
    }
    canvas.fill(line - 1, shown.y, 2, shown.h, 0xFFFFFF);
    let label = |text: &mut Text, canvas: &mut Canvas, x: i32, says: &str, from_right: bool| {
        let wide = text.width(says, 11) + 16;
        let x = if from_right { x - wide } else { x };
        canvas.round_fill(x, shown.y + 10, wide, 22, 5, 0x000000, 184);
        text.draw(canvas, x + 8, shown.y + 10 + (22 - Text::line_height(11)) / 2, 11, 0xFFFFFF, says);
    };
    let png = tuner.png_size().map_or(String::new(), |png| format!(" \u{b7} {}", convert::size_words(png)));
    label(text, &mut canvas, shown.x + 10, &format!("Original{png}"), false);
    if let Some((_, _, size)) = &preview {
        label(text, &mut canvas, shown.right() - 10, &format!("{} \u{b7} {}", tuner.short(), convert::size_words(*size)), true);
    }
    if loupe && shown.contains(pointer.0, pointer.1) {
        let lens = Rect::new((pointer.0 + 24).min(area.right() - 228), (pointer.1 + 24).min(area.bottom() - 158), 220, 150);
        canvas.round_fill(lens.x - 2, lens.y - 2, lens.w + 4, lens.h + 4, 10, 0xFFFFFF, 256);
        let (cx, cy) = (lens.x + lens.w / 2, lens.y + lens.h / 2);
        let source_at = |x: i32, along: i32| (((x - along) as f32 + 0.5) / times) as i32;
        let (sx, sy, cut) = (source_at(pointer.0, shown.x), source_at(pointer.1, shown.y), source_at(line, shown.x));
        let after = preview.as_ref().map(|(after, ..)| after.as_slice());
        canvas.paint(lens, |x, y| {
            let (ax, ay) = (sx + (x - cx).div_euclid(LOUPE), sy + (y - cy).div_euclid(LOUPE));
            if ax < 0 || ay < 0 || ax >= sw || ay >= sh {
                return 0x1E1E1E;
            }
            let at = (ay * sw + ax) as usize;
            (match after {
                Some(after) if ax >= cut => after[at],
                _ => before[at],
            }) & 0xFF_FFFF
        });
        let mark = cx + (cut - sx) * LOUPE;
        if mark > lens.x && mark < lens.right() {
            canvas.fill(mark - 1, lens.y, 2, lens.h, crate::studio::ACCENT);
        }
        text.draw(&mut canvas, lens.x + 6, lens.bottom() - 18, 10, crate::studio::ACCENT, "800%");
    }
    let hint = "The line follows the pointer \u{b7} Z for a loupe \u{b7} \u{2190} \u{2192} the quality";
    text.draw(&mut canvas, area.x + 12, area.bottom() - 24, 11, DIM_TEXT_COLOR, hint);
    let panel = Rect::new(width - crate::tune::WIDTH - PAD, PAD, crate::tune::WIDTH, tuner.height());
    tuner.draw(&mut canvas, text, panel);

}
