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
//! decision parts — `.gitattributes` parsing ([`parse_gitattributes`]) and line generation
//! ([`import_attr_lines`]) — stay table-testable without a real repo. The lockable lines land in
//! an **idempotent `_import` marker block** via the shared [`crate::markerblock`] mechanism (the
//! same as Onboarding #48), so import never clobbers hand-edits or a Baustein block (#63).

use crate::classifier::{classify, AttrMarker};
use crate::import_gate::{decide_import, GateDecision, RepoState};
use crate::markerblock::upsert_block;
use crate::projection::{project_product, ProductView};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind};
use std::path::Path;

/// Reserved Marker-Block-`id` for the lines the **Import** path (E38) writes into
/// `.gitattributes` from the files actually on disk. It is not a Baustein — the leading
/// underscore keeps it out of the Bibliothek id-space — but it rides the **same** idempotent
/// [`crate::markerblock`] mechanism as Onboarding (#48), so import never clobbers hand-edits or
/// a Baustein's own block, and re-importing is idempotent (closes #63).
const IMPORT_MARKER_ID: &str = "_import";

/// Heavy-binary threshold: a blob this size or larger, already committed into history, is a
/// "Riesen-Binary" (E38) that warrants the `git lfs migrate` rewrite. 50 MiB — well above
/// source files, comfortably below typical CAD/mesh exports the tool wants out of git.
const GIANT_BINARY_BYTES: u64 = 50 * 1024 * 1024;

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

    upsert_gitattributes(root, &import_attr_lines(&markers))?;

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

/// The canonical `.gitattributes` **lines** for the import marker block, from pattern →
/// lockable decisions.
///
/// Pure and total. Lockable patterns get the full LFS+lockable attribute line — byte-identical
/// to [`crate::onboardglue`]'s `attr_line`, so import and onboarding produce the same rule for
/// the same pattern. Non-lockable patterns are left out (git's default text handling is correct
/// and an unmarked rule keeps the file honest). Deterministically ordered by pattern via the
/// `BTreeMap`. The lines land in the idempotent `_import` marker block via [`upsert_gitattributes`].
pub fn import_attr_lines(markers: &BTreeMap<String, bool>) -> Vec<String> {
    markers
        .iter()
        .filter(|(_, lockable)| **lockable)
        // -text -diff stops git from trying to merge/diff; lockable enables path locks.
        .map(|(pattern, _)| format!("{pattern} filter=lfs diff=lfs merge=lfs -text lockable"))
        .collect()
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

/// Upsert the import-detected lockable lines into the `.gitattributes` `_import` marker block,
/// **idempotently**, via the shared [`crate::markerblock`] mechanism (#48/#63). Reads the
/// existing file (missing ⇒ empty, never an error), rewrites only the tool's own block, and
/// writes back **only when something changed** — hand-edits and Baustein blocks outside the
/// `_import` block survive byte-for-byte.
fn upsert_gitattributes(root: &Path, lines: &[String]) -> std::io::Result<()> {
    let path = root.join(".gitattributes");
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let updated = upsert_block(&existing, IMPORT_MARKER_ID, lines);
    if updated == existing {
        return Ok(());
    }
    std::fs::write(&path, updated)
}

/// Run a git subcommand in `root`, mapping a non-zero exit to an `io::Error`.
fn run_git(root: &Path, args: &[&str]) -> std::io::Result<()> {
    let out = crate::gitrunner::command(root).args(args).output()?;
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
    let out = crate::gitrunner::command(root)
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

// ---- Import Gate I/O glue (Issue #7, E38) ----
//
// The pure decision lives in `crate::import_gate`. Here we only *gather* the three repo
// facts from the real folder, and — strictly behind a `MigrateBehindGate` decision plus an
// explicit confirmation — run the destructive `git lfs migrate`. The dangerous git call is
// kept far from the pure gate so it can never be reached by accident.

/// What the Import Gate decided for a folder, plus the facts it decided on, for the UI to
/// explain the stakes before any history is touched.
#[derive(Debug, Serialize, Clone)]
pub struct GateReport {
    /// The one decision: clean-init | migrate-behind-gate | refuse.
    pub decision: GateDecision,
    /// Whether the folder already carries git history.
    pub has_history: bool,
    /// Whether shared clones (a remote) exist — the refuse trigger (E38).
    pub shared_clones_exist: bool,
    /// Whether heavy binaries are already committed into history.
    pub giant_binaries_in_history: bool,
}

/// Probe a folder's git state and run the pure Import Gate over it (no mutation).
///
/// This is the read step the UI calls first: it reports whether the safe clean import
/// applies, whether the gated history rewrite is offered, or whether the tool must refuse
/// because the repo is shared. It never writes anything.
pub fn evaluate_import_gate(root: &Path) -> std::io::Result<GateReport> {
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Kein Ordner: {}", root.display()),
        ));
    }
    let has_history = repo_has_history(root);
    let shared_clones_exist = has_history && repo_has_shared_clones(root);
    let giant_binaries_in_history = has_history && repo_has_giant_binaries_in_history(root);
    let state = RepoState {
        has_history,
        shared_clones_exist,
        giant_binaries_in_history,
    };
    Ok(GateReport {
        decision: decide_import(state),
        has_history,
        shared_clones_exist,
        giant_binaries_in_history,
    })
}

