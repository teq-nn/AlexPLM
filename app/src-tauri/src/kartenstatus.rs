//! Abgeleiteter Artefakt-Karten-Status (Issue #53, E26) — der reine, totale Kern.
//!
//! Der Status einer Artefakt-Karte wird **live abgeleitet, nie gespeichert** (E26, „keine
//! zweite Wahrheit"): *aus Git* der Datei-Zustand, *aus Kanten* die **Stale**-Warnung. Im
//! `_plm` lebt nur, was Git nicht kennen kann (Pflicht/Optional/Freigabe — nicht hier).
//!
//! Dieses Modul ist der **tiefe Kern** der Karten-Projektion, die #53 der Graph-/Werkbank-
//! Sicht hinzufügt und auf die #55 (Graph-Raum-Filter) und #56 (Kanten) aufsetzen:
//!
//! > `Git-Zustände der Karten-Dateien (+ Stale-Flag) → ein Karten-Status`
//!
//! Wie im Haus üblich: **reiner Kern hier, kein I/O.** Die Git-/Platten-Lese-Glue lebt in der
//! dünnen Glue-Schicht (`werkbank.rs`/`graphread.rs`); dieses Modul entscheidet nur und ist
//! durch `#[cfg(test)]`-Tabellentests über das Kreuzprodukt belegt.
//!
//! Die fünf Git-Zustände aus E26 — **Vorhanden / Geändert / fehlt / Übernommen / Ignoriert** —
//! werden aus `git status --porcelain`-Codes je Datei abgeleitet ([`GitFileState`]); der
//! Karten-Status faltet die Datei-Zustände zu **einem** Status, plus ein **Stale**-Flag aus den
//! Kanten (E40: keine Kante = kein Stale). „Nicht zugeordnet" ist bewusst **kein** Status — eine
//! Waise hat keine Karte (E11), das ist das Unzugeordnet-Fach.

use serde::Serialize;

/// Der **Git-Zustand einer einzelnen Datei** der Karte, abgeleitet aus `git status`. Reiner
/// Input-Fakt; der Karten-Status ([`KartenStatus`]) faltet die Datei-Zustände zusammen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileState {
    /// Erfasst und unverändert gegenüber dem letzten Stand — der ruhige Normalfall.
    Vorhanden,
    /// Erfasst, aber im Arbeitsbereich geändert (noch nicht in einem Stand) — der laute Fall.
    Geaendert,
    /// Erfasst, aber auf der Platte verschwunden (deleted) — fehlt.
    Fehlt,
    /// Frisch hinzugekommen / neu erfasst (added/untracked-but-folded) — übernommen.
    Uebernommen,
    /// Bewusst ignoriert (`.gitignore`) — kein Artefakt-Inhalt, der stumme Sonderfall.
    Ignoriert,
}

impl GitFileState {
    /// Den Datei-Zustand aus einem `git status --porcelain` **XY**-Code ableiten (zwei Zeichen:
    /// Index- + Worktree-Status). Total: jeder Code fällt in genau einen Zustand, Unbekanntes
    /// (oder ein leerer Code = sauber erfasst) auf das ruhige **Vorhanden** — das Werkzeug
    /// behauptet nie lauter, als es weiß (E26).
    ///
    /// Mapping (an `git status --porcelain` v1 angelehnt, platform-neutral):
    /// - `!!`            → Ignoriert (ignorierte Datei)
    /// - `??` / `A ` / `AM` (neu/added) → Übernommen
    /// - irgendein `D` (gelöscht, Index oder Worktree) → fehlt
    /// - irgendein `M`/`T`/`R`/`C` (modifiziert/typ/umbenannt/kopiert) → Geändert
    /// - leer / unbekannt → Vorhanden (erfasst, ruhig)
    pub fn from_porcelain(code: &str) -> GitFileState {
        let code = code.trim_end_matches(['\r', '\n']);
        let bytes: Vec<char> = code.chars().take(2).collect();
        let x = bytes.first().copied().unwrap_or(' ');
        let y = bytes.get(1).copied().unwrap_or(' ');

        // Ignoriert hat in porcelain den eigenen `!!`-Code (nur mit --ignored sichtbar).
        if x == '!' && y == '!' {
            return GitFileState::Ignoriert;
        }
        // Neu/hinzugekommen: untracked (`??`) oder frisch added (`A` im Index).
        if (x == '?' && y == '?') || x == 'A' {
            return GitFileState::Uebernommen;
        }
        // Verschwunden: gelöscht in Index oder Worktree.
        if x == 'D' || y == 'D' {
            return GitFileState::Fehlt;
        }
        // Geändert: modifiziert / typgeändert / umbenannt / kopiert, Index oder Worktree.
        if matches!(x, 'M' | 'T' | 'R' | 'C') || matches!(y, 'M' | 'T' | 'R' | 'C') {
            return GitFileState::Geaendert;
        }
        // Leerer / unbekannter Code: erfasst und ruhig.
        GitFileState::Vorhanden
    }

