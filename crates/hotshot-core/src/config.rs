use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_storage_dir")]
    pub storage_dir: PathBuf,

    #[serde(default)]
    pub image: ImageConfig,

    #[serde(default)]
    pub storage: StorageConfig,

    #[serde(default)]
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    #[serde(default = "default_format")]
    pub format: ImageFormat,
    #[serde(default = "default_quality")]
    pub quality: u8,
    #[serde(default = "default_filename_template")]
    pub filename_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_organize_by")]
    pub organize_by: OrganizeBy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrganizeBy {
    Month,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default)]
    pub copy_to_clipboard: bool,
    #[serde(default)]
    pub notification: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageFormat::Png => write!(f, "png"),
            ImageFormat::Jpeg => write!(f, "jpeg"),
            ImageFormat::Webp => write!(f, "webp"),
        }
    }
}

impl std::str::FromStr for ImageFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "png" => Ok(ImageFormat::Png),
            "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
            "webp" => Ok(ImageFormat::Webp),
            _ => Err(format!("unknown format: {s}. use: png, jpeg, webp")),
        }
    }
}

impl ImageFormat {
    pub fn extension(&self) -> &str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
        }
    }
}

fn default_storage_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Screenshots")
}

fn default_format() -> ImageFormat {
    ImageFormat::Png
}

fn default_quality() -> u8 {
    90
}

fn default_filename_template() -> String {
    "{timestamp}-{random}".to_string()
}

fn default_organize_by() -> OrganizeBy {
    OrganizeBy::Month
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage_dir: default_storage_dir(),
            image: ImageConfig::default(),
            storage: StorageConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            quality: default_quality(),
            filename_template: default_filename_template(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            organize_by: default_organize_by(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            copy_to_clipboard: false,
            notification: false,
        }
    }
}

impl std::fmt::Display for OrganizeBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrganizeBy::Month => write!(f, "month"),
            OrganizeBy::None => write!(f, "none"),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("hotshot")
            .join("config.toml")
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = self.to_commented_toml();
        std::fs::write(&path, contents)?;
        Ok(())
    }

    fn to_commented_toml(&self) -> String {
        let mut s = String::new();
        s.push_str("# hotshot configuration\n");
        s.push_str("#\n");
        s.push_str("# Base directory for screenshots\n");
        s.push_str(&format!(
            "storage_dir = {:?}\n",
            self.storage_dir.display()
        ));
        s.push_str("\n[image]\n");
        s.push_str("# Image format: png, jpeg, webp\n");
        s.push_str(&format!("format = \"{}\"\n", self.image.format));
        s.push_str("# Compression quality for jpeg/webp (1-100, ignored for png)\n");
        s.push_str(&format!("quality = {}\n", self.image.quality));
        s.push_str("# Filename template. Variables: {timestamp}, {random}\n");
        s.push_str(&format!(
            "filename_template = \"{}\"\n",
            self.image.filename_template
        ));
        s.push_str("\n[storage]\n");
        s.push_str("# How to organize screenshots: \"month\" (YYYY-MM subdirs) or \"none\" (flat)\n");
        s.push_str(&format!("organize_by = \"{}\"\n", self.storage.organize_by));
        s.push_str("\n[behavior]\n");
        s.push_str("# Automatically copy screenshot to clipboard after capture\n");
        s.push_str(&format!(
            "copy_to_clipboard = {}\n",
            self.behavior.copy_to_clipboard
        ));
        s.push_str("# Show desktop notification after capture\n");
        s.push_str(&format!("notification = {}\n", self.behavior.notification));
        s
    }

    pub fn load_or_create() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        if path.exists() {
            Self::load()
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "storage_dir" => self.storage_dir = PathBuf::from(value),
            "image.format" | "format" => {
                self.image.format = match value.to_lowercase().as_str() {
                    "png" => ImageFormat::Png,
                    "jpeg" | "jpg" => ImageFormat::Jpeg,
                    "webp" => ImageFormat::Webp,
                    _ => return Err(format!("invalid format: {value}. use: png, jpeg, webp")),
                }
            }
            "image.quality" | "quality" => {
                self.image.quality = value
                    .parse()
                    .map_err(|_| format!("invalid quality: {value}. use: 1-100"))?;
                if self.image.quality == 0 || self.image.quality > 100 {
                    return Err("quality must be 1-100".to_string());
                }
            }
            "image.filename_template" | "filename_template" => {
                self.image.filename_template = value.to_string();
            }
            "storage.organize_by" | "organize_by" => {
                self.storage.organize_by = match value.to_lowercase().as_str() {
                    "month" => OrganizeBy::Month,
                    "none" => OrganizeBy::None,
                    _ => return Err(format!("invalid organize_by: {value}. use: month, none")),
                }
            }
            "behavior.copy_to_clipboard" | "copy_to_clipboard" => {
                self.behavior.copy_to_clipboard = value
                    .parse()
                    .map_err(|_| format!("invalid bool: {value}. use: true/false"))?;
            }
            "behavior.notification" | "notification" => {
                self.behavior.notification = value
                    .parse()
                    .map_err(|_| format!("invalid bool: {value}. use: true/false"))?;
            }
            _ => return Err(format!("unknown config key: {key}")),
        }
        Ok(())
    }

    pub fn display(&self) -> String {
        self.to_commented_toml()
    }
}
