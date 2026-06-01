//! The **Aufgaben-Block** decision core — does a set of open Aufgaben block a checkpoint?
//! (Issue #49, E42, PRD US 30/§49.)
//!
//! Following the house pattern (`syncdecider.rs`, `warden.rs`, `classifier.rs`): one **pure,
//! total, deterministic** function over a plain snapshot. It knows **no** git internals, no
//! clock, no I/O — the loading glue (read the product's tasks + the Revision-Art) lives in
//! [`crate::aufgabenblockglue`]; this module only decides. Snapshot in, exactly one
//! [`BlockDecision`] out.
//!
//! The load-bearing rule of E42 is that the **Strenge lives on the Revision-Art**
//! ([`RevisionArt`], from Issue #41), **not** on a Branch-Typ:
//!
//! - **Prototyp** (lax) → an open Aufgabe **never** blocks. Tagging a Prototyp stays frictionless.
//! - **Freigabe** (streng) → an open, block-capable Aufgabe is a **harter Block**: a Freigabe may
//!   not be reached while it is open.
//! - A task's **„blockiert überall"** opt-out ([`Task::blocks_everywhere`], US 30) makes it block
//!   **kontextunabhängig** — it blocks even for a Prototyp, regardless of the Revision-Art.
//! - A **Hinweis** never blocks, in any context (it is not block-capable; US 27).
//!
//! The signature shape is `(Aufgaben-Menge × Revision-Art {Prototyp|Freigabe} × Checkpoint)
//! → blockiert / nicht blockiert`. The pure core takes the task snapshot and the intended Art
//! and returns whether the checkpoint is blocked and **by which tasks** — so Issue #52's
//! Freigabe-Gate UI can name the offenders without re-deciding anything.

use crate::graph::RevisionArt;
use crate::tasks::Task;
use serde::Serialize;

/// Whether a **single** open, block-capable Aufgabe blocks at the given Revision-Art. Pure.
///
/// The two axes that make an Aufgabe block (E42):
/// - the **context**: a [`RevisionArt::Freigabe`] is streng (every open Aufgabe blocks it); a
///   [`RevisionArt::Prototyp`] is lax (open Aufgaben do not block it);
/// - the task's own **„blockiert überall"** opt-out: when set, it blocks **kontextunabhängig** —
///   even at a Prototyp.
///
/// A task that is not block-capable (a Hinweis, or a done/dropped Aufgabe) never blocks; the
/// caller filters those out before reaching here, but the rule holds for any input.
fn task_blocks_at(task: &Task, art: RevisionArt) -> bool {
    if !task.is_block_capable() {
        // A Hinweis — or a done/dropped Aufgabe — never blocks, in any context (US 27).
        return false;
    }
    // An open Aufgabe blocks if EITHER the context is streng (Freigabe) OR it opted to block
    // everywhere (US 30) — the latter ignores the Revision-Art entirely.
    task.blocks_everywhere || matches!(art, RevisionArt::Freigabe)
}

/// The single decision the Aufgaben-Block core returns for a checkpoint. Exactly one; total. It
/// carries the **ids of the blocking tasks** so the UI can name them („Freigabe blockiert durch
/// N offene Aufgaben") without re-running the rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BlockDecision {
    /// Whether the checkpoint is blocked at all. `true` iff [`BlockDecision::blocking_task_ids`]
    /// is non-empty (kept as an explicit, serialized field so the UI reads one honest flag).
    pub blocked: bool,
    /// The ids of the open Aufgaben that block this checkpoint, in input order. Empty ⇔ not
    /// blocked. Exactly the tasks for which [`task_blocks_at`] held.
    pub blocking_task_ids: Vec<String>,
}

impl BlockDecision {
    /// Whether the checkpoint is blocked (a harter Block). True ⇔ at least one blocking task.
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// How many open Aufgaben block this checkpoint. Zero ⇔ not blocked.
    pub fn blocking_count(&self) -> usize {
        self.blocking_task_ids.len()
    }
}

