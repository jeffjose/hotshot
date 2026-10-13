//! A recording of an area of the screen, where a picture is one moment
//! of it. The recording itself is ffmpeg's doing, which reads the
//! screen and packs it faster than anything written here would; what
//! is here is the line round the area, the button that stops it, the
//! keys pressed said along its bottom, and the making of a GIF or the
//! like out of what ffmpeg recorded.
//!
//! One recording at a time: starting hotshot to record again, while it
//! is recording, stops the one there is. That is what lets one key both
//! start and stop.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::canvas::{Canvas, Rect};
use crate::options::Options;
use crate::output::{self, Video};
use crate::text::Text;
use crate::x11::{self, Display, Result};

/// The color of the line round what is being recorded, and of the dot
/// that says it is.
const RED: u32 = 0xE5484D;
/// What the button and the keys are drawn on, and in; and how opaque
/// the first is, of 255, where what is under it can show through.
const PANEL: u32 = 0x161616;
const INK: u32 = 0xF2F2F2;
const PANEL_OPAQUE: f32 = 224.0;
/// How long the keys last pressed go on being said, with none after.
const KEYS_LAST: Duration = Duration::from_millis(2200);
/// How many letters of them are said at once: the latest.
const KEYS_MOST: usize = 28;

/// Told to stop: by another hotshot, or by whoever stops programs.
static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn told_to_stop(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

/// Says that it is recording and how to stop, and has Enter or Ctrl+D
/// stop it where hotshot was started from a terminal: there is someone
/// at it to press them. Started by a key, it has nothing to read from,
/// and what would look like Ctrl+D at once is not taken for one.
/// `button` says whether there is a button on the screen to say too.
fn told_how_to_stop(button: bool) {
    // SAFETY: asks whether a descriptor is a terminal, and no more.
    let (typed, seen) = unsafe { (libc::isatty(0) == 1, libc::isatty(2) == 1) };
    if typed {
        std::thread::spawn(|| {
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            STOP.store(true, Ordering::SeqCst);
        });
    }
    let also = if button { "the button beside it, or " } else { "" };
    match (typed, seen || !button) {
        (true, _) => eprintln!("hotshot: recording. Press Enter here to stop (Ctrl+D does too); so does {also}`hotshot --record` again."),
        (false, true) => eprintln!("hotshot: recording. To stop: {also}`hotshot --record` again."),
        _ => {}
    }
}

/// Said once it has been stopped, where what was recorded has still to
/// be made the file that was asked for: that takes a moment, and the
/// terminal would otherwise say nothing while it does.
fn told_it_stopped(making: bool) {
    // SAFETY: asks whether a descriptor is a terminal, and no more.
    if unsafe { libc::isatty(2) } == 1 {
        eprintln!("hotshot: stopped{}", if making { "; making the file..." } else { "." });
    }
}

/// Where it is written down which hotshot is recording.
fn marker() -> Option<PathBuf> {
    output::state_file("recording")
}

/// Whether a recording is being made by another hotshot.
pub fn running() -> Option<i32> {
    let pid: i32 = std::fs::read_to_string(marker()?).ok()?.trim().parse().ok()?;
    // SAFETY: a signal of 0 is sent to nothing; it only asks whether
    // there is such a process to send one to.
    (pid > 0 && unsafe { libc::kill(pid, 0) } == 0).then_some(pid)
}

/// Stops the recording another hotshot is making, if one is. Whether
/// there was one to stop.
pub fn stop_other() -> bool {
    match running() {
        Some(pid) => {
            // SAFETY: the process is one of hotshot's own, which takes
            // this signal to mean what is meant by it here.
            unsafe { libc::kill(pid, libc::SIGUSR1) };
            true
        }
        None => {
            if let Some(file) = marker() {
                let _ = std::fs::remove_file(file);
            }
            false
        }
    }
}

/// A small window over everything, that no window manager has a hand
/// in: the button, the keys, and each side of the line. It keeps how
/// opaque its pixels are where the display will blend them, and is
/// square and solid where it will not.
struct Panel {
    window: Window,
    gc: Gcontext,
    deep: bool,
    shown: bool,
}

impl Panel {
    fn new(conn: &RustConnection, display: &Display, deep: Option<Visualid>, color: u32) -> Result<Panel> {
        let (depth, visual) = deep.map_or((x11rb::COPY_FROM_PARENT as u8, x11rb::COPY_FROM_PARENT), |visual| (32, visual));
        let mut look = CreateWindowAux::new().background_pixel(if deep.is_some() { 0 } else { color }).border_pixel(0).override_redirect(1).event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS);
        if let Some(visual) = deep {
            let colors = conn.generate_id()?;
            conn.create_colormap(ColormapAlloc::NONE, colors, display.root, visual)?;
            look = look.colormap(colors);
        }
        let window = conn.generate_id()?;
        conn.create_window(depth, window, display.root, -10, -10, 1, 1, 0, WindowClass::INPUT_OUTPUT, visual, &look)?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
        Ok(Panel { window, gc, deep: deep.is_some(), shown: false })
    }

    /// Puts it somewhere, showing a picture.
    fn show(&mut self, conn: &RustConnection, place: Rect, pixels: &[u32]) -> Result<()> {
        conn.configure_window(self.window, &ConfigureWindowAux::new().x(place.x).y(place.y).width(place.w as u32).height(place.h as u32).stack_mode(StackMode::ABOVE))?;
        if !std::mem::replace(&mut self.shown, true) {
            conn.map_window(self.window)?;
        }
        x11::put(conn, self.window, self.gc, self.deep, place.w, place.h, pixels)
    }

    fn hide(&mut self, conn: &RustConnection) -> Result<()> {
        if std::mem::replace(&mut self.shown, false) {
            conn.unmap_window(self.window)?;
            conn.flush()?;
        }
        Ok(())
    }
}

