//! Das globale **Konto** (ADR 0004, Issue #90): genau **eine** app-weite Server-Identität für das
//! selbst-gehostete Forgejo/Gitea, einmal eingerichtet und für alle Produkte wiederverwendet.
//!
//! Bis #90 tippte die Einrichtungs-Zeremonie pro Produkt Server-Adresse + Zugangsdaten neu. Das
//! Konto zentralisiert das: eine **Server-Adresse** (app-level als JSON neben der Produkt-Registry,
//! #45) + ein Satz **Zugangsdaten** (Username + Token, host-keyed im OS-Keystore wie `credentials.rs`,
//! #22). Beim Speichern prüft `GET /api/v1/user` Verbindung + Token-Gültigkeit und liefert den
//! **Account-Namen** zurück (den wir für den Besitzer-Default brauchen). Das **Anlege-Recht** wird
//! bewusst **nicht** vorab geprüft — es wird ehrlich beim ersten Veröffentlichen durch
//! `forgejo::ensure_repo` mit präziser 403-Meldung durchgesetzt.
//!
//! Hauspattern (wie `setup.rs`, `forgejo.rs`, `registry.rs`): die **Entscheidungen** sind ein reiner,
//! totaler, tabellen-getesteter Kern ([`normalize_base_url`], [`interpret_user_status`]); die dünne,
//! isolierte Seiteneffekt-Schicht ([`crate::forgejo::verify_account`] für das Netz, die JSON-/Keystore-
//! Glue hier) ist der einzige Teil, der Netz/Platte/Keystore berührt. Tests treiben den Kern direkt
//! und berühren nie einen echten Server.
//!
//! **Kein Geheimnis verlässt das Backend Richtung Frontend:** [`read_konto`] und [`save_konto`] geben
//! nur Base-URL + Username zurück, **nie** den Token; der Token wird nirgends geloggt.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Dateiname der app-level Konto-Server-Adresse, im App-Config-Verzeichnis (neben der
/// Produkt-Registry, #45). JSON für einen ehrlichen, diffbaren, handlesbaren Eintrag (nur die
/// Base-URL — **nie** Zugangsdaten, die bleiben im OS-Keystore).
pub const KONTO_FILE: &str = "konto.json";

// ----------------------------------------------------------------------------------------------
// Reiner Kern 1 — Server-Adresse normalisieren
// ----------------------------------------------------------------------------------------------

/// Eine getippte Server-Adresse zu einer kanonischen Base-URL `scheme://host[:port]` normalisieren.
///
/// Rein und total: shellt nie aus, paniert nie, liefert auf schlechte Eingabe einen menschlichen
/// deutschen Fehlertext. Regeln (bewusst streng, damit ein Tippfehler hier laut scheitert, nicht
/// erst beim Speichern/Veröffentlichen) — gespiegelt aus `setup::normalize_remote`:
/// - Eine schema-lose Adresse wird auf `https://` (die Forgejo/Gitea-Norm) gesetzt.
/// - Nur `http(s)` ist erlaubt — `ssh`/`file`/etc. sind für ein Konto außer Reichweite.
/// - Ein abschließender Schrägstrich wird getrimmt.
/// - Eine Adresse mit Pfad (`host/pfad`), Leerraum im Host oder leerem Host wird abgelehnt.
pub fn normalize_base_url(input: &str) -> Result<String, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("Server-Adresse fehlt.".into());
    }

    // Schema abspalten *vor* dem Slash-Trimmen, damit `https://` (leerer Host) erkannt und nicht
    // mit einem schema-losen Host namens `https:` verwechselt wird.
    let (scheme, hostport) = match raw.split_once("://") {
        Some(("https", rest)) => ("https", rest),
        Some(("http", rest)) => ("http", rest),
        Some((other, _)) => return Err(format!("Nicht unterstütztes Protokoll: {other}://")),
        None => ("https", raw),
    };
    let hostport = hostport.trim_end_matches('/');
    if hostport.is_empty() || hostport.contains('/') || hostport.contains(char::is_whitespace) {
        return Err("Server-Adresse ist keine gültige Host-Adresse.".into());
    }
    Ok(format!("{scheme}://{hostport}"))
}

