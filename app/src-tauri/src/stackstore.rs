//! Produkt-Stack-Speicher — die Anti-Drift-Vollkopie in `_plm/stack.json` (Issue #39, ADR 0002/0003).
//!
//! Beim Anlegen/Konfigurieren eines Produkts wird der gewählte Toolstack als **vollständige,
//! selbsttragende Kopie** nach `<produkt>/_plm/stack.json` geschrieben. Jede Kopie trägt einen
//! Herkunfts-Stempel `{from: id, version}` — **nur** für Anzeige/„Update verfügbar", **kein**
//! Live-Link. Eine spätere Bibliotheks-Änderung erreicht das Produkt **nie** (harte Anti-Drift).
//!
//! Glue wie `edgestore.rs`: alles I/O hier, das Modell ist rein (`baustein.rs`). Lesemuster:
//! fehlende/leere/korrupte Datei ⇒ leerer Stack, nie Fehler; geschrieben wird pretty-printed JSON.

use crate::baustein::Baustein;
use crate::bibliothek::Bibliothek;
use crate::plmstore::PlmDocument;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Datei für den Produkt-Stack innerhalb von `_plm/`.
pub const STACK_FILE: &str = "stack.json";

/// Herkunfts-Stempel einer kopierten Baustein-Definition: aus welchem Bibliotheks-Baustein
/// (`from`) und in welcher `version` sie kopiert wurde. Nur Anzeige; kein Live-Link (ADR 0003).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Herkunft {
    /// Bibliotheks-`id`, aus der kopiert wurde.
    pub from: String,
    /// Version zum Zeitpunkt des Kopierens.
    pub version: u32,
}

/// Eine Baustein-Vollkopie im Produkt-Stack: die ganze Definition **plus** Herkunfts-Stempel.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackBaustein {
    /// Herkunfts-Stempel (Anzeige/„Update verfügbar"); kein Live-Link.
    pub herkunft: Herkunft,
    /// Die vollständige, selbsttragende Kopie der Baustein-Definition.
    #[serde(flatten)]
    pub baustein: Baustein,
}

impl StackBaustein {
    /// Eine selbsttragende Kopie eines Bibliotheks-Bausteins mit Herkunfts-Stempel.
    pub fn copy_of(b: &Baustein) -> Self {
        StackBaustein {
            herkunft: Herkunft { from: b.id.clone(), version: b.version },
            baustein: b.clone(),
        }
    }
}

/// Der Produkt-Stack: die kopierten Bausteine eines Produkts. Vollständig selbsttragend; das
/// Produkt funktioniert auch ohne Bibliothek (ADR 0003).
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProduktStack {
    /// Optionaler Herkunfts-Name des gewählten Toolstacks (Anzeige).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolstack: Option<String>,
    /// Die kopierten Bausteine (Vollkopie je + Stempel).
    pub bausteine: Vec<StackBaustein>,
}

/// Das `_plm/stack.json`-Dokument — Pfad, Degradation und pretty/atomares Schreiben liegen in der
/// tiefen [`PlmDocument`]-Schicht; hier liegt nur die Stack-Domänenlogik darüber.
const STACK: PlmDocument<ProduktStack> = PlmDocument::new(STACK_FILE);

/// Den Produkt-Stack lesen. Fehlende/leere/korrupte Datei ⇒ **leerer Stack**, nie Fehler.
pub fn read_stack(root: &Path) -> ProduktStack {
    STACK.read(root)
}

/// Den Produkt-Stack pretty-printed nach `_plm/stack.json` schreiben (legt `_plm/` an, atomar).
pub fn write_stack(root: &Path, stack: &ProduktStack) -> std::io::Result<()> {
    STACK.write(root, stack)
}

