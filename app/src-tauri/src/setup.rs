//! The one-time **Einrichtungs-Zeremonie** (Issue #5, E41).
//!
//! This is the *explicit exception* where git-near vocabulary is acceptable: connecting a
//! self-hosted Forgejo/Gitea remote, the first publish, enabling `locksverify`, and inviting a
//! colleague are rare, one-time, low-risk acts (E41) — so unlike the silent daily sync, the
//! ceremony may speak plainly about "Server", "veröffentlichen" and the clone URL.
//!
//! Structure follows the house pattern (`import.rs`, `lockglue.rs`): the **decisions** are a
//! pure, total, table-testable core — host/credential validation & URL normalization
//! ([`normalize_remote`]), the `locksverify` config invocation ([`locksverify_config`]), and
//! the one-time / already-configured state machine ([`decide_setup_state`]). The thin,
//! isolated, side-effecting layer ([`configure_remote`], [`publish_product`], [`read_setup`])
//! is the only part that shells out to git. Tests drive the core directly and stand a bare
//! local repo in for the "remote" — they never touch a real Forgejo/Gitea server.

use serde::Serialize;
use std::io::{Error, ErrorKind};
use std::path::Path;

/// The remote name the ceremony writes. A single, well-known name keeps "already configured"
/// detection simple and the daily sync silent (it just pushes/pulls this remote, never named).
pub const REMOTE_NAME: &str = "origin";

// ----------------------------------------------------------------------------------------------
// Pure core 1 — host + credential validation & URL normalization
// ----------------------------------------------------------------------------------------------

/// A validated, normalized Forgejo/Gitea remote, ready for `git remote add`. Pure data produced
/// by [`normalize_remote`]; the credentials are kept apart from the bare clone URL so we can show
/// colleagues a copy-pasteable clone URL **without** leaking the owner's token/password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteConfig {
    /// The push/fetch URL with any `user[:token]@` credentials embedded. Retained for validation
    /// and tests; since Issue #22 git is **not** configured with this — credentials live in the OS
    /// keystore and `.git/config` gets the credential-free [`clone_url`](RemoteConfig::clone_url).
    pub push_url: String,
    /// The bare `https://host/owner/repo.git` URL with NO credentials — safe to show/share and
    /// what a colleague clones (the invite). Never carries the owner's secret.
    pub clone_url: String,
    /// The `https://host` origin, used to scope the `lfs.<url>.locksverify` config (E41 note).
    pub host_url: String,
}

