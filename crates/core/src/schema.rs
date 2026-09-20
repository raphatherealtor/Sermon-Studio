//! Embedded SQL schema. The canonical schema lives in `schema.sql`; the canon
//! and pastor vaults share the same file but are created with the relevant
//! subset of statements.

/// The full schema text (both vaults).
pub const FULL_SCHEMA: &str = include_str!("schema.sql");

/// Statements that belong to canon.db (static vault).
pub const CANON_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS bible_books (
    book_num INTEGER PRIMARY KEY,
    osis_id TEXT UNIQUE NOT NULL,
    name TEXT UNIQUE NOT NULL,
    testament TEXT NOT NULL CHECK(testament IN ('OT', 'NT')),
    canonical_order INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS bible_verses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_num INTEGER NOT NULL REFERENCES bible_books(book_num),
    chapter INTEGER NOT NULL,
    verse INTEGER NOT NULL,
    text_kjv TEXT NOT NULL,
    UNIQUE(book_num, chapter, verse)
);
CREATE TABLE IF NOT EXISTS strongs_lexicon (
    strong_id TEXT PRIMARY KEY,
    testament TEXT NOT NULL CHECK(testament IN ('OT', 'NT')),
    lemma TEXT NOT NULL,
    transliteration TEXT NOT NULL,
    pronunciation TEXT,
    part_of_speech TEXT,
    definition TEXT NOT NULL,
    gloss TEXT NOT NULL,
    derivation TEXT
);
CREATE TABLE IF NOT EXISTS verse_words (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    verse_id INTEGER NOT NULL REFERENCES bible_verses(id),
    word_order INTEGER NOT NULL,
    surface_word TEXT NOT NULL,
    strong_id TEXT REFERENCES strongs_lexicon(strong_id),
    morphology TEXT
);
CREATE TABLE IF NOT EXISTS cross_references (
    from_verse_id INTEGER NOT NULL REFERENCES bible_verses(id),
    to_verse_id INTEGER NOT NULL REFERENCES bible_verses(id),
    rank INTEGER DEFAULT 1,
    PRIMARY KEY (from_verse_id, to_verse_id)
);
"#;

/// Statements that belong to pastor.db (derived vault).
pub const PASTOR_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sermon_index (
    id TEXT PRIMARY KEY,
    file_path TEXT UNIQUE NOT NULL,
    file_hash TEXT NOT NULL,
    title TEXT NOT NULL,
    date_preached TEXT,
    series TEXT,
    liturgical_season TEXT,
    primary_passage TEXT NOT NULL,
    exegetical_proposition TEXT,
    big_idea TEXT NOT NULL,
    structure_type TEXT NOT NULL,
    last_indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- NOTE ON THE FTS DEFINITION:
-- The v1.0 spec sketched an external-content FTS5 table (content='sermon_index')
-- whose column list included `body_content`, a column that does not exist on
-- `sermon_index`. External-content FTS5 requires every indexed column to exist
-- on the content table, so that definition cannot be queried as written.
-- We resolve it with a self-contained FTS5 table: it stores its own copy of the
-- text (pastor.db is a derived, rebuildable cache, so duplication is free) and
-- therefore supports snippet(), bm25() and porter stemming without a join.
-- The indexer keeps it in sync explicitly.
CREATE VIRTUAL TABLE IF NOT EXISTS sermons_fts USING fts5(
    sermon_id UNINDEXED,
    title,
    primary_passage,
    big_idea,
    body_content,
    tokenize='porter unicode61'
);
CREATE TABLE IF NOT EXISTS sermon_body (
    sermon_id TEXT PRIMARY KEY REFERENCES sermon_index(id) ON DELETE CASCADE,
    body_content TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS scripture_sermon_links (
    sermon_id TEXT NOT NULL REFERENCES sermon_index(id) ON DELETE CASCADE,
    book_num INTEGER NOT NULL,
    chapter INTEGER NOT NULL,
    verse INTEGER NOT NULL,
    PRIMARY KEY (sermon_id, book_num, chapter, verse)
);
CREATE INDEX IF NOT EXISTS idx_ssl_verse ON scripture_sermon_links(book_num, chapter, verse);
CREATE INDEX IF NOT EXISTS idx_ssl_sermon ON scripture_sermon_links(sermon_id);
CREATE TABLE IF NOT EXISTS illustration_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sermon_id TEXT NOT NULL REFERENCES sermon_index(id) ON DELETE CASCADE,
    illustration_key TEXT NOT NULL,
    label TEXT NOT NULL,
    first_seen TEXT,
    last_used TEXT,
    use_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE(sermon_id, illustration_key)
);
CREATE INDEX IF NOT EXISTS idx_illus_key ON illustration_usage(illustration_key);
CREATE TABLE IF NOT EXISTS index_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;
