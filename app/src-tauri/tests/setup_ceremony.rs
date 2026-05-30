//! End-to-end test of the Einrichtungs-Zeremonie glue (Issue #5, E41).
//!
//! A **bare local repo** stands in for the self-hosted Forgejo/Gitea server — the ceremony's
//! `git remote add` + first push are exercised against `file://…/remote.git`. NOTHING here ever
//! touches a real external server or sends real credentials anywhere; the pure validation /
//! normalization / locksverify / state-machine logic is covered by the unit tests in
//! `src/setup.rs`, so this file proves only that the side-effecting glue wires up against git.

use app_lib::setup::{configure_remote, publish_product, read_setup, SetupStage, REMOTE_NAME};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A product repo with one commit on `main`.
fn seed_product(root: &Path) {
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    std::fs::write(root.join("README.md"), b"produkt").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "init"]);
}

/// A bare repo standing in for the self-hosted server.
fn seed_bare_remote(root: &Path) {
    let out = Command::new("git")
        .args(["init", "--bare", "-b", "main"])
        .arg(root)
        .output()
        .expect("init bare");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn ceremony_connects_remote_then_first_push_publishes_and_settles() {
    let tmp = tempfile::tempdir().unwrap();
    let product2 = tmp.path().join("product2");
    std::fs::create_dir_all(&product2).unwrap();
    seed_product(&product2);

    // The bare repo standing in for the self-hosted server, laid out so a Forgejo-style
    // `<host>/team/remote.git` URL resolves to it on the local filesystem.
    let team_dir = tmp.path().join("team");
    std::fs::create_dir_all(&team_dir).unwrap();
    let bare = team_dir.join("remote.git");
    seed_bare_remote(&bare);

    // Before the ceremony: no server connected -> the one-time ceremony is offered.
    let before = read_setup(&product2).unwrap();
    assert_eq!(before.stage, SetupStage::NotConfigured);
    assert!(!before.has_remote);
    assert_eq!(before.clone_url, None);

    // Connect the "server": host = file://<tmp>, owner = team, repo = remote -> file URL
    // file://<tmp>/team/remote.git, which is the bare repo above (NOT a real Forgejo server).
    let host2 = format!("file://{}", tmp.path().display());
    let connected = configure_remote(&product2, &host2, "team", "remote", "", "").unwrap();

    // A server is now connected, but nothing published yet -> the publish step is offered.
    assert_eq!(connected.stage, SetupStage::RemoteSetNotPublished);
    assert!(connected.has_remote);
    assert!(!connected.has_published);
    // the colleague-facing clone URL is exposed and carries no credentials.
    let clone_url = connected.clone_url.clone().expect("clone url present once connected");
    assert!(clone_url.ends_with("/team/remote.git"));
    assert!(!clone_url.contains('@'), "clone url must not leak credentials: {clone_url}");

    // The remote is actually wired in git under the well-known name.
    let remote_url = git_out(&product2, &["remote", "get-url", REMOTE_NAME]);
    assert!(remote_url.ends_with("/team/remote.git"), "remote set: {remote_url}");

    // locksverify was enabled for the host (E41).
    let lv = git_out(
        &product2,
        &["config", "--local", &format!("lfs.{host2}/team/remote.git/info/lfs.locksverify")],
    );
    // The key git-lfs reads is scoped to the host origin, not the full repo path:
    let lv_host = git_out(
        &product2,
        &["config", "--local", &format!("lfs.{host2}/info/lfs.locksverify")],
    );
    assert!(lv == "true" || lv_host == "true", "locksverify enabled (got '{lv}' / '{lv_host}')");

    // First push publishes the product -> ceremony settles to "eingerichtet".
    let published = publish_product(&product2).unwrap();
    assert_eq!(published.stage, SetupStage::Eingerichtet);
    assert!(published.has_published);

    // The bare remote actually received the branch.
    let remote_main = git_out(&bare, &["rev-parse", "main"]);
    let local_main = git_out(&product2, &["rev-parse", "main"]);
    assert_eq!(remote_main, local_main, "first push published main to the remote");

    // Reading the state back is consistent (settled) — the ceremony is one-time and done.
    let after = read_setup(&product2).unwrap();
    assert_eq!(after.stage, SetupStage::Eingerichtet);
}

#[test]
fn publish_without_a_connected_server_refuses_clearly() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    std::fs::create_dir_all(&product).unwrap();
    seed_product(&product);

    let err = publish_product(&product).unwrap_err();
    assert!(
        err.to_string().contains("Server"),
        "publishing with no server must refuse with a clear message, got: {err}"
    );
}

#[test]
fn connecting_is_idempotent_updates_url_on_re_run() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    std::fs::create_dir_all(&product).unwrap();
    seed_product(&product);

    let host = format!("file://{}", tmp.path().display());
    configure_remote(&product, &host, "team", "first", "", "").unwrap();
    let second = configure_remote(&product, &host, "team", "second", "", "").unwrap();

    // The remote URL is updated in place, not duplicated.
    let url = git_out(&product, &["remote", "get-url", REMOTE_NAME]);
    assert!(url.ends_with("/team/second.git"), "re-run updates url: {url}");
    assert_eq!(second.stage, SetupStage::RemoteSetNotPublished);
}