/// Einen Produkt-Stack aus gewählten Bibliotheks-Bausteinen als **Vollkopie** in das Produkt
/// schreiben (ADR 0003). Die Bausteine werden aus der `lib` gelesen und vollständig kopiert; eine
/// unbekannte `id` wird übersprungen (das Produkt trägt nur, was real existiert). `toolstack` ist
/// der Anzeige-Name des gewählten Stacks (optional). Gibt den geschriebenen Stack zurück.
pub fn create_product_stack(
    root: &Path,
    lib: &Bibliothek,
    baustein_ids: &[String],
    toolstack: Option<String>,
) -> std::io::Result<ProduktStack> {
    let bausteine = baustein_ids
        .iter()
        .filter_map(|id| lib.read_baustein(id))
        .map(|b| StackBaustein::copy_of(&b))
        .collect();
    let stack = ProduktStack { toolstack, bausteine };
    write_stack(root, &stack)?;
    Ok(stack)
}

/// Einen bestehenden Produkt-Stack **additiv** um weitere Bibliotheks-Bausteine erweitern (PRD §50:
/// „diesmal Zephyr statt PlatformIO" / „später ergänzen"). Anti-Drift bleibt gewahrt: bereits
/// kopierte Bausteine werden **nicht** neu aus der `lib` gezogen (kein stilles Versions-Update),
/// nur fehlende `id`s werden als Vollkopie angehängt. Unbekannte/bereits vorhandene `id`s werden
/// übersprungen. Der Toolstack-Anzeigename bleibt unverändert. Gibt den erweiterten Stack zurück.
pub fn extend_product_stack(
    root: &Path,
    lib: &Bibliothek,
    neue_baustein_ids: &[String],
) -> std::io::Result<ProduktStack> {
    let mut stack = read_stack(root);
    for id in neue_baustein_ids {
        let schon_da = stack.bausteine.iter().any(|sb| &sb.baustein.id == id);
        if schon_da {
            continue; // bereits kopiert — nie neu ziehen (Anti-Drift)
        }
        if let Some(b) = lib.read_baustein(id) {
            stack.bausteine.push(StackBaustein::copy_of(&b));
        }
    }
    write_stack(root, &stack)?;
    Ok(stack)
}

/// Einen Baustein im Stack **stilllegen** bzw. wieder **reaktivieren** — die reine
/// Zustandsänderung (Issue #51, E17). **Label-only:** setzt nur den `stillgelegt`-Schalter der
/// Voll-Kopie; **kein** Glob/Ignore/LFS wird angefasst, nichts wird entfernt — daher (fast) voll
/// umkehrbar (`reaktivieren` = `stilllegen` mit `false`). Gibt zurück, ob ein Baustein mit der `id`
/// gefunden und (falls nötig) geändert wurde. Total und rein — kein I/O.
pub fn set_baustein_stillgelegt(stack: &mut ProduktStack, id: &str, stillgelegt: bool) -> bool {
    if let Some(sb) = stack.bausteine.iter_mut().find(|sb| sb.baustein.id == id) {
        sb.baustein.stillgelegt = stillgelegt;
        true
    } else {
        false
    }
}

