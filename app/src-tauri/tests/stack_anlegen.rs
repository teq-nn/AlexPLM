//! Acceptance test for Issue #50 — „Stack aus der Bibliothek anlegen (Kopie, anti-drift) + Tool
//! erweitern".
//!
//! The unit slices already cover the pure pieces (`stackstore.rs`: copy/extend/anti-drift;
//! `onboardglue.rs`: marker blocks; `markerblock.rs`: idempotence). This end-to-end glue test
//! exercises the exact public API the Tauri commands call (`create_product_stack` →
//! `onboard_stack_dotfiles` → `extend_product_stack`), over real temp folders, against the
//! Issue #50 acceptance criteria:
//!
//! 1. Confirming copies the chosen stack into `_plm/stack.json` AND onboards each Baustein
//!    (its marker blocks land in the product's dotfiles).
//! 2. The product stack is a copy — a later Bibliothek change never reaches the product
//!    (the dedicated anti-drift assertion lives in `bibliothek_antidrift.rs`; here we re-check
//!    that the materialised dotfiles reflect the COPY, not a later edit).
//! 3. A Baustein can be added additively after creation — its block is onboarded too, and the
//!    pre-existing Baustein's copy is left verbatim.
//! 4. Creating with a minimal selection still yields a valid, openable product (a single-Baustein
//!    stack reads back intact and carries its onboarding).

use app_lib::baustein::{Baustein, Oeffnen, Toolstack};
use app_lib::bibliothek::Bibliothek;
use app_lib::onboardglue::onboard_stack_dotfiles;
use app_lib::stackstore::{create_product_stack, extend_product_stack, read_stack};
use std::path::PathBuf;

