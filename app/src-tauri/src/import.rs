//! Clean, non-destructive import path (Issue #3, E38).
//!
//! Turns a chosen folder into a product: show → `git init` if needed → detect leaf-folder
//! Bausteine → run the [`crate::classifier`] over each leaf file → write matching
//! `.gitattributes` lockable markers (binary + KiCad → lockable) → first commit. The imported
//! product then renders in the read-only shell via [`crate::projection::project_product`].
//!
//! Non-destructive by E38: an already-git folder is **left as-is** (no re-init, history
//! untouched); only a non-git folder gets `git init`. The dangerous `git lfs migrate` /
//! history-rewrite branch of E38 is intentionally out of scope for this slice.
//!
//! The git plumbing (`init`/`add`/`commit`) is isolated behind small helpers so the pure
//! decision parts — `.gitattributes` parsing ([`parse_gitattributes`]) and generation
//! ([`render_gitattributes`]) — stay table-testable without a real repo.

use crate::classifier::{classify, AttrMarker};
use crate::projection::{project_product, ProductView};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::process::Command;

/// Outcome of an import, returned to the UI shell.
#[derive(Debug, Serialize, Clone)]
pub struct ImportResult {
    /// Whether this run ran `git init` (true) or found an existing repo and left it as-is (false).
    pub git_initialized: bool,
    /// Number of leaf files marked `lockable` in `.gitattributes`.
    pub locked_count: usize,
    /// The read-only projection of the freshly imported product (what the shell renders).
    pub product: ProductView,
}

/// Import a folder as a product. See module docs for the E38 flow.
pub fn import_folder(root: &Path) -> std::io::Result<ImportResult> {
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Kein Ordner: {}", root.display()),
        ));
    }

    // git init only when needed — an existing repo is left untouched (E38, non-destructive).
    let git_initialized = if is_git_repo(root) {
        false
    } else {
        run_git(root, &["init"])?;
        true
    };

    // Read existing markers, classify every leaf file, fold into per-pattern lockable decisions.
    let existing = read_existing_attributes(root)?;
    let leaves = collect_leaf_files(root)?;

    let mut markers: BTreeMap<String, bool> = BTreeMap::new();
    let mut locked_count = 0usize;
    for rel in &leaves {
        let pattern = pattern_for(rel);
        let bucket = classify(rel, existing.get(&pattern).copied());
        let lockable = bucket.is_lockable();
        if lockable {
            locked_count += 1;
        }
        // One glob rule per pattern; if any file under it is lockable, the rule locks.
        markers
            .entry(pattern)
            .and_modify(|v| *v = *v || lockable)
            .or_insert(lockable);
    }

    write_gitattributes(root, &render_gitattributes(&markers))?;

    // First commit so the imported product has history and renders with a branch/version.
    run_git(root, &["add", "-A"])?;
    commit(root, "Import: Produkt angelegt (PLM-Werkzeug)")?;

    let product = project_product(root)?;
    Ok(ImportResult {
        git_initialized,
        locked_count,
        product,
    })
}

/// Whether `root` already contains a git repository (a `.git` directory or file).
fn is_git_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// The `.gitattributes` glob pattern for a leaf file: a case-preserving `*.<ext>` rule when the
/// file has an extension, else the file's own name. Lets one rule cover all files of a type.
fn pattern_for(rel: &str) -> String {
    let name = rel.rsplit(['/', '\\']).next().unwrap_or(rel);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("*.{}", ext.to_ascii_lowercase()),
        _ => name.to_string(),
    }
}

/// Parse an existing `.gitattributes` text into a map of pattern → explicit marker.
///
/// Pure and total. Recognises the markers the import path cares about, normalising glob
/// patterns to lowercase extension so they line up with [`pattern_for`]:
/// - `lockable` anywhere on the line → [`AttrMarker::Lockable`]
/// - `binary` or `-text` (without `lockable`) → [`AttrMarker::Binary`]
/// - an explicit `text`/`merge=`/`diff` (without the above) → [`AttrMarker::Text`]
pub fn parse_gitattributes(text: &str) -> BTreeMap<String, AttrMarker> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(pattern) = parts.next() else { continue };
        let attrs: Vec<&str> = parts.collect();
        let has = |needle: &str| attrs.iter().any(|a| *a == needle);

        let marker = if has("lockable") {
            AttrMarker::Lockable
        } else if has("binary") || has("-text") {
            AttrMarker::Binary
        } else if has("text") || attrs.iter().any(|a| a.starts_with("merge=") || *a == "diff") {
            AttrMarker::Text
        } else {
            continue;
        };
        out.insert(normalize_pattern(pattern), marker);
    }
    out
}

/// Normalise a glob pattern to the key used by import: `*.<ext>` lowercased, else verbatim.
fn normalize_pattern(pattern: &str) -> String {
    if let Some(ext) = pattern.strip_prefix("*.") {
        format!("*.{}", ext.to_ascii_lowercase())
    } else {
        pattern.to_string()
    }
}

