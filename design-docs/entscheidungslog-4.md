# Entscheidungslog — PLM-Werkzeug

Stand: 29.05.2026 (5. Sitzung). Jede Entscheidung mit Begründung und — wo zutreffend — was sie im Originalkonzept (`plm_software_konzept.md`) oder in früheren Einträgen (E1–E33, `entscheidungslog-3.md` und früher) ersetzt oder überholt. Neu in Sitzung 5: E34–E41 — der Mehrbenutzer-Ast wird zuende gegrillt (Topologie, die tragende Binär-Invariante, Bloat-Deckelung samt Archiv-Auslagern, stiller Sync), dazu drei bislang ungegrillte Programmflächen (Import, Auto-Commit-Schleife, Kanten anlegen). Drei Realitätsprüfungen sind eingearbeitet (git-lfs-Lock-Semantik, Forgejo/Gitea-Tragfähigkeit, KiCad-Merge-Verhalten).

---

## E34 — Mehrbenutzer-Topologie: beide auf `main`, kein personengebundener Dauer-Branch
**Entscheidung:** Im geteilten Fall committen beide lokal direkt auf dasselbe `main` und gleichen per Pull/Push ab. Es gibt **keine** lang lebenden, personengebundenen Arbeits-Branches. Branches bleiben das, was E27/E32 festlegen: bewusste Varianten/Experimente, kein „mein Bereich".
**Warum:** Passt zum „du arbeitest im Ordner, das Werkzeug committet still" (E5/E22) — ein Solo-artiger Fluss, nur zu zweit. Divergierende Merges werden dadurch häufig, aber **harmlos**: sie berühren fast immer nur Text (E7 merget) und Binärdateien sind dank der Invariante (E35) zum Pull-Zeitpunkt frei. Personengebundene Branches täten das Gegenteil — seltene, große Merges mit höherer Trefferwahrscheinlichkeit auf eine *noch offen gehaltene* Binärsperre — und würden Branches zu einem Personen-Konzept machen, das E27/E32 bewusst vermeiden.
**Verfeinert E31/E32.** Schließt den Sitzung-2-Offenpunkt „derselbe Branch an zwei Orten" auf der Topologie-Seite.

---