/// A patch with round corners and some words on it, as pixels: each
/// with how opaque it is in its top byte. `dot` puts a red dot before
/// the words, and `stop` a square after them, which is what the button
/// that stops a recording has.
fn patch(text: &mut Text, px: u32, words: &str, dot: bool, stop: bool) -> (i32, i32, Vec<u32>) {
    let (size, pad) = (px as i32, px as i32 * 3 / 4);
    let lead = if dot { size } else { 0 };
    let tail = if stop { size + pad / 2 } else { 0 };
    let (wide, high) = (text.width(words, px) + 2 * pad + lead + tail, Text::line_height(px) + pad);
    let mut pixels = vec![PANEL; (wide * high) as usize];
    let mut canvas = Canvas::new(&mut pixels, wide, high);
    if dot {
        canvas.disc(pad + size / 3, high / 2, size / 3, RED, 256);
    }
    text.draw(&mut canvas, pad + lead, (high - Text::line_height(px)) / 2, px, INK, words);
    if stop {
        let side = size * 2 / 3;
        canvas.round_fill(wide - pad - side, (high - side) / 2, side, side, side / 5, INK, 256);
    }
    // How opaque: all of it nearly, and none past its round corners.
    let radius = (high as f32 / 3.0).min(wide as f32 / 2.0);
    for (at, pixel) in pixels.iter_mut().enumerate() {
        let (x, y) = ((at as i32 % wide) as f32 + 0.5 - wide as f32 / 2.0, (at as i32 / wide) as f32 + 0.5 - high as f32 / 2.0);
        let (across, down) = (x.abs() - (wide as f32 / 2.0 - radius), y.abs() - (high as f32 / 2.0 - radius));
        let away = across.max(0.0).hypot(down.max(0.0)) + across.max(down).min(0.0) - radius;
        *pixel = *pixel & 0xFF_FFFF | (((0.5 - away).clamp(0.0, 1.0) * PANEL_OPAQUE) as u32) << 24;
    }
    (wide, high, pixels)
}

