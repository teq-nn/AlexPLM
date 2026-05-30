//! The `GIT_ASKPASS` helper mode of the app binary (Issue #22).
//!
//! When git needs a username or password for an HTTPS remote it runs the program named by
//! `GIT_ASKPASS` with the human prompt as its single argument and reads the answer from stdout.
//! [`crate::gitrunner`] points `GIT_ASKPASS` at **our own binary** and tags the child with the
//! [`ASKPASS_ENV`](crate::gitrunner::ASKPASS_ENV) marker; `main` checks [`is_askpass_invocation`]
//! on startup and, when set, runs [`run`] instead of launching the GUI. This is the whole reason
//! the credential prompt never reaches the terminal: git asks us, and we answer from the OS
//! keystore. The same binary is the helper on Windows and Linux — no `.sh`/`.bat`.
//!
//! The prompt parsing is pure and table-tested ([`parse_prompt`]); [`answer`] adds the keystore
//! read; [`run`] is the thin stdout/stderr/exit-code shell. A missing token or an unreachable
//! keystore makes git **fail fast** (non-zero exit, nothing on stdout) rather than hang — and an
//! unreachable keystore is tagged on stderr so the caller can tell the two apart.

use crate::credentials::{self, CredentialError};
use crate::gitrunner::{ASKPASS_ENV, KEYSTORE_UNAVAILABLE_MARKER};

/// Which secret git is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// `Username for 'https://host'` → the git username.
    Username,
    /// `Password for 'https://user@host'` → the access token.
    Password,
}

/// Why the askpass helper could not produce an answer.
#[derive(Debug)]
pub enum AskpassError {
    /// The prompt was not a recognizable git username/password request.
    Unparsable,
    /// The keystore lookup failed (not found, unavailable, or other).
    Credential(CredentialError),
}

/// Is this process being run by git as its askpass helper? True iff the env marker
/// [`ASKPASS_ENV`] is present (git propagates the env we set on the git command to this child).
pub fn is_askpass_invocation() -> bool {
    std::env::var_os(ASKPASS_ENV).is_some()
}

/// Parse a git askpass prompt into the requested [`Field`] and the **host origin** to look up
/// (`https://host[:port]`, with any `user@` userinfo stripped so it matches the ceremony's stored
/// key). Pure and total; returns `None` for anything that isn't a username/password prompt.
pub fn parse_prompt(prompt: &str) -> Option<(Field, String)> {
    let p = prompt.trim();
    let lower = p.to_ascii_lowercase();
    let field = if lower.starts_with("username") {
        Field::Username
    } else if lower.starts_with("password") {
        Field::Password
    } else {
        return None;
    };

    // The URL git shows is wrapped in single quotes: `Username for 'https://host': `.
    let open = p.find('\'')?;
    let rest = &p[open + 1..];
    let close = rest.find('\'')?;
    let url = &rest[..close];
    if url.is_empty() {
        return None;
    }
    Some((field, strip_userinfo(url)))
}

/// Strip any `user[:pw]@` userinfo from a URL, leaving the bare `scheme://host[:port]` origin.
/// Pure and total (a URL without userinfo is returned trimmed of a trailing slash).
fn strip_userinfo(url: &str) -> String {
    let url = url.trim_end_matches('/');
    match url.split_once("://") {
        Some((scheme, rest)) => match rest.split_once('@') {
            // Only treat the part before '@' as userinfo if it has no path slash.
            Some((creds, after)) if !creds.contains('/') => format!("{scheme}://{after}"),
            _ => url.to_string(),
        },
        None => url.to_string(),
    }
}

/// Produce the answer to a git askpass `prompt` by reading the OS keystore. The credential half of
/// the helper, separated from I/O so it is testable against a mock keystore without capturing
/// stdout.
pub fn answer(prompt: &str) -> Result<String, AskpassError> {
    let (field, host) = parse_prompt(prompt).ok_or(AskpassError::Unparsable)?;
    let secret = match field {
        Field::Username => credentials::username(&host),
        Field::Password => credentials::token(&host),
    };
    secret.map_err(AskpassError::Credential)
}

/// Run the askpass helper end to end: print the answer to stdout for git, or fail fast. Returns the
/// process exit code `main` should exit with.
///
/// - answer found → print it, exit `0`.
/// - keystore unreachable → print the [`KEYSTORE_UNAVAILABLE_MARKER`] to **stderr** (so the caller
///   can classify it), exit `1`.
/// - not found / unparsable → print nothing, exit `1`. With `GIT_TERMINAL_PROMPT=0` git then fails
///   immediately with an auth error instead of hanging on a hidden terminal prompt.
pub fn run(prompt: Option<&str>) -> i32 {
    let Some(prompt) = prompt else {
        return 1;
    };
    match answer(prompt) {
        Ok(secret) => {
            println!("{secret}");
            0
        }
        Err(AskpassError::Credential(CredentialError::Unavailable(_))) => {
            eprintln!("{KEYSTORE_UNAVAILABLE_MARKER}");
            1
        }
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prompt_reads_field_and_host() {
        // table: prompt -> (field, host)
        let cases: &[(&str, Field, &str)] = &[
            ("Username for 'https://forge.example.de': ", Field::Username, "https://forge.example.de"),
            ("Password for 'https://anna@forge.example.de': ", Field::Password, "https://forge.example.de"),
            // host with a port is kept; userinfo with a colon is stripped whole
            ("Password for 'http://u:x@192.168.0.9:3000': ", Field::Password, "http://192.168.0.9:3000"),
            // trailing slash on the shown URL is normalized away
            ("Username for 'https://h/': ", Field::Username, "https://h"),
        ];
        for (prompt, field, host) in cases {
            let (f, h) = parse_prompt(prompt).expect("parses");
            assert_eq!(f, *field, "field for {prompt:?}");
            assert_eq!(h, *host, "host for {prompt:?}");
        }
    }

    #[test]
    fn parse_prompt_rejects_non_credential_prompts() {
        assert!(parse_prompt("Are you sure? (yes/no)").is_none());
        assert!(parse_prompt("Username for ''").is_none()); // empty URL
        assert!(parse_prompt("Username for https://no-quotes").is_none());
        assert!(parse_prompt("").is_none());
    }

    #[test]
    fn run_with_no_prompt_fails_fast() {
        assert_eq!(run(None), 1);
    }
}
