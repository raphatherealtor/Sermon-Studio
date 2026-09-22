//! Tauri IPC command layer.
//!
//! Every command is a thin bridge to [`crate::core_api`] (which composes the
//! Wave 1 `sermon_core` capabilities). Command names are snake_case and match
//! `src/lib/backend/TauriSermonBackend.ts` exactly; a unit test below
//! cross-checks the adapter source so the two can never drift apart.
//!
//! Classification of the Rocket `SermonBackend` contract (see Track F report):
//!   A — wired directly to a core capability
//!   B — thin composition of existing core capabilities
//!   C — Track D/E capability through thin transport mapping
//!   D — genuinely unsupported (typed unsupported error)

use crate::core_api::{self, ApiResult, CreateSermonReq};
use crate::dto;
use crate::AppState;
use serde::Deserialize;
use std::path::PathBuf;
use tauri::State;

/// All custom IPC command names, in adapter order. The unit test below checks
/// that TauriSermonBackend.ts invokes exactly these names.
#[cfg_attr(not(test), allow(dead_code))]
pub const COMMAND_NAMES: &[&str] = &[
    "list_sermons",
    "search_sermons",
    "create_sermon",
    "load_sermon",
    "save_sermon",
    "rename_sermon",
    "duplicate_sermon",
    "archive_sermon",
    "delete_sermon",
    "pin_sermon",
    "parse_references",
    "lint_sermon",
    "get_passage",
    "get_strongs",
    "get_cross_references",
    "get_preached_on",
    "get_chain_study",
    "sync_index",
    "rebuild_index",
    "rescan_library",
    "repair_index",
    "cancel_index_operation",
    "get_index_status",
    "get_archive_stats",
    "get_illustration_fatigue",
    "get_related_sermons",
    "get_passage_history",
    "get_sermon_insights",
    "set_librarian_enabled",
    "create_export_snapshot",
    "execute_export_job",
    "export_sermon",
    "reveal_exported_file",
    "resolve_conflict",
    "prepare_diff",
    "prepare_merge",
    "get_filesystem_status",
    "recover_sermon",
    "reconnect_sermon",
    "attach_research_file",
    "list_research_attachments",
    "get_research_attachment",
    "get_extracted_pages",
    "update_research_metadata",
    "remove_research_attachment",
    "open_research_file",
    "load_settings",
    "save_settings",
    "test_directive_codec",
    // V1 config + canon/librarian surface, retained (real core capabilities).
    "get_config",
    "set_config",
    "set_vault",
    "get_verse_words",
    "get_lexicon",
    "verses_for_strong",
    "cross_references",
    "sermons_for_verse",
    "librarian_catalog",
    "librarian_related",
    "list_canon_sources",
];

// ---------------------------------------------------------------------------
// Sermon list / archive (A/B)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_sermons(state: State<AppState>) -> ApiResult<Vec<dto::SermonSummaryDto>> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::list_sermons(&conn)
}

#[tauri::command]
pub fn search_sermons(
    state: State<AppState>,
    query: String,
    filters: Option<dto::SearchFiltersDto>,
) -> ApiResult<Vec<dto::SearchResultDto>> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::search_sermons(&conn, &query, &filters.unwrap_or_default())
}

