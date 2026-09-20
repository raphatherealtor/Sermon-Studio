//! Crash-safe persistence helpers for canonical sermon files.
//!
//! The Markdown vault is the source of truth, so every write to it must be
//! atomic: write a temporary sibling file, flush it to disk, then rename it
//! over the canonical path. A save that fails partway must never corrupt or
//! truncate the previously valid sermon file — the temp file is simply
//! discarded and the original stays intact.
//!
//! Naming convention: temporary files are hidden dot-files matching
//! `.<name>.sstmp-<pid>-<n>` in the *same directory* as the target, so that
//! (a) the rename is always same-volume/atomic and (b) the indexer and the
//! vault watcher can cheaply ignore them (they skip hidden files and
//! `.sstmp` artifacts).

use crate::error::{CoreError, Result};
use crate::sermon;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Suffix used by in-flight atomic-save temp files. Both the indexer's hidden
/// filter and the watcher's ignore-list treat these as non-sermons.
pub const TEMP_SUFFIX: &str = ".sstmp";

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_sibling(target: &Path) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = target
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "sermon.md".to_string());
    target
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".{}.{}-{}-{}",
            name,
            TEMP_SUFFIX.trim_start_matches('.'),
            std::process::id(),
            n
        ))
}

/// Atomically replace the file at `path` with `bytes`.
///
/// Sequence: write temp sibling → `sync_all` (flush to disk) → rename over the
/// target. On any failure the temp file is removed and the previous valid
/// file is untouched.
pub fn save_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.is_dir() {
        return Err(CoreError::Other(format!(
            "refusing to overwrite directory: {}",
            path.display()
        )));
    }
    let parent = path.parent().ok_or_else(|| {
        CoreError::Other(format!("path has no parent directory: {}", path.display()))
    })?;
    std::fs::create_dir_all(parent)?;

    let tmp = temp_sibling(path);
    let outcome = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        use std::io::Write;
        f.write_all(bytes)?;
        // Durability point: bytes must survive a crash before the rename.
        f.sync_all()?;
        drop(f);
        // Atomic replace: on Windows std::fs::rename maps to MoveFileExW with
        // MOVEFILE_REPLACE_EXISTING, on POSIX rename(2) replaces atomically.
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();

    if let Err(e) = outcome {
        // Best-effort cleanup; never mask the original error.
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    // Best-effort directory durability so the rename itself survives a crash
    // on POSIX filesystems. Windows has no portable directory fsync; the
    // rename is already committed by the time we get here.
    #[cfg(unix)]
    {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Join a vault-relative path onto `vault`, rejecting anything that could
/// escape the vault (absolute paths, `..`, drive/prefix components, empty).
///
/// Sermon identity beyond the vault root is meaningless — files are canonical
/// *inside* the vault — so this is deliberately strict.
pub fn safe_join(vault: &Path, rel_path: &str) -> Result<PathBuf> {
    if rel_path.trim().is_empty() {
        return Err(CoreError::Other("empty vault-relative path".to_string()));
    }
    let rel = Path::new(rel_path);
    if rel.is_absolute() {
        return Err(CoreError::Other(format!(
            "absolute path is not a vault-relative sermon path: {rel_path}"
        )));
    }
    for comp in rel.components() {
        match comp {
            Component::Normal(_) => {}
            other => {
                return Err(CoreError::Other(format!(
                    "unsafe path component {:?} in sermon path: {}",
                    other, rel_path
                )))
            }
        }
    }
    Ok(vault.join(rel))
}

/// Atomically save sermon `content` at `rel_path` inside `vault`.
///
/// Creates parent directories as needed (sub-vaults are allowed). Returns the
/// SHA-256 of the persisted content so callers can record the baseline for
/// conflict detection *of exactly what reached the disk*.
///
/// File persistence happens strictly BEFORE any derived-index update; callers
/// index after this returns and may fail without endangering the sermon.
pub fn save_sermon_atomic(vault: &Path, rel_path: &str, content: &str) -> Result<String> {
    let full = safe_join(vault, rel_path)?;
    save_atomic(&full, content.as_bytes())?;
    Ok(sermon::sha256_hex(content.as_bytes()))
}

/// Read a vault-relative sermon file (strictly inside the vault).
pub fn read_vault_file(vault: &Path, rel_path: &str) -> Result<String> {
    let full = safe_join(vault, rel_path)?;
    Ok(std::fs::read_to_string(full)?)
}

/// Whether a vault-relative file currently exists on disk.
pub fn vault_file_exists(vault: &Path, rel_path: &str) -> bool {
    match safe_join(vault, rel_path) {
        Ok(p) => p.is_file(),
        Err(_) => false,
    }
}

/// Best-effort last-modified time (RFC 3339) of a vault-relative file, used
/// only for display in conflict reports — never for correctness decisions.
pub fn vault_file_modified_at(vault: &Path, rel_path: &str) -> Option<String> {
    let full = safe_join(vault, rel_path).ok()?;
    let meta = std::fs::metadata(full).ok()?;
    let modified = meta.modified().ok()?;
    let dt: chrono::DateTime<chrono::Utc> = modified.into();
    Some(dt.to_rfc3339())
}

/// True if `file_name` looks like an in-flight atomic-save temp artifact and
/// must never be treated as a sermon.
pub fn is_temp_artifact(file_name: &str) -> bool {
    file_name.starts_with('.') || file_name.contains(TEMP_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_save_replaces_content_and_leaves_no_temp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("sermon.md");
        std::fs::write(&target, "original").unwrap();

        let hash = save_sermon_atomic(tmp.path(), "sermon.md", "revised content").unwrap();
        assert_eq!(hash, sermon::sha256_hex(b"revised content"));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "revised content");

        // No `.sstmp` artifacts left behind.
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| is_temp_artifact(&e.file_name().to_string_lossy()))
            .collect();
        assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    }

    #[test]
    fn atomic_save_creates_missing_subdirectories() {
        let tmp = tempfile::tempdir().unwrap();
        save_sermon_atomic(tmp.path(), "2024/advent/sermon.md", "hello").unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("2024/advent/sermon.md")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn failed_atomic_save_preserves_previous_file() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("keep.md");
        std::fs::write(&target, "previous valid sermon").unwrap();

        // A path whose "parent" is a regular file cannot be created: the save
        // must fail without touching the existing valid file.
        let bogus = tmp.path().join("keep.md").join("nested").join("x.md");
        assert!(save_atomic(&bogus, b"new").is_err());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "previous valid sermon"
        );
    }

    #[test]
    fn safe_join_rejects_escape_attempts() {
        let vault = Path::new("/vault");
        assert!(safe_join(vault, "../../etc/passwd").is_err());
        assert!(safe_join(vault, "a/../../b.md").is_err());
        assert!(safe_join(vault, "/abs/path.md").is_err());
        assert!(safe_join(vault, "").is_err());
        assert!(safe_join(vault, "   ").is_err());
        assert_eq!(
            safe_join(vault, "sub/dir/sermon.md").unwrap(),
            vault.join("sub/dir/sermon.md")
        );
    }

    #[test]
    fn temp_artifacts_are_recognized() {
        assert!(is_temp_artifact(".sermon.md.sstmp-123-0"));
        assert!(is_temp_artifact(".hidden.md"));
        assert!(!is_temp_artifact("sermon.md"));
    }
}
