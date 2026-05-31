# docs-tooling

Werkzeug rund um das Benutzerhandbuch (`/docs`, gebaut mit MkDocs Material).

## Handbuch lokal bauen / ansehen

```bash
pip install -r docs-tooling/requirements.txt
mkdocs serve            # lokaler Vorschau-Server auf http://127.0.0.1:8000
mkdocs build --strict   # statische Ausgabe nach ./site (wie in der CI)
```

Veröffentlicht wird automatisch per GitHub Actions
(`.github/workflows/docs.yml`) als GitHub Pages, sobald Änderungen unter `docs/`,
`mkdocs.yml` oder den Build-Abhängigkeiten auf `main` landen.

> Einmalig im Repo nötig: **Settings → Pages → Source = „GitHub Actions"**.

## Screenshots neu erzeugen

Die Bilder unter `docs/img/` sind reproduzierbar. Sie zeigen die **echten** Svelte-
Komponenten und die echten Design-Tokens — nur die Backend-Daten sind ein
repräsentativer Platzhalter ("Ember Reverb"), per gemocktem Tauri-Backend eingespeist.

```bash
# 1. Frontend bauen (statische SPA nach app/build)
pnpm -C app install
pnpm -C app build

# 2. Headless-Chrome besorgen (einmalig; nutzt die Google-CDN, nicht die Playwright-CDN)
npx puppeteer@23 browsers install chrome

# 3. Abhängigkeiten der Aufnahme-Skripte
npm --prefix docs-tooling/screenshots install

# 4. Aufnehmen → schreibt PNGs nach docs/img/
node docs-tooling/screenshots/capture.mjs
```

Falls die Chrome-Binärdatei woanders liegt, den Pfad per Umgebungsvariable setzen:

```bash
CHROME_PATH=/pfad/zu/chrome node docs-tooling/screenshots/capture.mjs
```

Die Szenen und die Platzhalter-Daten stehen in
[`screenshots/capture.mjs`](screenshots/capture.mjs); neue Aufnahmen fügst du dort als
weitere `shot(...)`-Aufrufe hinzu.
