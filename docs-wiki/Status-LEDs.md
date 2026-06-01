# Status-LEDs

Das Werkzeug kommuniziert Status über kleine, gefüllte **LED-Punkte** — nicht über bunte
Flächen oder Text-Badges. Die Logik ist bewusst sparsam: **Grau ist Routine, Orange ist die
Ausnahme.**

## Die LEDs der Artefakt-Karten

Jede Artefakt-Karte trägt oben links einen Punkt, der den Zustand ihrer (sperrbaren)
Hauptdatei zeigt:

| LED | Bedeutung | Heißt für dich |
|---|---|---|
| 🟢 **Grün — frei** | niemand bearbeitet die Datei; sie liegt schreibgeschützt (read-only) auf der Platte | du kannst sie übernehmen, indem du sie bearbeitest |
| ⚪ **Grau — in Arbeit / ruhend** | normaler Arbeitszustand bzw. von dir in Bearbeitung | alles in Ordnung, keine Aktion nötig |
| 🟠 **Orange — fremd gesperrt** | ein:e Kolleg:in bearbeitet diese Binärdatei gerade | nicht gleichzeitig anfassen; Tooltip zeigt „gesperrt von X seit …" |

> **ℹ️ Read-only ist Absicht**
>
> Dass eine sperrbare Binärdatei schreibgeschützt auf der Platte liegt, ist **kein Fehler** —
> es *ist* das Signal „frei, niemand dran". Sie bearbeiten zu wollen, holt automatisch die
> Sperre und macht sie für dich beschreibbar.

## Statuspunkte in den Instrument-Leisten

In den dunklen „Display"-Leisten (Sync-Status, „geteilt", Import-Ergebnis) bedeuten die
Punkte:

| LED | Bedeutung |
|---|---|
| 🟢 **Grün** | abgeschlossen / freigegeben / frisch angelegt (der „fertig"-Zustand) |
| ⚪ **Grau** | ruhend / gesichert / in Arbeit (der ruhige Alltag) |

## Der Sync-Status in Worten

Neben den Punkten meldet das Werkzeug den Abgleich in Worten (siehe
[Mehrbenutzer & Sync](Mehrbenutzer-und-Sync)):

- **aktuell** — dein Stand entspricht dem geteilten Stand,
- **gesichert** — deine Arbeit wurde (privat) gesichert,
- **{Name} arbeitet an {Datei}** — eine fremde Sperre (hat Vorrang),
- **geteilt** — das Produkt ist eingerichtet und mit dem Server verbunden.

Der Abgleich selbst ist Handarbeit (**Sichern** / **Holen**) — die gefährliche Mechanik
darunter bleibt versteckt.

## Die eine laute Farbe

Orange ist **rationiert**. Es erscheint nur bei einer echten Ausnahme:

- eine **fremde Sperre** auf einer Datei, die du anfassen willst,
- der **orange Rahmen** der lauten Sync-Ausnahme („welcher Stand gilt?").

Wenn auf einem Bildschirm mehr als ein, zwei orange Elemente leuchten, will dir das Werkzeug
etwas Wichtiges sagen. Routine ist immer grau.
