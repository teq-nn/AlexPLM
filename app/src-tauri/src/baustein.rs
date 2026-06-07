//! Baustein-Modell (Issue #39, ADR 0003) — reiner Kern + serde.
//!
//! Ein **Baustein** bündelt das Tool-Wissen für genau ein Tool: den Heimat-Ordner
//! (Arbeitsbereich), die Artefakt-Globs (geordnet — `[0]` ist die Hauptdatei-Regel), die
//! Ignore-/LFS-Muster, die Öffnen-Aktion, optionale Startaufgaben und interne Default-Kanten.
//!
//! Dieses Modul ist **rein**: nur das Datenmodell, seine serde-Form und ein paar totale
//! Entscheidungs-Helfer (Hauptdatei-Wahl, Öffnen-Auflösung). Es macht **kein** I/O — die
//! Speicherung (Bibliothek, Produkt-Stack) lebt in `bibliothek.rs` / `stackstore.rs`.
//!
//! **Identität:** stabile Kebab-`id` (`"kicad"`) + monotone Ganzzahl-`version` (ADR 0003).
//! **Lockability ist KEIN Baustein-Feld** — sie ist formatintrinsisch und bleibt in
//! `classifier.rs` (verworfene Alternative in ADR 0003).

use serde::{Deserialize, Serialize};

/// Was die Öffnen-Aktion einer Artefakt-Karte tun soll, wenn der Nutzer sie anklickt.
/// `Auto` heißt: dominante Einzeldatei → diese öffnen, sonst den Ordner öffnen (PRD §14).
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Oeffnen {
    /// Hauptdatei wenn es eine dominante gibt, sonst Ordner (PRD §14). Default.
    #[default]
    Auto,
    /// Immer die Hauptdatei im OS-Standardprogramm öffnen.
    Datei,
    /// Immer den Heimat-Ordner öffnen.
    Ordner,
}

/// Art einer Startaufgabe: eine **Aufgabe** kann blockieren (verpflichtend), ein **Hinweis**
/// blockiert nie (PRD §27). Die Trennung läuft über die Blockier-Fähigkeit, nicht die Wichtigkeit.
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AufgabenTyp {
    /// Verpflichtend; *kann* blockieren (siehe `blockiert`).
    Aufgabe,
    /// Blockiert nie.
    Hinweis,
}

/// Eine Startaufgabe, die beim Onboarding eines Bausteins in einem Produkt angelegt wird.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Startaufgabe {
    /// Menschlicher Titel der Aufgabe/des Hinweises.
    pub titel: String,
    /// Aufgabe (verpflichtend) oder Hinweis (nie blockierend).
    pub typ: AufgabenTyp,
    /// Ob diese Aufgabe das Freigabe-Gate hart blockiert. Für `Hinweis` immer `false`.
    #[serde(default)]
    pub blockiert: bool,
}

/// Eine interne Default-Kante des Bausteins: ein abgeleitetes Glob „stammt aus" einem Quell-Glob
/// (z.B. Fertigungs-STL stammt aus der CAD-Quelle). Pattern-basiert, nicht pro-Datei (PRD §13).
/// **Baustein-Default** (E20): kommt beim Onboarding automatisch, ganz **innerhalb** des Bausteins.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultKante {
    /// Glob des abgeleiteten Artefakts.
    pub derived_glob: String,
    /// Glob der Quelle, aus der es stammt.
    pub source_glob: String,
}

