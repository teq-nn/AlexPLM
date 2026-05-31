# Konzept: Schlanke PLM-Software für kleine Teams

## 1. Ziel und Grundidee

Die Software richtet sich an Einzelpersonen, Entwickler, Maker, kleine Unternehmen und kleine Teams, die produktbezogene Entwicklungsdaten strukturiert verwalten möchten.

Ziel ist eine schlanke PLM-ähnliche Lösung, die Produktversionen, Entwicklungsdateien, Workflows und Aufgaben in einer intuitiven Oberfläche zusammenführt.

Der Fokus liegt auf:

- pragmatischer Dateiverwaltung
- nachvollziehbarer Versionierung
- vollständigen Produktständen
- workflowbasierter Vollständigkeitskontrolle
- Aufgabenverwaltung
- Änderungsdokumentation
- einfachem Zugriff auf alte und aktuelle Produktstände

Die Software soll kein schwergewichtiges Enterprise-PLM-System ersetzen. Sie soll bewusst leichter, verständlicher und näher an der realen Arbeitsweise kleiner Entwicklungsumgebungen sein.

Rollen- und Rechteverwaltung ist für die erste Version nicht zwingend erforderlich, soll aber durch ein geeignetes Datenmodell später ergänzt werden können.

---

## 2. Zielgruppe

Die Software ist gedacht für:

- Einzelentwickler
- kleine Entwicklungsteams
- kleine Unternehmen
- Maker und Hardware-Startups
- Produktentwickler mit dateibasierter Arbeitsweise
- Teams, die mit CAD-, Elektronik-, Firmware- und Dokumentationsdateien arbeiten

Nicht primär im Fokus der ersten Version stehen:

- große Konzernstrukturen
- komplexe Rechte- und Freigabeprozesse
- ERP-Integration
- Lagerverwaltung
- Compliance-Management
- digitale Signaturen
- tiefe CAD- oder PLM-Integration

---

## 3. Grundstruktur

Die oberste Einheit der Software ist das Produkt.

Ein Produkt entspricht in der Regel einem verkaufsfähigen Endprodukt oder einem übergeordneten Entwicklungsprojekt.

Ein Produkt kann einfach oder modular aufgebaut sein.

### 3.1 Einfache Produktstruktur

Bei einfachen Produkten enthält das Produkt direkt Arbeitsbereiche und Artefakte.

Beispiel:

```text
Ember Reverb
  Elektronik
    KiCad-Projekt
    Schaltplan
    PCB-Layout
    BOM
    Fertigungsdaten

  Mechanik
    Gehäuse-CAD
    Zeichnung

  Dokumentation
    Manual
    Testprotokoll
```

### 3.2 Modulare Produktstruktur

Bei komplexeren Produkten können Arbeitsbereiche oder Module verwendet werden.

Beispiel:

```text
Ember Reverb
  Main PCB
    Schaltplan
    Layout
    BOM
    Bestückungsdaten

  Front PCB
    Schaltplan
    Layout
    BOM

  Gehäuse
    CAD-Modell
    Zeichnung
    Frontplattenlayout

  Firmware
    Quellcode
    Build-Datei
    Release Notes

  Dokumentation
    Manual
    Testprotokoll
    Produktfotos
```

Die Modulstruktur ist optional. Der Nutzer kann frei entscheiden, ob er ein flaches oder ein modular aufgebautes Produkt anlegt.

---

## 4. Zentrale Begriffe

### Produkt

Ein Produkt ist die zentrale Einheit der Software. Es entspricht meist einem verkaufsfertigen Endprodukt.

### Branch

Ein Branch ist ein Entwicklungszweig innerhalb eines Produkts. Branches können für Experimente, Varianten oder alternative Entwicklungsrichtungen genutzt werden.

### Version

Eine Version ist ein vollständiger Produktstand innerhalb eines Branches.

### Arbeitsbereich

Ein Arbeitsbereich ist ein physischer Ordner innerhalb einer Produktversion. Beispiele:

- Elektronik
- Mechanik
- Firmware
- Dokumentation
- Produktion

### Artefakt

Ein Artefakt ist ein logisch verwalteter Bestandteil innerhalb eines Arbeitsbereichs. Beispiele:

- Schaltplan
- PCB-Layout
- BOM
- Gerberdaten
- Pick-and-Place
- Gehäuse-CAD
- Testprotokoll
- Manual

Ein Artefakt ist nicht zwingend genau eine Datei. Es kann eine Datei, mehrere Dateien oder einen Ordnerbestandteil umfassen.

### Datei

Eine Datei ist ein konkretes Element im Dateisystem. Dateien können einem Artefakt zugeordnet sein.

### Workflow

Ein Workflow ist eine einfache Startvorlage. Er definiert Arbeitsbereiche, Artefakte und optionale Startaufgaben.

### Aufgabe

Eine Aufgabe ist ein Arbeitspunkt. Sie kann frei angelegt oder aus einem Workflow erzeugt werden.

---

## 5. Versionierungsmodell

In der ersten Ausbaustufe arbeitet die Software mit globalen Produktversionen.

