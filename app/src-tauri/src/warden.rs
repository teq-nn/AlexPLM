//! The **Lock Warden** — the safety-critical decision core of the two push types (Issue #9).
//!
//! This is the heart that carries the **Binär-Invariante** (E35, glossary):
//!
//! > *Eine gesperrte Binäränderung darf den geteilten Stand nicht erreichen, solange die
//! > Sperre gehalten wird.* — A locked binary change must NEVER reach the shared `main`
//! > stand while the lock is held.
//!
//! Following the house pattern (`classifier.rs`, `locks.rs`, `setup.rs`), the decision lives in
//! one **pure, total, deterministic** function over a plain [`WardenSnapshot`] — it knows **no**
//! git internals, no clock, no process. Snapshot in, exactly **one** [`WardenAction`] out. The
//! side-effecting git/LFS push glue that *carries out* the action is isolated in
//! [`crate::pushglue`]; this module only decides.
//!
//! The snapshot is the full cross-product of four axes:
//! - **path kind** — binary/unmergeable (lockable, E31) vs. mergeable text;
//! - **lock state** — held by me, held by someone else, or unlocked;
//! - **cleanliness** — is the path locally clean (committed, nothing open) or dirty;
//! - **checkpoint kind** — *laufend* (an ongoing intermediate checkpoint → Sicherungs-Push) vs.
//!   *Revision* (the explicit "ich bin fertig damit" release → Freigabe-Push).
//!
//! Two push types fall out of the invariant (E35 / glossary "Freigabe-Push vs. Sicherungs-Push"):
//! - **Sicherungs-Push** (laufend) — a *private* act: mirror local intermediate commits (incl. a
//!   half-finished binary under an active lock) to a **personal** backup ref/namespace. Backup
//!   yes, release no — it does **not** publish to the shared `main`.
//! - **Freigabe-Push** (Revision) — a *public* act: bring the finished binary to the shared
//!   `main` stand **and release the lock atomically** ("unlock at push", which the tool itself
//!   implements because `git lfs unlock` is a separate explicit command). Binary content reaches
//!   the LFS store **only** here (the bloat cap, E36).
//!
//! And the self-healing rule (E31/E35): at **every** checkpoint, auto-unlock any held lock whose
//! path is locally **clean** — never otherwise.

use serde::Serialize;

/// Whether the artifact at the path is an unmergeable binary (lockable, E31) or mergeable text.
/// Derived from the #3 classifier upstream; the Warden takes it as a plain fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathKind {
    /// An unmergeable binary (CAD/mesh/photo/KiCad) — lockable; the invariant guards these.
    Binary,
    /// Mergeable text (firmware, docs, BOM) — git merges it; never locked.
    Text,
}

/// The lock state of the path, as read back from `git lfs locks` (E37, single source of truth).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockState {
    /// We hold the lock on this path (our own coordination claim, E31).
    HeldByMe,
    /// Someone else holds the lock — not ours to push or release. The loud exception.
    HeldByOther,
    /// No lock anywhere on this path.
    Unlocked,
}

/// Whether the path is locally **clean** — committed and nothing open — or **dirty**.
/// "Clean" is the precondition for auto-unlock (E35: „committet, gepusht, keine offene
/// Bearbeitung"); a dirty path still has work the user has not let go of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cleanliness {
    /// No open local work on the path — safe to release the lock.
    Clean,
    /// Open/uncommitted local work on the path — the user is still holding it.
    Dirty,
}

/// Which kind of checkpoint the Warden is reasoning about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checkpoint {
    /// An ongoing, intermediate checkpoint — the silent everyday rhythm. The release-bearing
    /// push it can produce is at most a **Sicherungs-Push** (private backup), never a release.
    Laufend,
    /// The explicit revision act: „ich bin fertig damit". This is the only checkpoint that can
    /// produce a **Freigabe-Push** (publish to shared `main` + atomic unlock).
    Revision,
}

