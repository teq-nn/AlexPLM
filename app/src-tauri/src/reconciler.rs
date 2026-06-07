//! The **Reconciler** — the pure decision core of the Reconcile-beim-Öffnen (Issue #129, E49a).
//!
//! Following the house pattern (`syncdecider.rs`, `import_gate.rs`, `locks.rs`): one **pure, total,
//! deterministic** function over a plain snapshot. It knows **no** git internals, no clock, no
//! process — reading the real observed state and carrying out the silent catch-up live in
//! [`crate::reconcileglue`]; this module only **decides**. Snapshot in, exactly one
//! [`ReconcileDecision`] out.
//!
//! ## Why a reconcile at all (E49)
//!
//! Between two openings the world moves **outside** the tool: a colleague worked on another
//! machine, a `west update` ran in the terminal, a save happened while the tool was closed. The
//! `_plm` store remembers the **last-seen** state — what the tool last knew. On open the tool
//! observes the **real** state and compares the two. If work happened outside, the tool **silently
//! catches up** (re-seeds its memory) instead of letting the user work on a stale picture. A
//! divergence that is *not* silently resolvable is named in **domain language** — never raw git
//! text (the E41 line this core also obeys).
//!
//! ## The three truth-places, named honestly (E49)
//!
//! The tool never pretends there is one store. There are exactly **three** places a fact can live,
//! and the tool names each for what it is — the [`TruthOrt`]:
//!
//! - **Disk = Inhalt** ([`TruthOrt::Inhalt`]): the worktree files on disk. The *content*. The one
//!   place the user's actual bytes live.
//! - **git = Verlauf** ([`TruthOrt::Verlauf`]): the commit history. The *history* — durable,
//!   shared, the record of how the content got here.
//! - **Server-Sperren = flüchtige Koordination** ([`TruthOrt::Koordination`]): the `git lfs locks`
//!   on the server. *Ephemeral coordination* only — who is currently holding an unmergeable file.
//!   It is never content and never history; it evaporates when a lock is released.
//!
//! The whole point of E49 is that these three drift **independently**, so a reconcile must read all
//! three and judge each honestly. The Reconciler decides, per truth-place, whether the drift is a
//! silent catch-up or a real contradiction the user must hear about.
//!
//! ## The decision
//!
//! - [`ReconcileDecision::Aktuell`] — nothing drifted; the memory already matched reality. A no-op
//!   open the user never sees.
//! - [`ReconcileDecision::StillAufgeholt`] — work happened outside, but it is **silently
//!   resolvable**: the tool catches up (the glue re-seeds the `_plm` memory) with **no prompt**.
//!   Carries the named catch-ups for the log, in domain language.
//! - [`ReconcileDecision::Abgleichfrage`] — a drift that is **not** silently resolvable (a contested
//!   ownership the tool cannot decide for the user). The single moment the open raises its voice:
//!   one domain-language question, **never** a git conflict marker.

use crate::syncdecider::text_has_git_marker;
use serde::Serialize;

// ----------------------------------------------------------------------------------------------
// The three truth-places — named honestly (E49)
// ----------------------------------------------------------------------------------------------

/// One of the three places a fact can honestly live (E49). The tool never collapses them into a
/// single fictional store — disk, git and the server-locks each drift on their own, so the
/// reconcile names each catch-up by the place it happened in.
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TruthOrt {
    /// **Disk = Inhalt.** The worktree files — the user's actual bytes. The *content*.
    Inhalt,
    /// **git = Verlauf.** The commit history — durable, shared. The *history*.
    Verlauf,
    /// **Server-Sperren = flüchtige Koordination.** The `git lfs locks` — *ephemeral coordination*
    /// of who currently holds an unmergeable file. Never content, never history.
    Koordination,
}

