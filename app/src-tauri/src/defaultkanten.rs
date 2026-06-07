//! Default-Kanten-Ableitung (Issue #56, E20) — der **reine Kern** + serde, kein I/O.
//!
//! Über dem Stale-Kern aus #10 (`edges.rs`) sitzen jetzt **drei Herkunftsstufen** einer Kante (E20):
//!
//! - **Baustein-Default** — Kante **innerhalb** eines Bausteins (Gerber ← Layout). Wird beim
//!   Onboarding **automatisch** angelegt: für jede [`DefaultKante`] des Bausteins eine konkrete
//!   [`Edge`] zwischen den Artefakten, die der `derived_glob`/`source_glob` **in der Heimat dieses
//!   Bausteins** trifft.
//! - **Baustein-Paar-Default** — Kante über **zwei** bekannt zusammengehörige Bausteine
//!   (Pick-and-Place ← Layout + BOM). Liegt als [`PaarDefaultKante`] bei einem Baustein
//!   („wenn Partner `partner_id` auch im Stack ist, schlage Z vor"). Ergebnis ist ein
//!   **deterministischer Vorschlag** ([`KantenVorschlag`]) — **kein** automatisches Anlegen, per
//!   Klick bestätigt. Kein ML, keine Daten, kein Parser (E21).
//! - **Hand-Kante** — bleibt in #10; nur das echt Idiosynkratische.
//!
//! **Still in Ruhe (E17):** Ist der **Quell-Baustein stillgelegt**, greifen seine Globs nicht mehr
//! → es entsteht **keine** Default-Kante mehr (und damit nach „keine Kante = keine Warnung", E40,
//! auch keine Stale-Warnung). Die Kante geht still in Ruhe, ohne Fehler, ohne Block.
//!
//! Wie im Haus üblich: **reiner Kern hier**, `#[cfg(test)]`-Tabellentests; die Persistenz/das
//! Onboarding-Glue lebt in `edgestore.rs`/`lib.rs`. Die Glob-/Heimat-Logik wird **nicht** neu
//! erfunden — sie kommt aus dem `zuordnung.rs`-Kern (`zuordnen`), damit es nur **eine** Wahrheit über
//! Zuordnung gibt.

use crate::edges::{Edge, KantenHerkunft};
use crate::stackstore::ProduktStack;
use crate::zuordnung::{zuordnen, BausteinRegel, Zuordnung};
use serde::Serialize;

/// Ein **bekannter Artefakt-Knoten** des Produkts: eine erfasste Datei mit dem Artefakt-Pfad
/// (produkt-relativer Ordner = die Identität, die [`Edge`] trägt), zu dem sie gehört. Genau das,
/// was die Werkbank/Projektion ohnehin kennt; hier nur als reiner Snapshot herein.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtefaktDatei {
    /// Produkt-relativer Pfad der Datei (Vorwärts-Slashes).
    pub pfad: String,
    /// Produkt-relativer **Ordner** der Datei — die Artefakt-Identität, die eine Kante trägt.
    pub ordner: String,
}

/// Ein **deterministischer Kanten-Vorschlag** aus einer Paar-Default-Regel (E20). Wird in der UI
/// angezeigt und **per Klick** zu einer echten [`Edge`] (Herkunft `PaarDefault`) bestätigt — nie
/// automatisch angelegt.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KantenVorschlag {
    /// Das abgeleitete Artefakt (Ordner-Pfad).
    pub derived: String,
    /// Die vorgeschlagene Quelle (Ordner-Pfad).
    pub source: String,
    /// `id` des Bausteins, dessen Paar-Regel den Vorschlag trägt (Anzeige).
    pub baustein_id: String,
    /// `id` des Partner-Bausteins, der den Vorschlag mit auslöst (Anzeige).
    pub partner_id: String,
}

