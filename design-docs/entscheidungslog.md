# Entscheidungslog — PLM-Werkzeug

Stand: 29.05.2026. Jede Entscheidung mit Begründung und — wo zutreffend — was sie im Originalkonzept (`plm_software_konzept.md`) ersetzt oder überholt.

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

## Offene / verschobene Punkte
- **Cloud/Mehrgerät-Konflikt:** derselbe Branch an zwei Orten gleichzeitig geändert → kommt wieder, sobald ein Remote dranhängt (E4). Bis dahin geparkt.
- **§9 Multi-User-Workspace** — geparkt (E1).
- **LFS-Host-Ökonomie** — erst bei Hosting-Entscheidung (E4).
- **Nicht weiter durchgegrillt:** §10–13, §19–26 (Artefakte, Workflows, Aufgaben, UI im Detail, Suche/Tags). Der Speicher-/Versionskern steht; diese Schichten liegen weitgehend orthogonal darüber und sind beim nächsten Mal dran.