impl TruthOrt {
    /// The honest domain noun for this truth-place, woven into the catch-up log and the question.
    /// Names the place for *what it is* — never a git ref or marker.
    pub fn label(self) -> &'static str {
        match self {
            TruthOrt::Inhalt => "Inhalt",
            TruthOrt::Verlauf => "Verlauf",
            TruthOrt::Koordination => "Koordination",
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Snapshots — the last-seen `_plm` memory and the real observed state
// ----------------------------------------------------------------------------------------------

/// What the `_plm` store last remembered about the three truth-places — the **last-seen** state
/// from the previous session (Issue #129, E49a). Plain data; persisted/loaded by the glue. The
/// degradation rule (ADR 0002) means a missing memory reads as [`PlmMemory::default`] — a first
/// open then simply sees everything as "freshly learned", never an error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlmMemory {
    /// The git history tip (`HEAD` commit id) the tool last saw. Empty before the first open.
    pub last_head: String,
    /// Whether the worktree was clean (no uncommitted content) when the tool last looked.
    pub was_clean: bool,
    /// The unmergeable artifacts the tool last knew **we ourselves** held a server-lock on. Used to
    /// notice a lock that changed hands while we were away (the one drift we cannot silently decide).
    pub own_locks: Vec<String>,
}

/// The **real observed state** of the three truth-places, read fresh on open (Issue #129, E49a).
/// Side-effecting collection lives in [`crate::reconcileglue`]; this is the pure input the
/// Reconciler compares against the [`PlmMemory`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservedState {
    /// The git history tip (`HEAD`) as it really is now. May have advanced outside the tool
    /// (terminal `commit`, `west`, a pull on another machine).
    pub head: String,
    /// Whether the worktree is clean right now. A dirty worktree the memory thought was clean means
    /// content changed on disk outside the tool.
    pub clean: bool,
    /// The artifacts **we** currently hold a server-lock on, per `git lfs locks`.
    pub own_locks: Vec<String>,
    /// The artifacts a **colleague** now holds a server-lock on, with the owner's name. Used to
    /// detect a lock that was ours last-seen but is foreign now — a contested ownership.
    pub foreign_locks: Vec<ForeignHold>,
}

/// One foreign server-lock the open observed: an unmergeable artifact a colleague currently holds.
/// Plain data; the owner name is woven into a possible question and so is neutralised first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignHold {
    /// The product-relative artifact path the colleague holds, named as an artifact (never a ref).
    pub path: String,
    /// The colleague holding it, for the domain-language question. May be empty/unknown.
    pub owner: String,
}

// ----------------------------------------------------------------------------------------------
// The decision
// ----------------------------------------------------------------------------------------------

/// One named catch-up the silent reconcile carried out, in domain language (E49). Says *which*
/// truth-place drifted and *what* the tool quietly learned — for the calm log only; the user is
/// never prompted. Carries **no** git marker by construction.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Aufholung {
    /// The truth-place that drifted (disk / git / server-locks), named honestly.
    pub ort: TruthOrt,
    /// The one-line domain-language note, e.g. „Verlauf ist außerhalb weitergelaufen — aufgeholt".
    pub notiz: String,
}

/// The single decision the Reconciler returns. Exactly one; total.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReconcileDecision {
    /// **aktuell** — the `_plm` memory already matched reality. Nothing drifted; nothing to do and
    /// nothing shown. The quiet common case.
    Aktuell,
    /// **still aufgeholt** — work happened outside, but every drift is silently resolvable: the tool
    /// catches up (the glue re-seeds the `_plm` memory) with **no** user prompt (E49). Carries the
    /// named catch-ups for the calm log. (A struct variant, not a newtype-of-`Vec`, so the
    /// internally-tagged enum serialises cleanly for the typed frontend bindings.)
    StillAufgeholt { aufholungen: Vec<Aufholung> },
    /// **Abgleichfrage** — a drift that cannot be silently resolved (a contested ownership). The one
    /// moment the open raises its voice: a single domain-language question, never a git marker.
    Abgleichfrage(Abgleichfrage),
}

impl ReconcileDecision {
    /// Whether this decision resolves silently — no prompt, no orange frame. True for both
    /// [`ReconcileDecision::Aktuell`] and [`ReconcileDecision::StillAufgeholt`] (E49: the tool
    /// "silently catches up").
    pub fn is_silent(&self) -> bool {
        !matches!(self, ReconcileDecision::Abgleichfrage(_))
    }

