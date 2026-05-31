//! Per-tag Meilenstein-Art persistence (Issue #41, E42).
//!
//! Thin, side-effecting layer that stores the **Art** (Prototyp/Freigabe) of every
//! Meilenstein, keyed by its human version label, as JSON in the product folder. All
//! filesystem access lives here; the pure toggle state machine in
//! [`crate::graph::toggle_milestone_art`] never does I/O. Same split as `edgestore.rs` over
//! `edges.rs` and `graphread.rs` over `graph.rs`.
//!
//! The store holds **only what git cannot know** (E8/E18): git carries the tag (the version
//! label and which commit it points at); the Art is the one PLM fact layered on top, so a
//! tag with no recorded Art is simply the default **Prototyp** (lax — E42), never an error.

use crate::graph::MilestoneArt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// File that holds the per-tag Meilenstein-Art map, in the product root. Dotfile so the
/// `projection.rs` walk (which skips hidden entries) never mistakes it for a Baustein, and
/// prefixed `.plm-` like the sibling edge store.
pub const ART_FILE: &str = ".plm-meilenstein-art.json";

/// Absolute path of the Art store for a product `root`.
fn art_path(root: &Path) -> PathBuf {
    root.join(ART_FILE)
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
    let json = serde_json::to_string_pretty(map).map_err(std::io::Error::other)?;
    std::fs::write(art_path(root), json)
}

/// The recorded [`MilestoneArt`] for a version label. A tag with no recorded Art is the
/// default **Prototyp** (E42) — a freshly promoted Meilenstein is lax until toggled.
pub fn read_art(root: &Path, version: &str) -> MilestoneArt {
    match read_map(root).get(version) {
        Some(token) => MilestoneArt::from_token(token),
        None => MilestoneArt::default(),
    }
}

/// Record the [`MilestoneArt`] for a version label and persist it. Returns the stored Art.
pub fn set_art(root: &Path, version: &str, art: MilestoneArt) -> std::io::Result<MilestoneArt> {
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
        assert_eq!(read_art(&dir, "v1.0"), MilestoneArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_then_read_round_trips_per_tag() {
        let dir = tmp();
        set_art(&dir, "v1.0", MilestoneArt::Freigabe).unwrap();
        set_art(&dir, "v0.9", MilestoneArt::Prototyp).unwrap();
        assert_eq!(read_art(&dir, "v1.0"), MilestoneArt::Freigabe);
        assert_eq!(read_art(&dir, "v0.9"), MilestoneArt::Prototyp);
        // An untouched tag is still the default.
        assert_eq!(read_art(&dir, "v0.1"), MilestoneArt::Prototyp);

        // Un-Release: flipping back is persisted (E42 reversible).
        set_art(&dir, "v1.0", MilestoneArt::Prototyp).unwrap();
        assert_eq!(read_art(&dir, "v1.0"), MilestoneArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_degrades_to_prototyp() {
        let dir = tmp();
        fs::write(art_path(&dir), "{ not json ]").unwrap();
        assert_eq!(read_art(&dir, "v1.0"), MilestoneArt::Prototyp);
        let _ = fs::remove_dir_all(&dir);
    }
}
