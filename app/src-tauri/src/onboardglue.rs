//! Onboarding-Glue: Tag-1-Ignore/LFS in die Dotfiles hängen (Issue #48, adressiert #63).
//!
//! Dünne Seiteneffekt-Schicht über dem reinen [`crate::markerblock`]-Kern. Beim Onboarding eines
//! Bausteins in ein Produkt:
//! 1. die gewünschten Zeilen je Dotfile aus dem Baustein **ableiten** (rein, total),
//! 2. `.gitignore`/`.gitattributes` lesen (fehlend ⇒ leer, nie Fehler),
//! 3. den Marker-Block des Bausteins idempotent setzen ([`markerblock::upsert_block`]),
//! 4. zurückschreiben — **nur wenn sich etwas ändert** (kein unnötiger Schreibzugriff/Diff).
//!
//! **Tag-1-Pflicht:** Das geschieht beim Anlegen/Erweitern des Produkt-Stacks, also **bevor** das
//! Tool des Bausteins seine erste Binärdatei/Müll erzeugt — daher kein späteres `lfs migrate`.
//!
//! **Quelle der Muster (bewusste Entscheidung, ADR-würdig):**
//! - **Ignore** = die `ignore`-Liste des Bausteins, 1:1 (das ist Tool-Wissen, ADR 0003) — **plus**
//!   die aus den `rekonstruierbar`-Regeln (E50b) abgeleiteten Zeilen (siehe unten). Beide Pfad-Klassen
//!   teilen denselben `.gitignore`-Marker-Block (E18, keine Spiegelung), denn beide steuern, **was git
//!   sieht**: Ignore wirft Müll weg, Rekonstruierbar wirft ableitbaren Ballast weg und **hält das
//!   gepinnte Manifest** verfolgt. Reihenfolge: erst Ignore, dann je Rekonstruierbar-Regel das
//!   Framework-Ignore und direkt darunter die Manifest-Negationen (`!west.yml`).
//! - **LFS** = die `lfs`-Liste des Bausteins **vereinigt** mit den Bausteins-`globs`, deren
//!   Endung formatintrinsisch *lockable* ist (über [`crate::classifier`], die einzige Wahrheit
//!   über Lockability — ADR 0003). So landen z.B. CAD-`globs` auch dann unter LFS, wenn der
//!   Baustein keine explizite `lfs`-Liste pflegt (schließt #63: Onboarding ohne LFS-Muster).
//!   Reihenfolge: erst explizite `lfs`, dann abgeleitete Globs; Duplikate werden entfernt.
//!
//! **Nested-`.git`-Grenze (E50a).** Die hier erzeugten Muster sind die *deklarative* Hälfte: sie sagen
//! git, welche rekonstruierbaren Dateien es ignorieren soll. Die *beobachtende* Hälfte — der Walk, der
//! an einem genesteten `.git` stoppt ([`crate::nestedboundary`]) — sorgt dafür, dass Watcher/
//! Klassifizierer gar nicht erst in den fremden Framework-Baum hineinsehen. Beide ziehen am selben
//! Strang: kein rekonstruierbarer Ballast im Repo, kein Commit-Sturm aus dem fremden Baum.

use crate::baustein::{Baustein, RekonstruierbarRegel};
use crate::classifier::{classify, Bucket};
use crate::markerblock::upsert_block;
use crate::stackstore::ProduktStack;
use std::path::Path;

/// Dateiname der Ignore-Dotfile.
const GITIGNORE: &str = ".gitignore";
/// Dateiname der Attribut-Dotfile.
const GITATTRIBUTES: &str = ".gitattributes";

