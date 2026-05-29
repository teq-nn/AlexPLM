//! Graph Projection (Issue #8).
//!
//! Pure, read-only projection from a **snapshot** of git/LFS facts into the dark
//! "display" version tree the UI renders: Stände as nodes, Meilensteine marked, and
//! nodes whose binary content has been **offloaded** (E36) marked as such. The version
//! bar reads the **active Meilenstein** version from this same projection.
//!
//! Like `projection.rs` and `autocommit.rs`, the decision logic is a pure function:
//! **snapshot in, projection out, no I/O**. The git/LFS reading glue lives in
//! [`crate::graphread`] and is kept deliberately thin; everything testable lives here and
//! is exercised by `#[cfg(test)]` table tests plus an end-to-end snapshot test.
//!
//! Vocabulary stays in the domain (E33/E39): nodes are **Stände**, a promoted Stand is a
//! **Meilenstein**; the words "commit"/"tag" never surface to the user.

use serde::Serialize;

/// One commit as observed in the repository, newest-relevant fields only. This is the raw
/// fact the reading glue collects; the projection turns a list of these into the tree.
///
/// `id` is the commit's stable identity (its hash); `parents` are the ids it descends from
/// (one for a normal Stand, two for a merge). `message` is the boring machine message
/// (E39) — never shown, used only to recover the recorded path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFact {
    pub id: String,
    pub parents: Vec<String>,
    pub message: String,
    /// Machine timestamp `YYYY-MM-DDTHH:MM:SSZ`.
    pub timestamp: String,
}

/// A Meilenstein fact: a commit the user promoted, carrying its human version label and
/// whether `VERSION_NOTES.md` text exists for it (the only place human text lives — E28).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneFact {
    /// Commit id this Meilenstein points at.
    pub commit_id: String,
    /// Human version label, e.g. `v0.4`. Mono in the version bar.
    pub version: String,
    /// Whether a non-empty `VERSION_NOTES.md` text was persisted for this Meilenstein.
    pub has_notes: bool,
}

/// The complete read-only snapshot fed to the projection. Collected by the git/LFS glue;
/// the projection performs **no I/O** over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSnapshot {
    /// Commits, any order; the projection orders them newest-first by timestamp.
    pub commits: Vec<CommitFact>,
    /// Promoted Stände (Meilensteine).
    pub milestones: Vec<MilestoneFact>,
    /// Commit ids whose binary content has been offloaded to a cold archive (E36). The
    /// git history is untouched — only the heavy LFS content left the server.
    pub offloaded: Vec<String>,
    /// Archive label shown on offloaded nodes, e.g. `2025-11`. `None` if unknown.
    pub offloaded_archive: Option<String>,
}

/// A single node in the version tree: a Stand, possibly promoted to a Meilenstein and/or
/// with its binary content offloaded. Serialized straight to the UI.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct StandNode {
    /// Stable id (commit hash). The UI keys rows on this; never displayed as git.
    pub id: String,
    /// Machine timestamp `YYYY-MM-DDTHH:MM:SSZ`.
    pub timestamp: String,
    /// Product-relative path the auto-commit recorded, parsed from the boring message;
    /// `"."` when the message is not the auto shape (e.g. a Meilenstein/import commit).
    pub path: String,
    /// Set when this Stand has been promoted to a Meilenstein: its human version label.
    pub milestone: Option<String>,
    /// Whether a `VERSION_NOTES.md` text exists for this Meilenstein (only if `milestone`).
    pub has_notes: bool,
    /// Whether this node's binary content has been offloaded (E36). The node stays in the
    /// tree, honestly marked "Inhalt ausgelagert".
    pub offloaded: bool,
}

/// The dark "display" version tree the UI renders, plus the active milestone version that
/// the version bar shows in Mono.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct VersionGraph {
    /// Stände newest-first.
    pub nodes: Vec<StandNode>,
    /// The active Meilenstein version (newest promoted Stand), or `None` if the product has
    /// no Meilenstein yet. The version bar shows this in Mono.
    pub active_milestone: Option<String>,
    /// Archive label for offloaded nodes, surfaced once for the legend; `None` if none.
    pub offloaded_archive: Option<String>,
}

/// Parse the product-relative path out of a boring auto-commit message
/// (`auto: <path>, <timestamp>` — see [`crate::autocommit::machine_message`]). Returns the
/// path, or `"."` for any message that is not the auto shape (Meilenstein, import, init).
/// Pure over the message string so it is table-testable.
pub fn path_from_message(message: &str) -> String {
    let rest = match message.strip_prefix("auto: ") {
        Some(r) => r,
        None => return ".".to_string(),
    };
    // The message is `auto: <path>, <timestamp>`; the timestamp has no comma, so split off
    // the last `, ` to recover a path that itself may (in theory) contain commas.
    match rest.rsplit_once(", ") {
        Some((path, _ts)) => path.to_string(),
        None => rest.to_string(),
    }
}

