//! canon.db builder.
//!
//! Ingests the normalized TSVs produced by `tools/etl.py` into the static,
//! read-only canon vault:
//!   * verses.tsv       — KJV verse text
//!   * verse_words.tsv  — Strong's-tagged KJV words (interlinear)
//!   * lexicon.tsv      — Strong's Greek/Hebrew lexicon
//!   * xrefs.tsv        — cross references (OpenBible, CC-BY)
//!
//! The builder is idempotent: it recreates the canon tables from scratch, so it
//! can be re-run at any time from the raw sources.
//!
//! ## V1 extension (additive)
//!
//! This file is **extended**, not rewritten, by the V1 scaffold:
//!   * [`schema_ext`] adds the `sources`/`topics`/`topic_verses`/`chain_edges`/
//!     `canon_meta` tables and the new columns on existing tables.
//!   * [`adapters`] declares the source registry and the per-dataset adapters.
//!   * a [`schema_ext::CanonManifest`] is written into `canon_meta` at the end.
//!
//! The original ingestion logic below is unchanged.

pub mod adapters;
pub mod schema_ext;
pub mod validate;

use crate::books::BOOKS;
use crate::error::Result;
use crate::schema::CANON_SCHEMA;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Canon schema/data version. Bump when the canon data or schema changes.
pub const CANON_VERSION: &str = "2025.01";

