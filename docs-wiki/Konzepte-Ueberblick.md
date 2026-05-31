# Überblick & Denkweise

Bevor wir die einzelnen Begriffe durchgehen, lohnt sich das *Warum* — denn die meisten
Entscheidungen der Oberfläche folgen aus drei Haltungen.

## 1. Das Werkzeug sitzt neben deinen Ordnern, nicht davor

Klassische PLM-Systeme saugen deine Dateien in eine Datenbank und geben sie nur durch ihre
eigene Brille wieder heraus. Dieses Werkzeug macht das Gegenteil: deine echten
Projektordner auf der Platte **sind** die Wahrheit. Das Werkzeug

- erfindet **keine zweite Struktur** neben dem Dateisystem,
- zeigt echte Pfade ruhig und sichtbar an (in Monospace, gedämpft),
- und lässt jede Version auch **außerhalb** der Software vollständig nutzbar.

Folge: Wenn du das Werkzeug morgen löschst, hast du immer noch saubere, vollständige
Ordner mit deiner ganzen Arbeit.

## 2. Produktentwicklungs-Begriffe, nicht Git-Begriffe

Unter der Haube nutzt das Werkzeug Git (und Git-LFS) als Motor. Aber der Motor bleibt unter
der Haube. Die Substantive, die du auf dem Bildschirm siehst, sind **Begriffe aus der
Produktentwicklung**:

> Produkt · Arbeitsbereich · Artefakt · Baustein · Meilenstein · Freigabe · Aufgabe · Stand

Die gefährlichen Git-Mechaniken (Rebase, harte Resets, History umschreiben) bleiben
versteckt oder hinter ausdrücklichen Bestätigungen. Das ist die **Git-Client-Grenze**:

> **Design-Leitplanke**
>
> Sobald das Werkzeug dich bittet, in Commits / Merges / Rebases zu denken, ist es zu einem
> Git-Client degeneriert und hat seinen Sinn verloren.

Mehr dazu, was sichtbar bleibt und was nicht, unter [Git-Ehrlichkeit](Git-Ehrlichkeit).

## 3. Ruhe ist der Normalzustand — Orange ist die Ausnahme

Die Oberfläche ist bewusst monochrom und warm-grau gehalten, mit **genau einer** lauten
Farbe: Orange. Diese Sparsamkeit ist kein Zierrat, sondern Bedeutung:

- **Grau = Routine.** Stilles Speichern, stiller Abgleich, alles in Ordnung.
- **Orange = laute Ausnahme.** Eine fremde Sperre, ein echter Widerspruch beim Abgleich,
  eine bewusste Freigabe. Wenn auf einem Bildschirm mehr als ein, zwei orange Elemente
  leuchten, will dir das Werkzeug etwas Wichtiges sagen.

Status wird über kleine **LED-Punkte** kommuniziert, nicht über bunte Flächen. Die
Bedeutung der LEDs findest du in der [Referenz](Status-LEDs).

## Die Grundstruktur auf einen Blick

```text
Produkt  (z. B. "Ember Reverb")
├── Arbeitsbereich  (echter Ordner, z. B. elektronik/)
│   └── Artefakt     (logischer Bestandteil, z. B. der Schaltplan)
│       └── Datei(en) auf der Platte
├── Werkzeugkasten   (die für dieses Produkt gewählten Bausteine — eine Kopie)
└── Versionsbaum     (Stände & Meilensteine über die Zeit)
```

Jeden dieser Begriffe schauen wir uns auf den folgenden Seiten genauer an — der natürliche
Einstieg ist [Produkt, Arbeitsbereich & Artefakt](Produkt-Arbeitsbereich-Artefakt).
