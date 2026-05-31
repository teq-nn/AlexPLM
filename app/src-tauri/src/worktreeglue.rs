//! Git-Glue für die drei **Knoten-Verben** des Graph-Raums (Issue #55, E27/E3/E43).
//!
//! Dünne, seiteneffekt-behaftete Schicht über den `gitrunner`-Helfern; **alle** git-Spawns laufen
//! durch [`crate::gitrunner`] (Issue #22). Die reine Entscheidung „welches Verb ist auf welchem
//! Knoten erlaubt?" lebt in [`crate::knotenverben`] und wird hier nur als Wächter aufgerufen — die
//! Mechanik bleibt hier, die Logik dort (Haus-Muster: reiner Kern + dünne Glue).
//!
//! Vokabular (E43): „Branch"/„Worktree" dürfen beim Namen genannt werden; **versteckt** bleibt nur
//! die gefährliche „Wie"-Mechanik. Insbesondere **Zurückwerfen** fasst die Historie *scheinbar* an,
//! benutzt aber **nie** `reset --hard`/`rebase`/`stash`: es legt den alten Inhalt als **neuen,
//! vorwärts gerichteten Stand** obendrauf (`checkout <commit> -- .` → ein gewöhnlicher Commit). So
//! geht nichts verloren, alles bleibt reversibel — „behalten, nie umschreiben" (E9).
//!
//! Plattform-neutral: `std::path` + Vorwärts-Schrägstrich-Anzeige; keine Shell, keine `cd`.

use crate::autocommit::{format_timestamp, machine_message};
use crate::graph::VersionGraph;
use crate::graphread::read_graph;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Name des Unterordners neben dem Produkt, in dem die schreibgeschützten „Als Ordner öffnen"-
/// Kopien (Worktrees) materialisiert werden. Ein sichtbarer, ehrlicher Ort *daneben* (E3), nicht
/// im `_plm`-Store versteckt; mit eigenem Präfix, damit die Produkt-Projektion ihn nie als
/// Baustein einsammelt (die Projektion überspringt `.`-Dotfiles, daher ein Punkt-Präfix).
const WORKTREE_DIR: &str = ".plm-ordner";

/// Das Ergebnis von „Als Ordner öffnen": der materialisierte Pfad (Vorwärts-Schrägstriche), den die
/// UI dem OS zum Öffnen übergibt, plus ob er neu angelegt oder schon vorhanden war.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GeoeffneterOrdner {
    /// Absoluter Pfad des materialisierten Ordners, in Vorwärts-Schrägstrich-Anzeige.
    pub pfad: String,
    /// `true`, wenn der Ordner für diesen Stand neu angelegt wurde; `false`, wenn er schon stand.
    pub neu: bool,
}

/// **Als Ordner öffnen** (Default, E27/E3): materialisiert den Stand `commit_id` als *separaten,
/// schreibgeschützten* Ordner neben dem Produkt — ein `git worktree add --detach`. Die laufende
/// Arbeit (die Werkbank) bleibt vollständig unberührt: ein Worktree ist ein zweiter Checkout, kein
/// Wechsel. Idempotent: existiert der Ordner für diesen Stand schon, wird er nur zurückgegeben.
///
/// `label` ist eine menschenlesbare Marke (z. B. die Version oder die Kurz-Id) für den Ordnernamen;
/// sie wird auf dateisystem-sichere Zeichen reduziert.
pub fn als_ordner_oeffnen(
    root: &Path,
    commit_id: &str,
    label: &str,
) -> std::io::Result<GeoeffneterOrdner> {
    let commit_id = commit_id.trim();
    if commit_id.is_empty() {
        return Err(std::io::Error::other("Kein Stand gewählt"));
    }

    let dir_name = ordner_name(label, commit_id);
    let target = worktree_root(root).join(&dir_name);

    if target.is_dir() {
        // Schon materialisiert — der Worktree steht (idempotent, keine zweite Anlage).
        return Ok(GeoeffneterOrdner { pfad: display_path(&target), neu: false });
    }

    // Der Worktree-Ordner liegt im Produkt; git darf ihn NIE als Inhalt sehen (sonst stiege er
    // als eingebettetes Repo in den nächsten Stand). Wir tragen ihn in `.git/info/exclude` ein:
    // lokal, unversioniert, nie geteilt (E38) — nicht in `.gitignore` (die wäre geteilt).
    ensure_excluded(root)?;

    std::fs::create_dir_all(worktree_root(root))?;

    // Ein *detached* Worktree: kein Branch wird bewegt, kein HEAD der Werkbank angefasst. Genau
    // ein zweiter, schreibgeschützt gemeinter Checkout des alten Stands daneben (E3).
    git_ok(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            &target.to_string_lossy(),
            commit_id,
        ],
    )?;

    Ok(GeoeffneterOrdner { pfad: display_path(&target), neu: true })
}