/// What a key is called, as it is said along the bottom of a
/// recording: a letter is itself, and the rest have a sign or a short
/// name. Nothing for a key that is only held for another, as Shift is.
pub fn key_name(symbol: u32) -> Option<String> {
    Some(match symbol {
        0x20 => "\u{2423}".into(),
        0xFF08 => "\u{232b}".into(),
        0xFF09 | 0xFE20 => "\u{21e5}".into(),
        0xFF0D | 0xFF8D => "\u{23ce}".into(),
        0xFF1B => "Esc".into(),
        0xFFFF | 0xFF9F => "Del".into(),
        0xFF50 | 0xFF95 => "Home".into(),
        0xFF57 | 0xFF9C => "End".into(),
        0xFF55 | 0xFF9A => "PgUp".into(),
        0xFF56 | 0xFF9B => "PgDn".into(),
        0xFF63 | 0xFF9E => "Ins".into(),
        0xFF51 | 0xFF96 => "\u{2190}".into(),
        0xFF52 | 0xFF97 => "\u{2191}".into(),
        0xFF53 | 0xFF98 => "\u{2192}".into(),
        0xFF54 | 0xFF99 => "\u{2193}".into(),
        0xFF61 => "PrtSc".into(),
        0xFFBE..=0xFFC9 => format!("F{}", symbol - 0xFFBD),
        0xFFAA..=0xFFB9 => char::from_u32(symbol - 0xFF80)?.to_string(),
        0x21..=0x7E | 0xA0..=0xFF => char::from_u32(symbol)?.to_string(),
        0x0100_0000..=0x0110_FFFF => char::from_u32(symbol - 0x0100_0000)?.to_string(),
        _ => return None,
    })
}

/// Which of the things held with a key a key is, as the display counts
/// them: Shift, Ctrl, Alt, the logo key, or the one for a keyboard's
/// third row of signs. None for a key that is pressed for itself.
pub fn held_by(symbol: u32) -> Option<u16> {
    match symbol {
        0xFFE1 | 0xFFE2 => Some(0x01),
        0xFFE3 | 0xFFE4 => Some(0x04),
        0xFFE7..=0xFFEA => Some(0x08),
        0xFFEB | 0xFFEC => Some(0x40),
        0xFE03 | 0xFF7E => Some(0x80),
        _ => None,
    }
}

/// A key pressed, as it is said: with what was held with it before it,
/// but for Shift with a letter, which is only how the letter was typed.
/// `state` is what was held, as the display counts it.
pub fn key_said(symbol: u32, state: u16) -> Option<String> {
    let name = key_name(symbol)?;
    let (shift, ctrl, alt, logo) = (state & 0x01 != 0, state & 0x04 != 0, state & 0x08 != 0, state & 0x40 != 0);
    let typed = name.chars().count() == 1 && !matches!(symbol, 0xFF00..=0xFFFF);
    let mut said = String::new();
    for (held, word) in [(ctrl, "Ctrl"), (alt, "Alt"), (logo, "Super"), (shift && (!typed || ctrl || alt || logo), "Shift")] {
        if held {
            said.push_str(word);
            said.push('+');
        }
    }
    // With something held, a letter is said as the key it is on.
    match (said.is_empty(), typed) {
        (false, true) => said.push_str(&name.to_uppercase()),
        _ => said.push_str(&name),
    }
    Some(said)
}

/// The keys pressed of late, as one line: letters typed one after
/// another run together, and anything else stands apart from them.
#[derive(Default)]
pub struct Keys {
    said: Vec<String>,
}

impl Keys {
    pub fn pressed(&mut self, said: String) {
        self.said.push(said);
        let over = self.said.len().saturating_sub(KEYS_MOST);
        self.said.drain(..over);
    }

    pub fn clear(&mut self) {
        self.said.clear();
    }

    pub fn line(&self) -> String {
        let mut line = String::new();
        let mut last_long = false;
        for (at, said) in self.said.iter().enumerate() {
            let long = said.chars().count() > 1;
            if at > 0 && (long || last_long) {
                line.push(' ');
            }
            line.push_str(said);
            last_long = long;
        }
        line
    }
}

/// Where a recording is to be saved, and as what kind of file: what
/// was asked for by name, or a name made from the time in the folder
/// pictures are saved to.
fn destination(options: &Options) -> Result<(PathBuf, Video)> {
    if let Some(file) = options.output.as_ref().filter(|file| file.as_path() != Path::new("-")) {
        let kind = file.extension().and_then(|ending| Video::named(&ending.to_string_lossy())).unwrap_or(options.video);
        if let Some(dir) = file.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        return Ok((file.clone(), kind));
    }
    let dir = options.save_to.as_ref().unwrap_or(&options.dir);
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot make {}: {error}", dir.display()))?;
    let name = output::dated(&options.filename).replace("screenshot", "recording");
    let ending = options.video.ending();
    let file = (1..).map(|copy| dir.join(if copy == 1 { format!("{name}.{ending}") } else { format!("{name}-{copy}.{ending}") })).find(|file| !file.exists()).expect("some name is free");
    Ok((file, options.video))
}

