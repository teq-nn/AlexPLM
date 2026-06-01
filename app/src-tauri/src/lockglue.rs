//! Side-effecting git glue for the Status Reader (Issue #6).
//!
//! Everything in [`crate::locks`] is pure; this module is the thin, isolated layer that
//! actually talks to git: it reads `git lfs locks --json` and `git status --porcelain`, folds
//! them into a [`StatusSnapshot`], and acquires a `git lfs lock` when a lockable artifact is
//! opened/edited. Per E37 there is exactly **one** source of truth — git itself — so this layer
//! only ever *reads it back*; it never caches or mirrors lock state.
//!
//! The JSON/porcelain parsing is kept pure and table-testable ([`parse_locks_json`],
//! [`parse_porcelain_paths`]); only [`snapshot`] and [`acquire_lock`] shell out.

use crate::locks::{is_lockable, LockInfo, StatusSnapshot};
use serde::Deserialize;
use std::io::{Error, ErrorKind};
use std::path::Path;

/// One held-by-me lock that an auto-unlock sweep examined, with the action the Warden decided and
/// whether the lock was actually released. Surfaced so the caller (and a test) can see exactly
/// which clean paths were freed at a checkpoint without re-deriving the decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweptLock {
    /// Product-relative path the lock was on.
    pub path: String,
    /// True iff the path was locally clean and the lock was released by this sweep.
    pub released: bool,
}

/// Shape of one entry in `git lfs locks --json`. We only read the fields we render.
#[derive(Debug, Deserialize)]
struct RawLock {
    path: String,
    #[serde(default)]
    owner: RawOwner,
    #[serde(default)]
    locked_at: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawOwner {
    #[serde(default)]
    name: String,
}

/// Parse the `git lfs locks --json` array into our [`LockInfo`] rows. Pure and total: malformed
/// or empty output yields an empty list rather than an error (the panel just shows nothing).
pub fn parse_locks_json(json: &str) -> Vec<LockInfo> {
    serde_json::from_str::<Vec<RawLock>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|r| LockInfo {
            path: r.path,
            owner: r.owner.name,
            locked_at: r.locked_at,
        })
        .collect()
}

/// Parse `git status --porcelain` output into the set of changed, product-relative,
/// forward-slash paths. Pure and total. Handles the `XY <path>` form and the `XY old -> new`
/// rename form (we take the new path). Quoted paths keep their literal bytes minus the quotes.
pub fn parse_porcelain_paths(porcelain: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        // Columns 0..2 are the status code, then a space, then the path.
        let rest = line[3..].trim_end();
        let path = match rest.split_once(" -> ") {
            Some((_, new)) => new, // rename/copy: the new path is what's on disk
            None => rest,
        };
        let path = path.trim_matches('"');
        if !path.is_empty() {
            out.push(path.replace('\\', "/"));
        }
    }
    out
}

/// Build the full [`StatusSnapshot`] for `root` by reading git: all LFS locks, the current
/// lfs/user identity, and the dirty worktree paths. Side-effecting (the only read of git here).
pub fn snapshot(root: &Path) -> std::io::Result<StatusSnapshot> {
    let locks = read_locks(root)?;
    let me = current_owner_name(root);
    let dirty = read_dirty_paths(root)?;
    Ok(StatusSnapshot { locks, me, dirty })
}

/// Read & parse `git lfs locks --json`. This call reaches the LFS endpoint over the network, so it
/// is **bounded** ([`gitrunner::output_bounded`]): a rejected credential sends git-lfs into an
/// unbounded 401-retry loop, and this read is fired by the 4-second status loop — left unbounded,
/// the hung children pile up until the app dies. A timeout (or any LFS error — e.g. no remote
/// configured yet, or a 401) is treated as "no locks" rather than a hard failure, so the read-only
/// shell still renders and the daily rhythm stays quiet.
fn read_locks(root: &Path) -> std::io::Result<Vec<LockInfo>> {
    // No server-side repo before publishing → no remote locks can exist, and `git lfs locks` would
    // only loop on the absent repo's 401 (Forgejo's LFS endpoint authenticates before checking
    // existence), wedging this bounded call for its full timeout on every 4-second status tick.
    // Skip the networked read entirely and report no locks until the product is published.
    if !crate::setup::is_published(root) {
        return Ok(Vec::new());
    }
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["lfs", "locks", "--json"]);
    match crate::gitrunner::output_bounded(&mut cmd) {
        Ok(out) if out.status.success() => {
            Ok(parse_locks_json(&String::from_utf8_lossy(&out.stdout)))
        }
        _ => Ok(Vec::new()),
    }
}

