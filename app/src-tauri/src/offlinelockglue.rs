//! Side-effecting glue for the **Offline-Lock / Absichts-Sperre** (Issue #136, E49b).
//!
//! Everything in [`crate::offlinelock`] is a pure decision; this module is the thin, isolated layer
//! that (a) **opens a lockable binary even with no reachable lock server** — recording a local
//! Absichts-Sperre instead of failing — (b) persists/reads those intent-locks under `.plm-local/`,
//! (c) tells the card whether a path is „offline bearbeitet, Sperre nicht bestätigt", and (d) on
//! **reconnect** reads the real server locks and lets the pure Eingang-B reconciler judge the local
//! intents against them, surfacing a detected double-edit as the existing laute Ausnahme.
//!
//! ## `.plm-local/` — the local, ungeteilte store (E38)
//!
//! An Absichts-Sperre is a **local** fact: it records what *this* machine intended while offline,
//! and it must **never** travel to a colleague's clone (that would resurrect the very „false safety"
//! E49b removes — a colleague would see a lock that was only ever an unconfirmed local intent). So
//! it lives under `.plm-local/`, kept out of every shared stand the same way the „Als Ordner öffnen"
//! worktrees are (E38): via `.git/info/exclude`, the **local** ignore list, never the committed
//! `.gitignore`. The committed, shared `_plm/` store is the wrong home for it precisely because it
//! *is* shared.
//!
//! ## Open even when the lock server is unreachable
//!
//! [`acquire_lock_or_intent`] is the E49b entry the open-action uses instead of a bare
//! [`crate::lockglue::acquire_lock`]. It tries to take the real `git lfs lock`; if that fails
//! because the **server is unreachable** (network down, auth/keystore problem, a timed-out
//! credential loop) it records an Absichts-Sperre, makes the file writable locally and reports
//! [`OpenLock::OfflineIntent`] — the binary opens, no false safety. A **foreign-held** lock is a
//! different beast: that is real, present coordination the user must see, so it stays a loud error.

use crate::lockglue::{current_owner_name, set_writable, snapshot};
use crate::locks::is_lockable;
use crate::offlinelock::{reconcile_intents, AbsichtsSperre, IntentReconcile, ServerSperre};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The local, ungeteilte store directory (E38) for facts that must never travel to a colleague's
/// clone — today exactly the Absichts-Sperren. A sibling of the committed `_plm/`, but kept out of
/// every shared stand via `.git/info/exclude` (see [`ensure_excluded`]).
const LOCAL_DIR: &str = ".plm-local";

/// The intent-lock record inside `.plm-local/`. ADR-0002-degrading: missing/empty/corrupt ⇒ an
/// empty list, so a connect with no offline intents simply has nothing to reconcile — never an error.
const INTENTS_FILE: &str = "absichts-sperren.json";

/// The outcome of opening a lockable binary through the offline-aware path (Issue #136, E49b). One
/// of three; total. The non-lockable case is folded into [`OpenLock::Locked`] (a no-op „nothing to
/// coordinate", mirroring [`crate::lockglue::acquire_lock`]'s `Ok(false)` for text).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OpenLock {
    /// The real `git lfs lock` was taken (or the path is mergeable text / pre-publish, so there was
    /// nothing to coordinate). The binary is safely ours to edit; no Absichts-Sperre recorded.
    Locked,
    /// The lock **server was unreachable**, so a local Absichts-Sperre was recorded and the file was
    /// made writable: the binary opens, but the card must say „offline bearbeitet, Sperre nicht
    /// bestätigt" — no false safety. Reconciled on the next connect.
    OfflineIntent,
}

impl OpenLock {
    /// Whether this open recorded an unconfirmed offline intent (the card shows the honest warning).
    pub fn is_offline_intent(&self) -> bool {
        matches!(self, OpenLock::OfflineIntent)
    }
}

/// The persisted shape of one Absichts-Sperre under `.plm-local/` (Issue #136, E49b). Serialised
/// separately from the pure [`AbsichtsSperre`] so the on-disk schema is the glue's concern and the
/// pure core stays free of serde derives it does not need.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct StoredIntent {
    /// The product-relative artifact path the user opened offline.
    path: String,
}

