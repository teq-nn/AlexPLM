//! Der **Compose-Kern** — die reine Entscheidung der Produkt-Komposition (Issue #139, E51c/E52).
//!
//! Folgt dem Haus-Muster (`reconciler.rs`, `syncdecider.rs`, `import_gate.rs`): **eine reine,
//! totale, deterministische** Funktion über eine schlichte Eingabe. Sie kennt **kein** git, keine
//! Uhr, keinen Prozess — das git-Plumbing (`read-tree`/`commit-tree`, der mehrelterige
//! Compose-Commit) lebt in [`crate::composeglue`]; dieses Modul entscheidet **ausschließlich** die
//! Komposition: welcher Heimat-Teilbaum aus welchem Release-Tag kommt, und die daraus folgende
//! **Produkt-Stückliste (BOM)**.
//!
//! ## Warum eine Komposition (E51/E52)
//!
//! Jeder **Baustein** reift für sich und setzt bei seiner Freigabe einen **dauerhaften** Tag
//! (`freigabe/<heimat>/<label>` — E51a, #131), der durabel auf genau den freigegebenen Stand zeigt.
//! Eine **Produkt-Revision** setzt nun pro verpflichtendem Baustein einen **gewählten Release-Tag**
//! zu einem reproduzierbaren Gesamt-Stand zusammen — ein **Compose-Commit, ohne Submodule** —,
//! dessen Baum **physisch** der Stückliste entspricht. „Neue PCB + alte Firmware" wird so **ein**
//! Produkt-Stand, dessen Ordner exakt das enthält (**Baum = BOM**); „als Ordner öffnen" liefert nie
//! versehentlich WIP, weil der Compose-Commit auf eingefrorene Tags zeigt, nicht auf den HEAD eines
//! Bausteins.
//!
//! ## Die eine Garantie (Property)
//!
//! Jeder Heimat-Teilbaum kommt aus **genau einem** gewählten Tag. Der Compose-Kern erlaubt darum
//! keine zwei Auswahlen auf denselben Heimat-Pfad und keinen Heimat-Pfad, der einen anderen
//! enthält — sonst wäre nicht mehr eindeutig, wessen Tag einen Unterordner stellt. Diese Invariante
//! wird im Kern geprüft (table + property) und in der Glue physisch eingelöst: der gebaute Baum
//! deckt sich Eintrag für Eintrag mit der BOM (kein „Tag HEAD + Lüge im Manifest").

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ----------------------------------------------------------------------------------------------
// Eingabe — die Auswahl pro Baustein
// ----------------------------------------------------------------------------------------------

/// Eine **Baustein-Auswahl** für die Produkt-Komposition (Issue #139): „nimm den Heimat-Teilbaum
/// `heimat` aus dem freigegebenen Stand, auf den `release_tag` zeigt". Schlichte Daten — die Glue
/// füllt `release_tag` aus dem dauerhaften Baustein-Freigabe-Tag (E51a) und `heimat` aus dem
/// Heimat-Ordner des Bausteins (z.B. `elektronik`). Der Compose-Kern macht **kein** I/O über sie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BausteinWahl {
    /// `id` des Bausteins (z.B. `"kicad"`), nur fürs Protokoll/die BOM-Lesbarkeit getragen.
    pub baustein_id: String,
    /// Der Heimat-Ordner, den dieser Baustein im Produkt regiert (z.B. `"elektronik"`). Der
    /// Teilbaum unter genau diesem Pfad wird aus dem gewählten Tag in den Compose-Baum gehoben.
    pub heimat: String,
    /// Der dauerhafte Release-Tag des freigegebenen Stands (E51a, z.B. `freigabe/elektronik/Rev-B`),
    /// auf den durabel der für diesen Bereich freigegebene Stand zeigt. Aus ihm liest die Glue den
    /// Heimat-Teilbaum (`read-tree`); er wird zugleich Eltern-Stand des Compose-Commits.
    pub release_tag: String,
}

// ----------------------------------------------------------------------------------------------
// Ausgabe — die Compose-Baum-Spezifikation + die Produkt-Stückliste (BOM)
// ----------------------------------------------------------------------------------------------

