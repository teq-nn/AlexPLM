//! Auto-Lock & Status-Signale — the **Status Reader** (Issue #6).
//!
//! The tool coordinates concurrent binary edits with `git lfs lock`. Opening/editing a
//! *lockable* artifact — one the #3 [`crate::classifier`] sorts into an unmergeable bucket
//! (a binary, or a nominally-text-but-unmergeable KiCad file) — auto-acquires a lock. The
//! per-artifact LED the UI shows is **derived purely** from a snapshot of `git lfs locks`
//! plus the local worktree status: there is **no** second source of truth and **no** presence
//! service (E37 — "lies zurück statt spiegeln"). A small live panel lists *foreign* locks,
//! again read only from `git lfs locks`.
//!
//! Following the `projection.rs` / `classifier.rs` pattern, every decision lives in a pure,
//! table-testable function over plain snapshots; the side-effecting git glue (shelling out to
//! `git lfs locks` / `git lfs lock` / `git status`) is isolated in [`crate::lockglue`] so the
//! derivation never touches a real repo, clock, or process.

use crate::classifier::{classify, Bucket};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Lockable buckets — delegated to the #3 Mergeability Classifier
// ---------------------------------------------------------------------------

/// Is this artifact *lockable* — i.e. should opening/editing it auto-acquire a `git lfs lock`?
///
/// Lockable = the file is **unmergeable**: a real binary (CAD source, mesh, photo, PDF, …) or
/// a nominally-text-but-unmergeable KiCad source. This reuses the #3 classifier's three-bucket
/// decision directly ([`Bucket::is_lockable`]) so import-time marking and edit-time auto-lock
/// agree on exactly the same set of files. Pure over the filename → table-testable.
pub fn is_lockable(path: &str) -> bool {
    classify(path, None).is_lockable()
}

/// The classifier bucket a path lands in — exposed so the auto-lock glue can log/explain *why*
/// a file is lockable without re-deriving it. Pure passthrough.
pub fn bucket_of(path: &str) -> Bucket {
    classify(path, None)
}

// ---------------------------------------------------------------------------
// Snapshots: the pure inputs to the Status Reader
// ---------------------------------------------------------------------------

/// One row from `git lfs locks` (read-only). Mirrors the fields we use from the `--json` form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockInfo {
    /// Locked path, product-relative with forward slashes.
    pub path: String,
    /// Lock owner's display name.
    pub owner: String,
    /// When the lock was taken, as reported by git-lfs (ISO-8601 string, verbatim).
    pub locked_at: String,
}

/// A point-in-time read of the coordination world: every lock git-lfs reports, who *we* are
/// (to split own vs. foreign locks) and which paths the worktree currently shows as dirty.
/// This is the *whole* input to the Status Reader — no hidden state, no service (E37).
#[derive(Debug, Clone, Default)]
pub struct StatusSnapshot {
    /// All locks reported by `git lfs locks` (own + foreign).
    pub locks: Vec<LockInfo>,
    /// The current user's git-lfs display name, used to tell own locks from foreign ones.
    pub me: String,
    /// Product-relative paths the worktree reports as modified/dirty (`git status`).
    pub dirty: Vec<String>,
}

// ---------------------------------------------------------------------------
// Derived per-artifact status (the LED)
// ---------------------------------------------------------------------------

/// The derived status of one artifact — the direct translation of the lock signals into the
/// LED colours from the Stilbeschreibung (§ Status-Punkt). Nothing here is stored; it is
/// recomputed from a [`StatusSnapshot`] every time (E37).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactStatus {
    /// Free / clean, no lock anywhere → **green** LED (`--led-free`).
    Free,
    /// In progress / ruhend: locked **by us**, or just locally dirty → **grey** LED
    /// (`--led-working`). The quiet everyday state.
    InProgress,
    /// Locked **by someone else** → needs attention → **orange** LED (`--led-attention`),
    /// tooltip "gesperrt von X seit …". The single loud exception.
    LockedByOther,
}

/// What the UI needs to render one artifact's LED: the derived status plus, when foreign-
/// locked, who holds it and since when (for the "gesperrt von X seit …" tooltip).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactSignal {
    /// Product-relative artifact path the signal is for.
    pub path: String,
    pub status: ArtifactStatus,
    /// Foreign lock owner, present iff `status == LockedByOther`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    /// Foreign lock timestamp, present iff `status == LockedByOther`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
    /// Ready-to-show tooltip, "gesperrt von X seit …", present iff foreign-locked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

