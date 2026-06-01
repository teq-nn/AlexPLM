# ADR 0004 — Globales Konto: eine app-weite Server-Identität statt Credentials pro Produkt

- **Status:** Akzeptiert
- **Datum:** 2026-06-01
- **Kontext:** Issue #87. Bis hierher tippte die Einrichtungs-Zeremonie (`setup.rs`, ADR/Issue #5)
  pro Produkt **fünf** Felder — Server-Adresse, Besitzer, Produkt-Name, Username, Passwort — und
  `configure_remote` schrieb die Zugangsdaten dabei in den OS-Keystore. Die Zugangsdaten lagen zwar
  schon **host-keyed** (Keystore-Key = Host-Origin, geteilt über alle Produkte desselben Servers),
  aber das **Eintippen** wiederholte sich bei jedem Produkt. Der Wunsch: einmal anmelden, diese eine
  Identität für jede Server-Aktion nutzen.

## Entscheidung

### Ein Konto, app-weit
Es gibt genau **ein Konto** (CONTEXT.md): eine **Server-Adresse** + ein Satz **Zugangsdaten**
(Username + Token), gültig für alle Produkte. Das Werkzeug spricht mit **einem** selbstgehosteten
Forgejo/Gitea. Persistenz: die Server-Adresse liegt app-level als JSON im Tauri-App-Config-Verzeichnis
(neben der Produkt-Registry, ADR/Issue #45); die Zugangsdaten bleiben im OS-Keystore (`credentials.rs`,
Issue #22), host-keyed unter dem Konto-Host — askpass und `forgejo::ensure_repo` finden sie unverändert.

### Konto ist der einzige Schreiber von Zugangsdaten
Zugangsdaten werden **ausschließlich** beim Konto-Speichern in den Keystore geschrieben. Die
Produkt-Zeremonie schreibt **keine** Credentials mehr: `configure_remote` wird credential-frei und
baut nur noch die credential-freie Clone-URL aus Konto-Base-URL + owner/repo und setzt das Remote
(+ `locksverify`). `normalize_remote` verliert die `user`/`token`-Parameter.

### Produkt nennt nur owner/repo
„Server anbinden" fragt nur noch **Besitzer/Team** (optional) + **Produkt-Name**. Ein angegebener
Besitzer (z. B. eine Organisation) gilt verbatim; leer → der Standard-Account des Konto-Users. Ohne
eingerichtetes Konto **verweigert sich die Zeremonie** und verweist auf die Einstellungen — kein
Credential-Feld pro Produkt mehr.

### Prüfung beim Speichern: nur Verbindung + Token-Gültigkeit
Beim Konto-Speichern wird `GET /api/v1/user` aufgerufen: 200 bestätigt Verbindung + gültigen Token
und liefert den **Account-Namen** (den wir für den Besitzer-Default brauchen); 401 → Token falsch,
Netzfehler → Server-Adresse prüfen. Das **Anlege-Recht** wird **nicht** vorab geprüft — Forgejo gibt
weder Token-Scopes noch `max_repo_creation` über einen einfachen Endpoint preis. Es wird ohnehin beim
ersten Veröffentlichen durch `forgejo::ensure_repo` **idempotent** und mit präziser 403-Meldung
durchgesetzt.

### Konto ist von bestehenden Produkt-Remotes entkoppelt
Konto **ändern/entfernen** rührt die `.git/config`-Remotes vorhandener Produkte **nie** an (kein
automatisches Umbiegen, kein Massen-Repoint). Das Konto setzt nur die Base-URL für *künftige*
„Server anbinden"-Schritte und die Zugangsdaten für den *aktuellen* Host. Ein Server-Adress-Wechsel
ist damit ein bewusster, seltener Akt; „Konto entfernen" löscht Keystore-Eintrag + Base-URL, lässt
Produkte unangetastet (lokales Arbeiten läuft weiter, nur Teilen pausiert).

### Lokales Arbeiten braucht kein Konto (Just-in-time)
Der gesamte vernetzte Rhythmus ist bereits hinter `setup::is_published` gegated (`syncglue.rs`,
`lockglue.rs`, `pushglue.rs`). Produkt anlegen, importieren, Bausteine/Stack wählen, arbeiten,
Auto-Commit-Stände, Versionsbaum, Aufgaben, Werkbank, Suche laufen **ohne** Konto. Das Konto wird
erst im **Teilen-Moment** nötig. Folglich: **kein Login-Wall beim Start**; das Konto-Panel ist über
ein Zahnrad im Header erreichbar, und der „Kein Konto"-Redirect sitzt am Öffnen der Zeremonie.

## Begründung

- **Host-keyed Keystore war schon da** — die globale Identität ist daher kein neuer Sicherheits-Pfad,
  sondern entfernt nur das wiederholte Eintippen und zentralisiert den *einen* Schreiber.
- **Ehrliche Prüfung** — ein Fake-„Rechte ok", das beim Push doch 403 wirft, wäre schlechter als die
  klare Trennung „Token gültig (jetzt) / Anlege-Recht ehrlich beim ersten Push".
- **Entkopplung** vermeidet stille Massen-Beschädigung von Remotes bei einem Server-Wechsel und hält
  das Konto als reine *Vorlage für künftige* Anbindungen verständlich.

## Verworfene Alternativen

- **Mehrere Konten / Server gleichzeitig** — verworfen: das Glossar dreht sich um ein Produkt-Universum
  auf einem Server; der host-keyed Keystore lässt Mehr-Server technisch später offen, ohne die UX jetzt
  zu belasten.
- **Produktlokale Credentials als Rückfall behalten** — verworfen: widerspricht „global, not per repo"
  und hielte zwei Credential-Schreiber am Leben.
- **Probe-Anlegeversuch beim Speichern** (Test-Repo anlegen + löschen) — verworfen: Seiteneffekt auf
  dem Server, fragil, und garantiert das Recht nur für *diesen* Owner zu *diesem* Zeitpunkt.
- **Konto-Wechsel zieht bestehende Produkt-Remotes mit** — verworfen: implizites Umbiegen ist
  überraschend und schwer rückgängig; Repoint verwaister Produkte ist ein eigenes Thema (Issue #89).
- **Login-Wall beim ersten Start** — verworfen: lokales Arbeiten braucht nachweislich kein Konto.

## Konsequenzen

- Neues Modul für das Konto (Server-Adresse-Persistenz + Forgejo-Verify), neue Tauri-Commands
  (`read_konto`/`save_konto`/`clear_konto`), neues Settings/Konto-Panel im Frontend + Zahnrad im Header.
- `setup::normalize_remote` verliert `user`/`token`; `configure_remote`/`connect_server` werden
  credential-frei und beziehen Base-URL + Besitzer-Default-Username aus dem Konto.
- `EinrichtungsZeremonie.svelte`: Credential-Felder entfallen; „Zugangsdaten ändern" zeigt aufs
  Konto-Panel; ohne Konto Hinweis + Redirect statt Eingabe.
- Bestehende, pro Produkt gespeicherte Zugangsdaten auf demselben Host bleiben gültig (host-keyed); ein
  Produkt auf einem anderen Host als das Konto scheitert beim Teilen ehrlich (typisierter Auth-Fehler).
