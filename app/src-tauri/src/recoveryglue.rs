//! Die **Recovery-Transaktion** als git-Glue (Issue #133, E56/E56a).
//!
//! Dünne, seiteneffekt-behaftete Schicht über den `gitrunner`-Helfern; **alle** git-Spawns laufen
//! durch [`crate::gitrunner`] (Issue #22). Die reine Entscheidung „festschreiben oder zurückdrehen?"
//! lebt im Kern [`crate::recovery`] und wird hier nur aufgerufen — die Mechanik bleibt hier, die
//! Logik dort (Haus-Muster: reiner Kern + dünne Glue, wie `pushglue`/`syncglue`/`worktreeglue`).
//!
//! Die tragende Idee von E56: Jede **gefährliche** Operation (Abzweigen, Zurückwerfen, Merge, …)
//! läuft als **Transaktion**. Vorher legt das Werkzeug einen **Schnappschuss** als git-Ref an —
//! ein leichtes Netz, das den exakten Stand (HEAD + Arbeitsbereich) festhält. Läuft die Operation
//! sauber durch, wird das Netz still eingerollt. Scheitert sie — oder fällt mittendrin der Strom —,
//! **dreht das Werkzeug automatisch zurück** auf genau diesen Schnappschuss: das Repo landet wieder
//! auf dem Stand von vorher, es geht nichts kaputt. Und der Nutzer liest die **ehrliche Domänen-
//! Meldung** „ist schiefgegangen, ich hab's zurückgedreht" statt roher git-Texte ([`crate::recovery`]).
//!
//! **Stromausfall** ist gedeckt, ohne dass dieser Lauf je zu Ende kommen muss: Der Schnappschuss
//! ist ein **persistenter** git-Ref unter [`SNAPSHOT_REF`]. Stirbt der Prozess mitten in der
//! Operation, bleibt dieser Ref liegen; der nächste Start räumt ihn per [`aufraeumen_offene`] auf
//! und dreht zurück — derselbe Effekt wie ein synchroner Fehler, nur zeitversetzt.
//!
//! Plattform-neutral: nur git über `gitrunner`, keine Shell, kein `cd`.

use crate::recovery::{entscheide, RueckfallSnapshot, ZURUECKGEDREHT};
use std::path::Path;

/// Der git-Ref, unter dem der Schnappschuss einer laufenden Transaktion liegt. Ein eigener
/// `refs/`-Namensraum, **nie** ein Branch oder Tag (taucht so in keiner Projektion/Linie auf, E43)
/// und **nie** gepusht (rein lokal, wie `.git/info/exclude`). Liegt er beim Start noch herum, war
/// eine Transaktion nicht zu Ende gekommen (Absturz/Stromausfall) → der nächste Start dreht zurück.
pub const SNAPSHOT_REF: &str = "refs/plm/rueckfall";

