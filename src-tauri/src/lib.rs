//! Sermon Studio — Tauri 2.0 application entry point.
//!
//! Offline, Linux-native. No network, no telemetry, no runtime cloud deps.
//! The Librarian (optional local cataloger) is off by default.

mod commands;
mod config;

use config::AppConfig;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
}

impl AppState {
    /// Open the derived pastor index (creating the schema if needed).
    pub fn pastor(&self) -> anyhow::Result<Connection> {
        let cfg = self.config.lock().unwrap().clone();
        let path = PathBuf::from(&cfg.pastor_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(sermon_core::indexer::open_pastor_db(&path)?)
    }

    /// Open the static canon vault read-only.
    pub fn canon(&self) -> anyhow::Result<Connection> {
        let cfg = self.config.lock().unwrap().clone();
        let path = PathBuf::from(&cfg.canon_path);
        Ok(sermon_core::open_canon_readonly(&path)?)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = config_file_path();
    let cfg = AppConfig::load(&config_path);

    // Startup reconciliation is the correctness mechanism; the file watcher is
    // only an optimization. Run maintenance in the background so a large or
    // missing vault can never block or crash app startup — any failure stays
    // recoverable by the next reconciliation pass.
    sermon_core::reconcile::spawn_startup_maintenance(
        PathBuf::from(&cfg.vault_path),
        PathBuf::from(&cfg.pastor_path),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            config: Mutex::new(cfg),
            config_path,
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::set_vault,
            commands::list_sermons,
            commands::search_sermons,
            commands::read_sermon,
            commands::save_sermon,
            commands::new_sermon,
            commands::rebuild_index,
            commands::sync_index,
            commands::archive_stats,
            commands::get_passage,
            commands::get_verse_words,
            commands::get_lexicon,
            commands::verses_for_strong,
            commands::cross_references,
            commands::sermons_for_verse,
            commands::illustration_fatigue,
            commands::librarian_catalog,
            commands::librarian_related,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sermon Studio");
}

fn config_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home));
    PathBuf::from(base).join("sermon-studio").join("config.json")
}
