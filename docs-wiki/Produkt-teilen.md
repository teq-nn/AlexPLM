# Produkt teilen

Solange du allein arbeitest, braucht das Werkzeug keinen Server. Sobald ihr zu zweit oder
mehr an einem Produkt arbeitet, wird ein **Git-Remote** angebunden. Das geschieht in einer
einmaligen **Einrichtungs-Zeremonie** pro Produkt.

> **ℹ️ In Arbeit**
>
> Die Teilen-Funktionen werden gerade ausgebaut (u. a. sichtbare manuelle Sync-Knöpfe und
> das robuste Veröffentlichen an bereits gefüllte Server-Repositories). Dieser Abschnitt
> beschreibt das stabile Grundprinzip; einzelne Schritte und Dialoge können sich noch
> ändern. Diese Seite wird ergänzt, sobald die Funktionen fertig sind.

## Die Einrichtungs-Zeremonie (einmalig)

Bei einem frisch angelegten Produkt bietet das Werkzeug an, es zu teilen. Die Zeremonie führt
durch:

1. **Server anbinden** — die Adresse deines Git-Servers (Remote) hinterlegen.
2. **Veröffentlichen** — den aktuellen Stand erstmals auf den Server bringen.
3. **Einladen** — eine zugangsfreie Klon-Adresse erhalten, die du Kolleg:innen gibst.

> **ℹ️ Warum hier Git-nähere Worte erlaubt sind**
>
> Das Teilen ist ein **seltener, einmaliger** Schritt pro Produkt. Anders als der tägliche,
> stille Abgleich darf die Sprache hier näher an Git liegen — das ist Absicht.

Sobald ein Produkt geteilt ist, zeigt die Werkbank dies ruhig mit einem **„geteilt"**-Status
in der Werkzeugleiste an.

## Der Alltag danach

Nach dem Einrichten bleibt die Zusammenarbeit **still**:

- Das Werkzeug gleicht den Stand im Hintergrund ab (**aktuell / gesichert**).
- Binärdateien werden über **Sperren** koordiniert; wer was in Arbeit hat, steht im
  „Fremde Sperren"-Panel.
- Nur bei einem echten, nicht auflösbaren Widerspruch hebt das Werkzeug die Stimme und fragt
  in eigener Sprache, **welcher Stand gilt**.

Das vollständige Modell — stiller Sync, die zwei Push-Arten, die Binär-Invariante und die
laute Ausnahme — steht unter [Mehrbenutzer & Sync](Mehrbenutzer-und-Sync).

> **⚠️ Server ist im Team Pflicht**
>
> Da Sperren serverseitig koordiniert werden, ist ein erreichbarer Remote für ein geteiltes
> Produkt zwingend. Fällt der Server aus, könnt ihr lokal weiterarbeiten, aber die
> Koordination ruht, bis er wieder erreichbar ist.
