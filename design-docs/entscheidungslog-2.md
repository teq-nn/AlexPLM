# Entscheidungslog — PLM-Werkzeug

Stand: 29.05.2026 (3. Sitzung). Jede Entscheidung mit Begründung und — wo zutreffend — was sie im Originalkonzept (`plm_software_konzept.md`) oder in früheren Einträgen ersetzt oder überholt. Neu in Sitzung 3: E16–E22 (Baustein-/Stack-Modell, Lebenszyklus, Anti-Drift, dreistufiger Freigabe-Block, dreistufige Kanten, „lernendes System" verworfen, Design-Haltung).

---

## E1 — Es ist ein Werkzeug, kein Produkt
**Entscheidung:** Gebaut primär für den eigenen Gebrauch.
**Warum:** Speichermodell, fehlende Markt-/Preis-/Wettbewerbsüberlegungen und die hochspezifischen Eigen-Tools (KiCad, Fusion 360, PlatformIO, JLCPCB) zeigen alle auf „eigener Schmerz". Test bestanden: würde es auch bauen, wenn es sicher niemand sonst nutzt.
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
**Erweitert (3. Sitzung):** Die Tag-1-Pflicht gilt jetzt nicht nur beim Produktanlegen, sondern **bei jedem Baustein-Onboarding** (E17) — die LFS-/Ignore-Muster eines neuen Bausteins stehen, *bevor* dessen Tool die erste Binärdatei/Müll erzeugt.

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
**Folge (3. Sitzung):** Die Globs müssen *irgendwo* leben → sie werden Bestandteil eines **Bausteins** (E16).

---

## E11 — Arbeitsbereich = harter Anker, Artefakt = weiches Label
**Entscheidung:** Unzugeordnete Datei wird **getrackt und bleibt physisch liegen** (Variante c), nur das *Label* fehlt. Unzugeordnet-Fach **pro Arbeitsbereich** (nie global), mit Kontext-Vorschlag aus den Ordner-Geschwistern. Im Alltag passiv; ein Meilenstein/Tag löst den Vollständigkeits-Check aus → jede Waise verhindert „technisch vollständig" (Freigabe trotzdem möglich per §22.1).
**Warum:** „Nichts geht durch Weglassen verloren" (Seele des Tools) bleibt wahr, ohne modale Dialoge. Der Ordnerkontext ist der stärkste Zuordnungshinweis — den verliert man bei einem globalen Stapel („Wild-West, später weiß man nicht wohin"). Der erzwungene Check am Meilenstein verhindert, dass das Fach unbemerkt einen Freigabestand überlebt.
**Voraussetzung:** **Ignore-Presets pro Tool** als erste Klasse; `.gitignore` Tag-1-Pflicht → **erweitert E4**. Alles getrackt außer explizit ignoriert.
**Stärkt §18/§22** (Waisen als Teil des Vollständigkeits-Checks), **dreht §14 weiter** mit E10.

---

## E12 — Abgeleitet-von-Kante gegen veraltete Export-/Fertigungsdaten
**Entscheidung:** Optionale deklarierte Kante „Artefakt X abgeleitet von **Menge** {Quell-Artefakte}". Stale-Check rein über **Git-Reihenfolge** (kein Inhaltsvergleich, kein Parser): wurde irgendeine Quelle nach dem letzten Stand der Ableitung geändert? → **Warnung, kein Block**, v. a. beim Meilenstein.
**Warum:** Fängt den teuren Realfehler (alte Gerber an JLCPCB schicken, nachdem das PCB geändert wurde) ohne die ausgeschlossenen Inhaltsvergleiche/Parser (§30). Quelle als **Menge**, weil Pick-and-Place an PCB-Layout *und* BOM hängt — 1:1 verschliefe die BOM-Änderung. Asymmetrie: falscher „prüf nach" billig, verpasster Stale-Export teuer.
**Grenze (ehrlich):** erkennt „veraltet", nicht „aus falscher/ungespeicherter Datei exportiert". Frischer Zeitstempel ≠ inhaltlich korrekt.
**Bezug:** ergänzt §12 (Haupt-/Zusatzdateien, Artefakt-Beziehungen).
**Verfeinert in 3. Sitzung durch E20** (Herkunft der Kanten dreistufig).

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

