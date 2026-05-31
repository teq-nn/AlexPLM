//! Anti-Drift acceptance test (Issue #39, ADR 0003).
//!
//! The required acceptance criterion: a product stack copied into `_plm/stack.json` is a
//! **self-contained copy** with no live link to the Bibliothek. After mutating the Bibliothek
//! Baustein on disk, re-reading the product stack must yield the UNCHANGED copy.
//!
//! Two layers, matching the house pattern:
//! 1. The pure seeding *decision* over a hand-built table (in `bibliothek.rs` unit tests).
//! 2. This end-to-end glue test over real temp folders: seed → copy into a product → mutate the
//!    Bibliothek → assert the product is untouched.

use app_lib::baustein::{Baustein, Oeffnen, Toolstack};
use app_lib::bibliothek::Bibliothek;
use app_lib::stackstore::{create_product_stack, read_stack};
use std::path::PathBuf;

fn baustein(id: &str, version: u32, heimat: &str, glob: &str) -> Baustein {
    Baustein {
        id: id.to_string(),
        version,
        name: id.to_string(),
        heimat: heimat.to_string(),
        globs: vec![glob.to_string()],
        ignore: vec![],
        lfs: vec![],
        oeffnen: Oeffnen::Auto,
        startaufgaben: vec![],
        default_kanten: vec![],
        paar_default_kanten: vec![],
        stillgelegt: false,
    }
}

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "plm-antidrift-{tag}-{}-{}",
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
fn a_bibliothek_edit_never_alters_an_existing_product_stack() {
    let lib_dir = tmp("lib");
    let product = tmp("product");

    // Seed a Bibliothek with two default Bausteine + a toolstack referencing them.
    let lib = Bibliothek::new(&lib_dir);
    lib.seed_from(
        &[
            baustein("kicad", 1, "elektronik", "*.kicad_pro"),
            baustein("fusion", 1, "mechanik", "*.f3d"),
        ],
        &[Toolstack {
            id: "standard-hw".to_string(),
            name: "Standard Hardware".to_string(),
            baustein_ids: vec!["kicad".to_string(), "fusion".to_string()],
        }],
    )
    .unwrap();

    // Create a product carrying a copied stack of the chosen default Bausteine.
    let ids = lib.read_toolstack("standard-hw").unwrap().baustein_ids;
    let original = create_product_stack(&product, &lib, &ids, Some("standard-hw".to_string()))
        .unwrap();
    assert_eq!(original.bausteine.len(), 2);
    assert_eq!(original.bausteine[0].baustein.heimat, "elektronik");
    assert_eq!(original.bausteine[0].baustein.globs, vec!["*.kicad_pro".to_string()]);
    // provenance stamp recorded
    assert_eq!(original.bausteine[0].herkunft.from, "kicad");
    assert_eq!(original.bausteine[0].herkunft.version, 1);

    // ---- Now MUTATE the Bibliothek Baustein on disk (a "central fix" + a user edit) ----
    let mut mutated = lib.read_baustein("kicad").unwrap();
    mutated.version = 99;
    mutated.heimat = "GANZ-ANDERS".to_string();
    mutated.globs = vec!["*.totally-different".to_string()];
    mutated.stillgelegt = true;
    lib.write_baustein(&mutated).unwrap();
    // sanity: the Bibliothek really changed
    assert_eq!(lib.read_baustein("kicad").unwrap().heimat, "GANZ-ANDERS");

    // ---- Re-read the product stack: it must be UNCHANGED (anti-drift) ----
    let after = read_stack(&product);
    assert_eq!(
        after, original,
        "a Bibliothek edit must never alter an existing product stack (ADR 0003 anti-drift)"
    );
    // explicit field-level checks for clarity
    assert_eq!(after.bausteine[0].baustein.heimat, "elektronik");
    assert_eq!(after.bausteine[0].baustein.globs, vec!["*.kicad_pro".to_string()]);
    assert_eq!(after.bausteine[0].herkunft.version, 1);
    assert!(!after.bausteine[0].baustein.stillgelegt);

    let _ = std::fs::remove_dir_all(&lib_dir);
    let _ = std::fs::remove_dir_all(&product);
}

#[test]
fn product_stack_survives_a_missing_bibliothek() {
    // ADR 0003: the product functions even if the Bibliothek is gone — the copy is self-contained.
    let lib_dir = tmp("lib2");
    let product = tmp("product2");
    let lib = Bibliothek::new(&lib_dir);
    lib.seed_from(&[baustein("doku", 1, "doku", "*.md")], &[]).unwrap();
    let original = create_product_stack(&product, &lib, &["doku".to_string()], None).unwrap();

    // Blow away the entire Bibliothek.
    std::fs::remove_dir_all(&lib_dir).unwrap();

    // Product stack still reads back intact.
    assert_eq!(read_stack(&product), original);
    assert_eq!(read_stack(&product).bausteine[0].baustein.id, "doku");

    let _ = std::fs::remove_dir_all(&product);
}
