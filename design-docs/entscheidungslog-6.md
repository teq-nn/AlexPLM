# Entscheidungslog — Werkbank

Stand: 07.06.2026 (7. Sitzung). Fortschreibung von `entscheidungslog-5.md`. Jede Entscheidung mit
Begründung und — wo zutreffend — was sie in früheren Einträgen (E1–E47) oder im Originalkonzept
(`plm_software_konzept.md`) ersetzt oder überholt. Begriffe und Entscheidungen aus Sitzung 1–6 gelten
unverändert weiter.

---

## E55 — Ehrliche git-Substantive sichtbar (read-only)
**Entscheidung:** Der HW-Ingenieur darf die ehrlichen git-Substantive — **Commit, Branch, Tag, Push** —
**sehen**, ohne sie bedienen zu müssen. Das Wort ist klar benannt; es gibt **keinen Ort**, an dem eine
Wiederherstellungs-Formel getippt wird. Reine **Sichtbarkeit/String-Bestätigung** über die bestehende
Projektions-/Graph-Anzeige (read-only).
**Wo:** Die Detail-Karte des Versionsbaums (`VersionTree.svelte`, der Graph-Raum-Layer aus E45) nennt
die vier Substantive als ruhige Readouts neben den Domänenwörtern: **Commit** (der Stand), **Branch**
(die Linie, auf der er sitzt), **Tag** (das git-Tag unter einer Revision — eine Revision ist
„technisch ein Tag auf einem Commit", Glossar) und **Push** (dieser Stand hat die geteilte Linie
erreicht — der ehrliche Begriff hinter „veröffentlicht"). Das Domänenwort bleibt die Überschrift; das
git-Substantiv reitet als ehrliches Echo daneben.
**Nicht operierbar:** Keines dieser Substantive bekommt eine Schaltfläche, kein Feld, keine
Wiederherstellungs-/git-Formel. Die Karte ist ein reiner Lese-Readout (`pointer-events: none`); die
gefährliche „Wie"-Mechanik (Zurückwerfen, abzweigen, freigeben) bleibt — unverändert — in ihren
eigenen, abgeriegelten Verben (E27/E38/E43). Bestehendes Domänen-Wording bleibt konsistent: keine
rohen Konflikt-/Kommando-Strings.
**Warum:** Setzt **E43** fort, das das Basis-Git-Vokabular (*commit, branch, tag, push, …*) bereits für
**sichtbar und erlaubt** erklärt hat; für ein git-kundiges Zweierteam (E1) verwirrt das Tarnen
gewöhnlicher Begriffe mehr, als es verbirgt. E55 macht die schon erlaubten Substantive an der
bestehenden Anzeige konkret **sichtbar**, ohne sie bedienbar zu machen.
**Verfeinert:** E43 (macht die dort freigegebenen Substantive an der Graph-Projektion sichtbar);
ergänzt E45 (der read-only Graph-Raum) und E47 (das „veröffentlicht"-Prädikat, jetzt zusätzlich ehrlich
als „Push" benannt). „Tag" hier ist das git-Tag unter einer Revision — **nicht** die in **E44**
gestrichene objektübergreifende Freitext-Tag-Schicht.
**Umfang jetzt:** User-sichtbare Strings in der Detail-Karte + dieser Eintrag + eine
String-/Sichtbarkeits-Bestätigung (Test). Keine neuen Kommandos, keine Backend-Änderung.
