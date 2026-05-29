# UI-Stilbeschreibung — „Werkstatt-Instrument"

Stand: 29.05.2026. Visuelle Vorlage: **Teenage Engineering EP‑133 / K.O. II** (Design-Linie in der Tradition von Braun / Dieter Rams). Diese Beschreibung übersetzt die *Designsprache* des Geräts in Tokens und Komponenten-Regeln für unsere PLM-Oberfläche. Sie kopiert **nicht** die Marke (kein TE-Logo, keine Katakana-Beschriftung) — sie übernimmt die Haltung.

---

## 1. Leitidee: ein einziger lauter Akzent

Das Gerät ist fast komplett monochrom — warmes Hellgrau, Schwarz, Creme — mit **genau einer** lauten Farbe: Orange. Diese Sparsamkeit ist kein Zufall, sie ist das tragende Prinzip, und sie deckt sich 1:1 mit unserer Tool-Philosophie:

| Gerät | Unser Tool |
|---|---|
| Orange = der eine laute Akzent | **Laute Ausnahme** (E41): Konflikt, „wessen Stand gilt?", bewusste Freigabe, Historie-anfassen-Gate |
| Graues, ruhiges Grundbild | **Stiller Alltag** (E39/E41): still committen, stiller Sync — der Normalzustand ist leise |
| Kleine LED-Punkte als Status | **Abgeleiteter Status** (E35/E37): frei / gesperrt-von-X / in Arbeit |
| 7-Segment-Zahlendisplay | **Versionsleiste** (§24): Branch + Version + Status auf einen Blick |
| Alles funktional beschriftet | **„Sag nur, was du weißt"** (E26/E30): ehrliche Labels, keine erfundenen Zustände |
| Sichtbare Schrauben, ehrliche Konstruktion | **Transparente Ordnerstruktur**: das Tool versteckt das Dateisystem nicht |

**Regel Nummer eins für den Agenten:** Orange ist rationiert. Wenn auf einem Screen mehr als ein, zwei orange Elemente leuchten, ist etwas falsch. Routine ist grau.

---

## 2. Farb-Tokens

Werte sind nah am Gerät und bewusst *warm* gehalten (kein kaltes Reinweiß/Reingrau). Final justierbar.

```
/* Flächen / Chassis */
--surface-base:        #E4E2DC;  /* Gehäuse-Grau, Haupt-Hintergrund */
--surface-raised:      #F0EEE8;  /* helleres Panel, Karten-Oberfläche */
--surface-sunken:      #D7D4CD;  /* eingelassene Bereiche, Rillen */

/* Display / dunkle Zone */
--screen-bg:           #0E0D0C;  /* fast-schwarz, leicht warm */
--screen-fg:           #E8E6E1;  /* helle Schrift auf dunkel */

/* Tasten */
--key-dark:            #1C1A19;  /* schwarze Taste, warm */
--key-light:           #EDEAE3;  /* creme Taste */
--key-mid:             #94918C;  /* mittelgraue Taste (z. B. neutrale Aktion) */

/* Der eine Akzent — rationiert einsetzen */
--accent:              #F0421C;  /* Signal-Orange */
--accent-ink:          #FFFFFF;  /* Text auf Orange */

/* Status-Punkte (klein, LED-artig) */
--led-free:            #3C9A4B;  /* frei / sauber  (dezentes Grün) */
--led-working:         #C9C6BF;  /* in Arbeit / ruhend (helles Grau, „an") */
--led-attention:       #F0421C;  /* gesperrt / braucht Aufmerksamkeit (= accent) */
--led-off:             #6B6864;  /* aus / inaktiv */

/* Text */
--ink-strong:          #1C1A19;  /* Überschriften, Werte */
--ink-default:         #3A3833;  /* Fließtext */
--ink-muted:           #6B6864;  /* Labels, Sekundärinfo */

/* Linien */
--hairline:            #C9C6BF;  /* dünne Trennlinien, 1px */
```

Sekundärfarbe **nur im Display**: Das Gerät erlaubt sich im dunklen Screen einen blauen Punkt neben dem roten. Übertragen heißt das: Auf dunklen Flächen (Versionsbaum, Graph) darf ein kühles Blau (`#2E7FF0`) als zweite Datenfarbe auftauchen — z. B. „fremder Stand" vs. „mein Stand". Auf hellen Flächen bleibt es bei Grau + Orange.

---

## 3. Typografie

Zwei Schriften, klar getrennt nach Funktion — genau wie das Gerät Beschriftung und LED-Zahlen trennt.

- **Labels / UI-Text:** eine enge Neo-Grotesk (Inter, Aktiv Grotesk, Helvetica Now). Funktions-Labels in **GROSSBUCHSTABEN, leicht gesperrt** (`letter-spacing: 0.04em`), klein (11–12px). So wie `OUTPUT`, `SYNC`, `LEVEL` auf dem Gerät.
- **Daten / Werte:** eine Monospace (Berkeley Mono, JetBrains Mono, Space Mono). Für Versionsnummern, Pfade, Zeitstempel, Hashes, Statuswerte. Das gibt den „technischen, ehrlichen" Ton und passt zu Pfaden/`version.json`.
- **Versionsanzeige:** Die große aktive Versionsnummer in der Versionsleiste darf das 7‑Segment-Gefühl aufgreifen (große Mono-Ziffern, evtl. ein Hauch dunkler Hintergrund wie ein Mini-Display). Kein echtes 7‑Segment-Font nötig — der Eindruck reicht.

