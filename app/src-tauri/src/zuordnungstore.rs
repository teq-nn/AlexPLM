//! Manuelle Zuordnungs-Overrides (Folge von Issue #47/#50) — `_plm/zuordnung.json`.
//!
//! Die Pattern-Zuordnung (#47) bildet Karten **per Konvention** (Glob + Heimat). Reale/importierte
//! Produkte folgen dieser Ordner-Konvention aber oft nicht — dann landet alles im Unzugeordnet-Fach.
//! Dieser Speicher trägt die **manuelle Zuordnung aus der Software heraus**: der Nutzer ordnet eine
//! Waise einem Baustein zu, ohne Datei im Dateimanager zu verschieben. Die Zuordnung ist ein
//! **Etikett**, kein Verschieben — zerstörungsfrei und jederzeit lösbar.
//!
//! Form: eine flache Karte `produkt-relativer Pfad (Vorwärts-Slashes) → Baustein-id`. Committet im
//! geteilten `_plm/` (ADR 0002), damit das Team dieselbe Zuordnung sieht. Glue wie `edgestore.rs`:
//! alles I/O hier; fehlende/leere/korrupte Datei ⇒ leere Karte, nie Fehler; pretty-printed JSON.
//!
//! Eine Override gewinnt über die Glob/Heimat-Konvention und ignoriert die Heimat-Grenze: der
//! Nutzer darf **jede** Datei **jedem** Baustein zuordnen ([`crate::werkbank::build_werkbank_with_overrides`]).

use crate::plmstore::PlmCollection;
use std::collections::BTreeMap;
use std::path::Path;

/// Alte Einzeldatei der manuellen Zuordnungen innerhalb von `_plm/` (ADR 0002). Jetzt umgestellt auf
/// **eine Datei pro Zuordnung** unter `_plm/zuordnung/` (E54, Issue #132); bleibt für die Migration.
pub const ZUORDNUNG_FILE: &str = "zuordnung.json";
/// Per-Eintrag-Verzeichnis: eine JSON-Datei pro Zuordnung, benannt nach dem (escapten) Pfad (E54).
pub const ZUORDNUNG_DIR: &str = "zuordnung";

/// Eine Override-Karte: produkt-relativer Pfad → Baustein-id. `BTreeMap`, damit das JSON stabil
/// (alphabetisch) und diffbar bleibt.
pub type Zuordnungen = BTreeMap<String, String>;

/// Die Zuordnungs-Sammlung — **eine ID-benannte Datei pro Zuordnung** (Schlüssel = produkt-relativer
/// Pfad) unter `_plm/zuordnung/`, mit Migration aus der alten Einzeldatei `_plm/zuordnung.json`. Der
/// Payload ist die Baustein-id. Zwei gleichzeitig gesetzte Zuordnungen landen in zwei Dateien und
/// kollidieren so nie im Merge. Pfad, Per-Datei-Degradation und das atomare pretty-Schreiben liegen
/// in der tiefen [`PlmCollection`]-Schicht; hier liegt nur die Zuordnungs-Domänenlogik darüber.
const ZUORDNUNG: PlmCollection<String> = PlmCollection::new(ZUORDNUNG_DIR, ZUORDNUNG_FILE);

/// Pfad in Vorwärts-Slash-Normalform (Backslashes → `/`, getrimmt), passend zu den von
/// `git ls-files` gelieferten Pfaden. So trifft eine gespeicherte Override ihre Datei sicher.
fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

/// Die manuellen Zuordnungen lesen. Fehlende/leere/korrupte Datei ⇒ **leere Karte**, nie Fehler.
pub fn read_overrides(root: &Path) -> Zuordnungen {
    ZUORDNUNG.read(root)
}

/// Die manuellen Zuordnungen pretty-printed zurückschreiben (legt `_plm/` an, atomar).
fn write_overrides(root: &Path, map: &Zuordnungen) -> std::io::Result<()> {
    ZUORDNUNG.write(root, map)
}

/// Eine Datei einem Baustein zuordnen (Etikett setzen) und persistieren. Überschreibt eine frühere
/// Zuordnung derselben Datei. Gibt die aktualisierte Karte zurück.
pub fn assign(root: &Path, file: &str, baustein_id: &str) -> std::io::Result<Zuordnungen> {
    let mut map = read_overrides(root);
    map.insert(normalize(file), baustein_id.to_string());
    write_overrides(root, &map)?;
    Ok(map)
}

