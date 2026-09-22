//! Sermon Studio — Tauri 2.0 application entry point.
//!
//! Offline desktop application. No network, no telemetry, no runtime cloud deps.
//! The Librarian (optional local cataloger) is off by default.
//!
//! Track F wiring: startup maintenance ([`sermon_core::reconcile`]) is
//! spawned before the event loop starts. It runs startup reconciliation (the
//! correctness mechanism) and then keeps the debounced vault watcher running
//! (an optimization only). Both are failure-safe by construction: a missing
//! vault, an unreadable database, or a failed watcher is reported on stderr
//! and never blocks startup or touches canonical sermon files.

mod commands;
mod config;
mod core_api;
mod dto;

use config::AppConfig;
use core_api::SessionBaselines;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
    /// What the editor loaded/saved this session, keyed by sermon id — the
    /// local side of Track C's editor-vs-disk conflict evaluation.
    pub baselines: Mutex<SessionBaselines>,
    /// Immutable canonical source captured by create_export_snapshot and used
    /// by the subsequent Track D render command.
    pub export_snapshots: Mutex<HashMap<String, core_api::ExportSourceSnapshot>>,
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

    /// Open the static canon vault read-only (configured path or packaged resource).
    pub fn canon(&self) -> anyhow::Result<Connection> {
        let cfg = self.config.lock().unwrap().clone();
        let path = cfg.resolve_canon_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(sermon_core::open_canon_readonly(&path)?)
    }
}

/// The paths startup maintenance needs, when a vault is configured.
/// Returns `None` for an unconfigured/empty vault (fresh installs pick a
/// vault later; maintenance is spawned again from the settings screen path).
pub fn maintenance_paths(cfg: &AppConfig) -> Option<(PathBuf, PathBuf)> {
    let vault = cfg.vault_path.trim();
    if vault.is_empty() {
        return None;
    }
    Some((PathBuf::from(vault), PathBuf::from(&cfg.pastor_path)))
}

/// Spawn Track C startup maintenance for `cfg`. Failure-safe: the spawned
/// thread owns all error reporting and never panics the caller; canonical
/// sermon files are never modified by maintenance.
pub fn spawn_maintenance(cfg: &AppConfig) -> bool {
    match maintenance_paths(cfg) {
        Some((vault, pastor)) => {
            sermon_core::reconcile::spawn_startup_maintenance(vault, pastor);
            true
        }
        None => false,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = config_file_path();
    let cfg = AppConfig::load(&config_path);

    // Startup reconciliation is the correctness mechanism; the watcher is an
    // optimization. Both are non-blocking and failure-safe.
    spawn_maintenance(&cfg);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            config: Mutex::new(cfg),
            config_path,
            baselines: Mutex::new(SessionBaselines::default()),
            export_snapshots: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            // ── Rocket contract surface (Track F) ──────────────────────────
            commands::list_sermons,
            commands::search_sermons,
            commands::create_sermon,
            commands::load_sermon,
            commands::save_sermon,
            commands::rename_sermon,
            commands::duplicate_sermon,
            commands::archive_sermon,
            commands::delete_sermon,
            commands::pin_sermon,
            commands::parse_references,
            commands::lint_sermon,
            commands::get_passage,
            commands::get_strongs,
            commands::get_cross_references,
            commands::get_preached_on,
            commands::get_chain_study,
            commands::sync_index,
            commands::rebuild_index,
            commands::rescan_library,
            commands::repair_index,
            commands::cancel_index_operation,
            commands::get_index_status,
            commands::get_archive_stats,
            commands::get_illustration_fatigue,
            commands::get_related_sermons,
            commands::get_passage_history,
            commands::get_sermon_insights,
            commands::set_librarian_enabled,
            commands::create_export_snapshot,
            commands::execute_export_job,
            commands::export_sermon,
            commands::reveal_exported_file,
            commands::resolve_conflict,
            commands::prepare_diff,
            commands::prepare_merge,
            commands::get_filesystem_status,
            commands::recover_sermon,
            commands::reconnect_sermon,
            commands::load_settings,
            commands::save_settings,
            commands::test_directive_codec,
            // ── Retained V1 surface ────────────────────────────────────────
            commands::get_config,
            commands::set_config,
            commands::set_vault,
            commands::get_verse_words,
            commands::get_lexicon,
            commands::verses_for_strong,
            commands::cross_references,
            commands::sermons_for_verse,
            commands::librarian_catalog,
            commands::librarian_related,
            commands::list_canon_sources,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sermon Studio");
}

fn config_file_path() -> PathBuf {
    config::dirs_config_dir().join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_paths_requires_configured_vault() {
        let mut cfg = AppConfig {
            vault_path: String::new(),
            canon_path: "/c.db".into(),
            pastor_path: "/p.db".into(),
            librarian_enabled: false,
            font_size: 17,
            high_contrast: true,
        };
        assert!(maintenance_paths(&cfg).is_none());
        assert!(!spawn_maintenance(&cfg), "no vault → no maintenance spawned");

        cfg.vault_path = "/vault".into();
        let (v, p) = maintenance_paths(&cfg).unwrap();
        assert_eq!(v, PathBuf::from("/vault"));
        assert_eq!(p, PathBuf::from("/p.db"));
    }

    #[test]
    fn every_command_in_registry_is_registered_in_handler() {
        // The handler list lives in run(); guard against registry drift by
        // checking the source contains each registered name.
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
            .unwrap();
        for name in commands::COMMAND_NAMES {
            let decl = format!("commands::{name},");
            assert!(src.contains(&decl), "command {name} not registered in invoke_handler");
        }
    }
}
