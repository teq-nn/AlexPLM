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

use app_lib::pushglue::{
    personal_backup_ref, publish_branch, run_checkpoint, sicherungs_push, SHARED_BRANCH,
};
use app_lib::setup::is_published;
use app_lib::warden::{
    decide, Checkpoint, Cleanliness, LockState, PathKind, WardenAction, WardenSnapshot,
};
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

// --------------------------------------------------------------------------------------------
// Issue #83 — the daily rhythm is gated on "published". A product whose server-repo does not yet
// exist (remote configured, but the first publish never ran, so there is no upstream) must NOT run
// any networked git: against an absent Forgejo repo a Sicherungs-Push answers „Push to create is
// not enabled" and `git lfs locks` loops on the LFS endpoint's 401 until the bounded call times
// out — on every status tick. The Warden refuses silently until published instead.
// --------------------------------------------------------------------------------------------

/// A product repo with one commit on `main` and a remote configured, but **never published** — no
/// `push --set-upstream`, so the branch has no upstream and the bare "remote" stays empty (exactly
/// the state after the connect step of the ceremony, before "Veröffentlichen"). Mirrors a real
/// Forgejo repo that does not exist yet server-side.
fn seed_product_remote_but_unpublished(product: &Path, bare: &Path) {
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
    // NOTE: deliberately NO `push --set-upstream` — this product is connected but not published.
}

/// `is_published` is false while the product is only connected (remote set, no upstream) and flips
/// to true once the first publish sets the upstream — the predicate the rhythm gate keys on.
#[test]
fn is_published_is_false_until_the_first_publish_sets_an_upstream() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_remote_but_unpublished(&product, &bare);

    assert!(!is_published(&product), "connected-but-unpublished must read as not published");

    // The first publish (the ceremony's push --set-upstream) seeds the remote and sets the upstream.
    git(&product, &["push", "--set-upstream", "origin", "main"]);
    assert!(is_published(&product), "after the first publish the product is published");
}

/// The gate's observable effect: on a connected-but-unpublished product, a checkpoint over a dirty
/// path must `Refuse` WITHOUT running any networked git — no Sicherungs-Push reaches the (absent)
/// server-repo, so no personal backup ref appears. Without the gate the Warden would decide a
/// Sicherungs-Push here; the empty bare remote would even accept it. The gate keeps the rhythm quiet.
#[test]
fn unpublished_product_refuses_checkpoint_and_pushes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_remote_but_unpublished(&product, &bare);

    // A dirty, committed-then-edited text path — the very state that decides a Sicherungs-Push once
    // published (see `sicherungs_push_lands_in_personal_namespace_not_shared_main`).
    std::fs::write(product.join("firmware.c"), b"int main(){}").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: firmware.c, t"]);
    std::fs::write(product.join("firmware.c"), b"int main(){return 1;}").unwrap();

    let action = run_checkpoint(&product, "firmware.c", Checkpoint::Laufend).unwrap();
    assert_eq!(
        action,
        WardenAction::Refuse,
        "an unpublished product must Refuse the checkpoint, never attempt a push"
    );

    // CRUCIAL: nothing reached the remote — no personal backup ref was created, proving no
    // networked git ran (the gate short-circuited before the snapshot's `git lfs locks` and push).
    assert!(
        !git_ok(&bare, &["rev-parse", "--verify", &personal_backup_ref("anna", "main")]),
        "the gate must run no networked git on an unpublished product"
    );
}

// --------------------------------------------------------------------------------------------
// Issue #30 / E47 — a Freigabe carries the Revision label (the `version/*` tag) to the server, so
// a published Revision is visible server-side. Before the fix only the branch ref travelled and the
// tag stayed local. A label on an UNpublished Variante must not leak.
// --------------------------------------------------------------------------------------------

#[test]
fn freigabe_carries_the_revision_label_and_leaves_unpublished_ones_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_with_remote(&product, &bare);

    // A finished Stand on the shared line, named as a Revision (a lightweight `version/*` tag —
    // exactly what promote_to_milestone writes).
    std::fs::write(product.join("docs.md"), b"# done").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: docs.md, t"]);
    let published = git_out(&product, &["rev-parse", "main"]);
    git(&product, &["tag", "version/v0.4"]);

    // A Revision on a separate Variante that is NOT being published — its label must stay behind.
    git(&product, &["checkout", "-b", "variante"]);
    std::fs::write(product.join("exp.md"), b"# experiment").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: exp.md, t"]);
    git(&product, &["tag", "version/exp"]);
    git(&product, &["checkout", "main"]);

    publish_branch(&product).unwrap();

    // The published line's label reached the server and points at the published Stand.
    assert!(
        git_ok(&bare, &["rev-parse", "--verify", "refs/tags/version/v0.4"]),
        "the published Revision's label must travel to the server"
    );
    assert_eq!(
        git_out(&bare, &["rev-parse", "refs/tags/version/v0.4"]),
        published,
        "the label on the server points at the published Stand"
    );

    // The unpublished Variante's label did NOT leak (its Stand is not on the shared line).
    assert!(
        !git_ok(&bare, &["rev-parse", "--verify", "refs/tags/version/exp"]),
        "a label on an unpublished Variante must not reach the server"
    );

    // And the Versionsbaum reads the published Stand as „veröffentlicht", the Variante's Stand not.
    let graph = app_lib::graphread::read_graph(&product).unwrap();
    let node = |id: &str| graph.nodes.iter().find(|n| n.id == id).expect("node in tree");
    let variante_tip = git_out(&product, &["rev-parse", "variante"]);
    assert!(
        node(&published).veroeffentlicht,
        "the published Stand reads as veröffentlicht"
    );
    assert!(
        !node(&variante_tip).veroeffentlicht,
        "an unpublished Variante's Stand is not veröffentlicht"
    );
}

