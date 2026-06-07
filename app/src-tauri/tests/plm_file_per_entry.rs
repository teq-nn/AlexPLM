//! End-to-end per-entry `_plm` I/O tests (Issue #132, E54): build real product folders on disk and
//! assert that each of the four coordination concerns — Aufgaben, Release-Pointer, Kanten,
//! Zuordnungen — stores **one ID-named JSON file per entry** under its `_plm/<belang>/` directory.
//!
//! The point of E54 is that two **concurrently created** entries land in two **different** files, so
//! Werkbank's own coordination files never produce a merge conflict on `_plm`. We prove that by
//! simulating the two sides of a clean git merge: side A writes its entry through the store, then
//! side B's entry is dropped in as the *additional* file a conflict-free merge would yield — and the
//! store reads both back. We also pin the degradation invariant (missing/empty/corrupt ⇒ empty
//! state, never an error, now **per file**) and the migration of the legacy single array/map files.
//!
//! The exhaustive per-method behaviour lives in the `#[cfg(test)]` unit tests inside each store and
//! in `plmstore.rs`; here we prove the side-effecting glue wires up correctly against real folders.

use app_lib::artstore::{read_art, set_art, ART_DIR, ART_FILE};
use app_lib::edgestore::{add_persisted_edge, read_edges, EDGES_DIR, EDGES_FILE};
use app_lib::graph::RevisionArt;
use app_lib::plmstore::PLM_DIR;
use app_lib::taskstore::{create_task, read_tasks, TASKS_DIR, TASKS_FILE};
use app_lib::tasks::{NewTask, TaskKind};
use app_lib::zuordnungstore::{assign, read_overrides, ZUORDNUNG_DIR, ZUORDNUNG_FILE};
use std::fs;
use std::path::Path;

/// Count the `.json` files directly under `_plm/<belang>/` — the per-entry files of one concern.
fn entry_file_count(root: &Path, belang: &str) -> usize {
    let dir = root.join(PLM_DIR).join(belang);
    let Ok(entries) = fs::read_dir(&dir) else { return 0 };
    entries
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count()
}

fn new_task(title: &str) -> NewTask {
    NewTask {
        title: title.to_string(),
        kind: TaskKind::Aufgabe,
        link: None,
        due: None,
        blocks_everywhere: false,
    }
}

/// Each concern lays its entries out as one ID-named file per entry under `_plm/<belang>/`.
#[test]
fn each_concern_writes_one_file_per_entry() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    create_task(root, new_task("Gehäuse prüfen")).unwrap();
    create_task(root, new_task("Platine routen")).unwrap();
    assert_eq!(entry_file_count(root, TASKS_DIR), 2, "two tasks -> two files");

    set_art(root, "v1.0", RevisionArt::Freigabe).unwrap();
    set_art(root, "v0.9", RevisionArt::Prototyp).unwrap();
    assert_eq!(entry_file_count(root, ART_DIR), 2, "two Release-Pointer -> two files");

    add_persisted_edge(root, "fertigung/stl", "mechanik/gehaeuse").unwrap();
    add_persisted_edge(root, "gerber/zip", "elektronik/board").unwrap();
    assert_eq!(entry_file_count(root, EDGES_DIR), 2, "two edges -> two files");

    assign(root, "elektronik/board.kicad_pcb", "kicad").unwrap();
    assign(root, "mechanik/teil.FCStd", "fusion").unwrap();
    assert_eq!(entry_file_count(root, ZUORDNUNG_DIR), 2, "two Zuordnungen -> two files");
}

/// E54's core promise: two entries created on two sides of a merge never collide. We model the merge
/// by letting side A persist its entry through the store, then placing side B's entry as the extra
/// file a clean three-way merge would add — and the store must read **both**, no conflict.
#[test]
fn two_concurrently_created_entries_merge_without_collision() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Side A creates a task through the real store.
    create_task(root, new_task("Aufgabe von Anna")).unwrap();
    let anna_id = read_tasks(root)[0].id.clone();

    // Side B independently created its own task in its own clone; a clean git merge just brings the
    // extra file along. We reproduce that by writing a second, separate entry file by hand.
    let bert_file = root.join(PLM_DIR).join(TASKS_DIR).join("tBERT.json");
    let bert = serde_json::json!({
        "id": "tBERT",
        "title": "Aufgabe von Bert",
        "kind": "aufgabe",
        "status": "offen",
        "created_at": "2026-06-07T00:00:00Z",
    });
    fs::write(&bert_file, serde_json::to_string_pretty(&bert).unwrap()).unwrap();

    // Both tasks come back — the two creations never shared a file, so nothing conflicted.
    let titles: Vec<String> = read_tasks(root).into_iter().map(|t| t.title).collect();
    assert!(titles.contains(&"Aufgabe von Anna".to_string()));
    assert!(titles.contains(&"Aufgabe von Bert".to_string()));
    assert_ne!(anna_id, "tBERT", "the two entries are genuinely two distinct files");
}

