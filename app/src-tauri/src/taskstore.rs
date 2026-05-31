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

use crate::tasks::{add_task, edit_task, remove_task, set_status, NewTask, Task, TaskEdit, TaskStatus};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The tool's committed, shared store directory (ADR 0002). `projection.rs` skips it by name.
pub const PLM_DIR: &str = "_plm";
/// File that holds the product's Aufgaben & Hinweise, inside `_plm/` (ADR 0002).
pub const TASKS_FILE: &str = "aufgaben.json";

/// Absolute path of the `_plm/aufgaben.json` task file for a product `root`.
fn tasks_path(root: &Path) -> PathBuf {
    root.join(PLM_DIR).join(TASKS_FILE)
}

/// Read the persisted task list for a product. A missing/empty/corrupt file means **zero
/// tasks** (opt-in) — not an error.
pub fn read_tasks(root: &Path) -> Vec<Task> {
    let raw = std::fs::read_to_string(tasks_path(root)).unwrap_or_default();
    if raw.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist the task list, pretty-printed for an honest, diffable on-disk record. Creates the
/// `_plm/` directory as needed before writing.
fn write_tasks(root: &Path, tasks: &[Task]) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join(PLM_DIR))?;
    let json = serde_json::to_string_pretty(tasks).map_err(std::io::Error::other)?;
    std::fs::write(tasks_path(root), json)
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
        fs::create_dir_all(dir.join(PLM_DIR)).unwrap();
        fs::write(tasks_path(&dir), "{ not json ]").unwrap();
        assert!(read_tasks(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_to_the_new_plm_location() {
        let dir = tmp();
        create_task(&dir, new("x", TaskKind::Aufgabe, None)).unwrap();
        assert!(dir.join(PLM_DIR).join(TASKS_FILE).is_file(), "tasks live in _plm/aufgaben.json");
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
