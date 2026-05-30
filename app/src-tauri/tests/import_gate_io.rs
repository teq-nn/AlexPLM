//! End-to-end Import Gate tests (Issue #7, E38): build real git repos on disk and assert
//! that `evaluate_import_gate` reads the three repo facts correctly and that the pure gate
//! maps them to the right decision. Also asserts the destructive `migrate_history_behind_gate`
//! refuses whenever the gate would not return `migrate-behind-gate` — the safety invariant
//! that the dangerous history rewrite can never be reached except through the gate.

use app_lib::import::{evaluate_import_gate, migrate_history_behind_gate};
use app_lib::import_gate::GateDecision;
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").arg("-C").arg(root).args(args).output().unwrap()
}

fn init_repo(root: &Path) {
    assert!(git(root, &["init"]).status.success());
}

fn commit_all(root: &Path, msg: &str) {
    git(root, &["add", "-A"]);
    assert!(git(
        root,
        &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--allow-empty", "-m", msg],
    )
    .status
    .success());
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Write a blob at or above the 50 MiB giant threshold so it counts as a giant binary.
fn write_giant(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let buf = vec![0u8; 51 * 1024 * 1024];
    fs::write(path, &buf).unwrap();
}

#[test]
fn fresh_folder_with_no_history_is_clean_init() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("mechanik/part/body.f3d"), b"small");

    let report = evaluate_import_gate(root).unwrap();
    assert_eq!(report.decision, GateDecision::CleanInit);
    assert!(!report.has_history);
    assert!(!report.shared_clones_exist);
    assert!(!report.giant_binaries_in_history);
}

#[test]
fn history_without_giants_unshared_is_clean_init() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("firmware/regler/main.c"), b"int main(){}");
    init_repo(root);
    commit_all(root, "seed");

    let report = evaluate_import_gate(root).unwrap();
    assert_eq!(report.decision, GateDecision::CleanInit);
    assert!(report.has_history);
    assert!(!report.shared_clones_exist);
    assert!(!report.giant_binaries_in_history);
}

#[test]
fn unshared_history_with_giant_binary_is_migrate_behind_gate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write_giant(&root.join("mechanik/gehaeuse/gehaeuse.f3d"));
    init_repo(root);
    commit_all(root, "seed with a giant binary already committed");

    let report = evaluate_import_gate(root).unwrap();
    assert_eq!(
        report.decision,
        GateDecision::MigrateBehindGate,
        "fresh/unshared repo with giant in history -> the gated migrate"
    );
    assert!(report.has_history);
    assert!(!report.shared_clones_exist);
    assert!(report.giant_binaries_in_history, "the giant blob must be detected");
}

#[test]
fn shared_clone_always_refuses_even_with_giant_binary() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write_giant(&root.join("mechanik/gehaeuse/gehaeuse.f3d"));
    init_repo(root);
    commit_all(root, "seed with a giant binary");
    // a configured remote stands in for "shared clones exist".
    git(root, &["remote", "add", "origin", "https://example.invalid/p.git"]);

    let report = evaluate_import_gate(root).unwrap();
    assert_eq!(
        report.decision,
        GateDecision::Refuse,
        "shared clones must always refuse, never poison others' clones (E38)"
    );
    assert!(report.shared_clones_exist);
}

#[test]
fn migrate_refuses_when_gate_does_not_permit() {
    // A clean-init repo (no giants) must never run the destructive rewrite, even if asked.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("firmware/regler/main.c"), b"int main(){}");
    init_repo(root);
    commit_all(root, "seed");

    let err = migrate_history_behind_gate(root).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
}

#[test]
fn migrate_refuses_on_shared_repo_with_giant() {
    // Even with a giant in history, a shared repo must refuse the rewrite (the strongest case).
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write_giant(&root.join("mechanik/gehaeuse/gehaeuse.f3d"));
    init_repo(root);
    commit_all(root, "seed with giant");
    git(root, &["remote", "add", "origin", "https://example.invalid/p.git"]);

    let err = migrate_history_behind_gate(root).unwrap_err();
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::PermissionDenied,
        "shared repo must refuse the destructive rewrite"
    );
}
