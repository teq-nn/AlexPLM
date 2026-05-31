//! Filesystem watcher that drives the silent auto-commit loop (Issue #4) and the watcher-side
//! **Auto-Lock** (Issue #42).
//!
//! Thin, side-effecting layer over the pure cores in [`crate::autocommit`] and
//! [`crate::autolock`]. It watches the product root recursively and feeds raw write events into:
//! - the [`Debouncer`] — when a burst of saves goes quiet for the window, exactly one local commit
//!   and a new **Stand** is produced; and
//! - the **Auto-Lock** trigger — the *instant* a save touches a lockable path we do not already
//!   hold, a `git lfs lock` is acquired (and the file made writable for me). This closes the
//!   Binär-Invarianten-Fenster **before** the first checkpoint: the lock exists from the first
//!   edit, not from the first commit or a manual card click (Issue #42).
//!
//! No git vocabulary leaves this layer: it hands upward a `Stand { path, timestamp }` (rendered as
//! a new Stände entry) and the product-relative path of any freshly auto-locked artifact (so the
//! UI re-reads the LED signals).
//!
//! The timing and the auto-lock *decisions* live in `autocommit` / `autolock` and are table-tested
//! there. This module only owns the `notify` plumbing, the per-session "already locked" memory, and
//! the loop, kept deliberately small.

