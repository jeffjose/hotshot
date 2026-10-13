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
use crate::dbus::{self, Bus, Message, Value};
use crate::options::Options;
use crate::text::Text;
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

/// hotshot's own icon, as it is carried: Solar's camera in a color and
/// white, which reads on a dark panel and a light one alike. Made by
/// `tools/app_icon.py`.
const ICON: &[u8] = include_bytes!("../assets/hotshot.png");

/// The icon as pixels, so wide and tall, each with how opaque it is in
/// its top byte: as large as fits, in the middle, with nothing round it.
fn icon(wide: i32, high: i32) -> Vec<u32> {
    let Some((width, height, pixels)) = pin::read(ICON) else { return vec![0; (wide * high) as usize] };
    let side = wide.min(high).max(1);
    let small = pin::sized(&pixels, width, height, side, side);
    let mut out = vec![0u32; (wide * high) as usize];
    let (left, top) = ((wide - side) / 2, (high - side) / 2);
    for row in 0..side {
        out[((top + row) * wide + left) as usize..][..side as usize].copy_from_slice(&small[(row * side) as usize..][..side as usize]);
    }
    out
}

/// What hotshot is started again with, for something chosen.
fn start(told: &[&str]) -> Result<()> {
    let told: Vec<&std::ffi::OsStr> = told.iter().map(|word| word.as_ref()).collect();
    pin::again(&told)
}

/// The name the tray's keeper goes by on the bus, where desktops have
/// agreed on one; and where on this program the icon and its list are.
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";
const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
/// The number the line between the choices and the last of them goes
/// by, in the list; the choices are numbered from 1.
const LINE: i32 = 100;

/// What the icon says of itself, to whoever shows it.
fn item_says() -> Vec<(&'static str, Value)> {
    let pictures: Vec<Value> = [22, 24, 32, 48, 64]
        .into_iter()
        .map(|side| Value::Struct(vec![Value::I32(side), Value::I32(side), Value::Bytes(icon(side, side).into_iter().flat_map(u32::to_be_bytes).collect())]))
        .collect();
    let none = || Value::Array("(iiay)".into(), Vec::new());
    vec![
        ("Category", Value::text("ApplicationStatus")),
        ("Id", Value::text("hotshot")),
        ("Title", Value::text("hotshot")),
        ("Status", Value::text("Active")),
        ("WindowId", Value::I32(0)),
        ("IconName", Value::text("")),
        ("IconThemePath", Value::text("")),
        ("IconPixmap", Value::Array("(iiay)".into(), pictures)),
        ("OverlayIconName", Value::text("")),
        ("OverlayIconPixmap", none()),
        ("AttentionIconName", Value::text("")),
        ("AttentionIconPixmap", none()),
        ("AttentionMovieName", Value::text("")),
        ("ToolTip", Value::Struct(vec![Value::text(""), none(), Value::text("hotshot"), Value::text("Take a picture of the screen, or record it")])),
        ("ItemIsMenu", Value::Bool(false)),
        ("Menu", Value::Path(MENU_PATH.into())),
    ]
}

fn menu_says() -> Vec<(&'static str, Value)> {
    vec![("Version", Value::U32(3)), ("Status", Value::text("normal")), ("TextDirection", Value::text("ltr")), ("IconThemePath", Value::Array("s".into(), Vec::new()))]
}

/// What one line of the list is, by its number: what it says, or that
/// it is the line between.
fn line_says(id: i32) -> Option<Value> {
    if id == LINE {
        return Some(Value::named(vec![("type", Value::text("separator"))]));
    }
    let (name, told) = CHOICES.get(usize::try_from(id - 1).ok()?)?;
    let name = if *told == ["--record"] && record::running().is_some() { "Stop recording" } else { name };
    Some(Value::named(vec![("label", Value::text(name)), ("enabled", Value::Bool(true)), ("visible", Value::Bool(true))]))
}

/// The numbers of the list's lines, in order: the line between comes
/// before the last, which is the tray's own end.
fn lines() -> Vec<i32> {
    let mut ids: Vec<i32> = (1..=CHOICES.len() as i32).collect();
    ids.insert(ids.len() - 1, LINE);
    ids
}

/// Asks the tray's keeper to show the icon.
fn register(bus: &mut Bus) -> dbus::Result<()> {
    bus.ask(Message::call(WATCHER, "/StatusNotifierWatcher", WATCHER, "RegisterStatusNotifierItem", vec![Value::text(&bus.name.clone())])).map(|_| ())
}

