//! The `_plm` document layer — one deep module behind every per-concern JSON store (ADR 0002).
//!
//! Each product keeps „nur das, was git nicht ohnehin weiß" as one JSON file per concern under a
//! committed, shared `_plm/` directory (Stack, Aufgaben, Meilenstein-Art, Kanten, Zuordnungen).
//! Every one of those stores used to re-implement the same skeleton: resolve `_plm/<datei>`, read
//! with the degradation rule (missing/empty/corrupt ⇒ leerer Zustand, nie Fehler), and write
//! pretty-printed JSON. This module owns that skeleton **once** so the stores above it are pure
//! domain logic.
//!
//! Three layers:
//!
//! 1. **Primitives** over an explicit `&Path` ([`read_or_default`], [`read_optional`],
//!    [`write_pretty`]) — the ADR-0002 degradation invariant and an atomic, pretty write, usable
//!    for any JSON document including the app-level ones outside `_plm/` (`registry.rs`,
//!    `bibliothek.rs`).
//! 2. **[`PlmDocument<T>`]** — a primitive bound to one filename inside a product's `_plm/`. The
//!    single-value stores (the Produkt-Stack) are a handful of domain functions over one of these.
//! 3. **[`PlmCollection<T>`]** — a per-entry store: **one ID-named JSON file per entry** under a
//!    `_plm/<belang>/` directory, instead of one shared array/map file (E54, Issue #132). Two
//!    concurrently created entries land in two separate files, so Werkbank's own coordination
//!    files (Aufgaben, Release-Pointer, Kanten, Zuordnungen) never collide in a merge. The
//!    degradation invariant is **per file**: a missing/empty/corrupt single entry is skipped, and
//!    a missing directory is simply the empty collection — never an error. Existing array/map files
//!    are migrated on first read (folded in) and on the next write (written out per entry).
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

/// A per-entry store inside a product's `_plm/`: **one ID-named JSON file per entry** under a
/// `_plm/<belang>/` directory, instead of one shared array/map file (E54, Issue #132).
///
/// Why a file per entry: the old single `aufgaben.json`/`kanten.json`/… meant two developers who
/// each created an entry touched the **same** file at the same line — a guaranteed merge conflict on
/// Werkbank's own coordination files. With one file per entry, two concurrently created entries land
/// in two **different** files, so git merges them without a conflict. (A genuine edit of the *same*
/// entry on both sides still conflicts — that is a real, honest conflict, not an artefact of layout.)
///
/// Shape: every concern is modelled as a `key → payload` map. The key names the file
/// (`<sanitized-key>.json`); the file holds the full payload `T`. Aufgaben key on the task id,
/// Kanten on their endpoint pair, Release-Pointer on the version label, Zuordnungen on the file path.
///
/// Degradation (ADR 0002), now **per file**: a missing directory is simply the empty collection; a
/// single missing/empty/corrupt entry file is **skipped**, never fatal — one hand-mangled entry can
/// never brick the whole list. Entries come back ordered by key (stable, diffable iteration).
///
/// Migration: the collection also knows the **legacy** single-file [`PlmDocument`] location. When the
/// per-entry directory is absent, [`read`](PlmCollection::read) folds the old array/map file in, so
/// existing products are never silently emptied; the next [`write`](PlmCollection::write) lays the
/// entries out one file per entry (leaving the legacy file behind as harmless sediment).
pub struct PlmCollection<T> {
    /// Directory name inside `_plm/` that holds the per-entry files (e.g. `aufgaben`).
    dir: &'static str,
    /// The legacy single-file location (e.g. `_plm/aufgaben.json`), read once for migration. It
    /// carries a `BTreeMap<String, T>` — the same `key → payload` shape this collection persists.
    legacy: PlmDocument<std::collections::BTreeMap<String, T>>,
    /// `T` flows only through the methods — `fn() -> T` keeps the marker covariant and `Send`/`Sync`.
    _marker: PhantomData<fn() -> T>,
}

/// Render a collection key safe for a file name: every byte that is not an ASCII letter, digit, `-`
/// or `_` becomes `%XX` (its hex code). Reversible in spirit and collision-free (the escape char `%`
/// is itself escaped), so two distinct keys never share a file — and a key with a `/` (Zuordnungs-
/// Pfad) or `|` (Kanten-Paar) can never escape its `_plm/<belang>/` directory.
fn key_to_filename(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 5);
    for b in key.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

impl<T> PlmCollection<T> {
    /// A per-entry collection living in `_plm/<dir>/`, migrating from the legacy single file
    /// `_plm/<legacy_file>` (a `key → payload` map).
    pub const fn new(dir: &'static str, legacy_file: &'static str) -> Self {
        PlmCollection {
            dir,
            legacy: PlmDocument::new(legacy_file),
            _marker: PhantomData,
        }
    }

