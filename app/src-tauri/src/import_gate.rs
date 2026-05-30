//! Import Gate for the dangerous import branch (Issue #7, E38).
//!
//! When turning an existing folder into a product, three repo facts decide how safe the
//! import is. This module is the **pure, total, deterministic** decision: it maps the
//! repo-state cross-product to exactly one [`GateDecision`]. It does **no** I/O — probing the
//! real repo lives in [`crate::import`]; the destructive `git lfs migrate` glue is isolated
//! from this decision so the dangerous operation can never be reached except through it.
//!
//! The three facts (the "cross-product" the PRD asks us to cover):
//! - `has_history` — the folder already is a git repo with commits.
//! - `shared_clones_exist` — other people already hold clones (a remote / shared upstream).
//! - `giant_binaries_in_history` — heavy binaries are **already committed** into history.
//!
//! The decision, in order of safety (E38):
//! 1. Shared clones present ⇒ **always** [`GateDecision::Refuse`]. `git lfs migrate import`
//!    rewrites history; rewriting shared history poisons everyone else's clones. We never do
//!    it once the repo is shared — no matter what else is true. (The shared-clones ⇒ refuse
//!    invariant is exhaustively asserted below.)
//! 2. Fresh / unshared **and** giant binaries already in history ⇒
//!    [`GateDecision::MigrateBehindGate`]: the only situation where the history rewrite is
//!    offered — and even then only behind the bewusste "Historie anfassen" confirmation.
//! 3. Otherwise ⇒ [`GateDecision::CleanInit`]: the clean, non-destructive path from #3
//!    (`git init` if needed → track → first commit). No history is touched.

use serde::Serialize;

/// The three observable facts about a folder's git state. Pure data; gather it via
/// [`crate::import`] (which does the I/O) and feed it to [`decide_import`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoState {
    /// The folder already is a git repo carrying commits.
    pub has_history: bool,
    /// Other clones of this repo exist (a configured remote / shared upstream).
    pub shared_clones_exist: bool,
    /// Heavy binaries are already committed into the git history.
    pub giant_binaries_in_history: bool,
}

/// Exactly one outcome of the Import Gate. Serialised kebab-case for the UI.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GateDecision {
    /// Safe: clean, non-destructive import (Issue #3 path). Delegated to `import_folder`.
    CleanInit,
    /// Dangerous but allowed: heavy binaries sit in history of a fresh/unshared repo, so a
    /// `git lfs migrate` history rewrite is offered — only behind the "Historie anfassen" gate.
    MigrateBehindGate,
    /// Forbidden: the repo is shared, so rewriting history would poison others' clones (E38).
    Refuse,
}

impl GateDecision {
    /// Whether this decision permits the destructive `git lfs migrate` history rewrite.
    /// Only [`GateDecision::MigrateBehindGate`] does — and even then only after the user
    /// crosses the "Historie anfassen" confirmation.
    pub fn allows_history_rewrite(self) -> bool {
        matches!(self, GateDecision::MigrateBehindGate)
    }
}

