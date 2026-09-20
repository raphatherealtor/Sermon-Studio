//! Filesystem watcher for the sermon vault — an *optimization only*.
//!
//! The watcher watches the vault with the `notify` crate, coalesces bursts of
//! filesystem events into a debounced set (default **750 ms** — the window is
//! the time an artifact must stay *quiet* before it is reported), and hands
//! the affected vault-relative paths to a caller-provided callback. The
//! callback in the app wires into [`crate::reconcile::reconcile_paths`] for
//! targeted, hash-driven repairs.
//!
//! Correctness invariant: the watcher is never the sole correctness mechanism.
//! Dropped events, a failed watcher, or a missed rename are always repaired by
//! startup reconciliation ([`crate::reconcile::startup_reconcile`]). That is
//! also why the debouncer is intentionally simple and fully unit-testable
//! without a real filesystem.

use notify::{Watcher, RecursiveMode, RecommendedWatcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Debounce window: how long an artifact must stay quiet before its events
/// are flushed to the repair callback.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(750);

/// Poll cadence of the debounce loop. Smaller than the debounce window by
/// design; the window — not this cadence — decides latency.
const TICK: Duration = Duration::from_millis(100);

/// Coalesces raw event paths and releases a path once it has been quiet for
/// at least the debounce window. Re-offering a path during the window pushes
/// its deadline back (a burst collapses into one flush).
///
/// All time-dependent entry points have an `*_at` variant taking an explicit
/// `Instant` so behavior is deterministically unit-testable.
#[derive(Debug)]
pub struct DebounceBuffer {
    window: Duration,
    pending: HashMap<PathBuf, Instant>,
}

impl DebounceBuffer {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            pending: HashMap::new(),
        }
    }

    pub fn offer(&mut self, path: impl Into<PathBuf>) {
        self.offer_at(path, Instant::now());
    }

    pub fn offer_at(&mut self, path: impl Into<PathBuf>, now: Instant) {
        self.pending.insert(path.into(), now);
    }

    /// Remove and return every path that has been quiet for at least the
    /// debounce window, in sorted order (deterministic repair order).
    pub fn drain_ready(&mut self) -> Vec<PathBuf> {
        self.drain_ready_at(Instant::now())
    }

    pub fn drain_ready_at(&mut self, now: Instant) -> Vec<PathBuf> {
        let window = self.window;
        let mut ready: Vec<PathBuf> = Vec::new();
        for (path, offered_at) in self.pending.iter() {
            if now.duration_since(*offered_at) >= window {
                ready.push(path.clone());
            }
        }
        for p in &ready {
            self.pending.remove(p);
        }
        ready.sort();
        ready
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// True when `path` should be ignored: outside the vault, not a `.md` file,
/// hidden, or an in-flight atomic-save temp artifact.
pub fn is_relevant_path(vault: &Path, path: &Path) -> bool {
    let rel = match path.strip_prefix(vault) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let name = match rel.file_name().map(|s| s.to_string_lossy().to_string()) {
        Some(n) => n,
        None => return false,
    };
    if crate::atomic_save::is_temp_artifact(&name) {
        return false;
    }
    rel.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Vault-relative, forward-slash-normalized path string for a relevant event
/// path; `None` when the path should be ignored.
pub fn normalize_event_path(vault: &Path, path: &Path) -> Option<String> {
    if !is_relevant_path(vault, path) {
        return None;
    }
    path.strip_prefix(vault)
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"))
}

/// Running watcher. Dropping it (or calling [`WatcherHandle::stop`]) stops
/// the background loop and the OS watcher.
pub struct WatcherHandle {
    watcher: Option<RecommendedWatcher>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WatcherHandle {
    /// Stop the watcher and wait (bounded) for its loop to exit.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.watcher.take(); // dropping the notify watcher ends event delivery
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.watcher.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Watch `vault` recursively and invoke `on_quiescent` with the sorted list
/// of vault-relative `.md` paths that changed and have now been quiet for
/// `debounce`. Errors from the OS watcher surface via the returned
/// `notify::Result`; runtime event errors are ignored (the watcher is an
/// optimization — reconciliation is the correctness mechanism).
pub fn watch_vault<F>(vault: PathBuf, debounce: Duration, on_quiescent: F) -> notify::Result<WatcherHandle>
where
    F: Fn(Vec<String>) + Send + 'static,
{
    let (tx, rx) = channel::<notify::Event>();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;
    watcher.watch(&vault, RecursiveMode::Recursive)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("sermon-vault-watcher".to_string())
        .spawn(move || {
            debounce_loop(&vault, &rx, debounce, &stop_for_thread, on_quiescent);
        })?;

    Ok(WatcherHandle {
        watcher: Some(watcher),
        stop,
        thread: Some(thread),
    })
}

/// The watcher's event loop: feed events into the debounce buffer, flush
/// quiet paths to the callback. Extracted for clarity; driven by real events
/// in the live test below.
fn debounce_loop(
    vault: &Path,
    rx: &Receiver<notify::Event>,
    debounce: Duration,
    stop: &AtomicBool,
    on_quiescent: impl Fn(Vec<String>),
) {
    let mut buffer = DebounceBuffer::new(debounce);
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match rx.recv_timeout(TICK) {
            Ok(event) => {
                // Access events (reads, metadata probes) never change content.
                if matches!(event.kind, notify::event::EventKind::Access(_)) {
                    continue;
                }
                for path in event.paths {
                    if let Some(rel) = normalize_event_path(vault, &path) {
                        buffer.offer(rel);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let ready = buffer.drain_ready();
        if !ready.is_empty() {
            // Buffered entries are vault-relative, forward-slash strings.
            let rels: Vec<String> = ready
                .iter()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            on_quiescent(rels);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn base() -> Instant {
        Instant::now()
    }

    #[test]
    fn debounce_coalesces_a_burst_into_one_flush() {
        let mut buf = DebounceBuffer::new(Duration::from_millis(750));
        let t0 = base();
        for i in 0..5 {
            buf.offer_at("sermon.md", t0 + Duration::from_millis(i * 100));
        }
        // Still inside the window relative to the last offer (t0+400ms).
        assert_eq!(buf.drain_ready_at(t0 + Duration::from_millis(800)).len(), 0);
        assert_eq!(buf.pending_count(), 1, "burst must collapse to one entry");
        // Quiet past the window (t0+400 + 750) -> exactly one flush.
        assert_eq!(
            buf.drain_ready_at(t0 + Duration::from_millis(1200)),
            vec![PathBuf::from("sermon.md")]
        );
        assert!(buf.is_empty());
    }

    #[test]
    fn debounce_flushes_ready_paths_sorted_and_deterministic() {
        let mut buf = DebounceBuffer::new(Duration::from_millis(750));
        let t0 = base();
        buf.offer_at("c.md", t0);
        buf.offer_at("a.md", t0 + Duration::from_millis(10));
        buf.offer_at("b.md", t0 + Duration::from_millis(20));
        // Only c.md has been quiet for the full window (755 >= 750).
        assert_eq!(
            buf.drain_ready_at(t0 + Duration::from_millis(755)),
            vec![PathBuf::from("c.md")]
        );
        assert_eq!(
            buf.drain_ready_at(t0 + Duration::from_millis(2000)),
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")]
        );
    }

    #[test]
    fn debounce_reearm_after_flush() {
        let mut buf = DebounceBuffer::new(Duration::from_millis(750));
        let t0 = base();
        buf.offer_at("a.md", t0);
        assert!(buf.drain_ready_at(t0 + Duration::from_secs(1)).len() == 1);
        // New event after the flush starts a fresh window.
        buf.offer_at("a.md", t0 + Duration::from_secs(2));
        assert_eq!(buf.drain_ready_at(t0 + Duration::from_secs(2)).len(), 0);
        assert_eq!(buf.drain_ready_at(t0 + Duration::from_secs(3)).len(), 1);
    }

    #[test]
    fn event_filter_ignores_non_sermon_and_temp_paths() {
        let vault = Path::new("/vault");
        assert!(is_relevant_path(vault, Path::new("/vault/sermon.md")));
        assert!(is_relevant_path(vault, Path::new("/vault/sub/sermon.md")));
        assert!(!is_relevant_path(vault, Path::new("/vault/notes.txt")));
        assert!(!is_relevant_path(vault, Path::new("/vault/.sermon.md.sstmp-42-0")));
        assert!(!is_relevant_path(vault, Path::new("/vault/.hidden.md")));
        assert!(!is_relevant_path(vault, Path::new("/elsewhere/sermon.md")));
        assert_eq!(
            normalize_event_path(vault, Path::new("/vault/sub/sermon.md")).as_deref(),
            Some("sub/sermon.md")
        );
    }

    /// Live integration: real OS events must survive the debounce window and
    /// reach the callback exactly once per artifact. Generous timeouts keep
    /// this stable on slow filesystems; it is still an optimization-level
    /// test — correctness tests live in `reconcile`.
    #[test]
    fn live_watch_reports_quiet_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().to_path_buf();

        let (hit_tx, hit_rx) = mpsc::channel::<Vec<String>>();
        let handle = watch_vault(
            vault.clone(),
            Duration::from_millis(150), // small window keeps the test fast
            move |paths| {
                let _ = hit_tx.send(paths);
            },
        )
        .expect("watcher should start");

        // Small settle delay so the watcher is armed before we write.
        std::thread::sleep(Duration::from_millis(150));
        std::fs::write(vault.join("a.md"), b"content-a").unwrap();
        std::fs::write(vault.join("a.md"), b"content-a2").unwrap();
        std::fs::write(vault.join("ignored.txt"), b"nope").unwrap();

        let hit = hit_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a debounced flush should arrive");
        assert_eq!(hit, vec!["a.md".to_string()]);

        handle.stop();
    }
}