/// Build canon.db at `out_path` from the clean data directory `clean_dir`.
pub fn build_canon_db(clean_dir: &Path, out_path: &Path) -> Result<CanonStats> {
    if out_path.exists() {
        std::fs::remove_file(out_path)?;
    }
    let mut conn = Connection::open(out_path)?;
    conn.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA cache_size=-64000;")?;
    conn.execute_batch(CANON_SCHEMA)?;

    // V1: apply the additive extension (new tables + columns) before ingesting.
    schema_ext::apply_canon_extensions(&conn)?;

    let mut stats = CanonStats::default();

    // 1. Books.
    {
        let tx = conn.transaction()?;
        for (num, osis, name, testament) in BOOKS {
            tx.execute(
                "INSERT INTO bible_books(book_num, osis_id, name, testament, canonical_order)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![num, osis, name, testament, num],
            )?;
        }
        tx.commit()?;
        stats.books = BOOKS.len();
    }

    // 2. Verses.
    let verses_tsv = clean_dir.join("verses.tsv");
    let mut verse_ids: HashMap<(i64, i64, i64), i64> = HashMap::new();
    if verses_tsv.exists() {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO bible_verses(book_num, chapter, verse, text_kjv)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for line in read_lines(&verses_tsv)? {
                let f: Vec<&str> = line.splitn(4, '\t').collect();
                if f.len() < 4 {
                    continue;
                }
                let b: i64 = f[0].parse().unwrap_or(0);
                let c: i64 = f[1].parse().unwrap_or(0);
                let v: i64 = f[2].parse().unwrap_or(0);
                if b == 0 || c == 0 || v == 0 {
                    continue;
                }
                stmt.execute(params![b, c, v, f[3]])?;
                stats.verses += 1;
            }
        }
        tx.commit()?;
    }

    // Load verse id map.
    {
        let mut stmt = conn.prepare("SELECT id, book_num, chapter, verse FROM bible_verses")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        })?;
        for row in rows {
            let (id, b, c, v) = row?;
            verse_ids.insert((b, c, v), id);
        }
    }

    // 3. Lexicon.
    let lex_tsv = clean_dir.join("lexicon.tsv");
    if lex_tsv.exists() {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO strongs_lexicon
                 (strong_id, testament, lemma, transliteration, pronunciation, part_of_speech, definition, gloss, derivation)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            )?;
            for line in read_lines(&lex_tsv)? {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 9 {
                    continue;
                }
                stmt.execute(params![
                    f[0], f[1], f[2], f[3],
                    opt(f[4]), opt(f[5]), f[6], f[7], opt(f[8])
                ])?;
                stats.lexicon += 1;
            }
        }
        tx.commit()?;
    }

    // 4. verse_words.
    let vw_tsv = clean_dir.join("verse_words.tsv");
    if vw_tsv.exists() {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO verse_words(verse_id, word_order, surface_word, strong_id, morphology)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for line in read_lines(&vw_tsv)? {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 6 {
                    continue;
                }
                let b: i64 = f[0].parse().unwrap_or(0);
                let c: i64 = f[1].parse().unwrap_or(0);
                let v: i64 = f[2].parse().unwrap_or(0);
                let vid = match verse_ids.get(&(b, c, v)) {
                    Some(id) => *id,
                    None => continue,
                };
                let order: i64 = f[3].parse().unwrap_or(0);
                let strong = if f[5].is_empty() { None } else { Some(f[5]) };
                stmt.execute(params![vid, order, f[4], strong, opt(f.get(6).copied().unwrap_or(""))])?;
                stats.verse_words += 1;
            }
        }
        tx.commit()?;
    }

    // 5. Cross references.
    let xref_tsv = clean_dir.join("xrefs.tsv");
    if xref_tsv.exists() {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO cross_references(from_verse_id, to_verse_id, rank)
                 VALUES (?1, ?2, ?3)",
            )?;
            for line in read_lines(&xref_tsv)? {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 7 {
                    continue;
                }
                let fb: i64 = f[0].parse().unwrap_or(0);
                let fc: i64 = f[1].parse().unwrap_or(0);
                let fv: i64 = f[2].parse().unwrap_or(0);
                let tb: i64 = f[3].parse().unwrap_or(0);
                let tc: i64 = f[4].parse().unwrap_or(0);
                let tv: i64 = f[5].parse().unwrap_or(0);
                let rank: i64 = f[6].parse().unwrap_or(1);
                let from_id = match verse_ids.get(&(fb, fc, fv)) {
                    Some(id) => *id,
                    None => continue,
                };
                let to_id = match verse_ids.get(&(tb, tc, tv)) {
                    Some(id) => *id,
                    None => continue,
                };
                stmt.execute(params![from_id, to_id, rank])?;
                stats.cross_references += 1;
            }
        }
        tx.commit()?;
    }

    // 6. Indexes + analyze.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_bible_lookup ON bible_verses(book_num, chapter, verse);
         CREATE INDEX IF NOT EXISTS idx_verse_words_strong ON verse_words(strong_id);
         CREATE INDEX IF NOT EXISTS idx_verse_words_verse ON verse_words(verse_id);
         CREATE INDEX IF NOT EXISTS idx_xref_from ON cross_references(from_verse_id);
         CREATE INDEX IF NOT EXISTS idx_xref_to ON cross_references(to_verse_id);
         ANALYZE;",
    )?;

    // 7. V1: register sources, run dataset adapters (topics + enrichment), and
    // write the version manifest.
    let registry = adapters::default_registry();
    adapters::register_sources(&conn, &registry)?;
    {
        let mut ctx = adapters::IngestContext {
            clean_dir,
            conn: &conn,
            verse_ids: &verse_ids,
        };
        adapters::ingest_all(&mut ctx, &registry)?;
    }
    let manifest = build_manifest(clean_dir, &registry);
    manifest.write(&conn)?;

    Ok(stats)
}

/// Build the canon version manifest from the adapter registry and the clean dir.
fn build_manifest(clean_dir: &Path, registry: &[Box<dyn adapters::CanonAdapter>]) -> schema_ext::CanonManifest {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;

    let mut source_versions = BTreeMap::new();
    let mut source_checksums = BTreeMap::new();
    for a in registry {
        let s = a.source();
        source_versions.insert(s.id.clone(), s.version.clone().unwrap_or_else(|| "unversioned".to_string()));
        // Checksum the TSV this adapter reads, if present.
        let tsv = match s.id.as_str() {
            "kjv-pd" => Some("verses.tsv"),
            "strongs-pd" | "stepbible-tbesh" | "stepbible-tbesg" => Some("lexicon.tsv"),
            "stepbible-tagnt" | "stepbible-tahot" => Some("verse_words.tsv"),
            "openbible-xrefs" => Some("xrefs.tsv"),
            "naves-topical" | "torrey-topical" => Some("topics.tsv"),
            _ => None,
        };
        if let Some(name) = tsv {
            let path = clean_dir.join(name);
            if let Ok(bytes) = std::fs::read(&path) {
                let mut h = Sha256::new();
                h.update(&bytes);
                source_checksums.insert(s.id.clone(), format!("{:x}", h.finalize()));
            }
        }
    }

    schema_ext::CanonManifest {
        canon_version: CANON_VERSION.to_string(),
        build_timestamp: reproducible_build_timestamp(),
        source_versions,
        source_checksums,
    }
}

