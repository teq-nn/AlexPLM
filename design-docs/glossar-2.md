# Glossar — geschärfte Begriffe

Stand: 29.05.2026 (3. Sitzung). Entstanden beim Grillen des PLM-Konzepts. Diese Begriffe haben im Gespräch eine *präzise* Bedeutung bekommen, die im Originaldokument noch verschwommen war. Neu in Sitzung 3: das Baustein-/Stack-Modell und seine Folgen.

---

## Werkzeug vs. Produkt
- **Werkzeug** — etwas, das du primär für dich selbst baust. Markt, Preis, Wettbewerb, Multi-User sind irrelevant. Maßstab: „trägt der Ansatz technisch, und lohnt er den Aufwand gegenüber dem Status quo?"
- **Produkt** — soll vor anderen im Markt landen. Maßstab: „sollte es existieren, gibt es das schon, warum würde jemand wechseln?"

**Entscheidung:** Dieses Vorhaben ist ein **Werkzeug**. (Test bestanden: „auch bauen, wenn es sicher niemand außer dir nutzt" → ja.)

> **Begriffsschärfung (Sitzung 3):** „Werkzeug" bezeichnet ab jetzt **ausschließlich die PLM-Software, die du baust**. Eine *Entwicklungssoftware* (KiCad, Fusion 360, Zephyr) heißt **Tool** bzw. **Software** — nie „Werkzeug". Damit verschwindet die alte Doppelbelegung in den Ignore-Presets („pro Werkzeug" hieß dort eigentlich „pro Tool").

---

## Tool / Software (Entwicklungssoftware)
Eine konkrete Entwicklungssoftware, mit der du arbeitest: KiCad, Fusion 360, Zephyr, PlatformIO. **Nicht** zu verwechseln mit **Werkzeug** (= die PLM-Software) und **nicht** mit **Bauteil/Komponente** (= Hardware, BOM).

---

## Baustein (das Atom des Stack-Modells)
Ein **Baustein** ist ein wiederverwendbares Bündel von Erwartungen + Schutzregeln + Heimat, meist für *ein Tool*. Er trägt:
- **Heimat-Arbeitsbereich** — der Ordner, in dem dieses Tool arbeitet (KiCad → `elektronik/`, Zephyr → `firmware/`, Fusion → `mechanik/`).
- **Artefakt-Globs** — die Erwartungen mit Muster (vgl. Pattern-Zuordnung).
- **Ignore-Presets** — bekannter Müll dieses Tools.
- **LFS-Muster** — welche Ausgaben über LFS laufen.
- **Startaufgaben** (optional) und **Öffnen-Aktion**.
- **Abgeleitet-von-Kanten innerhalb des Bausteins** (Baustein-Default, s. u.).

Präzisierung: Ein Baustein ist meist *tool-förmig*, manchmal *output-förmig* (z. B. „Dokumentation" hängt an keiner bestimmten Software). Beide Sorten tragen dieselben Felder — das Tool ist der *häufige*, nicht der *einzige* Fall.

Das „Gerüst" aus Sitzung 2 ist damit **kein eigenes Ding mehr**: Was früher „Startvorlage legt Ordner an" hieß, fällt jetzt von selbst aus dem Zusammenstecken der Bausteine heraus (jeder Baustein kennt seine Heimat). Gespeichert werden **Bausteine** und **Stacks**.

> **Verworfen:** „Framework" als Wort dafür — in Software ist ein Framework das *dauerhafte* Ding, in dem Code lebt, also das Gegenteil eines einmaligen Gerüsts; und im Baustein-Modell braucht es kein eigenes Wort mehr.

---

## Standard-Toolstack vs. Produkt-Stack
- **Standard-Toolstack** — ein wiederverwendbares, **geteiltes** Baustein-Bündel in der Bibliothek. Beim Anlegen eines Produkts wählbar und direkt um einzelne Tools erweiterbar („diesmal Zephyr statt PlatformIO").
- **Produkt-Stack** — der beim Anlegen daraus erzeugte, **ins Produkt kopierte** und dort **lebende** Schnappschuss. Im Entwicklungszyklus erweiter-/austauschbar.

**Regel (Kern aus Sitzung 2, hier angewandt):** Eine Änderung am **Standard-Toolstack** rührt bestehende **Produkt-Stacks nie** an — kein Konventions-Drift. Der Produkt-Stack ist eine Kopie, keine Live-Abhängigkeit. Eine lokale Änderung fließt höchstens in *künftige* Produkte zurück.

---

## Lebenszyklus eines Bausteins: Erweitern vs. Austauschen
- **Erweitern** — Baustein dazu. Rein additiv, harmlos.
- **Austauschen** — nichts anderes als **Erweitern + Stilllegen** des alten. Die ganze Gefahr sitzt im Stilllegen.

**Stilllegen ist label-only und nur-additiv:**
- Artefakt-Globs des alten Bausteins **hören auf zu greifen** → seine schon committeten Dateien werden zu **Waisen** in ihrer Heimat (nichts wird verschoben/gelöscht, nur das Label fällt weg).
- Ignore- und LFS-Regeln des alten Bausteins **bleiben als Sediment liegen** und werden **nie automatisch entfernt** (Entfernen ist die einzige Operation, die alten Müll wieder sichtbar macht oder ein teures `git lfs migrate` auslöst).
- Der neue Baustein wird wie am ersten Tag onboardet (seine LFS-Muster stehen, *bevor* das Tool seine erste Binärdatei erzeugt → **Tag-1-Pflicht jetzt pro Baustein-Onboarding**).
- Physisches Aufräumen alten Tool-Krams ist **kein** Teil des Austauschs, sondern nur eine getrennte, bewusste „Aufräumen"-Aktion, die ehrlich sagt, dass sie Historie anfasst.

---

## Sediment
Tote, aber unschädliche Schutzregeln (Ignore-/LFS-Zeilen) eines stillgelegten Bausteins, die in den Dotfiles **liegen bleiben**. Behalten ist billig; Entfernen kann wehtun. (Gleiche Logik wie das Behalten der Zwischenstände in Sitzung 2: behalten, nie umschreiben.)

---

## Dotfiles als alleinige Wahrheit (Anti-Drift)
Für **Ignore** und **LFS** sind `.gitignore` und `.gitattributes` die **alleinige Wahrheit** — Git liest nur diese. Eine Kopie der Muster im `_plm` wäre dieselbe Drift, die andernorts erschlagen wurde.
- Der **Produkt-Stack im `_plm`** besitzt nur, **was Git nicht kennt**: Zuordnung Artefaktname ↔ Glob, Heimat, Öffnen-Aktion, Startaufgaben, Hand-/Paar-Kanten.
- **Onboarding** hängt die Ignore-/LFS-Zeilen eines Bausteins **idempotent** an die Dotfiles an, eingefasst in einen **Marker-Block** (`# >>> baustein: zephyr >>>` … `# <<<`), damit das Tool *seine* Zeilen beim Austausch wiederfindet.
- **Hand-Edits gewinnen automatisch**, weil sie die Wahrheit *sind*. Alles außerhalb der Marker-Blöcke fasst das Tool nie an.
- „Sediment bleibt liegen" heißt schlicht: der Marker-Block des alten Bausteins bleibt unangetastet stehen.

---

## Version vs. Commit (zentrale Trennung)
Das Originaldokument ließ „Version" zwei Jobs gleichzeitig machen. Ab jetzt getrennt:

- **Commit (Zwischenstand / Auto-Speicherung)** — jeder gespeicherte Zwischenstand. Billig, automatisch, im Hintergrund. Das unsichtbare Sicherheitsnetz / der Rückgängig-Verlauf. Existierte im Originalkonzept gar nicht.
- **Version / Meilenstein** — ein bewusst benannter Stand (`Rev A`, `v0.4`, `Serie 2026-01`), den du fertigst oder freigibst. Technisch ein **Tag** auf einem bestimmten Commit. Trägt die ganze Zeremonie: Notizen, Status, Freigabe.

Merksatz: *Du taggst nicht jeden Tastendruck.* Ein Meilenstein ist, was du herstellen oder ausliefern könntest.

---

## „Wohin" vs. „Wie" (die Git-Sichtbarkeitsgrenze)
Nicht *Git verstecken*, sondern: **Git's Denkmodell darf durchscheinen, Git's Fallstricke nicht.** Test, auf welche Seite etwas gehört:

- **„Wohin" — darf sichtbar sein:** Commit, Branch, History-Graph, Tag, „diese Version baut auf jener auf", „bring mich zu Rev A". Orte und Zustände. Geben Übersicht.
- **„Wie" — automatisiert, nie Aufgabe des Users:** `stash`, `revert <hash>`, `reset --hard`, rebase, Konflikte von Hand lösen, LFS-Tracking & `migrate`, `gc`/`prune`. Beschwörungsformeln, fehleranfällig, hier verliert man Daten.

Regel: Der User darf wissen, dass er auf Git arbeitet und im Graphen denken — er soll nie aufgefordert werden, eine Recovery-Formel zu tippen. Will er „zu Rev A", klickt er den Punkt an; das Tool führt das gefährliche Kommando im Hintergrund aus.

---

## Drei Sorten „Kopie"
Das Originaldokument benutzte „Kopie", als gäbe es nur eine. Es gibt drei:

- **Echte Kopie (deep copy)** — jedes Byte dupliziert. Das beschrieb das Originalkonzept (volle Versionsordner).
- **Reflink / CoW-Kopie** (`cp --reflink`, macOS `clonefile`) — sieht aus wie vollständige unabhängige Kopie, teilt aber Datenblöcke bis zur Änderung. Sofort, quasi kostenlos. Braucht ein CoW-Dateisystem (APFS/Btrfs/XFS/ReFS), funktioniert **nicht** auf Cloud-Sync-Ordnern.
- **Hardlink** — ebenfalls geteilt, aber gefährlich: Ändern einer „Kopie" ändert alle. Ungeeignet.

---

## Branch vs. Worktree
- **Branch** — *keine* Kopie auf der Platte. Nur ein Zeiger auf einen Commit. Alle Branches teilen sich eine Objektdatenbank; normalerweise ist nur einer ausgecheckt.
- **Worktree** (`git worktree add`) — ein zweites, vollständig ausgechecktes Arbeitsverzeichnis als echter Ordner auf der Platte. So bekommst du das „vollständige Versionsordner"-Gefühl on demand, ohne den Cloud-Schmerz.

Wichtig: Git dedupliziert die *Historie* und den *Cloud-Transfer*, **nicht** die ausgecheckten Arbeitsdateien. N gleichzeitige Worktrees = N volle Ordner Plattenplatz. „Alle Versionen als Ordner *und* kein Extra-Platz" ist mit keiner Technik machbar.

---

## Git-LFS
Erweiterung, die große Binärdateien aus der normalen Git-Historie heraushält und per Hash speichert (identische Blobs nur einmal = Deduplizierung). **Fallstrick:** Die Tracking-Muster (`.gitattributes`) müssen stehen, *bevor* die erste große Binärdatei committet wird — sonst teures Historie-Umschreiben (`git lfs migrate`). → Gehört auf die „Wie"-Seite: vollständig vom Tool verwaltet. (Tag-1-Pflicht jetzt auch pro Baustein-Onboarding, s. o.)

---

## Technisch vollständig vs. Freigegeben
- **Technisch vollständig** — automatisch berechneter Zustand (alle Pflicht-Artefakte da, keine blockierenden Aufgaben offen). PLM-Metadaten, orthogonal zu Git.
- **Freigegeben** — immer manuell. Unter Git: ein Tag auf einem unveränderlichen Commit → Schreibschutz gibt es geschenkt.

---

## Arbeitsbereich (harter Anker) vs. Artefakt (weiches Label)
Im Originaldokument lagen beide auf einer Ebene. Jetzt sauber getrennt nach *Härte*:

- **Arbeitsbereich — harter Anker.** Der echte Ordner, in dem eine Datei physisch *liegt* (`elektronik/`, `mechanik/`). Git trackt jede Datei an ihrem Pfad; diesen Anker verliert eine Datei nie. (Im Stack-Modell: die **Heimat** eines Bausteins.)
- **Artefakt — weiches Label.** Die Bedeutung *über* der Datei (was sie *ist*: Schaltplan, BOM). Kann fehlen, ohne dass die Datei verlorengeht.

Folge: Eine **Waise** (unzugeordnete Datei) ist nur ein *fehlendes Label*, kein Umzug. Sie bleibt physisch bei ihren Ordner-Geschwistern. Das **Unzugeordnet-Fach ist pro Arbeitsbereich**, nie global — so bleibt der Ordnerkontext als stärkster Zuordnungshinweis erhalten.

---

## Pattern-Zuordnung vs. Hand-Zuordnung
- **Pattern → Artefakt (Default).** Ein Artefakt ist eine *Erwartung mit Muster* (`*.kicad_sch` in `elektronik/` = Schaltplan). Das Tool ordnet automatisch zu (convention over configuration). Die Muster kommen aus dem **Baustein** (Sitzung 3).
- **Hand-Zuordnung (Korrektur).** Nur nötig, wenn eine Datei zu *keiner* Regel passt.

---

## Ignore-Presets (Zwilling der Artefakt-Regeln)
Pro Tool mitgelieferter Regelsatz, der bekannten Müll (`*.kicad_prl`, `*-backups/`, `fp-info-cache`, `.lck`, `.DS_Store`, `.pio/`) aus Tracking *und* Statusanzeige heraushält. Nicht selbst erfinden — fertige `.gitignore`-Vorlagen existieren (GitHub-KiCad-Template). Erste Klasse, **Tag-1-Pflicht** zusammen mit den LFS-`.gitattributes`. Im Stack-Modell Teil eines **Bausteins**.

Regel für unzugeordnete Dateien: **alles wird getrackt, außer es matcht explizit eine Ignore-Regel.** „Nichts geht durch Weglassen verloren" bleibt damit wahr.

---

## Abgeleitet-von-Kante (dreistufig seit Sitzung 3)
Eine deklarierte Beziehung „Artefakt X ist **abgeleitet von** Quell-Artefakten {A, B, …}" (z. B. Fertigungsdaten ← PCB-Layout; Pick-and-Place ← PCB-Layout *und* BOM).

Herkunft jetzt in **drei Stufen** (vorher: nur Hand):
- **Baustein-Default** — Kante *innerhalb* eines Bausteins (Gerber ← Layout). Kommt beim Onboarding automatisch, keine Handarbeit.
- **Baustein-Paar-Default** — Kante, die zwei *bekannt zusammengehörige* Bausteine überspannt (Pick-and-Place ← Layout + BOM). Liegt als „wenn A und B beide im Stack, schlage Kante Z vor" bei den Bausteinen → **deterministischer Vorschlag** beim Onboarding, per Klick bestätigt. Kein ML, keine Daten, kein Parser.
- **Hand-Kante** — nur noch das echt Idiosynkratische, das keine Paarung vorhersagt.

Alle drei landen im `_plm` des **Produkt-Stacks** (einziger Ort, der den fertig zusammengesteckten Gesamtsatz kennt).

Unveränderter Kern:
- Die Quelle ist eine **Menge**, flach — keine Ketten, keine Tiefe.
- **Stale-Check** rein über die **Git-Reihenfolge** (kein Inhalt, kein Parser): wurde *irgendeine* Quelle nach dem letzten Stand der Ableitung geändert?
- Ergebnis ist eine **Warnung, kein Block**, v. a. beim Meilenstein.
- **Grenze:** fängt „veraltet" (Quelle neuer als Ableitung), *nicht* „aus falscher/ungespeicherter Datei exportiert". Frischer Zeitstempel = „nicht veraltet", nicht „garantiert richtig".
- **Stilllege-Verhalten:** Wird ein Baustein stillgelegt und seine Quell-Artefakte zu Waisen, geht die Kante **still in Ruhe** (kein Fehler, kein Block) — gleiche Label-only-Logik.

---

## Task vs. Hinweis
Zwei Aufgabentypen, getrennt durch **Blockier-*Fähigkeit*** — nicht durch Wichtigkeit:

- **Task** — verpflichtend, *kann* eine Freigabe blockieren. Hängt an einer Sache (Artefakt/Version/Branch).
- **Hinweis** — anekdotisch, „zur rechten Zeit aufdringlich", *blockiert aber nie*, egal wie laut.

(Bewusst nicht „Reminder" genannt — zu nah am Wecker, den das Konzept anderweitig kennt.)

---

## Branch-Strenge
Die Blockier-Wirkung einer Aufgabe ist **Eigenschaft des Branch-Typs**, in dem sie sitzt, nicht der Aufgabe selbst.

- **Prototyp-/Experiment-Branch — lasch:** offene Tasks dösen, blockieren nicht.
- **Production/Release — streng:** offene Tasks blockieren.
- Eine Aufgabe **erbt** die Strenge ihres Branches. Sie greift an *jedem Übergang nach oben*: **Tag setzen** *und* **Merge nach Production** (E7).
- **Opt-out pro Task:** Schalter „blockiert überall" für den seltenen kontextunabhängigen Fall.

---

## Dreistufiger Freigabe-Block (Sitzung 3)
Beim Meilenstein/Tag werden offene Punkte **nicht** auf einen Haufen geworfen, sondern nach Härte gestaffelt:

1. **Warnung** — Stale-Kante (Abgeleitet-von). Wird gezeigt, blockiert nie, braucht keine Begründung.
2. **Weicher Block** — Waise oder fehlendes Pflicht-Artefakt. Blockiert „technisch vollständig", aber per **protokollierter Begründung** bewusst überwindbar („Prototypenstand, Testprotokoll folgt").
3. **Harter Block** — offener blockierender Task auf strengem Branch. **Nicht** per Begründungstext wegzudrücken; nur durch **Erledigen / Verwerfen / Herabstufen zum Hinweis** des Tasks *selbst* überwindbar. Sperrt nie aus (der Ausweg ist *ein Klick auf den Task*), bleibt aber ein bewusster Handgriff am Blocker.

---

## Design-Haltung: streng am Checkpoint, Ausweg einen Handgriff entfernt
Querschnitt-Prinzip, das fast alle Entscheidungen trägt: **Im Alltag fragt das Werkzeug nichts und blockiert nichts; streng wird es nur am bewussten Checkpoint (Meilenstein/Merge nach oben). Und selbst dort ist der Ausweg immer genau einen bewussten Handgriff entfernt — nie null (sonst Fassade) und nie unmöglich (sonst sperrt es dich gegen dich selbst aus und wird umgangen).**
