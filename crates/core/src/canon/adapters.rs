//! # canon.db ETL adapters (V1 scaffold)
//!
//! Each upstream dataset is wrapped in a [`CanonAdapter`] that declares its
//! [`SourceRecord`] (id, name, license, attribution, url, version) and knows how
//! to ingest its normalized TSV into canon.db.
//!
//! ## No runtime downloads
//!
//! Adapters read **only** from the local `clean_dir` produced by `tools/etl.py`.
//! There is no network access anywhere in this module. Raw acquisition is a
//! separate, offline, human-run step.
//!
//! ## Scaffold scope
//!
//! The `ingest` implementations here are deliberately minimal stubs: they
//! register the source and (if the expected TSV exists) count rows. The full
//! column mapping for each dataset is a later track. What is frozen *now* is the
//! adapter interface and the source registry, so the pipeline can be built
//! against a stable seam.

use crate::error::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

/// A registered data source. Mirrors a row in the `sources` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub id: String,
    pub name: String,
    pub license_code: String,
    pub attribution: String,
    pub url: Option<String>,
    pub version: Option<String>,
}

impl SourceRecord {
    /// Insert (or replace) this source row.
    pub fn upsert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO sources(id, name, license_code, attribution, url, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                self.id,
                self.name,
                self.license_code,
                self.attribution,
                self.url,
                self.version
            ],
        )?;
        Ok(())
    }
}

/// Rows ingested by an adapter.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AdapterStats {
    pub rows: usize,
}

/// Shared context passed to every adapter during a build.
pub struct IngestContext<'a> {
    /// Directory of normalized TSVs (from `tools/etl.py`).
    pub clean_dir: &'a Path,
    /// Open canon.db connection (extension already applied).
    pub conn: &'a Connection,
    /// (book, chapter, verse) -> bible_verses.id
    pub verse_ids: &'a HashMap<(i64, i64, i64), i64>,
}

/// The adapter seam. Implementors declare a source and ingest their TSV.
pub trait CanonAdapter {
    /// The source this adapter contributes.
    fn source(&self) -> SourceRecord;

    /// Ingest normalized data into canon.db. Must be idempotent.
    fn ingest(&self, ctx: &mut IngestContext) -> Result<AdapterStats>;
}

/// Register every adapter's source row. Returns the number registered.
pub fn register_sources(conn: &Connection, adapters: &[Box<dyn CanonAdapter>]) -> Result<usize> {
    for a in adapters {
        a.source().upsert(conn)?;
    }
    Ok(adapters.len())
}

// ── Concrete adapters ────────────────────────────────────────────────────────
// Each is a thin metadata + stub-ingest wrapper. `ingest` counts rows in the
// expected TSV if present; full column mapping lands in a later track.

macro_rules! adapter {
    ($ty:ident, $id:literal, $name:literal, $license:literal, $attr:literal, $url:literal, $tsv:literal) => {
        #[doc = concat!("Adapter for ", $name, ".")]
        pub struct $ty;

        impl CanonAdapter for $ty {
            fn source(&self) -> SourceRecord {
                SourceRecord {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    license_code: $license.to_string(),
                    attribution: $attr.to_string(),
                    url: Some($url.to_string()),
                    version: None,
                }
            }

            fn ingest(&self, ctx: &mut IngestContext) -> Result<AdapterStats> {
                let path = ctx.clean_dir.join($tsv);
                if !path.exists() {
                    return Ok(AdapterStats::default());
                }
                let text = std::fs::read_to_string(&path)?;
                let rows = text.lines().filter(|l| !l.trim().is_empty()).count();
                Ok(AdapterStats { rows })
            }
        }
    };
}

