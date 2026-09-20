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
        _ => return s,
    };
    s = alias.to_string();
    s
}
