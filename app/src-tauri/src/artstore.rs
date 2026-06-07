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
use crate::plmstore::PlmCollection;
use std::collections::BTreeMap;
use std::path::Path;

/// Legacy single-file location of the Revision-Art map, inside `_plm/` (ADR 0002). It carried either
/// the **flache** produkt-globale `version → token`-Map (vor E51a) oder das Schema-2-Dokument mit
/// Heimat-Achse (E51a). Beide Formen werden beim Lesen transparent in die per-Eintrag-Ablage unter
/// `_plm/revisionen/` migriert (E54, Issue #132).
pub const ART_FILE: &str = "revisionen.json";
/// Per-entry directory holding one JSON file per Release-Pointer (E54). Jeder Eintrag ist ein
/// `(Heimat, version)`-Paar; der Schlüssel kodiert beide Achsen kollisionsfrei (siehe [`entry_key`]).
pub const ART_DIR: &str = "revisionen";

/// Der **produkt-globale** Heimat-Scope (E51a). Bestehende, vor der Heimat-Achse geschriebene
/// Arten landen bei der Migration hier, und ein Aufrufer ohne Baustein-Bezug (eine produkt-weite
/// Revision) nutzt denselben Scope. Ein leerer String wäre zweideutig zu „kein Heimat-Ordner",
/// darum ein stabiles, nie als echter Ordnername vorkommendes Sentinel.
pub const GLOBAL_HEIMAT: &str = "*produkt*";

/// The Release-Pointer collection — **one ID-named file per recorded Revision-Art** (E54, Issue
/// #132), unter `_plm/revisionen/`, mit Migration von der alten Einzeldatei `_plm/revisionen.json`.
/// Der Eintragsschlüssel kodiert die **Heimat-Achse** (E51a, Issue #131) zusammen mit dem
/// Versions-Label (siehe [`entry_key`]); der Payload ist das Art-token. Zwei auf zwei Seiten
/// promotete `(Heimat, version)`-Paare landen in zwei Dateien, kollidieren im Merge also nie. Pfad,
/// Per-Datei-Degradation und der atomare Pretty-Write leben in der tiefen [`PlmCollection`]-Schicht;
/// dieser Store ist die Heimat-skopierte Art-Domäne darüber.
const ART: PlmCollection<String> = PlmCollection::new(ART_DIR, ART_FILE);

/// Encode a `(heimat, version)` pair into a single, **collision-free** collection key (E51a × E54).
/// Längen-präfigiert (`<heimat-byte-len>|<heimat>|<version>`), damit weder ein `|` in der Heimat
/// noch im Versions-Label zwei verschiedene Paare auf denselben Schlüssel — und damit dieselbe Datei
/// — abbilden kann. [`PlmCollection`] escaped den Schlüssel zusätzlich für den Dateinamen, der
/// In-Memory-Schlüssel hier ist aber schon für sich injektiv.
fn entry_key(heimat: &str, version: &str) -> String {
    format!("{}|{}|{}", heimat.len(), heimat, version)
}

/// Inverse of [`entry_key`]: split a stored key back into `(heimat, version)`. A malformed key
/// (hand-edited, falsche Länge) degradiert zu `None` und wird vom Aufrufer übersprungen — nie ein
/// Fehler (E22).
fn split_entry_key(key: &str) -> Option<(String, String)> {
    let (len_str, rest) = key.split_once('|')?;
    let n: usize = len_str.parse().ok()?;
    if rest.len() < n {
        return None;
    }
    let (heimat, tail) = rest.split_at(n);
    let version = tail.strip_prefix('|')?;
    Some((heimat.to_string(), version.to_string()))
}

/// In-memory domain shape (E51a): pro **Heimat** eine `version-label → Art-token`-Map. Wird aus der
/// per-Eintrag-Ablage (und der migrierten Altdaten) zusammengesetzt und zurückgeschrieben.
type Heimaten = BTreeMap<String, BTreeMap<String, String>>;

/// The raw Schema-2 document the legacy single file may have carried (E51a). Nur zum
/// Erkennen+Migrieren bestehender Dateien gelesen.
#[derive(serde::Deserialize)]
struct LegacyDoc {
    #[serde(default)]
    heimaten: Heimaten,
}