## E16 — Der „Workflow" zerfällt in Bausteine und Stacks
**Entscheidung:** Der überladene Begriff „Workflow" wird ersetzt. Das **Atom** ist der **Baustein** — ein wiederverwendbares Bündel meist *für ein Tool*: Heimat-Arbeitsbereich + Artefakt-Globs + Ignore-Presets + LFS-Muster + Öffnen-Aktion + optionale Startaufgaben + interne Abgeleitet-von-Kanten. Daraus: **Standard-Toolstack** (geteilt, in der Bibliothek, beim Anlegen um einzelne Tools erweiterbar) → beim Produktanlegen als Schnappschuss kopiert zum **Produkt-Stack** (ins Produkt, lebend). Das frühere „Gerüst/Framework" ist **kein eigenes Ding mehr** — es fällt aus dem Zusammenstecken der Bausteine heraus (jeder Baustein kennt seine Heimat).
**Warum:** E10 (Artefakt = Glob) und E11/E4 (Ignore-/LFS-Presets Tag-1-Pflicht) hatten „Workflow" still von einer *einmaligen Startvorlage* zu einem *lebenden Regelsatz* gemacht, an dem das Produkt dauerhaft hängt — das Original (§19/§28.5) wusste davon nichts. Das Atom „Baustein" macht das explizit und erlaubt Erweitern/Austauschen einzelner Tools.
**Begriffskollisionen aufgelöst:** **Werkzeug** = ab jetzt nur die PLM-Software; **Tool/Software** = eine Entwicklungssoftware (KiCad, Zephyr …); **Bauteil/Komponente** bleibt der Hardware (BOM). „Framework" verworfen (in Software das *dauerhafte* Ding, also irreführend; und im Modell überflüssig).
**Anti-Drift (Kern aus Sitzung 2):** Eine Änderung am Standard-Toolstack rührt bestehende Produkt-Stacks **nie** an — der Produkt-Stack ist Kopie, keine Live-Abhängigkeit; eine lokale Änderung fließt höchstens in *künftige* Produkte zurück.
**Überholt:** **§19** (Workflow als Startvorlage) und **§28.5** (Workflow-Vorlage nur mit `name`+`required`, ohne Muster). §19.2 („zurück in die Vorlage") wird ungefährlich: betrifft nur künftige Produkte.

---

## E17 — Baustein-Lebenszyklus: Stilllegen ist label-only und additiv
**Entscheidung:** **Erweitern** = Baustein dazu (rein additiv). **Austauschen** = Erweitern + **Stilllegen** des alten. Stilllegen:
- Artefakt-Globs hören auf zu greifen → alte Dateien werden **Waisen** (E11), nichts wird verschoben/gelöscht.
- Ignore-/LFS-Regeln des alten Bausteins bleiben als **Sediment** liegen, werden **nie automatisch entfernt**.
- Neuer Baustein wird wie am ersten Tag onboardet → **Tag-1-Pflicht pro Onboarding** (erweitert E4).
- Physisches Aufräumen ist **kein** Teil des Austauschs, sondern nur eine getrennte, bewusste Aktion, die ehrlich sagt, dass sie Historie anfasst.
**Warum:** Entfernen ist die einzige Operation, die alten Müll wieder sichtbar macht oder ein teures `git lfs migrate` auslöst (E9-Logik: behalten, nie umschreiben). So bleibt ein Austausch fast vollständig umkehrbar und löst nie still Daten auf. Beispiel: PlatformIO → Zephyr für die Firmware; `.pio/`-Ignore und alte `.bin` bleiben harmloses Sediment/Waisen, Zephyrs `.elf`/`.hex` werden vor ihrer Entstehung korrekt getrackt.
**Bezug:** wendet E11 (Waisen), E4/E11 (Tag-1), E9 (behalten statt umschreiben) auf Baustein-Ebene an.

---

## E18 — Dotfiles sind die alleinige Wahrheit für Ignore/LFS (Anti-Drift auf Baustein-Ebene)
**Entscheidung:** `.gitignore` und `.gitattributes` sind die **einzige** Quelle für Ignore- und LFS-Muster; das Tool **liest sie zurück**, hält keinen Zweitstand. Der Produkt-Stack im `_plm` besitzt nur, **was Git nicht kennt**: Zuordnung Artefaktname ↔ Glob, Heimat, Öffnen-Aktion, Startaufgaben, Hand-/Paar-Kanten. Onboarding hängt die Ignore-/LFS-Zeilen eines Bausteins **idempotent** in einen **Marker-Block** (`# >>> baustein: zephyr >>>` … `# <<<`) an die Dotfiles. **Hand-Edits gewinnen automatisch**, weil sie die Wahrheit *sind*; alles außerhalb der Marker-Blöcke fasst das Tool nie an.
**Warum:** Eine gespiegelte Kopie der Muster im `_plm` wäre exakt die Drift, die E8 erschlagen hat (zwei Quellen, die auseinanderlaufen) — heikel, sobald etwas die Dotfiles direkt anfasst (du von Hand, KiCad 9 mit eingebauter Git-Integration, `git lfs migrate`). Der Marker-Block dient nur dem Wiederfinden eigener Zeilen beim Austausch (E17).
**Stärkt:** E8 (keine PLM-Duplikate dessen, was Git kennt), auf Baustein-Ebene. Macht Fremd-Edits gefahrlos.

---

## E19 — Dreistufiger Freigabe-Block statt einem „trotzdem freigeben"-Knopf
**Entscheidung:** Offene Punkte beim Meilenstein/Tag werden nach Härte gestaffelt:
1. **Warnung** (Stale-Kante, E12/E20) — sichtbar, blockiert nie, keine Begründung.
2. **Weicher Block** (Waise / fehlendes Pflicht-Artefakt, E11) — blockiert „technisch vollständig", aber per **protokollierter Begründung** überwindbar (= §22.1 wie geschrieben).
3. **Harter Block** (offener blockierender Task auf strengem Branch, E15) — **nicht** per Begründungstext wegzudrücken; nur durch **Erledigen / Verwerfen / Herabstufen zum Hinweis** des Tasks selbst.
Zusätzlich: die §22-Vorbedingung „`VERSION_NOTES.md` erzeugt?" **fliegt raus** — nach E13 ist die Notiz ein *Ergebnis* des Tags, kein Eingang (zirkulär).
**Warum:** Der flache §22.1-Knopf macht den härtesten Fall zahnlos: „blockiert" hieße nur noch „blockiert, bis du einen Satz tippst", und E15s bewusster Checkpoint wäre Fassade. Stufe 3 sperrt trotzdem nie aus (Ausweg = *ein Klick auf den Task*), bleibt aber ein bewusster Handgriff am Blocker. Beispiel: Rev-A-Tag mit verirrter `.csv` (weich), geändertem PCB seit letztem Gerber (Warnung) und offenem „Footprint Q3 prüfen" (hart).
**Verfeinert §22/§22.1.** Instanz der Design-Haltung E22.

---

## E20 — Abgeleitet-von-Kanten: drei Herkunftsstufen statt „alles von Hand"
**Entscheidung:** Die Herkunft einer Kante (E12) zerfällt in drei Stufen, alle landen im `_plm` des Produkt-Stacks:
- **Baustein-Default** — Kante *innerhalb* eines Bausteins (Gerber ← Layout). Automatisch beim Onboarding.
- **Baustein-Paar-Default** — Kante über zwei *bekannt zusammengehörige* Bausteine (Pick-and-Place ← Layout + BOM). Liegt als „wenn A und B im Stack, schlage Z vor" bei den Bausteinen → **deterministischer Vorschlag**, per Klick bestätigt.
- **Hand-Kante** — nur das echt Idiosynkratische.
**Warum:** Nach E10 (Konvention statt Eingabe) ist Handarbeit für die *offensichtlichen* Kanten genau die Bürokratie, gegen die das Tool antritt. Manche Kanten überspannen aber zwei Bausteine und haben damit keine Heimat auf Baustein-Ebene → die Paar-Stufe. Stale-Check (Git-Reihenfolge, kein Parser) bleibt unverändert; nur die *Herkunft* der Kante ändert sich.
**Konsistenz:** Wird ein Baustein stillgelegt (E17) und seine Quell-Artefakte zu Waisen, geht die Kante **still in Ruhe** — gleiche Label-only-Logik.
**Verfeinert E12.**

---

## E21 — „Lernendes System" für Kanten-Vorschläge verworfen
**Entscheidung:** Kein statistisches/ML-System zum Vorschlagen von Kanten. Stattdessen die deterministische **Baustein-Paar-Default**-Stufe (E20). Eine reine **Git-Reihenfolge-Heuristik** („Artefakt wird wiederholt direkt nach jenen zweien geändert — Kante deklarieren?") bleibt als spätere, ausdrücklich **nicht-lernende** Annehmlichkeit geparkt (würde die E12-Stale-Maschinerie wiederbenutzen).
**Warum:** ML kollidiert mit E1 (Solo-Werkzeug → kein Daten-Korpus; dünne Solo-Daten = Rauschen) und E4/§30 (kein Inhalts-Parser, autark, kein Nach-Hause-Telefonieren). Vor allem: Was nach den zwei Default-Stufen für die Hand übrig bleibt, ist per Definition das *Untypische* — gerade dort taugt Lernen aus dünnen Daten am wenigsten. Der Aufwand lohnt nirgends.

---

## E22 — Design-Haltung: streng am Checkpoint, Ausweg einen Handgriff entfernt
**Entscheidung (Querschnitt-Prinzip):** Im Alltag fragt und blockiert das Werkzeug nichts; streng wird es nur am **bewussten Checkpoint** (Meilenstein/Merge nach oben). Selbst dort ist der Ausweg immer genau **einen bewussten Handgriff** entfernt — nie null (sonst Fassade) und nie unmöglich (sonst sperrt es dich gegen dich selbst aus und wird umgangen).
**Warum:** Macht explizit, was E8 (Netz unsichtbar, Zeremonie nur am Tag), E11 (Waisen-Check nur am Meilenstein), E15 (Strenge nur am Übergang nach oben) und E19 (dreistufiger Block) gemeinsam tragen. Leitlinie für alle künftigen Schichten (u. a. die noch offene UI).

---

## Offene / verschobene Punkte
- **UI-Schichten noch nicht durchgegrillt:** §24 Versionsleiste/Versionsbaum, §25 Artefakt-Karten, §26 Suche/Filter/Tags. Liegen weitgehend orthogonal über dem jetzt fertigen Kern (Speicher, Versionen, Bausteine/Stacks, Aufgaben, Kanten, Freigabe). **Nächster Stoff.**
- **Physisches Aufräumen stillgelegter Bausteine** (E17) — als getrennte, bewusste „Historie anfassen"-Aktion noch nicht im Detail durchgesprochen.
- **Git-Reihenfolge-Heuristik für Kanten-Vorschläge** (E21) — spätere, ausdrücklich nicht-lernende Annehmlichkeit, im Eis.
- **Cloud/Mehrgerät-Konflikt:** derselbe Branch an zwei Orten gleichzeitig geändert → kommt wieder, sobald ein Remote dranhängt (E4). Bis dahin geparkt.
- **§9 Multi-User-Workspace** — geparkt (E1).
- **LFS-Host-Ökonomie** — erst bei Hosting-Entscheidung (E4).