Jede relevante Änderung erzeugt eine neue Version des gesamten Produkts. Auch wenn sich nur ein einzelner Bestandteil ändert, beispielsweise der Schaltplan oder die Firmware, wird eine neue Produktversion angelegt.

Eine Produktversion beschreibt immer einen vollständigen Stand des Produkts inklusive aller Arbeitsbereiche, Artefakte und Dateien.

Nicht geänderte Dateien werden aus der vorherigen Version übernommen oder als Kopie in der neuen Version abgelegt. Dadurch bleibt jede Produktversion unabhängig nachvollziehbar und vollständig reproduzierbar.

Optionale Module besitzen in der ersten Version der Software keine vollständig eigene Versionierung, sondern sind Bestandteil der globalen Produktversion. Eine spätere Erweiterung um unabhängige Modulversionen soll möglich bleiben.

---

## 6. Branches und Versionsnummern

Branches besitzen eigene Namen. Versionen werden innerhalb eines Branches verwaltet.

Eine Version wird eindeutig durch die Kombination aus Produkt, Branch und Versionsnummer identifiziert.

Beispiele:

```text
Ember Reverb / main / v0.4
Ember Reverb / alternate-enclosure / v0.4
```

Da dieselbe Versionsnummer auf mehreren Branches existieren kann, muss die Benutzeroberfläche Branch und Version immer gemeinsam anzeigen.

### 6.1 Standard-Branch

Der Standard-Branch ist der Branch, von dem aus neue Versionen standardmäßig erstellt werden.

Ein abgeschlossener Branch kann:

- als Variante behalten werden
- archiviert werden
- als neuer Standard-Branch übernommen werden

Beim Abschluss eines Branches fragt die Software:

```text
Was soll mit diesem Branch passieren?

[Als Variante behalten]
[Als neuen Standard übernehmen]
[Archivieren]
```

### 6.2 Versionsnummern

Die Software schlägt Versionsnummern automatisch vor, erzwingt aber kein festes Schema.

Mögliche Benennungen:

```text
v0.1
v0.2
v1.0
Rev A
Rev B
Prototype 1
Serie 2026-01
```

Der Nutzer kann Versionsnamen frei ändern.

---

## 7. Speicher- und Dateilogik

Jede Produktversion wird als eigener vollständiger Ordner im Dateisystem gespeichert.

Die Software arbeitet also nicht nur mit internen Verweisen, sondern erzeugt pro Version einen physischen vollständigen Dateistand.

Beispiel:

```text
/products/
  ember-reverb/
    branches/
      main/
        versions/
          v0.1/
          v0.2/
          v0.3/

      alternate-enclosure/
        versions/
          v0.1/
          v0.2/
```

Dadurch bleibt jede Version auch außerhalb der Software nachvollziehbar und sicherbar.

### 7.1 Erstellen neuer Versionen

Beim Erstellen einer neuen Version wird zunächst die ausgewählte Basisversion vollständig kopiert.

Danach kann der Nutzer festlegen, welche Arbeitsbereiche oder Artefakte geändert werden sollen.

Beispiel:

```text
Basisversion: main / v0.3
Neue Version: main / v0.4

Geändert:
  Schaltplan
  BOM

Übernommen:
  PCB
  Gehäuse
  Firmware
  Dokumentation
```

Auch übernommene Dateien liegen physisch im neuen Versionsordner.

---

## 8. Speicherort beim Anlegen eines Produkts

Beim Anlegen eines neuen Produkts legt der Nutzer den Speicherort des Produktordners fest.

Der Speicherort kann sein:

- lokaler Ordner
- Netzlaufwerk
- Serverpfad
- synchronisierter Cloud-Ordner

Die Software erzeugt am gewählten Speicherort die vollständige Produktstruktur.

Beispiel:

```text
Neues Produkt anlegen

Produktname: Ember Reverb
Speicherort: /Engineering/Products/
Produktordner: /Engineering/Products/ember-reverb/

Workflow: Geräteentwicklung
Start-Branch: main
Start-Version: v0.1
```

Die Software unterscheidet zwischen:

```text
Basis-Speicherort
= übergeordneter Ordner, in dem Produkte liegen

Produktordner
= konkreter Ordner dieses Produkts

Versionsordner
= konkreter Ordner einer Version
```

Wenn ein Speicherort nicht erreichbar ist, zeigt die Software einen eindeutigen Hinweis.

Beispiel:

```text
Der Speicherort dieses Produkts ist aktuell nicht erreichbar:
/Engineering/Products/ember-reverb/

Bitte Laufwerk verbinden oder Speicherort ändern.
```

Es soll eine Funktion geben:

```text
Projektordner neu verknüpfen
```

Damit kann ein Produkt wieder mit der Software verbunden werden, wenn es verschoben wurde.

---

## 9. Gemeinsame Workspace-Konfiguration

Die Software unterstützt eine gemeinsame Workspace-Konfigurationsdatei.

Diese Datei kann von mehreren Nutzern geladen werden, damit alle mit denselben Projekten, Speicherorten, Workflows und Standards arbeiten.

Beispiel:

```text
plm-workspace.json
```

Die Workspace-Konfiguration enthält keine eigentlichen Produktdateien, sondern verweist auf bestehende Produktordner und gemeinsame Vorlagen.