fn baustein(id: &str, version: u32, heimat: &str, globs: &[&str], ignore: &[&str]) -> Baustein {
    Baustein {
        id: id.to_string(),
        version,
        name: id.to_string(),
        heimat: heimat.to_string(),
        globs: globs.iter().map(|s| s.to_string()).collect(),
        ignore: ignore.iter().map(|s| s.to_string()).collect(),
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
        "plm-stack50-{tag}-{}-{}",
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

/// Seed a Bibliothek with two CAD/EE Bausteine + a standard toolstack and return the lib handle.
fn seeded_lib(lib_dir: &std::path::Path) -> Bibliothek {
    let lib = Bibliothek::new(lib_dir);
    lib.seed_from(
        &[
            baustein("kicad", 1, "elektronik", &["*.kicad_pcb"], &["*.autosave"]),
            baustein("fusion", 1, "mechanik", &["*.f3d"], &["fusion-backup/"]),
        ],
        &[Toolstack {
            id: "standard-hw".to_string(),
            name: "Standard Hardware".to_string(),
            baustein_ids: vec!["kicad".to_string(), "fusion".to_string()],
        }],
    )
    .unwrap();
    lib
}

/// AC: confirming a chosen standard stack copies it into `_plm/stack.json` AND onboards each
/// Baustein — every chosen Baustein owns its idempotent marker block in the product's dotfiles.
#[test]
fn confirming_copies_the_stack_and_onboards_every_baustein() {
    let lib_dir = tmp("lib");
    let product = tmp("product");
    let lib = seeded_lib(&lib_dir);

    // The product-creation ceremony: resolve the chosen toolstack to its ids, copy them in.
    let ids = lib.read_toolstack("standard-hw").unwrap().baustein_ids;
    let stack = create_product_stack(&product, &lib, &ids, Some("standard-hw".to_string())).unwrap();
    // Then onboard each Baustein (the command does this right after the copy).
    onboard_stack_dotfiles(&product, &stack).unwrap();

    // The copy landed in _plm/stack.json with the provenance stamp.
    assert_eq!(stack.bausteine.len(), 2);
    assert_eq!(read_stack(&product), stack);

    // Each Baustein got its own marker block in the product's dotfiles.
    let gitignore = std::fs::read_to_string(product.join(".gitignore")).unwrap();
    let gitattributes = std::fs::read_to_string(product.join(".gitattributes")).unwrap();
    assert!(gitignore.contains("# >>> baustein: kicad >>>"));
    assert!(gitignore.contains("*.autosave"));
    assert!(gitignore.contains("# >>> baustein: fusion >>>"));
    assert!(gitignore.contains("fusion-backup/"));
    // kicad_pcb is a lockable EE source → it lands under LFS/lockable; f3d (CAD) too.
    assert!(gitattributes.contains("*.kicad_pcb filter=lfs"));
    assert!(gitattributes.contains("*.f3d filter=lfs"));

    let _ = std::fs::remove_dir_all(&lib_dir);
    let _ = std::fs::remove_dir_all(&product);
}

/// AC: a Baustein can be added additively after creation — „diesmal Zephyr statt PlatformIO".
/// The new Baustein is copied + onboarded; the pre-existing copy is left verbatim (anti-drift),
/// and naming the already-present id again is a quiet no-op.
#[test]
fn a_baustein_can_be_added_additively_after_creation() {
    let lib_dir = tmp("lib");
    let product = tmp("product");
    let lib = seeded_lib(&lib_dir);

    // Start with a minimal one-Baustein product.
    let stack = create_product_stack(&product, &lib, &["kicad".to_string()], None).unwrap();
    onboard_stack_dotfiles(&product, &stack).unwrap();
    assert_eq!(read_stack(&product).bausteine.len(), 1);

    // The Bibliothek later ships a new Baustein the user wants this time.
    lib.write_baustein(&baustein("zephyr", 1, "firmware", &["*.c"], &["build/", "twister-out/"]))
        .unwrap();

    // Extend additively; naming kicad again must be skipped (already copied).
    let extended =
        extend_product_stack(&product, &lib, &["kicad".to_string(), "zephyr".to_string()]).unwrap();
    onboard_stack_dotfiles(&product, &extended).unwrap();

    assert_eq!(extended.bausteine.len(), 2);
    assert_eq!(extended.bausteine[0].baustein.id, "kicad");
    assert_eq!(extended.bausteine[1].baustein.id, "zephyr");

    // The new Baustein's block was onboarded into the dotfiles alongside the existing one.
    let gitignore = std::fs::read_to_string(product.join(".gitignore")).unwrap();
    assert!(gitignore.contains("# >>> baustein: kicad >>>"));
    assert!(gitignore.contains("# >>> baustein: zephyr >>>"));
    assert!(gitignore.contains("twister-out/"));
    // Exactly one kicad block (no duplication from the second onboarding pass).
    assert_eq!(gitignore.matches("# >>> baustein: kicad >>>").count(), 1);

    let _ = std::fs::remove_dir_all(&lib_dir);
    let _ = std::fs::remove_dir_all(&product);
}

/// AC: a later Bibliothek change must not alter the already-materialised product — neither the
/// copied stack nor the onboarded dotfiles drift after a central edit.
#[test]
fn a_later_bibliothek_change_does_not_alter_the_materialised_product() {
    let lib_dir = tmp("lib");
    let product = tmp("product");
    let lib = seeded_lib(&lib_dir);

    let stack = create_product_stack(&product, &lib, &["kicad".to_string()], None).unwrap();
    onboard_stack_dotfiles(&product, &stack).unwrap();
    let stack_before = read_stack(&product);
    let ignore_before = std::fs::read_to_string(product.join(".gitignore")).unwrap();

    // Central edit: bump version, change ignore patterns, even retire the Baustein.
    let mut mutated = lib.read_baustein("kicad").unwrap();
    mutated.version = 99;
    mutated.ignore = vec!["ganz-anders/".to_string()];
    mutated.stillgelegt = true;
    lib.write_baustein(&mutated).unwrap();

    // The product's copied stack is untouched (anti-drift).
    assert_eq!(read_stack(&product), stack_before);
    // Re-onboarding from the (unchanged) product stack reproduces the same dotfiles — the central
    // edit never reached them.
    onboard_stack_dotfiles(&product, &read_stack(&product)).unwrap();
    let ignore_after = std::fs::read_to_string(product.join(".gitignore")).unwrap();
    assert_eq!(ignore_after, ignore_before);
    assert!(ignore_after.contains("*.autosave"));
    assert!(!ignore_after.contains("ganz-anders/"));

    let _ = std::fs::remove_dir_all(&lib_dir);
    let _ = std::fs::remove_dir_all(&product);
}

/// AC: creating with a minimal selection still yields a valid, openable product — a single-Baustein
/// stack reads back intact and is self-contained even if the Bibliothek later vanishes.
#[test]
fn a_minimal_selection_yields_a_valid_openable_product() {
    let lib_dir = tmp("lib");
    let product = tmp("product");
    let lib = Bibliothek::new(&lib_dir);
    lib.seed_from(&[baustein("doku", 1, "doku", &["*.md"], &[])], &[]).unwrap();

    // The smallest meaningful product: one Baustein, no named toolstack.
    let stack = create_product_stack(&product, &lib, &["doku".to_string()], None).unwrap();
    onboard_stack_dotfiles(&product, &stack).unwrap();
    assert_eq!(stack.bausteine.len(), 1);
    assert_eq!(stack.bausteine[0].baustein.id, "doku");

    // The product is self-contained: it reads back intact even if the Bibliothek is gone.
    std::fs::remove_dir_all(&lib_dir).unwrap();
    let reread = read_stack(&product);
    assert_eq!(reread, stack);
    assert_eq!(reread.bausteine[0].baustein.id, "doku");

    let _ = std::fs::remove_dir_all(&product);
}
