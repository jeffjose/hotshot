use super::{CaptureError, CaptureMode, Region};
use image::RgbaImage;
use x11rb::connection::Connection;
use x11rb::protocol::render::{self, Pictformat};
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

pub fn capture(mode: &CaptureMode) -> Result<RgbaImage, CaptureError> {
    match mode {
        CaptureMode::Fullscreen => capture_fullscreen(),
        CaptureMode::Region(region) => capture_region(*region),
        CaptureMode::RegionInteractive => capture_region_interactive(),
        CaptureMode::ActiveWindow => capture_active_window(),
    }
}

fn connect() -> Result<(RustConnection, usize), CaptureError> {
    x11rb::connect(None).map_err(|e| CaptureError::X11(format!("failed to connect: {e}")))
}

fn capture_fullscreen() -> Result<RgbaImage, CaptureError> {
    let (conn, screen_num) = connect()?;
    let screen = &conn.setup().roots[screen_num];
    let width = screen.width_in_pixels;
    let height = screen.height_in_pixels;

    capture_window_region(&conn, screen.root, 0, 0, width, height)
}

fn capture_region(region: Region) -> Result<RgbaImage, CaptureError> {
    let (conn, screen_num) = connect()?;
    let screen = &conn.setup().roots[screen_num];

    capture_window_region(
        &conn,
        screen.root,
        region.x as i16,
        region.y as i16,
        region.width as u16,
        region.height as u16,
    )
}

// ---------------------------------------------------------------------------
// Native X11 interactive region selector (replaces slop dependency)
// ---------------------------------------------------------------------------

/// RAII cleanup for X11 resources allocated during overlay.
struct OverlayResources<'a> {
    conn: &'a RustConnection,
    window: u32,
    screen_pixmap: u32,
    screen_picture: u32,
    window_picture: u32,
    dim_picture: u32,
    dim_pixmap: u32,
    border_picture: u32,
    border_pixmap: u32,
    colormap: u32,
}

impl<'a> Drop for OverlayResources<'a> {
    fn drop(&mut self) {
        let _ = render::free_picture(self.conn, self.border_picture);
        let _ = self.conn.free_pixmap(self.border_pixmap);
        let _ = render::free_picture(self.conn, self.dim_picture);
        let _ = self.conn.free_pixmap(self.dim_pixmap);
        let _ = render::free_picture(self.conn, self.window_picture);
        let _ = render::free_picture(self.conn, self.screen_picture);
        let _ = self.conn.free_pixmap(self.screen_pixmap);
        let _ = self.conn.unmap_window(self.window);
        let _ = self.conn.destroy_window(self.window);
        let _ = self.conn.free_colormap(self.colormap);
        let _ = self.conn.flush();
    }
}

