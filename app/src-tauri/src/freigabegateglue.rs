//! Thin loading glue for the **Freigabe-Gate** decision (Issue #52).
//!
//! Mirrors the house split (`aufgabenblockglue.rs` over `aufgabenblock.rs`, `edgestore.rs` over
//! `edges.rs`): the decision never lives here — it lives in the pure [`crate::freigabegate`] core.
//! This layer only **collects** the product's open points from the existing stores and hands them,
//! together with the intended [`RevisionArt`], to [`decide_gate`]:
//!
//! - the **Aufgaben** snapshot ([`crate::taskstore::read_tasks`]) — the #49 core inside
//!   `decide_gate` decides which hard-block at this Art;
//! - the fired **Stale-Warnungen** ([`crate::edgestore::read_edge_view`]) — each a Warnung;
//! - the **Waisen** ([`crate::werkbank::read_werkbank`]) — each a weicher Block.
//!
//! There is no decision logic and no re-deriving the staffing here. The fehlende-Pflicht weicher
//! Block has no data model yet (it lands with the Pflicht-Artefakt slice); the core already
//! accepts it, so this glue passes an empty list until then. The personenübergreifende Warnung
//! likewise needs the last-pusher fact (a later slice) — passed as `None` for now; the core and
//! UI already carry it through when present.
//!
//! Like the rest of the gate family, the intended Revision-Art is an **input**, not something
//! this glue computes: Issue #52's UI calls this with [`RevisionArt::Freigabe`] at the moment a
//! Prototyp is toggled up to a Freigabe, to staff the open points before the tag is raised.

use crate::artstore::read_art_in;
use crate::edgestore::read_edge_view;
use crate::freigabegate::{decide_gate, GateInputs, GateVerdict};
use crate::graph::RevisionArt;
use crate::taskstore::read_tasks;
use crate::werkbank::read_werkbank;
use std::path::Path;

/// Load the product's open points (Aufgaben + Stale-Kanten + Waisen) and decide the
/// Freigabe-Gate verdict at the intended `art` (Issue #52). Side-effecting only in the reads of
/// the `_plm` stores and the worktree; the judgement is the pure [`decide_gate`] core's.
///
/// A product with no tasks, no edges and no Waisen yields a clean `Taggen` verdict. Each read is
/// best-effort: a missing store contributes zero items rather than failing the gate (the gate
/// must never lock the user out — E22).
pub fn gate_for_art(root: &Path, art: RevisionArt) -> GateVerdict {
    let tasks = read_tasks(root);
    let stale = read_edge_view(root).warnings;
    // Waisen: every Unzugeordnet-Fach's files, flattened (each is a tracked file without Etikett).
    let waisen = read_werkbank(root)
        .map(|w| {
            w.unzugeordnet
                .into_iter()
                .flat_map(|fach| fach.dateien)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let inputs = GateInputs {
        tasks,
        stale,
        waisen,
        // No Pflicht-Artefakt model yet; no last-pusher fact yet. The core/UI carry both through
        // when a later slice supplies them.
        fehlende_pflicht: Vec::new(),
        fremd_warnung: None,
    };
    decide_gate(&inputs, art)
}

/// Wie [`gate_for_art`], aber die angestrebte Art kommt aus dem **Baustein-Scope** (E51a, Issue
/// #131): sie wird aus dem **Heimat-getragenen** Art-Store für `(heimat, version)` gelesen
/// ([`read_art_in`]) statt übergeben. So staffelt das Gate die offenen Punkte nach der Strenge
/// **genau dieses Bereichs** — der HW-Entwickler kann `elektronik` freigeben, während die
/// WIP-Firmware (für dieselbe Marke noch Prototyp) ihn nicht durch ein hartes Gate blockiert. Eine
/// nie freigegebene Baustein-Revision ist Default Prototyp (lax). Sperrt nie aus (E22).
pub fn gate_for_baustein(root: &Path, heimat: &str, version: &str) -> GateVerdict {
    let art = read_art_in(root, heimat, version);
    gate_for_art(root, art)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freigabegate::{Haerte, KnopfZustand, Punktart};
    use crate::tasks::{NewTask, TaskKind};
    use crate::taskstore::create_task;
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-freigabegate-ut-{}-{}",
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

    fn aufgabe(title: &str) -> NewTask {
        NewTask {
            title: title.to_string(),
            kind: TaskKind::Aufgabe,
            link: None,
            due: None,
            blocks_everywhere: false,
        }
    }

    /// An empty product (no stores) is a clean `Taggen` verdict in any context — the gate never
    /// locks the user out of an empty product.
    #[test]
    fn empty_product_is_clean() {
        let dir = tmp();
        let v = gate_for_art(&dir, RevisionArt::Freigabe);
        assert_eq!(v.knopf, KnopfZustand::Taggen);
        assert!(v.punkte.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The glue wires the real stores to the pure rule end-to-end: an open Aufgabe in the store
    /// hard-blocks a Freigabe (button off, named) but not a Prototyp (clean Taggen). Same staffing
    /// as the core, proven through the `_plm/aufgaben.json` store.
    #[test]
    fn loads_tasks_and_staffs_the_hard_block() {
        let dir = tmp();
        create_task(&dir, aufgabe("Footprint Q3 prüfen")).unwrap();

        // streng Freigabe: the open Aufgabe is a harter Block (button off, the task is named).
        let f = gate_for_art(&dir, RevisionArt::Freigabe);
        assert_eq!(f.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert!(f.harter_block);
        assert_eq!(f.punkte.len(), 1);
        assert_eq!(f.punkte[0].haerte, Haerte::Hart);
        assert_eq!(f.punkte[0].art, Punktart::Aufgabe);
        assert_eq!(f.punkte[0].label, "Footprint Q3 prüfen");
        assert_eq!(f.hard_blocking_task_ids().len(), 1);

        // lax Prototyp: the ordinary open Aufgabe does not block → clean Taggen.
        let p = gate_for_art(&dir, RevisionArt::Prototyp);
        assert_eq!(p.knopf, KnopfZustand::Taggen);
        assert!(p.punkte.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    /// **E51a Baustein-Scope der Art** (Issue #131): das Gate staffelt nach der **Heimat-getragenen**
    /// Art. Ein freigegebener Bereich (`elektronik`) staffelt die offene Aufgabe als harten Block;
    /// ein noch reifender Bereich (`firmware`, Default Prototyp für dieselbe Marke) ist sauber —
    /// jeder Bereich reift unabhängig.
    #[test]
    fn baustein_scope_staffs_the_gate_per_heimat() {
        use crate::artstore::set_art_in;
        let dir = tmp();
        create_task(&dir, aufgabe("Footprint Q3 prüfen")).unwrap();

        // elektronik wurde freigegeben → harter Block (Knopf aus, Aufgabe benannt).
        set_art_in(&dir, "elektronik", "v1.0", RevisionArt::Freigabe).unwrap();
        let e = gate_for_baustein(&dir, "elektronik", "v1.0");
        assert_eq!(e.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert!(e.harter_block);
        assert_eq!(e.punkte.len(), 1);
        assert_eq!(e.punkte[0].haerte, Haerte::Hart);
        assert_eq!(e.punkte[0].art, Punktart::Aufgabe);

        // firmware: dieselbe Marke nie freigegeben → Default Prototyp → sauberes Taggen.
        let f = gate_for_baustein(&dir, "firmware", "v1.0");
        assert_eq!(f.knopf, KnopfZustand::Taggen);
        assert!(f.punkte.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
