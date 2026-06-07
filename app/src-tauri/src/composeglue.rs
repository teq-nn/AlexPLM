//! Git-Plumbing-Glue für den **Compose-Commit** einer Produkt-Revision (Issue #139, E51c/E52).
//!
//! Die reine Entscheidung — welcher Heimat-Teilbaum aus welchem Release-Tag, welche Stückliste,
//! welche Eltern-Stände — lebt im **Compose-Kern** ([`crate::compose`]); diese Schicht löst sie
//! **physisch** ein, mit reinem git-Plumbing und **ohne Submodule**:
//!
//! 1. Für jeden Eintrag der Baum-Spezifikation den Heimat-Teilbaum des gewählten Tags via
//!    `read-tree --prefix=<heimat>` in einen **temporären Index** heben (nie den Arbeits-Index der
//!    Werkbank anfassen — der `GIT_INDEX_FILE`-Trick hält den Bau seiteneffektfrei für den Nutzer).
//! 2. Aus dem zusammengesetzten Index mit `write-tree` **einen** Baum schreiben.
//! 3. Mit `commit-tree` einen **mehrelterigen** Commit auf diesen Baum legen — ein Eltern-Stand pro
//!    gewähltem Release-Tag (E51c: der Multi-Parent-Graph trägt die Compose-Knoten, `parents: Vec`).
//!
//! So entsteht **ein** Produkt-Stand, dessen Baum „neue PCB + alte Firmware" exakt als die zwei
//! freigegebenen Teilbäume enthält (**Baum = BOM**); „als Ordner öffnen" auf diesem Compose-Commit
//! liefert nie WIP, weil er auf eingefrorene Tags zeigt, nie auf den HEAD eines Bausteins.
//!
//! Haus-Muster (`composeglue` über `compose` wie `reconcileglue` über `reconciler`): **alle**
//! git-Spawns laufen durch [`crate::gitrunner`] (Issue #22); die Logik bleibt im Kern, hier nur die
//! Mechanik. Die Invariante **Baum = BOM** wird nach dem Bau gegen den realen Baum gelesen
//! ([`compose_commit_baum_posten`]), damit kein „Tag HEAD + Lüge im Manifest" entstehen kann.

use crate::autocommit::format_timestamp;
use crate::compose::{compose, BausteinWahl, ComposeSpezifikation, StuecklistenPosten};
use std::path::Path;
use std::time::SystemTime;

/// Das Ergebnis eines gebauten **Compose-Commits** (Issue #139): die id des frischen Produkt-Stands,
/// die zugehörige Produkt-Stückliste (BOM) und die Eltern-Stände, die der mehrelterige Commit trägt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeCommit {
    /// Commit-id des frisch gebauten Produkt-Stands (der Compose-Commit). Intern — der Nutzer sieht
    /// nur den Produkt-Stand, nie „commit".
    pub commit_id: String,
    /// Die Produkt-Stückliste, die dieser Stand einlöst (ein Posten pro verpflichtendem Baustein).
    pub stueckliste: Vec<StuecklistenPosten>,
    /// Die aufgelösten Eltern-Stände (Commit-ids), einer pro gewähltem Release-Tag.
    pub parents: Vec<String>,
}

