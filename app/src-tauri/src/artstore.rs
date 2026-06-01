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
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The tool's committed, shared store directory (ADR 0002). `projection.rs` skips it by name.
pub const PLM_DIR: &str = "_plm";
/// File that holds the per-tag Revision-Art map, inside `_plm/` (ADR 0002).
pub const ART_FILE: &str = "revisionen.json";

/// Absolute path of the `_plm/revisionen.json` Art store for a product `root`.
fn art_path(root: &Path) -> PathBuf {
    root.join(PLM_DIR).join(ART_FILE)
}

/// Read the whole version-label -> Art-token map. A missing/empty/corrupt file means an
/// empty map (every tag then reads as the default Prototyp) — never an error.
fn read_map(root: &Path) -> BTreeMap<String, String> {
    let raw = std::fs::read_to_string(art_path(root)).unwrap_or_default();
    if raw.trim().is_empty() {
        return BTreeMap::new();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist the map, pretty-printed for an honest, diffable on-disk record (BTreeMap keeps the
/// keys ordered so the file stays stable across writes).
fn write_map(root: &Path, map: &BTreeMap<String, String>) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join(PLM_DIR))?;
    let json = serde_json::to_string_pretty(map).map_err(std::io::Error::other)?;
    std::fs::write(art_path(root), json)
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
        fs::create_dir_all(dir.join(PLM_DIR)).unwrap();
        fs::write(art_path(&dir), "{ not json ]").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), RevisionArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_to_the_new_plm_location() {
        let dir = tmp();
        set_art(&dir, "v1.0", RevisionArt::Freigabe).unwrap();
        assert!(
            dir.join(PLM_DIR).join(ART_FILE).is_file(),
            "revision art lives in _plm/revisionen.json"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