/// Reproducible build timestamp for the canon manifest.
///
/// Honors the [`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/specs/source-date-epoch/)
/// convention: when the environment variable is set to a Unix timestamp (in
/// seconds), that instant is used. Otherwise a fixed, documented epoch
/// (`1970-01-01T00:00:00Z`) is used, so that identical normalized inputs always
/// produce byte-identical manifests regardless of wall-clock time.
fn reproducible_build_timestamp() -> String {
    reproducible_timestamp_from(std::env::var("SOURCE_DATE_EPOCH").ok().as_deref())
}

/// Pure helper behind [`reproducible_build_timestamp`], split out so the rule
/// can be tested without mutating process-global environment state.
///
/// Rule: if `source_date_epoch` parses as an integer number of Unix seconds, the
/// timestamp is that instant rendered as RFC 3339 (UTC, second precision, `Z`).
/// Otherwise the fixed fallback `1970-01-01T00:00:00Z` is returned.
fn reproducible_timestamp_from(source_date_epoch: Option<&str>) -> String {
    if let Some(raw) = source_date_epoch {
        if let Ok(secs) = raw.trim().parse::<i64>() {
            if let Some(dt) = chrono::TimeZone::timestamp_opt(&chrono::Utc, secs, 0).single() {
                return dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            }
        }
    }
    "1970-01-01T00:00:00Z".to_string()
}