/// Die kanonische `.gitignore`-Zeilenmenge eines Bausteins: seine `ignore`-Liste 1:1 (Tool-Wissen),
/// gefolgt von den aus den `rekonstruierbar`-Regeln (E50b) abgeleiteten Zeilen. Beide Pfad-Klassen
/// teilen denselben Marker-Block (E18). Rein, total, deterministisch.
pub fn ignore_lines(b: &Baustein) -> Vec<String> {
    let mut lines = b.ignore.clone();
    lines.extend(rekonstruierbar_lines(&b.rekonstruierbar));
    lines
}

/// Die `.gitignore`-Zeilen, die eine Liste von [`RekonstruierbarRegel`] erzeugt (E50b, Issue #137).
///
/// Pro Regel entsteht **zuerst** das Framework-Ignore (der rekonstruierbare Ballast fliegt aus dem
/// Repo) und **direkt darunter** je gepinntem Manifest eine **Negation** (`!<manifest>`), die das
/// Manifest aus dem Ignore wieder herausholt — git verfolgt also weiter **Quelle + gepinntes Manifest**,
/// nie die rekonstruierbaren Framework-Dateien. Leere/whitespace-Einträge werden übersprungen (das
/// `validate_baustein`-Gate verhindert sie ohnehin am Speichern; hier degradieren wir still). Total.
///
/// Bewusst **kein** `!`-Doppeln: ist ein Manifest schon ohne führendes `!` angegeben, setzen wir es
/// davor; ein bereits negiert angegebenes Manifest bleibt unverändert (der Nutzer darf ausdrücklich
/// auch eine handgeänderte Komponente verfolgen, ohne dass wir sein `!` verschlucken oder verdoppeln).
pub fn rekonstruierbar_lines(regeln: &[RekonstruierbarRegel]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for regel in regeln {
        let framework = regel.framework.trim();
        if framework.is_empty() {
            continue;
        }
        lines.push(framework.to_string());
        for manifest in &regel.manifest {
            let m = manifest.trim();
            if m.is_empty() {
                continue;
            }
            // Manifest wieder verfolgen: als Negation, ohne ein bereits gesetztes `!` zu verdoppeln.
            if m.starts_with('!') {
                lines.push(m.to_string());
            } else {
                lines.push(format!("!{m}"));
            }
        }
    }
    lines
}

/// Die kanonische `.gitattributes`-Zeilenmenge eines Bausteins: explizite `lfs`-Muster vereinigt
/// mit den lockable `globs`, jeweils als vollständige LFS-Attributzeile gerendert. Rein, total,
/// deterministisch und dedupliziert (Reihenfolge erhalten: explizit zuerst, dann abgeleitet).
pub fn lfs_lines(b: &Baustein) -> Vec<String> {
    let mut patterns: Vec<String> = Vec::new();
    let push_unique = |patterns: &mut Vec<String>, p: &str| {
        if !patterns.iter().any(|x| x == p) {
            patterns.push(p.to_string());
        }
    };
    // 1) Explizite LFS-Muster des Bausteins.
    for p in &b.lfs {
        push_unique(&mut patterns, p);
    }
    // 2) Globs, deren Format intrinsisch lockable ist (CAD, Mesh, KiCad-Quellen, …).
    for g in &b.globs {
        if is_lockable_glob(g) {
            push_unique(&mut patterns, g);
        }
    }
    patterns.into_iter().map(|p| attr_line(&p)).collect()
}

/// Ob ein Glob ein lockable Format adressiert. Stützt sich auf den Classifier-Kern: wir prüfen die
/// Endung des Globs (z.B. `*.f3d`, `*.kicad_pcb`). Globs ohne lockable Endung (Quelltext, Doku)
/// liefern `false`. Total — beliebige Eingabe ergibt eine Entscheidung, nie Panik.
fn is_lockable_glob(glob: &str) -> bool {
    // `classify` betrachtet nur die letzte Endung des letzten Pfadsegments; ein Glob wie `*.f3d`
    // verhält sich dabei wie ein Dateiname. Reine, mergebare Globs (CMakeLists.txt, *.c) -> false.
    matches!(
        classify(glob, None),
        Bucket::BinaryUnmergeable | Bucket::NominalTextUnmergeable
    )
}