    /// Absolute path of this collection's per-entry directory under a product `root`
    /// (`<root>/_plm/<dir>/`).
    pub fn dir_path(&self, root: &Path) -> PathBuf {
        root.join(PLM_DIR).join(self.dir)
    }

    /// Absolute path of one entry's file (`<root>/_plm/<dir>/<sanitized-key>.json`).
    pub fn entry_path(&self, root: &Path, key: &str) -> PathBuf {
        self.dir_path(root).join(format!("{}.json", key_to_filename(key)))
    }
}

impl<T: DeserializeOwned> PlmCollection<T> {
    /// Read every entry of the collection as a `key → payload` map.
    ///
    /// Per-file degradation (ADR 0002): a single empty/corrupt entry file is skipped; a missing
    /// directory yields the empty map. When the per-entry directory is **absent**, the legacy
    /// single array/map file is folded in instead (migration) so existing products are not emptied.
    /// Keys come from the file stems (un-escaped), never trusting the payload to re-derive them.
    pub fn read(&self, root: &Path) -> std::collections::BTreeMap<String, T> {
        let dir = self.dir_path(root);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            // No per-entry directory yet — migrate from the legacy single file (empty if it too is
            // missing/corrupt). The next write lays the entries out one file per entry.
            return self.legacy.read(root);
        };
        let mut map = std::collections::BTreeMap::new();
        for entry in entries.flatten() {
            let path = entry.path();
            // Only our `.json` entry files; ignore the atomic `.tmp` siblings and anything else.
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            let key = filename_to_key(stem);
            // A single missing/empty/corrupt entry is skipped — never fatal (per-file degradation).
            if let Some(value) = read_optional::<T>(&path) {
                map.insert(key, value);
            }
        }
        map
    }
}

