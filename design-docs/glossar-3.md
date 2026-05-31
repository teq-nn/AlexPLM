# Glossar — geschärfte Begriffe

Stand: 29.05.2026 (4. Sitzung). Fortschreibung von `glossar-2.md`. Diese Begriffe haben in Sitzung 4 eine präzise Bedeutung bekommen: die UI-Räume, die Verortung der Software und die Folgen des Mehrbenutzer-Falls. Begriffe aus Sitzung 1–3 (Werkzeug/Tool, Baustein/Stack, Sediment, Version/Commit, Wohin/Wie, Kanten, Task/Hinweis usw.) gelten unverändert weiter.

---

## Werkbank vs. Graph-Raum (die zwei UI-Räume)
Die zwei gleichwertigen, aber getrennten Räume eines Produkts:

- **Werkbank** — die **Vorderseite**. Der aktuelle Arbeitszustand als **Artefakt-Karten je Arbeitsbereich**. Hier verbringst du den Alltag; sie fragt/blockiert nichts (Design-Haltung). Spiegelt einen *einzigen* Arbeitsbereich-Zustand (E3: neueste da).
- **Graph-Raum** — der **History-Graph** als separater Raum, den du *aufsuchst*. Reines „Wohin": Orientierung, Abstammung, „bring mich zu Rev A". Nicht die Arbeitsfläche.

Merksatz: *Der Graph ist die beste Übersicht, nicht die beste Werkbank.* Zwei verschiedene Jobs — wie Gits eigene Trennung `git-gui` (Jetzt) vs. `gitk` (History).

---

## Linke Navigation = Spiegel des Ordnerbaums
Die linke Leiste der Werkbank **spiegelt den echten Ordnerbaum**, nichts daneben (keine zweite Struktur → keine Drift, wie E18 bei den Dotfiles).

- **Blatt-Ordner** tragen einen **Baustein** (Globs/Ignore/LFS/Öffnen-Aktion). Hier sitzen die Regeln.
- **Eltern-Ordner** sind **regellose Gruppen** — rein kosmetische Gliederung, kein Baustein, keine Muster.

Damit löst sich der alte Begriff **„Modul"** (§3.2 modulare Produktstruktur) von selbst auf: ein „Modul" ist nichts weiter als ein **Eltern-Ordner ohne Regeln**. Beispiel: Eine „Firmware" aus Web-Frontend (React, `software/web/`) und Server (Zephyr, `software/firmware/`) sind **zwei Bausteine in zwei Blatt-Ordnern unter einem regellosen `software/`** — nicht ein „Modul".

Default (kein Zwang): ein Arbeitsbereich ↔ höchstens ein aktiver Baustein. Teilen sich ausnahmsweise zwei denselben Blatt-Ordner, bricht nichts (Sediment hängt am Baustein-Marker-Block, nicht am Ordner).

---

## Bibliothek
Der oberste Navigationspunkt, der **„Workflows" und „Vorlagen" zusammen ersetzt**. Enthält die **geteilten** Standard-Toolstacks und einzelnen Bausteine — reine Vorlagen.

Abzugrenzen vom **Produkt-Stack**, der *im Produkt* lebt (eigener Bereich, nicht oben). Die räumliche Trennung in der UI *ist* die Anti-Drift-Regel: der Produkt-Stack ist eine **Kopie** der Bibliothek beim Anlegen, keine Live-Abhängigkeit (E16). Bibliotheks-Änderungen berühren laufende Produkte nie.

> Oberste Navigation gesamt (Sitzung 4): **Produkte · Bibliothek · Einstellungen.** „Aufgaben" ist kein Top-Level mehr — Aufgaben leben im Produkt (branch-/artefakt-gebunden, E15); „was liegt überall an" ist ein Filter, kein Zuhause.

---

## Abgeleiteter Status (Artefakt-Karte)
Der Status einer Artefakt-Karte wird **live berechnet**, nicht gespeichert — gleiche „lies zurück statt spiegeln"-Haltung wie E18, auf Status angewandt:

