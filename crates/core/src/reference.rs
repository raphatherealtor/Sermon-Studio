//! Scripture reference parsing, normalization, and resolution.
//!
//! Accepts the many ways a pastor (modern or historical) writes a reference and
//! resolves it to one of three explicit outcomes:
//!
//!   * [`Resolution::Definite`]  — enough information to resolve deterministically
//!   * [`Resolution::Ambiguous`] — looks like a reference but needs context
//!   * [`Resolution::Invalid`]   — structurally recognizable but out of range
//!
//! Examples that resolve to the canonical dotted form:
//!
//!   "John 3:16"           -> "John.3.16"
//!   "Rom. viii. 28"       -> "Rom.8.28"
//!   "I Cor. xiii. 4-7"    -> "1Cor.13.4-1Cor.13.7"
//!   "St. John iii. 16"    -> "John.3.16"
//!   "II John 6"           -> "2John.1.6"
//!   "Psalm cxix.105"      -> "Ps.119.105"
//!
//! The parser never clamps out-of-range numbers and never invents an ending
//! verse: impossible chapters/verses are [`Resolution::Invalid`], context-only
//! forms (`v.6`, `vv.4-7`, `ff.`) are [`Resolution::Ambiguous`], and the
//! original matched text is always preserved separately from the canonical form.

use crate::books::{book_by_name, chapter_count, verse_count};
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

/// The three explicit resolution outcomes for a scripture reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Resolution {
    /// Fully resolvable: book, chapter and verse(s) are known and in range.
    Definite,
    /// A reference was recognized but needs external context to resolve.
    #[default]
    Ambiguous,
    /// Structurally a reference, but contains impossible or out-of-range data.
    Invalid,
}

/// The result of resolving a scripture reference.
///
/// `source` always holds the original matched text (trimmed, otherwise
/// untouched); `passage`/`canonical()` hold the normalized output when the
/// reference is [`Resolution::Definite`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedReference {
    pub resolution: Resolution,
    /// Original matched source text, preserved verbatim (trimmed).
    pub source: String,
    /// Recognized book number, when a book was present.
    pub book_num: Option<i64>,
    /// Start chapter (or the only chapter) when recognized.
    pub chapter: Option<i64>,
    /// End chapter, present only for cross-chapter ranges.
    pub end_chapter: Option<i64>,
    /// Start verse when recognized.
    pub verse_start: Option<i64>,
    /// End verse for a range; equals `verse_start` for a single verse.
    pub verse_end: Option<i64>,
    /// Exact verse numbers for a comma-separated verse list (single chapter).
    pub verses: Vec<i64>,
    /// True when the reference ends in an open continuation (`ff.`).
    pub open_ended: bool,
    /// Resolved passage, present when the reference is a single verse or a
    /// contiguous range. For an open-ended (`ff.`) reference this holds the
    /// definite anchor.
    pub passage: Option<PassageRef>,
    /// Human-readable reason when [`Resolution::Ambiguous`] or [`Resolution::Invalid`].
    pub reason: Option<String>,
}

impl ParsedReference {
    /// Canonical normalized form, present only when [`Resolution::Definite`].
    ///
    /// A single verse is "John.3.16", a range is "John.3.16-John.3.18", and a
    /// non-contiguous verse list is "John.3.16,John.3.18,John.3.20".
    pub fn canonical(&self) -> Option<String> {
        if self.resolution != Resolution::Definite {
            return None;
        }
        if let Some(p) = &self.passage {
            return Some(p.canonical());
        }
        if !self.verses.is_empty() {
            let book = self.book_num?;
            let chapter = self.chapter?;
            let osis = crate::books::BOOKS
                .iter()
                .find(|b| b.0 == book)
                .map(|b| b.1)
                .unwrap_or("?");
            let parts: Vec<String> = self
                .verses
                .iter()
                .map(|v| format!("{}.{}.{}", osis, chapter, v))
                .collect();
            return Some(parts.join(","));
        }
        None
    }
}

