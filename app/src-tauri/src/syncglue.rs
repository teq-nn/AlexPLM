//! Side-effecting git glue for the **stiller Sync** (Issue #11, E41).
//!
//! Everything in [`crate::syncdecider`] is a pure decision; this module is the thin, isolated
//! layer that (a) reads git to find how the local and remote stands have **diverged** and
//! classifies each diverged path into its mergeability [`Bucket`], and (b) **carries out** the
//! decision the Sync Decider returns: a [`SyncDecision::SilentMerge`] runs git's merge silently,
//! a [`SyncDecision::LoudException`] stops and hands the domain-language question back to the UI.
//!
//! Mirrors the house pattern (`pushglue.rs`, `lockglue.rs`, `setup.rs`): the decision never lives
//! here — it lives in the testable core; this glue only reads git and obeys. The two push types
//! of the daily rhythm (Sicherungs-Push laufend, Freigabe-Push at the Meilenstein) are the **#9
//! Warden's** job ([`crate::pushglue`]); this module owns only the **pull** side and the
//! divergence judgement. The daily vocabulary is "aktuell / X arbeitet an Y / gesichert" — never
//! push/pull/merge (E41).
//!
//! Tests stand a **bare local repo** in for the self-hosted Forgejo/Gitea remote (`file://…`);
//! they never touch a real server or LFS endpoint. The pure routing/marker guarantees are proven
//! exhaustively in [`crate::syncdecider`]; this layer is proven to wire up against git.

use crate::locks::bucket_of;
use crate::setup::REMOTE_NAME;
use crate::syncdecider::{decide_sync, DivergedPath, SyncDecision};
use serde::Serialize;
use std::io::Error;
use std::path::Path;

/// The shared release branch the daily sync tracks (E34: both colleagues on `main`).
pub const SHARED_BRANCH: &str = "main";

/// The quiet daily sync state shown to the user, in the tool's OWN vocabulary (E41). Never
/// push/pull/merge. This is what the calm status readout reflects after a silent sync; a loud
/// exception is carried separately as the [`SyncDecision`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncStatus {
    /// Local and remote agree — "aktuell". Nothing to do, nothing shown.
    Aktuell,
    /// A silent merge brought the colleague's free changes in — still "aktuell", quietly.
    Gesichert,
    /// The stiller Sync stopped on a real contradiction (E41): the single loud exception. Carries
    /// the domain-language question the UI raises in its one orange-frame moment.
    LauteAusnahme(crate::syncdecider::LoudQuestion),
}

/// The outcome of one silent daily sync pass, ready for the UI in the tool's vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncOutcome {
    pub status: SyncStatus,
}

// ----------------------------------------------------------------------------------------------
// Divergence assembly — read git, build the pure DivergedPath list for the Sync Decider
// ----------------------------------------------------------------------------------------------

/// Find the paths where the local branch and the fetched remote stand have diverged, each
/// classified into its mergeability [`Bucket`]. Side-effecting (reads git via
/// `git diff --name-only HEAD <remote>`); the decision itself stays pure.
///
/// `other` is the colleague's name to phrase a possible loud question with (from `git config` or
/// the lock owner upstream); `None` falls back to a neutral domain phrase.
pub fn diverged_paths(root: &Path, other: Option<String>) -> std::io::Result<Vec<DivergedPath>> {
    let remote_ref = format!("{REMOTE_NAME}/{SHARED_BRANCH}");
    // Paths that differ between our HEAD and the fetched remote tip. If the remote ref is unknown
    // (never published / offline), there is nothing to diverge against → empty.
    let out = crate::gitrunner::command(root)
        .args(["diff", "--name-only", "HEAD", &remote_ref])
        .output()?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let names = String::from_utf8_lossy(&out.stdout);
    Ok(parse_diff_names(&names)
        .into_iter()
        .map(|path| {
            let bucket = bucket_of(&path);
            DivergedPath { path, bucket, other: other.clone() }
        })
        .collect())
}

