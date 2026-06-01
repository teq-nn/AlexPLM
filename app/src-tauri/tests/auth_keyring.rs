//! Keyring-backed auth tests for Issue #22 — using the lib's in-process shared-map test keystore
//! (never a real OS keystore), and a local repo (never a real remote). Proves: credentials
//! round-trip the keystore wrapper; the askpass helper answers username/password from the keystore;
//! and the ceremony writes a **credential-free** `.git/config`. Since ADR 0004 (Issue #92) the
//! ceremony is also a **credential-free writer**: `configure_remote` no longer touches the keystore
//! at all (the Konto is the sole writer of credentials); the host-keyed secret stored by the Konto
//! stays valid for askpass/`ensure_repo` unchanged.

use app_lib::askpass::{self, AskpassError};
use app_lib::credentials::{self, CredentialError};
use app_lib::setup::{configure_remote, REMOTE_NAME};
use std::path::Path;
use std::process::Command;

/// Install the lib's in-process shared-map keystore once for this test process, so nothing here
/// ever touches the real Secret Service / Credential Manager. It lives in the lib so it uses the
/// same `keyring` instance as the production code (a second crate instance here would not unify).
fn ensure_mock_keystore() {
    credentials::install_in_memory_keystore_for_tests();
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn seed_product(root: &Path) {
    let init = Command::new("git").arg("-C").arg(root).args(["init", "-b", "main"]).output().unwrap();
    assert!(init.status.success());
    let _ = Command::new("git").arg("-C").arg(root).args(["config", "user.name", "anna"]).output();
    let _ = Command::new("git").arg("-C").arg(root).args(["config", "user.email", "a@e.de"]).output();
    std::fs::write(root.join("README.md"), b"p").unwrap();
    let _ = Command::new("git").arg("-C").arg(root).args(["add", "-A"]).output();
    let _ = Command::new("git").arg("-C").arg(root).args(["commit", "-m", "init"]).output();
}

#[test]
fn credentials_round_trip_through_the_keystore() {
    ensure_mock_keystore();
    let host = "https://round-trip.example.de";

    credentials::store(host, "anna", "tok-12345").unwrap();
    assert_eq!(credentials::username(host).unwrap(), "anna");
    assert_eq!(credentials::token(host).unwrap(), "tok-12345");

    credentials::delete(host).unwrap();
    assert!(matches!(credentials::token(host), Err(CredentialError::NotFound)));
    assert!(matches!(credentials::username(host), Err(CredentialError::NotFound)));
}

#[test]
fn askpass_answers_username_and_token_from_the_keystore() {
    ensure_mock_keystore();
    let host = "https://askpass.example.de";
    credentials::store(host, "bjoern", "secret-token-xyz").unwrap();

    // git asks for the username first, then the password (token).
    let user = askpass::answer(&format!("Username for '{host}': ")).unwrap();
    assert_eq!(user, "bjoern");

    // The password prompt embeds the user in the URL; the helper must still find the host's token.
    let token = askpass::answer("Password for 'https://bjoern@askpass.example.de': ").unwrap();
    assert_eq!(token, "secret-token-xyz");
}

#[test]
fn askpass_fails_fast_for_an_unknown_host() {
    ensure_mock_keystore();
    // No credential stored for this host → the helper must error (git then fails fast, no hang).
    let res = askpass::answer("Username for 'https://never-stored.example.de': ");
    assert!(matches!(res, Err(AskpassError::Credential(CredentialError::NotFound))));
}

#[test]
fn ceremony_writes_credential_free_config_and_finds_konto_credentials() {
    ensure_mock_keystore();
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    std::fs::create_dir_all(&product).unwrap();
    seed_product(&product);

    // The Konto is the sole writer of credentials: a Konto-save stored the host-keyed secret once.
    let host = "https://forge.keytest.de";
    credentials::store(host, "anna", "tok-secret-xyz").unwrap();

    // The ceremony's connect step is now credential-free: it takes the Konto Base-URL + the
    // owner-default (the Konto username) and never touches the keystore.
    configure_remote(&product, host, "team", "ember", "anna").unwrap();

    // The remote URL in .git/config must be credential-free — no '@', no token.
    let url = git_out(&product, &["remote", "get-url", REMOTE_NAME]);
    assert_eq!(url, "https://forge.keytest.de/team/ember.git");
    assert!(!url.contains('@'), "remote url must not embed credentials: {url}");
    assert!(!url.contains("tok-secret-xyz"), "token leaked into .git/config: {url}");

    // The raw .git/config text must not contain the token anywhere either.
    let cfg = std::fs::read_to_string(product.join(".git/config")).unwrap();
    assert!(!cfg.contains("tok-secret-xyz"), "token leaked into .git/config file");
    assert!(!cfg.contains("anna:"), "user:token form leaked into .git/config file");

    // The host-keyed secret the Konto stored stays valid — askpass/`ensure_repo` find it unchanged.
    assert_eq!(credentials::username(host).unwrap(), "anna");
    assert_eq!(credentials::token(host).unwrap(), "tok-secret-xyz");
}
