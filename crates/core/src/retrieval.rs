//! Retrieval engine: FTS5 search, scripture→sermon graph, lexicon lookup,
//! cross-reference traversal, and illustration-fatigue reporting.

use crate::error::Result;
use crate::reference::{self, VerseRef};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SermonHit {
    pub id: String,
    pub title: String,
    pub primary_passage: String,
    pub big_idea: String,
    pub date_preached: Option<String>,
    pub series: Option<String>,
    pub liturgical_season: Option<String>,
    pub structure_type: String,
    pub file_path: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerseRow {
    pub book_num: i64,
    pub book_name: String,
    pub chapter: i64,
    pub verse: i64,
    pub text_kjv: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconEntry {
    pub strong_id: String,
    pub testament: String,
    pub lemma: String,
    pub transliteration: String,
    pub pronunciation: Option<String>,
    pub part_of_speech: Option<String>,
    pub definition: String,
    pub gloss: String,
    pub derivation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordRow {
    pub word_order: i64,
    pub surface_word: String,
    pub strong_id: Option<String>,
    pub morphology: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IllustrationFatigue {
    pub illustration_key: String,
    pub label: String,
    pub total_uses: i64,
    pub sermon_count: i64,
    pub last_used: Option<String>,
}

/// Full-text search across the sermon archive.
pub fn search_sermons(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SermonHit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // FTS columns: 0=sermon_id(unindexed) 1=title 2=primary_passage 3=big_idea 4=body_content.
    let sql = "
        SELECT s.id, s.title, s.primary_passage, s.big_idea, s.date_preached, s.series,
               s.liturgical_season, s.structure_type, s.file_path,
               snippet(sermons_fts, 4, '<<', '>>', ' … ', 12) AS snip,
               bm25(sermons_fts, 0.0, 10.0, 5.0, 3.0, 1.0) AS score
        FROM sermons_fts
        JOIN sermon_index s ON s.id = sermons_fts.sermon_id
        WHERE sermons_fts MATCH ?1
        ORDER BY score
        LIMIT ?2";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![q, limit], |r| {
        Ok(SermonHit {
            id: r.get(0)?,
            title: r.get(1)?,
            primary_passage: r.get(2)?,
            big_idea: r.get(3)?,
            date_preached: r.get(4)?,
            series: r.get(5)?,
            liturgical_season: r.get(6)?,
            structure_type: r.get(7)?,
            file_path: r.get(8)?,
            snippet: r.get(9)?,
            score: r.get(10)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Fetch a single verse's KJV text.
pub fn get_verse(conn: &Connection, book_num: i64, chapter: i64, verse: i64) -> Result<Option<VerseRow>> {
    let mut stmt = conn.prepare(
        "SELECT v.book_num, b.name, v.chapter, v.verse, v.text_kjv
         FROM bible_verses v JOIN bible_books b ON b.book_num = v.book_num
         WHERE v.book_num=?1 AND v.chapter=?2 AND v.verse=?3",
    )?;
    let mut rows = stmt.query_map(params![book_num, chapter, verse], |r| {
        Ok(VerseRow {
            book_num: r.get(0)?,
            book_name: r.get(1)?,
            chapter: r.get(2)?,
            verse: r.get(3)?,
            text_kjv: r.get(4)?,
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Fetch a passage (inclusive range) of KJV verses.
pub fn get_passage(conn: &Connection, start: VerseRef, end: VerseRef) -> Result<Vec<VerseRow>> {
    let mut out = Vec::new();
    for v in (reference::PassageRef { start, end }).expand() {
        if let Some(row) = get_verse(conn, v.book_num, v.chapter, v.verse)? {
            out.push(row);
        }
    }
    Ok(out)
}

/// Look up a Strong's lexicon entry.
pub fn get_lexicon(conn: &Connection, strong_id: &str) -> Result<Option<LexiconEntry>> {
    let mut stmt = conn.prepare(
        "SELECT strong_id, testament, lemma, transliteration, pronunciation,
                part_of_speech, definition, gloss, derivation
         FROM strongs_lexicon WHERE strong_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![strong_id], |r| {
        Ok(LexiconEntry {
            strong_id: r.get(0)?,
            testament: r.get(1)?,
            lemma: r.get(2)?,
            transliteration: r.get(3)?,
            pronunciation: r.get(4)?,
            part_of_speech: r.get(5)?,
            definition: r.get(6)?,
            gloss: r.get(7)?,
            derivation: r.get(8)?,
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// All Strong's-tagged words for a verse (interlinear view).
pub fn get_verse_words(conn: &Connection, book_num: i64, chapter: i64, verse: i64) -> Result<Vec<WordRow>> {
    let mut stmt = conn.prepare(
        "SELECT vw.word_order, vw.surface_word, vw.strong_id, vw.morphology
         FROM verse_words vw
         JOIN bible_verses v ON v.id = vw.verse_id
         WHERE v.book_num=?1 AND v.chapter=?2 AND v.verse=?3
         ORDER BY vw.word_order",
    )?;
    let rows = stmt.query_map(params![book_num, chapter, verse], |r| {
        Ok(WordRow {
            word_order: r.get(0)?,
            surface_word: r.get(1)?,
            strong_id: r.get(2)?,
            morphology: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Every verse in the canon that uses a given Strong's number.
pub fn verses_for_strong(conn: &Connection, strong_id: &str, limit: i64) -> Result<Vec<VerseRow>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT v.book_num, b.name, v.chapter, v.verse, v.text_kjv
         FROM verse_words vw
         JOIN bible_verses v ON v.id = vw.verse_id
         JOIN bible_books b ON b.book_num = v.book_num
         WHERE vw.strong_id = ?1
         ORDER BY v.book_num, v.chapter, v.verse
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![strong_id, limit], |r| {
        Ok(VerseRow {
            book_num: r.get(0)?,
            book_name: r.get(1)?,
            chapter: r.get(2)?,
            verse: r.get(3)?,
            text_kjv: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Cross-references (TSK/OpenBible) for a verse, ranked by vote count.
pub fn cross_references(conn: &Connection, book_num: i64, chapter: i64, verse: i64, limit: i64) -> Result<Vec<(VerseRow, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT b2.name, v2.book_num, v2.chapter, v2.verse, v2.text_kjv, x.rank
         FROM cross_references x
         JOIN bible_verses v1 ON v1.id = x.from_verse_id
         JOIN bible_verses v2 ON v2.id = x.to_verse_id
         JOIN bible_books b2 ON b2.book_num = v2.book_num
         WHERE v1.book_num=?1 AND v1.chapter=?2 AND v1.verse=?3
         ORDER BY x.rank DESC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![book_num, chapter, verse, limit], |r| {
        Ok((
            VerseRow {
                book_num: r.get(1)?,
                book_name: r.get(0)?,
                chapter: r.get(2)?,
                verse: r.get(3)?,
                text_kjv: r.get(4)?,
            },
            r.get::<_, i64>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Sermons that touch a given verse (scripture→sermon link graph).
pub fn sermons_for_verse(conn: &Connection, book_num: i64, chapter: i64, verse: i64) -> Result<Vec<SermonHit>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.title, s.primary_passage, s.big_idea, s.date_preached, s.series,
                s.liturgical_season, s.structure_type, s.file_path
         FROM scripture_sermon_links l
         JOIN sermon_index s ON s.id = l.sermon_id
         WHERE l.book_num=?1 AND l.chapter=?2 AND l.verse=?3
         ORDER BY s.date_preached DESC",
    )?;
    let rows = stmt.query_map(params![book_num, chapter, verse], |r| {
        Ok(SermonHit {
            id: r.get(0)?,
            title: r.get(1)?,
            primary_passage: r.get(2)?,
            big_idea: r.get(3)?,
            date_preached: r.get(4)?,
            series: r.get(5)?,
            liturgical_season: r.get(6)?,
            structure_type: r.get(7)?,
            file_path: r.get(8)?,
            snippet: String::new(),
            score: 0.0,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Illustration fatigue: illustrations used more than once, most-used first.
pub fn illustration_fatigue(conn: &Connection, min_uses: i64) -> Result<Vec<IllustrationFatigue>> {
    let mut stmt = conn.prepare(
        "SELECT illustration_key, MAX(label) AS label, SUM(use_count) AS total,
                COUNT(DISTINCT sermon_id) AS sermons, MAX(last_used) AS last
         FROM illustration_usage
         GROUP BY illustration_key
         HAVING total >= ?1
         ORDER BY total DESC, last DESC",
    )?;
    let rows = stmt.query_map(params![min_uses], |r| {
        Ok(IllustrationFatigue {
            illustration_key: r.get(0)?,
            label: r.get(1)?,
            total_uses: r.get(2)?,
            sermon_count: r.get(3)?,
            last_used: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Aggregate archive statistics for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArchiveStats {
    pub sermon_count: i64,
    pub series_count: i64,
    pub verse_links: i64,
    pub illustration_count: i64,
    pub last_rebuild: Option<String>,
}

pub fn archive_stats(conn: &Connection) -> Result<ArchiveStats> {
    let sermon_count: i64 = conn.query_row("SELECT COUNT(*) FROM sermon_index", [], |r| r.get(0))?;
    let series_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT series) FROM sermon_index WHERE series IS NOT NULL AND series <> ''",
        [],
        |r| r.get(0),
    )?;
    let verse_links: i64 = conn.query_row("SELECT COUNT(*) FROM scripture_sermon_links", [], |r| r.get(0))?;
    let illustration_count: i64 = conn.query_row("SELECT COUNT(*) FROM illustration_usage", [], |r| r.get(0))?;
    let last_rebuild: Option<String> = conn
        .query_row("SELECT value FROM index_meta WHERE key='last_rebuild'", [], |r| r.get(0))
        .ok();
    Ok(ArchiveStats {
        sermon_count,
        series_count,
        verse_links,
        illustration_count,
        last_rebuild,
    })
}

/// List all sermons, newest first.
pub fn list_sermons(conn: &Connection) -> Result<Vec<SermonHit>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, primary_passage, big_idea, date_preached, series,
                liturgical_season, structure_type, file_path
         FROM sermon_index
         ORDER BY COALESCE(date_preached, '') DESC, title ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SermonHit {
            id: r.get(0)?,
            title: r.get(1)?,
            primary_passage: r.get(2)?,
            big_idea: r.get(3)?,
            date_preached: r.get(4)?,
            series: r.get(5)?,
            liturgical_season: r.get(6)?,
            structure_type: r.get(7)?,
            file_path: r.get(8)?,
            snippet: String::new(),
            score: 0.0,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