/// Derive one artifact's status from a snapshot. **Pure** — the heart of the Status Reader.
///
/// Precedence (loudest wins):
/// 1. a **foreign** lock on the path → [`ArtifactStatus::LockedByOther`] (orange, tooltip);
/// 2. otherwise an **own** lock, or a locally **dirty** path → [`ArtifactStatus::InProgress`]
///    (grey);
/// 3. otherwise → [`ArtifactStatus::Free`] (green).
pub fn derive_status(path: &str, snap: &StatusSnapshot) -> ArtifactSignal {
    if let Some(lock) = snap.locks.iter().find(|l| l.path == path && l.owner != snap.me) {
        return ArtifactSignal {
            path: path.to_string(),
            status: ArtifactStatus::LockedByOther,
            locked_by: Some(lock.owner.clone()),
            locked_at: Some(lock.locked_at.clone()),
            tooltip: Some(lock_tooltip(&lock.owner, &lock.locked_at)),
        };
    }

    let own_locked = snap.locks.iter().any(|l| l.path == path && l.owner == snap.me);
    let dirty = snap.dirty.iter().any(|d| d == path);
    let status = if own_locked || dirty {
        ArtifactStatus::InProgress
    } else {
        ArtifactStatus::Free
    };

    ArtifactSignal {
        path: path.to_string(),
        status,
        locked_by: None,
        locked_at: None,
        tooltip: None,
    }
}

/// Derive the status of many artifacts at once from a single snapshot.
pub fn derive_statuses(paths: &[String], snap: &StatusSnapshot) -> Vec<ArtifactSignal> {
    paths.iter().map(|p| derive_status(p, snap)).collect()
}

/// The orange-LED tooltip text: `gesperrt von X seit …`. Pure over owner + timestamp.
pub fn lock_tooltip(owner: &str, locked_at: &str) -> String {
    format!("gesperrt von {owner} seit {locked_at}")
}

