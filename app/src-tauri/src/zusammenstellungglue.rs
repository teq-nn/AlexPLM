//! Dünne Lade-Glue für den **Zusammenstellungs-Kern** (Issue #140, E52a).
//!
//! Spiegelt die Haus-Teilung (`freigabegateglue` über `freigabegate`, `composeglue` über
//! `compose`): die Entscheidung lebt **nie** hier — sie lebt im reinen [`crate::zusammenstellung`]-
//! Kern. Diese Schicht **sammelt** nur, was der Kern braucht, und reicht es durch:
//!
//! - die **Bausteine** des Produkt-Stacks ([`crate::stackstore::read_stack`]) — je ein
//!   Checklisten-Posten; stillgelegte (E10, label-only) fallen heraus, sie stellen keinen Bereich;
//! - die **verfügbaren freigegebenen Stände** je Heimat ([`crate::graphread::list_baustein_freigaben`])
//!   — die dauerhaften Baustein-Freigabe-Tags (E51a), aus denen frisch gewählt oder ein Vorstand
//!   mitgenommen wird;
//! - die **aktuelle Auswahl** der laufenden Schnür-Sitzung — vom Aufrufer übergeben (heimat →
//!   Auswahl), nie hier berechnet.
//!
//! **Pflicht/Optional** ist im heutigen Baustein-Modell **kein** Feld; der Aufrufer benennt die
//! optionalen Bereiche (`optionale_heimaten`). Ohne Angabe ist jeder aktive Baustein **verpflichtend**
//! — der Default-Fall „jeder Tool-Bereich gehört in die Revision". Die Trennung ist eine reine
//! Eingabe-Achse des Kerns (E52a), kein Rollen-/Rechte-Layer.
//!
//! Es gibt **keine** Entscheidungslogik hier: Vollständigkeit und Checklisten-Zustand entscheidet
//! allein [`zusammenstellen`]. Treu zur Degradations-Invariante (E22): ein leeres/fehlendes Produkt
//! ergibt eine leere, **vollständige** Checkliste, nie einen Fehler.

use crate::graphread::{list_baustein_freigaben, release_baustein_revision};
use crate::stackstore::read_stack;
use crate::zusammenstellung::{
    kaltstart_seed_liste, zusammenstellen, Auswahl, BausteinEintrag, SeedPosten,
    ZusammenstellungsBericht,
};
use std::collections::BTreeMap;
use std::path::Path;

/// Die **aktuelle Auswahl** eines Bereichs in der laufenden Schnür-Sitzung (Issue #140), wie der
/// Aufrufer sie übergibt: für einen Heimat-Bereich entweder ein frischer Freigabe-Stand, das
/// bewusste „Vorstand mitnehmen" oder (gar nicht in der Map ⇒) offen. Schlicht — die Glue setzt sie
/// in die Kern-[`Auswahl`] um. `specta::Type` + `Deserialize`, damit sie als Befehls-Argument über
/// die Tauri-Naht kommt (das Frontend schickt die Auswahl der Schnür-Sitzung).
#[derive(specta::Type, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WahlEingabe {
    /// Der Heimat-Bereich, für den gewählt wurde (z.B. `"elektronik"`).
    pub heimat: String,
    /// Der gewählte Release-Tag (ein verfügbarer Freigabe-Stand des Bereichs, E51a).
    pub release_tag: String,
    /// Ob es das bewusste „alter Stand reicht" ist (Vorstand mitnehmen) statt ein frischer Stand.
    /// Beides ist ein vollwertiger Beitrag (E52a) — die Flagge erhält nur die sichtbare Geste.
    pub vorstand_mitnehmen: bool,
}

impl WahlEingabe {
    /// In die Kern-[`Auswahl`] umsetzen: frischer Stand vs. Vorstand-Mitnahme.
    fn to_auswahl(&self) -> Auswahl {
        if self.vorstand_mitnehmen {
            Auswahl::VorstandMitnehmen { release_tag: self.release_tag.clone() }
        } else {
            Auswahl::FrischerStand { release_tag: self.release_tag.clone() }
        }
    }
}

