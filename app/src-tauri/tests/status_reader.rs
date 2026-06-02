//! End-to-end Status Reader test (Issue #6): build a real git repo on disk, make a lockable
//! artifact dirty, read the snapshot back from git (no second source of truth, E37), and assert
//! the derived per-artifact status. No LFS server is needed for the worktree-derived greys.
//!
//! The lock-driven greens/oranges are covered exhaustively by the pure table tests in
//! `src/locks.rs`; here we prove the side-effecting glue (`git status` reading + classifier
//! reuse) wires up correctly against a real repo.

use app_lib::lockglue::{ensure_local_writable, snapshot, writable_lockable_paths};
use app_lib::locks::{
    derive_status, derive_statuses, is_lockable, promote_in_progress_for_writable, ArtifactStatus,
};
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn touch(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn dirty_lockable_artifact_reads_back_as_in_progress_clean_one_as_free() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    git(root, &["init"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);

    // Two lockable artifacts; commit them clean first.
    touch(&root.join("mechanik/gehaeuse/gehaeuse.f3d"), b"v1");
    touch(&root.join("mechanik/halter/halter.step"), b"v1");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "init"]);

    // Now dirty one of them.
    touch(&root.join("mechanik/gehaeuse/gehaeuse.f3d"), b"v2-edited");

    // Read the world back from git purely — no cache, no service (E37).
    let snap = snapshot(root).unwrap();
    assert_eq!(snap.me, "anna");

    // table: artifact path -> expected derived status from the real snapshot
    let cases: &[(&str, ArtifactStatus)] = &[
        // edited (dirty), no lock -> grey / in-progress
        ("mechanik/gehaeuse/gehaeuse.f3d", ArtifactStatus::InProgress),
        // committed clean, no lock -> green / free
        ("mechanik/halter/halter.step", ArtifactStatus::Free),
    ];
    for (path, expected) in cases {
        assert_eq!(
            derive_status(path, &snap).status,
            *expected,
            "path = {path}"
        );
    }
}

/// Issue #104 regression: in a fresh, **unpublished** product, git-lfs rests a `lockable` file
/// read-only (no server lock can be held yet), so the sole local author can only open it read-only
/// and the Status Reader shows no lock. Opening the product must hand the write bit back, and the
/// writable bit must then grey the card (in progress / mine) even with no server lock to read.
#[test]
fn unpublished_lockable_file_is_made_writable_and_reads_as_in_progress() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    let rel = "elektronik/board.kicad_pcb";
    let abs = root.join(rel);
    touch(&abs, b"pcb");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "init"]);

    // Simulate git-lfs's lockable enforcement: the file rests read-only with no lock held.
    let mut perms = fs::metadata(&abs).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&abs, perms).unwrap();

    // Repro: read-only -> not "mine" -> the Status Reader would call it Free (no lock shown).
    let paths = vec![rel.to_string()];
    assert!(
        writable_lockable_paths(root, &paths).unwrap().is_empty(),
        "a read-only lockable file is not yet 'mine'"
    );

    // Fix part A: opening the unpublished product hands the write bit back ("all mine" pre-publish).
    ensure_local_writable(root).unwrap();
    assert!(
        !fs::metadata(&abs).unwrap().permissions().readonly(),
        "Issue #104: an unpublished lockable file must rest writable"
    );

    // Fix part B: the writable bit now greys the card (in progress) without any server lock.
    let writable = writable_lockable_paths(root, &paths).unwrap();
    assert_eq!(writable, vec![rel.to_string()]);
    let snap = snapshot(root).unwrap();
    let mut sigs = derive_statuses(&paths, &snap);
    promote_in_progress_for_writable(&mut sigs, &writable);
    assert_eq!(sigs[0].status, ArtifactStatus::InProgress);
}

/// Once a product is **published**, the real `git lfs locks` state is the single truth — the
/// on-disk writable bit must NOT be consulted, so a free (but for-whatever-reason writable) binary
/// is never spuriously greyed. Guards that Issue #104's pre-publish shortcut cannot leak into the
/// shared rhythm (zero regression to the published LED).
#[test]
fn published_product_ignores_the_on_disk_writable_bit() {
    let dir = tempfile::tempdir().unwrap();
    let product = dir.path().join("product");
    let bare = dir.path().join("remote.git");
    fs::create_dir_all(&product).unwrap();

    let out = Command::new("git")
        .args(["init", "--bare", "-b", "main"])
        .arg(&bare)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    git(&product, &["init", "-b", "main"]);
    git(&product, &["config", "user.name", "anna"]);
    git(&product, &["config", "user.email", "anna@example.com"]);
    let rel = "elektronik/board.kicad_pcb";
    touch(&product.join(rel), b"pcb"); // writable on disk
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "init"]);
    let url = format!("file://{}", bare.display());
    git(&product, &["remote", "add", "origin", &url]);
    git(&product, &["push", "--set-upstream", "origin", "main"]);

    // Published: the writable-bit shortcut is disabled, so the file is not reported as "mine".
    let paths = vec![rel.to_string()];
    assert!(
        writable_lockable_paths(&product, &paths).unwrap().is_empty(),
        "published products read locks from the server, never the on-disk bit"
    );
    // ensure_local_writable is likewise a no-op once published.
    ensure_local_writable(&product).unwrap();
}

#[test]
fn lockable_set_matches_the_classifier_buckets() {
    // The auto-lock decision is exactly the #3 classifier's unmergeable buckets.
    assert!(is_lockable("mechanik/gehaeuse/gehaeuse.f3d"));
    assert!(is_lockable("elektronik/board.kicad_pcb"));
    assert!(!is_lockable("firmware/main.c"));
    assert!(!is_lockable("docs/README.md"));
}