Typische Inhalte:

- Workspace-Name
- gemeinsame Speicherorte
- eingebundene Produktordner
- Workflow-Vorlagen
- Standard-Tags
- globale Artefakt-Vorlagen
- Standardwerte für neue Produkte

Zusätzlich besitzt jeder Nutzer eine lokale Benutzerkonfiguration. Diese speichert persönliche Einstellungen wie zuletzt geöffnete Produkte, UI-Präferenzen oder lokale Ansichtsoptionen.

Gemeinsame und lokale Konfiguration werden getrennt.

### 9.1 Produkt zur Workspace-Konfiguration hinzufügen

Beim Anlegen eines Produkts kann der Nutzer per Haken entscheiden:

```text
[x] Produkt zur gemeinsamen Workspace-Konfiguration hinzufügen
```

Andere Nutzer sehen das Produkt nach dem Aktualisieren ihres Workspaces.

---

## 10. Produktweite Metadaten

Jedes Produkt besitzt einen Verwaltungsordner, zum Beispiel:

```text
_plm/
```

Darin speichert die Software produktweite Metadaten.

Beispiel:

```text
/products/
  ember-reverb/
    _plm/
      product.json
      workflow.json
      branches.json
      tasks.json
      settings.json

    branches/
      main/
        versions/
          v0.1/
            VERSION_NOTES.md
            version.json
            elektronik/
            mechanik/
            dokumentation/
```

Mögliche Dateien:

- `product.json` für Produktname, Beschreibung, Artikelnummer, Status und Standard-Branch
- `workflow.json` für die produktbezogene Workflow-Struktur
- `branches.json` für Versionsbaum, Branches, Varianten und Standardstand
- `tasks.json` für produktweite und versionsbezogene Aufgaben
- `settings.json` für produktspezifische Einstellungen

---

## 11. Arbeitsbereiche, Artefakte und Dateien

Kategorien werden nicht zwingend als eigene Ordner im Dateisystem abgebildet.

Stattdessen unterscheidet die Software zwischen physischen Arbeitsbereichen und logisch verwalteten Artefakten.

Ein Arbeitsbereich ist ein Ordner innerhalb einer Produktversion.

Ein Artefakt ist ein logisch verwalteter Bestandteil innerhalb eines Arbeitsbereichs.

Mehrere Artefakte können ihre Dateien im selben Arbeitsbereich ablegen.

Das ist wichtig für Programme wie KiCad, bei denen Projektdatei, Schaltplan und Platinenlayout sinnvollerweise im gleichen Ordner liegen.

Beispiel:

```text
Ember Reverb
  main
    v0.3
      elektronik/
        ember.kicad_pro
        ember.kicad_sch
        ember.kicad_pcb
        fp-lib-table
        sym-lib-table
        gerber_export.zip
```

In der Software werden diese Dateien logisch getrennt dargestellt:

```text
Elektronik
  KiCad-Projekt: ember.kicad_pro
  Schaltplan: ember.kicad_sch
  PCB-Layout: ember.kicad_pcb
  Fertigungsdaten: gerber_export.zip
```

---

## 12. Hauptdateien, Zusatzdateien und Artefakt-Aktionen

Ein Artefakt kann aus einer einzelnen Datei, mehreren Dateien oder einem ganzen Ordnerbestandteil bestehen.

Die Software unterscheidet zwischen:

### Hauptdatei

Die Datei, die der Nutzer normalerweise direkt bearbeiten oder öffnen möchte.

### Zusatzdateien

Dateien, die zum Artefakt gehören, aber nicht die primäre Arbeitsdatei sind.

### Primäre Aktion

Die wichtigste Aktion auf der Artefakt-Karte.

Meistens ist das:

```text
Hauptdatei öffnen
```

Manchmal ist es aber:

```text
Ordner öffnen
Dateigruppe anzeigen
Exportpaket öffnen
```

Beispiele:

```text
Artefakt: Gehäuse-CAD
Hauptdatei: enclosure.f3d
Zusatzdateien:
- enclosure.step
- enclosure.stl
- front_panel.dxf
Primäre Aktion: Hauptdatei öffnen
```

```text
Artefakt: Firmware
Hauptdatei: keine
Zusatzdateien:
- src/
- include/
- platformio.ini
- README.md
Primäre Aktion: Ordner öffnen
```

```text
Artefakt: Fertigungsdaten
Hauptdatei: production_files.zip
Zusatzdateien:
- gerber.zip
- drill.zip
- pick_and_place.csv
- assembly_drawing.pdf
Primäre Aktion: Exportpaket öffnen
```

---

## 13. Dateien öffnen

Dateien können direkt aus der Software heraus geöffnet werden.

Beim Klick auf eine Datei oder Hauptdatei übergibt die Software den Dateipfad an das Betriebssystem.

Die Software verwendet in der ersten Ausbaustufe keine eigene Programmzuordnung.

Das Betriebssystem öffnet die Datei mit dem jeweils als Standard hinterlegten Programm.

Beispiele:

