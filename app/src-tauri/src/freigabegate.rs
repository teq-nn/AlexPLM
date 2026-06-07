//! The **Freigabe-Gate** decision core — the dreistufige Block in *einem* kontextabhängigen
//! Knopf (Issue #52, E19/E19.3, glossar „Freigabe-Block"/„Freigabe-Dialog").
//!
//! Following the house pattern (`aufgabenblock.rs`, `syncdecider.rs`, `warden.rs`): one **pure,
//! total, deterministic** function over a plain snapshot. It knows **no** git internals, no
//! clock, no I/O — the loading glue (read tasks + stale edges + Waisen/Pflicht) lives in
//! [`crate::freigabegateglue`]; this module only **classifies and sorts**. Snapshot in, exactly
//! one [`GateVerdict`] out.
//!
//! The load-bearing rule of E19 is that offene Punkte beim Revision/Tag are **nicht** auf
//! einen Haufen geworfen, sondern **nach Härte gestaffelt** — härtestes zuerst — behind a single
//! button whose Beschriftung *und* Schärfe wechselt (E19.3, „statt drei Knöpfen ein Knopf"):
//!
//! - **Warnung** ([`Haerte::Warnung`]) — eine Stale-Kante (E12/E20). Sichtbar, blockiert **nie**,
//!   braucht keine Begründung. The button is unaffected.
//! - **Weicher Block** ([`Haerte::Weich`]) — eine Waise / ein fehlendes Pflicht-Artefakt (E11).
//!   Blockiert „technisch vollständig", aber per **protokollierter Begründung** bewusst
//!   überwindbar (= §22.1): the button becomes „Trotzdem freigeben" and a logged sentence is
//!   required.
//! - **Harter Block** ([`Haerte::Hart`]) — eine offene blockierende Aufgabe am strengen
//!   Übergang (E15/E42). **Nicht** per Begründungstext wegzudrücken; nur durch Anfassen der
//!   Aufgabe selbst (Erledigen / Verwerfen / Herabstufen). The button is **aus**; the Ausweg is
//!   one click on the task — it never fully locks the user out (E22).
//!
//! The block is **personenübergreifend** (E19.1/E33): a colleague's open blocking Aufgabe holds
//! me too, and the dialog carries a cross-person Warnung („du taggst auch X' frischen Stand
//! mit"). That warning is informational; it carries no Härte of its own.
//!
//! ## Scope = Heimat-Pfade (E51b, Issue #138)
//!
//! Eine Baustein-Freigabe staffelt **nur** die offenen Punkte ihres eigenen Bereichs: gibt der
//! HW-Entwickler `elektronik` als „Rev B" frei, halten WIP-Firmware-Punkte (`firmware/…`) ihn
//! **nicht** als Geisel — sie fallen aus dem Urteil. Dafür nimmt der Kern einen [`Scope`] (die
//! Heimat-Pfade des reifenden Bausteins) entgegen und **filtert** die offenen Punkte darauf,
//! **bevor** er staffelt. Die Härte-Logik (Hart/Weich/Warnung, Knopf-Zustände) bleibt **unberührt**
//! — der Scope entscheidet nur, *welche* Punkte überhaupt zur Staffelung kommen.
//!
//! Ein Punkt liegt im Scope, wenn sein **Pfad** unter einem der Heimat-Pfade liegt (auf
//! Segmentgrenze, damit `elektro` nicht in `elektronik` „passt"): eine Waise/Stale-Kante über ihren
//! Pfad, eine Aufgabe über ihre Verknüpfung (`Arbeitsbereich`/`Artefakt`). Ein **produkt-weiter**
//! Punkt ohne Heimat-Pfad (eine Aufgabe am Produkt/an einer Version, ein label-only Pflicht-Eintrag)
//! ist **bereichsübergreifend** und bleibt in **jedem** Scope (er gehört keinem Bereich allein).
//! Ein **leerer** Scope filtert nicht — das ganze Produkt ist im Blick (der produkt-weite Aufrufer
//! und der Degradationspfad behalten ihr bisheriges Verhalten).
//!
//! The button has exactly **three Zustände** ([`KnopfZustand`]), driven by the härtestes vorhandene
//! item: clean → `Taggen`; only soft/warning → `TrotzdemFreigeben` (needs Begründung); any hard →
//! `GesperrtDurchAufgabe` (off). The pure core decides which — the UI re-derives nothing.

use crate::aufgabenblock::{decide_block, BlockDecision};
use crate::edges::StaleWarning;
use crate::graph::RevisionArt;
use crate::tasks::{Task, TaskLink};
use serde::Serialize;

/// The Härte of a single open point at the Freigabe-Gate (E19). Ordered **härtestes zuerst**:
/// the `Ord` derive ranks `Hart < Weich < Warnung`, so a plain ascending sort puts the hardest
/// items at the top of the list — exactly the „nach Härte sortierte Liste" of E19.3.
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Haerte {
    /// Harter Block: an open blocking Aufgabe. Button off; only the task itself dismisses it.
    Hart,
    /// Weicher Block: a Waise / missing Pflicht-Artefakt. Overridable by a logged Begründung.
    Weich,
    /// Warnung: a Stale-Kante. Visible, never blocks, no Begründung.
    Warnung,
}

