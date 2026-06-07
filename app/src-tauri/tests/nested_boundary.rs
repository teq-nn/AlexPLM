//! End-to-end test of the genested-`.git`-Grenze in the watcher (Issue #130, E50a).
//!
//! The pure stop-set predicate is proven exhaustively by the table tests in
//! `src/nestedboundary.rs`. This file pins down the side-effecting half against a real watcher and
//! a real repo: a write **behind** a nested `.git` (a framework-pulled `west`/ESP-IDF/`venv` tree)
//! must trigger **no commit** — no avalanche over foreign code — while a write in the product's own
//! tree still settles into exactly one Stand. So the foreign tree neither commits nor blocks the
//! product's own silent rhythm.

use app_lib::watcher::watch_product_with_window;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn init_repo(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("README.md"), b"start").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "init"]);
}

/// Poll until `cond` holds or the deadline passes (the watcher runs on its own thread). Same
/// idiom as `autocommit_watch.rs`, so the test stays robust under heavy parallel load.
fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

/// A nested `.git` is an opaque boundary: edits to thousands of vendored files behind it must NOT
/// produce a single Stand (no commit avalanche), while an edit to the product's own firmware does.
#[test]
fn writes_behind_nested_git_never_commit_but_own_tree_does() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    // A framework pulled `west` into firmware: its own real `.git`, plus a foreign working tree.
    let west = root.join("firmware/west");
    std::fs::create_dir_all(west.join("drivers")).unwrap();
    git(&west, &["init", "-q", "-b", "main"]); // a genuine nested repo, with a real `.git`

    let stands: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stand_sink = stands.clone();

    // A short quiet window so bursts settle quickly within the test.
    let window = Duration::from_millis(500);
    let _handle = watch_product_with_window(
        root,
        window,
        move |stand| stand_sink.lock().unwrap().push(stand.path),
        move |_path| {},
    )
    .unwrap();

    // Write into the foreign west tree — exactly the avalanche we must NOT commit. A handful of
    // distinct vendored files is enough to prove the point (the pure stop-set predicate proves the
    // boundary exhaustively); we keep the burst small so the watcher thread is not starved here.
    for i in 0..6 {
        std::fs::write(west.join("drivers/uart.c"), format!("// vendored rev {i}")).unwrap();
        std::fs::write(west.join(format!("gen_{i}.c")), b"vendored").unwrap();
        std::thread::sleep(Duration::from_millis(30));
    }

    // The product's own firmware still commits: the boundary stops at west, not before it. We use
    // this real edit as the synchronisation point — once a Stand for the firmware/app subtree
    // appears, the watcher has drained and settled a burst, so the "no Stand behind west" check
    // below is conclusive. (The settled path may be the file or its freshly-created parent dir,
    // depending on which notify event closes the quiet window — both are the product's own work.)
    std::fs::create_dir_all(root.join("firmware/app")).unwrap();
    std::fs::write(root.join("firmware/app/main.c"), b"int main(){}").unwrap();
    let got = wait_until(Duration::from_secs(15), || {
        stands.lock().unwrap().iter().any(|p| p.starts_with("firmware/app"))
    });
    assert!(got, "the product's own edit must settle into a Stand within the timeout");

    // No Stand may ever name a path behind the nested .git — no commit avalanche over foreign code.
    let seen = stands.lock().unwrap().clone();
    assert!(
        !seen.iter().any(|p| p.starts_with("firmware/west")),
        "no Stand may ever name a path behind the nested .git, got {seen:?}"
    );

    drop(_handle);
}
