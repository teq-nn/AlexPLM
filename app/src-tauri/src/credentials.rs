//! The OS-keystore wrapper for the auth token (Issue #22).
//!
//! The token (and the git username it pairs with) live in the **native OS keystore** — GNOME
//! Keyring / KWallet via the Secret Service on Linux, the Credential Manager on Windows — never in
//! `.git/config`. The ceremony writes them here ([`store`]); the askpass helper reads them back at
//! the moment git asks ([`username`] / [`token`]).
//!
//! Each product host gets two keystore entries, keyed by the host origin (`https://host[:port]`)
//! so the askpass helper — which only sees the host in git's prompt — can find them:
//! - service [`TOKEN_SERVICE`] → the access token,
//! - service [`USERNAME_SERVICE`] → the git username.
//!
//! All keystore failures are mapped to a typed [`CredentialError`]; nothing here panics, so a
//! missing Secret Service daemon on Linux surfaces as [`CredentialError::Unavailable`] and rides a
//! clean error up to the frontend instead of crashing the app.

use keyring::Entry;

/// Keystore service name for the access token.
const TOKEN_SERVICE: &str = "PLM-Werkzeug";
/// Keystore service name for the git username (kept apart from the token).
const USERNAME_SERVICE: &str = "PLM-Werkzeug.username";

/// Why a keystore operation failed, in terms the frontend can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    /// The keystore itself is unreachable (e.g. no Secret Service daemon running on Linux). The
    /// user can fix the environment and retry; this is never a crash.
    Unavailable(String),
    /// No credential is stored for this host yet (a fresh product, or after [`delete`]).
    NotFound,
    /// Any other keystore error.
    Other(String),
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialError::Unavailable(d) => {
                write!(f, "OS-Schlüsselbund nicht erreichbar: {d}")
            }
            CredentialError::NotFound => write!(f, "Keine Zugangsdaten hinterlegt."),
            CredentialError::Other(d) => write!(f, "Schlüsselbund-Fehler: {d}"),
        }
    }
}

impl std::error::Error for CredentialError {}

/// Normalize a host origin so the ceremony's `store` key and the askpass `token`/`username` lookup
/// key always match: trim a trailing slash. Pure.
fn host_key(host_url: &str) -> String {
    host_url.trim().trim_end_matches('/').to_string()
}

/// Store the username and token for `host_url` in the OS keystore (overwriting any previous pair).
/// `host_url` is the origin `https://host[:port]`.
pub fn store(host_url: &str, user: &str, token: &str) -> Result<(), CredentialError> {
    let host = host_key(host_url);
    username_entry(&host)?.set_password(user).map_err(map_err)?;
    token_entry(&host)?.set_password(token).map_err(map_err)?;
    Ok(())
}

/// Read the git username stored for `host_url`.
pub fn username(host_url: &str) -> Result<String, CredentialError> {
    token_or_user(&host_key(host_url), Field::Username)
}

/// Read the access token stored for `host_url`.
pub fn token(host_url: &str) -> Result<String, CredentialError> {
    token_or_user(&host_key(host_url), Field::Token)
}

/// Remove both stored entries for `host_url`. A missing entry is treated as already-removed.
pub fn delete(host_url: &str) -> Result<(), CredentialError> {
    let host = host_key(host_url);
    forget(username_entry(&host)?)?;
    forget(token_entry(&host)?)?;
    Ok(())
}

enum Field {
    Username,
    Token,
}

fn token_or_user(host: &str, field: Field) -> Result<String, CredentialError> {
    let entry = match field {
        Field::Username => username_entry(host)?,
        Field::Token => token_entry(host)?,
    };
    entry.get_password().map_err(map_err)
}

fn token_entry(host: &str) -> Result<Entry, CredentialError> {
    Entry::new(TOKEN_SERVICE, host).map_err(map_err)
}

fn username_entry(host: &str) -> Result<Entry, CredentialError> {
    Entry::new(USERNAME_SERVICE, host).map_err(map_err)
}

/// Delete one entry, treating "already gone" as success (idempotent).
fn forget(entry: Entry) -> Result<(), CredentialError> {
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(map_err(e)),
    }
}

/// Map a `keyring` error to our typed [`CredentialError`]. A missing entry is [`NotFound`]; a
/// storage/platform failure (no daemon, D-Bus down) is [`Unavailable`]; the rest are [`Other`].
fn map_err(e: keyring::Error) -> CredentialError {
    use keyring::Error as K;
    match e {
        K::NoEntry => CredentialError::NotFound,
        K::NoStorageAccess(inner) => CredentialError::Unavailable(inner.to_string()),
        K::PlatformFailure(inner) => CredentialError::Unavailable(inner.to_string()),
        other => CredentialError::Other(other.to_string()),
    }
}

/// Install an in-process, shared-map keystore for tests, replacing the real OS keystore for this
/// process. Hidden test support: lives here so it uses the **same** `keyring` instance as the
/// production code above (an integration-test crate would otherwise get a second, incompatible
/// `keyring` crate). Unlike the crate's built-in `mock`, this shares one map across every
/// `Entry::new`, so a `store` then `username`/`token` round-trips like a real keystore. Never
/// touches the OS Secret Service / Credential Manager. Idempotent.
#[doc(hidden)]
pub fn install_in_memory_keystore_for_tests() {
    use keyring::credential::{CredentialApi, CredentialBuilderApi};
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::{Mutex, Once, OnceLock};

    fn map() -> &'static Mutex<HashMap<(String, String), String>> {
        static MAP: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();
        MAP.get_or_init(|| Mutex::new(HashMap::new()))
    }

    #[derive(Debug)]
    struct Cred {
        key: (String, String),
    }
    impl CredentialApi for Cred {
        fn set_password(&self, password: &str) -> keyring::Result<()> {
            map().lock().unwrap().insert(self.key.clone(), password.to_string());
            Ok(())
        }
        fn get_password(&self) -> keyring::Result<String> {
            map().lock().unwrap().get(&self.key).cloned().ok_or(keyring::Error::NoEntry)
        }
        fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
            self.set_password(&String::from_utf8_lossy(secret))
        }
        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            self.get_password().map(String::into_bytes)
        }
        fn delete_credential(&self) -> keyring::Result<()> {
            match map().lock().unwrap().remove(&self.key) {
                Some(_) => Ok(()),
                None => Err(keyring::Error::NoEntry),
            }
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn debug_fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "InMemoryCred({:?})", self.key)
        }
    }

    #[derive(Debug)]
    struct Builder;
    impl CredentialBuilderApi for Builder {
        fn build(
            &self,
            _target: Option<&str>,
            service: &str,
            user: &str,
        ) -> keyring::Result<Box<keyring::Credential>> {
            Ok(Box::new(Cred {
                key: (service.to_string(), user.to_string()),
            }))
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        keyring::set_default_credential_builder(Box::new(Builder));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_key_trims_trailing_slash_so_store_and_lookup_match() {
        assert_eq!(host_key("https://forge.example.de/"), "https://forge.example.de");
        assert_eq!(host_key("  https://h:3000  "), "https://h:3000");
        assert_eq!(host_key("https://h"), "https://h");
    }
}
