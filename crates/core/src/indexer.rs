//! pastor.db indexer.
//!
//! Walks a vault directory of `.md` sermons and builds the derived index.
//! The index is *always* rebuildable from disk: `rebuild` drops every derived
//! table and re-ingests. `sync` performs an incremental update, skipping files
//! whose SHA-256 hash is unchanged.

use crate::error::{CoreError, Result};
use crate::schema::PASTOR_SCHEMA;
use crate::sermon::{self, SermonDoc};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct IndexStats {
    pub scanned: usize,
    pub indexed: usize,
    pub skipped_unchanged: usize,
    pub removed: usize,
    pub links: usize,
    pub illustrations: usize,
    pub elapsed_ms: u128,
}

/// Open (creating if needed) the pastor.db at `path` with the derived schema.
pub fn open_pastor_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // Reconciliation (watcher thread) and editor commands use separate
    // connections concurrently; give SQLite a bounded wait instead of failing
    // fast on a transient write lock.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(PASTOR_SCHEMA)?;
    Ok(conn)
}

/// Vault-relative path string with forward slashes — the platform-stable
/// representation stored in `sermon_index.file_path` (shared by the indexer,
/// reconciliation, and the watcher so they always agree).
pub(crate) fn rel_path_string(vault: &Path, path: &Path) -> String {
    path.strip_prefix(vault)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// True when any vault-relative path component is hidden and must be skipped
/// (mirrors the v1 rule: relative to the vault root, so a vault under a
/// dot-directory still works).
pub(crate) fn is_hidden_rel(rel: &str) -> bool {
    rel.split('/').any(|c| c.starts_with('.') && c != "." && c != "..")
}

/// Full rebuild: wipe all derived tables and re-ingest every `.md` in `vault`.
pub fn rebuild(vault: &Path, db_path: &Path) -> Result<IndexStats> {
    if !vault.is_dir() {
        return Err(CoreError::VaultMissing(vault.display().to_string()));
    }
    let start = std::time::Instant::now();
    let mut conn = open_pastor_db(db_path)?;

    // Drop derived content. `sermons_fts` is self-contained (it stores its
    // own text; see PASTOR_SCHEMA), so a plain DELETE clears it together with
    // the base tables and it is repopulated during re-ingest.
    conn.execute_batch(
        "DELETE FROM illustration_usage;
         DELETE FROM scripture_sermon_links;
         DELETE FROM sermon_body;
         DELETE FROM sermon_index;
         DELETE FROM sermons_fts;
         DELETE FROM index_meta;",
    )?;

    let mut stats = index_into(&mut conn, vault, false)?;
    stats.elapsed_ms = start.elapsed().as_millis();

    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('last_rebuild', ?1)",
        params![chrono::Utc::now().to_rfc3339()],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('vault', ?1)",
        params![vault.display().to_string()],
    )?;
    Ok(stats)
}

/// Incremental sync: only re-index files whose hash changed, and remove rows
/// for files that no longer exist.
pub fn sync(vault: &Path, db_path: &Path) -> Result<IndexStats> {
    if !vault.is_dir() {
        return Err(CoreError::VaultMissing(vault.display().to_string()));
    }
    let start = std::time::Instant::now();
    let mut conn = open_pastor_db(db_path)?;
    let mut stats = index_into(&mut conn, vault, true)?;
    stats.elapsed_ms = start.elapsed().as_millis();
    Ok(stats)
}

fn index_into(conn: &mut Connection, vault: &Path, incremental: bool) -> Result<IndexStats> {
    let mut stats = IndexStats::default();

    // Existing hashes for incremental comparison.
    let mut existing: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if incremental {
        let mut stmt = conn.prepare("SELECT file_path, file_hash FROM sermon_index")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (p, h) = row?;
            existing.insert(p, h);
        }
    }

    let mut seen_paths: HashSet<String> = HashSet::new();

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
        let rel = rel_path_string(vault, path);
        // Skip hidden files/dirs (also skips atomic-save `.sstmp` artifacts).
        if is_hidden_rel(&rel) {
            continue;
        }
        stats.scanned += 1;
        seen_paths.insert(rel.clone());

        let raw = std::fs::read_to_string(path)?;
        let hash = sermon::sha256_hex(raw.as_bytes());

        if incremental {
            if let Some(prev) = existing.get(&rel) {
                if prev == &hash {
                    stats.skipped_unchanged += 1;
                    continue;
                }
            }
        }

        let doc = SermonDoc::parse(&raw)?;
        index_one(conn, &rel, &doc, &mut stats)?;
        stats.indexed += 1;
    }

    // Remove rows for files that vanished.
    if incremental {
        let stale: Vec<String> = existing
            .keys()
            .filter(|p| !seen_paths.contains(*p))
            .cloned()
            .collect();
        for p in stale {
            conn.execute("DELETE FROM sermon_index WHERE file_path = ?1", params![p])?;
            stats.removed += 1;
        }
    }

    Ok(stats)
}

