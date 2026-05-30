//! Git/LFS reading glue for the Graph Projection (Issue #8).
//!
//! Thin, side-effecting layer that turns a real repository into a [`RepoSnapshot`] for the
//! pure [`crate::graph::project_graph`], and promotes a Stand to a **Meilenstein**. All git
//! invocation and filesystem access lives here; the pure projection in `graph.rs` never
//! does I/O. This is the same split as `watcher.rs` over `autocommit.rs`.
//!
//! A **Meilenstein** is a promoted Stand. Promoting persists the user's human text into
//! `VERSION_NOTES.md` — the **only** place human text lives (E28) — and tags the commit so
//! the version label is durable. The words "commit"/"tag" never leave this layer.

use crate::autocommit::{format_timestamp, machine_message};
use crate::graph::{CommitFact, MilestoneFact, RepoSnapshot, VersionGraph};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

/// File that holds all human milestone text (E28). One section per Meilenstein, newest on
/// top; the only place the tool ever stores text a human wrote.
pub const VERSION_NOTES: &str = "VERSION_NOTES.md";

/// Prefix of the durable version tag we write when promoting a Stand. Internal — the user
/// only ever sees the version label, never the tag mechanism.
const TAG_PREFIX: &str = "version/";

/// Read a repository at `root` into a [`RepoSnapshot`], then project it for the UI. The
/// projection itself is pure; this function only collects the facts.
pub fn read_graph(root: &Path) -> std::io::Result<VersionGraph> {
    let snapshot = read_snapshot(root)?;
    Ok(crate::graph::project_graph(&snapshot))
}

/// Collect commits (Stände) and version tags (Meilensteine) into a [`RepoSnapshot`].
/// Offloading (E36) is v1-fern (its bookkeeping lands later); we report none for now but
/// the projection already handles offloaded markers, exercised by the snapshot tests.
pub fn read_snapshot(root: &Path) -> std::io::Result<RepoSnapshot> {
    let commits = read_commits(root)?;
    let milestones = read_milestones(root, &commits)?;
    Ok(RepoSnapshot {
        commits,
        milestones,
        offloaded: Vec::new(),
        offloaded_archive: None,
    })
}

/// Read every commit reachable from HEAD as a [`CommitFact`]. Empty when the repo has no
/// commits yet (a fresh `git init`); not an error.
fn read_commits(root: &Path) -> std::io::Result<Vec<CommitFact>> {
    // Records separated by NUL; fields within a record by Unit Separator (0x1f) so commit
    // messages with newlines/commas survive intact. Format: hash, parents, ISO date, subject.
    let out = git(root, &["log", "--pretty=format:%H%x1f%P%x1f%cI%x1f%s%x00"])?;
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

/// Read version tags (`version/<label>`) and resolve each to the commit it points at,
/// noting whether `VERSION_NOTES.md` carries text for that label.
fn read_milestones(root: &Path, commits: &[CommitFact]) -> std::io::Result<Vec<MilestoneFact>> {
    let out = git(root, &["tag", "--list", &format!("{TAG_PREFIX}*")])?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let notes = std::fs::read_to_string(root.join(VERSION_NOTES)).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut milestones = Vec::new();
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
        milestones.push(MilestoneFact {
            commit_id,
            has_notes: notes_have_section(&notes, &version),
            version,
        });
    }
    Ok(milestones)
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

/// Prepend a milestone section to `VERSION_NOTES.md` text. Pure string transform so it is
/// table-testable: newest milestone on top, `## <version>` header, then the human text.
pub fn append_version_note(existing: &str, version: &str, text: &str, timestamp: &str) -> String {
    let body = text.trim();
    let section = format!("## {version}\n_{timestamp}_\n\n{body}\n");
    if existing.trim().is_empty() {
        section
    } else {
        format!("{section}\n{}", existing.trim_start())
    }
}

/// Promote the Stand at `commit_id` to a **Meilenstein**: persist the human `notes` text
/// into `VERSION_NOTES.md` (E28), commit that file, and tag the *promoted* Stand with its
/// version label so it is durable. Side-effecting — the only place that writes git here.
///
/// Returns the freshly projected [`VersionGraph`] so the UI updates in one round-trip.
/// `now` is injected so the persisted timestamp is testable.
pub fn promote_to_milestone(
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
        // VERSION_NOTES.md is the only place human text lives; a Meilenstein must carry it.
        return Err(std::io::Error::other("Meilenstein braucht einen Text"));
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
    //    is the Meilenstein; the notes commit above only carries the human text.
    let tag = format!("{TAG_PREFIX}{version}");
    git_ok(root, &["tag", "-f", &tag, commit_id])?;

    read_graph(root)
}

fn git(root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("git").arg("-C").arg(root).args(args).output()
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
