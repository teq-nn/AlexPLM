//! Aus der **laufenden** Werkbank ein Issue ins Produkt-Repository melden (Issue #85).
//!
//! Warum: Probleme/Wünsche sollen direkt aus der App im Git-Server als Issue landen — mit einem
//! Etikett, das sie als „aus der Laufzeit gemeldet" kenntlich macht, und optional dem Diagnose-Log
//! angehängt (`gitlog`-Ring), damit ein Bericht reproduzierbar wird. Ziel ist das **Repository des
//! gerade geöffneten Produkts** (Forgejo/Gitea-Remote `origin`); authentifiziert mit denselben
//! Konto-Zugangsdaten, die Push/Locks nutzen (OS-Keystore, host-keyed) — der Nutzer tippt nichts
//! Neues. **Kein Token wird je geloggt.**
//!
//! Hauspattern (wie `forgejo.rs`, `konto.rs`): die **Entscheidungen** sind ein reiner, totaler,
//! tabellen-getesteter Kern ([`repo_coords_from_remote`], [`issues_endpoint`], [`labels_endpoint`],
//! [`compose_issue_body`], [`find_label_id`], [`interpret_create_issue`]); die dünne, isolierte
//! Seiteneffekt-Schicht ([`submit_issue`]) ist der einzige Teil, der das Netz berührt. Tests treiben
//! den Kern direkt und berühren nie einen Server.

use std::io::{Error, ErrorKind};
use std::time::Duration;

/// Wall-clock-Schranke für die Issue-/Label-Calls — gespiegelt aus `forgejo::API_TIMEOUT`, damit ein
/// hängender Server den (off-main-thread laufenden) Melde-Pfad nie länger als nötig blockiert.
const API_TIMEOUT: Duration = Duration::from_secs(20);

/// Etikettname, der ein Issue als „aus der laufenden App gemeldet" kenntlich macht. Genau dieser
/// Name wird im Repo gesucht bzw. angelegt, damit jedes Laufzeit-Issue dasselbe Etikett trägt.
pub const RUNTIME_LABEL_NAME: &str = "aus-der-laufzeit";
/// Etikettfarbe (Forgejo erwartet `#rrggbb`). Ein warmes Bernstein, das sich von den üblichen
/// Status-Etiketten abhebt.
const RUNTIME_LABEL_COLOR: &str = "#d97706";
/// Beschreibung des Etiketts, einmalig beim Anlegen gesetzt.
const RUNTIME_LABEL_DESC: &str = "Aus der laufenden Werkbank gemeldet";

// ----------------------------------------------------------------------------------------------
// Reiner Kern 1 — Repo-Koordinaten aus der Remote-URL
// ----------------------------------------------------------------------------------------------

/// Aus der `origin`-Remote-URL des Produkts `(host_url, owner, repo)` lesen — die Adresse, an die das
/// Issue geht, und der Host-Schlüssel, unter dem die Konto-Zugangsdaten im Keystore liegen. Rein und
/// total: `None` für eine Form, die wir nicht deuten können.
///
/// Seit ADR 0004 / Issue #92 sind Remotes **credential-frei** geschrieben; trotzdem wird ein evtl.
/// `user[:token]@`-Userinfo defensiv abgestreift, damit der Host-Schlüssel exakt dem entspricht, unter
/// dem `credentials::store` ablegt (`scheme://host[:port]`, ohne Userinfo).
pub fn repo_coords_from_remote(url: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    // Userinfo (vor dem ersten '@', sofern dieses vor dem ersten Pfad-'/' steht) abstreifen.
    let after_userinfo = match rest.split_once('@') {
        Some((cred, tail)) if !cred.contains('/') => tail,
        _ => rest,
    };
    let (hostport, path) = after_userinfo.split_once('/')?;
    if hostport.is_empty() {
        return None;
    }
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.rsplit_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((format!("{scheme}://{hostport}"), owner.to_string(), repo.to_string()))
}

