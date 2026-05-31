//! Werkbank-Glue (Issue #47) — aus erfassten Dateien werden Artefakt-Karten + Unzugeordnet-Fach.
//!
//! Dünne, seiteneffekt-behaftete Schicht über dem reinen Kern [`crate::zuordnung`]. Sie
//! 1. liest den **Produkt-Stack** (`_plm/stack.json`, ADR 0003) als Glob-Satz,
//! 2. listet die **erfassten** Dateien des Produkts (das eine git-nahe I/O, hier gekapselt),
//! 3. lässt den reinen Kern jede Datei zuordnen und
//! 4. faltet das Ergebnis zu **Artefakt-Karten** (mit Hauptdatei + abgeleiteter primärer Aktion)
//!    und einem **Unzugeordnet-Fach pro Arbeitsbereich** (die Waisen).
//!
//! Das Falten selbst ([`build_werkbank`]) ist **rein** und tabellengetestet; nur das Sammeln der
//! Dateien ([`read_werkbank`]) fasst die Platte/Git an. Gleicher Schnitt wie `graphread.rs` über
//! `graph.rs`.

use crate::baustein::Oeffnen;
use crate::stackstore::read_stack;
use crate::zuordnung::{
    artefakt_key, primaer_aktion, zuordnen, BausteinRegel, OeffnenKonfig, PrimaerAktion, Zuordnung,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Eine **Artefakt-Karte** auf der Werkbank: per Konvention aus erfassten Dateien gebildet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtefaktKarte {
    /// Stabiler Schlüssel `"<baustein-id>:<ordner>"` — die UI keyt Karten darauf.
    pub artefakt_id: String,
    /// Menschlicher Baustein-Name (z.B. „KiCad"), trägt das Karten-Label.
    pub baustein: String,
    /// Ordner des Artefakts relativ zur Produktwurzel (Vorwärts-Slashes), die Heimat-Spur.
    pub ordner: String,
    /// Die **Hauptdatei** (höchste Glob-Priorität), produkt-relativ; `None` falls keine
    /// (degeneriert — eine Karte hat normalerweise immer mindestens eine Datei).
    pub hauptdatei: Option<String>,
    /// Alle erfassten Dateien dieses Artefakts, produkt-relativ, sortiert.
    pub dateien: Vec<String>,
    /// Die **abgeleitete primäre Aktion** des Ein-Klick-Öffnens (Datei vs. Ordner, PRD §14).
    pub primaer: PrimaerAktion,
    /// Das Ziel der primären Aktion, **absolut** auf der Platte (Datei- oder Ordnerpfad), damit
    /// die UI es direkt im OS-Standardprogramm öffnen kann. `None` nur im degenerierten Fall.
    pub ziel: Option<String>,
    /// Der **abgeleitete Karten-Status** + Stale-Flag (Issue #53, E26): live aus Git (Status)
    /// und Kanten (Stale) berechnet, nie gespeichert. Vom Falten zunächst auf den ruhigen
    /// Default gesetzt; die Glue [`read_werkbank`] füllt ihn aus echten Git-/Kanten-Fakten.
    pub projektion: crate::kartenstatus::KartenProjektion,
}

/// Ein **Unzugeordnet-Fach pro Arbeitsbereich**: die Waisen eines Arbeitsbereichs (oberster
/// Ordner). Eine Waise ist eine erfasste Datei ohne Etikett — nichts geht verloren, der Ordner
/// bleibt als Zuordnungs-Hinweis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnzugeordnetFach {
    /// Der Arbeitsbereich (oberster Ordnername; `""` = Produktwurzel).
    pub arbeitsbereich: String,
    /// Die Waisen-Dateien, produkt-relativ, sortiert.
    pub dateien: Vec<String>,
}

/// Die ganze Werkbank-Sicht eines Produkts: Karten + Unzugeordnet-Fächer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct WerkbankView {
    /// Artefakt-Karten, sortiert nach `artefakt_id`.
    pub karten: Vec<ArtefaktKarte>,
    /// Unzugeordnet-Fächer, eines je Arbeitsbereich mit Waisen, sortiert nach `arbeitsbereich`.
    pub unzugeordnet: Vec<UnzugeordnetFach>,
}

