//! An icon in the panel's tray, and the short list it opens: the one
//! part of hotshot that stays, and only when it is asked to with
//! `--tray`. A click on the icon is hotshot started; a right click is
//! the list of other ways to start it.
//!
//! The icon is put in the tray the way X11 programs always have been,
//! by asking whoever keeps the tray to take a window in. XFCE's panel
//! takes one so, and KDE's; GNOME has no tray of its own, and takes one
//! only with an extension that gives it one. `--menu` is the same list
//! with no icon, where the pointer is, for a key to be bound to.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use crate::canvas::{mix, Canvas, Rect};
use crate::options::Options;
use crate::text::{icons, Text};
use crate::x11::{self, Display, Result};
use crate::{pin, record};

const PANEL: u32 = 0x1F1F1F;
const EDGE: u32 = 0x3A3A3A;
const INK: u32 = 0xEEEEEE;
const FAINT: u32 = 0x9A9A9A;

/// What the list offers: what each is called, the key that does the
/// same from the editor or nothing, and what hotshot is told. Nothing
/// told at all is the tray's own end.
const CHOICES: [(&str, &[&str]); 7] = [
    ("Select an area", &[]),
    ("The whole screen", &["--full", "--edit"]),
    ("In three seconds", &["--delay", "3"]),
    ("The same area again", &["--again", "--edit"]),
    ("Reopen the last picture", &["--reopen"]),
    ("Record an area", &["--record"]),
    ("Quit", &["--quit"]),
];

