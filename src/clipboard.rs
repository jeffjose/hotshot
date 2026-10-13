//! The clipboard, which on X11 is not a place a picture is put but a
//! promise to hand it over when asked: whoever copied something has to
//! stay to be asked. So hotshot does, out of sight: the program that was
//! started ends at once, and a copy of it lives on until something else
//! is copied, or a clipboard manager takes the picture off its hands.

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::NONE;

use crate::x11::{Display, Result};

/// The most that is handed over at once. More than this goes a piece at
/// a time, each sent when the last has been taken.
const MOST_AT_ONCE: usize = 4 << 20;

/// A picture being handed over in pieces: to which window, on which of
/// its properties, and how far along it is.
struct Handover {
    window: Window,
    property: Atom,
    sent: usize,
    /// Which of the kinds it can be had as is being handed over.
    which: usize,
}

/// Puts a file on the clipboard, as file managers and chat programs
/// take one: by where it is, for whoever pastes to go and read.
pub fn offer_file(display: Display, file: &std::path::Path) -> Result<()> {
    let whole = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    // Where it is, as a link: every byte that is not plainly a letter
    // of a path said by its number.
    let link: String = whole.to_string_lossy().bytes().map(|byte| if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) { (byte as char).to_string() } else { format!("%{byte:02X}") }).collect();
    let link = format!("file://{link}");
    let kinds = vec![("text/uri-list", format!("{link}\r\n").into_bytes()), ("x-special/gnome-copied-files", format!("copy\n{link}").into_bytes()), ("UTF8_STRING", whole.to_string_lossy().into_owned().into_bytes())];
    offer_all(display, kinds)
}

/// Puts a PNG on the clipboard. Returns once the clipboard is hotshot's;
/// the handing over is done by a process left behind.
pub fn offer(display: Display, png: Vec<u8>) -> Result<()> {
    offer_all(display, vec![("image/png", png)])
}

/// Puts something on the clipboard in each of the kinds it can be had
/// as: whoever pastes says which it wants.
fn offer_all(display: Display, kinds: Vec<(&str, Vec<u8>)>) -> Result<()> {
    let conn = &display.conn;
    let atom = |name: &str| -> Result<Atom> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let (clipboard, targets, incr) = (atom("CLIPBOARD")?, atom("TARGETS")?, atom("INCR")?);
    let kinds: Vec<(Atom, Vec<u8>)> = kinds.into_iter().map(|(name, bytes)| Ok((atom(name)?, bytes))).collect::<Result<_>>()?;

    let owner = conn.generate_id()?;
    let look = CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    conn.create_window(0, owner, display.root, -1, -1, 1, 1, 0, WindowClass::INPUT_ONLY, x11rb::COPY_FROM_PARENT, &look)?;
    conn.set_selection_owner(owner, clipboard, display.time)?;
    if conn.get_selection_owner(clipboard)?.reply()?.owner != owner {
        return Err("another program would not let go of the clipboard".into());
    }
    conn.flush()?;

    // SAFETY: nothing else of this process is running that the child
    // goes on to need: it only ever talks to the display.
    match unsafe { libc::fork() } {
        // The one that was started is done; its copy of the connection
        // is closed without a word to the server.
        pid if pid > 0 => return Ok(()),
        0 => unsafe {
            // Out of the terminal's hands, and holding none of its files:
            // a shell waiting on a pipe would wait for as long as the
            // picture stayed on the clipboard.
            libc::setsid();
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
            if null >= 0 {
                for fd in 0..3 {
                    libc::dup2(null, fd);
                }
            }
        },
        // No second process to be had: this one stays, then.
        _ => {}
    }

    let most = std::env::var("HOTSHOT_INCR").ok().and_then(|bytes| bytes.parse().ok()).unwrap_or(MOST_AT_ONCE).min(conn.maximum_request_bytes() - 1024);
    let mut handovers: Vec<Handover> = Vec::new();
    loop {
        match conn.wait_for_event() {
            Err(_) | Ok(Event::SelectionClear(_)) => break,
            Ok(Event::SelectionRequest(request)) => {
                // Programs from before properties were named give none.
                let property = if request.property == NONE { request.target } else { request.property };
                let wanted = kinds.iter().position(|(kind, _)| *kind == request.target);
                let answered = if request.target == targets {
                    let all: Vec<Atom> = std::iter::once(targets).chain(kinds.iter().map(|(kind, _)| *kind)).collect();
                    conn.change_property32(PropMode::REPLACE, request.requestor, property, AtomEnum::ATOM, &all)?;
                    property
                } else if let Some((kind, bytes)) = wanted.map(|which| &kinds[which]).filter(|(_, bytes)| bytes.len() <= most) {
                    conn.change_property8(PropMode::REPLACE, request.requestor, property, *kind, bytes)?;
                    property
                } else if let Some(which) = wanted {
                    // Too large for one go: the size is said, and each
                    // piece follows as the one before is taken away.
                    conn.change_window_attributes(request.requestor, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE))?;
                    conn.change_property32(PropMode::REPLACE, request.requestor, property, incr, &[kinds[which].1.len() as u32])?;
                    handovers.push(Handover { window: request.requestor, property, sent: 0, which });
                    property
                } else {
                    NONE
                };
                let done = SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: request.time,
                    requestor: request.requestor,
                    selection: request.selection,
                    target: request.target,
                    property: answered,
                };
                conn.send_event(false, request.requestor, EventMask::NO_EVENT, done)?;
                conn.flush()?;
            }
            Ok(Event::PropertyNotify(notice)) if notice.state == Property::DELETE => {
                let Some(at) = handovers.iter().position(|handover| handover.window == notice.window && handover.property == notice.atom) else { continue };
                let handover = &mut handovers[at];
                let (kind, bytes) = &kinds[handover.which];
                let piece = &bytes[handover.sent..(handover.sent + most).min(bytes.len())];
                conn.change_property8(PropMode::REPLACE, handover.window, handover.property, *kind, piece)?;
                conn.flush()?;
                handover.sent += piece.len();
                // A piece with nothing in it is how the end is said.
                if piece.is_empty() {
                    handovers.swap_remove(at);
                }
            }
            // A window that asked and then went away, among other things.
            Ok(_) => {}
        }
    }
    std::process::exit(0)
}