// ----------------------------------------------------------------------------------------------
// Reiner Kern 2 — Endpunkte
// ----------------------------------------------------------------------------------------------

/// Der Issue-Erstellungs-Endpunkt. Rein.
pub fn issues_endpoint(host_url: &str, owner: &str, repo: &str) -> String {
    format!("{}/api/v1/repos/{owner}/{repo}/issues", host_url.trim_end_matches('/'))
}

/// Der Label-Listen-/Erstellungs-Endpunkt (gleiche URL, GET listet, POST legt an). Rein.
pub fn labels_endpoint(host_url: &str, owner: &str, repo: &str) -> String {
    format!("{}/api/v1/repos/{owner}/{repo}/labels", host_url.trim_end_matches('/'))
}

/// Die Listen-URL für GET: derselbe Endpunkt mit `?limit=100`, damit auch ein Repo mit vielen
/// Etiketten in einer Antwort kommt (Forgejo paginiert sonst auf die Default-Seite). Rein. Das POST
/// zum Anlegen nutzt weiter den blanken [`labels_endpoint`] (keine Query).
fn labels_list_url(host_url: &str, owner: &str, repo: &str) -> String {
    format!("{}?limit=100", labels_endpoint(host_url, owner, repo))
}

// ----------------------------------------------------------------------------------------------
// Reiner Kern 3 — den Issue-Body zusammensetzen
// ----------------------------------------------------------------------------------------------

/// Obergrenze der ans Issue angehängten Diagnose-Zeilen: der **jüngste Schwanz**, nicht das ganze
/// Session-Protokoll. Genug Kontext für „was gerade passierte" — inklusive der stillen Warden-
/// „Refuse"-Spur, die KEIN Fehler ist (genau die häufigste Verwirrung: ein Push, der schweigend
/// nichts tat) — ohne das Issue mit bis zu 1000 ruhigen Zeilen zu fluten. Der volle Ring bleibt im
/// Diagnose-Panel und in der Logdatei.
const LOG_TAIL_MAX: usize = 100;

/// Den Markdown-Body des Issues bauen: die Beschreibung des Nutzers, eine Herkunfts-Fußzeile und —
/// wenn `log` gegeben und nicht leer — das Diagnose-Log als eingezäunter Codeblock. Rein und total;
/// eine leere Beschreibung wird durch einen Platzhalter ersetzt, damit der Body nie leer ist.
///
/// Angehängt wird nur der jüngste Schwanz (höchstens [`LOG_TAIL_MAX`] nicht-leere Zeilen); werden
/// ältere Zeilen weggelassen, weist eine Elisions-Marke ihre Zahl aus — **kein stilles Kappen**.
pub fn compose_issue_body(beschreibung: &str, log: Option<&[String]>) -> String {
    let beschreibung = beschreibung.trim();
    let mut out = if beschreibung.is_empty() {
        "_(keine Beschreibung angegeben)_".to_string()
    } else {
        beschreibung.to_string()
    };
    out.push_str("\n\n---\n_Gemeldet aus der laufenden Werkbank._");
    if let Some(lines) = log {
        let lines: Vec<&str> =
            lines.iter().map(|l| l.as_str()).filter(|l| !l.trim().is_empty()).collect();
        if !lines.is_empty() {
            out.push_str("\n\n## Diagnose-Log\n\n```\n");
            // Nur der jüngste Schwanz — ältere Zeilen werden ehrlich als Marke ausgewiesen.
            let dropped = lines.len().saturating_sub(LOG_TAIL_MAX);
            if dropped > 0 {
                out.push_str(&format!("… {dropped} ältere Zeile(n) ausgelassen …\n"));
            }
            out.push_str(&lines[dropped..].join("\n"));
            out.push_str("\n```\n");
        }
    }
    out
}

// ----------------------------------------------------------------------------------------------
// Reiner Kern 4 — Antworten deuten
// ----------------------------------------------------------------------------------------------

