//! Bibliothek — Speicher der Standard-Bausteine/-Toolstacks + idempotentes Seeding (Issue #39).
//!
//! Zwei Schichten, wie im Haus üblich (reiner Kern + dünner Glue):
//!
//! 1. **Reine Entscheidung** ([`seed_decision`]): gegeben den gebündelten Default und den lokal
//!    vorhandenen Stand — was tun? Install / Upgrade / Keep (lokal verändert) / Keep (aktuell).
//!    Total, deterministisch, ohne I/O. `#[cfg(test)]`-Tabellentests darüber.
//! 2. **Glue** ([`Bibliothek`]): liest/schreibt JSON unter `app_data_dir`, lädt die gebündelten
//!    Defaults aus dem Tauri-Resource-Verzeichnis und führt das Seeding aus.
//!
//! Lesemuster wie `edgestore.rs`: fehlende/leere/korrupte Datei ⇒ leerer Zustand, nie Fehler;
//! geschrieben wird pretty-printed JSON.
//!
//! Anti-Drift (ADR 0003): die Bibliothek ist nur **Vorlagen-Quelle**. Ein Produkt-Stack ist eine
//! Vollkopie in `_plm/stack.json` (siehe `stackstore.rs`) — eine Bibliotheks-Änderung erreicht ein
//! laufendes Produkt nie.

use crate::baustein::{Baustein, Toolstack};
use std::path::{Path, PathBuf};

/// Was das Seeding mit einem gebündelten Default tun soll, gegeben der lokale Stand (ADR 0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedAction {
    /// Lokal nicht vorhanden → installieren.
    Install,
    /// Lokal älter und **unverändert** → auf die gebündelte Version upgraden.
    Upgrade,
    /// Lokal vorhanden und **verändert** (egal welche Version) → behalten, „Update verfügbar".
    KeepEdited,
    /// Lokal vorhanden, aktuell (oder neuer) und unverändert → nichts tun.
    KeepCurrent,
}

/// Ob ein lokaler Baustein gegenüber dem gebündelten Default als „unverändert" gilt: alle Felder
/// außer `version` stimmen überein. So zählt ein reines Versions-Hochzählen seitens des Repos nicht
/// als Nutzer-Edit, jede inhaltliche Abweichung dagegen schon. Rein.
fn is_unedited(local: &Baustein, bundled: &Baustein) -> bool {
    let mut local_normalized = local.clone();
    local_normalized.version = bundled.version;
    local_normalized == *bundled
}

/// Die reine Seeding-Entscheidung für **einen** Baustein (ADR 0003). Total + deterministisch.
///
/// - lokal nicht vorhanden → [`SeedAction::Install`]
/// - lokal verändert → [`SeedAction::KeepEdited`] (nie überschreiben)
/// - lokal unverändert und `local.version < bundled.version` → [`SeedAction::Upgrade`]
/// - sonst (unverändert, gleiche/neuere Version) → [`SeedAction::KeepCurrent`]
pub fn seed_decision(bundled: &Baustein, local: Option<&Baustein>) -> SeedAction {
    match local {
        None => SeedAction::Install,
        Some(local) => {
            if !is_unedited(local, bundled) {
                SeedAction::KeepEdited
            } else if local.version < bundled.version {
                SeedAction::Upgrade
            } else {
                SeedAction::KeepCurrent
            }
        }
    }
}

/// Ein einzelnes Ergebnis des Seedings, für den Aufrufer/Log nachvollziehbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedOutcome {
    pub id: String,
    pub action: SeedAction,
}

/// Glue über das Bibliotheks-Verzeichnis. `root` ist `<app-data>/plm-werkzeug/bibliothek`.
pub struct Bibliothek {
    root: PathBuf,
}

