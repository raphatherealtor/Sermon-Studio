//! # canon.db ETL adapters
//!
//! Each upstream dataset is wrapped in a [`CanonAdapter`] that declares its
//! [`SourceRecord`] (id, name, license, attribution, url, version) and knows how
//! to ingest its normalized TSV into canon.db.
//!
//! ## No runtime downloads
//!
//! Adapters read **only** from the local `clean_dir` produced by the ETL
//! (`tools/etl.py`). There is no network access anywhere in this module. Raw
//! acquisition is a separate, offline, human-run build step.
//!
//! ## Normalized TSV contracts (deterministic, tab-separated, LF)
//!
//! * `verses.tsv`       — `book_num \t chapter \t verse \t text`
//! * `lexicon.tsv`      — `strong_id \t testament \t lemma \t transliteration \t
//!                         pronunciation \t part_of_speech \t definition \t gloss \t
//!                         derivation \t usage_note \t source_id \t source_version`
//! * `verse_words.tsv`  — `book_num \t chapter \t verse \t word_order \t surface \t
//!                         strong_id \t morphology \t strongs_extended \t lemma \t
//!                         gloss \t source_id`
//! * `xrefs.tsv`        — `from_b \t from_c \t from_v \t to_b \t to_c \t to_v \t
//!                         rank \t weight \t source_id`
//! * `topics.tsv`       — `topic_id \t name \t source_id \t source_version`
//! * `topic_verses.tsv` — `topic_id \t book_num \t chapter \t verse \t weight \t source_id`
//!
//! The first columns of each row feed the pre-existing base ingestion in
//! [`crate::canon::build_canon_db`]; the trailing columns carry the V1 extension
//! (provenance, morphology, lemma, gloss, topic membership). This keeps the base
//! builder untouched while the adapters populate the additive columns/tables.

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
    /// Directory of normalized TSVs (from the ETL).
    pub clean_dir: &'a Path,
    /// Open canon.db connection (extension already applied).
    pub conn: &'a Connection,
    /// (book, chapter, verse) -> bible_verses.id
    pub verse_ids: &'a HashMap<(i64, i64, i64), i64>,
}

/// The adapter seam. Implementors declare a source and ingest their TSV.
pub trait CanonAdapter {
    fn source(&self) -> SourceRecord;
    fn ingest(&self, ctx: &mut IngestContext) -> Result<AdapterStats>;
}

/// Register every adapter's source row. Returns the number registered.
pub fn register_sources(conn: &Connection, adapters: &[Box<dyn CanonAdapter>]) -> Result<usize> {
    for a in adapters {
        a.source().upsert(conn)?;
    }
    Ok(adapters.len())
}

