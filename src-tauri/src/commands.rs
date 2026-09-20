//! Tauri IPC command layer.
//!
//! Every command is a thin, synchronous bridge to `sermon_core`. The frontend
//! never touches SQLite directly. Commands are deliberately granular so the UI
//! can stay modal-free and keyboard-driven.

use crate::config::AppConfig;
use crate::AppState;
use serde::Serialize;
use sermon_core::{indexer, librarian, reference, retrieval, sermon};
use std::path::PathBuf;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct ConfigView {
    pub vault_path: String,
    pub canon_path: String,
    pub pastor_path: String,
    pub librarian_enabled: bool,
    pub font_size: u32,
    pub high_contrast: bool,
    pub canon_present: bool,
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> ConfigView {
    let cfg = state.config.lock().unwrap().clone();
    ConfigView {
        canon_present: PathBuf::from(&cfg.canon_path).exists(),
        vault_path: cfg.vault_path,
        canon_path: cfg.canon_path,
        pastor_path: cfg.pastor_path,
        librarian_enabled: cfg.librarian_enabled,
        font_size: cfg.font_size,
        high_contrast: cfg.high_contrast,
    }
}

#[tauri::command]
pub fn set_config(state: State<AppState>, config: AppConfig) -> CmdResult<()> {
    {
        let mut cfg = state.config.lock().unwrap();
        *cfg = config.clone();
    }
    config.save(&state.config_path).map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn set_vault(state: State<AppState>, vault_path: String) -> CmdResult<ConfigView> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.vault_path = vault_path;
        cfg.save(&state.config_path).map_err(err)?;
    }
    Ok(get_config(state))
}

#[tauri::command]
pub fn list_sermons(state: State<AppState>) -> CmdResult<Vec<retrieval::SermonHit>> {
    let conn = state.pastor().map_err(err)?;
    retrieval::list_sermons(&conn).map_err(err)
}

#[tauri::command]
pub fn search_sermons(
    state: State<AppState>,
    query: String,
    limit: Option<i64>,
) -> CmdResult<Vec<retrieval::SermonHit>> {
    let conn = state.pastor().map_err(err)?;
    retrieval::search_sermons(&conn, &query, limit.unwrap_or(50)).map_err(err)
}

#[tauri::command]
pub fn read_sermon(state: State<AppState>, file_path: String) -> CmdResult<String> {
    let cfg = state.config.lock().unwrap().clone();
    let full = PathBuf::from(&cfg.vault_path).join(&file_path);
    std::fs::read_to_string(&full).map_err(err)
}

