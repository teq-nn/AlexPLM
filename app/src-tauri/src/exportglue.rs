//! Der **Notausgang**: „Export als einfache Ordner" (Issue #134, E56).
//!
//! Das letzte Sicherheitsnetz. Selbst wenn der Server/Backend klemmt (Auth-Schleife, Netz weg,
//! ein Sync hängt), muss der Nutzer seine Daten *immer* herausbekommen — als gewöhnliche, blanke
//! Ordner auf der Platte, die **ohne** `_plm`/git-Voodoo lesbar sind: kein `.git`, kein Store,
//! keine Werkzeug-Magie nötig, um sie zu öffnen. Genau die Tool-Unabhängigkeit der Daten, die die
//! Seele des Konzepts ist (E3/E1), aber als kalter, jederzeit erreichbarer Hebel.
//!
//! **Rein lokal.** Diese Schicht spricht NIE mit dem Server: sie liest die markierten Stände aus
//! dem lokalen git (die `version/<label>`-Tags, E47/#8) und schreibt deren Inhalt mit git-Plumbing
//! (`read-tree` + `checkout-index`) als blanke Datei-Bäume heraus. Der Jetzt-Zustand wird aus dem
//! tatsächlichen Arbeitsbereich kopiert — also exakt das, was *jetzt* auf der Platte liegt, inklusive
//! noch nicht festgeschriebener Arbeit, denn im Notfall zählt der echte aktuelle Stand, nicht der
//! zuletzt committete. Keine dieser Operationen braucht ein Remote; alle gehen durch
//! [`crate::gitrunner`] (Issue #22), aber keine fasst das Netz an.
//!
//! Der `_plm`-Store (ADR 0002) und das `.git`-Verzeichnis bleiben **draußen**: der Export ist der
//! blanke Arbeits-Inhalt, nicht der innere Werkzeug-Kram. „Einfacher Ordner" heißt wörtlich einfach.
//!
//! Plattform-neutral: `std::path` + Vorwärts-Schrägstrich-Anzeige; keine Shell, kein `tar`, kein `cd`.

use crate::graphread::VERSION_NOTES;
use std::path::{Path, PathBuf};

/// Präfix der dauerhaften Versions-Tags (E47/#8). Gespiegelt aus [`crate::graphread`] (dort
/// `pub(crate)`), damit der Notausgang die markierten Stände **rein lokal** selbst auflösen kann,
/// ohne den Lese-/Projektions-Pfad (der mehr tut) zu durchlaufen. Bleibt absichtlich `version/`
/// — das ist On-Disk-Zustand in bestehenden Repos (siehe `graphread::TAG_PREFIX`).
const TAG_PREFIX: &str = "version/";

/// Name des `_plm`-Stores (ADR 0002). Wird vom Export **ausgelassen** — der Notausgang liefert den
/// blanken Arbeits-Inhalt, nicht den inneren Werkzeug-Kram. Präfix-Regel wie in der Projektion.
const PLM_PREFIX: &str = "_plm";

/// Default-Name des Ausgabe-Ordners *neben* dem Produkt, in den die einfachen Ordner geschrieben
/// werden. Ein sichtbarer, ehrlicher Ort daneben (E3); mit Punkt-Präfix, damit ein versehentlich
/// *innerhalb* des Produkts liegender Export nie als Baustein projiziert wird (die Projektion
/// überspringt `.`-Dotfiles). In der Praxis wählt der Nutzer das Ziel; dies ist nur der Fallback-Name.
const EXPORT_DIR: &str = ".plm-export";

/// Ein einzelner materialisierter Stand des Exports: sein Name (Jetzt-Zustand oder Versions-Marke)
/// und der blanke Ordner-Pfad (Vorwärts-Schrägstriche), den die UI dem OS zum Öffnen übergibt.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExportierterStand {
    /// Menschenlesbare Marke des Stands: `"Jetzt-Zustand"` oder das Versions-Etikett (z. B. `v1.0`).
    pub marke: String,
    /// Absoluter Pfad des blanken Ordners dieses Stands, in Vorwärts-Schrägstrich-Anzeige.
    pub pfad: String,
}

