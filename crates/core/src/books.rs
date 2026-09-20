//! Canonical ordering of the 66 books of the Protestant Bible, with OSIS ids
//! and common display names. Used to normalize scripture references and to
//! populate `bible_books` in canon.db.

/// (book_num, osis_id, display_name, testament)
pub const BOOKS: &[(i64, &str, &str, &str)] = &[
    (1, "Gen", "Genesis", "OT"),
    (2, "Exod", "Exodus", "OT"),
    (3, "Lev", "Leviticus", "OT"),
    (4, "Num", "Numbers", "OT"),
    (5, "Deut", "Deuteronomy", "OT"),
    (6, "Josh", "Joshua", "OT"),
    (7, "Judg", "Judges", "OT"),
    (8, "Ruth", "Ruth", "OT"),
    (9, "1Sam", "1 Samuel", "OT"),
    (10, "2Sam", "2 Samuel", "OT"),
    (11, "1Kgs", "1 Kings", "OT"),
    (12, "2Kgs", "2 Kings", "OT"),
    (13, "1Chr", "1 Chronicles", "OT"),
    (14, "2Chr", "2 Chronicles", "OT"),
    (15, "Ezra", "Ezra", "OT"),
    (16, "Neh", "Nehemiah", "OT"),
    (17, "Esth", "Esther", "OT"),
    (18, "Job", "Job", "OT"),
    (19, "Ps", "Psalms", "OT"),
    (20, "Prov", "Proverbs", "OT"),
    (21, "Eccl", "Ecclesiastes", "OT"),
    (22, "Song", "Song of Solomon", "OT"),
    (23, "Isa", "Isaiah", "OT"),
    (24, "Jer", "Jeremiah", "OT"),
    (25, "Lam", "Lamentations", "OT"),
    (26, "Ezek", "Ezekiel", "OT"),
    (27, "Dan", "Daniel", "OT"),
    (28, "Hos", "Hosea", "OT"),
    (29, "Joel", "Joel", "OT"),
    (30, "Amos", "Amos", "OT"),
    (31, "Obad", "Obadiah", "OT"),
    (32, "Jonah", "Jonah", "OT"),
    (33, "Mic", "Micah", "OT"),
    (34, "Nah", "Nahum", "OT"),
    (35, "Hab", "Habakkuk", "OT"),
    (36, "Zeph", "Zephaniah", "OT"),
    (37, "Hag", "Haggai", "OT"),
    (38, "Zech", "Zechariah", "OT"),
    (39, "Mal", "Malachi", "OT"),
    (40, "Matt", "Matthew", "NT"),
    (41, "Mark", "Mark", "NT"),
    (42, "Luke", "Luke", "NT"),
    (43, "John", "John", "NT"),
    (44, "Acts", "Acts", "NT"),
    (45, "Rom", "Romans", "NT"),
    (46, "1Cor", "1 Corinthians", "NT"),
    (47, "2Cor", "2 Corinthians", "NT"),
    (48, "Gal", "Galatians", "NT"),
    (49, "Eph", "Ephesians", "NT"),
    (50, "Phil", "Philippians", "NT"),
    (51, "Col", "Colossians", "NT"),
    (52, "1Thess", "1 Thessalonians", "NT"),
    (53, "2Thess", "2 Thessalonians", "NT"),
    (54, "1Tim", "1 Timothy", "NT"),
    (55, "2Tim", "2 Timothy", "NT"),
    (56, "Titus", "Titus", "NT"),
    (57, "Phlm", "Philemon", "NT"),
    (58, "Heb", "Hebrews", "NT"),
    (59, "Jas", "James", "NT"),
    (60, "1Pet", "1 Peter", "NT"),
    (61, "2Pet", "2 Peter", "NT"),
    (62, "1John", "1 John", "NT"),
    (63, "2John", "2 John", "NT"),
    (64, "3John", "3 John", "NT"),
    (65, "Jude", "Jude", "NT"),
    (66, "Rev", "Revelation", "NT"),
];

/// Look up a book by its OSIS id (case-insensitive).
pub fn book_by_osis(osis: &str) -> Option<(i64, &'static str, &'static str, &'static str)> {
    BOOKS
        .iter()
        .find(|b| b.1.eq_ignore_ascii_case(osis))
        .copied()
}