/// Validate and normalize the fields the user typed in the ceremony into a [`RemoteConfig`].
///
/// Pure and total: never shells out, never panics, returns a human German error string on bad
/// input. Rules (deliberately strict so a typo fails loud here, not mid-push):
/// - `host` must be a non-empty `http(s)://host[:port]` with a host part. A bare `host:port`
///   without scheme is accepted and defaulted to `https://` (the safe Forgejo/Gitea default).
/// - `repo` must be non-empty and free of slashes/whitespace/control characters.
/// - `owner` is optional: left empty it defaults to the authenticated `user` (publishing under
///   one's own account). The effective owner must be free of slashes/whitespace/control chars.
/// - `user` may be empty (credentials supplied out-of-band, e.g. a credential helper); if given,
///   it is embedded; a `token` (password) without a `user` is rejected (git would misread it).
/// - the `.git` suffix on `repo` is optional and normalized to exactly one.
pub fn normalize_remote(
    host: &str,
    owner: &str,
    repo: &str,
    user: &str,
    token: &str,
) -> Result<RemoteConfig, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Server-Adresse fehlt.".into());
    }

    // Accept a scheme-less host (default to https, the Forgejo/Gitea norm); reject anything that
    // isn't http(s) — ssh/file/etc. are out of scope for this ceremony. Split the scheme off the
    // raw input *before* trimming slashes so `https://` (empty host) is caught, not mistaken for
    // a scheme-less host named `https:`.
    let (scheme, hostport) = match host.split_once("://") {
        Some(("https", rest)) => ("https", rest),
        Some(("http", rest)) => ("http", rest),
        // `file://` is accepted for an offline/local mirror (and is how tests stand a bare repo
        // in for a server); its "host" is an absolute path, so it skips the host-shape check.
        Some(("file", rest)) => ("file", rest),
        Some((other, _)) => return Err(format!("Nicht unterstütztes Protokoll: {other}://")),
        None => ("https", host.trim_end_matches('/')),
    };
    let hostport = hostport.trim_end_matches('/');
    let is_file = scheme == "file";
    if hostport.is_empty()
        || (!is_file && (hostport.contains('/') || hostport.contains(char::is_whitespace)))
    {
        return Err("Server-Adresse ist keine gültige Host-Adresse.".into());
    }
    let host_url = format!("{scheme}://{hostport}");

    let repo_raw = clean_segment(repo, "Produkt-Name")?;
    let repo = repo_raw.strip_suffix(".git").unwrap_or(&repo_raw);
    if repo.is_empty() {
        return Err("Produkt-Name fehlt.".into());
    }

    // Credentials: a password without a username is ambiguous to git; reject it. An empty user
    // means "credentials handled elsewhere" and yields a credential-free push URL.
    let user = user.trim();
    let token = token.trim();
    if user.is_empty() && !token.is_empty() {
        return Err("Passwort ohne Benutzernamen — bitte Benutzernamen angeben.".into());
    }

    // The owner is optional: left blank it defaults to the authenticated user (the username just
    // entered), so a user publishing under their own account need not repeat their name. Validate
    // the chosen owner the same way regardless of where it came from.
    let owner = if owner.trim().is_empty() {
        clean_segment(user, "Besitzer/Organisation")?
    } else {
        clean_segment(owner, "Besitzer/Organisation")?
    };

    let clone_url = format!("{host_url}/{owner}/{repo}.git");
    let push_url = if user.is_empty() {
        clone_url.clone()
    } else if token.is_empty() {
        format!("{scheme}://{}@{hostport}/{owner}/{repo}.git", pct(user))
    } else {
        format!("{scheme}://{}:{}@{hostport}/{owner}/{repo}.git", pct(user), pct(token))
    };

    Ok(RemoteConfig {
        push_url,
        clone_url,
        host_url,
    })
}

/// Validate one path segment (owner / repo): non-empty, no slash, whitespace, or control chars.
fn clean_segment(value: &str, field: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(format!("{field} fehlt."));
    }
    if v.contains('/') || v.contains('\\') || v.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(format!("{field} enthält ungültige Zeichen."));
    }
    Ok(v.to_string())
}

/// Percent-encode the characters in a userinfo component that would otherwise break a URL
/// (`@`, `:`, `/`, `#`, `?`, `%`, and whitespace). Total; leaves ordinary tokens untouched.
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'@' | b':' | b'/' | b'#' | b'?' | b'%' | b' ' | b'\t' => {
                out.push_str(&format!("%{b:02X}"));
            }
            _ => out.push(b as char),
        }
    }
    out
}

// ----------------------------------------------------------------------------------------------
// Pure core 2 — the `locksverify` config invocation (E41 / Realitätsbefund)
// ----------------------------------------------------------------------------------------------

/// The exact `git config` arguments that switch on LFS lock verification for a host (E41:
/// "`locksverify` muss aktiv eingeschaltet werden"). Pure: builds `git config --local
/// lfs.<host>/info/lfs.locksverify true`, the key git-lfs reads per-endpoint. Kept pure so the
/// precise key string is asserted by a unit test rather than discovered in production.
pub fn locksverify_config(host_url: &str) -> Vec<String> {
    vec![
        "config".into(),
        "--local".into(),
        format!("lfs.{}/info/lfs.locksverify", host_url.trim_end_matches('/')),
        "true".into(),
    ]
}

// ----------------------------------------------------------------------------------------------
// Pure core 3 — the one-time / already-configured state machine
// ----------------------------------------------------------------------------------------------

/// Where a product stands in the ceremony. Serialised kebab-case for the UI, which shows the
/// ceremony **only** in `NotConfigured` and a settled "eingerichtet" readout otherwise — so the
/// one-time flow is clearly separated from daily use (E41).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetupStage {
    /// No remote configured: offer the one-time ceremony.
    NotConfigured,
    /// A remote is configured but nothing has been published yet: offer the first publish.
    RemoteSetNotPublished,
    /// Remote configured and the product has been published: settled. Ceremony is done.
    Eingerichtet,
}