## E35 — Die tragende Binär-Invariante: Freigabe-Push = Entsperren; Sicherungs-Push fürs Backup
**Entscheidung:** Die gesamte Sicherheit des Binär-Mehrbenutzer-Falls ruht auf **einer** Invariante: *Eine gesperrte Binäränderung darf den geteilten Stand nicht erreichen, solange die Sperre gehalten wird.* Daraus zwei getrennte Push-Arten:
- **Freigabe-Push** — bringt die fertige Binärdatei auf den geteilten `main`-Stand und **ist** das Loslassen der Sperre („ich bin fertig damit"). Eine Binärdatei unter aktiver Sperre wird **nie zwischendurch** auf den geteilten Stand gepusht.
- **Sicherungs-Push** — spiegelt lokale Zwischen-Commits (inkl. der halbfertigen Binärdatei) in einen **persönlichen** Backup-Bereich auf dem Remote (eigener Ref/Namespace, Sicherheitsnetz E8). Backup ja, Freigabe nein.
Dazu die Selbstheilung aus E31 mit konkretem Auslöser: an jedem Checkpoint löst das Werkzeug automatisch jede gehaltene Sperre, deren Pfad lokal **sauber** ist (committet, gepusht, keine offene Bearbeitung).
**Warum (Realitätsprüfung):** git-lfs-Locks sind **pfadbasiert und branchübergreifend**; eine gesperrte Datei lässt sich **nicht per `git merge`** zusammenführen, und der Merge kippt schon, wenn er die Datei nur *berührt*. Naiv würde das eine Sperre von „Koordination" (E31) in einen **Sync-Blocker** verwandeln: der SW-Entwickler (reiner Text in `firmware/`) könnte an einem Pull scheitern, nur weil der HW-Entwickler eine geänderte `.f3d` gesperrt hält. Die Invariante killt das strukturell: was beim Pull je sichtbar wird, ist schon gepusht — also schon entsparrt; der Merge berührt nur freie Dateien. `git lfs unlock` ist ein **eigener, expliziter** Befehl (kein Auto-Geschenk von git-lfs), also muss das Werkzeug „unlock at push" selbst leisten und absichern.
**Ehrlicher Preis:** Ein HW-Zwischenstand liegt erst beim Loslassen auf dem geteilten Stand — der Sicherungs-Push fängt das Datenverlust-Risiko ab, ohne die Invariante zu brechen.
**Festigt/operationalisiert E31**, hängt an E34.

---

## E36 — Binär-Bloat gedeckelt; reversibles Auslagern pro Meilenstein-Alter statt Löschen
**Entscheidung:** Zwei Stufen.
1. **Deckelung (folgt aus E35):** Auto-Saves machen nur *lokale* Commits (E5 unangetastet). Die Binärdatei erreicht den **LFS-Store erst beim Freigabe-Push**. Damit landet **eine** Voll-Version pro Meilenstein im Store statt eine pro Save. Lokaler Hausputz via `git lfs prune` (gibt nur den eigenen Cache frei, fasst Server/Historie nicht an).
2. **Langfrist-Wachstum:** Statt alte Blobs destruktiv zu löschen, **Auslagern pro Meilenstein-Alter** („Binärinhalte älter als X auslagern"). Das Werkzeug exportiert die schweren LFS-Inhalte alter Meilensteine in ein Archiv beim Nutzer (NAS/Kaltplatte) und entfernt **nur diese OIDs** vom Server. Zurückholen ist trivial (LFS-Blobs sind inhaltsadressiert): Archiv einspeisen → Blobs wieder im Store → „Als Ordner öffnen" (E27) funktioniert wieder.
**Warum (Realitätsprüfung):** LFS hält das *git*-Repo schlank (Pointer), aber jede **gepushte** Binärversion liegt *für immer* im LFS-Store; eine oft geänderte Datei bläht den Store dauerhaft. Auslagern ehrt die eigenen Leitregeln — **behalten, nie umschreiben** (E9), **Kopie heraus, nie zurück** (E27): die **git-Historie wird nie angefasst** (Commits, Tags, `VERSION_NOTES.md`, LFS-*Pointer* bleiben), nur der schwere *Inhalt* wandert von „heißem Server" auf „kaltes Archiv". Reiner Batch-Download ohne Server-Entlastung wäre dagegen die zweite Datenkopie, die E8/E18 bekämpfen — Drift, kein Gewinn.
**Ehrlicher Preis:** Archiv-Zip verloren = diese alten Binärstände wirklich weg. Verantwortung liegt bei der Ablage-Disziplin des Nutzers, nicht bei einer Zerstör-Aktion des Werkzeugs — muss beim Auslagern klar angesagt werden. Ausgelagerte Knoten tragen im Graph ehrlich „Binärinhalt ausgelagert (Archiv vom …)", gleiche Haltung wie die Offline-Meldung der Suche (E30).
**Granularität:** pro Meilenstein-Alter (deckt ~95 %, simples mentales Modell); gezieltes Einzel-Artefakt-Auslagern ist späterer Feinschliff.
**Bau bleibt v1-fern** (Bloat über Jahre ist kein Tag-1-Problem); die Richtung steht. **Ersetzt** das in Sitzung 4 angerissene „hartes Server-Vergessen" durch reversibles Auslagern.

---

## E37 — Mehrbenutzer-Sichtbarkeit endet bei den Lock-Signalen, kein Präsenzdienst
**Entscheidung:** „Wer arbeitet gerade woran" wird **nicht** über die Lock-Signale hinaus gebaut. Read-only-Ruhezustand (= frei) und „gesperrt von X seit …" (E31) leisten das schon; ein kleines Live-Panel „fremde Sperren" ist nur ein Lesen von `git lfs locks`.
**Warum:** Ein Präsenz-/Heartbeat-Dienst wäre eine zweite Wahrheit neben git — exakt die Drift, die E8/E18/E30 bekämpfen — und brächte ein neues Server-Organ ohne echten Mehrwert. „Lies zurück statt spiegeln" (E18) auf die Mitarbeiter-Sichtbarkeit angewandt.
**Schließt** den Offenpunkt „Anzeige wer arbeitet woran über reine Lock-Signale hinaus" (verneint).

---

## E38 — Import bestehender Projekte: `lfs migrate`-Gate
**Entscheidung:** Bestehenden Ordner zum Produkt machen ist ein eigener Erststeg-Flow: Ordner zeigen → `git init` falls nötig → Blatt-Ordner-Bausteine erkennen → `.gitattributes`-Marker schreiben (E18/E24) → erster Commit. Liegen Riesen-Binaries **schon in der git-Historie**, braucht es `git lfs migrate` = **Historie umschreiben** — das nur hinter der „Historie anfassen"-Bestätigung (E27) und **nur bei frischem/ungeteiltem Repo**. Hat das Projekt schon geteilte Klone, **verweigert** das Werkzeug die Umschreibung.
**Warum:** Der häufigste reale Einstieg ist „ich habe schon einen KiCad-Ordner". Der Solo-Fall (nie in git) ist sauber: init + track + commit. Der gefährliche Fall ist nachträgliches LFS über schon committete Riesendateien — `lfs migrate import` schreibt Historie um und vergiftet geteilte Klone. Das Gate trennt beide hart und hält die destruktive Operation hinter derselben bewussten Schwelle wie E27.
**Schließt** die ungegrillte Programmfläche „bestehendes Projekt importieren".

---

## E39 — Auto-Commit-Schleife: still, entprellt, maschinen-betextet
**Entscheidung:** Auto-Commit löst **entprellt bei „Save beruhigt sich"** aus (Watcher sieht Schreibvorgang, wartet ein paar Sekunden Ruhe), nie pro Tastendruck. Die Commit-Message ist **maschinell und langweilig** („auto: elektronik/… , Zeitstempel"). Der Nutzer schreibt auf dem Happy Path **nie** eine Commit-Message. Commits sind im Alltag **unsichtbar**; der Nutzer sieht **Stände im Graph**, nicht Commits. Menschlicher Text entsteht nur am Meilenstein (E28 → `VERSION_NOTES.md`).
**Warum:** E5 sagt „Status = git status, still committen", ließ aber Wann/Wie-oft/Sichtbarkeit offen — genau hier kann die Git-Client-Grenze (E33) lecken. Verlangte das Werkzeug Commit-Messages, wäre es ein Git-Client. Entprellen verhindert Commit-Spam pro Tastendruck; Unsichtbarkeit hält die Substantive bei „Stand/Meilenstein" statt „Commit".
**Operationalisiert E5**, macht E33 an der heikelsten Stelle konkret.

---

## E40 — Kanten (Abgeleitet-von) v1 rein manuell, Opt-in
**Entscheidung:** Kanten werden für v1 **rein manuell** gezogen — eine billige Geste auf der Artefakt-Karte („abgeleitet von …", Picker oder Karte-auf-Karte). Die Stale-Warnung (E26) existiert **nur**, wo eine Kante von Hand gezogen wurde; keine Kante = keine Warnung. Kanten sind **Opt-in-Mehrwert, keine Pflicht**; viele Nutzer ziehen wenige oder keine, und das ist in Ordnung.
**Warum:** Die Git-Reihenfolge-Heuristik für Kanten-Vorschläge (E21) liegt auf Eis. Ohne sie ist Handarbeit die ehrliche Option — kein gefälschter Graph, keine Wand. „Keine Kante = keine Warnung" ist konsistent mit dem abgeleiteten Status (E26): das Werkzeug behauptet nur, was es weiß.
**Schließt** die ungegrillte Fläche „Kanten anlegen"; **E21 bleibt geparkt**.

---

## E41 — Täglicher Sync still mit lauter Ausnahme; Setup-Zeremonie darf git-nah bleiben
**Entscheidung:** Zwei verschiedene Momente, zwei verschiedene Haltungen.
- **Einrichtungs-Zeremonie (einmalig pro Produkt):** Server anbinden, ersten Push machen, Kollegen einladen — darf entspannt **git-nah** bleiben. Selten, kaum git-gefärbt, risikoarm; späteres Hübschermachen kostet fast nichts.
- **Täglicher Netz-Sync (Push/Pull):** läuft **still im Hintergrund**, wie der Commit (E39). Das Werkzeug pullt beim Öffnen/in Ruhephasen und pusht an den Checkpoints (Sicherungs-Push laufend, Freigabe-Push am Meilenstein, E35). Der Nutzer sieht „aktuell / X arbeitet gerade an Y / dein Stand ist gesichert", **nie** „push/pull".
- **Laute Ausnahme:** Bei einem echten, unauflösbaren Widerspruch **hält der stille Sync an** und fragt in **eigener** Sprache („dein und X' Gehäuse-Stand widersprechen sich — welcher gilt?") — **nie** git-Konflikt-Marker.
**Warum:** Teilen ist einmalig (Zeremonie billig, git-Sprache dort vertretbar), aber E34 (beide auf `main`) macht **Pull-dann-Push zum täglichen Akt**. „Git-Sprache kaschiert man später" scheitert genau hier: einen *täglichen* Reflex trainiert man dem Nutzer ein, und gegen eine eingeübte Gewohnheit kämpft das spätere Verstecken. **Nie zeigen ist leichter als später wieder verstecken** — der Kern von E33 als Querschnitt-Leitplanke, nicht als Feature. Der tägliche Commit ist über E39/E5 schon still; E41 zieht den *Sync* (Netz-Teil) auf dieselbe Linie, sodass git-Sprache aus dem Alltag fast vollständig verschwindet, ohne je gezeigt und wieder versteckt werden zu müssen.
**Grenze (ehrlich):** Sobald zwei still pushen/pullen, werden Konflikte möglich, und bei KiCad-Quellen ist der Konflikt nachweislich **datei-zerstörend** („Missing („-Korruption). Ein stiller Sync darf so etwas nicht durchrauschen lassen — daher die laute Ausnahme. Das ist das Minimum, das #3 aus den abgewählten #1/#2 unvermeidlich hereinzieht; die volle Konflikt-/Lockable-Frage bleibt geparkt.
**Operationalisiert E33/E34 für den Alltag**, hängt an E35/E39.

---

## Realitäts-Festigung (Sitzung 5)
- **git-lfs-Locks** sind pfadbasiert und branchübergreifend; gesperrte Dateien sind nicht mergebar, `unlock` ist ein eigener expliziter Befehl. → trägt E34/E35.
- **Forgejo/Gitea** tragen die git-lfs-**Lock-API** nativ (vom Git-LFS-Miterfinder bestätigt), laufen als einzelnes Go-Binary auf ~512 MB RAM, ~7,49–12,49 €/Monat auf einem VPS. → **LFS-Host-Ökonomie erledigt:** selbstgehostetes Forgejo ist der billige Dauerweg. Kleine Falte: `locksverify` muss aktiv eingeschaltet werden; hinter manchen Proxy-Setups (fcgi+unix) gibt es Stolperstellen (Implementierungsdetail).
- **KiCad-Quellen** (`.kicad_sch`/`.kicad_pcb`) sind *nominell* Text, mergen aber katastrophal — Merges können die Datei zerstören („Missing („-Fehler), selbst KiCads eigene Maintainer kämpfen mit Massen-Konflikten. → Die Achse Text-vs-Binär (E31) hat einen unbeachteten dritten Eimer: **nominell Text, faktisch unmergebar.** Begründet die laute Ausnahme in E41; volle Konsequenz (lockable nach Mergebarkeit statt nach Binär-Natur) ist offen.

---

## Offene / verschobene Punkte (Stand 5. Sitzung)
- **Nominell-Text-aber-unmergebar (KiCad)** — die volle Konsequenz aus dem Realitätsbefund: `lockable` sollte sich nach **Mergebarkeit** richten, nicht nach „Binär vs. Text" (würde E31 verfeinern). Bewusst nicht voll gegrillt; E41 deckt nur die Alltags-Ausnahme ab. Zusammen mit der allgemeinen **Merge-Konflikt-UX** (was sieht der Nutzer im Konfliktfall, ohne git-Marker — Säuretest für E33) das nächste lohnende Grill-Paket.
- **Physisches Aufräumen stillgelegter Bausteine** (E17) — getrennte, bewusste „Historie anfassen"-Aktion, weiter nicht im Detail.
- **Git-Reihenfolge-Heuristik für Kanten-Vorschläge** (E21) — auf Eis (E40 baut v1 ohne sie).
- **Archiv-Auslagern (E36) im Bau** — Richtung steht, v1-fern; gezieltes Einzel-Artefakt-Auslagern (statt nur pro Meilenstein-Alter) ist späterer Feinschliff.
- **Sperren-Feinheiten:** Bruch fremder Sperren als Notausgang (E31) — Ablauf nicht im Detail; `locksverify`-Aktivierung als Setup-Schritt.
- **App-Tech-Stack** (Tauri/Electron o. ä.) — Implementierung, bewusst außerhalb des Konzept-Grillens.
- **UI-Detailschichten:** §25 Artefakt-Karten-Detailbereich, §26 Filter-Feinheiten — Kern steht, Feinschliff offen.
- **Lese-Oberfläche auf dem Remote** — weiter verneint (E29).
