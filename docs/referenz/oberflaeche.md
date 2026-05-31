# Die Oberfläche

Ein geöffnetes Produkt teilt sich in klar getrennte Zonen. Dieses Bild dient als Landkarte:

![Die Werkbank mit ihren Zonen](../img/werkbank-uebersicht.png)

## 1. Versionsleiste (oben, dunkel)

![Versionsleiste](../img/versionsleiste.png)

Das „Display" des Geräts. Es zeigt auf einen Blick:

- **Produktname** · **Zweig** · **aktive Version** (z. B. `Ember Reverb · main · v0.4`),
- die **Art** der aktiven Version (`PROTOTYP` lax / `FREIGABE` schreibgeschützt),
- rechts die Zahl der **Bausteine** und den Zugang zur **Ansicht**.

Die Versionsnummer ist das größte, hellste Element — die Versionsorientierung ist immer
präsent.

## 2. Einstiegsleiste

Direkt darunter liegen die produktübergreifenden Aktionen: **Neues Produkt**, **Produkt
öffnen** und **Suche über Produkte**. Die Merkzeile *„anlegen schreibt — öffnen liest nur"*
hält den Unterschied präsent.

## 3. Werkzeugkasten-Leiste

![Werkzeugkasten-Leiste](../img/werkzeugkasten-leiste.png)

Zeigt den für dieses Produkt gewählten Werkzeugkasten (Standard + Anzahl Bausteine) mit einem
dezenten **„erweitern"**. Hat das Produkt noch keinen, steht hier die Aufforderung
**„Werkzeugkasten einrichten"**.

## 4. Bausteine / Artefakt-Karten (Mitte)

![Artefakt-Karten](../img/artefakt-karten.png)

Der Arbeitszustand als Karten je Arbeitsbereich. Jede Karte trägt einen
[Status-LED](status-leds.md), den Bausteinnamen, die Hauptdatei mit echtem Pfad und die
Ein-Klick-Aktion **ÖFFNEN** / **ORDNER ÖFFNEN**.

Darunter sammeln sich nicht zugeordnete Dateien im **Unzugeordnet-Fach**:

![Unzugeordnet-Fach](../img/unzugeordnet-fach.png)

## 5. Aufgaben & Hinweise

![Aufgaben](../img/aufgaben.png)

Unter den Karten liegt die Aufgaben-Liste des Produkts — Aufgaben (können blockieren) und
Hinweise (blockieren nie). Siehe [Aufgaben & Hinweise](../konzepte/aufgaben.md).

## 6. Versionsbaum (rechts, dunkel)

![Versionsbaum](../img/versionsbaum.png)

Die Historie als „Display"-Zone: Stände und benannte Meilensteine, abzweigende Zweige (im
Bild blau) und die aktive Linie. Reine Orientierung — ein Klick verschiebt nie still deine
Werkbank.

## 7. Fremde Sperren & Stände (ganz rechts)

![Fremde Sperren](../img/fremde-sperren.png)

- **Fremde Sperren** — welche Binärdateien Kolleg:innen gerade in Arbeit haben
  („gesperrt von X seit …").
- **Stände** — deine jüngsten stillen Sicherungspunkte, neueste zuerst.

## Bewegung & Ton

Die Oberfläche ist im Alltag **leise**: schnelle, unaufdringliche Übergänge, kein Blinken.
Laut wird sie nur an der einen Stelle, an der sie es sein muss — der
[lauten Ausnahme](../konzepte/mehrbenutzer.md#die-laute-ausnahme) beim Abgleich. Dort, und nur
dort, hebt ein oranger Rahmen kurz die Stimme.

## Spalten anpassen

Versionsbaum und die rechte Schiene lassen sich an ihren Trennlinien in der Breite ziehen;
die Werkbank füllt den Rest. Die eingestellten Breiten merkt sich das Werkzeug pro Produkt.