/// Die Id eines Etiketts mit Namen `name` (case-insensitiv) aus einem GET-`/labels`-Listen-Body
/// lesen. Rein, total; `None`, wenn der Body kein JSON-Array ist oder das Etikett fehlt.
pub fn find_label_id(list_body: &str, name: &str) -> Option<i64> {
    let value: serde_json::Value = serde_json::from_str(list_body).ok()?;
    let target = name.trim().to_ascii_lowercase();
    value.as_array()?.iter().find_map(|label| {
        let n = label.get("name")?.as_str()?.trim().to_ascii_lowercase();
        if n == target {
            label.get("id")?.as_i64()
        } else {
            None
        }
    })
}

/// Die `id` aus dem JSON-Body einer Label-Antwort (POST-201 eines angelegten Etiketts). Rein, total.
pub fn label_id_from_body(body: &str) -> Option<i64> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("id")?.as_i64()
}

/// Ein Etikett, wie es der Etiketten-Picker im Formular zeigt (Issue #85): Id (zum Anhängen), Name
/// und Farbe (Hex **ohne** führendes `#`, wie Forgejo/Gitea sie liefert — das Frontend setzt das
/// `#` zum Rendern davor). Kein Token, keine Server-Interna.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
}

/// Die Etiketten-Liste aus dem GET-`/labels`-Body lesen. Rein, total; `None`, wenn der Body kein
/// JSON-Array ist. Einträge ohne lesbare `id`/`name` werden übersprungen; die Farbe wird zu Hex ohne
/// führendes `#` normalisiert (leer, wenn keine da ist).
fn parse_labels(body: &str) -> Option<Vec<Label>> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(|l| {
                let id = l.get("id")?.as_i64()?;
                let name = l.get("name")?.as_str()?.trim().to_string();
                let color = l
                    .get("color")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches('#')
                    .to_string();
                (!name.is_empty()).then_some(Label { id, name, color })
            })
            .collect(),
    )
}

/// Status + Body der Etiketten-Liste in die Etiketten oder einen typisierten deutschen Fehler deuten.
/// Rein, total. Auth-Fehler tragen den verbatim Marker (wie [`interpret_create_issue`]).
pub fn interpret_labels_list(code: u16, body: &str) -> Result<Vec<Label>, String> {
    match code {
        200 => parse_labels(body).ok_or_else(|| {
            "Unerwartete Antwort des Servers — Etiketten nicht lesbar.".to_string()
        }),
        401 => Err(format!("401 unauthorized: {}", body.trim())),
        403 => Err(format!(
            "403 forbidden — keine Berechtigung, Etiketten zu lesen: {}",
            body.trim()
        )),
        404 => Err(format!(
            "Produkt-Repository auf dem Server nicht gefunden: {}",
            body.trim()
        )),
        other => Err(format!(
            "Konnte die Etiketten nicht laden (HTTP {other}): {}",
            body.trim()
        )),
    }
}

/// Eine Referenz auf das angelegte Issue, wie sie ans Frontend geht (für die „angelegt"-Bestätigung
/// mit Link). **Kein** Token, keine Server-Interna.
#[derive(specta::Type, Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IssueRef {
    /// Die Issue-Nummer im Repo (`#<number>`).
    pub number: u64,
    /// Die Web-URL des Issues, zum Öffnen im Browser.
    pub html_url: String,
}

/// Eine Issue-Referenz aus dem JSON-Body einer Erstellungs-Antwort lesen. Rein, total.
fn issue_ref_from_body(body: &str) -> Option<IssueRef> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let number = value.get("number")?.as_u64()?;
    let html_url = value.get("html_url")?.as_str()?.trim().to_string();
    (!html_url.is_empty()).then_some(IssueRef { number, html_url })
}