/// Resolve a single scripture reference token.
///
/// Returns `None` when the input does not look like a scripture reference at
/// all (no recognized book, no `v.`/`vv.` marker, no `ff.` marker). Otherwise
/// returns a [`ParsedReference`] whose [`ParsedReference::resolution`] is one of
/// [`Resolution::Definite`], [`Resolution::Ambiguous`], or [`Resolution::Invalid`].
pub fn resolve(input: &str) -> Option<ParsedReference> {
    let source = input.trim();
    if source.is_empty() {
        return None;
    }
    let normalized = source.replace(['\u{2013}', '\u{2014}'], "-");

    let (no_ff, open_ended) = strip_ff(&normalized);
    let (body, verse_only) = match strip_verse_marker(&no_ff) {
        Some(rest) => (rest, true),
        None => (no_ff, false),
    };
    let body = body.trim();
    if body.is_empty() {
        if open_ended {
            // A bare "ff." marker: an open continuation with no anchor.
            return Some(ParsedReference {
                resolution: Resolution::Ambiguous,
                source: source.to_string(),
                open_ended: true,
                reason: Some("open continuation without an anchor".to_string()),
                ..ParsedReference::default()
            });
        }
        return None;
    }

    match parse_whole(body, verse_only) {
        SideOutcome::NotRef => None,
        SideOutcome::Malformed(reason) => Some(invalid(source, reason)),
        SideOutcome::Loc(loc) => Some(classify(&loc, verse_only, open_ended, source)),
    }
}

/// Parse a full passage reference string into a normalized [`PassageRef`].
///
/// Backward-compatible entry point: returns `Ok` only for
/// [`Resolution::Definite`] references that can be represented as a single
/// passage, and an error for ambiguous, invalid, or non-reference input.
pub fn parse_passage(input: &str) -> Result<PassageRef> {
    match resolve(input) {
        Some(r) if r.resolution == Resolution::Definite => r.passage.ok_or_else(|| {
            CoreError::BadReference(input.to_string(), "no single-passage representation".into())
        }),
        Some(r) => Err(CoreError::BadReference(
            input.to_string(),
            r.reason
                .unwrap_or_else(|| "reference could not be resolved".to_string()),
        )),
        None => Err(CoreError::BadReference(
            input.to_string(),
            "not a scripture reference".to_string(),
        )),
    }
}