- `.kicad_pro` mit KiCad
- `.step` mit CAD-Viewer oder CAD-Programm
- `.xlsx` mit Excel oder LibreOffice
- `.pdf` mit PDF-Viewer
- `.md` mit Texteditor

---

## 14. Dateien hinzufügen, ersetzen und erkennen

Dateien können auf mehreren Wegen eingebunden werden:

- Drag-and-drop auf ein Artefakt
- Dateidialog
- direkte Erzeugung oder Bearbeitung im Arbeitsbereich
- Öffnen des Arbeitsbereichs im Dateiexplorer

Die Software prüft Arbeitsbereiche auf Dateiänderungen.

Erkannt werden können:

- neue Dateien
- geänderte Dateien
- gelöschte Dateien
- nicht mehr auffindbare Dateien

Bereits zugeordnete Dateien können automatisch ihrem Artefakt zugeordnet werden.

Neue Dateien werden zunächst als nicht zugeordnet angezeigt und können vom Nutzer:

- einem bestehenden Artefakt zugeordnet werden
- als neues Artefakt angelegt werden
- ignoriert werden

Beispiel:

```text
Neue Datei erkannt: gerber_export.zip

Zuordnen zu:
( ) Gerberdaten
( ) Fertigungsdaten
( ) Neues Artefakt erstellen
( ) Ignorieren
```

### 14.1 Umgang mit gelöschten oder umbenannten Dateien

Wenn eine zugeordnete Datei nicht mehr gefunden wird, markiert die Software das Artefakt nicht sofort endgültig als fehlend.

Stattdessen zeigt sie einen Prüfhinweis.

Mögliche Aktionen:

- Datei neu zuordnen
- als gelöscht markieren
- aus vorheriger Version wiederherstellen
- ignorieren

---

## 15. Änderungsnotizen

Die Software fragt Änderungsnotizen nicht automatisch für jede erkannte Dateiänderung ab.

Stattdessen unterscheidet sie zwischen:

- aktiv aus der Software geöffneten Dateien
- allgemein erkannten Änderungen im Arbeitsbereich

Wenn ein Nutzer eine Datei aus der Software heraus öffnet, merkt sich die Software diese Datei als aktiv bearbeitet oder geprüft.

Beim Zurückkehren in die Software kann abgefragt werden:

```text
Wurde diese Datei geändert?

[Nein, nur angesehen]
[Ja, geändert]
```

Wenn ja, fragt die Software:

```text
Was wurde geändert?
Warum wurde es geändert?
Sind weitere Artefakte betroffen?
```

Dateien, die im Arbeitsbereich automatisch neu entstehen oder sich im Hintergrund ändern, werden zunächst nur gesammelt.

Beim Abschließen oder Freigeben einer Version zeigt die Software eine Änderungsübersicht.

Dort kann der Nutzer:

- fehlende Änderungsbeschreibungen ergänzen
- Dateien Artefakten zuordnen
- Dateien als erzeugte Nebendateien markieren
- irrelevante Änderungen ignorieren

---

## 16. Versionsnotizen und Metadaten

Jede Version erhält automatisch eine menschenlesbare Datei:

```text
VERSION_NOTES.md
```

Diese Datei dokumentiert:

- Produktname
- Branch
- Version
- Basisversion
- Datum
- Status
- Zusammenfassung
- geänderte Artefakte
- übernommene Artefakte
- offene Punkte
- Freigabeinformationen

Zusätzlich erhält jede Version eine maschinenlesbare Datei:

```text
version.json
```

Diese enthält strukturierte Informationen zur Version.

Beispiel:

```json
{
  "product": "Ember Reverb",
  "branch": "main",
  "version": "v0.4",
  "base": {
    "branch": "main",
    "version": "v0.3"
  },
  "status": "in_progress",
  "created_at": "2026-05-27"
}
```

Die Software nutzt `version.json` für Darstellung, Suche, Statusberechnung und Versionslogik.

`VERSION_NOTES.md` dient als transparente Dokumentation im Dateisystem.

---

## 17. Versionen bearbeiten und schützen

Produktversionen besitzen einen Status.

Versionen im Status:

```text
Entwurf
In Arbeit
```

dürfen direkt bearbeitet werden.

Versionen im Status:

```text
Freigegeben
Archiviert
```

sind schreibgeschützt.

Wenn ein Nutzer eine freigegebene oder archivierte Version bearbeiten möchte, muss daraus eine neue Produktversion oder ein Branch erzeugt werden.

Dadurch werden abgeschlossene Produktstände vor versehentlichen Änderungen geschützt.

---

## 18. Statuslogik

Die Software unterscheidet zwischen technischer Vollständigkeit und manueller Freigabe.

### 18.1 Versionsstatus

Mögliche Statuswerte für Versionen:

```text
Entwurf
In Arbeit
Technisch vollständig
Freigegeben
Archiviert
```

Eine Version gilt automatisch als technisch vollständig, wenn:

- alle Pflicht-Artefakte vorhanden sind
- alle erforderlichen Dateien bestätigt wurden
- keine relevanten Artefaktprüfungen offen sind
- keine blockierenden Aufgaben offen sind

Die Freigabe erfolgt immer manuell.

### 18.2 Artefaktstatus

