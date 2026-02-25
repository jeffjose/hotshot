use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("failed to read metadata: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse metadata: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub capture_mode: String,
    pub display_server: String,
    pub file_size: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

impl Metadata {
    pub fn new(
        id: &str,
        width: u32,
        height: u32,
        format: &str,
        capture_mode: &str,
        display_server: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            timestamp: Utc::now(),
            width,
            height,
            format: format.to_string(),
            capture_mode: capture_mode.to_string(),
            display_server: display_server.to_string(),
            file_size: 0,
            tags: Vec::new(),
            notes: String::new(),
        }
    }

    /// Path to the sidecar JSON file for a given image path
    pub fn sidecar_path(image_path: &Path) -> PathBuf {
        image_path.with_extension("json")
    }

    pub fn save(&self, image_path: &Path) -> Result<(), MetadataError> {
        let json_path = Self::sidecar_path(image_path);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(json_path, json)?;
        Ok(())
    }

    pub fn load(image_path: &Path) -> Result<Self, MetadataError> {
        let json_path = Self::sidecar_path(image_path);
        let contents = std::fs::read_to_string(json_path)?;
        let metadata: Self = serde_json::from_str(&contents)?;
        Ok(metadata)
    }

    pub fn add_tags(&mut self, tags: &[String]) {
        for tag in tags {
            let tag = tag.trim().to_lowercase();
            if !tag.is_empty() && !self.tags.contains(&tag) {
                self.tags.push(tag);
            }
        }
    }

    pub fn remove_tags(&mut self, tags: &[String]) {
        let remove: Vec<String> = tags.iter().map(|t| t.trim().to_lowercase()).collect();
        self.tags.retain(|t| !remove.contains(t));
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        // Search tags
        if self.tags.iter().any(|t| t.contains(&q)) {
            return true;
        }
        // Search notes
        if self.notes.to_lowercase().contains(&q) {
            return true;
        }
        // Search id
        if self.id.contains(&q) {
            return true;
        }
        false
    }
}