/// Fahre eine **gefährliche Operation als Transaktion** (E56): erst einen Schnappschuss des exakten
/// Stands legen (HEAD + Arbeitsbereich), dann `op` laufen lassen, und das reine Rückfallnetz-Kern
/// ([`crate::recovery::entscheide`]) entscheiden lassen, ob **festgeschrieben** oder
/// **zurückgedreht** wird:
///
/// - lief `op` sauber durch → den Schnappschuss still einrollen, das Ergebnis von `op` durchreichen;
/// - ergab `op` einen Fehler → **automatisch zurückdrehen** auf den Schnappschuss (HEAD +
///   Arbeitsbereich landen exakt auf dem Stand von vorher) und die **ehrliche Domänen-Meldung**
///   [`ZURUECKGEDREHT`] zurückgeben — nie der rohe git-Text (der wandert nur ins Diagnose-Log).
///
/// `op` bekommt den `root` und gibt sein eigenes Ergebnis zurück; der Wrapper ist generisch über
/// diesen Erfolgstyp, sodass jede gefährliche Operation (Abzweigen/Zurückwerfen/Merge/…) ihn
/// unverändert tragen kann — „mostly glue" (E56). Schon eine liegengebliebene Transaktion wird vor
/// dem Start aufgeräumt, damit jeder Lauf mit einem sauberen Netz beginnt.
pub fn mit_rueckfallnetz<T, F>(root: &Path, op: F) -> std::io::Result<T>
where
    F: FnOnce(&Path) -> std::io::Result<T>,
{
    // Liegt vom letzten (abgestürzten) Lauf noch ein Netz, erst zurückdrehen — wir starten nie auf
    // einer halben Transaktion. Best-effort: ohne liegengebliebenes Netz ist das ein No-Op.
    let _ = aufraeumen_offene(root);

    // Vor der gefährlichen Operation: den exakten Stand als Schnappschuss-Ref festhalten. Schlägt
    // schon das fehl (kein Repo o. Ä.), gibt es nichts abzusichern — wir lassen `op` selbst den
    // echten Fehler deutlich melden, ohne ein halbes Netz zu hinterlassen.
    if schnappschuss_legen(root).is_err() {
        return op(root);
    }

    // Die gefährliche Operation fahren. Ihr Ausgang ist die einzige Tatsache, die der reine Kern
    // beurteilt.
    let ergebnis = op(root);
    let snap = RueckfallSnapshot { fehlgeschlagen: ergebnis.is_err() };

    if entscheide(snap).rollt_zurueck() {
        // Den rohen git-Grund NUR ins Diagnose-Log, nie vor den Nutzer (E56).
        if let Err(e) = &ergebnis {
            crate::gitlog::record("rueckfall", format!("Operation schiefgegangen → drehe zurück: {e}"));
        }
        // Automatisch zurückdrehen: HEAD + Arbeitsbereich landen exakt auf dem Schnappschuss.
        zurueckdrehen(root)?;
        crate::gitlog::record("rueckfall", "zurückgedreht — Repo steht wieder auf dem Schnappschuss");
        // Die ehrliche Domänen-Meldung statt des rohen git-Texts (E56).
        return Err(std::io::Error::other(ZURUECKGEDREHT));
    }

    // Sauber durchgelaufen: das Netz einrollen (es wird nicht mehr gebraucht) und das echte Ergebnis
    // durchreichen.
    let _ = schnappschuss_loeschen(root);
    ergebnis
}

/// Den exakten aktuellen Stand als Schnappschuss-Ref festhalten, **bevor** die gefährliche Operation
/// läuft. Wir legen den Ref auf den aktuellen HEAD; den **Arbeitsbereich** (offene, ungesicherte
/// Änderungen) sichern wir zusätzlich als eigenen, nicht angehängten Commit-Objekt-Schnappschuss,
/// damit das Zurückdrehen auch ungesicherte Arbeit exakt wiederherstellt. Idempotent gegen einen
/// alten Ref: er wird überschrieben (vorheriges Aufräumen hat ihn ohnehin schon entfernt).
fn schnappschuss_legen(root: &Path) -> std::io::Result<()> {
    let head = rev_parse(root, "HEAD")?;
    // Den Arbeitsbereich (inkl. unverfolgter Dateien) als baumelndes Commit festhalten, OHNE HEAD
    // oder den Index dauerhaft zu bewegen — `stash create` baut genau so ein Objekt und lässt den
    // Arbeitsbereich unberührt. Ist nichts offen, liefert es leer; dann ist der HEAD der Schnappschuss.
    let stash = stash_create(root).unwrap_or_default();
    // Wir hinterlegen die HEAD-Id im Ref selbst und (falls vorhanden) die Stash-Id im Ref-Reflog-
    // Message-freien Begleit-Ref. Pragmatisch: zwei Refs, beide rein lokal, beide unter refs/plm/.
    git_ok(root, &["update-ref", SNAPSHOT_REF, &head])?;
    if !stash.is_empty() {
        git_ok(root, &["update-ref", &format!("{SNAPSHOT_REF}-arbeit"), &stash])?;
    } else {
        // Kein offener Arbeitsstand: einen evtl. alten Arbeits-Ref entfernen, damit das Zurückdrehen
        // nicht fälschlich einen fremden Arbeitsstand einspielt.
        let _ = git_delete_ref(root, &format!("{SNAPSHOT_REF}-arbeit"));
    }
    Ok(())
}

