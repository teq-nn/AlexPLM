//! Minimal, read-only Graph Projection (Issue #2).
//!
//! Projects a product folder into the domain view the UI Shell renders: the product
//! name, its active branch/version, and one **Baustein** per leaf folder. This is the
//! read path only — it never mutates git or the working tree.
//!
//! The pure decision bits (`main_file_rank`, `pick_main_file`, `rel_path`) are split out
//! from the filesystem walk so they can be exercised by `#[cfg(test)]` table tests
//! without a real repo — the "reiner Kern + Tabellentest" pattern from the PRD.
//!
//! Genestete `.git` sind **opake Grenzen** (E50a, Issue #130): ein Unterordner mit eigenem
//! `.git` (eingezogene Toolchain wie `west`/ESP-IDF/`venv`) wird **nicht** betreten und **nicht**
//! als Baustein projiziert. Die Grenze ist dieselbe, an der auch Watcher und Klassifizierer
//! stoppen — das reine Grenz-Prädikat lebt in [`crate::nestedboundary`].

use crate::nestedboundary::GIT_MARKER;
use serde::Serialize;
use std::path::Path;

/// A leaf folder understood as a building block of the product.
// Distinct from the catalog `baustein::Baustein` — specta exports unique names, and the
// hand-written types.ts silently declaration-merged the two. This is the product-view leaf.
#[derive(specta::Type, Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename = "ProduktBaustein")]
pub struct Baustein {
    /// Folder name; the UI renders this as a caps label.
    pub name: String,
    /// Folder path relative to the product root (muted Mono in the UI).
    pub path: String,
    /// Representative file of this Baustein, relative to the product root, if any.
    pub main_file: Option<String>,
}

/// Read-only projection of a product folder.
#[derive(specta::Type, Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ProductView {
    pub name: String,
    pub branch: String,
    pub version: String,
    pub bausteine: Vec<Baustein>,
}

const DIR_DENYLIST: &[&str] = &["node_modules", "target", "__pycache__"];

/// Name prefix of the tool's own committed store directory (`_plm`, ADR 0002). The walk skips
/// any entry starting with this so the tool's notes are never projected as a Baustein.
const PLM_PREFIX: &str = "_plm";

/// Project a product folder into a [`ProductView`]. Pure read — no writes, no git mutation.
pub fn project_product(root: &Path) -> std::io::Result<ProductView> {
    let name = file_name_of(root);
    let branch = read_branch(root);
    let version = read_version(root);

    let mut bausteine = Vec::new();
    collect_bausteine(root, root, &mut bausteine)?;
    bausteine.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ProductView { name, branch, version, bausteine })
}

/// Recursively gather leaf folders (Bausteine). A directory with no non-hidden subfolders
/// but at least one non-hidden file is a Baustein; directories with subfolders are descended
/// into and not themselves treated as Bausteine.
fn collect_bausteine(root: &Path, dir: &Path, out: &mut Vec<Baustein>) -> std::io::Result<()> {
    let mut subdirs = Vec::new();
    let mut files = Vec::new();
    // Trägt dieses Verzeichnis selbst ein `.git`? Am Produkt-Wurzel ist das das eigene Repo;
    // tiefer ist es eine genestete, opake Grenze (E50a) — dann wird hier abgeschnitten.
    let mut has_git = false;

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let fname = entry.file_name().to_string_lossy().into_owned();
        // `.git` (Verzeichnis ODER Submodul-`.git`-Datei) markiert eine git-Grenze.
        if fname == GIT_MARKER {
            has_git = true;
        }
        if ft.is_dir() {
            if is_hidden(&fname) || is_plm(&fname) || DIR_DENYLIST.contains(&fname.as_str()) {
                continue;
            }
            subdirs.push(entry.path());
        } else if ft.is_file() && !is_hidden(&fname) {
            files.push(fname);
        }
    }

    // Genestetes `.git` unterhalb der Wurzel: opake Grenze. Nicht hineinsteigen, nicht als
    // Baustein projizieren — der fremde Toolchain-Baum bleibt für Werkbank unsichtbar (E50a).
    if has_git && dir != root {
        return Ok(());
    }

    if subdirs.is_empty() {
        if !files.is_empty() {
            let rel = rel_path(root, dir);
            let name = if dir == root { file_name_of(root) } else { file_name_of(dir) };
            let main_file = pick_main_file(&files).map(|f| join_rel(&rel, &f));
            let path = if rel.is_empty() { ".".to_string() } else { rel };
            out.push(Baustein { name, path, main_file });
        }
    } else {
        subdirs.sort();
        for sd in subdirs {
            collect_bausteine(root, &sd, out)?;
        }
    }
    Ok(())
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Whether an entry name belongs to the tool's own `_plm` store (ADR 0002). The Baustein walk
/// skips it by name so the visible, committed tool directory is never taken for an Arbeitsbereich
/// (previously the dotfile camouflage did this job).
fn is_plm(name: &str) -> bool {
    name.starts_with(PLM_PREFIX)
}

fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// Forward-slash path of `p` relative to `root`; empty string if `p == root`.
fn rel_path(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .ok()
        .map(|r| {
            r.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

fn join_rel(dir_rel: &str, file: &str) -> String {
    if dir_rel.is_empty() {
        file.to_string()
    } else {
        format!("{dir_rel}/{file}")
    }
}

/// How representative a filename is as the "main" file of a Baustein. Higher wins.
/// Pure function over the filename only.
fn main_file_rank(name: &str) -> u8 {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        // Source-of-truth CAD / project files
        "f3d" | "fcstd" | "step" | "stp" | "kicad_pro" => 5,
        // KiCad layout/schematic (nominal text, unmergeable)
        "kicad_pcb" | "kicad_sch" => 4,
        // Mesh / interchange
        "stl" | "3mf" | "iges" | "igs" => 3,
        // Human-readable docs
        "pdf" | "md" => 2,
        _ => 1,
    }
}

/// Pick the representative file of a Baustein: highest [`main_file_rank`], ties broken
/// alphabetically (first name wins). Deterministic and total over a non-empty list.
pub fn pick_main_file(files: &[String]) -> Option<String> {
    files
        .iter()
        .max_by(|a, b| {
            main_file_rank(a)
                .cmp(&main_file_rank(b))
                // reverse alpha so the alphabetically *first* name compares as greater
                .then_with(|| b.cmp(a))
        })
        .cloned()
}

/// Active branch from `.git/HEAD`, defaulting to `main` when there is no git or HEAD is detached.
fn read_branch(root: &Path) -> String {
    let head = std::fs::read_to_string(root.join(".git/HEAD")).unwrap_or_default();
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "main".to_string())
}

/// A version-like tag from `.git/refs/tags`, defaulting to `v0.0`. Cosmetic for this slice;
/// real revision/version logic lands with Graph Projection (Issue #8).
fn read_version(root: &Path) -> String {
    let tags_dir = root.join(".git/refs/tags");
    let mut best: Option<String> = None;
    if let Ok(entries) = std::fs::read_dir(tags_dir) {
        for entry in entries.flatten() {
            let tag = entry.file_name().to_string_lossy().into_owned();
            let looks_versiony =
                tag.starts_with('v') || tag.chars().next().is_some_and(|c| c.is_ascii_digit());
            if looks_versiony && best.as_ref().is_none_or(|b| tag > *b) {
                best = Some(tag);
            }
        }
    }
    best.unwrap_or_else(|| "v0.0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_orders_design_files_above_noise() {
        assert!(main_file_rank("body.f3d") > main_file_rank("notes.txt"));
        assert!(main_file_rank("board.kicad_pcb") > main_file_rank("part.stl"));
        assert!(main_file_rank("part.stl") > main_file_rank("readme.md"));
        assert!(main_file_rank("readme.md") > main_file_rank("scratch.log"));
    }

    #[test]
    fn rank_is_case_insensitive() {
        assert_eq!(main_file_rank("BODY.F3D"), main_file_rank("body.f3d"));
    }

    #[test]
    fn pick_main_file_is_total_and_deterministic() {
        // table: input files -> expected representative
        let cases: &[(&[&str], Option<&str>)] = &[
            (&[], None),
            (&["only.txt"], Some("only.txt")),
            (&["a.txt", "b.txt", "c.txt"], Some("a.txt")), // tie on rank -> first alpha
            (&["notes.md", "gehaeuse.f3d", "render.png"], Some("gehaeuse.f3d")),
            (&["board.kicad_pcb", "board.kicad_pro"], Some("board.kicad_pro")),
        ];
        for (files, expected) in cases {
            let owned: Vec<String> = files.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                pick_main_file(&owned).as_deref(),
                *expected,
                "files = {files:?}"
            );
        }
    }

    #[test]
    fn rel_path_uses_forward_slashes_and_is_empty_at_root() {
        let root = Path::new("/a/b");
        assert_eq!(rel_path(root, Path::new("/a/b")), "");
        assert_eq!(rel_path(root, Path::new("/a/b/elektronik/regler")), "elektronik/regler");
    }

    #[test]
    fn join_rel_handles_root_level_files() {
        assert_eq!(join_rel("", "x.f3d"), "x.f3d");
        assert_eq!(join_rel("elektronik", "x.f3d"), "elektronik/x.f3d");
    }

    #[test]
    fn is_plm_recognises_the_tool_store_by_name() {
        assert!(is_plm("_plm"));
        assert!(is_plm("_plm-archive")); // prefix rule, defensive
        assert!(!is_plm("elektronik"));
        assert!(!is_plm("plm")); // no leading underscore
    }
}
