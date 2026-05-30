//! End-to-end Status Reader test (Issue #6): build a real git repo on disk, make a lockable
//! artifact dirty, read the snapshot back from git (no second source of truth, E37), and assert
//! the derived per-artifact status. No LFS server is needed for the worktree-derived greys.
//!
//! The lock-driven greens/oranges are covered exhaustively by the pure table tests in
//! `src/locks.rs`; here we prove the side-effecting glue (`git status` reading + classifier
//! reuse) wires up correctly against a real repo.

use app_lib::lockglue::snapshot;
use app_lib::locks::{derive_status, is_lockable, ArtifactStatus};
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

#[test]
fn lockable_set_matches_the_classifier_buckets() {
    // The auto-lock decision is exactly the #3 classifier's unmergeable buckets.
    assert!(is_lockable("mechanik/gehaeuse/gehaeuse.f3d"));
    assert!(is_lockable("elektronik/board.kicad_pcb"));
    assert!(!is_lockable("firmware/main.c"));
    assert!(!is_lockable("docs/README.md"));
}