#[tauri::command]
pub fn get_related_sermons(state: State<AppState>, sermon_id: String, limit: Option<usize>) -> ApiResult<sermon_core::intelligence::IntelligenceResult> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    sermon_core::intelligence::related_sermons(&conn, &sermon_id, limit.unwrap_or(10)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_passage_history(state: State<AppState>, reference: String) -> ApiResult<sermon_core::intelligence::IntelligenceResult> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    sermon_core::intelligence::passage_history(&conn, &reference).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sermon_insights(state: State<AppState>, sermon_id: String, limit: Option<usize>) -> ApiResult<sermon_core::intelligence::IntelligenceResult> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    sermon_core::intelligence::sermon_insights(&conn, &sermon_id, limit.unwrap_or(10)).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSermonRequestDto {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub scripture: Option<String>,
    #[serde(default)]
    pub series: Option<String>,
}

#[tauri::command]
pub fn create_sermon(
    state: State<AppState>,
    request: Option<CreateSermonRequestDto>,
) -> ApiResult<dto::SermonDocumentDto> {
    let cfg = state.config.lock().unwrap().clone();
    let mut conn = state.pastor().map_err(|e| e.to_string())?;
    let req = request.unwrap_or(CreateSermonRequestDto {
        title: None,
        scripture: None,
        series: None,
    });
    let (doc, baseline) = core_api::create_sermon(
        &PathBuf::from(&cfg.vault_path),
        &mut conn,
        &CreateSermonReq {
            title: req.title,
            scripture: req.scripture,
            series: req.series,
        },
    )?;
    state
        .baselines
        .lock()
        .unwrap()
        .record(&doc.id.clone(), baseline);
    Ok(doc)
}

/// The frontend loads by sermon id; the backend resolves it through the index.
#[tauri::command]
pub fn load_sermon(state: State<AppState>, id: String) -> ApiResult<dto::SermonDocumentDto> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    let rel = core_api::find_sermon_path(&conn, &id)?
        .ok_or_else(|| format!("sermon not found: {id}"))?;
    let (mut doc, baseline) = core_api::load_sermon(&PathBuf::from(&cfg.vault_path), &rel)?;
    doc.fs_state = Some(
        dto::file_state_to_ts(sermon_core::reconcile::evaluate_file_state(
            Some(baseline.hash.as_str()),
            Some(baseline.content.as_str()),
            Some(baseline.content.as_str()),
        ))
        .to_string(),
    );
    state.baselines.lock().unwrap().record(&id, baseline);
    Ok(doc)
}

#[tauri::command]
pub fn save_sermon(
    state: State<AppState>,
    doc: dto::SermonDocumentDto,
) -> ApiResult<dto::SaveResultDto> {
    let cfg = state.config.lock().unwrap().clone();
    let mut conn = state.pastor().map_err(|e| e.to_string())?;
    let mut baselines = state.baselines.lock().unwrap();
    core_api::save_sermon(&PathBuf::from(&cfg.vault_path), &mut conn, &mut baselines, &doc)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameSermonRequestDto {
    pub id: String,
    pub new_title: String,
}

#[tauri::command]
pub fn rename_sermon(
    state: State<AppState>,
    request: RenameSermonRequestDto,
) -> ApiResult<dto::SermonSummaryDto> {
    let cfg = state.config.lock().unwrap().clone();
    let mut conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::rename_sermon(
        &PathBuf::from(&cfg.vault_path),
        &mut conn,
        &request.id,
        &request.new_title,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSermonRequestDto {
    pub id: String,
    #[serde(default)]
    pub new_title: Option<String>,
}

#[tauri::command]
pub fn duplicate_sermon(
    state: State<AppState>,
    request: DuplicateSermonRequestDto,
) -> ApiResult<dto::SermonDocumentDto> {
    let cfg = state.config.lock().unwrap().clone();
    let mut conn = state.pastor().map_err(|e| e.to_string())?;
    let (doc, baseline) = core_api::duplicate_sermon(
        &PathBuf::from(&cfg.vault_path),
        &mut conn,
        &request.id,
        request.new_title.as_deref(),
    )?;
    state.baselines.lock().unwrap().record(&doc.id.clone(), baseline);
    Ok(doc)
}

#[tauri::command]
pub fn archive_sermon(state: State<AppState>, id: String) -> ApiResult<()> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::archive_sermon(
        &PathBuf::from(&cfg.vault_path),
        &PathBuf::from(&cfg.pastor_path),
        &id,
    )
}

#[tauri::command]
pub fn delete_sermon(state: State<AppState>, id: String) -> ApiResult<()> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::delete_sermon(&PathBuf::from(&cfg.vault_path), &conn, &id)?;
    state.baselines.lock().unwrap().remove(&id);
    Ok(())
}

/// D: pinning is a UI preference with no core backing yet — a typed
/// unsupported error, never a fake success.
#[tauri::command]
pub fn pin_sermon(_state: State<AppState>, _id: String, _pinned: bool) -> ApiResult<()> {
    Err("unsupported: sermon pinning is not backed by the native core yet"
        .to_string())
}

// ---------------------------------------------------------------------------
// References (A) and linting (Track E through Track F transport)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn parse_references(text: String) -> ApiResult<Vec<dto::ReferenceMatchDto>> {
    Ok(dto::scan_references(&text))
}

