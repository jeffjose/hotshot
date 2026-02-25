use super::{CaptureError, CaptureMode, Region};
use image::RgbaImage;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;

pub fn capture(mode: &CaptureMode) -> Result<RgbaImage, CaptureError> {
    match mode {
        CaptureMode::Fullscreen => capture_fullscreen(),
        CaptureMode::Region(region) => capture_region(*region),
        CaptureMode::RegionInteractive => capture_region_interactive(),
        CaptureMode::ActiveWindow => capture_active_window(),
    }
}

fn connect() -> Result<(impl Connection, usize), CaptureError> {
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

fn capture_region_interactive() -> Result<RgbaImage, CaptureError> {
    // Use slop for interactive region selection on X11
    let output = std::process::Command::new("slop")
        .args(["-f", "%x,%y,%w,%h"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CaptureError::ToolNotFound(
                    "slop (install with: sudo apt install slop / sudo pacman -S slop)".to_string(),
                )
            } else {
                CaptureError::X11(format!("slop failed: {e}"))
            }
        })?;

    if !output.status.success() {
        return Err(CaptureError::SelectionCancelled);
    }

    let geometry = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let region = super::parse_region(&geometry)
        .map_err(|e| CaptureError::X11(format!("failed to parse slop output: {e}")))?;

    capture_region(region)
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
