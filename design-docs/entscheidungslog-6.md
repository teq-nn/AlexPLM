# Entscheidungslog — Werkbank

Stand: 07.06.2026 (Strang #128). Jede Entscheidung mit Begründung und — wo zutreffend — was sie in
früheren Einträgen (E1–E47) oder im Originalkonzept (`plm_software_konzept.md`) ersetzt oder
verfeinert. Dieser Log führt den gestapelten Strang um #128 (E48–E56) — die Punkte rund um das
ehrliche Zusammenführen von beobachtetem Zustand und Werkzeug-Gedächtnis.

---

## E49 — Reconcile beim Öffnen: stiller Divergenz-Abgleich gegen das `_plm`-Gedächtnis
**Entscheidung:** Beim **Öffnen** eines Produkts gleicht die Werkbank den **real beobachteten**
git-/Sperren-/Platten-Zustand **still** gegen das `_plm`-**Gedächtnis** (den zuletzt gesehenen
Zustand) ab. Ist außerhalb des Werkzeugs gearbeitet worden — im Terminal, per `west`, auf einer
anderen Maschine, oder während das Werkzeug zu war — **holt die Werkbank still auf**, statt den
Nutzer auf einem veralteten Stand weiterarbeiten zu lassen. Sie sät ihr Gedächtnis neu auf die
beobachtete Wirklichkeit und sagt **nichts**. Nur eine Divergenz, die **nicht** still auflösbar ist,
bekommt eine **Stimme** — und dann in **Domänensprache**, **nie** als roher git-Text.

Der reine Kern liegt auf der **Sync-Decider-Linie (Eingang A)**: `(zuletzt gesehenes _plm-Gedächtnis,
beobachteter git-/Sperren-Zustand) → Divergenz-Entscheidung + Meldung`, **ohne I/O**, total und
deterministisch — wie `syncdecider.rs`, `import_gate.rs`, `locks.rs`. Der Klebstoff liest den realen
Zustand und führt den stillen Abgleich aus; die Entscheidung wird dort **nie** getroffen, nur befolgt.

**Die drei Orte der Wahrheit, ehrlich benannt.** Die Werkbank tut **nicht** so, als gäbe es einen
Speicher. Es gibt genau **drei** Orte, an denen eine Tatsache wohnen kann, und jeder heißt im
Werkzeug, was er **ist**:

- **Platte = Inhalt** — die Dateien im Arbeitsbaum. Der *Inhalt*; der einzige Ort, an dem die echten
  Bytes des Nutzers liegen.
- **git = Verlauf** — die Commit-Historie. Die *Historie* — dauerhaft, geteilt, das Protokoll, wie der
  Inhalt hierher kam.
- **Server-Sperren = flüchtige Koordination** — die `git lfs locks`. Nur *flüchtige Koordination*: wer
  ein unteilbares Artefakt gerade hält. Nie Inhalt, nie Verlauf — sie verdunstet, sobald eine Sperre
  fällt.

Die drei driften **unabhängig**. Der Abgleich liest alle drei und urteilt je Ort ehrlich: **Verlauf**
außerhalb weitergelaufen → still aufgeholt; **Inhalt** auf der Platte außerhalb geändert → still
aufgeholt (der Watcher übernimmt ab da); **Koordination** sauber gewandert (Sperre dazugewonnen/
freigegeben, kein Streit) → still aufgeholt. Die **eine** Divergenz, die das Werkzeug **nicht** still
entscheiden darf: ein unteilbares Artefakt, das es zuletzt als **unseres** kannte, hält jetzt ein
**Kollege** — zwei Seiten halten es für ihres. Das hebt die einzige **Abgleichfrage** an („Bens Sperre
liegt jetzt auf deinem Gehaeuse — wessen Arbeit gilt?"). Das Lauteste gewinnt (wie die Präzedenz des
Status-Readers): ein einziger Eigentumsstreit übertrumpft alle stillen Aufholungen.

**Abgrenzung zum stillen Sync (E41).** Der Abgleich beim Öffnen holt das Werkzeug auf die *beobachtete*
Wirklichkeit auf; der stille Sync (`syncdecider.rs`/`syncglue.rs`) führt ein *tatsächlich divergiertes
Remote* zusammen. Verschiedene Aufgaben über verschiedene Eingaben — derselbe Eingang-A-Stil, dieselbe
Marker-Garantie. Beide teilen die **eine** Definition von „verbotenem Text" (`text_has_git_marker`), so
dass die Kerne nie auseinanderlaufen, was als git-Marker gilt.

**Eigenschaft (bewiesen, erschöpfend).** Keine Meldung — weder eine stille Aufholungs-Notiz noch die
Abgleichfrage — enthält je einen sichtbaren git-Marker (`<<<<<<<`, `HEAD`, `merge`, …). Feindselige
Pfade und Kollegennamen werden neutralisiert, bevor sie in die Frage gewoben werden.

**Warum:** Zwischen zwei Öffnungen bewegt sich die Welt draußen. Ohne Abgleich arbeitet der Nutzer auf
einem veralteten Bild, oder das Werkzeug zeigt rohen git-Text, wenn es stolpert. E49 macht das
Aufholen zum stillen Normalfall und reserviert die Stimme für den echten, nicht still lösbaren
Widerspruch — den Eigentumsstreit. Die ehrliche Benennung der drei Orte verhindert die Lebenslüge
„ein Speicher", die sonst genau beim Divergenz-Fall zerbricht.
**Verfeinert:** E37 (Lies zurück statt spiegeln — dieselbe einzige Sperren-Quelle), E41 (gleiche
Linie, gleiche Marker-Garantie; getrennte Aufgabe), ADR 0002 (`_plm`-Degradationsregel: ein fehlendes
Gedächtnis lernt die Welt beim ersten Öffnen, nie ein Fehler).
**Umfang jetzt (E49a):** der reine Reconcile-Kern (`reconciler.rs`) mit Tabellentest über die
Divergenzmatrix, der Klebstoff (`reconcileglue.rs`, `reconcile_product`-Kommando) mit Glue-Tests, die
stille Aufholung beim Öffnen ohne Prompt und die einzelne Abgleichfrage in der UI
(`AbgleichBeimOeffnen.svelte`) mit ehrlicher Benennung der drei Orte. Das **Auflösen** des
Eigentumsstreits (wessen Arbeit weiterläuft) ist ein Folge-Slice.

---

## Offene / verschobene Punkte (Strang #128)
- **Auflösen der Abgleichfrage (E49-Folge):** den Eigentumsstreit aktiv entscheiden (Sperre brechen /
  übernehmen / abwarten), analog zum `resolve_sync` des stillen Syncs. Hier nur gemeldet, nicht gelöst.
- Frühere Offenpunkte aus den Sitzungen 5/6 (Glossar/PRD-Kaskade aus E43, Produkt-only-Label E44,
  Werkbank-Status-Filter E45) unverändert.