    /// Whether this decision raises the single to-report question. True for exactly
    /// [`ReconcileDecision::Abgleichfrage`].
    pub fn is_to_report(&self) -> bool {
        matches!(self, ReconcileDecision::Abgleichfrage(_))
    }
}

/// The domain-language question shown when a drift is **not** silently resolvable (E49). Today the
/// one such drift is a contested ownership: an unmergeable artifact the tool last knew was *ours* is
/// now held by a colleague, so the tool cannot silently decide whose work continues. The question
/// names the artifact in the tool's own words and the contested truth-place — and holds **no** git
/// conflict marker by construction (see [`Abgleichfrage::contains_git_marker`]).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Abgleichfrage {
    /// The one-line question, e.g. „Bens Sperre liegt jetzt auf deinem Gehaeuse — wessen Arbeit
    /// gilt?". Domain language only.
    pub frage: String,
    /// The contested artifacts, named as artifacts (never git refs). At least one.
    pub artefakte: Vec<String>,
    /// The truth-place the contradiction lives in (always [`TruthOrt::Koordination`] today — a
    /// server-lock that changed hands).
    pub ort: TruthOrt,
}

impl Abgleichfrage {
    /// **The E41/E49 acid test**: this question carries **no** visible git conflict marker — none of
    /// `<<<<<<<`, `HEAD`, `merge`, … . True means a marker is present (and the invariant is broken).
    /// Reuses the single forbidden-text definition from [`crate::syncdecider`] so the two cores can
    /// never drift on what "forbidden" means.
    pub fn contains_git_marker(&self) -> bool {
        let mut texts: Vec<&str> = vec![self.frage.as_str(), self.ort.label()];
        texts.extend(self.artefakte.iter().map(String::as_str));
        texts.iter().any(|t| text_has_git_marker(t))
    }
}

// ----------------------------------------------------------------------------------------------
// The Reconciler — pure, total, deterministic
// ----------------------------------------------------------------------------------------------

/// The **Reconciler**: decide the single [`ReconcileDecision`] for one open (Issue #129, E49a).
/// **Pure, total, deterministic.** Knows no git internals — the input is plain data only.
///
/// The rule, in one line: **silently catch up every drift that is resolvable; raise one question
/// only for a contested ownership the tool cannot decide for the user.**
///
/// Per truth-place (E49), comparing the last-seen [`PlmMemory`] against the real [`ObservedState`]:
///
/// - **Verlauf (git):** `HEAD` advanced outside the tool (terminal/`west`/another machine) → a
///   silent catch-up. The history is durable and shared; the tool just re-learns the new tip. (A
///   genuinely diverged history is the **stiller Sync**'s job — [`crate::syncdecider`] — not this
///   open-time reconcile, which only catches the tool up to *observed* reality.)
/// - **Inhalt (disk):** the worktree is dirty but the memory thought it clean → a silent catch-up.
///   Content changed on disk while the tool was closed; the watcher/auto-commit takes it from here.
/// - **Koordination (server-locks):** an unmergeable artifact the tool last knew was **ours** is now
///   held by a **colleague** → this is the **one** drift that is *not* silently resolvable: two
///   people believe the contested artifact is theirs, and the tool must not silently pick. It raises
///   the [`ReconcileDecision::Abgleichfrage`]. A lock we simply *gained* or *lost* cleanly (no
///   contest) is a silent catch-up.
///
/// Any number of silent catch-ups across the three places collapse into one
/// [`ReconcileDecision::StillAufgeholt`]; a single contested ownership outranks them all into the
/// [`ReconcileDecision::Abgleichfrage`] (the loudest wins, mirroring the Status Reader's precedence).
/// Nothing drifted at all → [`ReconcileDecision::Aktuell`].
///
/// By construction the question carries no git conflict marker (proven exhaustively in the tests):
/// the Reconciler emits artifact names and truth-place labels, never git text.
pub fn reconcile(memory: &PlmMemory, observed: &ObservedState) -> ReconcileDecision {
    // 1. Koordination — the ONE drift that cannot be silently resolved: an artifact we last held is
    //    now held by a colleague. Loudest wins, so check it first and short-circuit (E49).
    let contested = contested_holds(memory, observed);
    if !contested.is_empty() {
        return ReconcileDecision::Abgleichfrage(build_frage(&contested));
    }

    // 2. The silently-resolvable catch-ups, one per drifted truth-place. Order is honest:
    //    Verlauf, then Inhalt, then Koordination — the durable history first.
    let mut aufholungen = Vec::new();

    if !observed.head.is_empty() && observed.head != memory.last_head {
        // git's history tip moved outside the tool — re-learn it, no prompt.
        aufholungen.push(Aufholung {
            ort: TruthOrt::Verlauf,
            notiz: "Verlauf ist außerhalb weitergelaufen — still aufgeholt".to_string(),
        });
    }

    if !observed.clean && memory.was_clean {
        // content changed on disk while the tool was closed — the watcher takes it from here.
        aufholungen.push(Aufholung {
            ort: TruthOrt::Inhalt,
            notiz: "Inhalt auf der Platte hat sich außerhalb geändert — still aufgeholt".to_string(),
        });
    }

    if coordination_drifted(memory, observed) {
        // server-locks changed cleanly (gained/released, no contest) — re-learn who holds what.
        aufholungen.push(Aufholung {
            ort: TruthOrt::Koordination,
            notiz: "Sperren auf dem Server haben sich geändert — still aufgeholt".to_string(),
        });
    }

    if aufholungen.is_empty() {
        ReconcileDecision::Aktuell
    } else {
        ReconcileDecision::StillAufgeholt { aufholungen }
    }
}