/// The full, plain input to the Lock Warden — the cross-product of the four axes. No hidden
/// state: this struct *is* the whole world the decision sees (mirrors the E37 discipline of the
/// Status Reader).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WardenSnapshot {
    pub kind: PathKind,
    pub lock: LockState,
    pub clean: Cleanliness,
    pub checkpoint: Checkpoint,
}

/// Exactly one action the Lock Warden returns per snapshot. The glue in [`crate::pushglue`] is
/// the only thing that turns one of these into git/LFS calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WardenAction {
    /// **Freigabe-Push** — publish the finished binary to the shared `main` stand AND release the
    /// lock atomically. The *only* action that publishes a binary to shared `main` / the LFS
    /// store (E35/E36). Reachable only at a Revision and only when the lock is being released.
    FreigabePush,
    /// **Sicherungs-Push** — mirror local intermediate commits to the personal backup
    /// namespace. Private backup; does **not** publish to shared `main`, does **not** unlock.
    SicherungsPush,
    /// **Auto-Unlock** — release a held-by-me lock whose path is locally clean (E31/E35 self-
    /// healing). Fires at every checkpoint iff clean; never otherwise.
    AutoUnlock,
    /// **Refuse** — do nothing: nothing to do, or not ours to touch (a foreign lock). The safe
    /// default that can never break the invariant.
    Refuse,
}

impl WardenAction {
    /// Whether this action publishes content to the **shared** `main` stand (and thus, for a
    /// binary, into the LFS store). True for exactly [`WardenAction::FreigabePush`] — the single
    /// public act. The Sicherungs-Push stays off shared `main` by construction (E35).
    pub fn publishes_to_shared_main(self) -> bool {
        matches!(self, WardenAction::FreigabePush)
    }

    /// Whether carrying out this action releases the lock. True for the atomic unlock-at-push
    /// ([`WardenAction::FreigabePush`]) and the self-healing [`WardenAction::AutoUnlock`].
    pub fn releases_lock(self) -> bool {
        matches!(self, WardenAction::FreigabePush | WardenAction::AutoUnlock)
    }

    /// Whether this action lets binary **content** reach the LFS store. By E36 this is true for
    /// exactly the Freigabe-Push — one full binary version per Revision, never per save.
    pub fn binary_reaches_lfs_store(self) -> bool {
        matches!(self, WardenAction::FreigabePush)
    }
}

