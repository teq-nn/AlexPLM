//! Der **Zusammenstellungs-Kern** — die reine Entscheidung über die **Vollständigkeit** einer
//! Produkt-Revision und ihren **Checklisten-Zustand** (Issue #140, E52a).
//!
//! Folgt dem Haus-Muster (`freigabegate.rs`, `aufgabenblock.rs`, `compose.rs`): **eine reine,
//! totale, deterministische** Funktion über einen schlichten Schnappschuss. Sie kennt **kein** git,
//! keine Uhr, keinen Prozess — das Sammeln der verfügbaren Freigabe-Stände je Baustein und der
//! aktuellen Auswahl lebt in der dünnen Glue ([`crate::zusammenstellungglue`]); dieses Modul
//! entscheidet **ausschließlich**, ob die Zusammenstellung vollständig ist, und wie die Checkliste
//! Baustein für Baustein aussieht.
//!
//! ## Was eine Produkt-Revision vollständig macht (E52a)
//!
//! Jeder **Baustein** reift für sich (E51) und stellt seinen Heimat-Bereich aus genau einem
//! freigegebenen Stand bei — dem **Compose-Kern** (E51c, #139) übergibt die Glue diese Auswahl, der
//! daraus den reproduzierbaren Gesamt-Stand baut. Bevor überhaupt komponiert werden kann, fragt der
//! Zusammenstellungs-Kern: **trägt jeder verpflichtende Baustein einen Beitrag?** Ein Beitrag ist
//! entweder ein **frischer Freigabe-Stand** für diese Produkt-Revision **oder** das **bewusste
//! „alter Stand reicht"** (den schon freigegebenen Vorstand mitnehmen). Beides zählt gleich — die
//! Mitnahme ist eine **ausdrückliche** Koordinations-Geste, kein stilles Auslassen.
//!
//! Die eine Regel, in einem Atemzug: **jeder Pflicht-Baustein braucht einen Beitrag**
//! (frisch **oder** „Vorstand mitnehmen"), **optionale** Bausteine blockieren **nie**. Erst wenn
//! kein Pflicht-Baustein mehr ausstehend ist, ist die Produkt-Revision **vollständig**.
//!
//! ## Keine Rollen, keine Rechte (E52a)
//!
//! Ein Beitrag ist **Koordination, keine Autorisierung**: der Kern fragt nur, *ob* ein Bereich
//! beigetragen hat, nie *wer* dazu berechtigt ist. Es gibt **keine** Rollen-/Rechte-Schicht — die
//! Checkliste ist eine geteilte Sicht auf den Reifestand, kein Freigabe-Gate für Personen.
//!
//! ## Die Checkliste (E52a)
//!
//! Der Kern liefert pro Baustein eine Zeile mit ihrem [`PostenZustand`] — die UI rendert daraus
//! „elektronik ✓ Rev B · firmware ⧖ ausstehend", ohne selbst noch etwas zu entscheiden.

use serde::Serialize;

// ----------------------------------------------------------------------------------------------
// Eingabe — pro Baustein: Pflicht/Optional, verfügbare Stände, aktuelle Auswahl
// ----------------------------------------------------------------------------------------------

/// Die **aktuelle Auswahl** für einen Baustein in der zusammenzustellenden Produkt-Revision (E52a).
/// Sie ist der einzige veränderliche Teil der Eingabe — Pflicht/Optional und die verfügbaren Stände
/// liegen fest, die Auswahl wandert, während der Nutzer die Revision schnürt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auswahl {
    /// Noch nichts gewählt — der Bereich steht aus. Bei einem Pflicht-Baustein blockiert das die
    /// Vollständigkeit; bei einem optionalen nie.
    Offen,
    /// Ein **frischer Freigabe-Stand** wurde für diese Produkt-Revision gewählt (sein Release-Tag).
    /// Ein vollwertiger Beitrag.
    FrischerStand { release_tag: String },
    /// **„Alter Stand reicht"** — der schon freigegebene Vorstand wird bewusst mitgenommen (sein
    /// Release-Tag). Ein **ausdrücklicher** Beitrag, gleichwertig zum frischen Stand (E52a).
    VorstandMitnehmen { release_tag: String },
}

