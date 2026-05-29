//! End-to-end read-path test (Issue #2): build a real product folder on disk,
//! project it, and assert the domain view the UI renders. No GUI needed.

use app_lib::projection::{project_product, Baustein};
use std::fs;
use std::path::Path;

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

fn find<'a>(view: &'a [Baustein], path: &str) -> &'a Baustein {
    view.iter()
        .find(|b| b.path == path)
        .unwrap_or_else(|| panic!("no Baustein at {path}; got {view:?}"))
}

#[test]
fn projects_leaf_folders_as_bausteine_with_representative_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // A product: leaf folders are Bausteine; intermediate folders are not.
    touch(&root.join("elektronik/regler/regler.kicad_pcb"));
    touch(&root.join("elektronik/regler/regler.kicad_pro"));
    touch(&root.join("elektronik/regler/notes.md"));
    touch(&root.join("mechanik/gehaeuse/gehaeuse.f3d"));
    touch(&root.join("mechanik/gehaeuse/render.png"));
    touch(&root.join("mechanik/halter/halter.step"));
    touch(&root.join("README.md")); // root has subdirs -> root is not a Baustein
    fs::create_dir_all(root.join("docs")).unwrap(); // empty -> ignored
    fs::create_dir_all(root.join(".git")).unwrap(); // hidden -> ignored

    let view = project_product(root).unwrap();

    // Exactly the three leaf folders, sorted by path.
    let paths: Vec<&str> = view.bausteine.iter().map(|b| b.path.as_str()).collect();
    assert_eq!(
        paths,
        ["elektronik/regler", "mechanik/gehaeuse", "mechanik/halter"]
    );

    // Representative file = highest-ranked, paths root-relative with forward slashes.
    assert_eq!(
        find(&view.bausteine, "elektronik/regler").main_file.as_deref(),
        Some("elektronik/regler/regler.kicad_pro") // .kicad_pro outranks .kicad_pcb / .md
    );
    assert_eq!(
        find(&view.bausteine, "mechanik/gehaeuse").main_file.as_deref(),
        Some("mechanik/gehaeuse/gehaeuse.f3d") // .f3d outranks .png
    );
    assert_eq!(
        find(&view.bausteine, "mechanik/halter").main_file.as_deref(),
        Some("mechanik/halter/halter.step")
    );

    // Names are folder names (UI uppercases them).
    assert_eq!(find(&view.bausteine, "mechanik/gehaeuse").name, "gehaeuse");
}

#[test]
fn defaults_branch_and_version_without_git() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("part/body.f3d"));

    let view = project_product(dir.path()).unwrap();
    assert_eq!(view.branch, "main");
    assert_eq!(view.version, "v0.0");
}

#[test]
fn reads_branch_from_git_head() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("part/body.f3d"));
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/HEAD"), "ref: refs/heads/gehaeuse-v2\n").unwrap();

    let view = project_product(root).unwrap();
    assert_eq!(view.branch, "gehaeuse-v2");
}

#[test]
fn flat_folder_of_files_is_a_single_baustein() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("body.f3d"));
    touch(&root.join("readme.txt"));

    let view = project_product(root).unwrap();
    assert_eq!(view.bausteine.len(), 1);
    assert_eq!(view.bausteine[0].path, ".");
    assert_eq!(view.bausteine[0].main_file.as_deref(), Some("body.f3d"));
}