/// The Lock Warden: decide the single action for one path from its snapshot. **Pure, total,
/// deterministic.** Knows no git internals.
///
/// Precedence (each branch is exhaustive over the remaining axes):
///
/// 1. **Foreign lock wins, loudly** → [`WardenAction::Refuse`]. Someone else's coordination claim
///    is never ours to push or release. (For a foreign lock we also never publish — the binary
///    invariant is upheld trivially.)
///
/// 2. **Auto-unlock iff clean** — a lock we hold whose path is locally **clean** → always
///    [`WardenAction::AutoUnlock`], at *every* checkpoint kind (E35 self-healing). A held lock on
///    a **dirty** path is never auto-unlocked.
///
/// 3. **The two push types**, for a path with open local work (`Dirty`):
///    - At a **Revision**: a binary we **hold and have changed** is released — published to
///      shared `main` with the lock dropped atomically → [`WardenAction::FreigabePush`]. This is
///      the *only* way a locked binary's content ever reaches shared `main`, and it does so
///      precisely by ending the lock, so the invariant ("…while the lock is held") holds.
///    - On a **laufend** checkpoint, a binary under our active lock is **never** released; its
///      half-finished state goes only to the private backup → [`WardenAction::SicherungsPush`].
///    - Mergeable **text** is never locked; at any checkpoint with open work it is mirrored to
///      backup on a laufend checkpoint and published on a Revision.
///
/// 4. Otherwise (no work to move, no lock to drop) → [`WardenAction::Refuse`] (nothing to do).
pub fn decide(snap: WardenSnapshot) -> WardenAction {
    // 1. A foreign lock is never ours to push or release — refuse, loudly (handled in the UI).
    if snap.lock == LockState::HeldByOther {
        return WardenAction::Refuse;
    }

    // 2. Self-healing: a lock we hold whose path is locally clean is auto-released, at EVERY
    //    checkpoint. Clean is the only trigger; a dirty held lock is never auto-unlocked here.
    if snap.lock == LockState::HeldByMe && snap.clean == Cleanliness::Clean {
        return WardenAction::AutoUnlock;
    }

    // From here the path is either unlocked, or held-by-me-and-dirty. Decide the push type.
    match (snap.kind, snap.lock, snap.clean, snap.checkpoint) {
        // --- Binary under our active lock, with open work ---
        // Revision: the explicit release. Publish to shared main AND drop the lock atomically.
        // This is the *only* place a held binary's content reaches shared main — and it reaches
        // it by *ending* the lock, so the invariant ("while the lock is held") is never violated.
        (PathKind::Binary, LockState::HeldByMe, Cleanliness::Dirty, Checkpoint::Revision) => {
            WardenAction::FreigabePush
        }
        // Laufend: NEVER release a locked binary change. Back the half-finished binary up to the
        // personal namespace only. This is the load-bearing branch of the Binär-Invariante.
        (PathKind::Binary, LockState::HeldByMe, Cleanliness::Dirty, Checkpoint::Laufend) => {
            WardenAction::SicherungsPush
        }

        // --- Mergeable text with open work (never locked) ---
        // Revision: publish to shared main; there is no lock to release.
        (PathKind::Text, _, Cleanliness::Dirty, Checkpoint::Revision) => {
            WardenAction::FreigabePush
        }
        // Laufend: back up the intermediate text commits privately.
        (PathKind::Text, _, Cleanliness::Dirty, Checkpoint::Laufend) => {
            WardenAction::SicherungsPush
        }

        // --- Unlocked binary with open work (edited but no lock yet held) ---
        // Revision: publish the finished binary to shared main (no lock to drop). It still
        // reaches the LFS store only here (E36). This is not a "locked binary change", so the
        // invariant does not bite.
        (PathKind::Binary, LockState::Unlocked, Cleanliness::Dirty, Checkpoint::Revision) => {
            WardenAction::FreigabePush
        }
        // Laufend: private backup only.
        (PathKind::Binary, LockState::Unlocked, Cleanliness::Dirty, Checkpoint::Laufend) => {
            WardenAction::SicherungsPush
        }

        // --- Nothing to move and nothing to drop ---
        // Clean + unlocked (the clean-held case is handled above): no open work, no lock to drop.
        _ => WardenAction::Refuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every combination of the four axes — the full state cross-product.
    fn all_snapshots() -> Vec<WardenSnapshot> {
        let kinds = [PathKind::Binary, PathKind::Text];
        let locks = [LockState::HeldByMe, LockState::HeldByOther, LockState::Unlocked];
        let cleans = [Cleanliness::Clean, Cleanliness::Dirty];
        let checkpoints = [Checkpoint::Laufend, Checkpoint::Revision];
        let mut out = Vec::new();
        for &kind in &kinds {
            for &lock in &locks {
                for &clean in &cleans {
                    for &checkpoint in &checkpoints {
                        out.push(WardenSnapshot { kind, lock, clean, checkpoint });
                    }
                }
            }
        }
        out
    }

    /// AC: returns exactly one action per snapshot; total over the full cross-product, never
    /// panics, knows no git internals (the input is plain data only).
    #[test]
    fn decide_is_total_over_the_full_cross_product() {
        let snaps = all_snapshots();
        assert_eq!(snaps.len(), 2 * 3 * 2 * 2, "the cross-product is fully enumerated");
        for snap in snaps {
            let action = decide(snap);
            assert!(
                matches!(
                    action,
                    WardenAction::FreigabePush
                        | WardenAction::SicherungsPush
                        | WardenAction::AutoUnlock
                        | WardenAction::Refuse
                ),
                "exactly one action for {snap:?}"
            );
        }
    }

    /// AC (the table): the decision over the full cross-product, asserted row by row.
    #[test]
    fn decide_table_covers_the_full_cross_product() {
        use Checkpoint::*;
        use Cleanliness::*;
        use LockState::*;
        use PathKind::*;
        use WardenAction::*;

        // table: (kind, lock, clean, checkpoint) -> exactly one action
        let cases: &[(PathKind, LockState, Cleanliness, Checkpoint, WardenAction)] = &[
            // --- foreign lock: always refuse (loud, not ours) ---
            (Binary, HeldByOther, Clean, Laufend, Refuse),
            (Binary, HeldByOther, Clean, Revision, Refuse),
            (Binary, HeldByOther, Dirty, Laufend, Refuse),
            (Binary, HeldByOther, Dirty, Revision, Refuse),
            (Text, HeldByOther, Clean, Laufend, Refuse),
            (Text, HeldByOther, Clean, Revision, Refuse),
            (Text, HeldByOther, Dirty, Laufend, Refuse),
            (Text, HeldByOther, Dirty, Revision, Refuse),
            // --- held by me + clean: auto-unlock at EVERY checkpoint ---
            (Binary, HeldByMe, Clean, Laufend, AutoUnlock),
            (Binary, HeldByMe, Clean, Revision, AutoUnlock),
            (Text, HeldByMe, Clean, Laufend, AutoUnlock),
            (Text, HeldByMe, Clean, Revision, AutoUnlock),
            // --- held by me + dirty BINARY: the invariant in action ---
            // laufend -> private backup only (NEVER Freigabe)
            (Binary, HeldByMe, Dirty, Laufend, SicherungsPush),
            // Revision -> Freigabe = publish + atomic unlock (the release)
            (Binary, HeldByMe, Dirty, Revision, FreigabePush),
            // --- held by me + dirty TEXT (text is never really locked, but cover it) ---
            (Text, HeldByMe, Dirty, Laufend, SicherungsPush),
            (Text, HeldByMe, Dirty, Revision, FreigabePush),
            // --- unlocked + dirty: open work, no lock held ---
            (Binary, Unlocked, Dirty, Laufend, SicherungsPush),
            (Binary, Unlocked, Dirty, Revision, FreigabePush),
            (Text, Unlocked, Dirty, Laufend, SicherungsPush),
            (Text, Unlocked, Dirty, Revision, FreigabePush),
            // --- unlocked + clean: nothing to do at any checkpoint ---
            (Binary, Unlocked, Clean, Laufend, Refuse),
            (Binary, Unlocked, Clean, Revision, Refuse),
            (Text, Unlocked, Clean, Laufend, Refuse),
            (Text, Unlocked, Clean, Revision, Refuse),
        ];
        // The table must cover every snapshot exactly once.
        assert_eq!(cases.len(), all_snapshots().len(), "table covers the whole cross-product");

        for (kind, lock, clean, checkpoint, expected) in cases {
            let snap = WardenSnapshot {
                kind: *kind,
                lock: *lock,
                clean: *clean,
                checkpoint: *checkpoint,
            };
            assert_eq!(decide(snap), *expected, "decide({snap:?})");
        }
    }

    /// THE safety-critical invariant, as an **exhaustive property test** over the full cross-
    /// product: a *locked binary change* (binary, lock held by us, the change still open) must
    /// NEVER yield a Freigabe-Push *while the lock is held* — i.e. on any non-release (laufend)
    /// checkpoint. The only Freigabe a held binary ever gets is the Revision release, which
    /// drops the lock atomically (asserted separately below).
    #[test]
    fn binaer_invariante_locked_binary_change_never_freigabe_while_held() {
        for snap in all_snapshots() {
            let is_locked_binary_change = snap.kind == PathKind::Binary
                && snap.lock == LockState::HeldByMe
                && snap.clean == Cleanliness::Dirty;
            if is_locked_binary_change && snap.checkpoint == Checkpoint::Laufend {
                assert_ne!(
                    decide(snap),
                    WardenAction::FreigabePush,
                    "INVARIANT VIOLATED: locked binary change yielded Freigabe-Push while held: {snap:?}"
                );
            }
        }
    }

    /// The deeper invariant, also exhaustive: whenever the Warden DOES publish a binary to shared
    /// `main` (Freigabe-Push), the lock must be released by that same action — a binary never
    /// reaches the shared stand *while the lock stays held* (E35). Equivalently: no action both
    /// publishes a binary to shared main and leaves the lock held.
    #[test]
    fn binaer_invariante_publish_to_shared_main_always_releases_the_lock() {
        for snap in all_snapshots() {
            let action = decide(snap);
            if snap.kind == PathKind::Binary && action.publishes_to_shared_main() {
                assert!(
                    action.releases_lock(),
                    "INVARIANT VIOLATED: a binary reached shared main without releasing the lock: {snap:?} -> {action:?}"
                );
            }
        }
    }

    /// AC: auto-unlock fires **iff** the locked path is locally clean — proven in BOTH directions
    /// over the whole cross-product.
    #[test]
    fn auto_unlock_fires_iff_locked_path_is_clean_both_directions() {
        for snap in all_snapshots() {
            let action = decide(snap);
            let own_lock_clean =
                snap.lock == LockState::HeldByMe && snap.clean == Cleanliness::Clean;
            if own_lock_clean {
                // forward: a held lock on a clean path is ALWAYS auto-unlocked
                assert_eq!(
                    action,
                    WardenAction::AutoUnlock,
                    "clean held lock must auto-unlock: {snap:?}"
                );
            } else {
                // reverse: auto-unlock NEVER fires when not (held-by-me AND clean)
                assert_ne!(
                    action,
                    WardenAction::AutoUnlock,
                    "auto-unlock must not fire unless held-by-me AND clean: {snap:?}"
                );
            }
        }
    }

    /// AC: a Sicherungs-Push never publishes to shared `main` (it is the private backup act).
    #[test]
    fn sicherungs_push_never_publishes_to_shared_main() {
        for snap in all_snapshots() {
            if decide(snap) == WardenAction::SicherungsPush {
                assert!(
                    !WardenAction::SicherungsPush.publishes_to_shared_main(),
                    "Sicherungs-Push must stay off shared main: {snap:?}"
                );
            }
        }
        // and the only action that publishes to shared main is Freigabe-Push
        assert!(WardenAction::FreigabePush.publishes_to_shared_main());
        assert!(!WardenAction::SicherungsPush.publishes_to_shared_main());
        assert!(!WardenAction::AutoUnlock.publishes_to_shared_main());
        assert!(!WardenAction::Refuse.publishes_to_shared_main());
    }

    /// AC: binary content reaches the LFS store ONLY at Freigabe-Push (the bloat cap, E36).
    #[test]
    fn binary_reaches_lfs_store_only_at_freigabe_push() {
        for snap in all_snapshots() {
            let action = decide(snap);
            if action.binary_reaches_lfs_store() {
                assert_eq!(
                    action,
                    WardenAction::FreigabePush,
                    "only Freigabe-Push lets a binary reach the LFS store: {snap:?}"
                );
            }
        }
        assert!(WardenAction::FreigabePush.binary_reaches_lfs_store());
        assert!(!WardenAction::SicherungsPush.binary_reaches_lfs_store());
        assert!(!WardenAction::AutoUnlock.binary_reaches_lfs_store());
        assert!(!WardenAction::Refuse.binary_reaches_lfs_store());
    }

    /// AC: a foreign lock is never ours to act on — always Refuse, regardless of the other axes.
    #[test]
    fn foreign_lock_is_always_refused() {
        for snap in all_snapshots() {
            if snap.lock == LockState::HeldByOther {
                assert_eq!(decide(snap), WardenAction::Refuse, "foreign lock -> refuse: {snap:?}");
            }
        }
    }
}
