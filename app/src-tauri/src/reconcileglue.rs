//! Side-effecting glue for the **Reconcile-beim-Öffnen** (Issue #129, E49a).
//!
//! Everything in [`crate::reconciler`] is a pure decision; this module is the thin, isolated layer
//! that (a) **reads the real observed state** of the three truth-places on open — disk (Inhalt), git
//! (Verlauf) and the server-locks (flüchtige Koordination) — (b) loads the **last-seen** `_plm`
//! memory, (c) lets the pure Reconciler judge the divergence, and (d) **carries out** the result: a
//! silent catch-up **re-seeds the `_plm` memory** (so the next open starts from the world as it
//! really is now) with no prompt; a contested ownership hands the domain-language question back to
//! the UI and leaves the memory untouched until the user resolves it.
//!
//! Mirrors the house pattern (`syncglue.rs`, `lockglue.rs`): the decision never lives here — it
//! lives in the testable core; this glue only reads the world and obeys. It reuses the **same** lock
//! snapshot the Status Reader reads ([`crate::lockglue::snapshot`]) so there is no second source of
//! truth (E37), and the **same** owner-identity split ([`crate::lockglue::owner_is_me`]) so own vs.
//! foreign locks are decided in exactly one place.
//!
//! The reconcile runs **once per open** (the glue caller is [`crate::open`] / the `open_product`
//! command), before the daily stiller Sync's idle ticks begin. The two are distinct (E49): the
//! Reconciler catches the tool up to *observed* reality; the stiller Sync ([`crate::syncglue`])
//! integrates a genuinely *diverged remote* — different jobs over different inputs.

