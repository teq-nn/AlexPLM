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

/// The current user's name as git-lfs would record it on a lock — `git config user.name`,
/// falling back to an empty string. Used purely to split own vs. foreign locks.
fn current_owner_name(root: &Path) -> String {
    crate::gitrunner::command(root)
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
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
    // Reaches the LFS endpoint — bound it so a rejected credential fails fast instead of hanging.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["lfs", "lock", rel_path]);
    let out = crate::gitrunner::output_bounded(&mut cmd)?;
    if out.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    // "already created lock" / "already locked" by us is fine — the file is locked, job done.
    if stderr.contains("already") {
        return Ok(true);
    }
    Err(Error::new(
        ErrorKind::Other,
        format!("git lfs lock {rel_path} failed: {}", stderr.trim()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
