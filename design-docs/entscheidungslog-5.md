# Entscheidungslog — PLM-Werkzeug

Stand: 31.05.2026 (6. Sitzung). Jede Entscheidung mit Begründung und — wo zutreffend — was sie in
früheren Einträgen (E1–E41) oder im Originalkonzept (`plm_software_konzept.md`) ersetzt oder überholt.
Neu in Sitzung 6: E42–E46 — die zuvor nie gegrillten Restpunkte aus der PRD-Lückenanalyse
(`v2-prd-luecken.md`): Strenge-Träger im `main`-Modell, die **Revision der Git-Sichtbarkeit** (zurück
zu E6), Tags, Filter und das Haupt-/Zusatzdatei-Aktionsmodell.

---

## E42 — Strenge ist Eigenschaft der Meilenstein-Art (Prototyp/Freigabe), per Toggle
**Entscheidung:** Die Block-Strenge (E19) hängt nicht mehr am **Branch-Typ** (E15), sondern an der
**Art des Meilensteins**. Ein neuer Meilenstein ist per Default **Prototyp** (lax: nur Warnungen,
kein harter Block) — so taggt man reibungsfrei. Ein **Toggle** hebt ihn auf **Freigabe** (streng):
im Moment des Umschaltens feuert der dreistufige Block (E19.3); besteht er, wird der Tag
schreibgeschützt (E8). Zurückschalten Freigabe→Prototyp ist ein bewusstes „Un-Release" (erlaubt —
der Ausweg ist einen Handgriff entfernt, E22). „Releasen" **ist** damit der Toggle, kein zweiter Akt.
Der zweite E15-Auslöser „Merge nach Production" **entfällt**: es gibt kein Production-`main` mehr
(E34), nur den Meilenstein-Akt als einzigen strengen Checkpoint. Das Pro-Task-Opt-out „blockiert
überall" bleibt für den seltenen kontextunabhängigen Fall.
**Warum:** E34 (beide auf `main`, keine personengebundenen Branches) löst den Branch-Typ als Träger
der Strenge auf — „`main` immer streng" wäre falsch, weil man Prototyp-Revs (Rev A) auf `main` taggt.
E15s echter Kern (manche Meilensteine sind Wegwerf-Prototypen, manche Releases) wandert sauber auf den
**Akt**. E22 (streng nur am bewussten Checkpoint) bleibt gewahrt.
**Überholt/verfeinert:** **E15** (Strenge nicht mehr branch-typ-gebunden); verfeinert **E19/E28/E32**.

---

