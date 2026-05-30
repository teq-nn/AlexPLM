//! Silent auto-commit core (Issue #4).
//!
//! A filesystem watcher observes the product folder and triggers a **debounced**
//! auto-commit when saving "settles" (a few seconds of quiet) — never per keystroke.
//! Rapid successive saves coalesce into a single commit after the quiet window. The
//! commit message is machine-generated and boring (`auto: <pfad>, <zeitstempel>`); the
//! user never writes one. Each settled save produces a new **Stand** in the UI; the word
//! "commit" never surfaces (E33/E39).
//!
//! As in `projection.rs`, the pure decision bits are split out from the side-effecting
//! git call so they can be exercised by table tests without timers, a real clock, or a
//! real repo: [`machine_message`] formats the boring message and [`Debouncer`] decides
//! *when* a burst of saves has settled. The side-effecting [`commit_all`] and the watcher
//! loop sit on top and are kept deliberately thin.

use serde::Serialize;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default quiet window: how long the folder must be silent after the last save before
/// the burst is considered settled and one commit is produced. A few seconds — long
/// enough that an editor's flurry of writes coalesces, short enough to feel live.
pub const DEFAULT_QUIET_WINDOW: Duration = Duration::from_secs(3);

/// A settled save, surfaced to the UI as a new **Stand**. Carries no git vocabulary.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Stand {
    /// Product-relative path that settled (forward slashes).
    pub path: String,
    /// Machine timestamp, `YYYY-MM-DDTHH:MM:SSZ` UTC.
    pub timestamp: String,
}

/// The boring, machine-generated message stamped on a settled save: `auto: <pfad>, <zeitstempel>`.
/// Pure function over path + timestamp so the format is table-testable. The user is never
/// prompted for this text and never sees it — it exists only inside git.
pub fn machine_message(path: &str, timestamp: &str) -> String {
    format!("auto: {path}, {timestamp}")
}

/// Format a [`SystemTime`] as a stable `YYYY-MM-DDTHH:MM:SSZ` UTC stamp without pulling in a
/// date crate. Pure over the instant so it is table-testable against known epoch seconds.
pub fn format_timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_epoch_secs(secs)
}

/// Civil (UTC) formatting of Unix epoch seconds. Pure, single allocation for the result.
fn format_epoch_secs(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (hh, mm, ss) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Days since 1970-01-01 -> (year, month, day). Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// What the [`Debouncer`] should do after observing an event at a given instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// A save was seen; the burst is still in progress. Wait for more quiet.
    Pending,
    /// The quiet window elapsed with no further saves: emit exactly **one** Stand for the
    /// whole burst, for this product-relative path.
    Settle { path: String },
    /// Nothing pending (e.g. a tick fired with no save outstanding). Do nothing.
    Idle,
}

/// Debounce state machine that coalesces a burst of saves into one settle.
///
/// Deliberately pure: it owns no timer and no clock. The caller feeds it the *current
/// instant* on every event ([`observe_save`] when a write is seen, [`poll`] on a timer
/// tick), and the debouncer decides. This makes "rapid saves coalesce into one settle
/// after the quiet window" and the exact timing table-testable with a fake clock.
#[derive(Debug)]
pub struct Debouncer {
    window: Duration,
    /// Set while a burst is in flight: (last-save instant, last-save path).
    pending: Option<(SystemTime, String)>,
}

impl Debouncer {
    /// New debouncer with the given quiet window.
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            pending: None,
        }
    }

    /// Quiet window in force.
    pub fn window(&self) -> Duration {
        self.window
    }

    /// Whether a burst is currently in flight (a save has been seen but not yet settled).
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Record a save seen at `now` for product-relative `path`. Always returns
    /// [`Decision::Pending`]: a fresh save can never settle the burst, it only (re)arms
    /// the quiet window. Successive saves overwrite the pending instant/path, so the
    /// window restarts and the whole burst will settle as **one** Stand.
    pub fn observe_save(&mut self, now: SystemTime, path: &str) -> Decision {
        self.pending = Some((now, path.to_string()));
        Decision::Pending
    }

    /// Check at `now` whether the pending burst has gone quiet for the full window.
    /// Returns [`Decision::Settle`] exactly once per burst (it clears the pending state),
    /// [`Decision::Pending`] if the window has not yet elapsed, or [`Decision::Idle`] if
    /// nothing is pending.
    pub fn poll(&mut self, now: SystemTime) -> Decision {
        match &self.pending {
            None => Decision::Idle,
            Some((last, path)) => {
                let quiet_for = now.duration_since(*last).unwrap_or(Duration::ZERO);
                if quiet_for >= self.window {
                    let path = path.clone();
                    self.pending = None;
                    Decision::Settle { path }
                } else {
                    Decision::Pending
                }
            }
        }
    }
}