    /// Wie **laut** dieser Zustand auf der Karte ist (höher = lauter, gewinnt beim Falten).
    /// Geändert/Fehlt sind die lauten „prüf-mich"-Fälle, Übernommen ein leiser Hinweis,
    /// Vorhanden der ruhige Normalfall, Ignoriert der stumme Außenfall. So bleibt die Karte
    /// „im Alltag fast stumm, laut erst am Meilenstein-Check" (E26/E22).
    fn lautstaerke(self) -> u8 {
        match self {
            GitFileState::Geaendert => 4,
            GitFileState::Fehlt => 3,
            GitFileState::Uebernommen => 2,
            GitFileState::Vorhanden => 1,
            GitFileState::Ignoriert => 0,
        }
    }
}

/// Der **abgeleitete Karten-Status** einer Artefakt-Karte (E26). Serialisiert in kebab-case zur
/// UI (`"vorhanden"`/`"geaendert"`/`"fehlt"`/`"uebernommen"`/`"ignoriert"`). Das orthogonale
/// **Stale**-Flag ([`KartenProjektion::stale`]) reitet daneben, nicht hier hinein — eine Karte
/// kann „vorhanden" **und** „stale" sein (Quelle neuer als Ableitung, E26/E40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum KartenStatus {
    /// Alle Dateien erfasst und ruhig — der Normalfall. Auch die Antwort für eine leere Karte.
    #[default]
    Vorhanden,
    /// Mindestens eine Datei ist im Arbeitsbereich geändert — der laute „prüf-mich"-Fall.
    Geaendert,
    /// Mindestens eine erfasste Datei ist verschwunden (deleted).
    Fehlt,
    /// Frisch hinzugekommene Dateien, sonst nichts Lauteres.
    Uebernommen,
    /// Die ganze Karte ist ignoriert (alle Dateien `!!`) — der stumme Außenfall.
    Ignoriert,
}

impl From<GitFileState> for KartenStatus {
    fn from(s: GitFileState) -> KartenStatus {
        match s {
            GitFileState::Vorhanden => KartenStatus::Vorhanden,
            GitFileState::Geaendert => KartenStatus::Geaendert,
            GitFileState::Fehlt => KartenStatus::Fehlt,
            GitFileState::Uebernommen => KartenStatus::Uebernommen,
            GitFileState::Ignoriert => KartenStatus::Ignoriert,
        }
    }
}

/// Die **abgeleitete Karten-Projektion** (#53): der gefaltete Git-Status **plus** das
/// orthogonale Stale-Flag. Das ist die Form, die #55 (Filter) und #56 (Kanten) konsumieren —
/// stabil und vollständig getrennt: `status` aus Git, `stale` aus Kanten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct KartenProjektion {
    /// Der live aus Git abgeleitete Karten-Status (E26).
    pub status: KartenStatus,
    /// **Stale** (E26/E40): es existiert eine Hand-Kante **und** eine Quelle ist neuer als die
    /// Ableitung. `false`, wo keine Kante zeigt — keine Kante = keine Warnung. Orthogonal zum
    /// `status`: eine ruhige „vorhandene" Karte kann trotzdem stale sein.
    pub stale: bool,
}

