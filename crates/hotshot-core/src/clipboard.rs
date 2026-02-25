use arboard::Clipboard;
use image::RgbaImage;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("clipboard error: {0}")]
    Arboard(#[from] arboard::Error),
    #[error("failed to create clipboard context")]
    Init,
}

pub fn copy_image(image: &RgbaImage) -> Result<(), ClipboardError> {
    let mut clipboard = Clipboard::new().map_err(|_| ClipboardError::Init)?;
    let img_data = arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: std::borrow::Cow::Borrowed(image.as_raw()),
    };
    clipboard.set_image(img_data)?;
    Ok(())
}