#[tauri::command]
pub fn save_sermon(
    state: State<AppState>,
    file_path: String,
    content: String,
) -> CmdResult<()> {
    let cfg = state.config.lock().unwrap().clone();
    let full = PathBuf::from(&cfg.vault_path).join(&file_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&full, &content).map_err(err)?;
    // Re-index just this file so search/graph stay current without a full pass.
    let mut conn = state.pastor().map_err(err)?;
    let doc = sermon::SermonDoc::parse(&content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(&mut conn, &file_path, &doc, &mut stats).map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn new_sermon(
    state: State<AppState>,
    title: String,
    passage: String,
) -> CmdResult<String> {
    let cfg = state.config.lock().unwrap().clone();
    let vault = PathBuf::from(&cfg.vault_path);
    std::fs::create_dir_all(&vault).map_err(err)?;
    let slug = sermon::slugify(&title);
    let filename = format!("{}.md", if slug.is_empty() { "untitled" } else { &slug });
    let full = vault.join(&filename);
    let content = sermon::new_sermon_template(&title, &passage);
    std::fs::write(&full, &content).map_err(err)?;
    // Index it immediately.
    let mut conn = state.pastor().map_err(err)?;
    let doc = sermon::SermonDoc::parse(&content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(&mut conn, &filename, &doc, &mut stats).map_err(err)?;
    Ok(filename)
}

#[tauri::command]
pub fn rebuild_index(state: State<AppState>) -> CmdResult<indexer::IndexStats> {
    let cfg = state.config.lock().unwrap().clone();
    let vault = PathBuf::from(&cfg.vault_path);
    let db = PathBuf::from(&cfg.pastor_path);
    indexer::rebuild(&vault, &db).map_err(err)
}

#[tauri::command]
pub fn sync_index(state: State<AppState>) -> CmdResult<indexer::IndexStats> {
    let cfg = state.config.lock().unwrap().clone();
    let vault = PathBuf::from(&cfg.vault_path);
    let db = PathBuf::from(&cfg.pastor_path);
    indexer::sync(&vault, &db).map_err(err)
}

#[tauri::command]
pub fn archive_stats(state: State<AppState>) -> CmdResult<retrieval::ArchiveStats> {
    let conn = state.pastor().map_err(err)?;
    retrieval::archive_stats(&conn).map_err(err)
}

// ---- Canon vault (read-only) ----

#[tauri::command]
pub fn get_passage(
    state: State<AppState>,
    reference: String,
) -> CmdResult<Vec<retrieval::VerseRow>> {
    let conn = state.canon().map_err(err)?;
    let p = reference::parse_passage(&reference).map_err(err)?;
    retrieval::get_passage(&conn, p.start, p.end).map_err(err)
}

#[tauri::command]
pub fn get_verse_words(
    state: State<AppState>,
    book_num: i64,
    chapter: i64,
    verse: i64,
) -> CmdResult<Vec<retrieval::WordRow>> {
    let conn = state.canon().map_err(err)?;
    retrieval::get_verse_words(&conn, book_num, chapter, verse).map_err(err)
}

#[tauri::command]
pub fn get_lexicon(
    state: State<AppState>,
    strong_id: String,
) -> CmdResult<Option<retrieval::LexiconEntry>> {
    let conn = state.canon().map_err(err)?;
    retrieval::get_lexicon(&conn, &strong_id).map_err(err)
}

#[tauri::command]
pub fn verses_for_strong(
    state: State<AppState>,
    strong_id: String,
    limit: Option<i64>,
) -> CmdResult<Vec<retrieval::VerseRow>> {
    let conn = state.canon().map_err(err)?;
    retrieval::verses_for_strong(&conn, &strong_id, limit.unwrap_or(100)).map_err(err)
}

#[tauri::command]
pub fn cross_references(
    state: State<AppState>,
    book_num: i64,
    chapter: i64,
    verse: i64,
    limit: Option<i64>,
) -> CmdResult<Vec<(retrieval::VerseRow, i64)>> {
    let conn = state.canon().map_err(err)?;
    retrieval::cross_references(&conn, book_num, chapter, verse, limit.unwrap_or(25)).map_err(err)
}

#[tauri::command]
pub fn sermons_for_verse(
    state: State<AppState>,
    reference: String,
) -> CmdResult<Vec<retrieval::SermonHit>> {
    let conn = state.pastor().map_err(err)?;
    let p = reference::parse_passage(&reference).map_err(err)?;
    retrieval::sermons_for_verse(&conn, p.start.book_num, p.start.chapter, p.start.verse).map_err(err)
}

#[tauri::command]
pub fn illustration_fatigue(
    state: State<AppState>,
    min_uses: Option<i64>,
) -> CmdResult<Vec<retrieval::IllustrationFatigue>> {
    let conn = state.pastor().map_err(err)?;
    retrieval::illustration_fatigue(&conn, min_uses.unwrap_or(2)).map_err(err)
}

// ---- Librarian (optional, off by default, fully offline) ----

#[tauri::command]
pub fn librarian_catalog(
    state: State<AppState>,
    sermon_id: String,
) -> CmdResult<Option<librarian::CatalogSuggestion>> {
    let enabled = state.config.lock().unwrap().librarian_enabled;
    if !enabled {
        // Invisible and off: return nothing rather than erroring.
        return Ok(None);
    }
    let conn = state.pastor().map_err(err)?;
    librarian::catalog(&conn, &sermon_id).map(Some).map_err(err)
}

#[tauri::command]
pub fn librarian_related(
    state: State<AppState>,
    sermon_id: String,
    limit: Option<usize>,
) -> CmdResult<Vec<librarian::RelatedSermon>> {
    let enabled = state.config.lock().unwrap().librarian_enabled;
    if !enabled {
        return Ok(Vec::new());
    }
    let conn = state.pastor().map_err(err)?;
    librarian::related_sermons(&conn, &sermon_id, limit.unwrap_or(5)).map_err(err)
}