impl KantenVorschlag {
    /// Die [`Edge`], die dieser Vorschlag bei Bestätigung anlegt (Herkunft `PaarDefault`).
    pub fn to_edge(&self) -> Edge {
        Edge::with_herkunft(self.derived.clone(), self.source.clone(), KantenHerkunft::PaarDefault)
    }
}

/// Die Artefakt-Ordner, in denen eine Datei liegt, die ein gegebenes **Glob in der Heimat
/// `heimat`** trifft. Nutzt den `zuordnen`-Kern nicht direkt (der entscheidet *welcher* Baustein),
/// sondern matcht das Glob selbst über `BausteinRegel` mit genau diesem einen Glob — so bleibt die
/// Glob-/Heimat-Logik die des Kerns, ohne dass ein anderer Baustein „dazwischenfunkt".
fn ordner_die_glob_treffen(
    dateien: &[ArtefaktDatei],
    heimat: &str,
    glob: &str,
    id: &str,
) -> Vec<String> {
    // Eine Ein-Glob-Regel mit der Heimat des Bausteins: trifft das Glob die Datei in der Heimat,
    // liefert `zuordnen` ein Artefakt; dessen Ordner ist die Artefakt-Identität.
    let regel = vec![BausteinRegel {
        id: id.to_string(),
        name: id.to_string(),
        heimat: heimat.to_string(),
        globs: vec![glob.to_string()],
        stillgelegt: false,
    }];
    let mut out: Vec<String> = Vec::new();
    for d in dateien {
        if matches!(zuordnen(&d.pfad, &regel), Zuordnung::Artefakt { .. }) && !out.contains(&d.ordner)
        {
            out.push(d.ordner.clone());
        }
    }
    out
}

/// **Baustein-Default-Kanten** ableiten (E20): für jede [`DefaultKante`] **innerhalb** eines (nicht
/// stillgelegten) Bausteins eine konkrete [`Edge`] zwischen den Artefakt-Ordnern, die `derived_glob`
/// und `source_glob` **in der Heimat dieses Bausteins** treffen. Rein und total.
///
/// **Still in Ruhe (E17):** ein **stillgelegter** Baustein liefert keine Default-Kanten (seine Globs
/// greifen nicht) — keine Kante, also auch keine Warnung. Self-Edges (`derived == source`, gleicher
/// Ordner) werden weggelassen. Das Ergebnis ist dedupliziert über die Endpunkte.
pub fn baustein_default_kanten(stack: &ProduktStack, dateien: &[ArtefaktDatei]) -> Vec<Edge> {
    let mut out: Vec<Edge> = Vec::new();
    for sb in &stack.bausteine {
        let b = &sb.baustein;
        if b.stillgelegt {
            continue; // still in Ruhe — greift nicht mehr (E17)
        }
        for dk in &b.default_kanten {
            let derived_ordner = ordner_die_glob_treffen(dateien, &b.heimat, &dk.derived_glob, &b.id);
            let source_ordner = ordner_die_glob_treffen(dateien, &b.heimat, &dk.source_glob, &b.id);
            for d in &derived_ordner {
                for s in &source_ordner {
                    if d == s {
                        continue; // kein Self-Edge (Quelle und Ableitung im selben Artefakt)
                    }
                    let edge = Edge::with_herkunft(d.clone(), s.clone(), KantenHerkunft::BausteinDefault);
                    if !out.iter().any(|e| e.same_endpoints(&edge)) {
                        out.push(edge);
                    }
                }
            }
        }
    }
    out
}