/// Read the whole `heimat → version → token` model. Liest zuerst die per-Eintrag-Ablage unter
/// `_plm/revisionen/` (jede Datei ist ein `(Heimat, version)`-Eintrag) und faltet — falls diese
/// **fehlt** — die alte Einzeldatei `_plm/revisionen.json` transparent ein (Migration, E54 × E51a):
/// das **Schema-2-Dokument** (mit Heimat-Achse) wie auch die **alte flache** produkt-globale Map
/// (in den [`GLOBAL_HEIMAT`]-Scope gehoben). Missing/leer/kaputt ⇒ leeres Modell, ein einzelner
/// mangled Eintrag wird übersprungen — nie ein Fehler (E22).
fn read_doc(root: &Path) -> Heimaten {
    // 1) Per-Eintrag-Ablage vorhanden? Dann ist sie die Wahrheit (E54).
    if ART.dir_path(root).is_dir() {
        let mut heimaten: Heimaten = BTreeMap::new();
        for (key, token) in ART.read(root) {
            // Ein per Hand verkorkster Schlüssel wird übersprungen (Per-Datei-Degradation).
            if let Some((heimat, version)) = split_entry_key(&key) {
                heimaten.entry(heimat).or_default().insert(version, token);
            }
        }
        return heimaten;
    }
    // 2) Keine Ablage ⇒ aus der alten Einzeldatei migrieren. Roh lesen, um zwischen dem
    //    Schema-2-Dokument und der alten flachen Map zu unterscheiden — beides degradiert zu „leer".
    let raw: serde_json::Value =
        crate::plmstore::read_optional(&root.join(crate::plmstore::PLM_DIR).join(ART_FILE))
            .unwrap_or(serde_json::Value::Null);
    if raw.is_null() {
        return BTreeMap::new();
    }
    // 2a) Schema-2-Dokument mit gefüllter Heimat-Achse (E51a).
    if let Ok(doc) = serde_json::from_value::<LegacyDoc>(raw.clone()) {
        if !doc.heimaten.is_empty() {
            return doc.heimaten;
        }
    }
    // 2b) Alte flache `version → token`-Map: in den produkt-globalen Scope heben (E51a).
    if let Ok(flat) = serde_json::from_value::<BTreeMap<String, String>>(raw) {
        if !flat.is_empty() {
            let mut heimaten: Heimaten = BTreeMap::new();
            heimaten.insert(GLOBAL_HEIMAT.to_string(), flat);
            return heimaten;
        }
    }
    // 2c) Unverständlich/leer ⇒ leer (Degradation).
    BTreeMap::new()
}

/// Persist the whole `heimat → version → token` model as **one file per `(Heimat, version)`-Eintrag**
/// unter `_plm/revisionen/` (pretty + atomic je Datei, E54). Die alte Einzeldatei bleibt als
/// harmloses Sediment liegen.
fn write_doc(root: &Path, heimaten: &Heimaten) -> std::io::Result<()> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for (heimat, versions) in heimaten {
        for (version, token) in versions {
            map.insert(entry_key(heimat, version), token.clone());
        }
    }
    ART.write(root, &map)
}