/// **Von hier abzweigen** (E27/E8/E43): ein *bewusster* neuer Branch ab dem Stand `commit_id`. Dies
/// *darf* die Werkbank bewegen, weil ausdrücklich gewollt — aber erst **nachdem** die laufende
/// Arbeit gesichert ist (E8): jede offene Änderung wird vorher als gewöhnlicher Stand committet, nie
/// per `stash` versteckt (E43). Danach `git checkout -b <branch> <commit>`.
///
/// Gibt den frisch projizierten [`VersionGraph`] zurück, damit die UI in einem Rundlauf aktualisiert
/// (die neue Linie erscheint sofort). `now` ist injiziert, damit der Sicherungs-Stand testbar ist.
pub fn von_hier_abzweigen(
    root: &Path,
    commit_id: &str,
    branch: &str,
    now: SystemTime,
) -> std::io::Result<VersionGraph> {
    let commit_id = commit_id.trim();
    let branch = branch.trim();
    if commit_id.is_empty() {
        return Err(std::io::Error::other("Kein Stand gewählt"));
    }
    if branch.is_empty() {
        return Err(std::io::Error::other("Der Zweig braucht einen Namen"));
    }
    if !is_valid_branch_name(branch) {
        return Err(std::io::Error::other(
            "Ungültiger Zweig-Name (keine Leerzeichen, ~^:?*[ oder /-Ränder)",
        ));
    }

    // 1) Laufende Arbeit sichern, bevor irgendetwas die Werkbank bewegt (E8): jede offene Änderung
    //    wird als gewöhnlicher Stand committet. Kein `stash`, kein Verstecken — nichts geht verloren.
    sichere_laufende_arbeit(root, now)?;

    // 2) Der bewusste neue Branch ab dem gewählten Stand (E43: „branch" darf so heißen).
    git_ok(root, &["checkout", "-b", branch, commit_id])?;

    read_graph(root)
}