Mögliche Statuswerte für Artefakte:

```text
Fehlt
Vorhanden
Übernommen
Geändert
Prüfung erforderlich
Aktualisierung erforderlich
Datei-Bestätigung erforderlich
Nicht zugeordnet
Ignoriert
Optional / nicht benötigt
```

---

## 19. Workflows

Workflows dienen als einfache Projektstart-Vorlagen.

Sie definieren:

- Arbeitsbereiche
- Artefakte
- Pflicht- oder optional-Status
- optionale Startaufgaben

Workflows sind keine starren Prozessketten und erzwingen keine Bearbeitungsreihenfolge.

Der Nutzer kann Aufgaben dezentral und parallel abarbeiten.

Entscheidend ist, dass am Ende einer Version alle erforderlichen Artefakte vorhanden und relevante Prüfungen abgeschlossen sind.

### 19.1 Workflow als anpassbare Startvorlage

Beim Anlegen eines Produkts kann der Nutzer einen Workflow auswählen.

Der Workflow erzeugt eine vorgeschlagene Struktur aus Arbeitsbereichen und Artefakten.

Diese Struktur kann direkt übernommen oder vor dem Erstellen angepasst werden.

Beispiel:

```text
Workflow: Geräteentwicklung

[x] Elektronik
    [x] KiCad-Projekt
    [x] Schaltplan
    [x] PCB-Layout
    [x] BOM
    [x] Fertigungsdaten

[x] Mechanik
    [x] CAD-Modell
    [x] Zeichnung
    [x] Frontplatte

[x] Dokumentation
    [x] Testprotokoll
    [x] Manual
```

### 19.2 Workflow-Pflege aus dem Projekt heraus

Wenn während eines Projekts neue Arbeitsbereiche oder Artefakte ergänzt werden, fragt die Software, ob diese dauerhaft in die Vorlage übernommen werden sollen.

Beispiel:

```text
Artefakt hinzufügen: Messprotokoll

[x] In aktueller Version hinzufügen
[x] Für zukünftige Versionen dieses Produkts übernehmen
[ ] Zur Workflow-Vorlage hinzufügen
```

Dadurch können Workflows organisch aus realen Projekten wachsen.

---

## 20. Aufgabenverwaltung

Die Software enthält eine einfache Aufgabenverwaltung.

Aufgaben können entstehen aus:

- Workflow-Vorlage
- manueller Eingabe
- Artefaktänderung
- erkannter Dateiänderung
- Freigabeprüfung

Eine Aufgabe kann optional verknüpft werden mit:

- Produkt
- Version
- Arbeitsbereich
- Artefakt
- Datei

Sie muss aber nicht zwingend mit einem Artefakt verbunden sein.

### 20.1 Aufgabenfelder

Eine Aufgabe enthält mindestens:

- Titel
- Beschreibung
- Status
- Fälligkeit
- Priorität
- optionale Verknüpfungen
- optional zuständige Person
- Kommentare

### 20.2 Aufgabenstatus

Mögliche Statuswerte:

```text
Offen
In Arbeit
Wartet
Erledigt
Verworfen
```

### 20.3 Aufgabenansichten

Die Aufgaben können dargestellt werden als:

- einfache Liste
- Checkliste
- optionales Kanban-Board

Standardmäßig sollte eine einfache Liste oder Checkliste verwendet werden. Das Kanban-Board kann als alternative Ansicht angeboten werden.

---

## 21. Aufgabenabschluss und Artefaktprüfung

Wenn eine Aufgabe mit einem Artefakt verknüpft ist, löst das Markieren der Aufgabe als erledigt eine Artefakt-Abschlussprüfung aus.

Die Software fordert den Nutzer auf:

- aktuelle Datei hochladen
- vorhandene Datei bestätigen
- Datei aus Arbeitsbereich verknüpfen
- Prüfung auf später verschieben

Beispiel:

```text
Aufgabe „BOM exportieren“ als erledigt markieren?

Diese Aufgabe ist mit dem Artefakt „BOM“ verbunden.

[Vorhandene Datei bestätigen]
[Neue Datei hochladen/verknüpfen]
[Im Arbeitsbereich suchen]
[Später erledigen]
```

Wenn der Nutzer die Prüfung verschiebt, bleibt die Aufgabe erledigt, aber das Artefakt erhält einen Prüfstatus.

Beispiel:

```text
Aufgabe: BOM exportieren
Status: Erledigt

Artefakt: BOM
Status: Datei-Bestätigung erforderlich
```

Eine Version kann erst vollständig abgeschlossen oder freigegeben werden, wenn alle erforderlichen Artefakte vorhanden oder bestätigt sind.

---

## 22. Abschluss und Freigabe einer Version

Der Abschluss einer Version erfolgt über einen geführten Freigabedialog.

Die Software prüft:

- ob alle Pflicht-Artefakte vorhanden sind
- ob alle Artefaktprüfungen abgeschlossen sind
- ob relevante Workflow-Aufgaben erledigt sind
- ob erkannte Dateiänderungen geprüft oder zugeordnet wurden
- ob Änderungsnotizen vorhanden sind
- ob `VERSION_NOTES.md` erzeugt oder aktualisiert wurde

