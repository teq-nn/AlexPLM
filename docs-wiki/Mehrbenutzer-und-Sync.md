# Mehrbenutzer & Sync

Sobald mehr als eine Person an einem Produkt arbeitet, kommt ein **Server (Git-Remote)** ins
Spiel. Dieses Kapitel erklärt, wie das Werkzeug Zusammenarbeit *ohne* Datenverlust und *ohne*
Git-Vokabular ermöglicht.

## Wo die Daten liegen

- **Die App läuft lokal.** Sie braucht direkten Datei- und Git-Zugriff — eine reine
  Browser-Lösung könnte das nicht.
- **Die Cloud ist nur ein Remote.** Sie dient als Backup, Mehrgeräte-Abgleich und
  Austauschpunkt — **nie** als Dateiablage. Dein Produktordner gehört auf deine Platte, nicht
  in einen synchronisierten Cloud-Ordner.

> **⚠️ Server wird Pflicht, sobald geteilt wird**
>
> Das Sperren von Binärdateien wird serverseitig koordiniert. Darum braucht ein **geteiltes**
> Produkt zwingend einen Remote. Solo und ungeteilt funktioniert alles auch offline.

## Stiller Sync

Der tägliche Abgleich läuft — wie das Speichern — **still im Hintergrund**: das Werkzeug
holt beim Öffnen und in Ruhephasen den neuesten Stand und sichert deinen. Du siehst dafür nie
„push/pull/merge", sondern nur einen ruhigen Status:

- **aktuell** — dein Stand entspricht dem geteilten Stand,
- **gesichert** — dein Stand wurde gesichert.

## Sperren: Koordination für Binärdateien

Die entscheidende Achse im Team ist nicht Person-gegen-Person, sondern **mergebar vs. nicht
mergebar**:

| Eimer | Beispiele | Umgang |
|---|---|---|
| **Text, mergebar** | Firmware-Quellen, Doku, Text-BOM | Git führt zusammen |
| **Binär, unmergebar** | `.f3d`, STEP, STL, ZIP, Fotos | **Sperren** |
| **nominell Text, faktisch unmergebar** | KiCad-Quellen (`.kicad_sch`/`.kicad_pcb`) | wie Binär behandeln |

Für unmergebare Dateien gilt:

- Im Ruhezustand liegen sperrbare Binärdateien **schreibgeschützt** auf der Platte — das
  *ist* das sichtbare Signal: „read-only = frei, niemand dran".
- **Bearbeiten holt die Sperre.** Öffnest du so eine Datei zum Bearbeiten, holt das Werkzeug
  automatisch die Sperre — die Datei wird *für dich* beschreibbar.
- Für Kolleg:innen erscheint sie dann als **„gesperrt von X seit …"**.

Wer im Team gerade welche Binärdatei in Arbeit hat, zeigt das Panel rechts:

![Das „Fremde Sperren"-Panel zeigt, dass Ben das Gehäuse in Arbeit hat](img/fremde-sperren.png)

Auf der zugehörigen Artefakt-Karte leuchtet dann der **orange** Status-LED — der eine laute
Akzent. Eine Sperre ist reine **Koordination** („nicht gleichzeitig, sonst geht Arbeit
verloren"), **keine** Autorisierung („wer darf was"). Rollen und Rechte gibt es bewusst
nicht.

> **ℹ️ In Arbeit**
>
> Das „Fremde Sperren"-Panel bekommt einen selbsterklärenderen Namen. Sichtbare manuelle
> Sync-Knöpfe (Sicherung / Freigabe) werden gerade ergänzt.

## Die zwei Push-Arten

Hinter dem stillen Sync stehen zwei scharf getrennte Arten, Arbeit auf den Server zu bringen:

- **Sicherungs-Push** *(privat)* — spiegelt deine Zwischenstände (inkl. halbfertiger
  Binärdatei) in einen **persönlichen** Backup-Bereich. Backup ja, Freigabe nein. Das
  Werkzeug meldet dies ruhig als **„gesichert"**.
- **Freigabe-Push** *(öffentlich)* — bringt die *fertige* Binärdatei auf den geteilten Stand
  **und** gibt die Sperre frei („ich bin fertig damit"). Gemeldet als **„freigegeben"**.

Daraus folgt die tragende Sicherheitsregel:

> **Binär-Invariante**
>
> Eine gesperrte Binäränderung darf den geteilten Stand nicht erreichen, solange die Sperre
> gehalten wird.

Das macht gefährliche Merge-Situationen strukturell unmöglich: Was beim Abgleich je sichtbar
wird, ist bereits freigegeben — also bereits entsperrt — und der Abgleich berührt nur freie
Dateien. Eine vergessene Sperre heilt sich am nächsten Checkpoint selbst.

## Die laute Ausnahme

Es gibt **einen** Moment, in dem das Werkzeug die Stimme hebt: Wenn der stille Sync auf einen
echten, nicht auflösbaren Widerspruch trifft, hält er an und fragt in **eigener** Sprache —
nie mit Git-Konfliktmarkern:

> „Dein und Bens Gehäuse-Stand widersprechen sich — welcher gilt?"
>
> [ mein Stand ] · [ Bens Stand ]

Du wählst, welcher Stand gilt; das Werkzeug schließt den Abgleich sauber ab. Das ist der
einzige orange umrahmte Augenblick der ganzen Oberfläche.

## Einrichtungs-Zeremonie

Ein Produkt zu teilen ist ein **einmaliger** Schritt pro Produkt (Server anbinden, erstmals
veröffentlichen, Kolleg:innen einladen). Dieser seltene, risikoarme Moment **darf**
git-näher formuliert sein — im Gegensatz zum täglichen Sync, der still bleibt. Den Ablauf
beschreibt [Produkt teilen](Produkt-teilen).
