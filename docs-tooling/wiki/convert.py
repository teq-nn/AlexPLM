#!/usr/bin/env python3
"""Konvertiert das MkDocs-Handbuch (../../docs) in GitHub-Wiki-taugliches Markdown (../../docs-wiki).

Das Wiki rendert nur einfaches GitHub-Markdown — kein MkDocs-Material. Dieses Skript übersetzt
daher die Material-spezifische Syntax in schlichtes GFM:

  - !!! admonition / ??? details      → Blockzitate mit fettem Kopf + Emoji
  - === "Tab"   (content tabs)        → #### Überschriften mit ausgerücktem Inhalt
  - <div class="grid cards"> … </div> → normale Liste
  - :material-icon: Icons             → entfernt
  - <span class="led led-…">          → farbige Emoji-Punkte (🟢 / ⚪ / 🟠)
  - ../img/… und img/…                → img/… (Bilder liegen im Wiki-Repo unter img/)
  - [Text](pfad/seite.md#anker)       → [Text](Wiki-Seitenname#anker)

Die MkDocs-Quelle bleibt unverändert; das Wiki ist eine erzeugte Kopie.
"""

from __future__ import annotations

import re
import shutil
from pathlib import Path

HERE = Path(__file__).resolve().parent
DOCS = HERE.parent.parent / "docs"
OUT = HERE.parent.parent / "docs-wiki"

# Quelle (relativ zu docs/)  →  (Wiki-Seitenname, Anzeigename in der Sidebar)
PAGES = {
    "index.md": ("Home", "🏠 Start"),
    "konzepte/ueberblick.md": ("Konzepte-Ueberblick", "Überblick & Denkweise"),
    "konzepte/produkt-struktur.md": ("Produkt-Arbeitsbereich-Artefakt", "Produkt, Arbeitsbereich & Artefakt"),
    "konzepte/bausteine.md": ("Bausteine-und-Werkzeugkasten", "Bausteine & Werkzeugkasten"),
    "konzepte/werkbank-graph.md": ("Werkbank-und-Graph-Raum", "Werkbank & Graph-Raum"),
    "konzepte/versionen.md": ("Versionen-und-Meilensteine", "Versionen & Meilensteine"),
    "konzepte/aufgaben.md": ("Aufgaben-und-Hinweise", "Aufgaben & Hinweise"),
    "konzepte/mehrbenutzer.md": ("Mehrbenutzer-und-Sync", "Mehrbenutzer & Sync"),
    "konzepte/git-ehrlichkeit.md": ("Git-Ehrlichkeit", "Git-Ehrlichkeit"),
    "erste-schritte/voraussetzungen.md": ("Voraussetzungen", "Voraussetzungen"),
    "erste-schritte/erstes-produkt.md": ("Erstes-Produkt", "Erstes Produkt"),
    "erste-schritte/teilen.md": ("Produkt-teilen", "Produkt teilen"),
    "referenz/oberflaeche.md": ("Die-Oberflaeche", "Die Oberfläche"),
    "referenz/status-leds.md": ("Status-LEDs", "Status-LEDs"),
    "referenz/glossar.md": ("Glossar", "Glossar"),
}

SECTIONS = [
    ("Konzepte", [
        "konzepte/ueberblick.md", "konzepte/produkt-struktur.md", "konzepte/bausteine.md",
        "konzepte/werkbank-graph.md", "konzepte/versionen.md", "konzepte/aufgaben.md",
        "konzepte/mehrbenutzer.md", "konzepte/git-ehrlichkeit.md",
    ]),
    ("Erste Schritte", [
        "erste-schritte/voraussetzungen.md", "erste-schritte/erstes-produkt.md",
        "erste-schritte/teilen.md",
    ]),
    ("Referenz", ["referenz/oberflaeche.md", "referenz/status-leds.md", "referenz/glossar.md"]),
]

# Quell-Dateiname (Stamm)  →  Wiki-Seitenname (für die Link-Umschreibung).
STEM2PAGE = {Path(src).stem: page for src, (page, _) in PAGES.items()}

ADMONITION = {
    "note": ("ℹ️", "Hinweis"),
    "info": ("ℹ️", "Info"),
    "tip": ("💡", "Tipp"),
    "warning": ("⚠️", "Achtung"),
    "danger": ("🛑", "Achtung"),
    "quote": (None, None),
}
LED = {"led-frei": "🟢", "led-arbeit": "⚪", "led-achtung": "🟠"}

TAB_RE = re.compile(r'^=== "(.*)"\s*$')
ADM_RE = re.compile(r'^(\s*)(?:!!!|\?\?\?\+?|\?\?\?) (\w+)(?:\s+"(.*)")?\s*$')
LED_RE = re.compile(r'<span class="led (led-[a-z]+)"></span>')
ICON_RE = re.compile(r':material-[a-z0-9-]+:\s*')
LINK_RE = re.compile(r'\]\(([^)\s]+?\.md)(#[^)]*)?\)')


