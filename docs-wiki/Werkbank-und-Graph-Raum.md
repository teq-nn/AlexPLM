# Werkbank & Graph-Raum

Ein geöffnetes Produkt hat zwei gleichwertige, aber getrennte Räume. Die Trennung ist
bewusst — wie Git selbst zwischen „Jetzt arbeiten" und „Historie ansehen" trennt.

- Die **Werkbank** ist die *Vorderseite*: dein aktueller Arbeitszustand. Hier verbringst du
  den Alltag.
- Der **Graph-Raum** ist der *Versionsbaum*: die Historie, die du *aufsuchst*, um dich zu
  orientieren oder zu einem alten Stand zu springen.

> Der Graph ist die beste Übersicht, nicht die beste Werkbank.

## Die Werkbank

Die Werkbank zeigt den aktuellen Stand als **Artefakt-Karten je Arbeitsbereich**. Sie fragt
und blockiert im Alltag nichts — sie ist ruhig.

### Artefakt-Karten

Jede Karte fasst die Dateien eines Artefakts zusammen, die das Werkzeug per Muster
(aus dem [Baustein](Bausteine-und-Werkzeugkasten)) erkannt hat:

![Eine Artefakt-Karte: Status-LED, Bausteinname, Hauptdatei, Pfad, Ein-Klick-Öffnen](img/artefakt-karte-einzeln.png)

Auf einer Karte siehst du:

- den **Status-LED** oben links (frei / in Arbeit / fremd gesperrt — siehe
  [Status-LEDs](Status-LEDs)),
- den **Bausteinnamen** als Großbuchstaben-Etikett (z. B. `KICAD`),
- die **Hauptdatei** prominent, den echten Pfad gedämpft darunter,
- einen Zähler weiterer Dateien (z. B. `+4`),
- die **Ein-Klick-Aktion** `ÖFFNEN` (Hauptdatei) bzw. `ORDNER ÖFFNEN`.

Ein Klick auf `ÖFFNEN` übergibt die Datei ans Betriebssystem, das sie mit dem
Standardprogramm öffnet. Bei sperrbaren Binärdateien holt das Werkzeug dabei automatisch die
Sperre (siehe [Mehrbenutzer & Sync](Mehrbenutzer-und-Sync)).

Mehrere Karten nebeneinander bilden die Werkbank eines Arbeitsbereichs:

![Mehrere Artefakt-Karten — KiCad (frei), Fusion 360 (fremd gesperrt), Zephyr (Ordner)](img/artefakt-karten.png)

> **ℹ️ Der Status wird gelesen, nicht gespeichert**
>
> Was eine Karte anzeigt (Vorhanden / Geändert / frei / gesperrt …) wird **live aus dem
> Zustand abgeleitet** — nicht als zweite Wahrheit gespeichert. Damit kann der angezeigte
> Status nie von der Realität abdriften.

### Das Unzugeordnet-Fach (Waisen)

Versionierte Dateien, die zu keinem Muster passen, verschwinden nicht — sie sammeln sich im
**Unzugeordnet-Fach** ihres Arbeitsbereichs:

![Das Unzugeordnet-Fach mit zwei Waisen-Dateien im Arbeitsbereich „dokumentation"](img/unzugeordnet-fach.png)

Diese **Waisen** sind nur unetikettiert — der Ordner-Kontext bleibt als Zuordnungs-Hinweis
erhalten. Du kannst eine Waise direkt in der App einem Baustein zuordnen; sie erhält dann
ihre Karte. Es geht nichts dadurch verloren, dass etwas (noch) kein Etikett hat.

## Der Graph-Raum (Verlauf)

Den Versionsbaum gibt es an zwei Orten:

- als **kompakte „Display"-Zone** rechts neben der Werkbank (für die schnelle Orientierung
  im Alltag),
- als **eigenen, vollflächigen Raum** — du wechselst über den Schalter **„Verlauf · Graph"**
  oben in der Leiste dorthin und mit **„Werkbank"** zurück.

![Der Graph-Raum mit Filtern, Revisionen v0.1–v0.4 und einer abzweigenden Variante](img/graph-raum.png)

Hier liest du Abstammung und Orientierung ab: welche Revisionen es gab, wo eine **Variante**
(im Bild blau, `alternate-enclosure`) abgezweigt ist, welche Linie aktiv ist und welche Stände
bereits **veröffentlicht** sind. Was die Knoten bedeuten, steht unter
[Versionen & Revisionen](Versionen-und-Revisionen).

### Filter

Oben im Graph-Raum sitzen zwei Filter, die **nur ausblenden** (nie etwas verändern):

- **Varianten** — Zweige neben der aktiven Linie ein-/ausblenden,
- **nur Revisionen** — nur die benannten Stände zeigen.

### Die drei Knoten-Verben

Ein Klick auf einen alten Knoten verschiebt **nie still** deine Werkbank. Stattdessen bietet
der Knoten drei Verben:

| Verb | Wirkung |
|---|---|
| **Als Ordner öffnen** (Standard) | legt eine **schreibgeschützte Kopie daneben** an (ein Worktree) — deine Werkbank ruht |
| **Von hier abzweigen** | legt eine neue **Variante** (Branch) an; laufende Arbeit wird vorher gesichert |
| **Zurückwerfen** | springt auf diesen Stand — als neuer Stand obendrauf, reversibel. Hinter der **schwarzen Gate-Taste** mit ausdrücklicher Zustimmung |

So bekommst du das „vollständiger Versionsordner"-Gefühl on demand, ohne deinen aktuellen
Arbeitsstand zu gefährden.
