//! Baustein **stilllegen** — die tiefe Sub-Funktion (Issue #51, E17/E18) als **reiner Kern**.
//!
//! Stilllegen ist **label-only und (fast) vollständig umkehrbar** (E17, Glossar „Stilllegen"):
//!
//! - Die Artefakt-**Globs** des stillgelegten Bausteins **hören auf zu greifen** → die zuvor von
//!   ihm getragenen Dateien werden zu **Waisen** (E11), die im Unzugeordnet-Fach des Arbeitsbereichs
//!   auftauchen. **Nichts wird verschoben oder gelöscht.**
//! - Die Ignore-/LFS-Marker-Block-Zeilen des Bausteins in den Dotfiles bleiben als **Sediment**
//!   liegen — sie werden **nie automatisch entfernt** (E17: Entfernen wäre die einzige Operation,
//!   die alten Müll wieder sichtbar macht oder ein teures `git lfs migrate` auslöst).
//!
//! Dieses Modul ist **rein, total, deterministisch** — kein I/O, kein Clock. Es bekommt den Stack
//! **vor** dem Stilllegen, die `id` des stillzulegenden Bausteins und die erfassten Dateien herein
//! und gibt die **Wirkung** zurück: welche Globs erlöschen, welche Dateien dadurch zu Waisen werden,
//! welche Sediment-Zeilen liegen bleiben — und die Invariante „nichts wurde bewegt/gelöscht". Die
//! eigentliche Zustandsänderung am Stack lebt in [`crate::stackstore`]; das Falten zur Werkbank in
//! [`crate::werkbank`]. Beide nutzen denselben `stillgelegt`-Schalter, den dieses Modul auswertet.
//!
//! Wie im Haus üblich: **reiner Kern hier**, `#[cfg(test)]`-Tabellentests belegen, dass Stilllegen
//! die erwartete Waisen-Menge erzeugt, das Sediment hält und nichts umsiedelt.

use crate::stackstore::ProduktStack;
use crate::zuordnung::{zuordnen, BausteinRegel, Zuordnung};
use serde::Serialize;

/// Die **Wirkung** eines Stilllegens, rein berechnet aus Stack + erfassten Dateien (kein I/O).
/// Das ist die Antwort der tiefen Sub-Funktion: *Was passiert, wenn ich diesen Baustein stilllege?*
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct StilllegenWirkung {
    /// Die Globs des Bausteins, die mit dem Stilllegen **erlöschen** (nicht mehr greifen).
    pub erloschene_globs: Vec<String>,
    /// Die zuvor vom Baustein getragenen Dateien, die durch das Stilllegen zu **Waisen** werden
    /// (produkt-relativ, Vorwärts-Slashes, sortiert). Sie wandern ins Unzugeordnet-Fach — nichts
    /// wird verschoben/gelöscht.
    pub neue_waisen: Vec<String>,
    /// Die Ignore-/LFS-Muster des Bausteins, die als **Sediment** in den Dotfiles liegen bleiben
    /// (nie automatisch entfernt). Reine Diagnose/Anzeige — die Dotfiles selbst bleiben unberührt.
    pub sediment: Vec<String>,
    /// Invariante: Stilllegen ist label-only — **nichts wird verschoben oder gelöscht**. Immer `true`
    /// (explizit getragen, damit die Akzeptanz „nichts relocated/removed" prüfbar im Vertrag steht).
    pub nichts_bewegt: bool,
}