Wenn alle Anforderungen erfüllt sind, kann die Version freigegeben werden.

Nach der Freigabe wird die Version schreibgeschützt.

### 22.1 Freigabe trotz offener Punkte

Sind noch offene Punkte vorhanden, zeigt die Software diese gesammelt an.

Der Nutzer kann zur Bearbeitung zurückkehren oder die Version bewusst trotzdem freigeben.

Eine Freigabe trotz offener Punkte muss begründet werden und wird in den Versionsnotizen dokumentiert.

Beispiel:

```text
Freigabe trotz offener Punkte

Bitte begründen:
[Prototypenstand für internen Test. Testprotokoll folgt später.]

[Freigabe bestätigen]
```

---

## 23. Benutzeroberfläche

Die Oberfläche soll grafisch intuitiv sein.

Ziel ist keine klassische reine Dateimanager-Ansicht, sondern ein Dashboard mit klaren Karten, Statushinweisen und Versionsbezug.

### 23.1 Hauptbereiche

Vorgeschlagene Hauptnavigation:

```text
Produkte
Workflows
Aufgaben
Vorlagen
Einstellungen
```

Innerhalb eines Produkts:

```text
Übersicht
Dateien / Artefakte
Aufgaben
Änderungen
Versionsbaum
Einstellungen
```

### 23.2 Produkt-Dashboard

Das Produkt-Dashboard enthält:

```text
Oben:
Versionsleiste

Links:
Arbeitsbereiche / Navigation

Mitte:
Artefakt-Karten des ausgewählten Arbeitsbereichs

Rechts:
Aufgaben, Hinweise, erkannte Änderungen

Tabs:
Übersicht | Dateien | Aufgaben | Änderungen | Versionsbaum | Einstellungen
```

---

## 24. Versionsleiste und Versionsbaum

Die Produktansicht enthält dauerhaft eine kompakte Versionsleiste.

Diese zeigt:

- Produktname
- aktiver Branch
- aktive Version
- Versionsstatus
- ob es der neueste Standardstand ist
- ob es eine ältere Version ist
- ob es ein Branch oder eine Variante ist
- ob die Version bearbeitbar oder schreibgeschützt ist

Beispiele:

```text
Ember Reverb | Branch: main | Version: v0.4 | Status: In Arbeit | Neuester Standardstand
```

```text
Ember Reverb | Branch: enclosure-variant | Version: v0.2 | Status: In Arbeit | Branch, nicht Standard
```

```text
Ember Reverb | Branch: main | Version: v0.2 | Status: Freigegeben | Ältere Version, schreibgeschützt
```

Der vollständige Versionsbaum wird als eigener Tab oder aufklappbarer Bereich dargestellt.

Dort sieht der Nutzer:

- alle Branches
- alle Versionen
- Varianten
- freigegebene Stände
- archivierte Versionen
- Standard-Branch

---

## 25. Artefakt-Karten

Artefakt-Karten sind die zentrale Darstellung für logisch verwaltete Bestandteile eines Produkts.

Eine Artefakt-Karte enthält mindestens:

- Name des Artefakts
- Status
- Hauptdatei
- Pflichtkennzeichnung
- wichtigste Aktion
- Hinweis auf Änderungsnotizen
- Hinweis auf Prüfung oder fehlende Datei

Beispiel:

```text
[Schaltplan]
Status: Geändert
Pflicht: Ja
Hauptdatei: ember.kicad_sch
Änderungsnotiz: vorhanden

Aktionen:
[Öffnen] [Datei ersetzen] [Details] [Als geprüft markieren]
```

Für ein fehlendes Artefakt:

```text
[Testprotokoll]
Status: Fehlt
Pflicht: Ja
Erwartete Datei: PDF, MD oder DOCX

Aktionen:
[Datei hinzufügen] [Als nicht benötigt markieren] [Aufgabe anzeigen]
```

Erweiterte Informationen werden in einem Detailbereich angezeigt.

---

## 26. Suche, Filter und Tags

Die Software bietet eine Suche über Metadaten und Dateinamen.

Durchsuchbar sind:

- Produktnamen
- Beschreibungen
- Branches
- Versionsnamen
- Arbeitsbereiche
- Artefakte
- Dateinamen
- Aufgaben
- Statuswerte
- Änderungsnotizen
- Änderungsgründe
- Tags

In der ersten Ausbaustufe wird keine Volltextsuche in Dateiinhalten umgesetzt.

### 26.1 Filter

Mögliche Filter:

```text
Status: Fehlend
Status: Prüfung erforderlich
Status: Geändert
Status: Freigegeben
Status: Archiviert
Aufgaben: Offen
Aufgaben: Überfällig
Branch: main
Branch: Variante
Version: Neuester Standardstand
Artefakt-Typ: BOM
Arbeitsbereich: Elektronik
```

### 26.2 Tags

Tags können manuell vergeben oder durch Workflows vorgeschlagen werden.

Tags können verwendet werden für:

- Produkte
- Branches
- Versionen
- Arbeitsbereiche
- Artefakte
- Aufgaben

Beispiele:

