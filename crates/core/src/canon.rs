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

use crate::books::BOOKS;
use crate::error::Result;
use crate::schema::CANON_SCHEMA;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Build canon.db at `out_path` from the clean data directory `clean_dir`.
pub fn build_canon_db(clean_dir: &Path, out_path: &Path) -> Result<CanonStats> {
    if out_path.exists() {
        std::fs::remove_file(out_path)?;
    }
    let mut conn = Connection::open(out_path)?;
    conn.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA cache_size=-64000;")?;
    conn.execute_batch(CANON_SCHEMA)?;

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

    Ok(stats)
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
}
