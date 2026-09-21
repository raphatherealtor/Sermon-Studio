//! Backend-facing API used by the Tauri command layer.
//!
//! Everything here operates on plain paths, connections, and session state —
//! no `tauri::State` — so the whole surface is unit-testable without a Tauri
//! runtime. Each function is a thin composition of Wave 1 core capabilities
//! (Track A reference resolution, Track B canonical AST, Track C
//! reconciliation/atomic save/indexing); none of their logic is reimplemented.

use crate::config::AppConfig;
use crate::dto;
use crate::dto::*;
use rusqlite::{params, Connection, OptionalExtension};
use sermon_core::{export, indexer, linter, reconcile, reference, retrieval, sermon};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub type ApiResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// Session baselines: what the editor loaded/saved this session, for conflict
// detection (Track C evaluates editor-vs-disk state from content hashes).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Baseline {
    pub file_path: String,
    /// Last-synced content (what the session anchored against disk: load-time
    /// bytes or the last successful save).
    pub content: String,
    /// SHA-256 of `content` — the session's disk anchor for conflict checks.
    pub hash: String,
    /// The editor buffer the session last presented for this sermon (set when
    /// a save is rejected as a conflict, cleared on a successful sync). The
    /// IPC status command only receives a sermon id, so the backend can only
    /// classify `local-dirty`/`both-changed` by remembering what the editor
    /// tried to save.
    pub buffer: Option<String>,
}

impl Baseline {
    pub fn new(file_path: String, content: String) -> Self {
        let hash = sermon::sha256_hex(content.as_bytes());
        Baseline {
            file_path,
            content,
            hash,
            buffer: None,
        }
    }

    /// The local side of editor-vs-disk comparisons: the pending editor
    /// buffer when one is known, else the last-synced content.
    pub fn local_content(&self) -> &str {
        self.buffer.as_deref().unwrap_or(&self.content)
    }
}

#[derive(Debug, Default)]
pub struct SessionBaselines {
    map: HashMap<String, Baseline>,
}

impl SessionBaselines {
    pub fn get(&self, sermon_id: &str) -> Option<&Baseline> {
        self.map.get(sermon_id)
    }

    pub fn record(&mut self, sermon_id: &str, baseline: Baseline) {
        self.map.insert(sermon_id.to_string(), baseline);
    }

    /// Remember the buffer a conflicted save tried to write, keeping the
    /// load-time anchor intact, so status/diff/keep-local see the editor's
    /// real content (the IPC contract never sends buffers to status calls).
    pub fn record_buffer(&mut self, sermon_id: &str, buffer: String) {
        match self.map.get_mut(sermon_id) {
            Some(b) => b.buffer = Some(buffer),
            None => {
                let mut b = Baseline::new(String::new(), buffer.clone());
                b.buffer = Some(buffer);
                self.map.insert(sermon_id.to_string(), b);
            }
        }
    }