/// Eine **Baustein-Paar-Default-Kante** (E20): „wenn dieser Baustein **und** der Partner-Baustein
/// `partner_id` beide im Stack sind, schlage die Kante `derived_glob` ← `source_glob` vor". Der
/// `derived_glob`/`source_glob` greift über die Heimaten **beider** Bausteine hinweg (das ist der
/// Sinn der Paar-Stufe: die Kante überspannt zwei Bausteine und hat auf Baustein-Ebene keine
/// Heimat). Rein deterministisch — **kein** ML, keine Daten, kein Parser (E21). Der Vorschlag wird
/// **per Klick bestätigt**, nie automatisch angelegt (Onboarding bleibt ruhig).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaarDefaultKante {
    /// `id` des Partner-Bausteins, der zusätzlich im Stack liegen muss, damit der Vorschlag greift.
    pub partner_id: String,
    /// Glob des abgeleiteten Artefakts (z.B. Pick-and-Place).
    pub derived_glob: String,
    /// Glob der Quelle, aus der es stammt (z.B. Layout **und** BOM — je eine Paar-Kante).
    pub source_glob: String,
}

/// Eine **Rekonstruierbar-Regel** (E50b, Issue #137) — die *dritte* Pfad-Klasse neben `ignore`/`lfs`.
///
/// Eine git-native Toolchain (`west`, ESP-IDF, PlatformIO, `venv`) zieht beim ersten Build tausende
/// **rekonstruierbare** Framework-Dateien in den Heimat-Ordner — Dateien, die ein gepinntes
/// **Manifest** (`west.yml`, `platformio.ini`, `sdkconfig`, eine Lockfile) jederzeit **wieder erzeugt**.
/// Statt diesen ableitbaren Ballast mitzucommitten, verfolgt der Baustein **nur Quelle + gepinntes
/// Manifest**: das `framework`-Muster wird ignoriert, das `manifest` bleibt ausdrücklich verfolgt. Der
/// Zustand bleibt reproduzierbar, das Repo schlank — und die Formulierung **ehrlich**: „du hast
/// vollständige Ordner" heißt hier „Quelle + rekonstruierendes Manifest", keine falsche Vollständigkeit.
///
/// Das **`rekonstruierbar` ist nicht `ignore`**: Ignore wirft Müll weg, der nie zurückkommen muss;
/// Rekonstruierbar wirft *ableitbaren* Ballast weg und **hält das Rezept** (das Manifest) verfolgt, das
/// ihn wiederherstellt. Beide leben ausschließlich im idempotenten Marker-Block der Dotfiles (E18, keine
/// Spiegelung); aus einer Rekonstruierbar-Regel wird ein Ignore-Muster **plus** eine Negation, die das
/// Manifest aus dem Ignore wieder herausnimmt (`!west.yml`), sodass git Quelle + Manifest weiter sieht.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RekonstruierbarRegel {
    /// Das Muster der **rekonstruierbaren** Framework-Dateien, das ignoriert wird (z.B. `modules/`,
    /// `.west/`, `.pio/`). Das ist der ableitbare Ballast, den das Manifest jederzeit neu erzeugt.
    pub framework: String,
    /// Die **gepinnten Manifest**-Pfade, die trotz des `framework`-Ignores ausdrücklich **verfolgt**
    /// bleiben (`west.yml`, `platformio.ini`, `sdkconfig`, eine Lockfile). Pro Eintrag entsteht eine
    /// Negations-Zeile (`!<manifest>`) im Marker-Block, die das Manifest aus dem Ignore zurückholt.
    /// Hier dürfen auch **handgeänderte** Komponenten stehen, damit lokale Patches nicht verlorengehen.
    pub manifest: Vec<String>,
}

