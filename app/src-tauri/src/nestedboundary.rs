//! Genestetes `.git` als opake Grenze — das **Nested-Grenze-Prädikat** (Issue #130, E50a).
//!
//! Frameworks, die git-native Toolchains (`west`, ESP-IDF, PlatformIO, `venv`) in einen
//! Baustein ziehen, legen **genestete `.git`** und tausende vendored Dateien an. Werkbank
//! behandelt jedes genestete `.git`/Submodul als **opake Grenze**: hinter ihr liegt ein
//! fremder Baum, in den weder Watcher, noch Klassifizierer, noch Projektion hineinsehen. So
//! löst der fremde Baum keine Commit-Lawine aus und verwirrt die Status-/Commit-Logik nicht.
//!
//! Diese Datei ist der **reiner Kern + Tabellentest** im Muster von [`crate::classifier`] und
//! [`crate::projection`]: rein, total, deterministisch, **kein I/O**. Der Walk über die
//! Platte (`std::fs::read_dir`) lebt in den Glue-Schichten ([`crate::watcher`],
//! [`crate::projection`]); hier wird nur **entschieden**.
//!
//! Das Modell ist bewusst klein:
//! - Aus einem **Walk** (den entdeckten `.git`-Fundstellen) baut [`boundary_set`] die
//!   **Grenzmenge** — die produkt-relativen Verzeichnisse, an denen gestoppt wird.
//! - Das **Wurzel-`.git`** ist *keine* Grenze: es ist das eigene Repo des Produkts. Nur
//!   *genestete* `.git` (Tiefe ≥ 1) sind opake Grenzen.
//! - [`Boundary::stops_descent_into`] beantwortet beim Abstieg: ist dieses Verzeichnis selbst
//!   eine Grenze? (Dann nicht hineinsteigen.)
//! - [`Boundary::contains`] beantwortet für einen beliebigen Pfad: liegt er **hinter** einer
//!   Grenze (im fremden Baum)? (Dann ignorieren — kein Commit, keine Klassifizierung.)

use std::collections::BTreeSet;

/// Der Verzeichnisname, der eine git-Grenze markiert. Ein Eintrag mit diesem Namen — ob
/// echtes Verzeichnis (normales Repo/Submodul) oder Datei (`.git`-Datei eines Submoduls,
/// `gitdir: …`) — schließt seinen **Elternordner** als opake Grenze ab.
pub const GIT_MARKER: &str = ".git";

/// Die **Grenzmenge** eines Produkts: die produkt-relativen Verzeichnisse, an denen der Walk
/// stoppt, weil dort ein genestetes `.git` sitzt. Forward-slash-Pfade, ohne führenden/folgenden
/// `/`. Ein leerer Pfad (das Produkt-Wurzel-`.git`) ist **nie** enthalten — er ist das eigene
/// Repo, keine Grenze.
///
/// Konstruiert ausschließlich über [`boundary_set`] aus einem Walk; ab dann rein abfragbar via
/// [`Boundary::contains`] und [`Boundary::stops_descent_into`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Boundary {
    /// Die Stop-Punkte (Heimat-Ordner der genesteten `.git`), normalisiert & sortiert.
    stops: BTreeSet<String>,
}

/// Normalisiere einen produkt-relativen Pfad auf die Grenzmengen-Form: Backslashes zu
/// Slashes, führende/folgende/doppelte `/` und `.`-Segmente raus. Total über jeden String.
fn normalize(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|seg| !seg.is_empty() && *seg != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Baue die **Grenzmenge** aus einem Walk. `git_locations` sind die produkt-relativen Pfade
/// **der gefundenen `.git`-Einträge** selbst (z. B. `firmware/west/.git`), wie ein Walk sie
/// aufsammelt — egal ob Verzeichnis oder Submodul-`.git`-Datei.
///
/// Rein, total, deterministisch, **kein I/O**. Pro Fundstelle wird der **Elternordner** zur
/// Grenze (das `.git`-Segment selbst fällt weg). Das **Wurzel-`.git`** (Elternordner = leer)
/// wird verworfen: es ist das eigene Repo des Produkts und niemals eine opake Grenze.
/// Reihenfolge und Duplikate im Input sind egal — die `BTreeSet` normalisiert beides.
pub fn boundary_set<I, S>(git_locations: I) -> Boundary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut stops = BTreeSet::new();
    for loc in git_locations {
        let norm = normalize(loc.as_ref());
        // Der Elternordner der `.git`-Fundstelle ist der Stop-Punkt.
        let parent = match norm.strip_suffix(GIT_MARKER) {
            Some(p) => normalize(p),
            // Kein `.git`-Endsegment → keine Grenze (defensiv; ein Walk liefert nur `.git`).
            None => continue,
        };
        // Wurzel-`.git` (leerer Elternordner) ist das eigene Repo, keine Grenze.
        if parent.is_empty() {
            continue;
        }
        stops.insert(parent);
    }
    Boundary { stops }
}