/// What kind of open point an item is — the source axis behind its [`Haerte`]. Kept distinct from
/// the Härte so the UI can render the right Auswege (a hard task gets Erledigen/Verwerfen/…; a
/// Waise gets nothing but the Begründung path).
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Punktart {
    /// An open blocking Aufgabe (→ harter Block).
    Aufgabe,
    /// A Waise — a tracked file without an Etikett (→ weicher Block).
    Waise,
    /// A missing Pflicht-Artefakt (→ weicher Block).
    FehlendePflicht,
    /// A Stale-Kante: the derivation is older than its source (→ Warnung).
    StaleKante,
}

/// One open point in the härte-sortierte Liste. Carries everything the UI needs to render the
/// row *and* its Auswege without re-deciding: its Härte, its kind, a stable `ref_id` (the task
/// id, the orphan path, the missing artefact label, or the stale derivation path) and a human
/// `label`. A hard task additionally carries its three Auswege via `ref_id` (the UI acts on the
/// task by that id).
// Serialize-only (a computed verdict, never read back), so field names are free to be snake_case —
// which is what the frontend reads (`ref_id`). A kebab `rename_all` here would silently ship
// `ref-id` and the UI would read `undefined`; specta now pins these names across the seam.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OffenerPunkt {
    /// The Härte of this point — drives both the sort and the row's visual weight.
    pub haerte: Haerte,
    /// What kind of point this is (the source axis behind the Härte).
    pub art: Punktart,
    /// A stable reference: task id for an Aufgabe, product-relative path for a Waise/Stale-Kante,
    /// the Pflicht label for a missing artefact. The UI keys rows and Auswege on this.
    pub ref_id: String,
    /// A human one-liner naming the point (the task title, the orphan filename, …).
    pub label: String,
}

/// The three Zustände of the *one* context-dependent button (E19.3). Exactly one; total.
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnopfZustand {
    /// Alles sauber (or only Warnungen) → the button is the plain „Taggen". Proceeds freely.
    Taggen,
    /// A weicher Block (and no harter) → „Trotzdem freigeben"; a protokollierter Satz is required.
    TrotzdemFreigeben,
    /// A harter Block → the button is **aus** (gesperrt). The Ausweg is a click on the Aufgabe;
    /// no Begründung dismisses it.
    GesperrtDurchAufgabe,
}

/// A personenübergreifende Warnung (E19.1/E33): a colleague's frischer Stand is being co-tagged.
/// Informational — carries no Härte. `None` ⇔ no foreign recent work to warn about.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FremdWarnung {
    /// The colleague whose Stand is being co-tagged.
    pub wer: String,
    /// A ready human sentence („du taggst auch X' frischen Stand mit").
    pub satz: String,
}

/// The single verdict the Freigabe-Gate core returns for a checkpoint. Exactly one; total. It
/// carries the **härte-sortierte Liste** (härtestes zuerst), the resulting **Knopf-Zustand**, and
/// the optional personenübergreifende Warnung — so the UI renders the whole gate without
/// re-deciding anything.
// Serialize-only; field names stay snake_case to match the UI (`harter_block`, `begruendung_noetig`,
// `fremd_warnung`). The enum-valued fields (`knopf`) keep their kebab *values* from KnopfZustand —
// only this container's field names changed.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateVerdict {
    /// The open points, **härtestes zuerst** (Hart, then Weich, then Warnung), stable within a
    /// Härte (input order preserved). Empty ⇔ alles sauber.
    pub punkte: Vec<OffenerPunkt>,
    /// The resulting state of the one context-dependent button.
    pub knopf: KnopfZustand,
    /// `true` iff a harter Block is present — the button is off and only acting on the Aufgabe
    /// dismisses it (mirrors `knopf == GesperrtDurchAufgabe`, kept explicit for the UI).
    pub harter_block: bool,
    /// `true` iff a weicher Block is present and no harter — i.e. a protokollierter Satz is
    /// required to proceed (mirrors `knopf == TrotzdemFreigeben`).
    pub begruendung_noetig: bool,
    /// The personenübergreifende Warnung, if a colleague's frischer Stand is being co-tagged.
    pub fremd_warnung: Option<FremdWarnung>,
}

impl GateVerdict {
    /// Whether the Freigabe may proceed **without** any deliberate handle: only when alles sauber
    /// (button is plain `Taggen`). A weicher Block still proceeds, but only with a Begründung.
    pub fn is_clean(&self) -> bool {
        matches!(self.knopf, KnopfZustand::Taggen)
    }