adapter!(
    KjvAdapter,
    "kjv-pd",
    "King James Version",
    "PD",
    "King James Version (public domain in the United States; Crown copyright in the United Kingdom).",
    "https://www.bibleprotector.com/",
    "verses.tsv"
);
adapter!(
    StrongsAdapter,
    "strongs-pd",
    "Strong's Exhaustive Concordance",
    "PD",
    "Strong's Exhaustive Concordance (1890), public domain.",
    "https://www.openscriptures.org/",
    "lexicon.tsv"
);
adapter!(
    StepBibleTagntAdapter,
    "stepbible-tagnt",
    "STEPBible TAGNT",
    "CC-BY-4.0",
    "STEPBible TAGNT (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    "verse_words.tsv"
);
adapter!(
    StepBibleTahotAdapter,
    "stepbible-tahot",
    "STEPBible TAHOT",
    "CC-BY-4.0",
    "STEPBible TAHOT (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    "verse_words.tsv"
);
adapter!(
    StepBibleTbeshAdapter,
    "stepbible-tbesh",
    "STEPBible TBESH",
    "CC-BY-4.0",
    "STEPBible TBESH (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    "lexicon.tsv"
);
adapter!(
    StepBibleTbesgAdapter,
    "stepbible-tbesg",
    "STEPBible TBESG",
    "CC-BY-4.0",
    "STEPBible TBESG (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    "lexicon.tsv"
);
adapter!(
    OpenBibleXrefsAdapter,
    "openbible-xrefs",
    "OpenBible.info Cross References",
    "CC-BY-4.0",
    "OpenBible.info cross-references, CC BY 4.0.",
    "https://www.openbible.info/labs/cross-references/",
    "xrefs.tsv"
);
adapter!(
    NavesTopicalAdapter,
    "naves-topical",
    "Nave's Topical Bible",
    "PD",
    "Nave's Topical Bible (1896), public domain.",
    "https://www.ccel.org/",
    "topics.tsv"
);
adapter!(
    TorreyTopicalAdapter,
    "torrey-topical",
    "Torrey's New Topical Textbook",
    "PD",
    "Torrey's New Topical Textbook (1897), public domain.",
    "https://www.ccel.org/",
    "topics.tsv"
);

/// The default adapter registry, in a stable order.
pub fn default_registry() -> Vec<Box<dyn CanonAdapter>> {
    vec![
        Box::new(KjvAdapter),
        Box::new(StrongsAdapter),
        Box::new(StepBibleTagntAdapter),
        Box::new(StepBibleTahotAdapter),
        Box::new(StepBibleTbeshAdapter),
        Box::new(StepBibleTbesgAdapter),
        Box::new(OpenBibleXrefsAdapter),
        Box::new(NavesTopicalAdapter),
        Box::new(TorreyTopicalAdapter),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::schema_ext::apply_canon_extensions;

    fn fresh_canon() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::schema::CANON_SCHEMA).unwrap();
        apply_canon_extensions(&conn).unwrap();
        conn
    }

    #[test]
    fn every_adapter_declares_a_complete_source() {
        for a in default_registry() {
            let s = a.source();
            assert!(!s.id.is_empty(), "adapter has empty id");
            assert!(!s.name.is_empty(), "adapter {} has empty name", s.id);
            assert!(!s.license_code.is_empty(), "adapter {} has empty license", s.id);
            assert!(!s.attribution.is_empty(), "adapter {} has empty attribution", s.id);
        }
    }

    #[test]
    fn registry_has_nine_sources_with_unique_ids() {
        let reg = default_registry();
        assert_eq!(reg.len(), 9);
        let mut ids: Vec<String> = reg.iter().map(|a| a.source().id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 9, "source ids must be unique");
    }

    #[test]
    fn register_sources_inserts_rows() {
        let conn = fresh_canon();
        let reg = default_registry();
        let n = register_sources(&conn, &reg).unwrap();
        assert_eq!(n, 9);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sources", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 9);
    }

    #[test]
    fn foreign_keys_resolve_to_registered_sources() {
        let conn = fresh_canon();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        let reg = default_registry();
        register_sources(&conn, &reg).unwrap();

        // Seed a verse so verse FKs resolve.
        conn.execute(
            "INSERT INTO bible_books(book_num, osis_id, name, testament, canonical_order)
             VALUES (45, 'Rom', 'Romans', 'NT', 45)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bible_verses(book_num, chapter, verse, text_kjv)
             VALUES (45, 8, 28, 'And we know...')",
            [],
        )
        .unwrap();
        let vid: i64 = conn
            .query_row("SELECT id FROM bible_verses WHERE book_num=45 AND chapter=8 AND verse=28", [], |r| r.get(0))
            .unwrap();

        // topic + topic_verse referencing a registered source.
        conn.execute(
            "INSERT INTO topics(id, name, source_id, source_version) VALUES ('t-love', 'Love', 'naves-topical', '1.0')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topic_verses(topic_id, verse_id, weight, source_id) VALUES ('t-love', ?1, 1.0, 'naves-topical')",
            [vid],
        )
        .unwrap();

        // chain edge referencing a registered source.
        conn.execute(
            "INSERT INTO chain_edges(from_verse, to_verse, kind, weight, source_id, rule_id)
             VALUES (?1, ?1, 'shared-strong', 1.0, 'openbible-xrefs', 'r1')",
            [vid],
        )
        .unwrap();

        // cross_references with source_id.
        conn.execute(
            "INSERT INTO cross_references(from_verse_id, to_verse_id, rank, weight, source_id)
             VALUES (?1, ?1, 1, 1.0, 'openbible-xrefs')",
            [vid],
        )
        .unwrap();

        // All FK targets resolve (no dangling source_id).
        let dangling: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM topics t LEFT JOIN sources s ON t.source_id = s.id WHERE s.id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dangling, 0);
    }
}
