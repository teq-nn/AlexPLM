# ADR 0001 — Desktop-Stack: Tauri v2 + Svelte

- **Status:** Akzeptiert
- **Datum:** 2026-05-30
- **Kontext:** Issue #2 (Gerüst & Stack-Entscheidung). Die PRD (#1) lässt die App-Tech-Stack-Wahl
  bewusst offen ("App-Tech-Stack-Wahl (Tauri/Electron o. ä.)" unter *Out of Scope* des Konzept-Grillens).
  Sie ist hier als erste Bau-Entscheidung zu treffen.

## Entscheidung

Wir bauen das PLM-Werkzeug als **Tauri v2** Desktop-App mit einem **Svelte (SvelteKit, adapter-static / SPA)**
Frontend in der WebView. Backend-Logik in **Rust**.

## Begründung

1. **Reine Kerne in Rust passen exakt zur Test-Doktrin der PRD.** Die PRD verlangt
   "reiner Kern + Tabellentest": die sicherheitskritischen Entscheidungen (Lock Warden,
   Mergeability Classifier, Sync Decider, Import Gate, Graph Projection, Edge Logic) sind
   reine Funktionen `Zustands-Snapshot → Entscheidung`. Rusts Enums, Pattern-Matching und
   `#[cfg(test)]`-Tabellentests sind ein natürliches Zuhause dafür — ohne echtes Repo testbar.
2. **git / git-lfs ist nur ein Unterprozess.** Die Vault-Engine ruft `git` / `git-lfs` über
   `std::process::Command`. Kein Bedarf für eine schwere git-Library; die Engine bleibt dumm
   und ausführend (PRD-Architekturgrundlinie).
3. **Schlank passt zur Haltung.** ~6 MB Binary, ~80 MB RAM-Idle. Ein ruhiges "Werkstatt-Instrument"
   soll sich nicht wie ein 200-MB-Chromium-Bündel anfühlen. Tauri nutzt die System-WebView
   (webkit2gtk-4.1 auf dieser Linux-Maschine bestätigt).
4. **Svelte für ein bespoke Instrument-UI.** Die Oberfläche ist klein, eigenwillig und
   CSS-token-getrieben (warm-grau, LED-Punkte, Mono-Werte). Svelte gibt minimalen Boilerplate,
   keine Virtual-DOM-Last und CSS-first-Styling, das sich 1:1 auf die PRD-Design-Tokens abbilden
   lässt. React-Vorteile (Ökosystem/Ubiquität) wiegen bei dieser UI-Größe wenig.

## Verworfene Alternative: Electron + React/TS

Ubiquitär und schnell zu scaffolden, aber: ~150 MB Binary, ~200 MB RAM, gebündeltes Chromium —
gegen die schlanke Instrument-Haltung. Reine Kerne in TS sind möglich, aber Rusts Typsystem
modelliert die Entscheidungs-Enums strenger. Electron bringt keinen tragenden Vorteil für ein
kleines, selbstgehostetes Team-Werkzeug.

## Konsequenzen

- **System-Voraussetzungen (Linux):** `webkit2gtk-4.1` (vorhanden), `librsvg2-dev` (für den Build),
  `git` + `git-lfs` (ab Issue #3 nötig; **git-lfs ist auf der Dev-Maschine noch nicht installiert**).
- **Repo-Layout:** App unter `app/` (Frontend `app/src`, Rust unter `app/src-tauri`), getrennt von
  `design-docs/`.
- **Frontend-Sprache:** TypeScript; **keine git-Begriffe** in sichtbarem Text (PRD §49).
- Die Vault-Engine wird als Rust-Modul implementiert; Kerne als separate, reine Rust-Module mit
  `#[cfg(test)]`-Tabellentests. Der erste Kern (Mergeability Classifier) etabliert das Muster (Issue #3).

## Plattform-Ziele (Windows + Linux)

Das PLM-Werkzeug **muss auf Windows und Linux laufen** (macOS ist nicht gefordert). Tauri trägt beide
nativ. **Entwicklung und Tests laufen vorerst nur unter Linux**; Windows wird gebaut/geprüft, sobald
eine Windows-Umgebung bereitsteht. Bis dahin gilt: plattformneutral schreiben und die folgenden
Windows-Stolpersteine bewusst vermeiden.

- **WebView-Laufzeit:** Linux nutzt `webkit2gtk-4.1` (+ `librsvg2-dev` zum Bündeln). Windows nutzt
  **WebView2** (Edge-Runtime) — auf Zielrechnern ggf. mitliefern/installieren (Tauri-`webview2`-Bootstrapper).
  `librsvg2`/`webkit2gtk` sind reine Linux-Build-Abhängigkeiten.
- **Pfade:** Niemals Separatoren hartkodieren. Im Rust-Kern immer `std::path::Path`/`PathBuf` +
  `join()` verwenden; nach außen (Anzeige, Domänenmodell) **Forward-Slash** normalisieren — so wie
  `projection.rs::rel_path` es bereits tut. Keine Annahmen über Groß-/Kleinschreibung (Windows ist
  case-insensitive, Linux case-sensitive) bei der Baustein-/Dateierkennung.
- **git / git-lfs als Unterprozess:** Auf Windows liegen die Binaries evtl. nicht im `PATH`
  (z. B. „Git for Windows"). Die Vault-Engine muss `git`/`git-lfs` robust auffinden (PATH-Suche,
  bekannte Installationsorte, ggf. konfigurierbar) statt blind `git` aufzurufen.
- **Zeilenenden:** CRLF vs. LF nicht als Inhaltsänderung missdeuten (relevant ab Auto-Commit/Watcher,
  Issue #4). Für textuelle Mergebarkeit (Mergeability Classifier) ist das Verhalten plattformgleich zu halten.
- **Dateisperren/`.git`-Zugriff:** Windows sperrt offene Dateien aggressiver; beim späteren Schreiben
  (Commit, `lfs`-Operationen) mit „file in use"-Fällen rechnen.

**Konsequenz für Tests:** Linux-only-CI ist für v1 akzeptiert, deckt aber die pfad-/PATH-/CRLF-bedingten
Windows-Fälle **nicht** ab. Diese Risiken bleiben offen, bis eine Windows-Prüfung dazukommt; reine
Logik-Kerne sind plattformneutral zu halten, damit sie später ohne Umbau auch unter Windows grün sind.