/// The artifacts that were **ours** last-seen but are **foreign** now — a contested ownership the
/// tool cannot silently decide. Pure set difference over the two snapshots.
fn contested_holds<'a>(memory: &PlmMemory, observed: &'a ObservedState) -> Vec<&'a ForeignHold> {
    observed
        .foreign_locks
        .iter()
        .filter(|f| memory.own_locks.iter().any(|own| own == &f.path))
        .collect()
}

/// Whether the coordination truth-place (server-locks) drifted in any **silently-resolvable** way:
/// the set of locks we hold changed, or some foreign lock appeared/vanished — but with **no**
/// contested ownership (that case is handled loud, above). Pure comparison.
fn coordination_drifted(memory: &PlmMemory, observed: &ObservedState) -> bool {
    let own_changed = !same_set(&memory.own_locks, &observed.own_locks);
    let any_foreign = !observed.foreign_locks.is_empty();
    own_changed || any_foreign
}

/// Whether two path lists hold the same set (order- and duplicate-insensitive). Pure helper.
fn same_set(a: &[String], b: &[String]) -> bool {
    a.iter().all(|x| b.contains(x)) && b.iter().all(|x| a.contains(x))
}

/// Build the domain-language [`Abgleichfrage`] for a set of contested holds. Names the colleague and
/// the first artifact in the headline; lists every contested artifact for the UI. Domain words only
/// — guaranteed marker-free (each path/name run through [`safe_text`]).
fn build_frage(contested: &[&ForeignHold]) -> Abgleichfrage {
    let artefakte: Vec<String> = contested.iter().map(|f| safe_text(&f.path)).collect();
    let other = contested
        .iter()
        .map(|f| f.owner.trim())
        .find(|o| !o.is_empty())
        .map(safe_text)
        .unwrap_or_else(|| "ein Kollege".to_string());
    let leaf = artefakte.first().map(|p| artifact_noun(p)).unwrap_or_else(|| "Stand".to_string());

    // „<Kollege>s Sperre liegt jetzt auf deinem <Artefakt> — wessen Arbeit gilt?" — domain language,
    // naming the flüchtige Koordination (the server-lock) in the tool's own words, never „lock".
    let frage =
        format!("{other}s Sperre liegt jetzt auf deinem {leaf} — wessen Arbeit gilt?");

    Abgleichfrage { frage, artefakte, ort: TruthOrt::Koordination }
}