impl<T: Serialize> PlmCollection<T> {
    /// Persist the whole collection as one file per entry under `_plm/<dir>/` (pretty + atomic each).
    ///
    /// The directory is brought in line with `map`: every entry is (re)written under its key, and
    /// stale `.json` files for keys no longer present are removed — so a delete on one side and a
    /// create on the other never collide (each touches only its own file). The legacy single file is
    /// left untouched as harmless sediment.
    pub fn write(&self, root: &Path, map: &std::collections::BTreeMap<String, T>) -> std::io::Result<()> {
        let dir = self.dir_path(root);
        std::fs::create_dir_all(&dir)?;
        // Write/refresh every current entry, one diffable file each.
        for (key, value) in map {
            write_pretty(&self.entry_path(root, key), value)?;
        }
        // Sweep entry files whose key is gone (a removal must not linger on disk).
        let wanted: std::collections::BTreeSet<String> =
            map.keys().map(|k| format!("{}.json", key_to_filename(k))).collect();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !wanted.contains(name) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Inverse of [`key_to_filename`]: turn an escaped file stem back into the original key. Unescapes
/// `%XX` byte triples; a malformed tail is left verbatim (degradation-friendly — we never panic on a
/// hand-edited name). Invalid UTF-8 after unescaping falls back to the raw stem.
fn filename_to_key(stem: &str) -> String {
    let bytes = stem.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| stem.to_string())
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

    // ── PlmCollection: one ID-named file per entry (E54, Issue #132) ──────────────────────────

    /// A stand-in per-entry collection: keys are arbitrary strings (incl. `/` and `|` to prove the
    /// file-name escaping), payloads are plain strings. Mirrors the four real concerns' shape.
    const COLL: PlmCollection<String> = PlmCollection::new("dinge", "dinge.json");

    fn entry_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn collection_writes_one_file_per_entry_and_round_trips() {
        let dir = tmp();
        let map = entry_map(&[("a", "1"), ("b", "2"), ("c", "3")]);
        COLL.write(&dir, &map).unwrap();
        // one ID-named file per entry under _plm/dinge/
        let count = std::fs::read_dir(COLL.dir_path(&dir))
            .unwrap()
            .filter(|e| e.as_ref().unwrap().path().extension().and_then(|x| x.to_str()) == Some("json"))
            .count();
        assert_eq!(count, 3, "exactly one JSON file per entry");
        assert!(COLL.entry_path(&dir, "a").is_file());
        assert_eq!(COLL.read(&dir), map, "reads back identical");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole point of E54: two concurrently created entries occupy two **different** files, so a
    /// merge of the two sides never collides on `_plm`. We simulate the two branches as two writes of
    /// the *other* side's entry into the same directory.
    #[test]
    fn two_new_entries_land_in_separate_files_no_collision() {
        let dir = tmp();
        // side A creates "anna-task"; side B creates "bert-task" — independently.
        COLL.write(&dir, &entry_map(&[("anna-task", "A")])).unwrap();
        // side B's file is simply added next to it (what a clean git merge would produce).
        write_pretty(&COLL.entry_path(&dir, "bert-task"), &"B".to_string()).unwrap();
        let merged = COLL.read(&dir);
        assert_eq!(merged, entry_map(&[("anna-task", "A"), ("bert-task", "B")]));
        // the two entries are genuinely two files (no shared line to conflict on).
        assert_ne!(COLL.entry_path(&dir, "anna-task"), COLL.entry_path(&dir, "bert-task"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Degradation is now **per file**: a single corrupt/empty entry is skipped, the rest survive,
    /// and a missing directory is just the empty collection — never an error.
    #[test]
    fn collection_degradation_is_per_file_and_never_errors() {
        let dir = tmp();
        // missing directory ⇒ empty
        assert!(COLL.read(&dir).is_empty());

        COLL.write(&dir, &entry_map(&[("good", "ok"), ("blank", "x")])).unwrap();
        // hand-mangle one entry, blank another — both must be skipped, not fatal.
        std::fs::write(COLL.entry_path(&dir, "good"), "ok-still").unwrap(); // valid json string? no:
        std::fs::write(COLL.entry_path(&dir, "good"), "{ not json ]").unwrap();
        std::fs::write(COLL.entry_path(&dir, "blank"), "   ").unwrap();
        // and add one healthy entry that must come through.
        write_pretty(&COLL.entry_path(&dir, "healthy"), &"v".to_string()).unwrap();
        assert_eq!(COLL.read(&dir), entry_map(&[("healthy", "v")]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Keys with `/` or `|` (Zuordnungs-Pfade, Kanten-Paare) round-trip through the escaped file name
    /// and cannot escape the `_plm/<belang>/` directory.
    #[test]
    fn keys_with_slashes_and_pipes_round_trip_and_stay_contained() {
        let dir = tmp();
        let map = entry_map(&[("mechanik/teil.step", "fusion"), ("a/b|c/d", "edge")]);
        COLL.write(&dir, &map).unwrap();
        // every file sits directly under _plm/dinge/ (no traversal out of the belang dir).
        for entry in std::fs::read_dir(COLL.dir_path(&dir)).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(path.parent().unwrap(), COLL.dir_path(&dir));
        }
        assert_eq!(COLL.read(&dir), map);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Migration: a product that only has the legacy single map file must read its entries (not be
    /// emptied), and the next write lays them out one file per entry.
    #[test]
    fn migration_folds_in_legacy_single_file_then_writes_per_entry() {
        let dir = tmp();
        // seed the legacy _plm/dinge.json with two entries (the old single-file format).
        write_pretty(&dir.join(PLM_DIR).join("dinge.json"), &entry_map(&[("x", "1"), ("y", "2")]))
            .unwrap();
        // no per-entry directory yet ⇒ read migrates from the legacy file.
        assert_eq!(COLL.read(&dir), entry_map(&[("x", "1"), ("y", "2")]));

        // the next write materialises the per-entry directory.
        let mut map = COLL.read(&dir);
        map.insert("z".to_string(), "3".to_string());
        COLL.write(&dir, &map).unwrap();
        assert!(COLL.entry_path(&dir, "x").is_file(), "legacy entries written out per file");
        assert!(COLL.entry_path(&dir, "z").is_file());
        // once the directory exists it wins over the legacy file.
        assert_eq!(COLL.read(&dir), map);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A removed entry's file is swept on the next write — a delete cannot linger on disk.
    #[test]
    fn write_sweeps_files_for_removed_keys() {
        let dir = tmp();
        COLL.write(&dir, &entry_map(&[("keep", "1"), ("drop", "2")])).unwrap();
        assert!(COLL.entry_path(&dir, "drop").is_file());
        COLL.write(&dir, &entry_map(&[("keep", "1")])).unwrap();
        assert!(!COLL.entry_path(&dir, "drop").exists(), "removed entry's file is swept");
        assert_eq!(COLL.read(&dir), entry_map(&[("keep", "1")]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_filename_escaping_round_trips() {
        for key in ["plain", "a/b", "a|b", "v1.0", "t123-0", "mechanik/teil.FCStd", "ä%/x"] {
            assert_eq!(filename_to_key(&key_to_filename(key)), key, "round-trip for {key:?}");
        }
        // the escape char itself is escaped, so distinct keys never collide on one file.
        assert_ne!(key_to_filename("a%2Fb"), key_to_filename("a/b"));
    }
}