/// **Der reine Kern**: falte die Git-Zustände der Karten-Dateien zu **einem** Karten-Status —
/// total. Es gewinnt der **lauteste** Zustand (Geändert > Fehlt > Übernommen > Vorhanden),
/// damit die Karte am Meilenstein-Check laut wird, sobald *eine* Datei Aufmerksamkeit braucht.
///
/// Sonderfälle:
/// - **Leere** Karte (keine Dateien) → ruhig `Vorhanden` (nie ein Fehler).
/// - **Alle** Dateien ignoriert → `Ignoriert` (der stumme Außenfall bleibt stumm); eine einzige
///   nicht-ignorierte Datei zieht die Karte aus `Ignoriert` heraus.
pub fn derive_karten_status(file_states: &[GitFileState]) -> KartenStatus {
    // Ignoriert ist der stumme Außenfall: nur wenn *alle* Dateien ignoriert sind, ist es die
    // Karte; sonst wird die ignorierte Datei beim Falten von jedem lauteren Zustand übertönt.
    let all_ignored =
        !file_states.is_empty() && file_states.iter().all(|s| *s == GitFileState::Ignoriert);
    if all_ignored {
        return KartenStatus::Ignoriert;
    }

    file_states
        .iter()
        .copied()
        .filter(|s| *s != GitFileState::Ignoriert)
        .max_by_key(|s| s.lautstaerke())
        .map(KartenStatus::from)
        .unwrap_or(KartenStatus::Vorhanden)
}

/// **Der reine Kern (komplett)**: leite die ganze [`KartenProjektion`] ab — gefalteter Git-Status
/// **plus** Stale-Flag. `stale` ist genau dann `true`, wenn die Karte (über *eine* ihrer Dateien)
/// am stumpfen Ende einer fired Stale-Kante steht. Total und rein; die Kanten-Auswertung selbst
/// (E26) liefert [`crate::edges::stale_warnings`] — hier nur „ist *diese* Karte betroffen?".
///
/// `karten_dateien` sind die produkt-relativen Pfade der Karte; `stale_derived` ist die Menge der
/// `derived`-Pfade aller fired Stale-Warnungen (so bleibt die Kanten-Logik die eine Wahrheit, und
/// dieser Kern hängt nicht an `edges` — er prüft nur Mengen-Zugehörigkeit).
pub fn derive_karten_projektion(
    file_states: &[GitFileState],
    karten_dateien: &[String],
    stale_derived: &[String],
) -> KartenProjektion {
    let stale = karten_dateien.iter().any(|d| stale_derived.contains(d));
    KartenProjektion {
        status: derive_karten_status(file_states),
        stale,
    }
}

#[cfg(test)]
mod tests {
    use super::GitFileState::*;
    use super::*;

    #[test]
    fn porcelain_codes_map_to_the_five_git_states() {
        // table: porcelain XY code -> GitFileState (E26's five Git-derived states).
        let cases: &[(&str, GitFileState)] = &[
            // sauber erfasst / leer -> Vorhanden (ruhig)
            ("", Vorhanden),
            ("  ", Vorhanden),
            // geändert: Index oder Worktree, plus type/rename/copy
            (" M", Geaendert),
            ("M ", Geaendert),
            ("MM", Geaendert),
            (" T", Geaendert),
            ("R ", Geaendert),
            ("C ", Geaendert),
            // verschwunden
            (" D", Fehlt),
            ("D ", Fehlt),
            // neu / hinzugekommen
            ("??", Uebernommen),
            ("A ", Uebernommen),
            ("AM", Uebernommen),
            // ignoriert
            ("!!", Ignoriert),
            // unbekannter Code degradiert ruhig auf Vorhanden (nie lauter als bekannt)
            ("XY", Vorhanden),
        ];
        for (code, expected) in cases {
            assert_eq!(
                GitFileState::from_porcelain(code),
                *expected,
                "porcelain code {code:?}"
            );
        }
    }

