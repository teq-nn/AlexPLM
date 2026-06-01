//! Git/LFS reading glue for the Graph Projection (Issue #8).
//!
//! Thin, side-effecting layer that turns a real repository into a [`RepoSnapshot`] for the
//! pure [`crate::graph::project_graph`], and promotes a Stand to a **Revision**. All git
//! invocation and filesystem access lives here; the pure projection in `graph.rs` never
//! does I/O. This is the same split as `watcher.rs` over `autocommit.rs`.
//!
//! A **Revision** is a promoted Stand. Promoting persists the user's human text into
//! `VERSION_NOTES.md` — the **only** place human text lives (E28) — and tags the commit so
//! the version label is durable. The words "commit"/"tag" never leave this layer.

use crate::artstore::{read_art, set_art};
use crate::autocommit::{format_timestamp, machine_message};
use crate::graph::{
    toggle_revision_art, BranchFact, CommitFact, RepoSnapshot, RevisionFact, VersionGraph,
};
use std::path::Path;
use std::time::SystemTime;

/// File that holds all human revision text (E28). One section per Revision, newest on
/// top; the only place the tool ever stores text a human wrote.
pub const VERSION_NOTES: &str = "VERSION_NOTES.md";

/// Prefix of the durable version tag we write when promoting a Stand. Internal — the user
/// only ever sees the version label, never the tag mechanism. `pub(crate)` so the push glue can
/// carry exactly these Revision labels to the server at Freigabe (E47, #30).
///
/// Tag-prefix decision (Issue #93): the **value** stays `"version/"` and is **not** renamed to
/// `"revision/"`. The Meilenstein→Revision change (E47/#30, #93) is a pure code-vocabulary rename
/// with no behavior change; the tag prefix is on-disk state in existing repositories. Renaming it
/// would silently orphan every already-written `version/<label>` tag (Revisionen would vanish from
/// the tree, and the push glue would stop carrying them) unless we also migrated tags. We keep the
/// on-disk format stable and only renamed the surrounding identifiers and docs.
pub(crate) const TAG_PREFIX: &str = "version/";

/// Prefix of the durable **write-protect marker** tag set on a Freigabe Revision (E8/E42).
/// Its presence is the git-side signal that the version tag is schreibgeschützt; un-releasing
/// removes it. Internal — the user only ever sees the Prototyp/Freigabe state, never the tag.
const PROTECT_PREFIX: &str = "version-protect/";

/// Read a repository at `root` into a [`RepoSnapshot`], then project it for the UI. The
/// projection itself is pure; this function only collects the facts.
pub fn read_graph(root: &Path) -> std::io::Result<VersionGraph> {
    let snapshot = read_snapshot(root)?;
    Ok(crate::graph::project_graph(&snapshot))
}

/// Collect commits (Stände) and version tags (Revisionen) into a [`RepoSnapshot`].
/// Offloading (E36) is v1-fern (its bookkeeping lands later); we report none for now but
/// the projection already handles offloaded markers, exercised by the snapshot tests.
pub fn read_snapshot(root: &Path) -> std::io::Result<RepoSnapshot> {
    let commits = read_commits(root)?;
    let revisions = read_revisions(root, &commits)?;
    let branches = read_branches(root)?;
    let active_branch = read_active_branch(root)?;
    let published = read_published(root)?;
    Ok(RepoSnapshot {
        commits,
        revisions,
        offloaded: Vec::new(),
        offloaded_archive: None,
        published,
        branches,
        active_branch,
    })
}

