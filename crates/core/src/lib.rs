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

pub mod atomic_save;
pub mod books;
pub mod canon;
pub mod directive;
pub mod error;
pub mod export;
pub mod indexer;
pub mod librarian;
pub mod linter;
pub mod reconcile;
pub mod reference;
pub mod retrieval;
pub mod schema;
pub mod sermon;
pub mod vault_watcher;

pub use directive::{Directive, SourceSpan};
pub use error::{CoreError, Result};
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