/// Run the destructive `git lfs migrate import` history rewrite — ONLY when the gate
/// permits it. Re-probes the live repo and refuses unless the current decision is
/// [`GateDecision::MigrateBehindGate`]; a stale UI can never push us past the gate. This is
/// the only place in the codebase that rewrites history.
///
/// After the rewrite the heavy blobs live in LFS (pointers in history, content in the store),
/// `.gitattributes` carries the lockable markers, and the product re-projects for the shell.
pub fn migrate_history_behind_gate(root: &Path) -> std::io::Result<ImportResult> {
    let report = evaluate_import_gate(root)?;
    if report.decision != GateDecision::MigrateBehindGate {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Historie-Umschreibung nicht erlaubt: das Gate gibt sie für diesen Ordner nicht frei \
             (geteilte Klone oder kein Bedarf).",
        ));
    }

    // Decide lockable patterns exactly as the clean path does, so post-migrate the same
    // unmergeable types are tracked by LFS and marked lockable.
    let existing = read_existing_attributes(root)?;
    let leaves = collect_leaf_files(root)?;
    let mut markers: BTreeMap<String, bool> = BTreeMap::new();
    let mut locked_count = 0usize;
    for rel in &leaves {
        let pattern = pattern_for(rel);
        let lockable = classify(rel, existing.get(&pattern).copied()).is_lockable();
        if lockable {
            locked_count += 1;
        }
        markers
            .entry(pattern)
            .and_modify(|v| *v = *v || lockable)
            .or_insert(lockable);
    }
    upsert_gitattributes(root, &import_attr_lines(&markers))?;

    // The destructive step: rewrite history so the lockable patterns become LFS pointers.
    let patterns: Vec<String> = markers
        .iter()
        .filter(|(_, lockable)| **lockable)
        .map(|(p, _)| p.clone())
        .collect();
    run_lfs_migrate(root, &patterns)?;

    run_git(root, &["add", "-A"])?;
    commit(root, "Import: Historie auf LFS umgeschrieben (PLM-Werkzeug, E38)")?;

    let product = project_product(root)?;
    Ok(ImportResult {
        git_initialized: false,
        locked_count,
        product,
    })
}