/// Read the source registry back out of a built canon.db, for the attribution
/// surface. Ordered by source id for determinism.
pub fn read_sources(conn: &Connection) -> Result<Vec<SourceRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, license_code, attribution, url, version FROM sources ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SourceRecord {
            id: r.get(0)?,
            name: r.get(1)?,
            license_code: r.get(2)?,
            attribution: r.get(3)?,
            url: r.get(4)?,
            version: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Run every adapter's `ingest` against `ctx`. Returns total ingested rows.
pub fn ingest_all(ctx: &mut IngestContext, adapters: &[Box<dyn CanonAdapter>]) -> Result<usize> {
    let mut total = 0usize;
    for a in adapters {
        total += a.ingest(ctx)?.rows;
    }
    Ok(total)
}

// ── Shared ingestion helpers ─────────────────────────────────────────────────

/// Read a TSV file into trimmed, non-empty rows.
fn read_tsv(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// Resolve `(book, chapter, verse)` to a `bible_verses.id`. Returns `None` for
/// unknown references (invalid references are rejected, never clamped).
fn resolve_verse(ctx: &IngestContext, b: i64, c: i64, v: i64) -> Option<i64> {
    ctx.verse_ids.get(&(b, c, v)).copied()
}

/// Parse an optional (possibly empty) string column to `Option<String>`.
fn opt(s: Option<&&str>) -> Option<String> {
    match s {
        Some(s) if !s.is_empty() => Some((*s).to_string()),
        _ => None,
    }
}

/// Ingest `topics.tsv` + `topic_verses.tsv` rows belonging to `source_id`.
fn ingest_topics(ctx: &mut IngestContext, source_id: &str) -> Result<AdapterStats> {
    let mut stats = AdapterStats::default();

    let topics_path = ctx.clean_dir.join("topics.tsv");
    if topics_path.exists() {
        let mut stmt = ctx.conn.prepare(
            "INSERT OR REPLACE INTO topics(id, name, source_id, source_version) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for line in read_tsv(&topics_path)? {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 3 || f[2] != source_id {
                continue;
            }
            stmt.execute(params![f[0], f[1], f[2], opt(f.get(3))])?;
            stats.rows += 1;
        }
    }

    let tv_path = ctx.clean_dir.join("topic_verses.tsv");
    if tv_path.exists() {
        let mut stmt = ctx.conn.prepare(
            "INSERT OR REPLACE INTO topic_verses(topic_id, verse_id, weight, source_id) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for line in read_tsv(&tv_path)? {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 6 || f[5] != source_id {
                continue;
            }
            let (b, c, v) = match (
                f[1].parse::<i64>(),
                f[2].parse::<i64>(),
                f[3].parse::<i64>(),
            ) {
                (Ok(b), Ok(c), Ok(v)) => (b, c, v),
                _ => continue, // malformed reference: reject, never clamp
            };
            let Some(verse_id) = resolve_verse(ctx, b, c, v) else {
                continue; // unknown verse: reject
            };
            let weight = f[4].parse::<f64>().unwrap_or(1.0);
            stmt.execute(params![f[0], verse_id, weight, source_id])?;
            stats.rows += 1;
        }
    }

    Ok(stats)
}

/// Enrich `verse_words` rows with morphology/lemma/gloss/extended-Strong's and
/// provenance, keyed by `(verse_id, word_order)`.
fn enrich_verse_words(ctx: &mut IngestContext, source_id: &str) -> Result<AdapterStats> {
    let mut stats = AdapterStats::default();
    let path = ctx.clean_dir.join("verse_words.tsv");
    if !path.exists() {
        return Ok(stats);
    }
    let mut stmt = ctx.conn.prepare(
        "UPDATE verse_words
         SET strongs_extended = ?1, classic_strongs = ?2, morphology_code = ?3,
             lemma = ?4, gloss = ?5, source_id = ?6
         WHERE verse_id = ?7 AND word_order = ?8",
    )?;
    for line in read_tsv(&path)? {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 11 || f.get(10) != Some(&source_id) {
            continue;
        }
        let (b, c, v, order) = match (
            f[0].parse::<i64>(),
            f[1].parse::<i64>(),
            f[2].parse::<i64>(),
            f[3].parse::<i64>(),
        ) {
            (Ok(b), Ok(c), Ok(v), Ok(o)) => (b, c, v, o),
            _ => continue,
        };
        let Some(verse_id) = resolve_verse(ctx, b, c, v) else {
            continue;
        };
        let classic_strongs = opt(Some(&f[5]));
        let morphology_code = opt(Some(&f[6]));
        stmt.execute(params![
            opt(Some(&f[7])),
            classic_strongs,
            morphology_code,
            opt(Some(&f[8])),
            opt(Some(&f[9])),
            source_id,
            verse_id,
            order,
        ])?;
        stats.rows += 1;
    }
    Ok(stats)
}