/// The recorded [`RevisionArt`] for a version label **within a Heimat scope** (E51a). A tag with no
/// recorded Art is the default **Prototyp** (E42) — a freshly promoted Baustein-Revision is lax
/// until toggled. `heimat` is the Baustein's Heimat-Ordner; pass [`GLOBAL_HEIMAT`] for a
/// produkt-weite Revision.
pub fn read_art_in(root: &Path, heimat: &str, version: &str) -> RevisionArt {
    match read_doc(root).get(heimat).and_then(|m| m.get(version)) {
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
    doc.entry(heimat.to_string())
        .or_default()
        .insert(version.to_string(), art.as_token().to_string());
    write_doc(root, &doc)?;
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
        // a single hand-mangled Release-Pointer file is skipped per-file, never fatal.
        let path = ART.entry_path(&dir, &entry_key(GLOBAL_HEIMAT, "v1.0"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json ]").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        assert_eq!(read_art_in(&dir, "elektronik", "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_one_file_per_release_pointer() {
        let dir = tmp();
        set_art(&dir, "v1.0", RevisionArt::Freigabe).unwrap();
        set_art(&dir, "v0.9", RevisionArt::Freigabe).unwrap();
        assert!(
            ART.entry_path(&dir, &entry_key(GLOBAL_HEIMAT, "v1.0")).is_file(),
            "each Release-Pointer is its own file under _plm/revisionen/"
        );
        // E54: two tags' Release-Pointers are two separate files — no merge collision.
        assert_ne!(
            ART.entry_path(&dir, &entry_key(GLOBAL_HEIMAT, "v1.0")),
            ART.entry_path(&dir, &entry_key(GLOBAL_HEIMAT, "v0.9"))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Migration: a product that only has the legacy `_plm/revisionen.json` map file keeps its
    /// recorded Arts (not reset to Prototyp); the next write lays them out one file per tag.
    #[test]
    fn migrates_legacy_revisionen_map_file() {
        let dir = tmp();
        let legacy: BTreeMap<String, String> = BTreeMap::from([
            ("v1.0".to_string(), RevisionArt::Freigabe.as_token().to_string()),
            ("v0.9".to_string(), RevisionArt::Prototyp.as_token().to_string()),
        ]);
        let path = dir.join("_plm").join(ART_FILE);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        // read folds the legacy map in.
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Freigabe);

        // the next write materialises the per-entry directory without losing the recorded Arts.
        set_art(&dir, "v2.0", RevisionArt::Freigabe).unwrap();
        assert!(
            ART.entry_path(&dir, &entry_key(GLOBAL_HEIMAT, "v1.0")).is_file(),
            "legacy Art written out per file"
        );
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art(&dir, "v2.0"), RevisionArt::Freigabe);
        let _ = fs::remove_dir_all(&dir);
    }

    /// **E51a Schema-Migration**: eine vorhandene **alte flache** `version → token`-Datei (Schema 1,
    /// produkt-global geschrieben, bevor es die Heimat-Achse gab) wird beim Lesen transparent in den
    /// produkt-globalen Scope gehoben — keine bereits freigegebene Revision verschwindet.
    #[test]
    fn migrates_old_flat_map_into_the_global_heimat_scope() {
        let dir = tmp();
        // Schreibe die ALTE flache Form von Hand (so lag sie auf der Platte vor #131).
        let legacy_path = dir.join("_plm").join(ART_FILE);
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, r#"{ "v1.0": "freigabe", "v0.9": "prototyp" }"#).unwrap();

        // Lesen migriert nach GLOBAL_HEIMAT — die Freigabe überlebt.
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art(&dir, "v0.9"), RevisionArt::Prototyp);
        // … und ist unter dem expliziten globalen Scope sichtbar.
        assert_eq!(read_art_in(&dir, GLOBAL_HEIMAT, "v1.0"), RevisionArt::Freigabe);

        // Ein Schreibvorgang persistiert das Modell in der per-Eintrag-Ablage (E54, mit Heimat-Achse),
        // ohne die migrierten Daten zu verlieren — Round-Trip über die Schema- und Layout-Grenze.
        set_art_in(&dir, "elektronik", "v2.0", RevisionArt::Freigabe).unwrap();
        let doc = read_doc(&dir);
        assert_eq!(
            doc.get(GLOBAL_HEIMAT).and_then(|m| m.get("v1.0")).map(String::as_str),
            Some("freigabe"),
            "migrierte produkt-globale Freigabe bleibt erhalten"
        );
        assert_eq!(
            doc.get("elektronik").and_then(|m| m.get("v2.0")).map(String::as_str),
            Some("freigabe"),
            "neue Baustein-Revision steht im Heimat-Scope"
        );
        // Per-Eintrag ausgelegt: je `(Heimat, version)`-Paar eine eigene Datei (E54).
        assert!(ART.entry_path(&dir, &entry_key(GLOBAL_HEIMAT, "v1.0")).is_file());
        assert!(ART.entry_path(&dir, &entry_key("elektronik", "v2.0")).is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    /// **Round-Trip der Heimat-skopierten Form** über die per-Eintrag-Ablage (E51a × E54): das
    /// geschriebene Modell liest sich verlustfrei zurück, mit drei voneinander unabhängigen Scopes.
    #[test]
    fn new_shape_round_trips_across_heimaten() {
        let dir = tmp();
        set_art_in(&dir, "elektronik", "v1.0", RevisionArt::Freigabe).unwrap();
        set_art_in(&dir, "firmware", "v1.0", RevisionArt::Prototyp).unwrap();
        set_art(&dir, "v9.9", RevisionArt::Freigabe).unwrap(); // produkt-global

        // Aus einem frisch gelesenen Modell (kein In-Memory-Cache) zurückgewonnen.
        assert_eq!(read_art_in(&dir, "elektronik", "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art_in(&dir, "firmware", "v1.0"), RevisionArt::Prototyp);
        assert_eq!(read_art(&dir, "v9.9"), RevisionArt::Freigabe);

        // Drei getrennte Heimat-Scopes (elektronik, firmware, produkt-global).
        let doc = read_doc(&dir);
        assert_eq!(doc.len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Eine leere alte flache Datei (`{}`) ist kein Migrations-Fall — sie degradiert zu „leer".
    #[test]
    fn empty_flat_map_degrades_to_empty() {
        let dir = tmp();
        let legacy_path = dir.join("_plm").join(ART_FILE);
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, "{}").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        assert!(read_doc(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der zusammengesetzte `(Heimat, version)`-Schlüssel ist injektiv: kein `|` in Heimat oder
    /// Versions-Label kann zwei verschiedene Paare auf denselben Schlüssel — und damit dieselbe
    /// Datei — abbilden.
    #[test]
    fn composite_key_is_collision_free_and_round_trips() {
        for (h, v) in [
            ("elektronik", "v1.0"),
            (GLOBAL_HEIMAT, "v9.9"),
            ("a|b", "c|d"),
            ("", "v0"),
            ("mechanik/teil", "rel|2"),
        ] {
            let key = entry_key(h, v);
            assert_eq!(split_entry_key(&key), Some((h.to_string(), v.to_string())));
        }
        // Ohne Längen-Präfix kollidierten diese beiden; mit ihm nicht.
        assert_ne!(entry_key("a|b", "c"), entry_key("a", "b|c"));
    }
}
