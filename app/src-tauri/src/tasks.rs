//! Aufgaben & Hinweise — the pure, total, deterministic core (Issue #40, PRD US 27–30).
//!
//! **Aufgaben** (verpflichtend, *können* blockieren) and **Hinweise** (blockieren nie) are
//! first-class objects in a product. The two are separated **purely by Blockier-Fähigkeit**,
//! not by importance: a [`TaskKind::Aufgabe`] *can* block (the actual block decision is a later
//! slice — Issue #49 / the Freigabe-Gate), a [`TaskKind::Hinweis`] never blocks. The minimal
//! model is **Titel / Status / Typ / Verknüpfung / Fälligkeit** plus a context-independent
//! **„blockiert überall"** opt-out flag (US 30) — *no Priorität, kein Kanban-Zwang* (US 28).
//!
//! As with `edges.rs` over `edgestore.rs` and `graph.rs` over `graphread.rs`, this module is a
//! **pure function layer**: task list in → task list out, **no I/O, no clock**. The caller
//! supplies the id and timestamp. The side-effecting persistence glue lives in
//! [`crate::taskstore`]; everything testable lives here and is exercised by `#[cfg(test)]`
//! table tests (round-tripping the optional-link variants, kind/block-capability, status edits).

use serde::{Deserialize, Serialize};

/// What an item *can* do at a checkpoint. The **only** thing that distinguishes the two kinds
/// (US 27): an [`Aufgabe`](TaskKind::Aufgabe) is block-*capable*, a [`Hinweis`](TaskKind::Hinweis)
/// never blocks. The block *decision* itself is out of scope here (Issue #49); this core only
/// records the capability so the Freigabe-Gate can later read it.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskKind {
    /// Verpflichtend — *kann* blockieren (am Freigabe-Revision, abhängig vom Kontext, E42).
    Aufgabe,
    /// Blockiert nie — eine reine Notiz/Erinnerung.
    Hinweis,
}

/// The lifecycle status of a task. Deliberately tiny (no Kanban columns, US 28): an item is
/// open until it is either done or dropped. Only `Offen` items can ever block.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    /// Noch offen — the only status that can carry a block.
    Offen,
    /// Erledigt.
    Erledigt,
    /// Verworfen (bewusst herabgestuft/gestrichen).
    Verworfen,
}

/// The optional Verknüpfung (US 28): an item can hang off the Produkt, a Version, an
/// Arbeitsbereich, or a single Artefakt — or off nothing at all (a free-floating item is
/// valid; the list stays schlank). Tagged so the on-disk JSON and the frontend mirror stay
/// honest and round-trip exactly.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", content = "ref", rename_all = "kebab-case")]
pub enum TaskLink {
    /// The product as a whole.
    Produkt,
    /// A specific Version/Revision, by its human label or Stand id.
    Version(String),
    /// An Arbeitsbereich, by its product-relative path.
    Arbeitsbereich(String),
    /// A single Artefakt, by its product-relative path.
    Artefakt(String),
}

/// One Aufgabe or Hinweis. The minimal model (US 28): `title` / `status` / `kind` / `link` /
/// `due` + the `blocks_everywhere` opt-out (US 30). `id` is a stable opaque key the store
/// assigns; `created_at` is an injected timestamp (the core never reads the clock).
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Task {
    /// Stable opaque id (the store assigns it; the UI keys rows on it).
    pub id: String,
    /// Titel — the one piece of free human text.
    pub title: String,
    /// Typ: distinguishes Aufgabe (block-capable) from Hinweis (never blocks) — and *only* that.
    pub kind: TaskKind,
    /// Status: offen / erledigt / verworfen.
    pub status: TaskStatus,
    /// Optional Verknüpfung to Produkt/Version/Arbeitsbereich/Artefakt. `None` = free-floating.
    #[serde(default)]
    pub link: Option<TaskLink>,
    /// Optional Fälligkeit as a date string `YYYY-MM-DD` (lexicographically orderable). `None` =
    /// kein Termin.
    #[serde(default)]
    pub due: Option<String>,
    /// „blockiert überall" opt-out (US 30): when set on an Aufgabe, it is meant to block
    /// **kontextunabhängig**. Ignored for a Hinweis (a Hinweis never blocks). The block decision
    /// itself is Issue #49; this is only the flag.
    #[serde(default)]
    pub blocks_everywhere: bool,
    /// Injected creation timestamp `YYYY-MM-DDTHH:MM:SSZ` (the core never reads a clock).
    pub created_at: String,
}

impl Task {
    /// Whether this item is **block-capable** at all: an open Aufgabe. A Hinweis is never
    /// block-capable; a done/dropped Aufgabe carries no block. This is the *capability* only —
    /// whether it *actually* blocks a given checkpoint is decided later (Issue #49). Pure.
    pub fn is_block_capable(&self) -> bool {
        self.kind == TaskKind::Aufgabe && self.status == TaskStatus::Offen
    }
}