/// Runs ffmpeg to the end, and says what went wrong if anything did.
fn ffmpeg(told: &[&str]) -> Result<()> {
    let done = Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-y"]).args(told).stdin(Stdio::null()).output().map_err(|error| format!("cannot run ffmpeg: {error}"))?;
    match done.status.success() {
        true => Ok(()),
        false => Err(format!("ffmpeg could not make the recording: {}", String::from_utf8_lossy(&done.stderr).trim()).into()),
    }
}

/// Makes a GIF, an APNG or a WebP out of what was recorded. A GIF by
/// gifski where that is installed, which makes a far better one than
/// ffmpeg; and no more than 50 frames a second, which is as fast as a
/// GIF can say it goes.
///
/// `cut` is how it is cut down first, as ffmpeg has such things said:
/// for a recording that is of more of the screen than was asked for.
fn convert(from: &Path, to: &Path, kind: Video, fps: u32, cut: Option<&str>) -> Result<()> {
    let (from, to) = (from.to_string_lossy(), to.to_string_lossy());
    // Nothing done to it is `null`, to ffmpeg.
    let first = cut.unwrap_or("null");
    match kind {
        // An MP4 recorded as one is done already; one cut out of a
        // recording of something else is made now.
        Video::Mp4 if cut.is_none() && from == to => Ok(()),
        Video::Mp4 => ffmpeg(&["-i", &from, "-vf", first, "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-an", &to]),
        Video::Gif => {
            let fps = fps.min(50).to_string();
            let frames = Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-i", &from, "-vf", first, "-r", &fps, "-pix_fmt", "yuv444p", "-f", "yuv4mpegpipe", "-"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
            if let Ok(mut frames) = frames {
                let pipe = frames.stdout.take().expect("its output was asked for");
                let made = Command::new("gifski").args(["--quiet", "--fps", &fps, "--quality", "90", "-o", &to, "-"]).stdin(pipe).stdout(Stdio::null()).stderr(Stdio::null()).status();
                let _ = frames.wait();
                if made.is_ok_and(|status| status.success()) && Path::new(&*to).exists() {
                    return Ok(());
                }
            }
            let through = format!("{first},fps={fps},split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a");
            ffmpeg(&["-i", &from, "-vf", &through, "-loop", "0", &to])
        }
        Video::Apng => ffmpeg(&["-i", &from, "-vf", first, "-plays", "0", "-f", "apng", &to]),
        Video::Webp => ffmpeg(&["-i", &from, "-vf", first, "-c:v", "libwebp_anim", "-lossless", "0", "-q:v", "82", "-loop", "0", "-an", &to]),
    }
}

/// Whether a program by a name is to be found.
pub fn installed(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// What GStreamer packs a recording with, of what it has here: the
/// best of them first, and at the last one it always has.
fn packer() -> &'static str {
    let has = |element: &str| Command::new("gst-inspect-1.0").args(["--exists", element]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success());
    if has("x264enc") {
        "x264enc speed-preset=ultrafast pass=quant quantizer=16 key-int-max=120"
    } else if has("openh264enc") {
        "openh264enc bitrate=24000000 complexity=low ! h264parse"
    } else if has("vp8enc") {
        "vp8enc deadline=1 cpu-used=8 threads=8 target-bitrate=16000000 keyframe-max-dist=120"
    } else {
        "jpegenc quality=92"
    }
}