```text
Prototyp
Serienstand
Kundenvariante
JLCPCB
CE-relevant
Firmware benötigt
Audio
OEM
Fertigung
```

---

## 27. Fachliches Datenmodell

Die Software basiert auf folgenden Kernobjekten:

```text
Workspace
Produkt
Branch
Version
Arbeitsbereich
Artefakt
Datei
Aufgabe
Workflow-Vorlage
Änderungsnotiz
Freigabe
Tag
```

### 27.1 Workspace

Ein Workspace beschreibt eine gemeinsame Arbeitsumgebung.

Er enthält:

- Produktverweise
- gemeinsame Speicherorte
- Workflow-Vorlagen
- Standard-Tags
- teamweite Einstellungen

### 27.2 Produkt

Ein Produkt ist die zentrale Einheit.

Es enthält:

- Produktname
- Beschreibung
- Speicherort
- Standard-Branch
- Workflow-Struktur
- Branches
- produktweite Aufgaben und Einstellungen

### 27.3 Branch

Ein Branch ist ein Entwicklungszweig oder eine Produktvariante.

Er enthält:

- Name
- Typ
- Status
- Herkunft
- Versionen

### 27.4 Version

Eine Version ist ein vollständiger Produktstand.

Sie enthält:

- Branch
- Versionsname
- Basisversion
- Status
- Arbeitsbereiche
- Artefakte
- Änderungsnotizen
- Freigabeinformationen

### 27.5 Arbeitsbereich

Ein Arbeitsbereich ist ein physischer Ordner innerhalb einer Version.

### 27.6 Artefakt

Ein Artefakt ist ein logisch verwalteter Bestandteil innerhalb eines Arbeitsbereichs.

### 27.7 Datei

Eine Datei ist ein konkretes Element im Dateisystem.

Die Software speichert dazu:

- Pfad
- Rolle
- Status
- Änderungsdatum
- Dateigröße
- optional Hash

### 27.8 Aufgabe

Eine Aufgabe ist ein Arbeitspunkt mit optionaler Verknüpfung zu Produkt, Version, Arbeitsbereich, Artefakt oder Datei.

### 27.9 Workflow-Vorlage

Eine Workflow-Vorlage definiert Startstruktur und Startaufgaben.

### 27.10 Änderungsnotiz

Eine Änderungsnotiz beschreibt, was geändert wurde und warum.

### 27.11 Freigabe

Eine Freigabe dokumentiert den Abschluss einer Version.

### 27.12 Tag

Tags dienen zur Suche, Filterung und Klassifizierung.

---

## 28. Beispielhafte JSON-Strukturen

### 28.1 Workspace-Konfiguration

```json
{
  "workspace_name": "Fieldfare Audio Development",
  "workspace_id": "fieldfare-audio-dev",
  "config_version": "1.0",
  "default_storage_locations": [
    {
      "name": "Produktentwicklung",
      "path": "/Engineering/Products"
    }
  ],
  "products": [
    {
      "name": "Ember Reverb",
      "path": "/Engineering/Products/ember-reverb"
    }
  ],
  "workflow_templates": [
    {
      "name": "Geräteentwicklung",
      "path": "/Engineering/PLM_Config/workflows/geraeteentwicklung.json"
    }
  ],
  "default_tags": [
    "Prototyp",
    "Serienstand",
    "Variante",
    "Fertigung"
  ]
}
```

### 28.2 Produkt-Konfiguration

```json
{
  "product_name": "Ember Reverb",
  "product_slug": "ember-reverb",
  "description": "Reverb-Pedal",
  "product_root_path": "/Engineering/Products/ember-reverb",
  "default_branch": "main",
  "status": "development",
  "created_at": "2026-05-27",
  "workflow_name": "Geräteentwicklung",
  "tags": ["Audio", "Pedal", "Prototyp"]
}
```

### 28.3 Branch-Datei

```json
{
  "default_branch": "main",
  "branches": [
    {
      "name": "main",
      "type": "standard",
      "status": "active",
      "created_from": null,
      "versions": ["v0.1", "v0.2", "v0.3"]
    },
    {
      "name": "alternate-enclosure",
      "type": "variant",
      "status": "active",
      "created_from": {
        "branch": "main",
        "version": "v0.3"
      },
      "versions": ["v0.3", "v0.4"]
    }
  ]
}
```

### 28.4 Version-Datei

```json
{
  "product": "Ember Reverb",
  "branch": "main",
  "version": "v0.4",
  "base": {
    "branch": "main",
    "version": "v0.3"
  },
  "status": "in_progress",
  "created_at": "2026-05-27",
  "workspaces": [
    {
      "name": "Elektronik",
      "path": "elektronik",
      "artifacts": [
        {
          "name": "Schaltplan",
          "required": true,
          "status": "changed",
          "primary_action": "open_main_file",
          "main_file": "elektronik/ember.kicad_sch",
          "files": [
            {
              "path": "elektronik/ember.kicad_sch",
              "role": "main",
              "status": "changed"
            }
          ],
          "change_note": {
            "summary": "Eingangsstufe angepasst",
            "reason": "Rauschverhalten verbessern"
          }
        }
      ]
    }
  ]
}
```

### 28.5 Workflow-Vorlage

