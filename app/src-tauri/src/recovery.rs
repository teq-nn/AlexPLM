//! Die **Rückfallnetz-Entscheidung** — der dünne, reine Kern der Recovery-Transaktion (Issue #133,
//! E56/E56a).
//!
//! Folgt dem Haus-Muster (`warden.rs`, `syncdecider.rs`, `freigabegate.rs`): **eine** reine,
//! totale, deterministische Funktion über einen schlichten Snapshot. Sie kennt **kein** git, keine
//! Uhr, kein I/O — die Mechanik (Schnappschuss-Ref legen, Operation fahren, bei Fehler
//! zurückdrehen) lebt in der Glue [`crate::recoveryglue`]; dieses Modul **entscheidet nur**: Ergab
//! die gefährliche Operation einen Fehler, wird **zurückgedreht**, sonst wird **festgeschrieben**.
//!
//! Die tragende Idee von E56: Jede gefährliche Operation läuft als **Transaktion** — vorher ein
//! Schnappschuss, bei Fehler oder Stromausfall ein **automatischer** Rückfall auf genau diesen
//! Schnappschuss. So sieht der Hardware-Entwickler nie ein kaputtes Repo. Und scheitert etwas, liest
//! er **„ist schiefgegangen, ich hab's zurückgedreht"** statt roher git-Texte — und bleibt
//! handlungsfähig. Der reine Kern hier ist genau dieser „Festschreiben-oder-Zurückdrehen"-Schalter:
//! Snapshot rein (lief die Operation durch?), exakt **eine** [`RueckfallEntscheidung`] raus.

use std::borrow::Cow;

/// Der ehrliche Satz, den der Nutzer nach einem zurückgedrehten Fehlversuch liest (E56). **Domänen-
/// sprache**, nie roher git-Text: „etwas ist schiefgegangen, ich hab's zurückgedreht" — das Repo
/// steht wieder genau auf dem Schnappschuss von vorher, es ging nichts kaputt. Der konkrete
/// git-Grund wandert nur ins Diagnose-Log ([`crate::gitlog`]), nie in diese Meldung.
pub const ZURUECKGEDREHT: &str = "Es ist etwas schiefgegangen — ich hab's zurückgedreht. \
Dein Stand ist unverändert, es ging nichts kaputt.";

/// Was nach dem Lauf der gefährlichen Operation mit dem Schnappschuss geschehen soll. Genau eine
/// von zwei Möglichkeiten; total. Die Glue [`crate::recoveryglue`] gehorcht nur — die Wahl trifft
/// allein [`entscheide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RueckfallEntscheidung {
    /// Die Operation lief sauber durch: den neuen Stand **festschreiben** und den Schnappschuss
    /// (das Netz) wieder einrollen — er wird nicht mehr gebraucht.
    Festschreiben,
    /// Die Operation schlug fehl (oder brach ab): **zurückdrehen** auf den Schnappschuss, sodass das
    /// Repo wieder exakt auf dem Stand von vorher landet. Trägt die ehrliche Domänen-Meldung.
    Zurueckdrehen,
}

impl RueckfallEntscheidung {
    /// `true` ⇔ es wird zurückgedreht (die Operation ist gescheitert). Bequemlichkeit für die Glue
    /// und die Tests, damit der Verzweigungssinn an einer Stelle benannt ist.
    pub fn rollt_zurueck(self) -> bool {
        matches!(self, RueckfallEntscheidung::Zurueckdrehen)
    }
}

/// Der schlichte Snapshot, den der reine Kern beurteilt — vom Glue-Lauf der gefährlichen Operation
/// schon eingesammelt, hier nie selbst beschafft. Bewusst minimal: die einzige Tatsache, die über
/// Festschreiben-oder-Zurückdrehen entscheidet, ist, **ob die Operation einen Fehler ergab**. Ein
/// Stromausfall mitten in der Operation ist derselbe Fall wie ein Fehler — er erreicht diesen Kern
/// gar nicht erst (kein Festschreiben lief), und der nächste Start findet den liegengebliebenen
/// Schnappschuss und dreht zurück; die Entscheidung ist in beiden Fällen dieselbe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RueckfallSnapshot {
    /// `true`, wenn die gefährliche Operation einen Fehler ergab (oder abbrach). `false` ⇔ sie lief
    /// sauber durch.
    pub fehlgeschlagen: bool,
}

/// Die **Rückfallnetz-Entscheidung**: gab die gefährliche Operation einen Fehler, wird
/// **zurückgedreht**, sonst **festgeschrieben**. **Rein, total, deterministisch** — kein I/O, keine
/// Uhr. Snapshot rein, genau eine [`RueckfallEntscheidung`] raus.
///
/// Das ist der ganze „commit-or-rollback"-Kern aus E56: ein Schalter über die eine relevante
/// Tatsache. Die Mechanik (Schnappschuss-Ref, Lauf, das eigentliche Zurückdrehen) ist Glue; die
/// Wahl ist hier — und damit ohne git tabellen-testbar.
pub fn entscheide(snap: RueckfallSnapshot) -> RueckfallEntscheidung {
    if snap.fehlgeschlagen {
        RueckfallEntscheidung::Zurueckdrehen
    } else {
        RueckfallEntscheidung::Festschreiben
    }
}

