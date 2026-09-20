# Sermon Studio

**An offline, Linux-native expository sermon writing studio, compiler, and retrieval engine.**

Sermon Studio is built for the working expositor: a plain-text-first writing
environment backed by a static Bible/lexicon canon and a rebuildable retrieval
index. It is 100% offline, has zero telemetry, and makes no runtime network
calls. The optional "Librarian" cataloger is local-only and **off by default**.

---

## Why it exists

A 30-year pastoral archive is a scholarly asset. It should not be locked inside a
proprietary database or a subscription service. Sermon Studio treats your sermons
as what they are: **plain CommonMark files on disk**, with YAML frontmatter. The
SQLite databases are *derived caches* — delete them and they rebuild from disk in
well under three seconds, with zero data loss.

| Principle | How it is honored |
| --- | --- |
| Data sovereignty | Sermons are `.md` files in *your* folder. Nothing is uploaded. |
| Plain text | CommonMark + YAML frontmatter. Readable in `vim`, `grep`, `git`. |
| Low RAM | WebKitGTK frontend, no Electron, no Node runtime at run time. |
| Low friction | Zero modal popups while writing; keyboard-first; high contrast. |
| Invisible AI | No chatbot. The Librarian is an optional, local, off-by-default cataloger. |

---

## Architecture

```
sermon-studio/
├── crates/
│   ├── core/        # sermon_core — the engine (no UI, no network)
│   └── cli/         # `sermon` — headless CLI over the same engine
├── src-tauri/       # Tauri 2.0 desktop shell (Rust IPC + WebKitGTK)
├── ui/              # TypeScript + TipTap frontend (Vite)
├── tools/etl.py     # Normalizes raw public-domain sources -> clean TSVs
├── scripts/         # build-canon.sh, build-app.sh, rebuild-index.sh
└── docs/            # ARCHITECTURE.md, USER_GUIDE.md, DATA_SOURCES.md
```

### Dual-Vault design

Sermon Studio separates **static canon** from **derived index**:

* **`canon.db`** — *static, read-only.* KJV text (31,102 verses), Strong's
  Greek/Hebrew lexicon (14,197 entries), per-word Strong's tagging
  (2,774,190 verse-words), and TSK/OpenBible cross-references (256,648 links).
  Built once from public-domain sources; never written at run time.

* **`pastor.db`** — *derived, writable, disposable.* Sermon metadata, an FTS5
  full-text index, the scripture→sermon link graph, and the illustration-fatigue
  tracker. **If you delete it, nothing is lost** — it is rebuilt from the `.md`
  vault.

### The rebuild guarantee

```
$ rm ~/.local/share/sermon-studio/pastor.db
$ sermon index --vault ~/Sermons --db ~/.local/share/sermon-studio/pastor.db
indexed 203 sermons (203 scanned, 397 links, 204 illustrations) in 359 ms
```

Measured on a 203-sermon archive: **359 ms** full rebuild, **9 ms** incremental
sync. The spec's 3-second budget is met with an order of magnitude to spare.

---

## Quick start

### 1. Build the canon vault (one time)

```bash
# Fetch public-domain sources into data/raw (see docs/DATA_SOURCES.md),
# then normalize and compile:
scripts/build-canon.sh
```

### 2. Use the headless CLI

```bash
# Full-text search across the archive
sermon search "shepherd" --db ~/.local/share/sermon-studio/pastor.db

# A verse, its Strong's words, and its cross-references
sermon verse "Rom 8:28" --canon data/canon.db

# A lexicon entry
sermon strong G26 --canon data/canon.db

# Sermons that have touched a text
sermon preached "Rom 8:28" --db ~/.local/share/sermon-studio/pastor.db

# Illustration fatigue (repeated illustrations)
sermon fatigue --db ~/.local/share/sermon-studio/pastor.db --min 3

# Archive statistics
sermon stats --db ~/.local/share/sermon-studio/pastor.db

# Create a new sermon template
sermon new "The Grace That Saves" --passage "Eph 2:8-9" --vault ~/Sermons
```

### 3. Build the desktop app

```bash
scripts/build-app.sh          # produces .AppImage and .deb
```

---

## Keyboard model

The editor is designed to be driven without a mouse.

| Shortcut | Action |
| --- | --- |
| `Ctrl+P` | Command palette |
| `Ctrl+S` | Save (auto-indexes the file) |
| `Ctrl+N` | New sermon |
| `Ctrl+K` | Focus search |
| `Ctrl+1` | Focus archive panel |
| `Ctrl+2` | Focus editor |
| `Ctrl+3` | Focus study panel |
| `Ctrl+B` / `Ctrl+I` | Bold / italic |
| `Esc` | Dismiss palette / blur |

There are **no modal dialogs during writing**. Feedback is a transient toast;
commands live in the palette overlay.

---

## The Librarian (optional, off by default)

The Librarian is a purely local cataloger. When enabled it computes
bag-of-words cosine similarity between sermons to surface *related sermons*,
suggest tags, and flag reused illustrations. It uses no model weights, no
network, and no external service. It is **disabled by default** and can be
toggled from the command palette or the status chip.

---

## License & data provenance

Application code: MIT. Bible and lexicon data are public domain or
permissively licensed; see `docs/DATA_SOURCES.md` for full attribution.
