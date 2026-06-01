//! Knoten-Verben + Graph-Raum-Filter — der reine, totale Kern (Issue #55, E27/E45).
//!
//! Der **Graph-Raum** (Verlauf) ist ein eigener Raum neben der Werkbank (E45): man *sucht ihn
//! auf*, er ist nicht der Start. Ein Klick auf einen alten Knoten bewegt die Werkbank **nie**
//! still — er bietet drei **Verben** an (E27/E3):
//!
//! - **Als Ordner öffnen** (Default) — eine schreibgeschützte Kopie *daneben* (Worktree/Export),
//!   on demand. Laufende Arbeit unberührt.
//! - **Von hier abzweigen** — ein bewusster neuer Branch (E43: „branch" darf beim Namen genannt
//!   werden); *darf* die Werkbank bewegen, weil ausdrücklich gewollt. Laufende Arbeit wird vorher
//!   gesichert (E8).
//! - **Zurückwerfen** — der destruktive Sprung auf einen alten Stand; **nie der Default**, immer
//!   hinter der schwarzen „Historie anfassen"-Gate (E38/E27). Die gefährliche Mechanik
//!   (`reset --hard`/`rebase`/`stash`) bleibt versteckt (E43): das Werkzeug setzt den alten Stand
//!   als **neuen, vorwärts gerichteten Stand** obendrauf — sicher, reversibel, nie umgeschrieben.
//!
//! Wie im Haus üblich: **reiner Kern hier, kein I/O.** Welche Verben auf welchem Knoten erlaubt
//! sind und welche Knoten ein Filter durchlässt, wird hier rein entschieden und durch
//! `#[cfg(test)]`-Tabellentests über das Kreuzprodukt belegt. Die seiteneffekt-behaftete
//! git-Glue (Worktree anlegen, abzweigen, sicher zurückwerfen) lebt dünn in
//! [`crate::worktreeglue`] und fährt ausschließlich über die `gitrunner`-Helfer.

use serde::Serialize;

/// Eines der drei **Knoten-Verben** des Graph-Raums (E27). Serialisiert in kebab-case zur UI
/// (`"als-ordner-oeffnen"` / `"von-hier-abzweigen"` / `"zurueckwerfen"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnotenVerb {
    /// Default: schreibgeschützte Kopie des Stands als Ordner daneben (Worktree). Werkbank ruht.
    AlsOrdnerOeffnen,
    /// Bewusster neuer Branch ab diesem Stand; laufende Arbeit wird vorher gesichert.
    VonHierAbzweigen,
    /// Destruktiver Sprung — hinter der schwarzen Gate, nie Default. Sicher umgesetzt (s. o.).
    Zurueckwerfen,
}

/// Die für die Verb-Entscheidung relevanten Fakten *eines* Knotens — rein aus der #8/#28/#53-
/// Projektion ([`crate::graph::StandNode`]) abgeleitet, hier ohne I/O hineingereicht. Kein neuer
/// Fakt wird erfunden: `is_active_tip` = der Knoten ist die Spitze der aktiven Linie (wo man
/// gerade steht), `offloaded` = sein Inhalt ist ausgelagert (E36).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnotenFakten {
    /// Dieser Knoten ist die Spitze der aktiven Linie — der „Jetzt-Zustand" der Werkbank.
    pub is_active_tip: bool,
    /// Der Binär-Inhalt dieses Knotens ist ausgelagert (E36): er lässt sich nicht materialisieren.
    pub offloaded: bool,
}

/// **Der reine Kern**: welche Verben auf diesem Knoten erlaubt sind — total, deterministisch,
/// stabil sortiert (Reihenfolge der Aufzählung = Reihenfolge im UI-Menü, Default zuerst).
///
/// Regeln:
/// - **Als Ordner öffnen** ist (fast) immer erlaubt — der ruhige Default; *außer* der Inhalt ist
///   ausgelagert, dann gibt es nichts zu materialisieren (E36).
/// - **Von hier abzweigen** ist immer erlaubt (auch von der aktiven Spitze: ein neuer Branch von
///   „hier" ist legitim) — *außer* ausgelagert (der abgezweigte Stand bräuchte den Inhalt).
/// - **Zurückwerfen** ist erlaubt, *außer* der Knoten ist bereits die aktive Spitze (man steht
///   schon dort — nichts zurückzuwerfen) oder sein Inhalt ist ausgelagert (kein Inhalt zum
///   Wiederherstellen). Es bleibt der destruktive Sonderfall hinter der Gate.
pub fn allowed_verbs(f: KnotenFakten) -> Vec<KnotenVerb> {
    let mut out = Vec::with_capacity(3);
    // Default zuerst.
    if !f.offloaded {
        out.push(KnotenVerb::AlsOrdnerOeffnen);
        out.push(KnotenVerb::VonHierAbzweigen);
    }
    if !f.is_active_tip && !f.offloaded {
        out.push(KnotenVerb::Zurueckwerfen);
    }
    out
}

