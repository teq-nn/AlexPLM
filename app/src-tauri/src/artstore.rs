//! Per-tag Revision-Art persistence (Issue #41, E42).
//!
//! Thin, side-effecting layer that stores the **Art** (Prototyp/Freigabe) of every
//! Revision, keyed by its human version label, as JSON in the product folder. All
//! filesystem access lives here; the pure toggle state machine in
//! [`crate::graph::toggle_revision_art`] never does I/O. Same split as `edgestore.rs` over
//! `edges.rs` and `graphread.rs` over `graph.rs`.
//!
//! The store holds **only what git cannot know** (E8/E18): git carries the tag (the version
//! label and which commit it points at); the Art is the one PLM fact layered on top, so a
//! tag with no recorded Art is simply the default **Prototyp** (lax — E42), never an error.

use crate::graph::RevisionArt;
use crate::plmstore::PlmCollection;
use std::collections::BTreeMap;
use std::path::Path;

/// Legacy single-file location of the per-tag Revision-Art map, inside `_plm/` (ADR 0002). Now
/// migrated to one file per Release-Pointer under `_plm/revisionen/` (E54, Issue #132).
pub const ART_FILE: &str = "revisionen.json";
/// Per-entry directory holding one JSON file per Release-Pointer, keyed by the version label (E54).
pub const ART_DIR: &str = "revisionen";

/// The Release-Pointer collection — **one ID-named file per recorded Revision-Art**, keyed by the
/// version label, under `_plm/revisionen/` and migrating from the legacy single `_plm/revisionen.json`
/// map. The payload is the Art-token string. Two tags promoted on two sides land in two files, so
/// the Release-Pointers never collide in a merge. Path, per-file degradation and the atomic pretty
/// write live in the deep [`PlmCollection`] layer; this store is the per-tag Art domain over it.
const ART: PlmCollection<String> = PlmCollection::new(ART_DIR, ART_FILE);

/// Read the whole version-label -> Art-token map. A missing/empty/corrupt entry means an
/// empty map (every tag then reads as the default Prototyp) — never an error; one mangled
/// Release-Pointer file is skipped, not fatal.
fn read_map(root: &Path) -> BTreeMap<String, String> {
    ART.read(root)
}

/// Persist the map as one file per Release-Pointer (pretty + atomic, creating `_plm/revisionen/`).
fn write_map(root: &Path, map: &BTreeMap<String, String>) -> std::io::Result<()> {
    ART.write(root, map)
}

/// The recorded [`RevisionArt`] for a version label. A tag with no recorded Art is the
/// default **Prototyp** (E42) — a freshly promoted Revision is lax until toggled.
pub fn read_art(root: &Path, version: &str) -> RevisionArt {
    match read_map(root).get(version) {
        Some(token) => RevisionArt::from_token(token),
        None => RevisionArt::default(),
    }
}

/// Record the [`RevisionArt`] for a version label and persist it. Returns the stored Art.
pub fn set_art(root: &Path, version: &str, art: RevisionArt) -> std::io::Result<RevisionArt> {
    let mut map = read_map(root);
    map.insert(version.to_string(), art.as_token().to_string());
    write_map(root, &map)?;
    Ok(art)
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
        // A tag the tool has never seen is lax by default (E42).
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
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

    #[test]
    fn corrupt_file_degrades_to_prototyp() {
        let dir = tmp();
        // a single hand-mangled Release-Pointer file is skipped per-file, never fatal.
        let path = ART.entry_path(&dir, "v1.0");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json ]").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_one_file_per_release_pointer() {
        let dir = tmp();
        set_art(&dir, "v1.0", RevisionArt::Freigabe).unwrap();
        set_art(&dir, "v0.9", RevisionArt::Freigabe).unwrap();
        assert!(
            ART.entry_path(&dir, "v1.0").is_file(),
            "each Release-Pointer is its own file under _plm/revisionen/"
        );
        // E54: two tags' Release-Pointers are two separate files — no merge collision.
        assert_ne!(ART.entry_path(&dir, "v1.0"), ART.entry_path(&dir, "v0.9"));
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
        assert!(ART.entry_path(&dir, "v1.0").is_file(), "legacy Art written out per file");
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Freigabe);
        assert_eq!(read_art(&dir, "v2.0"), RevisionArt::Freigabe);
        let _ = fs::remove_dir_all(&dir);
    }
}
