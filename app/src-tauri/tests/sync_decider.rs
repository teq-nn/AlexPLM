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

use app_lib::syncdecider::{text_has_git_marker, StandChoice};
use app_lib::syncglue::{resolve_sync, run_sync, SyncStatus, SHARED_BRANCH};
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

/// Issue #43, E41 — KiCad source (`.kicad_pcb`): a nominally-textual but factually unmergeable
/// file that BOTH sides changed must (a) stop loudly, never silently merge into „Missing („
/// corruption, then (b) once the user answers the loud question, `resolve_sync` must FINISH the
/// sync with the chosen side and leave NO conflict marker in the tree. Asserted for taking the
/// colleague's stand ("theirs").
#[test]
fn resolve_takes_theirs_for_kicad_and_leaves_no_markers() {
    let tmp = tempfile::tempdir().unwrap();
    let (_bare, anna, ben) = seed_two_clones(tmp.path());

    // Put a shared KiCad source into the common baseline both clones descend from, so the later
    // edits on each side are a genuine three-way divergence (not one ahead of the other).
    std::fs::write(anna.join("board.kicad_pcb"), b"(kicad_pcb\n  (layer F.Cu)\n  (net 0)\n)\n").unwrap();
    git(&anna, &["add", "-A"]);
    git(&anna, &["commit", "-m", "auto: kicad base"]);
    git(&anna, &["push", "origin", "main"]);
    git(&ben, &["pull", "--no-edit", "origin", "main"]);

    // Ben edits the KiCad source at the contested line and publishes.
    std::fs::write(ben.join("board.kicad_pcb"), b"(kicad_pcb\n  (layer F.Cu)\n  (net BEN)\n)\n").unwrap();
    git(&ben, &["add", "-A"]);
    git(&ben, &["commit", "-m", "auto: net ben"]);
    git(&ben, &["push", "origin", "main"]);

    // Anna edits the SAME line locally — a real, unmergeable contradiction over a KiCad source.
    std::fs::write(anna.join("board.kicad_pcb"), b"(kicad_pcb\n  (layer F.Cu)\n  (net ANNA)\n)\n").unwrap();
    git(&anna, &["add", "-A"]);
    git(&anna, &["commit", "-m", "auto: net anna"]);

    // The silent sync stops loudly — never merges the KiCad source.
    let outcome = run_sync(&anna, Some("Ben".to_string())).unwrap();
    assert!(
        matches!(outcome.status, SyncStatus::LauteAusnahme(_)),
        "a KiCad divergence must route loud, never silent-merge: {:?}",
        outcome.status
    );

    // The user answers: take Ben's stand. The sync finishes cleanly.
    let resolved = resolve_sync(&anna, "board.kicad_pcb", StandChoice::Theirs).unwrap();
    assert_eq!(resolved.status, SyncStatus::Gesichert, "resolve finishes the sync quietly");

    // Ben's whole side landed — taken whole, not line-merged — and NO marker survives.
    let pcb = std::fs::read_to_string(anna.join("board.kicad_pcb")).unwrap();
    assert!(pcb.contains("net BEN"), "the chosen (theirs) stand is now the file's content");
    assert!(!pcb.contains("net ANNA"), "the other side is gone from the worktree");
    assert!(!text_has_git_marker(&pcb), "no conflict marker in the resolved KiCad source");
    assert!(no_conflict_markers_in_tree(&anna), "resolution leaves no markers anywhere");
    assert!(merge_done(&anna), "the merge was committed — the sync is finished");
}

/// Issue #43 — the mirror case: keeping MY stand ("mine") over a contested binary. The user's own
/// bytes survive untouched, the merge is finished, and no marker is ever written. Also proves the
/// freely-mergeable text that rode along the same sync is still brought in.
#[test]
fn resolve_keeps_mine_for_binary_and_finishes_sync() {
    let tmp = tempfile::tempdir().unwrap();
    let (_bare, anna, ben) = seed_two_clones(tmp.path());

    // Ben changes the contested binary AND adds a free text file, then publishes both.
    std::fs::write(ben.join("gehaeuse.f3d"), b"BINARYv2-ben").unwrap();
    std::fs::write(ben.join("readme.md"), b"# from ben\n").unwrap();
    git(&ben, &["add", "-A"]);
    git(&ben, &["commit", "-m", "auto: gehaeuse + readme"]);
    git(&ben, &["push", "origin", "main"]);

    // Anna changes the SAME binary locally — the unmergeable contradiction.
    std::fs::write(anna.join("gehaeuse.f3d"), b"BINARYv2-anna").unwrap();
    git(&anna, &["add", "-A"]);
    git(&anna, &["commit", "-m", "auto: gehaeuse.f3d"]);

    assert!(matches!(
        run_sync(&anna, Some("Ben".to_string())).unwrap().status,
        SyncStatus::LauteAusnahme(_)
    ));

    // The user keeps her own stand for the binary.
    let resolved = resolve_sync(&anna, "gehaeuse.f3d", StandChoice::Mine).unwrap();
    assert_eq!(resolved.status, SyncStatus::Gesichert);

    // Anna's binary bytes survive whole; the free text that rode along still merged in.
    assert_eq!(
        std::fs::read(anna.join("gehaeuse.f3d")).unwrap(),
        b"BINARYv2-anna",
        "the chosen (mine) binary stand is kept whole"
    );
    let readme = std::fs::read_to_string(anna.join("readme.md")).unwrap();
    assert!(readme.contains("from ben"), "free text rode along the same sync and merged in");
    assert!(no_conflict_markers_in_tree(&anna), "no conflict markers in the resolved tree");
    assert!(merge_done(&anna), "the merge was committed");
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

/// Whether a merge commit is the current HEAD with no merge still in progress — i.e. the resolve
/// committed the merge and finished the sync.
fn merge_done(root: &Path) -> bool {
    // MERGE_HEAD is gone once the merge is committed.
    let in_progress = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", "--quiet", "MERGE_HEAD"])
        .output()
        .unwrap()
        .status
        .success();
    if in_progress {
        return false;
    }
    // HEAD has two parents → it is the merge commit the resolve created.
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-list", "--no-walk", "--count", "--merges", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim() == "1"
}

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