/// Das Ergebnis von „Export als einfache Ordner": der Wurzel-Ordner des Exports plus je ein
/// Eintrag pro herausgeschriebenem Stand (Jetzt-Zustand zuerst, dann die markierten Stände).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExportErgebnis {
    /// Absoluter Pfad des Wurzel-Ordners, in dem alle Stände als Unterordner liegen.
    pub wurzel: String,
    /// Die einzelnen Stände (Jetzt-Zustand + markierte Stände), in der Reihenfolge der Anlage.
    pub staende: Vec<ExportierterStand>,
}

/// **Export als einfache Ordner** (Issue #134, E56) — der Notausgang. Materialisiert den
/// Jetzt-Zustand und jeden markierten Stand (`version/<label>`-Tag) als blanke Ordner unter `ziel`.
///
/// Rein lokal: liest die Tags und Bäume aus dem lokalen git, schreibt mit Plumbing heraus, fasst
/// **nie** das Netz an — funktioniert also auch, wenn der Server klemmt. Idempotent gemeint: ein
/// schon vorhandener Stand-Ordner wird vor dem Schreiben geleert, damit ein zweiter Export denselben
/// blanken Inhalt liefert statt alte Reste zu mischen.
///
/// `ziel == None` ⇒ Fallback-Ort `<produkt>/.plm-export` (sichtbar, daneben, E3). Gibt die Wurzel
/// und je einen Eintrag pro Stand zurück, damit die UI den Ordner sofort öffnen kann.
pub fn export_einfache_ordner(
    root: &Path,
    ziel: Option<&Path>,
) -> std::io::Result<ExportErgebnis> {
    if !root.is_dir() {
        return Err(std::io::Error::other("Kein Produkt-Ordner"));
    }
    let wurzel = ziel.map(PathBuf::from).unwrap_or_else(|| root.join(EXPORT_DIR));
    std::fs::create_dir_all(&wurzel)?;

    let mut staende = Vec::new();

    // 1) Jetzt-Zustand: exakt das, was *jetzt* im Arbeitsbereich liegt (inkl. noch nicht
    //    festgeschriebener Arbeit) — im Notfall zählt der echte aktuelle Stand. Wir kopieren die
    //    erfassten Dateien aus dem Arbeitsbereich, lassen `_plm` und `.git` weg.
    let jetzt_marke = "Jetzt-Zustand";
    let jetzt_ziel = wurzel.join(ordner_name(jetzt_marke));
    leere_ordner(&jetzt_ziel)?;
    kopiere_jetzt_zustand(root, &jetzt_ziel)?;
    staende.push(ExportierterStand { marke: jetzt_marke.to_string(), pfad: display_path(&jetzt_ziel) });

    // 2) Jeder markierte Stand (Revision): den Baum des Tags blank herausschreiben — kein `.git`,
    //    kein `_plm`. Rein lokal über die Tag-Liste, keine Server-Runde.
    for label in lese_versions_marken(root)? {
        let stand_ziel = wurzel.join(ordner_name(&label));
        leere_ordner(&stand_ziel)?;
        materialisiere_tag(root, &label, &stand_ziel)?;
        staende.push(ExportierterStand { marke: label, pfad: display_path(&stand_ziel) });
    }

    Ok(ExportErgebnis { wurzel: display_path(&wurzel), staende })
}