**Wichtig — kein Cargo-Cult:** Die Katakana („サンプラー") und das K.O.‑Logo sind TE-Markenidentität. Nicht übernehmen. Der „technische Akzent" entsteht bei uns über die Monospace und kleine funktionale Sub-Labels, nicht über fremde Schriftzeichen.

---

## 4. Form & Layout

- **Rastergeführt, modular.** Klare Spalten, viel Ruhe dazwischen. Das Gerät ist großzügig — Weißraum (Graufläche) ist Teil des Designs, nicht verschenkter Platz.
- **Zonen mit hohem Kontrast.** Helles Chassis ↔ dunkles Display. Bei uns: heller Arbeitsbereich (Artefakt-Karten, Aufgaben) ↔ dunkle „Display"-Zonen für Versionsbaum/Graph und die Versionsleiste. Das gibt der Versionsorientierung (§24) optisch ein eigenes „Instrument-Display".
- **Ecken:** leicht gerundet (4–6px), nicht verspielt. Tasten am Gerät sind sanft, nicht knubbelig.
- **Tiefe sparsam.** Karten und Tasten dürfen einen Hauch Erhebung haben (1px helle Oberkante + weicher 2–4px Schatten), damit „drückbar" lesbar wird. Keine fetten Material-Schatten.
- **Hairlines statt Boxen.** Trennung über 1px-Linien in `--hairline`, nicht über schwere Rahmen.
- **Ehrliche Konstruktion.** Das Gerät zeigt seine Schrauben. Übertragen: echte Pfade ruhig sichtbar lassen (Mono, gedämpft), den realen Speicherort zeigen — die Oberfläche soll das Dateisystem nicht verschleiern.

---

## 5. Komponenten

**Artefakt-Karte (§25)** — das Pendant zur Tastenfläche.
- Helle Karte (`--surface-raised`), Hairline-Rahmen, leichte Erhebung.
- Oben links: **Status-Punkt** (LED-Logik, s. u.) + Artefaktname in GROSS-Label.
- Hauptdatei als Mono-Zeile, gedämpft.
- Primäre Aktion als Taste (s. u.).
- **Orange erscheint hier nur**, wenn die Karte Aufmerksamkeit braucht (Prüfung erforderlich, gesperrt, Konflikt). Eine „geänderte, aber saubere" Karte ist grau.

**Status-Punkt (LED).** Kleiner gefüllter Kreis (8px), nicht Text-Badge. Frei = `--led-free`, in Arbeit/ruhend = `--led-working`, Aufmerksamkeit/gesperrt = `--led-attention`. Das ist die direkte Übersetzung der Lock-Signale (E37) — „gesperrt von X seit …" als Tooltip am orangen Punkt.

**Tasten.**
- *Primär/laut* (Freigabe, „Konflikt lösen"): Orange-Fläche, weißer Text. Selten.
- *Neutral* (Öffnen, Details): creme oder mittelgrau, dunkler Text.
- *Sekundär* (Als geprüft markieren, Ignorieren): nur Hairline-Outline, kein Fill.
- Schwarze Taste = bewusste/destruktive Schwere (z. B. „Historie anfassen"-Gate, E38/E27): dunkel, separiert, nie versehentlich klickbar.

**Versionsleiste (§24)** — das „Display".
- Dunkle Zone, helle Mono-Schrift. `Ember Reverb · main · v0.4` mit der Versionsnummer als größtes, hellstes Element.
- Status rechts als kleiner Text + Punkt. „Schreibgeschützt" → gedämpft. „In Arbeit" → normal. Sync-Zustand („gesichert" / „X arbeitet an Y") leise hier, nie als „push/pull" (E41).

**Versionsbaum / Graph** — dunkle Zone, Knoten als helle Punkte, Kanten als dünne Linien. Hier darf Blau (fremder Stand) neben Grau (mein Stand) auftreten. Ausgelagerte Knoten (E36) ehrlich gedämpft mit Vermerk „Inhalt ausgelagert".

---

## 6. Bewegung & Ton

- **Leise.** Übergänge schnell und unaufdringlich (120–180ms). Keine Bounce-, keine Attention-Animationen im Alltag.
- **Laut nur bei der Ausnahme.** Wenn der stille Sync anhält und fragt „wessen Stand gilt?" (E41), *darf* es kurz auffallen: orange Rahmen, ein Moment Fokus. Das ist der einzige Ort, an dem die UI „die Stimme hebt".

---

## 7. Was zu vermeiden ist

- Orange als Deko, Farbverläufe darauf, mehrere Akzentfarben.
- Generische SaaS-Optik: blaue Buttons überall, weiche Pastell-Schatten, runde Avatar-Bubbles, Emoji-Status.
- Git-Vokabular sichtbar (push/pull/commit/merge) — Stilbruch *und* Konzeptbruch (E33/E39/E41).
- Fremde Markenelemente (Katakana, Logo-Anmutung des K.O. II).
- Reinweiß/Reinschwarz/kaltes Grau — immer die warmen Töne nehmen.

---

## 8. Ein-Satz-Brief für den Agenten

> Baue ein ruhiges, warm-graues Werkstatt-Instrument mit Mono-Daten und Großbuchstaben-Labels, in dem **Orange ausschließlich für laute Ausnahmen** reserviert ist und Status über kleine LED-Punkte statt über Farbe der ganzen Fläche kommuniziert wird.