/// Absolute path of the `.plm-local/` intent store under a product `root`.
fn intents_path(root: &Path) -> PathBuf {
    root.join(LOCAL_DIR).join(INTENTS_FILE)
}

/// Read the recorded Absichts-Sperren for `root`, lifted into the pure [`AbsichtsSperre`] the
/// reconciler reads. ADR-0002-degrading: missing/empty/corrupt ⇒ an empty list (a connect with no
/// offline intents just reconciles nothing), never an error.
pub fn read_intent_locks(root: &Path) -> Vec<AbsichtsSperre> {
    let stored: Vec<StoredIntent> = crate::plmstore::read_or_default(&intents_path(root));
    stored.into_iter().map(|s| AbsichtsSperre { path: s.path }).collect()
}

/// Whether `rel_path` currently carries an unconfirmed Absichts-Sperre — the fact the card turns
/// into „offline bearbeitet, Sperre nicht bestätigt". Pure read of the local store.
pub fn has_intent_lock(root: &Path, rel_path: &str) -> bool {
    read_intent_locks(root).iter().any(|i| i.path == rel_path)
}

/// Record an Absichts-Sperre for `rel_path` (idempotent — re-opening the same offline file never
/// duplicates it). Writes the local, ungeteilte store and ensures `.plm-local/` is git-excluded so
/// the intent can never leak into a shared stand. Best-effort on the exclude line (a missing
/// `.git/info` simply skips it — the store still records the intent).
fn record_intent(root: &Path, rel_path: &str) -> std::io::Result<()> {
    let mut stored: Vec<StoredIntent> = crate::plmstore::read_or_default(&intents_path(root));
    if !stored.iter().any(|s| s.path == rel_path) {
        stored.push(StoredIntent { path: rel_path.to_string() });
    }
    let _ = ensure_excluded(root);
    crate::plmstore::write_pretty(&intents_path(root), &stored)
}

/// Drop the Absichts-Sperren for the given paths (their intents have been confirmed against the
/// server on connect — they are real locks now, no longer just intents). Idempotent; a path not in
/// the store is simply ignored. Leaves any *still-contested* intent untouched.
fn clear_intents(root: &Path, paths: &[String]) -> std::io::Result<()> {
    let mut stored: Vec<StoredIntent> = crate::plmstore::read_or_default(&intents_path(root));
    let before = stored.len();
    stored.retain(|s| !paths.contains(&s.path));
    if stored.len() == before {
        return Ok(()); // nothing to clear
    }
    crate::plmstore::write_pretty(&intents_path(root), &stored)
}

/// Keep `.plm-local/` out of every shared stand by listing it in `.git/info/exclude` — the
/// **local**, ungeteilte ignore list (E38), never the committed `.gitignore` (which would carry the
/// exclusion onto colleagues' clones). Idempotent; a repo with no `.git/info` is skipped silently.
fn ensure_excluded(root: &Path) -> std::io::Result<()> {
    let line = format!("/{LOCAL_DIR}/");
    let exclude = root.join(".git/info/exclude");
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == line) {
        return Ok(());
    }
    match exclude.parent() {
        Some(parent) if parent.is_dir() => {}
        // No `.git/info` (not an ordinary repo) — skip silently; the store still records the intent.
        _ => return Ok(()),
    }
    let sep = if existing.is_empty() || existing.ends_with('\n') { "" } else { "\n" };
    std::fs::write(&exclude, format!("{existing}{sep}{line}\n"))
}