/// Berechne die [`StilllegenWirkung`] für die Baustein-`id` im gegebenen Stack über die erfassten
/// `tracked`-Dateien. **Rein und total.**
///
/// Vorgehen (deterministisch):
/// 1. Den stillzulegenden Baustein im Stack finden. Fehlt er, ist die Wirkung leer (mit `sediment`
///    leer) — nichts zu tun, nie Fehler.
/// 2. `erloschene_globs` = seine Globs; `sediment` = seine Ignore- + LFS-Muster (bleiben liegen).
/// 3. `neue_waisen` = die Dateien, die **vorher** diesem Baustein zugeordnet waren und **nachher**
///    (mit ihm stillgelegt) zur Waise fallen — exakt über den `zuordnen`-Kern berechnet, einmal mit
///    dem aktiven, einmal mit dem stillgelegten Baustein. So bleibt das Modell die einzige Wahrheit
///    und es gibt keine doppelte Glob-Logik. Fällt eine Datei nachher auf einen **anderen** Baustein
///    (überlappende Globs), ist sie **keine** neue Waise.
pub fn berechne_wirkung(stack: &ProduktStack, id: &str, tracked: &[String]) -> StilllegenWirkung {
    let Some(sb) = stack.bausteine.iter().find(|sb| sb.baustein.id == id) else {
        // Unbekannte id: label-only-Aktion ohne Wirkung. Nichts bewegt — die Invariante hält trivial.
        return StilllegenWirkung { nichts_bewegt: true, ..Default::default() };
    };
    let b = &sb.baustein;

    // Sediment = die Marker-Block-Quellen des Bausteins (Ignore + LFS), die liegen bleiben.
    let mut sediment: Vec<String> = Vec::new();
    sediment.extend(b.ignore.iter().cloned());
    sediment.extend(b.lfs.iter().cloned());

    // Zwei Regelsätze aus dem Stack: der aktuelle (vor dem Stilllegen) und der mit `id` stillgelegt.
    let regeln_vorher = regeln_aus_stack(stack, id, false);
    let regeln_nachher = regeln_aus_stack(stack, id, true);

    // Eine Datei wird zur **neuen** Waise, wenn sie vorher genau diesem Baustein gehörte und nachher
    // zur Waise fällt. Über den reinen `zuordnen`-Kern, damit Heimat/Glob/Priorität konsistent bleiben.
    let mut neue_waisen: Vec<String> = Vec::new();
    for path in tracked {
        let gehoerte_dem_baustein = matches!(
            zuordnen(path, &regeln_vorher),
            Zuordnung::Artefakt { ref artefakt_id, .. } if artefakt_id.starts_with(&format!("{id}:"))
        );
        if !gehoerte_dem_baustein {
            continue;
        }
        if matches!(zuordnen(path, &regeln_nachher), Zuordnung::Waise { .. }) {
            neue_waisen.push(path.clone());
        }
    }
    neue_waisen.sort();
    neue_waisen.dedup();

    StilllegenWirkung {
        erloschene_globs: b.globs.clone(),
        neue_waisen,
        sediment,
        nichts_bewegt: true,
    }
}

