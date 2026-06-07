//! End-to-end-Test der Recovery-Transaktion (Issue #133, E56/E56a).
//!
//! Ein **echtes Wegwerf-Repo** fährt die Transaktion: erst ein Schnappschuss, dann die gefährliche
//! Operation, und bei forciertem Fehler der **automatische Rückfall** auf den Schnappschuss. NICHTS
//! hier berührt einen Server oder LFS; die reine „commit-or-rollback"-Entscheidung + die ehrliche
//! Meldung sind in `src/recovery.rs` tabellengetestet — diese Datei beweist nur, dass die
//! seiteneffekt-behaftete Transaktions-Glue real gegen git zusammenspielt (Stil von
//! `setup_ceremony.rs`/`import_clean_init.rs`).

use app_lib::recovery::ZURUECKGEDREHT;
use app_lib::recoveryglue::{aufraeumen_offene, mit_rueckfallnetz, SNAPSHOT_REF};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn ref_exists(root: &Path, name: &str) -> bool {
    Command::new("git")
        .arg("-C").arg(root)
        .args(["rev-parse", "--verify", "--quiet", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ein Produkt-Repo mit einem Stand auf `main`.
fn seed_product(root: &Path) {
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    std::fs::write(root.join("f.txt"), b"v1\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "init"]);
}

/// Der Kern-Beweis von E56: gibt die gefährliche Operation einen Fehler — nachdem sie das Repo schon
/// angefasst hat —, dreht die Transaktion **automatisch zurück** (HEAD + Arbeitsbereich exakt auf den
/// Stand von vorher) und meldet die **ehrliche Domänen-Meldung**, nie den rohen git-Text.
#[test]
fn forced_error_auto_rolls_back_and_speaks_domain_language() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    seed_product(root);
    let before = git_out(root, &["rev-parse", "HEAD"]);

    let err = mit_rueckfallnetz(root, |r| {
        // Die gefährliche Operation richtet schon Schaden an: ein neuer Commit plus eine verirrte
        // unverfolgte Datei …
        std::fs::write(r.join("f.txt"), b"halb-kaputt\n").unwrap();
        git(r, &["commit", "-aqm", "halbe arbeit"]);
        std::fs::write(r.join("verirrt.txt"), b"muell\n").unwrap();
        // … und scheitert dann (forcierter Fehler mit rohem git-Text).
        Err::<i32, _>(std::io::Error::other("fatal: bad revision (roher git-Text)"))
    })
    .unwrap_err();

    // Die Meldung ist Domänensprache („ist schiefgegangen, ich hab's zurückgedreht"), kein git-Text.
    assert_eq!(err.to_string(), ZURUECKGEDREHT, "ehrliche Domänen-Meldung");
    assert!(!err.to_string().contains("fatal"), "kein roher git-Text leakt zum Nutzer");

    // Das Repo steht wieder exakt auf dem Schnappschuss von vorher — es ging nichts kaputt.
    assert_eq!(git_out(root, &["rev-parse", "HEAD"]), before, "HEAD ist zurückgedreht");
    assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v1\n");
    assert!(!root.join("verirrt.txt").exists(), "der Müll der gescheiterten Operation ist geräumt");

    // Das Schnappschuss-Netz ist eingerollt (kein Ref bleibt liegen).
    assert!(!ref_exists(root, SNAPSHOT_REF), "Netz nach dem Rückfall eingerollt");
}

/// Läuft die Operation sauber durch, schreibt die Transaktion ihren Stand fest, reicht das echte
/// Ergebnis durch und rollt das Netz ein — der Erfolgspfad fasst nichts an.
#[test]
fn success_commits_through_and_leaves_no_net() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    seed_product(root);
    let before = git_out(root, &["rev-parse", "HEAD"]);

    let res = mit_rueckfallnetz(root, |r| {
        std::fs::write(r.join("f.txt"), b"v2\n").unwrap();
        git(r, &["commit", "-aqm", "echte arbeit"]);
        Ok::<_, std::io::Error>("fertig")
    })
    .unwrap();

    assert_eq!(res, "fertig", "das echte Ergebnis wird durchgereicht");
    assert_ne!(git_out(root, &["rev-parse", "HEAD"]), before, "der Erfolg ist festgeschrieben");
    assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v2\n");
    assert!(!ref_exists(root, SNAPSHOT_REF), "kein Netz nach Erfolg");
}

/// Stromausfall-Zwilling (E56): liegt beim Start noch ein Schnappschuss-Ref herum (eine Transaktion
/// kam nicht zu Ende), dreht das Aufräumen darauf zurück — derselbe Effekt wie ein synchroner Fehler,
/// nur vom nächsten Start ausgelöst.
#[test]
fn leftover_snapshot_from_a_crash_is_rolled_back_on_next_start() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    seed_product(root);
    let before = git_out(root, &["rev-parse", "HEAD"]);

    // Eine halbe Transaktion simulieren: ein Schnappschuss-Ref auf den Vorher-Stand liegt, und das
    // Repo ist schon weitergewandert — als wäre der Strom mitten in der Operation weg gewesen.
    git(root, &["update-ref", SNAPSHOT_REF, &before]);
    std::fs::write(root.join("f.txt"), b"mitten-drin\n").unwrap();
    git(root, &["commit", "-aqm", "halbe arbeit vor dem absturz"]);
    assert!(ref_exists(root, SNAPSHOT_REF), "das Netz liegt (Absturz simuliert)");

    // Der nächste Start räumt auf → Rückfall.
    let drehte = aufraeumen_offene(root).unwrap();
    assert!(drehte, "ein liegengebliebenes Netz wird abgewickelt");
    assert_eq!(git_out(root, &["rev-parse", "HEAD"]), before, "zurück auf den Schnappschuss");
    assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "v1\n");
    assert!(!ref_exists(root, SNAPSHOT_REF), "Netz danach eingerollt");

    // Liegt nichts, ist ein erneuter Start-Aufräumlauf ein No-Op.
    assert!(!aufraeumen_offene(root).unwrap(), "ohne Netz nichts zu tun");
}
