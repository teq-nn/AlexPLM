//! Produktübergreifende Live-Suche — der Fan-out (Issue #45, E45).
//!
//! A cross-product search that holds **no central index and no mirror**: an index would be the
//! drift E8/E18 fight. Instead the search runs a live **Fan-out** — it walks the
//! [`crate::registry`] of product paths, opens each *reachable* repo, and greps live over three
//! honest, text-cheap sources:
//!
//! 1. **Dateinamen** — every (non-hidden) file's path under the product.
//! 2. **`_plm`** — the product's metadata store (the only PLM facts Git cannot know).
//! 3. **`VERSION_NOTES.md`** — the one place human milestone text lives (E28).
//!
//! A product whose folder cannot be opened (deleted, on an unmounted network drive, no read
//! permission) is **reported honestly** as offline — never silently dropped — so the user can
//! trust "alle erreichbaren durchsucht" and see "3 von 14 offline, nicht durchsucht".
//!
//! Split as always: the pure core ([`match_line`], [`rank_hit`], [`merge_results`], the field
//! classifier) is total and I/O-free with `#[cfg(test)]` tables; the thin glue
//! ([`search_product`], [`fan_out`]) walks the filesystem and feeds the pure core.

use crate::registry::RegisteredProduct;
use serde::Serialize;
use std::path::Path;

/// Which of a product's three searched sources a hit came from. Lets the UI label a hit and
/// lets ranking prefer the most meaningful source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HitField {
    /// A file name / path under the product matched.
    Dateiname,
    /// A line in the product's `_plm` metadata store matched.
    Plm,
    /// A line in `VERSION_NOTES.md` matched.
    VersionNotes,
}

/// One match inside one product. Carries enough context for the UI to show *where* without a
/// second round-trip: the product, the field, the relative file the hit lives in, and the
/// matched text (a file path, or the matched line for content hits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    /// Absolute path of the product this hit belongs to.
    pub product_path: String,
    /// Display name of the product (folder name).
    pub product_name: String,
    /// Which source matched.
    pub field: HitField,
    /// Product-relative file the hit was found in (forward slashes).
    pub file: String,
    /// The matched text: a relative file path for `Dateiname`, the matched line for content.
    pub text: String,
    /// Computed relevance; higher sorts first. Stable, derived purely from the hit.
    pub score: i32,
}

/// One registered product that could not be searched, with a short honest reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfflineProduct {
    pub product_path: String,
    pub product_name: String,
    /// Human German reason, e.g. "Ordner nicht erreichbar".
    pub reason: String,
}

/// The full result of one fan-out search. Reachable hits plus the honest offline tally, so the
/// UI can render results AND "N von M offline, nicht durchsucht" from one payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    /// All hits across all reachable products, already merged + ranked (best first).
    pub hits: Vec<SearchHit>,
    /// Registered products that could not be opened — reported, never silently dropped.
    pub offline: Vec<OfflineProduct>,
    /// How many registered products were searched (reachable).
    pub searched: usize,
    /// Total registered products considered (`searched + offline.len()`).
    pub total: usize,
}

// ---- Pure core: matching + ranking + merge (no I/O) -----------------------------------------

/// Does `haystack` contain `needle`, case-insensitively? The single match predicate the whole
/// fan-out rests on. An empty needle never matches (an empty query returns nothing, not
/// everything). Pure.
pub fn match_line(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Score a candidate hit. Higher is more relevant. Deterministic and pure — it judges only the
/// field and the matched text against the query, so ranking can be table-tested without disk.
///
/// Ordering rationale:
/// - a **file-name** hit (you searched for a part you can open) outranks buried content;
/// - among content, `_plm` (real PLM facts) outranks free-form `VERSION_NOTES`;
/// - an **exact** (whole-text, case-insensitive) match beats a mere substring;
/// - a hit where the needle is a larger fraction of the text ranks higher (less noise).
pub fn rank_hit(field: HitField, text: &str, needle: &str) -> i32 {
    let base = match field {
        HitField::Dateiname => 300,
        HitField::Plm => 200,
        HitField::VersionNotes => 100,
    };
    let text_l = text.to_lowercase();
    let needle_l = needle.to_lowercase();
    let exact = if text_l == needle_l { 50 } else { 0 };
    // Density: how much of the matched text the needle covers (0..=40). Guards div-by-zero.
    let density = if text_l.is_empty() {
        0
    } else {
        ((needle_l.len() * 40) / text_l.len().max(1)).min(40) as i32
    };
    base + exact + density
}

/// Merge per-product hit lists into one ranked list, best first. Ties (equal score) break
/// deterministically by product path, then field order, then text, so the output is stable
/// across runs (no HashMap iteration order leaking in). Pure.
pub fn merge_results(per_product: Vec<Vec<SearchHit>>) -> Vec<SearchHit> {
    let mut all: Vec<SearchHit> = per_product.into_iter().flatten().collect();
    all.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.product_path.cmp(&b.product_path))
            .then_with(|| (a.field as u8).cmp(&(b.field as u8)))
            .then_with(|| a.text.cmp(&b.text))
    });
    all
}

