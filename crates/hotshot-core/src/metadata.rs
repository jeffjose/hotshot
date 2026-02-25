use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("failed to read metadata: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse metadata: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Single entry for one screenshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub id: String,
    pub path: PathBuf,
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

/// The database: all screenshot metadata in one file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetadataDb {
    pub screenshots: Vec<Metadata>,
}

impl Metadata {
    pub fn new(
        id: &str,
        path: PathBuf,
        width: u32,
        height: u32,
        format: &str,
        capture_mode: &str,
        display_server: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            path,
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
        if self.tags.iter().any(|t| t.contains(&q)) {
            return true;
        }
        if self.notes.to_lowercase().contains(&q) {
            return true;
        }
        if self.id.contains(&q) {
            return true;
        }
        false
    }
}

impl MetadataDb {
    /// Path to the database file: ~/.config/hotshot/metadata.json
    pub fn db_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("hotshot")
            .join("metadata.json")
    }

    pub fn load() -> Result<Self, MetadataError> {
        let path = Self::db_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let db: MetadataDb = serde_json::from_str(&contents)?;
        Ok(db)
    }

    pub fn save(&self) -> Result<(), MetadataError> {
        let path = Self::db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn add(&mut self, entry: Metadata) {
        self.screenshots.push(entry);
    }

    /// Find by ID prefix. Returns index + reference.
    pub fn find(&self, id_prefix: &str) -> Result<(usize, &Metadata), String> {
        let matches: Vec<_> = self
            .screenshots
            .iter()
            .enumerate()
            .filter(|(_, m)| m.id.starts_with(id_prefix))
            .collect();

        match matches.len() {
            0 => Err(format!("screenshot not found: {id_prefix}")),
            1 => Ok((matches[0].0, matches[0].1)),
            n => Err(format!("ambiguous id '{id_prefix}': matches {n} screenshots")),
        }
    }

    /// Find mutable by ID prefix
    pub fn find_mut(&mut self, id_prefix: &str) -> Result<&mut Metadata, String> {
        let indices: Vec<_> = self
            .screenshots
            .iter()
            .enumerate()
            .filter(|(_, m)| m.id.starts_with(id_prefix))
            .map(|(i, _)| i)
            .collect();

        match indices.len() {
            0 => Err(format!("screenshot not found: {id_prefix}")),
            1 => Ok(&mut self.screenshots[indices[0]]),
            n => Err(format!("ambiguous id '{id_prefix}': matches {n} screenshots")),
        }
    }

    /// Remove by ID prefix, returns the removed entry
    pub fn remove(&mut self, id_prefix: &str) -> Result<Metadata, String> {
        let (idx, _) = self.find(id_prefix)?;
        Ok(self.screenshots.remove(idx))
    }

    /// List all, sorted newest first
    pub fn list_sorted(&self) -> Vec<&Metadata> {
        let mut entries: Vec<_> = self.screenshots.iter().collect();
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries
    }

    pub fn search(&self, query: &str) -> Vec<&Metadata> {
        self.screenshots
            .iter()
            .filter(|m| m.matches_query(query))
            .collect()
    }
}