/// Eine **Produkt-Revision komponieren** (Issue #139, E51c/E52): aus der Auswahl `{Baustein →
/// (Heimat, Release-Tag)}` einen **Compose-Commit ohne Submodule** bauen, dessen Baum physisch der
/// Stückliste entspricht. Der Compose-Kern entscheidet die Komposition; diese Glue löst sie mit
/// `read-tree`/`write-tree`/`commit-tree` ein und gibt den frischen Stand + die BOM zurück.
///
/// `now` ist injiziert, damit der Commit-Stempel testbar ist. Reihenfolge wie im Kern: der Baum
/// wird nach Heimat sortiert gesetzt (deterministisch), die Eltern in Auswahl-Reihenfolge gehängt.
pub fn compose_produkt_revision(
    root: &Path,
    wahlen: &[BausteinWahl],
    now: SystemTime,
) -> std::io::Result<ComposeCommit> {
    // 1) Reine Entscheidung: Baum-Spezifikation + BOM + Eltern-Tags. Ein Auswahl-Fehler (doppelte
    //    Heimat, Verschachtelung, leeres Feld) wird hier zur deutschen io-Meldung — nie still gebaut.
    let spec = compose(wahlen).map_err(|e| std::io::Error::other(e.meldung()))?;

    // 2) Jeden gewählten Release-Tag in seinen Commit auflösen — das sind die Eltern-Stände des
    //    mehrelterigen Compose-Commits (E51c) und zugleich die Quellen der `read-tree`-Teilbäume.
    let mut parent_commits: Vec<String> = Vec::with_capacity(spec.parents.len());
    for tag in &spec.parents {
        parent_commits.push(resolve_tag_commit(root, tag)?);
    }

    // 3) Den Compose-Baum in einem TEMPORÄREN Index zusammensetzen — nie den Werkbank-Index anfassen.
    let tree_oid = build_compose_tree(root, &spec)?;

    // 4) Den mehrelterigen Commit auf den fertigen Baum legen (ein `-p` pro Eltern-Stand).
    let timestamp = format_timestamp(now);
    let commit_id = commit_tree(root, &tree_oid, &parent_commits, &timestamp)?;

    Ok(ComposeCommit {
        commit_id,
        stueckliste: spec.stueckliste,
        parents: parent_commits,
    })
}