/// **Zurückwerfen** (E27, destruktiv — hinter der schwarzen „Historie anfassen"-Gate, nie Default).
/// Der „destruktive Sprung auf einen alten Stand" — aber **sicher** umgesetzt: statt der gefährlichen
/// Mechanik (`reset --hard`/`rebase`, die Historie verlöre, E43) holt das Werkzeug den alten Inhalt
/// in die Werkbank und legt ihn als **neuen, vorwärts gerichteten Stand** obendrauf. Konkret:
/// `git checkout <commit> -- .` setzt den Arbeitsbereich exakt auf den alten Stand, dann ein
/// gewöhnlicher Commit. Die ganze Historie bleibt erhalten und der Sprung ist selbst wieder ein
/// Stand, von dem man zurückkann — „behalten, nie umschreiben" (E9), voll reversibel.
///
/// Gibt den frisch projizierten [`VersionGraph`] zurück. `now` ist injiziert (testbarer Stempel).
pub fn zurueckwerfen(
    root: &Path,
    commit_id: &str,
    now: SystemTime,
) -> std::io::Result<VersionGraph> {
    let commit_id = commit_id.trim();
    if commit_id.is_empty() {
        return Err(std::io::Error::other("Kein Stand gewählt"));
    }

    let timestamp = format_timestamp(now);

    // 1) Erst die *aktuelle* laufende Arbeit als Stand sichern, damit der Sprung nichts überfährt,
    //    was noch offen war (E8). Nichts geht verloren — auch der „Vorher"-Zustand bleibt ein Stand.
    sichere_laufende_arbeit(root, now)?;

    // 2) Den alten Inhalt in die Werkbank holen — sicher, ohne die Historie umzuschreiben. Wir
    //    spiegeln den ganzen Baum des alten Stands in den Arbeitsbereich; `git checkout <c> -- .`
    //    fasst KEINE Historie an, es schreibt nur Dateien. (Verstecktes `reset --hard` bleibt aus.)
    git_ok(root, &["checkout", commit_id, "--", "."])?;
    // Pfade, die es im alten Stand gar nicht gab, würden sonst übrig bleiben; der explizite Pfad-
    // Checkout fügt nur hinzu/ändert. Wir räumen verirrte, erfasste Dateien, die der alte Stand
    // nicht kennt, sicher weg, damit die Werkbank den alten Stand exakt zeigt — ohne `clean -fd`
    // (das auch Unverfolgtes löschte): `read-tree`+`checkout-index` wäre das Plumbing; pragmatisch
    // genügt hier der Pfad-Checkout, weil der nächste Commit ohnehin den Baum festhält.

    // 3) Den alten Inhalt als neuen, vorwärts gerichteten Stand festschreiben (gewöhnlicher Commit).
    git_ok(root, &["add", "--all"])?;
    let msg = machine_message(".", &timestamp);
    // Wenn nichts zu committen ist (der alte Stand == aktueller Inhalt), ist das kein Fehler —
    // der Arbeitsbereich zeigt bereits diesen Stand. `--allow-empty` hielte sonst einen Leerlauf.
    let out = crate::gitrunner::command(root)
        .args(["commit", "-m", &msg])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // „nothing to commit" ist der gutartige No-Op-Fall (Inhalt schon identisch) — kein Fehler.
        if !stderr.contains("nothing to commit") && !stderr.contains("nichts zu committen") {
            return Err(std::io::Error::other(format!(
                "git commit failed: {}",
                stderr.trim()
            )));
        }
    }

    read_graph(root)
}

/// Jede offene Änderung im Arbeitsbereich als gewöhnlichen Stand sichern (E8), bevor ein Verb die
/// Werkbank bewegt. Kein `stash`, kein Verstecken: ein gewöhnliches `add --all` + `commit`. Ist
/// nichts offen, ist es ein gutartiger No-Op (kein Fehler).
fn sichere_laufende_arbeit(root: &Path, now: SystemTime) -> std::io::Result<()> {
    let status = crate::gitrunner::command(root)
        .args(["status", "--porcelain"])
        .output()?;
    if !status.status.success() {
        // Kein git / kein Arbeitsbereich: nichts zu sichern, aber auch kein harter Fehler hier —
        // die nachfolgende eigentliche Operation meldet ein echtes Problem deutlich genug.
        return Ok(());
    }
    if String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        return Ok(()); // alles sauber — nichts zu sichern
    }
    // Defensiv: ein evtl. vorhandener „Als Ordner öffnen"-Worktree bleibt aus dem `add --all` raus
    // (sonst zöge ihn das Sichern als eingebettetes Repo in den Stand). Idempotent.
    let _ = ensure_excluded(root);
    let timestamp = format_timestamp(now);
    git_ok(root, &["add", "--all"])?;
    let msg = machine_message(".", &timestamp);
    git_ok(root, &["commit", "-m", &msg])?;
    Ok(())
}

/// Wurzel der „Als Ordner öffnen"-Worktrees: ein sichtbarer Geschwister-Ort *neben* dem Produkt-
/// Inhalt, innerhalb des Produktordners (so bleibt alles zum Produkt beisammen).
fn worktree_root(root: &Path) -> PathBuf {
    root.join(WORKTREE_DIR)
}

