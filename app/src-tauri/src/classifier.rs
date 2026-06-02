//! Mergeability Classifier (Issue #3).
//!
//! The first standalone **reiner Kern + Tabellentest** core: a pure, total, deterministic
//! function that sorts a single leaf file into exactly one of three buckets. It does no I/O —
//! the filesystem walk and git mutation live in [`crate::import`]; this module only decides.
//!
//! The three buckets are the "dritter Eimer" from the Sitzung-5 glossary (Mergebar vs.
//! nicht-mergebar), refining the older Text-vs-Binär axis (E31):
//!
//! - [`Bucket::TextMergeable`] — echter Text, mergebar (firmware, docs, BOM text) → git merges.
//! - [`Bucket::BinaryUnmergeable`] — `.f3d`, STEP, STL, photos → must be locked (E31).
//! - [`Bucket::NominalTextUnmergeable`] — KiCad sources (`.kicad_sch`/`.kicad_pcb`): nominally
//!   text but merges corrupt the file ("Missing (" errors) → locked like binary.
//!
//! An existing `.gitattributes` marker for the path **overrides** extension classification:
//! a maintainer who already declared a lockable/binary or text marker is trusted (E18/E24).

use serde::Serialize;

/// One of the three mergeability buckets. A leaf file lands in exactly one.
#[derive(specta::Type, Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Bucket {
    /// Real text, git can merge: firmware, docs, BOM text.
    TextMergeable,
    /// Binary, unmergeable: CAD, mesh, photos → lockable.
    BinaryUnmergeable,
    /// Nominally text but practically unmergeable: KiCad sources → lockable.
    NominalTextUnmergeable,
}

impl Bucket {
    /// Whether files in this bucket must be marked `lockable` in `.gitattributes`.
    /// Both unmergeable buckets lock; only true mergeable text does not.
    pub fn is_lockable(self) -> bool {
        !matches!(self, Bucket::TextMergeable)
    }
}

/// An explicit hint parsed from an existing `.gitattributes` entry for a path.
/// When present it overrides extension-based classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrMarker {
    /// `lockable` (with or without `binary`/`-text`) → treated as binary-unmergeable.
    Lockable,
    /// `binary` / `-text -diff` without `lockable` → binary-unmergeable.
    Binary,
    /// `text` / `merge=...` / `diff` → forced text-mergeable.
    Text,
}

/// Extensions that are genuine binary, unmergeable artifacts → lock (E31).
const BINARY_EXTS: &[&str] = &[
    // CAD source-of-truth
    "f3d", "fcstd", "step", "stp", "iges", "igs", "sldprt", "sldasm", "ipt", "iam", "prt",
    // Mesh / print
    "stl", "3mf", "obj", "amf", "gcode",
    // Office / docs that are binary containers
    "pdf", "docx", "xlsx", "pptx", "doc", "xls", "ppt", "odt", "ods",
    // Images / media
    "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp", "ico", "psd", "ai", "eps",
    // Archives / blobs
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "bin", "exe", "dll", "so", "dylib",
];

/// Nominally text but merge-hostile KiCad sources → lock like binary (third bucket).
const NOMINAL_TEXT_EXTS: &[&str] = &["kicad_sch", "kicad_pcb"];

