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
use crate::syncdecider::{decide_sync, DivergedPath, StandChoice, SyncDecision};
use serde::Serialize;
use std::io::Error;
use std::path::Path;

/// The fallback shared release branch the daily sync tracks when the remote's real branch cannot
/// be resolved (E34). The fetch/diff/merge no longer use this blindly — they resolve the *actually
/// shared* branch of the remote first (see [`resolved_branch`]); a `master`-repo would otherwise
/// fail every pass with „couldn't find remote ref main" (Issue #64 on the pull side, #54-Folge).
pub const SHARED_BRANCH: &str = "main";

/// The branch the daily sync actually fetches/diffs/merges against — the remote's real shared
/// branch (Issue #64), reusing the same resolution as the push side ([`crate::pushglue`]). On a
/// `master`-repo this is `master`, so the silent pull works instead of failing on a missing `main`.
/// Cheap (local `symbolic-ref`/`rev-parse`); falls back to the current branch, then [`SHARED_BRANCH`].
fn resolved_branch(root: &Path) -> String {
    let current =
        crate::pushglue::current_branch(root).unwrap_or_else(|_| SHARED_BRANCH.to_string());
    crate::pushglue::shared_branch(root, &current)
}

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
    let remote_ref = format!("{REMOTE_NAME}/{}", resolved_branch(root));
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
    run_git(root, &["fetch", REMOTE_NAME, &resolved_branch(root)])
}

/// Carry out the **silent merge** of the fetched remote stand into the local branch. Only reached
/// when the Sync Decider has proven every diverged path is free, mergeable text — so git's merge
/// touches only mergeable files and can produce no conflict marker. No user prompt (E41).
fn silent_merge(root: &Path) -> std::io::Result<()> {
    let remote_ref = format!("{REMOTE_NAME}/{}", resolved_branch(root));
    run_git(root, &["merge", "--no-edit", &remote_ref])
}

// ----------------------------------------------------------------------------------------------
// Resolving the loud exception — apply the chosen side and FINISH the sync, no marker ever shown
// ----------------------------------------------------------------------------------------------

/// **Resolve a laute Ausnahme** (Issue #43, E41): the single, deliberate moment the user answers
/// the loud question by saying *whose stand applies* for the contested artifact. We then carry the
/// merge to completion with the chosen side — and a **raw git conflict marker is never written to
/// the worktree**, because every contested path is taken whole from one side, never line-merged.
///
/// The dangerous hand-resolution is hidden: the user only ever picked "mein Stand" / "Bens Stand".
///
/// The mechanism (the load-bearing E41 guarantee that a merge must never silently corrupt a file):
/// 1. Start the merge **without committing** (`merge --no-commit --no-ff`). git stages the freely
///    mergeable text on its own; the unmergeable contested paths land "both modified".
/// 2. For the contested artifact at `path`, **force the whole chosen side** in — `--ours` keeps the
///    user's bytes, `--theirs` takes the colleague's — then stage it. No three-way line merge ever
///    runs on it, so no `<<<<<<<`/`Missing („` corruption is possible.
/// 3. Defensively resolve **any other** still-conflicted path the same chosen way, so the tree is
///    guaranteed marker-free before we commit.
/// 4. Commit the merge. The sync is finished; the daily rhythm resumes "gesichert".
///
/// `path` is the contested product-relative artifact (as named in the [`LoudQuestion`]); `choice`
/// is the user's [`StandChoice`]. Returns the calm [`SyncStatus::Gesichert`] — the loud exception
/// is over and nothing git-flavoured surfaces.
pub fn resolve_sync(root: &Path, path: &str, choice: StandChoice) -> std::io::Result<SyncOutcome> {
    // Make sure we are resolving against the freshest remote stand (best-effort, never corrupts).
    let _ = fetch(root);
    let remote_ref = format!("{REMOTE_NAME}/{}", resolved_branch(root));

    // 1. Begin the merge but do NOT commit: git auto-stages free text, leaves contested paths for us.
    //    `--no-ff` so there is always a merge commit to finish, even on a trivial case.
    //    A non-zero exit here is the *expected* "merge has conflicts" signal — not an error; the
    //    real error would be an inability to start the merge (dirty worktree), caught by checking
    //    that a merge is actually in progress below.
    let _ = run_git(root, &["merge", "--no-commit", "--no-ff", &remote_ref]);

    // If git could not even start the merge (e.g. an unexpected state), there is nothing in
    // progress to resolve — surface a clean error rather than committing a half-state.
    if !merge_in_progress(root) {
        return Err(Error::other(
            "Der Stand ließ sich nicht zusammenführen — bitte erneut versuchen.".to_string(),
        ));
    }

    // 2. Force the whole chosen side for the contested artifact, then stage it. Taken whole from one
    //    side → never line-merged → no conflict marker, no „Missing („-Korruption.
    apply_choice(root, path, choice)?;

    // 3. Defensive: resolve every OTHER path git still reports as conflicted, the same chosen way,
    //    so the committed tree is provably marker-free (the E41 acid test).
    for other in conflicted_paths(root)? {
        apply_choice(root, &other, choice)?;
    }

    // 4. Finish the merge. The contested side is decided; the sync completes cleanly.
    run_git(root, &["commit", "--no-edit"])?;

    Ok(SyncOutcome { status: SyncStatus::Gesichert })
}