/// Enrich `strongs_lexicon` rows with provenance and usage notes, keyed by
/// `strong_id`.
fn enrich_lexicon(ctx: &mut IngestContext, source_id: &str) -> Result<AdapterStats> {
    let mut stats = AdapterStats::default();
    let path = ctx.clean_dir.join("lexicon.tsv");
    if !path.exists() {
        return Ok(stats);
    }
    let mut stmt = ctx.conn.prepare(
        "UPDATE strongs_lexicon SET usage_note = ?1, source_id = ?2, source_version = ?3
         WHERE strong_id = ?4",
    )?;
    for line in read_tsv(&path)? {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 10 || f.get(10) != Some(&source_id) {
            continue;
        }
        let strong_id = f[0];
        // Only enrich rows that already exist (malformed Strong's ids are not
        // silently created here).
        let touched = stmt.execute(params![opt(Some(&f[9])), source_id, opt(f.get(11)), strong_id])?;
        stats.rows += touched as usize;
    }
    Ok(stats)
}

/// Enrich `cross_references` rows with weight + provenance, keyed by endpoints.
fn enrich_xrefs(ctx: &mut IngestContext, source_id: &str) -> Result<AdapterStats> {
    let mut stats = AdapterStats::default();
    let path = ctx.clean_dir.join("xrefs.tsv");
    if !path.exists() {
        return Ok(stats);
    }
    let mut stmt = ctx.conn.prepare(
        "UPDATE cross_references SET weight = ?1, source_id = ?2
         WHERE from_verse_id = ?3 AND to_verse_id = ?4",
    )?;
    for line in read_tsv(&path)? {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 || f.get(8) != Some(&source_id) {
            continue;
        }
        let nums: Vec<i64> = f[..6].iter().filter_map(|s| s.parse::<i64>().ok()).collect();
        if nums.len() != 6 {
            continue;
        }
        let from = resolve_verse(ctx, nums[0], nums[1], nums[2]);
        let to = resolve_verse(ctx, nums[3], nums[4], nums[5]);
        let (Some(from_id), Some(to_id)) = (from, to) else {
            continue;
        };
        let weight = f[7].parse::<f64>().unwrap_or(1.0);
        stmt.execute(params![weight, source_id, from_id, to_id])?;
        stats.rows += 1;
    }
    Ok(stats)
}

// ── Concrete adapters ────────────────────────────────────────────────────────
// Each adapter is one complete `CanonAdapter` impl: source metadata + ingest.

