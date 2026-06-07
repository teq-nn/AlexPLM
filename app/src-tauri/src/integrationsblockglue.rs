//! Dünne Glue für die **Integrations-Aufgabe** (Issue #141, E53): Flaggen / Beantworten /
//! Protokollieren + die Block-Auflösung **an der Compose**.
//!
//! Spiegelt den Haus-Split (`taskstore.rs` über `tasks.rs`, `aufgabenblockglue.rs` über
//! `aufgabenblock.rs`): die **Entscheidung** lebt nie hier — sie lebt im reinen
//! [`crate::integrationsblock`]-Kern. Diese Schicht **liest/schreibt** den Akten-Beleg im
//! `_plm`-Speicher (der einmalige, gegen eine Quell-Revision erhobene Integrations-Beleg) und
//! reicht den Schnappschuss zusammen mit der **Compose-Auswahl** an [`entscheide_integrationsblock`].
//!
//! Drei Gesten, alle dünn:
//!
//! - **Flaggen** (HW): [`flagge_integration`] legt eine neue, **offene** Forderung an — „mein
//!   `quell_baustein` braucht gegen `ziel_baustein` einen Test, erhoben gegen `quell_rev`".
//! - **Beantworten** (SW/Empfänger): [`beantworte_integration`] setzt die Antwort ja/nein; der
//!   Beleg liegt damit auf Akte.
//! - **Block an der Compose**: [`integrationsblock_fuer_compose`] liest die offenen Forderungen und
//!   entscheidet **nur hier** (mit der Compose-Auswahl) den Block + die Leseschein-Zeilen. Die
//!   eigenständige Baustein-/FW-Freigabe ruft das **nie** auf — so blockiert eine Integration nie
//!   eine Einzel-Freigabe (E53).
//!
//! Der Speicher ist **opt-in** (E22): ein Produkt ohne Beleg-Datei hat null Forderungen; eine
//! fehlende/leere/kaputte Datei liest als leere Liste — nie ein Fehler.

use crate::compose::StuecklistenPosten;
use crate::integrationsblock::{
    entscheide_integrationsblock, ComposeBausteinRev, IntegrationsAntwort, IntegrationsAufgabe,
    IntegrationsBlockEntscheid,
};
use crate::plmstore::PlmDocument;
use std::path::Path;
use std::time::SystemTime;

/// Datei, die die Integrations-Belege eines Produkts hält, in `_plm/` (ADR 0002). Getrennt von den
/// gewöhnlichen Aufgaben (`aufgaben.json`), denn eine Integrations-Aufgabe ist ein **Cross-Baustein**-
/// Beleg gegen eine Quell-Revision, kein Baustein-internes To-do.
pub const INTEGRATIONEN_FILE: &str = "integrationen.json";

/// Das `_plm/integrationen.json`-Dokument — Pfad, Degradation und atomarer Pretty-Write liegen in der
/// tiefen [`PlmDocument`]-Schicht; diese Glue vergibt ids und reicht den Schnappschuss an den Kern.
const INTEGRATIONEN: PlmDocument<Vec<IntegrationsAufgabe>> = PlmDocument::new(INTEGRATIONEN_FILE);

/// Die persistierten Integrations-Belege eines Produkts lesen. Fehlend/leer/kaputt ⇒ **null
/// Forderungen** (opt-in) — nie ein Fehler (E22).
pub fn read_integrationen(root: &Path) -> Vec<IntegrationsAufgabe> {
    INTEGRATIONEN.read(root)
}

/// Die Beleg-Liste schreiben (pretty + atomar, legt `_plm/` bei Bedarf an).
fn write_integrationen(root: &Path, liste: &[IntegrationsAufgabe]) -> std::io::Result<()> {
    INTEGRATIONEN.write(root, &liste.to_vec())
}