/// Commit ids on the **published** (shared) line: every Stand reachable from the remote-tracking
/// `origin/<shared>` ref (E47, #30). A successful Freigabe-Push — or a fetch of a colleague's
/// publish — advances that ref, so this is exactly what *this machine knows* has reached the shared
/// stand. Empty before the first publish: the ref does not exist yet, so nothing reads as
/// veröffentlicht. Best-effort — any git failure (no remote, detached HEAD, fresh repo) yields none
/// rather than erroring, so the tree still renders for an unpublished product.
fn read_published(root: &Path) -> std::io::Result<Vec<String>> {
    let branch = crate::pushglue::current_branch(root)?;
    let shared = crate::pushglue::shared_branch(root, &branch);
    let tracking = format!("refs/remotes/{}/{}", crate::setup::REMOTE_NAME, shared);
    let out = git(root, &["rev-list", &tracking])?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// Read every Stand **across all lines** (Zweige), not just the active one — so a Zweig
/// created outside the tool surfaces in the Versionsbaum (Issue #28). `--all` walks every
/// ref (local + remote branches and tags); the projection lays them out into lanes. Empty
/// when the repo has no commits yet (a fresh `git init`); not an error.
fn read_commits(root: &Path) -> std::io::Result<Vec<CommitFact>> {
    // Records separated by NUL; fields within a record by Unit Separator (0x1f) so commit
    // messages with newlines/commas survive intact. Format: hash, parents, ISO date, subject.
    let out = git(
        root,
        &["log", "--all", "--pretty=format:%H%x1f%P%x1f%cI%x1f%s%x00"],
    )?;
    if !out.status.success() {
        // No commits yet -> `git log` exits non-zero with "does not have any commits".
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut commits = Vec::new();
    for record in stdout.split('\0') {
        let record = record.trim_start_matches('\n');
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(4, '\u{1f}');
        let id = fields.next().unwrap_or("").to_string();
        let parents = fields
            .next()
            .unwrap_or("")
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let iso = fields.next().unwrap_or("");
        let message = fields.next().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        commits.push(CommitFact {
            id,
            parents,
            message,
            timestamp: normalize_committer_date(iso),
        });
    }
    Ok(commits)
}

/// Map git's committer ISO-8601 (`2026-05-30T11:00:00+02:00`) onto the project's machine
/// stamp shape (`YYYY-MM-DDTHH:MM:SSZ`). Best-effort: keeps the leading 19 chars and pins
/// `Z`, which is enough for the newest-first ordering the projection needs.
fn normalize_committer_date(iso: &str) -> String {
    let head: String = iso.chars().take(19).collect();
    if head.len() == 19 {
        format!("{head}Z")
    } else {
        iso.to_string()
    }
}

/// Collect every local line (Zweig) and its tip Stand as a [`BranchFact`]. One per local
/// branch ref; the projection turns the set into the visible lanes. Empty when the repo has
/// no branches yet; not an error. We read local branches only — a Zweig the user fetched
/// from a colleague materialises as a local tracking branch once they switch to it, which is
/// the moment it belongs in *their* Versionsbaum.
fn read_branches(root: &Path) -> std::io::Result<Vec<BranchFact>> {
    // `for-each-ref` is the plumbing form: stable, scriptable output, no decoration. Unlike
    // `log --pretty`, its `--format` does NOT expand `%x1f`, so we put the fixed-width object
    // id first and the (possibly space-free) branch name second, split on the first space.
    let out = git(
        root,
        &[
            "for-each-ref",
            "--format=%(objectname) %(refname:short)",
            "refs/heads",
        ],
    )?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut branches = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        let Some((tip, name)) = line.split_once(' ') else {
            continue;
        };
        let tip = tip.trim().to_string();
        let name = name.trim().to_string();
        if name.is_empty() || tip.is_empty() {
            continue;
        }
        branches.push(BranchFact { name, tip });
    }
    Ok(branches)
}

/// The active line's domain name (the current branch). `None` when HEAD is detached or the
/// repo has no commits yet — the projection then falls back to the first line.
fn read_active_branch(root: &Path) -> std::io::Result<Option<String>> {
    let out = git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if !out.status.success() {
        return Ok(None);
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if name.is_empty() { None } else { Some(name) })
}

/// Read version tags (`version/<label>`) and resolve each to the commit it points at,
/// noting whether `VERSION_NOTES.md` carries text for that label.
fn read_revisions(root: &Path, commits: &[CommitFact]) -> std::io::Result<Vec<RevisionFact>> {
    let out = git(root, &["tag", "--list", &format!("{TAG_PREFIX}*")])?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let notes = std::fs::read_to_string(root.join(VERSION_NOTES)).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut revisions = Vec::new();
    for tag in stdout.lines().map(str::trim).filter(|t| !t.is_empty()) {
        let version = tag.trim_start_matches(TAG_PREFIX).to_string();
        // Resolve the tag to its commit; skip silently if it cannot be resolved.
        let rev = git(root, &["rev-list", "-n", "1", tag])?;
        if !rev.status.success() {
            continue;
        }
        let commit_id = String::from_utf8_lossy(&rev.stdout).trim().to_string();
        if commit_id.is_empty() || !commits.iter().any(|c| c.id == commit_id) {
            continue;
        }
        let art = read_art(root, &version);
        revisions.push(RevisionFact {
            commit_id,
            has_notes: notes_have_section(&notes, &version),
            art,
            version,
        });
    }
    Ok(revisions)
}

/// Whether `VERSION_NOTES.md` contains real human text for `version`. The section header
/// is `## <version>` and the line right under it is an italic timestamp meta (`_<ts>_`,
/// see [`append_version_note`]); "has notes" means at least one non-blank body line that is
/// *not* that meta line, before the next section/EOF.
pub fn notes_have_section(notes: &str, version: &str) -> bool {
    let header = format!("## {version}");
    let mut lines = notes.lines();
    while let Some(line) = lines.next() {
        if line.trim() == header {
            // Found the header; scan its body until the next "## " header or EOF.
            for body in lines.by_ref() {
                if body.starts_with("## ") {
                    return false;
                }
                let t = body.trim();
                // Skip blank lines and the auto-written `_timestamp_` meta line.
                if t.is_empty() || (t.starts_with('_') && t.ends_with('_')) {
                    continue;
                }
                return true;
            }
            return false;
        }
    }
    false
}

/// Prepend a revision section to `VERSION_NOTES.md` text. Pure string transform so it is
/// table-testable: newest revision on top, `## <version>` header, then the human text.
pub fn append_version_note(existing: &str, version: &str, text: &str, timestamp: &str) -> String {
    let body = text.trim();
    let section = format!("## {version}\n_{timestamp}_\n\n{body}\n");
    if existing.trim().is_empty() {
        section
    } else {
        format!("{section}\n{}", existing.trim_start())
    }
}

/// Promote the Stand at `commit_id` to a **Revision**: persist the human `notes` text
/// into `VERSION_NOTES.md` (E28), commit that file, and tag the *promoted* Stand with its
/// version label so it is durable. Side-effecting — the only place that writes git here.
///
/// Returns the freshly projected [`VersionGraph`] so the UI updates in one round-trip.
/// `now` is injected so the persisted timestamp is testable.
pub fn promote_to_revision(
    root: &Path,
    commit_id: &str,
    version: &str,
    notes: &str,
    now: SystemTime,
) -> std::io::Result<VersionGraph> {
    let version = version.trim();
    if version.is_empty() {
        return Err(std::io::Error::other("Version darf nicht leer sein"));
    }
    if notes.trim().is_empty() {
        // VERSION_NOTES.md is the only place human text lives; a Revision must carry it.
        return Err(std::io::Error::other("Revision braucht einen Text"));
    }

    // Write-protect (E8): a Freigabe tag is schreibgeschützt — promoting must not silently
    // overwrite a released Revision. The user has to deliberately un-release it first
    // (toggle back to Prototyp), which is one handle away (E22).
    if read_art(root, version).is_write_protected() && tag_exists(root, version)? {
        return Err(std::io::Error::other(format!(
            "Revision {version} ist freigegeben und schreibgeschützt — erst zurückschalten"
        )));
    }

    let timestamp = format_timestamp(now);

    // 1) Persist the human text — append to VERSION_NOTES.md, newest section on top (E28).
    let notes_path = root.join(VERSION_NOTES);
    let existing = std::fs::read_to_string(&notes_path).unwrap_or_default();
    let updated = append_version_note(&existing, version, notes, &timestamp);
    std::fs::write(&notes_path, updated)?;

    // 2) Commit the notes file with a boring machine message (no human text in the message).
    git_ok(root, &["add", "--", VERSION_NOTES])?;
    let msg = machine_message(VERSION_NOTES, &timestamp);
    git_ok(root, &["commit", "-m", &msg])?;

    // 3) Tag the *promoted* Stand with its durable version label. The user picks which Stand
    //    is the Revision; the notes commit above only carries the human text.
    let tag = format!("{TAG_PREFIX}{version}");
    git_ok(root, &["tag", "-f", &tag, commit_id])?;

    read_graph(root)
}

/// Toggle a Revision's **Art** between Prototyp and Freigabe (Issue #41, E42). The pure
/// two-state machine lives in [`toggle_revision_art`]; this glue persists the new Art and
/// applies the git-side write-protect on the tag, then re-projects so the UI updates in one
/// round-trip.
///
/// - Prototyp → Freigabe = „Releasen": record Freigabe and **write-protect** the tag (E8).
/// - Freigabe → Prototyp = the deliberate reversible „Un-Release": record Prototyp and lift
///   the write-protect.
///
/// SEAM for Issue #52: the dreistufige Freigabe-Gate block-check (E19.3) belongs *here*,
/// right before raising to Freigabe — if the gate fails, this would refuse the toggle. This
/// slice intentionally performs only the Art flip + write-protect; the gate check is #52.
pub fn toggle_revision_freigabe(root: &Path, version: &str) -> std::io::Result<VersionGraph> {
    let version = version.trim();
    if version.is_empty() {
        return Err(std::io::Error::other("Version darf nicht leer sein"));
    }
    if !tag_exists(root, version)? {
        return Err(std::io::Error::other(format!(
            "Keine Revision {version}"
        )));
    }

    let next = toggle_revision_art(read_art(root, version));

    // <<< Issue #52 plugs the dreistufige Freigabe-Gate block-check in here (before Freigabe).

    set_art(root, version, next)?;
    apply_write_protect(root, version, next.is_write_protected())?;
    read_graph(root)
}

/// Whether a durable version tag exists for `version`.
fn tag_exists(root: &Path, version: &str) -> std::io::Result<bool> {
    let tag = format!("{TAG_PREFIX}{version}");
    let out = git(root, &["tag", "--list", &tag])?;
    Ok(out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

/// Apply (or lift) the write-protect on a version tag (E8). A Freigabe tag is marked
/// `version-protect/<label>`; un-releasing removes that marker tag. The protect marker is the
/// durable git-side signal a server-side hook (or a future #52 gate) can enforce; locally it
/// is also what [`promote_to_revision`] checks before overwriting a released Revision.
fn apply_write_protect(root: &Path, version: &str, protect: bool) -> std::io::Result<()> {
    let target = format!("{TAG_PREFIX}{version}");
    let marker = format!("{PROTECT_PREFIX}{version}");
    if protect {
        // Point a marker tag at the same commit, so the protection is itself durable in git.
        git_ok(root, &["tag", "-f", &marker, &target])?;
    } else if tag_ref_exists(root, &marker)? {
        git_ok(root, &["tag", "-d", &marker])?;
    }
    Ok(())
}

/// Whether a fully-qualified tag name exists (used for the protect marker).
fn tag_ref_exists(root: &Path, tag: &str) -> std::io::Result<bool> {
    let out = git(root, &["tag", "--list", tag])?;
    Ok(out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

fn git(root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    crate::gitrunner::command(root).args(args).output()
}

fn git_ok(root: &Path, args: &[&str]) -> std::io::Result<()> {
    let out = git(root, args)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_version_note_puts_newest_section_on_top() {
        let first = append_version_note("", "v0.1", "  erster Stand  ", "2026-05-30T09:00:00Z");
        assert_eq!(first, "## v0.1\n_2026-05-30T09:00:00Z_\n\nerster Stand\n");

        let second = append_version_note(&first, "v0.2", "zweiter Stand", "2026-05-30T10:00:00Z");
        // Newest (v0.2) on top, older (v0.1) below.
        assert!(second.starts_with("## v0.2\n"));
        assert!(second.contains("## v0.1\n"));
        assert!(second.find("v0.2").unwrap() < second.find("v0.1").unwrap());
    }

    #[test]
    fn notes_have_section_detects_text_presence() {
        let notes = append_version_note("", "v0.1", "echter text", "2026-05-30T09:00:00Z");
        assert!(notes_have_section(&notes, "v0.1"));
        assert!(!notes_have_section(&notes, "v9.9")); // absent section
                                                      // A header with no body is "no notes".
        let empty = "## v0.3\n_2026-05-30T09:00:00Z_\n\n## v0.2\nbody\n";
        assert!(!notes_have_section(empty, "v0.3"));
        assert!(notes_have_section(empty, "v0.2"));
    }

    #[test]
    fn normalize_committer_date_pins_utc_shape() {
        assert_eq!(
            normalize_committer_date("2026-05-30T11:00:00+02:00"),
            "2026-05-30T11:00:00Z"
        );
        assert_eq!(
            normalize_committer_date("2026-05-30T11:00:00Z"),
            "2026-05-30T11:00:00Z"
        );
    }
}
