//! # canon.db data-quality gates
//!
//! Deterministic structural validation of a freshly built canon.db. Runs after
//! [`crate::canon::build_canon_db`] and reports hard errors (referential or
//! identity violations) and soft warnings (missing optional enrichment).
//!
//! These gates never mutate the database and make no network calls.

use crate::error::Result;
use rusqlite::Connection;

/// Result of running the quality gates against a canon.db connection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub book_count: i64,
    pub verse_count: i64,
    pub lexicon_count: i64,
    pub verse_word_count: i64,
    pub xref_count: i64,
    pub topic_count: i64,
    pub topic_verse_count: i64,
    pub source_count: i64,
    /// Hard failures (identity/referential violations).
    pub errors: Vec<String>,
    /// Soft findings (e.g. optional enrichment not present).
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

fn count(conn: &Connection, table: &str) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
}

fn count_null_fk(conn: &Connection, sql: &str) -> Result<i64> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

/// Run every gate against `conn` and return the report.
pub fn validate_canon(conn: &Connection) -> Result<ValidationReport> {
    let mut report = ValidationReport {
        book_count: count(conn, "bible_books")?,
        verse_count: count(conn, "bible_verses")?,
        lexicon_count: count(conn, "strongs_lexicon")?,
        verse_word_count: count(conn, "verse_words")?,
        xref_count: count(conn, "cross_references")?,
        topic_count: count(conn, "topics")?,
        topic_verse_count: count(conn, "topic_verses")?,
        source_count: count(conn, "sources")?,
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    // 1. All 66 canonical books registered.
    if report.book_count != 66 {
        report.errors.push(format!(
            "expected 66 canonical books, found {}",
            report.book_count
        ));
    }

    // 2. No duplicate canonical verse identity (book, chapter, verse).
    let distinct_verses: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM (SELECT DISTINCT book_num, chapter, verse FROM bible_verses)",
    )?;
    if distinct_verses != report.verse_count {
        report.errors.push(format!(
            "duplicate verse identity: {} rows vs {} distinct (book, chapter, verse)",
            report.verse_count, distinct_verses
        ));
    }

    // 3. No malformed Strong's ids silently accepted.
    let malformed: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM strongs_lexicon WHERE strong_id NOT GLOB '[GH][0-9]*'",
    )?;
    if malformed > 0 {
        report
            .errors
            .push(format!("{malformed} malformed Strong's ids in strongs_lexicon"));
    }

    // 4. verse_words -> verse FK integrity.
    let dangling_vw: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM verse_words vw LEFT JOIN bible_verses bv ON vw.verse_id = bv.id WHERE bv.id IS NULL",
    )?;
    if dangling_vw > 0 {
        report
            .errors
            .push(format!("{dangling_vw} verse_words rows reference a missing verse"));
    }

    // 5. cross_references endpoint integrity.
    let dangling_xr: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM cross_references cr
         LEFT JOIN bible_verses f ON cr.from_verse_id = f.id
         LEFT JOIN bible_verses t ON cr.to_verse_id = t.id
         WHERE f.id IS NULL OR t.id IS NULL",
    )?;
    if dangling_xr > 0 {
        report
            .errors
            .push(format!("{dangling_xr} cross_references rows have a missing endpoint"));
    }

    // 6. topic_verse -> verse FK integrity.
    let dangling_tv: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM topic_verses tv LEFT JOIN bible_verses bv ON tv.verse_id = bv.id WHERE bv.id IS NULL",
    )?;
    if dangling_tv > 0 {
        report
            .errors
            .push(format!("{dangling_tv} topic_verses rows reference a missing verse"));
    }

    // 7. No orphan source_id across provenance-bearing tables.
    for (table, col) in [
        ("topics", "source_id"),
        ("topic_verses", "source_id"),
        ("chain_edges", "source_id"),
        ("cross_references", "source_id"),
    ] {
        let orphan: i64 = count_null_fk(
            conn,
            &format!(
                "SELECT COUNT(*) FROM {table} t LEFT JOIN sources s ON t.{col} = s.id WHERE s.id IS NULL"
            ),
        )?;
        if orphan > 0 {
            report
                .errors
                .push(format!("{orphan} orphan source_id in {table}.{col}"));
        }
    }

    // 8. Strong's reference integrity (only when a lexicon is present).
    if report.lexicon_count > 0 {
        let dangling_strong: i64 = count_null_fk(
            conn,
            "SELECT COUNT(*) FROM verse_words vw
             WHERE vw.strong_id IS NOT NULL
               AND vw.strong_id NOT IN (SELECT strong_id FROM strongs_lexicon)",
        )?;
        if dangling_strong > 0 {
            report
                .warnings
                .push(format!("{dangling_strong} verse_words strong_ids lack a lexicon entry"));
        }
    }

    // 9. Soft: optional enrichment present?
    let enriched_vw: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM verse_words WHERE source_id IS NOT NULL",
    )?;
    if report.verse_word_count > 0 && enriched_vw == 0 {
        report
            .warnings
            .push("verse_words carries no source provenance (morphology enrichment missing)".into());
    }
    let enriched_xr: i64 = count_null_fk(
        conn,
        "SELECT COUNT(*) FROM cross_references WHERE source_id IS NOT NULL",
    )?;
    if report.xref_count > 0 && enriched_xr == 0 {
        report
            .warnings
            .push("cross_references carries no source provenance".into());
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::schema_ext::apply_canon_extensions;

    fn fresh_canon() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // FK enforcement OFF to match build_canon_db: the validate gates are the
        // correctness mechanism, and tests must be able to create orphan rows.
        conn.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
        conn.execute_batch(crate::schema::CANON_SCHEMA).unwrap();
        apply_canon_extensions(&conn).unwrap();
        conn
    }

    #[test]
    fn validates_a_minimal_healthy_canon() {
        let conn = fresh_canon();
        // Books + one verse + lexicon + verse_word + xref + topic, all consistent.
        conn.execute(
            "INSERT INTO bible_books(book_num, osis_id, name, testament, canonical_order) VALUES (45,'Rom','Romans','NT',45)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bible_verses(book_num, chapter, verse, text_kjv) VALUES (45,8,28,'x')",
            [],
        )
        .unwrap();
        let vid: i64 = conn
            .query_row("SELECT id FROM bible_verses", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO strongs_lexicon(strong_id, testament, lemma, transliteration, definition, gloss) VALUES ('G26','NT','agape','agape','love','love')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO verse_words(verse_id, word_order, surface_word, strong_id) VALUES (?1, 0, 'love', 'G26')",
            [vid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sources(id, name, license_code, attribution) VALUES ('openbible-xrefs','OpenBible','CC-BY-4.0','x')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cross_references(from_verse_id, to_verse_id, rank, source_id) VALUES (?1, ?1, 1, 'openbible-xrefs')",
            [vid],
        )
        .unwrap();

        let report = validate_canon(&conn).unwrap();
        // Books gate is the only expected error here (we seeded 1, not 66).
        assert!(report.errors.iter().any(|e| e.contains("66")));
        assert!(!report.errors.iter().any(|e| e.contains("verse_words") || e.contains("cross_references")));
        assert_eq!(report.verse_word_count, 1);
        assert_eq!(report.xref_count, 1);
    }

    #[test]
    fn detects_dangling_topic_verse_and_orphan_source() {
        let conn = fresh_canon();
        conn.execute(
            "INSERT INTO bible_books(book_num, osis_id, name, testament, canonical_order) VALUES (45,'Rom','Romans','NT',45)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bible_verses(book_num, chapter, verse, text_kjv) VALUES (45,8,28,'x')",
            [],
        )
        .unwrap();
        let vid: i64 = conn
            .query_row("SELECT id FROM bible_verses", [], |r| r.get(0))
            .unwrap();
        // topic_verses referencing a verse that doesn't exist (vid + 999).
        conn.execute(
            "INSERT INTO topics(id, name, source_id) VALUES ('t-x','X','missing-source')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topic_verses(topic_id, verse_id, source_id) VALUES ('t-x', ?1, 'missing-source')",
            [vid + 999],
        )
        .unwrap();

        let report = validate_canon(&conn).unwrap();
        assert!(report.errors.iter().any(|e| e.contains("topic_verses")));
        assert!(report.errors.iter().any(|e| e.contains("orphan source_id")));
    }

    #[test]
    fn detects_malformed_strongs_id() {
        let conn = fresh_canon();
        conn.execute(
            "INSERT INTO strongs_lexicon(strong_id, testament, lemma, transliteration, definition, gloss) VALUES ('XYZ','NT','x','x','x','x')",
            [],
        )
        .unwrap();
        let report = validate_canon(&conn).unwrap();
        assert!(report.errors.iter().any(|e| e.contains("malformed Strong's")));
    }
}
