//! Graph Projection tests (Issue #8).
//!
//! Two layers, matching the project's "reiner Kern + Tabellentest" pattern:
//!
//! 1. **Pure snapshot -> projection (no I/O).** A hand-built `RepoSnapshot` (the git/LFS
//!    facts) projects to the expected Stände, Meilenstein, and offloaded markers without
//!    touching a disk or git. This is the acceptance-criterion test.
//! 2. **End-to-end glue.** A real temp repo is read into a snapshot and promoting a Stand
//!    to a Meilenstein persists `VERSION_NOTES.md` and drives the version bar — exercising
//!    the thin git/LFS reading layer over the pure core.

use app_lib::graph::{project_graph, CommitFact, MilestoneFact, RepoSnapshot};
use app_lib::graphread::{promote_to_milestone, read_graph, read_snapshot, VERSION_NOTES};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

// ---- Layer 1: pure snapshot -> projection, no I/O -------------------------------------

#[test]
fn snapshot_projects_to_expected_stande_meilenstein_and_offloaded_markers() {
    // A git/LFS snapshot: three Stände, the middle one a Meilenstein, the oldest offloaded.
    let snapshot = RepoSnapshot {
        commits: vec![
            CommitFact {
                id: "c1".into(),
                parents: vec![],
                message: "auto: mechanik/gehaeuse/gehaeuse.f3d, 2025-01-01T00:00:00Z".into(),
                timestamp: "2025-01-01T00:00:00Z".into(),
            },
            CommitFact {
                id: "c2".into(),
                parents: vec!["c1".into()],
                message: "auto: VERSION_NOTES.md, 2026-03-01T12:00:00Z".into(),
                timestamp: "2026-03-01T12:00:00Z".into(),
            },
            CommitFact {
                id: "c3".into(),
                parents: vec!["c2".into()],
                message: "auto: elektronik/regler, 2026-05-30T09:15:00Z".into(),
                timestamp: "2026-05-30T09:15:00Z".into(),
            },
        ],
        milestones: vec![MilestoneFact {
            commit_id: "c2".into(),
            version: "v0.4".into(),
            has_notes: true,
        }],
        offloaded: vec!["c1".into()],
        offloaded_archive: Some("2025-11".into()),
    };

    let g = project_graph(&snapshot);

    // Stände rendered as nodes, newest first.
    let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["c3", "c2", "c1"]);

    let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();

    // Meilenstein marker on c2, with its human-text flag; version bar reads the active one.
    assert_eq!(node("c2").milestone.as_deref(), Some("v0.4"));
    assert!(node("c2").has_notes);
    assert_eq!(g.active_milestone.as_deref(), Some("v0.4"));

    // Plain Stände carry no milestone.
    assert_eq!(node("c3").milestone, None);

    // Offloaded marker on c1, node still present, archive label surfaced.
    assert!(node("c1").offloaded);
    assert!(!node("c3").offloaded);
    assert_eq!(g.offloaded_archive.as_deref(), Some("2025-11"));

    // Path recovered from the boring auto message (never the raw git message).
    assert_eq!(node("c3").path, "elektronik/regler");
    assert_eq!(node("c1").path, "mechanik/gehaeuse/gehaeuse.f3d");
}

// ---- Layer 2: end-to-end over a real temp repo ----------------------------------------

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_repo(root: &Path) {
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@t.test"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["config", "commit.gpgsign", "false"]);
}

fn commit_file(root: &Path, name: &str, body: &str, msg: &str) {
    std::fs::write(root.join(name), body).unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", msg]);
}

fn head(root: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn reads_a_real_repo_into_stande_then_promotes_one_to_a_meilenstein() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    // Two settled saves -> two Stände, boring machine messages.
    commit_file(root, "a.txt", "one", "auto: a.txt, 2026-05-30T09:00:00Z");
    commit_file(root, "b.txt", "two", "auto: b.txt, 2026-05-30T10:00:00Z");
    let target = head(root); // the Stand the user will promote

    // Before promotion: two nodes, no Meilenstein, version bar empty.
    let g = read_graph(root).unwrap();
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.active_milestone, None);
    assert!(g.nodes.iter().all(|n| n.milestone.is_none()));

    // Promote the newest Stand to a Meilenstein with human text.
    let now = UNIX_EPOCH + Duration::from_secs(1_777_000_000);
    let g =
        promote_to_milestone(root, &target, "v1.0", "Erstes vorzeigbares Gehäuse.", now).unwrap();

    // The promoted Stand now carries the version; the version bar shows it (Mono in UI).
    assert_eq!(g.active_milestone.as_deref(), Some("v1.0"));
    let promoted = g.nodes.iter().find(|n| n.id == target).unwrap();
    assert_eq!(promoted.milestone.as_deref(), Some("v1.0"));
    assert!(promoted.has_notes, "VERSION_NOTES text was persisted");

    // VERSION_NOTES.md is the ONLY place the human text lives (E28).
    let notes = std::fs::read_to_string(root.join(VERSION_NOTES)).unwrap();
    assert!(notes.contains("## v1.0"));
    assert!(notes.contains("Erstes vorzeigbares Gehäuse."));

    // The notes commit's message is boring/machine — no human text leaked into git.
    let last_msg = {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["log", "-1", "--pretty=%s"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert!(
        last_msg.starts_with("auto: "),
        "machine message, got {last_msg}"
    );
    assert!(
        !last_msg.contains("Gehäuse"),
        "human text must not be in the commit message"
    );

    // Re-reading the repo (fresh snapshot) shows the durable Meilenstein.
    let snap = read_snapshot(root).unwrap();
    assert!(snap
        .milestones
        .iter()
        .any(|m| m.version == "v1.0" && m.has_notes));
}

#[test]
fn promoting_requires_human_text() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    commit_file(root, "a.txt", "one", "auto: a.txt, 2026-05-30T09:00:00Z");
    let target = head(root);

    let err = promote_to_milestone(root, &target, "v1.0", "   ", UNIX_EPOCH).unwrap_err();
    assert!(
        err.to_string().contains("Text"),
        "must demand notes, got {err}"
    );
}

#[test]
fn fresh_repo_with_no_commits_projects_to_empty_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    let g = read_graph(root).unwrap();
    assert!(g.nodes.is_empty());
    assert_eq!(g.active_milestone, None);
}