// ----------------------------------------------------------------------------------------------
// Reiner Kern 2 — die `GET /api/v1/user`-Antwort deuten
// ----------------------------------------------------------------------------------------------

/// Die Antwort von `GET /api/v1/user` (Status + Body) in den Account-Namen oder einen typisierten
/// deutschen Fehler deuten. Rein und total; berührt nie das Netz.
///
/// - **200** → der `login`-Account-Name aus dem JSON-Body (Forgejo/Gitea liefert ihn dort). Fehlt er,
///   ist das ein klarer „unerwartete Antwort"-Fehler statt eines stillen leeren Namens.
/// - **401** → Token falsch/abgelaufen. Die Meldung trägt den verbatim `401 unauthorized`-Marker,
///   damit `gitrunner::classify_failure` sie als [`crate::gitrunner::GitFailure::Auth`] einstuft und
///   das Frontend den Token-Eingabe-Schritt wieder öffnet (derselbe Pfad wie ein abgelehnter Push).
/// - **404 / sonst** → klare deutsche Meldung „Server-Adresse prüfen" (kein API-Endpoint dort).
pub fn interpret_user_status(code: u16, body: &str) -> Result<String, String> {
    match code {
        200 => account_name_from_body(body).ok_or_else(|| {
            "Unerwartete Antwort des Servers — Account-Name nicht lesbar. Server-Adresse prüfen."
                .to_string()
        }),
        401 => Err(format!(
            "401 unauthorized — Zugangs-Token wurde abgelehnt: {}",
            body.trim()
        )),
        403 => Err(format!(
            "403 forbidden — Zugangs-Token wurde abgelehnt: {}",
            body.trim()
        )),
        404 => Err(
            "Kein Forgejo/Gitea unter dieser Adresse gefunden — bitte Server-Adresse prüfen."
                .to_string(),
        ),
        other => Err(format!(
            "Server antwortete unerwartet (HTTP {other}) — bitte Server-Adresse prüfen."
        )),
    }
}

/// Den `login`-Account-Namen aus dem JSON-Body einer `GET /api/v1/user`-200-Antwort lesen. Pur,
/// total; `None`, wenn der Body kein lesbares JSON-Objekt mit einem nicht-leeren `login` ist.
fn account_name_from_body(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let login = value.get("login")?.as_str()?.trim();
    (!login.is_empty()).then(|| login.to_string())
}

// ----------------------------------------------------------------------------------------------
// Die Frontend-Sicht des Kontos (nie der Token)
// ----------------------------------------------------------------------------------------------

/// Was das Frontend über das Konto sieht: die normalisierte Base-URL und der angemeldete Account —
/// **nie** der Token. Ein fehlendes Konto ist `None`-felder-frei: dann wird gar kein [`KontoView`]
/// geliefert (das Command gibt `Option<KontoView>` zurück).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KontoView {
    /// Die normalisierte Server-Base-URL `scheme://host[:port]`.
    pub base_url: String,
    /// Der angemeldete Account-Name (aus `GET /api/v1/user`, beim Speichern bestätigt).
    pub account: String,
}

/// Die app-level persistierte Konto-Server-Adresse (nur die Base-URL + der zuletzt bestätigte
/// Account-Name). JSON-Form auf der Platte; **nie** der Token (der lebt im OS-Keystore).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KontoConfig {
    /// Normalisierte Base-URL `scheme://host[:port]`.
    pub base_url: String,
    /// Der bei der letzten Prüfung bestätigte Account-Name (Username) — der Besitzer-Default.
    pub account: String,
}

// ----------------------------------------------------------------------------------------------
// Dünne Datei-I/O-Glue über dem reinen Kern (Server-Adresse app-level als JSON)
// ----------------------------------------------------------------------------------------------

