//! End-to-end Glue-Test für den **Notausgang** „Export als einfache Ordner" (Issue #134, E56).
//!
//! Baut ein echtes Wegwerf-Repo mit einem markierten Stand (`version/v1.0`) und einem neueren,
//! teils noch nicht festgeschriebenen Jetzt-Zustand, ruft die Materialisierung **rein lokal** auf
//! (kein Remote eingerichtet — genau der „Server klemmt"-Fall) und prüft: der Jetzt-Zustand und der
//! markierte Stand liegen als blanke Ordner auf der Platte, ohne `.git`, ohne `_plm` — also ohne
//! Werkzeug-Voodoo lesbar. Die reinen Helfer (Namens-Sicherheit, `_plm`-Erkennung) sind in
//! `src/exportglue.rs` tabellengetestet; hier beweisen wir die seiteneffekt-behaftete Glue gegen
//! echtes git.

use app_lib::exportglue::export_einfache_ordner;
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn touch(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Ein Produkt-Repo mit zwei Ständen: `v1.0` (markiert) und ein neuerer, teils ungesicherter
/// Jetzt-Zustand. Trägt auch einen `_plm`-Store, der NICHT in den blanken Export gehört.
fn produkt() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    // git in dieser Umgebung signiert Commits standardmäßig; der Notausgang ist davon unabhängig.
    git(root, &["config", "commit.gpgsign", "false"]);

    // Stand 1: zwei Arbeits-Dateien + ein Werkzeug-Store. Markiert als version/v1.0.
    touch(&root.join("elektronik/board.txt"), b"board v1");
    touch(&root.join("mechanik/gehaeuse.txt"), b"gehaeuse v1");
    touch(&root.join("_plm/stack.json"), b"{store}");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "auto: init"]);
    git(root, &["tag", "version/v1.0"]);

    // Stand 2 (Jetzt): board geändert + committet, gehaeuse geändert aber NOCH NICHT gesichert.
    touch(&root.join("elektronik/board.txt"), b"board v2 committet");
    git(root, &["commit", "-am", "auto: v2"]);
    touch(&root.join("mechanik/gehaeuse.txt"), b"gehaeuse OFFEN ungesichert");

    tmp
}

#[test]
fn exports_current_state_and_tagged_state_as_plain_folders() {
    let tmp = produkt();
    let root = tmp.path();
    // Ein vom Produkt getrenntes Ziel (so wie der Nutzer im Notfall einen externen Ort wählt).
    let ziel_dir = tempfile::tempdir().unwrap();
    let ziel = ziel_dir.path();

    let res = export_einfache_ordner(root, Some(ziel)).expect("Export läuft rein lokal");

    // Es gibt den Jetzt-Zustand UND den markierten Stand v1.0.
    let marken: Vec<&str> = res.staende.iter().map(|s| s.marke.as_str()).collect();
    assert!(marken.contains(&"Jetzt-Zustand"), "Jetzt-Zustand fehlt: {marken:?}");
    assert!(marken.contains(&"v1.0"), "markierter Stand v1.0 fehlt: {marken:?}");

    // ── Jetzt-Zustand: exakt das, was JETZT auf der Platte liegt (inkl. ungesicherter Arbeit). ──
    let jetzt = Path::new(&res.staende.iter().find(|s| s.marke == "Jetzt-Zustand").unwrap().pfad).to_path_buf();
    assert_eq!(fs::read_to_string(jetzt.join("elektronik/board.txt")).unwrap(), "board v2 committet");
    // Die noch nicht festgeschriebene Änderung MUSS mitgehen — im Notfall zählt der echte Stand.
    assert_eq!(
        fs::read_to_string(jetzt.join("mechanik/gehaeuse.txt")).unwrap(),
        "gehaeuse OFFEN ungesichert"
    );

    // ── Markierter Stand v1.0: der alte, festgeschriebene Inhalt. ──
    let v1 = Path::new(&res.staende.iter().find(|s| s.marke == "v1.0").unwrap().pfad).to_path_buf();
    assert_eq!(fs::read_to_string(v1.join("elektronik/board.txt")).unwrap(), "board v1");
    assert_eq!(fs::read_to_string(v1.join("mechanik/gehaeuse.txt")).unwrap(), "gehaeuse v1");

    // ── „Einfacher Ordner": ohne _plm/git-Voodoo lesbar — kein .git, kein _plm in BEIDEN Ständen. ──
    for stand in [&jetzt, &v1] {
        assert!(!stand.join(".git").exists(), "blanker Ordner darf kein .git tragen: {stand:?}");
        assert!(!stand.join("_plm").exists(), "der Werkzeug-Store gehört nicht in den Export: {stand:?}");
    }
}

#[test]
fn works_without_any_tagged_state_just_the_current_one() {
    // Ein frisches Produkt ohne markierten Stand: der Notausgang liefert wenigstens den Jetzt-Zustand.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init"]);
    git(root, &["config", "user.name", "anna"]);
    git(root, &["config", "user.email", "anna@example.com"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    touch(&root.join("firmware/main.c"), b"int main(){}");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "auto: init"]);

    // Fallback-Ziel (None) -> innerhalb des Produkts unter .plm-export.
    let res = export_einfache_ordner(root, None).expect("Export läuft");
    assert_eq!(res.staende.len(), 1, "nur der Jetzt-Zustand");
    assert_eq!(res.staende[0].marke, "Jetzt-Zustand");
    let jetzt = Path::new(&res.staende[0].pfad);
    assert_eq!(fs::read_to_string(jetzt.join("firmware/main.c")).unwrap(), "int main(){}");
    assert!(!jetzt.join(".git").exists());
}

#[test]
fn export_is_idempotent_a_second_run_yields_the_same_clean_content() {
    let tmp = produkt();
    let root = tmp.path();
    let ziel_dir = tempfile::tempdir().unwrap();
    let ziel = ziel_dir.path();

    let first = export_einfache_ordner(root, Some(ziel)).unwrap();
    let second = export_einfache_ordner(root, Some(ziel)).unwrap();

    // Gleiche Stände, gleiche Pfade — der zweite Lauf leert die Ordner vorher, mischt also keine Reste.
    assert_eq!(
        first.staende.iter().map(|s| s.pfad.clone()).collect::<Vec<_>>(),
        second.staende.iter().map(|s| s.pfad.clone()).collect::<Vec<_>>(),
    );
    let v1 = Path::new(&second.staende.iter().find(|s| s.marke == "v1.0").unwrap().pfad);
    assert_eq!(fs::read_to_string(v1.join("elektronik/board.txt")).unwrap(), "board v1");
}
