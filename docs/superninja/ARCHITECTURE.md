# Architecture

Sermon Studio is a three-layer system: a **static canon vault**, a **derived
index**, and a **thin UI** that never touches SQLite directly.

```
┌──────────────────────────────────────────────────────────────┐
│  UI (TypeScript + TipTap, WebKitGTK)                          │
│  keyboard-first · zero-modal · high-contrast                  │
└───────────────▲──────────────────────────────────────────────┘
                │ Tauri IPC (typed commands)
┌───────────────┴──────────────────────────────────────────────┐
│  src-tauri (Rust) — command layer, config, window             │
└───────────────▲──────────────────────────────────────────────┘
                │ direct function calls
┌───────────────┴──────────────────────────────────────────────┐
│  sermon_core (Rust library)                                   │
│   reference · sermon · canon · indexer · retrieval · librarian│
└───────▲───────────────────────────────▲──────────────────────┘
        │ read-only                     │ read/write (derived)
┌───────┴────────┐              ┌───────┴────────┐
│   canon.db     │              │   pastor.db    │
│  (static)      │              │  (rebuildable) │
└────────────────┘              └────────────────┘
        ▲                               ▲
        │ build-canon                   │ index / sync
   data/clean/*.tsv                ~/Sermons/*.md
        ▲
        │ tools/etl.py
   data/raw/*  (public-domain sources)
```

## Layer 1 — `sermon_core`

The engine is a pure Rust library with no UI and no network dependencies. Its
modules:

| Module | Responsibility |
| --- | --- |
| `books.rs` | The 66-book table (book number, OSIS id, name, testament) and tolerant name normalization ("I Chronicles" → "1 Chronicles", "Revelation of John" → "Revelation"). |
| `reference.rs` | Scripture reference parsing and normalization. Accepts `Rom 8:28`, `Romans 8:28-30`, `1 Cor 13:4-7`, `Psalm 23`, and cross-chapter ranges. Canonical form is dotted: `Rom.8.28-Rom.8.30`. |
| `sermon.rs` | Frontmatter + body parsing, SHA-256 hashing, slug generation, reference extraction from prose, and the new-sermon template. |
| `canon.rs` | Builds `canon.db` from normalized TSVs. Idempotent. |
| `indexer.rs` | Builds and incrementally syncs `pastor.db`. Full rebuild wipes derived tables; sync skips files whose hash is unchanged. |
| `retrieval.rs` | FTS5 search with `bm25()` ranking and `snippet()` highlighting; verse/passage/lexicon/Strong's lookups; the scripture→sermon graph; illustration fatigue; archive stats. |
| `librarian.rs` | Offline bag-of-words cosine similarity for related-sermon discovery. No model weights, no network. |

## Layer 2 — `src-tauri`

A Tauri 2.0 shell. Every IPC command is a thin, synchronous bridge to
`sermon_core`; the frontend never sees SQL. Commands are deliberately granular
so the UI can stay modal-free.

Configuration lives in `~/.config/sermon-studio/config.json`:

```json
{
  "vault_path": "/home/pastor/Sermons",
  "canon_path": "/home/pastor/.local/share/sermon-studio/canon.db",
  "pastor_path": "/home/pastor/.local/share/sermon-studio/pastor.db",
  "librarian_enabled": false,
  "font_size": 17,
  "high_contrast": true
}
```

## Layer 3 — `ui`

TypeScript + TipTap with the Markdown extension. The editor round-trips
CommonMark, so what you write is exactly what lands on disk. The layout is a
three-column grid (archive · editor · study) with a status bar and a
command-palette overlay.

## Data flow: saving a sermon

1. The editor serializes frontmatter + body to CommonMark.
2. `save_sermon` writes the `.md` file to the vault.
3. The same command re-parses the document and calls `index_single`, which
   updates `sermon_index`, `sermons_fts`, `scripture_sermon_links`, and
   `illustration_usage` in one transaction.
4. The archive list refreshes.

Because the file is written *before* indexing, a crash mid-index loses nothing:
the next `sync` reconciles from disk.

## Schema notes

The spec's FTS5 definition used an external-content table
(`content='sermon_index'`) referencing a `body_content` column that does not
exist on `sermon_index`. Sermon Studio resolves this with a **self-contained**
FTS5 table:

```sql
CREATE VIRTUAL TABLE sermons_fts USING fts5(
    sermon_id UNINDEXED,
    title,
    primary_passage,
    big_idea,
    body_content,
    tokenize='porter unicode61'
);
```

This keeps the index rebuildable (it is repopulated from `sermon_index` +
`sermon_body` on every rebuild) while avoiding the phantom-column problem. The
`porter unicode61` tokenizer gives stemming ("weaver"/"weaving") and Unicode
support for transliterated Greek/Hebrew.

## Performance

| Operation | Archive size | Time |
| --- | --- | --- |
| Full rebuild | 203 sermons | ~359 ms |
| Incremental sync (no changes) | 203 sermons | ~9 ms |
| FTS search | 203 sermons | < 5 ms |
| Verse + Strong's + xrefs | canon.db (139 MB) | < 10 ms |

The release profile uses `opt-level="z"`, LTO, `codegen-units=1`, `panic=abort`,
and `strip=true` to keep the binary small and the cold start fast.
