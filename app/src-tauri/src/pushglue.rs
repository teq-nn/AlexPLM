//! Side-effecting git/LFS glue for the two push types (Issue #9).
//!
//! Everything in [`crate::warden`] is a pure decision; this module is the thin, isolated layer
//! that (a) reads git to assemble a [`WardenSnapshot`] for a path, and (b) **carries out** the
//! single [`WardenAction`] the Warden returns. The Warden never touches git; this glue never
//! decides — it only obeys, so the safety-critical invariant lives entirely in the testable core.
//!
//! Mirrors the house pattern (`lockglue.rs`, `setup.rs`): the snapshot assembly and the carry-out
//! are kept small, the personal-backup ref-name derivation is a pure helper
//! ([`personal_backup_ref`], [`personal_backup_refspec`]) so the exact ref string is asserted by a
//! unit test rather than discovered in production, and tests stand a **bare local repo** in for
//! the remote — they never touch a real Forgejo/Gitea server or a real LFS endpoint.
//!
//! The two carry-outs realize the glossary's two push types:
//! - **Sicherungs-Push** — push the current branch into a *personal* namespace
//!   `refs/personal/<user>/<branch>` on the remote. A private backup that does **not** move the
//!   shared `main`. Does not unlock anything.
//! - **Freigabe-Push** — push the current branch to the shared `main` AND, for a held path,
//!   release the lock with an explicit `git lfs unlock` ("unlock at push", since git-lfs does not
//!   release on push by itself, E35). The two steps are sequenced so the lock is released only
//!   once the publish has succeeded — a binary's content reaches the shared stand / LFS store
//!   exactly here, at the moment the lock ends (E36).

use crate::locks::is_lockable;
use crate::setup::REMOTE_NAME;
use crate::warden::{
    decide, Checkpoint, Cleanliness, LockState, PathKind, WardenAction, WardenSnapshot,
};
use std::io::Error;
use std::path::Path;

/// The shared release branch the Freigabe-Push publishes to (E35: „geteilter `main`-Stand").
pub const SHARED_BRANCH: &str = "main";

// ----------------------------------------------------------------------------------------------
// Pure helper — the personal backup ref the Sicherungs-Push targets
// ----------------------------------------------------------------------------------------------

/// The personal-namespace ref a Sicherungs-Push backs up into: `refs/personal/<user>/<branch>`.
/// Pure over the user + branch so the exact ref string is unit-tested. A backup ref under
/// `refs/personal/<user>/…` is, by construction, **not** the shared `refs/heads/main`, so it can
/// never publish a release (E35). User/branch are sanitized to safe ref characters.
pub fn personal_backup_ref(user: &str, branch: &str) -> String {
    format!(
        "refs/personal/{}/{}",
        sanitize_ref_component(user),
        sanitize_ref_component(branch)
    )
}

/// The full refspec for the Sicherungs-Push: `<branch>:refs/personal/<user>/<branch>` — push the
/// local branch up into the personal namespace, leaving `refs/heads/*` (the shared stand) alone.
pub fn personal_backup_refspec(user: &str, branch: &str) -> String {
    format!("{}:{}", branch, personal_backup_ref(user, branch))
}

/// Replace anything that is not a safe git ref character with `-`, and avoid empty components.
fn sanitize_ref_component(s: &str) -> String {
    let cleaned: String = s
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' })
        .collect();
    if cleaned.is_empty() { "unknown".to_string() } else { cleaned }
}

// ----------------------------------------------------------------------------------------------
// Snapshot assembly — read git, build the pure WardenSnapshot for one path
// ----------------------------------------------------------------------------------------------

/// Assemble the Warden's input for `rel_path` at the given checkpoint by reading git: the path's
/// kind (classifier), its lock state (`git lfs locks`), and whether it is locally clean
/// (`git status`). Side-effecting (reads git); the decision itself stays pure.
pub fn snapshot_for(
    root: &Path,
    rel_path: &str,
    checkpoint: Checkpoint,
) -> std::io::Result<WardenSnapshot> {
    let kind = if is_lockable(rel_path) { PathKind::Binary } else { PathKind::Text };
    let lock = read_lock_state(root, rel_path);
    let clean = if is_path_dirty(root, rel_path)? { Cleanliness::Dirty } else { Cleanliness::Clean };
    Ok(WardenSnapshot { kind, lock, clean, checkpoint })
}

/// Read the lock state of one path from `git lfs locks --json`, splitting own vs. foreign by
/// `git config user.name`. Best-effort: an LFS error (no remote, etc.) reads back as Unlocked.
fn read_lock_state(root: &Path, rel_path: &str) -> LockState {
    let me = git_stdout(root, &["config", "user.name"]).unwrap_or_default();
    // git lfs locks hits the network — bound it so a rejected credential can't hang the checkpoint.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["lfs", "locks", "--json"]);
    let out = crate::gitrunner::output_bounded(&mut cmd);
    let json = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return LockState::Unlocked,
    };
    let locks = crate::lockglue::parse_locks_json(&json);
    match locks.iter().find(|l| l.path == rel_path) {
        Some(l) if l.owner == me => LockState::HeldByMe,
        Some(_) => LockState::HeldByOther,
        None => LockState::Unlocked,
    }
}

/// Whether `rel_path` has open local work — uncommitted changes in the worktree/index.
fn is_path_dirty(root: &Path, rel_path: &str) -> std::io::Result<bool> {
    let out = crate::gitrunner::command(root)
        .args(["status", "--porcelain", "--", rel_path])
        .output()?;
    if !out.status.success() {
        return Ok(false);
    }
    let dirty = crate::lockglue::parse_porcelain_paths(&String::from_utf8_lossy(&out.stdout));
    Ok(dirty.iter().any(|p| p == rel_path))
}

