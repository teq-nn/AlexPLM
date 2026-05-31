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
//!
//! ## Zweige (Issue #28)
//!
//! Stände do not always form one straight line: a colleague (or the user, in their own
//! working copy) may start a **Zweig** outside the tool and record Stände on it. The
//! projection collects Stände across *all* lines, not just the active one, and lays them
//! out into **Bahnen** (lanes): the active line is the trunk (lane 0) and each diverging
//! Zweig becomes its own visible line. The active line stays clearly marked. A product with
//! a single linear history simply has one lane and renders exactly as before.

use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

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

/// The **Art** (kind) of a Meilenstein (E42). The block-strictness is a property of the
/// milestone act, not of the branch. A freshly promoted Meilenstein is a **Prototyp**
/// (lax: warnings only, frictionless tagging); a deliberate **Toggle** raises it to a
/// **Freigabe** (streng), which write-protects the tag (E8). Toggling back is a deliberate
/// reversible „Un-Release" (E22). Serialized to the UI in kebab-case (`"prototyp"` /
/// `"freigabe"`).
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MilestoneArt {
    /// Lax: the default for a new Meilenstein. No write-protect, warnings only.
    #[default]
    Prototyp,
    /// Streng: a released Meilenstein. Its tag is write-protected; reaching it is the
    /// deliberate Freigabe toggle.
    Freigabe,
}

impl MilestoneArt {
    /// The on-disk token for the per-tag store. Stable, lowercase, never localized.
    pub fn as_token(self) -> &'static str {
        match self {
            MilestoneArt::Prototyp => "prototyp",
            MilestoneArt::Freigabe => "freigabe",
        }
    }

    /// Parse a stored token back into an Art. Anything unrecognized (or missing) falls back
    /// to the default `Prototyp` — a tag with no recorded Art is lax (E42), never an error.
    pub fn from_token(token: &str) -> MilestoneArt {
        match token.trim() {
            "freigabe" => MilestoneArt::Freigabe,
            _ => MilestoneArt::Prototyp,
        }
    }

    /// Whether this Art write-protects its tag (E8). Only a Freigabe is schreibgeschützt.
    pub fn is_write_protected(self) -> bool {
        matches!(self, MilestoneArt::Freigabe)
    }
}

/// The pure toggle state machine for the Meilenstein-Art (E42). Returns the Art a milestone
/// reaches when the user flips its toggle: Prototyp → Freigabe („Releasen") and
/// Freigabe → Prototyp (the deliberate reversible „Un-Release"). No I/O — the git/file glue
/// (write-protect on, write-protect off) lives in [`crate::graphread`].
///
/// NOTE (seam for Issue #52): raising to Freigabe is where the dreistufige Freigabe-Gate
/// block-check (E19.3) will plug in *before* this transition is allowed. This slice performs
/// the toggle + write-protect only; the gate check is deliberately left out (issue #52).
pub fn toggle_milestone_art(current: MilestoneArt) -> MilestoneArt {
    match current {
        MilestoneArt::Prototyp => MilestoneArt::Freigabe,
        MilestoneArt::Freigabe => MilestoneArt::Prototyp,
    }
}

/// A Meilenstein fact: a commit the user promoted, carrying its human version label, its
/// **Art** (Prototyp/Freigabe — E42), and whether `VERSION_NOTES.md` text exists for it (the
/// only place human text lives — E28).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneFact {
    /// Commit id this Meilenstein points at.
    pub commit_id: String,
    /// Human version label, e.g. `v0.4`. Mono in the version bar.
    pub version: String,
    /// Whether a non-empty `VERSION_NOTES.md` text was persisted for this Meilenstein.
    pub has_notes: bool,
    /// The Meilenstein-Art (Prototyp/Freigabe). A new Meilenstein defaults to Prototyp.
    pub art: MilestoneArt,
}