/// Eine stabile, undurchsichtige id für eine frische Forderung minten — wie [`crate::taskstore`]:
/// zeitbasiert + ein Zähler-Schwanz, damit zwei Forderungen in derselben Nanosekunde nie kollidieren.
fn mint_id(existing: &[IntegrationsAufgabe]) -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("integ{nanos}-{}", existing.len())
}

/// **Flaggen** (HW, E53): eine neue, **offene** Integrations-Forderung anlegen und protokollieren —
/// „mein `quell_baustein` braucht gegen `ziel_baustein` einen Test, erhoben gegen `quell_rev`". Die
/// Glue mintet die id; die Antwort startet **offen** (der Empfänger hat noch nicht geantwortet).
/// Gibt die frische Liste zurück, damit die UI in einem Roundtrip aktualisiert.
pub fn flagge_integration(
    root: &Path,
    quell_baustein: &str,
    ziel_baustein: &str,
    quell_rev: &str,
) -> std::io::Result<Vec<IntegrationsAufgabe>> {
    let mut liste = read_integrationen(root);
    let id = mint_id(&liste);
    liste.push(IntegrationsAufgabe {
        id,
        quell_baustein: quell_baustein.trim().to_string(),
        ziel_baustein: ziel_baustein.trim().to_string(),
        quell_rev: quell_rev.trim().to_string(),
        antwort: IntegrationsAntwort::Offen,
    });
    write_integrationen(root, &liste)?;
    Ok(liste)
}

/// **Beantworten** (SW/Empfänger, E53): die Antwort einer Forderung auf ja/nein setzen — der Beleg
/// liegt damit auf Akte. Eine fehlende id ist ein toleranter No-Op (die Liste kommt unverändert
/// zurück). Gibt die frische Liste zurück.
pub fn beantworte_integration(
    root: &Path,
    id: &str,
    antwort: IntegrationsAntwort,
) -> std::io::Result<Vec<IntegrationsAufgabe>> {
    let mut liste = read_integrationen(root);
    if let Some(a) = liste.iter_mut().find(|a| a.id == id) {
        a.antwort = antwort;
    }
    write_integrationen(root, &liste)?;
    Ok(liste)
}

/// Eine Forderung löschen (zurücknehmen). Fehlende id ⇒ No-Op. Gibt die frische Liste zurück.
pub fn loesche_integration(
    root: &Path,
    id: &str,
) -> std::io::Result<Vec<IntegrationsAufgabe>> {
    let mut liste = read_integrationen(root);
    liste.retain(|a| a.id != id);
    write_integrationen(root, &liste)?;
    Ok(liste)
}

/// Den **Integrations-Block an der Compose** entscheiden (Issue #141, E53): die offenen Forderungen
/// aus dem `_plm`-Speicher lesen und mit der **Compose-Auswahl** an den reinen Kern reichen. **Nur
/// hier** greift der Block — die eigenständige Baustein-/FW-Freigabe ruft diese Funktion nie auf.
///
/// `compose_auswahl` ist die Produkt-Stückliste der zu bauenden Revision ([`StuecklistenPosten`]),
/// aus der die Glue je Baustein die komponierte Revision ableitet ([`compose_revs_aus_bom`]). Eine
/// leere Auswahl heißt „keine Komposition" → der Kern blockiert nichts.
pub fn integrationsblock_fuer_compose(
    root: &Path,
    compose_auswahl: &[StuecklistenPosten],
) -> IntegrationsBlockEntscheid {
    let forderungen = read_integrationen(root);
    let compose = compose_revs_aus_bom(compose_auswahl);
    entscheide_integrationsblock(&forderungen, &compose)
}

/// Aus der **Produkt-Stückliste** (BOM) die Compose-Auswahl je Baustein ableiten, wie der
/// Integrations-Kern sie liest: pro Posten `(baustein_id, rev)`, wobei die `rev` das **Label** des
/// Release-Tags ist (das letzte Pfad-Segment von `freigabe/<heimat>/<label>`, E51a). So spricht der
/// Block dieselbe Quell-Revision wie der HW-Entwickler beim Flaggen („Rev D"), nicht den vollen
/// git-Tag-Pfad.
fn compose_revs_aus_bom(bom: &[StuecklistenPosten]) -> Vec<ComposeBausteinRev> {
    bom.iter()
        .map(|p| ComposeBausteinRev {
            baustein: p.baustein_id.clone(),
            rev: tag_label(&p.release_tag),
        })
        .collect()
}

