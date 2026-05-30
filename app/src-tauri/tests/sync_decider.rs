//! End-to-end test of the stiller Sync glue (Issue #11, E41).
//!
//! A **bare local repo** stands in for the self-hosted Forgejo/Gitea remote — the daily silent
//! sync is exercised against `file://…/remote.git`. NOTHING here ever touches a real server or LFS
//! endpoint. The pure routing/marker guarantees (any unmergeable touch → loud; no input ever
//! produces a git conflict marker) are proven exhaustively by the table/property tests in
//! `src/syncdecider.rs`. This file proves only that the side-effecting glue wires up against git:
//! that a free, mergeable divergence merges SILENTLY with no prompt and no markers, and that a
//! divergence on an UNMERGEABLE file (binary / KiCad) STOPS the sync loudly WITHOUT merging — so a
//! merge never silently corrupts the file (E41).

use app_lib::syncdecider::text_has_git_marker;
use app_lib::syncglue::{run_sync, SyncStatus, SHARED_BRANCH};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Two product clones (`anna`, `ben`) of one bare "remote", each on `main`.
fn seed_two_clones(tmp: &Path) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let bare = tmp.join("remote.git");
    let out = Command::new("git").args(["init", "--bare", "-b", "main"]).arg(&bare).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let url = format!("file://{}", bare.display());

    // Anna seeds the shared main with a baseline.
    let anna = tmp.join("anna");
    std::fs::create_dir_all(&anna).unwrap();
    git(&anna, &["init", "-b", "main"]);
    git(&anna, &["config", "user.name", "anna"]);
    git(&anna, &["config", "user.email", "anna@example.com"]);
    std::fs::write(anna.join("firmware.c"), b"int main(){return 0;}\n").unwrap();
    std::fs::write(anna.join("gehaeuse.f3d"), b"BINARYv1").unwrap();
    git(&anna, &["add", "-A"]);
    git(&anna, &["commit", "-m", "auto: baseline"]);
    git(&anna, &["remote", "add", "origin", &url]);
    git(&anna, &["push", "--set-upstream", "origin", "main"]);

    // Ben clones the same baseline.
    let ben = tmp.join("ben");
    let out = Command::new("git").args(["clone", &url]).arg(&ben).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    git(&ben, &["config", "user.name", "ben"]);
    git(&ben, &["config", "user.email", "ben@example.com"]);

    (bare, anna, ben)
}

/// Ben publishes a change to a FREE, mergeable text file. When Anna runs the silent sync, the
/// divergence resolves via a silent merge: status "gesichert", NO prompt, the file is updated, and
/// nothing git-flavoured surfaces.
#[test]
fn free_mergeable_divergence_merges_silently_no_markers() {
    let tmp = tempfile::tempdir().unwrap();
    let (_bare, anna, ben) = seed_two_clones(tmp.path());

    // Ben adds a free text file and publishes to shared main.
    std::fs::write(ben.join("docs.md"), b"# docs from ben\n").unwrap();
    git(&ben, &["add", "-A"]);
    git(&ben, &["commit", "-m", "auto: docs.md"]);
    git(&ben, &["push", "origin", "main"]);

    // Anna makes her own non-conflicting local text commit so the histories genuinely diverge.
    std::fs::write(anna.join("notes.txt"), b"annas notizen\n").unwrap();
    git(&anna, &["add", "-A"]);
    git(&anna, &["commit", "-m", "auto: notes.txt"]);

    let outcome = run_sync(&anna, Some("Ben".to_string())).unwrap();
    assert_eq!(
        outcome.status,
        SyncStatus::Gesichert,
        "free divergence must silently merge to 'gesichert' (no prompt): {:?}",
        outcome.status
    );

    // Ben's free change actually arrived (the merge ran).
    let docs = std::fs::read_to_string(anna.join("docs.md")).unwrap();
    assert!(docs.contains("from ben"), "the silent merge brought in Ben's free text");

    // And no file in the worktree carries a git conflict marker.
    assert!(no_conflict_markers_in_tree(&anna), "a silent merge must leave no conflict markers");
}

