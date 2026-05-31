//! Watcher-side **Auto-Lock** decision core (Issue #42).
//!
//! The Binär-Invariante (E35) guards a locked binary change from ever reaching the shared stand
//! while the lock is held. But the lock has to *exist* before the very first checkpoint, or there
//! is a window between "I edited the binary" and "the lock was taken" in which a colleague could
//! start editing the same file unseen. This module closes that window: the Watcher acquires a lock
//! on the **first dirty lockable path**, the instant a save event arrives — not on a manual card
//! click, and not after the first commit.
//!
//! Following the house pattern (`warden.rs`, `locks.rs`, `autocommit.rs`), the *decision* is a
//! pure, total, deterministic function over plain data — it knows no git, no clock, no process.
//! The side-effecting glue that carries it out (acquire the lock, flip the file's read-only bit)
//! lives in [`crate::lockglue`]; the watch loop in [`crate::watcher`] only feeds events in.
//!
//! Two facts the decision rests on:
//! - **Lockable = the #3 classifier's unmergeable buckets** — reused verbatim via
//!   [`crate::locks::is_lockable`], so import-time marking, edit-time auto-lock and the
//!   watcher-side auto-lock all agree on exactly the same set of files.
//! - **Read-only = free.** A lockable binary rests read-only on disk; acquiring the lock is what
//!   makes it writable *for me*. So the read-only/writable bit tracks lock ownership, never the
//!   other way round (the file system is a mirror of the lock, not a second source of truth).

/// The single decision the Watcher makes when a save touches a path: should it auto-acquire a
/// lock for that path right now?
///
/// Pure over the two facts that matter — is the path lockable, and do we already hold its lock
/// this session. Lock again only when the path is lockable **and** not already held by us; a
/// non-lockable (mergeable text) path is never locked, and a path we already locked is left alone
/// (idempotent — re-saving a file we are already editing must not re-fire a lock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoLock {
    /// Acquire the `git lfs lock` for this path now (and make it writable for me).
    Acquire,
    /// Do nothing: either not lockable, or we already hold the lock.
    Skip,
}

impl AutoLock {
    /// Whether this decision means "take the lock now".
    pub fn should_acquire(self) -> bool {
        matches!(self, AutoLock::Acquire)
    }
}

/// Decide whether a save to `rel_path` should auto-acquire its lock, given whether the path is
/// lockable and whether we already hold the lock this session. **Pure, total, deterministic.**
///
/// - lockable && not already held → [`AutoLock::Acquire`] (close the invariant window now);
/// - lockable && already held     → [`AutoLock::Skip`] (idempotent — already ours);
/// - not lockable                 → [`AutoLock::Skip`] (mergeable text is never locked).
pub fn decide_auto_lock(lockable: bool, already_held: bool) -> AutoLock {
    if lockable && !already_held {
        AutoLock::Acquire
    } else {
        AutoLock::Skip
    }
}

/// Whether a lockable path should rest **read-only** on disk given whether we hold its lock.
/// Pure: read-only = free (no lock of ours); writable = the lock is ours (the edit-intent that
/// took the lock makes the file mine to change). A non-lockable path is never made read-only by
/// us — mergeable text is always freely editable.
pub fn rests_read_only(lockable: bool, held_by_me: bool) -> bool {
    lockable && !held_by_me
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locks::is_lockable;

    /// The full cross-product of the two decision axes — total, never panics.
    #[test]
    fn decide_auto_lock_table_covers_the_cross_product() {
        // table: (lockable, already_held) -> decision
        let cases: &[(bool, bool, AutoLock)] = &[
            // lockable + not yet ours -> take the lock now (the whole point of the slice)
            (true, false, AutoLock::Acquire),
            // lockable + already ours -> idempotent skip (re-saving must not re-lock)
            (true, true, AutoLock::Skip),
            // mergeable text -> never locked, regardless of "held" (which can't happen)
            (false, false, AutoLock::Skip),
            (false, true, AutoLock::Skip),
        ];
        assert_eq!(cases.len(), 2 * 2, "the cross-product is fully enumerated");
        for (lockable, held, expected) in cases {
            assert_eq!(
                decide_auto_lock(*lockable, *held),
                *expected,
                "decide_auto_lock(lockable={lockable}, held={held})"
            );
        }
    }

    /// The acquire decision fires **iff** the path is lockable and not already held — proven in
    /// both directions over the cross-product.
    #[test]
    fn acquire_fires_iff_lockable_and_not_held() {
        for &lockable in &[true, false] {
            for &held in &[true, false] {
                let acquires = decide_auto_lock(lockable, held).should_acquire();
                assert_eq!(acquires, lockable && !held, "lockable={lockable} held={held}");
            }
        }
    }

    /// Read-only = free: a lockable path rests read-only exactly while we do NOT hold its lock;
    /// taking the lock makes it writable. Mergeable text is never made read-only by us.
    #[test]
    fn read_only_tracks_lock_ownership_for_lockable_paths_only() {
        // table: (lockable, held_by_me) -> rests read-only?
        let cases: &[(bool, bool, bool)] = &[
            (true, false, true),   // lockable, free -> read-only
            (true, true, false),   // lockable, mine -> writable
            (false, false, false), // text -> always writable
            (false, true, false),  // text -> always writable
        ];
        for (lockable, held, expected) in cases {
            assert_eq!(
                rests_read_only(*lockable, *held),
                *expected,
                "rests_read_only(lockable={lockable}, held={held})"
            );
        }
    }

    /// The auto-lock trigger keys off exactly the #3 classifier's lockable set — the same files
    /// import marks `lockable` and the Status Reader greys. Binaries/KiCad acquire; text never.
    #[test]
    fn auto_lock_trigger_matches_the_classifier_lockable_set() {
        // table: filename -> does a first save auto-acquire a lock? (== is_lockable, not yet held)
        let cases: &[(&str, bool)] = &[
            ("mechanik/gehaeuse/gehaeuse.f3d", true),
            ("mechanik/halter/halter.step", true),
            ("part.stl", true),
            ("elektronik/board.kicad_pcb", true),
            ("elektronik/board.kicad_sch", true),
            ("datasheet.pdf", true),
            ("render.PNG", true),
            ("firmware/main.c", false),
            ("docs/README.md", false),
            ("config.yaml", false),
            ("board.kicad_pro", false),
        ];
        for (name, expected) in cases {
            assert_eq!(
                decide_auto_lock(is_lockable(name), false).should_acquire(),
                *expected,
                "name = {name}"
            );
        }
    }
}
