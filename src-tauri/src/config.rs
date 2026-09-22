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
        let home = home_dir();
        let data_dir = dirs_data_dir();
        AppConfig {
            vault_path: home.join("Sermons").to_string_lossy().to_string(),
            canon_path: data_dir.join("canon.db").to_string_lossy().to_string(),
            pastor_path: data_dir.join("pastor.db").to_string_lossy().to_string(),
            librarian_enabled: false,
            font_size: 17,
            high_contrast: true,
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn home_dir() -> PathBuf {
    env_path("HOME")
        .or_else(|| env_path("USERPROFILE"))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn dirs_config_dir() -> PathBuf {
    env_path("XDG_CONFIG_HOME")
        .or_else(|| env_path("APPDATA"))
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("sermon-studio")
}

fn dirs_data_dir() -> PathBuf {
    env_path("XDG_DATA_HOME")
        .or_else(|| env_path("LOCALAPPDATA"))
        .unwrap_or_else(|| home_dir().join(".local").join("share"))
        .join("sermon-studio")
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

    /// Resolve the effective canon.db location.
    ///
    /// Looks first at the configured data-dir path, then at platform-neutral
    /// packaged fallbacks (next to the executable, and in the Tauri resource
    /// dir). Returns the first candidate that exists, else the configured path
    /// so that `canon_present` correctly reports absence on a clean run.
    pub fn resolve_canon_path(&self) -> PathBuf {
        let configured = PathBuf::from(&self.canon_path);
        if configured.exists() {
            return configured;
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for candidate in [
                    dir.join("canon.db"),
                    dir.join("resources").join("canon.db"),
                ] {
                    if candidate.exists() {
                        return candidate;
                    }
                }
            }
        }
        configured
    }
}