#[tauri::command]
pub fn lint_sermon(
    state: State<AppState>,
    doc: dto::SermonDocumentDto,
) -> ApiResult<Vec<dto::LintFindingDto>> {
    let cfg = state.config.lock().unwrap().clone();
    let pastor = state.pastor().ok();
    core_api::lint_document(
        &PathBuf::from(&cfg.vault_path),
        pastor.as_ref(),
        &doc,
    )
}

// ---------------------------------------------------------------------------
// Study rail (A/B)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_passage(state: State<AppState>, reference: String) -> ApiResult<dto::PassageResultDto> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    core_api::get_passage(&conn, &reference)
}

#[tauri::command]
pub fn get_strongs(state: State<AppState>, id: String) -> ApiResult<dto::StrongsEntryDto> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    core_api::get_strongs(&conn, &id)
}

#[tauri::command]
pub fn get_cross_references(
    state: State<AppState>,
    reference: String,
) -> ApiResult<Vec<dto::CrossReferenceDto>> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    core_api::get_cross_references(&conn, &reference)
}

#[tauri::command]
pub fn get_preached_on(
    state: State<AppState>,
    reference: String,
) -> ApiResult<Vec<dto::PreachedResultDto>> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::get_preached_on(&conn, &reference)
}

/// Chain Study (Wave 5 / Track N). Both vaults are optional at the seam: a
/// missing canon.db degrades to `canonAvailable: false`; a missing pastor.db
/// degrades to an empty "From Your Archive" overlay.
#[tauri::command]
pub fn get_chain_study(
    state: State<AppState>,
    reference: String,
) -> ApiResult<sermon_core::chain_study::ChainStudyResult> {
    let canon = state.canon().ok();
    let pastor = state.pastor().ok();
    core_api::get_chain_study(canon.as_ref(), pastor.as_ref(), &reference)
}

// ---------------------------------------------------------------------------
// Index / archive stats (A/B)
// ---------------------------------------------------------------------------

fn index_paths(state: &State<AppState>) -> (PathBuf, PathBuf) {
    let cfg = state.config.lock().unwrap().clone();
    (
        PathBuf::from(&cfg.vault_path),
        PathBuf::from(&cfg.pastor_path),
    )
}

#[tauri::command]
pub fn sync_index(state: State<AppState>) -> ApiResult<dto::IndexOperationResultDto> {
    let (vault, db) = index_paths(&state);
    core_api::sync_index(&vault, &db)
}

#[tauri::command]
pub fn rebuild_index(state: State<AppState>) -> ApiResult<dto::IndexOperationResultDto> {
    let (vault, db) = index_paths(&state);
    core_api::rebuild_index(&vault, &db)
}

#[tauri::command]
pub fn rescan_library(state: State<AppState>) -> ApiResult<dto::IndexOperationResultDto> {
    let (vault, db) = index_paths(&state);
    core_api::rescan_library(&vault, &db)
}

#[tauri::command]
pub fn repair_index(state: State<AppState>) -> ApiResult<dto::IndexOperationResultDto> {
    let (vault, db) = index_paths(&state);
    core_api::repair_index(&vault, &db)
}

/// D: the core index operations are synchronous and bounded; there is no
/// in-flight operation to cancel. Typed unsupported error, not a fake no-op
/// success.
#[tauri::command]
pub fn cancel_index_operation(_state: State<AppState>) -> ApiResult<()> {
    Err("unsupported: index operations are synchronous in the native core; \
         there is nothing to cancel"
        .to_string())
}

#[tauri::command]
pub fn get_index_status(state: State<AppState>) -> ApiResult<dto::IndexStatusDto> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::index_status(&PathBuf::from(&cfg.pastor_path))
}

#[tauri::command]
pub fn get_archive_stats(state: State<AppState>) -> ApiResult<dto::ArchiveStatsDto> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::archive_stats(&conn)
}

