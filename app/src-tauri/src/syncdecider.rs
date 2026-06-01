//! The **Sync Decider** — the pure decision core of the stiller Sync (Issue #11, E41).
//!
//! Following the house pattern (`classifier.rs`, `warden.rs`, `setup.rs`): one **pure, total,
//! deterministic** function over a plain snapshot. It knows **no** git internals, no clock, no
//! process — the side-effecting pull/merge glue lives in [`crate::syncglue`]; this module only
//! decides. Snapshot in, exactly **one** [`SyncDecision`] out.
//!
//! The daily net-sync runs **silently** (E41): pull on open/idle, Sicherungs-Push laufend,
//! Freigabe-Push at the Revision (the push types are the #9 Warden's job — this module never
//! re-decides them). When the local and remote stands have **diverged**, the Sync Decider judges
//! whether that divergence can be merged without ever showing the user a thing:
//!
//! - [`SyncDecision::SilentMerge`] — every diverged path is **free, mergeable text** (the #3
//!   [`Bucket::TextMergeable`]). git can merge it; the user is **never prompted** (E41: "still im
//!   Hintergrund").
//! - [`SyncDecision::LoudException`] — at least one diverged path is **unmergeable**: a binary
//!   ([`Bucket::BinaryUnmergeable`]) **or** a KiCad nominal-text source
//!   ([`Bucket::NominalTextUnmergeable`]). A merge there would **silently corrupt the file**
//!   (E41's „Missing („-Korruption), so the stiller Sync **stops** and asks **one** question in
//!   the tool's own domain language — never a git conflict marker.
//!
//! This is the load-bearing rule of E41: *a merge must NEVER silently corrupt a file.* The
//! decider is the single gate that guarantees it — any unmergeable touch is routed loud, always.
//!
//! The loud exception is the **single** place the UI raises its voice (the rationed orange frame,
//! E41 / ui-stilbeschreibung). It carries a [`LoudQuestion`] phrased in domain language
//! ("dein und X' <Stand> widersprechen sich — welcher gilt?"), and it carries **no git markers**.

use crate::classifier::Bucket;
use serde::{Deserialize, Serialize};

/// One diverged path: where local and remote stands disagree, plus its mergeability bucket
/// (from the #3 [`crate::classifier`]) and who, if anyone, holds the remote side. Plain data —
/// the decider takes the bucket as an already-decided fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivergedPath {
    /// The product-relative path that diverged (forward slashes). Shown to the user *as a
    /// domain artifact name*, never as a git ref.
    pub path: String,
    /// The mergeability bucket of this path, decided upstream by [`crate::classifier::classify`].
    pub bucket: Bucket,
    /// The colleague whose remote stand this path diverged against, if known. Used only to
    /// phrase the loud question ("dein und X' Gehäuse-Stand …"); `None` falls back to "der
    /// andere".
    pub other: Option<String>,
}

impl DivergedPath {
    /// Whether this diverged path is **unmergeable** — a binary or a KiCad nominal-text source.
    /// Both buckets that [`Bucket::is_lockable`] reports must route loud, because a git merge
    /// would touch (and so corrupt) the file. The single fact the whole E41 guarantee rests on.
    pub fn is_unmergeable(&self) -> bool {
        self.bucket.is_lockable()
    }
}

/// The single decision the Sync Decider returns for a divergence. Exactly one; total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SyncDecision {
    /// **Silent merge** — every diverged path is free, mergeable text. git merges it with **no
    /// user prompt** (E41). The daily vocabulary stays "aktuell / gesichert"; nothing surfaces.
    SilentMerge,
    /// **Loud exception** — at least one unmergeable touch. The stiller Sync **stops** and asks
    /// one domain-language question; the orange frame, the single attention moment. Carries the
    /// question and the offending paths — but **never** a git conflict marker.
    LoudException(LoudQuestion),
}

impl SyncDecision {
    /// Whether this decision merges silently with no user prompt. True for exactly
    /// [`SyncDecision::SilentMerge`].
    pub fn is_silent(&self) -> bool {
        matches!(self, SyncDecision::SilentMerge)
    }

    /// Whether this decision raises the loud exception (the single orange-frame moment). True for
    /// exactly [`SyncDecision::LoudException`].
    pub fn is_loud(&self) -> bool {
        matches!(self, SyncDecision::LoudException(_))
    }
}

/// Which stand the user must choose between in a loud exception. The whole question is framed in
/// domain language; this enum never leaks a git ref or marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StandChoice {
    /// Keep my local stand for the contested artifact.
    Mine,
    /// Take the colleague's stand for the contested artifact.
    Theirs,
}

