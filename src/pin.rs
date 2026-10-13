//! A picture left on the screen: in a window of its own with nothing
//! round it, over every other, to be dragged about, made larger and
//! smaller with the wheel, and done away with by Esc or a double click.
//!
//! Each is a process of its own, as the clipboard's keeper is: hotshot
//! starts itself again with the picture in a file, and is gone. Nothing
//! is running that is not showing a picture.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;

use crate::canvas::Rect;
use crate::editor::Key;
use crate::options::Options;
use crate::output;
use crate::x11::{self, Display, Result};

/// How long between two presses for them to be one double click, in
/// milliseconds; and how much larger or smaller a turn of the wheel
/// makes the picture.
const DOUBLE_CLICK: u32 = 400;
const STEP: f32 = 1.1;

/// Starts hotshot again to show a picture where it was taken, and
/// comes back at once. The picture goes by way of a file among what
/// hotshot keeps, which the one showing it does away with when it is
/// done.
pub fn leave(png: &[u8], at: (i32, i32)) -> Result<()> {
    let dir = output::state_file("pins").ok_or("there is nowhere to keep a picture to be left on the screen")?;
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos());
    let file = dir.join(format!("{}-{stamp}.png", std::process::id()));
    std::fs::write(&file, png)?;
    again(&["--show".as_ref(), file.as_os_str(), "--at".as_ref(), format!("{},{}", at.0, at.1).as_ref(), "--forget".as_ref()])
}

/// Starts hotshot again, told something else, and does not wait for it.
pub fn again(told: &[&std::ffi::OsStr]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(std::env::current_exe()?);
    command.args(told).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // Out of the terminal's hands, as the clipboard's keeper is: it
    // outlives what started it.
    // SAFETY: setsid is all that is done between the fork and the exec.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command.spawn()?;
    Ok(())
}

/// A PNG as pixels, each with how opaque it is in its top byte.
pub fn read(bytes: &[u8]) -> Option<(i32, i32, Vec<u32>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut data = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut data).ok()?;
    let pixels: Vec<u32> = match info.color_type {
        png::ColorType::Rgba => data.chunks_exact(4).map(|pixel| u32::from(pixel[3]) << 24 | u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2])).collect(),
        png::ColorType::GrayscaleAlpha => data.chunks_exact(2).map(|pixel| u32::from(pixel[1]) << 24 | u32::from(pixel[0]) * 0x010101).collect(),
        _ => return None,
    };
    let (width, height) = (info.width as i32, info.height as i32);
    (pixels.len() == (width * height) as usize && width > 0).then_some((width, height, pixels))
}

/// A picture made so wide and so tall: each pixel the average of those
/// it is made from where it is made smaller, and the nearest where it
/// is made larger.
pub fn sized(pixels: &[u32], width: i32, height: i32, wide: i32, high: i32) -> Vec<u32> {
    let mut out = Vec::with_capacity((wide * high) as usize);
    for y in 0..high {
        let (top, bottom) = (y * height / high, ((y + 1) * height / high).max(y * height / high + 1).min(height));
        for x in 0..wide {
            let (left, right) = (x * width / wide, ((x + 1) * width / wide).max(x * width / wide + 1).min(width));
            let (mut sum, mut count) = ([0u32; 4], 0u32);
            for row in top..bottom {
                for pixel in &pixels[(row * width + left) as usize..(row * width + right) as usize] {
                    [24, 16, 8, 0].iter().enumerate().for_each(|(part, shift)| sum[part] += pixel >> shift & 0xFF);
                    count += 1;
                }
            }
            let count = count.max(1);
            out.push((sum[0] / count) << 24 | (sum[1] / count) << 16 | (sum[2] / count) << 8 | sum[3] / count);
        }
    }
    out
}

/// Shows the picture in a file until it is done away with. `at` is
/// where its top-left corner goes: the middle of the screen, without.
pub fn show(file: &Path, at: Option<(i32, i32)>, options: &Options) -> Result<()> {
    let bytes = std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    let done = |result: Result<()>| {
        if options.forget {
            let _ = std::fs::remove_file(file);
        }
        result
    };
    let Some((width, height, pixels)) = read(&bytes) else { return done(Err(format!("{} is not a PNG hotshot can show", file.display()).into())) };
    done(shown(&bytes, width, height, &pixels, at, options))
}

