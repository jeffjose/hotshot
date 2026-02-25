use crate::capture::{CaptureMode, DisplayServer};
use crate::config::{Config, ImageFormat};
use crate::metadata::{Metadata, MetadataDb};
use chrono::Utc;
use image::RgbaImage;
use rand::Rng;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("metadata error: {0}")]
    Metadata(#[from] crate::metadata::MetadataError),
    #[error("{0}")]
    NotFound(String),
    #[error("trash error: {0}")]
    Trash(String),
}

pub struct Storage {
    config: Config,
}

impl Storage {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    fn generate_id() -> String {
        let now = Utc::now();
        let random: u16 = rand::rng().random();
        format!("{}-{:04x}", now.format("%Y%m%d-%H%M%S"), random)
    }

    fn target_dir(&self) -> PathBuf {
        if self.config.organize_by_month {
            let now = Utc::now();
            self.config.storage_dir.join(now.format("%Y-%m").to_string())
        } else {
            self.config.storage_dir.clone()
        }
    }

    /// Save a captured screenshot to disk and record in metadata DB
    pub fn save(
        &self,
        image: &RgbaImage,
        mode: &CaptureMode,
        display_server: DisplayServer,
        format: Option<&ImageFormat>,
    ) -> Result<Metadata, StorageError> {
        let id = Self::generate_id();
        let fmt = format.unwrap_or(&self.config.format);
        let dir = self.target_dir();
        std::fs::create_dir_all(&dir)?;

        let filename = format!("{}.{}", id, fmt.extension());
        let path = dir.join(&filename);

        // Save image
        let rgba = image::DynamicImage::ImageRgba8(image.clone());
        match fmt {
            ImageFormat::Png => rgba.save_with_format(&path, image::ImageFormat::Png)?,
            ImageFormat::Jpeg => {
                let rgb = rgba.to_rgb8();
                let mut writer = std::io::BufWriter::new(std::fs::File::create(&path)?);
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut writer,
                    self.config.quality,
                );
                encoder.encode_image(&rgb)?;
            }
            ImageFormat::Webp => rgba.save_with_format(&path, image::ImageFormat::WebP)?,
        }

        let file_size = std::fs::metadata(&path)?.len();

        let mode_str = match mode {
            CaptureMode::Fullscreen => "fullscreen",
            CaptureMode::Region(_) => "region",
            CaptureMode::RegionInteractive => "region-interactive",
            CaptureMode::ActiveWindow => "active-window",
        };

        let mut metadata = Metadata::new(
            &id,
            path,
            image.width(),
            image.height(),
            &fmt.to_string(),
            mode_str,
            &display_server.to_string(),
        );
        metadata.file_size = file_size;

        // Add to DB
        let mut db = MetadataDb::load()?;
        db.add(metadata.clone());
        db.save()?;

        Ok(metadata)
    }

    pub fn list(&self, limit: Option<usize>) -> Result<Vec<Metadata>, StorageError> {
        let db = MetadataDb::load()?;
        let mut entries: Vec<Metadata> = db.list_sorted().into_iter().cloned().collect();
        if let Some(limit) = limit {
            entries.truncate(limit);
        }
        Ok(entries)
    }

    pub fn find_by_id(&self, id_prefix: &str) -> Result<Metadata, StorageError> {
        let db = MetadataDb::load()?;
        let (_, entry) = db
            .find(id_prefix)
            .map_err(StorageError::NotFound)?;
        Ok(entry.clone())
    }

    pub fn search(&self, query: &str) -> Result<Vec<Metadata>, StorageError> {
        let db = MetadataDb::load()?;
        Ok(db.search(query).into_iter().cloned().collect())
    }

    pub fn delete(&self, id_prefix: &str) -> Result<Metadata, StorageError> {
        let mut db = MetadataDb::load()?;
        let entry = db
            .remove(id_prefix)
            .map_err(StorageError::NotFound)?;

        // Trash the image file
        if entry.path.exists() {
            trash::delete(&entry.path)
                .map_err(|e| StorageError::Trash(format!("failed to trash image: {e}")))?;
        }

        db.save()?;
        Ok(entry)
    }

    pub fn tag(&self, id_prefix: &str, tags: &[String]) -> Result<Metadata, StorageError> {
        let mut db = MetadataDb::load()?;
        let entry = db
            .find_mut(id_prefix)
            .map_err(StorageError::NotFound)?;
        entry.add_tags(tags);
        let result = entry.clone();
        db.save()?;
        Ok(result)
    }
}