/// The domain-language question shown in the single orange-frame loud exception. It names the
/// contested artifact in the tool's own words and offers the two stands to choose from. It holds
/// **no** git conflict marker by construction (see [`LoudQuestion::contains_git_marker`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoudQuestion {
    /// The one-line question, e.g. „dein und Bens Gehäuse-Stand widersprechen sich — welcher
    /// gilt?". Domain language only.
    pub frage: String,
    /// The contested artifact's product-relative paths, named as artifacts (never as git refs).
    /// At least one; these are exactly the unmergeable diverged paths.
    pub artefakte: Vec<String>,
    /// The two stands to choose between, each with its domain label.
    pub optionen: Vec<StandOption>,
}

/// One choosable stand in the loud question, with a domain label. No git wording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandOption {
    pub choice: StandChoice,
    /// The label, e.g. "mein Stand" / "Bens Stand". Domain language.
    pub label: String,
}

impl LoudQuestion {
    /// **The E41 acid test**, usable from any test: this question carries **no** visible git
    /// conflict marker — none of `<<<<<<<`, `=======`, `>>>>>>>`, nor the words push/pull/merge.
    /// True means a marker is present (and so the invariant would be broken).
    pub fn contains_git_marker(&self) -> bool {
        let mut texts: Vec<&str> = vec![self.frage.as_str()];
        texts.extend(self.artefakte.iter().map(String::as_str));
        texts.extend(self.optionen.iter().map(|o| o.label.as_str()));
        texts.iter().any(|t| text_has_git_marker(t))
    }
}

/// The classic merge-conflict markers git writes into a file.
const CONFLICT_MARKERS: &[&str] = &["<<<<<<<", "=======", ">>>>>>>", "|||||||"];

/// Raw git verbs/nouns the daily vocabulary must never show (E41). Matched as standalone tokens,
/// case-folded, so they cannot ride inside an innocent domain word.
const GIT_WORDS: &[&str] = &[
    "push", "pull", "merge", "commit", "rebase", "conflict", "konflikt", "head",
];

/// Whether a single rendered string contains a visible git conflict marker or raw git verb.
/// Centralized so both the production code and the tests share one definition of "forbidden".
pub fn text_has_git_marker(text: &str) -> bool {
    if CONFLICT_MARKERS.iter().any(|m| text.contains(m)) {
        return true;
    }
    let lower = text.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|tok| GIT_WORDS.contains(&tok))
}

/// The colleague's name to use in the loud question, falling back to a neutral domain phrase
/// when no name is known. Never a git identity.
fn other_name(diverged: &[DivergedPath]) -> String {
    let raw = diverged
        .iter()
        .find_map(|d| d.other.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "der andere".to_string());
    // A colleague name is data too — strip any marker char so a hostile name can never leak a
    // git conflict marker into the loud question (the E41 acid test covers this).
    strip_marker_chars(&raw)
}

/// Render the domain-language question for a set of contested (unmergeable) artifacts. Names the
/// first artifact's leaf in the question; lists all contested artifacts for the UI. Domain words
/// only — guaranteed marker-free (the path is run through [`safe_artifact_path`]).
fn build_question(other: &str, unmergeable: &[&DivergedPath]) -> LoudQuestion {
    let artefakte: Vec<String> =
        unmergeable.iter().map(|d| strip_marker_chars(&d.path)).collect();
    // A friendly artifact noun for the headline: the leaf file name without extension.
    let leaf = artefakte
        .first()
        .map(|p| artifact_noun(p))
        .unwrap_or_else(|| "Stand".to_string());

    // „dein und Xs <Artefakt>-Stand widersprechen sich — welcher gilt?" — domain language.
    let frage = format!("dein und {other}s {leaf}-Stand widersprechen sich — welcher gilt?");

    LoudQuestion {
        frage,
        artefakte,
        optionen: vec![
            StandOption { choice: StandChoice::Mine, label: "mein Stand".to_string() },
            StandOption { choice: StandChoice::Theirs, label: format!("{other}s Stand") },
        ],
    }
}

/// A human artifact noun from a product-relative path: the final segment, extension stripped,
/// first letter upper-cased. „mechanik/gehaeuse.f3d" → "Gehaeuse". Never a git ref.
fn artifact_noun(path: &str) -> String {
    let leaf = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = leaf.rsplit_once('.').map(|(s, _)| s).unwrap_or(leaf);
    let stem = if stem.is_empty() { leaf } else { stem };
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Stand".to_string(),
    }
}

