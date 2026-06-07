//! Der **Integrations-Block-Kern** — entscheidet, ob ein **Cross-Baustein-Integrationstest** eine
//! Produkt-Komposition blockiert, und leitet die passiven **Leseschein**-Zeilen ab (Issue #141, E53).
//!
//! Folgt dem Haus-Muster (`aufgabenblock.rs`, `compose.rs`, `syncdecider.rs`): **eine reine, totale,
//! deterministische** Funktion über einen schlichten Schnappschuss. Sie kennt **kein** git, keine
//! Uhr, kein I/O — das Flaggen/Beantworten/Protokollieren (die dünne Glue) lebt in
//! [`crate::integrationsblockglue`]; dieses Modul **entscheidet ausschließlich**: offene
//! Integrations-Aufgaben × Compose-Auswahl → Block-Entscheid + abgeleitete Leseschein-Zeilen.
//!
//! ## Warum eine Integrations-Aufgabe (E53)
//!
//! Manche Belege gehören **zwischen** zwei Bausteine, nicht in einen. Der HW-Entwickler markiert
//! seinen PCB-Stand **gegen die Firmware** als **„braucht FW-Test"** — eine **opt-in**, **einmalige**,
//! **blockierende** Integrations-Aufgabe, **erhoben gegen eine Quell-Revision** (den Stand seines
//! Bausteins zum Flagge-Zeitpunkt). Der SW-Entwickler (der **Empfänger**) beantwortet die Forderung
//! mit **ja/nein**, der Beleg liegt im Protokoll.
//!
//! Die tragenden Regeln (E53):
//!
//! - Ein **„nein"** (oder eine **noch offene**, unbeantwortete Forderung) hält einen **harten Block**
//!   — **aber nur an der Produkt-Compose**, nie an der eigenständigen Baustein-/FW-Freigabe. Jeder
//!   Baustein reift für sich; die Integrations-Strenge sitzt erst dort, wo die Stände
//!   **zusammenkommen**.
//! - Ein **„ja"** ist ein Beleg, der **einmal verbraucht** wird (kein Template): er gilt **genau für
//!   die Quell-Revision**, gegen die geflaggt wurde. Wird ein **neuer** Quell-Stand komponiert, ist
//!   der alte Beleg verbraucht — die Forderung muss am neuen Quell-Stand **neu geflaggt** werden
//!   (oder eben nicht). Ein „ja" gegen Rev D deckt **nicht** Rev E.
//! - Der **Leseschein** ist eine **passive** abgeleitete Zeile („FW zuletzt gegen PCB Rev D getestet,
//!   du nimmst Rev E — kein Test für diese Kombination"). Er **blockiert nichts** — er macht nur die
//!   bekannte/fehlende Test-Kombination an der Compose sichtbar.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ----------------------------------------------------------------------------------------------
// Eingabe — die offene Integrations-Aufgabe (gegen eine Quell-Revision erhoben)
// ----------------------------------------------------------------------------------------------

/// Die **Antwort** des Empfängers auf eine Integrations-Forderung (E53). Der HW-Entwickler flaggt,
/// der SW-Entwickler (Empfänger) beantwortet mit ja/nein; bis dahin ist die Forderung **offen**.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationsAntwort {
    /// Noch **offen** — der Empfänger hat die Forderung weder bejaht noch verneint. Hält den Block
    /// an der Compose (eine geforderte, aber unbelegte Integration darf nicht still durchgehen).
    Offen,
    /// **Ja** — der Empfänger bestätigt: für **diese** Quell-Revision ist der Integrationstest belegt.
    /// Hebt den Block für genau diese Kombination auf (einmaliger, verbrauchter Beleg).
    Ja,
    /// **Nein** — der Empfänger verneint: kein Test / Test nicht bestanden. Hält den harten Block an
    /// der Compose.
    Nein,
}