/// Ein **Eintrag der Compose-Baum-Spezifikation**: „der Teilbaum unter `heimat` kommt aus dem Stand,
/// auf den `release_tag` zeigt". Die Glue setzt jeden Eintrag mit `read-tree --prefix=<heimat>`
/// physisch in den Compose-Baum; der Kern legt nur fest, **welcher** Teilbaum aus **welchem** Tag.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComposeEintrag {
    /// Der Heimat-Pfad, dessen Teilbaum gesetzt wird (z.B. `"elektronik"`). Normalisiert
    /// (Vorwärts-Schrägstriche, keine führenden/abschließenden `/`).
    pub heimat: String,
    /// Der dauerhafte Release-Tag, aus dem der Teilbaum gehoben wird (z.B. `freigabe/elektronik/Rev-B`).
    pub release_tag: String,
    /// `id` des Bausteins, der diesen Heimat-Bereich regiert — fürs Protokoll/die Nachvollziehbarkeit.
    pub baustein_id: String,
}

/// Die **Produkt-Stückliste (BOM)** einer Produkt-Revision: pro verpflichtendem Baustein der eine
/// gewählte Release-Stand, der seinen Heimat-Bereich stellt. Genau das, was der Compose-Commit-Baum
/// physisch enthalten **muss** — die Glue prüft die Invariante **Baum = BOM** dagegen, damit kein
/// „Tag HEAD + Lüge im Manifest" entsteht.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StuecklistenPosten {
    /// `id` des Bausteins (z.B. `"kicad"`).
    pub baustein_id: String,
    /// Der Heimat-Bereich, den dieser Posten stellt (z.B. `"elektronik"`).
    pub heimat: String,
    /// Der dauerhafte Release-Tag des gewählten Stands (z.B. `freigabe/elektronik/Rev-B`).
    pub release_tag: String,
}

/// Die vollständige **Compose-Spezifikation**, die der Kern liefert: die Baum-Spezifikation (welcher
/// Heimat-Teilbaum aus welchem Tag) **plus** die daraus folgende Produkt-Stückliste. Beide tragen
/// **dieselben** Auswahlen — die BOM ist nicht ein zweites, unabhängiges Manifest, sondern die
/// nach Baustein gelesene Sicht derselben Wahrheit; deshalb deckt sich der gebaute Baum mit ihr.
///
/// Die `parents` sind die Eltern-Stände des mehrelterigen Compose-Commits (`parents: Vec` im
/// Graph-Modell, E51c): einer pro gewähltem Release-Tag, in Auswahl-Reihenfolge, dublettenfrei. So
/// trägt der Multi-Parent-Graph die Compose-Knoten, und jeder beitragende Stand bleibt erreichbar.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComposeSpezifikation {
    /// Die Baum-Spezifikation, ein Eintrag pro Heimat-Bereich, in stabiler (nach Heimat sortierter)
    /// Reihenfolge, damit der gebaute Baum deterministisch ist.
    pub baum: Vec<ComposeEintrag>,
    /// Die Produkt-Stückliste — derselbe Satz Auswahlen, nach Baustein gelesen (gleiche Reihenfolge
    /// wie `baum`).
    pub stueckliste: Vec<StuecklistenPosten>,
    /// Die Eltern-Stände des Compose-Commits (mehrelterig, E51c): die gewählten Release-Tags in
    /// Auswahl-Reihenfolge, ohne Dubletten. Die Glue löst jeden Tag in seinen Commit auf und
    /// übergibt sie als `-p` an `commit-tree`.
    pub parents: Vec<String>,
}

// ----------------------------------------------------------------------------------------------
// Der Fehler — eine Auswahl, die keine eindeutige Komposition ergibt
// ----------------------------------------------------------------------------------------------

