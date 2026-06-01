# Entscheidungslog — Werkbank

Stand: 29.05.2026 (2. Sitzung). Jede Entscheidung mit Begründung und — wo zutreffend — was sie im Originalkonzept (`plm_software_konzept.md`) ersetzt oder überholt.

---

## E1 — Es ist ein Werkzeug, kein Produkt
**Entscheidung:** Gebaut primär für den eigenen Gebrauch.
**Warum:** Speichermodell, fehlende Markt-/Preis-/Wettbewerbsüberlegungen und die hochspezifischen Eigen-Werkzeuge (KiCad, Fusion 360, PlatformIO, JLCPCB) zeigen alle auf „eigener Schmerz". Test bestanden: würde es auch bauen, wenn es sicher niemand sonst nutzt.
**Folge:** Markt, Preis, Go-to-Market, Multi-User raus. **§9 (gemeinsame Workspace-Konfiguration) geparkt** — nur relevant, falls du später mit einem kleinen bekannten Kreis teilst.

---

## E2 — Git + Git-LFS als Speicher-Motor statt voller physischer Kopien
**Entscheidung:** Versionierung läuft über Git; große Binärdateien (STEP, STL, .f3d, Zip, PDF, Fotos) über LFS; Text (KiCad-Quellen, BOM, Firmware, Doku) über normales Git mit echten Diffs. Ein Repo pro Produkt.
**Warum:** „Volle Kopie pro Version" ist auf lokaler Platte (bei ~1 GB/Version) unproblematisch, aber auf Cloud-Sync und Netzlaufwerken zäh bis tödlich (Voll-Upload unveränderter Daten je Version). Außerdem war das Datenmodell in §6/§27 bereits ein handgeschnitztes, schlechteres Git (Branches, `created_from`, Default-Branch). KiCad-Dateien sind selbst Text und git-nativ (KiCad 9 hat eingebaute Git-Integration).
**Ersetzt:** **§5** (globale Versionen als volle Kopien) und **§7** (jede Version ein physischer Ordner) sind überholt. Mapping: Produkt-Branch → Git-Branch; Version → getaggter Commit; §6.1-Aktionen → Branch behalten / mergen / Tag+liegenlassen.

---

## E3 — „Neueste da, ältere per Klick" statt „alle Versionen immer als Ordner"
**Entscheidung:** Ein Arbeitsverzeichnis pro Produkt. Ältere Stände werden bei Bedarf als echter, browsebarer Ordner materialisiert (Worktree / Export).
**Warum:** „Alle Versionen gleichzeitig als Ordner *und* kein Extra-Platz" ist physikalisch unmöglich. Materialisieren on demand erhält die Tool-Unabhängigkeit der Daten (die Seele des Konzepts), ohne den Speicher-/Cloud-Blowup. Bleibt ein Regler, kein Entweder-oder: man *kann* dauerhaft mehrere Worktrees behalten, wird aber nie dazu gezwungen.

---

## E4 — Lokal zuerst, Cloud-Remote später
**Entscheidung:** Erst lokales Git+LFS-Repo. Hosting (Gitea/Forgejo/GitLab/GitHub) später per `git remote add` nachrüsten.
**Warum:** Ein lokales Repo ist vollständig; LFS läuft lokal ohne Server. Nichts geht durch Aufschieben verloren.
**Tag-1-Pflicht:** `.gitattributes` mit den LFS-Mustern muss schon beim **Produktanlegen** stehen, sonst späteres teures `git lfs migrate`. Offene „Später"-Frage mit Zähnen: LFS-Ökonomie des gewählten Hosts (GitHub-LFS-Quota kostet) → selbstgehostetes Gitea/Forgejo ist der billige Dauerweg.
**Erweitert (2. Sitzung):** Tag-1-Pflicht umfasst jetzt **auch die `.gitignore`** (Ignore-Presets, siehe E11). Ohne sie ist schon die erste Auto-Commit-Welle voller Cache-Müll (`*.kicad_prl`, `*-backups/`, `.DS_Store`), der über E5 auch Statusanzeige und Netz verschmutzt.

