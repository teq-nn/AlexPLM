//! Minimal Forgejo/Gitea REST client for the one thing the ceremony was missing: **creating the
//! product's repository on the server** before the first publish push (Issue #5 follow-up).
//!
//! The previous `publish_product` only ran `git push`, which assumes the repo already exists
//! server-side; against a fresh `owner/repo` Forgejo answers `remote: Not found`. This module's one
//! effectful call — [`ensure_repo`] — POSTs to the create-repo endpoint using the **same token the
//! push uses** (already in the OS keystore from the connect step), so the user supplies nothing new.
//! It is **idempotent**: a repo that already exists (re-publishing, or a colleague who created it)
//! is treated as success.
//!
//! House pattern: the **decisions** are a pure, total, table-testable core ([`parse_clone_url`],
//! [`create_endpoint`], [`interpret_status`]); the thin side-effecting layer ([`ensure_repo`]) is
//! the only part that touches the network. Tests drive the core directly and never hit a server.

use std::io::{Error, ErrorKind};
use std::time::Duration;

/// Wall-clock bound on the create-repo call — mirrors `gitrunner::NETWORK_TIMEOUT` so a wedged
/// server can never hang the (off-main-thread) publish.
const API_TIMEOUT: Duration = Duration::from_secs(20);

/// Split a credential-free clone URL (`scheme://host[:port]/owner/repo.git`, as produced by
/// `setup::normalize_remote`) back into `(host_url, owner, repo)`. Pure, total: returns `None` for a
/// shape we did not produce. The `.git` suffix is optional; the host origin keeps any port.
pub fn parse_clone_url(url: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let (hostport, path) = rest.split_once('/')?;
    if hostport.is_empty() {
        return None;
    }
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    // owner/repo: repo is the last segment, owner everything before the final slash. `normalize_remote`
    // forbids slashes in either, so a well-formed URL has exactly `owner/repo` here.
    let (owner, repo) = path.rsplit_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((format!("{scheme}://{hostport}"), owner.to_string(), repo.to_string()))
}

/// Which create-repo endpoint to POST to. Pure. When `owner` is the authenticated user themselves
/// the repo is created under their account (`/api/v1/user/repos`); otherwise `owner` is treated as
/// an **organisation** (`/api/v1/orgs/{owner}/repos`) the user has rights to create in. Forgejo
/// usernames are case-insensitive, so the comparison is too.
pub fn create_endpoint(host_url: &str, owner: &str, user: &str) -> String {
    let base = host_url.trim_end_matches('/');
    if owner.trim().eq_ignore_ascii_case(user.trim()) {
        format!("{base}/api/v1/user/repos")
    } else {
        format!("{base}/api/v1/orgs/{owner}/repos")
    }
}

/// Interpret the API response status + body into success or a typed German error. Pure, total.
///
/// **Idempotent:** `409 Conflict` (and a `422` whose body says the repo already exists) mean the
/// repo is already there — exactly what we want, so they are success. Auth failures keep the verbatim
/// `401 unauthorized` / `403 forbidden` marker in the message so `gitrunner::classify_failure` maps
/// them to the credential field (the same path a rejected push token takes).
pub fn interpret_status(code: u16, body: &str) -> Result<(), String> {
    let low = body.to_ascii_lowercase();
    match code {
        200 | 201 => Ok(()),
        409 => Ok(()), // already exists — idempotent success
        422 if low.contains("already exist") => Ok(()),
        401 => Err(format!("401 unauthorized: {}", body.trim())),
        403 => Err(format!(
            "403 forbidden — keine Berechtigung, hier ein Repository anzulegen: {}",
            body.trim()
        )),
        404 => Err(format!(
            "Besitzer/Organisation auf dem Server nicht gefunden: {}",
            body.trim()
        )),
        other => Err(format!(
            "Konnte das Produkt nicht auf dem Server anlegen (HTTP {other}): {}",
            body.trim()
        )),
    }
}