/// Neutralize any user-supplied data (an artifact path or a colleague name) before it is woven
/// into the loud question, so it can never carry a visible git marker to the user (E41). Two
/// guards, matching exactly what [`text_has_git_marker`] forbids:
/// 1. the marker chars `<`, `=`, `>`, `|` are dropped (collapsing a `<<<<<<<` run to nothing);
/// 2. any standalone reserved git word (`HEAD`, `merge`, …) is replaced with a neutral `·`.
///
/// A path/name is data, not a ref — a hostile `<<<<<<< HEAD.f3d` must not let a marker through.
/// The E41 acid test exercises exactly such hostile input. The result satisfies
/// `!text_has_git_marker(&strip_marker_chars(s))` for every input (asserted in tests).
fn strip_marker_chars(s: &str) -> String {
    // 1. drop the four marker characters outright.
    let no_chars: String = s.chars().filter(|c| !matches!(c, '<' | '=' | '>' | '|')).collect();
    // 2. replace any standalone reserved git token, preserving the separators between tokens.
    let mut out = String::with_capacity(no_chars.len());
    let mut tok = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        if !tok.is_empty() {
            if GIT_WORDS.contains(&tok.to_lowercase().as_str()) {
                out.push('·');
            } else {
                out.push_str(tok);
            }
            tok.clear();
        }
    };
    for c in no_chars.chars() {
        if c.is_alphanumeric() {
            tok.push(c);
        } else {
            flush(&mut tok, &mut out);
            out.push(c);
        }
    }
    flush(&mut tok, &mut out);
    out
}