/// Eine **Integrations-Aufgabe** (Issue #141, E53): die Forderung des HW-Entwicklers, seinen Baustein
/// **gegen** einen anderen integrativ zu testen — **erhoben gegen eine Quell-Revision**. Schlichte
/// Daten; die Glue füllt sie aus dem `_plm`-Beleg-Speicher. Der Kern macht **kein** I/O über sie.
///
/// Das **Baustein-Paar** ist die Kombination, die getestet werden soll: `quell_baustein` ist der
/// flaggende Baustein (z.B. die Elektronik/PCB), `ziel_baustein` der Baustein, **gegen** den getestet
/// wird (z.B. die Firmware). `quell_rev` ist der Stand des flaggenden Bausteins, **gegen** den die
/// Forderung erhoben (und ein „ja" belegt) wurde — der Beleg gilt **genau** für diese Revision.
///
/// Serde-/specta-tauglich: dieselbe Form ist der **Akten-Beleg** im `_plm`-Speicher
/// ([`crate::integrationsblockglue`]) **und** der Wire-Typ für die UI — eine Wahrheit, nicht zwei.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct IntegrationsAufgabe {
    /// Stabile, undurchsichtige id der Forderung (die Glue vergibt sie; die UI keyt Zeilen darauf).
    pub id: String,
    /// Die `id` des **flaggenden** Bausteins (z.B. `"kicad"` / die PCB), der den Test fordert.
    pub quell_baustein: String,
    /// Die `id` des **Empfänger**-Bausteins, **gegen** den getestet wird (z.B. `"zephyr"` / die FW).
    pub ziel_baustein: String,
    /// Die **Quell-Revision**, gegen die die Forderung erhoben (und ein „ja" belegt) wurde — der
    /// Stand des flaggenden Bausteins zum Flagge-Zeitpunkt (z.B. `"Rev D"`). Einmalig: ein Beleg gilt
    /// genau für diese Revision; ein neuer Quell-Stand braucht eine neue Forderung.
    pub quell_rev: String,
    /// Die Antwort des Empfängers (offen/ja/nein) — der Beleg auf Akte.
    pub antwort: IntegrationsAntwort,
}

/// Eine **Compose-Auswahl** aus Sicht des Integrations-Blocks: pro Baustein die Revision, die gerade
/// **komponiert** wird (Issue #141, E53). Schlichte Daten — die Glue leitet sie aus der
/// Produkt-Komposition ([`crate::compose`]) ab (welcher Baustein steuert welchen freigegebenen
/// Stand bei). Der Kern liest daraus, **gegen welche** Quell-/Ziel-Stände die Forderungen zu prüfen
/// sind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeBausteinRev {
    /// Die `id` des Bausteins, der in dieser Komposition mitkommt (z.B. `"kicad"`).
    pub baustein: String,
    /// Die Revision dieses Bausteins, die komponiert wird (z.B. `"Rev E"`).
    pub rev: String,
}

// ----------------------------------------------------------------------------------------------
// Ausgabe — der Block-Entscheid + die abgeleiteten Leseschein-Zeilen
// ----------------------------------------------------------------------------------------------

/// Eine **passive Leseschein-Zeile** (E53): macht eine Test-Kombination an der Compose sichtbar,
/// **blockiert aber nichts**. Sie sagt, **welcher** belegte Stand zuletzt gegen den Partner getestet
/// wurde und **welcher** Stand jetzt komponiert wird — damit der Compose-Lesende selbst sieht, ob
/// die Kombination belegt ist („FW zuletzt gegen PCB Rev D getestet, du nimmst Rev E — kein Test für
/// diese Kombination").
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LesescheinZeile {
    /// Der flaggende (Quell-)Baustein des Paares (z.B. `"kicad"`).
    pub quell_baustein: String,
    /// Der Empfänger-(Ziel-)Baustein des Paares (z.B. `"zephyr"`).
    pub ziel_baustein: String,
    /// Die zuletzt **belegt getestete** Quell-Revision dieses Paares (`Some("Rev D")`), oder `None`,
    /// wenn für dieses Paar **gar kein** Test belegt ist.
    pub zuletzt_getestete_rev: Option<String>,
    /// Die Quell-Revision, die jetzt **komponiert** wird (z.B. `"Rev E"`).
    pub komponierte_rev: String,
    /// Ob die komponierte Kombination **belegt** ist (`true` ⇔ ein „ja" gegen genau die komponierte
    /// Quell-Revision liegt vor). Rein informativ — der Leseschein blockiert nie.
    pub belegt: bool,
}