/// Ein **Baustein**: das wiederverwendbare Tool-Wissen für ein Tool (ADR 0003).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baustein {
    /// Stabile Kebab-Identität, z.B. `"kicad"`. Eindeutig in der Bibliothek.
    pub id: String,
    /// Monotone Ganzzahl-Version; höher = neuer. Trägt das version-gegatete Seeding (ADR 0003).
    pub version: u32,
    /// Menschlicher Name, z.B. `"KiCad"`.
    pub name: String,
    /// Default-Heimat-Ordner (Arbeitsbereich), z.B. `"elektronik"`. Pro Produkt auflösbar.
    pub heimat: String,
    /// Artefakt-Globs, **geordnet**: `[0]` ist die Hauptdatei-Regel (höchste Priorität).
    pub globs: Vec<String>,
    /// Ignore-Presets (Marker-Block-Zeilen für `.gitignore`).
    #[serde(default)]
    pub ignore: Vec<String>,
    /// LFS-Muster (Marker-Block-Zeilen für `.gitattributes`).
    #[serde(default)]
    pub lfs: Vec<String>,
    /// Rekonstruierbar-Regeln (E50b, Issue #137): die **dritte** Pfad-Klasse — verfolge nur Quelle +
    /// gepinntes Manifest, ignoriere die rekonstruierbaren Framework-Dateien. Lebt im selben
    /// idempotenten `.gitignore`-Marker-Block wie die Ignore-Muster (E18, keine Spiegelung).
    #[serde(default)]
    pub rekonstruierbar: Vec<RekonstruierbarRegel>,
    /// Öffnen-Aktion der Artefakt-Karte.
    #[serde(default)]
    pub oeffnen: Oeffnen,
    /// Beim Onboarding anzulegende Startaufgaben/Hinweise.
    #[serde(default)]
    pub startaufgaben: Vec<Startaufgabe>,
    /// Interne Default-Kanten (pattern-basiert) — Baustein-Default (E20), beim Onboarding angelegt.
    #[serde(default)]
    pub default_kanten: Vec<DefaultKante>,
    /// Paar-Default-Kanten (E20): Vorschläge, sobald ein Partner-Baustein mit im Stack liegt.
    #[serde(default)]
    pub paar_default_kanten: Vec<PaarDefaultKante>,
    /// Label-only stillgelegt (PRD §10): alte Globs greifen nicht mehr, nichts wird gelöscht.
    #[serde(default)]
    pub stillgelegt: bool,
}

impl Baustein {
    /// Die Hauptdatei-Glob-Regel: der erste (höchstpriorisierte) Glob, falls vorhanden.
    /// Rein und total — `None` bei leerer Glob-Liste.
    pub fn hauptdatei_glob(&self) -> Option<&str> {
        self.globs.first().map(String::as_str)
    }

    /// Auflösung der effektiven Öffnen-Aktion gegeben, ob es eine dominante Einzeldatei gibt.
    /// `Auto` wird hier zu `Datei` (dominante Einzeldatei vorhanden) bzw. `Ordner` aufgelöst;
    /// eine ausdrückliche Wahl (`Datei`/`Ordner`) bleibt unverändert (PRD §14). Total + rein.
    pub fn resolve_oeffnen(&self, has_dominant_file: bool) -> Oeffnen {
        match self.oeffnen {
            Oeffnen::Auto if has_dominant_file => Oeffnen::Datei,
            Oeffnen::Auto => Oeffnen::Ordner,
            explicit => explicit,
        }
    }
}

/// Das Ergebnis einer Baustein-Validierung (reiner Kern, Issue #108). Ein **harter** Fehler
/// (`errors` nicht leer) verhindert das Speichern; **weiche** Warnungen (`warnings`) informieren
/// nur (Haus-Stil „degradieren, nie krachen"): z.B. ein Partner-`id`, der (noch) nicht in der
/// Bibliothek liegt, blockiert NICHT — der Vorschlag greift einfach erst, wenn der Partner existiert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Harte Feld-Fehler als (Feld-Schlüssel, deutsche Meldung). Leer ⇒ speicherbar.
    pub errors: Vec<(String, String)>,
    /// Weiche Warnungen (deutsche Meldungen); informieren nur, blockieren nie.
    pub warnings: Vec<String>,
}

impl ValidationReport {
    /// Ob der Baustein speicherbar ist (keine harten Fehler).
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    fn err(&mut self, field: &str, msg: &str) {
        self.errors.push((field.to_string(), msg.to_string()));
    }
}