/// Die markierten Stände eines Produkts rein lokal lesen: die `version/<label>`-Tags, auf ihr
/// nacktes Etikett (`<label>`) reduziert. Kein Server, keine Projektion. Eine git-freie/leere
/// Tag-Liste ist kein Fehler — dann gibt es eben nur den Jetzt-Zustand zu exportieren.
fn lese_versions_marken(root: &Path) -> std::io::Result<Vec<String>> {
    let out = crate::gitrunner::command(root)
        .args(["tag", "--list", &format!("{TAG_PREFIX}*")])
        .output()?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.trim_start_matches(TAG_PREFIX).to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Den Jetzt-Zustand kopieren: jede vom git **erfasste** Datei (`git ls-files`) aus dem
/// Arbeitsbereich in `ziel`, mit erhaltener Ordner-Struktur — aber den tatsächlichen Platten-Inhalt,
/// damit auch noch nicht festgeschriebene Arbeit im Notfall mitgeht. `_plm` bleibt draußen. Fehlt
/// git ganz (kein Repo), wird das als leere Liste behandelt: der Notausgang scheitert nicht daran.
fn kopiere_jetzt_zustand(root: &Path, ziel: &Path) -> std::io::Result<()> {
    let out = crate::gitrunner::command(root).args(["ls-files"]).output()?;
    if !out.status.success() {
        return Ok(()); // kein git / kein Arbeitsbereich — kein harter Fehler im Notausgang
    }
    for rel in String::from_utf8_lossy(&out.stdout).lines().map(str::trim) {
        if rel.is_empty() || ist_plm_pfad(rel) {
            continue;
        }
        let quelle = root.join(rel);
        // Eine erfasste Datei kann auf der Platte fehlen (gelöscht, aber noch im Index). Im Notfall
        // überspringen wir sie still — wir exportieren, was da ist, statt am Fehlen zu scheitern.
        if !quelle.is_file() {
            continue;
        }
        let zieldatei = ziel.join(rel);
        if let Some(parent) = zieldatei.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&quelle, &zieldatei)?;
    }
    Ok(())
}

/// Den Baum eines `version/<label>`-Tags als blanken Datei-Baum nach `ziel` schreiben — git-Plumbing
/// statt `tar` oder Shell-Pipe: ein temporärer Index (`read-tree`) plus `checkout-index -a -f` in
/// einen vom Repo getrennten Arbeitsbaum (`GIT_WORK_TREE`). Das schreibt **nur** Dateien, kein
/// `.git`, und fasst nie das Netz an. Den `_plm`-Store räumen wir danach wieder heraus.
fn materialisiere_tag(root: &Path, label: &str, ziel: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(ziel)?;
    let tag = format!("{TAG_PREFIX}{label}");

    // Ein temporärer Index NEBEN dem `.git`, damit der echte Index des Produkts unberührt bleibt.
    let index = root.join(".git").join(temp_index_name(label));

    // `read-tree <tag>`: den Baum des Tags in den temporären Index laden …
    git_plumbing_ok(root, &index, ziel, &["read-tree", &tag])?;
    // … und `checkout-index -a -f`: alle Einträge des Index als blanke Dateien in den Arbeitsbaum
    // schreiben. `-a` = alle, `-f` = vorhandene überschreiben (der Ordner ist ohnehin frisch geleert).
    let res = git_plumbing_ok(root, &index, ziel, &["checkout-index", "-a", "-f"]);

    // Der temporäre Index ist Wegwerf-Zustand — immer aufräumen, auch wenn der checkout scheiterte.
    let _ = std::fs::remove_file(&index);
    res?;

    // Den `_plm`-Store aus dem blanken Export wieder entfernen: er gehört nicht zum Arbeits-Inhalt.
    let plm = ziel.join(PLM_PREFIX);
    if plm.is_dir() {
        std::fs::remove_dir_all(&plm)?;
    }
    Ok(())
}

