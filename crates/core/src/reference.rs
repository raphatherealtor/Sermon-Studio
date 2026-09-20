//! Scripture reference parsing and normalization.
//!
//! Accepts the many ways a pastor writes a reference and normalizes it to the
//! canonical form used throughout the index:
//!
//!   "Rom.8.28-Rom.8.30"   (canonical, dotted)
//!   "Romans 8:28-30"
//!   "Rom 8:28-8:30"
//!   "Gen 1:1"
//!   "Psalm 23"
//!   "1 Cor 13:4-7"
//!
//! A reference is stored as a start verse and an optional end verse, each a
//! (book_num, chapter, verse) triple.

use crate::books::{book_by_name, book_by_osis};
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerseRef {
    pub book_num: i64,
    pub chapter: i64,
    pub verse: i64,
}

impl VerseRef {
    pub fn new(book_num: i64, chapter: i64, verse: i64) -> Self {
        Self { book_num, chapter, verse }
    }

    /// Canonical dotted form, e.g. "Rom.8.28".
    pub fn canonical(&self) -> String {
        let osis = crate::books::BOOKS
            .iter()
            .find(|b| b.0 == self.book_num)
            .map(|b| b.1)
            .unwrap_or("?");
        format!("{}.{}.{}", osis, self.chapter, self.verse)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassageRef {
    pub start: VerseRef,
    pub end: VerseRef,
}

impl PassageRef {
    pub fn single(v: VerseRef) -> Self {
        Self { start: v, end: v }
    }

    /// Canonical string. A single verse is "Rom.8.28"; a range is
    /// "Rom.8.28-Rom.8.30".
    pub fn canonical(&self) -> String {
        if self.start == self.end {
            self.start.canonical()
        } else {
            format!("{}-{}", self.start.canonical(), self.end.canonical())
        }
    }

    /// Expand into every verse in the passage (inclusive). Guarded against
    /// absurd ranges.
    pub fn expand(&self) -> Vec<VerseRef> {
        let mut out = Vec::new();
        let (mut b, mut c, mut v) = (self.start.book_num, self.start.chapter, self.start.verse);
        let (eb, ec, ev) = (self.end.book_num, self.end.chapter, self.end.verse);
        let mut guard = 0;
        loop {
            out.push(VerseRef::new(b, c, v));
            if (b, c, v) == (eb, ec, ev) {
                break;
            }
            v += 1;
            if v > 200 {
                v = 1;
                c += 1;
                if c > 150 {
                    c = 1;
                    b += 1;
                }
            }
            guard += 1;
            if guard > 5000 {
                break;
            }
        }
        out
    }
}

/// Parse a book token + chapter/verse tail. Returns (book_num, chapter, verse).
fn parse_single(token: &str) -> Result<VerseRef> {
    let t = token.trim();
    if t.is_empty() {
        return Err(CoreError::BadReference(token.to_string(), "empty".into()));
    }

    // Split off the trailing numbers: book part may contain digits (1 John).
    // Strategy: find the last alphabetic run; everything after is numbers.
    let bytes: Vec<char> = t.chars().collect();
    let mut split_at = bytes.len();
    for i in (0..bytes.len()).rev() {
        let ch = bytes[i];
        if ch.is_ascii_digit() || ch == '.' || ch == ':' || ch == ' ' {
            split_at = i;
        } else {
            break;
        }
    }
    let book_part: String = bytes[..split_at].iter().collect();
    let num_part: String = bytes[split_at..].iter().collect();

    let book = book_by_name(&book_part)
        .or_else(|| book_by_osis(&book_part))
        .ok_or_else(|| CoreError::BadReference(token.to_string(), format!("unknown book '{}'", book_part.trim())))?;

    // Numbers separated by '.', ':', or whitespace.
    let nums: Vec<i64> = num_part
        .split(|c: char| c == '.' || c == ':' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i64>().ok())
        .collect();

    let chapter = nums.first().copied().unwrap_or(1);
    let verse = nums.get(1).copied().unwrap_or(1);
    Ok(VerseRef::new(book.0, chapter, verse))
}

/// Parse a full passage reference string into a normalized PassageRef.
pub fn parse_passage(input: &str) -> Result<PassageRef> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(CoreError::BadReference(input.to_string(), "empty".into()));
    }

    // Normalize separators: en/em dashes to '-'.
    let normalized = raw.replace(['–', '—'], "-");

    // Case A: explicit two-part range "A-B" where B repeats the book.
    if let Some((left, right)) = split_range(&normalized) {
        let start = parse_single(left)?;
        // Right side may be a full ref ("Rom.8.30") or a shorthand ("30" or "8:30").
        let end = parse_end_shorthand(right, start)?;
        return Ok(PassageRef { start, end });
    }

    // Case B: single reference, possibly with an inline verse range "8:28-30".
    let start = parse_single(&normalized)?;
    Ok(PassageRef::single(start))
}

/// Split "A-B" into (A, B) only when the '-' is a range separator, not a
/// hyphen inside a book name (there are none in our canon).
fn split_range(s: &str) -> Option<(&str, &str)> {
    let idx = s.find('-')?;
    let left = s[..idx].trim();
    let right = s[idx + 1..].trim();
    if left.is_empty() || right.is_empty() {
        return None;
    }
    Some((left, right))
}

/// Interpret the right-hand side of a range. It may be:
///   - a full reference: "Rom.8.30" or "Romans 8:30"
///   - chapter:verse: "8:30"
///   - verse only: "30"
fn parse_end_shorthand(right: &str, start: VerseRef) -> Result<VerseRef> {
    let r = right.trim();
    // Full reference if it starts with a letter.
    if r.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false) {
        return parse_single(r);
    }
    let nums: Vec<i64> = r
        .split(|c: char| c == '.' || c == ':' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i64>().ok())
        .collect();
    match nums.as_slice() {
        [v] => Ok(VerseRef::new(start.book_num, start.chapter, *v)),
        [c, v] => Ok(VerseRef::new(start.book_num, *c, *v)),
        _ => Err(CoreError::BadReference(right.to_string(), "unparseable range end".into())),
    }
}

/// Normalize any reference string to canonical dotted form. Falls back to the
/// trimmed input if parsing fails (so indexing never hard-fails on a typo).
pub fn normalize(input: &str) -> String {
    match parse_passage(input) {
        Ok(p) => p.canonical(),
        Err(_) => input.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical() {
        let p = parse_passage("Rom.8.28-Rom.8.30").unwrap();
        assert_eq!(p.canonical(), "Rom.8.28-Rom.8.30");
        assert_eq!(p.expand().len(), 3);
    }

    #[test]
    fn parses_colon_range() {
        let p = parse_passage("Romans 8:28-30").unwrap();
        assert_eq!(p.canonical(), "Rom.8.28-Rom.8.30");
    }

    #[test]
    fn parses_single() {
        let p = parse_passage("John 3:16").unwrap();
        assert_eq!(p.canonical(), "John.3.16");
    }

    #[test]
    fn parses_numbered_book() {
        let p = parse_passage("1 Cor 13:4-7").unwrap();
        assert_eq!(p.canonical(), "1Cor.13.4-1Cor.13.7");
    }

    #[test]
    fn parses_chapter_only() {
        let p = parse_passage("Psalm 23").unwrap();
        assert_eq!(p.canonical(), "Ps.23.1");
    }

    #[test]
    fn parses_cross_chapter() {
        let p = parse_passage("Rom 8:38-9:2").unwrap();
        assert_eq!(p.canonical(), "Rom.8.38-Rom.9.2");
    }
}