/// One **Zweig** (branch line) as observed in the repository: its domain name and the id of
/// the Stand at its tip. The reading glue collects one of these per local/remote line; the
/// projection turns the set into visible [`StandNode::lane`]s. The word "branch" stays here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchFact {
    /// The line's domain name (e.g. `main`, `gehaeuse-v2`). Shown as the lane label.
    pub name: String,
    /// Commit id at the tip of this line.
    pub tip: String,
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
    /// Every observed line (Zweig), including the active one. Empty/one entry => the tree is
    /// a single linear history and lays out as one lane. May be empty for back-compat.
    pub branches: Vec<BranchFact>,
    /// Name of the active line (the user's current branch / HEAD). Drives which lane is the
    /// trunk and which Stände are marked active. `None` => fall back to the first line.
    pub active_branch: Option<String>,
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
    /// The Meilenstein-Art (Prototyp/Freigabe — E42), present only when `milestone` is set.
    /// A Freigabe write-protects the tag; a Prototyp is lax. `None` for a plain Stand.
    pub milestone_art: Option<MilestoneArt>,
    /// Whether a `VERSION_NOTES.md` text exists for this Meilenstein (only if `milestone`).
    pub has_notes: bool,
    /// Whether this node's binary content has been offloaded (E36). The node stays in the
    /// tree, honestly marked "Inhalt ausgelagert".
    pub offloaded: bool,
    /// Which **Bahn** (lane) this Stand sits on: `0` is the trunk (the active line), each
    /// diverging Zweig gets its own positive index. A single linear history is all lane `0`.
    pub lane: usize,
    /// The Zweig name labelling this Stand's lane (e.g. `gehaeuse-v2`). `None` for the trunk
    /// (lane 0) and for unnamed lines; the UI shows it once per lane (at the lane's tip).
    pub branch: Option<String>,
    /// `true` when this Stand lies on the active line (the user's current Zweig). The active
    /// line stays clearly marked even when other Zweige are visible.
    pub on_active: bool,
    /// The Stände this one **folgt auf** (its direct predecessors): one for a normal Stand,
    /// two where two Linien were **zusammengeführt**. The UI draws a connector from this Stand
    /// down to each predecessor present in the tree, so forks and Zusammenführungen become
    /// visible lines. Ids only — never shown as git.
    pub parents: Vec<String>,
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
    /// The Art of the active Meilenstein (Prototyp/Freigabe — E42), or `None` if there is no
    /// active Meilenstein. The version bar shows the Freigabe/Prototyp state alongside it.
    pub active_milestone_art: Option<MilestoneArt>,
    /// Archive label for offloaded nodes, surfaced once for the legend; `None` if none.
    pub offloaded_archive: Option<String>,
    /// Name of the active line (Zweig), echoed for the UI marker; `None` if unknown.
    pub active_branch: Option<String>,
    /// Number of distinct lanes in the tree; `1` for a single linear history. The UI uses
    /// it to size the lane gutter only when there is more than one line.
    pub lane_count: usize,
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
///
/// Branches (Issue #28): every observed line (Zweig) is laid out into a **Bahn** (lane). The
/// active line is the trunk (lane 0); each Stand that only a diverging Zweig can reach lands
/// on that Zweig's own lane, so an externally-created branch shows up as a distinct line. A
/// single linear history collapses to one lane and is unchanged.
pub fn project_graph(snapshot: &RepoSnapshot) -> VersionGraph {
    let milestone_of = |id: &str| snapshot.milestones.iter().find(|m| m.commit_id == id);
    let is_offloaded = |id: &str| snapshot.offloaded.iter().any(|o| o == id);

    let layout = LaneLayout::compute(snapshot);

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
                milestone_art: ms.map(|m| m.art),
                has_notes: ms.map(|m| m.has_notes).unwrap_or(false),
                offloaded: is_offloaded(&c.id),
                lane: layout.lane_of(&c.id),
                branch: layout.label_of(&c.id),
                on_active: layout.lane_of(&c.id) == 0,
                parents: c.parents.clone(),
            }
        })
        .collect();

    // Newest first; deterministic tie-break on id so equal timestamps order stably.
    nodes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));

    // Active Meilenstein = the newest promoted Stand *on the active line* (lane 0). Diverging
    // Zweige carry their own Meilensteine on the node but must not steal the version bar. We
    // pick the node so the version label and its Art come from the same Meilenstein.
    let active_node = nodes
        .iter()
        .filter(|n| n.on_active)
        .find(|n| n.milestone.is_some());
    let active_milestone = active_node.and_then(|n| n.milestone.clone());
    let active_milestone_art = active_node.and_then(|n| n.milestone_art);

    VersionGraph {
        nodes,
        active_milestone,
        active_milestone_art,
        offloaded_archive: snapshot.offloaded_archive.clone(),
        active_branch: layout.active_branch.clone(),
        lane_count: layout.lane_count.max(1),
    }
}

