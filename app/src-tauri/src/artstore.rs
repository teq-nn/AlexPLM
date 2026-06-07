//! Per-Baustein Revision-Art persistence (Issue #41 E42, **erweitert um Issue #131 / E51a**).
//!
//! Thin, side-effecting layer that stores the **Art** (Prototyp/Freigabe) of every Revision. All
//! filesystem access lives here; the pure toggle state machine in
//! [`crate::graph::toggle_revision_art`] never does I/O. Same split as `edgestore.rs` over
//! `edges.rs` and `graphread.rs` over `graph.rs`.
//!
//! ## Scope = Heimat-Ordner (E51a, Issue #131)
//!
//! Die Art wandert von der **produkt-globalen** Revision auf die **Baustein-Revision**: jeder
//! Baustein trägt seine **eigene** Revision + Art, der Scope ist sein **Heimat-Ordner**. Der
//! HW-Entwickler kann `elektronik` als „Rev B" freigeben, ohne dass WIP-Firmware ihn blockiert —
//! jeder Bereich reift unabhängig. Darum ist die Art-Map **pro Heimat** geführt (`heimat → version
//! → token`) statt produkt-global flach.
//!
//! The store holds **only what git cannot know** (E8/E18): git carries the tag (the version label
//! and which commit it points at); the Art is the one PLM fact layered on top, so a tag with no
//! recorded Art is simply the default **Prototyp** (lax — E42), never an error.
//!
//! ## Schema-Migration (E51a)
//!
//! Die alte `revisionen.json`-Form war eine **flache** `version → token`-Map (produkt-global). Die
//! neue Form ist ein versioniertes Dokument mit einer **Heimat-Achse**. Bestehende Daten werden
//! beim Lesen **transparent migriert**: eine alte flache Map landet im **produkt-globalen
//! Heimat-Scope** ([`GLOBAL_HEIMAT`]), damit keine bereits freigegebene Revision verschwindet.
//! Treu zur Degradations-Invariante (E22): fehlend/leer/kaputt ⇒ leerer Zustand, nie Fehler.

use crate::graph::RevisionArt;
use crate::plmstore::PlmDocument;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// File that holds the per-Heimat Revision-Art map, inside `_plm/` (ADR 0002).
pub const ART_FILE: &str = "revisionen.json";

/// Aktuelle Schema-Version des `revisionen.json`-Dokuments (E51a). `1` war die alte flache,
/// produkt-globale `version → token`-Map; `2` führt die **Heimat-Achse** ein.
const SCHEMA_VERSION: u32 = 2;

/// Der **produkt-globale** Heimat-Scope (E51a). Bestehende, vor der Heimat-Achse geschriebene
/// Arten landen bei der Migration hier, und ein Aufrufer ohne Baustein-Bezug (eine produkt-weite
/// Revision) nutzt denselben Scope. Ein leerer String wäre zweideutig zu „kein Heimat-Ordner",
/// darum ein stabiles, nie als echter Ordnername vorkommendes Sentinel.
pub const GLOBAL_HEIMAT: &str = "*produkt*";

/// Das `_plm/revisionen.json`-Dokument (Schema 2, E51a): pro **Heimat** eine version-label → Art-
/// token-Map. `BTreeMap` hält die Schlüssel geordnet, damit die Datei stabil und diffbar bleibt.
/// `schema` macht die Form selbstbeschreibend und künftige Migrationen erkennbar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct RevisionenDoc {
    /// Schema-Version dieses Dokuments. Fehlt sie (alte flache Form), migriert [`read_doc`].
    #[serde(default)]
    schema: u32,
    /// Pro Heimat-Ordner eine `version-label → Art-token`-Map. Der produkt-globale Scope liegt
    /// unter [`GLOBAL_HEIMAT`].
    #[serde(default)]
    heimaten: BTreeMap<String, BTreeMap<String, String>>,
}

/// Die rohe, **alte** flache Form (Schema 1): eine produkt-globale `version → token`-Map. Nur zum
/// Erkennen+Migrieren bestehender Dateien gelesen, nie mehr geschrieben.
type LegacyFlat = BTreeMap<String, String>;

/// Path, degradation and pretty/atomic write live in the deep [`PlmDocument`] layer; this store is
/// the per-Heimat Art domain over it. Das Dokument wird als **roher** JSON-Wert gelesen, weil wir
/// zwischen der neuen ([`RevisionenDoc`]) und der alten flachen ([`LegacyFlat`]) Form unterscheiden
/// müssen — beides degradiert sauber zu „leer".
const ART: PlmDocument<serde_json::Value> = PlmDocument::new(ART_FILE);