impl Boundary {
    /// Die Stop-Punkte als sortierte, produkt-relative Pfade. Für Tests und für Glue, die die
    /// Grenze protokollieren oder anzeigen will.
    pub fn stops(&self) -> Vec<String> {
        self.stops.iter().cloned().collect()
    }

    /// Gibt es überhaupt eine genestete Grenze? Reines Produkt ohne fremde Toolchain → `false`.
    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// Soll der Walk in dieses Verzeichnis **nicht** absteigen, weil es **selbst** eine Grenze
    /// ist (dort sitzt das genestete `.git`)? Genau dann `true`, wenn `dir` (normalisiert) ein
    /// Stop-Punkt ist. Das ist die Frage, die der rekursive Abstieg an jedem Unterordner stellt.
    pub fn stops_descent_into(&self, dir: &str) -> bool {
        self.stops.contains(&normalize(dir))
    }

    /// Liegt `path` **hinter** einer Grenze, also im fremden Baum (oder ist die Grenze selbst)?
    /// Genau dann `true`, wenn ein Stop-Punkt `path` ist **oder** ein Präfix-Verzeichnis von
    /// `path`. Das ist die Frage, die Watcher/Klassifizierer für einen einzelnen Pfad stellen:
    /// „gehört dieser Schreibvorgang / diese Datei zu fremdem Code?" → dann ignorieren.
    ///
    /// Präfix ist **segmentweise** (Pfad-Grenze), nicht textuell: `west` ist Grenze für
    /// `west/sub.c`, aber **nicht** für `westflügel/x.c`.
    pub fn contains(&self, path: &str) -> bool {
        let norm = normalize(path);
        self.stops.iter().any(|stop| is_under_or_eq(&norm, stop))
    }
}

