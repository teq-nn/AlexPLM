//! „Konto ändern & entfernen" tests (ADR 0004, Issue #91).
//!
//! Exercises the side-effecting glue the `clear_konto` command composes — the app-level Base-URL
//! JSON (`konto::{write,read,clear}_konto`) and the host-keyed keystore (`credentials`) — against
//! the lib's in-process shared-map test keystore (never a real OS keystore) and a local repo (never
//! a real remote). The pure normalize/interpret logic lives in `src/konto.rs` unit tests; this file
//! proves the lifecycle wiring:
//! - removing a Konto deletes the keystore entries AND the persisted Base-URL,
//! - removing is idempotent (no Konto / repeated entfernen is a no-op, not an error), and
//! - CRITICAL (ADR 0004, Kriterium 4): a product's `.git/config` remotes are UNTOUCHED by a clear.

use app_lib::credentials::{self, CredentialError};
use app_lib::konto::{clear_konto, konto_path, read_konto, write_konto, KontoConfig};
use std::path::Path;
use std::process::Command;

/// Install the lib's in-process shared-map keystore once, so nothing here ever touches the real
/// Secret Service / Credential Manager (same building block as `auth_keyring.rs`).
fn ensure_mock_keystore() {
    credentials::install_in_memory_keystore_for_tests();
}

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A product repo with one commit on `main` and a configured `origin` remote standing in for a
/// product that was already „angebunden" against the Konto host.
fn seed_product_with_remote(root: &Path, remote_url: &str) {
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "a@e.de"]);
    // Keep the seed commit independent of any environment that forces commit signing — the
    // invariant under test is the remote URL in .git/config, not the commit object itself.
    git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("README.md"), b"p").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "init"]);
    git(root, &["remote", "add", "origin", remote_url]);
}

/// Removing a Konto deletes BOTH the host-keyed keystore entries and the persisted Base-URL JSON,
/// leaving „kein Konto" (Kriterien 1 + 3).
#[test]
fn clear_konto_deletes_keystore_entries_and_persisted_base_url() {
    ensure_mock_keystore();
    let tmp = tempfile::tempdir().unwrap();
    let cfg_dir = tmp.path().join("config");
    let file = konto_path(&cfg_dir);
    let host = "https://forge.clear-test.de";

    // Set up a Konto: Base-URL JSON app-level + host-keyed credentials in the keystore.
    write_konto(
        &file,
        &KontoConfig { base_url: host.to_string(), account: "anna".to_string() },
    )
    .unwrap();
    credentials::store(host, "anna", "tok-secret-91").unwrap();
    assert!(read_konto(&file).is_some());
    assert_eq!(credentials::username(host).unwrap(), "anna");
    assert_eq!(credentials::token(host).unwrap(), "tok-secret-91");

    // „Konto entfernen": delete the keystore entries for the host, then remove the Base-URL JSON —
    // exactly what the `clear_konto` command composes.
    credentials::delete(host).unwrap();
    clear_konto(&file).unwrap();

    // read_konto now reports „kein Konto" (None), and the secret is gone from the keystore.
    assert_eq!(read_konto(&file), None);
    assert!(matches!(credentials::token(host), Err(CredentialError::NotFound)));
    assert!(matches!(credentials::username(host), Err(CredentialError::NotFound)));
}

/// „entfernen" with no existing Konto — and a repeated entfernen — is a no-op, not an error
/// (Kriterium 5). `credentials::delete` and `konto::clear_konto` are both idempotent.
#[test]
fn clear_konto_is_idempotent_without_an_existing_konto() {
    ensure_mock_keystore();
    let tmp = tempfile::tempdir().unwrap();
    let cfg_dir = tmp.path().join("config");
    let file = konto_path(&cfg_dir);
    let host = "https://forge.never-set.de";

    // No Konto was ever set: both the keystore delete and the JSON remove must succeed.
    assert_eq!(read_konto(&file), None);
    credentials::delete(host).expect("deleting absent credentials is success");
    clear_konto(&file).expect("clearing an absent konto is success");
    assert_eq!(read_konto(&file), None);

    // A second pass stays a clean no-op.
    credentials::delete(host).expect("repeated delete stays a no-op");
    clear_konto(&file).expect("repeated clear stays a no-op");
}

/// CRITICAL INVARIANT (ADR 0004, Kriterium 4): removing the Konto must NEVER touch the `.git/config`
/// remotes of existing products — no automatic rewriting, no mass-repoint. Local work continues; only
/// sharing pauses until a Konto is set again.
#[test]
fn clearing_konto_leaves_product_git_config_remotes_untouched() {
    ensure_mock_keystore();
    let tmp = tempfile::tempdir().unwrap();
    let cfg_dir = tmp.path().join("config");
    let file = konto_path(&cfg_dir);
    let host = "https://forge.invariant.de";

    // Two products already pointed at the Konto host (credential-free URLs, as the ceremony writes).
    let product_a = tmp.path().join("product-a");
    let product_b = tmp.path().join("product-b");
    std::fs::create_dir_all(&product_a).unwrap();
    std::fs::create_dir_all(&product_b).unwrap();
    let url_a = "https://forge.invariant.de/team/ember.git";
    let url_b = "https://forge.invariant.de/anna/widget.git";
    seed_product_with_remote(&product_a, url_a);
    seed_product_with_remote(&product_b, url_b);

    // A Konto is set up against that same host.
    write_konto(
        &file,
        &KontoConfig { base_url: host.to_string(), account: "anna".to_string() },
    )
    .unwrap();
    credentials::store(host, "anna", "tok-invariant").unwrap();

    // Capture the products' remote URLs + raw .git/config before removing the Konto.
    let before_a = git_out(&product_a, &["remote", "get-url", "origin"]);
    let before_b = git_out(&product_b, &["remote", "get-url", "origin"]);
    let cfg_a_before = std::fs::read_to_string(product_a.join(".git/config")).unwrap();
    let cfg_b_before = std::fs::read_to_string(product_b.join(".git/config")).unwrap();
    assert_eq!(before_a, url_a);
    assert_eq!(before_b, url_b);

    // „Konto entfernen": delete keystore entries + persisted Base-URL.
    credentials::delete(host).unwrap();
    clear_konto(&file).unwrap();
    assert_eq!(read_konto(&file), None);

    // The products' remotes are byte-for-byte unchanged — the Konto removal never reached them.
    assert_eq!(git_out(&product_a, &["remote", "get-url", "origin"]), url_a);
    assert_eq!(git_out(&product_b, &["remote", "get-url", "origin"]), url_b);
    assert_eq!(std::fs::read_to_string(product_a.join(".git/config")).unwrap(), cfg_a_before);
    assert_eq!(std::fs::read_to_string(product_b.join(".git/config")).unwrap(), cfg_b_before);
}
