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

use crate::defaultkanten::{
    baustein_default_kanten, mit_default_kanten, paar_default_vorschlaege, ArtefaktDatei,
    KantenVorschlag,
};
use crate::edges::{add_edge, remove_edge, stale_warnings, ArtifactStamp, Edge, KantenHerkunft, StaleWarning};
use crate::plmstore::{self, PlmCollection, PLM_DIR};
use crate::projection::project_product;
use crate::stackstore::read_stack;
use crate::werkbank::list_tracked_files;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Per-entry directory holding one JSON file per Kante, inside `_plm/` (E54, Issue #132).
pub const EDGES_DIR: &str = "kanten";
/// Legacy single-array file of the edge set under `_plm/` (ADR 0002). Still read for migration.
pub const EDGES_FILE: &str = "kanten.json";
/// Even older location of the edge set (pre-ADR-0002 dotfile). Still read for migration.
pub const LEGACY_EDGES_FILE: &str = ".plm-kanten.json";

/// The Kanten collection — **one ID-named file per edge** under `_plm/kanten/` (E54). The key is the
/// edge's endpoint pair (`derived` „stammt aus" `source`), so two manually drawn edges never share a
/// file and never collide in a merge. Path, per-file degradation and the atomic pretty write live in
/// the deep [`PlmCollection`] layer; this store is the edge domain over it.
///
/// The collection's built-in legacy is the `_plm/kanten.json` **array** file — but it carries a
/// `Vec<Edge>`, not the `key → Edge` map this collection persists, so its migration is done by hand
/// in [`read_edges`] (which also still folds in the even older `.plm-kanten.json` dotfile).
const EDGES: PlmCollection<Edge> = PlmCollection::new(EDGES_DIR, EDGES_FILE);

/// Absolute path of the legacy `.plm-kanten.json` dotfile for a product `root`.
fn legacy_edges_path(root: &Path) -> PathBuf {
    root.join(LEGACY_EDGES_FILE)
}

/// Absolute path of the legacy `_plm/kanten.json` single-array file for a product `root`.
fn array_edges_path(root: &Path) -> PathBuf {
    root.join(PLM_DIR).join(EDGES_FILE)
}

/// A stable key for an edge: its endpoint pair. Drawing the same edge twice keys to the same file
/// (so a redraw overwrites rather than duplicates); two **different** edges never collide.
fn edge_key(edge: &Edge) -> String {
    format!("{}\u{0001}{}", edge.derived, edge.source)
}

/// The edge set as the `key → Edge` map the per-entry collection persists.
fn edges_to_map(edges: &[Edge]) -> BTreeMap<String, Edge> {
    edges.iter().map(|e| (edge_key(e), e.clone())).collect()
}

/// Read the persisted manual edge set for a product. A missing/empty/corrupt entry means
/// **zero edges** (opt-in, E40) — not an error; a single mangled edge file is skipped, not fatal.
///
/// Migration cascade (newest wins, so a product is never silently emptied): the per-entry
/// `_plm/kanten/` directory, else the `_plm/kanten.json` array file (ADR 0002), else the even older
/// `.plm-kanten.json` dotfile. The next write lands in the per-entry directory; the legacy files are
/// left behind as harmless sediment.
pub fn read_edges(root: &Path) -> Vec<Edge> {
    if EDGES.dir_path(root).is_dir() {
        EDGES.read(root).into_values().collect()
    } else if array_edges_path(root).is_file() {
        plmstore::read_or_default(&array_edges_path(root))
    } else {
        plmstore::read_or_default(&legacy_edges_path(root))
    }
}

/// Persist the manual edge set as one file per edge under `_plm/kanten/` (creating it as needed);
/// the legacy array file and dotfile are left untouched as harmless sediment.
fn write_edges(root: &Path, edges: &[Edge]) -> std::io::Result<()> {
    EDGES.write(root, &edges_to_map(edges))
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

/// The edge set plus its computed Stale-Warnungen, returned to the UI in one round-trip. Trägt seit
/// #56 zusätzlich die offenen **Paar-Default-Vorschläge** (deterministisch, per Klick bestätigt).
#[derive(specta::Type, Debug, serde::Serialize)]
pub struct EdgeView {
    pub edges: Vec<Edge>,
    pub warnings: Vec<StaleWarning>,
    /// Offene Baustein-Paar-Default-Vorschläge (E20): noch nicht bestätigte Kanten. Leer, wenn keine
    /// passende Baustein-Paarung im Stack liegt oder alle bereits bestätigt sind.
    #[serde(default)]
    pub vorschlaege: Vec<KantenVorschlag>,
}

/// Die erfassten Dateien eines Produkts als reine [`ArtefaktDatei`]-Snapshots (Pfad + Ordner) für
/// den Default-Kanten-Kern. Side-effecting (`git ls-files`); die Ableitung selbst ist rein.
fn artefakt_dateien(root: &Path) -> Vec<ArtefaktDatei> {
    list_tracked_files(root)
        .unwrap_or_default()
        .into_iter()
        .map(|pfad| {
            let ordner = pfad.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            ArtefaktDatei { pfad, ordner }
        })
        .collect()
}

/// **Baustein-Default-Kanten anlegen (Onboarding, E20):** den Produkt-Stack + die erfassten Dateien
/// lesen, die Default-Kanten **innerhalb** der (aktiven) Bausteine ableiten und in `_plm/kanten.json`
/// einfügen — bestehende (Hand-)Kanten bleiben unangetastet ([`mit_default_kanten`]). Idempotent:
/// ein zweiter Lauf fügt nichts Neues hinzu. Stillgelegte Bausteine liefern keine Kante (still in
/// Ruhe, E17). Gibt die persistierte Kantenmenge zurück. **Nur bei Änderung** wird geschrieben.
pub fn onboard_default_edges(root: &Path) -> std::io::Result<Vec<Edge>> {
    let stack = read_stack(root);
    let dateien = artefakt_dateien(root);
    let defaults = baustein_default_kanten(&stack, &dateien);
    let vorhandene = read_edges(root);
    let merged = mit_default_kanten(vorhandene.clone(), &defaults);
    if merged != vorhandene {
        write_edges(root, &merged)?;
    }
    Ok(merged)
}

/// Einen **Paar-Default-Vorschlag bestätigen** (E20): eine Kante mit Herkunft `PaarDefault` zwischen
/// `derived` und `source` anlegen und persistieren. Dünn über [`add_persisted_edge_with_herkunft`].
pub fn confirm_pair_edge(root: &Path, derived: &str, source: &str) -> std::io::Result<Vec<Edge>> {
    add_persisted_edge_with_herkunft(root, derived, source, KantenHerkunft::PaarDefault)
}

/// Wie [`add_persisted_edge`], aber mit ausdrücklicher Herkunft (für Default-/Paar-Kanten).
pub fn add_persisted_edge_with_herkunft(
    root: &Path,
    derived: &str,
    source: &str,
    herkunft: KantenHerkunft,
) -> std::io::Result<Vec<Edge>> {
    let edges = add_edge(read_edges(root), Edge::with_herkunft(derived, source, herkunft));
    write_edges(root, &edges)?;
    Ok(edges)
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
    // Offene Paar-Default-Vorschläge (E20): deterministisch aus Stack + erfassten Dateien, abzüglich
    // bereits vorhandener Kanten. Rein berechnet; nur die Fakten werden hier gesammelt.
    let stack = read_stack(root);
    let dateien = artefakt_dateien(root);
    let vorschlaege = paar_default_vorschlaege(&stack, &dateien, &edges);
    EdgeView { edges, warnings, vorschlaege }
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

    /// Einen Paar-Default-Vorschlag bestätigen (E20): die persistierte Kante trägt Herkunft
    /// `PaarDefault` und ist endpunkt-dedupliziert gegen eine bereits vorhandene Hand-Kante.
    #[test]
    fn confirm_pair_edge_persists_with_paar_default_herkunft() {
        let dir = tmp();
        let edges = confirm_pair_edge(&dir, "fertigung", "elektronik/board").unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].herkunft, KantenHerkunft::PaarDefault);
        // auf der Platte ebenso
        let back = read_edges(&dir);
        assert_eq!(back, edges);

        // erneut bestätigen ist ein No-op (Endpunkt-Dedup)
        let again = confirm_pair_edge(&dir, "fertigung", "elektronik/board").unwrap();
        assert_eq!(again.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    /// `add_persisted_edge_with_herkunft` schreibt die Herkunft mit und rundtrippt durch die Datei.
    #[test]
    fn add_persisted_with_herkunft_round_trips() {
        let dir = tmp();
        add_persisted_edge_with_herkunft(&dir, "d", "s", KantenHerkunft::BausteinDefault).unwrap();
        let back = read_edges(&dir);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].herkunft, KantenHerkunft::BausteinDefault);
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
        // a single hand-mangled edge file is skipped per-file, never fatal.
        let path = EDGES.entry_path(&dir, &edge_key(&Edge::new("a", "b")));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json ]").unwrap();
        assert!(read_edges(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_one_file_per_edge_under_the_plm_dir() {
        let dir = tmp();
        add_persisted_edge(&dir, "a", "b").unwrap();
        assert!(
            EDGES.entry_path(&dir, &edge_key(&Edge::new("a", "b"))).is_file(),
            "each edge is its own file under _plm/kanten/"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// E54: two manually drawn edges land in two separate files — no shared line to merge-conflict on.
    #[test]
    fn two_edges_land_in_separate_files() {
        let dir = tmp();
        add_persisted_edge(&dir, "fertigung/stl", "mechanik/gehaeuse").unwrap();
        add_persisted_edge(&dir, "gerber/zip", "elektronik/board").unwrap();
        let a = EDGES.entry_path(&dir, &edge_key(&Edge::new("fertigung/stl", "mechanik/gehaeuse")));
        let b = EDGES.entry_path(&dir, &edge_key(&Edge::new("gerber/zip", "elektronik/board")));
        assert_ne!(a, b, "distinct endpoints -> distinct files");
        assert!(a.is_file() && b.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_legacy_dotfile_when_new_files_absent() {
        let dir = tmp();
        // a product that only has the oldest dotfile must not be silently emptied (migration).
        let legacy = vec![Edge::new("fertigung/stl", "mechanik/gehaeuse")];
        let json = serde_json::to_string_pretty(&legacy).unwrap();
        fs::write(legacy_edges_path(&dir), json).unwrap();

        let edges = read_edges(&dir);
        assert_eq!(edges, legacy, "legacy dotfile is read when no newer store exists");

        // the next write lands in the per-entry directory, leaving the dotfile as sediment.
        add_persisted_edge(&dir, "x/y", "p/q").unwrap();
        assert!(EDGES.dir_path(&dir).is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Migration from the intermediate `_plm/kanten.json` **array** file (ADR 0002 layout): its
    /// edges are read (not emptied), and the next write lays them out one file per edge.
    #[test]
    fn migrates_legacy_kanten_array_file() {
        let dir = tmp();
        let legacy = vec![
            Edge::new("fertigung/stl", "mechanik/gehaeuse"),
            Edge::with_herkunft("gerber/zip", "elektronik/board", KantenHerkunft::BausteinDefault),
        ];
        fs::create_dir_all(array_edges_path(&dir).parent().unwrap()).unwrap();
        fs::write(array_edges_path(&dir), serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        // read folds the array file in (array file wins while no per-entry directory exists).
        let read = read_edges(&dir);
        assert_eq!(read.len(), 2);
        assert!(read.contains(&legacy[0]) && read.contains(&legacy[1]));

        // the next write materialises the per-entry directory without losing the edges.
        add_persisted_edge(&dir, "neu/a", "neu/b").unwrap();
        assert!(EDGES.dir_path(&dir).is_dir());
        assert_eq!(read_edges(&dir).len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