/// Spiegelt `baustein::Oeffnen` auf den vom Kern verstandenen [`OeffnenKonfig`]. Reine Abbildung.
fn oeffnen_konfig(o: Oeffnen) -> OeffnenKonfig {
    match o {
        Oeffnen::Auto => OeffnenKonfig::Auto,
        Oeffnen::Datei => OeffnenKonfig::Datei,
        Oeffnen::Ordner => OeffnenKonfig::Ordner,
    }
}

/// Der Ordner-Anteil eines produkt-relativen Pfads (alles vor dem letzten `/`); `""` an der Wurzel.
/// Für den Artefakt-Schlüssel einer manuellen Zuordnung (die Heimat-Grenze wird dabei ignoriert).
fn ordner_of(rel: &str) -> String {
    match rel.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// Der oberste Ordner eines produkt-relativen Pfads = der **Arbeitsbereich**. `""` = Wurzel.
fn arbeitsbereich_of(rel: &str) -> String {
    match rel.split_once('/') {
        Some((top, _)) => top.to_string(),
        None => String::new(),
    }
}

/// Den absoluten Pfad eines produkt-relativen Pfads bilden (Vorwärts-Slashes → OS-Trennzeichen
/// übernimmt der Pfad-Join). Leerer relativer Ordner = die Produktwurzel selbst.
fn absolutize(root: &Path, rel: &str) -> String {
    let joined = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    joined.to_string_lossy().into_owned()
}

/// Was der reine Kern über **einen** Baustein zum Falten braucht: seine Regel + Öffnen-Konfig.
/// (Die Öffnen-Wahl ist Karten-Sache, nicht Zuordnungs-Sache, darum hier getrennt geführt.)
#[derive(Debug, Clone)]
pub struct StackRegel {
    pub regel: BausteinRegel,
    pub oeffnen: OeffnenKonfig,
}

/// Priorität einer **manuellen** Zuordnung (Override). Bewusst die niedrigste mögliche Priorität,
/// damit in einer gemischten Karte ein echter Glob-Treffer (kleiner Index) die **Hauptdatei**
/// bleibt; eine rein manuell zugeordnete Einzeldatei ist dann selbst die Hauptdatei (dominant).
const OVERRIDE_PRIO: usize = usize::MAX;

/// **Reiner Kern des Glue**: falte erfasste Dateien + Glob-Satz zu Karten + Unzugeordnet-Fächern.
/// Ohne manuelle Zuordnungen — siehe [`build_werkbank_with_overrides`].
pub fn build_werkbank(root: &Path, tracked: &[String], stack: &[StackRegel]) -> WerkbankView {
    build_werkbank_with_overrides(root, tracked, stack, &BTreeMap::new())
}

/// Wie [`build_werkbank`], aber mit **manuellen Zuordnungen** (`overrides`: Pfad → Baustein-id,
/// aus `_plm/zuordnung.json`). Eine Override **gewinnt** über die Glob/Heimat-Konvention und
/// ignoriert die Heimat-Grenze — der Nutzer darf jede Datei jedem (nicht stillgelegten, im Stack
/// vorhandenen) Baustein zuordnen. Zeigt eine Override auf einen unbekannten/stillgelegten
/// Baustein, fällt die Datei sauber auf die Konvention zurück. Total und deterministisch; kein I/O.
pub fn build_werkbank_with_overrides(
    root: &Path,
    tracked: &[String],
    stack: &[StackRegel],
    overrides: &BTreeMap<String, String>,
) -> WerkbankView {
    let regeln: Vec<BausteinRegel> = stack.iter().map(|s| s.regel.clone()).collect();

    // Artefakt-Schlüssel -> (Baustein-Index, Ordner, Dateien mit ihrer Hauptdatei-Priorität).
    struct Acc {
        baustein_idx: usize,
        ordner: String,
        // (datei, prioritaet) — kleinste Priorität ist die Hauptdatei.
        dateien: Vec<(String, usize)>,
    }
    let mut artefakte: BTreeMap<String, Acc> = BTreeMap::new();
    // Arbeitsbereich -> Waisen.
    let mut waisen: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for path in tracked {
        // Eine manuelle Zuordnung gewinnt — sofern sie auf einen vorhandenen, nicht stillgelegten
        // Baustein zeigt. Sie ergibt (Artefakt-Schlüssel, niedrigste Priorität) und ignoriert die
        // Heimat-Grenze (der Ordner stammt aus dem Datei-Pfad selbst).
        let override_hit = overrides.get(path).and_then(|bid| {
            regeln
                .iter()
                .find(|r| &r.id == bid && !r.stillgelegt)
                .map(|r| {
                    let ordner = ordner_of(path);
                    (artefakt_key(&r.id, &ordner), OVERRIDE_PRIO)
                })
        });

        // Sonst die Konvention (Glob + Heimat).
        let zuordnung = override_hit.or_else(|| match zuordnen(path, &regeln) {
            Zuordnung::Artefakt { artefakt_id, prioritaet, .. } => Some((artefakt_id, prioritaet)),
            Zuordnung::Waise { .. } => None,
        });

        match zuordnung {
            Some((artefakt_id, prioritaet)) => {
                // Den Baustein-Index zum Schlüssel finden (erste Regel, deren key matcht).
                let ordner = artefakt_id
                    .split_once(':')
                    .map(|(_, o)| o.to_string())
                    .unwrap_or_default();
                let baustein_idx = regeln
                    .iter()
                    .position(|r| artefakt_key(&r.id, &ordner) == artefakt_id)
                    .unwrap_or(0);
                let acc = artefakte.entry(artefakt_id.clone()).or_insert_with(|| Acc {
                    baustein_idx,
                    ordner: ordner.clone(),
                    dateien: Vec::new(),
                });
                acc.dateien.push((path.clone(), prioritaet));
            }
            None => {
                // Arbeitsbereich = top-level folder of the file itself (root file -> "").
                let bereich = arbeitsbereich_of(path);
                waisen.entry(bereich).or_default().push(path.clone());
            }
        }
    }

    let mut karten: Vec<ArtefaktKarte> = artefakte
        .into_iter()
        .map(|(artefakt_id, mut acc)| {
            acc.dateien.sort_by(|a, b| a.0.cmp(&b.0));
            // Hauptdatei = kleinste Priorität; bei Gleichstand alphabetisch erster (stabil).
            let best_prio = acc.dateien.iter().map(|(_, p)| *p).min();
            let mut hauptkandidaten: Vec<&String> = acc
                .dateien
                .iter()
                .filter(|(_, p)| Some(*p) == best_prio)
                .map(|(f, _)| f)
                .collect();
            hauptkandidaten.sort();
            let hauptdatei = hauptkandidaten.first().map(|s| (*s).clone());
            // Dominante Einzeldatei: genau **eine** Datei trägt die höchste Priorität.
            let has_dominant = hauptkandidaten.len() == 1;

            let stackregel = stack.get(acc.baustein_idx);
            let baustein = stackregel
                .map(|s| s.regel.name.clone())
                .unwrap_or_else(|| acc.ordner.clone());
            let konfig = stackregel.map(|s| s.oeffnen).unwrap_or(OeffnenKonfig::Auto);
            let primaer = primaer_aktion(konfig, has_dominant);

            let ziel = match primaer {
                PrimaerAktion::Datei => hauptdatei.as_ref().map(|f| absolutize(root, f)),
                PrimaerAktion::Ordner => Some(absolutize(root, &acc.ordner)),
            };

            let dateien = acc.dateien.into_iter().map(|(f, _)| f).collect();
            ArtefaktKarte {
                artefakt_id,
                baustein,
                ordner: acc.ordner,
                hauptdatei,
                dateien,
                primaer,
                ziel,
                // Ruhiger Default; die Glue ([`enrich_status`]) füllt Status + Stale aus Git/Kanten.
                projektion: crate::kartenstatus::KartenProjektion::default(),
            }
        })
        .collect();
    karten.sort_by(|a, b| a.artefakt_id.cmp(&b.artefakt_id));

    let unzugeordnet = waisen
        .into_iter()
        .map(|(arbeitsbereich, mut dateien)| {
            dateien.sort();
            UnzugeordnetFach { arbeitsbereich, dateien }
        })
        .collect();

    WerkbankView { karten, unzugeordnet }
}

/// **Reiner Kern (Issue #53)**: reichere jede Karte einer [`WerkbankView`] um ihre
/// [`KartenProjektion`] an — gefalteter Git-Status (E26) + Stale-Flag (E26/E40). `git_states`
/// ist die produkt-relative Pfad→[`GitFileState`]-Karte (Default `Vorhanden` für unbekannte,
/// also ruhig erfasste Dateien); `stale_derived` ist die Menge der `derived`-Pfade aller fired
/// Stale-Warnungen aus [`crate::edges::stale_warnings`]. Total, deterministisch, kein I/O —
/// dieselbe „lies zurück statt spiegeln"-Haltung wie der ganze Karten-Status (E26).
pub fn enrich_status(
    mut view: WerkbankView,
    git_states: &BTreeMap<String, crate::kartenstatus::GitFileState>,
    stale_derived: &[String],
) -> WerkbankView {
    use crate::kartenstatus::{derive_karten_projektion, GitFileState};
    for karte in &mut view.karten {
        // Unbekannte Dateien sind erfasst und ruhig (Vorhanden) — nie lauter als gewusst (E26).
        let file_states: Vec<GitFileState> = karte
            .dateien
            .iter()
            .map(|d| git_states.get(d).copied().unwrap_or(GitFileState::Vorhanden))
            .collect();
        karte.projektion = derive_karten_projektion(&file_states, &karte.dateien, stale_derived);
    }
    view
}

/// Den Glob-Satz aus dem Produkt-Stack ziehen (ADR 0003): je kopiertem Baustein eine Regel + seine
/// Öffnen-Konfig. Rein abgeleitet aus dem gelesenen Stack.
pub fn stack_regeln(stack: &crate::stackstore::ProduktStack) -> Vec<StackRegel> {
    stack
        .bausteine
        .iter()
        .map(|sb| StackRegel {
            regel: BausteinRegel {
                id: sb.baustein.id.clone(),
                name: sb.baustein.name.clone(),
                heimat: sb.baustein.heimat.clone(),
                globs: sb.baustein.globs.clone(),
                stillgelegt: sb.baustein.stillgelegt,
            },
            oeffnen: oeffnen_konfig(sb.baustein.oeffnen),
        })
        .collect()
}

/// **Glue**: lies den Produkt-Stack + die erfassten Dateien und falte sie zur Werkbank-Sicht.
/// Das einzige I/O hier; die Entscheidung lebt in [`build_werkbank`]. Ein Produkt ohne Stack hat
/// keinen Glob-Satz → alles wird zur Waise (nichts geht verloren), nie ein Fehler.
pub fn read_werkbank(root: &Path) -> std::io::Result<WerkbankView> {
    let stack = read_stack(root);
    let regeln = stack_regeln(&stack);
    let tracked = list_tracked_files(root)?;
    let overrides = crate::zuordnungstore::read_overrides(root);
    let view = build_werkbank_with_overrides(root, &tracked, &regeln, &overrides);

    // Issue #53: den abgeleiteten Karten-Status + Stale live auflegen — aus Git (Datei-Zustand)
    // und Kanten (Stale, E26/E40). Beides nur gelesen, nie gespeichert (E26).
    let git_states = read_git_states(root)?;
    let stale_derived: Vec<String> = crate::edgestore::read_edge_view(root)
        .warnings
        .into_iter()
        .map(|w| w.derived)
        .collect();
    Ok(enrich_status(view, &git_states, &stale_derived))
}

/// Den **Git-Zustand je erfasster/ignorierter Datei** lesen (`git status --porcelain --ignored`),
/// produkt-relativ mit Vorwärts-Slashes → [`crate::kartenstatus::GitFileState`]. Das `_plm/`-
/// Werkzeugverzeichnis (ADR 0002) wird ausgeklammert. Reine Lese-Glue; die Ableitung lebt im
/// Kern [`crate::kartenstatus`]. Ohne git / sauberes Repo: leere Karte (jede Datei dann ruhig
/// `Vorhanden`), nie ein Fehler.
pub fn read_git_states(
    root: &Path,
) -> std::io::Result<BTreeMap<String, crate::kartenstatus::GitFileState>> {
    use crate::kartenstatus::GitFileState;
    let out = crate::gitrunner::command(root)
        .args(["status", "--porcelain", "--ignored", "-z"])
        .output()?;
    let mut states: BTreeMap<String, GitFileState> = BTreeMap::new();
    if !out.status.success() {
        return Ok(states);
    }
    // `-z`-Records sind NUL-getrennt; ein Rename-Record hängt den alten Pfad als eigenen
    // NUL-getrennten Eintrag an. Jeder Record beginnt mit dem 2-Zeichen-XY-Code, dann ein
    // Leerzeichen, dann der Pfad. Wir lesen XY + Pfad; etwaige Rename-Altpfade ignorieren wir.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut records = stdout.split('\0').peekable();
    while let Some(record) = records.next() {
        if record.len() < 3 {
            continue;
        }
        let code = &record[..2];
        let path = record[3..].replace('\\', "/");
        // Rename/Copy: der nächste NUL-Record ist der Quellpfad — überspringen.
        if code.starts_with('R') || code.starts_with('C') {
            records.next();
        }
        if path == crate::stackstore::PLM_DIR
            || path.starts_with(&format!("{}/", crate::stackstore::PLM_DIR))
        {
            continue;
        }
        states.insert(path, GitFileState::from_porcelain(code));
    }
    Ok(states)
}

/// Die erfassten Dateien des Produkts, produkt-relativ mit Vorwärts-Slashes. Das committete
/// `_plm/`-Werkzeugverzeichnis (ADR 0002) wird ausgeklammert — es ist Tool-Buchhaltung, kein
/// Artefakt. Ein Ordner ohne git oder ohne erfasste Dateien liefert eine leere Liste (nie Fehler).
pub fn list_tracked_files(root: &Path) -> std::io::Result<Vec<String>> {
    let out = crate::gitrunner::command(root)
        .args(["ls-files", "-z"])
        .output()?;
    if !out.status.success() {
        // Kein git / keine erfassten Dateien: leere Werkbank, kein Fehler (degradiert sauber).
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut files: Vec<String> = stdout
        .split('\0')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\\', "/"))
        .filter(|s| s.as_str() != crate::stackstore::PLM_DIR && !s.starts_with(&format!("{}/", crate::stackstore::PLM_DIR)))
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sr(id: &str, name: &str, heimat: &str, globs: &[&str], oeffnen: OeffnenKonfig) -> StackRegel {
        StackRegel {
            regel: BausteinRegel {
                id: id.to_string(),
                name: name.to_string(),
                heimat: heimat.to_string(),
                globs: globs.iter().map(|s| s.to_string()).collect(),
                stillgelegt: false,
            },
            oeffnen,
        }
    }

    fn stack() -> Vec<StackRegel> {
        vec![
            sr("kicad", "KiCad", "elektronik", &["*.kicad_pro", "*.kicad_sch", "*.kicad_pcb"], OeffnenKonfig::Auto),
            sr("fusion", "Fusion 360", "mechanik", &["*.f3d", "*.step", "*.stl"], OeffnenKonfig::Auto),
            sr("doku", "Doku", "doku", &["*.md", "*.pdf"], OeffnenKonfig::Auto),
            sr("platformio", "PlatformIO", "firmware", &["platformio.ini", "*.c", "*.h"], OeffnenKonfig::Ordner),
        ]
    }

    fn root() -> &'static Path {
        Path::new("/produkt")
    }

    #[test]
    fn groups_files_into_artifact_cards_with_hauptdatei_and_action() {
        let tracked: Vec<String> = [
            "elektronik/regler/regler.kicad_pro",
            "elektronik/regler/regler.kicad_sch",
            "elektronik/regler/regler.kicad_pcb",
            "doku/handbuch.md",
            "firmware/platformio.ini",
            "firmware/src/main.c",
            "elektronik/regler/notizen.txt", // Waise
            "README.md",                     // Waise (root, not in doku/)
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let view = build_werkbank(root(), &tracked, &stack());

        // The kicad artifact groups its three files; Hauptdatei is the *.kicad_pro (priority 0).
        let kicad = view
            .karten
            .iter()
            .find(|k| k.artefakt_id == "kicad:elektronik/regler")
            .expect("kicad card");
        assert_eq!(kicad.baustein, "KiCad");
        assert_eq!(kicad.hauptdatei.as_deref(), Some("elektronik/regler/regler.kicad_pro"));
        assert_eq!(kicad.dateien.len(), 3);
        // Auto + dominant single Hauptdatei -> open the FILE.
        assert_eq!(kicad.primaer, PrimaerAktion::Datei);
        assert_eq!(
            kicad.ziel.as_deref(),
            Some("/produkt/elektronik/regler/regler.kicad_pro")
        );

        // doku card: one md, dominant -> file.
        let doku = view.karten.iter().find(|k| k.artefakt_id == "doku:doku").unwrap();
        assert_eq!(doku.primaer, PrimaerAktion::Datei);

        // platformio card is configured Ordner -> opens the folder regardless of dominance.
        // platformio.ini sits in firmware/ (Hauptdatei), main.c sits in firmware/src/ -> two cards.
        let pio_root = view.karten.iter().find(|k| k.artefakt_id == "platformio:firmware").unwrap();
        assert_eq!(pio_root.primaer, PrimaerAktion::Ordner);
        assert_eq!(pio_root.ziel.as_deref(), Some("/produkt/firmware"));
    }

    #[test]
    fn non_dominant_artifact_opens_the_folder_under_auto() {
        // Two equal-priority Hauptdatei hits (two *.kicad_pro) -> no dominant single file -> Ordner.
        let tracked: Vec<String> = [
            "elektronik/regler/a.kicad_pro",
            "elektronik/regler/b.kicad_pro",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let view = build_werkbank(root(), &tracked, &stack());
        let card = &view.karten[0];
        assert_eq!(card.primaer, PrimaerAktion::Ordner);
        assert_eq!(card.ziel.as_deref(), Some("/produkt/elektronik/regler"));
    }

    #[test]
    fn unassigned_files_land_in_an_unzugeordnet_fach_per_arbeitsbereich() {
        let tracked: Vec<String> = [
            "elektronik/regler/notizen.txt", // Arbeitsbereich elektronik
            "elektronik/scratch.log",        // Arbeitsbereich elektronik
            "mechanik/render.png",           // Arbeitsbereich mechanik
            "bom.csv",                       // root Arbeitsbereich ""
            "doku/handbuch.md",              // assigned, NOT a Waise
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let view = build_werkbank(root(), &tracked, &stack());

        // doku/handbuch.md is an artifact, the rest are Waisen grouped per Arbeitsbereich.
        assert!(view.karten.iter().any(|k| k.artefakt_id == "doku:doku"));

        let by = |b: &str| view.unzugeordnet.iter().find(|f| f.arbeitsbereich == b);
        assert_eq!(by("elektronik").unwrap().dateien, vec![
            "elektronik/regler/notizen.txt",
            "elektronik/scratch.log"
        ]);
        assert_eq!(by("mechanik").unwrap().dateien, vec!["mechanik/render.png"]);
        assert_eq!(by("").unwrap().dateien, vec!["bom.csv"]);
        // doku has no Waisen -> no Fach for it.
        assert!(by("doku").is_none());
    }

    #[test]
    fn manual_override_makes_a_card_across_heimat_and_wins_over_convention() {
        // hardware/teil.FCStd matches no glob and sits outside every Heimat -> normally a Waise.
        // A manual override to "fusion" must turn it into a fusion card (Heimat ignored).
        let tracked: Vec<String> = ["hardware/teil.FCStd", "hardware/render.png"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut overrides = BTreeMap::new();
        overrides.insert("hardware/teil.FCStd".to_string(), "fusion".to_string());

        let view = build_werkbank_with_overrides(root(), &tracked, &stack(), &overrides);

        let fusion = view
            .karten
            .iter()
            .find(|k| k.artefakt_id == "fusion:hardware")
            .expect("manual fusion card in hardware/");
        assert_eq!(fusion.baustein, "Fusion 360");
        assert_eq!(fusion.dateien, vec!["hardware/teil.FCStd"]);
        // Single file -> dominant -> opens the file.
        assert_eq!(fusion.primaer, PrimaerAktion::Datei);
        assert_eq!(fusion.ziel.as_deref(), Some("/produkt/hardware/teil.FCStd"));

        // The un-assigned render.png stays a Waise in the hardware Arbeitsbereich.
        let fach = view.unzugeordnet.iter().find(|f| f.arbeitsbereich == "hardware").unwrap();
        assert_eq!(fach.dateien, vec!["hardware/render.png"]);
    }

    #[test]
    fn override_to_unknown_baustein_falls_back_to_convention() {
        // Override points at a Baustein not in the stack -> ignored, file stays a Waise.
        let tracked = vec!["hardware/teil.FCStd".to_string()];
        let mut overrides = BTreeMap::new();
        overrides.insert("hardware/teil.FCStd".to_string(), "ghost".to_string());
        let view = build_werkbank_with_overrides(root(), &tracked, &stack(), &overrides);
        assert!(view.karten.is_empty());
        assert_eq!(view.unzugeordnet[0].dateien, vec!["hardware/teil.FCStd"]);
    }

    #[test]
    fn glob_hit_stays_hauptdatei_when_mixed_with_an_override() {
        // A real glob hit (regler.kicad_pro, prio 0) plus a manually added README in the same folder.
        let tracked: Vec<String> = ["elektronik/regler/regler.kicad_pro", "elektronik/regler/README"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut overrides = BTreeMap::new();
        overrides.insert("elektronik/regler/README".to_string(), "kicad".to_string());
        let view = build_werkbank_with_overrides(root(), &tracked, &stack(), &overrides);
        let kicad = view.karten.iter().find(|k| k.artefakt_id == "kicad:elektronik/regler").unwrap();
        assert_eq!(kicad.dateien.len(), 2);
        // The real glob hit remains the Hauptdatei; the override file is secondary.
        assert_eq!(kicad.hauptdatei.as_deref(), Some("elektronik/regler/regler.kicad_pro"));
    }

    #[test]
    fn empty_stack_makes_everything_a_waise_nothing_lost() {
        let tracked: Vec<String> = ["elektronik/x.kicad_pro", "doku/y.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let view = build_werkbank(root(), &tracked, &[]);
        assert!(view.karten.is_empty());
        let total: usize = view.unzugeordnet.iter().map(|f| f.dateien.len()).sum();
        assert_eq!(total, tracked.len(), "no tracked file is ever dropped");
    }

    #[test]
    fn cards_and_faecher_are_sorted_deterministically() {
        let tracked: Vec<String> = [
            "mechanik/g/g.f3d",
            "elektronik/r/r.kicad_pro",
            "zzz/note.txt",
            "aaa/note.txt",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let view = build_werkbank(root(), &tracked, &stack());
        let keys: Vec<&str> = view.karten.iter().map(|k| k.artefakt_id.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        let bereiche: Vec<&str> = view.unzugeordnet.iter().map(|f| f.arbeitsbereich.as_str()).collect();
        let mut sb = bereiche.clone();
        sb.sort();
        assert_eq!(bereiche, sb);
    }

    /// Issue #53: `enrich_status` attaches the derived Karten-Status (folded from per-file Git
    /// states, E26) and the orthogonal Stale flag (from edges, E26/E40) onto each card. A card
    /// with no Git facts stays the ruhige `Vorhanden`; a louder file makes the whole card loud;
    /// stale rides alongside and needs an edge (no edge → not stale).
    #[test]
    fn enrich_status_folds_git_and_stale_onto_each_card() {
        use crate::kartenstatus::{GitFileState, KartenStatus};
        let tracked: Vec<String> = [
            "elektronik/regler/regler.kicad_pro",
            "elektronik/regler/regler.kicad_pcb",
            "mechanik/g/g.f3d",
            "doku/handbuch.md",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let view = build_werkbank(root(), &tracked, &stack());

        // One kicad file is geändert → the kicad card is loud; the .f3d is gone → mechanik fehlt;
        // doku has no git fact → ruhig Vorhanden. mechanik's g.f3d is the derived end of an edge.
        let mut git = BTreeMap::new();
        git.insert("elektronik/regler/regler.kicad_pcb".to_string(), GitFileState::Geaendert);
        git.insert("mechanik/g/g.f3d".to_string(), GitFileState::Fehlt);
        let stale_derived = vec!["mechanik/g/g.f3d".to_string()];

        let enriched = enrich_status(view, &git, &stale_derived);
        let karte = |id_prefix: &str| {
            enriched
                .karten
                .iter()
                .find(|k| k.artefakt_id.starts_with(id_prefix))
                .unwrap_or_else(|| panic!("no card {id_prefix}"))
        };

        // kicad: one geänderte Datei makes the whole card laut; no edge → not stale.
        assert_eq!(karte("kicad:").projektion.status, KartenStatus::Geaendert);
        assert!(!karte("kicad:").projektion.stale);

        // mechanik: file fehlt → Fehlt; AND it sits at the derived end of an edge → stale.
        assert_eq!(karte("fusion:").projektion.status, KartenStatus::Fehlt);
        assert!(karte("fusion:").projektion.stale, "edge present + flagged → stale");

        // doku: no git fact at all → ruhig Vorhanden, not stale.
        assert_eq!(karte("doku:").projektion.status, KartenStatus::Vorhanden);
        assert!(!karte("doku:").projektion.stale);
    }
}
