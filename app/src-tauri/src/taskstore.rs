//! Persistence glue for Aufgaben & Hinweise (Issue #40).
//!
//! Thin, side-effecting layer that stores the product's task list as JSON in the product folder
//! (the product-local `_plm` store) and feeds the pure [`crate::tasks`] core. **All** filesystem
//! access, id minting and clock reads live here; the pure logic in `tasks.rs` never does I/O.
//! Same split as `edgestore.rs` over `edges.rs` and `graphread.rs` over `graph.rs`.
//!
//! The store is **opt-in**: a product with no task file has zero tasks. Reading a
//! missing/empty/corrupt file yields an empty list — never an error — so the list view degrades
//! to „keine Aufgaben" rather than breaking the shell.

use crate::plmstore::{self, PlmCollection, PLM_DIR};
use crate::tasks::{add_task, edit_task, remove_task, set_status, NewTask, Task, TaskEdit, TaskStatus};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Legacy single-array file of the product's Aufgaben & Hinweise, inside `_plm/` (ADR 0002).
/// Now migrated to one file per task under `_plm/aufgaben/` (E54, Issue #132); kept for migration.
pub const TASKS_FILE: &str = "aufgaben.json";
/// Per-entry directory holding one JSON file per task, keyed by task id (E54).
pub const TASKS_DIR: &str = "aufgaben";

/// The Aufgaben collection — **one ID-named file per task** under `_plm/aufgaben/` (E54). Path,
/// per-file degradation and the atomic pretty write live in the deep [`PlmCollection`] layer; this
/// store mints ids, reads the clock and feeds the pure `tasks` core. Keyed by [`Task::id`], so two
/// newly created tasks never share a file.
///
/// The legacy `_plm/aufgaben.json` was a **`Vec<Task>` array**, not the `key → Task` map this
/// collection persists, so its migration is done by hand in [`read_tasks`] (reading the array
/// directly) rather than through the collection's built-in (map-shaped) legacy path.
const TASKS: PlmCollection<Task> = PlmCollection::new(TASKS_DIR, TASKS_FILE);

/// Absolute path of the legacy `_plm/aufgaben.json` single-array file for a product `root`.
fn array_tasks_path(root: &Path) -> PathBuf {
    root.join(PLM_DIR).join(TASKS_FILE)
}

/// Read the persisted task list for a product. A missing/empty/corrupt entry means **zero
/// tasks** (opt-in) — not an error; a single mangled task file is skipped, not fatal.
///
/// The on-disk store is a `task-id → Task` map (one file each); the list is its values, ordered by
/// id. Ids are time-based ([`mint_id`]), so id order is creation order — the view is stable.
/// Migration (E54): when the per-entry directory is **absent**, the legacy `_plm/aufgaben.json`
/// array is read instead so existing products are not silently emptied. The next write lays the
/// tasks out one file per task.
pub fn read_tasks(root: &Path) -> Vec<Task> {
    if TASKS.dir_path(root).is_dir() {
        TASKS.read(root).into_values().collect()
    } else {
        plmstore::read_or_default::<Vec<Task>>(&array_tasks_path(root))
    }
}

/// Persist the task list as one file per task (pretty + atomic, creating `_plm/aufgaben/` as needed).
fn write_tasks(root: &Path, tasks: &[Task]) -> std::io::Result<()> {
    let map: BTreeMap<String, Task> =
        tasks.iter().map(|t| (t.id.clone(), t.clone())).collect();
    TASKS.write(root, &map)
}

/// A machine timestamp `YYYY-MM-DDTHH:MM:SSZ` for `now` — reused from the auto-commit layer so
/// every on-disk timestamp in the product shares one shape (lexicographically orderable).
fn now_stamp() -> String {
    crate::autocommit::format_timestamp(SystemTime::now())
}

/// Mint a stable opaque id for a new task. Time-based + a process/counter tail so two tasks
/// created in the same second never collide. Opaque to the UI (it only keys rows on it).
fn mint_id(existing: &[Task]) -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // include the current count so a fast burst within one nanos tick still differs
    format!("t{nanos}-{}", existing.len())
}

/// Create a task (Aufgabe or Hinweis) and persist it. The store mints the id and reads the
/// clock; the pure [`add_task`] does the rest. Returns the refreshed list. A blank title is a
/// no-op in the core (the list comes back unchanged).
pub fn create_task(root: &Path, new: NewTask) -> std::io::Result<Vec<Task>> {
    let existing = read_tasks(root);
    let id = mint_id(&existing);
    let tasks = add_task(existing, id, now_stamp(), new);
    write_tasks(root, &tasks)?;
    Ok(tasks)
}

/// Apply an edit to one task and persist the result. Absent id ⇒ unchanged list. Returns the
/// refreshed list.
pub fn update_task(root: &Path, id: &str, edit: TaskEdit) -> std::io::Result<Vec<Task>> {
    let tasks = edit_task(read_tasks(root), id, edit);
    write_tasks(root, &tasks)?;
    Ok(tasks)
}

