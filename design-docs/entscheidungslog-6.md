# Entscheidungslog — Werkbank

Stand: 07.06.2026 (Konzept-Grill-Review zu PRD #128). Fortsetzung von `entscheidungslog-5.md`
(E1–E47). Neu in dieser Runde: E48–E56 — die im Grill-Review von #128 geschärften Restpunkte.
Hier festgehalten ist der für Issue #131 baurelevante Eintrag **E51**; die übrigen Einträge der
Runde werden in ihren eigenen Slices ergänzt.

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