---

## E5 — LFS/Git-Fallstricke sind Sache des Tools, nicht des Users
**Entscheidung (verfeinert, siehe E6):** Das Tool verwaltet `.gitattributes`, sortiert jede Datei automatisch (groß/binär → LFS, Text → Git), liefert/prüft LFS mit, verwandelt Explorer-Änderungen still in Commits, nutzt Änderungsnotizen als Commit-Text.
**Warum:** Als Hardware-Entwickler willst/sollst du die Software-Tiefen nicht überblicken.
**Stärkt:** **§14** (Änderungserkennung) wird faktisch `git status` — robuster, weniger Eigenbau. „Nicht zugeordnet" = untracked.

---

## E6 — Git darf *durchscheinen*, nur die Fallstricke nicht (Korrektur zu E5)
**Entscheidung:** Nicht Git komplett verstecken. Git's Denkmodell (Commit, Branch, History-Graph, Tag, Abstammung) darf sichtbar sein und gibt Übersicht. Verborgen/automatisiert bleibt nur das „Wie" (stash, revert/reset by ref, rebase, manuelle Konfliktlösung, LFS-Tracking/migrate, gc/prune).
**Warum:** Die Grundideen von Git sind einfach zu verstehen und nützlich (Graph = beste Übersicht). Nur die fehleranfälligen Recovery-Formeln überfordern. Test „Wohin vs. Wie": Zustände/Orte sichtbar, Beschwörungsformeln versteckt.
**Korrigiert:** das frühere „vollständig gekapselt" aus E5.

---

