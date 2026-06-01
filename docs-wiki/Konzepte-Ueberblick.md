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

## 2. Eine PLM-Schicht, die ehrlich auf Git läuft

Unter der Haube nutzt das Werkzeug Git (und Git-LFS) als Motor. Anders als ein reiner
Git-Client legt es darüber eine **Produktentwicklungs-Schicht** mit eigenen Substantiven:

> Produkt · Arbeitsbereich · Artefakt · Baustein · Revision · Freigabe · Aufgabe

Basis-Git-Begriffe (Commit, Branch, Tag, Push, Pull, Merge) **dürfen** dabei sichtbar sein —
sie zu tarnen würde für ein git-kundiges Team nur verwirren. Versteckt und automatisiert
bleibt nur die **gefährliche Mechanik**, bei der man Daten verliert (Rebase, harte Resets,
History umschreiben):

> **Leitregel**
>
> Der Nutzer darf wissen, dass er auf Git arbeitet, und im Graphen denken — er soll aber nie
> aufgefordert werden, eine Recovery-Formel zu tippen.

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
└── Versionsbaum     (Commits & Revisionen über die Zeit)
```

Jeden dieser Begriffe schauen wir uns auf den folgenden Seiten genauer an — der natürliche
Einstieg ist [Produkt, Arbeitsbereich & Artefakt](Produkt-Arbeitsbereich-Artefakt).