/// The Import Gate: pure, total decision over the repo-state cross-product (E38).
///
/// See the module docs for the full rationale. In short:
/// - shared clones ⇒ always [`GateDecision::Refuse`] (never poison others' clones);
/// - else fresh/unshared **and** giant binaries already in history ⇒
///   [`GateDecision::MigrateBehindGate`];
/// - else ⇒ [`GateDecision::CleanInit`].
pub fn decide_import(state: RepoState) -> GateDecision {
    // 1. Shared clones first, unconditionally: rewriting shared history is never allowed.
    if state.shared_clones_exist {
        return GateDecision::Refuse;
    }
    // 2. Fresh / unshared with heavy binaries already committed *into history*: offer the
    //    gated migrate. There must be history to rewrite — giants "in history" without any
    //    history is a contradictory input, handled as the clean path below.
    if state.has_history && state.giant_binaries_in_history {
        return GateDecision::MigrateBehindGate;
    }
    // 3. Everything else is the clean, non-destructive import.
    GateDecision::CleanInit
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All eight points of the boolean cross-product, with the expected decision.
    /// (has_history, shared_clones_exist, giant_binaries_in_history) -> GateDecision.
    const CROSS_PRODUCT: &[(bool, bool, bool, GateDecision)] = &[
        // --- fresh folder, never in git ---
        (false, false, false, GateDecision::CleanInit), // empty/new folder -> clean init
        // giant binaries "in history" is impossible without history; treated as clean init.
        (false, false, true, GateDecision::CleanInit),
        // shared clones but no local history (an empty clone of a shared repo) -> refuse.
        (false, true, false, GateDecision::Refuse),
        (false, true, true, GateDecision::Refuse),
        // --- already has history, unshared (solo) ---
        (true, false, false, GateDecision::CleanInit), // history but no giants -> clean init
        (true, false, true, GateDecision::MigrateBehindGate), // THE dangerous-but-allowed case
        // --- already has history, shared ---
        (true, true, false, GateDecision::Refuse),
        (true, true, true, GateDecision::Refuse), // giants AND shared -> still refuse (E38)
    ];

    #[test]
    fn decide_import_covers_the_full_cross_product() {
        for (has_history, shared, giants, expected) in CROSS_PRODUCT {
            let state = RepoState {
                has_history: *has_history,
                shared_clones_exist: *shared,
                giant_binaries_in_history: *giants,
            };
            assert_eq!(
                decide_import(state),
                *expected,
                "decide_import({state:?})"
            );
        }
    }

    /// Property (exhaustive over the finite boolean domain): if shared clones exist, the gate
    /// ALWAYS refuses — regardless of history or giant binaries. Never poison others' clones.
    #[test]
    fn shared_clones_always_refuse() {
        for has_history in [false, true] {
            for giants in [false, true] {
                let state = RepoState {
                    has_history,
                    shared_clones_exist: true,
                    giant_binaries_in_history: giants,
                };
                assert_eq!(
                    decide_import(state),
                    GateDecision::Refuse,
                    "shared clones must always refuse: {state:?}"
                );
            }
        }
    }

    /// Property: `migrate-behind-gate` is offered ONLY when fresh/unshared AND giants in
    /// history. Exhaustively: it appears at exactly one point of the cross-product.
    #[test]
    fn migrate_offered_only_when_unshared_with_giants_in_history() {
        let mut migrate_points = 0;
        for has_history in [false, true] {
            for shared in [false, true] {
                for giants in [false, true] {
                    let state = RepoState {
                        has_history,
                        shared_clones_exist: shared,
                        giant_binaries_in_history: giants,
                    };
                    if decide_import(state) == GateDecision::MigrateBehindGate {
                        migrate_points += 1;
                        // every migrate decision implies: unshared, with giants in history.
                        assert!(!shared, "migrate must never be offered on shared repos");
                        assert!(giants, "migrate requires giants already in history");
                        assert!(has_history, "migrate requires history to rewrite");
                    }
                }
            }
        }
        assert_eq!(migrate_points, 1, "migrate-behind-gate has exactly one cross-product point");
    }

    /// Only `migrate-behind-gate` permits the destructive history rewrite.
    #[test]
    fn only_migrate_allows_history_rewrite() {
        assert!(GateDecision::MigrateBehindGate.allows_history_rewrite());
        assert!(!GateDecision::CleanInit.allows_history_rewrite());
        assert!(!GateDecision::Refuse.allows_history_rewrite());
    }

    /// Totality: every input yields a decision, never panics (the type system already makes
    /// this total, but assert it stays so as the function evolves).
    #[test]
    fn decide_import_is_total() {
        for has_history in [false, true] {
            for shared in [false, true] {
                for giants in [false, true] {
                    let _ = decide_import(RepoState {
                        has_history,
                        shared_clones_exist: shared,
                        giant_binaries_in_history: giants,
                    });
                }
            }
        }
    }
}
