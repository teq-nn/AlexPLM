# Entscheidungslog — PLM-Werkzeug

Stand: 29.05.2026 (4. Sitzung). Jede Entscheidung mit Begründung und — wo zutreffend — was sie im Originalkonzept (`plm_software_konzept.md`) oder in früheren Einträgen (E1–E22, `entscheidungslog-2.md`) ersetzt oder überholt. Neu in Sitzung 4: E23–E33 — die UI-Schichten (Wirbelsäule, Navigation, Artefakt-Karte, Graph-Klick, Meilenstein-Dialog, Suche), der Grundsatz „wo lebt die Software", und die entparkte Mehrbenutzer-Frage.

---

## E23 — UI-Wirbelsäule: Werkbank vorne, Graph als gleichwertiger zweiter Raum
**Entscheidung:** Die Vorderseite eines Produkts ist der **aktuelle Arbeitszustand** — Artefakt-Karten je Arbeitsbereich (§23.2 Mitte). Der **History-Graph** ist ein gleichwertiger, aber **separater** Raum, kein Startbildschirm.
**Warum:** Im Alltag fragt/blockiert das Werkzeug nichts und du „arbeitest im Ordner" (E22/E8) — worauf du 95 % der Zeit schaust, ist der Jetzt-Zustand, nicht die Vergangenheit. Der Graph ist reines „Wohin" (Glossar): ein Orientierungsraum, den man *aufsucht*. E6s „Graph = beste Übersicht" meint den besten *Überblicks*raum, nicht die beste *Arbeitsfläche* — zwei verschiedene Jobs. Prior Art stützt das hart: Gits eigene Werkzeuge trennen die Jobs physisch (`git-gui` = Commit/Jetzt, `gitk` = History), und selbst graph-zentrierte Clients (Sourcetree, GitKraken) halten eine separate „File-Status"-Ansicht für die laufende Arbeit.
**Bezug:** bestätigt §23.2 (Artefakt-Karten in der Mitte), ordnet §24 (Versionsbaum) als eigenen Raum ein. Instanz von E22.

---