/// Index a single already-parsed sermon (used by the editor's save path so the
/// index stays current without a full vault pass).
pub fn index_single(
    conn: &mut Connection,
    rel_path: &str,
    doc: &SermonDoc,
    stats: &mut IndexStats,
) -> Result<()> {
    index_one(conn, rel_path, doc, stats)
}

fn index_one(
    conn: &mut Connection,
    rel_path: &str,
    doc: &SermonDoc,
    stats: &mut IndexStats,
) -> Result<()> {
    let id = doc.resolved_id();
    let title = doc.title();
    let passage = doc.primary_passage();
    let big_idea = doc.big_idea();
    let structure = doc.structure_type();
    let fm = &doc.frontmatter;

    let tx = conn.transaction()?;

    // Remove any prior rows for this file path or id (handles renames/id changes).
    tx.execute("DELETE FROM sermon_index WHERE file_path = ?1 OR id = ?2", params![rel_path, id])?;

    tx.execute(
        "INSERT INTO sermon_index
         (id, file_path, file_hash, title, date_preached, series, liturgical_season,
          primary_passage, exegetical_proposition, big_idea, structure_type, last_indexed_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            id,
            rel_path,
            doc.file_hash,
            title,
            fm.date_preached,
            fm.series,
            fm.liturgical_season,
            passage,
            fm.exegetical_proposition,
            big_idea,
            structure,
            chrono::Utc::now().to_rfc3339(),
        ],
    )?;

    // Body cache + FTS content.
    tx.execute(
        "INSERT OR REPLACE INTO sermon_body(sermon_id, body_content) VALUES (?1, ?2)",
        params![id, doc.body],
    )?;
    // Refresh the self-contained FTS row for this sermon.
    tx.execute("DELETE FROM sermons_fts WHERE sermon_id = ?1", params![id])?;
    tx.execute(
        "INSERT INTO sermons_fts(sermon_id, title, primary_passage, big_idea, body_content)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, passage, big_idea, doc.body],
    )?;

    // Scripture link graph.
    let mut link_count = 0usize;
    let mut seen_verses: HashSet<(i64, i64, i64)> = HashSet::new();
    for pr in doc.all_references() {
        for v in pr.expand() {
            if seen_verses.insert((v.book_num, v.chapter, v.verse)) {
                tx.execute(
                    "INSERT OR IGNORE INTO scripture_sermon_links(sermon_id, book_num, chapter, verse)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![id, v.book_num, v.chapter, v.verse],
                )?;
                link_count += 1;
            }
        }
    }
    stats.links += link_count;

    // Illustration fatigue tracker.
    let now = chrono::Utc::now().to_rfc3339();
    let mut illus_count = 0usize;
    for label in &fm.illustrations {
        let key = sermon::slugify(label);
        if key.is_empty() {
            continue;
        }
        tx.execute(
            "INSERT OR REPLACE INTO illustration_usage
             (sermon_id, illustration_key, label, first_seen, last_used, use_count)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![id, key, label, now, now],
        )?;
        illus_count += 1;
    }
    stats.illustrations += illus_count;

    tx.commit()?;
    Ok(())
}

/// List all `.md` files in a vault (relative paths), for UI display.
pub fn list_vault_files(vault: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(vault).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_sermon(dir: &Path, name: &str, body: &str) {
        let p = dir.join(name);
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn rebuild_and_sync() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        std::fs::create_dir_all(&vault).unwrap();
        let db = tmp.path().join("pastor.db");

        write_sermon(
            &vault,
            "a.md",
            "---\nid: a\ntitle: \"Alpha\"\nprimary_passage: \"Rom.8.28\"\nbig_idea: \"God works.\"\nstructure_type: verse_by_verse\n---\n\nBody about Romans 8:28.\n",
        );
        write_sermon(
            &vault,
            "b.md",
            "---\nid: b\ntitle: \"Beta\"\nprimary_passage: \"John.3.16\"\nbig_idea: \"God loves.\"\nstructure_type: narrative\n---\n\nBody about John 3:16.\n",
        );

        let stats = rebuild(&vault, &db).unwrap();
        assert_eq!(stats.indexed, 2);

        // Incremental sync should skip both.
        let stats2 = sync(&vault, &db).unwrap();
        assert_eq!(stats2.skipped_unchanged, 2);
        assert_eq!(stats2.indexed, 0);

        // Modify one file -> one re-index.
        write_sermon(
            &vault,
            "a.md",
            "---\nid: a\ntitle: \"Alpha Revised\"\nprimary_passage: \"Rom.8.28\"\nbig_idea: \"God works all things.\"\nstructure_type: verse_by_verse\n---\n\nRevised body.\n",
        );
        let stats3 = sync(&vault, &db).unwrap();
        assert_eq!(stats3.indexed, 1);
        assert_eq!(stats3.skipped_unchanged, 1);
    }
}
