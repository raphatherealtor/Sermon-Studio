//! Deterministic reconciliation between the canonical Markdown vault and the
//! derived `pastor.db` index.
//!
//! Architecture (non-negotiable invariants):
//!
//! * Markdown sermon files on disk are **canonical**; `pastor.db` is derived
//!   and rebuildable at any time.
//! * **Startup reconciliation is the correctness mechanism.** It recovers from
//!   crashes, stale databases, externally edited files, interrupted indexing,
//!   newly copied files, and removed files — deterministically, from a full
//!   SHA-256 comparison of disk state vs. derived state. Modification
//!   timestamps are never used for correctness decisions.
//! * The file watcher ([`crate::vault_watcher`]) is an **optimization only**:
//!   it triggers targeted repairs quickly, but dropping its events can never
//!   lose data because startup reconciliation repairs everything.
//! * Canonical files are **never** modified, removed, or fabricated by this
//!   module. Only derived state is repaired.
//! * A missing file is never silently treated as "still indexed": the index
//!   row is dropped (the index reflects disk reality) and the disappearance
//!   is *recorded* in `index_meta` so the UI can surface the state instead of
//!   silently collapsing it. If the file reappears, the record is cleared and
//!   the sermon is re-indexed (reconnected/recovered).
//! * Sermon identity is the UUID in the frontmatter (`id:`), never the file
//!   path. Renames/moves preserve identity; two files claiming one identity
//!   are reported as collisions, deterministically resolved to the
//!   lexicographically first path, and never silently collapsed.
//!
//! The missing/collision bookkeeping lives in the pre-existing `index_meta`
//! key/value table (JSON values) so this module needs **no schema changes**.

use crate::error::{CoreError, Result};
use crate::indexer::{self, open_pastor_db};
use crate::sermon::{self, SermonDoc};
use crate::vault_watcher;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Filesystem / editor state model
// ---------------------------------------------------------------------------

/// The materially different editor-vs-disk states. These are never silently
/// collapsed: `DirtyLocal`, `DiskChanged`, and `Conflict` demand different UI
/// affordances (save / reload / resolve).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// Nothing changed on either side (or both sides converged byte-for-byte).
    Clean,
    /// The preacher has unsaved local changes; disk is at the baseline.
    DirtyLocal,
    /// The disk file changed (external edit); the editor holds the baseline.
    DiskChanged,
    /// Both the editor and the disk moved away from the baseline: a real
    /// conflict. Never auto-resolved; the caller offers Keep Local / Use
    /// Disk / Merge / Save Local As using the hashes in [`ConflictReport`].
    Conflict,
    /// The disk file vanished while the session holds content for it.
    Missing,
}

/// Pure, allocation-light evaluation of editor-vs-disk state from content
/// (hashed with SHA-256). No I/O, fully deterministic — the single authority
/// for conflict classification.
///
/// * `baseline_hash` — SHA-256 of the content when the editor session loaded
///   the file (`None` for sessions that never had an on-disk baseline, e.g. a
///   brand-new untitled sermon).
/// * `local_content` — the editor buffer, if one is held.
/// * `disk_content` — current disk bytes, `None` when the file is gone.
pub fn evaluate_file_state(
    baseline_hash: Option<&str>,
    local_content: Option<&str>,
    disk_content: Option<&str>,
) -> FileState {
    match baseline_hash {
        Some(baseline) => {
            let disk_changed = disk_content
                .map(|c| sermon::sha256_hex(c.as_bytes()) != baseline)
                .unwrap_or(true); // file gone => disk state left the baseline
            let local_changed = local_content
                .map(|c| sermon::sha256_hex(c.as_bytes()) != baseline)
                .unwrap_or(false);
            if disk_content.is_none() {
                FileState::Missing
            } else if local_changed && disk_changed {
                if local_content == disk_content {
                    // Both sides hold identical bytes: converged (e.g. the
                    // disk file was written from this very buffer).
                    FileState::Clean
                } else {
                    FileState::Conflict
                }
            } else if local_changed {
                FileState::DirtyLocal
            } else if disk_changed {
                FileState::DiskChanged
            } else {
                FileState::Clean
            }
        }
        None => match (local_content, disk_content) {
            // Session without baseline: a file appearing under the path we
            // are editing into is treated as external input, not a trample.
            (Some(local), Some(disk)) => {
                if local == disk {
                    FileState::Clean
                } else {
                    FileState::Conflict
                }
            }
            (Some(_), None) => FileState::DirtyLocal,
            (None, _) => FileState::Clean,
        },
    }
}

/// Everything the frontend needs to offer Keep Local / Use Disk / Merge /
/// Save Local As — without embedding either version (the caller already holds
/// the local buffer and can read the disk version through normal commands).
#[derive(Debug, Clone, Serialize)]
pub struct ConflictReport {
    pub file_path: String,
    pub state: FileState,
    pub baseline_hash: Option<String>,
    pub local_hash: Option<String>,
    pub disk_hash: Option<String>,
    /// Best-effort disk mtime (RFC 3339) for display only — never a
    /// correctness signal.
    pub disk_modified_at: Option<String>,
    pub has_local_changes: bool,
}