/// Status + Body der Issue-Erstellung in Erfolg oder einen typisierten deutschen Fehler deuten. Rein,
/// total. Auth-Fehler (401/403) tragen den verbatim Marker, auf den `gitrunner::classify_failure`
/// keyt, damit ein abgelehntes Token denselben Pfad nimmt wie ein abgelehnter Push.
pub fn interpret_create_issue(code: u16, body: &str) -> Result<IssueRef, String> {
    match code {
        200 | 201 => issue_ref_from_body(body).ok_or_else(|| {
            "Unerwartete Antwort des Servers — Issue-Nummer nicht lesbar.".to_string()
        }),
        401 => Err(format!("401 unauthorized: {}", body.trim())),
        403 => Err(format!(
            "403 forbidden — keine Berechtigung, hier ein Issue anzulegen: {}",
            body.trim()
        )),
        404 => Err(format!(
            "Produkt-Repository auf dem Server nicht gefunden: {}",
            body.trim()
        )),
        other => Err(format!(
            "Konnte das Problem nicht melden (HTTP {other}): {}",
            body.trim()
        )),
    }
}

// ----------------------------------------------------------------------------------------------
// Request-Bodies
// ----------------------------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct CreateIssueBody<'a> {
    title: &'a str,
    body: &'a str,
    labels: Vec<i64>,
}

#[derive(serde::Serialize)]
struct CreateLabelBody<'a> {
    name: &'a str,
    color: &'a str,
    description: &'a str,
}

// ----------------------------------------------------------------------------------------------
// Dünne Seiteneffekt-Schicht — der einzige Teil, der das Netz berührt
// ----------------------------------------------------------------------------------------------

/// Sicherstellen, dass das Laufzeit-Etikett im Repo existiert, und seine Id liefern. **Best-effort:**
/// das Anlegen eines Issues darf an einem Etikett-Schluckauf nicht scheitern, also wird jeder Fehler
/// hier zu `None` (das Issue geht dann ohne Etikett raus — ein echter Auth-Fehler taucht ohnehin
/// beim Issue-Erstellen wieder auf). Erst GET (Etikett schon da?), sonst POST (anlegen), bei Konflikt
/// (Wettlauf/„existiert bereits") erneut GET.
fn ensure_runtime_label(
    client: &reqwest::blocking::Client,
    host_url: &str,
    owner: &str,
    repo: &str,
    user: &str,
    token: &str,
) -> Option<i64> {
    let endpoint = labels_endpoint(host_url, owner, repo);
    let list_url = labels_list_url(host_url, owner, repo);
    let fetch = |c: &reqwest::blocking::Client| -> Option<i64> {
        let resp = c.get(&list_url).basic_auth(user, Some(token)).send().ok()?;
        if !resp.status().is_success() {
            return None;
        }
        find_label_id(&resp.text().unwrap_or_default(), RUNTIME_LABEL_NAME)
    };
    if let Some(id) = fetch(client) {
        return Some(id);
    }
    // Nicht gefunden — anlegen.
    if let Ok(resp) = client
        .post(&endpoint)
        .basic_auth(user, Some(token))
        .json(&CreateLabelBody {
            name: RUNTIME_LABEL_NAME,
            color: RUNTIME_LABEL_COLOR,
            description: RUNTIME_LABEL_DESC,
        })
        .send()
    {
        let body = resp.text().unwrap_or_default();
        if let Some(id) = label_id_from_body(&body) {
            return Some(id);
        }
    }
    // Konflikt/Wettlauf: ein anderer Pfad hat es evtl. gerade angelegt — erneut lesen.
    fetch(client)
}

