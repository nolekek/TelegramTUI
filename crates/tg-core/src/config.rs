use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::domain::ProxyConfig;
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_id: i32,
    pub api_hash: String,
    pub use_test_dc: bool,
    pub data_dir: PathBuf,
    pub log_level: String,
    pub proxy: Option<ProxyConfig>,
    pub use_mock: bool,
    #[serde(default)]
    pub downloads_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Default demo credentials or placeholders
            api_id: 94575, // Standard Telegram Desktop public test ID or custom
            api_hash: "a3406de8d171bb422bb6ddf3bbd800e2".to_string(),
            use_test_dc: false,
            data_dir: PathBuf::from(".data"),
            log_level: "info".to_string(),
            proxy: None,
            use_mock: false, // Real Telegram by default
            downloads_dir: None,
        }
    }
}

impl Config {
    /// Resolves configuration path and data directory using a hybrid strategy:
    /// - If `portable` is true or `./.data` exists, uses portable mode (`.data`).
    /// - Otherwise, uses OS-standard directories (XDG on Linux, %APPDATA% on Windows, Application Support on macOS).
    /// - Explicit custom overrides take highest precedence.
    pub fn resolve_paths(
        portable: bool,
        custom_config: Option<&Path>,
        custom_data_dir: Option<&Path>,
    ) -> (PathBuf, PathBuf) {
        let is_portable = portable || Path::new(".data").exists();

        let data_dir = if let Some(custom) = custom_data_dir {
            custom.to_path_buf()
        } else if is_portable {
            PathBuf::from(".data")
        } else {
            dirs::data_dir()
                .map(|p| p.join("telegram-tui"))
                .unwrap_or_else(|| PathBuf::from(".data"))
        };

        let config_path = if let Some(custom) = custom_config {
            custom.to_path_buf()
        } else if is_portable {
            data_dir.join("config.toml")
        } else {
            dirs::config_dir()
                .map(|p| p.join("telegram-tui").join("config.toml"))
                .unwrap_or_else(|| data_dir.join("config.toml"))
        };

        (config_path, data_dir)
    }

    /// Automatically loads or creates the configuration according to hybrid path resolution.
    pub fn load_or_create_auto(
        portable: bool,
        custom_config: Option<&Path>,
        custom_data_dir: Option<&Path>,
    ) -> Result<(Self, PathBuf)> {
        let (config_path, data_dir) = Self::resolve_paths(portable, custom_config, custom_data_dir);

        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content).map_err(|e| CoreError::ConfigError(e.to_string()))?
        } else {
            let mut default_config = Self::default();
            default_config.data_dir = data_dir.clone();

            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let toml_string = toml::to_string_pretty(&default_config)
                .map_err(|e| CoreError::ConfigError(e.to_string()))?;
            std::fs::write(&config_path, toml_string)?;
            default_config
        };

        // If custom data dir was explicitly requested via CLI, override it
        if let Some(custom) = custom_data_dir {
            config.data_dir = custom.to_path_buf();
        } else if config.data_dir == PathBuf::from(".data") && !portable && !Path::new(".data").exists() {
            config.data_dir = data_dir;
        }

        config.ensure_directories()?;
        Ok((config, config_path))
    }

    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: Config = toml::from_str(&content)
                .map_err(|e| CoreError::ConfigError(e.to_string()))?;
            let _ = config.ensure_directories();
            Ok(config)
        } else {
            let default_config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let toml_string = toml::to_string_pretty(&default_config)
                .map_err(|e| CoreError::ConfigError(e.to_string()))?;
            std::fs::write(path, toml_string)?;
            let _ = default_config.ensure_directories();
            Ok(default_config)
        }
    }

    pub fn ensure_directories(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(self.tdlib_database_path())?;
        std::fs::create_dir_all(self.tdlib_files_path())?;
        std::fs::create_dir_all(self.downloads_path())?;
        std::fs::create_dir_all(self.cache_media_path())?;
        if let Some(log_dir) = self.log_path().parent() {
            std::fs::create_dir_all(log_dir)?;
        }
        Ok(())
    }

    pub fn downloads_path(&self) -> PathBuf {
        self.downloads_dir
            .clone()
            .unwrap_or_else(|| self.data_dir.join("downloads"))
    }

    pub fn cache_media_path(&self) -> PathBuf {
        self.data_dir.join("cache").join("media")
    }

    pub fn clean_media_cache(&self) -> Result<()> {
        let cache_dir = self.cache_media_path();
        if cache_dir.exists() {
            let _ = std::fs::remove_dir_all(&cache_dir);
        }
        let _ = std::fs::create_dir_all(&cache_dir);
        Ok(())
    }

    pub fn tdlib_database_path(&self) -> PathBuf {
        self.data_dir.join("tdlib_db")
    }

    pub fn tdlib_files_path(&self) -> PathBuf {
        self.data_dir.join("tdlib_files")
    }

    pub fn log_path(&self) -> PathBuf {
        self.data_dir.join("logs").join("client.log")
    }
}

