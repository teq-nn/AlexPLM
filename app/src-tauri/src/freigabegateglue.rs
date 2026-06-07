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
//!
//! ## Scope = Heimat + git-diff-Ableitung (E51b, Issue #138)
//!
//! Eine **Baustein-Freigabe** scopt das Gate auf die **Heimat** dieses Bausteins: [`gate_for_baustein`]
//! übergibt dem reinen Kern einen [`Scope`] über `heimat`, sodass nur die offenen Punkte dieses
//! Bereichs gestaffelt werden (eine WIP-Firmware hält eine PCB-Freigabe nicht als Geisel). Der
//! produkt-weite [`gate_for_art`] übergibt einen **leeren** Scope (ganzes Produkt, bisheriges
//! Verhalten).
//!
//! **Welcher Baustein sich geändert hat**, wird **nicht von Hand** zugewiesen, sondern aus dem
//! **git-Diff der Heimat-Ordner** abgeleitet ([`changed_heimaten`]): aus den Kandidaten-Heimaten
//! des Produkt-Stacks bleiben genau die, in deren Ordner der Diff Pfade berührt. Das Einsortieren
//! nutzt dieselbe Segmentregel wie der Kern ([`crate::freigabegate::within_heimat`]).

use crate::artstore::read_art_in;
use crate::edgestore::read_edge_view;
use crate::freigabegate::{decide_gate, GateInputs, GateVerdict, Scope};
use crate::graph::RevisionArt;
use crate::syncglue::parse_diff_names;
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
    // Produkt-weiter Aufruf (E47-Push, der Versionsbalken): leerer Scope = ganzes Produkt im Blick.
    decide_gate(&inputs, art, &Scope::default())
}

/// Wie [`gate_for_art`], aber die angestrebte Art **und** der Scope kommen aus dem
/// **Baustein-Scope** (E51a/E51b, Issues #131/#138): die Art wird aus dem **Heimat-getragenen**
/// Art-Store für `(heimat, version)` gelesen ([`read_art_in`]), und das Gate **filtert** die offenen
/// Punkte zusätzlich auf die **Heimat** dieses Bausteins. So staffelt es nur die Punkte **genau
/// dieses Bereichs** — der HW-Entwickler kann `elektronik` freigeben, während die WIP-Firmware
/// (eigener Bereich, eigene noch reifende Revision) ihn weder durch ein hartes Gate noch durch ihre
/// offenen Punkte als Geisel hält. Eine nie freigegebene Baustein-Revision ist Default Prototyp
/// (lax). Sperrt nie aus (E22).
pub fn gate_for_baustein(root: &Path, heimat: &str, version: &str) -> GateVerdict {
    let art = read_art_in(root, heimat, version);
    let tasks = read_tasks(root);
    let stale = read_edge_view(root).warnings;
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
        fehlende_pflicht: Vec::new(),
        fremd_warnung: None,
    };
    decide_gate(&inputs, art, &Scope::heimat(heimat))
}

/// Aus dem **git-Diff der Heimat-Ordner** ableiten, **welche** Bausteine sich geändert haben (E51b,
/// Issue #138) — nicht von Hand zugewiesen. Gegeben die Kandidaten-Heimat-Pfade (die Heimaten der
/// Produkt-Stack-Bausteine) liefert die Funktion genau die Teilmenge, in deren Ordner der Diff
/// gegen `base` Pfade berührt. Eine Baustein-Freigabe scopt dann genau auf diese Heimat(en).
///
/// `base` ist die git-Referenz, gegen die verglichen wird (z.B. der letzte Freigabe-Tag oder
/// `HEAD`); ein leerer `base` vergleicht den **Arbeitsbaum gegen `HEAD`** (uncommitted + staged),
/// der Stand kurz vor dem Setzen des Freigabe-Tags. Seiteneffekt nur im git-Diff; das Einsortieren
/// ist die reine [`crate::freigabegate::within_heimat`]-Segmentregel — dieselbe wie im Kern.
///
/// Treu zur Degradations-Invariante (E22): kein Repo / Diff schlägt fehl ⇒ leere Liste (kein
/// Bereich „geändert"), nie ein Fehler.
pub fn changed_heimaten(root: &Path, candidate_heimaten: &[String], base: &str) -> Vec<String> {
    let changed = diff_names(root, base);
    candidate_heimaten
        .iter()
        .filter(|h| {
            changed
                .iter()
                .any(|p| crate::freigabegate::within_heimat(p, h))
        })
        .cloned()
        .collect()
}

