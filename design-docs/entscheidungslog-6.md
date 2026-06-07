# Entscheidungslog — Werkbank

Stand: 07.06.2026 (7. Sitzung / Strang #128). Fortschreibung von `entscheidungslog-5.md`. Jede
Entscheidung mit Begründung und — wo zutreffend — was sie in früheren Einträgen (E1–E47) oder im
Originalkonzept (`plm_software_konzept.md`) ersetzt, überholt oder verfeinert. Begriffe und
Entscheidungen aus Sitzung 1–6 gelten unverändert weiter. Dieser Log führt den gestapelten Strang um
#128 (E48–E56) — die Punkte rund um das ehrliche Zusammenführen von beobachtetem Zustand und
Werkzeug-Gedächtnis, die im Grill-Review geschärften Restpunkte sowie die ehrlichen git-Substantive.

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

**Umfang E49b — Offline-Sperre (Absichts-Sperre) + laute Doppelbearbeitungs-Ausnahme (Issue #136):**
Der HW-Entwickler öffnet ein sperrbares Binär (z. B. KiCad) **auch ohne erreichbaren Sperr-Server**.
Ist die Sperre nicht erreichbar, hält die Werkbank lokal eine **Absichts-Sperre** fest (in
`.plm-local/`, der **lokalen, ungeteilten** Ablage via `.git/info/exclude` — E38, nie ein geteilter
Stand, sonst lebte die „Lebenslüge Sperre" weiter), und die Karte zeigt ehrlich „offline bearbeitet,
Sperre nicht bestätigt" — **keine Schein-Sicherheit**. Der reine **Eingang-B-Kern**
(`offlinelock.rs`): `(lokale Absichts-Sperren, Server-Sperren beim Verbinden) → Kollisions-/
keine-Kollisions-Entscheidung`, pur, total, Tabellentest über das Kreuzprodukt. Eine erkannte
**Doppelbearbeitung** („du und Ben habt beide offline an X gearbeitet") fließt in **dieselbe laute
Ausnahme** wie Eingang A — die `Abgleichfrage` — mit Domänensatz inkl. der **Namen** der Beteiligten,
**nie** ein stilles Überschreiben. Eigenschaft (erschöpfend bewiesen): die laute Meldung trägt nie
einen rohen git-/Sperren-Marker. Klebstoff: `offlinelockglue.rs` (Absichts-Sperre aufzeichnen/lesen,
offline-bewusstes Öffnen `acquire_lock_or_intent`, Abgleich beim Verbinden), die Kommandos
`open_lockable_artifact` / `artifact_offline_intent` / `reconcile_offline_locks` und die Karten-Zeile.

---

## E50 — Pfad-Klasse `rekonstruierbar`: Quelle + gepinntes Manifest statt rekonstruierbarem Ballast
**Entscheidung:** Der Baustein bekommt eine **dritte** Pfad-Klasse neben `ignore`/`lfs`:
**`rekonstruierbar`**. Eine git-native Toolchain (`west`, ESP-IDF, PlatformIO, `venv`) zieht beim
ersten Build **tausende rekonstruierbare** Framework-Dateien in den Heimat-Ordner — Dateien, die ein
gepinntes **Manifest** (`west.yml`, `platformio.ini`, `sdkconfig`, eine Lockfile) jederzeit **wieder
erzeugt**. Statt diesen ableitbaren Ballast mitzucommitten, verfolgt der Baustein **nur Quelle +
gepinntes Manifest**: das Framework-Muster wird ignoriert, das Manifest bleibt ausdrücklich verfolgt.
Der Zustand bleibt **reproduzierbar**, das Repo **schlank**.

**`rekonstruierbar` ist nicht `ignore`.** Ignore wirft Müll weg, der nie zurückkommen muss;
Rekonstruierbar wirft *ableitbaren* Ballast weg und **hält das Rezept** — das gepinnte Manifest —
verfolgt, das ihn wiederherstellt. Aus einer Regel wird darum ein Ignore-Muster (das Framework)
**plus** je Manifest eine **Negation** (`!west.yml`), die git das Manifest weiter sehen lässt.
**Handgeänderte** Komponenten dürfen ausdrücklich mitverfolgt werden (auch als Negation gepinnt),
damit lokale Patches nicht verlorengehen.

**Wo die Zeilen leben.** Ausschließlich im **idempotenten Marker-Block** der Dotfiles (E18, keine
Spiegelung) — und zwar im selben `.gitignore`-Block wie die Ignore-Muster, denn beide steuern, **was
git sieht**. Reihenfolge: erst Ignore, dann je Rekonstruierbar-Regel das Framework-Ignore und direkt
darunter die Manifest-Negationen. Beim Stilllegen bleiben sie — wie Ignore/LFS — als **Sediment**
liegen (E17, nie automatisch entfernt). Die *deklarative* Hälfte (die Muster) ist die eine; die
*beobachtende* Hälfte ist das **Nested-`.git`-Grenze-Prädikat** (E50a): der Walk stoppt am genesteten
`.git`, sodass Watcher/Klassifizierer gar nicht erst in den fremden Framework-Baum hineinsehen. Beide
ziehen am selben Strang: kein rekonstruierbarer Ballast im Repo, kein Commit-Sturm aus dem fremden Baum.

**Ehrliche Formulierung.** „Du hast vollständige Ordner" heißt für eine git-native Toolchain **nicht**
„jede Vendored-Datei liegt im Repo", sondern „**Quelle + rekonstruierendes Manifest**" — keine falsche
Vollständigkeit. Darum verlangt die Validierung **beides**: ein Framework-Muster **und** mindestens ein
gepinntes Manifest. Ein Muster ohne Manifest wäre nur ein Ignore und verspräche eine
Wiederherstellbarkeit, die es nicht hat — harter Fehler.

**Warum:** Ein Framework-Baum gehört weder vollständig ins Repo (er bläht es und das LFS-Archiv
dauerhaft) noch blind ignoriert (dann ist der Stand **nicht** reproduzierbar). Die ehrliche Mitte ist:
**das Manifest pinnen, den Rest rekonstruieren**. Das ehrt „behalten, nie umschreiben" (E9) und „lies
zurück statt spiegeln" (E18) — der einzige gespeicherte Stand ist das, was git ohnehin kennt: Quelle
und Manifest.
**Verfeinert:** E18 (dieselbe alleinige Dotfile-Wahrheit, ein dritter Beitrag in denselben Block),
E17 (Rekonstruierbar-Zeilen werden beim Stilllegen zu Sediment), E31 (LFS bleibt für unmergebare
Binärquellen; Rekonstruierbar ist die *andere* Achse — ableitbar vs. unmergebar).
**Baut auf E50a** (#130, das Nested-`.git`-Grenze-Prädikat in `nestedboundary.rs`).
**Umfang jetzt (E50b, Issue #137):** die `rekonstruierbar`-Pfad-Klasse im Baustein-Schema
(`RekonstruierbarRegel`), die abgeleiteten `.gitignore`-Marker-Block-Zeilen (Framework-Ignore +
Manifest-Negation) im selben idempotenten Block wie Ignore (`onboardglue`), das Sediment-Verhalten
(`stilllegen`), die Validierung „Framework + Manifest", der Editor mit ehrlicher Formulierung und die
Tabellen-/Idempotenz-Tests im etablierten Marker-Block-/Classifier-Stil.

---

## E51 — Baustein-Revision + Art, unabhängige Freigabe (Scope = Heimat)
**Entscheidung:** Jeder **Baustein** trägt eine **eigene Revision** und eine **eigene Art**
(Prototyp/Freigabe — E42) mit **Scope = Heimat-Ordner**. Die **Art wandert** von der bisher
**produkt-globalen** Revision auf die **Baustein-Revision**: nicht mehr „das Produkt ist Prototyp/
Freigabe", sondern „`elektronik` ist freigegeben, `firmware` ist noch Prototyp". Eine Baustein-
Freigabe ist **unabhängig** — der HW-Entwickler gibt `elektronik` als „Rev B" frei, **ohne** dass
WIP-Firmware ihn blockiert; jeder Bereich reift für sich.

Eine Baustein-Freigabe setzt einen **dauerhaften Tag** (`freigabe/<heimat>/<label>`), damit ein
**alter Stand** des Bausteins später in eine **Produkt-Revision komponierbar** bleibt — der Tag
zeigt durabel auf genau den freigegebenen Stand, unabhängig davon, wie andere Bausteine danach
weiterlaufen. Zurücknehmen ist reversibel (E22): Heimat-Art zurück auf Prototyp, Tag entfernt.

**Schema-Migration:** Die bestehende `meilensteine`-/`revisionen.json`-Form war eine **flache**,
produkt-globale `version → Art`-Map. Sie bekommt eine **Heimat-Achse** (`heimat → version → Art`),
und alte Dateien werden beim Lesen **transparent** in den produkt-globalen Heimat-Scope migriert —
keine bereits freigegebene Revision verschwindet. Treu zur Degradations-Invariante (E22):
fehlend/leer/kaputt ⇒ leerer Zustand (alles Default Prototyp), nie Fehler.

**Wirkung auf den Block (E42/E19):** Der Aufgaben-Block und das Freigabe-Gate staffeln nun nach der
**Heimat-getragenen** Art statt nach einem produkt-globalen Argument: eine offene Aufgabe blockiert
**nur** den Bereich, der gerade als Freigabe reift. Die reinen Kerne (`aufgabenblock`,
`freigabegate`) bleiben unverändert pur — sie nehmen weiterhin eine Art entgegen; **neu** ist, dass
die Glue-Schicht diese Art aus dem **Baustein-Scope** auflöst.

**Warum:** Hardware, Firmware und Mechanik reifen in unterschiedlichem Tempo. Eine produkt-globale
Strenge erzwang einen künstlichen Gleichschritt (eine fertige Elektronik wartet auf eine halbe
Firmware). Der Scope der Strenge gehört dorthin, wo die Arbeit sitzt — an den **Baustein/Heimat**.
**Verfeinert:** E42 (die Art bleibt, ihr **Träger/Scope** ist jetzt die Baustein-Revision statt die
produkt-globale Revision) und E47 (Revision bleibt der benannte Punkt; die Art ist nun
Heimat-skaliert).
**Umfang jetzt (Issue #131):** Heimat-Achse + Migration in `revisionen.json`; dauerhafter
Baustein-Freigabe-Tag + unabhängige Freigabe/Rücknahme; Baustein-skalierte Block-/Gate-Auflösung in
der Glue; angepasste Tabellen-Tests und Degradations-/Round-Trip-Tests.

---

## E53 — Integrations-Aufgabe: ja/nein-Block an der Compose + passiver Leseschein
**Entscheidung:** Manche Belege gehören **zwischen** zwei Bausteine, nicht in einen. Die Werkbank
führt die **Integrations-Aufgabe** ein: einen **opt-in**, **einmaligen**, **blockierenden**
Cross-Baustein-Test als Forderung zwischen genau zwei Bausteinen. Der HW-Entwickler **flaggt** den
Stand seines Bausteins **gegen** einen anderen (z.B. „PCB braucht FW-Test"), **erhoben gegen eine
Quell-Revision** (den Stand seines Bausteins zum Flagge-Zeitpunkt). Der SW-Entwickler — der
**Empfänger** — **beantwortet** die Forderung mit **ja/nein**, der Beleg liegt im Protokoll.

Ein **„nein"** (oder eine noch **offene**, unbeantwortete Forderung) hält einen **harten Block** —
**aber nur an der Produkt-Compose**, **nie** an der eigenständigen Baustein-/FW-Freigabe. Jeder
Baustein reift für sich (E51a); die Integrations-Strenge sitzt erst dort, wo die Stände
**zusammenkommen**. Ein **„ja"** ist ein **einmal verbrauchter** Beleg (kein Template): er gilt
**genau für die Quell-Revision**, gegen die geflaggt wurde. Wird ein **neuer** Quell-Stand
komponiert, ist der alte Beleg verbraucht — die Forderung muss am neuen Quell-Stand **neu geflaggt**
werden (oder eben nicht). Ein „ja" gegen Rev D deckt nicht Rev E.

An der Compose erscheint zusätzlich ein **passiver Leseschein**: eine abgeleitete Zeile pro
Baustein-Paar (zuletzt belegt getestete Quell-Rev vs. zu komponierende Rev), z.B. „FW zuletzt gegen
PCB Rev D getestet, du nimmst Rev E — kein Test für diese Kombination". Der Leseschein **blockiert
nichts** — er macht nur die bekannte/fehlende Test-Kombination sichtbar.

**Warum:** Die gewöhnliche Aufgabe (E42/#40) ist **Baustein-intern** und staffelt nach der
Heimat-getragenen Art (E51a). Ein Integrationstest ist aber eine Aussage **über ein Paar** und nur
dort sinnvoll streng, wo die Stände komponiert werden — würde er die Einzel-Freigabe blockieren,
zwänge er wieder den künstlichen Gleichschritt, den E51 gerade aufgelöst hat. Die Einmaligkeit fällt
aus der Quell-Rev-Bindung: ein Beleg, der automatisch jeden Folge-Stand mitdeckte, wäre eine stille
Lüge über ungetestete Kombinationen. **Baut auf E51c/E52** (#139, der reine Compose-Kern, aus dessen
Auswahl der Integrations-Block die komponierten Revisionen liest).

**Umfang jetzt (Issue #141):** reiner **Integrations-Block-Kern** (`integrationsblock.rs`, neben dem
Aufgaben-Block-Kern): offene Integrations-Aufgaben × Compose-Auswahl → Block-Entscheid (offen/„nein"
⇒ harter Block) + abgeleitete passive Leseschein-Zeilen, kein I/O, Tabellentest über das Kreuzprodukt
offen/ja/nein × Compose-Auswahl; dünne Glue (`integrationsblockglue.rs`) fürs Flaggen/Beantworten/
Protokollieren im `_plm/integrationen.json`-Speicher und die Block-Auflösung **nur** an der Compose;
Tauri-Kommandos + regenerierte `bindings.ts`.

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

---

## Offene / verschobene Punkte (Strang #128)
- **Auflösen der Abgleichfrage (E49-Folge):** den Eigentumsstreit aktiv entscheiden (Sperre brechen /
  übernehmen / abwarten), analog zum `resolve_sync` des stillen Syncs. Hier nur gemeldet, nicht gelöst.
- Frühere Offenpunkte aus den Sitzungen 5/6 (Glossar/PRD-Kaskade aus E43, Produkt-only-Label E44,
  Werkbank-Status-Filter E45) unverändert.