    #[test]
    fn loudest_file_state_wins_the_card() {
        // table: file states of a card -> folded card status. Geändert > Fehlt > Übernommen >
        // Vorhanden; ignorierte Dateien werden von jedem lauteren Zustand übertönt.
        let cases: &[(&[GitFileState], KartenStatus)] = &[
            // leere Karte -> ruhig Vorhanden (nie ein Fehler)
            (&[], KartenStatus::Vorhanden),
            // alle ruhig
            (&[Vorhanden, Vorhanden], KartenStatus::Vorhanden),
            // eine geänderte Datei macht die ganze Karte laut
            (&[Vorhanden, Geaendert], KartenStatus::Geaendert),
            // Geändert schlägt Fehlt schlägt Übernommen
            (&[Fehlt, Geaendert], KartenStatus::Geaendert),
            (&[Uebernommen, Fehlt], KartenStatus::Fehlt),
            (&[Vorhanden, Uebernommen], KartenStatus::Uebernommen),
            // eine ignorierte Datei zieht die Karte NICHT herunter
            (&[Vorhanden, Ignoriert], KartenStatus::Vorhanden),
            (&[Ignoriert, Geaendert], KartenStatus::Geaendert),
            // nur wenn ALLE Dateien ignoriert sind, ist die Karte ignoriert
            (&[Ignoriert, Ignoriert], KartenStatus::Ignoriert),
        ];
        for (states, expected) in cases {
            assert_eq!(
                derive_karten_status(states),
                *expected,
                "file states {states:?}"
            );
        }
    }

    #[test]
    fn stale_is_orthogonal_to_status_and_needs_an_edge() {
        // E26/E40: stale fires iff one of the card's files is the `derived` end of a fired
        // Stale-Warnung; no edge (empty derived set) => never stale, regardless of git status.
        let dateien = vec!["fertigung/gerber".to_string()];

        // keine Kante => nie stale, auch bei lautem Git-Status
        let p = derive_karten_projektion(&[Geaendert], &dateien, &[]);
        assert_eq!(p.status, KartenStatus::Geaendert);
        assert!(!p.stale, "no edge = no stale (E40)");

        // Kante zeigt auf diese Karte => stale, orthogonal zum ruhigen „vorhanden"
        let stale_set = vec!["fertigung/gerber".to_string()];
        let p = derive_karten_projektion(&[Vorhanden], &dateien, &stale_set);
        assert_eq!(p.status, KartenStatus::Vorhanden);
        assert!(p.stale, "a quiet card can still be stale");

        // Kante zeigt auf eine ANDERE Karte => diese ist nicht stale
        let other = vec!["mechanik/gehaeuse".to_string()];
        let p = derive_karten_projektion(&[Vorhanden], &dateien, &other);
        assert!(!p.stale);
    }

    #[test]
    fn end_to_end_porcelain_to_projektion() {
        // The whole #53 chain for one card: porcelain codes -> file states -> folded status,
        // plus the edge-derived stale flag. Proves the layers compose.
        let codes = [" M", "  "]; // one geändert, one ruhig
        let states: Vec<GitFileState> =
            codes.iter().map(|c| GitFileState::from_porcelain(c)).collect();
        let dateien = vec!["fertigung/gerber".to_string()];
        let stale_set = vec!["fertigung/gerber".to_string()];

        let p = derive_karten_projektion(&states, &dateien, &stale_set);
        assert_eq!(p.status, KartenStatus::Geaendert);
        assert!(p.stale);
    }
}