    pub fn remove(&mut self, sermon_id: &str) {
        self.map.remove(sermon_id);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

// ---------------------------------------------------------------------------
// Settings mapping
// ---------------------------------------------------------------------------

pub fn settings_from_config(cfg: &AppConfig) -> AppSettingsDto {
    AppSettingsDto {
        library_path: cfg.vault_path.clone(),
        librarian_enabled: cfg.librarian_enabled,
        default_translation: "KJV".to_string(),
        autosave_interval_seconds: 30,
        editor_font_size: cfg.font_size,
        editor_font: String::new(),
        spellcheck: true,
        focus_mode: false,
        export_defaults: serde_json::Value::Object(Default::default()),
        keyboard_shortcuts: Default::default(),
        developer_mode: false,
    }
}

pub fn settings_to_config(dto: &AppSettingsDto, current: &AppConfig) -> AppConfig {
    AppConfig {
        vault_path: dto.library_path.clone(),
        canon_path: current.canon_path.clone(),
        pastor_path: current.pastor_path.clone(),
        librarian_enabled: dto.librarian_enabled,
        font_size: dto.editor_font_size,
        high_contrast: current.high_contrast,
    }
}

// ---------------------------------------------------------------------------
// Sermon CRUD
// ---------------------------------------------------------------------------

fn blocks_markdown(s: &sermon::Sermon) -> String {
    s.blocks
        .iter()
        .map(|b| b.to_markdown())
        .collect::<Vec<_>>()
        .join("")
}

/// Refuse to persist a body that is actually an HTML serialization (the
/// editor transport contract declares `body` as TipTap HTML). The canonical
/// sermon on disk is Markdown; writing HTML would silently strip frontmatter
/// and directives and churn the sermon identity on the next parse.
///
/// The discriminator is block-level HTML at a line start (`<p>`, `<div>`,
/// `<h1>` …) — TipTap's `getHTML()` always emits those, while canonical
/// sermon markdown never begins a line with a block tag. Inline HTML inside
/// prose is legal Markdown and passes through.
fn ensure_canonical_markdown(body: &str) -> ApiResult<()> {
    const BLOCK_TAGS: [&str; 15] = [
        "<p>", "<p ", "<div", "<h1", "<h2", "<h3", "<h4", "<h5", "<h6", "<ul", "<ol", "<li",
        "<blockquote", "<table", "<pre",
    ];
    for line in body.lines() {
        let trimmed = line.trim_start();
        if BLOCK_TAGS.iter().any(|tag| trimmed.starts_with(tag)) {
            return Err(
                "refusing to persist non-markdown body: the editor transport sent \
                 HTML, but the canonical sermon file is Markdown. No file was \
                 modified. Re-save from a markdown-serialized editor buffer."
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn word_count(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Map canonical Markdown source to the frontend `SermonDocument` transport.
/// Markdown remains canonical; `body` carries the manuscript Markdown and the
/// outline is derived from the AST's verbatim Markdown blocks.
pub fn document_from_raw(raw: &str, rel_path: &str) -> ApiResult<SermonDocumentDto> {
    let s = sermon::Sermon::parse(raw).map_err(err)?;
    let body = blocks_markdown(&s);
    let directives = directive_entries(&s);
    Ok(SermonDocumentDto {
        id: s
            .meta
            .id
            .clone()
            .unwrap_or_else(|| sermon::slugify(&s.meta.title.clone().unwrap_or_default())),
        title: s.meta.title.clone().unwrap_or_else(|| "(untitled)".to_string()),
        subtitle: None,
        scripture: s.meta.primary_passage.clone().unwrap_or_default(),
        series: s.meta.series.clone(),
        status: "draft".to_string(),
        body,
        outline: dto::outline_from_markdown(&blocks_markdown(&s)),
        tags: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
        preached_on: s.meta.date_preached.clone(),
        version: 1,
        directives,
        source_path: Some(rel_path.to_string()),
        fs_state: None,
    })
}

fn directive_entries(s: &sermon::Sermon) -> Vec<DirectiveEntryDto> {
    let mut out = Vec::new();
    for m in s.movements() {
        out.push(DirectiveEntryDto {
            key: "movement".to_string(),
            value: m.title.clone().unwrap_or_default(),
        });
    }
    for i in s.illustrations() {
        out.push(DirectiveEntryDto {
            key: "illustration".to_string(),
            value: i.title.clone().unwrap_or_default(),
        });
    }
    for a in s.applications() {
        out.push(DirectiveEntryDto {
            key: "application".to_string(),
            value: a
                .body
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
        });
    }
    for n in s.exegetical_notes() {
        out.push(DirectiveEntryDto {
            key: "exegetical-notes".to_string(),
            value: n.body.lines().next().unwrap_or_default().chars().take(80).collect(),
        });
    }
    out
}

fn summary_from_hit(hit: &retrieval::SermonHit) -> SermonSummaryDto {
    SermonSummaryDto {
        id: hit.id.clone(),
        title: hit.title.clone(),
        scripture: hit.primary_passage.clone(),
        series: hit.series.clone(),
        status: "draft".to_string(),
        word_count: 0,
        created_at: String::new(),
        updated_at: String::new(),
        preached_on: hit.date_preached.clone(),
        tags: Vec::new(),
        source_path: Some(hit.file_path.clone()),
        fs_state: None,
    }
}

pub fn list_sermons(conn: &Connection) -> ApiResult<Vec<SermonSummaryDto>> {
    let hits = retrieval::list_sermons(conn).map_err(err)?;
    Ok(hits.iter().map(summary_from_hit).collect())
}

pub fn search_sermons(
    conn: &Connection,
    query: &str,
    filters: &SearchFiltersDto,
) -> ApiResult<Vec<SearchResultDto>> {
    let hits = retrieval::search_sermons(conn, query, 100).map_err(err)?;
    let mut out = Vec::new();
    for h in hits {
        if let Some(series) = &filters.series {
            if h.series.as_deref() != Some(series.as_str()) {
                continue;
            }
        }
        if let Some(book) = &filters.scripture_book {
            let needle = book.to_lowercase();
            if !h.primary_passage.to_lowercase().starts_with(&needle) {
                continue;
            }
        }
        out.push(SearchResultDto {
            id: h.id,
            title: h.title,
            scripture: h.primary_passage,
            snippet: h.snippet,
            score: h.score,
        });
    }
    Ok(out)
}

/// Resolve a sermon id to its vault-relative path via the derived index,
/// falling back to the missing-sermon bookkeeping (Track C evidence).
pub fn find_sermon_path(conn: &Connection, sermon_id: &str) -> ApiResult<Option<String>> {
    let by_id: Option<String> = conn
        .query_row(
            "SELECT file_path FROM sermon_index WHERE id = ?1",
            params![sermon_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    if by_id.is_some() {
        return Ok(by_id);
    }
    // The index reflects disk reality; a missing file has no row. Check the
    // recorded evidence so the UI can still address the sermon.
    let missing = reconcile::missing_sermons(conn).map_err(err)?;
    for (path, rec) in &missing {
        if rec.sermon_id.as_deref() == Some(sermon_id) {
            return Ok(Some(path.clone()));
        }
    }
    Ok(None)
}

fn unique_filename(vault: &Path, slug: &str) -> String {
    let mut candidate = format!("{slug}.md");
    let mut n = 2u32;
    while sermon_core::atomic_save::vault_file_exists(vault, &candidate) {
        candidate = format!("{slug}-{n}.md");
        n += 1;
    }
    candidate
}

pub struct CreateSermonReq {
    pub title: Option<String>,
    pub scripture: Option<String>,
    pub series: Option<String>,
}

pub fn create_sermon(
    vault: &Path,
    conn: &mut Connection,
    req: &CreateSermonReq,
) -> ApiResult<(SermonDocumentDto, Baseline)> {
    let title = req
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Untitled Sermon".to_string());
    let scripture = req.scripture.clone().unwrap_or_default();
    let mut content = sermon::new_sermon_template(&title, &scripture);
    if let Some(series) = &req.series {
        content = content.replacen("series: \n", &format!("series: \"{series}\"\n"), 1);
    }
    let slug = sermon::slugify(&title);
    let filename = unique_filename(vault, if slug.is_empty() { "untitled" } else { &slug });
    sermon_core::atomic_save::save_sermon_atomic(vault, &filename, &content).map_err(err)?;
    let doc = sermon::SermonDoc::parse(&content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(conn, &filename, &doc, &mut stats).map_err(err)?;
    let dto = document_from_raw(&content, &filename)?;
    let baseline = Baseline::new(filename.clone(), content);
    Ok((dto, baseline))
}

pub fn load_sermon(vault: &Path, rel_path: &str) -> ApiResult<(SermonDocumentDto, Baseline)> {
    let raw = sermon_core::atomic_save::read_vault_file(vault, rel_path).map_err(err)?;
    let dto = document_from_raw(&raw, rel_path)?;
    let baseline = Baseline::new(rel_path.to_string(), raw);
    Ok((dto, baseline))
}

/// Conflict-safe save. Never auto-overwrites content the editor has not seen:
/// if the disk file differs from both the incoming buffer and the session
/// baseline, a conflict result is returned and the file is left untouched.
///
/// Transport contract: `SermonDocument.body` is the canonical **Markdown**
/// body (prose + `:::` directive fences). Frontmatter and sermon identity are
/// owned by the backend: the body is spliced into the splice source — current
/// disk raw, else the session baseline content, else the body alone for a
/// never-existed file — via [`replace_document_body`], so a prose-only edit
/// can never strip frontmatter or directives. [`ensure_canonical_markdown`]
/// refuses block-level HTML before any byte reaches the disk.
pub fn save_sermon(
    vault: &Path,
    conn: &mut Connection,
    baselines: &mut SessionBaselines,
    doc: &SermonDocumentDto,
) -> ApiResult<SaveResultDto> {
    let rel = doc
        .source_path
        .clone()
        .ok_or_else(|| "save_sermon: document has no sourcePath".to_string())?;
    let body = doc.body.clone();
    ensure_canonical_markdown(&body)?;
    let disk = sermon_core::atomic_save::read_vault_file(vault, &rel).ok();
    // Splice the edited body into the splice source so frontmatter and any
    // directives outside the edited region survive the write. The splice
    // source is the current disk content when present (already validated
    // below against the session baseline), else the session baseline raw,
    // else the body alone for a file that never existed.
    let splice_source = disk
        .clone()
        .or_else(|| baselines.get(doc.id.as_str()).map(|b| b.content.clone()));
    let new_content = splice_source
        .as_deref()
        .map(|raw| replace_document_body(raw, &body))
        .unwrap_or_else(|| body.clone());
    let baseline_hash = baselines
        .get(doc.id.as_str())
        .map(|b| b.hash.clone());

    if let Some(disk_content) = &disk {
        let disk_hash = sermon::sha256_hex(disk_content.as_bytes());
        let editor_saw_disk = baseline_hash.as_deref() == Some(disk_hash.as_str());
        if disk_content != &new_content && !editor_saw_disk {
            // Remember the rejected buffer (body spliced into the disk raw, so
            // it carries frontmatter like a real document) so
            // get_filesystem_status can classify local-dirty / both-changed
            // and Keep Local / Merge / Save Local As operate on the editor's
            // real content.
            baselines.record_buffer(&doc.id, new_content.clone());
            return Ok(SaveResultDto {
                success: false,
                saved_at: now_rfc3339(),
                version: doc.version,
                conflict: Some(ConflictInfoDto {
                    local_title: doc.title.clone(),
                    local_modified_at: now_rfc3339(),
                    disk_modified_at: sermon_core::atomic_save::vault_file_modified_at(
                        vault, &rel,
                    )
                    .unwrap_or_default(),
                    disk_version: 0,
                    disk_word_count: word_count(disk_content),
                    source_path: rel.clone(),
                    explanation: "The file changed on disk since this sermon was loaded. \
                                  Choose Keep Local, Use Disk, Merge, or Save Local As."
                        .to_string(),
                }),
            });
        }
    }

    sermon_core::atomic_save::save_sermon_atomic(vault, &rel, &new_content).map_err(err)?;
    let parsed = sermon::SermonDoc::parse(&new_content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(conn, &rel, &parsed, &mut stats).map_err(err)?;
    baselines.record(&doc.id, Baseline::new(rel, new_content));
    Ok(SaveResultDto {
        success: true,
        saved_at: now_rfc3339(),
        version: doc.version,
        conflict: None,
    })
}

/// YAML double-quoted scalar for a frontmatter value (no serde_yaml dep).
fn yaml_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    format!("\"{out}\"")
}

/// Split raw into (frontmatter, body, had_frontmatter), mirroring
/// `sermon_core::sermon::split_frontmatter` semantics.
fn split_fm_local(raw: &str) -> (&str, &str, bool) {
    if !raw.starts_with("---") {
        return ("", raw, false);
    }
    let Some(i) = raw.find('\n') else {
        return ("", "", false);
    };
    let after_first = &raw[i + 1..];
    let mut offset = 0usize;
    for line in after_first.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content.trim() == "---" {
            return (
                &after_first[..offset],
                &after_first[offset + line.len()..],
                true,
            );
        }
        offset += line.len();
    }
    ("", raw, false)
}

/// Replace (or insert) a single top-level `key:` line in the YAML frontmatter,
/// preserving every other byte of the file — including fields the typed
/// `Frontmatter` struct does not model and the verbatim body source.
fn replace_fm_field(raw: &str, key: &str, value: &str) -> ApiResult<String> {
    let (fm, body, had_fm) = split_fm_local(raw);
    let replacement = format!("{key}: {}", yaml_quote(value));
    let mut out_fm = String::with_capacity(fm.len() + replacement.len() + 8);
    let mut replaced = false;
    for chunk in fm.split_inclusive('\n') {
        let content = chunk.trim_end_matches(['\n', '\r']);
        let is_target = content.len() > key.len()
            && content.starts_with(key)
            && content.as_bytes()[key.len()] == b':'
            && !content.starts_with(|c: char| c == ' ' || c == '\t');
        if is_target {
            out_fm.push_str(&replacement);
            replaced = true;
        } else {
            out_fm.push_str(content);
        }
        out_fm.push('\n');
    }
    if !replaced {
        out_fm.push_str(&replacement);
        out_fm.push('\n');
    }
    let mut out = String::with_capacity(raw.len() + out_fm.len() + 8);
    if had_fm {
        out.push_str("---\n");
        out.push_str(&out_fm);
        out.push_str("---\n");
        out.push_str(body);
    } else {
        out.push_str("---\n");
        out.push_str(&out_fm);
        out.push_str("---\n\n");
        out.push_str(raw);
    }
    Ok(out)
}

pub fn rename_sermon(
    vault: &Path,
    conn: &mut Connection,
    sermon_id: &str,
    new_title: &str,
) -> ApiResult<SermonSummaryDto> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let raw = sermon_core::atomic_save::read_vault_file(vault, &rel).map_err(err)?;
    let meta = sermon::Sermon::parse(&raw).map_err(err)?.meta;
    let content = replace_fm_field(&raw, "title", new_title)?;
    sermon_core::atomic_save::save_sermon_atomic(vault, &rel, &content).map_err(err)?;
    let doc = sermon::SermonDoc::parse(&content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(conn, &rel, &doc, &mut stats).map_err(err)?;
    Ok(summary_from_hit(&retrieval::SermonHit {
        id: doc.resolved_id(),
        title: new_title.to_string(),
        primary_passage: doc.primary_passage(),
        big_idea: doc.big_idea(),
        date_preached: meta.date_preached.clone(),
        series: meta.series.clone(),
        liturgical_season: meta.liturgical_season.clone(),
        structure_type: doc.structure_type(),
        file_path: rel,
        snippet: String::new(),
        score: 0.0,
    }))
}

pub fn duplicate_sermon(
    vault: &Path,
    conn: &mut Connection,
    sermon_id: &str,
    new_title: Option<&str>,
) -> ApiResult<(SermonDocumentDto, Baseline)> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let raw = sermon_core::atomic_save::read_vault_file(vault, &rel).map_err(err)?;
    let meta = sermon::Sermon::parse(&raw).map_err(err)?.meta;
    let title = new_title
        .map(|t| t.to_string())
        .or_else(|| meta.title.as_ref().map(|t| format!("{t} (copy)")))
        .unwrap_or_else(|| "Untitled Sermon (copy)".to_string());
    // New identity for the copy: the duplicate is a different sermon.
    let new_id = format!("{}-{}", sermon::slugify(&title), std::process::id());
    let content = replace_fm_field(&raw, "title", &title)?;
    let content = replace_fm_field(&content, "id", &new_id)?;
    let slug = sermon::slugify(&title);
    let filename = unique_filename(vault, if slug.is_empty() { "untitled" } else { &slug });
    sermon_core::atomic_save::save_sermon_atomic(vault, &filename, &content).map_err(err)?;
    let doc = sermon::SermonDoc::parse(&content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(conn, &filename, &doc, &mut stats).map_err(err)?;
    let dto = document_from_raw(&content, &filename)?;
    let baseline = Baseline::new(filename, content);
    Ok((dto, baseline))
}

/// Archive = move the canonical file into the vault's `archive/` subfolder.
/// Reconciliation repairs the derived index via UUID identity (a rename).
pub fn archive_sermon(vault: &Path, db_path: &Path, sermon_id: &str) -> ApiResult<()> {
    let conn = indexer::open_pastor_db(db_path).map_err(err)?;
    let rel = find_sermon_path(&conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    drop(conn);
    let file_name = Path::new(&rel)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| format!("invalid sermon path: {rel}"))?;
    let dir_prefix = Path::new(&rel)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_default();
    let archive_dir = if dir_prefix.is_empty() {
        "archive".to_string()
    } else {
        format!("{dir_prefix}/archive")
    };
    let mut target = format!("{archive_dir}/{file_name}");
    let mut n = 2u32;
    while sermon_core::atomic_save::vault_file_exists(vault, &target) {
        let stem = file_name.trim_end_matches(".md");
        target = format!("{archive_dir}/{stem}-{n}.md");
        n += 1;
    }
    let src = sermon_core::atomic_save::safe_join(vault, &rel).map_err(err)?;
    let dst = sermon_core::atomic_save::safe_join(vault, &target).map_err(err)?;
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::rename(&src, &dst).map_err(err)?;
    reconcile::reconcile_paths(vault, db_path, &[rel, target]).map_err(err)?;
    Ok(())
}

/// Delete removes the canonical file and its derived rows (intentional
/// deletion — no missing-sermon record is created).
pub fn delete_sermon(vault: &Path, conn: &Connection, sermon_id: &str) -> ApiResult<()> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let full = sermon_core::atomic_save::safe_join(vault, &rel).map_err(err)?;
    if full.exists() {
        std::fs::remove_file(&full).map_err(err)?;
    }
    // Drop derived rows (mirrors reconcile's removal; FTS is self-contained).
    let row: Option<String> = conn
        .query_row(
            "SELECT id FROM sermon_index WHERE file_path = ?1",
            params![rel],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    if let Some(id) = &row {
        conn.execute("DELETE FROM sermons_fts WHERE sermon_id = ?1", params![id])
            .map_err(err)?;
    }
    conn.execute("DELETE FROM sermon_index WHERE file_path = ?1", params![rel])
        .map_err(err)?;
    // Clear any stale missing-sermon bookkeeping for the deleted path.
    let mut missing = reconcile::missing_sermons(conn).map_err(err)?;
    if missing.remove(&rel).is_some() {
        conn.execute(
            "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('missing_sermons', ?1)",
            params![serde_json::to_string(&missing).map_err(err)?],
        )
        .map_err(err)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Conflict resolution / filesystem status
// ---------------------------------------------------------------------------

pub fn get_filesystem_status(
    vault: &Path,
    conn: &Connection,
    baselines: &SessionBaselines,
    sermon_id: &str,
) -> ApiResult<FilesystemReconciliationStatusDto> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let baseline = baselines.get(sermon_id);
    let local_content = baseline.map(|b| b.local_content());
    let baseline_hash = baseline.map(|b| b.hash.as_str());
    let disk_content = sermon_core::atomic_save::read_vault_file(vault, &rel).ok();
    let state = reconcile::evaluate_file_state(
        baseline_hash,
        local_content,
        disk_content.as_deref(),
    );
    let mut state_str = dto::file_state_to_ts(state).to_string();

    // Track C evidence overrides for the richer Rocket states.
    let mut duplicate_paths: Option<Vec<String>> = None;
    let mut recovery_path: Option<String> = None;
    if disk_content.is_none() {
        let missing = reconcile::missing_sermons(conn).map_err(err)?;
        if missing.contains_key(&rel) {
            state_str = "recovery-available".to_string();
            recovery_path = Some(rel.clone());
        }
    }
    for c in reconcile::id_collisions(conn).map_err(err)? {
        if c.losers.contains(&rel) {
            state_str = "duplicate-detected".to_string();
            duplicate_paths = Some(c.losers.clone());
        } else if c.winner == rel {
            duplicate_paths = Some(c.losers.clone());
        }
    }

    Ok(FilesystemReconciliationStatusDto {
        sermon_id: sermon_id.to_string(),
        state: state_str,
        source_path: rel.clone(),
        local_modified_at: None,
        disk_modified_at: sermon_core::atomic_save::vault_file_modified_at(vault, &rel),
        renamed_to: None,
        duplicate_paths,
        recovery_path,
    })
}

pub fn prepare_diff(
    vault: &Path,
    conn: &Connection,
    baselines: &SessionBaselines,
    sermon_id: &str,
) -> ApiResult<DiffPreparationResultDto> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let local = baselines
        .get(sermon_id)
        .map(|b| b.local_content().to_string())
        .ok_or_else(|| "no local buffer for this sermon in this session".to_string())?;
    let disk = sermon_core::atomic_save::read_vault_file(vault, &rel).map_err(err)?;
    let local_lines: Vec<String> = local.lines().map(|s| s.to_string()).collect();
    let disk_lines: Vec<String> = disk.lines().map(|s| s.to_string()).collect();
    let diff = dto::diff_lines(&local_lines, &disk_lines);
    let hunks = dto::hunks_from_diff(&diff, 3);
    Ok(DiffPreparationResultDto {
        local_lines,
        disk_lines,
        hunks,
    })
}

pub fn prepare_merge(
    vault: &Path,
    conn: &Connection,
    baselines: &SessionBaselines,
    sermon_id: &str,
) -> ApiResult<MergePreparationResultDto> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let local = baselines
        .get(sermon_id)
        .map(|b| b.local_content().to_string())
        .unwrap_or_default();
    let disk = sermon_core::atomic_save::read_vault_file(vault, &rel).map_err(err)?;
    Ok(MergePreparationResultDto {
        base: local.clone(),
        local,
        disk,
        conflicts: Vec::new(),
    })
}

pub fn resolve_conflict(
    vault: &Path,
    conn: &mut Connection,
    baselines: &mut SessionBaselines,
    req: &ConflictResolutionDto,
) -> ApiResult<SaveResultDto> {
    let content: String;
    let mut target_rel: Option<String> = None;
    match req.strategy.as_str() {
        "keep-local" => {
            content = baselines
                .get(&req.sermon_id)
                .map(|b| b.local_content().to_string())
                .ok_or_else(|| {
                    "Keep Local unavailable: the local buffer for this sermon \
                     is not known in this session. Reload the sermon first."
                        .to_string()
                })?;
        }
        "use-disk" => {
            // No write: the frontend reloads from disk. Drop any pending local
            // buffer and re-anchor the session baseline at the current disk
            // bytes, so the next save or status check does not resurrect the
            // conflict the user just resolved.
            if let Some(rel) = find_sermon_path(conn, &req.sermon_id)? {
                match sermon_core::atomic_save::read_vault_file(vault, &rel) {
                    Ok(disk) => baselines.record(&req.sermon_id, Baseline::new(rel, disk)),
                    Err(_) => baselines.remove(&req.sermon_id),
                }
            }
            return Ok(SaveResultDto {
                success: true,
                saved_at: now_rfc3339(),
                version: 1,
                conflict: None,
            });
        }
        "merge" => {
            content = req
                .merged_body
                .clone()
                .ok_or_else(|| "merge strategy requires mergedBody".to_string())?;
        }
        "save-local-as" => {
            content = baselines
                .get(&req.sermon_id)
                .map(|b| b.local_content().to_string())
                .ok_or_else(|| {
                    "Save Local As unavailable: the local buffer for this sermon \
                     is not known in this session. Reload the sermon first."
                        .to_string()
                })?;
            target_rel = Some(
                req.save_as_path
                    .clone()
                    .ok_or_else(|| "save-local-as requires saveAsPath".to_string())?,
            );
        }
        other => return Err(format!("unknown conflict resolution strategy: {other}")),
    }

    let rel = match target_rel {
        Some(t) => t,
        None => find_sermon_path(conn, &req.sermon_id)?
            .ok_or_else(|| format!("sermon not found: {}", req.sermon_id))?,
    };
    ensure_canonical_markdown(&content)?;
    // The local buffer and merged body are body-only Markdown; splice into
    // the on-disk raw (else the session baseline raw) so frontmatter and
    // directives outside the edited region survive the resolution write. A
    // buffer that already carries its own frontmatter fence (Keep Local /
    // Save Local As after a conflicted save) is a full document and is
    // persisted verbatim — re-splicing it would duplicate the fence.
    let new_content = if split_fm_local(&content).2 {
        content.clone()
    } else {
        let splice_source = sermon_core::atomic_save::read_vault_file(vault, &rel)
            .ok()
            .or_else(|| baselines.get(&req.sermon_id).map(|b| b.content.clone()));
        splice_source
            .as_deref()
            .map(|raw| replace_document_body(raw, &content))
            .unwrap_or_else(|| content.clone())
    };
    sermon_core::atomic_save::save_sermon_atomic(vault, &rel, &new_content).map_err(err)?;
    let parsed = sermon::SermonDoc::parse(&new_content).map_err(err)?;
    let mut stats = indexer::IndexStats::default();
    indexer::index_single(conn, &rel, &parsed, &mut stats).map_err(err)?;
    baselines.record(&req.sermon_id, Baseline::new(rel, new_content));
    Ok(SaveResultDto {
        success: true,
        saved_at: now_rfc3339(),
        version: 1,
        conflict: None,
    })
}

pub fn recover_sermon(
    vault: &Path,
    db_path: &Path,
    conn: &Connection,
    sermon_id: &str,
) -> ApiResult<RecoveryResultDto> {
    let Some(rel) = find_sermon_path(conn, sermon_id)? else {
        return Ok(RecoveryResultDto {
            success: false,
            recovered_path: String::new(),
            message: "No record of this sermon exists (neither on disk nor in \
                      the missing-sermon ledger)."
                .to_string(),
        });
    };
    if sermon_core::atomic_save::vault_file_exists(vault, &rel) {
        reconcile::reconcile_paths(vault, db_path, &[rel.clone()]).map_err(err)?;
        Ok(RecoveryResultDto {
            success: true,
            recovered_path: rel.clone(),
            message: "The canonical file is present again; the derived index \
                      has been reconnected."
                .to_string(),
        })
    } else {
        Ok(RecoveryResultDto {
            success: false,
            recovered_path: String::new(),
            message: "The file is gone and only its content hash was recorded — \
                      there is no copy to recover from. Restore the file to \
                      the vault and recover again."
                .to_string(),
        })
    }
}

pub fn reconnect_sermon(
    vault: &Path,
    db_path: &Path,
    conn: &mut Connection,
    sermon_id: &str,
    new_path: &str,
) -> ApiResult<SermonSummaryDto> {
    if !sermon_core::atomic_save::vault_file_exists(vault, new_path) {
        return Err(format!("no sermon file at: {new_path}"));
    }
    reconcile::reconcile_paths(vault, db_path, &[new_path.to_string()]).map_err(err)?;
    let hits = retrieval::list_sermons(conn).map_err(err)?;
    let hit = hits
        .into_iter()
        .find(|h| h.id == sermon_id || h.file_path == new_path)
        .ok_or_else(|| format!("reconnect failed to produce an index row for {sermon_id}"))?;
    Ok(summary_from_hit(&hit))
}

// ---------------------------------------------------------------------------
// Index operations
// ---------------------------------------------------------------------------

fn index_op_result(stats: &indexer::IndexStats, verb: &str) -> IndexOperationResultDto {
    IndexOperationResultDto {
        success: true,
        message: format!(
            "{verb}: {} indexed, {} skipped unchanged, {} removed",
            stats.indexed, stats.skipped_unchanged, stats.removed
        ),
        documents_indexed: stats.indexed as u32,
        duration_ms: stats.elapsed_ms as u64,
        errors: Vec::new(),
    }
}

pub fn sync_index(vault: &Path, db_path: &Path) -> ApiResult<IndexOperationResultDto> {
    let stats = indexer::sync(vault, db_path).map_err(err)?;
    Ok(index_op_result(&stats, "sync"))
}

pub fn rebuild_index(vault: &Path, db_path: &Path) -> ApiResult<IndexOperationResultDto> {
    let stats = indexer::rebuild(vault, db_path).map_err(err)?;
    Ok(index_op_result(&stats, "rebuild"))
}

/// Hash-driven rescan = Track C reconciliation (the correctness mechanism).
pub fn rescan_library(vault: &Path, db_path: &Path) -> ApiResult<IndexOperationResultDto> {
    let report = reconcile::reconcile(vault, db_path).map_err(err)?;
    Ok(IndexOperationResultDto {
        success: true,
        message: format!(
            "rescan: {} scanned, {} reindexed, {} removed missing",
            report.scanned, report.reindexed, report.removed_missing
        ),
        documents_indexed: report.reindexed as u32,
        duration_ms: report.elapsed_ms as u64,
        errors: report.read_errors,
    })
}

/// Repair = startup reconciliation: rebuilds derived state from disk reality
/// without ever touching canonical files.
pub fn repair_index(vault: &Path, db_path: &Path) -> ApiResult<IndexOperationResultDto> {
    let report = reconcile::startup_reconcile(vault, db_path).map_err(err)?;
    Ok(IndexOperationResultDto {
        success: true,
        message: format!(
            "repair: {} scanned, {} reindexed, {} recovered, {} removed missing",
            report.scanned, report.reindexed, report.recovered, report.removed_missing
        ),
        documents_indexed: report.reindexed as u32,
        duration_ms: report.elapsed_ms as u64,
        errors: report.read_errors,
    })
}

pub fn index_status(db_path: &Path) -> ApiResult<IndexStatusDto> {
    let conn = indexer::open_pastor_db(db_path).map_err(err)?;
    let indexed_file_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM sermon_index", [], |r| r.get::<_, i64>(0))
        .map_err(err)? as u32;
    let meta = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM index_meta WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten()
    };
    Ok(IndexStatusDto {
        indexed_file_count,
        index_version: "1".to_string(),
        last_reconciliation_time: meta("last_reconcile"),
        last_full_scan_time: meta("last_rebuild"),
        status: "idle".to_string(),
        error_message: None,
    })
}

pub fn archive_stats(conn: &Connection) -> ApiResult<ArchiveStatsDto> {
    let stats = retrieval::archive_stats(conn).map_err(err)?;
    // The core stats do not track words; count them from the stored bodies.
    let total_words: i64 = {
        let mut stmt = conn
            .prepare("SELECT body_content FROM sermon_body")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?;
        let mut total = 0i64;
        for row in rows {
            total += row.map_err(err)?.split_whitespace().count() as i64;
        }
        total
    };
    Ok(ArchiveStatsDto {
        total_sermons: stats.sermon_count,
        total_series: stats.series_count,
        total_words,
        last_preached_on: None,
        oldest_sermon: None,
        newest_sermon: None,
        sermons_by_status: Default::default(),
        sermons_by_month: Vec::new(),
    })
}

pub fn illustration_fatigue(conn: &Connection) -> ApiResult<Vec<IllustrationFatigueDto>> {
    let rows = retrieval::illustration_fatigue(conn, 2).map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let severity = if r.total_uses >= 5 {
                "high"
            } else if r.total_uses >= 3 {
                "medium"
            } else {
                "low"
            };
            IllustrationFatigueDto {
                illustration: r.label,
                use_count: r.total_uses,
                last_used_in: String::new(),
                last_used_on: r.last_used.unwrap_or_default(),
                severity: severity.to_string(),
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Study rail
// ---------------------------------------------------------------------------

pub fn get_passage(conn: &Connection, reference: &str) -> ApiResult<PassageResultDto> {
    let passage = reference::parse_passage(reference).map_err(err)?;
    let rows = retrieval::get_passage(conn, passage.start, passage.end).map_err(err)?;
    let text = rows
        .iter()
        .map(|r| format!("{} {}:{} {}", r.book_name, r.chapter, r.verse, r.text_kjv))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(PassageResultDto {
        reference: reference.to_string(),
        text,
        translation: "KJV".to_string(),
        verses: rows
            .into_iter()
            .map(|r| VerseEntryDto {
                verse: r.verse,
                text: r.text_kjv,
            })
            .collect(),
        osis_ref: Some(passage.canonical()),
    })
}

pub fn get_strongs(conn: &Connection, id: &str) -> ApiResult<StrongsEntryDto> {
    let entry = retrieval::get_lexicon(conn, id)
        .map_err(err)?
        .ok_or_else(|| format!("Strong's entry not found: {id}"))?;
    let occurrences = retrieval::verses_for_strong(conn, id, 1_000_000)
        .map_err(err)?
        .len() as u32;
    Ok(StrongsEntryDto {
        id: entry.strong_id,
        lemma: entry.lemma,
        transliteration: entry.transliteration,
        definition: entry.definition,
        gloss: entry.gloss,
        part_of_speech: entry.part_of_speech.unwrap_or_default(),
        occurrences,
        usage_examples: Vec::new(),
        related_ids: Vec::new(),
    })
}

pub fn get_cross_references(
    conn: &Connection,
    reference: &str,
) -> ApiResult<Vec<CrossReferenceDto>> {
    let passage = reference::parse_passage(reference).map_err(err)?;
    let start = passage.start;
    let rows = retrieval::cross_references(conn, start.book_num, start.chapter, start.verse, 50)
        .map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|(v, rank)| CrossReferenceDto {
            reference: format!("{} {}:{}", v.book_name, v.chapter, v.verse),
            snippet: v.text_kjv,
            relevance: rank,
        })
        .collect())
}

pub fn get_preached_on(conn: &Connection, reference: &str) -> ApiResult<Vec<PreachedResultDto>> {
    let passage = reference::parse_passage(reference).map_err(err)?;
    let start = passage.start;
    let hits = retrieval::sermons_for_verse(conn, start.book_num, start.chapter, start.verse)
        .map_err(err)?;
    Ok(hits
        .into_iter()
        .map(|h| PreachedResultDto {
            sermon_id: h.id,
            sermon_title: h.title,
            preached_on: h.date_preached.unwrap_or_default(),
            series: h.series,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Codec test (frontend parity: semantic PASS/FAIL contract)
// ---------------------------------------------------------------------------

pub fn test_directive_codec(input: &str) -> CodecRoundTripResultDto {
    use sermon_core::directive;
    let parsed = directive::parse_directives(input);
    let serialized = directive::serialize_directives(&parsed);

    let mut pass = true;
    for d in &parsed {
        if !d.known {
            if !serialized.contains(&d.raw_source) {
                pass = false;
                break;
            }
        } else if !serialized.contains(&format!(":::{name}", name = d.name)) {
            pass = false;
            break;
        }
    }

    let parsed_dtos = parsed
        .iter()
        .map(|d| {
            let mut attributes: HashMap<String, String> = HashMap::new();
            for a in &d.attributes {
                attributes.insert(a.key.clone(), a.value.clone());
            }
            ParsedDirectiveDto {
                kind: if d.known { "known".into() } else { "unknown".into() },
                name: d.name.clone(),
                attributes,
                body: d.body.trim().to_string(),
                raw_source: d.raw_source.clone(),
            }
        })
        .collect();

    CodecRoundTripResultDto {
        pass,
        input: input.to_string(),
        parsed: parsed_dtos,
        serialized,
    }
}

// ---------------------------------------------------------------------------
// Structural linting (Track E business logic, Track F transport mapping)
// ---------------------------------------------------------------------------

fn replace_document_body(raw: &str, body: &str) -> String {
    let bom_len = if raw.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    if !raw[bom_len..].starts_with("---") {
        return body.to_string();
    }
    let Some(first_newline) = raw[bom_len..].find('\n').map(|i| bom_len + i) else {
        return body.to_string();
    };
    let mut offset = first_newline + 1;
    for line in raw[offset..].split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']).trim() == "---" {
            let body_start = offset + line.len();
            return format!("{}{}", &raw[..body_start], body);
        }
        offset += line.len();
    }
    body.to_string()
}

fn document_source(vault: &Path, doc: &SermonDocumentDto) -> String {
    doc.source_path
        .as_deref()
        .and_then(|path| sermon_core::atomic_save::read_vault_file(vault, path).ok())
        .map(|raw| replace_document_body(&raw, &doc.body))
        .unwrap_or_else(|| doc.body.clone())
}

fn lint_finding_dto(finding: linter::Finding, source: &str) -> LintFindingDto {
    let source_range = finding.span.map(|span| {
        let start = linter::line_col_at(source, span.start);
        let end = linter::line_col_at(source, span.end);
        SourceRangeDto {
            start_line: start.line,
            start_col: start.column,
            end_line: end.line,
            end_col: end.column,
        }
    });
    let block_id = finding.block.as_ref().map(|block| {
        block
            .order
            .map(|order| format!("{}-{order}", block.kind))
            .or_else(|| block.title.as_deref().map(sermon::slugify))
            .unwrap_or_else(|| block.kind.clone())
    });
    let location = source_range.as_ref().map(|range| {
        format!(
            "{}:{}-{}:{}",
            range.start_line, range.start_col, range.end_line, range.end_col
        )
    });
    let severity = match finding.severity {
        linter::Severity::Error => "error",
        linter::Severity::Warning => "warning",
    }
    .to_string();
    LintFindingDto {
        id: finding.id,
        severity,
        code: finding.rule_id.clone(),
        rule_id: finding.rule_id,
        message: finding.message,
        location,
        movement_id: finding
            .block
            .as_ref()
            .filter(|block| block.kind == "movement")
            .and(block_id.clone()),
        block_id,
        source_range,
    }
}

pub fn lint_document(
    vault: &Path,
    pastor: Option<&Connection>,
    doc: &SermonDocumentDto,
) -> ApiResult<Vec<LintFindingDto>> {
    let source = document_source(vault, doc);
    let parsed = sermon::Sermon::parse(&source).map_err(err)?;
    let sermon_id = if doc.id.trim().is_empty() {
        parsed
            .meta
            .id
            .clone()
            .unwrap_or_else(|| sermon::slugify(&doc.title))
    } else {
        doc.id.clone()
    };
    let mut findings = linter::lint_sermon(&parsed, &sermon_id);
    if let Some(conn) = pastor {
        let labels: Vec<String> = parsed
            .illustrations()
            .map(|illustration| {
                illustration
                    .title
                    .clone()
                    .or_else(|| illustration.id.clone())
                    .unwrap_or_else(|| {
                        illustration.body.lines().next().unwrap_or_default().to_string()
                    })
            })
            .collect();
        let mut fatigue = linter::lint_illustration_fatigue(
            conn,
            &sermon_id,
            parsed.meta.date_preached.as_deref(),
            &labels,
        )
        .map_err(err)?;
        findings.append(&mut fatigue);
    }
    Ok(findings
        .into_iter()
        .map(|finding| lint_finding_dto(finding, &doc.body))
        .collect())
}

// ---------------------------------------------------------------------------
// Immutable source snapshots + Track D PDF execution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ExportSourceSnapshot {
    pub snapshot_id: String,
    pub sermon_id: String,
    pub sermon_title: String,
    pub created_at: String,
    pub revision_hash: String,
    pub word_count: u32,
    pub raw_source: String,
}

static SNAPSHOT_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn new_snapshot_id(raw_source: &str) -> String {
    use std::sync::atomic::Ordering;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let counter = SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let identity = format!("{nonce}:{counter}:{raw_source}");
    format!("snap-{}", sermon::sha256_hex(identity.as_bytes()))
}

impl ExportSourceSnapshot {
    pub fn to_dto(&self) -> ExportSnapshotDto {
        ExportSnapshotDto {
            snapshot_id: self.snapshot_id.clone(),
            sermon_id: self.sermon_id.clone(),
            sermon_title: self.sermon_title.clone(),
            created_at: self.created_at.clone(),
            revision_hash: self.revision_hash.clone(),
            word_count: self.word_count,
            status: "ready".to_string(),
        }
    }
}

pub fn create_export_source_snapshot(
    vault: &Path,
    conn: &Connection,
    sermon_id: &str,
) -> ApiResult<ExportSourceSnapshot> {
    let rel = find_sermon_path(conn, sermon_id)?
        .ok_or_else(|| format!("sermon not found: {sermon_id}"))?;
    let raw_source = sermon_core::atomic_save::read_vault_file(vault, &rel).map_err(err)?;
    let parsed = sermon::Sermon::parse(&raw_source).map_err(err)?;
    Ok(ExportSourceSnapshot {
        snapshot_id: new_snapshot_id(&raw_source),
        sermon_id: sermon_id.to_string(),
        sermon_title: parsed
            .meta
            .title
            .clone()
            .unwrap_or_else(|| "(untitled)".to_string()),
        created_at: now_rfc3339(),
        revision_hash: sermon::sha256_hex(raw_source.as_bytes()),
        word_count: word_count(&raw_source),
        raw_source,
    })
}

fn core_export_request(request: &ExportRequestDto) -> ApiResult<export::ExportRequest> {
    let format = match request.format.as_str() {
        "pulpit_manuscript" => export::ExportFormat::PulpitManuscript,
        "church_bulletin" => export::ExportFormat::ChurchBulletin,
        other => return Err(format!("unsupported export format: {other}")),
    };
    let pulpit_mode = match request.manuscript_mode.as_deref() {
        Some(mode) => Some(
            export::PulpitMode::parse(mode)
                .ok_or_else(|| format!("unsupported pulpit manuscript mode: {mode}"))?,
        ),
        None => None,
    };
    Ok(export::ExportRequest {
        format,
        pulpit_mode,
        include_private_notes: request.options.include_notes.unwrap_or(false),
    })
}

fn export_format_name(format: export::ExportFormat) -> &'static str {
    match format {
        export::ExportFormat::PulpitManuscript => "pulpit_manuscript",
        export::ExportFormat::ChurchBulletin => "church_bulletin",
    }
}

fn export_output_path(
    vault: &Path,
    snapshot: &ExportSourceSnapshot,
    request: &ExportRequestDto,
) -> ApiResult<PathBuf> {
    if let Some(path) = request
        .options
        .output_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        let path = PathBuf::from(path);
        return Ok(if path.is_absolute() { path } else { vault.join(path) });
    }
    let filename = request
        .options
        .output_filename
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .map(|name| {
            Path::new(name)
                .file_name()
                .and_then(|part| part.to_str())
                .map(str::to_string)
                .ok_or_else(|| "invalid export output filename".to_string())
        })
        .transpose()?
        .unwrap_or_else(|| {
            format!(
                "{}-{}.pdf",
                sermon::slugify(&snapshot.sermon_title),
                request.format
            )
        });
    let filename = if filename.to_ascii_lowercase().ends_with(".pdf") {
        filename
    } else {
        format!("{filename}.pdf")
    };
    Ok(vault.join("exports").join(filename))
}

pub fn execute_export_snapshot(
    vault: &Path,
    snapshot: &ExportSourceSnapshot,
    request: &ExportRequestDto,
) -> ApiResult<ExportResultDto> {
    if request.sermon_id != snapshot.sermon_id {
        return Err("export request sermonId does not match immutable snapshot".to_string());
    }
    let parsed = sermon::Sermon::parse(&snapshot.raw_source).map_err(err)?;
    let core_request = core_export_request(request)?;
    let output_path = export_output_path(vault, snapshot, request)?;
    let outcome = export::export_sermon_with_id(
        &parsed,
        &snapshot.raw_source,
        core_request,
        &output_path,
        Some(snapshot.snapshot_id.clone()),
    );
    match (outcome.success, outcome.result, outcome.error) {
        (true, Some(result), _) => Ok(ExportResultDto {
            success: true,
            output_path: Some(result.output_path),
            message: "Export completed successfully".to_string(),
            format: export_format_name(result.format).to_string(),
            snapshot_id: Some(result.export_id),
            exported_at: Some(result.created_at),
            file_size_bytes: Some(result.file_size),
        }),
        (_, _, error) => Err(error.unwrap_or_else(|| "export failed".to_string())),
    }
}

// ---------------------------------------------------------------------------
// Time helper (RFC 3339, UTC)
// ---------------------------------------------------------------------------

pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_rfc3339(secs)
}

/// Minimal UTC RFC 3339 formatter (no external date dependency).
fn format_rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as u64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Howard Hinnant's civil-from-days algorithm.
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mth <= 2 { y + 1 } else { y };
    format!("{year:04}-{mth:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Hand-rolled temp dir (avoids adding a dev-dependency; cleaned on Drop).
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!(
                "sermon-studio-f-test-{}-{}-{}",
                tag,
                std::process::id(),
                n
            ));
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }

        pub fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub fn sermon_md(id: &str, title: &str, body: &str) -> String {
        format!(
            "---\nid: {id}\ntitle: \"{title}\"\nprimary_passage: \"John.3.16\"\nbig_idea: \"idea\"\nstructure_type: verse_by_verse\n---\n\n{body}\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use std::path::PathBuf;

    fn setup(tag: &str) -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new(tag);
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        (tmp, vault, db)
    }

    fn seed(vault: &Path, db: &Path) -> (rusqlite::Connection, String) {
        let content = sermon_md("alpha", "Alpha", "God so loved the world.");
        sermon_core::atomic_save::save_sermon_atomic(vault, "alpha.md", &content).unwrap();
        let mut conn = indexer::open_pastor_db(db).unwrap();
        let doc = sermon::SermonDoc::parse(&content).unwrap();
        let mut stats = indexer::IndexStats::default();
        indexer::index_single(&mut conn, "alpha.md", &doc, &mut stats).unwrap();
        (conn, content)
    }

    #[test]
    fn settings_roundtrip_preserves_core_fields() {
        let cfg = AppConfig {
            vault_path: "/v".into(),
            canon_path: "/c.db".into(),
            pastor_path: "/p.db".into(),
            librarian_enabled: false,
            font_size: 17,
            high_contrast: true,
        };
        let dto = settings_from_config(&cfg);
        let back = settings_to_config(&dto, &cfg);
        // AppConfig has no PartialEq (contracts.lock); compare fields directly.
        assert_eq!(back.vault_path, cfg.vault_path);
        assert_eq!(back.canon_path, cfg.canon_path);
        assert_eq!(back.pastor_path, cfg.pastor_path);
        assert_eq!(back.librarian_enabled, cfg.librarian_enabled);
        assert_eq!(back.font_size, cfg.font_size);
        assert_eq!(back.high_contrast, cfg.high_contrast);
    }

    #[test]
    fn create_load_save_roundtrip() {
        let (_tmp, vault, db) = setup("crls");
        let mut conn = indexer::open_pastor_db(&db).unwrap();
        let (dto, baseline) = create_sermon(
            &vault,
            &mut conn,
            &CreateSermonReq {
                title: Some("New Sermon".into()),
                scripture: Some("John 3:16".into()),
                series: None,
            },
        )
        .unwrap();
        assert_eq!(dto.title, "New Sermon");
        assert!(dto.source_path.as_deref().unwrap().ends_with("new-sermon.md"));
        assert!(sermon_core::atomic_save::vault_file_exists(
            &vault,
            dto.source_path.as_deref().unwrap()
        ));

        let (loaded, loaded_baseline) = load_sermon(&vault, dto.source_path.as_deref().unwrap()).unwrap();
        assert_eq!(loaded.title, "New Sermon");
        assert_eq!(loaded_baseline.hash, baseline.hash);

        let mut baselines = SessionBaselines::default();
        baselines.record(&loaded.id, loaded_baseline);
        let mut edited = loaded.clone();
        edited.body.push_str("\nEdited paragraph.\n");
        let saved = save_sermon(&vault, &mut conn, &mut baselines, &edited).unwrap();
        assert!(saved.success);
        let on_disk = sermon_core::atomic_save::read_vault_file(
            &vault,
            dto.source_path.as_deref().unwrap(),
        )
        .unwrap();
        assert!(on_disk.contains("Edited paragraph."));
    }

    #[test]
    fn save_never_overwrites_unseen_disk_changes() {
        let (_tmp, vault, db) = setup("conflict");
        let (mut conn, _content) = seed(&vault, &db);

        // Editor loads the sermon.
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        // External edit lands on disk.
        let external = sermon_md("alpha", "Alpha", "Externally edited body.");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();

        // Editor saves: must report a conflict, not overwrite.
        let mut edited = doc.clone();
        edited.body.push_str("\nLocal edit.\n");
        let result = save_sermon(&vault, &mut conn, &mut baselines, &edited).unwrap();
        assert!(!result.success);
        assert!(result.conflict.is_some());
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("Externally edited body."));
        assert!(!on_disk.contains("Local edit."));
    }

    #[test]
    fn save_refuses_html_body_and_leaves_canonical_file_untouched() {
        let (_tmp, vault, db) = setup("htmlguard");
        let (mut conn, _content) = seed(&vault, &db);
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        // The transport contract declares body as TipTap HTML; persisting it
        // would strip frontmatter/directives from the canonical file.
        let mut html_doc = doc.clone();
        html_doc.body = "<h1>Alpha</h1><p>God so loved the world.</p>".to_string();
        let result = save_sermon(&vault, &mut conn, &mut baselines, &html_doc);
        assert!(result.is_err(), "HTML body must be refused: {result:?}");
        assert!(result.unwrap_err().contains("non-markdown body"));

        // Canonical file is byte-identical after the refused save.
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("id: alpha"), "frontmatter must survive");
        assert!(!on_disk.contains("<h1>"));

        // Inline HTML inside an otherwise-markdown body is legal and passes.
        let mut inline = doc.clone();
        inline.body.push_str("\nText with <em>inline</em> HTML.\n");
        let result = save_sermon(&vault, &mut conn, &mut baselines, &inline).unwrap();
        assert!(result.success);
    }

    #[test]
    fn save_splices_body_into_disk_raw_preserving_frontmatter_and_identity() {
        let (_tmp, vault, db) = setup("splicefm");
        // Seed with an unknown frontmatter field that must survive a prose edit.
        let content = "---\nid: alpha\ntitle: \"Alpha\"\nprimary_passage: \"John.3.16\"\nbig_idea: \"idea\"\nstructure_type: verse_by_verse\ncustom_field: \"keep-me\"\n---\n\nGod so loved the world.\n";
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &content).unwrap();
        let mut conn = indexer::open_pastor_db(&db).unwrap();
        let doc = sermon::SermonDoc::parse(&content).unwrap();
        let mut stats = indexer::IndexStats::default();
        indexer::index_single(&mut conn, "alpha.md", &doc, &mut stats).unwrap();

        let (loaded, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        assert_eq!(loaded.id, "alpha");
        let mut baselines = SessionBaselines::default();
        baselines.record(&loaded.id, baseline);

        // A pure prose edit: the body alone must not replace the document.
        let mut edited = loaded.clone();
        edited.body.push_str("\nEdited paragraph.\n");
        let saved = save_sermon(&vault, &mut conn, &mut baselines, &edited).unwrap();
        assert!(saved.success);

        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("id: alpha"), "frontmatter must survive");
        assert!(
            on_disk.contains("custom_field: \"keep-me\""),
            "unknown frontmatter field must survive a body-only save"
        );
        assert!(on_disk.contains("Edited paragraph."));
        assert!(on_disk.contains("God so loved the world."));

        // Identity is stable across the save → reload round trip.
        let (reloaded, _) = load_sermon(&vault, "alpha.md").unwrap();
        assert_eq!(reloaded.id, loaded.id);
        assert!(reloaded.body.contains("Edited paragraph."));
    }

    #[test]
    fn save_preserves_directive_fences_and_known_directive_recognition() {
        let (_tmp, vault, db) = setup("splicedir");
        let body = "Opening prose.\n\n:::movement{title=\"The Eternal Word\" index=\"1\"}\nIn the beginning was the Word.\n:::\n\nMiddle prose.\n\n:::exegetical-notes\nGreek: logos.\n:::\n\nClosing prose.";
        let content = sermon_md("alpha", "Alpha", body);
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &content).unwrap();
        let mut conn = indexer::open_pastor_db(&db).unwrap();
        let doc = sermon::SermonDoc::parse(&content).unwrap();
        let mut stats = indexer::IndexStats::default();
        indexer::index_single(&mut conn, "alpha.md", &doc, &mut stats).unwrap();

        let (loaded, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        assert!(
            loaded.directives.iter().any(|d| d.key == "movement"),
            "movement directive recognized at load"
        );
        assert!(loaded.directives.iter().any(|d| d.key == "exegetical-notes"));
        let mut baselines = SessionBaselines::default();
        baselines.record(&loaded.id, baseline);

        let mut edited = loaded.clone();
        edited.body.push_str("\n\nFinal paragraph.\n");
        let saved = save_sermon(&vault, &mut conn, &mut baselines, &edited).unwrap();
        assert!(saved.success);

        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        // The loaded body is a re-serialization, so assert semantic survival
        // (fence + attribute value), not byte identity.
        assert!(on_disk.contains(":::movement"), "movement fence survives");
        assert!(on_disk.contains("The Eternal Word"));
        assert!(on_disk.contains(":::exegetical-notes"));

        // After save + reload the known directive is still recognized (not
        // demoted to unknown) and the fence survives in the body.
        let (reloaded, _) = load_sermon(&vault, "alpha.md").unwrap();
        assert!(
            reloaded.directives.iter().any(|d| d.key == "movement"),
            "movement directive still recognized after save+reload"
        );
        assert!(reloaded.body.contains(":::exegetical-notes"));
        assert!(reloaded.body.contains("Final paragraph."));
    }

    #[test]
    fn keep_local_after_conflicted_save_preserves_frontmatter_and_local_edit() {
        let (_tmp, vault, db) = setup("keeplocal");
        let (mut conn, _content) = seed(&vault, &db);
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        // External edit lands on disk, then the editor's save is rejected.
        let external = sermon_md("alpha", "Alpha", "External body.");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();
        let mut edited = doc.clone();
        edited.body.push_str("\nLocal edit.\n");
        let result = save_sermon(&vault, &mut conn, &mut baselines, &edited).unwrap();
        assert!(!result.success);

        // Keep Local must write the editor's content without stripping the
        // frontmatter (the recorded buffer carries the full raw document).
        let res = resolve_conflict(
            &vault,
            &mut conn,
            &mut baselines,
            &ConflictResolutionDto {
                sermon_id: doc.id.clone(),
                strategy: "keep-local".into(),
                merged_body: None,
                save_as_path: None,
            },
        )
        .unwrap();
        assert!(res.success);
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("id: alpha"), "frontmatter must survive keep-local");
        assert!(on_disk.contains("Local edit."));
        assert!(!on_disk.contains("External body."));
    }