/// Die Etiketten des Repos für den Picker im Formular lesen (Issue #85). GET `/labels?limit=100`,
/// authentifiziert mit den Konto-Zugangsdaten. Fehler reiten als `io::Error` hoch (auth-klassifiziert),
/// damit ein abgelehntes Token denselben Pfad nimmt wie sonst. Der Token wird nie geloggt.
pub fn list_labels(
    host_url: &str,
    owner: &str,
    repo: &str,
    user: &str,
    token: &str,
) -> std::io::Result<Vec<Label>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, format!("HTTP-Client-Fehler: {e}")))?;
    let resp = client
        .get(labels_list_url(host_url, owner, repo))
        .basic_auth(user, Some(token))
        .send()
        .map_err(|e| Error::new(ErrorKind::Other, format!("Server nicht erreichbar: {e}")))?;
    let code = resp.status().as_u16();
    let body = resp.text().unwrap_or_default();
    interpret_labels_list(code, &body).map_err(|m| Error::new(ErrorKind::Other, m))
}

/// Ein Issue ins Produkt-Repo melden: das Laufzeit-Etikett sicherstellen (best-effort) und das Issue
/// mit Titel + Body anlegen. `extra_labels` sind die im Formular gewählten Etikett-Ids — sie werden
/// mit der Laufzeit-Etikett-Id zusammengeführt (dedupliziert). Authentifiziert per Basic-Auth mit den
/// Konto-Zugangsdaten (ein Forgejo-Token ist API-Credential und git-Passwort zugleich). Fehler reiten
/// als `io::Error` hoch, damit die Command-Schicht sie über den bestehenden typisierten `AppError`-Pfad
/// einstuft (ein schlechtes Token öffnet das Token-Feld wieder). Der Token wird nie geloggt.
pub fn submit_issue(
    host_url: &str,
    owner: &str,
    repo: &str,
    user: &str,
    token: &str,
    title: &str,
    body: &str,
    extra_labels: &[i64],
) -> std::io::Result<IssueRef> {
    let client = reqwest::blocking::Client::builder()
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, format!("HTTP-Client-Fehler: {e}")))?;
    let labels = merge_labels(
        extra_labels,
        ensure_runtime_label(&client, host_url, owner, repo, user, token),
    );
    let resp = client
        .post(issues_endpoint(host_url, owner, repo))
        .basic_auth(user, Some(token))
        .json(&CreateIssueBody { title, body, labels })
        .send()
        .map_err(|e| Error::new(ErrorKind::Other, format!("Server nicht erreichbar: {e}")))?;
    let code = resp.status().as_u16();
    let body = resp.text().unwrap_or_default();
    interpret_create_issue(code, &body).map_err(|m| Error::new(ErrorKind::Other, m))
}