/// Einen git-Plumbing-Aufruf mit getrenntem Index + Arbeitsbaum ausführen. `GIT_INDEX_FILE` und
/// `GIT_WORK_TREE` lenken `read-tree`/`checkout-index` auf den temporären Index und den Export-Ordner,
/// ohne den echten Index/Arbeitsbereich des Produkts anzufassen. Geht durch [`crate::gitrunner`]
/// (Hardening), bleibt aber rein lokal (kein Netz).
fn git_plumbing_ok(
    root: &Path,
    index: &Path,
    work_tree: &Path,
    args: &[&str],
) -> std::io::Result<()> {
    let out = crate::gitrunner::command(root)
        .env("GIT_INDEX_FILE", index)
        .env("GIT_WORK_TREE", work_tree)
        .args(args)
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// Einen Ordner für eine frische Materialisierung leeren (anlegen, falls er fehlt): so liefert ein
/// zweiter Export denselben blanken Inhalt statt alte Reste mit dem neuen Stand zu mischen.
fn leere_ordner(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    Ok(())
}

/// Ob ein produkt-relativer Pfad in den `_plm`-Store fällt (erste Komponente trägt das Präfix).
/// Pure → tabellen-testbar. Vorwärts-Schrägstriche, wie `git ls-files` sie liefert.
fn ist_plm_pfad(rel: &str) -> bool {
    rel.split('/').next().is_some_and(|head| head.starts_with(PLM_PREFIX))
        || rel == VERSION_NOTES // die Werkzeug-Notizdatei gehört ebenfalls nicht in den blanken Export
}

/// Dateisystem-sicherer Ordnername für eine Marke: auf `[A-Za-z0-9._-]` reduziert, führende/
/// abschließende `-` getrimmt. Pure → tabellen-testbar. Leere/nur-unsichere Marke ⇒ `stand`.
fn ordner_name(marke: &str) -> String {
    let safe: String = marke
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect();
    let safe = safe.trim_matches('-');
    if safe.is_empty() { "stand".to_string() } else { safe.to_string() }
}

/// Eindeutiger Name für den temporären Index eines Tags, damit parallele Exports verschiedener
/// Stände sich nicht in die Quere kommen. Pure über die Marke.
fn temp_index_name(label: &str) -> String {
    format!("plm-export-{}.idx", ordner_name(label))
}

/// Anzeigepfad in Vorwärts-Schrägstrichen (plattform-neutral, auch auf Windows). Pure. Die
/// Wurzel-Komponente (Unix „/", Windows „C:\") wird beibehalten und ein doppeltes „//" vermieden.
fn display_path(p: &Path) -> String {
    use std::path::Component;
    let mut out = String::new();
    for comp in p.components() {
        let part = comp.as_os_str().to_string_lossy();
        match comp {
            Component::RootDir => out.push('/'),
            _ => {
                if !out.is_empty() && !out.ends_with('/') {
                    out.push('/');
                }
                out.push_str(&part);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordner_name_is_filesystem_safe() {
        // table: Marke -> erwarteter sicherer Ordnername
        let cases: &[(&str, &str)] = &[
            ("v1.0", "v1.0"),
            ("Jetzt-Zustand", "Jetzt-Zustand"),
            ("gehaeuse/v2 alpha", "gehaeuse-v2-alpha"),
            ("**laut**", "laut"),
            ("", "stand"),
            ("///", "stand"),
        ];
        for (marke, expected) in cases {
            assert_eq!(ordner_name(marke), *expected, "marke={marke:?}");
        }
    }

    #[test]
    fn ist_plm_pfad_recognises_the_tool_store() {
        // table: produkt-relativer Pfad -> gehört er zum blanken Export NICHT (true = auslassen)?
        let cases: &[(&str, bool)] = &[
            ("_plm/stack.json", true),
            ("_plm", true),
            ("_plm-archive/x", true), // Präfix-Regel, defensiv
            (VERSION_NOTES, true),    // Werkzeug-Notizdatei
            ("elektronik/board.kicad_pcb", false),
            ("mechanik/gehaeuse/gehaeuse.f3d", false),
            ("plm/echt", false), // kein führender Unterstrich -> echter Inhalt
        ];
        for (rel, expected) in cases {
            assert_eq!(ist_plm_pfad(rel), *expected, "rel={rel:?}");
        }
    }

    #[test]
    fn display_path_uses_forward_slashes() {
        let p = Path::new("/produkt/.plm-export/v1.0");
        assert_eq!(display_path(p), "/produkt/.plm-export/v1.0");
    }

    #[test]
    fn temp_index_name_is_unique_per_label() {
        assert_ne!(temp_index_name("v1.0"), temp_index_name("v2.0"));
        // dateisystem-sicher (über ordner_name)
        assert_eq!(temp_index_name("a/b"), "plm-export-a-b.idx");
    }
}