/// Die Meldung, die der Nutzer für eine Entscheidung liest. Festschreiben ist still (kein Text —
/// die Operation hat ja ihr eigenes Ergebnis); Zurückdrehen liefert die ehrliche Domänen-Meldung
/// [`ZURUECKGEDREHT`]. **Rein** über die Entscheidung → tabellen-testbar.
pub fn meldung(entscheidung: RueckfallEntscheidung) -> Option<&'static str> {
    match entscheidung {
        RueckfallEntscheidung::Festschreiben => None,
        RueckfallEntscheidung::Zurueckdrehen => Some(ZURUECKGEDREHT),
    }
}

/// Ein roher git-Fehlertext zu einer **ehrlichen Domänen-Meldung** veredelt (E56): egal was git
/// gesagt hat, der Nutzer liest „ist schiefgegangen, ich hab's zurückgedreht". **Rein** über den
/// Eingabetext (der nur ins Diagnose-Log wandert, nie in die Rückgabe) → tabellen-testbar. Gibt
/// einen `Cow` zurück, damit der konstante Satz ohne Allokation durchgereicht wird.
pub fn ehrliche_meldung(_roher_git_text: &str) -> Cow<'static, str> {
    // Der rohe Text wird bewusst verworfen — er gehört ins Diagnose-Log, nie vor den Nutzer (E56).
    Cow::Borrowed(ZURUECKGEDREHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Der Kern-Schalter** als Tabelle: lief die Operation durch → Festschreiben; ergab sie einen
    /// Fehler → Zurückdrehen. Genau eine Entscheidung je Snapshot, über den ganzen (zweielementigen)
    /// Eingaberaum.
    #[test]
    fn commit_or_rollback_per_outcome() {
        // table: fehlgeschlagen? -> erwartete Entscheidung
        let cases: &[(bool, RueckfallEntscheidung)] = &[
            (false, RueckfallEntscheidung::Festschreiben),
            (true, RueckfallEntscheidung::Zurueckdrehen),
        ];
        for (fehlgeschlagen, erwartet) in cases {
            let snap = RueckfallSnapshot { fehlgeschlagen: *fehlgeschlagen };
            assert_eq!(entscheide(snap), *erwartet, "fehlgeschlagen={fehlgeschlagen}");
            // Der Verzweigungssinn ist konsistent mit der Bequemlichkeit.
            assert_eq!(entscheide(snap).rollt_zurueck(), *fehlgeschlagen);
        }
    }

    /// **Total + deterministisch**: der Default-Snapshot (nichts schiefgegangen) schreibt fest, und
    /// derselbe Snapshot ergibt immer dieselbe Entscheidung.
    #[test]
    fn empty_commits_and_decision_is_deterministic() {
        let snap = RueckfallSnapshot::default();
        assert_eq!(entscheide(snap), RueckfallEntscheidung::Festschreiben);
        assert!(!entscheide(snap).rollt_zurueck());
        assert_eq!(entscheide(snap), entscheide(snap));
    }

    /// **Die Meldungen** als Tabelle: Festschreiben ist still, Zurückdrehen liefert die ehrliche
    /// Domänen-Meldung — und die trägt **nie** roh-git-Marken.
    #[test]
    fn message_per_decision_is_domain_language() {
        // table: Entscheidung -> erwartete Meldung
        let cases: &[(RueckfallEntscheidung, Option<&str>)] = &[
            (RueckfallEntscheidung::Festschreiben, None),
            (RueckfallEntscheidung::Zurueckdrehen, Some(ZURUECKGEDREHT)),
        ];
        for (entscheidung, erwartet) in cases {
            assert_eq!(meldung(*entscheidung), *erwartet, "{entscheidung:?}");
        }
        // Die Zurückgedreht-Meldung spricht Domänensprache: sie sagt, dass zurückgedreht wurde, und
        // enthält keinerlei git-Vokabel.
        assert!(ZURUECKGEDREHT.contains("zurückgedreht"));
        for marker in ["git", "reset", "HEAD", "rebase", "ref", "commit", "stash", "<<<<<<<"] {
            assert!(
                !ZURUECKGEDREHT.to_lowercase().contains(marker),
                "die Meldung darf keinen git-Marker tragen: {marker:?}"
            );
        }
    }

    /// **Veredelung**: egal welcher rohe git-Text hereinkommt, der Nutzer liest immer denselben
    /// ehrlichen Satz — der rohe Text wird nie durchgereicht (er gehört ins Diagnose-Log).
    #[test]
    fn raw_git_text_is_never_passed_through() {
        let rohe = &[
            "fatal: not a git repository",
            "error: failed to push some refs",
            "<<<<<<< HEAD\nkonflikt\n>>>>>>>",
            "fatal: bad revision 'HEAD'",
            "",
        ];
        for roh in rohe {
            let veredelt = ehrliche_meldung(roh);
            assert_eq!(veredelt, ZURUECKGEDREHT, "roh={roh:?}");
            // Kein Bruchstück des rohen Textes überlebt (außer leer, das nie kollidiert).
            if !roh.is_empty() {
                assert!(!veredelt.contains(*roh), "roher git-Text leakte in die Meldung");
            }
        }
    }
}