/// Eine vollständige LFS-+Lockable-Attributzeile für ein Muster — exakt das Format, das
/// [`crate::import::import_attr_lines`] erzeugt, damit Import und Onboarding übereinstimmen.
fn attr_line(pattern: &str) -> String {
    format!("{pattern} filter=lfs diff=lfs merge=lfs -text lockable")
}

/// Eine Dotfile lesen (fehlend/leer ⇒ leerer String, nie Fehler), den Marker-Block des Bausteins
/// idempotent setzen und **nur bei Änderung** zurückschreiben. Gibt zurück, ob geschrieben wurde.
fn upsert_dotfile(root: &Path, file: &str, id: &str, lines: &[String]) -> std::io::Result<bool> {
    let path = root.join(file);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = upsert_block(&existing, id, lines);
    if updated == existing {
        return Ok(false);
    }
    std::fs::write(&path, updated)?;
    Ok(true)
}

/// Tag-1-Ignore/LFS für **einen** Baustein in ein Produkt-`root` hängen (idempotent).
/// Setzt den Marker-Block in `.gitignore` (Ignore-Muster) und `.gitattributes` (LFS/Lockable).
pub fn onboard_baustein_dotfiles(root: &Path, b: &Baustein) -> std::io::Result<()> {
    upsert_dotfile(root, GITIGNORE, &b.id, &ignore_lines(b))?;
    upsert_dotfile(root, GITATTRIBUTES, &b.id, &lfs_lines(b))?;
    Ok(())
}

/// Tag-1-Ignore/LFS für **alle** Bausteine eines Produkt-Stacks hängen (idempotent). Wird direkt
/// nach dem Schreiben von `_plm/stack.json` aufgerufen, sodass die Muster stehen, bevor ein Tool
/// seine erste Datei erzeugt. Reihenfolge folgt dem Stack; jeder Baustein besitzt seinen Block.
pub fn onboard_stack_dotfiles(root: &Path, stack: &ProduktStack) -> std::io::Result<()> {
    for sb in &stack.bausteine {
        onboard_baustein_dotfiles(root, &sb.baustein)?;
    }
    Ok(())
}