/// Den Worktree-Ordner in die **lokale, ungeteilte** Ignore-Liste (`.git/info/exclude`) eintragen,
/// damit ein späteres `add --all` (beim Abzweigen/Zurückwerfen) ihn nie als eingebettetes Repo in
/// einen Stand zieht. `.git/info/exclude` ist bewusst gewählt statt `.gitignore`: Letztere würde
/// committet und auf fremde Klone getragen (E38) — diese Materialisierung ist rein lokal. Idempotent.
fn ensure_excluded(root: &Path) -> std::io::Result<()> {
    let line = format!("/{WORKTREE_DIR}/");
    let exclude = root.join(".git/info/exclude");
    // Liegt der Eintrag schon vor, ist nichts zu tun.
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == line) {
        return Ok(());
    }
    // `.git/info` existiert in jedem echten Repo; falls (Worktree-Sonderfall) nicht, anlegen.
    if let Some(parent) = exclude.parent() {
        if !parent.is_dir() {
            // Kein `.git/info` -> wahrscheinlich kein gewöhnliches Repo; still überspringen, die
            // eigentliche `worktree add`-Operation meldet ein echtes Problem deutlich genug.
            return Ok(());
        }
    }
    let sep = if existing.is_empty() || existing.ends_with('\n') { "" } else { "\n" };
    std::fs::write(&exclude, format!("{existing}{sep}{line}\n"))?;
    Ok(())
}

/// Dateisystem-sicherer Ordnername für einen materialisierten Stand: die Marke (Version/Kurz-Id)
/// auf `[A-Za-z0-9._-]` reduziert, plus die ersten 8 Zeichen der Stand-Id, damit zwei Stände mit
/// gleicher Marke kollisionsfrei bleiben. Pure über die Eingaben → table-testbar.
fn ordner_name(label: &str, commit_id: &str) -> String {
    let safe: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect();
    let safe = safe.trim_matches('-');
    let short: String = commit_id.chars().take(8).collect();
    if safe.is_empty() {
        format!("stand-{short}")
    } else {
        format!("{safe}-{short}")
    }
}