/// Read the dirty paths from `git status --porcelain`.
fn read_dirty_paths(root: &Path) -> std::io::Result<Vec<String>> {
    let out = crate::gitrunner::command(root)
        .args(["status", "--porcelain"])
        .output()?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    Ok(parse_porcelain_paths(&String::from_utf8_lossy(&out.stdout)))
}

/// The identity git-lfs records as a lock's `owner.name` for **us** — the Forgejo/Gitea **account
/// name**, not the local `git config user.name` (Issue #72). Forgejo stamps the server account
/// (= the repo-owner slug in the remote URL, e.g. `…/niklasonfire/woody.git` → `niklasonfire`)
/// onto every lock, while `git config user.name` is the *commit author* (e.g. "Niklas"). Comparing
/// own locks against the commit author made every own lock read as `HeldByOther`, so the Warden
/// refused every auto-unlock and Freigabe. We decouple lock identity (server account) from commit
/// author here: derive the account from the remote URL, and only fall back to `git config
/// user.name` when no remote is configured yet (a fresh, unpublished repo has no locks anyway).
///
/// Public so the push glue ([`crate::pushglue`]) splits own vs. foreign locks by the **same**
/// identity — the lock-ownership rule lives in exactly one place.
pub fn current_owner_name(root: &Path) -> String {
    crate::setup::remote_get_url(root)
        .and_then(|url| crate::forgejo::forgejo_account_from_remote_url(&url))
        .unwrap_or_else(|| git_user_name(root))
}