#[tauri::command]
pub fn get_illustration_fatigue(
    state: State<AppState>,
) -> ApiResult<Vec<dto::IllustrationFatigueDto>> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::illustration_fatigue(&conn)
}

#[tauri::command]
pub fn set_librarian_enabled(state: State<AppState>, enabled: bool) -> ApiResult<()> {
    let mut cfg = state.config.lock().unwrap();
    cfg.librarian_enabled = enabled;
    cfg.save(&state.config_path).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Export (Track D through Track F transport) / reveal (D)
// ---------------------------------------------------------------------------

fn create_snapshot(
    state: &State<AppState>,
    sermon_id: &str,
) -> ApiResult<dto::ExportSnapshotDto> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    let snapshot = core_api::create_export_source_snapshot(
        &PathBuf::from(&cfg.vault_path),
        &conn,
        sermon_id,
    )?;
    let result = snapshot.to_dto();
    state
        .export_snapshots
        .lock()
        .unwrap()
        .insert(snapshot.snapshot_id.clone(), snapshot);
    Ok(result)
}

fn execute_export(
    state: &State<AppState>,
    request: &dto::ExportRequestDto,
) -> ApiResult<dto::ExportResultDto> {
    let snapshot_id = request
        .snapshot_id
        .as_deref()
        .ok_or_else(|| "export request requires snapshotId".to_string())?;
    let snapshot = state
        .export_snapshots
        .lock()
        .unwrap()
        .get(snapshot_id)
        .cloned()
        .ok_or_else(|| format!("export snapshot not found: {snapshot_id}"))?;
    let cfg = state.config.lock().unwrap().clone();
    core_api::execute_export_snapshot(&PathBuf::from(&cfg.vault_path), &snapshot, request)
}

#[tauri::command]
pub fn create_export_snapshot(
    state: State<AppState>,
    request: dto::CreateExportSnapshotRequestDto,
) -> ApiResult<dto::ExportSnapshotDto> {
    create_snapshot(&state, &request.sermon_id)
}

#[tauri::command]
pub fn execute_export_job(
    state: State<AppState>,
    request: dto::ExportRequestDto,
) -> ApiResult<dto::ExportResultDto> {
    execute_export(&state, &request)
}

#[tauri::command]
pub fn export_sermon(
    state: State<AppState>,
    mut request: dto::ExportRequestDto,
) -> ApiResult<dto::ExportResultDto> {
    if request.snapshot_id.is_none() {
        let snapshot = create_snapshot(&state, &request.sermon_id)?;
        request.snapshot_id = Some(snapshot.snapshot_id);
    }
    execute_export(&state, &request)
}

/// D: file reveal needs a shell/opener integration that is intentionally not
/// part of this track.
#[tauri::command]
pub fn reveal_exported_file(
    _state: State<AppState>,
    _request: serde_json::Value,
) -> ApiResult<()> {
    Err("unsupported: file reveal is not available in the native backend yet"
        .to_string())
}

// ---------------------------------------------------------------------------
// Research packets (A) — thin bridges to sermon_core::research_store.
// The store owns validation, confinement, content-addressing, extraction,
// and atomic manifest updates; these commands only resolve the vault path.
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn attach_research_file(
    state: State<AppState>,
    request: dto::AttachResearchFileRequestDto,
) -> ApiResult<dto::ResearchAttachmentDto> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::attach_research_file(&PathBuf::from(&cfg.vault_path), &request)
}

#[tauri::command]
pub fn list_research_attachments(
    state: State<AppState>,
    sermon_id: String,
) -> ApiResult<Vec<dto::ResearchAttachmentDto>> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::list_research_attachments(&PathBuf::from(&cfg.vault_path), &sermon_id)
}

#[tauri::command]
pub fn get_research_attachment(
    state: State<AppState>,
    sermon_id: String,
    attachment_id: String,
) -> ApiResult<dto::ResearchAttachmentDto> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::get_research_attachment(&PathBuf::from(&cfg.vault_path), &sermon_id, &attachment_id)
}

