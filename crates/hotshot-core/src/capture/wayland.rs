use super::{CaptureError, CaptureMode, Region};
use image::RgbaImage;

pub fn capture(mode: &CaptureMode) -> Result<RgbaImage, CaptureError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CaptureError::Wayland(format!("failed to create runtime: {e}")))?;

    rt.block_on(capture_async(mode))
}

async fn capture_async(mode: &CaptureMode) -> Result<RgbaImage, CaptureError> {
    match mode {
        CaptureMode::Fullscreen => capture_portal(false).await,
        CaptureMode::RegionInteractive => capture_portal(true).await,
        CaptureMode::Region(region) => capture_fullscreen_and_crop(*region).await,
        CaptureMode::ActiveWindow => capture_portal(false).await,
    }
}

async fn capture_portal(interactive: bool) -> Result<RgbaImage, CaptureError> {
    use ashpd::desktop::screenshot::Screenshot;

    let response = Screenshot::request()
        .interactive(interactive)
        .send()
        .await
        .map_err(|e| CaptureError::Wayland(format!("screenshot request failed: {e}")))?
        .response()
        .map_err(|e| CaptureError::Wayland(format!("screenshot response failed: {e}")))?;

    let uri_str = response.uri().to_string();
    let path = uri_str.strip_prefix("file://").unwrap_or(&uri_str);

    let img = image::open(path)
        .map_err(|e| CaptureError::Wayland(format!("failed to open screenshot image: {e}")))?;

    // Clean up the temp file from the portal
    let _ = std::fs::remove_file(path);

    Ok(img.into_rgba8())
}

async fn capture_fullscreen_and_crop(region: Region) -> Result<RgbaImage, CaptureError> {
    let full = capture_portal(false).await?;

    let x = region.x.max(0) as u32;
    let y = region.y.max(0) as u32;
    let width = region.width.min(full.width().saturating_sub(x));
    let height = region.height.min(full.height().saturating_sub(y));

    if width == 0 || height == 0 {
        return Err(CaptureError::Wayland(
            "region is outside screen bounds".to_string(),
        ));
    }

    let cropped = image::imageops::crop_imm(&full, x, y, width, height).to_image();
    Ok(cropped)
}