fn shown(png: &[u8], width: i32, height: i32, pixels: &[u32], at: Option<(i32, i32)>, options: &Options) -> Result<()> {
    let mut display = Display::open()?;
    display.prepare()?;
    let conn = &display.conn;
    let atom = |name: &str| -> Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };

    // A window that keeps how opaque each pixel is, where there can be
    // one: rounded corners, and a shadow, show what is under them.
    let deep = x11::deep_visual(conn, display.screen);
    let (depth, visual) = deep.map_or((x11rb::COPY_FROM_PARENT as u8, x11rb::COPY_FROM_PARENT), |visual| (32, visual));
    let mut look = CreateWindowAux::new()
        .background_pixel(0)
        .border_pixel(0)
        .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::BUTTON1_MOTION | EventMask::KEY_PRESS | EventMask::STRUCTURE_NOTIFY);
    if let Some(visual) = deep {
        let colors = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colors, display.root, visual)?;
        look = look.colormap(colors);
    }
    let screen = Rect::new(0, 0, display.width, display.height);
    let mut place = Rect::new(0, 0, width, height);
    place = match at {
        Some((x, y)) => place.moved(x, y),
        None => place.moved((screen.w - width) / 2, (screen.h - height) / 2),
    };
    let window = conn.generate_id()?;
    conn.create_window(depth, window, display.root, place.x as i16, place.y as i16, place.w as u16, place.h as u16, 0, WindowClass::INPUT_OUTPUT, visual, &look)?;

    // Told to the window manager: nothing round it, over everything, on
    // every desktop, in no list of windows, and where it was put.
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, b"hotshot pin")?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"hotshot\0Hotshot\0")?;
    let hints = atom("_MOTIF_WM_HINTS")?;
    conn.change_property32(PropMode::REPLACE, window, hints, hints, &[2, 0, 0, 0, 0])?;
    conn.change_property32(PropMode::REPLACE, window, atom("_NET_WM_WINDOW_TYPE")?, AtomEnum::ATOM, &[atom("_NET_WM_WINDOW_TYPE_UTILITY")?])?;
    let states = [atom("_NET_WM_STATE_ABOVE")?, atom("_NET_WM_STATE_STICKY")?, atom("_NET_WM_STATE_SKIP_TASKBAR")?, atom("_NET_WM_STATE_SKIP_PAGER")?];
    conn.change_property32(PropMode::REPLACE, window, atom("_NET_WM_STATE")?, AtomEnum::ATOM, &states)?;
    let (protocols, delete) = (atom("WM_PROTOCOLS")?, atom("WM_DELETE_WINDOW")?);
    conn.change_property32(PropMode::REPLACE, window, protocols, AtomEnum::ATOM, &[delete])?;
    // Its place and size are the user's, which a window manager leaves.
    let sizes = [1 | 2, place.x as u32, place.y as u32, place.w as u32, place.h as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    conn.change_property32(PropMode::REPLACE, window, AtomEnum::WM_NORMAL_HINTS, AtomEnum::WM_SIZE_HINTS, &sizes)?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
    conn.map_window(window)?;
    conn.configure_window(window, &ConfigureWindowAux::new().x(place.x).y(place.y).stack_mode(StackMode::ABOVE))?;
    conn.flush()?;

    // Without a window that keeps how opaque, what would be see-through
    // is dark instead. And a line round a picture with square corners,
    // to tell it from what it is over.
    let edged = |pixels: &[u32], wide: i32, high: i32| -> Vec<u32> {
        let mut out: Vec<u32> = pixels.iter().map(|&pixel| if deep.is_some() { pixel } else { crate::canvas::mix(0x202020, pixel & 0xFF_FFFF, (pixel >> 24) + (pixel >> 31)) | 0xFF00_0000 }).collect();
        let square = [0, wide - 1, (high - 1) * wide, high * wide - 1].iter().all(|&corner| pixels[corner as usize] >> 24 == 255);
        if square && wide > 2 && high > 2 {
            for at in (0..wide).flat_map(|x| [x, (high - 1) * wide + x]).chain((0..high).flat_map(|y| [y * wide, y * wide + wide - 1])) {
                out[at as usize] = 0xFF00_0000 | crate::canvas::mix(out[at as usize] & 0xFF_FFFF, options.settings.accent, 150);
            }
        }
        out
    };
    let mut times = 1.0f32;
    let mut frame = edged(pixels, width, height);
    let draw = |frame: &[u32], place: Rect| x11::put(conn, window, gc, deep.is_some(), place.w, place.h, frame);

    // Where the pointer took hold of it, on the screen and in itself;
    // and when it was last pressed, to tell a double click.
    let (mut held, mut last): (Option<(i32, i32)>, u32) = (None, 0);
    loop {
        match conn.wait_for_event()? {
            Event::Expose(expose) if expose.count == 0 => draw(&frame, place)?,
            Event::ConfigureNotify(moved) if moved.window == window => {
                // Where the window manager says it now is, which may
                // not be where it was asked to be.
                if let Ok(Ok(found)) = conn.translate_coordinates(window, display.root, 0, 0).map(|cookie| cookie.reply()) {
                    (place.x, place.y) = (i32::from(found.dst_x), i32::from(found.dst_y));
                }
            }
            Event::ButtonPress(press) => match press.detail {
                1 => {
                    if press.time.wrapping_sub(last) < DOUBLE_CLICK {
                        break;
                    }
                    last = press.time;
                    held = Some((i32::from(press.root_x) - place.x, i32::from(press.root_y) - place.y));
                    conn.configure_window(window, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE))?;
                }
                3 => break,
                // The wheel: larger or smaller, about where the pointer
                // is, which stays over the same part of the picture.
                turn @ (4 | 5) => {
                    let wanted = (times * if turn == 4 { STEP } else { 1.0 / STEP }).clamp(0.1, 8.0);
                    let (wide, high) = (((width as f32 * wanted).round() as i32).clamp(16, 16_000), ((height as f32 * wanted).round() as i32).clamp(16, 16_000));
                    if (wide, high) != (place.w, place.h) {
                        let (px, py) = (i32::from(press.event_x), i32::from(press.event_y));
                        place = Rect::new(place.x + px - px * wide / place.w.max(1), place.y + py - py * high / place.h.max(1), wide, high);
                        times = wanted;
                        frame = edged(&sized(pixels, width, height, wide, high), wide, high);
                        conn.configure_window(window, &ConfigureWindowAux::new().x(place.x).y(place.y).width(wide as u32).height(high as u32))?;
                        draw(&frame, place)?;
                    }
                }
                _ => {}
            },
            Event::ButtonRelease(release) if release.detail == 1 => held = None,
            Event::MotionNotify(motion) => {
                if let Some((dx, dy)) = held {
                    // Having gone anywhere, the press was no half of a
                    // double click.
                    last = 0;
                    (place.x, place.y) = (i32::from(motion.root_x) - dx, i32::from(motion.root_y) - dy);
                    conn.configure_window(window, &ConfigureWindowAux::new().x(place.x).y(place.y))?;
                    conn.flush()?;
                }
            }
            Event::KeyPress(press) => {
                let state: u16 = press.state.into();
                match (display.key(press.detail, state), x11::mods(state).ctrl) {
                    (Key::Escape, _) | (Key::Char('q' | 'Q'), false) => break,
                    // Copied again, by a keeper of its own; and saved
                    // where pictures are saved.
                    (Key::Char('c' | 'C'), true) => {
                        let kept = keep(png)?;
                        again(&["--clip".as_ref(), kept.as_os_str(), "--forget".as_ref()])?;
                    }
                    (Key::Char('s' | 'S'), true) => drop(output::save(&options.dir, &options.filename, "png", png)),
                    // Back to the size it was taken at.
                    (Key::Char('0' | '1'), _) if (place.w, place.h) != (width, height) => {
                        (place.w, place.h, times) = (width, height, 1.0);
                        frame = edged(pixels, width, height);
                        conn.configure_window(window, &ConfigureWindowAux::new().width(width as u32).height(height as u32))?;
                        draw(&frame, place)?;
                    }
                    _ => {}
                }
            }
            Event::ClientMessage(message) if message.type_ == protocols && message.data.as_data32()[0] == delete => break,
            Event::DestroyNotify(_) => break,
            _ => {}
        }
        conn.flush()?;
    }
    Ok(())
}

/// A PNG put in a file of its own among what hotshot keeps, for
/// another hotshot to read and do away with.
fn keep(png: &[u8]) -> Result<PathBuf> {
    let dir = output::state_file("pins").ok_or("there is nowhere to keep a picture")?;
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos());
    let file = dir.join(format!("{}-{stamp}-copy.png", std::process::id()));
    std::fs::write(&file, png)?;
    Ok(file)
}