/// Read the whole document, **migrating** an old flat map into the new Heimat-scoped shape. A
/// missing/empty/corrupt file means an empty document (every tag then reads as the default
/// Prototyp) — never an error (E22). Die alte flache Form (Schema 1) wird in den produkt-globalen
/// Scope ([`GLOBAL_HEIMAT`]) gehoben, damit keine bereits aufgezeichnete Art verloren geht.
fn read_doc(root: &Path) -> RevisionenDoc {
    let raw = ART.read(root);
    if raw.is_null() {
        // Fehlend/leer/kaputt ⇒ leeres Dokument (Degradation, nie Fehler).
        return RevisionenDoc::default();
    }
    // 1) Neue Form: ein Objekt mit `heimaten`/`schema`. Serde-Default deckt fehlende Felder ab.
    if let Ok(doc) = serde_json::from_value::<RevisionenDoc>(raw.clone()) {
        // Ein altes flaches `{ "v1.0": "freigabe" }` deserialisiert ebenfalls (alle Felder
        // defaulten zu leer), liefert aber ein leeres `heimaten` — erkennbar und ein Fall für die
        // Migration unten. Nur wenn `heimaten` tatsächlich gefüllt ist, ist es die neue Form.
        if !doc.heimaten.is_empty() {
            return doc;
        }
    }
    // 2) Alte flache Form (Schema 1): in den produkt-globalen Scope migrieren.
    if let Ok(flat) = serde_json::from_value::<LegacyFlat>(raw) {
        if flat.is_empty() {
            return RevisionenDoc::default();
        }
        let mut heimaten = BTreeMap::new();
        heimaten.insert(GLOBAL_HEIMAT.to_string(), flat);
        return RevisionenDoc {
            schema: SCHEMA_VERSION,
            heimaten,
        };
    }
    // 3) Unverständlich ⇒ leer (Degradation).
    RevisionenDoc::default()
}

/// Persist the document (pretty + atomic, creating `_plm/` as needed), always at the current
/// schema version so the on-disk form is self-describing.
fn write_doc(root: &Path, mut doc: RevisionenDoc) -> std::io::Result<()> {
    doc.schema = SCHEMA_VERSION;
    let value = serde_json::to_value(&doc).map_err(std::io::Error::other)?;
    ART.write(root, &value)
}

/// The recorded [`RevisionArt`] for a version label **within a Heimat scope** (E51a). A tag with no
/// recorded Art is the default **Prototyp** (E42) — a freshly promoted Baustein-Revision is lax
/// until toggled. `heimat` is the Baustein's Heimat-Ordner; pass [`GLOBAL_HEIMAT`] for a
/// produkt-weite Revision.
pub fn read_art_in(root: &Path, heimat: &str, version: &str) -> RevisionArt {
    match read_doc(root).heimaten.get(heimat).and_then(|m| m.get(version)) {
        Some(token) => RevisionArt::from_token(token),
        None => RevisionArt::default(),
    }
}

/// Record the [`RevisionArt`] for a version label **within a Heimat scope** and persist it
/// (E51a). Returns the stored Art.
pub fn set_art_in(
    root: &Path,
    heimat: &str,
    version: &str,
    art: RevisionArt,
) -> std::io::Result<RevisionArt> {
    let mut doc = read_doc(root);
    doc.heimaten
        .entry(heimat.to_string())
        .or_default()
        .insert(version.to_string(), art.as_token().to_string());
    write_doc(root, doc)?;
    Ok(art)
}

/// The recorded [`RevisionArt`] for a **produkt-globale** Revision (legacy/no-Baustein scope).
/// Convenience over [`read_art_in`] with [`GLOBAL_HEIMAT`]; keeps the produkt-weite Aufrufer
/// (E47-Push, der Versionsbalken) kurz.
pub fn read_art(root: &Path, version: &str) -> RevisionArt {
    read_art_in(root, GLOBAL_HEIMAT, version)
}

