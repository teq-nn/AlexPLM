//! Aufgaben & Hinweise tests (Issue #40).
//!
//! Two layers, matching the project's "reiner Kern + Tabellentest" pattern:
//!
//! 1. **Pure task list -> task list (no I/O).** The model is exercised purely over hand-built
//!    lists — block-capability (the ONLY thing separating Aufgabe from Hinweis), the optional-
//!    link variants, and the lifecycle edits. These are the acceptance-criterion tests.
//! 2. **Persistence glue.** A real temp folder round-trips the list through the `_plm` store —
//!    exercising the thin I/O layer over the pure core, including the optional-link variants.

use app_lib::tasks::{add_task, set_status, NewTask, Task, TaskKind, TaskLink, TaskStatus};
use app_lib::taskstore::{create_task, delete_task, read_tasks, set_task_status, update_task};
use app_lib::tasks::TaskEdit;
use std::fs;
use std::path::PathBuf;

fn new(title: &str, kind: TaskKind, link: Option<TaskLink>) -> NewTask {
    NewTask {
        title: title.to_string(),
        kind,
        link,
        due: None,
        blocks_everywhere: false,
    }
}

// ---- Layer 1: pure list -> list, no I/O -----------------------------------------------

/// Acceptance, US 27: Aufgabe vs. Hinweis differ ONLY by Blockier-Fähigkeit. An open Aufgabe is
/// block-capable; a Hinweis never is, regardless of status; a done/dropped Aufgabe is not.
#[test]
fn block_capability_separates_aufgabe_from_hinweis() {
    // (kind, status, expect_capable)
    let cases: &[(TaskKind, TaskStatus, bool)] = &[
        (TaskKind::Aufgabe, TaskStatus::Offen, true),
        (TaskKind::Aufgabe, TaskStatus::Erledigt, false),
        (TaskKind::Aufgabe, TaskStatus::Verworfen, false),
        (TaskKind::Hinweis, TaskStatus::Offen, false),
        (TaskKind::Hinweis, TaskStatus::Erledigt, false),
        (TaskKind::Hinweis, TaskStatus::Verworfen, false),
    ];
    for (kind, status, expect) in cases {
        let mut tasks = add_task(Vec::new(), "t", "ts", new("x", *kind, None));
        tasks = set_status(tasks, "t", *status);
        assert_eq!(
            tasks[0].is_block_capable(),
            *expect,
            "kind={kind:?} status={status:?} expect_capable={expect}"
        );
    }
}

/// Acceptance: the optional Verknüpfung round-trips through JSON for EVERY variant, incl. the
/// free-floating `None` case (US 28).
#[test]
fn optional_link_variants_round_trip() {
    let links: &[Option<TaskLink>] = &[
        None,
        Some(TaskLink::Produkt),
        Some(TaskLink::Version("Rev B".to_string())),
        Some(TaskLink::Arbeitsbereich("mechanik".to_string())),
        Some(TaskLink::Artefakt("fertigung/stl/part.stl".to_string())),
    ];
    for link in links {
        let task = Task {
            id: "t".to_string(),
            title: "x".to_string(),
            kind: TaskKind::Aufgabe,
            status: TaskStatus::Offen,
            link: link.clone(),
            due: Some("2026-06-30".to_string()),
            blocks_everywhere: true,
            created_at: "ts".to_string(),
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(&back.link, link, "link variant did not round-trip: {link:?}");
        assert_eq!(back, task);
    }
}

// ---- Layer 2: persistence glue over a real temp folder --------------------------------

fn tmp() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "plm-tasks-it-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fresh product (no task file) is opt-in valid: zero tasks, no error.
#[test]
fn fresh_product_has_no_tasks() {
    let dir = tmp();
    assert!(read_tasks(&dir).is_empty());
    let _ = fs::remove_dir_all(&dir);
}

/// Creating a task persists it; reading it back round-trips title/kind/status/link/due/flag.
/// Covers all four link targets (Produkt/Version/Arbeitsbereich/Artefakt) plus free-floating.
#[test]
fn create_persists_all_link_variants() {
    let dir = tmp();
    create_task(&dir, new("frei", TaskKind::Hinweis, None)).unwrap();
    create_task(&dir, new("am Produkt", TaskKind::Aufgabe, Some(TaskLink::Produkt))).unwrap();
    create_task(&dir, new("an Version", TaskKind::Aufgabe, Some(TaskLink::Version("Rev B".to_string())))).unwrap();
    create_task(&dir, new("am Bereich", TaskKind::Aufgabe, Some(TaskLink::Arbeitsbereich("mechanik".to_string())))).unwrap();
    create_task(&dir, new("am Artefakt", TaskKind::Aufgabe, Some(TaskLink::Artefakt("a/b.f3d".to_string())))).unwrap();

    let tasks = read_tasks(&dir);
    assert_eq!(tasks.len(), 5);
    assert!(tasks.iter().all(|t| t.status == TaskStatus::Offen));
    assert_eq!(tasks[0].link, None);
    assert_eq!(tasks[1].link, Some(TaskLink::Produkt));
    assert_eq!(tasks[2].link, Some(TaskLink::Version("Rev B".to_string())));
    assert_eq!(tasks[3].link, Some(TaskLink::Arbeitsbereich("mechanik".to_string())));
    assert_eq!(tasks[4].link, Some(TaskLink::Artefakt("a/b.f3d".to_string())));
    // Aufgabe vs. Hinweis: only the open Aufgaben are block-capable.
    assert_eq!(tasks.iter().filter(|t| t.is_block_capable()).count(), 4);
    let _ = fs::remove_dir_all(&dir);
}

/// Editing, status changes and deletion all persist across reads.
#[test]
fn edit_status_and_delete_persist() {
    let dir = tmp();
    create_task(&dir, new("alt", TaskKind::Hinweis, Some(TaskLink::Produkt))).unwrap();
    let id = read_tasks(&dir)[0].id.clone();

    // promote the Hinweis to an Aufgabe, retitle it, clear the link, set a Fälligkeit
    update_task(
        &dir,
        &id,
        TaskEdit {
            title: Some("neu".to_string()),
            kind: Some(TaskKind::Aufgabe),
            link: Some(None),
            due: Some(Some("2026-07-01".to_string())),
            ..Default::default()
        },
    )
    .unwrap();
    let t = read_tasks(&dir).into_iter().next().unwrap();
    assert_eq!(t.title, "neu");
    assert_eq!(t.kind, TaskKind::Aufgabe);
    assert_eq!(t.link, None);
    assert_eq!(t.due.as_deref(), Some("2026-07-01"));
    assert!(t.is_block_capable(), "open Aufgabe is block-capable");

    // erledigen removes the block-capability
    set_task_status(&dir, &id, TaskStatus::Erledigt).unwrap();
    assert!(!read_tasks(&dir)[0].is_block_capable());

    // delete clears it out
    delete_task(&dir, &id).unwrap();
    assert!(read_tasks(&dir).is_empty());
    let _ = fs::remove_dir_all(&dir);
}
