//! Pattern-Zuordnung (Issue #47) — der reine, totale, deterministische Kern.
//!
//! Aus **Dateien auf der Platte** werden per Konvention **Artefakt-Karten** auf der Werkbank —
//! ohne Bürokratie pro Datei. Dieses Modul ist die **tiefe Kern**-Funktion: gegeben ein Pfad und
//! der **Glob-Satz** eines Produkt-Stacks (die geordneten Globs je Baustein, ADR 0003) entscheidet
//! es **total**:
//!
//! > `Pfad + Glob-Satz → Artefakt | Waise`
//!
//! Jeder Pfad fällt in **genau ein** Artefakt **oder** wird zur **Waise** (eine erfasste Datei
//! ohne Etikett). Durch Auslassung geht nichts verloren; der Ordner-Kontext der Waise bleibt als
//! Zuordnungs-Hinweis erhalten.
//!
//! **Hauptdatei** = der Treffer mit der höchsten Glob-Priorität (Baustein-`globs[0]` ist die
//! Hauptdatei-Regel — der erste, höchstpriorisierte Glob, ADR 0003 / `baustein.rs`).
//!
//! **Abgeleitete primäre Aktion**: gibt es eine dominante Einzeldatei (genau die Hauptdatei,
//! sonst nichts Gleichwertiges), wird **diese Datei** geöffnet; sonst der **Ordner**. Geöffnet
//! wird im OS-Standardprogramm (kein app-internes Programm-Mapping in v1, PRD §14).
//!
//! Wie im Haus üblich: **reiner Kern hier, kein I/O.** Der Plattenlauf + das Öffnen leben in der
//! dünnen Glue-Schicht (`werkbank.rs`); dieses Modul entscheidet nur. `#[cfg(test)]`-Tabellentests
//! belegen die Totalität über das Kreuzprodukt.

use serde::Serialize;

/// Der Glob-Satz **eines Bausteins** im Produkt-Stack: seine Identität + Heimat-Ordner + die
/// **geordneten** Globs (Index 0 = Hauptdatei-Regel). Rein abgeleitet aus `baustein.rs`; dieses
/// Modul liest nur, mutiert nichts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BausteinRegel {
    /// Stabile Baustein-`id`, z.B. `"kicad"`. Teil des Artefakt-Schlüssels.
    pub id: String,
    /// Menschlicher Name, z.B. `"KiCad"`. Trägt das Karten-Label.
    pub name: String,
    /// Heimat-Ordner (Arbeitsbereich), z.B. `"elektronik"`. Begrenzt den Wirkungsbereich der
    /// Globs: ein Glob greift nur **innerhalb** dieser Heimat (leere Heimat = Produktwurzel).
    pub heimat: String,
    /// Geordnete Artefakt-Globs; `[0]` ist die Hauptdatei-Regel (höchste Priorität).
    pub globs: Vec<String>,
    /// Label-only stillgelegt (PRD §10): greift nicht mehr, nichts wird gelöscht. Ein
    /// stillgelegter Baustein ordnet keine Datei mehr zu — die Datei fällt dann auf andere
    /// Regeln zurück oder wird zur Waise.
    pub stillgelegt: bool,
}

/// Wohin ein Pfad fällt: in **genau ein** Artefakt **oder** zur **Waise** (total).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Zuordnung {
    /// Dem Artefakt `artefakt_id` zugeordnet, getroffen durch `glob` mit Priorität `prioritaet`
    /// (kleiner = höher; 0 ist die Hauptdatei-Regel).
    Artefakt {
        /// Stabiler Artefakt-Schlüssel: `"<baustein-id>:<ordner>"` (siehe [`artefakt_key`]).
        artefakt_id: String,
        /// Index des treffenden Globs in den Baustein-Globs (0 = Hauptdatei-Regel).
        prioritaet: usize,
        /// Der konkrete Glob, der getroffen hat (für Diagnose/Test).
        glob: String,
    },
    /// Eine erfasste Datei **ohne** Etikett. Der `ordner`-Kontext bleibt als Hinweis erhalten.
    Waise {
        /// Ordner der Datei relativ zur Produktwurzel (Vorwärts-Slashes; `""` an der Wurzel).
        ordner: String,
    },
}