/// The create-repo request body. `private` is the safe default for an engineering product; the user
/// can open it up in Forgejo. `auto_init: false` keeps the server repo empty so the first push is a
/// clean fast-forward (an auto-initialised repo would diverge and reject the push).
#[derive(serde::Serialize)]
struct CreateRepoBody<'a> {
    name: &'a str,
    private: bool,
    auto_init: bool,
}

/// Ensure `owner/repo` exists on the Forgejo/Gitea server at `host_url`, creating it if needed.
/// Idempotent (an existing repo is success). Authenticates with the same `user`/`token` the push
/// uses (Basic auth — a Forgejo access token is a valid API credential and git password alike).
///
/// Errors ride up as `io::Error` so `publish_product` can surface them through the existing typed
/// `AppError` path; a bad token yields an auth-classified message that reopens the credential field.
pub fn ensure_repo(
    host_url: &str,
    owner: &str,
    repo: &str,
    user: &str,
    token: &str,
) -> std::io::Result<()> {
    let endpoint = create_endpoint(host_url, owner, user);
    let client = reqwest::blocking::Client::builder()
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, format!("HTTP-Client-Fehler: {e}")))?;
    let resp = client
        .post(&endpoint)
        .basic_auth(user, Some(token))
        .json(&CreateRepoBody {
            name: repo,
            private: true,
            auto_init: false,
        })
        .send()
        .map_err(|e| Error::new(ErrorKind::Other, format!("Server nicht erreichbar: {e}")))?;
    let code = resp.status().as_u16();
    let body = resp.text().unwrap_or_default();
    interpret_status(code, &body).map_err(|m| Error::new(ErrorKind::Other, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clone_url_splits_host_owner_repo() {
        assert_eq!(
            parse_clone_url("https://forgejo.nilius.online/alice/gizmo.git"),
            Some((
                "https://forgejo.nilius.online".to_string(),
                "alice".to_string(),
                "gizmo".to_string()
            ))
        );
    }

    #[test]
    fn parse_clone_url_keeps_port_and_optional_git_suffix() {
        assert_eq!(
            parse_clone_url("http://h:3000/org/repo"),
            Some(("http://h:3000".to_string(), "org".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn parse_clone_url_rejects_malformed() {
        assert_eq!(parse_clone_url("not a url"), None);
        assert_eq!(parse_clone_url("https://hostonly"), None); // no path
        assert_eq!(parse_clone_url("https:///owner/repo.git"), None); // empty host
    }

    #[test]
    fn create_endpoint_user_vs_org() {
        // owner == authenticated user -> personal repo endpoint (case-insensitive).
        assert_eq!(
            create_endpoint("https://h", "Alice", "alice"),
            "https://h/api/v1/user/repos"
        );
        // owner != user -> treated as an organisation.
        assert_eq!(
            create_endpoint("https://h/", "acme", "alice"),
            "https://h/api/v1/orgs/acme/repos"
        );
    }

    #[test]
    fn interpret_status_is_idempotent_on_exists() {
        assert!(interpret_status(201, "").is_ok());
        assert!(interpret_status(200, "").is_ok());
        assert!(interpret_status(409, "The repository already exists.").is_ok());
        assert!(interpret_status(422, r#"{"message":"repo already exists"}"#).is_ok());
    }

    #[test]
    fn interpret_status_auth_errors_carry_classifiable_markers() {
        // The messages must contain the markers `gitrunner::classify_failure` keys on, so a bad
        // token reopens the credential field instead of reading as a generic error.
        let e401 = interpret_status(401, "bad token").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e401), crate::gitrunner::GitFailure::Auth);
        let e403 = interpret_status(403, "no perms").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e403), crate::gitrunner::GitFailure::Auth);
    }

    #[test]
    fn interpret_status_other_is_not_auth() {
        let e = interpret_status(500, "boom").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e), crate::gitrunner::GitFailure::Other);
        assert!(e.contains("500"));
    }
}
