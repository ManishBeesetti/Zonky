use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::{Result, ZonkyError};
use crate::types::{BackendChoice, DeviceChoice};

/// Application configuration (persisted to TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZonkyConfig {
    /// Default inference backend
    #[serde(default)]
    pub default_backend: BackendChoice,

    /// Default device selection
    #[serde(default)]
    pub default_device: DeviceChoice,

    /// Model cache directory
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,

    /// Server settings
    #[serde(default)]
    pub server: ServerConfig,

    /// Whether to auto-evict models when VRAM is full
    #[serde(default = "default_true")]
    pub auto_evict: bool,

    /// Maximum number of concurrently loaded models
    #[serde(default = "default_max_models")]
    pub max_loaded_models: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Optional bearer token for API authentication
    #[serde(default)]
    pub api_key: Option<String>,
    /// Enable CORS for all origins
    #[serde(default = "default_true")]
    pub cors_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            api_key: None,
            cors_enabled: true,
        }
    }
}

impl Default for ZonkyConfig {
    fn default() -> Self {
        Self {
            default_backend: BackendChoice::Auto,
            default_device: DeviceChoice::Auto,
            cache_dir: default_cache_dir(),
            server: ServerConfig::default(),
            auto_evict: true,
            max_loaded_models: 4,
        }
    }
}

impl ZonkyConfig {
    /// Load config from the default path (~/.config/zonky/config.toml)
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: ZonkyConfig = toml::from_str(&content)
                .map_err(|e| ZonkyError::ConfigError(format!("Failed to parse config: {e}")))?;
            info!(path = %path.display(), "Loaded config");
            Ok(config)
        } else {
            info!("No config file found, using defaults");
            Ok(Self::default())
        }
    }

    /// Load config from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ZonkyConfig = toml::from_str(&content)
            .map_err(|e| ZonkyError::ConfigError(format!("Failed to parse config: {e}")))?;
        Ok(config)
    }

    /// Save config to the default path
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| ZonkyError::ConfigError(format!("Failed to serialize config: {e}")))?;
        std::fs::write(&path, content)?;
        info!(path = %path.display(), "Saved config");
        Ok(())
    }

    /// Default config file path
    pub fn config_path() -> PathBuf {
        directories::BaseDirs::new()
            .map(|d| d.config_dir().join("zonky").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".config/zonky/config.toml"))
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> PathBuf {
        self.cache_dir.clone()
    }
}

fn default_cache_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.cache_dir().join("zonky"))
        .unwrap_or_else(|| PathBuf::from(".cache/zonky"))
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_true() -> bool {
    true
}

fn default_max_models() -> usize {
    4
}