/// Per-Stand lane assignment, derived purely from the parent edges and the observed line
/// tips. Lane `0` is the trunk (the active line). A Stand belongs to the **lowest-indexed**
/// line that can reach it (through any parent path): shared history stays on the trunk and
/// only the Stände unique to a diverging Zweig land on that Zweig's own lane.
struct LaneLayout {
    /// commit id -> lane index.
    lane: HashMap<String, usize>,
    /// lane index -> Zweig name (label shown once at the lane's tip); lane 0 may be unnamed.
    labels: Vec<Option<String>>,
    active_branch: Option<String>,
    lane_count: usize,
}

impl LaneLayout {
    fn lane_of(&self, id: &str) -> usize {
        self.lane.get(id).copied().unwrap_or(0)
    }

    /// The Zweig name labelling `id`'s lane. `None` for the trunk (lane 0); the UI decides
    /// where on the lane to draw it (it draws it once, at the lane's tip).
    fn label_of(&self, id: &str) -> Option<String> {
        let lane = self.lane_of(id);
        if lane == 0 {
            return None;
        }
        self.labels.get(lane).cloned().flatten()
    }

    fn compute(snapshot: &RepoSnapshot) -> LaneLayout {
        // Order the lines: the active line first (trunk, lane 0), the rest by name so the
        // layout is deterministic. With zero or one line everything collapses to lane 0.
        let mut ordered: Vec<&BranchFact> = snapshot.branches.iter().collect();
        let active = snapshot
            .active_branch
            .clone()
            .filter(|a| snapshot.branches.iter().any(|b| &b.name == a));
        ordered.sort_by(|a, b| {
            let a_active = active.as_deref() == Some(a.name.as_str());
            let b_active = active.as_deref() == Some(b.name.as_str());
            b_active.cmp(&a_active).then_with(|| a.name.cmp(&b.name))
        });

        let parents: HashMap<&str, &[String]> = snapshot
            .commits
            .iter()
            .map(|c| (c.id.as_str(), c.parents.as_slice()))
            .collect();

        // For each line, in trunk-first order, claim every still-unclaimed ancestor of its
        // tip. Because the trunk is processed first, shared Stände stay on lane 0 and only
        // the Stände unique to a later line fall to that line's lane.
        let mut lane: HashMap<String, usize> = HashMap::new();
        let mut labels: Vec<Option<String>> = Vec::new();
        for (idx, branch) in ordered.iter().enumerate() {
            labels.push(if idx == 0 { None } else { Some(branch.name.clone()) });
            let mut stack = vec![branch.tip.clone()];
            let mut seen: BTreeSet<String> = BTreeSet::new();
            while let Some(id) = stack.pop() {
                if !seen.insert(id.clone()) {
                    continue;
                }
                lane.entry(id.clone()).or_insert(idx);
                if let Some(ps) = parents.get(id.as_str()) {
                    for p in ps.iter() {
                        stack.push(p.clone());
                    }
                }
            }
        }

        let lane_count = if ordered.is_empty() { 1 } else { ordered.len() };

        LaneLayout {
            lane,
            labels,
            active_branch: active,
            lane_count,
        }
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

    /// A commit with explicit parents, for branch/lane tests.
    fn commit_p(id: &str, ts: &str, parents: &[&str]) -> CommitFact {
        CommitFact {
            id: id.to_string(),
            parents: parents.iter().map(|s| s.to_string()).collect(),
            message: format!("auto: x, {ts}"),
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
            branches: vec![],
            active_branch: None,
        };
        let g = project_graph(&snap);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"], "newest-first by timestamp");
        assert_eq!(g.active_milestone, None, "no Meilenstein yet");
        assert!(g.nodes.iter().all(|n| n.milestone.is_none()));
        // No branch facts => one linear lane, every Stand on the active line.
        assert_eq!(g.lane_count, 1);
        assert!(g.nodes.iter().all(|n| n.lane == 0 && n.on_active));
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
                    art: MilestoneArt::Freigabe,
                },
                MilestoneFact {
                    commit_id: "b".into(),
                    version: "v0.2".into(),
                    has_notes: true,
                    art: MilestoneArt::Prototyp,
                },
            ],
            offloaded: vec![],
            offloaded_archive: None,
            branches: vec![],
            active_branch: None,
        };
        let g = project_graph(&snap);

        // The newest Meilenstein wins the version bar, and its Art rides along with it.
        assert_eq!(g.active_milestone.as_deref(), Some("v0.2"));
        assert_eq!(g.active_milestone_art, Some(MilestoneArt::Prototyp));

        // b and a carry their milestone label + Art; c does not.
        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(node("b").milestone.as_deref(), Some("v0.2"));
        assert!(node("b").has_notes);
        assert_eq!(node("b").milestone_art, Some(MilestoneArt::Prototyp));
        assert_eq!(node("a").milestone.as_deref(), Some("v0.1"));
        assert_eq!(node("a").milestone_art, Some(MilestoneArt::Freigabe));
        assert_eq!(node("c").milestone, None);
        assert_eq!(node("c").milestone_art, None);
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
                art: MilestoneArt::Prototyp,
            }],
            offloaded: vec!["old".into()],
            offloaded_archive: Some("2025-11".into()),
            branches: vec![],
            active_branch: None,
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
            branches: vec![],
            active_branch: None,
        };
        let g = project_graph(&snap);
        assert!(g.nodes.is_empty());
        assert_eq!(g.active_milestone, None);
        assert_eq!(g.lane_count, 1);
    }

    #[test]
    fn an_external_zweig_lands_on_its_own_lane_and_the_active_line_stays_marked() {
        // Trunk a<-b<-c on `main`; an external Zweig `gehaeuse-v2` branched off b with d, e.
        //   a -- b -- c        (main, active)
        //         \-- d -- e   (gehaeuse-v2, created outside the tool)
        let snap = RepoSnapshot {
            commits: vec![
                commit_p("a", "2026-05-30T09:00:00Z", &[]),
                commit_p("b", "2026-05-30T10:00:00Z", &["a"]),
                commit_p("c", "2026-05-30T11:00:00Z", &["b"]),
                commit_p("d", "2026-05-30T10:30:00Z", &["b"]),
                commit_p("e", "2026-05-30T12:00:00Z", &["d"]),
            ],
            milestones: vec![],
            offloaded: vec![],
            offloaded_archive: None,
            branches: vec![
                BranchFact { name: "main".into(), tip: "c".into() },
                BranchFact { name: "gehaeuse-v2".into(), tip: "e".into() },
            ],
            active_branch: Some("main".into()),
        };
        let g = project_graph(&snap);

        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();

        // Two lines => two lanes; shared history (a, b) stays on the trunk.
        assert_eq!(g.lane_count, 2);
        assert_eq!(g.active_branch.as_deref(), Some("main"));
        for id in ["a", "b", "c"] {
            assert_eq!(node(id).lane, 0, "{id} on trunk");
            assert!(node(id).on_active, "{id} on the active line");
            assert_eq!(node(id).branch, None);
        }
        // The external Zweig's unique Stände get their own, non-trunk lane and carry its name.
        for id in ["d", "e"] {
            assert_eq!(node(id).lane, 1, "{id} on the Zweig lane");
            assert!(!node(id).on_active, "{id} not on the active line");
            assert_eq!(node(id).branch.as_deref(), Some("gehaeuse-v2"));
        }

        // Parent links survive the projection so the UI can draw fork/Zusammenführung
        // connectors: c folgt auf b, the Zweig's d folgt auf the shared b, e folgt auf d.
        assert_eq!(node("a").parents, Vec::<String>::new(), "root has no predecessor");
        assert_eq!(node("c").parents, vec!["b".to_string()]);
        assert_eq!(node("d").parents, vec!["b".to_string()], "Zweig forks off the shared b");
        assert_eq!(node("e").parents, vec!["d".to_string()]);
    }

    #[test]
    fn the_active_line_is_the_trunk_even_when_it_is_not_named_first() {
        // Same shape, but the user is *on* the external Zweig: it must become the trunk.
        let snap = RepoSnapshot {
            commits: vec![
                commit_p("a", "2026-05-30T09:00:00Z", &[]),
                commit_p("b", "2026-05-30T10:00:00Z", &["a"]),
                commit_p("c", "2026-05-30T11:00:00Z", &["b"]),
                commit_p("d", "2026-05-30T10:30:00Z", &["b"]),
                commit_p("e", "2026-05-30T12:00:00Z", &["d"]),
            ],
            milestones: vec![],
            offloaded: vec![],
            offloaded_archive: None,
            branches: vec![
                BranchFact { name: "main".into(), tip: "c".into() },
                BranchFact { name: "gehaeuse-v2".into(), tip: "e".into() },
            ],
            active_branch: Some("gehaeuse-v2".into()),
        };
        let g = project_graph(&snap);
        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();

        // gehaeuse-v2 is active => its line is the trunk (lane 0); main's unique `c` diverges.
        assert!(node("e").on_active && node("e").lane == 0);
        assert!(node("d").on_active && node("d").lane == 0);
        assert!(node("a").on_active && node("a").lane == 0, "shared history on trunk");
        assert_eq!(node("c").lane, 1, "main's unique Stand on its own lane");
        assert!(!node("c").on_active);
        assert_eq!(node("c").branch.as_deref(), Some("main"));
    }

    #[test]
    fn a_new_meilenstein_defaults_to_prototyp() {
        // E42: the default Art is the lax Prototyp — tagging is frictionless.
        assert_eq!(MilestoneArt::default(), MilestoneArt::Prototyp);
        assert!(!MilestoneArt::Prototyp.is_write_protected());
        assert!(MilestoneArt::Freigabe.is_write_protected());
    }

    #[test]
    fn toggle_is_a_two_state_reversible_machine() {
        // table: current Art -> Art after the toggle. Prototyp→Freigabe is „Releasen",
        // Freigabe→Prototyp is the deliberate reversible „Un-Release" (E42).
        let cases = [
            (MilestoneArt::Prototyp, MilestoneArt::Freigabe),
            (MilestoneArt::Freigabe, MilestoneArt::Prototyp),
        ];
        for (current, expected) in cases {
            assert_eq!(toggle_milestone_art(current), expected, "from {current:?}");
        }
        // Two toggles return to the start — the act is fully reversible.
        for start in [MilestoneArt::Prototyp, MilestoneArt::Freigabe] {
            assert_eq!(toggle_milestone_art(toggle_milestone_art(start)), start);
        }
    }

    #[test]
    fn art_tokens_round_trip_and_default_when_unknown() {
        // table: token written/read for the per-tag `_plm` store. Unknown/empty => Prototyp.
        let known = [
            ("prototyp", MilestoneArt::Prototyp),
            ("freigabe", MilestoneArt::Freigabe),
        ];
        for (token, art) in known {
            assert_eq!(MilestoneArt::from_token(token), art);
            assert_eq!(art.as_token(), token);
        }
        // A tag with no recorded Art is lax (E42), never an error.
        for unknown in ["", "  ", "release", "garbage"] {
            assert_eq!(MilestoneArt::from_token(unknown), MilestoneArt::Prototyp);
        }
    }
}