#[tauri::command]
pub fn get_extracted_pages(
    state: State<AppState>,
    sermon_id: String,
    attachment_id: String,
) -> ApiResult<Vec<dto::ExtractedPageDto>> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::get_extracted_pages(&PathBuf::from(&cfg.vault_path), &sermon_id, &attachment_id)
}

#[tauri::command]
pub fn update_research_metadata(
    state: State<AppState>,
    sermon_id: String,
    attachment_id: String,
    request: dto::UpdateResearchMetadataRequestDto,
) -> ApiResult<dto::ResearchAttachmentDto> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::update_research_metadata(
        &PathBuf::from(&cfg.vault_path),
        &sermon_id,
        &attachment_id,
        &request,
    )
}

#[tauri::command]
pub fn remove_research_attachment(
    state: State<AppState>,
    sermon_id: String,
    attachment_id: String,
) -> ApiResult<()> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::remove_research_attachment(&PathBuf::from(&cfg.vault_path), &sermon_id, &attachment_id)
}

/// Open the stored original PDF. V1 returns the confinement-validated
/// absolute path; actual shell opening needs an opener integration that is
/// intentionally not part of this track (mirrors `reveal_exported_file`).
#[tauri::command]
pub fn open_research_file(
    state: State<AppState>,
    sermon_id: String,
    attachment_id: String,
) -> ApiResult<dto::OpenResearchFileResultDto> {
    let cfg = state.config.lock().unwrap().clone();
    core_api::open_research_file(&PathBuf::from(&cfg.vault_path), &sermon_id, &attachment_id)
}

// ---------------------------------------------------------------------------
// Conflict / filesystem reconciliation (B)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn resolve_conflict(
    state: State<AppState>,
    request: dto::ConflictResolutionDto,
) -> ApiResult<dto::SaveResultDto> {
    let cfg = state.config.lock().unwrap().clone();
    let mut conn = state.pastor().map_err(|e| e.to_string())?;
    let mut baselines = state.baselines.lock().unwrap();
    core_api::resolve_conflict(
        &PathBuf::from(&cfg.vault_path),
        &mut conn,
        &mut baselines,
        &request,
    )
}

#[tauri::command]
pub fn prepare_diff(
    state: State<AppState>,
    sermon_id: String,
) -> ApiResult<dto::DiffPreparationResultDto> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::prepare_diff(
        &PathBuf::from(&cfg.vault_path),
        &conn,
        &state.baselines.lock().unwrap(),
        &sermon_id,
    )
}

#[tauri::command]
pub fn prepare_merge(
    state: State<AppState>,
    sermon_id: String,
) -> ApiResult<dto::MergePreparationResultDto> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::prepare_merge(
        &PathBuf::from(&cfg.vault_path),
        &conn,
        &state.baselines.lock().unwrap(),
        &sermon_id,
    )
}

#[tauri::command]
pub fn get_filesystem_status(
    state: State<AppState>,
    sermon_id: String,
) -> ApiResult<dto::FilesystemReconciliationStatusDto> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::get_filesystem_status(
        &PathBuf::from(&cfg.vault_path),
        &conn,
        &state.baselines.lock().unwrap(),
        &sermon_id,
    )
}

#[tauri::command]
pub fn recover_sermon(
    state: State<AppState>,
    sermon_id: String,
) -> ApiResult<dto::RecoveryResultDto> {
    let cfg = state.config.lock().unwrap().clone();
    let conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::recover_sermon(
        &PathBuf::from(&cfg.vault_path),
        &PathBuf::from(&cfg.pastor_path),
        &conn,
        &sermon_id,
    )
}

#[tauri::command]
pub fn reconnect_sermon(
    state: State<AppState>,
    request: dto::ReconnectRequestDto,
) -> ApiResult<dto::SermonSummaryDto> {
    let cfg = state.config.lock().unwrap().clone();
    let mut conn = state.pastor().map_err(|e| e.to_string())?;
    core_api::reconnect_sermon(
        &PathBuf::from(&cfg.vault_path),
        &PathBuf::from(&cfg.pastor_path),
        &mut conn,
        &request.sermon_id,
        &request.new_path,
    )
}

