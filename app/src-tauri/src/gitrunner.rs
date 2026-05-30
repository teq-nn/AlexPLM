//! The single hardened git spawn point (Issue #22).
//!
//! Before this module every git-touching module built its own `Command::new("git")` with no
//! environment hardening, so any network operation (`fetch`/`push`) could fall back to a
//! **terminal** credential prompt — invisible to the Tauri frontend, fatal to the silent daily
//! sync. This module is the one place a `git` child is created, and it guarantees on **every**
//! spawn:
//!
//! - `GIT_TERMINAL_PROMPT=0` — git must never prompt the tty for credentials; a missing/bad token
//!   fails fast instead of hanging on an invisible prompt.
//! - `GIT_ASKPASS=<self>` — git asks **our own binary** (`current_exe()` in `--askpass` mode, keyed
//!   by [`ASKPASS_ENV`]) for the username/token, which it reads from the OS keystore. No
//!   platform-specific `.sh`/`.bat` helper — the same binary works on Windows and Linux.
//! - `LC_ALL=C` — git speaks English regardless of the user's locale, so the stderr classification
//!   in [`classify_failure`] is deterministic across machines.
//!
//! The two constructors ([`command`] / [`command_bare`]) return a ready `Command` callers extend
//! with their own args, so the migration of the eight existing modules is mechanical and their
//! behavior is unchanged except for the hardening. The failure classifier is pure and table-tested
//! so the exact stderr markers are asserted here rather than discovered in production.

use std::path::Path;
use std::process::Command;

/// Environment marker git propagates to the [`GIT_ASKPASS`](harden) child. The app binary checks
/// it on startup ([`crate::askpass::is_askpass_invocation`]) to enter `--askpass` mode instead of
/// launching the GUI. An env marker (not an argv flag) is the only portable signal, because git —
/// not us — owns the askpass child's argv.
pub const ASKPASS_ENV: &str = "PLM_GIT_ASKPASS";

/// Tag the askpass helper prints to its stderr when the OS keystore itself is unreachable (e.g. no
/// Secret Service daemon on Linux). It rides git's stderr up to the caller so [`classify_failure`]
/// can tell "keystore is down" apart from "token is wrong", and the frontend can say so precisely
/// instead of hanging or showing a raw git error.
pub const KEYSTORE_UNAVAILABLE_MARKER: &str = "PLM-KEYSTORE-UNAVAILABLE";

/// A `git -C <root> …` command, hardened so it can never prompt the terminal and always resolves
/// credentials through our askpass helper. Callers append their subcommand and args.
pub fn command(root: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C").arg(root);
    harden(&mut c);
    c
}

/// A hardened `git …` command with no `-C` prefix, for the few invocations that pass an explicit
/// repo path another way (e.g. `git init --bare <dir>`). Same hardening as [`command`].
pub fn command_bare() -> Command {
    let mut c = Command::new("git");
    harden(&mut c);
    c
}

/// Apply the credential-prompt hardening to a git `Command`. Idempotent; safe to call once per
/// spawn. If `current_exe()` cannot be resolved we still set `GIT_TERMINAL_PROMPT=0`, so the worst
/// case is a fast clean failure rather than a hidden tty prompt.
fn harden(c: &mut Command) {
    c.env("GIT_TERMINAL_PROMPT", "0");
    c.env(ASKPASS_ENV, "1");
    c.env("LC_ALL", "C");
    if let Ok(exe) = std::env::current_exe() {
        c.env("GIT_ASKPASS", exe);
    }
}

/// The kind of failure behind a non-zero git exit, judged purely from its stderr. Drives the typed
/// error the frontend receives: an [`Auth`](GitFailure::Auth) failure reopens the credential field,
/// a [`KeystoreUnavailable`](GitFailure::KeystoreUnavailable) tells the user the OS keystore is not
/// reachable, and everything else is a generic git error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFailure {
    /// Missing, wrong, or expired credentials — or git refusing to prompt (terminal disabled).
    Auth,
    /// Our askpass helper could not reach the OS keystore (e.g. no Secret Service daemon).
    KeystoreUnavailable,
    /// Any other git failure (network, refs, merge, …) — not credential-related.
    Other,
}