/// Ob ein bestimmtes Verb auf diesem Knoten erlaubt ist (Bequemlichkeit über [`allowed_verbs`]).
/// Die Glue ruft das als Wächter, bevor sie eine git-Operation überhaupt anfasst.
pub fn verb_allowed(f: KnotenFakten, verb: KnotenVerb) -> bool {
    allowed_verbs(f).contains(&verb)
}

/// Der **Filterzustand** des Graph-Raums (E45). Reine Anzeige-Filter — sie speichern/erfinden
/// **nichts** und schreiben nie um; sie blenden nur Knoten aus. Default = alles sichtbar.
///
/// - `varianten`: Varianten (Zweige neben der aktiven Linie) ein-/ausblenden. `false` = nur die
///   aktive Linie (der Stamm) bleibt sichtbar.
/// - `nur_revisionen`: nur Revisionen zeigen (E9) — die promovierten Stände, der Rest ruht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GraphFilter {
    /// Varianten (nicht-aktive Zweige) sichtbar? Default `true`.
    pub varianten: bool,
    /// Nur Revisionen zeigen? Default `false` (alle Stände).
    pub nur_revisionen: bool,
}

impl Default for GraphFilter {
    fn default() -> Self {
        GraphFilter {
            varianten: true,
            nur_revisionen: false,
        }
    }
}

/// Die für den Filter relevanten Fakten *eines* Knotens — rein aus der Projektion abgeleitet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterFakten {
    /// Der Knoten liegt auf der aktiven Linie (dem Stamm) — `false` = eine Variante (Zweig).
    pub on_active: bool,
    /// Der Knoten ist ein Revision (promovierter Stand).
    pub is_revision: bool,
}