/// Die **Zusammenstellung eines Produkts** laden und ihren Bericht entscheiden (Issue #140, E52a):
/// aus dem Produkt-Stack, den verfügbaren Freigabe-Ständen je Heimat und der übergebenen Auswahl die
/// Checkliste + Vollständigkeit. Seiteneffekte nur in den Lesepfaden (`_plm`-Stack + git-Tags); das
/// Urteil ist der reine [`zusammenstellen`]-Kern.
///
/// `wahlen` ist die aktuelle Auswahl der Schnür-Sitzung (heimat → frisch/Vorstand); ein Bereich
/// **ohne** Eintrag steht offen. `optionale_heimaten` benennt die optionalen Bereiche — alle übrigen
/// aktiven Bausteine sind verpflichtend (Default). Ein stillgelegter Baustein (label-only, E10)
/// fällt heraus: er stellt keinen Bereich mehr, also auch keinen Checklisten-Posten.
///
/// Degradation (E22): ein leeres Produkt ergibt eine leere, vollständige Checkliste, nie ein Fehler.
pub fn zusammenstellung_fuer_produkt(
    root: &Path,
    wahlen: &[WahlEingabe],
    optionale_heimaten: &[String],
) -> ZusammenstellungsBericht {
    zusammenstellen(&sammle_eintraege(root, wahlen, optionale_heimaten))
}

/// Die **Baustein-Einträge** des Produkt-Stacks für den Kern sammeln (Issue #140/#142): je aktiver
/// Baustein ein Eintrag aus (Pflicht?, verfügbare Freigabe-Stände, aktuelle Auswahl). **Eine**
/// Quelle für beide Kern-Aufgaben — die Checkliste ([`zusammenstellen`]) **und** die Cold-Start-
/// Seed-Liste ([`kaltstart_seed_liste`]) lesen exakt dieselben Einträge, nichts wird dupliziert.
///
/// Ein **stillgelegter** Baustein (label-only, E10) stellt keinen Bereich mehr und fällt heraus.
/// Pflicht/Optional ist im Baustein-Modell kein Feld (E52a): alle nicht in `optionale_heimaten`
/// benannten aktiven Bausteine sind verpflichtend (Default „jeder Tool-Bereich gehört in die
/// Revision"). `wahlen` ist die laufende Auswahl; fehlt ein Eintrag, steht der Bereich offen.
fn sammle_eintraege(
    root: &Path,
    wahlen: &[WahlEingabe],
    optionale_heimaten: &[String],
) -> Vec<BausteinEintrag> {
    // Auswahl + Optional-Set für schnellen Zugriff nach Heimat indizieren.
    let auswahl_je_heimat: BTreeMap<&str, &WahlEingabe> =
        wahlen.iter().map(|w| (w.heimat.as_str(), w)).collect();
    let optional_set: std::collections::BTreeSet<&str> =
        optionale_heimaten.iter().map(String::as_str).collect();

    let stack = read_stack(root);
    stack
        .bausteine
        .iter()
        // Ein stillgelegter Baustein stellt keinen Bereich (E10) — kein Checklisten-Posten.
        .filter(|sb| !sb.baustein.stillgelegt)
        .map(|sb| {
            let heimat = sb.baustein.heimat.clone();
            let auswahl = auswahl_je_heimat
                .get(heimat.as_str())
                .map(|w| w.to_auswahl())
                .unwrap_or(Auswahl::Offen);
            BausteinEintrag {
                baustein_id: sb.baustein.id.clone(),
                pflicht: !optional_set.contains(heimat.as_str()),
                verfuegbare_staende: list_baustein_freigaben(root, &heimat),
                auswahl,
                heimat,
            }
        })
        .collect()
}

/// Was **ein** Cold-Start-Seed-Akt getan hat (Issue #142, E52b): je gesätem Pflicht-Baustein der
/// Bereich und die initiale Versionsmarke, die aus dem aktuellen Stand freigegeben wurde. Die UI
/// rendert daraus „elektronik · firmware initial freigegeben", ohne neu zu entscheiden.
/// `specta::Type` + `Serialize`, damit der Bericht über die Tauri-Naht kommt.
#[derive(specta::Type, serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub struct GesaeterBaustein {
    /// `id` des gesäten Bausteins (z.B. `"kicad"`).
    pub baustein_id: String,
    /// Der Heimat-Bereich, der initial freigegeben wurde (z.B. `"elektronik"`).
    pub heimat: String,
    /// Die initiale Versionsmarke, die gesetzt wurde (z.B. `"v0.1"`).
    pub version: String,
}