impl Auswahl {
    /// Der Release-Tag des Beitrags, falls einer gewählt ist — sonst `None` (Auswahl offen).
    fn release_tag(&self) -> Option<&str> {
        match self {
            Auswahl::FrischerStand { release_tag } | Auswahl::VorstandMitnehmen { release_tag } => {
                Some(release_tag.as_str())
            }
            Auswahl::Offen => None,
        }
    }
}

/// Ein **Baustein-Eintrag** der Zusammenstellung (E52a): ob er verpflichtend ist, welche
/// freigegebenen Stände für seinen Bereich verfügbar sind und was gerade gewählt ist. Schlichte
/// Daten — die Glue füllt `verfuegbare_staende` aus den dauerhaften Baustein-Freigabe-Tags (E51a)
/// und `auswahl` aus der laufenden Schnür-Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BausteinEintrag {
    /// `id` des Bausteins (z.B. `"kicad"`), fürs Protokoll und die Checklisten-Lesbarkeit.
    pub baustein_id: String,
    /// Der Heimat-Bereich, den dieser Baustein im Produkt stellt (z.B. `"elektronik"`).
    pub heimat: String,
    /// Ob dieser Baustein **verpflichtend** ist. Ein Pflicht-Baustein ohne Beitrag blockiert die
    /// Vollständigkeit; ein **optionaler** blockiert nie (E52a).
    pub pflicht: bool,
    /// Die für diesen Bereich verfügbaren freigegebenen Stände (ihre Release-Tags, E51a), neuester
    /// zuerst wie von der Glue geliefert. Leer ⇒ es gibt noch nichts mitzunehmen.
    pub verfuegbare_staende: Vec<String>,
    /// Die aktuelle Auswahl für diesen Baustein.
    pub auswahl: Auswahl,
}

// ----------------------------------------------------------------------------------------------
// Ausgabe — der Checklisten-Zustand je Baustein + die Gesamt-Vollständigkeit
// ----------------------------------------------------------------------------------------------

/// Der **Zustand eines Checklisten-Postens** (E52a) — die eine Achse, die die UI als „✓ Rev B" /
/// „⧖ ausstehend" rendert. Genau einer pro Baustein; total.
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostenZustand {
    /// Ein **frischer** Freigabe-Stand wurde für diese Produkt-Revision beigetragen („✓ Rev B").
    Beigetragen,
    /// Der **Vorstand wird mitgenommen** — bewusst „alter Stand reicht" („✓ Vorstand"). Ein
    /// vollwertiger Beitrag, getrennt benannt, damit die Checkliste die Geste sichtbar macht.
    VorstandMitgenommen,
    /// Ein **verpflichtender** Bereich **ohne** Beitrag — er steht aus und hält die Vollständigkeit
    /// („⧖ ausstehend"). Nur ein Pflicht-Baustein kann diesen Zustand tragen.
    Ausstehend,
    /// Ein **optionaler** Bereich ohne Beitrag — er ist nicht gewählt, blockiert aber nie
    /// („– nicht dabei"). Optional & offen ergibt **nie** `Ausstehend` (E52a).
    OptionalOffen,
}

impl PostenZustand {
    /// Ob dieser Posten die Vollständigkeit **hält** — nur ein ausstehender Pflicht-Bereich tut das.
    /// Ein Beitrag (frisch/Vorstand) und ein offener optionaler Bereich halten nie auf. Rein/total.
    fn blockiert(&self) -> bool {
        matches!(self, PostenZustand::Ausstehend)
    }
}

