//! The X server: reading the screen off it, and the one window, as large
//! as the screen and over everything, that the frozen picture is shown in.
//!
//! The screen is read, and every frame handed back, through memory shared
//! with the server: a 4K screen is 33 MB, which is a long time to push
//! through a socket and no time at all to point at.
//!
//! The window is not the window manager's to place or decorate, so it is
//! up the moment it is asked for; and its first picture is its
//! background, which the server paints as it shows the window, so that
//! there is never a frame of anything else.

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::os::fd::AsRawFd;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::render::{self, ConnectionExt as _};
use x11rb::protocol::shm::{self, ConnectionExt as _};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use crate::canvas::Rect;
use crate::editor::{Cursor, Key, Mods};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// ZPixmap: whole pixels, one after another.
const Z_PIXMAP: u8 = 2;

/// A screenful of pixels: in memory shared with the server where it will
/// have that, and otherwise the program's own.
pub enum Buffer {
    Shared { segment: shm::Seg, address: *mut u32, len: usize },
    Own(Vec<u32>),
}

impl Buffer {
    /// What the server knows it by, when it is shared with the server.
    pub fn segment(&self) -> Option<shm::Seg> {
        match self {
            Buffer::Shared { segment, .. } => Some(*segment),
            Buffer::Own(_) => None,
        }
    }
}

impl Deref for Buffer {
    type Target = [u32];

    fn deref(&self) -> &[u32] {
        match self {
            // SAFETY: the segment is `len` pixels, mapped until drop.
            Buffer::Shared { address, len, .. } => unsafe { std::slice::from_raw_parts(*address, *len) },
            Buffer::Own(pixels) => pixels,
        }
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut [u32] {
        match self {
            // SAFETY: as above, and the server only reads or writes it
            // when asked to, which is never while it is borrowed here.
            Buffer::Shared { address, len, .. } => unsafe { std::slice::from_raw_parts_mut(*address, *len) },
            Buffer::Own(pixels) => pixels,
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let Buffer::Shared { address, .. } = self {
            // SAFETY: mapped by `shmat`, unmapped once.
            unsafe { libc::shmdt(address.cast()) };
        }
    }
}

/// What the display has to say.
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Motion(i32, i32, Mods),
    Press(u8, i32, i32, Mods, u32),
    Release(u8, i32, i32, Mods),
    Key(Key, Mods),
    Expose,
}

pub struct Display {
    pub conn: RustConnection,
    pub screen: usize,
    pub root: Window,
    pub width: i32,
    pub height: i32,
    depth: u8,
    shared: bool,
    window: Window,
    gc: Gcontext,
    /// The server's own copy of the screen as it was frozen, if it has one.
    frozen: Pixmap,
    /// The first key code, how many symbols each has, and the symbols.
    keys: (u8, usize, Vec<u32>),
    cursors: HashMap<Cursor, u32>,
    cursor: Option<Cursor>,
    /// What the display said while something else was being waited for,
    /// still to be heard.
    waiting: VecDeque<Event>,
    /// The server's time at the latest event, which owning the clipboard
    /// is dated by.
    pub time: u32,
    /// Under Wayland the windows are ones the compositor manages, one
    /// to each monitor.
    pub managed: bool,
    /// The windows that are up, each with the part of the screen it is
    /// over: one over all of it, but for those.
    panes: Vec<(Window, Rect)>,
}

impl Display {
    pub fn open() -> Result<Display> {
        let (conn, screen) = x11rb::connect(None).map_err(|error| format!("cannot open display: {error}"))?;
        let setup = conn.setup();
        let root = &setup.roots[screen];
        let pixel = setup.pixmap_formats.iter().find(|format| format.depth == root.root_depth).map_or(0, |format| format.bits_per_pixel);
        let visual = root.allowed_depths.iter().flat_map(|depth| &depth.visuals).find(|visual| visual.visual_id == root.root_visual);
        let plain = visual.is_some_and(|visual| visual.red_mask == 0xFF0000 && visual.green_mask == 0x00FF00 && visual.blue_mask == 0x0000FF);
        if pixel != 32 || !plain || setup.image_byte_order != ImageOrder::LSB_FIRST || cfg!(target_endian = "big") {
            return Err("this display does not keep its pixels as 8-bit red, green and blue, which is all hotshot knows".into());
        }
        let (root, width, height, depth) = (root.root, i32::from(root.width_in_pixels), i32::from(root.height_in_pixels), root.root_depth);
        let shared = conn.extension_information(shm::X11_EXTENSION_NAME)?.is_some();
        Ok(Display { conn, screen, root, width, height, depth, shared, window: NONE, gc: NONE, frozen: NONE, keys: (0, 0, Vec::new()), cursors: HashMap::new(), cursor: None, waiting: VecDeque::new(), time: CURRENT_TIME, managed: false, panes: Vec::new() })
    }