/// **Automatisch zurückdrehen** auf den Schnappschuss: HEAD + Arbeitsbereich landen exakt auf dem
/// Stand von vor der Operation. Das ist die *einzige* Stelle, an der `reset --hard` benutzt wird —
/// und bewusst: hier soll genau die fehlgeschlagene Operation vollständig verworfen und der
/// gesicherte Vorher-Zustand wiederhergestellt werden (kein „behalten" wie bei `zurueckwerfen`,
/// sondern das Netz greift). Danach, falls es offene Arbeit gab, den gesicherten Arbeitsstand
/// wieder einspielen, sodass auch Ungesichertes nicht verloren geht. Zum Schluss das Netz einrollen.
fn zurueckdrehen(root: &Path) -> std::io::Result<()> {
    // 1) HEAD + Index + Arbeitsbereich hart auf den Schnappschuss-HEAD setzen — die gescheiterte
    //    Operation ist damit restlos verworfen, das Repo steht wieder auf dem Stand von vorher.
    git_ok(root, &["reset", "--hard", SNAPSHOT_REF])?;
    // Unverfolgte Dateien, die die gescheiterte Operation hinterlassen hat, sicher wegräumen, damit
    // der Arbeitsbereich exakt dem Schnappschuss entspricht.
    let _ = git_ok(root, &["clean", "-fd"]);

    // 2) Gab es offene, ungesicherte Arbeit, sie aus dem Arbeits-Schnappschuss zurückspielen, damit
    //    der Rückfall auch sie exakt wiederherstellt (nichts geht verloren, E8). Best-effort.
    let arbeit_ref = format!("{SNAPSHOT_REF}-arbeit");
    if rev_parse(root, &arbeit_ref).is_ok() {
        let _ = git_ok(root, &["stash", "apply", &arbeit_ref]);
    }

    // 3) Das Netz einrollen — die Transaktion ist beendet.
    let _ = schnappschuss_loeschen(root);
    Ok(())
}

/// **Eine liegengebliebene Transaktion aufräumen** (Stromausfall/Absturz, E56): liegt beim Start
/// noch ein Schnappschuss-Ref herum, kam eine frühere Transaktion nicht zu Ende — also **zurück-
/// drehen** auf diesen Schnappschuss und das Netz einrollen. Gibt `true` zurück, wenn tatsächlich
/// ein Netz gefunden und abgewickelt wurde; `false`, wenn nichts lag (der Normalfall beim Start).
///
/// Das ist der zeitversetzte Zwilling des synchronen Fehlerpfads: derselbe `zurueckdrehen`, nur
/// vom nächsten Start ausgelöst statt vom selben Lauf. Beim App-Start einmal über die Produkte zu
/// rufen, deckt den „Strom weg mitten in der Operation"-Fall ohne jede zusätzliche Logik.
pub fn aufraeumen_offene(root: &Path) -> std::io::Result<bool> {
    if rev_parse(root, SNAPSHOT_REF).is_err() {
        return Ok(false); // kein Netz liegt — nichts kam abhanden, nichts zu tun
    }
    crate::gitlog::record(
        "rueckfall",
        "liegengebliebener Schnappschuss gefunden (Absturz?) → drehe zurück",
    );
    zurueckdrehen(root)?;
    Ok(true)
}

// ----------------------------------------------------------------------------------------------
// Kleine git-Helfer (lokal gehalten, damit diese Glue in sich geschlossen bleibt)
// ----------------------------------------------------------------------------------------------

/// Das Schnappschuss-Netz (beide Refs) einrollen. Best-effort: ein fehlender Ref ist kein Fehler.
fn schnappschuss_loeschen(root: &Path) -> std::io::Result<()> {
    let _ = git_delete_ref(root, SNAPSHOT_REF);
    let _ = git_delete_ref(root, &format!("{SNAPSHOT_REF}-arbeit"));
    Ok(())
}