/// The fields a caller supplies to create a task — everything except the store-assigned `id`
/// and the injected `created_at`. Keeps `add_task` total and the call sites honest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    pub title: String,
    pub kind: TaskKind,
    pub link: Option<TaskLink>,
    pub due: Option<String>,
    pub blocks_everywhere: bool,
}

/// The fields an edit may change. `None` leaves a field untouched; `Some` replaces it. A title
/// edit is ignored if it would blank the title (a task always keeps a Titel). Pure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskEdit {
    pub title: Option<String>,
    pub kind: Option<TaskKind>,
    pub status: Option<TaskStatus>,
    /// Outer `Some` means "change the link"; the inner `Option` is the new value (incl. clearing
    /// to `None`). Outer `None` leaves the existing link untouched.
    pub link: Option<Option<TaskLink>>,
    /// Same nested-option convention as `link`, for the Fälligkeit.
    pub due: Option<Option<String>>,
    pub blocks_everywhere: Option<bool>,
}

/// Append a new task to `tasks`, returning the new list. Pure: the `id` and `created_at` are
/// supplied by the (side-effecting) caller, so this stays deterministic and clock-free. A
/// blank/whitespace title is rejected by returning the list unchanged (the UI should never
/// offer it, and the core never fabricates a titleless task).
pub fn add_task(
    mut tasks: Vec<Task>,
    id: impl Into<String>,
    created_at: impl Into<String>,
    new: NewTask,
) -> Vec<Task> {
    if new.title.trim().is_empty() {
        return tasks;
    }
    tasks.push(Task {
        id: id.into(),
        title: new.title,
        kind: new.kind,
        status: TaskStatus::Offen,
        link: new.link,
        due: new.due,
        blocks_everywhere: new.blocks_everywhere,
        created_at: created_at.into(),
    });
    tasks
}

/// Apply an edit to the task with `id`, returning the new list. Absent id ⇒ no-op (tolerant).
/// A title edit that would blank the title is ignored (the task keeps its Titel). Pure.
pub fn edit_task(mut tasks: Vec<Task>, id: &str, edit: TaskEdit) -> Vec<Task> {
    if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
        if let Some(title) = edit.title {
            if !title.trim().is_empty() {
                t.title = title;
            }
        }
        if let Some(kind) = edit.kind {
            t.kind = kind;
        }
        if let Some(status) = edit.status {
            t.status = status;
        }
        if let Some(link) = edit.link {
            t.link = link;
        }
        if let Some(due) = edit.due {
            t.due = due;
        }
        if let Some(b) = edit.blocks_everywhere {
            t.blocks_everywhere = b;
        }
    }
    tasks
}

/// Set just the status of the task with `id` (the common gesture: erledigen / verwerfen /
/// wieder öffnen). Absent id ⇒ no-op. Pure.
pub fn set_status(tasks: Vec<Task>, id: &str, status: TaskStatus) -> Vec<Task> {
    edit_task(
        tasks,
        id,
        TaskEdit {
            status: Some(status),
            ..Default::default()
        },
    )
}