/// Normalize any reference string to canonical dotted form. Falls back to the
/// trimmed input if the input is not a definite reference (so indexing never
/// hard-fails on a typo).
pub fn normalize(input: &str) -> String {
    match resolve(input) {
        Some(r) => r.canonical().unwrap_or_else(|| input.trim().to_string()),
        None => input.trim().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Internal parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Sep(char),
}

#[derive(Debug, Clone, Default)]
struct Loc {
    book: Option<i64>,
    chapter: Option<i64>,
    end_chapter: Option<i64>,
    verse_start: Option<i64>,
    verse_end: Option<i64>,
    verses: Vec<i64>,
    is_list: bool,
    is_range: bool,
}

enum SideOutcome {
    NotRef,
    Loc(Loc),
    Malformed(String),
}

/// Strip a trailing "ff" / "ff." continuation marker. Returns the remaining
/// body plus whether the marker was present.
fn strip_ff(s: &str) -> (String, bool) {
    let t = s.trim_end();
    let lower = t.to_lowercase();
    let (cut, open) = if lower.ends_with("ff.") {
        (3, true)
    } else if lower.ends_with("ff") {
        (2, true)
    } else {
        (0, false)
    };
    if !open {
        return (t.to_string(), false);
    }
    let body = &t[..t.len() - cut];
    let body = body.trim_end();
    // "ff" must be a standalone marker, not the tail of a word like "off".
    if body
        .chars()
        .next_back()
        .map(|c| c.is_alphabetic())
        .unwrap_or(false)
    {
        return (t.to_string(), false);
    }
    (body.to_string(), true)
}

/// Strip a leading "v." / "vv." verse-only marker. Returns the remainder when a
/// marker is present (followed by a dot or whitespace).
fn strip_verse_marker(s: &str) -> Option<String> {
    let t = s.trim_start();
    let lower = t.to_lowercase();
    let rest = if lower.starts_with("vv") {
        Some(&t[2..])
    } else if lower.starts_with('v') {
        Some(&t[1..])
    } else {
        None
    };
    let rest = rest?;
    let trimmed = rest.trim_start_matches(['.', ' ', '\t']);
    if trimmed.len() < rest.len() {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Find the book name at the start of `s`, returning the book number and the
/// byte index where the numeric tail begins. Uses the longest known-book prefix.
fn find_book(s: &str) -> Option<(i64, usize)> {
    let max = s.len().min(32);
    for end in (1..=max).rev() {
        if !s.is_char_boundary(end) {
            continue;
        }
        let prefix = s[..end].trim_end_matches([' ', '.', '\t']);
        if prefix.is_empty() {
            continue;
        }
        if let Some(book) = book_by_name(prefix) {
            return Some((book.0, end));
        }
    }
    None
}

fn is_roman_char(c: char) -> bool {
    matches!(
        c,
        'i' | 'I' | 'v' | 'V' | 'x' | 'X' | 'l' | 'L' | 'c' | 'C' | 'd' | 'D' | 'm' | 'M'
    )
}

/// Parse a Roman numeral (case-insensitive) to an integer. Uses the standard
/// subtractive algorithm; returns `None` for empty input or non-Roman chars.
fn roman_to_int(s: &str) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let mut total: i64 = 0;
    let mut prev: i64 = 0;
    for c in s.chars() {
        let v = match c {
            'i' | 'I' => 1,
            'v' | 'V' => 5,
            'x' | 'X' => 10,
            'l' | 'L' => 50,
            'c' | 'C' => 100,
            'd' | 'D' => 500,
            'm' | 'M' => 1000,
            _ => return None,
        };
        if v > prev {
            total += v - 2 * prev;
        } else {
            total += v;
        }
        prev = v;
    }
    Some(total)
}

/// Tokenize a numeric tail into numbers and separators. Returns `None` when the
/// tail contains non-numeric prose (i.e. it is not a reference).
fn tokenize_tail(s: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let mut n: i64 = 0;
            while i < chars.len() && chars[i].is_ascii_digit() {
                n = n * 10 + (chars[i] as i64 - '0' as i64);
                i += 1;
            }
            out.push(Tok::Num(n));
        } else if is_roman_char(c) {
            let start = i;
            while i < chars.len() && is_roman_char(chars[i]) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            out.push(Tok::Num(roman_to_int(&run)?));
        } else if c == ':' || c == '.' || c == ',' || c == '-' {
            out.push(Tok::Sep(c));
            i += 1;
        } else {
            return None;
        }
    }
    // Drop trailing sentence punctuation ("John 3:16.").
    while matches!(out.last(), Some(Tok::Sep('.' | ',' | ':'))) {
        out.pop();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Interpret a token list (single locator or verse list; ranges are split at the
/// top level on '-') into a [`Loc`].
fn interpret_tail(toks: &[Tok], book: Option<i64>, verse_only: bool) -> Option<Loc> {
    if toks.iter().any(|t| matches!(t, Tok::Sep(','))) {
        interpret_list(toks, book, verse_only)
    } else {
        interpret_single(toks, book, verse_only)
    }
}

fn interpret_single(toks: &[Tok], book: Option<i64>, verse_only: bool) -> Option<Loc> {
    match toks {
        [Tok::Num(a)] => {
            if verse_only {
                Some(Loc {
                    verse_start: Some(*a),
                    verse_end: Some(*a),
                    ..Loc::default()
                })
            } else if is_single_chapter(book) {
                Some(Loc {
                    book,
                    chapter: Some(1),
                    verse_start: Some(*a),
                    verse_end: Some(*a),
                    ..Loc::default()
                })
            } else {
                Some(Loc {
                    book,
                    chapter: Some(*a),
                    ..Loc::default()
                })
            }
        }
        [Tok::Num(c), Tok::Sep(s), Tok::Num(v)] if *s == ':' || *s == '.' => Some(Loc {
            book,
            chapter: Some(*c),
            verse_start: Some(*v),
            verse_end: Some(*v),
            ..Loc::default()
        }),
        _ => None,
    }
}

fn interpret_list(toks: &[Tok], book: Option<i64>, verse_only: bool) -> Option<Loc> {
    if verse_only {
        let verses = parse_comma_verses(toks)?;
        return Some(Loc {
            verse_start: verses.first().copied(),
            verse_end: verses.last().copied(),
            verses,
            is_list: true,
            ..Loc::default()
        });
    }
    let (chapter, rest) = match toks {
        [Tok::Num(c), Tok::Sep(s), rest @ ..] if *s == ':' || *s == '.' => (*c, rest),
        _ => return None,
    };
    let verses = parse_comma_verses(rest)?;
    Some(Loc {
        book,
        chapter: Some(chapter),
        verse_start: verses.first().copied(),
        verse_end: verses.last().copied(),
        verses,
        is_list: true,
        ..Loc::default()
    })
}

fn parse_comma_verses(toks: &[Tok]) -> Option<Vec<i64>> {
    let mut verses = Vec::new();
    let mut expect_num = true;
    for t in toks {
        match t {
            Tok::Num(n) if expect_num => {
                verses.push(*n);
                expect_num = false;
            }
            Tok::Sep(',') if !expect_num => expect_num = true,
            _ => return None,
        }
    }
    if expect_num || verses.is_empty() {
        return None;
    }
    Some(verses)
}

/// Parse one side of the input (either the whole reference or one half of a
/// range).
fn parse_side(s: &str, verse_only: bool) -> SideOutcome {
    let s = s.trim();
    if s.is_empty() {
        return SideOutcome::NotRef;
    }
    if verse_only {
        let toks = match tokenize_tail(s) {
            Some(t) => t,
            None => return SideOutcome::NotRef,
        };
        return match interpret_tail(&toks, None, true) {
            Some(loc) => SideOutcome::Loc(loc),
            None => SideOutcome::Malformed("malformed verse reference".to_string()),
        };
    }
    let (book, tail_start) = match find_book(s) {
        Some(x) => x,
        None => return SideOutcome::NotRef,
    };
    let tail = s[tail_start..].trim();
    if tail.is_empty() {
        return SideOutcome::NotRef;
    }
    let toks = match tokenize_tail(tail) {
        Some(t) => t,
        None => return SideOutcome::NotRef,
    };
    match interpret_tail(&toks, Some(book), false) {
        Some(loc) => SideOutcome::Loc(loc),
        None => SideOutcome::Malformed("malformed reference".to_string()),
    }
}

/// Parse the right-hand side of a range as a shorthand end (verse, or
/// chapter:verse), inheriting the book from the left side.
fn parse_shorthand_end(s: &str) -> SideOutcome {
    let toks = match tokenize_tail(s) {
        Some(t) => t,
        None => return SideOutcome::NotRef,
    };
    match toks.as_slice() {
        [Tok::Num(v)] => SideOutcome::Loc(Loc {
            verse_start: Some(*v),
            verse_end: Some(*v),
            ..Loc::default()
        }),
        [Tok::Num(c), Tok::Sep(s), Tok::Num(v)] if *s == ':' || *s == '.' => SideOutcome::Loc(Loc {
            chapter: Some(*c),
            verse_start: Some(*v),
            verse_end: Some(*v),
            ..Loc::default()
        }),
        _ => SideOutcome::Malformed("malformed range end".to_string()),
    }
}

/// Parse the right-hand side of a verse-only range (a single verse number).
fn parse_verse_only_end(s: &str) -> SideOutcome {
    let toks = match tokenize_tail(s) {
        Some(t) => t,
        None => return SideOutcome::NotRef,
    };
    match toks.as_slice() {
        [Tok::Num(v)] => SideOutcome::Loc(Loc {
            verse_start: Some(*v),
            verse_end: Some(*v),
            ..Loc::default()
        }),
        _ => SideOutcome::Malformed("malformed verse range end".to_string()),
    }
}

/// Merge the left and right sides of a range into a single range [`Loc`].
fn merge_range(left: Loc, right: Loc, verse_only: bool) -> SideOutcome {
    if verse_only {
        let vs = match left.verse_start {
            Some(v) => v,
            None => return SideOutcome::Malformed("malformed verse range".to_string()),
        };
        let ve = match right.verse_start {
            Some(v) => v,
            None => return SideOutcome::Malformed("malformed verse range".to_string()),
        };
        return SideOutcome::Loc(Loc {
            verse_start: Some(vs),
            verse_end: Some(ve),
            is_range: true,
            ..Loc::default()
        });
    }

    let book = match left.book.or(right.book) {
        Some(b) => b,
        None => return SideOutcome::Malformed("range without a book".to_string()),
    };
    if let Some(rb) = right.book {
        if rb != book {
            return SideOutcome::Malformed("range spans different books".to_string());
        }
    }
    let chapter = match left.chapter {
        Some(c) => c,
        None => return SideOutcome::Malformed("range start missing a chapter".to_string()),
    };
    let vs = match left.verse_start {
        Some(v) => v,
        None => return SideOutcome::Malformed("range start missing a verse".to_string()),
    };
    let end_chapter = right.chapter.unwrap_or(chapter);
    let ve = match right.verse_start {
        Some(v) => v,
        None => return SideOutcome::Malformed("range end missing a verse".to_string()),
    };
    SideOutcome::Loc(Loc {
        book: Some(book),
        chapter: Some(chapter),
        end_chapter: Some(end_chapter),
        verse_start: Some(vs),
        verse_end: Some(ve),
        is_range: true,
        ..Loc::default()
    })
}

/// Parse the (already prefix-stripped) body, splitting a range on '-'.
fn parse_whole(s: &str, verse_only: bool) -> SideOutcome {
    if let Some(dash) = s.find('-') {
        let left = s[..dash].trim();
        let right = s[dash + 1..].trim();
        if left.is_empty() || right.is_empty() {
            return SideOutcome::Malformed("malformed range".to_string());
        }
        let left_loc = match parse_side(left, verse_only) {
            SideOutcome::NotRef => return SideOutcome::NotRef,
            SideOutcome::Malformed(r) => return SideOutcome::Malformed(r),
            SideOutcome::Loc(l) => l,
        };
        let right_outcome = if verse_only {
            parse_verse_only_end(right)
        } else if right
            .chars()
            .next()
            .map(|c| c.is_alphabetic())
            .unwrap_or(false)
        {
            parse_side(right, false)
        } else {
            parse_shorthand_end(right)
        };
        let right_loc = match right_outcome {
            SideOutcome::NotRef => return SideOutcome::NotRef,
            SideOutcome::Malformed(r) => return SideOutcome::Malformed(r),
            SideOutcome::Loc(l) => l,
        };
        merge_range(left_loc, right_loc, verse_only)
    } else {
        parse_side(s, verse_only)
    }
}

fn is_single_chapter(book: Option<i64>) -> bool {
    match book {
        Some(b) => chapter_count(b).map(|c| c == 1).unwrap_or(false),
        None => false,
    }
}

fn is_contiguous(verses: &[i64]) -> bool {
    verses.windows(2).all(|w| w[1] == w[0] + 1)
}

fn chapter_valid(book: i64, chapter: i64) -> bool {
    chapter >= 1 && chapter_count(book).map(|cc| chapter as u16 <= cc).unwrap_or(false)
}

fn verse_valid(book: i64, chapter: i64, verse: i64) -> bool {
    verse >= 1 && verse_count(book, chapter).map(|vc| verse as u16 <= vc).unwrap_or(false)
}

fn invalid(source: &str, reason: String) -> ParsedReference {
    ParsedReference {
        resolution: Resolution::Invalid,
        source: source.to_string(),
        reason: Some(reason),
        ..ParsedReference::default()
    }
}

/// Mark a partially-filled result as invalid while preserving the parsed
/// book/chapter/verse fields so callers can report exactly what was rejected.
fn invalid_from(base: &ParsedReference, reason: String) -> ParsedReference {
    ParsedReference {
        resolution: Resolution::Invalid,
        reason: Some(reason),
        passage: None,
        ..base.clone()
    }
}

/// Classify a fully parsed [`Loc`] into a [`ParsedReference`], applying
/// chapter/verse range validation without clamping.
fn classify(loc: &Loc, verse_only: bool, open_ended: bool, source: &str) -> ParsedReference {
    let mut r = classify_closed(loc, verse_only, source);
    r.open_ended = open_ended;
    // An open-ended reference inherits the uncertainty of its unspecified
    // terminal extent: the anchor is definite, but the reference as a whole is
    // ambiguous. Preserve the anchor (book/chapter/verse_start and `passage`)
    // while never fabricating a terminal verse.
    if open_ended && r.resolution == Resolution::Definite {
        r.resolution = Resolution::Ambiguous;
        r.verse_end = None;
        r.reason = Some("open-ended reference: terminal extent is unspecified".to_string());
    }
    r
}

fn classify_closed(loc: &Loc, verse_only: bool, source: &str) -> ParsedReference {
    let base = ParsedReference {
        resolution: Resolution::Ambiguous,
        source: source.to_string(),
        book_num: loc.book,
        chapter: loc.chapter,
        end_chapter: loc.end_chapter,
        verse_start: loc.verse_start,
        verse_end: loc.verse_end,
        verses: loc.verses.clone(),
        open_ended: false,
        passage: None,
        reason: None,
    };

    // Contextual verse-only references (v.6, vv.4-7) are always ambiguous until
    // a caller supplies book/chapter context, but a verse of 0 is invalid.
    if verse_only {
        if let Some(vs) = loc.verse_start {
            if vs <= 0 {
                return invalid_from(&base, format!("verse {vs} is out of range"));
            }
            if let Some(ve) = loc.verse_end {
                if ve <= 0 {
                    return invalid_from(&base, format!("verse {ve} is out of range"));
                }
                if ve < vs {
                    return invalid_from(&base, "range end precedes its start".to_string());
                }
            }
        }
        if loc.verses.iter().any(|&v| v <= 0) {
            return invalid_from(&base, "verse 0 is out of range".to_string());
        }
        return ParsedReference {
            reason: Some("verse reference requires passage context".to_string()),
            ..base
        };
    }

    let Some(book) = loc.book else {
        return base;
    };
    let Some(chapter) = loc.chapter else {
        return base;
    };

    if !chapter_valid(book, chapter) {
        return invalid_from(&base, format!("chapter {chapter} is out of range for book {book}"));
    }

    if loc.is_list {
        if loc.verses.is_empty() {
            return invalid_from(&base, "empty verse list".to_string());
        }
        for &v in &loc.verses {
            if !verse_valid(book, chapter, v) {
                return invalid_from(&base, format!("verse {v} is out of range in chapter {chapter}"));
            }
        }
        let mut r = base;
        r.verse_start = loc.verses.first().copied();
        r.verse_end = loc.verses.last().copied();
        if is_contiguous(&loc.verses) {
            let s = VerseRef::new(book, chapter, loc.verses[0]);
            let e = VerseRef::new(book, chapter, loc.verses[loc.verses.len() - 1]);
            r.passage = Some(PassageRef { start: s, end: e });
        }
        r.resolution = Resolution::Definite;
        return r;
    }

    // Chapter-only reference (e.g. "Psalm 23") resolves to verse 1.
    let Some(vs) = loc.verse_start else {
        let v = VerseRef::new(book, chapter, 1);
        return ParsedReference {
            resolution: Resolution::Definite,
            verse_start: Some(1),
            verse_end: Some(1),
            passage: Some(PassageRef::single(v)),
            reason: None,
            ..base
        };
    };

    if !verse_valid(book, chapter, vs) {
        return invalid_from(&base, format!("verse {vs} is out of range in chapter {chapter}"));
    }

    if loc.is_range {
        let Some(ve) = loc.verse_end else {
            return invalid_from(&base, "range is missing an end verse".to_string());
        };
        let end_ch = loc.end_chapter.unwrap_or(chapter);
        if !chapter_valid(book, end_ch) {
            return invalid_from(&base, format!("chapter {end_ch} is out of range for book {book}"));
        }
        if !verse_valid(book, end_ch, ve) {
            return invalid_from(&base, format!("verse {ve} is out of range in chapter {end_ch}"));
        }
        let start = VerseRef::new(book, chapter, vs);
        let end = VerseRef::new(book, end_ch, ve);
        if start > end {
            return invalid_from(&base, "range end precedes its start".to_string());
        }
        return ParsedReference {
            resolution: Resolution::Definite,
            passage: Some(PassageRef { start, end }),
            reason: None,
            ..base
        };
    }

    let v = VerseRef::new(book, chapter, vs);
    ParsedReference {
        resolution: Resolution::Definite,
        verse_end: Some(vs),
        passage: Some(PassageRef::single(v)),
        reason: None,
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Original baseline tests (kept green) --------------------------------

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

    // -- Definite resolution corpus ------------------------------------------

    const DEFINITE: &[(&str, &str)] = &[
        // Modern references.
        ("John 3:16", "John.3.16"),
        ("Romans 8:28", "Rom.8.28"),
        ("Rom 8:28", "Rom.8.28"),
        ("Rom. 8:28", "Rom.8.28"),
        ("1 Kings 18:21", "1Kgs.18.21"),
        ("1Kgs 18:21", "1Kgs.18.21"),
        ("John 3:16-18", "John.3.16-John.3.18"),
        ("Romans 8:28-30", "Rom.8.28-Rom.8.30"),
        ("John 3:16-3:18", "John.3.16-John.3.18"),
        ("Rom 8:38-9:2", "Rom.8.38-Rom.9.2"),
        ("Gen 1:1", "Gen.1.1"),
        ("Psalm 23", "Ps.23.1"),
        ("Psalms 23", "Ps.23.1"),
        ("1 Cor 13:4-7", "1Cor.13.4-1Cor.13.7"),
        ("Rom.8.28-Rom.8.30", "Rom.8.28-Rom.8.30"),
        ("Rom.8.28", "Rom.8.28"),
        ("Romans  8:28", "Rom.8.28"),
        ("John 3:16.", "John.3.16"),
        ("John 21:25", "John.21.25"),
        ("Matthew 28:20", "Matt.28.20"),
        ("Ps 119:176", "Ps.119.176"),
        ("John 3:36", "John.3.36"),
        ("Genesis 1:31", "Gen.1.31"),
        ("Jude 25", "Jude.1.25"),
        ("Song of Solomon 3:1", "Song.3.1"),
        ("Exodus 20:3", "Exod.20.3"),
        ("1 John 5:7", "1John.5.7"),
        ("Obadiah 1:1", "Obad.1.1"),
        ("Philemon 1:3", "Phlm.1.3"),
        ("Revelation 22:21", "Rev.22.21"),
        ("Matthew 28:1", "Matt.28.1"),
        ("Acts 2:38", "Acts.2.38"),
        ("Hebrews 11:1", "Heb.11.1"),
        ("James 1:5", "Jas.1.5"),
        ("Jn 3:16", "John.3.16"),
        ("Phil. 4:13", "Phil.4.13"),
        ("1Co 13:4", "1Cor.13.4"),
        ("Eph 2:8-9", "Eph.2.8-Eph.2.9"),
        // Verse lists.
        ("John 3:16,18,20", "John.3.16,John.3.18,John.3.20"),
        ("Romans 12:1, 2", "Rom.12.1-Rom.12.2"),
        // Historical / KJV references.
        ("I Cor. 13", "1Cor.13.1"),
        ("I Cor. xiii. 4-7", "1Cor.13.4-1Cor.13.7"),
        ("II John 6", "2John.1.6"),
        ("III John 4", "3John.1.4"),
        ("St. John iii. 16", "John.3.16"),
        ("Rom. viii. 28", "Rom.8.28"),
        ("Rom. xii. 1, 2", "Rom.12.1-Rom.12.2"),
        ("Deut. xxxii. 4", "Deut.32.4"),
        ("Psalm cxix.105", "Ps.119.105"),
        ("II Corinthians 13:14", "2Cor.13.14"),
        ("I Thessalonians 5:16", "1Thess.5.16"),
        ("III John 1:4", "3John.1.4"),
    ];

    #[test]
    fn definite_references_resolve() {
        for (input, canonical) in DEFINITE {
            let r = resolve(input).expect(input);
            assert_eq!(r.resolution, Resolution::Definite, "input: {input}");
            assert_eq!(r.canonical().as_deref(), Some(*canonical), "input: {input}");
            assert_eq!(r.source, *input, "input: {input}");
        }
    }

    // -- Ambiguous (contextual) references -----------------------------------

    const AMBIGUOUS: &[&str] = &[
        "vv.4-7", "v.6", "v. 6", "vv 4-7", "ff.", "vv.4", "v.6,7",
    ];

    #[test]
    fn contextual_references_are_ambiguous() {
        for input in AMBIGUOUS {
            let r = resolve(input).expect(input);
            assert_eq!(r.resolution, Resolution::Ambiguous, "input: {input}");
            assert_eq!(r.source, *input, "input: {input}");
        }
    }

    #[test]
    fn contextual_verses_preserve_numbers_for_later_resolution() {
        let r = resolve("vv.4-7").unwrap();
        assert_eq!(r.resolution, Resolution::Ambiguous);
        assert_eq!(r.book_num, None);
        assert_eq!(r.chapter, None);
        assert_eq!(r.verse_start, Some(4));
        assert_eq!(r.verse_end, Some(7));

        let v = resolve("v.6").unwrap();
        assert_eq!(v.resolution, Resolution::Ambiguous);
        assert_eq!(v.verse_start, Some(6));
        assert_eq!(v.verse_end, Some(6));
    }

    // -- Invalid references --------------------------------------------------

    const INVALID: &[&str] = &[
        "John 3:37",        // John 3 has 36 verses
        "John 21:26",       // John 21 has 25 verses
        "John 0:1",         // chapter 0
        "John 3:0",         // verse 0
        "Genesis 51:1",     // Genesis has 50 chapters
        "Psalm 151:1",      // Psalms has 150 chapters
        "Psalm 150:7",      // Psalm 150 has 6 verses
        "Jude 26",          // Jude has 25 verses (single chapter)
        "Romans 16:28",     // Romans 16 has 27 verses
        "Mark 16:21",       // Mark 16 has 20 verses
        "Genesis 1:32",     // Genesis 1 has 31 verses
        "John 3:16-10",     // end precedes start
        "John 3:16-99",     // end out of range
        "John 3:16-18-20",  // malformed range
        "2 John 0:1",       // chapter 0
        "II John 0",        // verse 0
    ];

    #[test]
    fn out_of_range_references_are_invalid() {
        for input in INVALID {
            let r = resolve(input).expect(input);
            assert_eq!(r.resolution, Resolution::Invalid, "input: {input}");
            assert!(r.passage.is_none(), "input: {input}");
            assert!(r.canonical().is_none(), "input: {input}");
            assert!(r.reason.is_some(), "input: {input}");
            assert_eq!(r.source, *input, "input: {input}");
        }
    }

    #[test]
    fn invalid_references_are_never_clamped() {
        // A verse just past the last valid verse must not be folded into a
        // nearby valid reference.
        let r = resolve("John 3:37").unwrap();
        assert_eq!(r.resolution, Resolution::Invalid);
        assert_eq!(r.book_num, Some(43));
        assert_eq!(r.chapter, Some(3));
        assert_eq!(r.verse_start, Some(37));
    }

    // -- Non-references (near-miss prose) ------------------------------------

    const NOT_REFERENCE: &[&str] = &[
        "I have 3 apples",
        "Chapter 5 of my book",
        "The year 2024",
        "John went to the store",
        "Hello world",
        "3:16",
        "",
        "   ",
        "For God so loved the world",
        "This is not scripture",
        "Apple pie recipe 3",
    ];

    #[test]
    fn prose_is_not_parsed_as_a_reference() {
        for input in NOT_REFERENCE {
            assert!(resolve(input).is_none(), "input: {input:?}");
        }
    }

    // -- Open-ended / ff. behavior -------------------------------------------

    #[test]
    fn ff_references_are_ambiguous_and_preserve_the_anchor() {
        // The anchor "John 3:16" is definite, but the full "ff." reference has
        // an unspecified terminal extent, so it inherits Ambiguous.
        for input in ["John iii.16ff.", "John 3:16ff.", "John 3:16 ff."] {
            let r = resolve(input).unwrap();
            assert_eq!(r.resolution, Resolution::Ambiguous, "input: {input}");
            assert!(r.open_ended, "ff. must set the open-ended flag: {input}");
            // The definite anchor is preserved as structured information.
            assert_eq!(r.book_num, Some(43), "input: {input}");
            assert_eq!(r.chapter, Some(3), "input: {input}");
            assert_eq!(r.verse_start, Some(16), "input: {input}");
            assert_eq!(
                r.passage.as_ref().map(PassageRef::canonical),
                Some("John.3.16".to_string()),
                "anchor passage must be preserved: {input}"
            );
            // No terminal verse may be invented.
            assert_eq!(r.verse_end, None, "no terminal verse may be invented: {input}");
            assert_eq!(r.canonical(), None, "ambiguous reference has no canonical form: {input}");
        }
    }

    #[test]
    fn bare_ff_remains_ambiguous() {
        let bare = resolve("ff.").unwrap();
        assert_eq!(bare.resolution, Resolution::Ambiguous);
        assert!(bare.open_ended);
        assert_eq!(bare.book_num, None);
    }

    #[test]
    fn closed_references_remain_definite() {
        assert_eq!(resolve("John 3:16").unwrap().resolution, Resolution::Definite);
        assert_eq!(resolve("John 3:16-18").unwrap().resolution, Resolution::Definite);
        assert_eq!(resolve("John 3:16").unwrap().canonical().as_deref(), Some("John.3.16"));
    }

    // -- Verse list behavior -------------------------------------------------

    #[test]
    fn verse_lists_preserve_exact_membership() {
        let r = resolve("John 3:16,18,20").unwrap();
        assert_eq!(r.resolution, Resolution::Definite);
        assert_eq!(r.verses, vec![16, 18, 20]);
        assert!(r.passage.is_none(), "non-contiguous list has no single passage");

        let c = resolve("Romans 12:1, 2").unwrap();
        assert_eq!(c.resolution, Resolution::Definite);
        assert_eq!(c.verses, vec![1, 2]);
        assert_eq!(c.canonical().as_deref(), Some("Rom.12.1-Rom.12.2"));
    }

    // -- Single-chapter books ------------------------------------------------

    #[test]
    fn single_chapter_books_treat_a_lone_number_as_a_verse() {
        assert_eq!(resolve("II John 6").unwrap().canonical().as_deref(), Some("2John.1.6"));
        assert_eq!(resolve("III John 4").unwrap().canonical().as_deref(), Some("3John.1.4"));
        assert_eq!(resolve("Jude 25").unwrap().canonical().as_deref(), Some("Jude.1.25"));
        // Explicit chapter:verse is still accepted.
        assert_eq!(resolve("2 John 1:6").unwrap().canonical().as_deref(), Some("2John.1.6"));
    }

    // -- Source preservation -------------------------------------------------

    #[test]
    fn original_source_text_is_preserved() {
        for (input, canonical) in [
            ("I Cor. xiii. 4-7", "1Cor.13.4-1Cor.13.7"),
            ("St. John iii. 16", "John.3.16"),
            ("Rom. viii. 28", "Rom.8.28"),
            ("Deut. xxxii. 4", "Deut.32.4"),
            ("Psalm cxix.105", "Ps.119.105"),
        ] {
            let r = resolve(input).unwrap();
            assert_eq!(r.source, input, "input: {input}");
            assert_ne!(r.source, canonical, "source must differ from canonical form");
            assert_eq!(r.canonical().as_deref(), Some(canonical), "input: {input}");
        }
    }

    // -- Chapter/verse count data sanity -------------------------------------

    #[test]
    fn canonical_shape_totals_are_consistent() {
        use crate::books::{chapter_count, verse_count};
        assert_eq!(chapter_count(19), Some(150)); // Psalms
        assert_eq!(verse_count(19, 119), Some(176)); // Psalm 119
        assert_eq!(chapter_count(43), Some(21)); // John
        assert_eq!(verse_count(43, 3), Some(36)); // John 3
        assert_eq!(verse_count(63, 1), Some(13)); // 2 John
        assert_eq!(chapter_count(65), Some(1)); // Jude (single chapter)
        assert_eq!(verse_count(65, 1), Some(25)); // Jude
        assert_eq!(verse_count(43, 22), None); // chapter out of range
        assert_eq!(verse_count(1, 0), None); // chapter 0
    }
}
