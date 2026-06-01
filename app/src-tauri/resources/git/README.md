# Gebündeltes git/git-lfs für den Windows-Build

Dieses Verzeichnis nimmt das **portable Git-for-Windows (MinGit)** auf, das der Windows-Build des
PLM-Werkzeugs mitliefert, damit git/git-lfs **keine** System-Voraussetzung mehr sind. Zur Laufzeit
verdrahtet `lib.rs::wire_bundled_git` (nur `#[cfg(windows)]`) den Pfad `cmd/git.exe` in den
`gitrunner` (`gitrunner::set_git_program`). Fehlt das Bundle, fällt der `gitrunner` still auf das
System-`git` im PATH zurück — die App startet trotzdem.

> Hinweis: Auf Linux/CI ist dieses Verzeichnis irrelevant — `set_git_program` wird dort nie gerufen,
> git kommt wie bisher aus dem PATH.

## Was hier hingehört

Das **`cmd/git.exe`-Layout** eines portablen Git-for-Windows, inklusive git-lfs:

```
resources/git/
  cmd/
    git.exe                      <- das Programm, auf das gitrunner zeigt
  mingw64/
    bin/                         <- gitrunner stellt dies dem Child-PATH voran (findet git-lfs.exe)
      git-lfs.exe
    libexec/
      git-core/                  <- GIT_EXEC_PATH (interne git-Subkommandos + lfs-Helfer)
    etc/
      gitconfig                  <- System-gitconfig (siehe unten)
```

Der `gitrunner` leitet die Bundle-Wurzel defensiv aus dem git-Pfad ab (`cmd/git.exe` → Parent von
`cmd`) und setzt — falls die Unterverzeichnisse existieren — `PATH`-Prepend auf `mingw64/bin` und
`GIT_EXEC_PATH` auf `mingw64/libexec/git-core`. Fehlt ein Verzeichnis, wird der jeweilige
Env-Eintrag schlicht weggelassen (nie ein Absturz).

## Woher

- Offizielle **Git-for-Windows-Releases**: <https://github.com/git-for-windows/git/releases>
- Das **„MinGit"-Asset** (`MinGit-<version>-64-bit.zip`) ist die schlanke, portable Variante ohne
  Installer. Es bringt das `cmd/git.exe`- und `mingw64/`-Layout bereits mit.
- **git-lfs.exe**: in aktuellen Git-for-Windows-Builds bereits unter `mingw64/bin/git-lfs.exe`
  enthalten. Ist es das nicht, das passende `git-lfs`-Windows-Release dorthin legen:
  <https://github.com/git-lfs/git-lfs/releases>. Danach einmalig `git lfs install --system` gegen
  das gebündelte git laufen lassen (oder den lfs-Filter direkt in die System-`gitconfig` eintragen).

## System-gitconfig (`mingw64/etc/gitconfig`)

Dem Bundle eine System-`gitconfig` beilegen — sie gilt dann für **jedes** Repo, das das gebündelte
git anfasst, ohne pro Repo etwas setzen zu müssen:

```ini
[core]
	autocrlf = false
	longpaths = true
```

- `core.autocrlf = false` — verhindert, dass das gebündelte git Zeilenenden umschreibt; sonst
  deutet der Watcher/Classifier reine CRLF/LF-Wechsel als Inhaltsänderung.
- `core.longpaths = true` — erlaubt tiefe PLM-Pfade jenseits von MAX_PATH (260 Zeichen).

## Vendoring-Politik (mit Maintainer klären)

Die git-Binaries gehören **nicht roh ins Repo** — Größe und LFS-Politik sind mit dem Maintainer
abzustimmen (z. B. via Git LFS, als separates Release-Asset oder als reiner CI-Download-Schritt).
Für den lokalen Windows-Build genügt es, das MinGit-ZIP hierher zu entpacken; dieses Verzeichnis
selbst (außer dieser README) wird **nicht** committet.
