//! End-to-end test of the Lock Warden push glue (Issue #9, E35).
//!
//! A **bare local repo** stands in for the self-hosted Forgejo/Gitea remote — the two push types
//! are exercised against `file://…/remote.git`. NOTHING here ever touches a real server or LFS
//! endpoint; the safety-critical decision logic (the Binär-Invariante, auto-unlock-iff-clean, the
//! full cross-product) is proven exhaustively by the pure table/property tests in `src/warden.rs`.
//! This file proves only that the side-effecting glue wires up against git: that a Sicherungs-Push
//! lands in the personal namespace and NOT on the shared `main`, and that a Freigabe-Push moves
//! the shared `main`. `git lfs` is not assumed installed, so the lock-bearing carry-outs are
//! verified at the snapshot/decision boundary rather than by driving a real LFS lock.

use app_lib::pushglue::{personal_backup_ref, run_checkpoint, sicherungs_push, SHARED_BRANCH};
use app_lib::warden::{Checkpoint, WardenAction};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git_ok(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A product repo with one commit on `main`, wired to a bare "remote".
fn seed_product_with_remote(product: &Path, bare: &Path) {
    let out = Command::new("git").args(["init", "--bare", "-b", "main"]).arg(bare).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    git(product, &["init", "-b", "main"]);
    git(product, &["config", "user.name", "anna"]);
    git(product, &["config", "user.email", "anna@example.com"]);
    std::fs::write(product.join("README.md"), b"produkt").unwrap();
    git(product, &["add", "-A"]);
    git(product, &["commit", "-m", "init"]);
    let url = format!("file://{}", bare.display());
    git(product, &["remote", "add", "origin", &url]);
    git(product, &["push", "--set-upstream", "origin", "main"]);
}

/// A product repo with one commit on `master` (NOT `main`), wired to a bare "remote" whose default
/// branch is `master` — the imported / fresh-`git init` case from Issue #64. The remote HEAD is
/// recorded locally (`git remote set-head -a`) just as a real `git clone` would, so the push glue
/// can resolve the actually-shared branch.
fn seed_product_with_remote_on_master(product: &Path, bare: &Path) {
    let out = Command::new("git").args(["init", "--bare", "-b", "master"]).arg(bare).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    git(product, &["init", "-b", "master"]);
    git(product, &["config", "user.name", "anna"]);
    git(product, &["config", "user.email", "anna@example.com"]);
    std::fs::write(product.join("README.md"), b"produkt").unwrap();
    git(product, &["add", "-A"]);
    git(product, &["commit", "-m", "init"]);
    let url = format!("file://{}", bare.display());
    git(product, &["remote", "add", "origin", &url]);
    git(product, &["push", "--set-upstream", "origin", "master"]);
    // Record the remote default branch locally, as a real clone would, so refs/remotes/origin/HEAD
    // resolves to origin/master.
    git(product, &["remote", "set-head", "origin", "-a"]);
}

/// Issue #64: on a repo whose shared branch is `master` (not `main`), a Freigabe-Push must land on
/// the actually-shared `master` — never a silent `master:main` split — and create no stray `main`.
#[test]
fn freigabe_push_lands_on_shared_master_not_main() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_with_remote_on_master(&product, &bare);

    // A finished, committed change to publish.
    std::fs::write(product.join("docs.md"), b"# done").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: docs.md, t"]);
    let local_master = git_out(&product, &["rev-parse", "master"]);

    // Dirty the text path so the snapshot decides Freigabe at a Meilenstein.
    std::fs::write(product.join("docs.md"), b"# done, edited").unwrap();

    let action = run_checkpoint(&product, "docs.md", Checkpoint::Meilenstein).unwrap();
    assert_eq!(action, WardenAction::FreigabePush, "dirty text at a Meilenstein -> Freigabe-Push");

    // The shared branch is `master`, and it advanced to the published commit.
    let shared_after = git_out(&bare, &["rev-parse", "master"]);
    assert_eq!(shared_after, local_master, "Freigabe-Push publishes to the shared master");

    // CRUCIAL: no stray `main` was created on the remote — there is no silent master/main split.
    assert!(
        !git_ok(&bare, &["rev-parse", "--verify", "main"]),
        "Freigabe-Push must not create a stray `main` on a master-repo"
    );
}