/// Look up a book by its display name (case-insensitive, tolerant of spacing).
pub fn book_by_name(name: &str) -> Option<(i64, &'static str, &'static str, &'static str)> {
    let norm = normalize_book_name(name);
    BOOKS
        .iter()
        .find(|b| normalize_book_name(b.2) == norm || b.1.eq_ignore_ascii_case(&norm))
        .copied()
}

/// Normalize a book name for tolerant matching: lowercase, strip punctuation
/// and spaces, expand common abbreviations.
pub fn normalize_book_name(name: &str) -> String {
    let mut s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    // Strip a leading "st" (Saint) marker, e.g. "St. John" -> "john".
    if s.len() > 2 && s.starts_with("st") {
        s = s[2..].to_string();
    }
    // Common abbreviations -> canonical display name (normalized).
    let alias = match s.as_str() {
        "psalm" | "psa" | "pslm" | "psm" => "psalms",
        "songofsongs" | "songofsolomon" | "canticles" | "song" => "songofsolomon",
        "revelations" | "rev" | "apocalypse" => "revelation",
        "ecclesiastes" | "eccles" | "ecc" => "ecclesiastes",
        "philippians" | "phil" | "php" => "philippians",
        "philemon" | "phlm" | "phm" => "philemon",
        "james" | "jas" | "jam" => "james",
        "hebrews" | "heb" => "hebrews",
        "matthew" | "matt" | "mt" => "matthew",
        "mark" | "mk" | "mrk" => "mark",
        "luke" | "lk" | "luk" => "luke",
        "john" | "jn" | "jhn" => "john",
        "acts" | "act" => "acts",
        "romans" | "rom" => "romans",
        "galatians" | "gal" => "galatians",
        "ephesians" | "eph" => "ephesians",
        "colossians" | "col" => "colossians",
        "titus" | "tit" => "titus",
        "jude" | "jud" => "jude",
        "genesis" | "gen" => "genesis",
        "exodus" | "exod" | "ex" => "exodus",
        "leviticus" | "lev" => "leviticus",
        "numbers" | "num" => "numbers",
        "deuteronomy" | "deut" | "dt" => "deuteronomy",
        "joshua" | "josh" => "joshua",
        "judges" | "judg" => "judges",
        "ruth" | "rut" => "ruth",
        "isaiah" | "isa" => "isaiah",
        "jeremiah" | "jer" => "jeremiah",
        "lamentations" | "lam" => "lamentations",
        "ezekiel" | "ezek" | "eze" => "ezekiel",
        "daniel" | "dan" => "daniel",
        "hosea" | "hos" => "hosea",
        "joel" | "joe" => "joel",
        "amos" | "amo" => "amos",
        "obadiah" | "obad" => "obadiah",
        "jonah" | "jon" => "jonah",
        "micah" | "mic" => "micah",
        "nahum" | "nah" => "nahum",
        "habakkuk" | "hab" => "habakkuk",
        "zephaniah" | "zeph" | "zep" => "zephaniah",
        "haggai" | "hag" => "haggai",
        "zechariah" | "zech" | "zec" => "zechariah",
        "malachi" | "mal" => "malachi",
        "ezra" | "ezr" => "ezra",
        "nehemiah" | "neh" => "nehemiah",
        "esther" | "esth" | "est" => "esther",
        "job" | "jb" => "job",
        "proverbs" | "prov" | "pro" => "proverbs",
        "1corinthians" | "1cor" | "1co" => "1corinthians",
        "2corinthians" | "2cor" | "2co" => "2corinthians",
        "1thessalonians" | "1thess" | "1th" => "1thessalonians",
        "2thessalonians" | "2thess" | "2th" => "2thessalonians",
        "1timothy" | "1tim" | "1ti" => "1timothy",
        "2timothy" | "2tim" | "2ti" => "2timothy",
        "1peter" | "1pet" | "1pe" => "1peter",
        "2peter" | "2pet" | "2pe" => "2peter",
        "1john" | "1jn" | "1jo" => "1john",
        "2john" | "2jn" | "2jo" => "2john",
        "3john" | "3jn" | "3jo" => "3john",
        "1kings" | "1kgs" | "1ki" => "1kings",
        "2kings" | "2kgs" | "2ki" => "2kings",
        "1samuel" | "1sam" | "1sa" => "1samuel",
        "2samuel" | "2sam" | "2sa" => "2samuel",
        "1chronicles" | "1chr" | "1ch" => "1chronicles",
        "2chronicles" | "2chr" | "2ch" => "2chronicles",
        // Roman-numeral book prefixes (I/II/III == 1/2/3), e.g. "I Cor.", "II John".
        "icor" | "icorinthians" | "ico" => "1corinthians",
        "iicor" | "iicorinthians" | "iico" => "2corinthians",
        "ikings" | "ikgs" | "iki" => "1kings",
        "iikings" | "iikgs" | "iiki" => "2kings",
        "isamuel" | "isam" => "1samuel",
        "iisamuel" | "iisam" => "2samuel",
        "ichronicles" | "ichr" | "ich" => "1chronicles",
        "iichronicles" | "iichr" | "iich" => "2chronicles",
        "ithes" | "ithess" | "ithessalonians" | "ith" => "1thessalonians",
        "iithes" | "iithess" | "iithessalonians" | "iith" => "2thessalonians",
        "itim" | "itimothy" | "iti" => "1timothy",
        "iitim" | "iitimothy" | "iiti" => "2timothy",
        "ipet" | "ipeter" | "ipe" => "1peter",
        "iipet" | "iipeter" | "iipe" => "2peter",
        "ijohn" | "ijn" | "ijo" => "1john",
        "iijohn" | "iijn" | "iijo" => "2john",
        "iiijohn" | "iiijn" | "iiijo" => "3john",
        _ => return s,
    };
    s = alias.to_string();
    s
}

