//! The Librarian — an optional, fully-offline background cataloger.
//!
//! Design constraints (from the user persona):
//!   * AI is OFF by default and completely invisible. There is no chat UI.
//!   * No network. No cloud. No model downloads at runtime.
//!   * It only ever *suggests* metadata; the pastor's `.md` files remain the
//!     source of truth and are never rewritten without an explicit action.
//!
//! The Librarian is deterministic and local: it uses lexical statistics
//! (term frequency, scripture overlap, illustration reuse) to surface related
//! sermons and candidate tags. It is a librarian, not an author.

use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSermon {
    pub id: String,
    pub title: String,
    pub primary_passage: String,
    pub similarity: f64,
    pub shared_verses: usize,
    pub shared_terms: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSuggestion {
    pub sermon_id: String,
    pub suggested_tags: Vec<String>,
    pub related: Vec<RelatedSermon>,
    pub reused_illustrations: Vec<String>,
}

/// Compute related sermons for a given sermon id using a blend of scripture
/// overlap and lexical (bag-of-words) cosine similarity.
pub fn related_sermons(conn: &Connection, sermon_id: &str, limit: usize) -> Result<Vec<RelatedSermon>> {
    // Gather all sermons' bodies.
    let mut stmt = conn.prepare(
        "SELECT s.id, s.title, s.primary_passage, COALESCE(b.body_content, '')
         FROM sermon_index s LEFT JOIN sermon_body b ON b.sermon_id = s.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut docs: Vec<(String, String, String, HashMap<String, f64>)> = Vec::new();
    for row in rows {
        let (id, title, passage, body) = row?;
        let tf = term_frequencies(&body);
        docs.push((id, title, passage, tf));
    }

    let target = docs.iter().find(|d| d.0 == sermon_id).cloned();
    let target = match target {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    // Scripture overlap sets.
    let target_verses = verse_set(conn, sermon_id)?;

    let mut scored: Vec<RelatedSermon> = Vec::new();
    for (id, title, passage, tf) in &docs {
        if id == &sermon_id {
            continue;
        }
        let cos = cosine(&target.3, tf);
        let other_verses = verse_set(conn, id)?;
        let shared = target_verses.intersection(&other_verses).count();
        let shared_terms = target
            .3
            .keys()
            .filter(|k| tf.contains_key(*k))
            .count();
        // Blend: scripture overlap is weighted heavily for a preacher.
        let similarity = cos + (shared as f64) * 0.15;
        if similarity > 0.02 || shared > 0 {
            scored.push(RelatedSermon {
                id: id.clone(),
                title: title.clone(),
                primary_passage: passage.clone(),
                similarity,
                shared_verses: shared,
                shared_terms,
            });
        }
    }
    scored.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored)
}

/// Produce catalog suggestions for a sermon: candidate tags, related sermons,
/// and any illustrations it reuses from elsewhere in the archive.
pub fn catalog(conn: &Connection, sermon_id: &str) -> Result<CatalogSuggestion> {
    let related = related_sermons(conn, sermon_id, 5)?;

    // Candidate tags: the most distinctive terms in this sermon relative to the
    // rest of the corpus (simple TF-IDF).
    let body: String = conn
        .query_row(
            "SELECT COALESCE(body_content,'') FROM sermon_body WHERE sermon_id=?1",
            rusqlite::params![sermon_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let tf = term_frequencies(&body);
    let mut df: HashMap<String, usize> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT COALESCE(body_content,'') FROM sermon_body")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for row in rows {
            let text = row?;
            let uniq: HashSet<String> = term_frequencies(&text).into_keys().collect();
            for t in uniq {
                *df.entry(t).or_insert(0) += 1;
            }
        }
    }
    let n_docs = df.values().copied().max().unwrap_or(1).max(1) as f64;
    let mut scored_terms: Vec<(String, f64)> = tf
        .iter()
        .filter(|(t, _)| t.len() > 3 && !STOPWORDS.contains(&t.as_str()))
        .map(|(t, f)| {
            let dfi = *df.get(t).unwrap_or(&1) as f64;
            let idf = (n_docs / dfi).ln().max(0.0) + 1.0;
            (t.clone(), f * idf)
        })
        .collect();
    scored_terms.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let suggested_tags: Vec<String> = scored_terms.into_iter().take(8).map(|(t, _)| t).collect();

    // Reused illustrations.
    let mut reused = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT i.illustration_key FROM illustration_usage i
             WHERE i.sermon_id = ?1
               AND (SELECT COUNT(DISTINCT sermon_id) FROM illustration_usage i2
                    WHERE i2.illustration_key = i.illustration_key) > 1",
        )?;
        let rows = stmt.query_map(rusqlite::params![sermon_id], |r| r.get::<_, String>(0))?;
        for row in rows {
            reused.push(row?);
        }
    }

    Ok(CatalogSuggestion {
        sermon_id: sermon_id.to_string(),
        suggested_tags,
        related,
        reused_illustrations: reused,
    })
}

fn verse_set(conn: &Connection, sermon_id: &str) -> Result<HashSet<(i64, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT book_num, chapter, verse FROM scripture_sermon_links WHERE sermon_id=?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![sermon_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })?;
    let mut set = HashSet::new();
    for row in rows {
        set.insert(row?);
    }
    Ok(set)
}

fn term_frequencies(text: &str) -> HashMap<String, f64> {
    let mut tf: HashMap<String, f64> = HashMap::new();
    for w in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
    {
        let lw = w.to_lowercase();
        if STOPWORDS.contains(&lw.as_str()) {
            continue;
        }
        *tf.entry(lw).or_insert(0.0) += 1.0;
    }
    tf
}

fn cosine(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    for (k, va) in a {
        if let Some(vb) = b.get(k) {
            dot += va * vb;
        }
    }
    let na: f64 = a.values().map(|v| v * v).sum::<f64>().sqrt();
    let nb: f64 = b.values().map(|v| v * v).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// A compact English stopword list (no external dependency, fully offline).
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "her", "was", "one",
    "our", "out", "day", "get", "has", "him", "his", "how", "man", "new", "now", "old", "see",
    "two", "way", "who", "boy", "did", "its", "let", "put", "say", "she", "too", "use", "that",
    "with", "have", "this", "will", "your", "from", "they", "know", "want", "been", "good",
    "much", "some", "time", "very", "when", "come", "here", "just", "like", "long", "make",
    "many", "more", "only", "over", "such", "take", "than", "them", "well", "were", "what",
    "which", "their", "there", "would", "about", "into", "could", "other", "these", "those",
    "then", "also", "unto", "shall", "thou", "thee", "thy", "thine", "ye", "hath", "doth",
    "upon", "him", "her", "his", "who", "whom", "whose", "may", "might", "must", "every",
    "because", "therefore", "wherefore", "behold", "verily", "amen", "lord", "god", "jesus",
    "christ", "said", "saying", "saith", "even", "yet", "nor", "neither", "either", "being",
    "made", "make", "came", "come", "went", "go", "going", "thing", "things", "man", "men",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_is_one() {
        let a = term_frequencies("grace mercy peace grace");
        let b = term_frequencies("grace mercy peace grace");
        assert!((cosine(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_disjoint_is_zero() {
        let a = term_frequencies("grace mercy");
        let b = term_frequencies("judgment wrath");
        assert_eq!(cosine(&a, &b), 0.0);
    }
}