/// The degradation invariant, now per file: a single hand-mangled / empty entry is skipped and a
/// missing directory is the empty state — the rest of the concern survives, never an error.
#[test]
fn one_corrupt_entry_is_skipped_the_rest_survive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // a healthy task plus a hand-mangled and a blank neighbour in the same belang directory.
    create_task(root, new_task("heile Aufgabe")).unwrap();
    let belang = root.join(PLM_DIR).join(TASKS_DIR);
    fs::write(belang.join("tCORRUPT.json"), "{ not json ]").unwrap();
    fs::write(belang.join("tBLANK.json"), "   ").unwrap();

    let tasks = read_tasks(root);
    assert_eq!(tasks.len(), 1, "the two broken entries are skipped, the healthy one survives");
    assert_eq!(tasks[0].title, "heile Aufgabe");
}

/// Missing stores read as empty for every concern — never an error.
#[test]
fn missing_stores_read_as_empty_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(read_tasks(root).is_empty());
    assert!(read_edges(root).is_empty());
    assert!(read_overrides(root).is_empty());
    assert_eq!(read_art(root, "v1.0"), RevisionArt::Prototyp, "unrecorded tag is the lax default");
}

/// Migration: a product carrying only the **legacy** single array/map files for every concern must
/// keep its data (not be silently emptied), and the next write lays each concern out per entry.
#[test]
fn legacy_single_files_migrate_to_per_entry_on_next_write() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let plm = root.join(PLM_DIR);
    fs::create_dir_all(&plm).unwrap();

    // Aufgaben: legacy array file.
    let legacy_tasks = serde_json::json!([
        { "id": "t-1", "title": "Alt-Aufgabe", "kind": "aufgabe", "status": "offen",
          "created_at": "2026-01-01T00:00:00Z" }
    ]);
    fs::write(plm.join(TASKS_FILE), serde_json::to_string_pretty(&legacy_tasks).unwrap()).unwrap();

    // Kanten: legacy array file.
    let legacy_edges = serde_json::json!([
        { "derived": "fertigung/stl", "source": "mechanik/gehaeuse" }
    ]);
    fs::write(plm.join(EDGES_FILE), serde_json::to_string_pretty(&legacy_edges).unwrap()).unwrap();

    // Release-Pointer: legacy map file.
    let legacy_art = serde_json::json!({ "v1.0": "freigabe" });
    fs::write(plm.join(ART_FILE), serde_json::to_string_pretty(&legacy_art).unwrap()).unwrap();

    // Zuordnungen: legacy map file.
    let legacy_zuord = serde_json::json!({ "hardware/teil.FCStd": "fusion" });
    fs::write(plm.join(ZUORDNUNG_FILE), serde_json::to_string_pretty(&legacy_zuord).unwrap()).unwrap();

    // Reads fold the legacy files in — nothing is lost.
    assert_eq!(read_tasks(root).len(), 1);
    assert_eq!(read_edges(root).len(), 1);
    assert_eq!(read_art(root, "v1.0"), RevisionArt::Freigabe);
    assert_eq!(read_overrides(root).get("hardware/teil.FCStd").map(String::as_str), Some("fusion"));

    // The next write of each concern materialises its per-entry directory without dropping the old data.
    create_task(root, new_task("Neu")).unwrap();
    add_persisted_edge(root, "neu/a", "neu/b").unwrap();
    set_art(root, "v2.0", RevisionArt::Freigabe).unwrap();
    assign(root, "elektronik/board.kicad_pcb", "kicad").unwrap();

    assert!(plm.join(TASKS_DIR).is_dir() && read_tasks(root).len() == 2);
    assert!(plm.join(EDGES_DIR).is_dir() && read_edges(root).len() == 2);
    assert!(plm.join(ART_DIR).is_dir() && read_art(root, "v1.0") == RevisionArt::Freigabe);
    assert!(plm.join(ZUORDNUNG_DIR).is_dir() && read_overrides(root).len() == 2);
}
