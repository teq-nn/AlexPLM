# Git-Ehrlichkeit

Das Werkzeug nutzt Git und Git-LFS als Motor. Die Frage ist nicht *ob*, sondern *wie viel*
davon du siehst. Die Haltung dazu heißt **git-ehrlich** — und sie ist bewusst offen:
Basis-Git-Vokabular ist **sichtbar und erlaubt**, versteckt bleibt nur die gefährliche
Mechanik.

> **ℹ️ Geänderte Haltung**
>
> Eine frühere Fassung dieses Werkzeugs versuchte, *jedes* Git-Wort zu verstecken (sogar
> „Commit" hieß nur „Stand"). Das wurde **zurückgenommen**: Für ein git-kundiges kleines
> Team stiftet das Tarnen gewöhnlicher Begriffe mehr Verwirrung, als es verbirgt.

## Was sichtbar sein darf — und was versteckt bleibt

Der Test, auf welche Seite etwas gehört, lautet **„Wohin" vs. „Wie"**:

| „Wohin" — darf sichtbar sein | „Wie" — bleibt automatisiert/versteckt |
|---|---|
| Commit, Branch, Tag, Merge | `reset --hard`, `rebase`, `stash`, `reflog` |
| Push, Pull, Remote, Clone | `cherry-pick`, `lfs migrate`, `gc` / `prune` |
| History / Graph, „baut auf … auf" | manuelle Konfliktmarker-Auflösung |
| „bring mich zu Rev A" | Lock/Unlock-Plumberei, Hand-Chirurgie an `.gitattributes` |

**Orte und Zustände** geben Übersicht — sie dürfen sichtbar sein. **Beschwörungsformeln**,
bei denen man Daten verliert, bleiben dem Werkzeug überlassen.

> **Leitregel**
>
> Der Nutzer darf wissen, dass er auf Git arbeitet, und im Graphen denken — er soll aber nie
> aufgefordert werden, eine Recovery-Formel zu tippen. Will er „zu Rev A", klickt er den
> Punkt an; das Werkzeug führt das gefährliche Kommando im Hintergrund aus.

## Der Wert liegt in zwei Dingen

Das Werkzeug ist **kein** verkappter Git-Client. Sein Mehrwert ist:

1. **Die PLM-Domänenschicht obendrauf** — Produkt, Arbeitsbereich, Artefakt, Baustein,
   Revision, Aufgabe, Kante. Das sind Produktentwicklungs-Begriffe, nicht Git-Begriffe.
2. **Das Automatisieren und Verstecken der gefährlichen Mechanik** — genau die „Wie"-Seite
   aus der Tabelle oben.

## Was im Alltag still bleibt

Auch in der offenen Haltung bleiben zwei Dinge bewusst leise:

- **Der Auto-Commit.** Speichern erzeugt im Hintergrund einen Commit — ohne dass du je eine
  Commit-Nachricht tippst. Er heißt jetzt offen „Commit", bleibt aber automatisch (siehe
  [Versionen & Revisionen](Versionen-und-Revisionen)). Menschlicher Text entsteht nur an einer Revision.
- **Die ruhige Werkstatt-Optik** mit der Orange-Rationierung und den LED-Punkten. Laut wird
  es nur an der echten Ausnahme (siehe [Mehrbenutzer & Sync](Mehrbenutzer-und-Sync)) — die jetzt
  einen Merge-Konflikt auch beim Namen nennen darf.

## Die schwarze Gate-Taste

Bewusste, schwere oder destruktive Aktionen — einen alten Stand **zurückwerfen**, eine
Migration, die die Historie umschreibt — bekommen eine eigene, **dunkle, abgesetzte Taste**
mit ausdrücklicher Zustimmung („Ich werfe bewusst auf diesen Stand zurück"). Die Taste ist
inert, bis du die Zustimmung setzt, und nie versehentlich klickbar.

Ein Beispiel ist der **Import**: Liegen riesige Binärdateien bereits in der Git-Historie
eines Ordners, würde ein Aufräumen die Historie umschreiben. Das Werkzeug erlaubt das nur

- hinter der ausdrücklichen Zustimmung **und**
- nur bei einem frischen / ungeteilten Repository.

Existieren bereits geteilte Klone, **verweigert** das Werkzeug die Umschreibung — sie würde
fremde Kopien vergiften.

## Was du als Nutzer:in daraus mitnimmst

- Du darfst Git-Begriffe sehen und im Graphen denken — das ist Absicht.
- Du musst nie eine gefährliche Git-Formel von Hand tippen.
- Eine dunkle, abgesetzte Taste bedeutet immer: *bewusst, schwer, gut überlegen*.