/// Prüft, ob `id` eine gültige Kebab-Kennung ist: `^[a-z0-9]+(-[a-z0-9]+)*$` — nichtleere Segmente
/// aus Kleinbuchstaben/Ziffern, durch einzelne Bindestriche getrennt, kein führender/abschließender
/// Bindestrich, keine Doppel-Bindestriche. Rein und total.
pub fn is_kebab_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let mut segments = id.split('-');
    segments.all(|seg| !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// Validiert einen Baustein vor dem Speichern (Issue #108, ADR 0003). **Reiner Kern**, kein I/O.
///
/// Regeln (Handoff §1.9):
/// - `id`: nichtleer, Kebab-Format. **Beim Anlegen** (`is_create == true`) zusätzlich eindeutig — die
///   Kennung darf noch nicht in der Bibliothek liegen. **Beim Bearbeiten** (`is_create == false`) ist
///   die `id` unveränderlich und der Schreibpfad ein Upsert auf genau diese `id` — daher wird die
///   Eindeutigkeit dort NICHT geprüft (sonst würde jedes Bearbeiten als Kollision durchfallen).
/// - `name`, `heimat`: nichtleer (getrimmt).
/// - `globs`: mindestens ein nichtleerer (getrimmter) Eintrag.
/// - Sub-Record-Globs (Default-/Paar-Kanten) nichtleer; `paar_default_kanten.partner_id` nichtleer.
/// - Selbst-Referenz (`partner_id == id`) verboten (harter Fehler).
/// - Partner-Existenz: **weiche** Warnung, kein harter Fehler.
///
/// `existing_ids` = die Kennungen aller bereits in der Bibliothek liegenden Bausteine (Quelle für die
/// Anlege-Eindeutigkeit und die Partner-Existenz-Warnung). Beim Bearbeiten darf der eigene Baustein
/// darin enthalten sein (Upsert).
pub fn validate_baustein(b: &Baustein, existing_ids: &[String], is_create: bool) -> ValidationReport {
    let mut r = ValidationReport::default();

    if b.id.is_empty() {
        r.err("id", "Kennung darf nicht leer sein");
    } else if !is_kebab_id(&b.id) {
        r.err("id", "Nur Kleinbuchstaben, Ziffern, Bindestriche");
    } else if is_create && existing_ids.iter().any(|x| x == &b.id) {
        r.err("id", "Kennung schon vergeben");
    }

    if b.name.trim().is_empty() {
        r.err("name", "Name darf nicht leer sein");
    }
    if b.heimat.trim().is_empty() {
        r.err("heimat", "Heimat ist erforderlich");
    }

    let live_globs = b.globs.iter().filter(|g| !g.trim().is_empty()).count();
    if live_globs == 0 {
        r.err("globs", "Mindestens ein Artefakt-Muster");
    }

    // Rekonstruierbar (E50b): jede Regel braucht ein Framework-Muster UND mindestens ein gepinntes
    // Manifest. Ein Muster ohne Manifest wäre nur ein Ignore — der ehrliche Vertrag „Quelle + Manifest"
    // verlangt das rekonstruierende Rezept, sonst verspräche er Wiederherstellbarkeit, die er nicht hat.
    for k in &b.rekonstruierbar {
        let manifest_live = k.manifest.iter().filter(|m| !m.trim().is_empty()).count();
        if k.framework.trim().is_empty() || manifest_live == 0 {
            r.err("rekonstruierbar", "Rekonstruierbar braucht ein Framework-Muster und ein gepinntes Manifest");
            break;
        }
    }

    for k in &b.default_kanten {
        if k.derived_glob.trim().is_empty() || k.source_glob.trim().is_empty() {
            r.err("default_kanten", "Default-Kanten brauchen Quelle und Ableitung");
            break;
        }
    }

    for k in &b.paar_default_kanten {
        if k.derived_glob.trim().is_empty() || k.source_glob.trim().is_empty() {
            r.err("paar_default_kanten", "Paar-Kanten brauchen Quelle und Ableitung");
            break;
        }
    }
    for k in &b.paar_default_kanten {
        if k.partner_id.trim().is_empty() {
            r.err("paar_default_kanten", "Paar-Kanten brauchen einen Partner-Baustein");
            break;
        }
    }
    for k in &b.paar_default_kanten {
        if k.partner_id == b.id && !b.id.is_empty() {
            r.err("paar_default_kanten", "Ein Baustein kann nicht sein eigener Partner sein");
            break;
        }
    }
    // Partner-Existenz: weiche Warnung (degradieren, nie krachen).
    for k in &b.paar_default_kanten {
        let pid = k.partner_id.trim();
        if !pid.is_empty() && pid != b.id && !existing_ids.iter().any(|x| x == pid) {
            r.warnings.push(format!(
                "Partner „{pid}“ liegt nicht in der Bibliothek — der Vorschlag greift erst, wenn er existiert."
            ));
        }
    }

    r
}

/// Entfernt **exakte** Duplikat-Globs aus der geordneten Glob-Liste (Reihenfolge bleibt; das erste
/// Vorkommen gewinnt — die Hauptdatei-Regel `[0]` bleibt also erhalten). Rein und total (Handoff §1.9).
pub fn dedup_globs(globs: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    globs.iter().filter(|g| seen.insert((*g).clone())).cloned().collect()
}

/// Ob eine Kennung zu einem **gebündelten** Default gehört (Issue #108). Rein und total: ein einfacher
/// Mitgliedschaftstest gegen die Kennungen der mitgelieferten Bausteine. Trägt die Boomerang-Schranke
/// fürs Löschen — ein gebündelter Baustein würde beim nächsten Start ohnehin wieder eingesät, darum
/// ist er nicht löschbar. Dieselbe Quelle speist auch das Herkunft-Etikett (mitgeliefert vs. eigen).
pub fn is_bundled_id(id: &str, bundled_ids: &[String]) -> bool {
    bundled_ids.iter().any(|x| x == id)
}

/// Ein **Toolstack**: eine benannte, geordnete Auswahl von Baustein-`id`s aus der Bibliothek
/// (ADR 0003). Repräsentiert einen Standard-Toolstack, aus dem ein Produkt-Stack kopiert wird.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolstack {
    /// Stabile Kebab-Identität, z.B. `"standard-hw"`.
    pub id: String,
    /// Menschlicher Name, z.B. `"Standard Hardware"`.
    pub name: String,
    /// Referenzierte Baustein-`id`s in Reihenfolge.
    pub baustein_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(id: &str, globs: &[&str], oeffnen: Oeffnen) -> Baustein {
        Baustein {
            id: id.to_string(),
            version: 1,
            name: id.to_string(),
            heimat: "elektronik".to_string(),
            globs: globs.iter().map(|s| s.to_string()).collect(),
            ignore: vec![],
            lfs: vec![],
            rekonstruierbar: vec![],
            oeffnen,
            startaufgaben: vec![],
            default_kanten: vec![],
            paar_default_kanten: vec![],
            stillgelegt: false,
        }
    }

    #[test]
    fn hauptdatei_is_the_first_ordered_glob() {
        // table: globs -> expected Hauptdatei-Glob
        let cases: &[(&[&str], Option<&str>)] = &[
            (&[], None),
            (&["*.kicad_pro"], Some("*.kicad_pro")),
            (&["*.kicad_pro", "*.kicad_sch", "*.kicad_pcb"], Some("*.kicad_pro")),
        ];
        for (globs, expected) in cases {
            let bs = b("kicad", globs, Oeffnen::Auto);
            assert_eq!(bs.hauptdatei_glob(), *expected, "globs = {globs:?}");
        }
    }

    #[test]
    fn resolve_oeffnen_covers_the_decision_table() {
        // table: (configured, has_dominant_file) -> resolved
        let cases: &[(Oeffnen, bool, Oeffnen)] = &[
            (Oeffnen::Auto, true, Oeffnen::Datei),
            (Oeffnen::Auto, false, Oeffnen::Ordner),
            (Oeffnen::Datei, true, Oeffnen::Datei),
            (Oeffnen::Datei, false, Oeffnen::Datei),
            (Oeffnen::Ordner, true, Oeffnen::Ordner),
            (Oeffnen::Ordner, false, Oeffnen::Ordner),
        ];
        for (configured, dominant, expected) in cases {
            let bs = b("x", &["*.f3d"], *configured);
            assert_eq!(
                bs.resolve_oeffnen(*dominant),
                *expected,
                "configured = {configured:?}, dominant = {dominant}"
            );
        }
    }

    #[test]
    fn oeffnen_defaults_to_auto() {
        assert_eq!(Oeffnen::default(), Oeffnen::Auto);
    }

    #[test]
    fn round_trips_through_json_with_defaults_omitted_readable() {
        let bs = Baustein {
            id: "fusion".to_string(),
            version: 2,
            name: "Fusion 360".to_string(),
            heimat: "mechanik".to_string(),
            globs: vec!["*.f3d".to_string(), "*.step".to_string()],
            ignore: vec![],
            lfs: vec!["*.f3d".to_string()],
            rekonstruierbar: vec![RekonstruierbarRegel {
                framework: "modules/".to_string(),
                manifest: vec!["west.yml".to_string()],
            }],
            oeffnen: Oeffnen::Auto,
            startaufgaben: vec![Startaufgabe {
                titel: "Stückliste prüfen".to_string(),
                typ: AufgabenTyp::Aufgabe,
                blockiert: true,
            }],
            default_kanten: vec![DefaultKante {
                derived_glob: "*.stl".to_string(),
                source_glob: "*.f3d".to_string(),
            }],
            paar_default_kanten: vec![PaarDefaultKante {
                partner_id: "kicad".to_string(),
                derived_glob: "*.pos".to_string(),
                source_glob: "*.kicad_pcb".to_string(),
            }],
            stillgelegt: false,
        };
        let json = serde_json::to_string_pretty(&bs).unwrap();
        let back: Baustein = serde_json::from_str(&json).unwrap();
        assert_eq!(bs, back);
    }

    #[test]
    fn is_kebab_id_table() {
        // table: id -> valid?
        let cases: &[(&str, bool)] = &[
            ("", false),
            ("kicad", true),
            ("standard-hw", true),
            ("a1-b2-c3", true),
            ("Fusion", false),    // uppercase
            ("ki cad", false),    // space
            ("-kicad", false),    // leading dash
            ("kicad-", false),    // trailing dash
            ("ki--cad", false),   // double dash
            ("ki_cad", false),    // underscore
            ("käse", false),      // umlaut
            ("1", true),
            ("1-2", true),
        ];
        for (id, expected) in cases {
            assert_eq!(is_kebab_id(id), *expected, "id = {id:?}");
        }
    }

    fn full(mut b: Baustein, f: impl FnOnce(&mut Baustein)) -> Baustein {
        f(&mut b);
        b
    }

    #[test]
    fn validate_baustein_field_rules() {
        let base = b("kicad", &["*.kicad_pro"], Oeffnen::Auto);
        let existing = vec!["kicad".to_string(), "fusion".to_string()];

        // Happy path: clean Baustein has no errors (edit path — its own id may be in `existing`).
        assert!(validate_baustein(&base, &existing, false).ok());

        // table: (mutate, expected error field)
        let cases: Vec<(Baustein, &str)> = vec![
            (full(base.clone(), |b| b.id = String::new()), "id"),
            (full(base.clone(), |b| b.id = "Bad ID".into()), "id"),
            (full(base.clone(), |b| b.name = "   ".into()), "name"),
            (full(base.clone(), |b| b.heimat = "".into()), "heimat"),
            (full(base.clone(), |b| b.globs = vec![]), "globs"),
            (full(base.clone(), |b| b.globs = vec!["   ".into()]), "globs"),
            (
                full(base.clone(), |b| {
                    b.default_kanten = vec![DefaultKante { derived_glob: "*.stl".into(), source_glob: "".into() }]
                }),
                "default_kanten",
            ),
            (
                full(base.clone(), |b| {
                    b.paar_default_kanten = vec![PaarDefaultKante {
                        partner_id: "".into(),
                        derived_glob: "*.pos".into(),
                        source_glob: "*.kicad_pcb".into(),
                    }]
                }),
                "paar_default_kanten",
            ),
            (
                full(base.clone(), |b| {
                    b.paar_default_kanten = vec![PaarDefaultKante {
                        partner_id: "fusion".into(),
                        derived_glob: "".into(),
                        source_glob: "*.kicad_pcb".into(),
                    }]
                }),
                "paar_default_kanten",
            ),
            // self-reference forbidden
            (
                full(base.clone(), |b| {
                    b.paar_default_kanten = vec![PaarDefaultKante {
                        partner_id: "kicad".into(),
                        derived_glob: "*.pos".into(),
                        source_glob: "*.kicad_pcb".into(),
                    }]
                }),
                "paar_default_kanten",
            ),
        ];
        for (bs, field) in cases {
            let r = validate_baustein(&bs, &existing, false);
            assert!(!r.ok(), "expected error for field {field}, baustein = {bs:?}");
            assert!(
                r.errors.iter().any(|(f, _)| f == field),
                "expected an error on field {field}, got {:?}",
                r.errors
            );
        }
    }

    #[test]
    fn create_time_uniqueness_blocks_a_colliding_id_but_edit_does_not() {
        let existing = vec!["kicad".to_string(), "fusion".to_string()];

        // CREATE with an id already in the Bibliothek ⇒ hard error on `id`.
        let collide = b("kicad", &["*.kicad_pro"], Oeffnen::Auto);
        let r = validate_baustein(&collide, &existing, true);
        assert!(!r.ok(), "create with a colliding id must be blocked");
        assert!(
            r.errors.iter().any(|(f, m)| f == "id" && m == "Kennung schon vergeben"),
            "expected the dedup id error, got {:?}",
            r.errors
        );

        // CREATE with a fresh id ⇒ fine.
        let fresh = b("freecad", &["*.fcstd"], Oeffnen::Auto);
        assert!(
            validate_baustein(&fresh, &existing, true).ok(),
            "create with a fresh id must pass"
        );

        // EDIT of an existing record (its own id is in `existing`) ⇒ NOT a collision (upsert).
        assert!(
            validate_baustein(&collide, &existing, false).ok(),
            "edit-save of an existing baustein must not trip the uniqueness rule"
        );
    }

    #[test]
    fn dangling_partner_is_a_soft_warning_not_an_error() {
        let existing = vec!["kicad".to_string()];
        let bs = full(b("kicad", &["*.kicad_pro"], Oeffnen::Auto), |b| {
            b.paar_default_kanten = vec![PaarDefaultKante {
                partner_id: "ghost".into(), // not in existing
                derived_glob: "*.pos".into(),
                source_glob: "*.kicad_pcb".into(),
            }];
        });
        let r = validate_baustein(&bs, &existing, false);
        assert!(r.ok(), "dangling partner must NOT be a hard error: {:?}", r.errors);
        assert_eq!(r.warnings.len(), 1, "dangling partner should warn once");
        assert!(r.warnings[0].contains("ghost"));
    }

    #[test]
    fn dedup_globs_keeps_order_and_first_occurrence() {
        let cases: &[(&[&str], &[&str])] = &[
            (&[], &[]),
            (&["*.a"], &["*.a"]),
            (&["*.a", "*.b", "*.a"], &["*.a", "*.b"]),
            (&["*.a", "*.a", "*.a"], &["*.a"]),
            (&["*.pro", "*.sch", "*.pcb"], &["*.pro", "*.sch", "*.pcb"]),
        ];
        for (input, expected) in cases {
            let got = dedup_globs(&input.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            let want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(got, want, "input = {input:?}");
        }
    }

    #[test]
    fn is_bundled_id_table() {
        let bundled: Vec<String> = ["kicad", "fusion"].iter().map(|s| s.to_string()).collect();
        // gebündelte Kennung → erkannt (löschen geblockt, Etikett „mitgeliefert")
        assert!(is_bundled_id("kicad", &bundled));
        assert!(is_bundled_id("fusion", &bundled));
        // nutzer-eigene Kennung → nicht gebündelt (löschbar, Etikett „eigen")
        assert!(!is_bundled_id("meine-cnc", &bundled));
        // leere Quelle → nichts ist gebündelt
        assert!(!is_bundled_id("kicad", &[]));
    }

    #[test]
    fn deserializes_with_missing_optional_fields() {
        // Only the required fields present: optional Vecs/flags fall back to defaults.
        let json = r#"{
            "id": "doku",
            "version": 1,
            "name": "Doku",
            "heimat": "doku",
            "globs": ["*.md", "*.pdf"]
        }"#;
        let bs: Baustein = serde_json::from_str(json).unwrap();
        assert_eq!(bs.id, "doku");
        assert!(bs.ignore.is_empty());
        assert!(bs.lfs.is_empty());
        assert_eq!(bs.oeffnen, Oeffnen::Auto);
        assert!(bs.rekonstruierbar.is_empty());
        assert!(bs.default_kanten.is_empty());
        assert!(bs.paar_default_kanten.is_empty());
        assert!(!bs.stillgelegt);
    }

    /// Rekonstruierbar (E50b): eine Regel braucht ein Framework-Muster UND ein gepinntes Manifest.
    /// Ein Muster ohne Manifest (oder ein Manifest ohne Muster) ist ein harter Fehler — der ehrliche
    /// „Quelle + Manifest"-Vertrag verlangt das rekonstruierende Rezept.
    #[test]
    fn rekonstruierbar_needs_framework_and_a_pinned_manifest() {
        let base = b("zephyr", &["*.c"], Oeffnen::Auto);
        let existing = vec!["zephyr".to_string()];

        // Happy path: Framework-Muster + gepinntes Manifest -> speicherbar.
        let ok = full(base.clone(), |b| {
            b.rekonstruierbar = vec![RekonstruierbarRegel {
                framework: "modules/".into(),
                manifest: vec!["west.yml".into()],
            }];
        });
        assert!(validate_baustein(&ok, &existing, false).ok(), "Quelle + Manifest muss passen");

        // table: (mutate) -> expects a hard error on `rekonstruierbar`
        let bad: Vec<Baustein> = vec![
            // Framework-Muster fehlt
            full(base.clone(), |b| {
                b.rekonstruierbar = vec![RekonstruierbarRegel {
                    framework: "   ".into(),
                    manifest: vec!["west.yml".into()],
                }];
            }),
            // Manifest fehlt -> wäre nur ein Ignore, kein rekonstruierendes Rezept
            full(base.clone(), |b| {
                b.rekonstruierbar = vec![RekonstruierbarRegel {
                    framework: "modules/".into(),
                    manifest: vec![],
                }];
            }),
            // Manifest nur aus Leerzeichen -> zählt als fehlend
            full(base.clone(), |b| {
                b.rekonstruierbar = vec![RekonstruierbarRegel {
                    framework: "modules/".into(),
                    manifest: vec!["  ".into()],
                }];
            }),
        ];
        for bs in bad {
            let r = validate_baustein(&bs, &existing, false);
            assert!(!r.ok(), "erwartete Fehler, baustein = {bs:?}");
            assert!(
                r.errors.iter().any(|(f, _)| f == "rekonstruierbar"),
                "Fehler auf rekonstruierbar erwartet, got {:?}",
                r.errors
            );
        }
    }
}