use crate::autocommit::{commit_all, Debouncer, Decision, Stand, DEFAULT_QUIET_WINDOW};
use crate::autolock::decide_auto_lock;
use crate::locks::is_lockable;
use notify::{EventKind, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// How often the loop wakes to re-check whether a pending burst has settled.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// A handle to a running watcher. Dropping it stops the watch loop and joins its thread.
pub struct WatchHandle {
    stop: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
    /// Kept alive so the OS watch stays registered for the loop's lifetime.
    _watcher: notify::RecommendedWatcher,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Should a raw filesystem event be treated as a "save" that arms the debouncer?
/// Creates/modifies/removes/renames of real content count; pure metadata pings and any
/// path inside `.git` do not (committing itself writes `.git`, which must not re-arm).
fn is_save_event(kind: &EventKind, paths: &[PathBuf]) -> bool {
    let touches_content = matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );
    if !touches_content {
        return false;
    }
    // Ignore anything under a `.git` directory — our own commit churns it.
    !paths.iter().any(|p| is_in_git_dir(p))
}

fn is_in_git_dir(p: &Path) -> bool {
    p.components()
        .any(|c| c.as_os_str() == std::ffi::OsStr::new(".git"))
}

/// Best-effort product-relative, forward-slash path for a changed file; falls back to the
/// product folder name (".") when the event has no usable path under the root.
fn rel_for_event(root: &Path, paths: &[PathBuf]) -> String {
    for p in paths {
        if let Ok(rel) = p.strip_prefix(root) {
            let s = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if !s.is_empty() {
                return s;
            }
        }
    }
    ".".to_string()
}

/// Start watching `root`. Each settled save triggers one local commit and invokes `on_stand`
/// with the resulting [`Stand`]; the first save to any lockable path auto-acquires its lock and
/// invokes `on_lock` with that product-relative path. The watch runs on a background thread until
/// the returned [`WatchHandle`] is dropped.
///
/// `on_stand` / `on_lock` are the side-effect sinks (in the app they emit Tauri events); injecting
/// them keeps this loop testable end-to-end against a real temp repo without a GUI.
pub fn watch_product<S, L>(root: &Path, on_stand: S, on_lock: L) -> notify::Result<WatchHandle>
where
    S: Fn(Stand) + Send + 'static,
    L: Fn(String) + Send + 'static,
{
    watch_product_with_window(root, DEFAULT_QUIET_WINDOW, on_stand, on_lock)
}

/// As [`watch_product`], with an explicit quiet window (tests use a short one).
pub fn watch_product_with_window<S, L>(
    root: &Path,
    window: Duration,
    on_stand: S,
    on_lock: L,
) -> notify::Result<WatchHandle>
where
    S: Fn(Stand) + Send + 'static,
    L: Fn(String) + Send + 'static,
{
    let root = root.to_path_buf();
    let (tx, rx) = mpsc::channel::<notify::Event>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_thread = stop.clone();

    let join = std::thread::spawn(move || {
        run_loop(&root, window, &rx, &stop_thread, &on_stand, &on_lock);
    });

    Ok(WatchHandle {
        stop,
        join: Some(join),
        _watcher: watcher,
    })
}

/// The watch loop: drain events into the debouncer, auto-lock the first dirty lockable path of a
/// burst, poll for settles, commit on settle. Separated so its control flow is obvious and the
/// side effects sit in one place.
fn run_loop<S, L>(
    root: &Path,
    window: Duration,
    rx: &mpsc::Receiver<notify::Event>,
    stop: &std::sync::atomic::AtomicBool,
    on_stand: &S,
    on_lock: &L,
) where
    S: Fn(Stand),
    L: Fn(String),
{
    let mut deb = Debouncer::new(window);
    // Per-session memory of paths we have already auto-locked, so re-saving a file we are already
    // editing never re-fires a `git lfs lock`. The lock itself is the source of truth (re-locking
    // is idempotent in the glue); this set just keeps the loop quiet between saves.
    let mut locked: HashSet<String> = HashSet::new();

    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        // Block up to POLL_INTERVAL for the next event; timeout drives the settle poll.
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(event) => {
                if is_save_event(&event.kind, &event.paths) {
                    let rel = rel_for_event(root, &event.paths);
                    // Auto-Lock (Issue #42): the FIRST save to a lockable path takes its lock right
                    // now — before any checkpoint — closing the Binär-Invarianten-Fenster. The
                    // pure trigger decides; the glue acquires the lock and makes the file writable.
                    if decide_auto_lock(is_lockable(&rel), locked.contains(&rel)).should_acquire() {
                        if let Ok(true) = crate::lockglue::acquire_lock(root, &rel) {
                            locked.insert(rel.clone());
                            on_lock(rel.clone());
                        }
                    }
                    deb.observe_save(SystemTime::now(), &rel);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if let Decision::Settle { path } = deb.poll(SystemTime::now()) {
            // One settled burst -> at most one local commit -> at most one new Stand.
            if let Ok(Some(stand)) = commit_all(root, &path, SystemTime::now()) {
                on_stand(stand);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind};

    #[test]
    fn git_internal_writes_are_not_saves() {
        let root = Path::new("/p");
        // our own commit touches .git/* — must never re-arm the debouncer
        assert!(!is_save_event(
            &EventKind::Modify(ModifyKind::Any),
            &[root.join(".git/index")]
        ));
        assert!(!is_save_event(
            &EventKind::Create(CreateKind::File),
            &[root.join(".git/refs/heads/main")]
        ));
    }

    #[test]
    fn content_writes_are_saves() {
        let root = Path::new("/p");
        assert!(is_save_event(
            &EventKind::Modify(ModifyKind::Any),
            &[root.join("elektronik/regler/regler.kicad_pcb")]
        ));
        assert!(is_save_event(
            &EventKind::Create(CreateKind::File),
            &[root.join("neu.txt")]
        ));
    }

    #[test]
    fn metadata_pings_are_not_saves() {
        let root = Path::new("/p");
        assert!(!is_save_event(
            &EventKind::Access(notify::event::AccessKind::Read),
            &[root.join("a.txt")]
        ));
        assert!(!is_save_event(&EventKind::Other, &[root.join("a.txt")]));
    }

    #[test]
    fn rel_for_event_is_product_relative_forward_slash() {
        let root = Path::new("/p");
        assert_eq!(
            rel_for_event(root, &[root.join("mechanik/gehaeuse/x.f3d")]),
            "mechanik/gehaeuse/x.f3d"
        );
        // event at the root itself or with no usable child -> "."
        assert_eq!(rel_for_event(root, &[root.to_path_buf()]), ".");
        assert_eq!(rel_for_event(root, &[]), ".");
    }
}
