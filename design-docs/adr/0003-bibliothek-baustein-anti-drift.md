# ADR 0003 — Bibliothek, Baustein-Identität und Anti-Drift-Verteilung

- **Status:** Akzeptiert
- **Datum:** 2026-05-31
- **Kontext:** Issue #39. Ein **Baustein** bündelt Tool-Wissen (Heimat-Ordner, Artefakt-Globs,
  Ignore-/LFS-Muster, Öffnen-Aktion, Startaufgaben, interne Default-Kanten). Die **Bibliothek** hält
  Standard-Bausteine und -Toolstacks außerhalb der Produkte. Zwei Kräfte stehen in Spannung:
  **Anti-Drift** (eine Bibliotheks-Änderung darf ein laufendes Produkt nie verändern, PRD §8) und der
  Wunsch, **kritische Verbesserungen zentral an alle Installationen** auszurollen.

## Entscheidung

### Baustein-Identität
Stabile Kebab-`id` (`"kicad"`) + monotone Ganzzahl-`version`. Felder:
`id, version, name, heimat, globs (geordnet — [0] = Hauptdatei), ignore[], lfs[],
oeffnen (auto|datei|ordner, default auto), startaufgaben[], default_kanten[], stillgelegt:bool`.
Lockability ist **kein** Baustein-Feld — sie ist formatintrinsisch und bleibt in `classifier.rs`.

### Bibliothek (Vorlagen-Quelle, außerhalb der Produkte)
Liegt im OS-App-Data-Verzeichnis (Tauri `app_data_dir`), JSON, nutzer-editierbar:
```
<app-data>/plm-werkzeug/bibliothek/
├── bausteine/<id>.json
└── toolstacks/<id>.json      ({ id, name, baustein_ids: [...] })
```
Sowohl einzelne Bausteine als auch komplette Toolstacks sind repräsentierbar.

### Produkt-Stack = Kopie (Anti-Drift, hart)
Beim Anlegen wird der gewählte Toolstack als **vollständige, selbsttragende Kopie** nach
`_plm/stack.json` geschrieben (ADR 0002). Die Kopie enthält die ganze Definition **plus** einen
Herkunfts-Stempel `{from: id, version}` — **nur** für Anzeige/„Update verfügbar", **kein** Live-Link.
Das Produkt funktioniert auch, wenn die Bibliothek fehlt. Eine Bibliotheks-Änderung erreicht ein
laufendes Produkt **nie** automatisch.

### Zentrale Verteilung der Default-Bausteine
Die **kanonischen** Defaults leben **im PLM-Software-Repo** unter
`app/src-tauri/resources/bibliothek/` und werden als Tauri-Ressourcen ausgeliefert. Eine „kritische
Änderung" ist ein **PR ins Repo**, der die `version` des Bausteins anhebt und mit dem nächsten Release
ausgeliefert wird. Beim ersten Start **und bei jedem App-Update** läuft ein **idempotentes Seeding**:

- Default lokal nicht vorhanden → installieren.
- Lokale `version` < gebündelte `version` **und unverändert** → upgraden.
- Lokal **verändert** → behalten, „Update verfügbar" anzeigen (der Herkunfts-/Versionsstempel macht
  das erkennbar).
- **Nutzer-eigene** Bausteine → nie angefasst.

Produkt-Kopien bleiben davon **unberührt** (Anti-Drift).

## Begründung

- **Vollkopie + Stempel** löst beide Kräfte ohne Widerspruch: das laufende Produkt driftet nie, aber
  die Versionsdifferenz bleibt sichtbar genug für einen bewussten Upgrade-Hinweis.
- **Repo als kanonische Quelle** macht „einmal verbessern, alle profitieren" zu einem gewöhnlichen PR
  + Release statt eines separaten Verteilkanals.
- **version-gegated, edits-schonend** verhindert sowohl stillen Stillstand (Seed-once) als auch stilles
  Zerstören lokaler Anpassungen (immer überschreiben).

## Verworfene Alternativen

- **Eine `bibliothek.json`** — weniger Dateien, aber größere Diffs und unhandlich beim Hand-Hinzufügen.
- **Gebündelt read-only ohne Nutzer-Kopie** — killt die „geteilte Standard-Toolstacks"-Geschichte (§7).
- **Update immer überschreiben** — einfachste Logik, kann aber lokale Tweaks an einem Default wegwischen.
- **Seed-once + manuelles Update** — kein automatisches Ausrollen zentraler Fixes.
- **Content-Hash-Identität / `lockable` als Baustein-Feld** — verworfen: erschwert „Baustein erweitern"
  bzw. dupliziert die Mergebarkeits-Wahrheit aus `classifier.rs`.

## Konsequenzen

- Neue Module (Issue #39): `baustein.rs` (Modell + serde, reiner Teil), `bibliothek.rs` (Speicher/Glue
  + idempotentes Seeding/Upgrade), Default-Definitionen unter `app/src-tauri/resources/bibliothek/`.
- Onboarding (#48) schreibt Ignore/LFS der kopierten Bausteine in Dotfile-Marker-Blöcke; Stilllegen
  (#51) setzt `stillgelegt` (label-only) und lässt Ignore/LFS als Sediment stehen.
- „Update verfügbar"-Hinweis ist ein späterer UI-Belang; der Versionsstempel ab #39 macht ihn möglich.
