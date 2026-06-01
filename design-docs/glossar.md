# Glossar — geschärfte Begriffe

Stand: 29.05.2026. Entstanden beim Grillen des PLM-Konzepts. Diese Begriffe haben im Gespräch eine *präzise* Bedeutung bekommen, die im Originaldokument noch verschwommen war.

---

## Werkzeug vs. Produkt
- **Werkzeug** — etwas, das du primär für dich selbst baust. Markt, Preis, Wettbewerb, Multi-User sind irrelevant. Maßstab: „trägt der Ansatz technisch, und lohnt er den Aufwand gegenüber dem Status quo?"
- **Produkt** — soll vor anderen im Markt landen. Maßstab: „sollte es existieren, gibt es das schon, warum würde jemand wechseln?"

**Entscheidung:** Dieses Vorhaben ist ein **Werkzeug**. (Test bestanden: „auch bauen, wenn es sicher niemand außer dir nutzt" → ja.)

---

## Version vs. Commit (zentrale Trennung)
Das Originaldokument ließ „Version" zwei Jobs gleichzeitig machen. Ab jetzt getrennt:

- **Stand (Zwischenstand / Auto-Speicherung)** — jeder gespeicherte Zwischenstand. Billig, automatisch, im Hintergrund. Das unsichtbare Sicherheitsnetz / der Rückgängig-Verlauf. Technisch ein Commit; existierte im Originalkonzept gar nicht.
- **Revision** — ein bewusst benannter Stand (`Rev A`, `v0.4`, `Serie 2026-01`), den du fertigst oder freigibst. Technisch ein **Tag** auf einem bestimmten Stand. Trägt die ganze Zeremonie: Notizen, Art (Prototyp/Freigabe). Ein benannter **Punkt auf einer Linie**, **kein** Zweig (→ Variante).

Merksatz: *Du benennst nicht jeden Tastendruck.* Eine Revision ist, was du herstellen oder ausliefern könntest.

**Begriffswechsel (E47, #30):** Was früher „Meilenstein" hieß, heißt jetzt **Revision**. Das Wort **„Meilenstein"** ist freigeräumt und künftig den **Zukunftszielen** vorbehalten (geplante Forgejo-Meilensteine als in der Zukunft liegende Baumknoten).

**veröffentlicht vs. gesichert vs. freigegeben (drei verschiedene Dinge, E47):**
- **gesichert** — im **privaten** Backup (`refs/personal/...`). Der stille Rhythmus. Erreicht die geteilte Linie nie.
- **veröffentlicht** — der Stand liegt auf der **geteilten** Linie (`origin/<shared>`). Erreicht **nur** über `freigeben`; dabei wandert das Revisions-Label mit. *Ort*-Eigenschaft des Stands.
- **freigegeben / Freigabe** — die schreibgeschützte **Art** einer Revision (E42). *Reife*-Eigenschaft, orthogonal zum Ort: ein Prototyp-Stand kann veröffentlicht sein, ohne „freigegeben" zu sein.

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
