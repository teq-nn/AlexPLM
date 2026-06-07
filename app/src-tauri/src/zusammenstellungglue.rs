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

use crate::graphread::list_baustein_freigaben;
use crate::stackstore::read_stack;
use crate::zusammenstellung::{
    zusammenstellen, Auswahl, BausteinEintrag, ZusammenstellungsBericht,
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
    // Auswahl + Optional-Set für schnellen Zugriff nach Heimat indizieren.
    let auswahl_je_heimat: BTreeMap<&str, &WahlEingabe> =
        wahlen.iter().map(|w| (w.heimat.as_str(), w)).collect();
    let optional_set: std::collections::BTreeSet<&str> =
        optionale_heimaten.iter().map(String::as_str).collect();

    let stack = read_stack(root);
    let eintraege: Vec<BausteinEintrag> = stack
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
        .collect();

    zusammenstellen(&eintraege)
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
}
