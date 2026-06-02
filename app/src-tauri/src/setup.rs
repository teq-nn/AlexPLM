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
/// by [`normalize_remote`]. Since ADR 0004 the ceremony is **credential-free** — the Konto is the
/// sole writer of credentials (host-keyed in the OS keystore) — so this carries only the bare,
/// shareable clone URL and the host origin; no `user:token@` URL is ever built here.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteConfig {
    /// The bare `https://host/owner/repo.git` URL with NO credentials — what `.git/config` gets
    /// (Issue #22: credentials live in the OS keystore, never in the URL) and what a colleague
    /// clones (the invite). Never carries a secret.
    pub clone_url: String,
    /// The `https://host` origin (no owner/repo). Carried for callers that need the bare host;
    /// `locksverify` is scoped to the repo's LFS endpoint via [`clone_url`](Self::clone_url), not
    /// this (Issue #110).
    pub host_url: String,
}

/// Validate and normalize the fields the ceremony provides into a [`RemoteConfig`].
///
/// Pure and total: never shells out, never panics, returns a human German error string on bad
/// input. Since ADR 0004 (Issue #92) this is **credential-free** — the Konto is the sole writer of
/// credentials, so `normalize_remote` no longer takes a `user`/`token` and never builds a
/// credentialed URL. Rules (deliberately strict so a typo fails loud here, not mid-push):
/// - `host` must be a non-empty `http(s)://host[:port]` with a host part (it comes from the Konto
///   Base-URL, already normalized). A bare `host:port` without scheme is defaulted to `https://`
///   (the safe Forgejo/Gitea default).
/// - `repo` must be non-empty and free of slashes/whitespace/control characters.
/// - `owner` is optional: left empty it defaults to `default_owner` (the Konto username — publishing
///   under one's own account). The effective owner must be free of slashes/whitespace/control chars.
/// - the `.git` suffix on `repo` is optional and normalized to exactly one.
pub fn normalize_remote(
    host: &str,
    owner: &str,
    repo: &str,
    default_owner: &str,
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

    // The owner is optional: left blank it defaults to `default_owner` (the Konto username), so a
    // user publishing under their own account need not repeat their name. A given owner (e.g. an
    // organization) is used verbatim. Validate the chosen owner the same way regardless of source.
    let owner = if owner.trim().is_empty() {
        clean_segment(default_owner, "Besitzer/Organisation")?
    } else {
        clean_segment(owner, "Besitzer/Organisation")?
    };

    let clone_url = format!("{host_url}/{owner}/{repo}.git");

    Ok(RemoteConfig {
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

// ----------------------------------------------------------------------------------------------
// Pure core 2 — the `locksverify` config invocation (E41 / Realitätsbefund)
// ----------------------------------------------------------------------------------------------

/// The exact `git config` arguments that switch on LFS lock verification (E41: "`locksverify`
/// muss aktiv eingeschaltet werden"). Pure: builds `git config --local
/// lfs.<repo>.git/info/lfs.locksverify true`.
///
/// git-lfs keys `locksverify` **per LFS endpoint**, not per host (Issue #110): the endpoint for a
/// Forgejo/Gitea repo is the clone URL with `/info/lfs` appended — `https://host/owner/repo.git/info/lfs`.
/// Scoping the config to the bare host (`lfs.https://host/info/lfs.locksverify`) writes a key
/// git-lfs never reads, so verification stays silently **off**. So this takes the repo's clone URL,
/// not the host origin. Kept pure so the precise key string is asserted by a unit test rather than
/// discovered in production.
pub fn locksverify_config(clone_url: &str) -> Vec<String> {
    vec![
        "config".into(),
        "--local".into(),
        format!("lfs.{}/info/lfs.locksverify", clone_url.trim_end_matches('/')),
        "true".into(),
    ]
}

// ----------------------------------------------------------------------------------------------
// Pure core 3 — the one-time / already-configured state machine
// ----------------------------------------------------------------------------------------------

/// Where a product stands in the ceremony. Serialised kebab-case for the UI, which shows the
/// ceremony **only** in `NotConfigured` and a settled "eingerichtet" readout otherwise — so the
/// one-time flow is clearly separated from daily use (E41).
#[derive(specta::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
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

/// The outcome of a publish attempt (Issue #44). Parallels [`crate::syncglue::SyncOutcome`]: most of
/// the time the product publishes and the ceremony advances; but if the chosen Server-Repo already
/// carries Stände that contradict the local product on an **unmergeable** artifact, publishing
/// **stops** and raises the single domain-language exception instead of letting git reject the push.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PublishOutcome {
    /// The product was published; the refreshed ceremony state (now `eingerichtet`) rides along.
    Published(SetupReport),
    /// Integrating the non-empty Server-Repo hit a real contradiction on an unmergeable artifact.
    /// The push was **not** performed; the user answers the loud question (via the same
    /// `resolve_sync` as the daily sync), then re-publishes — the re-push is then a clean
    /// fast-forward. Carries the domain-language question; never a git marker.
    LauteAusnahme(crate::syncdecider::LoudQuestion),
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
/// Since ADR 0004 (Issue #92) this is **credential-free**: the Konto is the sole writer of
/// credentials (host-keyed in the OS keystore at Konto-save time), so the `host` here is the Konto
/// Base-URL and `default_owner` the Konto username; nothing is written to the keystore. Validates
/// /normalizes the fields with the pure [`normalize_remote`] first (a typo fails here, before any
/// git call), then adds/updates the remote named [`REMOTE_NAME`] with the credential-free clone URL
/// and switches on LFS lock verification for the host. Returns the refreshed [`SetupReport`] so the
/// UI advances to the publish step in one round-trip. Does **not** push — the deliberate next step.
pub fn configure_remote(
    root: &Path,
    host: &str,
    owner: &str,
    repo: &str,
    default_owner: &str,
) -> std::io::Result<SetupReport> {
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Kein Ordner: {}", root.display()),
        ));
    }
    let cfg = normalize_remote(host, owner, repo, default_owner)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;

    // Add the remote, or update its URL if a previous ceremony already set one (idempotent). The
    // URL is the **credential-free** clone URL — no `user:token@` is ever written to `.git/config`.
    if remote_get_url(root).is_some() {
        run_git(root, &["remote", "set-url", REMOTE_NAME, &cfg.clone_url])?;
    } else {
        run_git(root, &["remote", "add", REMOTE_NAME, &cfg.clone_url])?;
    }

    // Enable locksverify for the repo's LFS endpoint (E41, Issue #110). Best-effort: a config write
    // failing must not abort the whole connect step — the remote is the important part, and the
    // user can retry.
    let _ = run_git(
        root,
        &locksverify_config(&cfg.clone_url)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );

    read_setup(root)
}

