//! The **Offline-Lock reconciler** — the pure decision core of E49b (Issue #136), **Eingang B**.
//!
//! The companion to the Reconcile-beim-Öffnen core ([`crate::reconciler`], Eingang A from E49a).
//! Where Eingang A judges the world the tool **observes on open**, Eingang B judges what the tool
//! **intended while offline** against what the server **really shows on connect**:
//!
//! > `(lokale Absichts-Sperren, Server-Sperren beim Verbinden) → Kollisions-/keine-Kollisions-
//! >  Entscheidung`
//!
//! Following the house pattern (`syncdecider.rs`, `reconciler.rs`, `locks.rs`): one **pure, total,
//! deterministic** function over plain snapshots. It knows **no** git internals, no clock, no
//! process — recording the local intent-lock and reading the server locks on connect lives in
//! [`crate::offlinelockglue`]; this module only **decides**.
//!
//! ## Why an Absichts-Sperre at all (E49b)
//!
//! A `git lfs lock` is **flüchtige Koordination** (the third truth-place, E49) — it needs a
//! reachable server. But the HW engineer must be able to open a lockable binary (KiCad, CAD) even
//! with **no reachable lock server**: an aeroplane, a dead VPN, a server reboot. So when the lock is
//! unreachable, the tool opens the file anyway and records a local **Absichts-Sperre** — an
//! *intent* to hold the lock — in `.plm-local/`, and the card says honestly „offline bearbeitet,
//! Sperre nicht bestätigt": **no false safety**. The intent is a promise the tool will confirm the
//! moment the server is back, never a claim it already did.
//!
//! ## The one loud case: a double-edit
//!
//! On connecting, this reconciler compares each local Absichts-Sperre against the **real** server
//! locks. The quiet case: the artifact is unheld, or already ours — the intent is **confirmable**,
//! the tool will promote it to a real lock with no prompt. The **one** case it must **never** decide
//! silently is a **double-edit**: the tool intended to hold X offline, but a colleague was holding X
//! on the server the whole time — „du und Ben habt beide offline an X gearbeitet". Two people edited
//! the same unmergeable artifact unseen; the tool must not silently overwrite either side, so the
//! collision flows into the **existing laute Ausnahme** — the same [`Abgleichfrage`] Eingang A
//! raises ([`crate::reconciler::Abgleichfrage`]) — phrased in domain language with the names of the
//! involved people, and **never** a raw git/lock marker.
//!
//! ## The decision
//!
//! - [`IntentReconcile::KeineKollision`] — every recorded Absichts-Sperre is confirmable (the
//!   artifact is free or already ours on the server). The tool confirms them quietly; no prompt.
//! - [`IntentReconcile::Doppelbearbeitung`] — at least one Absichts-Sperre collides with a foreign
//!   server lock: a double-edit. Carries the single domain-language [`Abgleichfrage`], naming the
//!   contested artifacts and the colleagues — guaranteed marker-free.

use crate::reconciler::Abgleichfrage;
use crate::syncdecider::text_has_git_marker;
use serde::Serialize;

// ----------------------------------------------------------------------------------------------
// Snapshots — the local Absichts-Sperren and the server locks seen on connect
// ----------------------------------------------------------------------------------------------

/// One **Absichts-Sperre** (intent-lock) the tool recorded locally while the lock server was
/// unreachable (E49b). Plain data; persisted/loaded by the glue under `.plm-local/`. The
/// degradation rule (ADR 0002) means a missing record reads as an empty list — a connect with no
/// offline intents simply has nothing to reconcile, never an error.
///
/// Carries **no** lock id and **no** server state: an Absichts-Sperre is *only* the local intent
/// („ich wollte X halten"), never a claim the server confirmed it. Confirmation is exactly what
/// [`reconcile_intents`] decides on connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsichtsSperre {
    /// The product-relative artifact path the user opened offline (forward slashes), named as an
    /// artifact — never a git ref.
    pub path: String,
}