use crate::lockglue::{owner_is_me, snapshot};
use crate::plmstore::PlmDocument;
use crate::reconciler::{
    reconcile, ForeignHold, ObservedState, PlmMemory, ReconcileDecision,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The persisted shape of the `_plm` last-seen memory (Issue #129, E49a). Lives at
/// `_plm/zuletzt-gesehen.json` — committed, shared, ADR-0002-degrading like every other `_plm`
/// store. „nur das, was git nicht ohnehin weiß": the last-seen tip lets us notice an *outside* move
/// without re-deriving it from a server round-trip every open.
///
/// Serialised separately from the pure [`PlmMemory`] so the on-disk schema is the glue's concern,
/// and the pure core stays free of serde derives it does not need. The fields mirror [`PlmMemory`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LastSeen {
    /// The git history tip (`HEAD`) the tool last saw.
    #[serde(default)]
    last_head: String,
    /// Whether the worktree was clean when the tool last looked.
    #[serde(default)]
    was_clean: bool,
    /// The artifacts the tool last knew **we** held a server-lock on.
    #[serde(default)]
    own_locks: Vec<String>,
}

impl LastSeen {
    /// Lift the persisted memory into the pure [`PlmMemory`] the Reconciler reads.
    fn into_memory(self) -> PlmMemory {
        PlmMemory {
            last_head: self.last_head,
            was_clean: self.was_clean,
            own_locks: self.own_locks,
        }
    }

    /// Snapshot the real observed state back down into the persisted memory — what we now know to be
    /// true, ready to be the next open's last-seen.
    fn from_observed(observed: &ObservedState) -> Self {
        LastSeen {
            last_head: observed.head.clone(),
            was_clean: observed.clean,
            own_locks: observed.own_locks.clone(),
        }
    }
}

/// The `_plm` document the last-seen memory lives in (ADR 0002: missing/empty/corrupt ⇒ a default
/// empty memory, so a first open simply learns the world — never an error).
const MEMORY: PlmDocument<LastSeen> = PlmDocument::new("zuletzt-gesehen.json");

/// The product-relative path of the memory file under `_plm/`. The reconcile **re-seeds** this file
/// on every silent catch-up, so its own write must never read back as drifted *content* (Inhalt) on
/// the next open — that would make the open forever "still aufgeholt" against its own bookkeeping.
/// `observe` excludes exactly this path (and its atomic temp sibling) when judging cleanliness.
const MEMORY_REL: &str = "_plm/zuletzt-gesehen.json";
const MEMORY_REL_TMP: &str = "_plm/zuletzt-gesehen.tmp";

/// Read the **real observed state** of the three truth-places for `root` (Issue #129, E49a).
/// Side-effecting — the only place the open reads git/disk/locks for the reconcile:
///
/// - **Verlauf (git):** `HEAD`'s commit id, via `git rev-parse HEAD`.
/// - **Inhalt (disk):** whether the worktree is clean, from the Status Reader's dirty set.
/// - **Koordination (server-locks):** own vs. foreign LFS locks, split by the **same** identity the
///   Status Reader uses ([`owner_is_me`]) so there is one ownership rule.
///
/// Reuses [`crate::lockglue::snapshot`] for locks+dirty+identity so there is no second source of
/// truth (E37). Best-effort by construction: an unpublished/offline repo simply reports no locks and
/// an unreadable `HEAD` reads as empty — the pure core then treats it as "nothing to contest".
pub fn observe(root: &Path) -> std::io::Result<ObservedState> {
    let snap = snapshot(root)?;

    let head = read_head(root);
    // The Inhalt truth-place is the USER's content, not the tool's own bookkeeping: the memory file
    // we re-seed every catch-up must be excluded so the open never drifts against itself (it would
    // otherwise report "Inhalt geändert" on every open and never settle to aktuell). We re-read
    // status with a pathspec that excludes exactly that file rather than filtering the shared dirty
    // set — git collapses a never-committed `_plm/` to the bare directory `_plm/`, which a path
    // string match would miss; a pathspec exclusion is exact regardless of how git groups it.
    let clean = read_clean_excluding_memory(root);

    let mut own_locks = Vec::new();
    let mut foreign_locks = Vec::new();
    for lock in &snap.locks {
        if owner_is_me(&lock.owner, &snap.me) {
            own_locks.push(lock.path.clone());
        } else {
            foreign_locks.push(ForeignHold { path: lock.path.clone(), owner: lock.owner.clone() });
        }
    }

    Ok(ObservedState { head, clean, own_locks, foreign_locks })
}

/// Whether the worktree is clean **of user content** — `git status --porcelain` with a pathspec
/// that excludes the tool's own memory file (and its atomic temp sibling). Reads only. An
/// unreadable repo safely reads back as clean (nothing to catch up on the Inhalt place).
fn read_clean_excluding_memory(root: &Path) -> bool {
    let out = crate::gitrunner::command(root)
        .args([
            "status",
            "--porcelain",
            "--",
            ".",
            &format!(":(exclude){MEMORY_REL}"),
            &format!(":(exclude){MEMORY_REL_TMP}"),
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            crate::lockglue::parse_porcelain_paths(&String::from_utf8_lossy(&o.stdout)).is_empty()
        }
        // Could not read status (unreadable repo) → nothing to catch up; treat as clean.
        _ => true,
    }
}

/// The git history tip (`HEAD`) as it really is now. Reads only; an unborn branch / unreadable repo
/// safely reads back as empty (the pure core then has nothing to reconcile against on the Verlauf
/// place). Never touches the worktree, so it can never corrupt a file.
fn read_head(root: &Path) -> String {
    crate::gitrunner::command(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Run the **silent reconcile on open** (Issue #129, E49a): read the real observed state, let the
/// pure Reconciler judge it against the last-seen `_plm` memory, and carry out the result:
///
/// - [`ReconcileDecision::Aktuell`] → nothing drifted; the memory already matched. Nothing shown.
/// - [`ReconcileDecision::StillAufgeholt`] → work happened outside but is silently resolvable:
///   **re-seed the `_plm` memory** to the observed reality with **no prompt** (E49: the tool
///   "silently catches up"). The next open starts from the world as it really is.
/// - [`ReconcileDecision::Abgleichfrage`] → a contested ownership the tool cannot decide: hand the
///   domain-language question back to the UI (the single orange-frame moment) and **leave the memory
///   untouched** — the contest is unresolved until the user answers, so we must not silently record
///   it as caught-up.
///
/// The decision is never made here — only obeyed. Returns the decision in the tool's vocabulary.
pub fn reconcile_on_open(root: &Path) -> std::io::Result<ReconcileDecision> {
    let memory = MEMORY.read(root).into_memory();
    let observed = observe(root)?;

    let decision = reconcile(&memory, &observed);

    // Catch up silently: re-seed the memory so the next open compares against today's reality. A
    // contested ownership (to-report) is left un-recorded — it is not resolved yet.
    if decision.is_silent() {
        // Best-effort persist: a failed write must never turn a silent catch-up into a loud error;
        // the worst case is the next open re-detects the same already-caught-up drift, harmlessly.
        let _ = MEMORY.write(root, &LastSeen::from_observed(&observed));
    }

    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconciler::ReconcileDecision;
    use std::path::PathBuf;
    use std::process::Command;

    /// A throwaway temp dir for one glue test.
    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-reconcileglue-it-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// Stand up a minimal real git repo with one commit, so `observe` reads a real `HEAD`.
    fn init_repo(root: &Path) -> String {
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.email", "t@example.com"]);
        git(root, &["config", "user.name", "Tester"]);
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "erster Stand"]);
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// AC (glue): on open the real state is READ — `observe` reports the actual `HEAD`, a clean
    /// worktree, and (unpublished → no remote locks) no contested coordination.
    #[test]
    fn observe_reads_the_real_head_and_clean_worktree() {
        let dir = tmp();
        let head = init_repo(&dir);
        let o = observe(&dir).unwrap();
        assert_eq!(o.head, head, "reads the real git history tip");
        assert!(o.clean, "a freshly committed worktree is clean");
        assert!(o.own_locks.is_empty() && o.foreign_locks.is_empty(), "unpublished -> no locks");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC (glue): a dirty worktree is observed as not-clean (the Inhalt truth-place drifting).
    #[test]
    fn observe_sees_a_dirty_worktree() {
        let dir = tmp();
        init_repo(&dir);
        std::fs::write(dir.join("a.txt"), "changed outside").unwrap();
        let o = observe(&dir).unwrap();
        assert!(!o.clean, "an outside edit makes the worktree dirty");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC (glue): on open silent divergence is caught up WITHOUT a prompt — the first open learns the
    /// world (empty memory) and persists it; the second open then sees `aktuell` (nothing drifted).
    #[test]
    fn open_silently_catches_up_then_is_aktuell() {
        let dir = tmp();
        init_repo(&dir);

        // First open: empty `_plm` memory vs. a real observed state -> silent catch-up, no prompt.
        let first = reconcile_on_open(&dir).unwrap();
        assert!(first.is_silent(), "a first open is silent, never a question: {first:?}");
        assert!(MEMORY.path(&dir).is_file(), "the catch-up re-seeded the _plm memory");

        // Second open with nothing changed: the memory now matches reality -> aktuell.
        let second = reconcile_on_open(&dir).unwrap();
        assert_eq!(second, ReconcileDecision::Aktuell, "a settled open is aktuell: {second:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC (glue): a git history that moved OUTSIDE the tool between opens is silently caught up — the
    /// memory is re-seeded to the new tip, so the *next* open is `aktuell` again. No prompt.
    #[test]
    fn outside_history_move_is_silently_caught_up() {
        let dir = tmp();
        init_repo(&dir);
        reconcile_on_open(&dir).unwrap(); // seed the memory at the first tip

        // Simulate work outside the tool: a terminal commit advances HEAD.
        std::fs::write(dir.join("b.txt"), "outside work").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "--quiet", "-m", "außerhalb"]);

        let d = reconcile_on_open(&dir).unwrap();
        assert!(d.is_silent(), "an outside history move is caught up silently: {d:?}");
        assert!(
            matches!(d, ReconcileDecision::StillAufgeholt { .. }),
            "the catch-up is a named still-aufgeholt, not a no-op: {d:?}"
        );
        // re-seeded -> the following open is aktuell again
        assert_eq!(reconcile_on_open(&dir).unwrap(), ReconcileDecision::Aktuell);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The persisted memory round-trips through serde and the pure core (the schema seam is honest).
    #[test]
    fn last_seen_round_trips_and_lifts_into_memory() {
        let ls = LastSeen {
            last_head: "abc".into(),
            was_clean: true,
            own_locks: vec!["x.f3d".into()],
        };
        let json = serde_json::to_string(&ls).unwrap();
        let back: LastSeen = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ls);
        let m = back.into_memory();
        assert_eq!(m.last_head, "abc");
        assert!(m.was_clean);
        assert_eq!(m.own_locks, vec!["x.f3d".to_string()]);
    }
}
