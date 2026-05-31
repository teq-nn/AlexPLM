# Bausteine, Bibliothek & Werkzeugkasten

Woher weiß das Werkzeug, dass `.kicad_sch`-Dateien zu einem Schaltplan gehören, in
`elektronik/` leben, als sperrbare Binärdaten behandelt werden und mit KiCad geöffnet
werden? Aus einem **Baustein**.

## Baustein

Ein **Baustein** bündelt das Wissen über **ein Werkzeug** (z. B. KiCad, Fusion 360,
Zephyr). Er enthält:

- **Heimat-Ordner** — in welchem Arbeitsbereich das Werkzeug üblicherweise lebt
  (KiCad → `elektronik/`),
- **Artefakt-Muster (Globs)** — welche Dateien zu welchem Artefakt gehören,
- **Ignore-Voreinstellungen** — was nicht versioniert werden soll (Build-Ordner, Caches),
- **LFS-/Sperr-Muster** — welche Dateitypen als unmergebare Binärdaten behandelt werden,
- **Öffnen-Aktion** — Hauptdatei öffnen oder Ordner öffnen,
- optionale **Startaufgaben** und interne **Default-Kanten**.

Ein Baustein wird **einmal** definiert und über viele Produkte hinweg wiederverwendet.

## Bibliothek

Die **Bibliothek** ist der gemeinsame Vorrat an Standard-Werkzeugkästen und einzelnen
Bausteinen. Sie lebt **außerhalb** jedes Produkts und ist eine reine **Vorlagenquelle** —
hier liegt das geteilte Wissen deines Teams über „so behandeln wir KiCad / Fusion / …".

## Werkzeugkasten (Produkt-Stack)

Wenn du einen Werkzeugkasten für ein Produkt einrichtest, **kopiert** das Werkzeug die
gewählten Bausteine aus der Bibliothek in das Produkt hinein (in dessen `_plm`-Verwaltungs­
ordner). Das ist die wichtige **Anti-Drift-Regel**:

!!! warning "Kopie, keine Live-Abhängigkeit"
    Der Werkzeugkasten eines Produkts ist eine **eigenständige Kopie**. Eine spätere Änderung
    in der Bibliothek verändert **niemals** ein bereits laufendes Produkt. Was du beim Anlegen
    gewählt hast, bleibt stabil — bis du es im Produkt ausdrücklich erweiterst.

In der Werkbank siehst du den Werkzeugkasten als ruhige Leiste über den Artefakt-Karten:

![Die Werkzeugkasten-Leiste: gewählter Standard plus Anzahl Bausteine, mit „erweitern"](../img/werkzeugkasten-leiste.png)

Hat ein Produkt noch keinen Werkzeugkasten, steht hier stattdessen eine Aufforderung
**„Werkzeugkasten einrichten"**. Den Ablauf zeigt die Anleitung
[Erstes Produkt](../erste-schritte/erstes-produkt.md).

## Heimat-Ordner & Waisen

Der **Heimat-Ordner** ist der Arbeitsbereich, den ein Baustein in einem konkreten Produkt
regiert (KiCad → `elektronik/`). Dateien, die zu den Mustern eines Bausteins passen, werden
automatisch zu Artefakt-Karten zusammengefasst.

Eine Datei, die zu **keinem** Muster passt, geht nicht verloren — sie landet im
**Unzugeordnet-Fach** ihres Arbeitsbereichs (siehe [Werkbank](werkbank-graph.md)). Solche
Dateien heißen **Waisen**: Es fehlt nur das Etikett, nicht die Datei.

## Stilllegen & Sediment

Ein Werkzeug wechseln (z. B. von einem CAD-Programm zu einem anderen) soll möglichst
reversibel sein. Darum wird ein Baustein nicht „gelöscht", sondern **stillgelegt** — nur ein
Etikett:

- Seine Muster greifen nicht mehr (zugehörige Dateien werden wieder zu Waisen).
- Seine Ignore-/LFS-Zeilen bleiben als inertes **Sediment** in den Konfig-Dateien liegen.
- **Nichts** auf der Platte wird verschoben oder gelöscht.

So ist ein Werkzeugwechsel beinahe vollständig umkehrbar.

!!! info "In Arbeit"
    Das geführte Anlegen aus der Bibliothek, das Erweitern und das Stilllegen werden gerade
    ausgebaut und verfeinert. Die hier beschriebenen Prinzipien sind stabil; einzelne
    Dialoge können sich noch ändern.