/// The **Aufgaben-Block decision**: given the product's task snapshot and the intended
/// Revision-Art for a checkpoint, decide whether the checkpoint is blocked and by which
/// open Aufgaben. **Pure, total, deterministic** — no I/O, no clock.
///
/// The rule (E42), in one line: **a Freigabe is blocked by any open Aufgabe; a Prototyp only by
/// an open „blockiert überall" Aufgabe; a Hinweis never blocks.**
///
/// - **Prototyp** → blocked only by open Aufgaben that opted into „blockiert überall" (US 30).
///   With none such, a Prototyp is never blocked (lax — tagging stays frictionless).
/// - **Freigabe** → blocked by **every** open, block-capable Aufgabe (streng — harter Block).
/// - A **Hinweis** is never block-capable, so it never appears among the blockers, in any context.
///
/// Returns a [`BlockDecision`] naming the blocking task ids in input order (so the UI can list
/// them); `blocked` is `true` iff that list is non-empty.
pub fn decide_block(tasks: &[Task], art: RevisionArt) -> BlockDecision {
    let blocking_task_ids: Vec<String> = tasks
        .iter()
        .filter(|t| task_blocks_at(t, art))
        .map(|t| t.id.clone())
        .collect();
    BlockDecision {
        blocked: !blocking_task_ids.is_empty(),
        blocking_task_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{TaskKind, TaskStatus};

    /// Build a task with the axes the block rule reads: kind, status, the „blockiert überall"
    /// opt-out. The other fields are irrelevant to the decision (link/due/title) — fixed here.
    fn task(id: &str, kind: TaskKind, status: TaskStatus, blocks_everywhere: bool) -> Task {
        Task {
            id: id.to_string(),
            title: "x".to_string(),
            kind,
            status,
            link: None,
            due: None,
            blocks_everywhere,
            created_at: "ts".to_string(),
        }
    }

    /// The four task shapes that drive the cross-product matrix, with a stable id each:
    /// an open blocking Aufgabe, a none (done Aufgabe), a Hinweis, and a „blockiert überall"
    /// Aufgabe. Each is exercised against BOTH Revision-Arten below.
    fn open_blocking() -> Task {
        task("open-aufgabe", TaskKind::Aufgabe, TaskStatus::Offen, false)
    }
    fn done_aufgabe() -> Task {
        task("done-aufgabe", TaskKind::Aufgabe, TaskStatus::Erledigt, false)
    }
    fn open_hinweis() -> Task {
        task("hinweis", TaskKind::Hinweis, TaskStatus::Offen, false)
    }
    fn blocks_everywhere_aufgabe() -> Task {
        task("ueberall", TaskKind::Aufgabe, TaskStatus::Offen, true)
    }

    /// **The core acceptance matrix**: the full cross-product of {Prototyp, Freigabe} ×
    /// {open blocking, none/done, Hinweis, „blockiert überall"}, asserting the four rules in one
    /// table. Each row is a single task so the per-shape rule is isolated.
    #[test]
    fn cross_product_of_art_and_task_shape() {
        // (art, task, expect_blocked)
        let cases: &[(RevisionArt, Task, bool)] = &[
            // --- Prototyp (lax): never blocks, EXCEPT a „blockiert überall" Aufgabe. -----------
            (RevisionArt::Prototyp, open_blocking(), false),
            (RevisionArt::Prototyp, done_aufgabe(), false),
            (RevisionArt::Prototyp, open_hinweis(), false),
            (RevisionArt::Prototyp, blocks_everywhere_aufgabe(), true), // kontextunabhängig
            // --- Freigabe (streng): an open blocking Aufgabe is a harter Block. ----------------
            (RevisionArt::Freigabe, open_blocking(), true),
            (RevisionArt::Freigabe, done_aufgabe(), false), // not open ⇒ not block-capable
            (RevisionArt::Freigabe, open_hinweis(), false), // a Hinweis never blocks
            (RevisionArt::Freigabe, blocks_everywhere_aufgabe(), true),
        ];
        for (art, t, expect) in cases {
            let d = decide_block(std::slice::from_ref(t), *art);
            assert_eq!(
                d.is_blocked(),
                *expect,
                "art={art:?} task={t:?} expect_blocked={expect}"
            );
            // The flag and the id-list agree: blocked ⇔ at least one named blocker.
            assert_eq!(d.blocked, !d.blocking_task_ids.is_empty());
            if *expect {
                assert_eq!(d.blocking_task_ids, vec![t.id.clone()]);
            } else {
                assert!(d.blocking_task_ids.is_empty());
            }
        }
    }

    /// AC: **Prototyp blocks never** — even with a whole pile of open blocking Aufgaben, as long
    /// as none opted into „blockiert überall". Tagging a Prototyp stays frictionless (E42).
    #[test]
    fn prototyp_never_blocks_on_ordinary_open_aufgaben() {
        let tasks = vec![
            open_blocking(),
            task("a2", TaskKind::Aufgabe, TaskStatus::Offen, false),
            open_hinweis(),
            done_aufgabe(),
        ];
        let d = decide_block(&tasks, RevisionArt::Prototyp);
        assert!(!d.is_blocked(), "a Prototyp is lax: open Aufgaben do not block it");
        assert_eq!(d.blocking_count(), 0);
    }

    /// AC: **Freigabe + open blocking Aufgabe → harter Block**, and the decision names exactly the
    /// open Aufgaben (in input order) — never the Hinweise or the done/dropped tasks.
    #[test]
    fn freigabe_hard_blocks_on_open_aufgaben_and_names_them() {
        let tasks = vec![
            open_blocking(),                                              // blocks
            open_hinweis(),                                              // never
            done_aufgabe(),                                              // not open
            task("a2", TaskKind::Aufgabe, TaskStatus::Offen, false),     // blocks
            task("verworfen", TaskKind::Aufgabe, TaskStatus::Verworfen, false), // not open
        ];
        let d = decide_block(&tasks, RevisionArt::Freigabe);
        assert!(d.is_blocked(), "Freigabe + open Aufgabe is a harter Block");
        assert_eq!(
            d.blocking_task_ids,
            vec!["open-aufgabe".to_string(), "a2".to_string()],
            "names exactly the open Aufgaben, in input order"
        );
    }

    /// AC: **„blockiert überall" blocks regardless of Revision-Art** — it is the
    /// context-independent opt-out (US 30). It blocks a Prototyp just as it blocks a Freigabe.
    #[test]
    fn blocks_everywhere_blocks_in_every_context() {
        for art in [RevisionArt::Prototyp, RevisionArt::Freigabe] {
            let d = decide_block(&[blocks_everywhere_aufgabe()], art);
            assert!(d.is_blocked(), "blockiert-ueberall blocks even at {art:?}");
            assert_eq!(d.blocking_task_ids, vec!["ueberall".to_string()]);
        }
    }

    /// AC: a **Hinweis never blocks**, in any context — not even when it carries the
    /// „blockiert überall" flag (a Hinweis is not block-capable; US 27). The flag is meaningful
    /// only on an Aufgabe.
    #[test]
    fn hinweis_never_blocks_even_with_blocks_everywhere() {
        let hinweis_ueberall = task("h", TaskKind::Hinweis, TaskStatus::Offen, true);
        for art in [RevisionArt::Prototyp, RevisionArt::Freigabe] {
            let d = decide_block(std::slice::from_ref(&hinweis_ueberall), art);
            assert!(!d.is_blocked(), "a Hinweis never blocks at {art:?}, flag or not");
            assert_eq!(d.blocking_count(), 0);
        }
    }

    /// AC: the core is **total** — an empty snapshot (no tasks) never blocks, in any context, and
    /// the decision is internally consistent (flag ⇔ non-empty id list).
    #[test]
    fn empty_snapshot_never_blocks_and_decision_is_total() {
        for art in [RevisionArt::Prototyp, RevisionArt::Freigabe] {
            let d = decide_block(&[], art);
            assert!(!d.is_blocked());
            assert_eq!(d.blocked, !d.blocking_task_ids.is_empty());
            assert_eq!(d.blocking_count(), 0);
        }
    }

    /// **Determinism**: the same snapshot + Art always yields the same decision (id list and all).
    #[test]
    fn decision_is_deterministic() {
        let tasks = vec![open_blocking(), blocks_everywhere_aufgabe(), open_hinweis()];
        let a = decide_block(&tasks, RevisionArt::Freigabe);
        let b = decide_block(&tasks, RevisionArt::Freigabe);
        assert_eq!(a, b);
    }
}