/// Den Glob-Satz aus dem Stack als `BausteinRegel`-Liste ziehen, wobei der Baustein `id` optional
/// als **stillgelegt** markiert wird (`force_stillgelegt`). Alle anderen behalten ihren echten
/// Zustand — so respektiert die Berechnung bereits zuvor stillgelegte Bausteine.
fn regeln_aus_stack(stack: &ProduktStack, id: &str, force_stillgelegt: bool) -> Vec<BausteinRegel> {
    stack
        .bausteine
        .iter()
        .map(|sb| {
            let b = &sb.baustein;
            BausteinRegel {
                id: b.id.clone(),
                name: b.name.clone(),
                heimat: b.heimat.clone(),
                globs: b.globs.clone(),
                stillgelegt: b.stillgelegt || (b.id == id && force_stillgelegt),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baustein::{Baustein, Oeffnen};
    use crate::stackstore::{Herkunft, StackBaustein};

    fn baustein(id: &str, heimat: &str, globs: &[&str], ignore: &[&str], lfs: &[&str]) -> Baustein {
        Baustein {
            id: id.to_string(),
            version: 1,
            name: id.to_string(),
            heimat: heimat.to_string(),
            globs: globs.iter().map(|s| s.to_string()).collect(),
            ignore: ignore.iter().map(|s| s.to_string()).collect(),
            lfs: lfs.iter().map(|s| s.to_string()).collect(),
            oeffnen: Oeffnen::Auto,
            startaufgaben: vec![],
            default_kanten: vec![],
            stillgelegt: false,
        }
    }

    fn stack_of(bs: &[Baustein]) -> ProduktStack {
        ProduktStack {
            toolstack: None,
            bausteine: bs
                .iter()
                .map(|b| StackBaustein {
                    herkunft: Herkunft { from: b.id.clone(), version: b.version },
                    baustein: b.clone(),
                })
                .collect(),
        }
    }

    fn ls(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// Der Kern-Akzeptanztest als **Tabelle**: für verschiedene Stilllege-Szenarien prüfen wir die
    /// drei Akzeptanzkriterien gemeinsam — erwartete Waisen-Menge, gehaltenes Sediment, nichts bewegt.
    #[test]
    fn stilllegen_yields_expected_waisen_keeps_sediment_moves_nothing() {
        // Der reale Austausch-Fall (E17): PlatformIO in der Firmware wird stillgelegt; Zephyr nicht.
        let pio = baustein(
            "platformio",
            "firmware",
            &["platformio.ini", "*.c", "*.h"],
            &[".pio/", "*.bin"],
            &["*.bin"],
        );
        let zephyr = baustein("zephyr", "firmware", &["*.overlay", "*.dts"], &["build/"], &[]);
        let doku = baustein("doku", "doku", &["*.md", "*.pdf"], &[], &[]);

        struct Case {
            name: &'static str,
            stack: ProduktStack,
            stilllegen: &'static str,
            tracked: Vec<String>,
            erwartete_waisen: Vec<String>,
            erwartetes_sediment: Vec<String>,
        }

        let cases = vec![
            Case {
                name: "platformio stillgelegt -> seine .c/.ini werden Waisen, Sediment bleibt",
                stack: stack_of(&[pio.clone(), zephyr.clone(), doku.clone()]),
                stilllegen: "platformio",
                tracked: ls(&[
                    "firmware/platformio.ini",
                    "firmware/src/main.c",
                    "firmware/boards/nrf.overlay", // gehört Zephyr -> KEINE neue Waise
                    "doku/handbuch.md",            // anderer Baustein -> unberührt
                    "firmware/notes.txt",          // war schon vorher Waise -> KEINE neue Waise
                ]),
                erwartete_waisen: ls(&["firmware/platformio.ini", "firmware/src/main.c"]),
                erwartetes_sediment: ls(&[".pio/", "*.bin", "*.bin"]), // ignore + lfs, liegen bleiben
            },
            Case {
                name: "unbekannte id -> keine Wirkung, nichts bewegt",
                stack: stack_of(&[pio.clone(), doku.clone()]),
                stilllegen: "ghost",
                tracked: ls(&["firmware/src/main.c", "doku/x.md"]),
                erwartete_waisen: vec![],
                erwartetes_sediment: vec![],
            },
            Case {
                name: "Baustein ohne erfasste Dateien -> Sediment bleibt, keine Waisen",
                stack: stack_of(&[pio.clone(), doku.clone()]),
                stilllegen: "platformio",
                tracked: ls(&["doku/x.md"]),
                erwartete_waisen: vec![],
                erwartetes_sediment: ls(&[".pio/", "*.bin", "*.bin"]),
            },
        ];

        for c in &cases {
            let w = berechne_wirkung(&c.stack, c.stilllegen, &c.tracked);
            assert_eq!(w.neue_waisen, c.erwartete_waisen, "Waisen für: {}", c.name);
            assert_eq!(w.sediment, c.erwartetes_sediment, "Sediment für: {}", c.name);
            // Akzeptanz „nichts relocated/removed": die Invariante steht immer im Vertrag.
            assert!(w.nichts_bewegt, "nichts_bewegt für: {}", c.name);
        }
    }

    /// Überlappende Globs (zwei Bausteine beanspruchen `*.c` in derselben Heimat): legt man den
    /// **erst** gelisteten still, fängt der zweite die Datei auf — sie wird **keine** Waise.
    #[test]
    fn overlapping_glob_catches_file_so_it_is_not_orphaned() {
        let a = baustein("platformio", "firmware", &["*.c"], &[], &[]);
        let b = baustein("zephyr", "firmware", &["*.c"], &[], &[]);
        let stack = stack_of(&[a, b]);
        let tracked = ls(&["firmware/main.c"]);

        // platformio (erster Treffer) stilllegen: zephyr fängt *.c auf -> keine neue Waise.
        let w = berechne_wirkung(&stack, "platformio", &tracked);
        assert!(w.neue_waisen.is_empty(), "zweiter Baustein fängt die Datei auf");
        assert_eq!(w.erloschene_globs, ls(&["*.c"]));
    }

    /// Erlöschende Globs sind genau die Globs des stillgelegten Bausteins (in Reihenfolge).
    #[test]
    fn erloschene_globs_are_the_bausteins_globs_in_order() {
        let pio = baustein("platformio", "firmware", &["platformio.ini", "*.c", "*.h"], &[], &[]);
        let stack = stack_of(&[pio]);
        let w = berechne_wirkung(&stack, "platformio", &[]);
        assert_eq!(w.erloschene_globs, ls(&["platformio.ini", "*.c", "*.h"]));
    }

    /// Ein **bereits** stillgelegter zweiter Baustein bleibt bei der Berechnung still — er fängt
    /// nichts auf. Eine Datei, die nur ihn als Alternative hätte, wird trotzdem zur Waise.
    #[test]
    fn already_decommissioned_other_baustein_does_not_catch() {
        let mut b = baustein("zephyr", "firmware", &["*.c"], &[], &[]);
        b.stillgelegt = true;
        let a = baustein("platformio", "firmware", &["*.c"], &[], &[]);
        let stack = stack_of(&[a, b]);
        let w = berechne_wirkung(&stack, "platformio", &ls(&["firmware/main.c"]));
        assert_eq!(w.neue_waisen, ls(&["firmware/main.c"]), "kein aktiver Auffänger -> Waise");
    }
}