/// Der **Integrations-Block-Entscheid** für eine Produkt-Komposition (Issue #141, E53). Genau einer;
/// total. Trägt die **ids der blockierenden Forderungen** (damit die UI sie benennen kann, ohne die
/// Regel erneut zu prüfen) und die abgeleiteten **passiven** Leseschein-Zeilen.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntegrationsBlockEntscheid {
    /// Ob die Compose blockiert ist. `true` ⇔ [`Self::blockierende_ids`] ist nicht leer (als
    /// ehrliches einzelnes Flag mitgeführt, damit die UI eine Wahrheit liest).
    pub blockiert: bool,
    /// Die ids der Integrations-Forderungen, die diese Compose blockieren, in Eingabe-Reihenfolge.
    /// Leer ⇔ nicht blockiert. Genau die Forderungen, deren komponierte Kombination „nein"/offen ist.
    pub blockierende_ids: Vec<String>,
    /// Die passiven Leseschein-Zeilen, eine pro relevantem Baustein-Paar, in stabiler Reihenfolge.
    /// Sie blockieren **nichts** — sie machen nur die bekannte/fehlende Test-Kombination sichtbar.
    pub lesescheine: Vec<LesescheinZeile>,
}

impl IntegrationsBlockEntscheid {
    /// Ob die Compose blockiert ist (ein harter Block). Wahr ⇔ mindestens eine blockierende Forderung.
    pub fn ist_blockiert(&self) -> bool {
        self.blockiert
    }

    /// Wie viele Integrations-Forderungen diese Compose blockieren. Null ⇔ nicht blockiert.
    pub fn blockierende_anzahl(&self) -> usize {
        self.blockierende_ids.len()
    }
}

// ----------------------------------------------------------------------------------------------
// Der Integrations-Block-Kern — rein, total, deterministisch
// ----------------------------------------------------------------------------------------------

/// Den **Integrations-Block-Kern** anwenden (Issue #141, E53): aus den offenen Integrations-Aufgaben
/// und der Compose-Auswahl den Block-Entscheid + die passiven Leseschein-Zeilen ableiten. **Rein,
/// total, deterministisch** — kein I/O, keine Uhr. **Der Block gilt nur an der Compose** (diese
/// Funktion wird ausschließlich beim Komponieren aufgerufen; die eigenständige Baustein-/FW-Freigabe
/// ruft sie nie auf — so blockiert eine Integrations-Forderung **nie** eine Einzel-Freigabe).
///
/// Die Regel, in einer Zeile: **eine Forderung blockiert, wenn ihre Quell-Revision gerade komponiert
/// wird und der Beleg dafür „nein" oder offen ist; ein „ja" gegen genau diese Quell-Revision hebt den
/// Block auf, ein „ja" gegen einen anderen Stand ist verbraucht.**
///
/// Im Einzelnen, pro Forderung:
/// - Wird ihre `quell_rev` **nicht** komponiert (oder ihr `quell_baustein` ist gar nicht in der
///   Auswahl), ist sie **irrelevant** für diese Compose — der Beleg ist (für diese Kombination) nicht
///   gefordert. Sie blockiert nicht.
/// - Wird ihre `quell_rev` komponiert und die Antwort ist **`Ja`**, ist der Test für **genau diese**
///   Kombination belegt — sie blockiert nicht (der einmalige Beleg ist eingelöst).
/// - Wird ihre `quell_rev` komponiert und die Antwort ist **`Nein`** oder **`Offen`**, ist es ein
///   **harter Block** — die Compose darf diese ungetestete/abgelehnte Kombination nicht still bauen.
///
/// Die **Einmaligkeit** fällt aus der Quell-Rev-Bindung: ein „ja" gegen `Rev D` deckt eine Compose,
/// die `Rev D` nimmt; nimmt die Compose `Rev E`, greift dieses „ja" nicht mehr — die Forderung müsste
/// am neuen Quell-Stand neu geflaggt werden (sonst gibt es schlicht keine Forderung gegen `Rev E`,
/// und der Leseschein meldet die fehlende Kombination).
///
/// Reihenfolge: `blockierende_ids` behält die **Eingabe-Reihenfolge** der Forderungen; die
/// Leseschein-Zeilen sind **nach Baustein-Paar sortiert** (stabil, deterministisch).
pub fn entscheide_integrationsblock(
    aufgaben: &[IntegrationsAufgabe],
    compose: &[ComposeBausteinRev],
) -> IntegrationsBlockEntscheid {
    // Schnelle Auflösung „welche Revision wird für Baustein X komponiert?".
    let komponiert: BTreeMap<&str, &str> =
        compose.iter().map(|c| (c.baustein.as_str(), c.rev.as_str())).collect();

    // 1) Block-Entscheid: pro Forderung prüfen, ob ihre Quell-Revision gerade komponiert wird und der
    //    Beleg dafür fehlt (nein/offen). Eingabe-Reihenfolge der ids bleibt erhalten.
    let blockierende_ids: Vec<String> = aufgaben
        .iter()
        .filter(|a| aufgabe_blockiert(a, &komponiert))
        .map(|a| a.id.clone())
        .collect();

    // 2) Leseschein-Zeilen ableiten: eine pro relevantem Baustein-Paar (nach Paar stabil sortiert).
    let lesescheine = leite_leseschein_ab(aufgaben, &komponiert);

    IntegrationsBlockEntscheid {
        blockiert: !blockierende_ids.is_empty(),
        blockierende_ids,
        lesescheine,
    }
}

