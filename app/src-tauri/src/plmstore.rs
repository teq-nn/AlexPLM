//! The `_plm` document layer — one deep module behind every per-concern JSON store (ADR 0002).
//!
//! Each product keeps „nur das, was git nicht ohnehin weiß" as one JSON file per concern under a
//! committed, shared `_plm/` directory (Stack, Aufgaben, Meilenstein-Art, Kanten, Zuordnungen).
//! Every one of those stores used to re-implement the same skeleton: resolve `_plm/<datei>`, read
//! with the degradation rule (missing/empty/corrupt ⇒ leerer Zustand, nie Fehler), and write
//! pretty-printed JSON. This module owns that skeleton **once** so the stores above it are pure
//! domain logic.
//!
//! Two layers:
//!
//! 1. **Primitives** over an explicit `&Path` ([`read_or_default`], [`read_optional`],
//!    [`write_pretty`]) — the ADR-0002 degradation invariant and an atomic, pretty write, usable
//!    for any JSON document including the app-level ones outside `_plm/` (`registry.rs`,
//!    `bibliothek.rs`).
//! 2. **[`PlmDocument<T>`]** — a primitive bound to one filename inside a product's `_plm/`. The
//!    five product-local stores are each a handful of domain functions over one of these.
//!
//! The degradation rule has a single home here: the warning view degrades to „nichts beansprucht",
//! the task list to „keine Aufgaben", the stack to „leer" — a hand-mangled or half-written file
//! never bricks the shell.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// The tool's committed, shared store directory (ADR 0002). `projection.rs` and the Werkbank-Walk
/// skip it by name so the Baustein-Walk never mistakes it for an Arbeitsbereich.
pub const PLM_DIR: &str = "_plm";

/// Read + parse a JSON document at `path`, applying the ADR-0002 degradation rule: a
/// missing/empty/corrupt file yields `T::default()` — **never an error**.
pub fn read_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    if raw.trim().is_empty() {
        return T::default();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Read + parse a JSON document at `path`, degrading to `None` instead of a default value: a
/// missing/empty/corrupt file yields `None` — never an error. For callers that distinguish
/// „nicht vorhanden" from „vorhanden, leer" (e.g. the Bibliothek).
pub fn read_optional<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let raw = std::fs::read_to_string(path).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

/// Pretty-print `value` to `path` for an honest, diffable on-disk record. Creates the parent
/// directory (e.g. `_plm/`) as needed and writes **atomically** — a sibling temp file is written
/// in full and then renamed over the target, so a crash mid-write can never leave a half-written,
/// corrupt JSON file. (`std::fs::rename` replaces an existing target on both Unix and Windows.)
pub fn write_pretty<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

/// One JSON document inside a product's `_plm/` store, bound to its filename. Owns path
/// resolution, the ADR-0002 degradation rule (via [`read_or_default`]) and the atomic pretty write
/// (via [`write_pretty`]); the store above it is pure domain logic.
///
/// Declared once per concern as a `const`, e.g. `const TASKS: PlmDocument<Vec<Task>> =
/// PlmDocument::new("aufgaben.json");`. The type parameter binds the document to its payload so a
/// store can never read it as the wrong shape.
pub struct PlmDocument<T> {
    /// File name inside `_plm/` (e.g. `kanten.json`).
    file: &'static str,
    /// `T` is only ever produced/consumed through the methods — `fn() -> T` keeps the marker
    /// covariant and `Send`/`Sync` regardless of `T`.
    _marker: PhantomData<fn() -> T>,
}

impl<T> PlmDocument<T> {
    /// A document at `_plm/<file>`.
    pub const fn new(file: &'static str) -> Self {
        PlmDocument { file, _marker: PhantomData }
    }

    /// Absolute path of this document under a product `root` (`<root>/_plm/<file>`).
    pub fn path(&self, root: &Path) -> PathBuf {
        root.join(PLM_DIR).join(self.file)
    }
}

impl<T: DeserializeOwned + Default> PlmDocument<T> {
    /// Read the document for product `root`. Missing/empty/corrupt ⇒ `T::default()` (ADR 0002).
    pub fn read(&self, root: &Path) -> T {
        read_or_default(&self.path(root))
    }
}

impl<T: Serialize> PlmDocument<T> {
    /// Persist the document for product `root` (pretty + atomic, creating `_plm/` as needed).
    pub fn write(&self, root: &Path, value: &T) -> std::io::Result<()> {
        write_pretty(&self.path(root), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-plmstore-ut-{}-{}",
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

    const DOC: PlmDocument<Vec<String>> = PlmDocument::new("dinge.json");

    #[test]
    fn document_round_trips_under_plm() {
        let dir = tmp();
        DOC.write(&dir, &vec!["a".to_string(), "b".to_string()]).unwrap();
        assert_eq!(DOC.path(&dir), dir.join("_plm").join("dinge.json"));
        assert!(DOC.path(&dir).is_file(), "document lives under _plm/");
        assert_eq!(DOC.read(&dir), vec!["a".to_string(), "b".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The degradation rule, owned once here instead of re-tested per store: missing, empty and
    /// corrupt all read as the default — never an error.
    #[test]
    fn degradation_rule_missing_empty_corrupt_all_default() {
        let dir = tmp();
        // missing
        assert!(DOC.read(&dir).is_empty());
        // empty
        std::fs::create_dir_all(dir.join(PLM_DIR)).unwrap();
        std::fs::write(DOC.path(&dir), "   ").unwrap();
        assert!(DOC.read(&dir).is_empty());
        // corrupt
        std::fs::write(DOC.path(&dir), "{ not json ]").unwrap();
        assert!(DOC.read(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_optional_distinguishes_absent_from_present() {
        let dir = tmp();
        let path = dir.join("konto.json");
        assert_eq!(read_optional::<BTreeMap<String, String>>(&path), None);
        write_pretty(&path, &BTreeMap::from([("k".to_string(), "v".to_string())])).unwrap();
        assert_eq!(
            read_optional::<BTreeMap<String, String>>(&path),
            Some(BTreeMap::from([("k".to_string(), "v".to_string())]))
        );
        // corrupt also degrades to None
        std::fs::write(&path, "{ not json ]").unwrap();
        assert_eq!(read_optional::<BTreeMap<String, String>>(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_pretty_creates_parent_and_is_diffable() {
        let dir = tmp();
        let path = dir.join("nested").join("deep").join("x.json");
        write_pretty(&path, &vec![1, 2, 3]).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains('\n'), "pretty-printed, not a single line");
        // no temp file left behind after a successful write
        assert!(!path.with_extension("tmp").exists(), "atomic temp file is renamed away");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