## E24 — Linke Navigation spiegelt den echten Ordnerbaum
**Entscheidung:** Die linke Navigation der Werkbank gliedert nach **Arbeitsbereichen** und spiegelt schlicht den **echten Ordnerbaum** — nichts daneben. **Eltern-Ordner sind regellose Gruppen** (kein Baustein, keine Globs/Ignore/LFS); **Bausteine sitzen nur in Blatt-Ordnern**. Die Baustein-Maschinerie (Onboarding, Erweitern/Austauschen, Sediment) lebt in einem separaten Stack-Verwaltungsbereich *im Produkt*, nicht in der täglichen linken Leiste.
**Warum:** Der Arbeitsbereich ist der *harte Anker* (E11), der Baustein das *Stilllegbare* (E17) — die Alltags-Achse muss das Dauerhafte sein. Stresstest E17: Wird PlatformIO stillgelegt, werden alte `.bin` zu Waisen *in* `firmware/`; gliederte die Nav nach Baustein, fielen genau diese Waisen aus der Ansicht, sobald der Baustein weg ist. Nach Arbeitsbereich bleiben sie sichtbar, wo das Sediment ohnehin liegt. Spiegelt die Nav den echten Baum, gibt es keine zweite, drift-anfällige Struktur (gleiche Anti-Drift-Logik wie E18).
**Löst auf:** **§3.2** (modulare Produktstruktur „Main PCB / Firmware …") — ein „Modul" ist jetzt einfach ein **Eltern-Ordner ohne Regeln**, kein eigenes Objekt, kein neuer Begriff. Beispiel: „Firmware" aus Web-Frontend (React) + Server (Zephyr) = zwei Arbeitsbereiche/Bausteine unter einem regellosen Eltern-Ordner `software/`.
**Default, kein Zwang:** „Ein Arbeitsbereich ↔ höchstens ein aktiver Baustein" ist der Normalfall. Teilen sich zwei Tools denselben Blatt-Ordner, bricht nichts (E18: Sediment hängt am Baustein-Marker-Block, nicht am Ordner); der Zweit-Belegung-Fall wird oft ein **output-förmiger Baustein** sein (Glossar: bringt nur einen Artefakt-Glob, keine eigenen Schutzregeln).

---

## E25 — Oberste Navigation: Produkte / Bibliothek / Einstellungen
**Entscheidung:** Die oberste Ebene schrumpft auf drei Punkte: **Produkte**, **Bibliothek**, **Einstellungen**.
**Warum:** „Workflows" ist nach E16 tot; „Workflows" und „Vorlagen" waren faktisch dasselbe → beide werden zur **Bibliothek** (die geteilten Standard-Toolstacks und Bausteine, reine Vorlagen). „Aufgaben" als Top-Level fliegt raus: Aufgaben sind nach E15 branch-/artefakt-gebunden und leben *im Produkt*; „was liegt überall an" ist ein Filter, kein Zuhause. Der **lebende Produkt-Stack** wohnt *im Produkt* (eigener Bereich) — nicht oben. Diese Trennung muss fühlbar sein, weil sie die Anti-Drift-Regel *ist*: der Produkt-Stack ist Kopie, keine Live-Abhängigkeit (E16). Läge beides am selben Ort, erwartete man fälschlich, dass Bibliotheks-Änderungen laufende Produkte mitändern.
**Überholt:** **§23.1** (Produkte/Workflows/Aufgaben/Vorlagen/Einstellungen).

---

## E26 — Artefakt-Status wird abgeleitet, nicht gespeichert
**Entscheidung:** Der Status einer Artefakt-Karte wird **live abgeleitet** aus Git + Kanten + Waisen-Check. Im `_plm` gespeichert wird nur, **was Git nicht kennen kann**: **Pflicht ja/nein**, **Optional/nicht benötigt**, **Freigegeben**.
**Warum:** E5 (Status ist faktisch `git status`), E8/E18 (`_plm` besitzt nur, was Git nicht kennt) auf den Karten-Status angewandt → kann gar nicht erst driften. Die zehn §18.2-Status zerfallen sauber: *abgeleitet aus Git* (Vorhanden / Geändert / fehlt / Übernommen / Ignoriert), *abgeleitet aus Kanten* (Stale-Warnung = altes „Aktualisierung erforderlich"), *echte PLM-Fakten* (Pflicht/Optional/Freigabe). Karte im Alltag fast stumm, laut erst am Meilenstein-Check (E22).
**Überholt/streicht:** **§18.2** „Prüfung erforderlich" und „Datei-Bestätigung erforderlich" (hingen an §21, das E15 ersatzlos gestrichen hat); „Nicht zugeordnet" ist kein Karten-Status, sondern das Unzugeordnet-Fach (E11) — eine Waise hat *keine* Karte. **§28.4**: `version.json` führt keinen `"status": "changed"` mehr. **§25**: der dauerhafte „Änderungsnotiz: vorhanden"-Indikator fällt weg (nach E13 entstehen Notizen gruppiert beim Tag, nicht pro Datei).

---

## E27 — Graph-Klick: inspizieren/materialisieren, nie still verschieben
**Entscheidung:** Klick auf einen alten Knoten **verschiebt nie still die Werkbank**. Drei Verben hinter dem Knoten:
- **Als Ordner öffnen** (Default) — alter Stand als *separater, schreibgeschützter* Ordner neben der Werkbank (Worktree/Export, E3). Laufende Arbeit unberührt.
- **Von hier abzweigen** — bewusster `checkout -b` vom Tag; *darf* die Werkbank bewegen, weil ausdrücklich gewollt; laufende Arbeit wird vorher ins Sicherheitsnetz gesichert (E8).
- **Zurückwerfen** — der destruktive Sprung auf einen alten Stand; nur hinter extra „Historie anfassen"-Bestätigung, **nie der Default**.
Der nackte `checkout`/`reset` der einen Werkbank taucht auf dem Happy Path **nicht** auf.
**Warum:** „bring mich zu Rev A" (§24/E6) naiv als `checkout` *bewegt* deine einzige Werkbank und gefährdet ungespeicherte Arbeit — exakt die „Wie"-Beschwörung, die E6 versteckt, getarnt als Ein-Klick-Knopf (= Fassade nach E22). Recovery aus dem Netz (E8) folgt derselben Logik: **Kopie heraus, nie Werkbank zurück** — wie E9 (behalten, nie umschreiben) und E17 (label-only, additiv). In einem Wegwerf-Prototyp gegen die naive „springt"-Variante geprüft und bestätigt.
**Verfeinert:** §24, Glossar „Wohin vs. Wie". Instanz von E22.

---

## E28 — Meilenstein-Dialog: ein Dialog, ein kontextabhängiger Knopf
**Entscheidung:** Der Freigabe-/Tag-Dialog zeigt offene Punkte in **einer** nach Härte sortierten Liste (härtestes zuerst). Statt drei Knöpfen **ein Knopf, der Beschriftung und Schärfe wechselt**:
- alles sauber → ruhiger „Taggen";
- **harter Block** (E19.3) → Knopf aus, daneben der Task mit seinen drei Auswegen (Erledigen/Verwerfen/Herabstufen) — kein Begründungs-Schlupfloch;
- **weicher Block** (E19.2) → „Trotzdem freigeben" + ein protokollierter Satz (§22.1);
- **Warnungen** (E19.1, Stale) stehen sichtbar oben, ohne Knopf-Wirkung.
**Warum:** Drei gleichberechtigte „Freigeben"-Varianten nebeneinander ebneten die Härte-Staffelung von E19 optisch wieder ein — zurück zum flachen §22.1-Knopf, den E19 zerlegt hat. Der wandernde Knopf hält die Staffelung fühlbar.
**Überholt:** die §22-Vorbedingung „wurde `VERSION_NOTES.md` erzeugt?" — zirkulär. Nach E13 ist die Notiz ein **Ergebnis** des Tags: das Zusammenfassungs-Feld *ist* die Eingabe, das Taggen *erzeugt* daraus `VERSION_NOTES.md`. **Verfeinert §22/§22.1**, macht E19 sichtbar.

---

## E29 — Verortung: lokales Desktop-Programm (Pflicht), Cloud nur als Remote
**Entscheidung:** Die **Anwendung** ist ein **lokales Desktop-Programm** — sie kann nichts anderes sein. **Daten** sind lokale Repos (E4). **Cloud** tritt nur in einer Rolle auf: als optionaler **Git-Remote** (Backup/Mehrgerät) — nie als Dateiablage.
**Warum:** Trennung zweier Fragen, die „lokal oder Cloud?" verschmilzt: *Wo läuft die App* vs. *wo liegen die Daten*. Die App muss lokal sein, weil ihre Kernaufgaben direkten Dateisystem-/git-Zugriff brauchen: Explorer-Änderungen still committen, `git status`/`.gitattributes`/`git lfs migrate` fahren (E5/E6), Dateipfade ans OS übergeben, um KiCad/Fusion zu starten (§13), Arbeitsbereiche beobachten und Worktrees materialisieren (E3/E10/E11). Eine browserbasierte SaaS-PLM kann nichts davon. Der Charme des Konzepts — „neben deinen echten Ordnern, nicht davor" (§8/§32) — *verlangt* lokale Ausführung.
**Festigt E4**, **überholt** den §8-Pfad „Produktordner *in* einem Sync-Ordner" als gesegneten Weg (E2 hatte ihn schon als den schlechten markiert: Voll-Upload je Version).

---

## E30 — Produktübergreifende Suche per Live-Fan-out, kein Index
**Entscheidung:** Bei vielen Produkten (>10 real) bekommt die Suche eine **produktübergreifende** Zeile — umgesetzt als **Live-Fan-out**: das Tool läuft die Produktliste ab, öffnet jedes *erreichbare* Repo und grept live über Dateinamen/`_plm`/`VERSION_NOTES.md`. **Kein zentraler Index**, kein Mirror. Nicht erreichbare Produkte werden ehrlich gemeldet („3 von 14 offline, nicht durchsucht").
**Warum:** Ein zentraler Such-Index wäre eine zweite Datenkopie außerhalb der Repos — exakt die Drift, die E8/E18 bekämpfen. Live-Fan-out = „lies zurück statt spiegeln" (E18), null Staleness. Bei dieser Größenordnung mit reinen Text-Metadaten rechnerisch billig. Den Fan-out fährt eine schlanke **Produkt-Registry** (Liste „welches Produkt liegt wo", nur Pfade, keine Inhalte → kein Drift) — der einzige überlebende Organ-Rest der geparkten Workspace-Config (§9/§28.1 hatte schon eine `products`-Pfadliste).
**Grenze (ehrlich):** Suche in *abgehängten* Produkten würde einen Index erzwingen. Bewusst nicht gebaut: offline ist offline (bzw. via Remote, E4). Ein Index dürfte allenfalls *zeigen*, nie *Wahrheit sein*. **Verfeinert §26** (Suche/Filter), entparkt §9 nur in diesem einen Punkt.

---

## E31 — Mehrbenutzer (§9 entparkt): Text mergen, Binär sperren — Auto-Lock als Read-only
**Entscheidung:** Mehrere Nutzer (real: ein HW- und ein SW-Entwickler) klonen denselben Remote; jede lokale App liest dasselbe Repo. Die Nebenläufigkeits-Achse ist **Text vs. Binär**, nicht Person vs. Person:
- **Text** (KiCad-Quellen, Firmware, BOM, Doku) → git **merget**; E7s Kollisions-Logik greift.
- **Binär/unmergebar** (`.f3d`, STEP, STL, Zip, Fotos — der LFS-Kram aus E2) → **gesperrt**.
Sperrung via git-lfs **`lockable`** (eine Zeile im `.gitattributes`-Marker-Block eines Bausteins, gesetzt beim Onboarding/Tag-1, E18). Verhalten: lockable-Binaries liegen überall im **Read-only-Ruhezustand** (= das sichtbare Signal — schreibgeschützt heißt „frei"); der **Edit-Wunsch** holt die Sperre (Datei wird *für dich* beschreibbar, beim anderen „gesperrt von X seit …"); **Entsperrung am Checkpoint** (Push/Meilenstein, E8/E13) → eine vergessene Sperre heilt sich selbst; **Bruch fremder Sperren** als ehrlicher Notausgang (E22). Das `git lfs lock`/`unlock` bleibt versteckt (E6).
**Warum:** Die Domäne schenkt die Nebenläufigkeit fast geschenkt — HW lebt in `elektronik/`/`mechanik/`, SW in `firmware/`/`web/`, disjunkt entlang der Baustein-Heimaten (E24). E7s „Kollisionen selten" überlebt — jetzt aus robustem Grund (Domänentrennung) statt aus der brüchigen Solo-Annahme. Prior Art: git-lfs macht lockable-Dateien lokal automatisch read-only; genau das Verhalten, das CAD-/PDM-Leute aus Check-out/Check-in kennen, ohne dass das Werkzeug ein Zeremoniell verlangt.
**Bleibt draußen (§30/E1):** Rollen, Rechte, mehrstufige Freigaben. Eine Sperre ist **Koordination, keine Autorisierung** — zwei vertraute Kollegen, keine Hierarchie. **Bleibt Werkzeug, kein Produkt** (E1-Test besteht weiter; §9 war für genau „kleinen bekannten Kreis" geparkt).
**Folge (Zähne):** Der **Remote wird Pflicht, sobald ein Produkt geteilt wird** (Locking ist serverkoordiniert) — rückt E4 von „später/optional" zu „verpflichtend im Mehrbenutzer-Fall". **Entparkt §9** im Kern. Der alte Punkt „derselbe Branch an zwei Orten" (Sitzung-2-Offenliste) ist damit für Binaries gelöst (Lock) und für Text auf E7 zurückgeführt.

---

## E32 — Gemeinsamer Meilenstein: ein bewusster Akt auf `main`, jeder darf, mit Warnung
**Entscheidung:** Der Meilenstein bleibt **ein einzelner bewusster Akt auf `main`** und erfasst, was zum Tag-Zeitpunkt dort liegt; unfertige Arbeit auf Branches ist schlicht nicht drin (E15). **Jeder im Kreis darf taggen** (kein Rollen-Konzept, §30/E1). Der dreistufige Block (E19) wird **produkt- und personenübergreifend**: ein offener blockierender Task der einen Person hält auch die andere am Taggen (E19.3). Zusätzlich zeigt der Dialog eine **personenübergreifende Warnung** (E19.1, kein Block): „du taggst auch X' Stand mit; X hat zuletzt vor 10 Min an der Firmware gepusht — sicher?"
**Warum:** „Rev B" ist jetzt der ganze Produktstand zweier Leute. „Jeder darf" ist nicht „blind": die Warnung verhindert das versehentliche Mit-Taggen fremder frischer Arbeit, ohne zu verbieten — E22 über zwei Köpfe.
**Verfeinert E19/E15** für den Mehrbenutzer-Fall.

---

## E33 — Design-Leitplanke: die Git-Client-Grenze
**Entscheidung (Querschnitt-Prinzip):** *In dem Moment, wo das Werkzeug dich bittet, in Commits/Merges/Rebases zu denken, ist es zu einem Git-Client degeneriert und hat seinen Sinn verloren.* Die Substantive des Werkzeugs sind **Produktentwicklungs**-Begriffe (Produkt, Arbeitsbereich, Artefakt, Baustein, Meilenstein, Freigabe, Task, Abgeleitet-von-Kante); Git ist der **Motor darunter**, auf der „Wie"-Seite versteckt (E6) — wie SQLite zu einer App, nicht wie die Windschutzscheibe auf den Motor.
**Warum:** Macht explizit, was E6/E22 tragen, und zieht die Grenze, die das Werkzeug von GitKraken/Sourcetree trennt. Ehrliche Konzession: für die reine Versionierungs-Scheibe überlappt es mit einem Git-Client; der Mehrwert ist die **Domänenschicht** obendrauf und das **Verstecken des gefährlichen „Wie"** — für den, der git *nicht* fließend können soll (E1: Hardware-Entwickler). Das ist die tragende Wette des Konzepts. Besonders scharf im Mehrbenutzer-Fall (E31): Kollaboration drückt Richtung sichtbarer git-Oberflächen (Pull/Push-Status, Konflikte zwischen Personen) — genau die müssen versteckt/übersetzt bleiben, sonst kippt das Werkzeug in einen Git-Client.

---

## Offene / verschobene Punkte (Stand 4. Sitzung)
- **Physisches Aufräumen stillgelegter Bausteine** (E17) — getrennte, bewusste „Historie anfassen"-Aktion, weiter nicht im Detail.
- **Git-Reihenfolge-Heuristik für Kanten-Vorschläge** (E21) — im Eis.
- **Mehrbenutzer-Feinheiten:** Anzeige „wer arbeitet gerade woran" über reine Lock-Signale hinaus; Verhalten der Sperre über mehrere Branches (git-lfs-Sperre gilt branchübergreifend) — angerissen, nicht ausgegrillt.
- **Lese-Oberfläche auf dem Remote** (Status/Notizen unterwegs ohne Daten) — nur falls je „ohne Repos bedienen" gewünscht; aktuell verneint (E29). Gälte wie ein Index: nur zeigen, nie Wahrheit.
- **LFS-Host-Ökonomie** — jetzt dringlicher, da Remote im Mehrbenutzer-Fall Pflicht (E31). Selbstgehostetes Gitea/Forgejo (mit Lock-API) ist der billige Dauerweg.
- **UI-Detailschichten:** §25 Artefakt-Karten-Detailbereich, §26 Filter-Feinheiten — Kern steht (E23–E28), Feinschliff offen.