/// Ob **eine einzelne** Integrations-Forderung die laufende Compose blockiert. Rein.
///
/// Sie blockiert genau dann, wenn ihre **Quell-Revision gerade komponiert** wird **und** der Beleg
/// dafür **nein/offen** ist. Ein „ja" gegen genau diese Quell-Revision hebt den Block auf; wird die
/// Quell-Revision nicht komponiert (anderer Stand oder Baustein nicht in der Auswahl), ist die
/// Forderung für diese Compose irrelevant und blockiert nicht.
fn aufgabe_blockiert(a: &IntegrationsAufgabe, komponiert: &BTreeMap<&str, &str>) -> bool {
    // Wird genau die Quell-Revision dieser Forderung komponiert? Sonst ist sie irrelevant
    // (der einmalige Beleg ist an die Quell-Revision gebunden).
    let trifft_compose = komponiert.get(a.quell_baustein.as_str()) == Some(&a.quell_rev.as_str());
    if !trifft_compose {
        return false;
    }
    // Quell-Rev wird komponiert: nur ein „ja" hebt den Block; nein/offen ist ein harter Block.
    !matches!(a.antwort, IntegrationsAntwort::Ja)
}

/// Die **passiven Leseschein-Zeilen** ableiten (E53): eine pro Baustein-Paar, das in einer Forderung
/// vorkommt **und** dessen Quell-Baustein gerade komponiert wird. Pro Paar: die zuletzt **belegt
/// getestete** Quell-Revision (das jüngste „ja"; `None`, falls keines), die jetzt komponierte
/// Quell-Revision und ob die komponierte Kombination belegt ist. Blockiert nichts.
///
/// Stabil nach Baustein-Paar `(quell, ziel)` sortiert (eine `BTreeMap` ordnet die Schlüssel), damit
/// die Zeilen-Reihenfolge deterministisch ist, unabhängig von der Eingabe-Reihenfolge.
fn leite_leseschein_ab(
    aufgaben: &[IntegrationsAufgabe],
    komponiert: &BTreeMap<&str, &str>,
) -> Vec<LesescheinZeile> {
    // Pro Baustein-Paar sammeln: die belegt-getesteten (Antwort = Ja) Quell-Revisionen, in
    // Eingabe-Reihenfolge (die Glue trägt die Forderungen chronologisch ein). Ein leerer Eintrag
    // hält fest, dass das Paar überhaupt vorkommt — so bekommt auch eine nie belegte, aber
    // geforderte Kombination einen Leseschein „kein Test".
    let mut belegt_je_paar: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for a in aufgaben {
        let paar = (a.quell_baustein.as_str(), a.ziel_baustein.as_str());
        let eintrag = belegt_je_paar.entry(paar).or_default();
        if matches!(a.antwort, IntegrationsAntwort::Ja) {
            eintrag.push(a.quell_rev.as_str());
        }
    }

    let mut zeilen: Vec<LesescheinZeile> = Vec::new();
    for ((quell, ziel), belegte_revs) in &belegt_je_paar {
        // Nur Paare, deren Quell-Baustein gerade tatsächlich komponiert wird, sind an der Compose
        // sichtbar relevant — sonst gibt es keine „komponierte Revision" zu lesen.
        let Some(komponierte_rev) = komponiert.get(*quell) else {
            continue;
        };
        // Ist die komponierte Quell-Revision selbst belegt (ein „ja" gegen genau sie)?
        let belegt = belegte_revs.contains(komponierte_rev);
        // Die zuletzt belegt getestete Quell-Revision: das letzte „ja" in Eingabe-Reihenfolge,
        // oder `None`, wenn keines vorliegt.
        let zuletzt_getestete_rev = belegte_revs.last().map(|r| r.to_string());
        zeilen.push(LesescheinZeile {
            quell_baustein: quell.to_string(),
            ziel_baustein: ziel.to_string(),
            zuletzt_getestete_rev,
            komponierte_rev: komponierte_rev.to_string(),
            belegt,
        });
    }
    zeilen
}