/// Den **Heimat-Ordner** jedes (nicht stillgelegten) Bausteins auf der Platte anlegen, damit der
/// Nutzer nach dem Einrichten sofort sieht, **wohin** seine Dateien gehören (PRD §50/§29: geführte
/// Anlage). Leere Heimat (Produktwurzel) wird übersprungen; ein bereits existierender Ordner ist
/// kein Fehler (`create_dir_all` ist idempotent). Leere Ordner sind für git unsichtbar — das ist
/// gewollt: erst eine echte Datei darin wird erfasst. Der Ordner ist nur die sichtbare Einladung.
pub fn scaffold_heimat_dirs(root: &Path, stack: &ProduktStack) -> std::io::Result<()> {
    for sb in &stack.bausteine {
        let b = &sb.baustein;
        if b.stillgelegt {
            continue;
        }
        let heimat = b.heimat.trim().trim_matches('/');
        if heimat.is_empty() {
            continue;
        }
        std::fs::create_dir_all(root.join(heimat))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baustein::Oeffnen;
    use std::path::PathBuf;

    fn baustein(id: &str, globs: &[&str], ignore: &[&str], lfs: &[&str]) -> Baustein {
        Baustein {
            id: id.to_string(),
            version: 1,
            name: id.to_string(),
            heimat: "h".to_string(),
            globs: globs.iter().map(|s| s.to_string()).collect(),
            ignore: ignore.iter().map(|s| s.to_string()).collect(),
            lfs: lfs.iter().map(|s| s.to_string()).collect(),
            rekonstruierbar: vec![],
            oeffnen: Oeffnen::Auto,
            startaufgaben: vec![],
            default_kanten: vec![],
            paar_default_kanten: vec![],
            stillgelegt: false,
        }
    }

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-onboard-ut-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Ignore-Zeilen = ignore-Liste 1:1.
    #[test]
    fn ignore_lines_are_the_ignore_list() {
        let b = baustein("zephyr", &["*.c"], &["build/", "twister-out/"], &[]);
        assert_eq!(ignore_lines(&b), vec!["build/".to_string(), "twister-out/".to_string()]);
    }

    /// LFS-Zeilen: lockable globs werden abgeleitet, mergeable globs nicht; explizite lfs zuerst.
    #[test]
    fn lfs_lines_derive_lockable_globs_and_keep_explicit_first() {
        // table: (globs, explicit lfs) -> expected patterns (the part before " filter=lfs ...")
        let cases: &[(&[&str], &[&str], &[&str])] = &[
            // CAD-Glob ist lockable -> abgeleitet; *.c ist mergeable -> nicht.
            (&["*.f3d", "*.c"], &[], &["*.f3d"]),
            // KiCad-Quellen sind nominal-text-unmergeable -> lockable.
            (&["*.kicad_pro", "*.kicad_sch", "*.kicad_pcb"], &[], &["*.kicad_sch", "*.kicad_pcb"]),
            // rein mergebare Globs -> keine LFS-Zeilen.
            (&["CMakeLists.txt", "*.c", "*.h"], &[], &[]),
            // explizite lfs zuerst, dann abgeleitete; ein bereits explizites Muster nicht doppelt.
            (&["*.f3d"], &["*.step", "*.f3d"], &["*.step", "*.f3d"]),
        ];
        for (globs, lfs, expected_patterns) in cases {
            let b = baustein("x", globs, &[], lfs);
            let expected: Vec<String> =
                expected_patterns.iter().map(|p| attr_line(p)).collect();
            assert_eq!(lfs_lines(&b), expected, "globs={globs:?} lfs={lfs:?}");
        }
    }

    /// Rekonstruierbar (E50b): jede Regel erzeugt das Framework-Ignore + die Manifest-Negationen,
    /// in stabiler Reihenfolge (Framework zuerst, dann die `!`-Negationen darunter). Whitespace- und
    /// Leereinträge fallen still weg; ein bereits negiert gepinntes Manifest wird nicht verdoppelt.
    #[test]
    fn rekonstruierbar_lines_emit_framework_ignore_plus_manifest_negations() {
        // table: (regeln) -> erwartete .gitignore-Zeilen
        let cases: &[(&[(&str, &[&str])], &[&str])] = &[
            // west: modules/ + .west/ ignorieren, west.yml weiter verfolgen
            (
                &[("modules/", &["west.yml"][..]), (".west/", &[][..])],
                &["modules/", "!west.yml", ".west/"],
            ),
            // PlatformIO: .pio/ ignorieren, platformio.ini + lockfile verfolgen
            (
                &[(".pio/", &["platformio.ini", "lockfile.json"][..])],
                &[".pio/", "!platformio.ini", "!lockfile.json"],
            ),
            // ESP-IDF: sdkconfig als gepinntes Manifest; handgeänderte Komponente bereits negiert -> nicht verdoppeln
            (
                &[("components/", &["sdkconfig", "!components/mein_patch/"][..])],
                &["components/", "!sdkconfig", "!components/mein_patch/"],
            ),
            // Whitespace/Leeres fällt still weg
            (
                &[("  modules/  ", &["  west.yml  ", "", "   "][..])],
                &["modules/", "!west.yml"],
            ),
            // leeres Framework-Muster -> ganze Regel übersprungen
            (&[("   ", &["west.yml"][..])], &[]),
            (&[], &[]),
        ];
        for (regeln, expected) in cases {
            let rk: Vec<RekonstruierbarRegel> = regeln
                .iter()
                .map(|(fw, ms)| RekonstruierbarRegel {
                    framework: fw.to_string(),
                    manifest: ms.iter().map(|m| m.to_string()).collect(),
                })
                .collect();
            let got = rekonstruierbar_lines(&rk);
            let want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(got, want, "regeln = {regeln:?}");
        }
    }

    /// `ignore_lines` hängt die rekonstruierbar-Zeilen **nach** der Ignore-Liste an (ein Marker-Block).
    #[test]
    fn ignore_lines_append_rekonstruierbar_after_plain_ignore() {
        let mut b = baustein("zephyr", &["*.c"], &["build/", "twister-out/"], &[]);
        b.rekonstruierbar = vec![RekonstruierbarRegel {
            framework: "modules/".into(),
            manifest: vec!["west.yml".into()],
        }];
        assert_eq!(
            ignore_lines(&b),
            vec![
                "build/".to_string(),
                "twister-out/".to_string(),
                "modules/".to_string(),
                "!west.yml".to_string(),
            ]
        );
    }

    /// End-to-end: ein rekonstruierbar-Baustein schreibt das Framework-Ignore + die Manifest-Negation
    /// idempotent in den `.gitignore`-Marker-Block; Quelle + Manifest bleiben für git sichtbar.
    #[test]
    fn onboarding_writes_rekonstruierbar_into_gitignore_idempotently() {
        let dir = tmp();
        let mut b = baustein("zephyr", &["*.c"], &["build/"], &[]);
        b.rekonstruierbar = vec![RekonstruierbarRegel {
            framework: ".west/".into(),
            manifest: vec!["west.yml".into()],
        }];
        onboard_baustein_dotfiles(&dir, &b).unwrap();
        let ignore1 = std::fs::read_to_string(dir.join(GITIGNORE)).unwrap();
        assert!(ignore1.contains("# >>> baustein: zephyr >>>"));
        assert!(ignore1.contains(".west/"));
        assert!(ignore1.contains("!west.yml")); // Manifest bleibt verfolgt — ehrliche „Quelle + Manifest"

        // Zweiter Lauf: nichts schreiben, nichts ändern (idempotent).
        let wrote = upsert_dotfile(&dir, GITIGNORE, "zephyr", &ignore_lines(&b)).unwrap();
        assert!(!wrote, "zweiter Lauf hätte nicht schreiben dürfen");
        onboard_baustein_dotfiles(&dir, &b).unwrap();
        let ignore2 = std::fs::read_to_string(dir.join(GITIGNORE)).unwrap();
        assert_eq!(ignore1, ignore2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// End-to-end: Onboarding schreibt idempotente Blöcke in beide Dotfiles (zweimal == einmal).
    #[test]
    fn onboarding_writes_idempotent_blocks_twice_equals_once() {
        let dir = tmp();
        let b = baustein("kicad", &["*.kicad_pcb"], &["*.autosave"], &[]);
        onboard_baustein_dotfiles(&dir, &b).unwrap();
        let ignore1 = std::fs::read_to_string(dir.join(GITIGNORE)).unwrap();
        let attr1 = std::fs::read_to_string(dir.join(GITATTRIBUTES)).unwrap();

        // Zweiter Lauf darf nichts schreiben und nichts ändern.
        let wrote_ignore = upsert_dotfile(&dir, GITIGNORE, "kicad", &ignore_lines(&b)).unwrap();
        let wrote_attr = upsert_dotfile(&dir, GITATTRIBUTES, "kicad", &lfs_lines(&b)).unwrap();
        assert!(!wrote_ignore, "zweiter .gitignore-Lauf hätte nicht schreiben dürfen");
        assert!(!wrote_attr, "zweiter .gitattributes-Lauf hätte nicht schreiben dürfen");

        onboard_baustein_dotfiles(&dir, &b).unwrap();
        let ignore2 = std::fs::read_to_string(dir.join(GITIGNORE)).unwrap();
        let attr2 = std::fs::read_to_string(dir.join(GITATTRIBUTES)).unwrap();
        assert_eq!(ignore1, ignore2);
        assert_eq!(attr1, attr2);

        assert!(ignore1.contains("# >>> baustein: kicad >>>"));
        assert!(ignore1.contains("*.autosave"));
        assert!(attr1.contains("*.kicad_pcb filter=lfs diff=lfs merge=lfs -text lockable"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Hand-Edits außerhalb des Marker-Blocks überleben das Onboarding unangetastet.
    #[test]
    fn hand_edits_outside_block_survive() {
        let dir = tmp();
        std::fs::write(dir.join(GITIGNORE), "# meins\nprivate/\n").unwrap();
        let b = baustein("zephyr", &["*.c"], &["build/"], &[]);
        onboard_baustein_dotfiles(&dir, &b).unwrap();
        let out = std::fs::read_to_string(dir.join(GITIGNORE)).unwrap();
        assert!(out.starts_with("# meins\nprivate/\n"));
        assert!(out.contains("# >>> baustein: zephyr >>>\nbuild/\n# <<< baustein: zephyr <<<"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Scaffolding legt je Baustein seinen Heimat-Ordner an; leere Heimat wird übersprungen,
    /// stillgelegte Bausteine ebenfalls; zweimal aufrufen ist idempotent.
    #[test]
    fn scaffold_creates_heimat_dirs_skipping_empty_and_stillgelegt() {
        use crate::stackstore::StackBaustein;
        let dir = tmp();
        let mut wurzel = baustein("root-baustein", &["*.x"], &[], &[]);
        wurzel.heimat = "".to_string(); // Produktwurzel -> kein Ordner
        let mut still = baustein("alt", &["*.y"], &[], &[]);
        still.heimat = "altbereich".to_string();
        still.stillgelegt = true;
        let mut mech = baustein("fusion", &["*.step"], &[], &[]);
        mech.heimat = "mechanik".to_string();

        let stack = ProduktStack {
            toolstack: None,
            bausteine: vec![
                StackBaustein::copy_of(&wurzel),
                StackBaustein::copy_of(&still),
                StackBaustein::copy_of(&mech),
            ],
        };
        scaffold_heimat_dirs(&dir, &stack).unwrap();
        scaffold_heimat_dirs(&dir, &stack).unwrap(); // idempotent

        assert!(dir.join("mechanik").is_dir(), "Heimat-Ordner angelegt");
        assert!(!dir.join("altbereich").exists(), "stillgelegt -> kein Ordner");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ein ganzer Stack: jeder Baustein bekommt seinen eigenen Block; alle koexistieren.
    #[test]
    fn onboard_whole_stack_gives_each_baustein_its_block() {
        use crate::stackstore::StackBaustein;
        let dir = tmp();
        let stack = ProduktStack {
            toolstack: None,
            bausteine: vec![
                StackBaustein::copy_of(&baustein("kicad", &["*.kicad_pcb"], &["*.autosave"], &[])),
                StackBaustein::copy_of(&baustein("fusion", &["*.f3d"], &[], &[])),
            ],
        };
        onboard_stack_dotfiles(&dir, &stack).unwrap();
        let ignore = std::fs::read_to_string(dir.join(GITIGNORE)).unwrap();
        let attr = std::fs::read_to_string(dir.join(GITATTRIBUTES)).unwrap();
        assert!(ignore.contains("baustein: kicad"));
        assert!(attr.contains("*.kicad_pcb filter=lfs"));
        assert!(attr.contains("*.f3d filter=lfs"));
        // idempotent über den ganzen Stack
        let ignore_again = {
            onboard_stack_dotfiles(&dir, &stack).unwrap();
            std::fs::read_to_string(dir.join(GITIGNORE)).unwrap()
        };
        assert_eq!(ignore, ignore_again);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