/// Ein **Checklisten-Posten** je Baustein (E52a). Trägt alles, was die UI für eine Zeile braucht,
/// ohne neu zu entscheiden: den Bereich, ob er Pflicht ist, seinen Zustand und — falls ein Beitrag
/// gewählt ist — den dazugehörigen Release-Tag (das menschliche „Rev B" der Checkliste).
// Serialize-only (ein berechneter Zustand, nie zurückgelesen) — die Feldnamen bleiben snake_case,
// genau wie das Frontend sie liest; specta pinnt sie über die Naht.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChecklistenPosten {
    /// `id` des Bausteins (z.B. `"kicad"`).
    pub baustein_id: String,
    /// Der Heimat-Bereich, den dieser Posten stellt (z.B. `"elektronik"`).
    pub heimat: String,
    /// Ob dieser Bereich verpflichtend ist (für die Anzeige „Pflicht"/„optional").
    pub pflicht: bool,
    /// Der Checklisten-Zustand dieses Bereichs.
    pub zustand: PostenZustand,
    /// Der Release-Tag des Beitrags (z.B. `freigabe/elektronik/Rev-B`), falls einer gewählt ist;
    /// sonst leer. Die UI zeigt daraus das menschliche „Rev B".
    pub release_tag: String,
}

/// Der **Zusammenstellungs-Bericht** einer Produkt-Revision (E52a): die Checkliste (ein Posten pro
/// Baustein, Eingabe-Reihenfolge erhalten) plus die daraus folgende **Vollständigkeit**. Total —
/// die UI rendert daraus die ganze Checkliste, ohne neu zu entscheiden.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZusammenstellungsBericht {
    /// Die Checkliste, ein Posten pro Baustein, in **Eingabe-Reihenfolge** (die Glue liefert sie in
    /// der natürlichen Stack-Reihenfolge; der Kern erhält sie stabil).
    pub posten: Vec<ChecklistenPosten>,
    /// Ob die Produkt-Revision **vollständig** ist: **jeder** Pflicht-Baustein trägt einen Beitrag.
    /// Optionale Bausteine zählen hier nie hinein (E52a).
    pub vollstaendig: bool,
    /// Die Bereiche (Heimat-Pfade), die noch **ausstehen** — verpflichtend und ohne Beitrag. Leer
    /// ⇔ vollständig. Die UI nennt sie beim Heimat-Namen, damit klar ist, *was* noch fehlt.
    pub ausstehende: Vec<String>,
}

// ----------------------------------------------------------------------------------------------
// Der Zusammenstellungs-Kern — rein, total, deterministisch
// ----------------------------------------------------------------------------------------------

/// Den **Zusammenstellungs-Kern** anwenden (Issue #140, E52a): aus den Baustein-Einträgen
/// (Pflicht/Optional × verfügbare Stände × aktuelle Auswahl) den [`ZusammenstellungsBericht`]
/// entscheiden — die Checkliste je Baustein **und** die Gesamt-Vollständigkeit. **Rein, total,
/// deterministisch.** Macht **kein** I/O; kennt **keine** Rollen/Rechte (ein Beitrag ist
/// Koordination, keine Autorisierung — E52a).
///
/// Die Regel, in einem Atemzug: jeder Posten bekommt seinen Zustand aus (Pflicht?, Auswahl) —
/// frisch/Vorstand ⇒ Beitrag, ein Pflicht-Bereich ohne Beitrag ⇒ `Ausstehend`, ein optionaler ohne
/// Beitrag ⇒ `OptionalOffen` (blockiert nie). Vollständig ist die Revision genau dann, wenn **kein**
/// Posten `Ausstehend` ist. Die Reihenfolge der Eingabe bleibt erhalten (stabile Checkliste).
pub fn zusammenstellen(eintraege: &[BausteinEintrag]) -> ZusammenstellungsBericht {
    let mut posten: Vec<ChecklistenPosten> = Vec::with_capacity(eintraege.len());

    for e in eintraege {
        // Der Zustand folgt allein aus (Pflicht?, ist die Auswahl ein Beitrag, welche Art Beitrag):
        let zustand = match &e.auswahl {
            // Ein frischer Freigabe-Stand ist der volle Beitrag.
            Auswahl::FrischerStand { .. } => PostenZustand::Beigetragen,
            // „Vorstand mitnehmen" zählt gleich — getrennt benannt, damit die Geste sichtbar bleibt.
            Auswahl::VorstandMitnehmen { .. } => PostenZustand::VorstandMitgenommen,
            // Kein Beitrag: ein Pflicht-Bereich steht aus, ein optionaler ist bloß nicht dabei.
            Auswahl::Offen => {
                if e.pflicht {
                    PostenZustand::Ausstehend
                } else {
                    PostenZustand::OptionalOffen
                }
            }
        };

        posten.push(ChecklistenPosten {
            baustein_id: e.baustein_id.clone(),
            heimat: e.heimat.clone(),
            pflicht: e.pflicht,
            zustand,
            // Der Release-Tag des Beitrags (frisch/Vorstand); leer, wenn nichts gewählt ist.
            release_tag: e.auswahl.release_tag().unwrap_or("").to_string(),
        });
    }

    // Vollständig ⇔ kein Posten hält auf. Nur ein ausstehender Pflicht-Bereich tut das (E52a) —
    // ein optionaler offener Bereich nie.
    let ausstehende: Vec<String> = posten
        .iter()
        .filter(|p| p.zustand.blockiert())
        .map(|p| p.heimat.clone())
        .collect();
    let vollstaendig = ausstehende.is_empty();

    ZusammenstellungsBericht { posten, vollstaendig, ausstehende }
}