impl LesescheinZeile {
    /// Die deutsche Domänen-Meldung des Leseschein für die UI/das Protokoll. Nennt die Bausteine und
    /// die Stände beim Namen, nie ein git-Wort. Eine belegte Kombination liest sich bestätigend, eine
    /// unbelegte als deutlicher (aber **nicht** blockierender) Hinweis auf die fehlende Kombination.
    pub fn meldung(&self) -> String {
        match (&self.zuletzt_getestete_rev, self.belegt) {
            (_, true) => format!(
                "{ziel} gegen {quell} {rev} getestet — diese Kombination ist belegt",
                ziel = self.ziel_baustein,
                quell = self.quell_baustein,
                rev = self.komponierte_rev,
            ),
            (Some(zuletzt), false) => format!(
                "{ziel} zuletzt gegen {quell} {zuletzt} getestet, du nimmst {komponiert} — kein Test für diese Kombination",
                ziel = self.ziel_baustein,
                quell = self.quell_baustein,
                zuletzt = zuletzt,
                komponiert = self.komponierte_rev,
            ),
            (None, false) => format!(
                "{ziel} noch nie gegen {quell} getestet, du nimmst {komponiert} — kein Test für diese Kombination",
                ziel = self.ziel_baustein,
                quell = self.quell_baustein,
                komponiert = self.komponierte_rev,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aufgabe(
        id: &str,
        quell_baustein: &str,
        ziel_baustein: &str,
        quell_rev: &str,
        antwort: IntegrationsAntwort,
    ) -> IntegrationsAufgabe {
        IntegrationsAufgabe {
            id: id.to_string(),
            quell_baustein: quell_baustein.to_string(),
            ziel_baustein: ziel_baustein.to_string(),
            quell_rev: quell_rev.to_string(),
            antwort,
        }
    }

    fn compose(baustein: &str, rev: &str) -> ComposeBausteinRev {
        ComposeBausteinRev { baustein: baustein.to_string(), rev: rev.to_string() }
    }

    /// **Die Kern-Akzeptanzmatrix** (E53): das volle Kreuzprodukt {offen, ja, nein} × {Quell-Rev wird
    /// komponiert, Quell-Rev wird NICHT komponiert} → blockiert?. „nein" hält den Block; ein „ja"
    /// gegen die komponierte Rev hebt ihn; eine nicht-komponierte Rev ist nie relevant (verbrauchter,
    /// an die Quell-Rev gebundener Beleg).
    #[test]
    fn kreuzprodukt_antwort_und_compose_auswahl() {
        // Die Forderung ist immer „PCB (kicad) gegen FW (zephyr), erhoben gegen Rev D".
        // Achse 1: die Antwort. Achse 2: ob die Compose genau Rev D nimmt (oder Rev E, also nicht).
        let komponiert_rev_d = vec![compose("kicad", "Rev D"), compose("zephyr", "v0.3")];
        let komponiert_rev_e = vec![compose("kicad", "Rev E"), compose("zephyr", "v0.3")];

        // (Antwort, nimmt-Rev-D?, erwartet_blockiert)
        let cases: &[(IntegrationsAntwort, bool, bool)] = &[
            // Quell-Rev (Rev D) WIRD komponiert: offen/nein blockieren, ja hebt auf.
            (IntegrationsAntwort::Offen, true, true),
            (IntegrationsAntwort::Nein, true, true), // „nein" hält den Block
            (IntegrationsAntwort::Ja, true, false),  // belegt für genau diese Kombination
            // Quell-Rev (Rev D) wird NICHT komponiert (Compose nimmt Rev E): nie relevant —
            // der einmalige Beleg ist an Rev D gebunden, gegen Rev E gibt es (noch) keine Forderung.
            (IntegrationsAntwort::Offen, false, false),
            (IntegrationsAntwort::Nein, false, false),
            (IntegrationsAntwort::Ja, false, false),
        ];

        for (antwort, nimmt_rev_d, erwartet) in cases {
            let a = aufgabe("i1", "kicad", "zephyr", "Rev D", *antwort);
            let compose = if *nimmt_rev_d { &komponiert_rev_d } else { &komponiert_rev_e };
            let d = entscheide_integrationsblock(std::slice::from_ref(&a), compose);
            assert_eq!(
                d.ist_blockiert(),
                *erwartet,
                "antwort={antwort:?} nimmt_rev_d={nimmt_rev_d} erwartet_blockiert={erwartet}"
            );
            // Flag und id-Liste stimmen überein: blockiert ⇔ mindestens eine benannte Forderung.
            assert_eq!(d.blockiert, !d.blockierende_ids.is_empty());
            if *erwartet {
                assert_eq!(d.blockierende_ids, vec!["i1".to_string()]);
            } else {
                assert!(d.blockierende_ids.is_empty());
            }
        }
    }

    /// AC: ein **„nein"** hält den **harten Block** an der Compose, und der Entscheid benennt genau
    /// die ablehnende Forderung.
    #[test]
    fn nein_haelt_den_block_und_benennt_die_forderung() {
        let a = aufgabe("i-nein", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Nein);
        let d = entscheide_integrationsblock(
            std::slice::from_ref(&a),
            &[compose("kicad", "Rev D"), compose("zephyr", "v0.3")],
        );
        assert!(d.ist_blockiert(), "ein „nein\" ist ein harter Block an der Compose");
        assert_eq!(d.blockierende_ids, vec!["i-nein".to_string()]);
    }

    /// AC: der Beleg ist **einmalig** und an die Quell-Revision gebunden. Ein „ja" gegen Rev D deckt
    /// eine Compose mit Rev D — nimmt die Compose Rev E, greift es nicht mehr; die Forderung muss am
    /// neuen Quell-Stand neu geflaggt werden (sonst gibt es gegen Rev E schlicht keine Forderung,
    /// also auch keinen Block, nur einen Leseschein).
    #[test]
    fn ja_ist_einmalig_und_an_die_quell_rev_gebunden() {
        let ja_gegen_d = aufgabe("i-ja", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Ja);

        // Rev D komponiert → das „ja" deckt sie → nicht blockiert.
        let mit_d = entscheide_integrationsblock(
            std::slice::from_ref(&ja_gegen_d),
            &[compose("kicad", "Rev D")],
        );
        assert!(!mit_d.ist_blockiert(), "ja gegen Rev D deckt eine Compose mit Rev D");

        // Rev E komponiert → das „ja" gegen Rev D ist verbraucht → keine Forderung gegen Rev E →
        // nicht blockiert (es existiert keine offene Forderung gegen Rev E), aber ein Leseschein
        // meldet die fehlende Kombination.
        let mit_e = entscheide_integrationsblock(
            std::slice::from_ref(&ja_gegen_d),
            &[compose("kicad", "Rev E")],
        );
        assert!(
            !mit_e.ist_blockiert(),
            "ein verbrauchtes ja gegen Rev D fordert nichts für Rev E (keine Forderung ⇒ kein Block)"
        );
        assert_eq!(mit_e.lesescheine.len(), 1, "ein Leseschein für die ungetestete Kombination");
        let zeile = &mit_e.lesescheine[0];
        assert_eq!(zeile.zuletzt_getestete_rev.as_deref(), Some("Rev D"));
        assert_eq!(zeile.komponierte_rev, "Rev E");
        assert!(!zeile.belegt, "Rev E ist nicht belegt");
    }

    /// AC: der **passive Leseschein** wird abgeleitet (zuletzt getestete Rev je Baustein-Paar vs. zu
    /// komponierende Rev) und **blockiert nichts** — selbst wenn er eine ungetestete Kombination
    /// meldet, bleibt `blockiert` falsch (es gibt keine offene Forderung gegen den komponierten Stand).
    #[test]
    fn leseschein_ist_passiv_und_blockiert_nie() {
        // Beleg „ja" gegen Rev D, komponiert wird aber Rev E.
        let ja_gegen_d = aufgabe("i-ja", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Ja);
        let d = entscheide_integrationsblock(
            std::slice::from_ref(&ja_gegen_d),
            &[compose("kicad", "Rev E"), compose("zephyr", "v0.4")],
        );
        assert!(!d.ist_blockiert(), "der Leseschein blockiert nie");
        assert_eq!(d.lesescheine.len(), 1);
        let z = &d.lesescheine[0];
        assert_eq!(z.quell_baustein, "kicad");
        assert_eq!(z.ziel_baustein, "zephyr");
        assert_eq!(z.zuletzt_getestete_rev.as_deref(), Some("Rev D"));
        assert_eq!(z.komponierte_rev, "Rev E");
        assert!(!z.belegt);
        // Der Beispielsatz aus E53 (sinngemäß): „FW zuletzt gegen PCB Rev D getestet, du nimmst
        // Rev E — kein Test für diese Kombination."
        let m = z.meldung();
        assert!(m.contains("Rev D") && m.contains("Rev E"), "Leseschein nennt beide Stände: {m}");
        assert!(m.contains("kein Test"), "Leseschein meldet die fehlende Kombination: {m}");
    }

    /// Ein Leseschein für eine **belegte** Kombination liest sich bestätigend und meldet `belegt`.
    #[test]
    fn leseschein_belegt_liest_sich_bestaetigend() {
        let ja_gegen_d = aufgabe("i-ja", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Ja);
        let d = entscheide_integrationsblock(
            std::slice::from_ref(&ja_gegen_d),
            &[compose("kicad", "Rev D")],
        );
        assert_eq!(d.lesescheine.len(), 1);
        let z = &d.lesescheine[0];
        assert!(z.belegt, "die komponierte Rev D ist belegt");
        assert!(z.meldung().contains("belegt"), "die Meldung bestätigt: {}", z.meldung());
    }

    /// AC: der Block gilt **nur an der Compose** — eine leere Compose-Auswahl ist die Stand-in für
    /// „keine Komposition" (die eigenständige Baustein-/FW-Freigabe ruft den Kern gar nicht erst auf).
    /// Ohne komponierten Quell-Stand ist keine Forderung relevant, also blockiert nichts, egal wie
    /// viele „nein" offen sind.
    #[test]
    fn ohne_compose_blockiert_nichts() {
        let aufgaben = vec![
            aufgabe("i1", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Nein),
            aufgabe("i2", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Offen),
        ];
        let d = entscheide_integrationsblock(&aufgaben, &[]);
        assert!(
            !d.ist_blockiert(),
            "ohne Compose-Auswahl (eigenständige Freigabe) blockiert eine Integrations-Forderung nie"
        );
        assert_eq!(d.blockierende_anzahl(), 0);
        assert!(d.lesescheine.is_empty(), "ohne komponierten Quell-Stand keine Leseschein-Zeile");
    }

    /// Eine Forderung, deren **Quell-Baustein gar nicht** in der Compose-Auswahl steht (er wird nicht
    /// komponiert), ist irrelevant — sie blockiert nicht und erzeugt keinen Leseschein.
    #[test]
    fn nicht_komponierter_quell_baustein_ist_irrelevant() {
        let a = aufgabe("i1", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Nein);
        // Komponiert wird nur die Firmware, nicht die PCB.
        let d = entscheide_integrationsblock(std::slice::from_ref(&a), &[compose("zephyr", "v0.3")]);
        assert!(!d.ist_blockiert(), "ein nicht komponierter Quell-Baustein fordert nichts");
        assert!(d.lesescheine.is_empty());
    }

    /// Mehrere Forderungen über mehrere Paare: der Entscheid benennt **alle** blockierenden in
    /// Eingabe-Reihenfolge und leitet die Leseschein-Zeilen **stabil nach Paar sortiert** ab.
    #[test]
    fn mehrere_paare_block_und_leseschein() {
        let aufgaben = vec![
            // PCB gegen FW, Rev D, nein → blockiert (Rev D wird komponiert).
            aufgabe("i-pcb-fw", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Nein),
            // Mechanik gegen PCB, v1.0, ja → belegt, blockiert nicht (v1.0 wird komponiert).
            aufgabe("i-mech-pcb", "fusion", "kicad", "v1.0", IntegrationsAntwort::Ja),
            // PCB gegen Mechanik, Rev D, offen → blockiert (Rev D wird komponiert).
            aufgabe("i-pcb-mech", "kicad", "fusion", "Rev D", IntegrationsAntwort::Offen),
        ];
        let compose = vec![
            compose("kicad", "Rev D"),
            compose("zephyr", "v0.3"),
            compose("fusion", "v1.0"),
        ];
        let d = entscheide_integrationsblock(&aufgaben, &compose);
        assert!(d.ist_blockiert());
        // Eingabe-Reihenfolge der blockierenden ids (das belegte „ja" ist nicht dabei).
        assert_eq!(d.blockierende_ids, vec!["i-pcb-fw".to_string(), "i-pcb-mech".to_string()]);
        // Leseschein-Zeilen stabil nach Paar (quell, ziel) sortiert: (fusion,kicad), (kicad,fusion),
        // (kicad,zephyr).
        let paare: Vec<(&str, &str)> = d
            .lesescheine
            .iter()
            .map(|z| (z.quell_baustein.as_str(), z.ziel_baustein.as_str()))
            .collect();
        assert_eq!(paare, vec![("fusion", "kicad"), ("kicad", "fusion"), ("kicad", "zephyr")]);
    }

    /// Der Kern ist **total** — ein leerer Schnappschuss (keine Forderungen) blockiert nie und liefert
    /// keine Leseschein-Zeilen, und der Entscheid ist intern konsistent (Flag ⇔ nicht-leere id-Liste).
    #[test]
    fn leerer_schnappschuss_ist_total() {
        let d = entscheide_integrationsblock(&[], &[compose("kicad", "Rev D")]);
        assert!(!d.ist_blockiert());
        assert_eq!(d.blockiert, !d.blockierende_ids.is_empty());
        assert_eq!(d.blockierende_anzahl(), 0);
        assert!(d.lesescheine.is_empty());
    }

    /// **Determinismus**: derselbe Schnappschuss + dieselbe Compose-Auswahl ergibt immer denselben
    /// Entscheid (id-Liste, Leseschein-Reihenfolge und alles).
    #[test]
    fn entscheid_ist_deterministisch() {
        let aufgaben = vec![
            aufgabe("i1", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Nein),
            aufgabe("i2", "fusion", "kicad", "v1.0", IntegrationsAntwort::Ja),
        ];
        let compose = vec![compose("kicad", "Rev D"), compose("fusion", "v1.0")];
        let a = entscheide_integrationsblock(&aufgaben, &compose);
        let b = entscheide_integrationsblock(&aufgaben, &compose);
        assert_eq!(a, b);
    }

    /// Die zuletzt getestete Rev je Paar ist das **jüngste „ja"** (die Glue trägt chronologisch ein):
    /// liegen mehrere „ja" gegen verschiedene Quell-Revisionen vor, nennt der Leseschein die letzte.
    #[test]
    fn zuletzt_getestete_rev_ist_das_juengste_ja() {
        let aufgaben = vec![
            aufgabe("i-alt", "kicad", "zephyr", "Rev C", IntegrationsAntwort::Ja),
            aufgabe("i-neu", "kicad", "zephyr", "Rev D", IntegrationsAntwort::Ja),
        ];
        // Komponiert wird Rev E (keine davon belegt) → Leseschein nennt die jüngste belegte: Rev D.
        let d = entscheide_integrationsblock(&aufgaben, &[compose("kicad", "Rev E")]);
        assert_eq!(d.lesescheine.len(), 1);
        assert_eq!(d.lesescheine[0].zuletzt_getestete_rev.as_deref(), Some("Rev D"));
        assert!(!d.lesescheine[0].belegt);
    }
}
