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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AufgabenTyp {
    /// Verpflichtend; *kann* blockieren (siehe `blockiert`).
    Aufgabe,
    /// Blockiert nie.
    Hinweis,
}

/// Eine Startaufgabe, die beim Onboarding eines Bausteins in einem Produkt angelegt wird.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaarDefaultKante {
    /// `id` des Partner-Bausteins, der zusätzlich im Stack liegen muss, damit der Vorschlag greift.
    pub partner_id: String,
    /// Glob des abgeleiteten Artefakts (z.B. Pick-and-Place).
    pub derived_glob: String,
    /// Glob der Quelle, aus der es stammt (z.B. Layout **und** BOM — je eine Paar-Kante).
    pub source_glob: String,
}

/// Ein **Baustein**: das wiederverwendbare Tool-Wissen für ein Tool (ADR 0003).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Ein **Toolstack**: eine benannte, geordnete Auswahl von Baustein-`id`s aus der Bibliothek
/// (ADR 0003). Repräsentiert einen Standard-Toolstack, aus dem ein Produkt-Stack kopiert wird.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        assert!(bs.default_kanten.is_empty());
        assert!(bs.paar_default_kanten.is_empty());
        assert!(!bs.stillgelegt);
    }
}