/// Open a lockable binary, recording an **Absichts-Sperre** if the lock server is unreachable
/// (Issue #136, E49b). The offline-aware replacement for a bare [`crate::lockglue::acquire_lock`]:
///
/// - non-lockable text / pre-publish → [`OpenLock::Locked`] (nothing to coordinate);
/// - the real `git lfs lock` succeeds → [`OpenLock::Locked`];
/// - the lock is held by a **colleague** → a loud `Err` (real, present coordination the user sees);
/// - the lock **server is unreachable** (network/auth/keystore/timeout) → record an Absichts-Sperre,
///   make the file writable locally, and return [`OpenLock::OfflineIntent`] so the binary opens with
///   the honest „Sperre nicht bestätigt" card — no false safety.
///
/// The decision *which* failure means „unreachable" vs. „foreign-held" is the pure
/// [`classify_lock_outcome`]; this function only carries it out.
pub fn acquire_lock_or_intent(root: &Path, rel_path: &str) -> std::io::Result<OpenLock> {
    // Non-lockable text (and the pre-publish „all mine" case [`acquire_lock`] handles) have no
    // server coordination to miss — reuse the existing acquire so the lockable set stays single-source.
    if !is_lockable(rel_path) || !crate::setup::is_published(root) {
        crate::lockglue::acquire_lock(root, rel_path)?;
        return Ok(OpenLock::Locked);
    }

    // Published + lockable: take the real `git lfs lock`, but classify a failure ourselves so an
    // unreachable server becomes an Absichts-Sperre rather than a dead end.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["lfs", "lock", rel_path]);
    match crate::gitrunner::output_bounded(&mut cmd) {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            match classify_lock_outcome(out.status.success(), &stderr) {
                LockOutcome::Held => {
                    let _ = set_writable(root, rel_path); // the lock is ours → writable for me
                    Ok(OpenLock::Locked)
                }
                LockOutcome::ForeignHeld => Err(std::io::Error::other(format!(
                    "git lfs lock {rel_path} failed: {}",
                    stderr.trim()
                ))),
                LockOutcome::ServerUnreachable => offline_intent(root, rel_path),
            }
        }
        // A timed-out / spawn-failed call is the server being unreachable, not a foreign lock.
        Err(_) => offline_intent(root, rel_path),
    }
}

/// Carry out the offline branch: record the Absichts-Sperre, make the file writable so the binary
/// opens, and report the unconfirmed intent. A failed write degrades to [`OpenLock::OfflineIntent`]
/// anyway — the file still opens; the worst case is the intent is not remembered for reconcile.
fn offline_intent(root: &Path, rel_path: &str) -> std::io::Result<OpenLock> {
    let _ = record_intent(root, rel_path);
    let _ = set_writable(root, rel_path);
    Ok(OpenLock::OfflineIntent)
}

/// What a `git lfs lock` attempt actually means — the pure classification the offline branch turns
/// on. Total over (success, stderr).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockOutcome {
    /// The lock is ours now (taken, or already held by us — git-lfs's idempotent „already" case).
    Held,
    /// A **colleague** holds the lock — real, present coordination, surfaced loud.
    ForeignHeld,
    /// The **server could not be reached** (network/auth/keystore/timeout) — record an intent.
    ServerUnreachable,
}

/// Classify a `git lfs lock` outcome from its exit + stderr. **Pure, total, case-insensitive** so
/// it is table-testable without a server. The order matters: a success is `Held`; then an „already
/// locked by us" success-shaped message is `Held`; a „locked by <someone>" message is `ForeignHeld`
/// (real coordination); and any **auth/keystore/network** failure is `ServerUnreachable` — that is
/// the E49b case where the binary must still open. Anything left over degrades to `ServerUnreachable`
/// too: we would rather open with an honest unconfirmed intent than wrongly refuse a file the user
/// needs (the reconcile on connect is the safety net that catches a real collision).
fn classify_lock_outcome(success: bool, stderr: &str) -> LockOutcome {
    let s = stderr.to_ascii_lowercase();
    if success || s.contains("already created lock") || s.contains("already locked by you") {
        return LockOutcome::Held;
    }
    // A foreign-held lock: git-lfs reports who holds it. „already locked by you" was caught above, so
    // a remaining „locked by" / „lock already exists" is someone else's, present and reachable.
    if s.contains("locked by") || s.contains("lock already exists") || s.contains("already locked") {
        return LockOutcome::ForeignHeld;
    }
    // Everything else — auth, keystore, timeout, „could not resolve host", refused connections —
    // means the server was not reachable to confirm the lock. Open offline with an intent.
    LockOutcome::ServerUnreachable
}