/// **Der reine Kern**: lässt dieser Filter diesen Knoten durch? Total. Ein Knoten überlebt nur,
/// wenn er *alle* aktiven Filterbedingungen erfüllt:
/// - Sind Varianten ausgeblendet, überleben nur Knoten der aktiven Linie.
/// - Ist „nur Revisionen" an, überleben nur Revisionen.
///
/// Wichtig (E45): der Filter blendet nur aus — er schreibt nichts um, speichert nichts. Die
/// aktive Spitze („wo ich stehe") liegt per Definition auf der aktiven Linie und übersteht den
/// Varianten-Filter immer; ist sie kein Revision, kann der Revision-Filter sie ausblenden
/// (rein optisch, der Jetzt-Zustand der Werkbank bleibt davon unberührt — eigener Raum, E45).
pub fn passes_filter(node: FilterFakten, filter: GraphFilter) -> bool {
    if !filter.varianten && !node.on_active {
        return false;
    }
    if filter.nur_revisionen && !node.is_revision {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::KnotenVerb::*;
    use super::*;

    fn fakten(is_active_tip: bool, offloaded: bool) -> KnotenFakten {
        KnotenFakten { is_active_tip, offloaded }
    }

    #[test]
    fn allowed_verbs_cover_the_cross_product_of_node_facts() {
        // table: (is_active_tip, offloaded) -> die erlaubten Verben, in Menü-Reihenfolge.
        let cases: &[((bool, bool), &[KnotenVerb])] = &[
            // ein gewöhnlicher alter Knoten: alle drei Verben (Default zuerst, Zurückwerfen zuletzt)
            ((false, false), &[AlsOrdnerOeffnen, VonHierAbzweigen, Zurueckwerfen]),
            // die aktive Spitze: kein Zurückwerfen (man steht schon dort), aber öffnen/abzweigen
            ((true, false), &[AlsOrdnerOeffnen, VonHierAbzweigen]),
            // ausgelagert: nichts zu materialisieren -> gar kein Verb
            ((false, true), &[]),
            // ausgelagert UND aktive Spitze (degeneriert): ebenfalls kein Verb
            ((true, true), &[]),
        ];
        for ((tip, off), expected) in cases {
            assert_eq!(
                allowed_verbs(fakten(*tip, *off)),
                expected.to_vec(),
                "facts: active_tip={tip}, offloaded={off}"
            );
        }
    }

    #[test]
    fn als_ordner_oeffnen_is_the_default_and_listed_first_when_present() {
        // Der ruhige Default steht, wo erlaubt, immer an erster Stelle (Menü-Reihenfolge).
        for (tip, off) in [(false, false), (true, false)] {
            let verbs = allowed_verbs(fakten(tip, off));
            assert_eq!(verbs.first(), Some(&AlsOrdnerOeffnen), "tip={tip}, off={off}");
        }
    }

    #[test]
    fn zurueckwerfen_is_never_offered_on_the_active_tip_or_offloaded() {
        // Der destruktive Sprung ist nie der Default und nie da, wo er sinnlos/unmöglich ist.
        assert!(!verb_allowed(fakten(true, false), Zurueckwerfen), "active tip");
        assert!(!verb_allowed(fakten(false, true), Zurueckwerfen), "offloaded");
        // aber auf einem gewöhnlichen alten Knoten ist er erlaubt (hinter der Gate)
        assert!(verb_allowed(fakten(false, false), Zurueckwerfen));
    }

    #[test]
    fn verb_allowed_agrees_with_allowed_verbs() {
        // Querprobe: verb_allowed ist genau die Mitgliedschaft in allowed_verbs.
        for tip in [false, true] {
            for off in [false, true] {
                let f = fakten(tip, off);
                let set = allowed_verbs(f);
                for v in [AlsOrdnerOeffnen, VonHierAbzweigen, Zurueckwerfen] {
                    assert_eq!(verb_allowed(f, v), set.contains(&v), "{f:?} / {v:?}");
                }
            }
        }
    }

    fn ff(on_active: bool, is_revision: bool) -> FilterFakten {
        FilterFakten { on_active, is_revision }
    }

    #[test]
    fn default_filter_shows_everything() {
        let f = GraphFilter::default();
        assert!(f.varianten && !f.nur_revisionen);
        for on_active in [false, true] {
            for ms in [false, true] {
                assert!(passes_filter(ff(on_active, ms), f), "default passes all");
            }
        }
    }

    #[test]
    fn filters_only_hide_over_the_cross_product() {
        // table: (varianten, nur_revisionen) x (on_active, is_revision) -> sichtbar?
        let cases: &[((bool, bool), (bool, bool), bool)] = &[
            // Varianten AUS: nur die aktive Linie überlebt
            ((false, false), (true, false), true),   // aktive Linie bleibt
            ((false, false), (false, false), false), // Variante fällt weg
            ((false, false), (false, true), false),  // auch ein Varianten-Revision fällt weg
            // nur Revisionen: nur promovierte Stände
            ((true, true), (true, true), true),       // aktiver Revision bleibt
            ((true, true), (true, false), false),     // gewöhnlicher Stand fällt weg
            ((true, true), (false, true), true),      // auch ein Varianten-Revision bleibt
            // beide Filter zusammen: aktive Linie UND Revision
            ((false, true), (true, true), true),
            ((false, true), (false, true), false),   // Varianten-Revision: Varianten-Filter wirft ihn raus
            ((false, true), (true, false), false),   // aktiver Nicht-Revision: Revision-Filter wirft ihn raus
        ];
        for ((var, nur_ms), (on_active, ms), expected) in cases {
            let filter = GraphFilter { varianten: *var, nur_revisionen: *nur_ms };
            assert_eq!(
                passes_filter(ff(*on_active, *ms), filter),
                *expected,
                "filter(var={var}, nur_ms={nur_ms}) node(on_active={on_active}, ms={ms})"
            );
        }
    }

    #[test]
    fn active_line_always_survives_the_varianten_filter() {
        // „Wo ich stehe" liegt auf der aktiven Linie und überlebt das Ausblenden der Varianten
        // immer — der eigene Raum bleibt navigierbar, egal wie scharf gefiltert wird (E45).
        let only_trunk = GraphFilter { varianten: false, nur_revisionen: false };
        assert!(passes_filter(ff(true, false), only_trunk));
        assert!(passes_filter(ff(true, true), only_trunk));
    }
}
