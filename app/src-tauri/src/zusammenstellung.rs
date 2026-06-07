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
}