    /// The ids of the open Aufgaben that hold a harter Block (in härte/list order). Empty ⇔ no
    /// harter Block. The UI lists exactly these with their three Auswege.
    pub fn hard_blocking_task_ids(&self) -> Vec<String> {
        self.punkte
            .iter()
            .filter(|p| p.art == Punktart::Aufgabe && p.haerte == Haerte::Hart)
            .map(|p| p.ref_id.clone())
            .collect()
    }
}

/// Der **Heimat-Scope** einer Baustein-Freigabe (E51b, Issue #138): die produkt-relativen
/// Heimat-Pfade des reifenden Bausteins, auf die der Kern die offenen Punkte filtert, **bevor** er
/// staffelt. Ein Punkt liegt im Scope, wenn sein Pfad unter **einem** dieser Pfade liegt (auf
/// Segmentgrenze). Ein **leerer** Scope filtert nicht — das ganze Produkt ist im Blick (der
/// produkt-weite Aufrufer und der Degradationspfad).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    /// Die Heimat-Pfade des Bereichs (z.B. `["elektronik"]`). Leer ⇒ ganzes Produkt (kein Filter).
    pub heimaten: Vec<String>,
}

impl Scope {
    /// Ein Scope über genau eine Heimat — die übliche Baustein-Freigabe (`elektronik`).
    pub fn heimat(heimat: impl Into<String>) -> Self {
        Scope {
            heimaten: vec![heimat.into()],
        }
    }

    /// Ob dieser Scope **nicht** filtert: ein leerer Scope nimmt das ganze Produkt in den Blick
    /// (produkt-weiter Aufrufer / Degradation).
    fn is_whole_product(&self) -> bool {
        self.heimaten.iter().all(|h| h.trim().is_empty())
    }

    /// Ob ein **pfad-tragender** Punkt im Scope liegt: sein Pfad unter einer der Heimaten (auf
    /// Segmentgrenze). Ein leerer Scope lässt alles durch.
    fn contains_path(&self, path: &str) -> bool {
        self.is_whole_product() || self.heimaten.iter().any(|h| within_heimat(path, h))
    }

    /// Ob ein **produkt-weiter** Punkt (ohne Heimat-Pfad) im Scope bleibt: er ist
    /// bereichsübergreifend und gehört keinem Bereich allein, also bleibt er in **jedem** Scope.
    /// (Eigene Methode statt eines Literals, damit die Regel an einer Stelle benannt ist.)
    fn keeps_product_wide(&self) -> bool {
        true
    }
}

/// Ob `path` **innerhalb** der Heimat `heimat` liegt (oder Heimat leer = ganze Produktwurzel).
/// Vergleich auf Segmentgrenze, damit `elektro` nicht in `elektronik` „passt". Rein + total.
/// (Spiegelt `zuordnung::within_heimat`; der Kern bleibt I/O-frei und self-contained.) `pub(crate)`,
/// damit die Glue (E51b) dieselbe Segmentregel zum Einsortieren der git-diff-Pfade nutzt.
pub(crate) fn within_heimat(path: &str, heimat: &str) -> bool {
    let h = heimat.trim().trim_matches('/');
    if h.is_empty() {
        return true;
    }
    let p = path.trim().trim_matches('/');
    p == h || p.starts_with(&format!("{h}/"))
}

/// Ob eine Aufgabe im `scope` liegt (E51b). Eine Aufgabe mit Heimat-Pfad-Verknüpfung
/// (`Arbeitsbereich`/`Artefakt`) gehört dem Bereich ihres Pfads — sie bleibt nur, wenn dieser im
/// Scope liegt. Eine **produkt-weite** Aufgabe (am Produkt, an einer Version, oder ohne
/// Verknüpfung) gehört keinem Bereich allein und bleibt bereichsübergreifend in jedem Scope. Rein.
fn task_in_scope(task: &Task, scope: &Scope) -> bool {
    match &task.link {
        Some(TaskLink::Arbeitsbereich(pfad)) | Some(TaskLink::Artefakt(pfad)) => {
            scope.contains_path(pfad)
        }
        // Produkt / Version / kein Link ⇒ produkt-weit, bereichsübergreifend.
        _ => scope.keeps_product_wide(),
    }
}

/// The collected inputs the gate classifies — already gathered by the glue, never fetched here.
/// Keeping this a plain snapshot keeps [`decide_gate`] pure and the table tests trivial.
#[derive(Debug, Clone, Default)]
pub struct GateInputs {
    /// The product's task snapshot (Aufgaben + Hinweise). The Aufgaben-Block core (#49) decides
    /// which of these hard-block; Hinweise never block.
    pub tasks: Vec<Task>,
    /// The fired Stale-Warnungen (#10) — each becomes a Warnung row.
    pub stale: Vec<StaleWarning>,
    /// The product-relative paths of Waisen — tracked files without an Etikett. Each → weicher
    /// Block.
    pub waisen: Vec<String>,
    /// The labels of fehlende Pflicht-Artefakte. Each → weicher Block. Label-only ⇒ produkt-weit
    /// (kein Heimat-Pfad), bleibt also in jedem Scope (E51b) — das Pflicht-Modell trägt noch keinen
    /// Pfad (eigene Slice).
    pub fehlende_pflicht: Vec<String>,
    /// The personenübergreifende Warnung, if any (E19.1).
    pub fremd_warnung: Option<FremdWarnung>,
}

