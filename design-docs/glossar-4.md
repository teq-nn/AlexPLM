# Glossar — geschärfte Begriffe

Stand: 29.05.2026 (5. Sitzung). Fortschreibung von `glossar-3.md`. Diese Begriffe haben in Sitzung 5 eine präzise Bedeutung bekommen: die Mehrbenutzer-Topologie, die tragende Binär-Invariante samt der zwei Push-Arten, die Bloat-Deckelung mit Archiv-Auslagern, sowie Import, Auto-Commit, manuelle Kanten, der stille Sync und der dritte Merge-Eimer. Begriffe aus Sitzung 1–4 (Werkzeug/Tool, Baustein/Stack, Sediment, Werkbank/Graph-Raum, Wohin/Wie, Bibliothek, abgeleiteter Status, Auto-Lock/`lockable`, Koordination vs. Autorisierung, Git-Client-Grenze usw.) gelten unverändert weiter.

---

## Beide-auf-`main` (Mehrbenutzer-Topologie)
Im geteilten Fall arbeiten alle direkt auf demselben `main` und gleichen per Pull/Push ab — **keine** personengebundenen Dauer-Branches. Branches bleiben bewusste Varianten/Experimente (E27/E32), nie „mein Bereich".

Folge: divergierende Merges sind häufig, aber **harmlos** — sie berühren fast nur Text (E7 merget), Binärdateien sind dank der Invariante zum Pull-Zeitpunkt frei. Gegenmodell (personengebundene Branches) wäre gefährlicher: seltene, große Merges, die eher eine *offen gehaltene* Binärsperre treffen.

---

## Binär-Invariante
Der eine Satz, auf dem die Sicherheit des Binär-Mehrbenutzer-Falls ruht:

> *Eine gesperrte Binäränderung darf den geteilten Stand nicht erreichen, solange die Sperre gehalten wird.*

Hält sie, ist die Merge-Landmine strukturell tot: was beim Pull je sichtbar wird, ist schon gepusht → schon entsperrt → der Merge berührt nur freie Dateien. Hintergrund (Realität): git-lfs-Locks sind **pfadbasiert und branchübergreifend**, und eine **gesperrte Datei ist nicht mergebar** — der Merge kippt schon beim bloßen Berühren. Ohne die Invariante würde eine Sperre von Koordination (E31) zum **Sync-Blocker** (jemand scheitert am Pull wegen einer fremden Sperre auf einer Datei, die er nie anfasst).

---

## Freigabe-Push vs. Sicherungs-Push
Zwei scharf getrennte Push-Arten (tragen die Invariante):

