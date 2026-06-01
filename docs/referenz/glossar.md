# Glossar

Die Begriffe des Werkzeugs auf einen Blick. Die ausführlichen Erklärungen stehen in den
[Konzept-Kapiteln](../konzepte/ueberblick.md).

### Produkt
Oberste Einheit, in der Regel ein verkaufsfähiges Endprodukt. Entspricht **einem Ordner** auf
der Platte; die Cloud ist nur Remote, nie Dateiablage.

### Arbeitsbereich
Ein echter Blatt-Ordner innerhalb eines Produkts (`elektronik/`, `mechanik/`, `firmware/`).
Das Werkzeug erfindet keine zweite Struktur neben dem Dateisystem.

### Artefakt
Logisch verwalteter Bestandteil innerhalb eines Arbeitsbereichs (Schaltplan, PCB-Layout,
BOM …). Kann eine Datei, mehrere Dateien oder einen Ordnerbestandteil umfassen.

### Datei
Konkretes Element im Dateisystem. Gehört zu einem Artefakt, behält aber ihren echten Pfad.

### Baustein
Wiederverwendbares Bündel an Werkzeug-Wissen (typischerweise eines pro Werkzeug: KiCad,
Fusion, Zephyr): Heimat-Ordner, Artefakt-Muster, Ignore-Presets, LFS-/Sperr-Muster,
Öffnen-Aktion, optionale Startaufgaben und interne Default-Kanten.

### Bibliothek
Geteilter Vorrat an Standard-Werkzeugkästen und einzelnen Bausteinen, **außerhalb** jedes
Produkts. Reine Vorlagenquelle.

### Werkzeugkasten (Produkt-Stack)
Die Bausteine, die in einem Produkt aktiv sind — als **eigenständige Kopie** der Bibliothek
(Anti-Drift). Eine Bibliotheks-Änderung berührt ein laufendes Produkt nie.

### Heimat-Ordner
Der Arbeitsbereich, den ein Baustein in einem Produkt regiert (KiCad → `elektronik/`).

### Sediment
Wird ein Baustein stillgelegt (nur Etikett), bleiben seine Ignore-/LFS-Zeilen als inertes
„Sediment" in den Konfig-Dateien liegen. Nichts wird verschoben oder gelöscht — ein
Werkzeugwechsel wird so beinahe vollständig umkehrbar.

### Waise
Eine versionierte Datei, die zu keinem Artefakt-Muster passt. Sie liegt im Unzugeordnet-Fach
ihres Arbeitsbereichs — nur das Etikett fehlt, nicht die Datei.

### Werkbank
Die Vorderseite eines Produkts: der aktuelle Arbeitszustand als Artefakt-Karten je
Arbeitsbereich. Hier verbringst du den Alltag.

### Graph-Raum / Versionsbaum
Die Historie als eigener Raum, den du aufsuchst: Commits, Revisionen, Varianten. Reine
Orientierung — die beste Übersicht, nicht die beste Werkbank.

### Artefakt-Karte
Die Darstellung eines Artefakts in der Werkbank: Status-LED, Bausteinname, Hauptdatei, echter
Pfad und Ein-Klick-Aktion.

### Stand / Commit
Ein im Hintergrund **still** angelegter Sicherungspunkt deiner Arbeit (technisch ein Commit).
Du schreibst dafür keine Commit-Nachricht. Erscheint in der **Commits**-Schiene und im
Versionsbaum.

### Revision
Ein bewusst zur benannten Version erhobener Commit (`v0.4`, `Rev B` …), technisch ein **Tag** —
ein benannter Punkt auf einer Linie, **kein** Zweig. Erzeugt eine lesbare `VERSION_NOTES.md`.
Ersetzt den früheren Begriff „Meilenstein".

### Revisions-Art (Prototyp / Freigabe)
**Prototyp** = laxer, bearbeitbarer Zwischenstand (Standard). **Freigabe** = abgeschlossener,
**schreibgeschützter** Stand; bewusst und umkehrbar umschaltbar.