/// Reconcile the recorded Absichts-Sperren against the **real** server locks **on connect** (Issue
/// #136, E49b) — the Eingang-B side of E49. Reads the local intents and the live `git lfs locks`
/// snapshot, lets the pure [`reconcile_intents`] judge the cross product, and carries out the result:
///
/// - [`IntentReconcile::KeineKollision`] → every offline intent is confirmable (free or already
///   ours): **clear** those intents from the local store (they are real locks now) with no prompt.
/// - [`IntentReconcile::Doppelbearbeitung`] → a double-edit a colleague was holding the whole time:
///   hand the domain-language [`crate::reconciler::Abgleichfrage`] back to the UI (the single loud
///   moment) and **leave the contested intents in place** — nothing is overwritten until the user
///   answers.
///
/// The decision is never made here — only obeyed. Best-effort by construction: an unpublished /
/// offline-again repo simply reports no server locks ([`snapshot`] degrades to empty), so the pure
/// core treats every intent as confirmable — never a false collision.
pub fn reconcile_intents_on_connect(root: &Path) -> std::io::Result<IntentReconcile> {
    let intents = read_intent_locks(root);
    let snap = snapshot(root)?;
    let server: Vec<ServerSperre> = snap
        .locks
        .iter()
        .map(|l| ServerSperre { path: l.path.clone(), owner: l.owner.clone() })
        .collect();
    let me = if snap.me.is_empty() { current_owner_name(root) } else { snap.me.clone() };

    let decision = reconcile_intents(&intents, &server, &me);

    // Quiet case: the confirmable intents are real locks now — drop them from the local store so the
    // card stops warning. A double-edit leaves the contested intents in place (unresolved).
    if let IntentReconcile::KeineKollision { bestaetigt } = &decision {
        // Best-effort: a failed clear must never turn a clean connect into a loud error; the worst
        // case is the next connect re-confirms the same already-confirmed intent, harmlessly.
        let _ = clear_intents(root, bestaetigt);
    }

    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-offlinelockglue-it-{}-{}",
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

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?} failed");
    }

    fn init_repo(root: &Path) {
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.email", "t@example.com"]);
        git(root, &["config", "user.name", "Tester"]);
    }

    /// The lock-outcome classifier table — the pure heart of the offline decision. (success, stderr)
    /// → outcome, covering held, foreign-held and the unreachable cases the E49b open turns on.
    #[test]
    fn classify_lock_outcome_table() {
        let cases: &[(bool, &str, LockOutcome)] = &[
            // success in any shape -> held
            (true, "", LockOutcome::Held),
            (false, "already created lock", LockOutcome::Held),
            (false, "lock already locked by you", LockOutcome::Held),
            // a colleague holds it -> foreign-held (loud, server WAS reachable)
            (false, "lfs: lock already exists", LockOutcome::ForeignHeld),
            (false, "ABC is locked by ben", LockOutcome::ForeignHeld),
            (false, "already locked", LockOutcome::ForeignHeld),
            // server unreachable -> record an intent
            (false, "fatal: Authentication failed for 'https://...'", LockOutcome::ServerUnreachable),
            (false, "PLM-KEYSTORE-UNAVAILABLE", LockOutcome::ServerUnreachable),
            (false, "fatal: unable to access ...: Could not resolve host", LockOutcome::ServerUnreachable),
            (false, "Connection refused", LockOutcome::ServerUnreachable),
            (false, "", LockOutcome::ServerUnreachable),
        ];
        for (success, stderr, expected) in cases {
            assert_eq!(
                classify_lock_outcome(*success, stderr),
                *expected,
                "classify_lock_outcome({success}, {stderr:?})"
            );
        }
    }

    /// Stand the repo up as „published" (an upstream-tracking branch, [`crate::setup::is_published`])
    /// but pointed at an **unreachable** server, so the lock path is exercised and the network read
    /// fails fast: a local bare repo gives the upstream, then the remote URL is bent to a dead host.
    fn published_but_offline(dir: &Path) {
        init_repo(dir);
        let bare = dir.with_extension("remote.git");
        let _ = std::fs::remove_dir_all(&bare);
        git(dir, &["init", "--bare", "--quiet", bare.to_str().unwrap()]);
        std::fs::write(dir.join("seed.txt"), "seed").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "--quiet", "-m", "seed"]);
        git(dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
        // establish the upstream so `is_published` is true …
        git(dir, &["push", "--quiet", "-u", "origin", "HEAD"]);
        // … then bend the remote at a dead host so any networked `git lfs lock` fails fast.
        git(dir, &["remote", "set-url", "origin", "http://127.0.0.1:1/teq/ghost.git"]);
    }

    /// AC (glue): an unreachable lock server still opens the binary — it records an Absichts-Sperre
    /// and reports the unconfirmed intent, and the card flag turns true.
    #[test]
    fn unreachable_server_records_intent_and_opens() {
        let dir = tmp();
        published_but_offline(&dir);
        assert!(crate::setup::is_published(&dir), "the test repo must read as published");
        std::fs::create_dir_all(dir.join("elektronik")).unwrap();
        std::fs::write(dir.join("elektronik/board.kicad_pcb"), b"(pcb)").unwrap();

        let out = acquire_lock_or_intent(&dir, "elektronik/board.kicad_pcb").unwrap();
        assert_eq!(out, OpenLock::OfflineIntent, "an unreachable lock opens with an intent");
        assert!(out.is_offline_intent());
        assert!(
            has_intent_lock(&dir, "elektronik/board.kicad_pcb"),
            "the Absichts-Sperre was recorded in .plm-local/"
        );
        // recorded under .plm-local/, the local ungeteilte store — never the committed _plm/
        assert!(dir.join(".plm-local").join(INTENTS_FILE).is_file());
        // and .plm-local/ was excluded locally so the intent can never reach a shared stand (E38)
        let exclude = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap_or_default();
        assert!(exclude.lines().any(|l| l.trim() == "/.plm-local/"), "excluded via .git/info/exclude");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(dir.with_extension("remote.git"));
    }

    /// AC (glue): a non-lockable text file never records an intent — there is nothing to coordinate,
    /// so the open is a plain [`OpenLock::Locked`] even with an unreachable remote.
    #[test]
    fn text_file_never_records_an_intent() {
        let dir = tmp();
        init_repo(&dir);
        git(&dir, &["remote", "add", "origin", "http://127.0.0.1:1/teq/ghost.git"]);
        std::fs::create_dir_all(dir.join("firmware")).unwrap();
        std::fs::write(dir.join("firmware/main.c"), b"int main(){}").unwrap();

        let out = acquire_lock_or_intent(&dir, "firmware/main.c").unwrap();
        assert_eq!(out, OpenLock::Locked, "mergeable text has nothing to coordinate");
        assert!(!has_intent_lock(&dir, "firmware/main.c"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC (glue): recording is idempotent — re-opening the same offline file does not duplicate the
    /// Absichts-Sperre.
    #[test]
    fn recording_an_intent_is_idempotent() {
        let dir = tmp();
        init_repo(&dir);
        record_intent(&dir, "a.f3d").unwrap();
        record_intent(&dir, "a.f3d").unwrap();
        let intents = read_intent_locks(&dir);
        assert_eq!(intents.len(), 1, "the same intent is recorded once");
        assert_eq!(intents[0].path, "a.f3d");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC (glue): on a clean reconnect (no real server locks, unpublished) every recorded intent is
    /// confirmable and is CLEARED from the local store — the card stops warning, no prompt.
    #[test]
    fn connect_clears_confirmable_intents_silently() {
        let dir = tmp();
        init_repo(&dir);
        record_intent(&dir, "a.f3d").unwrap();
        record_intent(&dir, "b.step").unwrap();

        let d = reconcile_intents_on_connect(&dir).unwrap();
        assert!(d.is_silent(), "a clean reconnect raises no question: {d:?}");
        assert!(!has_intent_lock(&dir, "a.f3d"), "confirmed intents are cleared");
        assert!(!has_intent_lock(&dir, "b.step"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The intent store degrades like every ADR-0002 store: missing/corrupt ⇒ empty, never an error.
    #[test]
    fn intent_store_degrades_to_empty() {
        let dir = tmp();
        assert!(read_intent_locks(&dir).is_empty(), "missing -> empty");
        std::fs::create_dir_all(dir.join(LOCAL_DIR)).unwrap();
        std::fs::write(intents_path(&dir), "{ not json ]").unwrap();
        assert!(read_intent_locks(&dir).is_empty(), "corrupt -> empty");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
