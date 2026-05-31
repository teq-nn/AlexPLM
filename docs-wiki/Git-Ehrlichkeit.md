# Git-Ehrlichkeit

Das Werkzeug nutzt Git und Git-LFS als Motor. Die Frage ist nicht *ob*, sondern *wie viel*
davon du siehst. Die Haltung dazu hat sich geschärft und heißt **git-ehrlich**.

## Der Motor unter der Haube

Git ist für dieses Werkzeug das, was SQLite für eine App ist: ein robuster Motor, den du im
Alltag nicht direkt bedienst. Die **Substantive** der Oberfläche sind
Produktentwicklungs-Begriffe (Produkt, Arbeitsbereich, Artefakt, Baustein, Meilenstein,
Freigabe, Aufgabe, Stand) — nicht Git-Begriffe.

Das ist die **Git-Client-Grenze**:

> Sobald das Werkzeug dich bittet, in Commits / Merges / Rebases zu denken, ist es zu einem
> Git-Client degeneriert und hat seinen Sinn verloren.

## Was sichtbar bleibt — und was versteckt

Die Regel ist nicht „Git komplett verstecken", sondern eine bewusste Abstufung:

- **Stiller Alltag (versteckt):** automatisches Speichern → *Stand*; täglicher Abgleich →
  *aktuell / gesichert*. Hier taucht **kein** Git-Vokabular auf — einen täglichen Reflex
  versteckt man nicht nachträglich wieder weg.
- **Seltene Zeremonie (git-nah erlaubt):** ein Produkt erstmals teilen/veröffentlichen.
  Selten und risikoarm — hier darf die Sprache näher an Git liegen.
- **Gefährliche Mechanik (hinter einem Gate):** alles, was Historie umschreibt oder
  destruktiv ist.

## Die schwarze Gate-Taste

Bewusste, schwere oder destruktive Aktionen — „Historie anfassen", ein alter Stand
zurückwerfen, eine Migration, die die Historie umschreibt — bekommen eine eigene, **dunkle,
abgesetzte Taste**. Sie ist nie versehentlich klickbar und immer von einer ausdrücklichen
Bestätigung begleitet.

Ein Beispiel ist der **Import**: Liegen riesige Binärdateien bereits in der Git-Historie
eines Ordners, würde ein Aufräumen die Historie umschreiben. Das Werkzeug erlaubt das nur

- hinter der ausdrücklichen „Historie anfassen"-Bestätigung **und**
- nur bei einem frischen / ungeteilten Repository.

Existieren bereits geteilte Klone, **verweigert** das Werkzeug die Umschreibung — sie würde
fremde Kopien vergiften.

## Was du als Nutzer:in daraus mitnimmst

- Im Alltag musst du Git nicht kennen.
- Wo das Werkzeug doch Git-nahe Worte verwendet (beim Teilen), ist das Absicht und ein
  seltener Moment.
- Eine dunkle, abgesetzte Taste bedeutet immer: *bewusst, schwer, gut überlegen*.

> **ℹ️ In Arbeit**
>
> Die durchgängige git-ehrliche Wortwahl in allen Dialogen wird gerade vereinheitlicht.