impl Bibliothek {
    /// Eine Bibliothek über dem gegebenen Wurzelverzeichnis (`…/plm-werkzeug/bibliothek`).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Bibliothek { root: root.into() }
    }

    fn bausteine_dir(&self) -> PathBuf {
        self.root.join("bausteine")
    }

    fn toolstacks_dir(&self) -> PathBuf {
        self.root.join("toolstacks")
    }

    fn baustein_path(&self, id: &str) -> PathBuf {
        self.bausteine_dir().join(format!("{id}.json"))
    }

    fn toolstack_path(&self, id: &str) -> PathBuf {
        self.toolstacks_dir().join(format!("{id}.json"))
    }

    /// Einen lokalen Baustein lesen; fehlende/korrupte Datei ⇒ `None` (nie Fehler).
    pub fn read_baustein(&self, id: &str) -> Option<Baustein> {
        read_json(&self.baustein_path(id))
    }

    /// Einen lokalen Toolstack lesen; fehlende/korrupte Datei ⇒ `None`.
    pub fn read_toolstack(&self, id: &str) -> Option<Toolstack> {
        read_json(&self.toolstack_path(id))
    }

    /// Alle lokal vorhandenen Bausteine, alphabetisch nach `id`. Korrupte Dateien werden
    /// übersprungen (degradiert, nie Fehler).
    pub fn list_bausteine(&self) -> Vec<Baustein> {
        let mut out: Vec<Baustein> = read_dir_json(&self.bausteine_dir());
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Alle lokal vorhandenen Toolstacks, alphabetisch nach `id`.
    pub fn list_toolstacks(&self) -> Vec<Toolstack> {
        let mut out: Vec<Toolstack> = read_dir_json(&self.toolstacks_dir());
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Einen Baustein pretty-printed schreiben (legt das Verzeichnis bei Bedarf an).
    pub fn write_baustein(&self, b: &Baustein) -> std::io::Result<()> {
        std::fs::create_dir_all(self.bausteine_dir())?;
        write_json(&self.baustein_path(&b.id), b)
    }

    /// Einen Toolstack pretty-printed schreiben (legt das Verzeichnis bei Bedarf an).
    pub fn write_toolstack(&self, t: &Toolstack) -> std::io::Result<()> {
        std::fs::create_dir_all(self.toolstacks_dir())?;
        write_json(&self.toolstack_path(&t.id), t)
    }

    /// Idempotentes, version-gegates Seeding aus den gebündelten Defaults (ADR 0003).
    ///
    /// Für jeden gebündelten Baustein wird [`seed_decision`] gefällt und nur bei `Install`/`Upgrade`
    /// geschrieben. Nutzer-veränderte und nutzer-eigene Bausteine bleiben unangetastet. Toolstacks
    /// werden installiert, falls fehlend (sie referenzieren nur `id`s; ein lokaler Toolstack-Edit
    /// bleibt erhalten). Gibt die Liste der Entscheidungen zurück.
    pub fn seed_from(
        &self,
        bundled_bausteine: &[Baustein],
        bundled_toolstacks: &[Toolstack],
    ) -> std::io::Result<Vec<SeedOutcome>> {
        let mut outcomes = Vec::new();
        for bundled in bundled_bausteine {
            let local = self.read_baustein(&bundled.id);
            let action = seed_decision(bundled, local.as_ref());
            if matches!(action, SeedAction::Install | SeedAction::Upgrade) {
                self.write_baustein(bundled)?;
            }
            outcomes.push(SeedOutcome { id: bundled.id.clone(), action });
        }
        for ts in bundled_toolstacks {
            if self.read_toolstack(&ts.id).is_none() {
                self.write_toolstack(ts)?;
            }
        }
        Ok(outcomes)
    }
}

/// Read+parse a JSON file; missing/empty/corrupt ⇒ `None` (degrade, never error).
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let raw = std::fs::read_to_string(path).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

/// Read every `*.json` in `dir` that parses as `T`; missing dir ⇒ empty, corrupt files skipped.
fn read_dir_json<T: serde::de::DeserializeOwned>(dir: &Path) -> Vec<T> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(parsed) = read_json::<T>(&path) {
            out.push(parsed);
        }
    }
    out
}

/// Pretty-print a value to `path` for an honest, diffable on-disk record.
fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