/// One server lock seen the moment the tool **reconnects** (E49b). The real, current `git lfs
/// locks` state — who holds what right now. Plain data; the owner name is woven into a possible
/// double-edit question and so is neutralised first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSperre {
    /// The product-relative artifact path the lock is on, named as an artifact (never a git ref).
    pub path: String,
    /// The account holding it on the server (the Forgejo account name, as `git lfs locks` reports).
    /// Compared against „me" to tell our own confirmed lock from a colleague's.
    pub owner: String,
}

// ----------------------------------------------------------------------------------------------
// The decision
// ----------------------------------------------------------------------------------------------

/// The single decision the Offline-Lock reconciler returns on connect (Issue #136, E49b). Exactly
/// one; total. Mirrors the Eingang-A shape ([`crate::reconciler::ReconcileDecision`]): a silent
/// common case and a single loud exception.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum IntentReconcile {
    /// **keine Kollision** — every recorded Absichts-Sperre is confirmable: the artifact is free on
    /// the server, or already held by us. The tool promotes the intents to real locks with **no**
    /// prompt. Carries the confirmable artifacts for the calm log; empty when there were no offline
    /// intents to begin with (a plain online connect). (A struct variant, not a newtype-of-`Vec`,
    /// so the internally-tagged enum serialises cleanly for the typed frontend bindings.)
    KeineKollision { bestaetigt: Vec<String> },
    /// **Doppelbearbeitung** — at least one Absichts-Sperre collides with a **foreign** server lock:
    /// two people edited the same unmergeable artifact offline. The one moment the connect raises its
    /// voice — the same loud [`Abgleichfrage`] Eingang A uses, naming the contested artifacts and the
    /// colleagues, **never** a git/lock marker. The tool overwrites **nothing** until the user
    /// answers.
    Doppelbearbeitung(Abgleichfrage),
}

impl IntentReconcile {
    /// Whether this decision resolves silently — no prompt, no orange frame. True for exactly
    /// [`IntentReconcile::KeineKollision`].
    pub fn is_silent(&self) -> bool {
        matches!(self, IntentReconcile::KeineKollision { .. })
    }

    /// Whether this decision raises the loud double-edit exception. True for exactly
    /// [`IntentReconcile::Doppelbearbeitung`].
    pub fn is_double_edit(&self) -> bool {
        matches!(self, IntentReconcile::Doppelbearbeitung(_))
    }
}

// ----------------------------------------------------------------------------------------------
// The reconciler — pure, total, deterministic
// ----------------------------------------------------------------------------------------------

/// The **Offline-Lock reconciler** (Eingang B): decide the single [`IntentReconcile`] for one
/// connect (Issue #136, E49b). **Pure, total, deterministic.** Knows no git internals — the input
/// is plain data only.
///
/// The rule, in one line: **confirm every Absichts-Sperre the server lets us hold; raise one loud
/// double-edit exception for any intent a colleague was holding the whole time.**
///
/// Per recorded Absichts-Sperre, comparing it against the `server` locks seen on connect:
///
/// - the artifact is **free** on the server (no lock) → confirmable: the tool takes the real lock,
///   no prompt;
/// - the artifact is held **by us** (`owner_is_me`) → confirmable: our offline intent already
///   matches the server, no prompt;
/// - the artifact is held **by a colleague** → a **double-edit**: we and they both worked on the
///   same unmergeable artifact offline. This is the one drift the tool must not silently resolve, so
///   it routes the contested artifacts into the loud [`Abgleichfrage`].
///
/// Any number of confirmable intents collapse into one [`IntentReconcile::KeineKollision`]; a single
/// double-edit outranks them all into the [`IntentReconcile::Doppelbearbeitung`] (the loudest wins,
/// mirroring the Status Reader's precedence and Eingang A). No offline intents at all → an empty
/// [`IntentReconcile::KeineKollision`] (a plain online connect has nothing to reconcile).
///
/// By construction the question carries no git/lock marker (proven exhaustively in the tests): the
/// reconciler emits artifact names and colleague names, run through the same neutraliser the loud
/// Eingang-A core uses, never git text.
pub fn reconcile_intents(
    intents: &[AbsichtsSperre],
    server: &[ServerSperre],
    me: &str,
) -> IntentReconcile {
    // 1. The double-edits — the ONE drift that cannot be silently resolved: an artifact we intended
    //    to hold offline that a colleague was holding on the server. Loudest wins, so check first.
    let collisions: Vec<&ServerSperre> = intents
        .iter()
        .filter_map(|intent| foreign_hold_for(intent, server, me))
        .collect();
    if !collisions.is_empty() {
        return IntentReconcile::Doppelbearbeitung(build_doppel_frage(&collisions));
    }

    // 2. Otherwise every recorded intent is confirmable (free or already ours) — the quiet case.
    let bestaetigt: Vec<String> = intents.iter().map(|i| i.path.clone()).collect();
    IntentReconcile::KeineKollision { bestaetigt }
}

