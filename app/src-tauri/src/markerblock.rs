//! Idempotente Marker-Blöcke in Dotfiles (Issue #48, adressiert auch #63) — der **reine Kern**.
//!
//! Beim Onboarding eines Bausteins in ein Produkt hängt das Werkzeug dessen Ignore-/LFS-Zeilen
//! **idempotent** in einen klar markierten Block der Dotfiles (`.gitignore`/`.gitattributes`).
//! Das Werkzeug findet so immer **seine eigenen** Zeilen wieder und fasst die Handarbeit des
//! Nutzers nie an. Die Dotfiles bleiben die **alleinige Wahrheit** für Ignore/LFS.
//!
//! Dieses Modul ist **rein, total, deterministisch** — kein I/O, kein Clock. Es bekommt den
//! aktuellen Dateitext, die Baustein-`id` und die gewünschten kanonischen Zeilen herein und gibt
//! den neuen Dateitext zurück. Die Seiteneffekt-Glue (Lesen/Schreiben der Dotfiles am Onboarding)
//! lebt in [`crate::onboardglue`]; alles Testbare lebt hier in `#[cfg(test)]`-Tabellentests.
//!
//! # Marker-Block-Format
//! ```text
//! # >>> baustein: <id> >>>
//! <zeile 1>
//! <zeile 2>
//! # <<< baustein: <id> <<<
//! ```
//!
//! # Hand-Edit-Semantik (bewusste, konservative Wahl — siehe Issue #48)
//! Der Marker-Block ist **Hoheitsgebiet des Werkzeugs**: Sein Inhalt wird beim Onboarding auf
//! die kanonischen Zeilen **neu geschrieben** (idempotent — zweimal == einmal, nie Duplikate,
//! nie Anhängen außerhalb). Alles **außerhalb** jedes Marker-Blocks ist unantastbar und
//! überlebt unverändert — das sind die Hand-Edits, die immer gewinnen. Wir fassen „Hand-Edits
//! innerhalb des Marker-Blocks gewinnen" also bewusst konservativ: Wer Werkzeug-Zeilen behalten
//! will, schreibt sie **außerhalb** des Blocks; innerhalb des Blocks regiert das Werkzeug.

/// Präfix der **Start**-Markerzeile eines Baustein-Blocks (ohne `id`).
const START_PREFIX: &str = "# >>> baustein: ";
/// Suffix der **Start**-Markerzeile.
const START_SUFFIX: &str = " >>>";
/// Präfix der **End**-Markerzeile eines Baustein-Blocks (ohne `id`).
const END_PREFIX: &str = "# <<< baustein: ";
/// Suffix der **End**-Markerzeile.
const END_SUFFIX: &str = " <<<";

/// Die Start-Markerzeile für eine Baustein-`id`.
fn start_marker(id: &str) -> String {
    format!("{START_PREFIX}{id}{START_SUFFIX}")
}

/// Die End-Markerzeile für eine Baustein-`id`.
fn end_marker(id: &str) -> String {
    format!("{END_PREFIX}{id}{END_SUFFIX}")
}

/// Liest die `id` aus einer Start-Markerzeile, falls die Zeile eine solche ist.
fn id_of_start(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix(START_PREFIX)
        .and_then(|rest| rest.strip_suffix(START_SUFFIX))
        .map(str::trim)
}

/// Erkennt eine End-Markerzeile für die gegebene `id`.
fn is_end_marker(line: &str, id: &str) -> bool {
    line.trim() == end_marker(id)
}