/// **Der eine Cold-Start-Akt** (Issue #142, E52b): beim **allerersten** Produkt-Release je
/// verpflichtendem Baustein **ohne** jeden freigegebenen Stand eine **initiale** Baustein-Revision
/// aus dem **aktuellen Stand** (HEAD) säen — statt N manueller Freigaben zu verlangen. Danach trägt
/// jeder Pflicht-Baustein einen Stand, und die erste Produkt-Revision ist komponierbar (E52a).
///
/// Die Teilung ist Haus-Muster: der reine [`kaltstart_seed_liste`]-Kern entscheidet **wen** es zu
/// säen gilt (Pflicht-Baustein ohne Stand — über dieselben [`sammle_eintraege`] wie die Checkliste);
/// diese Glue **tut** den Seitenffekt — sie setzt je Posten den **dauerhaften** Baustein-Freigabe-Tag
/// (E51a) auf den aktuellen Stand über [`release_baustein_revision`]. Schon revidierte und optionale
/// Bausteine werden nie gesät (das filtert der Kern bereits).
///
/// `version` ist die initiale Versionsmarke (z.B. `"v0.1"`/`"Rev A"`), die jeder gesäte Bereich
/// gemeinsam trägt — der Cold-Start ist **ein** Akt, ein gemeinsamer Startpunkt. Der aktuelle Stand
/// wird **einmal** als Commit-id (HEAD) aufgelöst, sodass alle initialen Tags auf denselben Stand
/// zeigen. Treu zur Degradations-Invariante (E22): ein leeres Produkt / kein Pflicht-Baustein ohne
/// Stand ⇒ es wird **nichts** gesät (leere Liste), nie ein Fehler. Idempotent in dem Sinn, dass ein
/// zweiter Aufruf nichts mehr findet (alle Pflicht-Bausteine tragen dann bereits einen Stand).
pub fn seede_initiale_revisionen(
    root: &Path,
    version: &str,
    optionale_heimaten: &[String],
) -> std::io::Result<Vec<GesaeterBaustein>> {
    let version = version.trim();
    if version.is_empty() {
        return Err(std::io::Error::other("Version darf nicht leer sein"));
    }

    // Wen es zu säen gilt, entscheidet allein der reine Kern (Pflicht-Baustein ohne Stand). Die
    // Auswahl ist leer — der Seed sät den **ersten** Stand, bevor überhaupt gewählt werden kann.
    let eintraege = sammle_eintraege(root, &[], optionale_heimaten);
    let seed: Vec<SeedPosten> = kaltstart_seed_liste(&eintraege);
    if seed.is_empty() {
        // Nichts zu säen — alle Pflicht-Bausteine tragen schon einen Stand (oder es gibt keine).
        return Ok(Vec::new());
    }

    // Den aktuellen Stand **einmal** auflösen, damit alle initialen Tags auf denselben HEAD zeigen.
    let head = aktueller_stand(root)?;

    let mut gesaet = Vec::with_capacity(seed.len());
    for posten in &seed {
        // Je Pflicht-Baustein die initiale Revision aus dem aktuellen Stand freigeben (E51a-Tag).
        release_baustein_revision(root, &posten.heimat, &head, version)?;
        gesaet.push(GesaeterBaustein {
            baustein_id: posten.baustein_id.clone(),
            heimat: posten.heimat.clone(),
            version: version.to_string(),
        });
    }
    Ok(gesaet)
}