/// Das **Label** eines Release-Tags — das letzte Pfad-Segment von `freigabe/<heimat>/<label>` (E51a).
/// Trägt der Tag keinen `/`, ist er selbst das Label. Rein/total.
fn tag_label(release_tag: &str) -> String {
    release_tag.rsplit('/').next().unwrap_or(release_tag).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-integrationsblock-ut-{}-{}",
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

    fn posten(baustein_id: &str, heimat: &str, release_tag: &str) -> StuecklistenPosten {
        StuecklistenPosten {
            baustein_id: baustein_id.to_string(),
            heimat: heimat.to_string(),
            release_tag: release_tag.to_string(),
        }
    }

    /// Ein Produkt ohne Beleg-Datei hat null Forderungen und ist nie blockiert (opt-in, E22).
    #[test]
    fn kein_speicher_blockiert_nie() {
        let dir = tmp();
        assert!(read_integrationen(&dir).is_empty());
        let d = integrationsblock_fuer_compose(&dir, &[posten("kicad", "elektronik", "freigabe/elektronik/Rev-D")]);
        assert!(!d.ist_blockiert());
        let _ = fs::remove_dir_all(&dir);
    }

    /// **AC End-to-End** (E53): HW flaggt → SW beantwortet → der Beleg liegt auf Akte, und der Block
    /// an der Compose folgt der Antwort. „nein" hält den Block, „ja" gegen genau die komponierte
    /// Quell-Rev hebt ihn — alles durch den realen `_plm/integrationen.json`-Speicher.
    #[test]
    fn flaggen_beantworten_protokoll_und_block_an_compose() {
        let dir = tmp();
        // HW flaggt: PCB (kicad) braucht gegen FW (zephyr) einen Test, erhoben gegen Rev-D — die
        // Quell-Rev ist das Label des dauerhaften Freigabe-Tags (E51a), gegen das auch die Compose liest.
        let liste = flagge_integration(&dir, "kicad", "zephyr", "Rev-D").unwrap();
        assert_eq!(liste.len(), 1);
        let id = liste[0].id.clone();
        assert_eq!(liste[0].antwort, IntegrationsAntwort::Offen, "frisch geflaggt ist offen");
        // Der Beleg liegt auf Akte (persistiert).
        assert!(INTEGRATIONEN.path(&dir).is_file(), "Beleg liegt in _plm/integrationen.json");

        // Die Compose nimmt genau Rev-D der PCB → offen ⇒ harter Block an der Compose.
        let compose = vec![posten("kicad", "elektronik", "freigabe/elektronik/Rev-D")];
        let offen = integrationsblock_fuer_compose(&dir, &compose);
        assert!(offen.ist_blockiert(), "eine offene Forderung blockiert die Compose");
        assert_eq!(offen.blockierende_ids, vec![id.clone()]);

        // SW (Empfänger) verneint → „nein" hält den Block.
        beantworte_integration(&dir, &id, IntegrationsAntwort::Nein).unwrap();
        assert!(integrationsblock_fuer_compose(&dir, &compose).ist_blockiert(), "„nein\" hält den Block");

        // SW bejaht → der Beleg deckt genau diese Kombination → kein Block mehr.
        beantworte_integration(&dir, &id, IntegrationsAntwort::Ja).unwrap();
        let belegt = integrationsblock_fuer_compose(&dir, &compose);
        assert!(!belegt.ist_blockiert(), "ein „ja\" gegen die komponierte Rev hebt den Block");
        // Der Leseschein bestätigt die belegte Kombination (blockiert aber nichts).
        assert_eq!(belegt.lesescheine.len(), 1);
        assert!(belegt.lesescheine[0].belegt);

        let _ = fs::remove_dir_all(&dir);
    }

    /// **AC Einmaligkeit / Re-Flaggen** (E53): ein „ja" gegen Rev D ist verbraucht, sobald die Compose
    /// Rev E nimmt — dann blockiert nichts (keine Forderung gegen Rev E), aber der passive Leseschein
    /// meldet die fehlende Kombination. Erst ein **neues Flaggen** gegen Rev E stellt den Block wieder her.
    #[test]
    fn beleg_ist_einmalig_und_muss_neu_geflaggt_werden() {
        let dir = tmp();
        let liste = flagge_integration(&dir, "kicad", "zephyr", "Rev-D").unwrap();
        beantworte_integration(&dir, &liste[0].id, IntegrationsAntwort::Ja).unwrap();

        // Compose nimmt nun Rev-E → das „ja" gegen Rev-D ist verbraucht → kein Block, aber ein
        // Leseschein für die ungetestete Kombination.
        let compose_e = vec![posten("kicad", "elektronik", "freigabe/elektronik/Rev-E")];
        let nach_rev_wechsel = integrationsblock_fuer_compose(&dir, &compose_e);
        assert!(!nach_rev_wechsel.ist_blockiert(), "verbrauchter Beleg fordert nichts für Rev-E");
        assert_eq!(nach_rev_wechsel.lesescheine.len(), 1);
        assert_eq!(
            nach_rev_wechsel.lesescheine[0].zuletzt_getestete_rev.as_deref(),
            Some("Rev-D")
        );
        assert!(!nach_rev_wechsel.lesescheine[0].belegt);

        // Re-Flaggen gegen Rev-E (HW erhebt die Forderung am neuen Quell-Stand neu) → wieder ein Block.
        flagge_integration(&dir, "kicad", "zephyr", "Rev-E").unwrap();
        assert!(
            integrationsblock_fuer_compose(&dir, &compose_e).ist_blockiert(),
            "die neu geflaggte (offene) Forderung gegen Rev E blockiert wieder"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// Das Release-Tag-Label wird korrekt aus dem dauerhaften Baustein-Freigabe-Tag gezogen (E51a):
    /// `freigabe/elektronik/Rev-D` → `Rev-D`, sodass der Block dieselbe Quell-Revision spricht wie
    /// der HW-Entwickler beim Flaggen.
    #[test]
    fn compose_revs_aus_bom_zieht_das_tag_label() {
        let bom = vec![
            posten("kicad", "elektronik", "freigabe/elektronik/Rev-D"),
            posten("zephyr", "firmware", "freigabe/firmware/v0.3"),
            posten("fusion", "mechanik", "nacktes-label"),
        ];
        let revs = compose_revs_aus_bom(&bom);
        assert_eq!(revs[0], ComposeBausteinRev { baustein: "kicad".into(), rev: "Rev-D".into() });
        assert_eq!(revs[1], ComposeBausteinRev { baustein: "zephyr".into(), rev: "v0.3".into() });
        assert_eq!(revs[2], ComposeBausteinRev { baustein: "fusion".into(), rev: "nacktes-label".into() });
    }

    /// Beantworten/Löschen auf eine fehlende id sind tolerante No-Ops (die Liste bleibt unverändert).
    #[test]
    fn beantworten_und_loeschen_sind_tolerant() {
        let dir = tmp();
        flagge_integration(&dir, "kicad", "zephyr", "Rev-D").unwrap();
        let vorher = read_integrationen(&dir);
        assert_eq!(beantworte_integration(&dir, "gibt-es-nicht", IntegrationsAntwort::Ja).unwrap(), vorher);
        assert_eq!(loesche_integration(&dir, "gibt-es-nicht").unwrap(), vorher);
        // Löschen der echten id leert die Liste.
        let id = vorher[0].id.clone();
        assert!(loesche_integration(&dir, &id).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
