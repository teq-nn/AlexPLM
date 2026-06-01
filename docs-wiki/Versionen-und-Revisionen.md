# Versionen & Revisionen

So denkt das Werkzeug über die Zeit — und warum du fast nie eine Commit-Nachricht schreibst.

## Stand / Commit: das stille Speichern

Während du arbeitest, beobachtet das Werkzeug deine Arbeitsbereiche. Wenn sich das Speichern
„beruhigt" hat (kurz nach dem letzten Schreiben, nicht bei jedem Tastendruck), legt es im
Hintergrund **still einen Commit** an. Das ist ein Sicherungspunkt deiner Arbeit.

Das Entscheidende: Du tippst dafür **keine Commit-Nachricht**. Die Nachricht ist maschinell
und langweilig. Frisch entstandene Commits erscheinen rechts in der **Commits**-Schiene und
als Knoten im Versionsbaum.

> **Du benennst nicht jeden Tastendruck**
>
> Der Auto-Commit bleibt automatisch — auch wenn er jetzt offen „Commit" heißt
> (siehe [Git-Ehrlichkeit](Git-Ehrlichkeit)). Menschlicher Text entsteht nur an einer
> Revision.

## Revision: der bewusst benannte Stand

Eine **Revision** ist ein Stand, den du bewusst zu einer **benannten Version** erhebst
(z. B. `v0.4`, `Rev B`, `Serie 2026-01`). Technisch ist sie ein **Tag** auf einem Commit —
ein benannter *Punkt auf einer Linie*, **kein** Zweig. Eine Revision ist, was du herstellen
oder ausliefern könntest.

Beim Erheben schreibst du eine kurze Zusammenfassung in die `VERSION_NOTES.md` — das ist der
*einzige* Ort für deinen Text. Im Versionsbaum sind Revisionen als helle Knoten mit ihrem
Versionsetikett markiert:

![Versionsbaum mit den Revisionen v0.1 bis v0.4](img/versionsbaum.png)

> **ℹ️ „Meilenstein" ist jetzt reserviert**
>
> Was früher „Meilenstein" hieß, heißt jetzt **Revision**. Das Wort *Meilenstein* ist
> bewusst freigeräumt und künftigen **Zukunftszielen** vorbehalten (geplante Ziele als in
> der Zukunft liegende Baumknoten).

### Zwei Arten von Revision

Eine Revision hat eine **Art**, die ihre Strenge bestimmt:

| Art | Bedeutung | Verhalten |
|---|---|---|
| **Prototyp** | lockerer Zwischenstand | bearbeitbar, lax — der Standard für eine neue Revision |
| **Freigabe** | abgeschlossener, geprüfter Stand | **schreibgeschützt**; bewusst und umkehrbar umschaltbar |

Im Promote-Dialog erhebst du einen Commit zur Revision: Feld **„Version"** (z. B. `v1.0`),
Feld **„VERSION_NOTES.md"** („Was macht diesen Commit vorzeigbar?"), dann **„Festschreiben"**.
Ist es bereits eine Revision, kannst du sie per **„Freigeben"** auf die Art *Freigabe* heben
(schreibgeschützt, streng) oder per **„Zurückschalten"** wieder zum Prototyp machen
(„Un-Release") — der Ausweg ist immer einen Handgriff entfernt.

## Drei Zustände, die man nicht verwechseln darf

Diese drei Wörter beschreiben **verschiedene** Dinge:

| Wort | Was es bedeutet | Eigenschaft von … |
|---|---|---|
| **gesichert** | im **privaten** Backup auf dem Server (erreicht die geteilte Linie nie) | dem Sicherungs-Status |
| **veröffentlicht** | der Stand liegt auf der **geteilten** Linie | dem **Ort** des Stands |
| **freigegeben** | die schreibgeschützte **Art** einer Revision | der **Reife** der Revision |

Der wichtige Punkt: **veröffentlicht** und **freigegeben** sind unabhängig. Ein
*Prototyp*-Stand kann auf der geteilten Linie liegen (also „veröffentlicht" sein), ohne
„freigegeben" zu sein. Im Versionsbaum trägt jeder Knoten daher ein eigenes Abzeichen
**„veröffentlicht"** (ja/nein), getrennt von der Revisions-Art.

Wie ein Stand veröffentlicht wird, steht unter [Mehrbenutzer & Sync](Mehrbenutzer-und-Sync).

## Das Freigabe-Gate

Wenn du eine Revision zur **Freigabe** machst, sammelt das Werkzeug die offenen Punkte in
**einer** nach Härte sortierten Liste und zeigt **einen** Knopf, der seine Bedeutung wechselt:

- **alles sauber** (oder nur Warnungen) → der Knopf heißt **„Taggen"** und gibt frei;
- **weicher Block** (technisch unvollständig, aber bewusst überwindbar) → **„Trotzdem
  freigeben"**, erst nach einem **protokollierten Begründungssatz** klickbar;
- **harter Block** (eine offene blockierende Aufgabe) → der Knopf ist **aus** („gesperrt durch
  Aufgabe"); daneben stehen die Auswege direkt an der Aufgabe: **erledigen**, **verwerfen**,
  **zum Hinweis**.

Offene Punkte stammen aus drei Quellen: offene **Aufgaben**, **Waisen / fehlende
Pflicht-Artefakte** und veraltete **Kanten** (Stale-Warnungen). Außerdem warnt das Gate, wenn
du den frischen Stand einer Kollegin mit-taggen würdest.

Die `VERSION_NOTES.md` ist dabei **Ergebnis** der Freigabe, keine Vorbedingung: dein
Zusammenfassungs-Text ist die Eingabe, das Taggen erzeugt die Datei.

> **ℹ️ Schreibschutz schützt vor Versehen**
>
> Eine freigegebene Revision ist schreibgeschützt (technisch ein Tag auf einem
> unveränderlichen Commit). Willst du daran weiterarbeiten, entsteht bewusst ein neuer Stand
> oder eine Variante — abgeschlossene Stände bleiben so vor versehentlichen Änderungen
> geschützt.

## Varianten & Versionsnummern

Eine **Variante** ist eine echte zweite Linie (ein Branch) für Experimente oder
Produktvarianten (im Bild oben: `alternate-enclosure`). Sie ist **orthogonal** zur Revision:
Eine Revision ist ein benannter Punkt *auf* einer Linie, eine Variante eine eigene Linie.
Eine Version wird eindeutig durch **Produkt + Linie + Versionsname** identifiziert — darum
zeigt die Versionsleiste beides zusammen.

Versionsnamen schlägt das Werkzeug vor, erzwingt aber kein Schema: `v0.1`, `v1.0`, `Rev A`,
`Prototype 1`, `Serie 2026-01` sind alle erlaubt. Wie du eine Variante anlegst, steht unter
[Werkbank & Graph-Raum](Werkbank-und-Graph-Raum#der-graph-raum-verlauf).
