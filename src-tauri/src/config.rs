//! Application configuration: vault location, database locations, and the
//! (off-by-default) Librarian toggle. Persisted as plain JSON in the app config
//! directory. No network, no telemetry.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// The user-selected sermon vault (e.g. ~/Sermons).
    pub vault_path: String,
    /// Path to the static canon vault.
    pub canon_path: String,
    /// Path to the derived pastor index.
    pub pastor_path: String,
    /// The Librarian is OFF by default and entirely local.
    pub librarian_enabled: bool,
    /// Editor font size in px (accessibility / high-contrast readability).
    pub font_size: u32,
    /// High-contrast theme on by default.
    pub high_contrast: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_dir = dirs_data_dir();
        AppConfig {
            vault_path: format!("{}/Sermons", home),
            canon_path: data_dir.join("canon.db").to_string_lossy().to_string(),
            pastor_path: data_dir.join("pastor.db").to_string_lossy().to_string(),
            librarian_enabled: false,
            font_size: 17,
            high_contrast: true,
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let base = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
    PathBuf::from(base).join("sermon-studio")
}

impl AppConfig {
    pub fn load(config_path: &Path) -> Self {
        match std::fs::read_to_string(config_path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => AppConfig::default(),
        }
    }

    pub fn save(&self, config_path: &Path) -> std::io::Result<()> {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = serde_json::to_string_pretty(self).unwrap_or_default();
        std::fs::write(config_path, s)
    }
}