/// The extension of a path, lowercased. Handles KiCad's underscore extensions
/// (`board.kicad_pcb` → `kicad_pcb`). Returns "" when there is no extension.
fn extension_of(path: &str) -> String {
    // Take the final path segment so directory names with dots don't fool us.
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rsplit_once('.') {
        // A leading dot with no other dot (e.g. ".gitignore") is a dotfile, not an extension.
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Classify a single leaf file into exactly one [`Bucket`].
///
/// Pure, total and deterministic. `path` is any leaf file path (relative or absolute;
/// only the final segment's extension matters). `marker` is an optional explicit
/// `.gitattributes` hint for that path, which **overrides** extension classification.
///
/// Unknown extensions and extension-less files default to [`Bucket::TextMergeable`]:
/// git can attempt a merge, which is the safe, non-locking default for plain text.
pub fn classify(path: &str, marker: Option<AttrMarker>) -> Bucket {
    // An existing marker is the maintainer's explicit decision — trust it (E18/E24).
    if let Some(m) = marker {
        return match m {
            AttrMarker::Lockable | AttrMarker::Binary => Bucket::BinaryUnmergeable,
            AttrMarker::Text => Bucket::TextMergeable,
        };
    }

    let ext = extension_of(path);
    if NOMINAL_TEXT_EXTS.contains(&ext.as_str()) {
        Bucket::NominalTextUnmergeable
    } else if BINARY_EXTS.contains(&ext.as_str()) {
        Bucket::BinaryUnmergeable
    } else {
        Bucket::TextMergeable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_covers_every_bucket_and_the_override() {
        // table: (path, existing marker) -> expected bucket
        let cases: &[(&str, Option<AttrMarker>, Bucket)] = &[
            // --- text-mergeable: real text, git merges ---
            ("firmware/main.c", None, Bucket::TextMergeable),
            ("docs/README.md", None, Bucket::TextMergeable),
            ("bom.csv", None, Bucket::TextMergeable),
            ("config.yaml", None, Bucket::TextMergeable),
            // unknown extension -> safe text default
            ("notes.weirdext", None, Bucket::TextMergeable),
            // no extension at all -> text default
            ("Makefile", None, Bucket::TextMergeable),
            // dotfile is not an extension
            (".gitignore", None, Bucket::TextMergeable),
            // KiCad project file is not merge-hostile -> text default
            ("elektronik/board.kicad_pro", None, Bucket::TextMergeable),
            // --- binary-unmergeable: lock ---
            ("mechanik/gehaeuse.f3d", None, Bucket::BinaryUnmergeable),
            ("halter.STEP", None, Bucket::BinaryUnmergeable),
            ("part.stl", None, Bucket::BinaryUnmergeable),
            ("render.png", None, Bucket::BinaryUnmergeable),
            ("datasheet.pdf", None, Bucket::BinaryUnmergeable),
            // --- nominal-text-unmergeable: KiCad ---
            ("elektronik/board.kicad_pcb", None, Bucket::NominalTextUnmergeable),
            ("elektronik/board.kicad_sch", None, Bucket::NominalTextUnmergeable),
            // --- override: existing marker wins over extension ---
            // a .kicad_pcb explicitly marked text -> forced mergeable
            ("board.kicad_pcb", Some(AttrMarker::Text), Bucket::TextMergeable),
            // a plain .txt explicitly marked lockable -> binary-unmergeable
            ("weird.txt", Some(AttrMarker::Lockable), Bucket::BinaryUnmergeable),
            // a .md explicitly marked binary -> binary-unmergeable
            ("notes.md", Some(AttrMarker::Binary), Bucket::BinaryUnmergeable),
            // an .f3d explicitly marked text -> forced mergeable (override beats default)
            ("body.f3d", Some(AttrMarker::Text), Bucket::TextMergeable),
        ];
        for (path, marker, expected) in cases {
            assert_eq!(
                classify(path, *marker),
                *expected,
                "classify({path:?}, {marker:?})"
            );
        }
    }

    #[test]
    fn classify_is_case_insensitive_on_extension() {
        assert_eq!(classify("A.F3D", None), classify("a.f3d", None));
        assert_eq!(classify("B.KiCad_Pcb", None), Bucket::NominalTextUnmergeable);
    }

    #[test]
    fn extension_of_takes_final_segment_only() {
        // a dot in a directory name must not be read as the file's extension
        assert_eq!(extension_of("v1.2/notes"), "");
        assert_eq!(extension_of("v1.2/board.kicad_pcb"), "kicad_pcb");
        assert_eq!(extension_of("plain"), "");
    }

    #[test]
    fn classify_is_total_over_arbitrary_input() {
        // empty path, trailing dot, hidden dirs — every input yields a bucket, never panics
        for p in ["", ".", "a.", "a/b/", "/abs/x.STL", "x.kicad_sch.bak"] {
            let _ = classify(p, None); // must not panic; total function
        }
        // ".bak" wins as the trailing extension here -> text default
        assert_eq!(classify("x.kicad_sch.bak", None), Bucket::TextMergeable);
    }

    #[test]
    fn lockable_buckets_lock_only_unmergeable() {
        assert!(!Bucket::TextMergeable.is_lockable());
        assert!(Bucket::BinaryUnmergeable.is_lockable());
        assert!(Bucket::NominalTextUnmergeable.is_lockable());
    }
}