/// Den Marker-Block eines Bausteins **idempotent** in `existing` setzen und den neuen Text
/// zurückgeben. Reine, totale Funktion (kein I/O).
///
/// Regeln (alle tabellengetestet):
/// - Ist kein Block für `id` vorhanden, wird genau einer am **Ende** angehängt (mit sauberer
///   Leerzeilen-Trennung zum vorhandenen Inhalt).
/// - Ist genau ein Block vorhanden, wird **nur dessen Inhalt** auf `lines` neu geschrieben;
///   alles davor/danach bleibt **byte-genau** erhalten.
/// - Sind (durch Handarbeit) mehrere Blöcke derselben `id` vorhanden, wird der **erste** zur
///   kanonischen Quelle neu geschrieben und die weiteren entfernt (Selbstheilung zu „genau einer").
/// - Inhalt **außerhalb** jedes Marker-Blocks ist unantastbar.
/// - Sind `lines` leer, bleibt der Block leer (nur die beiden Markerzeilen) bzw. wird ein leerer
///   Block angelegt — die Marker bleiben als Anker für künftige Läufe stehen.
/// - **Idempotent:** `upsert_block(upsert_block(x)) == upsert_block(x)`.
pub fn upsert_block(existing: &str, id: &str, lines: &[String]) -> String {
    let start = start_marker(id);

    // Den vorhandenen Text in Segmente zerlegen: alle Zeilen, dabei die Blöcke dieser `id`
    // herausschneiden. Wir sammeln die „Außen"-Zeilen unverändert und merken uns die Stelle des
    // ersten Blocks, an die der kanonische Block wieder eingesetzt wird.
    let mut outside: Vec<&str> = Vec::new();
    let mut insert_at: Option<usize> = None;

    let mut iter = existing.lines().peekable();
    while let Some(line) = iter.next() {
        if id_of_start(line) == Some(id) {
            // Beginn eines Blocks dieser id: bis zum passenden End-Marker (inklusive) wegwerfen.
            if insert_at.is_none() {
                insert_at = Some(outside.len());
            }
            while let Some(inner) = iter.next() {
                if is_end_marker(inner, id) {
                    break;
                }
            }
            // Ein evtl. fehlender End-Marker (verstümmelte Datei) verschluckt den Rest bewusst
            // nicht: der `while`-Lauf endet dann am Dateiende, der Block gilt als beendet.
        } else {
            outside.push(line);
        }
    }

    // Die kanonischen Blockzeilen bauen.
    let mut block: Vec<String> = Vec::with_capacity(lines.len() + 2);
    block.push(start);
    block.extend(lines.iter().cloned());
    block.push(end_marker(id));

    // Block an der gemerkten Stelle einsetzen, sonst am Ende anhängen.
    let mut result: Vec<String> = Vec::new();
    match insert_at {
        Some(pos) => {
            result.extend(outside[..pos].iter().map(|s| s.to_string()));
            result.extend(block);
            result.extend(outside[pos..].iter().map(|s| s.to_string()));
        }
        None => {
            result.extend(outside.iter().map(|s| s.to_string()));
            result.extend(block);
        }
    }

    render(&result, existing)
}