/// Ist `path` gleich `dir` oder liegt segmentweise darunter? `""` (Wurzel) ist nie „unter"
/// einem nicht-leeren `dir`. Rein und total.
fn is_under_or_eq(path: &str, dir: &str) -> bool {
    if path == dir {
        return true;
    }
    // segmentweises Präfix: `dir` + "/" muss exakt am Pfadanfang stehen.
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Kern-Tabellentest: aus einem Walk (den `.git`-Fundstellen) entsteht die korrekte
    /// Stop-Menge — mit und ohne genestete `.git`, das Wurzel-`.git` verworfen.
    #[test]
    fn boundary_set_yields_correct_stops_from_walk() {
        // table: (walk-Fundstellen `.git`) -> erwartete Stop-Menge
        let cases: &[(&[&str], &[&str])] = &[
            // kein genestetes `.git` -> leere Grenzmenge
            (&[], &[]),
            // nur das Wurzel-`.git` -> keine Grenze (eigenes Repo)
            (&[".git"], &[]),
            // ein genestetes `.git` (west) -> dessen Elternordner ist Stop
            (&["firmware/west/.git"], &["firmware/west"]),
            // Wurzel-`.git` zusammen mit genestetem -> nur das genestete zählt
            (&[".git", "firmware/west/.git"], &["firmware/west"]),
            // mehrere genestete Toolchains (ESP-IDF, PlatformIO, venv)
            (
                &[
                    "firmware/esp-idf/.git",
                    "firmware/.pio/libdeps/x/.git",
                    "py/.venv/.git",
                ],
                &["firmware/.pio/libdeps/x", "firmware/esp-idf", "py/.venv"],
            ),
            // Submodul als `.git`-Datei (gitdir:) zählt genauso
            (&["libs/foo/.git"], &["libs/foo"]),
            // Duplikate & gemischte Trenner & führende Slashes -> einmal, normalisiert
            (
                &["firmware/west/.git", "firmware\\west\\.git", "/firmware/west/.git"],
                &["firmware/west"],
            ),
        ];
        for (walk, expected) in cases {
            let b = boundary_set(walk.iter().copied());
            assert_eq!(b.stops(), *expected, "walk = {walk:?}");
        }
    }

    #[test]
    fn empty_boundary_for_a_clean_product() {
        let b = boundary_set::<_, &str>([]);
        assert!(b.is_empty());
        // ohne Grenze stoppt nichts und nichts liegt „hinter" einer Grenze
        assert!(!b.stops_descent_into("firmware"));
        assert!(!b.contains("firmware/main.c"));
    }

    /// `stops_descent_into`: der Abstieg stoppt **am** Grenzordner, nicht davor, nicht dahinter.
    #[test]
    fn stops_descent_only_at_the_boundary_dir() {
        let b = boundary_set(["firmware/west/.git"]);
        // table: (kandidat-Verzeichnis) -> stoppt der Abstieg dort?
        let cases: &[(&str, bool)] = &[
            ("firmware", false),         // Elternordner: weiter absteigen
            ("firmware/west", true),     // die Grenze selbst: nicht hinein
            ("firmware/west/", true),    // mit Trailing-Slash: gleiche Grenze
            ("firmware\\west", true),    // mit Backslash: gleiche Grenze
            ("firmware/westflügel", false), // ähnlicher Name, andere Grenze
            ("firmware/west/sub", false),   // läge schon hinter der Grenze -> hier nicht erreicht
        ];
        for (dir, expected) in cases {
            assert_eq!(b.stops_descent_into(dir), *expected, "dir = {dir:?}");
        }
    }

    /// `contains`: ein einzelner Pfad liegt hinter der Grenze (fremder Code) oder nicht.
    #[test]
    fn contains_is_segmentwise_and_total() {
        let b = boundary_set(["firmware/west/.git", "py/.venv/.git"]);
        // table: (kandidat-Pfad) -> liegt er hinter einer Grenze?
        let cases: &[(&str, bool)] = &[
            ("firmware/west", true),            // die Grenze selbst
            ("firmware/west/drivers/uart.c", true), // tief im fremden Baum
            ("firmware/west/.git/HEAD", true),  // im fremden `.git`
            ("firmware/main.c", false),         // eigener Firmware-Code, vor der Grenze
            ("firmware/westflügel/x.c", false), // segmentweise: kein Präfix von westflügel
            ("py/.venv/lib/site.py", true),     // zweite Grenze (venv)
            ("py/app.py", false),               // eigener Python-Code
            ("", false),                        // Wurzel liegt vor jeder Grenze
            ("README.md", false),               // Wurzeldatei
        ];
        for (path, expected) in cases {
            assert_eq!(b.contains(path), *expected, "path = {path:?}");
        }
    }

    #[test]
    fn normalize_is_total_over_messy_input() {
        // führende/folgende/doppelte Slashes, Backslashes, `.`-Segmente -> saubere Form
        assert_eq!(normalize("/a/b/"), "a/b");
        assert_eq!(normalize("a\\b\\c"), "a/b/c");
        assert_eq!(normalize("a//b/./c"), "a/b/c");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("."), "");
        assert_eq!(normalize("///"), "");
    }

    #[test]
    fn is_under_or_eq_is_segmentwise() {
        assert!(is_under_or_eq("a/b", "a/b")); // gleich
        assert!(is_under_or_eq("a/b/c", "a/b")); // darunter
        assert!(!is_under_or_eq("a/bc", "a/b")); // kein Segment-Präfix
        assert!(!is_under_or_eq("a", "a/b")); // darüber
        assert!(!is_under_or_eq("", "a")); // Wurzel nie unter nicht-leerem dir
    }
}