    /// Has its windows be ones the compositor manages, one to each
    /// monitor: for a Wayland session, where a window of the old kind
    /// is given no keys, and a managed one is on one monitor.
    pub fn manage(&mut self) {
        self.managed = true;
    }

    /// A screenful of pixels, shared with the server if it can be: it
    /// cannot when the server is another machine.
    pub fn buffer(&mut self) -> Buffer {
        let len = self.width as usize * self.height as usize;
        if self.shared {
            if let Some(buffer) = self.share(len) {
                return buffer;
            }
            self.shared = false;
        }
        Buffer::Own(vec![0; len])
    }

    fn share(&self, len: usize) -> Option<Buffer> {
        // SAFETY: a segment of `len` pixels is made and mapped; the
        // server is told of it, and it is marked to go once both let go.
        unsafe {
            let id = libc::shmget(libc::IPC_PRIVATE, len * 4, libc::IPC_CREAT | 0o600);
            if id < 0 {
                return None;
            }
            let address = libc::shmat(id, std::ptr::null(), 0);
            let taken = (address as isize != -1).then(|| {
                let segment = self.conn.generate_id().ok()?;
                self.conn.shm_attach(segment, id as u32, false).ok()?.check().ok()?;
                Some(segment)
            });
            libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut());
            match taken.flatten() {
                Some(segment) => Some(Buffer::Shared { segment, address: address.cast(), len }),
                None => {
                    if address as isize != -1 {
                        libc::shmdt(address);
                    }
                    None
                }
            }
        }
    }

    /// The screen as it was frozen, or if it has not been, as it is now.
    pub fn capture(&mut self) -> Result<Buffer> {
        let mut buffer = self.buffer();
        let (width, height) = (self.width as u16, self.height as u16);
        let from = if self.frozen == NONE { self.root } else { self.frozen };
        match &mut buffer {
            Buffer::Shared { segment, .. } => {
                self.conn.shm_get_image(from, 0, 0, width, height, !0, Z_PIXMAP, *segment, 0)?.reply()?;
            }
            Buffer::Own(pixels) => {
                let image = self.conn.get_image(ImageFormat::Z_PIXMAP, from, 0, 0, width, height, !0)?.reply()?;
                for (pixel, bytes) in pixels.iter_mut().zip(image.data.chunks_exact(4)) {
                    *pixel = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                }
            }
        }
        Ok(buffer)
    }

    /// Reads some rows of the frozen screen into their place in shared
    /// memory: a strip of it, where the whole is a long time coming.
    pub fn read_rows(&self, into: shm::Seg, rows: std::ops::Range<i32>) -> Result<()> {
        let from = if self.frozen == NONE { self.root } else { self.frozen };
        let offset = rows.start as u32 * self.width as u32 * 4;
        self.conn.shm_get_image(from, 0, rows.start as i16, self.width as u16, (rows.end - rows.start) as u16, !0, Z_PIXMAP, into, offset)?.reply()?;
        Ok(())
    }

    /// Freezes the screen: a copy of it is taken and, darkened by `dim`
    /// per cent, put up in the window over everything, all of it within
    /// the server and so at once. Reading a screen out of a graphics card
    /// is another matter, a quarter of a second for 4K on some; that is
    /// done afterwards, from the copy, with the frozen screen already up.
    ///
    /// Says whether it could: it takes the Render extension to darken a
    /// picture the server holds. Without it the window is put up by `show`.
    pub fn freeze(&mut self, dim: u32) -> Result<bool> {
        let Some(format) = self.picture_format()? else { return Ok(false) };
        let (width, height) = (self.width as u16, self.height as u16);
        let (frozen, dimmed, copier, picture) = (self.conn.generate_id()?, self.conn.generate_id()?, self.conn.generate_id()?, self.conn.generate_id()?);
        // Copied with the windows that are on it, which a copy of the
        // root otherwise leaves out as black holes.
        self.conn.create_gc(copier, self.root, &CreateGCAux::new().subwindow_mode(SubwindowMode::INCLUDE_INFERIORS).graphics_exposures(0))?;
        self.conn.create_pixmap(self.depth, frozen, self.root, width, height)?;
        self.conn.copy_area(self.root, frozen, copier, 0, 0, 0, 0, width, height)?;
        self.frozen = frozen;
        self.conn.create_pixmap(self.depth, dimmed, self.root, width, height)?;
        self.conn.copy_area(frozen, dimmed, copier, 0, 0, 0, 0, width, height)?;
        self.conn.render_create_picture(picture, dimmed, format, &render::CreatePictureAux::new())?;
        let black = render::Color { red: 0, green: 0, blue: 0, alpha: (dim.min(100) * 0xFFFF / 100) as u16 };
        self.conn.render_fill_rectangles(render::PictOp::OVER, picture, black, &[Rectangle { x: 0, y: 0, width, height }])?;
        self.conn.render_free_picture(picture)?;
        self.conn.free_gc(copier)?;
        self.raise(dimmed)?;
        self.conn.free_pixmap(dimmed)?;
        // It is up once the server answers something asked after it.
        self.conn.get_input_focus()?.reply()?;
        Ok(true)
    }