/// Keeps the icon in a tray that is kept over the bus, as trays are on
/// every desktop that has one now: whoever keeps it draws the icon and
/// its list itself, and says here what was pressed. Until it is told
/// to quit; an error, if the bus goes.
fn kept(mut bus: Bus, options: &Options) -> dbus::Result<()> {
    // Told when the tray's keeper comes back, after its panel has been
    // started again: it has to be asked afresh.
    let rule = format!("type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{WATCHER}'");
    bus.ask(Message::call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "AddMatch", vec![Value::text(&rule)]))?;
    if options.debug {
        eprintln!("hotshot: the icon is in the tray kept over the bus, as {}", bus.name);
    }
    let (mut revision, mut recording) = (1u32, record::running().is_some());
    loop {
        let Some(asked) = bus.next(Some(Duration::from_secs(1)))? else {
            // What the list says changes when a recording starts or
            // stops: whoever shows it is told to ask again.
            let now = record::running().is_some();
            if now != recording {
                (recording, revision) = (now, revision + 1);
                bus.send(&Message::signal(MENU_PATH, MENU, "LayoutUpdated", vec![Value::U32(revision), Value::I32(0)]))?;
            }
            continue;
        };
        if asked.kind == dbus::SIGNAL {
            let came = asked.member == "NameOwnerChanged" && asked.body.get(2).and_then(Value::as_str).is_some_and(|owner| !owner.is_empty());
            if came {
                let _ = register(&mut bus);
            }
            continue;
        }
        if asked.kind != dbus::CALL {
            continue;
        }
        let says = if asked.path == MENU_PATH { menu_says() } else { item_says() };
        let text = |at: usize| asked.body.get(at).and_then(Value::as_str).unwrap_or("");
        let number = |at: usize| asked.body.get(at).and_then(Value::as_i32).unwrap_or(0);
        match (asked.interface.as_str(), asked.member.as_str()) {
            ("org.freedesktop.DBus.Properties", "Get") => match says.into_iter().find(|(name, _)| *name == text(1)) {
                Some((_, value)) => bus.answer(&asked, vec![Value::any(value)])?,
                None => bus.refuse(&asked, "there is no such property")?,
            },
            ("org.freedesktop.DBus.Properties", "GetAll") => bus.answer(&asked, vec![Value::named(says)])?,
            ("org.freedesktop.DBus.Peer", "Ping") => bus.answer(&asked, Vec::new())?,
            ("org.freedesktop.DBus.Introspectable", "Introspect") => bus.answer(&asked, vec![Value::text(described(&asked.path))])?,
            // The icon pressed: hotshot, as a key would start it; with
            // the middle button, the whole screen to be drawn on.
            (ITEM, "Activate") => {
                bus.answer(&asked, Vec::new())?;
                let _ = start(&[]);
            }
            (ITEM, "SecondaryActivate") => {
                bus.answer(&asked, Vec::new())?;
                let _ = start(&["--full", "--edit"]);
            }
            // Asked for its list by a tray that draws none itself:
            // hotshot's own, where it says, or where the pointer is.
            (ITEM, "ContextMenu") => {
                bus.answer(&asked, Vec::new())?;
                let at = (number(0), number(1));
                let chosen = Display::open().ok().and_then(|mut display| {
                    display.prepare().ok()?;
                    let at = if at == (0, 0) { display.pointer() } else { at };
                    menu(&display, &mut Text::load(), at, true).ok().flatten()
                });
                match chosen {
                    Some(["--quit"]) => return Ok(()),
                    Some(told) => drop(start(told)),
                    None => {}
                }
            }
            (ITEM, "Scroll") => bus.answer(&asked, Vec::new())?,
            // The list, for a tray that draws it: all of it at once,
            // each line a number, what it says, and nothing under it.
            (MENU, "GetLayout") => {
                let under: Vec<Value> = lines().into_iter().filter_map(|id| Some(Value::any(Value::Struct(vec![Value::I32(id), line_says(id)?, Value::Array("v".into(), Vec::new())])))).collect();
                let all = Value::Struct(vec![Value::I32(0), Value::named(vec![("children-display", Value::text("submenu"))]), Value::Array("v".into(), under)]);
                bus.answer(&asked, vec![Value::U32(revision), all])?;
            }
            (MENU, "GetGroupProperties") => {
                let wanted: Vec<i32> = match asked.body.first() {
                    Some(Value::Array(_, ids)) if !ids.is_empty() => ids.iter().filter_map(Value::as_i32).collect(),
                    _ => lines(),
                };
                let each: Vec<Value> = wanted.into_iter().filter_map(|id| Some(Value::Struct(vec![Value::I32(id), line_says(id)?]))).collect();
                bus.answer(&asked, vec![Value::Array("(ia{sv})".into(), each)])?;
            }
            (MENU, "GetProperty") => {
                let found = line_says(number(0)).and_then(|says| match says {
                    Value::Array(_, entries) => entries.into_iter().find_map(|entry| match entry {
                        Value::Entry(name, value) if name.as_str() == Some(text(1)) => Some(*value),
                        _ => None,
                    }),
                    _ => None,
                });
                match found {
                    Some(value) => bus.answer(&asked, vec![value])?,
                    None => bus.refuse(&asked, "there is no such property")?,
                }
            }
            (MENU, "AboutToShow") => bus.answer(&asked, vec![Value::Bool(false)])?,
            (MENU, "AboutToShowGroup") => bus.answer(&asked, vec![Value::Array("i".into(), Vec::new()), Value::Array("i".into(), Vec::new())])?,
            // A line of the list pressed, or several said at once.
            (MENU, "Event" | "EventGroup") => {
                let events: Vec<(i32, String)> = match (asked.member.as_str(), asked.body.first()) {
                    ("Event", _) => vec![(number(0), text(1).to_owned())],
                    (_, Some(Value::Array(_, events))) => events
                        .iter()
                        .filter_map(|event| match event {
                            Value::Struct(parts) => Some((parts.first()?.as_i32()?, parts.get(1)?.as_str()?.to_owned())),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                bus.answer(&asked, if asked.member == "Event" { Vec::new() } else { vec![Value::Array("i".into(), Vec::new())] })?;
                for (id, event) in events {
                    let Some((_, told)) = usize::try_from(id - 1).ok().and_then(|at| CHOICES.get(at)).filter(|_| event == "clicked") else { continue };
                    if *told == ["--quit"] {
                        return Ok(());
                    }
                    let _ = start(told);
                }
            }
            _ => bus.refuse(&asked, "hotshot's icon does nothing by that name")?,
        }
    }
}

/// What there is at a place on this program, said the way the bus has
/// such things said, for whoever asks before it asks anything else.
fn described(path: &str) -> &'static str {
    match path {
        ITEM_PATH => concat!(
            "<node><interface name=\"org.kde.StatusNotifierItem\">",
            "<property name=\"Category\" type=\"s\" access=\"read\"/><property name=\"Id\" type=\"s\" access=\"read\"/>",
            "<property name=\"Title\" type=\"s\" access=\"read\"/><property name=\"Status\" type=\"s\" access=\"read\"/>",
            "<property name=\"WindowId\" type=\"i\" access=\"read\"/><property name=\"IconName\" type=\"s\" access=\"read\"/>",
            "<property name=\"IconThemePath\" type=\"s\" access=\"read\"/><property name=\"IconPixmap\" type=\"a(iiay)\" access=\"read\"/>",
            "<property name=\"OverlayIconName\" type=\"s\" access=\"read\"/><property name=\"OverlayIconPixmap\" type=\"a(iiay)\" access=\"read\"/>",
            "<property name=\"AttentionIconName\" type=\"s\" access=\"read\"/><property name=\"AttentionIconPixmap\" type=\"a(iiay)\" access=\"read\"/>",
            "<property name=\"AttentionMovieName\" type=\"s\" access=\"read\"/><property name=\"ToolTip\" type=\"(sa(iiay)ss)\" access=\"read\"/>",
            "<property name=\"ItemIsMenu\" type=\"b\" access=\"read\"/><property name=\"Menu\" type=\"o\" access=\"read\"/>",
            "<method name=\"Activate\"><arg type=\"i\" direction=\"in\"/><arg type=\"i\" direction=\"in\"/></method>",
            "<method name=\"SecondaryActivate\"><arg type=\"i\" direction=\"in\"/><arg type=\"i\" direction=\"in\"/></method>",
            "<method name=\"ContextMenu\"><arg type=\"i\" direction=\"in\"/><arg type=\"i\" direction=\"in\"/></method>",
            "<method name=\"Scroll\"><arg type=\"i\" direction=\"in\"/><arg type=\"s\" direction=\"in\"/></method>",
            "<signal name=\"NewIcon\"/><signal name=\"NewStatus\"><arg type=\"s\"/></signal>",
            "</interface></node>"
        ),
        MENU_PATH => concat!(
            "<node><interface name=\"com.canonical.dbusmenu\">",
            "<property name=\"Version\" type=\"u\" access=\"read\"/><property name=\"Status\" type=\"s\" access=\"read\"/>",
            "<property name=\"TextDirection\" type=\"s\" access=\"read\"/><property name=\"IconThemePath\" type=\"as\" access=\"read\"/>",
            "<method name=\"GetLayout\"><arg type=\"i\" direction=\"in\"/><arg type=\"i\" direction=\"in\"/><arg type=\"as\" direction=\"in\"/><arg type=\"u\" direction=\"out\"/><arg type=\"(ia{sv}av)\" direction=\"out\"/></method>",
            "<method name=\"GetGroupProperties\"><arg type=\"ai\" direction=\"in\"/><arg type=\"as\" direction=\"in\"/><arg type=\"a(ia{sv})\" direction=\"out\"/></method>",
            "<method name=\"GetProperty\"><arg type=\"i\" direction=\"in\"/><arg type=\"s\" direction=\"in\"/><arg type=\"v\" direction=\"out\"/></method>",
            "<method name=\"Event\"><arg type=\"i\" direction=\"in\"/><arg type=\"s\" direction=\"in\"/><arg type=\"v\" direction=\"in\"/><arg type=\"u\" direction=\"in\"/></method>",
            "<method name=\"EventGroup\"><arg type=\"a(isvu)\" direction=\"in\"/><arg type=\"ai\" direction=\"out\"/></method>",
            "<method name=\"AboutToShow\"><arg type=\"i\" direction=\"in\"/><arg type=\"b\" direction=\"out\"/></method>",
            "<method name=\"AboutToShowGroup\"><arg type=\"ai\" direction=\"in\"/><arg type=\"ai\" direction=\"out\"/><arg type=\"ai\" direction=\"out\"/></method>",
            "<signal name=\"LayoutUpdated\"><arg type=\"u\"/><arg type=\"i\"/></signal>",
            "<signal name=\"ItemsPropertiesUpdated\"><arg type=\"a(ia{sv})\"/><arg type=\"a(ias)\"/></signal>",
            "</interface></node>"
        ),
        _ => "<node><node name=\"StatusNotifierItem\"/><node name=\"MenuBar\"/></node>",
    }
}

/// Puts the icon in the tray and keeps it there, doing what is asked
/// of it, until it is told to quit. In the tray kept over the bus,
/// where there is one: that is every desktop's, on Wayland and off it.
/// Failing that, in the tray X11 has always had.
pub fn run(options: &Options) -> Result<()> {
    // What it starts is not waited for, and is not to be left as the
    // husk of a process when it ends.
    // SAFETY: only how a signal is taken is changed.
    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };
    match Bus::session().and_then(|mut bus| register(&mut bus).map(|()| bus)) {
        Ok(bus) => kept(bus, options),
        Err(error) => {
            if options.debug {
                eprintln!("hotshot: no tray kept over the bus ({error}); trying the one X11 has");
            }
            docked(options)
        }
    }
}

/// Puts the icon in the tray X11 has, by asking whoever keeps it to
/// take a window in, and keeps it there until it is told to quit or
/// the display goes.
fn docked(options: &Options) -> Result<()> {
    let mut display = Display::open()?;
    display.prepare()?;
    let mut text = Text::load();
    let conn = &display.conn;
    let atom = |name: &str| -> Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let (tray, opcode, info, manager, visual_of) = (atom(&format!("_NET_SYSTEM_TRAY_S{}", display.screen))?, atom("_NET_SYSTEM_TRAY_OPCODE")?, atom("_XEMBED_INFO")?, atom("MANAGER")?, atom("_NET_SYSTEM_TRAY_VISUAL")?);
    // Told when a tray comes, where there is none yet.
    conn.change_window_attributes(display.root, &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY))?;

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

        let draw = |side: (i32, i32)| -> Result<()> {
            let mut pixels = icon(side.0, side.1);
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
                Event::Expose(expose) if expose.window == window && expose.count == 0 => draw(side)?,
                Event::ConfigureNotify(sized) if sized.window == window => {
                    let now = (i32::from(sized.width).max(1), i32::from(sized.height).max(1));
                    if now != side {
                        side = now;
                        draw(side)?;
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
