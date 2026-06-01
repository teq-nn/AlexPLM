//! End-to-end test of the Einrichtungs-Zeremonie glue (Issue #5, E41).
//!
//! A **bare local repo** stands in for the self-hosted Forgejo/Gitea server — the ceremony's
//! `git remote add` + first push are exercised against `file://…/remote.git`. NOTHING here ever
//! touches a real external server or sends real credentials anywhere; the pure validation /
//! normalization / locksverify / state-machine logic is covered by the unit tests in
//! `src/setup.rs`, so this file proves only that the side-effecting glue wires up against git.

use app_lib::setup::{
    configure_remote, publish_product, read_setup, PublishOutcome, SetupStage, REMOTE_NAME,
};
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

    // First push publishes the product -> ceremony settles to "eingerichtet". An empty Server-Repo
    // takes the plain first-push path (nothing to integrate), so the outcome is `Published`.
    let PublishOutcome::Published(published) = publish_product(&product2, None).unwrap() else {
        panic!("first publish to an empty Server-Repo must succeed, not raise a loud exception");
    };
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

    let err = publish_product(&product, None).unwrap_err();
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

/// Clone the bare remote into `dir` as a colleague who already has push rights, configured with an
/// identity so commits succeed. Stands in for "someone else seeded the Server-Repo" (Issue #44).
fn clone_as_colleague(bare: &Path, dir: &Path, name: &str) {
    let url = format!("file://{}", bare.display());
    let out = Command::new("git").args(["clone", &url]).arg(dir).output().expect("clone");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    git(dir, &["config", "user.name", name]);
    git(dir, &["config", "user.email", &format!("{name}@example.com")]);
}

/// Issue #44 (supersedes #35): re-publishing to a Server-Repo that has gained NEW free-text Stände
/// must integrate them silently and push — NOT fail with a raw non-fast-forward rejection. This is
/// exactly the „master -> master (fetch first)" repro, with the integrate-then-publish fix.
#[test]
fn republish_to_diverged_remote_integrates_text_silently() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    std::fs::create_dir_all(&product).unwrap();
    seed_product(&product);

    let team_dir = tmp.path().join("team");
    std::fs::create_dir_all(&team_dir).unwrap();
    let bare = team_dir.join("remote.git");
    seed_bare_remote(&bare);

    let host = format!("file://{}", tmp.path().display());
    configure_remote(&product, &host, "team", "remote", "", "").unwrap();
    // First publish seeds the remote (now non-empty, upstream set).
    assert!(matches!(
        publish_product(&product, None).unwrap(),
        PublishOutcome::Published(_)
    ));

    // A colleague pushes a NEW free-text Stand to the shared line — the remote is now ahead.
    let colleague = tmp.path().join("colleague");
    clone_as_colleague(&bare, &colleague, "ben");
    std::fs::write(colleague.join("docs.md"), b"# docs from ben\n").unwrap();
    git(&colleague, &["add", "-A"]);
    git(&colleague, &["commit", "-m", "auto: docs.md"]);
    git(&colleague, &["push", "origin", "main"]);

    // The user diverges locally with their own free-text Stand, then RE-publishes. Without the fix
    // this push is rejected „fetch first"; with it, the colleague's Stand integrates silently.
    std::fs::write(product.join("notes.txt"), b"meine notizen\n").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: notes.txt"]);

    let outcome = publish_product(&product, Some("Ben".to_string())).unwrap();
    assert!(
        matches!(outcome, PublishOutcome::Published(_)),
        "diverged free-text remote must integrate silently and publish, got: {outcome:?}"
    );

    // The colleague's Stand actually arrived locally (the silent merge ran)…
    assert!(product.join("docs.md").exists(), "the silent integrate brought in the colleague's Stand");
    // …and the remote now matches the local line (the re-push completed as a fast-forward).
    assert_eq!(
        git_out(&bare, &["rev-parse", "main"]),
        git_out(&product, &["rev-parse", "main"]),
        "re-publish pushed the integrated line to the remote"
    );
}

/// Issue #44: when the diverged Server-Repo and the local product contradict on an UNMERGEABLE
/// artifact (a binary), publishing must STOP with the single domain-language exception — never
/// merge (which would corrupt the binary) and never push. The question carries no git marker.
#[test]
fn republish_to_diverged_remote_on_binary_raises_loud_without_pushing() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    std::fs::create_dir_all(&product).unwrap();
    // Baseline carries a binary both sides can later change.
    git(&product, &["init", "-b", "main"]);
    git(&product, &["config", "user.name", "anna"]);
    git(&product, &["config", "user.email", "anna@example.com"]);
    std::fs::write(product.join("gehaeuse.f3d"), b"BINARYv1").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "init"]);

    let team_dir = tmp.path().join("team");
    std::fs::create_dir_all(&team_dir).unwrap();
    let bare = team_dir.join("remote.git");
    seed_bare_remote(&bare);

    let host = format!("file://{}", tmp.path().display());
    configure_remote(&product, &host, "team", "remote", "", "").unwrap();
    assert!(matches!(
        publish_product(&product, None).unwrap(),
        PublishOutcome::Published(_)
    ));

    // Colleague changes the binary and pushes; the user changes the SAME binary locally.
    let colleague = tmp.path().join("colleague");
    clone_as_colleague(&bare, &colleague, "ben");
    std::fs::write(colleague.join("gehaeuse.f3d"), b"BINARYv2-ben").unwrap();
    git(&colleague, &["add", "-A"]);
    git(&colleague, &["commit", "-m", "auto: gehaeuse"]);
    git(&colleague, &["push", "origin", "main"]);

    std::fs::write(product.join("gehaeuse.f3d"), b"BINARYv2-anna").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: gehaeuse"]);

    let remote_before = git_out(&bare, &["rev-parse", "main"]);
    let outcome = publish_product(&product, Some("Ben".to_string())).unwrap();

    let PublishOutcome::LauteAusnahme(q) = outcome else {
        panic!("a binary contradiction must raise the loud exception, got: {outcome:?}");
    };
    // The question is domain language and carries no git marker (the E41 acid test).
    assert!(!q.contains_git_marker(), "loud publish question leaked a git marker: {q:?}");
    assert!(
        q.artefakte.iter().any(|a| a.contains("gehaeuse")),
        "the contested binary is named as an artifact: {:?}",
        q.artefakte
    );

    // The push was NOT performed — the remote is untouched and the binary was not merged.
    assert_eq!(
        git_out(&bare, &["rev-parse", "main"]),
        remote_before,
        "a loud exception must not push"
    );
    let bytes = std::fs::read(product.join("gehaeuse.f3d")).unwrap();
    assert_eq!(bytes, b"BINARYv2-anna", "no merge ran, so the local binary is untouched");
}