/// A laufender Checkpoint on a dirty text file → Sicherungs-Push: lands in the personal namespace
/// on the remote and leaves the shared `main` untouched (E35: backup yes, release no).
#[test]
fn sicherungs_push_lands_in_personal_namespace_not_shared_main() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_with_remote(&product, &bare);

    let shared_before = git_out(&bare, &["rev-parse", SHARED_BRANCH]);

    // A mergeable-text edit, committed locally (a local intermediate commit).
    std::fs::write(product.join("firmware.c"), b"int main(){}").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: firmware.c, t"]);

    // Drive the glue's Sicherungs-Push directly.
    sicherungs_push(&product).unwrap();

    // The personal backup ref now exists on the remote and points at our local main.
    let backup_ref = personal_backup_ref("anna", "main");
    assert!(
        git_ok(&bare, &["rev-parse", "--verify", &backup_ref]),
        "Sicherungs-Push must create the personal backup ref {backup_ref} on the remote"
    );
    let local_main = git_out(&product, &["rev-parse", "main"]);
    let backup = git_out(&bare, &["rev-parse", &backup_ref]);
    assert_eq!(backup, local_main, "backup ref mirrors the local branch");

    // CRUCIAL: the shared main on the remote did NOT move — a Sicherungs-Push never publishes.
    let shared_after = git_out(&bare, &["rev-parse", SHARED_BRANCH]);
    assert_eq!(shared_after, shared_before, "Sicherungs-Push must not move shared main");
    assert_ne!(shared_after, local_main, "the new local commit is NOT on shared main");
}

/// A Meilenstein checkpoint on a dirty text file → Freigabe-Push: moves the shared `main` on the
/// remote to the published work (the public act). (Text holds no lock, so there is nothing to
/// unlock — the lock-bearing path is covered by the pure warden tests + the snapshot test below.)
#[test]
fn freigabe_push_moves_shared_main_on_the_remote() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_with_remote(&product, &bare);

    // A finished, committed change to publish.
    std::fs::write(product.join("docs.md"), b"# done").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: docs.md, t"]);
    let local_main = git_out(&product, &["rev-parse", "main"]);

    // Make the worktree dirty on the text path so the snapshot decides Freigabe at a Meilenstein.
    std::fs::write(product.join("docs.md"), b"# done, edited").unwrap();

    let action = run_checkpoint(&product, "docs.md", Checkpoint::Meilenstein).unwrap();
    assert_eq!(action, WardenAction::FreigabePush, "dirty text at a Meilenstein -> Freigabe-Push");

    // The shared main on the remote advanced to the published commit (the public act).
    let shared_after = git_out(&bare, &["rev-parse", SHARED_BRANCH]);
    assert_eq!(shared_after, local_main, "Freigabe-Push publishes to the shared main");
}

/// A clean, unedited text path at any checkpoint → Refuse: nothing to move, the remote is
/// untouched (no push of any kind).
#[test]
fn clean_unlocked_path_refuses_and_touches_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_with_remote(&product, &bare);

    let shared_before = git_out(&bare, &["rev-parse", SHARED_BRANCH]);

    // README.md is committed and clean, no lock -> Refuse at a laufender checkpoint.
    let action = run_checkpoint(&product, "README.md", Checkpoint::Laufend).unwrap();
    assert_eq!(action, WardenAction::Refuse, "clean unlocked path -> Refuse");

    // Nothing moved on the remote and no personal backup ref was created.
    assert_eq!(git_out(&bare, &["rev-parse", SHARED_BRANCH]), shared_before);
    assert!(
        !git_ok(&bare, &["rev-parse", "--verify", &personal_backup_ref("anna", "main")]),
        "Refuse creates no backup ref"
    );
}