// ---- Thin glue: walk one product's filesystem and feed the pure core ------------------------

/// Directories we never descend into when collecting file names — build/output noise that is
/// never a product artifact (mirrors `projection.rs`). `.git` is skipped as a hidden dir.
const DIR_DENYLIST: &[&str] = &["node_modules", "target", "__pycache__"];

/// The product's metadata store directory (the only PLM facts Git cannot know).
const PLM_DIR: &str = "_plm";
/// The one file where human milestone text lives (E28).
const VERSION_NOTES: &str = "VERSION_NOTES.md";

/// Search one product live. Returns its hits, or an [`OfflineProduct`] if the folder cannot be
/// opened at all (deleted / unmounted / unreadable) — the honest-offline signal. A product that
/// opens but simply has no matches yields an empty hit list (reachable, just nothing found).
pub fn search_product(
    product: &RegisteredProduct,
    needle: &str,
) -> Result<Vec<SearchHit>, OfflineProduct> {
    let root = Path::new(&product.path);
    // Reachability probe: a registered product whose root cannot be listed is offline. We probe
    // with read_dir (not just exists) so an unreadable/permission-denied dir is caught too.
    if std::fs::read_dir(root).is_err() {
        return Err(OfflineProduct {
            product_path: product.path.clone(),
            product_name: product.name.clone(),
            reason: "Ordner nicht erreichbar".to_string(),
        });
    }

    let mut hits = Vec::new();
    // 1) Dateinamen: walk the tree, match each relative file path.
    walk_filenames(root, root, &mut |rel| {
        if match_line(&rel, needle) {
            hits.push(SearchHit {
                product_path: product.path.clone(),
                product_name: product.name.clone(),
                field: HitField::Dateiname,
                file: rel.clone(),
                text: rel,
                score: 0, // set below in one place
            });
        }
    });

    // 2) _plm: grep every file's text content live (it is small text metadata).
    let plm = root.join(PLM_DIR);
    if plm.is_dir() {
        walk_filenames(&plm, &plm, &mut |rel_in_plm| {
            let abs = plm.join(&rel_in_plm);
            grep_file_lines(&abs, needle, &mut |line| {
                let file = format!("{PLM_DIR}/{rel_in_plm}");
                hits.push(SearchHit {
                    product_path: product.path.clone(),
                    product_name: product.name.clone(),
                    field: HitField::Plm,
                    file: file.clone(),
                    text: line,
                    score: 0,
                });
            });
        });
    }

    // 3) VERSION_NOTES.md: grep the human milestone text.
    let notes = root.join(VERSION_NOTES);
    if notes.is_file() {
        grep_file_lines(&notes, needle, &mut |line| {
            hits.push(SearchHit {
                product_path: product.path.clone(),
                product_name: product.name.clone(),
                field: HitField::VersionNotes,
                file: VERSION_NOTES.to_string(),
                text: line,
                score: 0,
            });
        });
    }

    // Score every hit through the pure ranker (one place, so glue carries no ranking logic).
    for h in &mut hits {
        h.score = rank_hit(h.field, &h.text, needle);
    }
    Ok(hits)
}