/// Stabiler Artefakt-Schlüssel aus Baustein-`id` und dem **Ordner** des Pfads. Files, die vom
/// selben Baustein im selben Ordner getroffen werden, bilden **ein** Artefakt — die Konvention,
/// die aus Dateien Karten macht, ohne Pro-Datei-Bürokratie.
pub fn artefakt_key(baustein_id: &str, ordner: &str) -> String {
    format!("{baustein_id}:{ordner}")
}

/// Der Ordner-Anteil eines Pfads (alles vor dem letzten `/`), Vorwärts-Slashes; `""` an der
/// Wurzel. Rein und total.
fn ordner_of(path: &str) -> String {
    let p = normalize(path);
    match p.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// Der letzte Pfadsegment-Name (Dateiname), Vorwärts-Slashes. Rein und total.
fn name_of(path: &str) -> &str {
    let p = path.trim_end_matches('/');
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

/// Vorwärts-Slash-Normalform: Backslashes → `/`, doppelte Slashes zusammengefasst, führende/
/// nachlaufende Slashes entfernt. Total.
fn normalize(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut prev_slash = false;
    for ch in path.chars() {
        let c = if ch == '\\' { '/' } else { ch };
        if c == '/' {
            if !prev_slash {
                out.push('/');
            }
            prev_slash = true;
        } else {
            out.push(c);
            prev_slash = false;
        }
    }
    out.trim_matches('/').to_string()
}

/// Ob `path` **innerhalb** der Heimat `heimat` liegt (oder Heimat leer = ganze Produktwurzel).
/// Vergleich auf Segmentgrenze, damit `elektro` nicht in `elektronik` „passt". Total.
fn within_heimat(path: &str, heimat: &str) -> bool {
    let h = normalize(heimat);
    if h.is_empty() {
        return true;
    }
    let p = normalize(path);
    p == h || p.starts_with(&format!("{h}/"))
}

/// Ob ein einzelnes **Glob** den Dateinamen trifft. Bewusst minimal und **total**, passend zur
/// Glob-Form der Bibliothek (`*.ext`, fester Dateiname wie `platformio.ini`, sowie `name.*`):
///
/// - `*.<ext>`  → endet (case-insensitiv) auf `.<ext>`.
/// - `<name>.*` → beginnt (case-insensitiv) auf `<name>.`.
/// - sonst Glob mit `*`/`?` → einfaches Wildcard-Matching über den Dateinamen.
/// - ohne Wildcard → exakter (case-insensitiver) Dateiname.
///
/// Verzeichnis-Globs (Endung `/`, z.B. `build/`) treffen **nie** eine Datei — sie sind
/// Ignore-Muster, keine Artefakt-Globs.
fn glob_matches(glob: &str, file_name: &str) -> bool {
    if glob.is_empty() || glob.ends_with('/') {
        return false;
    }
    let name = file_name.to_ascii_lowercase();
    let g = glob.to_ascii_lowercase();
    if let Some(ext) = g.strip_prefix("*.") {
        // `*.ext` — kein weiteres Wildcard im Rest: schneller, häufigster Pfad.
        if !ext.contains(['*', '?']) {
            return name.ends_with(&format!(".{ext}")) && name.len() > ext.len() + 1;
        }
    }
    if !g.contains(['*', '?']) {
        return name == g;
    }
    wildcard_matches(&g, &name)
}

/// Totales `*`/`?`-Wildcard-Matching (greedy, ohne Backtracking-Stack-Risiko) über zwei
/// lowercased ASCII/Unicode-Strings. `*` = beliebig viele, `?` = genau ein Zeichen.
fn wildcard_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// **Der reine Kern**: ordne **einen** Pfad gegen den Glob-Satz zu — **total**. Ein Pfad fällt in
/// genau ein Artefakt (erster Baustein-Treffer in Listenreihenfolge, höchste Glob-Priorität
/// innerhalb des Bausteins) **oder** zur Waise.
///
/// Determinismus-Regeln:
/// 1. Bausteine werden in `regeln`-Reihenfolge geprüft; der **erste** Baustein mit Heimat-Match
///    und Glob-Treffer gewinnt das Artefakt (stabil, deterministisch).
/// 2. Innerhalb eines Bausteins gewinnt der **kleinste** Glob-Index (höchste Priorität).
/// 3. Stillgelegte Bausteine (PRD §10) und Verzeichnis-Globs treffen nie.
/// 4. Kein Treffer → Waise, Ordner-Kontext erhalten.
pub fn zuordnen(path: &str, regeln: &[BausteinRegel]) -> Zuordnung {
    let file = name_of(path);
    let ordner = ordner_of(path);

    // Versteckte/leere „Dateien" sind keine Artefakte — sie fallen sauber auf Waise zurück, statt
    // ein Glob zu fälschen. (Leerer Dateiname kann nur bei degeneriertem Input auftreten.)
    if !file.is_empty() && !file.starts_with('.') {
        for regel in regeln {
            if regel.stillgelegt || !within_heimat(path, &regel.heimat) {
                continue;
            }
            // Höchste Priorität = kleinster Index unter den treffenden Globs.
            let hit = regel
                .globs
                .iter()
                .enumerate()
                .find(|(_, g)| glob_matches(g, file));
            if let Some((prioritaet, glob)) = hit {
                return Zuordnung::Artefakt {
                    artefakt_id: artefakt_key(&regel.id, &ordner),
                    prioritaet,
                    glob: glob.clone(),
                };
            }
        }
    }
    Zuordnung::Waise { ordner }
}

/// Die abgeleitete **primäre Aktion** einer Artefakt-Karte (PRD §14). `Auto` löst sich aus dem
/// Bestand auf: gibt es eine **dominante Einzeldatei** (die Hauptdatei ist eindeutig — nur sie
/// trifft die höchste Priorität), wird die Datei geöffnet, sonst der Ordner. Eine ausdrückliche
/// Baustein-Wahl (`Datei`/`Ordner`) bleibt. Rein und total — dieselbe Tabelle wie
/// `Baustein::resolve_oeffnen`, hier aber an die abgeleitete Dominanz gekoppelt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimaerAktion {
    /// Die Hauptdatei im OS-Standardprogramm öffnen.
    Datei,
    /// Den Heimat-/Artefakt-Ordner im Dateimanager öffnen.
    Ordner,
}

/// Die Öffnen-Konfiguration eines Bausteins (gespiegelt aus `baustein::Oeffnen`, hier ohne
/// Abhängigkeit auf das Modell, damit der Kern eigenständig testbar bleibt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OeffnenKonfig {
    /// Dominante Einzeldatei → Datei, sonst Ordner.
    Auto,
    /// Immer die Hauptdatei.
    Datei,
    /// Immer den Ordner.
    Ordner,
}