// ---- Canon shape: KJV chapter/verse counts ---------------------------------
//
// Authoritative counts for the 1769 KJV canon. These let the reference parser
// reject out-of-range chapters and verses (INVALID) instead of silently clamping
// them. Totals: 66 books, 1189 chapters, 31102 verses.

/// Number of chapters in each book, indexed by `book_num - 1`.
pub const CHAPTER_COUNTS: &[u16; 66] = &[
    50, 40, 27, 36, 34, 24, 21, 4, 31, 24, 22, 25,
    29, 36, 10, 13, 10, 42, 150, 31, 12, 8, 66, 52,
    5, 48, 12, 14, 3, 9, 1, 4, 7, 3, 3, 3,
    2, 14, 4, 28, 16, 24, 21, 28, 16, 16, 13, 6,
    6, 4, 4, 5, 3, 6, 4, 3, 1, 13, 5, 5,
    3, 5, 1, 1, 1, 22,
];

/// Verse count per chapter, flattened in (book_num, chapter) order.
pub const VERSES_PER_CHAPTER: &[u16] = &[
    31, 25, 24, 26, 32, 22, 24, 22, 29, 32, 32, 20,
    18, 24, 21, 16, 27, 33, 38, 18, 34, 24, 20, 67,
    34, 35, 46, 22, 35, 43, 55, 32, 20, 31, 29, 43,
    36, 30, 23, 23, 57, 38, 34, 34, 28, 34, 31, 22,
    33, 26, 22, 25, 22, 31, 23, 30, 25, 32, 35, 29,
    10, 51, 22, 31, 27, 36, 16, 27, 25, 26, 36, 31,
    33, 18, 40, 37, 21, 43, 46, 38, 18, 35, 23, 35,
    35, 38, 29, 31, 43, 38, 17, 16, 17, 35, 19, 30,
    38, 36, 24, 20, 47, 8, 59, 57, 33, 34, 16, 30,
    37, 27, 24, 33, 44, 23, 55, 46, 34, 54, 34, 51,
    49, 31, 27, 89, 26, 23, 36, 35, 16, 33, 45, 41,
    50, 13, 32, 22, 29, 35, 41, 30, 25, 18, 65, 23,
    31, 40, 16, 54, 42, 56, 29, 34, 13, 46, 37, 29,
    49, 33, 25, 26, 20, 29, 22, 32, 32, 18, 29, 23,
    22, 20, 22, 21, 20, 23, 30, 25, 22, 19, 19, 26,
    68, 29, 20, 30, 52, 29, 12, 18, 24, 17, 24, 15,
    27, 26, 35, 27, 43, 23, 24, 33, 15, 63, 10, 18,
    28, 51, 9, 45, 34, 16, 33, 36, 23, 31, 24, 31,
    40, 25, 35, 57, 18, 40, 15, 25, 20, 20, 31, 13,
    31, 30, 48, 25, 22, 23, 18, 22, 28, 36, 21, 22,
    12, 21, 17, 22, 27, 27, 15, 25, 23, 52, 35, 23,
    58, 30, 24, 42, 15, 23, 29, 22, 44, 25, 12, 25,
    11, 31, 13, 27, 32, 39, 12, 25, 23, 29, 18, 13,
    19, 27, 31, 39, 33, 37, 23, 29, 33, 43, 26, 22,
    51, 39, 25, 53, 46, 28, 34, 18, 38, 51, 66, 28,
    29, 43, 33, 34, 31, 34, 34, 24, 46, 21, 43, 29,
    53, 18, 25, 27, 44, 27, 33, 20, 29, 37, 36, 21,
    21, 25, 29, 38, 20, 41, 37, 37, 21, 26, 20, 37,
    20, 30, 54, 55, 24, 43, 26, 81, 40, 40, 44, 14,
    47, 40, 14, 17, 29, 43, 27, 17, 19, 8, 30, 19,
    32, 31, 31, 32, 34, 21, 30, 17, 18, 17, 22, 14,
    42, 22, 18, 31, 19, 23, 16, 22, 15, 19, 14, 19,
    34, 11, 37, 20, 12, 21, 27, 28, 23, 9, 27, 36,
    27, 21, 33, 25, 33, 27, 23, 11, 70, 13, 24, 17,
    22, 28, 36, 15, 44, 11, 20, 32, 23, 19, 19, 73,
    18, 38, 39, 36, 47, 31, 22, 23, 15, 17, 14, 14,
    10, 17, 32, 3, 22, 13, 26, 21, 27, 30, 21, 22,
    35, 22, 20, 25, 28, 22, 35, 22, 16, 21, 29, 29,
    34, 30, 17, 25, 6, 14, 23, 28, 25, 31, 40, 22,
    33, 37, 16, 33, 24, 41, 30, 24, 34, 17, 6, 12,
    8, 8, 12, 10, 17, 9, 20, 18, 7, 8, 6, 7,
    5, 11, 15, 50, 14, 9, 13, 31, 6, 10, 22, 12,
    14, 9, 11, 12, 24, 11, 22, 22, 28, 12, 40, 22,
    13, 17, 13, 11, 5, 26, 17, 11, 9, 14, 20, 23,
    19, 9, 6, 7, 23, 13, 11, 11, 17, 12, 8, 12,
    11, 10, 13, 20, 7, 35, 36, 5, 24, 20, 28, 23,
    10, 12, 20, 72, 13, 19, 16, 8, 18, 12, 13, 17,
    7, 18, 52, 17, 16, 15, 5, 23, 11, 13, 12, 9,
    9, 5, 8, 28, 22, 35, 45, 48, 43, 13, 31, 7,
    10, 10, 9, 8, 18, 19, 2, 29, 176, 7, 8, 9,
    4, 8, 5, 6, 5, 6, 8, 8, 3, 18, 3, 3,
    21, 26, 9, 8, 24, 13, 10, 7, 12, 15, 21, 10,
    20, 14, 9, 6, 33, 22, 35, 27, 23, 35, 27, 36,
    18, 32, 31, 28, 25, 35, 33, 33, 28, 24, 29, 30,
    31, 29, 35, 34, 28, 28, 27, 28, 27, 33, 31, 18,
    26, 22, 16, 20, 12, 29, 17, 18, 20, 10, 14, 17,
    17, 11, 16, 16, 13, 13, 14, 31, 22, 26, 6, 30,
    13, 25, 22, 21, 34, 16, 6, 22, 32, 9, 14, 14,
    7, 25, 6, 17, 25, 18, 23, 12, 21, 13, 29, 24,
    33, 9, 20, 24, 17, 10, 22, 38, 22, 8, 31, 29,
    25, 28, 28, 25, 13, 15, 22, 26, 11, 23, 15, 12,
    17, 13, 12, 21, 14, 21, 22, 11, 12, 19, 12, 25,
    24, 19, 37, 25, 31, 31, 30, 34, 22, 26, 25, 23,
    17, 27, 22, 21, 21, 27, 23, 15, 18, 14, 30, 40,
    10, 38, 24, 22, 17, 32, 24, 40, 44, 26, 22, 19,
    32, 21, 28, 18, 16, 18, 22, 13, 30, 5, 28, 7,
    47, 39, 46, 64, 34, 22, 22, 66, 22, 22, 28, 10,
    27, 17, 17, 14, 27, 18, 11, 22, 25, 28, 23, 23,
    8, 63, 24, 32, 14, 49, 32, 31, 49, 27, 17, 21,
    36, 26, 21, 26, 18, 32, 33, 31, 15, 38, 28, 23,
    29, 49, 26, 20, 27, 31, 25, 24, 23, 35, 21, 49,
    30, 37, 31, 28, 28, 27, 27, 21, 45, 13, 11, 23,
    5, 19, 15, 11, 16, 14, 17, 15, 12, 14, 16, 9,
    20, 32, 21, 15, 16, 15, 13, 27, 14, 17, 14, 15,
    21, 17, 10, 10, 11, 16, 13, 12, 13, 15, 16, 20,
    15, 13, 19, 17, 20, 19, 18, 15, 20, 15, 23, 21,
    13, 10, 14, 11, 15, 14, 23, 17, 12, 17, 14, 9,
    21, 14, 17, 18, 6, 25, 23, 17, 25, 48, 34, 29,
    34, 38, 42, 30, 50, 58, 36, 39, 28, 27, 35, 30,
    34, 46, 46, 39, 51, 46, 75, 66, 20, 45, 28, 35,
    41, 43, 56, 37, 38, 50, 52, 33, 44, 37, 72, 47,
    20, 80, 52, 38, 44, 39, 49, 50, 56, 62, 42, 54,
    59, 35, 35, 32, 31, 37, 43, 48, 47, 38, 71, 56,
    53, 51, 25, 36, 54, 47, 71, 53, 59, 41, 42, 57,
    50, 38, 31, 27, 33, 26, 40, 42, 31, 25, 26, 47,
    26, 37, 42, 15, 60, 40, 43, 48, 30, 25, 52, 28,
    41, 40, 34, 28, 41, 38, 40, 30, 35, 27, 27, 32,
    44, 31, 32, 29, 31, 25, 21, 23, 25, 39, 33, 21,
    36, 21, 14, 23, 33, 27, 31, 16, 23, 21, 13, 20,
    40, 13, 27, 33, 34, 31, 13, 40, 58, 24, 24, 17,
    18, 18, 21, 18, 16, 24, 15, 18, 33, 21, 14, 24,
    21, 29, 31, 26, 18, 23, 22, 21, 32, 33, 24, 30,
    30, 21, 23, 29, 23, 25, 18, 10, 20, 13, 18, 28,
    12, 17, 18, 20, 15, 16, 16, 25, 21, 18, 26, 17,
    22, 16, 15, 15, 25, 14, 18, 19, 16, 14, 20, 28,
    13, 28, 39, 40, 29, 25, 27, 26, 18, 17, 20, 25,
    25, 22, 19, 14, 21, 22, 18, 10, 29, 24, 21, 21,
    13, 14, 25, 20, 29, 22, 11, 14, 17, 17, 13, 21,
    11, 19, 17, 18, 20, 8, 21, 18, 24, 21, 15, 27,
    21,
];

/// Number of chapters in a book (1-based book_num), if known.
pub fn chapter_count(book_num: i64) -> Option<u16> {
    let idx = book_num.checked_sub(1)? as usize;
    CHAPTER_COUNTS.get(idx).copied()
}

/// Number of verses in a chapter (1-based book_num and chapter), if known.
pub fn verse_count(book_num: i64, chapter: i64) -> Option<u16> {
    if chapter < 1 {
        return None;
    }
    let cc = chapter_count(book_num)?;
    if chapter as u16 > cc {
        return None;
    }
    let offset = chapter_offset(book_num)? + (chapter as usize - 1);
    VERSES_PER_CHAPTER.get(offset).copied()
}

/// Flattened index of a book's first chapter within `VERSES_PER_CHAPTER`.
fn chapter_offset(book_num: i64) -> Option<usize> {
    let idx = book_num.checked_sub(1)? as usize;
    if idx >= CHAPTER_COUNTS.len() {
        return None;
    }
    Some(CHAPTER_COUNTS[..idx].iter().map(|&c| c as usize).sum())
}
