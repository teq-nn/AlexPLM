# Entscheidungslog — Werkbank

Stand: 06.06.2026 (7. Sitzung). Diese Sitzung ist eine **Konzept-Grill-Review**: das gebaute v1/v2
und die Entscheidungskette E1–E47 wurden gegen die realen Team-Bedingungen (ein HW-Ingenieur ohne
git, ein SW-Entwickler mit minimalem git) und gegen die git-native Toolchain im Firmware-Baustein
gestresstet. Ergebnis: E48–E56. Mehrere Einträge **verfeinern** das Revisions-Modell (E42/E47) um
eine **Per-Baustein-Ebene** und einen **Produkt-Compose**; E43 wird ausdrücklich **bestätigt** (eine
Gegen-Entscheidung wurde geprüft und verworfen).

---

## E48 — Auftrag des Werkzeugs: das git-Voodoo wegnehmen; die PLM-Schicht ist „ein Ort zum Arbeiten"
**Entscheidung:** Der irreduzible Wert von Werkbank ist **nicht** die PLM-Substantiv-Schicht (Karten,
Graph, Revisionen) — die ist mit git + Konvention reproduzierbar und damit kein Burggraben. Der Wert
ist, dass das **Backend die gefährliche git-Mechanik vollständig übernimmt** für zwei Menschen, die
sie nicht selbst fahren können: der **HW-Entwickler kann kein git** (und wird es nicht lernen), der
**SW-Entwickler** kann minimal und fragt ab „kompliziert" ein LLM. Die PLM-Schicht obendrauf ist
bewusst ein **Kostüm**: sie gibt den Entwicklern „einen Ort zum Arbeiten" und trägt den gedanklichen
Workflow. Sie rechtfertigt sich **nur**, solange das Backend darunter das git-Voodoo trägt.
**Warum:** Klärt die Priorität für alle folgenden Entscheidungen — investiert wird in
Lock/Offline/Recovery und in die verlässliche Backend-Orchestrierung (das kann nur ein Werkzeug),
nicht in PLM-Zierrat. Macht aus Q11 der Grill-Review einen Leitsatz.
**Verhältnis:** Schärft E1 (Werkzeug, kein Produkt) um das reale Können der zwei Personen; trägt E43
(s. E55) und die Recovery-Etage (E56).

---

