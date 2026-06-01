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

use app_lib::graph::{
    project_graph, BranchFact, CommitFact, MilestoneArt, MilestoneFact, RepoSnapshot,
};
use app_lib::graphread::{
    promote_to_milestone, read_graph, read_snapshot, toggle_milestone_freigabe, VERSION_NOTES,
};
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
            art: MilestoneArt::Prototyp,
        }],
        offloaded: vec!["c1".into()],
        offloaded_archive: Some("2025-11".into()),
        published: vec![],
        branches: vec![],
        active_branch: None,
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

    // A single linear history (no branch facts) is one lane, all on the active line.
    assert_eq!(g.lane_count, 1);
    assert!(g.nodes.iter().all(|n| n.lane == 0 && n.on_active));
}

#[test]
fn an_externally_created_zweig_surfaces_as_a_distinct_line() {
    // Acceptance #1+#2+#3 at the pure-projection layer:
    //   c1 -- c2 -- c3          main (active)
    //          \-- f1 -- f2     gehaeuse-v2 (made outside the tool)
    let snapshot = RepoSnapshot {
        commits: vec![
            CommitFact { id: "c1".into(), parents: vec![], message: "auto: a, 2026-05-01T00:00:00Z".into(), timestamp: "2026-05-01T00:00:00Z".into() },
            CommitFact { id: "c2".into(), parents: vec!["c1".into()], message: "auto: b, 2026-05-02T00:00:00Z".into(), timestamp: "2026-05-02T00:00:00Z".into() },
            CommitFact { id: "c3".into(), parents: vec!["c2".into()], message: "auto: c, 2026-05-03T00:00:00Z".into(), timestamp: "2026-05-03T00:00:00Z".into() },
            CommitFact { id: "f1".into(), parents: vec!["c2".into()], message: "auto: d, 2026-05-04T00:00:00Z".into(), timestamp: "2026-05-04T00:00:00Z".into() },
            CommitFact { id: "f2".into(), parents: vec!["f1".into()], message: "auto: e, 2026-05-05T00:00:00Z".into(), timestamp: "2026-05-05T00:00:00Z".into() },
        ],
        milestones: vec![],
        offloaded: vec![],
        offloaded_archive: None,
        published: vec![],
        branches: vec![
            BranchFact { name: "main".into(), tip: "c3".into() },
            BranchFact { name: "gehaeuse-v2".into(), tip: "f2".into() },
        ],
        active_branch: Some("main".into()),
    };
    let g = project_graph(&snapshot);
    let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();

    // The Zweig shows up as its own distinct line (#1), labelled in domain vocabulary.
    assert_eq!(g.lane_count, 2);
    assert!(node("f1").lane != 0 && node("f2").lane != 0);
    assert_eq!(node("f2").branch.as_deref(), Some("gehaeuse-v2"));

    // The active line stays clearly marked (#2): shared + main-unique Stände are on lane 0.
    assert_eq!(g.active_branch.as_deref(), Some("main"));
    for id in ["c1", "c2", "c3"] {
        assert!(node(id).on_active && node(id).lane == 0, "{id} on active trunk");
    }
    assert!(!node("f1").on_active && !node("f2").on_active);
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
fn a_new_meilenstein_is_prototyp_and_the_toggle_releases_and_un_releases_it() {
    // Issue #41 / E42 end-to-end over a real repo: promote -> Prototyp by default; the toggle
    // raises it to Freigabe (write-protected) and toggles back to Prototyp ("Un-Release").
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    commit_file(root, "a.txt", "one", "auto: a.txt, 2026-05-30T09:00:00Z");
    let target = head(root);

    let now = UNIX_EPOCH + Duration::from_secs(1_777_000_000);
    let g = promote_to_milestone(root, &target, "v1.0", "Erstes Gehäuse.", now).unwrap();

    // Acceptance: a new Meilenstein defaults to Prototyp (lax).
    assert_eq!(g.active_milestone.as_deref(), Some("v1.0"));
    assert_eq!(g.active_milestone_art, Some(MilestoneArt::Prototyp));
    let node = |g: &app_lib::graph::VersionGraph, id: &str| {
        g.nodes.iter().find(|n| n.id == id).cloned().unwrap()
    };
    assert_eq!(node(&g, &target).milestone_art, Some(MilestoneArt::Prototyp));

    // Toggle -> Freigabe: the Art is now Freigabe (write-protected).
    let g = toggle_milestone_freigabe(root, "v1.0").unwrap();
    assert_eq!(g.active_milestone_art, Some(MilestoneArt::Freigabe));
    assert_eq!(node(&g, &target).milestone_art, Some(MilestoneArt::Freigabe));

    // Write-protect (E8): a released Meilenstein refuses to be overwritten by a re-promote.
    let err = promote_to_milestone(root, &target, "v1.0", "nochmal", now).unwrap_err();
    assert!(
        err.to_string().contains("schreibgeschützt"),
        "Freigabe must be write-protected, got {err}"
    );

    // Toggle back -> Prototyp ("Un-Release"): reversible, and re-promote is allowed again.
    let g = toggle_milestone_freigabe(root, "v1.0").unwrap();
    assert_eq!(g.active_milestone_art, Some(MilestoneArt::Prototyp));
    let g = promote_to_milestone(root, &target, "v1.0", "geänderter Text.", now).unwrap();
    assert!(node(&g, &target).has_notes);

    // The Art survives a fresh read of the repo (persisted per tag in the _plm-style store).
    let snap = read_snapshot(root).unwrap();
    let ms = snap.milestones.iter().find(|m| m.version == "v1.0").unwrap();
    assert_eq!(ms.art, MilestoneArt::Prototyp);
}

#[test]
fn toggling_an_unknown_meilenstein_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    commit_file(root, "a.txt", "one", "auto: a.txt, 2026-05-30T09:00:00Z");

    let err = toggle_milestone_freigabe(root, "v9.9").unwrap_err();
    assert!(
        err.to_string().contains("Keine Revision"),
        "must refuse a non-existent Revision, got {err}"
    );
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

#[test]
fn an_external_branch_committed_to_outside_the_app_appears_as_a_distinct_line() {
    // Acceptance #1+#2 over a *real* repo: build a branch the way a colleague would in their
    // own working copy (plain git, no app), then read the graph and confirm it surfaces.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    git(root, &["checkout", "-q", "-b", "main"]);

    commit_file(root, "a.txt", "1", "auto: a.txt, 2026-05-01T00:00:00Z");
    commit_file(root, "b.txt", "2", "auto: b.txt, 2026-05-02T00:00:00Z");
    let trunk_tip = head(root);

    // A Zweig started OUTSIDE the app: a new branch + a commit on it, then back to main.
    git(root, &["checkout", "-q", "-b", "gehaeuse-v2"]);
    commit_file(root, "c.txt", "3", "auto: c.txt, 2026-05-03T00:00:00Z");
    let zweig_tip = head(root);
    git(root, &["checkout", "-q", "main"]);

    let g = read_graph(root).unwrap();

    // All three Stände are collected across both lines — not just the active one (#1).
    assert_eq!(g.nodes.len(), 3, "Stände collected across all Zweige");
    assert!(g.lane_count >= 2, "the external Zweig forms its own lane");

    let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();
    // The active line (main) stays marked; the Zweig's unique Stand is off the trunk (#2).
    assert!(node(&trunk_tip).on_active && node(&trunk_tip).lane == 0);
    assert!(!node(&zweig_tip).on_active);
    assert_eq!(node(&zweig_tip).branch.as_deref(), Some("gehaeuse-v2"));
    assert_eq!(g.active_branch.as_deref(), Some("main"));
}

#[test]
fn a_single_linear_history_still_renders_as_one_line() {
    // Acceptance #3 over a real repo: one branch, no divergence => exactly one lane.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    git(root, &["checkout", "-q", "-b", "main"]);
    commit_file(root, "a.txt", "1", "auto: a.txt, 2026-05-01T00:00:00Z");
    commit_file(root, "b.txt", "2", "auto: b.txt, 2026-05-02T00:00:00Z");

    let g = read_graph(root).unwrap();
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.lane_count, 1, "single linear history => one lane");
    assert!(g.nodes.iter().all(|n| n.lane == 0 && n.on_active));
}