- **aus Git abgeleitet:** Vorhanden / Geändert / fehlt / Übernommen / Ignoriert.
- **aus Kanten abgeleitet:** Stale-Warnung (Quelle neuer als Ableitung).
- **echte PLM-Fakten, gespeichert im `_plm`:** Pflicht ja/nein, Optional/nicht benötigt, Freigegeben. Mehr nicht.

Gestrichen gegenüber §18.2: „Prüfung erforderlich" / „Datei-Bestätigung erforderlich" (hingen am gestrichenen §21, E15) und „Nicht zugeordnet" als Karten-Status (das ist das Unzugeordnet-Fach, E11 — eine Waise hat keine Karte). Die Karte ist im Alltag fast stumm, laut erst am Meilenstein-Check.

---

## Graph-Klick: die drei Verben
Klick auf einen alten Knoten **verschiebt nie still die Werkbank**. Stattdessen drei Verben:

- **Als Ordner öffnen** (Default) — schreibgeschützte Kopie *daneben* (Worktree/Export). Werkbank unberührt.
- **Von hier abzweigen** — bewusster neuer Branch; *darf* die Werkbank bewegen, weil gewollt; laufende Arbeit wird vorher ins Netz gesichert (E8).
- **Zurückwerfen** — destruktiver Sprung; hinter extra „Historie anfassen"-Bestätigung, nie der Default.

Recovery aus dem Sicherheitsnetz folgt derselben Regel: **Kopie heraus, nie Werkbank zurück.** Der nackte `checkout`/`reset` ist tiefstes „Wie" und versteckt.

---

## Meilenstein-Dialog: der kontextabhängige Knopf
Ein **einziger** Freigabe-Dialog mit nach Härte sortierter Liste (härtestes zuerst) und **einem Knopf, der seine Bedeutung wechselt** — statt drei Knöpfen nebeneinander:

- sauber → „Taggen";
- harter Block → Knopf aus, daneben der Task selbst (Erledigen/Verwerfen/Herabstufen);
- weicher Block → „Trotzdem freigeben" + protokollierter Satz;
- Warnung (Stale) → sichtbar oben, ohne Knopf-Wirkung.

`VERSION_NOTES.md` ist **Ergebnis** des Tags, nicht Vorbedingung: das Zusammenfassungs-Feld ist die Eingabe, das Taggen erzeugt die Datei. Die zirkuläre §22-Vorbedingung „Notiz vorhanden?" entfällt.

---

## Verortung: lokale App vs. Cloud-Remote
Zwei Fragen, die „lokal oder Cloud?" fälschlich verschmilzt:

- **Wo läuft die App** — **lokales Desktop-Programm**, zwingend. Braucht direkten Dateisystem-/git-Zugriff (still committen, `git status`, LFS/`migrate`, Tools per OS starten, Arbeitsbereiche beobachten, Worktrees materialisieren). Eine Browser-SaaS kann nichts davon.
- **Wo liegen die Daten** — lokale Repos (E4). **Cloud nur als Git-Remote** (Backup/Mehrgerät), **nie als Dateiablage** (Produktordner *in* Dropbox = der schlechte Weg, E2).

Seele des Konzepts: das Werkzeug sitzt *neben* deinen echten Ordnern, nicht *davor* (§8/§32) — das verlangt lokale Ausführung.

---

## Produkt-Registry
Eine schlanke Liste „welches Produkt liegt wo" — **nur Pfade, keine Inhalte**. Der einzige überlebende Organ-Rest der geparkten Workspace-Config (§9/§28.1). Kein Drift, weil sie nur auf Ordner zeigt, nicht spiegelt. Versorgt zwei Dinge: die Produktliste oben und den Such-Fan-out.

---

## Live-Fan-out (Suche ohne Index)
Produktübergreifende Suche = das Tool läuft die **Produkt-Registry** ab, öffnet jedes **erreichbare** Repo und grept **live** (Dateinamen, `_plm`, `VERSION_NOTES.md`). **Kein zentraler Index, kein Mirror** — ein Index wäre die Drift, die E8/E18 bekämpfen. Nicht erreichbare Produkte werden ehrlich gemeldet, nicht durchsucht. Bei >10 kleinen Produkten mit Text-Metadaten schnell genug.