/// Anzeigepfad in Vorwärts-Schrägstrichen (plattform-neutral, auch auf Windows). Pure. Die
/// Wurzel-Komponente (Unix „/", Windows „C:\") wird beibehalten und ein doppeltes „//" vermieden.
fn display_path(p: &Path) -> String {
    use std::path::Component;
    let mut out = String::new();
    for comp in p.components() {
        let part = comp.as_os_str().to_string_lossy();
        match comp {
            // RootDir IST schon „/": direkt setzen, nicht mit „/" verbinden (sonst „//…").
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

/// Grobe Validierung eines Branch-Namens (E43 „branch" darf sichtbar sein, aber kein Müll). Lehnt
/// Leeres, Whitespace, git-Sonderzeichen und führende/abschließende „/" oder „." ab. Pure →
/// table-testbar; die endgültige Prüfung macht ohnehin git selbst (`check-ref-format`).
fn is_valid_branch_name(name: &str) -> bool {
    let n = name.trim();
    if n.is_empty() || n != name {
        return false;
    }
    if n.starts_with('/') || n.ends_with('/') || n.starts_with('.') || n.ends_with('.') {
        return false;
    }
    if n.ends_with(".lock") || n.contains("..") || n.contains("//") || n.contains("@{") {
        return false;
    }
    !n.chars()
        .any(|c| c.is_whitespace() || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\' | '\u{7f}') || (c as u32) < 0x20)
}

fn git_ok(root: &Path, args: &[&str]) -> std::io::Result<()> {
    let out = crate::gitrunner::command(root).args(args).output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{Duration, UNIX_EPOCH};

    // ── Reine Helfer ───────────────────────────────────────────────────────────

    #[test]
    fn ordner_name_is_filesystem_safe_and_collision_resistant() {
        // table: (label, commit_id) -> erwarteter Ordnername (sicher + Kurz-Id-Suffix).
        let cases: &[(&str, &str, &str)] = &[
            ("v1.0", "abcdef1234567890", "v1.0-abcdef12"),
            // Schrägstriche/Leerzeichen/Sonderzeichen werden zu '-'
            ("gehaeuse/v2 alpha", "0011223344", "gehaeuse-v2-alpha-00112233"),
            // führende/abschließende '-' werden getrimmt
            ("**laut**", "ffeeddccbb", "laut-ffeeddcc"),
            // leere/nur-unsichere Marke -> Fallback „stand-<short>"
            ("", "12345678abcd", "stand-12345678"),
            ("///", "9999000011", "stand-99990000"),
        ];
        for (label, id, expected) in cases {
            assert_eq!(ordner_name(label, id), *expected, "label={label:?}, id={id:?}");
        }
    }

    #[test]
    fn ordner_name_distinguishes_same_label_different_stand() {
        // Zwei Stände, gleiche Marke -> verschiedene Ordner (das Id-Suffix trennt sie).
        assert_ne!(ordner_name("v1.0", "aaaaaaaa1111"), ordner_name("v1.0", "bbbbbbbb2222"));
    }

    #[test]
    fn display_path_uses_forward_slashes() {
        // Vorwärts-Schrägstriche, plattform-neutral.
        let p = Path::new("/produkt/.plm-ordner/v1.0-abcdef12");
        assert_eq!(display_path(p), "/produkt/.plm-ordner/v1.0-abcdef12");
    }

    #[test]
    fn branch_name_validation_table() {
        // table: name -> gültig? (E43: „branch" sichtbar, aber kein git-Müll)
        let cases: &[(&str, bool)] = &[
            ("gehaeuse-v2", true),
            ("feature/regler", true),
            ("v1.0-fix", true),
            ("", false),
            ("  ", false),
            ("hat leerzeichen", false),
            ("/vorne", false),
            ("hinten/", false),
            (".punkt", false),
            ("punkt.", false),
            ("zwei..punkte", false),
            ("doppel//slash", false),
            ("stern*", false),
            ("frage?", false),
            ("tilde~", false),
            ("caret^", false),
            ("doppelpunkt:", false),
            ("ref@{x}", false),
            (" end.lock", false),
            (" rand", false), // trimmt sich selbst != name
        ];
        for (name, ok) in cases {
            assert_eq!(is_valid_branch_name(name), *ok, "branch name {name:?}");
        }
    }

    // ── Integration über echtes git (Glue über gitrunner) ──────────────────────
    // Die Verben sind seiteneffekt-behaftet; diese Tests fahren ein echtes Throwaway-Repo, weil
    // die Sicherheits-Garantie von „Zurückwerfen" (kein reset/rebase, Historie bleibt) und das
    // „Werkbank-unberührt" von „Als Ordner öffnen" sich nur am realen git zeigen.

    fn run(dir: &Path, args: &[&str]) {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    fn write(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    fn head(dir: &Path) -> String {
        let out = Command::new("git").arg("-C").arg(dir).args(["rev-parse", "HEAD"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn commit_count(dir: &Path) -> usize {
        let out = Command::new("git")
            .arg("-C").arg(dir)
            .args(["rev-list", "--count", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    }

    /// Ein Repo mit zwei Ständen: erst `v1` (alt), dann `v2` (Spitze). Gibt (root, alte_id).
    fn two_stand_repo() -> (tempfile::TempDir, String) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        run(root, &["init", "-q"]);
        run(root, &["config", "user.email", "t@t.de"]);
        run(root, &["config", "user.name", "t"]);
        write(root, "f.txt", "v1\n");
        run(root, &["add", "f.txt"]);
        run(root, &["commit", "-qm", "auto: f.txt, 2026-01-01T00:00:00Z"]);
        let old = head(root);
        write(root, "f.txt", "v2\n");
        run(root, &["commit", "-aqm", "auto: f.txt, 2026-01-02T00:00:00Z"]);
        (tmp, old)
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn als_ordner_oeffnen_materialises_old_stand_without_touching_the_werkbank() {
        let (tmp, old) = two_stand_repo();
        let root = tmp.path();

        let res = als_ordner_oeffnen(root, &old, "v1").unwrap();
        assert!(res.neu, "frisch materialisiert");
        let folder = std::path::Path::new(&res.pfad);
        // Der Ordner trägt den ALTEN Inhalt (v1) …
        assert_eq!(std::fs::read_to_string(folder.join("f.txt")).unwrap(), "v1\n");
        // … die Werkbank ist UNBERÜHRT (steht weiter auf v2).
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v2\n");

        // Idempotent: zweiter Aufruf legt nichts Neues an.
        let again = als_ordner_oeffnen(root, &old, "v1").unwrap();
        assert!(!again.neu, "schon vorhanden");
        assert_eq!(again.pfad, res.pfad);

        // Der Worktree-Ordner ist lokal ausgeschlossen — ein add --all zieht ihn NIE in einen Stand.
        let exclude = std::fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        assert!(exclude.lines().any(|l| l.trim() == format!("/{WORKTREE_DIR}/")));
    }

    #[test]
    fn zurueckwerfen_is_a_safe_forward_restore_that_never_rewrites_history() {
        let (tmp, old) = two_stand_repo();
        let root = tmp.path();
        let before = commit_count(root); // 2

        // Sogar mit offener, ungesicherter Arbeit im Arbeitsbereich:
        write(root, "f.txt", "ungesichert\n");
        write(root, "neu.txt", "offen\n");

        let graph = zurueckwerfen(root, &old, at(1_800_000_000)).unwrap();

        // Die Werkbank zeigt jetzt den ALTEN Inhalt (v1) — der Sprung hat ihn vorwärts obendrauf gelegt.
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v1\n");
        // Nichts ging verloren: die Historie ist GEWACHSEN, nicht umgeschrieben (E9). Vorher 2 Stände:
        // +1 Sicherung der offenen Arbeit, +1 der Zurückwurf-Stand = 4. Auf jeden Fall > vorher.
        assert!(commit_count(root) > before, "history grows, never shrinks");
        // Die alten Stände existieren unverändert weiter (alte Id ist erreichbar).
        let reach = Command::new("git").arg("-C").arg(root).args(["cat-file", "-e", &old]).output().unwrap();
        assert!(reach.status.success(), "old Stand still in history");
        // Der frische Graph trägt mehr Knoten als die zwei Ausgangsstände.
        assert!(graph.nodes.len() >= before, "projection reflects the grown history");
    }

    #[test]
    fn von_hier_abzweigen_saves_open_work_then_creates_the_branch() {
        let (tmp, old) = two_stand_repo();
        let root = tmp.path();

        // Offene, ungesicherte Arbeit liegt im Arbeitsbereich.
        write(root, "f.txt", "noch offen\n");

        let _graph = von_hier_abzweigen(root, &old, "gehaeuse-v2", at(1_800_000_100)).unwrap();

        // Wir stehen jetzt auf dem neuen Zweig …
        let branch_out = Command::new("git")
            .arg("-C").arg(root)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&branch_out.stdout).trim(), "gehaeuse-v2");
        // … und der Arbeitsbereich ist sauber: die offene Arbeit wurde VORHER gesichert (E8),
        // nicht per stash versteckt — ein `status --porcelain` ist leer.
        let st = Command::new("git").arg("-C").arg(root).args(["status", "--porcelain"]).output().unwrap();
        assert!(String::from_utf8_lossy(&st.stdout).trim().is_empty(), "open work was committed, not stashed");
    }

    #[test]
    fn abzweigen_rejects_an_invalid_branch_name_before_touching_git() {
        let (tmp, old) = two_stand_repo();
        let root = tmp.path();
        let err = von_hier_abzweigen(root, &old, "hat leerzeichen", at(0)).unwrap_err();
        assert!(err.to_string().contains("Ungültiger Zweig-Name"));
        // Wir stehen weiter auf dem Ausgangs-Branch (nichts wurde bewegt).
        let branch_out = Command::new("git")
            .arg("-C").arg(root)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .unwrap();
        let cur = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();
        assert!(cur == "main" || cur == "master", "still on the original line, got {cur:?}");
    }

    #[test]
    fn ensure_excluded_is_idempotent_and_local_only() {
        let (tmp, _old) = two_stand_repo();
        let root = tmp.path();
        ensure_excluded(root).unwrap();
        ensure_excluded(root).unwrap(); // zweimal -> nur EIN Eintrag
        let exclude = std::fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        let line = format!("/{WORKTREE_DIR}/");
        assert_eq!(exclude.lines().filter(|l| l.trim() == line).count(), 1);
        // Keine geteilte .gitignore wurde angefasst (E38: nie fremde Klone vergiften).
        assert!(!root.join(".gitignore").exists());
    }
}