/// Warum eine Auswahl **keine** eindeutige Produkt-Komposition ergibt (Issue #139). Der Compose-Kern
/// ist rein und entscheidet hart: eine mehrdeutige Auswahl wird **nie** still „irgendwie" komponiert,
/// sondern als benannter Fehler zurückgegeben — sonst könnte zweierlei Tag denselben Ordner stellen
/// und die Garantie „jeder Heimat-Teilbaum aus genau einem Tag" zerbräche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeFehler {
    /// Es wurde gar nichts gewählt — eine Produkt-Revision braucht mindestens einen Baustein.
    KeineAuswahl,
    /// Eine Auswahl trägt einen leeren Heimat-Pfad oder einen leeren Release-Tag — beides ist nötig,
    /// um zu wissen, welcher Teilbaum aus welchem Stand kommt.
    LeeresFeld { baustein_id: String },
    /// Zwei Auswahlen zeigen auf **denselben** Heimat-Pfad — dann wäre nicht eindeutig, wessen Tag
    /// den Teilbaum stellt. (Trägt den kollidierenden Pfad.)
    DoppelteHeimat { heimat: String },
    /// Ein Heimat-Pfad **enthält** einen anderen (z.B. `elektronik` und `elektronik/regler`) — dann
    /// überdeckten sich zwei Teilbäume und ein Ordner käme aus zwei Tags. (Trägt beide Pfade.)
    VerschachtelteHeimat { aussen: String, innen: String },
}

