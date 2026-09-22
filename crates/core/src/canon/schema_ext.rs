//! # canon.db schema extension (V1)
//!
//! **This module EXTENDS the existing canon builder; it does not rewrite it.**
//! The base tables (`bible_books`, `bible_verses`, `strongs_lexicon`,
//! `verse_words`, `cross_references`) are created by [`crate::schema::CANON_SCHEMA`]
//! and populated by [`crate::canon::build_canon_db`]. This module adds:
//!
//! * new tables: `sources`, `topics`, `topic_verses`, `chain_edges`, `canon_meta`
//! * new columns on existing tables (added idempotently via `ALTER TABLE`)
//! * a version manifest written into `canon_meta`
//!
//! ## Reconciliation with the existing schema
//!
//! The V1 brief referred to tables named `lexicon` and `xrefs`. The foundation
//! actually names them `strongs_lexicon` and `cross_references`. This module
//! targets the **actual** names and records the mapping in the final report.
//!
//! ## Idempotency
//!
//! `apply_canon_extensions` is safe to call repeatedly. `CREATE TABLE IF NOT
//! EXISTS` handles tables; `ensure_column` checks `PRAGMA table_info` before
//! issuing `ALTER TABLE ... ADD COLUMN` (SQLite has no `ADD COLUMN IF NOT
//! EXISTS`).

use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// New tables introduced by the V1 canon extension.
pub const CANON_EXT_TABLES: &str = r#"
-- Source registry: every dataset that contributed rows to canon.db.
CREATE TABLE IF NOT EXISTS sources (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    license_code TEXT NOT NULL,
    attribution  TEXT NOT NULL,
    url          TEXT,
    version      TEXT
);

-- Topical index (Nave's, Torrey's, ...).
CREATE TABLE IF NOT EXISTS topics (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    source_id      TEXT NOT NULL REFERENCES sources(id),
    source_version TEXT
);

-- Topic -> verse membership with a weight.
CREATE TABLE IF NOT EXISTS topic_verses (
    topic_id  TEXT NOT NULL REFERENCES topics(id),
    verse_id  INTEGER NOT NULL REFERENCES bible_verses(id),
    weight    REAL NOT NULL DEFAULT 1.0,
    source_id TEXT NOT NULL REFERENCES sources(id),
    PRIMARY KEY (topic_id, verse_id)
);