// ---------------------------------------------------------------------------
// Settings (B)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn load_settings(state: State<AppState>) -> ApiResult<dto::AppSettingsDto> {
    let cfg = state.config.lock().unwrap().clone();
    Ok(core_api::settings_from_config(&cfg))
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: dto::AppSettingsDto) -> ApiResult<()> {
    let mut cfg = state.config.lock().unwrap();
    let updated = core_api::settings_to_config(&settings, &cfg);
    *cfg = updated;
    cfg.save(&state.config_path).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Codec test (A — frontend parity contract, Rust-authoritative codec)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn test_directive_codec(input: String) -> ApiResult<dto::CodecRoundTripResultDto> {
    Ok(core_api::test_directive_codec(&input))
}

// ---------------------------------------------------------------------------
// Retained V1 surface: config + canon study helpers + librarian. These are
// real core capabilities that predate the Rocket contract; kept so existing
// native screens keep working. The adapter's new surface above is primary.
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
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
        canon_present: cfg.resolve_canon_path().exists(),
        vault_path: cfg.vault_path,
        canon_path: cfg.canon_path,
        pastor_path: cfg.pastor_path,
        librarian_enabled: cfg.librarian_enabled,
        font_size: cfg.font_size,
        high_contrast: cfg.high_contrast,
    }
}

/// Attribution surface: enumerate the datasets that contributed to canon.db.
#[tauri::command]
pub fn list_canon_sources(state: State<AppState>) -> ApiResult<Vec<dto::CanonSourceDto>> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    let sources = sermon_core::canon::adapters::read_sources(&conn).map_err(|e| e.to_string())?;
    Ok(sources
        .into_iter()
        .map(|s| dto::CanonSourceDto {
            id: s.id,
            name: s.name,
            license_code: s.license_code,
            attribution: s.attribution,
            url: s.url,
            version: s.version,
        })
        .collect())
}