/// Den **Compose-Baum** in einem temporären Index aus den Heimat-Teilbäumen der gewählten Tags
/// zusammensetzen und sein Tree-Objekt schreiben. Nutzt einen eigenen `GIT_INDEX_FILE`, damit der
/// Index der Werkbank (der laufende Arbeitsbereich) unberührt bleibt — der Bau ist für den Nutzer
/// seiteneffektfrei. Pro Eintrag `read-tree --prefix=<heimat>/ <tag>` hebt genau den Teilbaum des
/// freigegebenen Stands unter seinen Heimat-Pfad; `write-tree` friert das Ganze als einen Baum ein.
fn build_compose_tree(root: &Path, spec: &ComposeSpezifikation) -> std::io::Result<String> {
    let index = temp_index_path(root);
    // Falls eine frühere, abgebrochene Komposition eine Index-Datei hinterließ: frisch starten.
    let _ = std::fs::remove_file(&index);

    let result = (|| -> std::io::Result<String> {
        for eintrag in &spec.baum {
            // Den **Heimat-Teilbaum** des Tags (nicht dessen ganzen Baum) unter den Heimat-Pfad
            // setzen: `read-tree --prefix=<heimat>/ <tag>:<heimat>` adressiert mit `<tag>:<heimat>`
            // genau das Teilbaum-Objekt unter dem Heimat-Pfad im freigegebenen Stand. So ist der
            // gesetzte Teilbaum **objektgleich** mit dem freigegebenen Heimat-Teilbaum — exakt die
            // Identität, gegen die die Invariante Baum = BOM prüft. Der abschließende `/` ist die
            // git-Konvention für „als Unterverzeichnis"; doppelte Heimaten sind vom Kern
            // ausgeschlossen, also kollidiert kein Präfix mit einem schon gesetzten.
            let prefix = format!("--prefix={}/", eintrag.heimat);
            let quelle = format!("{}:{}", eintrag.release_tag, eintrag.heimat);
            git_index_ok(root, &index, &["read-tree", &prefix, &quelle])?;
        }
        // Den zusammengesetzten Index als einen Baum schreiben.
        let out = git_index(root, &index, &["write-tree"])?;
        if !out.status.success() {
            return Err(std::io::Error::other(format!(
                "git write-tree failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    })();

    // Den temporären Index immer wieder wegräumen — er ist reines Bau-Gerüst, kein Zustand.
    let _ = std::fs::remove_file(&index);
    result
}

/// Den **mehrelterigen Compose-Commit** auf den fertigen Baum legen (E51c): `commit-tree <tree>` mit
/// einem `-p <parent>` pro gewähltem Release-Stand. Eine boring Maschinen-Nachricht (E39: kein
/// Menschentext im Commit). Liefert die Commit-id des frischen Produkt-Stands.
fn commit_tree(
    root: &Path,
    tree_oid: &str,
    parents: &[String],
    timestamp: &str,
) -> std::io::Result<String> {
    let mut args: Vec<String> = vec!["commit-tree".to_string(), tree_oid.to_string()];
    for p in parents {
        args.push("-p".to_string());
        args.push(p.clone());
    }
    args.push("-m".to_string());
    // Boring, maschinell: nennt nur, dass dies ein komponierter Produkt-Stand ist + den Stempel.
    args.push(format!("compose: produkt-revision, {timestamp}"));

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git(root, &arg_refs)?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!(
            "git commit-tree failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Einen Tag (oder beliebigen Ref/Commit-ish) in seine **Commit-id** auflösen. Ein Tag, der auf
/// keinen Commit zeigt, ist ein harter Fehler — ohne Eltern-Stand gäbe es keinen reproduzierbaren
/// Compose-Commit. `<tag>^{commit}` peelt einen evtl. annotierten Tag bis auf den Commit durch.
fn resolve_tag_commit(root: &Path, tag: &str) -> std::io::Result<String> {
    let spec = format!("{tag}^{{commit}}");
    let out = git(root, &["rev-parse", "--verify", "--quiet", &spec])?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || id.is_empty() {
        return Err(std::io::Error::other(format!(
            "Freigabe-Stand {tag} ist nicht auflösbar"
        )));
    }
    Ok(id)
}

/// Die **Posten, die der reale Baum eines Compose-Commits physisch enthält** — die git-seitige
/// Wahrheit, gegen die die Invariante **Baum = BOM** geprüft wird (Issue #139). Liest die obersten
/// Einträge des Commit-Baums (`ls-tree <commit>`) und, für jeden Baum-Eintrag (`tree`), aus welchem
/// gewählten Tag dieser Teilbaum stammt — bestimmt durch **Identität des Teilbaum-Objekts**: der
/// Heimat-Teilbaum im Compose-Commit ist objektgleich mit dem Teilbaum desselben Heimat-Pfads im
/// freigegebenen Stand. So zeigt der Test, dass der Baum genau die freigegebenen Stände trägt —
/// nicht ein „Tag HEAD + Lüge im Manifest".
///
/// `erwartet` ist die BOM, gegen die geprüft wird; nur deren Heimat-Pfade werden gelesen (eine
/// vollständige Baum-Diff wäre teurer und für die Invariante nicht nötig). Liefert die real
/// gefundenen Posten als `(heimat, release_tag)`, wo `release_tag` der BOM-Tag ist, dessen
/// Heimat-Teilbaum **objektgleich** im Compose-Commit liegt — oder ein leerer Tag, wenn der Baum
/// unter diesem Heimat-Pfad etwas anderes (oder nichts) trägt.
pub fn compose_commit_baum_posten(
    root: &Path,
    commit_id: &str,
    erwartet: &[StuecklistenPosten],
) -> std::io::Result<Vec<(String, String)>> {
    let mut gefunden = Vec::with_capacity(erwartet.len());
    for posten in erwartet {
        // Das Teilbaum-Objekt unter dem Heimat-Pfad IM Compose-Commit.
        let im_commit = subtree_oid(root, commit_id, &posten.heimat)?;
        // Das Teilbaum-Objekt unter demselben Heimat-Pfad IM freigegebenen Stand (dem BOM-Tag).
        let im_tag = subtree_oid(root, &posten.release_tag, &posten.heimat)?;
        // Objektgleich ⇒ der Compose-Commit trägt physisch genau diesen freigegebenen Teilbaum.
        let tag = if !im_commit.is_empty() && im_commit == im_tag {
            posten.release_tag.clone()
        } else {
            String::new()
        };
        gefunden.push((posten.heimat.clone(), tag));
    }
    Ok(gefunden)
}

/// Die Objekt-id des Teilbaums unter `heimat` in einem `commit-ish`. Leer, wenn dort kein Baum liegt
/// (Pfad fehlt oder ist eine Datei). `ls-tree -d <commit-ish> -- <heimat>` listet genau den einen
/// Baum-Eintrag; das zweite Feld der Ausgabe ist die Tree-Objekt-id.
fn subtree_oid(root: &Path, commit_ish: &str, heimat: &str) -> std::io::Result<String> {
    let out = git(root, &["ls-tree", "-d", commit_ish, "--", heimat])?;
    if !out.status.success() {
        return Ok(String::new());
    }
    // Format einer Zeile: `<mode> <type> <oid>\t<pfad>`. Wir wollen die `<oid>` der `tree`-Zeile.
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let mut felder = line.split_whitespace();
        let _mode = felder.next();
        let typ = felder.next();
        let oid = felder.next();
        if typ == Some("tree") {
            if let Some(oid) = oid {
                return Ok(oid.to_string());
            }
        }
    }
    Ok(String::new())
}

/// Der Pfad des temporären Bau-Index unter `.git/` — lokal, nie geteilt, pro Prozess eindeutig, damit
/// zwei parallele Kompositionen sich nicht ins Gehege kommen. Liegt in `.git/`, also fasst ihn keine
/// Produkt-Projektion und kein `status` an.
fn temp_index_path(root: &Path) -> std::path::PathBuf {
    root.join(".git").join(format!(
        "plm-compose-index-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

/// Ein hardened git-Aufruf (über [`crate::gitrunner`]) — wie in `graphread`.
fn git(root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    crate::gitrunner::command(root).args(args).output()
}

/// Ein git-Aufruf gegen einen **expliziten** Index (`GIT_INDEX_FILE`), damit `read-tree`/`write-tree`
/// den Werkbank-Index nie berühren. Sonst identisch zum hardened [`git`].
fn git_index(
    root: &Path,
    index: &Path,
    args: &[&str],
) -> std::io::Result<std::process::Output> {
    crate::gitrunner::command(root)
        .env("GIT_INDEX_FILE", index)
        .args(args)
        .output()
}

/// [`git_index`], aber ein Nicht-Null-Exit ist ein harter Fehler (mit git-stderr in der Meldung).
fn git_index_ok(root: &Path, index: &Path, args: &[&str]) -> std::io::Result<()> {
    let out = git_index(root, index, args)?;
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
    use std::path::PathBuf;
    use std::process::Command;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-composeglue-it-{}-{}",
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

    fn git_run(root: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git").arg("-C").arg(root).args(args).output().unwrap()
    }

    fn git_ok_run(root: &Path, args: &[&str]) {
        assert!(git_run(root, args).status.success(), "git {args:?} failed");
    }

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    /// Stand up a repo and tag two independent freigegebene Stände: an elektronik-Heimat at
    /// `freigabe/elektronik/Rev-B` and a firmware-Heimat at `freigabe/firmware/v0.3`. They live on
    /// the same line here but each tag carries the relevant subtree — exactly what E51a's durable
    /// Baustein-Freigabe-Tags provide. Returns the repo root.
    fn repo_mit_zwei_freigaben() -> PathBuf {
        let root = tmp();
        git_ok_run(&root, &["init", "--quiet"]);
        git_ok_run(&root, &["config", "user.email", "t@example.com"]);
        git_ok_run(&root, &["config", "user.name", "Tester"]);
        git_ok_run(&root, &["config", "commit.gpgsign", "false"]);

        // Stand 1: die freigegebene Elektronik (Rev B) — die "neue PCB".
        write(&root, "elektronik/pcb.kicad_pcb", b"REV B layout");
        write(&root, "elektronik/notes.txt", b"rev b");
        git_ok_run(&root, &["add", "-A"]);
        git_ok_run(&root, &["commit", "--quiet", "-m", "elektronik rev b"]);
        git_ok_run(&root, &["tag", "freigabe/elektronik/Rev-B"]);

        // Stand 2: dazu kommt eine ältere Firmware-Freigabe — die "alte Firmware".
        write(&root, "firmware/main.c", b"int main(){return 0;}");
        git_ok_run(&root, &["add", "-A"]);
        git_ok_run(&root, &["commit", "--quiet", "-m", "firmware v0.3"]);
        git_ok_run(&root, &["tag", "freigabe/firmware/v0.3"]);

        root
    }

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)
    }

    /// AC (Glue): `read-tree`/`commit-tree` bauen einen **mehrelterigen** Compose-Commit ohne
    /// Submodule — ein Eltern-Stand pro gewähltem Tag — und der Compose-Knoten trägt `parents: Vec`.
    #[test]
    fn baut_mehrelterigen_compose_commit_ohne_submodule() {
        let root = repo_mit_zwei_freigaben();
        let wahlen = vec![
            BausteinWahl {
                baustein_id: "kicad".into(),
                heimat: "elektronik".into(),
                release_tag: "freigabe/elektronik/Rev-B".into(),
            },
            BausteinWahl {
                baustein_id: "zephyr".into(),
                heimat: "firmware".into(),
                release_tag: "freigabe/firmware/v0.3".into(),
            },
        ];

        let cc = compose_produkt_revision(&root, &wahlen, now()).unwrap();

        // Mehrelterig: genau zwei Eltern-Stände, je einer pro gewähltem Release-Tag.
        assert_eq!(cc.parents.len(), 2, "ein Eltern-Stand pro gewähltem Tag");
        let elektronik_commit = String::from_utf8_lossy(
            &git_run(&root, &["rev-parse", "freigabe/elektronik/Rev-B^{commit}"]).stdout,
        )
        .trim()
        .to_string();
        let firmware_commit = String::from_utf8_lossy(
            &git_run(&root, &["rev-parse", "freigabe/firmware/v0.3^{commit}"]).stdout,
        )
        .trim()
        .to_string();
        assert!(cc.parents.contains(&elektronik_commit));
        assert!(cc.parents.contains(&firmware_commit));

        // Der reale Commit trägt genau diese zwei Eltern (git sieht den Multi-Parent-Graph).
        let parents_out =
            git_run(&root, &["rev-list", "--parents", "-n", "1", &cc.commit_id]);
        let line = String::from_utf8_lossy(&parents_out.stdout).trim().to_string();
        let ids: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(ids.len(), 3, "commit + zwei Eltern: {line}");

        // Ohne Submodule: kein `.gitmodules`, kein `commit`-Eintrag im Baum (alles echte Bäume/Blobs).
        let tree = git_run(&root, &["ls-tree", "-r", "-t", &cc.commit_id]);
        let tree = String::from_utf8_lossy(&tree.stdout);
        assert!(!tree.contains("commit "), "kein Submodul-Gitlink im Baum:\n{tree}");
        assert!(!tree.contains(".gitmodules"), "kein .gitmodules:\n{tree}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// AC (Invariante): der Baum der Produkt-Revision deckt sich **physisch** mit der BOM — jeder
    /// Heimat-Teilbaum im Compose-Commit ist objektgleich mit dem freigegebenen Stand seines Tags
    /// (kein „Tag HEAD + Lüge im Manifest"). Und der Inhalt ist exakt der freigegebene: neue PCB +
    /// alte Firmware, nichts sonst.
    #[test]
    fn baum_deckt_sich_physisch_mit_der_bom() {
        let root = repo_mit_zwei_freigaben();
        let wahlen = vec![
            BausteinWahl {
                baustein_id: "kicad".into(),
                heimat: "elektronik".into(),
                release_tag: "freigabe/elektronik/Rev-B".into(),
            },
            BausteinWahl {
                baustein_id: "zephyr".into(),
                heimat: "firmware".into(),
                release_tag: "freigabe/firmware/v0.3".into(),
            },
        ];
        let cc = compose_produkt_revision(&root, &wahlen, now()).unwrap();

        // Die Invariante: jeder BOM-Posten findet seinen objektgleichen Teilbaum im Compose-Commit.
        let posten = compose_commit_baum_posten(&root, &cc.commit_id, &cc.stueckliste).unwrap();
        for p in &cc.stueckliste {
            let gefunden = posten
                .iter()
                .find(|(h, _)| h == &p.heimat)
                .unwrap_or_else(|| panic!("Heimat {} fehlt im Baum", p.heimat));
            assert_eq!(
                gefunden.1, p.release_tag,
                "Heimat {} muss physisch aus {} kommen, nicht aus etwas anderem",
                p.heimat, p.release_tag
            );
        }

        // Und der Inhalt ist exakt der freigegebene Stand jedes Bereichs (Baum = BOM, konkret):
        let pcb = git_run(&root, &["show", &format!("{}:elektronik/pcb.kicad_pcb", cc.commit_id)]);
        assert_eq!(String::from_utf8_lossy(&pcb.stdout), "REV B layout");
        let fw = git_run(&root, &["show", &format!("{}:firmware/main.c", cc.commit_id)]);
        assert_eq!(String::from_utf8_lossy(&fw.stdout), "int main(){return 0;}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Der Bau ist für den Nutzer seiteneffektfrei: der Werkbank-Index (der laufende Arbeitsbereich)
    /// bleibt unberührt, und kein temporärer Bau-Index bleibt unter `.git/` liegen.
    #[test]
    fn bau_laesst_werkbank_index_unberuehrt_und_raeumt_auf() {
        let root = repo_mit_zwei_freigaben();
        // Eine uncommittete Änderung in der Werkbank: sie darf den Compose-Bau überleben.
        write(&root, "elektronik/pcb.kicad_pcb", b"WIP layout, noch nicht freigegeben");
        let status_vorher =
            String::from_utf8_lossy(&git_run(&root, &["status", "--porcelain"]).stdout).to_string();

        let wahlen = vec![BausteinWahl {
            baustein_id: "kicad".into(),
            heimat: "elektronik".into(),
            release_tag: "freigabe/elektronik/Rev-B".into(),
        }];
        let _ = compose_produkt_revision(&root, &wahlen, now()).unwrap();

        let status_nachher =
            String::from_utf8_lossy(&git_run(&root, &["status", "--porcelain"]).stdout).to_string();
        assert_eq!(status_vorher, status_nachher, "der Werkbank-Status bleibt unverändert");
        // Kein zurückgelassener Bau-Index.
        let leftovers: Vec<_> = std::fs::read_dir(root.join(".git"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("plm-compose-index-"))
            .collect();
        assert!(leftovers.is_empty(), "der temporäre Compose-Index wurde aufgeräumt");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Eine mehrdeutige Auswahl (doppelte Heimat) wird nie still gebaut — die Glue gibt den
    /// benannten Domänen-Fehler des Kerns als deutsche Meldung zurück, kein Compose-Commit entsteht.
    #[test]
    fn mehrdeutige_auswahl_baut_nichts() {
        let root = repo_mit_zwei_freigaben();
        let wahlen = vec![
            BausteinWahl {
                baustein_id: "kicad".into(),
                heimat: "elektronik".into(),
                release_tag: "freigabe/elektronik/Rev-B".into(),
            },
            BausteinWahl {
                baustein_id: "kicad2".into(),
                heimat: "elektronik".into(),
                release_tag: "freigabe/firmware/v0.3".into(),
            },
        ];
        let err = compose_produkt_revision(&root, &wahlen, now()).unwrap_err();
        assert!(err.to_string().contains("doppelt"), "benannter Domänen-Fehler: {err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Ein nicht auflösbarer Release-Tag ist ein harter Fehler — ohne den freigegebenen Stand gäbe es
    /// keinen reproduzierbaren Compose-Commit (nie ein erfundener Eltern).
    #[test]
    fn unaufloesbarer_tag_ist_fehler() {
        let root = repo_mit_zwei_freigaben();
        let wahlen = vec![BausteinWahl {
            baustein_id: "kicad".into(),
            heimat: "elektronik".into(),
            release_tag: "freigabe/gibt/es/nicht".into(),
        }];
        let err = compose_produkt_revision(&root, &wahlen, now()).unwrap_err();
        assert!(err.to_string().contains("nicht auflösbar"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