-- Chain Study edges (deterministic, rule-derived; NOT Thompson's copyrighted data).
CREATE TABLE IF NOT EXISTS chain_edges (
    from_verse INTEGER NOT NULL REFERENCES bible_verses(id),
    to_verse   INTEGER NOT NULL REFERENCES bible_verses(id),
    kind       TEXT NOT NULL,
    weight     REAL NOT NULL DEFAULT 1.0,
    source_id  TEXT NOT NULL REFERENCES sources(id),
    rule_id    TEXT,
    PRIMARY KEY (from_verse, to_verse, kind)
);

-- Key/value metadata, including the version manifest.
CREATE TABLE IF NOT EXISTS canon_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_topic_verses_topic ON topic_verses(topic_id);
CREATE INDEX IF NOT EXISTS idx_topic_verses_verse ON topic_verses(verse_id);
CREATE INDEX IF NOT EXISTS idx_chain_edges_from ON chain_edges(from_verse);
CREATE INDEX IF NOT EXISTS idx_chain_edges_to ON chain_edges(to_verse);
"#;

/// Columns added to existing tables. `(table, column, declaration)`.
pub const CANON_EXT_COLUMNS: &[(&str, &str, &str)] = &[
    // verse_words additions
    ("verse_words", "strongs_extended", "TEXT"),
    ("verse_words", "classic_strongs", "TEXT"),
    ("verse_words", "morphology_code", "TEXT"),
    ("verse_words", "lemma", "TEXT"),
    ("verse_words", "gloss", "TEXT"),
    ("verse_words", "source_id", "TEXT REFERENCES sources(id)"),
    // strongs_lexicon additions (derivation already exists in the base schema)
    ("strongs_lexicon", "usage_note", "TEXT"),
    ("strongs_lexicon", "source_id", "TEXT REFERENCES sources(id)"),
    ("strongs_lexicon", "source_version", "TEXT"),
    // cross_references additions
    ("cross_references", "weight", "REAL DEFAULT 1.0"),
    ("cross_references", "source_id", "TEXT REFERENCES sources(id)"),
];

/// Apply the canon extension to an open canon.db connection. Idempotent.
pub fn apply_canon_extensions(conn: &Connection) -> Result<()> {
    conn.execute_batch(CANON_EXT_TABLES)?;
    for (table, column, decl) in CANON_EXT_COLUMNS {
        ensure_column(conn, table, column, decl)?;
    }
    Ok(())
}

/// Add `column` to `table` if it does not already exist.
///
/// SQLite lacks `ADD COLUMN IF NOT EXISTS`, so we introspect first. `table` and
/// `column` are internal constants (never user input), so the formatted SQL is
/// safe.
fn ensure_column(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let existing: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<std::result::Result<_, _>>()?;
    if existing.iter().any(|c| c == column) {
        return Ok(());
    }
    conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
    Ok(())
}

/// The canon.db version manifest. Serialized to JSON and stored in `canon_meta`
/// under the key `manifest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonManifest {
    /// Monotonic canon version string (e.g. `2025.01`).
    pub canon_version: String,
    /// RFC 3339 build timestamp.
    pub build_timestamp: String,
    /// source_id -> source version.
    pub source_versions: BTreeMap<String, String>,
    /// source_id -> checksum of the raw input used.
    pub source_checksums: BTreeMap<String, String>,
}

impl CanonManifest {
    /// Serialize and store the manifest in `canon_meta`.
    pub fn write(&self, conn: &Connection) -> Result<()> {
        let json = serde_json::to_string(self)?;
        conn.execute(
            "INSERT OR REPLACE INTO canon_meta(key, value) VALUES ('manifest', ?1)",
            [json],
        )?;
        Ok(())
    }

    /// Read the manifest from `canon_meta`, if present.
    pub fn read(conn: &Connection) -> Result<Option<CanonManifest>> {
        let mut stmt = conn.prepare("SELECT value FROM canon_meta WHERE key = 'manifest'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            Ok(Some(serde_json::from_str(&json)?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_canon() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::schema::CANON_SCHEMA).unwrap();
        conn
    }

    #[test]
    fn extensions_apply_idempotently() {
        let conn = fresh_canon();
        apply_canon_extensions(&conn).unwrap();
        // Second call must not error (no duplicate columns/tables).
        apply_canon_extensions(&conn).unwrap();

        // New tables exist.
        for t in ["sources", "topics", "topic_verses", "chain_edges", "canon_meta"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {t}");
        }

        // New columns exist on existing tables.
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(verse_words)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for c in ["strongs_extended", "classic_strongs", "morphology_code", "lemma", "gloss", "source_id"] {
            assert!(cols.contains(&c.to_string()), "verse_words missing {c}");
        }
    }

    #[test]
    fn manifest_round_trips() {
        let conn = fresh_canon();
        apply_canon_extensions(&conn).unwrap();
        let mut m = CanonManifest {
            canon_version: "2025.01".to_string(),
            build_timestamp: "2025-01-01T00:00:00Z".to_string(),
            source_versions: BTreeMap::new(),
            source_checksums: BTreeMap::new(),
        };
        m.source_versions.insert("strongs-pd".to_string(), "1.0".to_string());
        m.source_checksums.insert("strongs-pd".to_string(), "deadbeef".to_string());
        m.write(&conn).unwrap();
        let back = CanonManifest::read(&conn).unwrap().unwrap();
        assert_eq!(back, m);
    }
}