/// The observable facts about a product's remote wiring. Gather via [`read_setup`] (I/O); feed
/// here for the pure decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupFacts {
    /// A remote named [`REMOTE_NAME`] is configured.
    pub has_remote: bool,
    /// The local branch has been published (an upstream / remote-tracking ref exists).
    pub has_published: bool,
}

/// Decide the ceremony stage from the facts. Pure, total, deterministic (E41: one-time, clearly
/// separated). "Published but somehow no remote" is impossible in practice and folds to
/// `NotConfigured` (re-run the ceremony) rather than claiming a false settled state.
pub fn decide_setup_state(facts: SetupFacts) -> SetupStage {
    match (facts.has_remote, facts.has_published) {
        (false, _) => SetupStage::NotConfigured,
        (true, false) => SetupStage::RemoteSetNotPublished,
        (true, true) => SetupStage::Eingerichtet,
    }
}

// ----------------------------------------------------------------------------------------------
// The full ceremony report (pure decision + the facts + a colleague's clone URL)
// ----------------------------------------------------------------------------------------------

/// What the UI renders for the ceremony: the stage, the facts behind it, and — once a remote is
/// configured — the credential-free clone URL to hand a colleague (the invite).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupReport {
    /// The pure stage decision (drives whether the ceremony or the settled readout shows).
    pub stage: SetupStage,
    /// Whether a remote named [`REMOTE_NAME`] is configured.
    pub has_remote: bool,
    /// Whether the product has been published (an upstream exists).
    pub has_published: bool,
    /// The credential-free clone URL a colleague uses, if a remote is configured (the invite).
    pub clone_url: Option<String>,
}

// ----------------------------------------------------------------------------------------------
// Side-effecting glue (the only part that shells out to git) — kept thin.
// ----------------------------------------------------------------------------------------------

/// Read a product's current ceremony state from git purely (no mutation): is a remote
/// configured, has it been published, and the credential-free clone URL for the invite.
pub fn read_setup(root: &Path) -> std::io::Result<SetupReport> {
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Kein Ordner: {}", root.display()),
        ));
    }
    let remote_url = remote_get_url(root);
    let has_remote = remote_url.is_some();
    let has_published = has_remote && branch_has_upstream(root);
    let facts = SetupFacts {
        has_remote,
        has_published,
    };
    Ok(SetupReport {
        stage: decide_setup_state(facts),
        has_remote,
        has_published,
        clone_url: remote_url.map(|u| strip_credentials(&u)),
    })
}

/// Configure the Forgejo/Gitea remote + enable `locksverify` (the connect step of the ceremony).
///
/// Validates/normalizes the typed fields with the pure [`normalize_remote`] first (a typo fails
/// here, before any git call), then adds/updates the remote named [`REMOTE_NAME`] and switches on
/// LFS lock verification for the host. Returns the refreshed [`SetupReport`] so the UI advances
/// to the publish step in one round-trip. Does **not** push — that is the deliberate next step.
pub fn configure_remote(
    root: &Path,
    host: &str,
    owner: &str,
    repo: &str,
    user: &str,
    token: &str,
) -> std::io::Result<SetupReport> {
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Kein Ordner: {}", root.display()),
        ));
    }
    let cfg = normalize_remote(host, owner, repo, user, token)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;

    // Auth into the OS keystore, NOT into `.git/config` (Issue #22). When credentials were typed,
    // store the user+token in the native keystore keyed by the host origin; git pulls them at
    // runtime through our askpass helper. An empty user means "credentials handled elsewhere" —
    // nothing to store, and the bare URL below carries no secret either way.
    let user_t = user.trim();
    let token_t = token.trim();
    if !user_t.is_empty() || !token_t.is_empty() {
        crate::credentials::store(&cfg.host_url, user_t, token_t).map_err(|e| match e {
            // Tag an unreachable keystore with the shared marker so the typed frontend error comes
            // out as `keystore` (the marker is stripped before the message is shown).
            crate::credentials::CredentialError::Unavailable(_) => Error::other(format!(
                "{} {e}",
                crate::gitrunner::KEYSTORE_UNAVAILABLE_MARKER
            )),
            other => Error::other(other.to_string()),
        })?;
    }

    // Add the remote, or update its URL if a previous ceremony already set one (idempotent). The
    // URL is the **credential-free** clone URL — no `user:token@` is ever written to `.git/config`.
    if remote_get_url(root).is_some() {
        run_git(root, &["remote", "set-url", REMOTE_NAME, &cfg.clone_url])?;
    } else {
        run_git(root, &["remote", "add", REMOTE_NAME, &cfg.clone_url])?;
    }

    // Enable locksverify for the host (E41). Best-effort: a config write failing must not abort
    // the whole connect step — the remote is the important part, and the user can retry.
    let _ = run_git(
        root,
        &locksverify_config(&cfg.host_url)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );

    read_setup(root)
}