    /// How the Render extension knows the screen's kind of pixel, if
    /// there is a Render extension.
    fn picture_format(&self) -> Result<Option<render::Pictformat>> {
        if self.conn.extension_information(render::X11_EXTENSION_NAME)?.is_none() {
            return Ok(None);
        }
        let visual = self.conn.setup().roots[self.screen].root_visual;
        let formats = self.conn.render_query_pict_formats()?.reply()?;
        let visuals = formats.screens.get(self.screen).into_iter().flat_map(|screen| &screen.depths).flat_map(|depth| &depth.visuals);
        Ok(visuals.into_iter().find(|known| known.visual == visual).map(|known| known.format))
    }

    /// Makes the window, as large as the screen and over everything,
    /// with a picture the server holds for its background.
    fn raise(&mut self, background: Pixmap) -> Result<()> {
        let events = EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION | EventMask::KEY_PRESS | EventMask::KEY_RELEASE;
        // One window over all of the screen, that nobody manages; or,
        // where it has to be managed to be given the keys, one to each
        // monitor, each all of its monitor: a managed window is on one.
        let places = match self.managed {
            false => vec![Rect::new(0, 0, self.width, self.height)],
            true => self.monitors(),
        };
        self.panes.clear();
        for place in places {
            let window = self.conn.generate_id()?;
            let look = match self.managed {
                false => CreateWindowAux::new().override_redirect(1).background_pixmap(background).event_mask(events),
                true => CreateWindowAux::new().background_pixel(0).event_mask(events | EventMask::STRUCTURE_NOTIFY),
            };
            self.conn.create_window(self.depth, window, self.root, place.x as i16, place.y as i16, place.w as u16, place.h as u16, 0, WindowClass::INPUT_OUTPUT, x11rb::COPY_FROM_PARENT, &look)?;
            self.conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, b"hotshot")?;
            self.conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"hotshot\0hotshot\0")?;
            if self.managed {
                // Told to whoever manages windows: all of this monitor,
                // nothing round it, over the rest, in no list of
                // windows, where it was put, and given the keys.
                let hints = self.atom("_MOTIF_WM_HINTS");
                self.conn.change_property32(PropMode::REPLACE, window, hints, hints, &[2, 0, 0, 0, 0])?;
                let states = [self.atom("_NET_WM_STATE_FULLSCREEN"), self.atom("_NET_WM_STATE_ABOVE"), self.atom("_NET_WM_STATE_SKIP_TASKBAR"), self.atom("_NET_WM_STATE_SKIP_PAGER")];
                self.conn.change_property32(PropMode::REPLACE, window, self.atom("_NET_WM_STATE"), AtomEnum::ATOM, &states)?;
                let sizes = [1 | 2, place.x as u32, place.y as u32, place.w as u32, place.h as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                self.conn.change_property32(PropMode::REPLACE, window, AtomEnum::WM_NORMAL_HINTS, AtomEnum::WM_SIZE_HINTS, &sizes)?;
                self.conn.change_property32(PropMode::REPLACE, window, AtomEnum::WM_HINTS, AtomEnum::WM_HINTS, &[1, 1, 0, 0, 0, 0, 0, 0, 0])?;
            }
            self.conn.map_window(window)?;
            self.panes.push((window, place));
        }
        self.window = self.panes[0].0;
        self.gc = self.conn.generate_id()?;
        self.conn.create_gc(self.gc, self.window, &CreateGCAux::new().graphics_exposures(0))?;
        self.conn.flush()?;
        if self.managed {
            // Each is up when it is told to draw itself, which a
            // window that is managed is not at once: waited for, a
            // while. The keys go to the one the pointer is over.
            let asked = std::time::Instant::now();
            let mut up = 0;
            while up < self.panes.len() && asked.elapsed() < std::time::Duration::from_millis(1500) {
                match self.conn.poll_for_event()? {
                    Some(Event::Expose(expose)) if expose.count == 0 => up += 1,
                    Some(other) => self.waiting.push_back(other),
                    None => std::thread::sleep(std::time::Duration::from_millis(5)),
                }
            }
            self.settle();
            let pointer = self.pointer();
            let under = self.panes.iter().find(|(_, place)| place.contains(pointer.0, pointer.1)).map_or(self.window, |(window, _)| *window);
            let _ = self.conn.set_input_focus(InputFocus::PARENT, under, CURRENT_TIME);
            self.conn.flush()?;
        }
        Ok(())
    }

    /// Where each managed window in fact is, and how large: whoever
    /// manages them has the last word on that, and it may not be the
    /// monitor it was made for. What is drawn in each is what of the
    /// screen it is over, wherever that turns out to be.
    fn settle(&mut self) {
        for (window, place) in &mut self.panes {
            let size = self.conn.get_geometry(*window).ok().and_then(|cookie| cookie.reply().ok());
            let at = self.conn.translate_coordinates(*window, self.root, 0, 0).ok().and_then(|cookie| cookie.reply().ok());
            if let (Some(size), Some(at)) = (size, at) {
                *place = Rect::new(i32::from(at.dst_x), i32::from(at.dst_y), i32::from(size.width), i32::from(size.height));
            }
        }
    }

    /// Puts the window up with a frame made here in it: the way round
    /// for a server that cannot darken the screen itself.
    pub fn show(&mut self, frame: &Buffer) -> Result<()> {
        self.raise(NONE)?;
        self.present(frame, &[Rect::new(0, 0, self.width, self.height)])
    }

    /// Sends parts of a frame of the whole screen to the windows they
    /// are in: as much of each part as is in each window, to where in
    /// that window it goes.
    fn put(&self, frame: &Buffer, parts: &[Rect]) -> Result<()> {
        for &(drawable, place) in &self.panes {
            for part in parts.iter().map(|part| part.intersect(place)).filter(|part| !part.is_empty()) {
                let (x, y, w, h) = (part.x as u16, part.y as u16, part.w as u16, part.h as u16);
                let (to_x, to_y) = ((part.x - place.x) as i16, (part.y - place.y) as i16);
                match frame {
                    Buffer::Shared { segment, .. } => {
                        self.conn.shm_put_image(drawable, self.gc, self.width as u16, self.height as u16, x, y, w, h, to_x, to_y, self.depth, Z_PIXMAP, false, *segment, 0)?;
                    }
                    // Through the socket, as many rows at a time as a request holds.
                    Buffer::Own(pixels) => {
                        let rows = ((self.conn.maximum_request_bytes() - 64) / (part.w as usize * 4)).max(1);
                        let mut top = part.y;
                        while top < part.bottom() {
                            let count = (part.bottom() - top).min(rows as i32);
                            let mut bytes = Vec::with_capacity((count * part.w * 4) as usize);
                            for row in top..top + count {
                                for pixel in &pixels[(row * self.width + part.x) as usize..][..part.w as usize] {
                                    bytes.extend_from_slice(&pixel.to_le_bytes());
                                }
                            }
                            self.conn.put_image(ImageFormat::Z_PIXMAP, drawable, self.gc, w, count as u16, to_x, (top - place.y) as i16, 0, self.depth, &bytes)?;
                            top += count;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Shows the parts of a frame that have changed, and waits for the
    /// server to have done it: frames made faster than it can show them
    /// would only queue up behind the pointer.
    pub fn present(&self, frame: &Buffer, parts: &[Rect]) -> Result<()> {
        if !parts.is_empty() {
            self.put(frame, parts)?;
            self.conn.get_input_focus()?.reply()?;
        }
        Ok(())
    }

    /// Takes the keyboard, which a window the window manager knows
    /// nothing of is otherwise never given, and the pointer with it.
    /// Fails while another program has them: the one that started
    /// hotshot at a key press does until the key is let go.
    pub fn grab(&self) -> bool {
        // A managed window is given the keys by whoever manages it, and
        // taking them from the rest of the display would take nothing.
        if self.managed {
            return true;
        }
        let keys = self.conn.grab_keyboard(false, self.window, CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC);
        let held = keys.ok().and_then(|cookie| cookie.reply().ok()).is_some_and(|reply| reply.status == GrabStatus::SUCCESS);
        if held {
            let events = EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION;
            if let Ok(cookie) = self.conn.grab_pointer(false, self.window, events, GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE, CURRENT_TIME) {
                let _ = cookie.reply();
            }
        }
        held
    }

    /// Takes the window down, and waits until it is.
    pub fn hide(&mut self) {
        if self.window != NONE {
            let _ = self.conn.ungrab_pointer(CURRENT_TIME);
            let _ = self.conn.ungrab_keyboard(CURRENT_TIME);
            for (window, _) in self.panes.drain(..) {
                let _ = self.conn.destroy_window(window);
            }
            let _ = self.conn.get_input_focus().map(|cookie| cookie.reply());
            self.window = NONE;
        }
    }

    /// Reads the cursors of the theme in use, and the keyboard's layout.
    pub fn prepare(&mut self) -> Result<()> {
        let setup = self.conn.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let mapping = self.conn.get_keyboard_mapping(first, last - first + 1)?;
        let database = x11rb::resource_manager::new_from_default(&self.conn)?;
        let handle = x11rb::cursor::Handle::new(&self.conn, self.screen, &database)?.reply()?;
        let names = [
            (Cursor::Arrow, "left_ptr"),
            (Cursor::Crosshair, "crosshair"),
            (Cursor::Move, "fleur"),
            (Cursor::Text, "xterm"),
            (Cursor::Falling, "bottom_right_corner"),
            (Cursor::Rising, "bottom_left_corner"),
            (Cursor::Across, "sb_h_double_arrow"),
            (Cursor::Upright, "sb_v_double_arrow"),
        ];
        for (cursor, name) in names {
            if let Ok(id) = handle.load_cursor(&self.conn, name) {
                self.cursors.insert(cursor, id);
            }
        }
        let mapping = mapping.reply()?;
        self.keys = (first, usize::from(mapping.keysyms_per_keycode), mapping.keysyms);
        Ok(())
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        if self.cursor != Some(cursor) {
            if let Some(&id) = self.cursors.get(&cursor) {
                for (window, _) in &self.panes {
                    let _ = self.conn.change_window_attributes(*window, &ChangeWindowAttributesAux::new().cursor(id));
                }
                let _ = self.conn.flush();
            }
            self.cursor = Some(cursor);
        }
    }

    /// Where the pointer is.
    pub fn pointer(&self) -> (i32, i32) {
        let asked = self.conn.query_pointer(self.root).ok().and_then(|cookie| cookie.reply().ok());
        asked.map_or((self.width / 2, self.height / 2), |reply| (i32::from(reply.root_x), i32::from(reply.root_y)))
    }

    /// The pointer as it is drawn just now, to be put in a picture: the
    /// server keeps it out of what it hands over of the screen. None
    /// where the server will not say.
    pub fn pointer_picture(&self) -> Option<crate::canvas::Pointer> {
        use x11rb::protocol::xfixes::ConnectionExt as _;
        self.conn.xfixes_query_version(4, 0).ok()?.reply().ok()?;
        let image = self.conn.xfixes_get_cursor_image().ok()?.reply().ok()?;
        let (w, h) = (i32::from(image.width), i32::from(image.height));
        (image.cursor_image.len() == (w * h) as usize && w > 0).then(|| crate::canvas::Pointer { x: i32::from(image.x) - i32::from(image.xhot), y: i32::from(image.y) - i32::from(image.yhot), w, h, pixels: image.cursor_image })
    }

    /// The monitors that make up the screen.
    pub fn monitors(&self) -> Vec<Rect> {
        // `HOTSHOT_MONITORS` says what they are instead, as
        // `800x600+0+0,800x600+800+0`: for trying more than one where
        // there is one.
        let said = std::env::var("HOTSHOT_MONITORS").ok().map(|said| {
            let number = |text: &str| text.parse::<i32>().ok();
            said.split(',').filter_map(|one| {
                let (size, place) = one.split_once('+')?;
                let ((wide, high), (x, y)) = (size.split_once('x')?, place.split_once('+')?);
                Some(Rect::new(number(x)?, number(y)?, number(wide)?, number(high)?))
            }).collect::<Vec<Rect>>()
        });
        if let Some(said) = said.filter(|said| !said.is_empty()) {
            return said;
        }
        let asked = self.conn.randr_get_monitors(self.root, true).ok().and_then(|cookie| cookie.reply().ok());
        let found = asked.map(|reply| reply.monitors.iter().map(|m| Rect::new(i32::from(m.x), i32::from(m.y), i32::from(m.width), i32::from(m.height))).collect());
        found.filter(|monitors: &Vec<Rect>| !monitors.is_empty()).unwrap_or_else(|| vec![Rect::new(0, 0, self.width, self.height)])
    }

    fn atom(&self, name: &str) -> Atom {
        let asked = self.conn.intern_atom(false, name.as_bytes()).ok().and_then(|cookie| cookie.reply().ok());
        asked.map_or(NONE, |reply| reply.atom)
    }

    /// Where each of some windows is on the screen, borders and title
    /// bar included, for those that are showing.
    fn places(&self, windows: &[Window]) -> Vec<Rect> {
        let frame = self.atom("_NET_FRAME_EXTENTS");
        let shadow = self.atom("_GTK_FRAME_EXTENTS");
        let kind = self.atom("_NET_WM_WINDOW_TYPE");
        let desktop = self.atom("_NET_WM_WINDOW_TYPE_DESKTOP");
        // Everything is asked of every window before any answer is
        // waited for, so that it is one trip to the server and not a
        // hundred.
        let asked: Vec<_> = windows
            .iter()
            .map(|&window| {
                (
                    self.conn.get_window_attributes(window),
                    self.conn.get_geometry(window),
                    self.conn.translate_coordinates(window, self.root, 0, 0),
                    self.conn.get_property(false, window, frame, AtomEnum::CARDINAL, 0, 4),
                    self.conn.get_property(false, window, shadow, AtomEnum::CARDINAL, 0, 4),
                    self.conn.get_property(false, window, kind, AtomEnum::ATOM, 0, 8),
                )
            })
            .collect();
        let mut places = Vec::new();
        for (attributes, geometry, origin, frame, shadow, kind) in asked {
            let sides = |cookie: std::result::Result<x11rb::cookie::Cookie<'_, RustConnection, GetPropertyReply>, _>| -> [i32; 4] {
                let values: Vec<u32> = cookie.ok().and_then(|cookie| cookie.reply().ok()).and_then(|reply| reply.value32().map(Iterator::collect)).unwrap_or_default();
                match values[..] {
                    [left, right, top, bottom] => [left as i32, right as i32, top as i32, bottom as i32],
                    _ => [0; 4],
                }
            };
            let (frame, shadow) = (sides(frame), sides(shadow));
            let kinds: Vec<u32> = kind.ok().and_then(|cookie| cookie.reply().ok()).and_then(|reply| reply.value32().map(Iterator::collect)).unwrap_or_default();
            let showing = attributes.ok().and_then(|cookie| cookie.reply().ok()).is_some_and(|reply| reply.map_state == MapState::VIEWABLE);
            let (Some(geometry), Some(origin)) = (geometry.ok().and_then(|cookie| cookie.reply().ok()), origin.ok().and_then(|cookie| cookie.reply().ok())) else { continue };
            if !showing || kinds.contains(&desktop) {
                continue;
            }
            // The window manager's frame is outside the window, and is
            // wanted; a shadow the program draws for itself is inside
            // it, and is not.
            let [left, right, top, bottom] = [frame[0] - shadow[0], frame[1] - shadow[1], frame[2] - shadow[2], frame[3] - shadow[3]];
            places.push(Rect::new(
                i32::from(origin.dst_x) - left,
                i32::from(origin.dst_y) - top,
                i32::from(geometry.width) + left + right,
                i32::from(geometry.height) + top + bottom,
            ));
        }
        places
    }

    fn listed(&self, property: &str) -> Vec<Window> {
        // Under Wayland the display knows of few of the windows there
        // are, and not where they are on the monitor kept to.
        if self.managed {
            return Vec::new();
        }
        let asked = self.conn.get_property(false, self.root, self.atom(property), AtomEnum::WINDOW, 0, 4096);
        asked.ok().and_then(|cookie| cookie.reply().ok()).and_then(|reply| reply.value32().map(Iterator::collect)).unwrap_or_default()
    }

    /// The windows on the screen, the topmost first.
    pub fn windows(&self) -> Vec<Rect> {
        let mut places = self.places(&self.listed("_NET_CLIENT_LIST_STACKING"));
        places.reverse();
        places
    }

    /// The windows on the screen that are showing, the topmost first,
    /// each with what it is called and where it is.
    pub fn titled(&self) -> Vec<(Window, String, Rect)> {
        let (name, utf8) = (self.atom("_NET_WM_NAME"), self.atom("UTF8_STRING"));
        let mut found = Vec::new();
        for window in self.listed("_NET_CLIENT_LIST_STACKING").into_iter().rev() {
            let Some(place) = self.places(&[window]).into_iter().next() else { continue };
            let said = |property: Atom, kind: Atom| {
                let reply = self.conn.get_property(false, window, property, kind, 0, 1024).ok()?.reply().ok()?;
                Some(String::from_utf8_lossy(&reply.value).into_owned()).filter(|title| !title.is_empty())
            };
            let title = said(name, utf8).or_else(|| said(AtomEnum::WM_NAME.into(), AtomEnum::STRING.into())).unwrap_or_else(|| "(no title)".to_owned());
            found.push((window, title, place));
        }
        found
    }

    /// A window's own picture, whatever is in front of it: what the
    /// compositor keeps of it, where there is a compositor, which is
    /// the inside of the window without the title bar round it.
    pub fn window_picture(&self, window: Window) -> Option<(i32, i32, Vec<u32>)> {
        use x11rb::protocol::composite::ConnectionExt as _;
        // What the compositor keeps is the picture of the window the
        // window manager wrapped this one in: the one whose parent is
        // the root. That has the title bar on it too.
        let mut outer = window;
        loop {
            let tree = self.conn.query_tree(outer).ok()?.reply().ok()?;
            if tree.parent == self.root || tree.parent == NONE {
                break;
            }
            outer = tree.parent;
        }
        self.conn.composite_query_version(0, 4).ok()?.reply().ok()?;
        let size = self.conn.get_geometry(outer).ok()?.reply().ok()?;
        let kept = self.conn.generate_id().ok()?;
        self.conn.composite_name_window_pixmap(outer, kept).ok()?.check().ok()?;
        let image = self.conn.get_image(ImageFormat::Z_PIXMAP, kept, 0, 0, size.width, size.height, !0).ok().and_then(|cookie| cookie.reply().ok());
        let _ = self.conn.free_pixmap(kept);
        let image = image?;
        if image.depth != 24 && image.depth != 32 {
            return None;
        }
        let pixels: Vec<u32> = image.data.chunks_exact(4).map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])).collect();
        // A picture that is one color all over is no picture: the
        // display had nothing kept, and said so with black.
        let whole = pixels.len() == usize::from(size.width) * usize::from(size.height) && pixels.iter().any(|pixel| *pixel != pixels[0]);
        whole.then_some((i32::from(size.width), i32::from(size.height), pixels))
    }

    /// The window that has the keyboard.
    pub fn active_window(&self) -> Option<Rect> {
        let active: Vec<Window> = self.listed("_NET_ACTIVE_WINDOW").into_iter().filter(|&window| window != NONE).collect();
        self.places(&active).into_iter().next()
    }

    /// The text on the clipboard, if there is any and whoever has it
    /// hands it over within half a second. What else the display says
    /// meanwhile is kept, to be heard afterwards. Text too long to come
    /// in one piece is not waited for.
    pub fn paste(&mut self) -> Option<String> {
        let (clipboard, text, slot) = (self.atom("CLIPBOARD"), self.atom("UTF8_STRING"), self.atom("HOTSHOT_PASTE"));
        self.conn.convert_selection(self.window, clipboard, text, slot, self.time).ok()?;
        self.conn.flush().ok()?;
        let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            match self.conn.poll_for_event().ok()? {
                Some(Event::SelectionNotify(notice)) if notice.property == NONE => return None,
                Some(Event::SelectionNotify(_)) => break,
                Some(other) => self.waiting.push_back(other),
                None if std::time::Instant::now() > until => return None,
                None => {
                    let mut ready = libc::pollfd { fd: self.conn.stream().as_raw_fd(), events: libc::POLLIN, revents: 0 };
                    // SAFETY: one valid descriptor, for a bounded time.
                    unsafe { libc::poll(&mut ready, 1, 20) };
                }
            }
        }
        let reply = self.conn.get_property(true, self.window, slot, AtomEnum::ANY, 0, u32::MAX / 4).ok()?.reply().ok()?;
        (reply.type_ == text).then(|| String::from_utf8(reply.value).ok()).flatten()
    }

    /// What has happened since last asked, having waited for something
    /// to: for as long as it takes, or for so many milliseconds. Of a
    /// run of the pointer's movements only the last is told, which is
    /// where it now is.
    pub fn inputs(&mut self, wait: Option<i32>) -> Result<Vec<Input>> {
        let mut inputs = Vec::new();
        let mut event = match self.waiting.pop_front() {
            Some(event) => Some(event),
            None => self.conn.poll_for_event()?,
        };
        if event.is_none() {
            self.conn.flush()?;
            event = match wait {
                None => Some(self.conn.wait_for_event()?),
                Some(milliseconds) => {
                    let mut ready = libc::pollfd { fd: self.conn.stream().as_raw_fd(), events: libc::POLLIN, revents: 0 };
                    // SAFETY: one valid descriptor, for a bounded time.
                    unsafe { libc::poll(&mut ready, 1, milliseconds) };
                    self.conn.poll_for_event()?
                }
            };
        }
        while let Some(next) = event {
            let input = match next {
                Event::MotionNotify(motion) => {
                    self.time = motion.time;
                    Some(Input::Motion(i32::from(motion.root_x), i32::from(motion.root_y), mods(motion.state.into())))
                }
                Event::ButtonPress(press) => {
                    self.time = press.time;
                    Some(Input::Press(press.detail, i32::from(press.root_x), i32::from(press.root_y), mods(press.state.into()), press.time))
                }
                Event::ButtonRelease(release) => {
                    self.time = release.time;
                    Some(Input::Release(release.detail, i32::from(release.root_x), i32::from(release.root_y), mods(release.state.into())))
                }
                // Alt going down or up is said as soon as it is: it is
                // held for the loupe, and what the display says is held
                // with a key is what was held before it.
                Event::KeyPress(press) => {
                    self.time = press.time;
                    let state: u16 = press.state.into();
                    let mut held = mods(state);
                    held.alt |= is_alt(self.symbol(press.detail, 0));
                    Some(Input::Key(self.key(press.detail, state), held))
                }
                Event::KeyRelease(release) if is_alt(self.symbol(release.detail, 0)) => {
                    let mut held = mods(release.state.into());
                    held.alt = false;
                    Some(Input::Key(Key::Other, held))
                }
                Event::Expose(expose) if expose.count == 0 => Some(Input::Expose),
                // A managed window moved or made another size by
                // whoever manages it: found again, and drawn afresh.
                Event::ConfigureNotify(_) if self.managed => {
                    self.settle();
                    Some(Input::Expose)
                }
                _ => None,
            };
            match (inputs.last_mut(), input) {
                (Some(last @ Input::Motion(..)), Some(motion @ Input::Motion(..))) => *last = motion,
                (_, Some(input)) => inputs.push(input),
                _ => {}
            }
            event = match self.waiting.pop_front() {
                Some(event) => Some(event),
                None => self.conn.poll_for_event()?,
            };
        }
        Ok(inputs)
    }

    /// The symbol a key code stands for, by the keyboard's own numbering
    /// of them, with Shift and the like as they are held; 0 for none.
    pub fn symbol(&self, code: u8, state: u16) -> u32 {
        let (first, per, symbols) = &self.keys;
        let start = usize::from(code.saturating_sub(*first)) * per;
        let Some(row) = symbols.get(start..start + per) else { return 0 };
        let at = |column: usize| row.get(column).copied().filter(|&symbol| symbol != 0);
        let (shift, num_lock) = (state & 0x01 != 0, state & 0x10 != 0);
        // The second layout of the keyboard is in the third and fourth
        // columns; what AltGr gives, in the fifth and sixth.
        let base = if state & 0x80 != 0 && at(4).is_some() {
            4
        } else if state & 0x2000 != 0 && at(2).is_some() {
            2
        } else {
            0
        };
        let (plain, shifted) = (at(base).unwrap_or(0), at(base + 1));
        let keypad = shifted.is_some_and(|symbol| (0xFFAA..=0xFFB9).contains(&symbol));
        if shift != (keypad && num_lock) { shifted.unwrap_or(plain) } else { plain }
    }

    /// The key a key code stands for, with Shift, Caps Lock and the like
    /// as they are held.
    pub fn key(&self, code: u8, state: u16) -> Key {
        let (symbol, shift, lock) = (self.symbol(code, state), state & 0x01 != 0, state & 0x02 != 0);
        let letter = match symbol {
            0xFF0D | 0xFF8D => return Key::Enter,
            0xFF1B => return Key::Escape,
            0xFF08 => return Key::Backspace,
            0xFFFF | 0xFF9F => return Key::Delete,
            0xFF51 | 0xFF96 => return Key::Left,
            0xFF52 | 0xFF97 => return Key::Up,
            0xFF53 | 0xFF98 => return Key::Right,
            0xFF54 | 0xFF99 => return Key::Down,
            0xFF50 | 0xFF95 => return Key::Home,
            0xFF57 | 0xFF9C => return Key::End,
            0xFFAA..=0xFFB9 => char::from_u32(symbol - 0xFF80),
            0x20..=0x7E | 0xA0..=0xFF => char::from_u32(symbol),
            0x0100_0000..=0x0110_FFFF => char::from_u32(symbol - 0x0100_0000),
            _ => None,
        };
        match letter {
            // Caps Lock is Shift for letters, and Shift undoes it.
            Some(letter) if letter.is_alphabetic() && lock => match shift {
                true => Key::Char(letter.to_lowercase().next().unwrap_or(letter)),
                false => Key::Char(letter.to_uppercase().next().unwrap_or(letter)),
            },
            Some(letter) => Key::Char(letter),
            None => Key::Other,
        }
    }
}