/// A human artifact noun from a product-relative path: the final segment, extension stripped, first
/// letter upper-cased. „mechanik/gehaeuse.f3d" → „Gehaeuse". Never a git ref. (Mirrors the Sync
/// Decider's `artifact_noun`; kept local so the two cores stay independent.)
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

/// Neutralise any user-supplied data (an artifact path or a colleague name) before it is woven into
/// the question, so it can never carry a visible git marker to the user (E41/E49). Drops the four
/// conflict-marker characters and replaces any standalone reserved git word with a neutral `·`, so
/// even a hostile `<<<<<<< HEAD.f3d` cannot leak a marker. The result satisfies
/// `!text_has_git_marker(&safe_text(s))` for every input (asserted in the tests).
fn safe_text(s: &str) -> String {
    let no_chars: String = s.chars().filter(|c| !matches!(c, '<' | '=' | '>' | '|')).collect();
    let mut out = String::with_capacity(no_chars.len());
    let mut tok = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        if !tok.is_empty() {
            if text_has_git_marker(tok) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(head: &str, clean: bool, own: &[&str]) -> PlmMemory {
        PlmMemory {
            last_head: head.to_string(),
            was_clean: clean,
            own_locks: own.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn obs(head: &str, clean: bool, own: &[&str], foreign: &[(&str, &str)]) -> ObservedState {
        ObservedState {
            head: head.to_string(),
            clean,
            own_locks: own.iter().map(|s| s.to_string()).collect(),
            foreign_locks: foreign
                .iter()
                .map(|(p, o)| ForeignHold { path: p.to_string(), owner: o.to_string() })
                .collect(),
        }
    }

    /// AC: nothing drifted — the `_plm` memory already matches reality → aktuell, no prompt, no
    /// catch-up. The quiet common open.
    #[test]
    fn nothing_drifted_is_aktuell() {
        let m = mem("abc123", true, &["mechanik/gehaeuse.f3d"]);
        let o = obs("abc123", true, &["mechanik/gehaeuse.f3d"], &[]);
        assert_eq!(reconcile(&m, &o), ReconcileDecision::Aktuell);
        assert!(reconcile(&m, &o).is_silent());
        assert!(!reconcile(&m, &o).is_to_report());
    }

    /// THE divergence matrix as a table (Issue #129): last-seen `_plm` memory × observed state →
    /// expected decision kind. Every silently-resolvable drift and the one to-report drift covered.
    #[test]
    fn divergence_matrix_silent_vs_to_report() {
        // (name, memory, observed, expect to-report?)
        #[allow(clippy::type_complexity)]
        let cases: &[(&str, PlmMemory, ObservedState, bool)] = &[
            (
                "in sync -> silent (aktuell)",
                mem("h1", true, &[]),
                obs("h1", true, &[], &[]),
                false,
            ),
            (
                "git history moved outside -> silent catch-up",
                mem("h1", true, &[]),
                obs("h2", true, &[], &[]),
                false,
            ),
            (
                "disk dirtied while closed -> silent catch-up",
                mem("h1", true, &[]),
                obs("h1", false, &[], &[]),
                false,
            ),
            (
                "we cleanly gained a lock -> silent catch-up",
                mem("h1", true, &[]),
                obs("h1", true, &["a.f3d"], &[]),
                false,
            ),
            (
                "we cleanly released a lock -> silent catch-up",
                mem("h1", true, &["a.f3d"]),
                obs("h1", true, &[], &[]),
                false,
            ),
            (
                "a colleague holds a NEW (never-ours) artifact -> silent catch-up",
                mem("h1", true, &[]),
                obs("h1", true, &[], &[("b.step", "Ben")]),
                false,
            ),
            (
                "everything drifted silently at once -> still one silent catch-up",
                mem("h1", true, &["a.f3d"]),
                obs("h2", false, &["c.f3d"], &[("b.step", "Ben")]),
                false,
            ),
            (
                "OUR lock is now held by a colleague -> TO-REPORT (contested)",
                mem("h1", true, &["a.f3d"]),
                obs("h1", true, &[], &[("a.f3d", "Ben")]),
                true,
            ),
            (
                "contested ownership outranks other silent drifts -> still TO-REPORT",
                mem("h2", true, &["a.f3d"]),
                obs("h9", false, &[], &[("a.f3d", "Ben")]),
                true,
            ),
        ];

        for (name, m, o, expect_report) in cases {
            let d = reconcile(m, o);
            assert_eq!(d.is_to_report(), *expect_report, "matrix case: {name} -> {d:?}");
            assert_eq!(d.is_silent(), !*expect_report, "silent/loud exclusive: {name}");
        }
    }

    /// AC: a git history that ran ahead outside the tool catches up silently, naming the Verlauf
    /// truth-place honestly.
    #[test]
    fn history_outside_catches_up_naming_verlauf() {
        let d = reconcile(&mem("old", true, &[]), &obs("new", true, &[], &[]));
        let ReconcileDecision::StillAufgeholt { aufholungen: a } = d else { panic!("expected silent catch-up: {d:?}") };
        assert!(a.iter().any(|x| x.ort == TruthOrt::Verlauf), "names the git/Verlauf place: {a:?}");
        assert!(a.iter().any(|x| x.notiz.contains("Verlauf")));
    }

    /// AC: disk content that changed on disk outside the tool catches up silently, naming the
    /// Inhalt truth-place honestly.
    #[test]
    fn dirty_disk_catches_up_naming_inhalt() {
        let d = reconcile(&mem("h", true, &[]), &obs("h", false, &[], &[]));
        let ReconcileDecision::StillAufgeholt { aufholungen: a } = d else { panic!("expected silent catch-up") };
        assert!(a.iter().any(|x| x.ort == TruthOrt::Inhalt), "names the disk/Inhalt place: {a:?}");
    }

    /// AC: server-locks that drifted cleanly catch up silently, naming the Koordination truth-place.
    #[test]
    fn lock_drift_catches_up_naming_koordination() {
        let d = reconcile(&mem("h", true, &[]), &obs("h", true, &["x.f3d"], &[]));
        let ReconcileDecision::StillAufgeholt { aufholungen: a } = d else { panic!("expected silent catch-up") };
        assert!(a.iter().any(|x| x.ort == TruthOrt::Koordination), "names the lock place: {a:?}");
    }

    /// AC: a contested ownership (our last-seen lock now foreign) raises ONE domain-language
    /// question that names the artifact and the colleague — and lists only the contested artifact.
    #[test]
    fn contested_lock_asks_one_domain_question() {
        let d = reconcile(
            &mem("h", true, &["mechanik/gehaeuse.f3d", "x.step"]),
            &obs("h", true, &["x.step"], &[("mechanik/gehaeuse.f3d", "Ben")]),
        );
        let ReconcileDecision::Abgleichfrage(q) = d else { panic!("expected to-report question: {d:?}") };
        assert_eq!(q.artefakte, vec!["mechanik/gehaeuse.f3d".to_string()], "only the contested one");
        assert!(q.frage.contains("Gehaeuse"), "names the contested artifact: {}", q.frage);
        assert!(q.frage.contains("Ben"), "names the colleague: {}", q.frage);
        assert!(q.frage.contains("wessen Arbeit gilt"), "asks whose work applies: {}", q.frage);
        assert_eq!(q.ort, TruthOrt::Koordination, "the contradiction lives in the lock place");
    }

    /// The three truth-places are named honestly — disk=Inhalt, git=Verlauf,
    /// server-locks=Koordination — and the labels carry no git wording.
    #[test]
    fn three_truth_places_are_honestly_named() {
        assert_eq!(TruthOrt::Inhalt.label(), "Inhalt");
        assert_eq!(TruthOrt::Verlauf.label(), "Verlauf");
        assert_eq!(TruthOrt::Koordination.label(), "Koordination");
        for ort in [TruthOrt::Inhalt, TruthOrt::Verlauf, TruthOrt::Koordination] {
            assert!(!text_has_git_marker(ort.label()), "truth-place label is git-free: {ort:?}");
        }
    }

    /// THE E41/E49 acid test as an exhaustive PROPERTY: over every combination of memory/observed,
    /// hostile paths and hostile names, the Reconciler NEVER produces a visible git conflict marker
    /// (`<<<<<<<`, `HEAD`, `merge`, …) in anything it shows the user.
    #[test]
    fn no_input_ever_produces_a_visible_git_marker() {
        let paths = ["a.f3d", "weird/<<<<<<< HEAD.f3d", "merge.kicad_pcb", "x.step"];
        let names = ["Ben", "", "<<<<<<<", "HEAD", ">>>>>>> merge"];
        let heads = ["", "h1", "<<<<<<< HEAD"];

        for p in paths {
            for n in names {
                for h in heads {
                    // contested: our last-seen lock now held by a (possibly hostile-named) colleague
                    let m = mem(h, true, &[p]);
                    let o = obs("other", false, &[], &[(p, n)]);
                    assert_marker_free(&reconcile(&m, &o));
                    // silent: a hostile head/path that merely catches up must also stay clean
                    let m2 = mem(h, true, &[]);
                    let o2 = obs("other", false, &[p], &[]);
                    assert_marker_free(&reconcile(&m2, &o2));
                }
            }
        }
    }

    /// Helper: a decision, silent or to-report, never carries a visible git marker. The silent ones
    /// show only domain notes; the question must be clean.
    fn assert_marker_free(d: &ReconcileDecision) {
        match d {
            ReconcileDecision::Aktuell => {}
            ReconcileDecision::StillAufgeholt { aufholungen: a } => {
                for x in a {
                    assert!(!text_has_git_marker(&x.notiz), "catch-up note leaked a marker: {x:?}");
                    assert!(!text_has_git_marker(x.ort.label()));
                }
            }
            ReconcileDecision::Abgleichfrage(q) => {
                assert!(!q.contains_git_marker(), "question leaked a git marker: {q:?}");
            }
        }
    }

    /// AC: `reconcile` is total — exactly one decision, never panics — over arbitrary input,
    /// including empty heads, empty paths and odd lock sets.
    #[test]
    fn reconcile_is_total() {
        let inputs: &[(PlmMemory, ObservedState)] = &[
            (PlmMemory::default(), ObservedState::default()),
            (mem("", false, &[""]), obs("", true, &[""], &[("", "")])),
            (mem("h", true, &["a"]), obs("", false, &[], &[])),
        ];
        for (m, o) in inputs {
            let d = reconcile(m, o);
            assert!(d.is_silent() ^ d.is_to_report(), "exactly one decision for {m:?} / {o:?}");
        }
    }

    /// First open with an empty `_plm` memory (degraded/missing, ADR 0002): a real observed state is
    /// simply learned as a silent catch-up — never an error, never a question.
    #[test]
    fn first_open_with_empty_memory_silently_learns() {
        let d = reconcile(&PlmMemory::default(), &obs("h1", true, &["a.f3d"], &[]));
        assert!(d.is_silent(), "a first open must never raise a question: {d:?}");
        assert!(matches!(d, ReconcileDecision::StillAufgeholt { .. }), "it learns the world quietly");
    }

    /// The neutraliser makes ANY hostile data marker-free — the property the question relies on.
    #[test]
    fn safe_text_neutralises_hostile_data() {
        for hostile in ["<<<<<<< HEAD", ">>>>>>> theirs", "=======", "merge.kicad_pcb", "HEAD"] {
            assert!(
                !text_has_git_marker(&safe_text(hostile)),
                "safe_text failed to neutralise {hostile:?} -> {:?}",
                safe_text(hostile)
            );
        }
        assert_eq!(safe_text("Ben"), "Ben");
        assert_eq!(safe_text("mechanik/gehaeuse.f3d"), "mechanik/gehaeuse.f3d");
    }
}
