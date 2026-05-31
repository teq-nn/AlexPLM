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

use crate::stackstore::PLM_DIR;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Datei für die manuellen Zuordnungen innerhalb von `_plm/`.
pub const ZUORDNUNG_FILE: &str = "zuordnung.json";

/// Pfad von `_plm/zuordnung.json` für ein Produkt `root`.
fn store_path(root: &Path) -> PathBuf {
    root.join(PLM_DIR).join(ZUORDNUNG_FILE)
}

/// Eine Override-Karte: produkt-relativer Pfad → Baustein-id. `BTreeMap`, damit das JSON stabil
/// (alphabetisch) und diffbar bleibt.
pub type Zuordnungen = BTreeMap<String, String>;

/// Pfad in Vorwärts-Slash-Normalform (Backslashes → `/`, getrimmt), passend zu den von
/// `git ls-files` gelieferten Pfaden. So trifft eine gespeicherte Override ihre Datei sicher.
fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

/// Die manuellen Zuordnungen lesen. Fehlende/leere/korrupte Datei ⇒ **leere Karte**, nie Fehler.
pub fn read_overrides(root: &Path) -> Zuordnungen {
    let raw = std::fs::read_to_string(store_path(root)).unwrap_or_default();
    if raw.trim().is_empty() {
        return Zuordnungen::new();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Die manuellen Zuordnungen pretty-printed zurückschreiben (legt `_plm/` an).
fn write_overrides(root: &Path, map: &Zuordnungen) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join(PLM_DIR))?;
    let json = serde_json::to_string_pretty(map).map_err(std::io::Error::other)?;
    std::fs::write(store_path(root), json)
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
        std::fs::create_dir_all(dir.join(PLM_DIR)).unwrap();
        std::fs::write(store_path(&dir), "{ not json ]").unwrap();
        assert!(read_overrides(&dir).is_empty());
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
