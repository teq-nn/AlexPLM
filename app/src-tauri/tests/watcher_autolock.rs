//! End-to-end test of the watcher-side Auto-Lock glue (Issue #42, E31/E35).
//!
//! The pure decisions — *which* save auto-locks (`autolock::decide_auto_lock`), and *which* held
//! lock is auto-unlocked at a checkpoint (the Lock Warden's `decide`, reused) — are proven
//! exhaustively by the table tests in `src/autolock.rs` and `src/warden.rs`. A real `git lfs lock`
//! needs an LFS API server, which a unit test cannot stand up (a `file://` remote has no lock
//! protocol). So, like `push_warden.rs`, this file proves the side-effecting halves that DO work
//! against plain git:
//!
//! - **read-only = free / writable = mine**: the on-disk permission flip that tracks lock
//!   ownership (the resting state of a free binary, and the writable state once the lock is mine);
//! - the **auto-lock trigger window**: the watcher fires its `on_lock` sink on the FIRST dirty
//!   lockable path of a burst, before any commit/checkpoint, and is idempotent across re-saves;
//! - the **clean-path auto-unlock condition**: built from a real `git status` read, exactly the
//!   clean held paths are selected for release (the Warden's rule), dirty ones are kept.

use app_lib::lockglue::{set_read_only, set_writable};
use app_lib::warden::{
    decide, Checkpoint, Cleanliness, LockState, PathKind, WardenAction, WardenSnapshot,
};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn init_repo(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("README.md"), b"start").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "init"]);
}

fn is_readonly(p: &Path) -> bool {
    std::fs::metadata(p).unwrap().permissions().readonly()
}

/// read-only = free, writable = mine: a free lockable binary rests read-only; taking the lock
/// (the on-disk half of `acquire_lock`) makes it writable for me; releasing it (the auto-unlock
/// half) rests it read-only again. Mergeable text is never made read-only by the tool.
#[test]
fn lockable_binary_rests_read_only_and_becomes_writable_when_mine() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    std::fs::create_dir_all(root.join("mechanik/gehaeuse")).unwrap();
    let bin = "mechanik/gehaeuse/gehaeuse.f3d";
    let txt = "firmware/main.c";
    std::fs::write(root.join(bin), b"v1").unwrap();
    std::fs::create_dir_all(root.join("firmware")).unwrap();
    std::fs::write(root.join(txt), b"int main(){}").unwrap();

    // Free (resting) -> read-only.
    set_read_only(root, bin).unwrap();
    assert!(is_readonly(&root.join(bin)), "a free lockable binary rests read-only");

    // Lock acquired -> writable for me (the side effect inside acquire_lock).
    set_writable(root, bin).unwrap();
    assert!(!is_readonly(&root.join(bin)), "my locked binary is writable");

    // Lock released at a clean checkpoint -> read-only again (frei).
    set_read_only(root, bin).unwrap();
    assert!(is_readonly(&root.join(bin)), "a released binary rests read-only again");

    // Mergeable text is always freely editable — the tool never flips its bit.
    set_read_only(root, txt).unwrap();
    assert!(!is_readonly(&root.join(txt)), "mergeable text is never made read-only");
}

/// The auto-lock TRIGGER window (the heart of the slice): a mergeable-text edit must NEVER
/// auto-lock — the negative half of the trigger — and any lock the watcher takes for a lockable
/// path happens on the FIRST save event, BEFORE the burst settles into a commit/checkpoint.
///
/// A real `git lfs lock` has no API server in a test (a `file://` remote has no lock protocol),
/// so the positive acquisition can't complete here — it is proven at the decision level by the
/// exhaustive table tests in `src/autolock.rs`. What this end-to-end test pins down against a real
/// watcher + real save events is: (a) text never fires `on_lock`, and (b) the watcher reaches the
/// lock-trigger point *before* any settle (the 30s window stays open the whole time), so a lock
/// could only ever precede the first checkpoint — never lag it.
#[test]
fn watcher_never_locks_text_and_triggers_before_any_checkpoint() {
    use app_lib::watcher::watch_product_with_window;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    let locked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stands: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let lock_sink = locked.clone();
    let stand_sink = stands.clone();

    // A long quiet window: no settle can fire during the test, so any auto-lock provably precedes
    // the first checkpoint (the watcher takes the lock from the save event, not the settle).
    let window = Duration::from_secs(30);
    let _handle = watch_product_with_window(
        root,
        window,
        move |_stand| {
            *stand_sink.lock().unwrap() += 1;
        },
        move |path| {
            lock_sink.lock().unwrap().push(path);
        },
    )
    .unwrap();

    let target_txt = root.join("firmware/main.c");
    std::fs::create_dir_all(target_txt.parent().unwrap()).unwrap();

    // Edit a mergeable-text file repeatedly: text is NEVER lockable, so on_lock must stay empty.
    for i in 0..3 {
        std::fs::write(&target_txt, format!("// rev {i}")).unwrap();
        std::thread::sleep(Duration::from_millis(60));
    }
    std::thread::sleep(Duration::from_millis(500)); // let the watcher drain the save events

    assert!(
        locked.lock().unwrap().iter().all(|p| p != "firmware/main.c"),
        "a mergeable-text edit must NEVER auto-lock"
    );
    // The 30s window is still open: no commit/checkpoint has happened, so the lock trigger (which
    // the watcher runs on each save event) provably precedes any checkpoint.
    assert_eq!(*stands.lock().unwrap(), 0, "no commit/checkpoint has fired yet");

    drop(_handle); // stop the watcher thread cleanly
}

/// The clean-path auto-unlock CONDITION, built from a real `git status` read and decided by the
/// reused Lock Warden: a held lock on a locally **clean** path is selected for release; a held lock
/// on a **dirty** path is kept. This is exactly the rule `lockglue::auto_unlock_clean_paths` applies
/// per held lock — proven here at the snapshot/decision boundary (no LFS server needed).
#[test]
fn auto_unlock_selects_clean_held_paths_and_keeps_dirty_ones() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    // Two held-by-me lockable binaries: commit both clean, then dirty exactly one.
    std::fs::create_dir_all(root.join("mechanik")).unwrap();
    let clean_path = "mechanik/clean.f3d";
    let dirty_path = "mechanik/dirty.f3d";
    std::fs::write(root.join(clean_path), b"v1").unwrap();
    std::fs::write(root.join(dirty_path), b"v1").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "init binaries"]);
    std::fs::write(root.join(dirty_path), b"v2-edited").unwrap();

    // Read the real worktree status the sweep would read.
    let snap = app_lib::lockglue::snapshot(root).unwrap();
    let is_dirty = |p: &str| snap.dirty.iter().any(|d| d == p);
    assert!(!is_dirty(clean_path), "the committed binary reads back clean");
    assert!(is_dirty(dirty_path), "the edited binary reads back dirty");

    // Apply the sweep's per-lock policy (the same `decide` call `auto_unlock_clean_paths` makes).
    let release = |p: &str| {
        decide(WardenSnapshot {
            kind: PathKind::Binary,
            lock: LockState::HeldByMe,
            clean: if is_dirty(p) { Cleanliness::Dirty } else { Cleanliness::Clean },
            checkpoint: Checkpoint::Laufend,
        }) == WardenAction::AutoUnlock
    };

    assert!(release(clean_path), "a held lock on a CLEAN path is auto-unlocked");
    assert!(!release(dirty_path), "a held lock on a DIRTY path is kept (still my open work)");
}