/// The list, at a place on the screen, until something in it is chosen
/// or a click elsewhere puts it away. What hotshot is to be told, for
/// what was chosen. `leaving` says whether it has the last of them, to
/// end the tray by.
pub fn menu(display: &Display, text: &mut Text, at: (i32, i32), leaving: bool) -> Result<Option<&'static [&'static str]>> {
    let conn = &display.conn;
    let px = ((display.height as f32 / 1080.0 * 14.0).round() as u32).max(13);
    let (pad, row) = (px as i32 * 3 / 4, Text::line_height(px) + px as i32 * 2 / 3);
    let recording = record::running().is_some();
    let choices: Vec<(&str, &'static [&'static str])> = CHOICES.iter().filter(|(name, _)| leaving || *name != "Quit").map(|&(name, told)| (if recording && told == ["--record"] { "Stop recording" } else { name }, told)).collect();
    let wide = choices.iter().map(|(name, _)| text.width(name, px)).max().unwrap_or(0) + 2 * pad + px as i32;
    let high = choices.len() as i32 * row + pad;
    // Under the pointer and to its right, or over it and to its left
    // where the screen ends first.
    let screen = Rect::new(0, 0, display.width, display.height);
    let place = Rect::new(if at.0 + wide > screen.w { at.0 - wide } else { at.0 }, if at.1 + high > screen.h { at.1 - high } else { at.1 }, wide, high).kept_within(screen);

    let window = conn.generate_id()?;
    let look = CreateWindowAux::new().background_pixel(PANEL).border_pixel(0).override_redirect(1).event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION | EventMask::LEAVE_WINDOW);
    conn.create_window(x11rb::COPY_FROM_PARENT as u8, window, display.root, place.x as i16, place.y as i16, wide as u16, high as u16, 0, WindowClass::INPUT_OUTPUT, x11rb::COPY_FROM_PARENT, &look)?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
    conn.map_window(window)?;
    conn.configure_window(window, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE))?;
    conn.flush()?;
    // Every press anywhere comes here while it is up, so that one
    // outside it puts it away. The pointer may be another's for the
    // moment, the panel's whose icon was pressed: it is asked for
    // until it is had, or for a little while.
    let asked = Instant::now();
    let mut grabbed = false;
    while !grabbed && asked.elapsed() < Duration::from_millis(400) {
        let grab = conn.grab_pointer(false, window, EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION, GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE, CURRENT_TIME)?.reply()?;
        grabbed = grab.status == GrabStatus::SUCCESS;
        if !grabbed {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    let mut draw = |over: Option<usize>| -> Result<()> {
        let mut pixels = vec![PANEL; (wide * high) as usize];
        let mut canvas = Canvas::new(&mut pixels, wide, high);
        canvas.round_outline(0, 0, wide, high, 0, 1, EDGE);
        for (index, (name, told)) in choices.iter().enumerate() {
            let top = pad / 2 + index as i32 * row;
            if over == Some(index) {
                canvas.round_fill(pad / 3, top, wide - 2 * (pad / 3), row, px as i32 / 3, mix(PANEL, 0xFFFFFF, 34), 256);
            }
            let ink = if *told == ["--quit"] { FAINT } else { INK };
            text.draw(&mut canvas, pad, top + (row - Text::line_height(px)) / 2, px, ink, name);
        }
        x11::put(conn, window, gc, false, wide, high, &pixels)
    };
    let row_at = |x: i32, y: i32| (x >= 0 && x < wide && y >= pad / 2 && y < pad / 2 + choices.len() as i32 * row).then(|| ((y - pad / 2) / row) as usize);
    let mut over = None;
    // The press that opened it is let go a moment later, over it or
    // not: that letting go chooses nothing.
    let chosen = loop {
        match conn.wait_for_event()? {
            Event::Expose(expose) if expose.count == 0 => draw(over)?,
            Event::MotionNotify(motion) => {
                let now = row_at(i32::from(motion.event_x), i32::from(motion.event_y));
                if now != over {
                    over = now;
                    draw(over)?;
                }
            }
            Event::LeaveNotify(_) if over.is_some() => {
                over = None;
                draw(over)?;
            }
            Event::ButtonPress(press) => break row_at(i32::from(press.event_x), i32::from(press.event_y)),
            Event::ButtonRelease(release) if asked.elapsed() > Duration::from_millis(350) => {
                if let Some(row) = row_at(i32::from(release.event_x), i32::from(release.event_y)) {
                    break Some(row);
                }
            }
            _ => {}
        }
    };
    conn.ungrab_pointer(CURRENT_TIME)?;
    conn.destroy_window(window)?;
    conn.flush()?;
    Ok(chosen.map(|row| choices[row].1))
}

/// The list where the pointer is, and what was chosen from it done.
pub fn menu_once(_options: &Options) -> Result<()> {
    let mut display = Display::open()?;
    display.prepare()?;
    let mut text = Text::load();
    let at = display.pointer();
    if let Some(told) = menu(&display, &mut text, at, false)? {
        drop(display);
        let told: Vec<&std::ffi::OsStr> = told.iter().map(|word| word.as_ref()).collect();
        pin::again(&told)?;
    }
    Ok(())
}

/// The icon as pixels, so wide and tall, each with how opaque it is in
/// its top byte: the picture of a camera, in white, on nothing.
fn icon(text: &mut Text, wide: i32, high: i32, ink: u32) -> Vec<u32> {
    // Drawn in white on black, which leaves how bright each pixel is
    // as how much of the icon is in it.
    let mut pixels = vec![0u32; (wide * high) as usize];
    let px = (wide.min(high) as f32 * 0.72).round().max(8.0) as u32;
    text.draw_centered(&mut Canvas::new(&mut pixels, wide, high), wide / 2, high / 2, px, 0xFFFFFF, icons::CAMERA);
    pixels.iter().map(|pixel| (pixel & 0xFF) << 24 | ink).collect()
}

/// What the icon is drawn in: light on a dark panel, dark on a light
/// one. The panel is looked at just past each corner of the icon,
/// where it is least likely to have anything of its own.
fn ink_for(display: &Display, window: Window, side: (i32, i32)) -> u32 {
    let conn = &display.conn;
    let Ok(Ok(at)) = conn.translate_coordinates(window, display.root, 0, 0).map(|cookie| cookie.reply()) else { return INK };
    let (x, y) = (i32::from(at.dst_x), i32::from(at.dst_y));
    let mut bright = Vec::new();
    for (px, py) in [(x - 1, y), (x + side.0, y), (x - 1, y + side.1 - 1), (x + side.0, y + side.1 - 1)] {
        if px < 0 || py < 0 || px >= display.width || py >= display.height {
            continue;
        }
        if let Ok(Ok(found)) = conn.get_image(ImageFormat::Z_PIXMAP, display.root, px as i16, py as i16, 1, 1, !0).map(|cookie| cookie.reply()) {
            if let [blue, green, red, ..] = found.data[..] {
                bright.push((u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114) / 1000);
            }
        }
    }
    match bright.is_empty() || bright.iter().sum::<u32>() / (bright.len() as u32) < 140 {
        true => INK,
        false => 0x2E3436,
    }
}

/// Puts the icon in the tray and keeps it there, doing what is asked
/// of it, until it is told to quit or the display goes.
pub fn run(options: &Options) -> Result<()> {
    let mut display = Display::open()?;
    display.prepare()?;
    let mut text = Text::load();
    let conn = &display.conn;
    let atom = |name: &str| -> Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let (tray, opcode, info, manager, visual_of) = (atom(&format!("_NET_SYSTEM_TRAY_S{}", display.screen))?, atom("_NET_SYSTEM_TRAY_OPCODE")?, atom("_XEMBED_INFO")?, atom("MANAGER")?, atom("_NET_SYSTEM_TRAY_VISUAL")?);
    // Told when a tray comes, where there is none yet.
    conn.change_window_attributes(display.root, &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY))?;
    // What it starts is not waited for, and is not to be left as the
    // husk of a process when it ends.
    // SAFETY: only how a signal is taken is changed.
    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };

    loop {
        // Whoever keeps the tray: waited for, where nobody does yet.
        let mut keeper = conn.get_selection_owner(tray)?.reply()?.owner;
        while keeper == NONE {
            if options.debug {
                eprintln!("hotshot: no tray to put the icon in yet; waiting for one");
            }
            match conn.wait_for_event()? {
                Event::ClientMessage(message) if message.type_ == manager && message.data.as_data32()[1] == tray => keeper = message.data.as_data32()[2],
                _ => {}
            }
        }
        conn.change_window_attributes(keeper, &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY))?;
        // The tray says which visual it wants its icons in: one that
        // keeps how opaque each pixel is, mostly, which lets the panel
        // show round the icon. Otherwise the panel shows round it by
        // being what the window's own background is.
        let wanted = conn.get_property(false, keeper, visual_of, AtomEnum::VISUALID, 0, 1)?.reply().ok().and_then(|reply| reply.value32().and_then(|mut values| values.next()));
        let deep = wanted.filter(|&visual| conn.setup().roots[display.screen].allowed_depths.iter().any(|depth| depth.depth == 32 && depth.visuals.iter().any(|known| known.visual_id == visual)));
        let mut look = CreateWindowAux::new().border_pixel(0).event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::STRUCTURE_NOTIFY);
        look = match deep {
            Some(visual) => {
                let colors = conn.generate_id()?;
                conn.create_colormap(ColormapAlloc::NONE, colors, display.root, visual)?;
                look.background_pixel(0).colormap(colors)
            }
            None => look.background_pixmap(BackPixmap::PARENT_RELATIVE),
        };
        let (window, mut side) = (conn.generate_id()?, (24, 24));
        conn.create_window(if deep.is_some() { 32 } else { x11rb::COPY_FROM_PARENT as u8 }, window, display.root, 0, 0, 24, 24, 0, WindowClass::INPUT_OUTPUT, deep.unwrap_or(x11rb::COPY_FROM_PARENT), &look)?;
        conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, b"hotshot")?;
        conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"hotshot\0Hotshot\0")?;
        // It is to be shown, once it has been taken in; and it is no
        // smaller than this, which a tray that is not told takes to
        // be no size at all, and puts where it cannot be seen.
        conn.change_property32(PropMode::REPLACE, window, info, info, &[0, 1])?;
        let sizes = [1 << 4, 0, 0, 0, 0, 24, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        conn.change_property32(PropMode::REPLACE, window, AtomEnum::WM_NORMAL_HINTS, AtomEnum::WM_SIZE_HINTS, &sizes)?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
        // And the asking: take this window in.
        let ask = ClientMessageEvent::new(32, keeper, opcode, [CURRENT_TIME, 0, window, 0, 0]);
        conn.send_event(false, keeper, EventMask::NO_EVENT, ask)?;
        conn.flush()?;
        if options.debug {
            eprintln!("hotshot: asked the tray ({keeper:#x}) to take the icon in, {}", if deep.is_some() { "see-through" } else { "on the panel's own background" });
        }

        let draw = |text: &mut Text, side: (i32, i32)| -> Result<()> {
            let mut pixels = icon(text, side.0, side.1, ink_for(&display, window, side));
            if deep.is_none() {
                // What the panel has under it is put there by the
                // display; it is read back, and the icon laid on it.
                conn.clear_area(false, window, 0, 0, 0, 0)?;
                let Ok(under) = conn.get_image(ImageFormat::Z_PIXMAP, window, 0, 0, side.0 as u16, side.1 as u16, !0)?.reply() else { return Ok(()) };
                for (pixel, bytes) in pixels.iter_mut().zip(under.data.chunks_exact(4)) {
                    let behind = u32::from(bytes[2]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[0]);
                    *pixel = mix(behind, *pixel & 0xFF_FFFF, (*pixel >> 24) + (*pixel >> 31));
                }
            }
            x11::put(conn, window, gc, deep.is_some(), side.0, side.1, &pixels)
        };
        // Until the tray goes, taking the icon with it: then another is
        // waited for.
        loop {
            match conn.wait_for_event()? {
                Event::Expose(expose) if expose.window == window && expose.count == 0 => draw(&mut text, side)?,
                Event::ConfigureNotify(sized) if sized.window == window => {
                    let now = (i32::from(sized.width).max(1), i32::from(sized.height).max(1));
                    if now != side {
                        side = now;
                        draw(&mut text, side)?;
                    }
                }
                Event::ReparentNotify(taken) if taken.window == window && options.debug => eprintln!("hotshot: the icon is in the tray (in {:#x})", taken.parent),
                Event::ButtonPress(press) if press.event == window => {
                    let told = match press.detail {
                        // A click is hotshot as it is started by a key.
                        1 => Some(&[][..]),
                        2 => Some(&["--full", "--edit"][..]),
                        3 => menu(&display, &mut text, (i32::from(press.root_x), i32::from(press.root_y)), true)?,
                        _ => None,
                    };
                    match told {
                        Some(["--quit"]) => return Ok(()),
                        Some(told) => {
                            let told: Vec<&std::ffi::OsStr> = told.iter().map(|word| word.as_ref()).collect();
                            pin::again(&told)?;
                        }
                        None => {}
                    }
                }
                Event::DestroyNotify(gone) if gone.window == keeper || gone.window == window => break,
                _ => {}
            }
            conn.flush()?;
        }
        let _ = conn.destroy_window(window);
        conn.flush()?;
    }
}