fn opt(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn read_lines(path: &Path) -> Result<Vec<String>> {
    let f = std::fs::File::open(path)?;
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for line in reader.lines() {
        out.push(line?);
    }
    Ok(out)
}

#[derive(Debug, Default, Clone)]
pub struct CanonStats {
    pub books: usize,
    pub verses: usize,
    pub lexicon: usize,
    pub verse_words: usize,
    pub cross_references: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_minimal_tsvs() {
        let tmp = tempfile::tempdir().unwrap();
        let clean = tmp.path();
        std::fs::write(clean.join("verses.tsv"), "45\t8\t28\tAnd we know that all things work together for good.\n45\t8\t29\tFor whom he did foreknow, he also did predestinate.\n").unwrap();
        std::fs::write(clean.join("lexicon.tsv"), "G26\tNT\t\u{1f00}\u{03b3}\u{03ac}\u{03c0}\u{03b7}\tagape\tag-ah'-pay\t\tlove\tlove\tfrom G25\n").unwrap();
        std::fs::write(clean.join("verse_words.tsv"), "45\t8\t28\t0\tAnd\t\t\n45\t8\t28\t1\twe\t\t\n").unwrap();
        std::fs::write(clean.join("xrefs.tsv"), "45\t8\t28\t45\t8\t29\t50\n").unwrap();
        let db = tmp.path().join("canon.db");
        let stats = build_canon_db(clean, &db).unwrap();
        assert_eq!(stats.books, 66);
        assert_eq!(stats.verses, 2);
        assert_eq!(stats.lexicon, 1);
        assert_eq!(stats.verse_words, 2);
        assert_eq!(stats.cross_references, 1);
    }

    #[test]
    fn build_writes_manifest_and_registers_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let clean = tmp.path();
        std::fs::write(clean.join("verses.tsv"), "45\t8\t28\tAnd we know...\n").unwrap();
        let db = tmp.path().join("canon.db");
        build_canon_db(clean, &db).unwrap();

        let conn = Connection::open(&db).unwrap();
        let sources: i64 = conn.query_row("SELECT COUNT(*) FROM sources", [], |r| r.get(0)).unwrap();
        assert_eq!(sources, 9);
        let manifest = schema_ext::CanonManifest::read(&conn).unwrap().unwrap();
        assert_eq!(manifest.canon_version, CANON_VERSION);
        assert!(manifest.source_versions.contains_key("openbible-xrefs"));
        // Checksum recorded for the verses.tsv we wrote.
        assert!(manifest.source_checksums.contains_key("kjv-pd"));
    }

    #[test]
    fn rebuild_is_deterministic_in_content() {
        let tmp = tempfile::tempdir().unwrap();
        let clean = tmp.path();
        std::fs::write(clean.join("verses.tsv"), "45\t8\t28\tAnd we know...\n45\t8\t29\tFor whom...\n").unwrap();
        std::fs::write(clean.join("xrefs.tsv"), "45\t8\t28\t45\t8\t29\t50\n").unwrap();

        let db1 = tmp.path().join("a.db");
        let db2 = tmp.path().join("b.db");
        let s1 = build_canon_db(clean, &db1).unwrap();
        let s2 = build_canon_db(clean, &db2).unwrap();
        assert_eq!(s1.verses, s2.verses);
        assert_eq!(s1.cross_references, s2.cross_references);

        // Content-level determinism: same verse text and same xref set.
        let c1 = Connection::open(&db1).unwrap();
        let c2 = Connection::open(&db2).unwrap();
        let t1: String = c1.query_row("SELECT text_kjv FROM bible_verses WHERE book_num=45 AND chapter=8 AND verse=28", [], |r| r.get(0)).unwrap();
        let t2: String = c2.query_row("SELECT text_kjv FROM bible_verses WHERE book_num=45 AND chapter=8 AND verse=28", [], |r| r.get(0)).unwrap();
        assert_eq!(t1, t2);
        let x1: i64 = c1.query_row("SELECT COUNT(*) FROM cross_references", [], |r| r.get(0)).unwrap();
        let x2: i64 = c2.query_row("SELECT COUNT(*) FROM cross_references", [], |r| r.get(0)).unwrap();
        assert_eq!(x1, x2);
    }

    #[test]
    fn manifest_build_timestamp_is_reproducible() {
        // Two builds from identical normalized input must produce identical
        // manifest semantic content, including the build timestamp.
        let tmp = tempfile::tempdir().unwrap();
        let clean = tmp.path();
        std::fs::write(clean.join("verses.tsv"), "45\t8\t28\tAnd we know...\n").unwrap();
        std::fs::write(clean.join("xrefs.tsv"), "45\t8\t28\t45\t8\t29\t50\n").unwrap();

        let db1 = tmp.path().join("a.db");
        let db2 = tmp.path().join("b.db");
        build_canon_db(clean, &db1).unwrap();
        build_canon_db(clean, &db2).unwrap();

        let c1 = Connection::open(&db1).unwrap();
        let c2 = Connection::open(&db2).unwrap();
        let m1 = schema_ext::CanonManifest::read(&c1).unwrap().unwrap();
        let m2 = schema_ext::CanonManifest::read(&c2).unwrap().unwrap();
        assert_eq!(m1.build_timestamp, m2.build_timestamp);
        assert_eq!(m1, m2);
    }

    #[test]
    fn reproducible_timestamp_rule_is_deterministic() {
        // SOURCE_DATE_EPOCH is honored when supplied (Unix seconds -> RFC 3339 UTC).
        assert_eq!(reproducible_timestamp_from(Some("0")), "1970-01-01T00:00:00Z");
        assert_eq!(reproducible_timestamp_from(Some("1735689600")), "2025-01-01T00:00:00Z");
        // Surrounding whitespace is tolerated.
        assert_eq!(reproducible_timestamp_from(Some(" 1735689600 ")), "2025-01-01T00:00:00Z");
        // A fixed, documented fallback is used when absent or unparseable.
        assert_eq!(reproducible_timestamp_from(None), "1970-01-01T00:00:00Z");
        assert_eq!(reproducible_timestamp_from(Some("not-a-number")), "1970-01-01T00:00:00Z");
        assert_eq!(reproducible_timestamp_from(Some("")), "1970-01-01T00:00:00Z");
    }

    /// End-to-end: build a full canon.db (verses + lexicon + verse_words + xrefs
    /// + topics, with provenance columns), then validate and exercise the
    /// read-only study lookups the packaged app depends on.
    #[test]
    fn builds_and_serves_a_full_study_vault() {
        let tmp = tempfile::tempdir().unwrap();
        let clean = tmp.path();
        std::fs::write(
            clean.join("verses.tsv"),
            "43\t3\t16\tFor God so loved the world.\n45\t8\t28\tAnd we know that all things work together for good.\n45\t8\t29\tFor whom he did foreknow.\n",
        )
        .unwrap();
        std::fs::write(
            clean.join("lexicon.tsv"),
            "G26\tNT\tagape\tagape\tag-ah'-pay\tn f\tlove\tlove\tfrom G25\t\tstrongs-pd\t1890\n",
        )
        .unwrap();
        std::fs::write(
            clean.join("verse_words.tsv"),
            "45\t8\t28\t0\tlove\tG26\tN-NSF\tG26\tagape\tlove\tstepbible-tagnt\n",
        )
        .unwrap();
        std::fs::write(
            clean.join("xrefs.tsv"),
            "43\t3\t16\t45\t8\t28\t50\t0.5\topenbible-xrefs\n",
        )
        .unwrap();
        std::fs::write(
            clean.join("topics.tsv"),
            "naves-topical::love\tLove\tnaves-topical\t1896\n",
        )
        .unwrap();
        std::fs::write(
            clean.join("topic_verses.tsv"),
            "naves-topical::love\t45\t8\t28\t1.0\tnaves-topical\n",
        )
        .unwrap();

        let db = tmp.path().join("canon.db");
        let stats = build_canon_db(clean, &db).unwrap();
        assert_eq!(stats.books, 66);
        assert_eq!(stats.verses, 3);
        assert_eq!(stats.lexicon, 1);
        assert_eq!(stats.verse_words, 1);
        assert_eq!(stats.cross_references, 1);

        // Quality gates: no errors (all FK targets and provenance resolve).
        let conn = crate::open_canon_readonly(&db).unwrap();
        let report = validate::validate_canon(&conn).unwrap();
        assert!(report.is_ok(), "validation errors: {:?}", report.errors);
        assert_eq!(report.topic_count, 1);
        assert_eq!(report.topic_verse_count, 1);

        // 1. Passage lookup.
        let text: String = conn
            .query_row(
                "SELECT text_kjv FROM bible_verses WHERE book_num=43 AND chapter=3 AND verse=16",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(text, "For God so loved the world.");

        // 2. Strong's lookup (lemma + gloss + provenance).
        let (lemma, gloss, source_id): (String, String, String) = conn
            .query_row(
                "SELECT lemma, gloss, source_id FROM strongs_lexicon WHERE strong_id='G26'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(lemma, "agape");
        assert_eq!(gloss, "love");
        assert_eq!(source_id, "strongs-pd");

        // 3. Cross-reference lookup (weight + provenance).
        let xref: (f64, String) = conn
            .query_row(
                "SELECT weight, source_id FROM cross_references",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(xref, (0.5, "openbible-xrefs".to_string()));

        // 4. Topic membership.
        let topic_name: String = conn
            .query_row(
                "SELECT name FROM topics WHERE id='naves-topical::love'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(topic_name, "Love");

        // 5. Morphology enrichment landed on verse_words.
        let (morph, lemma, src): (String, String, String) = conn
            .query_row(
                "SELECT morphology_code, lemma, source_id FROM verse_words",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(morph, "N-NSF");
        assert_eq!(lemma, "agape");
        assert_eq!(src, "stepbible-tagnt");

        // 6. Attribution registry is readable.
        let sources = adapters::read_sources(&conn).unwrap();
        assert_eq!(sources.len(), 9);
        assert!(sources.iter().any(|s| s.id == "naves-topical" && s.license_code == "PD"));
    }
}