/// Classify a git failure from its stderr. Pure, total, case-insensitive.
///
/// The keystore marker is checked first because a down keystore *also* produces an auth-shaped
/// failure downstream; distinguishing it lets the UI give the precise message. The auth markers
/// cover git over HTTP(S) with `GIT_TERMINAL_PROMPT=0`: missing creds ("terminal prompts
/// disabled" / "could not read Username"), and rejected creds (401/403, "Authentication failed",
/// "access denied").
pub fn classify_failure(stderr: &str) -> GitFailure {
    let s = stderr.to_ascii_lowercase();

    if s.contains(&KEYSTORE_UNAVAILABLE_MARKER.to_ascii_lowercase()) {
        return GitFailure::KeystoreUnavailable;
    }

    const AUTH_MARKERS: &[&str] = &[
        "authentication failed",
        "could not read username",
        "could not read password",
        "invalid username or password",
        "http basic: access denied",
        "access denied",
        "terminal prompts disabled",
        "403 forbidden",
        "error: 403",
        "status code 403",
        "401 unauthorized",
        "status code 401",
        "remote: forbidden",
        "fatal: authentication",
    ];
    if AUTH_MARKERS.iter().any(|m| s.contains(m)) {
        return GitFailure::Auth;
    }

    GitFailure::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardened_command_sets_every_prompt_guard() {
        let root = Path::new("/tmp/x");
        let cmd = command(root);
        // Collect the env overrides the Command carries (key, Some(value)).
        let envs: std::collections::HashMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();

        assert_eq!(envs.get("GIT_TERMINAL_PROMPT"), Some(&Some("0".to_string())));
        assert_eq!(envs.get(ASKPASS_ENV), Some(&Some("1".to_string())));
        assert_eq!(envs.get("LC_ALL"), Some(&Some("C".to_string())));
        // GIT_ASKPASS points at a binary (whatever current_exe resolved to) — present and non-empty.
        let askpass = envs.get("GIT_ASKPASS").expect("GIT_ASKPASS set");
        assert!(askpass.as_ref().is_some_and(|p| !p.is_empty()));
    }

    #[test]
    fn bare_command_is_also_hardened() {
        let cmd = command_bare();
        let has_prompt_guard = cmd
            .get_envs()
            .any(|(k, v)| k == "GIT_TERMINAL_PROMPT" && v == Some(std::ffi::OsStr::new("0")));
        assert!(has_prompt_guard, "command_bare must harden too");
    }

    #[test]
    fn classify_failure_table() {
        // table: stderr fragment -> expected classification
        let cases: &[(&str, GitFailure)] = &[
            // --- auth: rejected credentials ---
            ("fatal: Authentication failed for 'https://h/o/p.git'", GitFailure::Auth),
            ("remote: HTTP Basic: Access denied", GitFailure::Auth),
            ("error: 403 Forbidden", GitFailure::Auth),
            ("The requested URL returned error: 403", GitFailure::Auth),
            ("fatal: unable to access ... The requested URL returned error: 401 Unauthorized", GitFailure::Auth),
            ("remote: Invalid username or password.", GitFailure::Auth),
            // --- auth: missing credentials with the terminal prompt disabled ---
            ("fatal: could not read Username for 'https://h': terminal prompts disabled", GitFailure::Auth),
            // --- keystore down: our askpass marker wins even amid auth noise ---
            (
                "PLM-KEYSTORE-UNAVAILABLE\nfatal: could not read Username: terminal prompts disabled",
                GitFailure::KeystoreUnavailable,
            ),
            // --- other: not credential-related ---
            ("fatal: couldn't find remote ref main", GitFailure::Other),
            ("error: failed to push some refs", GitFailure::Other),
            ("fatal: not a git repository", GitFailure::Other),
            ("", GitFailure::Other),
        ];
        for (stderr, expected) in cases {
            assert_eq!(
                classify_failure(stderr),
                *expected,
                "classify_failure({stderr:?})"
            );
        }
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify_failure("AUTHENTICATION FAILED"), GitFailure::Auth);
        assert_eq!(classify_failure("plm-keystore-unavailable"), GitFailure::KeystoreUnavailable);
    }
}