/// Find a 32-bit ARGB visual and the matching XRender Pictformat.
fn find_argb_visual_and_format(
    conn: &RustConnection,
    screen: &Screen,
) -> Result<(Visualid, u8, Pictformat), CaptureError> {
    let formats = render::query_pict_formats(conn)
        .map_err(|e| CaptureError::X11(format!("query_pict_formats: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("query_pict_formats reply: {e}")))?;

    // Walk render screens → depths → visuals to find a 32-bit ARGB visual.
    for pscreen in &formats.screens {
        for pdepth in &pscreen.depths {
            if pdepth.depth != 32 {
                continue;
            }
            for pvisual in &pdepth.visuals {
                // Verify the visual belongs to one of the screen's allowed depths.
                for sdepth in &screen.allowed_depths {
                    if sdepth.depth != 32 {
                        continue;
                    }
                    for v in &sdepth.visuals {
                        if v.visual_id == pvisual.visual {
                            return Ok((pvisual.visual, 32, pvisual.format));
                        }
                    }
                }
            }
        }
    }
    Err(CaptureError::X11(
        "no 32-bit ARGB visual found".to_string(),
    ))
}

/// Find the XRender Pictformat that matches a given visual.
fn find_pictformat_for_visual(
    conn: &RustConnection,
    visual: Visualid,
) -> Result<Pictformat, CaptureError> {
    let formats = render::query_pict_formats(conn)
        .map_err(|e| CaptureError::X11(format!("query_pict_formats: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("query_pict_formats reply: {e}")))?;

    for pscreen in &formats.screens {
        for pdepth in &pscreen.depths {
            for pvisual in &pdepth.visuals {
                if pvisual.visual == visual {
                    return Ok(pvisual.format);
                }
            }
        }
    }
    Err(CaptureError::X11(format!(
        "no pictformat for visual {visual}"
    )))
}

/// Capture the entire root window into a server-side Pixmap.
fn capture_screen_to_pixmap(
    conn: &RustConnection,
    screen: &Screen,
) -> Result<u32, CaptureError> {
    let root = screen.root;
    let w = screen.width_in_pixels;
    let h = screen.height_in_pixels;
    let depth = screen.root_depth;

    let pixmap = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    conn.create_pixmap(depth, pixmap, root, w, h)
        .map_err(|e| CaptureError::X11(format!("create_pixmap: {e}")))?;

    // Copy root window contents into the pixmap.
    let gc = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    conn.create_gc(gc, root, &CreateGCAux::default())
        .map_err(|e| CaptureError::X11(format!("create_gc: {e}")))?;
    conn.copy_area(root, pixmap, gc, 0, 0, 0, 0, w, h)
        .map_err(|e| CaptureError::X11(format!("copy_area: {e}")))?;
    conn.free_gc(gc)
        .map_err(|e| CaptureError::X11(format!("free_gc: {e}")))?;
    conn.flush()
        .map_err(|e| CaptureError::X11(format!("flush: {e}")))?;

    Ok(pixmap)
}

/// Create a single-pixel Pixmap filled with a solid color and wrap it in a Picture
/// with `repeat = true` so it acts as an infinite fill source.
fn create_solid_fill(
    conn: &RustConnection,
    window: u32,
    format: Pictformat,
    r: u16,
    g: u16,
    b: u16,
    a: u16,
) -> Result<(u32, u32), CaptureError> {
    let pixmap = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    conn.create_pixmap(32, pixmap, window, 1, 1)
        .map_err(|e| CaptureError::X11(format!("create_pixmap fill: {e}")))?;

    let picture = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    render::create_picture(
        conn,
        picture,
        pixmap,
        format,
        &render::CreatePictureAux::new().repeat(render::Repeat::NORMAL),
    )
    .map_err(|e| CaptureError::X11(format!("create_picture fill: {e}")))?;

    // Fill the 1×1 pixmap.
    render::fill_rectangles(
        conn,
        render::PictOp::SRC,
        picture,
        render::Color { red: r, green: g, blue: b, alpha: a },
        &[Rectangle { x: 0, y: 0, width: 1, height: 1 }],
    )
    .map_err(|e| CaptureError::X11(format!("fill_rectangles fill: {e}")))?;

    Ok((pixmap, picture))
}

/// Compute normalised selection rectangle from drag start/current positions.
fn compute_selection(x0: i16, y0: i16, x1: i16, y1: i16, sw: u16, sh: u16) -> (i16, i16, u16, u16) {
    let lx = x0.min(x1).max(0);
    let ly = y0.min(y1).max(0);
    let rx = x0.max(x1).min(sw as i16);
    let ry = y0.max(y1).min(sh as i16);
    let w = (rx - lx).max(0) as u16;
    let h = (ry - ly).max(0) as u16;
    (lx, ly, w, h)
}

/// Draw the overlay: dim everything, then "cut out" the selected region by compositing
/// the original screenshot there, and draw a white border around it.
fn draw_overlay(
    conn: &RustConnection,
    window_picture: u32,
    screen_picture: u32,
    dim_picture: u32,
    _border_picture: u32,
    sw: u16,
    sh: u16,
    sel: Option<(i16, i16, u16, u16)>,
) -> Result<(), CaptureError> {
    // 1) Composite full screenshot onto window (src → dst)
    render::composite(
        conn,
        render::PictOp::SRC,
        screen_picture,
        0u32,
        window_picture,
        0, 0,
        0, 0,
        0, 0,
        sw, sh,
    )
    .map_err(|e| CaptureError::X11(format!("composite screenshot: {e}")))?;

    // 2) Dim the entire window (50% black over everything)
    render::composite(
        conn,
        render::PictOp::OVER,
        dim_picture,
        0u32,
        window_picture,
        0, 0,
        0, 0,
        0, 0,
        sw, sh,
    )
    .map_err(|e| CaptureError::X11(format!("composite dim: {e}")))?;

    if let Some((sx, sy, sw_sel, sh_sel)) = sel {
        if sw_sel > 0 && sh_sel > 0 {
            // 3) Cut out: composite original screenshot over the selected region (removes dim)
            render::composite(
                conn,
                render::PictOp::SRC,
                screen_picture,
                0u32,
                window_picture,
                sx, sy,
                0, 0,
                sx, sy,
                sw_sel, sh_sel,
            )
            .map_err(|e| CaptureError::X11(format!("composite cutout: {e}")))?;

            // 4) White border (2px)
            let bw: i16 = 2;
            let border_rects = [
                // top
                Rectangle {
                    x: (sx - bw).max(0),
                    y: (sy - bw).max(0),
                    width: sw_sel + (2 * bw) as u16,
                    height: bw as u16,
                },
                // bottom
                Rectangle {
                    x: (sx - bw).max(0),
                    y: sy + sh_sel as i16,
                    width: sw_sel + (2 * bw) as u16,
                    height: bw as u16,
                },
                // left
                Rectangle {
                    x: (sx - bw).max(0),
                    y: sy,
                    width: bw as u16,
                    height: sh_sel,
                },
                // right
                Rectangle {
                    x: sx + sw_sel as i16,
                    y: sy,
                    width: bw as u16,
                    height: sh_sel,
                },
            ];
            render::fill_rectangles(
                conn,
                render::PictOp::OVER,
                window_picture,
                render::Color {
                    red: 0xffff,
                    green: 0xffff,
                    blue: 0xffff,
                    alpha: 0xffff,
                },
                &border_rects,
            )
            .map_err(|e| CaptureError::X11(format!("fill_rectangles border: {e}")))?;
        }
    }

    conn.flush()
        .map_err(|e| CaptureError::X11(format!("flush draw: {e}")))?;

    Ok(())
}

fn capture_region_interactive() -> Result<RgbaImage, CaptureError> {
    let (conn, screen_num) = connect()?;
    let screen = &conn.setup().roots[screen_num].clone();
    let sw = screen.width_in_pixels;
    let sh = screen.height_in_pixels;

    // ---- XRender init ----
    render::query_version(&conn, 0, 11)
        .map_err(|e| CaptureError::X11(format!("render query_version: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("render query_version reply: {e}")))?;

    let (argb_visual, argb_depth, argb_format) = find_argb_visual_and_format(&conn, screen)?;
    let root_pictformat = find_pictformat_for_visual(&conn, screen.root_visual)?;

    // ---- Capture screen ----
    let screen_pixmap = capture_screen_to_pixmap(&conn, screen)?;

    let screen_picture = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    render::create_picture(
        &conn,
        screen_picture,
        screen_pixmap,
        root_pictformat,
        &render::CreatePictureAux::new(),
    )
    .map_err(|e| CaptureError::X11(format!("create_picture screen: {e}")))?;

    // ---- Colormap for 32-bit visual ----
    let colormap = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, argb_visual)
        .map_err(|e| CaptureError::X11(format!("create_colormap: {e}")))?;

    // ---- Create overlay window ----
    let window = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    let win_aux = CreateWindowAux::new()
        .override_redirect(1)
        .background_pixel(0)
        .border_pixel(0)
        .colormap(colormap)
        .event_mask(
            EventMask::EXPOSURE
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION
                | EventMask::KEY_PRESS,
        );
    conn.create_window(
        argb_depth,
        window,
        screen.root,
        0,
        0,
        sw,
        sh,
        0,
        WindowClass::INPUT_OUTPUT,
        argb_visual,
        &win_aux,
    )
    .map_err(|e| CaptureError::X11(format!("create_window: {e}")))?;

    conn.map_window(window)
        .map_err(|e| CaptureError::X11(format!("map_window: {e}")))?;
    conn.flush()
        .map_err(|e| CaptureError::X11(format!("flush: {e}")))?;

    // ---- XRender pictures for the overlay window ----
    let window_picture = conn
        .generate_id()
        .map_err(|e| CaptureError::X11(format!("generate_id: {e}")))?;
    render::create_picture(
        &conn,
        window_picture,
        window,
        argb_format,
        &render::CreatePictureAux::new(),
    )
    .map_err(|e| CaptureError::X11(format!("create_picture window: {e}")))?;

    // ---- Solid-fill sources ----
    let (dim_pixmap, dim_picture) =
        create_solid_fill(&conn, window, argb_format, 0, 0, 0, 0x8000)?;
    let (border_pixmap, border_picture) =
        create_solid_fill(&conn, window, argb_format, 0xffff, 0xffff, 0xffff, 0xffff)?;

    // ---- Grab pointer and keyboard ----
    conn.grab_pointer(
        true,
        window,
        (EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION)
            .into(),
        GrabMode::ASYNC,
        GrabMode::ASYNC,
        window,
        0u32,
        Time::CURRENT_TIME,
    )
    .map_err(|e| CaptureError::X11(format!("grab_pointer: {e}")))?
    .reply()
    .map_err(|e| CaptureError::X11(format!("grab_pointer reply: {e}")))?;

    conn.grab_keyboard(true, window, Time::CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC)
        .map_err(|e| CaptureError::X11(format!("grab_keyboard: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("grab_keyboard reply: {e}")))?;

    let resources = OverlayResources {
        conn: &conn,
        window,
        screen_pixmap,
        screen_picture,
        window_picture,
        dim_picture,
        dim_pixmap,
        border_picture,
        border_pixmap,
        colormap,
    };

    // ---- Initial draw (fully dimmed) ----
    draw_overlay(
        &conn,
        window_picture,
        screen_picture,
        dim_picture,
        border_picture,
        sw,
        sh,
        None,
    )?;

    // ---- Event loop ----
    let mut drag_start: Option<(i16, i16)> = None;
    let mut current_pos: (i16, i16) = (0, 0);
    let result: Result<Region, CaptureError>;

    const ESCAPE_KEYCODE: u8 = 9;

    loop {
        let event = conn
            .wait_for_event()
            .map_err(|e| CaptureError::X11(format!("wait_for_event: {e}")))?;

        match event {
            x11rb::protocol::Event::Expose(_) => {
                let sel = drag_start.map(|(sx, sy)| compute_selection(sx, sy, current_pos.0, current_pos.1, sw, sh));
                draw_overlay(
                    &conn,
                    window_picture,
                    screen_picture,
                    dim_picture,
                    border_picture,
                    sw,
                    sh,
                    sel,
                )?;
            }
            x11rb::protocol::Event::ButtonPress(ev) => {
                if ev.detail == 1 {
                    // Left mouse button
                    drag_start = Some((ev.event_x, ev.event_y));
                    current_pos = (ev.event_x, ev.event_y);
                }
            }
            x11rb::protocol::Event::MotionNotify(ev) => {
                if drag_start.is_some() {
                    current_pos = (ev.event_x, ev.event_y);

                    // Coalesce pending motion events
                    while let Some(queued) = conn
                        .poll_for_event()
                        .map_err(|e| CaptureError::X11(format!("poll_for_event: {e}")))?
                    {
                        match queued {
                            x11rb::protocol::Event::MotionNotify(me) => {
                                current_pos = (me.event_x, me.event_y);
                            }
                            other => {
                                // We ate a non-motion event; need to handle it.
                                // Re-process by storing and breaking out.
                                // Since x11rb doesn't have "put back", handle inline:
                                match other {
                                    x11rb::protocol::Event::ButtonRelease(ev) if ev.detail == 1 => {
                                        if let Some((sx, sy)) = drag_start {
                                            let (rx, ry, rw, rh) =
                                                compute_selection(sx, sy, current_pos.0, current_pos.1, sw, sh);
                                            if rw > 0 && rh > 0 {
                                                result = Ok(Region {
                                                    x: rx as i32,
                                                    y: ry as i32,
                                                    width: rw as u32,
                                                    height: rh as u32,
                                                });
                                                drop(resources);
                                                conn.ungrab_pointer(Time::CURRENT_TIME)
                                                    .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                                                conn.ungrab_keyboard(Time::CURRENT_TIME)
                                                    .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                                                conn.flush()
                                                    .map_err(|e| CaptureError::X11(format!("flush: {e}")))?;
                                                let region = result?;
                                                return capture_region(region);
                                            }
                                        }
                                        drag_start = None;
                                    }
                                    x11rb::protocol::Event::KeyPress(ev) if ev.detail == ESCAPE_KEYCODE => {
                                        drop(resources);
                                        conn.ungrab_pointer(Time::CURRENT_TIME)
                                            .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                                        conn.ungrab_keyboard(Time::CURRENT_TIME)
                                            .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                                        conn.flush()
                                            .map_err(|e| CaptureError::X11(format!("flush: {e}")))?;
                                        return Err(CaptureError::SelectionCancelled);
                                    }
                                    _ => {}
                                }
                                break;
                            }
                        }
                    }

                    let (sx, sy) = drag_start.unwrap();
                    let sel = compute_selection(sx, sy, current_pos.0, current_pos.1, sw, sh);
                    draw_overlay(
                        &conn,
                        window_picture,
                        screen_picture,
                        dim_picture,
                        border_picture,
                        sw,
                        sh,
                        Some(sel),
                    )?;
                }
            }
            x11rb::protocol::Event::ButtonRelease(ev) => {
                if ev.detail == 1 {
                    if let Some((sx, sy)) = drag_start {
                        let (rx, ry, rw, rh) =
                            compute_selection(sx, sy, ev.event_x, ev.event_y, sw, sh);
                        if rw > 0 && rh > 0 {
                            drop(resources);
                            conn.ungrab_pointer(Time::CURRENT_TIME)
                                .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                            conn.ungrab_keyboard(Time::CURRENT_TIME)
                                .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                            conn.flush()
                                .map_err(|e| CaptureError::X11(format!("flush: {e}")))?;
                            let region = Region {
                                x: rx as i32,
                                y: ry as i32,
                                width: rw as u32,
                                height: rh as u32,
                            };
                            return capture_region(region);
                        }
                    }
                    drag_start = None;
                }
            }
            x11rb::protocol::Event::KeyPress(ev) => {
                if ev.detail == ESCAPE_KEYCODE {
                    drop(resources);
                    conn.ungrab_pointer(Time::CURRENT_TIME)
                        .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                    conn.ungrab_keyboard(Time::CURRENT_TIME)
                        .map_err(|e| CaptureError::X11(format!("ungrab: {e}")))?;
                    conn.flush()
                        .map_err(|e| CaptureError::X11(format!("flush: {e}")))?;
                    return Err(CaptureError::SelectionCancelled);
                }
            }
            _ => {}
        }
    }
}

fn capture_active_window() -> Result<RgbaImage, CaptureError> {
    let (conn, screen_num) = connect()?;
    let screen = &conn.setup().roots[screen_num];

    // Get _NET_ACTIVE_WINDOW
    let active_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .map_err(|e| CaptureError::X11(format!("intern_atom failed: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("intern_atom reply failed: {e}")))?
        .atom;

    let reply = conn
        .get_property(false, screen.root, active_atom, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| CaptureError::X11(format!("get_property failed: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("get_property reply failed: {e}")))?;

    if reply.value.len() < 4 {
        return Err(CaptureError::X11("no active window found".to_string()));
    }

    let window_id = u32::from_ne_bytes(reply.value[0..4].try_into().unwrap());
    if window_id == 0 {
        return Err(CaptureError::X11("no active window found".to_string()));
    }

    // Get window geometry (including decorations via translate)
    let geo = conn
        .get_geometry(window_id)
        .map_err(|e| CaptureError::X11(format!("get_geometry failed: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("get_geometry reply failed: {e}")))?;

    // Translate coordinates to root window
    let translated = conn
        .translate_coordinates(window_id, screen.root, 0, 0)
        .map_err(|e| CaptureError::X11(format!("translate_coordinates failed: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("translate reply failed: {e}")))?;

    capture_window_region(
        &conn,
        screen.root,
        translated.dst_x,
        translated.dst_y,
        geo.width,
        geo.height,
    )
}

fn capture_window_region(
    conn: &impl Connection,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Result<RgbaImage, CaptureError> {
    let reply = conn
        .get_image(ImageFormat::Z_PIXMAP, window, x, y, width, height, !0)
        .map_err(|e| CaptureError::X11(format!("get_image failed: {e}")))?
        .reply()
        .map_err(|e| CaptureError::X11(format!("get_image reply failed: {e}")))?;

    let mut data = reply.data;

    // X11 typically returns BGRA — convert to RGBA
    for chunk in data.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    RgbaImage::from_raw(width as u32, height as u32, data)
        .ok_or_else(|| CaptureError::X11("failed to create image from pixel data".to_string()))
}