/// Parse `git diff --name-only` output into a clean, de-duplicated path list. Pure + table-tested.
pub fn parse_diff_names(out: &str) -> Vec<String> {
    let mut seen = Vec::new();
    for line in out.lines() {
        let p = line.trim();
        if !p.is_empty() && !seen.iter().any(|x| x == p) {
            seen.push(p.to_string());
        }
    }
    seen
}

// ----------------------------------------------------------------------------------------------
// The one entry point: pull, decide, then carry out the silent merge or stop loudly
// ----------------------------------------------------------------------------------------------

/// Run one **silent daily sync pass** (E41): fetch the remote stand, ask the pure Sync Decider
/// whether the divergence is free (mergeable) or contradictory, and carry out the result:
///
/// - no divergence → [`SyncStatus::Aktuell`], nothing shown;
/// - free, mergeable divergence → run git's merge silently → [`SyncStatus::Gesichert`], no prompt;
/// - any unmergeable touch → **do NOT merge** (E41: a merge must never silently corrupt a file);
///   return [`SyncStatus::LauteAusnahme`] with the domain-language question for the UI.
///
/// The decision is never made here — only obeyed. Returns the outcome in the tool's vocabulary.
pub fn run_sync(root: &Path, other: Option<String>) -> std::io::Result<SyncOutcome> {
    // Pull the remote stand into our knowledge WITHOUT merging yet (a fetch never touches the
    // worktree, so it can never corrupt a file). The merge decision comes from the pure core.
    let _ = fetch(root); // best-effort: offline / unpublished simply yields no divergence

    let diverged = diverged_paths(root, other)?;
    let status = match decide_sync(&diverged) {
        // Free divergence (or none): bring the colleague's mergeable changes in silently.
        SyncDecision::SilentMerge => {
            if diverged.is_empty() {
                SyncStatus::Aktuell
            } else {
                silent_merge(root)?;
                SyncStatus::Gesichert
            }
        }
        // A real contradiction over an unmergeable file: STOP. Never merge; raise the loud
        // exception in domain language. The worktree is left untouched (no corrupting merge ran).
        SyncDecision::LoudException(q) => SyncStatus::LauteAusnahme(q),
    };
    Ok(SyncOutcome { status })
}

/// Fetch the remote stand into the remote-tracking ref. Reads only; never touches the worktree,
/// so it can never corrupt a file. Best-effort: an offline/unpublished repo is not an error here.
fn fetch(root: &Path) -> std::io::Result<()> {
    run_git(root, &["fetch", REMOTE_NAME, SHARED_BRANCH])
}

/// Carry out the **silent merge** of the fetched remote stand into the local branch. Only reached
/// when the Sync Decider has proven every diverged path is free, mergeable text — so git's merge
/// touches only mergeable files and can produce no conflict marker. No user prompt (E41).
fn silent_merge(root: &Path) -> std::io::Result<()> {
    let remote_ref = format!("{REMOTE_NAME}/{SHARED_BRANCH}");
    run_git(root, &["merge", "--no-edit", &remote_ref])
}

// ----------------------------------------------------------------------------------------------
// Small git helpers (kept local so this isolated glue is self-contained)
// ----------------------------------------------------------------------------------------------

/// Run a git subcommand in `root`, mapping a non-zero exit to an `io::Error`.
fn run_git(root: &Path, args: &[&str]) -> std::io::Result<()> {
    let out = crate::gitrunner::command(root).args(args).output()?;
    if !out.status.success() {
        return Err(Error::other(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diff_names_trims_dedupes_and_drops_blanks() {
        let out = "firmware/main.c\n\nmechanik/gehaeuse.f3d\nfirmware/main.c\n  \n";
        assert_eq!(
            parse_diff_names(out),
            vec!["firmware/main.c".to_string(), "mechanik/gehaeuse.f3d".to_string()]
        );
        assert!(parse_diff_names("").is_empty());
        assert!(parse_diff_names("\n\n").is_empty());
    }
}
