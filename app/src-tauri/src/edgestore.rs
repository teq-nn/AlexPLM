//! Persistence glue for manual edges (Issue #10).
//!
//! Thin, side-effecting layer that stores the manual „abgeleitet von" edge set as JSON in
//! the product folder and feeds the pure [`crate::edges`] core. All filesystem access lives
//! here; the pure logic in `edges.rs` never does I/O. Same split as `watcher.rs` over
//! `autocommit.rs` and `graphread.rs` over `graph.rs`.
//!
//! The store is **opt-in**: a product with no edge file has zero edges and therefore no
//! warnings (E40). Reading a missing/empty/corrupt file yields an empty set — never an error
//! — so the warning view degrades to „nothing claimed" rather than breaking the shell.

use crate::edges::{add_edge, remove_edge, stale_warnings, ArtifactStamp, Edge, StaleWarning};
use crate::projection::project_product;
use std::path::{Path, PathBuf};

/// The tool's committed, shared store directory (ADR 0002). `projection.rs` skips it by name.
pub const PLM_DIR: &str = "_plm";
/// File that holds the manual edge set, inside `_plm/` (ADR 0002).
pub const EDGES_FILE: &str = "kanten.json";
/// Legacy location of the edge set (pre-ADR-0002 dotfile). Still read for migration.
pub const LEGACY_EDGES_FILE: &str = ".plm-kanten.json";

/// Absolute path of the `_plm/kanten.json` edge file for a product `root`.
fn edges_path(root: &Path) -> PathBuf {
    root.join(PLM_DIR).join(EDGES_FILE)
}

/// Absolute path of the legacy `.plm-kanten.json` dotfile for a product `root`.
fn legacy_edges_path(root: &Path) -> PathBuf {
    root.join(LEGACY_EDGES_FILE)
}

/// Read the persisted manual edge set for a product. A missing/empty/corrupt file means
/// **zero edges** (opt-in, E40) — not an error.
///
/// Migration (ADR 0002): the new `_plm/kanten.json` wins; if it is absent the legacy
/// `.plm-kanten.json` dotfile is read so existing products are not silently emptied. The next
/// write lands in the new location.
pub fn read_edges(root: &Path) -> Vec<Edge> {
    let raw = match std::fs::read_to_string(edges_path(root)) {
        Ok(s) => s,
        // New file absent -> fall back to the legacy dotfile for migration.
        Err(_) => std::fs::read_to_string(legacy_edges_path(root)).unwrap_or_default(),
    };
    if raw.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist the manual edge set, pretty-printed for an honest, diffable on-disk record. Always
/// writes the new `_plm/kanten.json` (creating `_plm/` as needed); the legacy dotfile is left
/// untouched as harmless sediment.
fn write_edges(root: &Path, edges: &[Edge]) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join(PLM_DIR))?;
    let json = serde_json::to_string_pretty(edges).map_err(std::io::Error::other)?;
    std::fs::write(edges_path(root), json)
}

/// Draw a manual „abgeleitet von" edge (`derived` „stammt aus" `source`) and persist it.
/// De-dupes and refuses self-edges via the pure [`add_edge`]. Returns the new edge set.
pub fn add_persisted_edge(root: &Path, derived: &str, source: &str) -> std::io::Result<Vec<Edge>> {
    let edges = add_edge(read_edges(root), Edge::new(derived, source));
    write_edges(root, &edges)?;
    Ok(edges)
}

/// Remove a manual edge and persist the result. No-op if the edge is absent.
pub fn remove_persisted_edge(
    root: &Path,
    derived: &str,
    source: &str,
) -> std::io::Result<Vec<Edge>> {
    let edges = remove_edge(read_edges(root), &Edge::new(derived, source));
    write_edges(root, &edges)?;
    Ok(edges)
}

/// The edge set plus its computed Stale-Warnungen, returned to the UI in one round-trip.
#[derive(Debug, serde::Serialize)]
pub struct EdgeView {
    pub edges: Vec<Edge>,
    pub warnings: Vec<StaleWarning>,
}

/// Collect the artifact timestamps for a product by walking its Bausteine and reading each
/// representative file's modification time. Side-effecting; the comparison itself is pure.
///
/// An artifact with no readable timestamp is simply omitted — the pure core then declines to
/// warn about edges touching it (it won't compare what it can't see).
fn artifact_stamps(root: &Path) -> Vec<ArtifactStamp> {
    let view = match project_product(root) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    view.bausteine
        .iter()
        .filter_map(|b| {
            // Time the representative file when present, else the Baustein folder itself.
            let target = b
                .main_file
                .as_deref()
                .map(|f| root.join(f))
                .unwrap_or_else(|| root.join(&b.path));
            let mtime = std::fs::metadata(&target).and_then(|m| m.modified()).ok()?;
            Some(ArtifactStamp {
                path: b.path.clone(),
                timestamp: crate::autocommit::format_timestamp(mtime),
            })
        })
        .collect()
}

/// Read the manual edges for a product and compute their Stale-Warnungen over the current
/// artifact timestamps. The warning computation is the pure [`stale_warnings`]; this function
/// only gathers the facts (edges + timestamps) it feeds.
pub fn read_edge_view(root: &Path) -> EdgeView {
    let edges = read_edges(root);
    let artifacts = artifact_stamps(root);
    let warnings = stale_warnings(&edges, &artifacts);
    EdgeView { edges, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-edges-ut-{}-{}",
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
    fn missing_file_reads_as_zero_edges() {
        let dir = tmp();
        assert!(read_edges(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_then_read_round_trips_and_dedupes() {
        let dir = tmp();
        add_persisted_edge(&dir, "fertigung/stl", "mechanik/gehaeuse").unwrap();
        add_persisted_edge(&dir, "fertigung/stl", "mechanik/gehaeuse").unwrap(); // dup
        let edges = read_edges(&dir);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].derived, "fertigung/stl");
        assert_eq!(edges[0].source, "mechanik/gehaeuse");

        let edges = remove_persisted_edge(&dir, "fertigung/stl", "mechanik/gehaeuse").unwrap();
        assert!(edges.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_degrades_to_zero_edges() {
        let dir = tmp();
        fs::create_dir_all(dir.join(PLM_DIR)).unwrap();
        fs::write(edges_path(&dir), "{ not json ]").unwrap();
        assert!(read_edges(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_to_the_new_plm_location() {
        let dir = tmp();
        add_persisted_edge(&dir, "a", "b").unwrap();
        assert!(dir.join(PLM_DIR).join(EDGES_FILE).is_file(), "edges live in _plm/kanten.json");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_legacy_dotfile_when_new_file_absent() {
        let dir = tmp();
        // a product that only has the old dotfile must not be silently emptied (migration).
        let legacy = vec![Edge::new("fertigung/stl", "mechanik/gehaeuse")];
        let json = serde_json::to_string_pretty(&legacy).unwrap();
        fs::write(legacy_edges_path(&dir), json).unwrap();

        let edges = read_edges(&dir);
        assert_eq!(edges, legacy, "legacy dotfile is read when _plm/kanten.json is absent");

        // the next write lands in the new location, leaving the legacy file as sediment.
        add_persisted_edge(&dir, "x/y", "p/q").unwrap();
        assert!(dir.join(PLM_DIR).join(EDGES_FILE).is_file());
        let _ = fs::remove_dir_all(&dir);
    }
}