// --------------------------------------------------------------------------------------------
// Issue #54 — the two visible, manual push types: Sicherung never publishes; Freigabe does.
// --------------------------------------------------------------------------------------------

/// Issue #54 AC: the manual **Sicherungs-Push** (the toolbar „Sichern"-Knopf, backed by
/// `pushglue::sicherungs_push`) is the explicit personal backup. Even with a half-finished binary
/// edit in the worktree (the case the issue calls out — „inkl. halbfertiger Binärdateien"), it
/// backs the branch up into the personal namespace and **NEVER** moves the shared `main`.
#[test]
fn manual_sicherung_backs_up_half_finished_binary_and_never_publishes() {
    let tmp = tempfile::tempdir().unwrap();
    let product = tmp.path().join("product");
    let bare = tmp.path().join("remote.git");
    std::fs::create_dir_all(&product).unwrap();
    seed_product_with_remote(&product, &bare);

    let shared_before = git_out(&bare, &["rev-parse", SHARED_BRANCH]);

    // A half-finished binary, committed locally (the autocommit rhythm captures it as a Stand) —
    // exactly the kind of unfinished work a personal backup is meant to safeguard.
    std::fs::write(product.join("gehaeuse.f3d"), b"\x00\x01halffinished").unwrap();
    git(&product, &["add", "-A"]);
    git(&product, &["commit", "-m", "auto: gehaeuse.f3d, t"]);
    let local_main = git_out(&product, &["rev-parse", "main"]);

    // The visible manual backup press.
    sicherungs_push(&product).unwrap();

    // The personal backup ref now carries the half-finished binary…
    let backup_ref = personal_backup_ref("anna", "main");
    assert_eq!(
        git_out(&bare, &["rev-parse", &backup_ref]),
        local_main,
        "Sicherung backs the half-finished work up to the personal namespace"
    );
    // …but the shared main NEVER moved — others can never receive unfinished work as shared state.
    let shared_after = git_out(&bare, &["rev-parse", SHARED_BRANCH]);
    assert_eq!(shared_after, shared_before, "Sicherung must NEVER reach shared main");
    assert_ne!(shared_after, local_main, "the half-finished binary is NOT on shared main");
}

/// Issue #54 AC: the **Freigabe** publishes AND releases the lock — proven via the Warden's
/// decision (the safety-critical core). A held, dirty binary at a Meilenstein is the only state
/// that yields a Freigabe-Push, and that one action both publishes to shared `main` AND releases
/// the lock. Asserted at the decision boundary because `git lfs` is not assumed installed; the pure
/// `WardenAction` flags encode the exact carry-out the glue obeys.
#[test]
fn freigabe_decision_publishes_and_releases_the_lock_via_warden() {
    // A held binary with open work, at a Meilenstein → Freigabe-Push (the release).
    let snap = WardenSnapshot {
        kind: PathKind::Binary,
        lock: LockState::HeldByMe,
        clean: Cleanliness::Dirty,
        checkpoint: Checkpoint::Meilenstein,
    };
    let action = decide(snap);
    assert_eq!(action, WardenAction::FreigabePush, "held dirty binary at a Meilenstein -> Freigabe");
    // The one action that publishes to shared main AND, by the same decision, releases the lock.
    assert!(action.publishes_to_shared_main(), "Freigabe publishes to the shared main");
    assert!(action.releases_lock(), "Freigabe releases the lock (unlock-at-push)");

    // Conversely, the manual Sicherung (the laufend choice for the same held binary) NEVER does
    // either — it is the private backup, bound by the Binär-Invariante.
    let sicherung = decide(WardenSnapshot { checkpoint: Checkpoint::Laufend, ..snap });
    assert_eq!(sicherung, WardenAction::SicherungsPush, "laufend held binary -> Sicherung");
    assert!(!sicherung.publishes_to_shared_main(), "Sicherung must never publish");
    assert!(!sicherung.releases_lock(), "Sicherung must never release the lock");
}