impl ComposeFehler {
    /// Die deutsche Domänen-Meldung für die UI/das Protokoll. Nennt den Bereich beim Heimat-Namen,
    /// nie einen git-Begriff.
    pub fn meldung(&self) -> String {
        match self {
            ComposeFehler::KeineAuswahl => {
                "Eine Produkt-Revision braucht mindestens einen freigegebenen Baustein".to_string()
            }
            ComposeFehler::LeeresFeld { baustein_id } => {
                format!("Baustein {baustein_id}: Heimat und Freigabe-Stand dürfen nicht leer sein")
            }
            ComposeFehler::DoppelteHeimat { heimat } => {
                format!("Der Bereich {heimat} ist doppelt gewählt — er kann nur aus einem Stand kommen")
            }
            ComposeFehler::VerschachtelteHeimat { aussen, innen } => format!(
                "Die Bereiche {aussen} und {innen} überschneiden sich — ein Ordner kann nicht aus zwei Ständen kommen"
            ),
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Der Compose-Kern — rein, total, deterministisch
// ----------------------------------------------------------------------------------------------

/// Den **Compose-Kern** anwenden (Issue #139, E51c/E52): aus der Auswahl `{Baustein → (Heimat,
/// Release-Tag)}` die [`ComposeSpezifikation`] entscheiden — die Baum-Spezifikation (welcher
/// Heimat-Teilbaum aus welchem Tag), die Produkt-Stückliste und die mehrelterigen Compose-Eltern.
/// **Rein, total, deterministisch.** Macht **kein** I/O — die Glue löst die Tags auf und baut den
/// Baum.
///
/// Die eine Garantie: **jeder Heimat-Teilbaum kommt aus genau einem gewählten Tag**. Darum lehnt der
/// Kern jede Auswahl ab, die das verletzt — ein doppelter Heimat-Pfad
/// ([`ComposeFehler::DoppelteHeimat`]) oder zwei verschachtelte Pfade
/// ([`ComposeFehler::VerschachtelteHeimat`]) —, statt still „irgendwie" zu komponieren. Eine leere
/// Auswahl ([`ComposeFehler::KeineAuswahl`]) oder ein leeres Feld
/// ([`ComposeFehler::LeeresFeld`]) ist ebenso ein benannter Fehler.
///
/// Reihenfolge: die Baum-Spezifikation und die Stückliste sind **nach Heimat sortiert** (stabil),
/// damit der gebaute Baum unabhängig von der Eingabe-Reihenfolge deterministisch ist; die
/// `parents` behalten die **Auswahl-Reihenfolge** (dublettenfrei), denn die erste Wahl ist der
/// natürliche erste Eltern-Stand des Compose-Commits.
pub fn compose(wahlen: &[BausteinWahl]) -> Result<ComposeSpezifikation, ComposeFehler> {
    if wahlen.is_empty() {
        return Err(ComposeFehler::KeineAuswahl);
    }

    // 1) Jede Auswahl normalisieren und auf leere Felder prüfen.
    let mut normalisiert: Vec<BausteinWahl> = Vec::with_capacity(wahlen.len());
    for w in wahlen {
        let heimat = normalize_heimat(&w.heimat);
        let release_tag = w.release_tag.trim().to_string();
        if heimat.is_empty() || release_tag.is_empty() {
            return Err(ComposeFehler::LeeresFeld { baustein_id: w.baustein_id.trim().to_string() });
        }
        normalisiert.push(BausteinWahl {
            baustein_id: w.baustein_id.trim().to_string(),
            heimat,
            release_tag,
        });
    }

    // 2) Eindeutigkeit der Heimat-Bereiche prüfen: kein Pfad doppelt, keiner im anderen verschachtelt
    //    — sonst käme ein Ordner aus zwei Tags und die Garantie „ein Teilbaum, ein Tag" zerbräche.
    for (i, a) in normalisiert.iter().enumerate() {
        for b in &normalisiert[i + 1..] {
            if a.heimat == b.heimat {
                return Err(ComposeFehler::DoppelteHeimat { heimat: a.heimat.clone() });
            }
            if let Some((aussen, innen)) = nesting(&a.heimat, &b.heimat) {
                return Err(ComposeFehler::VerschachtelteHeimat {
                    aussen: aussen.to_string(),
                    innen: innen.to_string(),
                });
            }
        }
    }

    // 3) Die Eltern-Stände in Auswahl-Reihenfolge sammeln, Dubletten entfernen (zwei Bausteine
    //    könnten denselben Tag teilen — der Compose-Commit braucht ihn dann nur einmal als Eltern).
    let mut parents: Vec<String> = Vec::with_capacity(normalisiert.len());
    for w in &normalisiert {
        if !parents.contains(&w.release_tag) {
            parents.push(w.release_tag.clone());
        }
    }

    // 4) Baum-Spezifikation + Stückliste nach Heimat sortiert aufbauen (deterministischer Baum). Eine
    //    BTreeMap sortiert die Heimat-Schlüssel stabil; doppelte Heimaten sind oben ausgeschlossen.
    let nach_heimat: BTreeMap<&str, &BausteinWahl> =
        normalisiert.iter().map(|w| (w.heimat.as_str(), w)).collect();

    let baum: Vec<ComposeEintrag> = nach_heimat
        .values()
        .map(|w| ComposeEintrag {
            heimat: w.heimat.clone(),
            release_tag: w.release_tag.clone(),
            baustein_id: w.baustein_id.clone(),
        })
        .collect();

    let stueckliste: Vec<StuecklistenPosten> = nach_heimat
        .values()
        .map(|w| StuecklistenPosten {
            baustein_id: w.baustein_id.clone(),
            heimat: w.heimat.clone(),
            release_tag: w.release_tag.clone(),
        })
        .collect();

    Ok(ComposeSpezifikation { baum, stueckliste, parents })
}

/// Einen Heimat-Pfad normalisieren: trimmen, Rück- zu Vorwärts-Schrägstrichen, führende/abschließende
/// `/` entfernen, Mehrfach-`/` zusammenziehen. Rein/total — derselbe Bereich ergibt immer denselben
/// Schlüssel, sodass die Eindeutigkeits- und Verschachtelungs-Prüfung verlässlich greift.
fn normalize_heimat(heimat: &str) -> String {
    heimat
        .trim()
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Ob zwei (normalisierte) Heimat-Pfade ineinander verschachtelt sind — einer ist ein Pfad-Präfix
/// des anderen auf Segment-Grenze (`elektronik` enthält `elektronik/regler`, aber **nicht**
/// `elektronik-alt`). Liefert `(außen, innen)`, falls verschachtelt, sonst `None`. Rein/total.
fn nesting<'a>(a: &'a str, b: &'a str) -> Option<(&'a str, &'a str)> {
    fn enthaelt(aussen: &str, innen: &str) -> bool {
        innen.len() > aussen.len()
            && innen.starts_with(aussen)
            && innen.as_bytes()[aussen.len()] == b'/'
    }
    if enthaelt(a, b) {
        Some((a, b))
    } else if enthaelt(b, a) {
        Some((b, a))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wahl(id: &str, heimat: &str, tag: &str) -> BausteinWahl {
        BausteinWahl {
            baustein_id: id.to_string(),
            heimat: heimat.to_string(),
            release_tag: tag.to_string(),
        }
    }

    /// AC (Tabellentest): eine Auswahl von Baustein-Tags → die erwartete Baum-Komposition + BOM.
    /// „Neue PCB + alte Firmware" wird **ein** Produkt-Stand, dessen Baum exakt diese zwei Bereiche
    /// aus exakt diesen zwei freigegebenen Ständen stellt (Baum = BOM).
    #[test]
    fn auswahl_ergibt_erwarteten_baum_und_bom() {
        // table: (name, Auswahl, erwartete (heimat, tag)-Paare nach Heimat sortiert)
        let cases: &[(&str, Vec<BausteinWahl>, Vec<(&str, &str, &str)>)] = &[
            (
                "neue PCB + alte Firmware -> ein Stand, zwei Bereiche",
                vec![
                    wahl("kicad", "elektronik", "freigabe/elektronik/Rev-B"),
                    wahl("zephyr", "firmware", "freigabe/firmware/v0.3"),
                ],
                vec![
                    ("elektronik", "freigabe/elektronik/Rev-B", "kicad"),
                    ("firmware", "freigabe/firmware/v0.3", "zephyr"),
                ],
            ),
            (
                "Eingabe-Reihenfolge ändert den (nach Heimat sortierten) Baum nicht",
                vec![
                    wahl("zephyr", "firmware", "freigabe/firmware/v0.3"),
                    wahl("kicad", "elektronik", "freigabe/elektronik/Rev-B"),
                    wahl("fusion", "mechanik", "freigabe/mechanik/v1.0"),
                ],
                vec![
                    ("elektronik", "freigabe/elektronik/Rev-B", "kicad"),
                    ("firmware", "freigabe/firmware/v0.3", "zephyr"),
                    ("mechanik", "freigabe/mechanik/v1.0", "fusion"),
                ],
            ),
            (
                "ein einzelner Baustein komponiert sauber zu einem Bereich",
                vec![wahl("kicad", "elektronik", "freigabe/elektronik/Rev-B")],
                vec![("elektronik", "freigabe/elektronik/Rev-B", "kicad")],
            ),
        ];

        for (name, wahlen, erwartet) in cases {
            let spec = compose(wahlen).unwrap_or_else(|e| panic!("{name}: {e:?}"));

            // Baum-Spezifikation deckt sich Eintrag für Eintrag mit der Erwartung.
            let baum: Vec<(&str, &str, &str)> = spec
                .baum
                .iter()
                .map(|e| (e.heimat.as_str(), e.release_tag.as_str(), e.baustein_id.as_str()))
                .collect();
            assert_eq!(&baum, erwartet, "Baum für {name}");

            // Die BOM trägt denselben Satz Auswahlen, nach Baustein gelesen (Baum = BOM).
            let bom: Vec<(&str, &str, &str)> = spec
                .stueckliste
                .iter()
                .map(|p| (p.heimat.as_str(), p.release_tag.as_str(), p.baustein_id.as_str()))
                .collect();
            assert_eq!(&bom, erwartet, "BOM für {name}");
        }
    }

    /// Die Compose-Eltern (mehrelterig, E51c) sind die gewählten Tags in Auswahl-Reihenfolge,
    /// dublettenfrei — einer pro beitragendem Stand, sodass `parents: Vec` den Compose-Knoten trägt.
    #[test]
    fn parents_sind_die_gewaehlten_tags_in_auswahlreihenfolge() {
        let spec = compose(&[
            wahl("zephyr", "firmware", "freigabe/firmware/v0.3"),
            wahl("kicad", "elektronik", "freigabe/elektronik/Rev-B"),
        ])
        .unwrap();
        // Auswahl-Reihenfolge bleibt (firmware zuerst gewählt), unabhängig von der Baum-Sortierung.
        assert_eq!(
            spec.parents,
            vec!["freigabe/firmware/v0.3".to_string(), "freigabe/elektronik/Rev-B".to_string()]
        );
    }

    /// Teilen sich zwei Bausteine denselben Release-Tag (selber Stand stellt zwei Bereiche), taucht
    /// er nur **einmal** als Eltern-Stand auf — ein Compose-Commit braucht jeden Eltern nur einmal.
    #[test]
    fn geteilter_tag_ist_nur_ein_elternstand() {
        let spec = compose(&[
            wahl("a", "doc/datenblatt", "freigabe/doc/v2"),
            wahl("b", "doc/schaltplan", "freigabe/doc/v2"),
        ])
        .unwrap();
        assert_eq!(spec.parents, vec!["freigabe/doc/v2".to_string()], "geteilter Tag nur einmal");
        // Beide Bereiche stehen dennoch eigenständig im Baum/der BOM.
        assert_eq!(spec.baum.len(), 2);
        assert_eq!(spec.stueckliste.len(), 2);
    }

    /// AC (Property): über jede zulässige Auswahl kommt **jeder** Heimat-Teilbaum aus **genau einem**
    /// gewählten Tag — kein Heimat-Pfad erscheint zweimal in der Baum-Spezifikation, und der Tag jedes
    /// Eintrags ist einer der gewählten. Erschöpfend über eine Reihe von Eingabe-Permutationen.
    #[test]
    fn property_jeder_teilbaum_aus_genau_einem_tag() {
        let bereiche = [
            ("kicad", "elektronik", "freigabe/elektronik/Rev-B"),
            ("zephyr", "firmware", "freigabe/firmware/v0.3"),
            ("fusion", "mechanik", "freigabe/mechanik/v1.0"),
            ("doc", "doku", "freigabe/doku/2026"),
        ];

        // Über jede nicht-leere Teilmenge der disjunkten Bereiche, in mehreren Reihenfolgen.
        for mask in 1u8..(1 << bereiche.len()) {
            let mut gewaehlt: Vec<BausteinWahl> = Vec::new();
            for (i, (id, h, t)) in bereiche.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    gewaehlt.push(wahl(id, h, t));
                }
            }
            // Vorwärts und rückwärts gelesen — die Reihenfolge darf die Eindeutigkeit nicht ändern.
            for variante in [gewaehlt.clone(), {
                let mut r = gewaehlt.clone();
                r.reverse();
                r
            }] {
                let spec = compose(&variante).expect("disjunkte Bereiche komponieren");

                // Jeder Heimat-Pfad erscheint genau einmal.
                let mut heimaten: Vec<&str> = spec.baum.iter().map(|e| e.heimat.as_str()).collect();
                let vorher = heimaten.len();
                heimaten.sort_unstable();
                heimaten.dedup();
                assert_eq!(heimaten.len(), vorher, "jeder Heimat-Teilbaum genau einmal: {spec:?}");

                // Der Tag jedes Eintrags ist genau einer der gewählten Tags (kein erfundener Stand).
                for e in &spec.baum {
                    assert!(
                        variante.iter().any(|w| w.release_tag == e.release_tag),
                        "Teilbaum {} aus einem gewählten Tag: {}",
                        e.heimat,
                        e.release_tag
                    );
                }
                // Baum und BOM tragen exakt dieselben (heimat, tag)-Paare — Baum = BOM.
                let baum: Vec<(&str, &str)> =
                    spec.baum.iter().map(|e| (e.heimat.as_str(), e.release_tag.as_str())).collect();
                let bom: Vec<(&str, &str)> = spec
                    .stueckliste
                    .iter()
                    .map(|p| (p.heimat.as_str(), p.release_tag.as_str()))
                    .collect();
                assert_eq!(baum, bom, "Baum deckt sich mit BOM: {spec:?}");
            }
        }
    }

    /// Eine leere Auswahl ist ein benannter Fehler — eine Produkt-Revision braucht mindestens einen
    /// freigegebenen Baustein, nie ein still leeres Produkt.
    #[test]
    fn leere_auswahl_ist_fehler() {
        assert_eq!(compose(&[]), Err(ComposeFehler::KeineAuswahl));
    }

    /// Ein leeres Heimat-Feld oder ein leerer Release-Tag wird benannt abgelehnt — ohne beides ist
    /// nicht entscheidbar, welcher Teilbaum aus welchem Stand kommt.
    #[test]
    fn leeres_feld_ist_fehler() {
        assert_eq!(
            compose(&[wahl("kicad", "  ", "freigabe/elektronik/Rev-B")]),
            Err(ComposeFehler::LeeresFeld { baustein_id: "kicad".into() })
        );
        assert_eq!(
            compose(&[wahl("kicad", "elektronik", "   ")]),
            Err(ComposeFehler::LeeresFeld { baustein_id: "kicad".into() })
        );
    }

    /// Zwei Auswahlen auf denselben Heimat-Pfad sind mehrdeutig (welcher Tag stellt den Teilbaum?) —
    /// der Kern lehnt das ab, statt still einen zu wählen. Auch nach Normalisierung (`elektronik/`
    /// == `elektronik`).
    #[test]
    fn doppelte_heimat_ist_fehler() {
        assert_eq!(
            compose(&[
                wahl("kicad", "elektronik", "freigabe/elektronik/Rev-A"),
                wahl("kicad2", "elektronik/", "freigabe/elektronik/Rev-B"),
            ]),
            Err(ComposeFehler::DoppelteHeimat { heimat: "elektronik".into() })
        );
    }

    /// Ein Heimat-Pfad, der einen anderen enthält, überdeckte zwei Teilbäume — abgelehnt. Ein bloß
    /// gemeinsames Namens-Präfix ohne Segment-Grenze (`elektronik` vs. `elektronik-alt`) ist dagegen
    /// **keine** Verschachtelung und bleibt zulässig.
    #[test]
    fn verschachtelte_heimat_ist_fehler_aber_praefix_namen_nicht() {
        let err = compose(&[
            wahl("a", "elektronik", "freigabe/elektronik/Rev-B"),
            wahl("b", "elektronik/regler", "freigabe/elektronik-regler/v1"),
        ])
        .unwrap_err();
        assert_eq!(
            err,
            ComposeFehler::VerschachtelteHeimat {
                aussen: "elektronik".into(),
                innen: "elektronik/regler".into(),
            }
        );

        // Gleiches Namens-Präfix, aber keine Pfad-Verschachtelung -> zulässig.
        let ok = compose(&[
            wahl("a", "elektronik", "freigabe/elektronik/Rev-B"),
            wahl("b", "elektronik-alt", "freigabe/elektronik-alt/v1"),
        ]);
        assert!(ok.is_ok(), "ein gemeinsames Namens-Präfix ist keine Verschachtelung: {ok:?}");
    }

    /// `normalize_heimat` ist rein/total: trimmt, eint Schrägstriche, zieht Mehrfach-`/` zusammen.
    #[test]
    fn normalize_heimat_table() {
        let cases: &[(&str, &str)] = &[
            ("elektronik", "elektronik"),
            ("  elektronik  ", "elektronik"),
            ("elektronik/", "elektronik"),
            ("/elektronik/regler/", "elektronik/regler"),
            ("elektronik//regler", "elektronik/regler"),
            ("mechanik\\gehaeuse", "mechanik/gehaeuse"),
            ("", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize_heimat(input), *expected, "input = {input:?}");
        }
    }

    /// `compose` ist total — entweder genau eine Spezifikation oder genau ein benannter Fehler, nie
    /// ein Panic, auch über entartete Eingaben.
    #[test]
    fn compose_ist_total() {
        let inputs: Vec<Vec<BausteinWahl>> = vec![
            vec![],
            vec![wahl("", "", "")],
            vec![wahl("a", "x", "t1"), wahl("b", "x", "t2")],
            vec![wahl("a", "x", "t"), wahl("b", "y", "t")],
            vec![wahl("a", "x/y", "t1"), wahl("b", "x", "t2")],
        ];
        for input in &inputs {
            // Darf nicht panicken; Ok XOR Err ist durch den Typ garantiert — wir rufen es nur auf.
            let _ = compose(input);
        }
    }

    /// Jede Fehler-Meldung ist deutscher Domänentext (kein git-Wort, nie leer).
    #[test]
    fn fehler_meldungen_sind_domaenentext() {
        let fehler = [
            ComposeFehler::KeineAuswahl,
            ComposeFehler::LeeresFeld { baustein_id: "kicad".into() },
            ComposeFehler::DoppelteHeimat { heimat: "elektronik".into() },
            ComposeFehler::VerschachtelteHeimat {
                aussen: "elektronik".into(),
                innen: "elektronik/regler".into(),
            },
        ];
        for f in fehler {
            let m = f.meldung();
            assert!(!m.is_empty(), "Meldung nicht leer: {f:?}");
            for git_wort in ["commit", "tree", "read-tree", "tag", "HEAD", "git"] {
                assert!(!m.contains(git_wort), "Meldung trägt kein git-Wort ({git_wort}): {m}");
            }
        }
    }
}
