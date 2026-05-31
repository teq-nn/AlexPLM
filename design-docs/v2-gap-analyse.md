# v1 ↔ Design-Docs — Lückenanalyse & was als Nächstes kommt (v2)

Stand: 30.05.2026. Diese Analyse vergleicht den **gebauten v1-Stand** (`app/`, Tauri v2 + Svelte +
Rust, Issues #2–#28 + #22/#24/#25/#26 abgeschlossen) mit dem, was die Design-Docs verlangen
(PRD #1, `entscheidungslog-4.md` E34–E41, `glossar-4.md`, `ui-stilbeschreibung.md`, ADR 0001). Ziel:
ehrlich benennen, **was steht**, **was hohl ist**, **was ganz fehlt**, und daraus die **v2-Grill- und
Bau-Liste** ableiten. Geprüft gegen die fünf Leitregeln (E9, E27, E18, E26/E30, E33).

---

## 1. Was steht (v1 ist tragfähig und doc-treu)

Die Architektur-Grundlinie der PRD — *dünne, reine Logik-Kerne („deep modules") + dumme,
ausführende Glue* — ist sauber umgesetzt. Alle in der PRD genannten Kerne existieren und sind über
ihre Schnittstelle (Snapshot rein, Entscheidung raus, kein I/O) tabellen-/eigenschaftsgetestet:

| PRD-Modul | Rust-Datei | Zustand | Tests |
|---|---|---|---|
| Mergeability Classifier | `classifier.rs` | ✅ vollständig, 3 Eimer inkl. KiCad + `.gitattributes`-Override | inline + `import_clean_init.rs` |
| Lock Warden (Binär-Invariante) | `warden.rs` | ✅ voller Kreuzprodukt-Kern, Invariante als Property-Test verankert | inline + `push_warden.rs` |
| Sync Decider (laute Ausnahme) | `syncdecider.rs` | ✅ silent-merge vs. loud-exception, „kein git-Marker"-Säuretest | inline + `sync_decider.rs` |
| Import Gate (`lfs migrate`) | `import_gate.rs` | ✅ clean-init / migrate-behind-gate / refuse, „geteilt ⇒ refuse" | `import_gate_io.rs` |
| Graph Projection | `graph.rs`, `graphread.rs`, `projection.rs` | ✅ Stände/Meilensteine/Branches (#28), offloaded-Marker *vorbereitet* | `graph_projection.rs`, `projection_walk.rs` |
| Edge Logic | `edges.rs`, `edgestore.rs` | ✅ manuelle Kanten + Stale „nur wo Kante" | `edges_stale.rs` |
| Watcher / Auto-Commit | `watcher.rs`, `autocommit.rs` | ✅ entprellt, maschinen-betextet, „Stand"-Event | `autocommit_watch.rs` |
| Status Reader | `locks.rs`, `lockglue.rs` | ✅ LED-Status + Fremd-Sperren aus `git lfs locks` | `status_reader.rs` |
| Vault / git-Runner | `gitrunner.rs` + *-glue | ✅ gehärtet: `GIT_TERMINAL_PROMPT=0`, Askpass, Netz-Timeout | (über Glue-Tests) |
| Einrichtungs-Zeremonie | `setup.rs`, `forgejo.rs`, `credentials.rs`, `askpass.rs` | ✅ connect/publish/locksverify, Token im OS-Keystore (#22) | `setup_ceremony.rs`, `auth_keyring.rs` |
| UI Shell | `app/src/**` | ✅ warm-graues Instrument, Tokens, LED, dunkle Display-Zonen, Versionsbaum-SVG | — |

**Besonders stark:** Die tragende Binär-Invariante (E35) ist nicht nur Prosa, sondern als
*erschöpfender Property-Test* über das ganze Kreuzprodukt verankert (`warden.rs`:
`binaer_invariante_*`). Die git-Sprache wird im Alltag konsequent versteckt (E33/E39/E41): kein
push/pull/merge in sichtbarem Text, Auto-Commit still, Sync still, Token nie im Klartext in
`.git/config`. Die UI hält Orange rationiert und nutzt LED-Punkte statt Farbflächen.

---

## 2. Hohl: gebaut, aber im Alltag noch nicht handlungsfähig

Diese Dinge **sehen** v1-fertig aus, tragen aber nicht durch — der gefährlichste Lückentyp, weil er
in der Demo grün wirkt.

### 2.1 Die laute Ausnahme lässt sich nicht auflösen *(höchste Priorität)*
- **Doc-Anspruch:** US #22/#23, E41 — der stille Sync hält an, fragt „welcher gilt?", der Nutzer
  wählt **mein/Bens Stand**, und der Sync läuft danach weiter.
- **v1-Realität:** Der Kern entscheidet die laute Ausnahme korrekt (`syncdecider.rs`, `StandChoice::{Mine,Theirs}`
  existiert). Aber **es gibt keinen Backend-Befehl, der die Wahl ausführt.** Im Frontend ist
  `resolveLoud()` (`+page.svelte:147`) ein Stub: er schließt nur das orange Fenster, setzt
  `syncQuiet = "gesichert"` und ruft `runSync()` erneut auf — ohne in git je „mein" oder „Bens"
  Stand zu übernehmen. Der nächste Sync läuft sofort wieder in **dieselbe** Ausnahme.
- **Folge:** Der einzige Moment, in dem das Werkzeug die Stimme hebt, ist eine Sackgasse. Das ist
  die wichtigste Funktionslücke von v1.
- **v2:** Befehl `resolve_sync(path, choice)` bauen — `Mine` = unseren Stand behalten/überschreiben,
  `Theirs` = den fremden Stand übernehmen, jeweils mit Lock-/Invarianten-Respekt; danach Sync wieder
  freigeben. Das ist zugleich der Einstieg in die **volle Merge-Konflikt-UX** (siehe §4).

### 2.2 Auto-Lock ist eine manuelle Geste, kein „Datei geöffnet ⇒ gesperrt"
- **Doc-Anspruch:** US #11, E31 — *Öffnen/Bearbeiten* einer Binärdatei setzt **automatisch** eine
  Sperre.
- **v1-Realität:** Die Sperre wird nur über `editBaustein()` gesetzt — ausgelöst durch ein
  `onedit`-Ereignis auf der Artefakt-Karte (ein bewusster Klick in der App). Der Watcher, der
  Speichervorgänge sieht, erwirbt **keine** Sperre; er committet nur und löst einen laufenden
  Checkpoint aus. Wer KiCad/Fusion **außerhalb** der App öffnet und losarbeitet, hält bis zum
  Karten-Klick keine Sperre — genau das Fenster, das die Invariante eigentlich schließen soll.
- **v2:** Den Watcher (oder eine Datei-Öffnungs-Heuristik) so verdrahten, dass die **erste**
  Schmutzigkeit eines lockable-Pfads automatisch die Sperre erwirbt, bevor der erste laufende
  Checkpoint läuft. Mindestens: beim Watcher-Erstkontakt mit einem dirty lockable-Pfad `acquire_lock`
  aufrufen.

### 2.3 Offload-Marker reserviert, aber kein Archive Manager
- **Doc-Anspruch:** PRD-Modul „Archive Manager", US #35–#40, E36 — reversibles Auslagern alter
  Meilenstein-Inhalte ins Nutzer-Archiv; ausgelagerte Knoten tragen „Inhalt ausgelagert".
- **v1-Realität:** `graph.rs` kennt `offloaded`/`offloaded_archive` und projiziert sie korrekt
  (getestet), aber `graphread.rs:33` liefert **immer `none`** — es gibt **kein** `archive.rs`, kein
  Export/Re-Ingest, kein `git lfs prune` (US #40). Die PRD stuft das selbst als **v1-fern** ein
  („Bloat über Jahre ist kein Tag-1-Problem"), also korrekt geparkt — aber es ist eine bewusste
  Nicht-Implementierung, kein Versehen. Gehört in v2/v3 hinter die anderen Punkte.

### 2.4 „X arbeitet gerade an Y" steht nicht im ruhigen Status-Wortschatz
- **Doc-Anspruch:** US #21, E41 — Alltags-Anzeige „aktuell / **X arbeitet gerade an Y** / dein Stand
  ist gesichert".
- **v1-Realität:** Die ruhige Sync-Anzeige (`+page.svelte`) zeigt nur `aktuell`/`gesichert`. Das „X
  arbeitet an Y" lebt getrennt im Fremd-Sperren-Panel (#27). Die im Doc verlangte *eine* ruhige
  Statuszeile vereint beides nicht. Kleinere Lücke, aber doc-relevant.

---

## 3. Bekannte Bugs (offene Issues, blockieren echte Nutzung)

### 3.1 #35 — Veröffentlichen scheitert an nicht-leerem Server-Repo, roher git-Fehler leakt *(Bug, HITL)*
Der erste Push der Zeremonie nimmt ein **leeres** Server-Repo an. Hat das Repo schon Historie
(re-publish, vom Kollegen angelegt), wird der Push als non-fast-forward abgelehnt — und der **rohe
git-Text** (`master -> master (fetch first)`, push/pull-Hints, `locksverify`-Notiz) erscheint dem
Nutzer. Das ist **zugleich Funktions- und Konzeptbruch** (E33/E39: nie rohe git-Sprache). Braucht
eine Produktentscheidung: nicht-leeres Repo still integrieren (pull-then-publish) **oder** eine
einzige domänensprachliche laute Ausnahme. `locksverify` soll das Werkzeug ohnehin selbst setzen
(tut es bereits in `setup.rs`, aber die Notiz leakt im Fehlerpfad). **Erste v2-Aufgabe**, weil sie
das reale Teilen-Szenario blockiert.

---

## 4. Noch nicht gegrillt — die Konzept-Arbeit für v2 (offene Issues + Docs „Offene Punkte")

Diese Punkte brauchen zuerst eine **bewusste Entscheidung** (Grillen), bevor gebaut wird. Sie decken
sich mit den offenen Issues und der Liste am Ende von `entscheidungslog-4.md`.

| Thema | Quelle | Was zu entscheiden ist |
|---|---|---|
| **Meilenstein ↔ Branch-Modell & Push-Zeitpunkt** | #30 | Ist ein Meilenstein ein Label auf `main` (heute so) oder ein Branch? Wann genau verlässt Meilenstein-Arbeit den Rechner, und wie wird das sichtbar (ohne rohe git-Sprache)? Nutzer-Mentalmodell („Meilenstein = Branch") und Implementierung klaffen. |
| **Branch-Funktionen aus der App** | #36 | Branches erstellen/mergen aus dem Werkzeug, mit Aufgaben-Typ (Feature/Variante/Bug) und „auf main ziehen". E27/E32 sagen: Branches = bewusste Varianten. Muss kurz gegrillt werden — passt zu #30. |
| **„Neues Produkt": Baustein-Typ-Katalog + Ordner-Template** | #29 | Welche Baustein-Typen kann der Nutzer wählen, welche Ordnerstruktur wird gescaffoldet, wo lebt das Template? Heute ist „Neues Produkt" = Import eines vorhandenen Ordners; eine geführte Anlage fehlt. |
| **`lockable` nach Mergebarkeit (KiCad voll) + volle Merge-Konflikt-UX** | E41/Glossar „dritter Eimer", PRD Out-of-Scope | v1 deckt KiCad nur als laute Sync-Ausnahme. Die saubere Konsequenz (sperren nach Mergebarkeit statt Binär-Natur) + was der Nutzer im Konfliktfall im Detail sieht. Das Doc nennt das selbst „das nächste lohnende Grill-Paket". Hängt eng an §2.1 (Auflösung der lauten Ausnahme). |
| **Physisches Aufräumen stillgelegter Bausteine** | E17, Docs-Offen | Getrennte, bewusste „Historie anfassen"-Aktion. Ablauf nicht spezifiziert. |
| **Bruch fremder Sperren als Notausgang** | E31, Docs-Offen | Wenn ein Kollege eine Sperre hält und nicht erreichbar ist — Ablauf nicht im Detail. |
| **Kanten-Vorschlags-Heuristik** | E21, Docs-Offen | Bewusst auf Eis; v1 ist rein manuell. Nicht v2-dringend. |

---

## 5. Politur & kleinere Lücken

- **#27 „Fremde Sperren" umbenennen** — selbsterklärenderer Name (Kandidaten: „Belegte Bausteine",
  „Im Team in Arbeit"). Reine Sprach-/Produktentscheidung, billig.
- **§25 Artefakt-Karten-Detailbereich, §26 Filter-Feinheiten** — Kern steht, Feinschliff offen
  (PRD Out-of-Scope, bewusst).
- **`app/README.md` ist das Default-Tauri-Template** — kein echtes Projekt-README (Build, Test,
  System-Voraussetzungen aus ADR 0001). Billiger Doc-Schuldenabbau.
- **„Kollegen einladen"** ist heute nur die credential-freie Klon-URL (`setup.rs`). Ein echter
  server-seitiger Invite/Mitgliedschafts-Schritt (Forgejo-API) fehlt — die Zeremonie endet beim
  „URL kopieren". Akzeptabel für v1, in #5 so akzeptiert.

---

## 6. Plattform & Robustheit (Risiken, die ADR 0001 selbst markiert)

- **Windows nie gebaut/getestet.** ADR 0001 akzeptiert Linux-only-CI für v1, listet aber die offenen
  Windows-Stolpersteine: Pfad-Separatoren, `git`/`git-lfs` nicht im `PATH` („Git for Windows"),
  CRLF-vs-LF nicht als Inhaltsänderung missdeuten, aggressivere Datei-Sperren. **Bleibt offen, bis
  eine Windows-Prüfung dazukommt** — für ein Zwei-Personen-Team mit HW-Entwickler (oft Windows) ein
  realer v2-Kandidat.
- **git/git-lfs-Auffindung** ist (Stand Härtung #22) gehärtet für Linux; die robuste PATH-Suche für
  Windows-Installationsorte ist noch zu verifizieren.

---

## 7. Empfohlene v2-Reihenfolge

1. **#35 Veröffentlichen an nicht-leeres Repo + git-Leak schließen** — blockiert reales Teilen; reiner
   Bugfix mit kleiner Produktentscheidung.
2. **Laute Ausnahme auflösbar machen (§2.1)** — `resolve_sync(choice)`; schließt die einzige tote
   Stelle im stillen-Sync-Herzstück. Zieht das Grill-Paket „volle Merge-Konflikt-UX" (§4) nach sich.
3. **Auto-Lock beim ersten dirty lockable-Pfad (§2.2)** — schließt das offene Invarianten-Fenster
   zwischen externem Öffnen und Karten-Klick.
4. **#30 Meilenstein/Branch-Modell + #36 Branch-Funktionen grillen, dann bauen** — gemeinsames
   Grill-Paket (Topologie/Push-Zeitpunkt/Variante).
5. **#29 „Neues Produkt" geführte Anlage** (Baustein-Katalog + Template) und **#27 Umbenennung** —
   UX-Aufwertung, niedriges Risiko.
6. **`lockable` nach Mergebarkeit (KiCad voll)** — das vom Doc benannte „nächste lohnende Grill-Paket".
7. **Windows-Build/-Prüfung** — sobald eine Windows-Umgebung bereitsteht.
8. **Archive Manager / Auslagern (E36) + `git lfs prune`** — bewusst zuletzt (v1-fern), wenn Bloat real wird.

---

### Ein-Satz-Fazit
v1 trägt das **Herz** (sechs reine Kerne, die Binär-Invariante als Property-Test, die git-freie
Alltagssprache) sauber und doc-treu — die Lücke ist nicht das Fundament, sondern die **Auflösung der
lauten Ausnahme**, das **automatische Sperren beim echten Öffnen**, der **Teilen-Bug an nicht-leeren
Repos** und die noch ungegrillten **Branch-/Meilenstein-** und **Mergebarkeits-** Entscheidungen.