/// Perform the **first push** that publishes the product to the configured remote (E41, Issue #44).
///
/// Pushes the current branch to [`REMOTE_NAME`] and sets it as the upstream so the daily silent
/// sync has a tracking ref to push/pull without ever naming git. Requires a configured remote;
/// refuses clearly otherwise.
///
/// **Non-empty Server-Repo (Issue #44, supersedes #35):** before the push, integrate whatever the
/// chosen Server-Repo already carries — the blind push would otherwise be rejected as a raw
/// non-fast-forward. [`crate::syncglue::integrate_for_publish`] reuses the daily Sync Decider:
/// mergeable Stände are folded in silently and the push proceeds; a real contradiction on an
/// unmergeable artifact stops here and rides back as [`PublishOutcome::LauteAusnahme`] (no push, no
/// raw git text). `other` names a colleague in that question (`None` → neutral fallback).
pub fn publish_product(root: &Path, other: Option<String>) -> std::io::Result<PublishOutcome> {
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

    // Integrate a non-empty Server-Repo before pushing (Issue #44). On a real contradiction, stop
    // and hand the loud question to the ceremony — the push is deferred until the user resolves it.
    if let Some(loud) = crate::syncglue::integrate_for_publish(root, other)? {
        return Ok(PublishOutcome::LauteAusnahme(loud));
    }

    let branch = current_branch(root)?;
    run_git(root, &["push", "--set-upstream", REMOTE_NAME, &branch])?;
    Ok(PublishOutcome::Published(read_setup(root)?))
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

/// Whether the product is **published** — its current branch has an upstream tracking ref. This is
/// our proxy for "a repository for this product exists on the server": the ceremony's first publish
/// ([`publish_product`]) creates the server repo *and* sets this upstream, and a colleague's `git
/// clone` sets it too. The networked daily rhythm — the silent sync's `fetch`, the Status-Reader's
/// `git lfs locks`, the Lock Warden's Sicherungs-Push — is meaningless before this point: there is
/// no server-side repo yet, so a `fetch` 404s, a push answers „Push to create is not enabled", and
/// (worst) `git lfs locks` only loops on the **401** Forgejo's LFS endpoint returns for an absent
/// repo (it authenticates before checking existence), wedging the bounded git call for its full
/// timeout on every status tick. Gating the rhythm on this keeps an unpublished product silent
/// (E41) until it has a server to talk to. Reuses the same upstream check [`read_setup`] reads for
/// `has_published`, so "published" means one thing across the app.
pub fn is_published(root: &Path) -> bool {
    branch_has_upstream(root)
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
    fn normalize_builds_credential_free_clone_url() {
        // Since ADR 0004 the ceremony is credential-free: only the bare clone URL + host origin are
        // built, never a `user:token@` URL — the Konto is the sole writer of credentials.
        let cfg =
            normalize_remote("https://forge.example.de", "team", "ember-reverb", "anna").unwrap();
        assert_eq!(cfg.clone_url, "https://forge.example.de/team/ember-reverb.git");
        assert_eq!(cfg.host_url, "https://forge.example.de");
        // the clone URL never carries credentials
        assert!(!cfg.clone_url.contains('@'));
    }

    #[test]
    fn normalize_defaults_scheme_to_https_and_trims_slashes() {
        let cfg = normalize_remote("forge.example.de/", "t", "p", "").unwrap();
        assert_eq!(cfg.host_url, "https://forge.example.de");
        assert_eq!(cfg.clone_url, "https://forge.example.de/t/p.git");
    }

    #[test]
    fn normalize_keeps_http_when_explicit() {
        let cfg = normalize_remote("http://192.168.0.9:3000", "t", "p", "").unwrap();
        assert_eq!(cfg.host_url, "http://192.168.0.9:3000");
        assert_eq!(cfg.clone_url, "http://192.168.0.9:3000/t/p.git");
    }

    #[test]
    fn normalize_collapses_repeated_git_suffix() {
        let cfg = normalize_remote("https://h", "o", "repo.git", "").unwrap();
        assert_eq!(cfg.clone_url, "https://h/o/repo.git");
    }

    #[test]
    fn normalize_empty_owner_defaults_to_konto_username() {
        // Owner left blank → the repo lives under the Konto user's account (the passed default
        // owner). The shareable clone URL uses that username as the owner.
        let cfg = normalize_remote("https://h", "", "p", "anna").unwrap();
        assert_eq!(cfg.clone_url, "https://h/anna/p.git");
    }

    #[test]
    fn normalize_explicit_owner_overrides_default() {
        // A given owner (a team/organization) is used verbatim, independent of the Konto username.
        let cfg = normalize_remote("https://h", "team", "p", "anna").unwrap();
        assert_eq!(cfg.clone_url, "https://h/team/p.git");
    }

    #[test]
    fn normalize_rejects_bad_input() {
        // table: (host, owner, repo, default_owner) -> should error
        let bad: &[(&str, &str, &str, &str)] = &[
            ("", "o", "p", ""),             // empty host
            ("   ", "o", "p", ""),          // blank host
            ("ssh://h", "o", "p", ""),      // unsupported scheme
            ("https://", "o", "p", ""),     // no host part
            ("https://a/b", "o", "p", ""),  // host with a path
            ("https://h", "", "p", ""),     // empty owner AND empty default owner
            ("https://h", "o", "", ""),     // empty repo
            ("https://h", "o", ".git", ""), // repo is only the suffix
            ("https://h", "a/b", "p", ""),  // owner with slash
            ("https://h", "o", "p p", ""),  // repo with whitespace
        ];
        for (h, o, r, d) in bad {
            assert!(
                normalize_remote(h, o, r, d).is_err(),
                "expected error for {:?}",
                (h, o, r, d)
            );
        }
    }

    // ---- locksverify_config: the exact git config invocation ----

    #[test]
    fn locksverify_config_targets_the_repo_lfs_endpoint() {
        // Issue #110: git-lfs keys locksverify per LFS endpoint (the clone URL + /info/lfs), not
        // per host. Scoping to the bare host writes a key git-lfs never reads → verification off.
        assert_eq!(
            locksverify_config("https://forge.example.de/team/ember-reverb.git"),
            vec![
                "config",
                "--local",
                "lfs.https://forge.example.de/team/ember-reverb.git/info/lfs.locksverify",
                "true",
            ]
        );
    }

    #[test]
    fn locksverify_config_matches_the_normalized_clone_url() {
        // The key must target exactly the clone URL `configure_remote` writes as the remote, so
        // git-lfs (which derives its endpoint from that same remote URL) actually reads it.
        let cfg =
            normalize_remote("https://forge.example.de", "team", "ember-reverb", "anna").unwrap();
        let args = locksverify_config(&cfg.clone_url);
        assert_eq!(args[2], format!("lfs.{}/info/lfs.locksverify", cfg.clone_url));
    }

    #[test]
    fn locksverify_config_trims_trailing_slash() {
        let args = locksverify_config("https://h/o/p.git/");
        assert_eq!(args[2], "lfs.https://h/o/p.git/info/lfs.locksverify");
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

    // ---- PublishOutcome: the frontend wire contract (Issue #44) ----

    /// The `PublishOutcome` serialises with the internal `kind` tag and the inner struct's fields
    /// flattened alongside it — exactly the shape `PublishOutcome` in `types.ts` reads. A drift here
    /// would silently break the ceremony's publish handling, so pin the wire shape.
    #[test]
    fn publish_outcome_serialises_with_kind_tag_and_flattened_fields() {
        let published = PublishOutcome::Published(SetupReport {
            stage: SetupStage::Eingerichtet,
            has_remote: true,
            has_published: true,
            clone_url: Some("https://h/o/p.git".to_string()),
        });
        let v: serde_json::Value = serde_json::to_value(&published).unwrap();
        assert_eq!(v["kind"], "published");
        assert_eq!(v["stage"], "eingerichtet"); // SetupReport fields ride alongside `kind`
        assert_eq!(v["has_published"], true);

        let loud = PublishOutcome::LauteAusnahme(crate::syncdecider::LoudQuestion {
            frage: "dein und Bens Gehäuse-Stand widersprechen sich — welcher gilt?".to_string(),
            artefakte: vec!["mechanik/gehaeuse.f3d".to_string()],
            optionen: vec![],
        });
        let v: serde_json::Value = serde_json::to_value(&loud).unwrap();
        assert_eq!(v["kind"], "laute-ausnahme");
        assert!(v["frage"].as_str().unwrap().contains("welcher gilt"));
        assert!(v["artefakte"].is_array());
    }
}
