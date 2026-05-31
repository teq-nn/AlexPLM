# Wiki-Veröffentlichung

Das GitHub-Wiki ist eine **erzeugte Kopie** des Handbuchs aus `/docs`. Die MkDocs-Quelle
bleibt die maßgebliche Fassung; das Wiki ist die schlichte, private Sofort-Lösung (es erbt die
Sichtbarkeit des Repos — bei privatem Repo ist auch das Wiki privat).

## Neu erzeugen

```bash
python3 docs-tooling/wiki/convert.py   # schreibt nach ./docs-wiki
```

Erzeugt wird:

- eine Wiki-Seite je Handbuch-Seite (Material-Syntax → einfaches GitHub-Markdown),
- `_Sidebar.md` (Navigation) und `_Footer.md`,
- alle Screenshots unter `docs-wiki/img/`.

## Ins Wiki veröffentlichen

> Aus der Cloud-Session ist das nicht möglich (der Git-Proxy erlaubt nur das Haupt-Repo).
> Diese Schritte führst du **lokal** aus.

```bash
# 0. Wiki einmalig anlegen: im Browser  Repo → Wiki → "Create the first page" → speichern.
#    (Erst dadurch existiert das Wiki-Repo <repo>.wiki.git.)

# 1. Wiki-Repo clonen (neben dem Haupt-Repo)
git clone https://github.com/teq-nn/AlexPLM.wiki.git

# 2. Erzeugte Seiten + Bilder hineinkopieren (ersetzt den Inhalt 1:1)
cp -r AlexPLM/docs-wiki/* AlexPLM.wiki/

# 3. Veröffentlichen
cd AlexPLM.wiki
git add -A
git commit -m "Handbuch aus /docs"
git push
```

Danach ist das Handbuch unter `https://github.com/teq-nn/AlexPLM/wiki` erreichbar
(für alle, die das Repo sehen dürfen).

## Was im Wiki anders ist als in MkDocs

Bewusste Vereinfachungen, weil das Wiki nur einfaches GitHub-Markdown rendert:

| MkDocs Material | Im Wiki |
|---|---|
| Theme (warmes Grau, oranger Akzent, eure Schriften) | GitHub-Standard-Styling |
| `!!! note/tip/warning` Boxen | Blockzitate mit Emoji-Kopf |
| Content-Tabs (`=== "…"`) | `####`-Abschnitte |
| Karten-Kacheln auf der Startseite | normale Liste |
| Volltextsuche | GitHub-Wiki-Suche |
| verschachtelte Navigation | manuell gepflegte `_Sidebar.md` |
| Status-LED-Punkte (CSS) | farbige Emoji 🟢 ⚪ 🟠 |

Inhalt und Screenshots sind vollständig identisch.