/// Die produkt-relativen Pfade, die der git-Diff gegen `base` berührt. `base` leer ⇒ Arbeitsbaum
/// gegen `HEAD` (`git diff --name-only`); sonst gegen die genannte Referenz. Spiegelt den
/// `syncglue`-Lesepfad (`git diff --name-only` + [`parse_diff_names`]). Best-effort: kein
/// Repo / Fehlschlag ⇒ leer (E22).
fn diff_names(root: &Path, base: &str) -> Vec<String> {
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["diff", "--name-only"]);
    if !base.trim().is_empty() {
        cmd.arg(base);
    }
    match cmd.output() {
        Ok(out) if out.status.success() => {
            parse_diff_names(&String::from_utf8_lossy(&out.stdout))
        }
        _ => Vec::new(),
    }
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

    /// Eine offene Aufgabe, an einen Arbeitsbereich (Heimat-Pfad) gehängt — der Scope-Bezug (E51b).
    fn aufgabe_at(title: &str, arbeitsbereich: &str) -> NewTask {
        let mut t = aufgabe(title);
        t.link = Some(crate::tasks::TaskLink::Arbeitsbereich(arbeitsbereich.to_string()));
        t
    }

    /// Ein git-Befehl im Repo (Test-Helfer).
    fn git(root: &Path, args: &[&str]) {
        crate::gitrunner::command(root)
            .args(args)
            .output()
            .unwrap();
    }

    /// Ein leeres git-Repo mit erstem Commit anlegen, damit `git diff` eine Basis hat.
    fn git_init(root: &Path) {
        git(root, &["init", "-q"]);
        git(root, &["config", "user.email", "t@t"]);
        git(root, &["config", "user.name", "t"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        fs::write(root.join("README"), "x").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "init"]);
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

    /// **E51b Heimat-Scope durch die Stores** (Issue #138): eine **firmware**-verknüpfte offene
    /// Aufgabe hält eine **elektronik**-Freigabe **nicht** als Geisel — sie fällt aus dem Scope, auch
    /// wenn beide Bereiche freigegeben sind. Aus Sicht der firmware-Freigabe blockiert dieselbe
    /// Aufgabe sehr wohl. Jeder Bereich reift für sich.
    #[test]
    fn baustein_gate_scopes_open_points_to_its_heimat() {
        use crate::artstore::set_art_in;
        let dir = tmp();
        // Eine Aufgabe, die fest zur firmware gehört.
        create_task(&dir, aufgabe_at("Bootloader fixen", "firmware")).unwrap();

        // Beide Bereiche sind freigegeben (streng) …
        set_art_in(&dir, "elektronik", "v1.0", RevisionArt::Freigabe).unwrap();
        set_art_in(&dir, "firmware", "v1.0", RevisionArt::Freigabe).unwrap();

        // … aber die elektronik-Freigabe scopt auf elektronik → die firmware-Aufgabe fällt heraus.
        let e = gate_for_baustein(&dir, "elektronik", "v1.0");
        assert_eq!(e.knopf, KnopfZustand::Taggen, "firmware-WIP hält elektronik nicht");
        assert!(e.punkte.is_empty());

        // Aus Sicht der firmware-Freigabe ist genau diese Aufgabe ein harter Block.
        let f = gate_for_baustein(&dir, "firmware", "v1.0");
        assert_eq!(f.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert_eq!(f.hard_blocking_task_ids(), vec![f.punkte[0].ref_id.clone()]);

        let _ = fs::remove_dir_all(&dir);
    }

    /// **E51b „welcher Baustein änderte sich" aus dem git-Diff** (Issue #138): zwischen dem letzten
    /// Stand (`base`) und dem aktuellen HEAD wurde **nur** unter `elektronik/` committet, also liefert
    /// [`changed_heimaten`] aus den Kandidaten-Heimaten genau `elektronik` — nicht von Hand
    /// zugewiesen, sondern aus dem Diff der Heimat-Ordner abgeleitet (`git diff --name-only base`).
    /// Ein Geschwisterordner mit gemeinsamem Präfix (`elektronik-alt/`) zählt **nicht** dazu
    /// (Segmentgrenze).
    #[test]
    fn changed_heimaten_derives_the_changed_baustein_from_the_git_diff() {
        let dir = tmp();
        git_init(&dir);

        let candidates = vec![
            "elektronik".to_string(),
            "firmware".to_string(),
            "mechanik".to_string(),
        ];
        // Der letzte Freigabe-Stand = der erste Commit. Gegen ihn noch keine Änderung.
        assert!(changed_heimaten(&dir, &candidates, "HEAD").is_empty());

        // Einen neuen Stand committen, der NUR elektronik/ (und einen Präfix-Geschwisterordner)
        // berührt — so läge er nach einem Werkbank-Autocommit vor dem Setzen des Freigabe-Tags.
        fs::create_dir_all(dir.join("elektronik")).unwrap();
        fs::write(dir.join("elektronik/board.kicad_pcb"), "rev b").unwrap();
        fs::create_dir_all(dir.join("elektronik-alt")).unwrap();
        fs::write(dir.join("elektronik-alt/x"), "y").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "rev b"]);

        // Gegen den vorherigen Stand (HEAD~1) ist genau elektronik berührt — nicht elektronik-alt.
        let changed = changed_heimaten(&dir, &candidates, "HEAD~1");
        assert_eq!(
            changed,
            vec!["elektronik".to_string()],
            "nur die elektronik-Heimat ist im Diff berührt, elektronik-alt passt nicht hinein"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// **Degradation (E22)**: ohne git-Repo schlägt der Diff fehl ⇒ leere Liste, nie ein Fehler.
    #[test]
    fn changed_heimaten_degrades_without_a_repo() {
        let dir = tmp();
        let candidates = vec!["elektronik".to_string()];
        assert!(changed_heimaten(&dir, &candidates, "HEAD").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
