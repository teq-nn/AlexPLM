# Glossar — geschärfte Begriffe

Stand: 29.05.2026 (2. Sitzung). Entstanden beim Grillen des PLM-Konzepts. Diese Begriffe haben im Gespräch eine *präzise* Bedeutung bekommen, die im Originaldokument noch verschwommen war.

---

## Werkzeug vs. Produkt
- **Werkzeug** — etwas, das du primär für dich selbst baust. Markt, Preis, Wettbewerb, Multi-User sind irrelevant. Maßstab: „trägt der Ansatz technisch, und lohnt er den Aufwand gegenüber dem Status quo?"
- **Produkt** — soll vor anderen im Markt landen. Maßstab: „sollte es existieren, gibt es das schon, warum würde jemand wechseln?"

**Entscheidung:** Dieses Vorhaben ist ein **Werkzeug**. (Test bestanden: „auch bauen, wenn es sicher niemand außer dir nutzt" → ja.)

---

## Version vs. Commit (zentrale Trennung)
Das Originaldokument ließ „Version" zwei Jobs gleichzeitig machen. Ab jetzt getrennt:

- **Commit (Zwischenstand / Auto-Speicherung)** — jeder gespeicherte Zwischenstand. Billig, automatisch, im Hintergrund. Das unsichtbare Sicherheitsnetz / der Rückgängig-Verlauf. Existierte im Originalkonzept gar nicht.
- **Version / Meilenstein** — ein bewusst benannter Stand (`Rev A`, `v0.4`, `Serie 2026-01`), den du fertigst oder freigibst. Technisch ein **Tag** auf einem bestimmten Commit. Trägt die ganze Zeremonie: Notizen, Status, Freigabe.

Merksatz: *Du fabst nicht jeden Tastendruck.* Ein Meilenstein ist, was du herstellen oder ausliefern könntest.

---

## „Wohin" vs. „Wie" (die Git-Sichtbarkeitsgrenze)
Nicht *Git verstecken*, sondern: **Git's Denkmodell darf durchscheinen, Git's Fallstricke nicht.** Test, auf welche Seite etwas gehört:

- **„Wohin" — darf sichtbar sein:** Commit, Branch, History-Graph, Tag, „diese Version baut auf jener auf", „bring mich zu Rev A". Orte und Zustände. Geben Übersicht.
- **„Wie" — automatisiert, nie Aufgabe des Users:** `stash`, `revert <hash>`, `reset --hard`, rebase, Konflikte von Hand lösen, LFS-Tracking & `migrate`, `gc`/`prune`. Beschwörungsformeln, fehleranfällig, hier verliert man Daten.

Regel: Der User darf wissen, dass er auf Git arbeitet und im Graphen denken — er soll nie ausgefordert werden, eine Recovery-Formel zu tippen. Will er „zu Rev A", klickt er den Punkt an; das Tool führt das gefährliche Kommando im Hintergrund aus.

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
Erweiterung, die große Binärdateien aus der normalen Git-Historie heraushält und per Hash speichert (identische Blobs nur einmal = Deduplizierung). **Fallstrick:** Die Tracking-Muster (`.gitattributes`) müssen stehen, *bevor* die erste große Binärdatei committet wird — sonst teures Historie-Umschreiben (`git lfs migrate`). → Gehört auf die „Wie"-Seite: vollständig vom Tool verwaltet.

---

## Technisch vollständig vs. Freigegeben
- **Technisch vollständig** — automatisch berechneter Zustand (alle Pflicht-Artefakte da, keine blockierenden Aufgaben offen). PLM-Metadaten, orthogonal zu Git.
- **Freigegeben** — immer manuell. Unter Git: ein Tag auf einem unveränderlichen Commit → Schreibschutz gibt es geschenkt.

---

## Arbeitsbereich (harter Anker) vs. Artefakt (weiches Label)
Im Originaldokument lagen beide auf einer Ebene. Jetzt sauber getrennt nach *Härte*:

- **Arbeitsbereich — harter Anker.** Der echte Ordner, in dem eine Datei physisch *liegt* (`elektronik/`, `mechanik/`). Git trackt jede Datei an ihrem Pfad; diesen Anker verliert eine Datei nie.
- **Artefakt — weiches Label.** Die Bedeutung *über* der Datei (was sie *ist*: Schaltplan, BOM). Kann fehlen, ohne dass die Datei verlorengeht.

Folge: Eine **Waise** (unzugeordnete Datei) ist nur ein *fehlendes Label*, kein Umzug. Sie bleibt physisch bei ihren Ordner-Geschwistern. Das **Unzugeordnet-Fach ist pro Arbeitsbereich**, nie global — so bleibt der Ordnerkontext als stärkster Zuordnungshinweis erhalten.

---

## Pattern-Zuordnung vs. Hand-Zuordnung
- **Pattern → Artefakt (Default).** Ein Artefakt ist eine *Erwartung mit Muster* (`*.kicad_sch` in `elektronik/` = Schaltplan). Das Tool ordnet automatisch zu (convention over configuration).
- **Hand-Zuordnung (Korrektur).** Nur nötig, wenn eine Datei zu *keiner* Regel passt.

---

## Ignore-Presets (Zwilling der Artefakt-Regeln)
Pro Werkzeug mitgelieferter Regelsatz, der bekannten Müll (`*.kicad_prl`, `*-backups/`, `fp-info-cache`, `.lck`, `.DS_Store`) aus Tracking *und* Statusanzeige heraushält. Nicht selbst erfinden — fertige `.gitignore`-Vorlagen existieren (GitHub-KiCad-Template). Erste Klasse, **Tag-1-Pflicht** zusammen mit den LFS-`.gitattributes`.

Regel für unzugeordnete Dateien: **alles wird getrackt, außer es matcht explizit eine Ignore-Regel.** „Nichts geht durch Weglassen verloren" bleibt damit wahr.

---

## Abgeleitet-von-Kante
Eine deklarierte Beziehung „Artefakt X ist **abgeleitet von** Quell-Artefakten {A, B, …}" (z. B. Fertigungsdaten ← PCB-Layout; Pick-and-Place ← PCB-Layout *und* BOM).

- Die Quelle ist eine **Menge**, flach — keine Ketten, keine Tiefe.
- **Stale-Check** rein über die **Git-Reihenfolge** (kein Inhalt, kein Parser, vgl. §30): wurde *irgendeine* Quelle nach dem letzten Stand der Ableitung geändert?
- Ergebnis ist eine **Warnung, kein Block**, v. a. beim Meilenstein.
- **Grenze:** fängt „veraltet" (Quelle neuer als Ableitung), *nicht* „aus falscher/ungespeicherter Datei exportiert". Frischer Zeitstempel = „nicht veraltet", nicht „garantiert richtig".

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
