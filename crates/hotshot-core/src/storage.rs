use crate::capture::{CaptureMode, DisplayServer};
use crate::config::{Config, ImageFormat};
use crate::metadata::Metadata;
use chrono::Utc;
use image::RgbaImage;
use rand::Rng;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("metadata error: {0}")]
    Metadata(#[from] crate::metadata::MetadataError),
    #[error("screenshot not found: {0}")]
    NotFound(String),
    #[error("ambiguous id '{0}': matches {1} screenshots")]
    Ambiguous(String, usize),
    #[error("trash error: {0}")]
    Trash(String),
}

pub struct Storage {
    config: Config,
}

/// A screenshot entry found on disk
#[derive(Debug)]
pub struct ScreenshotEntry {
    pub image_path: PathBuf,
    pub metadata: Metadata,
}

impl Storage {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Generate a unique screenshot ID based on timestamp + random hex
    fn generate_id() -> String {
        let now = Utc::now();
        let random: u16 = rand::rng().random();
        format!("{}-{:04x}", now.format("%Y%m%d-%H%M%S"), random)
    }

    /// Get the directory for a screenshot based on config
    fn target_dir(&self) -> PathBuf {
        if self.config.organize_by_month {
            let now = Utc::now();
            self.config.storage_dir.join(now.format("%Y-%m").to_string())
        } else {
            self.config.storage_dir.clone()
        }
    }

    /// Save a captured screenshot to disk with metadata
    pub fn save(
        &self,
        image: &RgbaImage,
        mode: &CaptureMode,
        display_server: DisplayServer,
        format: Option<&ImageFormat>,
    ) -> Result<ScreenshotEntry, StorageError> {
        let id = Self::generate_id();
        let fmt = format.unwrap_or(&self.config.format);
        let dir = self.target_dir();
        std::fs::create_dir_all(&dir)?;

        let filename = format!("{}.{}", id, fmt.extension());
        let path = dir.join(&filename);

        // Save image in the requested format
        let rgba = image::DynamicImage::ImageRgba8(image.clone());
        match fmt {
            ImageFormat::Png => rgba.save_with_format(&path, image::ImageFormat::Png)?,
            ImageFormat::Jpeg => {
                let rgb = rgba.to_rgb8();
                let mut writer = std::io::BufWriter::new(std::fs::File::create(&path)?);
                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, self.config.quality);
                encoder.encode_image(&rgb)?;
            }
            ImageFormat::Webp => rgba.save_with_format(&path, image::ImageFormat::WebP)?,
        }

        // Get file size
        let file_size = std::fs::metadata(&path)?.len();

        let mode_str = match mode {
            CaptureMode::Fullscreen => "fullscreen",
            CaptureMode::Region(_) => "region",
            CaptureMode::RegionInteractive => "region-interactive",
            CaptureMode::ActiveWindow => "active-window",
        };

        let mut metadata = Metadata::new(
            &id,
            image.width(),
            image.height(),
            &fmt.to_string(),
            mode_str,
            &display_server.to_string(),
        );
        metadata.file_size = file_size;
        metadata.save(&path)?;

        Ok(ScreenshotEntry {
            image_path: path,
            metadata,
        })
    }

    /// List all screenshots, newest first
    pub fn list(&self, limit: Option<usize>) -> Result<Vec<ScreenshotEntry>, StorageError> {
        let mut entries = Vec::new();
        self.scan_dir(&self.config.storage_dir, &mut entries)?;

        // Sort by timestamp descending (newest first)
        entries.sort_by(|a, b| b.metadata.timestamp.cmp(&a.metadata.timestamp));

        if let Some(limit) = limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    fn scan_dir(
        &self,
        dir: &Path,
        entries: &mut Vec<ScreenshotEntry>,
    ) -> Result<(), StorageError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_dir(&path, entries)?;
            } else if is_image_file(&path) {
                if let Ok(metadata) = Metadata::load(&path) {
                    entries.push(ScreenshotEntry {
                        image_path: path,
                        metadata,
                    });
                }
            }
        }

        Ok(())
    }

    /// Find a screenshot by ID (prefix match)
    pub fn find_by_id(&self, id_prefix: &str) -> Result<ScreenshotEntry, StorageError> {
        let all = self.list(None)?;
        let matches: Vec<_> = all
            .into_iter()
            .filter(|e| e.metadata.id.starts_with(id_prefix))
            .collect();

        match matches.len() {
            0 => Err(StorageError::NotFound(id_prefix.to_string())),
            1 => Ok(matches.into_iter().next().unwrap()),
            n => Err(StorageError::Ambiguous(id_prefix.to_string(), n)),
        }
    }

    /// Search screenshots by query (tags, notes, id)
    pub fn search(&self, query: &str) -> Result<Vec<ScreenshotEntry>, StorageError> {
        let all = self.list(None)?;
        let results: Vec<_> = all
            .into_iter()
            .filter(|e| e.metadata.matches_query(query))
            .collect();
        Ok(results)
    }

    /// Delete a screenshot (move to trash)
    pub fn delete(&self, id_prefix: &str) -> Result<ScreenshotEntry, StorageError> {
        let entry = self.find_by_id(id_prefix)?;
        let json_path = Metadata::sidecar_path(&entry.image_path);

        trash::delete(&entry.image_path)
            .map_err(|e| StorageError::Trash(format!("failed to trash image: {e}")))?;
        if json_path.exists() {
            trash::delete(&json_path)
                .map_err(|e| StorageError::Trash(format!("failed to trash metadata: {e}")))?;
        }

        Ok(entry)
    }

    /// Tag a screenshot
    pub fn tag(&self, id_prefix: &str, tags: &[String]) -> Result<ScreenshotEntry, StorageError> {
        let mut entry = self.find_by_id(id_prefix)?;
        entry.metadata.add_tags(tags);
        entry.metadata.save(&entry.image_path)?;
        Ok(entry)
    }
}

fn is_image_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("png" | "jpg" | "jpeg" | "webp")
    )
}
