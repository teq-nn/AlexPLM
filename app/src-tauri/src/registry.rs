//! The Produkt-Registry (Issue #45, E45).
//!
//! A lean, **path-only** list of the products the user wants to search across — no content is
//! ever cached here (a second copy of artifacts/metadata would be exactly the drift E8/E18
//! fight). The registry lives at **app level**, not inside any product: it is the one place
//! that knows "these N folders are my products". Storing only paths means the registry can
//! never disagree with a product's real on-disk truth — at search time each path is opened
//! live (see [`crate::search`]).
//!
//! Split, like the rest of the codebase: the pure set logic ([`add_path`], [`remove_path`],
//! [`normalize`]) is total and I/O-free and carries `#[cfg(test)]` tables; the thin glue
//! ([`read_registry`], [`add_registered`], [`remove_registered`]) does the JSON file I/O over
//! that pure core. A missing/empty/corrupt registry file reads as an **empty registry** — never
//! an error — so a fresh install or a hand-mangled file degrades to "no products yet".

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File name of the app-level registry, under the app config dir. JSON for an honest, diffable,
/// hand-readable record (it holds only paths).
pub const REGISTRY_FILE: &str = "produkt-registry.json";

/// One registered product: **only its path** (forward-or-native as the OS gave it) plus the
/// folder name as a convenience label the UI can show without re-reading the disk. The label is
/// derived from the path, never an independent fact — so it cannot drift from the real folder.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredProduct {
    /// Absolute path to the product folder. The single source of truth for this entry.
    pub path: String,
    /// Folder name, derived from `path` — a display convenience, not a second fact.
    pub name: String,
}

impl RegisteredProduct {
    /// Build an entry from a path, deriving the display name from the final path component.
    pub fn from_path(path: String) -> Self {
        let name = Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        RegisteredProduct { path, name }
    }
}

/// Normalize a product path so equality is stable: trim surrounding whitespace and drop a single
/// trailing separator (so `/a/b` and `/a/b/` are the same product). Pure; does not touch disk
/// and does not canonicalize symlinks (a path that cannot be opened is reported honestly at
/// search time rather than silently rewritten here).
pub fn normalize(path: &str) -> String {
    let trimmed = path.trim();
    // Drop exactly one trailing '/' or '\\', but never reduce a bare root ("/" stays "/").
    let stripped = trimmed
        .strip_suffix('/')
        .or_else(|| trimmed.strip_suffix('\\'))
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed);
    stripped.to_string()
}

/// Add a product path to the set, de-duplicated by its [`normalize`]d form. Pure: returns the
/// new, sorted-by-path set. Adding an already-present product is a no-op (idempotent). An empty
/// path (after normalize) is rejected — it is never a real product — and returns the set
/// unchanged.
pub fn add_path(mut set: Vec<RegisteredProduct>, path: &str) -> Vec<RegisteredProduct> {
    let norm = normalize(path);
    if norm.is_empty() {
        return set;
    }
    if !set.iter().any(|p| p.path == norm) {
        set.push(RegisteredProduct::from_path(norm));
        set.sort_by(|a, b| a.path.cmp(&b.path));
    }
    set
}

/// Remove a product path from the set by its [`normalize`]d form. Pure; a path that is not
/// present is simply left out (idempotent).
pub fn remove_path(set: Vec<RegisteredProduct>, path: &str) -> Vec<RegisteredProduct> {
    let norm = normalize(path);
    set.into_iter().filter(|p| p.path != norm).collect()
}

/// Re-point a registry entry from `old_path` to `new_path` (Issue #89, PRD-US5): a moved product
/// (folder renamed/moved outside the app) is **re-linked**, not orphaned. Pure: drops the old
/// entry and adds the new path through the same [`normalize`]/de-dup path as [`add_path`], so the
/// result can never grow a duplicate. The display name is re-derived from the new path
/// ([`RegisteredProduct::from_path`]) — never carried over independently.
///
/// Modelled deliberately as remove-then-add so the invariants are exactly those of the existing
/// pure core:
/// - If `new_path` is empty (after normalize) it is not a product: the old entry is removed and
///   nothing is re-added (the registry is left without that broken entry rather than re-pointed
///   to nowhere — the command layer rejects an empty/implausible target before reaching here).
/// - If `new_path` (normalized) is already a registered product, the merge is automatic: removing
///   `old_path` and adding the already-present `new_path` is a no-op add, so one entry survives —
///   no duplicate.
/// - Re-linking a path to itself (old == new) is idempotent: the entry is removed and re-added
///   unchanged.
pub fn relink_path(
    set: Vec<RegisteredProduct>,
    old_path: &str,
    new_path: &str,
) -> Vec<RegisteredProduct> {
    let without_old = remove_path(set, old_path);
    add_path(without_old, new_path)
}

// ---- Thin file-I/O glue over the pure core --------------------------------------------------

/// Read the registry from `file`. A missing/empty/corrupt file means an **empty registry** —
/// never an error (a fresh install has no file yet; a hand-mangled file must not brick search).
pub fn read_registry(file: &Path) -> Vec<RegisteredProduct> {
    crate::plmstore::read_or_default(file)
}

/// Persist the registry, pretty-printed (it is only paths — keep it diffable and readable).
/// Creates the parent app-config directory if it does not exist yet (pretty + atomic).
fn write_registry(file: &Path, set: &[RegisteredProduct]) -> std::io::Result<()> {
    crate::plmstore::write_pretty(file, &set.to_vec())
}

/// Register a product path and persist the result. Returns the refreshed, sorted set.
pub fn add_registered(file: &Path, path: &str) -> std::io::Result<Vec<RegisteredProduct>> {
    let set = add_path(read_registry(file), path);
    write_registry(file, &set)?;
    Ok(set)
}