/// Records an area under Wayland, where the screen is not read but
/// handed over by the desktop as it changes: a monitor of it, by a
/// stream of PipeWire's, which GStreamer packs as it comes and ffmpeg
/// cuts the area out of afterwards. Until hotshot is started to record
/// again, the time it was given runs out, or the desktop is told to
/// stop handing over. There is no line round the area and no button,
/// which the desktop would not keep over what is recorded; it shows
/// that the screen is being handed over itself.
///
/// `HOTSHOT_GST_SOURCE` names what GStreamer is to record instead, of
/// all of `screen`, for trying this with no desktop to ask.
pub fn run_cast(area: Rect, screen: Rect, options: &Options) -> Result<PathBuf> {
    if !installed("gst-launch-1.0") {
        return Err("recording under Wayland is done by GStreamer (gst-launch-1.0, with its PipeWire plugin), which is not installed".into());
    }
    let (file, kind) = destination(options)?;
    let raw = output::state_file("recording.mkv").ok_or("there is nowhere to keep a recording while it is made")?;
    if let Some(dir) = raw.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let (source, place, mut cast) = match std::env::var("HOTSHOT_GST_SOURCE") {
        Ok(source) => (source, screen, None),
        Err(_) => {
            let cast = crate::wayland::cast(area, options.record_pointer).map_err(|error| format!("the desktop would not hand the screen over to be recorded: {error}"))?;
            (format!("pipewiresrc path={} do-timestamp=true keepalive-time=1000", cast.node), cast.place, Some(cast))
        }
    };
    // SAFETY: the handler does nothing but set a flag.
    unsafe {
        for signal in [libc::SIGUSR1, libc::SIGINT, libc::SIGTERM] {
            libc::signal(signal, told_to_stop as extern "C" fn(libc::c_int) as libc::sighandler_t);
        }
    }
    if let Some(marker) = marker() {
        std::fs::write(marker, std::process::id().to_string())?;
    }
    let (rate, to) = (format!("video/x-raw,framerate={}/1", options.fps), format!("location={}", raw.display()));
    let mut told: Vec<&str> = vec!["-e", "-q"];
    told.extend(source.split_whitespace());
    told.extend(["!", "videoconvert", "!", "videorate", "!", &rate, "!", "queue", "!"]);
    told.extend(packer().split_whitespace());
    told.extend(["!", "matroskamux", "!", "filesink", &to]);
    let mut recorder = Command::new("gst-launch-1.0").args(&told).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|error| format!("cannot run gst-launch-1.0: {error}"))?;
    told_how_to_stop(false);

    let began = Instant::now();
    loop {
        // The bus is listened to as the time passes: the desktop says
        // on it when it has been told to stop handing over.
        let closed = match cast.as_mut().map(|cast| cast.bus.next(Some(Duration::from_millis(50)))) {
            Some(Ok(Some(said))) => said.kind == crate::dbus::SIGNAL && said.member == "Closed",
            Some(Err(_)) => true,
            Some(Ok(None)) => false,
            None => {
                std::thread::sleep(Duration::from_millis(50));
                false
            }
        };
        let over = options.duration > 0.0 && began.elapsed().as_secs_f64() >= options.duration;
        if closed || over || STOP.load(Ordering::SeqCst) || recorder.try_wait()?.is_some() {
            break;
        }
    }
    // Told to end, GStreamer writes the end of its file; it is given a
    // while to, and ended anyway after that.
    told_it_stopped(true);
    // SAFETY: the process is one this started, and is told to stop.
    unsafe { libc::kill(recorder.id() as i32, libc::SIGINT) };
    let asked = Instant::now();
    while recorder.try_wait()?.is_none() {
        if asked.elapsed() > Duration::from_secs(10) {
            let _ = recorder.kill();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let done = recorder.wait_with_output()?;
    drop(cast);
    if let Some(marker) = marker() {
        let _ = std::fs::remove_file(marker);
    }
    if std::fs::metadata(&raw).map_or(0, |file| file.len()) == 0 {
        return Err(format!("GStreamer recorded nothing: {}", String::from_utf8_lossy(&done.stderr).trim()).into());
    }
    // The area out of the monitor recorded, as parts of it: how many
    // pixels the recording is across is the desktop's affair, and need
    // not be how many the display counts.
    let part = area.intersect(place);
    let part = if part.is_empty() { place } else { part };
    let of = |length: i32, whole: i32| length as f64 / whole.max(1) as f64;
    let cut = format!("crop=trunc(iw*{:.6}/2)*2:trunc(ih*{:.6}/2)*2:trunc(iw*{:.6}):trunc(ih*{:.6})", of(part.w, place.w), of(part.h, place.h), of(part.x - place.x, place.w), of(part.y - place.y, place.h));
    if !installed("ffmpeg") {
        // With nothing to cut it or make it another kind of file: all
        // of the monitor, as GStreamer packed it.
        let whole = file.with_extension("mkv");
        std::fs::rename(&raw, &whole).or_else(|_| std::fs::copy(&raw, &whole).map(|_| ()))?;
        eprintln!("hotshot: ffmpeg is not installed, so this is all of the monitor and not the area, as GStreamer recorded it");
        return Ok(whole);
    }
    let made = convert(&raw, &file, kind, options.fps, Some(&cut));
    let _ = std::fs::remove_file(&raw);
    made?;
    Ok(file)
}

/// Records an area of the screen until it is stopped: by the button
/// beside it, by hotshot being started to record again, or by the time
/// it was given running out. Gives the file it made.
pub fn run(area: Rect, options: &Options, mut text: Text) -> Result<PathBuf> {
    let mut display = Display::open()?;
    display.prepare()?;
    let screen = Rect::new(0, 0, display.width, display.height);
    // Whole pixels in twos, each way, which is what an MP4 is made of.
    let area = area.intersect(screen);
    let area = Rect::new(area.x, area.y, area.w & !1, area.h & !1);
    if area.w < 16 || area.h < 16 {
        return Err("that is too small an area to record".into());
    }
    let (file, kind) = destination(options)?;
    // An MP4 is recorded as what it will be; anything else is made
    // afterwards, out of a recording that has lost nothing.
    let raw = match kind {
        Video::Mp4 => file.clone(),
        _ => output::state_file("recording.mkv").ok_or("there is nowhere to keep a recording while it is made")?,
    };
    if let Some(dir) = raw.parent() {
        std::fs::create_dir_all(dir)?;
    }

    // The signs that it is recording, none of them in what is recorded:
    // a line round the area, just outside it, and a button beside it
    // wherever there is room off it.
    let conn = &display.conn;
    let composited = conn.get_selection_owner(conn.intern_atom(false, format!("_NET_WM_CM_S{}", display.screen).as_bytes())?.reply()?.atom)?.reply()?.owner != x11rb::NONE;
    let deep = x11::deep_visual(conn, display.screen).filter(|_| composited);
    let px = ((display.height as f32 / 1080.0 * 15.0).round() as u32).max(13);
    let thick = (px as i32 / 6).max(2);
    let mut sides = Vec::new();
    for side in [Rect::new(area.x - thick, area.y - thick, area.w + 2 * thick, thick), Rect::new(area.x - thick, area.bottom(), area.w + 2 * thick, thick), Rect::new(area.x - thick, area.y, thick, area.h), Rect::new(area.right(), area.y, thick, area.h)] {
        let mut panel = Panel::new(conn, &display, None, RED)?;
        panel.show(conn, side, &vec![RED; (side.w * side.h) as usize])?;
        sides.push(panel);
    }
    let mut button = Panel::new(conn, &display, deep, PANEL)?;
    let mut keys_panel = Panel::new(conn, &display, deep, PANEL)?;
    let (gap, sample) = (px as i32 / 2 + thick, patch(&mut text, px, "00:00", true, true));
    let button_at = [(area.right() - sample.0, area.bottom() + gap), (area.right() - sample.0, area.y - gap - sample.1), (area.right() + gap, area.bottom() - sample.1), (area.x - gap - sample.0, area.bottom() - sample.1)]
        .into_iter()
        .map(|(x, y)| Rect::new(x, y, sample.0, sample.1))
        .find(|place| place.intersect(screen) == *place);
    told_how_to_stop(button_at.is_some());

    // The keys pressed anywhere, asked of the display as it hears them
    // itself: no window of hotshot's has the keyboard.
    let mut keys = Keys::default();
    let mut keys_at: Option<Instant> = None;
    if options.keys {
        conn.xinput_xi_query_version(2, 0)?.reply().map_err(|_| "this display will not say what keys are pressed")?;
        let wanted = xinput::EventMask { deviceid: 1, mask: vec![xinput::XIEventMask::RAW_KEY_PRESS | xinput::XIEventMask::RAW_KEY_RELEASE] };
        conn.xinput_xi_select_events(display.root, &[wanted])?;
    }
    // What is held as it begins: Shift, Ctrl, Alt and the like.
    let mut held: u16 = conn.query_pointer(display.root)?.reply().map_or(0, |pointer| u16::from(pointer.mask) & (0x01 | 0x04 | 0x08 | 0x40 | 0x80));
    conn.flush()?;

    // SAFETY: the handler does nothing but set a flag.
    unsafe {
        for signal in [libc::SIGUSR1, libc::SIGINT, libc::SIGTERM] {
            libc::signal(signal, told_to_stop as extern "C" fn(libc::c_int) as libc::sighandler_t);
        }
    }
    if let Some(marker) = marker() {
        if let Some(dir) = marker.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(marker, std::process::id().to_string())?;
    }

    let from = format!("{}+{},{}", std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into()), area.x, area.y);
    let (fps, size, long) = (options.fps.to_string(), format!("{}x{}", area.w, area.h), format!("{}", options.duration));
    let mut told: Vec<&str> = vec!["-hide_banner", "-loglevel", "error", "-y", "-f", "x11grab", "-framerate", &fps, "-draw_mouse", if options.record_pointer { "1" } else { "0" }, "-video_size", &size, "-i", &from];
    if options.duration > 0.0 {
        told.extend(["-t", &long]);
    }
    match kind {
        Video::Mp4 => told.extend(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"]),
        _ => told.extend(["-c:v", "libx264rgb", "-preset", "ultrafast", "-qp", "0"]),
    }
    let raw_name = raw.to_string_lossy().into_owned();
    told.push(&raw_name);
    let mut recorder: Child = Command::new("ffmpeg").args(&told).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|error| format!("cannot run ffmpeg, which is what records: {error}"))?;

    let began = Instant::now();
    let mut said = u64::MAX;
    // Whatever goes wrong while it records, ffmpeg is not left
    // recording with nothing to stop it.
    let mut watch = || -> Result<bool> {
        Ok(loop {
        // The time so far, on the button, each second.
        let seconds = began.elapsed().as_secs();
        if let (true, Some(place)) = (seconds != said, button_at) {
            said = seconds;
            // As wide as the time now is, which is not as wide as it
            // was: its right end stays where it is.
            let (wide, high, pixels) = patch(&mut text, px, &format!("{:02}:{:02}", seconds / 60, seconds % 60), true, true);
            button.show(conn, Rect::new(place.right() - wide, place.y, wide, high), &pixels)?;
        }
        let mut changed = false;
        while let Some(event) = conn.poll_for_event()? {
            match event {
                Event::ButtonPress(press) if press.event == button.window => STOP.store(true, Ordering::SeqCst),
                // A key heard this way comes with nothing said of what
                // is held with it, so that is kept count of here: by the
                // time the display could be asked, a quick Ctrl+S is
                // over. Caps Lock and the like are asked for, being
                // states and not keys held.
                Event::XinputRawKeyPress(press) => {
                    let code = press.detail as u8;
                    match held_by(display.symbol(code, 0)) {
                        Some(bit) => held |= bit,
                        None => {
                            let locks: u16 = conn.query_pointer(display.root)?.reply().map_or(0, |pointer| u16::from(pointer.mask) & (0x02 | 0x10 | 0x2000));
                            let state = held | locks;
                            if let Some(key) = key_said(display.symbol(code, state), state) {
                                keys.pressed(key);
                                (keys_at, changed) = (Some(Instant::now()), true);
                            }
                        }
                    }
                }
                Event::XinputRawKeyRelease(release) => {
                    if let Some(bit) = held_by(display.symbol(release.detail as u8, 0)) {
                        held &= !bit;
                    }
                }
                _ => {}
            }
        }
        if keys_at.is_some_and(|at| at.elapsed() > KEYS_LAST) {
            keys.clear();
            keys_at = None;
            keys_panel.hide(conn)?;
        }
        if changed {
            let (wide, high, pixels) = patch(&mut text, px * 3 / 2, &keys.line(), false, false);
            let wide_most = wide.min(area.w - 2 * gap);
            // Along the bottom of what is recorded, in the middle; the
            // end of it, where there is more than fits.
            let shown: Vec<u32> = if wide_most < wide { (0..high).flat_map(|row| pixels[(row * wide + wide - wide_most) as usize..((row + 1) * wide) as usize].to_vec()).collect() } else { pixels };
            if wide_most > 0 && high < area.h {
                keys_panel.show(conn, Rect::new(area.x + (area.w - wide_most) / 2, area.bottom() - high - (px as i32 * 2).min(area.h / 8), wide_most, high), &shown)?;
            }
        }
        conn.flush()?;
        if STOP.load(Ordering::SeqCst) {
            break true;
        }
        if recorder.try_wait()?.is_some() {
            break false;
        }
        std::thread::sleep(Duration::from_millis(15));
        })
    };
    let stopped = match watch() {
        Ok(stopped) => stopped,
        Err(error) => {
            let _ = recorder.kill();
            let _ = recorder.wait();
            if let Some(marker) = marker() {
                let _ = std::fs::remove_file(marker);
            }
            return Err(error);
        }
    };

    // The signs go first, so that none is in the last of the recording;
    // then ffmpeg is told it is done, and writes the end of its file.
    keys_panel.hide(conn)?;
    button.hide(conn)?;
    for side in &mut sides {
        side.hide(conn)?;
    }
    conn.flush()?;
    told_it_stopped(kind != Video::Mp4);
    if stopped {
        if let Some(mut pipe) = recorder.stdin.take() {
            let _ = pipe.write_all(b"q");
        }
    }
    let done = recorder.wait_with_output()?;
    if let Some(marker) = marker() {
        let _ = std::fs::remove_file(marker);
    }
    if !raw.exists() || std::fs::metadata(&raw).map_or(0, |file| file.len()) == 0 {
        return Err(format!("ffmpeg recorded nothing: {}", String::from_utf8_lossy(&done.stderr).trim()).into());
    }
    if kind != Video::Mp4 {
        let made = convert(&raw, &file, kind, options.fps, None);
        let _ = std::fs::remove_file(&raw);
        made?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_said_as_they_are_known_and_typing_runs_together() {
        assert_eq!(key_said(0x61, 0), Some("a".into()));
        // Shift with a letter is how the letter was typed, and no more.
        assert_eq!(key_said(0x41, 0x01), Some("A".into()));
        assert_eq!(key_said(0x63, 0x04), Some("Ctrl+C".into()));
        assert_eq!(key_said(0x53, 0x05), Some("Ctrl+Shift+S".into()));
        assert_eq!(key_said(0xFF09, 0x08), Some("Alt+\u{21e5}".into()));
        assert_eq!(key_said(0xFF08, 0), Some("\u{232b}".into()));
        assert_eq!(key_said(0xFFFF, 0), Some("Del".into()));
        assert_eq!(key_said(0xFFC2, 0), Some("F5".into()));
        // Shift alone, and Ctrl alone, are no keys to be said.
        assert_eq!((key_said(0xFFE1, 0), key_said(0xFFE3, 0x04)), (None, None));

        let mut keys = Keys::default();
        for key in ["a", "b", "c", "\u{232b}", "\u{232b}", "Del", "x", "Ctrl+S", "y"] {
            keys.pressed(key.into());
        }
        assert_eq!(keys.line(), "abc\u{232b}\u{232b} Del x Ctrl+S y");
        // No more of them than fit: the latest.
        for _ in 0..40 {
            keys.pressed("z".into());
        }
        assert_eq!(keys.line().chars().count(), KEYS_MOST);
        keys.clear();
        assert_eq!(keys.line(), "");
    }

    #[test]
    fn a_patch_is_as_wide_as_its_words_and_see_through_at_its_corners() {
        let mut text = Text::load();
        let (short, high, pixels) = patch(&mut text, 16, "00:07", true, true);
        let (long, _, _) = patch(&mut text, 16, "00:07 and more", true, true);
        assert!(long > short && high > 16);
        assert_eq!((pixels[0] >> 24, pixels[(high / 2 * short + short / 2) as usize] >> 24), (0, PANEL_OPAQUE as u32));
    }
}