/// Sends a picture to a window, as many rows at a time as the server
/// will take in one go: for the small windows that are not the frozen
/// screen, and have no memory shared with the server. With `deep`, the
/// window keeps how opaque each pixel is, and the colors are sent made
/// that much fainter, which is how the server wants them.
pub fn put(conn: &RustConnection, window: Window, gc: Gcontext, deep: bool, width: i32, height: i32, pixels: &[u32]) -> Result<()> {
    let each = ((conn.maximum_request_bytes().saturating_sub(64) / 4) as i32 / width.max(1)).clamp(1, height.max(1));
    for top in (0..height).step_by(each as usize) {
        let rows = each.min(height - top);
        let mut bytes = Vec::with_capacity((rows * width * 4) as usize);
        for pixel in &pixels[(top * width) as usize..((top + rows) * width) as usize] {
            let opaque = pixel >> 24;
            let part = |shift: u32| if deep { ((pixel >> shift & 0xFF) * opaque / 255) as u8 } else { (pixel >> shift) as u8 };
            bytes.extend_from_slice(&[part(0), part(8), part(16), if deep { opaque as u8 } else { 0 }]);
        }
        conn.put_image(ImageFormat::Z_PIXMAP, window, gc, width as u16, rows as u16, 0, top as i16, 0, if deep { 32 } else { 24 }, &bytes)?;
    }
    conn.flush()?;
    Ok(())
}

/// A visual that keeps how opaque each pixel is, where the display has
/// one: what a window has to be made with for what is under it to show
/// through it.
pub fn deep_visual(conn: &RustConnection, screen: usize) -> Option<Visualid> {
    let depth = conn.setup().roots[screen].allowed_depths.iter().find(|depth| depth.depth == 32)?;
    depth.visuals.iter().find(|visual| visual.class == VisualClass::TRUE_COLOR && visual.red_mask == 0xFF0000).map(|visual| visual.visual_id)
}

/// Whether a key's symbol is an Alt key's, or Meta's, which some
/// keyboards have where Alt is.
fn is_alt(symbol: u32) -> bool {
    matches!(symbol, 0xFFE7..=0xFFEA)
}

pub fn mods(state: u16) -> Mods {
    Mods { shift: state & 0x01 != 0, ctrl: state & 0x04 != 0, alt: state & 0x08 != 0 }
}
