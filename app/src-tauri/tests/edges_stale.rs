//! Edge Logic tests (Issue #10).
//!
//! Two layers, matching the project's "reiner Kern + Tabellentest" pattern:
//!
//! 1. **Pure edge set + timestamps -> warnings (no I/O).** The Edge Logic is exercised purely
//!    over a hand-built edge set and artifact timestamps — including the explicit NEGATIVE
//!    case (no edge = no warning). These are the acceptance-criterion tests.
//! 2. **Persistence glue.** A real temp folder round-trips the edge set through the store and
//!    the warning view is computed over it — exercising the thin I/O layer over the pure core.

use app_lib::edges::{stale_warnings, ArtifactStamp, Edge};
use app_lib::edgestore::{add_persisted_edge, read_edge_view, read_edges, remove_persisted_edge};
use std::fs;
use std::path::PathBuf;

fn stamp(path: &str, ts: &str) -> ArtifactStamp {
    ArtifactStamp {
        path: path.to_string(),
        timestamp: ts.to_string(),
    }
}

// ---- Layer 1: pure edge set + timestamps -> warnings, no I/O --------------------------

/// Acceptance: a warning shows IFF a manual edge exists AND the source is newer than the
/// derivation. The full relation as a table, judged purely.
#[test]
fn warning_iff_manual_edge_and_source_newer() {
    let edges = vec![Edge::new("fertigung/stl", "mechanik/gehaeuse")];

    // source newer than derivation -> stale warning
    let stale = vec![
        stamp("mechanik/gehaeuse", "2026-05-30T11:00:00Z"),
        stamp("fertigung/stl", "2026-05-30T09:00:00Z"),
    ];
    let w = stale_warnings(&edges, &stale);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].derived, "fertigung/stl");
    assert_eq!(w[0].source, "mechanik/gehaeuse");

    // source NOT newer (older) -> no warning
    let fresh = vec![
        stamp("mechanik/gehaeuse", "2026-05-30T08:00:00Z"),
        stamp("fertigung/stl", "2026-05-30T09:00:00Z"),
    ];
    assert!(stale_warnings(&edges, &fresh).is_empty());

    // source equal -> no warning
    let equal = vec![
        stamp("mechanik/gehaeuse", "2026-05-30T09:00:00Z"),
        stamp("fertigung/stl", "2026-05-30T09:00:00Z"),
    ];
    assert!(stale_warnings(&edges, &equal).is_empty());
}

/// Acceptance, explicit NEGATIVE test: zero edges yields zero warnings even when the
/// timestamps would otherwise look stale. No edge = no warning (E26/E40).
#[test]
fn no_edge_means_no_warning() {
    let artifacts = vec![
        stamp("mechanik/gehaeuse", "2026-05-30T11:00:00Z"),
        stamp("fertigung/stl", "2026-01-01T00:00:00Z"),
    ];
    assert!(
        stale_warnings(&[], &artifacts).is_empty(),
        "a product with zero edges must produce no warnings, regardless of timestamps"
    );
}

// ---- Layer 2: persistence glue over a real temp folder --------------------------------

fn tmp() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "plm-edges-it-{}-{}",
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

/// A fresh product (no edge file) is opt-in valid: zero edges, zero warnings — even though
/// it has artifacts whose timestamps differ.
#[test]
fn fresh_product_has_no_edges_and_no_warnings() {
    let dir = tmp();
    // two Bausteine with files so they have timestamps
    fs::create_dir_all(dir.join("mechanik/gehaeuse")).unwrap();
    fs::create_dir_all(dir.join("fertigung/stl")).unwrap();
    fs::write(dir.join("mechanik/gehaeuse/gehaeuse.f3d"), b"a").unwrap();
    fs::write(dir.join("fertigung/stl/part.stl"), b"b").unwrap();

    let view = read_edge_view(&dir);
    assert!(view.edges.is_empty(), "no edge file -> zero edges");
    assert!(view.warnings.is_empty(), "no edges -> no warnings");

    let _ = fs::remove_dir_all(&dir);
}

/// Drawing an edge persists it; the warning view then reflects the real artifact mtimes.
/// Touching the source after the derivation makes the derivation stale.
#[test]
fn drawn_edge_persists_and_surfaces_a_stale_warning() {
    let dir = tmp();
    fs::create_dir_all(dir.join("mechanik/gehaeuse")).unwrap();
    fs::create_dir_all(dir.join("fertigung/stl")).unwrap();
    // derivation written first (older), then source (newer)
    fs::write(dir.join("fertigung/stl/part.stl"), b"b").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(dir.join("mechanik/gehaeuse/gehaeuse.f3d"), b"a").unwrap();

    // draw: stl is derived from gehaeuse
    let edges = add_persisted_edge(&dir, "fertigung/stl", "mechanik/gehaeuse").unwrap();
    assert_eq!(edges.len(), 1);
    // persisted on disk and re-readable
    assert_eq!(read_edges(&dir), edges);

    let view = read_edge_view(&dir);
    assert_eq!(view.warnings.len(), 1, "source newer than derivation -> stale");
    assert_eq!(view.warnings[0].derived, "fertigung/stl");
    assert_eq!(view.warnings[0].source, "mechanik/gehaeuse");

    // removing the edge removes the warning (back to opt-in zero state)
    let edges = remove_persisted_edge(&dir, "fertigung/stl", "mechanik/gehaeuse").unwrap();
    assert!(edges.is_empty());
    assert!(read_edge_view(&dir).warnings.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