/// Unregister a product path and persist the result. No-op if absent. Returns the refreshed set.
pub fn remove_registered(file: &Path, path: &str) -> std::io::Result<Vec<RegisteredProduct>> {
    let set = remove_path(read_registry(file), path);
    write_registry(file, &set)?;
    Ok(set)
}

/// Re-point a registry entry from `old_path` to `new_path` and persist (Issue #89). Replaces the
/// old entry rather than orphaning it; never grows a duplicate (see [`relink_path`]). Returns the
/// refreshed, sorted set.
pub fn relink_registered(
    file: &Path,
    old_path: &str,
    new_path: &str,
) -> std::io::Result<Vec<RegisteredProduct>> {
    let set = relink_path(read_registry(file), old_path, new_path);
    write_registry(file, &set)?;
    Ok(set)
}

/// Resolve the registry file path under an app config directory. Kept separate so the command
/// layer can hand in the Tauri-resolved app config dir while tests use a temp dir.
pub fn registry_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(REGISTRY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_and_drops_one_trailing_separator() {
        let cases: &[(&str, &str)] = &[
            ("  /a/b  ", "/a/b"),
            ("/a/b/", "/a/b"),
            ("/a/b\\", "/a/b"),
            ("/a/b", "/a/b"),
            ("/", "/"),    // bare root is preserved
            ("", ""),      // empty stays empty (rejected by add_path)
            ("   ", ""),   // whitespace-only normalizes to empty
        ];
        for (input, expected) in cases {
            assert_eq!(normalize(input), *expected, "normalize({input:?})");
        }
    }

    #[test]
    fn add_path_is_dedup_idempotent_and_sorted() {
        let set = add_path(Vec::new(), "/p/charlie");
        let set = add_path(set, "/p/alpha");
        let set = add_path(set, "/p/charlie/"); // same as /p/charlie after normalize -> no-op
        let set = add_path(set, "/p/bravo");
        let paths: Vec<&str> = set.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths, vec!["/p/alpha", "/p/bravo", "/p/charlie"]);
        // exactly one charlie despite the duplicate add
        assert_eq!(set.iter().filter(|p| p.path == "/p/charlie").count(), 1);
    }

    #[test]
    fn add_path_rejects_empty_and_derives_name() {
        let set = add_path(Vec::new(), "   ");
        assert!(set.is_empty(), "empty/whitespace path is not a product");

        let set = add_path(Vec::new(), "/home/me/Regler-V2");
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].name, "Regler-V2", "name is derived from the folder");
    }

    #[test]
    fn remove_path_is_idempotent_and_normalizes() {
        let set = add_path(add_path(Vec::new(), "/p/a"), "/p/b");
        // remove with a trailing slash still matches the normalized entry
        let set = remove_path(set, "/p/a/");
        let paths: Vec<&str> = set.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths, vec!["/p/b"]);
        // removing something absent is a no-op
        let set = remove_path(set, "/p/never");
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn relink_path_repoints_normalizes_dedups_and_derives_name() {
        // A small fixture registry to relink within.
        let base = || {
            add_path(
                add_path(add_path(Vec::new(), "/p/alpha"), "/p/bravo"),
                "/p/charlie",
            )
        };

        struct Case {
            what: &'static str,
            old: &'static str,
            new: &'static str,
            // expected paths after relink (sorted), and the expected name of the new entry (if any)
            expect_paths: Vec<&'static str>,
            expect_name_of: Option<(&'static str, &'static str)>, // (path, name)
        }

        let cases = vec![
            Case {
                what: "moved folder: old entry replaced by new path, no duplicate",
                old: "/p/bravo",
                new: "/home/me/Regler-V2",
                expect_paths: vec!["/home/me/Regler-V2", "/p/alpha", "/p/charlie"],
                expect_name_of: Some(("/home/me/Regler-V2", "Regler-V2")),
            },
            Case {
                what: "new path with trailing slash normalizes (no /p/bravo/ ghost)",
                old: "/p/bravo",
                new: "/srv/data/Neu/",
                expect_paths: vec!["/p/alpha", "/p/charlie", "/srv/data/Neu"],
                expect_name_of: Some(("/srv/data/Neu", "Neu")),
            },
            Case {
                what: "merge-if-duplicate: relink onto an already-registered path keeps ONE entry",
                old: "/p/bravo",
                new: "/p/charlie",
                expect_paths: vec!["/p/alpha", "/p/charlie"],
                expect_name_of: Some(("/p/charlie", "charlie")),
            },
            Case {
                what: "idempotent self-relink leaves the set unchanged",
                old: "/p/bravo",
                new: "/p/bravo",
                expect_paths: vec!["/p/alpha", "/p/bravo", "/p/charlie"],
                expect_name_of: Some(("/p/bravo", "bravo")),
            },
            Case {
                what: "relinking a path that is not present just adds the new one",
                old: "/p/never",
                new: "/p/delta",
                expect_paths: vec!["/p/alpha", "/p/bravo", "/p/charlie", "/p/delta"],
                expect_name_of: Some(("/p/delta", "delta")),
            },
        ];

        for c in cases {
            let set = relink_path(base(), c.old, c.new);
            let paths: Vec<&str> = set.iter().map(|p| p.path.as_str()).collect();
            assert_eq!(paths, c.expect_paths, "{}: paths", c.what);
            // no duplicate of the relinked target ever survives
            if let Some((path, name)) = c.expect_name_of {
                let matches: Vec<&RegisteredProduct> =
                    set.iter().filter(|p| p.path == path).collect();
                assert_eq!(matches.len(), 1, "{}: exactly one entry for {path}", c.what);
                assert_eq!(matches[0].name, name, "{}: name derived from new path", c.what);
            }
        }
    }
}