/// Whether the folder is a git repo carrying at least one commit.
fn repo_has_history(root: &Path) -> bool {
    if !is_git_repo(root) {
        return false;
    }
    crate::gitrunner::command(root)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether shared clones exist — proxied by any configured remote. A repo with a remote may
/// have clones we would poison by rewriting history, so its mere presence forces refuse (E38).
fn repo_has_shared_clones(root: &Path) -> bool {
    crate::gitrunner::command(root)
        .args(["remote"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Whether any blob reachable from history is at or above the giant-binary threshold.
fn repo_has_giant_binaries_in_history(root: &Path) -> bool {
    // List every (type, oid, size) of all objects across all refs; scan for a giant blob.
    let rev = crate::gitrunner::command(root)
        .args(["rev-list", "--objects", "--all"])
        .output();
    let Ok(rev) = rev else { return false };
    if !rev.status.success() || rev.stdout.is_empty() {
        return false;
    }
    // `rev-list --objects` prints "<oid> [<path>]"; cat-file's batch-check wants just the oid,
    // so strip everything after the first whitespace per line before piping it in.
    let oids: String = String::from_utf8_lossy(&rev.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(|oid| format!("{oid}\n"))
        .collect();
    let batch = crate::gitrunner::command(root)
        .args(["cat-file", "--batch-check=%(objecttype) %(objectsize)"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();
    let Ok(mut child) = batch else { return false };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(oids.as_bytes());
    }
    let Ok(out) = child.wait_with_output() else { return false };
    String::from_utf8_lossy(&out.stdout).lines().any(|line| {
        let mut p = line.split_whitespace();
        matches!(p.next(), Some("blob"))
            && p.next()
                .and_then(|s| s.parse::<u64>().ok())
                .map(|sz| sz >= GIANT_BINARY_BYTES)
                .unwrap_or(false)
    })
}

/// Run `git lfs migrate import` over the lockable patterns. Destructive: rewrites history.
fn run_lfs_migrate(root: &Path, patterns: &[String]) -> std::io::Result<()> {
    if patterns.is_empty() {
        return Ok(());
    }
    let mut args: Vec<String> = vec![
        "lfs".into(),
        "migrate".into(),
        "import".into(),
        "--everything".into(),
        "--yes".into(),
    ];
    for p in patterns {
        args.push(format!("--include={p}"));
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git(root, &refs)
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
    fn import_attr_lines_emits_only_lockable_patterns_sorted() {
        let mut markers = BTreeMap::new();
        markers.insert("*.png".to_string(), true);
        markers.insert("*.c".to_string(), false); // text -> omitted
        markers.insert("*.kicad_pcb".to_string(), true);
        markers.insert("*.f3d".to_string(), true);

        let lines = import_attr_lines(&markers);
        // every lockable pattern present as a full attr line, the text one omitted, sorted.
        assert_eq!(
            lines,
            vec![
                "*.f3d filter=lfs diff=lfs merge=lfs -text lockable".to_string(),
                "*.kicad_pcb filter=lfs diff=lfs merge=lfs -text lockable".to_string(),
                "*.png filter=lfs diff=lfs merge=lfs -text lockable".to_string(),
            ]
        );
        assert!(!lines.iter().any(|l| l.starts_with("*.c ")));
    }

    /// The #63 regression: import writes its detected lockable patterns into an idempotent
    /// `_import` marker block, and a second import run with the same markers is a no-op — never
    /// duplicating lines, never appending outside the block (it rides `markerblock::upsert_block`).
    #[test]
    fn import_attr_lines_land_in_idempotent_import_marker_block() {
        let mut markers = BTreeMap::new();
        markers.insert("*.step".to_string(), true);
        markers.insert("*.fcstd".to_string(), true);
        markers.insert("*.md".to_string(), false); // text -> omitted

        let lines = import_attr_lines(&markers);
        let once = upsert_block("", IMPORT_MARKER_ID, &lines);
        // the lockable patterns are present in the block, the text one is not.
        assert!(once.contains("# >>> baustein: _import >>>"));
        assert!(once.contains("*.step filter=lfs diff=lfs merge=lfs -text lockable"));
        assert!(once.contains("*.fcstd filter=lfs diff=lfs merge=lfs -text lockable"));
        assert!(!once.contains("*.md"));
        // re-import with identical detection is a pure no-op (twice == once).
        let twice = upsert_block(&once, IMPORT_MARKER_ID, &lines);
        assert_eq!(once, twice);
    }

    /// Hand-edits and a Baustein's own onboarding block survive an import upsert untouched —
    /// import only owns its `_import` block (closes the #63 acceptance "Hand-Edits bleiben").
    #[test]
    fn import_upsert_preserves_hand_edits_and_baustein_blocks() {
        let existing = "# meine Hand-Edits\n*.secret -text\n\
            # >>> baustein: kicad >>>\n*.kicad_pcb filter=lfs diff=lfs merge=lfs -text lockable\n# <<< baustein: kicad <<<\n";
        let mut markers = BTreeMap::new();
        markers.insert("*.step".to_string(), true);
        let out = upsert_block(existing, IMPORT_MARKER_ID, &import_attr_lines(&markers));
        // foreign content is byte-preserved …
        assert!(out.contains("# meine Hand-Edits\n*.secret -text"));
        assert!(out.contains("# >>> baustein: kicad >>>\n*.kicad_pcb filter=lfs diff=lfs merge=lfs -text lockable\n# <<< baustein: kicad <<<"));
        // … and the import block is added with the detected lockable pattern.
        assert!(out.contains("# >>> baustein: _import >>>\n*.step filter=lfs diff=lfs merge=lfs -text lockable\n# <<< baustein: _import <<<"));
    }
}
