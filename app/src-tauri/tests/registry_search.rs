//! Produkt-Registry + produktübergreifende Live-Suche tests (Issue #45, E45).
//!
//! Two layers, matching the project's "reiner Kern + Tabellentest" pattern:
//!
//! 1. **Pure core (no I/O).** Match predicate, ranking and result-merge judged purely over
//!    hand-built inputs — including the NEGATIVE cases (empty query, field ordering).
//! 2. **Fan-out glue over a real temp registry of throwaway product dirs.** This is the
//!    acceptance test: it builds a registry of three products (two reachable, one whose folder
//!    is removed → offline), runs the live fan-out, and asserts the offline product is reported
//!    honestly with a clear count — never silently dropped.

use app_lib::registry::{add_path, read_registry, registry_path, RegisteredProduct};
use app_lib::search::{fan_out, match_line, merge_results, rank_hit, HitField, SearchHit};
use std::fs;
use std::path::PathBuf;

// ---- Layer 1: pure core, no I/O -------------------------------------------------------------

#[test]
fn empty_query_matches_nothing() {
    assert!(!match_line("anything at all", ""));
}

#[test]
fn ranking_prefers_filename_then_plm_then_notes() {
    let f = rank_hit(HitField::Dateiname, "regler.f3d", "regler");
    let p = rank_hit(HitField::Plm, "freigegeben: regler", "regler");
    let n = rank_hit(HitField::VersionNotes, "regler nochmal", "regler");
    assert!(f > p && p > n);
}

#[test]
fn merge_sorts_best_first_and_is_stable() {
    let mk = |path: &str, score| SearchHit {
        product_path: path.to_string(),
        product_name: "P".to_string(),
        field: HitField::Dateiname,
        file: "x".to_string(),
        text: "x".to_string(),
        score,
    };
    let merged = merge_results(vec![vec![mk("/z", 5)], vec![mk("/a", 5), mk("/m", 9)]]);
    assert_eq!(merged[0].score, 9);
    // tie at 5 breaks by product path
    assert_eq!(merged[1].product_path, "/a");
    assert_eq!(merged[2].product_path, "/z");
}

// ---- Layer 2: live fan-out over a real temp registry ----------------------------------------

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "plm-registry-it-{tag}-{}-{}",
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

/// Build a throwaway product folder with a Baustein file, a `_plm` metadata file, and
/// `VERSION_NOTES.md`, all seeded with the given keyword so every source can be hit.
fn make_product(parent: &std::path::Path, name: &str, keyword: &str) -> PathBuf {
    let root = parent.join(name);
    fs::create_dir_all(root.join("mechanik")).unwrap();
    fs::create_dir_all(root.join("_plm")).unwrap();
    // Dateiname source: a file whose name carries the keyword.
    fs::write(root.join(format!("mechanik/{keyword}.f3d")), b"binary-ish").unwrap();
    // _plm source: a metadata line carrying the keyword.
    fs::write(
        root.join("_plm/product.json"),
        format!("{{\n  \"hinweis\": \"{keyword} ist pflicht\"\n}}\n"),
    )
    .unwrap();
    // VERSION_NOTES source: human revision text carrying the keyword.
    fs::write(
        root.join("VERSION_NOTES.md"),
        format!("# v1.0\n\n{keyword} ueberarbeitet und freigegeben\n"),
    )
    .unwrap();
    root
}

/// Persist a path-only registry file (the app-level store) from a list of product paths.
fn write_registry_file(dir: &std::path::Path, paths: &[&str]) -> PathBuf {
    let mut set: Vec<RegisteredProduct> = Vec::new();
    for p in paths {
        set = add_path(set, p);
    }
    let file = registry_path(dir);
    fs::create_dir_all(dir).unwrap();
    fs::write(&file, serde_json::to_string_pretty(&set).unwrap()).unwrap();
    file
}

/// Acceptance: the fan-out searches reachable repos AND reports the unreachable one honestly
/// with a clear count, never silently skipping it. Hits cover all three sources.
#[test]
fn fan_out_searches_reachable_and_reports_offline_honestly() {
    let base = tmp("fanout");
    let config = tmp("config");

    let p_alpha = make_product(&base, "alpha", "regler");
    let p_beta = make_product(&base, "beta", "regler");
    // A third product whose folder we then remove -> it must be reported offline.
    let p_ghost = make_product(&base, "ghost", "regler");

    let registry_file = write_registry_file(
        &config,
        &[
            p_alpha.to_str().unwrap(),
            p_beta.to_str().unwrap(),
            p_ghost.to_str().unwrap(),
        ],
    );

    // Make ghost unreachable (deleted / unmounted drive analogue).
    fs::remove_dir_all(&p_ghost).unwrap();

    let registry = read_registry(&registry_file);
    assert_eq!(registry.len(), 3, "three products registered (path-only)");

    let result = fan_out(&registry, "regler");

    // Honest offline reporting: ghost is reported, with a clear searched/total count.
    assert_eq!(result.total, 3);
    assert_eq!(result.searched, 2, "two reachable products searched");
    assert_eq!(result.offline.len(), 1, "the removed product is reported, not dropped");
    assert_eq!(result.offline[0].product_path, p_ghost.to_str().unwrap());
    assert!(!result.offline[0].reason.is_empty());

    // Hits cover the two reachable products across all three searched sources.
    assert!(!result.hits.is_empty());
    let fields: std::collections::HashSet<_> = result.hits.iter().map(|h| h.field).collect();
    assert!(fields.contains(&HitField::Dateiname), "filename source searched");
    assert!(fields.contains(&HitField::Plm), "_plm source searched");
    assert!(fields.contains(&HitField::VersionNotes), "VERSION_NOTES searched");

    // No hit belongs to the offline product.
    assert!(
        result.hits.iter().all(|h| h.product_path != p_ghost.to_str().unwrap()),
        "offline product contributes no hits"
    );

    // Best-first: a filename hit outranks content for the same query.
    assert_eq!(result.hits[0].field, HitField::Dateiname);

    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&config);
}

/// A query that matches nothing returns zero hits but still honestly counts every reachable
/// product as searched (nothing-found is not the same as offline).
#[test]
fn no_match_still_counts_all_reachable_as_searched() {
    let base = tmp("nomatch");
    let config = tmp("nomatch-config");
    let p = make_product(&base, "alpha", "regler");
    let registry_file = write_registry_file(&config, &[p.to_str().unwrap()]);

    let result = fan_out(&read_registry(&registry_file), "voellig-anderes-wort");
    assert!(result.hits.is_empty(), "no hits for an unrelated query");
    assert_eq!(result.searched, 1, "the product was still reachable + searched");
    assert!(result.offline.is_empty());

    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&config);
}

/// An empty query returns nothing and touches no product (no fan-out, no false offline).
#[test]
fn empty_query_returns_empty_without_offline() {
    let base = tmp("emptyq");
    let config = tmp("emptyq-config");
    let p = make_product(&base, "alpha", "regler");
    let registry_file = write_registry_file(&config, &[p.to_str().unwrap()]);

    let result = fan_out(&read_registry(&registry_file), "   ");
    assert!(result.hits.is_empty());
    assert!(result.offline.is_empty());
    assert_eq!(result.searched, 1);
    assert_eq!(result.total, 1);

    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&config);
}