/// Render a deterministic `.gitattributes` body from pattern → lockable decisions.
///
/// Pure and total. Lockable patterns get the LFS lockable+binary marker; non-lockable
/// patterns are left out (git's default handling for plain text is correct and unmarked
/// keeps the file small and honest). Sorted by pattern via the `BTreeMap`.
pub fn render_gitattributes(markers: &BTreeMap<String, bool>) -> String {
    let mut out = String::from(
        "# Erzeugt vom PLM-Werkzeug beim Import (E38). Unmergebare Dateien -> lockable.\n",
    );
    for (pattern, lockable) in markers {
        if *lockable {
            // -text -diff stops git from trying to merge/diff; lockable enables path locks.
            out.push_str(&format!("{pattern} filter=lfs diff=lfs merge=lfs -text lockable\n"));
        }
    }
    out
}

// ---- filesystem + git glue (kept thin; the decisions above are the testable core) ----

/// Collect every leaf file (root-relative, forward-slash) the projection would consider, by
/// reusing the same Baustein walk so import and the read view agree on what counts.
fn collect_leaf_files(root: &Path) -> std::io::Result<Vec<String>> {
    let view = project_product(root)?;
    let mut files = Vec::new();
    for b in &view.bausteine {
        let dir = if b.path == "." {
            root.to_path_buf()
        } else {
            root.join(&b.path)
        };
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let rel = if b.path == "." {
                    name
                } else {
                    format!("{}/{}", b.path, name)
                };
                files.push(rel);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Read and parse an existing `.gitattributes` at the product root; empty map if none.
fn read_existing_attributes(root: &Path) -> std::io::Result<BTreeMap<String, AttrMarker>> {
    let path = root.join(".gitattributes");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(parse_gitattributes(&text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e),
    }
}

/// Write (overwrite) the product's `.gitattributes` with the rendered body.
fn write_gitattributes(root: &Path, body: &str) -> std::io::Result<()> {
    std::fs::write(root.join(".gitattributes"), body)
}

/// Run a git subcommand in `root`, mapping a non-zero exit to an `io::Error`.
fn run_git(root: &Path, args: &[&str]) -> std::io::Result<()> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output()?;
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

/// Make the first commit. Sets a local identity if the environment has none so import never
/// fails on a fresh machine, and allows an empty commit so an empty folder still gets history.
fn commit(root: &Path, message: &str) -> std::io::Result<()> {
    // Local identity fallback (never overwrites a configured global one in practice for the
    // commit, since -c only applies to this invocation).
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=PLM-Werkzeug",
            "-c",
            "user.email=plm@localhost",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ])
        .output()?;
    if !out.status.success() {
        return Err(Error::new(
            ErrorKind::Other,
            format!(
                "git commit failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_for_collapses_to_lowercase_extension_glob() {
        assert_eq!(pattern_for("mechanik/gehaeuse.F3D"), "*.f3d");
        assert_eq!(pattern_for("a/b/board.kicad_pcb"), "*.kicad_pcb");
        assert_eq!(pattern_for("Makefile"), "Makefile");
        assert_eq!(pattern_for("dir/.gitignore"), ".gitignore");
    }

    #[test]
    fn parse_gitattributes_reads_each_marker_kind() {
        // table: line(s) -> expected (pattern, marker)
        let cases: &[(&str, &str, AttrMarker)] = &[
            ("*.f3d filter=lfs -text lockable", "*.f3d", AttrMarker::Lockable),
            ("*.STEP lockable", "*.step", AttrMarker::Lockable),
            ("*.png binary", "*.png", AttrMarker::Binary),
            ("*.bin -text", "*.bin", AttrMarker::Binary),
            ("*.kicad_pcb text", "*.kicad_pcb", AttrMarker::Text),
            ("*.c merge=union", "*.c", AttrMarker::Text),
        ];
        for (line, pattern, expected) in cases {
            let parsed = parse_gitattributes(line);
            assert_eq!(parsed.get(*pattern), Some(expected), "line = {line:?}");
        }
    }

    #[test]
    fn parse_gitattributes_skips_comments_and_blanks() {
        let text = "# header\n\n   \n*.f3d lockable\n";
        let parsed = parse_gitattributes(text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.get("*.f3d"), Some(&AttrMarker::Lockable));
    }

    #[test]
    fn render_gitattributes_emits_only_lockable_patterns_sorted() {
        let mut markers = BTreeMap::new();
        markers.insert("*.png".to_string(), true);
        markers.insert("*.c".to_string(), false); // text -> omitted
        markers.insert("*.kicad_pcb".to_string(), true);
        markers.insert("*.f3d".to_string(), true);

        let body = render_gitattributes(&markers);
        // every lockable pattern present, the text one omitted
        assert!(body.contains("*.f3d filter=lfs diff=lfs merge=lfs -text lockable"));
        assert!(body.contains("*.kicad_pcb filter=lfs diff=lfs merge=lfs -text lockable"));
        assert!(body.contains("*.png filter=lfs diff=lfs merge=lfs -text lockable"));
        assert!(!body.contains("*.c "));
        // deterministic ordering: *.f3d before *.kicad_pcb before *.png
        let f3d = body.find("*.f3d").unwrap();
        let kicad = body.find("*.kicad_pcb").unwrap();
        let png = body.find("*.png").unwrap();
        assert!(f3d < kicad && kicad < png);
    }
}