### Freigabe
Der Abschluss einer Version (Revisions-Art „Freigabe"). Bringt den Stand zugleich auf den
geteilten Server (er gilt dann als **veröffentlicht**) und gibt die Sperre frei.

### Freigabe-Gate
Der Dialog beim Freigeben: ein **kontextabhängiger Knopf**, der offene Punkte nach Härte
sortiert sammelt — **Taggen** (sauber), **Trotzdem freigeben** (weicher Block, mit
Begründung) oder **gesperrt durch Aufgabe** (harter Block).

### veröffentlicht
Ein Stand liegt auf der **geteilten Linie**. Eine **Ort**-Eigenschaft, unabhängig von der
Freigabe-Art. Pro Knoten im Versionsbaum als Abzeichen sichtbar.

### Aufgabe / Hinweis
Arbeitspunkte eines Produkts. Sie unterscheiden sich **nur** durch ihre Blockier-Fähigkeit:
eine **Aufgabe** kann eine Freigabe blockieren, ein **Hinweis** nie.

### Variante / Zweig
Eine echte zweite Linie (Branch) für Varianten oder Experimente. **Orthogonal** zur Revision.
Eine Version wird eindeutig durch Produkt + Linie + Versionsname identifiziert.

### Manueller Sync (Sichern / Holen)
Der Netz-Abgleich ist Handarbeit: **Sichern** schiebt deine Arbeit ins private Backup,
**Holen** bringt den geteilten Stand herein. Der lokale Auto-Commit bleibt still.

### Knoten-Verben
Die drei Aktionen an einem alten Graph-Knoten: **Als Ordner öffnen** (schreibgeschützte Kopie
daneben), **Von hier abzweigen** (neue Variante), **Zurückwerfen** (destruktiv, hinter der
Gate-Taste).

### Sperre
Koordination für unmergebare Binärdateien: Bearbeiten holt die Sperre, andere sehen
„gesperrt von X seit …". Eine Sperre ist **Koordination**, keine Autorisierung.

### Sicherung / Freigabe (Push-Arten)
**Sicherung** (privat) = Backup deiner Zwischenstände, der **Sichern**-Knopf. **Freigabe**
(öffentlich) = bringt die fertige Binärdatei auf den geteilten Stand **und** gibt die Sperre
frei; gebunden an den Freigabe-Toggle einer Revision.

### Binär-Invariante
Die tragende Sicherheitsregel: *Eine gesperrte Binäränderung darf den geteilten Stand nicht
erreichen, solange die Sperre gehalten wird.* Macht gefährliche Merges strukturell unmöglich.

### Laute Ausnahme
Der einzige Moment, in dem das Werkzeug die Stimme hebt: ein echter, nicht auflösbarer
Widerspruch beim Holen oder Veröffentlichen. Es fragt „welcher Stand gilt?".

### Konto
Die **eine** app-weite Server-Identität (Adresse + Zugangsdaten), gültig für alle Produkte.
Über das Zahnrad erreichbar; nur zum Teilen nötig, nicht fürs lokale Arbeiten.

### Einrichtungs-Zeremonie
Der einmalige Schritt, ein Produkt zu teilen (Server anbinden, veröffentlichen, einladen).
Hier darf die Sprache git-näher sein.

### Produkt-Registry / Produktliste
Schlanke Liste „welches Produkt liegt wo" — **nur Pfade, keine Inhalte**. Versorgt die
**Produktliste** (Produktwechsel) und die produktübergreifende Suche.

### Git-ehrlich
Die Haltung zum Motor unter der Haube: Basis-Git-Begriffe (Commit, Branch, Tag, Push, Pull,
Merge) dürfen sichtbar sein; nur die **gefährliche Mechanik** bleibt versteckt/automatisiert,
Destruktives hinter einer abgesetzten, dunklen Gate-Taste.