/// Load the bundled default Bausteine + Toolstacks from a Tauri resource directory
/// (`…/resources/bibliothek`). Corrupt/missing entries are skipped — degrade, never error.
pub fn load_bundled(
    resource_bibliothek_dir: &Path,
) -> (Vec<Baustein>, Vec<Toolstack>) {
    let mut bausteine: Vec<Baustein> = read_dir_json(&resource_bibliothek_dir.join("bausteine"));
    bausteine.sort_by(|a, b| a.id.cmp(&b.id));
    let mut toolstacks: Vec<Toolstack> = read_dir_json(&resource_bibliothek_dir.join("toolstacks"));
    toolstacks.sort_by(|a, b| a.id.cmp(&b.id));
    (bausteine, toolstacks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baustein::Oeffnen;

    fn baustein(id: &str, version: u32, heimat: &str) -> Baustein {
        Baustein {
            id: id.to_string(),
            version,
            name: id.to_string(),
            heimat: heimat.to_string(),
            globs: vec!["*.x".to_string()],
            ignore: vec![],
            lfs: vec![],
            oeffnen: Oeffnen::Auto,
            startaufgaben: vec![],
            default_kanten: vec![],
            stillgelegt: false,
        }
    }

    #[test]
    fn seed_decision_table() {
        let bundled = baustein("kicad", 2, "elektronik");

        // not present -> install
        assert_eq!(seed_decision(&bundled, None), SeedAction::Install);

        // present, older, unedited -> upgrade
        let older_unedited = baustein("kicad", 1, "elektronik");
        assert_eq!(seed_decision(&bundled, Some(&older_unedited)), SeedAction::Upgrade);

        // present, same version, unedited -> keep current
        let same = baustein("kicad", 2, "elektronik");
        assert_eq!(seed_decision(&bundled, Some(&same)), SeedAction::KeepCurrent);

        // present, newer, unedited -> keep current (never downgrade)
        let newer = baustein("kicad", 3, "elektronik");
        assert_eq!(seed_decision(&bundled, Some(&newer)), SeedAction::KeepCurrent);

        // present, older, but EDITED -> keep edited (never clobber a local tweak)
        let older_edited = baustein("kicad", 1, "mechanik"); // heimat differs => edited
        assert_eq!(seed_decision(&bundled, Some(&older_edited)), SeedAction::KeepEdited);

        // present, same version, but EDITED -> keep edited
        let same_edited = baustein("kicad", 2, "mechanik");
        assert_eq!(seed_decision(&bundled, Some(&same_edited)), SeedAction::KeepEdited);
    }

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-biblio-ut-{}-{}",
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
    fn seeding_installs_then_is_idempotent() {
        let dir = tmp();
        let lib = Bibliothek::new(&dir);
        let bundled = vec![baustein("kicad", 1, "elektronik"), baustein("fusion", 1, "mechanik")];
        let stacks = vec![Toolstack {
            id: "standard-hw".to_string(),
            name: "Standard".to_string(),
            baustein_ids: vec!["kicad".to_string(), "fusion".to_string()],
        }];

        let first = lib.seed_from(&bundled, &stacks).unwrap();
        assert!(first.iter().all(|o| o.action == SeedAction::Install));
        assert_eq!(lib.list_bausteine().len(), 2);
        assert_eq!(lib.list_toolstacks().len(), 1);

        // second run: nothing changed -> all keep-current, no extra files
        let second = lib.seed_from(&bundled, &stacks).unwrap();
        assert!(second.iter().all(|o| o.action == SeedAction::KeepCurrent));
        assert_eq!(lib.list_bausteine().len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seeding_upgrades_unedited_but_keeps_edited() {
        let dir = tmp();
        let lib = Bibliothek::new(&dir);

        // seed v1
        lib.seed_from(&[baustein("kicad", 1, "elektronik")], &[]).unwrap();

        // user edits kicad locally (changes heimat)
        let mut edited = lib.read_baustein("kicad").unwrap();
        edited.heimat = "custom".to_string();
        lib.write_baustein(&edited).unwrap();

        // a freshly added user-only baustein must never be touched
        lib.write_baustein(&baustein("meine-cnc", 1, "fertigung")).unwrap();

        // ship v2 of kicad
        let outcomes = lib.seed_from(&[baustein("kicad", 2, "elektronik")], &[]).unwrap();
        assert_eq!(outcomes[0].action, SeedAction::KeepEdited);
        // local edit preserved (not upgraded)
        assert_eq!(lib.read_baustein("kicad").unwrap().heimat, "custom");
        // user baustein untouched
        assert!(lib.read_baustein("meine-cnc").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upgrades_an_unedited_older_baustein() {
        let dir = tmp();
        let lib = Bibliothek::new(&dir);
        lib.seed_from(&[baustein("kicad", 1, "elektronik")], &[]).unwrap();

        let outcomes = lib.seed_from(&[baustein("kicad", 2, "elektronik")], &[]).unwrap();
        assert_eq!(outcomes[0].action, SeedAction::Upgrade);
        assert_eq!(lib.read_baustein("kicad").unwrap().version, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_and_corrupt_read_as_empty() {
        let dir = tmp();
        let lib = Bibliothek::new(&dir);
        assert!(lib.list_bausteine().is_empty());
        assert!(lib.read_baustein("nope").is_none());

        std::fs::create_dir_all(lib.bausteine_dir()).unwrap();
        std::fs::write(lib.baustein_path("broken"), "{ not json ]").unwrap();
        assert!(lib.read_baustein("broken").is_none());
        assert!(lib.list_bausteine().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