    #[test]
    fn merge_strategy_splices_merged_body_into_disk_frontmatter() {
        let (_tmp, vault, db) = setup("mergefm");
        let (mut conn, _content) = seed(&vault, &db);
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        let external = sermon_md("alpha", "Alpha", "Disk body.");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();

        // mergedBody is body-only Markdown from the frontend merge view.
        let res = resolve_conflict(
            &vault,
            &mut conn,
            &mut baselines,
            &ConflictResolutionDto {
                sermon_id: doc.id.clone(),
                strategy: "merge".into(),
                merged_body: Some(
                    "Merged body paragraph.\n\n:::movement{title=\"M\"}\nMerged movement.\n:::\n"
                        .to_string(),
                ),
                save_as_path: None,
            },
        )
        .unwrap();
        assert!(res.success);
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("id: alpha"), "frontmatter must survive merge");
        assert!(on_disk.contains("Merged body paragraph."));
        assert!(on_disk.contains(":::movement{title=\"M\"}"));
        assert!(!on_disk.contains("Disk body."));
    }

    #[test]
    fn use_disk_reanchors_baseline_and_drops_pending_buffer() {
        let (_tmp, vault, db) = setup("usedisk");
        let (mut conn, _content) = seed(&vault, &db);
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        // External edit, then a rejected local save (records the buffer).
        let external = sermon_md("alpha", "Alpha", "Disk version wins.");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();
        let mut edited = doc.clone();
        edited.body.push_str("\nLocal edit.\n");
        let result = save_sermon(&vault, &mut conn, &mut baselines, &edited).unwrap();
        assert!(!result.success);

        // Status before resolving: both changed.
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "both-changed");

        // Use Disk: disk is adopted, buffer dropped, status clean again.
        let res = resolve_conflict(
            &vault,
            &mut conn,
            &mut baselines,
            &ConflictResolutionDto {
                sermon_id: doc.id.clone(),
                strategy: "use-disk".into(),
                merged_body: None,
                save_as_path: None,
            },
        )
        .unwrap();
        assert!(res.success);
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "clean");
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("Disk version wins."));
    }

    #[test]
    fn html_body_is_refused_for_conflict_strategies_too() {
        let (_tmp, vault, db) = setup("htmlresolve");
        let (mut conn, _content) = seed(&vault, &db);
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        let res = resolve_conflict(
            &vault,
            &mut conn,
            &mut baselines,
            &ConflictResolutionDto {
                sermon_id: doc.id.clone(),
                strategy: "merge".into(),
                merged_body: Some("<p>merged as HTML</p>".to_string()),
                save_as_path: None,
            },
        );
        assert!(res.is_err(), "HTML merged body must be refused");
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("id: alpha"), "canonical file untouched");
    }

    #[test]
    fn resolve_conflict_keep_local_and_save_as() {
        let (_tmp, vault, db) = setup("resolve");
        let (mut conn, _content) = seed(&vault, &db);
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        let mut baselines = SessionBaselines::default();
        baselines.record(&doc.id, baseline);

        // Keep Local writes the session buffer.
        let external = sermon_md("alpha", "Alpha", "External body.");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();
        let res = resolve_conflict(
            &vault,
            &mut conn,
            &mut baselines,
            &ConflictResolutionDto {
                sermon_id: doc.id.clone(),
                strategy: "keep-local".into(),
                merged_body: None,
                save_as_path: None,
            },
        )
        .unwrap();
        assert!(res.success);
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("God so loved the world."));

        // Save Local As writes to a new path, leaving the original alone.
        let res = resolve_conflict(
            &vault,
            &mut conn,
            &mut baselines,
            &ConflictResolutionDto {
                sermon_id: doc.id.clone(),
                strategy: "save-local-as".into(),
                merged_body: None,
                save_as_path: Some("drafts/alpha-local.md".into()),
            },
        )
        .unwrap();
        assert!(res.success);
        assert!(sermon_core::atomic_save::vault_file_exists(&vault, "drafts/alpha-local.md"));
    }

    #[test]
    fn filesystem_status_reports_conflict_and_missing() {
        let (_tmp, vault, db) = setup("fsstatus");
        let (conn, content) = seed(&vault, &db);
        let mut baselines = SessionBaselines::default();
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        baselines.record(&doc.id, baseline);

        // Clean right after load.
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "clean");

        // Local edits only (a rejected save records the pending buffer while
        // keeping the load-time anchor).
        baselines.record_buffer(&doc.id, format!("{content}\nlocal edit\n"));
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "local-dirty");

        // Both changed.
        let external = sermon_md("alpha", "Alpha", "External edit.");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "both-changed");

        // Missing + recorded → recovery-available.
        std::fs::remove_file(vault.join("alpha.md")).unwrap();
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "missing");
        drop(conn);
        // Record the disappearance through reconciliation, then re-check.
        let conn = indexer::open_pastor_db(&db).unwrap();
        reconcile::reconcile(&vault, &db).unwrap();
        let st = get_filesystem_status(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(st.state, "recovery-available");
        assert_eq!(st.recovery_path.as_deref(), Some("alpha.md"));
    }

    #[test]
    fn prepare_diff_produces_hunks() {
        let (_tmp, vault, db) = setup("diff");
        let (conn, content) = seed(&vault, &db);
        let mut baselines = SessionBaselines::default();
        let (doc, baseline) = load_sermon(&vault, "alpha.md").unwrap();
        baselines.record(&doc.id, baseline);
        let external = format!("{content}extra line\n");
        sermon_core::atomic_save::save_sermon_atomic(&vault, "alpha.md", &external).unwrap();
        let diff = prepare_diff(&vault, &conn, &baselines, &doc.id).unwrap();
        assert_eq!(diff.disk_lines.len(), diff.local_lines.len() + 1);
        assert!(!diff.hunks.is_empty());
    }

    #[test]
    fn archive_moves_file_and_reindex_follows() {
        let (_tmp, vault, db) = setup("archive");
        let (conn, _content) = seed(&vault, &db);
        assert!(find_sermon_path(&conn, "alpha").unwrap().is_some());
        drop(conn);
        archive_sermon(&vault, &db, "alpha").unwrap();
        assert!(!sermon_core::atomic_save::vault_file_exists(&vault, "alpha.md"));
        assert!(sermon_core::atomic_save::vault_file_exists(&vault, "archive/alpha.md"));
        let conn = indexer::open_pastor_db(&db).unwrap();
        assert_eq!(
            find_sermon_path(&conn, "alpha").unwrap().as_deref(),
            Some("archive/alpha.md")
        );
    }

    #[test]
    fn delete_removes_file_and_derived_rows() {
        let (_tmp, vault, db) = setup("delete");
        let (conn, _content) = seed(&vault, &db);
        delete_sermon(&vault, &conn, "alpha").unwrap();
        assert!(!sermon_core::atomic_save::vault_file_exists(&vault, "alpha.md"));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sermon_index", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn rename_updates_title_and_preserves_body() {
        let (_tmp, vault, db) = setup("rename");
        let (mut conn, content) = seed(&vault, &db);
        let summary = rename_sermon(&vault, &mut conn, "alpha", "Alpha Renewed").unwrap();
        assert_eq!(summary.title, "Alpha Renewed");
        let on_disk = sermon_core::atomic_save::read_vault_file(&vault, "alpha.md").unwrap();
        assert!(on_disk.contains("title: \"Alpha Renewed\""));
        // Body blocks preserved verbatim (including trailing content).
        let body_tail: String = content.lines().skip(8).collect::<Vec<_>>().join("\n");
        assert!(on_disk.contains(body_tail.trim()));
        assert_eq!(summary.id, "alpha", "identity must survive a rename");
    }

    #[test]
    fn duplicate_gets_new_identity() {
        let (_tmp, vault, db) = setup("dup");
        let (mut conn, _content) = seed(&vault, &db);
        let (dto, _b) = duplicate_sermon(&vault, &mut conn, "alpha", None).unwrap();
        assert_ne!(dto.id, "alpha");
        assert!(dto.title.contains("copy"));
        assert!(sermon_core::atomic_save::vault_file_exists(
            &vault,
            dto.source_path.as_deref().unwrap()
        ));
    }

    #[test]
    fn codec_command_matches_frontend_pass_contract() {
        let good = ":::movement{title=\"T\"}\nBody.\n:::\n\n:::unknown{a=\"1\"}\nKeep.\n:::";
        let res = test_directive_codec(good);
        assert!(res.pass);
        assert_eq!(res.parsed.len(), 2);
        assert_eq!(res.parsed[1].kind, "unknown");
        assert!(res.serialized.contains(&res.parsed[1].raw_source));
    }

    #[test]
    fn lint_transport_returns_real_track_e_findings() {
        let (_tmp, vault, _db) = setup("lint-bridge");
        let raw = "---\nid: lint-me\ntitle: \"Lint Me\"\n---\n\n:::movement{title=\"Unanchored\"}\nBody.\n:::\n";
        sermon_core::atomic_save::save_sermon_atomic(&vault, "lint-me.md", raw).unwrap();
        let doc = document_from_raw(raw, "lint-me.md").unwrap();
        let findings = lint_document(&vault, None, &doc).unwrap();
        let rules: Vec<&str> = findings.iter().map(|finding| finding.rule_id.as_str()).collect();
        assert!(rules.contains(&linter::RULE_MISSING_BIG_IDEA));
        assert!(rules.contains(&linter::RULE_ORPHANED_MOVEMENT));
        assert!(rules.contains(&linter::RULE_MISSING_APPLICATION));
        assert!(findings.iter().all(|finding| finding.code == finding.rule_id));
    }

    #[test]
    fn export_transport_uses_real_snapshot_and_track_d_pdf() {
        let (_tmp, vault, db) = setup("export-bridge");
        let (conn, _content) = seed(&vault, &db);
        let snapshot = create_export_source_snapshot(&vault, &conn, "alpha").unwrap();
        let output = vault.join("exports").join("alpha.pdf");
        let request = ExportRequestDto {
            sermon_id: "alpha".to_string(),
            format: "pulpit_manuscript".to_string(),
            manuscript_mode: Some("manuscript".to_string()),
            options: ExportOptionsDto {
                include_notes: Some(false),
                output_filename: None,
                output_path: Some(output.display().to_string()),
            },
            snapshot_id: Some(snapshot.snapshot_id.clone()),
        };
        let result = execute_export_snapshot(&vault, &snapshot, &request).unwrap();
        assert!(result.success);
        assert_eq!(result.snapshot_id.as_deref(), Some(snapshot.snapshot_id.as_str()));
        assert_eq!(result.format, "pulpit_manuscript");
        let pdf = std::fs::read(output).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn rfc3339_format_is_stable() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
        // 1970 is not a leap year: 365 days lands on 1971-01-01.
        assert_eq!(format_rfc3339(86_400 * 365), "1971-01-01T00:00:00Z");
        // Time-of-day components.
        assert_eq!(
            format_rfc3339(86_400 * 365 + 37_123),
            "1971-01-01T10:18:43Z"
        );
    }

    #[test]
    fn index_status_reports_counts_and_meta() {
        let (_tmp, vault, db) = setup("status");
        let (_conn, _c) = seed(&vault, &db);
        indexer::rebuild(&vault, &db).unwrap();
        let st = index_status(&db).unwrap();
        assert_eq!(st.indexed_file_count, 1);
        assert!(st.last_full_scan_time.is_some());
        assert_eq!(st.status, "idle");
    }

    #[test]
    fn archive_stats_counts_words() {
        let (_tmp, vault, db) = setup("stats");
        let (conn, _c) = seed(&vault, &db);
        let stats = archive_stats(&conn).unwrap();
        assert_eq!(stats.total_sermons, 1);
        assert!(stats.total_words >= 5);
    }

    #[test]
    fn recover_reports_no_copy_when_gone() {
        let (_tmp, vault, db) = setup("recover");
        let (_conn, _c) = seed(&vault, &db);
        std::fs::remove_file(vault.join("alpha.md")).unwrap();
        reconcile::reconcile(&vault, &db).unwrap();
        let conn = indexer::open_pastor_db(&db).unwrap();
        let res = recover_sermon(&vault, &db, &conn, "alpha").unwrap();
        assert!(!res.success);
        assert!(res.message.contains("no copy"));
    }
}
