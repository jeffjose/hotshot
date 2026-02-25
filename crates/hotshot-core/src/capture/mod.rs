pub mod wayland;
pub mod x11;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::fmt;
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
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureMode {
    Fullscreen,
    Region(Region),
    RegionInteractive,
    ActiveWindow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub name: String,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl fmt::Display for Monitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}x{}+{}+{}",
            self.name, self.width, self.height, self.x, self.y
        )
    }
}

impl Monitor {
    pub fn to_region(&self) -> Region {
        Region {
            x: self.x as i32,
            y: self.y as i32,
            width: self.width as u32,
            height: self.height as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

pub fn capture(mode: &CaptureMode, display_bounds: Option<Region>) -> Result<RgbaImage, CaptureError> {
    let display = detect_display_server()?;
    match display {
        DisplayServer::X11 => x11::capture(mode, display_bounds),
        DisplayServer::Wayland => wayland::capture(mode),
    }
}

pub fn list_monitors() -> Result<Vec<Monitor>, CaptureError> {
    let display = detect_display_server()?;
    match display {
        DisplayServer::X11 => x11::list_monitors(),
        DisplayServer::Wayland => Err(CaptureError::Other(
            "monitor listing not yet supported on Wayland".to_string(),
        )),
    }
}

/// Resolve a display specifier (name like "HDMI-1" or index like "0") to a Monitor.
pub fn resolve_display(spec: &str) -> Result<Monitor, CaptureError> {
    let monitors = list_monitors()?;
    if monitors.is_empty() {
        return Err(CaptureError::Other("no monitors found".to_string()));
    }

    // Try as index first
    if let Ok(idx) = spec.parse::<usize>() {
        let count = monitors.len();
        return monitors.into_iter().nth(idx).ok_or_else(|| {
            CaptureError::Other(format!(
                "display index {idx} out of range (0..{})",
                count - 1
            ))
        });
    }

    // Try as name
    monitors
        .into_iter()
        .find(|m| m.name == spec)
        .ok_or_else(|| CaptureError::Other(format!("no display named '{spec}'")))
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