## E49 — Detect-and-Reconcile statt „einziger Fahrer"; drei ehrliche Wahrheits-Orte; optimistisches Offline-Lock
**Entscheidung:** Die bisher **unausgesprochene** Annahme „das Werkzeug ist der einzige Orchestrator
von git + Sperren" wird aufgegeben und durch **Detect-and-Reconcile** als erstklassiges Konzept
ersetzt:
- **Reconciliation beim Öffnen:** realer git-Zustand (HEAD, Refs, `.gitattributes`-Hash, lokaler
  Lock-Cache vs. Server-Sperren) wird gegen das verglichen, was das `_plm` zuletzt gesehen hat.
  Divergenz (Terminal-Aktion des SW-Devs, externes `west`-Hantieren) → **ruhige, domänensprachliche**
  Meldung („hier wurde außerhalb des Werkzeugs gearbeitet — ich gleiche ab"), nie roher git-Text.
- **Drei Wahrheits-Orte werden ehrlich benannt:** **Platte = Inhalt**, **git = History**,
  **Server-Sperren = Koordination** — und die Lock-Wahrheit ist **flüchtig** (stirbt mit dem Server).
  Das alte „nur die Platte ist die Wahrheit" wird dadurch nicht falsch, aber präzisiert.
- **Optimistisches Offline-Lock:** Ist der Lock-Server beim Öffnen einer Binärdatei nicht erreichbar
  (Zug, Server down), notiert das Werkzeug lokal eine **Absichts-Sperre** und gleicht sie beim
  nächsten Verbinden gegen die Server-Sperren ab. Kollision → die schon gebaute **laute Ausnahme**
  (`LauteAusnahme.svelte`), formuliert als „du und Ben habt beide offline an X gearbeitet". Solange
  ungesichert, zeigt die Karte sichtbar **„offline bearbeitet, Sperre nicht bestätigt"** statt still
  „alles gut".
**Warum:** Der HW-Dev wird KiCad offline/bei Server-Ausfall öffnen — genau die Bedingung, unter der
die Binär-Invariante (E35) heute still bricht und keiner der zwei Menschen per git retten kann.
**Verfeinert:** E35 (Binär-Invariante hält jetzt auch offline durch Reconcile statt durch Annahme);
ergänzt die laute Ausnahme (E41) um den Offline-Fall.

---

## E50 — Baustein bekommt die Pfad-Klasse „rekonstruierbar"; genestetes git ist eine opake Grenze
**Entscheidung:** Die git-native Toolchain im Firmware-/Software-Baustein (Python-`venv`+`pip`,
ESP-IDF/`west`-Module, PlatformIO) kollidiert strukturell mit Werkbanks eigener git-Schicht. Drei
Festlegungen:
- **Dritte Pfad-Klasse `rekonstruierbar`** im Baustein, neben `ignore` und `lfs`: Pfade, die
  **bewusst nicht getrackt, aber aus einem committeten Manifest wiederherstellbar** sind. Der
  Firmware-Baustein committet **nur Quelle + gepinntes Manifest** (`west.yml`, `platformio.ini`,
  `sdkconfig`, Lockfile mit exakten Commits; bei Python `venv` + `pip install` aus Manifest), **nicht**
  die Vendored-Frameworks/Components. Höchstens **händisch veränderte** Components werden getrackt.
- **Genestetes `.git`/Submodul = opake, ignorierte Grenze (v1):** Watcher, Klassifizierer und
  Projektion stoppen an einem `.git` *unterhalb* der Produktwurzel — kein Hineinlaufen, kein
  Auto-Commit über fremde Bäume. **Keine** Submodul-*Unterstützung* in v1 (bewusst, dokumentiert).
  Submodule werden auch sonst vermieden („in normalen Projekten schon kompliziert").
- **Reproduzierbarkeits-Versprechen ehrlich nachjustiert:** „lösch das Werkzeug, du hast saubere,
  *vollständige* Ordner" gilt für git-native Toolchains als „saubere Quelle **+ ein Manifest, das den
  Rest rekonstruiert**" — eine andere, ehrlich benannte Aussage als „eingefrorener Vollstand".
**Warum:** Heute filtert `watcher.rs` nur das Top-Level-`.git`; ein einziges `west update` löst eine
Commit-Lawine über fremden Code aus, und `projection.rs` liest `.git/` von Hand und setzt ein flaches
Repo voraus. Ungetestet, explodiert beim ersten echten Firmware-Produkt.
**Offen (Folge-Bau):** Watcher/Klassifizierer/Projektion nested-git-aware machen; `rekonstruierbar`
ins Baustein-Schema + Default-Bausteine für Zephyr/ESP-IDF/PlatformIO/Python.
**Verfeinert:** E16 (Baustein-Definition), E18 (Dotfiles als alleinige Wahrheit bleibt).

---

## E51 — Per-Baustein-Revision (+ eigene Art); Produkt-Revision als synthetischer Compose-Commit; Gate baustein-scoped
**Entscheidung:** Versionierung bekommt eine **Per-Baustein-Ebene** unter der Produktebene:
- **Baustein-Revision:** Jeder Baustein/Arbeitsbereich trägt seine **eigene** benannte Revision
  (`elektronik → Rev B`) **und eigene Art** (Prototyp/Freigabe, E42), unabhängig gebumpt. Welcher
  Baustein sich änderte, ist per **git-diff auf den Heimat-Ordner** ableitbar. Eine
  **Baustein-Freigabe mintet einen dauerhaften git-Tag** (`firmware/v1.1`) — damit ein alter Stand
  später komponierbar bleibt.
- **Produkt-Revision = synthetischer Compose-Commit:** Eine produkt-globale Version bleibt jederzeit
  möglich, ist aber ein **konstruierter** Commit (git-Plumbing `read-tree`/`commit-tree`, mehrere
  Eltern), der pro Baustein **einen gewählten Release-Tag** zu **einem reproduzierbaren whole-tree-
  Schnappschuss** zusammenfügt. Der Baum **stimmt physisch mit dem BOM überein** (z.B. neue PCB Rev B
  + alte FW 1.1), **ohne Submodule**. Ist ein Baustein WIP, wird sein **voriger** Release-Tag
  geschippt („übernommen", §7.1) oder rechtzeitig aktualisiert — beides derselbe Compose-Mechanismus.
  WIP eines anderen Bausteins bleibt unberührt auf der Arbeitslinie.
- **Freigabe-Gate baustein-scoped:** Der Vollständigkeits-/Block-Check (`freigabegate.rs`) schärft auf
  die **geänderten Arbeitsbereiche**. Eine PCB-Freigabe prüft nur `elektronik/`s Waisen/Pflicht/Tasks;
  Firmware-WIP **blockiert sie nie**. Heute ist der Gate produkt-global flach (`waisen`/
  `fehlende_pflicht` ohne Heimat-Schärfung) — das verbaut paralleles HW/SW-Arbeiten und wird geändert.
**Warum:** Ein git-Tag ist ein ganzer Baum; „neue PCB + alte FW" als *ein* Tag auf HEAD kann das
physisch nicht (HEAD trägt die WIP-FW). Der Compose ist der einzige ehrliche Weg, der whole-tree-
Reproduzierbarkeit **und** unabhängige Baustein-Kadenz hält, ohne Submodule. „Baum = HEAD taggen, im
Manifest lügen" wurde verworfen (Baum ≠ BOM, „als Ordner öffnen" gäbe WIP).
**Verfeinert:** E42 (Art wandert von der produkt-globalen Revision auf die **Baustein**-Revision),
E47 (zur „Revision" tritt die **Produkt-Revision/Compose**); E2/E8 (git-Tag als Motor bleibt). Der
Multi-Parent-Graph (`graph.rs`, „zwei bei einem Merge") trägt Compose-Knoten bereits.

---

## E52 — Globale Release-Zusammenstellung: jeder Pflicht-Baustein ≥1 Release; Cold-Start-Seeding; mehrparteiig ohne Rollen
**Entscheidung:**
- **Jeder *Pflicht*-Baustein braucht ≥1 Release**, um eine Produkt-Revision zu bauen. **Optionale**
  Bausteine binden **nicht** (sonst hält ein nie-gefüllter Doku-Baustein jedes Produkt-Release als
  Geisel) — konsistent mit der Pflicht/Optional-Trennung (E11, `freigabegate.rs`).
- **Cold-Start:** Das **allererste** globale Release ist **ein** Akt, der pro Pflicht-Baustein
  automatisch eine **initiale Revision aus dem Ist-Stand** seedet — kein N-faches Hand-Release vorab.
- **Mehrparteiige Zusammenstellung ohne Autorisierung:** Die Produkt-Revision ist erst fertig, wenn
  jeder Pflicht-Baustein eine Entscheidung beigesteuert hat (frischer Tag **oder** explizites
  „voriges schippen"). Modelliert als **Checkliste/Assembly-Zustand** („Rev 5 in Zusammenstellung:
  elektronik ✓ Rev B · firmware ⧖ ausstehend") — **keine Rollen/Rechte**, kein Baustein-Eigentümer
  als Autorisierung. E31 (Sperre = Koordination, nicht Autorisierung) und §30 (keine mehrstufigen
  Freigabeprozesse) bleiben gewahrt.
**Warum:** Hält die produkt-globale Version möglich (E51) und macht ihre Zusammenstellung explizit,
ohne das MVP um eine Rechte-Schicht zu erweitern.
**Verfeinert:** E11, E19; baut auf E51.

---

## E53 — Baustein-übergreifende Integrations-Tests: opt-in Aufgabe, rev-spezifisch, einmalig, mit passivem Leseschein
**Entscheidung:** Die Gefahr **nie zusammen getesteter Kombinationen** (Compose von {PCB Rev D, FW 1.1},
wo FW 1.1 nur gegen Rev A getestet war) wird über die **bestehende Aufgaben-Maschinerie** adressiert —
**manuell und opt-in**, kein automatischer Stale-Check über alle Bausteine:
- Der HW-Dev **flaggt** eine **blockierende Aufgabe** („needs fw test"), verknüpft via
  `TaskLink::Arbeitsbereich` mit dem anderen Baustein, **gegen eine bestimmte Quell-Rev** erhoben.
- Der **Empfänger** (verantwortlicher Bereich der Ziel-Bausteins) antwortet **ja/nein**. **„Ja"** löst
  den Block; **„nein" hält den harten Block** — ein bewusst geforderter, negativ zurückgekommener Test
  darf nicht still durchschippen.
- Der Block greift **nur am globalen Compose**, **nicht** am eigenständigen Release des Ziel-Bausteins
  (der SW-Dev muss FW 1.2 aus anderen Gründen freigeben können). Fügt sich in das baustein-scoped Gate
  (E51) ein.
- **Einmalig, kein Auto-Re-Arm:** Beantwortet → der Beweis ist **verbraucht** (wie eine erledigte
  Aufgabe, **keine Vorlage**). Bei der nächsten Quell-Rev **flaggt der Mensch neu — oder nicht, falls
  unnötig**. Die Maschine re-blockt **nicht** automatisch.
- **Passiver Leseschein am Compose** (blockiert nichts, aus den Akten + BOM abgeleitet): *„FW zuletzt
  gegen PCB Rev D getestet, du nimmst Rev E — für diese Kombination liegt kein Test vor."* Tötet das
  „vergessen neu zu flaggen", ohne zu zwingen — doc-treu zu E26/E30 („sag nur, was du weißt").
**Warum:** Opt-in vermeidet Fehlalarm-Rauschen; die Wiederverwendung von `TaskKind::Aufgabe`
(block-fähig) + `Haerte::Hart` macht es ~90 % geschenkt. Bewusste Restgefahr (rein manuell ⇒ nur
gefangen, wenn jemand flaggt) wird durch den passiven Leseschein abgefedert, nicht durch
Maschinen-Zwang.
**Verfeinert:** E14/E15 (Aufgaben), E19 (Block-Härten); baut auf E51/E52.

---

## E54 — `_plm`: eine Datei pro Eintrag statt einer großen Array-Datei
**Entscheidung:** Jeder `_plm`-Belang (Aufgaben, Release-Pointer, Kanten, Zuordnungen …) wird als
**eine Datei pro Eintrag** gespeichert (eine Aufgabe = eine Datei, nach stabiler ID benannt), **nicht**
als eine große Array-Datei. Zwei gleichzeitig angelegte Aufgaben = zwei **neue** Dateien → git merged
sauber, **nie ein Konfliktmarker**.
**Warum:** `_plm` ist committet und geteilt (ADR 0002); heute eine JSON-Datei pro Belang. Zwei Leute,
die gleichzeitig je eine Aufgabe anlegen, schreiben die **ganze** `tasks.json` neu → git-Merge-Konflikt
auf der eigenen Koordinationsdatei — und Konfliktmarker-Auflösung ist genau die „gefährliche Mechanik",
die E43 versteckt. Die diese Sitzung dazugekommenen `_plm`-Schreiber (Release-Pointer E51, Integrations-
Tasks E53, Compose-BOM) vergrößern die Konfliktfläche. Pro-Eintrag-Dateien passen zudem zu „Dateisystem
ist die Wahrheit".
**Verfeinert:** ADR 0002 (`plmstore.rs`-Skelett bleibt, Granularität wechselt auf pro-Eintrag).

---

## E55 — Git-Sichtbarkeit (E43) bestätigt: Sichtbarkeit ≠ Bedienbarkeit; Per-Persona-Verstecken verworfen
**Entscheidung:** E43 **steht**. Einfaches git-Vokabular (Commit, Branch, Tag, Push, Merge, Pull,
Graph) bleibt **sichtbar** — auch für den HW-Dev. Eine in dieser Sitzung erwogene Gegen-Entscheidung
(git-Substantive **pro Person** verstecken, weil der HW-Dev keine CLI kann) wurde **geprüft und
verworfen**. Die tragende Unterscheidung:
- **Sehen** (git-Substantive sichtbar): Ein **Ingenieur** profitiert vom ehrlichen Wort; „kann keine
  CLI" heißt **nicht** „darf das Wort nicht sehen".
- **Bedienen/Retten** (rebase/reset/Konfliktmarker/Lock-Plumberei/Compose-Bau): macht **ausschließlich
  das Backend**.

Die Verwirrung, vor der E43 warnte, sind **erfundene Synonyme** („Stand" statt „Commit") — für einen
Ingenieur ist das echte Wort klarer. Das schärft E48: man **sieht** ehrliche git-Substantive und
**führt** nie die gefährlichen aus.
**Warum:** Korrigiert den Fehlschluss „kein CLI ⇒ git verstecken". Visibilität und Bedienbarkeit sind
orthogonal.
**Bestätigt:** E43, E6. **Verwirft:** den Per-Persona-Versteck-Vorschlag dieser Sitzung.

---

## E56 — Recovery-Etage: jede gefährliche Operation als Transaktion + „Export als einfache Ordner"
**Entscheidung:** Weil das Backend alle gefährliche git-Mechanik trägt (E48) und **keiner der zwei
Menschen per CLI retten kann**, gibt es eine garantierte Auffang-Etage:
1. **Transaktion mit Vorab-Snapshot:** Jede gefährliche Operation (Compose-Bau, Lock-Plumberei,
   Tag-Schieben, jeder Eingriff, der bei Stromausfall/Fehler einen Halbstand hinterlassen könnte)
   läuft hinter einer **Snapshot-Ref**, die das Backend vorher setzt. Bei jedem Fehler **automatischer
   Rollback** auf den letzten guten Stand; der Mensch sieht „ist schiefgegangen, ich hab's
   zurückgedreht", **nie** ein kaputtes Repo.
2. **„Export als einfache Ordner" (Knopf):** Materialisiert aktuellen + getaggte Stände als **reine
   Ordner** auf der Platte — der Rückfall auf das eigene Fundament („lösch das Werkzeug, du hast
   saubere Ordner"). Letzte Rettung, wenn selbst das Backend klemmt.
**Warum:** `west`-Halbstände, scheiternde LFS-Smudges, Stromausfall im Compose — diese Zustände
**werden** auftreten; ohne Auffang-Etage ist das Zweierteam handlungsunfähig.
**Verfeinert:** E48 (operationalisiert den „Backend trägt das Voodoo"-Auftrag); ergänzt E3 (Worktree/
Export on demand) um den Vollexport als Notausgang.

---

## Offene / verschobene Punkte (Stand 7. Sitzung)
- **Folge-Bau aus E50:** Watcher/Klassifizierer/Projektion nested-git-aware; `rekonstruierbar` ins
  Baustein-Schema; Default-Bausteine Zephyr/ESP-IDF/PlatformIO/Python.
- **Folge-Bau aus E51:** Compose-Commit-Mechanik (`read-tree`/`commit-tree`), Baustein-Revisions-Tags,
  baustein-scoped Gate-Eingang; Graph-Darstellung der Compose-Knoten.
- **Folge-Bau aus E54:** `plmstore.rs` von Array- auf Pro-Eintrag-Granularität umstellen (Migration).
- **Identifier-Nachzug:** Rust-Bezeichner und Doppel-Komponenten (`ArtefaktKarte` vs. `ArtifactCard`),
  die fünf Glossar-Versionen — Hygiene aus früheren Umbenennungen (E47), weiterhin offen.
- Frühere Offenpunkte unverändert: Auslagern/Archiv (v1-fern, E36), Bruch fremder Sperren (E31),
  Kanten-Heuristik (E21), Windows-Prüfung (ADR 0001), Glossar/PRD-Kaskade aus E43/E47.