## E43 — Git-Sichtbarkeit revidiert: zurück zu E6, nur die gefährliche Mechanik bleibt versteckt
**Entscheidung:** Basis-Git-Vokabular ist **sichtbar und erlaubt** — *commit, branch, tag, merge,
push, pull, History/Graph, remote, clone*. Ein Commit heißt Commit, ein Branch heißt Branch. Versteckt
bleibt **nur die gefährliche „Wie"-Mechanik**: `reset --hard`, `rebase`, `stash`, `reflog`,
`cherry-pick`, `lfs migrate`, `gc`/`prune`, manuelle Konfliktmarker-Auflösung, die Lock/Unlock-
Plumberei, Hand-Chirurgie an `.gitattributes`/`.gitignore`. Die erfundenen Domänen-Synonyme
(„Stand" statt Commit, das Verbergen von „branch"/„push") werden **zurückgenommen** — sie stiften mehr
Verwirrung, als sie verbergen.
**Tägliche Sync-Folge:** Der Netz-Sync wird **manuell** — sichtbare **Push-/Pull-Schaltflächen**
(nicht mehr stiller Hintergrund-Sync, **E41 revidiert**). Der **Auto-Commit bleibt still** (E39 —
Reibungsreduktion, nicht Vokabular; jetzt offen „Commit" genannt, weiterhin keine Hand-Messages auf
dem Happy Path). Die **zwei Push-Arten bleiben** (E35): **Sicherung** = der Push-Button im Alltag
(persönliches Backup), **Freigabe** = gebunden an den Freigabe-Toggle des Meilensteins (E42; bringt
die Datei auf den geteilten `main` und löst die Sperre).
**Warum:** Reaktiviert **E6** als Leitregel. E33/E39/E41 hatten die Sichtbarkeitsgrenze auf „keine
git-Substantive" verschärft; für ein git-kundiges Zweierteam (E1) bringt das nur Übersetzungs-
Verwirrung. Der Wert des Werkzeugs ist (a) die **PLM-Domänenschicht** obendrauf (Produkt,
Arbeitsbereich, Artefakt, Baustein, Meilenstein, Task, Kante) und (b) das **Automatisieren/Verstecken
der gefährlichen Mechanik** — nicht das Tarnen gewöhnlicher git-Begriffe.
**Überholt:** **E33** wird umgeschrieben — aus „sobald das Werkzeug dich in Commits/Merges denken
lässt, ist es ein Git-Client" wird *„ein PLM-Werkzeug, das **ehrlich auf git läuft** und nur die
gefährliche Mechanik automatisiert/versteckt"*. Die **Vokabular-Haltung** von **E39/E41**;
`ui-stilbeschreibung` §7/§125 („kein git-Vokabular sichtbar"); und die git-Versteck-Klauseln der
**PRD-User-Stories #21, #43–#49**.
**Bleibt unangetastet:** stiller Auto-Commit (E39), die ruhige Werkstatt-Instrument-UI samt
Orange-Rationierung/LED, die laute Ausnahme (darf jetzt „Merge-Konflikt" beim Namen nennen),
Auto-Unlock + Binär-Invariante (E35).

---

## E44 — Tags gestrichen
**Entscheidung:** Keine objektübergreifende Freitext-**Tag**-Schicht (Original §26.2, nie gegrillt).
Die Beispiel-Tags lösen sich in bestehende erstklassige Konzepte auf: *Prototyp/Serienstand* →
**Meilenstein-Art** (E42); *Firmware benötigt* → **Task** (E14); *JLCPCB/Fertigung* →
**Artefakt/Baustein**; *Kundenvariante* → **Variante**; *Audio/OEM* → **Produktbeschreibung**.
**Warum:** Tags sind hand-gepflegte, drift-anfällige **Zweitwahrheit** (E18/E26) — genau das, was das
Werkzeug überall sonst entfernt hat („Prototyp" getaggt, wird Serie → das Tag lügt). Der einzige
Restbedarf (themen-Triage über *viele* Produkte) ist geparkt wie E30s Index-Grenze; falls je nötig,
nur ein **dünnes produkt-only Label** im Such-Fan-out — nicht auf Versionen/Artefakten/Tasks.
**Streicht:** §26.2.

---

## E45 — Filter minimal und nur über Bekanntes; Werkbank-Status-Filter vertagt
**Entscheidung:** Filter speichern/erfinden nichts (kein Drift) und sind erlaubt — aber **nur über
abgeleitete/erstklassige Fakten** (E26/E30, „sag nur, was du weißt"). v2 baut: **Graph-Raum**
(Varianten ein/aus, „nur Meilensteine" = E9) und **Suche** (Produkt-/Meilenstein-/Artefakt-Name). Der
**Werkbank-Status-Filter wird vertagt** — Ordnerbaum-Nav (E24) und Meilenstein-Check decken den
Alltag; er lohnt erst bei großen Produkten. **Kein** Arbeitsbereich-Filter (Nav deckt ihn), **kein**
Tag-Filter (E44), „archiviert" nur falls der Status überlebt.
**Verfeinert:** §26.1.

---

## E46 — Primäre Aktion abgeleitet; Hauptdatei aus Glob-Priorität; kein gespeicherter Rollen-Status
**Entscheidung:** Das §12-Modell (Haupt-/Zusatzdatei + primäre Aktion) wird übernommen, aber in die
**Baustein-Artefakt-Definition** verlegt und **abgeleitet** statt gespeichert: der Baustein deklariert
Glob(s) + **Endungs-Priorität**; die **Hauptdatei** ist der höchstpriorisierte Treffer; die **Aktion**
ergibt sich — dominante Einzeldatei → diese öffnen, sonst Ordner öffnen. Optionaler Pro-Artefakt-
Override für den Sonderfall. Geöffnet wird per **OS-Handover** (§13, keine eigene Programmzuordnung).
**Kein** pro-Datei `role`/`status` mehr.
**Warum:** E10 (Konvention statt Eingabe) + E18/E26 (kein gespiegelter Zweitstand). Tötet §28.4
`version.json.role`/`status` endgültig — die Karte zeigt Hauptdatei (Mono) + primäre Aktion, beides
live abgeleitet.
**Verfeinert:** §12/§13/§25; festigt E10/E26.

---

## Offene / verschobene Punkte (Stand 6. Sitzung)
- **Glossar/PRD-Kaskade aus E43:** Glossar-Einträge „Stand vs. Commit", „Wohin vs. Wie",
  „Git-Client-Grenze" und die git-Versteck-Klauseln der PRD/`ui-stilbeschreibung` müssen auf die neue
  Sichtbarkeitslinie umgeschrieben werden; die Build-Strings (`Stand`/`gesichert`) folgen. Noch nicht
  ausgeführt.
- **Produkt-only Label (E44)** — falls die Produktzahl Themen-Triage erzwingt; geparkt wie E30.
- **Werkbank-Status-Filter (E45)** — vertagt bis große Produkte es rechtfertigen.
- Frühere Offenpunkte aus Sitzung 5 (Auslagern v1-fern, Bruch fremder Sperren, Kanten-Heuristik,
  Lese-Oberfläche, Windows) unverändert.
