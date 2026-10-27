//! Configuration Management for TgTui
//! Loads API credentials, encryption keys, and preferences from config.toml and environment variables.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_id: Option<i32>,
    pub api_hash: Option<String>,
    pub database_directory: String,
    pub files_directory: String,
    pub database_encryption_key: String,
    pub use_test_dc: bool,
    pub use_file_database: bool,
    pub use_chat_info_database: bool,
    pub use_message_database: bool,
    pub enable_storage_optimizer: bool,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_id: None,
            api_hash: None,
            database_directory: "./tdlib_data".into(),
            files_directory: "./tdlib_data/files".into(),
            database_encryption_key: String::new(),
            use_test_dc: false,
            use_file_database: true,
            use_chat_info_database: true,
            use_message_database: true,
            enable_storage_optimizer: true,
            theme: "dark_truecolor".into(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        PathBuf::from("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config: AppConfig = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => toml::from_str(&content).unwrap_or_default(),
                Err(_) => AppConfig::default(),
            }
        } else {
            let default_cfg = AppConfig::default();
            let _ = default_cfg.save();
            default_cfg
        };

        // Override with Environment Variables if present (Security Best Practice)
        if let Ok(id_str) = env::var("TG_API_ID") {
            if let Ok(id) = id_str.trim().parse::<i32>() {
                config.api_id = Some(id);
            }
        }

        if let Ok(hash) = env::var("TG_API_HASH") {
            let hash_clean = hash.trim().to_string();
            if !hash_clean.is_empty() {
                config.api_hash = Some(hash_clean);
            }
        }

        if let Ok(key) = env::var("TG_DATABASE_ENCRYPTION_KEY") {
            config.database_encryption_key = key.trim().to_string();
        }

        config
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        fs::write(Self::config_path(), content)
    }

    /// Validates Telegram API credentials
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(id) = self.api_id {
            if id <= 0 {
                errors.push("TG_API_ID must be a positive integer".into());
            }
        }

        if let Some(ref hash) = self.api_hash {
            if hash.len() != 32 {
                errors.push("TG_API_HASH must be a 32-character hexadecimal string".into());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let mut cfg = AppConfig::default();
        cfg.api_id = Some(123456);
        cfg.api_hash = Some("0123456789abcdef0123456789abcdef".into());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_invalid_api_id() {
        let mut cfg = AppConfig::default();
        cfg.api_id = Some(-5);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_invalid_api_hash_length() {
        let mut cfg = AppConfig::default();
        cfg.api_hash = Some("short_hash".into());
        assert!(cfg.validate().is_err());
    }
}