/// Project a [`RepoSnapshot`] into the [`VersionGraph`] the UI renders. **Pure**: no I/O,
/// no git mutation — snapshot in, projection out.
///
/// Ordering: newest-first. With real git the snapshot already arrives in `git log` order
/// (newest first); to stay deterministic for tests regardless of input order we sort by
/// timestamp descending, breaking ties by id descending.
pub fn project_graph(snapshot: &RepoSnapshot) -> VersionGraph {
    let milestone_of = |id: &str| snapshot.milestones.iter().find(|m| m.commit_id == id);
    let is_offloaded = |id: &str| snapshot.offloaded.iter().any(|o| o == id);

    let mut nodes: Vec<StandNode> = snapshot
        .commits
        .iter()
        .map(|c| {
            let ms = milestone_of(&c.id);
            StandNode {
                id: c.id.clone(),
                timestamp: c.timestamp.clone(),
                path: path_from_message(&c.message),
                milestone: ms.map(|m| m.version.clone()),
                has_notes: ms.map(|m| m.has_notes).unwrap_or(false),
                offloaded: is_offloaded(&c.id),
            }
        })
        .collect();

    // Newest first; deterministic tie-break on id so equal timestamps order stably.
    nodes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));

    // Active Meilenstein = the newest promoted Stand in that newest-first order.
    let active_milestone = nodes.iter().find_map(|n| n.milestone.clone());

    VersionGraph {
        nodes,
        active_milestone,
        offloaded_archive: snapshot.offloaded_archive.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, ts: &str, msg: &str) -> CommitFact {
        CommitFact {
            id: id.to_string(),
            parents: vec![],
            message: msg.to_string(),
            timestamp: ts.to_string(),
        }
    }

    #[test]
    fn path_from_message_parses_auto_shape_and_defaults_otherwise() {
        // table: message -> recovered path
        let cases: &[(&str, &str)] = &[
            (
                "auto: elektronik/regler, 2026-05-30T09:15:00Z",
                "elektronik/regler",
            ),
            ("auto: ., 1970-01-01T00:00:00Z", "."),
            (
                "auto: mechanik/gehaeuse/gehaeuse.f3d, 2026-12-31T23:59:59Z",
                "mechanik/gehaeuse/gehaeuse.f3d",
            ),
            // not the auto shape -> "."
            ("init", "."),
            ("Meilenstein v0.4", "."),
        ];
        for (msg, expected) in cases {
            assert_eq!(path_from_message(msg), *expected, "msg={msg}");
        }
    }

    #[test]
    fn projects_stande_as_nodes_newest_first() {
        let snap = RepoSnapshot {
            commits: vec![
                commit("a", "2026-05-30T09:00:00Z", "init"),
                commit("c", "2026-05-30T11:00:00Z", "auto: x, 2026-05-30T11:00:00Z"),
                commit("b", "2026-05-30T10:00:00Z", "auto: y, 2026-05-30T10:00:00Z"),
            ],
            milestones: vec![],
            offloaded: vec![],
            offloaded_archive: None,
        };
        let g = project_graph(&snap);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"], "newest-first by timestamp");
        assert_eq!(g.active_milestone, None, "no Meilenstein yet");
        assert!(g.nodes.iter().all(|n| n.milestone.is_none()));
    }

    #[test]
    fn promoted_stand_becomes_a_meilenstein_and_drives_the_version_bar() {
        let snap = RepoSnapshot {
            commits: vec![
                commit("a", "2026-05-30T09:00:00Z", "auto: x, 2026-05-30T09:00:00Z"),
                commit("b", "2026-05-30T10:00:00Z", "auto: y, 2026-05-30T10:00:00Z"),
                commit("c", "2026-05-30T11:00:00Z", "auto: z, 2026-05-30T11:00:00Z"),
            ],
            milestones: vec![
                MilestoneFact {
                    commit_id: "a".into(),
                    version: "v0.1".into(),
                    has_notes: true,
                },
                MilestoneFact {
                    commit_id: "b".into(),
                    version: "v0.2".into(),
                    has_notes: true,
                },
            ],
            offloaded: vec![],
            offloaded_archive: None,
        };
        let g = project_graph(&snap);

        // The newest Meilenstein wins the version bar.
        assert_eq!(g.active_milestone.as_deref(), Some("v0.2"));

        // b and a carry their milestone label; c does not.
        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(node("b").milestone.as_deref(), Some("v0.2"));
        assert!(node("b").has_notes);
        assert_eq!(node("a").milestone.as_deref(), Some("v0.1"));
        assert_eq!(node("c").milestone, None);
    }

    #[test]
    fn offloaded_nodes_are_marked_but_remain_in_the_tree() {
        let snap = RepoSnapshot {
            commits: vec![
                commit(
                    "old",
                    "2025-01-01T00:00:00Z",
                    "auto: g.f3d, 2025-01-01T00:00:00Z",
                ),
                commit(
                    "new",
                    "2026-05-30T11:00:00Z",
                    "auto: g.f3d, 2026-05-30T11:00:00Z",
                ),
            ],
            milestones: vec![MilestoneFact {
                commit_id: "old".into(),
                version: "v0.1".into(),
                has_notes: true,
            }],
            offloaded: vec!["old".into()],
            offloaded_archive: Some("2025-11".into()),
        };
        let g = project_graph(&snap);

        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();
        assert!(node("old").offloaded, "old content offloaded");
        assert!(!node("new").offloaded);
        // Still in the tree, honestly marked — history untouched (E36).
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.offloaded_archive.as_deref(), Some("2025-11"));
        // The Meilenstein label survives offloading (only content left, not the pointer/tag).
        assert_eq!(node("old").milestone.as_deref(), Some("v0.1"));
    }

    #[test]
    fn empty_repo_projects_to_an_empty_tree() {
        let snap = RepoSnapshot {
            commits: vec![],
            milestones: vec![],
            offloaded: vec![],
            offloaded_archive: None,
        };
        let g = project_graph(&snap);
        assert!(g.nodes.is_empty());
        assert_eq!(g.active_milestone, None);
    }
}