/// Einen Baustein eines Produkts **stilllegen** bzw. reaktivieren und den Stack zurückschreiben
/// (Issue #51). Liest den Stack, setzt den Schalter ([`set_baustein_stillgelegt`]) und persistiert
/// **nur bei Änderung**. Die Dotfile-Marker-Blöcke (Sediment) werden bewusst **nicht** angefasst —
/// sie bleiben liegen (E17). Gibt den (ggf. geänderten) Stack zurück; eine unbekannte `id` lässt
/// den Stack unverändert (nie Fehler).
pub fn stilllegen_baustein(root: &Path, id: &str, stillgelegt: bool) -> std::io::Result<ProduktStack> {
    let mut stack = read_stack(root);
    if set_baustein_stillgelegt(&mut stack, id, stillgelegt) {
        write_stack(root, &stack)?;
    }
    Ok(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baustein::{Baustein, Oeffnen, Toolstack};
    use std::path::PathBuf;

    fn baustein(id: &str, version: u32, heimat: &str) -> Baustein {
        Baustein {
            id: id.to_string(),
            version,
            name: id.to_string(),
            heimat: heimat.to_string(),
            globs: vec!["*.x".to_string()],
            ignore: vec![],
            lfs: vec![],
            oeffnen: Oeffnen::Auto,
            startaufgaben: vec![],
            default_kanten: vec![],
            paar_default_kanten: vec![],
            stillgelegt: false,
        }
    }

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plm-stack-ut-{}-{}",
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

    #[test]
    fn missing_stack_reads_as_empty() {
        let dir = tmp();
        assert!(read_stack(&dir).bausteine.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_stack_degrades_to_empty() {
        let dir = tmp();
        std::fs::create_dir_all(STACK.path(&dir).parent().unwrap()).unwrap();
        std::fs::write(STACK.path(&dir), "{ not json ]").unwrap();
        assert!(read_stack(&dir).bausteine.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_carries_full_definition_and_provenance() {
        let b = baustein("kicad", 3, "elektronik");
        let copy = StackBaustein::copy_of(&b);
        assert_eq!(copy.herkunft, Herkunft { from: "kicad".to_string(), version: 3 });
        assert_eq!(copy.baustein, b);

        // round-trips through JSON (flatten keeps the Baustein fields at top level next to herkunft)
        let json = serde_json::to_string_pretty(&copy).unwrap();
        assert!(json.contains("\"herkunft\""));
        assert!(json.contains("\"globs\""));
        let back: StackBaustein = serde_json::from_str(&json).unwrap();
        assert_eq!(back, copy);
    }

    #[test]
    fn create_product_stack_copies_chosen_bausteine() {
        let dir = tmp();
        let libdir = tmp();
        let lib = Bibliothek::new(&libdir);
        lib.seed_from(
            &[baustein("kicad", 1, "elektronik"), baustein("fusion", 1, "mechanik")],
            &[Toolstack {
                id: "standard-hw".to_string(),
                name: "Standard".to_string(),
                baustein_ids: vec!["kicad".to_string(), "fusion".to_string()],
            }],
        )
        .unwrap();

        let stack = create_product_stack(
            &dir,
            &lib,
            &["kicad".to_string(), "fusion".to_string(), "ghost".to_string()],
            Some("standard-hw".to_string()),
        )
        .unwrap();

        // unknown "ghost" id skipped; the two real ones copied in order
        assert_eq!(stack.bausteine.len(), 2);
        assert_eq!(stack.bausteine[0].baustein.id, "kicad");
        assert_eq!(stack.bausteine[1].baustein.id, "fusion");
        assert_eq!(stack.toolstack.as_deref(), Some("standard-hw"));

        // re-read from disk equals what we wrote
        assert_eq!(read_stack(&dir), stack);

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&libdir);
    }

    #[test]
    fn extend_appends_new_and_preserves_existing_copies() {
        let dir = tmp();
        let libdir = tmp();
        let lib = Bibliothek::new(&libdir);
        // Bibliothek hat kicad@1 + zephyr@1.
        lib.seed_from(
            &[baustein("kicad", 1, "elektronik"), baustein("zephyr", 1, "firmware")],
            &[],
        )
        .unwrap();

        // Stack zunächst nur mit kicad@1.
        create_product_stack(&dir, &lib, &["kicad".to_string()], None).unwrap();

        // Bibliothek aktualisiert kicad auf @2 — die bestehende Kopie darf NICHT mitwandern.
        lib.seed_from(&[baustein("kicad", 2, "elektronik")], &[]).unwrap();

        // Additiv um zephyr erweitern; kicad erneut mit anführen (muss übersprungen werden).
        let stack = extend_product_stack(
            &dir,
            &lib,
            &["kicad".to_string(), "zephyr".to_string(), "ghost".to_string()],
        )
        .unwrap();

        assert_eq!(stack.bausteine.len(), 2);
        // kicad-Kopie bleibt @1 (Anti-Drift), nicht neu auf @2 gezogen.
        assert_eq!(stack.bausteine[0].baustein.id, "kicad");
        assert_eq!(stack.bausteine[0].baustein.version, 1);
        assert_eq!(stack.bausteine[0].herkunft.version, 1);
        // zephyr@1 neu angehängt.
        assert_eq!(stack.bausteine[1].baustein.id, "zephyr");
        assert_eq!(stack.bausteine[1].herkunft, Herkunft { from: "zephyr".to_string(), version: 1 });
        // unbekanntes "ghost" übersprungen; Persistenz stimmt.
        assert_eq!(read_stack(&dir), stack);

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&libdir);
    }

    #[test]
    fn stilllegen_is_label_only_and_reversible() {
        // Tabelle: (start-stillgelegt, gesetzt) -> erwartet-stillgelegt + ob gefunden.
        let mut stack = ProduktStack {
            toolstack: None,
            bausteine: vec![
                StackBaustein::copy_of(&baustein("kicad", 1, "elektronik")),
                StackBaustein::copy_of(&baustein("fusion", 1, "mechanik")),
            ],
        };
        // Die übrige Definition bleibt unberührt (label-only): Globs/Ignore/LFS unverändert.
        let kicad_vorher = stack.bausteine[0].baustein.clone();

        // Stilllegen: nur der Schalter kippt.
        assert!(set_baustein_stillgelegt(&mut stack, "kicad", true));
        assert!(stack.bausteine[0].baustein.stillgelegt);
        assert!(!stack.bausteine[1].baustein.stillgelegt, "fusion unberührt");
        assert_eq!(stack.bausteine[0].baustein.globs, kicad_vorher.globs, "Globs liegen bleiben");

        // Reaktivieren: voll umkehrbar — der Baustein ist wieder identisch zum Ausgangszustand.
        assert!(set_baustein_stillgelegt(&mut stack, "kicad", false));
        assert_eq!(stack.bausteine[0].baustein, kicad_vorher);

        // Unbekannte id: nichts gefunden, Stack unverändert.
        let snapshot = stack.clone();
        assert!(!set_baustein_stillgelegt(&mut stack, "ghost", true));
        assert_eq!(stack, snapshot);
    }

    #[test]
    fn stilllegen_baustein_persists_and_unknown_id_is_a_noop() {
        let dir = tmp();
        let libdir = tmp();
        let lib = Bibliothek::new(&libdir);
        lib.seed_from(&[baustein("kicad", 1, "elektronik")], &[]).unwrap();
        create_product_stack(&dir, &lib, &["kicad".to_string()], None).unwrap();

        // Stilllegen wird persistiert.
        let stack = stilllegen_baustein(&dir, "kicad", true).unwrap();
        assert!(stack.bausteine[0].baustein.stillgelegt);
        assert!(read_stack(&dir).bausteine[0].baustein.stillgelegt, "auf Platte stillgelegt");

        // Reaktivieren persistiert ebenso (umkehrbar).
        let stack = stilllegen_baustein(&dir, "kicad", false).unwrap();
        assert!(!stack.bausteine[0].baustein.stillgelegt);
        assert!(!read_stack(&dir).bausteine[0].baustein.stillgelegt);

        // Unbekannte id: kein Fehler, Stack unverändert.
        let stack = stilllegen_baustein(&dir, "ghost", true).unwrap();
        assert_eq!(stack, read_stack(&dir));

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&libdir);
    }

    #[test]
    fn extend_on_missing_stack_starts_fresh() {
        let dir = tmp();
        let libdir = tmp();
        let lib = Bibliothek::new(&libdir);
        lib.seed_from(&[baustein("kicad", 1, "elektronik")], &[]).unwrap();

        let stack = extend_product_stack(&dir, &lib, &["kicad".to_string()]).unwrap();
        assert_eq!(stack.bausteine.len(), 1);
        assert_eq!(stack.bausteine[0].baustein.id, "kicad");

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&libdir);
    }
}