/// Eine Revision auflösen (z. B. `HEAD` oder ein Ref-Name) → die Objekt-Id. Schlägt fehl, wenn die
/// Revision nicht existiert — so erkennt der Aufrufer „kein Schnappschuss liegt".
fn rev_parse(root: &Path, rev: &str) -> std::io::Result<String> {
    let out = crate::gitrunner::command(root)
        .args(["rev-parse", "--verify", "--quiet", rev])
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!("rev-parse {rev} fehlgeschlagen")));
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.is_empty() {
        return Err(std::io::Error::other(format!("rev-parse {rev} leer")));
    }
    Ok(id)
}

/// Den Arbeitsbereich (inkl. Index + unverfolgter Dateien) als **baumelndes** Commit-Objekt
/// festhalten, ohne HEAD/Index zu bewegen — `git stash create` tut genau das. Liefert die Id oder
/// leer, wenn nichts offen ist.
fn stash_create(root: &Path) -> std::io::Result<String> {
    let out = crate::gitrunner::command(root)
        .args(["stash", "create", "rueckfall-schnappschuss"])
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other("stash create fehlgeschlagen"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Einen Ref löschen (best-effort; ein bereits fehlender Ref ist kein Fehler für den Aufrufer).
fn git_delete_ref(root: &Path, name: &str) -> std::io::Result<()> {
    git_ok(root, &["update-ref", "-d", name])
}

/// Ein git-Subkommando in `root` fahren; ein Nicht-Null-Exit wird zum `io::Error`. (Spiegelt das
/// `git_ok` der Schwester-Glue `worktreeglue`.)
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
    fn ref_exists(dir: &Path, name: &str) -> bool {
        Command::new("git")
            .arg("-C").arg(dir)
            .args(["rev-parse", "--verify", "--quiet", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Ein Repo mit einem Stand. Gibt (tmp, root-HEAD-Id).
    fn one_stand_repo() -> (tempfile::TempDir, String) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        run(root, &["init", "-q", "-b", "main"]);
        run(root, &["config", "user.email", "t@t.de"]);
        run(root, &["config", "user.name", "t"]);
        write(root, "f.txt", "v1\n");
        run(root, &["add", "f.txt"]);
        run(root, &["commit", "-qm", "init"]);
        let h = head(root);
        (tmp, h)
    }

    /// **Erfolg**: läuft die Operation durch, schreibt der Wrapper ihren neuen Stand fest, gibt das
    /// Ergebnis durch und rollt das Netz ein (kein Schnappschuss-Ref bleibt liegen).
    #[test]
    fn commits_and_rolls_up_the_net_on_success() {
        let (tmp, before) = one_stand_repo();
        let root = tmp.path();

        let res = mit_rueckfallnetz(root, |r| {
            write(r, "f.txt", "v2\n");
            run(r, &["commit", "-aqm", "echte arbeit"]);
            Ok::<_, std::io::Error>(42)
        })
        .unwrap();

        assert_eq!(res, 42, "das echte Ergebnis wird durchgereicht");
        // Der neue Stand steht (HEAD ist gewandert).
        assert_ne!(head(root), before, "der erfolgreiche Stand ist festgeschrieben");
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v2\n");
        // Das Netz ist eingerollt — kein Schnappschuss-Ref liegt mehr.
        assert!(!ref_exists(root, SNAPSHOT_REF), "Netz nach Erfolg eingerollt");
        assert!(!ref_exists(root, &format!("{SNAPSHOT_REF}-arbeit")));
    }

    /// **Forcierter Fehler ⇒ Rückfall**: gibt die Operation einen Fehler — nachdem sie das Repo
    /// schon angefasst hat —, dreht der Wrapper automatisch zurück (HEAD + Arbeitsbereich exakt auf
    /// vorher) und meldet die ehrliche Domänen-Meldung, nie den rohen git-Text.
    #[test]
    fn forced_error_rolls_back_to_the_snapshot_with_an_honest_message() {
        let (tmp, before) = one_stand_repo();
        let root = tmp.path();

        let err = mit_rueckfallnetz(root, |r| {
            // Die Operation richtet schon Schaden an …
            write(r, "f.txt", "halb-kaputt\n");
            run(r, &["commit", "-aqm", "halbe arbeit"]);
            write(r, "muell.txt", "verirrt\n"); // unverfolgte Datei dazu
                                                 // … und scheitert dann.
            Err::<i32, _>(std::io::Error::other("fatal: not a git repository (roher git-Text)"))
        })
        .unwrap_err();

        // Die Meldung ist Domänensprache, kein roher git-Text (E56).
        assert_eq!(err.to_string(), ZURUECKGEDREHT);
        assert!(!err.to_string().contains("fatal"), "kein roher git-Text in der Meldung");
        // Das Repo steht wieder exakt auf dem Schnappschuss von vorher.
        assert_eq!(head(root), before, "HEAD ist zurückgedreht");
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v1\n");
        // Die verirrte unverfolgte Datei der gescheiterten Operation ist weg.
        assert!(!root.join("muell.txt").exists(), "der Müll der gescheiterten Operation ist geräumt");
        // Das Netz ist eingerollt.
        assert!(!ref_exists(root, SNAPSHOT_REF), "Netz nach Rückfall eingerollt");
    }

    /// **Ungesicherte Arbeit überlebt den Rückfall**: war beim Start offene, ungesicherte Arbeit im
    /// Arbeitsbereich, stellt das Zurückdrehen sie wieder her — der Rückfall verliert nichts (E8).
    #[test]
    fn rollback_restores_open_unsaved_work() {
        let (tmp, before) = one_stand_repo();
        let root = tmp.path();

        // Offene, ungesicherte Arbeit liegt schon, BEVOR die Transaktion startet.
        write(root, "f.txt", "offen-ungesichert\n");

        let _ = mit_rueckfallnetz(root, |r| {
            run(r, &["commit", "-aqm", "die op committet die offene arbeit"]);
            Err::<i32, _>(std::io::Error::other("op scheitert danach"))
        })
        .unwrap_err();

        // HEAD ist auf den Vorher-Stand zurück …
        assert_eq!(head(root), before, "HEAD zurückgedreht");
        // … und die offene, ungesicherte Arbeit ist wiederhergestellt, nicht verloren.
        assert_eq!(
            std::fs::read_to_string(root.join("f.txt")).unwrap(),
            "offen-ungesichert\n",
            "ungesicherte Arbeit überlebt den Rückfall"
        );
    }

    /// **Stromausfall-Zwilling**: liegt beim Start ein Schnappschuss-Ref herum (eine Transaktion kam
    /// nicht zu Ende), dreht [`aufraeumen_offene`] auf ihn zurück und rollt das Netz ein.
    #[test]
    fn leftover_snapshot_is_cleaned_up_by_rolling_back() {
        let (tmp, before) = one_stand_repo();
        let root = tmp.path();

        // Eine halbe Transaktion simulieren: Schnappschuss auf den Vorher-Stand, dann „crasht" die
        // Operation mitten im Werk (committet weiter), ohne je festzuschreiben/zurückzudrehen.
        schnappschuss_legen(root).unwrap();
        write(root, "f.txt", "mitten-drin\n");
        run(root, &["commit", "-aqm", "halbe arbeit vor dem absturz"]);
        assert!(ref_exists(root, SNAPSHOT_REF), "das Netz liegt (Absturz simuliert)");

        // Der nächste Start räumt auf → Rückfall.
        let drehte = aufraeumen_offene(root).unwrap();
        assert!(drehte, "ein liegengebliebenes Netz wird abgewickelt");
        assert_eq!(head(root), before, "der nächste Start dreht auf den Schnappschuss zurück");
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v1\n");
        assert!(!ref_exists(root, SNAPSHOT_REF), "Netz danach eingerollt");

        // Liegt nichts, ist Aufräumen ein No-Op.
        assert!(!aufraeumen_offene(root).unwrap(), "ohne Netz nichts zu tun");
    }
}