/// Inspect one in-editor sermon against the current disk content.
pub fn inspect_file_state(
    vault: &Path,
    rel_path: &str,
    baseline_hash: Option<&str>,
    local_content: Option<&str>,
) -> Result<ConflictReport> {
    let disk_content = crate::atomic_save::read_vault_file(vault, rel_path).ok();
    let state = evaluate_file_state(baseline_hash, local_content, disk_content.as_deref());
    Ok(ConflictReport {
        file_path: rel_path.to_string(),
        state,
        baseline_hash: baseline_hash.map(str::to_string),
        local_hash: local_content.map(|c| sermon::sha256_hex(c.as_bytes())),
        disk_hash: disk_content.as_ref().map(|c| sermon::sha256_hex(c.as_bytes())),
        disk_modified_at: crate::atomic_save::vault_file_modified_at(vault, rel_path),
        has_local_changes: state == FileState::DirtyLocal || state == FileState::Conflict,
    })
}

// ---------------------------------------------------------------------------
// Vault scanning
// ---------------------------------------------------------------------------

/// One canonical Markdown file discovered in the vault.
#[derive(Debug, Clone, Serialize)]
pub struct DiskEntry {
    /// Vault-relative path with forward slashes (platform-stable, matches
    /// `sermon_index.file_path`).
    pub rel_path: String,
    /// SHA-256 of the raw file bytes.
    pub content_hash: String,
    pub size_bytes: u64,
    /// Durable identity from frontmatter (`id:`), when present.
    pub frontmatter_id: Option<String>,
}

/// Result of a full vault scan (deterministically sorted by relative path).
#[derive(Debug, Clone, Serialize)]
pub struct VaultScan {
    pub entries: Vec<DiskEntry>,
    /// Files that could not be read (permissions, encoding). Canonical files
    /// are never modified because of this — the error is only reported.
    pub read_errors: Vec<String>,
}

/// Enumerate candidate sermon files exactly the way the indexer does (same
/// extension and hidden-component rules), deterministically sorted.
pub(crate) fn iter_vault_markdown(vault: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for entry in WalkDir::new(vault)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = indexer::rel_path_string(vault, path);
        if indexer::is_hidden_rel(&rel) {
            continue;
        }
        out.push((rel, path.to_path_buf()));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Scan the whole vault and hash every sermon file (SHA-256, raw bytes).
pub fn scan_vault(vault: &Path) -> Result<VaultScan> {
    if !vault.is_dir() {
        return Err(CoreError::VaultMissing(vault.display().to_string()));
    }
    let mut entries = Vec::new();
    let mut read_errors = Vec::new();
    for (rel, full) in iter_vault_markdown(vault) {
        let size_bytes = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
        match std::fs::read_to_string(&full) {
            Ok(raw) => {
                let frontmatter_id = SermonDoc::parse(&raw)
                    .ok()
                    .and_then(|d| d.frontmatter.id)
                    .filter(|s| !s.trim().is_empty());
                entries.push(DiskEntry {
                    content_hash: sermon::sha256_hex(raw.as_bytes()),
                    frontmatter_id,
                    rel_path: rel,
                    size_bytes,
                });
            }
            Err(e) => read_errors.push(format!("{rel}: {e}")),
        }
    }
    Ok(VaultScan {
        entries,
        read_errors,
    })
}

// ---------------------------------------------------------------------------
// Missing / collision bookkeeping (index_meta JSON — no schema changes)
// ---------------------------------------------------------------------------

const MISSING_META_KEY: &str = "missing_sermons";
const COLLISIONS_META_KEY: &str = "id_collisions";
const LAST_RECONCILE_KEY: &str = "last_reconcile";

/// A sermon whose canonical file disappeared. Recorded evidence, not truth:
/// the derived index no longer lists the sermon (disk reality wins), and this
/// record lets the UI show *what* went missing instead of silently deleting it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingRecord {
    pub sermon_id: Option<String>,
    pub file_path: String,
    pub last_hash: Option<String>,
    pub detected_at: String,
}

fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v = conn
        .query_row(
            "SELECT value FROM index_meta WHERE key = ?1",
            params![key],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(v)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Read the missing-sermon bookkeeping (`rel_path` -> record).
pub fn missing_sermons(conn: &Connection) -> Result<BTreeMap<String, MissingRecord>> {
    match get_meta(conn, MISSING_META_KEY)? {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(BTreeMap::new()),
    }
}

fn write_missing_sermons(
    conn: &Connection,
    map: &BTreeMap<String, MissingRecord>,
) -> Result<()> {
    set_meta(conn, MISSING_META_KEY, &serde_json::to_string(map)?)?;
    Ok(())
}

/// Read recorded identity collisions from the last reconciliation.
pub fn id_collisions(conn: &Connection) -> Result<Vec<IdCollision>> {
    match get_meta(conn, COLLISIONS_META_KEY)? {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Plan / report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReindexReason {
    /// On disk, not in the index (newly copied or first run).
    New,
    /// Hash differs from the indexed hash (externally edited, interrupted
    /// indexing, stale row).
    Changed,
    /// Same frontmatter UUID appeared at a new path: identity continuity.
    Renamed,
    /// A previously missing file came back (reconnected/recovered).
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ReconcileAction {
    /// (Re-)index one canonical file. Never modifies the file itself.
    Reindex {
        rel_path: String,
        reason: ReindexReason,
    },
    /// Drop derived rows for a file that no longer exists on disk and record
    /// the disappearance. Never fabricates the file.
    RemoveMissing { rel_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdCollision {
    pub sermon_id: String,
    /// Lexicographically first path — the deterministic index winner.
    pub winner: String,
    pub losers: Vec<String>,
}

/// What reconciliation *would* do, without writing anything.
#[derive(Debug, Clone, Serialize)]
pub struct ReconcilePlan {
    /// Reindex actions first (sorted by path), then removals (sorted).
    pub actions: Vec<ReconcileAction>,
    pub unchanged: Vec<String>,
    /// Missing-bookkeeping entries to clear because the file is back.
    pub recovered_clear: Vec<String>,
    pub id_collisions: Vec<IdCollision>,
    pub read_errors: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ReconcileReport {
    pub scanned: usize,
    pub unchanged: usize,
    pub new_files: usize,
    pub changed_files: usize,
    pub renamed: usize,
    pub recovered: usize,
    /// Total files actually (re-)indexed.
    pub reindexed: usize,
    pub removed_missing: usize,
    pub id_collisions: Vec<IdCollision>,
    pub read_errors: Vec<String>,
    /// Files detected missing during this run.
    pub missing_now: Vec<String>,
    pub dry_run: bool,
    pub elapsed_ms: u128,
}

fn load_indexed(conn: &Connection) -> Result<HashMap<String, (String, String)>> {
    let mut stmt = conn.prepare("SELECT file_path, id, file_hash FROM sermon_index")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (p, id, h) = row?;
        map.insert(p, (id, h));
    }
    Ok(map)
}

/// Compare a completed vault scan against the derived index and classify
/// every difference. Pure read + pure classification — no writes.
pub fn plan_reconciliation(vault: &Path, db_path: &Path) -> Result<ReconcilePlan> {
    let scan = scan_vault(vault)?;
    let conn = open_pastor_db(db_path)?;
    plan_from_scan(&conn, &scan)
}

fn plan_from_scan(conn: &Connection, scan: &VaultScan) -> Result<ReconcilePlan> {
    let indexed = load_indexed(conn)?;
    let missing_meta = missing_sermons(conn)?;

    // Identity collisions: two+ disk files claiming one frontmatter UUID.
    // Deterministic winner: lexicographically first path.
    let mut ids_on_disk: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in &scan.entries {
        if let Some(id) = &e.frontmatter_id {
            ids_on_disk.entry(id.clone()).or_default().insert(e.rel_path.clone());
        }
    }
    let mut id_collisions = Vec::new();
    let mut collision_losers: HashSet<String> = HashSet::new();
    for (id, paths) in &ids_on_disk {
        if paths.len() > 1 {
            let mut it = paths.iter();
            let winner = it.next().cloned().unwrap_or_default();
            let losers: Vec<String> = it.cloned().collect();
            collision_losers.extend(losers.iter().cloned());
            id_collisions.push(IdCollision {
                sermon_id: id.clone(),
                winner,
                losers,
            });
        }
    }

    let disk_paths: HashSet<&String> = scan.entries.iter().map(|e| &e.rel_path).collect();
    let mut id_to_indexed_path: HashMap<&str, &str> = HashMap::new();
    for (p, (id, _)) in &indexed {
        id_to_indexed_path.entry(id.as_str()).or_insert(p.as_str());
    }

    let mut actions = Vec::new();
    let mut unchanged = Vec::new();

    for e in &scan.entries {
        if collision_losers.contains(&e.rel_path) {
            continue; // never index a collision loser
        }
        // A file recorded missing that is back on disk is a recovery,
        // regardless of whether its index row survived.
        let reason = if missing_meta.contains_key(&e.rel_path) {
            Some(ReindexReason::Recovered)
        } else {
            match indexed.get(&e.rel_path) {
                Some((_, idx_hash)) if idx_hash == &e.content_hash => {
                    unchanged.push(e.rel_path.clone());
                    None
                }
                Some(_) => Some(ReindexReason::Changed),
                None => {
                    if let Some(fid) = &e.frontmatter_id {
                        if let Some(other) = id_to_indexed_path.get(fid.as_str()) {
                            if *other != e.rel_path.as_str() {
                                Some(ReindexReason::Renamed)
                            } else {
                                Some(ReindexReason::New)
                            }
                        } else {
                            Some(ReindexReason::New)
                        }
                    } else {
                        Some(ReindexReason::New)
                    }
                }
            }
        };
        if let Some(reason) = reason {
            actions.push(ReconcileAction::Reindex {
                rel_path: e.rel_path.clone(),
                reason,
            });
        }
    }

    // Indexed paths no longer on disk.
    let mut removals = Vec::new();
    let disk_ids: HashSet<&str> = ids_on_disk.keys().map(String::as_str).collect();
    for (p, (id, _)) in &indexed {
        if disk_paths.contains(p) {
            continue;
        }
        if disk_ids.contains(id.as_str()) {
            // Identity continued at another path: the reindex of the new path
            // already replaces the old row (index_one deletes by path OR id).
            // Not a disappearance — no missing record for a rename.
            continue;
        }
        removals.push(ReconcileAction::RemoveMissing { rel_path: p.clone() });
    }
    // Reindexes first, then removals (a rename's delete-by-id must run before
    // its old path is considered for removal bookkeeping).
    actions.extend(removals);

    // Any recorded missing file that is back on disk gets its record cleared,
    // whether it needed a reindex or returned byte-identical.
    let recovered_clear: Vec<String> = missing_meta
        .keys()
        .filter(|p| disk_paths.contains(p))
        .cloned()
        .collect();

    Ok(ReconcilePlan {
        actions,
        unchanged,
        recovered_clear,
        id_collisions,
        read_errors: scan.read_errors.clone(),
    })
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

fn index_file(
    vault: &Path,
    conn: &mut Connection,
    rel_path: &str,
) -> Result<()> {
    let raw = crate::atomic_save::read_vault_file(vault, rel_path)?;
    let doc = SermonDoc::parse(&raw)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(conn, rel_path, &doc, &mut stats)?;
    Ok(())
}

/// Remove every derived row for a sermon (`file_path` keyed). FTS is a
/// self-contained table, so its row is deleted explicitly; the FK-cascaded
/// tables go with the `sermon_index` row.
fn remove_derived_rows(conn: &Connection, rel_path: &str) -> Result<Option<(String, String)>> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT id, file_hash FROM sermon_index WHERE file_path = ?1",
            params![rel_path],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, _hash)) = &row {
        conn.execute("DELETE FROM sermons_fts WHERE sermon_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM sermon_index WHERE file_path = ?1",
            params![rel_path],
        )?;
    }
    Ok(row)
}

fn execute_plan(
    vault: &Path,
    conn: &mut Connection,
    plan: &ReconcilePlan,
    dry_run: bool,
) -> Result<ReconcileReport> {
    let mut report = ReconcileReport {
        dry_run,
        ..Default::default()
    };
    if dry_run {
        for a in &plan.actions {
            match a {
                ReconcileAction::Reindex { reason, .. } => {
                    report.reindexed += 1;
                    match reason {
                        ReindexReason::New => report.new_files += 1,
                        ReindexReason::Changed => report.changed_files += 1,
                        ReindexReason::Renamed => report.renamed += 1,
                        ReindexReason::Recovered => report.recovered += 1,
                    }
                }
                ReconcileAction::RemoveMissing { rel_path } => {
                    report.removed_missing += 1;
                    report.missing_now.push(rel_path.clone());
                }
            }
        }
        report.unchanged = plan.unchanged.len();
        report.id_collisions = plan.id_collisions.clone();
        report.read_errors = plan.read_errors.clone();
        return Ok(report);
    }

    let mut missing = missing_sermons(conn)?;
    report.read_errors = plan.read_errors.clone();

    // Reindexes first so a rename's delete-by-id runs before removal checks.
    for action in &plan.actions {
        match action {
            ReconcileAction::Reindex { rel_path, reason } => match index_file(vault, conn, rel_path)
            {
                Ok(()) => {
                    report.reindexed += 1;
                    match reason {
                        ReindexReason::New => report.new_files += 1,
                        ReindexReason::Changed => report.changed_files += 1,
                        ReindexReason::Renamed => report.renamed += 1,
                        ReindexReason::Recovered => report.recovered += 1,
                    }
                }
                Err(e) => {
                    // The canonical file stays exactly as it is; we only
                    // exclude stale derived text and report the failure.
                    remove_derived_rows(conn, rel_path)?;
                    report.read_errors.push(format!("{rel_path}: {e}"));
                }
            },
            ReconcileAction::RemoveMissing { rel_path } => {
                let row = remove_derived_rows(conn, rel_path)?;
                let entry = missing.entry(rel_path.clone()).or_insert_with(|| {
                    let (sermon_id, last_hash) = row
                        .as_ref()
                        .map(|(id, h)| (Some(id.clone()), Some(h.clone())))
                        .unwrap_or((None, None));
                    MissingRecord {
                        sermon_id,
                        file_path: rel_path.clone(),
                        last_hash,
                        detected_at: chrono::Utc::now().to_rfc3339(),
                    }
                });
                // Refresh evidence for repeated runs; keep first detection.
                if let Some((id, h)) = &row {
                    entry.sermon_id = Some(id.clone());
                    entry.last_hash = Some(h.clone());
                }
                report.removed_missing += 1;
                report.missing_now.push(rel_path.clone());
            }
        }
    }

    // Collision losers: drop their derived rows (files untouched) so the
    // index holds exactly the deterministic winner.
    for c in &plan.id_collisions {
        for loser in &c.losers {
            remove_derived_rows(conn, loser)?;
        }
    }

    // Clear recovered bookkeeping.
    for rel in &plan.recovered_clear {
        missing.remove(rel);
    }
    write_missing_sermons(conn, &missing)?;
    set_meta(
        conn,
        COLLISIONS_META_KEY,
        &serde_json::to_string(&plan.id_collisions)?,
    )?;
    set_meta(conn, LAST_RECONCILE_KEY, &chrono::Utc::now().to_rfc3339())?;

    report.unchanged = plan.unchanged.len();
    report.id_collisions = plan.id_collisions.clone();
    Ok(report)
}

/// Full reconciliation: scan the vault, compare against the derived index,
/// and repair exactly what differs. Canonical files are never written.
pub fn reconcile(vault: &Path, db_path: &Path) -> Result<ReconcileReport> {
    let start = Instant::now();
    let scan = scan_vault(vault)?;
    let mut conn = open_pastor_db(db_path)?;
    let plan = plan_from_scan(&conn, &scan)?;
    let mut report = execute_plan(vault, &mut conn, &plan, false)?;
    report.scanned = scan.entries.len();
    report.elapsed_ms = start.elapsed().as_millis();
    Ok(report)
}

/// Dry run of [`reconcile`]: classification only, zero writes.
pub fn plan_reconciliation_report(vault: &Path, db_path: &Path) -> Result<ReconcileReport> {
    let start = Instant::now();
    let scan = scan_vault(vault)?;
    let mut conn = open_pastor_db(db_path)?;
    let plan = plan_from_scan(&conn, &scan)?;
    let mut report = execute_plan(vault, &mut conn, &plan, true)?;
    report.scanned = scan.entries.len();
    report.elapsed_ms = start.elapsed().as_millis();
    Ok(report)
}

/// Targeted repair for a specific set of vault-relative paths — the entry
/// point the file watcher uses after its debounce window fires. Still
/// hash-driven (events are hints; the hash decides whether work happens).
pub fn reconcile_paths(
    vault: &Path,
    db_path: &Path,
    rel_paths: &[String],
) -> Result<ReconcileReport> {
    let start = Instant::now();
    let mut conn = open_pastor_db(db_path)?;
    let mut report = ReconcileReport::default();
    let mut missing = missing_sermons(&conn)?;
    let mut missing_dirty = false;

    let mut seen = HashSet::new();
    for rel in rel_paths {
        if rel.is_empty() || !seen.insert(rel.as_str()) {
            continue;
        }
        report.scanned += 1;
        let indexed_hash: Option<String> = conn
            .query_row(
                "SELECT file_hash FROM sermon_index WHERE file_path = ?1",
                params![rel],
                |r| r.get(0),
            )
            .optional()?;

        if crate::atomic_save::vault_file_exists(vault, rel) {
            let was_missing = missing.remove(rel).is_some();
            if was_missing {
                missing_dirty = true;
            }
            let raw = crate::atomic_save::read_vault_file(vault, rel);
            match raw.and_then(|raw| SermonDoc::parse(&raw).map(|d| (raw, d))) {
                Ok((raw, doc)) => {
                    let hash = sermon::sha256_hex(raw.as_bytes());
                    if indexed_hash.as_deref() == Some(hash.as_str()) && !was_missing {
                        report.unchanged += 1;
                        continue;
                    }
                    let mut stats = indexer::IndexStats::default();
                    indexer::index_single(&mut conn, rel, &doc, &mut stats)?;
                    report.reindexed += 1;
                    if was_missing {
                        report.recovered += 1;
                    } else if indexed_hash.is_some() {
                        report.changed_files += 1;
                    } else {
                        report.new_files += 1;
                    }
                }
                Err(e) => {
                    remove_derived_rows(&conn, rel)?;
                    report.read_errors.push(format!("{rel}: {e}"));
                }
            }
        } else {
            // Gone from disk. If the index still lists it, repair derived
            // state and record the disappearance.
            if let Some(hash) = indexed_hash {
                let row: Option<String> = conn
                    .query_row(
                        "SELECT id FROM sermon_index WHERE file_path = ?1",
                        params![rel],
                        |r| r.get(0),
                    )
                    .optional()?;
                remove_derived_rows(&conn, rel)?;
                let detected = missing.entry(rel.clone()).or_insert_with(|| MissingRecord {
                    sermon_id: row.clone(),
                    file_path: rel.clone(),
                    last_hash: Some(hash.clone()),
                    detected_at: chrono::Utc::now().to_rfc3339(),
                });
                detected.last_hash = Some(hash);
                missing_dirty = true;
                report.removed_missing += 1;
                report.missing_now.push(rel.clone());
            }
        }
    }

    if missing_dirty {
        write_missing_sermons(&conn, &missing)?;
    }
    report.elapsed_ms = start.elapsed().as_millis();
    Ok(report)
}

/// The startup correctness mechanism. Deterministically repairs the derived
/// index from canonical disk state after crashes, stale databases, external
/// edits, interrupted indexing, copied files, and removals.
///
/// Creates the database (and its parent directory) if it does not exist —
/// deleting `pastor.db` must never endanger sermons. A missing vault is an
/// error (`CoreError::VaultMissing`): fresh installs call this after the user
/// picks a vault, and callers must treat failure as non-fatal to app startup.
pub fn startup_reconcile(vault: &Path, db_path: &Path) -> Result<ReconcileReport> {
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    reconcile(vault, db_path)
}

/// One-call background maintenance for the desktop app: run startup
/// reconciliation, then keep a debounced watcher running that performs
/// targeted repairs. Never blocks and never panics the caller; failures are
/// reported on stderr and are always recoverable by the next reconciliation.
///
/// The watcher handle is intentionally kept alive for the process lifetime.
pub fn spawn_startup_maintenance(vault: PathBuf, db_path: PathBuf) {
    std::thread::spawn(move || {
        match startup_reconcile(&vault, &db_path) {
            Ok(r) => eprintln!(
                "startup reconciliation: scanned={} reindexed={} unchanged={} removed_missing={} errors={}",
                r.scanned, r.reindexed, r.unchanged, r.removed_missing, r.read_errors.len()
            ),
            Err(e) => eprintln!("startup reconciliation deferred: {e}"),
        }
        let vault_for_cb = vault.clone();
        let db_for_cb = db_path.clone();
        match vault_watcher::watch_vault(
            vault.clone(),
            vault_watcher::DEFAULT_DEBOUNCE,
            move |rel_paths| {
                if let Err(e) = reconcile_paths(&vault_for_cb, &db_for_cb, &rel_paths) {
                    eprintln!("targeted reconciliation failed (startup pass will repair): {e}");
                }
            },
        ) {
            Ok(handle) => {
                // Keep the watcher alive for the whole app lifetime.
                std::mem::forget(handle);
            }
            Err(e) => eprintln!(
                "vault watcher unavailable (reconciliation remains the correctness mechanism): {e}"
            ),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval;
    use std::io::Write as _;

    fn sermon_md(id: &str, title: &str, body: &str) -> String {
        format!(
            "---\nid: {id}\ntitle: \"{title}\"\nprimary_passage: \"Rom.8.28\"\nbig_idea: \"{title} idea\"\nstructure_type: verse_by_verse\n---\n\n{body}\n"
        )
    }

    fn write_file(dir: &Path, name: &str, content: &str) {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    fn indexed_paths(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT file_path FROM sermon_index ORDER BY file_path")
            .unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    fn fts_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM sermons_fts", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn reconcile_indexes_new_vault_then_skips_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "God works all things."));
        write_file(&vault, "b.md", &sermon_md("beta", "Beta", "God so loved."));

        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.new_files, 2);
        assert_eq!(report.reindexed, 2);
        assert_eq!(report.unchanged, 0);

        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["a.md", "b.md"]);
        assert_eq!(fts_count(&conn), 2);
        drop(conn);

        // Second run: everything unchanged, nothing rewritten.
        let again = reconcile(&vault, &db).unwrap();
        assert_eq!(again.scanned, 2);
        assert_eq!(again.unchanged, 2);
        assert_eq!(again.reindexed, 0);
    }

    #[test]
    fn reconcile_detects_modified_content_via_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "first draft"));

        reconcile(&vault, &db).unwrap();

        // Same length, same name — only the content bytes differ.
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "second draft"));
        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.changed_files, 1);
        assert_eq!(report.reindexed, 1);

        // The derived row and FTS reflect the new content.
        let conn = open_pastor_db(&db).unwrap();
        let (title, hash): (String, String) = conn
            .query_row("SELECT title, file_hash FROM sermon_index WHERE file_path='a.md'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(title, "Alpha");
        let raw = std::fs::read_to_string(vault.join("a.md")).unwrap();
        assert_eq!(hash, sermon::sha256_hex(raw.as_bytes()));
        let hits = retrieval::search_sermons(&conn, "second", 10).unwrap();
        assert_eq!(hits.len(), 1, "FTS must be refreshed for changed sermons");
        let stale = retrieval::search_sermons(&conn, "first", 10).unwrap();
        assert!(stale.is_empty(), "stale FTS content must not linger");
    }

    #[test]
    fn reconcile_repairs_stale_index_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        reconcile(&vault, &db).unwrap();

        // Simulate a corrupted/stale derived row (e.g. interrupted indexing).
        let conn = open_pastor_db(&db).unwrap();
        conn.execute(
            "UPDATE sermon_index SET file_hash = 'deadbeef', title = 'STALE' WHERE file_path = 'a.md'",
            [],
        )
        .unwrap();
        drop(conn);

        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.changed_files, 1, "stale row must be detected by hash");

        let conn = open_pastor_db(&db).unwrap();
        let title: String = conn
            .query_row("SELECT title FROM sermon_index WHERE file_path='a.md'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Alpha");
    }

    #[test]
    fn missing_sermon_is_recorded_not_fabricated() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        write_file(&vault, "b.md", &sermon_md("beta", "Beta", "body"));
        reconcile(&vault, &db).unwrap();

        std::fs::remove_file(vault.join("a.md")).unwrap();
        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.removed_missing, 1);
        assert_eq!(report.missing_now, vec!["a.md"]);

        // Index reflects disk reality; the file is NOT fabricated.
        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["b.md"]);
        assert_eq!(fts_count(&conn), 1);
        let missing = missing_sermons(&conn).unwrap();
        let rec = missing.get("a.md").expect("missing state must be recorded");
        assert_eq!(rec.sermon_id.as_deref(), Some("alpha"));
        drop(conn);
        assert!(!vault.join("a.md").exists(), "must never fabricate a sermon file");
    }

    #[test]
    fn missing_sermon_recovers_when_file_returns() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        reconcile(&vault, &db).unwrap();
        std::fs::remove_file(vault.join("a.md")).unwrap();
        reconcile(&vault, &db).unwrap();

        // Reconnected/recovered: identical file returns (e.g. sync tool).
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.recovered, 1);
        assert_eq!(report.reindexed, 1);

        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["a.md"]);
        assert!(missing_sermons(&conn).unwrap().is_empty());
    }

    #[test]
    fn deleting_pastor_db_is_harmless_full_rebuild_via_reconcile() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        write_file(&vault, "b.md", &sermon_md("beta", "Beta", "body"));
        reconcile(&vault, &db).unwrap();

        // A user deleting the derived database must never endanger sermons.
        drop(open_pastor_db(&db));
        std::fs::remove_file(&db).unwrap();
        let report = startup_reconcile(&vault, &db).unwrap();
        assert_eq!(report.new_files, 2);

        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["a.md", "b.md"]);
        assert_eq!(fts_count(&conn), 2);
    }

    #[test]
    fn rename_preserves_uuid_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "2024/a.md", &sermon_md("alpha", "Alpha", "body"));
        reconcile(&vault, &db).unwrap();

        std::fs::create_dir_all(vault.join("2025")).unwrap();
        std::fs::rename(vault.join("2024/a.md"), vault.join("2025/renamed.md")).unwrap();
        std::fs::remove_dir(vault.join("2024")).unwrap();
        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.renamed, 1, "UUID continuity must be detected");

        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["2025/renamed.md"]);
        let id: String = conn
            .query_row("SELECT id FROM sermon_index", [], |r| r.get(0))
            .unwrap();
        assert_eq!(id, "alpha");
        assert!(missing_sermons(&conn).unwrap().is_empty(), "a rename is not a disappearance");
    }

    #[test]
    fn parse_error_is_reported_and_file_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "good.md", &sermon_md("good", "Good", "body"));
        let bad = "---\nid: bad\ntitle: \"unterminated\n---\n\nbody\n";
        write_file(&vault, "bad.md", bad);

        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.new_files, 1);
        assert_eq!(report.read_errors.len(), 1, "parse failure must be reported");
        assert!(report.read_errors[0].starts_with("bad.md"));

        // The canonical file is untouched and simply not indexed.
        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["good.md"]);
        drop(conn);
        assert_eq!(std::fs::read_to_string(vault.join("bad.md")).unwrap(), bad);
    }

    #[test]
    fn identity_collision_is_deterministic_and_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("dup", "Winner A", "body a"));
        write_file(&vault, "z.md", &sermon_md("dup", "Copy Z", "body z"));

        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.id_collisions.len(), 1);
        assert_eq!(report.id_collisions[0].winner, "a.md");
        assert_eq!(report.id_collisions[0].losers, vec!["z.md"]);

        let conn = open_pastor_db(&db).unwrap();
        assert_eq!(indexed_paths(&conn), vec!["a.md"]);
        let collisions = id_collisions(&conn).unwrap();
        assert_eq!(collisions.len(), 1);
    }

    #[test]
    fn targeted_reconcile_repairs_only_requested_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        write_file(&vault, "b.md", &sermon_md("beta", "Beta", "body"));
        write_file(&vault, "c.md", &sermon_md("gamma", "Gamma", "body"));
        reconcile(&vault, &db).unwrap();

        write_file(&vault, "b.md", &sermon_md("beta", "Beta Revised", "body 2"));
        let report = reconcile_paths(&vault, &db, &["b.md".to_string()]).unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.changed_files, 1);
        assert_eq!(report.reindexed, 1);
        assert_eq!(report.unchanged, 0);

        let conn = open_pastor_db(&db).unwrap();
        let title: String = conn
            .query_row("SELECT title FROM sermon_index WHERE file_path='b.md'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Beta Revised");
    }

    #[test]
    fn targeted_reconcile_handles_disappearance_and_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        reconcile(&vault, &db).unwrap();

        std::fs::remove_file(vault.join("a.md")).unwrap();
        let report = reconcile_paths(&vault, &db, &["a.md".to_string()]).unwrap();
        assert_eq!(report.removed_missing, 1);
        let conn = open_pastor_db(&db).unwrap();
        assert!(missing_sermons(&conn).unwrap().contains_key("a.md"));
        drop(conn);

        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));
        let back = reconcile_paths(&vault, &db, &["a.md".to_string()]).unwrap();
        assert_eq!(back.recovered, 1);
        let conn = open_pastor_db(&db).unwrap();
        assert!(missing_sermons(&conn).unwrap().is_empty());
        assert_eq!(indexed_paths(&conn), vec!["a.md"]);
    }

    #[test]
    fn index_failure_after_save_keeps_canonical_file() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        reconcile(&vault, &db).unwrap();

        // Save succeeds and the file is durably on disk...
        let content = sermon_md("alpha", "Alpha", "saved before index failure");
        let hash = crate::atomic_save::save_sermon_atomic(&vault, "a.md", &content).unwrap();
        // ...but the derived indexing step fails (database path unusable:
        // its parent "directory" is a regular file).
        let blocker = tmp.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        let bogus_db = blocker.join("pastor.db");
        assert!(startup_reconcile(&vault, &bogus_db).is_err(), "indexing must fail here");

        // The sermon still exists safely on disk, byte-for-byte what we saved.
        let on_disk = std::fs::read_to_string(vault.join("a.md")).unwrap();
        assert_eq!(sermon::sha256_hex(on_disk.as_bytes()), hash);

        // A later reconciliation repairs the derived index.
        let report = reconcile(&vault, &db).unwrap();
        assert_eq!(report.new_files, 1);
        let conn = open_pastor_db(&db).unwrap();
        let stored: String = conn
            .query_row("SELECT file_hash FROM sermon_index WHERE file_path='a.md'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, hash);
    }

    #[test]
    fn dry_run_makes_no_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        write_file(&vault, "a.md", &sermon_md("alpha", "Alpha", "body"));

        let report = plan_reconciliation_report(&vault, &db).unwrap();
        assert!(report.dry_run);
        assert_eq!(report.new_files, 1);

        let conn = open_pastor_db(&db).unwrap();
        assert!(indexed_paths(&conn).is_empty(), "dry run must not write");
    }

    #[test]
    fn file_state_matrix() {
        let base = sermon_md("a", "A", "original");
        let base_hash = sermon::sha256_hex(base.as_bytes());
        let local = sermon_md("a", "A", "local edit");
        let disk = sermon_md("a", "A", "disk edit");

        assert_eq!(
            evaluate_file_state(Some(&base_hash), Some(&base), Some(&base)),
            FileState::Clean
        );
        assert_eq!(
            evaluate_file_state(Some(&base_hash), Some(&local), Some(&base)),
            FileState::DirtyLocal
        );
        assert_eq!(
            evaluate_file_state(Some(&base_hash), Some(&base), Some(&disk)),
            FileState::DiskChanged
        );
        assert_eq!(
            evaluate_file_state(Some(&base_hash), Some(&local), Some(&disk)),
            FileState::Conflict
        );
        // Converged: disk was saved from this very buffer.
        assert_eq!(
            evaluate_file_state(Some(&base_hash), Some(&local), Some(&local)),
            FileState::Clean
        );
        // Missing file with local work: still Missing (Save Local As path).
        assert_eq!(
            evaluate_file_state(Some(&base_hash), Some(&local), None),
            FileState::Missing
        );
        // No baseline at all (new untitled sermon).
        assert_eq!(evaluate_file_state(None, Some(&local), None), FileState::DirtyLocal);
        assert_eq!(
            evaluate_file_state(None, Some(&local), Some(&disk)),
            FileState::Conflict
        );
    }

    #[test]
    fn inspect_file_state_reports_conflict_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let base = sermon_md("a", "A", "original");
        write_file(&vault, "a.md", &base);
        let baseline_hash = sermon::sha256_hex(base.as_bytes());

        let local = sermon_md("a", "A", "local edit");
        let disk = sermon_md("a", "A", "disk edit");
        write_file(&vault, "a.md", &disk);

        let report =
            inspect_file_state(&vault, "a.md", Some(&baseline_hash), Some(&local)).unwrap();
        assert_eq!(report.state, FileState::Conflict);
        assert_eq!(report.baseline_hash.as_deref(), Some(baseline_hash.as_str()));
        assert_eq!(
            report.local_hash.as_deref(),
            Some(sermon::sha256_hex(local.as_bytes()).as_str())
        );
        assert_eq!(
            report.disk_hash.as_deref(),
            Some(sermon::sha256_hex(disk.as_bytes()).as_str())
        );
        assert!(report.has_local_changes);
    }

    #[test]
    fn missing_vault_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let missing_vault = tmp.path().join("nope");
        assert!(matches!(
            reconcile(&missing_vault, &tmp.path().join("p.db")),
            Err(CoreError::VaultMissing(_))
        ));
    }
}