def deindent(lines: list[str], n: int) -> list[str]:
    out = []
    for ln in lines:
        if ln.strip() == "":
            out.append("")
        elif ln[:n].strip() == "":
            out.append(ln[n:])
        else:
            out.append(ln)
    return out


def pass_tabs(text: str) -> str:
    """=== "Titel"  →  #### Titel  (Inhalt um 4 Spaces ausgerückt)."""
    lines = text.split("\n")
    out: list[str] = []
    i = 0
    while i < len(lines):
        m = TAB_RE.match(lines[i])
        if not m:
            out.append(lines[i]); i += 1; continue
        title = m.group(1)
        i += 1
        body: list[str] = []
        while i < len(lines):
            ln = lines[i]
            if TAB_RE.match(ln):  # Geschwister-Tab beendet den aktuellen
                break
            if ln.strip() == "" or ln[:4].strip() == "":
                body.append(ln); i += 1
            else:
                break
        # abschließende Leerzeilen abtrennen
        while body and body[-1].strip() == "":
            body.pop()
        out.append("")
        out.append(f"#### {title}")
        out.append("")
        out.extend(deindent(body, 4))
        out.append("")
    return "\n".join(out)


def pass_admonitions(text: str) -> str:
    lines = text.split("\n")
    out: list[str] = []
    i = 0
    while i < len(lines):
        m = ADM_RE.match(lines[i])
        if not m:
            out.append(lines[i]); i += 1; continue
        indent, kind, title = m.group(1), m.group(2).lower(), m.group(3)
        base = len(indent)
        i += 1
        body: list[str] = []
        while i < len(lines):
            ln = lines[i]
            if ln.strip() == "" or len(ln) - len(ln.lstrip(" ")) >= base + 4:
                body.append(ln); i += 1
            else:
                break
        while body and body[-1].strip() == "":
            body.pop()
        body = deindent([b[base:] if b.strip() else "" for b in body], 4)

        emoji, default = ADMONITION.get(kind, ("ℹ️", "Hinweis"))
        head = None
        if kind == "quote":
            head = f"**{title}**" if title else None
        else:
            label = title if title else default
            head = f"**{emoji} {label}**"

        out.append("")
        if head:
            out.append(f"> {head}")
            out.append(">")
        for b in body:
            out.append(f"> {b}".rstrip())
        out.append("")
    return "\n".join(out)


def pass_grid_cards(text: str) -> str:
    lines = text.split("\n")
    return "\n".join(
        ln for ln in lines
        if not ln.strip().startswith('<div class="grid cards"')
        and ln.strip() != "</div>"
    )


def rewrite_link(m: re.Match) -> str:
    target, anchor = m.group(1), m.group(2) or ""
    stem = Path(target).stem
    page = STEM2PAGE.get(stem)
    if not page:
        return m.group(0)  # unbekannt: unverändert lassen
    return f"]({page}{anchor})"


def pass_inline(text: str) -> str:
    text = LED_RE.sub(lambda m: LED.get(m.group(1), ""), text)
    text = ICON_RE.sub("", text)
    text = text.replace("](../img/", "](img/")
    text = LINK_RE.sub(rewrite_link, text)
    return text


def convert(text: str) -> str:
    text = pass_tabs(text)
    text = pass_admonitions(text)
    text = pass_grid_cards(text)
    text = pass_inline(text)
    text = re.sub(r"\n{3,}", "\n\n", text)  # mehrfache Leerzeilen eindampfen
    return text.strip() + "\n"


def build_sidebar() -> str:
    out = ["**[🏠 Start](Home)**", ""]
    for section, srcs in SECTIONS:
        out.append(f"**{section}**")
        out.append("")
        for src in srcs:
            page, label = PAGES[src]
            out.append(f"- [{label}]({page})")
        out.append("")
    return "\n".join(out).strip() + "\n"


def build_footer() -> str:
    return (
        "_Erzeugt aus `/docs` (MkDocs-Quelle) mit `docs-tooling/wiki/convert.py`. "
        "Nicht direkt im Wiki bearbeiten — Änderungen in `/docs` vornehmen und neu erzeugen._\n"
    )


def main() -> None:
    if OUT.exists():
        shutil.rmtree(OUT)
    (OUT / "img").mkdir(parents=True)

    for src, (page, _) in PAGES.items():
        text = (DOCS / src).read_text(encoding="utf-8")
        (OUT / f"{page}.md").write_text(convert(text), encoding="utf-8")

    for png in sorted((DOCS / "img").glob("*.png")):
        shutil.copy2(png, OUT / "img" / png.name)

    (OUT / "_Sidebar.md").write_text(build_sidebar(), encoding="utf-8")
    (OUT / "_Footer.md").write_text(build_footer(), encoding="utf-8")

    pages = sorted(p.name for p in OUT.glob("*.md"))
    imgs = sorted(p.name for p in (OUT / "img").glob("*.png"))
    print(f"{len(pages)} Wiki-Seiten + {len(imgs)} Bilder → {OUT}")
    for p in pages:
        print(f"  • {p}")


if __name__ == "__main__":
    main()
