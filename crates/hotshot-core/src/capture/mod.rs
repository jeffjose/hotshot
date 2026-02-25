pub mod wayland;
pub mod x11;

use image::RgbaImage;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("X11 capture failed: {0}")]
    X11(String),
    #[error("Wayland capture failed: {0}")]
    Wayland(String),
    #[error("no display server detected")]
    NoDisplay,
    #[error("region selection cancelled")]
    SelectionCancelled,
    #[error("external tool not found: {0}")]
    ToolNotFound(String),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub enum CaptureMode {
    Fullscreen,
    Region(Region),
    RegionInteractive,
    ActiveWindow,
}

#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
}

impl std::fmt::Display for DisplayServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayServer::X11 => write!(f, "x11"),
            DisplayServer::Wayland => write!(f, "wayland"),
        }
    }
}

pub fn detect_display_server() -> Result<DisplayServer, CaptureError> {
    // Check WAYLAND_DISPLAY first (more specific)
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Ok(DisplayServer::Wayland);
    }
    // Check XDG_SESSION_TYPE
    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        match session_type.as_str() {
            "wayland" => return Ok(DisplayServer::Wayland),
            "x11" => return Ok(DisplayServer::X11),
            _ => {}
        }
    }
    // Check DISPLAY for X11
    if std::env::var("DISPLAY").is_ok() {
        return Ok(DisplayServer::X11);
    }
    Err(CaptureError::NoDisplay)
}

pub fn capture(mode: &CaptureMode) -> Result<RgbaImage, CaptureError> {
    let display = detect_display_server()?;
    match display {
        DisplayServer::X11 => x11::capture(mode),
        DisplayServer::Wayland => wayland::capture(mode),
    }
}

/// Parse a region string like "100,200,800,600" or "800x600+100+200"
pub fn parse_region(s: &str) -> Result<Region, String> {
    // Try WxH+X+Y format
    if s.contains('x') && s.contains('+') {
        let parts: Vec<&str> = s.split(['x', '+']).collect();
        if parts.len() == 4 {
            let width: u32 = parts[0].parse().map_err(|_| "invalid width")?;
            let height: u32 = parts[1].parse().map_err(|_| "invalid height")?;
            let x: i32 = parts[2].parse().map_err(|_| "invalid x")?;
            let y: i32 = parts[3].parse().map_err(|_| "invalid y")?;
            return Ok(Region {
                x,
                y,
                width,
                height,
            });
        }
    }
    // Try X,Y,W,H format
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 4 {
        let x: i32 = parts[0].trim().parse().map_err(|_| "invalid x")?;
        let y: i32 = parts[1].trim().parse().map_err(|_| "invalid y")?;
        let width: u32 = parts[2].trim().parse().map_err(|_| "invalid width")?;
        let height: u32 = parts[3].trim().parse().map_err(|_| "invalid height")?;
        return Ok(Region {
            x,
            y,
            width,
            height,
        });
    }
    Err(format!(
        "invalid region format: '{s}'. use: X,Y,W,H or WxH+X+Y"
    ))
}