/// The **Freigabe-Gate decision**: given the collected open points, the intended Revision-Art and
/// the **Heimat-Scope** of the reifenden Bausteins, produce the härte-sortierte Liste +
/// Knopf-Zustand. **Pure, total, deterministic** — no I/O, no clock.
///
/// The rule (E19/E19.3), in one breath: a Freigabe collects its open points from the
/// Aufgaben-Block (harter Block), the Waisen/Pflicht-Check (weicher Block) and the Stale-Kanten
/// (Warnung), sorts them härtestes zuerst, and chooses the one button's Zustand from the härtestes
/// vorhandene item — `Taggen` (clean) / `TrotzdemFreigeben` (weich, needs Begründung) /
/// `GesperrtDurchAufgabe` (hart, off).
///
/// **Scope (E51b):** before any staffing the open points are **filtered to `scope`** — a Punkt
/// außerhalb der Heimat-Pfade fällt heraus, sodass die Firmware-WIP eine Elektronik-Freigabe nicht
/// als Geisel hält. Ein **leerer** Scope filtert nicht (ganzes Produkt). Die Härte-Logik darunter
/// ist **unverändert**: der Scope entscheidet nur, *welche* Punkte gestaffelt werden, nie *wie*.
///
/// The hard-block set is decided by the reused [`decide_block`] core (#49) at the given `art`, so
/// a Prototyp surfaces only its „blockiert überall" Aufgaben as hard, while a Freigabe surfaces
/// every open Aufgabe. The Waisen/Pflicht weicher Block and the Stale Warnung do not depend on the
/// Art (they are technical completeness, not Strenge).
pub fn decide_gate(inputs: &GateInputs, art: RevisionArt, scope: &Scope) -> GateVerdict {
    // Scope-Filter zuerst (E51b): nur die Aufgaben des Bereichs gehen in den #49-Kern, damit eine
    // out-of-scope blockierende Aufgabe gar nicht erst als harter Block auftaucht. Die Strenge-/
    // Härte-Logik darunter bleibt unberührt.
    let scoped_tasks: Vec<Task> = inputs
        .tasks
        .iter()
        .filter(|t| task_in_scope(t, scope))
        .cloned()
        .collect();
    let block: BlockDecision = decide_block(&scoped_tasks, art);

    let mut punkte: Vec<OffenerPunkt> = Vec::new();

    // Harter Block: the open Aufgaben that block at this Art (decided by the #49 core, in its
    // input order). Name each with its title for the row + its three Auswege.
    for id in &block.blocking_task_ids {
        let label = scoped_tasks
            .iter()
            .find(|t| &t.id == id)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| id.clone());
        punkte.push(OffenerPunkt {
            haerte: Haerte::Hart,
            art: Punktart::Aufgabe,
            ref_id: id.clone(),
            label,
        });
    }

    // Weicher Block: Waisen, then fehlende Pflicht-Artefakte. Overridable by a logged Begründung.
    // Eine Waise trägt einen Pfad → wird auf den Scope gefiltert (E51b).
    for pfad in inputs.waisen.iter().filter(|p| scope.contains_path(p)) {
        punkte.push(OffenerPunkt {
            haerte: Haerte::Weich,
            art: Punktart::Waise,
            ref_id: pfad.clone(),
            label: pfad.clone(),
        });
    }
    // Fehlende Pflicht: label-only ⇒ produkt-weit, bleibt in jedem Scope (E51b).
    for pflicht in &inputs.fehlende_pflicht {
        if !scope.keeps_product_wide() {
            continue;
        }
        punkte.push(OffenerPunkt {
            haerte: Haerte::Weich,
            art: Punktart::FehlendePflicht,
            ref_id: pflicht.clone(),
            label: pflicht.clone(),
        });
    }

    // Warnung: the Stale-Kanten. Visible, never block. Each carries the derived path → auf den
    // Scope gefiltert (E51b).
    for w in inputs.stale.iter().filter(|w| scope.contains_path(&w.derived)) {
        punkte.push(OffenerPunkt {
            haerte: Haerte::Warnung,
            art: Punktart::StaleKante,
            ref_id: w.derived.clone(),
            label: w.derived.clone(),
        });
    }

    // härtestes zuerst — stable within a Härte (the source loops above already preserve input
    // order, and a stable sort keeps that order for equal keys).
    punkte.sort_by_key(|p| p.haerte);

    let has_hart = punkte.iter().any(|p| p.haerte == Haerte::Hart);
    let has_weich = punkte.iter().any(|p| p.haerte == Haerte::Weich);

    // The one button's Zustand is chosen by the härtestes vorhandene item (E19.3):
    let knopf = if has_hart {
        KnopfZustand::GesperrtDurchAufgabe
    } else if has_weich {
        KnopfZustand::TrotzdemFreigeben
    } else {
        // alles sauber, or only Warnungen — the button is the plain „Taggen".
        KnopfZustand::Taggen
    };

    GateVerdict {
        punkte,
        knopf,
        harter_block: has_hart,
        begruendung_noetig: !has_hart && has_weich,
        fremd_warnung: inputs.fremd_warnung.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{TaskKind, TaskStatus};

    fn task(
        id: &str,
        title: &str,
        kind: TaskKind,
        status: TaskStatus,
        blocks_everywhere: bool,
    ) -> Task {
        Task {
            id: id.to_string(),
            title: title.to_string(),
            kind,
            status,
            link: None,
            due: None,
            blocks_everywhere,
            created_at: "ts".to_string(),
        }
    }
    fn open_aufgabe(id: &str, title: &str) -> Task {
        task(id, title, TaskKind::Aufgabe, TaskStatus::Offen, false)
    }
    /// Eine offene Aufgabe, an einen Arbeitsbereich (Heimat-Pfad) gehängt — der Scope-Bezug (E51b).
    fn open_aufgabe_at(id: &str, title: &str, arbeitsbereich: &str) -> Task {
        let mut t = open_aufgabe(id, title);
        t.link = Some(TaskLink::Arbeitsbereich(arbeitsbereich.to_string()));
        t
    }
    /// Der **leere** Scope: kein Filter, ganzes Produkt im Blick (bisheriges Verhalten).
    fn whole() -> Scope {
        Scope::default()
    }
    fn stale(derived: &str) -> StaleWarning {
        StaleWarning {
            derived: derived.to_string(),
            source: "src".to_string(),
            source_timestamp: "2026-05-31T12:00:00Z".to_string(),
            derived_timestamp: "2026-05-30T12:00:00Z".to_string(),
        }
    }

    /// **Härte ordering**: the `Ord` derive ranks härtestes zuerst — `Hart < Weich < Warnung` — so
    /// a plain ascending sort yields the E19.3 list (hardest at the top).
    #[test]
    fn haerte_orders_hardest_first() {
        let mut v = vec![Haerte::Warnung, Haerte::Hart, Haerte::Weich];
        v.sort();
        assert_eq!(v, vec![Haerte::Hart, Haerte::Weich, Haerte::Warnung]);
    }

    /// **The core acceptance matrix**: every combination of {hart?, weich?, warnung?} → the
    /// correct single Knopf-Zustand (E19.3), proven in one table. The härtestes vorhandene item
    /// always decides the button.
    #[test]
    fn button_state_per_combination() {
        // (has_hart, has_weich, has_warn, expect_knopf)
        let cases: &[(bool, bool, bool, KnopfZustand)] = &[
            // alles sauber, and warning-only, both → plain Taggen (proceed freely).
            (false, false, false, KnopfZustand::Taggen),
            (false, false, true, KnopfZustand::Taggen),
            // a weicher Block (with/without a warning) → Trotzdem freigeben + Begründung.
            (false, true, false, KnopfZustand::TrotzdemFreigeben),
            (false, true, true, KnopfZustand::TrotzdemFreigeben),
            // any harter Block dominates → button off, no matter what else is present.
            (true, false, false, KnopfZustand::GesperrtDurchAufgabe),
            (true, true, false, KnopfZustand::GesperrtDurchAufgabe),
            (true, false, true, KnopfZustand::GesperrtDurchAufgabe),
            (true, true, true, KnopfZustand::GesperrtDurchAufgabe),
        ];
        for (hart, weich, warn, expect) in cases {
            let inputs = GateInputs {
                tasks: if *hart {
                    vec![open_aufgabe("a1", "Footprint Q3 prüfen")]
                } else {
                    vec![]
                },
                waisen: if *weich {
                    vec!["fertigung/verirrt.csv".to_string()]
                } else {
                    vec![]
                },
                stale: if *warn {
                    vec![stale("layout/board.kicad_pcb")]
                } else {
                    vec![]
                },
                fehlende_pflicht: vec![],
                fremd_warnung: None,
            };
            let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());
            assert_eq!(
                v.knopf, *expect,
                "hart={hart} weich={weich} warn={warn} → {expect:?}"
            );
            assert_eq!(v.harter_block, *hart, "harter_block flag must mirror knopf");
            assert_eq!(
                v.begruendung_noetig,
                !*hart && *weich,
                "begruendung_noetig iff weich and not hart"
            );
            assert_eq!(v.is_clean(), matches!(expect, KnopfZustand::Taggen));
        }
    }

    /// **Sort order over a real mixed bag** (E19 example: a Waise (weich), a changed PCB since the
    /// last Gerber (Warnung) and an open „Footprint Q3 prüfen" (hart)): the list comes out
    /// härtestes zuerst, and within a Härte the input order is preserved (stable).
    #[test]
    fn collects_and_sorts_hardest_first_stable_within_haerte() {
        let inputs = GateInputs {
            tasks: vec![
                open_aufgabe("hart-2", "Bohrplan prüfen"),
                open_aufgabe("hart-1", "Footprint Q3 prüfen"),
            ],
            waisen: vec!["fertigung/verirrt.csv".to_string()],
            fehlende_pflicht: vec!["Testprotokoll".to_string()],
            stale: vec![stale("layout/board.kicad_pcb")],
            fremd_warnung: None,
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());

        let order: Vec<(Haerte, &str)> = v
            .punkte
            .iter()
            .map(|p| (p.haerte, p.ref_id.as_str()))
            .collect();
        assert_eq!(
            order,
            vec![
                (Haerte::Hart, "hart-2"), // tasks in #49 input order
                (Haerte::Hart, "hart-1"),
                (Haerte::Weich, "fertigung/verirrt.csv"), // Waisen before fehlende Pflicht
                (Haerte::Weich, "Testprotokoll"),
                (Haerte::Warnung, "layout/board.kicad_pcb"),
            ],
            "härtestes zuerst, stable within a Härte"
        );
        // The hardest item present is a task → button off.
        assert_eq!(v.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert_eq!(v.hard_blocking_task_ids(), vec!["hart-2", "hart-1"]);
    }

    /// **Hard block is not dismissable by reason text** — only by acting on the task; and it
    /// **never** appears as a Begründung path. With a hard block present, `begruendung_noetig` is
    /// false even though a weicher Block also exists, and the offending task ids are named so the
    /// UI can show their Auswege.
    #[test]
    fn hard_block_offers_the_task_not_a_reason_box() {
        let inputs = GateInputs {
            tasks: vec![open_aufgabe("t1", "Footprint Q3 prüfen")],
            waisen: vec!["verirrt.csv".to_string()],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());
        assert!(v.harter_block);
        assert!(
            !v.begruendung_noetig,
            "a hard block is never dismissed by a reason"
        );
        assert_eq!(v.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert_eq!(v.hard_blocking_task_ids(), vec!["t1"]);
        assert!(!v.is_clean(), "a hard block is never clean");
    }

    /// **Soft block requires a logged sentence**: a Waise (no hard block) yields
    /// `TrotzdemFreigeben` + `begruendung_noetig`, and offers no task to act on.
    #[test]
    fn soft_block_requires_begruendung() {
        let inputs = GateInputs {
            waisen: vec!["fertigung/verirrt.csv".to_string()],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());
        assert_eq!(v.knopf, KnopfZustand::TrotzdemFreigeben);
        assert!(v.begruendung_noetig);
        assert!(!v.harter_block);
        assert!(v.hard_blocking_task_ids().is_empty());
        assert_eq!(v.punkte.len(), 1);
        assert_eq!(v.punkte[0].art, Punktart::Waise);
    }

    /// **Warnung never blocks**: a Stale-Kante with nothing else leaves the button at plain
    /// `Taggen` and is still listed (visible, no Begründung).
    #[test]
    fn warnung_is_visible_but_never_blocks() {
        let inputs = GateInputs {
            stale: vec![stale("layout/board.kicad_pcb")],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());
        assert_eq!(v.knopf, KnopfZustand::Taggen);
        assert!(
            v.is_clean(),
            "a Warnung alone does not block — taggen proceeds"
        );
        assert!(!v.begruendung_noetig);
        assert_eq!(v.punkte.len(), 1);
        assert_eq!(v.punkte[0].haerte, Haerte::Warnung);
    }

    /// **Reuses the #49 strictness**: the hard-block set is exactly [`decide_block`] at the Art.
    /// At a Prototyp an ordinary open Aufgabe does **not** hard-block (so it is absent from the
    /// list), while a „blockiert überall" Aufgabe hard-blocks even a Prototyp.
    #[test]
    fn hard_block_follows_revision_art_strictness() {
        let ordinary = vec![open_aufgabe("a1", "Footprint prüfen")];
        // Prototyp + ordinary open Aufgabe → not a hard block (the #49 lax rule).
        let p = decide_gate(
            &GateInputs {
                tasks: ordinary.clone(),
                ..Default::default()
            },
            RevisionArt::Prototyp,
            &whole(),
        );
        assert_eq!(
            p.knopf,
            KnopfZustand::Taggen,
            "a Prototyp is lax — no hard block"
        );
        assert!(p.punkte.is_empty());
        // Freigabe + the same Aufgabe → harter Block.
        let f = decide_gate(
            &GateInputs {
                tasks: ordinary,
                ..Default::default()
            },
            RevisionArt::Freigabe,
            &whole(),
        );
        assert_eq!(f.knopf, KnopfZustand::GesperrtDurchAufgabe);

        // „blockiert überall" hard-blocks even a Prototyp.
        let ueberall = vec![task(
            "u",
            "überall",
            TaskKind::Aufgabe,
            TaskStatus::Offen,
            true,
        )];
        let pu = decide_gate(
            &GateInputs {
                tasks: ueberall,
                ..Default::default()
            },
            RevisionArt::Prototyp,
            &whole(),
        );
        assert_eq!(pu.knopf, KnopfZustand::GesperrtDurchAufgabe);
    }

    /// A **Hinweis** never appears as a blocking point, in any context (it is not block-capable).
    #[test]
    fn hinweis_never_appears_as_a_block() {
        let inputs = GateInputs {
            tasks: vec![task(
                "h",
                "nur ein Hinweis",
                TaskKind::Hinweis,
                TaskStatus::Offen,
                true,
            )],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());
        assert_eq!(v.knopf, KnopfZustand::Taggen, "a Hinweis never blocks");
        assert!(v.punkte.is_empty());
    }

    /// **Personenübergreifend (E19.1/E33)**: the cross-person Warnung is carried through verbatim
    /// and is independent of the Härte staffing (it warns even when alles sauber). A colleague's
    /// open blocking Aufgabe holds me too — same hard block, regardless of whose task it is.
    #[test]
    fn cross_person_warning_is_carried_and_block_is_cross_person() {
        let warn = FremdWarnung {
            wer: "Alex".to_string(),
            satz: "du taggst auch Alex' frischen Stand mit".to_string(),
        };
        // alles sauber but a colleague pushed recently → the warning still appears.
        let clean = decide_gate(
            &GateInputs {
                fremd_warnung: Some(warn.clone()),
                ..Default::default()
            },
            RevisionArt::Freigabe,
            &whole(),
        );
        assert_eq!(clean.knopf, KnopfZustand::Taggen);
        assert_eq!(clean.fremd_warnung, Some(warn.clone()));

        // a colleague's open blocking Aufgabe holds me too — the gate doesn't know whose it is,
        // it blocks all the same (the block is personenübergreifend).
        let held = decide_gate(
            &GateInputs {
                tasks: vec![open_aufgabe("alex-task", "Alex: Footprint prüfen")],
                fremd_warnung: Some(warn.clone()),
                ..Default::default()
            },
            RevisionArt::Freigabe,
            &whole(),
        );
        assert_eq!(held.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert_eq!(held.hard_blocking_task_ids(), vec!["alex-task"]);
        assert_eq!(held.fremd_warnung, Some(warn));
    }

    /// **Total + deterministic**: empty inputs → clean Taggen with an empty list, and the same
    /// inputs always yield the same verdict.
    #[test]
    fn empty_is_clean_and_decision_is_deterministic() {
        let empty = GateInputs::default();
        let v = decide_gate(&empty, RevisionArt::Freigabe, &whole());
        assert!(v.is_clean());
        assert!(v.punkte.is_empty());
        assert!(v.fremd_warnung.is_none());

        let inputs = GateInputs {
            tasks: vec![open_aufgabe("t", "x")],
            waisen: vec!["w".to_string()],
            stale: vec![stale("s")],
            ..Default::default()
        };
        assert_eq!(
            decide_gate(&inputs, RevisionArt::Freigabe, &whole()),
            decide_gate(&inputs, RevisionArt::Freigabe, &whole())
        );
    }

    /// **E51b Scope-Filter — out-of-scope fällt heraus, in-scope bleibt hart/weich** (Issue #138):
    /// das E19-Beispiel-Gemenge über zwei Bereiche. Eine Freigabe von `elektronik` staffelt **nur**
    /// die Punkte unter `elektronik/` — die `firmware`-WIP (eine offene blockierende Aufgabe und eine
    /// firmware-Waise) fällt aus dem Urteil und hält die Freigabe nicht als Geisel. Die Härte-Logik
    /// darunter ist unverändert: die in-scope-Aufgabe ist weiter ein **harter** Block, die in-scope-
    /// Waise weiter **weich**, die in-scope-Stale-Kante weiter eine **Warnung**.
    #[test]
    fn scope_drops_out_of_scope_points_and_keeps_in_scope_hardness() {
        let inputs = GateInputs {
            tasks: vec![
                // in scope (elektronik) → harter Block …
                open_aufgabe_at("e-hart", "Footprint Q3 prüfen", "elektronik"),
                // … out of scope (firmware) → fällt heraus, blockiert die elektronik-Freigabe nicht.
                open_aufgabe_at("f-hart", "Bootloader fixen", "firmware"),
            ],
            waisen: vec![
                "elektronik/verirrt.csv".to_string(), // in scope → weich
                "firmware/build/stray.bin".to_string(), // out of scope → fällt heraus
            ],
            stale: vec![
                stale("elektronik/board.kicad_pcb"), // in scope → Warnung
                stale("firmware/app.elf"),           // out of scope → fällt heraus
            ],
            ..Default::default()
        };

        let v = decide_gate(&inputs, RevisionArt::Freigabe, &Scope::heimat("elektronik"));

        // Nur die drei elektronik-Punkte bleiben, härtestes zuerst — die firmware-Punkte sind weg.
        let order: Vec<(Haerte, &str)> = v
            .punkte
            .iter()
            .map(|p| (p.haerte, p.ref_id.as_str()))
            .collect();
        assert_eq!(
            order,
            vec![
                (Haerte::Hart, "e-hart"),
                (Haerte::Weich, "elektronik/verirrt.csv"),
                (Haerte::Warnung, "elektronik/board.kicad_pcb"),
            ],
            "nur in-scope-Punkte, Härte-Logik unverändert"
        );
        // Die in-scope-Aufgabe ist weiter ein harter Block; die firmware-Aufgabe gerade nicht.
        assert_eq!(v.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert_eq!(v.hard_blocking_task_ids(), vec!["e-hart"]);
    }

    /// **Aus Sicht der Firmware-Freigabe** dreht sich das Bild um (Issue #138): derselbe Eingabe-
    /// schnappschuss, aber Scope = `firmware` → nur die firmware-Aufgabe blockiert, die elektronik-
    /// Punkte fallen heraus. Jeder Bereich reift für sich.
    #[test]
    fn scope_is_per_heimat_independent() {
        let inputs = GateInputs {
            tasks: vec![
                open_aufgabe_at("e-hart", "Footprint Q3 prüfen", "elektronik"),
                open_aufgabe_at("f-hart", "Bootloader fixen", "firmware"),
            ],
            waisen: vec!["elektronik/verirrt.csv".to_string()],
            ..Default::default()
        };

        let f = decide_gate(&inputs, RevisionArt::Freigabe, &Scope::heimat("firmware"));
        assert_eq!(f.knopf, KnopfZustand::GesperrtDurchAufgabe);
        assert_eq!(
            f.hard_blocking_task_ids(),
            vec!["f-hart"],
            "nur die firmware-Aufgabe hält die firmware-Freigabe"
        );
        // Die elektronik-Waise gehört nicht zu firmware → kein weicher Block daraus.
        assert!(
            f.punkte.iter().all(|p| p.art != Punktart::Waise),
            "out-of-scope-Waise fällt heraus"
        );
    }

    /// **Segmentgrenze**: ein Geschwister-Ordner mit gemeinsamem Präfix (`elektronik-alt/`) liegt
    /// **nicht** im Scope von `elektronik` — `elektro` „passt" nicht in `elektronik`.
    #[test]
    fn scope_respects_segment_boundaries() {
        let inputs = GateInputs {
            waisen: vec![
                "elektronik/x.csv".to_string(),      // echt in scope
                "elektronik-alt/x.csv".to_string(),  // nur Präfix-Kollision → draußen
            ],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &Scope::heimat("elektronik"));
        let refs: Vec<&str> = v.punkte.iter().map(|p| p.ref_id.as_str()).collect();
        assert_eq!(refs, vec!["elektronik/x.csv"], "nur das echte Kind, nicht der Geschwisterordner");
    }

    /// **Produkt-weite Punkte bleiben bereichsübergreifend** (Issue #138): eine Aufgabe am Produkt
    /// (kein Heimat-Pfad) und ein label-only Pflicht-Eintrag gehören keinem Bereich allein — sie
    /// bleiben in **jedem** Scope. Eine produkt-weite blockierende Aufgabe hält darum auch eine
    /// Baustein-Freigabe.
    #[test]
    fn product_wide_points_stay_in_every_scope() {
        let mut produktweit = open_aufgabe("p-hart", "Konformitätserklärung");
        produktweit.link = Some(TaskLink::Produkt);
        let inputs = GateInputs {
            tasks: vec![produktweit],
            fehlende_pflicht: vec!["Testprotokoll".to_string()],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &Scope::heimat("elektronik"));
        assert_eq!(
            v.hard_blocking_task_ids(),
            vec!["p-hart"],
            "eine produkt-weite Aufgabe blockiert jeden Bereich"
        );
        assert!(
            v.punkte.iter().any(|p| p.art == Punktart::FehlendePflicht),
            "ein label-only Pflicht-Punkt bleibt bereichsübergreifend"
        );
    }

    /// **Der leere Scope filtert nicht** (E51b): das ganze Produkt bleibt im Blick — derselbe
    /// Verdikt wie vor der Scope-Verschärfung (Rückwärtskompatibilität für den produkt-weiten
    /// Aufrufer und den Degradationspfad).
    #[test]
    fn empty_scope_sees_the_whole_product() {
        let inputs = GateInputs {
            tasks: vec![open_aufgabe_at("f-hart", "Bootloader fixen", "firmware")],
            waisen: vec!["mechanik/verirrt.csv".to_string()],
            stale: vec![stale("elektronik/board.kicad_pcb")],
            ..Default::default()
        };
        let v = decide_gate(&inputs, RevisionArt::Freigabe, &whole());
        assert_eq!(v.punkte.len(), 3, "ein leerer Scope lässt alle Bereiche durch");
        assert_eq!(v.knopf, KnopfZustand::GesperrtDurchAufgabe);
    }
}
