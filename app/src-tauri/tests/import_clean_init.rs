//! End-to-end import test (Issue #3, E38): build a real folder on disk, run the clean
//! non-destructive import, and assert git-init behaviour, the `.gitattributes` markers the
//! Mergeability Classifier produces (incl. KiCad and the override case), the first commit,
//! and that the imported product projects into the shell view. No GUI needed.

use app_lib::import::import_folder;
use std::fs;
use std::path::Path;
use std::process::Command;

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

fn git(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").arg("-C").arg(root).args(args).output().unwrap()
}

fn is_repo(root: &Path) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"]).status.success()
}

fn commit_count(root: &Path) -> usize {
    let out = git(root, &["rev-list", "--count", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

fn product_with_all_three_buckets(root: &Path) {
    // text-mergeable, binary-unmergeable, nominal-text-unmergeable (KiCad) all present
    touch(&root.join("firmware/regler/main.c"));
    touch(&root.join("firmware/regler/notes.md"));
    touch(&root.join("elektronik/board/board.kicad_pcb"));
    touch(&root.join("elektronik/board/board.kicad_sch"));
    touch(&root.join("mechanik/gehaeuse/gehaeuse.f3d"));
    touch(&root.join("mechanik/gehaeuse/render.png"));
}

#[test]
fn non_git_folder_gets_initialized_and_first_commit_with_lockable_markers() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    product_with_all_three_buckets(root);

    assert!(!is_repo(root), "precondition: not a git repo yet");

    let result = import_folder(root).unwrap();

    // git init ran; first commit created.
    assert!(result.git_initialized, "non-git folder should be git init'd");
    assert!(is_repo(root));
    assert_eq!(commit_count(root), 1, "exactly the first commit");

    // .gitattributes written with lockable markers for the unmergeable buckets only.
    let attrs = fs::read_to_string(root.join(".gitattributes")).unwrap();
    assert!(attrs.contains("*.f3d") && attrs.contains("lockable"), "binary -> lockable");
    assert!(attrs.contains("*.png"), "binary image -> lockable");
    assert!(attrs.contains("*.kicad_pcb"), "KiCad pcb -> lockable");
    assert!(attrs.contains("*.kicad_sch"), "KiCad sch -> lockable");
    // text-mergeable buckets are NOT locked.
    assert!(!attrs.contains("*.c "), "text .c must not be lockable");
    assert!(!attrs.contains("*.md "), "text .md must not be lockable");

    // every lockable line carries the lockable marker
    for line in attrs.lines().filter(|l| l.starts_with("*.")) {
        assert!(line.contains("lockable"), "rule line should be lockable: {line}");
    }

    // 4 leaf files are lockable: f3d, png, kicad_pcb, kicad_sch
    assert_eq!(result.locked_count, 4, "f3d + png + kicad_pcb + kicad_sch");

    // imported product projects into the shell view.
    assert_eq!(result.product.bausteine.len(), 3);
    let paths: Vec<&str> = result.product.bausteine.iter().map(|b| b.path.as_str()).collect();
    assert_eq!(paths, ["elektronik/board", "firmware/regler", "mechanik/gehaeuse"]);
}

#[test]
fn already_git_folder_is_left_as_is() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("part/body.f3d"));

    // Pre-existing repo with one commit and a distinctive branch.
    assert!(git(root, &["init"]).status.success());
    git(root, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--allow-empty", "-m", "seed"]);
    git(root, &["branch", "-M", "gehaeuse-v2"]);
    let before = commit_count(root);

    let result = import_folder(root).unwrap();

    // No re-init; existing history preserved (our commit adds to it, never rewrites).
    assert!(!result.git_initialized, "existing repo must be left as-is");
    assert!(commit_count(root) >= before, "history preserved, import adds a commit");
    // branch untouched -> projection still reads it
    assert_eq!(result.product.branch, "gehaeuse-v2");
}

#[test]
fn existing_gitattributes_marker_overrides_classification() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // A KiCad pcb that the maintainer has explicitly declared plain text, and a .txt they
    // declared lockable — both must override the extension-based default.
    touch(&root.join("elektronik/board/board.kicad_pcb"));
    touch(&root.join("elektronik/board/weird.txt"));
    fs::write(
        root.join(".gitattributes"),
        "*.kicad_pcb text\n*.txt lockable\n",
    )
    .unwrap();

    let result = import_folder(root).unwrap();
    let attrs = fs::read_to_string(root.join(".gitattributes")).unwrap();

    // override: KiCad forced text -> import adds NO lockable rule for it, despite it being
    // merge-hostile by default. The maintainer's own `*.kicad_pcb text` line is preserved
    // verbatim (the idempotent `_import` block never clobbers hand-edits, #63) — so we assert
    // on the *absence of a lockable rule*, not the absence of the pattern altogether.
    assert!(
        !attrs
            .lines()
            .any(|l| l.trim_start().starts_with("*.kicad_pcb") && l.contains("lockable")),
        "explicit text marker wins -> no lockable rule for kicad_pcb; got:\n{attrs}"
    );
    // override: plain .txt forced lockable -> locked despite being text by default.
    assert!(
        attrs.lines().any(|l| l.starts_with("*.txt") && l.contains("lockable")),
        "explicit lockable marker wins -> .txt locked; got:\n{attrs}"
    );
    assert_eq!(result.locked_count, 1, "only the overridden .txt is lockable");
}