// ----------------------------------------------------------------------------------------------
// The one entry point: decide, then carry out, the single action for a path at a checkpoint
// ----------------------------------------------------------------------------------------------

/// Read the snapshot for `rel_path` at `checkpoint`, let the pure [`decide`] choose the single
/// action, and carry it out against git. Returns the action taken so the caller (and the UI) can
/// report it in the tool's vocabulary. The decision is never made here — only obeyed.
pub fn run_checkpoint(
    root: &Path,
    rel_path: &str,
    checkpoint: Checkpoint,
) -> std::io::Result<WardenAction> {
    let snap = snapshot_for(root, rel_path, checkpoint)?;
    let action = decide(snap);
    carry_out(root, rel_path, action)?;
    Ok(action)
}

/// Carry out one already-decided [`WardenAction`] against git/LFS. Pure dispatch over the action;
/// the safety reasoning happened in [`decide`].
pub fn carry_out(root: &Path, rel_path: &str, action: WardenAction) -> std::io::Result<()> {
    match action {
        WardenAction::SicherungsPush => sicherungs_push(root),
        WardenAction::FreigabePush => freigabe_push(root, rel_path),
        WardenAction::AutoUnlock => auto_unlock(root, rel_path),
        WardenAction::Refuse => Ok(()),
    }
}

/// **Sicherungs-Push** — mirror the current branch into the personal backup namespace on the
/// remote. Private backup: it pushes `refs/heads/<branch>` up to `refs/personal/<user>/<branch>`,
/// so it can never touch the shared `main`. Does not unlock.
pub fn sicherungs_push(root: &Path) -> std::io::Result<()> {
    let branch = current_branch(root)?;
    let user = git_stdout(root, &["config", "user.name"]).unwrap_or_default();
    let refspec = personal_backup_refspec(&user, &branch);
    run_git(root, &["push", REMOTE_NAME, &refspec])
}

/// **Freigabe-Push** — publish the finished work to the shared `main` AND release the lock
/// atomically ("unlock at push", E35). The publish runs first; only on success is the lock
/// dropped, so a failed publish never leaves the path released-but-unpublished. A non-lockable
/// (text) path simply has no lock to drop.
pub fn freigabe_push(root: &Path, rel_path: &str) -> std::io::Result<()> {
    let branch = current_branch(root)?;
    // Publish to the shared branch (the public act). Binary content reaches the LFS store here.
    run_git(root, &["push", REMOTE_NAME, &format!("{branch}:{SHARED_BRANCH}")])?;
    // "Unlock at push": git-lfs does not release on push by itself, so the tool does it — only
    // after the publish succeeded. Text paths hold no lock; skip them.
    if is_lockable(rel_path) {
        unlock(root, rel_path)?;
    }
    Ok(())
}

/// **Auto-Unlock** — release a held-by-me lock on a clean path (E31/E35 self-healing).
pub fn auto_unlock(root: &Path, rel_path: &str) -> std::io::Result<()> {
    unlock(root, rel_path)
}

/// Release one `git lfs lock`. Treats an already-unlocked path as success (idempotent), so a
/// double checkpoint never surfaces a scary error.
fn unlock(root: &Path, rel_path: &str) -> std::io::Result<()> {
    // Reaches the LFS endpoint — bound it so a rejected credential fails fast instead of hanging.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(["lfs", "unlock", rel_path]);
    let out = crate::gitrunner::output_bounded(&mut cmd)?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("no lock") || stderr.contains("not locked") || stderr.contains("unable to find") {
        return Ok(());
    }
    Err(Error::other(format!(
        "git lfs unlock {rel_path} failed: {}",
        stderr.trim()
    )))
}

// ----------------------------------------------------------------------------------------------
// Small git helpers (kept local so this isolated glue is self-contained)
// ----------------------------------------------------------------------------------------------

/// The current branch name; falls back to [`SHARED_BRANCH`] for a fresh repo on no branch.
fn current_branch(root: &Path) -> std::io::Result<String> {
    let name = git_stdout(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    Ok(if !name.is_empty() && name != "HEAD" { name } else { SHARED_BRANCH.to_string() })
}

/// Run a git subcommand in `root`, mapping a non-zero exit to an `io::Error`.
fn run_git(root: &Path, args: &[&str]) -> std::io::Result<()> {
    // Bounded: the two push types reach the network (and trigger git-lfs transfer), where a
    // rejected credential would otherwise loop forever.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(args);
    let out = crate::gitrunner::output_bounded(&mut cmd)?;
    if !out.status.success() {
        return Err(Error::other(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Trimmed stdout of a successful git subcommand, or `None` on failure.
fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let out = crate::gitrunner::command(root).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_backup_ref_is_under_personal_namespace_not_heads() {
        let r = personal_backup_ref("anna", "main");
        assert_eq!(r, "refs/personal/anna/main");
        // crucially: it is NOT the shared refs/heads/main — it can never be a release.
        assert!(!r.starts_with("refs/heads/"));
        assert!(r.starts_with("refs/personal/"));
    }

    #[test]
    fn personal_backup_refspec_pushes_branch_into_personal_namespace() {
        assert_eq!(
            personal_backup_refspec("anna", "main"),
            "main:refs/personal/anna/main"
        );
    }

    #[test]
    fn ref_components_are_sanitized() {
        // spaces / slashes / odd chars in the user name never escape the personal namespace.
        let r = personal_backup_ref("a n/na", "feature/x");
        assert_eq!(r, "refs/personal/a-n-na/feature-x");
        assert!(r.starts_with("refs/personal/"));
        // an empty user falls back rather than producing refs/personal//main
        assert_eq!(personal_backup_ref("   ", "main"), "refs/personal/unknown/main");
    }
}