/// Den Konto-Dateipfad unter einem App-Config-Verzeichnis auflösen. Getrennt gehalten, damit die
/// Command-Schicht das Tauri-aufgelöste Verzeichnis reicht, Tests aber ein Temp-Verzeichnis nutzen.
pub fn konto_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(KONTO_FILE)
}

/// Die persistierte Konto-Konfiguration lesen. Eine fehlende/leere/kaputte Datei bedeutet **kein
/// Konto** (`None`) — nie ein Fehler (eine frische Installation hat noch keine Datei; eine
/// handverhunzte Datei darf das Konto-Panel nicht bricken).
pub fn read_konto(file: &Path) -> Option<KontoConfig> {
    let raw = std::fs::read_to_string(file).unwrap_or_default();
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

/// Die Konto-Konfiguration persistieren, pretty-printed (nur Base-URL + Account — diffbar/lesbar
/// halten). Legt das App-Config-Verzeichnis an, falls es noch nicht existiert. Schreibt **nie** den
/// Token (der ist Sache des OS-Keystores).
pub fn write_konto(file: &Path, config: &KontoConfig) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config).map_err(std::io::Error::other)?;
    std::fs::write(file, json)
}

/// Die persistierte Konto-Server-Adresse entfernen (ADR 0004, Issue #91, „Konto entfernen"). **Nur**
/// die app-level JSON-Datei — die Zugangsdaten im OS-Keystore und (kritisch) die `.git/config`-Remotes
/// vorhandener Produkte bleiben unangetastet (die Keystore-Löschung erledigt die Command-Schicht über
/// `credentials::delete`). **Idempotent**: eine bereits fehlende Datei ist Erfolg, kein Fehler — so
/// ist „Konto entfernen" ohne eingerichtetes Konto eine no-op.
pub fn clear_konto(file: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- normalize_base_url: der Adress-Normalisierungs-Kern ----

    #[test]
    fn normalize_base_url_accepts_and_canonicalizes() {
        // table: (input, expected normalized base-url)
        let cases: &[(&str, &str)] = &[
            // schema-lose Adresse -> https als Forgejo/Gitea-Default
            ("forge.example.de", "https://forge.example.de"),
            // expliziter Trailing-Slash getrimmt
            ("https://forge.example.de/", "https://forge.example.de"),
            ("forge.example.de/", "https://forge.example.de"),
            // umgebender Leerraum getrimmt
            ("  https://forge.example.de  ", "https://forge.example.de"),
            // explizites http mit Port bleibt erhalten
            ("http://192.168.0.9:3000", "http://192.168.0.9:3000"),
            ("https://h:3000/", "https://h:3000"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                normalize_base_url(input).as_deref(),
                Ok(*expected),
                "normalize_base_url({input:?})"
            );
        }
    }

    #[test]
    fn normalize_base_url_rejects_bad_input() {
        // table: input -> should error (leer, falsches Protokoll, leerer Host, Host-mit-Pfad,
        // Leerraum im Host).
        let bad: &[&str] = &[
            "",                      // leer
            "   ",                   // nur Leerraum
            "ssh://h",               // falsches Protokoll
            "file:///mirror",        // falsches Protokoll (für ein Konto)
            "https://",              // leerer Host
            "https://forge.de/team", // Host mit Pfad
            "https://forge .de",     // Leerraum im Host
        ];
        for input in bad {
            assert!(
                normalize_base_url(input).is_err(),
                "expected error for {input:?}"
            );
        }
    }

    // ---- interpret_user_status: die `GET /api/v1/user`-Deutung ----

    #[test]
    fn interpret_user_status_200_returns_account_name() {
        let body = r#"{"id":1,"login":"niklasonfire","full_name":"Niklas"}"#;
        assert_eq!(
            interpret_user_status(200, body),
            Ok("niklasonfire".to_string())
        );
        // umgebender Leerraum im login wird getrimmt
        assert_eq!(
            interpret_user_status(200, r#"{"login":"  anna  "}"#),
            Ok("anna".to_string())
        );
    }

    #[test]
    fn interpret_user_status_200_without_login_is_clear_error() {
        // 200, aber kein lesbarer Account-Name -> klarer Fehler statt stillem leeren Namen.
        for body in ["{}", r#"{"login":""}"#, "not json", ""] {
            let e = interpret_user_status(200, body).unwrap_err();
            assert!(e.contains("Server-Adresse prüfen"), "body {body:?}: {e}");
        }
    }

    #[test]
    fn interpret_user_status_401_403_carry_auth_marker() {
        // 401/403 müssen den Marker tragen, auf den `gitrunner::classify_failure` keyt, damit das
        // Frontend den Token-Schritt wieder öffnet statt einen generischen Fehler zu zeigen.
        let e401 = interpret_user_status(401, "bad token").unwrap_err();
        assert_eq!(
            crate::gitrunner::classify_failure(&e401),
            crate::gitrunner::GitFailure::Auth
        );
        let e403 = interpret_user_status(403, "forbidden").unwrap_err();
        assert_eq!(
            crate::gitrunner::classify_failure(&e403),
            crate::gitrunner::GitFailure::Auth
        );
    }

    #[test]
    fn interpret_user_status_404_and_other_point_at_the_server_address() {
        // 404 (kein API dort) und sonstige Codes sind KEIN Auth-Fehler — sie verweisen klar auf die
        // Server-Adresse, nicht auf den Token.
        let e404 = interpret_user_status(404, "<html>not found</html>").unwrap_err();
        assert!(e404.contains("Server-Adresse"));
        assert_ne!(
            crate::gitrunner::classify_failure(&e404),
            crate::gitrunner::GitFailure::Auth
        );
        let e500 = interpret_user_status(500, "boom").unwrap_err();
        assert!(e500.contains("Server-Adresse"));
        assert!(e500.contains("500"));
        assert_ne!(
            crate::gitrunner::classify_failure(&e500),
            crate::gitrunner::GitFailure::Auth
        );
    }

    // ---- Persistenz-Glue: Base-URL app-level als JSON, robust gegen Müll ----

    #[test]
    fn read_konto_missing_or_corrupt_reads_as_none() {
        let dir = std::env::temp_dir().join(format!("konto-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = konto_path(&dir);
        // fehlende Datei -> kein Konto
        assert_eq!(read_konto(&file), None);
        // leere Datei -> kein Konto
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, "   ").unwrap();
        assert_eq!(read_konto(&file), None);
        // kaputtes JSON -> kein Konto (kein Brick)
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(read_konto(&file), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_then_read_konto_round_trips_without_token() {
        let dir = std::env::temp_dir().join(format!("konto-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = konto_path(&dir);
        let config = KontoConfig {
            base_url: "https://forge.example.de".to_string(),
            account: "anna".to_string(),
        };
        write_konto(&file, &config).unwrap();
        assert_eq!(read_konto(&file), Some(config.clone()));
        // Die persistierte Datei trägt KEIN Token-Feld (nur Base-URL + Account).
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(raw.contains("base_url"));
        assert!(raw.contains("account"));
        assert!(!raw.to_lowercase().contains("token"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_konto_removes_the_file_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("konto-clear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = konto_path(&dir);

        // „entfernen" ohne eingerichtetes Konto ist eine no-op, kein Fehler (Kriterium 5).
        assert_eq!(read_konto(&file), None);
        clear_konto(&file).expect("clear on a missing file is success");
        assert_eq!(read_konto(&file), None);

        // Mit eingerichtetem Konto: clear entfernt die JSON-Datei → wieder „kein Konto".
        write_konto(
            &file,
            &KontoConfig {
                base_url: "https://forge.example.de".to_string(),
                account: "anna".to_string(),
            },
        )
        .unwrap();
        assert!(read_konto(&file).is_some());
        clear_konto(&file).expect("clear on an existing file succeeds");
        assert!(!file.exists(), "the konto JSON file must be gone after clear");
        assert_eq!(read_konto(&file), None);

        // Ein zweites „entfernen" ist erneut eine no-op (Idempotenz).
        clear_konto(&file).expect("repeated clear stays a no-op");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
