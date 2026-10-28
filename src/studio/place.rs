//! Where the canvas's window was, how large, and whether it filled the
//! screen when it was last closed, to open it the same way again, as jog
//! does. Kept in `canvas-window` among hotshot's state, as four numbers
//! and a 1 for filling the screen: `x y width height 1`.

use std::path::PathBuf;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, Window};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Place {
    /// Where its frame's corner is, title bar and borders included.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub maximized: bool,
}

fn file() -> Option<PathBuf> {
    crate::output::state_file("canvas-window")
}

fn read(text: &str) -> Option<Place> {
    let numbers: Vec<i32> = text.split_whitespace().map_while(|word| word.parse().ok()).collect();
    match numbers[..] {
        [x, y, w, h, ref rest @ ..] => Some(Place { x, y, w, h, maximized: rest.first() == Some(&1) }),
        _ => None,
    }
}

/// Where it was last, moved and shrunk to fit a screen this size; or,
/// the first time, four fifths of the screen in its middle.
pub fn last(screen: (i32, i32)) -> Place {
    let kept = file().and_then(|file| std::fs::read_to_string(file).ok()).and_then(|text| read(&text));
    fitted(kept, screen)
}

fn fitted(kept: Option<Place>, (across, down): (i32, i32)) -> Place {
    match kept {
        Some(place) => {
            let (w, h) = (place.w.clamp(400.min(across), across), place.h.clamp(300.min(down), down));
            Place { x: place.x.clamp(0, across - w), y: place.y.clamp(0, down - h), w, h, ..place }
        }
        None => {
            let (w, h) = ((across * 4 / 5).clamp(900.min(across), 2400), (down * 4 / 5).clamp(600.min(down), 1600));
            Place { x: (across - w) / 2, y: (down - h) / 2, w, h, maximized: false }
        }
    }
}

/// Written down as the window is now, as it closes. Filling the screen,
/// the place it had before that is kept, to go back to.
pub fn keep(conn: &impl Connection, window: Window, root: Window) {
    let Some(file) = file() else { return };
    let atom = |name: &str| conn.intern_atom(false, name.as_bytes()).ok()?.reply().ok().map(|reply| reply.atom);
    let numbers = |name: &str| -> Vec<u32> {
        let asked = atom(name).and_then(|name| conn.get_property(false, window, name, AtomEnum::ANY, 0, 32).ok()?.reply().ok());
        asked.and_then(|reply| reply.value32().map(Iterator::collect)).unwrap_or_default()
    };
    let maximized = atom("_NET_WM_STATE_MAXIMIZED_VERT").is_some_and(|vert| numbers("_NET_WM_STATE").contains(&vert));
    let place = if maximized {
        let before = std::fs::read_to_string(&file).ok().and_then(|text| read(&text));
        let Some(before) = before else { return };
        Place { maximized: true, ..before }
    } else {
        let geometry = conn.get_geometry(window).ok().and_then(|cookie| cookie.reply().ok());
        let origin = conn.translate_coordinates(window, root, 0, 0).ok().and_then(|cookie| cookie.reply().ok());
        let (Some(geometry), Some(origin)) = (geometry, origin) else { return };
        // A window manager puts the corner of its frame where it is
        // asked to, so it is the frame's corner that is kept.
        let (left, top) = match numbers("_NET_FRAME_EXTENTS")[..] {
            [left, _, top, _] => (left as i32, top as i32),
            _ => (0, 0),
        };
        Place { x: i32::from(origin.dst_x) - left, y: i32::from(origin.dst_y) - top, w: i32::from(geometry.width), h: i32::from(geometry.height), maximized: false }
    };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, format!("{} {} {} {} {}\n", place.x, place.y, place.w, place.h, u8::from(place.maximized)));
}

/// Asked of the window manager before the window is shown: its place
/// and size are the user's, and it fills the screen if it did.
pub fn ask(conn: &impl Connection, window: Window, place: Place) -> Result<(), x11rb::errors::ConnectionError> {
    use x11rb::protocol::xproto::PropMode;
    use x11rb::wrapper::ConnectionExt as _;
    let sizes = [1 | 2, place.x as u32, place.y as u32, place.w as u32, place.h as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    conn.change_property32(PropMode::REPLACE, window, AtomEnum::WM_NORMAL_HINTS, AtomEnum::WM_SIZE_HINTS, &sizes)?;
    if place.maximized {
        let atom = |name: &str| conn.intern_atom(false, name.as_bytes()).ok().and_then(|cookie| cookie.reply().ok()).map(|reply| reply.atom);
        if let (Some(state), Some(vert), Some(horz)) = (atom("_NET_WM_STATE"), atom("_NET_WM_STATE_MAXIMIZED_VERT"), atom("_NET_WM_STATE_MAXIMIZED_HORZ")) {
            conn.change_property32(PropMode::REPLACE, window, state, AtomEnum::ATOM, &[vert, horz])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_place_is_read_and_fitted_to_the_screen() {
        assert_eq!(read("10 20 1200 800 1\n"), Some(Place { x: 10, y: 20, w: 1200, h: 800, maximized: true }));
        assert_eq!(read("10 20 1200 800"), Some(Place { x: 10, y: 20, w: 1200, h: 800, maximized: false }));
        assert_eq!(read("nonsense"), None);
        // Off a smaller screen than it was closed on, it is brought back on.
        let place = fitted(read("3000 1500 1200 800"), (1920, 1080));
        assert_eq!((place.x, place.y, place.w, place.h), (720, 280, 1200, 800));
        let place = fitted(read("0 0 5000 3000"), (1920, 1080));
        assert_eq!((place.w, place.h), (1920, 1080));
        // The first time, four fifths of the screen, in its middle.
        assert_eq!(fitted(None, (1920, 1080)), Place { x: 192, y: 108, w: 1536, h: 864, maximized: false });
    }
}