/// Record the [`RevisionArt`] for a **produkt-globale** Revision (legacy/no-Baustein scope).
pub fn set_art(root: &Path, version: &str, art: RevisionArt) -> std::io::Result<RevisionArt> {
    set_art_in(root, GLOBAL_HEIMAT, version, art)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-art-ut-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_reads_as_prototyp() {
        let dir = tmp();
        // A tag the tool has never seen is lax by default (E42), in any Heimat scope.
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        assert_eq!(read_art_in(&dir, "elektronik", "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_then_read_round_trips_per_tag() {
        let dir = tmp();
        set_art(&dir, "v1.0", RevisionArt::Freigabe).unwrap();
        set_art(&dir, "v0.9", RevisionArt::Prototyp).unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art(&dir, "v0.9"), RevisionArt::Prototyp);
        // An untouched tag is still the default.
        assert_eq!(read_art(&dir, "v0.1"), RevisionArt::Prototyp);

        // Un-Release: flipping back is persisted (E42 reversible).
        set_art(&dir, "v1.0", RevisionArt::Prototyp).unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    /// **E51a Kern**: jeder Baustein trägt seine **eigene** Art mit Scope = Heimat. Dieselbe
    /// Versionsmarke kann in zwei Heimaten unabhängig reifen — `elektronik` freigegeben, `firmware`
    /// noch Prototyp — ohne sich gegenseitig zu blockieren.
    #[test]
    fn art_is_scoped_per_heimat_and_independent() {
        let dir = tmp();
        // Der HW-Entwickler gibt elektronik frei …
        set_art_in(&dir, "elektronik", "v1.0", RevisionArt::Freigabe).unwrap();
        // … die WIP-Firmware bleibt für dieselbe Versionsmarke Prototyp.
        set_art_in(&dir, "firmware", "v1.0", RevisionArt::Prototyp).unwrap();

        assert_eq!(read_art_in(&dir, "elektronik", "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art_in(&dir, "firmware", "v1.0"), RevisionArt::Prototyp);
        // Eine dritte Heimat hat die Marke nie gesehen ⇒ Default Prototyp.
        assert_eq!(read_art_in(&dir, "mechanik", "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_degrades_to_prototyp() {
        let dir = tmp();
        fs::create_dir_all(ART.path(&dir).parent().unwrap()).unwrap();
        fs::write(ART.path(&dir), "{ not json ]").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        assert_eq!(read_art_in(&dir, "elektronik", "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_to_the_new_plm_location() {
        let dir = tmp();
        set_art(&dir, "v1.0", RevisionArt::Freigabe).unwrap();
        assert!(
            ART.path(&dir).is_file(),
            "revision art lives in _plm/revisionen.json"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// **E51a Schema-Migration**: eine vorhandene **alte flache** `version → token`-Datei (Schema 1,
    /// produkt-global geschrieben, bevor es die Heimat-Achse gab) wird beim Lesen transparent in den
    /// produkt-globalen Scope gehoben — keine bereits freigegebene Revision verschwindet.
    #[test]
    fn migrates_old_flat_map_into_the_global_heimat_scope() {
        let dir = tmp();
        // Schreibe die ALTE flache Form von Hand (so lag sie auf der Platte vor #131).
        fs::create_dir_all(ART.path(&dir).parent().unwrap()).unwrap();
        fs::write(
            ART.path(&dir),
            r#"{ "v1.0": "freigabe", "v0.9": "prototyp" }"#,
        )
        .unwrap();

        // Lesen migriert nach GLOBAL_HEIMAT — die Freigabe überlebt.
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art(&dir, "v0.9"), RevisionArt::Prototyp);
        // … und ist unter dem expliziten globalen Scope sichtbar.
        assert_eq!(read_art_in(&dir, GLOBAL_HEIMAT, "v1.0"), RevisionArt::Freigabe);

        // Ein Schreibvorgang persistiert das Dokument in der NEUEN Form (Schema 2, mit Heimat-Achse),
        // ohne die migrierten Daten zu verlieren — Round-Trip über die Schema-Grenze.
        set_art_in(&dir, "elektronik", "v2.0", RevisionArt::Freigabe).unwrap();
        let doc = read_doc(&dir);
        assert_eq!(doc.schema, SCHEMA_VERSION, "auf aktuelles Schema gehoben");
        assert_eq!(
            doc.heimaten.get(GLOBAL_HEIMAT).and_then(|m| m.get("v1.0")).map(String::as_str),
            Some("freigabe"),
            "migrierte produkt-globale Freigabe bleibt erhalten"
        );
        assert_eq!(
            doc.heimaten.get("elektronik").and_then(|m| m.get("v2.0")).map(String::as_str),
            Some("freigabe"),
            "neue Baustein-Revision steht im Heimat-Scope"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// **Round-Trip der neuen Form**: das geschriebene Schema-2-Dokument liest sich verlustfrei
    /// zurück, mit zwei voneinander unabhängigen Heimat-Scopes.
    #[test]
    fn new_shape_round_trips_across_heimaten() {
        let dir = tmp();
        set_art_in(&dir, "elektronik", "v1.0", RevisionArt::Freigabe).unwrap();
        set_art_in(&dir, "firmware", "v1.0", RevisionArt::Prototyp).unwrap();
        set_art(&dir, "v9.9", RevisionArt::Freigabe).unwrap(); // produkt-global

        // Aus einem frisch gelesenen Dokument (kein In-Memory-Cache) zurückgewonnen.
        assert_eq!(read_art_in(&dir, "elektronik", "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art_in(&dir, "firmware", "v1.0"), RevisionArt::Prototyp);
        assert_eq!(read_art(&dir, "v9.9"), RevisionArt::Freigabe);

        // Das Dokument trägt die aktuelle Schema-Marke und drei getrennte Scopes.
        let doc = read_doc(&dir);
        assert_eq!(doc.schema, SCHEMA_VERSION);
        assert_eq!(doc.heimaten.len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Eine leere alte flache Datei (`{}`) ist kein Migrations-Fall — sie degradiert zu „leer".
    #[test]
    fn empty_flat_map_degrades_to_empty() {
        let dir = tmp();
        fs::create_dir_all(ART.path(&dir).parent().unwrap()).unwrap();
        fs::write(ART.path(&dir), "{}").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        assert!(read_doc(&dir).heimaten.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