/// Die gewählten Etikett-Ids mit der (optionalen) Laufzeit-Etikett-Id zusammenführen, Reihenfolge
/// wahrend dedupliziert. Rein, total — der Kern getrennt von der Netz-Schicht für einen direkten Test.
fn merge_labels(extra: &[i64], runtime: Option<i64>) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::with_capacity(extra.len() + 1);
    for &id in extra.iter().chain(runtime.iter()) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_coords_splits_host_owner_repo() {
        assert_eq!(
            repo_coords_from_remote("https://forgejo.nilius.online/alice/gizmo.git"),
            Some((
                "https://forgejo.nilius.online".to_string(),
                "alice".to_string(),
                "gizmo".to_string()
            ))
        );
    }

    #[test]
    fn repo_coords_keeps_port_and_optional_git_suffix() {
        assert_eq!(
            repo_coords_from_remote("http://h:3000/org/repo"),
            Some(("http://h:3000".to_string(), "org".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn repo_coords_strips_userinfo_so_host_key_matches_keystore() {
        // A credential-bearing remote (legacy) must still yield the bare host origin, so the
        // keystore lookup key matches what `credentials::store` wrote (no userinfo).
        assert_eq!(
            repo_coords_from_remote("https://user:tok@h/org/repo.git"),
            Some(("https://h".to_string(), "org".to_string(), "repo".to_string()))
        );
        assert_eq!(
            repo_coords_from_remote("https://user@h/org/repo"),
            Some(("https://h".to_string(), "org".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn repo_coords_rejects_malformed() {
        assert_eq!(repo_coords_from_remote("not a url"), None);
        assert_eq!(repo_coords_from_remote("https://hostonly"), None);
        assert_eq!(repo_coords_from_remote("https://h/justrepo.git"), None);
        assert_eq!(repo_coords_from_remote("https:///org/repo.git"), None);
    }

    #[test]
    fn endpoints_are_well_formed() {
        assert_eq!(
            issues_endpoint("https://h/", "o", "r"),
            "https://h/api/v1/repos/o/r/issues"
        );
        assert_eq!(
            labels_endpoint("https://h", "o", "r"),
            "https://h/api/v1/repos/o/r/labels"
        );
    }

    #[test]
    fn compose_body_without_log_carries_description_and_footer() {
        let body = compose_issue_body("  Knopf tut nichts  ", None);
        assert!(body.starts_with("Knopf tut nichts"));
        assert!(body.contains("Gemeldet aus der laufenden Werkbank."));
        assert!(!body.contains("## Diagnose-Log"));
    }

    #[test]
    fn compose_body_empty_description_gets_placeholder() {
        let body = compose_issue_body("   ", None);
        assert!(body.contains("keine Beschreibung"));
    }

    #[test]
    fn compose_body_with_log_appends_fenced_block_skipping_blank_lines() {
        let log = vec![
            "12:00:00 [git] push ok".to_string(),
            "   ".to_string(),
            "12:00:01 [warden] Refuse".to_string(),
        ];
        let body = compose_issue_body("kaputt", Some(&log));
        assert!(body.contains("## Diagnose-Log"));
        assert!(body.contains("```"));
        assert!(body.contains("12:00:00 [git] push ok"));
        assert!(body.contains("12:00:01 [warden] Refuse"));
    }

    #[test]
    fn compose_body_caps_log_to_recent_tail_with_honest_elision() {
        // 250 non-blank lines → only the newest LOG_TAIL_MAX are attached, the rest are flagged.
        let log: Vec<String> = (0..250).map(|i| format!("12:00:00 [git] line {i}")).collect();
        let body = compose_issue_body("zu viel", Some(&log));
        let dropped = 250 - LOG_TAIL_MAX;
        assert!(body.contains(&format!("… {dropped} ältere Zeile(n) ausgelassen …")));
        // Newest line kept; the oldest (and the last dropped one) are gone.
        assert!(body.contains("[git] line 249"));
        assert!(body.contains(&format!("[git] line {}", dropped))); // first kept line
        assert!(!body.contains("[git] line 0"));
        assert!(!body.contains(&format!("[git] line {}", dropped - 1))); // last dropped line
    }

    #[test]
    fn compose_body_under_the_cap_has_no_elision_marker() {
        let log: Vec<String> = (0..5).map(|i| format!("[git] {i}")).collect();
        let body = compose_issue_body("ok", Some(&log));
        assert!(!body.contains("ausgelassen"));
        assert!(body.contains("[git] 0"));
        assert!(body.contains("[git] 4"));
    }

    #[test]
    fn compose_body_with_empty_log_omits_the_block() {
        // An all-blank (or empty) log must not produce a dangling, empty Diagnose-Log section.
        assert!(!compose_issue_body("x", Some(&[])).contains("Diagnose-Log"));
        assert!(!compose_issue_body("x", Some(&["  ".to_string()])).contains("Diagnose-Log"));
    }

    #[test]
    fn find_label_id_matches_case_insensitively() {
        let list = r#"[{"id":3,"name":"bug"},{"id":7,"name":"Aus-Der-Laufzeit"}]"#;
        assert_eq!(find_label_id(list, RUNTIME_LABEL_NAME), Some(7));
        assert_eq!(find_label_id(list, "bug"), Some(3));
        assert_eq!(find_label_id(list, "missing"), None);
        assert_eq!(find_label_id("not json", RUNTIME_LABEL_NAME), None);
    }

    #[test]
    fn parse_labels_reads_id_name_color_and_normalizes_hash() {
        let body = r##"[
            {"id":3,"name":"bug","color":"#e11d48"},
            {"id":7,"name":" aus-der-laufzeit ","color":"d97706"},
            {"id":9,"name":"ohne Farbe"}
        ]"##;
        let labels = parse_labels(body).unwrap();
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0], Label { id: 3, name: "bug".into(), color: "e11d48".into() });
        // name getrimmt, Farbe ohne führendes '#'
        assert_eq!(labels[1], Label { id: 7, name: "aus-der-laufzeit".into(), color: "d97706".into() });
        // fehlende Farbe → leer, kein Eintrag fällt weg
        assert_eq!(labels[2], Label { id: 9, name: "ohne Farbe".into(), color: "".into() });
    }

    #[test]
    fn parse_labels_skips_unreadable_entries_and_rejects_non_arrays() {
        // Ein Eintrag ohne id/name wird übersprungen, nicht der ganze Aufruf verworfen.
        let body = r#"[{"name":"keine id"},{"id":5,"name":"ok"}]"#;
        let labels = parse_labels(body).unwrap();
        assert_eq!(labels, vec![Label { id: 5, name: "ok".into(), color: "".into() }]);
        assert_eq!(parse_labels("{}"), None);
        assert_eq!(parse_labels("not json"), None);
    }

    #[test]
    fn interpret_labels_list_auth_and_ok() {
        assert!(interpret_labels_list(200, "[]").unwrap().is_empty());
        let e401 = interpret_labels_list(401, "bad").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e401), crate::gitrunner::GitFailure::Auth);
    }

    #[test]
    fn merge_labels_dedups_and_appends_runtime_last_preserving_order() {
        assert_eq!(merge_labels(&[3, 1], Some(7)), vec![3, 1, 7]);
        // Laufzeit-Id schon gewählt → nicht doppelt.
        assert_eq!(merge_labels(&[7, 3], Some(7)), vec![7, 3]);
        // Doppelte Auswahl wird entdoppelt; ohne Laufzeit-Id bleibt nur die Auswahl.
        assert_eq!(merge_labels(&[3, 3, 1], None), vec![3, 1]);
        assert_eq!(merge_labels(&[], Some(7)), vec![7]);
        assert!(merge_labels(&[], None).is_empty());
    }

    #[test]
    fn labels_list_url_adds_limit_query() {
        assert_eq!(
            labels_list_url("https://h", "o", "r"),
            "https://h/api/v1/repos/o/r/labels?limit=100"
        );
    }

    #[test]
    fn label_id_from_body_reads_id() {
        assert_eq!(label_id_from_body(r#"{"id":42,"name":"x"}"#), Some(42));
        assert_eq!(label_id_from_body("{}"), None);
        assert_eq!(label_id_from_body("nope"), None);
    }

    #[test]
    fn interpret_create_issue_201_returns_ref() {
        let ok = interpret_create_issue(
            201,
            r#"{"number":12,"html_url":"https://h/o/r/issues/12"}"#,
        )
        .unwrap();
        assert_eq!(ok, IssueRef { number: 12, html_url: "https://h/o/r/issues/12".to_string() });
    }

    #[test]
    fn interpret_create_issue_201_without_fields_is_clear_error() {
        assert!(interpret_create_issue(201, "{}").is_err());
        assert!(interpret_create_issue(201, "not json").is_err());
    }

    #[test]
    fn interpret_create_issue_auth_errors_carry_classifiable_markers() {
        let e401 = interpret_create_issue(401, "bad token").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e401), crate::gitrunner::GitFailure::Auth);
        let e403 = interpret_create_issue(403, "no perms").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e403), crate::gitrunner::GitFailure::Auth);
    }

    #[test]
    fn interpret_create_issue_other_is_not_auth() {
        let e = interpret_create_issue(500, "boom").unwrap_err();
        assert_eq!(crate::gitrunner::classify_failure(&e), crate::gitrunner::GitFailure::Other);
        assert!(e.contains("500"));
    }
}
