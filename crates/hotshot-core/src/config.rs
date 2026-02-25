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
    #[serde(default = "default_format")]
    pub format: ImageFormat,
    #[serde(default = "default_quality")]
    pub quality: u8,
    #[serde(default = "default_organize_by_month")]
    pub organize_by_month: bool,
    #[serde(default = "default_filename_template")]
    pub filename_template: String,
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

fn default_organize_by_month() -> bool {
    true
}

fn default_filename_template() -> String {
    "{timestamp}-{random}".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage_dir: default_storage_dir(),
            format: default_format(),
            quality: default_quality(),
            organize_by_month: default_organize_by_month(),
            filename_template: default_filename_template(),
            copy_to_clipboard: false,
            notification: false,
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
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
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
            "format" => {
                self.format = match value.to_lowercase().as_str() {
                    "png" => ImageFormat::Png,
                    "jpeg" | "jpg" => ImageFormat::Jpeg,
                    "webp" => ImageFormat::Webp,
                    _ => return Err(format!("invalid format: {value}. use: png, jpeg, webp")),
                }
            }
            "quality" => {
                self.quality = value
                    .parse()
                    .map_err(|_| format!("invalid quality: {value}. use: 1-100"))?;
                if self.quality == 0 || self.quality > 100 {
                    return Err("quality must be 1-100".to_string());
                }
            }
            "organize_by_month" => {
                self.organize_by_month = value
                    .parse()
                    .map_err(|_| format!("invalid bool: {value}. use: true/false"))?;
            }
            "copy_to_clipboard" => {
                self.copy_to_clipboard = value
                    .parse()
                    .map_err(|_| format!("invalid bool: {value}. use: true/false"))?;
            }
            _ => return Err(format!("unknown config key: {key}")),
        }
        Ok(())
    }

    pub fn display(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_else(|_| format!("{self:#?}"))
    }
}
