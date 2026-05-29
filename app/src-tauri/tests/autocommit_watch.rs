//! End-to-end auto-commit test (Issue #4): build a real git repo on disk, start the silent
//! watcher with a short quiet window, fire a burst of rapid saves, and assert that the burst
//! coalesces into exactly ONE new local commit / one Stand with the boring machine message.
//! No GUI needed — the watcher's Stand sink is injected directly.

use app_lib::autocommit::{machine_message, Stand};
use app_lib::watcher::watch_product_with_window;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn git(root: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_repo(root: &std::path::Path) {
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@t.test"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("README.md"), b"start").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "init"]);
}

fn commit_count(root: &std::path::Path) -> usize {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

fn last_message(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["log", "-1", "--pretty=%s"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Poll until `cond` holds or the deadline passes (the watcher runs on its own thread).
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

#[test]
fn rapid_saves_coalesce_into_one_commit_with_boring_message() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    let base = commit_count(root);

    // Capture every Stand the watcher emits.
    let stands: Arc<Mutex<Vec<Stand>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = stands.clone();

    // Short window so the test is fast but still exercises real debouncing.
    let window = Duration::from_millis(600);
    let _handle = watch_product_with_window(root, window, move |s| {
        sink.lock().unwrap().push(s);
    })
    .unwrap();

    // A burst of rapid saves to the same Baustein, faster than the quiet window.
    let target = root.join("elektronik/regler");
    std::fs::create_dir_all(&target).unwrap();
    for i in 0..6 {
        std::fs::write(target.join("regler.kicad_pcb"), format!("rev {i}")).unwrap();
        std::thread::sleep(Duration::from_millis(80));
    }

    // After the quiet window settles, exactly one new commit / one Stand should appear.
    let got = wait_until(Duration::from_secs(5), || !stands.lock().unwrap().is_empty());
    assert!(got, "expected a settled Stand within the timeout");

    // Give a moment to ensure no *second* commit sneaks in from the same burst.
    std::thread::sleep(Duration::from_millis(700));

    let stands = stands.lock().unwrap();
    assert_eq!(
        stands.len(),
        1,
        "rapid saves must coalesce into ONE Stand, got {stands:?}"
    );
    assert_eq!(
        commit_count(root),
        base + 1,
        "exactly one new commit for the burst"
    );

    // The commit message is the boring machine format; the user wrote none of it.
    let stand = &stands[0];
    assert_eq!(stand.path, "elektronik/regler/regler.kicad_pcb");
    assert_eq!(
        last_message(root),
        machine_message(&stand.path, &stand.timestamp)
    );
    assert!(last_message(root).starts_with("auto: "));
}

#[test]
fn two_separated_bursts_produce_two_commits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    let base = commit_count(root);

    let stands: Arc<Mutex<Vec<Stand>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = stands.clone();
    let window = Duration::from_millis(500);
    let _handle = watch_product_with_window(root, window, move |s| {
        sink.lock().unwrap().push(s);
    })
    .unwrap();

    let seen = |n: usize| stands.lock().unwrap().len() >= n;

    // First burst.
    std::fs::write(root.join("a.txt"), b"one").unwrap();
    assert!(wait_until(Duration::from_secs(5), || seen(1)));

    // Clear quiet gap, then a second, distinct burst.
    std::thread::sleep(Duration::from_millis(900));
    std::fs::write(root.join("b.txt"), b"two").unwrap();
    assert!(wait_until(Duration::from_secs(5), || seen(2)));

    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(
        stands.lock().unwrap().len(),
        2,
        "two separated bursts -> two Stände"
    );
    assert_eq!(commit_count(root), base + 2);
}
