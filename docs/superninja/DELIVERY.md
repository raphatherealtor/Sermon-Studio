# Sermon Studio v1.0 — Delivery Summary

**An offline, Linux-native expository sermon writing studio, compiler, and
retrieval engine.** Built to the specification: plain-text-first, data-sovereign,
keyboard-driven, zero-modal, with an invisible off-by-default AI cataloger.

---

## What was delivered

### 1. The engine — `sermon_core` (Rust library)

A pure, UI-free, network-free library. Modules: `books`, `reference`, `sermon`,
`canon`, `indexer`, `retrieval`, `librarian`. **13/13 unit tests pass.**

### 2. The headless CLI — `sermon`

Subcommands: `build-canon`, `index`, `sync`, `search`, `verse`, `strong`,
`preached`, `fatigue`, `stats`, `new`.

### 3. The desktop app — Tauri 2.0

Rust IPC command layer (20 commands) + TypeScript/TipTap frontend. Three-column
keyboard-first layout, command palette, zero modal dialogs, high-contrast theme.

### 4. The canon vault — `canon.db` (139 MB, static, read-only)

| Table | Rows |
| --- | --- |
| `bible_books` | 66 |
| `bible_verses` (KJV) | 31,102 |
| `strongs_lexicon` | 14,197 |
| `verse_words` (Strong's-tagged) | 2,774,190 |
| `cross_references` (TSK/OpenBible) | 256,648 |

### 5. The derived index — `pastor.db` (rebuildable)

Sermon metadata, FTS5 full-text index, scripture→sermon link graph,
illustration-fatigue tracker. **Delete it and it rebuilds from disk.**

### 6. Packages

| Artifact | Size | Path |
| --- | --- | --- |
| Debian package | 2.6 MB | `target/release/bundle/deb/Sermon Studio_1.0.0_amd64.deb` |
| AppImage | 93 MB | `target/release/bundle/appimage/Sermon Studio_1.0.0_amd64.AppImage` |
| Release binary | 5.7 MB | `target/release/sermon-studio` |

---

## Verification results

### Functional (end-to-end)

```
$ sermon stats --db pastor.db
sermons: 203 · series: 15 · verse links: 397 · illustrations: 204

$ sermon verse "Rom 8:28" --canon canon.db
Romans 8:28  And we know that all things work together for good...
  -- Strong's words --  (G1161, G1492, G3956, G4903, G18, G25, G2316, G2822...)
  -- Cross references --  (496) 1 Peter 5:10 · (396) James 1:12 · (320) Gen 50:20

$ sermon strong G26 --canon canon.db
G26  ἀγάπη  (agápē)  gloss: (feast of) charity(-ably), dear, love

$ sermon search "shepherd" --db pastor.db
• The Shepherd Who Leads  [Ps.23.1-Ps.23.6]  preached: 2022-05-01
    # The <<Shepherd>> Who Leads ...

$ sermon preached "Rom 8:28" --db pastor.db
• Groaning for Glory  [Rom.8.28-Rom.8.30]  2024-07-14

$ sermon fatigue --db pastor.db --min 3
33×  The weaver and the tapestry  (33 sermons)
24×  The lost coin  (24 sermons)
22×  The good shepherd  (22 sermons)
```

### Performance (spec: rebuild < 3 s)

| Operation | Archive | Measured |
| --- | --- | --- |
| Full rebuild from disk | 203 sermons | **359 ms** |
| Incremental sync (no changes) | 203 sermons | **9 ms** |
| FTS search | 203 sermons | < 5 ms |
| Verse + Strong's + xrefs | canon.db | < 10 ms |

### Zero-data-loss guarantee

```
$ rm pastor.db
$ sermon index --vault ~/Sermons --db pastor.db
indexed 203 sermons (203 scanned, 397 links, 204 illustrations) in 354 ms
$ sermon stats --db pastor.db
sermons: 203 · series: 15 · verse links: 397 · illustrations: 204   ← identical
```

### Build verification

* `cargo test -p sermon_core` → **13 passed, 0 failed**
* `cargo build` (Tauri app) → **success**
* `cargo tauri build --bundles deb` → **.deb produced**
* `cargo tauri build --bundles appimage` → **AppImage produced**
* AppImage `--appimage-extract` → valid squashfs, stripped 5.7 MB binary
* Cold-start smoke test under Xvfb → launches, GTK initializes cleanly, no crash
* Frontend assets confirmed embedded in the binary (`index-*.js`, `index-*.css`)

---

## Spec deviations (documented)

**FTS5 external-content → self-contained.** The spec's `sermons_fts` used
`content='sermon_index'` with a `body_content` column that does not exist on
`sermon_index`. This is unsatisfiable in SQLite. Resolved with a self-contained
FTS5 table (`sermon_id UNINDEXED, title, primary_passage, big_idea,
body_content, tokenize='porter unicode61'`), repopulated from `sermon_index` +
`sermon_body` on every rebuild. This preserves the rebuildability guarantee and
adds porter stemming. See `docs/ARCHITECTURE.md`.

---

## Repository layout

```
sermon-studio/
├── crates/core/        sermon_core engine (+ schema.sql, 13 tests)
├── crates/cli/         `sermon` headless CLI
├── src-tauri/          Tauri 2.0 shell (commands.rs, config.rs, lib.rs)
├── ui/                 TypeScript + TipTap frontend (Vite)
├── tools/etl.py        Raw → clean TSV normalizer
├── scripts/            build-canon.sh · build-app.sh · rebuild-index.sh
├── docs/               ARCHITECTURE.md · USER_GUIDE.md · DATA_SOURCES.md
├── preview/index.html  Static UI preview (no backend needed)
├── Makefile
└── README.md
```

## How to run

```bash
# Headless CLI
cargo build --release -p sermon
./target/release/sermon verse "John 3:16" --canon data/canon.db

# Desktop app (from source)
scripts/build-app.sh

# Install the .deb
sudo dpkg -i "target/release/bundle/deb/Sermon Studio_1.0.0_amd64.deb"

# Run the AppImage
chmod +x "target/release/bundle/appimage/Sermon Studio_1.0.0_amd64.AppImage"
"./target/release/bundle/appimage/Sermon Studio_1.0.0_amd64.AppImage"
```

## Data provenance

KJV text and Strong's lexicons are public domain. Cross-references are
OpenBible.info (CC-BY). Full attribution in `docs/DATA_SOURCES.md`.