/// **Baustein-Paar-Default-Vorschläge** ableiten (E20): für jede [`PaarDefaultKante`] eines (nicht
/// stillgelegten) Bausteins, **deren Partner-Baustein ebenfalls (aktiv) im Stack liegt**, einen
/// deterministischen [`KantenVorschlag`]. Der `derived_glob` wird in der Heimat des **tragenden**
/// Bausteins gesucht, der `source_glob` in der Heimat des **Partners** (die Kante überspannt beide).
/// Rein und total — **kein** automatisches Anlegen; die UI bestätigt per Klick.
///
/// **Still in Ruhe (E17):** ist der tragende **oder** der Partner-Baustein stillgelegt, fällt der
/// Vorschlag weg. Vorschläge, deren Kante bereits in `vorhandene` existiert (egal welcher Herkunft),
/// werden **nicht** erneut vorgeschlagen (kein doppelter Lärm). Dedupliziert über die Endpunkte.
pub fn paar_default_vorschlaege(
    stack: &ProduktStack,
    dateien: &[ArtefaktDatei],
    vorhandene: &[Edge],
) -> Vec<KantenVorschlag> {
    let mut out: Vec<KantenVorschlag> = Vec::new();
    for sb in &stack.bausteine {
        let b = &sb.baustein;
        if b.stillgelegt {
            continue;
        }
        for pk in &b.paar_default_kanten {
            // Partner muss vorhanden UND aktiv sein.
            let Some(partner) = stack
                .bausteine
                .iter()
                .map(|x| &x.baustein)
                .find(|x| x.id == pk.partner_id)
            else {
                continue;
            };
            if partner.stillgelegt {
                continue;
            }
            let derived_ordner = ordner_die_glob_treffen(dateien, &b.heimat, &pk.derived_glob, &b.id);
            let source_ordner =
                ordner_die_glob_treffen(dateien, &partner.heimat, &pk.source_glob, &partner.id);
            for d in &derived_ordner {
                for s in &source_ordner {
                    if d == s {
                        continue;
                    }
                    // Schon als Kante vorhanden (Hand/Default/bestätigt)? Dann nicht erneut vorschlagen.
                    let already =
                        vorhandene.iter().any(|e| e.derived == *d && e.source == *s);
                    let suggested = out.iter().any(|v| v.derived == *d && v.source == *s);
                    if already || suggested {
                        continue;
                    }
                    out.push(KantenVorschlag {
                        derived: d.clone(),
                        source: s.clone(),
                        baustein_id: b.id.clone(),
                        partner_id: partner.id.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Default-Kanten in eine bestehende Kantenmenge **einfügen** (Onboarding/Refresh): jede abgeleitete
/// Default-Kante wird über [`crate::edges::add_edge`]-Semantik (Endpunkt-Dedup, kein Self-Edge)
/// hinzugefügt. Bereits vorhandene Endpunkte bleiben **unangetastet** — eine Hand-Kante wird also
/// nie von einer Default-Kante überschrieben. Rein; gibt die vereinigte Menge zurück.
pub fn mit_default_kanten(vorhandene: Vec<Edge>, defaults: &[Edge]) -> Vec<Edge> {
    let mut out = vorhandene;
    for d in defaults {
        if d.derived == d.source {
            continue;
        }
        if !out.iter().any(|e| e.same_endpoints(d)) {
            out.push(d.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baustein::{Baustein, DefaultKante, Oeffnen, PaarDefaultKante};
    use crate::stackstore::{Herkunft as SHerkunft, StackBaustein};

    fn baustein(
        id: &str,
        heimat: &str,
        globs: &[&str],
        default_kanten: Vec<DefaultKante>,
        paar: Vec<PaarDefaultKante>,
    ) -> Baustein {
        Baustein {
            id: id.to_string(),
            version: 1,
            name: id.to_string(),
            heimat: heimat.to_string(),
            globs: globs.iter().map(|s| s.to_string()).collect(),
            ignore: vec![],
            lfs: vec![],
            rekonstruierbar: vec![],
            oeffnen: Oeffnen::Auto,
            startaufgaben: vec![],
            default_kanten,
            paar_default_kanten: paar,
            stillgelegt: false,
        }
    }

    fn dk(derived: &str, source: &str) -> DefaultKante {
        DefaultKante { derived_glob: derived.to_string(), source_glob: source.to_string() }
    }

    fn pk(partner: &str, derived: &str, source: &str) -> PaarDefaultKante {
        PaarDefaultKante {
            partner_id: partner.to_string(),
            derived_glob: derived.to_string(),
            source_glob: source.to_string(),
        }
    }

    fn stack_of(bs: &[Baustein]) -> ProduktStack {
        ProduktStack {
            toolstack: None,
            bausteine: bs
                .iter()
                .map(|b| StackBaustein {
                    herkunft: SHerkunft { from: b.id.clone(), version: b.version },
                    baustein: b.clone(),
                })
                .collect(),
        }
    }

    fn datei(pfad: &str) -> ArtefaktDatei {
        // Ordner = alles vor dem letzten Slash (Vorwärts-Slashes), wie die Werkbank.
        let ordner = pfad.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        ArtefaktDatei { pfad: pfad.to_string(), ordner }
    }

    /// **Provenance-Tabelle** für Baustein-Default-Kanten: die abgeleitete Kante trägt Herkunft
    /// `BausteinDefault`, verbindet die richtigen Artefakt-Ordner und respektiert die Heimat.
    #[test]
    fn baustein_default_derives_within_heimat_with_provenance() {
        // fusion: *.stl stammt aus *.f3d, beides in der Heimat mechanik.
        let fusion = baustein(
            "fusion",
            "mechanik",
            &["*.f3d", "*.stl"],
            vec![dk("*.stl", "*.f3d")],
            vec![],
        );
        let stack = stack_of(&[fusion]);
        let dateien = vec![
            datei("mechanik/gehaeuse/gehaeuse.f3d"),
            datei("mechanik/gehaeuse/gehaeuse.stl"), // SELBES Artefakt -> kein Self-Edge
            datei("mechanik/deckel/deckel.f3d"),
            datei("mechanik/deckel/deckel.stl"),
            datei("elektronik/x.stl"), // außerhalb der Heimat mechanik -> ignoriert
        ];
        let edges = baustein_default_kanten(&stack, &dateien);

        // Jede abgeleitete Kante trägt Herkunft BausteinDefault, ist kein Self-Edge und liegt ganz
        // innerhalb der Heimat mechanik (das *.stl aus elektronik wird nie zur Quelle/Ableitung).
        assert!(edges.iter().all(|e| e.herkunft == KantenHerkunft::BausteinDefault));
        assert!(edges.iter().all(|e| e.derived != e.source), "kein Self-Edge");
        assert!(edges
            .iter()
            .all(|e| e.source.starts_with("mechanik/") && e.derived.starts_with("mechanik/")));
        // Konkret: mechanik/deckel (hat .stl) leitet sich von mechanik/gehaeuse (hat .f3d) ab —
        // die Globs greifen ordnerübergreifend in derselben Heimat (deckel<-deckel ist Self-Edge).
        assert!(
            edges
                .iter()
                .any(|e| e.derived == "mechanik/deckel" && e.source == "mechanik/gehaeuse"),
            "deckel.stl muss aus einer f3d-Quelle abgeleitet sein: {edges:?}"
        );
    }

    /// **Still in Ruhe (E17):** ein stillgelegter Quell-Baustein liefert keine Default-Kante mehr —
    /// keine Kante, also auch keine Warnung (E40). Das ist das „leise Zur-Ruhe-Gehen".
    #[test]
    fn stillgelegter_baustein_yields_no_default_edge() {
        let mut fusion = baustein(
            "fusion",
            "mechanik",
            &["*.f3d", "*.stl"],
            vec![dk("*.stl", "*.f3d")],
            vec![],
        );
        let dateien = vec![
            datei("mechanik/teil/teil.f3d"),
            datei("mechanik/abgeleitet/teil.stl"),
        ];
        // aktiv: es entsteht eine Kante
        let aktiv = baustein_default_kanten(&stack_of(&[fusion.clone()]), &dateien);
        assert_eq!(aktiv.len(), 1, "aktiv: genau eine Default-Kante");
        // stillgelegt: KEINE Kante
        fusion.stillgelegt = true;
        let still = baustein_default_kanten(&stack_of(&[fusion]), &dateien);
        assert!(still.is_empty(), "stillgelegt -> Kante geht still in Ruhe");
    }

    /// **Paar-Default-Vorschlag** nur, wenn beide Bausteine (aktiv) im Stack sind; deterministisch,
    /// per Klick zu einer Kante mit Herkunft `PaarDefault`. Tabelle über An-/Abwesenheit + Stillegung.
    #[test]
    fn paar_default_suggested_only_when_both_present_and_active() {
        // jlcpcb (Fertigung): Pick-and-Place (*.pos) stammt aus dem KiCad-PCB (*.kicad_pcb).
        let jlc = baustein(
            "jlcpcb",
            "fertigung",
            &["*.pos"],
            vec![],
            vec![pk("kicad", "*.pos", "*.kicad_pcb")],
        );
        let kicad = baustein("kicad", "elektronik", &["*.kicad_pcb"], vec![], vec![]);
        let dateien = vec![
            datei("fertigung/pnp.pos"),
            datei("elektronik/board/board.kicad_pcb"),
        ];

        // beide aktiv -> genau ein Vorschlag mit korrekter Herkunft beim Bestätigen
        let v = paar_default_vorschlaege(&stack_of(&[jlc.clone(), kicad.clone()]), &dateien, &[]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].derived, "fertigung");
        assert_eq!(v[0].source, "elektronik/board");
        assert_eq!(v[0].baustein_id, "jlcpcb");
        assert_eq!(v[0].partner_id, "kicad");
        assert_eq!(v[0].to_edge().herkunft, KantenHerkunft::PaarDefault);

        // Partner fehlt -> kein Vorschlag
        let nur_jlc = paar_default_vorschlaege(&stack_of(&[jlc.clone()]), &dateien, &[]);
        assert!(nur_jlc.is_empty(), "ohne Partner kein Paar-Vorschlag");

        // Partner stillgelegt -> kein Vorschlag (still in Ruhe)
        let mut kicad_still = kicad.clone();
        kicad_still.stillgelegt = true;
        let v_still = paar_default_vorschlaege(&stack_of(&[jlc.clone(), kicad_still]), &dateien, &[]);
        assert!(v_still.is_empty(), "Partner stillgelegt -> kein Vorschlag");

        // bereits als Kante vorhanden -> nicht erneut vorschlagen (kein doppelter Lärm)
        let schon = vec![Edge::new("fertigung", "elektronik/board")];
        let v_schon = paar_default_vorschlaege(&stack_of(&[jlc, kicad]), &dateien, &schon);
        assert!(v_schon.is_empty(), "vorhandene Kante wird nicht erneut vorgeschlagen");
    }

    /// `mit_default_kanten`: Default-Kanten werden eingefügt, eine bestehende **Hand-Kante** auf
    /// denselben Endpunkten bleibt aber unangetastet (nie überschrieben).
    #[test]
    fn mit_default_kanten_preserves_existing_hand_edge() {
        let hand = Edge::new("a", "b"); // Hand-Kante a<-b
        let defaults = vec![
            Edge::with_herkunft("a", "b", KantenHerkunft::BausteinDefault), // gleiche Endpunkte -> No-op
            Edge::with_herkunft("c", "d", KantenHerkunft::BausteinDefault), // neu
            Edge::with_herkunft("x", "x", KantenHerkunft::BausteinDefault), // Self-Edge -> weggelassen
        ];
        let merged = mit_default_kanten(vec![hand], &defaults);
        assert_eq!(merged.len(), 2);
        // a<-b bleibt die Hand-Kante
        let ab = merged.iter().find(|e| e.derived == "a" && e.source == "b").unwrap();
        assert_eq!(ab.herkunft, KantenHerkunft::Hand, "Hand-Kante nicht überschrieben");
        assert!(merged.iter().any(|e| e.derived == "c" && e.source == "d"));
        assert!(!merged.iter().any(|e| e.derived == "x"));
    }
}
