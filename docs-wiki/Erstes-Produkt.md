# Erstes Produkt — Schritt für Schritt

Diese Anleitung führt dich vom ersten Start bis zu deinem ersten Meilenstein. Als
durchgehendes Beispiel dient das Produkt **„Ember Reverb"** (ein Effektpedal).

> **💡 Zwei Wörter, die alles klären**
>
> Im Werkzeug gilt durchgehend: **anlegen schreibt — öffnen liest nur.** Diese Zeile steht
> auch unter den Knöpfen, damit immer klar ist, was eine Aktion bewirkt.

## Schritt 1 — Programm starten

Nach dem Start zeigt das Werkzeug einen ruhigen Startbildschirm mit zwei Wegen, ein Produkt
zu beginnen:

![Startbildschirm: „Neues Produkt" und „Produkt öffnen"](img/leer-startbildschirm.png)

- **Neues Produkt** — legt ein neues Produkt an (schreibt). Du wählst einen Ordner; das
  Werkzeug richtet ihn als Produkt ein.
- **Produkt öffnen** — öffnet ein bestehendes Produkt (liest nur).
- **Suche über Produkte** — durchsucht alle bekannten Produkte (oben rechts).

## Schritt 2 — Produkt anlegen oder öffnen

#### Neues Produkt anlegen

Klicke **Neues Produkt** und wähle den Ordner, der dein Produkt werden soll. Das Werkzeug
prüft den Ordner zuerst schonend und richtet ihn dann ein (es legt im Hintergrund die
Versionierung an und markiert sperrbare Dateitypen).

> **ℹ️ Bestehende Ordner sind willkommen**
>
> Du kannst einen Ordner mit vorhandenen Dateien anlegen — das Werkzeug übernimmt sie
> zerstörungsfrei. Nur im seltenen Fall, dass riesige Binärdateien bereits in einer
> Git-Historie stecken, fragt es vorsichtig nach (siehe
> [Git-Ehrlichkeit](Git-Ehrlichkeit)).

#### Bestehendes Produkt öffnen

Klicke **Produkt öffnen** und wähle den Produktordner. Das Werkzeug liest den Ordner ein,
baut die Werkbank auf und beginnt still, Änderungen zu beobachten.

## Schritt 3 — Werkzeugkasten einrichten

Damit aus deinen Dateien Artefakt-Karten werden, braucht das Produkt einen
**Werkzeugkasten** — die Auswahl der Werkzeuge (Bausteine), mit denen du arbeitest. Hat ein
Produkt noch keinen, lädt dich eine Leiste dazu ein („Werkzeugkasten einrichten"). Du wählst
einen Standard aus der Bibliothek und passt ihn an; das Werkzeug **kopiert** ihn ins Produkt.

![Die Werkzeugkasten-Leiste eines eingerichteten Produkts](img/werkzeugkasten-leiste.png)

Danach zeigt die Leiste ruhig den gewählten Standard und die Zahl der Bausteine, mit einem
dezenten **„erweitern"** für später.

> **ℹ️ Anti-Drift**
>
> Der Werkzeugkasten ist eine **Kopie**. Spätere Änderungen in der Bibliothek verändern dein
> laufendes Produkt nie — Details unter [Bausteine & Werkzeugkasten](Bausteine-und-Werkzeugkasten).

## Schritt 4 — Die Werkbank kennenlernen

Jetzt steht die Werkbank. So sieht ein eingerichtetes Produkt aus:

![Die vollständige Werkbank von „Ember Reverb"](img/werkbank-uebersicht.png)

Orientiere dich an den Zonen (ausführlich in der [Oberflächen-Referenz](Die-Oberflaeche)):

1. **Versionsleiste** (oben) — Produkt, Zweig, aktive Version, Status.
2. **Bausteine / Artefakt-Karten** (Mitte) — dein Arbeitszustand.
3. **Versionsbaum** (rechts, dunkel) — die Historie.
4. **Fremde Sperren & Stände** (ganz rechts) — was Kolleg:innen in Arbeit haben und deine
   jüngsten Sicherungspunkte.

## Schritt 5 — Eine Datei öffnen und bearbeiten

Jede Artefakt-Karte hat eine Ein-Klick-Aktion:

![Eine Artefakt-Karte mit „ÖFFNEN"](img/artefakt-karte-einzeln.png)

- **ÖFFNEN** übergibt die Hauptdatei ans Betriebssystem; sie öffnet sich im Standardprogramm.
- Bei sperrbaren Binärdateien (CAD, Gehäuse) holt das Werkzeug dabei **automatisch die
  Sperre** — die Datei wird für dich beschreibbar, für andere als „gesperrt von dir"
  sichtbar.
- Hat ein Artefakt keine einzelne Hauptdatei (z. B. ein Firmware-Ordner), heißt die Aktion
  **ORDNER ÖFFNEN**.

Während du arbeitest und speicherst, legt das Werkzeug **still Stände** an — du musst dafür
nichts tun und nichts beschriften. Die neuen Stände erscheinen rechts in der Schiene.

## Schritt 6 — Aufgaben festhalten

Unter den Karten liegt die Aufgaben-Liste. Halte hier fest, was noch zu tun ist:

![Aufgaben und Hinweise](img/aufgaben.png)

- **Aufgaben** können später eine Freigabe blockieren.
- **Hinweise** erinnern nur und blockieren nie.

Mehr dazu unter [Aufgaben & Hinweise](Aufgaben-und-Hinweise).

## Schritt 7 — Einen Meilenstein setzen

Wenn ein Stand eine echte Version sein soll, erhebst du ihn im Versionsbaum zu einem
**Meilenstein** und gibst ihm einen Namen (z. B. `v0.4`) und eine kurze Zusammenfassung:

![Der Versionsbaum mit benannten Meilensteinen](img/versionsbaum.png)

- Ein neuer Meilenstein ist zunächst ein **Prototyp** (lax, bearbeitbar).
- Schaltest du ihn auf **Freigabe**, wird er **schreibgeschützt** — der Stand ist
  abgeschlossen.
- Aus deiner Zusammenfassung entsteht automatisch eine lesbare `VERSION_NOTES.md` neben
  deinen Dateien.

Die ganze Logik dahinter steht unter [Versionen & Meilensteine](Versionen-und-Meilensteine).

## Wie geht es weiter?

- Willst du das Produkt im Team nutzen? → [Produkt teilen](Produkt-teilen)
- Unsicher, was ein Bereich oder eine LED bedeutet? → [Die Oberfläche](Die-Oberflaeche)
  und [Status-LEDs](Status-LEDs)
- Begriff nachschlagen? → [Glossar](Glossar)