// ----------------------------------------------------------------------------------------------
// Cold-Start: die initiale Seed-Liste — rein, total, deterministisch (Issue #142, E52b)
// ----------------------------------------------------------------------------------------------

/// Ein **Cold-Start-Seed-Posten** (E52b): ein verpflichtender Baustein, der **noch keinen einzigen**
/// freigegebenen Stand trägt — er braucht beim **allerersten** Produkt-Release eine **initiale**
/// Revision aus dem aktuellen Stand, sonst hält er die erste Produkt-Revision auf, bevor sie
/// überhaupt komponierbar ist. Schlichte Daten — die Glue ([`crate::zusammenstellungglue`]) setzt je
/// Posten in **einem** Akt eine initiale Baustein-Revision (E51a-Tag) aus dem aktuellen Stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedPosten {
    /// `id` des Bausteins (z.B. `"kicad"`), fürs Protokoll und die Lesbarkeit.
    pub baustein_id: String,
    /// Der Heimat-Bereich, der initial freigegeben wird (z.B. `"elektronik"`).
    pub heimat: String,
}

/// Die **Cold-Start-Seed-Liste** ableiten (Issue #142, E52b): aus **denselben** Baustein-Einträgen,
/// die der [`zusammenstellen`]-Kern liest (Pflicht/Optional × verfügbare Stände), genau die
/// **verpflichtenden** Bausteine herausfiltern, die **noch keinen einzigen** freigegebenen Stand
/// tragen (`verfuegbare_staende` leer). **Rein, total, deterministisch**; macht **kein** I/O — die
/// Auswahl spielt hier keine Rolle, denn der Seed sät den **ersten** Stand, bevor überhaupt gewählt
/// werden kann.
///
/// Das Cold-Start-Problem (E52b): auf dem **allerersten** Produkt-Release trägt **kein** Pflicht-
/// Baustein eine Revision — die erste Produkt-Revision wäre damit niemals vollständig, ohne dass der
/// Nutzer erst N Bausteine **manuell** freigibt. Statt N Handgriffe liefert der Kern diese Liste, und
/// **ein** Akt der Glue sät je Baustein eine initiale Revision aus dem aktuellen Stand. Danach trägt
/// jeder Pflicht-Baustein einen Stand, und die erste Produkt-Revision ist komponierbar.
///
/// Die Regel, in einem Atemzug: ein Baustein gehört genau dann in die Seed-Liste, wenn er
/// **verpflichtend** ist **und** seine `verfuegbare_staende` **leer** sind. Ein optionaler Baustein
/// wird nie gesät (er blockiert nie — E52a); ein Pflicht-Baustein, der **schon** einen Stand trägt,
/// braucht keine initiale Revision mehr. Die Reihenfolge der Eingabe bleibt erhalten.
pub fn kaltstart_seed_liste(eintraege: &[BausteinEintrag]) -> Vec<SeedPosten> {
    eintraege
        .iter()
        // Nur Pflicht-Bausteine **ohne** jeden freigegebenen Stand brauchen einen initialen Seed.
        // Optionale blockieren nie; schon revidierte Pflicht-Bausteine haben bereits einen Stand.
        .filter(|e| e.pflicht && e.verfuegbare_staende.is_empty())
        .map(|e| SeedPosten {
            baustein_id: e.baustein_id.clone(),
            heimat: e.heimat.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eintrag(id: &str, heimat: &str, pflicht: bool, auswahl: Auswahl) -> BausteinEintrag {
        BausteinEintrag {
            baustein_id: id.to_string(),
            heimat: heimat.to_string(),
            pflicht,
            verfuegbare_staende: Vec::new(),
            auswahl,
        }
    }
    fn frisch(tag: &str) -> Auswahl {
        Auswahl::FrischerStand { release_tag: tag.to_string() }
    }
    fn vorstand(tag: &str) -> Auswahl {
        Auswahl::VorstandMitnehmen { release_tag: tag.to_string() }
    }
    /// Wie [`eintrag`], aber mit gesetzten verfügbaren Ständen — für die Cold-Start-Seed-Tabelle, in
    /// der „schon revidiert?" allein an der Stände-Liste hängt (nicht an der Auswahl).
    fn eintrag_mit_staenden(
        id: &str,
        heimat: &str,
        pflicht: bool,
        staende: &[&str],
    ) -> BausteinEintrag {
        BausteinEintrag {
            baustein_id: id.to_string(),
            heimat: heimat.to_string(),
            pflicht,
            verfuegbare_staende: staende.iter().map(|s| s.to_string()).collect(),
            auswahl: Auswahl::Offen,
        }
    }

    /// **Die Kern-Akzeptanzmatrix** (AC): jede Kombination aus {Pflicht?, Beitrag?} → die richtige
    /// Vollständigkeit, in **einer** Tabelle bewiesen. Ein Pflicht-Bereich **ohne** Beitrag
    /// blockiert; ein optionaler **nie** — egal ob beigetragen oder nicht.
    #[test]
    fn vollstaendigkeit_pro_pflicht_x_beitrag() {
        // (name, pflicht, auswahl, erwarteter Zustand, blockiert die Vollständigkeit?)
        let cases: &[(&str, bool, Auswahl, PostenZustand, bool)] = &[
            // Pflicht mit frischem Beitrag → beigetragen, blockiert nicht.
            ("pflicht + frisch", true, frisch("freigabe/elektronik/Rev-B"), PostenZustand::Beigetragen, false),
            // Pflicht mit Vorstand-Mitnahme → vollwertiger Beitrag, blockiert nicht.
            ("pflicht + vorstand", true, vorstand("freigabe/firmware/v0.2"), PostenZustand::VorstandMitgenommen, false),
            // Pflicht OHNE Beitrag → ausstehend, blockiert.
            ("pflicht + offen", true, Auswahl::Offen, PostenZustand::Ausstehend, true),
            // Optional mit frischem Beitrag → beigetragen, blockiert nicht.
            ("optional + frisch", false, frisch("freigabe/doku/2026"), PostenZustand::Beigetragen, false),
            // Optional OHNE Beitrag → bloß nicht dabei, blockiert NIE.
            ("optional + offen", false, Auswahl::Offen, PostenZustand::OptionalOffen, false),
        ];

        for (name, pflicht, auswahl, erwarteter_zustand, blockiert) in cases {
            let bericht = zusammenstellen(&[eintrag("b", "bereich", *pflicht, auswahl.clone())]);
            assert_eq!(bericht.posten.len(), 1, "{name}");
            assert_eq!(bericht.posten[0].zustand, *erwarteter_zustand, "Zustand für {name}");
            // Ein einzelner blockierender Posten ⇒ die ganze Revision ist unvollständig.
            assert_eq!(bericht.vollstaendig, !*blockiert, "Vollständigkeit für {name}");
            assert_eq!(
                bericht.ausstehende.is_empty(),
                !*blockiert,
                "ausstehende-Liste für {name}"
            );
        }
    }

    /// **AC: ein Pflicht-Baustein ohne Beitrag blockiert, ein optionaler nie** — im Gemenge. „neue
    /// PCB + alte Firmware (mitgenommen)" ist vollständig; fehlt der Firmware-Beitrag, hält sie auf;
    /// ein offener optionaler Doku-Bereich ändert daran in keinem Fall etwas.
    #[test]
    fn pflicht_ohne_beitrag_blockiert_optional_nie() {
        // Beide Pflicht-Bereiche tragen bei (einer frisch, einer als Vorstand), Doku ist optional &
        // offen → vollständig, trotz offener Doku.
        let voll = zusammenstellen(&[
            eintrag("kicad", "elektronik", true, frisch("freigabe/elektronik/Rev-B")),
            eintrag("zephyr", "firmware", true, vorstand("freigabe/firmware/v0.2")),
            eintrag("doku", "doku", false, Auswahl::Offen),
        ]);
        assert!(voll.vollstaendig, "beide Pflicht beigetragen → vollständig: {voll:?}");
        assert!(voll.ausstehende.is_empty());
        // Der optionale offene Bereich ist gelistet, aber nicht ausstehend.
        let doku = voll.posten.iter().find(|p| p.heimat == "doku").unwrap();
        assert_eq!(doku.zustand, PostenZustand::OptionalOffen);

        // Fehlt der Firmware-Beitrag → genau dieser Pflicht-Bereich hält die Revision auf; der
        // optionale Doku-Bereich (immer noch offen) zählt nicht hinein.
        let unvoll = zusammenstellen(&[
            eintrag("kicad", "elektronik", true, frisch("freigabe/elektronik/Rev-B")),
            eintrag("zephyr", "firmware", true, Auswahl::Offen),
            eintrag("doku", "doku", false, Auswahl::Offen),
        ]);
        assert!(!unvoll.vollstaendig, "ein ausstehender Pflicht-Bereich → unvollständig");
        assert_eq!(
            unvoll.ausstehende,
            vec!["firmware".to_string()],
            "nur der ausstehende Pflicht-Bereich, nie der optionale"
        );
    }

    /// **AC: „Vorstand mitnehmen" zählt als ausdrücklicher Beitrag** — ein Pflicht-Bereich, dessen
    /// alter Stand bewusst mitgenommen wird, macht die Revision vollständig, genau wie ein frischer
    /// Stand. Die Mitnahme ist eine Geste, kein stilles Auslassen: sie trägt ihren Release-Tag und
    /// einen eigenen, sichtbaren Zustand.
    #[test]
    fn vorstand_mitnehmen_ist_ein_beitrag() {
        let bericht = zusammenstellen(&[eintrag(
            "zephyr",
            "firmware",
            true,
            vorstand("freigabe/firmware/v0.2"),
        )]);
        assert!(bericht.vollstaendig, "ein mitgenommener Vorstand vervollständigt einen Pflicht-Bereich");
        assert!(bericht.ausstehende.is_empty());
        let p = &bericht.posten[0];
        assert_eq!(p.zustand, PostenZustand::VorstandMitgenommen);
        assert_eq!(p.release_tag, "freigabe/firmware/v0.2", "der mitgenommene Stand ist benannt");
    }

    /// **AC: die Checkliste zeigt je Baustein den Zustand** (E52a-Beispiel) — „elektronik ✓ Rev B ·
    /// firmware ⧖ ausstehend". Der Kern liefert pro Bereich Zustand + Release-Tag, die UI rendert
    /// daraus die Zeile. Reihenfolge der Eingabe bleibt erhalten.
    #[test]
    fn checkliste_traegt_zustand_und_stand_je_baustein() {
        let bericht = zusammenstellen(&[
            eintrag("kicad", "elektronik", true, frisch("freigabe/elektronik/Rev-B")),
            eintrag("zephyr", "firmware", true, Auswahl::Offen),
        ]);
        // Eingabe-Reihenfolge erhalten: elektronik zuerst, firmware danach.
        let zeilen: Vec<(&str, PostenZustand, &str)> = bericht
            .posten
            .iter()
            .map(|p| (p.heimat.as_str(), p.zustand, p.release_tag.as_str()))
            .collect();
        assert_eq!(
            zeilen,
            vec![
                ("elektronik", PostenZustand::Beigetragen, "freigabe/elektronik/Rev-B"),
                ("firmware", PostenZustand::Ausstehend, ""),
            ],
            "die Checkliste trägt je Bereich Zustand + Stand"
        );
        assert!(!bericht.vollstaendig);
        assert_eq!(bericht.ausstehende, vec!["firmware".to_string()]);
    }

    /// **Keine Rollen/Rechte** (E52a): der Kern fragt nur, *ob* ein Bereich beigetragen hat — nie
    /// *wer* dazu berechtigt ist. Zwei Beiträge, völlig verschiedene Bausteine, derselbe Verdikt
    /// (vollständig) — es gibt keine Person/Rolle in der Eingabe und keine im Bericht. (Strukturell:
    /// die Typen tragen schlicht keine Autorisierungs-Achse.)
    #[test]
    fn kein_rollen_oder_rechte_layer() {
        let bericht = zusammenstellen(&[
            eintrag("kicad", "elektronik", true, frisch("freigabe/elektronik/Rev-B")),
            eintrag("zephyr", "firmware", true, frisch("freigabe/firmware/v0.3")),
        ]);
        assert!(bericht.vollstaendig, "zwei Beiträge → vollständig, ohne jede Rechte-Frage");
        // Der Bericht trägt nur Bereich/Zustand/Stand — keine Person, keine Rolle, kein Recht.
        for p in &bericht.posten {
            assert!(!p.baustein_id.is_empty());
            assert!(matches!(
                p.zustand,
                PostenZustand::Beigetragen | PostenZustand::VorstandMitgenommen
            ));
        }
    }

    /// **Total + deterministisch**: eine leere Zusammenstellung ist vollständig (kein Pflicht-Bereich
    /// hält auf — kein Auslassen aus dem Nichts), und dieselbe Eingabe ergibt immer denselben
    /// Bericht. Der Kern panickt über keine Eingabe.
    #[test]
    fn leer_ist_vollstaendig_und_deterministisch() {
        let leer = zusammenstellen(&[]);
        assert!(leer.vollstaendig, "ohne Pflicht-Bereiche gibt es nichts, was aussteht");
        assert!(leer.posten.is_empty());
        assert!(leer.ausstehende.is_empty());

        let eintraege = vec![
            eintrag("kicad", "elektronik", true, frisch("freigabe/elektronik/Rev-B")),
            eintrag("doku", "doku", false, Auswahl::Offen),
        ];
        assert_eq!(zusammenstellen(&eintraege), zusammenstellen(&eintraege));
    }

    /// Mehrere ausstehende Pflicht-Bereiche werden **alle** benannt (in Eingabe-Reihenfolge), damit
    /// die UI klar sagt, *was* noch fehlt — nie nur „unvollständig" ohne das Warum.
    #[test]
    fn mehrere_ausstehende_werden_alle_benannt() {
        let bericht = zusammenstellen(&[
            eintrag("kicad", "elektronik", true, Auswahl::Offen),
            eintrag("zephyr", "firmware", true, frisch("freigabe/firmware/v0.3")),
            eintrag("fusion", "mechanik", true, Auswahl::Offen),
        ]);
        assert!(!bericht.vollstaendig);
        assert_eq!(
            bericht.ausstehende,
            vec!["elektronik".to_string(), "mechanik".to_string()],
            "beide ausstehenden Pflicht-Bereiche, in Eingabe-Reihenfolge"
        );
    }

    /// **AC: die Cold-Start-Seed-Liste über {alle/manche/keine schon revidiert}** (E52b) — die eine
    /// Tabelle, die beweist, *wer* beim allerersten Release eine initiale Revision braucht. Ein
    /// Pflicht-Baustein **ohne** jeden Stand gehört hinein; ein **schon revidierter** (er trägt schon
    /// einen `freigabe/...`-Stand) und jeder **optionale** nie. Optional blockiert auch im Cold-Start
    /// nie (E52a).
    #[test]
    fn kaltstart_seed_liste_ueber_schon_revidiert() {
        // (name, Einträge, erwartete Seed-Heimaten in Eingabe-Reihenfolge)
        let cases: &[(&str, Vec<BausteinEintrag>, Vec<&str>)] = &[
            // KEINER schon revidiert: beide Pflicht-Bausteine brauchen einen Seed.
            (
                "keiner revidiert",
                vec![
                    eintrag_mit_staenden("kicad", "elektronik", true, &[]),
                    eintrag_mit_staenden("zephyr", "firmware", true, &[]),
                ],
                vec!["elektronik", "firmware"],
            ),
            // MANCHE schon revidiert: elektronik trägt schon einen Stand → nur firmware wird gesät.
            (
                "manche revidiert",
                vec![
                    eintrag_mit_staenden("kicad", "elektronik", true, &["freigabe/elektronik/Rev-A"]),
                    eintrag_mit_staenden("zephyr", "firmware", true, &[]),
                ],
                vec!["firmware"],
            ),
            // ALLE schon revidiert: nichts mehr zu säen — der Cold-Start ist überstanden.
            (
                "alle revidiert",
                vec![
                    eintrag_mit_staenden("kicad", "elektronik", true, &["freigabe/elektronik/Rev-A"]),
                    eintrag_mit_staenden("zephyr", "firmware", true, &["freigabe/firmware/v0.1"]),
                ],
                vec![],
            ),
            // Optionale ohne Stand werden NIE gesät (sie blockieren nie — E52a); der Pflicht-Bereich
            // ohne Stand schon.
            (
                "optional wird nie gesät",
                vec![
                    eintrag_mit_staenden("kicad", "elektronik", true, &[]),
                    eintrag_mit_staenden("doku", "doku", false, &[]),
                ],
                vec!["elektronik"],
            ),
            // Leeres Produkt: leere Seed-Liste, nie ein Panik (total).
            ("leer", vec![], vec![]),
        ];

        for (name, eintraege, erwartet) in cases {
            let seed = kaltstart_seed_liste(eintraege);
            let heimaten: Vec<&str> = seed.iter().map(|s| s.heimat.as_str()).collect();
            assert_eq!(heimaten, *erwartet, "Seed-Liste für {name}");
        }
    }

    /// **AC: nach dem Seed ist die erste Produkt-Revision komponierbar** (E52b) — Brücke vom Seed
    /// zum Vollständigkeits-Kern: bekämen die gesäten Pflicht-Bausteine ihren initialen Stand und
    /// würde er als frischer Beitrag gewählt, ist die Zusammenstellung vollständig. Hier rein
    /// modelliert: dieselben Bausteine **mit** Stand + frischer Auswahl → `vollstaendig`.
    #[test]
    fn nach_seed_ist_erste_produkt_revision_komponierbar() {
        // Vor dem Seed: beide Pflicht-Bausteine ohne Stand → beide stehen in der Seed-Liste, und die
        // Zusammenstellung (alles offen) ist unvollständig.
        let vor = vec![
            eintrag_mit_staenden("kicad", "elektronik", true, &[]),
            eintrag_mit_staenden("zephyr", "firmware", true, &[]),
        ];
        assert_eq!(kaltstart_seed_liste(&vor).len(), 2, "beide brauchen einen Seed");
        assert!(!zusammenstellen(&vor).vollstaendig, "vor dem Seed: noch nichts beigetragen");

        // Nach dem Seed: jeder Pflicht-Baustein trägt seinen initialen Stand und wählt ihn frisch →
        // die Seed-Liste ist leer und die erste Produkt-Revision ist vollständig, ohne N Handgriffe.
        let nach = vec![
            eintrag("kicad", "elektronik", true, frisch("freigabe/elektronik/Rev-A")),
            eintrag("zephyr", "firmware", true, frisch("freigabe/firmware/v0.1")),
        ];
        let nach_staende = vec![
            eintrag_mit_staenden("kicad", "elektronik", true, &["freigabe/elektronik/Rev-A"]),
            eintrag_mit_staenden("zephyr", "firmware", true, &["freigabe/firmware/v0.1"]),
        ];
        assert!(kaltstart_seed_liste(&nach_staende).is_empty(), "nach dem Seed: nichts mehr zu säen");
        assert!(zusammenstellen(&nach).vollstaendig, "nach dem Seed: erste Produkt-Revision komponierbar");
    }
}