macro_rules! adapter {
    ($ty:ident, $id:literal, $name:literal, $license:literal, $attr:literal, $url:literal, $ingest:expr) => {
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
                ($ingest)(ctx)
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
    |_ctx| Ok(AdapterStats::default())
);

adapter!(
    StrongsAdapter,
    "strongs-pd",
    "Strong's Exhaustive Concordance",
    "CC-BY-SA",
    "Strong's Exhaustive Concordance (1890 Greek / 1894 Hebrew) — public domain. Digital transcription: Open Scriptures strongs-greek/hebrew-dictionary.js (CC BY-SA).",
    "https://github.com/openscriptures/strongs",
    |ctx| enrich_lexicon(ctx, "strongs-pd")
);

adapter!(
    StepBibleTagntAdapter,
    "stepbible-tagnt",
    "STEPBible TAGNT",
    "CC-BY-4.0",
    "STEPBible TAGNT (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    |ctx| enrich_verse_words(ctx, "stepbible-tagnt")
);

adapter!(
    StepBibleTahotAdapter,
    "stepbible-tahot",
    "STEPBible TAHOT",
    "CC-BY-4.0",
    "STEPBible TAHOT (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    |ctx| enrich_verse_words(ctx, "stepbible-tahot")
);

adapter!(
    StepBibleTbeshAdapter,
    "stepbible-tbesh",
    "STEPBible TBESH",
    "CC-BY-4.0",
    "STEPBible TBESH (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    |ctx| enrich_lexicon(ctx, "stepbible-tbesh")
);

adapter!(
    StepBibleTbesgAdapter,
    "stepbible-tbesg",
    "STEPBible TBESG",
    "CC-BY-4.0",
    "STEPBible TBESG (Tyndale House, Cambridge), CC BY 4.0.",
    "https://github.com/STEPBible/STEPBible-Data",
    |ctx| enrich_lexicon(ctx, "stepbible-tbesg")
);

adapter!(
    OpenBibleXrefsAdapter,
    "openbible-xrefs",
    "OpenBible.info Cross References",
    "CC-BY-4.0",
    "OpenBible.info cross-references, CC BY 4.0.",
    "https://www.openbible.info/labs/cross-references/",
    |ctx| enrich_xrefs(ctx, "openbible-xrefs")
);

adapter!(
    NavesTopicalAdapter,
    "naves-topical",
    "Nave's Topical Bible",
    "PD",
    "Nave's Topical Bible (1896), public domain.",
    "https://www.ccel.org/",
    |ctx| ingest_topics(ctx, "naves-topical")
);

adapter!(
    TorreyTopicalAdapter,
    "torrey-topical",
    "Torrey's New Topical Textbook",
    "PD",
    "Torrey's New Topical Textbook (1897), public domain.",
    "https://www.ccel.org/",
    |ctx| ingest_topics(ctx, "torrey-topical")
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
        // FK enforcement stays OFF to match `build_canon_db`, which relies on the
        // explicit validate gates rather than SQLite FKs (and so tests can create
        // the orphan rows those gates detect).
        conn.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
        conn.execute_batch(crate::schema::CANON_SCHEMA).unwrap();
        apply_canon_extensions(&conn).unwrap();
        conn
    }

    fn seed_verse(conn: &Connection, b: i64, c: i64, v: i64) -> i64 {
        conn.execute(
            "INSERT OR IGNORE INTO bible_books(book_num, osis_id, name, testament, canonical_order)
             VALUES (?1, ?1, ?1, 'NT', ?1)",
            [b],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bible_verses(book_num, chapter, verse, text_kjv) VALUES (?1, ?2, ?3, 'x')",
            params![b, c, v],
        )
        .unwrap();
        conn.query_row(
            "SELECT id FROM bible_verses WHERE book_num=?1 AND chapter=?2 AND verse=?3",
            params![b, c, v],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn every_adapter_declares_a_complete_source() {
        for a in default_registry() {
            let s = a.source();
            assert!(!s.id.is_empty());
            assert!(!s.name.is_empty());
            assert!(!s.license_code.is_empty());
            assert!(!s.attribution.is_empty());
        }
    }

    #[test]
    fn registry_has_nine_sources_with_unique_ids() {
        let reg = default_registry();
        assert_eq!(reg.len(), 9);
        let mut ids: Vec<String> = reg.iter().map(|a| a.source().id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 9);
    }

    #[test]
    fn topics_ingest_populates_topics_and_topic_verses() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("topics.tsv"),
            "t-love\tLove\tnaves-topical\t1.0\nt-hope\tHope\ttorrey-topical\t2.0\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("topic_verses.tsv"),
            "t-love\t45\t8\t28\t1.0\tnaves-topical\nt-hope\t45\t8\t28\t0.5\ttorrey-topical\n",
        )
        .unwrap();

        let conn = fresh_canon();
        let vid = seed_verse(&conn, 45, 8, 28);
        let mut verse_ids = HashMap::new();
        verse_ids.insert((45, 8, 28), vid);
        let mut ctx = IngestContext {
            clean_dir: tmp.path(),
            conn: &conn,
            verse_ids: &verse_ids,
        };

        let naves = NavesTopicalAdapter;
        let stats = naves.ingest(&mut ctx).unwrap();
        assert_eq!(stats.rows, 2); // 1 topic + 1 topic_verse

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM topics", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        let tv: i64 = conn.query_row("SELECT COUNT(*) FROM topic_verses", [], |r| r.get(0)).unwrap();
        assert_eq!(tv, 1);
        // Torrey's rows were NOT ingested by the Nave's adapter.
        let torrey: i64 = conn
            .query_row("SELECT COUNT(*) FROM topics WHERE source_id='torrey-topical'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(torrey, 0);
    }

    #[test]
    fn topic_verse_with_unknown_reference_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("topics.tsv"), "t-x\tX\tnaves-topical\t1.0\n").unwrap();
        // Points at a verse that does not exist in bible_verses.
        std::fs::write(
            tmp.path().join("topic_verses.tsv"),
            "t-x\t45\t99\t99\t1.0\tnaves-topical\n",
        )
        .unwrap();

        let conn = fresh_canon();
        let mut verse_ids = HashMap::new();
        let mut ctx = IngestContext {
            clean_dir: tmp.path(),
            conn: &conn,
            verse_ids: &verse_ids,
        };
        let stats = NavesTopicalAdapter.ingest(&mut ctx).unwrap();
        // Topic row ingests; the dangling topic_verse is dropped.
        assert_eq!(stats.rows, 1);
        let tv: i64 = conn.query_row("SELECT COUNT(*) FROM topic_verses", [], |r| r.get(0)).unwrap();
        assert_eq!(tv, 0);
    }

    #[test]
    fn verse_words_enrichment_populates_morphology_and_provenance() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("verse_words.tsv"),
            "45\t8\t28\t0\tAnd\tG1161\tCONJ\tG1161\tδε\tbut, and\tstepbible-tagnt\n",
        )
        .unwrap();

        let conn = fresh_canon();
        let vid = seed_verse(&conn, 45, 8, 28);
        conn.execute(
            "INSERT INTO verse_words(verse_id, word_order, surface_word, strong_id, morphology)
             VALUES (?1, 0, 'And', 'G1161', NULL)",
            [vid],
        )
        .unwrap();

        let mut verse_ids = HashMap::new();
        verse_ids.insert((45, 8, 28), vid);
        let mut ctx = IngestContext {
            clean_dir: tmp.path(),
            conn: &conn,
            verse_ids: &verse_ids,
        };
        let stats = StepBibleTagntAdapter.ingest(&mut ctx).unwrap();
        assert_eq!(stats.rows, 1);

        let (ext, lemma, gloss, src): (String, String, String, String) = conn
            .query_row(
                "SELECT strongs_extended, lemma, gloss, source_id FROM verse_words WHERE verse_id=?1",
                [vid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(ext, "G1161");
        assert_eq!(lemma, "δε");
        assert_eq!(gloss, "but, and");
        assert_eq!(src, "stepbible-tagnt");
    }

    #[test]
    fn xref_enrichment_populates_weight_and_source() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("xrefs.tsv"), "45\t8\t28\t45\t8\t29\t50\t0.75\topenbible-xrefs\n").unwrap();

        let conn = fresh_canon();
        let from = seed_verse(&conn, 45, 8, 28);
        let to = seed_verse(&conn, 45, 8, 29);
        conn.execute(
            "INSERT INTO cross_references(from_verse_id, to_verse_id, rank) VALUES (?1, ?2, 50)",
            params![from, to],
        )
        .unwrap();

        let mut verse_ids = HashMap::new();
        verse_ids.insert((45, 8, 28), from);
        verse_ids.insert((45, 8, 29), to);
        let mut ctx = IngestContext {
            clean_dir: tmp.path(),
            conn: &conn,
            verse_ids: &verse_ids,
        };
        let stats = OpenBibleXrefsAdapter.ingest(&mut ctx).unwrap();
        assert_eq!(stats.rows, 1);

        let (weight, src): (f64, String) = conn
            .query_row(
                "SELECT weight, source_id FROM cross_references WHERE from_verse_id=?1 AND to_verse_id=?2",
                params![from, to],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(weight, 0.75);
        assert_eq!(src, "openbible-xrefs");
    }
}