/// The *foreign* locks (held by anyone but us) from a snapshot — the live "Belegte Bausteine"
/// panel. Pure projection of the same single source of truth (E37).
pub fn foreign_locks(snap: &StatusSnapshot) -> Vec<LockInfo> {
    snap.locks
        .iter()
        .filter(|l| l.owner != snap.me)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(path: &str, owner: &str, at: &str) -> LockInfo {
        LockInfo {
            path: path.into(),
            owner: owner.into(),
            locked_at: at.into(),
        }
    }

    #[test]
    fn lockable_buckets_are_binary_or_kicad_not_mergeable_text() {
        // table: filename -> is it lockable (auto-lock on edit)? — delegates to #3 classifier.
        let cases: &[(&str, bool)] = &[
            ("gehaeuse.f3d", true),
            ("halter.step", true),
            ("part.stl", true),
            ("board.kicad_pcb", true), // KiCad: nominal text but unmergeable -> lockable
            ("board.kicad_sch", true),
            ("datasheet.pdf", true),
            ("render.PNG", true),       // case-insensitive
            ("board.kicad_pro", false), // project file is not merge-hostile -> text
            ("notes.md", false),        // mergeable text -> not lockable
            ("readme.txt", false),
            ("firmware.c", false),
            ("config.yaml", false),
            ("Makefile", false),
        ];
        for (name, expected) in cases {
            assert_eq!(is_lockable(name), *expected, "name = {name}");
        }
    }

    /// The Status Reader's core table: a locks+worktree snapshot -> the expected per-artifact
    /// status (the acceptance-criteria test).
    #[test]
    fn status_reader_derives_per_artifact_status_from_snapshot() {
        let snap = StatusSnapshot {
            me: "anna".into(),
            locks: vec![
                lock("mechanik/gehaeuse/gehaeuse.f3d", "bjoern", "2026-05-30T09:15:00Z"),
                lock("elektronik/regler/regler.kicad_pcb", "anna", "2026-05-30T08:00:00Z"),
            ],
            dirty: vec!["mechanik/halter/halter.step".into()],
        };

        // table: artifact path -> expected derived status
        let cases: &[(&str, ArtifactStatus)] = &[
            // foreign lock -> orange
            ("mechanik/gehaeuse/gehaeuse.f3d", ArtifactStatus::LockedByOther),
            // own lock -> grey (in progress / ruhend)
            ("elektronik/regler/regler.kicad_pcb", ArtifactStatus::InProgress),
            // locally dirty, no lock -> grey
            ("mechanik/halter/halter.step", ArtifactStatus::InProgress),
            // nothing -> green (free)
            ("mechanik/deckel/deckel.f3d", ArtifactStatus::Free),
        ];
        for (path, expected) in cases {
            assert_eq!(
                derive_status(path, &snap).status,
                *expected,
                "path = {path}"
            );
        }
    }

    #[test]
    fn foreign_lock_carries_owner_timestamp_and_tooltip() {
        let snap = StatusSnapshot {
            me: "anna".into(),
            locks: vec![lock(
                "mechanik/gehaeuse/gehaeuse.f3d",
                "bjoern",
                "2026-05-30T09:15:00Z",
            )],
            dirty: vec![],
        };
        let sig = derive_status("mechanik/gehaeuse/gehaeuse.f3d", &snap);
        assert_eq!(sig.status, ArtifactStatus::LockedByOther);
        assert_eq!(sig.locked_by.as_deref(), Some("bjoern"));
        assert_eq!(sig.locked_at.as_deref(), Some("2026-05-30T09:15:00Z"));
        assert_eq!(
            sig.tooltip.as_deref(),
            Some("gesperrt von bjoern seit 2026-05-30T09:15:00Z")
        );
    }

    #[test]
    fn own_and_clean_artifacts_carry_no_owner_or_tooltip() {
        let snap = StatusSnapshot {
            me: "anna".into(),
            locks: vec![lock("a/x.f3d", "anna", "2026-05-30T08:00:00Z")],
            dirty: vec![],
        };
        let own = derive_status("a/x.f3d", &snap);
        assert_eq!(own.status, ArtifactStatus::InProgress);
        assert_eq!(own.locked_by, None);
        assert_eq!(own.tooltip, None);

        let free = derive_status("a/y.f3d", &snap);
        assert_eq!(free.status, ArtifactStatus::Free);
        assert_eq!(free.tooltip, None);
    }

    /// A foreign lock outranks local dirtiness on the same path: the loud orange wins.
    #[test]
    fn foreign_lock_outranks_local_dirty() {
        let snap = StatusSnapshot {
            me: "anna".into(),
            locks: vec![lock("a/x.f3d", "bjoern", "2026-05-30T09:15:00Z")],
            dirty: vec!["a/x.f3d".into()],
        };
        assert_eq!(
            derive_status("a/x.f3d", &snap).status,
            ArtifactStatus::LockedByOther
        );
    }

    #[test]
    fn foreign_locks_panel_lists_only_others_locks() {
        let snap = StatusSnapshot {
            me: "anna".into(),
            locks: vec![
                lock("a/x.f3d", "bjoern", "t1"),
                lock("b/y.step", "anna", "t2"), // mine -> excluded
                lock("c/z.kicad_pcb", "carla", "t3"),
            ],
            dirty: vec![],
        };
        let foreign = foreign_locks(&snap);
        let owners: Vec<&str> = foreign.iter().map(|l| l.owner.as_str()).collect();
        assert_eq!(owners, ["bjoern", "carla"]);
    }

    #[test]
    fn lock_tooltip_is_fixed_german_phrasing() {
        assert_eq!(
            lock_tooltip("bjoern", "2026-05-30T09:15:00Z"),
            "gesperrt von bjoern seit 2026-05-30T09:15:00Z"
        );
    }

    #[test]
    fn derive_statuses_maps_a_batch() {
        let snap = StatusSnapshot {
            me: "anna".into(),
            locks: vec![lock("a/x.f3d", "bjoern", "t1")],
            dirty: vec!["b/y.step".into()],
        };
        let paths = vec![
            "a/x.f3d".to_string(),
            "b/y.step".to_string(),
            "c/z.f3d".to_string(),
        ];
        let sigs = derive_statuses(&paths, &snap);
        assert_eq!(sigs[0].status, ArtifactStatus::LockedByOther);
        assert_eq!(sigs[1].status, ArtifactStatus::InProgress);
        assert_eq!(sigs[2].status, ArtifactStatus::Free);
    }
}