/// Run the full fan-out over a registry. Each product is opened live; reachable products
/// contribute hits, unreachable ones land in `offline`. The result is merged + ranked and
/// carries the honest searched/total counts. An empty needle returns an empty result without
/// touching disk.
pub fn fan_out(registry: &[RegisteredProduct], needle: &str) -> SearchResult {
    let total = registry.len();
    if needle.trim().is_empty() {
        return SearchResult {
            hits: Vec::new(),
            offline: Vec::new(),
            searched: total,
            total,
        };
    }
    let needle = needle.trim();

    let mut per_product = Vec::new();
    let mut offline = Vec::new();
    for product in registry {
        match search_product(product, needle) {
            Ok(hits) => per_product.push(hits),
            Err(off) => offline.push(off),
        }
    }
    let searched = total - offline.len();
    SearchResult {
        hits: merge_results(per_product),
        offline,
        searched,
        total,
    }
}

/// Walk a directory tree, calling `visit` with each non-hidden file's path relative to `base`
/// (forward slashes). Skips hidden entries and the build-noise denylist. Best-effort: an
/// unreadable subdirectory is skipped, not fatal (the product as a whole is still reachable).
fn walk_filenames(base: &Path, dir: &Path, visit: &mut impl FnMut(String)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || DIR_DENYLIST.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => walk_filenames(base, &path, visit),
            Ok(ft) if ft.is_file() => {
                if let Ok(rel) = path.strip_prefix(base) {
                    let rel = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    visit(rel);
                }
            }
            _ => {}
        }
    }
}

/// Grep one text file line by line, calling `visit` with each matching (trimmed) line. A file
/// that cannot be read (binary, gone, no permission) is skipped silently — it is not an offline
/// product, just an un-greppable file. Lines are bounded in length so a giant minified blob does
/// not blow up the payload.
fn grep_file_lines(file: &Path, needle: &str, visit: &mut impl FnMut(String)) {
    const MAX_LINE: usize = 400;
    let raw = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(_) => return,
    };
    for line in raw.lines() {
        if match_line(line, needle) {
            let trimmed = line.trim();
            let shown = if trimmed.len() > MAX_LINE {
                format!("{}…", &trimmed[..MAX_LINE])
            } else {
                trimmed.to_string()
            };
            visit(shown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_line_is_case_insensitive_and_rejects_empty_needle() {
        assert!(match_line("Gehaeuse-Regler", "gehaeuse"));
        assert!(match_line("gehaeuse", "GEHAEUSE"));
        assert!(!match_line("anything", ""), "empty needle never matches");
        assert!(!match_line("", "x"));
    }

    #[test]
    fn rank_orders_filename_over_plm_over_notes() {
        let f = rank_hit(HitField::Dateiname, "regler.f3d", "regler");
        let p = rank_hit(HitField::Plm, "pflicht: regler", "regler");
        let n = rank_hit(HitField::VersionNotes, "regler ueberarbeitet", "regler");
        assert!(f > p, "filename {f} should outrank _plm {p}");
        assert!(p > n, "_plm {p} should outrank version-notes {n}");
    }

    #[test]
    fn rank_rewards_exact_match() {
        let exact = rank_hit(HitField::Dateiname, "regler", "regler");
        let substr = rank_hit(HitField::Dateiname, "regler-final-v2", "regler");
        assert!(exact > substr, "exact {exact} should beat substring {substr}");
    }

    #[test]
    fn merge_is_stable_best_first() {
        let mk = |path: &str, field, text: &str, score| SearchHit {
            product_path: path.to_string(),
            product_name: "P".to_string(),
            field,
            file: text.to_string(),
            text: text.to_string(),
            score,
        };
        let merged = merge_results(vec![
            vec![mk("/p/b", HitField::Plm, "x", 10)],
            vec![
                mk("/p/a", HitField::Dateiname, "y", 30),
                mk("/p/a", HitField::VersionNotes, "z", 10),
            ],
        ]);
        let scores: Vec<i32> = merged.iter().map(|h| h.score).collect();
        assert_eq!(scores, vec![30, 10, 10], "best score first");
        // tie at 10 breaks by product path: /p/a before /p/b
        assert_eq!(merged[1].product_path, "/p/a");
        assert_eq!(merged[2].product_path, "/p/b");
    }
}