/// Die manuelle Zuordnung einer Datei wieder lösen (die Datei fällt zurück auf die Konvention bzw.
/// wird wieder zur Waise). No-op, wenn keine Override existiert. Gibt die aktualisierte Karte zurück.
pub fn clear(root: &Path, file: &str) -> std::io::Result<Zuordnungen> {
    let mut map = read_overrides(root);
    map.remove(&normalize(file));
    write_overrides(root, &map)?;
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-zuordnung-ut-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_store_reads_as_empty() {
        let dir = tmp();
        assert!(read_overrides(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_store_degrades_to_empty() {
        let dir = tmp();
        // eine einzelne hand-verbogene Zuordnungs-Datei wird per-Datei übersprungen, nie fatal.
        let path = ZUORDNUNG.entry_path(&dir, "hardware/teil.x");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json ]").unwrap();
        assert!(read_overrides(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// E54: zwei gleichzeitig gesetzte Zuordnungen landen in zwei getrennten Dateien — kein Merge-
    /// Konflikt auf `_plm`. Pfade mit `/` werden für den Dateinamen escapt und bleiben im Verzeichnis.
    #[test]
    fn two_assignments_land_in_separate_files() {
        let dir = tmp();
        assign(&dir, "elektronik/board.kicad_pcb", "kicad").unwrap();
        assign(&dir, "mechanik/teil.FCStd", "fusion").unwrap();
        let a = ZUORDNUNG.entry_path(&dir, "elektronik/board.kicad_pcb");
        let b = ZUORDNUNG.entry_path(&dir, "mechanik/teil.FCStd");
        assert_ne!(a, b, "verschiedene Pfade -> verschiedene Dateien");
        assert!(a.is_file() && b.is_file());
        assert_eq!(a.parent().unwrap(), ZUORDNUNG.dir_path(&dir), "im _plm/zuordnung/ enthalten");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Migration: ein Produkt mit nur der alten `_plm/zuordnung.json`-Karte behält seine Zuordnungen
    /// (wird nicht still geleert); der nächste Schreibvorgang legt sie als eine Datei pro Eintrag ab.
    #[test]
    fn migrates_legacy_zuordnung_map_file() {
        let dir = tmp();
        let legacy: Zuordnungen =
            BTreeMap::from([("hardware/teil.FCStd".to_string(), "fusion".to_string())]);
        let path = dir.join("_plm").join(ZUORDNUNG_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        // Lesen faltet die alte Karte ein.
        assert_eq!(read_overrides(&dir).get("hardware/teil.FCStd").map(String::as_str), Some("fusion"));

        // Der nächste Schreibvorgang materialisiert das Per-Eintrag-Verzeichnis ohne Verlust.
        assign(&dir, "elektronik/board.kicad_pcb", "kicad").unwrap();
        assert!(ZUORDNUNG.entry_path(&dir, "hardware/teil.FCStd").is_file(), "Altzuordnung pro Datei");
        assert_eq!(read_overrides(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn assign_then_read_round_trips_and_overwrites() {
        let dir = tmp();
        assign(&dir, "hardware/teil.FCStd", "fusion").unwrap();
        assign(&dir, "hardware/teil.FCStd", "kicad").unwrap(); // overwrite
        let map = read_overrides(&dir);
        assert_eq!(map.get("hardware/teil.FCStd").map(String::as_str), Some("kicad"));
        assert_eq!(map.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn assign_normalizes_backslashes() {
        let dir = tmp();
        assign(&dir, "hardware\\teil.FCStd", "fusion").unwrap();
        assert_eq!(
            read_overrides(&dir).get("hardware/teil.FCStd").map(String::as_str),
            Some("fusion")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_removes_and_is_a_noop_when_absent() {
        let dir = tmp();
        assign(&dir, "a/b.x", "fusion").unwrap();
        let map = clear(&dir, "a/b.x").unwrap();
        assert!(map.is_empty());
        // second clear is a no-op, still empty, no error
        assert!(clear(&dir, "a/b.x").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