/// Perform the **first push** that publishes the product to the configured remote (E41).
///
/// Pushes the current branch to [`REMOTE_NAME`] and sets it as the upstream so the daily silent
/// sync (a later slice) has a tracking ref to push/pull without ever naming git. Requires a
/// configured remote; refuses clearly otherwise. Returns the refreshed report (now
/// `Eingerichtet`).
pub fn publish_product(root: &Path) -> std::io::Result<SetupReport> {
    let remote = remote_get_url(root).ok_or_else(|| {
        Error::new(
            ErrorKind::Other,
            "Kein Server angebunden — bitte zuerst den Server anbinden.",
        )
    })?;
    // Create the repository on the server *first* — the push assumes it exists, and a fresh
    // owner/repo answers "Not found" otherwise. Idempotent: an existing repo is fine. Uses the
    // token already in the keystore, so the user supplies nothing new here.
    ensure_server_repo(&remote)?;
    let branch = current_branch(root)?;
    run_git(root, &["push", "--set-upstream", REMOTE_NAME, &branch])?;
    read_setup(root)
}

/// Ensure the server-side repository behind the configured remote exists (creating it via the
/// Forgejo/Gitea API), so the first publish push has a target. Reads the credentials from the OS
/// keystore — the same pair the push authenticates with. No stored credentials (the "handled
/// elsewhere" connect path) or a non-API `file://` mirror → skip and let the push proceed as before.
fn ensure_server_repo(remote_url: &str) -> std::io::Result<()> {
    let clone = strip_credentials(remote_url);
    if clone.starts_with("file://") {
        return Ok(()); // local/offline mirror (and the test stand-in) has no REST API
    }
    let Some((host_url, owner, repo)) = crate::forgejo::parse_clone_url(&clone) else {
        // Unrecognisable URL shape: don't block publishing — let the push surface the real error.
        return Ok(());
    };

    use crate::credentials::CredentialError;
    let user = match crate::credentials::username(&host_url) {
        Ok(u) => u,
        // No credential stored (empty-user connect): can't call the API; let the push try.
        Err(CredentialError::NotFound) => return Ok(()),
        Err(e) => return Err(keystore_io_error(e)),
    };
    let token = match crate::credentials::token(&host_url) {
        Ok(t) => t,
        Err(CredentialError::NotFound) => return Ok(()),
        Err(e) => return Err(keystore_io_error(e)),
    };

    crate::forgejo::ensure_repo(&host_url, &owner, &repo, &user, &token)
}

/// Map a keystore failure to an `io::Error`, tagging an unreachable keystore with the shared marker
/// so the typed frontend error comes out as `keystore` (mirrors the tagging in `configure_remote`).
fn keystore_io_error(e: crate::credentials::CredentialError) -> Error {
    match e {
        crate::credentials::CredentialError::Unavailable(_) => {
            Error::other(format!("{} {e}", crate::gitrunner::KEYSTORE_UNAVAILABLE_MARKER))
        }
        other => Error::other(other.to_string()),
    }
}