/// Ben publishes a change to the SAME UNMERGEABLE binary Anna also changed locally. The silent
/// sync must STOP loudly: status is the laute Ausnahme with a domain-language question — and it
/// must NOT have merged (so the binary is never corrupted), and the question carries no git
/// marker.
#[test]
fn unmergeable_divergence_stops_loudly_without_merging() {
    let tmp = tempfile::tempdir().unwrap();
    let (_bare, anna, ben) = seed_two_clones(tmp.path());

    // Ben changes the binary and publishes.
    std::fs::write(ben.join("gehaeuse.f3d"), b"BINARYv2-ben").unwrap();
    git(&ben, &["add", "-A"]);
    git(&ben, &["commit", "-m", "auto: gehaeuse.f3d"]);
    git(&ben, &["push", "origin", "main"]);

    // Anna changes the SAME binary locally — a real, unmergeable contradiction.
    std::fs::write(anna.join("gehaeuse.f3d"), b"BINARYv2-anna").unwrap();
    git(&anna, &["add", "-A"]);
    git(&anna, &["commit", "-m", "auto: gehaeuse.f3d"]);
    let anna_head_before = head(&anna);

    let outcome = run_sync(&anna, Some("Ben".to_string())).unwrap();
    match outcome.status {
        SyncStatus::LauteAusnahme(q) => {
            // domain language, names the artifact + colleague, asks whose stand applies
            assert!(q.frage.contains("Gehaeuse"), "names the contested artifact: {}", q.frage);
            assert!(q.frage.contains("Ben"), "names the colleague: {}", q.frage);
            assert!(q.frage.contains("welcher gilt"), "asks whose stand applies: {}", q.frage);
            assert!(q.artefakte.iter().any(|a| a.ends_with("gehaeuse.f3d")));
            // NEVER a git conflict marker, anywhere in the rendered question
            assert!(!q.contains_git_marker(), "loud question leaked a git marker: {q:?}");
            assert!(!text_has_git_marker(&q.frage));
        }
        other => panic!("expected the laute Ausnahme, got {other:?}"),
    }

    // CRUCIAL (E41): the sync did NOT merge — Anna's HEAD is unchanged, her binary untouched, so a
    // merge never silently corrupted the file.
    assert_eq!(head(&anna), anna_head_before, "a loud exception must NOT have merged");
    assert_eq!(
        std::fs::read(anna.join("gehaeuse.f3d")).unwrap(),
        b"BINARYv2-anna",
        "the binary is left exactly as the user had it — never corrupted by a merge"
    );
    assert!(no_conflict_markers_in_tree(&anna), "no conflict markers were written");
}

/// When the two stands already agree, the silent sync is a no-op: "aktuell", nothing shown.
#[test]
fn agreeing_stands_are_aktuell() {
    let tmp = tempfile::tempdir().unwrap();
    let (_bare, anna, _ben) = seed_two_clones(tmp.path());
    let outcome = run_sync(&anna, Some("Ben".to_string())).unwrap();
    assert_eq!(outcome.status, SyncStatus::Aktuell, "no divergence -> aktuell");
}

// --- helpers ---

fn head(root: &Path) -> String {
    let out = Command::new("git").arg("-C").arg(root).args(["rev-parse", SHARED_BRANCH]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Scan every tracked working-tree file for a git conflict marker — the acid test that a sync
/// never left one behind.
fn no_conflict_markers_in_tree(root: &Path) -> bool {
    let out = Command::new("git").arg("-C").arg(root).args(["ls-files"]).output().unwrap();
    for rel in String::from_utf8_lossy(&out.stdout).lines() {
        let p = root.join(rel.trim());
        if let Ok(text) = std::fs::read_to_string(&p) {
            if text_has_git_marker(&text) {
                return false;
            }
        }
    }
    true
}