#[tauri::command]
pub fn set_config(state: State<AppState>, config: crate::config::AppConfig) -> ApiResult<()> {
    {
        let mut cfg = state.config.lock().unwrap();
        *cfg = config.clone();
    }
    config
        .save(&state.config_path)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_vault(state: State<AppState>, vault_path: String) -> ApiResult<ConfigView> {
    let changed = {
        let mut cfg = state.config.lock().unwrap();
        let changed = cfg.vault_path != vault_path;
        cfg.vault_path = vault_path;
        cfg.save(&state.config_path).map_err(|e| e.to_string())?;
        changed
    };
    // Fresh installs have no vault at first launch, so startup maintenance
    // was never spawned; starting it here is what makes the first picked
    // vault reconcile and watch without an app restart.
    if changed {
        let cfg = state.config.lock().unwrap().clone();
        crate::spawn_maintenance(&cfg);
    }
    Ok(get_config(state))
}

#[tauri::command]
pub fn get_verse_words(
    state: State<AppState>,
    book_num: i64,
    chapter: i64,
    verse: i64,
) -> ApiResult<Vec<sermon_core::retrieval::WordRow>> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    sermon_core::retrieval::get_verse_words(&conn, book_num, chapter, verse)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_lexicon(
    state: State<AppState>,
    strong_id: String,
) -> ApiResult<Option<sermon_core::retrieval::LexiconEntry>> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    sermon_core::retrieval::get_lexicon(&conn, &strong_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn verses_for_strong(
    state: State<AppState>,
    strong_id: String,
    limit: Option<i64>,
) -> ApiResult<Vec<sermon_core::retrieval::VerseRow>> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    sermon_core::retrieval::verses_for_strong(&conn, &strong_id, limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cross_references(
    state: State<AppState>,
    book_num: i64,
    chapter: i64,
    verse: i64,
    limit: Option<i64>,
) -> ApiResult<Vec<(sermon_core::retrieval::VerseRow, i64)>> {
    let conn = state.canon().map_err(|e| e.to_string())?;
    sermon_core::retrieval::cross_references(&conn, book_num, chapter, verse, limit.unwrap_or(25))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sermons_for_verse(
    state: State<AppState>,
    reference: String,
) -> ApiResult<Vec<sermon_core::retrieval::SermonHit>> {
    let conn = state.pastor().map_err(|e| e.to_string())?;
    let p = sermon_core::reference::parse_passage(&reference).map_err(|e| e.to_string())?;
    sermon_core::retrieval::sermons_for_verse(
        &conn,
        p.start.book_num,
        p.start.chapter,
        p.start.verse,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn librarian_catalog(
    state: State<AppState>,
    sermon_id: String,
) -> ApiResult<Option<sermon_core::librarian::CatalogSuggestion>> {
    let enabled = state.config.lock().unwrap().librarian_enabled;
    if !enabled {
        return Ok(None);
    }
    let conn = state.pastor().map_err(|e| e.to_string())?;
    sermon_core::librarian::catalog(&conn, &sermon_id)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn librarian_related(
    state: State<AppState>,
    sermon_id: String,
    limit: Option<usize>,
) -> ApiResult<Vec<sermon_core::librarian::RelatedSermon>> {
    let enabled = state.config.lock().unwrap().librarian_enabled;
    if !enabled {
        return Ok(Vec::new());
    }
    let conn = state.pastor().map_err(|e| e.to_string())?;
    sermon_core::librarian::related_sermons(&conn, &sermon_id, limit.unwrap_or(5))
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests: command-name/payload consistency with the frontend adapter.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_invokes_exactly_the_registered_command_names() {
        let root = env!("CARGO_MANIFEST_DIR");
        let adapter = std::fs::read_to_string(format!(
            "{root}/../src/lib/backend/TauriSermonBackend.ts"
        ))
        .expect("read TauriSermonBackend.ts");
        // The Rocket contract methods must all be wired through the adapter.
        // The retained V1 config/canon/librarian commands are not part of the
        // SermonBackend interface; other frontend modules may call them.
        let split = COMMAND_NAMES
            .iter()
            .position(|n| *n == "get_config")
            .expect("V1 split marker present");
        for name in &COMMAND_NAMES[..split] {
            let needle = format!("'{name}'");
            assert!(
                adapter.contains(&needle),
                "adapter does not invoke registered command {name}"
            );
        }
        // The adapter must not invent commands the backend does not register.
        for m in find_invocations(&adapter) {
            assert!(
                COMMAND_NAMES.contains(&m.as_str()),
                "adapter invokes unregistered command: {m}"
            );
        }
    }

    fn find_invocations(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = src;
        while let Some(start) = rest.find("tauriInvoke<") {
            let after = &rest[start..];
            // Skip the adapter's own generic function declaration.
            if rest[..start].ends_with("function ") {
                rest = &after[1..];
                continue;
            }
            if let Some(q) = after.find('\'') {
                let tail = &after[q + 1..];
                if let Some(e) = tail.find('\'') {
                    out.push(tail[..e].to_string());
                    rest = &tail[e..];
                    continue;
                }
            }
            break;
        }
        out
    }

    #[test]
    fn wave_two_lint_and_export_commands_are_linked() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands.rs"))
            .unwrap();
        for cmd in ["lint_sermon", "create_export_snapshot", "execute_export_job", "export_sermon"] {
            assert!(src.contains(&format!("pub fn {cmd}")), "{cmd} missing");
        }
        assert!(src.contains("core_api::lint_document"));
        assert!(src.contains("core_api::execute_export_snapshot"));
    }

    #[test]
    fn build_rs_registers_exactly_the_command_names() {
        // build.rs autogenerates `allow-<command>` ACL permissions from its
        // APP_COMMANDS list; drift between the two breaks the capability
        // grant at build time, so fail here instead.
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"))
            .expect("read build.rs");
        assert_eq!(
            COMMAND_NAMES.len(),
            60,
            "COMMAND_NAMES changed; update build.rs APP_COMMANDS too"
        );
        for name in COMMAND_NAMES {
            // The manifest must use the exact snake_case names invoked by
            // JavaScript; Tauri derives kebab-case permission identifiers.
            let needle = format!("\"{name}\",");
            assert!(
                src.contains(&needle),
                "build.rs APP_COMMANDS missing command: {name}"
            );
        }
    }
}