/// Set just the status of one task (erledigen / verwerfen / wieder öffnen) and persist. Returns
/// the refreshed list.
pub fn set_task_status(root: &Path, id: &str, status: TaskStatus) -> std::io::Result<Vec<Task>> {
    let tasks = set_status(read_tasks(root), id, status);
    write_tasks(root, &tasks)?;
    Ok(tasks)
}

/// Delete one task and persist the result. Absent id ⇒ no-op. Returns the refreshed list.
pub fn delete_task(root: &Path, id: &str) -> std::io::Result<Vec<Task>> {
    let tasks = remove_task(read_tasks(root), id);
    write_tasks(root, &tasks)?;
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{TaskKind, TaskLink};
    use std::fs;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-tasks-ut-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A fully-formed [`Task`] for seeding the legacy array file (id is overridden by the caller).
    fn tasks_seed(title: &str) -> Task {
        Task {
            id: String::new(),
            title: title.to_string(),
            kind: TaskKind::Aufgabe,
            status: TaskStatus::Offen,
            link: None,
            due: None,
            blocks_everywhere: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn new(title: &str, kind: TaskKind, link: Option<TaskLink>) -> NewTask {
        NewTask {
            title: title.to_string(),
            kind,
            link,
            due: None,
            blocks_everywhere: false,
        }
    }

    #[test]
    fn missing_file_reads_as_zero_tasks() {
        let dir = tmp();
        assert!(read_tasks(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_degrades_to_zero_tasks() {
        let dir = tmp();
        // a single hand-mangled entry file is skipped per-file, never fatal.
        let path = TASKS.entry_path(&dir, "t-corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json ]").unwrap();
        assert!(read_tasks(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_one_file_per_task_under_the_plm_dir() {
        let dir = tmp();
        create_task(&dir, new("x", TaskKind::Aufgabe, None)).unwrap();
        let id = read_tasks(&dir)[0].id.clone();
        assert!(TASKS.entry_path(&dir, &id).is_file(), "each task is its own file under _plm/aufgaben/");
        let _ = fs::remove_dir_all(&dir);
    }

    /// E54: two newly created tasks land in two separate files — no shared line to merge-conflict on.
    #[test]
    fn two_created_tasks_land_in_separate_files() {
        let dir = tmp();
        create_task(&dir, new("a", TaskKind::Aufgabe, None)).unwrap();
        create_task(&dir, new("b", TaskKind::Hinweis, None)).unwrap();
        let tasks = read_tasks(&dir);
        let p0 = TASKS.entry_path(&dir, &tasks[0].id);
        let p1 = TASKS.entry_path(&dir, &tasks[1].id);
        assert_ne!(p0, p1, "distinct ids -> distinct files");
        assert!(p0.is_file() && p1.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Migration: a product that only has the legacy `_plm/aufgaben.json` array file must keep its
    /// tasks (not be silently emptied); the next write lays them out one file per task.
    #[test]
    fn migrates_legacy_aufgaben_array_file() {
        let dir = tmp();
        // seed the old single-array file the way pre-E54 Werkbank wrote it.
        let legacy = vec![
            Task { id: "t-1".to_string(), ..tasks_seed("Alt-Aufgabe") },
            Task { id: "t-2".to_string(), ..tasks_seed("Alt-Hinweis") },
        ];
        let path = dir.join("_plm").join(TASKS_FILE);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        // read folds the legacy array in.
        let read = read_tasks(&dir);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].title, "Alt-Aufgabe");

        // the next mutation materialises the per-entry directory without losing the old tasks.
        create_task(&dir, new("Neu", TaskKind::Aufgabe, None)).unwrap();
        assert!(TASKS.entry_path(&dir, "t-1").is_file(), "legacy task written out per file");
        assert_eq!(read_tasks(&dir).len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_then_read_round_trips_with_link() {
        let dir = tmp();
        create_task(
            &dir,
            new("Gehäuse prüfen", TaskKind::Aufgabe, Some(TaskLink::Artefakt("mechanik/gehaeuse".to_string()))),
        )
        .unwrap();
        let tasks = read_tasks(&dir);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Gehäuse prüfen");
        assert_eq!(tasks[0].kind, TaskKind::Aufgabe);
        assert_eq!(tasks[0].status, TaskStatus::Offen);
        assert_eq!(tasks[0].link, Some(TaskLink::Artefakt("mechanik/gehaeuse".to_string())));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn created_tasks_get_distinct_ids() {
        let dir = tmp();
        create_task(&dir, new("a", TaskKind::Aufgabe, None)).unwrap();
        create_task(&dir, new("b", TaskKind::Hinweis, None)).unwrap();
        let tasks = read_tasks(&dir);
        assert_eq!(tasks.len(), 2);
        assert_ne!(tasks[0].id, tasks[1].id);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_status_and_delete_persist() {
        let dir = tmp();
        create_task(&dir, new("x", TaskKind::Aufgabe, None)).unwrap();
        let id = read_tasks(&dir)[0].id.clone();
        set_task_status(&dir, &id, TaskStatus::Erledigt).unwrap();
        assert_eq!(read_tasks(&dir)[0].status, TaskStatus::Erledigt);
        delete_task(&dir, &id).unwrap();
        assert!(read_tasks(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