/// The local `git config user.name`, trimmed, or an empty string. The fallback lock identity when
/// no remote is configured (an unpublished repo, which has no server-side locks to match anyway).
fn git_user_name(root: &Path) -> String {
    crate::gitrunner::command(root)
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Whether a lock's `owner.name` is **us**. Case-insensitive (Forgejo account names are
/// case-insensitive) and trimmed, so a casing/whitespace difference never makes an own lock read
/// as foreign. An empty identity (no remote, no user.name) matches nothing.
pub fn owner_is_me(owner: &str, me: &str) -> bool {
    let me = me.trim();
    !me.is_empty() && owner.trim().eq_ignore_ascii_case(me)
}

/// Auto-acquire a `git lfs lock` for a lockable artifact being opened/edited (E31).
///
/// Non-lockable (mergeable text) paths are a no-op: nothing to coordinate, returns `Ok(false)`.
/// A lockable path is locked and returns `Ok(true)`. A lock already held (by us) is not an error
/// — git-lfs is idempotent enough here and we treat an "already locked by you" outcome as
/// success so re-opening a file never surfaces a scary error. Foreign-held locks *do* surface
/// the git-lfs error, since that is real, loud coordination the user must see.
pub fn acquire_lock(root: &Path, rel_path: &str) -> std::io::Result<bool> {
    if !is_lockable(rel_path) {
        return Ok(false);
    }
    // Pre-publish there is no server to register the lock with — `git lfs lock` would only loop on
    // the absent repo's 401. Make the file writable locally (the on-disk side of „editing = mine",
    // Issue #42) and report success; real lock coordination begins once the product is published.
    if !crate::setup::is_published(root) {
        let _ = set_writable(root, rel_path);
        return Ok(true);
    }
    // Reaches the LFS endpoint — bound it so a rejected credential fails fast instead of hanging.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["lfs", "lock", rel_path]);
    let out = crate::gitrunner::output_bounded(&mut cmd)?;
    let acquired = out.status.success() || {
        // "already created lock" / "already locked" by us is fine — the file is locked, job done.
        String::from_utf8_lossy(&out.stderr).contains("already")
    };
    if acquired {
        // The lock is ours now → the file becomes writable *for me* (read-only = free, Issue #42).
        // Best-effort: a permission flip failing must never turn a successful lock into an error.
        let _ = set_writable(root, rel_path);
        return Ok(true);
    }
    Err(Error::new(
        ErrorKind::Other,
        format!(
            "git lfs lock {rel_path} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    ))
}

/// Make a lockable artifact **writable for me** — the on-disk side of taking the lock (Issue #42:
/// "the act of editing/locking makes the file writable for me"). No-op for a non-lockable path or
/// a missing file; best-effort, never fails loudly (the lock, not the bit, is the source of truth).
pub fn set_writable(root: &Path, rel_path: &str) -> std::io::Result<()> {
    set_read_only_bit(root, rel_path, false)
}

/// Make a lockable artifact **read-only** — the resting state of a free binary (Issue #42:
/// "read-only = free"). No-op for a non-lockable path or a missing file.
pub fn set_read_only(root: &Path, rel_path: &str) -> std::io::Result<()> {
    set_read_only_bit(root, rel_path, true)
}

/// Flip the read-only bit on a lockable artifact. Pure plumbing over `std::fs` permissions; a
/// non-lockable path or a path that does not exist on disk is silently ignored so the caller can
/// fire this for every save without guarding.
fn set_read_only_bit(root: &Path, rel_path: &str, read_only: bool) -> std::io::Result<()> {
    if !is_lockable(rel_path) {
        return Ok(());
    }
    let abs = root.join(rel_path);
    let meta = match std::fs::metadata(&abs) {
        Ok(m) => m,
        Err(_) => return Ok(()), // not on disk (e.g. a deletion) — nothing to flip
    };
    let mut perms = meta.permissions();
    if perms.readonly() == read_only {
        return Ok(()); // already in the wanted state
    }
    perms.set_readonly(read_only);
    std::fs::set_permissions(&abs, perms)
}

/// At a checkpoint, **auto-unlock every held-by-me lock whose path is locally clean** (E31/E35
/// self-healing, Issue #42). Reuses the pure [`crate::warden::decide`] for each held lock — the
/// lock policy is decided in exactly one place, never duplicated here. For each of *our* locks
/// this reads whether the path is dirty, asks the Warden, and (when the Warden says
/// [`crate::warden::WardenAction::AutoUnlock`]) releases the lock and lets the freed binary rest
/// read-only again. A clean text lock is released too (text is never really locked, but the
/// Warden's rule is uniform). Foreign locks are never touched.
pub fn auto_unlock_clean_paths(root: &Path) -> std::io::Result<Vec<SweptLock>> {
    use crate::warden::{decide, Checkpoint, Cleanliness, LockState, PathKind, WardenAction, WardenSnapshot};

    let snap = self::snapshot(root)?;
    let me = &snap.me;
    let mut swept = Vec::new();

    for lock in snap.locks.iter().filter(|l| owner_is_me(&l.owner, me)) {
        let path = &lock.path;
        let dirty = snap.dirty.iter().any(|d| d == path);
        let warden_snap = WardenSnapshot {
            kind: if is_lockable(path) { PathKind::Binary } else { PathKind::Text },
            lock: LockState::HeldByMe,
            clean: if dirty { Cleanliness::Dirty } else { Cleanliness::Clean },
            // Auto-unlock is checkpoint-kind-independent (E35: "at EVERY checkpoint"); a laufender
            // Checkpoint is the conservative choice and never yields a Freigabe for a clean path.
            checkpoint: Checkpoint::Laufend,
        };
        let release = decide(warden_snap) == WardenAction::AutoUnlock;
        if release {
            crate::pushglue::auto_unlock(root, path)?;
            // The lock is gone → the binary rests read-only again (read-only = free, Issue #42).
            let _ = set_read_only(root, path);
        }
        swept.push(SweptLock {
            path: path.clone(),
            released: release,
        });
    }
    Ok(swept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_is_me_is_case_insensitive_and_trimmed() {
        // The Forgejo account stamped on a lock matches "me" regardless of casing/whitespace, so an
        // own lock never reads as foreign over a trivial difference (Issue #72).
        assert!(owner_is_me("niklasonfire", "niklasonfire"));
        assert!(owner_is_me("NiklasOnFire", "niklasonfire"));
        assert!(owner_is_me("  niklasonfire  ", "niklasonfire"));
        // A genuinely different owner (incl. the old commit-author name) is foreign.
        assert!(!owner_is_me("anna", "niklasonfire"));
        assert!(!owner_is_me("Niklas", "niklasonfire"));
        // An empty identity (no remote, no user.name) matches nothing rather than everything.
        assert!(!owner_is_me("anyone", ""));
        assert!(!owner_is_me("", ""));
    }

    #[test]
    fn parse_locks_json_reads_path_owner_and_time() {
        let json = r#"[
            {"id":"1","path":"mechanik/gehaeuse/gehaeuse.f3d","owner":{"name":"bjoern"},"locked_at":"2026-05-30T09:15:00Z"},
            {"id":"2","path":"elektronik/board.kicad_pcb","owner":{"name":"anna"},"locked_at":"2026-05-30T08:00:00Z"}
        ]"#;
        let locks = parse_locks_json(json);
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].path, "mechanik/gehaeuse/gehaeuse.f3d");
        assert_eq!(locks[0].owner, "bjoern");
        assert_eq!(locks[0].locked_at, "2026-05-30T09:15:00Z");
        assert_eq!(locks[1].owner, "anna");
    }

    #[test]
    fn parse_locks_json_is_total_on_empty_or_garbage() {
        assert!(parse_locks_json("[]").is_empty());
        assert!(parse_locks_json("").is_empty());
        assert!(parse_locks_json("not json").is_empty());
        // missing owner.name defaults to empty rather than panicking
        let locks = parse_locks_json(r#"[{"path":"a.f3d","locked_at":"t"}]"#);
        assert_eq!(locks.len(), 1);
        assert_eq!(locks[0].owner, "");
    }

    #[test]
    fn parse_porcelain_extracts_changed_paths() {
        let porcelain = " M mechanik/gehaeuse/gehaeuse.f3d\n?? neu.txt\nA  elektronik/board.kicad_pcb\n";
        let paths = parse_porcelain_paths(porcelain);
        assert_eq!(
            paths,
            [
                "mechanik/gehaeuse/gehaeuse.f3d",
                "neu.txt",
                "elektronik/board.kicad_pcb"
            ]
        );
    }

    #[test]
    fn parse_porcelain_takes_new_path_of_a_rename() {
        let porcelain = "R  alt/name.f3d -> neu/name.f3d\n";
        let paths = parse_porcelain_paths(porcelain);
        assert_eq!(paths, ["neu/name.f3d"]);
    }

    #[test]
    fn parse_porcelain_is_total_on_blank() {
        assert!(parse_porcelain_paths("").is_empty());
        assert!(parse_porcelain_paths("\n\n").is_empty());
    }

    #[test]
    fn set_read_only_flips_lockable_files_and_ignores_text_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("mechanik/gehaeuse")).unwrap();
        let bin = "mechanik/gehaeuse/gehaeuse.f3d";
        let txt = "firmware/main.c";
        std::fs::write(root.join(bin), b"v1").unwrap();
        std::fs::create_dir_all(root.join("firmware")).unwrap();
        std::fs::write(root.join(txt), b"int main(){}").unwrap();

        // A lockable binary toggles read-only / writable.
        set_read_only(root, bin).unwrap();
        assert!(std::fs::metadata(root.join(bin)).unwrap().permissions().readonly());
        set_writable(root, bin).unwrap();
        assert!(!std::fs::metadata(root.join(bin)).unwrap().permissions().readonly());

        // Mergeable text is never made read-only by us.
        set_read_only(root, txt).unwrap();
        assert!(!std::fs::metadata(root.join(txt)).unwrap().permissions().readonly());

        // A missing path is a silent no-op (e.g. a deletion).
        set_read_only(root, "mechanik/gone.f3d").unwrap();
        set_writable(root, "mechanik/gone.f3d").unwrap();
    }
}
