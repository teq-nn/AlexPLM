# Glossar

Die Begriffe des Werkzeugs auf einen Blick. Die ausführlichen Erklärungen stehen in den
[Konzept-Kapiteln](Konzepte-Ueberblick).

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
Die Historie als eigener Raum, den du aufsuchst: Stände, Meilensteine, Zweige. Reine
Orientierung — die beste Übersicht, nicht die beste Werkbank.

### Artefakt-Karte
Die Darstellung eines Artefakts in der Werkbank: Status-LED, Bausteinname, Hauptdatei, echter
Pfad und Ein-Klick-Aktion.

### Stand
Ein im Hintergrund **still** angelegter Sicherungspunkt deiner Arbeit. Du schreibst dafür
keinen Text. Stände — nicht Commits — siehst du im Versionsbaum.

### Meilenstein
Ein bewusst zur benannten Version erhobener Stand (`v0.4`, `Rev B` …). Erzeugt eine lesbare
`VERSION_NOTES.md`.

### Meilenstein-Art (Prototyp / Freigabe)
**Prototyp** = laxer, bearbeitbarer Zwischenstand (Standard). **Freigabe** = abgeschlossener,
**schreibgeschützter** Stand; bewusst und umkehrbar umschaltbar.

### Freigabe
Der Abschluss einer Version. Erfolgt über einen Dialog mit einem kontextabhängigen Knopf, der
offene Punkte nach Härte sortiert sammelt.

### Aufgabe / Hinweis
Arbeitspunkte eines Produkts. Sie unterscheiden sich **nur** durch ihre Blockier-Fähigkeit:
eine **Aufgabe** kann eine Freigabe blockieren, ein **Hinweis** nie.

### Zweig
Ein bewusster Entwicklungszweig für Varianten oder Experimente. Eine Version wird eindeutig
durch Produkt + Zweig + Versionsname identifiziert.

### Stiller Sync
Der tägliche Abgleich im Hintergrund. Du siehst nur „aktuell / gesichert" — nie
push/pull/merge.

### Sperre
Koordination für unmergebare Binärdateien: Bearbeiten holt die Sperre, andere sehen
„gesperrt von X seit …". Eine Sperre ist **Koordination**, keine Autorisierung.

### Sicherungs-Push / Freigabe-Push
**Sicherungs-Push** (privat) = Backup deiner Zwischenstände. **Freigabe-Push** (öffentlich) =
bringt die fertige Binärdatei auf den geteilten Stand **und** gibt die Sperre frei.

### Binär-Invariante
Die tragende Sicherheitsregel: *Eine gesperrte Binäränderung darf den geteilten Stand nicht
erreichen, solange die Sperre gehalten wird.* Macht gefährliche Merges strukturell unmöglich.

### Laute Ausnahme
Der einzige Moment, in dem das Werkzeug die Stimme hebt: ein echter, nicht auflösbarer
Widerspruch beim Abgleich. Es fragt in eigener Sprache „welcher Stand gilt?", nie mit
Git-Konfliktmarkern.

### Einrichtungs-Zeremonie
Der einmalige Schritt, ein Produkt zu teilen (Server anbinden, veröffentlichen, einladen).
Hier darf die Sprache git-näher sein.

### Produkt-Registry
Schlanke Liste „welches Produkt liegt wo" — **nur Pfade, keine Inhalte**. Versorgt die
Produktliste und die produktübergreifende Suche.

### Git-ehrlich
Die Haltung zum Motor unter der Haube: stiller Alltag ohne Git-Vokabular, seltene Zeremonie
git-nah erlaubt, gefährliche Mechanik hinter einer abgesetzten, dunklen Gate-Taste.