/// The URL configured for [`REMOTE_NAME`], or `None` if no such remote exists. Public so the lock
/// glue can derive the Forgejo account name from it (Issue #72) instead of inventing its own remote
/// read — there is one source of the remote URL.
pub fn remote_get_url(root: &Path) -> Option<String> {
    let out = crate::gitrunner::command(root)
        .args(["remote", "get-url", REMOTE_NAME])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// Whether the current branch has an upstream / remote-tracking ref — our proxy for "published".
fn branch_has_upstream(root: &Path) -> bool {
    crate::gitrunner::command(root)
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The current branch name (e.g. `main`). Falls back to `main` for a fresh repo on no branch.
fn current_branch(root: &Path) -> std::io::Result<String> {
    let out = crate::gitrunner::command(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if out.status.success() && !name.is_empty() && name != "HEAD" {
        name
    } else {
        "main".to_string()
    })
}

/// Strip any `user[:token]@` credentials from a remote URL so it is safe to display/share as the
/// colleague's clone URL. Pure helper; total (a URL without credentials is returned unchanged).
fn strip_credentials(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => match rest.split_once('@') {
            // Only treat the part before '@' as credentials if it has no path slash in it.
            Some((creds, after)) if !creds.contains('/') => format!("{scheme}://{after}"),
            _ => url.to_string(),
        },
        None => url.to_string(),
    }
}

/// Run a git subcommand in `root`, mapping a non-zero exit to an `io::Error`. Mirrors the helper
/// in `import.rs`; kept local so this isolated glue stays self-contained.
fn run_git(root: &Path, args: &[&str]) -> std::io::Result<()> {
    // Bounded: the publish push reaches the network, and on a rejected credential git-lfs would
    // loop forever. Local subcommands (remote add, config) finish in milliseconds, well under the
    // bound, so this is harmless to them.
    let mut cmd = crate::gitrunner::command(root);
    cmd.args(args);
    let out = crate::gitrunner::output_bounded(&mut cmd)?;
    if !out.status.success() {
        return Err(Error::new(
            ErrorKind::Other,
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- normalize_remote: the validation & normalization core ----

    #[test]
    fn normalize_builds_clone_and_push_urls() {
        let cfg = normalize_remote(
            "https://forge.example.de",
            "team",
            "ember-reverb",
            "anna",
            "tok123",
        )
        .unwrap();
        assert_eq!(cfg.clone_url, "https://forge.example.de/team/ember-reverb.git");
        assert_eq!(cfg.host_url, "https://forge.example.de");
        assert_eq!(
            cfg.push_url,
            "https://anna:tok123@forge.example.de/team/ember-reverb.git"
        );
    }

    #[test]
    fn normalize_defaults_scheme_to_https_and_trims_slashes() {
        let cfg = normalize_remote("forge.example.de/", "t", "p", "", "").unwrap();
        assert_eq!(cfg.host_url, "https://forge.example.de");
        // no credentials -> push url equals the bare clone url
        assert_eq!(cfg.push_url, cfg.clone_url);
        assert_eq!(cfg.clone_url, "https://forge.example.de/t/p.git");
    }

    #[test]
    fn normalize_keeps_http_when_explicit() {
        let cfg = normalize_remote("http://192.168.0.9:3000", "t", "p", "", "").unwrap();
        assert_eq!(cfg.host_url, "http://192.168.0.9:3000");
        assert_eq!(cfg.clone_url, "http://192.168.0.9:3000/t/p.git");
    }

    #[test]
    fn normalize_collapses_repeated_git_suffix() {
        let cfg = normalize_remote("https://h", "o", "repo.git", "", "").unwrap();
        assert_eq!(cfg.clone_url, "https://h/o/repo.git");
    }

    #[test]
    fn normalize_user_only_embeds_user_without_colon() {
        let cfg = normalize_remote("https://h", "o", "p", "anna", "").unwrap();
        assert_eq!(cfg.push_url, "https://anna@h/o/p.git");
    }

    #[test]
    fn normalize_empty_owner_defaults_to_authenticated_user() {
        // Owner left blank → the repo lives under the authenticated user's account. Both the
        // shareable clone URL and the credentialed push URL use the username as the owner.
        let cfg = normalize_remote("https://h", "", "p", "anna", "secret").unwrap();
        assert_eq!(cfg.clone_url, "https://h/anna/p.git");
        assert_eq!(cfg.push_url, "https://anna:secret@h/anna/p.git");
    }

    #[test]
    fn normalize_explicit_owner_overrides_user() {
        // A given owner (a team) is used verbatim, independent of the authenticated user.
        let cfg = normalize_remote("https://h", "team", "p", "anna", "").unwrap();
        assert_eq!(cfg.clone_url, "https://h/team/p.git");
        assert_eq!(cfg.push_url, "https://anna@h/team/p.git");
    }

    #[test]
    fn normalize_percent_encodes_credentials() {
        // an email-style username and a token with reserved chars must be encoded so the URL parses
        let cfg = normalize_remote("https://h", "o", "p", "a@b.de", "x/y@z").unwrap();
        assert_eq!(cfg.push_url, "https://a%40b.de:x%2Fy%40z@h/o/p.git");
        // the clone url shown to colleagues stays clean of credentials
        assert_eq!(cfg.clone_url, "https://h/o/p.git");
    }

    #[test]
    fn normalize_rejects_bad_input() {
        // table: (host, owner, repo, user, token) -> should error
        let bad: &[(&str, &str, &str, &str, &str)] = &[
            ("", "o", "p", "", ""),                      // empty host
            ("   ", "o", "p", "", ""),                   // blank host
            ("ssh://h", "o", "p", "", ""),               // unsupported scheme
            ("https://", "o", "p", "", ""),              // no host part
            ("https://a/b", "o", "p", "", ""),           // host with a path
            ("https://h", "", "p", "", ""),              // empty owner
            ("https://h", "o", "", "", ""),              // empty repo
            ("https://h", "o", ".git", "", ""),          // repo is only the suffix
            ("https://h", "a/b", "p", "", ""),           // owner with slash
            ("https://h", "o", "p p", "", ""),           // repo with whitespace
            ("https://h", "o", "p", "", "tok"),          // token without user
        ];
        for (h, o, r, u, t) in bad {
            assert!(
                normalize_remote(h, o, r, u, t).is_err(),
                "expected error for {:?}",
                (h, o, r, u, t)
            );
        }
    }

    // ---- locksverify_config: the exact git config invocation ----

    #[test]
    fn locksverify_config_targets_the_host_lfs_endpoint() {
        assert_eq!(
            locksverify_config("https://forge.example.de"),
            vec![
                "config",
                "--local",
                "lfs.https://forge.example.de/info/lfs.locksverify",
                "true",
            ]
        );
    }

    #[test]
    fn locksverify_config_trims_trailing_slash() {
        let args = locksverify_config("https://h/");
        assert_eq!(args[2], "lfs.https://h/info/lfs.locksverify");
        assert_eq!(args[3], "true");
    }

    // ---- decide_setup_state: the one-time / already-configured state machine ----

    #[test]
    fn decide_setup_state_covers_the_cross_product() {
        // table: (has_remote, has_published) -> stage
        let cases: &[(bool, bool, SetupStage)] = &[
            (false, false, SetupStage::NotConfigured),
            (false, true, SetupStage::NotConfigured), // no remote wins -> re-run ceremony
            (true, false, SetupStage::RemoteSetNotPublished),
            (true, true, SetupStage::Eingerichtet),
        ];
        for (has_remote, has_published, expected) in cases {
            assert_eq!(
                decide_setup_state(SetupFacts {
                    has_remote: *has_remote,
                    has_published: *has_published,
                }),
                *expected,
                "decide_setup_state({has_remote}, {has_published})"
            );
        }
    }

    #[test]
    fn ceremony_shows_only_when_not_configured() {
        // The ceremony is one-time: it is offered iff no remote is configured.
        for has_published in [false, true] {
            let s = decide_setup_state(SetupFacts {
                has_remote: false,
                has_published,
            });
            assert_eq!(s, SetupStage::NotConfigured);
        }
        // Once a remote exists, the ceremony is never re-offered as NotConfigured.
        for has_published in [false, true] {
            let s = decide_setup_state(SetupFacts {
                has_remote: true,
                has_published,
            });
            assert_ne!(s, SetupStage::NotConfigured);
        }
    }

    // ---- strip_credentials: the colleague-safe clone URL ----

    #[test]
    fn strip_credentials_removes_userinfo() {
        assert_eq!(
            strip_credentials("https://anna:tok@h/o/p.git"),
            "https://h/o/p.git"
        );
        assert_eq!(strip_credentials("https://anna@h/o/p.git"), "https://h/o/p.git");
        // already clean: unchanged
        assert_eq!(strip_credentials("https://h/o/p.git"), "https://h/o/p.git");
        // a path containing '@' but no userinfo is left intact
        assert_eq!(strip_credentials("https://h/o/p@v1.git"), "https://h/o/p@v1.git");
    }
}