/// Die Commit-id des **aktuellen Stands** (HEAD) auflösen — der Stand, aus dem der Cold-Start die
/// initialen Baustein-Revisionen sät. Fehlt HEAD (frisches Repo ohne Commit), ist das ein Fehler:
/// ohne einen Stand gibt es nichts zu säen.
fn aktueller_stand(root: &Path) -> std::io::Result<String> {
    let out = crate::gitrunner::command(root)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(
            "Kein aktueller Stand (HEAD) — es gibt nichts zu säen",
        ));
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.is_empty() {
        return Err(std::io::Error::other("Leerer HEAD — es gibt nichts zu säen"));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baustein::Baustein;
    use crate::stackstore::{write_stack, ProduktStack, StackBaustein};
    use crate::zusammenstellung::PostenZustand;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-zusammenstellung-ut-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(root: &Path, args: &[&str]) {
        crate::gitrunner::command(root).args(args).output().unwrap();
    }

    /// Ein git-Repo mit erstem Commit, damit Freigabe-Tags eine Basis haben.
    fn git_init(root: &Path) {
        git(root, &["init", "-q"]);
        git(root, &["config", "user.email", "t@t"]);
        git(root, &["config", "user.name", "t"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        fs::write(root.join("README"), "x").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "init"]);
    }

    fn baustein(id: &str, heimat: &str) -> Baustein {
        Baustein {
            id: id.to_string(),
            version: 1,
            name: id.to_string(),
            heimat: heimat.to_string(),
            globs: vec!["**/*".to_string()],
            ignore: Vec::new(),
            lfs: Vec::new(),
            rekonstruierbar: Vec::new(),
            oeffnen: Default::default(),
            startaufgaben: Vec::new(),
            default_kanten: Vec::new(),
            paar_default_kanten: Vec::new(),
            stillgelegt: false,
        }
    }

    fn stack_mit(bausteine: Vec<Baustein>) -> ProduktStack {
        ProduktStack {
            toolstack: None,
            bausteine: bausteine.iter().map(StackBaustein::copy_of).collect(),
        }
    }

    fn wahl(heimat: &str, tag: &str, vorstand: bool) -> WahlEingabe {
        WahlEingabe {
            heimat: heimat.to_string(),
            release_tag: tag.to_string(),
            vorstand_mitnehmen: vorstand,
        }
    }

    /// Ein leeres/fehlendes Produkt ergibt eine leere, **vollständige** Checkliste (E22) — die
    /// Zusammenstellung sperrt nie aus.
    #[test]
    fn leeres_produkt_ist_leer_und_vollstaendig() {
        let dir = tmp();
        let b = zusammenstellung_fuer_produkt(&dir, &[], &[]);
        assert!(b.vollstaendig);
        assert!(b.posten.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Die Glue verdrahtet Stack + Auswahl Ende-zu-Ende an die reine Regel: zwei Pflicht-Bausteine,
    /// einer frisch gewählt, einer offen → unvollständig, der offene steht aus. Dieselbe Staffelung
    /// wie der Kern, bewiesen durch den realen `_plm/stack.json`-Store.
    #[test]
    fn laedt_stack_und_staffelt_die_checkliste() {
        let dir = tmp();
        write_stack(
            &dir,
            &stack_mit(vec![baustein("kicad", "elektronik"), baustein("zephyr", "firmware")]),
        )
        .unwrap();

        // Nur elektronik gewählt (frisch); firmware bleibt offen.
        let b = zusammenstellung_fuer_produkt(
            &dir,
            &[wahl("elektronik", "freigabe/elektronik/Rev-B", false)],
            &[],
        );
        assert!(!b.vollstaendig, "ein offener Pflicht-Bereich → unvollständig");
        assert_eq!(b.ausstehende, vec!["firmware".to_string()]);

        let elektronik = b.posten.iter().find(|p| p.heimat == "elektronik").unwrap();
        assert_eq!(elektronik.zustand, PostenZustand::Beigetragen);
        assert_eq!(elektronik.release_tag, "freigabe/elektronik/Rev-B");
        let firmware = b.posten.iter().find(|p| p.heimat == "firmware").unwrap();
        assert_eq!(firmware.zustand, PostenZustand::Ausstehend);

        let _ = fs::remove_dir_all(&dir);
    }

    /// **Optional blockiert nie** (E52a): derselbe offene firmware-Bereich, aber als **optional**
    /// benannt → die Revision ist vollständig, obwohl firmware nichts beiträgt.
    #[test]
    fn optionaler_bereich_blockiert_nicht() {
        let dir = tmp();
        write_stack(
            &dir,
            &stack_mit(vec![baustein("kicad", "elektronik"), baustein("zephyr", "firmware")]),
        )
        .unwrap();

        let b = zusammenstellung_fuer_produkt(
            &dir,
            &[wahl("elektronik", "freigabe/elektronik/Rev-B", false)],
            &["firmware".to_string()], // firmware ist optional
        );
        assert!(b.vollstaendig, "ein optionaler offener Bereich hält nie auf");
        assert!(b.ausstehende.is_empty());
        let firmware = b.posten.iter().find(|p| p.heimat == "firmware").unwrap();
        assert_eq!(firmware.zustand, PostenZustand::OptionalOffen);
        assert!(!firmware.pflicht);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein **stillgelegter** Baustein (E10, label-only) stellt keinen Bereich mehr — er fällt aus der
    /// Checkliste und hält die Vollständigkeit nicht als Geisel.
    #[test]
    fn stillgelegter_baustein_faellt_aus_der_checkliste() {
        let dir = tmp();
        let mut alt = baustein("alt", "altbereich");
        alt.stillgelegt = true;
        write_stack(&dir, &stack_mit(vec![baustein("kicad", "elektronik"), alt])).unwrap();

        let b = zusammenstellung_fuer_produkt(
            &dir,
            &[wahl("elektronik", "freigabe/elektronik/Rev-B", false)],
            &[],
        );
        assert!(b.vollstaendig, "der stillgelegte Bereich steht nicht aus");
        assert_eq!(b.posten.len(), 1, "nur der aktive Baustein ist ein Posten");
        assert_eq!(b.posten[0].heimat, "elektronik");

        let _ = fs::remove_dir_all(&dir);
    }

    /// **Verfügbare Stände aus den dauerhaften Freigabe-Tags** (E51a, #140): die Glue listet die für
    /// einen Heimat-Bereich gesetzten `freigabe/<heimat>/…`-Tags und reicht sie als
    /// `verfuegbare_staende` durch — die Stände, aus denen frisch gewählt oder ein Vorstand
    /// mitgenommen wird. Ein **mitgenommener Vorstand** vervollständigt den Pflicht-Bereich.
    #[test]
    fn verfuegbare_staende_und_vorstand_mitnehmen() {
        let dir = tmp();
        git_init(&dir);
        write_stack(&dir, &stack_mit(vec![baustein("zephyr", "firmware")])).unwrap();

        // Zwei freigegebene Firmware-Stände setzen (die Tags spiegeln, was E51a schreibt).
        git(&dir, &["tag", "freigabe/firmware/v0.2"]);
        fs::write(dir.join("a"), "y").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "fw v0.3"]);
        git(&dir, &["tag", "freigabe/firmware/v0.3"]);

        // Den älteren Vorstand bewusst mitnehmen → der Pflicht-Bereich ist beigetragen.
        let b = zusammenstellung_fuer_produkt(
            &dir,
            &[wahl("firmware", "freigabe/firmware/v0.2", true)],
            &[],
        );
        assert!(b.vollstaendig, "ein mitgenommener Vorstand vervollständigt den Bereich");
        let p = &b.posten[0];
        assert_eq!(p.zustand, PostenZustand::VorstandMitgenommen);
        assert_eq!(p.release_tag, "freigabe/firmware/v0.2");
        // Beide gesetzten Stände stehen als verfügbar zur Wahl (die Glue hat sie gelistet).
        let staende = list_baustein_freigaben(&dir, "firmware");
        assert!(staende.contains(&"freigabe/firmware/v0.2".to_string()));
        assert!(staende.contains(&"freigabe/firmware/v0.3".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }

    /// **AC: ein Akt sät je Pflicht-Baustein eine initiale Revision aus dem aktuellen Stand, und
    /// danach ist die erste Produkt-Revision komponierbar** (Issue #142, E52b). Cold-Start: zwei
    /// Pflicht-Bausteine, **keiner** je freigegeben → `seede_initiale_revisionen` sät beiden einen
    /// `freigabe/<heimat>/<version>`-Tag auf HEAD, ganz ohne N manuelle Freigaben. Danach trägt jeder
    /// Bereich einen Stand: wählt man ihn als Vorstand mit, ist die Zusammenstellung **vollständig**.
    #[test]
    fn ein_akt_seedet_initiale_revisionen_und_macht_komponierbar() {
        let dir = tmp();
        git_init(&dir);
        write_stack(
            &dir,
            &stack_mit(vec![baustein("kicad", "elektronik"), baustein("zephyr", "firmware")]),
        )
        .unwrap();

        // Vor dem Seed: kein Bereich trägt einen Stand → die erste Produkt-Revision (alles offen)
        // ist unvollständig, beide Pflicht-Bereiche stehen aus.
        let vor = zusammenstellung_fuer_produkt(&dir, &[], &[]);
        assert!(!vor.vollstaendig, "vor dem Seed: noch nichts freigegeben");
        assert_eq!(vor.ausstehende, vec!["elektronik".to_string(), "firmware".to_string()]);

        // Der eine Cold-Start-Akt: je Pflicht-Baustein eine initiale Revision aus dem aktuellen Stand.
        let gesaet = seede_initiale_revisionen(&dir, "v0.1", &[]).unwrap();
        let heimaten: Vec<&str> = gesaet.iter().map(|g| g.heimat.as_str()).collect();
        assert_eq!(heimaten, vec!["elektronik", "firmware"], "beide Pflicht-Bereiche gesät");
        assert!(gesaet.iter().all(|g| g.version == "v0.1"));

        // Beide tragen jetzt einen dauerhaften Baustein-Freigabe-Tag auf den aktuellen Stand (E51a).
        let head = aktueller_stand(&dir).unwrap();
        for (heimat, tag) in [
            ("elektronik", "freigabe/elektronik/v0.1"),
            ("firmware", "freigabe/firmware/v0.1"),
        ] {
            let staende = list_baustein_freigaben(&dir, heimat);
            assert!(staende.contains(&tag.to_string()), "{heimat} trägt den initialen Tag");
            let auf = String::from_utf8_lossy(
                &crate::gitrunner::command(&dir)
                    .args(["rev-parse", &format!("{tag}^{{commit}}")])
                    .output()
                    .unwrap()
                    .stdout,
            )
            .trim()
            .to_string();
            assert_eq!(auf, head, "{tag} zeigt auf den aktuellen Stand");
        }

        // Nach dem Seed ist die erste Produkt-Revision komponierbar: jeder Bereich nimmt seinen
        // initialen Stand als Vorstand mit → vollständig, ohne N manuelle Freigaben.
        let nach = zusammenstellung_fuer_produkt(
            &dir,
            &[
                wahl("elektronik", "freigabe/elektronik/v0.1", true),
                wahl("firmware", "freigabe/firmware/v0.1", true),
            ],
            &[],
        );
        assert!(nach.vollstaendig, "nach dem Seed: erste Produkt-Revision komponierbar");
        assert!(nach.ausstehende.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    /// **AC: der Seed lässt schon revidierte Bausteine in Ruhe und sät optionale nie** (E52b). Cold-
    /// Start mit gemischtem Stand: elektronik trägt **schon** einen Freigabe-Stand, doku ist
    /// **optional**, firmware ist Pflicht **ohne** Stand → genau **firmware** wird gesät, sonst nichts.
    #[test]
    fn seed_laesst_schon_revidierte_und_optionale_in_ruhe() {
        let dir = tmp();
        git_init(&dir);
        write_stack(
            &dir,
            &stack_mit(vec![
                baustein("kicad", "elektronik"),
                baustein("zephyr", "firmware"),
                baustein("doku", "doku"),
            ]),
        )
        .unwrap();
        // elektronik ist schon einmal freigegeben (trägt einen Stand).
        git(&dir, &["tag", "freigabe/elektronik/Rev-A"]);

        let gesaet = seede_initiale_revisionen(&dir, "v0.1", &["doku".to_string()]).unwrap();
        let heimaten: Vec<&str> = gesaet.iter().map(|g| g.heimat.as_str()).collect();
        assert_eq!(heimaten, vec!["firmware"], "nur der Pflicht-Bereich ohne Stand wird gesät");

        // elektronik behält genau seinen alten Stand (kein zweiter initialer Tag), doku bleibt leer.
        assert_eq!(list_baustein_freigaben(&dir, "elektronik"), vec!["freigabe/elektronik/Rev-A"]);
        assert!(list_baustein_freigaben(&dir, "doku").is_empty(), "optionaler Bereich wird nie gesät");
        assert_eq!(list_baustein_freigaben(&dir, "firmware"), vec!["freigabe/firmware/v0.1"]);

        let _ = fs::remove_dir_all(&dir);
    }

    /// **AC: sind alle Pflicht-Bausteine schon revidiert, sät der Akt nichts** (E52b) — der Cold-
    /// Start ist überstanden, ein erneuter Aufruf ist ein No-op (leere Liste), nie ein Fehler. Das
    /// macht den Akt idempotent: zweimal säen säet beim zweiten Mal nichts mehr.
    #[test]
    fn seed_ist_no_op_wenn_alle_schon_revidiert() {
        let dir = tmp();
        git_init(&dir);
        write_stack(&dir, &stack_mit(vec![baustein("zephyr", "firmware")])).unwrap();

        // Erster Akt sät firmware; ein zweiter findet nichts mehr (firmware trägt nun einen Stand).
        let erst = seede_initiale_revisionen(&dir, "v0.1", &[]).unwrap();
        assert_eq!(erst.len(), 1);
        let zweit = seede_initiale_revisionen(&dir, "v0.2", &[]).unwrap();
        assert!(zweit.is_empty(), "alle Pflicht-Bausteine tragen schon einen Stand → nichts zu säen");

        let _ = fs::remove_dir_all(&dir);
    }

    /// **Degradation** (E22): ein leeres Produkt (kein Stack) sät nichts und ist kein Fehler.
    #[test]
    fn leeres_produkt_seedet_nichts() {
        let dir = tmp();
        git_init(&dir);
        let gesaet = seede_initiale_revisionen(&dir, "v0.1", &[]).unwrap();
        assert!(gesaet.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
