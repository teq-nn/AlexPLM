# v1-PRD ↔ vollständige Design-Docs (E1–E41) — was die PRD nicht sah, was v2 braucht

Stand: 31.05.2026. Diese Analyse ist der **Gegenpol** zu `v2-gap-analyse.md`: jene vergleicht den
*gebauten* Stand gegen die Docs; **diese hier vergleicht die v1-PRD (Issue #1) gegen die jetzt
vollständige Entscheidungskette E1–E41** und benennt, **was als Konzept in der PRD fehlt oder auf
eine einzige Wegwerf-User-Story zusammengeschrumpft ist**.

## 0. Wurzelursache — warum die Lücke existiert

Die PRD sagt es selbst im Schlussabsatz: sie ist die **Synthese der 5. Sitzung** —
`entscheidungslog-4.md` (E34–E41), `glossar-4.md`, `ui-stilbeschreibung.md`. E1–E33 und das
ursprüngliche `plm_software_konzept.md` **lagen beim Schreiben nicht vor**. Sitzung 5 ist die
**Mehrbenutzer-/Sync-/Sperren-/Import-/Archiv-Schicht** — und genau die trägt die PRD sauber.

Alles, was **davor** gegrillt wurde — das **Baustein-/Stack-/Bibliothek-Modell** (das
Organisationsprinzip des ganzen Werkzeugs), die **Aufgaben** (Task/Hinweis), der **dreistufige
Freigabe-Block**, die **UI-Wirbelsäule** (Werkbank vs. Graph-Raum, Navigation), die
**produktübergreifende Suche**, die **Waisen/Vollständigkeit** — fehlt in der PRD oder ist auf ein,
zwei Sätze reduziert. Das ist keine kleine Politur: es ist die halbe Domäne.

> **Zweite, tiefere Lücke:** Das `plm_software_konzept.md`, auf das **jede** Entscheidung per
> §-Nummer verweist (§3.2, §6, §12, §13, §18.2, §22, §23–§26, §28, §30 …), ist **nie ins Repo
> committet worden** (in der gesamten Historie nicht vorhanden). Die meisten §-Stellen sind durch
> E-Entscheidungen überholt, aber einige noch lebende (Öffnen-Aktion/Tool-Start §13, Haupt-/
> Zusatzdateien §12, Layout-Details §23.2, Suche/Tags §26) sind nur teilweise in den E-Logs
> aufgefangen. **Empfehlung: dieses Dokument auffinden und committen** (oder die noch lebenden
> §-Verweise rekonstruieren), bevor die unten genannten Flächen gebaut werden.

Was die PRD **gut** abdeckt (nicht erneut verhandeln): die sechs reinen Kerne, die Binär-Invariante,
die zwei Push-Arten, stiller Sync + laute Ausnahme, Import-Gate, Auslagern, die UI-Design-Tokens.

---

## 1. Die fehlenden Domänen-Säulen (in E1–E33, in der PRD abwesend)

Jede Säule: **was die Docs verlangen** · **was die PRD hat** · **was v2 braucht** (Modul + User
Stories + Datenmodell).

### 1.1 Baustein-/Stack-/Bibliothek-Modell — *die größte Lücke* (E10, E16, E17, E18, E20, E24, E25)
- **Docs:** Das **Atom** ist der **Baustein** — ein wiederverwendbares Bündel pro Tool:
  Heimat-Arbeitsbereich + Artefakt-Globs + Ignore-Presets + LFS-Muster + Öffnen-Aktion +
  Startaufgaben + interne Abgeleitet-von-Kanten (E16, `glossar-2`). **Standard-Toolstack** (geteilt,
  in der **Bibliothek**) → beim Produktanlegen als **Schnappschuss** kopiert zum **Produkt-Stack**
  (lebend, im `_plm`). **Anti-Drift:** Stack ist Kopie, keine Live-Abhängigkeit (E16). **Onboarding**
  hängt Ignore-/LFS-Zeilen **idempotent** in **Marker-Blöcke** (`# >>> baustein: zephyr >>>`) der
  Dotfiles; Hand-Edits gewinnen, alles außerhalb der Marker bleibt unangetastet (E18). **Lebenszyklus:**
  Erweitern (additiv) / Austauschen (= Erweitern + **Stilllegen**); Stilllegen ist **label-only** —
  alte Globs greifen nicht mehr → Dateien werden **Waisen**, Ignore/LFS bleiben als **Sediment**
  liegen (E17). **Tag-1-Pflicht pro Baustein-Onboarding** (`.gitattributes`/`.gitignore` stehen,
  *bevor* das Tool die erste Binärdatei erzeugt, E4/E17).
- **PRD:** Genau **eine** US (#9 „Blatt-Ordner werden als Bausteine erkannt") + #10 (Pfade sichtbar).
  **Kein Modul**, keine Bibliothek, kein Standard-/Produkt-Stack, kein Onboarding, kein Stilllegen/
  Sediment, keine Pattern→Glob-Zuordnung. Im Bau ist „Neues Produkt" nur Ordner-Import.
- **v2 braucht:**
  - **Modul `Stack/Baustein` (deep module):** reine Funktionen für (a) Onboarding — gegeben
    Baustein-Definition + aktuelle Dotfiles → idempotenter Marker-Block-Diff; (b) Stilllegen →
    welche Globs erlöschen, welche Pfade werden Waisen, welches Sediment bleibt; (c)
    Pattern-Zuordnung — Pfad + Glob-Satz → Artefaktname | Waise. Testbar ohne Repo.
  - **Bibliothek-Speicher** (geteilte Standard-Toolstacks + Einzel-Bausteine) und **Produkt-Stack
    im `_plm`** (Kopie). Mitgelieferte Default-Bausteine: KiCad→`elektronik/`, Fusion→`mechanik/`,
    Zephyr/PlatformIO→`firmware/`, Doku (output-förmig).
  - **User Stories (neu):** Standard-Toolstack wählen/erweitern beim Anlegen; Baustein dazu
    (erweitern); Tool austauschen (PlatformIO→Zephyr) mit Sediment/Waisen-Verhalten;
    Bibliotheks-Änderung berührt laufende Produkte **nie**.
  - **Datenmodell:** `_plm` besitzt nur, was Git nicht kennt — Zuordnung Artefaktname↔Glob, Heimat,
    Öffnen-Aktion, Startaufgaben, Hand-/Paar-Kanten. Ignore/LFS leben **ausschließlich** in den
    Dotfiles (E18, keine Spiegelung).

### 1.2 Aufgaben: Task vs. Hinweis + Branch-Strenge (E14, E15)
- **Docs:** **Task** = verpflichtend, *kann* blockieren, hängt an Artefakt/Version/Branch. **Hinweis**
  = blockiert **nie**. Trennende Eigenschaft ist die **Blockier-Fähigkeit**, nicht die Wichtigkeit.
  **Strenge ist Eigenschaft des Branch-Typs** (Prototyp lasch / Production streng); eine Aufgabe
  **erbt** sie und greift an **jedem Übergang nach oben** (Tag setzen *und* Merge nach Production).
  **Opt-out pro Task** („blockiert überall"). Modell-Minimum: Titel, Status, Typ, optionale
  Verknüpfung, Fälligkeit (kein Kanban/Priorität).
- **PRD:** **Nichts.** Keine einzige Aufgaben-User-Story; „Task" taucht nur in der Substantiv-Liste
  von E33 auf.
- **v2 braucht:** **Modul `Aufgaben`** (reine Block-Entscheidung: Aufgabenmenge × Branch-Strenge ×
  Checkpoint-Art → blockiert/blockiert-nicht). User-Story-Block „Aufgaben & Hinweise". `_plm`
  speichert die Aufgaben (Git kennt sie nicht).
  - ⚠️ **Reconciliation (offen):** E15 ruht auf **Branch-Typen** (Prototyp/Production); E34 setzt
    aber „**beide auf `main`**, kein personengebundener Branch". Im `main`-Alltag ist unklar, was ein
    „strenger Branch" ist. **Muss neu gegrillt werden:** Strenge an **Meilenstein-Akt** statt an
    Branch-Typ? Siehe 3.1.

### 1.3 Dreistufiger Freigabe-Block + Meilenstein-Dialog (E19, E28, E32)
- **Docs:** Beim Meilenstein/Tag wird nach Härte gestaffelt: **(1) Warnung** (Stale-Kante, blockiert
  nie) · **(2) weicher Block** (Waise/fehlendes Pflicht-Artefakt — per protokollierter Begründung
  überwindbar) · **(3) harter Block** (offener blockierender Task auf strengem Branch — nur durch
  Erledigen/Verwerfen/Herabstufen des Tasks selbst). **Ein** Dialog, **ein kontextabhängiger Knopf**
  (Beschriftung/Schärfe wechseln statt drei Knöpfe, E28). Mehrbenutzer: Block greift **personen-
  übergreifend**, plus **personenübergreifende Warnung** „du taggst auch X' frischen Stand mit" (E32).
  `VERSION_NOTES.md` ist **Ergebnis** des Tags, nie Vorbedingung.
- **PRD:** Meilenstein = bewusster Tag + `VERSION_NOTES.md` (US #6–#8). **Vollständigkeits-Check,
  drei Stufen, der wandernde Knopf, die Personen-Warnung — alle abwesend.** Die „laute Ausnahme" der
  PRD ist **nur** der *Sync*-Konflikt, **nicht** das Freigabe-Gating.
- **v2 braucht:** **Modul `Freigabe-Gate` (deep module):** offene-Punkte-Menge → sortierte Liste +
  Knopf-Zustand {taggen | trotzdem-freigeben+Begründung | gesperrt-durch-Task}. Hängt an 1.2 (Tasks)
  und 1.4 (Waisen). User-Story-Block „Meilenstein-Freigabe". E32-Personenwarnung verbindet sich mit
  dem schon gebauten Fremd-Sperren-/`git lfs locks`-Lesen.

### 1.4 Unzugeordnet-Fach / Waisen / Vollständigkeit (E11, glossar-1)
- **Docs:** Eine unzugeordnete Datei wird **getrackt und bleibt physisch liegen**, nur das *Label*
  fehlt → **Waise**. **Unzugeordnet-Fach pro Arbeitsbereich** (nie global), Kontext-Vorschlag aus
  Ordner-Geschwistern. Im Alltag passiv; der **Meilenstein löst den Vollständigkeits-Check** aus →
  jede Waise verhindert „technisch vollständig" (Freigabe per weichem Block trotzdem möglich). Regel:
  **alles getrackt, außer explizit ignoriert.**
- **PRD:** Nicht erwähnt (außer implizit über die Pflicht-Artefakt-Idee).
- **v2 braucht:** Waisen-Erkennung als Teil des Pattern-Zuordnungs-Moduls (1.1); Unzugeordnet-Fach
  in der Werkbank-UI (1.6); Speisung des weichen Blocks (1.3). User-Story „Waise sehen/zuordnen".

### 1.5 Abgeleiteter Artefakt-Status + `_plm`/`version.json`-Datenmodell (E8, E18, E26)
- **Docs:** Karten-Status wird **live abgeleitet**: *aus Git* (Vorhanden/Geändert/fehlt/Übernommen/
  Ignoriert), *aus Kanten* (Stale-Warnung), und nur **drei echte PLM-Fakten** im `_plm` (Pflicht
  ja/nein, Optional/nicht-benötigt, Freigegeben). `version.json` führt **keine** Abstammung und
  **keinen** `status:"changed"` (E8/E26 — Git kennt das). `VERSION_NOTES.md` = menschenlesbare
  Spiegelung des Tag-Textes, entsteht **am Meilenstein**.
- **PRD:** „Graph Projection" projiziert Stände/Meilensteine/Kanten/Stale/ausgelagert — aber der
  **Karten-Status** und der **präzise `_plm`-Inhalt** (was gespeichert wird vs. abgeleitet) sind
  nicht spezifiziert.
- **v2 braucht:** Status-Ableitung in `Graph Projection`/`Status Reader` erweitern um die
  Karten-Status; verbindliche `_plm`-Schema-Definition (genau die drei Fakten + 1.1-Felder + Tasks).

### 1.6 UI-Wirbelsäule: Werkbank vs. Graph-Raum, Navigation (E23, E24, E25)
- **Docs:** Zwei **gleichwertige, getrennte Räume**: **Werkbank** (Vorderseite, aktueller
  Arbeitszustand als **Artefakt-Karten je Arbeitsbereich**) und **Graph-Raum** (History, separat,
  *aufgesucht*, nicht Startbildschirm). **Linke Navigation spiegelt den echten Ordnerbaum**
  (Eltern-Ordner = regellose Gruppen, Bausteine nur in Blatt-Ordnern). **Oberste Navigation:
  Produkte · Bibliothek · Einstellungen** (kein „Aufgaben"-Top-Level; Stack lebt *im* Produkt).
- **PRD:** Der UI-Abschnitt ist **ausschließlich** Farb-Tokens, Typo, Komponenten-Regeln. **Keine
  Screen-Architektur, keine Navigation, keine Werkbank-/Graph-Trennung.** Der Bau hat eine einzelne
  `+page.svelte`.
- **v2 braucht:** Verbindliche **UI-Struktur** in der PRD: Top-Nav (3 Punkte), Werkbank mit
  Ordnerbaum-Nav + Artefakt-Karten + Unzugeordnet-Fach + Stack-Verwaltungsbereich, separater
  Graph-Raum. User-Story-Block „Navigation & Räume".

### 1.7 Graph-Klick: drei Verben + Materialisieren on demand (E3, E27)
- **Docs:** Klick auf einen alten Knoten **verschiebt nie still die Werkbank**. Drei Verben: **Als
  Ordner öffnen** (Default — schreibgeschützter Worktree/Export *daneben*) · **Von hier abzweigen**
  (bewusster neuer Branch, sichert vorher ins Netz) · **Zurückwerfen** (destruktiv, hinter
  „Historie anfassen"-Gate, nie Default). Nackter `checkout`/`reset` taucht nie im Happy Path auf.
  Materialisieren on demand statt „alle Versionen als Ordner" (E3).
- **PRD:** Graph Projection ist read-only; **die Interaktions-Verben und die Worktree-
  Materialisierung fehlen.** Der gebaute Versionsbaum-SVG (#28) zeigt, aber kann nichts dahinter.
- **v2 braucht:** Vault-Operationen „als Ordner öffnen (Worktree)", „von hier abzweigen",
  „zurückwerfen (hinter Gate)". User-Story-Block „Alten Stand inspizieren/abzweigen".

### 1.8 Produktübergreifende Suche / Live-Fan-out / Produkt-Registry (E30)
- **Docs:** Bei >10 Produkten eine produktübergreifende Such-Zeile als **Live-Fan-out** (Registry
  ablaufen, jedes *erreichbare* Repo öffnen, live über Dateinamen/`_plm`/`VERSION_NOTES.md` grepen).
  **Kein Index, kein Mirror.** Nicht erreichbare Produkte ehrlich melden. **Produkt-Registry** =
  schlanke Pfadliste (keine Inhalte), versorgt Produktliste + Fan-out.
- **PRD:** Nicht erwähnt.
- **v2 braucht:** **Produkt-Registry** (überlebender Rest der Workspace-Config §9) + Such-Fan-out.
  User-Story „über alle Produkte suchen, offline ehrlich melden".

### 1.9 Allgemeine Merge-/Kollisions-UX (E7)
- **Docs:** Echte Kollision (Datei beidseitig geändert) → Tool **stoppt**, zeigt Kollisionen im
  **Klartext** (Datei, Datum, Größe, ggf. Vorschau), Nutzer wählt **pro Datei** — **kein Git-Wort**.
- **PRD:** Der `Sync Decider` deckt die *Mehrbenutzer*-laute-Ausnahme ab (silent-merge vs.
  loud-exception). Die **pro-Datei-Klartext-Auflösung** von E7 ist der allgemeinere Unterbau — und
  genau die unbebaute Stelle (`resolve_sync`), die `v2-gap-analyse.md` §2.1 als höchste Priorität
  führt. **Hier treffen sich beide Analysen.**
- **v2 braucht:** die in `v2-gap-analyse.md` §2.1 geforderte Auflösung — jetzt erweitert um E7s
  pro-Datei-Wahl als allgemeines Muster (nicht nur mein/Bens-Gesamtstand).

---

## 2. Begriffe, die in der PRD fehlen (sollten ins PRD-Glossar/Out-of-Scope)

Aus `glossar-1`/`-2`/`-3`, von der PRD nicht aufgegriffen: **Werkzeug vs. Tool/Software vs. Bauteil**
(Dreifach-Begriffstrennung, E16) · **Baustein/Standard-Toolstack/Produkt-Stack/Sediment/Marker-Block**
· **Task/Hinweis/Branch-Strenge** · **Waise/Unzugeordnet-Fach** · **Technisch-vollständig vs.
Freigegeben** · **Werkbank vs. Graph-Raum** · **Wohin vs. Wie** · **drei Sorten Kopie / Branch vs.
Worktree** · **Produkt-Registry/Live-Fan-out**. Diese Begriffe tragen die obigen Säulen und sollten
in eine v2-PRD übernommen werden.

---

## 3. Echte Reconciliations (nicht nur Hinzufügen — hier widersprechen sich Sitzungen)

### 3.1 Branch-Strenge (E15) ⟂ „beide auf `main`" (E34) — *muss gegrillt werden*
E15s Aufgaben-Strenge ist an **Branch-Typen** (Prototyp lasch / Production streng) verankert; E34/E32
verlegen den Alltag auf **ein gemeinsames `main`** ohne personengebundene Branches und machen den
**Meilenstein** zum strengen Akt. Im `main`-Modell gibt es kein „Production-Branch" mehr als Träger
der Strenge. **Offene Frage für v2:** Strenge an den **Freigabe-Akt** (E28/E32) statt an den
Branch-Typ hängen; „Prototyp vs. Production" wird zur Eigenschaft des **Meilensteins/Tags**, nicht
des Branches. Branches bleiben nur noch bewusste Varianten (E27). Das ist eine bewusste Entscheidung,
kein reines Übernehmen.

### 3.2 „Git darf durchscheinen" (E6) ⟂ „git nie sichtbar" (E33/E39/E41)
E6 (Sitzung 2): Gits **Denkmodell** — Commit, Branch, Tag, History-Graph, Abstammung — **darf**
sichtbar sein; nur das gefährliche „Wie" (stash/reset/rebase/migrate) wird versteckt. Sitzung 4/5
(E33/E39/E41) verschärft: **keine git-Substantive** (push/pull/commit/merge) im Alltag. Die PRD
übernimmt die **strenge** Linie — korrekt für den *täglichen* Fluss (E41: einen täglichen Reflex
versteckt man nicht nachträglich). **Aber E6s Nuance darf nicht verloren gehen:** die *Konzepte*
Graph/Meilenstein/Variante/Abstammung **bleiben sichtbar** — in **Domänensprache** (Stand,
Meilenstein, Variante, „abgeleitet von"), nur die git-**Wörter und -Beschwörungen** verschwinden.
Sonst „überversteckt" v2 (z. B. Varianten-Branches aus E27/E32 unsichtbar machen). **In die v2-PRD
als ausdrückliche Leitregel aufnehmen.**

### 3.3 Kanten dreistufig (E20) ⟂ „v1 rein manuell" (E40)
Die PRD wählt für v1 **korrekt** E40 (nur Hand-Kanten). **Aber** die **Baustein-Default-** und
**Baustein-Paar-Default-Kanten** (E20) sind Teil des Baustein-Modells (1.1). **Sobald Bausteine in
v2 kommen, kommen diese Kanten-Herkünfte mit** — die `Edge Logic` muss dann Default-/Paar-Kanten aus
dem Onboarding entgegennehmen. Heute baut `edges.rs` nur Hand-Kanten; das ist v1-richtig, v2 muss es
erweitern, sobald 1.1 steht.

---

## 4. Konkrete v2-PRD-Ergänzungen (Zusammenfassung zum Einarbeiten)

**Neue/erweiterte Module:**
1. **`Stack/Baustein`** (deep module) — Onboarding-Diff, Stilllegen, Pattern-Zuordnung. *(1.1)*
2. **`Aufgaben`** (deep module) — Block-Entscheidung. *(1.2)*
3. **`Freigabe-Gate`** (deep module) — dreistufiger Block + Knopf-Zustand. *(1.3)*
4. **`Produkt-Registry` + Such-Fan-out** (I/O). *(1.8)*
5. **`Graph Projection`/`Status Reader`** erweitern um Karten-Status + die drei Graph-Klick-Verben
   (Worktree/Abzweigen/Zurückwerfen) im **Vault**. *(1.5, 1.7)*
6. **`Edge Logic`** vorbereiten auf Default-/Paar-Kanten aus dem Onboarding. *(3.3)*

**Neue User-Story-Blöcke** (heute komplett fehlend): Bausteine/Stacks/Bibliothek · Aufgaben &
Hinweise · Meilenstein-Freigabe (dreistufig) · Waisen/Unzugeordnet · Navigation & Räume · Alten
Stand inspizieren/abzweigen · Produktübergreifende Suche.

**Datenmodell festschreiben:** verbindliches `_plm`-Schema (nur die drei PLM-Fakten + Stack-Felder +
Aufgaben + Hand-/Paar-Kanten); Dotfiles als alleinige Wahrheit für Ignore/LFS; `version.json` ohne
Abstammung/Status; `VERSION_NOTES.md` als Tag-Ergebnis.

**Leitregel ergänzen:** E6-Nuance (Konzepte in Domänensprache sichtbar, git-Wörter versteckt) neben
die fünf bestehenden Leitregeln stellen. *(3.2)*

**Doku-Schuld:** `plm_software_konzept.md` auffinden/committen oder lebende §-Verweise
rekonstruieren. *(§0)*

---

## 5. Verhältnis zur bestehenden `v2-gap-analyse.md` (Reihenfolge)

`v2-gap-analyse.md` priorisiert die **Bau-Lücken** im *vorhandenen* (Sitzung-5-)Umfang: #35 Publish-
Bug, laute Ausnahme auflösbar machen (§2.1 = hier 1.9/E7), Auto-Lock beim ersten dirty Pfad,
Branch/Meilenstein-Modell grillen. **Diese bleiben die ersten Schritte** — sie machen das *gebaute
Herz* alltagstauglich.

**Dieses Dokument** zeigt darüber hinaus, dass selbst nach diesen Fixes **das halbe Konzept (E1–E33)
noch nie in die PRD und damit nie in den Bau** gelangt ist. Empfohlene Verschränkung:

1. **Zuerst** die `v2-gap-analyse.md`-Bau-Lücken schließen (laute Ausnahme auflösbar, Publish-Bug,
   Auto-Lock) — kleinster Weg zu einem teilbaren Werkzeug.
2. **Parallel** das **Branch-Strenge ⟂ main**-Reconciliation (3.1) und das **Branch/Meilenstein-
   Modell** (#30/#36) grillen — sie blockieren sowohl Aufgaben (1.2) als auch Freigabe-Gate (1.3).
3. **Dann** die große fehlende Säule bauen: **Baustein/Stack/Bibliothek (1.1)** + geführte „Neues
   Produkt"-Anlage (#29) — das Organisationsprinzip, ohne das „Bausteine" nur Ordner-Erkennung sind.
4. **Darauf** Aufgaben (1.2) → Freigabe-Gate + Waisen (1.3/1.4) → UI-Wirbelsäule (1.6) →
   Graph-Verben (1.7) → Suche (1.8).
5. Auslagern/Archiv (E36) bleibt wie gehabt zuletzt (v1-fern).

### Ein-Satz-Fazit
Die v1-PRD ist eine **exzellente PRD der 5. Sitzung** — und nur dieser; die „substantiell fehlenden
Teile" sind das **gesamte Baustein-/Stack-/Aufgaben-/Freigabe-/Navigations-/Suche-Konzept aus E1–E33**,
plus zwei echte Reconciliations (Branch-Strenge vs. `main`, „git darf durchscheinen" vs. „nie
sichtbar") und die verschollene `plm_software_konzept.md`.
