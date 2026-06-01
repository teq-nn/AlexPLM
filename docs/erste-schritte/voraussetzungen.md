# Voraussetzungen

!!! note "Installation"
    Dieses Handbuch nimmt an, dass die **Werkbank bereits installiert** ist. Eine
    ausführliche Installationsanleitung folgt separat.

## Was du brauchst

- **Die Werkbank** als Desktop-Programm (Windows, macOS oder Linux).
- **Git** und **Git-LFS** auf dem System — die Werkbank nutzt sie als Motor im Hintergrund.
- Deine **Fachprogramme** (z. B. KiCad, Fusion 360, ein PDF-Betrachter). Die Werkbank öffnet
  Dateien immer mit dem im Betriebssystem hinterlegten **Standardprogramm**.
- Für die **Zusammenarbeit im Team** zusätzlich: Zugang zu einem **Git-Server** (Remote), der
  Git-LFS-Sperren unterstützt (z. B. Forgejo/Gitea). Solo und ungeteilt brauchst du keinen
  Server.

## Empfehlungen

- Lege deine Produktordner auf eine **lokale Platte** (oder ein zuverlässiges Netzlaufwerk) —
  **nicht** in einen synchronisierten Cloud-Ordner wie Dropbox. Die Cloud kommt nur als
  Git-Remote ins Spiel, siehe [Mehrbenutzer & Sync](../konzepte/mehrbenutzer.md).
- Sorge dafür, dass deine Fachprogramme korrekt als Standardprogramm für ihre Dateitypen
  registriert sind — dann funktioniert das Ein-Klick-Öffnen der Artefakt-Karten reibungslos.

Wenn das steht, geht es weiter mit dem [ersten Produkt](erstes-produkt.md).