/// Stage every change in `root` and create one local commit with the boring machine
/// message. Side-effecting: the only place that touches git. Returns the [`Stand`] to
/// surface, or `Ok(None)` if there was nothing to commit (a settle with no real change).
///
/// Local commit only — never pushes, never touches LFS, never prompts for text (E36/E39).
pub fn commit_all(root: &Path, rel_path: &str, now: SystemTime) -> std::io::Result<Option<Stand>> {
    let timestamp = format_timestamp(now);

    // Stage everything under the product root.
    let add = crate::gitrunner::command(root)
        .args(["add", "-A"])
        .output()?;
    if !add.status.success() {
        return Err(git_err("git add", &add.stderr));
    }

    // Nothing staged -> nothing to commit; not an error, just no new Stand.
    let diff = crate::gitrunner::command(root)
        .args(["diff", "--cached", "--quiet"])
        .status()?;
    if diff.success() {
        return Ok(None);
    }

    let message = machine_message(rel_path, &timestamp);
    let commit = crate::gitrunner::command(root)
        .args(["commit", "-m", &message])
        .output()?;
    if !commit.status.success() {
        return Err(git_err("git commit", &commit.stderr));
    }

    Ok(Some(Stand {
        path: rel_path.to_string(),
        timestamp,
    }))
}

fn git_err(what: &str, stderr: &[u8]) -> std::io::Error {
    std::io::Error::other(format!(
        "{what} failed: {}",
        String::from_utf8_lossy(stderr).trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_message_is_boring_and_fixed_format() {
        // table: (path, timestamp) -> exact message
        let cases: &[(&str, &str, &str)] = &[
            (
                "elektronik/regler",
                "2026-05-30T09:15:00Z",
                "auto: elektronik/regler, 2026-05-30T09:15:00Z",
            ),
            (".", "1970-01-01T00:00:00Z", "auto: ., 1970-01-01T00:00:00Z"),
            (
                "mechanik/gehaeuse/gehaeuse.f3d",
                "2026-12-31T23:59:59Z",
                "auto: mechanik/gehaeuse/gehaeuse.f3d, 2026-12-31T23:59:59Z",
            ),
        ];
        for (path, ts, expected) in cases {
            assert_eq!(machine_message(path, ts), *expected, "path={path} ts={ts}");
        }
    }

    #[test]
    fn machine_message_is_the_only_text_and_user_writes_none() {
        // The message is fully derived from path + timestamp; no field is free human text.
        let m = machine_message("elektronik/regler", "2026-05-30T09:15:00Z");
        assert!(m.starts_with("auto: "));
    }

    #[test]
    fn timestamp_formats_known_epochs_as_utc() {
        // table: epoch seconds -> civil UTC stamp (well-known reference points)
        let cases: &[(u64, &str)] = &[
            (0, "1970-01-01T00:00:00Z"),
            (1_000_000_000, "2001-09-09T01:46:40Z"),
            (1_700_000_000, "2023-11-14T22:13:20Z"),
        ];
        for (secs, expected) in cases {
            assert_eq!(format_epoch_secs(*secs), *expected, "secs={secs}");
        }
        assert_eq!(format_timestamp(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    }

    /// A burst of rapid saves must coalesce into exactly ONE settle after the quiet window.
    #[test]
    fn rapid_saves_coalesce_into_one_settle() {
        let window = Duration::from_secs(3);
        let mut d = Debouncer::new(window);
        let t0 = UNIX_EPOCH;
        let at = |s: u64| t0 + Duration::from_secs(s);

        // Five rapid saves within the window, each (re)arming it. None settles.
        let mut settles = 0;
        for s in [0, 1, 1, 2, 2] {
            assert_eq!(
                d.observe_save(at(s), "elektronik/regler"),
                Decision::Pending
            );
            // a timer tick right after each save: still inside the window from the last save
            if let Decision::Settle { .. } = d.poll(at(s)) {
                settles += 1;
            }
        }
        assert_eq!(settles, 0, "no settle while saves keep arriving");
        assert!(d.is_pending());

        // Quiet for less than the window after the last save (last save at t=2): not yet.
        assert_eq!(d.poll(at(4)), Decision::Pending);

        // Quiet for the full window (last save t=2, +3s = t=5): settles exactly once.
        assert_eq!(
            d.poll(at(5)),
            Decision::Settle {
                path: "elektronik/regler".into()
            }
        );
        // And only once — the pending burst is now cleared.
        assert_eq!(d.poll(at(6)), Decision::Idle);
    }

    #[test]
    fn debounce_timing_boundary_is_inclusive_at_the_window() {
        let mut d = Debouncer::new(Duration::from_secs(3));
        let t0 = UNIX_EPOCH;
        d.observe_save(t0, "a/b");
        // exactly at the window settles; just under stays pending.
        assert_eq!(d.poll(t0 + Duration::from_millis(2_999)), Decision::Pending);
        assert_eq!(
            d.poll(t0 + Duration::from_secs(3)),
            Decision::Settle { path: "a/b".into() }
        );
    }

    #[test]
    fn a_later_save_restarts_the_quiet_window() {
        let mut d = Debouncer::new(Duration::from_secs(3));
        let t0 = UNIX_EPOCH;
        let at = |s: u64| t0 + Duration::from_secs(s);

        d.observe_save(at(0), "x");
        // almost settled...
        assert_eq!(d.poll(at(2)), Decision::Pending);
        // ...then another save arrives, restarting the window from t=2.
        d.observe_save(at(2), "x");
        // t=4 is only 2s after the last save -> still pending (would have settled w/o the 2nd save).
        assert_eq!(d.poll(at(4)), Decision::Pending);
        // t=5 is 3s after the last save -> settles.
        assert_eq!(d.poll(at(5)), Decision::Settle { path: "x".into() });
    }

    #[test]
    fn poll_without_pending_is_idle() {
        let mut d = Debouncer::new(Duration::from_secs(3));
        assert_eq!(d.poll(UNIX_EPOCH), Decision::Idle);
    }
}
