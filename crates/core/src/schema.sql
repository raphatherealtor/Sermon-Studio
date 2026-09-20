-- =============================================================================
-- Sermon Studio v1.0 — Canonical Database Schema
-- =============================================================================
-- Two-vault architecture:
--   canon.db   : Static, read-only Bible & lexical engine (rebuilt from raw data)
--   pastor.db  : Writable, derived index (rebuildable from ~/Sermons/*.md)
--
-- The .md files on disk are the ONLY source of truth for sermons.
-- pastor.db may be deleted at any time and rebuilt in < 3s with zero data loss.
-- =============================================================================

PRAGMA foreign_keys = ON;

-- -----------------------------------------------------------------------------
-- CANON.DB (Static, Read-Only Bible & Lexical Engine)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS bible_books (
    book_num INTEGER PRIMARY KEY,      -- 1 to 66
    osis_id TEXT UNIQUE NOT NULL,       -- e.g., 'Gen', 'Matt', 'Rom'
    name TEXT UNIQUE NOT NULL,          -- e.g., 'Romans'
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
    strong_id TEXT PRIMARY KEY,        -- 'H1254' or 'G26'
    testament TEXT NOT NULL CHECK(testament IN ('OT', 'NT')),
    lemma TEXT NOT NULL,               -- בָּרָא or ἀγάπη
    transliteration TEXT NOT NULL,      -- 'bara' or 'agape'
    pronunciation TEXT,
    part_of_speech TEXT,
    definition TEXT NOT NULL,
    gloss TEXT NOT NULL,               -- 1-line short translation
    derivation TEXT                    -- Root word cross-reference
);

CREATE TABLE IF NOT EXISTS verse_words (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    verse_id INTEGER NOT NULL REFERENCES bible_verses(id),
    word_order INTEGER NOT NULL,
    surface_word TEXT NOT NULL,
    strong_id TEXT REFERENCES strongs_lexicon(strong_id),
    morphology TEXT                    -- MorphGNT / OSHB morphological tag
);

CREATE TABLE IF NOT EXISTS cross_references (
    from_verse_id INTEGER NOT NULL REFERENCES bible_verses(id),
    to_verse_id INTEGER NOT NULL REFERENCES bible_verses(id),
    rank INTEGER DEFAULT 1,
    PRIMARY KEY (from_verse_id, to_verse_id)
);

CREATE INDEX IF NOT EXISTS idx_bible_lookup ON bible_verses(book_num, chapter, verse);
CREATE INDEX IF NOT EXISTS idx_verse_words_strong ON verse_words(strong_id);
CREATE INDEX IF NOT EXISTS idx_verse_words_verse ON verse_words(verse_id);
CREATE INDEX IF NOT EXISTS idx_xref_from ON cross_references(from_verse_id);
CREATE INDEX IF NOT EXISTS idx_xref_to ON cross_references(to_verse_id);

-- -----------------------------------------------------------------------------
-- PASTOR.DB (Derived Writable Index & Sermon Archive)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sermon_index (
    id TEXT PRIMARY KEY,               -- UUID or slug
    file_path TEXT UNIQUE NOT NULL,    -- Relative path from ~/Sermons
    file_hash TEXT NOT NULL,           -- SHA-256 hash of on-disk .md file
    title TEXT NOT NULL,
    date_preached TEXT,                -- ISO-8601 YYYY-MM-DD
    series TEXT,
    liturgical_season TEXT,            -- e.g., 'Advent', 'Lent', 'Easter', 'Ordinary'
    primary_passage TEXT NOT NULL,      -- Normalized string (e.g., 'Rom.8.28-Rom.8.30')
    exegetical_proposition TEXT,        -- Past tense: Original authorial intent
    big_idea TEXT NOT NULL,            -- Present tense: Central homiletical takeaway
    structure_type TEXT NOT NULL,       -- 'verse_by_verse', 'narrative', 'deductive'
    last_indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- FTS5 external-content index over sermon_index. Uses the implicit INTEGER
-- rowid of sermon_index as the content rowid (id remains the TEXT primary key).
CREATE VIRTUAL TABLE IF NOT EXISTS sermons_fts USING fts5(
    title,
    primary_passage,
    big_idea,
    body_content,
    content='sermon_index',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Keep the external-content FTS index in sync with sermon_index.
CREATE TRIGGER IF NOT EXISTS sermon_index_ai AFTER INSERT ON sermon_index BEGIN
    INSERT INTO sermons_fts(rowid, title, primary_passage, big_idea, body_content)
    VALUES (new.rowid, new.title, new.primary_passage, new.big_idea, '');
END;

CREATE TRIGGER IF NOT EXISTS sermon_index_ad AFTER DELETE ON sermon_index BEGIN
    INSERT INTO sermons_fts(sermons_fts, rowid, title, primary_passage, big_idea, body_content)
    VALUES ('delete', old.rowid, old.title, old.primary_passage, old.big_idea, '');
END;

CREATE TRIGGER IF NOT EXISTS sermon_index_au AFTER UPDATE ON sermon_index BEGIN
    INSERT INTO sermons_fts(sermons_fts, rowid, title, primary_passage, big_idea, body_content)
    VALUES ('delete', old.rowid, old.title, old.primary_passage, old.big_idea, '');
    INSERT INTO sermons_fts(rowid, title, primary_passage, big_idea, body_content)
    VALUES (new.rowid, new.title, new.primary_passage, new.big_idea, '');
END;

-- Full body text is stored separately so the FTS external-content table can be
-- refreshed without re-reading disk on every query. This is a derived cache.
CREATE TABLE IF NOT EXISTS sermon_body (
    sermon_id TEXT PRIMARY KEY REFERENCES sermon_index(id) ON DELETE CASCADE,
    body_content TEXT NOT NULL
);

-- Scripture-to-sermon link graph: every verse a sermon touches.
CREATE TABLE IF NOT EXISTS scripture_sermon_links (
    sermon_id TEXT NOT NULL REFERENCES sermon_index(id) ON DELETE CASCADE,
    book_num INTEGER NOT NULL,
    chapter INTEGER NOT NULL,
    verse INTEGER NOT NULL,
    PRIMARY KEY (sermon_id, book_num, chapter, verse)
);
CREATE INDEX IF NOT EXISTS idx_ssl_verse ON scripture_sermon_links(book_num, chapter, verse);
CREATE INDEX IF NOT EXISTS idx_ssl_sermon ON scripture_sermon_links(sermon_id);

-- Illustration fatigue tracker: detects over-used illustrations across sermons.
CREATE TABLE IF NOT EXISTS illustration_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sermon_id TEXT NOT NULL REFERENCES sermon_index(id) ON DELETE CASCADE,
    illustration_key TEXT NOT NULL,     -- normalized slug/hash of the illustration
    label TEXT NOT NULL,                -- human-readable label
    first_seen TEXT,                    -- ISO-8601
    last_used TEXT,                     -- ISO-8601
    use_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE(sermon_id, illustration_key)
);
CREATE INDEX IF NOT EXISTS idx_illus_key ON illustration_usage(illustration_key);

-- Index bookkeeping / provenance.
CREATE TABLE IF NOT EXISTS index_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
