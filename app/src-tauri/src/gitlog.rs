//! Diagnose-Log für den Git-/Sync-Pfad (Folge von Issue #9/#54).
//!
//! Warum: die zwei Push-Typen werden bewusst geschluckt (die „stille" Vokabel der Oberfläche),
//! also hat der Nutzer keinerlei Sicht darauf, **warum** ein Push nichts tut — ob der Lock Warden
//! gar nicht erst „pushen" entschieden hat (`Refuse`) oder ob das `git push` real scheiterte
//! (Auth, Netz, Ref). Dieser Speicher ist eine dünne, immer-an Diagnose-Senke: jede **vernetzte**
//! git-Ausführung (Kommando, Exit-Code, stderr) und jede Lock-Warden-Entscheidung wird hier
//! festgehalten — in einen In-Memory-Ring (vom In-App-Diagnose-Panel gezeigt) und, sobald beim
//! Start ein Dateipfad gesetzt ist, zusätzlich in eine Logdatei (`tail -f` außerhalb der App).
//!
//! Bewusst getrennt von der stillen UI-Vokabel: es ändert **nie** Verhalten, es beobachtet nur.
//! Es werden **keine Tokens** geloggt — git hält die im Askpass-Kind, nie in der argv.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Obergrenze des In-Memory-Rings. Alt fällt vorne raus — das Panel zeigt das jüngste Geschehen,
/// die Logdatei (falls gesetzt) bleibt das vollständige, dauerhafte Protokoll.
const CAP: usize = 1000;

struct Sink {
    lines: VecDeque<String>,
    file: Option<PathBuf>,
}

fn sink() -> &'static Mutex<Sink> {
    static S: OnceLock<Mutex<Sink>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Sink { lines: VecDeque::new(), file: None }))
}

/// Wanduhr-Zeit (UTC) als `HH:MM:SS` für eine Zeile. Diagnose-Granularität; kein Datum, keine TZ.
fn stamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

/// Auf höchstens `max` Zeichen kürzen (an Zeichengrenze, nie mitten in UTF-8), mit `…`-Marke.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Die Out-of-App-Logdatei auf `path` zeigen lassen (wird angelegt, dann angehängt).
pub fn set_file(path: PathBuf) {
    if let Ok(mut s) = sink().lock() {
        s.file = Some(path);
    }
}

/// Der absolute Pfad der Out-of-App-Logdatei, falls gesetzt (für „`tail -f <pfad>`" im Panel).
pub fn file_path() -> Option<PathBuf> {
    sink().lock().ok().and_then(|s| s.file.clone())
}

/// Eine Diagnose-Zeile unter kurzem `kind`-Tag festhalten. Schiebt in den (begrenzten) Ring und —
/// falls eine Datei gesetzt ist — hängt eine Zeile an. Panik-frei; ein I/O-Fehler wird verschluckt
/// (das Panel bleibt die verlässliche Sicht), der Aufrufer wird nie blockiert oder gestört.
pub fn record(kind: &str, msg: impl AsRef<str>) {
    let line = format!("{} [{}] {}", stamp(), kind, msg.as_ref());
    if let Ok(mut s) = sink().lock() {
        while s.lines.len() >= CAP {
            s.lines.pop_front();
        }
        s.lines.push_back(line.clone());
        if let Some(path) = s.file.clone() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

/// Die Diagnose-Zeile einer git-Ausführung formatieren (rein, testbar). Bei Misserfolg wird das
/// (gekürzte) stderr angehängt — der eigentliche `push`/`lfs`-Fehler; bei Erfolg bleibt es weg.
fn fmt_git_line(label: &str, code: Option<i32>, success: bool, stderr: &str, elapsed_ms: u128) -> String {
    let exit = code.map(|c| format!("exit {c}")).unwrap_or_else(|| "Signal".into());
    let tail = {
        let e = stderr.trim();
        if success || e.is_empty() {
            String::new()
        } else {
            format!("  stderr: {}", truncate(e, 600))
        }
    };
    format!("{label} -> {exit} ({elapsed_ms}ms){tail}")
}

/// Eine vernetzte git-Ausführung protokollieren: Kommando, Exit-Code, Dauer und — bei Misserfolg —
/// das (gekürzte) stderr. Der eine Ort, an dem der eigentliche `push`/`lfs`-Fehler sichtbar wird.
pub fn record_git(label: &str, code: Option<i32>, success: bool, stderr: &str, elapsed_ms: u128) {
    record("git", fmt_git_line(label, code, success, stderr, elapsed_ms));
}

/// Eine Momentaufnahme des In-Memory-Rings (alt → neu) für das In-App-Panel.
pub fn snapshot() -> Vec<String> {
    sink().lock().map(|s| s.lines.iter().cloned().collect()).unwrap_or_default()
}

/// Den In-Memory-Ring leeren (die Datei bleibt als dauerhaftes Protokoll unberührt).
pub fn clear() {
    if let Ok(mut s) = sink().lock() {
        s.lines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NB: der In-Memory-Ring ist global und wird von der ganzen Suite (jede vernetzte git-
    // Ausführung) parallel befüllt — positionsbasierte Ring-Asserts wären flaky. Die Logik wird
    // daher über die reinen Formatierer geprüft, nicht über den geteilten globalen Zustand.

    #[test]
    fn git_line_appends_stderr_only_on_failure() {
        let ok = fmt_git_line("git push origin x:y", Some(0), true, "irrelevant", 12);
        assert!(ok.contains("exit 0"));
        assert!(!ok.contains("stderr"), "success hides stderr");

        let bad = fmt_git_line("git push origin a:b", Some(128), false, "fatal: Authentication failed", 30);
        assert!(bad.contains("exit 128"));
        assert!(bad.contains("stderr: fatal: Authentication failed"));
        assert!(bad.contains("(30ms)"));
    }

    #[test]
    fn git_line_signal_when_no_exit_code() {
        let killed = fmt_git_line("git lfs locks", None, false, "", 20000);
        assert!(killed.contains("Signal"), "no exit code -> Signal");
    }

    #[test]
    fn truncate_is_utf8_safe_and_marks() {
        let s = "ärgerlîch—lange—zeile";
        let t = truncate(s, 5);
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 6, "5 chars + ellipsis");
        assert_eq!(truncate("kurz", 10), "kurz", "short string untouched");
    }
}
