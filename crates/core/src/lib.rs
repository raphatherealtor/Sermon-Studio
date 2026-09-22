//! # Sermon Studio Core
//!
//! Offline, Linux-native expository sermon writing studio, compiler, and
//! retrieval engine.
//!
//! ## Architecture
//!
//! ```text
//!   ~/Sermons/*.md   (source of truth, plain CommonMark + YAML frontmatter)
//!        │
//!        ├── indexer ──► pastor.db   (derived, rebuildable < 3s)
//!        │
//!   raw data ──► canon.db            (static, read-only Bible + lexicon)
//! ```
//!
//! Nothing here touches the network. There is no telemetry. The `.md` files are
//! canonical; both databases can be deleted and rebuilt from disk.
//!
//! ## V1 scaffold (reconciled onto Track J)
//!
//! Track J ships the authoritative Sermon Intelligence engine as a single
//! module ([`intelligence`], i.e. `intelligence.rs`). The V1 scaffold is
//! reconciled *around* that engine rather than replacing it:
//!   * [`provenance`] — the provenance contract and the `is_lifetime_corpus`
//!     hard invariant (only `your-archive` is lifetime corpus). Track J's
//!     `Evidence` carries a `provenance` field sourced from this module.
//!   * [`research_packet`] — the research-packet data model, content addressing,
//!     and the constitutional isolation predicates.
//!   * [`canon`] is *extended* (not rewritten) with [`canon::schema_ext`] and
//!     [`canon::adapters`].
//!
//! There is deliberately **no** `intelligence/mod.rs`: Track J's
//! `intelligence.rs` remains the single, authoritative engine module.

pub mod atomic_save;
pub mod books;
pub mod canon;
pub mod chain_study;
pub mod directive;
pub mod error;
pub mod export;
pub mod indexer;
pub mod intelligence;
pub mod librarian;
pub mod linter;
pub mod provenance;
pub mod reconcile;
pub mod reference;
pub mod research_packet;
pub mod research_store;
pub mod retrieval;
pub mod schema;
pub mod sermon;
pub mod vault_watcher;

pub use directive::{Directive, SourceSpan};
pub use error::{CoreError, Result};
pub use provenance::{is_lifetime_corpus, Provenance, ProvenanceClass};
pub use reference::{PassageRef, VerseRef};
pub use sermon::{Block, Frontmatter, Sermon, SermonDoc};

/// Convenience: open canon.db read-only.
pub fn open_canon_readonly(path: &std::path::Path) -> Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    Ok(conn)
}
