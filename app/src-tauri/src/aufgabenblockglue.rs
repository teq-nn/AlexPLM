//! Thin loading glue for the **Aufgaben-Block** decision (Issue #49).
//!
//! Mirrors the house split (`syncglue.rs` over `syncdecider.rs`, `pushglue.rs` over `warden.rs`):
//! the decision never lives here — it lives in the pure [`crate::aufgabenblock`] core. This layer
//! only **reads** the product's task snapshot from the `_plm` store ([`crate::taskstore`]) and
//! hands it, together with the intended [`MilestoneArt`], to [`decide_block`]. No decision logic,
//! no re-deriving the rule.
//!
//! The intended Meilenstein-Art is an **input**, not something this glue computes: a checkpoint is
//! always reached *with an intent* (the user is about to release a Freigabe, or stay a Prototyp),
//! and the Strenge lives on that Art (E42). Issue #52's Freigabe-Gate will call this with
//! [`MilestoneArt::Freigabe`] before raising a tag; a Prototyp check can call it with
//! [`MilestoneArt::Prototyp`] to surface only the „blockiert überall" opt-outs.

use crate::aufgabenblock::{decide_block, BlockDecision};
use crate::graph::MilestoneArt;
use crate::taskstore::read_tasks;
use std::path::Path;

/// Load the product's Aufgaben snapshot and decide whether a checkpoint at the intended
/// `art` is blocked (Issue #49). Side-effecting only in the read of `_plm/aufgaben.json`
/// (a missing/empty store is zero tasks → never blocked); the judgement is the pure core's.
///
/// Returns the [`BlockDecision`] — `blocked` plus the ids of the blocking open Aufgaben — so the
/// UI (and Issue #52's Freigabe-Gate) can name the offenders without re-deciding anything.
pub fn block_for_art(root: &Path, art: MilestoneArt) -> BlockDecision {
    let tasks = read_tasks(root);
    decide_block(&tasks, art)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taskstore::create_task;
    use crate::tasks::{NewTask, TaskKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-aufgabenblock-ut-{}-{}",
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

    fn new(title: &str, kind: TaskKind, blocks_everywhere: bool) -> NewTask {
        NewTask {
            title: title.to_string(),
            kind,
            link: None,
            due: None,
            blocks_everywhere,
        }
    }

    /// A product with no task store is never blocked, in any context (tasks are opt-in).
    #[test]
    fn no_task_store_is_never_blocked() {
        let dir = tmp();
        assert!(!block_for_art(&dir, MilestoneArt::Prototyp).is_blocked());
        assert!(!block_for_art(&dir, MilestoneArt::Freigabe).is_blocked());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The glue wires the real store to the pure rule: an open Aufgabe blocks a Freigabe but not
    /// a Prototyp, while a „blockiert überall" Aufgabe blocks both. Same rule as the core, proven
    /// end-to-end through the `_plm/aufgaben.json` store.
    #[test]
    fn loads_tasks_and_applies_the_rule() {
        let dir = tmp();
        create_task(&dir, new("offene Aufgabe", TaskKind::Aufgabe, false)).unwrap();

        // lax Prototyp: an ordinary open Aufgabe does not block
        assert!(!block_for_art(&dir, MilestoneArt::Prototyp).is_blocked());
        // streng Freigabe: it does
        let f = block_for_art(&dir, MilestoneArt::Freigabe);
        assert!(f.is_blocked());
        assert_eq!(f.blocking_count(), 1);

        // add a „blockiert überall" Aufgabe → now a Prototyp is blocked too
        create_task(&dir, new("überall", TaskKind::Aufgabe, true)).unwrap();
        let p = block_for_art(&dir, MilestoneArt::Prototyp);
        assert!(p.is_blocked());
        assert_eq!(p.blocking_count(), 1, "only the blockiert-ueberall task blocks a Prototyp");

        let _ = fs::remove_dir_all(&dir);
    }
}