/// The **Sync Decider**: decide the single [`SyncDecision`] for a divergence. **Pure, total,
/// deterministic.** Knows no git internals — the input is plain data only.
///
/// The rule (E41), in one line: **any unmergeable touch routes loud; otherwise merge silent.**
///
/// - An **empty** divergence (nothing actually diverged) is a [`SyncDecision::SilentMerge`] — a
///   fast-forward / no-op pull the user never sees.
/// - If **every** diverged path is [`Bucket::TextMergeable`] → [`SyncDecision::SilentMerge`]:
///   git merges free text with no prompt.
/// - If **any** diverged path is unmergeable (binary OR KiCad nominal-text) →
///   [`SyncDecision::LoudException`]: a merge there could corrupt the file, so the stiller Sync
///   stops and asks one domain-language question over exactly the unmergeable paths.
///
/// By construction the loud question carries no git conflict marker (proven exhaustively in the
/// tests): the decider never emits git text — it emits the artifact name and the two stands.
pub fn decide_sync(diverged: &[DivergedPath]) -> SyncDecision {
    // Collect every unmergeable touch. ANY one of them forces the loud exception (E41): a merge
    // must never silently corrupt a file.
    let unmergeable: Vec<&DivergedPath> = diverged.iter().filter(|d| d.is_unmergeable()).collect();

    if unmergeable.is_empty() {
        // Free, mergeable divergence (or nothing diverged at all): git merges silently, no prompt.
        return SyncDecision::SilentMerge;
    }

    let other = other_name(diverged);
    SyncDecision::LoudException(build_question(&other, &unmergeable))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dp(path: &str, bucket: Bucket) -> DivergedPath {
        DivergedPath { path: path.to_string(), bucket, other: Some("Ben".to_string()) }
    }

    /// Every bucket, as a single-path divergence, with the expected routing per kind.
    fn one_of_each_bucket() -> Vec<(DivergedPath, bool /* expect loud */)> {
        vec![
            (dp("firmware/main.c", Bucket::TextMergeable), false),
            (dp("docs/README.md", Bucket::TextMergeable), false),
            (dp("mechanik/gehaeuse.f3d", Bucket::BinaryUnmergeable), true),
            (dp("render.png", Bucket::BinaryUnmergeable), true),
            (dp("elektronik/board.kicad_pcb", Bucket::NominalTextUnmergeable), true),
            (dp("elektronik/board.kicad_sch", Bucket::NominalTextUnmergeable), true),
        ]
    }

    /// AC: free, mergeable divergence resolves via silent-merge with no user prompt — and an
    /// empty divergence is the silent no-op pull.
    #[test]
    fn free_mergeable_divergence_is_silent_with_no_prompt() {
        // nothing diverged -> silent
        assert!(decide_sync(&[]).is_silent(), "empty divergence is a silent no-op");

        // a pile of only mergeable text -> silent, no loud question at all
        let only_text = vec![
            dp("firmware/main.c", Bucket::TextMergeable),
            dp("docs/README.md", Bucket::TextMergeable),
            dp("bom.csv", Bucket::TextMergeable),
        ];
        let d = decide_sync(&only_text);
        assert!(d.is_silent(), "only-text divergence must silent-merge: {d:?}");
        assert!(!d.is_loud(), "silent merge raises no loud exception");
    }

    /// AC: any unmergeable touch (binary OR KiCad) routes to loud-exception. Asserted per bucket
    /// AND for a mixed pile where a single unmergeable file is hidden among free text.
    #[test]
    fn any_unmergeable_touch_routes_loud() {
        for (path, expect_loud) in one_of_each_bucket() {
            let d = decide_sync(std::slice::from_ref(&path));
            assert_eq!(d.is_loud(), expect_loud, "routing for {path:?}");
        }

        // one KiCad source hidden among lots of free text -> still loud (a merge would corrupt it)
        let mixed = vec![
            dp("firmware/main.c", Bucket::TextMergeable),
            dp("docs/a.md", Bucket::TextMergeable),
            dp("elektronik/board.kicad_pcb", Bucket::NominalTextUnmergeable),
            dp("bom.csv", Bucket::TextMergeable),
        ];
        let d = decide_sync(&mixed);
        assert!(d.is_loud(), "a single unmergeable touch forces loud: {d:?}");

        // one binary hidden among free text -> loud
        let mixed_bin = vec![
            dp("docs/a.md", Bucket::TextMergeable),
            dp("mechanik/gehaeuse.f3d", Bucket::BinaryUnmergeable),
        ];
        assert!(decide_sync(&mixed_bin).is_loud(), "a single binary touch forces loud");
    }

    /// AC: the loud exception only lists the *unmergeable* contested artifacts — the free text
    /// that rode along is merged silently and never named in the question.
    #[test]
    fn loud_question_lists_only_the_unmergeable_artifacts() {
        let mixed = vec![
            dp("firmware/main.c", Bucket::TextMergeable),
            dp("mechanik/gehaeuse.f3d", Bucket::BinaryUnmergeable),
            dp("elektronik/board.kicad_pcb", Bucket::NominalTextUnmergeable),
            dp("docs/a.md", Bucket::TextMergeable),
        ];
        match decide_sync(&mixed) {
            SyncDecision::LoudException(q) => {
                assert_eq!(
                    q.artefakte,
                    vec![
                        "mechanik/gehaeuse.f3d".to_string(),
                        "elektronik/board.kicad_pcb".to_string()
                    ],
                    "only the two unmergeable paths are contested"
                );
                assert!(
                    !q.artefakte.iter().any(|a| a.ends_with(".c") || a.ends_with(".md")),
                    "free text is never named in the loud question"
                );
            }
            other => panic!("expected loud exception, got {other:?}"),
        }
    }

    /// AC: the loud question asks in DOMAIN language with two clear stands — and names the
    /// colleague when known, falling back to a neutral domain phrase otherwise.
    #[test]
    fn loud_question_is_domain_language_with_two_stands() {
        let d = decide_sync(&[dp("mechanik/gehaeuse.f3d", Bucket::BinaryUnmergeable)]);
        let SyncDecision::LoudException(q) = d else { panic!("expected loud") };
        assert!(q.frage.contains("Gehaeuse"), "names the contested artifact: {}", q.frage);
        assert!(q.frage.contains("Ben"), "names the colleague: {}", q.frage);
        assert!(q.frage.contains("welcher gilt"), "asks whose stand applies: {}", q.frage);
        assert_eq!(q.optionen.len(), 2, "exactly two stands to choose from");
        assert_eq!(q.optionen[0].choice, StandChoice::Mine);
        assert_eq!(q.optionen[1].choice, StandChoice::Theirs);
        assert_eq!(q.optionen[0].label, "mein Stand");
        assert_eq!(q.optionen[1].label, "Bens Stand");

        // no colleague name known -> neutral domain phrase, still no git wording
        let anon = DivergedPath {
            path: "render.png".to_string(),
            bucket: Bucket::BinaryUnmergeable,
            other: None,
        };
        let SyncDecision::LoudException(q2) = decide_sync(&[anon]) else { panic!("loud") };
        assert!(q2.frage.contains("der andere"), "neutral fallback: {}", q2.frage);
        assert!(!q2.contains_git_marker());
    }

    /// THE E41 acid test, as an **exhaustive property test**: over every combination of buckets,
    /// hostile paths and hostile names, the Sync Decider NEVER produces a visible git conflict
    /// marker or raw git verb in anything it shows the user. A merge never surfaces git text.
    #[test]
    fn no_input_ever_produces_a_visible_git_conflict_marker() {
        let buckets = [
            Bucket::TextMergeable,
            Bucket::BinaryUnmergeable,
            Bucket::NominalTextUnmergeable,
        ];
        // hostile paths / names that LOOK like git markers must not be able to leak one through
        let paths = [
            "a.c",
            "mechanik/gehaeuse.f3d",
            "elektronik/board.kicad_pcb",
            "weird/<<<<<<< HEAD.f3d",
            "merge.kicad_sch",
            "x.png",
        ];
        let others = [
            None,
            Some("Ben".to_string()),
            Some(String::new()),
            Some("<<<<<<<".to_string()),
        ];

        // enumerate every single-path divergence across buckets/paths/names …
        for b in buckets {
            for p in paths {
                for o in &others {
                    let dpv = DivergedPath { path: p.to_string(), bucket: b, other: o.clone() };
                    assert_marker_free(&decide_sync(&[dpv]));
                }
            }
        }

        // … and a multi-path pile, including hostile names
        let pile = vec![
            DivergedPath {
                path: "<<<<<<<.f3d".into(),
                bucket: Bucket::BinaryUnmergeable,
                other: Some(">>>>>>>".into()),
            },
            DivergedPath { path: "ok.c".into(), bucket: Bucket::TextMergeable, other: None },
            DivergedPath {
                path: "merge.kicad_pcb".into(),
                bucket: Bucket::NominalTextUnmergeable,
                other: Some("Ben".into()),
            },
        ];
        assert_marker_free(&decide_sync(&pile));
    }

    /// Helper: a decision, silent or loud, never carries a visible git marker. A silent merge
    /// shows nothing at all; a loud exception's rendered question must be clean.
    fn assert_marker_free(d: &SyncDecision) {
        match d {
            SyncDecision::SilentMerge => { /* shows nothing — trivially marker-free */ }
            SyncDecision::LoudException(q) => {
                assert!(!q.contains_git_marker(), "loud question leaked a git marker: {q:?}");
            }
        }
    }

    /// AC: decide_sync is total and returns exactly one decision — never panics — over arbitrary
    /// input, including empty paths and odd buckets.
    #[test]
    fn decide_sync_is_total() {
        let inputs: Vec<Vec<DivergedPath>> = vec![
            vec![],
            vec![DivergedPath { path: String::new(), bucket: Bucket::TextMergeable, other: None }],
            vec![DivergedPath { path: ".".into(), bucket: Bucket::BinaryUnmergeable, other: None }],
            vec![DivergedPath {
                path: "a/b/".into(),
                bucket: Bucket::NominalTextUnmergeable,
                other: None,
            }],
        ];
        for inp in inputs {
            let d = decide_sync(&inp);
            assert!(d.is_silent() ^ d.is_loud(), "exactly one decision for {inp:?}");
        }
    }

    /// The marker-detector itself is correct: it catches the markers and raw git verbs, but does
    /// NOT false-positive on innocent domain words.
    #[test]
    fn git_marker_detector_catches_markers_and_verbs_only() {
        assert!(text_has_git_marker("<<<<<<< HEAD"));
        assert!(text_has_git_marker("======="));
        assert!(text_has_git_marker(">>>>>>> theirs"));
        assert!(text_has_git_marker("bitte einmal pushen? push"));
        assert!(text_has_git_marker("MERGE the branch"));
        // innocent domain text passes
        assert!(!text_has_git_marker(
            "dein und Bens Gehäuse-Stand widersprechen sich — welcher gilt?"
        ));
        assert!(!text_has_git_marker("mein Stand"));
        assert!(!text_has_git_marker("Gehaeuse"));
    }

    /// The neutralizer makes ANY hostile data marker-free — the property the loud question relies
    /// on. Both marker chars and reserved git words are stripped; innocent names survive.
    #[test]
    fn strip_marker_chars_neutralizes_any_hostile_data() {
        for hostile in [
            "<<<<<<< HEAD",
            ">>>>>>> theirs",
            "=======",
            "weird/<<<<<<< HEAD.f3d",
            "merge.kicad_pcb",
            "push",
            "|||||||",
        ] {
            assert!(
                !text_has_git_marker(&strip_marker_chars(hostile)),
                "strip failed to neutralize {hostile:?} -> {:?}",
                strip_marker_chars(hostile)
            );
        }
        // an innocent name is left intact
        assert_eq!(strip_marker_chars("Ben"), "Ben");
        assert_eq!(strip_marker_chars("mechanik/gehaeuse.f3d"), "mechanik/gehaeuse.f3d");
    }
}