/// Fügt die Zeilen zu einem Text zusammen. Erhält einen abschließenden Zeilenumbruch (Dotfiles
/// enden konventionell mit `\n`). Eine vollständig leere Eingabe bleibt leer.
fn render(lines: &[String], _original: &str) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ls(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// Idempotenz-Property: zweimal anwenden == einmal anwenden (die Kern-Akzeptanz).
    #[test]
    fn applying_twice_equals_once() {
        // table: (start text, id, lines)
        let cases: &[(&str, &str, &[&str])] = &[
            ("", "zephyr", &["build/", "twister-out/"]),
            ("*.log\n", "zephyr", &["build/"]),
            (
                "# meine Hand-Edits\nfoo/\n",
                "kicad",
                &["*-backups/", "*.autosave"],
            ),
            // leere Zeilenmenge -> leerer Block, immer noch idempotent
            ("bar/\n", "doku", &[]),
        ];
        for (text, id, lines) in cases {
            let once = upsert_block(text, id, &ls(lines));
            let twice = upsert_block(&once, id, &ls(lines));
            assert_eq!(once, twice, "nicht idempotent für id={id} text={text:?}");
        }
    }

    /// Anlegen in eine leere Datei: genau ein Block, kanonischer Inhalt, ein abschließendes \n.
    #[test]
    fn creates_single_block_in_empty_file() {
        let out = upsert_block("", "zephyr", &ls(&["build/", "twister-out/"]));
        assert_eq!(
            out,
            "# >>> baustein: zephyr >>>\nbuild/\ntwister-out/\n# <<< baustein: zephyr <<<\n"
        );
    }

    /// Inhalt außerhalb des Blocks bleibt byte-genau erhalten — davor UND danach.
    #[test]
    fn content_outside_the_block_is_never_touched() {
        let original = "# Hand-Edit oben\nmeins/\n\n# >>> baustein: kicad >>>\nALT\n# <<< baustein: kicad <<<\n\n# Hand-Edit unten\ndeins/\n";
        let out = upsert_block(original, "kicad", &ls(&["*.autosave", "fp-info-cache"]));
        let expected = "# Hand-Edit oben\nmeins/\n\n# >>> baustein: kicad >>>\n*.autosave\nfp-info-cache\n# <<< baustein: kicad <<<\n\n# Hand-Edit unten\ndeins/\n";
        assert_eq!(out, expected);
        // Die Hand-Edit-Zeilen überleben unverändert.
        assert!(out.contains("# Hand-Edit oben\nmeins/"));
        assert!(out.contains("# Hand-Edit unten\ndeins/"));
    }

    /// Ein vorhandener Block wird sauber **ersetzt**, nicht dupliziert.
    #[test]
    fn pre_existing_block_is_replaced_cleanly_not_duplicated() {
        let original =
            "# >>> baustein: zephyr >>>\nveraltet/\n# <<< baustein: zephyr <<<\n";
        let out = upsert_block(original, "zephyr", &ls(&["build/"]));
        assert_eq!(
            out,
            "# >>> baustein: zephyr >>>\nbuild/\n# <<< baustein: zephyr <<<\n"
        );
        // genau ein Start-Marker
        assert_eq!(out.matches("# >>> baustein: zephyr >>>").count(), 1);
        assert!(!out.contains("veraltet/"));
    }

    /// Blöcke mehrerer Bausteine koexistieren; ein Upsert fasst nur den **eigenen** Block an.
    #[test]
    fn multiple_bausteins_blocks_coexist_only_own_is_touched() {
        let start = upsert_block("", "kicad", &ls(&["*.autosave"]));
        let both = upsert_block(&start, "zephyr", &ls(&["build/"]));
        assert_eq!(both.matches("baustein: kicad").count(), 2); // start+end
        assert_eq!(both.matches("baustein: zephyr").count(), 2);

        // Nur den kicad-Block ändern; der zephyr-Block bleibt unverändert.
        let changed = upsert_block(&both, "kicad", &ls(&["*.autosave", "fp-info-cache"]));
        assert!(changed.contains("fp-info-cache"));
        // zephyr-Block unangetastet:
        assert!(changed.contains("# >>> baustein: zephyr >>>\nbuild/\n# <<< baustein: zephyr <<<"));
    }

    /// Mehrere (durch Handarbeit entstandene) Blöcke derselben id heilen zu genau einem.
    #[test]
    fn duplicate_blocks_of_same_id_collapse_to_one() {
        let original = "A/\n# >>> baustein: x >>>\nalt1/\n# <<< baustein: x <<<\nB/\n# >>> baustein: x >>>\nalt2/\n# <<< baustein: x <<<\nC/\n";
        let out = upsert_block(original, "x", &ls(&["neu/"]));
        // genau ein Block, an der Stelle des ERSTEN, Außen-Zeilen erhalten und in Reihenfolge.
        assert_eq!(out.matches("# >>> baustein: x >>>").count(), 1);
        assert_eq!(
            out,
            "A/\n# >>> baustein: x >>>\nneu/\n# <<< baustein: x <<<\nB/\nC/\n"
        );
    }

    /// Leere Zeilenmenge legt einen leeren Anker-Block an (nur die Marker).
    #[test]
    fn empty_lines_yield_empty_anchor_block() {
        let out = upsert_block("", "doku", &ls(&[]));
        assert_eq!(out, "# >>> baustein: doku >>>\n# <<< baustein: doku <<<\n");
    }

    /// Ein anderer Baustein-Block bleibt unberührt, wenn wir eine fremde id anhängen — und der
    /// neue Block landet am Ende, mit erhaltenem Bestandsinhalt davor.
    #[test]
    fn appends_after_existing_unrelated_content() {
        let original = "*.tmp\n# >>> baustein: other >>>\nx/\n# <<< baustein: other <<<\n";
        let out = upsert_block(original, "zephyr", &ls(&["build/"]));
        assert_eq!(
            out,
            "*.tmp\n# >>> baustein: other >>>\nx/\n# <<< baustein: other <<<\n# >>> baustein: zephyr >>>\nbuild/\n# <<< baustein: zephyr <<<\n"
        );
    }
}