/// Leite die primäre Aktion ab. `has_dominant_file` = es gibt eine eindeutige Hauptdatei (genau
/// eine Datei trifft die höchste vorkommende Priorität). Total + rein.
pub fn primaer_aktion(konfig: OeffnenKonfig, has_dominant_file: bool) -> PrimaerAktion {
    match konfig {
        OeffnenKonfig::Datei => PrimaerAktion::Datei,
        OeffnenKonfig::Ordner => PrimaerAktion::Ordner,
        OeffnenKonfig::Auto if has_dominant_file => PrimaerAktion::Datei,
        OeffnenKonfig::Auto => PrimaerAktion::Ordner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regel(id: &str, heimat: &str, globs: &[&str]) -> BausteinRegel {
        BausteinRegel {
            id: id.to_string(),
            name: id.to_string(),
            heimat: heimat.to_string(),
            globs: globs.iter().map(|s| s.to_string()).collect(),
            stillgelegt: false,
        }
    }

    /// The real bundled Bibliothek glob sets (kicad / fusion / doku / platformio), so the table
    /// proves totality against the actual conventions, not toy data.
    fn stack() -> Vec<BausteinRegel> {
        vec![
            regel("kicad", "elektronik", &["*.kicad_pro", "*.kicad_sch", "*.kicad_pcb"]),
            regel("fusion", "mechanik", &["*.f3d", "*.f3z", "*.step", "*.stp", "*.stl"]),
            regel("doku", "doku", &["*.md", "*.pdf"]),
            regel(
                "platformio",
                "firmware",
                &["platformio.ini", "*.c", "*.cpp", "*.h", "*.hpp", "*.ino"],
            ),
        ]
    }

    #[test]
    fn zuordnung_is_total_every_path_maps_to_exactly_one_artifact_or_waise() {
        let s = stack();
        // table: path -> expected (Some((baustein_id, prioritaet)) for an artifact, None for Waise)
        let cases: &[(&str, Option<(&str, usize)>)] = &[
            // --- kicad in its Heimat ---
            ("elektronik/regler/regler.kicad_pro", Some(("kicad", 0))), // Hauptdatei-Regel
            ("elektronik/regler/regler.kicad_sch", Some(("kicad", 1))),
            ("elektronik/regler/regler.kicad_pcb", Some(("kicad", 2))),
            // --- fusion ---
            ("mechanik/gehaeuse/gehaeuse.f3d", Some(("fusion", 0))),
            ("mechanik/gehaeuse/gehaeuse.step", Some(("fusion", 2))),
            ("mechanik/gehaeuse/gehaeuse.stl", Some(("fusion", 4))),
            // --- doku ---
            ("doku/handbuch.md", Some(("doku", 0))),
            ("doku/datenblatt.pdf", Some(("doku", 1))),
            // --- platformio: fixed name + extension globs ---
            ("firmware/platformio.ini", Some(("platformio", 0))),
            ("firmware/src/main.c", Some(("platformio", 1))),
            ("firmware/src/app.cpp", Some(("platformio", 2))),
            ("firmware/include/api.h", Some(("platformio", 3))),
            // --- Waisen: tracked file lacking a label, folder context preserved ---
            ("elektronik/regler/notizen.txt", None), // unknown ext in a known Heimat
            ("README.md", None),                     // doku glob, but at root, NOT in doku/ Heimat
            ("mechanik/gehaeuse/render.png", None),  // not a fusion glob
            ("bom.csv", None),                       // root, no Heimat
            ("scratch/random.xyz", None),            // unrelated folder
            // --- cross-Heimat protection: a .kicad_pcb OUTSIDE elektronik is a Waise ---
            ("mechanik/board.kicad_pcb", None),
            // --- segment-boundary: `elektronikx` must NOT match the `elektronik` Heimat ---
            ("elektronikx/x.kicad_pro", None),
            // --- degenerate / hidden inputs stay total (never panic, never a false artifact) ---
            ("", None),
            (".", None),
            ("elektronik/.hidden", None),
            ("mechanik/", None),
        ];
        for (path, expected) in cases {
            let got = zuordnen(path, &s);
            match (expected, &got) {
                (Some((id, prio)), Zuordnung::Artefakt { artefakt_id, prioritaet, .. }) => {
                    assert!(
                        artefakt_id.starts_with(&format!("{id}:")),
                        "path {path:?}: expected baustein {id}, got {artefakt_id}"
                    );
                    assert_eq!(*prioritaet, *prio, "path {path:?}: priority mismatch");
                }
                (None, Zuordnung::Waise { .. }) => {}
                _ => panic!("path {path:?}: expected {expected:?}, got {got:?}"),
            }
        }
    }

    #[test]
    fn artefakt_key_groups_by_baustein_and_folder() {
        // Two files matched by the same Baustein in the same folder share one artifact key.
        let s = stack();
        let a = zuordnen("elektronik/regler/regler.kicad_pro", &s);
        let b = zuordnen("elektronik/regler/regler.kicad_pcb", &s);
        let key = |z: &Zuordnung| match z {
            Zuordnung::Artefakt { artefakt_id, .. } => artefakt_id.clone(),
            _ => unreachable!(),
        };
        assert_eq!(key(&a), key(&b));
        assert_eq!(key(&a), "kicad:elektronik/regler");

        // A different folder under the same Heimat is a different artifact.
        let c = zuordnen("elektronik/sensor/sensor.kicad_pro", &s);
        assert_ne!(key(&a), key(&c));
        assert_eq!(key(&c), "kicad:elektronik/sensor");
    }

    #[test]
    fn first_matching_baustein_wins_when_two_overlap() {
        // Two Bausteine both claim *.c in firmware (platformio + zephyr-like). First listed wins.
        let regeln = vec![
            regel("platformio", "firmware", &["*.c", "*.h"]),
            regel("zephyr", "firmware", &["*.c", "*.h"]),
        ];
        match zuordnen("firmware/main.c", &regeln) {
            Zuordnung::Artefakt { artefakt_id, .. } => assert!(artefakt_id.starts_with("platformio:")),
            z => panic!("expected artifact, got {z:?}"),
        }
    }

    #[test]
    fn stillgelegt_baustein_yields_waise_then_falls_through() {
        // A stillgelegter Baustein must not claim; the file falls to the next rule or a Waise.
        let mut decommissioned = regel("kicad", "elektronik", &["*.kicad_pro"]);
        decommissioned.stillgelegt = true;
        let regeln = vec![decommissioned];
        assert!(matches!(
            zuordnen("elektronik/x.kicad_pro", &regeln),
            Zuordnung::Waise { .. }
        ));
    }

    #[test]
    fn glob_matches_covers_the_glob_forms_in_the_bibliothek() {
        // *.ext (case-insensitive), exact filename, name.*, and the directory-glob veto.
        assert!(glob_matches("*.kicad_pcb", "Board.KiCad_Pcb"));
        assert!(glob_matches("*.md", "readme.md"));
        assert!(!glob_matches("*.md", "md")); // ".md" needs a stem before it
        assert!(glob_matches("platformio.ini", "platformio.ini"));
        assert!(!glob_matches("platformio.ini", "platformio.cfg"));
        assert!(glob_matches("regler.*", "regler.kicad_pro"));
        assert!(glob_matches("v?.step", "v1.step"));
        assert!(!glob_matches("v?.step", "v12.step"));
        assert!(!glob_matches("build/", "build")); // directory glob never matches a file
        assert!(!glob_matches("", "anything"));
    }

    #[test]
    fn within_heimat_respects_segment_boundaries() {
        assert!(within_heimat("elektronik/x.kicad_pro", "elektronik"));
        assert!(within_heimat("elektronik", "elektronik"));
        assert!(!within_heimat("elektronikx/x", "elektronik"));
        assert!(!within_heimat("x.kicad_pro", "elektronik")); // root file, wrong Heimat
        assert!(within_heimat("anywhere/x", "")); // empty Heimat = whole product
    }

    #[test]
    fn ordner_and_name_split_paths_totally() {
        assert_eq!(ordner_of("a/b/c.txt"), "a/b");
        assert_eq!(ordner_of("c.txt"), "");
        assert_eq!(ordner_of(""), "");
        assert_eq!(name_of("a/b/c.txt"), "c.txt");
        assert_eq!(name_of("c.txt"), "c.txt");
        assert_eq!(name_of("a\\b\\c.txt"), "c.txt"); // backslashes too
    }

    #[test]
    fn primaer_aktion_covers_the_decision_table() {
        // table: (konfig, has_dominant_file) -> action
        let cases: &[(OeffnenKonfig, bool, PrimaerAktion)] = &[
            (OeffnenKonfig::Auto, true, PrimaerAktion::Datei),
            (OeffnenKonfig::Auto, false, PrimaerAktion::Ordner),
            (OeffnenKonfig::Datei, true, PrimaerAktion::Datei),
            (OeffnenKonfig::Datei, false, PrimaerAktion::Datei),
            (OeffnenKonfig::Ordner, true, PrimaerAktion::Ordner),
            (OeffnenKonfig::Ordner, false, PrimaerAktion::Ordner),
        ];
        for (konfig, dominant, expected) in cases {
            assert_eq!(
                primaer_aktion(*konfig, *dominant),
                *expected,
                "konfig = {konfig:?}, dominant = {dominant}"
            );
        }
    }

    #[test]
    fn wildcard_matches_is_total_and_correct() {
        assert!(wildcard_matches("a*c", "abc"));
        assert!(wildcard_matches("a*c", "ac"));
        assert!(wildcard_matches("*", ""));
        assert!(wildcard_matches("*.*", "x.y"));
        assert!(!wildcard_matches("a*c", "ab"));
        assert!(wildcard_matches("a?c", "abc"));
        assert!(!wildcard_matches("a?c", "ac"));
        // never panics on adversarial input
        for p in ["", "*", "***", "?", "*?*"] {
            for t in ["", "x", "xyz"] {
                let _ = wildcard_matches(p, t);
            }
        }
    }
}