## E7 — Merge-Verhalten beim Übernehmen-als-Standard
**Entscheidung:**
- Datei nur auf *deiner* Seite geändert → automatisch, deine Version gewinnt, kein Nachfragen.
- Datei nur auf *Production* geändert (während du woanders warst) → bleibt automatisch erhalten.
- Datei auf *beiden* Seiten geändert (echte Kollision) → Tool **stoppt**, zeigt die Kollisionen im Klartext (Datei, Datum, Größe, wenn möglich Vorschau), du wählst pro Datei. Kein Git-Wort.
**Warum:** Reines „Quelle gewinnt immer" überschreibt still auch das, was du gar nicht angefasst hast (Beispiel: ausgelieferter Firmware-Bugfix auf Production geht beim Übernehmen eines Gehäuse-Experiments verloren). Bei Solo-Projekten sind echte Kollisionen selten → fast nie Nachfragen, aber der eine teure Fall ist abgesichert.
**Berührt:** §6.1 („als neuen Standard übernehmen" = Merge in Production-Branch).

---

## E8 — Zwei Ebenen: sichtbare Meilensteine, unsichtbares Sicherheitsnetz
**Entscheidung:** „Version" = bewusst benannter Meilenstein (Tag). Darunter automatische Zwischen-Commits als Sicherheitsnetz/Rückgängig-Verlauf, das du im Normalbetrieb nicht siehst, aber im Notfall erreichst.
**Warum:** Hardware-„Versionen" sind Dinge, die man fertigt (Rev A/B) — nicht jeder Tastendruck. „Version = jeder Commit" gäbe `v0.387` und machte die Freigabe-Zeremonie zu Lärm. Das Netz ist reiner Gewinn (hast du heute nicht).
**Überholt:** **§5** „jede relevante Änderung erzeugt eine neue Version". **Verfeinert §16:** `VERSION_NOTES.md` = menschenlesbare Spiegelung des Commit-/Tag-Textes; `version.json` führt die **Abstammung nicht mehr selbst** (kennt Git), nur noch PLM-Status/Zusammenfassung/Freigabe — sonst Drift-Gefahr.
**Stärkt §17:** Freigegebene Version = Tag auf unveränderlichem Commit → Schreibschutz geschenkt; „zum Bearbeiten neu abzweigen" = `checkout -b` vom Tag.

---

## E9 — Auto-Commits einklappbar (Anzeigefilter), nicht zusammenfassen
**Entscheidung:** Graph zeigt standardmäßig nur Meilenstein-Tags; Zwischenstände sind aufklappbar. Reiner **Anzeigefilter** — nichts wird umgeschrieben oder weggeworfen.
**Warum:** „Einklappbar" und „zusammenfassen" schließen sich aus — nur eins geht. Aufklappbar erfordert, alles zu behalten. Kein riskantes Historie-Umschreiben.
**Preis:** Speicher für behaltene Zwischenstände — bei dir vernachlässigbar (KiCad-Quellen = Text/winzig; identische Binär-Blobs dedupliziert LFS per Hash). Nennenswert nur, wenn dieselbe große CAD über den Tag mehrfach in verschiedenen Ständen gespeichert wird — genau die willst du zurückholen können.

---

## E10 — Pattern-Zuordnung als Default, Hand nur als Korrektur
**Entscheidung:** Ein Artefakt ist eine Erwartung mit Muster (Glob), das Tool ordnet neue Dateien automatisch zu. Hand-Zuordnung nur, wenn keine Regel greift.
**Warum:** Pro-Datei-Handzuordnung (§14) ist genau die Dateneingabe-Bürokratie, gegen die das Werkzeug antritt; beim KiCad-Speichern fallen mehrere Dateien gleichzeitig an. Convention over configuration (wie Build-Tools/Linter ihre Dateien per Glob finden) hält die laufende Reibung bei ~0. Preis: gelegentlicher Fehlmatch — billiger als Dauer-Klickerei.
**Dreht um:** **§14** („neue Datei → erst unzugeordnet → Hand").

---

## E11 — Arbeitsbereich = harter Anker, Artefakt = weiches Label
**Entscheidung:** Unzugeordnete Datei wird **getrackt und bleibt physisch liegen** (Variante c), nur das *Label* fehlt. Unzugeordnet-Fach **pro Arbeitsbereich** (nie global), mit Kontext-Vorschlag aus den Ordner-Geschwistern. Im Alltag passiv; ein Meilenstein/Tag löst den Vollständigkeits-Check aus → jede Waise verhindert „technisch vollständig" (Freigabe trotzdem möglich per §22.1).
**Warum:** „Nichts geht durch Weglassen verloren" (Seele des Tools) bleibt wahr, ohne modale Dialoge. Der Ordnerkontext ist der stärkste Zuordnungshinweis — den verliert man bei einem globalen Stapel („Wild-West, später weiß man nicht wohin"). Der erzwungene Check am Meilenstein verhindert, dass das Fach unbemerkt einen Freigabestand überlebt.
**Voraussetzung:** **Ignore-Presets pro Werkzeug** als erste Klasse; `.gitignore` Tag-1-Pflicht → **erweitert E4**. Alles getrackt außer explizit ignoriert.
**Stärkt §18/§22** (Waisen als Teil des Vollständigkeits-Checks), **dreht §14 weiter** mit E10.

---

## E12 — Abgeleitet-von-Kante gegen veraltete Export-/Fertigungsdaten
**Entscheidung:** Optionale deklarierte Kante „Artefakt X abgeleitet von **Menge** {Quell-Artefakte}". Stale-Check rein über **Git-Reihenfolge** (kein Inhaltsvergleich, kein Parser): wurde irgendeine Quelle nach dem letzten Stand der Ableitung geändert? → **Warnung, kein Block**, v. a. beim Meilenstein.
**Warum:** Fängt den teuren Realfehler (alte Gerber an JLCPCB schicken, nachdem das PCB geändert wurde) ohne die ausgeschlossenen Inhaltsvergleiche/Parser (§30). Quelle als **Menge**, weil Pick-and-Place an PCB-Layout *und* BOM hängt — 1:1 verschliefe die BOM-Änderung. Asymmetrie: falscher „prüf nach" billig, verpasster Stale-Export teuer.
**Grenze (ehrlich):** erkennt „veraltet", nicht „aus falscher/ungespeicherter Datei exportiert". Frischer Zeitstempel ≠ inhaltlich korrekt.
**Bezug:** ergänzt §12 (Haupt-/Zusatzdateien, Artefakt-Beziehungen).

---

## E13 — Änderungsnotizen beim Meilenstein, nicht pro Dateiwechsel
**Entscheidung:** Im Alltag fragt das Tool nichts; Auto-Commits laufen mit maschineller Nachricht. Beim **Tag** kommt *eine* artefakt-gruppierte Zusammenfassung → `VERSION_NOTES.md`. Freiwilliger Pin einzelner Zwischenstände bleibt möglich (Bringschuld, nie Holschuld).
**Warum:** Behebt den internen Widerspruch zwischen §15 (Notiz pro Dateiwechsel) und E8 (Commits = Wegwerf-Zwischenstände, die man nie ansieht). Das „Warum" gehört an den lesbaren Rev-Stand, nicht an `auto-commit #387`. Drei Pflichtfragen pro Dateiwechsel wären genau die Reibung, gegen die das Tool antritt.
**Überholt:** **§15** (Pro-Datei-Abfrage „Geändert? Warum?").

---

## E14 — Zwei Aufgabentypen: Task vs. Hinweis
**Entscheidung:** **Task** = verpflichtend, *kann* blockieren. **Hinweis** = anekdotisch, „zur rechten Zeit aufdringlich", *blockiert nie*. Trennende Eigenschaft ist die **Blockier-Fähigkeit**, nicht die Wichtigkeit.
**Warum:** Ein einzelner Aufgaben-Begriff mit Schieberegler verwischt genau die Grenze, die für die Freigabe zählt. Bewusst nicht „Reminder" (zu nah am Wecker).
**Verfeinert §20.**

---

## E15 — Strenge ist Eigenschaft des Branch-Typs, nicht der Aufgabe
**Entscheidung:** Prototyp-/Experiment-Branch = lasch (Tasks blockieren nicht), Production/Release = streng (Tasks blockieren). Eine Aufgabe **erbt** die Strenge ihres Branches; sie greift an *jedem Übergang nach oben* — **Tag setzen** *und* **Merge nach Production** (E7). **Opt-out pro Task** („blockiert überall") für den seltenen kontextunabhängigen Fall. **Workflow-Startaufgaben sind Vorschlag, kein Muss** (wegwerfbar).
**Warum:** Streng wird's nur am bewussten Checkpoint (analog E8) — im Alltag nie. Merge nach Production als zweiter Auslöser, damit ein offener Task nicht durch die Hintertür durchrutscht (sonst: alter „Footprint prüfen"-Task übersteht stillschweigend die Rev-A-Freigabe). Startaufgaben als Muss würde dich gegen dich selbst aussperren → Regeln werden ignoriert; ein Werkzeug, das behindert statt hilft, verliert man (E1).
**Überholt §21:** Datei-Zeremonie beim Erledigen entfällt komplett — die Prüfung liegt im Meilenstein-Check (E11), „erledigt" heißt schlicht erledigt. **Verfeinert §18** (blockierende Aufgaben), **§20** (Modell-Minimum: Titel, Status, Typ, optionale Verknüpfung, Fälligkeit; Priorität + Kanban als Jira-Ballast raus, Kanban evtl. später als reine Ansicht).

---

## Offene / verschobene Punkte
- **Cloud/Mehrgerät-Konflikt:** derselbe Branch an zwei Orten gleichzeitig geändert → kommt wieder, sobald ein Remote dranhängt (E4). Bis dahin geparkt.
- **§9 Multi-User-Workspace** — geparkt (E1).
- **LFS-Host-Ökonomie** — erst bei Hosting-Entscheidung (E4).
- **Noch nicht durchgegrillt:** §19 Workflows (nur am Rand über E15 berührt), §22 Freigabedialog im Detail, §23–26 (UI, Versionsleiste, Artefakt-Karten, Suche/Tags). Speicher-, Versions-, Artefakt- und Aufgaben-Kern stehen jetzt; diese Schichten liegen weitgehend orthogonal darüber.
