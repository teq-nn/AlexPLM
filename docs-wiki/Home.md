# PLM-Werkzeug

Ein schlankes, dateibasiertes **Produktdaten-Management** für Einzelentwickler und
kleine Hardware-, Elektronik- und Firmware-Teams.

Das Werkzeug bringt Produktversionen, Entwicklungsdateien und Aufgaben in einer ruhigen,
übersichtlichen Oberfläche zusammen — ohne ein schwergewichtiges Enterprise-PLM zu sein und
ohne dich zu zwingen, in Git zu denken.

![Die Werkbank eines geöffneten Produkts](img/werkbank-uebersicht.png)

## Die Grundidee in drei Sätzen

1. **Deine echten Ordner bleiben die Wahrheit.** Das Werkzeug sitzt *neben* deinen
   Projektordnern auf der Platte, nicht davor. Es erfindet keine zweite Struktur, jede
   Version bleibt auch außerhalb der Software nachvollziehbar.
2. **Versionierung ohne Git-Vokabular.** Speichern erzeugt im Hintergrund still einen
   *Stand*. Du siehst Stände und *Meilensteine* im Versionsbaum — nie Commits, Branches
   oder Merges, solange du es nicht ausdrücklich willst.
3. **Zusammenarbeit ohne Datenverlust.** Im Team koordiniert das Werkzeug binäre Dateien
   (CAD, Gehäuse, Fotos) über *Sperren* und gleicht den Stand still im Hintergrund ab. Nur
   bei einem echten Widerspruch hebt es die Stimme.

## Für wen ist das gedacht?

- Einzelentwickler, Maker und Hardware-Startups
- kleine Entwicklungsteams mit CAD-, Elektronik-, Firmware- und Dokumentationsdateien
- alle, die dateibasiert arbeiten und trotzdem **vollständige, reproduzierbare
  Produktstände** und eine **nachvollziehbare Historie** wollen

> **ℹ️ Lokales Desktop-Programm**
>
> Das PLM-Werkzeug ist ein **lokales Desktop-Programm**. Es braucht direkten Zugriff auf
> dein Dateisystem (Dateien beobachten, Programme öffnen, Versionsstände schreiben). Die
> Cloud kommt — wenn überhaupt — nur als *Backup- und Austausch-Remote* ins Spiel, **nie**
> als Dateiablage. Mehr dazu unter [Mehrbenutzer & Sync](Mehrbenutzer-und-Sync).

## Wie du dieses Handbuch liest

-   **[Konzepte](Konzepte-Ueberblick)**

    Die Begriffe und Denkweise des Werkzeugs: Produkt, Arbeitsbereich, Baustein,
    Werkbank, Meilenstein, Sperren. Lies das einmal, dann ist der Rest selbsterklärend.

-   **[Erste Schritte](Erstes-Produkt)**

    Eine bebilderte Schritt-für-Schritt-Anleitung — vom ersten Öffnen bis zum ersten
    Meilenstein. (Wir nehmen an, dass das Programm bereits installiert ist.)

-   **[Referenz](Die-Oberflaeche)**

    Jeder Bereich der Oberfläche erklärt, die Bedeutung der Status-LEDs und ein
    Glossar zum Nachschlagen.

> **ℹ️ Das Werkzeug wächst noch**
>
> Einige Funktionen befinden sich aktiv in Entwicklung. Solche Abschnitte sind im
> Handbuch klar als **„in Arbeit"** gekennzeichnet und können sich noch ändern.