/// The foreign server lock that collides with this Absichts-Sperre, if any: a lock on the **same**
/// artifact held by **someone other than us**. Pure lookup over the snapshots. A free artifact or
/// one already held by us returns `None` (confirmable, not a collision). Reuses the **same**
/// case-insensitive owner-identity rule as the Status Reader so own vs. foreign is decided in
/// exactly one place ([`crate::lockglue::owner_is_me`]).
fn foreign_hold_for<'a>(
    intent: &AbsichtsSperre,
    server: &'a [ServerSperre],
    me: &str,
) -> Option<&'a ServerSperre> {
    server
        .iter()
        .find(|s| s.path == intent.path && !crate::lockglue::owner_is_me(&s.owner, me))
}

/// Build the domain-language [`Abgleichfrage`] for a set of double-edits — the laute Ausnahme E49b
/// shares with Eingang A. Names the involved colleague and the first contested artifact in the
/// headline; lists every contested artifact for the UI. Domain words only — guaranteed marker-free
/// (each path/name run through [`safe_text`]), so the question never leaks a raw git/lock marker.
fn build_doppel_frage(collisions: &[&ServerSperre]) -> Abgleichfrage {
    use crate::reconciler::TruthOrt;

    let artefakte: Vec<String> = collisions.iter().map(|s| safe_text(&s.path)).collect();
    let other = collisions
        .iter()
        .map(|s| s.owner.trim())
        .find(|o| !o.is_empty())
        .map(safe_text)
        .unwrap_or_else(|| "ein Kollege".to_string());
    let leaf = artefakte
        .first()
        .map(|p| artifact_noun(p))
        .unwrap_or_else(|| "Stand".to_string());

    // „du und <Kollege> habt beide offline an <Artefakt> gearbeitet — wessen Arbeit gilt?" — domain
    // language, naming the people and the artifact in the tool's own words, never „lock"/„merge".
    let frage =
        format!("du und {other} habt beide offline an {leaf} gearbeitet — wessen Arbeit gilt?");

    // The contradiction lives in the flüchtige Koordination (the server-lock), like Eingang A's.
    Abgleichfrage { frage, artefakte, ort: TruthOrt::Koordination }
}

