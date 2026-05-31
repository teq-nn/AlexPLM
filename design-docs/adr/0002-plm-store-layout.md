# ADR 0002 — `_plm`-Store: committeter, geteilter Produkt-Zustand

- **Status:** Akzeptiert
- **Datum:** 2026-05-31
- **Kontext:** Issue #39 (Baustein-Modell, Grundlagen-Gate für PRD v2). Mehrere v2-Slices
  (#40 Aufgaben, #41 Meilenstein-Art, #56 Kanten, der Produkt-Stack selbst) brauchen einen Ort
  für „nur das, was git nicht ohnehin weiß". Die PRD nennt diesen Ort `_plm`. Bisher lag der
  einzige Vorläufer verstreut als Dotfile (`.plm-kanten.json`, siehe `edgestore.rs`).

## Entscheidung

Pro Produkt gibt es **ein sichtbares, git-getracktes Verzeichnis `_plm/`** mit **einer JSON-Datei
pro Belang**:

```
produkt/
├── elektronik/  mechanik/  firmware/   (Arbeitsbereiche)
├── _plm/                 (committet, geteilte Wahrheit)
│   ├── stack.json        (Produkt-Stack = Kopie der Bausteine, ADR 0003)
│   ├── aufgaben.json
│   ├── meilensteine.json (Meilenstein-Art pro Tag)
│   └── kanten.json
└── .plm-local/           (gitignored: gehaltene Sperren, Maschinen-Zustand)
```

- `_plm/` ist **committet und geteilt** — beide Entwickler sehen dieselben Aufgaben, dieselbe
  Meilenstein-Art, dieselben Kanten. Das trägt die personenübergreifenden Blocks (PRD §34) und die
  geteilten Stale-Warnungen.
- **Ephemeres** (gehaltene Sperren, rein lokaler Maschinen-Zustand) liegt **außerhalb** in
  `.plm-local/` und ist **gitignored** — es darf nie geteilte Wahrheit werden.
- `projection.rs` **überspringt** Einträge, die mit `_plm` beginnen, damit der Baustein-Walk das
  Werkzeug-Verzeichnis nie für einen Arbeitsbereich hält (bisher leistete das die Dotfile-Tarnung).
- Lesemuster bleibt wie `edgestore.rs`: fehlende/leere/korrupte Datei ⇒ **leerer Zustand, nie Fehler**;
  geschrieben wird pretty-printed JSON (ehrlich, diffbar).

## Begründung

1. **Ein Zuhause statt Dotfile-Streu.** Ein Verzeichnis ist als „Bereich des Werkzeugs" lesbar und
   hält die wachsende Zahl v2-Belange (Stack, Aufgaben, Meilensteine, Kanten) beieinander.
2. **Geteilt = committet ist eine Anforderung, kein Komfort.** §34 (fremde offene Block-Aufgabe hält
   auch mich) und die Stale-/Meilenstein-Ehrlichkeit funktionieren nur, wenn beide Seiten denselben
   Stand sehen. Lokaler `_plm` würde diese Geschichten still brechen.
3. **Sichtbar + bewusst übersprungen** ist ehrlicher als versteckt: der Nutzer sieht, dass das Werkzeug
   hier seine wenigen Notizen ablegt; der Walk ignoriert es per Namensregel statt per Tarnung.

## Verworfene Alternativen

- **Verstreute Dotfiles weiterführen** — kein gemeinsames Zuhause, größere kognitive Last, jede neue
  Datei braucht erneut die Hidden-Tarnung.
- **`_plm` lokal/gitignored** — einfacher, bricht aber §34 und die geteilte Stale-/Meilenstein-Sicht.

## Konsequenzen

- `edgestore.rs` wandert von `.plm-kanten.json` nach `_plm/kanten.json` (Migration: alten Pfad noch
  lesen, neu schreiben unter `_plm/`).
- Offene PRs #58 (Meilenstein-Art) und #61 (Aufgaben) nutzen noch das alte Dotfile-Muster
  (`.plm-meilenstein-art.json`, `.plm-aufgaben.json`) und sind vor dem Merge auf `_plm/` anzugleichen.
- `projection.rs` braucht die `_plm`-Skip-Regel; Tests entsprechend.
- `.gitignore` des Produkts muss `.plm-local/` führen (Onboarding #48 hängt das idempotent ein).
