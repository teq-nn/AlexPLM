# Aufgaben & Hinweise

Jedes Produkt hat eine einfache, eingebaute Aufgabenverwaltung — direkt unter den
Artefakt-Karten der Werkbank.

![Die Aufgaben-Liste eines Produkts mit Aufgaben und einem Hinweis](img/aufgaben.png)

## Zwei Arten: Aufgabe vs. Hinweis

Es gibt genau zwei Arten von Einträgen, und sie unterscheiden sich **nur** in einer Sache —
ihrer **Blockier-Fähigkeit**, nicht in ihrer Wichtigkeit:

| Art | Kann eine Freigabe blockieren? |
|---|---|
| **Aufgabe** | ja — eine offene Aufgabe *kann* eine Freigabe blockieren |
| **Hinweis** | nein — ein Hinweis blockiert nie, er erinnert nur |

So bleibt die Liste im Alltag ruhig und grau: Erst am Freigabe-Gate kann eine Aufgabe
„laut" werden.

## Felder einer Aufgabe

Eine Aufgabe ist bewusst minimal gehalten:

- **Titel** — das einzige Stück freier, menschlicher Text,
- **Art** — Aufgabe oder Hinweis,
- **Status** — `offen`, `erledigt` oder `verworfen` (kein Kanban),
- **Fälligkeit** — optionales Datum,
- **Verknüpfung** — optional mit Produkt, Version, Arbeitsbereich oder Artefakt,
- **„blockiert überall"** — eine Aufgabe kann kontextunabhängig blockieren.

Eine Aufgabe **muss** mit nichts verknüpft sein; sie darf frei schweben.

## Woher Aufgaben kommen

Aufgaben kannst du jederzeit von Hand anlegen. Zusätzlich können sie als **Startaufgaben**
entstehen, wenn ein [Baustein](Bausteine-und-Werkzeugkasten) in ein Produkt aufgenommen wird (z. B.
„Schaltplan erstellen", „Testprotokoll ablegen").

## Blockier-Logik (an der Revision)

Ob offene Aufgaben eine Freigabe blockieren, hängt von der **Revisions-Art** ab
(siehe [Versionen & Revisionen](Versionen-und-Revisionen)):

- eine **Freigabe** wird von **jeder** offenen Aufgabe blockiert,
- ein **Prototyp** nur von einer offenen Aufgabe mit dem Schalter **„blockiert überall"**,
- ein **Hinweis** blockiert **nie**.

Diese Entscheidung fließt in das **Freigabe-Gate** ein — sie ist nicht an einen Linien-Typ
gebunden, sondern an die Art der Revision, die du gerade setzen willst. Eine blockierende
Aufgabe erscheint dort als **harter Block** mit den Auswegen **erledigen**, **verwerfen** und
**zum Hinweis** direkt an der Aufgabe.