```json
{
  "workflow_name": "Geräteentwicklung",
  "description": "Startvorlage für elektronische Geräteentwicklung",
  "workspaces": [
    {
      "name": "Elektronik",
      "artifacts": [
        {
          "name": "KiCad-Projekt",
          "required": true
        },
        {
          "name": "Schaltplan",
          "required": true
        },
        {
          "name": "PCB-Layout",
          "required": true
        },
        {
          "name": "BOM",
          "required": true
        },
        {
          "name": "Fertigungsdaten",
          "required": true
        }
      ]
    },
    {
      "name": "Mechanik",
      "artifacts": [
        {
          "name": "CAD-Modell",
          "required": true
        },
        {
          "name": "Zeichnung",
          "required": false
        }
      ]
    }
  ],
  "start_tasks": [
    {
      "title": "Schaltplan erstellen",
      "linked_artifact": "Elektronik/Schaltplan"
    },
    {
      "title": "PCB-Layout erstellen",
      "linked_artifact": "Elektronik/PCB-Layout"
    },
    {
      "title": "Testprotokoll erstellen",
      "linked_artifact": "Dokumentation/Testprotokoll"
    }
  ]
}
```

---

## 29. MVP-Funktionsumfang

Die erste nutzbare Version soll den Kern der Produktdatenverwaltung abbilden.

Der MVP umfasst:

- Produkte anlegen
- Speicherort pro Produkt wählen
- Produktordnerstruktur automatisch erzeugen
- gemeinsame Workspace-Konfiguration laden
- Workflow-Vorlage auswählen
- Arbeitsbereiche und Artefakte aus Workflow übernehmen
- Arbeitsbereiche und Artefakte manuell ergänzen
- neue Artefakte optional zur Workflow-Vorlage hinzufügen
- globale Produktversionen erstellen
- vollständigen Versionsordner aus Basisversion kopieren
- Branches anlegen
- Standard-Branch markieren
- Versionen nach Freigabe schreibschützen
- Dateien aus der Software öffnen
- Arbeitsbereichsordner im Dateiexplorer öffnen
- Dateien per Drag-and-drop oder Dateidialog zuordnen
- neue/geänderte/gelöschte Dateien im Arbeitsbereich erkennen
- erkannte Dateien Artefakten zuordnen oder ignorieren
- Aufgaben aus Workflow erzeugen
- freie Aufgaben anlegen
- Aufgaben mit Artefakten verknüpfen
- Artefaktprüfung beim Erledigen verknüpfter Aufgaben
- Änderungsnotizen für aktiv geöffnete/geänderte Dateien
- Abschlussdialog für Versionen
- `VERSION_NOTES.md` erzeugen
- `version.json` erzeugen
- produktweite `_plm`-Metadaten erzeugen
- Suche über Produkte, Versionen, Artefakte, Aufgaben und Dateinamen
- Filter nach Status
- Tags

---

## 30. Bewusst nicht im MVP

Nicht Teil des MVP sind:

- komplexe Rollen- und Rechteverwaltung
- mehrstufige Freigabeprozesse
- digitale Signaturen
- unabhängige Modulversionierung
- tiefe CAD-Integration
- KiCad-spezifische Parser
- ERP-Anbindung
- Lagerverwaltung
- Seriennummernverwaltung
- Volltextsuche in PDFs oder Office-Dateien
- automatische BOM-Analyse
- Dateiinhaltsvergleiche
- komplexe Automatisierungsregeln
- Zeiterfassung

---

## 31. Spätere Erweiterungen

Mögliche spätere Erweiterungen:

- Benutzerrollen und Rechte
- Review- und Freigabeprozesse mit mehreren Personen
- unabhängige Modulversionierung
- Produktvarianten mit gemeinsamen Modulen
- automatische Volltextsuche
- PDF-/Office-Volltextsuche
- BOM-Analyse
- Lieferanten- und Einkaufsdaten
- Lagerbestand
- Artikelnummern-Management
- Seriennummern
- ERP-Anbindung
- Git-Integration
- CAD-/KiCad-spezifische Parser
- automatische Vergleichsansichten
- Benachrichtigungen
- Zeiterfassung
- Webzugriff für Teams
- Kommentare mit Erwähnungen
- Audit-Log

---

## 32. Zusammenfassung

Die Software soll eine schlanke, dateisystembasierte PLM-Lösung für kleine Teams und Einzelpersonen werden.

Der Kern besteht aus vollständigen Produktversionen, klarer Dateistruktur, einfacher Workflow-Vorlage, Artefakt-Karten, Aufgabenverwaltung, Änderungsdokumentation und geführter Freigabe.

Die Software soll den Nutzer nicht in eine starre Prozesskette zwingen, sondern sicherstellen, dass am Ende eines Entwicklungsstands alle relevanten Dateien vorhanden, zugeordnet und dokumentiert sind.

Die reale Ordnerstruktur bleibt transparent und nutzbar. Jede Version ist auch außerhalb der Software nachvollziehbar.

Damit entsteht ein Werkzeug zwischen klassischem Dateimanager, Aufgabenverwaltung und leichtgewichtigem PLM-System.