/// The git arguments that **take one whole side** of a contested path during a merge, per the
/// user's [`StandChoice`]. Pure + table-tested: keeping the side→flag mapping in one place means a
/// new decision branch is tested in isolation, and the I/O wrapper [`apply_choice`] just obeys it.
///
/// `Mine` → `--ours` (keep the user's bytes); `Theirs` → `--theirs` (take the colleague's). The
/// path is taken **whole** from that side — never three-way merged — so no marker can be written.
pub fn checkout_args(path: &str, choice: StandChoice) -> [String; 4] {
    let side = match choice {
        StandChoice::Mine => "--ours",
        StandChoice::Theirs => "--theirs",
    };
    [
        "checkout".to_string(),
        side.to_string(),
        "--".to_string(),
        path.to_string(),
    ]
}

/// Force the whole chosen side of one contested path into the worktree and stage it. The path is
/// taken whole (`checkout --ours/--theirs`) — never line-merged — so no conflict marker is written.
fn apply_choice(root: &Path, path: &str, choice: StandChoice) -> std::io::Result<()> {
    let args = checkout_args(path, choice);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git(root, &argv)?;
    run_git(root, &["add", "--", path])
}

/// Whether a merge is currently in progress (`MERGE_HEAD` exists). Used to tell "git started the
/// merge and left conflicts for us" (the normal path) from "git refused to start" (an error).
fn merge_in_progress(root: &Path) -> bool {
    crate::gitrunner::command(root)
        .args(["rev-parse", "--verify", "--quiet", "MERGE_HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The paths git still reports as conflicted ("both modified"), via `git diff --name-only
/// --diff-filter=U`. These are exactly the paths a marker would survive on, so every one is forced
/// to the chosen side before the commit.
fn conflicted_paths(root: &Path) -> std::io::Result<Vec<String>> {
    let out = crate::gitrunner::command(root)
        .args(["diff", "--name-only", "--diff-filter=U"])
        .output()?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    Ok(parse_diff_names(&String::from_utf8_lossy(&out.stdout)))
}

// ----------------------------------------------------------------------------------------------
// Small git helpers (kept local so this isolated glue is self-contained)
// ----------------------------------------------------------------------------------------------

/// Run a git subcommand in `root`, mapping a non-zero exit to an `io::Error`.
fn run_git(root: &Path, args: &[&str]) -> std::io::Result<()> {
    // Bounded: the fetch reaches the network. Local subcommands (merge) finish well under the
    // bound, so this is harmless to them while a stuck connection can no longer hang the sync.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The two resolve branches map to the two git "take a whole side" flags — never a three-way
    /// merge (so no marker can be written). `Mine` keeps our bytes, `Theirs` takes the colleague's.
    #[test]
    fn checkout_args_take_the_chosen_whole_side() {
        let mine = checkout_args("elektronik/board.kicad_pcb", StandChoice::Mine);
        assert_eq!(
            mine,
            ["checkout", "--ours", "--", "elektronik/board.kicad_pcb"].map(String::from)
        );
        let theirs = checkout_args("mechanik/gehaeuse.f3d", StandChoice::Theirs);
        assert_eq!(
            theirs,
            ["checkout", "--theirs", "--", "mechanik/gehaeuse.f3d"].map(String::from)
        );
        // Neither flag is a three-way "-X" line merge — the path is always taken whole.
        for c in [StandChoice::Mine, StandChoice::Theirs] {
            let args = checkout_args("x.kicad_sch", c);
            assert_eq!(args[0], "checkout");
            assert!(args[1] == "--ours" || args[1] == "--theirs");
        }
    }

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