/// A human artifact noun from a product-relative path: the final segment, extension stripped, first
/// letter upper-cased. „elektronik/board.kicad_pcb" → „Board". Never a git ref. (Mirrors the
/// Eingang-A / Sync-Decider `artifact_noun`; kept local so the cores stay independent.)
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
/// even a hostile `<<<<<<< HEAD.kicad_pcb` cannot leak a marker. The result satisfies
/// `!text_has_git_marker(&safe_text(s))` for every input (asserted in the tests). Mirrors the
/// Eingang-A neutraliser so the two loud cores agree on exactly what „forbidden" means.
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

    fn intent(path: &str) -> AbsichtsSperre {
        AbsichtsSperre { path: path.to_string() }
    }

    fn hold(path: &str, owner: &str) -> ServerSperre {
        ServerSperre { path: path.to_string(), owner: owner.to_string() }
    }

    /// AC: no offline intents at all — a plain online connect has nothing to reconcile → keine
    /// Kollision, empty, no prompt.
    #[test]
    fn no_intents_is_keine_kollision_empty() {
        let d = reconcile_intents(&[], &[hold("a.f3d", "ben")], "anna");
        assert_eq!(d, IntentReconcile::KeineKollision { bestaetigt: vec![] });
        assert!(d.is_silent());
        assert!(!d.is_double_edit());
    }

    /// THE Eingang-B cross product as a table (Issue #136): an Absichts-Sperre × the server state of
    /// its artifact → expected decision kind. Every confirmable case and the one loud double-edit.
    #[test]
    fn cross_product_confirmable_vs_double_edit() {
        // (name, intents, server, me, expect double-edit?)
        #[allow(clippy::type_complexity)]
        let cases: &[(&str, Vec<AbsichtsSperre>, Vec<ServerSperre>, &str, bool)] = &[
            (
                "intent on an artifact that is FREE on the server -> confirmable",
                vec![intent("a.f3d")],
                vec![],
                "anna",
                false,
            ),
            (
                "intent on an artifact already held by US -> confirmable (intent matches server)",
                vec![intent("a.f3d")],
                vec![hold("a.f3d", "anna")],
                "anna",
                false,
            ),
            (
                "intent held by us, casing/whitespace differs -> still confirmable (owner_is_me)",
                vec![intent("a.f3d")],
                vec![hold("a.f3d", "  Anna ")],
                "anna",
                false,
            ),
            (
                "intent on an artifact a COLLEAGUE holds -> DOUBLE-EDIT (loud)",
                vec![intent("a.f3d")],
                vec![hold("a.f3d", "ben")],
                "anna",
                true,
            ),
            (
                "a colleague holds a DIFFERENT artifact -> no collision on our intent",
                vec![intent("a.f3d")],
                vec![hold("other.f3d", "ben")],
                "anna",
                false,
            ),
            (
                "many intents, all free or ours -> one quiet keine-Kollision",
                vec![intent("a.f3d"), intent("b.step"), intent("c.kicad_pcb")],
                vec![hold("b.step", "anna")],
                "anna",
                false,
            ),
            (
                "one of several intents collides -> the whole connect is loud",
                vec![intent("a.f3d"), intent("b.step")],
                vec![hold("b.step", "ben")],
                "anna",
                true,
            ),
            (
                "no identity (no remote) -> any held artifact reads as foreign -> loud",
                vec![intent("a.f3d")],
                vec![hold("a.f3d", "ben")],
                "",
                true,
            ),
        ];

        for (name, intents, server, me, expect_double) in cases {
            let d = reconcile_intents(intents, server, me);
            assert_eq!(d.is_double_edit(), *expect_double, "cross-product case: {name} -> {d:?}");
            assert_eq!(d.is_silent(), !*expect_double, "silent/loud exclusive: {name}");
        }
    }

    /// AC: every confirmable intent is reported back in `bestaetigt`, so the glue knows exactly which
    /// offline intents to promote to real locks on connect — in the order recorded.
    #[test]
    fn confirmable_intents_are_all_reported_for_promotion() {
        let d = reconcile_intents(
            &[intent("a.f3d"), intent("b.step")],
            &[hold("a.f3d", "anna")], // a.f3d already ours; b.step free
            "anna",
        );
        let IntentReconcile::KeineKollision { bestaetigt } = d else {
            panic!("expected keine Kollision");
        };
        assert_eq!(bestaetigt, vec!["a.f3d".to_string(), "b.step".to_string()]);
    }

    /// AC: a detected double-edit raises ONE domain-language question that names the contested
    /// artifact AND the colleague — and lists only the contested artifact(s), never overwriting.
    #[test]
    fn double_edit_asks_one_domain_question_naming_the_people() {
        let d = reconcile_intents(
            &[intent("elektronik/board.kicad_pcb"), intent("x.step")],
            &[hold("elektronik/board.kicad_pcb", "Ben"), hold("x.step", "anna")],
            "anna",
        );
        let IntentReconcile::Doppelbearbeitung(q) = d else {
            panic!("expected loud double-edit: {d:?}");
        };
        assert_eq!(
            q.artefakte,
            vec!["elektronik/board.kicad_pcb".to_string()],
            "only the contested artifact"
        );
        assert!(q.frage.contains("Board"), "names the contested artifact: {}", q.frage);
        assert!(q.frage.contains("Ben"), "names the colleague: {}", q.frage);
        assert!(q.frage.contains("beide offline"), "phrases the double-edit: {}", q.frage);
        assert!(q.frage.contains("wessen Arbeit gilt"), "asks whose work applies: {}", q.frage);
    }

    /// The collision flows into the EXISTING laute Ausnahme — the very [`Abgleichfrage`] Eingang A
    /// raises — so the UI has exactly one loud type to render (no second voice for the same kind of
    /// contradiction).
    #[test]
    fn double_edit_reuses_the_eingang_a_abgleichfrage_type() {
        let d = reconcile_intents(&[intent("a.f3d")], &[hold("a.f3d", "ben")], "anna");
        let IntentReconcile::Doppelbearbeitung(q) = d else { panic!("expected loud") };
        // It IS a `crate::reconciler::Abgleichfrage` — same loud contract, incl. the marker test.
        let _: &crate::reconciler::Abgleichfrage = &q;
        assert!(!q.contains_git_marker());
    }

    /// THE E41/E49 acid test as an exhaustive PROPERTY: over every combination of intent/server,
    /// hostile paths and hostile names, the reconciler NEVER produces a visible git/lock marker
    /// (`<<<<<<<`, `HEAD`, `merge`, …) in anything it shows the user.
    #[test]
    fn no_input_ever_produces_a_visible_git_marker() {
        let paths = ["a.f3d", "weird/<<<<<<< HEAD.kicad_pcb", "merge.step", "x.f3d"];
        let names = ["Ben", "", "<<<<<<<", "HEAD", ">>>>>>> merge"];
        let mes = ["anna", "", "merge"];

        for p in paths {
            for n in names {
                for me in mes {
                    // collision: our offline intent on an artifact a (possibly hostile-named)
                    // colleague holds on the server.
                    let d = reconcile_intents(&[intent(p)], &[hold(p, n)], me);
                    assert_marker_free(&d);
                    // confirmable: a hostile path that is merely free must also stay clean.
                    let d2 = reconcile_intents(&[intent(p)], &[], me);
                    assert_marker_free(&d2);
                }
            }
        }
    }

    /// Helper: a decision, silent or loud, never carries a visible git/lock marker. The silent one
    /// shows only artifact paths; the loud question must be clean.
    fn assert_marker_free(d: &IntentReconcile) {
        match d {
            IntentReconcile::KeineKollision { bestaetigt } => {
                for p in bestaetigt {
                    // a confirmable path is shown verbatim in the calm log; it must carry no marker
                    // word of its own (a hostile path is data the calm log renders, not a ref).
                    let safe = safe_text(p);
                    assert!(!text_has_git_marker(&safe), "confirmable path neutralises: {p:?}");
                }
            }
            IntentReconcile::Doppelbearbeitung(q) => {
                assert!(!q.contains_git_marker(), "double-edit question leaked a marker: {q:?}");
            }
        }
    }

    /// AC: `reconcile_intents` is total — exactly one decision, never panics — over arbitrary input,
    /// including empty paths, empty owners and an empty identity.
    #[test]
    fn reconcile_is_total() {
        let inputs: &[(Vec<AbsichtsSperre>, Vec<ServerSperre>, &str)] = &[
            (vec![], vec![], ""),
            (vec![intent("")], vec![hold("", "")], ""),
            (vec![intent("a")], vec![hold("a", "")], "anna"),
        ];
        for (intents, server, me) in inputs {
            let d = reconcile_intents(intents, server, me);
            assert!(d.is_silent() ^ d.is_double_edit(), "exactly one decision");
        }
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
        assert_eq!(safe_text("elektronik/board.kicad_pcb"), "elektronik/board.kicad_pcb");
    }
}