Grenze: Suche in *abgehängten* Produkten würde einen Index erzwingen — bewusst nicht gebaut (offline ist offline; sonst via Remote). Ein Index dürfte nur *zeigen*, nie *Wahrheit sein*.

---

## Text vs. Binär (die Nebenläufigkeits-Achse)
Im Mehrbenutzer-Fall ist die entscheidende Achse **nicht** Person-gegen-Person, sondern **Text vs. Binär**:

- **Text** (KiCad-Quellen, Firmware, BOM, Doku) — git **merget**; E7s Kollisions-Logik trägt.
- **Binär/unmergebar** (`.f3d`, STEP, STL, Zip, Fotos — der LFS-Kram) — **kann nicht gemergt** werden; zwei gleichzeitige Bearbeitungen = stiller Verlust. Hier (und nur hier) braucht es Sperren.

Die Domäne trennt die Arbeit ohnehin fast: HW in `elektronik/`/`mechanik/`, SW in `firmware/`/`web/` — disjunkt entlang der Baustein-Heimaten. Echte Kollisionen bleiben selten, jetzt aus robustem Grund statt aus Solo-Annahme.

---

## Auto-Lock / Read-only-Ruhezustand / `lockable`
Der Schutz für unmergebare Binaries, über git-lfs:

- **`lockable`** — ein Attribut in `.gitattributes` (Marker-Block eines Bausteins, Tag-1 beim Onboarding, E18). Markiert Dateitypen als sperrbar.
- **Read-only-Ruhezustand** — lockable-Binaries liegen auf jeder Platte **schreibgeschützt**. Das *ist* das sichtbare Signal: read-only = frei, niemand dran. Nur diese Binaries; Text bleibt normal.
- **Sperren = der Edit-Wunsch** — willst du bearbeiten, holt das Tool die Sperre (Datei wird *für dich* beschreibbar; beim anderen „gesperrt von X seit …"). Das `git lfs lock` bleibt versteckt (E6).
- **Entsperren = der Checkpoint** — automatisch beim Push/Meilenstein (E8/E13); eine vergessene Sperre heilt sich selbst.
- **Bruch** — fremde Sperre brechen geht als ehrlicher Notausgang (Urlaub/weg), bewusst angesagt (E22).

---

## Koordination vs. Autorisierung
Scharfe Trennung für den Mehrbenutzer-Fall: Eine Sperre (oben) ist **Koordination** — „nicht gleichzeitig, sonst geht Arbeit verloren". Sie ist **keine Autorisierung** — kein „wer *darf* was". Rollen, Rechte, mehrstufige Freigaben bleiben draußen (§30/E1): zwei vertraute Kollegen, keine Hierarchie. Darum darf auch **jeder taggen** (E32). Das Vorhaben bleibt ein **Werkzeug** (kein Produkt): §9 war für genau diesen „kleinen bekannten Kreis" geparkt, der E1-Test besteht weiter.

Folge mit Zähnen: Locking ist serverkoordiniert → der **Remote wird Pflicht, sobald ein Produkt geteilt wird** (rückt E4 von optional zu verpflichtend im Mehrbenutzer-Fall).

---

## Die Git-Client-Grenze (Design-Leitplanke)
Querschnitt-Prinzip, das die Identität des Werkzeugs schützt:

> *Sobald das Werkzeug dich bittet, in Commits/Merges/Rebases zu denken, ist es zu einem Git-Client degeneriert und hat seinen Sinn verloren.*

Die Substantive des Werkzeugs sind **Produktentwicklungs**-Begriffe (Produkt, Arbeitsbereich, Artefakt, Baustein, Meilenstein, Freigabe, Task, Kante). Git ist der **Motor darunter**, auf der „Wie"-Seite versteckt (E6) — wie SQLite zu einer App, nicht die Windschutzscheibe auf den Motor. Das trennt das Werkzeug von GitKraken/Sourcetree. Ehrliche Konzession: auf der reinen Versionierungs-Scheibe überlappt es mit einem Git-Client; der Mehrwert ist die Domänenschicht plus das Verstecken des gefährlichen „Wie" — für den, der git *nicht* fließend können soll (E1).