/// Remove the task with `id`, returning the new list. Removing an absent id is a no-op. Pure.
pub fn remove_task(mut tasks: Vec<Task>, id: &str) -> Vec<Task> {
    tasks.retain(|t| t.id != id);
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(title: &str, kind: TaskKind) -> NewTask {
        NewTask {
            title: title.to_string(),
            kind,
            link: None,
            due: None,
            blocks_everywhere: false,
        }
    }

    /// A created task starts `Offen`, keeps the supplied id/timestamp, and carries its kind.
    #[test]
    fn add_task_starts_open_and_keeps_id() {
        let tasks = add_task(Vec::new(), "t1", "2026-05-31T10:00:00Z", new("Gehäuse prüfen", TaskKind::Aufgabe));
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "t1");
        assert_eq!(tasks[0].title, "Gehäuse prüfen");
        assert_eq!(tasks[0].kind, TaskKind::Aufgabe);
        assert_eq!(tasks[0].status, TaskStatus::Offen);
        assert_eq!(tasks[0].created_at, "2026-05-31T10:00:00Z");
        assert!(tasks[0].link.is_none());
        assert!(tasks[0].due.is_none());
        assert!(!tasks[0].blocks_everywhere);
    }

    /// A blank/whitespace title is refused — the core never fabricates a titleless task.
    #[test]
    fn add_task_refuses_blank_title() {
        let tasks = add_task(Vec::new(), "t1", "ts", new("   ", TaskKind::Hinweis));
        assert!(tasks.is_empty());
    }

    /// Block-CAPABILITY is purely (kind == Aufgabe && status == Offen). This is the heart of
    /// US 27: Aufgabe vs. Hinweis differ ONLY by Blockier-Fähigkeit. Table over both axes.
    #[test]
    fn block_capability_is_open_aufgabe_only() {
        // (kind, status, expect_capable)
        let cases: &[(TaskKind, TaskStatus, bool)] = &[
            (TaskKind::Aufgabe, TaskStatus::Offen, true),
            (TaskKind::Aufgabe, TaskStatus::Erledigt, false),
            (TaskKind::Aufgabe, TaskStatus::Verworfen, false),
            // A Hinweis NEVER blocks, regardless of status.
            (TaskKind::Hinweis, TaskStatus::Offen, false),
            (TaskKind::Hinweis, TaskStatus::Erledigt, false),
            (TaskKind::Hinweis, TaskStatus::Verworfen, false),
        ];
        for (kind, status, expect) in cases {
            let mut tasks = add_task(Vec::new(), "t", "ts", new("x", *kind));
            tasks = set_status(tasks, "t", *status);
            assert_eq!(
                tasks[0].is_block_capable(),
                *expect,
                "kind={kind:?} status={status:?} expect_capable={expect}"
            );
        }
    }

    /// The optional Verknüpfung round-trips through JSON for every variant — incl. the
    /// free-floating (`None`) case. Acceptance: optional-link variants are covered.
    #[test]
    fn link_variants_round_trip_through_json() {
        let links: &[Option<TaskLink>] = &[
            None,
            Some(TaskLink::Produkt),
            Some(TaskLink::Version("Rev B".to_string())),
            Some(TaskLink::Arbeitsbereich("mechanik/gehaeuse".to_string())),
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
            assert_eq!(back, task, "link variant did not round-trip: {link:?}");
        }
    }

    /// Editing replaces only the supplied fields; the nested-option convention lets a link/due
    /// be cleared to `None` distinctly from being left untouched.
    #[test]
    fn edit_task_changes_only_supplied_fields() {
        let mut tasks = add_task(
            Vec::new(),
            "t",
            "ts",
            NewTask {
                title: "alt".to_string(),
                kind: TaskKind::Hinweis,
                link: Some(TaskLink::Produkt),
                due: Some("2026-06-30".to_string()),
                blocks_everywhere: false,
            },
        );
        // change title + kind + clear the link, leave due untouched
        tasks = edit_task(
            tasks,
            "t",
            TaskEdit {
                title: Some("neu".to_string()),
                kind: Some(TaskKind::Aufgabe),
                link: Some(None), // explicit clear
                ..Default::default()
            },
        );
        let t = &tasks[0];
        assert_eq!(t.title, "neu");
        assert_eq!(t.kind, TaskKind::Aufgabe);
        assert!(t.link.is_none(), "link explicitly cleared");
        assert_eq!(t.due.as_deref(), Some("2026-06-30"), "due left untouched");
    }

    /// A blank title edit is ignored — a task always keeps a Titel.
    #[test]
    fn edit_task_ignores_blank_title() {
        let mut tasks = add_task(Vec::new(), "t", "ts", new("behalten", TaskKind::Aufgabe));
        tasks = edit_task(tasks, "t", TaskEdit { title: Some("  ".to_string()), ..Default::default() });
        assert_eq!(tasks[0].title, "behalten");
    }

    /// Edit/set-status/remove on an absent id are all tolerant no-ops.
    #[test]
    fn mutations_on_absent_id_are_no_ops() {
        let tasks = add_task(Vec::new(), "t", "ts", new("x", TaskKind::Aufgabe));
        assert_eq!(edit_task(tasks.clone(), "nope", TaskEdit { status: Some(TaskStatus::Erledigt), ..Default::default() }), tasks);
        assert_eq!(set_status(tasks.clone(), "nope", TaskStatus::Verworfen), tasks);
        assert_eq!(remove_task(tasks.clone(), "nope"), tasks);
    }

    /// set_status flips the lifecycle and (for an Aufgabe) the block-capability with it.
    #[test]
    fn set_status_flips_lifecycle_and_capability() {
        let mut tasks = add_task(Vec::new(), "t", "ts", new("x", TaskKind::Aufgabe));
        assert!(tasks[0].is_block_capable());
        tasks = set_status(tasks, "t", TaskStatus::Erledigt);
        assert_eq!(tasks[0].status, TaskStatus::Erledigt);
        assert!(!tasks[0].is_block_capable());
        // re-opening restores the capability
        tasks = set_status(tasks, "t", TaskStatus::Offen);
        assert!(tasks[0].is_block_capable());
    }

    #[test]
    fn remove_task_is_tolerant_and_removes_by_id() {
        let mut tasks = add_task(Vec::new(), "a", "ts", new("x", TaskKind::Aufgabe));
        tasks = add_task(tasks, "b", "ts", new("y", TaskKind::Hinweis));
        tasks = remove_task(tasks, "a");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "b");
    }
}