- **Freigabe-Push** — bringt die fertige Binärdatei auf den **geteilten** `main`-Stand und **ist** das Loslassen der Sperre („ich bin fertig damit"). Eine Binärdatei unter aktiver Sperre wird **nie zwischendurch** auf den geteilten Stand gepusht.
- **Sicherungs-Push** — spiegelt lokale Zwischen-Commits (inkl. halbfertiger Binärdatei) in einen **persönlichen** Backup-Bereich auf dem Remote (eigener Ref/Namespace, Sicherheitsnetz E8). **Backup ja, Freigabe nein.**

Merksatz: *Der Freigabe-Push ist ein öffentlicher Akt, der Sicherungs-Push ein privater.* `git lfs unlock` ist ein eigener expliziter Befehl — „unlock at push" leistet das Werkzeug selbst.

---

## Bloat-Deckelung
Auto-Saves machen nur **lokale** Commits (E5 unangetastet); die Binärdatei erreicht den **LFS-Store erst beim Freigabe-Push**. Damit liegt **eine** Voll-Version pro Meilenstein im Store statt eine pro Save. Hintergrund: LFS hält das *git*-Repo schlank (Pointer), aber jede **gepushte** Binärversion liegt *für immer* im Store. Lokaler Hausputz: `git lfs prune` (gibt nur den eigenen Cache frei, fasst Server/Historie nicht an).

---

## Auslagern (Archiv pro Meilenstein-Alter)
Die nicht-destruktive Antwort auf Langfrist-Wachstum: statt alte Blobs zu löschen, exportiert das Werkzeug die schweren **LFS-Inhalte** alter Meilensteine in ein **Archiv beim Nutzer** (NAS/Kaltplatte) und entfernt **nur diese OIDs** vom Server.

- **Nie Historie/Text:** Commits, Tags, `VERSION_NOTES.md` und die LFS-**Pointer** bleiben für immer auf dem Server (winzig). Nur der schwere *Inhalt* wandert von „heißem Server" auf „kaltes Archiv". „Archiv" heißt *Inhalt alter Meilensteine auslagern*, niemals „Historie wegpacken".
- **Voll reversibel:** LFS-Blobs sind inhaltsadressiert (OID) → Archiv einspeisen → Blobs wieder im Store → „Als Ordner öffnen" (E27) funktioniert wieder. Ehrt „behalten, nie umschreiben" (E9) und „Kopie heraus, nie zurück" (E27).
- **Ehrlicher Preis:** Archiv verloren = diese alten Binärstände wirklich weg (Verantwortung beim Nutzer, nicht beim Werkzeug). Ausgelagerte Knoten tragen im Graph „Binärinhalt ausgelagert (Archiv vom …)" — gleiche Offline-Ehrlichkeit wie die Suche (E30).
- **Granularität:** pro **Meilenstein-Alter** („älter als X auslagern"). Reiner Batch-Download ohne Server-Entlastung ist *kein* Auslagern — das wäre nur eine zweite Datenkopie (Drift, E8/E18).

---

## Import-Flow & das `lfs migrate`-Gate
Bestehenden Ordner zum Produkt machen: zeigen → `git init` falls nötig → Blatt-Ordner-Bausteine erkennen → `.gitattributes`-Marker schreiben → erster Commit.

Der gefährliche Zweig: Riesen-Binaries liegen **schon in der git-Historie** → `git lfs migrate` = **Historie umschreiben**. Das **Gate**: nur hinter „Historie anfassen"-Bestätigung (E27) **und nur bei frischem/ungeteiltem Repo**. Geteilte Klone vorhanden → das Werkzeug **verweigert** die Umschreibung (sie würde fremde Klone vergiften). Solo-Fall (nie in git) ist sauber.

---

## Stand vs. Commit (Sichtbarkeitsgrenze)
Konsequenz der stillen Auto-Commit-Schleife: Der Nutzer sieht **Stände im Graph**, nie **Commits**. Auto-Commit löst **entprellt** aus („Save beruhigt sich", nicht pro Tastendruck); die Message ist **maschinell und langweilig** („auto: …, Zeitstempel"). Der Nutzer schreibt auf dem Happy Path **nie** eine Commit-Message — täte er es, wäre das Werkzeug ein Git-Client (E33). Menschlicher Text entsteht nur am Meilenstein (E28 → `VERSION_NOTES.md`).

---

## Kante (Abgeleitet-von) — v1 manuell
Eine Kante verbindet zwei Artefakte als „dies stammt aus jenem". In v1 **rein manuell** gezogen (Geste auf der Artefakt-Karte). Die **Stale-Warnung** (E26, Quelle neuer als Ableitung) existiert **nur**, wo eine Kante von Hand gezogen wurde — **keine Kante = keine Warnung**. Kanten sind **Opt-in-Mehrwert, keine Pflicht**; kein gefälschter Graph. Die Vorschlags-Heuristik (E21) bleibt auf Eis.

---

## Stiller Sync vs. Setup-Zeremonie
Zwei Momente des Mehrbenutzer-Falls, scharf getrennt:

- **Einrichtungs-Zeremonie** — einmalig pro Produkt (Server anbinden, erster Push, Kollegen einladen). Darf **git-nah** bleiben; selten, risikoarm, später hübschbar.
- **Täglicher Sync** (Push/Pull) — **still im Hintergrund**, wie der Commit (E39): pullen beim Öffnen/in Ruhe, pushen an den Checkpoints (E35). Der Nutzer sieht „aktuell / X arbeitet an Y / gesichert", nie „push/pull".
- **Laute Ausnahme** — bei echtem, unauflösbarem Widerspruch hält der Sync an und fragt in **eigener** Sprache („wessen Stand gilt?"), nie git-Konflikt-Marker.

Merksatz: *Nie zeigen ist leichter als später wieder verstecken.* Die Zeremonie ist einmalig (git-Sprache vertretbar), der Sync täglich (E34) — und einen täglichen Reflex kaschiert man nicht nachträglich weg (E33).

---

## Mergebar vs. nicht-mergebar (der dritte Eimer)
Verfeinerung der Achse Text-vs-Binär (E31): Was zählt, ist nicht „Text oder Binär", sondern **mergebar oder nicht**. Drei Eimer statt zwei:

- **Echter Text, mergebar** — Firmware, Doku, BOM-Text → git merget (E7).
- **Binär, unmergebar** — `.f3d`, STEP, STL, Fotos → sperren (E31).
- **Nominell Text, faktisch unmergebar** — KiCad-Quellen (`.kicad_sch`/`.kicad_pcb`): Merges können die Datei zerstören („Missing („-Fehler). Gehören praktisch zu „sperren wie Binär".

In v1 nur als **laute Sync-Ausnahme** abgedeckt (E41); die saubere Konsequenz (`lockable` nach Mergebarkeit) ist offen.
